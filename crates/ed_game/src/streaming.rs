//! Terrain/world tile streaming. Picks the tile set from the quadtree LOD,
//! builds missing tiles on worker pools, and swaps them in without holes
//! (a coarse tile stays until every finer tile covering it has arrived).
//!
//! Pipeline per tile (decisions live in the engine-free `ed_runtime`):
//! 1. **select** (4 Hz): `lod::select` gives the *wanted* set; the view test
//!    (real tile heights, `ed_runtime::view`) gives the *in-view* subset.
//!    Visible tiles are queued first, ranked by distance and direction of
//!    travel; stale queued jobs are cancelled.
//! 2. **IO pool**: download source tiles into the disk cache
//!    (`tilegen::prefetch`), so network waits never hold a CPU worker.
//! 3. **CPU pool**: generate the tile (`tilegen::build`) and convert meshes
//!    and colliders off the main thread (`ready_tile`).
//! 4. **apply**: spawn the tile in stages under a per-frame budget
//!    (`ed_runtime::budget`), hidden until complete so the coarse tile keeps
//!    covering the area meanwhile.
//! 5. **retire**: hole-free where you can look (`ed_runtime::scheduler`);
//!    loaded tiles that leave the view stay resident up to a cap
//!    (`ed_runtime::residency`), so turning around doesn't rebuild them.

use crate::assets::WorldMaterials;
use crate::building_materials::BuildingMaterials;
use crate::config::Config;
use crate::meshconv::to_mesh;
use crate::origin::{Focus, GeoAnchored, WorldOrigin};
use avian3d::prelude::*;
use bevy::camera::visibility::RenderLayers;
use bevy::mesh::MeshTag;
use bevy::prelude::*;
use bevy::tasks::AsyncComputeTaskPool;
use crossbeam_channel::{Receiver, Sender};
use ed_geo::tile::TileId;
use ed_runtime::budget::FrameBudget;
use ed_runtime::jobs::JobPool;
use ed_runtime::residency::Lru;
use ed_runtime::scheduler;
use ed_runtime::view::{self as rview, TileBounds, ViewCone};
use ed_world::bparts::Mat;
use ed_world::buildings::BuildingMeshes;
use ed_world::lod::{self, LodParams};
use ed_world::tilegen::{self, DetailSet, TileBuild, TileJob};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

pub struct StreamingPlugin;

impl Plugin for StreamingPlugin {
    fn build(&self, app: &mut App) {
        let (dtx, drx) = crossbeam_channel::unbounded();
        let (etx, erx) = crossbeam_channel::unbounded();
        app.add_plugins(crate::near_chunks::NearChunksPlugin)
            .insert_resource(TileManager::new())
            .insert_resource(DetailManager { tx: dtx, rx: drx, etx, erx, tiles: HashMap::new(), in_flight: 0, last: -1.0 })
            .add_systems(Startup, resolve_vector_template)
            .add_systems(
                Update,
                (
                    // The main menu is just a map: don't load the world until a spawn is chosen.
                    select_tiles.run_if(not(in_state(crate::menu::GameState::Menu))),
                    pump_jobs,
                    apply_tiles,
                    retire_tiles,
                    upgrade_far_buildings,
                    stream_detail.run_if(not(in_state(crate::menu::GameState::Menu))),
                    receive_detail,
                    receive_explode,
                )
                    .chain(),
            );
    }
}

/// Main-thread apply budget per frame, in cost units (about one entity
/// spawn, or ~10k triangles of mesh upload, per unit). `EW_APPLY_BUDGET`
/// overrides it for tuning. The first item of a frame always runs.
const APPLY_UNITS: u32 = 250;
/// Wall-clock cap for the apply loop itself. Spawns are deferred commands, so
/// the units above are the real limiter; this only stops a runaway frame.
const APPLY_MS: u64 = 4;
/// Loaded tiles kept resident while out of view, at most (`EW_RESIDENT_TILES`).
const RESIDENT_CAP: usize = if cfg!(target_os = "windows") { 6 } else { 48 };
/// Off-screen wanted tiles closer than this many tile widths are still built
/// (at low priority): the ground you could turn and look at right away.
const OFFSCREEN_PREBUILD: f64 = 1.5;
/// Height allowance above the ground for buildings and trees in the view test.
const BUILDING_ALLOWANCE_M: f32 = 500.0;
/// Buildings farther than this (m) use the far massing mesh; crossing inward swaps to full geometry.
fn far_lod_m() -> f64 {
    // Off by default: the massing mesh has no spatial clusters, so it is drawn un-culled and
    // measured *worse* than the clustered full geometry at steady state (42 fps with it off vs
    // 24-28 with it on, Shibuya street). Re-enable once `prep` clusters it without the meshopt
    // passes. `EW_FAR_LOD_M=3500` turns it back on.
    env_num("EW_FAR_LOD_M", 0.0)
}

fn env_num<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}


/// Building geometry already converted to Bevy meshes/colliders, so the main
/// thread only has to register assets (trimesh BVH builds are slow).
struct BuildingBatch {
    buckets: Vec<(Mat, Mesh, PreppedMesh)>,
    collider: Option<Collider>,
}

/// Native-renderer prep result computed on the build worker, so the mirror never has to
/// copy the mesh attributes on the main thread. `None` on non-macOS or when native is off.
#[cfg(target_os = "macos")]
type PreppedMesh = Option<ed_metal::world::MeshPrep>;
#[cfg(not(target_os = "macos"))]
type PreppedMesh = Option<()>;

#[cfg(target_os = "macos")]
fn prep_of(m: &ed_world::mesh::MeshData) -> PreppedMesh {
    crate::native::enabled().then(|| ed_metal::world::prep(&m.positions, &m.normals, &m.uvs, &m.colors, &m.indices))
}
#[cfg(not(target_os = "macos"))]
fn prep_of(_: &ed_world::mesh::MeshData) -> PreppedMesh {
    None
}

/// Far massing meshes are already LOD: pack only, no meshopt passes.
#[cfg(target_os = "macos")]
fn prep_of_flat(m: &ed_world::mesh::MeshData) -> PreppedMesh {
    crate::native::enabled().then(|| ed_metal::world::prep_flat(&m.positions, &m.normals, &m.uvs, &m.colors, &m.indices))
}
#[cfg(not(target_os = "macos"))]
fn prep_of_flat(_: &ed_world::mesh::MeshData) -> PreppedMesh {
    None
}

fn prepare_batch(b: &BuildingMeshes, with_collider: bool) -> BuildingBatch {
    let buckets = b.out.meshes.iter().filter(|(_, m)| !m.is_empty()).map(|(mat, m)| (*mat, to_mesh(m), prep_of(m))).collect();
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

#[derive(Clone, Copy, Debug)]
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
    /// Bridge decks and tunnel tubes (trimesh), spawned with the buildings' physics shell.
    road_collider: Option<Collider>,
    /// Shadow stand-in for a subdivided terrain mesh: the bare grid triangles
    /// (same surface, ~1/9 the triangles; see [`Shadow::CameraOnly`]).
    terrain_shadow: Option<Mesh>,
    parts: Vec<(Part, Mesh, PreppedMesh)>,
    /// Opaque static meshes converted to meshlets (virtual geometry) off-thread.
    meshlets: Vec<(MeshletMesh, Vec3)>,
    /// Rock chunks (fine tiles): (centre, mesh), entities only near the camera.
    rock_chunks: Vec<(Vec3, Mesh)>,
    /// Library placements merged into one mesh + collider (meshlets off).
    placements: Option<crate::library::MergedPlacements>,
    /// Building LOD of the whole tile, merged per material, one collider.
    lod: BuildingBatch,
    /// Per-chunk LOD source for tiles with near detail (see [`Explode`]).
    lod_chunks: Option<Arc<Vec<((u8, u8), BuildingMeshes)>>>,
    /// Trees: impostor quads and card chunks, converted on the worker.
    trees: crate::foliage::PreparedTrees,
    /// Built with the far massing mesh (full buckets are held in `TileManager::deferred`).
    far: bool,
    far_mesh: Option<Mesh>,
    far_prep: PreppedMesh,
}

