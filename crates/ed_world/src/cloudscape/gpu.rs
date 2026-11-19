//! One frame of the cloud field for the GPU: `clouds.slang` `CloudData`, [`CLOUD_ROWS`] float4
//! rows. Everything that needs f64 (wind drift, camera position modulo the noise periods,
//! storm positions) is folded here, so the shader only adds camera-relative f32 offsets and
//! the field is the same from any floating origin.
//!
//! Layout (row: contents):
//! - 0: on, max steps, frame (u32 bits), debug view
//! - 1: lowest cloud (m), highest (m), ground (m), max distance (m)
//! - 2: camera altitude (m), planet radius (m), detail rise (m, mod the detail period), step fraction
//! - 3: min step (m), max step (m), min transmittance, fine step scale
//! - 4: detail fade start, end (m), rain-shaft noise uv at the camera (x, y)
//! - 5: storm count, stride (steps), refine (steps), unused
//! - 6..30: three layers of [`LAYER_ROWS`] rows (see [`layer_rows`])
//! - 30..94: [`MAX_STORMS`] storms of 4 rows (see [`storm_rows`])
//! - 94..100: the renderer's reprojection rows (previous view-projection, camera delta, size)

use super::field::{CloudField, LayerFrame, DETAIL_FADE_M};
use super::light::{MarchCfg, FINE, REFINE, STRIDE};
use super::noise::{DETAIL_PERIOD_M, SHAPE_PERIOD_M};
use super::storm::{StormCell, MAX_STORMS};
use glam::{DVec2, DVec3};

pub const LAYER_ROWS: usize = 8;
pub const LAYER_ROW0: usize = 6;
pub const STORM_ROW0: usize = LAYER_ROW0 + 3 * LAYER_ROWS;
/// First row the renderer fills (previous view-projection, camera delta, half-res size).
pub const REPROJECT_ROW0: usize = STORM_ROW0 + MAX_STORMS * 4;
pub const CLOUD_ROWS: usize = REPROJECT_ROW0 + 6;
pub const CLOUD_WORDS: usize = CLOUD_ROWS * 4;

/// What one frame packs.
pub struct GpuFrame<'a> {
    pub field: &'a CloudField<'a>,
    /// Camera position in the render frame (m; x east, z -north), f64.
    pub camera: DVec3,
    /// Camera altitude above the datum (m).
    pub camera_alt_m: f64,
    pub planet_radius_m: f64,
    pub cfg: MarchCfg,
    pub frame: u32,
    /// 0 off, 1 cloud transmittance, 2 cloud depth, 3 ground shadow (`EW_CLOUDS_DEBUG`).
    pub debug: u32,
}

#[inline]
fn frac(v: f64) -> f32 {
    v.rem_euclid(1.0) as f32
}

/// Rows of one layer, relative to the camera at east/north `en_cam`:
/// - 0: base, top, coverage threshold, cumuliform
/// - 1: towering, density, wispiness, active
/// - 2: wind-frame axes (row 0 xy, row 1 xy): east/north metres -> (along / stretch, across)
/// - 3: weather uv at the camera, coverage field A (xy, offset included) and B (zw)
/// - 4: shape uv at the camera (xy), detail uv at the camera (zw)
/// - 5: crossfade weight A, B, variance norm, 1 / weather tile (m)
/// - 6: shear (east, north per metre of height), precipitation, shape vertical offset
/// - 7: layer index, cell size (m), coverage, unused
pub fn layer_rows(f: &LayerFrame, en_cam: DVec2, index: usize) -> [[f32; 4]; LAYER_ROWS] {
    let l = &f.layer;
    let q = {
        let p = en_cam - f.drift;
        DVec2::new(f.axes[0].as_dvec2().dot(p), f.axes[1].as_dvec2().dot(p))
    };
    let tile = f.weather_tile_m;
    let wa = [frac(q.x / tile + f.fade_off.0.x as f64), frac(q.y / tile + f.fade_off.0.y as f64)];
    let wb = [frac(q.x / tile + f.fade_off.1.x as f64), frac(q.y / tile + f.fade_off.1.y as f64)];
    [
        [l.base_m, l.top_m, f.threshold, l.cumuliform],
        [l.towering, l.density, l.wispiness, if l.active() { 1.0 } else { 0.0 }],
        [f.axes[0].x, f.axes[0].y, f.axes[1].x, f.axes[1].y],
        [wa[0], wa[1], wb[0], wb[1]],
        [frac(q.x / SHAPE_PERIOD_M), frac(q.y / SHAPE_PERIOD_M), frac(q.x / DETAIL_PERIOD_M), frac(q.y / DETAIL_PERIOD_M)],
        [f.fade.0, f.fade.1, f.fade_norm, (1.0 / tile) as f32],
        [l.shear.x, l.shear.y, l.precipitation, f.shape_y],
        [index as f32, l.cell_m, l.coverage, 0.0],
    ]
}

