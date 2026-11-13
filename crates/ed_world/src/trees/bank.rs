//! The tree bank (trees G4, first half of G6): per stand species x variant x age class, one
//! renderer mesh with a 4-level LOD chain, deterministic, built once and cached by the caller.
//!
//! * LOD 0 (projected height over [`T0_PX`]): the full generated tree, bark tubes and leaf
//!   cards. LOD 1 (down to [`T1_PX`]): the generator's far model (thick limbs only, fewer and
//!   larger cards). LOD 2 (down to the hand-off): a crown shell (star-shaped hull of the far
//!   cards, plus a trunk prism), opaque. LOD 3: empty; the far forest rung (`EW_TREES_FAR`)
//!   draws the crowns below [`HANDOFF_PX`] (its lift ramp starts at the same reference size).
//! * Vertex data rides in the renderer's per-vertex `uvs` words as bits (`MaterialModel::
//!   Foliage`): word 0 = atlas UV (unorm16 x2), word 1 = [`pack_aux`] (value, hue, wind sway,
//!   flutter, phase, card / bark flags, branch group). `common.slang` decodes the same layout.
//! * Leaf colour comes from ONE table: [`leaf_albedo`] scales the far rung's class albedo
//!   ([`CROWN_ALBEDO`], mirrored in `ed_render::terrain`) by the stand species' own colour, so
//!   near and far crowns match by construction.
//! * Per-instance variety ([`instance_params`]) is a pure function of `StandTree.seed`.

use super::*;
use crate::stand::{STAND_SPECIES, StandTree};

/// Variants per species (conifers: [`CONIFER_VARIANTS`]).
pub const BANK_VARIANTS: u32 = 8;
pub const CONIFER_VARIANTS: u32 = 6;
pub const AGE_CLASSES: u8 = 3;
/// The far rung's reference tree (m), `ed_render::terrain::TREE_REF_M`.
pub const TREE_REF_M: f32 = 15.0;
/// Hand-off to the far rung: the reference tree's projected height (px) where the near tree
/// leaves; `ed_render::terrain::LIFT_NONE_PX` (the lift ramp starts here).
pub const HANDOFF_PX: f32 = 12.0;
/// LOD 0 -> 1 and 1 -> 2 switch heights (px of the tree's projected height).
pub const T0_PX: f32 = 250.0;
pub const T1_PX: f32 = 40.0;
/// Trees G5 (impostors on, `EW_TREES_IMPOSTOR`): the ONE table of tier hand-offs, in px of
/// projected height. The renderer gets it as `ed_render::impostor::ImpostorBands` (cull, resolve
/// and the far rung all read it from `FrameData.imp`).
///
/// * T1 -> T2: the bank mesh's empty level sits at [`T1_PX`] (it leaves at about 0.75 x that,
///   the LOD hysteresis, and comes back at it); the impostor is whole under `IMP_IN_PX.0` of the
///   tree's own height and gone over `IMP_IN_PX.1`, so it is whole wherever the mesh can leave.
/// * T2 -> T3: each tree leaves at its own hashed size in `IMP_OUT_PX` (reference tree px);
///   the far rung's canopy paint rises linearly over the same band (`PAINT_PX`), so the covered
///   area stays constant, and the crown lift (`LIFT_PX`) ramps in only once the impostors are
///   gone.
pub const IMP_IN_PX: (f32, f32) = (42.0, 48.0);
pub const IMP_OUT_PX: (f32, f32) = (6.0, 8.0);
pub const PAINT_PX: (f32, f32) = (8.0, 6.0);
pub const LIFT_PX: (f32, f32) = (6.0, 3.5);
/// Mesh instances exist only for nodes where some tree can project over this (px; under it
/// every tree is an impostor whatever the LOD hysteresis).
pub const MESH_KEEP_PX: f32 = 28.0;
/// Sun shadow cascades from this one on take every tree's impostor (bank meshes cast only in
/// the nearer cascades, where their detail shows).
pub const IMP_SHADOW_CASCADE: u32 = 2;
/// Impostors are drawn this many pixels' worth of distance nearer the viewer (along their own
/// view ray, so the picture does not move): trees stand on the finest resident ground but the
/// terrain draws coarser with distance (16 px selection error), and on convex slopes it would
/// bury them.
pub const IMP_GROUND_PULL_PX: f32 = 12.0;

/// LOD errors of the impostor chain (no shell): full, simplified, empty at `t1_px` (the
/// table's [`T1_PX`] unless the caller moves the T1 -> T2 hand-off; the impostor bands and
/// the mesh-keep range scale with it).
pub fn lod_errors_impostor(h: f32, t1_px: f32) -> [f32; 3] {
    let e2 = h / t1_px;
    let e1 = (h / T0_PX).min(e2 * 0.8);
    [0.0, e1, e2]
}

/// Card budget of the simplified level (T1).
pub const T1_CARDS: usize = 150;
/// Far-rung crown albedo per class (broadleaf, conifer), linear;
/// `ed_render::terrain::CROWN_ALBEDO` (a test in `ew` pins the two).
pub const CROWN_ALBEDO: [[f32; 3]; 2] = [[0.050, 0.110, 0.024], [0.026, 0.064, 0.020]];

fn lin(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}
fn hex_lin(hex: u32) -> [f32; 3] {
    [lin(((hex >> 16) & 255) as f32 / 255.0), lin(((hex >> 8) & 255) as f32 / 255.0), lin((hex & 255) as f32 / 255.0)]
}

