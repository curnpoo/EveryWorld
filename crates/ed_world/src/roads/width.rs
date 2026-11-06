//! Street width from the buildings that line it.
//!
//! The vector tiles carry no road width. In a built-up street the buildings stand behind the sidewalks, so the
//! distance between the two facades is the right-of-way. A ray goes out each side of the street every
//! `STEP_M` metres to the first building wall; when both sides are walled in most places and the face-to-face
//! distances agree, their median is the right-of-way, and the carriageway is what is left after two sidewalks.
//! Elsewhere (front yards, open land) the class default stays.

use super::RoadClass;
use super::net::{BuildingPoly, RoadNet, dist};
use std::collections::BTreeMap;

const STEP_M: f64 = 12.0;
const END_SKIP_M: f64 = 15.0;
const REACH_M: f64 = 32.0;
const WALLED: f64 = 0.6;
const AGREE: f64 = 0.35;
const WALK_M: f64 = 2.4;
const CELL: f64 = 64.0;

struct Index<'a> {
    polys: &'a [BuildingPoly],
    cells: BTreeMap<(i64, i64), Vec<u32>>,
}

impl<'a> Index<'a> {
    fn new(polys: &'a [BuildingPoly]) -> Self {
        let mut cells: BTreeMap<(i64, i64), Vec<u32>> = BTreeMap::new();
        for (i, b) in polys.iter().enumerate() {
            for gx in (b.lo[0] / CELL).floor() as i64..=(b.hi[0] / CELL).floor() as i64 {
                for gy in (b.lo[1] / CELL).floor() as i64..=(b.hi[1] / CELL).floor() as i64 {
                    cells.entry((gx, gy)).or_default().push(i as u32);
                }
            }
        }
        Self { polys, cells }
    }

    /// Distance along the ray from `o` in direction `d` (unit) to the nearest building wall, if within `reach`.
    fn ray(&self, o: [f64; 2], d: [f64; 2], reach: f64) -> Option<f64> {
        let mut best: Option<f64> = None;
        let mut seen: Vec<u32> = Vec::new();
        let steps = (reach / (CELL * 0.5)).ceil() as usize + 1;
        for k in 0..=steps {
            let t = (k as f64 * CELL * 0.5).min(reach);
            let (gx, gy) = (
                ((o[0] + d[0] * t) / CELL).floor() as i64,
                ((o[1] + d[1] * t) / CELL).floor() as i64,
            );
            for &i in self.cells.get(&(gx, gy)).into_iter().flatten() {
                if seen.contains(&i) {
                    continue;
                }
                seen.push(i);
                let ring = &self.polys[i as usize].ring;
                for w in 0..ring.len() {
                    let (a, b) = (ring[w], ring[(w + 1) % ring.len()]);
                    let (e, f) = ([b[0] - a[0], b[1] - a[1]], [a[0] - o[0], a[1] - o[1]]);
                    let den = d[0] * e[1] - d[1] * e[0];
                    if den.abs() < 1e-12 {
                        continue;
                    }
                    let t = (f[0] * e[1] - f[1] * e[0]) / den;
                    let u = (f[0] * d[1] - f[1] * d[0]) / den;
                    if t > 0.0
                        && t <= reach
                        && (0.0..=1.0).contains(&u)
                        && best.is_none_or(|b| t < b)
                    {
                        best = Some(t);
                    }
                }
            }
        }
        best
    }
}

/// Right-of-way (m, facade to facade) measured along each edge; `None` where the street is not walled.
pub fn right_of_way(net: &RoadNet) -> Vec<Option<f32>> {
    let idx = Index::new(&net.buildings);
    net.edges
        .iter()
        .map(|e| {
            if !matches!(
                e.class,
                RoadClass::Secondary | RoadClass::Tertiary | RoadClass::Minor
            ) || !e.paved
                || e.bridge
                || e.tunnel
                || net.buildings.is_empty()
            {
                return None;
            }
            let total = e.length();
            if total < 2.0 * END_SKIP_M + STEP_M {
                return None;
            }
            let (mut samples, mut walled) = (0usize, Vec::<f64>::new());
            let mut next = END_SKIP_M;
            let mut s = 0.0;
            for w in e.pts.windows(2) {
                let len = dist(w[0], w[1]);
                let dir = [
                    (w[1][0] - w[0][0]) / len.max(1e-9),
                    (w[1][1] - w[0][1]) / len.max(1e-9),
                ];
                while next < s + len && next < total - END_SKIP_M {
                    let t = next - s;
                    let o = [w[0][0] + dir[0] * t, w[0][1] + dir[1] * t];
                    samples += 1;
                    let (l, r) = (
                        idx.ray(o, [-dir[1], dir[0]], REACH_M),
                        idx.ray(o, [dir[1], -dir[0]], REACH_M),
                    );
                    if let (Some(l), Some(r)) = (l, r) {
                        walled.push(l + r);
                    }
                    next += STEP_M;
                }
                s += len;
            }
            if samples < 3 || (walled.len() as f64) < WALLED * samples as f64 {
                return None;
            }
            walled.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let (q1, med, q3) = (
                walled[walled.len() / 4],
                walled[walled.len() / 2],
                walled[walled.len() * 3 / 4],
            );
            ((q3 - q1) <= AGREE * med).then_some(med as f32)
        })
        .collect()
}

/// Carriageway width given a right-of-way and the class default.
pub fn carriageway(class: RoadClass, row: Option<f32>) -> f32 {
    let d = class.width();
    match row {
        Some(r) => (r - 2.0 * WALK_M as f32 - 0.6).clamp(0.7 * d, 1.8 * d),
        None => d,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roads::net::Edge;

    fn street(x0: f64, x1: f64) -> Edge {
        Edge {
            class: RoadClass::Minor,
            layer: 0,
            bridge: false,
            tunnel: false,
            ramp: false,
            oneway: false,
            paved: true,
            cycleway: false,
            pts: vec![[x0, 0.0], [x1, 0.0]],
        }
    }

    fn wall(x0: f64, x1: f64, y0: f64, y1: f64) -> BuildingPoly {
        BuildingPoly {
            ring: vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]],
            lo: [x0, y0],
            hi: [x1, y1],
        }
    }

    #[test]
    fn walled_street_gets_measured_width() {
        let mut net = RoadNet::from_edges(2400.0, vec![street(0.0, 300.0)]);
        for i in 0..10 {
            let x = i as f64 * 30.0;
            net.buildings.push(wall(x, x + 28.0, -20.0, -8.0)); // 8 m south of the axis...
            net.buildings.push(wall(x, x + 28.0, 8.0, 20.0));
        }
        let row = right_of_way(&net);
        let r = row[0].expect("walled both sides");
        assert!((r - 16.0).abs() < 0.5, "{r}");
        let w = carriageway(RoadClass::Minor, Some(r));
        assert!((4.55..=11.7).contains(&w), "{w}");
    }

    #[test]
    fn open_land_keeps_the_class_width() {
        let net = RoadNet::from_edges(2400.0, vec![street(0.0, 300.0)]);
        assert_eq!(right_of_way(&net)[0], None);
        assert_eq!(
            carriageway(RoadClass::Minor, None),
            RoadClass::Minor.width()
        );
    }
}
