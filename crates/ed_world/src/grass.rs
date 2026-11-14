//! Grass style A (Ghost-of-Tsushima-style procedural grass): deterministic **clump records**
//! per ground node; the renderer expands every clump into curved blades on the GPU from the
//! clump's seed and its [`GrassType`] row (`ed_render::grass`, `shaders/grass` in
//! visibility.slang and resolve.slang). No meshes: a clump is 32 bytes.
//!
//! **Clumps.** One candidate per cell of a global cube-face lattice (level [`CLUMP_LEVEL`],
//! ~0.6 m, so ~0.36 m² per clump), jittered inside its cell; row crops (`aniso < 1`) are jittered
//! too and face a field direction that turns slowly across the land (~200 m), never a grid. What grows
//! comes from a caller's type function (real ground: [`type_from_ground`]; the grass lab:
//! [`lab_type`]); a clump exists when its hash is under the type's density times the
//! ground's. Height varies in patches (a ~9 m noise field) and per clump; a slower field
//! makes some patches drier (straw-tinted). Where grass grows and which type it is come from
//! the shared ground field (`gf_*`), which the terrain carpet mirrors.
//!
//! **Distance** ([`lod`], mirrored by common.slang `grass_lod`): blades per clump fall as
//! `(r0 / d)^2` past `r0` (constant on-screen density), down to a floor (`bmin`; style E lowers
//! it with distance to 1-3 wide blades, [`LodParams`]); past that the clumps themselves thin by
//! rank. Ranks are stratified ([`clump_rank`]): a keep of `4^-m` leaves an even lattice of
//! clumps `2^m` cells apart, each jittered over its block, so far grass is fewer, bigger clumps
//! with no holes. The generator emits only ranks under the node's keep (a tier of
//! [`clump_keep`] at its nearest point), so far nodes carry few clumps.
//!
//! Deterministic: a pure function of (node, seed, type function); fixed lattice order, seeded
//! hashes only, no maps.

use crate::ground_cover::{Climate, Frame, GroundSample, SUB, candidate, cell_key, cells, layer_weights};
use crate::noise::{fbm3, smoothstep, value3};
use ed_geo::NodeId;
use ed_geo::node::lattice_point;
use glam::DVec3;
use std::f32::consts::TAU;

/// Grass nodes are this level (~76 m), as the ground cover's.
pub const NODE_LEVEL: u8 = 17;
/// Clump lattice level (~0.6 m cells).
pub const CLUMP_LEVEL: u8 = 24;
/// Default full-density radius (m): every blade of a clump draws inside it.
pub const R0_M: f32 = 14.0;
/// Default range (m): nothing past it (the terrain colour carries the grass).
pub const RANGE_M: f32 = 110.0;
/// Fewest blades a clump draws before clumps themselves thin.
pub const BMIN: f32 = 5.0;
/// Most blades in a clump (the renderer's largest bucket).
pub const MAX_BLADES: u32 = 64;

/// One grass type: a row of the GPU table (common.slang `GrassType`). Colours are linear
/// albedo. Data, not code: the shader has no per-type branch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GrassType {
    pub name: &'static str,
    /// Blade height range (m) at height scale 1.
    pub height: (f32, f32),
    /// Blade width at the root (m).
    pub width: f32,
    /// Blades per clump at full density (at most [`MAX_BLADES`]).
    pub blades: u32,
    /// Clump radius (m) along its facing.
    pub clump_r: f32,
    /// Across / along spread of the clump (1: round; row crops are narrow).
    pub aniso: f32,
    /// Most lean from vertical (radians).
    pub lean: f32,
    /// Wind stiffness 0..1 (1 does not move).
    pub stiffness: f32,
    /// Curvature 0..1 (0: straight, 1: bends over from the root up).
    pub curve: f32,
    pub root: [f32; 3],
    pub mid: [f32; 3],
    pub tip: [f32; 3],
    /// Dry patches tint towards this.
    pub dry: [f32; 3],
    /// Seed head: share of the blade at the tip (0: none) and its width over the root's.
    pub head: f32,
    pub head_w: f32,
    /// Share of lattice cells holding a clump (0..1).
    pub density: f32,
    /// Shell grass (styles C and D): turf height (m) at height scale 1. Tall types use it as
    /// the short underlayer between their clumps.
    pub shell_h: f32,
    /// Strand cell (m) of the shells: a power of two (`2^-k`), so the strand lattice is
    /// world-stable under the 32 m map snap ([`MAP_SNAP_M`]).
    pub strand_m: f32,
    /// Share of strand cells holding a strand, and of the shell floor that is grass (0..1).
    pub shell_cover: f32,
    /// Specular and sky-sheen scale on the blades and strands (0..1; 1 the plain leaf model).
    pub sheen: f32,
    /// What a field of it reads as from afar (linear albedo): the terrain under and past the
    /// grass takes this colour so the grass edge blends.
    pub carpet: [f32; 3],
    /// Shells only (style D draws no blades of it).
    pub shell_only: bool,
}

pub const LAWN: u8 = 0;
pub const MEADOW: u8 = 1;
pub const LONG: u8 = 2;
pub const WHEAT: u8 = 3;
pub const SEDGE: u8 = 4;
pub const TUFT: u8 = 5;
pub const TUNDRA: u8 = 6;

/// The type table (index = `kind`).
pub const TYPES: [GrassType; 7] = [
    GrassType {
        name: "lawn",
        height: (0.12, 0.18),
        width: 0.022,
        blades: 64,
        clump_r: 0.42,
        aniso: 1.0,
        lean: 0.75,
        stiffness: 0.85,
        curve: 0.35,
        root: [0.030, 0.050, 0.012],
        mid: [0.065, 0.110, 0.022],
        tip: [0.085, 0.135, 0.030],
        dry: [0.20, 0.18, 0.08],
        head: 0.0,
        head_w: 1.0,
        density: 0.85,
        shell_h: 0.075,
        strand_m: 0.0078125,
        shell_cover: 0.92,
        sheen: 0.25,
        carpet: [0.058, 0.098, 0.020],
        shell_only: true,
    },
    GrassType {
        name: "meadow",
        height: (0.22, 0.50),
        width: 0.0115,
        blades: 64,
        clump_r: 0.44,
        aniso: 1.0,
        lean: 0.55,
        stiffness: 0.55,
        curve: 0.6,
        root: [0.022, 0.034, 0.010],
        mid: [0.070, 0.120, 0.026],
        tip: [0.150, 0.190, 0.060],
        dry: [0.26, 0.22, 0.10],
        head: 0.0,
        head_w: 1.0,
        density: 0.8,
        shell_h: 0.06,
        strand_m: 0.0078125,
        shell_cover: 0.8,
        sheen: 0.55,
        carpet: [0.068, 0.110, 0.026],
        shell_only: false,
    },
    GrassType {
        name: "long grass",
        height: (0.55, 0.95),
        width: 0.0125,
        blades: 56,
        clump_r: 0.42,
        aniso: 1.0,
        lean: 0.7,
        stiffness: 0.35,
        curve: 0.85,
        root: [0.022, 0.030, 0.010],
        mid: [0.080, 0.118, 0.030],
        tip: [0.200, 0.200, 0.085],
        dry: [0.30, 0.25, 0.12],
        head: 0.0,
        head_w: 1.0,
        density: 0.78,
        shell_h: 0.06,
        strand_m: 0.015625,
        shell_cover: 0.75,
        sheen: 0.6,
        carpet: [0.085, 0.112, 0.036],
        shell_only: false,
    },
    GrassType {
        name: "wheat",
        height: (0.80, 0.95),
        width: 0.0075,
        blades: 48,
        clump_r: 0.32,
        aniso: 0.3,
        lean: 0.12,
        stiffness: 0.5,
        curve: 0.2,
        root: [0.10, 0.085, 0.030],
        mid: [0.36, 0.28, 0.105],
        tip: [0.50, 0.38, 0.150],
        dry: [0.45, 0.36, 0.16],
        head: 0.13,
        head_w: 2.2,
        density: 1.0,
        shell_h: 0.035,
        strand_m: 0.015625,
        shell_cover: 0.45,
        sheen: 0.8,
        carpet: [0.30, 0.225, 0.085],
        shell_only: false,
    },
    GrassType {
        name: "sedge",
        height: (0.6, 1.1),
        width: 0.012,
        blades: 40,
        clump_r: 0.35,
        aniso: 1.0,
        lean: 0.35,
        stiffness: 0.6,
        curve: 0.25,
        root: [0.035, 0.05, 0.015],
        mid: [0.085, 0.11, 0.03],
        tip: [0.15, 0.15, 0.06],
        dry: [0.24, 0.2, 0.09],
        head: 0.12,
        head_w: 1.6,
        density: 0.8,
        shell_h: 0.1,
        strand_m: 0.0078125,
        shell_cover: 0.8,
        sheen: 0.55,
        carpet: [0.085, 0.105, 0.035],
        shell_only: false,
    },
    GrassType {
        name: "tuft",
        height: (0.25, 0.5),
        width: 0.008,
        blades: 28,
        clump_r: 0.22,
        aniso: 1.0,
        lean: 0.6,
        stiffness: 0.7,
        curve: 0.4,
        root: [0.1, 0.085, 0.04],
        mid: [0.2, 0.17, 0.08],
        tip: [0.3, 0.26, 0.13],
        dry: [0.3, 0.25, 0.12],
        head: 0.0,
        head_w: 1.0,
        density: 0.12,
        shell_h: 0.0,
        strand_m: 0.0078125,
        shell_cover: 0.0,
        sheen: 0.55,
        carpet: [0.22, 0.17, 0.1],
        shell_only: false,
    },
    GrassType {
        name: "tundra",
        // Tussock tundra (Toolik: cottongrass and sedge tussocks, 20-40 cm, olive to russet-gold
        // with dead straw in every tuft): tight tussocks, stiff, darker gaps between them.
        height: (0.2, 0.38),
        width: 0.009,
        blades: 44,
        clump_r: 0.2,
        aniso: 1.0,
        lean: 0.45,
        stiffness: 0.8,
        curve: 0.3,
        root: [0.035, 0.032, 0.014],
        mid: [0.10, 0.09, 0.038],
        tip: [0.26, 0.19, 0.075],
        dry: [0.28, 0.20, 0.10],
        head: 0.0,
        head_w: 1.0,
        density: 0.6,
        shell_h: 0.05,
        strand_m: 0.0078125,
        shell_cover: 0.8,
        sheen: 0.45,
        carpet: [0.085, 0.072, 0.034],
        shell_only: false,
    },
];

