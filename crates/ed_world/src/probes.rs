//! Terrain-following light probes ("probe GI").
//!
//! A camera-centred grid of probes hugs the ground: each column sits on the
//! terrain and its `ny` layers are spaced by `t^2` (dense near the ground,
//! sparse up to `span` metres). Every probe traces rays through a 2.5D
//! [`SceneField`] (terrain + building tops) and stores, per ambient-cube face:
//!
//! * `vis`: sky visibility relative to open flat ground (1 = unoccluded),
//! * `bounce`: cosine-weighted albedo of what the face sees (light bounced
//!   off sunlit surroundings, in units of the sky's up-irradiance).
//!
//! The renderer combines them with the sky env map:
//! `diffuse = env(N) * vis(N) + env(up) * bounce(N) * K` (see the patched
//! `vendor/bevy_pbr/.../irradiance_volume.wesl`). Everything here is plain data
//! and deterministic.
//!
//! Frame: volume-local, x = east, y = up, z = south (render ENU minus the
//! volume centre). The volume is axis-aligned in that frame.

use glam::{Vec2, Vec3};

/// 2.5D scene: per cell ground height, top of solid (ground or roof), albedo.
#[derive(Clone, Debug)]
pub struct SceneField {
    /// x, z of cell (0, 0)'s centre.
    pub origin: Vec2,
    pub cell: f32,
    pub nx: usize,
    pub nz: usize,
    /// Terrain height (no buildings).
    pub ground: Vec<f32>,
    /// Top of solid: max(terrain, roofs).
    pub top: Vec<f32>,
    pub albedo: Vec<[f32; 3]>,
}

impl SceneField {
    pub fn new(origin: Vec2, cell: f32, nx: usize, nz: usize) -> Self {
        let n = nx * nz;
        Self { origin, cell, nx, nz, ground: vec![0.0; n], top: vec![0.0; n], albedo: vec![[0.2; 3]; n] }
    }

    /// Centre of cell (i, k).
    pub fn cell_center(&self, i: usize, k: usize) -> Vec2 {
        self.origin + Vec2::new(i as f32, k as f32) * self.cell
    }

    fn cell_of(&self, x: f32, z: f32) -> Option<usize> {
        let fx = ((x - self.origin.x) / self.cell + 0.5).floor();
        let fz = ((z - self.origin.y) / self.cell + 0.5).floor();
        if fx < 0.0 || fz < 0.0 || fx >= self.nx as f32 || fz >= self.nz as f32 {
            return None;
        }
        Some(fz as usize * self.nx + fx as usize)
    }

    /// Bilinear terrain height (clamped at the field edge).
    pub fn ground_at(&self, x: f32, z: f32) -> f32 {
        let fx = ((x - self.origin.x) / self.cell).clamp(0.0, (self.nx - 1) as f32);
        let fz = ((z - self.origin.y) / self.cell).clamp(0.0, (self.nz - 1) as f32);
        let (x0, z0) = (fx.floor() as usize, fz.floor() as usize);
        let (x1, z1) = ((x0 + 1).min(self.nx - 1), (z0 + 1).min(self.nz - 1));
        let (tx, tz) = (fx - x0 as f32, fz - z0 as f32);
        let g = |x: usize, z: usize| self.ground[z * self.nx + x];
        let a = g(x0, z0) + (g(x1, z0) - g(x0, z0)) * tx;
        let b = g(x0, z1) + (g(x1, z1) - g(x0, z1)) * tx;
        a + (b - a) * tz
    }

    fn mean_albedo(&self) -> [f32; 3] {
        let mut m = [0.0; 3];
        for a in &self.albedo {
            for c in 0..3 {
                m[c] += a[c];
            }
        }
        let n = self.albedo.len().max(1) as f32;
        m.map(|v| v / n)
    }
}

