//! CPU-side cluster (meshlet) builder for the GPU-driven renderer (pure Rust, no dependencies).
//!
//! [`build`] splits an indexed triangle mesh into clusters of at most [`MAX_TRIS`] triangles
//! and [`MAX_VERTS`] unique vertices. Triangles are sorted by the 3D Morton code of their
//! centroid and packed greedily in that order, so consecutive clusters are spatially compact.
//! Each cluster gets its own vertex copies (re-indexed locally, `u16`), so a cluster can be
//! drawn with `base_vertex` plus its local indices.
//!
//! Each cluster also carries a bounding sphere and a normal cone. [`cluster_visible`] combines
//! the sphere-vs-frustum test from [`crate::cull`] with the backface cone test.
//!
//! Determinism: no `HashMap` is used. Every ordering comes from a sort on a total key or from
//! insertion order, so the same input gives the same output on every machine.
//!
//! Winding follows the rest of the crate: counter-clockwise seen from the front, so
//! `(b - a) x (c - a)` is the front-face normal.

use crate::cull::sphere_visible;
use std::collections::BTreeMap;

/// Maximum triangles per cluster.
pub const MAX_TRIS: usize = 128;
/// Maximum unique vertices per cluster.
pub const MAX_VERTS: usize = 128;

/// Bits per axis for the Morton code. 3 x 21 = 63 bits fit in a `u64`.
const MORTON_BITS: u32 = 21;

/// One cluster. Layout is 48 bytes, `repr(C)`, all fields 4-byte aligned.
///
/// * `first_index` is an offset into [`ClusteredMesh::indices`] (in `u16` units).
/// * `base_vertex` is the offset of the cluster's first vertex in [`ClusteredMesh::vertices`].
/// * Culling: the cluster is backface-culled when `dot(normalize(center - cam), cone_axis) >=
///   cone_cutoff`. `cone_cutoff` is `sin` of the normal-cone half angle (see [`build`] for why
///   it is not the cosine). A cluster with `cone_cutoff >= 1.0` and a zero axis is never culled.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cluster {
    pub center: [f32; 3],
    pub radius: f32,
    pub cone_axis: [f32; 3],
    pub cone_cutoff: f32,
    pub first_index: u32,
    pub tri_count: u32,
    pub base_vertex: i32,
    pub pad: u32,
}

const _: () = assert!(std::mem::size_of::<Cluster>() == 48);

/// Output of [`build`].
#[derive(Clone, Debug, PartialEq)]
pub struct ClusteredMesh {
    pub clusters: Vec<Cluster>,
    /// Triangle indices, local to each cluster's `base_vertex`, cluster-contiguous
    /// (cluster `i` owns `indices[first_index .. first_index + 3 * tri_count]`).
    pub indices: Vec<u16>,
    /// Vertex positions, cluster-contiguous. Vertices shared by two clusters are duplicated.
    pub vertices: Vec<[f32; 3]>,
}

/// Splits a triangle mesh into clusters.
///
/// `indices` is a triangle list (length must be a multiple of 3). Every triangle is kept,
/// including degenerate ones, and appears in exactly one cluster.
///
/// The normal cone is built from unit face normals. Degenerate triangles are skipped for the
/// cone. If the cone is wider than a hemisphere (or there are no non-degenerate triangles),
/// the cluster is marked never-cullable (`cone_cutoff = 1.0`, zero axis).
///
/// # Panics
/// If `indices.len()` is not a multiple of 3, or an index is out of range for `positions`.
pub fn build(positions: &[[f32; 3]], indices: &[u32]) -> ClusteredMesh {
    assert!(indices.len().is_multiple_of(3),"index count must be a multiple of 3, got {}", indices.len());
    let mut out = ClusteredMesh { clusters: Vec::new(), indices: Vec::new(), vertices: Vec::new() };
    if indices.is_empty() {
        return out;
    }
    let tris: Vec<[u32; 3]> = indices.chunks_exact(3).map(|t| [t[0], t[1], t[2]]).collect();

    // Mesh bounds, used to quantise centroids for the Morton code.
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for t in &tris {
        for &v in t {
            let p = pos64(positions, v);
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
    }
    let ext = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];

    // Sort triangles by (morton, triangle index). The index breaks ties, so the key is total.
    let mut keys: Vec<(u64, u32)> = tris
        .iter()
        .enumerate()
        .map(|(ti, t)| {
            let mut c = [0.0f64; 3];
            for &v in t {
                let p = pos64(positions, v);
                for k in 0..3 {
                    c[k] += p[k] / 3.0;
                }
            }
            let q = [0, 1, 2].map(|k| quantize(c[k], lo[k], ext[k]));
            (morton(q), ti as u32)
        })
        .collect();
    keys.sort_unstable();

    let mut packer = Packer::default();
    for &(_, ti) in &keys {
        let tri = tris[ti as usize];
        if packer.tris.len() + 1 > MAX_TRIS || packer.vert_count_with(tri) > MAX_VERTS {
            packer.flush(positions, &mut out);
        }
        packer.push(tri);
    }
    packer.flush(positions, &mut out);
    out
}

