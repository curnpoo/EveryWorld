//! Thunderstorm cells: where the cumulonimbus are, and what stage each one is in. A pure
//! function of (seed, weather, time): the slots sit on a grid that drifts with the steering
//! (mid-level) wind, so a storm 60 km out is the same storm when you fly to it, and it keeps
//! coming towards you.
//!
//! Each slot runs a life cycle of [`LIFE_S`] and then hosts a new storm at a new jitter:
//! - **growth** (0..0.3): a towering cumulus whose top climbs to the tropopause,
//! - **mature** (0.3..0.7): full height, the anvil spreads downwind on the jet, rain,
//! - **decay** (0.7..1): the core thins and the rain stops; the detached anvil lingers, then fades.

use super::layer::CloudLayer;
use super::weather::CloudWeather;
use super::{lerp, smoothstep};
use crate::hash01;
use glam::{DVec2, Vec2};

/// Slot spacing (m). One storm at most per slot.
pub const SLOT_M: f64 = 24_000.0;
/// Life cycle of one storm (s): about 100 minutes, a single-cell to small multicell storm.
pub const LIFE_S: f64 = 6_000.0;
/// Most cells the renderer takes (nearest first).
pub const MAX_STORMS: usize = 16;

/// One cumulonimbus at one time. Positions are render-frame metres (x east, z -north).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StormCell {
    pub x: f64,
    pub z: f64,
    /// Updraft core radius at mid height (m).
    pub radius_m: f32,
    /// Cloud base (m), the low layer's.
    pub base_m: f32,
    /// Current top (m): climbs during growth.
    pub top_m: f32,
    /// Core strength 0..1 (thins during decay).
    pub core: f32,
    /// Anvil extent 0..1.
    pub anvil: f32,
    /// Rain 0..1 under the core.
    pub rain: f32,
    /// Unit direction the anvil spreads (render x, z): the upper wind.
    pub anvil_dir: Vec2,
    /// Anvil reach downwind at full extent (m).
    pub anvil_len_m: f32,
    /// Tower lean (render x, z per metre of height).
    pub lean: Vec2,
    /// Per-storm hash, for noise offsets.
    pub id: u32,
}

/// Life-cycle stage weights at phase `s` (0..1): (top fraction, core, anvil, rain).
pub fn life(s: f32) -> (f32, f32, f32, f32) {
    let top = lerp(0.35, 1.0, smoothstep(0.0, 0.3, s));
    let core = smoothstep(0.0, 0.08, s) * (1.0 - smoothstep(0.7, 0.92, s));
    let anvil = smoothstep(0.28, 0.5, s) * (1.0 - smoothstep(0.88, 1.0, s));
    let rain = smoothstep(0.25, 0.4, s) * (1.0 - smoothstep(0.66, 0.8, s));
    (top, core, anvil, rain)
}

fn en_to_render(v: Vec2) -> Vec2 {
    Vec2::new(v.x, -v.y)
}

