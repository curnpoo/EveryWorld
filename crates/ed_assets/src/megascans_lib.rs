//! The Megascans library: a folder of per-asset folders named by Fab id
//! (`EW_MEGASCANS_DIR`, or `megascans_dir` in the bake source), kind detection, `scan`, and
//! the importers for every kind.
//!
//! - **Kinds** ([`Kind`]): surface | 3d | 3d-plant | decal | imperfection. Read from a
//!   Megascans metadata JSON in the asset folder when there is one (`semanticTags.asset_type`,
//!   `type`, `assetType`, `categories`), otherwise inferred from file names ([`detect`]).
//! - **Maps** are found by file-name token (`<anything>_<4K>_<Albedo>[_LOD0].<png|jpg>`),
//!   anywhere in the folder (sub-folders included). With several resolutions the largest wins
//!   unless the source asks for one (`resolution = "2K"`). LOD-specific maps (`Normal_LOD0`)
//!   prefer LOD 0. Textures go through the surface path ([`crate::megascans::import_maps`]).
//! - **3D assets and plants** ([`import_model`]): FBX goes through headless Blender
//!   (`tools/blender/megascans/fbx_to_gltf.py`, embedded here) to GLB in a temporary folder;
//!   glTF/GLB is read directly. Geometry only ([`crate::gltf_import::read_geometry`]); every
//!   piece is assigned a LOD (`LOD<n>` in the node, mesh or file name; else 0) and a variant
//!   (`Var<n>`; plants ship several). The supplied LODs are the chain (at most
//!   [`MAX_LODS`], picked evenly in log-triangle count when more are supplied), each one's
//!   error **measured** against LOD 0 ([`crate::lod::measured_chain`]); coarser levels are
//!   generated only when fewer than [`MIN_LODS`] remain.
//! - Plants: opacity -> albedo alpha, alpha-tested (`AlphaMode::Mask`), double-sided,
//!   translucency slot; triangles reported per LOD, a warning (not a failure) when LOD 0 is over
//!   [`PLANT_WARN_TRIS`]. No billboards or impostors yet.
//! - Decals and imperfections: textures and a material only, tagged
//!   ([`crate::megascans::P_MEGASCANS_KIND`]).
//!
//! Engine-free and deterministic: folder listings are sorted, Blender output is only read for
//! geometry and ordered by (variant, LOD, file, node order).

use crate::geom::{Lod, MeshBuild};
use crate::gltf_import;
use crate::lod::{LodReport, measured_chain};
use crate::megascans::{MapOptions, Maps, Surface, import_maps};
use anyhow::{Context, Result, anyhow, bail};
use glam::Vec3;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Environment variable naming the library root (wins over the source's `megascans_dir`).
pub const ENV_DIR: &str = "EW_MEGASCANS_DIR";
/// Environment variable naming the Blender executable.
pub const ENV_BLENDER: &str = "EW_BLENDER";
/// Fewest LOD levels a mesh leaves the importer with (proving.rs / lod.rs: 3 or 4 levels).
pub const MIN_LODS: usize = 3;
/// Most LOD levels kept (the renderer reads 4: `ed_render::scene::MAX_LODS`).
pub const MAX_LODS: usize = 4;
/// Plants above this many LOD 0 triangles get a warning.
pub const PLANT_WARN_TRIS: u32 = 500_000;
/// The Blender conversion script, embedded so `ew-bake` needs no repository checkout.
pub const FBX_TO_GLTF: &str = include_str!("../../../tools/blender/megascans/fbx_to_gltf.py");

/// What a Megascans download is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Surface,
    Model3d,
    Plant,
    Decal,
    Imperfection,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Surface => "surface",
            Kind::Model3d => "3d",
            Kind::Plant => "3d-plant",
            Kind::Decal => "decal",
            Kind::Imperfection => "imperfection",
        }
    }
    pub fn parse(s: &str) -> Result<Kind> {
        Ok(match s {
            "surface" => Kind::Surface,
            "3d" => Kind::Model3d,
            "3d-plant" | "plant" => Kind::Plant,
            "decal" => Kind::Decal,
            "imperfection" => Kind::Imperfection,
            _ => bail!("unknown Megascans kind `{s}` (surface, 3d, 3d-plant, decal, imperfection)"),
        })
    }
    /// Value of the material tag [`crate::megascans::P_MEGASCANS_KIND`].
    pub fn code(self) -> f32 {
        match self {
            Kind::Surface => 0.0,
            Kind::Model3d => 1.0,
            Kind::Plant => 2.0,
            Kind::Decal => 3.0,
            Kind::Imperfection => 4.0,
        }
    }
    pub fn has_mesh(self) -> bool {
        matches!(self, Kind::Model3d | Kind::Plant)
    }
}

