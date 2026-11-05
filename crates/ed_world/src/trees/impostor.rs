//! Hemi-octahedral tree impostors: the bottom of the tree LOD chain.
//!
//! Every species variant is rendered once, on the CPU, from [`FRAMES`] x
//! [`FRAMES`] directions over the upper hemisphere into an atlas layer
//! (albedo + tree-local normal). At runtime each tree far away is one
//! camera-facing quad that shows the frame nearest to the view direction,
//! so a forest costs 2 triangles per tree instead of hundreds.
//!
//! View direction <-> frame mapping is the hemi-octahedral one: a direction
//! `d` (tree-local, `d.y >= 0`, pointing from the tree to the camera) maps
//! to `uv` in the unit square with [`oct_encode`]; frame `(i, j)` is the
//! direction at its centre ([`frame_dir`]). The shader repeats both, so keep
//! `impostor.wesl` in sync.
//!
//! Per-tile instance data comes from [`build_mesh`]: one quad per tree,
//! with everything the vertex shader needs to place, thin and fade it.

use super::textures::Rgba8Image;
use super::{BarkStyle, TreeModel, VARIANTS, WindMesh, instance_rotation, model, variant_for};
use crate::foliage::{TreeInstance, TreeKind};
use crate::mesh::MeshData;
use glam::{Vec2, Vec3};
use std::sync::OnceLock;

/// View grid per side (FRAMES x FRAMES views).
pub const FRAMES: u32 = 8;
/// Pixels per frame side at mip 0.
pub const FRAME_PX: u32 = 32;
/// Atlas layer side.
pub const ATLAS_PX: u32 = FRAMES * FRAME_PX;
/// Mip levels kept (frame 32 px down to 1 px).
pub const MIP_LEVELS: u32 = 6;
/// Fine tiles show merged leaf-card meshes per chunk up to this distance
/// (chunk centre to camera); impostors cover everything beyond.
pub const CARD_RANGE_M: f32 = 800.0;
/// Supersampling per axis while baking.
const SS: u32 = 4;
/// Frame side in model radii: the bounding sphere plus a 1 px margin.
const PAD: f32 = FRAME_PX as f32 / (FRAME_PX as f32 - 2.0);

/// Atlas layer of a species variant.
pub fn layer(kind: TreeKind, variant: u32) -> u32 {
    kind as u32 * VARIANTS + variant % VARIANTS
}

pub fn layer_count() -> u32 {
    TreeKind::ALL.len() as u32 * VARIANTS
}

/// Model-space bounding sphere of a species variant (full model).
#[derive(Debug, Clone, Copy)]
pub struct Bounds {
    pub center: Vec3,
    pub radius: f32,
}

pub fn bounds(kind: TreeKind, variant: u32) -> Bounds {
    static B: OnceLock<Vec<Bounds>> = OnceLock::new();
    let all = B.get_or_init(|| {
        TreeKind::ALL.iter().flat_map(|&k| (0..VARIANTS).map(move |v| model_bounds(model(k, v)))).collect()
    });
    all[layer(kind, variant) as usize]
}

fn model_bounds(m: &TreeModel) -> Bounds {
    let pts = || m.bark.mesh.positions.iter().chain(m.leaves.mesh.positions.iter()).map(|p| Vec3::from(*p));
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for p in pts() {
        lo = lo.min(p);
        hi = hi.max(p);
    }
    if lo.x > hi.x {
        return Bounds { center: Vec3::Y, radius: 1.0 };
    }
    let center = (lo + hi) * 0.5;
    let radius = pts().map(|p| p.distance(center)).fold(0.1f32, f32::max);
    Bounds { center, radius }
}

/// Direction (upper hemisphere) -> atlas uv in the unit square.
pub fn oct_encode(d: Vec3) -> Vec2 {
    let d = Vec3::new(d.x, d.y.max(0.0), d.z);
    let d = d / (d.x.abs() + d.y.abs() + d.z.abs()).max(1e-9);
    Vec2::new(d.x + d.z, d.x - d.z) * 0.5 + 0.5
}

