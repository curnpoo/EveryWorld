//! The weather a sky is made from. [`CloudWeather`] is the contract with the climate sim
//! (`ed_climate`, O3-W4 layer 4): physical quantities, not cloud knobs, so the sim never has
//! to know how clouds are drawn. [`CloudWeather::layers`] is the meteorology that turns them
//! into cloud layers:
//!
//! - **Cloud base** is the lifting condensation level, about 125 m per degree of dew-point
//!   spread above the ground (Espy's rule).
//! - **Convective depth** grows with instability: fair-weather cumulus at low instability,
//!   congestus in the middle, cumulonimbus to the tropopause at the top.
//! - **Stable and humid** makes a stratocumulus deck instead of heaps.
//! - **Frontal lift** makes the mid and high sheets (altostratus, cirrostratus) and the steady
//!   rain of a warm front.
//! - **Winds aloft** move each layer; the difference between them is the shear that leans
//!   towers and stretches cirrus.
//!
//! [`CloudWeather::lerp`] blends two weathers, so a front passing or a storm building is a
//! continuous change, never a switch.

use super::layer::{CloudLayer, LayerKind};
use super::{lerp, saturate, smoothstep};
use glam::Vec2;

/// Deepest ordinary convection (m): cumulus congestus. Deeper is a storm cell.
pub const CONGESTUS_MAX_M: f32 = 6_500.0;

/// Air-mass state at one place and time. Winds are m/s as `(east, north)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CloudWeather {
    /// Relative humidity of the boundary layer (0..1, ~0-3 km).
    pub humidity_low: f32,
    /// Relative humidity of the mid troposphere (0..1, ~3-7 km).
    pub humidity_mid: f32,
    /// Relative humidity of the upper troposphere (0..1, ~7-12 km).
    pub humidity_high: f32,
    /// Convective instability, 0 (stable) .. 1 (deep convection; a CAPE proxy).
    pub instability: f32,
    /// Surface temperature minus dew point (°C). Sets the cloud base.
    pub dewpoint_spread_c: f32,
    /// Ground altitude under the sky (m above sea level).
    pub ground_m: f32,
    /// Tropopause height (m): the ceiling of every cloud and where anvils spread.
    pub tropopause_m: f32,
    /// Large-scale ascent, 0..1 (warm and cold fronts, lows): stratiform sheets and steady rain.
    pub frontal_lift: f32,
    /// Boundary-layer wind (m/s; east, north).
    pub wind_low: Vec2,
    /// Mid-level (steering) wind: storm cells and altocumulus move with it.
    pub wind_mid: Vec2,
    /// Upper wind (jet level): cirrus and anvils.
    pub wind_high: Vec2,
    /// Precipitation intensity, 0..1 (drizzle .. downpour).
    pub precipitation: f32,
    /// World seed: which clouds, where.
    pub seed: u64,
}

impl Default for CloudWeather {
    fn default() -> Self {
        Self::preset(Preset::Scattered)
    }
}

/// Named weathers standing in for the sim (and for look-dev and tests).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    Clear,
    /// Fair-weather cumulus humilis: small, flat, far apart.
    Fair,
    /// Cumulus mediocris with some altocumulus and cirrus: a classic summer afternoon.
    Scattered,
    /// Cumulus congestus: thick, tall towers, darker bases.
    Towering,
    /// Cumulonimbus: towers to the tropopause with anvils, rain shafts, dark bellies.
    Thunderstorm,
    /// A stratocumulus deck: lumpy grey overcast.
    Overcast,
    /// A warm front: altostratus over nimbostratus, steady rain, cirrostratus veil.
    Frontal,
    /// Only high cirrus: light, wispy streaks.
    Cirrus,
    /// Every layer at once: cumulus, altocumulus, cirrus.
    Layered,
}