/// Map names (canonical, in report order) and the file-name tokens that mean them.
pub const MAP_NAMES: &[(&str, &[&str])] = &[
    ("Albedo", &["albedo", "basecolor", "basecolour", "diffuse"]),
    ("Normal", &["normal"]),
    ("Roughness", &["roughness"]),
    ("Gloss", &["gloss", "glossiness"]),
    ("AO", &["ao", "occlusion"]),
    ("Metalness", &["metalness", "metallic"]),
    ("Displacement", &["displacement", "height"]),
    ("Opacity", &["opacity", "alpha", "mask"]),
    ("Translucency", &["translucency", "transmission"]),
    ("Cavity", &["cavity"]),
    ("Specular", &["specular"]),
    ("Bump", &["bump"]),
    ("Fuzz", &["fuzz"]),
];

const IMAGE_EXT: &[&str] = &["png", "jpg", "jpeg", "webp", "exr", "tif", "tiff", "tga"];
const DECODABLE_EXT: &[&str] = &["png", "jpg", "jpeg", "webp"];
const MESH_EXT: &[&str] = &["fbx", "gltf", "glb", "obj", "abc"];

/// One image file in an asset folder.
#[derive(Clone, Debug, PartialEq)]
pub struct MapFile {
    pub map: &'static str,
    pub path: PathBuf,
    /// Resolution token in pixels (`4K` -> 4096), when the name has one.
    pub res: Option<u32>,
    pub lod: Option<u32>,
    pub ext: String,
}

/// One mesh file in an asset folder.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshFile {
    pub path: PathBuf,
    pub ext: String,
    pub lod: Option<u32>,
    pub variant: Option<u32>,
    /// A `_High` source scan (skipped when LOD files exist).
    pub high: bool,
}

/// Everything in one asset folder (sorted, sub-folders included up to 4 deep).
#[derive(Clone, Debug, Default)]
pub struct Listing {
    pub dir: PathBuf,
    pub files: Vec<PathBuf>,
    pub maps: Vec<MapFile>,
    pub meshes: Vec<MeshFile>,
    pub metadata: Vec<PathBuf>,
    pub archives: Vec<PathBuf>,
    pub bytes: u64,
}

fn ext_of(p: &Path) -> String {
    p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase()
}

/// Lower-case name tokens (split on `_ - . space`).
fn tokens(s: &str) -> Vec<String> {
    s.split(['_', '-', '.', ' ', '/', '\\']).filter(|t| !t.is_empty()).map(|t| t.to_ascii_lowercase()).collect()
}

/// `<prefix><digits>` as one token, or `<prefix>` followed by a digits token.
fn numbered(s: &str, prefixes: &[&str]) -> Option<u32> {
    let t = tokens(s);
    for (i, tok) in t.iter().enumerate() {
        for p in prefixes {
            if let Some(rest) = tok.strip_prefix(p) {
                if !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()) {
                    return rest.parse().ok();
                }
                if rest.is_empty()
                    && let Some(n) = t.get(i + 1).filter(|n| n.bytes().all(|b| b.is_ascii_digit()))
                {
                    return n.parse().ok();
                }
            }
        }
    }
    None
}

/// `LOD<n>` in a name.
pub fn lod_of(s: &str) -> Option<u32> {
    numbered(s, &["lod"])
}

/// `Var<n>` / `Variation<n>` in a name.
pub fn variant_of(s: &str) -> Option<u32> {
    numbered(s, &["variation", "var"])
}

/// `4K` -> 4096 (a token of digits followed by `k`).
pub fn res_of(stem: &str) -> Option<u32> {
    tokens(stem).iter().find_map(|t| t.strip_suffix('k').filter(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit())).and_then(|d| d.parse::<u32>().ok()).map(|k| k * 1024))
}

/// The map a file stem names (its last token that is a map name).
pub fn map_of(stem: &str) -> Option<&'static str> {
    tokens(stem).iter().rev().find_map(|t| MAP_NAMES.iter().find(|(_, keys)| keys.contains(&t.as_str())).map(|(n, _)| *n))
}

fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) -> Result<()> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))?.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for p in entries {
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.starts_with('.') || name == "__MACOSX" {
            continue;
        }
        if p.is_dir() {
            if depth < 4 {
                walk(&p, depth + 1, out)?;
            }
        } else {
            out.push(p);
        }
    }
    Ok(())
}

impl Listing {
    pub fn read(dir: &Path) -> Result<Listing> {
        let mut files = Vec::new();
        walk(dir, 0, &mut files)?;
        let mut l = Listing { dir: dir.to_path_buf(), ..Default::default() };
        for p in &files {
            l.bytes += std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
            let ext = ext_of(p);
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let rel = p.strip_prefix(dir).unwrap_or(p).to_string_lossy().replace('\\', "/");
            if IMAGE_EXT.contains(&ext.as_str()) {
                if let Some(map) = map_of(stem) {
                    l.maps.push(MapFile { map, path: p.clone(), res: res_of(stem), lod: lod_of(stem), ext });
                }
            } else if MESH_EXT.contains(&ext.as_str()) {
                let toks = tokens(stem);
                let high = toks.iter().any(|t| t == "high" || t == "highpoly");
                l.meshes.push(MeshFile { path: p.clone(), ext, lod: lod_of(stem), variant: variant_of(&rel), high });
            } else if ext == "json" {
                l.metadata.push(p.clone());
            } else if ext == "zip" {
                l.archives.push(p.clone());
            }
        }
        l.files = files;
        Ok(l)
    }
    fn has(&self, map: &str) -> bool {
        self.maps.iter().any(|m| m.map == map)
    }
    fn rel(&self, p: &Path) -> String {
        p.strip_prefix(&self.dir).unwrap_or(p).to_string_lossy().replace('\\', "/")
    }
}

