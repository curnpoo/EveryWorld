//! Road heights for a whole network: ground roads, ramps, bridge decks and tunnel floors in one solve.
//!
//! Every road is resampled to ~4 m. Each sample gets a *target* (the smoothed ground for ground roads, a chord
//! between the abutments for decks, the lower convex hull of the ground for tunnels) and, where a deck crosses
//! another road, a *lower bound*: the lower road's height plus headroom plus the deck's depth, held over the
//! crossing's footprint. The solution is the lowest set of heights that
//!   * is at least the target and the lower bound everywhere, and
//!   * never changes faster than the road's maximum grade between neighbouring samples
//!     (across junctions too, so a ramp leaving a raised road climbs back to the ground at that grade).
//! That is a max-plus fixpoint, computed with a priority queue. Raising is the only operation, so a deck that
//! must clear a road simply lifts its own approaches onto fill, which is how real overpasses are built.
//! A second pass rounds the humps (moving average on raised samples), re-applies the bounds and the grade.
//!
//! Locality: a sample is influenced only by data within (height difference / grade) metres, a few hundred
//! metres, so tiles that share a halo compute the same heights at the same place.
//!
//! The crossings come from plan geometry only (no heights), so they are found once; the heights of the lower
//! road feed the bound of the upper one, which is why the solve runs a few rounds.

use super::RoadClass;
use super::ground::smooth_ground;
use super::measured::Measured;
use super::net::{RoadNet, dist};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

pub const SPACING_M: f64 = 4.0;
/// Usable headroom under a deck, in metres (tall trucks), measured to the underside.
pub const CLEAR_M: f32 = 6.0;
/// Rail needs more.
pub const CLEAR_RAIL_M: f32 = 6.5;
/// A deck is never closer than this to the ground target between its abutments.
pub const DECK_FLOOR_M: f32 = 0.7;
/// Crossing angles flatter than this (degrees) are parallel roads, not a crossing.
const MIN_ANGLE_DEG: f64 = 8.0;
/// A ramp that forks off a road overlaps it near the fork; no clearance is asked within this arc distance.
const FORK_EXEMPT_M: f32 = 160.0;
const ROUNDS: usize = 3;

pub fn max_grade(c: RoadClass) -> f32 {
    use RoadClass::*;
    match c {
        Motorway | Trunk => 0.06,
        Primary => 0.07,
        Secondary => 0.08,
        Tertiary => 0.09,
        Minor => 0.10,
        Service | Track | Path => 0.12,
        Rail => 0.025,
    }
}

/// Distance from the driving surface down to the underside of a deck of this class.
pub fn deck_depth(c: RoadClass) -> f32 {
    match c {
        RoadClass::Path | RoadClass::Track => 0.7,
        RoadClass::Rail => 1.8,
        _ => 1.5,
    }
}

#[derive(Debug, Clone)]
pub struct SolvedEdge {
    pub net_idx: usize,
    pub pts: Vec<[f64; 2]>,
    /// Arc length at each sample.
    pub s: Vec<f32>,
    /// Smoothed ground target.
    pub ground: Vec<f32>,
    /// Solved height of the driving surface.
    pub z: Vec<f32>,
    /// Tunnel section per sample: 0 open cut, 1 low arch, 2 full tube. Always 0 off tunnels.
    pub kind: Vec<u8>,
    pub node_a: usize,
    pub node_b: usize,
}