impl Preset {
    pub const ALL: [Preset; 9] = [
        Preset::Clear,
        Preset::Fair,
        Preset::Scattered,
        Preset::Towering,
        Preset::Thunderstorm,
        Preset::Overcast,
        Preset::Frontal,
        Preset::Cirrus,
        Preset::Layered,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Preset::Clear => "clear",
            Preset::Fair => "fair",
            Preset::Scattered => "scattered",
            Preset::Towering => "towering",
            Preset::Thunderstorm => "thunderstorm",
            Preset::Overcast => "overcast",
            Preset::Frontal => "frontal",
            Preset::Cirrus => "cirrus",
            Preset::Layered => "layered",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.name() == s)
    }
}

impl CloudWeather {
    pub fn preset(p: Preset) -> Self {
        // A south-westerly flow that veers and strengthens with height (warm-air advection).
        let base = Self {
            humidity_low: 0.0,
            humidity_mid: 0.0,
            humidity_high: 0.0,
            instability: 0.0,
            dewpoint_spread_c: 10.0,
            ground_m: 0.0,
            tropopause_m: 11_500.0,
            frontal_lift: 0.0,
            wind_low: Vec2::new(3.0, 2.0),
            wind_mid: Vec2::new(8.0, 3.0),
            wind_high: Vec2::new(22.0, 4.0),
            precipitation: 0.0,
            seed: 0xC10D,
        };
        match p {
            Preset::Clear => Self { humidity_low: 0.35, humidity_mid: 0.2, humidity_high: 0.2, ..base },
            Preset::Fair => Self { humidity_low: 0.68, humidity_mid: 0.3, humidity_high: 0.35, instability: 0.12, dewpoint_spread_c: 9.0, ..base },
            Preset::Scattered => Self { humidity_low: 0.76, humidity_mid: 0.62, humidity_high: 0.66, instability: 0.32, dewpoint_spread_c: 11.0, ..base },
            Preset::Towering => Self { humidity_low: 0.84, humidity_mid: 0.55, humidity_high: 0.5, instability: 0.58, dewpoint_spread_c: 10.0, wind_mid: Vec2::new(6.0, 2.0), ..base },
            Preset::Thunderstorm => Self {
                humidity_low: 0.9,
                humidity_mid: 0.66,
                humidity_high: 0.6,
                instability: 0.92,
                dewpoint_spread_c: 8.0,
                wind_mid: Vec2::new(9.0, 3.0),
                wind_high: Vec2::new(26.0, 6.0),
                precipitation: 0.85,
                ..base
            },
            Preset::Overcast => Self { humidity_low: 0.95, humidity_mid: 0.5, humidity_high: 0.3, instability: 0.04, dewpoint_spread_c: 6.0, frontal_lift: 0.2, ..base },
            Preset::Frontal => Self {
                humidity_low: 0.97,
                humidity_mid: 0.95,
                humidity_high: 0.85,
                instability: 0.05,
                dewpoint_spread_c: 3.0,
                frontal_lift: 0.9,
                precipitation: 0.6,
                wind_low: Vec2::new(6.0, 5.0),
                wind_mid: Vec2::new(14.0, 8.0),
                ..base
            },
            Preset::Cirrus => Self { humidity_low: 0.4, humidity_mid: 0.3, humidity_high: 0.8, ..base },
            Preset::Layered => Self { humidity_low: 0.8, humidity_mid: 0.78, humidity_high: 0.74, instability: 0.42, dewpoint_spread_c: 10.0, frontal_lift: 0.1, ..base },
        }
    }

