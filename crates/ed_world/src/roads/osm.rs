//! OSM tags on road edges: lanes, width, speed limit, sidewalks, cycle lanes.
//!
//! The baked ways (`ed_data::osmtags`) are matched to the network's edges by geometry: every ~10 m along an edge
//! the nearest OSM way segment within 8 m votes, and the way with the most votes (at least half of the samples,
//! and a compatible road class) gives its tags. Left and right in OSM are relative to the way's direction, so
//! they are swapped when the edge runs the other way. Pure functions; no tags means today's defaults.

use super::RoadClass;
use super::net::{RoadNet, dist};
use ed_data::osmtags::{OsmTile, OsmWay};
use std::collections::{BTreeMap, HashMap};

const MATCH_M: f64 = 8.0;
const STEP_M: f64 = 10.0;
const CELL: f64 = 32.0;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct EdgeTags {
    pub lanes: Option<u8>,
    pub lanes_fwd: Option<u8>,
    pub lanes_back: Option<u8>,
    /// Mapped carriageway width in metres.
    pub width: Option<f32>,
    pub maxspeed_kmh: Option<f32>,
    /// Sidewalk on the (left, right) of the edge's direction; `None` when the way says nothing about it.
    pub sidewalk: Option<(bool, bool)>,
    /// Cycle facility on the (left, right): 0 none, 1 painted lane, 2 separated track.
    pub cycle: (u8, u8),
}

impl EdgeTags {
    pub fn is_empty(&self) -> bool {
        *self == EdgeTags::default()
    }

    /// Lanes in the edge's direction and against it, from whatever the way gives.
    pub fn lane_split(&self, oneway: bool) -> Option<(u8, u8)> {
        if oneway {
            return self.lanes.or(self.lanes_fwd).map(|n| (n.clamp(1, 8), 0));
        }
        match (self.lanes_fwd, self.lanes_back, self.lanes) {
            (Some(f), Some(b), _) => Some((f, b)),
            (Some(f), None, Some(n)) if n > f => Some((f, n - f)),
            (None, Some(b), Some(n)) if n > b => Some((n - b, b)),
            (_, _, Some(n)) => Some((n.div_ceil(2).max(1), n / 2)),
            _ => None,
        }
    }
}

fn num(s: &str) -> Option<f32> {
    s.split_whitespace().next()?.replace(',', ".").parse().ok()
}

fn speed_kmh(s: &str) -> Option<f32> {
    let v = num(s)?;
    Some(if s.contains("mph") { v * 1.609 } else { v })
}

fn side_flag(v: &str) -> Option<bool> {
    match v {
        "yes" | "both" | "left" | "right" | "lane" | "track" | "separate" => Some(true),
        "no" | "none" => Some(false),
        _ => None,
    }
}

fn cycle_kind(v: &str) -> u8 {
    match v {
        "lane" | "opposite_lane" | "shared_busway" => 1,
        "track" | "opposite_track" => 2,
        _ => 0,
    }
}

/// Interpret one way's tags. Sides are relative to the way's own direction.
pub fn parse_tags(t: &HashMap<String, String>) -> EdgeTags {
    let g = |k: &str| t.get(k).map(String::as_str);
    let n = |k: &str| g(k).and_then(num).filter(|v| *v >= 1.0 && *v <= 12.0).map(|v| v as u8);
    let sidewalk = {
        let both = g("sidewalk:both").or(match g("sidewalk") {
            Some("both") => Some("yes"),
            Some("no") | Some("none") => Some("no"),
            _ => None,
        });
        let (l, r) = (g("sidewalk:left").or(match g("sidewalk") { Some("left") => Some("yes"), Some("right") => Some("no"), _ => None }), g("sidewalk:right").or(match g("sidewalk") { Some("right") => Some("yes"), Some("left") => Some("no"), _ => None }));
        let (l, r) = (l.or(both).and_then(side_flag), r.or(both).and_then(side_flag));
        (l.is_some() || r.is_some()).then(|| (l.unwrap_or(false), r.unwrap_or(false)))
    };
    let cyc = |side: &str| -> u8 {
        let k = |key: &str| g(key).map(cycle_kind).unwrap_or(0);
        let one = k(&format!("cycleway:{side}")).max(k("cycleway:both")).max(k("cycleway"));
        if g(&format!("cycleway:{side}:lane")).is_some() { one.max(1) } else { one }
    };
    EdgeTags {
        lanes: n("lanes"),
        lanes_fwd: n("lanes:forward"),
        lanes_back: n("lanes:backward"),
        width: g("width").and_then(num).filter(|w| *w >= 2.0 && *w <= 40.0),
        maxspeed_kmh: g("maxspeed").or(g("maxspeed:forward")).and_then(speed_kmh).filter(|v| *v >= 5.0 && *v <= 160.0),
        sidewalk,
        cycle: (cyc("left"), cyc("right")),
    }
}

