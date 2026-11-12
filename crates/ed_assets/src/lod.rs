//! Discrete LOD chains (P1-D11), moved here from `ew` in M3.
//!
//! Each level aims at a quarter of LOD 0's triangles per step (1/4, 1/16, 1/64) with
//! meshoptimizer `simplify` (absolute error). It falls back to `simplify_sloppy` when that
//! cannot reach 60% of the previous level. Stops early under 8 triangles.
//!
//! **Stored error is measured, not reported.** The M2 caveat: the sloppy simplifier's own
//! error estimate understated coarse levels. Each level's error is the largest distance from
//! LOD 0's surface samples (every vertex used by a triangle, plus every triangle centroid) to
//! the simplified surface. A dropped shell therefore counts in full. The stored value is
//! `max(measured, reported, previous level)`, so the chain is non-decreasing.

use glam::Vec3;
use rayon::prelude::*;

use crate::geom::Lod;

/// One level's report.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LodReport {
    pub triangles: u32,
    /// Stored error (metres): max(measured, reported, previous).
    pub error: f32,
    /// What meshoptimizer reported, metres.
    pub reported: f32,
    /// What the surface measurement found, metres.
    pub measured: f32,
    pub sloppy: bool,
}

/// Builds up to `max_levels` levels (LOD 0 is `indices` unchanged).
pub fn build_chain(positions: &[[f32; 3]], indices: &[u32], max_levels: usize) -> (Vec<Lod>, Vec<LodReport>) {
    let scale = meshopt::simplify_scale_decoder(positions);
    let base_tris = indices.len() / 3;
    let samples = surface_samples(positions, indices);
    let mut lods = vec![Lod { indices: indices.to_vec(), error: 0.0 }];
    let mut report = vec![LodReport { triangles: base_tris as u32, error: 0.0, reported: 0.0, measured: 0.0, sloppy: false }];
    let mut prev_err = 0.0f32;
    for level in 1..max_levels {
        let target_tris = base_tris >> (2 * level);
        if target_tris < 8 {
            break;
        }
        let prev_len = lods[level - 1].indices.len();
        let target = target_tris * 3;
        let mut err = 0.0f32;
        let mut idx = meshopt::simplify_decoder(indices, positions, target, f32::MAX, meshopt::SimplifyOptions::ErrorAbsolute, Some(&mut err));
        let mut sloppy = false;
        if idx.len() as f32 > prev_len as f32 * 0.6 || idx.len() < 24 {
            sloppy = true;
            idx.clear();
            for mul in [1usize, 2, 4, 8] {
                let mut e = 0.0f32;
                let r = meshopt::simplify_sloppy_decoder(indices, positions, target * mul, f32::MAX, Some(&mut e));
                if r.len() >= 24 && (r.len() as f32) <= prev_len as f32 * 0.6 {
                    idx = r;
                    err = e * scale;
                    break;
                }
            }
        }
        if idx.len() < 24 || idx.len() >= prev_len {
            break;
        }
        let measured = surface_distance(positions, &idx, &samples);
        prev_err = prev_err.max(err).max(measured);
        report.push(LodReport { triangles: (idx.len() / 3) as u32, error: prev_err, reported: err, measured, sloppy });
        lods.push(Lod { indices: idx, error: prev_err });
    }
    (lods, report)
}

