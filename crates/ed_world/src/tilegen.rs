//! One tile, start to finish: fetch elevation + vector data, then generate
//! terrain, ground cover, water, roads, buildings, trees, rocks and street
//! props. Runs on a worker thread; output is plain data.

use crate::biome::Biome;
use crate::buildings::{self, BuildingContext, BuildingMeshes};
use crate::mesh::MeshData;
use crate::project::TileProjector;
use crate::style::Region;
use crate::surface::SurfaceMap;
use crate::terrain::{self, TerrainGrid};
use crate::{foliage, props, roads, trees, water};
use ed_data::{DiskCache, elevation, vector};
use ed_geo::{EnuFrame, tile::TileId};
use glam::{DVec3, Vec3};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct TileJob {
    pub tile: TileId,
    pub frame: EnuFrame,
    pub cache: DiskCache,
    /// OpenFreeMap tile URL template (None = offline / not resolved yet).
    pub vector_template: Option<String>,
}

#[derive(Debug, Default)]
pub struct TileStats {
    pub buildings: usize,
    pub trees: usize,
    pub rocks: usize,
    pub props: usize,
    pub triangles: usize,
}

#[derive(Debug)]
pub struct TileBuild {
    pub tile: TileId,
    pub frame: EnuFrame,
    /// Render-space anchor (in `frame`) that all meshes are relative to.
    pub anchor: DVec3,
    pub terrain: MeshData,
    /// Satellite colour texture (size, RGBA8) over the tile's uv square.
    pub sat_tex: Option<(u32, Vec<u8>)>,
    pub collider: Option<(Vec<Vec3>, Vec<[u32; 3]>)>,
    pub water: MeshData,
    pub roads: MeshData,
    /// Distant building versions, one per detail chunk (always shown unless
    /// the chunk's full-detail version is loaded).
    pub building_lod: Vec<((u8, u8), BuildingMeshes)>,
    /// Inputs for building full-detail chunks on demand near the player.
    pub building_detail: Option<Arc<DetailSet>>,
    /// Trees and shrubs: per-chunk far-LOD meshes + instances for near detail.
    pub trees: trees::TileTrees,
    pub rocks: MeshData,
    /// Rocks split into spatial chunks (fine tiles only) for culling.
    pub rock_chunks: Vec<MeshData>,
    pub street_props: MeshData,
    /// Library asset instances (street props, ...) in tile mesh space.
    pub placements: Vec<crate::library::Placement>,
    /// (name, IATA, local position) of airports in this tile.
    pub airports: Vec<(String, String, Vec3)>,
    pub min_max_height: (f32, f32),
    /// Finest tiles only: what the game needs to scatter near-camera ground
    /// cover (grass, pebbles, ...) over this tile.
    pub ground: Option<std::sync::Arc<crate::ground_scatter::GroundSource>>,
    pub stats: TileStats,
    pub errors: Vec<String>,
}

/// Elevation zoom for a terrain tile.
/// Network half of [`build`]: download every source tile `build` will read
/// into the disk cache, without decoding. Meant for an IO thread so the CPU
/// workers that run [`build`] never block on the network. Errors are ignored
/// here; `build` retries and records them (including its coarser-zoom
/// elevation fallbacks, which are rare and stay on the build path).
pub fn prefetch(job: &TileJob) {
    let tile = job.tile;
    let _ = elevation::prefetch(&job.cache, tile.ancestor(elevation_zoom(tile.z)).0);
    if tile.z >= 7
        && let Some(tpl) = &job.vector_template
    {
        let _ = vector::prefetch(&job.cache, tpl, tile.ancestor(tile.z.min(vector::MAX_ZOOM)).0);
    }
    let sat_max = ed_data::satellite::MAX_ZOOM;
    if ed_data::satellite::prefetch(&job.cache, tile.ancestor(tile.z.min(sat_max)).0).is_ok() && tile.z < sat_max && tile.z <= 9 {
        for c in tile.children() {
            let _ = ed_data::satellite::prefetch(&job.cache, c);
        }
    }
}

