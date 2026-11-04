//! Terrain/world tile streaming. Picks the tile set from the quadtree LOD,
//! builds missing tiles on worker threads, and swaps them in without holes
//! (a coarse tile stays until every finer tile covering it has arrived).

use crate::assets::WorldMaterials;
use crate::building_materials::BuildingMaterials;
use crate::config::Config;
use crate::meshconv::to_mesh;
use crate::origin::{Focus, GeoAnchored, WorldOrigin};
use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::tasks::AsyncComputeTaskPool;
use crossbeam_channel::{Receiver, Sender};
use ed_geo::tile::TileId;
use ed_world::bparts::Mat;
use ed_world::buildings::BuildingMeshes;
use ed_world::lod::{self, LodParams};
use ed_world::tilegen::{self, DetailSet, TileBuild, TileJob};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub struct StreamingPlugin;

impl Plugin for StreamingPlugin {
    fn build(&self, app: &mut App) {
        let (tx, rx) = crossbeam_channel::unbounded();
        let (dtx, drx) = crossbeam_channel::unbounded();
        app.insert_resource(TileManager { tx, rx, ..default_manager() })
            .insert_resource(DetailManager { tx: dtx, rx: drx, tiles: HashMap::new(), in_flight: 0, last: -1.0 })
            .add_systems(Startup, resolve_vector_template)
            .add_systems(
                Update,
                (
                    // The main menu is just a map: don't load the world until a spawn is chosen.
                    select_tiles.run_if(not(in_state(crate::menu::GameState::Menu))),
                    receive_tiles,
                    retire_tiles,
                    stream_detail.run_if(not(in_state(crate::menu::GameState::Menu))),
                    receive_detail,
                )
                    .chain(),
            );
    }
}

/// Tile builds are CPU-heavy; keep some cores free for the render and main threads.
const MAX_IN_FLIGHT: usize = 4;

/// Building geometry already converted to Bevy meshes/colliders, so the main
/// thread only has to register assets (trimesh BVH builds are slow).
struct BuildingBatch {
    buckets: Vec<(Mat, Mesh)>,
    collider: Option<Collider>,
}

fn prepare_batch(b: &BuildingMeshes, with_collider: bool) -> BuildingBatch {
    let buckets = b.out.meshes.iter().filter(|(_, m)| !m.is_empty()).map(|(mat, m)| (*mat, to_mesh(m))).collect();
    let collider = (with_collider && !b.collider.is_empty()).then(|| {
        let col = &b.collider;
        let verts: Vec<Vec3> = col.positions.iter().map(|p| Vec3::from(*p)).collect();
        let tris: Vec<[u32; 3]> = col.indices.chunks_exact(3).map(|t| [t[0], t[1], t[2]]).collect();
        Collider::trimesh(verts, tris)
    });
    BuildingBatch { buckets, collider }
}

use bevy::pbr::experimental::meshlet::{MeshletMesh, MeshletMesh3d};

/// Convert a palettized per-tile mesh to a meshlet mesh, recentred on its AABB
/// centre (returned as the entity offset): Bevy's cluster verifier asserts bounding
/// spheres nest within 1e-4, which f32 cannot hold km from the origin.
fn to_meshlet(m: &ed_world::mesh::MeshData) -> Option<(MeshletMesh, Vec3)> {
    if m.positions.is_empty() { return None; }
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for p in &m.positions { lo = lo.min(Vec3::from(*p)); hi = hi.max(Vec3::from(*p)); }
    let c = (lo + hi) * 0.5;
    let mut local = ed_world::mesh::MeshData { positions: m.positions.iter().map(|p| (Vec3::from(*p) - c).to_array()).collect(), ..Default::default() };
    local.normals = m.normals.clone();
    local.uvs = m.uvs.clone();
    local.indices = m.indices.clone();
    crate::library::meshlet_mesh(&local).map(|ml| (ml, c))
}

#[derive(Clone, Copy)]
enum Part {
    Terrain,
    Water,
    Roads,
    Rocks,
    Props,
}

/// A finished tile with every Bevy-side asset built off the main thread.
struct ReadyTile {
    build: TileBuild,
    collider: Option<Collider>,
    parts: Vec<(Part, Mesh)>,
    /// Opaque static meshes converted to meshlets (virtual geometry) off-thread.
    meshlets: Vec<(MeshletMesh, Vec3)>,
    lod: Vec<((u8, u8), BuildingBatch)>,
}