/// A far tile's full building buckets, waiting for the camera to come within `far_lod_m()`.
struct DeferredBuildings {
    buckets: Vec<(Mat, Mesh, PreppedMesh)>,
    root: Entity,
    far_entity: Entity,
}

/// Merge every chunk's LOD buildings into one mesh per material and one
/// physics shell for the tile.
fn merge_lod(chunks: &[((u8, u8), BuildingMeshes)], with_prep: bool) -> BuildingBatch {
    let mut by_mat: HashMap<Mat, ed_world::mesh::MeshData> = HashMap::new();
    let mut col = ed_world::mesh::MeshData::default();
    for (_, b) in chunks {
        for (mat, m) in &b.out.meshes {
            if !m.is_empty() {
                by_mat.entry(*mat).or_default().append(m);
            }
        }
        let base = col.positions.len() as u32;
        col.positions.extend_from_slice(&b.collider.positions);
        col.indices.extend(b.collider.indices.iter().map(|i| i + base));
    }
    let mut buckets: Vec<(Mat, Mesh, PreppedMesh)> = by_mat.into_iter().map(|(mat, m)| (mat, to_mesh(&m), if with_prep { prep_of(&m) } else { None })).collect();
    buckets.sort_by_key(|(m, _, _)| format!("{m:?}"));
    let collider = (!col.indices.is_empty()).then(|| {
        let verts: Vec<Vec3> = col.positions.iter().map(|p| Vec3::from(*p)).collect();
        let tris: Vec<[u32; 3]> = col.indices.chunks_exact(3).map(|t| [t[0], t[1], t[2]]).collect();
        Collider::trimesh(verts, tris)
    });
    BuildingBatch { buckets, collider }
}

fn ready_tile(mut b: TileBuild, far: bool) -> ReadyTile {
    // Print before any field is taken: the apply path drains parts, trees and rock chunks, so a
    // later print shows zeros for them (that is why the canyon total looked unaccounted).
    if std::env::var_os("EW_TILE_STATS").is_some() {
        let rock_chunks: usize = b.rock_chunks.iter().map(|m| m.triangle_count()).sum();
        eprintln!(
            "TILESTAT z{} terrain={} water={} roads={} trees={} rocks={} rock_chunks={} street={} bld={} total={} trees_n={} rocks_n={}",
            b.tile.z,
            b.terrain.triangle_count(),
            b.water.triangle_count(),
            b.roads.triangle_count(),
            b.trees.triangle_count(),
            b.rocks.triangle_count(),
            rock_chunks,
            b.street_props.triangle_count(),
            b.building_lod.iter().map(|(_, m)| m.triangle_count()).sum::<usize>(),
            b.stats.triangles,
            b.stats.trees,
            b.stats.rocks
        );
    }
    // The collider is the terrain grid without the render subdivision, so it
    // doubles as the terrain's shadow caster (the sun never needs the splat vertices).
    let terrain_shadow = b.collider.as_ref().filter(|(v, _)| v.len() * 4 < b.terrain.positions.len()).map(|(verts, tris)| {
        let mut m = Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, crate::native::world_mesh_usage());
        m.insert_attribute(Mesh::ATTRIBUTE_POSITION, verts.clone());
        m.insert_indices(bevy::mesh::Indices::U32(tris.iter().flatten().copied().collect()));
        m
    });
    let collider = b.collider.take().map(|(verts, tris)| Collider::trimesh(verts, tris));
    let road_collider = b.road_collider.take().map(|(verts, tris)| Collider::trimesh(verts, tris));
    let parts = [
        (Part::Terrain, &b.terrain),
        (Part::Water, &b.water),
        (Part::Roads, &b.roads),
        (Part::Rocks, &b.rocks),
        (Part::Props, &b.street_props),
    ]
    .into_iter()
    .filter(|(_, m)| !m.is_empty())
    .map(|(p, m)| (p, to_mesh(m), prep_of(m)))
    .collect::<Vec<_>>();
    // Ordinary meshes by default; `EW_MESHLET=1` draws the same chunks as meshlets (A/B).
    let (meshlets, rock_chunks, placements) = if !crate::library::meshlets_enabled() {
        let rocks = b.rock_chunks.iter().filter(|m| !m.is_empty()).map(|m| (crate::near_chunks::center_of(&m.positions), to_mesh(m))).collect();
        (Vec::new(), rocks, Some(crate::library::merge_placements(&b.placements)))
    } else {
        (b.rock_chunks.iter().filter_map(|m| to_meshlet(m)).collect(), Vec::new(), None)
    };
    let chunks = std::mem::take(&mut b.building_lod);
    let lod = merge_lod(&chunks, !far);
    let far_mesh = (far && !b.building_far.is_empty()).then(|| to_mesh(&b.building_far));
    let far_prep = if far_mesh.is_some() { prep_of_flat(&b.building_far) } else { None };
    if std::env::var_os("EW_FAR_DBG").is_some() {
        info!(
            "tile {:?} far={far} massing={} tris full={} tris",
            b.tile,
            b.building_far.triangle_count(),
            chunks.iter().map(|(_, m)| m.triangle_count()).sum::<usize>()
        );
    }
    let lod_chunks = (b.building_detail.is_some() && !chunks.is_empty()).then(|| Arc::new(chunks));
    let trees = crate::foliage::prepare_trees(std::mem::take(&mut b.trees));
    ReadyTile { build: b, collider, road_collider, terrain_shadow, parts, meshlets, rock_chunks, placements, lod, lod_chunks, trees, far, far_mesh, far_prep }
}

/// Road graph of a tile (AI, racing, traffic): nodes, lanes, speeds, in the tile's render space.
#[derive(Component)]
#[allow(dead_code)] // consumed by the traffic / AI systems to come
pub struct TileNav(pub Arc<ed_world::roads::nav::NavTile>);

