//! Near trees (trees G4): every simulated stand tree (`ed_world::stand`) within the hand-off
//! distance becomes a GPU instance of a tree bank mesh (`ed_world::trees::bank`), at full
//! density (no caps, no thinning). The renderer picks the LOD per instance by projected size;
//! the last LOD is empty, which hands the tree to the far forest rung (`EW_TREES_FAR`) at the
//! reference tree's `HANDOFF_PX`, the size where that rung's canopy lift starts.
//!
//! Per level-14 node: stand from the cached level-12 height and canopy pages (built on worker
//! threads, results in sorted order), one renderer node origin, one instance per tree with its
//! per-instance variety (`bank::instance_params`) in the instance user word. Nodes leave with
//! their instances when they fall out of range. Bank meshes are built lazily per (species,
//! variant, age) the stands use.
//!
//! Impostors (trees G5, default on; `EW_TREES_IMPOSTOR=0` returns to the G4 shell chain):
//! every stand tree out to the T2 -> T3 hand-off (`bank::IMP_OUT_PX`, about twice the G4
//! distance) goes into the renderer's impostor buffer; mesh instances exist only in nodes
//! where a tree can still project over `bank::MESH_KEEP_PX`, and their LOD chain ends at T1
//! (no shell). One octahedral impostor per bank entry, baked on worker threads and cached on
//! disk (`impostors/v<BAKE_VERSION>/...`). `EW_IMP_TIER=low|medium|high` (frame 32/48/64 px,
//! default high), `EW_IMP_SHADOWS=0` (impostors cast no sun shadow).
//!
//! Switches: `EW_TREES_NEAR=0` (no near trees: the frame is the trees-off frame), `EW_WIND=0`
//! (trees hold still).

use crate::terrain::{CacheSource, TerrainWorld, build_canopy};
use ed_data::DiskCache;
use ed_geo::{CanopyPage, HeightPage, NodeId};
use ed_render::scene::{InstanceDesc, InstanceId, Material, MaterialId, MaterialTextures, MeshId, NodeSlot, TextureAsset, TextureHandle};
use ed_render::impostor::{ImpostorBands, ImpostorKind, ImpostorRange, ImpostorTree};
use ed_render::{Camera, Renderer};
use ed_world::trees::bank_impostor::{self, ImpostorTier};
use ed_world::stand::{CanopySample, StandTree, stand_for_node};
use ed_world::trees::bank;
use ed_world::wind::WindField;
use glam::{Affine3A, DVec3, Quat, Vec3};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

/// Stand nodes are this level (about 600 m).
pub const NODE_LEVEL: u8 = 14;
/// Canopy and height pages the stands sample (the G2 stand level).
const PAGE_LEVEL: u8 = 12;
/// Stand seed (the G2 dumps).
const STAND_SEED: u64 = 0xE7E2;
/// Nodes this near are kept whatever the view direction (turning around never shows a hole).
const KEEP_ALL_M: f64 = 450.0;

type Page = Option<(Arc<HeightPage>, Arc<CanopyPage>)>;
type Pages = BTreeMap<NodeId, Page>;

type BankKey = (u8, u32, u8);

/// A bank entry's CPU side: the mesh and its impostor bake (impostors on), built off the main
/// thread; the main thread only uploads it.
struct Prep {
    mesh: bank::BankMesh,
    bake: Option<bank_impostor::BankImpostor>,
    cached: bool,
}

/// From the background worker (all pure functions of their node or key).
enum Built {
    /// Bank entries a chunk of stands needs (sent before the stands).
    Prep { keys: Vec<(BankKey, Prep)>, bank_ms: f64, bake_ms: f64 },
    Atlas(TextureAsset),
    Stand { id: NodeId, stand: Vec<StandTree>, keys: Vec<BankKey>, pages_ms: f64, stand_ms: f64 },
}

fn leaf_atlas_asset() -> TextureAsset {
    let a = ed_world::trees::textures::leaf_atlas();
    let mut mips = Vec::new();
    let (mut w, mut h, mut off) = (a.width as usize, a.height as usize, 0usize);
    for _ in 0..a.mip_levels {
        let n = w * h * 4;
        mips.push(a.data[off..off + n].to_vec());
        off += n;
        w = (w / 2).max(1);
        h = (h / 2).max(1);
    }
    TextureAsset { format: ed_gpu::Format::Rgba8Srgb, width: a.width, height: a.height, mips }
}

fn keys_of(stand: &[StandTree]) -> BTreeSet<BankKey> {
    stand.iter().map(|t| {
        let p = bank::instance_params(t);
        (t.species, p.variant, p.age)
    }).collect()
}

/// Bank meshes (and impostor bakes, cached on disk by content) for `need`, built in parallel,
/// in `need` order. Returns (mesh ms, bake ms) too.
fn prepare_bank(cap: usize, need: &[BankKey], imp: bool, tier: ImpostorTier, cache: &DiskCache) -> (Vec<(BankKey, Prep)>, f64, f64) {
    let t = Instant::now();
    let slots: Vec<OnceLock<bank::BankMesh>> = (0..need.len()).map(|_| OnceLock::new()).collect();
    par_for(cap, need.len(), |k| {
        let (s, v, a) = need[k];
        let _ = slots[k].set(bank::bank_mesh(s as usize, v, a));
    });
    let meshes: Vec<bank::BankMesh> = slots.into_iter().map(|s| s.into_inner().expect("bank worker")).collect();
    let bank_ms = t.elapsed().as_secs_f64() * 1e3;
    let mut bakes: Vec<Option<(bank_impostor::BankImpostor, bool)>> = (0..meshes.len()).map(|_| None).collect();
    let mut bake_ms = 0.0;
    if imp {
        let ti = Instant::now();
        let keys: Vec<String> = need.iter().zip(&meshes).map(|(k, m)| imp_cache_key(*k, m, tier)).collect();
        let out: Vec<OnceLock<(bank_impostor::BankImpostor, bool)>> = (0..meshes.len()).map(|_| OnceLock::new()).collect();
        let leaves = OnceLock::new();
        par_for(cap, meshes.len(), |k| {
            if let Some(b) = cache.get(&keys[k], None).and_then(|b| bank_impostor::BankImpostor::from_bytes(&b)) {
                let _ = out[k].set((b, true));
                return;
            }
            let lt = leaves.get_or_init(|| bank_impostor::LeafTex::new(&ed_world::trees::textures::leaf_atlas()));
            let b = bank_impostor::bake(&meshes[k], 1, lt, tier);
            let _ = cache.put(&keys[k], &b.to_bytes());
            let _ = out[k].set((b, false));
        });
        for (k, o) in out.into_iter().enumerate() {
            bakes[k] = Some(o.into_inner().expect("bake worker"));
        }
        bake_ms = ti.elapsed().as_secs_f64() * 1e3;
    }
    let v = need
        .iter()
        .zip(meshes)
        .zip(bakes)
        .map(|((k, mesh), b)| {
            let (bake, cached) = match b {
                Some((b, c)) => (Some(b), c),
                None => (None, false),
            };
            (*k, Prep { mesh, bake, cached })
        })
        .collect();
    (v, bank_ms, bake_ms)
}

