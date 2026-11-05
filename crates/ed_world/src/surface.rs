//! Ground cover. Rasterizes OpenMapTiles `landcover`, `landuse`, `park` and
//! `water` polygons into a small class grid per tile, then turns
//! class + biome + slope + altitude into a stylized ground color and foliage
//! density.

use crate::biome::Biome;
use crate::mesh::rgb;
use crate::noise;
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
    /// WorldCover shrubland: scrub, maquis, sagebrush.
    Shrub,
    /// WorldCover bare / sparse vegetation: desert pavement, scree, dry flats.
    Bare,
    /// Moss and lichen (tundra, high latitude barrens).
    Moss,
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
            Cover::Farmland | Cover::Grass | Cover::Wood | Cover::Wetland | Cover::Shrub | Cover::Bare | Cover::Moss => 2,
            Cover::Sand | Cover::Rock | Cover::Ice => 3,
            Cover::Park | Cover::Pitch => 4,
            Cover::Water => 5,
        }
    }

    pub fn is_urban(self) -> bool {
        matches!(self, Cover::Residential | Cover::Commercial | Cover::Industrial)
    }
}

/// Number of ground material layers the terrain shader splats.
pub const LAYERS: usize = 12;

/// Ground materials. Order = texture-array layer = splat channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Ground {
    Grass = 0,
    DryGrass,
    Moss,
    ForestFloor,
    Dirt,
    Farmland,
    Sand,
    Gravel,
    Rock,
    Snow,
    Paving,
    Mud,
}

impl Ground {
    pub const ALL: [Ground; LAYERS] = [
        Ground::Grass, Ground::DryGrass, Ground::Moss, Ground::ForestFloor, Ground::Dirt, Ground::Farmland,
        Ground::Sand, Ground::Gravel, Ground::Rock, Ground::Snow, Ground::Paving, Ground::Mud,
    ];

    /// Average albedo of the layer's texture (linear).
    pub fn mean_color(self) -> [f32; 4] {
        rgb(match self {
            Ground::Grass => 0x6e9a3c,
            Ground::DryGrass => 0xb8a05c,
            Ground::Moss => 0x5b7a30,
            Ground::ForestFloor => 0x5c4d33,
            Ground::Dirt => 0x8a6c4c,
            Ground::Farmland => 0x7a5f44,
            Ground::Sand => 0xdcc493,
            Ground::Gravel => 0x9a958b,
            Ground::Rock => 0x8b857b,
            Ground::Snow => 0xeef3f8,
            Ground::Paving => 0xa39f97,
            Ground::Mud => 0x56473a,
        })
    }

    /// Living layers take the regional vegetation tint.
    pub fn is_living(self) -> bool {
        matches!(self, Ground::Grass | Ground::DryGrass | Ground::Moss | Ground::Farmland)
    }
}