/// Inverse of [`oct_encode`].
pub fn oct_decode(uv: Vec2) -> Vec3 {
    let g = uv * 2.0 - 1.0;
    let x = (g.x + g.y) * 0.5;
    let z = (g.x - g.y) * 0.5;
    Vec3::new(x, 1.0 - x.abs() - z.abs(), z).normalize()
}

/// View direction of frame `(i, j)` (column, row).
pub fn frame_dir(i: u32, j: u32) -> Vec3 {
    oct_decode(Vec2::new((i as f32 + 0.5) / FRAMES as f32, (j as f32 + 0.5) / FRAMES as f32))
}

/// Billboard axes for a view direction `v` (tree to camera): (right, up).
/// The shader builds the same basis from the live view direction.
pub fn view_basis(v: Vec3) -> (Vec3, Vec3) {
    let u0 = Vec3::Y - v * v.y;
    let up = if u0.length_squared() > 1e-6 { u0.normalize() } else { Vec3::Z };
    (up.cross(v).normalize(), up)
}

/// Baked atlas: `layer_count()` layers of `ATLAS_PX`^2 with [`MIP_LEVELS`]
/// mips each, layer-major (layer 0 all mips, then layer 1, ...), which is
/// wgpu's default upload order.
pub struct ImpostorAtlas {
    pub size: u32,
    pub layers: u32,
    pub mip_levels: u32,
    /// sRGB albedo, alpha = coverage (alpha-tested at 0.5).
    pub albedo: Vec<u8>,
    /// Tree-local normal * 0.5 + 0.5 (linear), alpha unused.
    pub normal: Vec<u8>,
}

/// Source textures the tree materials use (see `textures`).
pub struct BakeSources<'a> {
    pub leaves: &'a Rgba8Image,
    pub bark: &'a Rgba8Image,
    pub palm_bark: &'a Rgba8Image,
}

/// Bake every species variant, spread over the machine's cores.
pub fn bake(src: &BakeSources) -> ImpostorAtlas {
    let n = layer_count() as usize;
    let threads = std::thread::available_parallelism().map_or(4, |c| c.get()).min(n);
    let mut layers: Vec<Option<(Vec<u8>, Vec<u8>)>> = (0..n).map(|_| None).collect();
    std::thread::scope(|s| {
        let chunk = n.div_ceil(threads);
        for (ci, slot) in layers.chunks_mut(chunk).enumerate() {
            s.spawn(move || {
                for (k, out) in slot.iter_mut().enumerate() {
                    let l = ci * chunk + k;
                    let kind = TreeKind::ALL[l / VARIANTS as usize];
                    *out = Some(bake_layer(model(kind, l as u32 % VARIANTS), src));
                }
            });
        }
    });
    let (mut albedo, mut normal) = (Vec::new(), Vec::new());
    for (a, nm) in layers.into_iter().flatten() {
        albedo.extend(a);
        normal.extend(nm);
    }
    ImpostorAtlas { size: ATLAS_PX, layers: n as u32, mip_levels: MIP_LEVELS, albedo, normal }
}

// ------------------------------------------------------------ texturing --

struct Tex<'a> {
    img: &'a Rgba8Image,
    offsets: Vec<usize>,
    wrap: bool,
}

impl<'a> Tex<'a> {
    fn new(img: &'a Rgba8Image, wrap: bool) -> Self {
        let mut offsets = Vec::new();
        let (mut w, mut h, mut o) = (img.width as usize, img.height as usize, 0usize);
        for _ in 0..img.mip_levels.max(1) {
            offsets.push(o);
            o += w * h * 4;
            w = (w / 2).max(1);
            h = (h / 2).max(1);
        }
        Self { img, offsets, wrap }
    }

