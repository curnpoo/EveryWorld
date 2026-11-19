//! Clouds in every world (Phase 2 item 5): the weather the sky is made from, the baked cloud
//! noise, and the per-frame words for the renderer's cloud passes (`ed_render::clouds`).
//! The cloud science is `ed_world::cloudscape`; this is the glue.
//!
//! Weather: until the climate sim (`ed_climate`, O3-W4) fills [`CloudWeather`], it comes from
//! `EW_WEATHER`: a preset name (`clear fair scattered towering thunderstorm overcast frontal
//! cirrus layered`, default `scattered`), or `FROM>TO:SECONDS` to blend from one to the other
//! over that much sim time (a storm building, a front arriving). The low-level wind is the
//! trees' and grass's wind field (O3-W1), so clouds, trees and grass move with one weather.
//!
//! Switches: `EW_CLOUDS=0` (renderer side, passes off), `EW_CLOUDS_DEBUG=transmittance|depth|shadow`,
//! `EW_CLOUDS_STEPS=n` (march steps, default the High tier's 160).

use ed_render::{CloudNoise, NoiseTexture, Renderer};
use ed_world::cloudscape::gpu::{cloud_words, GpuFrame};
use ed_world::cloudscape::light::MarchCfg;
use ed_world::cloudscape::noise::{f16_bytes, mips_2d, mips_3d};
use ed_world::cloudscape::storm::storm_cells;
use ed_world::cloudscape::weather::{CloudWeather, Preset};
use ed_world::cloudscape::{CloudField, NoiseBank, EARTH_RADIUS_M};
use ed_world::wind::WindField;
use glam::{DVec2, Vec2};
use std::sync::mpsc::{Receiver, TryRecvError};

/// World seed of the cloud noise (fixed: every client bakes the same bank).
pub const CLOUD_SEED: u64 = 0xC10D;
/// Storms farther than this are not sent (m).
const STORM_RANGE_M: f64 = 120_000.0;

/// The weather over time: one preset, or a blend from one to another.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeatherPlan {
    pub from: CloudWeather,
    pub to: CloudWeather,
    /// Sim seconds the blend takes (0: `from` only).
    pub over_s: f64,
}

impl WeatherPlan {
    /// `EW_WEATHER`; `scattered` when unset or unknown (with a message).
    pub fn from_env() -> Self {
        let v = std::env::var("EW_WEATHER").unwrap_or_default();
        Self::parse(&v).unwrap_or_else(|| {
            if !v.is_empty() {
                eprintln!("ew: EW_WEATHER={v} not understood (a preset, or FROM>TO:SECONDS); using scattered");
            }
            Self::fixed(Preset::Scattered)
        })
    }

    pub fn fixed(p: Preset) -> Self {
        let w = CloudWeather::preset(p);
        Self { from: w, to: w, over_s: 0.0 }
    }

    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }
        match s.split_once('>') {
            None => Preset::from_name(s).map(Self::fixed),
            Some((a, rest)) => {
                let (b, secs) = rest.split_once(':').unwrap_or((rest, "600"));
                Some(Self { from: CloudWeather::preset(Preset::from_name(a)?), to: CloudWeather::preset(Preset::from_name(b)?), over_s: secs.parse().ok()? })
            }
        }
    }

    /// The weather at sim time `t_s`.
    pub fn at(&self, t_s: f64) -> CloudWeather {
        if self.over_s <= 0.0 {
            return self.from;
        }
        self.from.lerp(&self.to, (t_s / self.over_s).clamp(0.0, 1.0) as f32)
    }
}

enum Bank {
    Baking(Receiver<NoiseBank>),
    Ready(Box<NoiseBank>),
    Failed,
}

/// The sky of one world: weather, noise, and what the renderer gets each frame.
pub struct Sky {
    pub plan: WeatherPlan,
    bank: Bank,
    uploaded: bool,
    frame: u32,
    debug: u32,
    cfg: MarchCfg,
    /// Ground altitude the cloud bases sit over (m), fixed at the first frame so bases do not
    /// move with the camera over hills.
    ground_m: Option<f32>,
    wind: Vec2,
}

