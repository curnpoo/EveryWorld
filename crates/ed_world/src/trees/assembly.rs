//! Math-driven assembly of authored parts (trees G6): a whorled conifer (Norway spruce,
//! European larch) grown from an [`AsmGenome`] (one draw from the species' [`AsmRanges`]) with
//! the species' authored sprays ([`super::parts`]) instanced along every branch, each with its
//! own rotation, scale and bend. The same plan builds the near model and the far (T1) model
//! (one two-card stand-in per kept spray), so the LOD chain and the impostor bake show the
//! same tree.
//!
//! Deterministic: every branch draws from its own hashed stream (no shared counter between the
//! near and far builds), no map iteration.

use super::parts::{Part, PartSet};
use super::*;

/// Genome ranges of an assembled conifer (min, max). Lengths in metres for a ~20 m tree,
/// angles in radians.
#[derive(Clone, Copy, Debug)]
pub struct AsmRanges {
    /// Height growth between whorls (one year's leader).
    pub whorl_step: (f32, f32),
    /// Branches per whorl.
    pub branches: (f32, f32),
    /// Interwhorl branches per whorl gap (mean count).
    pub interwhorl: (f32, f32),
    /// Branch elevation at the live crown base and at the top (negative = down).
    pub elev_base: (f32, f32),
    pub elev_top: (f32, f32),
    /// Branch droop (m of drop per m of branch at its tip, lower crown).
    pub droop: (f32, f32),
    /// The tip's turn up again (swan neck), per m of branch.
    pub tip_rise: (f32, f32),
    /// Longest branch / tree height.
    pub width_k: (f32, f32),
    /// Crown envelope exponent: 1 a straight cone, < 1 fuller shoulders, > 1 a spire.
    pub env_pow: (f32, f32),
    /// Live crown base (fraction of height).
    pub crown_base: (f32, f32),
    /// Phyllotaxis: azimuth jitter of branches in a whorl (rad).
    pub jitter: (f32, f32),
    /// Probability a whorl branch is missing (gaps).
    pub gap: (f32, f32),
    /// Probability a dead branch below the crown shows a stub.
    pub dead_stub: (f32, f32),
    /// Spray size multiplier and spacing (fraction of a spray's length between sprays).
    pub spray_scale: (f32, f32),
    pub spray_spacing: (f32, f32),
    /// Share of curtain-carrying sprays (a, b) in the lower and middle crown.
    pub curtain: (f32, f32),
    /// Stem lean (rad) and sweep (bow, fraction of height).
    pub lean: (f32, f32),
    pub sweep: (f32, f32),
    /// Foliage clothes the outer this-many metres of a branch.
    pub foliage_m: (f32, f32),
    /// Leader size multiplier.
    pub top: (f32, f32),
}

/// One tree's draw from [`AsmRanges`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AsmGenome {
    pub whorl_step: f32,
    pub branches: f32,
    pub interwhorl: f32,
    pub elev_base: f32,
    pub elev_top: f32,
    pub droop: f32,
    pub tip_rise: f32,
    pub width_k: f32,
    pub env_pow: f32,
    pub crown_base: f32,
    pub jitter: f32,
    pub gap: f32,
    pub dead_stub: f32,
    pub spray_scale: f32,
    pub spray_spacing: f32,
    pub curtain: f32,
    pub lean: f32,
    pub sweep: f32,
    pub foliage_m: f32,
    pub top: f32,
}

impl AsmGenome {
    /// Deterministic draw from `r`.
    pub fn sample(r: &AsmRanges, seed: u64) -> AsmGenome {
        let h = |salt: u64| crate::hash01(seed, 0xA5E0 + salt);
        let l = |(a, b): (f32, f32), salt: u64| a + (b - a) * h(salt);
        AsmGenome {
            whorl_step: l(r.whorl_step, 1),
            branches: l(r.branches, 2),
            interwhorl: l(r.interwhorl, 3),
            elev_base: l(r.elev_base, 4),
            elev_top: l(r.elev_top, 5),
            droop: l(r.droop, 6),
            tip_rise: l(r.tip_rise, 7),
            width_k: l(r.width_k, 8),
            env_pow: l(r.env_pow, 9),
            crown_base: l(r.crown_base, 10),
            jitter: l(r.jitter, 11),
            gap: l(r.gap, 12),
            dead_stub: l(r.dead_stub, 13),
            spray_scale: l(r.spray_scale, 14),
            spray_spacing: l(r.spray_spacing, 15),
            curtain: l(r.curtain, 16),
            lean: l(r.lean, 17),
            sweep: l(r.sweep, 18),
            foliage_m: l(r.foliage_m, 19),
            top: l(r.top, 20),
        }
    }
}

