//! Cloud lighting and the reference ray march. `clouds.slang` mirrors [`march`]; the CPU
//! version is the look reference (and what `ed_render/examples/cloud_preview.rs` draws).
//!
//! Lighting, per sample:
//! - **Sun**: transmittance towards the sun from a short light march ([`light_depth`]): six
//!   exponentially growing steps (~2 km) and two long coarse ones (so tall towers shade
//!   their own lee and the cloud beside them).
//! - **Phase**: a dual-lobe Henyey-Greenstein (strong forward lobe for the silver lining
//!   when looking towards the sun, a weak back lobe for the glow away from it).
//! - **Multiple scattering**: the octave approximation (Wrenninge et al. 2013): each octave
//!   lets light through more easily (`a`), contributes less (`b`) and is more isotropic
//!   (`c`). Without it, thick clouds have black cores; with it, they glow inside.
//! - **Powder**: thin, sun-facing edges are bright and the folds between billows darker when
//!   seen with the sun behind the viewer (Schneider 2015), which is what makes cumulus look
//!   lumpy and solid rather than like flat fog.
//! - **Ambient**: sky light from above, ground bounce from below, weighted by the sample's
//!   height in its own column: tops see the sky, bases see the ground.
//!
//! Integration is energy-conserving per step (Hillaire 2016), so step size changes the
//! result very little; the start is jittered per pixel (and per frame on the GPU, which
//! accumulates), so there is no banding and, after accumulation, no grain.

use super::field::{CloudField, Lod};
use super::{lerp, saturate};
use glam::{DVec3, Vec3};

/// Light from the outside world at the clouds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CloudLight {
    /// Unit vector towards the sun (render frame).
    pub sun_dir: Vec3,
    /// Sun irradiance reaching cloud level (linear RGB).
    pub sun: Vec3,
    /// Mean sky radiance from above (linear RGB).
    pub sky: Vec3,
    /// Mean radiance from below: the lit ground (linear RGB).
    pub ground: Vec3,
}

/// March quality. [`MarchCfg::reference`] is the converged look; the GPU tiers are cheaper.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MarchCfg {
    pub max_steps: u32,
    /// Step length as a fraction of the distance (far clouds take longer steps).
    pub step_frac: f32,
    pub min_step_m: f32,
    pub max_step_m: f32,
    /// Farthest cloud drawn (m).
    pub max_dist_m: f32,
    /// Stop when this little light gets through.
    pub min_transmittance: f32,
}

impl MarchCfg {
    pub fn reference() -> Self {
        Self { max_steps: 1024, step_frac: 0.004, min_step_m: 12.0, max_step_m: 220.0, max_dist_m: 160_000.0, min_transmittance: 0.002 }
    }

    /// The High tier the GPU pass runs (`clouds.slang` defaults).
    pub fn high() -> Self {
        Self { max_steps: 160, step_frac: 0.016, min_step_m: 60.0, max_step_m: 900.0, max_dist_m: 140_000.0, min_transmittance: 0.01 }
    }
}

/// What a view ray met.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MarchOut {
    /// In-scattered light (linear RGB), before aerial perspective.
    pub radiance: Vec3,
    /// Transmittance of the clouds along the ray.
    pub transmittance: f32,
    /// Opacity-weighted mean distance of the cloud (m); 0 when none.
    pub depth_m: f32,
}

/// Step scale inside a cloud's envelope, and of a stride through clear air (both in steps).
pub const FINE: f32 = 0.4;
pub const STRIDE: f32 = 2.0;
/// Fine steps taken after the last envelope hit before striding again (covers one stride).
pub const REFINE: u32 = 6;

/// Multiple-scattering octaves and their factors (extinction, contribution, eccentricity).
pub const OCTAVES: u32 = 4;
pub const MS_A: f32 = 0.45;
pub const MS_B: f32 = 0.6;
pub const MS_C: f32 = 0.5;
/// Phase lobes: forward g, back g, back weight.
pub const G_FWD: f32 = 0.8;
pub const G_BACK: f32 = -0.3;
pub const BACK_W: f32 = 0.3;

/// Henyey-Greenstein phase (1/sr).
pub fn hg(cos: f32, g: f32) -> f32 {
    let g2 = g * g;
    (1.0 - g2) / (4.0 * std::f32::consts::PI * (1.0 + g2 - 2.0 * g * cos).max(1e-4).powf(1.5))
}