#[derive(Resource)]
pub struct TileManager {
    /// Build-worker native prep results, drained by the Metal mirror (no main-thread copy).
    prep_tx: Sender<(AssetId<Mesh>, PreppedMesh)>,
    prep_rx: Receiver<(AssetId<Mesh>, PreppedMesh)>,
    /// Network: warms the disk cache for a tile's sources.
    io: JobPool<TileId, ()>,
    /// CPU: generates the tile and prepares Bevy meshes/colliders.
    cpu: JobPool<TileId, ReadyTile>,
    /// Job parameters for tiles between the IO and CPU stages.
    jobs: HashMap<TileId, TileJob>,
    /// Built tiles being spawned, a stage at a time.
    applying: VecDeque<Applying>,
    template: Option<String>,
    template_resolved: bool,
    pub loaded: HashMap<TileId, Entity>,
    /// Anywhere in the pipeline: IO, CPU or apply.
    pub pending: HashSet<TileId>,
    /// Wanted and in view: what must be loaded for the frame to be complete.
    pub desired: HashSet<TileId>,
    /// The full LOD selection, in every direction.
    wanted: HashSet<TileId>,
    in_view: HashSet<TileId>,
    /// Ground height range of loaded tiles (for the view test of their
    /// descendants and themselves).
    heights: HashMap<TileId, (f32, f32)>,
    /// Share of each loaded tile's ground at or below sea level.
    sea_frac: HashMap<TileId, f32>,
    /// The z14 tile the focus is in. Used to ask whether the sea is near the camera.
    pub focus_tile: Option<TileId>,
    /// Full building buckets for tiles built as far LOD, keyed by tile.
    deferred: HashMap<TileId, DeferredBuildings>,
    /// Distance decision made when the tile was queued (used by the build worker).
    far_hint: HashMap<TileId, bool>,
    /// Satellite pool slot per loaded/applying tile (`MeshTag` value).
    sat_tags: HashMap<TileId, u32>,
    /// When each loaded tile was last in view.
    seen: Lru<TileId>,
    resident_cap: usize,
    pub ground_estimate: f64,
    last_select: f32,
    /// Origin generation last seen; a jump means old tiles are off-screen and can retire at once.
    last_generation: u64,
    jumped: bool,
    /// Camera rotation last select tick (adaptive build concurrency while turning).
    last_cam_rot: Option<Quat>,
    last_focus: Option<(Vec3, f32, u64)>,
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
    fn new() -> Self {
        let (io_n, cpu_n) = ed_runtime::jobs::default_threads();
        let io_n = env_num("EW_IO_THREADS", io_n);
        let cpu_n = env_num("EW_TILE_THREADS", cpu_n);
        let (prep_tx, prep_rx) = crossbeam_channel::unbounded();
        info!("tile streaming: {io_n} IO threads, {cpu_n} build threads");
        Self {
            prep_tx,
            prep_rx,
            io: JobPool::new("tile-io", io_n),
            cpu: JobPool::new("tile-build", cpu_n),
            jobs: HashMap::new(),
            applying: VecDeque::new(),
            template: None,
            template_resolved: false,
            loaded: HashMap::new(),
            pending: HashSet::new(),
            desired: HashSet::new(),
            wanted: HashSet::new(),
            in_view: HashSet::new(),
            heights: HashMap::new(),
            sea_frac: HashMap::new(),
            focus_tile: None,
            deferred: HashMap::new(),
            far_hint: HashMap::new(),
            sat_tags: HashMap::new(),
            seen: Lru::default(),
            resident_cap: if std::env::var_os("EW_KEEP_TILES").is_some() { usize::MAX } else { env_num("EW_RESIDENT_TILES", RESIDENT_CAP) },
            ground_estimate: 0.0,
            last_select: -1.0,
            last_generation: 0,
            jumped: false,
            last_cam_rot: None,
            last_focus: None,
            totals: Totals::default(),
            tile_totals: HashMap::new(),
        }
    }

    fn recount(&mut self) {
        self.totals = self.tile_totals.values().fold(Totals::default(), |a, t| Totals {
            buildings: a.buildings + t.buildings,
            trees: a.trees + t.trees,
            triangles: a.triangles + t.triangles,
        });
    }

    /// Everything requested has arrived.
    pub fn idle(&self) -> bool {
        self.template_resolved && self.pending.is_empty() && self.desired.iter().all(|t| self.loaded.contains_key(t))
    }

    /// True when the sea is within a few hundred metres of `center`: one of the loaded tiles at or
    /// next to it has ground at or below sea level. The ocean is drawn everywhere as a global
    /// disc, but the wave detail is only visible close to the camera, so this is what decides
    /// whether the ocean FFT is worth running at all.
    pub fn sea_near(&self) -> bool {
        // The elevation data carries real bathymetry, so a tile that reaches below sea level has
        // the sea in it. A river channel at 0 m does not count, and a coastal tile a few km away
        // does not either: the waves are only visible inside a couple of hundred metres.
        // The ocean itself is clamped to 0 m in the elevation data, so the test is on the area of
        // the tile that is at or below sea level: a tile that is mostly sea turns the waves on, a
        // city tile with a river at 0 m does not.
        let Some(center) = self.focus_tile else { return false };
        self.sea_frac.get(&center).copied().unwrap_or(0.0) > 0.02
    }

    /// Share of the tile the focus is in that is at or below sea level.
    pub fn focus_sea_fraction(&self) -> f32 {
        let Some(center) = self.focus_tile else { return 0.0 };
        self.sea_frac.get(&center).copied().unwrap_or(0.0)
    }

    /// (io queued+running, cpu queued+running, applying) for the HUD/perf log.
    pub fn pipeline(&self) -> (usize, usize, usize) {
        let (iq, ir) = self.io.load();
        let (cq, cr) = self.cpu.load();
        (iq + ir, cq + cr, self.applying.len())
    }

    /// Drain native mesh preps computed by the build workers.
    pub fn drain_preps(&self, mut f: impl FnMut(AssetId<Mesh>, PreppedMesh)) {
        while let Ok((id, p)) = self.prep_rx.try_recv() {
            f(id, p);
        }
    }

    /// Known ground height range for `t`: its own, else its nearest loaded ancestor's.
    fn known_heights(&self, t: TileId) -> Option<(f32, f32)> {
        (0..=t.z).rev().find_map(|z| self.heights.get(&t.ancestor(z).0).copied())
    }
}

#[derive(Component)]
pub struct WorldTile;

/// The bare-grid terrain shadow proxy on the sun-only layer (`CLOUD_SHADOW_LAYER`). The Metal
/// mirror marks it as a terrain caster so the far (terrain-only) cascade keeps drawing it; the
/// Bevy pipeline uses its layer, so this is inert there.
#[derive(Component)]
pub struct TerrainShadowProxy;

/// Full-detail building chunks near the player.
#[derive(Resource)]
pub struct DetailManager {
    tx: Sender<(TileId, (u8, u8), bool, BuildingBatch)>,
    rx: Receiver<(TileId, (u8, u8), bool, BuildingBatch)>,
    /// Per-chunk LOD batches for a tile being exploded.
    etx: Sender<(TileId, Vec<((u8, u8), BuildingBatch)>)>,
    erx: Receiver<(TileId, Vec<((u8, u8), BuildingBatch)>)>,
    tiles: HashMap<TileId, TileDetail>,
    in_flight: usize,
    last: f32,
}

struct TileDetail {
    set: Arc<DetailSet>,
    root: Entity,
    /// The tile's merged LOD buildings (shown while not exploded).
    lod_root: Option<Entity>,
    lod_src: Option<Arc<Vec<((u8, u8), BuildingMeshes)>>>,
    /// Per-chunk LOD entities while exploded.
    lod: HashMap<(u8, u8), Entity>,
    explode: Explode,
    /// Last time this tile had detail loaded, pending or wanted.
    busy_at: f32,
    /// Shown detail chunks and whether each is the mid (`lite`) level.
    loaded: HashMap<(u8, u8), (Entity, bool)>,
    pending: HashSet<(u8, u8)>,
}

/// A tile's LOD buildings are ONE merged mesh per material. Swapping a chunk
/// for full detail needs per-chunk LOD, so the tile "explodes" into chunk
/// entities first (prepared on a worker, spawned a few per frame), and
/// implodes back once no detail has been near for a while. Only the few
/// tiles around the player are ever exploded.
enum Explode {
    Merged,
    Pending,
    Spawning(Vec<((u8, u8), BuildingBatch)>),
    Exploded,
}

/// Exploded tiles with no detail near for this long merge back.
const IMPLODE_AFTER_S: f32 = 8.0;
/// Per-chunk LOD spawns per frame while exploding.
const EXPLODE_CHUNKS_PER_FRAME: usize = 8;