    /// Blend towards `o` by `t` (0: self, 1: `o`). Fronts arriving and storms building are this.
    pub fn lerp(&self, o: &Self, t: f32) -> Self {
        let t = saturate(t);
        let v = |a: Vec2, b: Vec2| a.lerp(b, t);
        Self {
            humidity_low: lerp(self.humidity_low, o.humidity_low, t),
            humidity_mid: lerp(self.humidity_mid, o.humidity_mid, t),
            humidity_high: lerp(self.humidity_high, o.humidity_high, t),
            instability: lerp(self.instability, o.instability, t),
            dewpoint_spread_c: lerp(self.dewpoint_spread_c, o.dewpoint_spread_c, t),
            ground_m: lerp(self.ground_m, o.ground_m, t),
            tropopause_m: lerp(self.tropopause_m, o.tropopause_m, t),
            frontal_lift: lerp(self.frontal_lift, o.frontal_lift, t),
            wind_low: v(self.wind_low, o.wind_low),
            wind_mid: v(self.wind_mid, o.wind_mid),
            wind_high: v(self.wind_high, o.wind_high),
            precipitation: lerp(self.precipitation, o.precipitation, t),
            seed: if t < 0.5 { self.seed } else { o.seed },
        }
    }

    /// Takes the low-level wind from the trees' and grass's wind field, so clouds, trees and
    /// grass move with one weather (O3-W1). Upper winds keep their direction change with
    /// height and scale with the new surface speed.
    pub fn with_surface_wind(mut self, wind_en: Vec2) -> Self {
        let old = self.wind_low.length().max(0.5);
        let k = (wind_en.length() / old).clamp(0.0, 6.0);
        let rot = |v: Vec2| {
            // Keep each level's veer relative to the boundary layer.
            let a = self.wind_low.to_angle();
            let b = wind_en.to_angle();
            Vec2::from_angle(b - a).rotate(v)
        };
        self.wind_mid = rot(self.wind_mid) * k.max(0.5);
        self.wind_high = rot(self.wind_high) * k.max(0.5);
        self.wind_low = wind_en;
        self
    }

    /// Lifting condensation level above sea level (m): the flat base of convective cloud.
    pub fn lcl_m(&self) -> f32 {
        self.ground_m + (self.dewpoint_spread_c.max(0.0) * 125.0).clamp(250.0, 3500.0)
    }

    /// Chance that a 20 km storm cell slot holds a thunderstorm at any time (0..1).
    pub fn storm_chance(&self) -> f32 {
        smoothstep(0.62, 0.95, self.instability) * smoothstep(0.7, 0.88, self.humidity_low)
    }

    /// The three cloud layers of this weather (low, mid, high).
    pub fn layers(&self) -> [CloudLayer; 3] {
        [self.low_layer(), self.mid_layer(), self.high_layer()]
    }

    /// Lean of towers between two wind levels (m of horizontal offset per m of height): the
    /// wind difference over a 25 m/s reference, capped at ~19 degrees. A 5 m/s difference
    /// leans a tower ~11 degrees downwind.
    fn shear(&self, a: Vec2, b: Vec2) -> Vec2 {
        ((b - a) / 25.0).clamp_length_max(0.35)
    }

