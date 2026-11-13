//! The simulated stand (trees design G2, option C): a seeded growth-and-competition pass that
//! turns a node's canopy page (cover, height, mix, understory) into individual trees.
//!
//! **Seamless and order-independent by construction.**
//!
//! 1. *Global candidate lattice.* Each cube face carries an integer grid of `2^22` cells per
//!    side (`~2.4 m`, the node lattice `lattice_point(face, i, j, 2^22)`; a level-`L` node owns
//!    the `2^(22-L)` cell block at `(x, y)`). Every cell holds 1 or 2 candidates whose jitter,
//!    priority, class roll and acceptance roll are `hash(seed, face, ci, cj, k)`. Nothing is
//!    keyed by node, so every node that sees a cell sees the same candidates.
//! 2. *Local field.* The canopy field (cover, height, mix, understory, altitude, wetness) is
//!    sampled through the caller's closure at a **global coarse lattice** (every 4th cell,
//!    `~9.6 m`, which is the level-12 page spacing) and interpolated in integer cell space, so a
//!    candidate's field value does not depend on which node asked. The same lattice carries the
//!    metric (metres per cell, analytic for the equiangular cube map), the gap noise and the
//!    edge warp.
//! 3. *Competition.* A candidate is *alive* when its roll beats a cover-derived probability.
//!    Alive candidates compete by rank (class first: canopy, sub-canopy, sapling; then the
//!    priority hash). `PASSES` rounds of "accept local maxima, eliminate their neighbours" give
//!    a result that depends only on candidates within `(2 * PASSES - 1) * R_MAX_M` metres, so
//!    evaluating a window with that halo gives, for every cell inside the window proper, the
//!    answer any other window would give. Trees belong to the node holding their cell (half-open
//!    integer ranges), so every tree is in exactly one node.
//! 4. *Face edges.* Candidates are per face and do not compete across a face edge (a tree and its
//!    neighbour on the other face can overlap by up to a crown); the node-seam guarantee is
//!    exact everywhere else.
//!
//! The pass writes its measured crown cover, canopy height and conifer mix back to the canopy
//! page (`measure`, `write_back`), which is what the far forest marches (trees plan 10.2.1).

use crate::foliage::TreeKind;
use ed_geo::node::{lattice_point, warp, NodeId};
use ed_geo::{encode_height, CanopyPage, ObjectId, PAGE_CELLS, PAGE_N};
use glam::DVec3;
use std::f64::consts::{FRAC_PI_4, PI};

/// Cells per face side: `2^FACE_LEVEL`.
pub const FACE_LEVEL: u32 = 22;
const N: i64 = 1 << FACE_LEVEL;
/// Sub-cell position steps (jitter is quantised to `1/SUB` cell).
const SUB: i64 = 256;
/// Spacing (cells) of the global coarse lattice the field is sampled on.
const COARSE: i64 = 4;
/// Elimination rounds. The dependency radius of the result is `(2 * PASSES - 1) * R_MAX_M`.
pub const PASSES: usize = 3;
/// Largest competition distance between two crowns (m): `max(COMPETE) * 2 * CROWN_MAX_M` (8.8) rounded up.
pub const R_MAX_M: f32 = 9.0;
/// Largest crown radius (m).
pub const CROWN_MAX_M: f32 = 8.0;
/// Halo (m) around a window that the dominance passes need.
pub const HALO_M: f32 = (2 * PASSES - 1) as f32 * R_MAX_M;
/// Smallest metres per cell on a face (face-edge midpoint), for converting metres to cells.
const MIN_CELL_M: f32 = 1.6;

const EARTH_R: f64 = 6_371_008.8;
/// Metres per cell at a face centre.
const K_M: f64 = EARTH_R * FRAC_PI_4 * 2.0 / N as f64;

/// Candidates per cell: 1 or 2 (mean 1.5).
const K_AVG: f32 = 1.5;

/// Age classes.
pub const SAPLING: u8 = 0;
pub const SUB_CANOPY: u8 = 1;
pub const CANOPY: u8 = 2;

/// One class's tuning (data, calibrated by `calibrate_cover` in the tests).
#[derive(Clone, Copy, Debug)]
pub struct ClassParams {
    /// Share of the candidate pool.
    pub frac: f32,
    /// Share of the page cover this class is asked to supply on its own.
    pub share: f32,
    /// Acceptance multiplier (absorbs competition losses).
    pub kappa: f32,
    /// Height of the class as a fraction of the canopy height `(lo, span)`.
    pub height: (f32, f32),
    /// Crown radius factor relative to the canopy-height crown (for the density target only).
    pub r_factor: f32,
}

/// Indexed by age class.
pub const CLASSES: [ClassParams; 3] = [
    ClassParams { frac: 0.25, share: 0.30, kappa: 1.6, height: (0.08, 0.20), r_factor: 0.40 },
    ClassParams { frac: 0.30, share: 0.40, kappa: 1.6, height: (0.40, 0.30), r_factor: 0.72 },
    ClassParams { frac: 0.45, share: 0.80, kappa: 1.5, height: (0.82, 0.36), r_factor: 1.00 },
];

/// Crown overlap rule: two crowns of classes `a`, `b` may not be closer than
/// `COMPETE[a][b] * (r_a + r_b)` metres between centres. Smaller trees tuck under bigger ones.
pub const COMPETE: [[f32; 3]; 3] = [[0.55, 0.38, 0.32], [0.38, 0.55, 0.40], [0.32, 0.40, 0.46]];

/// Gap-noise threshold at full cover (fraction of the noise range that becomes clearing).
const GAP_STRENGTH: f32 = 0.30;

/// Calibration: page cover to the density-target cover, the inverse of the competition's
/// saturation curve measured by `calibrate_cover` (flat field, canopy 25-35 m). Without it a
/// target of 0.5 yields 0.7 and a target of 1.0 yields 0.9.
pub const COVER_PREWARP: [(f32, f32); 13] = [(0.0, 0.0), (0.05, 0.0227), (0.1, 0.045), (0.2, 0.0947), (0.3, 0.145), (0.4, 0.209), (0.5, 0.2756), (0.6, 0.363), (0.7, 0.463), (0.8, 0.605), (0.9, 0.82), (0.95, 1.0), (1.0, 1.3)];

fn prewarp(c: f32) -> f32 {
    let t = &COVER_PREWARP;
    for w in t.windows(2) {
        if c <= w[1].0 {
            return w[0].1 + (w[1].1 - w[0].1) * (c - w[0].0) / (w[1].0 - w[0].0);
        }
    }
    t[t.len() - 1].1
}

/// Crown radius (m) from height (m): `0.72 h^0.42`, stretched toward closed canopy, species-scaled.
fn crown_radius(h: f32, cover: f32, crown_k: f32) -> f32 {
    (0.72 * libm::powf(h, 0.42) * (1.0 + 0.25 * cover) * crown_k).clamp(0.35, CROWN_MAX_M)
}

// ---------------------------------------------------------------------------------------------
// Species table (data rows)
// ---------------------------------------------------------------------------------------------

const M_SAP: u8 = 1;
const M_SUB: u8 = 2;
const M_CAN: u8 = 4;
const M_ALL: u8 = 7;

/// A stand species niche: where and as what it grows. Rows, not branches.
#[derive(Clone, Copy, Debug)]
pub struct StandSpecies {
    pub name: &'static str,
    /// The renderer's kind this maps to (distinct species can share one until assets exist).
    pub kind: TreeKind,
    pub conifer: bool,
    pub shrub: bool,
    /// Debug/board colour (sRGB hex); the renderer's palette comes from `trees::species`.
    pub leaf: u32,
    /// `(lat_lo, lat_hi, lon_lo, lon_hi)` degrees.
    pub geo: [f32; 4],
    /// Altitude window (m); soft 250 m shoulders.
    pub alt: (f32, f32),
    /// Canopy-height window (m) it grows in; soft 30 % shoulders.
    pub canopy_h: (f32, f32),
    /// Preference for wet ground, `-1..1`.
    pub wet: f32,
    pub weight: f32,
    /// Allowed age classes (`M_SAP | M_SUB | M_CAN`).
    pub classes: u8,
    /// Height cap (m): shrubs stay low whatever the canopy.
    pub max_h: f32,
    /// Crown radius multiplier (conifers narrow, oaks wide).
    pub crown_k: f32,
    /// Crown bias (O3-8): how much the crown reads in the canopy page relative to the true
    /// crown (1 = true scale). Applied when the stand writes back into the page ([`measure`]);
    /// never in a shader.
    pub crown_bias: f32,
    /// Leaf phenology ([`crate::season`]).
    pub pheno: Pheno,
}