/// Tests one cluster against the frustum and the backface cone.
///
/// `planes` use the [`crate::cull`] convention (inward, `dot(n, p) + d >= 0` is inside).
/// `cam` is the camera position in the same space as the cluster.
///
/// The cone test is skipped when the camera is inside the bounding sphere, because the
/// direction is then undefined and the cluster may be partly in front of the camera.
pub fn cluster_visible(c: &Cluster, planes: &[[f32; 4]; 6], cam: [f32; 3]) -> bool {
    if !sphere_visible(planes, c.center, c.radius) {
        return false;
    }
    if c.cone_cutoff >= 1.0 {
        return true;
    }
    let v = [c.center[0] - cam[0], c.center[1] - cam[1], c.center[2] - cam[2]];
    let len2 = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
    if len2 <= c.radius * c.radius {
        return true;
    }
    let dot = v[0] * c.cone_axis[0] + v[1] * c.cone_axis[1] + v[2] * c.cone_axis[2];
    dot / len2.sqrt() < c.cone_cutoff
}

/// Greedy packer: accumulates triangles for the current cluster, flushing when a limit would
/// be exceeded.
#[derive(Default)]
struct Packer {
    /// Global vertex index -> cluster-local index. Ordered map, so no hash-order dependence.
    map: BTreeMap<u32, u16>,
    /// Cluster-local index -> global vertex index, in first-use order.
    verts: Vec<u32>,
    /// Triangles as global indices (for the cone) and as local indices (for the output).
    tris: Vec<[u32; 3]>,
    local: Vec<[u16; 3]>,
}

impl Packer {
    /// Vertex count of the current cluster if `tri` were added.
    fn vert_count_with(&self, tri: [u32; 3]) -> usize {
        let new = (0..3)
            .filter(|&k| !self.map.contains_key(&tri[k]) && !tri[..k].contains(&tri[k]))
            .count();
        self.verts.len() + new
    }

    fn push(&mut self, tri: [u32; 3]) {
        let mut l = [0u16; 3];
        for k in 0..3 {
            let v = tri[k];
            let verts = &mut self.verts;
            l[k] = *self.map.entry(v).or_insert_with(|| {
                verts.push(v);
                (verts.len() - 1) as u16
            });
        }
        self.tris.push(tri);
        self.local.push(l);
    }

    /// Emits the current cluster (if any) into `out` and resets the packer.
    fn flush(&mut self, positions: &[[f32; 3]], out: &mut ClusteredMesh) {
        if self.tris.is_empty() {
            return;
        }
        let pts: Vec<[f32; 3]> = self.verts.iter().map(|&v| positions[v as usize]).collect();
        let (center, radius) = bounding_sphere(&pts);
        let (cone_axis, cone_cutoff) = normal_cone(positions, &self.tris);

        let first_index = out.indices.len() as u32;
        let base_vertex = i32::try_from(out.vertices.len()).expect("vertex count exceeds i32");
        out.vertices.extend_from_slice(&pts);
        for l in &self.local {
            out.indices.extend_from_slice(l);
        }
        out.clusters.push(Cluster {
            center,
            radius,
            cone_axis,
            cone_cutoff,
            first_index,
            tri_count: self.tris.len() as u32,
            base_vertex,
            pad: 0,
        });
        *self = Packer::default();
    }
}

