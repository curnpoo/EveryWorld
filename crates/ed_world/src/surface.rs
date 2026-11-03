//! Ground cover. Rasterizes OpenMapTiles `landcover`, `landuse`, `park` and
//! `water` polygons into a small class grid per tile, then turns
//! class + biome + slope + altitude into a stylized ground color and foliage
//! density.

use crate::biome::Biome;
use crate::mesh::{lerp_color, rgb, scale_color};
use ed_data::mvt::{self, GeomType};
use ed_geo::tile::TileId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Cover {
    /// Nothing mapped: fall back to biome default.
    Natural = 0,
    Wood,
    Grass,
    Farmland,
    Wetland,
    Sand,
    Rock,
    Ice,
    Residential,
    Commercial,
    Industrial,
    Park,
    Pitch,
    Water,
}

impl Cover {
    fn from_omt(layer: &str, class: &str, subclass: Option<&str>) -> Option<Cover> {
        Some(match (layer, class) {
            ("water", _) => Cover::Water,
            ("park", _) => Cover::Park,
            ("landcover", "wood" | "forest") => Cover::Wood,
            ("landcover", "grass") => match subclass {
                Some("park" | "garden" | "village_green" | "recreation_ground") => Cover::Park,
                _ => Cover::Grass,
            },
            ("landcover", "farmland") => Cover::Farmland,
            ("landcover", "wetland") => Cover::Wetland,
            ("landcover", "sand") => Cover::Sand,
            ("landcover", "rock") => Cover::Rock,
            ("landcover", "ice") => Cover::Ice,
            ("landuse", "residential" | "suburb" | "neighbourhood" | "quarter") => Cover::Residential,
            ("landuse", "commercial" | "retail") => Cover::Commercial,
            ("landuse", "industrial" | "railway" | "garages") => Cover::Industrial,
            ("landuse", "pitch" | "stadium" | "playground") => Cover::Pitch,
            ("landuse", "cemetery") => Cover::Park,
            _ => return None,
        })
    }

    /// Rough priority so more specific features win over broad ones.
    fn priority(self) -> u8 {
        match self {
            Cover::Natural => 0,
            Cover::Residential | Cover::Commercial | Cover::Industrial => 1,
            Cover::Farmland | Cover::Grass | Cover::Wood | Cover::Wetland => 2,
            Cover::Sand | Cover::Rock | Cover::Ice => 3,
            Cover::Park | Cover::Pitch => 4,
            Cover::Water => 5,
        }
    }

    pub fn is_urban(self) -> bool {
        matches!(self, Cover::Residential | Cover::Commercial | Cover::Industrial)
    }
}

/// Ground-cover grid for one tile.
#[derive(Debug, Clone)]
pub struct SurfaceMap {
    pub tile: TileId,
    pub biome: Biome,
    pub lat: f64,
    pub res: usize,
    pub cover: Vec<Cover>,
}

impl SurfaceMap {
    pub fn uniform(tile: TileId, biome: Biome) -> Self {
        let c = tile.center();
        Self { tile, biome, lat: c.lat_deg, res: 1, cover: vec![Cover::Natural] }
    }

    /// Rasterize a vector tile's cover polygons. `vt_tile` may be an ancestor
    /// of `tile` (lower-zoom vector data for far terrain).
    pub fn from_vector(tile: TileId, vt_tile: TileId, vt: &mvt::Tile, biome: Biome, res: usize) -> Self {
        let c = tile.center();
        let mut cover = vec![Cover::Natural; res * res];
        let (u0, v0, s) = if vt_tile == tile { (0.0, 0.0, 1.0) } else {
            let (_, u0, v0, s) = tile.ancestor(vt_tile.z);
            (u0, v0, s)
        };
        for layer_name in ["landuse", "landcover", "park", "water"] {
            let Some(layer) = vt.layer(layer_name) else { continue };
            let ext = layer.extent as f64;
            for f in &layer.features {
                if f.geom_type != GeomType::Polygon {
                    continue;
                }
                let class = f.str("class").unwrap_or("");
                let Some(cv) = Cover::from_omt(layer_name, class, f.str("subclass")) else { continue };
                for (outer, holes) in mvt::polygons(f) {
                    // Convert the polygon into this tile's [0,1] space.
                    let to_local = |r: &Vec<[f32; 2]>| -> Vec<[f64; 2]> {
                        r.iter()
                            .map(|p| [((p[0] as f64 / ext) - u0) / s, ((p[1] as f64 / ext) - v0) / s])
                            .collect()
                    };
                    let outer = to_local(&outer);
                    let holes: Vec<_> = holes.iter().map(to_local).collect();
                    fill_polygon(&mut cover, res, &outer, &holes, cv);
                }
            }
        }
        Self { tile, biome, lat: c.lat_deg, res, cover }
    }

    pub fn cover_at(&self, u: f64, v: f64) -> Cover {
        let x = ((u * self.res as f64) as usize).min(self.res - 1);
        let y = ((v * self.res as f64) as usize).min(self.res - 1);
        self.cover[y * self.res + x]
    }

