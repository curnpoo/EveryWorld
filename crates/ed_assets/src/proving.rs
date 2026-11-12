//! The proving-ground generator (M2 city, moved from `ew` in M3): 27 procedural parts with
//! LOD chains, the material table and the 100k-instance layout. Bake-time only: `ew` loads
//! the result from `proving.ewpack` and has no geometry generator of its own.
//!
//! - Parts are built in a y-up mesh space (metres), oriented CCW from outside, and get LOD
//!   chains from [`crate::lod::build_chain`] (error measured on the result).
//! - Layout is a pure function of a fixed seed (hash, no RNG state, no maps). Instances are
//!   written in the node's ENU frame: see [`to_enu`].
//! - Nothing is coplanar (props sit on the ground slab's top face with no shared faces), so
//!   depth ties do not depend on draw order and frames are byte-identical across runs.

use crate::geom::{Lod, MeshBuild};
use crate::lod::{LodReport, build_chain};
use crate::material;
use ed_geo::ObjectId;
use ed_pack::kinds::{Material, MaterialModel, Transform};
use glam::{Affine3A, DVec3, Quat, Vec2, Vec3};

/// Node origin of the proving ground in renderer world space (y-up; far from 0 on purpose).
pub const ORIGIN: DVec3 = DVec3::new(6_378_137.0, 0.0, -1_250_000.0);
pub const BLOCKS: i32 = 20;
pub const BLOCK_PITCH: f32 = 80.0;
pub const BLOCK_SIZE: f32 = 60.0;
pub const PER_BLOCK: usize = 250;
/// Tallest building, metres (the rooftop pass flies above it).
pub const MAX_BUILDING_H: f32 = 118.0;
/// The plaza block (sphere grid), by block index.
pub const PLAZA: (i32, i32) = (10, 10);
/// Proving-ground objects: `OBJECT_BASE + instance number`.
pub const OBJECT_BASE: u128 = 0x9000_0000;
/// Maximum LOD levels per part.
pub const MAX_LODS: usize = 4;

/// Asset path of part `i` (`proving/parts/<name>`), the ground slab and material `i`.
pub fn part_asset(name: &str) -> String {
    format!("proving/parts/{name}")
}
pub const GROUND_ASSET: &str = "proving/parts/ground";
/// Asset paths of every proving part and the ground (names only, for tools and the editor).
pub fn asset_names() -> Vec<String> {
    PARTS.iter().map(|d| part_asset(d.name)).chain([GROUND_ASSET.to_string()]).collect()
}
pub fn material_asset(i: usize) -> String {
    format!("proving/materials/{i:03}")
}

/// The y-up proving space -> ENU (x east, y north, z up) is a rotation of +90 degrees about X:
/// `(x, y, z) -> (x, -z, y)`. Translations permute exactly; rotations compose with it.
pub fn to_enu(a: &Affine3A) -> Transform {
    let (s, r, t) = a.to_scale_rotation_translation();
    Transform { translation: [t.x, -t.z, t.y], rotation: (enu_from_yup() * r).normalize().to_array(), scale: s.to_array() }
}
/// Inverse of [`to_enu`] (ENU node frame -> y-up renderer space).
pub fn from_enu(t: &Transform) -> Affine3A {
    let p = Vec3::from(t.translation);
    let r = (enu_from_yup().inverse() * Quat::from_array(t.rotation)).normalize();
    Affine3A::from_scale_rotation_translation(Vec3::from(t.scale), r, Vec3::new(p.x, p.z, -p.y))
}
pub fn enu_from_yup() -> Quat {
    Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)
}

// ---------------------------------------------------------------------------------------------
// Deterministic hash
// ---------------------------------------------------------------------------------------------
// ---------------------------------------------------------------------------------------------
// Deterministic hash
// ---------------------------------------------------------------------------------------------

fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Uniform in [0, 1) from (a, b, c).
fn h01(a: u64, b: u64, c: u64) -> f32 {
    (mix(a ^ mix(b ^ mix(c))) >> 40) as f32 / (1u64 << 24) as f32
}

/// Smooth 3D value noise in [-1, 1] (for blob displacement).
fn noise3(p: Vec3, seed: u64) -> f32 {
    let i = p.floor();
    let f = p - i;
    let s = f * f * (Vec3::splat(3.0) - 2.0 * f);
    let c = |dx: i32, dy: i32, dz: i32| {
        let k = |v: f32, d: i32| (v as i64 + d as i64) as u64;
        h01(k(i.x, dx), k(i.y, dy), k(i.z, dz) ^ seed) * 2.0 - 1.0
    };
    let l = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let x00 = l(c(0, 0, 0), c(1, 0, 0), s.x);
    let x10 = l(c(0, 1, 0), c(1, 1, 0), s.x);
    let x01 = l(c(0, 0, 1), c(1, 0, 1), s.x);
    let x11 = l(c(0, 1, 1), c(1, 1, 1), s.x);
    l(l(x00, x10, s.y), l(x01, x11, s.y), s.z)
}

// ---------------------------------------------------------------------------------------------
// Mesh building
// ---------------------------------------------------------------------------------------------

/// Triangle soup under construction (shared vertices where a surface is smooth).
#[derive(Default, Clone)]
pub struct Builder {
    pub pos: Vec<Vec3>,
    pub nrm: Vec<Vec3>,
    pub idx: Vec<u32>,
}

impl Builder {
    fn base(&self) -> u32 {
        self.pos.len() as u32
    }