/// Centre of the AABB, and the largest distance from that centre to a point. The radius is
/// rounded up to the next `f32`, so it never excludes a point.
fn bounding_sphere(pts: &[[f32; 3]]) -> ([f32; 3], f32) {
    let mut lo = [f32::INFINITY; 3];
    let mut hi = [f32::NEG_INFINITY; 3];
    for p in pts {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    let center = [0, 1, 2].map(|k| lo[k] * 0.5 + hi[k] * 0.5);
    let c64 = center.map(f64::from);
    let mut max_d2 = 0.0f64;
    for p in pts {
        let d2: f64 = (0..3).map(|k| (f64::from(p[k]) - c64[k]).powi(2)).sum();
        max_d2 = max_d2.max(d2);
    }
    let max_d = max_d2.sqrt();
    let mut r = max_d as f32;
    if f64::from(r) < max_d {
        r = r.next_up();
    }
    (center, r)
}

/// Normal cone of a triangle set: unit axis and `cone_cutoff = sin(half angle)`.
///
/// Why sine and not cosine: the cluster is backfacing when every normal `n` satisfies
/// `dot(n, v) >= 0`, where `v` is the direction from the camera to the cluster. If `v` makes
/// an angle of at most `90 - theta` with the axis, every normal (within `theta` of the axis)
/// satisfies that. The condition is `dot(axis, v) >= sin(theta)`. A cosine cutoff would cull
/// too eagerly for `theta > 45 degrees`, which is wrong.
///
/// Returns `([0; 3], 1.0)` (never cull) when the cone is degenerate.
fn normal_cone(positions: &[[f32; 3]], tris: &[[u32; 3]]) -> ([f32; 3], f32) {
    const NEVER: ([f32; 3], f32) = ([0.0; 3], 1.0);
    let mut normals: Vec<[f64; 3]> = Vec::with_capacity(tris.len());
    let mut sum = [0.0f64; 3];
    for t in tris {
        let a = pos64(positions, t[0]);
        let b = pos64(positions, t[1]);
        let c = pos64(positions, t[2]);
        let e1 = sub(b, a);
        let e2 = sub(c, a);
        let n = cross(e1, e2);
        let len = dot(n, n).sqrt();
        if len > 0.0 && len.is_finite() {
            let u = [n[0] / len, n[1] / len, n[2] / len];
            normals.push(u);
            for k in 0..3 {
                sum[k] += u[k];
            }
        }
    }
    if normals.is_empty() {
        return NEVER;
    }
    let slen = dot(sum, sum).sqrt();
    if slen < 1e-9 {
        return NEVER;
    }
    let axis = [sum[0] / slen, sum[1] / slen, sum[2] / slen];
    let cos_min = normals.iter().map(|&n| dot(n, axis)).fold(f64::INFINITY, f64::min);
    if cos_min <= 0.0 {
        // Half angle of 90 degrees or more: the cone is wider than a hemisphere.
        return NEVER;
    }
    let sin = (1.0 - cos_min * cos_min).max(0.0).sqrt();
    // Round up so that f32 rounding can only make the cone wider, never narrower.
    let cutoff = (sin as f32).next_up().min(1.0);
    if cutoff >= 1.0 {
        return NEVER;
    }
    ([axis[0] as f32, axis[1] as f32, axis[2] as f32], cutoff)
}

fn pos64(positions: &[[f32; 3]], i: u32) -> [f64; 3] {
    positions[i as usize].map(f64::from)
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/// Maps `p` in `[lo, lo + ext]` to an integer in `0..2^MORTON_BITS`. A zero extent maps to 0.
fn quantize(p: f64, lo: f64, ext: f64) -> u32 {
    if ext <= 0.0 {
        return 0;
    }
    let t = ((p - lo) / ext).clamp(0.0, 1.0);
    (t * f64::from((1u32 << MORTON_BITS) - 1)) as u32
}

/// Spreads the low 21 bits of `v` so that bit `i` moves to bit `3i`.
fn spread_bits(v: u32) -> u64 {
    let mut x = u64::from(v) & ((1u64 << MORTON_BITS) - 1);
    x = (x | (x << 32)) & 0x001f_0000_0000_ffff;
    x = (x | (x << 16)) & 0x001f_0000_ff00_00ff;
    x = (x | (x << 8)) & 0x100f_00f0_0f00_f00f;
    x = (x | (x << 4)) & 0x10c3_0c30_c30c_30c3;
    x = (x | (x << 2)) & 0x1249_2492_4924_9249;
    x
}

/// Interleaves three 21-bit coordinates into a 63-bit Morton code (x in bit 0 of each triple).
fn morton(q: [u32; 3]) -> u64 {
    spread_bits(q[0]) | (spread_bits(q[1]) << 1) | (spread_bits(q[2]) << 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Flat `n x n` quad grid in the xz plane (y = 0), counter-clockwise from +y, quads of
    /// size 1. Returns `(positions, indices)`.
    fn grid(n: u32) -> (Vec<[f32; 3]>, Vec<u32>) {
        let stride = n + 1;
        let mut pos = Vec::new();
        for z in 0..=n {
            for x in 0..=n {
                pos.push([x as f32, 0.0, z as f32]);
            }
        }
        let id = |x: u32, z: u32| z * stride + x;
        let mut idx = Vec::new();
        for z in 0..n {
            for x in 0..n {
                let (v00, v10, v01, v11) = (id(x, z), id(x + 1, z), id(x, z + 1), id(x + 1, z + 1));
                idx.extend_from_slice(&[v00, v01, v11, v00, v11, v10]);
            }
        }
        (pos, idx)
    }

    /// Deterministic pseudo-random triangle soup (LCG), with shared and degenerate triangles.
    fn soup(verts: u32, tris: u32) -> (Vec<[f32; 3]>, Vec<u32>) {
        let mut s = 0x2545_f491_u64;
        let mut next = move || {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (s >> 33) as u32
        };
        let pos: Vec<[f32; 3]> = (0..verts)
            .map(|_| {
                [
                    (next() % 1000) as f32 * 0.01,
                    (next() % 1000) as f32 * 0.01,
                    (next() % 1000) as f32 * 0.01,
                ]
            })
            .collect();
        let idx: Vec<u32> = (0..tris * 3).map(|_| next() % verts).collect();
        (pos, idx)
    }

    fn canon(p: [[f32; 3]; 3]) -> [[u32; 3]; 3] {
        let mut t = p.map(|v| v.map(f32::to_bits));
        t.sort();
        t
    }

    /// Triangle position sets from the cluster output, in cluster order.
    fn output_tris(m: &ClusteredMesh) -> Vec<[[u32; 3]; 3]> {
        let mut out = Vec::new();
        for c in &m.clusters {
            let first = c.first_index as usize;
            for t in 0..c.tri_count as usize {
                let tri = &m.indices[first + 3 * t..first + 3 * t + 3];
                let base = c.base_vertex as usize;
                let p = [tri[0], tri[1], tri[2]].map(|i| m.vertices[base + i as usize]);
                out.push(canon(p));
            }
        }
        out.sort();
        out
    }

    fn input_tris(pos: &[[f32; 3]], idx: &[u32]) -> Vec<[[u32; 3]; 3]> {
        let mut v: Vec<_> = idx.chunks_exact(3).map(|t| canon([pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]])).collect();
        v.sort();
        v
    }

    /// Number of vertices a cluster uses (local indices are contiguous from 0).
    fn vert_count(m: &ClusteredMesh, c: &Cluster) -> usize {
        let first = c.first_index as usize;
        let n = 3 * c.tri_count as usize;
        m.indices[first..first + n].iter().map(|&i| i as usize + 1).max().unwrap_or(0)
    }

    fn all_planes() -> [[f32; 4]; 6] {
        [[0.0, 0.0, 0.0, 1.0]; 6]
    }

    #[test]
    fn cluster_layout() {
        assert_eq!(std::mem::size_of::<Cluster>(), 48);
        assert_eq!(std::mem::align_of::<Cluster>(), 4);
    }

    #[test]
    fn morton_bits_interleave() {
        assert_eq!(morton([1, 0, 0]), 1);
        assert_eq!(morton([0, 1, 0]), 2);
        assert_eq!(morton([0, 0, 1]), 4);
        assert_eq!(morton([2, 0, 0]), 8);
        assert_eq!(morton([0, 0, 0]), 0);
    }

    #[test]
    fn empty_input() {
        let m = build(&[], &[]);
        assert!(m.clusters.is_empty() && m.indices.is_empty() && m.vertices.is_empty());
    }

    #[test]
    fn every_triangle_exactly_once() {
        for (pos, idx) in [grid(100), soup(1000, 5000)] {
            let m = build(&pos, &idx);
            assert_eq!(output_tris(&m), input_tris(&pos, &idx));
            let total: u32 = m.clusters.iter().map(|c| c.tri_count).sum();
            assert_eq!(total as usize, idx.len() / 3);
            assert_eq!(m.indices.len(), idx.len());
        }
    }

    #[test]
    fn limits_respected() {
        let (pos, idx) = soup(1000, 5000);
        let m = build(&pos, &idx);
        for c in &m.clusters {
            assert!(c.tri_count as usize <= MAX_TRIS);
            let nv = vert_count(&m, c);
            assert!(nv <= MAX_VERTS, "cluster uses {nv} vertices");
            assert!(c.base_vertex as usize + nv <= m.vertices.len());
        }
    }

    #[test]
    fn build_is_deterministic() {
        for (pos, idx) in [grid(37), soup(500, 3000)] {
            assert_eq!(build(&pos, &idx), build(&pos, &idx));
        }
    }

    #[test]
    fn bounding_sphere_contains_vertices() {
        let (pos, idx) = soup(1000, 5000);
        let m = build(&pos, &idx);
        for c in &m.clusters {
            let nv = vert_count(&m, c);
            let base = c.base_vertex as usize;
            let center = c.center.map(f64::from);
            for p in &m.vertices[base..base + nv] {
                let d: f64 = (0..3).map(|k| (f64::from(p[k]) - center[k]).powi(2)).sum();
                assert!(d.sqrt() <= f64::from(c.radius), "vertex outside sphere");
            }
        }
    }

    #[test]
    fn cone_culls_back_side_of_flat_patch() {
        // One cluster: 2x2 quads, normal +y.
        let (pos, idx) = grid(2);
        let m = build(&pos, &idx);
        assert_eq!(m.clusters.len(), 1);
        let c = m.clusters[0];
        assert!((c.cone_axis[1] - 1.0).abs() < 1e-6, "axis {:?}", c.cone_axis);
        assert!(c.cone_cutoff < 1e-3, "flat patch cutoff {}", c.cone_cutoff);

        let planes = all_planes();
        let below = [1.0, -10.0, 1.0];
        let above = [1.0, 10.0, 1.0];
        assert!(!cluster_visible(&c, &planes, below), "camera below should cull");
        assert!(cluster_visible(&c, &planes, above), "camera above should keep");
    }

    #[test]
    fn frustum_still_culls() {
        let (pos, idx) = grid(2);
        let c = build(&pos, &idx).clusters[0];
        // Plane accepts only x <= -10. The cluster centre is at x = 1 with radius ~1.41, so it is culled.
        let mut planes = all_planes();
        planes[0] = [-1.0, 0.0, 0.0, -10.0];
        assert!(!cluster_visible(&c, &planes, [1.0, 10.0, 1.0]));
        // Same plane with the cluster moved inside: kept.
        planes[0] = [-1.0, 0.0, 0.0, 0.0];
        assert!(cluster_visible(&c, &planes, [1.0, 10.0, 1.0]));
    }

    #[test]
    fn never_cull_when_cone_degenerate() {
        // Two opposite-facing triangles share one cluster: the cone is wider than a hemisphere.
        let pos = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]];
        let idx = [0, 2, 1, 0, 1, 2];
        let m = build(&pos, &idx);
        let c = m.clusters[0];
        assert_eq!(c.cone_cutoff, 1.0);
        assert_eq!(c.cone_axis, [0.0; 3]);
        assert!(cluster_visible(&c, &all_planes(), [0.5, -10.0, 0.5]));
        assert!(cluster_visible(&c, &all_planes(), [0.5, 10.0, 0.5]));
    }

    #[test]
    fn grid_cluster_count() {
        let (pos, idx) = grid(100);
        let tris = idx.len() / 3;
        let m = build(&pos, &idx);
        let avg = tris as f64 / m.clusters.len() as f64;
        println!("grid 100x100: {} tris, {} clusters, avg {:.1} tris/cluster", tris, m.clusters.len(), avg);
        let ideal = tris as f64 / MAX_TRIS as f64;
        assert!((m.clusters.len() as f64) < 1.5 * ideal, "{} clusters vs ideal {ideal:.1}", m.clusters.len());
    }
}
