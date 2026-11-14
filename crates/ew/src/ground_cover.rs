//! Ground cover to 150 m (trees G7): `ed_world::ground_cover` items drawn as foliage instances,
//! owned by the near trees (it needs their canopy pages and stands: trunks to keep clear of and
//! old trees to drop logs by). `EW_GROUND_COVER=0` turns it off.
//!
//! Level-17 nodes (~76 m) around the camera, all around near it and in the view cone farther
//! out. A node is generated on worker threads once the tree nodes around it are applied, at a
//! density tier from its nearest distance (the shader thins the rest by rank, so a tier is only
//! an upper bound); a camera coming closer adds the missing ranks, nothing is rebuilt.
//! Placement on the terrain runs on the main thread under a per-frame budget (`EW_GC_APPLY_MS`).
//! `EW_GC_BAND=lo,hi` keeps only items whose distance is inside the band (measurement).

use crate::terrain::TerrainWorld;
use ed_geo::{CanopyPage, HeightPage, NodeId, ObjectId};
use ed_render::scene::{GROUND_COVER_FLAG, InstanceDesc, InstanceId, Material, MaterialId, MeshId, NodeSlot};
use ed_render::{Camera, Renderer};
use ed_world::ground_cover::{self as gc, CoverItem, GroundSample, Trunk};
use ed_world::stand::{CanopySample, StandTree};
use glam::{Affine3A, DVec3, Quat, Vec3};
use std::collections::BTreeMap;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Instant;

const SEED: u64 = 0x6C07;
/// Pages the samples come from (the near trees' canopy pages).
const PAGE_LEVEL: u8 = 12;
/// Node range: the solid items' reach (logs and boulders; the plants end at `gc::REACH_M`).
const RANGE_M: f64 = 140.0;
/// Nodes this near are kept all around.
const NEAR_M: f64 = 60.0;
/// Camera slack (m) when choosing a node's tier.
const SLACK_M: f64 = 12.0;
/// Density tiers (keep thresholds).
const TIERS: [f32; 8] = [1.0, 0.5, 0.25, 0.12, 0.06, 0.03, 0.015, 0.0];
/// Simplified level below this many pixels of item radius.
const LOD1_PX: f32 = 7.0;

/// Pages snapshot for one job.
pub(crate) type PageSet = BTreeMap<NodeId, (Arc<HeightPage>, Arc<CanopyPage>)>;

struct Job {
    id: NodeId,
    keep: f32,
    pages: PageSet,
    stands: Vec<Arc<Vec<StandTree>>>,
}

struct Done {
    id: NodeId,
    keep: f32,
    items: Vec<CoverItem>,
    ms: f64,
}

struct Entry {
    slot: NodeSlot,
    origin: DVec3,
    keep: f32,
    instances: Vec<InstanceId>,
}

/// Placement in progress (sliced).
struct Placing {
    id: NodeId,
    keep: f32,
    /// The node's previous keep (items under it already exist), `None` for a new node.
    prev: Option<f32>,
    items: Vec<CoverItem>,
    pos: usize,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct GroundStats {
    pub nodes: usize,
    pub instances: usize,
    pub pending: usize,
    pub protos: usize,
    pub gen_ms: f64,
    pub gen_nodes: usize,
    pub place_ms: f64,
    /// Main-thread ms of the last update.
    pub last_ms: f64,
    /// Items per kind (resident).
    pub by_pass: [usize; 3],
}

pub struct GroundCover {
    jobs: Sender<Job>,
    done: Receiver<Done>,
    nodes: BTreeMap<NodeId, Entry>,
    /// Requested (keep) and not yet applied.
    requested: BTreeMap<NodeId, f32>,
    ready: BTreeMap<NodeId, Done>,
    placing: Option<Placing>,
    protos: Vec<Option<(MeshId, MaterialId)>>,
    free_slots: Vec<NodeSlot>,
    apply_ms: f64,
    band: Option<(f64, f64)>,
    /// Grass style A draws the grass: the tufts (grass, turf, dry and shade grass) stay out.
    pub no_grass: bool,
    pub stats: GroundStats,
}

/// `EW_GROUND_COVER` (default on).
pub fn from_env() -> bool {
    std::env::var("EW_GROUND_COVER").map_or(true, |v| v != "0")
}

pub(crate) fn sampler(pages: &PageSet) -> impl Fn(DVec3) -> Option<GroundSample> + '_ {
    move |d: DVec3| {
        let body = pages.keys().next()?.body();
        let id = NodeId::from_unit(body, d, PAGE_LEVEL);
        let (hp, cp) = pages.get(&id)?;
        let (u, v) = id.local_uv(d);
        let page_m = (id.corner(1) - id.corner(0)).length() * 6_371_008.8;
        let du = 8.0 / page_m;
        let h = |u: f64, v: f64| hp.sample(u.clamp(0.0, 1.0), v.clamp(0.0, 1.0));
        let alt = h(u, v);
        let gx = (h(u + du, v) - h(u - du, v)) / 16.0;
        let gy = (h(u, v + du) - h(u, v - du)) / 16.0;
        let slope_cos = (1.0 / (1.0 + gx * gx + gy * gy).sqrt()) as f32;
        Some(GroundSample { canopy: CanopySample::from_page(cp, u, v, alt as f32, 0.3), slope_cos })
    }
}

