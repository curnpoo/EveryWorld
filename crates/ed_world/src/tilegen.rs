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
    /// Baked DEM detail (same size and layout as `sat_tex`, see `detail_normal`).
    pub detail_tex: Option<(u32, Vec<u8>)>,
    /// Far sun-shadow textures (`EW_FAR_SHADOWS=1` only, see `farshadow.rs`).
    pub far_shadow: Option<crate::farshadow::FarShadowTex>,
    pub collider: Option<(Vec<Vec3>, Vec<[u32; 3]>)>,
    /// Bridge decks and tunnel tubes (the terrain collider does not cover them).
    pub road_collider: Option<(Vec<Vec3>, Vec<[u32; 3]>)>,
    /// Road network for AI and traffic (nodes, lanes, speeds), tile-local.
    pub nav: Option<Arc<roads::nav::NavTile>>,
    pub water: MeshData,
    pub roads: MeshData,
    /// Distant building versions, one per detail chunk (always shown unless
    /// the chunk's full-detail version is loaded).
    pub building_lod: Vec<((u8, u8), BuildingMeshes)>,
    /// Far massing (extruded footprints + flat roofs) for tiles past `FAR_LOD_M`.
    pub building_far: MeshData,
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
    /// Share of the tile's ground at or below sea level. The elevation data clamps the ocean to
    /// 0 m, so the minimum height cannot tell the sea from a coastal plain at 0 m, but the area
    /// can. The renderer uses this to decide whether the ocean waves are worth simulating.
    pub sea_fraction: f32,
    /// Finest tiles only: what the game needs to scatter near-camera ground
    /// cover (grass, pebbles, ...) over this tile.
    pub ground: Option<std::sync::Arc<crate::ground_scatter::GroundSource>>,
    /// Finest tiles only: distances to sound sources + built-up share (`soundmap`).
    pub sound: Option<std::sync::Arc<crate::soundmap::SoundMap>>,
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
    if tile.z >= 14 && roads::v2_enabled() {
        // The road network is assembled from the 3x3 block around the tile.
        roads::ground::prefetch(&job.cache, tile);
        if let Some(src) = ed_data::roadz::source_for(tile) {
            for dy in -1..=1i64 {
                for dx in -1..=1i64 {
                    if let Some(n) = tile.offset(dx, dy) {
                        ed_data::roadz::prefetch(&job.cache, src, n);
                    }
                }
            }
        }
        if let Some(tpl) = &job.vector_template {
            for (dx, dy) in [
                (-1, -1),
                (0, -1),
                (1, -1),
                (-1, 0),
                (1, 0),
                (-1, 1),
                (0, 1),
                (1, 1),
            ] {
                if let Some(n) = tile.offset(dx, dy) {
                    let _ = vector::prefetch(&job.cache, tpl, n);
                }
            }
        }
    }
    if crate::farshadow::enabled() {
        crate::farshadow::prefetch(&job.cache, tile);
    }
    let sat_max = ed_data::satellite::MAX_ZOOM;
    if ed_data::satellite::prefetch(&job.cache, tile.ancestor(tile.z.min(sat_max)).0).is_ok() {
        // Coarse tiles mosaic their descendants for extra texels; warm those too.
        let levels = sat_levels(tile.z).min(2);
        if levels > 0 && tile.z + levels as u8 <= sat_max {
            for id in sat_grid_ids(tile, levels) {
                let _ = ed_data::satellite::prefetch(&job.cache, id);
            }
        }
    }
}

pub(crate) fn elevation_zoom(z: u8) -> u8 {
    z.min(14)
}

/// How many times to subdivide the satellite mosaic for a tile (and thus the
/// texture side, `256 << levels`). Coarse tiles span huge areas, so they mosaic
/// deeper (2x2 children, 4x4 grandchildren, ...) to keep real texels when the
/// tile is viewed from high altitude instead of a blurry 1 km/texel smear.
/// Levels are capped at 2 (1024 px) so a tile's texture fits a SAT class.
pub fn sat_levels(z: u8) -> usize {
    if z <= 6 {
        2
    } else if z <= 9 {
        1
    } else {
        0
    }
}