/// Background builder: pages and stands, off the main thread. Requests are batches of nodes;
/// each batch's pages and stands are built with the same data-parallel loops the blocking path
/// used, in chunks so results stream back. Output depends only on the node, never on timing.
struct Worker {
    jobs: Sender<(Vec<NodeId>, usize, usize)>,
    done: Receiver<Built>,
    /// Nodes still wanted (the worker skips the rest).
    wanted: Arc<Mutex<BTreeSet<NodeId>>>,
}

/// Nodes per streamed chunk of a batch.
const CHUNK: usize = 4;

fn spawn_worker(src: Arc<CacheSource>, cache: Arc<DiskCache>, pages: Arc<Mutex<Pages>>, imp: bool, tier: ImpostorTier, mut have: BTreeSet<BankKey>) -> Worker {
    let (jobs, jobs_rx) = channel::<(Vec<NodeId>, usize, usize)>();
    let (done_tx, done) = channel::<Built>();
    let wanted: Arc<Mutex<BTreeSet<NodeId>>> = Arc::new(Mutex::new(BTreeSet::new()));
    let w2 = wanted.clone();
    // A named thread (`thread::Builder`) trips the `ew` no-procedural-geometry lint; unnamed is fine.
    std::thread::spawn(move || {
            if done_tx.send(Built::Atlas(leaf_atlas_asset())).is_err() {
                return;
            }
            while let Ok((batch, chunk, cap)) = jobs_rx.recv() {
                for chunk in batch.chunks(chunk.max(1)) {
                    let live: Vec<NodeId> = {
                        let w = w2.lock().unwrap();
                        chunk.iter().copied().filter(|id| w.contains(id)).collect()
                    };
                    if live.is_empty() {
                        continue;
                    }
                    let pages_ms = build_pages(cap, &src, &cache, &pages, &live) / live.len() as f64;
                    let t = Instant::now();
                    // A snapshot of the page table: the main thread keeps reading it meanwhile.
                    let snap = pages.lock().unwrap().clone();
                    let stands = stands_for(cap, &snap, &live);
                    let stand_ms = t.elapsed().as_secs_f64() * 1e3 / live.len() as f64;
                    let need: Vec<BankKey> = stands.iter().flat_map(|s| keys_of(s)).collect::<BTreeSet<_>>().into_iter().filter(|k| !have.contains(k)).collect();
                    if !need.is_empty() {
                        let (keys, bank_ms, bake_ms) = prepare_bank(cap, &need, imp, tier, &cache);
                        have.extend(need);
                        if done_tx.send(Built::Prep { keys, bank_ms, bake_ms }).is_err() {
                            return;
                        }
                    }
                    for (id, stand) in live.into_iter().zip(stands) {
                        let keys: Vec<BankKey> = keys_of(&stand).into_iter().collect();
                        if done_tx.send(Built::Stand { id, stand, keys, pages_ms, stand_ms }).is_err() {
                            return;
                        }
                    }
                }
            }
        });
    Worker { jobs, done, wanted }
}

/// Level-12 pages around `ids` (3x3 neighbourhood of each parent), built in parallel and
/// inserted into the shared map. Returns the build time (ms).
fn build_pages(cap: usize, src: &CacheSource, cache: &DiskCache, pages: &Mutex<Pages>, ids: &[NodeId]) -> f64 {
    let mut need = BTreeSet::new();
    {
        let pg = pages.lock().unwrap();
        for id in ids {
            let p = id.ancestor(PAGE_LEVEL);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let q = p.neighbour(dx, dy);
                    if !pg.contains_key(&q) {
                        need.insert(q);
                    }
                }
            }
        }
    }
    let need: Vec<NodeId> = need.into_iter().collect();
    if need.is_empty() {
        return 0.0;
    }
    let t = Instant::now();
    let slots: Vec<OnceLock<Page>> = (0..need.len()).map(|_| OnceLock::new()).collect();
    par_for(cap, need.len(), |k| {
        let hp = ed_data::node::node_heights(src, need[k]);
        let cp = build_canopy(cache, need[k], &hp);
        let _ = slots[k].set(cp.map(|cp| (Arc::new(hp), cp)));
    });
    let mut pg = pages.lock().unwrap();
    for (id, s) in need.iter().zip(slots) {
        pg.insert(*id, s.into_inner().flatten());
    }
    t.elapsed().as_secs_f64() * 1e3
}

