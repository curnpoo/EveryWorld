//! Pack content -> renderer (M3). `ew` has no geometry generator of its own: everything it
//! draws comes from baked `.ewpack` files ([`ed_pack`]), converted here into renderer meshes,
//! textures, materials and instances.
//!
//! - Packs are mounted in order ([`PACKS`]: proving, bench, character); a key present in two
//!   packs resolves to the first. The proving pack is required (a missing one is an error that
//!   names the bake command); the others are optional and only their placements go missing.
//! - Every texture, material, skeleton, mesh and prefab of the mounted packs is loaded (keys in
//!   sorted order, so ids are deterministic). Instances come from the proving node's
//!   `Instances` chunk ([`ed_assets::bake::proving_node`]); an instance names a mesh asset or a
//!   prefab, whose nodes become one renderer instance each. Skinned meshes get one skinned
//!   copy per placement, posed each frame by [`ed_anim::sine_pose`].
//! - Hot reload ([`Content::poll_reload`]): a pack whose file changed is reopened and its
//!   manifest diffed against the mounted one; only changed chunks are decoded and re-uploaded
//!   (textures: add new then release old, materials: rewritten in place, static meshes:
//!   `replace_mesh`, prefabs / skinned meshes / instance lists: their placements respawned).
//!   GPU releases are deferred by the renderer until frames in flight finish.

use ed_assets::bake::proving_node;
use ed_assets::geom::decode;
use ed_assets::material as pm;
use ed_assets::proving::{ORIGIN, from_enu};
use ed_gpu::{Device, Format};
use ed_pack::kinds::{ColourRole, Instance, Instances, Material as PackMaterial, MaterialModel as PackModel, Mesh as PackMesh, Param, Prefab, Skeleton, Texture, TextureFormat, Transform};
use ed_pack::{AssetId, ChunkHash, ChunkKey, ChunkKind, PackReader, Rung};
use ed_render::Renderer;
use ed_render::scene::{InstanceDesc, InstanceId, Material, MaterialId, MaterialModel, MaterialTextures, MeshAsset, MeshId, MeshLod, NodeSlot, SkinId, SkinnedMeshAsset, TextureAsset, TextureHandle, UvTransform};
use glam::{Affine3A, Mat4, Quat, Vec2, Vec3};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// One pack `ew` mounts.
#[derive(Clone, Copy, Debug)]
pub struct PackSpec {
    pub name: &'static str,
    pub required: bool,
    /// How to build it (printed when it is missing).
    pub bake: &'static str,
}

pub const PROVING_BAKE: &str = "cargo run --release -p ed_assets --bin ew-bake -- build assets-src/proving.toml -o target/packs/proving.ewpack";
pub const BENCH_BAKE: &str =
    "/Applications/Blender.app/Contents/MacOS/Blender -b -P tools/blender/make_prefab.py -- --out target/blender && cargo run --release -p ed_assets --bin ew-bake -- build assets-src/bench.toml -o target/packs/bench.ewpack";
pub const CHARACTER_BAKE: &str = "EW_SOLARCAT=/path/to/Solarcat.glb cargo run --release -p ed_assets --bin ew-bake -- build assets-src/character.toml -o target/packs/character.ewpack";

/// Mount order (first wins on duplicate keys).
pub const PACKS: [PackSpec; 3] = [
    PackSpec { name: "proving", required: true, bake: PROVING_BAKE },
    PackSpec { name: "bench", required: false, bake: BENCH_BAKE },
    PackSpec { name: "character", required: false, bake: CHARACTER_BAKE },
];

/// Extra bound-sphere margin of skinned meshes (the sway bends limbs out of the bind pose).
const POSE_MARGIN: f32 = 1.4;

/// File identity used to notice a rebuilt pack (bake writes temp + rename).
type Stamp = Option<(SystemTime, u64)>;

fn stamp(p: &Path) -> Stamp {
    std::fs::metadata(p).ok().and_then(|m| Some((m.modified().ok()?, m.len())))
}

struct Mounted {
    spec: PackSpec,
    path: PathBuf,
    stamp: Stamp,
    reader: Option<PackReader<File>>,
}

/// A mesh asset: static (uploaded once, shared by all instances) or skinned (copied per
/// placement).
enum MeshEntry {
    Static(MeshId),
    Skinned(Box<(SkinnedMeshAsset, Skeleton)>),
}

/// One renderer instance of a placement.
enum Part {
    Static(InstanceId),
    Skinned { inst: InstanceId, skin: SkinId, skeleton: AssetId },
}