    /// Nearest texel at `uv` on mip `lod`: linear rgb + alpha.
    fn sample(&self, uv: [f32; 2], lod: f32) -> [f32; 4] {
        let l = (lod.max(0.0) as usize).min(self.offsets.len() - 1);
        let (w, h) = ((self.img.width as usize >> l).max(1), (self.img.height as usize >> l).max(1));
        let (mut u, mut v) = (uv[0], uv[1]);
        if self.wrap {
            u = u.rem_euclid(1.0);
            v = v.rem_euclid(1.0);
        }
        let x = ((u * w as f32) as isize).clamp(0, w as isize - 1) as usize;
        let y = ((v * h as f32) as isize).clamp(0, h as isize - 1) as usize;
        let i = self.offsets[l] + (y * w + x) * 4;
        let d = &self.img.data;
        let lin = |b: u8| {
            let v = b as f32 / 255.0;
            if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        [lin(d[i]), lin(d[i + 1]), lin(d[i + 2]), d[i + 3] as f32 / 255.0]
    }

    fn texels(&self) -> f32 {
        (self.img.width * self.img.height) as f32
    }
}

// -------------------------------------------------------------- raster --

#[derive(Clone, Copy, Default)]
struct Px {
    depth: f32,
    color: [f32; 3],
    normal: [f32; 3],
    hit: bool,
}

/// One supersampled frame.
struct Frame {
    side: usize,
    px: Vec<Px>,
}

impl Frame {
    fn new(side: usize) -> Self {
        Self { side, px: vec![Px { depth: f32::MIN, ..Default::default() }; side * side] }
    }

    /// Rasterize `mesh` seen from `v` (orthographic, bounding sphere fills the frame).
    fn draw(&mut self, mesh: &WindMesh, tex: &Tex, alpha_test: bool, b: Bounds, v: Vec3) {
        let (right, up) = view_basis(v);
        let side = self.side as f32;
        let scale = side * 0.5 / (b.radius * PAD);
        let m: &MeshData = &mesh.mesh;
        let proj = |i: u32| {
            let p = Vec3::from(m.positions[i as usize]) - b.center;
            Vec3::new(side * 0.5 + p.dot(right) * scale, side * 0.5 - p.dot(up) * scale, p.dot(v))
        };
        for t in m.indices.chunks_exact(3) {
            let (a, bb, c) = (proj(t[0]), proj(t[1]), proj(t[2]));
            let area = (bb.x - a.x) * (c.y - a.y) - (bb.y - a.y) * (c.x - a.x);
            if area.abs() < 1e-6 {
                continue;
            }
            let uv = [m.uvs[t[0] as usize], m.uvs[t[1] as usize], m.uvs[t[2] as usize]];
            let uv_area = ((uv[1][0] - uv[0][0]) * (uv[2][1] - uv[0][1]) - (uv[1][1] - uv[0][1]) * (uv[2][0] - uv[0][0])).abs() * tex.texels();
            let lod = 0.5 * (uv_area / area.abs()).max(1.0).log2();
            let x0 = a.x.min(bb.x).min(c.x).floor().max(0.0) as usize;
            let x1 = (a.x.max(bb.x).max(c.x).ceil() as usize).min(self.side);
            let y0 = a.y.min(bb.y).min(c.y).floor().max(0.0) as usize;
            let y1 = (a.y.max(bb.y).max(c.y).ceil() as usize).min(self.side);
            for y in y0..y1 {
                for x in x0..x1 {
                    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                    let w0 = ((bb.x - px) * (c.y - py) - (bb.y - py) * (c.x - px)) / area;
                    let w1 = ((c.x - px) * (a.y - py) - (c.y - py) * (a.x - px)) / area;
                    let w2 = 1.0 - w0 - w1;
                    if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                        continue;
                    }
                    let depth = a.z * w0 + bb.z * w1 + c.z * w2;
                    let dst = &mut self.px[y * self.side + x];
                    if depth <= dst.depth {
                        continue;
                    }
                    let mix2 = |q: [[f32; 2]; 3]| [q[0][0] * w0 + q[1][0] * w1 + q[2][0] * w2, q[0][1] * w0 + q[1][1] * w1 + q[2][1] * w2];
                    let s = tex.sample(mix2(uv), lod);
                    if alpha_test && s[3] < 0.5 {
                        continue;
                    }
                    let attr3 = |q: &Vec<[f32; 3]>| Vec3::from(q[t[0] as usize]) * w0 + Vec3::from(q[t[1] as usize]) * w1 + Vec3::from(q[t[2] as usize]) * w2;
                    let attr4 = |k: usize| m.colors[t[0] as usize][k] * w0 + m.colors[t[1] as usize][k] * w1 + m.colors[t[2] as usize][k] * w2;
                    *dst = Px {
                        depth,
                        color: [s[0] * attr4(0), s[1] * attr4(1), s[2] * attr4(2)],
                        normal: attr3(&m.normals).normalize_or(Vec3::Y).to_array(),
                        hit: true,
                    };
                }
            }
        }
    }
}

