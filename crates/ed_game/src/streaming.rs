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
use ed_world::lod::{self, LodParams};
use ed_world::buildings::BuildingMeshes;
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
            .add_systems(Update, (select_tiles, receive_tiles, retire_tiles, stream_detail, receive_detail).chain());
    }
}

const MAX_IN_FLIGHT: usize = 12;

#[derive(Resource)]
pub struct TileManager {
    tx: Sender<TileBuild>,
    rx: Receiver<TileBuild>,
    template: Option<String>,
    template_resolved: bool,
    pub loaded: HashMap<TileId, Entity>,
    pub pending: HashSet<TileId>,
    pub desired: HashSet<TileId>,
    pub ground_estimate: f64,
    last_select: f32,
    pub totals: Totals,
}

#[derive(Default, Debug, Clone, Copy)]
pub struct Totals {
    pub buildings: usize,
    pub trees: usize,
    pub triangles: usize,
}

impl TileManager {
    /// Everything requested has arrived.
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
    }
}

#[derive(Component)]
pub struct WorldTile(pub TileId);

/// Full-detail building chunks near the player.
#[derive(Resource)]
pub struct DetailManager {
    tx: Sender<(TileId, (u8, u8), BuildingMeshes)>,
    rx: Receiver<(TileId, (u8, u8), BuildingMeshes)>,
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
fn spawn_buckets(commands: &mut Commands, meshes: &mut Assets<Mesh>, bm: &BuildingMaterials, b: &BuildingMeshes, parent: Entity) {
    for (mat, mesh) in &b.out.meshes {
        if mesh.is_empty() {
            continue;
        }
        commands.spawn((Mesh3d(meshes.add(to_mesh(mesh))), MeshMaterial3d(bm.get(*mat)), Transform::default(), ChildOf(parent)));
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
    let desired: HashSet<TileId> = lod::select(cam, mgr.ground_estimate, &params).into_iter().collect();
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
            let _ = tx.send(tilegen::build(&job));
        })
        .detach();
    }
    mgr.desired = desired;
}

#[allow(clippy::too_many_arguments)]
fn receive_tiles(
    mut commands: Commands,
    mut mgr: ResMut<TileManager>,
    mut dm: ResMut<DetailManager>,
    mut meshes: ResMut<Assets<Mesh>>,
    mats: Res<WorldMaterials>,
    bmats: Res<BuildingMaterials>,
    focus: Query<&GlobalTransform, With<Focus>>,
    origin: Res<WorldOrigin>,
) {
    let focus_pos = focus.iter().next().map(|t| t.translation());
    // Bound per-frame upload work.
    for _ in 0..3 {
        let Ok(mut b) = mgr.rx.try_recv() else { break };
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
            WorldTile(b.tile),
            anchor,
            Transform::default(),
            Visibility::default(),
            Name::new(format!("tile {}", b.tile.path())),
        ));
        if let Some((verts, tris)) = b.collider {
            parent.insert((RigidBody::Static, Collider::trimesh(verts, tris)));
        }
        if let Some(g) = b.ground.take() {
            parent.insert(crate::ground_scatter::GroundCover(g));
        }
        let parent_id = parent.id();
        let near = b.tile.z >= 13;
        if !b.terrain.is_empty() {
            let mut t = commands.spawn((Mesh3d(meshes.add(to_mesh(&b.terrain))), MeshMaterial3d(mats.terrain.clone()), Transform::default(), ChildOf(parent_id)));
            if !near {
                t.insert(bevy::light::NotShadowCaster);
            }
        }
        let mut child = |mesh: &ed_world::mesh::MeshData, mat: &Handle<StandardMaterial>, shadows: bool| {
            if mesh.is_empty() {
                return;
            }
            let mut e = commands.spawn((Mesh3d(meshes.add(to_mesh(mesh))), MeshMaterial3d(mat.clone()), Transform::default()));
            if !shadows {
                e.insert(bevy::light::NotShadowCaster);
            }
            e.insert(ChildOf(parent_id));
        };
        child(&b.water, &mats.water, false);
        child(&b.roads, &mats.matte, false);
        child(&b.foliage, &mats.matte, true);
        child(&b.rocks, &mats.matte, true);
        child(&b.street_props, &mats.matte, true);
        // Distant buildings, one entity per detail chunk, with physics shells.
        let mut lod_entities = HashMap::new();
        for (chunk, bmesh) in &b.building_lod {
            let e = commands.spawn((Transform::default(), Visibility::default(), ChildOf(parent_id))).id();
            spawn_buckets(&mut commands, &mut meshes, &bmats, bmesh, e);
            if !bmesh.collider.is_empty() {
                let col = &bmesh.collider;
                let verts: Vec<Vec3> = col.positions.iter().map(|p| Vec3::from(*p)).collect();
                let tris: Vec<[u32; 3]> = col.indices.chunks_exact(3).map(|t| [t[0], t[1], t[2]]).collect();
                commands.spawn((RigidBody::Static, Collider::trimesh(verts, tris), Transform::default(), ChildOf(parent_id)));
            }
            lod_entities.insert(*chunk, e);
        }
        if let Some(set) = &b.building_detail {
            dm.tiles.insert(b.tile, TileDetail { set: set.clone(), root: parent_id, lod: lod_entities, loaded: HashMap::new(), pending: HashSet::new() });
        }
        mgr.totals.buildings += b.stats.buildings;
        mgr.totals.trees += b.stats.trees;
        mgr.totals.triangles += b.stats.triangles;
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
    for (t, e) in retire {
        commands.entity(e).despawn();
        mgr.loaded.remove(&t);
        dm.tiles.remove(&t);
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
            let _ = tx.send((tid, chunk, set.build_chunk(chunk)));
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
    spawn_buckets(&mut commands, &mut meshes, &bmats, &bmesh, e);
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