/// A chain from LODs someone else made (Megascans LOD0..LODn), sharing `positions`.
///
/// `supplied[0]` is LOD 0. Each later level's error is **measured** against LOD 0's surface
/// with [`surface_distance`] (the same measurement [`build_chain`] stores), kept
/// non-decreasing. Levels that do not have fewer triangles than the one before are skipped.
/// When fewer than `min_levels` remain, coarser levels are generated from the last one with
/// the same simplifier steps as [`build_chain`] (a quarter of the triangles per step) until
/// `min_levels` is reached or the mesh is under 8 triangles. Returns the levels, their
/// reports, and the index (into `supplied`) of each kept supplied level.
pub fn measured_chain(positions: &[[f32; 3]], supplied: &[Vec<u32>], min_levels: usize) -> (Vec<Lod>, Vec<LodReport>, Vec<usize>) {
    let lod0 = &supplied[0];
    let samples = surface_samples(positions, lod0);
    let mut lods = vec![Lod { indices: lod0.clone(), error: 0.0 }];
    let mut report = vec![LodReport { triangles: (lod0.len() / 3) as u32, error: 0.0, reported: 0.0, measured: 0.0, sloppy: false }];
    let mut kept = vec![0usize];
    let mut prev_err = 0.0f32;
    for (k, idx) in supplied.iter().enumerate().skip(1) {
        if idx.len() < 3 || idx.len() >= lods.last().expect("lod 0").indices.len() {
            continue;
        }
        let measured = surface_distance(positions, idx, &samples);
        prev_err = prev_err.max(measured);
        report.push(LodReport { triangles: (idx.len() / 3) as u32, error: prev_err, reported: 0.0, measured, sloppy: false });
        lods.push(Lod { indices: idx.clone(), error: prev_err });
        kept.push(k);
    }
    let scale = meshopt::simplify_scale_decoder(positions);
    while lods.len() < min_levels {
        let last = &lods.last().expect("lod 0").indices;
        let prev_len = last.len();
        let target_tris = prev_len / 3 / 4;
        if target_tris < 8 {
            break;
        }
        let target = target_tris * 3;
        let mut err = 0.0f32;
        let mut idx = meshopt::simplify_decoder(last, positions, target, f32::MAX, meshopt::SimplifyOptions::ErrorAbsolute, Some(&mut err));
        let mut sloppy = false;
        if idx.len() as f32 > prev_len as f32 * 0.6 || idx.len() < 24 {
            sloppy = true;
            idx.clear();
            for mul in [1usize, 2, 4, 8] {
                let mut e = 0.0f32;
                let r = meshopt::simplify_sloppy_decoder(last, positions, target * mul, f32::MAX, Some(&mut e));
                if r.len() >= 24 && (r.len() as f32) <= prev_len as f32 * 0.6 {
                    idx = r;
                    err = e * scale;
                    break;
                }
            }
        }
        if idx.len() < 24 || idx.len() >= prev_len {
            break;
        }
        let measured = surface_distance(positions, &idx, &samples);
        prev_err = prev_err.max(err).max(measured);
        report.push(LodReport { triangles: (idx.len() / 3) as u32, error: prev_err, reported: err, measured, sloppy });
        lods.push(Lod { indices: idx, error: prev_err });
    }
    (lods, report, kept)
}

/// Points on LOD 0's surface: every referenced vertex and every triangle centroid, in a fixed
/// order.
pub fn surface_samples(positions: &[[f32; 3]], indices: &[u32]) -> Vec<Vec3> {
    let mut used = vec![false; positions.len()];
    for &i in indices {
        used[i as usize] = true;
    }
    let mut s: Vec<Vec3> = positions.iter().zip(&used).filter(|(_, u)| **u).map(|(p, _)| Vec3::from(*p)).collect();
    for t in indices.chunks_exact(3) {
        let (a, b, c) = (Vec3::from(positions[t[0] as usize]), Vec3::from(positions[t[1] as usize]), Vec3::from(positions[t[2] as usize]));
        s.push((a + b + c) / 3.0);
    }
    s
}

/// Largest distance from `samples` to the triangles `indices` (uniform-grid search).
/// Order-independent (max), so the result does not depend on the thread count.
pub fn surface_distance(positions: &[[f32; 3]], indices: &[u32], samples: &[Vec3]) -> f32 {
    let tris: Vec<[Vec3; 3]> = indices.chunks_exact(3).map(|t| [0, 1, 2].map(|k| Vec3::from(positions[t[k] as usize]))).collect();
    if tris.is_empty() {
        return f32::INFINITY;
    }
    let grid = Grid::new(&tris);
    samples.par_iter().map(|p| grid.nearest(&tris, *p)).reduce(|| 0.0f32, f32::max)
}

struct Grid {
    lo: Vec3,
    cell: f32,
    dim: [i32; 3],
    start: Vec<u32>,
    items: Vec<u32>,
}

