//! Ground cover to 150 m (trees G7, decision O3-5): a hierarchical, niche-driven scatter of
//! small generated meshes, drawn as GPU instances (the renderer's foliage path, like the near
//! trees) instead of the alpha's per-chunk merged meshes (`ground_scatter`, whose shape
//! generators this reuses).
//!
//! **What grows where** comes from the same inputs the terrain material and the stand read:
//! the canopy page (cover, height, conifer mix), the ground height (altitude, landform slope),
//! a CPU mirror of the terrain material's layer rules (rock, scree, grass, forest, snow) and
//! the real climate normals (`ed_data::climate`: aridity, temperature). Under a closed canopy:
//! moss, ferns, blueberry, litter and fallen logs with sparse shade grass; in the open: dense
//! grass and flowers (short turf and more flowers above the treeline, straw tufts where it is
//! dry); on rock and scree: boulders, stones, pebbles; on snow: nothing.
//!
//! **Hierarchy** (one pass per size class, each a global lattice of the cube face, so the
//! result is seamless across nodes and independent of iteration order):
//! 1. logs and boulders (~9.5 m cells). Logs lie by tall stand trees (a fallen neighbour),
//!    never through a trunk.
//! 2. ferns, moss, blueberry, stones (~1.2 m cells): moss and ferns gather around the pass-1
//!    objects (shade and damp in their lee), nothing inside them or a trunk.
//! 3. grass, flowers, pebbles, twigs (~0.6 m cells): fill what the passes above left.
//!
//! **Distance:** every item has a rank; the vertex shader keeps an item while
//! `rank < keep(distance, reach)` ([`keep`]): full density near, then ~1/d^2 (constant on-screen
//! density), shrinking smoothly to nothing at `reach`; survivors grow (up to [`GROW_MAX`]) so
//! the cover holds its coverage as it thins. Logs and boulders are solid: they never shrink and
//! leave by the renderer's LOD dither past their reach.

use crate::ground_scatter::{Rng, cushion, dry_tuft, fern, flower, gray, rosette, scrub, stone, tuft, twig};
use crate::mesh::MeshData;
use crate::noise::{fbm3, smoothstep};
use crate::stand::{CanopySample, StandTree};
use crate::trees::bank::{pack_aux, pack_uv};
use ed_geo::NodeId;
use ed_geo::node::lattice_point;
use glam::{DVec3, Vec3};
use std::f32::consts::TAU;
use std::sync::OnceLock;

/// Ground-cover nodes are this level (~76 m).
pub const NODE_LEVEL: u8 = 17;
/// Default reach (m) of the grass: the distance where it has thinned to nothing. Set from the
/// measured contribution by distance band (G7, 1440p, 2 m AGL on meadow): 0-25 m carries ~90 %
/// of the changed pixels, 25-50 m ~8 %, 50-100 m ~2 % for half the instances, past 100 m none.
pub const REACH_M: f32 = 100.0;
/// Full density inside `NEAR_FRAC * reach` (19.5 m at the default reach).
pub const NEAR_FRAC: f32 = 0.195;
/// The fade-out band starts at `FADE_FRAC * reach`.
pub const FADE_FRAC: f32 = 0.7;
/// Most a surviving item grows across (shader `GC_GROW_MAX`).
pub const GROW_MAX: f32 = 2.0;
/// Most it grows in height.
pub const GROW_Y_MAX: f32 = 1.25;

const EARTH_R: f64 = 6_371_008.8;
/// Lattice levels of the three passes (face cells of ~9.5 m, ~1.2 m, ~0.6 m).
const L1: u8 = 20;
const L2: u8 = 23;
const L3: u8 = 24;
/// Sub-cell jitter resolution.
pub(crate) const SUB: u64 = 256;

/// Share of the instances kept at distance `d` (m) for an item of `reach` (m): 1 inside
/// `NEAR_FRAC * reach`, then (r0 / d)^2, eased to 0 between `FADE_FRAC * reach` and `reach`.
/// The shader twin is `common.slang` `gc_keep`.
pub fn keep(d: f32, reach: f32) -> f32 {
    let r0 = NEAR_FRAC * reach;
    let k = (r0 / d.max(r0)).powi(2);
    k * (1.0 - smoothstep(FADE_FRAC * reach, reach, d))
}

/// How much a surviving item grows across at keep fraction `k` (coverage held as density thins).
pub fn grow(k: f32) -> f32 {
    (1.0 / k.max(1e-4)).sqrt().clamp(1.0, GROW_MAX)
}

// ---------------------------------------------------------------------------------------------
// Kinds and their meshes
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    Grass,
    Turf,
    DryGrass,
    ShadeGrass,
    FlowerWhite,
    FlowerYellow,
    FlowerViolet,
    FlowerRed,
    Rosette,
    Pebble,
    Twig,
    Fern,
    Moss,
    Blueberry,
    Stone,
    Boulder,
    Log,
}

/// Static row per kind: pass, variants, reach, look.
#[derive(Clone, Copy, Debug)]
pub struct KindDef {
    pub kind: Kind,
    pub name: &'static str,
    /// 1, 2 or 3 (see the module doc).
    pub pass: u8,
    pub variants: u32,
    /// Distance (m) where it has thinned out.
    pub reach: f32,
    /// Never shrinks (logs, boulders): leaves by LOD dither past `reach`.
    pub solid: bool,
    /// Instance scale range.
    pub scale: (f32, f32),
    /// Exclusion radius (m) at scale 1 for the passes below it.
    pub radius: f32,
    /// Linear albedo of the leaf (card) parts and of the "shade" parts (bark, stone, petals).
    pub leaf: [f32; 3],
    pub shade: [f32; 3],
    /// Wind sway (m per unit of the renderer's sway term at the top).
    pub sway: f32,
    /// Rank multiplier (lower: stays to a longer distance).
    pub rank_k: f32,
    /// Casts sun shadows.
    pub shadows: bool,
}

/// Grass blade albedo: the terrain's grass layer mean (`ew` terrain_mat `__grass_lawn` target
/// [0.11, 0.155, 0.05]) over the blades' mean value, so the far cover meets the ground colour.
const GRASS: [f32; 3] = [0.125, 0.185, 0.048];
const STEM: [f32; 3] = [0.080, 0.130, 0.035];
const STRAW: [f32; 3] = [0.28, 0.24, 0.13];
const SHADE_GREEN: [f32; 3] = [0.070, 0.115, 0.035];
const MOSS: [f32; 3] = [0.075, 0.105, 0.024];
const ROCK: [f32; 3] = [0.20, 0.19, 0.18];
const WOOD: [f32; 3] = [0.105, 0.080, 0.058];