/// Detail is built inside this radius and dropped beyond the outer one.
const DETAIL_IN_M: f32 = 230.0;
const DETAIL_OUT_M: f32 = 320.0;
const DETAIL_IN_FLIGHT: usize = 2;
/// Full kit (mullions, louvres, balusters, flower heads) inside NEAR_IN; the
/// mid level beyond NEAR_OUT, where those parts are under a pixel
/// (~18k -> a few k triangles per building). Hysteresis between the two.
const NEAR_IN_M: f32 = 70.0;
const NEAR_OUT_M: f32 = 100.0;
/// Tiles with a building chunk this close explode into per-chunk LOD so their
/// buildings cast shadows (the far cascade ends at 700 m, `sky.rs`).
/// Bevy renderer only: on native the merged tile LOD casts, so no explode.
const SHADOW_EXPLODE_M: f32 = 750.0;
/// Rock chunks are entities only within this distance of the camera.
const ROCK_RANGE_M: f32 = 500.0;

/// Spawn one entity per material bucket under `parent`.
///
fn spawn_buckets(commands: &mut Commands, meshes: &mut Assets<Mesh>, bm: &BuildingMaterials, b: BuildingBatch, parent: Entity, shadow: Shadow, preps: &Sender<(AssetId<Mesh>, PreppedMesh)>) {
    if hidden_part("lod") {
        return;
    }
    for (mat, mesh, prep) in b.buckets {
        let h = meshes.add(mesh);
        if let Some(p) = prep {
            let _ = preps.send((h.id(), Some(p)));
        }
        let mut e = commands.spawn((Mesh3d(h), MeshMaterial3d(bm.get(mat)), Transform::default(), ChildOf(parent)));
        match shadow {
            Shadow::Cast => {}
            Shadow::None => {
                e.insert(bevy::light::NotShadowCaster);
            }
            Shadow::CameraOnly => {
                e.insert(RenderLayers::layer(crate::camera::CAMERA_ONLY_LAYER));
            }
        }
    }
}

/// How building meshes take part in the sun's shadow cascades.
///
/// Measured at Zermatt (2144x1206, M3 Pro): building shadows were 5-6 ms a frame,
/// almost all of it the full-detail chunks (facades, frames, sills drawn again
/// into both cascades). So detail is camera-only and the per-chunk LOD under it
/// stays alive on the sun-only layer as its shadow stand-in. The merged per-tile
/// LOD casts nothing: its bounds cover the whole tile, so it would be drawn whole
/// into every cascade; per-chunk LOD (tiles within `SHADOW_EXPLODE_M`) casts.
#[derive(Clone, Copy)]
enum Shadow {
    Cast,
    None,
    CameraOnly,
}

/// Switch a per-chunk LOD between normal and shadow-only (detail shown above it).
fn set_shadow_proxy(commands: &mut Commands, children: &Query<&Children>, chunk: Entity, proxy: bool) {
    let Ok(kids) = children.get(chunk) else { return };
    for k in kids.iter() {
        if proxy {
            commands.entity(k).insert(RenderLayers::layer(crate::clouds::CLOUD_SHADOW_LAYER));
        } else {
            commands.entity(k).remove::<RenderLayers>();
        }
    }
}

fn resolve_vector_template(cfg: Res<Config>, mut mgr: ResMut<TileManager>) {
    match ed_data::vector::tile_template(&cfg.cache) {
        Ok(t) => mgr.template = Some(t),
        Err(e) => warn!("vector tiles unavailable ({e}); terrain only"),
    }
    mgr.template_resolved = true;
}

/// Pick the wanted and visible tile sets, cancel stale work, queue new work.
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
    if mgr.last_generation != origin.generation {
        mgr.last_generation = origin.generation;
        mgr.jumped = true;
    }
    let Some(ft) = focus.iter().next() else { return };
    let focus_pos = ft.translation();
    let cam = origin.to_geodetic(focus_pos);
    // Where the focus is, at the finest zoom the world streams. The ocean FFT is only worth
    // running when the sea is near this tile.
    mgr.focus_tile = Some(ed_geo::tile::TileId::containing(cam.lat_deg, cam.lon_deg, 14));

    // Velocity for prefetching ahead (skip the tick a floating-origin rebase jumps).
    let vel = match mgr.last_focus {
        Some((p, t, generation)) if generation == origin.generation && now > t => (focus_pos - p) / (now - t),
        _ => Vec3::ZERO,
    };
    mgr.last_focus = Some((focus_pos, now, origin.generation));
    let ahead_k = scheduler::ahead_weight(vel.length());
    let vel_dir = vel.normalize_or_zero();
    // Adaptive build concurrency: while the camera turns or moves, keep most cores free for the
    // main thread (rotation measured 2.9 fps at 8 build threads, 15.2 at 2; static load still
    // uses all of them).
    let cam_rot = camera.single().ok().map(|(t, _)| t.rotation());
    let turned = mgr.last_cam_rot.zip(cam_rot).is_some_and(|(a, b)| a.angle_between(b) > 0.02);
    mgr.last_cam_rot = cam_rot;
    let moving = turned || vel.length() > 2.0;
    if std::env::var_os("EW_BUILD_LIMIT_DBG").is_some() {
        info!("build limit: turned={turned} vel={:.2} moving={moving}", vel.length());
    }
    mgr.cpu.set_limit(if moving { env_num("EW_BUILD_LIMIT_MOVING", 2) } else { 0 });

    // Zoom out the root set when high up so the horizon stays covered, and cap the
    // finest zoom so altitude doesn't split into hundreds of tiny tiles (which can't
    // all build in time and leave the sea globe showing through). At 100 km this
    // yields a few dozen ~500 km tiles that cover the visible disc almost at once.
    let (root_zoom, max_zoom) = if cam.alt_m > 200_000.0 {
        (2u8, 4u8)
    } else if cam.alt_m > 100_000.0 {
        (3u8, 5u8)
    } else if cam.alt_m > 40_000.0 {
        (4u8, 7u8)
    } else if cam.alt_m > 15_000.0 {
        (5u8, 10u8)
    } else {
        (6u8, 14u8)
    };
    let params = LodParams {
        root_zoom,
        max_zoom,
        fine_range_m: env_num("EW_FINE_RANGE_M", 16_000.0),
        ..default()
    };
    let wanted: HashSet<TileId> = lod::select(cam, mgr.ground_estimate, &params).into_iter().collect();
    let cone = camera.single().ok().and_then(|(t, p)| match p {
        Projection::Perspective(p) => Some(ViewCone {
            pos: t.translation(),
            forward: t.forward().as_vec3(),
            right: t.right().as_vec3(),
            up: t.up().as_vec3(),
            tan_half_fov: (p.fov * 0.5).tan(),
            aspect: p.aspect_ratio,
        }),
        _ => None,
    });
    let ground = mgr.ground_estimate as f32;
    // `EW_NO_VIEW_CONE=1`: treat every wanted tile as in view (A/B for rotation-triggered
    // streaming: everything prebuilds at load, rotating never queues anything new).
    let no_cone = std::env::var_os("EW_NO_VIEW_CONE").is_some();
    let in_view: HashSet<TileId> = if no_cone {
        wanted.clone()
    } else {
        wanted
        .iter()
        .copied()
        .filter(|t| {
            let c = t.center();
            // Never treat the ground around the camera as off-screen: its
            // colliders must survive looking away.
            if cam.distance_to(c) < t.width_m() * 0.75 + 1_500.0 {
                return true;
            }
            let Some(cone) = &cone else { return true };
            let (lo, hi) = rview::height_range(mgr.known_heights(*t), ground, BUILDING_ALLOWANCE_M);
            let b = TileBounds {
                bottom: origin.to_render(ed_geo::Geodetic { alt_m: lo as f64, ..c }),
                top: origin.to_render(ed_geo::Geodetic { alt_m: hi as f64, ..c }),
                half_width: (t.width_m() * 0.5) as f32,
            };
            let (center, radius) = b.sphere();
            cone.sees_sphere(center, radius)
        })
        .collect()
    };

    // Cancel queued work for tiles that are no longer wanted.
    if std::env::var_os("EW_KEEP_TILES").is_none() {
        let mut dropped = Vec::new();
        mgr.io.retain_queued(|t| wanted.contains(t) || { dropped.push(*t); false });
        mgr.cpu.retain_queued(|t| wanted.contains(t) || { dropped.push(*t); false });
        for t in dropped {
            mgr.pending.remove(&t);
            mgr.jobs.remove(&t);
        }
    }

    // Queue (or re-rank) wanted tiles: visible ones, then the ground right around you.
    let priority = |t: &TileId, visible: bool| {
        let d = cam.distance_to(t.center()) / t.width_m();
        let to = origin.to_render(ed_geo::Geodetic { alt_m: cam.alt_m, ..t.center() }) - focus_pos;
        let ahead = ahead_k * vel_dir.dot(to.normalize_or_zero());
        (d, scheduler::build_priority(d, visible, ahead))
    };
    let mut started = 0;
    for t in &wanted {
        if mgr.loaded.contains_key(t) {
            continue;
        }
        let visible = in_view.contains(t);
        let (d, p) = priority(t, visible);
        if mgr.pending.contains(t) {
            mgr.io.set_priority(t, p);
            mgr.cpu.set_priority(t, p);
            continue;
        }
        // Off-screen wanted tiles beyond the prebuild ring wait: at street level `wanted` is
        // ~354 tiles (z14 out to 16 km) while the view holds ~137, so building them all is far
        // too expensive. Rotating into them is a known build storm (see CHANGES 2026-11-06).
        if std::env::var_os("EW_KEEP_TILES").is_none() && !visible && d > OFFSCREEN_PREBUILD {
            continue;
        }
        // 0 (or negative) disables the far massing LOD entirely; it must not mean "every tile is far".
        let far_m = far_lod_m();
        let far_lod = far_m > 0.0 && cam.distance_to(t.center()) > far_m;
        // `EW_CHEAP_FAR=1` also skips trees/props/rocks when building far tiles (A/B switch).
        let cheap_far = far_lod && std::env::var_os("EW_CHEAP_FAR").is_some();
        let job = TileJob { tile: *t, frame: origin.frame, cache: cfg.cache.clone(), vector_template: mgr.template.clone(), far: cheap_far };
        mgr.far_hint.insert(*t, far_lod);
        let io_job = job.clone();
        if mgr.io.submit(*t, p, move || tilegen::prefetch(&io_job)) {
            mgr.jobs.insert(*t, job);
            mgr.pending.insert(*t);
            started += 1;
        }
    }
    if started > 0 {
        debug!("streaming: queued {started} tiles");
    }

    // Residency: loaded tiles in view are fresh; the oldest unseen ones go
    // once over the cap (never ones near the camera, which count as in view).
    mgr.seen.tick();
    let loaded: Vec<TileId> = mgr.loaded.keys().copied().collect();
    for t in loaded.iter().filter(|t| in_view.contains(t)) {
        mgr.seen.touch(t);
    }
    let desired = wanted.intersection(&in_view).copied().collect();
    mgr.wanted = wanted;
    mgr.in_view = in_view;
    mgr.desired = desired;
}

