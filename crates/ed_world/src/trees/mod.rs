//! Procedural trees and shrubs (v2).
//!
//! Every species is grown, not assembled: trunks and branches are tapered
//! tubes swept along curved paths (parallel-transport frames, so they twist
//! and bend smoothly), foliage is alpha-tested leaf-cluster cards and
//! ribbons (palm fronds, ferns, grass, banana leaves) textured from a
//! procedurally painted atlas ([`textures`]).
//!
//! Per-vertex data the renderer uses:
//! - `color`: species tint x baked ambient occlusion (darker toward the crown
//!   interior and the ground) x a warm, lighter rim on the crown surface.
//! - `normal` on leaves is bent toward the crown's outward direction, so a
//!   cloud of cards shades like one soft volume instead of a pile of planes.
//! - `wind` (UV1): `x` packs a per-branch phase and the sway amplitude
//!   (meters of bend at unit wind, growing with height and along branches),
//!   `y` is the leaf flutter amplitude. See [`pack_wind`].
//!
//! Each species has [`VARIANTS`] seeded variations, and two levels of
//! detail: the full model (bark + leaves) and a far model (one mesh in the
//! leaf material, a handful of big cards and a simplified trunk) grown from
//! the same seed so the silhouette matches.

pub mod assembly;
pub mod bank;
pub mod bank_impostor;
pub mod impostor;
pub mod parts;
pub mod species;
pub mod textures;
pub use species::{Genome, SpeciesDef};

use crate::foliage::{TreeInstance, TreeKind};
use crate::mesh::MeshData;
use glam::{Quat, Vec3};
use std::f32::consts::{PI, TAU};
use std::sync::OnceLock;
pub use textures::{LeafCell, atlas_uv};

/// Seeded variations per species.
pub const VARIANTS: u32 = 3;

// ------------------------------------------------------------------ rng --

/// Small deterministic stream on top of [`crate::hash01`].
/// Interior counter so draws work inside `builder.method(builder.rng.r(..))`.
#[derive(Debug, Clone)]
pub(crate) struct Rng {
    seed: u64,
    i: std::cell::Cell<u64>,
}

impl Rng {
    pub(crate) fn new(seed: u64) -> Self {
        Self { seed, i: std::cell::Cell::new(0) }
    }
    pub(crate) fn f(&self) -> f32 {
        self.i.set(self.i.get() + 1);
        crate::hash01(self.seed, self.i.get())
    }
    fn r(&self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f()
    }
    fn unit(&self) -> Vec3 {
        let z = self.r(-1.0, 1.0);
        let a = self.r(0.0, TAU);
        let s = (1.0 - z * z).max(0.0).sqrt();
        Vec3::new(s * a.cos(), z, s * a.sin())
    }
}

// ----------------------------------------------------------- wind mesh --

/// Mesh plus per-vertex wind data (UV1 in the renderer).
#[derive(Debug, Clone, Default)]
pub struct WindMesh {
    pub mesh: MeshData,
    pub wind: Vec<[f32; 2]>,
}

/// Pack sway amplitude (meters at unit wind, clamped to 0..4) and a phase
/// (0..1, 64 steps) into one float: `floor(phase*64)*4 + sway`.
pub fn pack_wind(sway: f32, flutter: f32, phase: f32) -> [f32; 2] {
    let p = (phase.rem_euclid(1.0) * 64.0).floor().min(63.0);
    [p * 4.0 + sway.clamp(0.0, 3.99), flutter]
}

/// Inverse of [`pack_wind`]: (sway, flutter, phase).
pub fn unpack_wind(w: [f32; 2]) -> (f32, f32, f32) {
    let p = (w[0] / 4.0).floor();
    (w[0] - p * 4.0, w[1], p / 64.0)
}

impl WindMesh {
    pub fn is_empty(&self) -> bool {
        self.mesh.is_empty()
    }
    fn vertex(&mut self, p: Vec3, n: Vec3, uv: [f32; 2], c: [f32; 4], w: [f32; 2]) -> u32 {
        self.wind.push(w);
        self.mesh.vertex(p.to_array(), n.to_array(), uv, c)
    }
    /// Append `src` rotated, uniformly scaled and moved. Sway amplitudes are
    /// scaled too, so the merged mesh needs no transform scale.
    pub fn append_transformed(&mut self, src: &WindMesh, rot: Quat, scale: f32, offset: Vec3, phase_shift: f32) {
        let base = self.mesh.positions.len() as u32;
        for i in 0..src.mesh.positions.len() {
            let p = rot * (Vec3::from(src.mesh.positions[i]) * scale) + offset;
            let n = rot * Vec3::from(src.mesh.normals[i]);
            self.mesh.positions.push(p.to_array());
            self.mesh.normals.push(n.to_array());
            self.mesh.uvs.push(src.mesh.uvs[i]);
            self.mesh.colors.push(src.mesh.colors[i]);
            let (s, f, ph) = unpack_wind(src.wind[i]);
            self.wind.push(pack_wind(s * scale, f, ph + phase_shift));
        }
        self.mesh.indices.extend(src.mesh.indices.iter().map(|i| i + base));
    }
    pub fn triangle_count(&self) -> usize {
        self.mesh.triangle_count()
    }
}

/// Which bark material a model's bark mesh uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BarkStyle {
    /// Fissured bark texture.
    Rough,
    /// Ringed palm trunk texture.
    Palm,
}

/// A grown plant in model space: Y up, base at the origin, meters.
#[derive(Debug, Clone)]
pub struct TreeModel {
    /// Woody parts, drawn with the bark material ([`TreeModel::bark_style`]).
    pub bark: WindMesh,
    /// Leaves, fronds and smooth succulent parts, drawn with the
    /// alpha-tested, two-sided leaf material.
    pub leaves: WindMesh,
    pub bark_style: BarkStyle,
    pub height: f32,
    pub crown_radius: f32,
    /// Branch index per vertex of `bark` and of `leaves` (G11: a destruction phase splits a
    /// branch off by these). 0 is the trunk and anything not in a branch; each branch (and the
    /// twigs and leaf cards it carries, not its children) has its own index.
    pub bark_branch: Vec<u32>,
    pub leaf_branch: Vec<u32>,
}

// --------------------------------------------------------------- colors --

fn srgb(hex: u32) -> [f32; 4] {
    crate::mesh::rgb(hex)
}

/// Vertex-color multiplier that turns the atlas' neutral green into `hex`.
fn leaf_tint(hex: u32) -> [f32; 4] {
    let want = srgb(hex);
    let lin = |c: f32| if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) };
    let b = textures::LEAF_BASE;
    [want[0] / lin(b[0]), want[1] / lin(b[1]), want[2] / lin(b[2]), 1.0]
}

fn mulc(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [a[0] * b[0], a[1] * b[1], a[2] * b[2], a[3]]
}

fn scalec(a: [f32; 4], k: f32) -> [f32; 4] {
    [a[0] * k, a[1] * k, a[2] * k, a[3]]
}

fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn any_perp(t: Vec3) -> Vec3 {
    let a = if t.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
    t.cross(a).normalize()
}

// -------------------------------------------------------------- builder --

#[derive(Clone, Copy)]
struct TubeStyle {
    sides: usize,
    /// (count, depth): cactus ribs as an angular radius modulation.
    ribs: Option<(f32, f32)>,
    /// Root flare amount at the base (0 = none).
    flare: f32,
    /// Ratio of the b-axis radius to the n-axis radius (flat pads).
    aniso: f32,
    /// Smooth skin (cactus, banana stem): leaf material, solid UV.
    smooth: bool,
}

impl TubeStyle {
    fn bark(sides: usize) -> Self {
        Self { sides, ribs: None, flare: 0.0, aniso: 1.0, smooth: false }
    }
}

/// Where a leaf vertex came from (finish pass treats them differently).
#[derive(Clone, Copy, PartialEq, Eq)]
enum LeafVert {
    Card,
    Solid,
}

struct Builder {
    bark: WindMesh,
    leaves: WindMesh,
    leaf_kind: Vec<LeafVert>,
    rng: Rng,
    far: bool,
    /// Fraction of cards kept in the far model (and their area boost).
    far_keep: f32,
    card_index: u64,
    seed: u64,
    /// Nominal height for sway normalization.
    h: f32,
    /// Overall sway multiplier (stiff cactus .. floppy grass).
    flex: f32,
    bark_col: [f32; 4],
    leaf_col: [f32; 4],
    /// How far leaf normals bend toward the crown's outward direction.
    normal_bend: f32,
    bark_tile: f32,
    bark_style: BarkStyle,
    /// Optional bark color pattern: (t along tube, angle, world pos) -> multiplier.
    bark_pattern: Option<fn(f32, f32, Vec3) -> [f32; 4]>,
    /// Extra tint for cards grown by [`Builder::branch`].
    card_tint: [f32; 4],
    /// Near cards as flat quads (2 triangles) instead of cupped 3x2 grids: needle sprays are
    /// small and many, so their count, not their curl, makes the crown (trees look pass).
    pub(crate) flat_cards: bool,
    /// Branch index of the vertices added since the last [`Builder::mark`], and the per-vertex
    /// lists stamped so far.
    cur_branch: u32,
    next_branch: u32,
    /// Generators that do not mark branches themselves: every `tube` starts a branch.
    auto_branch: bool,
    bark_branch: Vec<u32>,
    leaf_branch: Vec<u32>,
}

impl Builder {
    /// Gives every vertex added since the last mark to the current branch, then makes `id` current.
    pub(crate) fn mark(&mut self, id: u32) {
        let cur = self.cur_branch;
        self.bark_branch.resize(self.bark.mesh.positions.len(), cur);
        self.leaf_branch.resize(self.leaves.mesh.positions.len(), cur);
        self.cur_branch = id;
        self.auto_branch = false;
    }

    /// A fresh branch index; the current branch is `mark`ed and the new one made current.
    /// Returns the previous index, to restore with `mark` when the branch ends.
    pub(crate) fn begin_branch(&mut self) -> u32 {
        self.next_branch += 1;
        let prev = self.cur_branch;
        self.mark(self.next_branch);
        prev
    }

    /// The first tube of a tree is its trunk (index 0, with anything added before it); later
    /// tubes of generators that do not mark branches start one each.
    fn auto_mark(&mut self) {
        if self.auto_branch && !self.bark.mesh.positions.is_empty() {
            self.next_branch += 1;
            let id = self.next_branch;
            self.mark(id);
            self.auto_branch = true;
        }
    }

    fn new(seed: u64, far: bool, far_keep: f32) -> Self {
        Self {
            bark: WindMesh::default(),
            leaves: WindMesh::default(),
            leaf_kind: Vec::new(),
            rng: Rng::new(seed),
            far,
            far_keep,
            card_index: 0,
            seed,
            h: 10.0,
            flex: 1.0,
            bark_col: srgb(0x6b5444),
            leaf_col: leaf_tint(0x5f9040),
            normal_bend: 0.75,
            bark_tile: 1.2,
            bark_style: BarkStyle::Rough,
            bark_pattern: None,
            card_tint: [1.0; 4],
            flat_cards: false,
            cur_branch: 0,
            next_branch: 0,
            auto_branch: true,
            bark_branch: Vec::new(),
            leaf_branch: Vec::new(),
        }
    }

    /// Sway amplitude (m at unit wind) at `p`, plus branch whip.
    fn sway(&self, p: Vec3, whip: f32) -> f32 {
        let hn = (p.y.max(0.0) / self.h).min(1.5);
        self.flex * (0.035 * self.h * hn.powf(1.8) + whip)
    }