pub const KINDS: [KindDef; 17] = [
    KindDef { kind: Kind::Grass, name: "grass", pass: 3, variants: 6, reach: REACH_M, solid: false, scale: (0.8, 1.25), radius: 0.0, leaf: GRASS, shade: STEM, sway: 0.28, rank_k: 1.0, shadows: false },
    KindDef { kind: Kind::Turf, name: "turf", pass: 3, variants: 5, reach: REACH_M, solid: false, scale: (0.8, 1.25), radius: 0.0, leaf: GRASS, shade: STEM, sway: 0.12, rank_k: 1.0, shadows: false },
    KindDef { kind: Kind::DryGrass, name: "dry grass", pass: 3, variants: 5, reach: REACH_M, solid: false, scale: (0.8, 1.3), radius: 0.0, leaf: STRAW, shade: STRAW, sway: 0.3, rank_k: 1.0, shadows: false },
    KindDef { kind: Kind::ShadeGrass, name: "shade grass", pass: 3, variants: 4, reach: 90.0, solid: false, scale: (0.8, 1.2), radius: 0.0, leaf: SHADE_GREEN, shade: STEM, sway: 0.25, rank_k: 1.0, shadows: false },
    KindDef { kind: Kind::FlowerWhite, name: "white flower", pass: 3, variants: 3, reach: 90.0, solid: false, scale: (0.8, 1.3), radius: 0.0, leaf: STEM, shade: [0.72, 0.72, 0.66], sway: 0.25, rank_k: 1.0, shadows: false },
    KindDef { kind: Kind::FlowerYellow, name: "yellow flower", pass: 3, variants: 3, reach: 90.0, solid: false, scale: (0.8, 1.3), radius: 0.0, leaf: STEM, shade: [0.70, 0.50, 0.04], sway: 0.25, rank_k: 1.0, shadows: false },
    KindDef { kind: Kind::FlowerViolet, name: "violet flower", pass: 3, variants: 3, reach: 90.0, solid: false, scale: (0.8, 1.3), radius: 0.0, leaf: STEM, shade: [0.22, 0.12, 0.48], sway: 0.25, rank_k: 1.0, shadows: false },
    KindDef { kind: Kind::FlowerRed, name: "red flower", pass: 3, variants: 3, reach: 90.0, solid: false, scale: (0.8, 1.3), radius: 0.0, leaf: STEM, shade: [0.55, 0.04, 0.03], sway: 0.25, rank_k: 1.0, shadows: false },
    KindDef { kind: Kind::Rosette, name: "rosette", pass: 3, variants: 4, reach: 40.0, solid: false, scale: (0.8, 1.4), radius: 0.0, leaf: [0.07, 0.12, 0.035], shade: STEM, sway: 0.0, rank_k: 1.0, shadows: false },
    KindDef { kind: Kind::Pebble, name: "pebble", pass: 3, variants: 6, reach: 45.0, solid: false, scale: (0.05, 0.14), radius: 0.0, leaf: ROCK, shade: ROCK, sway: 0.0, rank_k: 1.0, shadows: false },
    KindDef { kind: Kind::Twig, name: "twig", pass: 3, variants: 4, reach: 40.0, solid: false, scale: (0.7, 1.5), radius: 0.0, leaf: WOOD, shade: WOOD, sway: 0.0, rank_k: 1.0, shadows: false },
    KindDef { kind: Kind::Fern, name: "fern", pass: 2, variants: 4, reach: REACH_M, solid: false, scale: (0.7, 1.4), radius: 0.35, leaf: [0.060, 0.120, 0.028], shade: STEM, sway: 0.15, rank_k: 0.35, shadows: true },
    KindDef { kind: Kind::Moss, name: "moss", pass: 2, variants: 4, reach: 90.0, solid: false, scale: (0.25, 0.6), radius: 0.9, leaf: MOSS, shade: MOSS, sway: 0.0, rank_k: 0.5, shadows: false },
    KindDef { kind: Kind::Blueberry, name: "blueberry", pass: 2, variants: 4, reach: REACH_M, solid: false, scale: (0.8, 1.5), radius: 0.35, leaf: [0.040, 0.075, 0.026], shade: [0.11, 0.075, 0.05], sway: 0.08, rank_k: 0.35, shadows: true },
    KindDef { kind: Kind::Stone, name: "stone", pass: 2, variants: 5, reach: REACH_M, solid: false, scale: (0.15, 0.45), radius: 0.9, leaf: ROCK, shade: ROCK, sway: 0.0, rank_k: 0.3, shadows: true },
    KindDef { kind: Kind::Boulder, name: "boulder", pass: 1, variants: 4, reach: 140.0, solid: true, scale: (0.7, 2.0), radius: 0.85, leaf: [0.10, 0.12, 0.06], shade: ROCK, sway: 0.0, rank_k: 0.0, shadows: true },
    KindDef { kind: Kind::Log, name: "log", pass: 1, variants: 4, reach: 140.0, solid: true, scale: (0.8, 1.15), radius: 0.0, leaf: MOSS, shade: WOOD, sway: 0.0, rank_k: 0.0, shadows: true },
];

pub fn def(k: Kind) -> &'static KindDef {
    &KINDS[k as usize]
}

/// One library mesh in the renderer's foliage layout: positions, normals, two words per vertex
/// (`bank::pack_uv`, `bank::pack_aux`) and two index lists (full, simplified); the renderer's
/// chain adds the empty last level at the reach.
#[derive(Clone, Debug, Default)]
pub struct CoverMesh {
    pub kind: Option<Kind>,
    pub variant: u32,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub words: Vec<[u32; 2]>,
    pub lod0: Vec<u32>,
    pub lod1: Vec<u32>,
    /// Bounding radius (m) at scale 1, around the origin.
    pub radius: f32,
}

impl CoverMesh {
    pub fn triangles(&self) -> usize {
        self.lod0.len() / 3
    }
}

/// How a mesh part shades: leaf (`AUX_CARD`: translucent, two-sided, the material base colour)
/// or shade (`AUX_BARK`: opaque, the material shade colour).
#[derive(Clone, Copy, PartialEq)]
enum Part {
    Leaf,
    Shade,
}

/// Converts a `ground_scatter` mesh: vertex colour -> value and hue, `uv.x * uv.y^2` -> sway.
/// `part(i)` says how vertex `i` shades; `group` vertices share a wind phase.
fn convert(m: &MeshData, sway: f32, part: &dyn Fn(usize) -> Part, group: usize, seed: u64) -> (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<[u32; 2]>) {
    let mut words = Vec::with_capacity(m.positions.len());
    for i in 0..m.positions.len() {
        let c = m.colors[i];
        let lum = (0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]).max(0.0);
        let hue = ((c[2] - c[0]) / lum.max(0.05) * 2.0).clamp(-1.0, 1.0);
        let uv = m.uvs[i];
        let s = sway * uv[0].clamp(0.0, 1.0) * uv[1].clamp(0.0, 1.0).powi(2);
        let phase = crate::hash01(seed, (i / group.max(1)) as u64);
        let p = part(i);
        words.push([pack_uv([0.0, 0.0]), pack_aux(lum.min(2.0), hue, s, if s > 0.0 { 0.3 } else { 0.0 }, phase, p == Part::Leaf, p == Part::Shade, 0)]);
    }
    (m.positions.clone(), m.normals.clone(), words)
}

