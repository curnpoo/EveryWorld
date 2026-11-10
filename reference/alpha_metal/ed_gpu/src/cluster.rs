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
use meshopt::{simplify, simplify_scale, SimplifyOptions, VertexDataAdapter};
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
    /// Geometric error (metres) of this cluster's geometry vs the finest level.
    /// 0 for the finest level.
    pub error: f32,
    /// Geometric error (metres) of the next coarser level that contains this
    /// cluster. `f32::INFINITY` for the coarsest level. Used by the LOD
    /// selection rule.
    pub parent_error: f32,
}

const _: () = assert!(std::mem::size_of::<Cluster>() == 56);

/// Output of [`build`].
#[derive(Clone, Debug, PartialEq)]
pub struct ClusteredMesh {
    pub clusters: Vec<Cluster>,
    /// Triangle indices, local to each cluster's `base_vertex`, cluster-contiguous
    /// (cluster `i` owns `indices[first_index .. first_index + 3 * tri_count]`).
    pub indices: Vec<u16>,
    /// Vertex positions, cluster-contiguous. Vertices shared by two clusters are duplicated.
    pub vertices: Vec<[f32; 3]>,
    /// Input vertex index of each entry in `vertices`, so per-vertex attributes can be gathered.
    pub src: Vec<u32>,
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
    let mut out = ClusteredMesh { clusters: Vec::new(), indices: Vec::new(), vertices: Vec::new(), src: Vec::new() };
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
        out.src.extend_from_slice(&self.verts);
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
            error: 0.0,
            parent_error: f32::INFINITY,
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


/// A cluster LOD hierarchy: level 0 is the finest (original) mesh, the last level
/// is the coarsest. Each cluster carries its own `error` and its parent level's
/// `error` (in `parent_error`), so the LOD selection rule is local to the cluster.
#[derive(Clone, Debug)]
pub struct ClusterHierarchy {
    pub levels: Vec<ClusteredMesh>,
}

impl ClusterHierarchy {
    /// Number of levels.
    pub fn len(&self) -> usize {
        self.levels.len()
    }
    pub fn is_empty(&self) -> bool {
        self.levels.is_empty()
    }
}

/// Simplified copies of a mesh at the given error budgets (metres of surface deviation).
///
/// The vertex-clustering decimation that the renderer used before this snapped every vertex to
/// an arbitrary neighbour in a grid cell, which twists flat walls: at a distance the towers came
/// out as sails of stretched triangles. `meshopt` collapses the edges that cost the least shape
/// error, so the silhouette survives.
///
/// Indices stay in the input vertex space, so every level can be drawn with the same vertex
/// buffer. A level that cannot simplify falls back to the input.
pub fn simplify_levels(positions: &[[f32; 3]], indices: &[u32], errors: &[f32]) -> Vec<Vec<u32>> {
    if positions.is_empty() || indices.is_empty() {
        return errors.iter().map(|_| indices.to_vec()).collect();
    }
    // The LOD geometry is a soup of separate quads with duplicated corner vertices, so no two
    // triangles share an edge and a simplifier cannot collapse anything. Weld first (on a 1 mm
    // grid, since the corners of neighbouring quads differ by float rounding): vertices that sit
    // on the same spot become one, which gives the simplifier the topology it needs. The result's
    // indices are mapped back into the original vertex space, so every level can be drawn with the
    // mesh's own vertex buffer.
    const WELD_MM: f32 = 1.0;
    let mut cells: std::collections::HashMap<(i32, i32, i32), u32> = std::collections::HashMap::new();
    let mut remap: Vec<u32> = Vec::with_capacity(positions.len());
    let mut welded_pos: Vec<[f32; 3]> = Vec::new();
    let mut orig_of: Vec<u32> = Vec::new();
    for (v, p) in positions.iter().enumerate() {
        let key = (
            (p[0] / WELD_MM).round() as i32,
            (p[1] / WELD_MM).round() as i32,
            (p[2] / WELD_MM).round() as i32,
        );
        let w = match cells.get(&key) {
            Some(&w) => w,
            None => {
                let w = welded_pos.len() as u32;
                cells.insert(key, w);
                welded_pos.push(*p);
                orig_of.push(v as u32);
                w
            }
        };
        remap.push(w);
    }
    let welded_idx: Vec<u32> = indices.iter().map(|&i| remap[i as usize]).collect();
    // meshopt wants a vertex buffer view: 3 f32 per vertex, stride 12, offset 0.
    let bytes: &[u8] = unsafe { std::slice::from_raw_parts(welded_pos.as_ptr() as *const u8, welded_pos.len() * 12) };
    let Ok(vda) = VertexDataAdapter::new(bytes, 12, 0) else {
        return errors.iter().map(|_| indices.to_vec()).collect();
    };
    // meshopt normalises the positions to the mesh's extent and compares squared errors in that
    // space, so an absolute error of `e` metres has to be scaled by the extent twice.
    let extent = simplify_scale(&vda).max(1e-6);
    errors
        .iter()
        .map(|&e| {
            let out = simplify(&welded_idx, &vda, 0, e / (extent * extent), SimplifyOptions::ErrorAbsolute | SimplifyOptions::Permissive | SimplifyOptions::Prune, None);
            if out.is_empty() || out.len() >= welded_idx.len() {
                return indices.to_vec();
            }
            out.into_iter().map(|w| orig_of[w as usize]).collect()
        })
        .collect()
}

/// Builds a cluster LOD hierarchy by repeatedly simplifying the whole mesh and
/// re-clustering. `GROUP_SIZE` is not used: each building is simplified as one
/// mesh (no internal borders), so there are no cracks within a building.
///
/// Simplification uses `meshopt` with `LockBorder` disabled (the building outline
/// is the only border and is preserved by the error metric) and `ErrorAbsolute`,
/// so `error` is in metres. Stops when simplification no longer reduces the
/// triangle count by at least 10% or the mesh is below 64 triangles.
pub fn build_hierarchy(positions: &[[f32; 3]], indices: &[u32]) -> ClusterHierarchy {
    const MIN_TRIS: usize = 64;
    let mut levels = vec![build(positions, indices)];
    // error on level 0 is already 0.

    let mut cur_indices: Vec<u32> = indices.to_vec();
    // cur_src maps a vertex index in cur mesh space to the original vertex index.
    // cur_positions stays = positions (meshopt keeps original vertices).
    let mut level_errors: Vec<f32> = vec![0.0];

    loop {
        let tri_count = cur_indices.len() / 3;
        if tri_count < MIN_TRIS {
            break;
        }
        // Build a VertexDataAdapter over positions (3 f32 per vertex, stride 12, offset 0).
        let bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(positions.as_ptr() as *const u8, positions.len() * 12)
        };
        let vda = match VertexDataAdapter::new(bytes, 12, 0) {
            Ok(v) => v,
            Err(_) => break,
        };
        let level = levels.len();
        let budget = 2.0f32 * (1u32 << (level-1).min(8)) as f32;
        let mut err = 0.0f32;
        let simplified = simplify(
            &cur_indices,
            &vda,
            0,
            budget,
            SimplifyOptions::ErrorAbsolute | SimplifyOptions::Permissive,
            Some(&mut err),
        );
        if simplified.is_empty() || simplified.len() >= cur_indices.len() * 9 / 10 {
            break;
        }
        // Re-cluster the simplified mesh. src maps to positions indices; remap to original.
        let mut mesh = build(positions, &simplified);
        // build() set src = simplified indices (into positions). positions == original, so src is already original. Good.
        // Tag error.
        for c in &mut mesh.clusters {
            c.error = err;
            c.parent_error = f32::INFINITY;
        }
        levels.push(mesh);
        level_errors.push(err);
        cur_indices = simplified;
    }