/// Linear float pixel of one atlas level: rgb, normal, coverage.
#[derive(Clone, Copy, Default)]
struct Texel {
    color: [f32; 3],
    normal: [f32; 3],
    a: f32,
}

fn bake_layer(m: &TreeModel, src: &BakeSources) -> (Vec<u8>, Vec<u8>) {
    let b = model_bounds(m);
    let leaves = Tex::new(src.leaves, false);
    let bark = Tex::new(if m.bark_style == BarkStyle::Palm { src.palm_bark } else { src.bark }, true);
    let side = ATLAS_PX as usize;
    let fp = FRAME_PX as usize;
    let mut level0 = vec![Texel::default(); side * side];
    for j in 0..FRAMES {
        for i in 0..FRAMES {
            let v = frame_dir(i, j);
            let mut f = Frame::new(fp * SS as usize);
            f.draw(&m.bark, &bark, false, b, v);
            f.draw(&m.leaves, &leaves, true, b, v);
            // Box-filter the supersampled frame down: colour and normal
            // averaged over covered samples, alpha = coverage.
            for y in 0..fp {
                for x in 0..fp {
                    let mut t = Texel::default();
                    let mut hits = 0.0;
                    for sy in 0..SS as usize {
                        for sx in 0..SS as usize {
                            let p = f.px[(y * SS as usize + sy) * f.side + x * SS as usize + sx];
                            if p.hit {
                                for k in 0..3 {
                                    t.color[k] += p.color[k];
                                    t.normal[k] += p.normal[k];
                                }
                                hits += 1.0;
                            }
                        }
                    }
                    if hits > 0.0 {
                        t.color = t.color.map(|c| c / hits);
                        t.normal = Vec3::from(t.normal).normalize_or(Vec3::Y).to_array();
                    }
                    t.a = hits / (SS * SS) as f32;
                    level0[(j as usize * fp + y) * side + i as usize * fp + x] = t;
                }
            }
        }
    }
    dilate(&mut level0, side, fp);
    // Leaves are alpha tested at 0.5 at runtime: lift partial coverage so a
    // sparse crown stays a crown, then keep that coverage down the mips.
    for t in &mut level0 {
        t.a = (t.a * 1.6).min(1.0);
    }
    let mut levels = vec![level0];
    for l in 1..MIP_LEVELS as usize {
        let (ps, prev) = (side >> (l - 1), levels.last().unwrap());
        let ns = ps / 2;
        let mut next = vec![Texel::default(); ns * ns];
        for y in 0..ns {
            for x in 0..ns {
                let mut t = Texel::default();
                let mut wsum = 0.0;
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let p = prev[(y * 2 + dy) * ps + x * 2 + dx];
                    let w = p.a + 1e-4;
                    for k in 0..3 {
                        t.color[k] += p.color[k] * w;
                        t.normal[k] += p.normal[k] * w;
                    }
                    wsum += w;
                    t.a += p.a * 0.25;
                }
                t.color = t.color.map(|c| c / wsum);
                t.normal = Vec3::from(t.normal).normalize_or(Vec3::Y).to_array();
                next[y * ns + x] = t;
            }
        }
        keep_coverage(&levels[0], side, &mut next, ns);
        levels.push(next);
    }
    let (mut albedo, mut normal) = (Vec::new(), Vec::new());
    let srgb = |v: f32| {
        let v = v.clamp(0.0, 1.0);
        let s = if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 };
        (s * 255.0 + 0.5) as u8
    };
    let unorm = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    for lv in &levels {
        for t in lv {
            albedo.extend([srgb(t.color[0]), srgb(t.color[1]), srgb(t.color[2]), unorm(t.a)]);
            normal.extend([unorm(t.normal[0] * 0.5 + 0.5), unorm(t.normal[1] * 0.5 + 0.5), unorm(t.normal[2] * 0.5 + 0.5), 255]);
        }
    }
    (albedo, normal)
}

