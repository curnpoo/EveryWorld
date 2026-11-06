//! Bridges the map does not tag.
//!
//! Two ground-level roads (or a road and a railway) that cross with no junction between them are grade
//! separated, whether or not anyone tagged a bridge. The ground tells which is on top: the upper line runs along
//! the sides of the depression the lower one crosses, the lower line runs along its floor. For each crossing the
//! median ground height at 40 m and 80 m either side along each line is compared; the line whose neighbourhood is
//! at least `SEP_M` higher, and whose ground 40 m out on both sides is above the crossing, gets a bridge over the
//! other's carriageway plus a margin. The edge is split there and the middle piece becomes a bridge, which the
//! height solver then lifts like any tagged one. Off with `EW_INFER_BRIDGES=0`.

use super::RoadClass;
use super::net::{Edge, RoadNet, dist};
use std::collections::HashMap;

pub const SEP_M: f64 = 4.0;
/// The upper line's ground 40 m out must stand this far above the crossing, on both sides.
pub const SIDE_M: f64 = 1.5;
const MIN_ANGLE_DEG: f64 = 25.0;
/// A crossing within this distance of an end of either line is a junction, not a grade separation.
const NODE_CLEAR_M: f64 = 15.0;
const CELL: f64 = 64.0;

pub fn enabled() -> bool {
    std::env::var("EW_INFER_BRIDGES").map(|v| v != "0").unwrap_or(true)
}

fn candidate(e: &Edge) -> bool {
    e.level() == 0 && !e.tunnel && e.pts.len() >= 2 && (e.class == RoadClass::Rail || (e.class.rank() >= 1 && e.paved) || e.class == RoadClass::Service)
}

fn arc_len(pts: &[[f64; 2]]) -> Vec<f64> {
    let mut s = vec![0.0; pts.len()];
    for i in 1..pts.len() {
        s[i] = s[i - 1] + dist(pts[i - 1], pts[i]);
    }
    s
}

/// Point at arc length `at` (clamped) along a polyline.
fn point_at(pts: &[[f64; 2]], s: &[f64], at: f64) -> [f64; 2] {
    let at = at.clamp(0.0, *s.last().unwrap());
    let i = s.partition_point(|&v| v <= at).clamp(1, pts.len() - 1);
    let t = if s[i] > s[i - 1] { (at - s[i - 1]) / (s[i] - s[i - 1]) } else { 0.0 };
    [pts[i - 1][0] + (pts[i][0] - pts[i - 1][0]) * t, pts[i - 1][1] + (pts[i][1] - pts[i - 1][1]) * t]
}

fn median4(mut v: [f64; 4]) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (v[1] + v[2]) * 0.5
}

/// (median ground at 40 and 80 m either side along the line, the lower of the two 40 m samples).
fn neighbourhood(pts: &[[f64; 2]], s: &[f64], at: f64, ground: &dyn Fn(f64, f64) -> f32) -> (f64, f64) {
    let g = |d: f64| {
        let p = point_at(pts, s, at + d);
        ground(p[0], p[1]) as f64
    };
    let (a, b, c, d) = (g(-80.0), g(-40.0), g(40.0), g(80.0));
    (median4([a, b, c, d]), b.min(c))
}

#[derive(Clone, Copy)]
pub struct Span {
    pub s0: f64,
    pub s1: f64,
}