    /// Surface of revolution around +Y: `profile` (radius, y) from bottom to top, `segs`
    /// around. A repeated profile point is a hard crease. Ends at r = 0 close the solid.
    pub fn lathe(&mut self, profile: &[(f32, f32)], segs: u32, xf: Affine3A) -> &mut Self {
        let n = profile.len();
        let seg_n = |a: (f32, f32), b: (f32, f32)| {
            let d = Vec2::new(b.0 - a.0, b.1 - a.1);
            Vec2::new(d.y, -d.x).normalize_or_zero()
        };
        let b0 = self.base();
        for (k, &(r, y)) in profile.iter().enumerate() {
            let prev = (k > 0 && profile[k - 1] != profile[k]).then(|| seg_n(profile[k - 1], profile[k]));
            let next = (k + 1 < n && profile[k + 1] != profile[k]).then(|| seg_n(profile[k], profile[k + 1]));
            // At a crease (the point repeats) use only the segment on this copy's side.
            let crease_after = k + 1 < n && profile[k + 1] == profile[k];
            let crease_before = k > 0 && profile[k - 1] == profile[k];
            let n2 = match (prev, next) {
                (Some(p), Some(q)) if !crease_after && !crease_before => (p + q).normalize_or_zero(),
                (Some(p), _) if crease_after || next.is_none() => p,
                (_, Some(q)) => q,
                (Some(p), None) => p,
                (None, None) => Vec2::Y,
            };
            for s in 0..=segs {
                let a = s as f32 / segs as f32 * std::f32::consts::TAU;
                let (sn, cs) = a.sin_cos();
                self.pos.push(xf.transform_point3(Vec3::new(r * cs, y, r * sn)));
                self.nrm.push(xf.transform_vector3(Vec3::new(n2.x * cs, n2.y, n2.x * sn)).normalize_or(Vec3::Y));
            }
        }
        let row = segs + 1;
        for k in 0..n as u32 - 1 {
            if profile[k as usize] == profile[k as usize + 1] {
                continue;
            }
            for s in 0..segs {
                let (a, b) = (b0 + k * row + s, b0 + k * row + s + 1);
                let (c, d) = (a + row, b + row);
                self.idx.extend_from_slice(&[a, b, d, a, d, c]);
            }
        }
        self
    }

    /// Displaced sphere: `rows` x `segs`, radius `r` times `1 + amp * noise(freq * dir)`.
    #[allow(clippy::too_many_arguments)]
    pub fn blob(&mut self, r: f32, rows: u32, segs: u32, amp: f32, freq: f32, seed: u64, xf: Affine3A) -> &mut Self {
        let b0 = self.base();
        for k in 0..=rows {
            let th = k as f32 / rows as f32 * std::f32::consts::PI;
            for s in 0..=segs {
                let ph = s as f32 / segs as f32 * std::f32::consts::TAU;
                let dir = Vec3::new(th.sin() * ph.cos(), -th.cos(), th.sin() * ph.sin());
                // Seam: sample the noise with the wrapped direction so the seam closes.
                let rr = r * (1.0 + amp * noise3(dir * freq + Vec3::splat(17.0), seed));
                self.pos.push(xf.transform_point3(dir * rr));
                self.nrm.push(Vec3::ZERO);
            }
        }
        let row = segs + 1;
        for k in 0..rows {
            for s in 0..segs {
                let (a, b) = (b0 + k * row + s, b0 + k * row + s + 1);
                let (c, d) = (a + row, b + row);
                self.idx.extend_from_slice(&[a, d, b, a, c, d]);
            }
        }
        self.smooth_normals(b0);
        self
    }

    /// Torus around +Y: ring radius `big`, tube radius `small`.
    pub fn torus(&mut self, big: f32, small: f32, segs: u32, sides: u32, xf: Affine3A) -> &mut Self {
        let b0 = self.base();
        for s in 0..=segs {
            let a = s as f32 / segs as f32 * std::f32::consts::TAU;
            let c = Vec3::new(a.cos(), 0.0, a.sin());
            for t in 0..=sides {
                let b = t as f32 / sides as f32 * std::f32::consts::TAU;
                let n = c * b.cos() + Vec3::Y * b.sin();
                self.pos.push(xf.transform_point3(c * big + n * small));
                self.nrm.push(xf.transform_vector3(n).normalize());
            }
        }
        let row = sides + 1;
        for s in 0..segs {
            for t in 0..sides {
                let (a, b) = (b0 + s * row + t, b0 + s * row + t + 1);
                let (c, d) = (a + row, b + row);
                self.idx.extend_from_slice(&[a, b, d, a, d, c]);
            }
        }
        self
    }

    /// Box from `lo` to `hi`, each face an `n` x `n` grid (flat normals).
    pub fn boxed(&mut self, lo: Vec3, hi: Vec3, n: u32, xf: Affine3A) -> &mut Self {
        let c = (lo + hi) * 0.5;
        let e = (hi - lo) * 0.5;
        for (nn, u, v) in [
            (Vec3::X, Vec3::Y, Vec3::Z),
            (Vec3::NEG_X, Vec3::Z, Vec3::Y),
            (Vec3::Y, Vec3::Z, Vec3::X),
            (Vec3::NEG_Y, Vec3::X, Vec3::Z),
            (Vec3::Z, Vec3::X, Vec3::Y),
            (Vec3::NEG_Z, Vec3::Y, Vec3::X),
        ] {
            let b0 = self.base();
            let nw = xf.transform_vector3(nn).normalize();
            for j in 0..=n {
                for i in 0..=n {
                    let (su, sv) = (i as f32 / n as f32 * 2.0 - 1.0, j as f32 / n as f32 * 2.0 - 1.0);
                    let p = c + (nn + u * su + v * sv) * e;
                    self.pos.push(xf.transform_point3(p));
                    self.nrm.push(nw);
                }
            }
            for j in 0..n {
                for i in 0..n {
                    let a = b0 + j * (n + 1) + i;
                    let (b, d, cc) = (a + 1, a + n + 2, a + n + 1);
                    self.idx.extend_from_slice(&[a, b, d, a, d, cc]);
                }
            }
        }
        self
    }

