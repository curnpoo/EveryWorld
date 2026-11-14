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
//!
//! Styles C and D (`EW_GRASS_STYLE=c|d`, [`Style`]): shell grass over the near ground and a
//! carpet tint on the terrain past it, from two type maps around the camera ([`Shells`]); D
//! keeps the blades for the tall types. Knobs: [`shell_config_from_env`].
//!
//! Style E (`EW_GRASS_STYLE=e`): blades for every type (lawn too) out to ~500 m, the Ghost of
//! Tsushima way: past `r0` blades per clump fall, the floor itself falls to 1-3 wide blades far
//! (`EW_GRASS_BFAR`, `EW_GRASS_DFAR`), kept clumps are an even lattice that spreads over its
//! cells (`EW_GRASS_SPREAD`), survivors widen (never under `EW_GRASS_PX` pixels) and grow
//! (`EW_GRASS_GROW`), and far blades take their clump's mean colour (`EW_GRASS_MEAN`). The
//! terrain tint (no shells by default, `EW_GRASS_SHELLS` for an underlayer) carries the colour
//! under and past the blades. Far blades cast no shadows (`EW_GRASS_SHADOW_M`).

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
const TIERS: [f32; 9] = [1.0, 0.5, 0.25, 0.12, 0.06, 0.03, 0.015, 0.0075, 0.004];
/// Height grid step (m).
const GRID_M: f32 = 2.0;

/// Grass styles (`EW_GRASS_STYLE`): A blades in clumps; C shells only; D blades for the tall
/// types over a shell underlayer, shells alone for lawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    A,
    C,
    D,
    E,
}

impl Style {
    pub fn parse(v: &str) -> Option<Style> {
        match v.to_ascii_lowercase().as_str() {
            "a" => Some(Style::A),
            "c" => Some(Style::C),
            "d" => Some(Style::D),
            "e" => Some(Style::E),
            _ => None,
        }
    }
    /// Draws blades (A, D, E).
    pub fn blades(self) -> bool {
        self != Style::C
    }
    /// Draws shells and the terrain tint (C, D; E the tint, shells only when asked).
    pub fn shells(self) -> bool {
        self != Style::A
    }
}

/// `EW_GRASS_STYLE`: a procedural grass style (and the ground cover's grass tufts off).
pub fn style() -> Option<Style> {
    std::env::var("EW_GRASS_STYLE").ok().and_then(|v| Style::parse(&v))
}

/// Shell knobs: `EW_GRASS_SHELLS` (shell count, 0: none), `EW_GRASS_SHELL_RANGE` (m),
/// `EW_GRASS_TINT` (terrain carpet tint strength, 0: off).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShellConfig {
    pub shells: u32,
    pub range: f32,
    pub tint: f32,
}

pub fn shell_config_from_env(style: Style) -> ShellConfig {
    let shells = if style == Style::E { 0 } else { 16 };
    ShellConfig {
        shells: std::env::var("EW_GRASS_SHELLS").ok().and_then(|v| v.parse().ok()).unwrap_or(shells).min(ed_render::terrain::SHELL_MAX),
        range: env_f32("EW_GRASS_SHELL_RANGE", 35.0).clamp(2.0, 60.0),
        tint: env_f32("EW_GRASS_TINT", 1.0).clamp(0.0, 1.0),
    }
}

fn env_f32(k: &str, d: f32) -> f32 {
    std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d)
}

/// The renderer config for `style` from the environment.
pub fn config_from_env(style: Style) -> GrassConfig {
    let d = if style == Style::E { E_DEFAULTS } else { GrassConfig::default() };
    let bmin = env_f32("EW_GRASS_BMIN", d.bmin).clamp(1.0, 64.0);
    GrassConfig {
        r0: env_f32("EW_GRASS_R0", d.r0).max(1.0),
        range: env_f32("EW_GRASS_RANGE", d.range).max(5.0),
        bmin,
        width_max: env_f32("EW_GRASS_WIDTH", d.width_max).max(1.0),
        shadows: std::env::var("EW_GRASS_SHADOWS").map_or(true, |v| v != "0"),
        up_blend: env_f32("EW_GRASS_UP", d.up_blend).clamp(0.0, 1.0),
        wind: std::env::var("EW_GRASS_WIND").map_or(true, |v| v != "0"),
        bfar: env_f32("EW_GRASS_BFAR", if style == Style::E { d.bfar } else { bmin }).clamp(1.0, bmin),
        dfar: env_f32("EW_GRASS_DFAR", d.dfar).max(0.0),
        px_min: env_f32("EW_GRASS_PX", d.px_min).max(0.0),
        spread: env_f32("EW_GRASS_SPREAD", d.spread).clamp(0.0, 1.0),
        grow: env_f32("EW_GRASS_GROW", d.grow).clamp(0.0, 1.0),
        mean: env_f32("EW_GRASS_MEAN", d.mean).clamp(0.0, 1.0),
        shadow_m: env_f32("EW_GRASS_SHADOW_M", d.shadow_m).max(0.0),
        under: env_f32("EW_GRASS_UNDER", d.under).clamp(0.0, 0.95),
        noise: env_f32("EW_GRASS_NOISE", d.noise).clamp(0.0, 1.0),
        modulate: std::env::var("EW_GRASS_TINT_MOD").map_or(d.modulate, |v| v != "0"),
        fill: env_f32("EW_GRASS_FILL", d.fill).clamp(0.0, 4.0),
    }
}