impl Grid {
    fn new(tris: &[[Vec3; 3]]) -> Grid {
        let mut lo = Vec3::splat(f32::MAX);
        let mut hi = Vec3::splat(f32::MIN);
        for t in tris {
            for p in t {
                lo = lo.min(*p);
                hi = hi.max(*p);
            }
        }
        let ext = (hi - lo).max(Vec3::splat(1e-4));
        let n = (tris.len() as f32).cbrt().clamp(1.0, 64.0);
        let cell = ext.max_element() / n;
        let dim = [0, 1, 2].map(|k| ((ext[k] / cell).ceil() as i32).max(1));
        let ncell = (dim[0] * dim[1] * dim[2]) as usize;
        let cells_of = |t: &[Vec3; 3]| {
            let a = t[0].min(t[1]).min(t[2]);
            let b = t[0].max(t[1]).max(t[2]);
            let c = |p: Vec3, k: usize| (((p[k] - lo[k]) / cell).floor() as i32).clamp(0, dim[k] - 1);
            ([c(a, 0), c(a, 1), c(a, 2)], [c(b, 0), c(b, 1), c(b, 2)])
        };
        let mut count = vec![0u32; ncell + 1];
        let idx = |x: i32, y: i32, z: i32| ((z * dim[1] + y) * dim[0] + x) as usize;
        for t in tris {
            let (a, b) = cells_of(t);
            for z in a[2]..=b[2] {
                for y in a[1]..=b[1] {
                    for x in a[0]..=b[0] {
                        count[idx(x, y, z) + 1] += 1;
                    }
                }
            }
        }
        for i in 1..count.len() {
            count[i] += count[i - 1];
        }
        let mut fill = count.clone();
        let mut items = vec![0u32; *count.last().unwrap() as usize];
        for (ti, t) in tris.iter().enumerate() {
            let (a, b) = cells_of(t);
            for z in a[2]..=b[2] {
                for y in a[1]..=b[1] {
                    for x in a[0]..=b[0] {
                        let c = idx(x, y, z);
                        items[fill[c] as usize] = ti as u32;
                        fill[c] += 1;
                    }
                }
            }
        }
        Grid { lo, cell, dim, start: count, items }
    }

    fn nearest(&self, tris: &[[Vec3; 3]], p: Vec3) -> f32 {
        let c = [0, 1, 2].map(|k| ((p[k] - self.lo[k]) / self.cell).floor() as i32);
        // Distance from p to the grid box; rings start being useful from there.
        let maxr = self.dim.iter().copied().max().unwrap() + c.iter().map(|v| v.abs()).max().unwrap() + 1;
        let mut best = f32::MAX;
        for r in 0..=maxr {
            for z in c[2] - r..=c[2] + r {
                for y in c[1] - r..=c[1] + r {
                    for x in c[0] - r..=c[0] + r {
                        let on_shell = (x - c[0]).abs() == r || (y - c[1]).abs() == r || (z - c[2]).abs() == r;
                        if !on_shell || x < 0 || y < 0 || z < 0 || x >= self.dim[0] || y >= self.dim[1] || z >= self.dim[2] {
                            continue;
                        }
                        let ci = ((z * self.dim[1] + y) * self.dim[0] + x) as usize;
                        for &t in &self.items[self.start[ci] as usize..self.start[ci + 1] as usize] {
                            let t = &tris[t as usize];
                            best = best.min(point_triangle_distance(p, t[0], t[1], t[2]));
                        }
                    }
                }
            }
            // Every cell outside ring r is at least r cells away from p's cell.
            if best <= r as f32 * self.cell {
                break;
            }
        }
        best
    }
}