    /// Area-weighted vertex normals for vertices from `from` on.
    fn smooth_normals(&mut self, from: u32) {
        let first_tri = self.idx.iter().position(|&i| i >= from).unwrap_or(self.idx.len()) / 3 * 3;
        for t in self.idx[first_tri..].as_chunks::<3>().0 {
            let (a, b, c) = (self.pos[t[0] as usize], self.pos[t[1] as usize], self.pos[t[2] as usize]);
            let n = (b - a).cross(c - a);
            for &i in t {
                self.nrm[i as usize] += n;
            }
        }
        // Weld the seam column and the poles: same position, same normal.
        let mut order: Vec<u32> = (from..self.base()).collect();
        let key = |p: Vec3| (p * 1e4).round().as_ivec3().to_array();
        order.sort_by_key(|&i| key(self.pos[i as usize]));
        let mut k = 0;
        while k < order.len() {
            let mut e = k + 1;
            let kk = key(self.pos[order[k] as usize]);
            while e < order.len() && key(self.pos[order[e] as usize]) == kk {
                e += 1;
            }
            let sum: Vec3 = order[k..e].iter().map(|&i| self.nrm[i as usize]).sum();
            for &i in &order[k..e] {
                self.nrm[i as usize] = sum;
            }
            k = e;
        }
        for n in &mut self.nrm[from as usize..] {
            *n = n.normalize_or(Vec3::Y);
        }
    }

    /// Makes every triangle wind CCW seen from the side its vertex normals point to.
    fn orient(&mut self) {
        for t in self.idx.as_chunks_mut::<3>().0 {
            let (a, b, c) = (self.pos[t[0] as usize], self.pos[t[1] as usize], self.pos[t[2] as usize]);
            let g = (b - a).cross(c - a);
            let n = self.nrm[t[0] as usize] + self.nrm[t[1] as usize] + self.nrm[t[2] as usize];
            if g.dot(n) < 0.0 {
                t.swap(1, 2);
            }
        }
    }
}

/// Orients the part and builds its LOD chain (up to 4 levels, measured error).
pub fn build_part(b: &Builder) -> (MeshBuild, Vec<Lod>, Vec<LodReport>) {
    let mut b = b.clone();
    b.orient();
    let positions: Vec<[f32; 3]> = b.pos.iter().map(|p| p.to_array()).collect();
    let normals: Vec<[f32; 3]> = b.nrm.iter().map(|n| n.to_array()).collect();
    let (lods, report) = build_chain(&positions, &b.idx, MAX_LODS);
    (MeshBuild { positions, normals, uvs: Vec::new(), joints: Vec::new(), weights: Vec::new(), indices: b.idx }, lods, report)
}

/// Unit cube centred on the origin (the ground slab, scaled per instance).
pub fn cube() -> MeshBuild {
    let mut b = Builder::default();
    b.boxed(Vec3::splat(-0.5), Vec3::splat(0.5), 1, Affine3A::IDENTITY);
    b.orient();
    MeshBuild {
        positions: b.pos.iter().map(|p| p.to_array()).collect(),
        normals: b.nrm.iter().map(|n| n.to_array()).collect(),
        uvs: Vec::new(),
        joints: Vec::new(),
        weights: Vec::new(),
        indices: b.idx,
    }
}

// ---------------------------------------------------------------------------------------------
// Parts
// ---------------------------------------------------------------------------------------------

/// Where a part goes in a block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Building,
    Kerb,
    Yard,
}

pub struct PartDef {
    pub name: &'static str,
    pub slot: Slot,
    pub build: fn() -> Builder,
}

fn id() -> Affine3A {
    Affine3A::IDENTITY
}
fn at(x: f32, y: f32, z: f32) -> Affine3A {
    Affine3A::from_translation(Vec3::new(x, y, z))
}
fn cyl(r: f32, h: f32) -> Vec<(f32, f32)> {
    vec![(0.0, 0.0), (r, 0.0), (r, 0.0), (r, h), (r, h), (0.0, h)]
}