/// Style E's defaults (far blades).
pub const E_DEFAULTS: GrassConfig = GrassConfig {
    r0: 20.0,
    range: 500.0,
    bmin: gr::BMIN,
    width_max: 12.0,
    shadows: true,
    up_blend: 0.3,
    wind: true,
    bfar: 2.0,
    dfar: 50.0,
    px_min: 1.0,
    spread: 1.0,
    grow: 0.25,
    mean: 1.0,
    shadow_m: 40.0,
    under: 0.45,
    noise: 0.5,
    modulate: true,
    fill: 0.0,
};

/// The node-tier rule of a renderer config (the GPU's `grass_lod`).
pub fn lod_params(c: &GrassConfig) -> gr::LodParams {
    gr::LodParams { r0: c.r0, bmin: c.bmin, bfar: c.bfar, dfar: c.dfar }
}

struct Job {
    id: NodeId,
    keep: f32,
    lab: bool,
    style: Style,
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
    /// Load-set nodes (in cone or near, within range) not resident this frame.
    pub holes: usize,
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
    style: Style,
    residency: bool,
    resident_cap: usize,
    shells: Option<Shells>,
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
    let mut clumps = if j.lab {
        gr::scatter_node(j.id, &lab_at, j.keep, SEED)
    } else {
        let sample = crate::ground_cover::sampler(&j.pages);
        let cl = ed_world::ground_cover::Climate::at(j.id.centre().normalize());
        let at = |d: DVec3| sample(d).and_then(|g| gr::type_from_ground(&g, &cl, d));
        gr::scatter_node(j.id, &at, j.keep, SEED)
    };
    if j.style == Style::D {
        // Style D: shell-only types (lawn) are carpet, not blades.
        clumps.retain(|c| gr::TYPES.get(c.kind as usize).is_none_or(|t| !t.shell_only));
    }
    Done { id: j.id, keep: j.keep, clumps, ms: t.elapsed().as_secs_f64() * 1e3 }
}

/// A shell map build: both maps (fine, then coarse) around a snapped centre.
struct MapJob {
    face: u8,
    centre: [f64; 2],
    a: f64,
    lab: bool,
    pages: PageSet,
}

fn build_maps(j: &MapJob) -> Vec<u32> {
    let sample = crate::ground_cover::sampler(&j.pages);
    let cl = ed_world::ground_cover::Climate::at(gr::face_dir(j.face, j.centre[0], j.centre[1], j.a));
    let at = |d: DVec3| -> Option<(u8, f32)> {
        if j.lab {
            return lab_at(d);
        }
        let g = sample(d)?;
        gr::type_from_ground(&g, &cl, d)
    };
    let mut m = gr::build_map(j.face, j.centre, gr::MAP_FINE_M, j.a, &at, SEED);
    m.extend(gr::build_map(j.face, j.centre, gr::MAP_COARSE_M, j.a, &at, SEED));
    m
}

/// The shell maps around the camera: rebuilt off-thread when the snapped centre moves; the
/// renderer keeps the last complete pair (maps, centre and offsets always agree).
struct Shells {
    config: ShellConfig,
    have: Option<(u8, [f64; 2])>,
    building: Option<((u8, [f64; 2]), Receiver<Vec<u32>>)>,
}

