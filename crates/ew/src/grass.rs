//! Grass style A (`EW_GRASS_STYLE=a`): `ed_world::grass` clumps drawn by the renderer's
//! procedural blade path (`ed_render::grass`). Owned by the near trees beside the ground cover
//! (it reads the same pages and ground heights); with style A on, the ground cover leaves out
//! its grass tufts.
//!
//! Level-17 nodes around the camera, all around near it and in the view cone farther out, out
//! to the grass range. A node is generated on worker threads at a keep tier from its nearest
//! distance; a camera coming closer asks for a denser tier and the node's block is replaced.
//! On the main thread a node's clumps are set on a ~2 m height grid of the ground (bilinear,
//! normal from the cell) under a per-frame budget (`EW_GRASS_APPLY_MS`).
//!
//! Knobs: `EW_GRASS_R0` (full-density radius, m), `EW_GRASS_RANGE` (m), `EW_GRASS_BMIN`
//! (fewest blades per clump), `EW_GRASS_WIDTH` (most widening), `EW_GRASS_UP` (shading normal
//! blend to the ground), `EW_GRASS_SHADOWS=0`, `EW_GRASS_WIND=0`.

use crate::ground_cover::{PageSet, TreeView};
use crate::terrain::TerrainWorld;
use ed_geo::NodeId;
use ed_render::grass::{GrassClump, GrassConfig, GrassRange};
use ed_render::scene::NodeSlot;
use ed_render::{Camera, Renderer};
use ed_world::grass::{self as gr, Clump};
use glam::DVec3;
use std::collections::BTreeMap;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Instant;

const SEED: u64 = 0x6A55_0A;
const PAGE_LEVEL: u8 = 12;
/// Nodes this close are wanted all around the camera (m).
const NEAR_M: f64 = 30.0;
/// Distance slack for the keep tier (the camera moves before the node lands).
const SLACK_M: f64 = 8.0;
const TIERS: [f32; 7] = [1.0, 0.5, 0.25, 0.12, 0.06, 0.03, 0.015];
/// Height grid step (m).
const GRID_M: f32 = 2.0;

/// `EW_GRASS_STYLE=a`: style A on (and the ground cover's grass tufts off).
pub fn style_a() -> bool {
    std::env::var("EW_GRASS_STYLE").is_ok_and(|v| v.eq_ignore_ascii_case("a"))
}

fn env_f32(k: &str, d: f32) -> f32 {
    std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d)
}

/// The renderer config from the environment.
pub fn config_from_env() -> GrassConfig {
    let d = GrassConfig::default();
    GrassConfig {
        r0: env_f32("EW_GRASS_R0", gr::R0_M).max(1.0),
        range: env_f32("EW_GRASS_RANGE", gr::RANGE_M).max(5.0),
        bmin: env_f32("EW_GRASS_BMIN", gr::BMIN).clamp(1.0, 64.0),
        width_max: env_f32("EW_GRASS_WIDTH", d.width_max).max(1.0),
        shadows: std::env::var("EW_GRASS_SHADOWS").map_or(true, |v| v != "0"),
        up_blend: env_f32("EW_GRASS_UP", d.up_blend).clamp(0.0, 1.0),
        wind: std::env::var("EW_GRASS_WIND").map_or(true, |v| v != "0"),
    }
}

struct Job {
    id: NodeId,
    keep: f32,
    lab: bool,
    pages: PageSet,
}

struct Done {
    id: NodeId,
    keep: f32,
    clumps: Vec<Clump>,
    ms: f64,
}