fn p_lamp() -> Builder {
    let mut b = Builder::default();
    b.lathe(&[(0.0, 0.0), (0.16, 0.0), (0.16, 0.0), (0.16, 0.4), (0.08, 0.6), (0.06, 5.6), (0.06, 5.6), (0.0, 5.6)], 24, id());
    b.torus(0.35, 0.05, 32, 10, at(0.0, 5.6, 0.0));
    b.blob(0.22, 12, 20, 0.0, 1.0, 0, at(0.0, 5.75, 0.0));
    b
}
fn p_bollard() -> Builder {
    let mut b = Builder::default();
    b.lathe(&[(0.0, 0.0), (0.14, 0.0), (0.14, 0.0), (0.14, 0.75), (0.12, 0.85), (0.07, 0.93), (0.0, 0.95)], 28, id());
    b
}
fn p_tree() -> Builder {
    let mut b = Builder::default();
    b.lathe(&[(0.0, 0.0), (0.35, 0.0), (0.35, 0.0), (0.28, 0.5), (0.2, 3.0), (0.2, 3.0), (0.0, 3.0)], 16, id());
    b.blob(2.2, 20, 32, 0.25, 1.6, 1, at(0.0, 4.2, 0.0));
    b.blob(1.5, 16, 24, 0.3, 2.0, 2, at(0.6, 5.6, 0.3));
    b
}
fn p_conifer() -> Builder {
    let mut b = Builder::default();
    b.lathe(&cyl(0.18, 1.2), 12, id());
    let mut prof = vec![(0.0, 1.0)];
    for k in 0..4 {
        let y = 1.0 + k as f32 * 1.4;
        let r = 1.8 - k as f32 * 0.38;
        prof.extend_from_slice(&[(r, y + 0.3), (r * 0.45, y + 1.6)]);
    }
    prof.push((0.0, 7.0));
    b.lathe(&prof, 28, id());
    b
}
fn p_bush() -> Builder {
    let mut b = Builder::default();
    b.blob(0.9, 18, 28, 0.35, 2.5, 3, at(0.0, 0.6, 0.0));
    b
}
fn p_rock() -> Builder {
    let mut b = Builder::default();
    b.blob(0.8, 20, 30, 0.4, 1.8, 4, Affine3A::from_scale_rotation_translation(Vec3::new(1.3, 0.7, 1.0), Quat::IDENTITY, Vec3::new(0.0, 0.35, 0.0)));
    b
}
fn p_bench() -> Builder {
    let mut b = Builder::default();
    b.boxed(Vec3::new(-0.9, 0.42, -0.22), Vec3::new(0.9, 0.48, 0.22), 6, id());
    b.boxed(Vec3::new(-0.9, 0.55, -0.27), Vec3::new(0.9, 0.95, -0.22), 6, id());
    for x in [-0.8f32, 0.8] {
        b.boxed(Vec3::new(x - 0.04, 0.0, -0.2), Vec3::new(x + 0.04, 0.42, 0.2), 3, id());
    }
    b
}
fn p_hydrant() -> Builder {
    let mut b = Builder::default();
    b.lathe(&[(0.0, 0.0), (0.2, 0.0), (0.2, 0.0), (0.2, 0.08), (0.14, 0.12), (0.14, 0.6), (0.18, 0.62), (0.15, 0.7), (0.06, 0.82), (0.0, 0.84)], 28, id());
    b.lathe(&cyl(0.06, 0.4), 16, Affine3A::from_rotation_translation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2), Vec3::new(0.2, 0.45, 0.0)));
    b
}
fn p_barrel() -> Builder {
    let mut b = Builder::default();
    let mut prof = vec![(0.0, 0.0), (0.28, 0.0), (0.28, 0.0)];
    for k in 0..=10 {
        let t = k as f32 / 10.0;
        prof.push((0.28 + 0.05 * (t * std::f32::consts::PI).sin(), t * 0.9));
    }
    prof.extend_from_slice(&[(0.28, 0.9), (0.0, 0.9)]);
    b.lathe(&prof, 32, id());
    b
}
fn p_crate() -> Builder {
    let mut b = Builder::default();
    b.boxed(Vec3::new(-0.5, 0.0, -0.5), Vec3::new(0.5, 1.0, 0.5), 8, id());
    b.boxed(Vec3::new(-0.3, 1.0, -0.3), Vec3::new(0.3, 1.5, 0.3), 6, Affine3A::from_rotation_y(0.4));
    b
}
fn p_cone() -> Builder {
    let mut b = Builder::default();
    b.lathe(&[(0.0, 0.0), (0.22, 0.0), (0.22, 0.0), (0.22, 0.04), (0.16, 0.04), (0.16, 0.04), (0.03, 0.7), (0.0, 0.71)], 32, id());
    b
}
fn p_kiosk() -> Builder {
    let mut b = Builder::default();
    b.lathe(&cyl(1.6, 2.6), 40, id());
    b.lathe(&[(0.0, 2.6), (2.1, 2.6), (2.1, 2.6), (2.1, 2.75), (1.2, 3.4), (0.15, 3.9), (0.0, 4.0)], 40, id());
    b
}
fn p_column() -> Builder {
    let mut b = Builder::default();
    let mut prof = vec![(0.0, 0.0), (0.6, 0.0), (0.6, 0.0), (0.6, 0.3), (0.6, 0.3), (0.42, 0.3), (0.42, 0.3)];
    for k in 0..=12 {
        let t = k as f32 / 12.0;
        prof.push((0.42 - 0.06 * t, 0.3 + t * 5.4));
    }
    prof.extend_from_slice(&[(0.55, 5.9), (0.55, 6.1), (0.55, 6.1), (0.0, 6.1)]);
    b.lathe(&prof, 36, id());
    b
}
fn p_planter() -> Builder {
    let mut b = Builder::default();
    b.boxed(Vec3::new(-0.7, 0.0, -0.7), Vec3::new(0.7, 0.6, 0.7), 5, id());
    b.blob(0.8, 14, 24, 0.3, 2.4, 5, at(0.0, 0.9, 0.0));
    b
}
fn p_mushroom() -> Builder {
    let mut b = Builder::default();
    b.lathe(&[(0.0, 0.0), (0.25, 0.0), (0.25, 0.0), (0.18, 0.6), (0.18, 0.6), (0.9, 0.62), (0.75, 0.9), (0.4, 1.1), (0.0, 1.15)], 36, id());
    b
}
fn p_vase() -> Builder {
    let mut b = Builder::default();
    let mut prof = vec![(0.0, 0.0), (0.3, 0.0), (0.3, 0.0)];
    for k in 0..=16 {
        let t = k as f32 / 16.0;
        prof.push((0.3 + 0.25 * (t * 3.0).sin() * (1.0 - t) + 0.1 * t, t * 1.4));
    }
    prof.push((0.0, 1.4));
    b.lathe(&prof, 36, id());
    b
}
fn p_ring() -> Builder {
    let mut b = Builder::default();
    b.torus(1.2, 0.18, 48, 16, Affine3A::from_rotation_translation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2), Vec3::new(0.0, 1.45, 0.0)));
    b.boxed(Vec3::new(-0.5, 0.0, -0.3), Vec3::new(0.5, 0.15, 0.3), 3, id());
    b
}
fn p_dome() -> Builder {
    let mut b = Builder::default();
    let mut prof = vec![(0.0, 0.0), (2.2, 0.0), (2.2, 0.0), (2.2, 1.5)];
    for k in 0..=12 {
        let a = k as f32 / 12.0 * std::f32::consts::FRAC_PI_2;
        prof.push((2.2 * a.cos(), 1.5 + 2.2 * a.sin()));
    }
    b.lathe(&prof, 40, id());
    b
}
fn p_gear() -> Builder {
    let mut b = Builder::default();
    // Toothed disc: the radius varies around via a per-ring warp after the lathe.
    b.lathe(&cyl(1.0, 0.3), 96, id());
    for p in &mut b.pos {
        let a = p.z.atan2(p.x);
        let r = Vec2::new(p.x, p.z).length();
        if r > 0.5 {
            let k = if (a * 12.0 / std::f32::consts::PI).rem_euclid(2.0) < 1.0 { 1.15 } else { 1.0 };
            p.x *= k;
            p.z *= k;
        }
    }
    let rot = Affine3A::from_rotation_translation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2), Vec3::new(0.0, 1.2, 0.0));
    for p in &mut b.pos {
        *p = rot.transform_point3(*p);
    }
    for n in &mut b.nrm {
        *n = rot.transform_vector3(*n);
    }
    b.boxed(Vec3::new(-0.15, 0.0, -0.15), Vec3::new(0.15, 1.2, 0.15), 2, id());
    b
}
fn p_capsule() -> Builder {
    let mut b = Builder::default();
    let mut prof = Vec::new();
    for k in 0..=8 {
        let a = -std::f32::consts::FRAC_PI_2 + k as f32 / 8.0 * std::f32::consts::FRAC_PI_2;
        prof.push((0.4 * a.cos(), 0.4 + 0.4 * a.sin()));
    }
    for k in 0..=8 {
        let a = k as f32 / 8.0 * std::f32::consts::FRAC_PI_2;
        prof.push((0.4 * a.cos(), 1.6 + 0.4 * a.sin()));
    }
    b.lathe(&prof, 32, id());
    b
}
fn p_statue() -> Builder {
    let mut b = Builder::default();
    b.boxed(Vec3::new(-0.8, 0.0, -0.8), Vec3::new(0.8, 1.2, 0.8), 6, id());
    b.blob(0.7, 24, 36, 0.35, 2.2, 6, Affine3A::from_scale_rotation_translation(Vec3::new(0.8, 1.6, 0.8), Quat::IDENTITY, Vec3::new(0.0, 2.3, 0.0)));
    b
}
fn p_sign() -> Builder {
    let mut b = Builder::default();
    b.lathe(&cyl(0.05, 2.6), 16, id());
    b.lathe(&[(0.0, -0.02), (0.4, -0.02), (0.4, -0.02), (0.4, 0.02), (0.4, 0.02), (0.0, 0.02)], 8, Affine3A::from_rotation_translation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2), Vec3::new(0.0, 2.3, 0.07)));
    b
}
fn p_spheres() -> Builder {
    let mut b = Builder::default();
    b.blob(0.5, 24, 40, 0.0, 1.0, 0, at(0.0, 0.5, 0.0));
    b
}
fn building(levels: u32, crown: bool) -> Builder {
    let mut b = Builder::default();
    // Unit footprint, height 1, floors as stacked slabs with recessed bands (relief that
    // the simplifier can remove at distance); scaled per instance.
    let floors = levels;
    for f in 0..floors {
        let y0 = f as f32 / floors as f32;
        let y1 = (f as f32 + 0.82) / floors as f32;
        let y2 = (f + 1) as f32 / floors as f32;
        b.boxed(Vec3::new(-0.5, y0, -0.5), Vec3::new(0.5, y1, 0.5), 2, id());
        b.boxed(Vec3::new(-0.485, y1, -0.485), Vec3::new(0.485, y2, 0.485), 2, id());
    }
    if crown {
        b.boxed(Vec3::new(-0.3, 1.0, -0.3), Vec3::new(0.3, 1.04, 0.3), 2, id());
    }
    b
}
fn p_tower() -> Builder {
    building(24, true)
}
fn p_block() -> Builder {
    building(10, false)
}
fn p_slab() -> Builder {
    building(16, true)
}
fn p_hut() -> Builder {
    let mut b = Builder::default();
    b.boxed(Vec3::new(-1.5, 0.0, -1.2), Vec3::new(1.5, 2.4, 1.2), 4, id());
    b.lathe(&[(0.0, 2.4), (2.3, 2.4), (2.3, 2.4), (0.0, 3.8)], 4, Affine3A::from_rotation_y(std::f32::consts::FRAC_PI_4));
    b
}

