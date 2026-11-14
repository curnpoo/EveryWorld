//! Grass style A (Ghost-of-Tsushima-style procedural grass): deterministic **clump records**
//! per ground node; the renderer expands every clump into curved blades on the GPU from the
//! clump's seed and its [`GrassType`] row (`ed_render::grass`, `shaders/grass` in
//! visibility.slang and resolve.slang). No meshes: a clump is 32 bytes.
//!
//! **Clumps.** One candidate per cell of a global cube-face lattice (level [`CLUMP_LEVEL`],
//! ~0.6 m, so ~0.36 m² per clump), jittered inside its cell; row crops (`aniso < 1`) sit on the
//! cell centres and face along the lattice's `j` axis, so they line up into rows. What grows
//! comes from a caller's type function (real ground: [`type_from_ground`]; the grass lab:
//! [`lab_type`]); a clump exists when its hash is under the type's density times the
//! ground's. Height varies in patches (a ~9 m noise field) and per clump; a slower field
//! makes some patches drier (straw-tinted).
//!
//! **Distance** ([`lod`], mirrored by common.slang `grass_lod`): blades per clump fall as
//! `(r0 / d)^2` past `r0` (constant on-screen density), down to `bmin`; past that the clumps
//! themselves thin by rank. The generator emits only ranks under the node's keep (a tier of
//! [`clump_keep`] at its nearest point), so far nodes carry few clumps.
//!
//! Deterministic: a pure function of (node, seed, type function); fixed lattice order, seeded
//! hashes only, no maps.

use crate::ground_cover::{Climate, Frame, GroundSample, SUB, candidate, cell_key, cells, layer_weights};
use crate::noise::{fbm3, smoothstep};
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
}

pub const LAWN: u8 = 0;
pub const MEADOW: u8 = 1;
pub const LONG: u8 = 2;
pub const WHEAT: u8 = 3;

/// The type table (index = `kind`).
pub const TYPES: [GrassType; 4] = [
    GrassType {
        name: "lawn",
        height: (0.12, 0.18),
        width: 0.02,
        blades: 64,
        clump_r: 0.5,
        aniso: 1.0,
        lean: 0.5,
        stiffness: 0.85,
        curve: 0.35,
        root: [0.040, 0.060, 0.016],
        mid: [0.085, 0.130, 0.032],
        tip: [0.115, 0.160, 0.045],
        dry: [0.22, 0.20, 0.09],
        head: 0.0,
        head_w: 1.0,
        density: 0.85,
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
    },
];

/// Words per GPU row (six float4).
pub const ROW_FLOATS: usize = 24;

/// The GPU row of `t` (common.slang `GrassType`): (h min, h max, width, blades),
/// (clump r, lean, stiffness, curve), (root, aniso), (mid, head), (tip, head width),
/// (dry, density).
pub fn gpu_row(t: &GrassType) -> [f32; ROW_FLOATS] {
    let [r, m, p, d] = [t.root, t.mid, t.tip, t.dry];
    [
        t.height.0, t.height.1, t.width, t.blades.min(MAX_BLADES) as f32,
        t.clump_r, t.lean, t.stiffness, t.curve,
        r[0], r[1], r[2], t.aniso,
        m[0], m[1], m[2], t.head,
        p[0], p[1], p[2], t.head_w,
        d[0], d[1], d[2], t.density,
    ]
}

/// Every type's GPU row, in order.
pub fn gpu_rows() -> Vec<[f32; ROW_FLOATS]> {
    TYPES.iter().map(gpu_row).collect()
}

/// Continuous blades per clump and the clump keep at distance `d` (m) for a type of `blades`
/// blades: `blades * min(1, (r0/d)^2)` down to `bmin`, then clumps keep `rank < keep` with
/// `keep = blades * (r0/d)^2 / bmin`. Mirrored by common.slang `grass_lod`.
pub fn lod(d: f32, blades: f32, r0: f32, bmin: f32) -> (f32, f32) {
    let k = (r0 / d.max(1e-3)).powi(2).min(1.0);
    let raw = blades * k;
    (raw.max(bmin.min(blades)), (raw / bmin.min(blades)).min(1.0))
}

