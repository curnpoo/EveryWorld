//! Climate/biome classification from real climate normals (WorldClim 2.1 via
//! `ed_data::climate`): temperature with lapse-rate cooling, coldest-month
//! temperature, aridity and the dry-summer signal. Biomes change on geological
//! time scales, so this is static data, separate from the weather sim. Where
//! the climate grid has no data (poles, open sea) the old latitude/box
//! heuristics are the fallback.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Biome {
    TropicalRainforest,
    Savanna,
    HotDesert,
    Mediterranean,
    TemperateForest,
    Grassland,
    /// Semi-arid: dry grass, shrubs, scattered trees.
    Steppe,
    Boreal,
    Tundra,
    Alpine,
    IceCap,
}

/// Approximate mean annual temperature (°C) at sea level by latitude.
fn sea_level_temp(lat: f64) -> f64 {
    let a = lat.abs();
    27.0 - 0.0055 * a * a - 0.12 * a
}

fn in_box(lat: f64, lon: f64, la0: f64, la1: f64, lo0: f64, lo1: f64) -> bool {
    (la0..=la1).contains(&lat) && (lo0..=lo1).contains(&lon)
}

/// Is this one of Earth's large arid regions?
pub fn is_arid(lat: f64, lon: f64) -> bool {
    in_box(lat, lon, 15.0, 32.0, -17.0, 35.0)        // Sahara
        || in_box(lat, lon, 12.0, 32.0, 35.0, 60.0)  // Arabia
        || in_box(lat, lon, 25.0, 38.0, 55.0, 72.0)  // Iran/Afghanistan/Pakistan
        || in_box(lat, lon, 36.0, 46.0, 80.0, 112.0) // Taklamakan/Gobi
        || in_box(lat, lon, -32.0, -19.0, 117.0, 142.0) // Australian interior
        || in_box(lat, lon, -28.0, -15.0, -71.5, -68.0) // Atacama
        || in_box(lat, lon, -29.0, -17.0, 12.0, 22.0) // Namib/Kalahari
        || in_box(lat, lon, 29.0, 37.5, -117.0, -108.0) // Mojave/Sonoran
}

pub fn is_mediterranean(lat: f64, lon: f64) -> bool {
    in_box(lat, lon, 30.0, 45.0, -10.0, 37.0)
        || in_box(lat, lon, 32.0, 42.0, -124.5, -116.0) // California
        || in_box(lat, lon, -38.0, -30.0, -73.0, -70.0) // central Chile
        || in_box(lat, lon, -35.0, -31.0, 115.0, 120.0) // SW Australia
        || in_box(lat, lon, -35.0, -32.0, 17.0, 21.0) // Cape Town
}

impl Biome {
    /// Biome from climate normals at `elevation_m`. `None` when the grid has
    /// no land data there.
    pub fn from_climate(lat: f64, lon: f64, elevation_m: f64) -> Option<Biome> {
        let c = ed_data::climate::Climate::at(lat, lon)?;
        let e = elevation_m.max(0.0) as f32;
        let (t, tmin, a) = (c.temp_at(e), c.tmin_at(e), c.aridity(e));
        let treeline = (3_800.0 - 45.0 * (lat.abs() - 20.0).max(0.0)).clamp(400.0, 4_200.0);
        if elevation_m > treeline && lat.abs() < 62.0 {
            return Some(Biome::Alpine);
        }
        if t < -18.0 {
            return Some(Biome::IceCap);
        }
        if t < -10.0 {
            return Some(Biome::Tundra);
        }
        if a < 0.15 {
            return Some(Biome::HotDesert); // all deserts; hot or cold is decided by temperature in colour
        }
        if a < 0.30 && t > 2.0 {
            return Some(Biome::Steppe);
        }
        if t < 5.0 {
            return Some(Biome::Boreal);
        }
        if tmin > 15.0 {
            // Frost-free tropics: wet all year is forest, strongly seasonal is savanna.
            return Some(if c.precip_mm > 1_800.0 && a > 1.2 && !c.dry_summer() { Biome::TropicalRainforest } else { Biome::Savanna });
        }
        if c.dry_summer() && tmin > -3.0 && a < 1.3 {
            return Some(Biome::Mediterranean);
        }
        if a < 0.55 && t > 4.0 {
            return Some(Biome::Grassland);
        }
        Some(Biome::TemperateForest)
    }

    pub fn classify(lat: f64, lon: f64, elevation_m: f64) -> Biome {
        Self::from_climate(lat, lon, elevation_m).unwrap_or_else(|| Self::classify_heuristic(lat, lon, elevation_m))
    }

    /// Latitude-band/box guess, used only where there is no climate data.
    pub fn classify_heuristic(lat: f64, lon: f64, elevation_m: f64) -> Biome {
        let t = sea_level_temp(lat) - 0.0065 * elevation_m.max(0.0);
        // Mountains: alpine above the local treeline (glaciers on peaks come
        // from the land-cover `ice` class, not from here).
        let treeline = (3_800.0 - 45.0 * (lat.abs() - 20.0).max(0.0)).clamp(400.0, 4_200.0);
        if elevation_m > treeline && lat.abs() < 62.0 {
            return Biome::Alpine;
        }
        if t < -12.0 {
            return Biome::IceCap;
        }
        if t < -4.0 {
            return Biome::Tundra;
        }
        if is_arid(lat, lon) {
            return Biome::HotDesert;
        }
        if t < 4.0 {
            return Biome::Boreal;
        }
        if is_mediterranean(lat, lon) {
            return Biome::Mediterranean;
        }
        if lat.abs() < 10.0 {
            return Biome::TropicalRainforest;
        }
        if lat.abs() < 20.0 {
            return Biome::Savanna;
        }
        Biome::TemperateForest
    }

    /// Altitude above which snow lies year-round.
    pub fn snowline_m(lat: f64) -> f32 {
        (5_200.0 - 60.0 * (lat.abs() as f32 - 20.0).max(0.0)).clamp(0.0, 5_500.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places() {
        assert_eq!(Biome::classify(45.97, 7.66, 4_000.0), Biome::Alpine); // Matterhorn
        assert_eq!(Biome::classify(23.4, 25.6, 500.0), Biome::HotDesert); // Sahara
        assert_eq!(Biome::classify(43.5, 5.4, 150.0), Biome::Mediterranean); // Aix-en-Provence
        assert_eq!(Biome::classify(40.7, -74.0, 10.0), Biome::TemperateForest); // NYC
        assert_eq!(Biome::classify(-3.1, -60.0, 50.0), Biome::TropicalRainforest); // Manaus
        assert_eq!(Biome::classify(64.1, -21.9, 30.0), Biome::Boreal); // Reykjavik-ish
        assert_eq!(Biome::classify(-80.0, 0.0, 2_500.0), Biome::IceCap); // Antarctica
    }
}
