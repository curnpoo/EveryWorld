//! Near-camera ground cover: grass tufts, wildflowers, pebbles, sculpted
//! rocks, moss cushions, scrub, ferns, twigs, snow lumps and sand ripples.
//!
//! *What* grows where comes from the same splat weights the terrain shader
//! paints with ([`SurfaceMap::splat`]), so a tuft only sprouts where the
//! ground under it is drawn as grass.
//!
//! A tile is cut into square chunks; the game asks for the chunks near the
//! camera and turns each into one merged mesh. Every instance carries a
//! pivot and a random *rank*; the vertex shader shrinks an instance to
//! nothing once `rank > fade(distance)`, so density thins smoothly with
//! distance and nothing pops. A chunk only contains instances that can be
//! visible from its nearest point (`cutoff`), so far chunks stay light.
//!
//! No primitives: every shape is generated (bent, tapered blades; chipped,
//! noise-displaced rocks; lumpy cushions), deterministic per seed.

use crate::biome::Biome;
use crate::foliage::Occupancy;
use crate::mesh::{MeshData, rgb};
use crate::noise::{fbm3, smoothstep};
use crate::project::TileProjector;
use crate::surface::{Cover, Ground, LAYERS, SurfaceMap};
use crate::terrain::TerrainGrid;
use glam::{Mat3, Quat, Vec3};
use std::f32::consts::TAU;
use std::sync::OnceLock;

/// Everything needed to scatter over one (finest-zoom) tile.
pub struct GroundSource {
    pub proj: TileProjector,
    pub grid: TerrainGrid,
    pub surface: SurfaceMap,
    pub occupancy: Option<Occupancy>,
}

impl std::fmt::Debug for GroundSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GroundSource").field("tile", &self.proj.tile).field("biome", &self.surface.biome).finish()
    }
}

/// Chunks per tile side. ~13-19 m chunks on z14 tiles.
pub const CHUNKS: usize = 128;
/// Nothing is scattered beyond this distance (meters).
pub const MAX_REACH: f32 = 100.0;
/// Reference reach of the fade curve.
const REF_REACH: f32 = 90.0;
const REF_NEAR: f32 = 16.0;

/// Fraction of instances (by rank) visible at distance `d` for a kind whose
/// reach is `reach`. Full density inside ~reach/5.6, then falling as 1/d^2
/// (constant on-screen density), then gone at `reach`. The WGSL twin of
/// this lives in `ground_scatter.wesl`; keep them in sync.
pub fn fade(d: f32, reach: f32) -> f32 {
    let x = d * REF_REACH / reach.max(1.0);
    let f = (REF_NEAR / x.max(REF_NEAR)).powi(2);
    f * (1.0 - smoothstep(0.75 * REF_REACH, REF_REACH, x))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Tuft,
    DryTuft,
    Flower,
    Rosette,
    Pebble,
    Rock,
    Moss,
    Scrub,
    Twig,
    Fern,
    SnowLump,
    Ripple,
}

impl Kind {
    pub const ALL: [Kind; 12] = [
        Kind::Tuft, Kind::DryTuft, Kind::Flower, Kind::Rosette, Kind::Pebble, Kind::Rock, Kind::Moss, Kind::Scrub,
        Kind::Twig, Kind::Fern, Kind::SnowLump, Kind::Ripple,
    ];

    /// Candidate spacing (m): one roll per cell.
    fn cell(self) -> f32 {
        match self {
            Kind::Tuft => 0.21,
            Kind::DryTuft => 0.42,
            Kind::Flower => 0.7,
            Kind::Rosette => 0.9,
            Kind::Pebble => 0.32,
            Kind::Rock => 1.6,
            Kind::Moss => 0.5,
            Kind::Scrub => 1.5,
            Kind::Twig => 1.1,
            Kind::Fern => 1.3,
            Kind::SnowLump => 1.3,
            Kind::Ripple => 1.4,
        }
    }

    /// Distance (m) at which the kind has fully faded out.
    pub fn reach(self) -> f32 {
        match self {
            Kind::Tuft | Kind::DryTuft => 80.0,
            Kind::Flower => 55.0,
            Kind::Rosette => 30.0,
            Kind::Pebble => 24.0,
            Kind::Rock => 100.0,
            Kind::Moss => 55.0,
            Kind::Scrub => 100.0,
            Kind::Twig => 32.0,
            Kind::Fern => 70.0,
            Kind::SnowLump => 70.0,
            Kind::Ripple => 45.0,
        }
    }

    /// How strongly each ground layer grows this kind (probability per cell
    /// at 100 % of that layer).
    fn affinity(self, biome: Biome) -> [f32; LAYERS] {
        let mut a = [0.0; LAYERS];
        let mut set = |g: Ground, v: f32| a[g as usize] = v;
        let dry = matches!(biome, Biome::Mediterranean | Biome::Savanna | Biome::HotDesert | Biome::Steppe);
        let wet = matches!(biome, Biome::TropicalRainforest | Biome::TemperateForest | Biome::Boreal);
        match self {
            Kind::Tuft => {
                set(Ground::Grass, 0.95);
                set(Ground::Moss, 0.2);
                set(Ground::ForestFloor, 0.04);
                set(Ground::Dirt, 0.12);
                set(Ground::Farmland, 0.06);
                set(Ground::Gravel, 0.06);
                set(Ground::Mud, 0.35);
                set(Ground::Paving, 0.03);
            }
            Kind::DryTuft => {
                set(Ground::DryGrass, 0.9);
                set(Ground::Sand, if dry { 0.05 } else { 0.08 });
                set(Ground::Dirt, 0.08);
                set(Ground::Gravel, 0.06);
            }
            Kind::Flower => {
                if biome != Biome::HotDesert {
                    set(Ground::Grass, if matches!(biome, Biome::Alpine | Biome::Grassland | Biome::Mediterranean) { 0.18 } else { 0.1 });
                    set(Ground::DryGrass, if biome == Biome::Mediterranean { 0.08 } else { 0.03 });
                }
            }
            Kind::Rosette => {
                set(Ground::Grass, 0.06);
                set(Ground::Dirt, 0.05);
                set(Ground::Paving, 0.025);
                set(Ground::Gravel, 0.03);
            }
            Kind::Pebble => {
                set(Ground::Gravel, 1.0);
                set(Ground::Rock, 0.6);
                set(Ground::Dirt, 0.3);
                set(Ground::Sand, if dry { 0.12 } else { 0.05 });
                set(Ground::Farmland, 0.12);
                set(Ground::DryGrass, 0.12);
                set(Ground::Mud, 0.1);
                set(Ground::Paving, 0.04);
                set(Ground::Grass, 0.02);
                set(Ground::ForestFloor, 0.04);
                set(Ground::Snow, 0.02);
            }
            Kind::Rock => {
                set(Ground::Rock, 0.55);
                set(Ground::Gravel, 0.22);
                set(Ground::Snow, 0.04);
                set(Ground::Dirt, 0.03);
                set(Ground::DryGrass, 0.04);
                set(Ground::Grass, 0.01);
                set(Ground::Sand, if biome == Biome::HotDesert { 0.025 } else { 0.0 });
                set(Ground::ForestFloor, 0.03);
                set(Ground::Moss, 0.04);
            }
            Kind::Moss => {
                set(Ground::Moss, 0.9);
                set(Ground::ForestFloor, if wet { 0.3 } else { 0.08 });
                set(Ground::Rock, if wet { 0.08 } else { 0.0 });
                set(Ground::Mud, 0.15);
            }
            Kind::Scrub => {
                set(Ground::DryGrass, if dry { 0.3 } else { 0.03 });
                set(Ground::Sand, if biome == Biome::HotDesert { 0.05 } else { 0.0 });
                set(Ground::Gravel, if dry { 0.06 } else { 0.0 });
                set(Ground::Dirt, if dry { 0.08 } else { 0.0 });
            }
            Kind::Twig => set(Ground::ForestFloor, 0.55),
            Kind::Fern => {
                if !dry {
                    set(Ground::ForestFloor, 0.5);
                    set(Ground::Moss, 0.2);
                }
            }
            Kind::SnowLump => set(Ground::Snow, 0.3),
            // Ripples live in the sand texture; geometry read as slivers.
            Kind::Ripple => {}
        }
        a
    }

