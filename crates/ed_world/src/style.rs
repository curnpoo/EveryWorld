//! Regional architecture. Picks how a building *looks* from where it is.
//!
//! This is deliberately a data table: adding "Kyoto machiya" or "Lagos
//! compound" is one entry, not new code. Long term this moves to RON/JSON
//! files under `assets/styles/` so it can be tuned without recompiling.

use crate::biome::Biome;
use crate::mesh::rgb;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoofKind {
    Flat,
    Gable,
    Hip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Region {
    NorthAmericaCity,
    NorthAmericaSuburb,
    LatinAmerica,
    Mediterranean,
    Paris,
    WesternEurope,
    Alpine,
    British,
    Nordic,
    EasternEurope,
    NorthAfricaMiddleEast,
    SubSaharan,
    SouthAsia,
    EastAsia,
    Japan,
    SoutheastAsia,
    Oceania,
}

#[derive(Debug, Clone)]
pub struct RegionStyle {
    pub walls: &'static [u32],
    pub roofs: &'static [u32],
    /// Roof shapes for low-rise buildings, weighted by repetition.
    pub roof_kinds: &'static [RoofKind],
    /// Roof rise per meter of half-width.
    pub pitch: f32,
    pub floor_height: f32,
    /// Default storeys when OSM has no height.
    pub default_floors: (u8, u8),
    /// Ground-floor shopfront probability in commercial/dense areas.
    pub shopfront: f32,
    pub awnings: &'static [u32],
}

/// What a single building ends up looking like.
#[derive(Debug, Clone, Copy)]
pub struct BuildingLook {
    pub wall: [f32; 4],
    pub roof: [f32; 4],
    pub roof_kind: RoofKind,
    pub pitch: f32,
    pub floor_height: f32,
    pub shopfront: bool,
    pub awning: [f32; 4],
    /// Glass curtain wall (towers).
    pub glassy: bool,
}

fn in_box(lat: f64, lon: f64, la0: f64, la1: f64, lo0: f64, lo1: f64) -> bool {
    (la0..=la1).contains(&lat) && (lo0..=lo1).contains(&lon)
}

impl Region {
    pub const ALL: [Region; 17] = [
        Region::NorthAmericaCity,
        Region::NorthAmericaSuburb,
        Region::LatinAmerica,
        Region::Mediterranean,
        Region::Paris,
        Region::WesternEurope,
        Region::Alpine,
        Region::British,
        Region::Nordic,
        Region::EasternEurope,
        Region::NorthAfricaMiddleEast,
        Region::SubSaharan,
        Region::SouthAsia,
        Region::EastAsia,
        Region::Japan,
        Region::SoutheastAsia,
        Region::Oceania,
    ];

    pub fn locate(lat: f64, lon: f64, elevation_m: f64, dense: bool) -> Region {
        use Region::*;
        if in_box(lat, lon, 48.75, 48.95, 2.2, 2.47) {
            return Paris;
        }
        if in_box(lat, lon, 45.5, 48.0, 5.8, 16.5) && elevation_m > 600.0 {
            return Alpine;
        }
        if in_box(lat, lon, 30.0, 46.5, 129.0, 146.0) {
            return Japan;
        }
        if in_box(lat, lon, 15.0, 72.0, -170.0, -52.0) && !in_box(lat, lon, 15.0, 32.5, -118.0, -86.0) {
            return if dense { NorthAmericaCity } else { NorthAmericaSuburb };
        }
        if in_box(lat, lon, -56.0, 32.5, -118.0, -30.0) {
            return LatinAmerica;
        }
        if in_box(lat, lon, 49.8, 59.5, -8.5, 1.8) {
            return British;
        }
        if in_box(lat, lon, 55.0, 71.5, 4.0, 32.0) || in_box(lat, lon, 63.0, 67.0, -25.0, -13.0) {
            return Nordic;
        }
        if in_box(lat, lon, 35.0, 45.5, -10.0, 36.0) || in_box(lat, lon, 43.0, 45.5, 3.0, 8.0) {
            return Mediterranean;
        }
        if in_box(lat, lon, 45.0, 55.0, -5.5, 15.0) {
            return WesternEurope;
        }
        if in_box(lat, lon, 42.0, 70.0, 15.0, 180.0) {
            return EasternEurope;
        }
        if in_box(lat, lon, 12.0, 42.0, -18.0, 63.0) {
            return NorthAfricaMiddleEast;
        }
        if in_box(lat, lon, -35.0, 12.0, -18.0, 52.0) {
            return SubSaharan;
        }
        if in_box(lat, lon, 5.0, 37.0, 60.0, 92.0) {
            return SouthAsia;
        }
        if in_box(lat, lon, 18.0, 54.0, 92.0, 135.0) {
            return EastAsia;
        }
        if in_box(lat, lon, -11.0, 18.0, 92.0, 141.0) {
            return SoutheastAsia;
        }
        if in_box(lat, lon, -48.0, -10.0, 112.0, 180.0) {
            return Oceania;
        }
        if dense { NorthAmericaCity } else { WesternEurope }
    }

    pub fn style(self) -> RegionStyle {
        use RoofKind::*;
        match self {
            Region::NorthAmericaCity => RegionStyle {
                walls: &[0xa8553f, 0x9b4a3a, 0xc9a77c, 0xb86b4b, 0xd8cdb8, 0x8c5a48, 0xe0d6c4],
                roofs: &[0x6b6b6b, 0x5a5a5e, 0x7a756e],
                roof_kinds: &[Flat],
                pitch: 0.0,
                floor_height: 3.6,
                default_floors: (3, 6),
                shopfront: 0.7,
                awnings: &[0x1f8f86, 0x2c6fbf, 0xe0a800, 0xc23b8f, 0x2a2a2a, 0xc0392b],
            },
            Region::NorthAmericaSuburb => RegionStyle {
                walls: &[0xe8e2d4, 0xc9d6df, 0xd9c9a8, 0xf2efe6, 0xb8c4a8, 0xd6b8a0],
                roofs: &[0x4a4a4f, 0x5c534b, 0x3e4651, 0x6b5e52],
                roof_kinds: &[Gable, Gable, Hip],
                pitch: 0.5,
                floor_height: 3.0,
                default_floors: (1, 2),
                shopfront: 0.3,
                awnings: &[0x2c6fbf, 0xc0392b, 0x2a2a2a],
            },
            Region::LatinAmerica => RegionStyle {
                walls: &[0xf2c14e, 0xe76f51, 0x2a9d8f, 0xf4a3b4, 0x8ecae6, 0xf1faee, 0xe9c46a],
                roofs: &[0xb5532c, 0x8f8f8f, 0xa64b2a],
                roof_kinds: &[Flat, Flat, Hip],
                pitch: 0.35,
                floor_height: 3.2,
                default_floors: (1, 3),
                shopfront: 0.6,
                awnings: &[0xe63946, 0x1d3557, 0xf4a261],
            },
            Region::Mediterranean => RegionStyle {
                walls: &[0xf1e3c6, 0xe8c99b, 0xf4ead5, 0xdeb887, 0xe9d2b0, 0xf6efe2],
                roofs: &[0xc0603b, 0xb5532c, 0xcc6f45],
                roof_kinds: &[Hip, Hip, Gable],
                pitch: 0.35,
                floor_height: 3.2,
                default_floors: (2, 3),
                shopfront: 0.4,
                awnings: &[0x2e7d6b, 0x355c7d, 0xb23a48],
            },
            Region::Paris => RegionStyle {
                walls: &[0xe9dfc9, 0xe2d6bd, 0xefe6d2],
                roofs: &[0x6e7b85, 0x5f6b74],
                roof_kinds: &[Hip],
                pitch: 0.9,
                floor_height: 3.3,
                default_floors: (5, 7),
                shopfront: 0.85,
                awnings: &[0xa4161a, 0x1b4332, 0x14213d, 0x6a040f],
            },
            Region::WesternEurope => RegionStyle {
                walls: &[0xe8dcc6, 0xb5654a, 0xd9cdb5, 0xf0e8da, 0xc9a882],
                roofs: &[0x9c4a32, 0x5b5f66, 0x8a3f2c],
                roof_kinds: &[Gable, Gable, Hip],
                pitch: 0.75,
                floor_height: 3.1,
                default_floors: (2, 4),
                shopfront: 0.5,
                awnings: &[0x264653, 0x9d0208, 0x3a5a40],
            },
            Region::Alpine => RegionStyle {
                walls: &[0x8b5a2b, 0xa0673a, 0xf2ece0, 0x7a4b25],
                roofs: &[0x4a4a4a, 0x5b4636, 0x3d3d3d],
                roof_kinds: &[Gable],
                pitch: 0.55,
                floor_height: 2.9,
                default_floors: (2, 3),
                shopfront: 0.2,
                awnings: &[0xb22222],
            },
            Region::British => RegionStyle {
                walls: &[0x9e4f3c, 0xb06a4f, 0xe6dfd0, 0x8b4636],
                roofs: &[0x4b4f57, 0x3f434a],
                roof_kinds: &[Gable, Gable, Hip],
                pitch: 0.7,
                floor_height: 3.0,
                default_floors: (2, 3),
                shopfront: 0.5,
                awnings: &[0x1b4332, 0x14213d, 0x7f1d1d],
            },
            Region::Nordic => RegionStyle {
                walls: &[0x9b2d20, 0xe8c547, 0xf0ece2, 0x5b7c99, 0xd9d2c0],
                roofs: &[0x3a3a3a, 0x7f2f22, 0x4a4a4a],
                roof_kinds: &[Gable],
                pitch: 0.7,
                floor_height: 3.0,
                default_floors: (2, 3),
                shopfront: 0.3,
                awnings: &[0x1d3557],
            },
            Region::EasternEurope => RegionStyle {
                walls: &[0xd8cfc0, 0xc7d3d9, 0xe6d3a3, 0xbfb6a8, 0xe0c9c0],
                roofs: &[0x6b6b6b, 0x8a3f2c, 0x4f6f52],
                roof_kinds: &[Flat, Gable, Hip],
                pitch: 0.6,
                floor_height: 2.9,
                default_floors: (3, 9),
                shopfront: 0.3,
                awnings: &[0x1d3557, 0x9d0208],
            },
            Region::NorthAfricaMiddleEast => RegionStyle {
                walls: &[0xe6d2a8, 0xd9b98a, 0xf3ede0, 0xc9a26b, 0xece4d4],
                roofs: &[0xd9c6a0, 0xcbb894],
                roof_kinds: &[Flat],
                pitch: 0.0,
                floor_height: 3.2,
                default_floors: (1, 4),
                shopfront: 0.5,
                awnings: &[0x2a9d8f, 0xe76f51, 0x264653],
            },
            Region::SubSaharan => RegionStyle {
                walls: &[0xe9dcc3, 0xc98a5a, 0xf2e8cf, 0x9fc5e8, 0xf6bd60],
                roofs: &[0x9aa3a8, 0x8a3f2c, 0xa6a6a6, 0x6b7b8c],
                roof_kinds: &[Hip, Gable, Flat],
                pitch: 0.25,
                floor_height: 3.0,
                default_floors: (1, 2),
                shopfront: 0.4,
                awnings: &[0x2a9d8f, 0xe63946, 0xf4a261],
            },
            Region::SouthAsia => RegionStyle {
                walls: &[0xf4d35e, 0xee964b, 0x95d5b2, 0xf28482, 0xf6f1e9, 0xa2d2ff],
                roofs: &[0xbdb5a6, 0xa8a090],
                roof_kinds: &[Flat],
                pitch: 0.0,
                floor_height: 3.0,
                default_floors: (2, 5),
                shopfront: 0.7,
                awnings: &[0x0077b6, 0xe63946, 0x2a9d8f],
            },
            Region::EastAsia => RegionStyle {
                walls: &[0xd9d9d6, 0xe8e4dc, 0xc4c9cc, 0xf0ede6],
                roofs: &[0x8c8c8c, 0x5d6a74],
                roof_kinds: &[Flat],
                pitch: 0.0,
                floor_height: 3.0,
                default_floors: (4, 12),
                shopfront: 0.7,
                awnings: &[0xd62828, 0x003049, 0xfcbf49],
            },
            Region::Japan => RegionStyle {
                walls: &[0xf0ece4, 0xd8d3c8, 0xc9c2b2, 0xe8e1d3, 0x8f7a63],
                roofs: &[0x3d4650, 0x4a4f57, 0x5b6770],
                roof_kinds: &[Gable, Hip, Flat],
                pitch: 0.5,
                floor_height: 3.0,
                default_floors: (2, 3),
                shopfront: 0.6,
                awnings: &[0x1d3557, 0xd62828, 0x2a2a2a],
            },
            Region::SoutheastAsia => RegionStyle {
                walls: &[0xf6f1e9, 0xffd6a5, 0xcaffbf, 0x9bf6ff, 0xffc6ff],
                roofs: &[0xb5532c, 0x7a8b99, 0x9aa3a8],
                roof_kinds: &[Hip, Flat],
                pitch: 0.4,
                floor_height: 3.0,
                default_floors: (2, 4),
                shopfront: 0.7,
                awnings: &[0xe63946, 0x0077b6],
            },
            Region::Oceania => RegionStyle {
                walls: &[0xb5654a, 0xe8e2d4, 0xd9c9a8, 0xf2efe6],
                roofs: &[0xa64b2a, 0x4a5562, 0x6b7b8c],
                roof_kinds: &[Hip, Hip, Gable],
                pitch: 0.4,
                floor_height: 3.0,
                default_floors: (1, 2),
                shopfront: 0.4,
                awnings: &[0x264653, 0xe76f51],
            },
        }
    }
}

/// Decide a building's look. `seed` must be stable per building (OSM id).
pub fn look_for(
    region: Region,
    _biome: Biome,
    seed: u64,
    height_m: f32,
    commercial: bool,
    osm_colour: Option<[f32; 4]>,
) -> BuildingLook {
    let st = region.style();
    let pick = |salt: u64, n: usize| ((crate::hash01(seed, salt) * n as f32) as usize).min(n - 1);
    let tower = height_m > 40.0;
    let glassy = tower && crate::hash01(seed, 7) < 0.55;
    let wall = if glassy {
        rgb([0x6f8fa6, 0x7d9bb0, 0x5f7f96, 0x8aa4b4, 0x4f6d7a][pick(8, 5)])
    } else if tower {
        rgb([0xd9d4cc, 0xc8c2b8, 0xb8b2a8, 0xe6e1d8][pick(8, 4)])
    } else {
        osm_colour.unwrap_or_else(|| rgb(st.walls[pick(1, st.walls.len())]))
    };
    let roof_kind = if tower || height_m > 25.0 { RoofKind::Flat } else { st.roof_kinds[pick(3, st.roof_kinds.len())] };
    BuildingLook {
        wall,
        roof: rgb(st.roofs[pick(2, st.roofs.len())]),
        roof_kind,
        pitch: st.pitch,
        floor_height: st.floor_height,
        shopfront: commercial && crate::hash01(seed, 4) < st.shopfront,
        awning: rgb(st.awnings[pick(5, st.awnings.len())]),
        glassy,
    }
}

/// Default height when OSM has none: storeys from the region's range.
pub fn default_height(region: Region, seed: u64) -> f32 {
    let st = region.style();
    let (lo, hi) = st.default_floors;
    let floors = lo as f32 + (crate::hash01(seed, 9) * (hi - lo + 1) as f32).floor();
    floors * st.floor_height
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regions() {
        assert_eq!(Region::locate(40.75, -73.98, 10.0, true), Region::NorthAmericaCity);
        assert_eq!(Region::locate(48.86, 2.35, 35.0, true), Region::Paris);
        assert_eq!(Region::locate(43.53, 5.45, 150.0, false), Region::Mediterranean); // Provence villa
        assert_eq!(Region::locate(46.02, 7.75, 1600.0, false), Region::Alpine); // Zermatt
        assert_eq!(Region::locate(31.63, -8.0, 450.0, true), Region::NorthAfricaMiddleEast); // Marrakech
        assert_eq!(Region::locate(35.68, 139.76, 20.0, true), Region::Japan);
        assert_eq!(Region::locate(19.43, -99.13, 2240.0, true), Region::LatinAmerica); // CDMX
        assert_eq!(Region::locate(51.5, -0.12, 20.0, true), Region::British);
    }

    #[test]
    fn deterministic() {
        let a = look_for(Region::Mediterranean, Biome::Mediterranean, 12345, 9.0, false, None);
        let b = look_for(Region::Mediterranean, Biome::Mediterranean, 12345, 9.0, false, None);
        assert_eq!(a.wall, b.wall);
        assert_eq!(a.roof_kind, b.roof_kind);
    }
}