/// Stands of `ids` from resident `pages`, built in parallel (results in input order).
fn stands_for(cap: usize, pages: &Pages, ids: &[NodeId]) -> Vec<Vec<StandTree>> {
    let sample = |d: DVec3| -> Option<CanopySample> {
        let id = NodeId::from_unit(ids[0].body(), d, PAGE_LEVEL);
        let (hp, cp) = pages.get(&id)?.as_ref()?;
        let (u, v) = id.local_uv(d);
        Some(CanopySample::from_page(cp, u, v, hp.sample(u, v) as f32, 0.3))
    };
    let slots: Vec<OnceLock<Vec<StandTree>>> = (0..ids.len()).map(|_| OnceLock::new()).collect();
    par_for(cap, ids.len(), |k| {
        let _ = slots[k].set(stand_for_node(ids[k], &sample, STAND_SEED));
    });
    slots.into_iter().map(|s| s.into_inner().unwrap_or_default()).collect()
}

struct NodeEntry {
    slot: NodeSlot,
    /// Node origin (render frame at placement).
    origin: DVec3,
    /// Mesh instances (none while the node is impostors only).
    instances: Vec<InstanceId>,
    /// Every tree: trunk base relative to `origin`, height (m).
    trees: Vec<(Vec3, f32)>,
    /// Impostor block (impostors on).
    imp: Option<ImpostorRange>,
    meshed: bool,
    /// Still being applied in slices (not remeshed meanwhile).
    building: bool,
    /// The node's stand (re-meshing as the camera moves needs no rebuild).
    stand: Arc<Vec<StandTree>>,
}

/// One node being applied in slices (a node's 30k trees do not fit one frame's budget):
/// placement, then the impostor block, then the mesh instances.
struct Build {
    id: NodeId,
    stand: Arc<Vec<StandTree>>,
    gh: f64,
    pos: usize,
    /// Per stand tree: trunk base relative to the node origin and up (`None`: no bank entry).
    placed: Vec<Option<(Vec3, Vec3, u32)>>,
    imps: Vec<ImpostorTree>,
    /// 0 placing, 1 impostors, 2 mesh instances.
    phase: u8,
}

/// Trees per slice (about 1 ms).
const SLICE: usize = 3000;

/// Near-tree counters (headless report).
#[derive(Clone, Copy, Debug, Default)]
pub struct NearStats {
    pub nodes: usize,
    pub instances: usize,
    pub bank_meshes: usize,
    pub bank_bytes: usize,
    pub bank_ms: f64,
    pub pages_ms: f64,
    pub stand_ms: f64,
    pub stand_nodes: usize,
    pub apply_ms: f64,
    pub imp_trees: usize,
    pub imp_kinds: usize,
    pub imp_frame_px: u32,
    pub imp_bytes: usize,
    pub imp_bake_ms: f64,
    pub imp_baked: usize,
    pub imp_cached: usize,
    /// Nodes applied in the last update and the wall time of its apply stage (ms).
    pub applied_last: usize,
    pub apply_last_ms: f64,
    /// Nodes requested and not yet applied.
    pub pending: usize,
}

pub struct NearTrees {
    pub on: bool,
    pub wind: Option<WindField>,
    cache: Arc<DiskCache>,
    pages: Arc<Mutex<Pages>>,
    worker: Option<Worker>,
    /// Nodes sent to the worker, not yet applied.
    requested: BTreeSet<NodeId>,
    /// Finished stands waiting for their turn under the apply budget.
    building: Option<Build>,
    ready: BTreeMap<NodeId, (Arc<Vec<StandTree>>, Vec<BankKey>)>,
    /// Bank entries built by the worker, waiting for upload.
    prep: BTreeMap<BankKey, Prep>,
    /// Per-frame apply budget: nodes, ms, new bank entries.
    apply_nodes: usize,
    /// Threads the background build may use (half the cores: the main thread keeps its own).
    worker_threads: usize,
    apply_ms: f64,
    bank_keys: usize,
    atlas: Option<TextureHandle>,
    /// The leaf atlas texture the worker built (uploaded with the first bank entry).
    atlas_cpu: Option<TextureAsset>,
    /// Mesh, material, height, impostor kind.
    bank: BTreeMap<(u8, u32, u8), (MeshId, MaterialId, f32, u32)>,
    /// Impostors on (`EW_TREES_IMPOSTOR`, default on) and their frame tier.
    pub imp: bool,
    imp_tier: ImpostorTier,
    imp_shadows: bool,
    /// T1 -> T2 hand-off (px): `bank::T1_PX`, or `EW_IMP_T1_PX` (the impostor-in band and
    /// the mesh-keep range scale with it).
    imp_t1: f32,
    nodes: BTreeMap<NodeId, NodeEntry>,
    free_slots: Vec<NodeSlot>,
    pub stats: NearStats,
    /// Hand-off distance of the last update (m).
    pub handoff_m: f64,
    /// Seasons (trees G5; `EW_SEASON=0` off): the calendar day (default midsummer, `--date`) and
    /// the rate scalar (`EW_SEASON_RATE`, 1 = real time, 0 holds midsummer).
    season: Option<SeasonCfg>,
}

#[derive(Clone, Copy, Debug)]
struct SeasonCfg {
    day: f64,
    rate: f64,
}

fn env_num<T: std::str::FromStr>(k: &str, d: T) -> T {
    std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d)
}

/// `EW_TREES_NEAR` (default on).
pub fn near_from_env() -> bool {
    std::env::var("EW_TREES_NEAR").map_or(true, |v| v != "0")
}