/// Mean linear leaf colour of the class's trees (shrubs excluded).
fn class_mean(conifer: bool) -> [f32; 3] {
    let mut s = [0.0f32; 3];
    let mut n = 0.0;
    for sp in STAND_SPECIES.iter().filter(|s| s.conifer == conifer && !s.shrub) {
        let c = hex_lin(sp.leaf);
        for k in 0..3 {
            s[k] += c[k];
        }
        n += 1.0;
    }
    s.map(|x| x / n)
}

/// Leaf albedo (linear) of stand species `species`: the far rung's class albedo times the
/// species' colour relative to its class mean (so the class means are exactly
/// [`CROWN_ALBEDO`]).
pub fn leaf_albedo(species: usize) -> [f32; 3] {
    let sp = &STAND_SPECIES[species];
    let c = hex_lin(sp.leaf);
    let m = class_mean(sp.conifer);
    let a = CROWN_ALBEDO[sp.conifer as usize];
    [0, 1, 2].map(|k| a[k] * (c[k] / m[k].max(1e-6)).clamp(0.2, 5.0))
}

pub fn variants(species: usize) -> u32 {
    if STAND_SPECIES[species].conifer { CONIFER_VARIANTS } else { BANK_VARIANTS }
}

/// Bank mesh height (m) of an age class: instances scale it to their own height, so the bank
/// is built near the heights the stand grows (scale stays close to 1).
pub fn target_height(species: usize, age: u8) -> f32 {
    let sp = &STAND_SPECIES[species];
    let canopy = (0.5 * (sp.canopy_h.0 + sp.canopy_h.1)).clamp(3.0, 32.0).min(sp.max_h);
    match age {
        0 => (canopy * 0.18).max(1.2),
        1 => canopy * 0.5,
        _ => canopy,
    }
}

// ---------------------------------------------------------------------------------------------
// Shapes: which generator grows a stand species
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Spruce,
    Fir,
    Larch,
    Hemlock,
    StonePine,
    MountainPine,
    Pine,
    Oak,
    Maple,
    Birch,
    TulipPoplar,
    /// Anything else: the alpha generator of the species' `TreeKind`.
    Kind(TreeKind),
}

pub fn shape(species: usize) -> Shape {
    let sp = &STAND_SPECIES[species];
    match sp.name {
        "Norway spruce" | "Spruce (generic)" => Shape::Spruce,
        "Silver fir" => Shape::Fir,
        "European larch" => Shape::Larch,
        "Eastern hemlock" => Shape::Hemlock,
        "Swiss stone pine" => Shape::StonePine,
        "Mountain pine" => Shape::MountainPine,
        "Scots pine" | "Eastern white pine" => Shape::Pine,
        "Tulip poplar" => Shape::TulipPoplar,
        _ => match sp.kind {
            TreeKind::Broadleaf => Shape::Oak,
            TreeKind::Maple => Shape::Maple,
            TreeKind::Birch => Shape::Birch,
            k => Shape::Kind(k),
        },
    }
}

/// Whorled conifer parameters (spruce, fir, larch, hemlock).
struct Whorl {
    /// Base branch length / tree height.
    width_k: f32,
    step: f32,
    droop: f32,
    /// Card density (larch: airy).
    density: f32,
    card: f32,
    cell: LeafCell,
    hang: bool,
    fir: bool,
    bark: u32,
    /// Live crown starts this far up (fraction of height); dead stubs below.
    crown_base: f32,
    trunk_r: f32,
    /// Leader nods over (hemlock).
    nod: f32,
}