/// One entry of the Instances chunk, or an authored placement ([`Content::set_authored`]).
struct Placement {
    src: Instance,
    parts: Vec<Part>,
    /// Each part's transform relative to the placement (prefab node globals).
    locals: Vec<Affine3A>,
    /// From the layer files (editable), not the pack.
    authored: bool,
}

/// What [`Content::object`] knows about one object.
#[derive(Clone, Debug, PartialEq)]
pub struct ObjectDesc {
    pub asset: AssetId,
    /// Asset path when known (proving parts, authored prefabs).
    pub asset_name: Option<String>,
    /// Placement transform, node frame (y-up, relative to the proving origin).
    pub transform: Affine3A,
    pub parts: usize,
    pub authored: bool,
    pub material: Option<AssetId>,
}

/// What one load or reload did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoadStats {
    /// Keys whose hash changed, appeared or disappeared (reload) or all keys (load).
    pub changed_keys: usize,
    pub textures: usize,
    pub texture_bytes: u64,
    pub materials: usize,
    pub meshes: usize,
    pub skinned_meshes: usize,
    pub prefabs: usize,
    pub placements: usize,
    pub instances: usize,
    /// Instances naming an asset no mounted pack has.
    pub missing: usize,
    /// Reload: chunks whose hash changed only through metadata (not re-uploaded).
    pub metadata_only: usize,
}

/// Mounted packs and what they put into the renderer.
pub struct Content {
    packs: Vec<Mounted>,
    /// Merged view: key -> (pack index, hash).
    keys: BTreeMap<ChunkKey, (usize, ChunkHash)>,
    node: NodeSlot,
    default_material: MaterialId,
    textures: BTreeMap<AssetId, TextureHandle>,
    /// Material assets: id and the texture assets it names.
    materials: BTreeMap<AssetId, (MaterialId, PackMaterial)>,
    /// Skeletons by the mesh they deform, and skeleton asset -> mesh asset.
    skeletons: BTreeMap<AssetId, Skeleton>,
    skeleton_mesh: BTreeMap<AssetId, AssetId>,
    meshes: BTreeMap<AssetId, MeshEntry>,
    prefabs: BTreeMap<AssetId, Prefab>,
    placements: Vec<Placement>,
    /// Skinned parts (skin, skeleton mesh asset), rebuilt after every apply so [`Content::animate`]
    /// does not walk all placements each frame.
    skinned: Vec<(SkinId, AssetId)>,
    /// BLAKE3 of each loaded chunk's payload: a chunk whose hash changed only through its
    /// metadata (provenance names the source file's bytes, so every chunk of a re-exported
    /// glTF gets a new hash) is not re-uploaded.
    payloads: BTreeMap<ChunkKey, [u8; 32]>,
    /// Object id -> placement index.
    by_object: BTreeMap<ed_geo::ObjectId, usize>,
    /// Known asset paths (proving parts, authored prefabs) for the inspector.
    names: BTreeMap<AssetId, String>,
    /// Pack collision (node shapes and per-asset shapes), re-read when its chunks change.
    pub collision: crate::collide::PackCollision,
    /// Bumped whenever [`Content::collision`] changes (hot reload rebuilds the sim statics).
    pub collision_gen: u64,
    /// Notes for the log (optional packs missing, ...).
    pub notes: Vec<String>,
    pub load_ms: f64,
    pub stats: LoadStats,
}

/// Default pack folder: `target/packs` under the current directory, or under the workspace
/// root when run from a crate folder (tests).
pub fn default_pack_dir() -> PathBuf {
    let local = PathBuf::from("target/packs");
    if local.join("proving.ewpack").exists() {
        return local;
    }
    let ws = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/packs");
    if ws.join("proving.ewpack").exists() { ws } else { local }
}

fn texture_format(t: &Texture<'_>) -> Result<Format, String> {
    Ok(match (t.format, t.role) {
        (TextureFormat::Bc7, ColourRole::Srgb) => Format::Bc7Srgb,
        (TextureFormat::Bc7, _) => Format::Bc7Unorm,
        (TextureFormat::Bc5, _) => Format::Bc5Unorm,
        (TextureFormat::Bc4, _) => Format::Bc4Unorm,
        (TextureFormat::Rgba8Unorm, ColourRole::Srgb) => Format::Rgba8Srgb,
        (TextureFormat::Rgba8Unorm, _) => Format::Rgba8Unorm,
        (f, r) => return Err(format!("texture format {f:?} / {r:?} is not supported by the renderer")),
    })
}

fn texture_asset(t: &Texture<'_>) -> Result<TextureAsset, String> {
    Ok(TextureAsset { format: texture_format(t)?, width: t.width, height: t.height, mips: t.mips.iter().map(|m| m.to_vec()).collect() })
}