/// Move tiles between pipeline stages: IO done -> CPU queue, CPU done -> apply queue.
fn pump_jobs(mut mgr: ResMut<TileManager>) {
    let mgr = &mut *mgr;
    while let Some((t, _)) = mgr.io.try_recv() {
        let Some(job) = mgr.jobs.remove(&t) else { continue };
        if !mgr.wanted.contains(&t) {
            mgr.pending.remove(&t);
            continue;
        }
        // Visible tiles first; `select_tiles` refines the rank every 0.25 s.
        let p = if mgr.in_view.contains(&t) { 0.0 } else { 1_000.0 };
        let far = mgr.far_hint.remove(&t).unwrap_or(false);
        if !mgr.cpu.submit(t, p, move || ready_tile(tilegen::build(&job), far)) {
            mgr.pending.remove(&t);
        }
    }
    while let Some((t, ready)) = mgr.cpu.try_recv() {
        match ready {
            Some(r) if mgr.wanted.contains(&t) => {
                for e in &r.build.errors {
                    debug!("tile {t:?}: {e}");
                }
                mgr.applying.push_back(Applying::new(r));
            }
            Some(_) => {
                mgr.pending.remove(&t);
            }
            None => {
                warn!("tile {t:?}: build panicked");
                mgr.pending.remove(&t);
            }
        }
    }
}

/// Debug bisect: `EW_HIDE=terrain,roads,rocks,props,water,lod` skips spawning those tile parts.
fn hidden_part(name: &str) -> bool {
    static H: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    H.get_or_init(|| std::env::var("EW_HIDE").map(|v| v.to_lowercase().split(',').map(str::to_string).collect()).unwrap_or_default())
        .iter()
        .any(|h| h == &name.to_lowercase())
}