/// What a land-cover class is made of, per biome. Weights need not sum to 1.
fn recipe(c: Cover, b: Biome) -> Vec<(Ground, f32)> {
    use Ground::*;
    let dry = matches!(b, Biome::Mediterranean | Biome::Savanna | Biome::HotDesert | Biome::Steppe);
    match c {
        Cover::Natural => match b {
            Biome::TropicalRainforest => vec![(ForestFloor, 0.45), (Grass, 0.3), (Moss, 0.2), (Mud, 0.05)],
            Biome::Savanna => vec![(DryGrass, 0.7), (Dirt, 0.3)],
            Biome::HotDesert => vec![(Sand, 0.7), (Gravel, 0.18), (Dirt, 0.12)],
            Biome::Mediterranean => vec![(DryGrass, 0.5), (Dirt, 0.3), (Gravel, 0.2)],
            Biome::TemperateForest => vec![(Grass, 0.75), (Moss, 0.12), (Dirt, 0.13)],
            Biome::Grassland => vec![(Grass, 0.5), (DryGrass, 0.45), (Dirt, 0.05)],
            Biome::Steppe => vec![(DryGrass, 0.55), (Dirt, 0.25), (Gravel, 0.2)],
            Biome::Boreal => vec![(Moss, 0.4), (ForestFloor, 0.3), (Grass, 0.3)],
            Biome::Tundra => vec![(Moss, 0.4), (DryGrass, 0.3), (Gravel, 0.3)],
            Biome::Alpine => vec![(Grass, 0.5), (Gravel, 0.25), (Rock, 0.25)],
            Biome::IceCap => vec![(Snow, 1.0)],
        },
        Cover::Wood => match b {
            Biome::TropicalRainforest => vec![(ForestFloor, 0.6), (Moss, 0.2), (Mud, 0.2)],
            Biome::Mediterranean | Biome::Savanna | Biome::HotDesert | Biome::Steppe => vec![(ForestFloor, 0.5), (DryGrass, 0.3), (Dirt, 0.2)],
            Biome::Boreal | Biome::Alpine | Biome::Tundra => vec![(ForestFloor, 0.45), (Moss, 0.4), (Grass, 0.15)],
            _ => vec![(ForestFloor, 0.5), (Moss, 0.25), (Grass, 0.25)],
        },
        Cover::Grass => if dry { vec![(DryGrass, 0.8), (Dirt, 0.2)] } else { vec![(Grass, 0.85), (DryGrass, 0.1), (Dirt, 0.05)] },
        // Parks and pitches are watered, even in dry places.
        Cover::Park => vec![(Grass, 0.85), (Dirt, 0.05), (Gravel, 0.1)],
        Cover::Pitch => vec![(Grass, 1.0)],
        Cover::Farmland => vec![(Farmland, 0.85), (Dirt, 0.15)],
        Cover::Wetland => vec![(Mud, 0.4), (Moss, 0.3), (Grass, 0.3)],
        Cover::Sand => vec![(Sand, 1.0)],
        Cover::Rock => if b == Biome::HotDesert { vec![(Rock, 0.4), (Gravel, 0.3), (Sand, 0.3)] } else { vec![(Rock, 0.7), (Gravel, 0.3)] },
        Cover::Ice => vec![(Snow, 1.0)],
        Cover::Residential => if dry {
            vec![(DryGrass, 0.3), (Dirt, 0.2), (Paving, 0.3), (Gravel, 0.2)]
        } else {
            vec![(Grass, 0.45), (Dirt, 0.12), (Paving, 0.28), (Gravel, 0.15)]
        },
        Cover::Commercial => vec![(Paving, 0.7), (Gravel, 0.2), (Grass, 0.1)],
        Cover::Industrial => vec![(Gravel, 0.5), (Paving, 0.3), (Dirt, 0.2)],
        Cover::Water => vec![(Mud, 0.6), (Sand, 0.4)],
        Cover::Shrub => match b {
            Biome::HotDesert => vec![(Sand, 0.45), (Gravel, 0.3), (Dirt, 0.15), (DryGrass, 0.1)],
            _ if dry => vec![(DryGrass, 0.4), (Dirt, 0.35), (Gravel, 0.25)],
            Biome::Tundra | Biome::Alpine => vec![(Moss, 0.3), (Gravel, 0.35), (DryGrass, 0.2), (Rock, 0.15)],
            _ => vec![(Grass, 0.4), (ForestFloor, 0.25), (Dirt, 0.25), (Moss, 0.1)],
        },
        Cover::Bare => match b {
            Biome::HotDesert => vec![(Sand, 0.45), (Gravel, 0.35), (Rock, 0.2)],
            Biome::Alpine | Biome::Tundra | Biome::IceCap => vec![(Gravel, 0.45), (Rock, 0.4), (Dirt, 0.15)],
            _ => vec![(Dirt, 0.45), (Gravel, 0.3), (Rock, 0.25)],
        },
        Cover::Moss => vec![(Moss, 0.6), (DryGrass, 0.15), (Gravel, 0.25)],
    }
}

/// What dry ground shows where vegetation fails (satellite says it is not green).
fn bare_ground(b: Biome) -> &'static [(Ground, f32)] {
    use Ground::*;
    match b {
        Biome::HotDesert => &[(Sand, 0.5), (Gravel, 0.3), (Dirt, 0.2)],
        Biome::Alpine | Biome::Tundra | Biome::IceCap => &[(Gravel, 0.5), (Rock, 0.3), (Dirt, 0.2)],
        _ => &[(Dirt, 0.55), (Gravel, 0.3), (Sand, 0.15)],
    }
}

fn normalize(w: &mut [f32; LAYERS]) {
    let s: f32 = w.iter().sum();
    if s > 1e-6 {
        w.iter_mut().for_each(|x| *x /= s);
    } else {
        *w = [0.0; LAYERS];
        w[Ground::Dirt as usize] = 1.0;
    }
}

fn sharpen(w: &mut [f32; LAYERS], p: f32) {
    w.iter_mut().for_each(|x| *x = x.max(0.0).powf(p));
    normalize(w);
}