fn f(p: Option<&Param>, d: f32) -> f32 {
    match p {
        Some(Param::Float(v)) => *v,
        _ => d,
    }
}
fn v3(p: Option<&Param>, d: Vec3) -> Vec3 {
    match p {
        Some(Param::Vec3(v)) => Vec3::from(*v),
        Some(Param::Vec4(v)) => Vec3::new(v[0], v[1], v[2]),
        _ => d,
    }
}

/// Pack material -> renderer material (parameter names from [`ed_assets::material`]).
pub fn material(m: &PackMaterial, tex: impl Fn(&AssetId) -> Option<TextureHandle>) -> Material {
    let p = |k: &str| m.params.get(k);
    let base = v3(p(pm::P_BASE), Vec3::splat(0.5));
    let mut out = match m.model {
        PackModel::Toon => Material::toon(base, v3(p(pm::P_SHADE), Vec3::splat(0.45)), f(p(pm::P_BANDS), 3.0), f(p(pm::P_RIM), 0.0)),
        _ => Material::pbr(base, f(p(pm::P_METALLIC), 0.0), f(p(pm::P_ROUGHNESS), 0.5)),
    };
    if m.model == PackModel::Toon {
        out.model = MaterialModel::Toon;
    }
    out.checker = f(p(pm::P_CHECKER), 0.0) > 0.5;
    let t = |slot: &str| m.textures.get(slot).and_then(&tex);
    out.textures = MaterialTextures { base: t(pm::T_BASE), normal: t(pm::T_NORMAL), orm: t(pm::T_ORM) };
    if let Some(Param::Vec4(v)) = p(pm::P_UV) {
        out.uv = UvTransform { offset: Vec2::new(v[0], v[1]), scale: Vec2::new(v[2], v[3]), rotation: f(p(pm::P_UV_ROT), 0.0) };
    } else if let Some(r) = p(pm::P_UV_ROT) {
        out.uv.rotation = f(Some(r), 0.0);
    }
    out
}

fn mesh_asset(m: &PackMesh<'_>) -> Result<(MeshAsset, Vec<[u16; 4]>, Vec<[f32; 4]>), String> {
    let d = decode(m).map_err(|e| format!("{e:#}"))?;
    let n = d.build.positions.len();
    let uvs = if d.build.uvs.len() == n { d.build.uvs } else { Vec::new() };
    let lods = d.lods.into_iter().take(ed_render::scene::MAX_LODS).map(|l| MeshLod { indices: l.indices, error: l.error }).collect();
    Ok((MeshAsset { positions: d.build.positions, normals: d.build.normals, uvs, lods }, d.build.joints, d.build.weights))
}

fn affine(t: &Transform) -> Affine3A {
    Affine3A::from_scale_rotation_translation(Vec3::from(t.scale), Quat::from_array(t.rotation), Vec3::from(t.translation))
}

impl Content {
    /// Mounts the packs in `dir` and loads them into `r` (one node at the proving origin).
    pub fn load(r: &mut Renderer, dev: &dyn Device, dir: &Path) -> Result<Content, String> {
        Self::load_specs(r, dev, dir, &PACKS)
    }

    pub fn load_specs(r: &mut Renderer, dev: &dyn Device, dir: &Path, specs: &[PackSpec]) -> Result<Content, String> {
        let t0 = std::time::Instant::now();
        let mut packs = Vec::new();
        let mut notes = Vec::new();
        for s in specs {
            let path = dir.join(format!("{}.ewpack", s.name));
            let reader = match PackReader::open_file(&path) {
                Ok(rd) => Some(rd),
                Err(e) if s.required => {
                    return Err(format!("pack {} ({}): {e}\n  ew draws only baked content; build it with:\n    {}", s.name, path.display(), s.bake));
                }
                Err(_) => {
                    notes.push(format!("optional pack {} not found ({}); its placements are skipped. Build it with: {}", s.name, path.display(), s.bake));
                    None
                }
            };
            packs.push(Mounted { spec: *s, stamp: stamp(&path), path, reader });
        }
        let node = r.instances().add_node(ORIGIN).ok_or("node table full")?;
        let default_material = r.add_material(Material::pbr(Vec3::splat(0.6), 0.0, 0.6)).ok_or("material table full")?;
        let mut c = Content {
            packs,
            keys: BTreeMap::new(),
            node,
            default_material,
            textures: BTreeMap::new(),
            materials: BTreeMap::new(),
            skeletons: BTreeMap::new(),
            skeleton_mesh: BTreeMap::new(),
            meshes: BTreeMap::new(),
            prefabs: BTreeMap::new(),
            placements: Vec::new(),
            skinned: Vec::new(),
            payloads: BTreeMap::new(),
            by_object: BTreeMap::new(),
            names: ed_assets::proving::asset_names().into_iter().map(|n| (AssetId::from_path(&n), n)).collect(),
            collision: Default::default(),
            collision_gen: 0,
            notes,
            load_ms: 0.0,
            stats: LoadStats::default(),
        };
        c.keys = c.merged();
        let all: BTreeSet<ChunkKey> = c.keys.keys().copied().collect();
        for k in &all {
            let h = c.payload_hash(k)?;
            c.payloads.insert(*k, h);
        }
        let mut st = c.apply(r, dev, &all, &BTreeSet::new())?;
        st.changed_keys = all.len();
        c.stats = st;
        c.load_ms = t0.elapsed().as_secs_f64() * 1e3;
        Ok(c)
    }