/// The part table (order is the mesh order; deterministic).
pub const PARTS: &[PartDef] = &[
    PartDef { name: "tower", slot: Slot::Building, build: p_tower },
    PartDef { name: "block", slot: Slot::Building, build: p_block },
    PartDef { name: "slab", slot: Slot::Building, build: p_slab },
    PartDef { name: "lamp", slot: Slot::Kerb, build: p_lamp },
    PartDef { name: "bollard", slot: Slot::Kerb, build: p_bollard },
    PartDef { name: "tree", slot: Slot::Kerb, build: p_tree },
    PartDef { name: "conifer", slot: Slot::Kerb, build: p_conifer },
    PartDef { name: "hydrant", slot: Slot::Kerb, build: p_hydrant },
    PartDef { name: "bench", slot: Slot::Kerb, build: p_bench },
    PartDef { name: "sign", slot: Slot::Kerb, build: p_sign },
    PartDef { name: "cone", slot: Slot::Kerb, build: p_cone },
    PartDef { name: "planter", slot: Slot::Kerb, build: p_planter },
    PartDef { name: "bush", slot: Slot::Yard, build: p_bush },
    PartDef { name: "rock", slot: Slot::Yard, build: p_rock },
    PartDef { name: "barrel", slot: Slot::Yard, build: p_barrel },
    PartDef { name: "crate", slot: Slot::Yard, build: p_crate },
    PartDef { name: "kiosk", slot: Slot::Yard, build: p_kiosk },
    PartDef { name: "column", slot: Slot::Yard, build: p_column },
    PartDef { name: "mushroom", slot: Slot::Yard, build: p_mushroom },
    PartDef { name: "vase", slot: Slot::Yard, build: p_vase },
    PartDef { name: "ring", slot: Slot::Yard, build: p_ring },
    PartDef { name: "dome", slot: Slot::Yard, build: p_dome },
    PartDef { name: "gear", slot: Slot::Yard, build: p_gear },
    PartDef { name: "capsule", slot: Slot::Yard, build: p_capsule },
    PartDef { name: "statue", slot: Slot::Yard, build: p_statue },
    PartDef { name: "hut", slot: Slot::Yard, build: p_hut },
    PartDef { name: "sphere", slot: Slot::Yard, build: p_spheres },
];