    /// Sweep a tube along `path` with per-point `radii` and `whip`.
    fn tube(&mut self, path: &[Vec3], radii: &[f32], whip: &[f32], phase: f32, st: TubeStyle) {
        self.auto_mark();
        let n_pts = path.len();
        if n_pts < 2 {
            return;
        }
        // Far LOD: decimate rings, thin the tube, draw into the leaf mesh.
        // Near: thin twigs drop every other ring.
        let rmax = radii.iter().fold(0.0f32, |m, r| m.max(*r));
        let step = if self.far { (n_pts / 3).max(1) } else if rmax < 0.035 && n_pts > 3 { 2 } else { 1 };
        let mut idx: Vec<usize> = (0..n_pts).step_by(step).collect();
        if *idx.last().unwrap() != n_pts - 1 {
            idx.push(n_pts - 1);
        }
        let sides = if self.far {
            if st.aniso < 0.5 { 4 } else if st.sides >= 12 { 6 } else if st.sides >= 8 { 4 } else { 3 }
        } else {
            st.sides
        };
        let to_leaves = self.far || st.smooth;
        let tangent = |i: usize| {
            let a = path[i.saturating_sub(1)];
            let b = path[(i + 1).min(n_pts - 1)];
            (b - a).normalize_or(Vec3::Y)
        };
        // Parallel-transport frames along the full path.
        let mut frames = Vec::with_capacity(n_pts);
        let mut nrm = any_perp(tangent(0));
        let mut prev_t = tangent(0);
        for i in 0..n_pts {
            let t = tangent(i);
            nrm = Quat::from_rotation_arc(prev_t, t) * nrm;
            nrm = (nrm - t * nrm.dot(t)).normalize_or(any_perp(t));
            frames.push((t, nrm, t.cross(nrm)));
            prev_t = t;
        }
        let mut s_acc = vec![0.0f32; n_pts];
        for i in 1..n_pts {
            s_acc[i] = s_acc[i - 1] + (path[i] - path[i - 1]).length();
        }
        let total = s_acc[n_pts - 1].max(1e-4);
        let u_rep = (TAU * radii[0] / self.bark_tile).round().max(1.0);
        let solid_uv = atlas_uv(LeafCell::Solid, 0.5, 0.5);
        // Hash, not the rng: far models skip tubes and must not shift the stream.
        let seed_angle = crate::hash01(path[0].x.to_bits() as u64, path[0].z.to_bits() as u64 ^ self.seed) * TAU;
        let ribs = if self.far { None } else { st.ribs };
        let mesh = if to_leaves { &mut self.leaves } else { &mut self.bark };
        let base = mesh.mesh.positions.len() as u32;
        for &i in &idx {
            let (t, n, b) = frames[i];
            let p = path[i];
            let tt = s_acc[i] / total;
            for j in 0..=sides {
                let th = j as f32 / sides as f32 * TAU;
                let (c, s) = (th.cos(), th.sin());
                let mut r = radii[i];
                let mut dr = 0.0;
                if let Some((count, depth)) = ribs {
                    r *= 1.0 + depth * (count * th).cos();
                    dr = -radii[i] * depth * count * (count * th).sin();
                }
                if st.flare > 0.0 && p.y < 2.5 {
                    let lobes = ((5.0 * th + seed_angle).cos().max(0.0)).powi(2);
                    r *= 1.0 + st.flare * lobes * (-(p.y + 0.4).max(0.0) / 0.7).exp();
                }
                let dir = n * c + b * s * st.aniso;
                let around = -n * s + b * c * st.aniso;
                let pos = p + dir * r;
                let nr = (n * c + b * s / st.aniso.max(0.05)).normalize();
                let normal = (nr - around.normalize() * dr / r.max(1e-4) * 0.5).normalize_or(nr);
                let _ = t;
                let uv = if to_leaves { solid_uv } else { [j as f32 / sides as f32 * u_rep, s_acc[i] / self.bark_tile] };
                let mut col = self.bark_col;
                if let Some(f) = self.bark_pattern {
                    col = mulc(col, f(tt, th, pos));
                }
                let w = pack_wind(self.flex * (0.035 * self.h * (pos.y.max(0.0) / self.h).min(1.5).powf(1.8) + whip[i]), 0.0, phase);
                mesh.wind.push(w);
                mesh.mesh.vertex(pos.to_array(), normal.to_array(), uv, col);
                if to_leaves {
                    self.leaf_kind.push(LeafVert::Solid);
                }
            }
        }
        let ring = sides as u32 + 1;
        for k in 0..idx.len() as u32 - 1 {
            for j in 0..sides as u32 {
                let a = base + k * ring + j;
                let b2 = a + ring;
                // Outward-facing: (around x along) points away from the axis.
                mesh.mesh.tri(a, a + 1, b2);
                mesh.mesh.tri(a + 1, b2 + 1, b2);
            }
        }
    }

    /// Next card id; in the far model returns `None` for dropped cards.
    fn card_keep(&mut self) -> Option<f32> {
        self.card_index += 1;
        if !self.far {
            return Some(1.0);
        }
        let h = crate::hash01(self.seed ^ 0xFA4, self.card_index);
        (h < self.far_keep).then(|| (1.0 / self.far_keep).sqrt().min(3.6))
    }

    /// A leaf-cluster card: base at `base`, growing along `up`, facing
    /// `normal`. `curl` cups the sides and bends the tip (negative = droop).
    #[allow(clippy::too_many_arguments)]
    fn card(&mut self, base: Vec3, up: Vec3, normal: Vec3, w: f32, h: f32, cell: LeafCell, sway: f32, phase: f32, curl: f32, tint: [f32; 4]) {
        let Some(k) = self.card_keep() else { return };
        let up = up.normalize_or(Vec3::Y);
        let right = up.cross(normal).normalize_or(any_perp(up));
        let normal = right.cross(up);
        let (w, h) = (w * k, h * k);
        // Far cards grow around their center so the crown keeps its size.
        let base = if k > 1.0 { base - up * h * (k - 1.0) / (2.0 * k) } else { base };
        // Near: 3 columns (cupped) x 2 rows; far: a flat quad.
        let n = if self.far || self.flat_cards { 2 } else { 3 };
        let rows = 2;
        let col = mulc(self.leaf_col, tint);
        let first = self.leaves.mesh.positions.len() as u32;
        for j in 0..rows {
            for i in 0..n {
                let u = i as f32 / (n - 1) as f32;
                let v = j as f32 / (rows - 1) as f32;
                let x = (u - 0.5) * 2.0;
                let bend = curl * (x * x * 0.4 + v * 0.3) * w.min(h);
                let p = base + right * (u - 0.5) * w + up * v * h + normal * bend;
                let flutter = 0.06 * (0.3 + 0.7 * v);
                let wv = pack_wind(sway + self.flex * 0.03 * v * h, flutter * self.flex, phase);
                self.leaves.vertex(p, normal, atlas_uv(cell, u, v), col, wv);
                self.leaf_kind.push(LeafVert::Card);
            }
        }
        for j in 0..rows as u32 - 1 {
            for i in 0..n as u32 - 1 {
                let a = first + j * n as u32 + i;
                self.leaves.mesh.tri(a, a + 1, a + n as u32);
                self.leaves.mesh.tri(a + 1, a + n as u32 + 1, a + n as u32);
            }
        }
    }

    /// A ribbon along `path` (fronds, blades, banana leaves). `side` gives
    /// the across direction per point, `width` the full width; `fold` lifts
    /// the midrib (V cross-section; negative hangs the edges down).
    #[allow(clippy::too_many_arguments)]
    fn ribbon(&mut self, path: &[Vec3], side: &[Vec3], width: &[f32], fold: f32, cell: LeafCell, whip0: f32, phase: f32, tint: [f32; 4]) {
        let Some(k) = self.card_keep() else { return };
        let n_pts = path.len();
        let idx: Vec<usize> = if self.far { (0..n_pts).step_by(((n_pts - 1) / 3).max(1)).chain([n_pts - 1]).collect() } else { (0..n_pts).collect() };
        let mut idx = idx;
        idx.dedup();
        let cols: &[f32] = if self.far { &[0.0, 1.0] } else { &[0.0, 0.5, 1.0] };
        let col = mulc(self.leaf_col, tint);
        let mut s_acc = vec![0.0f32; n_pts];
        for i in 1..n_pts {
            s_acc[i] = s_acc[i - 1] + (path[i] - path[i - 1]).length();
        }
        let total = s_acc[n_pts - 1].max(1e-4);
        let first = self.leaves.mesh.positions.len() as u32;
        for &i in &idx {
            let t = (path[(i + 1).min(n_pts - 1)] - path[i.saturating_sub(1)]).normalize_or(Vec3::Y);
            let sd = side[i].normalize_or(any_perp(t));
            let mut nrm = sd.cross(t).normalize_or(Vec3::Y);
            if nrm.y < 0.0 {
                nrm = -nrm;
            }
            let w = width[i] * k.min(1.6);
            let v = s_acc[i] / total;
            for &u in cols {
                let x = (u - 0.5) * 2.0;
                let lift = fold * (1.0 - x.abs()) * w * 0.5;
                let p = path[i] + sd * x * w * 0.5 + nrm * lift;
                let n = (nrm - sd * x * fold * 0.8).normalize_or(nrm);
                let whip = whip0 + 0.05 * s_acc[i];
                let wv = pack_wind(self.sway(p, whip), 0.05 * self.flex * v, phase);
                self.leaves.vertex(p, n, atlas_uv(cell, u, v), col, wv);
                self.leaf_kind.push(LeafVert::Card);
            }
        }
        let c = cols.len() as u32;
        for k in 0..idx.len() as u32 - 1 {
            for j in 0..c - 1 {
                let a = first + k * c + j;
                self.leaves.mesh.tri(a, a + c, a + 1);
                self.leaves.mesh.tri(a + 1, a + c, a + c + 1);
            }
        }
    }

    /// Bake AO, crown rim and bent normals; produce the model.
    fn finish(mut self) -> TreeModel {
        self.mark(0);
        // Crown volume from leaf cards.
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        let mut any = false;
        for (i, p) in self.leaves.mesh.positions.iter().enumerate() {
            if self.leaf_kind[i] == LeafVert::Card {
                lo = lo.min(Vec3::from(*p));
                hi = hi.max(Vec3::from(*p));
                any = true;
            }
        }
        let center = (lo + hi) * 0.5;
        let radii = ((hi - lo) * 0.5).max(Vec3::splat(0.25));
        let crown = |p: Vec3| -> (f32, Vec3) {
            let d = (p - center) / radii;
            (d.length(), (d / radii).normalize_or(Vec3::Y))
        };
        let height = self
            .bark
            .mesh
            .positions
            .iter()
            .chain(self.leaves.mesh.positions.iter())
            .fold(0.0f32, |m, p| m.max(p[1]));
        for i in 0..self.leaves.mesh.positions.len() {
            let p = Vec3::from(self.leaves.mesh.positions[i]);
            let ground = 0.72 + 0.28 * smooth(0.0, 1.2, p.y);
            if self.leaf_kind[i] == LeafVert::Solid {
                let ao = if any { 0.55 + 0.45 * smooth(0.35, 1.05, crown(p).0) } else { 1.0 };
                self.leaves.mesh.colors[i] = scalec(self.leaves.mesh.colors[i], ao * ground);
                continue;
            }
            let (dist, out) = crown(p);
            let mut n = Vec3::from(self.leaves.mesh.normals[i]);
            if n.dot(out) < 0.0 {
                n = -n;
            }
            let n = n.lerp(out, self.normal_bend).normalize_or(out);
            self.leaves.mesh.normals[i] = n.to_array();
            let hfrac = ((p.y - lo.y) / (hi.y - lo.y).max(0.1)).clamp(0.0, 1.0);
            let ao = (0.4 + 0.6 * smooth(0.1, 1.0, dist)) * (0.8 + 0.2 * hfrac) * ground;
            let rim = smooth(0.78, 1.08, dist) * (0.5 + 0.5 * hfrac);
            let c = scalec(self.leaves.mesh.colors[i], ao);
            let warm = [c[0] * 1.18, c[1] * 1.12, c[2] * 0.8, c[3]];
            self.leaves.mesh.colors[i] = [
                c[0] + (warm[0] - c[0]) * rim * 0.45,
                c[1] + (warm[1] - c[1]) * rim * 0.45,
                c[2] + (warm[2] - c[2]) * rim * 0.45,
                1.0,
            ];
        }
        for i in 0..self.bark.mesh.positions.len() {
            let p = Vec3::from(self.bark.mesh.positions[i]);
            let ao = if any { 0.5 + 0.5 * smooth(0.3, 1.05, crown(p).0) } else { 1.0 };
            let ground = 0.7 + 0.3 * smooth(-0.3, 1.2, p.y);
            self.bark.mesh.colors[i] = scalec(self.bark.mesh.colors[i], ao * ground);
        }
        let crown_radius = if any { radii.x.max(radii.z) } else { 0.5 };
        TreeModel { bark: self.bark, leaves: self.leaves, bark_style: self.bark_style, height, crown_radius, bark_branch: self.bark_branch, leaf_branch: self.leaf_branch }
    }