/// How the kind was decided.
#[derive(Clone, Debug, PartialEq)]
pub enum KindSource {
    Metadata(PathBuf),
    FileNames,
    Manifest,
}

/// Fields of a Megascans metadata JSON that name the asset type.
fn metadata_kind(v: &serde_json::Value) -> Option<Kind> {
    let mut words: Vec<String> = Vec::new();
    for p in ["/semanticTags/asset_type", "/type", "/assetType", "/asset_type", "/assetCategory"] {
        if let Some(s) = v.pointer(p).and_then(|s| s.as_str()) {
            words.push(s.to_string());
        }
    }
    for p in ["/categories", "/assetCategories", "/tags/categories"] {
        if let Some(a) = v.pointer(p).and_then(|a| a.as_array()) {
            words.extend(a.iter().filter_map(|s| s.as_str()).map(str::to_string));
        }
    }
    let norm: Vec<String> = words.iter().map(|w| w.to_ascii_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect()).collect();
    let any = |keys: &[&str]| norm.iter().any(|w| keys.contains(&w.as_str()));
    if any(&["3dplant", "3dplants", "plant", "plants", "megaplant", "megaplants"]) {
        Some(Kind::Plant)
    } else if any(&["imperfection", "imperfections"]) {
        Some(Kind::Imperfection)
    } else if any(&["decal", "decals", "atlas", "atlases"]) {
        Some(Kind::Decal)
    } else if any(&["3d", "3dasset", "3dassets", "3dmodel", "model", "mesh"]) {
        Some(Kind::Model3d)
    } else if any(&["surface", "surfaces", "material", "materials"]) {
        Some(Kind::Surface)
    } else {
        None
    }
}

/// Detects the kind: metadata JSON first, then file names.
pub fn detect(l: &Listing) -> (Kind, KindSource) {
    for m in &l.metadata {
        if let Some(k) = std::fs::read(m).ok().and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok()).as_ref().and_then(metadata_kind) {
            return (k, KindSource::Metadata(m.clone()));
        }
    }
    let has_mesh = !l.meshes.is_empty();
    let k = if has_mesh {
        if l.has("Opacity") || l.has("Translucency") || l.meshes.iter().any(|m| m.variant.is_some()) { Kind::Plant } else { Kind::Model3d }
    } else if !l.has("Albedo") {
        Kind::Imperfection
    } else if l.has("Opacity") && !l.has("Displacement") {
        Kind::Decal
    } else {
        Kind::Surface
    };
    (k, KindSource::FileNames)
}

/// Picks one file per map: decodable formats only, LOD 0 (or no LOD) first, then the wanted
/// resolution (or the largest), then path order. Returns the maps and notes for the report.
pub fn select_maps(l: &Listing, want_res: Option<u32>) -> (Maps, Vec<String>) {
    let mut notes = Vec::new();
    let mut pick = |map: &str| -> Option<PathBuf> {
        let all: Vec<&MapFile> = l.maps.iter().filter(|m| m.map == map).collect();
        if all.is_empty() {
            return None;
        }
        let ok: Vec<&MapFile> = all.iter().copied().filter(|m| DECODABLE_EXT.contains(&m.ext.as_str())).collect();
        if ok.is_empty() {
            notes.push(format!("{map} only as {} (not readable): export PNG or JPEG", all[0].ext.to_uppercase()));
            return None;
        }
        let lod0: Vec<&MapFile> = ok.iter().copied().filter(|m| m.lod.unwrap_or(0) == 0).collect();
        let pool = if lod0.is_empty() { ok } else { lod0 };
        let pool: Vec<&MapFile> = match want_res {
            Some(r) if pool.iter().any(|m| m.res == Some(r)) => pool.iter().copied().filter(|m| m.res == Some(r)).collect(),
            Some(r) => {
                notes.push(format!("{map}: no {}K file, using the largest", r / 1024));
                pool
            }
            None => pool,
        };
        let best = pool.iter().map(|m| m.res.unwrap_or(0)).max().unwrap_or(0);
        let choice = pool.iter().find(|m| m.res.unwrap_or(0) == best).expect("non-empty");
        let res: std::collections::BTreeSet<u32> = pool.iter().filter_map(|m| m.res).collect();
        if res.len() > 1 {
            notes.push(format!("{map}: {} resolutions, using {}", res.len(), l.rel(&choice.path)));
        }
        Some(choice.path.clone())
    };
    let maps = Maps {
        albedo: pick("Albedo"),
        normal: pick("Normal"),
        roughness: pick("Roughness"),
        gloss: pick("Gloss"),
        ao: pick("AO"),
        metalness: pick("Metalness"),
        displacement: pick("Displacement"),
        opacity: pick("Opacity"),
        translucency: pick("Translucency"),
    };
    (maps, notes)
}