fn ready_tile(mut b: TileBuild) -> ReadyTile {
    let collider = b.collider.take().map(|(verts, tris)| Collider::trimesh(verts, tris));
    let parts = [
        (Part::Terrain, &b.terrain),
        (Part::Water, &b.water),
        (Part::Roads, &b.roads),
        (Part::Rocks, &b.rocks),
        (Part::Props, &b.street_props),
    ]
    .into_iter()
    .filter(|(_, m)| !m.is_empty())
    .map(|(p, m)| (p, to_mesh(m)))
    .collect::<Vec<_>>();
    // `EW_NO_MESHLET=1` draws the same chunks as ordinary meshes (A/B the virtual-geometry path).
    let (meshlets, parts) = if !crate::library::meshlets_enabled() {
        let mut parts = parts;
        parts.extend(b.rock_chunks.iter().map(|m| (Part::Rocks, to_mesh(m))));
        (Vec::new(), parts)
    } else {
        (b.rock_chunks.iter().filter_map(|m| to_meshlet(m)).collect(), parts)
    };
    let lod = std::mem::take(&mut b.building_lod).into_iter().map(|(c, m)| (c, prepare_batch(&m, true))).collect();
    ReadyTile { build: b, collider, parts, meshlets, lod }
}

#[derive(Resource)]
pub struct TileManager {
    tx: Sender<ReadyTile>,
    rx: Receiver<ReadyTile>,
    template: Option<String>,
    template_resolved: bool,
    pub loaded: HashMap<TileId, Entity>,
    pub pending: HashSet<TileId>,
    pub desired: HashSet<TileId>,
    pub ground_estimate: f64,
    last_select: f32,
    pub totals: Totals,
    /// Per-tile contribution to `totals`, so retiring a tile subtracts it.
    tile_totals: HashMap<TileId, Totals>,
}

#[derive(Default, Debug, Clone, Copy)]
pub struct Totals {
    pub buildings: usize,
    pub trees: usize,
    pub triangles: usize,
}

impl TileManager {
    /// Everything requested has arrived.
    fn recount(&mut self) {
        self.totals = self.tile_totals.values().fold(Totals::default(), |a, t| Totals {
            buildings: a.buildings + t.buildings,
            trees: a.trees + t.trees,
            triangles: a.triangles + t.triangles,
        });
    }

    pub fn idle(&self) -> bool {
        self.template_resolved && self.pending.is_empty() && self.desired.iter().all(|t| self.loaded.contains_key(t))
    }
}

fn default_manager() -> TileManager {
    let (tx, rx) = crossbeam_channel::unbounded();
    TileManager {
        tx,
        rx,
        template: None,
        template_resolved: false,
        loaded: HashMap::new(),
        pending: HashSet::new(),
        desired: HashSet::new(),
        ground_estimate: 0.0,
        last_select: -1.0,
        totals: Totals::default(),
        tile_totals: HashMap::new(),
    }
}

#[derive(Component)]
pub struct WorldTile;

/// Full-detail building chunks near the player.
#[derive(Resource)]
pub struct DetailManager {
    tx: Sender<(TileId, (u8, u8), BuildingBatch)>,
    rx: Receiver<(TileId, (u8, u8), BuildingBatch)>,
    tiles: HashMap<TileId, TileDetail>,
    in_flight: usize,
    last: f32,
}

struct TileDetail {
    set: Arc<DetailSet>,
    root: Entity,
    lod: HashMap<(u8, u8), Entity>,
    loaded: HashMap<(u8, u8), Entity>,
    pending: HashSet<(u8, u8)>,
}

/// Detail is built inside this radius and dropped beyond the outer one.
const DETAIL_IN_M: f32 = 230.0;
const DETAIL_OUT_M: f32 = 320.0;
const DETAIL_IN_FLIGHT: usize = 2;

/// Spawn one entity per material bucket under `parent`.
fn spawn_buckets(commands: &mut Commands, meshes: &mut Assets<Mesh>, bm: &BuildingMaterials, b: BuildingBatch, parent: Entity) {
    for (mat, mesh) in b.buckets {
        commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(bm.get(mat)), Transform::default(), ChildOf(parent)));
    }
}

fn resolve_vector_template(cfg: Res<Config>, mut mgr: ResMut<TileManager>) {
    match ed_data::vector::tile_template(&cfg.cache) {
        Ok(t) => mgr.template = Some(t),
        Err(e) => warn!("vector tiles unavailable ({e}); terrain only"),
    }
    mgr.template_resolved = true;
}