#[derive(Clone, Debug)]
pub struct ProbeParams {
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
    /// Horizontal probe spacing (m).
    pub spacing: f32,
    /// Lowest layer height above ground (m).
    pub h0: f32,
    /// Height of the top layer above the lowest (m).
    pub span: f32,
    /// Rays per probe (uniform sphere; each feeds three faces).
    pub rays: usize,
    /// Max ray length (m).
    pub max_dist: f32,
    /// Unit vector toward the sun (volume frame, y up); ZERO = ignore the sun (every hit counts as lit).
    pub sun: Vec3,
}

impl Default for ProbeParams {
    fn default() -> Self {
        Self { nx: 64, ny: 6, nz: 64, spacing: 4.0, h0: 0.4, span: 48.0, rays: 48, max_dist: 80.0, sun: Vec3::ZERO }
    }
}

/// Ambient-cube face directions: +X, -X, +Y, -Y, +Z, -Z.
pub const FACES: [Vec3; 6] = [Vec3::X, Vec3::NEG_X, Vec3::Y, Vec3::NEG_Y, Vec3::Z, Vec3::NEG_Z];

/// One baked probe: rgba = (bounce rgb, vis) per face, in [`FACES`] order.
#[derive(Clone, Copy, Debug, Default)]
pub struct Probe {
    pub pos: Vec3,
    pub faces: [[f32; 4]; 6],
    /// Was inside solid; values were filled from neighbours.
    pub filled: bool,
}

#[derive(Clone, Debug)]
pub struct ProbeVolume {
    pub params: ProbeParams,
    /// Box in volume-local space (x/z centred on 0).
    pub box_min: Vec3,
    pub box_max: Vec3,
    /// Probes, index = x + nx * (j + ny * z).
    pub probes: Vec<Probe>,
    /// GPU texel data, dims (nx, 2 * ny, 4 * nz), x fastest.
    pub texels: Vec<[f32; 4]>,
}

impl ProbeVolume {
    pub fn dims(&self) -> (usize, usize, usize) {
        (self.params.nx, 2 * self.params.ny, 4 * self.params.nz)
    }
    pub fn probe(&self, i: usize, j: usize, k: usize) -> &Probe {
        let p = &self.params;
        &self.probes[i + p.nx * (j + p.ny * k)]
    }
}

/// Height of layer `j` above the lowest layer.
pub fn layer_height(p: &ProbeParams, j: usize) -> f32 {
    let t = j as f32 / (p.ny.max(2) - 1) as f32;
    p.span * t * t
}

/// Deterministic, roughly uniform sphere directions (Fibonacci lattice).
pub fn sphere_dirs(n: usize) -> Vec<Vec3> {
    let ga = std::f32::consts::PI * (3.0 - 5f32.sqrt());
    (0..n)
        .map(|i| {
            let y = 1.0 - 2.0 * (i as f32 + 0.5) / n as f32;
            let r = (1.0 - y * y).max(0.0).sqrt();
            let a = ga * i as f32;
            Vec3::new(r * a.cos(), y, r * a.sin())
        })
        .collect()
}

enum Hit {
    Sky,
    Surface([f32; 3], Vec3),
}

fn trace(f: &SceneField, o: Vec3, d: Vec3, max_dist: f32, max_top: f32, mean: [f32; 3]) -> Hit {
    let base = f.cell * 0.6;
    let mut t = base * 0.5;
    while t < max_dist {
        let p = o + d * t;
        if d.y >= 0.0 && p.y > max_top {
            return Hit::Sky;
        }
        let Some(c) = f.cell_of(p.x, p.z) else {
            return if d.y > 0.0 { Hit::Sky } else { Hit::Surface(mean, p) };
        };
        if p.y < f.top[c] {
            return Hit::Surface(f.albedo[c], p);
        }
        // Coarser steps further out: far occluders matter less.
        t += base + t * 0.04;
    }
    if d.y > 0.0 { Hit::Sky } else { Hit::Surface(mean, o + d * max_dist) }
}

/// Fraction of a cosine lobe around `n` that is above the horizon of flat ground.
pub fn open_fraction(n: Vec3) -> f32 {
    (1.0 + n.y) * 0.5
}