/// Words per GPU row (eight float4).
pub const ROW_FLOATS: usize = 32;

/// The GPU row of `t` (common.slang `GrassType`): (h min, h max, width, blades),
/// (clump r, lean, stiffness, curve), (root, aniso), (mid, head), (tip, head width),
/// (dry, density), (shell h, strand m, shell cover, sheen), (carpet, shell only).
pub fn gpu_row(t: &GrassType) -> [f32; ROW_FLOATS] {
    let [r, m, p, d, c] = [t.root, t.mid, t.tip, t.dry, t.carpet];
    [
        t.height.0, t.height.1, t.width, t.blades.min(MAX_BLADES) as f32,
        t.clump_r, t.lean, t.stiffness, t.curve,
        r[0], r[1], r[2], t.aniso,
        m[0], m[1], m[2], t.head,
        p[0], p[1], p[2], t.head_w,
        d[0], d[1], d[2], t.density,
        t.shell_h, t.strand_m, t.shell_cover, t.sheen,
        c[0], c[1], c[2], if t.shell_only { 1.0 } else { 0.0 },
    ]
}

/// Tallest shell (m): the top shell's height (the patch field scales heights up to
/// [`HEIGHT_SCALE_MAX`]).
pub fn shell_h_max() -> f32 {
    TYPES.iter().map(|t| t.shell_h).fold(0.0, f32::max) * HEIGHT_SCALE_MAX
}

/// Every type's GPU row, in order.
pub fn gpu_rows() -> Vec<[f32; ROW_FLOATS]> {
    TYPES.iter().map(gpu_row).collect()
}

/// Distance rules shared by the node tiers (here) and the GPU (common.slang `grass_lod`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LodParams {
    /// Full-density radius (m).
    pub r0: f32,
    /// Fewest blades per clump near (before clumps thin).
    pub bmin: f32,
    /// Fewest blades per clump far (style E: far clumps carry 1-3 wide blades). `bfar = bmin`
    /// keeps the near rule everywhere (style A).
    pub bfar: f32,
    /// The fewest-blades floor falls as `bmin * dfar / d` past `dfar` (m), down to `bfar`
    /// (0: off).
    pub dfar: f32,
}

impl LodParams {
    /// Style A's rule: `bmin` everywhere.
    pub const fn near(r0: f32, bmin: f32) -> LodParams {
        LodParams { r0, bmin, bfar: bmin, dfar: 0.0 }
    }
}

/// Continuous blades per clump and the clump keep at distance `d` (m) for a type of `blades`
/// blades: `blades * min(1, (r0/d)^2)` down to the floor `bm(d)`, then clumps keep
/// `rank < keep` with `keep = blades * (r0/d)^2 / bm(d)` (the on-screen blade density holds,
/// spread over more, fewer-bladed clumps far). Mirrored by common.slang `grass_lod`.
pub fn lod_p(d: f32, blades: f32, p: &LodParams) -> (f32, f32) {
    let d = d.max(1e-3);
    let k = (p.r0 / d).powi(2).min(1.0);
    let raw = blades * k;
    let floor = if p.dfar > 0.0 { (p.bmin * p.dfar / d).clamp(p.bfar.min(p.bmin), p.bmin) } else { p.bmin };
    let bm = floor.min(blades).max(1e-3);
    (raw.max(bm), (raw / bm).min(1.0))
}

/// [`lod_p`] with style A's rule.
pub fn lod(d: f32, blades: f32, r0: f32, bmin: f32) -> (f32, f32) {
    lod_p(d, blades, &LodParams::near(r0, bmin))
}

/// The keep a node needs for clumps up to `d` m away (the type with most blades thins last).
pub fn clump_keep(d: f32, r0: f32) -> f32 {
    clump_keep_p(d, &LodParams::near(r0, BMIN))
}

/// [`clump_keep`] under any rule.
pub fn clump_keep_p(d: f32, p: &LodParams) -> f32 {
    let most = TYPES.iter().map(|t| t.blades).max().unwrap_or(1) as f32;
    lod_p(d, most, p).1
}

/// Rank levels of the stratified lattice ([`clump_rank`]).
pub const RANK_LEVELS: u32 = 5;

/// Coarseness of lattice cell `(i, j)`: how many times both indices halve evenly (capped at
/// [`RANK_LEVELS`] - 1). Cells of level `>= m` form a regular lattice `2^m` cells apart.
pub fn cell_level(i: i64, j: i64) -> u32 {
    let tz = |x: i64| if x == 0 { 63 } else { x.trailing_zeros() };
    tz(i).min(tz(j)).min(RANK_LEVELS - 1)
}

/// Stratified rank of a cell of level `l` (0..1) from a uniform roll `h`: level `l` cells take
/// ranks in `[4^-(l+1), 4^-l)` (the coarsest level `[0, 4^-l)`), so the clumps under a keep
/// of `4^-m` are exactly the level `>= m` cells: an even lattice, not a random thinning (far
/// grass keeps no holes and no clusters). The share under any keep stays the keep.
pub fn clump_rank(l: u32, h: f32) -> f32 {
    let hi = 0.25f32.powi(l as i32);
    let lo = if l + 1 >= RANK_LEVELS { 0.0 } else { hi * 0.25 };
    lo + (hi - lo) * h
}

/// The coarsest lattice step (cells) holding every rank under `keep`.
fn rank_step(keep: f32) -> i64 {
    let mut m = 0;
    while m + 1 < RANK_LEVELS && keep <= 0.25f32.powi(m as i32 + 1) {
        m += 1;
    }
    1 << m
}

/// One clump.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Clump {
    /// Unit body direction of its centre.
    pub dir: DVec3,
    /// Node-frame position (m; the frame of [`node_dir`]).
    pub xy: [f32; 2],
    /// Unit body-space tangent it faces (rows run along it).
    pub facing: DVec3,
    pub kind: u8,
    /// Rank 0..1 against the distance keep.
    pub rank: f32,
    pub seed: u32,
    /// Height scale (patch field times jitter; ~0.5..1.5).
    pub height: f32,
    /// Dry patch share 0..1.
    pub dry: f32,
}

/// Body direction of node-frame point `xy` (m) of node `id` (the frame [`Clump::xy`] uses).
pub fn node_dir(id: NodeId, xy: [f32; 2]) -> DVec3 {
    Frame::new(id).dir(xy)
}

