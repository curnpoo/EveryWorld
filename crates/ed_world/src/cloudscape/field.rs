//! The cloud density at any point: the one function the CPU reference and the GPU pass share
//! (`clouds.slang` `cloud_sample` mirrors [`CloudField::sample`] line for line).
//!
//! Per layer, at a point (x, z, height h):
//! 1. **Weather map**: where clouds are. Two coverage fields drift with the layer's wind and
//!    crossfade over [`EVOLVE_S`] (variance-preserving), so clouds form and dissolve as they
//!    travel instead of sliding like a decal. A regional channel clusters them. The coverage
//!    threshold comes from the bank's quantile table, so coverage 0.3 covers 30% of the sky.
//! 2. **Column height**: the tower channel decides how tall each cloud grows (`towering`), so
//!    a field of cumulus has a few big ones among many small ones.
//! 3. **Height profile**: flat bases for heaps (a 60-90 m ramp at the condensation level), soft
//!    bases for sheets, domed or flat tops.
//! 4. **Shape**: Perlin-Worley dilated by Worley fBm, carved by the envelope (profile x
//!    presence): the envelope thins towards the top, so only the strongest noise survives
//!    there and tops come out as rounded turrets.
//! 5. **Detail erosion**: Worley fBm (billows) high in the cloud, Perlin (torn wisps) low and
//!    in wispy layers, warped by the shape noise so edges curl. Faded out with distance
//!    ([`Lod`]) before it can alias, and the source of no grain anywhere.
//! 6. **Storm cells** add their own envelope (core, overshooting dome, anvil) to the low layer.
//! 7. **Rain shafts** fill the air under raining columns down to the ground.

use super::layer::{CloudLayer, LayerKind};
use super::noise::{NoiseBank, DETAIL_PERIOD_M, SHAPE_PERIOD_M, WEATHER_CELLS};
use super::storm::StormCell;
use super::{lerp, remap, saturate, smoothstep};
use glam::{DVec2, Vec2, Vec3, Vec4};

/// Extinction (1/m) of density-1 cloud at full noise. Cumulus cores measure 0.05-0.15 /m.
pub const SIGMA_MAX: f32 = 0.11;
/// Extinction (1/m) of a full-strength rain shaft.
pub const SIGMA_RAIN: f32 = 0.0012;
/// Single-scattering albedo of rain (cloud is 1: water droplets barely absorb).
pub const RAIN_ALBEDO: f32 = 0.55;
/// Coverage crossfade period (s): clouds form and dissolve over about this long.
pub const EVOLVE_S: f64 = 1_500.0;
/// Tops boil upward this fast (m/s of detail-noise rise relative to the cloud).
pub const BOIL_MS: f64 = 0.8;
/// Distance band (m) over which detail erosion fades to nothing.
pub const DETAIL_FADE_M: (f32, f32) = (18_000.0, 45_000.0);

/// How much detail a sample takes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lod {
    /// Weight of detail erosion, 0..1.
    pub detail: f32,
}

impl Lod {
    pub const FULL: Lod = Lod { detail: 1.0 };
    /// Shape only (far clouds, the long light samples).
    pub const COARSE: Lod = Lod { detail: 0.0 };

    /// Detail weight at `dist_m` from the camera.
    pub fn at(dist_m: f32) -> Lod {
        Lod { detail: 1.0 - smoothstep(DETAIL_FADE_M.0, DETAIL_FADE_M.1, dist_m) }
    }
}

/// One density sample.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sample {
    /// Extinction coefficient (1/m).
    pub sigma_t: f32,
    /// Single-scattering albedo (1 for cloud, [`RAIN_ALBEDO`] for rain).
    pub albedo: f32,
    /// Height within the cloud's own column, 0 base .. 1 top.
    pub column_hf: f32,
    /// Optical depth of cloud above and below the sample in its column, estimated from the
    /// distance to the column's top and base at a typical interior density (no extra samples).
    /// Ambient light uses them: the sky reaches a base through the whole cloud above it.
    pub tau_up: f32,
    pub tau_down: f32,
}