    /// Grow a smooth path: `segs` steps of `len/segs`, bending toward `+Y *
    /// up` each step and wandering by `wiggle`.
    fn path(&mut self, start: Vec3, dir: Vec3, len: f32, segs: usize, up: f32, wiggle: f32) -> Vec<Vec3> {
        let mut pts = vec![start];
        let mut d = dir.normalize_or(Vec3::Y);
        let step = len / segs as f32;
        let mut p = start;
        for _ in 0..segs {
            let wob = self.rng.unit();
            d = (d + Vec3::Y * up / segs as f32 * 2.0 + wob * wiggle / (segs as f32).sqrt()).normalize_or(d);
            p += d * step;
            pts.push(p);
        }
        pts
    }
}

fn lerp_path(path: &[Vec3], t: f32) -> (Vec3, Vec3, usize) {
    let f = t.clamp(0.0, 1.0) * (path.len() - 1) as f32;
    let i = (f.floor() as usize).min(path.len() - 2);
    let k = f - i as f32;
    let p = path[i].lerp(path[i + 1], k);
    (p, (path[i + 1] - path[i]).normalize_or(Vec3::Y), i)
}

// -------------------------------------------------- generic branching --

#[derive(Clone, Copy)]
struct LeafSpec {
    cell: LeafCell,
    /// Card height (m); width = size * aspect.
    size: f32,
    aspect: f32,
    per_twig: usize,
    /// Leaves start this far along a twig.
    from: f32,
    curl: f32,
    /// 0 = cards face outward from the trunk, 1 = face the sky.
    face_up: f32,
    /// 0 = cards grow along the twig, 1 = point straight up.
    point_up: f32,
}

#[derive(Clone, Copy)]
struct BranchSpec {
    /// Branch levels below the trunk.
    levels: usize,
    children: [usize; 4],
    start: [f32; 4],
    /// Child angle from the parent (degrees) per parent level.
    angle: [f32; 4],
    len: [f32; 4],
    /// Upward tropism per level (negative droops).
    up: [f32; 4],
    wiggle: [f32; 4],
    rad_ratio: f32,
    /// Child length factor by position along the parent (crown silhouette).
    shape: fn(f32) -> f32,
    leaf: LeafSpec,
    /// Leaves on the second-to-last level too (denser, fuller crowns).
    leaves_on_parent: bool,
}

const SIDES: [usize; 5] = [12, 8, 6, 4, 3];
const SEGS: [usize; 5] = [12, 7, 5, 4, 3];

impl Builder {
    #[allow(clippy::too_many_arguments)]
    fn branch(&mut self, spec: &BranchSpec, level: usize, start: Vec3, dir: Vec3, len: f32, r0: f32, whip0: f32, phase: f32, flare: f32) {
        let prev_branch = if level == 0 { self.cur_branch } else { self.begin_branch() };
        let segs = SEGS[level.min(4)];
        let up = spec.up[level.min(3)];
        let wiggle = spec.wiggle[level.min(3)];
        let path = self.path(start, dir, len, segs, up, wiggle);
        let tip_r = (r0 * 0.3).max(0.008);
        let radii: Vec<f32> = (0..path.len()).map(|i| r0 + (tip_r - r0) * (i as f32 / segs as f32).powf(0.8)).collect();
        let whip: Vec<f32> = (0..path.len()).map(|i| whip0 + 0.012 * (level as f32 + 1.0) * len * i as f32 / segs as f32).collect();
        // Far model: only the thick wood (trunk, a tree's main limbs).
        let draw = !self.far || level == 0 || (self.h > 5.0 && r0 >= 0.15 * self.h / 10.0);
        if draw {
            let sides = SIDES[level.min(4)].min(((r0 * 60.0).round() as usize).clamp(if level == 0 { 5 } else { 3 }, 12));
            let mut st = TubeStyle::bark(sides);
            st.flare = flare;
            self.tube(&path, &radii, &whip, phase, st);
        }
        if level < spec.levels {
            let n = spec.children[level];
            let start_t = spec.start[level];
            let base_az = self.rng.f() * TAU;
            for k in 0..n {
                let t = start_t + (0.97 - start_t) * ((k as f32 + 0.5 + (self.rng.f() - 0.5) * 0.7) / n as f32).clamp(0.0, 1.0);
                let (p, tan, _) = lerp_path(&path, t);
                let az = base_az + k as f32 * 2.399_963 + self.rng.r(-0.3, 0.3);
                let perp = any_perp(tan);
                let perp = Quat::from_axis_angle(tan, az) * perp;
                let ang = spec.angle[level].to_radians() * self.rng.r(0.8, 1.2);
                let cdir = (tan * ang.cos() + perp * ang.sin()).normalize();
                let clen = len * spec.len[level] * (spec.shape)(t) * self.rng.r(0.8, 1.2);
                let r_here = r0 + (tip_r - r0) * t.powf(0.8);
                let cr = (r_here * spec.rad_ratio * self.rng.r(0.85, 1.15)).min(r_here * 0.85).max(0.006);
                let w_here = whip0 + 0.012 * (level as f32 + 1.0) * len * t;
                let cphase = phase + self.rng.r(0.05, 0.35);
                self.branch(spec, level + 1, p, cdir, clen.max(0.15), cr, w_here, cphase, 0.0);
            }
        }
        let leafy = level == spec.levels || (spec.leaves_on_parent && level + 1 == spec.levels);
        if leafy && spec.leaf.per_twig > 0 {
            let l = spec.leaf;
            let n = if level == spec.levels { l.per_twig } else { l.per_twig.div_ceil(2) };
            for k in 0..n {
                let t = l.from + (1.0 - l.from) * (k as f32 + self.rng.f() * 0.8) / n as f32;
                let (p, tan, _) = lerp_path(&path, t);
                let outward = Vec3::new(p.x, 0.0, p.z).normalize_or(self.rng.unit());
                let up_dir = tan.lerp(Vec3::Y, l.point_up) + self.rng.unit() * 0.45;
                let face = (outward.lerp(Vec3::Y, l.face_up) + self.rng.unit() * 0.5).normalize_or(Vec3::Y);
                let size = l.size * self.rng.r(0.8, 1.2);
                let whip = whip0 + 0.012 * (level as f32 + 1.0) * len * t;
                let sway = self.sway(p, whip);
                self.card(p - up_dir.normalize_or(Vec3::Y) * size * 0.12, up_dir, face, size * l.aspect, size, l.cell, sway, phase + self.rng.r(0.0, 0.1), l.curl, self.card_tint);
            }
        }
        if level > 0 {
            self.mark(prev_branch);
        }
    }

    /// Standard tree: a trunk with `spec` branching on top.
    fn grow_tree(&mut self, spec: &BranchSpec, trunk_len: f32, trunk_r: f32, lean: Vec3, flare: f32) {
        let dir = (Vec3::Y + lean).normalize();
        self.branch(spec, 0, Vec3::new(0.0, -0.4, 0.0), dir, trunk_len, trunk_r, 0.0, self.rng.f(), flare);
    }
}

// ---------------------------------------------------------------- species --

/// Grow a species variant with the default genome (the shared, cached look).
pub fn build(kind: TreeKind, variant: u32, far: bool) -> TreeModel {
    build_genome(kind, variant, far, &Genome::default())
}

/// Grow a species variant shaped by `genome` (see `species.rs`). The default
/// genome reproduces [`build`] exactly; this is the entry point for
/// per-instance unique meshes.
pub fn build_genome(kind: TreeKind, variant: u32, far: bool, genome: &Genome) -> TreeModel {
    // Far models keep roughly this many cards per tree.
    let near_cards = if far { build_inner(kind, variant, false, 1.0, genome).1 } else { 0 };
    let keep = if far { (species::def(kind).far_cards as f32 / near_cards.max(1) as f32).min(1.0) } else { 1.0 };
    build_inner(kind, variant, far, keep, genome).0
}

fn build_inner(kind: TreeKind, variant: u32, far: bool, keep: f32, genome: &Genome) -> (TreeModel, u64) {
    let seed = 0x7EE5_0000 + (kind as u64) * 977 + variant as u64 * 31;
    let mut b = Builder::new(seed, far, keep);
    let v = variant as usize % VARIANTS as usize;
    let leaf = species::def(kind).leaf[v];
    use TreeKind::*;
    match kind {
        Broadleaf => oak(&mut b, leaf),
        Maple => maple(&mut b, leaf),
        PlaneTree => plane(&mut b, leaf),
        Birch => birch(&mut b, leaf),
        Conifer => spruce(&mut b, leaf, false),
        Fir => spruce(&mut b, leaf, true),
        Pine => pine(&mut b, leaf),
        Palm => coconut(&mut b, leaf),
        DatePalm => date_palm(&mut b, leaf),
        Cypress => cypress(&mut b, leaf),
        Olive => olive(&mut b, leaf),
        Acacia => acacia(&mut b, leaf),
        Baobab => baobab(&mut b, leaf),
        Banana => banana(&mut b, leaf),
        Cactus => saguaro(&mut b, v),
        PricklyPear => prickly_pear(&mut b),
        Shrub => shrub(&mut b, leaf, LeafCell::Oval, [1.0; 4]),
        Boxwood => boxwood(&mut b, leaf),
        Juniper => juniper(&mut b, leaf),
        Oleander => oleander(&mut b, leaf, v),
        Fern => fern(&mut b, leaf),
        GrassTussock => tussock(&mut b, leaf),
        FloweringBush => flowering(&mut b, leaf, v),
    }
    let cards = b.card_index;
    let mut model = b.finish();
    genome.apply(&mut model);
    (model, cards)
}

fn crown_dome(t: f32) -> f32 {
    1.0 - 0.45 * t
}
fn crown_ovoid(t: f32) -> f32 {
    0.55 + 0.7 * (PI * (0.15 + 0.85 * t)).sin() - 0.25 * t
}
fn crown_cone(t: f32) -> f32 {
    1.0 - 0.8 * t
}
fn crown_flat(_t: f32) -> f32 {
    1.0
}

fn oak(b: &mut Builder, leaf: u32) {
    b.h = 13.0;
    b.leaf_col = leaf_tint(leaf);
    b.bark_col = srgb(0x6a5b4c);
    let spec = BranchSpec {
        levels: 3,
        children: [6, 5, 4, 0],
        start: [0.42, 0.2, 0.25, 0.0],
        angle: [58.0, 45.0, 42.0, 0.0],
        len: [0.72, 0.55, 0.5, 0.0],
        up: [0.25, 0.08, 0.12, 0.0],
        wiggle: [0.12, 0.35, 0.35, 0.3],
        rad_ratio: 0.62,
        shape: crown_dome,
        leaf: LeafSpec { cell: LeafCell::Oak, size: 2.0, aspect: 1.0, per_twig: 5, from: 0.3, curl: 0.25, face_up: 0.3, point_up: 0.35 },
        leaves_on_parent: true,
    };
    let lean = Vec3::new(b.rng.r(-0.08, 0.08), 0.0, b.rng.r(-0.08, 0.08));
    let tl = b.rng.r(7.5, 9.0);
    b.grow_tree(&spec, tl, 0.42, lean, 0.35);
}