/// Norway spruce: a narrow dark cone, live crown to near the ground, level branches drooping
/// in the lower crown with upturned tips, comb branchlets hanging under them.
pub const SPRUCE: AsmRanges = AsmRanges {
    whorl_step: (0.38, 0.56),
    branches: (4.0, 6.5),
    interwhorl: (0.4, 1.2),
    elev_base: (-0.38, -0.12),
    elev_top: (0.35, 0.62),
    droop: (-0.42, -0.22),
    tip_rise: (0.06, 0.16),
    width_k: (0.17, 0.23),
    env_pow: (0.8, 1.05),
    crown_base: (0.03, 0.14),
    jitter: (0.2, 0.5),
    gap: (0.03, 0.12),
    dead_stub: (0.4, 0.8),
    spray_scale: (0.9, 1.15),
    spray_spacing: (0.5, 0.65),
    curtain: (0.55, 0.9),
    lean: (0.0, 0.035),
    sweep: (0.0, 0.02),
    foliage_m: (1.6, 2.4),
    top: (0.8, 1.2),
};

/// European larch: a light, open, irregular cone; upswept upper branches, long drooping
/// branchlets lower down, a higher crown base.
pub const LARCH: AsmRanges = AsmRanges {
    whorl_step: (0.5, 0.8),
    branches: (3.0, 5.5),
    interwhorl: (0.8, 1.8),
    elev_base: (-0.3, 0.0),
    elev_top: (0.5, 0.85),
    droop: (-0.45, -0.2),
    tip_rise: (0.04, 0.12),
    width_k: (0.15, 0.22),
    env_pow: (0.7, 1.0),
    crown_base: (0.2, 0.4),
    jitter: (0.3, 0.7),
    gap: (0.1, 0.25),
    dead_stub: (0.5, 0.9),
    spray_scale: (0.9, 1.15),
    spray_spacing: (0.6, 0.85),
    curtain: (0.3, 0.7),
    lean: (0.0, 0.05),
    sweep: (0.0, 0.03),
    foliage_m: (2.0, 3.0),
    top: (0.8, 1.2),
};

/// Species-level constants of an assembled conifer.
#[derive(Clone, Copy, Debug)]
pub struct AsmSpecies {
    pub ranges: &'static AsmRanges,
    /// Parts prefix (`<prefix>_spray_a`, ...).
    pub prefix: &'static str,
    pub bark: u32,
    /// Butt radius of a 20 m tree (m).
    pub trunk_r: f32,
    /// Spray budget of the near model (T0 triangles ~ 11 per spray plus bark).
    pub max_units: usize,
    /// Sprays along a branch overlap by `overlap` and grow at most `max_grow` x their authored
    /// size to clothe a big tree's branches within the budget.
    pub overlap: f32,
    pub max_grow: f32,
}

pub const SPRUCE_SPECIES: AsmSpecies = AsmSpecies { ranges: &SPRUCE, prefix: "spruce", bark: 0x6e5444, trunk_r: 0.26, max_units: 270, overlap: 1.5, max_grow: 3.2 };
pub const LARCH_SPECIES: AsmSpecies = AsmSpecies { ranges: &LARCH, prefix: "larch", bark: 0x6a4c3e, trunk_r: 0.26, max_units: 260, overlap: 2.4, max_grow: 4.5 };

/// Cards of one far stand-in ([`PartSet::far`]): the T1 budget is `T1_CARDS / FAR_CARDS` sprays.
pub const FAR_CARDS: usize = 2;

struct Branch {
    tp: Vec3,
    t: f32,
    az: f32,
    live: bool,
    inter: bool,
    key: u64,
}