/// Find the crossings, decide the upper line, split edges and mark the bridge pieces. Returns how many bridge
/// spans were added.
pub fn infer_bridges(net: &mut RoadNet, ground: &dyn Fn(f64, f64) -> f32) -> usize {
    let cand: Vec<usize> = (0..net.edges.len()).filter(|&i| candidate(&net.edges[i])).collect();
    if cand.len() < 2 {
        return 0;
    }
    let arcs: HashMap<usize, Vec<f64>> = cand.iter().map(|&i| (i, arc_len(&net.edges[i].pts))).collect();
    let mut grid: HashMap<(i32, i32), Vec<(u32, u32)>> = HashMap::new();
    for &ei in &cand {
        let p = &net.edges[ei].pts;
        for k in 0..p.len() - 1 {
            let (x0, x1) = ((p[k][0].min(p[k + 1][0]) / CELL).floor() as i32, (p[k][0].max(p[k + 1][0]) / CELL).floor() as i32);
            let (y0, y1) = ((p[k][1].min(p[k + 1][1]) / CELL).floor() as i32, (p[k][1].max(p[k + 1][1]) / CELL).floor() as i32);
            for gx in x0..=x1 {
                for gy in y0..=y1 {
                    grid.entry((gx, gy)).or_default().push((ei as u32, k as u32));
                }
            }
        }
    }
    let mut spans: HashMap<usize, Vec<Span>> = HashMap::new();
    let mut seen: std::collections::HashSet<(u32, u32, u32, u32)> = Default::default();
    let min_sin = MIN_ANGLE_DEG.to_radians().sin();
    let mut keys: Vec<_> = grid.keys().copied().collect();
    keys.sort();
    for key in keys {
        let cell = &grid[&key];
        for (ia, &(ea, sa)) in cell.iter().enumerate() {
            for &(eb, sb) in &cell[ia + 1..] {
                if ea == eb {
                    continue;
                }
                let k = if ea < eb { (ea, sa, eb, sb) } else { (eb, sb, ea, sa) };
                if !seen.insert(k) {
                    continue;
                }
                let (a, b) = (&net.edges[ea as usize], &net.edges[eb as usize]);
                let (p0, p1) = (a.pts[sa as usize], a.pts[sa as usize + 1]);
                let (q0, q1) = (b.pts[sb as usize], b.pts[sb as usize + 1]);
                let (r, s) = ([p1[0] - p0[0], p1[1] - p0[1]], [q1[0] - q0[0], q1[1] - q0[1]]);
                let den = r[0] * s[1] - r[1] * s[0];
                let (lr, ls) = (r[0].hypot(r[1]), s[0].hypot(s[1]));
                if den.abs() < 1e-9 || lr < 1e-9 || ls < 1e-9 || den.abs() / (lr * ls) < min_sin {
                    continue;
                }
                let qp = [q0[0] - p0[0], q0[1] - p0[1]];
                let t = (qp[0] * s[1] - qp[1] * s[0]) / den;
                let u = (qp[0] * r[1] - qp[1] * r[0]) / den;
                if !(0.0..=1.0).contains(&t) || !(0.0..=1.0).contains(&u) {
                    continue;
                }
                let p = [p0[0] + r[0] * t, p0[1] + r[1] * t];
                let (sa_arc, sb_arc) = (arcs[&(ea as usize)][sa as usize] + t * lr, arcs[&(eb as usize)][sb as usize] + u * ls);
                let (la, lb) = (*arcs[&(ea as usize)].last().unwrap(), *arcs[&(eb as usize)].last().unwrap());
                // A junction: the crossing sits at an end of either line.
                if sa_arc < NODE_CLEAR_M || la - sa_arc < NODE_CLEAR_M || sb_arc < NODE_CLEAR_M || lb - sb_arc < NODE_CLEAR_M {
                    continue;
                }
                let (ha, side_a) = neighbourhood(&a.pts, &arcs[&(ea as usize)], sa_arc, ground);
                let (hb, side_b) = neighbourhood(&b.pts, &arcs[&(eb as usize)], sb_arc, ground);
                let gp = ground(p[0], p[1]) as f64;
                let a_over = ha - hb >= SEP_M && side_a >= gp + SIDE_M;
                let b_over = hb - ha >= SEP_M && side_b >= gp + SIDE_M;
                if a_over == b_over {
                    continue;
                }
                let (upper, over_s, lower_class, sin) = if a_over { (ea as usize, sa_arc, b.class, den.abs() / (lr * ls)) } else { (eb as usize, sb_arc, a.class, den.abs() / (lr * ls)) };
                let half = (lower_class.width() as f64 / (2.0 * sin) + 20.0).clamp(25.0, 90.0);
                spans.entry(upper).or_default().push(Span { s0: over_s - half, s1: over_s + half });
            }
        }
    }
    if spans.is_empty() {
        return 0;
    }
    let mut added = 0;
    let old = std::mem::take(&mut net.edges);
    for (i, e) in old.into_iter().enumerate() {
        match spans.remove(&i) {
            None => net.edges.push(e),
            Some(mut v) => {
                v.sort_by(|a, b| a.s0.partial_cmp(&b.s0).unwrap());
                let mut merged: Vec<Span> = Vec::new();
                for s in v {
                    match merged.last_mut() {
                        Some(m) if s.s0 <= m.s1 + 30.0 => m.s1 = m.s1.max(s.s1),
                        _ => merged.push(s),
                    }
                }
                added += merged.len();
                net.edges.extend(split_edge(&e, &merged));
            }
        }
    }
    added
}