fn maple(b: &mut Builder, leaf: u32) {
    b.h = 14.0;
    b.leaf_col = leaf_tint(leaf);
    b.bark_col = srgb(0x6e6152);
    let spec = BranchSpec {
        levels: 3,
        children: [8, 4, 4, 0],
        start: [0.38, 0.2, 0.25, 0.0],
        angle: [42.0, 45.0, 40.0, 0.0],
        len: [0.6, 0.55, 0.5, 0.0],
        up: [0.4, 0.15, 0.1, 0.0],
        wiggle: [0.1, 0.3, 0.35, 0.3],
        rad_ratio: 0.6,
        shape: crown_ovoid,
        leaf: LeafSpec { cell: LeafCell::Palmate, size: 1.9, aspect: 1.0, per_twig: 5, from: 0.3, curl: 0.2, face_up: 0.35, point_up: 0.4 },
        leaves_on_parent: true,
    };
    let tl = b.rng.r(10.0, 11.5);
    b.grow_tree(&spec, tl, 0.36, Vec3::new(b.rng.r(-0.05, 0.05), 0.0, 0.0), 0.25);
}

fn plane_bark(_t: f32, th: f32, p: Vec3) -> [f32; 4] {
    // Camouflage patches: cream, olive and grey plates.
    let n = crate::hash01((p.y * 2.2).floor() as u64 * 31 + (th * 2.5).floor() as u64, 77);
    let m = crate::hash01((p.y * 1.3 + 0.5).floor() as u64 * 17 + (th * 1.7 + 1.0).floor() as u64, 78);
    let cream = [1.45, 1.4, 1.15, 1.0];
    let olive = [0.95, 1.0, 0.75, 1.0];
    if n > 0.55 { cream } else if m > 0.5 { olive } else { [1.0; 4] }
}

fn plane(b: &mut Builder, leaf: u32) {
    b.h = 17.0;
    b.leaf_col = leaf_tint(leaf);
    b.bark_col = srgb(0x8a8270);
    b.bark_pattern = Some(plane_bark);
    let spec = BranchSpec {
        levels: 3,
        children: [9, 5, 3, 0],
        start: [0.36, 0.2, 0.3, 0.0],
        angle: [36.0, 50.0, 45.0, 0.0],
        len: [0.6, 0.55, 0.55, 0.0],
        up: [0.3, 0.02, 0.06, 0.0],
        wiggle: [0.08, 0.3, 0.3, 0.3],
        rad_ratio: 0.6,
        shape: crown_ovoid,
        leaf: LeafSpec { cell: LeafCell::Palmate, size: 2.1, aspect: 1.0, per_twig: 4, from: 0.3, curl: 0.2, face_up: 0.3, point_up: 0.3 },
        leaves_on_parent: true,
    };
    let tl = b.rng.r(13.0, 15.0);
    b.grow_tree(&spec, tl, 0.4, Vec3::ZERO, 0.2);
}

fn birch_bark(t: f32, th: f32, p: Vec3) -> [f32; 4] {
    // White bark, dark horizontal lenticel dashes, darker rough base.
    let row = (p.y * 6.0).floor() as u64;
    let col = (th * 3.0).floor() as u64;
    let dash = crate::hash01(row * 13 + col, 91) > 0.82;
    let base = 1.0 - 0.6 * (1.0 - smooth(0.0, 0.12, t));
    let k = if dash { 0.3 } else { 1.0 };
    [k * base, k * base, k * base, 1.0]
}

fn birch(b: &mut Builder, leaf: u32) {
    b.h = 14.0;
    b.leaf_col = leaf_tint(leaf);
    b.bark_col = srgb(0xe2ddd2);
    b.bark_pattern = Some(birch_bark);
    b.flex = 1.4;
    let spec = BranchSpec {
        levels: 3,
        children: [13, 4, 3, 0],
        start: [0.3, 0.2, 0.25, 0.0],
        angle: [48.0, 45.0, 35.0, 0.0],
        len: [0.32, 0.6, 0.55, 0.0],
        up: [0.15, -0.35, -0.5, 0.0],
        wiggle: [0.06, 0.25, 0.3, 0.3],
        rad_ratio: 0.55,
        shape: crown_ovoid,
        leaf: LeafSpec { cell: LeafCell::Oval, size: 1.35, aspect: 0.9, per_twig: 4, from: 0.2, curl: -0.3, face_up: 0.2, point_up: 0.0 },
        leaves_on_parent: true,
    };
    let tl = b.rng.r(12.5, 14.0);
    b.grow_tree(&spec, tl, 0.19, Vec3::new(b.rng.r(-0.1, 0.1), 0.0, b.rng.r(-0.1, 0.1)), 0.15);
}

/// Spruce (or fir): whorls of drooping branches carrying needle sprays.
fn spruce(b: &mut Builder, leaf: u32, fir: bool) {
    let h = b.rng.r(16.0, 20.0) * if fir { 0.9 } else { 1.0 };
    b.h = h;
    b.leaf_col = leaf_tint(leaf);
    b.bark_col = srgb(if fir { 0x6e665c } else { 0x5e4a3c });
    b.normal_bend = 0.6;
    let trunk = b.path(Vec3::new(0.0, -0.4, 0.0), Vec3::Y, h + 0.4, 14, 0.0, 0.015);
    let radii: Vec<f32> = (0..trunk.len()).map(|i| 0.38 * (1.0 - i as f32 / 14.0).max(0.03)).collect();
    let whip = vec![0.0; trunk.len()];
    let st = TubeStyle { flare: 0.25, ..TubeStyle::bark(10) };
    b.tube(&trunk, &radii, &whip, 0.0, st);
    let width = if fir { 3.2 } else { 4.4 };
    let step = if fir { 0.55 } else { 0.65 };
    let mut y = 1.4;
    let mut whorl = 0;
    while y < h - 0.6 {
        let t = y / h;
        let n = 5 + (b.rng.f() * 3.0) as usize;
        let az0 = b.rng.f() * TAU;
        for k in 0..n {
            let az = az0 + k as f32 / n as f32 * TAU + b.rng.r(-0.2, 0.2);
            let out = Vec3::new(az.cos(), 0.0, az.sin());
            let len = (width * (1.0 - t).powf(0.9) + 0.35) * b.rng.r(0.85, 1.15);
            let droop = if fir { -0.05 } else { -0.25 - 0.2 * (1.0 - t) };
            let dir = (out + Vec3::Y * (if fir { 0.05 } else { -0.05 })).normalize();
            let path = b.path(Vec3::new(0.0, y, 0.0), dir, len, 5, droop, 0.08);
            // Tips turn back up.
            let path: Vec<Vec3> = path.iter().enumerate().map(|(i, p)| *p + Vec3::Y * 0.06 * len * (i as f32 / 5.0).powi(3)).collect();
            let radii: Vec<f32> = (0..path.len()).map(|i| (0.07 * (1.0 - t) + 0.02) * (1.0 - 0.8 * i as f32 / 5.0)).collect();
            let whip: Vec<f32> = (0..path.len()).map(|i| 0.04 * len * i as f32 / 5.0).collect();
            let phase = b.rng.f();
            if !b.far {
                let pick = |v: &[f32]| -> Vec<f32> { v.iter().step_by(2).copied().collect() };
                let p2: Vec<Vec3> = path.iter().step_by(2).copied().collect();
                b.tube(&p2, &pick(&radii), &pick(&whip), phase, TubeStyle::bark(3));
            }
            // Sprays: flat along the branch, plus a hanging curtain on spruce.
            let cards = ((len / 0.75).ceil() as usize).max(1);
            for c in 0..cards {
                let ct = (c as f32 + 0.3) / cards as f32;
                let (p, tan, _) = lerp_path(&path, ct * 0.85);
                let size = (0.75 + 0.5 * (1.0 - ct)) * if fir { 0.95 } else { 1.1 };
                let sway = b.sway(p, 0.04 * len * ct);
                let face = (Vec3::Y + b.rng.unit() * 0.35).normalize();
                b.card(p - tan * size * 0.1, tan, face, size * 0.95, size, LeafCell::NeedleSpray, sway, phase, if fir { 0.1 } else { -0.35 }, [1.0; 4]);
                if fir {
                    let side = tan.cross(Vec3::Y).normalize_or(Vec3::X);
                    let face2 = (side * 0.85 + Vec3::Y * 0.5).normalize();
                    b.card(p - tan * size * 0.1, tan, face2, size * 0.9, size, LeafCell::NeedleSpray, sway, phase + 0.03, 0.15, [0.95, 0.97, 0.97, 1.0]);
                }
                if !fir && c > 0 {
                    let side = tan.cross(Vec3::Y).normalize_or(Vec3::X);
                    let hang = (tan * 0.35 - Vec3::Y).normalize();
                    b.card(p + Vec3::Y * 0.1, hang, side, size * 0.8, size * 0.85, LeafCell::NeedleSpray, sway, phase + 0.05, 0.2, [0.92, 0.95, 0.95, 1.0]);
                }
            }
        }
        y += step * b.rng.r(0.85, 1.15) * (1.0 - 0.35 * t);
        whorl += 1;
    }
    let _ = whorl;
    // Leader: a couple of vertical sprays crown the top.
    for k in 0..3 {
        let a = k as f32 / 3.0 * PI;
        let face = Vec3::new(a.cos(), 0.0, a.sin());
        let sway = b.sway(Vec3::new(0.0, h, 0.0), 0.0);
        b.card(Vec3::new(0.0, h - 1.2, 0.0), Vec3::Y, face, 0.7, 1.7, LeafCell::NeedleSpray, sway, 0.0, 0.0, [1.0; 4]);
    }
}

fn pine_bark(t: f32, _th: f32, _p: Vec3) -> [f32; 4] {
    // Scots pine: grey-brown below, orange flaky bark up high.
    let k = smooth(0.45, 0.7, t);
    [1.0 + 0.55 * k, 1.0 + 0.1 * k, 1.0 - 0.3 * k, 1.0]
}

fn pine(b: &mut Builder, leaf: u32) {
    b.h = 19.0;
    b.leaf_col = leaf_tint(leaf);
    b.bark_col = srgb(0x6d5646);
    b.bark_pattern = Some(pine_bark);
    let spec = BranchSpec {
        levels: 3,
        children: [10, 5, 3, 0],
        start: [0.58, 0.25, 0.3, 0.0],
        angle: [65.0, 40.0, 35.0, 0.0],
        len: [0.32, 0.6, 0.5, 0.0],
        up: [0.1, 0.35, 0.45, 0.0],
        wiggle: [0.05, 0.4, 0.4, 0.4],
        rad_ratio: 0.5,
        shape: crown_cone,
        leaf: LeafSpec { cell: LeafCell::PineTuft, size: 2.0, aspect: 1.0, per_twig: 4, from: 0.4, curl: 0.15, face_up: 0.45, point_up: 0.6 },
        leaves_on_parent: false,
    };
    let tl = b.rng.r(16.0, 19.0);
    b.grow_tree(&spec, tl, 0.34, Vec3::new(b.rng.r(-0.06, 0.06), 0.0, b.rng.r(-0.06, 0.06)), 0.2);
}