fn whorl_conifer(b: &mut Builder, leaf: u32, p: &Whorl) {
    let h = b.rng.r(17.0, 21.0);
    b.h = h;
    b.leaf_col = leaf_tint(leaf);
    b.bark_col = srgb(p.bark);
    b.normal_bend = 0.6;
    let lean = Vec3::new(b.rng.r(-0.02, 0.02), 0.0, b.rng.r(-0.02, 0.02));
    let mut trunk = b.path(Vec3::new(0.0, -0.4, 0.0), (Vec3::Y + lean).normalize(), h + 0.4, 16, 0.0, 0.012);
    if p.nod > 0.0 {
        let n = trunk.len();
        for (i, q) in trunk.iter_mut().enumerate().skip(n - 4) {
            q.x += p.nod * ((i + 4 - n) as f32 / 3.0).powi(2);
        }
    }
    let radii: Vec<f32> = (0..trunk.len()).map(|i| p.trunk_r * (1.0 - i as f32 / 16.0).max(0.03).powf(0.9)).collect();
    let whip = vec![0.0; trunk.len()];
    b.tube(&trunk, &radii, &whip, 0.0, TubeStyle { flare: 0.45, ..TubeStyle::bark(10) });
    let width = p.width_k * h;
    let mut y = 1.0;
    while y < h - 0.6 {
        let t = y / h;
        let (tp, _, _) = lerp_path(&trunk, (y + 0.4) / (h + 0.4));
        let n = 4 + (b.rng.f() * 4.0) as usize;
        let az0 = b.rng.f() * TAU;
        let live = t >= p.crown_base * b.rng.r(0.8, 1.2);
        for k in 0..n {
            let az = az0 + k as f32 / n as f32 * TAU + b.rng.r(-0.35, 0.35);
            let out = Vec3::new(az.cos(), 0.0, az.sin());
            if !live {
                // Dead lower branches: short bare stubs (near model only), some broken off.
                if !b.far && b.rng.f() < 0.6 {
                    let len = b.rng.r(0.4, 1.6) * (h / 20.0);
                    let path = b.path(tp, (out - Vec3::Y * 0.15).normalize(), len, 2, -0.1, 0.1);
                    let r = 0.03 * (1.0 - t) + 0.012;
                    b.tube(&path, &[r, r * 0.6, r * 0.3], &[0.0; 3], 0.0, TubeStyle::bark(3));
                }
                continue;
            }
            // Crown silhouette: a narrow spire, widest a third up the live crown.
            let ct = ((t - p.crown_base) / (1.0 - p.crown_base)).clamp(0.0, 1.0);
            let prof = (1.0 - ct).powf(0.85) * (0.75 + 0.25 * (ct * 3.0).min(1.0));
            let len = (width * prof + 0.3) * b.rng.r(0.8, 1.2);
            let droop = p.droop * (0.6 + 0.4 * (1.0 - t));
            let dir = (out + Vec3::Y * if p.fir { 0.05 } else { -0.03 }).normalize();
            let path = b.path(tp, dir, len, 5, droop, 0.09);
            let path: Vec<Vec3> = path.iter().enumerate().map(|(i, q)| *q + Vec3::Y * 0.07 * len * (i as f32 / 5.0).powi(3)).collect();
            let radii: Vec<f32> = (0..path.len()).map(|i| (0.06 * (1.0 - t) + 0.018) * (1.0 - 0.8 * i as f32 / 5.0)).collect();
            let whip: Vec<f32> = (0..path.len()).map(|i| 0.04 * len * i as f32 / 5.0).collect();
            let phase = b.rng.f();
            if !b.far {
                let pick = |v: &[f32]| -> Vec<f32> { v.iter().step_by(2).copied().collect() };
                let p2: Vec<Vec3> = path.iter().step_by(2).copied().collect();
                b.tube(&p2, &pick(&radii), &pick(&whip), phase, TubeStyle::bark(3));
            }
            let cards = ((len / (0.7 * p.card)).ceil() as usize).max(1);
            for c in 0..cards {
                if b.rng.f() > p.density {
                    continue;
                }
                let ct = (c as f32 + 0.3) / cards as f32;
                let (q, tan, _) = lerp_path(&path, ct * 0.9);
                let size = (0.7 + 0.45 * (1.0 - ct)) * p.card;
                let sway = b.sway(q, 0.04 * len * ct);
                let face = (Vec3::Y + b.rng.unit() * 0.4).normalize();
                b.card(q - tan * size * 0.1, tan, face, size * 0.95, size, p.cell, sway, phase, if p.fir { 0.1 } else { -0.3 }, [1.0; 4]);
                let side = tan.cross(Vec3::Y).normalize_or(Vec3::X);
                if p.fir {
                    let face2 = (side * 0.85 + Vec3::Y * 0.5).normalize();
                    b.card(q - tan * size * 0.1, tan, face2, size * 0.9, size, p.cell, sway, phase + 0.03, 0.15, [0.95, 0.97, 0.97, 1.0]);
                }
                if p.hang && c > 0 {
                    let hang = (tan * 0.35 - Vec3::Y).normalize();
                    b.card(q + Vec3::Y * 0.08, hang, side, size * 0.75, size * 0.85, p.cell, sway, phase + 0.05, 0.2, [0.9, 0.94, 0.94, 1.0]);
                }
            }
        }
        y += p.step * b.rng.r(0.8, 1.25) * (1.0 - 0.3 * t);
    }
    // Leader.
    let top = *trunk.last().unwrap();
    for k in 0..3 {
        let a = k as f32 / 3.0 * PI;
        let face = Vec3::new(a.cos(), 0.0, a.sin());
        let sway = b.sway(top, 0.0);
        b.card(top - Vec3::Y * 1.1, Vec3::Y, face, 0.6 * p.card, 1.5 * p.card, p.cell, sway, 0.0, 0.0, [1.0; 4]);
    }
}

fn stone_pine(b: &mut Builder, leaf: u32, mountain: bool) {
    b.h = if mountain { 5.0 } else { 14.0 };
    b.leaf_col = leaf_tint(leaf);
    b.bark_col = srgb(0x5d5048);
    let spec = BranchSpec {
        levels: 3,
        children: if mountain { [9, 5, 3, 0] } else { [16, 5, 3, 0] },
        start: [if mountain { 0.05 } else { 0.16 }, 0.2, 0.25, 0.0],
        angle: [62.0, 40.0, 35.0, 0.0],
        len: [if mountain { 0.6 } else { 0.32 }, 0.55, 0.5, 0.0],
        up: [0.3, 0.4, 0.45, 0.0],
        wiggle: [0.1, 0.4, 0.4, 0.4],
        rad_ratio: 0.5,
        shape: crown_ovoid,
        leaf: LeafSpec { cell: LeafCell::PineTuft, size: 1.5, aspect: 1.0, per_twig: 5, from: 0.35, curl: 0.15, face_up: 0.5, point_up: 0.65 },
        leaves_on_parent: true,
    };
    let tl = if mountain { b.rng.r(3.0, 4.0) } else { b.rng.r(11.0, 13.5) };
    let lean = Vec3::new(b.rng.r(-0.06, 0.06), 0.0, b.rng.r(-0.06, 0.06)) * if mountain { 4.0 } else { 1.0 };
    b.grow_tree(&spec, tl, if mountain { 0.14 } else { 0.38 }, lean, 0.35);
}

