//! Procedural soundscape for EveryWorld (`docs/plans/sound.md`).
//!
//! Engine-free: the game fills a `Params` snapshot each frame and fires
//! `Event`s; the audio thread pulls interleaved stereo from `Scape`. No audio
//! files: everything is synthesised from noise, filters and oscillators, the
//! same way `ed_engine_audio` does engines.

pub mod dsp;
pub mod params;
pub mod parts;
pub mod recipes;
pub mod scape;

pub use params::{Event, Params, Surface};
pub use scape::{Scape, ScapeHandle, channel};

use ed_world::biome::Biome;
use ed_world::foliage::TreeKind;
use params::{fauna, veg};

/// Which wind-through-leaves voice a species uses.
pub fn veg_class(k: TreeKind) -> usize {
    use TreeKind::*;
    match k {
        Conifer | Fir | Pine | Juniper => veg::CONIFER,
        Cypress => veg::CYPRESS,
        Broadleaf | Maple | PlaneTree | Baobab => veg::BROADLEAF,
        Birch => veg::BIRCH,
        Olive | Oleander => veg::OLIVE,
        Palm | DatePalm => veg::PALM,
        Banana => veg::BANANA,
        Acacia => veg::SPARSE,
        Cactus | PricklyPear => veg::CACTUS,
        Shrub | Boxwood | FloweringBush | Fern => veg::SHRUB,
        GrassTussock => veg::GRASS,
    }
}

/// Context for choosing animals.
#[derive(Clone, Copy, Debug, Default)]
pub struct FaunaCtx {
    /// 0 night .. 1 full day.
    pub daylight: f32,
    /// Peaks at dawn and dusk (dawn chorus).
    pub twilight: f32,
    pub temp_c: f32,
    /// 0..1 how close standing / slow water is.
    pub near_water: f32,
    pub wetland: f32,
    pub coastal: f32,
    /// Mountain meadow with grazing (cowbells).
    pub pasture: f32,
    /// Frozen lake nearby (ice booms).
    pub frozen_lake: f32,
    /// 0..1 how built-up (cities scare most wildlife away, not pigeons/gulls).
    pub urban: f32,
}

/// Animal mix (weights per `fauna::*`) for a biome, time of day and place.
pub fn fauna_mix(b: Biome, c: FaunaCtx) -> [f32; fauna::N] {
    use Biome::*;
    let mut f = [0.0; fauna::N];
    let warm = dsp::smoothstep(8.0, 16.0, c.temp_c);
    let hot = dsp::smoothstep(22.0, 30.0, c.temp_c);
    let night = 1.0 - c.daylight;
    let wild = 1.0 - 0.75 * c.urban;
    // birds: (day weight, how much they need trees)
    let birds = match b {
        TropicalRainforest => 1.0,
        TemperateForest | Mediterranean | Boreal => 0.8,
        Savanna | Grassland | Steppe => 0.5,
        Alpine | Tundra => 0.25,
        HotDesert => 0.15,
        IceCap => 0.0,
    };
    f[fauna::BIRDS] = birds * (0.35 * c.daylight + 1.2 * c.twilight).min(1.0) * warm.max(0.2) * (1.0 - 0.5 * c.urban);
    f[fauna::DRONE] = match b {
        TropicalRainforest => 1.0,
        Savanna => 0.4,
        _ => 0.0,
    } * (0.5 + 0.5 * night)
        * wild;
    f[fauna::CICADAS] = match b {
        Mediterranean | Savanna | TropicalRainforest => 1.0,
        TemperateForest => 0.4,
        HotDesert | Steppe => 0.3,
        _ => 0.0,
    } * c.daylight
        * hot
        * wild;
    f[fauna::CRICKETS] = match b {
        IceCap | Tundra | Alpine | Boreal => 0.2,
        HotDesert => 0.4,
        _ => 1.0,
    } * night
        * dsp::smoothstep(12.0, 16.0, c.temp_c) // Dolbear stops near 13 °C
        * wild;
    f[fauna::FROGS] = (c.wetland.max(0.6 * c.near_water)) * night.max(0.3 * c.twilight) * warm * wild;
    f[fauna::OWLS] = match b {
        TemperateForest | Boreal | Mediterranean => 1.0,
        TropicalRainforest | Grassland => 0.5,
        _ => 0.15,
    } * night
        * wild;
    f[fauna::GULLS] = c.coastal * (0.3 + 0.7 * c.daylight) * (1.0 - 0.4 * (b == IceCap) as u8 as f32);
    f[fauna::COWBELLS] = c.pasture * c.daylight * wild;
    f[fauna::ICE] = c.frozen_lake * (1.0 - warm);
    f[fauna::LOONS] = match b {
        Boreal | Tundra => 1.0,
        TemperateForest => 0.3,
        _ => 0.0,
    } * c.near_water
        * (night + c.twilight).min(1.0)
        * warm.max(0.4);
    for x in &mut f {
        *x = x.clamp(0.0, 1.0);
    }
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tree_kind_has_a_voice() {
        for k in TreeKind::ALL {
            assert!(veg_class(*k) < veg::N);
        }
    }

    #[test]
    fn crickets_at_warm_nights_cicadas_at_hot_days() {
        let night = FaunaCtx { daylight: 0.0, temp_c: 22.0, ..Default::default() };
        let day = FaunaCtx { daylight: 1.0, temp_c: 30.0, ..Default::default() };
        let n = fauna_mix(Biome::Mediterranean, night);
        let d = fauna_mix(Biome::Mediterranean, day);
        assert!(n[fauna::CRICKETS] > 0.8 && d[fauna::CRICKETS] < 0.01);
        assert!(d[fauna::CICADAS] > 0.8 && n[fauna::CICADAS] < 0.01);
        let cold = fauna_mix(Biome::Mediterranean, FaunaCtx { temp_c: 5.0, ..night });
        assert!(cold[fauna::CRICKETS] < 0.01, "no crickets below ~13 °C");
    }

    #[test]
    fn ice_cap_is_nearly_silent() {
        let c = FaunaCtx { daylight: 1.0, twilight: 0.5, temp_c: -20.0, ..Default::default() };
        let f = fauna_mix(Biome::IceCap, c);
        assert!(f.iter().sum::<f32>() < 0.1);
    }
}