/// Leaf phenology of a species at the reference place (lat 46, altitude 500 m): day of year
/// of leaf-out, of the autumn colour peak and of leaf drop; `crate::season` shifts them by
/// latitude and altitude (Hopkins' bioclimatic law).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pheno {
    pub leaf_out: f32,
    pub colour: f32,
    pub drop: f32,
    /// Deciduous: the autumn leaf albedo (sRGB hex, the colour of the leaf itself, not a tint).
    /// Evergreen: the winter leaf albedo the crown moves toward by `winter`.
    pub colour_hex: u32,
    pub evergreen: bool,
    /// Evergreen only: how far toward `colour_hex` the crown goes in the cold season (0..1).
    pub winter: f32,
}

const GLOBAL: [f32; 4] = [-90.0, 90.0, -180.0, 180.0];
const EUROPE: [f32; 4] = [36.0, 70.0, -12.0, 45.0];
const E_NAMERICA: [f32; 4] = [25.0, 50.0, -100.0, -60.0];
const TROPICS: [f32; 4] = [-25.0, 25.0, -180.0, 180.0];

#[allow(clippy::too_many_arguments)]
const fn sp(name: &'static str, kind: TreeKind, conifer: bool, leaf: u32, geo: [f32; 4], alt: (f32, f32), canopy_h: (f32, f32), wet: f32, weight: f32, classes: u8, max_h: f32, crown_k: f32) -> StandSpecies {
    StandSpecies { name, kind, conifer, shrub: false, leaf, geo, alt, canopy_h, wet, weight, classes, max_h, crown_k, crown_bias: 1.0, pheno: EVERGREEN }
}
/// Temperate evergreen: a little duller and browner from November to March.
const EVERGREEN: Pheno = Pheno { leaf_out: 80.0, colour: 300.0, drop: 0.0, colour_hex: 0x38402a, evergreen: true, winter: 0.6 };
/// Deciduous row: leaf-out, autumn colour peak and drop (day of year at the reference), colour.
const fn dec(mut s: StandSpecies, leaf_out: f32, colour: f32, drop: f32, hex: u32) -> StandSpecies {
    s.pheno = Pheno { leaf_out, colour, drop, colour_hex: hex, evergreen: false, winter: 0.0 };
    s
}
/// Tropical evergreen: no seasonal colour.
const fn trop(mut s: StandSpecies) -> StandSpecies {
    s.pheno.winter = 0.0;
    s
}
const fn bias(mut s: StandSpecies, b: f32) -> StandSpecies {
    s.crown_bias = b;
    s
}
const fn shrub(mut s: StandSpecies) -> StandSpecies {
    s.shrub = true;
    s
}

use TreeKind as K;

/// Zermatt/Alps, Blue Ridge, generic temperate/boreal and tropical rows. Order is the pick order.
pub static STAND_SPECIES: [StandSpecies; 30] = [
    // --- Europe / Alps ---
    bias(sp("Norway spruce", K::Conifer, true, 0x3b5d3b, EUROPE, (500.0, 2100.0), (12.0, 50.0), 0.0, 3.0, M_ALL, 55.0, 0.80), 1.05),
    bias(sp("Silver fir", K::Fir, true, 0x3a5b3d, EUROPE, (400.0, 1600.0), (20.0, 50.0), 0.1, 1.2, M_ALL, 55.0, 0.90), 1.05),
    bias(dec(sp("European larch", K::Conifer, true, 0x547e4b, EUROPE, (1300.0, 2400.0), (8.0, 40.0), 0.0, 2.5, M_ALL, 45.0, 0.75), 110.0, 295.0, 335.0, 0xb88428), 1.05),
    sp("Swiss stone pine", K::Pine, true, 0x3e6241, EUROPE, (1700.0, 2500.0), (5.0, 25.0), 0.0, 1.5, M_ALL, 28.0, 0.70),
    sp("Scots pine", K::Pine, true, 0x496a3e, EUROPE, (0.0, 1700.0), (10.0, 35.0), -0.3, 1.2, M_ALL, 38.0, 0.85),
    sp("Mountain pine", K::Pine, true, 0x3e5f3b, EUROPE, (1800.0, 2400.0), (1.0, 8.0), 0.0, 0.8, M_ALL, 6.0, 1.00),
    dec(sp("Silver birch", K::Birch, false, 0x86b04a, EUROPE, (0.0, 2000.0), (4.0, 30.0), 0.4, 1.0, M_ALL, 32.0, 0.70), 112.0, 285.0, 310.0, 0xc29a28),
    dec(sp("Grey alder", K::Broadleaf, false, 0x6f8f55, EUROPE, (200.0, 1800.0), (4.0, 25.0), 0.8, 0.8, M_SAP | M_SUB, 20.0, 0.80), 105.0, 300.0, 325.0, 0x7f7a32),
    dec(shrub(sp("Green alder", K::Shrub, false, 0x5a7d3c, EUROPE, (1400.0, 2400.0), (0.0, 90.0), 0.3, 1.2, M_SAP, 3.5, 1.00)), 118.0, 290.0, 315.0, 0x8a7a30),
    dec(sp("European beech", K::Broadleaf, false, 0x5f8f3a, EUROPE, (0.0, 1400.0), (18.0, 42.0), 0.0, 2.0, M_ALL, 45.0, 1.10), 113.0, 292.0, 318.0, 0x9a5a22),
    dec(sp("Sycamore maple", K::Maple, false, 0x6c9a3e, EUROPE, (300.0, 1500.0), (15.0, 38.0), 0.2, 0.8, M_ALL, 40.0, 1.00), 108.0, 288.0, 312.0, 0xb8782a),
    dec(sp("Pedunculate oak", K::Broadleaf, false, 0x5d8a3a, EUROPE, (0.0, 900.0), (15.0, 40.0), 0.0, 1.2, M_ALL, 42.0, 1.20), 121.0, 298.0, 328.0, 0x8a5e2a),
    shrub(sp("Alpine juniper", K::Juniper, true, 0x4d6e4a, EUROPE, (1500.0, 2700.0), (0.0, 90.0), -0.3, 1.0, M_SAP, 3.0, 1.00)),
    shrub(sp("Alpenrose", K::FloweringBush, false, 0x5b7f46, EUROPE, (1700.0, 2500.0), (0.0, 90.0), 0.0, 1.5, M_SAP, 2.0, 1.00)),
    // --- Eastern North America / Blue Ridge ---
    dec(sp("White oak", K::Broadleaf, false, 0x5d8a3a, E_NAMERICA, (0.0, 1100.0), (15.0, 38.0), 0.0, 2.5, M_ALL, 40.0, 1.20), 133.0, 285.0, 308.0, 0x8c4a2a),
    dec(sp("Tulip poplar", K::Broadleaf, false, 0x6aa83e, E_NAMERICA, (0.0, 1000.0), (25.0, 55.0), 0.2, 1.8, M_SUB | M_CAN, 55.0, 0.80), 130.0, 278.0, 302.0, 0xcaa42a),
    dec(sp("Red maple", K::Maple, false, 0x8a9a3a, E_NAMERICA, (0.0, 1500.0), (12.0, 35.0), 0.3, 2.0, M_ALL, 36.0, 1.00), 126.0, 275.0, 300.0, 0xa8321e),
    bias(sp("Eastern hemlock", K::Conifer, true, 0x385a3a, E_NAMERICA, (300.0, 1500.0), (18.0, 40.0), 0.6, 1.3, M_ALL, 42.0, 0.80), 1.05),
    sp("Eastern white pine", K::Pine, true, 0x446941, E_NAMERICA, (100.0, 1500.0), (25.0, 50.0), 0.0, 1.0, M_ALL, 52.0, 0.85),
    dec(sp("Sweet birch", K::Birch, false, 0x7ca545, E_NAMERICA, (600.0, 1500.0), (12.0, 30.0), 0.1, 0.8, M_ALL, 30.0, 0.80), 128.0, 280.0, 303.0, 0xc09a2a),
    dec(sp("Hickory", K::Broadleaf, false, 0x6b9442, E_NAMERICA, (0.0, 800.0), (18.0, 40.0), -0.2, 1.2, M_ALL, 40.0, 1.00), 135.0, 283.0, 306.0, 0xb08a2a),
    shrub(sp("Rosebay rhododendron", K::FloweringBush, false, 0x3f6a3a, E_NAMERICA, (300.0, 1500.0), (0.0, 90.0), 0.5, 2.5, M_SAP, 4.0, 1.00)),
    // --- Generic temperate/boreal (always on, low weight) ---
    dec(sp("Broadleaf (generic)", K::Broadleaf, false, 0x5d8a3a, GLOBAL, (0.0, 4000.0), (0.0, 90.0), 0.0, 0.10, M_ALL, 90.0, 1.00), 115.0, 290.0, 315.0, 0x9a6a25),
    sp("Spruce (generic)", K::Conifer, true, 0x3b5d3b, GLOBAL, (0.0, 4000.0), (0.0, 90.0), 0.0, 0.10, M_ALL, 90.0, 0.80),
    dec(sp("Birch (generic)", K::Birch, false, 0x86b04a, GLOBAL, (0.0, 4000.0), (0.0, 90.0), 0.2, 0.05, M_ALL, 90.0, 0.70), 112.0, 285.0, 310.0, 0xc29a28),
    dec(shrub(sp("Shrub (generic)", K::Shrub, false, 0x5f8a40, GLOBAL, (0.0, 4000.0), (0.0, 90.0), 0.0, 0.30, M_SAP, 3.0, 1.00)), 118.0, 290.0, 315.0, 0x8a7a30),
    // --- Tropics ---
    trop(sp("Tropical hardwood", K::Broadleaf, false, 0x4f7f2e, TROPICS, (0.0, 2500.0), (15.0, 90.0), 0.0, 3.0, M_ALL, 90.0, 1.10)),
    trop(sp("Rainforest palm", K::Palm, false, 0x6d9a3c, TROPICS, (0.0, 1500.0), (5.0, 90.0), 0.2, 0.8, M_SAP | M_SUB, 14.0, 0.70)),
    trop(shrub(sp("Tree fern", K::Fern, false, 0x4b8a3a, TROPICS, (0.0, 2500.0), (5.0, 90.0), 0.4, 1.0, M_SAP, 3.0, 1.00))),
    trop(sp("Acacia", K::Acacia, false, 0x7c8e40, TROPICS, (0.0, 2000.0), (3.0, 14.0), -0.5, 0.8, M_ALL, 15.0, 1.30)),
];

