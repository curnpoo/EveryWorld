//! Measured heights (`ed_data::roadz`) matched to the network.
//!
//! A surveyed road height is truth where it exists. Segments are converted to the network's metre frame, their
//! datum is brought onto our elevation model (the median of source Z minus our ground along the source's own
//! vertices, so a 0.5-2 m survey offset makes no step where a measured deck meets our graded approach), and an
//! edge sample takes the height of the nearest compatible segment: within 8 m, running the same way (a road
//! crossing under a deck is rejected by its direction), and, for sources that flag it, on the same level.

use super::net::{RoadNet, dist};
use ed_data::roadz::Seg;
use ed_geo::tile::TileId;
use std::collections::HashMap;

const MATCH_M: f64 = 8.0;
const CELL: f64 = 32.0;
const MIN_COS: f64 = 0.7;

#[derive(Debug, Clone)]
pub struct MSeg {
    pub a: [f64; 2],
    pub b: [f64; 2],
    pub za: f32,
    pub zb: f32,
    pub level: i8,
    pub lanes: Option<u8>,
    pub width: Option<f32>,
}

#[derive(Debug, Clone, Default)]
pub struct Measured {
    segs: Vec<MSeg>,
    grid: HashMap<(i32, i32), Vec<u32>>,
    /// The source flags bridge / tunnel levels, so matching requires them.
    strict: bool,
    /// Datum offset that was subtracted (m), for the log.
    pub offset: f32,
}

impl Measured {
    pub fn is_empty(&self) -> bool {
        self.segs.is_empty()
    }

    pub fn len(&self) -> usize {
        self.segs.len()
    }

    /// `blocks`: for each tile of the 3x3 block, (dx, dy, tile, its segments).
    pub fn build(wm: f64, blocks: &[(i32, i32, TileId, Vec<Seg>)], strict: bool, ground: &dyn Fn(f64, f64) -> f32) -> Measured {
        let mut segs = Vec::new();
        for (dx, dy, tile, list) in blocks {
            for s in list {
                let to_m = |p: &[f64; 3]| {
                    let (u, v) = tile.geodetic_to_uv(p[1], p[0]);
                    [(*dx as f64 + u) * wm, (*dy as f64 + v) * wm]
                };
                for w in s.pts.windows(2) {
                    segs.push(MSeg { a: to_m(&w[0]), b: to_m(&w[1]), za: w[0][2] as f32, zb: w[1][2] as f32, level: s.level, lanes: s.lanes, width: s.width });
                }
            }
        }
        Self::from_segs(segs, strict, ground)
    }

    pub fn from_segs(mut segs: Vec<MSeg>, strict: bool, ground: &dyn Fn(f64, f64) -> f32) -> Measured {
        // Datum: median of (source Z - our ground) along ground-level vertices (all vertices when levels are not flagged).
        let mut diffs: Vec<f32> = segs.iter().filter(|s| !strict || s.level == 0).flat_map(|s| [(s.a, s.za), (s.b, s.zb)]).map(|(p, z)| z - ground(p[0], p[1])).collect();
        let offset = if diffs.is_empty() {
            0.0
        } else {
            diffs.sort_by(|a, b| a.partial_cmp(b).unwrap());
            diffs[diffs.len() / 2].clamp(-6.0, 6.0)
        };
        for s in &mut segs {
            s.za -= offset;
            s.zb -= offset;
        }
        let mut grid: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
        for (i, s) in segs.iter().enumerate() {
            let (x0, x1) = (((s.a[0].min(s.b[0]) - MATCH_M) / CELL).floor() as i32, ((s.a[0].max(s.b[0]) + MATCH_M) / CELL).floor() as i32);
            let (y0, y1) = (((s.a[1].min(s.b[1]) - MATCH_M) / CELL).floor() as i32, ((s.a[1].max(s.b[1]) + MATCH_M) / CELL).floor() as i32);
            for gx in x0..=x1 {
                for gy in y0..=y1 {
                    grid.entry((gx, gy)).or_default().push(i as u32);
                }
            }
        }
        Measured { segs, grid, strict, offset }
    }

