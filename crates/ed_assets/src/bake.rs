//! Source TOML -> `.ewpack` (`ew-bake build` / `watch`).
//!
//! ```toml
//! [pack]
//! name = "proving"
//! game_packaged = false      # true: may hold "packaged game only" and megascans content
//! data_version = 1
//!
//! [proving]                  # the procedural proving ground (parts, materials, 100k layout)
//! registry = "everyworld-original"
//!
//! [[place]]                  # extra instances in the proving node (y-up proving space)
//! asset = "prefabs/planter_bench"
//! position = [6.0, 0.0, 318.0]
//! yaw_deg = 90.0
//!
//! [[gltf]]                   # path relative to this file, or path_env = "EW_SOLARCAT"
//! path = "../target/blender/bench.glb"
//! asset = "prefabs/planter_bench"
//! registry = "everyworld-original"
//! manifest = "../target/blender/bench.prefab.toml"   # optional footprint/anchor sidecar
//!
//! [[texture]]
//! asset = "surfaces/brick/albedo"
//! path = "brick.png"
//! role = "colour"            # colour | linear | normal | single
//! registry = "everyworld-original"
//!
//! [[material]]
//! asset = "surfaces/brick"
//! path = "brick.material.toml"
//! registry = "everyworld-original"
//!
//! [[megascans]]              # dir or dir_env; must be outside the repository
//! dir_env = "EW_MEGASCANS_ROCK"
//! asset = "surfaces/rock"
//! fab_id = "xxxxxxxx"
//! ```
//!
//! Every chunk carries a licence (the registry id) and provenance:
//! `src=<blake3 of the source path as written, or env:<VAR>/<file name>>`,
//! `content=<blake3 of the input bytes>`, `tool=ew-bake <version>`, `registry=<id>` and for
//! Megascans `fab-id=<id>`. Private absolute paths never enter a pack. Output is a pure
//! function of the inputs: no timestamps, sorted keys, deterministic encoders.

