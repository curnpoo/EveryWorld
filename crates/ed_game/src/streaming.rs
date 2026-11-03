//! Terrain/world tile streaming. Picks the tile set from the quadtree LOD,
//! builds missing tiles on worker threads, and swaps them in without holes
//! (a coarse tile stays until every finer tile covering it has arrived).

use crate::assets::WorldMaterials;
use crate::config::Config;
use crate::meshconv::to_mesh;
use crate::origin::{Focus, GeoAnchored, WorldOrigin};
use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::tasks::AsyncComputeTaskPool;
use crossbeam_channel::{Receiver, Sender};
use ed_geo::tile::TileId;
use ed_world::lod::{self, LodParams};
use ed_world::tilegen::{self, TileBuild, TileJob};
use std::collections::{HashMap, HashSet};

pub struct StreamingPlugin;

impl Plugin for StreamingPlugin {
    fn build(&self, app: &mut App) {
        let (tx, rx) = crossbeam_channel::unbounded();
        app.insert_resource(TileManager { tx, rx, ..default_manager() })
            .add_systems(Startup, resolve_vector_template)
            .add_systems(Update, (select_tiles, receive_tiles, retire_tiles).chain());
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

fn receive_tiles(
    mut commands: Commands,
    mut mgr: ResMut<TileManager>,
    mut meshes: ResMut<Assets<Mesh>>,
    mats: Res<WorldMaterials>,
    focus: Query<&GlobalTransform, With<Focus>>,
    origin: Res<WorldOrigin>,
) {
    let focus_pos = focus.iter().next().map(|t| t.translation());
    // Bound per-frame upload work.
    for _ in 0..3 {
        let Ok(b) = mgr.rx.try_recv() else { break };
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
        let parent_id = parent.id();
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
        let near = b.tile.z >= 13;
        child(&b.terrain, &mats.terrain, near);
        child(&b.water, &mats.water, false);
        child(&b.roads, &mats.matte, false);
        child(&b.foliage, &mats.matte, true);
        child(&b.rocks, &mats.matte, true);
        child(&b.street_props, &mats.matte, true);
        if let Some(bm) = &b.buildings {
            child(&bm.facades, &mats.facade, true);
            child(&bm.glass, &mats.glass, true);
            child(&bm.storefronts, &mats.storefront, true);
            child(&bm.roofs, &mats.matte, true);
            let col = bm.collider();
            if !col.is_empty() {
                let verts: Vec<Vec3> = col.positions.iter().map(|p| Vec3::from(*p)).collect();
                let tris: Vec<[u32; 3]> = col.indices.chunks_exact(3).map(|t| [t[0], t[1], t[2]]).collect();
                commands.spawn((RigidBody::Static, Collider::trimesh(verts, tris), Transform::default(), ChildOf(parent_id)));
            }
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
fn retire_tiles(mut commands: Commands, mut mgr: ResMut<TileManager>) {
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
    }
}