/// The storms within `range_m` of `center` (render x, z) at time `t_s`, nearest first, at most
/// [`MAX_STORMS`]. Empty when the weather has no storm chance.
pub fn storm_cells(w: &CloudWeather, low: &CloudLayer, center: DVec2, range_m: f64, t_s: f64) -> Vec<StormCell> {
    let chance = w.storm_chance();
    if chance <= 0.0 || !low.active() {
        return Vec::new();
    }
    let steer = en_to_render(w.wind_mid).as_dvec2();
    let drift = steer * t_s;
    // The slot grid lives in the drifting frame.
    let c = center - drift;
    let r_slots = (range_m / SLOT_M).ceil() as i64 + 1;
    let (ci, cj) = ((c.x / SLOT_M).floor() as i64, (c.y / SLOT_M).floor() as i64);
    let high = en_to_render(w.wind_high);
    let anvil_dir = high.try_normalize().unwrap_or(Vec2::X);
    let anvil_len = (8_000.0 + high.length() * 900.0).min(40_000.0);
    let lean = en_to_render(low.shear) * 0.6;
    let mut out = Vec::new();
    for j in cj - r_slots..=cj + r_slots {
        for i in ci - r_slots..=ci + r_slots {
            let slot = ((i as u64) << 32) ^ (j as u64 & 0xffff_ffff) ^ w.seed.rotate_left(17);
            let ph0 = hash01(slot, 1) as f64;
            let life_t = t_s / LIFE_S + ph0;
            let cycle = life_t.floor() as u64;
            let s = (life_t - life_t.floor()) as f32;
            let key = slot ^ cycle.wrapping_mul(0x9E37_79B9_7F4A_7C15);
            if hash01(key, 2) >= chance {
                continue;
            }
            let jx = (hash01(key, 3) as f64 - 0.5) * 0.7;
            let jz = (hash01(key, 4) as f64 - 0.5) * 0.7;
            let p = DVec2::new((i as f64 + 0.5 + jx) * SLOT_M, (j as f64 + 0.5 + jz) * SLOT_M) + drift;
            if (p - center).length() > range_m {
                continue;
            }
            let (top_f, core, anvil, rain) = life(s);
            if core <= 0.0 && anvil <= 0.0 {
                continue;
            }
            let full_top = lerp(0.85, 1.0, hash01(key, 5)) * w.tropopause_m;
            let base = low.base_m;
            out.push(StormCell {
                x: p.x,
                z: p.y,
                radius_m: lerp(2_500.0, 5_000.0, hash01(key, 6)),
                base_m: base,
                top_m: base + (full_top - base) * top_f,
                core,
                anvil,
                rain: rain * w.precipitation.max(0.3),
                anvil_dir,
                anvil_len_m: anvil_len,
                lean,
                id: (key ^ (key >> 32)) as u32,
            });
        }
    }
    out.sort_by(|a, b| {
        let da = (DVec2::new(a.x, a.z) - center).length_squared();
        let db = (DVec2::new(b.x, b.z) - center).length_squared();
        da.total_cmp(&db).then(a.id.cmp(&b.id))
    });
    out.truncate(MAX_STORMS);
    out
}

impl StormCell {
    /// How much of the column at (`x`, `z`) and height `h` this storm fills, 0..1, and the
    /// column's height fraction for ambient light.
    pub fn envelope(&self, x: f64, z: f64, h: f32) -> (f32, f32) {
        let dz = h - self.base_m;
        let height = (self.top_m - self.base_m).max(1.0);
        if dz < 0.0 || dz > height {
            return (0.0, 0.0);
        }
        let hf = dz / height;
        let rel = Vec2::new((x - self.x) as f32, (z - self.z) as f32) - self.lean * dz;
        // Core: a slightly narrower base, full width in the middle, a rounded overshooting dome.
        let r = self.radius_m * (0.75 + 0.25 * smoothstep(0.0, 0.3, hf)) * (1.0 - 0.35 * smoothstep(0.85, 1.0, hf));
        let core = (1.0 - smoothstep(r * 0.45, r, rel.length())) * self.core * (1.0 - smoothstep(0.93, 1.0, hf));
        // Anvil: a sheet in the top fifth, flat-topped at the tropopause, spreading downwind.
        let band = smoothstep(0.7, 0.84, hf) * (1.0 - smoothstep(0.94, 1.0, hf));
        let along = rel.dot(self.anvil_dir);
        let across = rel.perp_dot(self.anvil_dir);
        let reach = self.anvil_len_m * self.anvil;
        let a = Vec2::new((along - reach * 0.4) / (reach * 0.6 + self.radius_m), across / (self.radius_m * (1.1 + 1.6 * self.anvil)));
        let anvil = band * (1.0 - smoothstep(0.55, 1.0, a.length())) * self.anvil;
        (core.max(anvil), hf)
    }