/// Expands a leading `~/`.
fn expand_home(s: &str) -> PathBuf {
    match (s.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(h)) => PathBuf::from(h).join(rest),
        _ => PathBuf::from(s),
    }
}

/// The library root: `EW_MEGASCANS_DIR` if set, else the source's `megascans_dir` (relative to
/// the source file's folder, `~/` expanded).
pub fn library_root(manifest_value: Option<&str>, source_dir: &Path) -> Option<PathBuf> {
    if let Some(v) = std::env::var_os(ENV_DIR).filter(|v| !v.is_empty()) {
        return Some(expand_home(&v.to_string_lossy()));
    }
    manifest_value.map(|v| source_dir.join(expand_home(v)))
}

/// The folder of asset `fab_id` in the library: `<root>/<fab_id>`, else the one folder whose
/// name contains the id (Bridge-style `<Name>_<id>`), case-insensitive.
pub fn find_asset(root: &Path, fab_id: &str) -> Result<PathBuf> {
    let exact = root.join(fab_id);
    if exact.is_dir() {
        return Ok(exact);
    }
    let id = fab_id.to_ascii_lowercase();
    let mut hits: Vec<PathBuf> = std::fs::read_dir(root)
        .with_context(|| format!("Megascans library {}", root.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir() && p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.to_ascii_lowercase().contains(&id)))
        .collect();
    hits.sort();
    match hits.len() {
        1 => Ok(hits.remove(0)),
        0 => bail!("no folder named {fab_id} in the Megascans library {}", root.display()),
        _ => bail!("{} folders in {} match {fab_id}: {:?}; rename the one you mean to the bare id", hits.len(), root.display(), hits),
    }
}

/// The Blender executable: `EW_BLENDER`, the macOS app, or `blender` on PATH.
pub fn blender_path() -> Option<PathBuf> {
    if let Some(v) = std::env::var_os(ENV_BLENDER).filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(v));
    }
    let mac = PathBuf::from("/Applications/Blender.app/Contents/MacOS/Blender");
    if mac.is_file() {
        return Some(mac);
    }
    std::env::var_os("PATH").and_then(|p| std::env::split_paths(&p).map(|d| d.join("blender")).find(|b| b.is_file()))
}

/// Converts FBX files to GLB with headless Blender: `out/<i>.glb` per input, in order.
pub fn convert_fbx(blender: &Path, files: &[PathBuf], out: &Path) -> Result<Vec<PathBuf>> {
    std::fs::create_dir_all(out)?;
    let script = out.join("fbx_to_gltf.py");
    std::fs::write(&script, FBX_TO_GLTF)?;
    let mut cmd = Command::new(blender);
    cmd.args(["--background", "--factory-startup", "--python-exit-code", "1", "--python"]).arg(&script).arg("--").arg("--out").arg(out);
    cmd.args(files);
    let o = cmd.output().with_context(|| format!("running Blender ({})", blender.display()))?;
    let outs: Vec<PathBuf> = (0..files.len()).map(|i| out.join(format!("{i}.glb"))).collect();
    if !o.status.success() || outs.iter().any(|p| !p.is_file()) {
        let log = String::from_utf8_lossy(&o.stderr).to_string() + &String::from_utf8_lossy(&o.stdout);
        let tail: Vec<&str> = log.lines().rev().take(15).collect();
        bail!("Blender FBX conversion failed ({}):\n{}", o.status, tail.into_iter().rev().collect::<Vec<_>>().join("\n"));
    }
    Ok(outs)
}

/// A temporary folder, removed on drop.
struct TempDir(PathBuf);
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Model import options.
#[derive(Clone, Debug)]
pub struct ModelOptions {
    /// Multiplies positions (glTF sources in centimetres: 0.01). FBX unit scale is applied by
    /// Blender already.
    pub scale: f32,
}

impl Default for ModelOptions {
    fn default() -> Self {
        ModelOptions { scale: 1.0 }
    }
}

/// One variant's mesh with its chain.
pub struct ModelVariant {
    /// `main` (single variant) or `var<N>`.
    pub label: String,
    pub build: MeshBuild,
    pub lods: Vec<Lod>,
    pub report: Vec<LodReport>,
    /// Supplied LOD numbers kept (generated levels are not listed).
    pub supplied: Vec<u32>,
    /// Supplied LOD numbers found (before picking).
    pub found: Vec<u32>,
}

pub struct Model {
    pub variants: Vec<ModelVariant>,
    /// Mesh source files read (FBX or glTF, plus glTF buffers), sorted.
    pub sources: Vec<PathBuf>,
    pub via_blender: bool,
    pub notes: Vec<String>,
}