struct Entry {
    slot: NodeSlot,
    keep: f32,
    range: Option<GrassRange>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct GrassStats {
    pub nodes: usize,
    pub clumps: usize,
    pub pending: usize,
    pub gen_ms: f64,
    pub gen_nodes: usize,
    pub place_ms: f64,
    /// Main-thread ms of the last update.
    pub last_ms: f64,
}

pub struct Grass {
    jobs: Sender<Job>,
    done: Receiver<Done>,
    nodes: BTreeMap<NodeId, Entry>,
    requested: BTreeMap<NodeId, f32>,
    ready: BTreeMap<NodeId, Done>,
    free_slots: Vec<NodeSlot>,
    config: GrassConfig,
    uploaded: bool,
    apply_ms: f64,
    pub stats: GrassStats,
}

/// The grass lab's type map from a body direction (metres east and north of the anchor, in its
/// tangent plane: the body is an ellipsoid, so geocentric latitude is not the place's).
fn lab_at(d: DVec3) -> Option<(u8, f32)> {
    let lab = &crate::terrain::GRASSLAB;
    let a = crate::terrain::geodetic_dir(lab.lat, lab.lon);
    let east = DVec3::Z.cross(a).normalize();
    let north = a.cross(east);
    let r = 6_367_000.0;
    gr::lab_type((d - a).dot(east) * r, (d - a).dot(north) * r)
}

fn run_job(j: Job) -> Done {
    let t = Instant::now();
    let clumps = if j.lab {
        gr::scatter_node(j.id, &lab_at, j.keep, SEED)
    } else {
        let sample = crate::ground_cover::sampler(&j.pages);
        let cl = ed_world::ground_cover::Climate::at(j.id.centre().normalize());
        let at = |d: DVec3| sample(d).and_then(|g| gr::type_from_ground(&g, &cl));
        gr::scatter_node(j.id, &at, j.keep, SEED)
    };
    Done { id: j.id, keep: j.keep, clumps, ms: t.elapsed().as_secs_f64() * 1e3 }
}

fn spawn_workers(n: usize) -> (Sender<Job>, Receiver<Done>) {
    let (jt, jr) = channel::<Job>();
    let (dt, dr) = channel::<Done>();
    let jr = Arc::new(Mutex::new(jr));
    for _ in 0..n {
        let (jr, dt) = (jr.clone(), dt.clone());
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

impl Grass {
    pub fn new() -> Grass {
        let threads = std::env::var("EW_GRASS_THREADS").ok().and_then(|v| v.parse().ok()).unwrap_or((std::thread::available_parallelism().map_or(8, |t| t.get()) / 4).max(1));
        let (jobs, done) = spawn_workers(threads);
        Grass {
            jobs,
            done,
            nodes: BTreeMap::new(),
            requested: BTreeMap::new(),
            ready: BTreeMap::new(),
            free_slots: Vec::new(),
            config: config_from_env(),
            uploaded: false,
            apply_ms: std::env::var("EW_GRASS_APPLY_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(1.5),
            stats: GrassStats::default(),
        }
    }

    /// The keep tier for a node whose nearest point is `d` m away.
    fn tier_for(&self, d: f64) -> f32 {
        let need = gr::clump_keep((d - SLACK_M).max(0.0) as f32, self.config.r0);
        TIERS.iter().rev().copied().find(|t| *t >= need).unwrap_or(1.0)
    }

    /// Wanted nodes and their keep tier.
    fn wanted(&self, wd: &TerrainWorld, tv: &TreeView, camera: &Camera, viewport: [u32; 2]) -> BTreeMap<NodeId, f32> {
        let body = &wd.anchor.body;
        let range = self.config.range as f64;
        let cd = wd.anchor.render_to_body(camera.position).normalize();
        let c0 = NodeId::from_unit(body.id, cd, gr::NODE_LEVEL);
        let gh = (tv.ground)(cd).unwrap_or(0.0);
        let size = (body.point(c0.neighbour(1, 0).centre().normalize(), gh) - body.point(c0.centre().normalize(), gh)).length();
        let half = size * 0.75;
        let n = (range / size).ceil() as i32 + 1;
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
                if dmin > range {
                    continue;
                }
                let cone = to.normalize_or_zero().dot(fwd) > (half_diag + (half / dist.max(1.0)).atan()).min(std::f64::consts::PI).cos();
                if dmin < NEAR_M || cone {
                    out.insert(id, self.tier_for(dmin));
                }
            }
        }
        out
    }

    fn release(&mut self, r: &mut Renderer, dev: &dyn ed_gpu::Device, id: NodeId) -> Result<(), String> {
        if let Some(e) = self.nodes.remove(&id) {
            if let Some(g) = e.range {
                r.remove_grass_clumps(dev, g).map_err(|e| e.to_string())?;
            }
            self.free_slots.push(e.slot);
        }
        Ok(())
    }

    pub(crate) fn update(&mut self, wd: &TerrainWorld, r: &mut Renderer, dev: &dyn ed_gpu::Device, camera: &Camera, viewport: [u32; 2], settle: bool, tv: &TreeView) -> Result<(), String> {
        let t0 = Instant::now();
        if !self.uploaded {
            r.set_grass_types(dev, &gr::gpu_rows()).map_err(|e| e.to_string())?;
            r.set_grass_config(Some(self.config));
            self.uploaded = true;
        }
        let wanted = self.wanted(wd, tv, camera, viewport);
        let gone: Vec<NodeId> = self.nodes.iter().filter(|(id, e)| wanted.get(id).is_none_or(|k| *k * 8.0 < e.keep && !settle)).map(|(id, _)| *id).collect();
        for id in gone {
            self.release(r, dev, id)?;
        }
        self.requested.retain(|id, _| wanted.contains_key(id));
        self.ready.retain(|id, _| wanted.contains_key(id));
        // Request: missing nodes and nodes needing a denser tier, nearest first.
        let eye = wd.anchor.render_to_body(camera.position).normalize();
        let mut new: Vec<(f64, NodeId, f32)> = Vec::new();
        for (&id, &keep) in &wanted {
            let have = self.nodes.get(&id).map(|e| e.keep);
            let asked = self.requested.get(&id).copied().or(self.ready.get(&id).map(|d| d.keep));
            if have.is_some_and(|k| k >= keep) || asked.is_some_and(|k| k >= keep) {
                continue;
            }
            new.push((id.centre().normalize().angle_between(eye), id, keep));
        }
        new.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        for (_, id, keep) in new {
            let mut pages = PageSet::new();
            if !wd.grass_lab {
                let pn = id.ancestor(PAGE_LEVEL);
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
            }
            self.requested.insert(id, keep);
            self.jobs.send(Job { id, keep, lab: wd.grass_lab, pages }).map_err(|_| "grass worker stopped".to_string())?;
        }
        // Collect.
        loop {
            let d = match self.done.try_recv() {
                Ok(d) => Some(d),
                Err(std::sync::mpsc::TryRecvError::Empty) if settle && !self.requested.is_empty() => Some(self.done.recv().map_err(|_| "grass worker stopped".to_string())?),
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
                Err(_) => return Err("grass worker stopped".into()),
            };
            let Some(d) = d else { break };
            self.stats.gen_ms += d.ms;
            self.stats.gen_nodes += 1;
            if self.requested.get(&d.id).is_some_and(|k| *k == d.keep) {
                self.requested.remove(&d.id);
                self.ready.insert(d.id, d);
            }
        }
        // Place under the budget, nearest first.
        let tp = Instant::now();
        let mut order: Vec<(f64, NodeId)> = self.ready.keys().map(|id| (id.centre().normalize().angle_between(eye), *id)).collect();
        order.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        for (_, id) in order {
            if !settle && tp.elapsed().as_secs_f64() * 1e3 >= self.apply_ms {
                break;
            }
            let d = self.ready.remove(&id).expect("ready");
            if !self.place(wd, r, dev, tv, d)? {
                // Ground not resident yet: ask again later.
                continue;
            }
        }
        self.stats.place_ms += tp.elapsed().as_secs_f64() * 1e3;
        self.stats.nodes = self.nodes.len();
        self.stats.clumps = r.grass_clumps() as usize;
        self.stats.pending = self.requested.len() + self.ready.len();
        self.stats.last_ms = t0.elapsed().as_secs_f64() * 1e3;
        Ok(())
    }

    /// Sets a generated node on the ground and swaps it in. False when the ground is missing.
    fn place(&mut self, wd: &TerrainWorld, r: &mut Renderer, dev: &dyn ed_gpu::Device, tv: &TreeView, d: Done) -> Result<bool, String> {
        let body = &wd.anchor.body;
        let id = d.id;
        let cd = id.centre().normalize();
        let Some(gh) = (tv.ground)(cd) else { return Ok(false) };
        let c = body.point(cd, gh);
        let origin = wd.anchor.body_to_render(c);
        let up_ref = wd.anchor.body_to_render(body.point(cd, gh + 1.0)) - origin;
        let to_render = |v: DVec3| wd.anchor.body_to_render(c + v) - origin;
        // Height grid over the node frame (render positions of the ground).
        let (mut x0, mut x1, mut y0, mut y1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        for k in 0..4u8 {
            let p = gr::node_xy(id, id.corner(k).normalize());
            (x0, x1, y0, y1) = (x0.min(p[0]), x1.max(p[0]), y0.min(p[1]), y1.max(p[1]));
        }
        let (x0, y0) = (x0 - GRID_M, y0 - GRID_M);
        let nx = ((x1 + GRID_M - x0) / GRID_M).ceil() as usize + 1;
        let ny = ((y1 + GRID_M - y0) / GRID_M).ceil() as usize + 1;
        let mut grid: Vec<Option<DVec3>> = Vec::with_capacity(nx * ny);
        for j in 0..ny {
            for i in 0..nx {
                let dir = gr::node_dir(id, [x0 + i as f32 * GRID_M, y0 + j as f32 * GRID_M]);
                grid.push((tv.ground)(dir).map(|h| wd.anchor.body_to_render(body.point(dir, h))));
            }
        }
        if grid.iter().all(Option::is_none) {
            return Ok(false);
        }
        let mut out = Vec::with_capacity(d.clumps.len());
        for cl in &d.clumps {
            let fx = ((cl.xy[0] - x0) / GRID_M).clamp(0.0, (nx - 2) as f32);
            let fy = ((cl.xy[1] - y0) / GRID_M).clamp(0.0, (ny - 2) as f32);
            let (i, j) = ((fx.floor() as usize).min(nx - 2), (fy.floor() as usize).min(ny - 2));
            let (u, v) = ((fx - i as f32) as f64, (fy - j as f32) as f64);
            let (Some(p00), Some(p10), Some(p01), Some(p11)) = (grid[j * nx + i], grid[j * nx + i + 1], grid[(j + 1) * nx + i], grid[(j + 1) * nx + i + 1]) else { continue };
            let p = (p00 * (1.0 - u) + p10 * u) * (1.0 - v) + (p01 * (1.0 - u) + p11 * u) * v;
            let mut n = ((p10 - p00) + (p11 - p01)).cross((p01 - p00) + (p11 - p10)).normalize_or_zero();
            if n.dot(up_ref) < 0.0 {
                n = -n;
            }
            if n.dot(up_ref.normalize()) < 0.3 {
                continue;
            }
            let fa = to_render(cl.facing).normalize_or_zero();
            out.push(GrassClump {
                pos: (p - origin).as_vec3(),
                kind: cl.kind as u32,
                rank: cl.rank,
                up: n.as_vec3(),
                facing: fa.as_vec3(),
                seed: cl.seed,
                height: cl.height,
                dry: cl.dry,
            });
        }
        // New block first, then the old one goes (no gap).
        let slot = match self.nodes.get(&id) {
            Some(e) => e.slot,
            None => match self.free_slots.pop() {
                Some(s) => s,
                None => r.instances().add_node(origin).ok_or("renderer node table full")?,
            },
        };
        r.instances().set_node(slot, origin);
        let range = r.add_grass_clumps(dev, slot, &out).map_err(|e| e.to_string())?;
        if let Some(old) = self.nodes.insert(id, Entry { slot, keep: d.keep, range }).and_then(|e| e.range) {
            r.remove_grass_clumps(dev, old).map_err(|e| e.to_string())?;
        }
        Ok(true)
    }

    /// Releases everything.
    pub fn clear(&mut self, r: &mut Renderer, dev: &dyn ed_gpu::Device) -> Result<(), String> {
        let ids: Vec<NodeId> = self.nodes.keys().copied().collect();
        for id in ids {
            self.release(r, dev, id)?;
        }
        self.requested.clear();
        self.ready.clear();
        Ok(())
    }
}

impl Default for Grass {
    fn default() -> Self {
        Self::new()
    }
}