fn elevation_zoom(z: u8) -> u8 {
    z.min(13)
}

/// Everything needed to build any detail chunk of a tile later.
#[derive(Debug)]
pub struct DetailSet {
    pub ctx: buildings::DetailCtx,
    pub chunks: HashMap<(u8, u8), Vec<buildings::BuildingSpec>>,
}

impl DetailSet {
    /// Chunk center in the tile's local space (relative to the tile anchor).
    pub fn chunk_center(&self, c: (u8, u8)) -> Vec3 {
        let n = buildings::CHUNKS as f64;
        let (u, v) = ((c.0 as f64 + 0.5) / n, (c.1 as f64 + 0.5) / n);
        self.ctx.proj.local(u, v, self.ctx.grid.sample(u, v) as f64)
    }

    pub fn build_chunk(&self, c: (u8, u8)) -> BuildingMeshes {
        match self.chunks.get(&c) {
            Some(specs) => buildings::build_detail(specs.iter(), &self.ctx),
            None => BuildingMeshes::default(),
        }
    }
}

pub fn build(job: &TileJob) -> TileBuild {
    let tile = job.tile;
    let proj = TileProjector::new(tile, job.frame);
    let mut errors = Vec::new();

    // --- Elevation (falls back to coarser zooms, then sea level) ---
    let mut grid = None;
    let segs = if tile.z >= 13 { 64 } else if tile.z >= 9 { 48 } else { 96 };
    let mut ez = elevation_zoom(tile.z);
    while grid.is_none() {
        let src_id = tile.ancestor(ez).0;
        match elevation::fetch(&job.cache, src_id) {
            Ok(h) => grid = Some(TerrainGrid::from_heights(tile, &h, segs)),
            Err(e) if ez > 0 && ez + 3 > elevation_zoom(tile.z) => {
                errors.push(format!("elevation z{ez}: {e}"));
                ez -= 1;
            }
            Err(e) => {
                errors.push(format!("elevation: {e}"));
                grid = Some(TerrainGrid::flat(tile, segs, 0.0));
            }
        }
    }
    let mut grid = grid.unwrap();
    let (hmin, hmax) = grid.min_max();
    let center = tile.center();
    let biome = Biome::classify(center.lat_deg, center.lon_deg, ((hmin + hmax) * 0.5).max(0.0) as f64);

    // --- Vector data ---
    let vt = if tile.z >= 7 {
        job.vector_template.as_ref().and_then(|tpl| {
            let vz = tile.z.min(vector::MAX_ZOOM);
            let vid = tile.ancestor(vz).0;
            match vector::fetch(&job.cache, tpl, vid) {
                Ok(t) => Some((vid, t)),
                Err(e) => {
                    errors.push(format!("vector: {e}"));
                    None
                }
            }
        })
    } else {
        None
    };
    let mut surface = match &vt {
        Some((vid, t)) => SurfaceMap::from_vector(tile, *vid, t, biome, if tile.z >= 13 { 192 } else { 64 }),
        None => SurfaceMap::uniform(tile, biome),
    };
    // Real-world colour (Sentinel-2 cloudless). Optional: offline just means no tint.
    match ed_data::satellite::fetch(&job.cache, tile.ancestor(tile.z.min(ed_data::satellite::MAX_ZOOM)).0) {
        Ok(s) => {
            surface = surface.with_satellite(s);
            // Coarse tiles span huge areas: mosaic the four children for 2x the texels.
            if tile.z < ed_data::satellite::MAX_ZOOM && tile.z <= 9 {
                let k: Vec<_> = tile.children().iter().filter_map(|c| ed_data::satellite::fetch(&job.cache, *c).ok()).collect();
                if let Ok(k) = <[_; 4]>::try_from(k) {
                    surface = surface.with_satellite_kids(k);
                }
            }
        }
        Err(e) => errors.push(format!("satellite: {e}")),
    }

    let uncarved = grid.clone();
    if vt.is_some() {
        water::carve_terrain(&mut grid, &surface, 2.5);
    }
    let skirt = (tile.width_m() as f32 * 0.02).clamp(5.0, 3_000.0);
    let terrain_mesh = terrain::build_mesh(&grid, &proj, &surface, skirt);
    let sat_n = if tile.z <= 9 { 512 } else if tile.z <= 12 { 256 } else { 128 };
    let sat_tex = surface.sat_texture(sat_n, |u, v| grid.sample(u, v)).map(|d| (sat_n as u32, d));
    let collider = (tile.z >= 13).then(|| terrain::build_collider(&grid, &proj));
    let up = proj.up();

    let mut out = TileBuild {
        tile,
        frame: job.frame,
        anchor: proj.anchor,
        terrain: terrain_mesh,
        sat_tex,
        collider,
        water: MeshData::default(),
        roads: MeshData::default(),
        building_lod: Vec::new(),
        building_detail: None,
        trees: trees::TileTrees::default(),
        rocks: MeshData::default(),
        rock_chunks: Vec::new(),
        street_props: MeshData::default(),
        placements: Vec::new(),
        airports: Vec::new(),
        min_max_height: (hmin, hmax),
        ground: None,
        stats: TileStats::default(),
        errors,
    };

    if let Some((vid, vt)) = &vt {
        // Airports (labels exist at lower zooms too).
        if let Some(l) = vt.layer("aerodrome_label") {
            let ext = l.extent as f64;
            for f in &l.features {
                if let Some(p) = f.rings.first().and_then(|r| r.first()) {
                    let (u, v) = (p[0] as f64 / ext, p[1] as f64 / ext);
                    if *vid == tile && (0.0..1.0).contains(&u) && (0.0..1.0).contains(&v) {
                        out.airports.push((
                            f.str("name:en").or(f.str("name")).unwrap_or("Airport").to_string(),
                            f.str("iata").unwrap_or("").to_string(),
                            proj.local(u, v, grid.sample(u, v) as f64),
                        ));
                    }
                }
            }
        }
        if tile.z >= 14 && *vid == tile {
            let (gmin, gmax) = (hmin, hmax);
            let region = Region::locate(center.lat_deg, center.lon_deg, ((gmin + gmax) * 0.5) as f64, buildings::tile_is_dense(vt));
            out.water = water::generate(vt, &proj, &uncarved);
            let r = roads::generate(vt, &proj, &grid, &surface, region);
            out.placements.extend(props::place_street_props(crate::library::get(), &r.props, up));
            out.stats.props = r.props.len();
            let ctx = BuildingContext { proj: &proj, grid: &grid, surface: &surface, biome, roads: &r.centerlines };
            let specs = buildings::prepare(vt, &ctx, region);
            out.stats.buildings = specs.len();
            let mut chunks: HashMap<(u8, u8), Vec<buildings::BuildingSpec>> = HashMap::new();
            for s in specs {
                chunks.entry(s.chunk).or_default().push(s);
            }
            let mut lod: Vec<((u8, u8), BuildingMeshes)> = chunks.iter().map(|(c, v)| (*c, buildings::build_lod(v.iter(), up))).collect();
            lod.sort_by_key(|(c, _)| *c);
            out.building_lod = lod;
            out.building_detail = Some(Arc::new(DetailSet {
                ctx: buildings::DetailCtx { proj, grid: grid.clone(), roads: r.centerlines.clone() },
                chunks,
            }));
            out.roads = r.mesh;
        }
        if tile.z >= 10 {
            let occ = foliage::Occupancy::from_vector(vt, 512);
            let max = match tile.z {
                14.. => 9_000,
                12 | 13 => 6_000,
                11 => 5_000,
                _ => 4_000,
            };
            let (trees, rocks, kept) = foliage::scatter(&proj, &grid, &surface, Some(&occ), max);
            out.stats.trees = trees.len();
            out.stats.rocks = rocks.len();
            // Fine tiles: chunks with far cards and instances, so the game can
            // swap in full detail near the camera; impostors take over past
            // the cards. Coarse tiles: impostors only.
            let fine = tile.z >= 14;
            let near_cut = if fine { trees::impostor::CARD_RANGE_M - 150.0 } else { 0.0 };
            let impostors = trees::impostor::build_mesh(&trees, up, Vec3::ZERO, kept, near_cut);
            out.trees = if fine { trees::bake_tile(trees, up, proj.tile.width_m() as f32, 12) } else { trees::TileTrees { up, chunks: Vec::new(), impostors: Default::default() } };
            out.trees.impostors = impostors;
            // Rocks are small: only fine tiles show them, in culled chunks.
            if tile.z >= 14 {
                out.rock_chunks = props::bake_rocks_chunked(&rocks, up, proj.tile.width_m() as f32, 16);
            }
            if tile.z >= 14 {
                out.ground = Some(std::sync::Arc::new(crate::ground_scatter::GroundSource {
                    proj,
                    grid: grid.clone(),
                    surface: surface.clone(),
                    occupancy: Some(occ),
                }));
            }
        }
    }
    out.stats.triangles = out.terrain.triangle_count()
        + out.water.triangle_count()
        + out.roads.triangle_count()
        + out.trees.triangle_count()
        + out.rocks.triangle_count()
        + out.rock_chunks.iter().map(|m| m.triangle_count()).sum::<usize>()
        + out.street_props.triangle_count()
        + out.building_lod.iter().map(|(_, b)| b.triangle_count()).sum::<usize>();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed_geo::Geodetic;

    /// Real data, real places. `cargo test -p ed_world tilegen -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_places() {
        let cache = DiskCache::new(std::env::temp_dir().join("ed-net-test"));
        let tpl = vector::tile_template(&cache).ok();
        for (name, lat, lon) in [
            ("Central Park", 40.7812, -73.9665),
            ("Zermatt", 46.0207, 7.7491),
            ("Aix-en-Provence", 43.5297, 5.4474),
            ("Marrakech", 31.6295, -7.9811),
            ("Shibuya", 35.6595, 139.7005),
        ] {
            let tile = TileId::containing(lat, lon, 14);
            let job = TileJob { tile, frame: EnuFrame::new(Geodetic::new(lat, lon, 0.0)), cache: cache.clone(), vector_template: tpl.clone() };
            let t0 = std::time::Instant::now();
            let b = build(&job);
            eprintln!(
                "{name:16} {:?} h={:.0}..{:.0} bldg={} trees={} rocks={} props={} tris={} airports={} {:?} errors={:?}",
                tile, b.min_max_height.0, b.min_max_height.1, b.stats.buildings, b.stats.trees, b.stats.rocks, b.stats.props,
                b.stats.triangles, b.airports.len(), t0.elapsed(), b.errors
            );
            assert!(b.stats.triangles > 1000);
            if let Some(d) = &b.building_detail {
                let (u, v) = tile.geodetic_to_uv(lat, lon);
                let c = ((u * buildings::CHUNKS as f64) as u8, (v * buildings::CHUNKS as f64) as u8);
                let t1 = std::time::Instant::now();
                let m = d.build_chunk(c);
                eprintln!("    detail chunk {c:?}: {} buildings, {} tris, {:?}", m.count, m.triangle_count(), t1.elapsed());
                let mut per: Vec<_> = m.out.meshes.iter().map(|(k, v)| (format!("{k:?}"), v.triangle_count())).collect();
                per.sort_by_key(|p| std::cmp::Reverse(p.1));
                eprintln!("      {per:?}");
            }
        }
    }
}