/// `1` inside `[lo, hi]`, falling to `0` over `soft` outside it.
fn window(x: f32, lo: f32, hi: f32, soft_lo: f32, soft_hi: f32) -> f32 {
    if x < lo {
        (1.0 - (lo - x) / soft_lo.max(1e-3)).max(0.0)
    } else if x > hi {
        (1.0 - (x - hi) / soft_hi.max(1e-3)).max(0.0)
    } else {
        1.0
    }
}

impl StandSpecies {
    /// Niche weight for the local field; 0 when it cannot grow here.
    fn niche(&self, s: &Samp, class: u8, edge: f32) -> f32 {
        if self.classes & (1 << class) == 0 {
            return 0.0;
        }
        let g = self.geo;
        if s.lat < g[0] || s.lat > g[1] || s.lon < g[2] || s.lon > g[3] {
            return 0.0;
        }
        let mut w = self.weight * window(s.alt, self.alt.0, self.alt.1, 250.0, 250.0);
        if w <= 0.0 {
            return 0.0;
        }
        let h = if s.height > 0.0 { s.height } else { 8.0 };
        w *= window(h, self.canopy_h.0, self.canopy_h.1, 0.3 * self.canopy_h.0 + 1.0, 0.3 * self.canopy_h.1 + 1.0);
        if self.shrub {
            w *= 0.3 + 3.0 * s.under + 2.0 * edge;
        } else {
            let fit = if self.conifer { s.mix } else { 1.0 - s.mix };
            let f = 0.04 + fit;
            w *= f * f.sqrt();
        }
        w * (1.0 + self.wet * (s.wet - 0.3))
    }
}

fn pick_species(s: &Samp, class: u8, edge: f32, roll: f32) -> u8 {
    let mut w = [0f32; STAND_SPECIES.len()];
    let mut sum = 0.0;
    for (i, sp) in STAND_SPECIES.iter().enumerate() {
        w[i] = sp.niche(s, class, edge);
        sum += w[i];
    }
    if sum <= 0.0 {
        return if s.mix > 0.5 { 23 } else { 22 };
    }
    let t = roll * sum;
    let mut acc = 0.0;
    for (i, wi) in w.iter().enumerate() {
        acc += wi;
        if t < acc && *wi > 0.0 {
            return i as u8;
        }
    }
    w.iter().rposition(|x| *x > 0.0).unwrap_or(0) as u8
}

// ---------------------------------------------------------------------------------------------
// Hashing and noise (integer based; no platform-dependent math)
// ---------------------------------------------------------------------------------------------

#[inline]
fn mix64(z: u64) -> u64 {
    let mut z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}
#[inline]
fn unit(x: u64) -> f32 {
    (x >> 40) as f32 / 16_777_216.0
}
#[inline]
fn roll(h: u64, n: u64) -> f32 {
    unit(mix64(h ^ n.wrapping_mul(0x9E37_79B9_7F4A_7C15)))
}

fn lat_hash(s: u64, i: i64, j: i64) -> f32 {
    unit(mix64(s ^ (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (j as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)))
}

/// Value noise on the integer cell grid, `0..1`, feature size `scale` cells.
fn vnoise(s: u64, x: f64, y: f64, scale: f64) -> f32 {
    let (gx, gy) = (x / scale, y / scale);
    let (i, j) = (gx.floor(), gy.floor());
    let (tx, ty) = ((gx - i) as f32, (gy - j) as f32);
    let (tx, ty) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
    let (i, j) = (i as i64, j as i64);
    let a = lat_hash(s, i, j);
    let b = lat_hash(s, i + 1, j);
    let c = lat_hash(s, i, j + 1);
    let d = lat_hash(s, i + 1, j + 1);
    let top = a + (b - a) * tx;
    let bot = c + (d - c) * tx;
    top + (bot - top) * ty
}

// ---------------------------------------------------------------------------------------------
// Field
// ---------------------------------------------------------------------------------------------

/// One local canopy sample, supplied by the caller's closure.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CanopySample {
    /// Crown cover `0..1`.
    pub cover: f32,
    /// Canopy top (m).
    pub height_m: f32,
    /// Conifer share `0..1`.
    pub mix: f32,
    /// Shrub/ground-cover density `0..1`.
    pub under: f32,
    /// Ground altitude (m), for the species niche.
    pub alt_m: f32,
    /// Wetness `0..1` (near water / low ground), 0.3 when unknown.
    pub wet: f32,
}

impl CanopySample {
    /// From a page at node-local `(u, v)` plus the ground altitude and wetness.
    pub fn from_page(page: &CanopyPage, u: f64, v: f64, alt_m: f32, wet: f32) -> CanopySample {
        let b = page.bilinear(u, v);
        CanopySample { cover: b[0], height_m: b[1], mix: b[2], under: b[3], alt_m, wet }
    }
}