/// `gn`x`gn` descendant tiles at `z + log2(gn)` covering `tile`, row-major
/// `gx + gn*gy`, clamped at the poles and wrapped at the antimeridian.
fn sat_grid_ids(tile: TileId, levels: usize) -> Vec<TileId> {
    let gn = 1usize << levels;
    let zp = tile.z + levels as u8;
    let np = TileId::n(zp);
    let mut out = Vec::with_capacity(gn * gn);
    for gy in 0..gn {
        for gx in 0..gn {
            let x = (tile.x * gn as u32 + gx as u32) % np;
            let y = (tile.y * gn as u32 + gy as u32).min(np - 1);
            out.push(TileId::new(zp, x, y));
        }
    }
    out
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

    /// `lite`: mid-distance level (`buildings::build_detail_lite`).
    pub fn build_chunk(&self, c: (u8, u8), lite: bool) -> BuildingMeshes {
        match self.chunks.get(&c) {
            Some(specs) if lite => buildings::build_detail_lite(specs.iter(), &self.ctx),
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
    let mut dem = None;
    // Far tiles get denser meshes so silhouettes keep their ridgelines; the
    // baked detail texture carries what is still finer than a quad.
    let segs = if tile.z >= 14 { 192 } else if tile.z >= 13 { 128 } else { 96 };
    let mut ez = elevation_zoom(tile.z);
    while grid.is_none() {
        let src_id = tile.ancestor(ez).0;
        match elevation::fetch(&job.cache, src_id) {
            Ok(h) => {
                grid = Some(TerrainGrid::from_heights(tile, &h, segs));
                dem = Some(h);
            }
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
    // Erosion-style relief on steep high ground (see `amplify`): goes into the
    // grid itself so mesh, collider and everything placed on it agree.
    let amp = dem.as_ref().map(|h| crate::amplify::Amplifier::new(tile, h));
    if let Some(a) = &amp {
        crate::amplify::apply(&mut grid, a);
    }
    let (hmin, hmax) = grid.min_max();
    // How much of the tile is at or below sea level (the ocean is clamped to 0 m in the
    // elevation data, so this is what separates the sea from a coastal plain at 0 m).
    let sea_fraction = if grid.heights.is_empty() {
        0.0
    } else {
        grid.heights.iter().filter(|h| **h <= 0.0).count() as f32 / grid.heights.len() as f32
    };
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
    // ESA WorldCover fills what OSM leaves unmapped (forest, scrub, bare ground)
    // and supplies canopy fractions for tree density. Optional: offline keeps OSM-only cover.
    if tile.z >= 8 {
        let (nw, se) = (tile.uv_to_geodetic(0.0, 0.0), tile.uv_to_geodetic(1.0, 1.0));
        let n = if tile.z >= 13 { 96 } else { 64 };
        match ed_data::worldcover::sample_area(se.lat_deg, nw.lon_deg, nw.lat_deg, se.lon_deg, n, n, 2, &job.cache) {
            Ok(lc) => surface = surface.with_worldcover(&lc),
            Err(e) => errors.push(format!("worldcover: {e}")),
        }
    }
    if tile.z >= 10 {
        let c = tile.center();
        match ed_data::geology::lookup(c.lat_deg, c.lon_deg, &job.cache) {
            Ok(l) => surface.lith = l,
            Err(e) => errors.push(format!("geology: {e}")),
        }
    }
    // Real-world colour (Sentinel-2 cloudless). Optional: offline just means no tint.
    match ed_data::satellite::fetch(&job.cache, tile.ancestor(tile.z.min(ed_data::satellite::MAX_ZOOM)).0) {
        Ok(s) => {
            surface = surface.with_satellite(s);
            // Coarse tiles span huge areas: mosaic their descendants (2x2, 4x4, ...)
            // so a high view keeps real texels instead of a blurry smear.
            let levels = sat_levels(tile.z).min(2);
            if levels > 0 && tile.z + levels as u8 <= ed_data::satellite::MAX_ZOOM {
                let gn = 1usize << levels;
                let tiles: Vec<_> = sat_grid_ids(tile, levels)
                    .into_iter()
                    .map(|id| ed_data::satellite::fetch(&job.cache, id))
                    .collect::<Result<_, _>>()
                    .unwrap_or_default();
                if tiles.len() == gn * gn {
                    surface = surface.with_satellite_grid(tiles, gn);
                }
            }
        }
        Err(e) => errors.push(format!("satellite: {e}")),
    }

    let uncarved = grid.clone();
    if vt.is_some() {
        water::carve_terrain(&mut grid, &surface, 2.5);
    }
    // Roads: decide bed heights, then cut/fill the grid to them so the road,
    // its collider, buildings and trees all share one ground.
    let mut road_net: Option<(
        roads::net::RoadNet,
        roads::solve::Solved,
        roads::ground::GroundFn,
    )> = None;
    let road_plan = match &vt {
        Some((vid, t)) if tile.z >= 14 && *vid == tile && roads::v2_enabled() => {
            // v2: one network over the 3x3 block of vector tiles, solved for heights in world space.
            let (net, solved, gfn) =
                roads::build_v2(&job.cache, job.vector_template.as_deref(), tile, t);
            if std::env::var_os("EW_TILE_STATS").is_some() {
                let st = &solved.stats;
                eprintln!(
                    "ROADS v2 z{} edges={} tagged={} samples={} bridges={} tunnels={} measured={} crossings={} deficits={} max_fill={:.1} max_cut={:.1}",
                    tile.z,
                    net.edges.len(),
                    net.edges.iter().filter(|e| !e.tags.is_empty()).count(),
                    st.samples,
                    st.bridges,
                    st.tunnels,
                    st.measured,
                    st.crossings,
                    st.deficits.len(),
                    st.max_fill,
                    st.max_cut
                );
            }
            let tp = std::time::Instant::now();
            let p = roads::plan2::plan_v2(&net, &solved, &proj, &grid, &surface);
            if std::env::var_os("EW_ROAD_TALLY").is_some() {
                eprintln!("  ROADTIME plan_v2={:?}", tp.elapsed());
            }
            if p.grades_terrain() {
                grid = grid.upsampled(2);
                roads::grade(&mut grid, &p);
                roads::plan2::carve_holes(&mut grid, &p);
            }
            road_net = Some((net, solved, gfn));
            Some(p)
        }
        Some((vid, t)) if tile.z >= 14 && *vid == tile => {
            let mut p = roads::plan(t, &proj, &grid, &surface);
            if p.grades_terrain() {
                // Roads are narrower than a source cell: densify so the bed can be cut in.
                grid = grid.upsampled(2);
                p = roads::plan(t, &proj, &grid, &surface);
                roads::grade(&mut grid, &p);
            }
            Some(p)
        }
        _ => None,
    };
    let skirt = (tile.width_m() as f32 * 0.02).clamp(5.0, 3_000.0);
    let terrain_mesh = terrain::build_mesh(&grid, &proj, &surface, skirt);
    let sat_n = 256usize << sat_levels(tile.z);
    let sat_tex = surface.sat_texture(sat_n, |u, v| grid.sample(u, v)).map(|d| (sat_n as u32, d));
    // Detail rides on the satellite layer slot, so only tiles with one get it.
    let detail_tex = match (&sat_tex, &amp) {
        (Some(_), Some(a)) => Some((sat_n as u32, crate::detail_normal::bake_with(&uncarved, a, sat_n, tile.width_m(), true))),
        _ => None,
    };
    let collider = (tile.z >= 13).then(|| terrain::build_collider(&grid, &proj));
    let up = proj.up();

    let mut out = TileBuild {
        tile,
        frame: job.frame,
        anchor: proj.anchor,
        terrain: terrain_mesh,
        sat_tex,
        detail_tex,
        far_shadow: None,
        collider,
        road_collider: None,
        nav: None,
        water: MeshData::default(),
        roads: MeshData::default(),
        building_lod: Vec::new(),
        building_far: MeshData::default(),
        building_detail: None,
        trees: trees::TileTrees::default(),
        rocks: MeshData::default(),
        rock_chunks: Vec::new(),
        street_props: MeshData::default(),
        placements: Vec::new(),
        airports: Vec::new(),
        min_max_height: (hmin, hmax),
        sea_fraction,
        ground: None,
        sound: None,
        stats: TileStats::default(),
        errors,
    };

    let mut far_bldgs = Vec::new();
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
            out.sound = Some(Arc::new(crate::soundmap::SoundMap::build(vt, &uncarved, tile.width_m() as f32)));
            let r = roads::generate(road_plan.as_ref().unwrap(), &proj, &grid, &surface, region);
            out.placements.extend(props::place_street_props(crate::library::get(), &r.props, up));
            out.stats.props = r.props.len();
            // Overture's footprints (baked offline) replace OpenFreeMap's where available.
            let mut overture = ed_data::overture::building_tile(&job.cache, tile);
            let wm = tile.width_m();
            // Parking structures become open, drivable decks instead of boxes (Overture's `class = parking`).
            let mut garage_vis = MeshData::default();
            let mut garage_col = MeshData::default();
            if let Some(ot) = overture.as_mut() {
                for layer in ot.layers.iter_mut().filter(|l| l.name == "building") {
                    let ext = layer.extent as f64;
                    layer.features.retain(|f| {
                        if f.str("class") != Some("parking")
                            || f.num("render_min_height").unwrap_or(0.0) > 0.5
                        {
                            return true;
                        }
                        let Some((outer, _)) = ed_data::mvt::polygons(f).into_iter().next() else {
                            return true;
                        };
                        let m2 =
                            ed_data::mvt::ring_area(&outer).abs() as f64 * (wm / ext) * (wm / ext);
                        if (m2 as f32) < crate::garage::MIN_AREA_M2 {
                            return true;
                        }
                        let ring: Vec<Vec3> = outer
                            .iter()
                            .map(|p| {
                                proj.local(
                                    p[0] as f64 / ext,
                                    p[1] as f64 / ext,
                                    grid.sample(
                                        (p[0] as f64 / ext).clamp(0.0, 1.0),
                                        (p[1] as f64 / ext).clamp(0.0, 1.0),
                                    ) as f64,
                                )
                            })
                            .collect();
                        let floors =
                            crate::garage::floors_for(f.num("render_height").map(|h| h as f32));
                        match crate::garage::build(&ring, up, floors) {
                            Some(g) => {
                                garage_vis.append(&g.visible);
                                garage_col.append(&g.collider);
                                false
                            }
                            None => true,
                        }
                    });
                }
            }
            let bvt = overture.as_ref().unwrap_or(vt);
            let keep = road_net
                .as_ref()
                .map(|(n, _, _)| roads::keepout::KeepOut::new(n));
            let keep_fn = keep
                .as_ref()
                .map(|k| move |u: f64, v: f64| k.blocks(u * wm, v * wm));
            let ctx = BuildingContext {
                proj: &proj,
                grid: &grid,
                surface: &surface,
                biome,
                roads: &r.centerlines,
                keepout: keep_fn
                    .as_ref()
                    .map(|f| f as &(dyn Fn(f64, f64) -> bool + Sync)),
            };
            let specs = buildings::prepare(bvt, &ctx, region);
            out.stats.buildings = specs.len();
            if crate::farshadow::enabled() {
                far_bldgs.extend(specs.iter().map(|s| s.shadow_outline()));
            }
            let mut chunks: HashMap<(u8, u8), Vec<buildings::BuildingSpec>> = HashMap::new();
            for s in specs {
                chunks.entry(s.chunk).or_default().push(s);
            }
            let mut lod: Vec<((u8, u8), BuildingMeshes)> = chunks.iter().map(|(c, v)| (*c, buildings::build_lod(v.iter(), up))).collect();
            lod.sort_by_key(|(c, _)| *c);
            out.building_lod = lod;
            out.building_far = buildings::build_massing(chunks.values().flatten(), up);
            out.building_detail = Some(Arc::new(DetailSet {
                ctx: buildings::DetailCtx { proj, grid: grid.clone(), roads: r.centerlines.clone() },
                chunks,
            }));
            let mut deck_col = r.collider;
            deck_col.append(&garage_col);
            if !deck_col.is_empty() {
                let verts: Vec<Vec3> = deck_col.positions.iter().map(|p| Vec3::from(*p)).collect();
                let tris: Vec<[u32; 3]> = deck_col
                    .indices
                    .chunks_exact(3)
                    .map(|t| [t[0], t[1], t[2]])
                    .collect();
                out.road_collider = Some((verts, tris));
            }
            out.street_props.append(&garage_vis);
            if let Some((net, solved, gfn)) = &road_net {
                out.nav = Some(Arc::new(roads::nav::NavTile::build(
                    net, solved, &**gfn, &proj, region,
                )));
            }
            out.roads = r.mesh;
        } else if tile.z == 13 && *vid == tile {
            // Far tiles: real OSM footprints as cheap massing only (no roads, props, detail, colliders).
            let region = Region::locate(
                center.lat_deg,
                center.lon_deg,
                ((hmin + hmax) * 0.5) as f64,
                buildings::tile_is_dense(vt),
            );
            let ctx = BuildingContext {
                proj: &proj,
                grid: &grid,
                surface: &surface,
                biome,
                roads: &[],
                keepout: None,
            };
            let specs = buildings::prepare(vt, &ctx, region);
            out.stats.buildings = specs.len();
            let mut chunks: HashMap<(u8, u8), Vec<buildings::BuildingSpec>> = HashMap::new();
            for s in specs {
                chunks.entry(s.chunk).or_default().push(s);
            }
            let mut lod: Vec<((u8, u8), BuildingMeshes)> = chunks.iter().map(|(c, v)| (*c, buildings::build_lod(v.iter(), up))).collect();
            lod.sort_by_key(|(c, _)| *c);
            out.building_lod = lod;
            out.building_far = buildings::build_massing(chunks.values().flatten(), up);
        }
        if tile.z >= 10 {
            let occ = foliage::Occupancy::from_vector(vt, 512);
            let max = match tile.z {
                14.. => 16_000,
                12 | 13 => 10_000,
                11 => 7_000,
                _ => 5_000,
            };
            let (trees, rocks, kept) = foliage::scatter(&proj, &grid, &surface, Some(&occ), max);
            out.stats.trees = trees.len();
            out.stats.rocks = rocks.len();
            // Fine tiles: chunks with far cards and instances, so the game can
            // swap in full detail near the camera; impostors take over past
            // the cards. Coarse tiles: impostors only.
            let fine = tile.z >= 14;
            let near_cut = if fine { trees::impostor::CARD_RANGE_M - 210.0 } else { 0.0 };
            let impostors = trees::impostor::build_mesh(&trees, up, Vec3::ZERO, kept, near_cut, Some(&proj.uv_map()));
            out.trees = if fine { trees::bake_tile(trees, up, proj.tile.width_m() as f32, 12) } else { trees::TileTrees { up, chunks: Vec::new(), impostors: Default::default() } };
            out.trees.impostors = impostors;
            // Rocks are small: only fine tiles show them, in culled chunks.
            if tile.z >= 14 {
                out.rock_chunks = props::bake_rocks_chunked(&rocks, up, proj.tile.width_m() as f32, 16);
                // Cliff kits: faceted slabs on steep faces (see `cliffs`).
                let slabs = crate::cliffs::scatter(&proj, &grid, &surface);
                out.stats.rocks += slabs.len();
                out.rock_chunks.extend(crate::cliffs::bake_chunked(&slabs, up, proj.tile.width_m() as f32, 16));
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
    if crate::farshadow::enabled() {
        let inp = crate::farshadow::TileInputs { tile, grid: &grid, hmin, surface: &surface, proj: &proj, buildings: far_bldgs };
        out.far_shadow = Some(crate::farshadow::bake(&job.cache, &inp));
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

    /// Legacy versus v2 roads on real tiles, including big interchanges. Prints triangle counts, solver
    /// statistics and timings. `cargo test --release -p ed_world road_compare -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn road_compare() {
        let cache = DiskCache::new(std::env::temp_dir().join("ed-net-test"));
        let tpl = vector::tile_template(&cache).ok();
        for (name, lat, lon) in [
            ("Central Park", 40.7812, -73.9665),
            ("Zermatt", 46.0207, 7.7491),
            ("Shibuya", 35.6595, 139.7005),
            ("LA Four Level", 34.0561, -118.2358),
            ("Dallas High Five", 32.9056, -96.7693),
            ("Zurich Hardbruecke", 47.3857, 8.5171),
        ] {
            let tile = TileId::containing(lat, lon, 14);
            let job = TileJob {
                tile,
                frame: EnuFrame::new(Geodetic::new(lat, lon, 0.0)),
                cache: cache.clone(),
                vector_template: tpl.clone(),
            };
            prefetch(&job);
            for v2 in [false, true] {
                roads::force_v2(Some(v2));
                let t0 = std::time::Instant::now();
                let b = build(&job);
                let col = b.road_collider.as_ref().map_or(0, |c| c.1.len());
                eprintln!(
                    "{name:18} {} road_tris={:7} deck_collider_tris={:6} terrain_collider_tris={:6} nav_edges={:4} total={:?} errors={}",
                    if v2 { "v2    " } else { "legacy" },
                    b.roads.triangle_count(),
                    col,
                    b.collider.as_ref().map_or(0, |c| c.1.len()),
                    b.nav.as_ref().map_or(0, |n| n.edges.len()),
                    t0.elapsed(),
                    b.errors.len()
                );
            }
        }
        roads::force_v2(None);
    }

    /// Measured heights on real tiles (France: IGN, Norway: NVDB): how many decks and tunnels the survey covers.
    #[test]
    #[ignore]
    fn measured_real() {
        let cache = DiskCache::new(std::env::temp_dir().join("ed-net-test"));
        let tpl = vector::tile_template(&cache).ok();
        for (name, lat, lon) in [("Paris Pont Neuf", 48.8566, 2.3412), ("Oslo Bjorvika", 59.9085, 10.7480), ("Lyon", 45.7578, 4.8320)] {
            let tile = TileId::containing(lat, lon, 14);
            let vt = vector::fetch(&cache, tpl.as_ref().unwrap(), tile).unwrap();
            let t0 = std::time::Instant::now();
            let (net, solved, _) = roads::build_v2(&cache, tpl.as_deref(), tile, &vt);
            let st = &solved.stats;
            eprintln!(
                "{name}: edges={} tagged={} bridges={} tunnels={} measured={} crossings={} deficits={} {:?}",
                net.edges.len(), net.edges.iter().filter(|e| !e.tags.is_empty()).count(), st.bridges, st.tunnels, st.measured, st.crossings, st.deficits.len(), t0.elapsed()
            );
        }
    }

    /// Garages from baked Overture tiles: `tools/bake_overture.py` first.
    #[test]
    #[ignore]
    fn garages_real() {
        let cache = DiskCache::new(std::env::temp_dir().join("ed-net-test"));
        let tpl = vector::tile_template(&cache).ok();
        for (name, lat, lon) in [("Shibuya", 35.6595, 139.7005), ("LAX", 33.9425, -118.4081)] {
            let tile = TileId::containing(lat, lon, 14);
            let job = TileJob {
                tile,
                frame: EnuFrame::new(Geodetic::new(lat, lon, 0.0)),
                cache: cache.clone(),
                vector_template: tpl.clone(),
            };
            let b = build(&job);
            eprintln!(
                "{name}: garage+prop mesh tris={} road_collider_tris={} buildings={}",
                b.street_props.triangle_count(),
                b.road_collider.as_ref().map_or(0, |c| c.1.len()),
                b.stats.buildings
            );
        }
    }

    /// Dump the solved network of a tile as JSON for plotting: `EW_DUMP=/path/out.json EW_LL=lat,lon`.
    #[test]
    #[ignore]
    fn dump_roads() {
        let Some(out) = std::env::var_os("EW_DUMP") else {
            return;
        };
        let ll: Vec<f64> = std::env::var("EW_LL")
            .unwrap()
            .split(',')
            .map(|v| v.parse().unwrap())
            .collect();
        let cache = DiskCache::new(std::env::temp_dir().join("ed-net-test"));
        let tpl = vector::tile_template(&cache).ok();
        let tile = TileId::containing(ll[0], ll[1], 14);
        let vt = vector::fetch(&cache, tpl.as_ref().unwrap(), tile).unwrap();
        let (net, solved, gfn) = roads::build_v2(&cache, tpl.as_deref(), tile, &vt);
        let mut rows = Vec::new();
        for e in &solved.edges {
            let ne = &net.edges[e.net_idx];
            let pts: Vec<[f64; 3]> = (0..e.pts.len())
                .map(|k| [e.pts[k][0], e.pts[k][1], e.z[k] as f64])
                .collect();
            let g: Vec<f64> = (0..e.pts.len()).map(|k| e.ground[k] as f64).collect();
            rows.push(serde_json::json!({"class": format!("{:?}", ne.class), "level": ne.level(), "bridge": ne.bridge, "tunnel": ne.tunnel, "pts": pts, "ground": g}));
        }
        let cross: Vec<_> = solved
            .crossings
            .iter()
            .map(|c| serde_json::json!({"p": c.p, "upper": c.upper, "lower": c.lower}))
            .collect();
        let _ = &gfn;
        let doc = serde_json::json!({"wm": net.wm, "edges": rows, "crossings": cross, "deficits": solved.stats.deficits.iter().map(|d| serde_json::json!([d.0, d.1])).collect::<Vec<_>>()});
        std::fs::write(out, doc.to_string()).unwrap();
    }

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
                let m = d.build_chunk(c, false);
                eprintln!("    detail chunk {c:?}: {} buildings, {} tris, {:?}", m.count, m.triangle_count(), t1.elapsed());
                let mut per: Vec<_> = m.out.meshes.iter().map(|(k, v)| (format!("{k:?}"), v.triangle_count())).collect();
                per.sort_by_key(|p| std::cmp::Reverse(p.1));
                eprintln!("      {per:?}");
            }
        }
    }
}
