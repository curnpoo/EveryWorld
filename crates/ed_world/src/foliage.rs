//! Trees and rocks. Density comes from land cover, species from biome, and
//! an occupancy mask keeps them off roads and out of buildings.

use crate::biome::Biome;
use crate::mesh::{rgb, scale_color};
use crate::project::TileProjector;
use crate::surface::{Cover, SurfaceMap};
use crate::terrain::TerrainGrid;
use ed_data::mvt::{self, GeomType};
use glam::Vec3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TreeKind {
    Broadleaf,
    Conifer,
    Birch,
    Palm,
    Acacia,
    Cypress,
    Shrub,
    Cactus,
}

#[derive(Debug, Clone, Copy)]
pub struct TreeInstance {
    pub kind: TreeKind,
    pub pos: Vec3,
    pub scale: f32,
    pub yaw: f32,
    pub tint: [f32; 4],
}

#[derive(Debug, Clone, Copy)]
pub struct RockInstance {
    pub pos: Vec3,
    pub scale: Vec3,
    pub yaw: f32,
    pub tint: [f32; 4],
}

/// Cells blocked by buildings/roads, in tile [0,1] space.
pub struct Occupancy {
    res: usize,
    bits: Vec<bool>,
}

impl Occupancy {
    pub fn from_vector(vt: &mvt::Tile, res: usize) -> Self {
        let mut o = Self { res, bits: vec![false; res * res] };
        if let Some(l) = vt.layer("building") {
            let ext = l.extent as f32;
            for f in &l.features {
                for ring in &f.rings {
                    // Conservative: block the footprint's bounding box (+1 cell).
                    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
                    for p in ring {
                        x0 = x0.min(p[0]);
                        y0 = y0.min(p[1]);
                        x1 = x1.max(p[0]);
                        y1 = y1.max(p[1]);
                    }
                    o.fill_rect(x0 / ext, y0 / ext, x1 / ext, y1 / ext, 1);
                }
            }
        }
        if let Some(l) = vt.layer("transportation") {
            let ext = l.extent as f32;
            for f in l.features.iter().filter(|f| f.geom_type == GeomType::LineString) {
                for line in &f.rings {
                    for w in line.windows(2) {
                        let steps = ((w[1][0] - w[0][0]).abs().max((w[1][1] - w[0][1]).abs()) / ext * res as f32).ceil() as usize + 1;
                        for k in 0..=steps {
                            let t = k as f32 / steps as f32;
                            let x = (w[0][0] + (w[1][0] - w[0][0]) * t) / ext;
                            let y = (w[0][1] + (w[1][1] - w[0][1]) * t) / ext;
                            o.fill_rect(x, y, x, y, 1);
                        }
                    }
                }
            }
        }
        o
    }

    fn fill_rect(&mut self, u0: f32, v0: f32, u1: f32, v1: f32, pad: i64) {
        let r = self.res as i64;
        let c = |t: f32| (t * r as f32).floor() as i64;
        for y in (c(v0) - pad).max(0)..=(c(v1) + pad).min(r - 1) {
            for x in (c(u0) - pad).max(0)..=(c(u1) + pad).min(r - 1) {
                self.bits[(y * r + x) as usize] = true;
            }
        }
    }

    pub fn blocked(&self, u: f64, v: f64) -> bool {
        let x = ((u * self.res as f64) as usize).min(self.res - 1);
        let y = ((v * self.res as f64) as usize).min(self.res - 1);
        self.bits[y * self.res + x]
    }
}

fn species(biome: Biome, cover: Cover, r: f32, lat: f64) -> TreeKind {
    use TreeKind::*;
    match biome {
        Biome::TropicalRainforest => if r < 0.25 { Palm } else { Broadleaf },
        Biome::Savanna => if r < 0.7 { Acacia } else { Shrub },
        Biome::HotDesert => if r < 0.5 { Shrub } else if lat.abs() < 35.0 && r < 0.75 { Palm } else { Cactus },
        Biome::Mediterranean => if r < 0.3 { Cypress } else if r < 0.45 && cover != Cover::Wood { Palm } else { Broadleaf },
        Biome::TemperateForest | Biome::Grassland => if r < 0.2 { Conifer } else if r < 0.3 { Birch } else { Broadleaf },
        Biome::Boreal => if r < 0.75 { Conifer } else { Birch },
        Biome::Alpine => if r < 0.85 { Conifer } else { Shrub },
        Biome::Tundra | Biome::IceCap => Shrub,
    }
}