    /// Lies on the terrain (follows its normal) rather than growing upright.
    fn hugs_ground(self) -> bool {
        matches!(self, Kind::Pebble | Kind::Rock | Kind::Moss | Kind::Twig | Kind::SnowLump | Kind::Ripple | Kind::Rosette)
    }
}

// ---------------------------------------------------------------------------
// Prototype shapes. uv.x = flex (0 rigid .. 1 sways fully), uv.y = height
// along the plant (0 root .. 1 tip). Colors are pre-tint shading (around 1).
// ---------------------------------------------------------------------------

struct Rng(u64);
impl Rng {
    fn f(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x9E37_79B9);
        crate::hash01(self.0, 77)
    }
    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f()
    }
}

fn gray(k: f32) -> [f32; 4] {
    [k, k, k, 1.0]
}

/// One curved, tapered blade (5 verts, 3 tris) appended to `m`.
#[allow(clippy::too_many_arguments)]
fn blade(m: &mut MeshData, root: Vec3, dir: f32, lean: f32, h: f32, w: f32, flex: f32, base: f32, tip: f32) {
    let fwd = Vec3::new(dir.cos(), 0.0, dir.sin());
    let side = Vec3::new(-dir.sin(), 0.0, dir.cos());
    let pt = |t: f32| root + Vec3::Y * (h * t * (1.0 - 0.35 * lean * t)) + fwd * (h * lean * t * t);
    // Normal: across the blade, bent toward the sky so tufts read soft.
    let n_at = |t: f32| {
        let tan = (pt((t + 0.05).min(1.0)) - pt((t - 0.05).max(0.0))).normalize_or_zero();
        let n = side.cross(tan).normalize_or_zero();
        let n = if n.y < 0.0 { -n } else { n };
        (n * 0.45 + Vec3::Y * 0.55).normalize().to_array()
    };
    let shade = |t: f32| {
        let k = base + (tip - base) * t;
        // Tips go a touch warmer.
        [k * (1.0 + 0.08 * t), k * (1.0 + 0.02 * t), k * (1.0 - 0.1 * t), 1.0]
    };
    let i0 = m.vertex((pt(0.0) - side * w * 0.5).to_array(), n_at(0.0), [flex, 0.0], shade(0.0));
    let i1 = m.vertex((pt(0.0) + side * w * 0.5).to_array(), n_at(0.0), [flex, 0.0], shade(0.0));
    let i2 = m.vertex((pt(0.5) - side * w * 0.36).to_array(), n_at(0.5), [flex, 0.5], shade(0.5));
    let i3 = m.vertex((pt(0.5) + side * w * 0.36).to_array(), n_at(0.5), [flex, 0.5], shade(0.5));
    let i4 = m.vertex(pt(1.0).to_array(), n_at(1.0), [flex, 1.0], shade(1.0));
    m.tri(i0, i2, i1);
    m.tri(i1, i2, i3);
    m.tri(i2, i4, i3);
}

fn tuft(seed: u64, blades: usize, h: (f32, f32), w: f32, spread: f32, lean: f32) -> MeshData {
    let mut r = Rng(seed);
    let mut m = MeshData::default();
    for _ in 0..blades {
        let a = r.f() * TAU;
        let rr = spread * r.f().sqrt();
        let root = Vec3::new(a.cos() * rr, 0.0, a.sin() * rr);
        let dir = a + r.range(-0.6, 0.6);
        let hh = r.range(h.0, h.1);
        blade(&mut m, root, dir, r.range(lean * 0.4, lean), hh, w * r.range(0.8, 1.2), 1.0, r.range(0.42, 0.6), r.range(1.0, 1.25));
    }
    m
}

/// Dry tuft with a few taller seed stalks.
fn dry_tuft(seed: u64) -> MeshData {
    let mut m = tuft(seed, 9, (0.22, 0.48), 0.006, 0.05, 0.55);
    let mut r = Rng(seed ^ 0xABCD);
    for _ in 0..3 {
        let a = r.f() * TAU;
        let root = Vec3::new(a.cos() * 0.02, 0.0, a.sin() * 0.02);
        let h = r.range(0.45, 0.7);
        blade(&mut m, root, a, r.range(0.1, 0.3), h, 0.003, 1.0, 0.7, 1.3);
        // Seed head: a slim spindle of tiny blades near the tip.
        let tip = root + Vec3::Y * h * 0.86;
        for k in 0..5 {
            let d = a + k as f32 * 1.3;
            blade(&mut m, tip + Vec3::Y * (k as f32 * 0.012), d, 0.6, 0.045, 0.006, 1.0, 1.15, 1.35);
        }
    }
    m
}