impl NearTrees {
    pub fn new(cache: &Path) -> NearTrees {
        let wind = std::env::var("EW_WIND").map_or(true, |v| v != "0").then(WindField::default);
        NearTrees {
            on: near_from_env(),
            wind,
            cache: Arc::new(DiskCache::new(cache)),
            pages: Arc::new(Mutex::new(BTreeMap::new())),
            worker: None,
            requested: BTreeSet::new(),
            building: None,
            ready: BTreeMap::new(),
            prep: BTreeMap::new(),
            worker_threads: env_num("EW_TREES_THREADS", (std::thread::available_parallelism().map_or(8, |t| t.get()) / 2).max(1)),
            apply_nodes: env_num("EW_TREES_APPLY", 2usize).max(1),
            apply_ms: env_num("EW_TREES_APPLY_MS", 2.0f64),
            bank_keys: env_num("EW_TREES_BANK", 8usize).max(1),
            atlas: None,
            atlas_cpu: None,
            bank: BTreeMap::new(),
            imp: std::env::var("EW_TREES_IMPOSTOR").map_or(true, |v| v != "0"),
            imp_tier: match std::env::var("EW_IMP_TIER").as_deref() {
                Ok("low") => ImpostorTier::Low,
                Ok("medium") => ImpostorTier::Medium,
                _ => ImpostorTier::High,
            },
            imp_shadows: std::env::var("EW_IMP_SHADOWS").map_or(true, |v| v != "0"),
            imp_t1: std::env::var("EW_IMP_T1_PX").ok().and_then(|v| v.parse::<f32>().ok()).filter(|v| *v >= 20.0).unwrap_or(bank::T1_PX),
            nodes: BTreeMap::new(),
            free_slots: Vec::new(),
            stats: NearStats::default(),
            handoff_m: 0.0,
            season: std::env::var("EW_SEASON").map_or(true, |v| v != "0").then(|| SeasonCfg { day: ed_world::season::REF_DAY, rate: env_num("EW_SEASON_RATE", 1.0f64) }),
        }
    }

    /// The calendar day of year (`ed_world::season::day_of_year`) the trees show; `None` keeps
    /// the default (midsummer). Ignored with `EW_SEASON=0`.
    pub fn set_date(&mut self, day: Option<f64>) {
        if let (Some(d), Some(s)) = (day, self.season.as_mut()) {
            s.day = d;
        }
    }

    /// Renderer limits for a terrain run with near trees (every instance of a dense forest).
    pub fn renderer_desc() -> ed_render::RendererDesc {
        ed_render::RendererDesc { max_instances: 1 << 20, max_vertices: 1 << 23, max_indices: 1 << 24, max_meshes: 1024, max_nodes: 4000, ..Default::default() }
    }

    /// The level-14 nodes that should hold trees for `camera`: inside the hand-off distance, all
    /// around near the camera and inside the view cone farther out.
    fn wanted(&self, wd: &TerrainWorld, camera: &Camera, viewport: [u32; 2]) -> (BTreeSet<NodeId>, f64) {
        let body = &wd.anchor.body;
        let ppr = viewport[1] as f64 * 0.5 / (camera.fov_y as f64 * 0.5).tan();
        let handoff_px = if self.imp { bank::IMP_OUT_PX.0 } else { bank::HANDOFF_PX };
        let handoff = bank::TREE_REF_M as f64 * ppr / handoff_px as f64;
        let cb = wd.anchor.render_to_body(camera.position);
        let cd = cb.normalize();
        let gh = wd.ground(camera.position).unwrap_or(0.0);
        let c0 = NodeId::from_unit(body.id, cd, NODE_LEVEL);
        let size = (body.point(c0.neighbour(1, 0).centre().normalize(), gh) - body.point(c0.centre().normalize(), gh)).length();
        let n = (handoff / size).ceil() as i32 + 1;
        let fwd = camera.forward().as_dvec3();
        let aspect = viewport[0] as f64 / viewport[1] as f64;
        let half_diag = ((camera.fov_y as f64 * 0.5).tan() * (1.0 + aspect * aspect).sqrt()).atan();
        let mut out = BTreeSet::new();
        for dy in -n..=n {
            for dx in -n..=n {
                let id = c0.neighbour(dx, dy);
                let d = id.centre().normalize();
                // The node's own ground (the camera's ground misplaces nodes on slopes and in
                // valleys far below a high camera, dropping them out of the cone).
                let p = wd.anchor.body_to_render(body.point(d, self.ground_at(wd, d).unwrap_or(gh)));
                let to = p - camera.position;
                let dist = to.length();
                if dist > handoff + size * 0.75 {
                    continue;
                }
                let near = dist < KEEP_ALL_M + size * 0.75;
                let cone = to.normalize_or_zero().dot(fwd) > (half_diag + (size * 0.75 / dist.max(1.0)).atan()).min(std::f64::consts::PI).cos();
                if near || cone {
                    out.insert(id);
                }
            }
        }
        (out, handoff)
    }