fn run_job(j: Job) -> Done {
    let t = Instant::now();
    let stands: Vec<&[StandTree]> = j.stands.iter().map(|s| s.as_slice()).collect();
    let trunks: Vec<Trunk> = gc::trunks_near(j.id, &stands, 30.0);
    let sample = sampler(&j.pages);
    let items = gc::scatter_node(j.id, &sample, &trunks, j.keep, SEED);
    Done { id: j.id, keep: j.keep, items, ms: t.elapsed().as_secs_f64() * 1e3 }
}

fn spawn_workers(n: usize) -> (Sender<Job>, Receiver<Done>) {
    let (jt, jr) = channel::<Job>();
    let (dt, dr) = channel::<Done>();
    let jr = Arc::new(Mutex::new(jr));
    for _ in 0..n {
        let (jr, dt) = (jr.clone(), dt.clone());
        // Unnamed threads: the named-thread API trips the `ew` no-procedural-geometry lint.
        std::thread::spawn(move || {
            loop {
                let j = { jr.lock().unwrap().recv() };
                let Ok(j) = j else { return };
                if dt.send(run_job(j)).is_err() {
                    return;
                }
            }
        });
    }
    (jt, dr)
}

/// The tier keep for a node whose nearest point is `d` m away.
fn tier_for(d: f64) -> f32 {
    let need = gc::keep((d - SLACK_M).max(0.0) as f32, gc::REACH_M);
    // Never the empty tier inside the range: solid items (rank 0) still need a node.
    TIERS.iter().rev().copied().find(|t| *t >= need && *t > 0.0).unwrap_or(1.0)
}

/// What the ground cover needs from the near trees each update.
pub(crate) struct TreeView<'a> {
    /// Ground height (m) along a body direction (the trees' `ground_at`).
    pub ground: &'a dyn Fn(DVec3) -> Option<f64>,
    /// Applied stand of a level-14 tree node (`None`: not applied yet).
    pub stand: &'a dyn Fn(NodeId) -> Option<Arc<Vec<StandTree>>>,
    /// Resident page of a level-12 node.
    pub page: &'a dyn Fn(NodeId) -> Option<(Arc<HeightPage>, Arc<CanopyPage>)>,
    pub tree_level: u8,
}