/// Node-frame point (m) of body direction `d`.
pub fn node_xy(id: NodeId, d: DVec3) -> [f32; 2] {
    Frame::new(id).xy(d)
}

const EARTH_R: f64 = 6_371_008.8;

/// Metres along the body surface, wrapped to keep f32 noise inputs small (seams every 4 km).
fn wrap_m(d: DVec3) -> [f32; 3] {
    let p = d * EARTH_R;
    [p.x.rem_euclid(4096.0) as f32, p.y.rem_euclid(4096.0) as f32, p.z.rem_euclid(4096.0) as f32]
}

/// Most patch height scale ([`patch`]).
pub const HEIGHT_SCALE_MAX: f32 = 1.45;

/// The patch fields at body direction `d`: height scale (0.55..[`HEIGHT_SCALE_MAX`], patches
/// of ~9, ~35 and ~90 m, so a meadow has tall swales and short knolls at several sizes) and dry
/// share (0..1, ~23 m patches). Shared by the clumps and the shell map.
pub fn patch(d: DVec3, seed: u64) -> (f32, f32) {
    let w = wrap_m(d);
    let p = 0.5 * fbm3(w[0] / 9.0, w[1] / 9.0, w[2] / 9.0, seed ^ 0x6A55, 3)
        + 0.3 * value3(w[0] / 35.0, w[1] / 35.0, w[2] / 35.0, seed ^ 0x6A56)
        + 0.2 * value3(w[0] / 90.0, w[1] / 90.0, w[2] / 90.0, seed ^ 0x6A57);
    let dry = smoothstep(0.58, 0.8, fbm3(w[0] / 23.0, w[1] / 23.0, w[2] / 23.0, seed ^ 0xD27, 3));
    (0.55 + 0.9 * smoothstep(0.28, 0.72, p), dry)
}

// ---------------------------------------------------------------------------------------------
// The ground field (grass G4b): one definition of where grass is, shared bit for bit with the
// terrain carpet (terrain_common.slang `gf_*`), so green ground and blades share a footprint.
// Every input is face metres (`face_metres`, wrapped to `FACE_PERIOD_M`), which the shader
// has as `d.tex.xy + (g - 32) * cell`.
// ---------------------------------------------------------------------------------------------

/// Hash of a lattice cell and a salt to two values 0..1 (low and high 16 bits;
/// terrain_common.slang `gf_hash2`).
fn gf_hash2(cx: i32, cy: i32, salt: u32) -> [f32; 2] {
    let mut h = (cx as u32).wrapping_mul(0x8da6_b343) ^ (cy as u32).wrapping_mul(0xd816_3841) ^ salt.wrapping_mul(0x9E37_79B1);
    h ^= h >> 13;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 16;
    [(h & 0xFFFF) as f32 / 65535.0, (h >> 16) as f32 / 65535.0]
}

/// Two channels of periodic value noise (period 4096 m; `cell` divides it) in 0..1 on one
/// lattice (`gf_noise2`): one hash per corner feeds both.
pub fn gf_noise2(x: f32, y: f32, cell: f32, salt: u32) -> [f32; 2] {
    let (qx, qy) = (x / cell, y / cell);
    let (ix, iy) = (qx.floor(), qy.floor());
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let (fx, fy) = (s(qx - ix), s(qy - iy));
    let per = (4096.0 / cell + 0.5) as i32;
    let (cx, cy) = ((ix as i32).rem_euclid(per), (iy as i32).rem_euclid(per));
    let (dx, dy) = ((cx + 1) % per, (cy + 1) % per);
    let (a, b, c, d) = (gf_hash2(cx, cy, salt), gf_hash2(dx, cy, salt), gf_hash2(cx, dy, salt), gf_hash2(dx, dy, salt));
    let l = |a: f32, b: f32, t: f32| a + (b - a) * t;
    std::array::from_fn(|k| l(l(a[k], b[k], fx), l(c[k], d[k], fx), fy))
}

/// Periodic value noise in 0..1 (the first channel of [`gf_noise2`]).
pub fn gf_noise(x: f32, y: f32, cell: f32, salt: u32) -> f32 {
    gf_noise2(x, y, cell, salt)[0]
}

/// Domain warp (m) of the land-cover lookups: ~9 m over 64 m, ~3.5 m over 16 m and ~1.2 m
/// over 4 m, so the 10 m land-cover cells and page lattices never show as lines or squares.
pub fn gf_warp(x: f32, y: f32) -> [f32; 2] {
    let (a, b, c) = (gf_noise2(x, y, 64.0, 11), gf_noise2(x, y, 16.0, 12), gf_noise2(x, y, 4.0, 13));
    std::array::from_fn(|k| 18.0 * (a[k] - 0.5) + 7.0 * (b[k] - 0.5) + 2.4 * (c[k] - 0.5))
}

/// Threshold dither 0.2..0.8 (patches of ~7 and 25 m; the 4 m warp octave rags their edges):
/// partial grass is patches, not a uniform thin layer.
pub fn gf_dither(x: f32, y: f32) -> f32 {
    let n = 0.65 * gf_noise2(x, y, 8.0, 31)[0] + 0.35 * gf_noise2(x, y, 32.0, 32)[0];
    0.2 + 0.6 * smoothstep(0.2, 0.8, n)
}

/// The rule-table pick 0..1: patches at ~80, ~20 and ~5 m, spread to about uniform so each
/// row gets about its weight's share of the ground.
pub fn gf_pick(x: f32, y: f32) -> f32 {
    let n = 0.5 * gf_noise(x, y, 128.0, 41) + 0.33 * gf_noise2(x, y, 32.0, 32)[1] + 0.17 * gf_noise2(x, y, 8.0, 31)[1];
    smoothstep(0.24, 0.76, n)
}

/// Density patches 0.5..1 (~40 m, and the dither's ~7 m term: grass is thicker where its
/// patches start): thicker and thinner grass, darker and lighter.
pub fn gf_dens(x: f32, y: f32) -> f32 {
    let n = 0.6 * gf_noise(x, y, 64.0, 51) + 0.4 * gf_noise2(x, y, 8.0, 31)[0];
    0.5 + 0.5 * smoothstep(0.25, 0.7, n)
}

/// Where grass grows (0..1) from the rule table's grass weight `v` (share of the sample), the
/// canopy `cover` and the dither: the grass share of the treeless ground, feathered out under
/// the canopy edge (full to cover 0.3, none past 0.85), as patches.
pub fn gf_grass(v: f32, cover: f32, dither: f32) -> f32 {
    let treeless = (1.0 - cover).max(0.2);
    let t = smoothstep(0.08, 0.5, (v / treeless).clamp(0.0, 1.0)) * (1.0 - smoothstep(0.3, 0.85, cover));
    smoothstep(dither - 0.18, dither + 0.18, t)
}

/// Shares with the gaps of a sparse canopy as grassland: a 10 m cell classed as trees whose
/// canopy covers only part of it has grass between the crowns (wood pasture, larch meadows),
/// not bare forest floor. Where nothing grows trees (`cover` ~0) the shares stay: rock, sand
/// and built ground stay bare (terrain_common.slang `gf_understorey`). The grove's thin fringe
/// is shaped by [`gf_fringe_mask`], not here.
pub fn gf_understorey(mut s: [f32; 4], cover: f32) -> [f32; 4] {
    let gap = (1.0 - s.iter().sum::<f32>() - cover).max(0.0);
    s[0] += gap * smoothstep(0.002, 0.02, cover);
    s
}

/// The understorey fringe's patch threshold 0.15..0.85 (patches of ~1 and ~2 m,
/// terrain_common.slang `gf_fringe`).
pub fn gf_fringe(x: f32, y: f32) -> f32 {
    let n = 0.6 * gf_noise2(x, y, 4.0, 13)[0] + 0.4 * gf_noise(x, y, 2.0, 15);
    0.15 + 0.7 * smoothstep(0.25, 0.75, n)
}

/// How much of the grass a sparse canopy's gaps grow at its fringe (0..1): over the soft
/// cover's thin edge (~0.003..0.1, several metres) grass thins into the bare ground as a soft,
/// patchy blend (threshold `fringe`), not one line along a cover contour. Land cover that is
/// grassland itself (`shares` before [`gf_understorey`]) is never thinned.
pub fn gf_fringe_mask(shares: [f32; 4], cover: f32, fringe: f32) -> f32 {
    let u = smoothstep(0.003, 0.1, cover);
    let m = smoothstep(fringe - 0.2, fringe + 0.2, u);
    m.max(smoothstep(0.05, 0.3, shares.iter().sum::<f32>()))
}