/// Flower: a bent stem, a few leaves and a head of cupped petals.
fn flower(seed: u64, petal: [f32; 4], center: [f32; 4], h: f32, petals: usize, petal_len: f32) -> MeshData {
    let mut r = Rng(seed);
    let mut m = MeshData::default();
    let dir = r.f() * TAU;
    let lean = r.range(0.05, 0.25);
    blade(&mut m, Vec3::ZERO, dir, lean, h, 0.004, 1.0, 0.5, 0.9);
    for _ in 0..3 {
        blade(&mut m, Vec3::ZERO, r.f() * TAU, 0.8, h * r.range(0.25, 0.45), 0.012, 0.8, 0.45, 0.85);
    }
    let fwd = Vec3::new(dir.cos(), 0.0, dir.sin());
    let head = Vec3::Y * (h * (1.0 - 0.35 * lean)) + fwd * (h * lean);
    let tilt = Quat::from_axis_angle(Vec3::new(-dir.sin(), 0.0, dir.cos()), r.range(0.1, 0.5));
    let up = tilt * Vec3::Y;
    let c = m.vertex(head.to_array(), up.to_array(), [1.0, 1.0], center);
    let ring: Vec<u32> = (0..petals)
        .flat_map(|k| {
            let a0 = k as f32 / petals as f32 * TAU;
            let half = TAU / petals as f32 * 0.42;
            [a0 - half, a0, a0 + half]
                .into_iter()
                .enumerate()
                .map(|(j, a)| {
                    let len = if j == 1 { petal_len } else { petal_len * 0.62 };
                    let lift = if j == 1 { 0.35 } else { 0.2 };
                    let p = head + tilt * Vec3::new(a.cos() * len, len * lift, a.sin() * len);
                    let n = (up * 0.8 + (p - head).normalize_or_zero() * 0.2).normalize();
                    (p, n)
                })
                .collect::<Vec<_>>()
        })
        .map(|(p, n)| m.vertex(p.to_array(), n.to_array(), [1.0, 1.0], petal))
        .collect();
    for k in 0..petals {
        let (a, b, d) = (ring[k * 3], ring[k * 3 + 1], ring[k * 3 + 2]);
        m.tri(c, b, a);
        m.tri(c, d, b);
    }
    // Small raised center disc.
    let cc = m.vertex((head + up * petal_len * 0.25).to_array(), up.to_array(), [1.0, 1.0], center);
    let disc: Vec<u32> = (0..6)
        .map(|k| {
            let a = k as f32 / 6.0 * TAU;
            let p = head + tilt * Vec3::new(a.cos() * petal_len * 0.28, petal_len * 0.12, a.sin() * petal_len * 0.28);
            m.vertex(p.to_array(), up.to_array(), [1.0, 1.0], center)
        })
        .collect();
    for k in 0..6 {
        m.tri(cc, disc[(k + 1) % 6], disc[k]);
    }
    m
}

/// Flat rosette of lance-shaped leaves (dandelion, plantain).
fn rosette(seed: u64) -> MeshData {
    let mut r = Rng(seed);
    let mut m = MeshData::default();
    let leaves = 6 + (r.f() * 4.0) as usize;
    for k in 0..leaves {
        let a = k as f32 / leaves as f32 * TAU + r.range(-0.2, 0.2);
        let len = r.range(0.06, 0.12);
        let (f, s) = (Vec3::new(a.cos(), 0.0, a.sin()), Vec3::new(-a.sin(), 0.0, a.cos()));
        let w = len * 0.28;
        let p = |t: f32, side: f32| f * len * t + s * side * w * (t * (1.0 - t) * 4.0).sqrt() + Vec3::Y * (0.012 + 0.03 * t * (1.0 - t));
        let shade = r.range(0.5, 0.7);
        let n = (Vec3::Y * 0.9 + f * 0.1).normalize().to_array();
        let v: Vec<u32> = [(0.0, 0.0), (0.35, -1.0), (0.35, 1.0), (0.7, -1.0), (0.7, 1.0), (1.0, 0.0)]
            .iter()
            .map(|&(t, sd)| m.vertex(p(t, sd).to_array(), n, [0.2, t], gray(shade * (0.85 + 0.3 * t))))
            .collect();
        m.tri(v[0], v[1], v[2]);
        m.tri(v[1], v[3], v[2]);
        m.tri(v[2], v[3], v[4]);
        m.tri(v[3], v[5], v[4]);
    }
    m
}

/// Icosphere (unit radius), `sub` subdivisions, shared vertices.
fn icosphere(sub: u32) -> (Vec<Vec3>, Vec<[u32; 3]>) {
    let t = (1.0 + 5f32.sqrt()) / 2.0;
    let mut v: Vec<Vec3> = [
        [-1.0, t, 0.0], [1.0, t, 0.0], [-1.0, -t, 0.0], [1.0, -t, 0.0],
        [0.0, -1.0, t], [0.0, 1.0, t], [0.0, -1.0, -t], [0.0, 1.0, -t],
        [t, 0.0, -1.0], [t, 0.0, 1.0], [-t, 0.0, -1.0], [-t, 0.0, 1.0],
    ]
    .iter()
    .map(|p| Vec3::from(*p).normalize())
    .collect();
    let mut f: Vec<[u32; 3]> = vec![
        [0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11], [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8],
        [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9], [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1],
    ];
    for _ in 0..sub {
        let mut cache = std::collections::HashMap::new();
        let mut mid = |a: u32, b: u32, v: &mut Vec<Vec3>| -> u32 {
            let key = (a.min(b), a.max(b));
            *cache.entry(key).or_insert_with(|| {
                v.push(((v[a as usize] + v[b as usize]) * 0.5).normalize());
                (v.len() - 1) as u32
            })
        };
        let mut nf = Vec::with_capacity(f.len() * 4);
        for [a, b, c] in f {
            let (ab, bc, ca) = (mid(a, b, &mut v), mid(b, c, &mut v), mid(c, a, &mut v));
            nf.extend_from_slice(&[[a, ab, ca], [b, bc, ab], [c, ca, bc], [ab, bc, ca]]);
        }
        f = nf;
    }
    (v, f)
}