/// Rows of one storm relative to the camera at render (`cx`, `cz`):
/// - 0: x, z (render, camera-relative), core radius, base
/// - 1: top, core, anvil, rain
/// - 2: anvil direction (render x, z), anvil reach, unused
/// - 3: lean (render x, z per m), unused, unused
pub fn storm_rows(s: &StormCell, cx: f64, cz: f64) -> [[f32; 4]; 4] {
    [
        [(s.x - cx) as f32, (s.z - cz) as f32, s.radius_m, s.base_m],
        [s.top_m, s.core, s.anvil, s.rain],
        [s.anvil_dir.x, s.anvil_dir.y, s.anvil_len_m, 0.0],
        [s.lean.x, s.lean.y, 0.0, 0.0],
    ]
}

/// The `CloudData` words of one frame (the reprojection rows are left zero for the renderer).
pub fn cloud_words(g: &GpuFrame<'_>) -> Vec<u32> {
    let f = g.field;
    let mut rows = vec![[0.0f32; 4]; CLOUD_ROWS];
    let (lo, hi) = f.bounds();
    let on = hi > lo;
    let en_cam = DVec2::new(g.camera.x, -g.camera.z);
    let rise = (f.time_s * super::field::BOIL_MS).rem_euclid(DETAIL_PERIOD_M) as f32;
    rows[0] = [if on { 1.0 } else { 0.0 }, g.cfg.max_steps as f32, 0.0, g.debug as f32];
    rows[1] = [lo, hi, f.ground_m, g.cfg.max_dist_m];
    rows[2] = [g.camera_alt_m as f32, g.planet_radius_m as f32, rise, g.cfg.step_frac];
    rows[3] = [g.cfg.min_step_m, g.cfg.max_step_m, g.cfg.min_transmittance, FINE];
    rows[4] = [DETAIL_FADE_M.0, DETAIL_FADE_M.1, frac(en_cam.x / 900.0), frac(en_cam.y / 900.0)];
    let storms = f.storms.len().min(MAX_STORMS);
    rows[5] = [storms as f32, STRIDE, REFINE as f32, 0.0];
    for (i, lf) in f.frames.iter().enumerate() {
        let r = layer_rows(lf, en_cam, i);
        rows[LAYER_ROW0 + i * LAYER_ROWS..LAYER_ROW0 + (i + 1) * LAYER_ROWS].copy_from_slice(&r);
    }
    for (i, s) in f.storms.iter().take(MAX_STORMS).enumerate() {
        let r = storm_rows(s, g.camera.x, g.camera.z);
        rows[STORM_ROW0 + i * 4..STORM_ROW0 + (i + 1) * 4].copy_from_slice(&r);
    }
    let mut w: Vec<u32> = rows.iter().flat_map(|r| r.map(f32::to_bits)).collect();
    w[2] = g.frame;
    w
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloudscape::field::Lod;
    use crate::cloudscape::noise::NoiseBank;
    use crate::cloudscape::storm::storm_cells;
    use crate::cloudscape::weather::{CloudWeather, Preset};
    use glam::{Vec2, Vec3};

    fn bank() -> &'static NoiseBank {
        static B: std::sync::OnceLock<NoiseBank> = std::sync::OnceLock::new();
        B.get_or_init(|| NoiseBank::bake_sized(0xC10D, 32, 16, 128))
    }

    #[test]
    fn layout_fits() {
        assert_eq!(STORM_ROW0, 30);
        assert_eq!(REPROJECT_ROW0, 94);
        assert_eq!(CLOUD_WORDS, 400);
    }

    /// The shader's camera-relative weather uv (`uv_cam + axes * rel / tile`, f32) lands on the
    /// same weather texel as the CPU's f64 evaluation, far from the origin and late in the day.
    #[test]
    fn camera_relative_uv_matches_f64() {
        let w = CloudWeather::preset(Preset::Scattered);
        let layers = w.layers();
        let t = 86_400.0 * 3.0 + 1234.5;
        let f = CloudField::new(bank(), layers, Vec::new(), t, 0.0, w.seed);
        let cam = DVec3::new(731_000.25, 1500.0, -2_480_000.75);
        let en_cam = DVec2::new(cam.x, -cam.z);
        let lf = &f.frames[0];
        let r = layer_rows(lf, en_cam, 0);
        for rel in [DVec2::new(0.0, 0.0), DVec2::new(35_000.0, -12_000.0), DVec2::new(-90_000.0, 70_000.0)] {
            let a = Vec2::new(r[2][0], r[2][1]);
            let b = Vec2::new(r[2][2], r[2][3]);
            let relf = rel.as_vec2();
            let gpu = Vec2::new(r[3][0], r[3][1]) + Vec2::new(a.dot(relf), b.dot(relf)) * r[5][3];
            let p = en_cam + rel - lf.drift;
            let q = DVec2::new(lf.axes[0].as_dvec2().dot(p), lf.axes[1].as_dvec2().dot(p));
            let cpu = (q / lf.weather_tile_m + lf.fade_off.0.as_dvec2()).as_vec2();
            let d = (gpu - cpu) - (gpu - cpu).round();
            // Under a hundredth of a weather texel.
            assert!(d.abs().max_element() < 1.0 / 512.0 / 100.0, "{rel:?}: {d:?}");
        }
    }

    #[test]
    fn storms_and_flags_pack() {
        let w = CloudWeather::preset(Preset::Thunderstorm);
        let layers = w.layers();
        let storms = storm_cells(&w, &layers[0], DVec2::ZERO, 120_000.0, 600.0);
        let n = storms.len();
        let f = CloudField::new(bank(), layers, storms, 600.0, 0.0, w.seed);
        let g = GpuFrame { field: &f, camera: DVec3::new(100.0, 2.0, -50.0), camera_alt_m: 2.0, planet_radius_m: crate::cloudscape::EARTH_RADIUS_M, cfg: MarchCfg::high(), frame: 77, debug: 0 };
        let words = cloud_words(&g);
        assert_eq!(words.len(), CLOUD_WORDS);
        assert_eq!(words[2], 77);
        let fl = |i: usize| f32::from_bits(words[i]);
        assert_eq!(fl(0), 1.0);
        assert_eq!(fl(5 * 4) as usize, n.min(MAX_STORMS));
        let s0 = f.storms[0];
        assert_eq!(fl(STORM_ROW0 * 4), (s0.x - 100.0) as f32);
        assert_eq!(fl(STORM_ROW0 * 4 + 1), (s0.z + 50.0) as f32);
        // An empty sky packs "off".
        let clear = CloudWeather::preset(Preset::Clear);
        let fc = CloudField::new(bank(), clear.layers(), Vec::new(), 0.0, 0.0, clear.seed);
        let gc = GpuFrame { field: &fc, ..g };
        assert_eq!(f32::from_bits(cloud_words(&gc)[0]), 0.0);
        let _ = (Lod::FULL, Vec3::ZERO);
    }

    /// Every constant the shader mirrors has the same value here (`clouds.slang` is the only
    /// other place the cloud maths lives).
    #[test]
    fn shader_constants_match() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../ed_render/shaders/clouds.slang");
        let Ok(src) = std::fs::read_to_string(path) else {
            panic!("{path} missing");
        };
        use crate::cloudscape::{field as fd, light as lt, noise as nz};
        let want: &[(&str, f64)] = &[
            ("SIGMA_MAX", fd::SIGMA_MAX as f64),
            ("SIGMA_RAIN", fd::SIGMA_RAIN as f64),
            ("RAIN_ALBEDO", fd::RAIN_ALBEDO as f64),
            ("SIGMA_INTERIOR", fd::SIGMA_INTERIOR as f64),
            ("CARVE", fd::CARVE as f64),
            ("EDGE_RAMP", fd::EDGE_RAMP as f64),
            ("SHAPE_PERIOD_M", nz::SHAPE_PERIOD_M),
            ("DETAIL_PERIOD_M", nz::DETAIL_PERIOD_M),
            ("MS_A", lt::MS_A as f64),
            ("MS_B", lt::MS_B as f64),
            ("MS_C", lt::MS_C as f64),
            ("G_FWD", lt::G_FWD as f64),
            ("G_BACK", lt::G_BACK as f64),
            ("BACK_W", lt::BACK_W as f64),
        ];
        for (name, v) in want {
            let line = src.lines().find(|l| l.contains(&format!("static const float CL_{name} ="))).unwrap_or_else(|| panic!("CL_{name} not in clouds.slang"));
            let num: f64 = line.split('=').nth(1).unwrap().trim().trim_end_matches(';').trim_end_matches('f').parse().unwrap();
            assert!((num - v).abs() < 1e-6 * v.abs().max(1.0), "CL_{name}: shader {num}, Rust {v}");
        }
        // The light steps table, row by row.
        for (i, (d, l)) in lt::LIGHT_STEPS.iter().enumerate() {
            assert!(src.contains(&format!("float2({d:.1}, {l:.1})")), "light step {i} ({d}, {l}) differs in clouds.slang");
        }
    }
}
