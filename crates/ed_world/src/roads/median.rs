//! Median barriers on divided highways.
//!
//! Two one-way carriageways of a motorway or trunk road that run side by side in opposite directions, a few
//! metres to a few tens of metres apart, are a divided highway. A concrete barrier goes down the middle where
//! the gap is narrow; a wide grassy median gets none. The barrier follows the solved heights of both decks, so
//! it stands on the surface on bridges and ramps as well.

use super::net::{RoadNet, dist};
use super::solve::Solved;
use std::collections::BTreeMap;

/// No barrier where the carriageways are further apart than this (a wide median stays open).
pub const MAX_GAP_M: f64 = 18.0;
/// No barrier for a run shorter than this.
pub const MIN_RUN_M: f64 = 40.0;
const CELL: f64 = 32.0;

/// A barrier centreline: (x, y, z) in the network frame.
pub type Run = Vec<[f64; 3]>;

pub fn median_runs(net: &RoadNet, solved: &Solved) -> Vec<Run> {
    // Candidate carriageways.
    let cand: Vec<usize> = (0..solved.edges.len())
        .filter(|&si| {
            let e = &net.edges[solved.edges[si].net_idx];
            e.oneway
                && matches!(
                    e.class,
                    super::RoadClass::Motorway | super::RoadClass::Trunk
                )
                && !e.tunnel
        })
        .collect();
    if cand.len() < 2 {
        return Vec::new();
    }
    let mut grid: BTreeMap<(i64, i64), Vec<(u32, u32)>> = BTreeMap::new();
    for &si in &cand {
        for (k, p) in solved.edges[si].pts.iter().enumerate() {
            grid.entry(((p[0] / CELL).floor() as i64, (p[1] / CELL).floor() as i64))
                .or_default()
                .push((si as u32, k as u32));
        }
    }
    let dir = |si: usize, k: usize| -> [f64; 2] {
        let e = &solved.edges[si];
        let (a, b) = (
            e.pts[k.saturating_sub(1)],
            e.pts[(k + 1).min(e.pts.len() - 1)],
        );
        let l = dist(a, b).max(1e-9);
        [(b[0] - a[0]) / l, (b[1] - a[1]) / l]
    };
    let mut runs: Vec<Run> = Vec::new();
    for &si in &cand {
        let e = &solved.edges[si];
        let hw_i = net.edges[e.net_idx].class.width() as f64 / 2.0;
        let mut cur: Run = Vec::new();
        let mut cur_partner: Option<usize> = None;
        let mut flush = |cur: &mut Run| {
            if cur.len() >= 2
                && cur
                    .windows(2)
                    .map(|w| dist([w[0][0], w[0][1]], [w[1][0], w[1][1]]))
                    .sum::<f64>()
                    >= MIN_RUN_M
            {
                runs.push(std::mem::take(cur));
            } else {
                cur.clear();
            }
        };
        for k in 0..e.pts.len() {
            let p = e.pts[k];
            let di = dir(si, k);
            let (cx, cy) = ((p[0] / CELL).floor() as i64, (p[1] / CELL).floor() as i64);
            let mut best: Option<(f64, usize, usize)> = None;
            for gx in cx - 1..=cx + 1 {
                for gy in cy - 1..=cy + 1 {
                    for &(sj, kj) in grid.get(&(gx, gy)).into_iter().flatten() {
                        let sj = sj as usize;
                        // Only the lower-numbered carriageway draws the barrier.
                        if sj <= si {
                            continue;
                        }
                        let q = solved.edges[sj].pts[kj as usize];
                        let d = dist(p, q);
                        let hw_j = net.edges[solved.edges[sj].net_idx].class.width() as f64 / 2.0;
                        if d < hw_i + hw_j + 0.8 || d > MAX_GAP_M + hw_i + hw_j {
                            continue;
                        }
                        let dj = dir(sj, kj as usize);
                        if di[0] * dj[0] + di[1] * dj[1] > -0.8 {
                            continue;
                        }
                        if best.is_none_or(|b| d < b.0) {
                            best = Some((d, sj, kj as usize));
                        }
                    }
                }
            }
            match best {
                Some((_, sj, kj)) if cur_partner.is_none() || cur_partner == Some(sj) => {
                    let q = solved.edges[sj].pts[kj];
                    cur_partner = Some(sj);
                    cur.push([
                        (p[0] + q[0]) * 0.5,
                        (p[1] + q[1]) * 0.5,
                        ((e.z[k] + solved.edges[sj].z[kj]) * 0.5) as f64,
                    ]);
                }
                _ => {
                    flush(&mut cur);
                    cur_partner = None;
                }
            }
        }
        flush(&mut cur);
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roads::RoadClass;
    use crate::roads::solve::{solve, tests::edge};

    fn oneway(pts: Vec<[f64; 2]>) -> crate::roads::net::Edge {
        let mut e = edge(RoadClass::Motorway, false, pts);
        e.oneway = true;
        e
    }

    #[test]
    fn opposite_carriageways_get_a_barrier_down_the_middle() {
        // Eastbound at y=0, westbound at y=30 (hw 7.5 each: a 15 m gap between pavements).
        let net = RoadNet::from_edges(
            2400.0,
            vec![
                oneway(vec![[0.0, 0.0], [400.0, 0.0]]),
                oneway(vec![[400.0, 30.0], [0.0, 30.0]]),
            ],
        );
        let s = solve(&net, &|_, _| 0.0);
        let runs = median_runs(&net, &s);
        assert_eq!(runs.len(), 1, "{runs:?}");
        assert!(runs[0].iter().all(|p| (p[1] - 15.0).abs() < 0.01));
        assert!(runs[0].len() > 50);
    }

    #[test]
    fn wide_median_and_same_direction_roads_get_none() {
        let wide = RoadNet::from_edges(
            2400.0,
            vec![
                oneway(vec![[0.0, 0.0], [400.0, 0.0]]),
                oneway(vec![[400.0, 80.0], [0.0, 80.0]]),
            ],
        );
        assert!(median_runs(&wide, &solve(&wide, &|_, _| 0.0)).is_empty());
        let same = RoadNet::from_edges(
            2400.0,
            vec![
                oneway(vec![[0.0, 0.0], [400.0, 0.0]]),
                oneway(vec![[0.0, 30.0], [400.0, 30.0]]),
            ],
        );
        assert!(median_runs(&same, &solve(&same, &|_, _| 0.0)).is_empty());
    }
}