/// The mesh files a model import reads: glTF/GLB when present (no Blender), else FBX; `_High`
/// scans are skipped when LOD files exist.
pub fn model_sources(l: &Listing) -> Result<Vec<&MeshFile>> {
    let gltf: Vec<&MeshFile> = l.meshes.iter().filter(|m| m.ext == "gltf" || m.ext == "glb").collect();
    let fbx: Vec<&MeshFile> = l.meshes.iter().filter(|m| m.ext == "fbx").collect();
    let mut pick = if !gltf.is_empty() { gltf } else { fbx };
    if pick.is_empty() {
        let exts: Vec<&str> = l.meshes.iter().map(|m| m.ext.as_str()).collect();
        bail!("no FBX or glTF mesh in {} (found {exts:?}); download the FBX or glTF format", l.dir.display());
    }
    if pick.iter().any(|m| !m.high) {
        pick.retain(|m| !m.high);
    }
    Ok(pick)
}

/// Picks at most `max` of `n` levels (by triangle count, decreasing): the first, the last, and
/// the ones closest to an even spread in log-triangle count between them.
fn pick_levels(tris: &[usize], max: usize) -> Vec<usize> {
    let n = tris.len();
    if n <= max {
        return (0..n).collect();
    }
    let (a, b) = ((tris[0].max(1) as f64).ln(), (tris[n - 1].max(1) as f64).ln());
    let mut out = vec![0usize];
    for j in 1..max - 1 {
        let target = a + (b - a) * j as f64 / (max - 1) as f64;
        let lo = out.last().expect("first") + 1;
        let hi = n - 1 - (max - 1 - j); // leave room for the remaining picks
        let best = (lo..hi.max(lo + 1).min(n - 1)).min_by(|&x, &y| {
            let dx = ((tris[x].max(1) as f64).ln() - target).abs();
            let dy = ((tris[y].max(1) as f64).ln() - target).abs();
            dx.total_cmp(&dy)
        });
        out.push(best.unwrap_or(lo));
    }
    out.push(n - 1);
    out.dedup();
    out
}

/// Geometry of one LOD of one variant, merged from its pieces.
#[derive(Default)]
struct Merged {
    build: MeshBuild,
    any_uv: bool,
}

impl Merged {
    fn add(&mut self, b: &MeshBuild, scale: f32) {
        let base = self.build.positions.len() as u32;
        let had_uv = !self.build.uvs.is_empty() || self.any_uv;
        if !b.uvs.is_empty() && !had_uv {
            self.build.uvs = vec![[0.0; 2]; self.build.positions.len()];
        }
        self.any_uv |= !b.uvs.is_empty();
        self.build.positions.extend(b.positions.iter().map(|p| [p[0] * scale, p[1] * scale, p[2] * scale]));
        self.build.normals.extend_from_slice(&b.normals);
        if self.any_uv {
            if b.uvs.is_empty() {
                self.build.uvs.extend(std::iter::repeat_n([0.0; 2], b.positions.len()));
            } else {
                self.build.uvs.extend_from_slice(&b.uvs);
            }
        }
        self.build.indices.extend(b.indices.iter().map(|i| i + base));
    }
}