#[derive(Clone, Copy, Default)]
struct Samp {
    cover: f32,
    height: f32,
    hpow: f32,
    mix: f32,
    under: f32,
    alt: f32,
    wet: f32,
    lat: f32,
    lon: f32,
    cmax: f32,
    gap: f32,
    wx: f32,
    wy: f32,
    su: f32,
    sv: f32,
}

/// Metres per cell along (i, j) from the equiangular cube map: the face coordinates `a`, `b`
/// are the tangent warps of the lattice position.
fn cell_scales(a: f64, b: f64) -> (f32, f32) {
    let q = 1.0 + a * a + b * b;
    let su = K_M * (1.0 + a * a) * (1.0 + b * b).sqrt() / q;
    let sv = K_M * (1.0 + b * b) * (1.0 + a * a).sqrt() / q;
    (su as f32, sv as f32)
}

/// Metres per face cell `(along i, along j)` at fractional cell coordinates `(fx, fy)`.
pub fn cell_metres(fx: f64, fy: f64) -> (f32, f32) {
    let t = |f: f64| libm::tan(((f / N as f64) * 2.0 - 1.0) * FRAC_PI_4);
    cell_scales(t(fx), t(fy))
}

struct Field {
    cx0: i64,
    cy0: i64,
    w: usize,
    h: usize,
    s: Vec<Samp>,
}

impl Field {
    fn build(face: u8, lo: (i64, i64), hi: (i64, i64), seed: u64, sample: &dyn Fn(DVec3) -> Option<CanopySample>) -> Field {
        let cx0 = (lo.0.div_euclid(COARSE) - 3).max(0);
        let cy0 = (lo.1.div_euclid(COARSE) - 3).max(0);
        let cx1 = (hi.0.div_euclid(COARSE) + 4).min(N / COARSE);
        let cy1 = (hi.1.div_euclid(COARSE) + 4).min(N / COARSE);
        let (w, h) = ((cx1 - cx0 + 1) as usize, (cy1 - cy0 + 1) as usize);
        let ta: Vec<f64> = (cx0..=cx1).map(|c| warp((c * COARSE) as u64, N as u64)).collect();
        let tb: Vec<f64> = (cy0..=cy1).map(|c| warp((c * COARSE) as u64, N as u64)).collect();
        let fs = mix64(seed ^ (face as u64 + 1).wrapping_mul(0xD6E8_FEB8_6659_FD93));
        let (s_gap1, s_gap2, s_wx, s_wy, s_wx2, s_wy2) = (mix64(fs ^ 1), mix64(fs ^ 2), mix64(fs ^ 3), mix64(fs ^ 4), mix64(fs ^ 5), mix64(fs ^ 6));
        let mut s = Vec::with_capacity(w * h);
        for jy in 0..h {
            for ix in 0..w {
                let (gi, gj) = ((cx0 + ix as i64) * COARSE, (cy0 + jy as i64) * COARSE);
                let dir = lattice_point(face, gi as u64, gj as u64, N as u64);
                let c = sample(dir).unwrap_or_default();
                let (su, sv) = cell_scales(ta[ix], tb[jy]);
                let (x, y) = (gi as f64, gj as f64);
                let gap = 0.5 * vnoise(s_gap1, x, y, 5.0) + 0.5 * vnoise(s_gap2, x, y, 14.0);
                let wx = 6.0 * (2.0 * vnoise(s_wx, x, y, 10.0) - 1.0) + 2.0 * (2.0 * vnoise(s_wx2, x, y, 4.0) - 1.0);
                let wy = 6.0 * (2.0 * vnoise(s_wy, x, y, 10.0) - 1.0) + 2.0 * (2.0 * vnoise(s_wy2, x, y, 4.0) - 1.0);
                let hpow = if c.height_m > 0.0 { libm::powf(c.height_m, 0.42) } else { 0.0 };
                s.push(Samp {
                    cover: c.cover.clamp(0.0, 1.0),
                    height: c.height_m,
                    hpow,
                    mix: c.mix.clamp(0.0, 1.0),
                    under: c.under.clamp(0.0, 1.0),
                    alt: c.alt_m,
                    wet: c.wet,
                    lat: (libm::asin(dir.z.clamp(-1.0, 1.0)) * (180.0 / PI)) as f32,
                    lon: (libm::atan2(dir.y, dir.x) * (180.0 / PI)) as f32,
                    cmax: 0.0,
                    gap,
                    wx,
                    wy,
                    su,
                    sv,
                });
            }
        }
        let cover: Vec<f32> = s.iter().map(|p| p.cover).collect();
        for jy in 0..h {
            for ix in 0..w {
                let mut m = 0.0f32;
                for dy in jy.saturating_sub(1)..=(jy + 1).min(h - 1) {
                    for dx in ix.saturating_sub(1)..=(ix + 1).min(w - 1) {
                        m = m.max(cover[dy * w + dx]);
                    }
                }
                s[jy * w + ix].cmax = m;
            }
        }
        Field { cx0, cy0, w, h, s }
    }

    /// Bilinear sample at absolute fractional cell coordinates.
    fn at(&self, fx: f64, fy: f64) -> Samp {
        let gx = (fx / COARSE as f64 - self.cx0 as f64).clamp(0.0, (self.w - 1) as f64 - 1e-9);
        let gy = (fy / COARSE as f64 - self.cy0 as f64).clamp(0.0, (self.h - 1) as f64 - 1e-9);
        let (i, j) = (gx as usize, gy as usize);
        let (tx, ty) = ((gx - i as f64) as f32, (gy - j as f64) as f32);
        let w = [(1.0 - tx) * (1.0 - ty), tx * (1.0 - ty), (1.0 - tx) * ty, tx * ty];
        let p = [&self.s[j * self.w + i], &self.s[j * self.w + i + 1], &self.s[(j + 1) * self.w + i], &self.s[(j + 1) * self.w + i + 1]];
        let mut o = Samp::default();
        let (mut hw, mut hs, mut ps, mut ms) = (0.0, 0.0, 0.0, 0.0);
        for k in 0..4 {
            let q = p[k];
            o.cover += w[k] * q.cover;
            let ww = w[k] * q.cover;
            hw += ww;
            hs += ww * q.height;
            ps += ww * q.hpow;
            ms += ww * q.mix;
            o.under += w[k] * q.under;
            o.alt += w[k] * q.alt;
            o.wet += w[k] * q.wet;
            o.lat += w[k] * q.lat;
            o.lon += w[k] * q.lon;
            o.cmax += w[k] * q.cmax;
            o.gap += w[k] * q.gap;
            o.wx += w[k] * q.wx;
            o.wy += w[k] * q.wy;
            o.su += w[k] * q.su;
            o.sv += w[k] * q.sv;
        }
        if hw > 0.0 {
            o.height = hs / hw;
            o.hpow = ps / hw;
            o.mix = ms / hw;
        } else {
            // No cover here: take the nearest corner's height so sparse trees keep a size.
            let q = p[if tx > 0.5 { 1 } else { 0 } + if ty > 0.5 { 2 } else { 0 }];
            o.height = q.height;
            o.hpow = q.hpow;
            o.mix = q.mix;
        }
        o
    }
}

// ---------------------------------------------------------------------------------------------
// Solve
// ---------------------------------------------------------------------------------------------

/// One placed tree.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StandTree {
    /// Unit body direction of the trunk.
    pub dir: DVec3,
    /// Position in face cell coordinates (`cell_metres` converts to metres).
    pub fx: f64,
    pub fy: f64,
    pub height_m: f32,
    pub crown_r_m: f32,
    /// Index into [`STAND_SPECIES`].
    pub species: u8,
    pub kind: TreeKind,
    /// 0 sapling, 1 sub-canopy, 2 canopy.
    pub age_class: u8,
    pub seed: u32,
    pub id: ObjectId,
}