fn tulip_poplar(b: &mut Builder, leaf: u32) {
    b.h = 24.0;
    b.leaf_col = leaf_tint(leaf);
    b.bark_col = srgb(0x7a7466);
    let spec = BranchSpec {
        levels: 3,
        children: [9, 4, 3, 0],
        start: [0.55, 0.2, 0.25, 0.0],
        angle: [38.0, 45.0, 40.0, 0.0],
        len: [0.36, 0.55, 0.5, 0.0],
        up: [0.4, 0.12, 0.12, 0.0],
        wiggle: [0.08, 0.3, 0.35, 0.3],
        rad_ratio: 0.55,
        shape: crown_ovoid,
        leaf: LeafSpec { cell: LeafCell::Palmate, size: 2.0, aspect: 1.0, per_twig: 5, from: 0.3, curl: 0.2, face_up: 0.35, point_up: 0.35 },
        leaves_on_parent: true,
    };
    let tl = b.rng.r(21.0, 24.0);
    b.grow_tree(&spec, tl, 0.45, Vec3::new(b.rng.r(-0.03, 0.03), 0.0, b.rng.r(-0.03, 0.03)), 0.35);
}

/// Grows one tree of `shape`; returns the model and its card count (far-model budget).
fn grow(shape: Shape, kind: TreeKind, seed: u64, leaf: u32, far: bool, keep: f32) -> (TreeModel, u64) {
    let mut b = Builder::new(seed, far, keep);
    let w = |width_k, step, droop, density, card, cell, hang, fir, bark, crown_base, trunk_r, nod| Whorl { width_k, step, droop, density, card, cell, hang, fir, bark, crown_base, trunk_r, nod };
    match shape {
        Shape::Spruce => whorl_conifer(&mut b, leaf, &w(0.16, 0.5, -0.32, 1.0, 1.0, LeafCell::NeedleSpray, true, false, 0x5e4a3c, 0.12, 0.42, 0.0)),
        Shape::Fir => whorl_conifer(&mut b, leaf, &w(0.15, 0.55, -0.06, 1.0, 0.95, LeafCell::NeedleSpray, false, true, 0x7a7266, 0.2, 0.4, 0.0)),
        Shape::Larch => whorl_conifer(&mut b, leaf, &w(0.17, 0.7, -0.12, 0.5, 0.75, LeafCell::PineTuft, false, false, 0x6b5040, 0.3, 0.4, 0.0)),
        Shape::Hemlock => whorl_conifer(&mut b, leaf, &w(0.22, 0.5, -0.4, 1.0, 0.9, LeafCell::NeedleSpray, true, false, 0x5a4636, 0.15, 0.45, 0.9)),
        Shape::StonePine => stone_pine(&mut b, leaf, false),
        Shape::MountainPine => stone_pine(&mut b, leaf, true),
        Shape::Pine => pine(&mut b, leaf),
        Shape::Oak => oak(&mut b, leaf),
        Shape::Maple => maple(&mut b, leaf),
        Shape::Birch => birch(&mut b, leaf),
        Shape::TulipPoplar => tulip_poplar(&mut b, leaf),
        Shape::Kind(_) => {
            let _ = kind;
            unreachable!("Kind shapes use build_inner")
        }
    }
    let cards = b.card_index;
    (b.finish(), cards)
}

// ---------------------------------------------------------------------------------------------
// Vertex words
// ---------------------------------------------------------------------------------------------

pub const AUX_CARD: u32 = 1 << 27;
pub const AUX_BARK: u32 = 1 << 28;
/// Branch groups (per-instance drop mask bits): 0..3 lower crown quadrants, 4..7 upper.
pub const GROUPS: u32 = 8;

/// Packs a vertex's aux word: `value` 0..2 (7 bits), `hue` -1..1 (4 bits: + bluer, - warmer),
/// `sway` m at unit wind 0..4 (7 bits, sqrt-coded), `flutter` 0..1 (3 bits), `phase` 0..1
/// (6 bits), card and bark flags, branch `group` (3 bits). Mirrored by `common.slang`.
#[allow(clippy::too_many_arguments)]
pub fn pack_aux(value: f32, hue: f32, sway: f32, flutter: f32, phase: f32, card: bool, bark: bool, group: u32) -> u32 {
    let q = |x: f32, n: u32| (x.clamp(0.0, 1.0) * n as f32).round() as u32;
    q(value / 2.0, 127)
        | q((hue + 1.0) * 0.5, 15) << 7
        | q((sway / 4.0).max(0.0).sqrt(), 127) << 11
        | q(flutter, 7) << 18
        | ((phase.rem_euclid(1.0) * 64.0) as u32).min(63) << 21
        | if card { AUX_CARD } else { 0 }
        | if bark { AUX_BARK } else { 0 }
        | (group & 7) << 29
}

/// (value, hue, sway, flutter, phase) of an aux word.
pub fn unpack_aux(w: u32) -> (f32, f32, f32, f32, f32) {
    let value = (w & 127) as f32 / 127.0 * 2.0;
    let hue = ((w >> 7) & 15) as f32 / 15.0 * 2.0 - 1.0;
    let s = ((w >> 11) & 127) as f32 / 127.0;
    (value, hue, s * s * 4.0, ((w >> 18) & 7) as f32 / 7.0, ((w >> 21) & 63) as f32 / 64.0)
}

pub fn pack_uv(uv: [f32; 2]) -> u32 {
    let q = |x: f32| (x.clamp(0.0, 1.0) * 65535.0).round() as u32;
    q(uv[0]) | q(uv[1]) << 16
}

// ---------------------------------------------------------------------------------------------
// Bank meshes
// ---------------------------------------------------------------------------------------------