/// Cut `e` at the span ends; the pieces inside a span become bridges (layer 1).
pub fn split_edge(e: &Edge, spans: &[Span]) -> Vec<Edge> {
    let s = arc_len(&e.pts);
    let total = *s.last().unwrap();
    let spans: Vec<(f64, f64)> = spans.iter().map(|sp| (sp.s0.max(0.0), sp.s1.min(total))).filter(|(a, b)| b - a >= 5.0).collect();
    let mut cuts: Vec<f64> = vec![0.0, total];
    for &(a, b) in &spans {
        cuts.push(a);
        cuts.push(b);
    }
    cuts.sort_by(|x, y| x.partial_cmp(y).unwrap());
    cuts.dedup_by(|x, y| (*x - *y).abs() < 1e-6);
    let mut out = Vec::new();
    for w in cuts.windows(2) {
        let (a, b) = (w[0], w[1]);
        if b - a < 0.5 {
            continue;
        }
        let mid = (a + b) * 0.5;
        let mut pts = vec![point_at(&e.pts, &s, a)];
        pts.extend(e.pts.iter().zip(&s).filter(|(_, si)| **si > a + 1e-6 && **si < b - 1e-6).map(|(p, _)| *p));
        pts.push(point_at(&e.pts, &s, b));
        let mut piece = e.clone();
        piece.pts = pts;
        if spans.iter().any(|&(x, y)| mid > x && mid < y) {
            piece.bridge = true;
            piece.layer = piece.layer.max(1);
        }
        out.push(piece);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roads::solve::tests::edge;

    /// A valley along y = 500: ground rises away from it.
    fn valley(_x: f64, y: f64) -> f32 {
        (10.0 * (1.0 - (-((y - 500.0) / 60.0).powi(2)).exp())) as f32
    }

    #[test]
    fn the_road_along_the_valley_side_goes_over_the_one_on_its_floor() {
        // Motorway along the valley floor (y = 500); a primary road crosses it from side to side with no node.
        let mut net = RoadNet::from_edges(2400.0, vec![edge(RoadClass::Motorway, false, vec![[0.0, 500.0], [1000.0, 500.0]]), edge(RoadClass::Primary, false, vec![[500.0, 0.0], [500.0, 1000.0]])]);
        let n = infer_bridges(&mut net, &valley);
        assert_eq!(n, 1);
        let bridges: Vec<_> = net.edges.iter().filter(|e| e.bridge).collect();
        assert_eq!(bridges.len(), 1);
        assert_eq!(bridges[0].class, RoadClass::Primary);
        let len = bridges[0].length();
        assert!((50.0..=180.0).contains(&len), "span {len}");
        // The bridge is centred on the crossing; the primary road became three pieces, the motorway is whole.
        assert_eq!(net.edges.iter().filter(|e| e.class == RoadClass::Primary).count(), 3);
        assert_eq!(net.edges.iter().filter(|e| e.class == RoadClass::Motorway).count(), 1);
        let (a, b) = (bridges[0].pts[0], *bridges[0].pts.last().unwrap());
        assert!(((a[1] + b[1]) / 2.0 - 500.0).abs() < 1.0, "centred on the crossing");
        // Total length is preserved by the split.
        let total: f64 = net.edges.iter().filter(|e| e.class == RoadClass::Primary).map(|e| e.length()).sum();
        assert!((total - 1000.0).abs() < 1e-6);
    }

    #[test]
    fn flat_ground_and_slopes_infer_nothing() {
        let roads = || vec![edge(RoadClass::Motorway, false, vec![[0.0, 500.0], [1000.0, 500.0]]), edge(RoadClass::Primary, false, vec![[500.0, 0.0], [500.0, 1000.0]])];
        let mut flat = RoadNet::from_edges(2400.0, roads());
        assert_eq!(infer_bridges(&mut flat, &|_, _| 0.0), 0);
        let mut slope = RoadNet::from_edges(2400.0, roads());
        assert_eq!(infer_bridges(&mut slope, &|x, y| ((x + y) * 0.05) as f32), 0);
        assert!(flat.edges.iter().all(|e| !e.bridge));
    }

    #[test]
    fn junctions_and_tagged_bridges_are_left_alone() {
        // The two roads end at the crossing point: a junction, not a grade separation.
        let mut net = RoadNet::from_edges(2400.0, vec![edge(RoadClass::Motorway, false, vec![[0.0, 500.0], [500.0, 500.0]]), edge(RoadClass::Primary, false, vec![[500.0, 500.0], [500.0, 1000.0]])]);
        assert_eq!(infer_bridges(&mut net, &valley), 0);
        // Already a bridge: level != 0, not a candidate.
        let mut tagged = RoadNet::from_edges(2400.0, vec![edge(RoadClass::Motorway, false, vec![[0.0, 500.0], [1000.0, 500.0]]), edge(RoadClass::Primary, true, vec![[500.0, 0.0], [500.0, 1000.0]])]);
        assert_eq!(infer_bridges(&mut tagged, &valley), 0);
    }
}