/// Interior extinction, as a fraction of [`SIGMA_MAX`] x density, for the ambient estimate.
pub const SIGMA_INTERIOR: f32 = 0.4;
/// The carve threshold never drops below `1 - CARVE`: interiors stay lumpy, never a solid block.
pub const CARVE: f32 = 0.55;
/// Share of the noise range above the carve threshold over which density ramps to full.
pub const EDGE_RAMP: f32 = 0.55;

/// Per-layer constants of one frame: wind drift, crossfade, the weather-map transform. The
/// GPU block carries exactly these (see `gpu.rs`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayerFrame {
    pub layer: CloudLayer,
    /// Drift of the layer since t = 0 (m; east, north).
    pub drift: DVec2,
    /// East/north metres -> wind frame (along / stretch, across): rows of a 2x2 matrix.
    pub axes: [Vec2; 2],
    /// Weather-map tile size (m) along both wind-frame axes.
    pub weather_tile_m: f64,
    /// Crossfade weights of the two coverage fields and their per-cycle tile offsets.
    pub fade: (f32, f32),
    pub fade_off: (Vec2, Vec2),
    /// Noise threshold for the layer's coverage.
    pub threshold: f32,
    /// 1 / sqrt(wa^2 + wb^2): restores the variance the crossfade removes.
    pub fade_norm: f32,
    /// Vertical shape-noise offset of this layer (tile units), so layers never repeat each other.
    pub shape_y: f32,
}

/// The cloud field at one time.
pub struct CloudField<'a> {
    pub bank: &'a NoiseBank,
    pub frames: [LayerFrame; 3],
    pub storms: Vec<StormCell>,
    pub time_s: f64,
    pub ground_m: f32,
}

#[inline]
fn wrap(v: f64) -> f32 {
    v.rem_euclid(1.0) as f32
}

impl LayerFrame {
    pub fn new(layer: CloudLayer, bank: &NoiseBank, t: f64, seed: u64) -> Self {
        let wind = layer.wind;
        let dir = wind.try_normalize().unwrap_or(Vec2::X);
        let perp = dir.perp();
        let s = layer.stretch.max(1.0);
        let axes = [dir / s, perp];
        let drift = wind.as_dvec2() * t;
        // Two coverage fields, phases half a period apart. Each jumps to a fresh offset when
        // its weight is zero, so the sky never repeats and never pops.
        let ph = t / EVOLVE_S;
        let li = layer.kind as u64;
        let cyc = |p: f64| {
            let c = p.floor() as u64;
            let f = (p - p.floor()) as f32;
            let o = Vec2::new(crate::hash01(seed ^ c.wrapping_mul(0x9E37_79B9), 11 + li), crate::hash01(seed ^ c.wrapping_mul(0x85EB_CA6B), 13 + li));
            (1.0 - (2.0 * f - 1.0).abs(), o)
        };
        let (wa, oa) = cyc(ph);
        let (wb, ob) = cyc(ph + 0.5);
        Self {
            layer,
            drift,
            axes,
            weather_tile_m: layer.cell_m.max(100.0) as f64 * WEATHER_CELLS,
            fade: (wa, wb),
            fade_off: (oa, ob),
            // Domes and carving draw a smaller footprint than the raw threshold covers; widen it
            // (more for heaps) so the drawn sky fraction matches `coverage`.
            threshold: bank.cov_threshold(1.0 - (1.0 - layer.coverage).powf(lerp(1.3, 2.0, layer.cumuliform))),
            fade_norm: 1.0 / (wa * wa + wb * wb).sqrt().max(0.5),
            shape_y: 0.31 * li as f32,
        }
    }

    /// Wind-frame position (m) of an east/north point, with drift removed. f64: the f32 the
    /// GPU uses is camera-relative.
    fn wind_frame(&self, en: DVec2) -> DVec2 {
        let q = en - self.drift;
        DVec2::new(self.axes[0].as_dvec2().dot(q), self.axes[1].as_dvec2().dot(q))
    }
}