fn select_tiles(
    time: Res<Time>,
    cfg: Res<Config>,
    origin: Res<WorldOrigin>,
    mut mgr: ResMut<TileManager>,
    focus: Query<&GlobalTransform, With<Focus>>,
    camera: Query<(&GlobalTransform, &Projection), (With<Camera3d>, Without<crate::globe::GlobeCamera>)>,
) {
    if !mgr.template_resolved {
        return;
    }
    let now = time.elapsed_secs();
    if now - mgr.last_select < 0.25 {
        return;
    }
    mgr.last_select = now;
    let Some(ft) = focus.iter().next() else { return };
    let cam = origin.to_geodetic(ft.translation());
    let params = LodParams {
        // Zoom out the root set when high up so the horizon stays covered.
        root_zoom: if cam.alt_m > 200_000.0 { 3 } else if cam.alt_m > 40_000.0 { 5 } else { 6 },
        ..default()
    };
    let view = camera.single().ok();
    let desired: HashSet<TileId> = lod::select(cam, mgr.ground_estimate, &params)
        .into_iter()
        .filter(|tile| view.is_none_or(|(transform, projection)| tile_intersects_view(*tile, &origin, transform, projection)))
        .collect();
    // Nearest first.
    let mut missing: Vec<TileId> =
        desired.iter().filter(|t| !mgr.loaded.contains_key(t) && !mgr.pending.contains(t)).copied().collect();
    missing.sort_by(|a, b| {
        let da = cam.distance_to(a.center()) / a.width_m();
        let db = cam.distance_to(b.center()) / b.width_m();
        da.total_cmp(&db)
    });
    let pool = AsyncComputeTaskPool::get();
    for tile in missing {
        if mgr.pending.len() >= MAX_IN_FLIGHT {
            break;
        }
        mgr.pending.insert(tile);
        let job = TileJob { tile, frame: origin.frame, cache: cfg.cache.clone(), vector_template: mgr.template.clone() };
        let tx = mgr.tx.clone();
        pool.spawn(async move {
            let _ = tx.send(ready_tile(tilegen::build(&job)));
        })
        .detach();
    }
    mgr.desired = desired;
}

/// Keep tiles whose conservative bounds intersect the camera frustum.
/// The bounds include extra height for buildings; nearby tiles stay loaded
/// while the camera looks away from the ground.
fn tile_intersects_view(tile: TileId, origin: &WorldOrigin, camera: &GlobalTransform, projection: &Projection) -> bool {
    let Projection::Perspective(p) = projection else { return true };
    let center_geo = tile.center();
    let center = origin.to_render(ed_geo::Geodetic { alt_m: 0.0, ..center_geo });
    let to_center = center - camera.translation();
    let radius = tile.width_m() as f32 * std::f32::consts::FRAC_1_SQRT_2 + 500.0;
    // Never cull tiles around the camera: their colliders must survive looking away.
    if to_center.length() < tile.width_m() as f32 * 1.5 + 500.0 {
        return true;
    }
    let depth = to_center.dot(camera.forward().as_vec3());
    if depth + radius < 0.0 {
        return false;
    }

    let half_depth = depth.max(0.0) + radius;
    let tan_half_fov = (p.fov * 0.5).tan();
    let horizontal = to_center.dot(camera.right().as_vec3()).abs();
    let vertical = to_center.dot(camera.up().as_vec3()).abs();
    (horizontal <= half_depth * tan_half_fov * p.aspect_ratio + radius
        && vertical <= half_depth * tan_half_fov + radius)
        || origin.to_geodetic(camera.translation()).distance_to(center_geo) < 1_500.0
}