fn bake_probe(f: &SceneField, p: &ProbeParams, pos: Vec3, dirs: &[Vec3], max_top: f32, mean: [f32; 3]) -> [[f32; 4]; 6] {
    let mut sky = [0.0f32; 6];
    let mut bounce = [[0.0f32; 3]; 6];
    let mut wsum = [0.0f32; 6];
    for &d in dirs {
        let hit = trace(f, pos, d, p.max_dist, max_top, mean);
        for (fi, n) in FACES.iter().enumerate() {
            let w = d.dot(*n);
            if w <= 0.0 {
                continue;
            }
            wsum[fi] += w;
            match hit {
                Hit::Sky => sky[fi] += w,
                Hit::Surface(a, hp) => {
                    // Sun-aware bounce: a hit that the sun cannot reach only gets sky fill.
                    let lit = if p.sun == Vec3::ZERO {
                        1.0
                    } else if p.sun.y <= 0.02 {
                        0.0
                    } else {
                        match trace(f, hp + Vec3::Y * 0.4 + p.sun * 0.3, p.sun, 150.0, max_top, mean) {
                            Hit::Sky => 1.0,
                            Hit::Surface(..) => 0.0,
                        }
                    };
                    let g = 0.25 + 0.75 * lit;
                    for c in 0..3 {
                        bounce[fi][c] += w * a[c] * g;
                    }
                }
            }
        }
    }
    let mut out = [[0.0f32; 4]; 6];
    for fi in 0..6 {
        let ws = wsum[fi].max(1e-6);
        let s = sky[fi] / ws;
        let open = open_fraction(FACES[fi]);
        let vis = if open > 0.25 { (s / open).min(1.0) } else { s };
        out[fi] = [bounce[fi][0] / ws, bounce[fi][1] / ws, bounce[fi][2] / ws, vis];
    }
    out
}

/// Bake a probe volume centred on (0, 0) of `field`'s frame.
pub fn bake(field: &SceneField, params: &ProbeParams, threads: usize) -> ProbeVolume {
    let p = params.clone();
    let dirs = sphere_dirs(p.rays);
    let mean = field.mean_albedo();
    let max_top = field.top.iter().copied().fold(f32::MIN, f32::max) + 0.5;
    let half_x = p.nx as f32 * p.spacing * 0.5;
    let half_z = p.nz as f32 * p.spacing * 0.5;
    let col = |i: usize, k: usize| Vec2::new((i as f32 + 0.5) * p.spacing - half_x, (k as f32 + 0.5) * p.spacing - half_z);

    // Probe positions + whether they start inside solid.
    let mut probes = vec![Probe::default(); p.nx * p.ny * p.nz];
    for k in 0..p.nz {
        for i in 0..p.nx {
            let c = col(i, k);
            let g = field.ground_at(c.x, c.y) + p.h0;
            for j in 0..p.ny {
                let pos = Vec3::new(c.x, g + layer_height(&p, j), c.y);
                let inside = field.cell_of(pos.x, pos.z).is_some_and(|ci| pos.y < field.top[ci]);
                probes[i + p.nx * (j + p.ny * k)] = Probe { pos, faces: [[0.0; 4]; 6], filled: inside };
            }
        }
    }

    // Trace (parallel over contiguous probe chunks).
    let threads = threads.max(1);
    let chunk = probes.len().div_ceil(threads);
    std::thread::scope(|s| {
        for part in probes.chunks_mut(chunk) {
            let (dirs, p) = (&dirs, &p);
            s.spawn(move || {
                for pr in part.iter_mut().filter(|pr| !pr.filled) {
                    pr.faces = bake_probe(field, p, pr.pos, dirs, max_top, mean);
                }
            });
        }
    });

    dilate(&mut probes, &p);

    // Box: ground range plus the layer span, small margins.
    let (mut gmin, mut gmax) = (f32::MAX, f32::MIN);
    for k in 0..p.nz {
        for i in 0..p.nx {
            let c = col(i, k);
            let g = field.ground_at(c.x, c.y);
            gmin = gmin.min(g);
            gmax = gmax.max(g);
        }
    }
    let box_min = Vec3::new(-half_x, gmin - 2.0, -half_z);
    let box_max = Vec3::new(half_x, gmax + p.h0 + p.span + 4.0, half_z);
    let texels = pack(&probes, &p, field, box_min, box_max);
    ProbeVolume { params: p, box_min, box_max, probes, texels }
}