    /// Uploads bank entries for `keys`: the worker's prepared ones, else built here (blocking).
    fn ensure_bank(&mut self, r: &mut Renderer, dev: &dyn ed_gpu::Device, keys: &BTreeSet<(u8, u32, u8)>) -> Result<(), String> {
        let need: Vec<(u8, u32, u8)> = keys.iter().copied().filter(|k| !self.bank.contains_key(k)).collect();
        if need.is_empty() {
            return Ok(());
        }
        if self.atlas.is_none() {
            let tex = self.atlas_cpu.take().unwrap_or_else(leaf_atlas_asset);
            self.atlas = Some(r.add_texture(dev, &tex).map_err(|e| format!("leaf atlas: {e}"))?);
        }
        let missing: Vec<BankKey> = need.iter().copied().filter(|k| !self.prep.contains_key(k)).collect();
        if !missing.is_empty() {
            let (built, bank_ms, bake_ms) = prepare_bank(usize::MAX, &missing, self.imp, self.imp_tier, &self.cache);
            self.stats.bank_ms += bank_ms;
            self.stats.imp_bake_ms += bake_ms;
            self.prep.extend(built);
        }
        let mut meshes = Vec::new();
        let mut bakes = Vec::new();
        for k in &need {
            let p = self.prep.remove(k).expect("prepared");
            if p.bake.is_some() {
                if p.cached {
                    self.stats.imp_cached += 1;
                } else {
                    self.stats.imp_baked += 1;
                }
            }
            meshes.push(p.mesh);
            bakes.push(p.bake);
        }
        if self.imp {
            self.stats.imp_frame_px = self.imp_tier.frame_px();
        }
        let t = Instant::now();
        for ((key, m), bake) in need.iter().zip(meshes).zip(bakes) {
            self.stats.bank_bytes += m.bytes();
            let mesh = if self.imp {
                // No shell: full, simplified, empty at T1 (the impostor takes over there).
                let e = bank::lod_errors_impostor(m.height, self.imp_t1);
                let lods = vec![(m.lods[0].0.clone(), e[0]), (m.lods[1].0.clone(), e[1]), (Vec::new(), e[2])];
                r.add_foliage_mesh(dev, &m.positions, &m.normals, &m.words, &lods)
            } else {
                r.add_foliage_mesh(dev, &m.positions, &m.normals, &m.words, &m.lods)
            }
            .map_err(|e| format!("bank mesh {key:?}: {e}"))?;
            let mut mat = Material::pbr(Vec3::from(m.leaf_albedo), 0.0, 0.65);
            mat.shade_color = Vec3::from(m.bark_albedo);
            mat.textures = MaterialTextures { base: self.atlas, ..Default::default() };
            let mat = r.add_material(mat).ok_or("material table full")?;
            let kind = match bake {
                Some(b) => {
                    let size = b.size();
                    let mut tex = [TextureHandle(0); 3];
                    for (i, mips) in b.tex.iter().enumerate() {
                        let a = TextureAsset { format: ed_gpu::Format::Bc5Unorm, width: size, height: size, mips: mips.clone() };
                        tex[i] = r.add_texture(dev, &a).map_err(|e| format!("impostor texture {key:?}: {e}"))?;
                    }
                    self.stats.imp_bytes += b.bytes();
                    let k = ImpostorKind {
                        tex: [0; 3],
                        frames: b.frames,
                        frame_px: b.frame_px,
                        mips: b.tex[0].len() as u32,
                        center_y: b.center_y,
                        radius: b.radius,
                        height: m.height,
                        leaf: m.leaf_albedo,
                        bark: m.bark_albedo,
                        crown_base: crown_base(&m),
                        crown_radius: m.crown_radius,
                        crown_max: m.positions.iter().map(|p| (p[0] * p[0] + p[2] * p[2]).sqrt()).fold(0.0, f32::max),
                    };
                    r.add_impostor_kind(dev, tex, k).map_err(|e| format!("impostor kind {key:?}: {e}"))?
                }
                None => 0,
            };
            self.bank.insert(*key, (mesh, mat, m.height, kind));
        }
        self.stats.imp_kinds = if self.imp { self.bank.len() } else { 0 };
        self.stats.bank_meshes = self.bank.len();
        self.stats.bank_ms += t.elapsed().as_secs_f64() * 1e3;
        Ok(())
    }

    /// The one hand-off table (bank.rs) as the renderer's bands.
    fn bands(&self) -> ImpostorBands {
        ImpostorBands {
            t2_in: (bank::IMP_IN_PX.0 * self.imp_t1 / bank::T1_PX, bank::IMP_IN_PX.1 * self.imp_t1 / bank::T1_PX),
            t3_out: bank::IMP_OUT_PX,
            lift: bank::LIFT_PX,
            paint: bank::PAINT_PX,
            ref_tree_m: bank::TREE_REF_M,
            pad: bank_impostor::IMP_PAD,
            value_max: bank_impostor::VALUE_MAX,
            shadows: self.imp_shadows,
            shadow_cascade: std::env::var("EW_IMP_SHADOW_CASCADE").ok().and_then(|v| v.parse().ok()).unwrap_or(bank::IMP_SHADOW_CASCADE),
            ground_pull_px: std::env::var("EW_IMP_PULL_PX").ok().and_then(|v| v.parse().ok()).unwrap_or(bank::IMP_GROUND_PULL_PX),
        }
    }

    /// Whether node `e` gets mesh instances: some tree in it can project over `MESH_KEEP_PX`
    /// (with 150 m of slack so camera moves between updates never show a mesh-less big tree).
    fn wants_mesh(&self, e: &NodeEntry, camera: &Camera, ppr: f64) -> bool {
        if !self.imp {
            return true;
        }
        let hmax = e.trees.iter().fold(0.0f32, |m, t| m.max(t.1)) as f64;
        let range = hmax * ppr / (bank::MESH_KEEP_PX * self.imp_t1 / bank::T1_PX) as f64 + 150.0;
        e.trees.iter().any(|t| (e.origin + t.0.as_dvec3() - camera.position).length() < range)
    }

    /// Mesh instances of `trees` (one node's stand) under `slot`.
    fn add_instances(&self, wd: &TerrainWorld, r: &mut Renderer, slot: NodeSlot, origin: DVec3, gh: f64, trees: &[StandTree]) -> Result<Vec<InstanceId>, String> {
        let mut out = Vec::with_capacity(trees.len());
        for tr in trees {
            let p = bank::instance_params(tr);
            let Some(&(mesh, material, _, _)) = self.bank.get(&(tr.species, p.variant, p.age)) else { continue };
            let (base, up, h) = self.place_h(wd, tr, gh);
            let s = p.scale;
            let rot = Quat::from_rotation_arc(Vec3::Y, up);
            let lean_axis = Vec3::new(p.lean_az.cos(), 0.0, p.lean_az.sin());
            let rot = rot * Quat::from_axis_angle(lean_axis, p.lean) * Quat::from_rotation_y(p.yaw);
            let xf = Affine3A::from_scale_rotation_translation(Vec3::new(s * p.widen, s, s * p.widen), rot, (base - origin).as_vec3());
            let Some(i) = r.instances().add(&InstanceDesc { node: slot, transform: xf, mesh, material, object: tr.id, flags: 0 }) else {
                return Err("instance pool full".into());
            };
            r.instances().set_user(i, p.user);
            r.instances().set_season(i, self.season_word(tr, h));
            out.push(i);
        }
        Ok(out)
    }

    /// Trunk base (render frame), up and ground height (m) of stand tree `tr`.
    fn place_h(&self, wd: &TerrainWorld, tr: &StandTree, gh: f64) -> (DVec3, Vec3, f64) {
        let body = &wd.anchor.body;
        let h = self.ground_at(wd, tr.dir).unwrap_or(gh);
        let base = wd.anchor.body_to_render(body.point(tr.dir, h));
        let up = (wd.anchor.body_to_render(body.point(tr.dir, h + 1.0)) - base).normalize().as_vec3();
        (base, up, h)
    }