fn flip(t: &mut EdgeTags) {
    t.sidewalk = t.sidewalk.map(|(l, r)| (r, l));
    t.cycle = (t.cycle.1, t.cycle.0);
    std::mem::swap(&mut t.lanes_fwd, &mut t.lanes_back);
}

/// Can a vector-tile road class be this OSM highway?
fn compatible(c: RoadClass, hw: &str) -> bool {
    use RoadClass::*;
    let base = hw.trim_end_matches("_link");
    match c {
        Motorway => base == "motorway",
        Trunk => matches!(base, "trunk" | "motorway"),
        Primary => matches!(base, "primary" | "trunk"),
        Secondary => matches!(base, "secondary" | "primary"),
        Tertiary => matches!(base, "tertiary" | "secondary"),
        Minor => matches!(base, "residential" | "unclassified" | "living_street" | "tertiary" | "road" | "raceway" | "service"),
        Service => matches!(base, "service" | "busway" | "residential" | "unclassified"),
        Track => base == "track",
        Path => matches!(base, "path" | "footway" | "cycleway" | "pedestrian" | "steps" | "bridleway"),
        Rail => false,
    }
}

struct Seg {
    way: u32,
    a: [f64; 2],
    b: [f64; 2],
}

/// Attach tags to every edge of `net`. `tiles`: (dx, dy, baked tile) in the network's frame.
pub fn attach(net: &mut RoadNet, tiles: &[(i32, i32, &OsmTile)]) {
    let wm = net.wm;
    let mut ways: Vec<&OsmWay> = Vec::new();
    let mut grid: BTreeMap<(i64, i64), Vec<Seg>> = BTreeMap::new();
    for &(dx, dy, t) in tiles {
        let k = wm / t.extent as f64;
        for w in &t.ways {
            let wi = ways.len() as u32;
            ways.push(w);
            for line in &w.lines {
                for p in line.windows(2) {
                    let a = [(dx as f64 * wm) + p[0][0] as f64 * k, (dy as f64 * wm) + p[0][1] as f64 * k];
                    let b = [(dx as f64 * wm) + p[1][0] as f64 * k, (dy as f64 * wm) + p[1][1] as f64 * k];
                    let (x0, x1) = (((a[0].min(b[0]) - MATCH_M) / CELL).floor() as i64, ((a[0].max(b[0]) + MATCH_M) / CELL).floor() as i64);
                    let (y0, y1) = (((a[1].min(b[1]) - MATCH_M) / CELL).floor() as i64, ((a[1].max(b[1]) + MATCH_M) / CELL).floor() as i64);
                    for gx in x0..=x1 {
                        for gy in y0..=y1 {
                            grid.entry((gx, gy)).or_default().push(Seg { way: wi, a, b });
                        }
                    }
                }
            }
        }
    }
    if ways.is_empty() {
        return;
    }
    for e in net.edges.iter_mut() {
        if e.pts.len() < 2 {
            continue;
        }
        let mut votes: BTreeMap<u32, (u32, i32)> = BTreeMap::new(); // way -> (votes, direction sum)
        let mut samples = 0u32;
        let mut next = 0.0;
        let mut s = 0.0;
        for w in e.pts.windows(2) {
            let len = dist(w[0], w[1]);
            let dir = [(w[1][0] - w[0][0]) / len.max(1e-9), (w[1][1] - w[0][1]) / len.max(1e-9)];
            while next <= s + len {
                let t = (next - s).max(0.0);
                let p = [w[0][0] + dir[0] * t, w[0][1] + dir[1] * t];
                samples += 1;
                let mut best: Option<(f64, u32, i32)> = None;
                let (cx, cy) = ((p[0] / CELL).floor() as i64, (p[1] / CELL).floor() as i64);
                for seg in grid.get(&(cx, cy)).into_iter().flatten() {
                    if !compatible(e.class, &ways[seg.way as usize].highway) {
                        continue;
                    }
                    let (dx, dy) = (seg.b[0] - seg.a[0], seg.b[1] - seg.a[1]);
                    let l2 = (dx * dx + dy * dy).max(1e-9);
                    let tt = (((p[0] - seg.a[0]) * dx + (p[1] - seg.a[1]) * dy) / l2).clamp(0.0, 1.0);
                    let d = (p[0] - (seg.a[0] + dx * tt)).hypot(p[1] - (seg.a[1] + dy * tt));
                    if d <= MATCH_M && best.is_none_or(|b| d < b.0) {
                        let along = (dx * dir[0] + dy * dir[1]) / l2.sqrt();
                        best = Some((d, seg.way, if along >= 0.0 { 1 } else { -1 }));
                    }
                }
                if let Some((_, wi, sgn)) = best {
                    let v = votes.entry(wi).or_default();
                    v.0 += 1;
                    v.1 += sgn;
                }
                next += STEP_M;
            }
            s += len;
        }
        let Some((&wi, &(n, dsum))) = votes.iter().max_by_key(|(w, v)| (v.0, std::cmp::Reverse(**w))) else { continue };
        if samples == 0 || (n as f64) < 0.5 * samples as f64 {
            continue;
        }
        let mut tags = parse_tags(&ways[wi as usize].tags);
        if dsum < 0 {
            flip(&mut tags);
        }
        e.tags = tags;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roads::net::Edge;

    fn tags(kv: &[(&str, &str)]) -> HashMap<String, String> {
        kv.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn tag_interpretation() {
        let t = parse_tags(&tags(&[("lanes", "4"), ("lanes:forward", "3"), ("width", "14.5"), ("maxspeed", "30 mph"), ("sidewalk", "left"), ("cycleway:right", "lane")]));
        assert_eq!((t.lanes, t.lanes_fwd, t.width), (Some(4), Some(3), Some(14.5)));
        assert!((t.maxspeed_kmh.unwrap() - 48.3).abs() < 0.1);
        assert_eq!(t.sidewalk, Some((true, false)));
        assert_eq!(t.cycle, (0, 1));
        assert_eq!(t.lane_split(false), Some((3, 1)));
        let both = parse_tags(&tags(&[("sidewalk", "both"), ("cycleway", "track"), ("lanes", "2")]));
        assert_eq!(both.sidewalk, Some((true, true)));
        assert_eq!(both.cycle, (2, 2));
        assert_eq!(both.lane_split(false), Some((1, 1)));
        assert_eq!(both.lane_split(true), Some((2, 0)));
        assert!(parse_tags(&tags(&[("surface", "asphalt")])).is_empty());
        assert_eq!(parse_tags(&tags(&[("sidewalk:both", "no")])).sidewalk, Some((false, false)));
    }

    fn edge(class: RoadClass, pts: Vec<[f64; 2]>) -> Edge {
        Edge { class, layer: 0, bridge: false, tunnel: false, ramp: false, oneway: false, paved: true, cycleway: false, tags: EdgeTags::default(), pts }
    }

    fn way(highway: &str, kv: &[(&str, &str)], line: Vec<[f32; 2]>) -> OsmWay {
        OsmWay { id: 1, highway: highway.into(), tags: tags(kv), lines: vec![line] }
    }

    #[test]
    fn matches_by_geometry_and_swaps_sides_when_reversed() {
        // wm 4096: one tile unit is one metre. A road along +x at y=100.
        let mk = |pts: Vec<[f64; 2]>| RoadNet::from_edges(4096.0, vec![edge(RoadClass::Secondary, pts)]);
        let t = OsmTile { extent: 4096.0, baked: String::new(), ways: vec![way("secondary", &[("lanes", "2"), ("sidewalk", "left")], vec![[0.0, 101.0], [500.0, 101.0]]), way("primary", &[("lanes", "6")], vec![[0.0, 400.0], [500.0, 400.0]])] };
        let mut same = mk(vec![[10.0, 100.0], [300.0, 100.0]]);
        attach(&mut same, &[(0, 0, &t)]);
        assert_eq!(same.edges[0].tags.lanes, Some(2));
        assert_eq!(same.edges[0].tags.sidewalk, Some((true, false)));
        let mut rev = mk(vec![[300.0, 100.0], [10.0, 100.0]]);
        attach(&mut rev, &[(0, 0, &t)]);
        assert_eq!(rev.edges[0].tags.sidewalk, Some((false, true)), "left of the way is right of an edge running the other way");
        // Nothing within 8 m, or the wrong class: untouched.
        let mut far = mk(vec![[10.0, 250.0], [300.0, 250.0]]);
        attach(&mut far, &[(0, 0, &t)]);
        assert!(far.edges[0].tags.is_empty());
        let mut wrong = RoadNet::from_edges(4096.0, vec![edge(RoadClass::Tertiary, vec![[10.0, 400.0], [300.0, 400.0]])]);
        attach(&mut wrong, &[(0, 0, &t)]);
        assert!(wrong.edges[0].tags.is_empty(), "a tertiary road is not the primary next to it");
    }
}