/// The keep a node needs for clumps up to `d` m away (the type with most blades thins last).
pub fn clump_keep(d: f32, r0: f32) -> f32 {
    let most = TYPES.iter().map(|t| t.blades).max().unwrap_or(1) as f32;
    lod(d, most, r0, BMIN).1
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

/// Clumps of node `id` (level 14..=20) whose rank is under `keep_min`. `type_at(d)` is the grass
/// type and the ground's density (0..1) at body direction `d` (`None`: no grass). Clumps come in
/// lattice order.
pub fn scatter_node(id: NodeId, type_at: &dyn Fn(DVec3) -> Option<(u8, f32)>, keep_min: f32, seed: u64) -> Vec<Clump> {
    let f = Frame::new(id);
    let face = id.face();
    let (a0, a1, b0, b1, n) = cells(id, CLUMP_LEVEL, 0);
    let mut out = Vec::new();
    for j in b0..b1 {
        for i in a0..a1 {
            let key = cell_key(face, CLUMP_LEVEL, i as u64, j as u64) ^ seed.wrapping_mul(0x9E37_79B9_7F4A_7C15);
            let rank = crate::hash01(key, 7);
            if rank >= keep_min {
                continue;
            }
            let (d, key) = candidate(face, CLUMP_LEVEL, i, j, n, seed);
            let Some((kind, dens)) = type_at(d) else { continue };
            let Some(t) = TYPES.get(kind as usize) else { continue };
            if crate::hash01(key, 3) >= t.density * dens.clamp(0.0, 1.0) {
                continue;
            }
            // Row crops: on the cell centre, facing along the lattice j axis.
            let (dir, facing) = if t.aniso < 0.999 {
                let (i_, j_, n_) = (i as u64 * SUB + SUB / 2, j as u64 * SUB + SUB / 2, n as u64 * SUB);
                let c = lattice_point(face, i_, j_, n_);
                let c1 = lattice_point(face, i_, (j_ + SUB).min(n_), n_);
                let c0 = lattice_point(face, i_, j_.saturating_sub(SUB), n_);
                let a = c1 - c0;
                (c, (a - c * a.dot(c)).normalize())
            } else {
                let yaw = crate::hash01(key, 6) as f64 * TAU as f64;
                let e = f.e1 * yaw.cos() + f.e2 * yaw.sin();
                (d, (e - d * e.dot(d)).normalize())
            };
            let w = wrap_m(dir);
            let patch = fbm3(w[0] / 9.0, w[1] / 9.0, w[2] / 9.0, seed ^ 0x6A55, 3);
            let height = (0.55 + 0.9 * smoothstep(0.2, 0.8, patch)) * (0.88 + 0.24 * crate::hash01(key, 5));
            let dry = smoothstep(0.58, 0.8, fbm3(w[0] / 23.0, w[1] / 23.0, w[2] / 23.0, seed ^ 0xD27, 3));
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
    }
    out
}

/// Real ground: grass where the terrain material shows grass (G7's layer mirror), by climate:
/// long dry grass where it is arid, short turf high up, a meadow mix elsewhere.
pub fn type_from_ground(g: &GroundSample, cl: &Climate) -> Option<(u8, f32)> {
    let c = &g.canopy;
    let w = layer_weights(c.alt_m, g.slope_cos, c.cover);
    let grassy = (w[2] + w[5]) * (1.0 - smoothstep(0.1, 0.6, c.cover));
    if grassy < 0.15 {
        return None;
    }
    let kind = if cl.aridity < 0.35 {
        LONG
    } else if c.alt_m > 1900.0 {
        LAWN
    } else {
        MEADOW
    };
    Some((kind, smoothstep(0.15, 0.6, grassy)))
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

#[cfg(test)]
mod tests {
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
        assert_eq!(gpu_rows().len(), 4);
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
    fn rows_line_up() {
        let id = node();
        let w = scatter_node(id, &|_| Some((WHEAT, 1.0)), 1.0, 3);
        assert!(w.len() > 100);
        // Row clumps sit on cell centres and face along j: neighbours along the facing are
        // ~one cell apart and on the same line.
        let a = w[w.len() / 2];
        // The neighbour best aligned with the facing is about one cell away, on the row line.
        let next = w
            .iter()
            .filter(|c| c.dir != a.dir && c.dir.distance(a.dir) * EARTH_R < 1.0)
            .max_by(|x, y| (x.dir - a.dir).normalize().dot(a.facing).abs().total_cmp(&(y.dir - a.dir).normalize().dot(a.facing).abs()))
            .unwrap();
        let step = (next.dir - a.dir) * EARTH_R;
        assert!(step.length() > 0.35 && step.length() < 0.8, "{}", step.length());
        assert!(step.normalize().dot(a.facing).abs() > 0.995);
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
}