/// Distance from `p` to triangle `abc` (Ericson, Real-Time Collision Detection 5.1.5).
pub fn point_triangle_distance(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> f32 {
    let ab = b - a;
    let ac = c - a;
    let ap = p - a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return p.distance(a);
    }
    let bp = p - b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return p.distance(b);
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return p.distance(a + ab * v);
    }
    let cp = p - c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return p.distance(c);
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return p.distance(a + ac * w);
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return p.distance(b + (c - b) * w);
    }
    let denom = 1.0 / (va + vb + vc);
    let v = vb * denom;
    let w = vc * denom;
    if !(v.is_finite() && w.is_finite()) {
        // Degenerate triangle: nearest of its corners.
        return p.distance(a).min(p.distance(b)).min(p.distance(c));
    }
    p.distance(a + ab * v + ac * w)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bumpy UV sphere plus many tiny separate shells (the sloppy simplifier's weak case).
    fn test_mesh() -> (Vec<[f32; 3]>, Vec<u32>) {
        let (rows, segs) = (48u32, 96u32);
        let mut p = Vec::new();
        let mut idx = Vec::new();
        for r in 0..=rows {
            let th = r as f32 / rows as f32 * std::f32::consts::PI;
            for s in 0..=segs {
                let ph = s as f32 / segs as f32 * std::f32::consts::TAU;
                let rad = 1.0 + 0.05 * (ph * 7.0).sin() * (th * 5.0).sin();
                p.push([rad * th.sin() * ph.cos(), rad * th.cos(), rad * th.sin() * ph.sin()]);
            }
        }
        for r in 0..rows {
            for s in 0..segs {
                let a = r * (segs + 1) + s;
                let b = a + segs + 1;
                idx.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
            }
        }
        // 64 small detached boxes floating 0.3 m off the sphere.
        for k in 0..64u32 {
            let ang = k as f32 * 0.7;
            let c = Vec3::new(1.4 * ang.cos(), (k as f32 / 32.0) - 1.0, 1.4 * ang.sin());
            let base = p.len() as u32;
            for i in 0..8u32 {
                let o = Vec3::new((i & 1) as f32, ((i >> 1) & 1) as f32, ((i >> 2) & 1) as f32) * 0.04;
                p.push((c + o).to_array());
            }
            for f in [[0, 2, 1], [1, 2, 3], [4, 5, 6], [5, 7, 6], [0, 1, 4], [1, 5, 4], [2, 6, 3], [3, 6, 7], [0, 4, 2], [2, 4, 6], [1, 3, 5], [3, 7, 5]] {
                idx.extend(f.iter().map(|&v| base + v));
            }
        }
        (p, idx)
    }

    #[test]
    fn error_is_non_decreasing_and_never_below_the_measurement() {
        let (p, idx) = test_mesh();
        let (lods, rep) = build_chain(&p, &idx, 4);
        assert!(lods.len() >= 3, "{rep:?}");
        for w in rep.windows(2) {
            assert!(w[1].error >= w[0].error, "{rep:?}");
            assert!(w[1].triangles < w[0].triangles, "{rep:?}");
        }
        for (l, r) in lods.iter().zip(&rep).skip(1) {
            // Independent re-measurement agrees with what is stored.
            let s = surface_samples(&p, &idx);
            let d = surface_distance(&p, &l.indices, &s);
            assert!(l.error >= d - 1e-6 && l.error >= r.reported, "{r:?} measured again {d}");
        }
    }

    #[test]
    fn dropped_shells_count_in_full() {
        let (p, idx) = test_mesh();
        // Keep only the sphere: the boxes 0.3-0.4 m away are now missing.
        let sphere_tris = 48 * 96 * 2 * 3;
        let s = surface_samples(&p, &idx);
        let d = surface_distance(&p, &idx[..sphere_tris], &s);
        assert!(d > 0.3, "{d}");
        assert!(surface_distance(&p, &idx, &s) < 1e-5);
    }

    #[test]
    fn point_triangle_cases() {
        let (a, b, c) = (Vec3::ZERO, Vec3::X, Vec3::Y);
        assert!((point_triangle_distance(Vec3::new(0.2, 0.2, 1.0), a, b, c) - 1.0).abs() < 1e-6);
        assert!((point_triangle_distance(Vec3::new(-1.0, 0.0, 0.0), a, b, c) - 1.0).abs() < 1e-6);
        assert!((point_triangle_distance(Vec3::new(1.0, 1.0, 0.0), a, b, c) - 0.5f32.sqrt()).abs() < 1e-6);
    }

    #[test]
    fn measured_chain_keeps_supplied_levels_and_extends_short_ones() {
        let (p, idx) = test_mesh();
        let sphere_tris = 48 * 96 * 2 * 3;
        // A "supplied" LOD 1: the sphere without the boxes.
        let (lods, rep, kept) = measured_chain(&p, &[idx.clone(), idx[..sphere_tris].to_vec()], 3);
        assert_eq!(kept, [0, 1]);
        assert!(lods.len() >= 3, "{rep:?}");
        assert!(rep[1].measured > 0.3 && rep[1].error == rep[1].measured, "{rep:?}");
        for w in rep.windows(2) {
            assert!(w[1].error >= w[0].error && w[1].triangles < w[0].triangles, "{rep:?}");
        }
        // A level that is not coarser is skipped.
        let (_, _, kept) = measured_chain(&p, &[idx.clone(), idx.clone()], 1);
        assert_eq!(kept, [0]);
    }
}