    /// The tree's season word (0 with seasons off): its species row and its place's phenology shift.
    fn season_word(&self, tr: &StandTree, h: f64) -> u32 {
        if self.season.is_none() {
            return 0;
        }
        // Body frame is ECEF-like: z is north.
        let lat = tr.dir.z.clamp(-1.0, 1.0).asin().to_degrees();
        ed_world::season::season_word(tr.species as usize, lat, h)
    }

    /// Brings the near trees up to date for `camera`. `settle` builds and applies everything
    /// wanted before returning (headless: no holes, deterministic). Otherwise the stands build
    /// on the worker thread and finished ones apply under the per-frame budget
    /// (`EW_TREES_APPLY` nodes, `EW_TREES_APPLY_MS`, `EW_TREES_BANK` new bank entries); a node
    /// not applied yet shows the far rung. `time` drives the wind.
    pub fn update(&mut self, wd: &TerrainWorld, r: &mut Renderer, dev: &dyn ed_gpu::Device, camera: &Camera, viewport: [u32; 2], time: f64, settle: bool) -> Result<(), String> {
        let rest = r.lighting().shadow_distance;
        if !self.on {
            return Ok(());
        }
        r.set_wind(self.wind.map(|w| w.gpu_words(glam::DVec2::new(camera.position.x, -camera.position.z), time)), rest, true);
        r.set_impostor_bands(self.imp.then(|| self.bands()));
        let rows = ed_world::season::gpu_rows();
        r.set_season(self.season.map(|c| ed_render::SeasonParams { phase_day: ed_world::season::phase_day(c.day, c.rate) as f32, autumn_gain: ed_world::season::AUTUMN_GAIN as f32, far_rows: ed_world::season::far_rows(), rows: &rows }));
        let ppr = viewport[1] as f64 * 0.5 / (camera.fov_y as f64 * 0.5).tan();
        let (wanted, handoff) = self.wanted(wd, camera, viewport);
        self.handoff_m = handoff;
        // Leave: nodes out of range release their instances, impostors and node slot.
        let gone: Vec<NodeId> = self.nodes.keys().copied().filter(|id| !wanted.contains(id)).collect();
        for id in gone {
            let e = self.nodes.remove(&id).expect("listed");
            for i in e.instances {
                r.instances().remove(i);
            }
            if let Some(rg) = e.imp {
                r.remove_impostor_trees(dev, rg).map_err(|e| format!("impostors: {e}"))?;
            }
            self.free_slots.push(e.slot);
        }
        if self.building.as_ref().is_some_and(|b| !wanted.contains(&b.id)) {
            self.building = None;
        }
        self.ready.retain(|id, _| wanted.contains(id));
        self.requested.retain(|id| wanted.contains(id));
        let t_start = Instant::now();
        let ms_left = |t0: &Instant, budget: f64| settle || t0.elapsed().as_secs_f64() * 1e3 < budget;
        // Resident nodes: mesh instances come and go with the camera (impostors stay). Gaining
        // them is budgeted (the stand is kept, nothing rebuilds).
        let mut to_mesh = Vec::new();
        for (id, e) in &self.nodes {
            if e.building {
                continue;
            }
            let want = self.wants_mesh(e, camera, ppr);
            if want != e.meshed {
                to_mesh.push((*id, want));
            }
        }
        let mut meshed_now = 0usize;
        for (id, want) in to_mesh {
            if want {
                if !settle && (meshed_now >= self.apply_nodes || !ms_left(&t_start, self.apply_ms)) {
                    continue;
                }
                let (slot, origin, stand) = {
                    let e = &self.nodes[&id];
                    (e.slot, e.origin, e.stand.clone())
                };
                let gh = self.ground_at(wd, id.centre().normalize()).unwrap_or(0.0);
                let inst = self.add_instances(wd, r, slot, origin, gh, &stand)?;
                let e = self.nodes.get_mut(&id).expect("listed");
                e.instances = inst;
                e.meshed = true;
                meshed_now += 1;
            } else {
                let e = self.nodes.get_mut(&id).expect("listed");
                e.meshed = false;
                for i in std::mem::take(&mut e.instances) {
                    r.instances().remove(i);
                }
            }
        }
        // Request: wanted nodes with no entry and no request, nearest first.
        if self.worker.is_none() {
            self.worker = Some(spawn_worker(wd.source_arc(), self.cache.clone(), self.pages.clone(), self.imp, self.imp_tier, self.bank.keys().copied().collect()));
        }
        let mut new: Vec<NodeId> = wanted.iter().copied().filter(|id| !self.nodes.contains_key(id) && !self.requested.contains(id)).collect();
        if !new.is_empty() {
            let eye = wd.anchor.render_to_body(camera.position).normalize();
            new.sort_by_key(|id| ((id.centre().angle_between(eye) * 1e9) as u64, *id));
            self.requested.extend(new.iter().copied());
            let w = self.worker.as_ref().expect("worker");
            *w.wanted.lock().unwrap() = wanted.clone();
            w.jobs.send((new, if settle { usize::MAX } else { CHUNK }, if settle { usize::MAX } else { self.worker_threads })).map_err(|_| "near-trees worker stopped".to_string())?;
        } else if let Some(w) = &self.worker {
            *w.wanted.lock().unwrap() = wanted.clone();
        }
        // Collect finished stands (settle: wait for every request).
        loop {
            let w = self.worker.as_ref().expect("worker");
            let b = match w.done.try_recv() {
                Ok(b) => Some(b),
                Err(std::sync::mpsc::TryRecvError::Empty) if settle && self.requested.iter().any(|id| !self.ready.contains_key(id)) => Some(w.done.recv().map_err(|_| "near-trees worker stopped".to_string())?),
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
                Err(_) => return Err("near-trees worker stopped".into()),
            };
            let Some(b) = b else { break };
            match b {
                Built::Atlas(a) => self.atlas_cpu = Some(a),
                Built::Prep { keys, bank_ms, bake_ms } => {
                    self.stats.bank_ms += bank_ms;
                    self.stats.imp_bake_ms += bake_ms;
                    self.prep.extend(keys);
                }
                Built::Stand { id, stand, keys, pages_ms, stand_ms } => {
                    self.stats.pages_ms += pages_ms;
                    self.stats.stand_ms += stand_ms;
                    self.stats.stand_nodes += 1;
                    if self.requested.contains(&id) {
                        self.ready.insert(id, (Arc::new(stand), keys));
                    }
                }
            }
        }
        // Apply, in sorted node order, under the budget.
        let ta = Instant::now();
        let mut applied = 0usize;
        let ids: Vec<NodeId> = self.ready.keys().copied().collect();
        if settle {
            let keys: BTreeSet<(u8, u32, u8)> = ids.iter().flat_map(|id| self.ready[id].1.iter().copied()).collect();
            self.ensure_bank(r, dev, &keys)?;
        } else {
            // Bank uploads (mesh and impostor textures): one entry at a time until the budget
            // is spent, for the entries the ready nodes need, in node order.
            let mut left = if self.atlas.is_none() && self.atlas_cpu.is_none() { 0 } else { self.bank_keys };
            'up: for id in &ids {
                for k in self.ready[id].1.clone() {
                    if self.bank.contains_key(&k) || !self.prep.contains_key(&k) {
                        continue;
                    }
                    if left == 0 || (left < self.bank_keys && !ms_left(&ta, self.apply_ms)) {
                        break 'up;
                    }
                    self.ensure_bank(r, dev, &BTreeSet::from([k]))?;
                    left -= 1;
                }
            }
        }
        let tp = Instant::now();
        let mut ids = ids.into_iter();
        loop {
            if !settle && !ms_left(&tp, self.apply_ms) {
                break;
            }
            if self.building.is_none() {
                if !settle && applied >= self.apply_nodes {
                    break;
                }
                // The next ready node (sorted) whose bank entries are all uploaded.
                let next = ids.by_ref().find(|id| settle || self.ready[id].1.iter().all(|k| self.bank.contains_key(k)));
                let Some(id) = next else { break };
                let (stand, _) = self.ready.remove(&id).expect("ready");
                self.requested.remove(&id);
                self.begin_build(wd, r, id, stand)?;
                applied += 1;
            }
            if self.step_build(wd, r, dev, camera, ppr, if settle { usize::MAX } else { SLICE })? {
                self.building = None;
            }
        }
        self.stats.apply_ms += t_start.elapsed().as_secs_f64() * 1e3;
        self.stats.applied_last = applied;
        self.stats.apply_last_ms = ta.elapsed().as_secs_f64() * 1e3;
        self.stats.pending = self.requested.len() + self.building.is_some() as usize;
        self.stats.instances = self.nodes.values().map(|e| e.instances.len()).sum();
        self.stats.imp_trees = self.nodes.values().filter(|e| e.imp.is_some()).map(|e| e.trees.len()).sum();
        self.stats.nodes = self.nodes.len();
        Ok(())
    }

    /// Starts applying node `id`: its renderer slot and an empty entry (the rest comes in slices).
    fn begin_build(&mut self, wd: &TerrainWorld, r: &mut Renderer, id: NodeId, stand: Arc<Vec<StandTree>>) -> Result<(), String> {
        let body = &wd.anchor.body;
        let cd = id.centre().normalize();
        let gh = self.ground_at(wd, cd).unwrap_or(0.0);
        let origin = wd.anchor.body_to_render(body.point(cd, gh));
        let slot = match self.free_slots.pop() {
            Some(s) => {
                r.instances().set_node(s, origin);
                s
            }
            None => r.instances().add_node(origin).ok_or("renderer node table full")?,
        };
        let n = stand.len();
        self.nodes.insert(id, NodeEntry { slot, origin, instances: Vec::new(), trees: Vec::with_capacity(n), imp: None, meshed: false, building: true, stand: stand.clone() });
        self.building = Some(Build { id, stand, gh, pos: 0, placed: Vec::with_capacity(n), imps: Vec::new(), phase: 0 });
        Ok(())
    }

    /// Advances the node being built by about `budget` trees; true when it is complete.
    fn step_build(&mut self, wd: &TerrainWorld, r: &mut Renderer, dev: &dyn ed_gpu::Device, camera: &Camera, ppr: f64, budget: usize) -> Result<bool, String> {
        let mut b = self.building.take().expect("building");
        let (slot, origin) = {
            let e = &self.nodes[&b.id];
            (e.slot, e.origin)
        };
        let end = (b.pos + budget).min(b.stand.len());
        match b.phase {
            0 => {
                let mut recs = Vec::new();
                for tr in &b.stand[b.pos..end] {
                    let p = bank::instance_params(tr);
                    let Some(&(_, _, bank_h, kind)) = self.bank.get(&(tr.species, p.variant, p.age)) else {
                        b.placed.push(None);
                        continue;
                    };
                    let (base, up, h) = self.place_h(wd, tr, b.gh);
                    let rel = (base - origin).as_vec3();
                    let season = self.season_word(tr, h);
                    b.placed.push(Some((rel, up, season)));
                    recs.push((rel, p.scale * bank_h));
                    if self.imp {
                        b.imps.push(ImpostorTree {
                            pos: rel,
                            kind,
                            hash: (tr.seed.wrapping_mul(0x9E37_79B1) >> 24) as u8,
                            scale: p.scale,
                            up,
                            yaw: p.yaw,
                            widen: p.widen,
                            user: p.user,
                            season,
                        });
                    }
                }
                self.nodes.get_mut(&b.id).expect("building").trees.extend(recs);
                b.pos = end;
                if b.pos >= b.stand.len() {
                    b.phase = 1;
                }
            }
            1 => {
                let imp = if self.imp && !b.imps.is_empty() {
                    Some(r.add_impostor_trees(dev, slot, &b.imps).map_err(|e| format!("impostors: {e}"))?.ok_or("impostor buffer full")?)
                } else {
                    None
                };
                b.imps = Vec::new();
                let mut e = self.nodes.remove(&b.id).expect("building");
                e.imp = imp;
                e.meshed = self.wants_mesh(&e, camera, ppr);
                let meshed = e.meshed;
                self.nodes.insert(b.id, e);
                b.pos = 0;
                b.phase = if meshed { 2 } else { 3 };
            }
            2 => {
                let mut out = Vec::new();
                for (tr, pl) in b.stand[b.pos..end].iter().zip(&b.placed[b.pos..end]) {
                    let Some((rel, up, season)) = pl else { continue };
                    let p = bank::instance_params(tr);
                    let Some(&(mesh, material, _, _)) = self.bank.get(&(tr.species, p.variant, p.age)) else { continue };
                    let s = p.scale;
                    let rot = Quat::from_rotation_arc(Vec3::Y, *up);
                    let lean_axis = Vec3::new(p.lean_az.cos(), 0.0, p.lean_az.sin());
                    let rot = rot * Quat::from_axis_angle(lean_axis, p.lean) * Quat::from_rotation_y(p.yaw);
                    let xf = Affine3A::from_scale_rotation_translation(Vec3::new(s * p.widen, s, s * p.widen), rot, *rel);
                    let Some(i) = r.instances().add(&InstanceDesc { node: slot, transform: xf, mesh, material, object: tr.id, flags: 0 }) else {
                        return Err("instance pool full".into());
                    };
                    r.instances().set_user(i, p.user);
                    r.instances().set_season(i, *season);
                    out.push(i);
                }
                self.nodes.get_mut(&b.id).expect("building").instances.extend(out);
                b.pos = end;
                if b.pos >= b.stand.len() {
                    b.phase = 3;
                }
            }
            _ => {}
        }
        if b.phase == 3 {
            self.nodes.get_mut(&b.id).expect("building").building = false;
            return Ok(true);
        }
        self.building = Some(b);
        Ok(false)
    }

    /// Ground height along body direction `d`: resident terrain pages, else the stand's page.
    fn ground_at(&self, wd: &TerrainWorld, d: DVec3) -> Option<f64> {
        if let Some(h) = wd.planet.height(wd.residency(), wd.anchor.body.id, d * wd.anchor.body.point(d, 0.0).length()) {
            return Some(h.0);
        }
        let id = NodeId::from_unit(wd.anchor.body.id, d, PAGE_LEVEL);
        let hp = self.pages.lock().unwrap().get(&id)?.as_ref()?.0.clone();
        let (u, v) = id.local_uv(d);
        Some(hp.sample(u, v))
    }

    /// Trees per tier for `camera` (CPU estimate from each tree's projected height, the same
    /// rule the GPU follows): [T0 full, T1 simplified, T2 shell or impostor, handed off].
    pub fn tiers(&self, camera: &Camera, viewport: [u32; 2]) -> [usize; 4] {
        let ppr = viewport[1] as f64 * 0.5 / (camera.fov_y as f64 * 0.5).tan();
        let out_px = if self.imp { (bank::IMP_OUT_PX.0 + bank::IMP_OUT_PX.1) as f64 * 0.5 } else { bank::HANDOFF_PX as f64 };
        let mut out = [0usize; 4];
        for e in self.nodes.values() {
            for &(p, h) in &e.trees {
                let dist = (e.origin + p.as_dvec3() - camera.position).length().max(1.0);
                let px = h as f64 * ppr / dist;
                let tier = if px > bank::T0_PX as f64 {
                    0
                } else if px > if self.imp { self.imp_t1 } else { bank::T1_PX } as f64 {
                    1
                } else if bank::TREE_REF_M as f64 * ppr / dist > out_px {
                    2
                } else {
                    3
                };
                out[tier] += 1;
            }
        }
        out
    }
}

