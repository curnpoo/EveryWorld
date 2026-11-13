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
//! Switches: `EW_TREES_NEAR=0` (no near trees: the frame is the trees-off frame), `EW_WIND=0`
//! (trees hold still).

use crate::terrain::{CacheSource, TerrainWorld, build_canopy};
use ed_data::DiskCache;
use ed_geo::{CanopyPage, HeightPage, NodeId};
use ed_render::scene::{InstanceDesc, InstanceId, Material, MaterialId, MaterialTextures, MeshId, NodeSlot, TextureAsset, TextureHandle};
use ed_render::{Camera, Renderer};
use ed_world::stand::{CanopySample, StandTree, stand_for_node};
use ed_world::trees::bank;
use ed_world::wind::WindField;
use glam::{Affine3A, DVec3, Quat, Vec3};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::time::Instant;

/// Stand nodes are this level (about 600 m).
pub const NODE_LEVEL: u8 = 14;
/// Canopy and height pages the stands sample (the G2 stand level).
const PAGE_LEVEL: u8 = 12;
/// Stand seed (the G2 dumps).
const STAND_SEED: u64 = 0xE7E2;
/// Nodes this near are kept whatever the view direction (turning around never shows a hole).
const KEEP_ALL_M: f64 = 450.0;

type Pages = BTreeMap<NodeId, Option<(Arc<HeightPage>, Arc<CanopyPage>)>>;

struct NodeEntry {
    slot: NodeSlot,
    /// Instance, trunk base (render frame at placement), height (m).
    instances: Vec<(InstanceId, DVec3, f32)>,
}

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
}