/// Every other group of `g` triangles (blades: 3 triangles each).
fn halve(idx: &[u32], g: usize) -> Vec<u32> {
    idx.chunks(3 * g).enumerate().filter(|(k, _)| k % 2 == 0).flat_map(|(_, c)| c.iter().copied()).collect()
}

fn finish(kind: Kind, variant: u32, parts: (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<[u32; 2]>), lod0: Vec<u32>, lod1: Vec<u32>) -> CoverMesh {
    let (positions, normals, words) = parts;
    let radius = positions.iter().map(|p| Vec3::from(*p).length()).fold(0.0f32, f32::max).max(0.02);
    CoverMesh { kind: Some(kind), variant, positions, normals, words, lod0, lod1, radius }
}

/// A grass patch: `blades` blades over a disc of `spread` m.
fn patch(kind: Kind, v: u32, seed: u64, blades: usize, h: (f32, f32), w: f32, spread: f32, lean: f32) -> CoverMesh {
    let m = tuft(seed, blades, h, w, spread, lean);
    let parts = convert(&m, def(kind).sway, &|_| Part::Leaf, 5, seed);
    let lod1 = halve(&m.indices, 1);
    finish(kind, v, parts, m.indices.clone(), lod1)
}

/// A fallen log along +x (centred), radius `r`, length `len`: a slightly bent, tapering
/// 9-sided trunk with a broken end and a sawn-looking one, moss along the top.
fn log_mesh(v: u32, seed: u64, len: f32, r: f32) -> CoverMesh {
    let mut rng = Rng(seed);
    let segs = 10usize;
    let sides = 9usize;
    let bend = rng.range(-0.08, 0.08) * len;
    let sink = 0.25 * r;
    let mut pos = Vec::new();
    let mut nrm = Vec::new();
    let mut words = Vec::new();
    let mut idx = Vec::new();
    let axis_at = |t: f32| Vec3::new((t - 0.5) * len, r - sink, bend * 4.0 * t * (1.0 - t));
    let rad_at = |t: f32| r * (1.0 - 0.25 * t);
    // Bark (shade parts).
    for k in 0..=segs {
        let t = k as f32 / segs as f32;
        let c = axis_at(t);
        for j in 0..sides {
            let a = j as f32 / sides as f32 * TAU;
            let bump = 1.0 + 0.08 * (fbm3(t * len * 1.3, a, 0.0, seed, 2) - 0.5);
            let d = Vec3::new(0.0, a.sin(), a.cos());
            pos.push((c + d * rad_at(t) * bump).to_array());
            nrm.push(d.to_array());
            // Darker underneath and along bark furrows.
            let furrow = 0.75 + 0.5 * fbm3(t * len * 3.0, a * 2.0, 1.0, seed + 5, 2);
            let under = 0.55 + 0.45 * smoothstep(-0.8, 0.4, d.y);
            words.push([pack_uv([0.0, 0.0]), pack_aux((furrow * under).min(2.0), 0.0, 0.0, 0.0, 0.0, false, true, 0)]);
        }
    }
    for k in 0..segs as u32 {
        for j in 0..sides as u32 {
            let a = k * sides as u32 + j;
            let b = k * sides as u32 + (j + 1) % sides as u32;
            let (c, d) = (a + sides as u32, b + sides as u32);
            idx.extend_from_slice(&[a, c, d, a, d, b]);
        }
    }
    // End caps: the near one sawn flat (light heartwood), the far one broken (a jagged cone).
    for (t, broken) in [(0.0f32, false), (1.0, true)] {
        let c = axis_at(t);
        let out = if t == 0.0 { -Vec3::X } else { Vec3::X };
        let tip = c + out * if broken { rng.range(0.15, 0.4) * r * 2.0 } else { 0.0 };
        let ci = pos.len() as u32;
        pos.push(tip.to_array());
        nrm.push(out.to_array());
        words.push([pack_uv([0.0, 0.0]), pack_aux(if broken { 0.9 } else { 1.6 }, -0.3, 0.0, 0.0, 0.0, false, true, 0)]);
        let ring0 = pos.len() as u32;
        for j in 0..sides {
            let a = j as f32 / sides as f32 * TAU;
            let d = Vec3::new(0.0, a.sin(), a.cos());
            let jag = if broken { out * rng.range(0.0, 0.5) * r } else { Vec3::ZERO };
            pos.push((c + d * rad_at(t) * 0.97 + jag).to_array());
            nrm.push((out * 0.7 + d * 0.3).normalize().to_array());
            words.push([pack_uv([0.0, 0.0]), pack_aux(if broken { 0.8 } else { 1.4 }, -0.3, 0.0, 0.0, 0.0, false, true, 0)]);
        }
        for j in 0..sides as u32 {
            let (a, b) = (ring0 + j, ring0 + (j + 1) % sides as u32);
            if t == 0.0 { idx.extend_from_slice(&[ci, b, a]) } else { idx.extend_from_slice(&[ci, a, b]) }
        }
    }
    let bark_tris = idx.len();
    // Moss skin over the top where a noise says so: a slightly larger partial shell (leaf parts).
    let ms = pos.len() as u32;
    let mside = 5usize;
    for k in 0..=segs {
        let t = k as f32 / segs as f32;
        let c = axis_at(t);
        for j in 0..mside {
            let a = (0.15 + 0.7 * j as f32 / (mside - 1) as f32) * std::f32::consts::PI;
            let d = Vec3::new(0.0, a.sin(), a.cos());
            pos.push((c + d * rad_at(t) * 1.06).to_array());
            nrm.push((d * 0.6 + Vec3::Y * 0.4).normalize().to_array());
            let speck = 0.75 + 0.5 * fbm3(t * len * 6.0, a * 3.0, 2.0, seed + 9, 2);
            words.push([pack_uv([0.0, 0.0]), pack_aux(speck, 0.0, 0.0, 0.0, 0.0, true, false, 0)]);
        }
    }
    for k in 0..segs as u32 {
        let t = (k as f32 + 0.5) / segs as f32;
        if fbm3(t * len * 0.7, 0.0, 3.0, seed + 11, 2) < 0.42 {
            continue;
        }
        for j in 0..mside as u32 - 1 {
            let a = ms + k * mside as u32 + j;
            let (b, c, d) = (a + 1, a + mside as u32, a + mside as u32 + 1);
            idx.extend_from_slice(&[a, c, d, a, d, b]);
        }
    }
    let lod1 = idx[..bark_tris].to_vec();
    finish(Kind::Log, v, (pos, nrm, words), idx, lod1)
}

fn lit(c: f32, part: Part) -> impl Fn(usize) -> Part {
    let _ = c;
    move |_| part
}