/// Share of the ground that is not grass which is forest floor (litter), the rest bare soil.
pub fn gf_litter(cover: f32) -> f32 {
    smoothstep(0.05, 0.35, cover)
}

/// Face metres of `d` on the Earth, wrapped to the shader's period.
pub fn gf_xy(d: DVec3) -> (u8, f32, f32) {
    let (f, x, y) = face_metres(d, ed_geo::WGS84_A);
    (f, x.rem_euclid(FACE_PERIOD_M) as f32, y.rem_euclid(FACE_PERIOD_M) as f32)
}

/// The land-cover lookup point of `d`: `d` moved by [`gf_warp`] (the ground pages and the
/// canopy are read there; the terrain does the same in face metres).
pub fn gf_lookup(d: DVec3) -> DVec3 {
    let (f, x, y) = face_metres(d, ed_geo::WGS84_A);
    let w = gf_warp(x.rem_euclid(FACE_PERIOD_M) as f32, y.rem_euclid(FACE_PERIOD_M) as f32);
    face_dir(f, x + w[0] as f64, y + w[1] as f64, ed_geo::WGS84_A)
}

/// Offsets (face metres) of the canopy-cover taps around the [`gf_lookup`] point: the cover that
/// decides grass, litter and soil is the mean of the centre and these four (a ~9 m box), so a
/// grove's steep canopy fringe ramps over metres (terrain_common.slang `GF_SOFT`).
pub const GF_SOFT: [[f32; 2]; 4] = [[4.0, 2.0], [-2.0, 4.0], [-4.0, -2.0], [2.0, -4.0]];

/// The five cover lookup points of `d`: [`gf_lookup`] and its [`GF_SOFT`] taps.
pub fn gf_lookups(d: DVec3) -> [DVec3; 5] {
    let (f, x, y) = face_metres(d, ed_geo::WGS84_A);
    let w = gf_warp(x.rem_euclid(FACE_PERIOD_M) as f32, y.rem_euclid(FACE_PERIOD_M) as f32);
    let (cx, cy) = (x + w[0] as f64, y + w[1] as f64);
    let o = |k: usize| face_dir(f, cx + GF_SOFT[k][0] as f64, cy + GF_SOFT[k][1] as f64, ed_geo::WGS84_A);
    [face_dir(f, cx, cy, ed_geo::WGS84_A), o(0), o(1), o(2), o(3)]
}

/// Clumps of node `id` (level 14..=20) whose rank is under `keep_min`. `type_at(d)` is the grass
/// type and the ground's density (0..1) at body direction `d` (`None`: no grass). Clumps come in
/// lattice order.
pub fn scatter_node(id: NodeId, type_at: &dyn Fn(DVec3) -> Option<(u8, f32)>, keep_min: f32, seed: u64) -> Vec<Clump> {
    let f = Frame::new(id);
    let face = id.face();
    let (a0, a1, b0, b1, n) = cells(id, CLUMP_LEVEL, 0);
    // Under a small keep only the coarse lattice levels can pass: walk just those cells.
    let step = rank_step(keep_min);
    let up = |x: i64| (x + step - 1).div_euclid(step) * step;
    let mut out = Vec::new();
    let mut j = up(b0);
    while j < b1 {
        let mut i = up(a0);
        while i < a1 {
            let (ci, cj) = (i, j);
            i += step;
            let key = cell_key(face, CLUMP_LEVEL, ci as u64, cj as u64) ^ seed.wrapping_mul(0x9E37_79B9_7F4A_7C15);
            let lvl = cell_level(ci, cj);
            let rank = clump_rank(lvl, crate::hash01(key, 7));
            if rank >= keep_min {
                continue;
            }
            let (d, key) = candidate(face, CLUMP_LEVEL, ci, cj, n, seed);
            let Some((kind, dens)) = type_at(d) else { continue };
            let Some(t) = TYPES.get(kind as usize) else { continue };
            if crate::hash01(key, 3) >= t.density * dens.clamp(0.0, 1.0) {
                continue;
            }
            // Coarse-level clumps jitter over their whole block (2^l cells a side), so the even
            // far lattice does not show as a grid.
            let dir = if lvl > 0 {
                let b = (SUB << lvl) - 2;
                let ju = 1 + (crate::hash01(key, 10) * b as f32) as u64 % b;
                let jv = 1 + (crate::hash01(key, 11) * b as f32) as u64 % b;
                lattice_point(face, ci as u64 * SUB + ju, cj as u64 * SUB + jv, n as u64 * SUB)
            } else {
                d
            };
            // Facing: random, except row crops (`aniso < 1`), which share a field direction
            // that turns slowly over ~200 m (their narrow clumps read as a field's grain; no
            // lattice rows, which showed as straight lines on any land).
            let yaw = if t.aniso < 0.999 {
                let w = wrap_m(dir);
                let a = value3(w[0] / 200.0, w[1] / 200.0, w[2] / 200.0, seed ^ 0xF1E1D) * TAU * 1.5;
                (a + 0.25 * (crate::hash01(key, 6) - 0.5)) as f64
            } else {
                crate::hash01(key, 6) as f64 * TAU as f64
            };
            let e = f.e1 * yaw.cos() + f.e2 * yaw.sin();
            let facing = (e - dir * e.dot(dir)).normalize();
            let (hp, dry) = patch(dir, seed);
            let height = hp * (0.88 + 0.24 * crate::hash01(key, 5));
            out.push(Clump {
                dir,
                xy: f.xy(dir),
                facing,
                kind,
                rank,
                seed: (crate::hash01(key, 8) * 16_777_216.0) as u32 ^ ((crate::hash01(key, 9) * 256.0) as u32) << 24,
                height,
                dry,
            });
        }
        j += step;
    }
    out
}

/// Which land-cover share a [`Niche`] feeds on.
#[derive(Clone, Copy)]
enum Feed {
    Herb = 0,
    Crop = 1,
    Shrub = 2,
    Wet = 3,
}

/// Cold or warm ground: the mean annual temperature at the sample's altitude
/// (`Climate::temp_sl` less a 6.5 K/km lapse) under [`COLD_C`].
#[derive(Clone, Copy, PartialEq)]
enum Temp {
    Any,
    Cold,
    Warm,
}

/// Mean annual temperature (C) under which ground counts as cold (tundra).
const COLD_C: f32 = -3.0;

/// One rule of the real-ground grass mix: a share, its multiplier, the ranges it applies in
/// and the grass it grows. Rows are data; a sample sums the rows' weights (0 outside ranges).
struct Niche {
    feed: Feed,
    mul: f32,
    /// Aridity range (`Climate::aridity`, low = dry), `lo <= a < hi`.
    arid: (f32, f32),
    /// Altitude range (m), `lo <= alt < hi`.
    alt: (f32, f32),
    temp: Temp,
    kind: u8,
}

const ANY: (f32, f32) = (f32::NEG_INFINITY, f32::INFINITY);
/// Altitude of the alpine turf.
const LAWN_ALT: f32 = 2600.0;
/// Aridity under which grassland is long dry grass.
const DRY_ARID: f32 = 0.35;