/// Column constants at one (x, z) of one layer: presence (0..1), top fraction of the band.
#[derive(Clone, Copy, Debug)]
struct Column {
    presence: f32,
    htop: f32,
    rain: f32,
}

impl<'a> CloudField<'a> {
    /// The field for `layers` at time `t_s`, with `storms` (from `storm::storm_cells`).
    pub fn new(bank: &'a NoiseBank, layers: [CloudLayer; 3], storms: Vec<StormCell>, t_s: f64, ground_m: f32, seed: u64) -> Self {
        let frames = layers.map(|l| LayerFrame::new(l, bank, t_s, seed));
        Self { bank, frames, storms, time_s: t_s, ground_m }
    }

    /// Lowest and highest altitude (m) any cloud or rain can be at.
    pub fn bounds(&self) -> (f32, f32) {
        let mut lo = f32::MAX;
        let mut hi = f32::MIN;
        for f in self.frames.iter().filter(|f| f.layer.active()) {
            lo = lo.min(f.layer.floor_m(self.ground_m));
            hi = hi.max(f.layer.top_m);
        }
        for s in &self.storms {
            lo = lo.min(if s.rain > 0.0 { self.ground_m } else { s.base_m });
            hi = hi.max(s.top_m);
        }
        if lo > hi { (0.0, 0.0) } else { (lo, hi) }
    }

    fn column(&self, f: &LayerFrame, en: DVec2) -> Column {
        let l = &f.layer;
        let q = f.wind_frame(en);
        let uv = Vec2::new(wrap(q.x / f.weather_tile_m), wrap(q.y / f.weather_tile_m));
        let wa = self.bank.weather.sample(uv + f.fade_off.0);
        let wb = self.bank.weather.sample(uv + f.fade_off.1);
        // Variance-preserving crossfade of the two coverage fields about their mean.
        let mean = 0.5;
        let cov = mean + ((wa.x - mean) * f.fade.0 + (wb.y - mean) * f.fade.1) * f.fade_norm;
        // Regional clustering: shift the threshold by the slow channel.
        let region = (wa.w - 0.5) * 0.18 * (1.0 - l.coverage);
        let th = f.threshold - region;
        // Presence is radial: 0 at the cloud's edge, rising towards its core over most of the
        // noise range above the threshold. The profile reads it as "how far inside": higher in
        // a heap needs a stronger core, which is what makes domes instead of extruded walls.
        let soft = (1.0 - th).clamp(0.12, 0.5) * lerp(0.6, 0.85, l.cumuliform);
        let presence = saturate(remap(cov, th, th + soft, 0.0, 1.0));
        // Tower height: the tallest cells grow to the top of the band.
        let tw = wa.z * f.fade.0 + wb.z * f.fade.1;
        let tall = smoothstep(1.0 - l.towering - 0.15, 1.0 - l.towering + 0.35, tw);
        let htop = lerp(lerp(0.3, 0.55, l.towering), 1.0, tall);
        // Sheets: thick and thin patches (the mottled grey of a deck seen from below).
        let sheet = lerp(0.4, 1.0, smoothstep(0.25, 0.8, cov));
        let htop = lerp(sheet, htop, l.cumuliform);
        // Rain falls from the cores of the tallest columns (and everywhere under a deep sheet).
        let rain = l.precipitation * smoothstep(0.35, 0.8, presence) * lerp(1.0, tall, l.cumuliform);
        Column { presence, htop, rain }
    }