    fn merged(&self) -> BTreeMap<ChunkKey, (usize, ChunkHash)> {
        let mut m = BTreeMap::new();
        for (i, p) in self.packs.iter().enumerate() {
            if let Some(rd) = &p.reader {
                for e in rd.entries() {
                    m.entry(e.key).or_insert((i, e.hash));
                }
            }
        }
        m
    }

    fn read(&self, key: &ChunkKey) -> Result<ed_pack::Chunk<'static>, String> {
        let (p, _) = self.keys.get(key).ok_or_else(|| format!("{key:?}: not mounted"))?;
        let rd = self.packs[*p].reader.as_ref().ok_or("pack not open")?;
        rd.read(key).map_err(|e| format!("{} {key:?}: {e}", self.packs[*p].spec.name))
    }

    fn payload_hash(&self, key: &ChunkKey) -> Result<[u8; 32], String> {
        Ok(*blake3::hash(self.read(key)?.payload()).as_bytes())
    }

    /// Applies `changed` keys (added or rewritten) and `removed` keys to the renderer.
    fn apply(&mut self, r: &mut Renderer, dev: &dyn Device, changed: &BTreeSet<ChunkKey>, removed: &BTreeSet<ChunkKey>) -> Result<LoadStats, String> {
        let mut st = LoadStats::default();
        let of = |set: &BTreeSet<ChunkKey>, k: ChunkKind| -> Vec<AssetId> {
            set.iter()
                .filter_map(|key| match *key {
                    ChunkKey::Asset { asset, kind } if kind == k => Some(asset),
                    _ => None,
                })
                .collect()
        };
        // 1. Textures: upload the new ones; old handles are released after materials moved.
        let mut old_textures = Vec::new();
        let mut dirty_tex = BTreeSet::new();
        for a in of(changed, ChunkKind::Texture) {
            let c = self.read(&ChunkKey::asset(a, ChunkKind::Texture))?;
            let t: Texture<'_> = c.decode().map_err(|e| format!("texture: {e}"))?;
            let ta = texture_asset(&t)?;
            st.texture_bytes += ta.mips.iter().map(|m| m.len() as u64).sum::<u64>();
            let h = r.add_texture(dev, &ta).map_err(|e| format!("texture: {e}"))?;
            if let Some(old) = self.textures.insert(a, h) {
                old_textures.push(old);
            }
            dirty_tex.insert(a);
            st.textures += 1;
        }
        for a in of(removed, ChunkKind::Texture) {
            if let Some(old) = self.textures.remove(&a) {
                old_textures.push(old);
                dirty_tex.insert(a);
            }
        }
        // 2. Materials: changed ones, plus every material naming a changed texture.
        let mut dirty_mat: BTreeSet<AssetId> = of(changed, ChunkKind::Material).into_iter().collect();
        for (a, (_, m)) in &self.materials {
            if m.textures.values().any(|t| dirty_tex.contains(t)) {
                dirty_mat.insert(*a);
            }
        }
        for a in dirty_mat {
            let key = ChunkKey::asset(a, ChunkKind::Material);
            let pmat: PackMaterial = if changed.contains(&key) {
                let c = self.read(&key)?;
                c.decode().map_err(|e| format!("material: {e}"))?
            } else {
                match self.materials.get(&a) {
                    Some((_, m)) => m.clone(),
                    None => continue,
                }
            };
            let m = material(&pmat, |t| self.textures.get(t).copied());
            match self.materials.get_mut(&a) {
                Some(e) => {
                    r.set_material(e.0, m);
                    e.1 = pmat;
                }
                None => {
                    let id = r.add_material(m).ok_or("material table full")?;
                    self.materials.insert(a, (id, pmat));
                }
            }
            st.materials += 1;
        }
        for old in old_textures {
            r.remove_texture(dev, old);
        }
        // 3. Skeletons (keyed by the mesh they deform).
        let mut dirty_assets: BTreeSet<AssetId> = BTreeSet::new();
        for a in of(changed, ChunkKind::Skeleton) {
            let c = self.read(&ChunkKey::asset(a, ChunkKind::Skeleton))?;
            let sk: Skeleton = c.decode().map_err(|e| format!("skeleton: {e}"))?;
            dirty_assets.insert(sk.mesh);
            self.skeleton_mesh.insert(a, sk.mesh);
            self.skeletons.insert(sk.mesh, sk);
        }
        for a in of(removed, ChunkKind::Skeleton) {
            if let Some(m) = self.skeleton_mesh.remove(&a) {
                self.skeletons.remove(&m);
                dirty_assets.insert(m);
            }
        }
        // 4. Meshes.
        let mut retire_meshes = Vec::new();
        let mut mesh_keys: BTreeSet<AssetId> = of(changed, ChunkKind::Mesh).into_iter().collect();
        mesh_keys.extend(dirty_assets.iter().copied().filter(|a| self.keys.contains_key(&ChunkKey::asset(*a, ChunkKind::Mesh))));
        for a in mesh_keys {
            let c = self.read(&ChunkKey::asset(a, ChunkKind::Mesh))?;
            let pmesh: PackMesh<'_> = c.decode().map_err(|e| format!("mesh: {e}"))?;
            let (asset, joints, weights) = mesh_asset(&pmesh)?;
            match self.skeletons.get(&a) {
                Some(sk) if !joints.is_empty() => {
                    let s = SkinnedMeshAsset { mesh: asset, joints, weights, joint_count: sk.joints.len() as u32, pose_margin: POSE_MARGIN };
                    if let Some(MeshEntry::Static(id)) = self.meshes.insert(a, MeshEntry::Skinned(Box::new((s, sk.clone())))) {
                        retire_meshes.push(id);
                    }
                    dirty_assets.insert(a);
                    st.skinned_meshes += 1;
                }
                _ => {
                    match self.meshes.get(&a) {
                        Some(MeshEntry::Static(id)) => r.replace_mesh(dev, *id, &asset).map_err(|e| format!("mesh: {e}"))?,
                        _ => {
                            let id = r.add_mesh(dev, &asset).map_err(|e| format!("mesh: {e}"))?;
                            if self.meshes.insert(a, MeshEntry::Static(id)).is_some() {
                                dirty_assets.insert(a);
                            }
                        }
                    }
                    st.meshes += 1;
                }
            }
        }
        for a in of(removed, ChunkKind::Mesh) {
            if let Some(MeshEntry::Static(id)) = self.meshes.remove(&a) {
                retire_meshes.push(id);
            }
            dirty_assets.insert(a);
        }
        // 5. Prefabs.
        for a in of(changed, ChunkKind::Prefab) {
            let c = self.read(&ChunkKey::asset(a, ChunkKind::Prefab))?;
            let p: Prefab = c.decode().map_err(|e| format!("prefab: {e}"))?;
            self.prefabs.insert(a, p);
            dirty_assets.insert(a);
            st.prefabs += 1;
        }
        for a in of(removed, ChunkKind::Prefab) {
            self.prefabs.remove(&a);
            dirty_assets.insert(a);
        }
        // Prefabs whose node meshes changed shape (static <-> skinned) respawn too.
        let via: Vec<AssetId> = self.prefabs.iter().filter(|(_, p)| p.nodes.iter().any(|n| n.mesh.is_some_and(|m| dirty_assets.contains(&m)))).map(|(a, _)| *a).collect();
        dirty_assets.extend(via);
        // Material assets that disappeared or appeared change which material placements use.
        for a in of(changed, ChunkKind::Material).into_iter().chain(of(removed, ChunkKind::Material)) {
            if removed.contains(&ChunkKey::asset(a, ChunkKind::Material)) {
                self.materials.remove(&a);
            }
            dirty_assets.insert(a);
        }
        // 6. Placements (generated ones from the instances chunk; authored ones are kept and
        // only respawn when their asset changed).
        let inst_key = ChunkKey::node(proving_node(), ChunkKind::Instances, Rung::R0);
        if changed.contains(&inst_key) || removed.contains(&inst_key) {
            let (authored, generated): (Vec<Placement>, Vec<Placement>) = std::mem::take(&mut self.placements).into_iter().partition(|p| p.authored);
            for p in generated {
                Self::despawn(r, p);
            }
            if self.keys.contains_key(&inst_key) {
                let c = self.read(&inst_key)?;
                let list: Instances = c.decode().map_err(|e| format!("instances: {e}"))?;
                self.placements = list.instances.into_iter().map(|src| Placement { src, parts: Vec::new(), locals: Vec::new(), authored: false }).collect();
            }
            for i in 0..self.placements.len() {
                self.spawn(r, dev, i, &mut st)?;
            }
            for p in authored {
                let dirty = dirty_assets.contains(&p.src.asset);
                self.placements.push(p);
                if dirty {
                    let i = self.placements.len() - 1;
                    let parts = std::mem::take(&mut self.placements[i].parts);
                    Self::despawn_parts(r, parts);
                    self.spawn(r, dev, i, &mut st)?;
                }
            }
        } else if !dirty_assets.is_empty() {
            for i in 0..self.placements.len() {
                let s = &self.placements[i].src;
                if dirty_assets.contains(&s.asset) || s.material.is_some_and(|m| dirty_assets.contains(&m)) {
                    let parts = std::mem::take(&mut self.placements[i].parts);
                    Self::despawn_parts(r, parts);
                    self.spawn(r, dev, i, &mut st)?;
                }
            }
        }
        for id in retire_meshes {
            r.remove_mesh(id).map_err(|e| format!("retire mesh: {e}"))?;
        }
        // 7. Collision: re-read when any collision chunk changed.
        if changed.iter().chain(removed).any(crate::collide::PackCollision::is_collision_key) {
            let keys: Vec<ChunkKey> = self.keys.keys().copied().collect();
            self.collision = crate::collide::PackCollision::from_keys(keys, |k| self.read(k)?.decode().map_err(|e| format!("collision: {e}")))?;
            self.collision_gen += 1;
        }
        self.reindex_skinned();
        Ok(st)
    }

    fn despawn_parts(r: &mut Renderer, parts: Vec<Part>) {
        for p in parts {
            match p {
                Part::Static(i) => {
                    r.instances().remove(i);
                }
                Part::Skinned { inst, skin, .. } => {
                    r.instances().remove(inst);
                    let _ = r.remove_skinned_mesh(skin);
                }
            }
        }
    }
    fn despawn(r: &mut Renderer, p: Placement) {
        Self::despawn_parts(r, p.parts);
    }

    fn material_id(&self, a: Option<AssetId>) -> MaterialId {
        a.and_then(|a| self.materials.get(&a)).map(|m| m.0).unwrap_or(self.default_material)
    }

    /// Creates the renderer instances of placement `i`.
    fn spawn(&mut self, r: &mut Renderer, dev: &dyn Device, i: usize, st: &mut LoadStats) -> Result<(), String> {
        let src = self.placements[i].src.clone();
        let base = from_enu(&src.transform);
        // (mesh asset, transform, material asset)
        let mut nodes: Vec<(AssetId, Affine3A, Option<AssetId>)> = Vec::new();
        if self.meshes.contains_key(&src.asset) {
            nodes.push((src.asset, base, src.material));
        } else if let Some(p) = self.prefabs.get(&src.asset) {
            let mut globals: Vec<Affine3A> = Vec::with_capacity(p.nodes.len());
            for n in &p.nodes {
                let parent = n.parent.and_then(|k| globals.get(k as usize).copied()).unwrap_or(Affine3A::IDENTITY);
                let g = parent * affine(&n.transform);
                globals.push(g);
                if let Some(m) = n.mesh {
                    nodes.push((m, base * g, src.material.or(n.material)));
                }
            }
        } else {
            st.missing += 1;
            return Ok(());
        }
        let mut parts = Vec::with_capacity(nodes.len());
        let inv = base.inverse();
        let mut locals = Vec::with_capacity(nodes.len());
        for (mesh, xf, mat) in nodes {
            let material = self.material_id(mat);
            let desc = |m: MeshId| InstanceDesc { node: self.node, transform: xf, mesh: m, material, object: src.object, flags: 0 };
            match self.meshes.get(&mesh) {
                Some(MeshEntry::Static(id)) => {
                    let d = desc(*id);
                    parts.push(Part::Static(r.instances().add(&d).ok_or("instance pool full")?));
                    locals.push(inv * xf);
                }
                Some(MeshEntry::Skinned(b)) => {
                    let (skin, mid) = r.add_skinned_mesh(dev, &b.0).map_err(|e| format!("skinned mesh: {e}"))?;
                    let pal = ed_anim::palette(&b.1, &ed_anim::rest_locals(&b.1));
                    r.set_skin_pose(skin, &pal).map_err(|e| format!("skin pose: {e}"))?;
                    let d = desc(mid);
                    let inst = r.instances().add(&d).ok_or("instance pool full")?;
                    parts.push(Part::Skinned { inst, skin, skeleton: mesh });
                    locals.push(inv * xf);
                }
                None => st.missing += 1,
            }
        }
        st.instances += parts.len();
        st.placements += 1;
        self.placements[i].parts = parts;
        self.placements[i].locals = locals;
        Ok(())
    }

    /// Replaces the authored placements (layer `place` ops, the character) with `list`:
    /// (object, asset, node-frame y-up transform, asset path).
    pub fn set_authored(&mut self, r: &mut Renderer, dev: &dyn Device, list: &[(ed_geo::ObjectId, AssetId, Affine3A, String)]) -> Result<LoadStats, String> {
        let (old, keep): (Vec<Placement>, Vec<Placement>) = std::mem::take(&mut self.placements).into_iter().partition(|p| p.authored);
        self.placements = keep;
        for p in old {
            Self::despawn(r, p);
        }
        let mut st = LoadStats::default();
        for (object, asset, xf, name) in list {
            self.names.insert(*asset, name.clone());
            let src = Instance { object: *object, transform: ed_assets::proving::to_enu(xf), asset: *asset, material: None, variant: 0, flags: 0 };
            self.placements.push(Placement { src, parts: Vec::new(), locals: Vec::new(), authored: true });
            let i = self.placements.len() - 1;
            self.spawn(r, dev, i, &mut st)?;
        }
        self.reindex_skinned();
        Ok(st)
    }

    /// Moves an authored object's instances (node-frame y-up transform). Generated objects are
    /// read-only: `false`.
    pub fn move_object(&mut self, r: &mut Renderer, object: ed_geo::ObjectId, xf: &Affine3A) -> bool {
        let Some(&i) = self.by_object.get(&object) else { return false };
        let p = &mut self.placements[i];
        if !p.authored {
            return false;
        }
        p.src.transform = ed_assets::proving::to_enu(xf);
        for (part, local) in p.parts.iter().zip(&p.locals) {
            let inst = match part {
                Part::Static(i) => *i,
                Part::Skinned { inst, .. } => *inst,
            };
            r.instances().set_transform(inst, &(*xf * *local));
        }
        true
    }

    /// What is known about an object (a generated or authored placement).
    pub fn object(&self, object: ed_geo::ObjectId) -> Option<ObjectDesc> {
        let p = &self.placements[*self.by_object.get(&object)?];
        Some(ObjectDesc {
            asset: p.src.asset,
            asset_name: self.names.get(&p.src.asset).cloned(),
            transform: from_enu(&p.src.transform),
            parts: p.parts.len(),
            authored: p.authored,
            material: p.src.material,
        })
    }

    /// Every placed object: (id, authored).
    pub fn objects(&self) -> impl Iterator<Item = (ed_geo::ObjectId, bool)> + '_ {
        self.placements.iter().map(|p| (p.src.object, p.authored))
    }

    /// Whether a mesh or prefab asset is loaded.
    pub fn has_asset(&self, a: AssetId) -> bool {
        self.meshes.contains_key(&a) || self.prefabs.contains_key(&a)
    }

    pub fn asset_name(&self, a: AssetId) -> Option<&str> {
        self.names.get(&a).map(String::as_str)
    }

    /// Poses every skinned placement at simulation time `t` seconds ([`ed_anim::sine_pose`]).
    pub fn animate(&self, r: &mut Renderer, t: f64) -> Result<(), String> {
        for (skin, skeleton) in &self.skinned {
            if let Some(MeshEntry::Skinned(b)) = self.meshes.get(skeleton) {
                let pal = ed_anim::palette(&b.1, &ed_anim::sine_pose(&b.1, t, ed_anim::Sway::default()));
                r.set_skin_pose(*skin, &pal).map_err(|e| format!("skin pose: {e}"))?;
            }
        }
        Ok(())
    }

    fn reindex_skinned(&mut self) {
        self.by_object = self.placements.iter().enumerate().map(|(i, p)| (p.src.object, i)).collect();
        self.skinned = self
            .placements
            .iter()
            .flat_map(|p| &p.parts)
            .filter_map(|part| match part {
                Part::Skinned { skin, skeleton, .. } => Some((*skin, *skeleton)),
                _ => None,
            })
            .collect();
    }

    /// Number of skinned placements' parts.
    pub fn skinned_parts(&self) -> usize {
        self.placements.iter().flat_map(|p| &p.parts).filter(|p| matches!(p, Part::Skinned { .. })).count()
    }
    pub fn instance_count(&self) -> usize {
        self.placements.iter().map(|p| p.parts.len()).sum()
    }
    pub fn placement_count(&self) -> usize {
        self.placements.len()
    }
    pub fn mesh_count(&self) -> usize {
        self.meshes.len()
    }
    pub fn material_count(&self) -> usize {
        self.materials.len()
    }
    pub fn texture_count(&self) -> usize {
        self.textures.len()
    }
    /// Mounted packs: (name, path, open).
    pub fn mounted(&self) -> Vec<(&'static str, &Path, bool)> {
        self.packs.iter().map(|p| (p.spec.name, p.path.as_path(), p.reader.is_some())).collect()
    }

    /// Pack files whose stamp changed since they were mounted.
    pub fn changed_packs(&self) -> Vec<usize> {
        self.packs.iter().enumerate().filter(|(_, p)| stamp(&p.path) != p.stamp).map(|(i, _)| i).collect()
    }

    /// Reopens changed pack files and applies the manifest diff. `Ok(None)`: nothing changed.
    /// A pack that fails to open keeps its old mount (and the error is returned).
    pub fn poll_reload(&mut self, r: &mut Renderer, dev: &dyn Device) -> Result<Option<LoadStats>, String> {
        let changed = self.changed_packs();
        if changed.is_empty() {
            return Ok(None);
        }
        self.reload(r, dev, &changed).map(Some)
    }

    /// Reopens packs `which` and applies the diff of the merged manifest.
    pub fn reload(&mut self, r: &mut Renderer, dev: &dyn Device, which: &[usize]) -> Result<LoadStats, String> {
        for &i in which {
            let p = &mut self.packs[i];
            let st = stamp(&p.path);
            match PackReader::open_file(&p.path) {
                Ok(rd) => {
                    p.reader = Some(rd);
                    p.stamp = st;
                }
                Err(e) => {
                    p.stamp = st;
                    if p.spec.required || p.reader.is_some() {
                        return Err(format!("reopening {}: {e} (keeping the previous content)", p.path.display()));
                    }
                }
            }
        }
        let new = self.merged();
        let mut changed = BTreeSet::new();
        let mut removed = BTreeSet::new();
        for (k, (_, h)) in &new {
            if self.keys.get(k).map(|v| v.1) != Some(*h) {
                changed.insert(*k);
            }
        }
        for k in self.keys.keys() {
            if !new.contains_key(k) {
                removed.insert(*k);
                self.payloads.remove(k);
            }
        }
        self.keys = new;
        // Same payload, new metadata (provenance): nothing to upload.
        let mut meta_only = 0;
        let mut upload = BTreeSet::new();
        for k in changed {
            let h = self.payload_hash(&k)?;
            if self.payloads.insert(k, h) == Some(h) {
                meta_only += 1;
            } else {
                upload.insert(k);
            }
        }
        let mut st = self.apply(r, dev, &upload, &removed)?;
        st.changed_keys = upload.len() + removed.len();
        st.metadata_only = meta_only;
        Ok(st)
    }

    /// One line per pack plus totals (summary files).
    pub fn describe(&self) -> String {
        let mut s = String::new();
        for p in &self.packs {
            let n = p.reader.as_ref().map(|r| r.key_count()).unwrap_or(0);
            s.push_str(&format!("  pack {}: {} ({})\n", p.spec.name, p.path.display(), if p.reader.is_some() { format!("{n} keys") } else { "not mounted".into() }));
        }
        for n in &self.notes {
            s.push_str(&format!("  note: {n}\n"));
        }
        s
    }

    /// Releases skinned copies and instances (meshes, textures and materials go with the
    /// renderer).
    pub fn clear(&mut self, r: &mut Renderer) {
        for p in std::mem::take(&mut self.placements) {
            Self::despawn(r, p);
        }
        self.skinned.clear();
    }

    /// Mat4 palette for tests: the current pose of the first skinned placement.
    pub fn first_skin_palette(&self, t: f64) -> Option<Vec<Mat4>> {
        self.placements.iter().flat_map(|p| &p.parts).find_map(|part| match part {
            Part::Skinned { skeleton, .. } => match self.meshes.get(skeleton) {
                Some(MeshEntry::Skinned(b)) => Some(ed_anim::palette(&b.1, &ed_anim::sine_pose(&b.1, t, ed_anim::Sway::default()))),
                _ => None,
            },
            _ => None,
        })
    }
}