/// Imports a 3D asset or plant's meshes as LOD chains, one per variant.
pub fn import_model(l: &Listing, o: &ModelOptions) -> Result<Model> {
    let files = model_sources(l)?;
    let mut notes = Vec::new();
    let via_blender = files.iter().any(|f| f.ext == "fbx");
    // GLB per source file (FBX through Blender into a temporary folder).
    let tmp;
    let glbs: Vec<PathBuf> = if via_blender {
        let blender = blender_path().ok_or_else(|| anyhow!("{}: FBX needs Blender (set {ENV_BLENDER}, or download the glTF format instead)", l.dir.display()))?;
        let key = blake3::hash(l.dir.to_string_lossy().as_bytes()).to_hex();
        tmp = TempDir(std::env::temp_dir().join(format!("ew-megascans-{}-{}", std::process::id(), &key[..12])));
        let _ = std::fs::remove_dir_all(&tmp.0);
        let paths: Vec<PathBuf> = files.iter().map(|f| f.path.clone()).collect();
        convert_fbx(&blender, &paths, &tmp.0)?
    } else {
        files.iter().map(|f| f.path.clone()).collect()
    };
    // (variant, lod) -> pieces in (file, node) order.
    let mut groups: BTreeMap<(u32, u32), Merged> = BTreeMap::new();
    let mut sources: Vec<PathBuf> = files.iter().map(|f| f.path.clone()).collect();
    for (f, glb) in files.iter().zip(&glbs) {
        let bytes = std::fs::read(glb).with_context(|| format!("reading {}", glb.display()))?;
        let dir = f.path.parent();
        if f.ext == "gltf" {
            // External buffers are inputs too (content hash).
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes)
                && let Some(bufs) = v.get("buffers").and_then(|b| b.as_array())
            {
                for b in bufs {
                    if let (Some(u), Some(d)) = (b.get("uri").and_then(|u| u.as_str()), dir)
                        && !u.starts_with("data:")
                    {
                        sources.push(d.join(u.replace("%20", " ")));
                    }
                }
            }
        }
        let pieces = gltf_import::read_geometry(&bytes, dir).with_context(|| l.rel(&f.path))?;
        if pieces.is_empty() {
            notes.push(format!("{}: no meshes", l.rel(&f.path)));
        }
        let rel = l.rel(&f.path);
        for p in pieces {
            let lod = lod_of(&p.node_name).or_else(|| lod_of(&p.mesh_name)).or(f.lod).unwrap_or(0);
            let var = variant_of(&p.node_name).or_else(|| variant_of(&p.mesh_name)).or(f.variant).or_else(|| variant_of(&rel)).unwrap_or(0);
            groups.entry((var, lod)).or_default().add(&p.build, o.scale);
        }
    }
    sources.sort();
    sources.dedup();
    let mut by_var: BTreeMap<u32, Vec<(u32, MeshBuild)>> = BTreeMap::new();
    for ((v, lod), m) in groups {
        if !m.build.indices.is_empty() {
            by_var.entry(v).or_default().push((lod, m.build));
        }
    }
    if by_var.is_empty() {
        bail!("{}: no triangles in the mesh files", l.dir.display());
    }
    let single = by_var.len() == 1;
    let mut variants = Vec::new();
    for (v, levels) in by_var {
        let label = if single { "main".to_string() } else { format!("var{v}") };
        let found: Vec<u32> = levels.iter().map(|(k, _)| *k).collect();
        let tris: Vec<usize> = levels.iter().map(|(_, b)| b.indices.len() / 3).collect();
        let keep = pick_levels(&tris, MAX_LODS);
        // One vertex array: the kept levels' vertices back to back.
        let mut all = MeshBuild::default();
        let any_uv = keep.iter().any(|&k| !levels[k].1.uvs.is_empty());
        let mut supplied_idx = Vec::new();
        for &k in &keep {
            let b = &levels[k].1;
            let base = all.positions.len() as u32;
            all.positions.extend_from_slice(&b.positions);
            all.normals.extend_from_slice(&b.normals);
            if any_uv {
                if b.uvs.is_empty() {
                    all.uvs.extend(std::iter::repeat_n([0.0; 2], b.positions.len()));
                } else {
                    all.uvs.extend_from_slice(&b.uvs);
                }
            }
            supplied_idx.push(b.indices.iter().map(|i| i + base).collect::<Vec<u32>>());
        }
        let (lods, report, kept) = measured_chain(&all.positions, &supplied_idx, MIN_LODS);
        all.indices = lods[0].indices.clone();
        all.validate().with_context(|| format!("{}: variant {label}", l.dir.display()))?;
        let supplied: Vec<u32> = kept.iter().map(|&i| found[keep[i]]).collect();
        if found.len() > 1 && !found.contains(&0) {
            notes.push(format!("{label}: no LOD0 file; finest supplied is LOD{}", found[0]));
        }
        let generated = lods.len() - kept.len();
        if generated > 0 {
            notes.push(format!("{label}: {} supplied LOD(s), generated {generated} coarser", kept.len()));
        }
        let ext = all.bounds();
        let size = Vec3::from(ext.max) - Vec3::from(ext.min);
        if size.max_element() > 100.0 {
            notes.push(format!("{label}: {:.0} m across; if the source is in centimetres set scale = 0.01", size.max_element()));
        }
        variants.push(ModelVariant { label, build: all, lods, report, supplied, found });
    }
    Ok(Model { variants, sources, via_blender, notes })
}

/// Map options for a kind (plants: alpha test when there is an opacity map).
pub fn map_options(kind: Kind, maps: &Maps, normal_dx: bool, material_asset: Option<String>) -> MapOptions {
    let mut o = MapOptions { normal_dx, material_asset, ..Default::default() };
    match kind {
        Kind::Surface => {}
        Kind::Model3d => o.kind_tag = Some(kind.code()),
        Kind::Plant => {
            o.kind_tag = Some(kind.code());
            o.alpha_test = maps.opacity.as_ref().map(|_| 0.5);
            o.double_sided = true;
            o.translucency = true;
        }
        Kind::Decal => {
            o.kind_tag = Some(kind.code());
            o.alpha_blend = maps.opacity.is_some();
        }
        Kind::Imperfection => {
            o.kind_tag = Some(kind.code());
            o.albedo_optional = true;
        }
    }
    o
}

/// Textures + material for any kind (the surface path).
pub fn import_textures(kind: Kind, maps: &Maps, base: &str, normal_dx: bool, material_asset: Option<String>) -> Result<Surface> {
    import_maps(maps, base, &map_options(kind, maps, normal_dx, material_asset))
}

// ---------------------------------------------------------------------------------------------
// scan

/// One asset folder as `ew-bake megascans scan` reports it.
#[derive(Clone, Debug)]
pub struct ScanEntry {
    pub id: String,
    pub kind: Kind,
    pub kind_source: KindSource,
    /// `Albedo 4K jpg 4096x4096` per map file.
    pub maps: Vec<String>,
    /// Mesh files by format with LODs and variants, e.g. `fbx LOD0-LOD5`.
    pub lods: Vec<String>,
    /// Triangles per LOD (glTF sources only; FBX needs Blender, which scan does not run).
    pub triangles: Vec<String>,
    pub bytes: u64,
    pub problems: Vec<String>,
}