/// Sculpted stone: an icosphere chipped by random planes (flat facets with
/// soft, bevel-like edges from smooth normals), noise-displaced, squashed
/// and flattened underneath. `lichen` > 0 paints green-grey on top faces.
fn stone(seed: u64, sub: u32, chips: usize, rough: f32, squash: f32, lichen: f32) -> MeshData {
    let mut r = Rng(seed);
    let (mut v, f) = icosphere(sub);
    let planes: Vec<(Vec3, f32)> = (0..chips)
        .map(|_| {
            let n = Vec3::new(r.range(-1.0, 1.0), r.range(-0.3, 1.0), r.range(-1.0, 1.0)).normalize_or(Vec3::Y);
            (n, r.range(0.55, 0.85))
        })
        .collect();
    let s = seed % 997;
    for p in &mut v {
        let mut q = *p;
        for (n, d) in &planes {
            let e = q.dot(*n) - d;
            if e > 0.0 {
                q -= *n * e;
            }
        }
        let k = fbm3(q.x * 2.3 + s as f32, q.y * 2.3, q.z * 2.3, seed, 4) - 0.5;
        q *= 1.0 + rough * k;
        q.y *= squash;
        // Flatten the base so it sits in the ground instead of balancing.
        if q.y < -0.25 * squash {
            q.y = -0.25 * squash + (q.y + 0.25 * squash) * 0.3;
        }
        *p = q;
    }
    let mut nrm = vec![Vec3::ZERO; v.len()];
    for t in &f {
        let (a, b, c) = (v[t[0] as usize], v[t[1] as usize], v[t[2] as usize]);
        let fnrm = (b - a).cross(c - a);
        for &i in t {
            nrm[i as usize] += fnrm;
        }
    }
    let mut m = MeshData::default();
    for (i, p) in v.iter().enumerate() {
        let n = nrm[i].normalize_or(Vec3::Y);
        // Mottled stone, darker underneath (contact AO), lichen up top.
        let mott = 0.85 + 0.3 * fbm3(p.x * 5.0, p.y * 5.0, p.z * 5.0, seed + 3, 3);
        let ao = 0.55 + 0.45 * smoothstep(-0.25 * squash, 0.35 * squash, p.y);
        let mut c = [mott * ao; 4];
        c[3] = 1.0;
        let li = lichen * smoothstep(0.35, 0.8, n.y) * smoothstep(0.5, 0.65, fbm3(p.x * 4.0, p.y * 4.0, p.z * 4.0, seed + 9, 3));
        c = [c[0] * (1.0 - li) + 0.62 * li, c[1] * (1.0 - li) + 0.75 * li, c[2] * (1.0 - li) + 0.45 * li, 1.0];
        m.vertex(p.to_array(), n.to_array(), [0.0, 0.0], c);
    }
    for t in &f {
        // Icosphere faces are CCW from outside.
        m.tri(t[0], t[1], t[2]);
    }
    m
}

/// Lumpy cushion (moss, snow): squashed displaced dome.
fn cushion(seed: u64, height: f32, lump: f32, top: [f32; 3], bottom: [f32; 3]) -> MeshData {
    let (mut v, f) = icosphere(2);
    for p in &mut v {
        let k = fbm3(p.x * 3.0, p.y * 3.0, p.z * 3.0, seed, 4) - 0.5;
        *p *= 1.0 + lump * k;
        p.y = if p.y < 0.0 { p.y * 0.08 } else { p.y * height };
    }
    let mut nrm = vec![Vec3::ZERO; v.len()];
    for t in &f {
        let (a, b, c) = (v[t[0] as usize], v[t[1] as usize], v[t[2] as usize]);
        for &i in t {
            nrm[i as usize] += (b - a).cross(c - a);
        }
    }
    let mut m = MeshData::default();
    for (i, p) in v.iter().enumerate() {
        let n = nrm[i].normalize_or(Vec3::Y);
        let t = smoothstep(0.0, height, p.y);
        let speck = 0.9 + 0.2 * fbm3(p.x * 14.0, p.y * 14.0, p.z * 14.0, seed + 1, 2);
        let c = [
            (bottom[0] + (top[0] - bottom[0]) * t) * speck,
            (bottom[1] + (top[1] - bottom[1]) * t) * speck,
            (bottom[2] + (top[2] - bottom[2]) * t) * speck,
            1.0,
        ];
        m.vertex(p.to_array(), (n * 0.7 + Vec3::Y * 0.3).normalize().to_array(), [0.0, t], c);
    }
    for t in &f {
        m.tri(t[0], t[1], t[2]);
    }
    m
}

/// A bent stick lying on the ground, with a side branch or two. Built as a
/// thin triangular prism along a noisy path.
fn twig(seed: u64, len: f32, radius: f32, lying: bool) -> MeshData {
    let mut r = Rng(seed);
    let mut m = MeshData::default();
    let branch = |m: &mut MeshData, start: Vec3, dir: Vec3, len: f32, rad: f32, r: &mut Rng| {
        let segs = 5;
        let mut pts = Vec::new();
        let mut p = start;
        let mut d = dir.normalize();
        for _ in 0..=segs {
            pts.push(p);
            d = (d + Vec3::new(r.range(-0.3, 0.3), if lying { 0.0 } else { r.range(-0.1, 0.25) }, r.range(-0.3, 0.3))).normalize();
            p += d * len / segs as f32;
        }
        let ring = |k: usize| -> Vec<Vec3> {
            let a = pts[k.min(segs)];
            let t = (pts[(k + 1).min(segs)] - pts[k.saturating_sub(1)]).normalize_or(Vec3::X);
            let s = t.cross(Vec3::Y).normalize_or(Vec3::Z);
            let u = s.cross(t);
            let rr = rad * (1.0 - 0.7 * k as f32 / segs as f32);
            (0..3).map(|j| a + (s * (j as f32 * TAU / 3.0).cos() + u * (j as f32 * TAU / 3.0).sin()) * rr).collect()
        };
        let base = m.positions.len() as u32;
        for k in 0..=segs {
            let a = pts[k];
            for p in ring(k) {
                let n = (p - a).normalize_or(Vec3::Y);
                let shade = 0.7 + 0.3 * (k as f32 / segs as f32);
                m.vertex(p.to_array(), n.to_array(), [if lying { 0.0 } else { 0.3 }, k as f32 / segs as f32], gray(shade));
            }
        }
        for k in 0..segs as u32 {
            for j in 0..3u32 {
                let (a, b) = (base + k * 3 + j, base + k * 3 + (j + 1) % 3);
                let (c, d) = (a + 3, b + 3);
                m.tri(a, b, d);
                m.tri(a, d, c);
            }
        }
        pts
    };
    let dir = Vec3::new(1.0, if lying { 0.0 } else { 1.5 }, 0.2);
    let start = if lying { Vec3::new(-len * 0.5, radius, 0.0) } else { Vec3::ZERO };
    let pts = branch(&mut m, start, dir, len, radius, &mut r);
    for k in [2usize, 3] {
        if r.f() < 0.7 {
            let side = Vec3::new(r.range(-0.5, 0.5), if lying { 0.0 } else { 0.6 }, r.range(0.6, 1.0) * if r.f() < 0.5 { -1.0 } else { 1.0 });
            branch(&mut m, pts[k], side, len * r.range(0.25, 0.45), radius * 0.6, &mut r);
        }
    }
    m
}