/// One bank tree: a vertex array shared by the LOD index lists (renderer `MeshAsset` layout).
#[derive(Clone, Debug, Default)]
pub struct BankMesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    /// [`pack_uv`], [`pack_aux`] per vertex.
    pub words: Vec<[u32; 2]>,
    /// Index list and geometric error (m) per LOD; the last is empty (hand-off).
    pub lods: Vec<(Vec<u32>, f32)>,
    pub height: f32,
    pub crown_radius: f32,
    pub leaf_albedo: [f32; 3],
    pub bark_albedo: [f32; 3],
}

impl BankMesh {
    pub fn triangles(&self, lod: usize) -> usize {
        self.lods[lod].0.len() / 3
    }
    pub fn bytes(&self) -> usize {
        self.positions.len() * (16 + 8) + self.lods.iter().map(|l| l.0.len() * 4).sum::<usize>()
    }
}

fn lum(c: [f32; 4]) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}
fn warmth(c: [f32; 4]) -> f32 {
    (c[0] - c[2]) / c[1].max(1e-4)
}

fn in_solid_cell(uv: [f32; 2]) -> bool {
    uv[0] < 0.25 && uv[1] > 0.75
}

/// Card groups: connected components of the card triangles, binned by their centroid
/// (azimuth quadrant, lower or upper half of the crown).
fn card_groups(m: &MeshData, card: &[bool], mid_y: f32) -> Vec<u32> {
    let n = m.positions.len();
    let mut parent: Vec<u32> = (0..n as u32).collect();
    fn find(p: &mut [u32], mut x: u32) -> u32 {
        while p[x as usize] != x {
            p[x as usize] = p[p[x as usize] as usize];
            x = p[x as usize];
        }
        x
    }
    for t in m.indices.chunks_exact(3) {
        let a = find(&mut parent, t[0]);
        for &v in &t[1..] {
            let r = find(&mut parent, v);
            if r != a {
                parent[r as usize] = a;
            }
        }
    }
    let mut sum: Vec<([f32; 3], f32)> = vec![([0.0; 3], 0.0); n];
    for i in 0..n {
        let r = find(&mut parent, i as u32) as usize;
        for k in 0..3 {
            sum[r].0[k] += m.positions[i][k];
        }
        sum[r].1 += 1.0;
    }
    (0..n)
        .map(|i| {
            if !card[i] {
                return 0;
            }
            let r = find(&mut parent, i as u32) as usize;
            let c = sum[r].0.map(|x| x / sum[r].1);
            let q = ((c[2].atan2(c[0]) + PI) / TAU * 4.0) as u32 % 4;
            q + if c[1] > mid_y { 4 } else { 0 }
        })
        .collect()
}

struct Lod {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    words: Vec<[u32; 2]>,
    indices: Vec<u32>,
}

/// Converts a generated model (scaled by `s`) into vertex words. `leaf_ref`/`bark_ref`: the
/// reference colours value and hue are measured against.
fn convert(model: &TreeModel, s: f32, leaf_ref: [f32; 4], bark_ref: [f32; 4], mid_y: f32) -> Lod {
    let mut out = Lod { positions: Vec::new(), normals: Vec::new(), words: Vec::new(), indices: Vec::new() };
    for (mesh, is_bark) in [(&model.bark, true), (&model.leaves, false)] {
        let m = &mesh.mesh;
        let base = out.positions.len() as u32;
        let solid: Vec<bool> = m.uvs.iter().map(|uv| is_bark || in_solid_cell(*uv)).collect();
        // Solid cells in the leaf mesh: far-model limbs (bark coloured) unless greenish.
        let bark_v: Vec<bool> = (0..m.positions.len())
            .map(|i| {
                is_bark || (solid[i] && {
                    let c = m.colors[i];
                    let dl = (warmth(c) - warmth(leaf_ref)).abs();
                    let db = (warmth(c) - warmth(bark_ref)).abs();
                    db < dl
                })
            })
            .collect();
        let card: Vec<bool> = solid.iter().map(|s| !s).collect();
        let groups = if is_bark { vec![0; m.positions.len()] } else { card_groups(m, &card, mid_y / s) };
        for i in 0..m.positions.len() {
            let p = m.positions[i];
            out.positions.push([p[0] * s, p[1] * s, p[2] * s]);
            out.normals.push(m.normals[i]);
            let c = m.colors[i];
            let r = if bark_v[i] { bark_ref } else { leaf_ref };
            let value = lum(c) / lum(r).max(1e-5);
            let hue = ((warmth(r) - warmth(c)) * 2.5).clamp(-1.0, 1.0);
            let (sway, flutter, phase) = unpack_wind(mesh.wind[i]);
            let uv = if is_bark { atlas_uv(LeafCell::Solid, 0.5, 0.5) } else { m.uvs[i] };
            out.words.push([pack_uv(uv), pack_aux(value, hue, sway * s, flutter, phase, card[i], bark_v[i], groups[i])]);
        }
        out.indices.extend(m.indices.iter().map(|&i| i + base));
    }
    out
}