#[allow(clippy::too_many_arguments)]
fn receive_tiles(
    mut commands: Commands,
    mut mgr: ResMut<TileManager>,
    mut dm: ResMut<DetailManager>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut meshlet_assets: ResMut<Assets<MeshletMesh>>,
    mats: Res<WorldMaterials>,
    bmats: Res<BuildingMaterials>,
    focus: Query<&GlobalTransform, With<Focus>>,
    origin: Res<WorldOrigin>,
    foliage: Res<crate::foliage::FoliageAssets>,
    mut terrain_mats: ResMut<Assets<crate::ground_material::TerrainMaterial>>,
    mut images: ResMut<Assets<Image>>,
    library: Option<Res<crate::library::CookedLibrary>>,
) {
    let focus_pos = focus.iter().next().map(|t| t.translation());
    // Bound per-frame upload work.
    for _ in 0..2 {
        let Ok(ReadyTile { build: mut b, collider, parts, meshlets, lod }) = mgr.rx.try_recv() else { break };
        mgr.pending.remove(&b.tile);
        for e in &b.errors {
            debug!("tile {:?}: {e}", b.tile);
        }
        if !mgr.desired.contains(&b.tile) {
            continue; // stale by the time it arrived
        }
        // Ground height estimate under the player for LOD (finest tile wins).
        if let Some(fp) = focus_pos {
            let g = origin.to_geodetic(fp);
            let (u, v) = b.tile.geodetic_to_uv(g.lat_deg, g.lon_deg);
            if (0.0..1.0).contains(&u) && (0.0..1.0).contains(&v) && b.tile.z >= 12 {
                mgr.ground_estimate = ((b.min_max_height.0 + b.min_max_height.1) * 0.5) as f64;
            }
        }
        let anchor = GeoAnchored::from_render(&b.frame, b.anchor);
        let mut parent = commands.spawn((
            WorldTile,
            anchor,
            Transform::default(),
            Visibility::default(),
            Name::new(format!("tile {}", b.tile.path())),
        ));
        if let Some(collider) = collider {
            parent.insert((RigidBody::Static, collider));
        }
        if let Some(g) = b.ground.take() {
            parent.insert(crate::ground_scatter::GroundCover(g));
        }
        let parent_id = parent.id();
        let near = b.tile.z >= 13;
        for (part, mesh) in parts {
            if matches!(part, Part::Terrain) {
                // Per-tile material: same ground splat, plus this tile's satellite texture.
                let tile_mat = match (b.sat_tex.take(), terrain_mats.get(&mats.terrain).cloned()) {
                    (Some((n, data)), Some(mut m)) => {
                        if let Some(dir) = std::env::var_os("EW_DUMP_SAT") {
                            let _ = std::fs::create_dir_all(&dir);
                            let mut ppm = format!("P6\n{n} {n}\n255\n").into_bytes();
                            for px in data.chunks_exact(4) {
                                ppm.extend_from_slice(&px[..3]);
                            }
                            let _ = std::fs::write(std::path::Path::new(&dir).join(format!("{}_{}_{}.ppm", b.tile.z, b.tile.x, b.tile.y)), ppm);
                        }
                        m.extension.sat_tex = images.add(crate::ground_material::sat_image(n, data));
                        terrain_mats.add(m)
                    }
                    _ => mats.terrain.clone(),
                };
                let mut e = commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(tile_mat), Transform::default()));
                if !near {
                    e.insert(bevy::light::NotShadowCaster);
                }
                e.insert(ChildOf(parent_id));
                continue;
            }
            let (mat, shadows) = match part {
                Part::Terrain => unreachable!(),
                Part::Water => (&mats.water, false),
                Part::Roads => (&mats.matte, false),
                Part::Rocks | Part::Props => (&mats.matte, true),
            };
            let mut e = commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(mat.clone()), Transform::default()));
            if !shadows {
                e.insert(bevy::light::NotShadowCaster);
            }
            e.insert(ChildOf(parent_id));
        }
        if let Some(lib) = &library {
            crate::library::spawn_placements(&mut commands, lib, &mats, parent_id, &b.placements);
        }
        for (m, off) in meshlets {
            commands.spawn((MeshletMesh3d(meshlet_assets.add(m)), MeshMaterial3d(mats.palette_matte.clone()), Transform::from_translation(off), ChildOf(parent_id)));
        }
        // Distant buildings, one entity per detail chunk, with physics shells.
        let mut lod_entities = HashMap::new();
        for (chunk, mut batch) in lod {
            let e = commands.spawn((Transform::default(), Visibility::default(), ChildOf(parent_id))).id();
            if let Some(col) = batch.collider.take() {
                commands.spawn((RigidBody::Static, col, Transform::default(), ChildOf(parent_id)));
            }
            spawn_buckets(&mut commands, &mut meshes, &bmats, batch, e);
            lod_entities.insert(chunk, e);
        }
        if let Some(set) = &b.building_detail {
            dm.tiles.insert(b.tile, TileDetail { set: set.clone(), root: parent_id, lod: lod_entities, loaded: HashMap::new(), pending: HashSet::new() });
        }
        crate::foliage::spawn_tile_trees(&mut commands, &mut meshes, &foliage, parent_id, &b.trees);
        if std::env::var_os("EW_TILE_STATS").is_some() {
            eprintln!("TILESTAT z{} terrain={} water={} roads={} trees={} rocks={} street={} bldg_lod={} trees_n={}", b.tile.z, b.terrain.triangle_count(), b.water.triangle_count(), b.roads.triangle_count(), b.trees.triangle_count(), b.rocks.triangle_count(), b.street_props.triangle_count(), b.building_lod.iter().map(|(_, m)| m.triangle_count()).sum::<usize>(), b.stats.trees);
        }
        mgr.tile_totals.insert(b.tile, Totals { buildings: b.stats.buildings, trees: b.stats.trees, triangles: b.stats.triangles });
        mgr.recount();
        mgr.loaded.insert(b.tile, parent_id);
    }
}