/// Dual-lobe phase with eccentricity scaled by `k` (octaves get rounder).
pub fn phase(cos: f32, k: f32) -> f32 {
    lerp(hg(cos, G_FWD * k), hg(cos, G_BACK * k), BACK_W)
}

/// Sun scattering factor (1/sr) for a sample whose optical depth towards the sun is `tau`,
/// viewed at `cos` from the sun, with local extinction `sigma` (powder).
pub fn sun_scatter(tau: f32, cos: f32, sigma: f32) -> f32 {
    let (mut a, mut b, mut c, mut sum) = (1.0f32, 1.0f32, 1.0f32, 0.0f32);
    for _ in 0..OCTAVES {
        sum += b * phase(cos, c) * (-tau * a).exp();
        a *= MS_A;
        b *= MS_B;
        c *= MS_C;
    }
    // Powder: darkens where little cloud lies towards the viewer-side surface, strongest with
    // the sun behind the viewer; fades out looking into the sun (silver lining stays).
    let powder = 1.0 - (-sigma * 120.0).exp();
    let away = saturate(0.5 - 0.5 * cos);
    sum * lerp(1.0, lerp(0.55, 1.0, powder), away)
}

/// Diffuse transmittance through optical depth `tau` of strongly forward-scattering cloud
/// (two-stream, g ~ 0.85): `1 / (1 + 0.75 (1 - g) tau)`. Far slower than Beer's law: a 1 km
/// deep cumulus still lets a fifth of the skylight reach its base.
pub fn diffuse_t(tau: f32) -> f32 {
    1.0 / (1.0 + 0.11 * tau.max(0.0))
}

/// Ambient in-scattering radiance for a sample with `tau_up` of cloud above it and
/// `tau_down` below: tops see the sky, bases the lit ground, both through the cloud.
pub fn ambient(l: &CloudLight, tau_up: f32, tau_down: f32) -> Vec3 {
    l.sky * diffuse_t(tau_up) + l.ground * (0.6 * diffuse_t(tau_down))
}

/// Distances (m, midpoints) and lengths of the light-march steps. The last two are coarse.
pub const LIGHT_STEPS: [(f32, f32); 8] =
    [(25.0, 50.0), (80.0, 60.0), (160.0, 100.0), (290.0, 160.0), (500.0, 260.0), (860.0, 460.0), (1_900.0, 1_600.0), (4_200.0, 3_000.0)];

/// Planet frame of a marched view: the camera at altitude `alt0` above the datum, at render
/// position `origin`; the planet's centre is straight below the camera.
#[derive(Clone, Copy, Debug)]
pub struct Planet {
    pub radius_m: f64,
}

impl Planet {
    /// Altitude (m) of a point offset `r` (render frame) from a camera at altitude `alt0`.
    /// Stable in f32 too: `(|v|^2 - R^2) / (|v| + R)` without forming `R^2`.
    pub fn altitude(&self, alt0: f64, r: DVec3) -> f64 {
        let y = alt0 + r.y;
        let h2 = r.x * r.x + r.z * r.z;
        let rr = self.radius_m;
        let num = h2 + y * y + 2.0 * y * rr;
        let len = (h2 + (rr + y) * (rr + y)).sqrt();
        num / (len + rr)
    }

    /// Distances along unit `d` from altitude `alt0` to the sphere at altitude `h`: the two
    /// roots `(near, far)` of `|o + d t - C| = R + h`, or `None` when the ray misses it.
    pub fn sphere(&self, alt0: f64, d: DVec3, h: f64) -> Option<(f64, f64)> {
        let ry = self.radius_m + alt0;
        let b = d.y * ry;
        // c = Ry^2 - Rs^2 without cancellation.
        let c = (alt0 - h) * (2.0 * self.radius_m + alt0 + h);
        let disc = b * b - c;
        if disc < 0.0 {
            return None;
        }
        let s = disc.sqrt();
        // Citardauq: the stable pair of roots of t^2 + 2 b t + c = 0.
        let q = -b - s.copysign(b);
        let (t0, t1) = if q.abs() < 1e-12 { (-s, s) } else { (q, c / q) };
        Some((t0.min(t1), t0.max(t1)))
    }