// ---------------------------------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------------------------------

/// One placed instance (relative to [`ORIGIN`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub part: usize,
    pub material: usize,
    pub transform: Affine3A,
}

/// Material table: index 0 ground, 1..=49 the sphere grid, then the palette.
pub fn materials() -> Vec<Material> {
    let mut ground = material::simple(MaterialModel::Pbr, [0.16, 0.17, 0.15], 0.0, 0.85);
    ground.params.insert(material::P_CHECKER.into(), ed_pack::kinds::Param::Float(1.0));
    let mut m = vec![ground];
    for row in 0..7 {
        for col in 0..7 {
            m.push(material::simple(MaterialModel::Pbr, [0.85, 0.32, 0.18], row as f32 / 6.0, (0.05 + col as f32 / 6.0 * 0.95).min(1.0)));
        }
    }
    let pal = [
        Vec3::new(0.62, 0.58, 0.52),
        Vec3::new(0.48, 0.32, 0.24),
        Vec3::new(0.70, 0.66, 0.60),
        Vec3::new(0.28, 0.33, 0.40),
        Vec3::new(0.75, 0.55, 0.35),
        Vec3::new(0.20, 0.45, 0.20),
        Vec3::new(0.55, 0.15, 0.12),
        Vec3::new(0.85, 0.80, 0.55),
    ];
    for (i, c) in pal.iter().enumerate() {
        m.push(material::simple(MaterialModel::Pbr, c.to_array(), if i == 3 { 0.6 } else { 0.0 }, 0.35 + 0.08 * i as f32));
        m.push(material::toon((*c * 1.1).to_array(), (*c * 0.45).to_array(), 3.0, 0.35));
    }
    m
}
/// First palette material (PBR at `PAL + 2k`, toon at `PAL + 2k + 1`).
pub const PAL: usize = 50;
pub const PAL_COLOURS: usize = 8;

fn part_index(name: &str) -> usize {
    PARTS.iter().position(|p| p.name == name).expect("part")
}