    /// Envelope (0..1) of a column with radial `presence` at height `h` of layer `l` whose top
    /// is at fraction `htop` of the band, and the height fraction within the column.
    ///
    /// Heaps: a flat base at the condensation level (a 90 m ramp), a body slightly narrower at
    /// the base than at mid height, and a dome: the presence needed rises towards the top, so
    /// only the core reaches it. Sheets: soft above and below, nearly the same width throughout.
    fn profile(l: &CloudLayer, h: f32, htop: f32, presence: f32) -> (f32, f32) {
        let thick = l.thickness() * htop;
        let dh = h - l.base_m;
        if dh < 0.0 || dh > thick || presence <= 0.0 {
            return (0.0, 0.0);
        }
        let x = dh / thick;
        let cu = l.cumuliform;
        let bottom = smoothstep(0.0, lerp(0.3 * thick, 90.0_f32.min(thick * 0.25), cu), dh);
        // Presence needed at this height: the dome (heaps) or a soft top (sheets).
        let dome = lerp(0.0, 0.92, smoothstep(0.2, 1.0, x).powf(1.3));
        let waist = 0.12 * (1.0 - smoothstep(0.0, 0.2, x));
        let need = lerp(0.05 + 0.5 * smoothstep(0.6, 1.0, x), dome + waist, cu);
        let e = saturate((presence - need) / (1.0 - need).max(0.08) * lerp(1.5, 2.5, cu));
        (bottom * e, x)
    }

    /// Cheap upper bound of the density's envelope (no 3D noise): 0 means certainly empty.
    /// The marcher uses it to stride through clear air.
    pub fn envelope(&self, x: f64, z: f64, h: f32) -> f32 {
        let en = DVec2::new(x, -z);
        let mut e = 0.0f32;
        for f in &self.frames {
            let l = &f.layer;
            if !l.active() || h > l.top_m || h < l.floor_m(self.ground_m) {
                continue;
            }
            let c = self.column(f, en);
            if h < l.base_m {
                e = e.max(c.rain);
                continue;
            }
            e = e.max(Self::profile(l, h, c.htop, c.presence).0);
        }
        for s in &self.storms {
            e = e.max(s.envelope(x, z, h).0).max(if h < s.base_m { s.rain_at(x, z) } else { 0.0 });
        }
        e
    }

    /// Density at render-frame (`x`, `z`) and altitude `h` (m above the datum).
    pub fn sample(&self, x: f64, z: f64, h: f32, lod: Lod) -> Sample {
        let en = DVec2::new(x, -z);
        let mut out = Sample { sigma_t: 0.0, albedo: 1.0, column_hf: 1.0, tau_up: 0.0, tau_down: 0.0 };
        let mut rain = 0.0f32;
        let mut best = 0.0f32;
        for (li, f) in self.frames.iter().enumerate() {
            let l = &f.layer;
            if !l.active() || h > l.top_m || h < l.floor_m(self.ground_m) {
                continue;
            }
            let c = self.column(f, en);
            if h < l.base_m {
                rain = rain.max(c.rain);
                continue;
            }
            let (prof, x_col) = Self::profile(l, h, c.htop, c.presence);
            // Storm cells belong to the low layer: take the stronger envelope.
            let (mut env, mut hf) = (prof, x_col);
            let mut density = l.density;
            let col_m = l.thickness() * c.htop;
            let (mut above, mut below) = ((1.0 - x_col) * col_m, x_col * col_m);
            if l.kind == LayerKind::Low {
                for s in &self.storms {
                    let (se, shf) = s.envelope(x, z, h);
                    if se > env {
                        env = se;
                        hf = shf;
                        density = density.max(1.5);
                        let sh = s.top_m - s.base_m;
                        (above, below) = ((1.0 - shf) * sh, shf * sh);
                    }
                }
            }
            if env <= 0.0 {
                continue;
            }
            let sigma = self.shape(f, li, en, h, env, hf, lod) * density * SIGMA_MAX;
            if sigma > out.sigma_t {
                best = sigma;
                out.column_hf = hf;
                let si = density * SIGMA_MAX * SIGMA_INTERIOR;
                (out.tau_up, out.tau_down) = (si * above, si * below);
            }
            out.sigma_t += sigma;
        }
        // Storm cores reach above the low layer's band: sample them on their own there.
        if self.frames[0].layer.active() && h > self.frames[0].layer.top_m {
            for s in &self.storms {
                let (se, shf) = s.envelope(x, z, h);
                if se > 0.0 {
                    let sigma = self.shape(&self.frames[0], 0, en, h, se, shf, lod) * 1.5 * SIGMA_MAX;
                    out.sigma_t += sigma;
                    if sigma > best {
                        best = sigma;
                        out.column_hf = shf;
                        let (si, sh) = (1.5 * SIGMA_MAX * SIGMA_INTERIOR, s.top_m - s.base_m);
                        (out.tau_up, out.tau_down) = (si * (1.0 - shf) * sh, si * shf * sh);
                    }
                }
            }
        }
        for s in &self.storms {
            if h < s.base_m {
                rain = rain.max(s.rain_at(x, z));
            }
        }
        if rain > 0.0 && out.sigma_t < 1e-4 {
            // Rain shafts: streaky in the vertical, thinning just under the base (evaporating
            // virga at the edges), dense near the ground under heavy cores.
            let p = Vec3::new(wrap(en.x / 900.0), (h / 4000.0).fract(), wrap(en.y / 900.0));
            let n = self.bank.detail.sample(p).w;
            let shaft = saturate(remap(n, 0.35, 0.75, 0.0, 1.0)) * rain;
            out.sigma_t = shaft * SIGMA_RAIN;
            out.albedo = RAIN_ALBEDO;
            out.column_hf = 0.0;
            // Under a raining cloud: the sky is behind kilometres of it.
            out.tau_up = 40.0 * rain.sqrt();
        }
        out
    }