/// Crown shell (LOD 2, drawn at 12..40 px): a lathe of the full model's leaf cards (the
/// simplified level's cards are enlarged and would swell it). The crown
/// is cut into `NS` height slabs; each ring vertex sits at the cards' radial extent in its
/// slab and azimuth sector (shrunk a little: card corners stick out of the leaf mass), so a
/// spruce keeps its cone and a beech its dome. Apex at the crown top, a flat-ish underside,
/// and a 3-sided trunk prism up to the crown base.
fn shell(near: &Lod, height: f32, trunk_r: f32) -> Lod {
    const NA: usize = 6;
    const NS: usize = 4;
    let cards: Vec<Vec3> = near.positions.iter().zip(&near.words).filter(|(_, w)| w[1] & AUX_CARD != 0).map(|(p, _)| Vec3::from(*p)).collect();
    let (y0, y1) = if cards.is_empty() {
        (height * 0.3, height)
    } else {
        cards.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| (lo.min(p.y), hi.max(p.y)))
    };
    let span = (y1 - y0).max(0.3);
    // Ring heights: slab centres, the top ring a little under the apex.
    let ring_y = |k: usize| y0 + span * (k as f32 + 0.5) / NS as f32 * 0.92;
    let mut r = vec![0.0f32; NA * NS];
    for p in &cards {
        let k = (((p.y - y0) / span * NS as f32) as usize).min(NS - 1);
        let az = ((p.z.atan2(p.x) + PI) / TAU * NA as f32) as usize % NA;
        let d = (p.x * p.x + p.z * p.z).sqrt();
        r[k * NA + az] = r[k * NA + az].max(d);
    }
    // Empty sectors take their slab's mean; then a light smoothing around each ring.
    for k in 0..NS {
        let row = &r[k * NA..(k + 1) * NA];
        let n = row.iter().filter(|&&x| x > 0.0).count();
        let mean = if n > 0 { row.iter().sum::<f32>() / n as f32 } else { 0.3 };
        let row0: Vec<f32> = row.iter().map(|&x| if x > 0.0 { x } else { mean }).collect();
        for az in 0..NA {
            r[k * NA + az] = ((row0[az] * 2.0 + row0[(az + 1) % NA] + row0[(az + NA - 1) % NA]) / 4.0 * 0.85).max(0.15);
        }
    }
    let mut out = Lod { positions: Vec::new(), normals: Vec::new(), words: Vec::new(), indices: Vec::new() };
    let leaf_word = |value: f32, y: f32| pack_aux(value, 0.0, 0.035 * height * (y / height).clamp(0.0, 1.5).powf(1.8), 0.0, 0.0, false, false, 0);
    let uv = pack_uv(atlas_uv(LeafCell::Solid, 0.5, 0.5));
    // Underside centre, rings bottom to top, apex.
    let bottom = Vec3::Y * (y0 + span * 0.04);
    out.positions.push(bottom.to_array());
    out.normals.push([0.0, -1.0, 0.0]);
    out.words.push([uv, leaf_word(0.45, bottom.y)]);
    for k in 0..NS {
        let y = ring_y(k);
        // Slope of the profile here (for the normal): radius change per height.
        let rk = |k: usize| r[k * NA..(k + 1) * NA].iter().sum::<f32>() / NA as f32;
        let dr = if NS > 1 { (rk((k + 1).min(NS - 1)) - rk(k.saturating_sub(1))) / (span / NS as f32 * if k == 0 || k == NS - 1 { 1.0 } else { 2.0 }) } else { 0.0 };
        for az in 0..NA {
            let a = (az as f32 + 0.5) / NA as f32 * TAU - PI;
            let d = Vec3::new(a.cos(), 0.0, a.sin());
            let p = d * r[k * NA + az] + Vec3::Y * y;
            out.positions.push(p.to_array());
            out.normals.push((d - Vec3::Y * dr).normalize_or(Vec3::Y).to_array());
            // Lower crown darker (sky occlusion), top lit.
            let v = 0.55 + 0.45 * (k as f32 + 0.5) / NS as f32;
            out.words.push([uv, leaf_word(v, p.y)]);
        }
    }
    let top = Vec3::Y * y1;
    out.positions.push(top.to_array());
    out.normals.push([0.0, 1.0, 0.0]);
    out.words.push([uv, leaf_word(1.0, top.y)]);
    let ring = |k: usize, az: usize| 1 + (k * NA + az % NA) as u32;
    let last = out.positions.len() as u32 - 1;
    for az in 0..NA {
        out.indices.extend([0, ring(0, az), ring(0, az + 1)]);
        for k in 0..NS - 1 {
            let (a, b, c, d) = (ring(k, az), ring(k, az + 1), ring(k + 1, az), ring(k + 1, az + 1));
            out.indices.extend([a, c, b, b, c, d]);
        }
        out.indices.extend([ring(NS - 1, az), last, ring(NS - 1, az + 1)]);
    }
    // Trunk prism from just below the ground into the crown.
    let base = out.positions.len() as u32;
    let yt = y0 + span * 0.15;
    for k in 0..3 {
        let a = k as f32 / 3.0 * TAU;
        let n = Vec3::new(a.cos(), 0.0, a.sin());
        for y in [-0.4f32, yt] {
            out.positions.push((n * trunk_r * if y < 0.0 { 1.25 } else { 0.7 } + Vec3::Y * y).to_array());
            out.normals.push(n.to_array());
            out.words.push([uv, pack_aux(0.8, 0.0, 0.0, 0.0, 0.0, false, true, 0)]);
        }
    }
    for k in 0..3u32 {
        let (a0, a1, b0, b1) = (base + k * 2, base + k * 2 + 1, base + (k + 1) % 3 * 2, base + (k + 1) % 3 * 2 + 1);
        out.indices.extend([a0, a1, b0, b0, a1, b1]);
    }
    out
}

/// LOD switch errors (m) for a tree of height `h`: LOD 1 at [`T0_PX`], LOD 2 at [`T1_PX`] of
/// projected height, empty at the hand-off (the reference tree at [`HANDOFF_PX`]; saplings
/// leave earlier, under 4 px tall). Kept increasing.
pub fn lod_errors(h: f32) -> [f32; 4] {
    let e3 = (TREE_REF_M / HANDOFF_PX).min(h / 4.0);
    let e2 = (h / T1_PX).min(e3 * 0.8);
    let e1 = (h / T0_PX).min(e2 * 0.8);
    [0.0, e1, e2, e3]
}