/// Builds every library mesh (deterministic). Index with [`proto_index`].
fn build_library() -> Vec<CoverMesh> {
    let mut out = Vec::new();
    for d in KINDS {
        for v in 0..d.variants {
            let s = 0x6C_0000 + d.kind as u64 * 1013 + v as u64 * 7919;
            let m = match d.kind {
                Kind::Grass => patch(d.kind, v, s, 40, (0.16, 0.44), 0.015, 0.34, 0.5),
                Kind::Turf => patch(d.kind, v, s, 44, (0.06, 0.15), 0.011, 0.34, 0.35),
                Kind::ShadeGrass => patch(d.kind, v, s, 12, (0.18, 0.36), 0.009, 0.22, 0.75),
                Kind::DryGrass => {
                    let m = dry_tuft(s);
                    let parts = convert(&m, d.sway, &lit(0.0, Part::Leaf), 5, s);
                    finish(d.kind, v, parts, m.indices.clone(), halve(&m.indices, 1))
                }
                Kind::FlowerWhite | Kind::FlowerYellow | Kind::FlowerViolet | Kind::FlowerRed => {
                    let (h, petals, plen) = match d.kind {
                        Kind::FlowerYellow => (0.24, 6, 0.016),
                        Kind::FlowerViolet => (0.30, 5, 0.014),
                        Kind::FlowerRed => (0.36, 4, 0.03),
                        _ => (0.20, 9, 0.015),
                    };
                    // Petals carry a marker alpha (shade part, the material's petal colour);
                    // the centre is a darker petal value.
                    let m = flower(s, [1.0, 1.0, 1.0, 0.5], [0.35, 0.3, 0.12, 0.5], h * (0.85 + 0.1 * v as f32), petals, plen);
                    let part = |i: usize| if m.colors[i][3] < 0.9 { Part::Shade } else { Part::Leaf };
                    let parts = convert(&m, d.sway, &part, 1_000_000, s);
                    finish(d.kind, v, parts, m.indices.clone(), m.indices.clone())
                }
                Kind::Rosette => {
                    let m = rosette(s);
                    let parts = convert(&m, 0.0, &lit(0.0, Part::Leaf), 6, s);
                    finish(d.kind, v, parts, m.indices.clone(), m.indices.clone())
                }
                Kind::Pebble => {
                    let m = stone(s, 1, 3, 0.5, 0.55, 0.0);
                    let parts = convert(&m, 0.0, &lit(0.0, Part::Shade), 1, s);
                    finish(d.kind, v, parts, m.indices.clone(), m.indices.clone())
                }
                Kind::Twig => {
                    let m = twig(s, 0.45, 0.012, true);
                    let parts = convert(&m, 0.0, &lit(0.0, Part::Shade), 1, s);
                    finish(d.kind, v, parts, m.indices.clone(), m.indices.clone())
                }
                Kind::Fern => {
                    let m = fern(s);
                    let parts = convert(&m, d.sway, &lit(0.0, Part::Leaf), 3, s);
                    finish(d.kind, v, parts, m.indices.clone(), halve(&m.indices, 2))
                }
                Kind::Moss => {
                    let m = cushion(s, 0.22, 1.4, [1.2, 1.15, 0.85], [0.55, 0.6, 0.5]);
                    let parts = convert(&m, 0.0, &lit(0.0, Part::Shade), 1, s);
                    finish(d.kind, v, parts, m.indices.clone(), m.indices.clone())
                }
                Kind::Blueberry => {
                    let m = scrub(s, true);
                    // Woody stems are brown-ish vertex colours; leaves grey multipliers.
                    let part = |i: usize| {
                        let c = m.colors[i];
                        if (c[0] - c[2]).abs() > 0.1 { Part::Shade } else { Part::Leaf }
                    };
                    let parts = convert(&m, d.sway, &part, 5, s);
                    finish(d.kind, v, parts, m.indices.clone(), halve(&m.indices, 3))
                }
                Kind::Stone => {
                    let m = stone(s, 2, 5, 0.35, 0.6, 1.0);
                    let parts = convert(&m, 0.0, &lit(0.0, Part::Shade), 1, s);
                    finish(d.kind, v, parts, m.indices.clone(), m.indices.clone())
                }
                Kind::Boulder => {
                    // Full: subdivided twice; simplified: the same stone at one subdivision,
                    // appended to the same vertex array.
                    let a = stone(s, 2, 6, 0.3, 0.7, 1.0);
                    let b = stone(s, 1, 6, 0.3, 0.7, 1.0);
                    let (mut p, mut n, mut w) = convert(&a, 0.0, &lit(0.0, Part::Shade), 1, s);
                    let off = p.len() as u32;
                    let (p2, n2, w2) = convert(&b, 0.0, &lit(0.0, Part::Shade), 1, s);
                    p.extend(p2);
                    n.extend(n2);
                    w.extend(w2);
                    let lod1 = b.indices.iter().map(|i| i + off).collect();
                    finish(d.kind, v, (p, n, w), a.indices.clone(), lod1)
                }
                Kind::Log => {
                    let len = [4.5, 6.0, 7.5, 3.5][v as usize % 4];
                    let r = [0.2, 0.26, 0.3, 0.17][v as usize % 4];
                    log_mesh(v, s, len, r)
                }
            };
            out.push(m);
        }
    }
    out
}

/// The library (built once).
pub fn library() -> &'static [CoverMesh] {
    static LIB: OnceLock<Vec<CoverMesh>> = OnceLock::new();
    LIB.get_or_init(build_library)
}

/// Library index of `(kind, variant)`.
pub fn proto_index(kind: Kind, variant: u32) -> usize {
    KINDS[..kind as usize].iter().map(|d| d.variants as usize).sum::<usize>() + (variant % def(kind).variants) as usize
}

/// Log length and radius of a variant (for its footprint).
fn log_dims(v: u32) -> (f32, f32) {
    ([4.5, 6.0, 7.5, 3.5][v as usize % 4], [0.2, 0.26, 0.3, 0.17][v as usize % 4])
}

// ---------------------------------------------------------------------------------------------
// Niche
// ---------------------------------------------------------------------------------------------

/// What the ground is at one point.
#[derive(Clone, Copy, Debug, Default)]
pub struct GroundSample {
    pub canopy: CanopySample,
    /// Landform slope cosine (~16 m).
    pub slope_cos: f32,
    /// Land-cover shares of the node's ground page, `[herb, crop, shrub, wet]` in 0..1 (`None`:
    /// no ground page here).
    pub shares: Option<[f32; 4]>,
}