/// Spread colour and normal into empty texels (per frame) so bilinear
/// filtering and mips at crown edges don't pull in black.
fn dilate(px: &mut [Texel], side: usize, frame: usize) {
    for _ in 0..4 {
        let src = px.to_vec();
        for y in 0..side {
            for x in 0..side {
                if src[y * side + x].a > 0.0 {
                    continue;
                }
                let (fx, fy) = (x / frame * frame, y / frame * frame);
                let (mut c, mut n, mut k) = ([0.0f32; 3], [0.0f32; 3], 0.0);
                for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    if nx < fx as i32 || ny < fy as i32 || nx >= (fx + frame) as i32 || ny >= (fy + frame) as i32 {
                        continue;
                    }
                    let q = src[ny as usize * side + nx as usize];
                    if q.a > 0.0 || q.color != [0.0; 3] {
                        for i in 0..3 {
                            c[i] += q.color[i];
                            n[i] += q.normal[i];
                        }
                        k += 1.0;
                    }
                }
                if k > 0.0 && px[y * side + x].color == [0.0; 3] {
                    px[y * side + x].color = c.map(|v| v / k);
                    px[y * side + x].normal = Vec3::from(n).normalize_or(Vec3::Y).to_array();
                }
            }
        }
    }
}

/// Rescale alpha per frame so the share of texels above the 0.5 cutoff
/// matches level 0 (crowns don't thin out with distance).
fn keep_coverage(level0: &[Texel], side0: usize, next: &mut [Texel], side: usize) {
    let f0 = side0 / FRAMES as usize;
    let f = side / FRAMES as usize;
    if f < 2 {
        return;
    }
    let cov = |px: &[Texel], s: usize, fs: usize, i: usize, j: usize, k: f32| {
        let mut c = 0;
        for y in j * fs..(j + 1) * fs {
            for x in i * fs..(i + 1) * fs {
                if px[y * s + x].a * k > 0.5 {
                    c += 1;
                }
            }
        }
        c as f32 / (fs * fs) as f32
    };
    for j in 0..FRAMES as usize {
        for i in 0..FRAMES as usize {
            let target = cov(level0, side0, f0, i, j, 1.0);
            if target <= 0.0 || target >= 1.0 {
                continue;
            }
            let (mut lo, mut hi) = (0.5f32, 4.0f32);
            for _ in 0..10 {
                let mid = (lo + hi) * 0.5;
                if cov(next, side, f, i, j, mid) < target { lo = mid } else { hi = mid }
            }
            let k = (lo + hi) * 0.5;
            for y in j * f..(j + 1) * f {
                for x in i * f..(i + 1) * f {
                    next[y * side + x].a = (next[y * side + x].a * k).min(1.0);
                }
            }
        }
    }
}

// ---------------------------------------------------------- tile mesh --

/// One quad per tree for the impostor material. Vertex layout (all four
/// corners of a quad share everything but `corner`):
/// - `positions`: bounding-sphere centre (tile space).
/// - `corners`: quad corner in -1..1.
/// - `params`: (sphere radius m, atlas layer, share of the real trees this
///   tile kept, near cut-off m).
/// - `axes`: tree-local +X in tile space (xyz), w = thinning rank 0..1.
/// - `normals`: tree up (tile space).
/// - `colors`: the tree's tint (satellite canopy colour, alpha = weight).
#[derive(Debug, Clone, Default)]
pub struct ImpostorMesh {
    pub positions: Vec<[f32; 3]>,
    pub corners: Vec<[f32; 2]>,
    pub params: Vec<[f32; 4]>,
    pub axes: Vec<[f32; 4]>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
    /// Tile (u, v) of the tree's base, for per-tile lookups (far shadows).
    pub tile_uv: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}

impl ImpostorMesh {
    pub fn len(&self) -> usize {
        self.positions.len() / 4
    }
    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }
}

