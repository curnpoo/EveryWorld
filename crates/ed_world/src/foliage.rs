//! Trees, shrubs and rocks. Density comes from land cover, species from
//! biome, and an occupancy mask keeps them off roads and out of buildings.
//! Geometry lives in [`crate::trees`].

use crate::biome::Biome;
use crate::mesh::{rgb, scale_color};
use crate::project::TileProjector;
use crate::surface::{Cover, SurfaceMap};
use crate::terrain::TerrainGrid;
use ed_data::mvt::{self, GeomType};
use glam::Vec3;

/// Plant species. The first eight names are kept from v1 (Broadleaf is an
/// oak-like tree, Conifer a spruce, Palm a coconut palm, Cactus a saguaro).
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
    Maple,
    PlaneTree,
    Fir,
    Pine,
    DatePalm,
    Olive,
    Baobab,
    Banana,
    PricklyPear,
    Boxwood,
    Juniper,
    Oleander,
    Fern,
    GrassTussock,
    FloweringBush,
}

impl TreeKind {
    pub const ALL: &'static [TreeKind] = &[
        TreeKind::Broadleaf,
        TreeKind::Conifer,
        TreeKind::Birch,
        TreeKind::Palm,
        TreeKind::Acacia,
        TreeKind::Cypress,
        TreeKind::Shrub,
        TreeKind::Cactus,
        TreeKind::Maple,
        TreeKind::PlaneTree,
        TreeKind::Fir,
        TreeKind::Pine,
        TreeKind::DatePalm,
        TreeKind::Olive,
        TreeKind::Baobab,
        TreeKind::Banana,
        TreeKind::PricklyPear,
        TreeKind::Boxwood,
        TreeKind::Juniper,
        TreeKind::Oleander,
        TreeKind::Fern,
        TreeKind::GrassTussock,
        TreeKind::FloweringBush,
    ];

    /// Trees (vs shrubs and ground plants).
    pub const TREES: &'static [TreeKind] = &[
        TreeKind::Broadleaf,
        TreeKind::Maple,
        TreeKind::PlaneTree,
        TreeKind::Birch,
        TreeKind::Conifer,
        TreeKind::Fir,
        TreeKind::Pine,
        TreeKind::Palm,
        TreeKind::DatePalm,
        TreeKind::Cypress,
        TreeKind::Olive,
        TreeKind::Acacia,
        TreeKind::Baobab,
        TreeKind::Banana,
        TreeKind::Cactus,
        TreeKind::PricklyPear,
    ];

    pub const SHRUBS: &'static [TreeKind] = &[
        TreeKind::Shrub,
        TreeKind::Boxwood,
        TreeKind::Juniper,
        TreeKind::Oleander,
        TreeKind::FloweringBush,
        TreeKind::Fern,
        TreeKind::GrassTussock,
    ];

    pub fn is_shrub(self) -> bool {
        Self::SHRUBS.contains(&self)
    }
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

/// Pick from a weighted mix with a roll in [0,1).
fn pick(mix: &[(TreeKind, f32)], r: f32) -> TreeKind {
    let total: f32 = mix.iter().map(|m| m.1).sum();
    let mut acc = 0.0;
    for (k, w) in mix {
        acc += w / total;
        if r < acc {
            return *k;
        }
    }
    mix.last().unwrap().0
}

