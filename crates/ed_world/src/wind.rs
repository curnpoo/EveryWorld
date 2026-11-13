//! Wind field (trees G4b): one weather sample that moves every tree, and later every blade of
//! grass, from the same source. A pure function of (position, time): no state, no wall clock,
//! so the sim and the renderer agree wherever they evaluate it.
//!
//! The field is a steady breeze plus a seeded gust band travelling downwind: gusts are
//! bands across the wind direction, a few hundred metres apart, that sweep over a forest
//! so neighbouring trees bend one after the other. The renderer gets the field as eight
//! floats ([`WindField::gpu_words`]) and evaluates the same band in `common.slang`
//! (`wind_gust`), relative to the camera; the procedural weather sim (`ed_climate`) will
//! later set `base` per region and time.
//!
//! Positions are metres in a local horizontal frame (x east, y north); the renderer's
//! frame is (x east, z -north), see [`WindField::gpu_words`].

use glam::{DVec2, Vec3};

/// What the weather says at one point and time.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WeatherSample {
    /// Mean wind (m/s; x east, y north, z up).
    pub wind: Vec3,
    /// Gust factor `0..1` on top of the mean wind (1: the crest of a gust band).
    pub gust: f32,
    /// Small-scale turbulence `0..1` (leaf flutter, needle shimmer).
    pub turbulence: f32,
}

/// A steady breeze plus a travelling gust band. `Default` is the static default breeze.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindField {
    /// Mean wind (m/s; x east, y north).
    pub base: DVec2,
    /// Gust band spacing along the wind (m).
    pub gust_spacing: f64,
    /// Gust strength `0..1` (fraction of the mean wind added at a crest).
    pub gust_strength: f32,
    pub turbulence: f32,
    pub seed: u64,
}

impl Default for WindField {
    fn default() -> Self {
        // A light breeze from the south-west (~3.5 m/s, Beaufort 3: leaves in constant motion).
        WindField { base: DVec2::new(2.5, 2.5), gust_spacing: 260.0, gust_strength: 0.6, turbulence: 0.35, seed: 0x57_1D }
    }
}

/// Gust band profile at band phase `x` (cycles): sharp crests, broad lulls, `0..1`.
pub fn gust_profile(x: f64) -> f32 {
    let s = 0.5 + 0.5 * (x * std::f64::consts::TAU).sin();
    (s * s * s) as f32
}

impl WindField {
    /// Unit wind direction (x east, y north); north when calm.
    pub fn direction(&self) -> DVec2 {
        self.base.try_normalize().unwrap_or(DVec2::Y)
    }

    /// Gust band speed (m/s): the bands ride the wind.
    pub fn gust_speed(&self) -> f64 {
        self.base.length()
    }

    /// Seeded phase offset of the band pattern (cycles).
    fn seed_phase(&self) -> f64 {
        crate::hash01(self.seed, 0x6057) as f64
    }

    /// Band phase (cycles) at `p` (m) and time `t` (s).
    pub fn band_phase(&self, p: DVec2, t: f64) -> f64 {
        (p.dot(self.direction()) - self.gust_speed() * t) / self.gust_spacing + self.seed_phase()
    }

    pub fn sample(&self, p: DVec2, t: f64) -> WeatherSample {
        let gust = gust_profile(self.band_phase(p, t)) * self.gust_strength;
        WeatherSample { wind: Vec3::new(self.base.x as f32, self.base.y as f32, 0.0), gust, turbulence: self.turbulence * (0.6 + 0.4 * gust) }
    }

    /// The field for the GPU (`common.slang` `WindData`, two float4), relative to the camera
    /// at `camera` (m, same frame as [`WindField::sample`]) and time `t`:
    /// `[wind east, wind up, wind -north, time mod 1 h]`,
    /// `[band phase at the camera (fraction), spacing (m), gust strength, turbulence]`.
    /// The renderer adds `on` and the rest distance itself.
    pub fn gpu_words(&self, camera: DVec2, t: f64) -> [f32; 8] {
        let ph = self.band_phase(camera, t);
        [
            self.base.x as f32,
            0.0,
            -self.base.y as f32,
            t.rem_euclid(3600.0) as f32,
            ph.rem_euclid(1.0) as f32,
            self.gust_spacing as f32,
            self.gust_strength,
            self.turbulence,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_and_travelling() {
        let w = WindField::default();
        let p = DVec2::new(120.0, -40.0);
        assert_eq!(w.sample(p, 3.0), w.sample(p, 3.0));
        // The band moves downwind at the wind speed: the same gust a metre-per-second later
        // is that many metres downwind.
        let d = w.direction();
        for t in [0.0, 1.7, 33.0] {
            let a = w.sample(p, t).gust;
            let b = w.sample(p + d * w.gust_speed() * 2.0, t + 2.0).gust;
            assert!((a - b).abs() < 1e-5, "{a} {b}");
        }
        // Gusts vary in space (bands, not a uniform pulse).
        let g: Vec<f32> = (0..40).map(|k| w.sample(p + d * k as f64 * 20.0, 0.0).gust).collect();
        let (lo, hi) = g.iter().fold((1.0f32, 0.0f32), |(lo, hi), &x| (lo.min(x), hi.max(x)));
        assert!(hi - lo > 0.4, "{lo} {hi}");
    }

    #[test]
    fn gpu_phase_matches_cpu() {
        let w = WindField::default();
        let cam = DVec2::new(5000.0, 9000.0);
        let t = 77.25;
        let g = w.gpu_words(cam, t);
        // The shader: phase = g[4] + dot(rel, dir) / spacing.
        let rel = DVec2::new(30.0, -12.0);
        let shader = g[4] as f64 + rel.dot(w.direction()) / g[5] as f64;
        let cpu = w.band_phase(cam + rel, t);
        assert!((gust_profile(shader) - gust_profile(cpu)).abs() < 1e-4);
    }
}