    /// Steps 4-5 for one layer: noise carved by the envelope `env`, eroded by detail.
    #[allow(clippy::too_many_arguments)]
    fn shape(&self, f: &LayerFrame, li: usize, en: DVec2, h: f32, env: f32, hf: f32, lod: Lod) -> f32 {
        let l = &f.layer;
        let dh = (h - l.base_m) as f64;
        // Towers lean downwind; noise rides the wind (and the wind frame, so cirrus streaks).
        let q = f.wind_frame(en - l.shear.as_dvec2() * dh);
        let ps = Vec3::new(wrap(q.x / SHAPE_PERIOD_M), h / SHAPE_PERIOD_M as f32 + f.shape_y, wrap(q.y / SHAPE_PERIOD_M));
        let s: Vec4 = self.bank.shape.sample(ps);
        let wfbm = s.y * 0.625 + s.z * 0.25 + s.w * 0.125;
        let base = remap(s.x, wfbm - 1.0, 1.0, 0.0, 1.0);
        // Carve: where the envelope is thin, only the strongest noise survives.
        let th = 1.0 - env * CARVE;
        // A short ramp above the threshold: cumulus edges are sharp, not fog.
        let mut c = saturate(remap(base, th, lerp(th, 1.0, EDGE_RAMP), 0.0, 1.0));
        if c <= 0.0 {
            return 0.0;
        }
        if lod.detail > 0.0 {
            // Detail rides the cloud, rises slowly (boiling tops) and curls with the shape noise.
            let warp = Vec3::new(s.y - 0.5, s.z - 0.5, s.w - 0.5) * 0.35;
            let rise = (self.time_s * BOIL_MS).rem_euclid(DETAIL_PERIOD_M) as f32;
            let pd = Vec3::new(wrap(q.x / DETAIL_PERIOD_M), (h - rise) / DETAIL_PERIOD_M as f32 + li as f32 * 0.29, wrap(q.y / DETAIL_PERIOD_M)) + warp;
            let d = self.bank.detail.sample(pd);
            // Weighted to the lower octaves: a strong single fine Worley frequency reads as regular
            // ripples on flat tops under a low sun.
            let billow = d.x * 0.6 + d.y * 0.32 + d.z * 0.08;
            // Wispy at the bottom and in wispy layers; billowy (inverted Worley) at the top.
            let wisp = lerp(billow, d.w, l.wispiness);
            let e = lerp(wisp, 1.0 - billow, saturate(hf * 3.0) * (1.0 - l.wispiness));
            let amount = lerp(0.3, 0.55, l.wispiness) * lod.detail;
            c = saturate(remap(c, e * amount, 1.0, 0.0, 1.0));
            // Ice fibres: ridged Perlin on the wind-stretched coordinates gives the long, fine
            // streaks of cirrus (only in very wispy layers).
            let fib = 1.0 - (2.0 * d.w - 1.0).abs();
            let fw = saturate((l.wispiness - 0.75) * 5.0) * lod.detail.max(0.5);
            c *= lerp(1.0, smoothstep(0.65, 0.97, fib), fw);
        }
        // Denser towards the middle and top of heaps (bright, solid tops; translucent bases).
        c * lerp(1.0, lerp(0.55, 1.0, smoothstep(0.0, 0.35, hf)), l.cumuliform)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloudscape::storm::storm_cells;
    use crate::cloudscape::weather::{CloudWeather, Preset};
    use std::sync::OnceLock;

    fn bank() -> &'static NoiseBank {
        static B: OnceLock<NoiseBank> = OnceLock::new();
        B.get_or_init(|| NoiseBank::bake_sized(0xC10D, 32, 16, 128))
    }