fn lod_ranges(mut v: Vec<u32>) -> String {
    v.sort();
    v.dedup();
    v.iter().map(|k| format!("LOD{k}")).collect::<Vec<_>>().join(" ")
}

/// Scans one asset folder (reads names, image headers and glTF geometry; writes nothing).
pub fn scan_asset(dir: &Path) -> Result<ScanEntry> {
    let id = dir.file_name().and_then(|n| n.to_str()).unwrap_or("?").to_string();
    let l = Listing::read(dir)?;
    let (kind, kind_source) = detect(&l);
    let mut problems = Vec::new();
    if l.files.is_empty() {
        problems.push("empty folder".into());
    }
    for a in &l.archives {
        problems.push(format!("unextracted archive {}", l.rel(a)));
    }
    let mut maps = Vec::new();
    for m in &l.maps {
        let dims = if DECODABLE_EXT.contains(&m.ext.as_str()) {
            image::image_dimensions(&m.path).map(|(w, h)| format!(" {w}x{h}")).unwrap_or_else(|_| {
                problems.push(format!("{}: unreadable image", l.rel(&m.path)));
                String::new()
            })
        } else {
            String::new()
        };
        let res = m.res.map(|r| format!(" {}K", r / 1024)).unwrap_or_default();
        let lod = m.lod.map(|k| format!(" LOD{k}")).unwrap_or_default();
        maps.push(format!("{}{res}{lod} {}{dims}", m.map, m.ext));
    }
    let (sel, notes) = select_maps(&l, None);
    problems.extend(notes.into_iter().filter(|n| !n.contains("resolutions")));
    let needs_albedo = !matches!(kind, Kind::Imperfection);
    if needs_albedo && sel.albedo.is_none() {
        problems.push("no readable Albedo map".into());
    }
    if kind == Kind::Plant && sel.opacity.is_none() {
        problems.push("plant without an Opacity map (leaves will be opaque)".into());
    }
    let mut lods = Vec::new();
    let mut triangles = Vec::new();
    if kind.has_mesh() {
        let mut by_ext: BTreeMap<&str, Vec<&MeshFile>> = BTreeMap::new();
        for m in &l.meshes {
            by_ext.entry(m.ext.as_str()).or_default().push(m);
        }
        for (ext, ms) in &by_ext {
            let mut vars: Vec<u32> = ms.iter().filter_map(|m| m.variant).collect();
            vars.sort();
            vars.dedup();
            let high = ms.iter().filter(|m| m.high).count();
            let file_lods: Vec<u32> = ms.iter().filter_map(|m| m.lod).collect();
            let mut s = format!("{ext}: {} file(s)", ms.len());
            if !file_lods.is_empty() {
                let _ = write!(s, ", {}", lod_ranges(file_lods));
            }
            if !vars.is_empty() {
                let _ = write!(s, ", variants {}", vars.iter().map(|v| format!("Var{v}")).collect::<Vec<_>>().join(" "));
            }
            if high > 0 {
                let _ = write!(s, ", {high} high-poly scan(s) (skipped)");
            }
            lods.push(s);
        }
        match model_sources(&l) {
            Ok(src) if src.iter().all(|f| f.ext != "fbx") => {
                let mut per: BTreeMap<(u32, u32), usize> = BTreeMap::new();
                let mut extent = 0.0f32;
                for f in src {
                    let read = std::fs::read(&f.path).map_err(anyhow::Error::from).and_then(|b| gltf_import::read_geometry(&b, f.path.parent()));
                    match read {
                        Ok(pieces) => {
                            for p in pieces {
                                let lod = lod_of(&p.node_name).or_else(|| lod_of(&p.mesh_name)).or(f.lod).unwrap_or(0);
                                let var = variant_of(&p.node_name).or_else(|| variant_of(&p.mesh_name)).or(f.variant).unwrap_or(0);
                                *per.entry((var, lod)).or_default() += p.build.indices.len() / 3;
                                let b = p.build.bounds();
                                extent = extent.max((Vec3::from(b.max) - Vec3::from(b.min)).max_element());
                            }
                        }
                        Err(e) => problems.push(format!("{}: {e:#}", l.rel(&f.path))),
                    }
                }
                for ((v, k), t) in &per {
                    let var = if per.keys().any(|(v2, _)| *v2 != 0) { format!("Var{v} ") } else { String::new() };
                    triangles.push(format!("{var}LOD{k} {t}"));
                }
                if kind == Kind::Plant && per.iter().any(|((_, k), t)| *k == 0 && *t > PLANT_WARN_TRIS as usize) {
                    problems.push(format!("LOD0 over {PLANT_WARN_TRIS} triangles (heavy plant; imports with a warning)"));
                }
                if extent > 100.0 {
                    problems.push(format!("{extent:.0} units across: probably centimetres, set scale = 0.01"));
                }
                if !per.is_empty() && per.keys().filter(|(v, _)| *v == per.keys().next().expect("one").0).count() < MIN_LODS {
                    problems.push(format!("fewer than {MIN_LODS} LODs: coarser ones will be generated"));
                }
            }
            Ok(_) => {
                if blender_path().is_none() {
                    problems.push(format!("FBX needs Blender, not found (set {ENV_BLENDER}) or download glTF"));
                }
                triangles.push("(FBX: counted at bake time)".into());
            }
            Err(e) => problems.push(format!("{e:#}")),
        }
    }
    // Metadata id vs folder name.
    for m in &l.metadata {
        if let Some(mid) = std::fs::read(m).ok().and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok()).and_then(|v| v.get("id").and_then(|i| i.as_str()).map(str::to_string))
            && !id.to_ascii_lowercase().contains(&mid.to_ascii_lowercase())
        {
            problems.push(format!("metadata id {mid} differs from the folder name"));
        }
    }
    Ok(ScanEntry { id, kind, kind_source, maps, lods, triangles, bytes: l.bytes, problems })
}