impl Shells {
    fn grass(&self, face: u8, centre: [f64; 2], maps: Arc<Vec<u32>>) -> ed_render::terrain::ShellGrass {
        let fo = gr::map_offset(centre, gr::MAP_FINE_M);
        let co = gr::map_offset(centre, gr::MAP_COARSE_M);
        let wrap = |x: f64| x.rem_euclid(gr::FACE_PERIOD_M) as f32;
        ed_render::terrain::ShellGrass {
            types: gr::gpu_rows(),
            maps: Some(maps),
            face,
            centre: [wrap(centre[0]), wrap(centre[1])],
            offsets: [fo[0], fo[1], co[0], co[1]],
            cells: [gr::MAP_FINE_M as f32, gr::MAP_COARSE_M as f32],
            shells: self.config.shells,
            range: self.config.range,
            tint: self.config.tint,
            h_max: gr::shell_h_max(),
        }
    }

    fn update(&mut self, wd: &TerrainWorld, r: &mut Renderer, camera: &Camera, settle: bool, tv: &TreeView) {
        let body = &wd.anchor.body;
        let a = body.equatorial_m;
        let cd = wd.anchor.render_to_body(camera.position).normalize();
        let (face, x, y) = gr::face_metres(cd, a);
        let key = (face, gr::map_centre(x, y));
        if self.have != Some(key) && self.building.as_ref().is_none_or(|b| b.0 != key) {
            let mut pages = PageSet::new();
            if !wd.grass_lab {
                let pn = NodeId::from_unit(body.id, cd, PAGE_LEVEL);
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        let p = pn.neighbour(dx, dy);
                        if let Some(pg) = (tv.page)(p) {
                            pages.insert(p, pg);
                        }
                    }
                }
            }
            // Without the centre page yet, wait (a map of nothing would flash bare ground).
            if wd.grass_lab || !pages.is_empty() {
                let job = MapJob { face, centre: key.1, a, lab: wd.grass_lab, pages };
                let (tx, rx) = channel();
                std::thread::spawn(move || {
                    let _ = tx.send(build_maps(&job));
                });
                self.building = Some((key, rx));
            }
        }
        let got = match &self.building {
            Some((_, rx)) if settle => rx.recv().ok(),
            Some((_, rx)) => rx.try_recv().ok(),
            None => None,
        };
        if let Some(maps) = got {
            let (k, _) = self.building.take().expect("building");
            self.have = Some(k);
            let g = self.grass(k.0, k.1, Arc::new(maps));
            if let Some(t) = r.terrain_mut() {
                t.set_grass(Some(g));
            }
        }
    }
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
    pub fn new(style: Style) -> Grass {
        let threads = std::env::var("EW_GRASS_THREADS").ok().and_then(|v| v.parse().ok()).unwrap_or((std::thread::available_parallelism().map_or(8, |t| t.get()) / 4).max(1));
        let (jobs, done) = spawn_workers(threads);
        Grass {
            jobs,
            done,
            nodes: BTreeMap::new(),
            requested: BTreeMap::new(),
            ready: BTreeMap::new(),
            free_slots: Vec::new(),
            config: config_from_env(style),
            uploaded: false,
            apply_ms: std::env::var("EW_GRASS_APPLY_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(1.5),
            style,
            residency: std::env::var("EW_RESIDENCY").map_or(true, |v| v != "0"),
            resident_cap: std::env::var("EW_GRASS_RESIDENT").ok().and_then(|v| v.parse().ok()).unwrap_or(0),
            shells: style.shells().then(|| Shells { config: shell_config_from_env(style), have: None, building: None }),
            stats: GrassStats::default(),
        }
    }

    /// The keep tier for a node whose nearest point is `d` m away.
    fn tier_for(&self, d: f64) -> f32 {
        let need = gr::clump_keep_p((d - SLACK_M).max(0.0) as f32, &lod_params(&self.config));
        TIERS.iter().rev().copied().find(|t| *t >= need).unwrap_or(1.0)
    }

    /// The load set (near, or in the view cone, within range) with keep tiers; the extra nodes in
    /// range out of the cone; the keep map (both plus a 10% hysteresis band; value = cosine of
    /// the angle to the view direction). Residency off: keep = load, no extras.
    fn wanted(&self, wd: &TerrainWorld, tv: &TreeView, camera: &Camera, viewport: [u32; 2]) -> (BTreeMap<NodeId, f32>, BTreeMap<NodeId, f32>, BTreeMap<NodeId, f64>) {
        let body = &wd.anchor.body;
        let range = self.config.range as f64;
        let slack = if self.residency { 1.1 } else { 1.0 };
        let cd = wd.anchor.render_to_body(camera.position).normalize();
        let c0 = NodeId::from_unit(body.id, cd, gr::NODE_LEVEL);
        let gh = (tv.ground)(cd).unwrap_or(0.0);
        let size = (body.point(c0.neighbour(1, 0).centre().normalize(), gh) - body.point(c0.centre().normalize(), gh)).length();
        let half = size * 0.75;
        let n = (range * slack / size).ceil() as i32 + 1;
        let fwd = camera.forward().as_dvec3();
        let aspect = viewport[0] as f64 / viewport[1].max(1) as f64;
        let half_diag = ((camera.fov_y as f64 * 0.5).tan() * (1.0 + aspect * aspect).sqrt()).atan();
        let mut load = BTreeMap::new();
        let mut extra = BTreeMap::new();
        let mut keep = BTreeMap::new();
        for dy in -n..=n {
            for dx in -n..=n {
                let id = c0.neighbour(dx, dy);
                let d = id.centre().normalize();
                let p = wd.anchor.body_to_render(body.point(d, (tv.ground)(d).unwrap_or(gh)));
                let to = p - camera.position;
                let dist = to.length();
                let dmin = (dist - half).max(0.0);
                if dmin > range * slack {
                    continue;
                }
                let cosv = to.normalize_or_zero().dot(fwd);
                if dmin > range {
                    keep.insert(id, cosv);
                    continue;
                }
                let cone = cosv > (half_diag + (half / dist.max(1.0)).atan()).min(std::f64::consts::PI).cos();
                if dmin < NEAR_M || cone {
                    load.insert(id, self.tier_for(dmin));
                    keep.insert(id, cosv);
                } else if self.residency {
                    extra.insert(id, self.tier_for(dmin));
                    keep.insert(id, cosv);
                }
            }
        }
        (load, extra, keep)
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
        if let Some(sh) = &mut self.shells {
            sh.update(wd, r, camera, settle, tv);
        }
        if !self.style.blades() {
            self.stats.last_ms = t0.elapsed().as_secs_f64() * 1e3;
            return Ok(());
        }
        if !self.uploaded {
            r.set_grass_types(dev, &gr::gpu_rows()).map_err(|e| e.to_string())?;
            r.set_grass_config(Some(self.config));
            self.uploaded = true;
        }
        let (wanted, extra, keep) = self.wanted(wd, tv, camera, viewport);
        // Residency cap: `EW_GRASS_RESIDENT` (default 3x the load set, min 64), and under the
        // renderer's node table.
        let cap = if self.resident_cap > 0 { self.resident_cap } else { (wanted.len() * 3).max(64) }.min(3500);
        let mut gone: Vec<NodeId> = self
            .nodes
            .iter()
            .filter(|(id, e)| {
                let tier = wanted.get(id).or(extra.get(id));
                match tier {
                    Some(k) => *k * 8.0 < e.keep && !settle,
                    None => !keep.contains_key(id),
                }
            })
            .map(|(id, _)| *id)
            .collect();
        if self.residency && self.nodes.len() > gone.len() + cap {
            let gs: std::collections::BTreeSet<NodeId> = gone.iter().copied().collect();
            let mut cand: Vec<(f64, NodeId)> = self.nodes.keys().copied().filter(|id| !gs.contains(id) && keep.contains_key(id) && !wanted.contains_key(id)).map(|id| (keep[&id], id)).collect();
            cand.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            let over = self.nodes.len() - gone.len() - cap;
            gone.extend(cand.into_iter().take(over).map(|(_, id)| id));
        }
        for id in gone {
            self.release(r, dev, id)?;
        }
        self.requested.retain(|id, _| keep.contains_key(id));
        self.ready.retain(|id, _| keep.contains_key(id));
        // Request: missing nodes and nodes needing a denser tier, nearest first.
        let eye = wd.anchor.render_to_body(camera.position).normalize();
        let mut new: Vec<(f64, NodeId, f32)> = Vec::new();
        // Load set first; the out-of-cone nodes only once it is resident and nothing is pending
        // (a few per frame, nearest first, under the cap and the clump buffer's 3/4 mark).
        let load_done = self.requested.is_empty() && self.ready.is_empty() && wanted.keys().all(|id| self.nodes.contains_key(id));
        let full = r.grass_clumps() as usize * 4 > ed_render::grass::MAX_GRASS_CLUMPS as usize * 3;
        let room = if load_done && !full { cap.saturating_sub(self.nodes.len()).min(4) } else { 0 };
        let extras: Vec<(NodeId, f32)> = if room > 0 { extra.iter().map(|(a, b)| (*a, *b)).collect() } else { Vec::new() };
        let mut cands: Vec<(NodeId, f32, bool)> = wanted.iter().map(|(a, b)| (*a, *b, false)).collect();
        cands.extend(extras.into_iter().map(|(a, b)| (a, b, true)));
        let mut extra_new: Vec<(f64, NodeId, f32)> = Vec::new();
        for (id, keep, is_extra) in cands {
            let have = self.nodes.get(&id).map(|e| e.keep);
            let asked = self.requested.get(&id).copied().or(self.ready.get(&id).map(|d| d.keep));
            if have.is_some_and(|k| k >= keep) || asked.is_some_and(|k| k >= keep) {
                continue;
            }
            let item = (id.centre().normalize().angle_between(eye), id, keep);
            if is_extra { extra_new.push(item) } else { new.push(item) }
        }
        new.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        extra_new.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        extra_new.truncate(room);
        new.extend(extra_new);
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
            self.jobs.send(Job { id, keep, lab: wd.grass_lab, style: self.style, pages }).map_err(|_| "grass worker stopped".to_string())?;
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
        self.stats.holes = wanted.keys().filter(|id| !self.nodes.contains_key(id)).count();
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
        if let Some(sh) = &mut self.shells {
            sh.have = None;
            sh.building = None;
            if let Some(t) = r.terrain_mut() {
                t.set_grass(None);
            }
        }
        Ok(())
    }
}