fn cypress(b: &mut Builder, leaf: u32) {
    let h = b.rng.r(13.0, 16.0);
    b.h = h;
    b.flex = 0.8;
    b.leaf_col = leaf_tint(leaf);
    b.bark_col = srgb(0x5a4a3e);
    b.normal_bend = 0.85;
    let trunk = b.path(Vec3::new(0.0, -0.4, 0.0), Vec3::Y, h * 0.9, 10, 0.0, 0.02);
    let radii: Vec<f32> = (0..trunk.len()).map(|i| 0.28 * (1.0 - i as f32 / 10.0).max(0.05)).collect();
    b.tube(&trunk, &radii, &vec![0.0; trunk.len()], 0.0, TubeStyle::bark(8));
    // Flame silhouette: widest a third of the way up, pointed top.
    let prof = |t: f32| (PI * t.powf(0.6)).sin().powf(0.9) * (1.0 - 0.15 * t);
    let rmax = b.rng.r(1.1, 1.5);
    let mut y = 0.8;
    let mut k = 0;
    while y < h - 0.3 {
        let t = y / h;
        let az = k as f32 * 2.399_963;
        k += 1;
        let out = Vec3::new(az.cos(), 0.0, az.sin());
        let len = (rmax * prof(t) * 1.6).max(0.4);
        let dir = (out * 0.45 + Vec3::Y).normalize();
        let path = b.path(Vec3::new(0.0, y, 0.0), dir, len, 3, 0.2, 0.05);
        let phase = b.rng.f();
        if !b.far {
            let radii: Vec<f32> = (0..path.len()).map(|i| 0.04 * (1.0 - i as f32 / 4.0)).collect();
            b.tube(&path, &radii, &vec![0.0; path.len()], phase, TubeStyle::bark(3));
        }
        let cards = 4;
        for c in 0..cards {
            let (p, _tan, _) = lerp_path(&path, 0.25 + 0.75 * c as f32 / cards as f32);
            let r_here = rmax * prof(p.y / h);
            let outp = Vec3::new(p.x, 0.0, p.z).normalize_or(out);
            let p = Vec3::new(outp.x * r_here * 0.55, p.y, outp.z * r_here * 0.55);
            let size = 1.3 + 0.9 * prof(t);
            let sway = b.sway(p, 0.0);
            let face = (outp + b.rng.unit() * 0.4).normalize();
            b.card(p - Vec3::Y * size * 0.3, Vec3::Y + b.rng.unit() * 0.3, face, size * 0.8, size, LeafCell::Scale, sway, phase, 0.3, [1.0; 4]);
        }
        y += 0.32 * b.rng.r(0.8, 1.2);
    }
    let sway = b.sway(Vec3::new(0.0, h, 0.0), 0.0);
    b.card(Vec3::new(0.0, h - 1.4, 0.0), Vec3::Y, Vec3::X, 0.6, 1.8, LeafCell::Scale, sway, 0.0, 0.0, [1.0; 4]);
    b.card(Vec3::new(0.0, h - 1.4, 0.0), Vec3::Y, Vec3::Z, 0.6, 1.8, LeafCell::Scale, sway, 0.0, 0.0, [1.0; 4]);
}

fn olive(b: &mut Builder, leaf: u32) {
    b.h = 7.0;
    b.leaf_col = leaf_tint(leaf);
    b.bark_col = srgb(0x7a7368);
    let spec = BranchSpec {
        levels: 3,
        children: [5, 4, 4, 0],
        start: [0.45, 0.2, 0.2, 0.0],
        angle: [50.0, 45.0, 45.0, 0.0],
        len: [0.95, 0.6, 0.5, 0.0],
        up: [0.15, 0.05, 0.0, 0.0],
        wiggle: [0.55, 0.45, 0.4, 0.3],
        rad_ratio: 0.62,
        shape: crown_dome,
        leaf: LeafSpec { cell: LeafCell::Olive, size: 1.4, aspect: 1.0, per_twig: 4, from: 0.25, curl: 0.2, face_up: 0.35, point_up: 0.3 },
        leaves_on_parent: true,
    };
    // Gnarled, often split trunk.
    let stems = 1 + (b.rng.f() * 2.0) as usize;
    for s in 0..stems {
        let a = s as f32 / stems as f32 * TAU + b.rng.f();
        let lean = if stems > 1 { Vec3::new(a.cos(), 0.0, a.sin()) * 0.3 } else { Vec3::ZERO };
        let tl = b.rng.r(2.6, 3.4);
        let r = 0.34 / (stems as f32).sqrt();
        b.grow_tree(&spec, tl, r, lean, 0.4);
    }
}

fn acacia(b: &mut Builder, leaf: u32) {
    b.h = 8.0;
    b.leaf_col = leaf_tint(leaf);
    b.bark_col = srgb(0x5d5048);
    b.normal_bend = 0.55;
    // Low split into fanning stems, then a flat table of foliage.
    let trunk_len = b.rng.r(2.2, 3.0);
    let trunk = b.path(Vec3::new(0.0, -0.4, 0.0), Vec3::Y, trunk_len + 0.4, 6, 0.0, 0.1);
    let radii: Vec<f32> = (0..trunk.len()).map(|i| 0.32 - 0.08 * i as f32 / 6.0).collect();
    b.tube(&trunk, &radii, &vec![0.0; trunk.len()], 0.0, TubeStyle { flare: 0.25, ..TubeStyle::bark(10) });
    let top = *trunk.last().unwrap();
    let stems = 4 + (b.rng.f() * 2.0) as usize;
    let table_y = b.rng.r(6.2, 7.2);
    for s in 0..stems {
        let az = s as f32 / stems as f32 * TAU + b.rng.r(-0.3, 0.3);
        let out = Vec3::new(az.cos(), 0.0, az.sin());
        let dir = (out * 0.75 + Vec3::Y).normalize();
        let len = (table_y - top.y) / dir.y * b.rng.r(0.95, 1.1);
        let path = b.path(top, dir, len, 6, -0.05, 0.25);
        let radii: Vec<f32> = (0..path.len()).map(|i| 0.17 * (1.0 - 0.6 * i as f32 / 6.0)).collect();
        let whip: Vec<f32> = (0..path.len()).map(|i| 0.01 * i as f32).collect();
        let phase = b.rng.f();
        b.tube(&path, &radii, &whip, phase, TubeStyle::bark(7));
        let end = *path.last().unwrap();
        // Horizontal fan of twigs under the table.
        for k in 0..7 {
            let a = az + (k as f32 - 3.0) * 0.42 + b.rng.r(-0.15, 0.15);
            let d = Vec3::new(a.cos(), 0.08, a.sin());
            let l = b.rng.r(2.0, 3.4);
            let tw = b.path(end, d, l, 4, 0.04, 0.15);
            if !b.far {
                let r: Vec<f32> = (0..tw.len()).map(|i| 0.05 * (1.0 - 0.7 * i as f32 / 4.0)).collect();
                b.tube(&tw, &r, &vec![0.05; tw.len()], phase, TubeStyle::bark(4));
            }
            for c in 0..6 {
                let (p, tan, _) = lerp_path(&tw, 0.15 + 0.85 * c as f32 / 6.0);
                let size = b.rng.r(1.6, 2.3);
                let sway = b.sway(p, 0.06);
                let up = (tan + b.rng.unit() * 0.3).normalize();
                let h = b.rng.unit();
                let face = (Vec3::Y * 0.55 + Vec3::new(h.x, 0.0, h.z).normalize_or(Vec3::X) * 0.85).normalize();
                b.card(p + Vec3::Y * b.rng.r(-0.1, 0.5), up, face, size, size * 0.85, LeafCell::Acacia, sway, phase, 0.2, [1.0; 4]);
            }
        }
    }
}

fn baobab(b: &mut Builder, leaf: u32) {
    let h = b.rng.r(9.0, 11.0);
    b.h = h + 4.0;
    b.flex = 0.5;
    b.leaf_col = leaf_tint(leaf);
    b.bark_col = srgb(0x9a8a7e);
    // Bottle trunk.
    let trunk = b.path(Vec3::new(0.0, -0.4, 0.0), Vec3::Y, h, 14, 0.0, 0.02);
    let rmax = b.rng.r(1.7, 2.2);
    let radii: Vec<f32> = (0..trunk.len())
        .map(|i| {
            let t = i as f32 / 14.0;
            rmax * (1.0 + 0.12 * (PI * t).sin()) * (1.0 - 0.62 * t.powf(2.2))
        })
        .collect();
    b.tube(&trunk, &radii, &vec![0.0; trunk.len()], 0.0, TubeStyle { flare: 0.15, ..TubeStyle::bark(16) });
    let top = *trunk.last().unwrap();
    let spec = BranchSpec {
        levels: 2,
        children: [4, 3, 0, 0],
        start: [0.35, 0.3, 0.0, 0.0],
        angle: [50.0, 45.0, 0.0, 0.0],
        len: [0.55, 0.5, 0.0, 0.0],
        up: [0.05, 0.15, 0.0, 0.0],
        wiggle: [0.5, 0.5, 0.4, 0.0],
        rad_ratio: 0.45,
        shape: crown_flat,
        leaf: LeafSpec { cell: LeafCell::Palmate, size: 1.5, aspect: 1.0, per_twig: 3, from: 0.4, curl: 0.2, face_up: 0.4, point_up: 0.5 },
        leaves_on_parent: false,
    };
    let limbs = 6 + (b.rng.f() * 3.0) as usize;
    for k in 0..limbs {
        let az = k as f32 / limbs as f32 * TAU + b.rng.r(-0.2, 0.2);
        let out = Vec3::new(az.cos(), 0.0, az.sin());
        let dir = (out + Vec3::Y * b.rng.r(0.4, 1.0)).normalize();
        let start = top - Vec3::Y * b.rng.r(0.0, 0.8) + out * radii[13] * 0.5;
        let len = b.rng.r(3.0, 4.5);
        let phase = b.rng.f();
        b.branch(&spec, 0, start, dir, len, radii[13] * 0.45, 0.0, phase, 0.0);
    }
}

/// Smooth, curved palm trunk (bezier) with ring-scar texture.
fn palm_trunk(b: &mut Builder, h: f32, lean: f32, r0: f32, bulge: f32, curve: f32) -> (Vec3, Vec3) {
    let az = b.rng.f() * TAU;
    let side = Vec3::new(az.cos(), 0.0, az.sin());
    let p0 = Vec3::new(0.0, -0.4, 0.0);
    let p1 = Vec3::new(0.0, h * 0.35, 0.0) + side * lean * h * (0.6 - curve * 0.4);
    let p2 = Vec3::new(0.0, h * 0.7, 0.0) + side * lean * h * (0.9 + curve * 0.2);
    let p3 = Vec3::new(0.0, h, 0.0) + side * lean * h * (1.0 + curve * 0.4);
    let n = 40;
    let path: Vec<Vec3> = (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let u = 1.0 - t;
            p0 * u * u * u + p1 * 3.0 * u * u * t + p2 * 3.0 * u * t * t + p3 * t * t * t
        })
        .collect();
    let radii: Vec<f32> = (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            r0 * (1.0 + bulge * (-t * 14.0).exp()) * (1.0 - 0.28 * t) * (1.0 + 0.04 * (t * 9.0).sin())
        })
        .collect();
    let whip = vec![0.0; path.len()];
    b.tube(&path, &radii, &whip, 0.0, TubeStyle { flare: 0.0, ..TubeStyle::bark(14) });
    let top = *path.last().unwrap();
    let tan = (path[n] - path[n - 1]).normalize();
    (top, tan)
}

/// One palm frond: arching rachis, leaflets hanging in a V.
#[allow(clippy::too_many_arguments)]
fn frond(b: &mut Builder, base: Vec3, az: f32, elev: f32, len: f32, width: f32, droop: f32, cell: LeafCell, tint: [f32; 4]) {
    let out = Vec3::new(az.cos(), 0.0, az.sin());
    let mut d = (out * elev.cos() + Vec3::Y * elev.sin()).normalize();
    let n = 10;
    let mut p = base;
    let mut path = vec![p];
    for i in 0..n {
        let t = i as f32 / n as f32;
        d = (d - Vec3::Y * droop * (0.3 + t) * 0.35).normalize();
        p += d * len / n as f32;
        path.push(p);
    }
    let side = vec![out.cross(Vec3::Y).normalize(); path.len()];
    let widths: Vec<f32> = (0..path.len())
        .map(|i| {
            let t = i as f32 / n as f32;
            width * (t * 6.0).min(1.0).powf(0.7) * (1.0 - 0.75 * t.powf(1.5))
        })
        .collect();
    let phase = b.rng.f();
    b.ribbon(&path, &side, &widths, -0.35, cell, 0.05, phase, tint);
}

