//! The road network in world space, assembled from the centre vector tile and its 8 neighbours.
//!
//! Why: a bridge, a ramp or a graded road bed crosses tile borders, and each tile used to plan its own piece, so
//! the pieces disagreed. Here every source tile contributes its geometry clipped to its own square (never the
//! overlapping buffer), pieces that meet on a border are joined into one edge, and everything downstream
//! (levels, deck solver, grading) works on whole edges. The result at a point depends only on the data within a
//! bounded distance of it, so two tiles that both see that point compute the same thing.
//!
//! Coordinates: metres, +x east, +y south, origin at the NW corner of the centre tile. The centre tile's width in
//! metres is used for all nine tiles (Web Mercator is conformal; the error over 3 tiles is far below a decimetre).

use super::RoadClass;
use ed_data::mvt::{self, GeomType};
use ed_geo::tile::TileId;

/// Pieces whose end points on a tile border are this close (m) are the same road.
const JOIN_M: f64 = 1.5;

#[derive(Debug, Clone)]
pub struct Edge {
    pub class: RoadClass,
    /// OSM `layer` as given by the tile (0 when absent).
    pub layer: i8,
    pub bridge: bool,
    pub tunnel: bool,
    pub ramp: bool,
    pub oneway: bool,
    pub paved: bool,
    pub cycleway: bool,
    pub pts: Vec<[f64; 2]>,
}

impl Edge {
    /// Vertical level used for crossings: >0 is a deck, <0 is underground, 0 is the ground.
    /// `layer` on an ordinary ground road is ignored (embankments carry it without being bridges).
    pub fn level(&self) -> i8 {
        if self.bridge {
            self.layer.max(1)
        } else if self.tunnel {
            self.layer.min(-1)
        } else {
            0
        }
    }

    pub fn length(&self) -> f64 {
        self.pts.windows(2).map(|w| dist(w[0], w[1])).sum()
    }

    /// Roads that take part in the height solve: graded ground roads and every bridge or tunnel.
    pub fn in_graph(&self) -> bool {
        self.bridge || self.tunnel || (self.class.rank() >= 1 && self.paved)
    }
}

/// A building outline in the same frame (for road widths and keep-out tests).
#[derive(Debug, Clone)]
pub struct BuildingPoly {
    pub ring: Vec<[f64; 2]>,
    pub lo: [f64; 2],
    pub hi: [f64; 2],
}

#[derive(Debug, Clone, Default)]
pub struct RoadNet {
    pub tile: Option<TileId>,
    /// Width of the centre tile in metres.
    pub wm: f64,
    pub edges: Vec<Edge>,
    pub buildings: Vec<BuildingPoly>,
}

#[derive(Debug, Clone)]
struct Piece {
    src: (i32, i32),
    edge: Edge,
}

pub fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// Liang-Barsky: the part of segment a-b inside [0,e]^2 as parameters, if any.
fn clip_seg(a: [f64; 2], b: [f64; 2], e: f64) -> Option<(f64, f64)> {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [(-dx, a[0]), (dx, e - a[0]), (-dy, a[1]), (dy, e - a[1])] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                if r > t1 {
                    return None;
                }
                t0 = t0.max(r);
            } else {
                if r < t0 {
                    return None;
                }
                t1 = t1.min(r);
            }
        }
    }
    (t1 > t0 || (t1 == t0 && t0 > 0.0 && t0 < 1.0)).then_some((t0, t1))
}