impl Default for Grass {
    fn default() -> Self {
        Self::new(Style::A)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn styles_parse() {
        assert_eq!(Style::parse("A"), Some(Style::A));
        assert_eq!(Style::parse("c"), Some(Style::C));
        assert_eq!(Style::parse("d"), Some(Style::D));
        assert_eq!(Style::parse("b"), None);
        assert!(Style::A.blades() && !Style::A.shells());
        assert!(!Style::C.blades() && Style::C.shells());
        assert!(Style::D.blades() && Style::D.shells());
        assert_eq!(Style::parse("e"), Some(Style::E));
        assert!(Style::E.blades() && Style::E.shells());
    }

    #[test]
    fn style_e_reaches_far_on_few_records() {
        let e = lod_params(&E_DEFAULTS);
        // Blades reach 500 m; far nodes carry a small, bounded share of their lattice.
        assert!(E_DEFAULTS.range >= 400.0 && E_DEFAULTS.bfar < E_DEFAULTS.bmin);
        let k = gr::clump_keep_p(480.0, &e);
        assert!(k > 0.01 && k < 0.1, "{k}");
        assert!(TIERS.iter().any(|t| *t >= k && *t < 2.5 * k));
        // Style A's rule is unchanged by the far knobs' defaults.
        let a = GrassConfig::default();
        assert_eq!(lod_params(&a), gr::LodParams::near(a.r0, a.bmin));
        // Far clumps hold 2 blades; the blade density per square metre still falls as 1/d^2.
        let (c, keep) = gr::lod_p(300.0, 64.0, &e);
        assert_eq!(c, 2.0);
        assert!((c * keep - 64.0 * (E_DEFAULTS.r0 / 300.0).powi(2)).abs() < 1e-3);
    }

    #[test]
    fn shell_maps_match_the_renderer() {
        use ed_render::terrain as t;
        assert_eq!(gr::MAP_N, t::SHELL_MAP_N);
        assert_eq!(gr::ROW_FLOATS, t::SHELL_TYPE_FLOATS);
        assert!(gr::TYPES.len() <= t::SHELL_MAX_TYPES);
        assert_eq!(gr::FACE_PERIOD_M, t::TEX_PERIOD_M);
        // Every strand cell and map cell divides the snap, so the lattices are world-stable.
        for ty in &gr::TYPES {
            let k = gr::MAP_SNAP_M / ty.strand_m as f64;
            assert_eq!(k, k.round(), "{}", ty.name);
            assert_eq!(ty.strand_m.log2(), ty.strand_m.log2().round(), "{}", ty.name);
        }
        assert_eq!(gr::MAP_SNAP_M % gr::MAP_COARSE_M, 0.0);
        let (_, types, map, total) = t::shell_layout(512);
        assert!(map as usize >= (types as usize + gr::TYPES.len()) * t::SHELL_TYPE_FLOATS);
        assert_eq!(total, map as usize + 2 * t::SHELL_MAP_N * t::SHELL_MAP_N);
    }
}