    /// The part of the ray inside the shell between altitudes `lo` and `hi`, clipped to
    /// `[0, t_max]`. The first segment only (a ray leaving the slab downwards hits the ground).
    pub fn shell(&self, alt0: f64, d: DVec3, lo: f64, hi: f64, t_max: f64) -> Option<(f64, f64)> {
        let (t0, t1) = if alt0 < lo {
            // Below the slab: leave the inner sphere, then the outer one.
            let (_, a) = self.sphere(alt0, d, lo)?;
            let (_, b) = self.sphere(alt0, d, hi)?;
            (a, b)
        } else if alt0 <= hi {
            // Inside: until the outer sphere or, looking down, the inner one.
            let (_, out) = self.sphere(alt0, d, hi)?;
            let down = self.sphere(alt0, d, lo).map(|(n, _)| n).filter(|&n| n > 0.0);
            (0.0, down.map_or(out, |n| n.min(out)))
        } else {
            // Above: into the outer sphere, out at the inner one or the far side.
            let (n, f) = self.sphere(alt0, d, hi)?;
            if f <= 0.0 {
                return None;
            }
            let inner = self.sphere(alt0, d, lo).map(|(n, _)| n).filter(|&x| x > 0.0);
            (n.max(0.0), inner.map_or(f, |x| x.min(f)))
        };
        let (t0, t1) = (t0.max(0.0), t1.min(t_max));
        (t1 > t0).then_some((t0, t1))
    }
}

/// Optical depth towards the sun from render point `p` (camera-relative `r`). `jitter` (0..1)
/// scales the step distances by 0.7..1.3: fixed distances quantise the sun's optical depth
/// over smooth cloud tops into contour rings.
#[allow(clippy::too_many_arguments)]
pub fn light_depth(f: &CloudField<'_>, planet: &Planet, cam: DVec3, alt0: f64, r: DVec3, sun: Vec3, near: bool, jitter: f32) -> f32 {
    let mut tau = 0.0f32;
    let sd = sun.as_dvec3();
    let k = 0.7 + 0.6 * jitter;
    for (i, &(d, len)) in LIGHT_STEPS.iter().enumerate() {
        let (d, len) = (d * k, len * k);
        let q = r + sd * d as f64;
        let h = planet.altitude(alt0, q) as f32;
        let lod = if i < 4 && near { Lod::FULL } else { Lod::COARSE };
        let s = f.sample(cam.x + q.x, cam.z + q.z, h, lod);
        tau += s.sigma_t * len;
    }
    tau
}

/// The march's stepping rule (`clouds.slang` `cl_march` mirrors it): strides of
/// [`STRIDE`] steps through clear air, [`FINE`] steps inside a cloud's envelope, and on a stride
/// that lands inside an envelope, a back-up to the clear sample just before that stride, so a
/// cloud's lit skin is never skipped.
///
/// Backing up happens only right after a stride. Backing up to an older clear sample (from
/// before the previous cloud) re-walks that cloud and can loop until the step budget is gone:
/// the rays where the gap between two clouds was exactly the fine countdown long ended early,
/// leaving see-through hairlines along cloud undersides (found in the scattered previews).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stepper {
    /// Where the next sample is.
    pub t: f32,
    fine_left: u32,
    t_clear: f32,
    strided: bool,
}

impl Stepper {
    pub fn new(t: f32) -> Self {
        Self { t, fine_left: 0, t_clear: t, strided: false }
    }

    /// One visit at `self.t`, where the envelope is `empty` or not and the step is `dt`.
    /// Returns the step to integrate over when the caller should sample here (then add it to
    /// `t`); otherwise `t` has moved on already.
    pub fn visit(&mut self, empty: bool, dt: f32) -> Option<f32> {
        if empty {
            self.t_clear = self.t;
            if self.fine_left > 0 {
                self.fine_left -= 1;
                self.t += dt * FINE;
                self.strided = false;
            } else {
                self.t += dt * STRIDE;
                self.strided = true;
            }
            return None;
        }
        if self.strided {
            self.strided = false;
            self.fine_left = REFINE;
            self.t = self.t_clear + dt * FINE;
            return None;
        }
        self.fine_left = REFINE;
        Some(dt * FINE)
    }
}