pub fn foliage_color(kind: TreeKind, biome: Biome, r: f32) -> [f32; 4] {
    let base = match kind {
        TreeKind::Broadleaf => [0x5f9e3f, 0x6aa84f, 0x4f8f3a, 0x78b04a][(r * 4.0) as usize % 4],
        TreeKind::Conifer => [0x2f6b45, 0x3b7a4c, 0x2d5f3f][(r * 3.0) as usize % 3],
        TreeKind::Birch => 0x8fbf5a,
        TreeKind::Palm => 0x6aa84f,
        TreeKind::Acacia => 0x7f9b48,
        TreeKind::Cypress => 0x3d5f36,
        TreeKind::Shrub => if biome == Biome::HotDesert { 0x9a9a62 } else { 0x6f8f4a },
        TreeKind::Cactus => 0x5f8a54,
    };
    scale_color(rgb(base), 0.9 + 0.2 * r)
}

/// Scatter trees and rocks over a tile. Roughly one candidate per 8x8 m cell.
pub fn scatter(
    proj: &TileProjector,
    grid: &TerrainGrid,
    surface: &SurfaceMap,
    occ: Option<&Occupancy>,
    max_trees: usize,
) -> (Vec<TreeInstance>, Vec<RockInstance>) {
    let width = proj.tile.width_m() as f32;
    let cells = ((width / 8.0) as usize).clamp(16, 512);
    let cell_area_ha = (width / cells as f32).powi(2) / 10_000.0;
    let tkey = (proj.tile.x as u64) << 32 | proj.tile.y as u64 | (proj.tile.z as u64) << 58;
    let snowline = Biome::snowline_m(surface.lat);
    let mut trees = Vec::new();
    let mut rocks = Vec::new();
    for j in 0..cells {
        for i in 0..cells {
            let key = (j * cells + i) as u64;
            let ju = crate::hash01(tkey, key * 3);
            let jv = crate::hash01(tkey, key * 3 + 1);
            let roll = crate::hash01(tkey, key * 3 + 2);
            let u = (i as f64 + ju as f64) / cells as f64;
            let v = (j as f64 + jv as f64) / cells as f64;
            if occ.is_some_and(|o| o.blocked(u, v)) {
                continue;
            }
            let h = grid.sample(u, v);
            if h < 0.5 {
                continue; // in the sea
            }
            let cover = surface.cover_at(u, v);
            if cover == Cover::Water {
                continue;
            }
            let p_tree = (surface.tree_density(u, v) * cell_area_ha).min(0.95)
                * (1.0 - ((h - (snowline - 800.0)) / 600.0).clamp(0.0, 1.0));
            if roll < p_tree && trees.len() < max_trees {
                let r = crate::hash01(tkey, key * 7 + 5);
                let kind = species(surface.biome, cover, r, surface.lat);
                let scale = match kind {
                    TreeKind::Shrub => 0.6 + r * 0.6,
                    TreeKind::Cactus => 0.7 + r * 0.5,
                    _ => 0.75 + r * 0.6,
                };
                trees.push(TreeInstance {
                    kind,
                    pos: proj.local(u, v, h as f64),
                    scale,
                    yaw: r * std::f32::consts::TAU * 7.0,
                    tint: foliage_color(kind, surface.biome, crate::hash01(tkey, key * 11)),
                });
                continue;
            }
            // Rocks: rocky ground, mountains above treeline, deserts.
            let p_rock = match (cover, surface.biome) {
                (Cover::Rock, _) => 0.25,
                (Cover::Natural, Biome::Alpine | Biome::Tundra) => 0.08,
                (Cover::Natural | Cover::Sand, Biome::HotDesert) => 0.03,
                (Cover::Natural | Cover::Grass, _) => 0.006,
                _ => 0.0,
            };
            if roll > 1.0 - p_rock {
                let r = crate::hash01(tkey, key * 13);
                let s = 0.4 + r * r * 2.5;
                rocks.push(RockInstance {
                    pos: proj.local(u, v, h as f64 - 0.2 * s as f64),
                    scale: Vec3::new(s, s * (0.5 + 0.4 * jv), s * (0.7 + 0.5 * ju)),
                    yaw: r * 40.0,
                    tint: scale_color(rgb(if surface.biome == Biome::HotDesert { 0xc49a6c } else { 0x9b958c }), 0.85 + 0.3 * ju),
                });
            }
        }
    }
    (trees, rocks)
}