    /// Rain-shaft strength 0..1 under the core at (`x`, `z`).
    pub fn rain_at(&self, x: f64, z: f64) -> f32 {
        if self.rain <= 0.0 {
            return 0.0;
        }
        let d = Vec2::new((x - self.x) as f32, (z - self.z) as f32).length();
        self.rain * (1.0 - smoothstep(self.radius_m * 0.3, self.radius_m * 0.8, d))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloudscape::weather::Preset;

    fn storms(t: f64, c: DVec2) -> Vec<StormCell> {
        let w = CloudWeather::preset(Preset::Thunderstorm);
        storm_cells(&w, &w.layers()[0], c, 120_000.0, t)
    }

    #[test]
    fn none_in_fair_weather() {
        let w = CloudWeather::preset(Preset::Scattered);
        assert!(storm_cells(&w, &w.layers()[0], DVec2::ZERO, 200_000.0, 0.0).is_empty());
    }

    #[test]
    fn deterministic_nearest_first_and_capped() {
        let a = storms(1234.0, DVec2::ZERO);
        assert!(!a.is_empty());
        assert_eq!(a, storms(1234.0, DVec2::ZERO));
        assert!(a.len() <= MAX_STORMS);
        let d: Vec<f64> = a.iter().map(|c| DVec2::new(c.x, c.z).length()).collect();
        assert!(d.windows(2).all(|p| p[0] <= p[1]));
    }

    #[test]
    fn storms_drift_with_the_steering_wind() {
        let w = CloudWeather::preset(Preset::Thunderstorm);
        let steer = DVec2::new(w.wind_mid.x as f64, -w.wind_mid.y as f64);
        let a = storms(1000.0, DVec2::ZERO);
        let b = storms(1060.0, DVec2::ZERO);
        // Same storm id a minute later, moved by the wind.
        let s = a[0];
        let m = b.iter().find(|c| c.id == s.id).expect("still there a minute later");
        let moved = DVec2::new(m.x - s.x, m.z - s.z);
        assert!((moved - steer * 60.0).length() < 1e-3, "{moved:?}");
    }

    #[test]
    fn life_cycle_shapes_the_storm() {
        let (t0, c0, a0, r0) = life(0.05);
        let (t1, c1, a1, r1) = life(0.5);
        let (_, c2, a2, r2) = life(0.85);
        assert!(t0 < 0.6 && t1 == 1.0);
        assert!(a0 == 0.0 && a1 > 0.8 && a2 > 0.3, "anvil lingers after the core");
        assert!(r0 == 0.0 && r1 == 1.0 && r2 == 0.0);
        assert!(c0 > 0.0 && c1 == 1.0 && c2 < 0.5);
    }

    #[test]
    fn envelope_has_core_and_downwind_anvil() {
        let s = StormCell {
            x: 0.0,
            z: 0.0,
            radius_m: 4_000.0,
            base_m: 1_000.0,
            top_m: 11_000.0,
            core: 1.0,
            anvil: 1.0,
            rain: 1.0,
            anvil_dir: Vec2::X,
            anvil_len_m: 25_000.0,
            lean: Vec2::ZERO,
            id: 1,
        };
        assert!(s.envelope(0.0, 0.0, 5_000.0).0 > 0.99);
        assert_eq!(s.envelope(0.0, 0.0, 900.0).0, 0.0);
        // Anvil: far downwind near the top, nothing upwind or lower down.
        assert!(s.envelope(15_000.0, 0.0, 9_800.0).0 > 0.5);
        assert!(s.envelope(-15_000.0, 0.0, 9_800.0).0 < 0.01);
        assert!(s.envelope(15_000.0, 0.0, 5_000.0).0 < 0.01);
        assert!(s.rain_at(0.0, 0.0) > 0.99 && s.rain_at(5_000.0, 0.0) == 0.0);
    }
}
