//! Car data, generated from `vendor/engine-sound-kit/cars.js` into
//! `data/cars.json` by `tools/gen_cars.mjs` (never retyped by hand). Every car
//! is stored resolved (`resolveCar`): its sound merged over `DEFAULT_SOUND`.
//!
//! Field names follow the JS (camelCase in JSON). Keys the JS reads with
//! `||` or `??` fallbacks are `Option` or default to 0 here, and the voice
//! applies the same fallbacks.

use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    pub default_sound: Sound,
    pub gearbox: std::collections::BTreeMap<String, Gearbox>,
    pub cars: Vec<Car>,
}

#[derive(Debug, Clone, Copy, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Gearbox {
    pub up: f64,
    pub down: f64,
    pub cut: f64,
    pub blip: f64,
    pub clutch: bool,
    pub cvt: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Car {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub group: String,
    #[serde(default)]
    pub tagline: String,
    /// "piston", "jet", "steam", "pod" or "warp".
    pub kind: String,
    /// 720 for a four-stroke, 360 for a two-stroke.
    pub cycle: f64,
    pub engine: Engine,
    /// [crank angle in degrees, exhaust bank] in firing order.
    pub firing: Vec<[f64; 2]>,
    pub sound: Sound,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Engine {
    pub top_mph: Option<f64>,
    pub mode: Option<String>,
    pub idle: f64,
    pub redline: f64,
    pub limiter: f64,
    pub launch: f64,
    pub rev_up: f64,
    pub rev_down: f64,
    pub shift: [f64; 2],
    pub start: Start,
    pub gearbox: String,
    pub peak_kw: f64,
    pub mass: f64,
    pub cd_a: f64,
    pub crr: f64,
    pub max_accel: f64,
    pub disp: Option<f64>,
    pub ratios: Vec<f64>,
    #[serde(rename = "final")]
    pub final_drive: f64,
    pub tire: f64,
    pub cyl: usize,
    pub down_rpm: Option<f64>,
    pub min_gear_s: Option<f64>,
    pub skip_shift: bool,
    pub fit_taller: bool,
}

#[derive(Debug, Clone, Copy, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Start {
    pub crank: f64,
    pub crank_rpm: f64,
    pub flare: f64,
    pub bark: f64,
    pub settle: f64,
}

#[derive(Debug, Clone, Copy, Deserialize, Default)]
#[serde(default)]
pub struct PipeDef {
    pub bank: usize,
    pub ms: f64,
    pub pan: f64,
}

#[derive(Debug, Clone, Copy, Deserialize, Default)]
#[serde(default)]
pub struct BodyDef {
    pub hz: f64,
    pub db: f64,
    pub q: f64,
}

#[derive(Debug, Clone, Copy, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct PulseDef {
    pub idle_ms: f64,
    pub top_ms: f64,
}

#[derive(Debug, Clone, Copy, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Muffler {
    pub closed_hz: f64,
    pub open_hz: f64,
}

#[derive(Debug, Clone, Copy, Deserialize, Default)]
#[serde(default)]
pub struct Intake {
    pub amt: f64,
    pub hz: f64,
    pub q: f64,
}

#[derive(Debug, Clone, Copy, Deserialize, Default)]
#[serde(default)]
pub struct Shelf {
    pub hz: f64,
    pub db: f64,
}

#[derive(Debug, Clone, Copy, Deserialize, Default)]
#[serde(default)]
pub struct Thump {
    pub amt: f64,
    pub hz: f64,
    pub q: f64,
    pub sub: f64,
}

#[derive(Debug, Clone, Copy, Deserialize, Default)]
#[serde(default)]
pub struct Starter {
    pub amt: f64,
    pub pitch: f64,
}

#[derive(Debug, Clone, Copy, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Bov {
    #[default]
    None,
    Bov,
    Flutter,
}

#[derive(Debug, Clone, Copy, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Turbo {
    pub amt: f64,
    pub spool_rpm: f64,
    pub spool_s: f64,
    pub damp: f64,
    pub whistle: f64,
    pub whistle_hz: [f64; 2],
    pub hiss: f64,
    pub bov: Bov,
    pub bov_amt: f64,
    pub psi: f64,
    pub whistle_twin: f64,
    pub whistle_fade: f64,
    pub whistle_idle: f64,
    pub whistle_pulse: f64,
    pub hiss_hz: f64,
    pub hiss_q: f64,
    pub rush: f64,
    pub bov_hz: Option<[f64; 2]>,
    pub bov_q: f64,
    pub bov_attack: f64,
    pub bov_decay: f64,
    /// JS `bovShift ?? 1`: absent means 1, 0 means "no vent on upshifts".
    pub bov_shift: Option<f64>,
}

#[derive(Debug, Clone, Copy, Deserialize, Default)]
#[serde(default)]
pub struct Ratio {
    pub amt: f64,
    pub ratio: f64,
}

#[derive(Debug, Clone, Copy, Deserialize, Default)]
#[serde(default)]
pub struct Amt {
    pub amt: f64,
}

#[derive(Debug, Clone, Copy, Deserialize, Default)]
#[serde(default)]
pub struct Gunshot {
    pub amt: f64,
    pub flame: f64,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Sound {
    pub pipes: Vec<PipeDef>,
    pub reflect: f64,
    pub damp: f64,
    pub cross: f64,
    pub header: Option<Vec<f64>>,
    pub pulse: PulseDef,
    pub noise: f64,
    pub rough: f64,
    pub trim: f64,
    pub lope: f64,
    pub idle_roll: f64,
    pub muffler: Muffler,
    pub body: Vec<BodyDef>,
    pub intake: Intake,
    pub bass: Shelf,
    pub thump: Thump,
    pub overrun_rough: f64,
    pub overrun_dark: f64,
    pub starter: Starter,
    pub mech: f64,
    pub turbo: Turbo,
    pub blower: Ratio,
    pub gear_whine: Ratio,
    pub crackle: f64,
    pub pop_amt: f64,
    pub bang: f64,
    pub bang_burst: [f64; 2],
    pub bang_gap_ms: f64,
    pub bang_crack: f64,
    pub jake: Amt,
    pub air_brake: f64,
    pub gunshot: Gunshot,
    pub antilag: f64,
    pub toon: f64,
    pub tone_hz: f64,
    pub drive: f64,
    pub gain: f64,
}

static CATALOG: OnceLock<Catalog> = OnceLock::new();

/// Every vehicle in the kit, parsed once from the embedded JSON.
pub fn catalog() -> &'static Catalog {
    CATALOG.get_or_init(|| serde_json::from_str(include_str!("../data/cars.json")).expect("data/cars.json is valid"))
}

/// A car by id (`"huracan"`, `"jet"`, ...).
pub fn car(id: &str) -> Option<&'static Car> {
    catalog().cars.iter().find(|c| c.id == id)
}

/// The car's index in the catalog (what the mixer's commands carry).
pub fn car_index(id: &str) -> Option<usize> {
    catalog().cars.iter().position(|c| c.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_loads_every_car() {
        let c = catalog();
        assert_eq!(c.cars.len(), 31);
        let h = car("huracan").unwrap();
        assert_eq!(h.firing.len(), 10);
        assert_eq!(h.sound.turbo.bov, Bov::Bov);
        assert_eq!(h.sound.turbo.bov_shift, Some(0.0));
        assert!(car("gt3").unwrap().sound.turbo.bov_shift.is_none());
        assert_eq!(car("jet").unwrap().kind, "jet");
        assert_eq!(car("moped").unwrap().cycle, 360.0);
        assert!(c.gearbox["manual"].clutch);
    }
}