/// Every instance of the proving ground, in a fixed order. Exactly `BLOCKS^2 * PER_BLOCK`
/// city instances, then the ground slab and the 49 spheres of the plaza grid.
pub fn layout() -> Vec<Placed> {
    let kerb: Vec<usize> = (0..PARTS.len()).filter(|&i| PARTS[i].slot == Slot::Kerb).collect();
    let yard: Vec<usize> = (0..PARTS.len()).filter(|&i| PARTS[i].slot == Slot::Yard).collect();
    let bld: Vec<usize> = (0..PARTS.len()).filter(|&i| PARTS[i].slot == Slot::Building).collect();
    let half = BLOCKS as f32 * BLOCK_PITCH * 0.5;
    let mut out = Vec::with_capacity((BLOCKS * BLOCKS) as usize * PER_BLOCK + 50);
    for bz in 0..BLOCKS {
        for bx in 0..BLOCKS {
            let (x0, z0) = (-half + bx as f32 * BLOCK_PITCH + 10.0, -half + bz as f32 * BLOCK_PITCH + 10.0);
            let seed = (bx as u64) << 32 | bz as u64;
            let start = out.len();
            let plaza = (bx, bz) == PLAZA;
            // Buildings: 3 per side on the block edge (12), tall and toon or PBR by block.
            let toon_block = h01(seed, 1, 0) < 0.4;
            if !plaza {
                for side in 0..4u64 {
                    for k in 0..3u64 {
                        let r = |s: u64| h01(seed, 10 + side * 3 + k, s);
                        let w = 15.0 + 3.0 * r(1);
                        let d = 12.0 + 4.0 * r(2);
                        let tall = r(3);
                        let hgt = if tall > 0.85 { 70.0 + 48.0 * r(4) } else { 14.0 + 36.0 * tall };
                        let along = 4.0 + k as f32 * 18.0 + w * 0.5;
                        let (cx, cz, rot) = match side {
                            0 => (x0 + along, z0 + d * 0.5 + 1.0, false),
                            1 => (x0 + BLOCK_SIZE - d * 0.5 - 1.0, z0 + along, true),
                            2 => (x0 + BLOCK_SIZE - along, z0 + BLOCK_SIZE - d * 0.5 - 1.0, false),
                            _ => (x0 + d * 0.5 + 1.0, z0 + BLOCK_SIZE - along, true),
                        };
                        let (sx, sz) = if rot { (d, w) } else { (w, d) };
                        let part = bld[(r(5) * bld.len() as f32) as usize % bld.len()];
                        let colour = (r(6) * PAL_COLOURS as f32) as usize % PAL_COLOURS;
                        let toon = toon_block ^ (r(7) < 0.15);
                        out.push(Placed {
                            part,
                            material: PAL + colour * 2 + toon as usize,
                            transform: Affine3A::from_scale_rotation_translation(Vec3::new(sx, hgt, sz), Quat::IDENTITY, Vec3::new(cx, 0.0, cz)),
                        });
                    }
                }
            }
            // Kerb props: around the block, 0.8 m in from its edge (the sidewalk).
            let n_kerb = if plaza { 120 } else { 100 };
            for k in 0..n_kerb as u64 {
                let t = (k as f32 + 0.5 * h01(seed, 200, k)) / n_kerb as f32 * 4.0;
                let side = t as u32 % 4;
                let f = t.fract() * (BLOCK_SIZE - 2.0) + 1.0;
                let inset = -1.2;
                let (px, pz) = match side {
                    0 => (x0 + f, z0 + inset),
                    1 => (x0 + BLOCK_SIZE - inset, z0 + f),
                    2 => (x0 + BLOCK_SIZE - f, z0 + BLOCK_SIZE - inset),
                    _ => (x0 + inset, z0 + BLOCK_SIZE - f),
                };
                let part = kerb[(h01(seed, 300, k) * kerb.len() as f32) as usize % kerb.len()];
                let s = 0.85 + 0.3 * h01(seed, 400, k);
                let yaw = h01(seed, 500, k) * std::f32::consts::TAU;
                let colour = (h01(seed, 600, k) * PAL_COLOURS as f32) as usize % PAL_COLOURS;
                let toon = PARTS[part].name.contains("tree") || h01(seed, 700, k) < 0.3;
                out.push(Placed {
                    part,
                    material: PAL + colour * 2 + toon as usize,
                    transform: Affine3A::from_scale_rotation_translation(Vec3::splat(s), Quat::from_rotation_y(yaw), Vec3::new(px, 0.0, pz)),
                });
            }
            // Courtyard clutter on a jittered grid inside the buildings ring (hidden from
            // the street by the buildings: the occlusion test's main customer).
            let n_yard = PER_BLOCK - (out.len() - start);
            let (inner0, inner1) = if plaza { (4.0, BLOCK_SIZE - 4.0) } else { (19.0, BLOCK_SIZE - 19.0) };
            let cols = (n_yard as f32).sqrt().ceil() as usize;
            for k in 0..n_yard {
                let (i, j) = (k % cols, k / cols);
                let cell = (inner1 - inner0) / cols as f32;
                let kk = k as u64;
                let px = x0 + inner0 + (i as f32 + 0.2 + 0.6 * h01(seed, 800, kk)) * cell;
                let pz = z0 + inner0 + (j as f32 + 0.2 + 0.6 * h01(seed, 900, kk)) * cell;
                if plaza && (px - x0 - BLOCK_SIZE * 0.5).abs() < 14.0 && (pz - z0 - BLOCK_SIZE * 0.5).abs() < 14.0 {
                    // Keep the sphere grid's square clear: push the prop to the plaza rim.
                    let (dx, dz) = (px - x0 - BLOCK_SIZE * 0.5, pz - z0 - BLOCK_SIZE * 0.5);
                    let m = dx.abs().max(dz.abs()).max(0.1);
                    let k2 = 16.0 / m;
                    let p = Vec3::new(x0 + BLOCK_SIZE * 0.5 + dx * k2, 0.0, z0 + BLOCK_SIZE * 0.5 + dz * k2);
                    out.push(yard_prop(&yard, seed, kk, p));
                } else {
                    out.push(yard_prop(&yard, seed, kk, Vec3::new(px, 0.0, pz)));
                }
            }
            debug_assert_eq!(out.len() - start, PER_BLOCK);
        }
    }
    // Ground slab: top face at y = 0, under everything (no prop face lies on it: every
    // part's lowest face is its base, and the slab's top is 1 mm under it).
    out.push(Placed {
        part: usize::MAX,
        material: 0,
        transform: Affine3A::from_scale_rotation_translation(Vec3::new(4000.0, 2.0, 4000.0), Quat::IDENTITY, Vec3::new(0.0, -1.001, 0.0)),
    });
    // The 7 x 7 sphere grid in the plaza (1.6 m spheres, 3 m apart).
    let pc = Vec3::new(-half + PLAZA.0 as f32 * BLOCK_PITCH + 10.0 + BLOCK_SIZE * 0.5, 0.0, -half + PLAZA.1 as f32 * BLOCK_PITCH + 10.0 + BLOCK_SIZE * 0.5);
    let sphere = part_index("sphere");
    for row in 0..7 {
        for col in 0..7 {
            let p = pc + Vec3::new((col as f32 - 3.0) * 3.0, 0.5, (row as f32 - 3.0) * 3.0);
            out.push(Placed { part: sphere, material: 1 + row * 7 + col, transform: Affine3A::from_scale_rotation_translation(Vec3::splat(1.6), Quat::IDENTITY, p) });
        }
    }
    out
}