impl StandTree {
    pub fn species_def(&self) -> &'static StandSpecies {
        &STAND_SPECIES[self.species as usize]
    }

    /// Trunk radius (m): diameter at breast height from a height allometry
    /// (`dbh = 0.0095 h^1.2`, about 0.35 m at 20 m), conifers 15% slimmer. Shrubs get 0.
    pub fn trunk_radius_m(&self) -> f32 {
        let sp = self.species_def();
        if sp.shrub {
            return 0.0;
        }
        let dbh = 0.0095 * self.height_m.max(0.0).powf(1.2) * if sp.conifer { 0.85 } else { 1.0 };
        0.5 * dbh
    }

    /// Collidable trunk height (m): up to roughly the crown base (broadleaf 40%, conifer 25% of height).
    pub fn trunk_height_m(&self) -> f32 {
        self.height_m * if self.species_def().conifer { 0.25 } else { 0.4 }
    }
}

#[derive(Clone, Copy)]
struct Cand {
    ci: i32,
    cj: i32,
    class: u8,
    qx: u8,
    qy: u8,
    species: u8,
    rx: f32,
    ry: f32,
    su: f32,
    sv: f32,
    r: f32,
    h: f32,
    rank: u64,
    seed: u32,
    hk: u64,
}

fn ln_table() -> Vec<f32> {
    (0..=1024).map(|i| -libm::logf(1.0 - (i as f32 / 1024.0).min(0.97))).collect()
}

/// Trees of the cell block `[i0, i1) x [j0, j1)` on `face`: the whole solve. The block is
/// extended by [`HALO_M`] internally; the result depends only on the data inside that halo.
pub fn stand_for_cells(face: u8, i0: i64, i1: i64, j0: i64, j1: i64, sample: &dyn Fn(DVec3) -> Option<CanopySample>, seed: u64) -> Vec<StandTree> {
    let halo = (HALO_M / MIN_CELL_M).ceil() as i64 + 1;
    let (ri0, ri1, rj0, rj1) = ((i0 - halo).max(0), (i1 + halo).min(N), (j0 - halo).max(0), (j1 + halo).min(N));
    let field = Field::build(face, (ri0, rj0), (ri1, rj1), seed, sample);
    let lnt = ln_table();
    let face_seed = mix64(seed ^ (face as u64 + 1).wrapping_mul(0xA24B_AED4_963E_E407));

    // ---- candidates that are alive ----
    let mut cands: Vec<Cand> = Vec::new();
    for cj in rj0..rj1 {
        for ci in ri0..ri1 {
            let h0 = mix64(face_seed ^ (((ci as u64) << 24) ^ (cj as u64)).wrapping_mul(0x9E37_79B9_7F4A_7C15));
            let kc = 1 + (h0 >> 63) as u8;
            for k in 0..kc {
                let hk = mix64(h0 ^ (k as u64 + 1).wrapping_mul(0xA24B_AED4_963E_E407));
                let jit = mix64(hk ^ 0x51);
                let (qx, qy) = ((jit & 0xFF) as u8, ((jit >> 8) & 0xFF) as u8);
                let (px, py) = (ci as f64 + qx as f64 / SUB as f64, cj as f64 + qy as f64 / SUB as f64);
                let b = field.at(px, py);
                if b.cover < 0.002 && b.cmax < 0.002 {
                    continue;
                }
                let s = field.at(px + (b.wx / b.su) as f64, py + (b.wy / b.sv) as f64);
                let c = s.cover;
                if c < 0.002 {
                    continue;
                }
                let edge = (s.cmax - c).clamp(0.0, 1.0);
                // Class from the roll; edges favour saplings and shrubs.
                let rc = roll(hk, 1);
                let t2 = CLASSES[2].frac * (1.0 - 0.7 * edge);
                let t1 = t2 + CLASSES[1].frac;
                let class = if rc < t2 { CANOPY } else if rc < t1 { SUB_CANOPY } else { SAPLING };
                let cp = CLASSES[class as usize];
                let hcan = if s.height > 0.0 { s.height } else { 8.0 };
                let r_base = 0.72 * (if s.hpow > 0.0 { s.hpow } else { libm::powf(hcan, 0.42) }) * cp.r_factor * (1.0 + 0.25 * c);
                let r_base = r_base.clamp(0.35, CROWN_MAX_M);
                let short = 1.0 + 0.25 * ((14.0 - hcan) / 11.0).clamp(0.0, 1.0);
                let cc = (prewarp(c) * short * cp.share).min(0.97);
                let rho = lnt[(cc * 1024.0) as usize] / (PI as f32 * r_base * r_base);
                let mut p = cp.kappa * rho * (s.su * s.sv) / (K_AVG * cp.frac);
                // Gap dynamics: clearings where the low-frequency noise dips, strongest in closed forest.
                let gt = GAP_STRENGTH * ((c - 0.35) / 0.4).clamp(0.0, 1.0);
                let gm = ((gt - s.gap) / 0.06).clamp(0.0, 1.0);
                p *= 1.0 - gm;
                // Edge band: more saplings, fewer big trees.
                if class == SAPLING {
                    p *= 1.0 + 1.5 * edge + 0.8 * s.under;
                } else if class == CANOPY {
                    p *= 1.0 - 0.5 * edge;
                }
                if roll(hk, 2) >= p {
                    continue;
                }
                // Alive: species, size, rank.
                let sp_i = pick_species(&s, class, edge, roll(hk, 3));
                let sp = &STAND_SPECIES[sp_i as usize];
                let hh = (hcan * (cp.height.0 + cp.height.1 * roll(hk, 4))).min(sp.max_h).max(0.5);
                let r = crown_radius(hh, c, sp.crown_k);
                let pri = (mix64(hk ^ 0x77) >> 32) as u64;
                let tie = mix64(hk ^ 0x99) & 0x3FFF_FFFF;
                cands.push(Cand {
                    ci: ci as i32,
                    cj: cj as i32,
                    class,
                    qx,
                    qy,
                    species: sp_i,
                    rx: (ci - ri0) as f32 + qx as f32 / SUB as f32,
                    ry: (cj - rj0) as f32 + qy as f32 / SUB as f32,
                    su: s.su,
                    sv: s.sv,
                    r,
                    h: hh,
                    rank: (class as u64) << 62 | pri << 30 | tie,
                    seed: (mix64(hk ^ 0x33) >> 32) as u32,
                    hk,
                });
            }
        }
    }

    let (cands, accepted) = dominate(cands, ((ri1 - ri0) as usize, (rj1 - rj0) as usize));

    // ---- emit trees owned by the block ----
    let mut out = Vec::new();
    for (x, c) in cands.iter().enumerate() {
        if !accepted[x] || (c.ci as i64) < i0 || (c.ci as i64) >= i1 || (c.cj as i64) < j0 || (c.cj as i64) >= j1 {
            continue;
        }
        let dir = lattice_point(face, c.ci as u64 * SUB as u64 + c.qx as u64, c.cj as u64 * SUB as u64 + c.qy as u64, (N * SUB) as u64);
        let sp = &STAND_SPECIES[c.species as usize];
        let idh = mix64(c.hk ^ 0xABCD);
        let id = ObjectId((((idh as u128) << 64) | mix64(idh ^ 0x1234) as u128) & !(1u128 << 127));
        out.push(StandTree {
            dir,
            fx: c.ci as f64 + c.qx as f64 / SUB as f64,
            fy: c.cj as f64 + c.qy as f64 / SUB as f64,
            height_m: c.h,
            crown_r_m: c.r,
            species: c.species,
            kind: sp.kind,
            age_class: c.class,
            seed: c.seed,
            id,
        });
    }
    out.sort_by_key(|t| t.id);
    out
}