/// Dry scrub: woody sticks fanning out with clusters of small narrow leaves.
fn scrub(seed: u64, leafy: bool) -> MeshData {
    let mut r = Rng(seed);
    let mut m = MeshData::default();
    let stems = if leafy { 9 } else { 12 };
    let h = r.range(0.25, 0.55);
    for _ in 0..stems {
        let a = r.f() * TAU;
        let out = r.range(0.4, 1.1);
        let dir = Vec3::new(a.cos() * out, 1.0, a.sin() * out);
        let t = twig(r.0 ^ 0x51, h * r.range(0.7, 1.2), 0.006, false);
        let rot = Quat::from_rotation_arc(Vec3::new(1.0, 1.5, 0.2).normalize(), dir.normalize());
        let off = m.positions.len() as u32;
        for k in 0..t.positions.len() {
            m.vertex((rot * Vec3::from(t.positions[k])).to_array(), (rot * Vec3::from(t.normals[k])).to_array(), [0.25, t.uvs[k][1]], [0.42, 0.33, 0.24, 1.0]);
        }
        m.indices.extend(t.indices.iter().map(|i| i + off));
        // Leaves near the ends.
        let tip = rot * Vec3::from(t.positions[t.positions.len() - 2]);
        let leaves = if leafy { 14 } else { 4 };
        for _ in 0..leaves {
            let p = tip * r.range(0.55, 1.0) + Vec3::new(r.range(-0.03, 0.03), r.range(-0.02, 0.03), r.range(-0.03, 0.03));
            let col = if leafy && r.f() < 0.18 { 1.6 } else { r.range(0.55, 0.8) };
            blade(&mut m, p, r.f() * TAU, r.range(0.3, 0.9), r.range(0.03, 0.06), if leafy { 0.008 } else { 0.012 }, 0.5, col * 0.85, col);
        }
    }
    m
}

/// Fern: arching fronds with alternating leaflets.
fn fern(seed: u64) -> MeshData {
    let mut r = Rng(seed);
    let mut m = MeshData::default();
    let fronds = 5 + (r.f() * 4.0) as usize;
    for k in 0..fronds {
        let a = k as f32 / fronds as f32 * TAU + r.range(-0.3, 0.3);
        let len = r.range(0.35, 0.65);
        let rise = r.range(0.5, 0.9);
        let f = Vec3::new(a.cos(), 0.0, a.sin());
        let s = Vec3::new(-a.sin(), 0.0, a.cos());
        let path = |t: f32| f * len * t + Vec3::Y * len * rise * (t * (1.3 - t));
        let steps = 9;
        for i in 0..steps {
            let t = (i as f32 + 0.5) / steps as f32;
            let p0 = path(t);
            let tan = (path(t + 0.02) - path(t - 0.02)).normalize();
            let size = len * 0.22 * (1.0 - t * 0.85);
            for side in [-1.0f32, 1.0] {
                let tipd = (s * side * 0.85 + tan * 0.35 - Vec3::Y * 0.15).normalize();
                let base_w = tan * size * 0.22;
                let n = (tipd.cross(tan) * side).normalize_or(Vec3::Y);
                let n = (if n.y < 0.0 { -n } else { n } * 0.6 + Vec3::Y * 0.4).normalize().to_array();
                let shade = 0.55 + 0.35 * t;
                let a0 = m.vertex((p0 - base_w).to_array(), n, [0.7, t], gray(shade * 0.85));
                let a1 = m.vertex((p0 + base_w).to_array(), n, [0.7, t], gray(shade * 0.85));
                let a2 = m.vertex((p0 + tipd * size).to_array(), n, [0.7, t + 0.05], gray(shade * 1.1));
                m.tri(a0, a2, a1);
            }
        }
        // Midrib.
        blade(&mut m, Vec3::ZERO, a, 0.0, 0.02, 0.006, 0.7, 0.4, 0.5);
    }
    m
}

/// Sand ripple: a long, low, slightly curved ridge with a sharp crest.
fn ripple(seed: u64) -> MeshData {
    let mut r = Rng(seed);
    let mut m = MeshData::default();
    let len = r.range(0.8, 1.8);
    let bend = r.range(-0.15, 0.15);
    let segs = 12;
    let profile = [(-1.0f32, 0.0f32), (-0.45, 0.6), (0.0, 1.0), (0.25, 0.55), (0.5, 0.0)];
    let base = 0u32;
    for k in 0..=segs {
        let t = k as f32 / segs as f32 - 0.5;
        let hfall = 1.0 - (2.0 * t).powi(4);
        let c = Vec3::new(t * len, 0.0, bend * (1.0 - 4.0 * t * t));
        let (wd, ht) = (0.13 * hfall.max(0.15), 0.035 * hfall);
        for (pz, py) in profile {
            let p = c + Vec3::new(0.0, py * ht - 0.004, pz * wd);
            let shade = 0.88 + 0.16 * py;
            m.vertex(p.to_array(), [0.0, 1.0, 0.0], [0.0, py], gray(shade));
        }
    }
    let w = profile.len() as u32;
    for k in 0..segs as u32 {
        for j in 0..w - 1 {
            let a = base + k * w + j;
            let (b, c, d) = (a + 1, a + w, a + w + 1);
            m.tri(a, b, d);
            m.tri(a, d, c);
        }
    }
    // Recompute smooth normals.
    let mut nrm = vec![Vec3::ZERO; m.positions.len()];
    for t in m.indices.chunks_exact(3) {
        let (a, b, c) = (Vec3::from(m.positions[t[0] as usize]), Vec3::from(m.positions[t[1] as usize]), Vec3::from(m.positions[t[2] as usize]));
        let n = (b - a).cross(c - a);
        let n = if n.y < 0.0 { -n } else { n };
        for &i in t {
            nrm[i as usize] += n;
        }
    }
    for (i, n) in nrm.iter().enumerate() {
        m.normals[i] = n.normalize_or(Vec3::Y).to_array();
    }
    m
}

/// Variants per kind. Index with a hash.
pub struct Library {
    pub protos: Vec<Vec<MeshData>>,
    /// Lawn (short, dense) and alpine (short, stiff) grass variants.
    lawn: Vec<MeshData>,
    alpine: Vec<MeshData>,
    garrigue: Vec<MeshData>,
}

const FLOWER_COLORS: [u32; 8] = [0xf4f1e6, 0xf2d23c, 0x9a6bd1, 0x5b7fd8, 0xe58fb6, 0xd8402c, 0xf5a742, 0xffffff];