fn coconut(b: &mut Builder, leaf: u32) {
    let h = b.rng.r(11.0, 15.0);
    b.h = h;
    b.flex = 1.1;
    b.leaf_col = leaf_tint(leaf);
    b.bark_col = srgb(0x9a8a74);
    b.bark_style = BarkStyle::Palm;
    b.normal_bend = 0.35;
    b.bark_tile = 1.2;
    let (top, _) = palm_trunk(b, h, b.rng.r(0.08, 0.2), 0.24, 0.6, b.rng.r(0.3, 1.0));
    let n = 15 + (b.rng.f() * 4.0) as usize;
    for k in 0..n {
        let az = k as f32 * 2.399_963 + b.rng.r(-0.2, 0.2);
        let age = k as f32 / n as f32; // later = older, lower
        let elev = (55.0 - 75.0 * age + b.rng.r(-8.0, 8.0)).to_radians();
        let len = b.rng.r(4.2, 5.4) * (1.0 - 0.15 * (1.0 - age));
        frond(b, top + Vec3::Y * 0.15, az, elev, len, b.rng.r(1.1, 1.4), 0.9 + 0.5 * age, LeafCell::PalmFrond, [1.0; 4]);
    }
    // Crown shaft: a short swollen green-brown cap where fronds attach.
    let cap: Vec<Vec3> = (0..5).map(|i| top + Vec3::Y * (i as f32 * 0.18 - 0.4)).collect();
    let cap_r: Vec<f32> = (0..5).map(|i| 0.26 * (1.0 + 0.25 * (PI * i as f32 / 4.0).sin()) * (1.0 - 0.5 * (i as f32 / 4.0).powi(3))).collect();
    b.tube(&cap, &cap_r, &[0.0; 5], 0.0, TubeStyle::bark(12));
}

fn date_palm(b: &mut Builder, leaf: u32) {
    let h = b.rng.r(9.0, 13.0);
    b.h = h;
    b.flex = 0.8;
    b.leaf_col = leaf_tint(leaf);
    b.bark_col = srgb(0x8a7458);
    b.bark_style = BarkStyle::Palm;
    b.normal_bend = 0.35;
    b.bark_tile = 0.8;
    let (top, _) = palm_trunk(b, h, b.rng.r(0.0, 0.05), 0.36, 0.25, 0.2);
    let n = 26 + (b.rng.f() * 6.0) as usize;
    for k in 0..n {
        let az = k as f32 * 2.399_963 + b.rng.r(-0.2, 0.2);
        let age = k as f32 / n as f32;
        let elev = (72.0 - 70.0 * age + b.rng.r(-6.0, 6.0)).to_radians();
        frond(b, top + Vec3::Y * 0.2, az, elev, b.rng.r(3.6, 4.6), b.rng.r(0.9, 1.1), 0.45 + 0.35 * age, LeafCell::PalmFrond, [1.0; 4]);
    }
    // Skirt of dead fronds.
    let dead = 9 + (b.rng.f() * 5.0) as usize;
    for k in 0..dead {
        let az = k as f32 / dead as f32 * TAU + b.rng.r(-0.2, 0.2);
        let elev = b.rng.r(-80.0, -62.0).to_radians();
        frond(b, top - Vec3::Y * 0.35, az, elev, b.rng.r(2.4, 3.2), 0.7, 0.1, LeafCell::DeadFrond, leaf_tint_inv(b.leaf_col));
    }
}

/// Undo the species tint (dead fronds keep the atlas' own browns).
fn leaf_tint_inv(c: [f32; 4]) -> [f32; 4] {
    [1.0 / c[0].max(0.05), 1.0 / c[1].max(0.05), 1.0 / c[2].max(0.05), 1.0]
}

fn banana(b: &mut Builder, leaf: u32) {
    b.h = 4.5;
    b.flex = 1.3;
    b.leaf_col = leaf_tint(leaf);
    b.bark_col = srgb(0x6f8a48);
    b.normal_bend = 0.3;
    let stems = 2 + (b.rng.f() * 2.0) as usize;
    for s in 0..stems {
        let a = s as f32 / stems as f32 * TAU + b.rng.f();
        let off = Vec3::new(a.cos(), 0.0, a.sin()) * if s == 0 { 0.0 } else { b.rng.r(0.4, 0.8) };
        let sh = if s == 0 { b.rng.r(2.8, 3.4) } else { b.rng.r(1.4, 2.6) };
        let stem = b.path(off - Vec3::Y * 0.3, Vec3::Y + off * 0.05, sh + 0.3, 6, 0.0, 0.03);
        let radii: Vec<f32> = (0..stem.len()).map(|i| 0.15 * (1.0 - 0.4 * i as f32 / 6.0) * (sh / 3.0).sqrt()).collect();
        b.tube(&stem, &radii, &vec![0.0; stem.len()], 0.0, TubeStyle { smooth: true, ..TubeStyle::bark(9) });
        let top = *stem.last().unwrap();
        let leaves = 6 + (b.rng.f() * 3.0) as usize;
        for k in 0..leaves {
            let az = k as f32 * 2.399_963 + b.rng.f();
            let age = k as f32 / leaves as f32;
            let out = Vec3::new(az.cos(), 0.0, az.sin());
            let elev = (70.0 - 85.0 * age + b.rng.r(-10.0, 10.0)).to_radians();
            let mut d = (out * elev.cos() + Vec3::Y * elev.sin()).normalize();
            let len = b.rng.r(2.2, 3.0) * (sh / 3.0).sqrt();
            let n = 8;
            // Short petiole, then the blade.
            let mut p = top + d * 0.35;
            let mut path = vec![p];
            for i in 0..n {
                let t = i as f32 / n as f32;
                d = (d - Vec3::Y * (0.12 + 0.25 * age) * (0.5 + t)).normalize();
                p += d * len / n as f32;
                path.push(p);
            }
            let twist = b.rng.r(-0.4, 0.4);
            let side: Vec<Vec3> = (0..path.len()).map(|i| Quat::from_axis_angle(d, twist * i as f32 / n as f32) * out.cross(Vec3::Y)).collect();
            let w = b.rng.r(0.8, 1.0) * (sh / 3.0).sqrt();
            let widths: Vec<f32> = (0..path.len()).map(|i| w * ((i as f32 / n as f32) * 5.0).min(1.0)).collect();
            let tint = if age > 0.8 { [1.25, 1.0, 0.6, 1.0] } else { [1.0; 4] };
            let phase = b.rng.f();
            b.ribbon(&path, &side, &widths, 0.25, LeafCell::Banana, 0.15, phase, tint);
        }
    }
}

fn saguaro_skin(_t: f32, th: f32, _p: Vec3) -> [f32; 4] {
    // Lighter rib crests (spines catch the light), darker grooves.
    let c = (14.0 * th).cos();
    let k = 0.82 + 0.25 * c.max(0.0).powf(3.0);
    [k, k, k * 0.95, 1.0]
}

/// Tube that closes in a rounded dome at its end.
fn domed_radii(n: usize, r: f32, len: f32, dome: f32) -> Vec<f32> {
    (0..=n)
        .map(|i| {
            let s = i as f32 / n as f32 * len;
            let from_tip = len - s;
            if from_tip < dome {
                r * (1.0 - (1.0 - from_tip / dome).powi(2)).max(0.0).sqrt()
            } else {
                r
            }
        })
        .collect()
}

fn saguaro(b: &mut Builder, v: usize) {
    let h = b.rng.r(7.0, 10.0);
    b.h = h;
    b.flex = 0.05;
    b.bark_col = srgb(0x5e8a52);
    b.bark_pattern = Some(saguaro_skin);
    let ribs = Some((14.0, 0.07));
    let st = TubeStyle { ribs, smooth: true, ..TubeStyle::bark(42) };
    let n = 24;
    let trunk = b.path(Vec3::new(0.0, -0.4, 0.0), Vec3::Y, h + 0.4, n, 0.0, 0.01);
    let r = 0.34;
    let radii: Vec<f32> = domed_radii(n, r, h + 0.4, r * 1.2).iter().enumerate().map(|(i, x)| x * (1.0 - 0.1 * i as f32 / n as f32)).collect();
    b.tube(&trunk, &radii, &vec![0.0; trunk.len()], 0.0, st);
    let arms = [1usize, 2, 3][v % 3] + (b.rng.f() * 1.5) as usize;
    for k in 0..arms {
        let az = k as f32 / arms as f32 * TAU + b.rng.r(-0.5, 0.5);
        let out = Vec3::new(az.cos(), 0.0, az.sin());
        let y0 = b.rng.r(2.2, h * 0.6);
        let up_len = b.rng.r(1.5, (h - y0) * 0.8).max(1.0);
        let reach = b.rng.r(0.55, 0.85);
        // Out, elbow, up: a smooth quarter-bend.
        let m = 18;
        let ar = 0.24 * b.rng.r(0.85, 1.1);
        let mut path = Vec::new();
        for i in 0..=m {
            let t = i as f32 / m as f32;
            let p = if t < 0.4 {
                let a = t / 0.4 * PI * 0.5;
                Vec3::new(0.0, y0, 0.0) + out * (r * 0.5 + reach * a.sin()) + Vec3::Y * reach * (1.0 - a.cos())
            } else {
                Vec3::new(0.0, y0 + reach, 0.0) + out * (r * 0.5 + reach) + Vec3::Y * up_len * (t - 0.4) / 0.6
            };
            path.push(p);
        }
        let len: f32 = path.windows(2).map(|w| (w[1] - w[0]).length()).sum();
        let radii = domed_radii(m, ar, len, ar * 1.2);
        b.tube(&path, &radii, &vec![0.0; path.len()], 0.0, TubeStyle { sides: 32, ..st });
    }
}

/// A flat, lens-shaped prickly-pear pad grown from `base` along `dir`.
fn pad(b: &mut Builder, base: Vec3, dir: Vec3, flat: Vec3, len: f32, tint: [f32; 4]) -> Vec<Vec3> {
    let n = 8;
    let bend = b.rng.unit() * 0.08;
    let path: Vec<Vec3> = (0..=n).map(|i| base + (dir + bend * i as f32 / n as f32).normalize() * len * i as f32 / n as f32).collect();
    let w = len * b.rng.r(0.32, 0.4);
    let radii: Vec<f32> = (0..=n).map(|i| w * (PI * (i as f32 / n as f32).powf(0.8)).sin().max(0.0).powf(0.6).max(0.02)).collect();
    let old = b.bark_col;
    b.bark_col = mulc(b.bark_col, tint);
    // Frame: n-axis across the pad, b-axis (thin) through it.
    let _ = flat;
    b.tube(&path, &radii, &vec![0.0; path.len()], b.rng.f(), TubeStyle { aniso: 0.22, smooth: true, ..TubeStyle::bark(14) });
    b.bark_col = old;
    path
}

fn pear_skin(_t: f32, th: f32, p: Vec3) -> [f32; 4] {
    // Areole dots in a diamond lattice.
    let a = ((p.y * 9.0 + th * 2.0).fract() - 0.5).abs() + ((p.y * 9.0 - th * 2.0).fract() - 0.5).abs();
    let k = if a < 0.12 { 1.45 } else { 1.0 };
    [k, k, k * 0.8, 1.0]
}