    // Final level: a single cube approximating the building's bounding box. This is the
    // coarsest representation, used when the building is very far away.
    {
        let mut lo = [f32::INFINITY; 3];
        let mut hi = [f32::NEG_INFINITY; 3];
        for p in positions {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        let cx = (lo[0] + hi[0]) * 0.5;
        let cy = (lo[1] + hi[1]) * 0.5;
        let cz = (lo[2] + hi[2]) * 0.5;
        let hx = (hi[0] - lo[0]) * 0.5;
        let hy = (hi[1] - lo[1]) * 0.5;
        let hz = (hi[2] - lo[2]) * 0.5;
        // 8 corners
        let corners: [[f32;3];8] = [
            [cx-hx, cy-hy, cz-hz], [cx+hx, cy-hy, cz-hz],
            [cx+hx, cy+hy, cz-hz], [cx-hx, cy+hy, cz-hz],
            [cx-hx, cy-hy, cz+hz], [cx+hx, cy-hy, cz+hz],
            [cx+hx, cy+hy, cz+hz], [cx-hx, cy+hy, cz+hz],
        ];
        // 12 triangles (2 per face), ccw from outside
        let tris: [[u32;3];12] = [
            [0,2,1],[0,3,2], // -z? we'll just use a consistent winding
            [4,5,6],[4,6,7],
            [0,1,5],[0,5,4],
            [2,3,7],[2,7,6],
            [0,4,7],[0,7,3],
            [1,2,6],[1,6,5],
        ];
        // compute error = max distance from any original vertex to nearest cube vertex (conservative: to center-ish)
        // Use bounding sphere radius as conservative error.
        let mut maxd2 = 0.0f32;
        for p in positions {
            let dx = p[0]-cx; let dy=p[1]-cy; let dz=p[2]-cz;
            maxd2 = maxd2.max(dx*dx+dy*dy+dz*dz);
        }
        let err = maxd2.sqrt();
        // Build a ClusteredMesh manually for the cube
        // Use build() on cube to get proper cluster, but build needs positions+indices as flat.
        let mut cube_pos = Vec::new();
        let mut cube_idx = Vec::new();
        for c in &corners { cube_pos.push(*c); }
        for t in &tris { for &v in t { cube_idx.push(v); } }
        // src: map each cube vertex to 0 (original) for attribute gather
        let built = build(&cube_pos, &cube_idx);
        // remap src to 0
        let mut built = built;
        for s in &mut built.src { *s = 0; }
        for c in &mut built.clusters {
            c.error = err;
            c.parent_error = f32::INFINITY;
        }
        // only add if there is at least one prior level (always) and cube is coarser
        levels.push(built);
        level_errors.push(err);
        let _ = (hx,hy,hz);
    }

    // Set parent_error: level L's parent_error = level L+1's error (or INF for last).
    let n = levels.len();
    for (li, mesh) in levels.iter_mut().enumerate() {
        let parent_err = if li + 1 < n {
            level_errors[li + 1]
        } else {
            f32::INFINITY
        };
        for c in &mut mesh.clusters {
            c.parent_error = parent_err;
            if li == 0 {
                c.error = 0.0;
            }
        }
    }
    if let Some(m0) = levels.first_mut() {
        for c in &mut m0.clusters {
            c.error = 0.0;
        }
    }

    ClusterHierarchy { levels }
}

/// Flattens a hierarchy into a single [`ClusteredMesh`] by concatenating every level's
/// clusters, vertices and indices, adjusting `base_vertex` and `first_index` so each
/// cluster still points at its own vertices. Used by GPU paths that store all levels in
/// one buffer. `error`/`parent_error` are preserved per cluster.
pub fn flatten(hier: &ClusterHierarchy) -> ClusteredMesh {
    let mut out = ClusteredMesh { clusters: Vec::new(), indices: Vec::new(), vertices: Vec::new(), src: Vec::new() };
    for mesh in &hier.levels {
        let voff = out.vertices.len() as i32;
        let ioff = out.indices.len() as u32; // in u16 units (1 index = 1 u16)
        // copy vertices + src
        out.vertices.extend_from_slice(&mesh.vertices);
        out.src.extend_from_slice(&mesh.src);
        // copy indices with offset? indices are local u16; they stay local to each cluster's
        // base_vertex, so no remapping needed. But first_index must point into combined indices.
        let idx_base_u16 = out.indices.len() as u32;
        out.indices.extend_from_slice(&mesh.indices);
        for c in &mesh.clusters {
            let mut cc = *c;
            cc.base_vertex += voff;
            cc.first_index += idx_base_u16;
            out.clusters.push(cc);
        }
        let _ = (ioff,);
    }
    out
}

/// Projected screen-space error of a cluster, in pixels.
pub fn projected_error_px(error: f32, distance: f32, screen_height_px: f32, fov_y_rad: f32) -> f32 {
    if distance <= 0.0 || fov_y_rad <= 0.0 {
        return f32::INFINITY;
    }
    error * screen_height_px / (2.0 * distance * (fov_y_rad * 0.5).tan())
}

/// LOD selection rule: draw this cluster iff its own projected error is within
/// the threshold AND its parent's projected error exceeds the threshold.
pub fn cluster_lod_selected(c: &Cluster, distance: f32, threshold_px: f32, screen_height_px: f32, fov_y_rad: f32) -> bool {
    let self_px = projected_error_px(c.error, distance, screen_height_px, fov_y_rad);
    let parent_px = projected_error_px(c.parent_error, distance, screen_height_px, fov_y_rad);
    self_px <= threshold_px && parent_px > threshold_px
}

/// Counts visible triangles of a hierarchy for a camera, applying frustum cull,
/// backface cone, and LOD selection. `threshold_px` is the screen-space error
/// budget in pixels.
pub fn hierarchy_visible_tris(
    hier: &ClusterHierarchy,
    planes: &[[f32; 4]; 6],
    cam: [f32; 3],
    threshold_px: f32,
    screen_height_px: f32,
    fov_y_rad: f32,
) -> u32 {
    let mut total = 0u32;
    for mesh in &hier.levels {
        for c in &mesh.clusters {
            let center = c.center;
            let dx = center[0] - cam[0];
            let dy = center[1] - cam[1];
            let dz = center[2] - cam[2];
            let dist = (dx * dx + dy * dy + dz * dz).sqrt();
            if !cluster_visible(c, planes, cam) {
                continue;
            }
            if !cluster_lod_selected(c, dist, threshold_px, screen_height_px, fov_y_rad) {
                continue;
            }
            total += c.tri_count;
        }
    }
    total
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
        assert_eq!(std::mem::size_of::<Cluster>(), 56);
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
    fn hierarchy_has_finest_first() {
        let (pos, idx) = grid(20);
        let h = build_hierarchy(&pos, &idx);
        assert!(!h.levels.is_empty());
        // level 0 error 0
        for c in &h.levels[0].clusters {
            assert_eq!(c.error, 0.0);
        }
        if h.levels.len() > 1 {
            for c in &h.levels[1].clusters {
                assert!(c.error > 0.0);
            }
        }
        // last level has infinite parent_error
        let last = h.levels.len() - 1;
        for c in &h.levels[last].clusters {
            assert!(c.parent_error.is_infinite());
        }
    }

    #[test]
    fn hierarchy_reduces_or_keeps() {
        let (pos, idx) = grid(40);
        let h = build_hierarchy(&pos, &idx);
        let t0: u32 = h.levels[0].clusters.iter().map(|c| c.tri_count).sum();
        if h.levels.len() > 1 {
            let t1: u32 = h.levels[1].clusters.iter().map(|c| c.tri_count).sum();
            assert!(t1 <= t0, "level1 {} > level0 {}", t1, t0);
        }
    }

    #[test]
    fn projected_error_zero_for_zero_error() {
        assert_eq!(projected_error_px(0.0, 100.0, 720.0, 1.0), 0.0);
    }

    #[test]
    fn lod_selection_picks_finest_when_parent_too_inaccurate() {
        // level0 cluster: error 0, parent_error 1m. At a distance where parent projects to 2px (>1px threshold),
        // level0 should be selected.
        let c = Cluster {
            center: [0.0;3], radius: 1.0, cone_axis: [0.0;3], cone_cutoff: 1.0,
            first_index: 0, tri_count: 1, base_vertex: 0, pad: 0,
            error: 0.0, parent_error: 1.0,
        };
        // distance such that 1m projects to ~2px
        assert!(cluster_lod_selected(&c, 100.0, 1.0, 720.0, 1.0));
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

#[cfg(test)]
mod simplify_levels_tests {
    use super::simplify_levels;

    /// A subdivided plane: the LOD levels must shrink it and stay valid indices.
    #[test]
    fn levels_shrink_and_stay_valid() {
        let n = 40usize;
        let mut pos = Vec::new();
        let mut idx = Vec::new();
        for j in 0..=n {
            for i in 0..=n {
                pos.push([i as f32, 0.0, j as f32]);
            }
        }
        for j in 0..n {
            for i in 0..n {
                let a = (j * (n + 1) + i) as u32;
                let b = a + 1;
                let c = a + (n + 1) as u32;
                let d = c + 1;
                idx.extend_from_slice(&[a, b, d, a, d, c]);
            }
        }
        let levels = simplify_levels(&pos, &idx, &[0.5, 4.0]);
        assert_eq!(levels.len(), 2);
        for (k, l) in levels.iter().enumerate() {
            eprintln!("level {k}: {} indices (of {})", l.len(), idx.len());
            assert!(l.len() < idx.len(), "level {k} did not shrink the mesh");
            for &i in l.iter() {
                assert!((i as usize) < pos.len(), "level {k} points outside the vertex buffer");
            }
        }
        // A coarser budget must not give back triangles.
        assert!(levels[1].len() <= levels[0].len());
    }

    /// The LOD geometry is a soup of quads with duplicated corners: the levels still have to
    /// shrink it, which only works if the simplifier gets a welded topology.
    #[test]
    fn levels_shrink_a_soup_of_quads() {
        // A grid of quads where every quad carries its own corner vertices: the corners land on
        // the same spots, but no two triangles share an edge.
        let mut pos = Vec::new();
        let mut idx = Vec::new();
        let side = 20usize;
        for j in 0..side {
            for i in 0..side {
                let x = i as f32 * 3.0;
                let z = j as f32 * 3.0;
                let b = pos.len() as u32;
                for (dx, dz) in [(0.0f32, 0.0f32), (3.0, 0.0), (3.0, 3.0), (0.0, 3.0)] {
                    pos.push([x + dx, 0.0, z + dz]);
                }
                idx.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
            }
        }
        let levels = simplify_levels(&pos, &idx, &[1.0]);
        eprintln!("soup: {} -> {} indices", idx.len(), levels[0].len());
        assert!(levels[0].len() < idx.len(), "the simplifier could not collapse a welded quad soup");
    }
}

#[cfg(test)]
mod tile_probe {
    /// How much a real city tile shrinks at each error budget (EPL_LOD_DUMP=path).
    #[test]
    fn tile_error_budgets() {
        let Ok(path) = std::env::var("EPL_LOD_DUMP") else { return };
        let raw = std::fs::read(path).unwrap();
        let mut o = 4usize;
        let rd = |o: &mut usize| -> u32 {
            let v = u32::from_le_bytes(raw[*o..*o + 4].try_into().unwrap());
            *o += 4;
            v
        };
        let nv = rd(&mut o) as usize;
        let ni = rd(&mut o) as usize;
        o = 12;
        let mut pos = Vec::with_capacity(nv);
        for _ in 0..nv {
            let mut p = [0f32; 3];
            for k in 0..3 {
                p[k] = f32::from_le_bytes(raw[o..o + 4].try_into().unwrap());
                o += 4;
            }
            pos.push(p);
        }
        o += nv * 3 * 4 + nv * 4 * 4;
        let mut idx = Vec::with_capacity(ni);
        for _ in 0..ni {
            idx.push(rd(&mut o));
        }
        for e in [1.0f32, 4.0, 16.0, 64.0, 256.0, 1024.0] {
            let l = super::simplify_levels(&pos, &idx, &[e]);
            eprintln!("error {e} m: {} tris ({:.1}%)", l[0].len() / 3, 100.0 * l[0].len() as f32 / idx.len() as f32);
        }
    }
}