/// Disk-cache key of a bank entry's impostor: the bake version, tier and a hash of what the
/// bake reads (the simplified level's geometry and words), so a changed bank never reuses an
/// old bake.
fn imp_cache_key(key: (u8, u32, u8), m: &bank::BankMesh, tier: ImpostorTier) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |w: u32| {
        for b in w.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x100_0000_01b3);
        }
    };
    for &i in &m.lods[1].0 {
        let p = m.positions[i as usize];
        eat(i);
        eat(p[0].to_bits());
        eat(p[1].to_bits());
        eat(p[2].to_bits());
        eat(m.words[i as usize][0]);
        eat(m.words[i as usize][1]);
    }
    format!("impostors/v{}/{}_{}_{}_{}_{:016x}.bin", bank_impostor::BAKE_VERSION, key.0, key.1, key.2, tier.frame_px(), h)
}

/// Runs `f(0..n)` on worker threads (contiguous chunks); callers store results by index, so the
/// outcome does not depend on thread timing.
fn par_for(cap: usize, n: usize, f: impl Fn(usize) + Sync) {
    let threads = std::thread::available_parallelism().map_or(8, |t| t.get()).min(16).min(cap.max(1)).min(n.max(1));
    let chunk = n.div_ceil(threads);
    std::thread::scope(|s| {
        for c in 0..threads {
            let f = &f;
            s.spawn(move || {
                for k in c * chunk..((c + 1) * chunk).min(n) {
                    f(k);
                }
            });
        }
    });
}

/// Lowest point of the crown (m, tree space): the lowest vertex well off the trunk axis.
fn crown_base(m: &bank::BankMesh) -> f32 {
    let r = 0.35 * m.crown_radius;
    let y = m.positions.iter().filter(|p| p[0] * p[0] + p[2] * p[2] > r * r).map(|p| p[1]).fold(f32::INFINITY, f32::min);
    if y.is_finite() { y.clamp(0.0, m.height) } else { 0.3 * m.height }
}