pub struct NearTrees {
    pub on: bool,
    pub wind: Option<WindField>,
    cache: Arc<DiskCache>,
    pages: Pages,
    atlas: Option<TextureHandle>,
    bank: BTreeMap<(u8, u32, u8), (MeshId, MaterialId, f32)>,
    nodes: BTreeMap<NodeId, NodeEntry>,
    free_slots: Vec<NodeSlot>,
    pub stats: NearStats,
    /// Hand-off distance of the last update (m).
    pub handoff_m: f64,
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
            pages: BTreeMap::new(),
            atlas: None,
            bank: BTreeMap::new(),
            nodes: BTreeMap::new(),
            free_slots: Vec::new(),
            stats: NearStats::default(),
            handoff_m: 0.0,
        }
    }

    /// Renderer limits for a terrain run with near trees (every instance of a dense forest).
    pub fn renderer_desc() -> ed_render::RendererDesc {
        ed_render::RendererDesc { max_instances: 1 << 20, max_vertices: 1 << 23, max_indices: 1 << 24, max_meshes: 1024, max_nodes: 1024, ..Default::default() }
    }

    /// The level-14 nodes that should hold trees for `camera`: inside the hand-off distance, all
    /// around near the camera and inside the view cone farther out.
    fn wanted(&self, wd: &TerrainWorld, camera: &Camera, viewport: [u32; 2]) -> (BTreeSet<NodeId>, f64) {
        let body = &wd.anchor.body;
        let ppr = viewport[1] as f64 * 0.5 / (camera.fov_y as f64 * 0.5).tan();
        let handoff = bank::TREE_REF_M as f64 * ppr / bank::HANDOFF_PX as f64;
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
                let p = wd.anchor.body_to_render(body.point(d, gh));
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

    /// Level-12 pages around `ids` (3x3 neighbourhood of each parent), built in parallel.
    fn ensure_pages(&mut self, src: &CacheSource, ids: &[NodeId]) {
        let mut need = BTreeSet::new();
        for id in ids {
            let p = id.ancestor(PAGE_LEVEL);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let q = p.neighbour(dx, dy);
                    if !self.pages.contains_key(&q) {
                        need.insert(q);
                    }
                }
            }
        }
        let need: Vec<NodeId> = need.into_iter().collect();
        if need.is_empty() {
            return;
        }
        let t = Instant::now();
        let slots: Vec<OnceLock<Option<(Arc<HeightPage>, Arc<CanopyPage>)>>> = (0..need.len()).map(|_| OnceLock::new()).collect();
        let cache = &*self.cache;
        par_for(need.len(), |k| {
            let hp = ed_data::node::node_heights(src, need[k]);
            let cp = build_canopy(cache, need[k], &hp);
            let _ = slots[k].set(cp.map(|cp| (Arc::new(hp), cp)));
        });
        for (id, s) in need.iter().zip(slots) {
            self.pages.insert(*id, s.into_inner().flatten());
        }
        self.stats.pages_ms += t.elapsed().as_secs_f64() * 1e3;
    }

    fn stands(&mut self, ids: &[NodeId]) -> Vec<Vec<StandTree>> {
        let t = Instant::now();
        let pages = &self.pages;
        let sample = |d: DVec3| -> Option<CanopySample> {
            let id = NodeId::from_unit(ids[0].body(), d, PAGE_LEVEL);
            let (hp, cp) = pages.get(&id)?.as_ref()?;
            let (u, v) = id.local_uv(d);
            Some(CanopySample::from_page(cp, u, v, hp.sample(u, v) as f32, 0.3))
        };
        let slots: Vec<OnceLock<Vec<StandTree>>> = (0..ids.len()).map(|_| OnceLock::new()).collect();
        par_for(ids.len(), |k| {
            let _ = slots[k].set(stand_for_node(ids[k], &sample, STAND_SEED));
        });
        self.stats.stand_ms += t.elapsed().as_secs_f64() * 1e3;
        self.stats.stand_nodes += ids.len();
        slots.into_iter().map(|s| s.into_inner().unwrap_or_default()).collect()
    }

    /// Bank meshes for `keys` (built in parallel, uploaded in key order).
    fn ensure_bank(&mut self, r: &mut Renderer, dev: &dyn ed_gpu::Device, keys: &BTreeSet<(u8, u32, u8)>) -> Result<(), String> {
        let need: Vec<(u8, u32, u8)> = keys.iter().copied().filter(|k| !self.bank.contains_key(k)).collect();
        if need.is_empty() {
            return Ok(());
        }
        let t = Instant::now();
        if self.atlas.is_none() {
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
            let tex = TextureAsset { format: ed_gpu::Format::Rgba8Srgb, width: a.width, height: a.height, mips };
            self.atlas = Some(r.add_texture(dev, &tex).map_err(|e| format!("leaf atlas: {e}"))?);
        }
        let slots: Vec<OnceLock<bank::BankMesh>> = (0..need.len()).map(|_| OnceLock::new()).collect();
        par_for(need.len(), |k| {
            let (s, v, a) = need[k];
            let _ = slots[k].set(bank::bank_mesh(s as usize, v, a));
        });
        for (key, s) in need.iter().zip(slots) {
            let m = s.into_inner().expect("bank worker");
            self.stats.bank_bytes += m.bytes();
            let mesh = r.add_foliage_mesh(dev, &m.positions, &m.normals, &m.words, &m.lods).map_err(|e| format!("bank mesh {key:?}: {e}"))?;
            let mut mat = Material::pbr(Vec3::from(m.leaf_albedo), 0.0, 0.65);
            mat.shade_color = Vec3::from(m.bark_albedo);
            mat.textures = MaterialTextures { base: self.atlas, ..Default::default() };
            let mat = r.add_material(mat).ok_or("material table full")?;
            self.bank.insert(*key, (mesh, mat, m.height));
        }
        self.stats.bank_meshes = self.bank.len();
        self.stats.bank_ms += t.elapsed().as_secs_f64() * 1e3;
        Ok(())
    }

    /// Brings the near trees up to date for `camera` (headless: everything wanted is built
    /// before the frame). `time` drives the wind.
    pub fn update(&mut self, wd: &TerrainWorld, r: &mut Renderer, dev: &dyn ed_gpu::Device, camera: &Camera, viewport: [u32; 2], time: f64) -> Result<(), String> {
        let rest = r.lighting().shadow_distance;
        if !self.on {
            return Ok(());
        }
        r.set_wind(self.wind.map(|w| w.gpu_words(glam::DVec2::new(camera.position.x, -camera.position.z), time)), rest, true);
        let (wanted, handoff) = self.wanted(wd, camera, viewport);
        self.handoff_m = handoff;
        // Leave: nodes out of range release their instances and node slot.
        let gone: Vec<NodeId> = self.nodes.keys().copied().filter(|id| !wanted.contains(id)).collect();
        for id in gone {
            let e = self.nodes.remove(&id).expect("listed");
            for (i, _, _) in e.instances {
                r.instances().remove(i);
            }
            self.free_slots.push(e.slot);
        }
        let new: Vec<NodeId> = wanted.iter().copied().filter(|id| !self.nodes.contains_key(id)).collect();
        if new.is_empty() {
            return Ok(());
        }
        self.ensure_pages(wd.source(), &new);
        let stands = self.stands(&new);
        let keys: BTreeSet<(u8, u32, u8)> = stands.iter().flatten().map(|t| {
            let p = bank::instance_params(t);
            (t.species, p.variant, p.age)
        }).collect();
        self.ensure_bank(r, dev, &keys)?;
        let t = Instant::now();
        let body = &wd.anchor.body;
        for (id, trees) in new.iter().zip(stands) {
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
            let mut instances = Vec::with_capacity(trees.len());
            for tr in &trees {
                let p = bank::instance_params(tr);
                let Some(&(mesh, material, bank_h)) = self.bank.get(&(tr.species, p.variant, p.age)) else { continue };
                let h = self.ground_at(wd, tr.dir).unwrap_or(gh);
                let base = wd.anchor.body_to_render(body.point(tr.dir, h));
                let up = (wd.anchor.body_to_render(body.point(tr.dir, h + 1.0)) - base).normalize().as_vec3();
                let s = p.scale;
                let rot = Quat::from_rotation_arc(Vec3::Y, up);
                let lean_axis = Vec3::new(p.lean_az.cos(), 0.0, p.lean_az.sin());
                let rot = rot * Quat::from_axis_angle(lean_axis, p.lean) * Quat::from_rotation_y(p.yaw);
                let xf = Affine3A::from_scale_rotation_translation(Vec3::new(s * p.widen, s, s * p.widen), rot, (base - origin).as_vec3());
                let Some(i) = r.instances().add(&InstanceDesc { node: slot, transform: xf, mesh, material, object: tr.id, flags: 0 }) else {
                    return Err("instance pool full".into());
                };
                r.instances().set_user(i, p.user);
                instances.push((i, base, s * bank_h));
            }
            self.stats.instances += instances.len();
            self.nodes.insert(*id, NodeEntry { slot, instances });
        }
        self.stats.instances = self.nodes.values().map(|e| e.instances.len()).sum();
        self.stats.nodes = self.nodes.len();
        self.stats.apply_ms += t.elapsed().as_secs_f64() * 1e3;
        Ok(())
    }

    /// Ground height along body direction `d`: resident terrain pages, else the stand's page.
    fn ground_at(&self, wd: &TerrainWorld, d: DVec3) -> Option<f64> {
        if let Some(h) = wd.planet.height(wd.residency(), wd.anchor.body.id, d * wd.anchor.body.point(d, 0.0).length()) {
            return Some(h.0);
        }
        let id = NodeId::from_unit(wd.anchor.body.id, d, PAGE_LEVEL);
        let (hp, _) = self.pages.get(&id)?.as_ref()?;
        let (u, v) = id.local_uv(d);
        Some(hp.sample(u, v))
    }

    /// Instances per LOD tier for `camera` (CPU estimate from each tree's projected height, the
    /// same rule the GPU LOD selection follows): [T0 full, T1 simplified, T2 shell, handed off].
    pub fn tiers(&self, camera: &Camera, viewport: [u32; 2]) -> [usize; 4] {
        let ppr = viewport[1] as f64 * 0.5 / (camera.fov_y as f64 * 0.5).tan();
        let mut out = [0usize; 4];
        for e in self.nodes.values() {
            for &(_, p, h) in &e.instances {
                let dist = (p - camera.position).length().max(1.0);
                let px = h as f64 * ppr / dist;
                let tier = if px > bank::T0_PX as f64 {
                    0
                } else if px > bank::T1_PX as f64 {
                    1
                } else if px > bank::HANDOFF_PX as f64 * h as f64 / bank::TREE_REF_M as f64 {
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

/// Runs `f(0..n)` on worker threads (contiguous chunks); callers store results by index, so the
/// outcome does not depend on thread timing.
fn par_for(n: usize, f: impl Fn(usize) + Sync) {
    let threads = std::thread::available_parallelism().map_or(8, |t| t.get()).min(16).min(n.max(1));
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