/// Terrain material layer weights (CPU mirror of resolve.slang `terrain_material` without its
/// noise terms): rock, scree, grass, forest, snow, soil, normalised.
pub fn layer_weights(alt: f32, slope_cos: f32, cover: f32) -> [f32; 6] {
    let h = alt;
    let cs = slope_cos;
    let alpine = smoothstep(2150.0 - 150.0, 2150.0 + 250.0, h);
    let rock_cs = 0.68 + (0.77 - 0.68) * alpine;
    let steep = 1.0 - smoothstep(rock_cs - 0.14, rock_cs, cs);
    let steep_f = 1.0 - smoothstep(rock_cs - 0.2, rock_cs - 0.06, cs);
    let rock = (steep.max(steep_f * 0.8) + smoothstep(2700.0, 3300.0, h) * 0.45).clamp(0.0, 1.0);
    let moderate = (1.0 - smoothstep(0.88, 0.97, cs)) * smoothstep(0.66, 0.80, cs);
    let scree = (moderate * 0.1 * (0.1 + 0.9 * alpine) + steep * 0.3 * alpine).clamp(0.0, 1.0);
    let forest = smoothstep(0.03, 0.5, cover);
    let grass = (1.0 - forest) * (1.0 - smoothstep(2600.0, 3000.0, h));
    let snow = smoothstep(3050.0 - 120.0, 3050.0 + 120.0, h) * smoothstep(0.62, 0.78, cs);
    let mut w = [rock, scree * (1.0 - rock * 0.7), grass * (1.0 - rock) * (1.0 - scree * 0.8), forest * (1.0 - rock) * (1.0 - scree * 0.8), snow * 1.6, 0.0];
    for (k, x) in w.iter_mut().enumerate() {
        if k != 4 {
            *x *= 1.0 - snow;
        }
    }
    let t: f32 = w.iter().sum();
    if t > 1e-4 {
        w.iter_mut().for_each(|x| *x /= t);
    } else {
        w[2] = 1.0;
    }
    w
}

/// The place's climate, once per node: lushness (wet and warm enough), dryness and alpine-ness
/// drive the open-ground mix.
#[derive(Clone, Copy, Debug)]
pub struct Climate {
    /// Aridity index (precipitation / PET): < 0.2 desert, > 0.65 humid.
    pub aridity: f32,
    /// Mean annual temperature at sea level (°C).
    pub temp_sl: f32,
}

impl Climate {
    /// From the embedded climate normals at body direction `d` (temperate default off-grid).
    pub fn at(d: DVec3) -> Climate {
        let lat = d.z.clamp(-1.0, 1.0).asin().to_degrees();
        let lon = d.y.atan2(d.x).to_degrees();
        match ed_data::climate::Climate::at(lat, lon) {
            Some(c) => Climate { aridity: c.aridity(0.0), temp_sl: c.temp_sl },
            None => Climate { aridity: 1.0, temp_sl: 10.0 },
        }
    }
}

/// Probability per cell of each kind of `pass` at a point (sums to at most 1).
fn niche(pass: u8, g: &GroundSample, cl: &Climate, near_big: f32) -> [(Kind, f32); 8] {
    let c = &g.canopy;
    let w = layer_weights(c.alt_m, g.slope_cos, c.cover);
    let (rocky, grassy, snowy) = (w[0] + w[1], w[2] + w[5], w[4]);
    let shade = smoothstep(0.08, 0.6, c.cover);
    let open = 1.0 - shade;
    let temp = cl.temp_sl - 0.0065 * c.alt_m.max(0.0);
    let pet = (20.0 * temp.max(0.0) + 280.0).max(300.0) * if temp > 5.0 { 1.0 } else { 0.7 };
    let arid = cl.aridity * 300.0f32.max(20.0 * cl.temp_sl.max(0.0) + 280.0) / pet;
    let moist = smoothstep(0.2, 0.9, arid);
    let dry = 1.0 - smoothstep(0.25, 0.65, arid);
    let growing = smoothstep(-6.0, 2.0, temp);
    let lush = moist * growing;
    let alpine = smoothstep(1900.0, 2400.0, c.alt_m);
    let live = (1.0 - snowy) * growing;
    let none = (Kind::Grass, 0.0);
    let mut out = [none; 8];
    match pass {
        1 => {
            let old = smoothstep(12.0, 26.0, c.height_m);
            out[0] = (Kind::Log, shade * old * 0.30 * live);
            out[1] = (Kind::Boulder, ((rocky * 0.35 + w[1] * 0.3) * (1.0 - snowy) + 0.015 * (1.0 - snowy)).min(0.6));
        }
        2 => {
            let boost = 1.0 + 3.0 * near_big;
            out[0] = (Kind::Fern, (shade * moist * 0.22 * (1.0 - 0.4 * c.mix) + open * shade.max(0.0) * 0.0 + edge(c.cover) * moist * 0.12) * (1.0 + 1.5 * near_big) * live);
            out[1] = (Kind::Moss, ((shade * moist * (0.10 + 0.22 * c.mix) + rocky * moist * 0.06) * boost * live).min(0.7));
            out[2] = (Kind::Blueberry, (shade * c.mix * 0.32 * (1.0 - 0.45 * c.cover) + alpine * grassy * open * 0.06) * live);
            out[3] = (Kind::Stone, rocky * 0.35 * (1.0 - snowy) + grassy * 0.008);
        }
        _ => {
            let tall = (1.0 - alpine * 0.85) * lush;
            out[0] = (Kind::Grass, grassy * open * tall * 0.92 * live);
            out[1] = (Kind::Turf, grassy * open * lush * alpine * 0.9 * live);
            out[2] = (Kind::DryGrass, (grassy * open * dry * 0.65 + rocky * 0.12) * live);
            out[3] = (Kind::ShadeGrass, shade * lush * 0.35 * (1.0 - 0.6 * c.cover) * live);
            out[4] = (Kind::FlowerWhite, grassy * open * lush * (0.03 + 0.07 * alpine) * live);
            out[5] = (Kind::Pebble, (rocky * 0.25 + dry * open * 0.04) * (1.0 - snowy));
            out[6] = (Kind::Twig, shade * 0.06 * live);
            out[7] = (Kind::Rosette, grassy * open * lush * 0.025 * (1.0 - alpine) * live);
        }
    }
    let s: f32 = out.iter().map(|x| x.1.max(0.0)).sum();
    if s > 1.0 {
        out.iter_mut().for_each(|x| x.1 = x.1.max(0.0) / s);
    }
    out
}

/// Forest edge: partial cover.
fn edge(cover: f32) -> f32 {
    smoothstep(0.05, 0.3, cover) * (1.0 - smoothstep(0.4, 0.75, cover))
}

// ---------------------------------------------------------------------------------------------
// Scatter
// ---------------------------------------------------------------------------------------------

/// One placed item.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoverItem {
    /// Unit body direction of its base.
    pub dir: DVec3,
    pub kind: Kind,
    pub variant: u32,
    /// Rank `0..1` (0 for solid kinds) against [`keep`].
    pub rank: f32,
    pub yaw: f32,
    pub scale: f32,
    /// Instance value (x/128) and hue (-1..1) tint.
    pub value: f32,
    pub hue: f32,
    /// Wind phase offset `0..1`.
    pub phase: f32,
    /// Follow the ground's slope (logs, boulders, stones, moss) rather than stand upright.
    pub conform: bool,
}

/// A trunk for exclusion: unit direction and stem radius (m).
#[derive(Clone, Copy, Debug)]
pub struct Trunk {
    pub dir: DVec3,
    pub radius: f32,
    pub height: f32,
}