/// Species mix by biome and land cover. Woods get trees with an understory
/// of shrubs/ferns; parks and towns get street trees and garden shrubs.
pub fn species(biome: Biome, cover: Cover, r: f32, lat: f64) -> TreeKind {
    use TreeKind::*;
    let urban = matches!(cover, Cover::Residential | Cover::Commercial | Cover::Industrial | Cover::Park);
    match biome {
        Biome::TropicalRainforest => {
            if urban {
                pick(&[(Palm, 4.0), (Banana, 2.0), (FloweringBush, 2.0), (Broadleaf, 2.0), (Fern, 1.0)], r)
            } else {
                pick(&[(Broadleaf, 5.0), (Palm, 2.0), (Banana, 2.0), (Fern, 2.5), (Shrub, 1.0)], r)
            }
        }
        Biome::Savanna => pick(&[(Acacia, 6.0), (Baobab, if lat.abs() < 25.0 { 0.6 } else { 0.0 }), (GrassTussock, 3.0), (Shrub, 1.5)], r),
        Biome::HotDesert => {
            let palms = if lat.abs() < 35.0 { 2.0 } else { 0.5 };
            if urban {
                pick(&[(DatePalm, 5.0), (Oleander, 2.0), (Olive, 1.0), (PricklyPear, 1.0)], r)
            } else {
                pick(&[(Shrub, 3.0), (GrassTussock, 2.0), (DatePalm, palms), (Cactus, 1.5), (PricklyPear, 1.5)], r)
            }
        }
        Biome::Mediterranean => {
            if cover == Cover::Wood {
                pick(&[(Pine, 4.0), (Olive, 2.0), (Cypress, 1.0), (Juniper, 1.5), (Oleander, 0.5)], r)
            } else if urban {
                pick(&[(PlaneTree, 3.0), (Cypress, 2.0), (Palm, 1.5), (DatePalm, 1.0), (Olive, 1.5), (Oleander, 2.0), (Boxwood, 1.0)], r)
            } else {
                pick(&[(Olive, 4.0), (Cypress, 2.0), (Pine, 1.5), (Juniper, 1.5), (Oleander, 1.0), (GrassTussock, 1.0)], r)
            }
        }
        Biome::TemperateForest | Biome::Grassland => {
            if urban {
                pick(&[(PlaneTree, 3.0), (Maple, 2.0), (Broadleaf, 2.0), (Birch, 1.0), (Boxwood, 1.5), (FloweringBush, 1.5), (Shrub, 1.0)], r)
            } else if cover == Cover::Wood {
                pick(&[(Broadleaf, 4.0), (Maple, 2.5), (Birch, 1.5), (Conifer, 1.5), (Pine, 1.0), (Fern, 1.5), (Shrub, 1.0)], r)
            } else {
                pick(&[(Broadleaf, 3.0), (Maple, 1.5), (Birch, 1.0), (Conifer, 1.0), (Shrub, 2.0), (GrassTussock, 1.5)], r)
            }
        }
        Biome::Boreal => pick(&[(Conifer, 5.0), (Fir, 2.5), (Pine, 2.0), (Birch, 2.0), (Juniper, 1.0), (Fern, 0.7)], r),
        Biome::Alpine => pick(&[(Conifer, 3.0), (Fir, 3.0), (Pine, 1.5), (Juniper, 2.0), (GrassTussock, 1.5)], r),
        Biome::Tundra | Biome::IceCap => pick(&[(Juniper, 2.0), (GrassTussock, 3.0), (Shrub, 1.0)], r),
    }
}

/// Per-instance tint hint (kept for callers; v2 models carry their own
/// per-variant colors, so this is near white with a little variation).
pub fn foliage_color(kind: TreeKind, biome: Biome, r: f32) -> [f32; 4] {
    let dry = matches!(biome, Biome::HotDesert | Biome::Savanna) && !matches!(kind, TreeKind::Cactus | TreeKind::PricklyPear);
    let base = if dry { [1.06, 1.0, 0.86, 1.0] } else { [1.0; 4] };
    scale_color(base, 0.94 + 0.12 * r)
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
    // Expected tree count, so a busy tile is thinned evenly instead of
    // filling up row by row and leaving its southern part bare.
    let mut expected = 0.0f32;
    for j in (0..cells).step_by(4) {
        for i in (0..cells).step_by(4) {
            let (u, v) = ((i as f64 + 0.5) / cells as f64, (j as f64 + 0.5) / cells as f64);
            expected += (surface.tree_density(u, v) * cell_area_ha).min(0.95) * 16.0;
        }
    }
    let thin = (max_trees as f32 / expected.max(1.0)).min(1.0);
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
                * thin
                * (1.0 - ((h - (snowline - 800.0)) / 600.0).clamp(0.0, 1.0));
            if roll < p_tree && trees.len() < max_trees {
                let r = crate::hash01(tkey, key * 7 + 5);
                let kind = species(surface.biome, cover, r, surface.lat);
                let r2 = crate::hash01(tkey, key * 7 + 6);
                let scale = if kind.is_shrub() { 0.7 + r2 * 0.6 } else { 0.8 + r2 * 0.45 };
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