/// Conflict test: centres closer than `COMPETE * (r_a + r_b)` metres (metric from the mean cell scale).
#[inline]
fn conflict(a: &Cand, b: &Cand) -> bool {
    let (su, sv) = (0.5 * (a.su + b.su), 0.5 * (a.sv + b.sv));
    let (dx, dy) = ((a.rx - b.rx) * su, (a.ry - b.ry) * sv);
    let d = COMPETE[a.class as usize][b.class as usize] * (a.r + b.r);
    dx * dx + dy * dy < d * d
}

/// `PASSES` rounds of "accept the local maxima of the undecided, eliminate their neighbours".
fn dominate(input: Vec<Cand>, dims: (usize, usize)) -> (Vec<Cand>, Vec<bool>) {
    let n = input.len();
    if n == 0 {
        return (input, Vec::new());
    }
    let c = &input[..];
    let rmax = c.iter().fold(0.0f32, |m, x| m.max(x.r));
    let amax = COMPETE.iter().flatten().fold(0.0f32, |m, &a| m.max(a));
    debug_assert!(amax * 2.0 * CROWN_MAX_M <= R_MAX_M);
    let bs = ((2.0 * amax * rmax / MIN_CELL_M).ceil() as usize).max(2);
    let (bw, bh) = (dims.0 / bs + 1, dims.1 / bs + 1);
    let bucket = |x: &Cand| ((x.ry as usize) / bs).min(bh - 1) * bw + ((x.rx as usize) / bs).min(bw - 1);
    let mut start = vec![0u32; bw * bh + 1];
    for x in c {
        start[bucket(x) + 1] += 1;
    }
    for i in 0..bw * bh {
        start[i + 1] += start[i];
    }
    // Counting-sort the candidates by bucket (stable, so deterministic) for cache locality.
    let mut fill = start.clone();
    let mut sorted: Vec<Cand> = Vec::with_capacity(n);
    let mut order = vec![0u32; n];
    for (i, x) in c.iter().enumerate() {
        let b = bucket(x);
        order[fill[b] as usize] = i as u32;
        fill[b] += 1;
    }
    sorted.extend(order.iter().map(|&i| c[i as usize]));
    drop(order);
    let c = &sorted[..];
    // Conflict pairs (each found once, from the lower index), then CSR both ways.
    let mut pairs: Vec<(u32, u32)> = Vec::new();
    for (i, x) in c.iter().enumerate() {
        let (bx, by) = (bucket(x) % bw, bucket(x) / bw);
        for yy in by.saturating_sub(1)..=(by + 1).min(bh - 1) {
            for xx in bx.saturating_sub(1)..=(bx + 1).min(bw - 1) {
                let b = yy * bw + xx;
                let lo = (start[b] as usize).max(i + 1);
                for j in lo..start[b + 1] as usize {
                    if conflict(x, &c[j]) {
                        pairs.push((i as u32, j as u32));
                    }
                }
            }
        }
    }
    let mut adj_start = vec![0u32; n + 1];
    for &(a, b) in &pairs {
        adj_start[a as usize + 1] += 1;
        adj_start[b as usize + 1] += 1;
    }
    for i in 0..n {
        adj_start[i + 1] += adj_start[i];
    }
    let mut fill = adj_start.clone();
    let mut adj = vec![0u32; pairs.len() * 2];
    for &(a, b) in &pairs {
        adj[fill[a as usize] as usize] = b;
        fill[a as usize] += 1;
        adj[fill[b as usize] as usize] = a;
        fill[b as usize] += 1;
    }
    drop(pairs);
    let nb = |i: usize| &adj[adj_start[i] as usize..adj_start[i + 1] as usize];
    // 0 undecided, 1 accepted, 2 eliminated.
    let mut state = vec![0u8; n];
    let mut newly: Vec<u32> = Vec::new();
    for _ in 0..PASSES {
        newly.clear();
        for i in 0..n {
            if state[i] == 0 && !nb(i).iter().any(|&j| state[j as usize] == 0 && c[j as usize].rank > c[i].rank) {
                newly.push(i as u32);
            }
        }
        for &i in &newly {
            state[i as usize] = 1;
        }
        for i in 0..n {
            if state[i] == 0 && nb(i).iter().any(|&j| state[j as usize] == 1) {
                state[i] = 2;
            }
        }
    }
    let acc = state.iter().map(|&s| s == 1).collect();
    (sorted, acc)
}

/// Trees whose cell lies inside node `id` (half-open, so each tree belongs to exactly one
/// node), sorted by `ObjectId`. `sample(dir)` returns the canopy field at a body direction,
/// from the page of whichever node contains it (neighbours' pages serve the halo); `None` is
/// "no canopy". The node must be at level 22 or shallower; stands are meant for levels >= 11.
pub fn stand_for_node(id: NodeId, sample: &dyn Fn(DVec3) -> Option<CanopySample>, seed: u64) -> Vec<StandTree> {
    let l = id.level() as u32;
    assert!(l <= FACE_LEVEL, "stand: node level {l} is finer than the candidate lattice");
    let side = 1i64 << (FACE_LEVEL - l);
    let (i0, j0) = (id.x() as i64 * side, id.y() as i64 * side);
    stand_for_cells(id.face(), i0, i0 + side, j0, j0 + side, sample, seed)
}

// ---------------------------------------------------------------------------------------------
// Write-back
// ---------------------------------------------------------------------------------------------

/// What the stand measured on the page lattice (`(cells + 1)^2`, row `j`, column `i`).
#[derive(Clone, Debug)]
pub struct Measured {
    pub cells: usize,
    /// Crown cover fraction.
    pub cover: Vec<f32>,
    /// Mean top height (m) of the crowns covering the sample (0 where uncovered).
    pub height: Vec<f32>,
    /// Conifer share of the covering crowns.
    pub mix: Vec<f32>,
}

const SS: usize = 8;

/// Rasterises the crown discs onto the page lattice of `id` (`cells` cells per side, 256 for a
/// page): each sample is the covered fraction of its footprint, taller crowns on top.
pub fn measure(id: NodeId, trees: &[StandTree], cells: usize) -> Measured {
    let l = id.level() as u32;
    let side = (1i64 << (FACE_LEVEL - l)) as f64;
    let step = side / cells as f64; // face cells per lattice step
    let (i0, j0) = (id.x() as f64 * side, id.y() as f64 * side);
    let np = cells + 1;
    let res = np * SS;
    let mut top = vec![0f32; res * res];
    let mut con = vec![0u8; res * res];
    // Pixel (px, py) centre, in face cells.
    let pix = |p: usize, o: f64| o + ((p as f64 + 0.5) / SS as f64 - 0.5) * step;
    for t in trees {
        let (su, sv) = cell_metres(t.fx, t.fy);
        let r_m = t.crown_r_m as f64 * t.species_def().crown_bias as f64;
        let (rx, ry) = (r_m / su as f64, r_m / sv as f64);
        let (cx, cy) = (t.fx, t.fy);
        // Pixel index of face-cell coordinate f: p = ((f - o)/step + 0.5) * SS - 0.5.
        let to_p = |f: f64, o: f64| ((f - o) / step + 0.5) * SS as f64 - 0.5;
        let (p0, p1) = (to_p(cx - rx, i0).floor().max(0.0) as usize, (to_p(cx + rx, i0).ceil().max(-1.0) + 1.0).min(res as f64) as usize);
        let (q0, q1) = (to_p(cy - ry, j0).floor().max(0.0) as usize, (to_p(cy + ry, j0).ceil().max(-1.0) + 1.0).min(res as f64) as usize);
        let conifer = t.species_def().conifer as u8;
        for q in q0..q1 {
            let dy = (pix(q, j0) - cy) / ry;
            for p in p0..p1 {
                let dx = (pix(p, i0) - cx) / rx;
                if dx * dx + dy * dy <= 1.0 && t.height_m > top[q * res + p] {
                    top[q * res + p] = t.height_m;
                    con[q * res + p] = conifer + 1;
                }
            }
        }
    }
    let mut m = Measured { cells, cover: vec![0.0; np * np], height: vec![0.0; np * np], mix: vec![0.0; np * np] };
    for j in 0..np {
        for i in 0..np {
            let (mut n, mut hs, mut cs) = (0u32, 0.0f32, 0u32);
            for q in 0..SS {
                for p in 0..SS {
                    let k = (j * SS + q) * res + i * SS + p;
                    if con[k] != 0 {
                        n += 1;
                        hs += top[k];
                        cs += (con[k] - 1) as u32;
                    }
                }
            }
            let k = j * np + i;
            m.cover[k] = n as f32 / (SS * SS) as f32;
            if n > 0 {
                m.height[k] = hs / n as f32;
                m.mix[k] = cs as f32 / n as f32;
            }
        }
    }
    m
}