/// Rows of one share and range split it by `mul` (the parts that are grass; summing under 1
/// leaves bare ground, e.g. sparse shrubland tufts on soil). Rows of one group are in pick
/// order (short to tall), so neighbouring patches step through the mix rather than jump.
const NICHES: [Niche; 14] = [
    Niche { feed: Feed::Crop, mul: 1.0, arid: ANY, alt: ANY, temp: Temp::Any, kind: WHEAT },
    Niche { feed: Feed::Wet, mul: 0.7, arid: ANY, alt: ANY, temp: Temp::Warm, kind: SEDGE },
    Niche { feed: Feed::Wet, mul: 0.3, arid: ANY, alt: ANY, temp: Temp::Warm, kind: LONG },
    Niche { feed: Feed::Wet, mul: 1.0, arid: ANY, alt: ANY, temp: Temp::Cold, kind: TUNDRA },
    Niche { feed: Feed::Shrub, mul: 0.35, arid: ANY, alt: ANY, temp: Temp::Any, kind: TUFT },
    Niche { feed: Feed::Herb, mul: 1.0, arid: ANY, alt: (f32::NEG_INFINITY, LAWN_ALT), temp: Temp::Cold, kind: TUNDRA },
    Niche { feed: Feed::Herb, mul: 0.15, arid: (f32::NEG_INFINITY, DRY_ARID), alt: (f32::NEG_INFINITY, LAWN_ALT), temp: Temp::Warm, kind: TUFT },
    Niche { feed: Feed::Herb, mul: 0.2, arid: (f32::NEG_INFINITY, DRY_ARID), alt: (f32::NEG_INFINITY, LAWN_ALT), temp: Temp::Warm, kind: MEADOW },
    Niche { feed: Feed::Herb, mul: 0.65, arid: (f32::NEG_INFINITY, DRY_ARID), alt: (f32::NEG_INFINITY, LAWN_ALT), temp: Temp::Warm, kind: LONG },
    Niche { feed: Feed::Herb, mul: 0.15, arid: (DRY_ARID, f32::INFINITY), alt: (f32::NEG_INFINITY, LAWN_ALT), temp: Temp::Warm, kind: LAWN },
    Niche { feed: Feed::Herb, mul: 0.55, arid: (DRY_ARID, f32::INFINITY), alt: (f32::NEG_INFINITY, LAWN_ALT), temp: Temp::Warm, kind: MEADOW },
    Niche { feed: Feed::Herb, mul: 0.3, arid: (DRY_ARID, f32::INFINITY), alt: (f32::NEG_INFINITY, LAWN_ALT), temp: Temp::Warm, kind: LONG },
    Niche { feed: Feed::Herb, mul: 0.6, arid: ANY, alt: (LAWN_ALT, f32::INFINITY), temp: Temp::Any, kind: LAWN },
    Niche { feed: Feed::Herb, mul: 0.4, arid: ANY, alt: (LAWN_ALT, f32::INFINITY), temp: Temp::Any, kind: MEADOW },
];

/// Floats per row of the terrain carpet table ([`carpet_rows`], grass G4).
pub const CARPET_ROW_FLOATS: usize = 16;
/// Bare ground (what trees and the four shares leave) on dry land: the terrain's soil layer
/// is gained toward this (linear albedo; tan desert floor and sand), toward its own calibrated
/// mean where it is humid. Below `BARE_ARID_RAMP.0` aridity fully arid, above `.1` humid.
pub const BARE_ARID: [f32; 3] = [0.36, 0.26, 0.15];
pub const BARE_ARID_RAMP: (f32, f32) = (0.08, 0.45);

/// The terrain carpet table (grass G4): the terrain shader mixes the carpet colour of every
/// [`NICHES`] row by the ground page's shares, the node's climate and the pixel's altitude, so
/// the ground under and past the blades is what the rule table grows, at every distance.
/// `grass_ref` and `soil_ref` are the terrain grass and soil layers' calibrated means (the
/// shader gains those layers by `carpet / grass_ref` and `bare / soil_ref`).
///
/// Row 0 (header): (grass ref rgb, rows), (soil ref rgb, 0), (bare arid rgb, ramp lo),
/// (0, 0, 0, ramp hi). Rows 1..: (feed, mul, tallness 0..1, 0), (aridity lo, hi, altitude lo,
/// hi), (temperature lo, hi C at altitude, 0, 0), (carpet rgb, 0). `mul` splits the share:
/// that part is grass, the rest bare (sparse shrubland tufts on soil).
pub fn carpet_rows(grass_ref: [f32; 3], soil_ref: [f32; 3]) -> Vec<[f32; CARPET_ROW_FLOATS]> {
    const BIG: f32 = 1e9;
    let r = |x: (f32, f32)| (x.0.max(-BIG), x.1.min(BIG));
    let mut out = vec![[
        grass_ref[0], grass_ref[1], grass_ref[2], NICHES.len() as f32,
        soil_ref[0], soil_ref[1], soil_ref[2], 0.0,
        BARE_ARID[0], BARE_ARID[1], BARE_ARID[2], BARE_ARID_RAMP.0,
        0.0, 0.0, 0.0, BARE_ARID_RAMP.1,
    ]];
    for n in &NICHES {
        let t = &TYPES[n.kind as usize];
        let (a, al) = (r(n.arid), r(n.alt));
        let tp = match n.temp {
            Temp::Any => (-BIG, BIG),
            Temp::Cold => (-BIG, COLD_C),
            Temp::Warm => (COLD_C, BIG),
        };
        let tall = smoothstep(0.12, 0.5, t.height.1);
        let c = t.carpet;
        out.push([
            n.feed as u8 as f32, n.mul, tall, 0.0,
            a.0, a.1, al.0, al.1,
            tp.0, tp.1, 0.0, 0.0,
            c[0], c[1], c[2], 0.0,
        ]);
    }
    out
}

/// Blend half-widths of the rule table's ranges (aridity, altitude m, temperature C):
/// resolve.slang `T_CARPET_SOFT`. The table's edges are soft transitions, not lines.
const SOFT: [f32; 3] = [0.05, 150.0, 1.5];

/// Smooth membership of `x` in `[lo, hi)` with half-width `s` (resolve.slang `t_band`).
fn band(x: f32, r: (f32, f32), s: f32) -> f32 {
    let a = if r.0 < -1e8 { 1.0 } else { smoothstep(0.0, 1.0, (x - r.0) / (2.0 * s) + 0.5) };
    let b = if r.1 > 1e8 { 1.0 } else { 1.0 - smoothstep(0.0, 1.0, (x - r.1) / (2.0 * s) + 0.5) };
    a * b
}

/// The rule table's row weights on one sample (the carpet's, resolve.slang `t_carpet`).
fn niche_weights(shares: [f32; 4], alt: f32, aridity: f32, temp_sl: f32) -> [f32; NICHES.len()] {
    let temp = temp_sl - 0.0065 * alt.max(0.0);
    let mut wts = [0.0f32; NICHES.len()];
    for (w, n) in wts.iter_mut().zip(&NICHES) {
        let tp = match n.temp {
            Temp::Any => ANY,
            Temp::Cold => (f32::NEG_INFINITY, COLD_C),
            Temp::Warm => (COLD_C, f32::INFINITY),
        };
        let ok = band(aridity, n.arid, SOFT[0]) * band(alt, n.alt, SOFT[1]) * band(temp, tp, SOFT[2]);
        *w = shares[n.feed as usize].clamp(0.0, 1.0) * n.mul * ok;
    }
    wts
}

/// The row picked at `pick` (0..1) from `wts` (its kind), `None` when no row has weight.
fn niche_pick(wts: &[f32; NICHES.len()], pick: f32) -> Option<u8> {
    let veg: f32 = wts.iter().sum();
    if veg <= 1e-6 {
        return None;
    }
    let mut at = pick.clamp(0.0, 0.9999) * veg;
    for (w, n) in wts.iter().zip(&NICHES) {
        if at < *w {
            return Some(n.kind);
        }
        at -= w;
    }
    // Rounding: the last row that has weight.
    wts.iter().zip(&NICHES).rev().find(|(w, _)| **w > 0.0).map(|(_, n)| n.kind)
}

/// Real ground: grass where the terrain carpet shows grass (one footprint: [`gf_grass`]). With
/// a ground page (`g.shares`, read at the [`gf_lookup`] point) the land cover decides the type
/// through [`NICHES`] (crops to wheat, wetland to sedge, shrubland to sparse tufts, grassland
/// by climate as a patchy mix; bare ground gets none) and the canopy feathers it out; without
/// one, climate and altitude alone: long dry grass where it is arid, short turf high up, a
/// meadow mix elsewhere. `d` is the body direction (the patch noise of the pick).
pub fn type_from_ground(g: &GroundSample, cl: &Climate, d: DVec3) -> Option<(u8, f32)> {
    let c = &g.canopy;
    let w = layer_weights(c.alt_m, g.slope_cos, c.cover);
    let Some(shares) = g.shares else {
        let grassy = (w[2] + w[5]) * (1.0 - smoothstep(0.1, 0.6, c.cover));
        if grassy < 0.15 {
            return None;
        }
        let kind = if cl.aridity < 0.35 {
            LONG
        } else if c.alt_m > 2600.0 {
            LAWN
        } else {
            MEADOW
        };
        return Some((kind, smoothstep(0.15, 0.6, grassy)));
    };
    // Cliffs, scree and snow stay bare; altitude alone never vetoes.
    let open = (1.0 - (w[0] + w[1] + w[4])).clamp(0.0, 1.0);
    if open < 0.15 {
        return None;
    }
    let (_, x, y) = gf_xy(d);
    let wts = niche_weights(gf_understorey(shares, c.cover), c.alt_m, cl.aridity, cl.temp_sl);
    let gp = gf_grass(wts.iter().sum(), c.cover, gf_dither(x, y)) * gf_fringe_mask(shares, c.cover, gf_fringe(x, y));
    if gp < 0.02 {
        return None;
    }
    let kind = niche_pick(&wts, gf_pick(x, y))?;
    Some((kind, gp * smoothstep(0.15, 0.6, open) * gf_dens(x, y)))
}