/// Impostor quads for a tile's trees, positions relative to `origin`.
///
/// `kept` is the share of the real trees the tile's scatter kept (1 on fine
/// tiles; less on coarse tiles, which the shader compensates for).
/// `near_cut` hides impostors closer than this (where full trees or leaf
/// cards take over); 0 shows them at any distance.
pub fn build_mesh(trees: &[TreeInstance], up: Vec3, origin: Vec3, kept: f32, near_cut: f32, uv: Option<&crate::project::UvMap>) -> ImpostorMesh {
    let mut out = ImpostorMesh::default();
    for t in trees {
        let var = variant_for(t);
        let b = bounds(t.kind, var);
        let rot = instance_rotation(t, up);
        let c = t.pos - origin + rot * (b.center * t.scale);
        let ax = rot * Vec3::X;
        let rank = crate::hash01(t.pos.x.to_bits() as u64 ^ 0x1A7E, t.pos.z.to_bits() as u64);
        let base = out.positions.len() as u32;
        let tuv = uv.map_or([0.5, 0.5], |m| m.uv(t.pos));
        for corner in [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]] {
            out.positions.push(c.to_array());
            out.corners.push(corner);
            out.params.push([b.radius * t.scale, layer(t.kind, var) as f32, kept, near_cut]);
            out.axes.push([ax.x, ax.y, ax.z, rank]);
            out.normals.push(up.to_array());
            out.colors.push(t.tint);
            out.tile_uv.push(tuv);
        }
        out.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn octahedral_roundtrip() {
        for j in 0..FRAMES {
            for i in 0..FRAMES {
                let d = frame_dir(i, j);
                assert!(d.y >= 0.0);
                let uv = oct_encode(d);
                let (fi, fj) = ((uv.x * FRAMES as f32) as u32, (uv.y * FRAMES as f32) as u32);
                assert_eq!((fi, fj), (i, j), "{d:?}");
            }
        }
        // Straight down and the horizon land inside the grid.
        for d in [Vec3::Y, Vec3::X, -Vec3::Z, Vec3::new(1.0, 0.0, 1.0).normalize()] {
            let uv = oct_encode(d);
            assert!((0.0..=1.0).contains(&uv.x) && (0.0..=1.0).contains(&uv.y), "{d:?} {uv:?}");
            assert!(oct_decode(uv).dot(d) > 0.999, "{d:?}");
        }
    }

    #[test]
    fn basis_is_orthonormal_and_upright() {
        for d in [Vec3::Z, Vec3::new(0.3, 0.5, -0.8).normalize(), Vec3::Y] {
            let (r, u) = view_basis(d);
            assert!(r.dot(u).abs() < 1e-5 && r.dot(d).abs() < 1e-5 && u.dot(d).abs() < 1e-5);
            if d.y < 0.99 {
                assert!(u.y > 0.0);
            }
        }
        let (r, _) = view_basis(Vec3::Z);
        assert!(r.dot(Vec3::X) > 0.999, "camera on +Z sees +X on the right");
    }

    #[test]
    fn bakes_a_tree() {
        // Small sources so the test stays quick.
        let solid = |a: u8| Rgba8Image { width: 4, height: 4, mip_levels: 1, data: [200, 200, 200, a].repeat(16) };
        let (l, bk) = (solid(255), solid(255));
        let src = BakeSources { leaves: &l, bark: &bk, palm_bark: &bk };
        let m = model(TreeKind::Broadleaf, 0);
        let (albedo, normal) = bake_layer(m, &src);
        let mip_bytes: usize = (0..MIP_LEVELS).map(|k| ((ATLAS_PX >> k) as usize).pow(2) * 4).sum();
        assert_eq!(albedo.len(), mip_bytes);
        assert_eq!(normal.len(), mip_bytes);
        // Every frame shows the tree, centred, with empty corners.
        let side = ATLAS_PX as usize;
        let fp = FRAME_PX as usize;
        for j in 0..FRAMES as usize {
            for i in 0..FRAMES as usize {
                let a = |x: usize, y: usize| albedo[((j * fp + y) * side + i * fp + x) * 4 + 3];
                let covered = (0..fp).flat_map(|y| (0..fp).map(move |x| (x, y))).filter(|&(x, y)| a(x, y) > 127).count();
                assert!(covered > fp * fp / 10, "frame {i},{j}: {covered}");
                assert!(a(0, 0) < 128 && a(fp - 1, fp - 1) < 128, "frame {i},{j} corner covered");
                assert!(a(fp / 2, fp / 2) > 127 || covered > fp * fp / 4, "frame {i},{j} empty centre");
            }
        }
        // Coverage survives the mips.
        let off1 = side * side * 4;
        let s1 = side / 2;
        let cov1 = (0..s1 * s1).filter(|k| albedo[off1 + k * 4 + 3] > 127).count() as f32 / (s1 * s1) as f32;
        let cov0 = (0..side * side).filter(|k| albedo[k * 4 + 3] > 127).count() as f32 / (side * side) as f32;
        assert!((cov1 - cov0).abs() < 0.08, "coverage {cov0} -> {cov1}");
    }

    #[test]
    fn mesh_is_one_quad_per_tree() {
        let trees: Vec<TreeInstance> = (0..10)
            .map(|i| TreeInstance { kind: TreeKind::ALL[i % 5], pos: Vec3::new(i as f32 * 10.0, 0.0, 0.0), scale: 1.0, yaw: i as f32, tint: [1.0; 4] })
            .collect();
        let m = build_mesh(&trees, Vec3::Y, Vec3::ZERO, 0.5, 0.0, None);
        assert_eq!(m.len(), 10);
        assert_eq!(m.indices.len(), 60);
        for p in &m.params {
            assert!(p[0] > 0.3 && p[0] < 30.0 && p[1] < layer_count() as f32);
        }
        // Sphere centre sits above the base.
        assert!(m.positions[0][1] > 0.5);
    }
}