/// Crown cover per page sample (see [`measure`]).
pub fn measure_cover(id: NodeId, trees: &[StandTree], cells: usize) -> Vec<f32> {
    measure(id, trees, cells).cover
}

/// A page whose cover, canopy height and mix are what the stand measured; understory is kept.
pub fn write_back(page: &CanopyPage, m: &Measured) -> CanopyPage {
    assert_eq!(m.cells, PAGE_CELLS, "write_back needs a page-lattice measurement");
    CanopyPage::from_fn(|i, j| {
        let k = j * PAGE_N + i;
        let old = page.get(i, j);
        [(m.cover[k].clamp(0.0, 1.0) * 255.0).round() as u8, encode_height(m.height[k]), (m.mix[k].clamp(0.0, 1.0) * 255.0).round() as u8, old[3]]
    })
}

/// Area-weighted mean cover (`0..1`) of a page and of a measurement (for the 5 % gate).
pub fn mean_cover(page: &CanopyPage) -> f32 {
    page.samples.iter().map(|s| s[0] as f32 / 255.0).sum::<f32>() / page.samples.len() as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed_geo::node::BodyId;

    fn node_at(lat: f64, lon: f64, level: u8) -> NodeId {
        let d = ed_geo::Geodetic::new(lat, lon, 0.0).to_ecef().normalize();
        NodeId::from_unit(BodyId(0), d, level)
    }

    fn flat(cover: f32, h: f32) -> impl Fn(DVec3) -> Option<CanopySample> {
        move |_| Some(CanopySample { cover, height_m: h, mix: 0.5, under: 0.3, alt_m: 900.0, wet: 0.3 })
    }

    fn area_m2(id: NodeId) -> f64 {
        let (su, sv) = cell_metres((id.x() as f64 + 0.5) * (1u64 << (FACE_LEVEL - id.level() as u32)) as f64, (id.y() as f64 + 0.5) * (1u64 << (FACE_LEVEL - id.level() as u32)) as f64);
        let side = (1u64 << (FACE_LEVEL - id.level() as u32)) as f64;
        side * su as f64 * side * sv as f64
    }

    /// Cover of a stand over a node as the mean of the measured page (interior samples only,
    /// so the crowns of neighbours overhanging the edge do not bias it).
    fn measured_cover(id: NodeId, f: &dyn Fn(DVec3) -> Option<CanopySample>) -> (f32, Vec<StandTree>) {
        let trees = stand_for_node(id, f, 7);
        let m = measure(id, &trees, PAGE_CELLS);
        let mut s = 0.0;
        let mut n = 0.0;
        for j in 12..PAGE_N - 12 {
            for i in 12..PAGE_N - 12 {
                s += m.cover[j * PAGE_N + i];
                n += 1.0;
            }
        }
        (s / n, trees)
    }

    /// Ids and trunks are a pure function of the node: loading nodes in another order, or
    /// twice, gives identical ids, in id order, with identical trunk sizes (G10).
    #[test]
    fn ids_and_trunks_independent_of_load_order() {
        let f = flat(0.7, 25.0);
        let c = node_at(46.02, 7.75, 15);
        let nodes: Vec<NodeId> = (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (dx, dy))).map(|(dx, dy)| c.neighbour(dx, dy)).collect();
        let key = |t: &StandTree| (t.id, t.trunk_radius_m().to_bits(), t.trunk_height_m().to_bits());
        let fwd: Vec<Vec<_>> = nodes.iter().map(|&n| stand_for_node(n, &f, 5).iter().map(key).collect()).collect();
        let mut rev: Vec<(usize, Vec<_>)> = nodes.iter().enumerate().rev().map(|(i, &n)| (i, stand_for_node(n, &f, 5).iter().map(key).collect())).collect();
        rev.sort_by_key(|r| r.0);
        for (i, (a, b)) in fwd.iter().zip(&rev).enumerate() {
            assert!(!a.is_empty(), "node {i} empty");
            assert_eq!(a, &b.1);
            assert!(a.windows(2).all(|w| w[0].0 < w[1].0));
        }
        // No tree appears in two nodes.
        let mut all: Vec<_> = fwd.iter().flatten().map(|k| k.0).collect();
        let n = all.len();
        all.sort();
        all.dedup();
        assert_eq!(all.len(), n);
    }

    #[test]
    fn trunk_sizes_are_sane() {
        let f = flat(0.8, 25.0);
        let trees = stand_for_node(node_at(46.02, 7.75, 15), &f, 5);
        let big = trees.iter().filter(|t| t.height_m > 18.0 && !t.species_def().shrub).collect::<Vec<_>>();
        assert!(!big.is_empty());
        for t in big {
            let r = t.trunk_radius_m();
            assert!((0.08..0.5).contains(&r), "radius {r} for h {}", t.height_m);
            assert!(t.trunk_height_m() < t.height_m);
        }
    }

    #[test]
    fn deterministic() {
        let id = node_at(46.02, 7.75, 15);
        let f = flat(0.7, 25.0);
        let a = stand_for_node(id, &f, 1);
        let b = stand_for_node(id, &f, 1);
        assert!(!a.is_empty());
        assert_eq!(a, b);
        let c = stand_for_node(id, &f, 2);
        assert_ne!(a, c);
        // Ids are unique and sorted.
        assert!(a.windows(2).all(|w| w[0].id < w[1].id));
    }

    /// Evaluating a parent and its four children (each with its own halo, so each runs a
    /// different window) gives the same trees: nothing accepted by one evaluation is rejected
    /// by another, and no tree appears twice.
    #[test]
    fn seams_children_equal_parent() {
        for (cover, h) in [(0.9, 28.0), (0.5, 12.0)] {
            // A field with structure: cover varies in space so the edge/gap paths are used.
            let f = move |d: DVec3| {
                let t = ((d.x * 4.0e4).sin() * (d.y * 3.0e4).cos()) as f32 * 0.5 + 0.5;
                Some(CanopySample { cover: (cover * (0.4 + 0.6 * t)).min(1.0), height_m: h, mix: t, under: 0.3, alt_m: 800.0, wet: 0.3 })
            };
            let p = node_at(46.02, 7.75, 14);
            let whole = stand_for_node(p, &f, 3);
            let mut parts: Vec<StandTree> = p.children().unwrap().iter().flat_map(|c| stand_for_node(*c, &f, 3)).collect();
            parts.sort_by_key(|t| t.id);
            assert!(whole.len() > 100);
            assert_eq!(whole.len(), parts.len(), "cover {cover}");
            assert_eq!(whole, parts, "cover {cover}");
            // Two neighbours: strips each side of the shared edge, evaluated by both.
            let a = node_at(46.02, 7.75, 15);
            let b = a.neighbour(1, 0);
            let (ta, tb) = (stand_for_node(a, &f, 3), stand_for_node(b, &f, 3));
            let mut ids: Vec<_> = ta.iter().chain(tb.iter()).map(|t| t.id).collect();
            let n = ids.len();
            ids.sort();
            ids.dedup();
            assert_eq!(ids.len(), n, "duplicate across the shared edge");
            let side = 1i64 << (FACE_LEVEL - 15);
            let (i0, j0) = (a.x() as i64 * side, a.y() as i64 * side);
            // A window straddling the edge sees exactly the union of what each node placed in it.
            let win = stand_for_cells(a.face(), i0 + side - 40, i0 + side + 40, j0 + 20, j0 + side - 20, &f, 3);
            let mut want: Vec<StandTree> = ta.iter().chain(tb.iter()).filter(|t| t.fx >= (i0 + side - 40) as f64 && t.fx < (i0 + side + 40) as f64 && t.fy >= (j0 + 20) as f64 && t.fy < (j0 + side - 20) as f64).cloned().collect();
            want.sort_by_key(|t| t.id);
            assert!(!win.is_empty());
            assert_eq!(win, want);
        }
    }

    #[test]
    fn cover_matches_page() {
        let id = node_at(46.02, 7.75, 14);
        for (cover, h) in [(0.3, 20.0), (0.7, 25.0), (1.0, 30.0)] {
            let (got, trees) = measured_cover(id, &flat(cover, h));
            let err = (got - cover).abs() / cover;
            eprintln!("cover {cover}: measured {got:.3} ({:+.1} %), {} trees", 100.0 * (got - cover) / cover, trees.len());
            assert!(err < 0.05, "cover {cover} h {h}: got {got}");
        }
    }

    /// Prints measured cover over a grid of (cover, height) to tune [`CLASSES`].
    #[test]
    #[ignore = "calibration"]
    fn calibrate_cover() {
        let id = node_at(46.02, 7.75, 14);
        for h in [3.0f32, 12.0, 25.0, 35.0] {
            let mut line = format!("h {h:>4}: ");
            for cover in [0.05f32, 0.15, 0.3, 0.5, 0.7, 0.85, 1.0] {
                let (got, _) = measured_cover(id, &flat(cover, h));
                line += &format!("{cover:.2}->{got:.3}  ");
            }
            eprintln!("{line}");
        }
    }

    #[test]
    fn no_two_trees_closer_than_the_rule() {
        let id = node_at(46.02, 7.75, 15);
        let trees = stand_for_node(id, &flat(0.8, 28.0), 5);
        let mut checked = 0;
        for (a, t) in trees.iter().enumerate() {
            for u in &trees[a + 1..] {
                let (su, sv) = cell_metres(t.fx, t.fy);
                let (dx, dy) = ((t.fx - u.fx) as f32 * su, (t.fy - u.fy) as f32 * sv);
                let d = (dx * dx + dy * dy).sqrt();
                let rule = COMPETE[t.age_class as usize][u.age_class as usize] * (t.crown_r_m + u.crown_r_m);
                if d < 20.0 {
                    checked += 1;
                }
                // Accepted pairs in the survivor set never violate the rule (mean-scale metric,
                // so allow the tiny scale difference between the two ends).
                assert!(d >= rule * 0.995, "{d} < {rule}");
            }
        }
        assert!(checked > 1000);
    }

    #[test]
    fn jungle_and_tundra() {
        let id = node_at(46.02, 7.75, 14);
        let a = area_m2(id) / 1.0e4; // hectares
        // Jungle-like: cover 0.95, 35 m. Tropical latitude is irrelevant to the count.
        let jungle = stand_for_node(id, &flat(0.95, 35.0), 11);
        let can = jungle.iter().filter(|t| t.age_class == CANOPY).count() as f64 / a;
        let all = jungle.len() as f64 / a;
        eprintln!("jungle: {can:.0} canopy trees/ha, {all:.0} total/ha");
        assert!((250.0..=400.0).contains(&can), "{can}");
        assert!(all > can * 1.3, "understory missing");
        // Tundra: cover 0.03, 4 m: lone trees.
        let tundra: Vec<StandTree> = stand_for_node(id, &flat(0.03, 4.0), 11).into_iter().filter(|t| t.age_class == CANOPY).collect();
        let per_ha = tundra.len() as f64 / a;
        eprintln!("tundra: {per_ha:.1} canopy trees/ha");
        assert!(tundra.len() > 20 && per_ha < 80.0);
        // Lone trees: the nearest neighbour of a tree is several crown radii away on average.
        let mut sum = 0.0;
        for t in tundra.iter().take(200) {
            let (su, sv) = cell_metres(t.fx, t.fy);
            let near = tundra.iter().filter(|u| u.id != t.id).map(|u| {
                let (dx, dy) = ((t.fx - u.fx) as f32 * su, (t.fy - u.fy) as f32 * sv);
                (dx * dx + dy * dy).sqrt() / t.crown_r_m
            });
            sum += near.fold(f32::MAX, f32::min);
        }
        assert!(sum / 200.0 > 2.5, "mean nearest neighbour {} crowns", sum / 200.0);
    }

    #[test]
    fn write_back_agrees_with_input() {
        let id = node_at(46.02, 7.75, 14);
        let f = flat(0.7, 25.0);
        let trees = stand_for_node(id, &f, 9);
        let page = CanopyPage::from_fn(|_, _| [(0.7f32 * 255.0).round() as u8, encode_height(25.0), 128, 40]);
        let m = measure(id, &trees, PAGE_CELLS);
        let out = write_back(&page, &m);
        // Interior mean cover within 5 % of the page.
        let (mut s, mut n) = (0.0f32, 0.0f32);
        for j in 12..PAGE_N - 12 {
            for i in 12..PAGE_N - 12 {
                s += out.cover(i, j);
                n += 1.0;
            }
        }
        let got = s / n;
        assert!((got - 0.7).abs() / 0.7 < 0.05, "{got}");
        // Canopy height written back is the crown tops, in the right ballpark.
        let mut hs = 0.0;
        let mut hn = 0.0;
        for j in 12..PAGE_N - 12 {
            for i in 12..PAGE_N - 12 {
                if out.get(i, j)[1] > 0 {
                    hs += out.height_m(i, j);
                    hn += 1.0;
                }
            }
        }
        let mh = hs / hn;
        assert!((15.0..32.0).contains(&mh), "{mh}");
        assert_eq!(out.get(5, 5)[3], 40);
    }

    #[test]
    fn species_follow_altitude_and_mix() {
        let id = node_at(46.02, 7.75, 14);
        let f = |alt: f32, mix: f32| move |_: DVec3| Some(CanopySample { cover: 0.8, height_m: 22.0, mix, under: 0.3, alt_m: alt, wet: 0.3 });
        let count = |trees: &[StandTree], name: &str| trees.iter().filter(|t| t.species_def().name == name && t.age_class == CANOPY).count();
        let high = stand_for_node(id, &f(2100.0, 0.95), 4);
        assert!(count(&high, "European larch") + count(&high, "Swiss stone pine") > count(&high, "European beech") * 3);
        let low = stand_for_node(id, &f(500.0, 0.05), 4);
        assert!(count(&low, "European beech") + count(&low, "Pedunculate oak") > count(&low, "European larch") * 3);
        // Species always map to a kind in the existing table.
        assert!(low.iter().all(|t| (t.kind as usize) < crate::trees::species::SPECIES.len()));
    }

    /// Release timing: trees and ms per level-12 node (6 km^2) at full forest.
    #[test]
    #[ignore = "timing; run with --release --nocapture"]
    fn timing_level12() {
        let id = node_at(46.02, 7.75, 12);
        let km2 = area_m2(id) / 1.0e6;
        let f = flat(0.95, 30.0);
        let t = std::time::Instant::now();
        let trees = stand_for_node(id, &f, 1);
        let ms = t.elapsed().as_secs_f64() * 1e3;
        let t = std::time::Instant::now();
        let m = measure(id, &trees, PAGE_CELLS);
        let ms_m = t.elapsed().as_secs_f64() * 1e3;
        eprintln!("level 12: {km2:.2} km2, {} trees ({:.0}/ha), stand {ms:.0} ms ({:.0} ms/km2), measure {ms_m:.0} ms, cover {:.3}", trees.len(), trees.len() as f64 / (km2 * 100.0), ms / km2, m.cover.iter().sum::<f32>() / m.cover.len() as f32);
    }
}