fn prickly_pear(b: &mut Builder) {
    b.h = 2.0;
    b.flex = 0.05;
    b.bark_col = srgb(0x6a9a52);
    b.bark_pattern = Some(pear_skin);
    let mut queue: Vec<(Vec3, Vec3, f32, usize)> = Vec::new();
    for k in 0..3 {
        let az = k as f32 / 3.0 * TAU + b.rng.f();
        let d = (Vec3::new(az.cos(), 0.0, az.sin()) * 0.35 + Vec3::Y).normalize();
        queue.push((Vec3::new(0.0, -0.1, 0.0), d, b.rng.r(0.65, 0.85), 0));
    }
    let mut tips = Vec::new();
    let mut count = 0;
    while let Some((base, dir, len, depth)) = queue.pop() {
        count += 1;
        let flat = any_perp(dir);
        let tint = if depth == 0 { [0.85, 0.9, 0.8, 1.0] } else { [1.0; 4] };
        let path = pad(b, base, dir, flat, len, tint);
        let end = *path.last().unwrap();
        if depth < 2 && count < 14 {
            let kids = 1 + (b.rng.f() * 2.0) as usize;
            for _ in 0..kids {
                let t = b.rng.r(0.7, 0.92);
                let (p, _, _) = lerp_path(&path, t);
                let nd = (dir + b.rng.unit() * 0.9 + Vec3::Y * 0.3).normalize();
                queue.push((p, nd, len * b.rng.r(0.8, 1.0), depth + 1));
            }
        } else {
            tips.push((end, dir));
        }
    }
    // Red fruits (tunas) on the outer pad rims.
    let fruit = srgb(0xb0304a);
    let old = (b.bark_col, b.bark_pattern);
    b.bark_col = fruit;
    b.bark_pattern = None;
    for (end, dir) in tips.iter().take(5) {
        let base = *end - *dir * 0.05;
        let d = (*dir + b.rng.unit() * 0.6).normalize();
        let n = 6;
        let path: Vec<Vec3> = (0..=n).map(|i| base + d * 0.09 * i as f32 / n as f32 * 1.5).collect();
        let radii: Vec<f32> = (0..=n).map(|i| 0.045 * (PI * (i as f32 / n as f32).powf(0.7)).sin().max(0.15)).collect();
        if !b.far {
            b.tube(&path, &radii, &vec![0.0; path.len()], 0.0, TubeStyle { smooth: true, ..TubeStyle::bark(8) });
        }
    }
    (b.bark_col, b.bark_pattern) = old;
}

/// Bushy multi-stem shrub.
fn shrub(b: &mut Builder, leaf: u32, cell: LeafCell, tint: [f32; 4]) {
    b.h = 1.8;
    b.flex = 1.2;
    b.leaf_col = leaf_tint(leaf);
    b.bark_col = srgb(0x5a4a3c);
    let spec = BranchSpec {
        levels: 2,
        children: [5, 4, 0, 0],
        start: [0.3, 0.3, 0.0, 0.0],
        angle: [45.0, 40.0, 0.0, 0.0],
        len: [0.6, 0.55, 0.0, 0.0],
        up: [0.2, 0.15, 0.0, 0.0],
        wiggle: [0.3, 0.35, 0.3, 0.0],
        rad_ratio: 0.6,
        shape: crown_dome,
        leaf: LeafSpec { cell, size: 0.95, aspect: 1.0, per_twig: 4, from: 0.2, curl: 0.25, face_up: 0.3, point_up: 0.4 },
        leaves_on_parent: true,
    };
    let stems = 5 + (b.rng.f() * 3.0) as usize;
    for s in 0..stems {
        let az = s as f32 / stems as f32 * TAU + b.rng.r(-0.3, 0.3);
        let lean = Vec3::new(az.cos(), 0.0, az.sin()) * b.rng.r(0.3, 0.7);
        let phase = b.rng.f();
        let d = (Vec3::Y + lean).normalize();
        b.card_tint = tint;
        b.branch(&spec, 0, Vec3::new(0.0, -0.15, 0.0), d, b.rng.r(1.0, 1.5), 0.04, 0.0, phase, 0.0);
    }
}

fn boxwood(b: &mut Builder, leaf: u32) {
    b.h = 1.2;
    b.flex = 0.6;
    b.leaf_col = leaf_tint(leaf);
    b.bark_col = srgb(0x55463a);
    b.normal_bend = 0.9;
    let rx = b.rng.r(0.6, 0.8);
    let ry = b.rng.r(0.5, 0.65);
    let center = Vec3::new(0.0, ry * 0.95, 0.0);
    // A few stems inside.
    for s in 0..4 {
        let az = s as f32 / 4.0 * TAU;
        let d = (Vec3::new(az.cos(), 0.0, az.sin()) * 0.4 + Vec3::Y).normalize();
        let path = b.path(Vec3::new(0.0, -0.1, 0.0), d, ry * 1.4, 4, 0.1, 0.2);
        let r: Vec<f32> = (0..path.len()).map(|i| 0.035 * (1.0 - 0.6 * i as f32 / 4.0)).collect();
        if !b.far {
            b.tube(&path, &r, &vec![0.0; path.len()], 0.0, TubeStyle::bark(4));
        }
    }
    // Leaf cards over a lumpy ellipsoid shell (Fibonacci sphere).
    let n = 110;
    for i in 0..n {
        let z = 1.0 - 2.0 * (i as f32 + 0.5) / n as f32;
        let a = i as f32 * 2.399_963;
        let s = (1.0 - z * z).sqrt();
        let dir = Vec3::new(s * a.cos(), z, s * a.sin());
        if dir.y < -0.55 {
            continue;
        }
        let lump = 1.0 + 0.12 * ((a * 3.0).sin() * (z * 5.0).cos());
        let shell = Vec3::new(rx, ry, rx) * lump * b.rng.r(0.85, 1.0);
        let p = center + dir * shell;
        let up = (any_perp(dir) + b.rng.unit() * 0.5).normalize();
        let face = (dir + b.rng.unit() * 0.3).normalize();
        let size = b.rng.r(0.42, 0.55);
        let sway = b.sway(p, 0.01);
        b.card(p - up * size * 0.5, up, face, size, size, LeafCell::Oval, sway, b.rng.f(), 0.3, [1.0; 4]);
    }
}

fn juniper(b: &mut Builder, leaf: u32) {
    b.h = 1.2;
    b.flex = 0.8;
    b.leaf_col = leaf_tint(leaf);
    b.bark_col = srgb(0x6a5040);
    let spec = BranchSpec {
        levels: 2,
        children: [6, 3, 0, 0],
        start: [0.2, 0.3, 0.0, 0.0],
        angle: [35.0, 40.0, 0.0, 0.0],
        len: [0.45, 0.5, 0.0, 0.0],
        up: [0.12, 0.1, 0.0, 0.0],
        wiggle: [0.2, 0.3, 0.3, 0.0],
        rad_ratio: 0.6,
        shape: crown_flat,
        leaf: LeafSpec { cell: LeafCell::Scale, size: 0.8, aspect: 1.0, per_twig: 3, from: 0.2, curl: 0.2, face_up: 0.55, point_up: 0.25 },
        leaves_on_parent: true,
    };
    let n = 8 + (b.rng.f() * 3.0) as usize;
    for s in 0..n {
        let az = s as f32 / n as f32 * TAU + b.rng.r(-0.2, 0.2);
        let d = (Vec3::new(az.cos(), 0.0, az.sin()) + Vec3::Y * b.rng.r(0.3, 0.6)).normalize();
        let phase = b.rng.f();
        b.branch(&spec, 0, Vec3::new(0.0, -0.1, 0.0), d, b.rng.r(1.4, 2.0), 0.05, 0.0, phase, 0.0);
    }
}

fn oleander(b: &mut Builder, leaf: u32, v: usize) {
    b.h = 2.6;
    b.flex = 1.1;
    b.leaf_col = leaf_tint(leaf);
    b.bark_col = srgb(0x6d6450);
    let blossom = [[1.0, 1.0, 1.0, 1.0], [1.15, 1.05, 1.05, 1.0], [1.05, 0.85, 1.1, 1.0]][v];
    let stems = 7 + (b.rng.f() * 4.0) as usize;
    for s in 0..stems {
        let az = s as f32 / stems as f32 * TAU + b.rng.r(-0.3, 0.3);
        let d = (Vec3::new(az.cos(), 0.0, az.sin()) * b.rng.r(0.25, 0.5) + Vec3::Y).normalize();
        let len = b.rng.r(1.8, 2.6);
        let path = b.path(Vec3::new(0.0, -0.1, 0.0), d, len, 6, -0.05, 0.12);
        let r: Vec<f32> = (0..path.len()).map(|i| 0.028 * (1.0 - 0.6 * i as f32 / 6.0)).collect();
        let whip: Vec<f32> = (0..path.len()).map(|i| 0.05 * i as f32).collect();
        let phase = b.rng.f();
        if !b.far {
            b.tube(&path, &r, &whip, phase, TubeStyle::bark(5));
        }
        for c in 0..5 {
            let t = 0.3 + 0.7 * c as f32 / 5.0;
            let (p, tan, _) = lerp_path(&path, t);
            let face = (Vec3::new(p.x, 0.0, p.z).normalize_or(Vec3::X) + b.rng.unit() * 0.5).normalize();
            let size = b.rng.r(0.65, 0.85);
            let sway = b.sway(p, 0.05 * t * len);
            let (cell, tint) = if c >= 3 { (LeafCell::Flowers, mulc(leaf_tint_inv(b.leaf_col), blossom)) } else { (LeafCell::Lance, [1.0; 4]) };
            b.card(p - tan * size * 0.2, tan + Vec3::Y * 0.3, face, size, size, cell, sway, phase, 0.2, tint);
        }
    }
}

fn fern(b: &mut Builder, leaf: u32) {
    b.h = 1.0;
    b.flex = 1.6;
    b.leaf_col = leaf_tint(leaf);
    b.normal_bend = 0.3;
    let n = 14 + (b.rng.f() * 6.0) as usize;
    for k in 0..n {
        let az = k as f32 * 2.399_963 + b.rng.r(-0.2, 0.2);
        let out = Vec3::new(az.cos(), 0.0, az.sin());
        let elev = b.rng.r(50.0, 80.0).to_radians();
        let len = b.rng.r(1.0, 1.5);
        let mut d = (out * elev.cos() + Vec3::Y * elev.sin()).normalize();
        let segs = 8;
        let mut p = Vec3::new(0.0, 0.02, 0.0);
        let mut path = vec![p];
        for i in 0..segs {
            let t = i as f32 / segs as f32;
            d = (d - Vec3::Y * (0.15 + 0.3 * t)).normalize();
            p += d * len / segs as f32;
            path.push(p);
        }
        let side = vec![out.cross(Vec3::Y); path.len()];
        let widths: Vec<f32> = (0..path.len()).map(|i| 0.42 * (PI * (i as f32 / segs as f32).powf(0.7)).sin().max(0.05)).collect();
        let phase = b.rng.f();
        b.ribbon(&path, &side, &widths, 0.2, LeafCell::FernFrond, 0.02, phase, [1.0; 4]);
    }
}

fn tussock(b: &mut Builder, leaf: u32) {
    b.h = 0.7;
    b.flex = 2.0;
    b.leaf_col = leaf_tint(leaf);
    b.normal_bend = 0.25;
    let n = 18 + (b.rng.f() * 8.0) as usize;
    for k in 0..n {
        let az = k as f32 * 2.399_963 + b.rng.r(-0.3, 0.3);
        let out = Vec3::new(az.cos(), 0.0, az.sin());
        let elev = b.rng.r(55.0, 85.0).to_radians();
        let len = b.rng.r(0.7, 1.15);
        let mut d = (out * elev.cos() + Vec3::Y * elev.sin()).normalize();
        let segs = 5;
        let mut p = out * 0.05;
        let mut path = vec![p];
        for i in 0..segs {
            let t = i as f32 / segs as f32;
            d = (d - Vec3::Y * (0.08 + 0.2 * t)).normalize();
            p += d * len / segs as f32;
            path.push(p);
        }
        let side = vec![out.cross(Vec3::Y); path.len()];
        let widths = vec![0.32; path.len()];
        let phase = b.rng.f();
        let tint = if b.rng.f() < 0.3 { [1.25, 1.1, 0.75, 1.0] } else { [1.0; 4] };
        b.ribbon(&path, &side, &widths, 0.0, LeafCell::Grass, 0.0, phase, tint);
    }
}