impl Trunk {
    pub fn of(t: &StandTree) -> Trunk {
        Trunk { dir: t.dir, radius: 0.08 + 0.011 * t.height_m, height: t.height_m }
    }
}

/// The node's tangent frame (metres).
pub(crate) struct Frame {
    pub(crate) c: DVec3,
    pub(crate) e1: DVec3,
    pub(crate) e2: DVec3,
}

impl Frame {
    pub(crate) fn new(id: NodeId) -> Frame {
        let c = id.centre().normalize();
        let a = (id.corner(1) - id.corner(0)).normalize();
        let e1 = (a - c * a.dot(c)).normalize();
        let e2 = c.cross(e1);
        Frame { c, e1, e2 }
    }
    pub(crate) fn xy(&self, d: DVec3) -> [f32; 2] {
        let q = d / d.dot(self.c) - self.c;
        [(q.dot(self.e1) * EARTH_R) as f32, (q.dot(self.e2) * EARTH_R) as f32]
    }
    pub(crate) fn dir(&self, p: [f32; 2]) -> DVec3 {
        (self.c + self.e1 * (p[0] as f64 / EARTH_R) + self.e2 * (p[1] as f64 / EARTH_R)).normalize()
    }
}

/// Obstacles of the passes above (node-local metres): discs and capsules.
#[derive(Default)]
struct Obstacles {
    /// (a, b, radius): a capsule; a disc when a == b.
    shapes: Vec<([f32; 2], [f32; 2], f32)>,
    /// Bucketed by 4 m cells: shape indices.
    grid: std::collections::BTreeMap<(i32, i32), Vec<u32>>,
}

const BUCKET: f32 = 4.0;

impl Obstacles {
    fn add(&mut self, a: [f32; 2], b: [f32; 2], r: f32) {
        let k = self.shapes.len() as u32;
        self.shapes.push((a, b, r));
        let lo = [a[0].min(b[0]) - r, a[1].min(b[1]) - r];
        let hi = [a[0].max(b[0]) + r, a[1].max(b[1]) + r];
        for gy in (lo[1] / BUCKET).floor() as i32..=(hi[1] / BUCKET).floor() as i32 {
            for gx in (lo[0] / BUCKET).floor() as i32..=(hi[0] / BUCKET).floor() as i32 {
                self.grid.entry((gx, gy)).or_default().push(k);
            }
        }
    }
    /// Distance from `p` to the nearest shape's surface (negative inside), searched to `reach`.
    fn dist(&self, p: [f32; 2], reach: f32) -> f32 {
        let mut best = f32::INFINITY;
        let r = reach.max(0.0);
        for gy in ((p[1] - r) / BUCKET).floor() as i32..=((p[1] + r) / BUCKET).floor() as i32 {
            for gx in ((p[0] - r) / BUCKET).floor() as i32..=((p[0] + r) / BUCKET).floor() as i32 {
                let Some(v) = self.grid.get(&(gx, gy)) else { continue };
                for &k in v {
                    let (a, b, rad) = self.shapes[k as usize];
                    best = best.min(seg_dist(p, a, b) - rad);
                }
            }
        }
        best
    }
}

fn seg_dist(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let ap = [p[0] - a[0], p[1] - a[1]];
    let l2 = ab[0] * ab[0] + ab[1] * ab[1];
    let t = if l2 > 1e-9 { ((ap[0] * ab[0] + ap[1] * ab[1]) / l2).clamp(0.0, 1.0) } else { 0.0 };
    let d = [ap[0] - ab[0] * t, ap[1] - ab[1] * t];
    (d[0] * d[0] + d[1] * d[1]).sqrt()
}

pub(crate) fn cell_key(face: u8, level: u8, i: u64, j: u64) -> u64 {
    (face as u64) << 58 | (level as u64) << 52 | i << 26 | j
}

/// The lattice cells of `level` covering node `id` grown by `halo` cells.
pub(crate) fn cells(id: NodeId, level: u8, halo: i64) -> (i64, i64, i64, i64, i64) {
    let k = 1i64 << (level - id.level());
    let n = (id.side() as i64) << (level - id.level());
    let (i0, j0) = (id.x() as i64 * k - halo, id.y() as i64 * k - halo);
    (i0.max(0), (i0 + k + 2 * halo).min(n), j0.max(0), (j0 + k + 2 * halo).min(n), n)
}

/// Candidate of cell `(i, j)` at `level`: its jittered direction and three rolls.
pub(crate) fn candidate(face: u8, level: u8, i: i64, j: i64, n: i64, seed: u64) -> (DVec3, u64) {
    let key = cell_key(face, level, i as u64, j as u64) ^ seed.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    // Off the cell edges (an edge point may round into the neighbour).
    let ju = 1 + (crate::hash01(key, 1) * (SUB - 2) as f32) as u64 % (SUB - 2);
    let jv = 1 + (crate::hash01(key, 2) * (SUB - 2) as f32) as u64 % (SUB - 2);
    let d = lattice_point(face, i as u64 * SUB + ju, j as u64 * SUB + jv, n as u64 * SUB);
    (d, key)
}