    /// The nearest compatible segment at `p` (unit direction `dir`; `want`: 1 bridge, -1 tunnel, 0 ground) and
    /// the measured height there.
    pub fn at(&self, p: [f64; 2], dir: [f64; 2], want: i8) -> Option<(f32, &MSeg)> {
        let key = ((p[0] / CELL).floor() as i32, (p[1] / CELL).floor() as i32);
        let mut best: Option<(f64, f32, &MSeg)> = None;
        for &i in self.grid.get(&key)? {
            let s = &self.segs[i as usize];
            if self.strict && s.level.signum() != want.signum() {
                continue;
            }
            let (dx, dy) = (s.b[0] - s.a[0], s.b[1] - s.a[1]);
            let l2 = (dx * dx + dy * dy).max(1e-9);
            if ((dx * dir[0] + dy * dir[1]) / l2.sqrt()).abs() < MIN_COS {
                continue;
            }
            let t = (((p[0] - s.a[0]) * dx + (p[1] - s.a[1]) * dy) / l2).clamp(0.0, 1.0);
            let d = (p[0] - (s.a[0] + dx * t)).hypot(p[1] - (s.a[1] + dy * t));
            if d <= MATCH_M && best.is_none_or(|b| d < b.0) {
                best = Some((d, s.za + (s.zb - s.za) * t as f32, s));
            }
        }
        best.map(|(_, z, s)| (z, s))
    }
}

/// Fill lanes and width from the survey for edges that have no OSM tags (France reports both on every road).
pub fn fill_tags(net: &mut RoadNet, m: &Measured) {
    for e in net.edges.iter_mut().filter(|e| e.tags.is_empty() && e.class.is_car_road() && e.class.rank() >= 1 && e.pts.len() >= 2) {
        let total: f64 = e.pts.windows(2).map(|w| dist(w[0], w[1])).sum();
        let mut found: Vec<(Option<u8>, Option<f32>)> = Vec::new();
        for f in [0.25, 0.5, 0.75] {
            let (mut acc, mut at) = (0.0, e.pts[0]);
            let mut dir = [1.0, 0.0];
            for w in e.pts.windows(2) {
                let l = dist(w[0], w[1]);
                if acc + l >= total * f {
                    let t = ((total * f - acc) / l.max(1e-9)).clamp(0.0, 1.0);
                    at = [w[0][0] + (w[1][0] - w[0][0]) * t, w[0][1] + (w[1][1] - w[0][1]) * t];
                    dir = [(w[1][0] - w[0][0]) / l.max(1e-9), (w[1][1] - w[0][1]) / l.max(1e-9)];
                    break;
                }
                acc += l;
            }
            if let Some((_, s)) = m.at(at, dir, if e.bridge { 1 } else if e.tunnel { -1 } else { 0 }) {
                found.push((s.lanes, s.width));
            }
        }
        if found.len() >= 2 {
            e.tags.lanes = found.iter().filter_map(|f| f.0).next();
            e.tags.width = found.iter().filter_map(|f| f.1).next();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(a: [f64; 2], b: [f64; 2], z: f32, level: i8) -> MSeg {
        MSeg { a, b, za: z, zb: z, level, lanes: Some(2), width: Some(7.0) }
    }

    #[test]
    fn datum_is_brought_onto_our_ground_and_levels_must_agree() {
        // Source reads 1.5 m above our ground on its ground-level road.
        let ground = |_: f64, _: f64| 100.0f32;
        let m = Measured::from_segs(vec![seg([0.0, 0.0], [100.0, 0.0], 101.5, 0), seg([200.0, 0.0], [300.0, 0.0], 111.5, 1)], true, &ground);
        assert!((m.offset - 1.5).abs() < 1e-4, "{}", m.offset);
        let (z, s) = m.at([250.0, 2.0], [1.0, 0.0], 1).unwrap();
        assert_eq!(s.level, 1);
        assert!((z - (111.5 - m.offset)).abs() < 1e-4);
        assert!(m.at([250.0, 2.0], [1.0, 0.0], 0).is_none(), "a bridge edge does not take the ground road's height");
        assert!(m.at([250.0, 2.0], [0.0, 1.0], 1).is_none(), "a road crossing under it runs the other way");
        assert!(m.at([250.0, 30.0], [1.0, 0.0], 1).is_none(), "too far");
    }

    #[test]
    fn unflagged_sources_match_any_level() {
        let m = Measured::from_segs(vec![seg([0.0, 0.0], [100.0, 0.0], 50.0, 0)], false, &|_, _| 50.0);
        assert!(m.at([50.0, 1.0], [1.0, 0.0], 1).is_some());
        assert!(m.at([50.0, 1.0], [1.0, 0.0], -1).is_some());
    }

    #[test]
    fn survey_lanes_and_width_fill_untagged_edges() {
        use crate::roads::solve::tests::edge;
        use crate::roads::RoadClass;
        let m = Measured::from_segs(vec![seg([0.0, 0.0], [400.0, 0.0], 10.0, 0)], true, &|_, _| 10.0);
        let mut net = RoadNet::from_edges(2400.0, vec![edge(RoadClass::Primary, false, vec![[10.0, 1.0], [380.0, 1.0]])]);
        fill_tags(&mut net, &m);
        assert_eq!(net.edges[0].tags.lanes, Some(2));
        assert_eq!(net.edges[0].tags.width, Some(7.0));
    }
}