impl Builder {
    /// Instances `part` at `origin` with basis (`x` along, `y` up, `z` side), per-axis `scale`
    /// (part space), bending its tip down by `bend` (m per m squared along +X; sprays only).
    #[allow(clippy::too_many_arguments)]
    fn part(&mut self, part: &Part, origin: Vec3, basis: [Vec3; 3], scale: Vec3, bend: f32, sway: f32, phase: f32, tint: [f32; 4], vertical: bool) {
        let col = mulc(self.leaf_col, tint);
        let first = self.leaves.mesh.positions.len() as u32;
        let main = (part.length * if vertical { scale.y } else { scale.x }).max(1e-3);
        for i in 0..part.positions.len() {
            let mut p = Vec3::from(part.positions[i]) * scale;
            let along = if vertical { p.y } else { p.x };
            if !vertical {
                p.y -= bend * p.x.max(0.0).powi(2);
            }
            let w = origin + basis[0] * p.x + basis[1] * p.y + basis[2] * p.z;
            let n0 = Vec3::from(part.normals[i]);
            let n = (basis[0] * n0.x + basis[1] * n0.y + basis[2] * n0.z).normalize_or(Vec3::Y);
            let v = (along / main).clamp(0.0, 1.0);
            let wv = pack_wind(sway + self.flex * 0.03 * along.abs(), 0.06 * (0.3 + 0.7 * v) * self.flex, phase);
            let uv = part.uvs[i];
            self.leaves.vertex(w, n, atlas_uv(part.cells[i], uv[0], uv[1]), col, wv);
            self.leaf_kind.push(LeafVert::Card);
        }
        self.leaves.mesh.indices.extend(part.indices.iter().map(|&k| k + first));
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Grows an assembled conifer of height `h` into `b` (near or far, by `b.far`).
pub(super) fn assembled_conifer(b: &mut Builder, leaf: u32, sp: &AsmSpecies, g: &AsmGenome, set: &PartSet, h: f32) {
    b.h = h;
    b.leaf_col = leaf_tint(leaf);
    b.bark_col = srgb(sp.bark);
    b.normal_bend = 0.6;
    let seed = b.seed;
    let tree = Rng::new(seed ^ 0x7A55_E3B1);
    // Stem: lean plus a gentle bow (sweep), conifer taper.
    let lean_az = tree.r(0.0, TAU);
    let bow_az = tree.r(0.0, TAU);
    let lean = Vec3::new(lean_az.cos(), 0.0, lean_az.sin()) * g.lean;
    let bow = Vec3::new(bow_az.cos(), 0.0, bow_az.sin()) * g.sweep * h;
    let trunk: Vec<Vec3> = (0..=16)
        .map(|i| {
            let y = -0.4 + (h + 0.4) * i as f32 / 16.0;
            let u = (y / h).clamp(0.0, 1.0);
            Vec3::new(0.0, y, 0.0) + lean * y + bow * (PI * u).sin()
        })
        .collect();
    let trunk_r = sp.trunk_r * (h / 20.0).clamp(0.08, 1.6);
    let radii: Vec<f32> = (0..trunk.len()).map(|i| trunk_r * (1.0 - i as f32 / 16.0).max(0.03).powf(0.9)).collect();
    b.tube(&trunk, &radii, &[0.0; 17], 0.0, TubeStyle { flare: 0.6, ..TubeStyle::bark(8) });
    let at = |y: f32| lerp_path(&trunk, (y + 0.4) / (h + 0.4)).0;

    // Plan the branches: whorls with phyllotaxis, gaps, interwhorl branches.
    let size = (h / 20.0).clamp(0.12, 1.5);
    let step0 = g.whorl_step * size.powf(0.6);
    let mut branches: Vec<Branch> = Vec::new();
    let mut y = 0.35 + tree.r(0.0, 0.3) * size;
    let mut az0 = tree.r(0.0, TAU);
    let cb = g.crown_base;
    let top_y = h - 0.55 * g.top * size.sqrt().min(1.0);
    while y < top_y {
        let t = y / h;
        let n = (g.branches + tree.r(-0.8, 0.8)).round().max(2.0) as usize;
        az0 += 2.4 + tree.r(-0.3, 0.3);
        let live = t >= cb * tree.r(0.8, 1.2);
        for k in 0..n {
            let gap = tree.f() < g.gap;
            let az = az0 + k as f32 / n as f32 * TAU + tree.r(-1.0, 1.0) * g.jitter;
            if !gap {
                branches.push(Branch { tp: at(y), t, az, live, inter: false, key: branches.len() as u64 });
            }
        }
        let step = step0 * tree.r(0.8, 1.25) * (1.0 - 0.35 * t);
        let ni = (g.interwhorl + tree.f()).floor() as usize;
        for _ in 0..ni {
            let yi = y + step * tree.r(0.25, 0.8);
            if yi < top_y {
                branches.push(Branch { tp: at(yi), t: yi / h, az: tree.r(0.0, TAU), live: yi / h >= cb * 1.1, inter: true, key: branches.len() as u64 });
            }
        }
        y += step;
    }

    // Branch shapes and spray counts (first pass: the spray budget).
    struct Shape1 {
        path: Vec<Vec3>,
        len: f32,
        ct: f32,
        ls: f32,
        s0: f32,
    }
    let spray_len = set.sprays[0].length;
    let shapes: Vec<Option<Shape1>> = branches
        .iter()
        .map(|br| {
            if !br.live {
                return None;
            }
            let r = Rng::new(seed ^ 0xB4A1 ^ br.key.wrapping_mul(0x9E37_79B9_7F4A_7C15));
            let ct = ((br.t - cb) / (1.0 - cb)).clamp(0.0, 1.0);
            let prof = (1.0 - ct).powf(g.env_pow) * (0.7 + 0.3 * (ct * 5.0).min(1.0));
            let len = (g.width_k * h * prof + 0.25 * size) * r.r(0.85, 1.15) * if br.inter { r.r(0.45, 0.7) } else { 1.0 };
            let elev = lerp(g.elev_base, g.elev_top, ct.powf(1.3)) + r.r(-0.08, 0.08);
            let dir = Vec3::new(br.az.cos() * elev.cos(), elev.sin(), br.az.sin() * elev.cos());
            let droop = g.droop * (0.4 + 0.6 * (1.0 - ct)) * r.r(0.75, 1.25);
            let rise = g.tip_rise * r.r(0.6, 1.4);
            let path: Vec<Vec3> = (0..=5)
                .map(|i| {
                    let u = i as f32 / 5.0;
                    br.tp + dir * len * u + Vec3::Y * len * (droop * u * u + rise * u.powi(4))
                })
                .collect();
            let ls = (spray_len * g.spray_scale * (0.7 + 0.3 * (1.0 - ct))).min(0.65 * len);
            let s0 = (1.0 - g.foliage_m * size.sqrt() / len).clamp(0.0, 0.55) * len;
            Some(Shape1 { path, len, ct, ls, s0 })
        })
        .collect();
    // Every live branch carries its tip spray; the rest of the budget spreads the others.
    let extra = |s: &Shape1| (s.len - s.s0 - 0.8 * s.ls).max(0.0) / (g.spray_spacing * s.ls);
    let live = shapes.iter().flatten().count() as f32;
    let want: f32 = shapes.iter().flatten().map(extra).sum();
    let fill = ((sp.max_units as f32 - live).max(0.0) / want.max(1e-3)).min(1.0);

    for (br, sh) in branches.iter().zip(&shapes) {
        let r = Rng::new(seed ^ 0x5B7A ^ br.key.wrapping_mul(0xD1B5_4A32_D192_ED03));
        let phase = r.f();
        let Some(s) = sh else {
            // Dead lower branch: a short bare stub (near only), some broken off.
            let keep = r.f() < g.dead_stub;
            let len = r.r(0.3, 1.4) * size;
            if keep && !b.far {
                let out = Vec3::new(br.az.cos(), -0.15, br.az.sin()).normalize();
                let path = [br.tp, br.tp + out * len * 0.5, br.tp + out * len - Vec3::Y * 0.05 * len];
                let rr = 0.022 * (1.0 - br.t) + 0.008;
                b.tube(&path, &[rr, rr * 0.6, rr * 0.3], &[0.0; 3], 0.0, TubeStyle::bark(3));
            }
            continue;
        };
        let ct = s.ct;
        if !b.far && !br.inter && (s.s0 > 0.2 || s.len > 2.5) {
            // Bare wood on the inner part; the tip is all foliage.
            let rr = (0.035 * (1.0 - br.t) + 0.012) * size.sqrt().max(0.4);
            let whip = [0.0, 0.04 * s.len * 0.8];
            b.tube(&[s.path[0], s.path[4]], &[rr, rr * 0.25], &whip, phase, TubeStyle::bark(3));
        }
        let m = 1 + (extra(s) * fill + r.f()).floor() as usize;
        // A big tree has more shoots than the budget has sprays: each spray stands for a
        // stretch of the branch (overlapping), growing up to `max_grow` x its authored size.
        let ls = (s.len - s.s0) * sp.overlap / m as f32;
        let grow = (ls / s.ls).clamp(1.0, sp.max_grow);
        let span = (s.len - s.s0 - 0.8 * s.ls * grow).max(0.0);
        let path_at = |d: f32| lerp_path(&s.path, (d / s.len).clamp(0.0, 1.0));
        for i in 0..m {
            // Same draws for near and far (the far model must place the same sprays).
            let yaw = if i + 1 == m { r.r(-0.1, 0.1) } else { (if (i + br.key as usize) % 2 == 0 { 1.0 } else { -1.0 }) * r.r(0.15, 0.55) };
            let roll = r.r(-0.3, 0.3);
            let pick = r.f();
            let sc = r.r(0.88, 1.12);
            let wide = r.r(0.85, 1.15);
            let bend = r.r(0.0, 0.35) * (1.0 - ct);
            let value = r.r(0.9, 1.08);
            let d = if m == 1 { (s.len - 0.8 * s.ls * grow).max(0.0) } else { s.s0 + span * i as f32 / (m - 1) as f32 };
            let (q, tan, _) = path_at(d);
            let Some(k) = b.card_keep() else { continue };
            let x0 = (Quat::from_axis_angle(Vec3::Y, yaw) * tan).normalize_or(Vec3::X);
            let z0 = x0.cross(Vec3::Y).normalize_or(Vec3::Z);
            let y0 = z0.cross(x0);
            let rot = Quat::from_axis_angle(x0, roll);
            let basis = [x0, rot * y0, rot * z0];
            let part = if b.far {
                set.far
            } else if ct > 0.72 || pick > g.curtain {
                set.sprays[2]
            } else if pick < g.curtain * 0.5 {
                set.sprays[0]
            } else {
                set.sprays[1]
            };
            let ls = s.ls * sc * grow * k;
            let f = ls / part.length.max(1e-3);
            // Far stand-ins grow about their middle (the crown keeps its size).
            let origin = if k > 1.0 { q - x0 * s.ls * sc * grow * (k - 1.0) * 0.5 } else { q };
            let sway = b.sway(q, 0.04 * d);
            let tint = [value, value, value * 0.98, 1.0];
            b.part(part, origin, basis, Vec3::new(f, f, f * wide), bend / f.max(0.2), sway, phase + i as f32 * 0.07, tint, false);
        }
    }

    // Leader: the top of the stem.
    let top = *trunk.last().unwrap();
    let r = Rng::new(seed ^ 0x1EAD);
    if let Some(k) = b.card_keep() {
        let part = set.leader;
        let f = g.top * size.sqrt().min(1.0) * k;
        let base = top - Vec3::Y * part.length * f * 0.9;
        let yaw = Quat::from_axis_angle(Vec3::Y, r.r(0.0, TAU));
        let up = (Vec3::Y + lean * 2.0).normalize();
        let x = (yaw * Vec3::X).normalize();
        let z = x.cross(up).normalize_or(Vec3::Z);
        let x = up.cross(z);
        let sway = b.sway(top, 0.0);
        b.part(part, base, [x, up, z], Vec3::splat(f), 0.0, sway, 0.0, [1.0; 4], true);
    }
}