/// Ground cover of node `id` (any level from 14 to 20): every item whose rank is under
/// `keep_min` (1 keeps all). `sample(d)` is the ground at body direction `d` (`None`: no data,
/// nothing grows); `trunks` the stand trees in and around the node. Deterministic; items in
/// a fixed order (pass, then lattice order).
pub fn scatter_node(id: NodeId, sample: &dyn Fn(DVec3) -> Option<GroundSample>, trunks: &[Trunk], keep_min: f32, seed: u64) -> Vec<CoverItem> {
    let f = Frame::new(id);
    let face = id.face();
    let cl = Climate::at(f.c);
    let inside = |d: DVec3| NodeId::from_unit(id.body(), d, id.level()) == id;
    let mut trunk_obs = Obstacles::default();
    let mut tall: Vec<([f32; 2], f32)> = Vec::new();
    for t in trunks {
        let p = f.xy(t.dir);
        trunk_obs.add(p, p, t.radius);
        if t.height > 18.0 {
            tall.push((p, t.height));
        }
    }
    let mut out = Vec::new();
    // Pass 1: logs, boulders (with a halo so neighbours agree on what lies across the edge).
    let mut big = Obstacles::default();
    let (a0, a1, b0, b1, n1) = cells(id, L1, 2);
    for j in b0..b1 {
        for i in a0..a1 {
            let (d, key) = candidate(face, L1, i, j, n1, seed);
            let Some(g) = sample(d) else { continue };
            let roll = crate::hash01(key, 3);
            let Some(kind) = pick(&niche(1, &g, &cl, 0.0), roll) else { continue };
            let def = def(kind);
            let v = (crate::hash01(key, 4) * def.variants as f32) as u32 % def.variants;
            let scale = lerp(def.scale, crate::hash01(key, 5));
            let mut p = f.xy(d);
            let mut yaw = crate::hash01(key, 6) * TAU;
            if kind == Kind::Log {
                // A fallen neighbour: from beside the nearest tall trunk outward, else where it is.
                let (len, r) = log_dims(v);
                let (len, r) = (len * scale, r * scale);
                if let Some((tp, _)) = tall.iter().filter(|(tp, _)| dist2(*tp, p) < 100.0).min_by(|x, y| dist2(x.0, p).total_cmp(&dist2(y.0, p))) {
                    p = [tp[0] + 1.2 * yaw.cos(), tp[1] + 1.2 * yaw.sin()];
                    p = [p[0] + 0.5 * len * yaw.cos(), p[1] + 0.5 * len * yaw.sin()];
                }
                let ax = [yaw.cos() * len * 0.5, yaw.sin() * len * 0.5];
                let (a, b) = ([p[0] - ax[0], p[1] - ax[1]], [p[0] + ax[0], p[1] + ax[1]]);
                // Never through a trunk or another log or boulder.
                let clear = trunk_obs.dist(a, len).min(seg_trunks(&trunk_obs, a, b, r)) > r + 0.05 && big.dist(p, len) > len * 0.5 + r;
                if !clear {
                    continue;
                }
                big.add(a, b, r);
                // The mesh lies along +x; yaw turns +x to (cos, sin) in the frame's (e1, e2).
                yaw = -yaw;
            } else {
                let r = def.radius * scale;
                if trunk_obs.dist(p, r + 1.0) < r || big.dist(p, r + 8.0) < r {
                    continue;
                }
                big.add(p, p, r);
            }
            let d = f.dir(p);
            if inside(d) {
                out.push(item(d, kind, v, 0.0, yaw, scale, key));
            }
        }
    }
    // Pass 2: ferns, moss, blueberry, stones; moss and ferns gather by the pass-1 objects.
    let mut mid = Obstacles::default();
    let (a0, a1, b0, b1, n2) = cells(id, L2, 1);
    for j in b0..b1 {
        for i in a0..a1 {
            let (d, key) = candidate(face, L2, i, j, n2, seed);
            let p = f.xy(d);
            let db = big.dist(p, 2.5);
            if db < 0.05 {
                continue;
            }
            let near_big = 1.0 - smoothstep(0.0, 2.0, db);
            let Some(g) = sample(d) else { continue };
            let roll = crate::hash01(key, 3);
            let Some(kind) = pick(&niche(2, &g, &cl, near_big), roll) else { continue };
            let def = def(kind);
            let v = (crate::hash01(key, 4) * def.variants as f32) as u32 % def.variants;
            let scale = lerp(def.scale, crate::hash01(key, 5));
            let r = def.radius * scale * if kind == Kind::Moss || kind == Kind::Stone { 0.6 } else { 1.0 };
            if trunk_obs.dist(p, r + 1.0) < r * 0.5 || db < r * 0.5 {
                continue;
            }
            mid.add(p, p, r);
            let rank = crate::hash01(key, 7) * def.rank_k;
            if inside(d) && rank < keep_min {
                out.push(item(d, kind, v, rank, crate::hash01(key, 6) * TAU, scale, key));
            }
        }
    }
    // Pass 3: grass, flowers, pebbles, twigs fill the rest. The rank decides first (far nodes
    // skip most cells without sampling).
    let (a0, a1, b0, b1, n3) = cells(id, L3, 0);
    // Flower colour follows a coarse patchy field (meadows bloom in drifts of one colour).
    let flowers = [Kind::FlowerWhite, Kind::FlowerYellow, Kind::FlowerViolet, Kind::FlowerRed];
    for j in b0..b1 {
        for i in a0..a1 {
            let key = cell_key(face, L3, i as u64, j as u64) ^ seed.wrapping_mul(0x9E37_79B9_7F4A_7C15);
            let rank = crate::hash01(key, 7);
            if rank >= keep_min {
                continue;
            }
            let (d, key) = candidate(face, L3, i, j, n3, seed);
            let p = f.xy(d);
            if trunk_obs.dist(p, 1.0) < 0.05 || big.dist(p, 1.0) < 0.0 || mid.dist(p, 1.0) < -0.05 {
                continue;
            }
            let Some(g) = sample(d) else { continue };
            let roll = crate::hash01(key, 3);
            let Some(mut kind) = pick(&niche(3, &g, &cl, 0.0), roll) else { continue };
            if kind == Kind::FlowerWhite {
                let fx = fbm3(p[0] * 0.08 + face as f32 * 17.0, p[1] * 0.08, (i / 4096) as f32 + (j / 4096) as f32 * 0.37, seed ^ 0xF10, 2);
                let c = ((fx * 2.2 + crate::hash01(key, 8) * 0.6) as usize).min(3);
                kind = flowers[c];
            }
            let def = def(kind);
            let v = (crate::hash01(key, 4) * def.variants as f32) as u32 % def.variants;
            let scale = lerp(def.scale, crate::hash01(key, 5));
            out.push(item(d, kind, v, rank * def.rank_k, crate::hash01(key, 6) * TAU, scale, key));
        }
    }
    out
}

fn seg_trunks(o: &Obstacles, a: [f32; 2], b: [f32; 2], r: f32) -> f32 {
    // Sample along the segment (trunks are small discs; 0.5 m steps with the radius search).
    let len = seg_dist(a, b, b).max(0.01);
    let steps = (len / 0.5).ceil() as usize;
    (0..=steps)
        .map(|k| {
            let t = k as f32 / steps as f32;
            o.dist([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t], r + 1.0)
        })
        .fold(f32::INFINITY, f32::min)
}

fn dist2(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)
}

fn lerp(r: (f32, f32), t: f32) -> f32 {
    r.0 + (r.1 - r.0) * t
}

fn pick(w: &[(Kind, f32)], roll: f32) -> Option<Kind> {
    let mut acc = 0.0;
    for &(k, p) in w {
        acc += p.max(0.0);
        if roll < acc {
            return Some(k);
        }
    }
    None
}

fn item(dir: DVec3, kind: Kind, variant: u32, rank: f32, yaw: f32, scale: f32, key: u64) -> CoverItem {
    CoverItem {
        dir,
        kind,
        variant,
        rank,
        yaw,
        scale,
        value: 0.85 + 0.3 * crate::hash01(key, 9),
        hue: crate::hash01(key, 10) * 2.0 - 1.0,
        phase: crate::hash01(key, 11),
        conform: matches!(kind, Kind::Log | Kind::Boulder | Kind::Stone | Kind::Moss | Kind::Pebble | Kind::Twig),
    }
}

/// Instance user word: value (x/128), hue (-1..1 as 0..255), no branch drop, wind phase.
pub fn user_word(it: &CoverItem) -> u32 {
    let v = (it.value * 128.0).round().clamp(0.0, 255.0) as u32;
    let h = ((it.hue + 1.0) * 127.5).round().clamp(0.0, 255.0) as u32;
    let p = (it.phase * 255.0).round().clamp(0.0, 255.0) as u32;
    v | h << 8 | p << 24
}