impl GroundCover {
    pub fn new() -> GroundCover {
        let threads = std::env::var("EW_GC_THREADS").ok().and_then(|v| v.parse().ok()).unwrap_or((std::thread::available_parallelism().map_or(8, |t| t.get()) / 4).max(1));
        let (jobs, done) = spawn_workers(threads);
        let band = std::env::var("EW_GC_BAND").ok().and_then(|v| {
            let (a, b) = v.split_once(',')?;
            Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
        });
        GroundCover {
            jobs,
            done,
            nodes: BTreeMap::new(),
            requested: BTreeMap::new(),
            ready: BTreeMap::new(),
            placing: None,
            protos: vec![None; gc::library().len()],
            free_slots: Vec::new(),
            apply_ms: std::env::var("EW_GC_APPLY_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0),
            band,
            no_grass: crate::grass::style().is_some(),
            stats: GroundStats::default(),
        }
    }

    /// Wanted nodes and their tier keep.
    fn wanted(&self, wd: &TerrainWorld, tv: &TreeView, camera: &Camera, viewport: [u32; 2]) -> BTreeMap<NodeId, f32> {
        let body = &wd.anchor.body;
        let cd = wd.anchor.render_to_body(camera.position).normalize();
        let c0 = NodeId::from_unit(body.id, cd, gc::NODE_LEVEL);
        let gh = (tv.ground)(cd).unwrap_or(0.0);
        let size = (body.point(c0.neighbour(1, 0).centre().normalize(), gh) - body.point(c0.centre().normalize(), gh)).length();
        let half = size * 0.75;
        let n = (RANGE_M / size).ceil() as i32 + 1;
        let fwd = camera.forward().as_dvec3();
        let aspect = viewport[0] as f64 / viewport[1].max(1) as f64;
        let half_diag = ((camera.fov_y as f64 * 0.5).tan() * (1.0 + aspect * aspect).sqrt()).atan();
        let mut out = BTreeMap::new();
        for dy in -n..=n {
            for dx in -n..=n {
                let id = c0.neighbour(dx, dy);
                let d = id.centre().normalize();
                let p = wd.anchor.body_to_render(body.point(d, (tv.ground)(d).unwrap_or(gh)));
                let to = p - camera.position;
                let dist = to.length();
                let dmin = (dist - half).max(0.0);
                if dmin > RANGE_M {
                    continue;
                }
                let near = dmin < NEAR_M;
                let cone = to.normalize_or_zero().dot(fwd) > (half_diag + (half / dist.max(1.0)).atan()).min(std::f64::consts::PI).cos();
                if near || cone {
                    out.insert(id, tier_for(dmin));
                }
            }
        }
        out
    }

    /// Uploads library mesh `k` (lazily: only kinds that grow here).
    fn proto(&mut self, r: &mut Renderer, dev: &dyn ed_gpu::Device, k: usize, ppr: f32) -> Result<(MeshId, MaterialId), String> {
        if let Some(p) = self.protos[k] {
            return Ok(p);
        }
        let m = &gc::library()[k];
        let d = gc::def(m.kind.expect("kind"));
        // Coarser where the item's radius is under LOD1_PX; nothing past its reach (the
        // smallest scale; the shader has thinned the rest to nothing there).
        let e2 = d.reach * 1.02 / (ppr * d.scale.0.max(0.05));
        let e1 = (m.radius / LOD1_PX).min(e2 * 0.8);
        let lods = vec![(m.lod0.clone(), 0.0), (m.lod1.clone(), e1), (Vec::new(), e2)];
        let mesh = r.add_foliage_mesh(dev, &m.positions, &m.normals, &m.words, &lods).map_err(|e| format!("ground cover mesh {}: {e}", d.name))?;
        let mut mat = Material::pbr(Vec3::from(d.leaf), 0.0, 0.75);
        mat.shade_color = Vec3::from(d.shade);
        let mat = r.add_material(mat).ok_or("material table full")?;
        self.protos[k] = Some((mesh, mat));
        self.stats.protos += 1;
        Ok((mesh, mat))
    }

    fn release(&mut self, r: &mut Renderer, id: NodeId) {
        if let Some(e) = self.nodes.remove(&id) {
            for i in e.instances {
                r.instances().remove(i);
            }
            self.free_slots.push(e.slot);
        }
    }

    pub(crate) fn update(&mut self, wd: &TerrainWorld, r: &mut Renderer, dev: &dyn ed_gpu::Device, camera: &Camera, viewport: [u32; 2], settle: bool, tv: &TreeView) -> Result<(), String> {
        let t0 = Instant::now();
        let ppr = (viewport[1] as f64 * 0.5 / (camera.fov_y as f64 * 0.5).tan()) as f32;
        let wanted = self.wanted(wd, tv, camera, viewport);
        // Leave, and drop far more density than needed (the camera moved away).
        let gone: Vec<NodeId> = self.nodes.iter().filter(|(id, e)| wanted.get(id).is_none_or(|k| *k * 8.0 < e.keep && !settle)).map(|(id, _)| *id).collect();
        for id in gone {
            self.release(r, id);
        }
        self.requested.retain(|id, _| wanted.contains_key(id));
        self.ready.retain(|id, _| wanted.contains_key(id));
        if self.placing.as_ref().is_some_and(|p| !wanted.contains_key(&p.id)) {
            self.placing = None;
        }
        // Request: nodes whose tree neighbourhood is applied, missing or needing more density.
        let mut new: Vec<(f64, NodeId, f32)> = Vec::new();
        for (&id, &keep) in &wanted {
            let have = self.nodes.get(&id).map(|e| e.keep).or(self.placing.as_ref().filter(|p| p.id == id).map(|p| p.keep));
            let asked = self.requested.get(&id).copied();
            if have.is_some_and(|k| k >= keep) || asked.is_some_and(|k| k >= keep) {
                continue;
            }
            let eye = wd.anchor.render_to_body(camera.position).normalize();
            new.push((id.centre().normalize().angle_between(eye), id, keep));
        }
        new.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        for (_, id, keep) in new {
            let tn = id.ancestor(tv.tree_level);
            let mut stands = Vec::with_capacity(9);
            let mut ok = true;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    match (tv.stand)(tn.neighbour(dx, dy)) {
                        Some(s) => stands.push(s),
                        None => ok = false,
                    }
                }
            }
            if !ok {
                continue;
            }
            let pn = id.ancestor(PAGE_LEVEL);
            let mut pages = PageSet::new();
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let p = pn.neighbour(dx, dy);
                    if let Some(pg) = (tv.page)(p) {
                        pages.insert(p, pg);
                    }
                }
            }
            if !pages.contains_key(&pn) {
                continue;
            }
            self.requested.insert(id, keep);
            self.jobs.send(Job { id, keep, pages, stands }).map_err(|_| "ground-cover worker stopped".to_string())?;
        }
        // Collect.
        loop {
            let d = match self.done.try_recv() {
                Ok(d) => Some(d),
                Err(std::sync::mpsc::TryRecvError::Empty) if settle && self.requested.keys().any(|id| !self.ready.contains_key(id)) => Some(self.done.recv().map_err(|_| "ground-cover worker stopped".to_string())?),
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
                Err(_) => return Err("ground-cover worker stopped".into()),
            };
            let Some(d) = d else { break };
            self.stats.gen_ms += d.ms;
            self.stats.gen_nodes += 1;
            if self.requested.get(&d.id).is_some_and(|k| *k == d.keep) {
                self.ready.insert(d.id, d);
            }
        }
        // Place under the budget.
        let tp = Instant::now();
        let left = |t: &Instant, ms: f64| settle || t.elapsed().as_secs_f64() * 1e3 < ms;
        while left(&tp, self.apply_ms) {
            if self.placing.is_none() {
                let Some(id) = self.ready.keys().next().copied() else { break };
                let d = self.ready.remove(&id).expect("ready");
                self.requested.remove(&id);
                let prev = self.nodes.get(&id).map(|e| e.keep);
                self.placing = Some(Placing { id, keep: d.keep, prev, items: d.items, pos: 0 });
            }
            if self.place_some(wd, r, dev, camera, ppr, tv, if settle { usize::MAX } else { 512 })? {
                self.placing = None;
            }
        }
        self.stats.place_ms += tp.elapsed().as_secs_f64() * 1e3;
        self.stats.nodes = self.nodes.len();
        self.stats.instances = self.nodes.values().map(|e| e.instances.len()).sum();
        self.stats.pending = self.requested.len() + self.placing.is_some() as usize;
        self.stats.last_ms = t0.elapsed().as_secs_f64() * 1e3;
        Ok(())
    }

    /// Places up to `budget` items of the node in progress; true when it is done.
    #[allow(clippy::too_many_arguments)]
    fn place_some(&mut self, wd: &TerrainWorld, r: &mut Renderer, dev: &dyn ed_gpu::Device, camera: &Camera, ppr: f32, tv: &TreeView, budget: usize) -> Result<bool, String> {
        let mut p = self.placing.take().expect("placing");
        let body = &wd.anchor.body;
        if !self.nodes.contains_key(&p.id) {
            let cd = p.id.centre().normalize();
            let gh = (tv.ground)(cd).unwrap_or(0.0);
            let origin = wd.anchor.body_to_render(body.point(cd, gh));
            let slot = match self.free_slots.pop() {
                Some(s) => {
                    r.instances().set_node(s, origin);
                    s
                }
                None => r.instances().add_node(origin).ok_or("renderer node table full")?,
            };
            self.nodes.insert(p.id, Entry { slot, origin, keep: p.keep, instances: Vec::new() });
        }
        let (slot, origin) = {
            let e = &self.nodes[&p.id];
            (e.slot, e.origin)
        };
        let end = p.pos.saturating_add(budget).min(p.items.len());
        let mut out = Vec::new();
        let season = |d: DVec3, h: f64| ed_world::season::season_word(ed_world::season::grass_row(), d.z.clamp(-1.0, 1.0).asin().to_degrees(), h);
        for it in &p.items[p.pos..end] {
            let def = gc::def(it.kind);
            if p.prev.is_some_and(|k| def.solid || it.rank < k) {
                continue;
            }
            if self.no_grass && matches!(it.kind, gc::Kind::Grass | gc::Kind::Turf | gc::Kind::DryGrass | gc::Kind::ShadeGrass) {
                continue;
            }
            let Some(h) = (tv.ground)(it.dir) else { continue };
            let base = wd.anchor.body_to_render(body.point(it.dir, h));
            if let Some((lo, hi)) = self.band {
                let d = (base - camera.position).length();
                if d < lo || d >= hi {
                    continue;
                }
            }
            let up = (wd.anchor.body_to_render(body.point(it.dir, h + 1.0)) - base).normalize();
            let mut n = up;
            if it.conform {
                // Ground normal from two samples ~0.8 m away (across the item's footprint).
                let e1 = up.cross(DVec3::Z).normalize_or(DVec3::X);
                let e2 = up.cross(e1);
                let at = |o: DVec3| -> Option<DVec3> {
                    let q = wd.anchor.render_to_body(base + o * 0.8).normalize();
                    Some(wd.anchor.body_to_render(body.point(q, (tv.ground)(q)?)))
                };
                if let (Some(a), Some(b)) = (at(e1), at(e2)) {
                    let nn = (a - base).cross(b - base).normalize_or_zero();
                    if nn.dot(up) > 0.2 {
                        n = nn;
                    } else if nn.dot(up) < -0.2 {
                        n = -nn;
                    }
                }
            }
            let k = gc::proto_index(it.kind, it.variant);
            let (mesh, material) = self.proto(r, dev, k, ppr)?;
            let s = it.scale;
            // Stones and boulders sit in the ground; blades root a little below it on slopes.
            let sink = match it.kind {
                gc::Kind::Boulder | gc::Kind::Stone | gc::Kind::Pebble => 0.10 * s as f64,
                gc::Kind::Log | gc::Kind::Moss => 0.02,
                _ => 0.03 + 0.15 * (1.0 - n.dot(up)).max(0.0),
            };
            let pos = (base - n * sink - origin).as_vec3();
            let rot = Quat::from_rotation_arc(Vec3::Y, n.as_vec3()) * Quat::from_rotation_y(it.yaw);
            let xf = Affine3A::from_scale_rotation_translation(Vec3::splat(s), rot, pos);
            let Some(i) = r.instances().add(&InstanceDesc { node: slot, transform: xf, mesh, material, object: ObjectId(0), flags: GROUND_COVER_FLAG }) else {
                return Err("instance pool full".into());
            };
            r.instances().set_user(i, gc::user_word(it));
            r.instances().set_season(i, season(it.dir, h));
            r.instances().set_ground(i, gc::ground_word(it));
            self.stats.by_pass[(def.pass as usize).clamp(1, 3) - 1] += 1;
            out.push(i);
        }
        let e = self.nodes.get_mut(&p.id).expect("placing");
        e.instances.extend(out);
        e.keep = e.keep.max(p.keep);
        p.pos = end;
        if p.pos >= p.items.len() {
            return Ok(true);
        }
        self.placing = Some(p);
        Ok(false)
    }

    /// Releases everything (the near trees went away).
    pub fn clear(&mut self, r: &mut Renderer) {
        let ids: Vec<NodeId> = self.nodes.keys().copied().collect();
        for id in ids {
            self.release(r, id);
        }
        self.requested.clear();
        self.ready.clear();
        self.placing = None;
    }
}

impl Default for GroundCover {
    fn default() -> Self {
        Self::new()
    }
}