/// The grass lab's type map (place "grasslab"): `(east, north)` metres from the anchor. A lawn
/// disc around the anchor (r ~6 m), then 20-degree sectors cycling meadow, long grass, wheat
/// with noisy edges, so every camera sees several types side by side.
pub fn lab_type(east: f64, north: f64) -> Option<(u8, f32)> {
    let r = (east * east + north * north).sqrt() as f32;
    let wob = fbm3(east as f32 / 6.0, north as f32 / 6.0, 3.7, 0x1AB, 2) - 0.5;
    if r < 6.0 + 3.0 * wob {
        return Some((LAWN, 1.0));
    }
    let az = (east.atan2(north) as f32).to_degrees() + 10.0 + 14.0 * wob * (r / (r + 10.0));
    let sector = (az / 20.0).floor().rem_euclid(3.0) as u8;
    Some(([MEADOW, LONG, WHEAT][sector as usize], 1.0))
}

// ---------------------------------------------------------------------------------------------
// The shell map (styles C and D): what grows where around the camera, for the GPU.
// ---------------------------------------------------------------------------------------------

/// Cells per map side (two maps: fine for the shells, coarse for the terrain tint).
pub const MAP_N: usize = 256;
/// Fine map cell (m): +-64 m around its centre.
pub const MAP_FINE_M: f64 = 0.5;
/// Coarse map cell (m): +-2048 m (the tint carries the grass past the blades; 8 m / +-1024 m
/// showed as a colour cut-off from the air). 16 is the largest that divides `FACE_PERIOD_M` by `MAP_N`.
pub const MAP_COARSE_M: f64 = 16.0;
/// The map centre snaps to this lattice (m) of face metres: every cell size and strand cell
/// divides it, so cells and strands keep their place when the map moves.
pub const MAP_SNAP_M: f64 = 32.0;
/// The terrain's face-metre period (ed_render `TEX_PERIOD_M`): the shader sees face metres
/// modulo this.
pub const FACE_PERIOD_M: f64 = 4096.0;

/// Face axes (normal, u, v) of the cube mapping (`ed_geo::node`, ed_render `face_axes`).
fn face_axes(face: u8) -> [DVec3; 3] {
    let (x, y, z) = (DVec3::X, DVec3::Y, DVec3::Z);
    match face {
        0 => [x, y, z],
        1 => [-x, -y, z],
        2 => [y, -x, z],
        3 => [-y, x, z],
        4 => [z, y, -x],
        _ => [-z, y, x],
    }
}

/// Face and face metres (`atan(u) a`, `atan(v) a`; the terrain's `tex` coordinates, unwrapped)
/// of body direction `d` on a body of equatorial radius `a`.
pub fn face_metres(d: DVec3, a: f64) -> (u8, f64, f64) {
    let ab = d.abs();
    let axis = if ab.x >= ab.y && ab.x >= ab.z { 0 } else if ab.y >= ab.z { 1 } else { 2 };
    let m = [d.x, d.y, d.z][axis];
    let face = (axis * 2) as u8 + (m < 0.0) as u8;
    let [n, u, v] = face_axes(face);
    let k = d.dot(n);
    (face, (d.dot(u) / k).atan() * a, (d.dot(v) / k).atan() * a)
}

/// Inverse of [`face_metres`]: the unit direction of face metres `(x, y)` on `face`.
pub fn face_dir(face: u8, x: f64, y: f64, a: f64) -> DVec3 {
    let [n, u, v] = face_axes(face);
    (n + u * (x / a).tan() + v * (y / a).tan()).normalize()
}

/// One map cell: kind + 1 (0: no grass), density, height scale (/ 2), dry share; a byte each.
pub fn map_word(kind: Option<u8>, dens: f32, height: f32, dry: f32) -> u32 {
    let Some(k) = kind else { return 0 };
    let b = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u32;
    (k as u32 + 1).min(255) | b(dens) << 8 | b(height / 2.0) << 16 | b(dry) << 24
}

/// Decoded [`map_word`]: kind, density, height scale, dry share.
pub fn map_cell(w: u32) -> Option<(u8, f32, f32, f32)> {
    let k = w & 0xFF;
    if k == 0 {
        return None;
    }
    let f = |s: u32| ((w >> s) & 0xFF) as f32 / 255.0;
    Some((k as u8 - 1, f(8), f(16) * 2.0, f(24)))
}

/// The snapped map centre (unwrapped face metres) for a camera at face metres `(x, y)`.
pub fn map_centre(x: f64, y: f64) -> [f64; 2] {
    [(x / MAP_SNAP_M).round() * MAP_SNAP_M, (y / MAP_SNAP_M).round() * MAP_SNAP_M]
}

/// Toroidal slot offset of a map: cell `c` (relative to the centre, `-N/2..N/2`) is stored at
/// `(c + offset) mod N`, so a cell's slot depends on its world index only.
pub fn map_offset(centre: [f64; 2], cell_m: f64) -> [u32; 2] {
    let o = |x: f64| ((x / cell_m).round() as i64).rem_euclid(MAP_N as i64) as u32;
    [o(centre[0]), o(centre[1])]
}