/// Clip a tile-space polyline to the tile square, returning the pieces inside.
fn clip_line(line: &[[f32; 2]], e: f64) -> Vec<Vec<[f64; 2]>> {
    let mut out: Vec<Vec<[f64; 2]>> = Vec::new();
    let mut cur: Vec<[f64; 2]> = Vec::new();
    for w in line.windows(2) {
        let (a, b) = (
            [w[0][0] as f64, w[0][1] as f64],
            [w[1][0] as f64, w[1][1] as f64],
        );
        match clip_seg(a, b, e) {
            Some((t0, t1)) => {
                let pa = [a[0] + (b[0] - a[0]) * t0, a[1] + (b[1] - a[1]) * t0];
                let pb = [a[0] + (b[0] - a[0]) * t1, a[1] + (b[1] - a[1]) * t1];
                if cur.is_empty() {
                    cur.push(pa);
                } else if t0 > 0.0 {
                    // Re-entered: the previous piece was closed below, so this starts a new one.
                    out.push(std::mem::take(&mut cur));
                    cur.push(pa);
                }
                cur.push(pb);
                if t1 < 1.0 {
                    out.push(std::mem::take(&mut cur));
                }
            }
            None => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out.retain(|p| p.len() >= 2 && p.windows(2).any(|w| dist(w[0], w[1]) > 1e-6));
    out
}

fn on_border(p: [f64; 2], wm: f64) -> bool {
    let near = |v: f64| {
        let f = v / wm;
        (f - f.round()).abs() * wm < 1e-6
    };
    near(p[0]) || near(p[1])
}

fn compatible(a: &Edge, b: &Edge) -> bool {
    a.class == b.class
        && a.bridge == b.bridge
        && a.tunnel == b.tunnel
        && a.layer == b.layer
        && a.oneway == b.oneway
        && a.paved == b.paved
        && a.ramp == b.ramp
}

impl RoadNet {
    /// `tiles`: (dx, dy, vector tile) for the centre (0,0) and any neighbours that were available.
    /// `wm` is the centre tile's width in metres. Pure and deterministic.
    pub fn from_tiles(tile: TileId, wm: f64, tiles: &[(i32, i32, &mvt::Tile)]) -> RoadNet {
        let mut pieces: Vec<Piece> = Vec::new();
        let mut buildings = Vec::new();
        for &(dx, dy, vt) in tiles {
            if let Some(layer) = vt.layer("transportation") {
                let e = layer.extent as f64;
                for f in &layer.features {
                    if f.geom_type != GeomType::LineString || f.props.contains_key("indoor") {
                        continue;
                    }
                    let Some(class) = RoadClass::from_omt(f.str("class").unwrap_or("")) else {
                        continue;
                    };
                    let brunnel = f.str("brunnel");
                    let oneway_v = f.num("oneway").unwrap_or(0.0);
                    let proto = Edge {
                        class,
                        layer: f.num("layer").unwrap_or(0.0).clamp(-8.0, 8.0) as i8,
                        bridge: brunnel == Some("bridge"),
                        tunnel: brunnel == Some("tunnel"),
                        ramp: f.num("ramp").unwrap_or(0.0) != 0.0,
                        oneway: oneway_v != 0.0,
                        paved: !matches!(f.str("surface"), Some("unpaved")),
                        cycleway: f.str("subclass") == Some("cycleway"),
                        pts: Vec::new(),
                    };
                    for line in &f.rings {
                        for mut p in clip_line(line, e) {
                            if oneway_v < 0.0 {
                                p.reverse();
                            }
                            let pts = p
                                .iter()
                                .map(|q| [(dx as f64 + q[0] / e) * wm, (dy as f64 + q[1] / e) * wm])
                                .collect();
                            pieces.push(Piece {
                                src: (dx, dy),
                                edge: Edge {
                                    pts,
                                    ..proto.clone()
                                },
                            });
                        }
                    }
                }
            }
            if let Some(layer) = vt.layer("building") {
                let e = layer.extent as f64;
                for f in &layer.features {
                    if f.geom_type != GeomType::Polygon
                        || matches!(f.props.get("hide_3d"), Some(mvt::Value::Bool(true)))
                        || f.num("render_min_height").unwrap_or(0.0) > 0.5
                    {
                        continue;
                    }
                    for (outer, _) in mvt::polygons(f) {
                        let ring: Vec<[f64; 2]> = outer
                            .iter()
                            .map(|q| {
                                [
                                    (dx as f64 + q[0] as f64 / e) * wm,
                                    (dy as f64 + q[1] as f64 / e) * wm,
                                ]
                            })
                            .collect();
                        let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
                        for p in &ring {
                            for k in 0..2 {
                                lo[k] = lo[k].min(p[k]);
                                hi[k] = hi[k].max(p[k]);
                            }
                        }
                        buildings.push(BuildingPoly { ring, lo, hi });
                    }
                }
            }
        }
        RoadNet {
            tile: Some(tile),
            wm,
            edges: join_pieces(pieces, wm),
            buildings,
        }
    }

    /// Hand-made network (tests, tools).
    pub fn from_edges(wm: f64, edges: Vec<Edge>) -> RoadNet {
        RoadNet {
            tile: None,
            wm,
            edges,
            buildings: Vec::new(),
        }
    }
}

/// Join pieces that meet on a tile border into whole edges.
fn join_pieces(pieces: Vec<Piece>, wm: f64) -> Vec<Edge> {
    let n = pieces.len();
    // Border ends: (piece, is_end) with position.
    let mut ends: Vec<(usize, bool, [f64; 2])> = Vec::new();
    for (i, p) in pieces.iter().enumerate() {
        let (a, b) = (p.edge.pts[0], *p.edge.pts.last().unwrap());
        if on_border(a, wm) {
            ends.push((i, false, a));
        }
        if on_border(b, wm) {
            ends.push((i, true, b));
        }
    }
    // Best partner of each border end: nearest compatible end of another tile within JOIN_M.
    let mut grid: std::collections::BTreeMap<(i64, i64), Vec<usize>> = Default::default();
    for (k, e) in ends.iter().enumerate() {
        grid.entry((
            (e.2[0] / JOIN_M).floor() as i64,
            (e.2[1] / JOIN_M).floor() as i64,
        ))
        .or_default()
        .push(k);
    }
    let mut best: Vec<Option<usize>> = vec![None; ends.len()];
    for (k, e) in ends.iter().enumerate() {
        let (cx, cy) = (
            (e.2[0] / JOIN_M).floor() as i64,
            (e.2[1] / JOIN_M).floor() as i64,
        );
        let mut cand: Option<(f64, usize)> = None;
        for gx in cx - 1..=cx + 1 {
            for gy in cy - 1..=cy + 1 {
                for &o in grid.get(&(gx, gy)).into_iter().flatten() {
                    let f = &ends[o];
                    if f.0 == e.0 || pieces[f.0].src == pieces[e.0].src {
                        continue;
                    }
                    let (pa, pb) = (&pieces[e.0].edge, &pieces[f.0].edge);
                    if !compatible(pa, pb) {
                        continue;
                    }
                    // One-way roads keep their direction: an end meets a start.
                    if pa.oneway && e.1 == f.1 {
                        continue;
                    }
                    let d = dist(e.2, f.2);
                    if d <= JOIN_M && cand.is_none_or(|c| (d, o) < (c.0, c.1)) {
                        cand = Some((d, o));
                    }
                }
            }
        }
        best[k] = cand.map(|c| c.1);
    }
    // Keep only mutual pairs.
    let mut link: std::collections::HashMap<(usize, bool), (usize, bool)> = Default::default();
    for (k, b) in best.iter().enumerate() {
        if let Some(o) = *b {
            if best[o] == Some(k) {
                link.insert((ends[k].0, ends[k].1), (ends[o].0, ends[o].1));
            }
        }
    }
    let mut used = vec![false; n];
    let mut out: Vec<Edge> = Vec::new();
    let mut walk = |start: usize, start_at_end: bool, used: &mut Vec<bool>| {
        // Walk away from `start`'s free end.
        let mut e = pieces[start].edge.clone();
        used[start] = true;
        if start_at_end {
            e.pts.reverse();
        }
        let (mut cur, mut cur_end) = (start, !start_at_end);
        while let Some(&(nx, nx_end)) = link.get(&(cur, cur_end)) {
            if used[nx] {
                break;
            }
            used[nx] = true;
            let mut p = pieces[nx].edge.pts.clone();
            if nx_end {
                p.reverse();
            }
            // p now starts at the joint; keep our own joint vertex.
            e.pts.extend(p.into_iter().skip(1));
            cur = nx;
            cur_end = !nx_end;
        }
        out.push(e);
    };
    for i in 0..n {
        if used[i] {
            continue;
        }
        let free_start = !link.contains_key(&(i, false));
        let free_end = !link.contains_key(&(i, true));
        if free_start {
            walk(i, false, &mut used);
        } else if free_end {
            walk(i, true, &mut used);
        }
    }
    // Rings of linked pieces with no free end.
    for i in 0..n {
        if !used[i] {
            walk(i, false, &mut used);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed_data::mvt::{Feature, Layer, Tile, Value};
    use std::collections::HashMap;

    fn road(line: Vec<[f32; 2]>, props: &[(&str, Value)]) -> Feature {
        let mut m = HashMap::new();
        m.insert("class".to_string(), Value::String("motorway".into()));
        for (k, v) in props {
            m.insert(k.to_string(), v.clone());
        }
        Feature {
            id: None,
            geom_type: GeomType::LineString,
            props: m,
            rings: vec![line],
        }
    }

    fn tile(features: Vec<Feature>) -> Tile {
        Tile {
            layers: vec![Layer {
                name: "transportation".into(),
                extent: 4096,
                features,
            }],
        }
    }

    #[test]
    fn clip_cuts_at_the_border_and_drops_the_buffer() {
        let pieces = clip_line(&[[-64.0, 100.0], [4160.0, 100.0]], 4096.0);
        assert_eq!(pieces.len(), 1);
        assert_eq!(pieces[0].first().unwrap(), &[0.0, 100.0]);
        assert_eq!(pieces[0].last().unwrap(), &[4096.0, 100.0]);
    }

    #[test]
    fn road_across_two_tiles_becomes_one_edge() {
        // The same road seen by both tiles, each with its 64-unit buffer.
        let a = tile(vec![road(vec![[2000.0, 500.0], [4160.0, 500.0]], &[])]);
        let b = tile(vec![road(vec![[-64.0, 500.0], [2000.0, 500.0]], &[])]);
        let t = TileId::new(14, 100, 100);
        let net = RoadNet::from_tiles(t, 2400.0, &[(0, 0, &a), (1, 0, &b)]);
        assert_eq!(
            net.edges.len(),
            1,
            "{:?}",
            net.edges.iter().map(|e| e.pts.len()).collect::<Vec<_>>()
        );
        let e = &net.edges[0];
        assert!((e.length() - (2096.0 + 2000.0) / 4096.0 * 2400.0).abs() < 1.0);
    }

    #[test]
    fn incompatible_roads_do_not_join() {
        let a = tile(vec![road(vec![[2000.0, 500.0], [4160.0, 500.0]], &[])]);
        let b = tile(vec![road(
            vec![[-64.0, 500.0], [2000.0, 500.0]],
            &[("brunnel", Value::String("bridge".into()))],
        )]);
        let net = RoadNet::from_tiles(TileId::new(14, 1, 1), 2400.0, &[(0, 0, &a), (1, 0, &b)]);
        assert_eq!(net.edges.len(), 2);
    }

    #[test]
    fn same_road_whichever_tile_is_the_centre() {
        // A road across three tiles in a row, each tile carrying its own clipped piece plus the buffer.
        let mk = |lo: f32, hi: f32| tile(vec![road(vec![[lo, 700.0], [hi, 900.0]], &[])]);
        // x runs -64..4160 in each tile; the road is one straight line of slope 0.0476 across tile units.
        let line = |x: f32| 700.0 + (x + 4096.0) * 200.0 / 8192.0;
        let a = tile(vec![road(
            vec![[0.0, line(-4096.0)], [4160.0, line(64.0)]],
            &[],
        )]);
        let b = tile(vec![road(
            vec![[-64.0, line(-64.0)], [4160.0, line(4160.0)]],
            &[],
        )]);
        let c = tile(vec![road(
            vec![[-64.0, line(4032.0)], [4096.0, line(8192.0)]],
            &[],
        )]);
        let _ = mk;
        let wm = 2000.0;
        // Centre = B sees A, B, C. Centre = A sees (0,0)=A and (1,0)=B only.
        let nb = RoadNet::from_tiles(
            TileId::new(14, 51, 10),
            wm,
            &[(-1, 0, &a), (0, 0, &b), (1, 0, &c)],
        );
        let na = RoadNet::from_tiles(TileId::new(14, 50, 10), wm, &[(0, 0, &a), (1, 0, &b)]);
        assert_eq!(nb.edges.len(), 1);
        assert_eq!(na.edges.len(), 1);
        // Shift A-centred points by one tile width into B's frame and compare inside B's own tile.
        let ea: Vec<[f64; 2]> = na.edges[0].pts.iter().map(|p| [p[0] - wm, p[1]]).collect();
        let eb = &nb.edges[0].pts;
        for p in eb.iter().filter(|p| p[0] > 0.0 && p[0] < wm) {
            // Nearest point of the other polyline.
            let d = ea
                .windows(2)
                .map(|w| {
                    let (dx, dy) = (w[1][0] - w[0][0], w[1][1] - w[0][1]);
                    let t = (((p[0] - w[0][0]) * dx + (p[1] - w[0][1]) * dy)
                        / (dx * dx + dy * dy).max(1e-9))
                    .clamp(0.0, 1.0);
                    (p[0] - (w[0][0] + dx * t)).hypot(p[1] - (w[0][1] + dy * t))
                })
                .fold(f64::MAX, f64::min);
            assert!(d < 0.05, "{d}");
        }
    }

    #[test]
    fn level_rules() {
        let mut e = Edge {
            class: RoadClass::Motorway,
            layer: 1,
            bridge: false,
            tunnel: false,
            ramp: false,
            oneway: false,
            paved: true,
            cycleway: false,
            pts: vec![],
        };
        assert_eq!(e.level(), 0, "layer alone does not make a deck");
        e.bridge = true;
        assert_eq!(e.level(), 1);
        e.layer = 3;
        assert_eq!(e.level(), 3);
        e.bridge = false;
        e.tunnel = true;
        e.layer = -2;
        assert_eq!(e.level(), -2);
        e.layer = 0;
        assert_eq!(e.level(), -1);
    }
}