impl SolvedEdge {
    /// Height at arc length `s` (linear between samples).
    pub fn z_at(&self, s: f32) -> f32 {
        let i = self
            .s
            .partition_point(|&v| v <= s)
            .clamp(1, self.s.len() - 1);
        let (s0, s1) = (self.s[i - 1], self.s[i]);
        let t = if s1 > s0 {
            ((s - s0) / (s1 - s0)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.z[i - 1] + (self.z[i] - self.z[i - 1]) * t
    }
    pub fn length(&self) -> f32 {
        *self.s.last().unwrap_or(&0.0)
    }
}

/// Where an upper road passes over a lower one with no junction between them.
#[derive(Debug, Clone)]
pub struct Crossing {
    pub upper: usize,
    pub su: f32,
    pub lower: usize,
    pub sl: f32,
    pub p: [f64; 2],
    pub sin: f32,
}

#[derive(Debug, Clone, Default)]
pub struct Stats {
    pub samples: usize,
    pub crossings: usize,
    /// (upper solved edge, clearance in m) for crossings that miss the target.
    pub deficits: Vec<(usize, f32)>,
    pub max_fill: f32,
    pub max_cut: f32,
    pub bridges: usize,
    pub tunnels: usize,
    /// Decks and tunnels whose heights came from a survey.
    pub measured: usize,
}

#[derive(Debug, Clone, Default)]
pub struct Solved {
    pub edges: Vec<SolvedEdge>,
    /// Net edge index -> index into `edges`, for the edges that took part.
    pub of_net: Vec<Option<usize>>,
    pub crossings: Vec<Crossing>,
    pub stats: Stats,
}

#[derive(PartialEq, PartialOrd, Clone, Copy)]
struct F(f32);
impl Eq for F {}
impl Ord for F {
    fn cmp(&self, o: &Self) -> Ordering {
        self.partial_cmp(o).unwrap_or(Ordering::Equal)
    }
}

/// Resample a polyline to at most `SPACING_M` between samples; every original vertex is kept.
pub fn densify(pts: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut out: Vec<[f64; 2]> = Vec::with_capacity(pts.len() * 2);
    for w in pts.windows(2) {
        let d = dist(w[0], w[1]);
        let n = (d / SPACING_M).ceil().max(1.0) as usize;
        if out.is_empty() {
            out.push(w[0]);
        }
        for k in 1..=n {
            let t = k as f64 / n as f64;
            let p = [
                w[0][0] + (w[1][0] - w[0][0]) * t,
                w[0][1] + (w[1][1] - w[0][1]) * t,
            ];
            if dist(*out.last().unwrap(), p) > 0.05 {
                out.push(p);
            }
        }
    }
    out
}

fn node_key(p: [f64; 2]) -> (i64, i64) {
    ((p[0] * 2.0).round() as i64, (p[1] * 2.0).round() as i64)
}

/// Lower convex hull of (s, g) evaluated at every s.
fn lower_hull(s: &[f32], g: &[f32]) -> Vec<f32> {
    let mut hull: Vec<usize> = Vec::new();
    for i in 0..s.len() {
        while hull.len() >= 2 {
            let (a, b) = (hull[hull.len() - 2], hull[hull.len() - 1]);
            // b is above the chord a-i: drop it.
            let cross = (s[b] - s[a]) * (g[i] - g[a]) - (g[b] - g[a]) * (s[i] - s[a]);
            if cross <= 0.0 {
                hull.pop();
            } else {
                break;
            }
        }
        hull.push(i);
    }
    let mut out = vec![0.0; s.len()];
    for w in hull.windows(2) {
        let (a, b) = (w[0], w[1]);
        for i in a..=b {
            let t = if s[b] > s[a] {
                (s[i] - s[a]) / (s[b] - s[a])
            } else {
                0.0
            };
            out[i] = g[a] + (g[b] - g[a]) * t;
        }
    }
    if hull.len() == 1 {
        out.fill(g[0]);
    }
    out
}

fn box_smooth(v: &[f32], radius: usize, passes: usize, mask: &[bool]) -> Vec<f32> {
    let mut a = v.to_vec();
    for _ in 0..passes {
        let b = a.clone();
        for i in 1..a.len().saturating_sub(1) {
            if !mask[i] {
                continue;
            }
            // Symmetric window so the ends stay put.
            let r = radius.min(i).min(a.len() - 1 - i);
            a[i] = b[i - r..=i + r].iter().sum::<f32>() / (2 * r + 1) as f32;
        }
    }
    a
}

/// Short runs of a tunnel section kind are absorbed by the longer neighbour.
fn merge_short_runs(kind: &mut [u8], s: &[f32], min_m: f32) {
    for _ in 0..3 {
        let mut i = 0;
        while i < kind.len() {
            let mut j = i;
            while j + 1 < kind.len() && kind[j + 1] == kind[i] {
                j += 1;
            }
            let len = s[j] - s[i] + 4.0;
            if len < min_m && (i > 0 || j + 1 < kind.len()) {
                let prev = (i > 0).then(|| kind[i - 1]);
                let next = (j + 1 < kind.len()).then(|| kind[j + 1]);
                let k = match (prev, next) {
                    (Some(p), Some(n)) => p.min(n).max(kind[i].min(p.max(n))),
                    (Some(p), None) => p,
                    (None, Some(n)) => n,
                    _ => kind[i],
                };
                for x in &mut kind[i..=j] {
                    *x = k;
                }
            }
            i = j + 1;
        }
    }
}

struct Graph {
    adj: Vec<Vec<(u32, f32, f32)>>,
}

fn raise(z: &mut [f32], g: &Graph) {
    let mut heap: BinaryHeap<(F, u32)> = z
        .iter()
        .enumerate()
        .map(|(i, &v)| (F(v), i as u32))
        .collect();
    while let Some((F(v), i)) = heap.pop() {
        if v < z[i as usize] {
            continue;
        }
        for &(j, d, gr) in &g.adj[i as usize] {
            let cand = v - gr * d;
            if cand > z[j as usize] + 1e-4 {
                z[j as usize] = cand;
                heap.push((F(cand), j));
            }
        }
    }
}

/// Find where upper roads pass over lower ones. Pure geometry.
fn find_crossings(edges: &[SolvedEdge], net: &RoadNet) -> Vec<Crossing> {
    let level = |e: &SolvedEdge| net.edges[e.net_idx].level();
    const CELL: f64 = 48.0;
    // Only segments of elevated roads go in the grid; every road's segments then look up their neighbourhood.
    // (A crossing always has an elevated road: that keeps the grid to a few percent of all segments.)
    let mut grid: BTreeMap<(i64, i64), Vec<(u32, u32)>> = BTreeMap::new();
    for (ei, e) in edges.iter().enumerate() {
        if level(e) < 1 {
            continue;
        }
        for k in 0..e.pts.len() - 1 {
            let (a, b) = (e.pts[k], e.pts[k + 1]);
            let (x0, x1) = (
                (a[0].min(b[0]) / CELL).floor() as i64,
                (a[0].max(b[0]) / CELL).floor() as i64,
            );
            let (y0, y1) = (
                (a[1].min(b[1]) / CELL).floor() as i64,
                (a[1].max(b[1]) / CELL).floor() as i64,
            );
            for gx in x0..=x1 {
                for gy in y0..=y1 {
                    grid.entry((gx, gy))
                        .or_default()
                        .push((ei as u32, k as u32));
                }
            }
        }
    }
    let mut pairs: Vec<((u32, u32), (u32, u32))> = Vec::new();
    for (ei, e) in edges.iter().enumerate() {
        for k in 0..e.pts.len() - 1 {
            let (a, b) = (e.pts[k], e.pts[k + 1]);
            let (x0, x1) = (
                (a[0].min(b[0]) / CELL).floor() as i64,
                (a[0].max(b[0]) / CELL).floor() as i64,
            );
            let (y0, y1) = (
                (a[1].min(b[1]) / CELL).floor() as i64,
                (a[1].max(b[1]) / CELL).floor() as i64,
            );
            for gx in x0..=x1 {
                for gy in y0..=y1 {
                    for &(ej, sj) in grid.get(&(gx, gy)).into_iter().flatten() {
                        if ej as usize != ei {
                            pairs.push(((ei as u32, k as u32), (ej, sj)));
                        }
                    }
                }
            }
        }
    }
    let mut seen: BTreeSet<(u32, u32, u32, u32)> = BTreeSet::new();
    let mut out: Vec<Crossing> = Vec::new();
    for &((ea, sa), (eb, sb)) in &pairs {
        {
            {
                let (la, lb) = (level(&edges[ea as usize]), level(&edges[eb as usize]));
                if la == lb || la.max(lb) < 1 {
                    continue;
                }
                let key = if ea < eb {
                    (ea, sa, eb, sb)
                } else {
                    (eb, sb, ea, sa)
                };
                if !seen.insert(key) {
                    continue;
                }
                let (a, b) = (&edges[ea as usize], &edges[eb as usize]);
                let (p0, p1) = (a.pts[sa as usize], a.pts[sa as usize + 1]);
                let (q0, q1) = (b.pts[sb as usize], b.pts[sb as usize + 1]);
                let (r, s) = (
                    [p1[0] - p0[0], p1[1] - p0[1]],
                    [q1[0] - q0[0], q1[1] - q0[1]],
                );
                let den = r[0] * s[1] - r[1] * s[0];
                let (lr, ls) = (r[0].hypot(r[1]), s[0].hypot(s[1]));
                if den.abs() < 1e-9 || lr < 1e-9 || ls < 1e-9 {
                    continue;
                }
                let sin = den.abs() / (lr * ls);
                if sin < MIN_ANGLE_DEG.to_radians().sin() {
                    continue;
                }
                let qp = [q0[0] - p0[0], q0[1] - p0[1]];
                let t = (qp[0] * s[1] - qp[1] * s[0]) / den;
                let u = (qp[0] * r[1] - qp[1] * r[0]) / den;
                if !(-1e-9..=1.0 + 1e-9).contains(&t) || !(-1e-9..=1.0 + 1e-9).contains(&u) {
                    continue;
                }
                let (sua, sub) = (
                    a.s[sa as usize] + (t * lr) as f32,
                    b.s[sb as usize] + (u * ls) as f32,
                );
                let (upper, lower, su, sl) = if la > lb {
                    (ea, eb, sua, sub)
                } else {
                    (eb, ea, sub, sua)
                };
                // A ramp forking off a road overlaps it near the shared node: not a crossing.
                let (eu, el) = (&edges[upper as usize], &edges[lower as usize]);
                let arc_to = |e: &SolvedEdge, s: f32, n: usize| -> Option<f32> {
                    if e.node_a == n {
                        Some(s)
                    } else if e.node_b == n {
                        Some(e.length() - s)
                    } else {
                        None
                    }
                };
                let mut shared = false;
                for n in [eu.node_a, eu.node_b] {
                    if let (Some(du), Some(dl)) = (arc_to(eu, su, n), arc_to(el, sl, n)) {
                        if du < FORK_EXEMPT_M && dl < FORK_EXEMPT_M {
                            shared = true;
                        }
                    }
                }
                if shared {
                    continue;
                }
                let p = [p0[0] + r[0] * t, p0[1] + r[1] * t];
                out.push(Crossing {
                    upper: upper as usize,
                    su,
                    lower: lower as usize,
                    sl,
                    p,
                    sin: sin as f32,
                });
            }
        }
    }
    out.sort_by(|a, b| (a.upper, a.lower, a.su.to_bits()).cmp(&(b.upper, b.lower, b.su.to_bits())));
    // A crossing on a polyline vertex is found by each of the segments that meet there: keep one.
    let mut merged: Vec<Crossing> = Vec::with_capacity(out.len());
    for c in out {
        if merged.last().is_some_and(|m| {
            m.upper == c.upper
                && m.lower == c.lower
                && (m.su - c.su).abs() < 12.0
                && (m.sl - c.sl).abs() < 12.0
        }) {
            continue;
        }
        merged.push(c);
    }
    merged
}

/// A deck's or tunnel floor's heights from the survey, if enough of the edge is covered: gaps filled by
/// interpolation, spikes removed by a median filter, grade-limited. `None` = infer as usual.
fn measured_profile(e: &SolvedEdge, m: &Measured, want: i8) -> Option<Vec<f32>> {
    let n = e.pts.len();
    let mut z: Vec<Option<f32>> = (0..n)
        .map(|k| {
            let (a, b) = (e.pts[k.saturating_sub(1)], e.pts[(k + 1).min(n - 1)]);
            let l = dist(a, b).max(1e-9);
            let (zm, _) = m.at(e.pts[k], [(b[0] - a[0]) / l, (b[1] - a[1]) / l], want)?;
            // A height far from the ground it should be above or below is a mismatch (another road).
            let g = e.ground[k];
            let sane = if want > 0 { zm >= g - 3.0 && zm <= g + 120.0 } else { zm <= g + 3.0 && zm >= g - 120.0 };
            sane.then_some(zm)
        })
        .collect();
    let hits = z.iter().filter(|v| v.is_some()).count();
    if hits < 3 || (hits as f64) < 0.6 * n as f64 {
        return None;
    }
    // Fill gaps: linear between known samples, constant past the ends.
    let known: Vec<usize> = (0..n).filter(|&k| z[k].is_some()).collect();
    let mut out = vec![0.0f32; n];
    for k in 0..n {
        out[k] = match z[k] {
            Some(v) => v,
            None => {
                let prev = known.iter().rev().find(|&&i| i < k);
                let next = known.iter().find(|&&i| i > k);
                match (prev, next) {
                    (Some(&a), Some(&b)) => {
                        let t = (e.s[k] - e.s[a]) / (e.s[b] - e.s[a]).max(1e-6);
                        z[a].unwrap() + (z[b].unwrap() - z[a].unwrap()) * t
                    }
                    (Some(&a), None) => z[a].unwrap(),
                    (None, Some(&b)) => z[b].unwrap(),
                    _ => unreachable!(),
                }
            }
        };
    }
    // Median of three removes single-sample spikes, then a grade limiter (12 %) in both directions.
    let src = out.clone();
    for k in 1..n - 1 {
        let mut t = [src[k - 1], src[k], src[k + 1]];
        t.sort_by(|a, b| a.partial_cmp(b).unwrap());
        out[k] = t[1];
    }
    for k in 1..n {
        let d = (e.s[k] - e.s[k - 1]).max(1e-3);
        out[k] = out[k].clamp(out[k - 1] - 0.12 * d, out[k - 1] + 0.12 * d);
    }
    for k in (0..n - 1).rev() {
        let d = (e.s[k + 1] - e.s[k]).max(1e-3);
        out[k] = out[k].clamp(out[k + 1] - 0.12 * d, out[k + 1] + 0.12 * d);
    }
    z.clear();
    Some(out)
}

pub fn solve(net: &RoadNet, ground: &dyn Fn(f64, f64) -> f32) -> Solved {
    solve_with(net, ground, None)
}

pub fn solve_with(net: &RoadNet, ground: &dyn Fn(f64, f64) -> f32, measured: Option<&Measured>) -> Solved {
    let mut edges: Vec<SolvedEdge> = Vec::new();
    let mut of_net: Vec<Option<usize>> = vec![None; net.edges.len()];
    let mut node_ids: BTreeMap<(i64, i64), usize> = BTreeMap::new();
    for (ni, e) in net.edges.iter().enumerate() {
        if !e.in_graph() || e.pts.len() < 2 {
            continue;
        }
        let pts = densify(&e.pts);
        if pts.len() < 2 {
            continue;
        }
        let mut s = vec![0.0f32; pts.len()];
        for i in 1..pts.len() {
            s[i] = s[i - 1] + dist(pts[i - 1], pts[i]) as f32;
        }
        let g: Vec<f32> = pts
            .iter()
            .map(|p| smooth_ground(ground, p[0], p[1]))
            .collect();
        let next = node_ids.len();
        let a = *node_ids.entry(node_key(pts[0])).or_insert(next);
        let next = node_ids.len();
        let b = *node_ids.entry(node_key(pts[pts.len() - 1])).or_insert(next);
        of_net[ni] = Some(edges.len());
        edges.push(SolvedEdge {
            net_idx: ni,
            pts,
            s,
            ground: g,
            z: Vec::new(),
            kind: Vec::new(),
            node_a: a,
            node_b: b,
        });
    }
    let mut stats = Stats::default();
    if edges.is_empty() {
        return Solved {
            edges,
            of_net,
            crossings: Vec::new(),
            stats,
        };
    }
    // Variables: nodes first, then each edge's interior samples.
    let nn = node_ids.len();
    let mut base = Vec::with_capacity(edges.len());
    let mut nv = nn;
    for e in &edges {
        base.push(nv);
        nv += e.pts.len() - 2;
    }
    let var = |ei: usize, k: usize, edges: &[SolvedEdge]| -> usize {
        let last = edges[ei].pts.len() - 1;
        if k == 0 {
            edges[ei].node_a
        } else if k == last {
            edges[ei].node_b
        } else {
            base[ei] + k - 1
        }
    };
    // Targets, floors.
    let mut is_measured = vec![false; edges.len()];
    let mut z0 = vec![f32::MIN; nv];
    let mut floor = vec![f32::MIN; nv];
    for (ei, e) in edges.iter_mut().enumerate() {
        let ne = &net.edges[e.net_idx];
        let n = e.pts.len();
        // Surveyed heights, where the source has this deck or tunnel.
        let meas_target = measured.filter(|_| ne.bridge || ne.tunnel).and_then(|m| measured_profile(e, m, if ne.bridge { 1 } else { -1 }));
        is_measured[ei] = meas_target.is_some();
        let mut target: Vec<f32> = if let Some(t) = meas_target {
            t
        } else if ne.tunnel {
            lower_hull(&e.s, &e.ground)
        } else if ne.bridge {
            let (ga, gb, l) = (e.ground[0], e.ground[n - 1], e.length().max(1e-3));
            e.s.iter().map(|&s| ga + (gb - ga) * s / l).collect()
        } else {
            let mask = vec![true; n];
            box_smooth(&e.ground, 4, 2, &mask)
        };
        // Ends are nodes: they carry the ground target exactly (a surveyed deck keeps its measured abutments, and
        // the approach roads are lifted to meet them).
        if !is_measured[ei] {
            target[0] = e.ground[0];
            target[n - 1] = e.ground[n - 1];
        }
        for k in 0..n {
            let v = {
                let last = n - 1;
                if k == 0 {
                    e.node_a
                } else if k == last {
                    e.node_b
                } else {
                    base[ei] + k - 1
                }
            };
            z0[v] = z0[v].max(target[k]);
            if ne.bridge && !is_measured[ei] && k > 0 && k < n - 1 {
                // Eases in from the abutments so the floor itself never demands a steep approach.
                let ease = (0.05 * e.s[k].min(e.length() - e.s[k])).min(DECK_FLOOR_M);
                floor[v] = floor[v].max(e.ground[k] + ease);
            }
            if !ne.bridge && !ne.tunnel {
                // A ground road never sinks under its own smoothed target.
                floor[v] = floor[v].max(target[k]);
            }
        }
        e.z = target;
        e.kind = vec![0; n];
    }
    for v in 0..nv {
        z0[v] = z0[v].max(floor[v]);
    }
    // Adjacency with grade limits (natural ground slope is allowed through).
    let mut adj: Vec<Vec<(u32, f32, f32)>> = vec![Vec::new(); nv];
    for (ei, e) in edges.iter().enumerate() {
        let gr0 = max_grade(net.edges[e.net_idx].class);
        for k in 0..e.pts.len() - 1 {
            let (va, vb) = (var(ei, k, &edges), var(ei, k + 1, &edges));
            if va == vb {
                continue;
            }
            let d = (e.s[k + 1] - e.s[k]).max(1e-3);
            let slope = ((e.ground[k + 1] - e.ground[k]) / d).abs() * 1.1;
            let gr = gr0.max(slope);
            adj[va].push((vb as u32, d, gr));
            adj[vb].push((va as u32, d, gr));
        }
    }
    let graph = Graph { adj };
    let crossings = find_crossings(&edges, net);
    stats.crossings = crossings.len();

    let mut z = z0.clone();
    raise(&mut z, &graph);
    let gather = |z: &[f32], edges: &mut [SolvedEdge]| {
        for (ei, e) in edges.iter_mut().enumerate() {
            let n = e.pts.len();
            for k in 0..n {
                let v = if k == 0 {
                    e.node_a
                } else if k == n - 1 {
                    e.node_b
                } else {
                    base[ei] + k - 1
                };
                e.z[k] = z[v];
            }
        }
    };
    gather(&z, &mut edges);
    for _ in 0..ROUNDS {
        // Lower bounds from the current heights of the roads underneath.
        let mut lb = vec![f32::MIN; nv];
        for c in &crossings {
            // A surveyed deck is where it is: its headroom is real, not a requirement to meet.
            if is_measured[c.upper] {
                continue;
            }
            let (up, lo) = (&edges[c.upper], &edges[c.lower]);
            let (cu, cl) = (net.edges[up.net_idx].class, net.edges[lo.net_idx].class);
            let clear = if cl == RoadClass::Rail {
                CLEAR_RAIL_M
            } else {
                CLEAR_M
            };
            let req = lo.z_at(c.sl) + clear + deck_depth(cu);
            let half = (cl.width() / (2.0 * c.sin) + cu.width() / 2.0 + 10.0).min(70.0);
            for k in 0..up.pts.len() {
                if (up.s[k] - c.su).abs() <= half {
                    let v = var(c.upper, k, &edges);
                    lb[v] = lb[v].max(req);
                }
            }
        }
        for v in 0..nv {
            z[v] = z[v].max(lb[v]).max(floor[v]);
        }
        raise(&mut z, &graph);
        // Round the humps: smooth raised samples only, then restore the bounds and the grade.
        gather(&z, &mut edges);
        let mut smoothed = z.clone();
        for (ei, e) in edges.iter().enumerate() {
            let ne = &net.edges[e.net_idx];
            if ne.tunnel {
                continue;
            }
            let n = e.pts.len();
            let mask: Vec<bool> = (0..n)
                .map(|k| ne.bridge || e.z[k] - e.ground[k] > 0.2)
                .collect();
            let sm = box_smooth(&e.z, 5, 2, &mask);
            for k in 1..n - 1 {
                if mask[k] {
                    smoothed[base[ei] + k - 1] = sm[k];
                }
            }
        }
        // Rounding never drops a deck under its chord, a road under its target, or anything under its bounds.
        for v in 0..nv {
            z[v] = smoothed[v].max(lb[v]).max(floor[v]).max(z0[v]);
        }
        raise(&mut z, &graph);
        gather(&z, &mut edges);
    }
    // Tunnel sections from the cover over the floor.
    for e in edges.iter_mut() {
        let ne = &net.edges[e.net_idx];
        if !ne.tunnel {
            continue;
        }
        let mut kind: Vec<u8> = e
            .ground
            .iter()
            .zip(&e.z)
            .map(|(g, z)| {
                if g - z < 1.5 {
                    0
                } else if g - z < 5.0 {
                    1
                } else {
                    2
                }
            })
            .collect();
        merge_short_runs(&mut kind, &e.s, 24.0);
        e.kind = kind;
    }
    // Statistics and the clearance audit.
    stats.measured = is_measured.iter().filter(|b| **b).count();
    for e in &edges {
        let ne = &net.edges[e.net_idx];
        stats.samples += e.pts.len();
        stats.bridges += ne.bridge as usize;
        stats.tunnels += ne.tunnel as usize;
        if !ne.bridge && !ne.tunnel {
            for k in 0..e.pts.len() {
                stats.max_fill = stats.max_fill.max(e.z[k] - e.ground[k]);
                stats.max_cut = stats.max_cut.max(e.ground[k] - e.z[k]);
            }
        }
    }
    for c in &crossings {
        if is_measured[c.upper] {
            continue; // surveyed headroom is what it is
        }
        let (up, lo) = (&edges[c.upper], &edges[c.lower]);
        let cu = net.edges[up.net_idx].class;
        let clear = up.z_at(c.su) - lo.z_at(c.sl) - deck_depth(cu);
        if clear < CLEAR_M - 0.1 {
            stats.deficits.push((c.upper, clear));
        }
    }
    Solved {
        edges,
        of_net,
        crossings,
        stats,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::roads::net::Edge;

    pub fn edge(class: RoadClass, bridge: bool, pts: Vec<[f64; 2]>) -> Edge {
        Edge {
            class,
            layer: if bridge { 1 } else { 0 },
            bridge,
            tunnel: false,
            ramp: false,
            oneway: false,
            paved: true,
            cycleway: false,
            tags: Default::default(),
            pts,
        }
    }

    fn flat(_: f64, _: f64) -> f32 {
        0.0
    }

    fn grade_ok(s: &SolvedEdge, g: f32) -> bool {
        s.z.windows(2)
            .zip(s.s.windows(2))
            .all(|(z, s)| (z[1] - z[0]).abs() <= g * (s[1] - s[0]) * 1.02 + 1e-3)
    }

    #[test]
    fn long_overpass_clears_the_road_under_it() {
        // A motorway along x, a 600 m bridge along y passing over it.
        let net = RoadNet::from_edges(
            2400.0,
            vec![
                edge(
                    RoadClass::Motorway,
                    false,
                    vec![[0.0, 300.0], [600.0, 300.0]],
                ),
                edge(RoadClass::Primary, true, vec![[300.0, 0.0], [300.0, 600.0]]),
            ],
        );
        let s = solve(&net, &flat);
        assert_eq!(s.crossings.len(), 1);
        assert!(s.stats.deficits.is_empty(), "{:?}", s.stats.deficits);
        let up = &s.edges[s.of_net[1].unwrap()];
        let lo = &s.edges[s.of_net[0].unwrap()];
        let c = &s.crossings[0];
        let clear = up.z_at(c.su) - lo.z_at(c.sl) - deck_depth(RoadClass::Primary);
        assert!(clear >= CLEAR_M - 0.05, "{clear}");
        assert!(grade_ok(up, max_grade(RoadClass::Primary)));
        assert!(
            up.z[0].abs() < 1e-3 && up.z.last().unwrap().abs() < 1e-3,
            "abutments stay on the ground"
        );
    }

    #[test]
    fn short_overpass_lifts_its_approaches_onto_fill() {
        // 40 m bridge over the motorway, ground roads continue on both sides and meet it at nodes.
        let net = RoadNet::from_edges(
            2400.0,
            vec![
                edge(
                    RoadClass::Motorway,
                    false,
                    vec![[0.0, 300.0], [600.0, 300.0]],
                ),
                edge(
                    RoadClass::Primary,
                    false,
                    vec![[300.0, 0.0], [300.0, 280.0]],
                ),
                edge(
                    RoadClass::Primary,
                    true,
                    vec![[300.0, 280.0], [300.0, 320.0]],
                ),
                edge(
                    RoadClass::Primary,
                    false,
                    vec![[300.0, 320.0], [300.0, 600.0]],
                ),
            ],
        );
        let s = solve(&net, &flat);
        assert!(s.stats.deficits.is_empty(), "{:?}", s.stats.deficits);
        let deck = &s.edges[s.of_net[2].unwrap()];
        assert!(deck.z[0] > 3.0, "abutment raised: {}", deck.z[0]);
        let appr = &s.edges[s.of_net[1].unwrap()];
        assert!(
            appr.z.last().unwrap() > &3.0 && appr.z[0] < 0.01,
            "approach climbs from the ground"
        );
        assert!(grade_ok(appr, max_grade(RoadClass::Primary)));
        assert!(s.stats.max_fill > 3.0);
    }

    #[test]
    fn flat_roads_stay_on_the_ground() {
        let net = RoadNet::from_edges(
            2400.0,
            vec![edge(
                RoadClass::Secondary,
                false,
                vec![[0.0, 0.0], [500.0, 100.0], [900.0, 50.0]],
            )],
        );
        let s = solve(&net, &flat);
        assert!(s.edges[0].z.iter().all(|z| z.abs() < 1e-3));
        assert!(s.stats.max_fill < 1e-3 && s.stats.max_cut < 1e-3);
    }

    #[test]
    fn natural_slope_is_not_treated_as_a_grade_violation() {
        // 15 % hillside, steeper than the motorway cap: the road follows it, no giant fill.
        let hill = |x: f64, _y: f64| (x * 0.15) as f32;
        let net = RoadNet::from_edges(
            2400.0,
            vec![edge(
                RoadClass::Motorway,
                false,
                vec![[0.0, 0.0], [400.0, 0.0]],
            )],
        );
        let s = solve(&net, &hill);
        assert!(s.stats.max_fill < 1.5, "{}", s.stats.max_fill);
    }

    #[test]
    fn tunnel_floor_hugs_under_a_hill() {
        let hill = |x: f64, _y: f64| (40.0 * (-((x - 300.0) / 120.0).powi(2)).exp()) as f32;
        let mut t = edge(RoadClass::Motorway, false, vec![[0.0, 0.0], [600.0, 0.0]]);
        t.tunnel = true;
        t.layer = -1;
        let net = RoadNet::from_edges(2400.0, vec![t]);
        let s = solve(&net, &hill);
        let e = &s.edges[0];
        assert!(
            e.z.iter().zip(&e.ground).all(|(z, g)| z <= &(g + 1e-3)),
            "floor never above ground"
        );
        assert!(e.kind.iter().any(|&k| k == 2), "deep middle is a full tube");
        assert_eq!(e.kind[0], 0, "portal region is open");
    }

    #[test]
    fn a_surveyed_deck_keeps_its_measured_height_and_lifts_its_approaches() {
        use crate::roads::measured::{MSeg, Measured};
        let roads = || {
            vec![
                edge(RoadClass::Primary, false, vec![[0.0, 300.0], [300.0, 300.0]]),
                edge(RoadClass::Primary, true, vec![[300.0, 300.0], [700.0, 300.0]]),
                edge(RoadClass::Primary, false, vec![[700.0, 300.0], [1000.0, 300.0]]),
            ]
        };
        let net = RoadNet::from_edges(2400.0, roads());
        let plain = solve(&net, &flat);
        let deck_plain = &plain.edges[plain.of_net[1].unwrap()];
        assert!(deck_plain.z.iter().all(|z| *z < 0.8), "inferred: a deck over flat ground stays within its 0.7 m floor");
        // The survey says the deck is 12 m up (and so is the ground-level road next to it, so the datum offset is 0).
        let m = Measured::from_segs(
            vec![
                MSeg { a: [300.0, 300.0], b: [700.0, 300.0], za: 12.0, zb: 12.0, level: 1, lanes: None, width: None },
                MSeg { a: [0.0, 300.0], b: [200.0, 300.0], za: 0.0, zb: 0.0, level: 0, lanes: None, width: None },
            ],
            true,
            &flat,
        );
        let s = solve_with(&net, &flat, Some(&m));
        assert_eq!(s.stats.measured, 1);
        let deck = &s.edges[s.of_net[1].unwrap()];
        assert!(deck.z.iter().all(|z| (z - 12.0).abs() < 0.3), "{:?}", &deck.z[..4]);
        let appr = &s.edges[s.of_net[0].unwrap()];
        let at_100m_back = appr.z_at(appr.length() - 100.0);
        assert!(at_100m_back >= 12.0 - 100.0 * max_grade(RoadClass::Primary) - 0.2, "approach climbs to meet the deck: {at_100m_back}");
        assert!(appr.z[0] < 0.5, "and is back on the ground far away");
    }

    #[test]
    fn same_result_from_a_shifted_frame() {
        // Locality/determinism: translating the network by whole tile widths changes nothing (to f32 rounding).
        let mk = |ox: f64, oy: f64| {
            RoadNet::from_edges(
                2400.0,
                vec![
                    edge(
                        RoadClass::Motorway,
                        false,
                        vec![[ox, oy + 300.0], [ox + 600.0, oy + 300.0]],
                    ),
                    edge(
                        RoadClass::Primary,
                        true,
                        vec![[ox + 300.0, oy], [ox + 300.0, oy + 600.0]],
                    ),
                ],
            )
        };
        let g = |ox: f64, oy: f64| {
            move |x: f64, y: f64| {
                ((x - ox) * 0.01 + (y - oy) * 0.005 + 6.0 * ((x - ox) * 0.02).sin()) as f32
            }
        };
        let (a, b) = (
            solve(&mk(0.0, 0.0), &g(0.0, 0.0)),
            solve(&mk(2400.0, 4800.0), &g(2400.0, 4800.0)),
        );
        for (ea, eb) in a.edges.iter().zip(&b.edges) {
            for (za, zb) in ea.z.iter().zip(&eb.z) {
                assert!((za - zb).abs() < 1e-2, "{za} vs {zb}");
            }
        }
    }
}