    fn low_layer(&self) -> CloudLayer {
        let inst = saturate(self.instability);
        let rh = saturate(self.humidity_low);
        let base = self.lcl_m();
        // Convective depth: humilis ~700 m, mediocris ~1.8 km, congestus up to ~6.5 km. The
        // layer's top is the tallest tower; most stop lower (towering). Cumulonimbus are the
        // storm cells (storm.rs), which grow from this layer to the tropopause.
        let depth = lerp(700.0, 11_000.0, inst.powf(1.6)).min(CONGESTUS_MAX_M);
        let top = (base + depth).min(self.tropopause_m);
        // Heaps when unstable, a deck when stable and saturated.
        let cumuliform = smoothstep(0.06, 0.3, inst).max(1.0 - smoothstep(0.85, 0.95, rh)).min(1.0);
        // Coverage peaks with moderate convection; deep convection concentrates into fewer,
        // bigger cells with clear air (subsidence) between them.
        let cu_cov = smoothstep(0.55, 0.92, rh) * lerp(0.5, 0.75, smoothstep(0.0, 0.5, inst)) * (1.0 - 0.35 * smoothstep(0.6, 0.95, inst));
        let deck_cov = smoothstep(0.8, 0.95, rh) * (1.0 - smoothstep(0.1, 0.35, inst));
        let coverage = cu_cov.max(deck_cov).max(smoothstep(0.85, 0.98, rh) * self.frontal_lift);
        let (top, cumuliform) = if cumuliform < 0.5 {
            // A deck: 400-900 m thick (thicker under a front: nimbostratus).
            (base + lerp(450.0, 900.0, rh) + self.frontal_lift * 1600.0, cumuliform)
        } else {
            (top, cumuliform)
        };
        CloudLayer {
            kind: LayerKind::Low,
            base_m: base,
            top_m: top,
            coverage: saturate(coverage),
            cumuliform,
            towering: lerp(0.25, 0.85, smoothstep(0.15, 0.7, inst)),
            density: lerp(0.8, 1.6, smoothstep(0.3, 0.9, inst).max(self.frontal_lift * 0.8)),
            wispiness: lerp(0.28, 0.12, inst) + (1.0 - rh) * 0.25,
            // Bigger, wider-spaced cells as convection deepens.
            cell_m: lerp(1_800.0, 5_500.0, inst) * lerp(1.0, 1.6, 1.0 - cumuliform),
            stretch: 1.0 + self.wind_low.length() * 0.02,
            wind: self.wind_low.lerp(self.wind_mid, 0.35),
            shear: self.shear(self.wind_low, self.wind_mid),
            precipitation: saturate(self.precipitation) * smoothstep(0.6, 0.9, rh),
        }
    }

    fn mid_layer(&self) -> CloudLayer {
        let rh = saturate(self.humidity_mid);
        let lift = saturate(self.frontal_lift);
        let base = (self.lcl_m() + 1_500.0).clamp(3_000.0, 5_000.0).max(self.ground_m + 1_500.0);
        let sheet = lift; // altostratus with lift, altocumulus without
        let thickness = lerp(350.0, 2_400.0, sheet);
        let coverage = smoothstep(0.5, 0.9, rh) * lerp(0.6, 1.0, lift);
        CloudLayer {
            kind: LayerKind::Mid,
            base_m: base,
            top_m: base + thickness,
            coverage,
            cumuliform: lerp(0.35, 0.0, sheet),
            towering: 0.2,
            density: lerp(0.45, 0.9, sheet),
            wispiness: 0.45,
            cell_m: lerp(900.0, 6_000.0, sheet),
            stretch: 1.4,
            wind: self.wind_mid,
            shear: self.shear(self.wind_mid, self.wind_high),
            precipitation: 0.0,
        }
    }