/// The bank mesh of stand species `species`, `variant` (`< variants(species)`) and age class
/// `age` (0 sapling, 1 sub-canopy, 2 canopy). Deterministic; slow (tens of ms): cache it.
pub fn bank_mesh(species: usize, variant: u32, age: u8) -> BankMesh {
    let sp = &STAND_SPECIES[species];
    let shape = shape(species);
    let kind = sp.kind;
    let seed = 0x7EE5_B000 + species as u64 * 977 + variant as u64 * 31 + age as u64 * 7919;
    let leaf = species::def(kind).leaf[variant as usize % VARIANTS as usize];
    // Age: young trees are narrower for their height (conifers keep their spire).
    let aspect = match age {
        0 => if sp.conifer { 0.85 } else { 0.75 },
        1 => 0.88,
        _ => 1.0,
    };
    let genome = Genome { height: 1.0, width: aspect, leaf_mul: [1.0; 3] };
    let (mut near, mut far) = match shape {
        Shape::Kind(k) => {
            // The alpha generator's species (shrubs, palms, ...): its own variant stream.
            let v = (variant + age as u32 * BANK_VARIANTS) % 64;
            (build_genome(k, v, false, &Genome::default()), build_genome(k, v, true, &Genome::default()))
        }
        _ => {
            let (n, cards) = grow(shape, kind, seed, leaf, false, 1.0);
            // T1 draws at 40..250 px for every tree in a forest: at most ~T1_CARDS cards (the
            // generator enlarges the kept cards to hold the crown's leaf area).
            let keep = (species::def(kind).far_cards.min(T1_CARDS) as f32 / cards.max(1) as f32).clamp(0.14, 1.0);
            (n, grow(shape, kind, seed, leaf, true, keep).0)
        }
    };
    genome.apply(&mut near);
    genome.apply(&mut far);
    let h = target_height(species, age);
    let s = h / near.height.max(0.1);
    // Reference colours: mean leaf card and bark colours of the near model.
    let mean = |m: &MeshData, sel: &dyn Fn(usize) -> bool| {
        let mut acc = [0.0f32; 4];
        let mut n = 0.0;
        for (i, c) in m.colors.iter().enumerate() {
            if sel(i) {
                for k in 0..4 {
                    acc[k] += c[k];
                }
                n += 1.0;
            }
        }
        if n > 0.0 { acc.map(|x| x / n) } else { [0.5, 0.5, 0.5, 1.0] }
    };
    let leaf_ref = mean(&near.leaves.mesh, &|i| !in_solid_cell(near.leaves.mesh.uvs[i]));
    let bark_ref = if near.bark.mesh.is_empty() { mean(&far.leaves.mesh, &|i| in_solid_cell(far.leaves.mesh.uvs[i])) } else { mean(&near.bark.mesh, &|_| true) };
    let mid_y = h * 0.55;
    let l0 = convert(&near, s, leaf_ref, bark_ref, mid_y);
    let l1 = convert(&far, s, leaf_ref, bark_ref, mid_y);
    let trunk_r = near.bark.mesh.positions.iter().filter(|p| p[1] < 1.0 && p[1] > 0.2).map(|p| (p[0] * p[0] + p[2] * p[2]).sqrt()).fold(0.0f32, f32::max).max(0.05) * s;
    let l2 = shell(&l0, h, trunk_r);
    let errors = lod_errors(h);
    let mut out = BankMesh {
        height: h,
        crown_radius: near.crown_radius * s,
        leaf_albedo: leaf_albedo(species),
        bark_albedo: [lin(bark_ref[0].min(1.0)).max(bark_ref[0] * 0.2), 0.0, 0.0],
        ..Default::default()
    };
    // Bark albedo: the generator's bark colours are already linear (`mesh::rgb`).
    // Real bark albedo is 0.05..0.3 (birch bark brighter): scale into that range.
    out.bark_albedo = [bark_ref[0], bark_ref[1], bark_ref[2]].map(|x| (x * 0.55).clamp(0.02, 0.35));
    for (k, l) in [l0, l1, l2].into_iter().enumerate() {
        let base = out.positions.len() as u32;
        out.positions.extend(l.positions);
        out.normals.extend(l.normals);
        out.words.extend(l.words);
        out.lods.push((l.indices.into_iter().map(|i| i + base).collect(), errors[k]));
    }
    out.lods.push((Vec::new(), errors[3]));
    out
}

// ---------------------------------------------------------------------------------------------
// Per-instance variety
// ---------------------------------------------------------------------------------------------

/// What one stand tree draws as: bank entry, transform and per-instance words.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InstanceParams {
    pub variant: u32,
    pub age: u8,
    /// Uniform scale of the bank mesh and an extra horizontal (crown width) factor.
    pub scale: f32,
    pub widen: f32,
    pub yaw: f32,
    /// Lean: angle (rad) about a horizontal axis at azimuth `lean_az`.
    pub lean: f32,
    pub lean_az: f32,
    /// Instance user word 0: value (8 bits, x/128), hue (8 bits, -1..1), branch drop mask
    /// (8 bits), wind phase offset (8 bits). `common.slang` `tree_user`.
    pub user: u32,
}