pub fn library() -> &'static Library {
    static LIB: OnceLock<Library> = OnceLock::new();
    LIB.get_or_init(|| {
        let v = |n: usize, f: &dyn Fn(u64) -> MeshData| (0..n).map(|i| f(1000 + i as u64 * 7919)).collect::<Vec<_>>();
        let mut protos = Vec::new();
        for k in Kind::ALL {
            protos.push(match k {
                Kind::Tuft => v(8, &|s| tuft(s, 9, (0.14, 0.36), 0.011, 0.06, 0.55)),
                Kind::DryTuft => v(5, &dry_tuft),
                Kind::Flower => (0..FLOWER_COLORS.len())
                    .map(|i| {
                        let s = 5000 + i as u64 * 31;
                        let (h, petals, plen) = match i {
                            1 => (0.22, 8, 0.016),  // buttercup-ish
                            2 | 3 => (0.3, 5, 0.012),
                            5 => (0.4, 4, 0.03),    // poppy
                            _ => (0.18, 9, 0.014), // daisy
                        };
                        let center = if i == 5 { rgb(0x1c1a14) } else { rgb(0xe8b828) };
                        flower(s, rgb(FLOWER_COLORS[i]), center, h, petals, plen)
                    })
                    .collect(),
                Kind::Rosette => v(4, &rosette),
                Kind::Pebble => v(8, &|s| stone(s, 1, 3, 0.5, 0.55, 0.0)),
                Kind::Rock => v(8, &|s| stone(s, 2, 5, 0.35, 0.6, 1.0)),
                Kind::Moss => v(5, &|s| cushion(s, 0.6, 1.3, [1.15, 1.15, 0.9], [0.55, 0.6, 0.45])),
                Kind::Scrub => v(5, &|s| scrub(s, false)),
                Kind::Twig => v(5, &|s| twig(s, 0.35, 0.008, true)),
                Kind::Fern => v(4, &fern),
                Kind::SnowLump => v(5, &|s| cushion(s, 0.3, 1.1, [1.0, 1.0, 1.0], [0.82, 0.86, 0.92])),
                Kind::Ripple => v(5, &ripple),
            });
        }
        Library {
            protos,
            lawn: v(5, &|s| tuft(s, 12, (0.05, 0.11), 0.008, 0.06, 0.35)),
            alpine: v(5, &|s| tuft(s, 9, (0.07, 0.16), 0.007, 0.04, 0.35)),
            garrigue: v(5, &|s| scrub(s, true)),
        }
    })
}

/// One chunk's merged mesh. `pivots[i]` = (instance root xyz in tile-local
/// space, reach_m.floor() + rank) for vertex i.
#[derive(Default)]
pub struct ScatterChunk {
    pub mesh: MeshData,
    pub pivots: Vec<[f32; 4]>,
    pub instances: usize,
}

impl GroundSource {
    fn chunk_uv(&self, ci: usize, cj: usize) -> (f64, f64, f64) {
        let s = 1.0 / CHUNKS as f64;
        (ci as f64 * s, cj as f64 * s, s)
    }

    /// Chunk center in tile-local render space (at ground height).
    pub fn chunk_center(&self, ci: usize, cj: usize) -> Vec3 {
        let (u0, v0, s) = self.chunk_uv(ci, cj);
        let (u, v) = (u0 + s * 0.5, v0 + s * 0.5);
        self.proj.local(u, v, self.grid.sample(u, v) as f64)
    }

    /// Chunk edge length (m).
    pub fn chunk_size_m(&self) -> f32 {
        (self.proj.tile.width_m() / CHUNKS as f64) as f32
    }

    /// Approximate (u, v) of a tile-local point, by projecting onto the
    /// tile's corner frame. Good to a few meters: only used to pick chunks.
    pub fn approx_uv(&self, p: Vec3) -> (f64, f64) {
        let o = self.proj.local(0.0, 0.0, 0.0);
        let eu = self.proj.local(1.0, 0.0, 0.0) - o;
        let ev = self.proj.local(0.0, 1.0, 0.0) - o;
        let d = p - o;
        ((d.dot(eu) / eu.length_squared()) as f64, (d.dot(ev) / ev.length_squared()) as f64)
    }

    /// Generate the instances of chunk (ci, cj) that may be visible from
    /// `nearest_m` away (the chunk's nearest point to the camera).
    pub fn build_chunk(&self, ci: usize, cj: usize, nearest_m: f32) -> ScatterChunk {
        let lib = library();
        let (u0, v0, s) = self.chunk_uv(ci, cj);
        let size_m = self.chunk_size_m();
        let surface = &self.surface;
        let biome = surface.biome;
        let tkey = (self.proj.tile.x as u64) << 32 | self.proj.tile.y as u64;
        // Corner frame for cheap, accurate placement inside the chunk.
        let (uc, vc) = (u0 + s * 0.5, v0 + s * 0.5);
        let c0 = self.proj.local(uc, vc, 0.0);
        let up = (self.proj.local(uc, vc, 1000.0) - c0).normalize();
        let du = (self.proj.local(uc + s * 0.5, vc, 0.0) - self.proj.local(uc - s * 0.5, vc, 0.0)) / s as f32;
        let dv = (self.proj.local(uc, vc + s * 0.5, 0.0) - self.proj.local(uc, vc - s * 0.5, 0.0)) / s as f32;
        let place = |u: f64, v: f64, h: f32| c0 + du * (u - uc) as f32 + dv * (v - vc) as f32 + up * h;
        // Ground normal from the height grid.
        let normal = |u: f64, v: f64| -> Vec3 {
            let e = s * 0.04;
            let hx = self.grid.sample(u + e, v) - self.grid.sample(u - e, v);
            let hz = self.grid.sample(u, v + e) - self.grid.sample(u, v - e);
            let tx = du * (2.0 * e) as f32 + up * hx;
            let tz = dv * (2.0 * e) as f32 + up * hz;
            let n = tz.cross(tx).normalize_or(up);
            if n.dot(up) < 0.0 { -n } else { n }
        };
        // Splat on a 5x5 lattice, bilinear inside.
        const L: usize = 5;
        let mut lattice = [[0.0f32; LAYERS]; L * L];
        for j in 0..L {
            for i in 0..L {
                let (u, v) = (u0 + s * i as f64 / (L - 1) as f64, v0 + s * j as f64 / (L - 1) as f64);
                let n = normal(u, v);
                let slope = 1.0 - n.dot(up).clamp(0.0, 1.0);
                lattice[j * L + i] = surface.splat(u, v, self.grid.sample(u, v), slope);
            }
        }
        let splat_at = |fu: f32, fv: f32| -> [f32; LAYERS] {
            let (x, y) = (fu * (L - 1) as f32, fv * (L - 1) as f32);
            let (i, j) = ((x as usize).min(L - 2), (y as usize).min(L - 2));
            let (tx, ty) = (x - i as f32, y - j as f32);
            let mut w = [0.0; LAYERS];
            for (k, o) in w.iter_mut().enumerate() {
                let a = lattice[j * L + i][k] * (1.0 - tx) + lattice[j * L + i + 1][k] * tx;
                let b = lattice[(j + 1) * L + i][k] * (1.0 - tx) + lattice[(j + 1) * L + i + 1][k] * tx;
                *o = a * (1.0 - ty) + b * ty;
            }
            w
        };
        let tint = surface.tint(uc, vc);
        let mut out = ScatterChunk::default();
        for (ki, kind) in Kind::ALL.iter().enumerate() {
            let reach = kind.reach();
            let cutoff = fade(nearest_m, reach);
            if cutoff <= 0.0 {
                continue;
            }
            let aff = kind.affinity(biome);
            if aff.iter().all(|&a| a == 0.0) {
                continue;
            }
            let cells = (size_m / kind.cell()).ceil().max(1.0) as usize;
            for cy in 0..cells {
                for cx in 0..cells {
                    let key = ((ci * CHUNKS + cj) as u64) << 24 ^ ((cy * cells + cx) as u64) << 4 ^ ki as u64;
                    let h = |n: u64| crate::hash01(tkey, key.wrapping_mul(31).wrapping_add(n));
                    let rank = h(0);
                    if rank >= cutoff {
                        continue;
                    }
                    let (fu, fv) = ((cx as f32 + h(1)) / cells as f32, (cy as f32 + h(2)) / cells as f32);
                    let w = splat_at(fu, fv);
                    let p: f32 = w.iter().zip(&aff).map(|(a, b)| a * b).sum();
                    if h(3) >= p {
                        continue;
                    }
                    let (u, v) = (u0 + s * fu as f64, v0 + s * fv as f64);
                    if self.occupancy.as_ref().is_some_and(|o| o.blocked(u, v)) {
                        continue;
                    }
                    let cover = surface.cover_at(u, v);
                    if cover == Cover::Water {
                        continue;
                    }
                    let gh = self.grid.sample(u, v);
                    if gh < 0.3 && cover != Cover::Natural {
                        continue;
                    }
                    let n = normal(u, v);
                    let slope = 1.0 - n.dot(up);
                    // Tall plants don't cling to cliffs; ripples need flat sand.
                    if (matches!(kind, Kind::Tuft | Kind::DryTuft | Kind::Flower | Kind::Fern | Kind::Scrub) && slope > 0.3)
                        || (*kind == Kind::Ripple && slope > 0.06)
                    {
                        continue;
                    }
                    let r = h(4);
                    let (mesh, scale, color) = self.pick(*kind, cover, &w, r, h(5), &tint, lib);
                    let yaw = h(6) * TAU;
                    let axis = if kind.hugs_ground() { (n * 0.8 + up * 0.2).normalize() } else { (up * 0.85 + n * 0.15).normalize() };
                    let rot = Quat::from_rotation_arc(Vec3::Y, axis) * Quat::from_rotation_y(yaw);
                    let sink = match kind {
                        Kind::Rock => 0.18 * scale.y,
                        Kind::Pebble => 0.12 * scale.y,
                        _ => 0.0,
                    };
                    let pivot = place(u, v, gh) - axis * sink;
                    bake(&mut out, mesh, pivot, rot, scale, color, reach.floor() + rank.min(0.999));
                    out.instances += 1;
                }
            }
        }
        out
    }