/// Blend `amount` of layer `g` over the current mix.
fn mix_into(w: &mut [f32; LAYERS], g: Ground, amount: f32) {
    let a = amount.clamp(0.0, 1.0);
    if a <= 0.0 {
        return;
    }
    w.iter_mut().for_each(|x| *x *= 1.0 - a);
    w[g as usize] += a;
}

/// Pack weights into 12 bytes (sum preserved to within rounding).
pub fn pack_splat(w: &[f32; LAYERS]) -> [u8; LAYERS] {
    let mut out = [0u8; LAYERS];
    for (o, x) in out.iter_mut().zip(w) {
        *o = (x.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    }
    out
}

/// Ground-cover grid for one tile.
#[derive(Debug, Clone)]
pub struct SurfaceMap {
    pub tile: TileId,
    pub biome: Biome,
    pub lat: f64,
    pub res: usize,
    pub cover: Vec<Cover>,
    /// WorldCover canopy: share of tree samples per cell (0..=255), `wc_res`^2, empty if not loaded.
    pub canopy: Vec<u8>,
    /// WorldCover shrub share per cell (0..=255), same grid as `canopy`.
    pub shrubs: Vec<u8>,
    pub wc_res: usize,
    /// Bedrock under the tile (Macrostrat), colours exposed rock and sand.
    pub lith: Option<ed_data::geology::Lithology>,
    /// Satellite colours (Sentinel-2) covering this tile, if fetched.
    pub sat: Option<SatView>,
}

/// A satellite tile plus the window of it that this surface tile occupies
/// (the sat tile is an ancestor of the surface tile).
#[derive(Debug, Clone)]
pub struct SatView {
    pub tile: std::sync::Arc<ed_data::satellite::SatTile>,
    pub u0: f64,
    pub v0: f64,
    pub s: f64,
    /// Optional finer mosaic: the four children (index cx + 2*cy) of `tile`.
    pub kids: Option<[std::sync::Arc<ed_data::satellite::SatTile>; 4]>,
}

impl SurfaceMap {
    pub fn uniform(tile: TileId, biome: Biome) -> Self {
        let c = tile.center();
        Self { tile, biome, lat: c.lat_deg, res: 1, cover: vec![Cover::Natural], canopy: Vec::new(), shrubs: Vec::new(), wc_res: 0, lith: None, sat: None }
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
        Self { tile, biome, lat: c.lat_deg, res, cover, canopy: Vec::new(), shrubs: Vec::new(), wc_res: 0, lith: None, sat: None }
    }

    /// Fill unmapped (`Natural`) cells from ESA WorldCover and keep its canopy
    /// and shrub fractions for tree density. OSM polygons still win where mapped.
    /// The cover grid is upsampled to the WorldCover grid if it is coarser.
    pub fn with_worldcover(mut self, lc: &ed_data::worldcover::LandCover) -> Self {
        use ed_data::worldcover as wc;
        if lc.w == 0 || lc.w != lc.h {
            return self;
        }
        if self.res < lc.w {
            let (old, r) = (std::mem::take(&mut self.cover), self.res);
            self.cover = (0..lc.w * lc.w).map(|i| old[(i / lc.w * r / lc.w) * r + (i % lc.w * r / lc.w)]).collect();
            self.res = lc.w;
        }
        for y in 0..self.res {
            for x in 0..self.res {
                let cell = &mut self.cover[y * self.res + x];
                if *cell != Cover::Natural {
                    continue;
                }
                let k = (y * lc.w / self.res) * lc.w + x * lc.w / self.res;
                *cell = match lc.class[k] {
                    wc::TREE | wc::MANGROVE => Cover::Wood,
                    wc::SHRUB => Cover::Shrub,
                    wc::GRASS => Cover::Grass,
                    wc::CROP => Cover::Farmland,
                    wc::BUILT => Cover::Residential,
                    wc::BARE => Cover::Bare,
                    wc::SNOW => Cover::Ice,
                    wc::WETLAND => Cover::Wetland,
                    wc::MOSS => Cover::Moss,
                    _ => Cover::Natural, // water is left to OSM (it carves terrain)
                };
            }
        }
        self.canopy = lc.tree.clone();
        self.shrubs = lc.shrub.clone();
        self.wc_res = lc.w;
        self
    }

    /// Attach satellite imagery. `sat.id` must be an ancestor of (or equal to) this tile.
    pub fn with_satellite(mut self, sat: ed_data::satellite::SatTile) -> Self {
        let (_, u0, v0, s) = self.tile.ancestor(sat.id.z);
        self.sat = Some(SatView { tile: std::sync::Arc::new(sat), u0, v0, s, kids: None });
        self
    }

    /// Upgrade to a 2x finer mosaic of the surface tile's children (tile must equal the sat tile).
    pub fn with_satellite_kids(mut self, kids: [ed_data::satellite::SatTile; 4]) -> Self {
        if let Some(s) = self.sat.as_mut() {
            s.kids = Some(kids.map(std::sync::Arc::new));
        }
        self
    }

    /// RGBA8 satellite texture over the tile (`size`^2): gamma RGB + land/water flag
    /// in alpha (see `sat_vertex`). `height(u,v)` gives ground height for the water test.
    pub fn sat_texture(&self, size: usize, height: impl Fn(f64, f64) -> f32) -> Option<Vec<u8>> {
        self.sat.as_ref()?;
        let mut out = Vec::with_capacity(size * size * 4);
        for j in 0..size {
            for i in 0..size {
                let (u, v) = (i as f64 / (size - 1) as f64, j as f64 / (size - 1) as f64);
                out.extend_from_slice(&self.sat_vertex(u, v, height(u, v)));
            }
        }
        Some(out)
    }

    /// Real-world colour (linear RGB) at tile (u, v), if satellite data is loaded.
    pub fn sat_linear(&self, u: f64, v: f64) -> Option<[f32; 3]> {
        let s = self.sat.as_ref()?;
        if let Some(k) = &s.kids {
            let (x, y) = (u.clamp(0.0, 1.0 - 1e-9) * 2.0, v.clamp(0.0, 1.0 - 1e-9) * 2.0);
            let (cx, cy) = (x as usize, y as usize);
            return Some(k[cx + 2 * cy].sample(x - cx as f64, y - cy as f64));
        }
        Some(s.tile.sample(s.u0 + u * s.s, s.v0 + v * s.s))
    }

    /// Is this a water cell (mapped water, or open sea in tiles with no vector data)?
    pub fn is_water(&self, u: f64, v: f64, height: f32) -> bool {
        self.cover_at(u, v) == Cover::Water || (self.res == 1 && height < 0.0)
    }

    /// Per-vertex far-field albedo: gamma-2.2 RGB + blend strength. Water
    /// gets a satellite-informed water colour (depth-darkened) at full strength.
    pub fn sat_vertex(&self, u: f64, v: f64, height: f32) -> [u8; 4] {
        let enc = |c: [f32; 3]| [0, 1, 2].map(|k| (c[k].clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0 + 0.5) as u8);
        let sat = self.sat_linear(u, v);
        if self.is_water(u, v, height) {
            // Shallow = lighter/greener, deep = navy. Satellite sets the hue.
            let deep = noise::smoothstep(0.0, -60.0, height.min(0.0) as f32);
            let base = [0.030 - 0.016 * deep, 0.115 - 0.055 * deep, 0.200 - 0.095 * deep];
            let c = match sat {
                Some(s) => {
                    // Satellite hue only on the shelf; open ocean uses one
                    // consistent navy so tile JPEG differences never show as seams.
                    let shelf = noise::smoothstep(-200.0, -20.0, height.min(0.0)) * 0.35;
                    // Lakes/rivers (above sea level) and shallows take the satellite hue;
                    // open deep ocean never does, so z>=7 and z<7 tiles match.
                    let inland = if self.res == 1 { 0.0 } else { 0.65 * noise::smoothstep(-60.0, -5.0, height) };
                    let w = shelf.max(inland);
                    [0, 1, 2].map(|k| base[k] * (1.0 - w) + s[k] * w * 0.9)
                }
                None => base,
            };
            let e = enc(c);
            // Alpha 128 flags water for the shader (255 = land, 0 = no data).
            return [e[0], e[1], e[2], 128];
        }
        match sat {
            Some(c) => {
                let e = enc(c);
                [e[0], e[1], e[2], 255]
            }
            None => [0, 0, 0, 0],
        }
    }

    pub fn cover_at(&self, u: f64, v: f64) -> Cover {
        let x = ((u * self.res as f64) as usize).min(self.res - 1);
        let y = ((v * self.res as f64) as usize).min(self.res - 1);
        self.cover[y * self.res + x]
    }

    /// Global position in z14-tile units (continuous across tiles and zooms),
    /// so noise patterns line up at tile seams.
    pub fn global_xy(&self, u: f64, v: f64) -> (f64, f64) {
        let k = 2f64.powi(14 - self.tile.z as i32);
        ((self.tile.x as f64 + u) * k, (self.tile.y as f64 + v) * k)
    }

    /// Per-layer ground weights (sum to 1) at tile (u, v). This is the source
    /// of truth for *what* the ground is made of: the terrain shader splats
    /// textures with it and the near-camera scatter picks grass, pebbles,
    /// moss etc. from it. `slope` is 0 flat .. 1 vertical.
    pub fn splat(&self, u: f64, v: f64, height: f32, slope: f32) -> [f32; LAYERS] {
        let (gx, gy) = self.global_xy(u, v);
        // Soft, organic cover edges: sample the class grid at a few
        // domain-warped points around (u, v) and blend their recipes.
        let cell = 1.0 / self.res as f64;
        let wx = (noise::fbm(gx * 90.0, gy * 90.0, 11, 3, 0) as f64 - 0.5) * 2.4 * cell;
        let wy = (noise::fbm(gx * 90.0, gy * 90.0, 12, 3, 0) as f64 - 0.5) * 2.4 * cell;
        let (cu, cv) = (u + wx, v + wy);
        let mut w = [0.0f32; LAYERS];
        let taps: [(f64, f64, f32); 5] = [(0.0, 0.0, 2.0), (-0.6, 0.0, 1.0), (0.6, 0.0, 1.0), (0.0, -0.6, 1.0), (0.0, 0.6, 1.0)];
        let green = self.greenness(u, v);
        for (du, dv, k) in taps {
            let c = self.cover_at((cu + du * cell).clamp(0.0, 1.0), (cv + dv * cell).clamp(0.0, 1.0));
            // Where the satellite shows no green, unmanaged vegetated cover
            // gives way to bare ground (no lawn in a desert).
            let gated = matches!(c, Cover::Natural | Cover::Grass | Cover::Shrub | Cover::Moss);
            for (l, x) in recipe(c, self.biome) {
                if gated && l.is_living() {
                    let keep = 0.08 + 0.92 * green;
                    w[l as usize] += x * k * keep;
                    for (bg, bx) in bare_ground(self.biome) {
                        w[*bg as usize] += x * k * (1.0 - keep) * bx;
                    }
                } else {
                    w[l as usize] += x * k;
                }
            }
        }
        normalize(&mut w);
        // Break uniform mixes into patches (a meadow with bare dirt spots
        // rather than a flat 80/20 blend). These weights live on mesh
        // vertices (3-30 m apart), so the noise must stay coarser than that:
        // anything finer is random per vertex, and the shader's height blend
        // turns it into one blob per vertex, i.e. a grid visible from the air.
        // Sub-vertex breakup is the shader's job (per pixel, aperiodic).
        for (l, x) in w.iter_mut().enumerate() {
            if *x > 0.0 {
                let s = 200 + l as u64 * 17;
                let n = 0.6 * noise::fbm(gx * 30.0, gy * 30.0, s, 2, 0) + 0.4 * noise::fbm(gx * 11.0, gy * 11.0, s + 5, 2, 0);
                *x *= 0.25 + 1.5 * n;
            }
        }
        sharpen(&mut w, 1.6);
        let mut out = w;
        // Steep ground: scree first, then bare rock.
        let scree = noise::smoothstep(0.10, 0.22, slope);
        let cliff = noise::smoothstep(0.18, 0.36, slope);
        mix_into(&mut out, Ground::Gravel, scree * 0.5 * (1.0 - cliff));
        mix_into(&mut out, Ground::Rock, cliff);
        // Beaches near sea level.
        let beach = if self.biome != Biome::IceCap && !self.cover_at(u, v).is_urban() {
            1.0 - noise::smoothstep(1.2, 3.0, height)
        } else {
            0.0
        };
        if height > -1.0 {
            mix_into(&mut out, Ground::Sand, beach * 0.9);
        }
        // Snow above the snowline (and a bit below on gentle slopes), sliding
        // off cliffs; patchy at its lower edge.
        let snowline = Biome::snowline_m(self.lat);
        let edge = noise::fbm(gx * 120.0, gy * 120.0, 77, 3, 0) * 260.0;
        let snow_t = ((height - snowline + 300.0 * (1.0 - slope) + edge - 130.0) / 220.0).clamp(0.0, 1.0)
            * (1.0 - noise::smoothstep(0.28, 0.45, slope));
        let snow_t = if self.biome == Biome::IceCap { snow_t.max(0.9) } else { snow_t };
        mix_into(&mut out, Ground::Snow, snow_t);
        normalize(&mut out);
        out
    }

    /// Per-vertex vegetation tint (linear RGB multiplier around 1.0) plus
    /// wetness in alpha. Applied by the shader to living layers only, so a
    /// Mediterranean meadow reads golden and a rainforest reads deep green
    /// from the same textures.
    pub fn tint(&self, u: f64, v: f64) -> [f32; 4] {
        let base: [f32; 3] = match self.biome {
            Biome::TropicalRainforest => [0.80, 1.06, 0.78],
            Biome::Savanna => [1.14, 1.0, 0.78],
            Biome::HotDesert => [1.16, 1.0, 0.80],
            Biome::Mediterranean => [1.10, 1.0, 0.80],
            Biome::TemperateForest => [1.0, 1.0, 1.0],
            Biome::Grassland => [1.06, 1.02, 0.88],
            Biome::Steppe => [1.10, 1.0, 0.84],
            Biome::Boreal => [0.86, 0.96, 0.94],
            Biome::Tundra => [1.04, 0.96, 0.86],
            Biome::Alpine => [0.96, 1.02, 0.90],
            Biome::IceCap => [1.0, 1.0, 1.0],
        };
        let (gx, gy) = self.global_xy(u, v);
        let c = self.cover_at(u, v);
        let mut base = base;
        // Satellite truth: scale the living layers so they average to what
        // the real place looks like. Canopy hides the ground under forests,
        // so woods only get a partial correction.
        let mut noise_k = 1.0;
        if let Some(sat) = self.sat_linear(u, v) {
            let strength = match c {
                Cover::Wood => 0.45,
                Cover::Natural | Cover::Grass | Cover::Park | Cover::Pitch | Cover::Farmland | Cover::Wetland | Cover::Shrub | Cover::Moss => 0.9,
                _ => 0.0,
            };
            let mut exp = [0.0f32; 3];
            let mut wsum = 0.0;
            for (g, x) in recipe(c, self.biome) {
                if g.is_living() {
                    let m = g.mean_color();
                    for k in 0..3 {
                        exp[k] += m[k] * x;
                    }
                    wsum += x;
                }
            }
            if strength > 0.0 && wsum > 0.05 {
                let r = [0, 1, 2].map(|k| sat[k] / (exp[k] / wsum).max(1e-3));
                let l = ((r[0] + r[1] + r[2]) / 3.0).max(1e-3);
                let bright = l.clamp(0.55, 1.45);
                for k in 0..3 {
                    let t = (r[k] / l).clamp(0.6, 1.6) * bright;
                    base[k] = base[k] + (t - base[k]) * strength;
                }
                noise_k = 0.5;
            }
        }
        // Large, slow patches: some fields greener, some drier, some darker.
        let dry = (noise::fbm(gx * 14.0, gy * 14.0, 31, 3, 0) - 0.5) * noise_k;
        let lum = 1.0 + (0.92 + 0.16 * noise::fbm(gx * 40.0, gy * 40.0, 32, 2, 0) - 1.0) * noise_k;
        let wet = match c {
            Cover::Wetland => 0.6,
            Cover::Water => 1.0,
            _ => 0.0,
        };
        [
            base[0] * lum * (1.0 + 0.16 * dry),
            base[1] * lum * (1.0 + 0.03 * dry),
            base[2] * lum * (1.0 - 0.20 * dry),
            wet,
        ]
    }

    /// Flat ground color (linear RGBA): what the textured ground averages to.
    /// Kept for far-field uses (maps, impostors) and as a sanity check.
    pub fn ground_color(&self, u: f64, v: f64, height: f32, slope: f32) -> [f32; 4] {
        let w = self.splat(u, v, height, slope);
        let t = self.tint(u, v);
        let mut c = [0.0f32; 4];
        for (l, x) in w.iter().enumerate() {
            let g = Ground::ALL[l];
            let mut m = g.mean_color();
            if g.is_living() {
                for k in 0..3 {
                    m[k] *= t[k];
                }
            }
            for k in 0..3 {
                c[k] += m[k] * x;
            }
        }
        c[3] = 1.0;
        c
    }

    /// How green the real ground looks (0 barren .. 1 vegetated), from the
    /// satellite's green-over-red excess. 1.0 when there is no imagery.
    pub fn greenness(&self, u: f64, v: f64) -> f32 {
        match self.sat_linear(u, v) {
            Some([r, g, _]) => noise::smoothstep(0.0, 0.10, (g - r) / (g + r + 1e-3)),
            None => 1.0,
        }
    }

    /// Tree-canopy share (0..1) from WorldCover at tile (u, v), if loaded.
    pub fn canopy_at(&self, u: f64, v: f64) -> Option<f32> {
        (!self.canopy.is_empty()).then(|| self.wc_cell(&self.canopy, u, v))
    }

    /// Canopy density 0..1 for far shadows: WorldCover share when loaded,
    /// else the scatterer's tree density relative to a closed forest.
    pub fn canopy_share(&self, u: f64, v: f64) -> f32 {
        self.canopy_at(u, v).unwrap_or_else(|| (self.tree_density(u, v) / forest_max(self.biome)).min(1.0))
    }

    pub fn shrub_at(&self, u: f64, v: f64) -> Option<f32> {
        (!self.shrubs.is_empty()).then(|| self.wc_cell(&self.shrubs, u, v))
    }

    fn wc_cell(&self, grid: &[u8], u: f64, v: f64) -> f32 {
        let n = self.wc_res;
        let x = ((u * n as f64) as usize).min(n - 1);
        let y = ((v * n as f64) as usize).min(n - 1);
        grid[y * n + x] as f32 / 255.0
    }

    /// Plants (trees + shrubs) per hectare for the foliage scatterer. With
    /// WorldCover loaded this is canopy-driven: a 90 % canopy cell gets a near
    /// full forest, a 5 % cell a few scattered trees, desert none. Without it,
    /// per-cover/biome defaults gated by how green the satellite says it is.
    pub fn tree_density(&self, u: f64, v: f64) -> f32 {
        let c = self.cover_at(u, v);
        let canopy = self.canopy_at(u, v);
        if let Some(f) = canopy {
            let trees = forest_max(self.biome) * f.powf(1.15);
            let shrubs = self.shrub_at(u, v).unwrap_or(0.0) * shrub_max(self.biome);
            return match c {
                Cover::Park => 35.0f32.max(trees),
                Cover::Residential => 12.0f32.max(trees * 0.7),
                Cover::Commercial | Cover::Industrial => (trees * 0.3).min(8.0),
                Cover::Farmland => (trees + shrubs).min(10.0),
                Cover::Rock | Cover::Sand | Cover::Ice | Cover::Water | Cover::Pitch => 0.0,
                Cover::Bare => (trees + shrubs).min(forest_max(self.biome) * 0.05),
                _ => trees + shrubs,
            };
        }
        let green = self.greenness(u, v);
        let d = match c {
            Cover::Wood => match self.biome {
                Biome::TropicalRainforest => 300.0,
                Biome::Boreal | Biome::TemperateForest => 220.0,
                Biome::HotDesert => 15.0,
                Biome::Steppe => 40.0,
                _ => 120.0,
            },
            Cover::Park => 35.0,
            Cover::Residential => 12.0,
            Cover::Shrub => shrub_max(self.biome) * 0.4,
            Cover::Grass | Cover::Wetland => 6.0,
            Cover::Farmland => 1.5,
            Cover::Natural => match self.biome {
                Biome::TropicalRainforest => 200.0,
                Biome::TemperateForest => 40.0,
                Biome::Boreal => 60.0,
                Biome::Mediterranean => 25.0,
                Biome::Savanna => 8.0,
                Biome::Grassland => 2.0,
                Biome::Steppe => 1.5,
                Biome::HotDesert => 0.2,
                Biome::Tundra | Biome::Alpine | Biome::IceCap => 0.0,
            },
            _ => 0.0,
        };
        // Managed land stays as is; wild land follows the real greenness.
        if matches!(c, Cover::Park | Cover::Residential | Cover::Farmland) { d } else { d * (0.05 + 0.95 * green) }
    }
}

/// Plants per hectare of closed canopy (100 % tree share), by biome.
fn forest_max(b: Biome) -> f32 {
    match b {
        Biome::TropicalRainforest => 450.0,
        Biome::TemperateForest => 300.0,
        Biome::Boreal => 350.0,
        Biome::Mediterranean => 160.0,
        Biome::Savanna => 90.0,
        Biome::Grassland => 80.0,
        Biome::Steppe => 50.0,
        Biome::HotDesert => 25.0,
        Biome::Alpine => 160.0,
        Biome::Tundra => 60.0,
        Biome::IceCap => 0.0,
    }
}

/// Shrubs per hectare of full shrub cover, by biome (dry scrub is sparse).
fn shrub_max(b: Biome) -> f32 {
    match b {
        Biome::HotDesert => 60.0,
        Biome::Steppe | Biome::Savanna => 120.0,
        Biome::Tundra | Biome::Alpine => 150.0,
        Biome::IceCap => 0.0,
        _ => 250.0,
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
        Biome::Steppe => rgb(0xbfae78),
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

    fn landcover(class: u8, tree: u8, shrub: u8) -> ed_data::worldcover::LandCover {
        ed_data::worldcover::LandCover { w: 4, h: 4, class: vec![class; 16], tree: vec![tree; 16], shrub: vec![shrub; 16] }
    }

    #[test]
    fn worldcover_fills_unmapped_cells_and_drives_density() {
        let tile = TileId::new(12, 2200, 1500);
        let forest = SurfaceMap::uniform(tile, Biome::TemperateForest).with_worldcover(&landcover(10, 230, 0));
        assert_eq!(forest.cover_at(0.5, 0.5), Cover::Wood);
        let desert = SurfaceMap::uniform(tile, Biome::HotDesert).with_worldcover(&landcover(60, 0, 0));
        assert_eq!(desert.cover_at(0.5, 0.5), Cover::Bare);
        let (dense, sparse) = (forest.tree_density(0.5, 0.5), desert.tree_density(0.5, 0.5));
        assert!(dense > 150.0, "closed forest should be dense, got {dense}");
        assert!(sparse < 1.0, "desert gets (almost) no trees, got {sparse}");
    }

    #[test]
    fn osm_polygons_beat_worldcover() {
        let tile = TileId::new(12, 2200, 1500);
        let mut m = SurfaceMap::uniform(tile, Biome::TemperateForest);
        m.cover = vec![Cover::Park; 1];
        let m = m.with_worldcover(&landcover(60, 0, 0));
        assert_eq!(m.cover_at(0.5, 0.5), Cover::Park);
    }

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

    fn map(c: Cover, b: Biome) -> SurfaceMap {
        let tile = TileId::new(14, 8544, 5827);
        let mut sm = SurfaceMap::uniform(tile, b);
        sm.cover = vec![c];
        sm
    }

    fn dominant(w: &[f32; LAYERS]) -> Ground {
        let (i, _) = w.iter().enumerate().fold((0, -1.0), |a, (i, &x)| if x > a.1 { (i, x) } else { a });
        Ground::ALL[i]
    }

    #[test]
    fn splat_is_normalized_and_sensible() {
        let cases = [
            (Cover::Wood, Biome::TemperateForest, 200.0, 0.0, Ground::ForestFloor),
            (Cover::Sand, Biome::HotDesert, 300.0, 0.0, Ground::Sand),
            (Cover::Grass, Biome::TemperateForest, 200.0, 0.0, Ground::Grass),
            (Cover::Commercial, Biome::TemperateForest, 200.0, 0.0, Ground::Paving),
            (Cover::Farmland, Biome::Grassland, 200.0, 0.0, Ground::Farmland),
            (Cover::Grass, Biome::Alpine, 1500.0, 0.6, Ground::Rock),
            (Cover::Natural, Biome::Alpine, 4500.0, 0.05, Ground::Snow),
        ];
        for (c, b, h, s, want) in cases {
            let sm = map(c, b);
            let mut hits = 0;
            for k in 0..16 {
                let (u, v) = (0.03 + k as f64 * 0.06, 0.5);
                let w = sm.splat(u, v, h, s);
                let sum: f32 = w.iter().sum();
                assert!((sum - 1.0).abs() < 1e-4, "{c:?} sum {sum}");
                if dominant(&w) == want {
                    hits += 1;
                }
            }
            assert!(hits >= 10, "{c:?}/{b:?}: {want:?} dominant only {hits}/16");
        }
    }

    #[test]
    fn ground_color_tracks_biome() {
        let desert = map(Cover::Natural, Biome::HotDesert).ground_color(0.5, 0.5, 300.0, 0.0);
        let forest = map(Cover::Grass, Biome::TropicalRainforest).ground_color(0.5, 0.5, 300.0, 0.0);
        assert!(desert[0] > forest[0] && desert[0] > desert[2]);
        assert!(forest[1] > forest[0]);
    }
}