/// Marches one view ray from render position `cam` (altitude `alt0` above the datum) along
/// unit `dir`, up to `t_max` (scene depth; f64::INFINITY for sky). `jitter` in 0..1 offsets
/// the first step.
#[allow(clippy::too_many_arguments)]
pub fn march(f: &CloudField<'_>, planet: &Planet, cam: DVec3, alt0: f64, dir: Vec3, light: &CloudLight, cfg: &MarchCfg, jitter: f32, t_max: f64) -> MarchOut {
    let none = MarchOut { radiance: Vec3::ZERO, transmittance: 1.0, depth_m: 0.0 };
    let (lo, hi) = f.bounds();
    if hi <= lo {
        return none;
    }
    let d = dir.as_dvec3();
    // Clouds stop at the ground (rain shafts reach it).
    let t_ground = planet.sphere(alt0, d, f.ground_m as f64).map(|(n, _)| n).filter(|&n| n > 0.0).unwrap_or(f64::INFINITY);
    let Some((t0, t1)) = planet.shell(alt0, d, lo as f64, hi as f64, t_max.min(cfg.max_dist_m as f64).min(t_ground)) else {
        return none;
    };
    let cos = dir.dot(light.sun_dir);
    let mut t = t0 as f32;
    let t1 = t1 as f32;
    let seg_min = (t1 - t) / cfg.max_steps as f32;
    let step = |t: f32| (t * cfg.step_frac).clamp(cfg.min_step_m, cfg.max_step_m).max(seg_min);
    // The jitter spans a whole clear-air stride (2 steps). Spanning one step leaves half of
    // each stride's phase unsampled, and where a ray first lands in a cloud then follows the
    // stride grid: brightness contours at the stride spacing on smooth tops (found on the
    // stratocumulus deck; no amount of samples averages a bias out).
    t += step(t) * STRIDE * jitter;
    let (mut tr, mut rad) = (1.0f32, Vec3::ZERO);
    let (mut depth_acc, mut w_acc) = (0.0f32, 0.0f32);
    let mut steps = 0;
    let mut st = Stepper::new(t);
    while st.t < t1 && steps < cfg.max_steps {
        steps += 1;
        let t = st.t;
        let dt = step(t);
        let r = d * t as f64;
        let h = planet.altitude(alt0, r) as f32;
        let (x, z) = (cam.x + r.x, cam.z + r.z);
        let Some(dt) = st.visit(f.envelope(x, z, h) <= 0.0, dt) else { continue };
        let s = f.sample(x, z, h, Lod::at(t));
        if s.sigma_t > 0.0 {
            // A fresh light jitter per step (golden-ratio sequence from the pixel's jitter).
            let lj = (jitter + steps as f32 * 0.618_034).fract();
            let tau = light_depth(f, planet, cam, alt0, r, light.sun_dir, t < 20_000.0, lj);
            let scat = light.sun * sun_scatter(tau, cos, s.sigma_t) + ambient(light, s.tau_up, s.tau_down);
            let src = scat * (s.sigma_t * s.albedo);
            let ts = (-s.sigma_t * dt).exp();
            rad += tr * (src - src * ts) / s.sigma_t;
            let a = tr * (1.0 - ts);
            depth_acc += a * t;
            w_acc += a;
            tr *= ts;
            if tr < cfg.min_transmittance {
                break;
            }
        }
        st.t += dt;
    }
    MarchOut { radiance: rad, transmittance: tr, depth_m: if w_acc > 0.0 { depth_acc / w_acc } else { 0.0 } }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloudscape::{noise::NoiseBank, storm::storm_cells, weather::{CloudWeather, Preset}};
    use glam::DVec2;
    use std::sync::OnceLock;

    fn bank() -> &'static NoiseBank {
        static B: OnceLock<NoiseBank> = OnceLock::new();
        B.get_or_init(|| NoiseBank::bake_sized(0xC10D, 32, 16, 128))
    }

    const P: Planet = Planet { radius_m: crate::cloudscape::EARTH_RADIUS_M };

    #[test]
    fn phase_integrates_to_one() {
        for k in [1.0, 0.5, 0.25] {
            let n = 4000;
            let s: f32 = (0..n).map(|i| {
                let c = -1.0 + 2.0 * (i as f32 + 0.5) / n as f32;
                phase(c, k) * 2.0 * std::f32::consts::PI * (2.0 / n as f32)
            }).sum();
            assert!((s - 1.0).abs() < 0.02, "k {k}: {s}");
        }
        assert!(phase(1.0, 1.0) > 10.0 * phase(0.0, 1.0), "strong forward lobe");
    }

    #[test]
    fn multiple_scattering_keeps_thick_cloud_lit() {
        // Single scattering at tau 10 is ~0; the octaves keep a visible share.
        let single = hg(0.0, G_FWD) * (-10.0f32).exp();
        assert!(sun_scatter(10.0, 0.0, 0.05) > 50.0 * single);
        assert!(sun_scatter(0.5, 0.0, 0.05) > sun_scatter(5.0, 0.0, 0.05));
    }

    #[test]
    fn altitude_is_curved_and_stable() {
        assert!((P.altitude(1000.0, DVec3::ZERO) - 1000.0).abs() < 1e-6);
        // 100 km away on the level the ground has dropped ~786 m (R = 6360 km).
        let h = P.altitude(0.0, DVec3::new(100_000.0, 0.0, 0.0));
        assert!((h - 786.1).abs() < 1.0, "{h}");
        // f32 version of the same formula stays within a metre at 150 km.
        let r = DVec3::new(120_000.0, 2_000.0, -80_000.0);
        let ex = P.altitude(1500.0, r);
        let rf = r.as_vec3();
        let (y, rr) = (1500.0f32 + rf.y, P.radius_m as f32);
        let h2 = rf.x * rf.x + rf.z * rf.z;
        let f32v = (h2 + y * y + 2.0 * y * rr) / ((h2 + (rr + y) * (rr + y)).sqrt() + rr);
        assert!((f32v as f64 - ex).abs() < 1.0, "{f32v} vs {ex}");
    }

    #[test]
    fn shell_intervals() {
        // Looking straight up from the ground at a 1-3 km slab.
        let (a, b) = P.shell(0.0, DVec3::Y, 1000.0, 3000.0, 1e9).unwrap();
        assert!((a - 1000.0).abs() < 1e-3 && (b - 3000.0).abs() < 1e-3);
        // Inside the slab looking down: until its floor.
        let (a, b) = P.shell(2000.0, -DVec3::Y, 1000.0, 3000.0, 1e9).unwrap();
        assert!(a == 0.0 && (b - 1000.0).abs() < 1e-3);
        // From orbit looking down: enter at the top.
        let (a, b) = P.shell(400_000.0, -DVec3::Y, 1000.0, 3000.0, 1e9).unwrap();
        assert!((a - 397_000.0).abs() < 1e-2 && (b - 399_000.0).abs() < 1e-2);
        // Horizontal from the ground: the slab is far away (curvature) and long.
        let (a, b) = P.shell(0.0, DVec3::X, 1000.0, 3000.0, 1e9).unwrap();
        assert!(a > 100_000.0 && b > a, "{a} {b}");
        // Clipped by depth.
        assert!(P.shell(0.0, DVec3::Y, 1000.0, 3000.0, 500.0).is_none());
    }

    fn scene(p: Preset) -> (CloudField<'static>, CloudLight) {
        let w = CloudWeather::preset(p);
        let layers = w.layers();
        let storms = storm_cells(&w, &layers[0], DVec2::ZERO, 80_000.0, 0.0);
        let f = CloudField::new(bank(), layers, storms, 0.0, 0.0, w.seed);
        let light = CloudLight { sun_dir: Vec3::new(0.3, 0.8, -0.5).normalize(), sun: Vec3::splat(10.0), sky: Vec3::new(0.3, 0.45, 0.8), ground: Vec3::splat(0.2) };
        (f, light)
    }

    #[test]
    fn overcast_blocks_the_sky_and_clear_does_not() {
        let cfg = MarchCfg::high();
        let (f, l) = scene(Preset::Overcast);
        let mut sum = 0.0;
        for i in 0..16 {
            let d = Vec3::new((i as f32 * 0.37).sin() * 0.3, 1.0, (i as f32 * 0.71).cos() * 0.3).normalize();
            sum += march(&f, &P, DVec3::ZERO, 2.0, d, &l, &cfg, 0.5, f64::INFINITY).transmittance;
        }
        assert!(sum / 16.0 < 0.15, "overcast transmittance {}", sum / 16.0);
        let (f, l) = scene(Preset::Clear);
        let o = march(&f, &P, DVec3::ZERO, 2.0, Vec3::Y, &l, &cfg, 0.5, f64::INFINITY);
        assert_eq!(o.transmittance, 1.0);
    }

    #[test]
    fn step_size_barely_changes_the_result() {
        // Energy-conserving integration: the High tier and the reference agree on a deck.
        let (f, l) = scene(Preset::Overcast);
        let d = Vec3::new(0.2, 1.0, 0.1).normalize();
        let a = march(&f, &P, DVec3::ZERO, 2.0, d, &l, &MarchCfg::reference(), 0.5, f64::INFINITY);
        let b = march(&f, &P, DVec3::ZERO, 2.0, d, &l, &MarchCfg::high(), 0.5, f64::INFINITY);
        let rel = (a.radiance - b.radiance).length() / a.radiance.length().max(1e-3);
        assert!(rel < 0.2, "reference {:?} high {:?}", a.radiance, b.radiance);
        assert!((a.transmittance - b.transmittance).abs() < 0.1);
    }

    /// Runs the stepper along a ray with clouds on `intervals` (constant step `dt`) and returns
    /// the visited positions and the sampled ones.
    fn walk(intervals: &[(f32, f32)], dt: f32, start: f32, end: f32) -> (Vec<f32>, Vec<f32>) {
        let mut st = Stepper::new(start);
        let (mut visited, mut sampled) = (Vec::new(), Vec::new());
        while st.t < end && visited.len() < 10_000 {
            let t = st.t;
            visited.push(t);
            let empty = !intervals.iter().any(|&(a, b)| t >= a && t <= b);
            if let Some(d) = st.visit(empty, dt) {
                sampled.push(t);
                st.t += d;
            }
        }
        (visited, sampled)
    }

    #[test]
    fn stepper_never_rewinds_past_one_stride_and_samples_every_cloud() {
        let dt = 10.0;
        let fine = dt * FINE;
        // Gaps of every length around the fine countdown (REFINE fine steps), the case that
        // used to rewind to a clear sample from before the previous cloud.
        for gap_steps in 1..=(REFINE + 4) {
            for phase in 0..8 {
                let mut iv = Vec::new();
                let mut a = 100.0 + phase as f32 * 0.37;
                for _ in 0..6 {
                    iv.push((a, a + 35.0));
                    a += 35.0 + gap_steps as f32 * fine + 0.01;
                }
                let end = a + 200.0;
                let (visited, sampled) = walk(&iv, dt, 0.0, end);
                assert!(visited.len() < 2_000, "gap {gap_steps}: {} visits (rewind loop)", visited.len());
                let mut hi = f32::MIN;
                for &t in &visited {
                    assert!(t >= hi - dt * STRIDE - 1e-3, "gap {gap_steps}: rewound from {hi} to {t}");
                    hi = hi.max(t);
                }
                assert!(*visited.last().unwrap() >= iv.last().unwrap().1, "gap {gap_steps}: stopped early");
                for &(a, b) in &iv {
                    assert!(sampled.iter().any(|&t| t >= a && t <= b), "gap {gap_steps}: cloud {a}..{b} never sampled");
                    // The first sample lands within one fine step of the cloud's near edge.
                    let first = sampled.iter().copied().find(|&t| t >= a).unwrap();
                    assert!(first - a <= fine + 1e-3, "gap {gap_steps}: skin skipped ({a} -> {first})");
                }
            }
        }
    }

    /// Rays a hair apart must not differ wildly in transmittance inside a cloud.
    #[test]
    fn no_hairline_leaks() {
        let (f, l) = scene(Preset::Scattered);
        let alt0 = (f.frames[0].layer.base_m - 250.0) as f64;
        let cfg = MarchCfg::high();
        let mut leaks = 0;
        let mut checked = 0;
        for i in 0..900u64 {
            let yaw = crate::hash01(i, 7) * std::f32::consts::TAU;
            let pitch = (2.0 + crate::hash01(i, 8) * 30.0).to_radians();
            let ray = |dp: f32| {
                let p = pitch + dp;
                Vec3::new(yaw.sin() * p.cos(), p.sin(), -yaw.cos() * p.cos())
            };
            let t = [-1.0e-4f32, 0.0, 1.0e-4].map(|dp| march(&f, &P, DVec3::ZERO, alt0, ray(dp), &l, &cfg, 0.5, f64::INFINITY).transmittance);
            if t[0] < 0.3 && t[2] < 0.3 {
                checked += 1;
                if t[1] > t[0] + 0.4 && t[1] > t[2] + 0.4 {
                    leaks += 1;
                }
            }
        }
        assert!(checked > 100, "only {checked} rays inside cloud");
        assert_eq!(leaks, 0, "{leaks} of {checked} rays leak through cloud their neighbours stop");
    }

    #[test]
    fn deterministic() {
        let (f, l) = scene(Preset::Scattered);
        let d = Vec3::new(0.5, 0.4, -0.2).normalize();
        let cfg = MarchCfg::high();
        assert_eq!(march(&f, &P, DVec3::ZERO, 2.0, d, &l, &cfg, 0.3, 1e9), march(&f, &P, DVec3::ZERO, 2.0, d, &l, &cfg, 0.3, 1e9));
    }
}