fn yard_prop(yard: &[usize], seed: u64, k: u64, p: Vec3) -> Placed {
    let part = yard[(h01(seed, 1000, k) * yard.len() as f32) as usize % yard.len()];
    let s = 0.7 + 0.8 * h01(seed, 1100, k);
    let yaw = h01(seed, 1200, k) * std::f32::consts::TAU;
    let colour = (h01(seed, 1300, k) * PAL_COLOURS as f32) as usize % PAL_COLOURS;
    let toon = h01(seed, 1400, k) < 0.45;
    Placed { part, material: PAL + colour * 2 + toon as usize, transform: Affine3A::from_scale_rotation_translation(Vec3::splat(s), Quat::from_rotation_y(yaw), p) }
}

/// Object id of layout instance `i`.
pub fn object(i: usize) -> ObjectId {
    ObjectId(OBJECT_BASE + i as u128)
}

/// Height (part-local metres) below which a thin prop's collider is cut: trees, conifers,
/// lamps and signs collide with their trunk or post only, so you can walk under the canopy.
pub const TRUNK_CUT: f32 = 2.2;

/// A part's collision shapes in its own (y-up, unit or metre) frame: a box around the mesh,
/// a sphere for the sphere part, and only the lower [`TRUNK_CUT`] metres for tall thin props.
pub fn part_collision(name: &str, positions: &[[f32; 3]]) -> Vec<ed_pack::kinds::CollisionShape> {
    use crate::collision::{aabb_box, bounds, surface};
    let pts = positions.iter().map(|p| Vec3::from(*p));
    let Some((lo, hi)) = bounds(pts) else { return Vec::new() };
    let slot = PARTS.iter().find(|d| d.name == name).map(|d| d.slot);
    let surf = if slot == Some(Slot::Building) { surface::BUILDING } else { surface::PROP };
    match name {
        "sphere" => {
            let c = (lo + hi) * 0.5;
            let r = (hi - lo).max_element() * 0.5;
            vec![ed_pack::kinds::CollisionShape { shape: ed_pack::kinds::Shape::Sphere { centre: c.to_array(), radius: r }, surface: surf }]
        }
        "tree" | "conifer" | "lamp" | "sign" => {
            let cut = lo.y + TRUNK_CUT;
            let low = bounds(positions.iter().map(|p| Vec3::from(*p)).filter(|p| p.y <= cut));
            let (l, h) = low.unwrap_or((lo, hi));
            vec![aabb_box(l, h.with_y(cut.min(hi.y)), surf)]
        }
        _ => vec![aabb_box(lo, hi, surf)],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_is_100k_city_instances_and_deterministic() {
        let a = layout();
        assert_eq!(a.len(), (BLOCKS * BLOCKS) as usize * PER_BLOCK + 1 + 49);
        assert_eq!(a.len() - 50, 100_000);
        assert_eq!(a, layout());
        let toon = a.iter().filter(|p| p.material >= PAL && (p.material - PAL) % 2 == 1).count();
        assert!(toon > 10_000 && toon < 90_000, "mixed PBR and toon: {toon}");
        for p in &a {
            assert!(p.part == usize::MAX || p.part < PARTS.len());
            assert!(p.transform.translation.is_finite());
        }
        let tallest = a.iter().filter(|p| p.part < 3).map(|p| p.transform.matrix3.y_axis.y).fold(0.0, f32::max);
        assert!(tallest <= MAX_BUILDING_H && tallest > 70.0, "{tallest}");
    }

    #[test]
    fn every_part_has_3_or_4_lods_with_rising_measured_error() {
        assert_eq!(PARTS.len(), 27);
        for p in PARTS {
            let (m, lods, rep) = build_part(&(p.build)());
            assert!((3..=4).contains(&lods.len()), "{}: {} lods {rep:?}", p.name, lods.len());
            for w in rep.windows(2) {
                assert!(w[1].triangles < w[0].triangles && w[1].error >= w[0].error, "{}: {rep:?}", p.name);
            }
            for r in &rep {
                assert!(r.error >= r.measured, "{}: stored error below the measurement {rep:?}", p.name);
            }
            assert!(rep.last().unwrap().error > 0.0, "{}: {rep:?}", p.name);
            m.validate().unwrap();
        }
    }

    #[test]
    fn parts_wind_outwards() {
        for p in PARTS {
            let (m, _, _) = build_part(&(p.build)());
            let v: f32 = m
                .indices
                .as_chunks::<3>()
                .0
                .iter()
                .map(|t| {
                    let (a, b, c) = (Vec3::from(m.positions[t[0] as usize]), Vec3::from(m.positions[t[1] as usize]), Vec3::from(m.positions[t[2] as usize]));
                    a.dot(b.cross(c)) / 6.0
                })
                .sum();
            assert!(v > 0.0, "{}: volume {v}", p.name);
        }
    }

    #[test]
    fn enu_round_trip_is_close() {
        for p in layout().iter().step_by(997) {
            let back = from_enu(&to_enu(&p.transform));
            assert!(back.translation.abs_diff_eq(p.transform.translation, 1e-3));
            assert!(back.matrix3.abs_diff_eq(p.transform.matrix3, 1e-3), "{:?} vs {:?}", back, p.transform);
        }
        let t = to_enu(&Affine3A::from_translation(Vec3::new(1.0, 2.0, 3.0)));
        assert_eq!(t.translation, [1.0, -3.0, 2.0]);
    }
}