impl Sky {
    /// `sync`: bake the noise now (headless captures, deterministic); otherwise on a worker
    /// thread, and clouds appear when it is done (~2-3 s release).
    pub fn new(sync: bool) -> Self {
        let bank = if sync {
            Bank::Ready(Box::new(NoiseBank::bake(CLOUD_SEED)))
        } else {
            let (tx, rx) = std::sync::mpsc::channel();
            let spawned = std::thread::Builder::new().name("cloud noise".into()).spawn(move || {
                let _ = tx.send(NoiseBank::bake(CLOUD_SEED));
            });
            if spawned.is_ok() { Bank::Baking(rx) } else { Bank::Failed }
        };
        let debug = match std::env::var("EW_CLOUDS_DEBUG").as_deref() {
            Ok("transmittance") | Ok("1") => 1,
            Ok("depth") | Ok("2") => 2,
            Ok("shadow") | Ok("3") => 3,
            _ => 0,
        };
        let mut cfg = MarchCfg::high();
        if let Some(n) = std::env::var("EW_CLOUDS_STEPS").ok().and_then(|v| v.parse::<u32>().ok()) {
            cfg.max_steps = n.clamp(16, 512);
        }
        let w = WindField::default().base;
        Self { plan: WeatherPlan::from_env(), bank, uploaded: false, frame: 0, debug, cfg, ground_m: None, wind: Vec2::new(w.x as f32, w.y as f32) }
    }

    /// The baked bank, once ready.
    fn bank(&mut self) -> Option<&NoiseBank> {
        if let Bank::Baking(rx) = &self.bank {
            match rx.try_recv() {
                Ok(b) => self.bank = Bank::Ready(Box::new(b)),
                Err(TryRecvError::Empty) => return None,
                Err(TryRecvError::Disconnected) => self.bank = Bank::Failed,
            }
        }
        match &self.bank {
            Bank::Ready(b) => Some(b),
            _ => None,
        }
    }

    /// Feeds the renderer for this frame: uploads the noise once it is baked, then the words
    /// for the camera at `camera` (render frame) and sim time `t_s`. Call after
    /// `ed_render::atmosphere::apply_camera_ground` (the camera's altitude and up come from it).
    pub fn update(&mut self, r: &mut Renderer, dev: &dyn ed_gpu::Device, camera: glam::DVec3, t_s: f64) {
        let ground = r.atmosphere().ground;
        let (cam_alt, ground_alt) = match ground {
            Some(g) => (g.camera_alt_m, g.ground_alt_m),
            None => (camera.y, Some(0.0)),
        };
        if self.ground_m.is_none() {
            self.ground_m = ground_alt.map(|g| g as f32);
        }
        let frame = self.frame;
        self.frame = self.frame.wrapping_add(1);
        let (debug, cfg, wind, ground_m, plan) = (self.debug, self.cfg, self.wind, self.ground_m.unwrap_or(0.0), self.plan);
        let uploaded = self.uploaded;
        let Some(bank) = self.bank() else {
            r.set_clouds(None);
            return;
        };
        if !uploaded {
            let tex3 = |v: &ed_world::cloudscape::noise::Volume| NoiseTexture { size: v.n as u32, mips: mips_3d(v).iter().map(|m| f16_bytes(m)).collect() };
            let noise = CloudNoise {
                shape: tex3(&bank.shape),
                detail: tex3(&bank.detail),
                weather: NoiseTexture { size: bank.weather.n as u32, mips: mips_2d(&bank.weather).iter().map(|m| f16_bytes(m)).collect() },
            };
            if let Err(e) = r.set_cloud_noise(dev, &noise) {
                eprintln!("ew: clouds off: noise upload failed: {e}");
                self.bank = Bank::Failed;
                r.set_clouds(None);
                return;
            }
            self.uploaded = true;
        }
        let Some(bank) = self.bank() else { return };
        let mut weather = plan.at(t_s).with_surface_wind(wind);
        weather.ground_m = ground_m;
        let layers = weather.layers();
        let storms = storm_cells(&weather, &layers[0], DVec2::new(camera.x, camera.z), STORM_RANGE_M, t_s);
        let field = CloudField::new(bank, layers, storms, t_s, weather.ground_m, weather.seed);
        let words = cloud_words(&GpuFrame { field: &field, camera, camera_alt_m: cam_alt, planet_radius_m: EARTH_RADIUS_M, cfg, frame, debug });
        r.set_clouds(Some(words));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weather_plans_parse_and_blend() {
        assert_eq!(WeatherPlan::parse("towering"), Some(WeatherPlan::fixed(Preset::Towering)));
        assert!(WeatherPlan::parse("nonsense").is_none());
        let p = WeatherPlan::parse("fair>thunderstorm:1200").unwrap();
        assert_eq!(p.over_s, 1200.0);
        assert_eq!(p.at(0.0), CloudWeather::preset(Preset::Fair));
        assert_eq!(p.at(5000.0), CloudWeather::preset(Preset::Thunderstorm));
        let mid = p.at(600.0);
        assert!(mid.instability > 0.3 && mid.instability < 0.8);
        assert_eq!(WeatherPlan::parse("fair>overcast").unwrap().over_s, 600.0);
    }
}