use crate::geom::{Lod, MeshBuild};
use crate::licence::{MEGASCANS, PackInfo, Registry, info_key};
use crate::lod::LodReport;
use crate::texture::{self, Role};
use crate::{GEN_VERSION, TOOL, collision, gltf_import, material, megascans, proving};
use anyhow::{Context, Result, anyhow, bail};
use ed_geo::{BodyId, NodeId, ObjectId};
use ed_pack::kinds::{Collision, CollisionShape, Instance, Instances, Material, PointKind, Prefab, PrefabPoint, Transform};
use ed_pack::{AssetId, Chunk, ChunkBody, ChunkHash, ChunkKey, ChunkKind, ChunkMeta, PackWriter, Rung};
use glam::{Affine3A, Quat, Vec3};
use rayon::prelude::*;
use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct PackSection {
    pub name: String,
    #[serde(default)]
    pub game_packaged: bool,
    #[serde(default = "one")]
    pub data_version: u32,
}
fn one() -> u32 {
    1
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct ProvingSrc {
    pub registry: String,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct PlaceSrc {
    pub asset: String,
    pub position: [f32; 3],
    #[serde(default)]
    pub yaw_deg: f32,
    #[serde(default = "unit")]
    pub scale: f32,
}
fn unit() -> f32 {
    1.0
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct GltfSrc {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub path_env: Option<String>,
    pub asset: String,
    pub registry: String,
    #[serde(default)]
    pub manifest: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct TexSrc {
    pub asset: String,
    pub path: String,
    pub role: String,
    pub registry: String,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct MatSrc {
    pub asset: String,
    pub path: String,
    pub registry: String,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct MegaSrc {
    #[serde(default)]
    pub dir: Option<String>,
    #[serde(default)]
    pub dir_env: Option<String>,
    pub asset: String,
    pub fab_id: String,
    #[serde(default = "megascans_id")]
    pub registry: String,
    #[serde(default)]
    pub normal_y: Option<String>,
}
fn megascans_id() -> String {
    MEGASCANS.into()
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct SourceFile {
    pub pack: PackSection,
    #[serde(default)]
    pub proving: Option<ProvingSrc>,
    #[serde(default)]
    pub place: Vec<PlaceSrc>,
    #[serde(default)]
    pub gltf: Vec<GltfSrc>,
    #[serde(default)]
    pub texture: Vec<TexSrc>,
    #[serde(default)]
    pub material: Vec<MatSrc>,
    #[serde(default)]
    pub megascans: Vec<MegaSrc>,
}

/// Prefab sidecar written next to a Blender export (`tools/blender/make_prefab.py`).
#[derive(Deserialize, Debug)]
pub struct PrefabManifest {
    #[serde(default)]
    pub footprint: Vec<[f32; 2]>,
    #[serde(default)]
    pub anchor: Option<[f32; 3]>,
}

/// What a build produced.
#[derive(Debug)]
pub struct BakeReport {
    pub name: String,
    pub game_packaged: bool,
    pub keys: usize,
    pub chunks: usize,
    pub bytes: u64,
    pub file_hash: ChunkHash,
    pub meshes: Vec<(String, Vec<LodReport>)>,
    pub textures: usize,
    /// Every file the build read (watch mode polls these).
    pub inputs: Vec<PathBuf>,
    pub seconds: f64,
}

/// Provenance of one source.
#[derive(Clone)]
struct Prov {
    src: String,
    content: String,
    registry: String,
    extra: Vec<String>,
}

struct Ctx {
    w: PackWriter,
    data_version: u32,
    game_packaged: bool,
    registry: Option<Registry>,
    licences: BTreeSet<String>,
    inputs: Vec<PathBuf>,
    meshes: Vec<(String, Vec<LodReport>)>,
    textures: usize,
}

impl Ctx {
    fn meta(&self, p: &Prov) -> ChunkMeta {
        let mut m = ChunkMeta::new(GEN_VERSION, self.data_version)
            .licence(&p.registry)
            .provenance(&format!("src={}", p.src))
            .provenance(&format!("content={}", p.content))
            .provenance(&format!("tool={TOOL}"))
            .provenance(&format!("registry={}", p.registry));
        for e in &p.extra {
            m = m.provenance(e);
        }
        m
    }
    fn licence(&mut self, id: &str) -> Result<()> {
        if let Some(r) = &self.registry {
            let errs = r.check(id, self.game_packaged);
            if !errs.is_empty() {
                bail!("{}", errs.join("; "));
            }
        }
        if self.licences.insert(id.to_string()) {
            self.w.add_licence(id)?;
        }
        Ok(())
    }
    fn put<'a, B: ChunkBody<'a>>(&mut self, key: ChunkKey, body: &B, p: &Prov) -> Result<()> {
        self.licence(&p.registry)?;
        let c = Chunk::encode(body, &self.meta(p))?;
        self.w.insert(key, c)?;
        Ok(())
    }
    fn put_mesh(&mut self, asset: &str, b: &MeshBuild, lods: &[Lod], report: Vec<LodReport>, p: &Prov) -> Result<()> {
        let m = b.to_pack(lods).with_context(|| format!("mesh {asset}"))?;
        self.put(ChunkKey::asset(AssetId::from_path(asset), ChunkKind::Mesh), &m, p)?;
        self.meshes.push((asset.to_string(), report));
        Ok(())
    }
    fn put_texture(&mut self, asset: &str, t: &ed_pack::kinds::Texture<'_>, p: &Prov) -> Result<()> {
        self.textures += 1;
        self.put(ChunkKey::asset(AssetId::from_path(asset), ChunkKind::Texture), t, p)
    }
    fn put_material(&mut self, asset: &str, m: &Material, p: &Prov) -> Result<()> {
        self.put(ChunkKey::asset(AssetId::from_path(asset), ChunkKind::Material), m, p)
    }
}

fn hash_hex(b: &[u8]) -> String {
    blake3::hash(b).to_hex().to_string()
}

/// The proving ground's node (instances key).
pub fn proving_node() -> NodeId {
    NodeId::from_unit(BodyId(3), proving::ORIGIN.normalize(), 10)
}

/// Nearest ancestor of `dir` with a `.git` entry (file in worktrees, folder otherwise).
fn repo_root(dir: &Path) -> Option<PathBuf> {
    dir.canonicalize().ok()?.ancestors().find(|a| a.join(".git").exists()).map(Path::to_path_buf)
}

/// Resolves `path` (relative to the source file) or `env` (a variable holding a path).
/// Returns the path and its provenance name (never an absolute private path).
fn resolve(dir: &Path, path: &Option<String>, env: &Option<String>, what: &str) -> Result<(PathBuf, String)> {
    match (path, env) {
        (Some(p), None) => Ok((dir.join(p), p.replace('\\', "/"))),
        (None, Some(v)) => {
            let p = std::env::var_os(v).ok_or_else(|| anyhow!("{what}: environment variable {v} is not set (it must hold the source path)"))?;
            let p = PathBuf::from(p);
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("source").to_string();
            Ok((p, format!("env:{v}/{name}")))
        }
        _ => bail!("{what}: give exactly one of `path` / `path_env` (or `dir` / `dir_env`)"),
    }
}

/// Parses a source file.
pub fn read_source(source: &Path) -> Result<SourceFile> {
    let text = std::fs::read_to_string(source).with_context(|| format!("reading {}", source.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing {}", source.display()))
}

/// Builds the pack in memory.
pub fn build_writer(source: &Path) -> Result<(PackWriter, BakeReport)> {
    let t0 = Instant::now();
    let src = read_source(source)?;
    let dir = source.parent().unwrap_or(Path::new(".")).to_path_buf();
    let src_name = source.file_name().and_then(|n| n.to_str()).unwrap_or("source.toml").to_string();
    let reg_dir = Registry::find(&dir).ok_or_else(|| anyhow!("no data/registry folder above {} (every chunk's licence id is checked against it)", dir.display()))?;
    let registry = Some(Registry::load(&reg_dir)?);
    let mut cx = Ctx {
        w: PackWriter::new(src.pack.data_version, GEN_VERSION),
        data_version: src.pack.data_version,
        game_packaged: src.pack.game_packaged,
        registry,
        licences: BTreeSet::new(),
        inputs: vec![source.to_path_buf()],
        meshes: Vec::new(),
        textures: 0,
    };

    // Pack info (our own metadata).
    let info = PackInfo { name: src.pack.name.clone(), game_packaged: src.pack.game_packaged, tool: TOOL.into() };
    let info_prov = Prov { src: hash_hex(src_name.as_bytes()), content: hash_hex(src.pack.name.as_bytes()), registry: "everyworld-original".into(), extra: vec![] };
    cx.licence(&info_prov.registry)?;
    let c = info.to_chunk(&cx.meta(&info_prov))?;
    cx.w.insert(info_key(), c)?;

    // Standalone textures.
    for t in &src.texture {
        let path = dir.join(&t.path);
        let bytes = std::fs::read(&path).with_context(|| format!("texture {}", t.asset))?;
        cx.inputs.push(path);
        let img = texture::decode_image(&bytes).with_context(|| format!("texture {}", t.asset))?;
        let tex = texture::bake(&img, Role::parse(&t.role)?, &AssetId::from_path(&t.asset))?;
        let p = Prov { src: hash_hex(t.path.as_bytes()), content: hash_hex(&bytes), registry: t.registry.clone(), extra: vec![] };
        cx.put_texture(&t.asset, &tex, &p)?;
    }
    // Material files.
    for m in &src.material {
        let path = dir.join(&m.path);
        let text = std::fs::read_to_string(&path).with_context(|| format!("material {}", m.asset))?;
        cx.inputs.push(path);
        let mat = material::parse(&text).with_context(|| format!("material {}", m.asset))?;
        let p = Prov { src: hash_hex(m.path.as_bytes()), content: hash_hex(text.as_bytes()), registry: m.registry.clone(), extra: vec![] };
        cx.put_material(&m.asset, &mat, &p)?;
    }
    // glTF.
    for g in &src.gltf {
        let (path, name) = resolve(&dir, &g.path, &g.path_env, &format!("gltf {}", g.asset))?;
        let bytes = std::fs::read(&path).with_context(|| format!("gltf {}: reading the source file", g.asset))?;
        cx.inputs.push(path.clone());
        let imp = gltf_import::import(&bytes, path.parent(), &g.asset).with_context(|| format!("gltf {}", g.asset))?;
        let p = Prov { src: hash_hex(name.as_bytes()), content: hash_hex(&bytes), registry: g.registry.clone(), extra: vec![] };
        for (a, t) in &imp.textures {
            cx.put_texture(a, t, &p)?;
        }
        for (a, m) in &imp.materials {
            cx.put_material(a, m, &p)?;
        }
        // Static (unskinned) prefabs get one box around all their mesh nodes, in the prefab
        // frame. Skinned characters are driven by the mover and get none.
        let prefab_collision = if imp.skeletons.is_empty() {
            let meshes: std::collections::BTreeMap<AssetId, &Vec<[f32; 3]>> = imp.meshes.iter().map(|m| (AssetId::from_path(&m.asset), &m.build.positions)).collect();
            let mut globals: Vec<Affine3A> = Vec::new();
            let mut pts = Vec::new();
            for n in &imp.prefab.1.nodes {
                let parent = n.parent.and_then(|k| globals.get(k as usize).copied()).unwrap_or(Affine3A::IDENTITY);
                let t = &n.transform;
                let g = parent * Affine3A::from_scale_rotation_translation(Vec3::from(t.scale), Quat::from_array(t.rotation), Vec3::from(t.translation));
                globals.push(g);
                if let Some(pos) = n.mesh.and_then(|m| meshes.get(&m)) {
                    pts.extend(pos.iter().map(|q| g.transform_point3(Vec3::from(*q))));
                }
            }
            collision::bounds(pts).map(|(lo, hi)| vec![collision::aabb_box(lo, hi, collision::surface::PROP)])
        } else {
            None
        };
        if let Some(shapes) = prefab_collision {
            cx.put(ChunkKey::asset(AssetId::from_path(&g.asset), ChunkKind::CollisionAsset), &Collision { for_asset: true, shapes }, &p)?;
        }
        for m in imp.meshes {
            cx.put_mesh(&m.asset, &m.build, &m.lods, m.report, &p)?;
        }
        for (a, s) in &imp.skeletons {
            cx.put(ChunkKey::asset(AssetId::from_path(a), ChunkKind::Skeleton), s, &p)?;
        }
        let mut prefab: Prefab = imp.prefab.1;
        let mut pp = p.clone();
        if let Some(mf) = &g.manifest {
            let mpath = dir.join(mf);
            let text = std::fs::read_to_string(&mpath).with_context(|| format!("prefab manifest {mf}"))?;
            cx.inputs.push(mpath);
            let man: PrefabManifest = toml::from_str(&text).with_context(|| format!("prefab manifest {mf}"))?;
            prefab.footprint = man.footprint;
            if let Some(a) = man.anchor {
                prefab.points.push(PrefabPoint { name: "anchor".into(), kind: PointKind::Anchor, transform: Transform { translation: a, ..Default::default() } });
            }
            pp.content = hash_hex(&[bytes.as_slice(), text.as_bytes()].concat());
        }
        cx.put(ChunkKey::asset(AssetId::from_path(&g.asset), ChunkKind::Prefab), &prefab, &pp)?;
    }
    // Megascans.
    for m in &src.megascans {
        if m.registry != MEGASCANS {
            bail!("megascans {}: registry must be `{MEGASCANS}`, not `{}`", m.asset, m.registry);
        }
        if !src.pack.game_packaged {
            bail!("megascans {}: Megascans content may only go into a pack with game_packaged = true", m.asset);
        }
        if m.fab_id.trim().is_empty() {
            bail!("megascans {}: fab_id (the Fab asset id) is required", m.asset);
        }
        let (mdir, name) = resolve(&dir, &m.dir, &m.dir_env, &format!("megascans {}", m.asset))?;
        let canon = mdir.canonicalize().with_context(|| format!("megascans {}: source folder", m.asset))?;
        if let Some(root) = repo_root(&dir)
            && canon.starts_with(&root)
        {
            bail!("megascans {}: the source folder is inside the repository ({}); keep Megascans sources outside it", m.asset, root.display());
        }
        let dx = match m.normal_y.as_deref().unwrap_or("gl") {
            "gl" => false,
            "dx" => true,
            v => bail!("megascans {}: normal_y must be gl or dx, not {v}", m.asset),
        };
        let s = megascans::import_dir(&canon, &m.asset, dx)?;
        let mut h = blake3::Hasher::new();
        for f in &s.files {
            h.update(&std::fs::read(f)?);
            cx.inputs.push(f.clone());
        }
        let p = Prov { src: hash_hex(name.as_bytes()), content: h.finalize().to_hex().to_string(), registry: m.registry.clone(), extra: vec![format!("fab-id={}", m.fab_id)] };
        for (a, t) in &s.textures {
            cx.put_texture(a, t, &p)?;
        }
        cx.put_material(&s.material.0, &s.material.1, &p)?;
    }
    // Proving ground.
    if let Some(pg) = &src.proving {
        let p = Prov { src: hash_hex(format!("{src_name}#proving").as_bytes()), content: hash_hex(format!("ed_assets::proving gen {GEN_VERSION}").as_bytes()), registry: pg.registry.clone(), extra: vec![] };
        let parts: Vec<_> = proving::PARTS.par_iter().map(|d| (d.name, proving::build_part(&(d.build)()))).collect();
        // Collision per part (asset frame), kept for placing below.
        let mut part_shapes: Vec<Vec<CollisionShape>> = Vec::with_capacity(parts.len());
        for (name, (b, lods, rep)) in parts {
            let shapes = proving::part_collision(name, &b.positions);
            cx.put(ChunkKey::asset(AssetId::from_path(&proving::part_asset(name)), ChunkKind::CollisionAsset), &Collision { for_asset: true, shapes: shapes.clone() }, &p)?;
            part_shapes.push(shapes);
            cx.put_mesh(&proving::part_asset(name), &b, &lods, rep, &p)?;
        }
        let cube = proving::cube();
        let ground_shapes = {
            let (lo, hi) = collision::bounds(cube.positions.iter().map(|q| Vec3::from(*q))).expect("cube has vertices");
            vec![collision::aabb_box(lo, hi, collision::surface::GROUND)]
        };
        cx.put(ChunkKey::asset(AssetId::from_path(proving::GROUND_ASSET), ChunkKind::CollisionAsset), &Collision { for_asset: true, shapes: ground_shapes.clone() }, &p)?;
        cx.put_mesh(proving::GROUND_ASSET, &cube, &[Lod { indices: cube.indices.clone(), error: 0.0 }], vec![], &p)?;
        for (i, m) in proving::materials().iter().enumerate() {
            cx.put_material(&proving::material_asset(i), m, &p)?;
        }
        let layout = proving::layout();
        let mut inst = Vec::with_capacity(layout.len() + src.place.len());
        for (i, pl) in layout.iter().enumerate() {
            let asset = if pl.part == usize::MAX { proving::GROUND_ASSET.to_string() } else { proving::part_asset(proving::PARTS[pl.part].name) };
            inst.push(Instance {
                object: proving::object(i),
                transform: proving::to_enu(&pl.transform),
                asset: AssetId::from_path(&asset),
                material: Some(AssetId::from_path(&proving::material_asset(pl.material))),
                variant: 0,
                flags: 0,
            });
        }
        for (k, pl) in src.place.iter().enumerate() {
            let a = Affine3A::from_scale_rotation_translation(Vec3::splat(pl.scale), Quat::from_rotation_y(pl.yaw_deg.to_radians()), Vec3::from(pl.position));
            inst.push(Instance {
                object: ObjectId(proving::OBJECT_BASE + (layout.len() + k) as u128),
                transform: proving::to_enu(&a),
                asset: AssetId::from_path(&pl.asset),
                material: None,
                variant: 0,
                flags: 0,
            });
        }
        // Node collision: every generated placement's part shapes in the node frame (the
        // game turns these into static bodies; it derives no collision of its own).
        let mut node_shapes = Vec::new();
        for (pl, i) in layout.iter().zip(&inst) {
            let shapes = if pl.part == usize::MAX { &ground_shapes } else { &part_shapes[pl.part] };
            node_shapes.extend(shapes.iter().map(|s| collision::place(s, &i.transform)));
        }
        cx.put(ChunkKey::node(proving_node(), ChunkKind::Collision, Rung::R0), &Collision { for_asset: false, shapes: node_shapes }, &p)?;
        let body = Instances::sorted(inst);
        cx.put(ChunkKey::node(proving_node(), ChunkKind::Instances, Rung::R0), &body, &p)?;
    } else if !src.place.is_empty() {
        bail!("[[place]] needs a [proving] section (places go into the proving node's instances)");
    }

    let report = BakeReport {
        name: src.pack.name,
        game_packaged: src.pack.game_packaged,
        keys: cx.w.key_count(),
        chunks: cx.w.chunk_count(),
        bytes: 0,
        file_hash: ChunkHash([0; 32]),
        meshes: cx.meshes,
        textures: cx.textures,
        inputs: cx.inputs,
        seconds: t0.elapsed().as_secs_f64(),
    };
    Ok((cx.w, report))
}

/// Builds `source` into `out` (atomic: temp file + rename).
pub fn build(source: &Path, out: &Path) -> Result<BakeReport> {
    let (w, mut rep) = build_writer(source)?;
    let (bytes, sum) = w.to_bytes()?;
    write_atomic(out, &bytes)?;
    rep.bytes = sum.bytes;
    rep.file_hash = sum.file_hash;
    Ok(rep)
}

/// Writes `bytes` to `out` through a temp file in the same folder and a rename, so a reader
/// never sees a half-written pack.
pub fn write_atomic(out: &Path, bytes: &[u8]) -> Result<()> {
    let dir = out.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(".{}.tmp-{}", out.file_name().and_then(|n| n.to_str()).unwrap_or("pack"), std::process::id()));
    std::fs::write(&tmp, bytes).with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::rename(&tmp, out).with_context(|| format!("renaming into {}", out.display()))?;
    Ok(())
}

/// BLAKE3 of a pack file (`ew-bake hash`).
pub fn file_hash(path: &Path) -> Result<String> {
    Ok(hash_hex(&std::fs::read(path).with_context(|| format!("reading {}", path.display()))?))
}

/// Modification times of `files` (missing files read as `None`).
pub fn stamps(files: &[PathBuf]) -> Vec<Option<std::time::SystemTime>> {
    files.iter().map(|f| std::fs::metadata(f).and_then(|m| m.modified()).ok()).collect()
}