pub fn instance_params(t: &StandTree) -> InstanceParams {
    let species = t.species as usize;
    let h = |salt: u64| crate::hash01(t.seed as u64, salt);
    let variant = (t.seed % variants(species)) as u32 % variants(species);
    let age = t.age_class.min(AGE_CLASSES - 1);
    let target = target_height(species, age);
    let scale = (t.height_m / target).clamp(0.35, 2.8);
    let widen = 0.88 + 0.24 * h(3);
    let value = 0.86 + 0.28 * h(4);
    let hue = h(5) * 2.0 - 1.0;
    let mut drop = 0u32;
    for g in 0..GROUPS {
        let p = if g < 4 { 0.16 } else { 0.05 };
        if h(10 + g as u64) < p {
            drop |= 1 << g;
        }
    }
    let user = ((value * 128.0).round() as u32).min(255) | (((hue * 0.5 + 0.5) * 255.0).round() as u32) << 8 | drop << 16 | ((h(6) * 255.0) as u32) << 24;
    InstanceParams { variant, age, scale, widen, yaw: h(1) * TAU, lean: h(2).powi(2) * 0.05, lean_az: h(7) * TAU, user }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aux_round_trip() {
        let w = pack_aux(1.3, -0.4, 0.8, 0.5, 0.25, true, false, 5);
        let (v, h, s, f, p) = unpack_aux(w);
        assert!((v - 1.3).abs() < 0.02 && (h + 0.4).abs() < 0.07 && (s - 0.8).abs() < 0.03 && (f - 0.57).abs() < 0.1 && (p - 0.25).abs() < 0.02);
        assert_eq!(w & AUX_CARD, AUX_CARD);
        assert_eq!(w >> 29, 5);
    }

    #[test]
    fn class_albedo_is_the_far_rung_table() {
        for conifer in [false, true] {
            let rows: Vec<usize> = (0..STAND_SPECIES.len()).filter(|&i| STAND_SPECIES[i].conifer == conifer && !STAND_SPECIES[i].shrub).collect();
            // Geometric mean of the per-species ratio is not 1; the arithmetic mean of the
            // unclamped albedos is the class albedo.
            let mean = rows.iter().map(|&i| leaf_albedo(i)).fold([0.0f32; 3], |a, c| [a[0] + c[0], a[1] + c[1], a[2] + c[2]]).map(|x| x / rows.len() as f32);
            let want = CROWN_ALBEDO[conifer as usize];
            for k in 0..3 {
                assert!((mean[k] - want[k]).abs() < want[k] * 0.08, "{conifer} {k}: {mean:?} vs {want:?}");
            }
        }
    }

    #[test]
    fn bank_mesh_is_deterministic_with_a_lod_chain() {
        let spruce = STAND_SPECIES.iter().position(|s| s.name == "Norway spruce").unwrap();
        let a = bank_mesh(spruce, 2, 2);
        let b = bank_mesh(spruce, 2, 2);
        assert_eq!(a.positions, b.positions);
        assert_eq!(a.words, b.words);
        assert_eq!(a.lods.len(), 4);
        assert!(a.lods[3].0.is_empty());
        let t: Vec<usize> = (0..3).map(|l| a.triangles(l)).collect();
        assert!(t[0] > t[1] && t[1] > t[2] && t[2] > 50, "{t:?}");
        let e: Vec<f32> = a.lods.iter().map(|l| l.1).collect();
        assert!(e.windows(2).all(|w| w[0] < w[1]), "{e:?}");
        let top = a.positions.iter().fold(0.0f32, |m, p| m.max(p[1]));
        assert!((top - a.height).abs() < a.height * 0.1, "{top} vs {}", a.height);
        // Narrow spire: crown radius well under a third of the height.
        assert!(a.crown_radius < a.height * 0.3, "{} {}", a.crown_radius, a.height);
        // Variants differ.
        assert_ne!(bank_mesh(spruce, 3, 2).positions, a.positions);
    }

    #[test]
    fn instance_params_vary() {
        let spruce = STAND_SPECIES.iter().position(|s| s.name == "Norway spruce").unwrap() as u8;
        let mk = |seed: u32| StandTree { dir: glam::DVec3::X, fx: 0.0, fy: 0.0, height_m: 25.0, crown_r_m: 3.0, species: spruce, kind: TreeKind::Conifer, age_class: 2, seed, id: ed_geo::ObjectId(0) };
        let ps: Vec<InstanceParams> = (0..100).map(|s| instance_params(&mk(s * 7919 + 1))).collect();
        let variants: std::collections::BTreeSet<u32> = ps.iter().map(|p| p.variant).collect();
        assert_eq!(variants.len(), CONIFER_VARIANTS as usize);
        let users: std::collections::BTreeSet<u32> = ps.iter().map(|p| p.user).collect();
        assert!(users.len() > 95);
        assert_eq!(instance_params(&mk(5)), instance_params(&mk(5)));
    }

    /// `cargo test -p ed_world --release --lib bank_stats -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn bank_stats() {
        let mut total = 0usize;
        let t0 = std::time::Instant::now();
        for sp in 0..STAND_SPECIES.len() {
            let t = std::time::Instant::now();
            let mut bytes = 0;
            let mut tris = [0usize; 3];
            for age in 0..AGE_CLASSES {
                for v in 0..variants(sp) {
                    let m = bank_mesh(sp, v, age);
                    bytes += m.bytes();
                    if age == 2 && v == 0 {
                        tris = [m.triangles(0), m.triangles(1), m.triangles(2)];
                    }
                }
            }
            total += bytes;
            println!("{:<22} {:?} tris {:?} {:.1} MB {:.0} ms", STAND_SPECIES[sp].name, shape(sp), tris, bytes as f64 / 1e6, t.elapsed().as_secs_f64() * 1e3);
        }
        println!("total {:.1} MB {:.0} ms", total as f64 / 1e6, t0.elapsed().as_secs_f64() * 1e3);
    }
}