fn is_ancestor(a: TileId, of: TileId) -> bool {
    a.z < of.z && of.ancestor(a.z).0 == a
}

/// Despawn tiles we no longer want, once their replacements are all in.
fn retire_tiles(mut commands: Commands, mut mgr: ResMut<TileManager>, mut dm: ResMut<DetailManager>) {
    let mut retire = Vec::new();
    for (&t, &e) in &mgr.loaded {
        if mgr.desired.contains(&t) {
            continue;
        }
        let covering: Vec<&TileId> = mgr.desired.iter().filter(|d| is_ancestor(t, **d) || is_ancestor(**d, t)).collect();
        if covering.iter().all(|d| mgr.loaded.contains_key(d)) {
            retire.push((t, e));
        }
    }
    let retired = !retire.is_empty();
    for (t, e) in retire {
        commands.entity(e).despawn();
        mgr.loaded.remove(&t);
        mgr.tile_totals.remove(&t);
        dm.tiles.remove(&t);
    }
    if retired {
        mgr.recount();
    }
}

/// Build full-detail building chunks near the focus; drop them when far.
fn stream_detail(
    time: Res<Time>,
    mut commands: Commands,
    mut dm: ResMut<DetailManager>,
    focus: Query<&GlobalTransform, With<Focus>>,
    tiles: Query<&GlobalTransform, With<WorldTile>>,
    mut vis: Query<&mut Visibility>,
) {
    let now = time.elapsed_secs();
    if now - dm.last < 0.3 {
        return;
    }
    dm.last = now;
    let Some(fp) = focus.iter().next().map(|t| t.translation()) else { return };
    let pool = AsyncComputeTaskPool::get();
    let mut want: Vec<(f32, TileId, (u8, u8))> = Vec::new();
    let mut drop: Vec<(TileId, (u8, u8))> = Vec::new();
    for (tid, td) in &dm.tiles {
        let Ok(gt) = tiles.get(td.root) else { continue };
        for chunk in td.set.chunks.keys() {
            let c = gt.transform_point(td.set.chunk_center(*chunk));
            let d = c.distance(fp);
            if d < DETAIL_IN_M && !td.loaded.contains_key(chunk) && !td.pending.contains(chunk) {
                want.push((d, *tid, *chunk));
            } else if d > DETAIL_OUT_M && td.loaded.contains_key(chunk) {
                drop.push((*tid, *chunk));
            }
        }
    }
    for (tid, chunk) in drop {
        if let Some(td) = dm.tiles.get_mut(&tid) {
            if let Some(e) = td.loaded.remove(&chunk) {
                commands.entity(e).despawn();
            }
            if let Some(l) = td.lod.get(&chunk).and_then(|e| vis.get_mut(*e).ok()) {
                *l.into_inner() = Visibility::Inherited;
            }
        }
    }
    want.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (_, tid, chunk) in want {
        if dm.in_flight >= DETAIL_IN_FLIGHT {
            break;
        }
        let tx = dm.tx.clone();
        let Some(td) = dm.tiles.get_mut(&tid) else { continue };
        td.pending.insert(chunk);
        let set = td.set.clone();
        dm.in_flight += 1;
        pool.spawn(async move {
            let _ = tx.send((tid, chunk, prepare_batch(&set.build_chunk(chunk), false)));
        })
        .detach();
    }
}

fn receive_detail(
    mut commands: Commands,
    mut dm: ResMut<DetailManager>,
    mut meshes: ResMut<Assets<Mesh>>,
    bmats: Res<BuildingMaterials>,
    mut vis: Query<&mut Visibility>,
) {
    let Ok((tid, chunk, bmesh)) = dm.rx.try_recv() else { return };
    dm.in_flight = dm.in_flight.saturating_sub(1);
    let Some(td) = dm.tiles.get_mut(&tid) else { return };
    td.pending.remove(&chunk);
    let e = commands.spawn((Transform::default(), Visibility::default(), ChildOf(td.root))).id();
    spawn_buckets(&mut commands, &mut meshes, &bmats, bmesh, e);
    td.loaded.insert(chunk, e);
    if let Some(l) = td.lod.get(&chunk).and_then(|e| vis.get_mut(*e).ok()) {
        *l.into_inner() = Visibility::Hidden;
    }
}

/// Number of full-detail chunks currently shown (for the HUD / capture).
pub fn detail_stats(dm: &DetailManager) -> (usize, usize) {
    let loaded = dm.tiles.values().map(|t| t.loaded.len()).sum();
    (loaded, dm.in_flight)
}