    fn high_layer(&self) -> CloudLayer {
        let rh = saturate(self.humidity_high);
        let lift = saturate(self.frontal_lift);
        let base = (self.tropopause_m - 3_000.0).max(7_000.0);
        let coverage = smoothstep(0.45, 0.85, rh) * lerp(0.6, 1.0, lift);
        let speed = self.wind_high.length();
        CloudLayer {
            kind: LayerKind::High,
            base_m: base,
            top_m: base + lerp(900.0, 1_800.0, lift),
            coverage,
            cumuliform: 0.0,
            towering: 0.2,
            // Ice: optical depth ~0.3-1 through the layer (cirrostratus under a front: ~2).
            density: lerp(0.006, 0.03, lift),
            wispiness: 0.95 - lift * 0.35,
            cell_m: lerp(3_200.0, 6_000.0, lift),
            // Fall streaks drawn out by the jet: the faster, the longer.
            stretch: (2.0 + speed * 0.12).min(6.0) * lerp(1.0, 0.6, lift),
            wind: self.wind_high,
            shear: Vec2::ZERO,
            precipitation: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l(p: Preset) -> [CloudLayer; 3] {
        CloudWeather::preset(p).layers()
    }

    #[test]
    fn clear_has_no_cloud() {
        assert!(l(Preset::Clear).iter().all(|l| l.coverage < 0.01), "{:?}", l(Preset::Clear));
    }

    #[test]
    fn convection_deepens_with_instability() {
        let (fair, tower, cb) = (l(Preset::Fair)[0], l(Preset::Towering)[0], l(Preset::Thunderstorm)[0]);
        assert!(fair.thickness() < 1_500.0, "fair cumulus are shallow: {}", fair.thickness());
        assert!(tower.thickness() > 3_500.0, "congestus: {}", tower.thickness());
        assert!(cb.thickness() <= CONGESTUS_MAX_M, "ordinary cells stop at congestus: {}", cb.thickness());
        assert!(fair.cumuliform > 0.9 && cb.cumuliform > 0.9);
        assert!(cb.density > fair.density && cb.precipitation > 0.5 && fair.precipitation == 0.0);
        assert!(CloudWeather::preset(Preset::Thunderstorm).storm_chance() > 0.5);
        assert_eq!(CloudWeather::preset(Preset::Fair).storm_chance(), 0.0);
    }

    #[test]
    fn stable_and_humid_makes_a_deck() {
        let o = l(Preset::Overcast)[0];
        assert!(o.cumuliform < 0.3 && o.coverage > 0.8 && o.thickness() < 1_500.0, "{o:?}");
    }

    #[test]
    fn fronts_make_sheets_and_rain() {
        let f = l(Preset::Frontal);
        assert!(f[1].cumuliform < 0.2 && f[1].coverage > 0.8, "altostratus {:?}", f[1]);
        assert!(f[0].precipitation > 0.3);
        assert!(f[2].coverage > 0.5);
    }

    #[test]
    fn cirrus_is_high_thin_and_streaked() {
        let c = l(Preset::Cirrus);
        assert!(c[0].coverage < 0.05 && c[2].coverage > 0.5);
        assert!(c[2].base_m > 7_000.0 && c[2].density < 0.05 && c[2].stretch > 3.0 && c[2].wispiness > 0.7);
    }

    #[test]
    fn layers_are_ordered_in_height() {
        for p in Preset::ALL {
            let [lo, mid, hi] = l(p);
            assert!(lo.base_m < mid.base_m && mid.base_m < hi.base_m, "{p:?}");
            assert!(mid.top_m <= hi.top_m);
        }
    }

    #[test]
    fn base_follows_dewpoint_spread_and_ground() {
        let mut w = CloudWeather::preset(Preset::Scattered);
        w.dewpoint_spread_c = 8.0;
        assert_eq!(w.lcl_m(), 1_000.0);
        w.ground_m = 1_600.0; // Zermatt
        assert_eq!(w.layers()[0].base_m, 2_600.0);
    }

    #[test]
    fn blending_is_continuous() {
        let (a, b) = (CloudWeather::preset(Preset::Fair), CloudWeather::preset(Preset::Thunderstorm));
        assert_eq!(a.lerp(&b, 0.0), a);
        assert_eq!(a.lerp(&b, 1.0), b);
        let mut prev = a.lerp(&b, 0.0).layers()[0].top_m;
        for i in 1..=20 {
            let top = a.lerp(&b, i as f32 / 20.0).layers()[0].top_m;
            assert!((top - prev).abs() < 2_500.0, "no jumps: {prev} -> {top}");
            prev = top;
        }
    }

    #[test]
    fn surface_wind_drives_every_level() {
        let w = CloudWeather::preset(Preset::Scattered).with_surface_wind(Vec2::new(0.0, -9.0));
        assert_eq!(w.wind_low, Vec2::new(0.0, -9.0));
        assert!(w.wind_mid.length() > CloudWeather::preset(Preset::Scattered).wind_mid.length());
        // Faster wind, faster clouds (O3-W5).
        assert!(w.layers()[0].wind.length() > CloudWeather::preset(Preset::Scattered).layers()[0].wind.length());
    }
}