    fn field(p: Preset, t: f64) -> CloudField<'static> {
        let w = CloudWeather::preset(p);
        let layers = w.layers();
        let storms = storm_cells(&w, &layers[0], DVec2::ZERO, 80_000.0, t);
        CloudField::new(bank(), layers, storms, t, w.ground_m, w.seed)
    }

    /// Fraction of a 42 km grid of columns with cloud anywhere in the low layer (what you
    /// see looking up).
    fn sky_fraction(f: &CloudField<'_>) -> f32 {
        let l = f.frames[0].layer;
        let n = 60;
        let mut hit = 0;
        for j in 0..n {
            for i in 0..n {
                let (x, z) = (i as f64 * 700.0, j as f64 * 700.0);
                if (0..24).any(|k| f.sample(x, z, l.base_m + l.thickness() * (k as f32 + 0.5) / 24.0, Lod::COARSE).sigma_t > 1e-4) {
                    hit += 1;
                }
            }
        }
        hit as f32 / (n * n) as f32
    }

    #[test]
    fn coverage_tracks_the_weather() {
        let fr: Vec<(Preset, f32, f32)> = [Preset::Clear, Preset::Fair, Preset::Scattered, Preset::Towering, Preset::Overcast]
            .into_iter()
            .map(|p| {
                let f = field(p, 0.0);
                (p, f.frames[0].layer.coverage, sky_fraction(&f))
            })
            .collect();
        eprintln!("{fr:?}");
        let v: Vec<f32> = fr.iter().map(|x| x.2).collect();
        assert_eq!(v[0], 0.0);
        assert!(v[1] > 0.08 && v[1] < v[2] && v[2] < v[4], "{fr:?}");
        assert!(v[4] > 0.85, "a deck covers the sky: {fr:?}");
        // Visible sky fraction follows the layer's coverage (the quantile table).
        for (p, cov, frac) in &fr[1..] {
            assert!((frac - cov).abs() < 0.2, "{p:?}: coverage {cov} drew {frac}");
        }
    }

    #[test]
    fn flat_bases_and_nothing_outside_the_bands() {
        let f = field(Preset::Towering, 0.0);
        let l = f.frames[0].layer;
        let mut below = 0.0f32;
        let mut above = 0.0f32;
        for i in 0..400 {
            let (x, z) = (i as f64 * 97.0, (i * 7 % 23) as f64 * 311.0);
            below = below.max(f.sample(x, z, l.base_m - 5.0, Lod::FULL).sigma_t);
            above = above.max(f.sample(x, z, l.top_m + 5.0, Lod::FULL).sigma_t);
        }
        assert_eq!(below, 0.0);
        assert_eq!(above, 0.0);
    }

    #[test]
    fn envelope_bounds_the_density() {
        for p in [Preset::Scattered, Preset::Thunderstorm, Preset::Frontal] {
            let f = field(p, 900.0);
            let (lo, hi) = f.bounds();
            for i in 0..3000 {
                let x = (crate::hash01(i, 1) as f64 - 0.5) * 60_000.0;
                let z = (crate::hash01(i, 2) as f64 - 0.5) * 60_000.0;
                let h = lo + crate::hash01(i, 3) * (hi - lo);
                let s = f.sample(x, z, h, Lod::FULL);
                if s.sigma_t > 0.0 {
                    assert!(f.envelope(x, z, h) > 0.0, "{p:?} density without envelope at {x} {z} {h}");
                }
            }
        }
    }

    #[test]
    fn clouds_move_with_the_wind() {
        // Over a short time (no crossfade change to speak of) the field translates with the wind.
        let (a, b) = (field(Preset::Scattered, 3000.0), field(Preset::Scattered, 3010.0));
        let l = a.frames[0].layer;
        let w = l.wind; // east, north
        let h = l.base_m + 200.0;
        let mut err = 0.0f32;
        let mut sum = 0.0f32;
        for i in 0..300 {
            let (x, z) = (i as f64 * 53.0, (i % 17) as f64 * 211.0);
            let s0 = a.sample(x, z, h, Lod::COARSE).sigma_t;
            // 10 s later the same cloud is 10 s of wind downwind (render z = -north).
            let s1 = b.sample(x + w.x as f64 * 10.0, z - w.y as f64 * 10.0, h, Lod::COARSE).sigma_t;
            err += (s0 - s1).abs();
            sum += s0;
        }
        assert!(sum > 0.0 && err / sum < 0.08, "drift mismatch {}", err / sum);
    }

    #[test]
    fn evolves_without_popping() {
        // Sample one point every 10 s for an hour in the drifting frame: smooth changes only.
        let mut prev: Option<f32> = None;
        let mut changed = 0.0;
        for k in 0..360 {
            let t = k as f64 * 10.0;
            let f = field(Preset::Scattered, t);
            let l = f.frames[0].layer;
            let (x, z) = (1000.0 + l.wind.x as f64 * t, -(l.wind.y as f64) * t);
            let s = f.envelope(x, z, l.base_m + 300.0);
            if let Some(p) = prev {
                assert!((s - p).abs() < 0.2, "pop at {t}: {p} -> {s}");
                changed += (s - p).abs();
            }
            prev = Some(s);
        }
        assert!(changed > 0.1, "clouds should evolve over an hour");
    }

    #[test]
    fn storms_are_tall_and_rain_reaches_the_ground() {
        let w = CloudWeather::preset(Preset::Thunderstorm);
        let layers = w.layers();
        // Find a mature storm.
        let mut t = 0.0;
        let s = loop {
            let cells = storm_cells(&w, &layers[0], DVec2::ZERO, 120_000.0, t);
            if let Some(c) = cells.iter().find(|c| c.core > 0.9 && c.rain > 0.3 && c.top_m > 10_000.0) {
                break *c;
            }
            t += 600.0;
            assert!(t < 50_000.0, "no mature storm found");
        };
        let f = CloudField::new(bank(), layers, vec![s], t, 0.0, w.seed);
        let mut core_hi = 0.0f32;
        for k in 0..40 {
            let h = s.top_m * 0.82 + k as f32 * 10.0;
            core_hi = core_hi.max(f.sample(s.x, s.z, h, Lod::COARSE).sigma_t);
        }
        assert!(core_hi > 0.0, "storm core reaches the upper troposphere");
        let mut rain = 0.0f32;
        for k in 0..60 {
            let o = k as f64 * 20.0;
            let r = f.sample(s.x + o, s.z, 200.0, Lod::FULL);
            if r.albedo < 1.0 {
                rain = rain.max(r.sigma_t);
            }
        }
        assert!(rain > 0.0, "rain shaft under the core");
    }

    #[test]
    fn detail_fades_with_distance() {
        assert_eq!(Lod::at(1_000.0), Lod::FULL);
        assert_eq!(Lod::at(60_000.0), Lod::COARSE);
        assert!(Lod::at(30_000.0).detail > 0.0 && Lod::at(30_000.0).detail < 1.0);
    }
}