/// Triangles in a Bevy mesh (for apply cost estimates).
fn tris(m: &Mesh) -> u32 {
    m.indices().map_or(0, |i| i.len() as u32 / 3)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Stage {
    /// Tile entity, terrain collider, terrain and ground-level meshes.
    Base,
    /// Library placements and meshlet chunks.
    Extras,
    /// Merged LOD buildings (one mesh per material) and their physics shell.
    Buildings,
    /// Impostors and the near-camera tree chunk set.
    Trees,
    /// Reveal the tile and hand it to retirement / detail streaming.
    Finish,
}

/// A built tile being spawned over several frames.
struct Applying {
    tile: TileId,
    stage: Stage,
    parent: Option<Entity>,
    ready: ReadyTile,
    lod_root: Option<Entity>,
}

impl Applying {
    fn new(ready: ReadyTile) -> Self {
        Self { tile: ready.build.tile, stage: Stage::Base, parent: None, ready, lod_root: None }
    }
}

/// Spawn built tiles under a per-frame budget. Each tile stays hidden until
/// its last stage, so the coarser tile it replaces keeps covering the area.
#[allow(clippy::too_many_arguments)]
fn apply_tiles(
    mut commands: Commands,
    mut mgr: ResMut<TileManager>,
    mut dm: ResMut<DetailManager>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut meshlet_assets: ResMut<Assets<MeshletMesh>>,
    mats: Res<WorldMaterials>,
    bmats: Res<BuildingMaterials>,
    focus: Query<&GlobalTransform, With<Focus>>,
    origin: Res<WorldOrigin>,
    impostors: Res<crate::impostors::Impostors>,
    mut pools: (ResMut<crate::ground_material::SatPool>, ResMut<crate::far_shadows::FarPool>),
    library: Option<Res<crate::library::CookedLibrary>>,
) {
    let t0 = std::time::Instant::now();
    let (sat_pool, far_pool) = (&mut *pools.0, &mut *pools.1);
    let mut budget = FrameBudget::new(env_num("EW_APPLY_BUDGET", APPLY_UNITS), Some(std::time::Duration::from_millis(APPLY_MS)));
    let focus_pos = focus.iter().next().map(|t| t.translation());
    let mgr = &mut *mgr;
    while !budget.exhausted() {
        let Some(mut a) = mgr.applying.pop_front() else { break };
        let t = a.tile;
        // Went stale mid-apply: undo and drop.
        if !mgr.wanted.contains(&t) {
            if let Some(p) = a.parent {
                commands.entity(p).despawn();
            }
            if let Some(tag) = mgr.sat_tags.remove(&t) {
                sat_pool.release(tag);
                far_pool.release(tag);
            }
            mgr.pending.remove(&t);
            continue;
        }
        let mut blocked = false;
        while !blocked && a.stage != Stage::Finish {
            match a.stage {
                Stage::Base => {
                    let cost = 4 + a.ready.parts.len() as u32 + a.ready.parts.iter().map(|(_, m, _)| tris(m)).sum::<u32>() / 10_000;
                    if !budget.try_spend(cost) {
                        blocked = true;
                        continue;
                    }
                    let b = &mut a.ready.build;
                    let anchor = GeoAnchored::from_render(&b.frame, b.anchor);
                    let mut parent = commands.spawn((WorldTile, anchor, Transform::default(), Visibility::Hidden, Name::new(format!("tile {}", t.path()))));
                    if let Some(collider) = a.ready.collider.take() {
                        parent.insert((RigidBody::Static, collider));
                    }
                    if let Some(n) = b.nav.take() {
                        parent.insert(TileNav(n));
                    }
                    if let Some(g) = b.ground.take() {
                        parent.insert(crate::ground_scatter::GroundCover(g));
                    }
                    if let Some(sm) = b.sound.take() {
                        parent.insert(crate::soundscape::TileSound(sm));
                    }
                    let parent_id = parent.id();
                    a.parent = Some(parent_id);
                    let near = t.z >= 13;
                    let mut sat_tag = match b.sat_tex.take() {
                        Some((n, data)) => {
                            dump_sat(t, n, &data);
                            let detail = b.detail_tex.take().filter(|(dn, _)| *dn == n).map(|(_, d)| d);
                            sat_pool.alloc(n, data, detail)
                        }
                        None => crate::ground_material::SAT_NONE,
                    };
                    // Far-shadow layer + 1 in bits 12-23 of the same tag.
                    sat_tag |= b.far_shadow.take().map_or(0, |f| far_pool.alloc(f));
                    if sat_tag != crate::ground_material::SAT_NONE {
                        mgr.sat_tags.insert(t, sat_tag);
                    }
                    let terrain_proxy = a.ready.terrain_shadow.take();
                    if let Some(m) = terrain_proxy.as_ref() {
                        commands.spawn((Mesh3d(meshes.add(m.clone())), MeshMaterial3d(mats.matte.clone()), Transform::default(), RenderLayers::layer(crate::clouds::CLOUD_SHADOW_LAYER), TerrainShadowProxy, ChildOf(parent_id)));
                    }
                    for (part, mesh, prep) in a.ready.parts.drain(..) {
                        if hidden_part(&format!("{part:?}")) {
                            continue;
                        }
                        let h = meshes.add(mesh);
                        if let Some(p) = prep {
                            let _ = mgr.prep_tx.send((h.id(), Some(p)));
                        }
                        let mut e = match part {
                            // One shared terrain material; the tile's satellite layer rides on its MeshTag.
                            Part::Terrain => commands.spawn((Mesh3d(h), MeshMaterial3d(mats.terrain.clone()), MeshTag::new(sat_tag), Transform::default())),
                            Part::Water => commands.spawn((Mesh3d(h), MeshMaterial3d(mats.water.clone()), Transform::default())),
                            Part::Roads => commands.spawn((Mesh3d(h), MeshMaterial3d(mats.matte.clone()), Transform::default())),
                            Part::Rocks | Part::Props => commands.spawn((Mesh3d(h), MeshMaterial3d(mats.matte.clone()), Transform::default())),
                        };
                        let shadows = match part {
                            Part::Terrain if terrain_proxy.is_some() => {
                                e.insert(RenderLayers::layer(crate::camera::CAMERA_ONLY_LAYER));
                                true
                            }
                            Part::Terrain => near,
                            Part::Water | Part::Roads => false,
                            Part::Rocks | Part::Props => true,
                        };
                        if !shadows {
                            e.insert(bevy::light::NotShadowCaster);
                        }
                        e.insert(ChildOf(parent_id));
                    }
                    let rocks = std::mem::take(&mut a.ready.rock_chunks);
                    if !rocks.is_empty() {
                        commands.spawn((crate::near_chunks::NearMeshes::new(ROCK_RANGE_M, mats.matte.clone(), rocks), Transform::default(), Visibility::default(), ChildOf(parent_id)));
                    }
                    a.stage = Stage::Extras;
                }
                Stage::Extras => {
                    let merged = a.ready.placements.as_ref().map_or(0, |m| m.mesh.as_ref().map_or(0, tris) / 10_000 + 2);
                    let cost = 1 + merged + if a.ready.placements.is_none() { a.ready.build.placements.len() as u32 } else { 0 } + a.ready.meshlets.len() as u32;
                    if !budget.try_spend(cost) {
                        blocked = true;
                        continue;
                    }
                    let parent_id = a.parent.unwrap();
                    match a.ready.placements.take() {
                        Some(m) => crate::library::spawn_merged(&mut commands, &mut meshes, &mats, parent_id, m),
                        None => {
                            if let Some(lib) = &library {
                                crate::library::spawn_placements(&mut commands, lib, &mats, parent_id, &a.ready.build.placements);
                            }
                        }
                    }
                    for (m, off) in a.ready.meshlets.drain(..) {
                        commands.spawn((MeshletMesh3d(meshlet_assets.add(m)), MeshMaterial3d(mats.palette_matte.clone()), Transform::from_translation(off), ChildOf(parent_id)));
                    }
                    a.stage = Stage::Buildings;
                }
                Stage::Buildings => {
                    let batch = &a.ready.lod;
                    let cost = 1 + batch.buckets.len() as u32 + if batch.collider.is_some() { 3 } else { 0 } + batch.buckets.iter().map(|(_, m, _)| tris(m)).sum::<u32>() / 10_000;
                    if !budget.try_spend(cost) {
                        blocked = true;
                        continue;
                    }
                    let mut batch = std::mem::replace(&mut a.ready.lod, BuildingBatch { buckets: Vec::new(), collider: None });
                    let parent_id = a.parent.unwrap();
                    if let Some(col) = a.ready.road_collider.take() {
                        commands.spawn((RigidBody::Static, col, Transform::default(), ChildOf(parent_id)));
                    }
                    if let Some(col) = batch.collider.take() {
                        commands.spawn((RigidBody::Static, col, Transform::default(), ChildOf(parent_id)));
                    }
                    let far_mesh = if a.ready.far { a.ready.far_mesh.take() } else { None };
                    let far_prep = if a.ready.far { a.ready.far_prep.take() } else { None };
                    if let (true, Some(mesh), Some(prep)) = (a.ready.far, far_mesh, far_prep) {
                        // Far tile: draw the massing now and keep the full buckets for the upgrade.
                        let h = meshes.add(mesh);
                        let _ = mgr.prep_tx.send((h.id(), Some(prep)));
                        let e = commands.spawn((Mesh3d(h), MeshMaterial3d(bmats.get(Mat::LodWall)), Transform::default(), ChildOf(parent_id))).id();
                        if !batch.buckets.is_empty() {
                            mgr.deferred.insert(t, DeferredBuildings { buckets: batch.buckets, root: parent_id, far_entity: e });
                        }
                    } else if !batch.buckets.is_empty() {
                        // Bevy's shadow pass has no sub-mesh culling, so the merged tile LOD
                        // would cast whole into every cascade — chunk stand-ins cover the sun
                        // instead (see `Shadow`). The Metal renderer cluster-culls per pass, so
                        // on native the merged LOD casts directly and chunks exist only for
                        // near detail.
                        let shadow = if crate::native::enabled() { Shadow::Cast } else { Shadow::None };
                        let e = commands.spawn((Transform::default(), Visibility::default(), ChildOf(parent_id))).id();
                        spawn_buckets(&mut commands, &mut meshes, &bmats, batch, e, shadow, &mgr.prep_tx);
                        a.lod_root = Some(e);
                    }
                    a.stage = Stage::Trees;
                }
                Stage::Trees => {
                    let tr = &a.ready.trees;
                    let cost = 2 + tr.impostors.as_ref().map_or(0, |(m, _)| tris(m)) / 10_000;
                    if !budget.try_spend(cost) {
                        blocked = true;
                        continue;
                    }
                    let tr = std::mem::replace(&mut a.ready.trees, crate::foliage::PreparedTrees { up: Vec3::Y, chunks: Vec::new(), impostors: None });
                    let far_tag = mgr.sat_tags.get(&t).map_or(0, |g| g & 0x00FF_F000);
                    crate::foliage::spawn_tile_trees(&mut commands, &mut meshes, &impostors, a.parent.unwrap(), tr, far_tag);
                    a.stage = Stage::Finish;
                }
                Stage::Finish => unreachable!(),
            }
        }
        if blocked {
            mgr.applying.push_front(a);
            break;
        }
        // Finish: reveal and register.
        let parent_id = a.parent.unwrap();
        commands.entity(parent_id).insert(Visibility::Inherited);
        let b = &a.ready.build;
        if let Some(fp) = focus_pos {
            // Ground height estimate under the player for LOD (finest tile wins).
            let g = origin.to_geodetic(fp);
            let (u, v) = t.geodetic_to_uv(g.lat_deg, g.lon_deg);
            if (0.0..1.0).contains(&u) && (0.0..1.0).contains(&v) && t.z >= 12 {
                mgr.ground_estimate = ((b.min_max_height.0 + b.min_max_height.1) * 0.5) as f64;
            }
        }
        if let Some(set) = &b.building_detail {
            dm.tiles.insert(
                t,
                TileDetail {
                    set: set.clone(),
                    root: parent_id,
                    lod_root: a.lod_root,
                    lod_src: a.ready.lod_chunks.take(),
                    lod: HashMap::new(),
                    explode: Explode::Merged,
                    busy_at: 0.0,
                    loaded: HashMap::new(),
                    pending: HashSet::new(),
                },
            );
        }
        mgr.tile_totals.insert(t, Totals { buildings: b.stats.buildings, trees: b.stats.trees, triangles: b.stats.triangles });
        mgr.heights.insert(t, b.min_max_height);
        mgr.sea_frac.insert(t, b.sea_fraction);
        mgr.recount();
        mgr.loaded.insert(t, parent_id);
        mgr.pending.remove(&t);
        mgr.seen.touch(&t);
    }
    crate::probes::add_apply(t0.elapsed());
}

/// `EW_DUMP_SAT=dir` writes each tile's satellite texture as a PPM (debugging).
fn dump_sat(t: TileId, n: u32, data: &[u8]) {
    let Some(dir) = std::env::var_os("EW_DUMP_SAT") else { return };
    let _ = std::fs::create_dir_all(&dir);
    let mut ppm = format!("P6\n{n} {n}\n255\n").into_bytes();
    for px in data.chunks_exact(4) {
        ppm.extend_from_slice(&px[..3]);
    }
    let _ = std::fs::write(std::path::Path::new(&dir).join(format!("{}_{}_{}.ppm", t.z, t.x, t.y)), ppm);
}

/// Despawn tiles we no longer want once their replacements are in (where
/// you can look), and the oldest out-of-view tiles once over the residency cap.
fn retire_tiles(mut commands: Commands, mut mgr: ResMut<TileManager>, mut dm: ResMut<DetailManager>, mut sat_pool: ResMut<crate::ground_material::SatPool>, mut far_pool: ResMut<crate::far_shadows::FarPool>) {
    let t0 = std::time::Instant::now();
    let mgr = &mut *mgr;
    let jumped = std::mem::take(&mut mgr.jumped);
    // After an origin jump the wanted set is an entirely new region: every loaded
    // tile that is no longer wanted is stale and must go now (not a few per
    // frame). Relying on `scheduler::retire` here is too conservative - its
    // blocking/ancestor holding keeps almost all of them alive, so the mesh slab
    // grows until the GPU runs out of memory.
    let retire = if jumped {
        let r: Vec<TileId> = mgr
            .loaded
            .keys()
            .copied()
            .filter(|t| !mgr.wanted.contains(t))
            .collect();
        info!("origin jump: retiring {} stale tiles at once", r.len());
        r
    } else {
        let mut r = scheduler::retire(mgr.loaded.keys().copied(), &mgr.wanted, |t| mgr.loaded.contains_key(t), |t| mgr.in_view.contains(t));
        // Over the cap: oldest unseen first. `over_cap` never returns tiles seen
        // this tick, and in-view tiles are touched every select.
        let over_cap = mgr.loaded.len() > mgr.resident_cap;
        if over_cap {
            r.extend(mgr.seen.over_cap(mgr.resident_cap).into_iter().filter(|t| !mgr.in_view.contains(t)));
            // Do not throttle retirement while over the residency cap: dropping
            // tiles fast enough to avoid GPU OOM matters more than avoiding a
            // despawn hitch here.
            r
        } else {
            r.truncate(env_num("EW_RETIRE_PER_FRAME", 64));
            r
        }
    };
    if retire.is_empty() {
        crate::probes::add_retire(t0.elapsed());
        return;
    }
    for t in retire {
        let Some(e) = mgr.loaded.remove(&t) else { continue };
        commands.entity(e).despawn();
        mgr.tile_totals.remove(&t);
        mgr.heights.remove(&t);
        mgr.sea_frac.remove(&t);
        mgr.seen.remove(&t);
        mgr.deferred.remove(&t);
        if let Some(td) = dm.tiles.remove(&t) {
            // A tile's detail data is tens of MB; freeing it on the main thread is part of the
            // fast-travel freeze. Drop it on a worker.
            bevy::tasks::AsyncComputeTaskPool::get().spawn(async move { drop(td) }).detach();
        }
        if let Some(tag) = mgr.sat_tags.remove(&t) {
            sat_pool.release(tag);
            far_pool.release(tag);
        }
    }
    mgr.recount();
    crate::probes::add_retire(t0.elapsed());
}

/// Swap far massing tiles to full geometry when the camera comes within `far_lod_m()`.
fn upgrade_far_buildings(
    mut commands: Commands,
    mut mgr: ResMut<TileManager>,
    mut meshes: ResMut<Assets<Mesh>>,
    bmats: Res<BuildingMaterials>,
    focus: Query<&GlobalTransform, With<Focus>>,
    tiles: Query<&GlobalTransform, With<WorldTile>>,
) {
    if mgr.deferred.is_empty() {
        return;
    }
    let Some(fp) = focus.iter().next().map(|t| t.translation()) else { return };
    let near = far_lod_m() as f32;
    let mut eligible: Vec<TileId> = mgr
        .deferred
        .iter()
        .filter_map(|(tid, d)| {
            let gt = tiles.get(d.root).ok()?;
            (gt.translation().distance(fp) <= near).then_some(*tid)
        })
        .collect();
    eligible.truncate(env_num("EW_UPGRADE_PER_FRAME", 2));
    for tid in eligible {
        let Some(d) = mgr.deferred.remove(&tid) else { continue };
        commands.entity(d.far_entity).despawn();
        let batch = BuildingBatch { buckets: d.buckets, collider: None };
        spawn_buckets(&mut commands, &mut meshes, &bmats, batch, d.root, Shadow::Cast, &mgr.prep_tx);
    }
}

/// Build full-detail building chunks near the focus; drop them when far.
/// A tile explodes its merged LOD into chunks before its first detail chunk
/// and implodes again once nothing has been near for a while.
fn stream_detail(
    time: Res<Time>,
    mut commands: Commands,
    mut dm: ResMut<DetailManager>,
    focus: Query<&GlobalTransform, With<Focus>>,
    tiles: Query<&GlobalTransform, With<WorldTile>>,
    mut vis: Query<&mut Visibility>,
    children: Query<&Children>,
) {
    let now = time.elapsed_secs();
    if now - dm.last < 0.3 {
        return;
    }
    dm.last = now;
    let Some(fp) = focus.iter().next().map(|t| t.translation()) else { return };
    let pool = AsyncComputeTaskPool::get();
    let mut want: Vec<(f32, TileId, (u8, u8), bool)> = Vec::new();
    let mut drop: Vec<(TileId, (u8, u8))> = Vec::new();
    let mut explode: Vec<TileId> = Vec::new();
    for (tid, td) in dm.tiles.iter_mut() {
        let Ok(gt) = tiles.get(td.root) else { continue };
        let mut wants = false;
        let mut shadow_near = false;
        for chunk in td.set.chunks.keys() {
            let c = gt.transform_point(td.set.chunk_center(*chunk));
            let d = c.distance(fp);
            // Exploding into per-chunk entities to feed Bevy's shadow passes churns the whole
            // caster list (cascades across 750 m); on native the merged LOD casts its own
            // shadows, so chunks are only built when near detail actually wants them.
            if !crate::native::enabled() {
                shadow_near |= d < SHADOW_EXPLODE_M;
            }
            let cur = td.loaded.get(chunk).map(|l| l.1);
            // Level this chunk should show: keep the current one inside the band.
            let lite = if d < NEAR_IN_M { false } else if d > NEAR_OUT_M { true } else { cur.unwrap_or(true) };
            if d < DETAIL_IN_M && cur != Some(lite) && !td.pending.contains(chunk) {
                wants = true;
                if matches!(td.explode, Explode::Exploded) || td.lod_src.is_none() {
                    // Level swaps rank after first loads (holes matter more).
                    want.push((d + if cur.is_some() { 1000.0 } else { 0.0 }, *tid, *chunk, lite));
                }
            } else if d > DETAIL_OUT_M && td.loaded.contains_key(chunk) {
                drop.push((*tid, *chunk));
            }
        }
        if wants || shadow_near || !td.loaded.is_empty() || !td.pending.is_empty() {
            td.busy_at = now;
        }
        if (wants || shadow_near) && matches!(td.explode, Explode::Merged) && td.lod_src.is_some() {
            explode.push(*tid);
        }
        // Nothing near for a while: merge back into the one-mesh-per-material LOD.
        if matches!(td.explode, Explode::Exploded) && now - td.busy_at > IMPLODE_AFTER_S {
            for (_, e) in td.lod.drain() {
                commands.entity(e).despawn();
            }
            if let Some(l) = td.lod_root.and_then(|e| vis.get_mut(e).ok()) {
                *l.into_inner() = Visibility::Inherited;
            }
            td.explode = Explode::Merged;
        }
    }
    for tid in explode {
        let Some(td) = dm.tiles.get_mut(&tid) else { continue };
        let Some(src) = td.lod_src.clone() else { continue };
        td.explode = Explode::Pending;
        let tx = dm.etx.clone();
        pool.spawn(async move {
            let batches = src.iter().map(|(c, m)| (*c, prepare_batch(m, false))).collect();
            let _ = tx.send((tid, batches));
        })
        .detach();
    }
    for (tid, chunk) in drop {
        if let Some(td) = dm.tiles.get_mut(&tid) {
            if let Some((e, _)) = td.loaded.remove(&chunk) {
                commands.entity(e).despawn();
            }
            if let Some(l) = td.lod.get(&chunk) {
                set_shadow_proxy(&mut commands, &children, *l, false);
            }
        }
    }
    want.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (_, tid, chunk, lite) in want {
        if dm.in_flight >= DETAIL_IN_FLIGHT {
            break;
        }
        let tx = dm.tx.clone();
        let Some(td) = dm.tiles.get_mut(&tid) else { continue };
        td.pending.insert(chunk);
        let set = td.set.clone();
        dm.in_flight += 1;
        pool.spawn(async move {
            let bm = set.build_chunk(chunk, lite);
            if std::env::var_os("EW_TILE_STATS").is_some() {
                let mut per: Vec<_> = bm.out.meshes.iter().map(|(k, m)| (m.triangle_count(), format!("{k:?}"))).collect();
                per.sort_by(|a, b| b.0.cmp(&a.0));
                eprintln!("DETAILSTAT lite={lite} n={} tris={} {:?}", bm.count, bm.triangle_count(), &per[..per.len().min(8)]);
            }
            let _ = tx.send((tid, chunk, lite, prepare_batch(&bm, false)));
        })
        .detach();
    }
}

fn receive_detail(
    mut commands: Commands,
    mut dm: ResMut<DetailManager>,
    mgr: Res<TileManager>,
    mut meshes: ResMut<Assets<Mesh>>,
    bmats: Res<BuildingMaterials>,
    children: Query<&Children>,
) {
    let Ok((tid, chunk, lite, bmesh)) = dm.rx.try_recv() else { return };
    dm.in_flight = dm.in_flight.saturating_sub(1);
    let Some(td) = dm.tiles.get_mut(&tid) else { return };
    td.pending.remove(&chunk);
    // Imploded while this was building: the merged LOD would overlap it.
    if td.lod_src.is_some() && !matches!(td.explode, Explode::Exploded) {
        return;
    }
    let e = commands.spawn((Transform::default(), Visibility::default(), ChildOf(td.root))).id();
    // The chunk's LOD stays on as the detail's shadow stand-in (see `Shadow`).
    let proxy = td.lod.get(&chunk).copied();
    spawn_buckets(&mut commands, &mut meshes, &bmats, bmesh, e, if proxy.is_some() { Shadow::CameraOnly } else { Shadow::Cast }, &mgr.prep_tx);
    // Level swap: the old level goes in the same command flush the new one
    // spawns in, so the chunk is never empty.
    if let Some((old, _)) = td.loaded.insert(chunk, (e, lite)) {
        commands.entity(old).despawn();
    }
    if let Some(l) = proxy {
        set_shadow_proxy(&mut commands, &children, l, true);
    }
}

/// Take prepared per-chunk LOD batches and spawn them a few per frame; once
/// a tile's are all in, hide its merged LOD.
fn receive_explode(
    mut commands: Commands,
    mut dm: ResMut<DetailManager>,
    mgr: Res<TileManager>,
    mut meshes: ResMut<Assets<Mesh>>,
    bmats: Res<BuildingMaterials>,
    mut vis: Query<&mut Visibility>,
) {
    while let Ok((tid, batches)) = dm.erx.try_recv() {
        if let Some(td) = dm.tiles.get_mut(&tid) {
            if matches!(td.explode, Explode::Pending) {
                td.explode = Explode::Spawning(batches);
            }
        }
    }
    let mut budget = EXPLODE_CHUNKS_PER_FRAME;
    for td in dm.tiles.values_mut() {
        let Explode::Spawning(batches) = &mut td.explode else { continue };
        while budget > 0 {
            let Some((chunk, batch)) = batches.pop() else { break };
            budget -= 1;
            let e = commands.spawn((Transform::default(), Visibility::default(), ChildOf(td.root))).id();
            spawn_buckets(&mut commands, &mut meshes, &bmats, batch, e, Shadow::Cast, &mgr.prep_tx);
            td.lod.insert(chunk, e);
        }
        if batches.is_empty() {
            if let Some(l) = td.lod_root.and_then(|e| vis.get_mut(e).ok()) {
                *l.into_inner() = Visibility::Hidden;
            }
            td.explode = Explode::Exploded;
        }
        if budget == 0 {
            break;
        }
    }
}

/// Number of full-detail chunks currently shown (for the HUD / capture).
pub fn detail_stats(dm: &DetailManager) -> (usize, usize) {
    let loaded = dm.tiles.values().map(|t| t.loaded.len()).sum();
    (loaded, dm.in_flight)
}