/// Scans every sub-folder of the library root (sorted).
pub fn scan(root: &Path) -> Result<Vec<ScanEntry>> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(root)
        .with_context(|| format!("Megascans library {}", root.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir() && !p.file_name().and_then(|n| n.to_str()).unwrap_or(".").starts_with('.'))
        .collect();
    dirs.sort();
    dirs.iter().map(|d| scan_asset(d)).collect()
}

fn human(b: u64) -> String {
    match b {
        b if b >= 1 << 30 => format!("{:.1} GB", b as f64 / (1u64 << 30) as f64),
        b if b >= 1 << 20 => format!("{:.1} MB", b as f64 / (1u64 << 20) as f64),
        b if b >= 1 << 10 => format!("{:.1} KB", b as f64 / 1024.0),
        b => format!("{b} B"),
    }
}

/// The scan report as text.
pub fn format_scan(root: &Path, entries: &[ScanEntry]) -> String {
    let mut s = format!("Megascans library {}\n", root.display());
    for e in entries {
        let src = match &e.kind_source {
            KindSource::Metadata(p) => format!("metadata {}", p.file_name().and_then(|n| n.to_str()).unwrap_or("?")),
            KindSource::FileNames => "file names".into(),
            KindSource::Manifest => "manifest".into(),
        };
        let _ = writeln!(s, "{}  kind={} ({src})  {}", e.id, e.kind.as_str(), human(e.bytes));
        if !e.maps.is_empty() {
            let _ = writeln!(s, "  maps: {}", e.maps.join(", "));
        }
        for l in &e.lods {
            let _ = writeln!(s, "  mesh {l}");
        }
        if !e.triangles.is_empty() {
            let _ = writeln!(s, "  triangles: {}", e.triangles.join(", "));
        }
        for p in &e.problems {
            let _ = writeln!(s, "  problem: {p}");
        }
    }
    let bad = entries.iter().filter(|e| !e.problems.is_empty()).count();
    let _ = writeln!(s, "{} asset(s), {} with problems", entries.len(), bad);
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_tokens() {
        assert_eq!(lod_of("SM_rock_LOD3"), Some(3));
        assert_eq!(lod_of("rock_LOD_2.001"), Some(2));
        assert_eq!(lod_of("lodge_LOD"), None);
        assert_eq!(variant_of("Var12/Var12_LOD0"), Some(12));
        assert_eq!(res_of("ab12c_4K_Albedo"), Some(4096));
        assert_eq!(map_of("ab12c_4K_Normal_LOD0"), Some("Normal"));
        assert_eq!(map_of("ab12c_2K_Gloss"), Some("Gloss"));
        assert_eq!(map_of("ab12c_preview"), None);
    }

    #[test]
    fn level_picking_spreads_and_keeps_ends() {
        assert_eq!(pick_levels(&[1000, 500, 100], 4), [0, 1, 2]);
        let p = pick_levels(&[100_000, 50_000, 20_000, 6000, 1500, 400, 100], 4);
        assert_eq!(p.len(), 4);
        assert_eq!((p[0], p[3]), (0, 6));
        assert!(p.windows(2).all(|w| w[0] < w[1]), "{p:?}");
    }

    #[test]
    fn metadata_kinds() {
        let k = |j: &str| metadata_kind(&serde_json::from_str(j).unwrap());
        assert_eq!(k(r#"{"semanticTags": {"asset_type": "3D plant"}}"#), Some(Kind::Plant));
        assert_eq!(k(r#"{"categories": ["3d", "rock", "boulder"]}"#), Some(Kind::Model3d));
        assert_eq!(k(r#"{"categories": ["surface", "ground"]}"#), Some(Kind::Surface));
        assert_eq!(k(r#"{"type": "decal"}"#), Some(Kind::Decal));
        assert_eq!(k(r#"{"categories": ["imperfection", "smudge"]}"#), Some(Kind::Imperfection));
        assert_eq!(k(r#"{"name": "x"}"#), None);
    }
}