    /// Choose a prototype, its size and color for one instance.
    #[allow(clippy::too_many_arguments)]
    fn pick<'a>(&self, kind: Kind, cover: Cover, w: &[f32; LAYERS], r: f32, r2: f32, tint: &[f32; 4], lib: &'a Library) -> (&'a MeshData, Vec3, [f32; 3]) {
        let biome = self.surface.biome;
        let protos = &lib.protos[kind as usize];
        let pick = |v: &'a [MeshData]| &v[(r * v.len() as f32) as usize % v.len()];
        let veg = |c: u32, k: f32| {
            let c = rgb(c);
            [c[0] * tint[0] * k, c[1] * tint[1] * k, c[2] * tint[2] * k]
        };
        let u = |s: f32| Vec3::splat(s);
        match kind {
            Kind::Tuft => {
                let lawn = matches!(cover, Cover::Park | Cover::Pitch | Cover::Residential | Cover::Commercial);
                let alpine = matches!(biome, Biome::Alpine | Biome::Tundra);
                let col = veg(if r2 < 0.3 { 0x7aa646 } else if r2 < 0.85 { 0x6a983a } else { 0x8fae4e }, 1.0);
                if lawn {
                    (pick(&lib.lawn), u(0.9 + 0.4 * r2), col)
                } else if alpine {
                    (pick(&lib.alpine), u(0.8 + 0.5 * r2), col)
                } else {
                    let tall = 0.8 + 0.6 * r2 + 0.4 * w[Ground::Mud as usize];
                    (pick(protos), Vec3::new(1.0, tall, 1.0), col)
                }
            }
            Kind::DryTuft => (pick(protos), Vec3::new(1.0, 0.75 + 0.6 * r2, 1.0), veg(if r2 < 0.5 { 0xc2a865 } else { 0xb39858 }, 1.0)),
            Kind::Flower => {
                // Regional palettes: poppies & yellow in the Mediterranean,
                // white/blue/purple in the Alps, daisies & dandelions on lawns.
                let lawn = matches!(cover, Cover::Park | Cover::Residential);
                let idx = match biome {
                    Biome::Mediterranean => [5, 1, 5, 2, 7][(r * 5.0) as usize % 5],
                    Biome::Alpine | Biome::Tundra => [2, 3, 0, 1, 4][(r * 5.0) as usize % 5],
                    _ if lawn => [0, 1, 0, 7][(r * 4.0) as usize % 4],
                    _ => [0, 1, 2, 3, 4, 6][(r * 6.0) as usize % 6],
                };
                let cover_scale = if cover == Cover::Pitch { 0.0 } else { 1.0 };
                (&protos[idx], u((0.8 + 0.5 * r2) * cover_scale), [1.0, 1.0, 1.0])
            }
            Kind::Rosette => (pick(protos), u(0.8 + 0.6 * r2), veg(0x5f8a3a, 1.0)),
            Kind::Pebble => {
                let s = 0.018 + 0.05 * r2 * r2;
                (pick(protos), Vec3::new(s, s, s * (0.7 + 0.5 * r)), self.stone_color(r2))
            }
            Kind::Rock => {
                let s = 0.12 + 0.4 * r2 * r2 * r2 + 0.15 * w[Ground::Rock as usize];
                (pick(protos), Vec3::new(s, s * (0.8 + 0.4 * r), s * (0.75 + 0.5 * r2)), self.stone_color(r))
            }
            Kind::Moss => {
                let s = 0.05 + 0.1 * r2;
                (pick(protos), Vec3::new(s, s * 1.4, s * (0.7 + 0.5 * r)), veg(if r < 0.5 { 0x5f8030 } else { 0x7a9038 }, 1.0))
            }
            Kind::Scrub => {
                let leafy = biome == Biome::Mediterranean || (biome == Biome::Savanna && r2 > 0.5);
                let col = if leafy { veg(0x7d8c5c, 1.0) } else { veg(0x8a8a5a, 1.0) };
                (if leafy { pick(&lib.garrigue) } else { pick(protos) }, u(0.7 + 0.8 * r2), col)
            }
            Kind::Twig => (pick(protos), u(0.6 + 0.8 * r2), [0.30, 0.22, 0.15]),
            Kind::Fern => (pick(protos), u(0.7 + 0.6 * r2), veg(if biome == Biome::Boreal { 0x4f7a34 } else { 0x5a8f36 }, 1.0)),
            Kind::SnowLump => {
                let s = 0.12 + 0.25 * r2;
                (pick(protos), Vec3::new(s, s * 0.6, s * (0.6 + 0.6 * r)), [0.92, 0.94, 0.97])
            }
            Kind::Ripple => {
                let c = Ground::Sand.mean_color();
                (pick(protos), u(0.8 + 0.4 * r2), [c[0], c[1], c[2]])
            }
        }
    }

    fn stone_color(&self, r: f32) -> [f32; 3] {
        let hex = match self.surface.biome {
            Biome::HotDesert => [0xb07a52, 0xc49a6c, 0x9a6a48][(r * 3.0) as usize % 3],
            Biome::Savanna | Biome::Steppe => [0xa0704c, 0x8f7a64, 0xb58a60][(r * 3.0) as usize % 3],
            Biome::Mediterranean => [0xc9c1b0, 0xb0a594, 0xd8d0c0][(r * 3.0) as usize % 3],
            _ => [0x9b958c, 0x8a857c, 0xa8a196, 0x7f7a72][(r * 4.0) as usize % 4],
        };
        let c = rgb(hex);
        [c[0], c[1], c[2]]
    }
}