/// Fill probes that sit inside solid from valid neighbours (same layer first,
/// then the layer above), so walls don't pull in garbage when interpolating.
fn dilate(probes: &mut [Probe], p: &ProbeParams) {
    let idx = |i: usize, j: usize, k: usize| i + p.nx * (j + p.ny * k);
    let mut valid: Vec<bool> = probes.iter().map(|pr| !pr.filled).collect();
    for _ in 0..8 {
        let mut changed = false;
        let snapshot: Vec<[[f32; 4]; 6]> = probes.iter().map(|pr| pr.faces).collect();
        let was = valid.clone();
        for k in 0..p.nz {
            for j in 0..p.ny {
                for i in 0..p.nx {
                    let me = idx(i, j, k);
                    if was[me] {
                        continue;
                    }
                    let mut acc = [[0.0f32; 4]; 6];
                    let mut n = 0.0;
                    let mut add = |o: usize| {
                        if was[o] {
                            for f in 0..6 {
                                for c in 0..4 {
                                    acc[f][c] += snapshot[o][f][c];
                                }
                            }
                            n += 1.0;
                        }
                    };
                    if i > 0 { add(idx(i - 1, j, k)); }
                    if i + 1 < p.nx { add(idx(i + 1, j, k)); }
                    if k > 0 { add(idx(i, j, k - 1)); }
                    if k + 1 < p.nz { add(idx(i, j, k + 1)); }
                    if j + 1 < p.ny { add(idx(i, j + 1, k)); }
                    if n > 0.0 {
                        for f in 0..6 {
                            for c in 0..4 {
                                acc[f][c] /= n;
                            }
                        }
                        probes[me].faces = acc;
                        valid[me] = true;
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    // Anything still unfilled (deep inside a huge footprint): dark, no sky.
    for (pr, v) in probes.iter_mut().zip(&valid) {
        if !v {
            pr.faces = [[0.05, 0.05, 0.05, 0.0]; 6];
        }
    }
}

/// Lay probes out in the GPU texture (see module docs / the WESL patch).
fn pack(probes: &[Probe], p: &ProbeParams, field: &SceneField, box_min: Vec3, box_max: Vec3) -> Vec<[f32; 4]> {
    let (w, h, d) = (p.nx, 2 * p.ny, 4 * p.nz);
    let mut tex = vec![[0.0f32; 4]; w * h * d];
    let at = |x: usize, y: usize, z: usize| x + w * (y + h * z);
    let size_y = box_max.y - box_min.y;
    let center_y = (box_max.y + box_min.y) * 0.5;
    let half_x = p.nx as f32 * p.spacing * 0.5;
    let half_z = p.nz as f32 * p.spacing * 0.5;
    for k in 0..p.nz {
        for i in 0..p.nx {
            for j in 0..p.ny {
                let pr = &probes[i + p.nx * (j + p.ny * k)];
                for axis in 0..3 {
                    tex[at(i, j, k + axis * p.nz)] = pr.faces[axis * 2];
                    tex[at(i, j + p.ny, k + axis * p.nz)] = pr.faces[axis * 2 + 1];
                }
            }
            let x = (i as f32 + 0.5) * p.spacing - half_x;
            let z = (k as f32 + 0.5) * p.spacing - half_z;
            let g = field.ground_at(x, z) + p.h0;
            tex[at(i, 0, k + 3 * p.nz)] = [(g - center_y) / size_y, p.span / size_y, 0.0, 0.0];
        }
    }
    tex
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(albedo: f32) -> SceneField {
        let mut f = SceneField::new(Vec2::splat(-100.0), 2.0, 101, 101);
        f.albedo.iter_mut().for_each(|a| *a = [albedo; 3]);
        f
    }

    fn small() -> ProbeParams {
        ProbeParams { nx: 4, ny: 3, nz: 4, spacing: 4.0, rays: 256, ..Default::default() }
    }

    #[test]
    fn open_flat_ground() {
        let v = bake(&flat(0.3), &small(), 2);
        let pr = v.probe(1, 0, 1);
        let up = pr.faces[2];
        let down = pr.faces[3];
        let side = pr.faces[0];
        assert!(up[3] > 0.97, "up vis {}", up[3]);
        assert!(up[0] < 0.02, "up bounce {}", up[0]);
        assert!(down[3] < 0.02, "down vis {}", down[3]);
        assert!((down[0] - 0.3).abs() < 0.02, "down bounce {}", down[0]);
        // Horizontal: full relative vis, half the lobe sees ground.
        assert!(side[3] > 0.9, "side vis {}", side[3]);
        assert!((side[0] - 0.15).abs() < 0.04, "side bounce {}", side[0]);
    }

    #[test]
    fn wall_occludes_facing_side() {
        let mut f = flat(0.2);
        // A 20 m tall wall 3 m east of x = 0 (cells x in [4, 8) m).
        for k in 0..f.nz {
            for i in 0..f.nx {
                let c = f.cell_center(i, k);
                if c.x >= 4.0 && c.x < 8.0 {
                    f.top[k * f.nx + i] = 20.0;
                    f.albedo[k * f.nx + i] = [0.5; 3];
                }
            }
        }
        let p = ProbeParams { nx: 2, ny: 2, nz: 2, spacing: 2.0, rays: 512, ..Default::default() };
        let v = bake(&f, &p, 1);
        let pr = v.probe(1, 0, 0); // x = +1 m, 3 m from the wall
        let east = pr.faces[0];
        let west = pr.faces[1];
        assert!(east[3] < 0.6 * west[3], "east {} west {}", east[3], west[3]);
        assert!(east[0] > west[0], "east sees the bright wall");
    }

    #[test]
    fn probes_follow_terrain() {
        let mut f = flat(0.2);
        for k in 0..f.nz {
            for i in 0..f.nx {
                let c = f.cell_center(i, k);
                let h = c.x * 0.5; // 50% slope
                f.ground[k * f.nx + i] = h;
                f.top[k * f.nx + i] = h;
            }
        }
        let p = small();
        let v = bake(&f, &p, 2);
        for k in 0..p.nz {
            for i in 0..p.nx {
                let pr = v.probe(i, 0, k);
                let g = f.ground_at(pr.pos.x, pr.pos.z);
                assert!((pr.pos.y - g - p.h0).abs() < 1e-3);
                assert!(!pr.filled);
            }
        }
        // Ground slab encodes the lowest layer in unit space.
        let (w, h, _) = v.dims();
        let t = v.texels[w * h * (3 * p.nz)];
        let size_y = v.box_max.y - v.box_min.y;
        let center_y = (v.box_max.y + v.box_min.y) * 0.5;
        let pr = v.probe(0, 0, 0);
        assert!((t[0] * size_y + center_y - pr.pos.y).abs() < 1e-3);
    }

    #[test]
    fn buried_probes_are_filled() {
        let mut f = flat(0.2);
        for k in 0..f.nz {
            for i in 0..f.nx {
                let c = f.cell_center(i, k);
                if c.x.abs() < 3.0 && c.y.abs() < 3.0 {
                    f.top[k * f.nx + i] = 30.0;
                }
            }
        }
        let p = ProbeParams { nx: 4, ny: 3, nz: 4, spacing: 2.0, rays: 64, ..Default::default() };
        let v = bake(&f, &p, 2);
        let inner = v.probe(1, 0, 1);
        assert!(inner.filled);
        assert!(inner.faces[2][3] > 0.0, "filled from neighbours, not left black");
    }
}