/// One map (`MAP_N` x `MAP_N` words, row-major by slot) of cell size `cell_m` around `centre`
/// (snapped, unwrapped face metres on `face`). Cell centres are sampled with `type_at`; the
/// height and dry share are the clumps' patch fields. Pure: (face, centre, cell, a, type_at, seed).
pub fn build_map(face: u8, centre: [f64; 2], cell_m: f64, a: f64, type_at: &dyn Fn(DVec3) -> Option<(u8, f32)>, seed: u64) -> Vec<u32> {
    let n = MAP_N as i64;
    let off = map_offset(centre, cell_m);
    let mut out = vec![0u32; MAP_N * MAP_N];
    for j in -n / 2..n / 2 {
        for i in -n / 2..n / 2 {
            let x = centre[0] + (i as f64 + 0.5) * cell_m;
            let y = centre[1] + (j as f64 + 0.5) * cell_m;
            let d = face_dir(face, x, y, a);
            let w = match type_at(d) {
                Some((k, dens)) if (k as usize) < TYPES.len() && dens > 0.0 => {
                    let (h, dry) = patch(d, seed);
                    map_word(Some(k), dens, h, dry)
                }
                _ => 0,
            };
            let si = (i + off[0] as i64).rem_euclid(n) as usize;
            let sj = (j + off[1] as i64).rem_euclid(n) as usize;
            out[sj * MAP_N + si] = w;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use crate::stand::CanopySample;

    fn ground(shares: Option<[f32; 4]>, alt: f32) -> GroundSample {
        GroundSample {
            canopy: CanopySample { cover: 0.0, height_m: 0.0, mix: 0.0, under: 0.0, alt_m: alt, wet: 0.3 },
            slope_cos: 0.99,
            shares,
        }
    }
    const TEMPERATE: Climate = Climate { aridity: 0.8, temp_sl: 12.0 };
    const ARID: Climate = Climate { aridity: 0.1, temp_sl: 22.0 };
    const POLAR: Climate = Climate { aridity: 0.8, temp_sl: -10.0 };

    fn dir(k: f64) -> DVec3 {
        DVec3::new(0.3 + k * 1e-6, 0.5, 0.8).normalize()
    }

    #[test]
    fn rule_table_picks_by_land_cover() {
        let at = |sh, cl: &Climate| type_from_ground(&ground(Some(sh), 600.0), cl, dir(0.0));
        assert_eq!(at([0.0; 4], &TEMPERATE), None);
        assert_eq!(at([0.03, 0.02, 0.0, 0.0], &TEMPERATE), None);
        assert_eq!(at([0.0, 0.9, 0.0, 0.0], &TEMPERATE).unwrap().0, WHEAT);
        let (k, dt) = at([0.0, 0.0, 0.99, 0.0], &ARID).unwrap();
        assert_eq!(k, TUFT);
        let (km, dm) = at([0.8, 0.0, 0.0, 0.0], &TEMPERATE).unwrap();
        assert!([LAWN, MEADOW, LONG].contains(&km), "{km}");
        assert!(dt < dm - 0.1, "tuft {dt} meadow {dm}");
        assert!([SEDGE, LONG].contains(&at([0.0, 0.0, 0.0, 0.9], &TEMPERATE).unwrap().0));
        assert_eq!(at([0.8, 0.0, 0.0, 0.0], &POLAR).unwrap().0, TUNDRA);
        assert_eq!(at([0.0, 0.0, 0.0, 0.9], &POLAR).unwrap().0, TUNDRA);
        assert!([TUFT, MEADOW, LONG].contains(&at([0.8, 0.0, 0.0, 0.0], &ARID).unwrap().0));
        let hi = type_from_ground(&ground(Some([0.8, 0.0, 0.0, 0.0]), 2700.0), &Climate { aridity: 0.8, temp_sl: 25.0 }, dir(0.0));
        assert!([LAWN, MEADOW].contains(&hi.unwrap().0));
    }

    #[test]
    fn grassland_is_a_mix_of_types() {
        // A temperate meadow over ~1 km: every humid herb row shows, about by its weight.
        let g = ground(Some([0.9, 0.0, 0.0, 0.0]), 600.0);
        let mut n = [0u32; 8];
        for k in 0..2000 {
            let d = DVec3::new(0.3 + k as f64 * 7e-7, 0.5 + (k % 37) as f64 * 3e-6, 0.8).normalize();
            n[type_from_ground(&g, &TEMPERATE, d).unwrap().0 as usize] += 1;
        }
        let f = |k: u8| n[k as usize] as f32 / 2000.0;
        assert!(f(MEADOW) > 0.3 && f(LONG) > 0.12 && f(LAWN) > 0.04, "{n:?}");
    }

    #[test]
    fn grass_follows_the_canopy_edge() {
        // Open herb: grass; full canopy: none; between: patches. A sparse canopy's gaps are grass.
        let at = |sh: [f32; 4], cover: f32, k: f64| {
            let mut g = ground(Some(sh), 600.0);
            g.canopy.cover = cover;
            type_from_ground(&g, &TEMPERATE, dir(k * 40.0)).is_some()
        };
        let frac = |sh: [f32; 4], cover: f32| (0..400).filter(|k| at(sh, cover, *k as f64)).count() as f32 / 400.0;
        assert_eq!(frac([0.9, 0.0, 0.0, 0.0], 0.0), 1.0);
        assert_eq!(frac([0.0; 4], 0.95), 0.0);
        let edge = frac([0.3, 0.0, 0.0, 0.0], 0.6);
        assert!(edge > 0.05 && edge < 0.95, "{edge}");
        assert!(frac([0.0; 4], 0.3) > 0.5, "wood pasture gaps");
        // Bare ground with no trees stays bare.
        assert_eq!(frac([0.0; 4], 0.0), 0.0);
    }

    #[test]
    fn carpet_rows_partition_each_share() {
        let rows = carpet_rows([0.11, 0.155, 0.05], [0.12, 0.10, 0.075]);
        assert_eq!(rows.len(), NICHES.len() + 1);
        assert_eq!(rows[0][3] as usize, NICHES.len());
        // The shader mixes every row of a share by its (softened) ranges: for any climate the
        // rows of one share must split it once (their weights sum to the share's grass part:
        // all of it, shrubland a third).
        let inr = |x: f32, lo: f32, hi: f32| x >= lo && x < hi;
        for feed in 0..4 {
            for &arid in &[0.05f32, 0.3, 0.36, 0.9, 2.0] {
                for &alt in &[-10.0f32, 500.0, 2599.0, 2601.0, 4000.0] {
                    for &temp in &[-20.0f32, -3.5, -2.5, 15.0] {
                        let sum: f32 = rows[1..]
                            .iter()
                            .filter(|r| r[0] as usize == feed && inr(arid, r[4], r[5]) && inr(alt, r[6], r[7]) && inr(temp, r[8], r[9]))
                            .map(|r| r[1])
                            .sum();
                        let want = if feed == 2 { 0.35 } else { 1.0 };
                        assert!((sum - want).abs() < 1e-5, "feed {feed} arid {arid} alt {alt} temp {temp}: {sum}");
                    }
                }
            }
        }
        let crop = rows[1..].iter().find(|r| r[0] == 1.0).unwrap();
        assert_eq!(&crop[12..15], &TYPES[WHEAT as usize].carpet);
    }

    #[test]
    fn no_ground_page_keeps_the_old_rule() {
        let g = ground(None, 600.0);
        assert_eq!(type_from_ground(&g, &TEMPERATE, dir(0.0)).map(|t| t.0), Some(MEADOW));
        assert_eq!(type_from_ground(&g, &ARID, dir(0.0)).map(|t| t.0), Some(LONG));
    }

    #[test]
    fn rule_table_is_deterministic_and_patchy() {
        let g = ground(Some([0.4, 0.0, 0.5, 0.0]), 600.0);
        assert_eq!(type_from_ground(&g, &ARID, dir(1.0)), type_from_ground(&g, &ARID, dir(1.0)));
        // Mixed shares: both kinds show, in patches (neighbours 1 m apart mostly agree; the
        // smallest patches are ~5 m).
        let (mut seen, mut same) = ([false; 8], 0);
        for k in 0..400 {
            let a = type_from_ground(&g, &ARID, dir(k as f64 * 3.0)).unwrap().0;
            seen[a as usize] = true;
            let b = type_from_ground(&g, &ARID, dir(k as f64 * 3.0 + 0.15)).unwrap().0;
            same += (a == b) as u32;
        }
        assert!(seen[TUFT as usize] && seen[LONG as usize], "{seen:?}");
        assert!(same > 350, "{same}");
    }

    use super::*;
    use ed_geo::BodyId;

    fn node() -> NodeId {
        let d = DVec3::new(47.05f64.to_radians().cos() * 7.6f64.to_radians().cos(), 47.05f64.to_radians().cos() * 7.6f64.to_radians().sin(), 47.05f64.to_radians().sin());
        NodeId::from_unit(BodyId(0), d, NODE_LEVEL)
    }

    fn area_m2(id: NodeId) -> f64 {
        let a = (id.corner(1) - id.corner(0)).length() * EARTH_R;
        let b = (id.corner(2) - id.corner(0)).length() * EARTH_R;
        a * b
    }

    #[test]
    fn rows_match_the_shader_layout() {
        for t in &TYPES {
            let r = gpu_row(t);
            assert_eq!(r[3] as u32, t.blades);
            assert!(t.blades <= MAX_BLADES && t.height.0 < t.height.1 && t.width > 0.0);
            assert_eq!(r[23], t.density);
        }
        assert_eq!(gpu_rows().len(), TYPES.len());
    }

    #[test]
    fn deterministic_and_inside_the_node() {
        let id = node();
        let ty = |_: DVec3| Some((MEADOW, 1.0));
        let a = scatter_node(id, &ty, 1.0, 7);
        let b = scatter_node(id, &ty, 1.0, 7);
        assert_eq!(a, b);
        assert!(!a.is_empty());
        let c = scatter_node(id, &ty, 1.0, 8);
        assert_ne!(a, c);
        for k in a.iter().step_by(97) {
            assert_eq!(NodeId::from_unit(id.body(), k.dir, id.level()), id);
            assert!((k.facing.dot(k.dir)).abs() < 1e-6 && (k.facing.length() - 1.0).abs() < 1e-9);
            assert!(k.height > 0.4 && k.height < 1.6 && (0.0..=1.0).contains(&k.dry));
        }
    }

    #[test]
    fn density_per_square_metre() {
        let id = node();
        let area = area_m2(id);
        let full = scatter_node(id, &|_| Some((LAWN, 1.0)), 1.0, 1).len() as f64 / area;
        // One clump per 0.3-0.6 m² at full density.
        assert!((1.6..=3.4).contains(&full), "{full} clumps/m2");
        let half = scatter_node(id, &|_| Some((LAWN, 0.5)), 1.0, 1).len() as f64 / area;
        assert!((half / full - 0.5).abs() < 0.05, "{half} vs {full}");
        let kept = scatter_node(id, &|_| Some((LAWN, 1.0)), 0.25, 1).len() as f64 / area;
        assert!((kept / full - 0.25).abs() < 0.03);
        assert!(scatter_node(id, &|_| None, 1.0, 1).is_empty());
    }

    #[test]
    fn crop_faces_its_field_without_a_lattice() {
        let id = node();
        let w = scatter_node(id, &|_| Some((WHEAT, 1.0)), 1.0, 3);
        assert!(w.len() > 100);
        // Neighbouring crop clumps face about the same way (the field direction); the facing
        // turns slowly across the land, and clumps are jittered, not on cell centres.
        let a = w[w.len() / 2];
        let near: Vec<&Clump> = w.iter().filter(|c| c.dir != a.dir && c.dir.distance(a.dir) * EARTH_R < 1.5).collect();
        assert!(near.len() > 3);
        let agree = near.iter().filter(|c| c.facing.dot(a.facing).abs() > 0.9).count();
        assert!(agree * 10 >= near.len() * 8, "{agree}/{}", near.len());
        let steps: Vec<f64> = near.iter().map(|c| c.dir.distance(a.dir) * EARTH_R).collect();
        let spread = steps.iter().cloned().fold(0.0f64, f64::max) - steps.iter().cloned().fold(9.0f64, f64::min);
        assert!(spread > 0.3, "{steps:?}");
    }

    #[test]
    fn lod_blades_then_clumps() {
        let (c, k) = lod(5.0, 64.0, 14.0, BMIN);
        assert_eq!((c, k), (64.0, 1.0));
        let (c, k) = lod(28.0, 64.0, 14.0, BMIN);
        assert!((c - 16.0).abs() < 1e-3 && k == 1.0);
        let (c, k) = lod(100.0, 64.0, 14.0, BMIN);
        assert_eq!(c, BMIN);
        assert!(k < 1.0 && k > 0.2);
        assert!(clump_keep(10.0, R0_M) == 1.0 && clump_keep(200.0, R0_M) < 0.2);
        // Style A's rule is the far rule with the floor held.
        let a = LodParams::near(14.0, BMIN);
        assert_eq!(lod_p(100.0, 64.0, &a), lod(100.0, 64.0, 14.0, BMIN));
        // Far rule: the floor falls to bfar, the on-screen blade density (count x keep) holds.
        let e = LodParams { r0: 14.0, bmin: 5.0, bfar: 2.0, dfar: 50.0 };
        let (c, k) = lod_p(400.0, 64.0, &e);
        assert_eq!(c, 2.0);
        assert!((c * k - 64.0 * (14.0f32 / 400.0).powi(2)).abs() < 1e-4);
        assert!(clump_keep_p(400.0, &e) > clump_keep_p(400.0, &a));
        let (c, _) = lod_p(60.0, 64.0, &e);
        assert!(c > 4.0 && c < 5.0, "{c}");
    }

    #[test]
    fn stratified_ranks_thin_to_an_even_lattice() {
        let id = node();
        let ty = |_: DVec3| Some((MEADOW, 1.0));
        let full = scatter_node(id, &ty, 1.0, 4);
        // A small keep walks only the coarse cells but keeps exactly the same clumps.
        for keep in [0.3f32, 0.2, 0.0625, 0.03, 0.01, 0.002] {
            let thin = scatter_node(id, &ty, keep, 4);
            let want: Vec<Clump> = full.iter().filter(|c| c.rank < keep).copied().collect();
            assert_eq!(thin.len(), want.len(), "{keep}");
            let mut a: Vec<u32> = thin.iter().map(|c| c.seed).collect();
            let mut b: Vec<u32> = want.iter().map(|c| c.seed).collect();
            a.sort();
            b.sort();
            assert_eq!(a, b, "{keep}");
            let r = thin.len() as f32 / full.len() as f32;
            assert!((r / keep - 1.0).abs() < 0.25, "{keep}: {r}");
        }
        // Even: at keep 1/16 every 4x4-cell block (2.4 m) of the node holds about one clump.
        let thin = scatter_node(id, &ty, 0.0625, 4);
        let mut occ = std::collections::BTreeMap::new();
        for c in &thin {
            let k = ((c.xy[0] / 2.4).floor() as i32, (c.xy[1] / 2.4).floor() as i32);
            *occ.entry(k).or_insert(0) += 1;
        }
        let most = occ.values().copied().max().unwrap();
        assert!(most <= 4, "{most}");
        assert_eq!(cell_level(0, 8), 3);
        assert_eq!(cell_level(4, 8), 2);
        assert_eq!(cell_level(3, 0), 0);
        assert_eq!(clump_rank(RANK_LEVELS - 1, 0.0), 0.0);
        assert!(clump_rank(0, 0.0) == 0.25 && clump_rank(1, 0.999) < 0.25);
    }

    #[test]
    fn lab_map_shows_several_types() {
        assert_eq!(lab_type(0.0, 1.0).map(|t| t.0), Some(LAWN));
        // The eye camera looks north: 30 m out, -30..30 degrees, all three field types.
        let mut seen = [false; 4];
        for k in -30..=30 {
            let a = (k as f64).to_radians();
            let (kind, _) = lab_type(30.0 * a.sin(), 30.0 * a.cos()).unwrap();
            seen[kind as usize] = true;
        }
        assert!(seen[1] && seen[2] && seen[3], "{seen:?}");
    }

    #[test]
    fn wheat_rows_are_finite() {
        // A node north-west of the lab anchor, all wheat: every clump is finite and faces along the ground.
        let id = node();
        let all_wheat = |_: DVec3| Some((WHEAT, 1.0));
        let c = scatter_node(id, &all_wheat, 1.0, 7);
        assert!(c.len() > 1000, "{}", c.len());
        for k in &c {
            assert!(k.dir.is_finite() && k.facing.is_finite(), "{k:?}");
            assert!(k.facing.dot(k.dir).abs() < 1e-6);
        }
    }

    #[test]
    fn face_metres_match_the_node_lattice() {
        let a = 6_378_137.0;
        for face in 0..6u8 {
            for (i, j) in [(3u64, 5u64), (512, 100), (1000, 1023)] {
                let n = 1024;
                let d = ed_geo::node::lattice_point(face, i, j, n);
                let (f, x, y) = face_metres(d, a);
                assert_eq!(f, face);
                let wu = ed_geo::node::warp(i, n);
                let wv = ed_geo::node::warp(j, n);
                assert!((x - wu.atan() * a).abs() < 1e-6 && (y - wv.atan() * a).abs() < 1e-6);
                assert!(face_dir(face, x, y, a).distance(d) < 1e-12);
            }
        }
    }

    #[test]
    fn map_words_round_trip_and_maps_are_stable() {
        assert_eq!(map_word(None, 1.0, 1.0, 0.0), 0);
        let (k, d, h, y) = map_cell(map_word(Some(WHEAT), 0.5, 1.2, 0.25)).unwrap();
        assert_eq!(k, WHEAT);
        assert!((d - 0.5).abs() < 0.01 && (h - 1.2).abs() < 0.01 && (y - 0.25).abs() < 0.01);
        let a = 6_378_137.0;
        let (face, x, y) = face_metres(node().centre().normalize(), a);
        let c0 = map_centre(x, y);
        assert_eq!(c0[0] % MAP_SNAP_M, 0.0);
        let ty = |d: DVec3| {
            let (_, x, y) = face_metres(d, a);
            Some((((x / 7.0).floor() as i64).rem_euclid(4) as u8, 1.0))
        };
        let m0 = build_map(face, c0, MAP_FINE_M, a, &ty, 3);
        assert_eq!(m0, build_map(face, c0, MAP_FINE_M, a, &ty, 3));
        // Moved by one snap: the overlapping cells keep their slots and words.
        let c1 = [c0[0] + MAP_SNAP_M, c0[1]];
        let m1 = build_map(face, c1, MAP_FINE_M, a, &ty, 3);
        let mut same = 0;
        for s in 0..MAP_N * MAP_N {
            if m0[s] == m1[s] {
                same += 1;
            }
        }
        assert!(same as f64 > 0.74 * (MAP_N * MAP_N) as f64, "{same}");
        // Every strand cell divides the snap (world-stable strands).
        for t in &TYPES {
            let k = (MAP_SNAP_M / t.strand_m as f64).round();
            assert!((k * t.strand_m as f64 - MAP_SNAP_M).abs() < 1e-9 && (k as u64).is_power_of_two(), "{}", t.name);
            assert!(t.shell_h >= 0.0 && t.shell_h <= 0.1 && t.sheen > 0.0 && t.sheen <= 1.0);
        }
        assert!((FACE_PERIOD_M / MAP_FINE_M) as usize % MAP_N == 0 && (FACE_PERIOD_M / MAP_COARSE_M) as usize % MAP_N == 0);
    }

    #[test]
    fn shell_rows() {
        let r = gpu_row(&TYPES[LAWN as usize]);
        assert_eq!(r[24], TYPES[0].shell_h);
        assert_eq!(r[31], 1.0);
        assert!(shell_h_max() >= TYPES[0].shell_h);
    }
}