fn bake(out: &mut ScatterChunk, proto: &MeshData, pivot: Vec3, rot: Quat, scale: Vec3, color: [f32; 3], packed: f32) {
    if scale.min_element() <= 0.0 {
        return;
    }
    let base = out.mesh.positions.len() as u32;
    let m = Mat3::from_quat(rot);
    let inv_s = Vec3::ONE / scale;
    for k in 0..proto.positions.len() {
        let p = pivot + m * (Vec3::from(proto.positions[k]) * scale);
        let n = (m * (Vec3::from(proto.normals[k]) * inv_s)).normalize_or(Vec3::Y);
        let c = proto.colors[k];
        out.mesh.vertex(p.to_array(), n.to_array(), proto.uvs[k], [c[0] * color[0], c[1] * color[1], c[2] * color[2], 1.0]);
        out.pivots.push([pivot.x, pivot.y, pivot.z, packed]);
    }
    out.mesh.indices.extend(proto.indices.iter().map(|i| i + base));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::biome::Biome;
    use ed_geo::tile::TileId;
    use ed_geo::{EnuFrame, Geodetic};

    fn source(cover: Cover, biome: Biome, h: f32) -> GroundSource {
        let tile = TileId::containing(46.0, 7.7, 14);
        let proj = TileProjector::new(tile, EnuFrame::new(Geodetic::new(46.0, 7.7, 0.0)));
        let grid = TerrainGrid::flat(tile, 16, h);
        let mut surface = SurfaceMap::uniform(tile, biome);
        surface.cover = vec![cover];
        GroundSource { proj, grid, surface, occupancy: None }
    }

    #[test]
    fn fade_is_monotone_and_bounded() {
        let mut last = 1.0;
        for d in 0..120 {
            let f = fade(d as f32, 80.0);
            assert!(f <= last + 1e-6 && (0.0..=1.0).contains(&f));
            last = f;
        }
        assert_eq!(fade(0.0, 80.0), 1.0);
        assert_eq!(fade(81.0, 80.0), 0.0);
    }

    #[test]
    fn meadow_grows_grass_desert_grows_none() {
        let near = source(Cover::Grass, Biome::TemperateForest, 300.0).build_chunk(64, 64, 0.0);
        assert!(near.instances > 500, "meadow instances {}", near.instances);
        assert_eq!(near.pivots.len(), near.mesh.positions.len());
        let far = source(Cover::Grass, Biome::TemperateForest, 300.0).build_chunk(64, 64, 70.0);
        assert!(far.instances < near.instances / 5, "far {} near {}", far.instances, near.instances);
        let sand = source(Cover::Sand, Biome::HotDesert, 300.0).build_chunk(64, 64, 0.0);
        let lib = library();
        let tuft_verts = lib.protos[Kind::Tuft as usize][0].positions.len();
        assert!(sand.instances > 10);
        assert!(sand.mesh.positions.len() < near.mesh.positions.len() || tuft_verts == 0);
    }

    #[test]
    fn instances_sit_on_the_ground() {
        let src = source(Cover::Rock, Biome::Alpine, 2000.0);
        let c = src.build_chunk(10, 20, 0.0);
        assert!(c.instances > 20);
        for p in c.pivots.iter().step_by(97) {
            let (u, v) = src.approx_uv(Vec3::new(p[0], p[1], p[2]));
            let ground = src.proj.local(u, v, 2000.0);
            assert!((Vec3::new(p[0], p[1], p[2]) - ground).length() < 3.0);
        }
    }

    #[test]
    fn prototypes_are_not_degenerate() {
        for (k, protos) in library().protos.iter().enumerate() {
            for m in protos {
                assert!(m.triangle_count() >= 3, "{:?}", Kind::ALL[k]);
                assert!(m.indices.iter().all(|&i| (i as usize) < m.positions.len()));
            }
        }
    }
}

#[cfg(test)]
mod extent_tests {
    use super::*;
    use crate::biome::Biome;
    use ed_geo::tile::TileId;
    use ed_geo::{EnuFrame, Geodetic};

    #[test]
    fn vertices_stay_near_their_pivot() {
        for (cover, biome) in [(Cover::Grass, Biome::TemperateForest), (Cover::Natural, Biome::Mediterranean), (Cover::Wood, Biome::Boreal), (Cover::Sand, Biome::HotDesert), (Cover::Rock, Biome::Alpine)] {
            let tile = TileId::containing(46.0, 7.7, 15);
            let proj = TileProjector::new(tile, EnuFrame::new(Geodetic::new(46.0, 7.7, 0.0)));
            let grid = TerrainGrid::flat(tile, 16, 300.0);
            let mut surface = SurfaceMap::uniform(tile, biome);
            surface.cover = vec![cover];
            let src = GroundSource { proj, grid, surface, occupancy: None };
            let c = src.build_chunk(64, 64, 0.0);
            for (k, p) in c.mesh.positions.iter().enumerate() {
                let pv = c.pivots[k];
                let d = Vec3::from(*p).distance(Vec3::new(pv[0], pv[1], pv[2]));
                assert!(d < 2.5, "{cover:?}: vertex {k} is {d} m from pivot");
            }
        }
    }
}