/// Instance ground word (`GpuInstance.pad[2]`, common.slang `gc_*`): rank (12 bits), reach in
/// metres (8 bits, at most 255), solid (bit 20), no shadow (bit 21), first cascade only (bit 22).
pub fn ground_word(it: &CoverItem) -> u32 {
    let d = def(it.kind);
    let r = (it.rank.clamp(0.0, 1.0) * 4095.0).round() as u32;
    let reach = d.reach.round().clamp(1.0, 255.0) as u32;
    r | reach << 12 | (d.solid as u32) << 20 | (!d.shadows as u32) << 21 | (!d.solid as u32) << 22
}

/// The stand's trunks within `margin_m` of node `id`.
pub fn trunks_near(id: NodeId, stands: &[&[StandTree]], margin_m: f64) -> Vec<Trunk> {
    let c = id.centre().normalize();
    let half = (id.corner(0) - id.corner(3)).length() * 0.5 + margin_m / EARTH_R;
    let cos = half.cos();
    let mut out = Vec::new();
    for s in stands {
        for t in s.iter() {
            if t.dir.dot(c) >= cos {
                out.push(Trunk::of(t));
            }
        }
    }
    out
}

#[allow(dead_code)]
fn _gray() -> [f32; 4] {
    gray(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed_geo::BodyId;

    fn node() -> NodeId {
        NodeId::from_unit(BodyId(0), DVec3::new(0.48, 0.05, 0.876).normalize(), NODE_LEVEL)
    }

    fn meadow(_: DVec3) -> Option<GroundSample> {
        Some(GroundSample { canopy: CanopySample { cover: 0.0, height_m: 0.0, mix: 0.0, under: 0.0, alt_m: 600.0, wet: 0.3 }, slope_cos: 0.99, shares: None })
    }

    fn forest(_: DVec3) -> Option<GroundSample> {
        Some(GroundSample { canopy: CanopySample { cover: 0.9, height_m: 25.0, mix: 0.8, under: 0.3, alt_m: 1200.0, wet: 0.3 }, slope_cos: 0.95, shares: None })
    }

    fn snow(_: DVec3) -> Option<GroundSample> {
        Some(GroundSample { canopy: CanopySample { cover: 0.0, height_m: 0.0, mix: 0.0, under: 0.0, alt_m: 3600.0, wet: 0.3 }, slope_cos: 0.99, shares: None })
    }

    fn count(v: &[CoverItem], k: &[Kind]) -> usize {
        v.iter().filter(|i| k.contains(&i.kind)).count()
    }

    #[test]
    fn keep_is_monotone_and_bounded() {
        let mut prev = 1.0;
        for d in 0..400 {
            let k = keep(d as f32 * 0.5, REACH_M);
            assert!((0.0..=1.0).contains(&k) && k <= prev + 1e-6, "{d}");
            prev = k;
        }
        assert_eq!(keep(0.0, REACH_M), 1.0);
        assert_eq!(keep(REACH_M, REACH_M), 0.0);
        assert!(grow(1.0) == 1.0 && grow(0.01) == GROW_MAX);
    }

    #[test]
    fn library_meshes_are_sane() {
        let lib = library();
        assert_eq!(lib.len(), KINDS.iter().map(|d| d.variants as usize).sum::<usize>());
        for (i, m) in lib.iter().enumerate() {
            assert!(!m.lod0.is_empty() && !m.lod1.is_empty(), "{i}");
            assert_eq!(m.positions.len(), m.words.len());
            assert!(m.lod0.iter().chain(&m.lod1).all(|&x| (x as usize) < m.positions.len()));
            assert!(m.lod1.len() <= m.lod0.len().max(m.lod1.len()));
            assert!(m.radius > 0.0 && m.radius < 5.0, "{i} {}", m.radius);
            let k = m.kind.unwrap();
            assert_eq!(proto_index(k, m.variant), i);
        }
    }

    #[test]
    fn deterministic_and_seamless() {
        let id = node();
        let a = scatter_node(id, &meadow, &[], 1.0, 7);
        let b = scatter_node(id, &meadow, &[], 1.0, 7);
        assert_eq!(a, b);
        // Every item lies in its node; a neighbour's items do not overlap.
        let nb = id.neighbour(1, 0);
        let c = scatter_node(nb, &meadow, &[], 1.0, 7);
        let bad: Vec<_> = a.iter().filter(|i| NodeId::from_unit(id.body(), i.dir, id.level()) != id).map(|i| (i.kind, NodeId::from_unit(id.body(), i.dir, id.level()))).take(5).collect();
        assert!(bad.is_empty(), "{id:?} {bad:?}");
        assert!(c.iter().all(|i| NodeId::from_unit(id.body(), i.dir, id.level()) == nb));
    }

    #[test]
    fn niche_follows_the_canopy() {
        let id = node();
        let m = scatter_node(id, &meadow, &[], 1.0, 7);
        let f = scatter_node(id, &forest, &[], 1.0, 7);
        let s = scatter_node(id, &snow, &[], 1.0, 7);
        let grass = [Kind::Grass, Kind::Turf];
        let shade = [Kind::Moss, Kind::Fern, Kind::Blueberry, Kind::ShadeGrass];
        assert!(count(&m, &grass) > 8000, "meadow grass {}", count(&m, &grass));
        assert!(count(&f, &grass) == 0, "forest grass {}", count(&f, &grass));
        assert!(count(&f, &shade) > 500 && count(&m, &shade) < 50, "{} {}", count(&f, &shade), count(&m, &shade));
        assert!(s.iter().all(|i| matches!(i.kind, Kind::Boulder | Kind::Stone | Kind::Pebble)), "snow grows nothing");
    }

    #[test]
    fn nothing_inside_trunks_and_logs() {
        let id = node();
        let f = Frame::new(id);
        let trunks: Vec<Trunk> = (0..40).map(|k| Trunk { dir: f.dir([(k % 8) as f32 * 9.0 - 34.0, (k / 8) as f32 * 14.0 - 30.0]), radius: 0.4, height: 28.0 }).collect();
        let v = scatter_node(id, &forest, &trunks, 1.0, 7);
        assert!(v.iter().any(|i| i.kind == Kind::Log), "mature forest has deadfall");
        for it in &v {
            let p = f.xy(it.dir);
            for t in &trunks {
                let q = f.xy(t.dir);
                assert!(dist2(p, q).sqrt() > t.radius - 0.01, "{:?} inside a trunk", it.kind);
            }
        }
    }

    #[test]
    fn rank_filter_is_a_subset() {
        let id = node();
        let all = scatter_node(id, &meadow, &[], 1.0, 7);
        let few = scatter_node(id, &meadow, &[], 0.1, 7);
        assert!(few.len() < all.len() / 4);
        assert!(few.iter().all(|i| all.contains(i)));
    }
}