/// `EW_IMPOSTOR_DUMP=dir cargo test -p ed_world --release dump_atlas -- --ignored --nocapture`
/// writes each layer's mip 0 (albedo over grey) as PPM for a visual check.
#[cfg(test)]
#[test]
#[ignore]
fn dump_atlas() {
    use super::textures;
    let (l, b, p) = (textures::leaf_atlas(), textures::bark_texture(), textures::palm_bark_texture());
    let t0 = std::time::Instant::now();
    let a = bake(&BakeSources { leaves: &l, bark: &b, palm_bark: &p });
    eprintln!("baked {} layers in {:?}", a.layers, t0.elapsed());
    {
        let per: usize = (0..MIP_LEVELS).map(|k| ((ATLAS_PX >> k) as usize).pow(2) * 4).sum();
        for layer in [0usize, 3, 7] {
            let mut off = layer * per;
            for k in 0..MIP_LEVELS {
                let n = ((ATLAS_PX >> k) as usize).pow(2);
                let (mut sum, mut cnt) = ([0u64; 3], 0u64);
                for px in a.albedo[off..off + n * 4].chunks_exact(4) {
                    if px[3] > 127 { for c in 0..3 { sum[c] += px[c] as u64; } cnt += 1; }
                }
                let m = |c: usize| sum[c] / cnt.max(1);
                eprintln!("layer {layer} mip {k}: opaque {cnt}/{n} mean rgb ({}, {}, {})", m(0), m(1), m(2));
                off += n * 4;
            }
        }
    }
    let Ok(dir) = std::env::var("EW_IMPOSTOR_DUMP") else { return };
    let per: usize = (0..MIP_LEVELS).map(|k| ((ATLAS_PX >> k) as usize).pow(2) * 4).sum();
    let s = ATLAS_PX as usize;
    for layer in 0..a.layers as usize {
        let src = &a.albedo[layer * per..layer * per + s * s * 4];
        let mut out = format!("P6 {s} {s} 255\n").into_bytes();
        for px in src.chunks_exact(4) {
            let k = if px[3] > 127 { 1.0 } else { 0.0 };
            for c in 0..3 {
                out.push((px[c] as f32 * k + 90.0 * (1.0 - k)) as u8);
            }
        }
        std::fs::write(format!("{dir}/layer{layer:02}.ppm"), out).unwrap();
    }
}