fn flowering(b: &mut Builder, leaf: u32, v: usize) {
    // Blossom color per variant: pink, pale, magenta (tints the whole card,
    // so it stays subtle).
    let blossom = [[1.0, 1.0, 1.0, 1.0], [1.12, 1.15, 1.15, 1.0], [1.08, 0.8, 1.15, 1.0]][v];
    shrub(b, leaf, LeafCell::Flowers, blossom);
}

// ---------------------------------------------------------- caching --

fn kind_index(k: TreeKind) -> usize {
    k as usize
}

struct Cache {
    near: Vec<OnceLock<TreeModel>>,
    far: Vec<OnceLock<TreeModel>>,
}

fn cache() -> &'static Cache {
    static C: OnceLock<Cache> = OnceLock::new();
    C.get_or_init(|| {
        let n = TreeKind::ALL.len() * VARIANTS as usize;
        Cache { near: (0..n).map(|_| OnceLock::new()).collect(), far: (0..n).map(|_| OnceLock::new()).collect() }
    })
}

/// Full-detail model, grown once per process and shared.
pub fn model(kind: TreeKind, variant: u32) -> &'static TreeModel {
    let i = kind_index(kind) * VARIANTS as usize + (variant % VARIANTS) as usize;
    cache().near[i].get_or_init(|| build(kind, variant % VARIANTS, false))
}

/// Far LOD model (everything in the leaf mesh), grown once and shared.
pub fn far_model(kind: TreeKind, variant: u32) -> &'static TreeModel {
    let i = kind_index(kind) * VARIANTS as usize + (variant % VARIANTS) as usize;
    cache().far[i].get_or_init(|| build(kind, variant % VARIANTS, true))
}

/// Which seeded variation an instance uses (stable per position).
pub fn variant_for(t: &TreeInstance) -> u32 {
    let k = (t.pos.x.to_bits() as u64) << 32 ^ t.pos.z.to_bits() as u64 ^ (t.pos.y.to_bits() as u64).rotate_left(17);
    (crate::hash01(k, 0x5EED) * VARIANTS as f32) as u32 % VARIANTS
}

/// Instance rotation: model +Y to the local `up`, then the yaw.
pub fn instance_rotation(t: &TreeInstance, up: Vec3) -> Quat {
    // Natural lean: a few degrees off vertical in a per-tree direction
    // (taller, slimmer crowns lean less; shrubs sprawl more).
    let h = |salt: u64| crate::hash01((t.pos.x.to_bits() as u64) << 32 ^ t.pos.z.to_bits() as u64, salt);
    let max_lean = if t.kind.is_shrub() { 0.12 } else { 0.06 };
    let dir = h(0xA1) * std::f32::consts::TAU;
    let lean = Quat::from_axis_angle(Vec3::new(dir.sin(), 0.0, dir.cos()), h(0xA2) * max_lean * h(0xA2));
    Quat::from_rotation_arc(Vec3::Y, up) * lean * Quat::from_rotation_y(t.yaw)
}

/// Per-instance wind phase shift so neighbours don't sway in lockstep.
pub fn instance_phase(t: &TreeInstance) -> f32 {
    crate::hash01(t.pos.x.to_bits() as u64, t.pos.z.to_bits() as u64)
}

/// Merge the far-LOD models of `trees` into one mesh, positions relative
/// to `origin`.
pub fn bake_far(trees: &[TreeInstance], up: Vec3, origin: Vec3) -> WindMesh {
    let mut out = WindMesh::default();
    for t in trees {
        let m = far_model(t.kind, variant_for(t));
        out.append_transformed(&m.leaves, instance_rotation(t, up), t.scale, t.pos - origin, instance_phase(t));
    }
    out
}

/// Full-detail merged meshes for a small set of trees (lookdev, tests):
/// (rough bark, palm bark, leaves).
pub fn bake_near(trees: &[TreeInstance], up: Vec3, origin: Vec3) -> (WindMesh, WindMesh, WindMesh) {
    let (mut rough, mut palm, mut leaves) = (WindMesh::default(), WindMesh::default(), WindMesh::default());
    for t in trees {
        let m = model(t.kind, variant_for(t));
        let rot = instance_rotation(t, up);
        let ph = instance_phase(t);
        let bark = if m.bark_style == BarkStyle::Palm { &mut palm } else { &mut rough };
        bark.append_transformed(&m.bark, rot, t.scale, t.pos - origin, ph);
        leaves.append_transformed(&m.leaves, rot, t.scale, t.pos - origin, ph);
    }
    (rough, palm, leaves)
}

// ------------------------------------------------------------ tile bake --

/// A patch of a tile's trees: the far-LOD cards merged into one mesh, plus
/// the instances so the game can swap in full-detail trees up close.
#[derive(Debug, Clone, Default)]
pub struct TreeChunk {
    /// Chunk center, relative to the tile anchor.
    pub center: Vec3,
    /// Far-LOD mesh (leaf cards), relative to `center`.
    pub far: WindMesh,
    /// Instances in this chunk (positions relative to the tile anchor).
    pub instances: Vec<TreeInstance>,
}

/// All trees of a tile.
#[derive(Debug, Clone, Default)]
pub struct TileTrees {
    pub up: Vec3,
    /// Fine tiles only: card chunks and instances for near detail.
    pub chunks: Vec<TreeChunk>,
    /// Every tree of the tile as an impostor quad (tile mesh space).
    pub impostors: impostor::ImpostorMesh,
}

impl TileTrees {
    pub fn count(&self) -> usize {
        self.chunks.iter().map(|c| c.instances.len()).sum::<usize>().max(self.impostors.len())
    }
    pub fn triangle_count(&self) -> usize {
        self.chunks.iter().map(|c| c.far.triangle_count()).sum::<usize>() + self.impostors.len() * 2
    }
}

/// Group `trees` into a `grid` x `grid` set of chunks over a tile of
/// `width` meters (local x/z), baking each chunk's far-LOD mesh.
pub fn bake_tile(trees: Vec<TreeInstance>, up: Vec3, width: f32, grid: usize) -> TileTrees {
    let grid = grid.max(1);
    let mut cells: Vec<Vec<TreeInstance>> = vec![Vec::new(); grid * grid];
    let cell = (width / grid as f32).max(1.0);
    let half = width * 0.5;
    for t in trees {
        let i = (((t.pos.x + half) / cell).floor() as i64).clamp(0, grid as i64 - 1) as usize;
        let j = (((t.pos.z + half) / cell).floor() as i64).clamp(0, grid as i64 - 1) as usize;
        cells[j * grid + i].push(t);
    }
    let chunks = cells
        .into_iter()
        .filter(|c| !c.is_empty())
        .map(|instances| {
            let center = instances.iter().map(|t| t.pos).sum::<Vec3>() / instances.len() as f32;
            let far = bake_far(&instances, up, center);
            TreeChunk { center, far, instances }
        })
        .collect();
    TileTrees { up, chunks, impostors: Default::default() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_bake_chunks_and_keeps_every_tree() {
        let trees: Vec<TreeInstance> = (0..200)
            .map(|i| TreeInstance {
                kind: TreeKind::ALL[i % TreeKind::ALL.len()],
                pos: Vec3::new((i % 20) as f32 * 50.0 - 500.0, 10.0, (i / 20) as f32 * 100.0 - 500.0),
                scale: 1.0,
                yaw: i as f32,
                tint: [1.0; 4],
            })
            .collect();
        let t = bake_tile(trees, Vec3::Y, 1000.0, 4);
        assert_eq!(t.count(), 200);
        assert_eq!(t.chunks.len(), 16);
        assert!(t.triangle_count() > 200 * 10 && t.triangle_count() < 200 * 400, "{}", t.triangle_count());
    }

    #[test]
    fn wind_packing_roundtrips() {
        let w = pack_wind(1.234, 0.05, 0.37);
        let (s, f, p) = unpack_wind(w);
        assert!((s - 1.234).abs() < 1e-4 && (f - 0.05).abs() < 1e-6);
        assert!((p - 0.359375).abs() < 1e-6, "{p}");
    }

    #[test]
    fn every_species_grows_within_budget() {
        for &k in TreeKind::ALL {
            for v in 0..VARIANTS {
                let m = build(k, v, false);
                let f = build(k, v, true);
                let near = m.bark.triangle_count() + m.leaves.triangle_count();
                let far = f.bark.triangle_count() + f.leaves.triangle_count();
                assert!(near > 100 && near < 20_000, "{k:?}/{v}: near {near}");
                assert!(far > 6 && far < 700, "{k:?}/{v}: far {far}");
                assert!(far * 3 < near, "{k:?}/{v}: far {far} vs near {near}");
                assert!(f.bark.is_empty(), "far model is leaf-material only");
                assert_eq!(m.leaves.wind.len(), m.leaves.mesh.positions.len());
                assert_eq!(m.bark.wind.len(), m.bark.mesh.positions.len());
                assert!(m.height > 0.4 && m.height < 25.0, "{k:?}: h {}", m.height);
                for p in m.bark.mesh.positions.iter().chain(m.leaves.mesh.positions.iter()) {
                    assert!(p.iter().all(|c| c.is_finite()));
                }
                for n in m.leaves.mesh.normals.iter() {
                    assert!((Vec3::from(*n).length() - 1.0).abs() < 1e-3);
                }
            }
        }
    }

    /// `cargo test -p ed_world budgets -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn print_budgets() {
        for &k in TreeKind::ALL {
            let m = build(k, 0, false);
            let f = build(k, 0, true);
            eprintln!(
                "{k:16?} near bark {:5} leaves {:5}  far {:4}  h {:5.1} crown {:4.1}",
                m.bark.triangle_count(),
                m.leaves.triangle_count(),
                f.leaves.triangle_count(),
                m.height,
                m.crown_radius
            );
        }
    }

    #[test]
    fn bark_faces_outward() {
        // Triangle winding must agree with the outward vertex normals, or
        // back-face culling shows the inside of the trunk.
        for &k in [TreeKind::Broadleaf, TreeKind::Palm, TreeKind::Baobab].iter() {
            let m = model(k, 0);
            let (p, n) = (&m.bark.mesh.positions, &m.bark.mesh.normals);
            let mut agree = 0i64;
            for t in m.bark.mesh.indices.chunks_exact(3) {
                let [a, b, c] = [t[0], t[1], t[2]].map(|i| Vec3::from(p[i as usize]));
                let face = (b - a).cross(c - a);
                let vn = Vec3::from(n[t[0] as usize]) + Vec3::from(n[t[1] as usize]) + Vec3::from(n[t[2] as usize]);
                if face.length_squared() > 1e-12 {
                    agree += if face.dot(vn) > 0.0 { 1 } else { -1 };
                }
            }
            assert!(agree > 0, "{k:?}: bark triangles face inward ({agree})");
        }
    }

    #[test]
    fn deterministic() {
        let a = build(TreeKind::Broadleaf, 1, false);
        let b = build(TreeKind::Broadleaf, 1, false);
        assert_eq!(a.leaves.mesh.positions, b.leaves.mesh.positions);
        let c = build(TreeKind::Broadleaf, 2, false);
        assert_ne!(a.leaves.mesh.positions.len(), 0);
        assert_ne!(a.leaves.mesh.positions, c.leaves.mesh.positions);
    }

    #[test]
    fn sway_grows_with_height() {
        let m = model(TreeKind::Broadleaf, 0);
        let (mut lo, mut hi) = (0.0f32, 0.0f32);
        for (p, w) in m.bark.mesh.positions.iter().zip(&m.bark.wind) {
            let s = unpack_wind(*w).0;
            if p[1] < 1.0 {
                lo = lo.max(s);
            }
            if p[1] > 8.0 {
                hi = hi.max(s);
            }
        }
        assert!(lo < 0.05 && hi > 0.2, "base {lo} top {hi}");
    }
}