    /// Stylized ground color (linear RGBA). `slope` is 0 flat .. 1 vertical.
    pub fn ground_color(&self, u: f64, v: f64, height: f32, slope: f32) -> [f32; 4] {
        let cover = self.cover_at(u, v);
        let base = match cover {
            Cover::Natural => biome_ground(self.biome),
            Cover::Wood => match self.biome {
                Biome::Boreal => rgb(0x4f7a4a),
                Biome::TropicalRainforest => rgb(0x3f8a3a),
                _ => rgb(0x5c8f45),
            },
            Cover::Grass => rgb(0x9cc46a),
            Cover::Park | Cover::Pitch => rgb(0x8dc66b),
            Cover::Farmland => rgb(0xc1c77a),
            Cover::Wetland => rgb(0x7e9e72),
            Cover::Sand => rgb(0xe8d6a8),
            Cover::Rock => rgb(0xa79d8e),
            Cover::Ice => rgb(0xeef4fa),
            Cover::Residential => rgb(0xb9b6ad),
            Cover::Commercial => rgb(0xbdb7b4),
            Cover::Industrial => rgb(0xaeaaa4),
            Cover::Water => rgb(0x6f8f96),
        };
        // Steep ground shows rock.
        let rock = rgb(0x8f877c);
        let mut c = lerp_color(base, rock, ((slope - 0.35) / 0.25).clamp(0.0, 1.0));
        // Beaches near sea level.
        if (0.0..2.5).contains(&height) && !cover.is_urban() && self.biome != Biome::IceCap {
            c = lerp_color(c, rgb(0xe9dcb5), 0.7);
        }
        // Snow above the snowline (and a bit below on gentle slopes).
        let snowline = Biome::snowline_m(self.lat);
        let snow_t = ((height - snowline + 300.0 * (1.0 - slope)) / 250.0).clamp(0.0, 1.0)
            * (1.0 - ((slope - 0.6) / 0.2).clamp(0.0, 1.0));
        c = lerp_color(c, rgb(0xf4f8fc), snow_t);
        // Subtle deterministic variation so big areas aren't flat.
        let n = crate::hash01((u * 4096.0) as u64 / 24, (v * 4096.0) as u64 / 24);
        scale_color(c, 0.94 + 0.12 * n)
    }

    /// Trees per hectare for the foliage scatterer.
    pub fn tree_density(&self, u: f64, v: f64) -> f32 {
        match self.cover_at(u, v) {
            Cover::Wood => match self.biome {
                Biome::TropicalRainforest => 300.0,
                Biome::Boreal | Biome::TemperateForest => 220.0,
                _ => 120.0,
            },
            Cover::Park => 35.0,
            Cover::Residential => 12.0,
            Cover::Grass | Cover::Wetland => 6.0,
            Cover::Farmland => 1.5,
            Cover::Natural => match self.biome {
                Biome::TropicalRainforest => 200.0,
                Biome::TemperateForest => 40.0,
                Biome::Boreal => 60.0,
                Biome::Mediterranean => 25.0,
                Biome::Savanna => 8.0,
                Biome::Grassland => 2.0,
                Biome::HotDesert => 0.3,
                Biome::Tundra | Biome::Alpine | Biome::IceCap => 0.0,
            },
            _ => 0.0,
        }
    }
}

pub fn biome_ground(b: Biome) -> [f32; 4] {
    match b {
        Biome::TropicalRainforest => rgb(0x5f9a48),
        Biome::Savanna => rgb(0xc9b36a),
        Biome::HotDesert => rgb(0xe3c58f),
        Biome::Mediterranean => rgb(0xb3b26b),
        Biome::TemperateForest => rgb(0x8fb862),
        Biome::Grassland => rgb(0xb5be6e),
        Biome::Boreal => rgb(0x7f9a66),
        Biome::Tundra => rgb(0xa5a383),
        Biome::Alpine => rgb(0x9aa77a),
        Biome::IceCap => rgb(0xf0f5fa),
    }
}

/// Scanline polygon fill (even-odd with holes) into a res x res class grid.
fn fill_polygon(grid: &mut [Cover], res: usize, outer: &[[f64; 2]], holes: &[Vec<[f64; 2]>], cv: Cover) {
    let (mut minv, mut maxv) = (f64::MAX, f64::MIN);
    for p in outer {
        minv = minv.min(p[1]);
        maxv = maxv.max(p[1]);
    }
    if maxv < 0.0 || minv > 1.0 {
        return;
    }
    let y0 = ((minv * res as f64).floor().max(0.0)) as usize;
    let y1 = ((maxv * res as f64).ceil().min(res as f64)) as usize;
    let mut xs = Vec::new();
    for y in y0..y1 {
        let sv = (y as f64 + 0.5) / res as f64;
        xs.clear();
        for ring in std::iter::once(outer).chain(holes.iter().map(Vec::as_slice)) {
            for i in 0..ring.len() {
                let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                if (a[1] <= sv) != (b[1] <= sv) {
                    xs.push(a[0] + (sv - a[1]) / (b[1] - a[1]) * (b[0] - a[0]));
                }
            }
        }
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        for pair in xs.chunks_exact(2) {
            let xa = ((pair[0] * res as f64 - 0.5).ceil().max(0.0)) as usize;
            let xb = ((pair[1] * res as f64 - 0.5).floor().min(res as f64 - 1.0)) as isize;
            for x in xa as isize..=xb {
                let cell = &mut grid[y * res + x as usize];
                if cv.priority() >= cell.priority() {
                    *cell = cv;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_square_with_hole() {
        let mut g = vec![Cover::Natural; 100];
        let outer = vec![[0.1, 0.1], [0.9, 0.1], [0.9, 0.9], [0.1, 0.9]];
        let hole = vec![[0.4, 0.4], [0.6, 0.4], [0.6, 0.6], [0.4, 0.6]];
        fill_polygon(&mut g, 10, &outer, &[hole], Cover::Wood);
        assert_eq!(g[2 * 10 + 2], Cover::Wood);
        assert_eq!(g[5 * 10 + 5], Cover::Natural, "hole stays empty");
        assert_eq!(g[0], Cover::Natural);
    }
}
