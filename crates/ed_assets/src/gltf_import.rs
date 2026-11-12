//! glTF 2.0 import (`.glb` or `.gltf` with embedded, data-URI or relative buffers).
//!
//! - Every mesh primitive becomes one pack `mesh` asset with its LOD chain; the node
//!   hierarchy is flattened into one `prefab` (one node per primitive, global transform;
//!   skinned primitives get identity, as glTF ignores the skinned node's transform).
//! - Materials: metal-rough. Base colour -> BC7 sRGB, normal -> BC5, metal-rough (+ occlusion
//!   when it shares the image or the size) -> ORM BC7 linear (R occlusion, G roughness,
//!   B metal: glTF's own channel layout).
//! - Images: PNG, JPEG, WebP (`image/webp` and `EXT_texture_webp`).
//! - `KHR_texture_transform`: when every texture of a material has the same transform on
//!   UV set 0, it is baked into the UVs of the primitives using it; otherwise the material
//!   carries it as parameters ([`crate::material::P_UV`]).
//! - Skins: one `skeleton` per skinned primitive (joints sorted parents-first, `JOINTS_0`
//!   remapped; root rest transforms are global, so the skinned result is in prefab space).
//! - `extensionsRequired` outside [`SUPPORTED_REQUIRED`] is rejected with the name in the
//!   error. glTF's own validation is skipped (gltf 1.4 does not know EXT_texture_webp); we
//!   check indices and accessors as we read them.
//!
//! Units: glTF metres, +Y up; mesh space keeps that (the instance transform maps to ENU).

use crate::geom::{Lod, MeshBuild};
use crate::lod::{LodReport, build_chain};
use crate::material::{self, T_BASE, T_NORMAL, T_ORM};
use crate::texture::{self, Rgba8, Role};
use anyhow::{Context, Result, anyhow, bail};
use ed_pack::AssetId;
use ed_pack::kinds::{AlphaMode, Joint, Material, MaterialModel, Prefab, PrefabNode, Skeleton, Texture, Transform};
use glam::{Mat4, Quat, Vec3};
use std::collections::BTreeMap;
use std::path::Path;

/// Required extensions this importer implements.
pub const SUPPORTED_REQUIRED: &[&str] = &["EXT_texture_webp", "KHR_texture_transform"];

/// One baked mesh asset.
pub struct ImportedMesh {
    pub asset: String,
    pub build: MeshBuild,
    pub lods: Vec<Lod>,
    pub report: Vec<LodReport>,
}

/// Everything a glTF file contributes, keyed by asset path (all under `base`).
pub struct Imported {
    pub meshes: Vec<ImportedMesh>,
    pub textures: Vec<(String, Texture<'static>)>,
    pub materials: Vec<(String, Material)>,
    pub skeletons: Vec<(String, Skeleton)>,
    pub prefab: (String, Prefab),
    /// Texture transforms that were baked into UVs (material index), for reports and tests.
    pub baked_uv_transforms: Vec<usize>,
}

/// A KHR_texture_transform (identity when absent).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UvXf {
    pub offset: [f32; 2],
    pub scale: [f32; 2],
    pub rotation: f32,
    pub tex_coord: u32,
}

impl Default for UvXf {
    fn default() -> Self {
        UvXf { offset: [0.0; 2], scale: [1.0; 2], rotation: 0.0, tex_coord: 0 }
    }
}

impl UvXf {
    /// `uv' = T * R * S * uv` (KHR_texture_transform, column-major
    /// `R = mat3(c, -s, 0,  s, c, 0,  0, 0, 1)`).
    pub fn apply(&self, uv: [f32; 2]) -> [f32; 2] {
        let (s, c) = self.rotation.sin_cos();
        let (x, y) = (uv[0] * self.scale[0], uv[1] * self.scale[1]);
        [c * x + s * y + self.offset[0], -s * x + c * y + self.offset[1]]
    }
    fn is_identity(&self) -> bool {
        self.offset == [0.0; 2] && self.scale == [1.0; 2] && self.rotation == 0.0
    }
}

fn read_xf(info: &serde_json::Value) -> UvXf {
    let mut x = UvXf { tex_coord: info.get("texCoord").and_then(|v| v.as_u64()).unwrap_or(0) as u32, ..Default::default() };
    if let Some(t) = info.get("extensions").and_then(|e| e.get("KHR_texture_transform")) {
        let f2 = |k: &str, d: [f32; 2]| {
            t.get(k).and_then(|v| v.as_array()).map(|a| [a[0].as_f64().unwrap_or(d[0] as f64) as f32, a[1].as_f64().unwrap_or(d[1] as f64) as f32]).unwrap_or(d)
        };
        x.offset = f2("offset", [0.0; 2]);
        x.scale = f2("scale", [1.0; 2]);
        x.rotation = t.get("rotation").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
        if let Some(tc) = t.get("texCoord").and_then(|v| v.as_u64()) {
            x.tex_coord = tc as u32;
        }
    }
    x
}

/// Rejects files that require extensions we do not implement.
pub fn check_required(doc: &gltf::Document) -> Result<()> {
    let bad: Vec<&str> = doc.extensions_required().filter(|e| !SUPPORTED_REQUIRED.contains(e)).collect();
    if !bad.is_empty() {
        bail!("glTF requires unsupported extension(s) {bad:?}; supported: {SUPPORTED_REQUIRED:?}");
    }
    Ok(())
}

fn b64(s: &str) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' | b'\n' | b'\r' => continue,
            _ => bail!("bad base64 in data URI"),
        };
        acc = acc << 6 | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Ok(out)
}

fn load_uri(uri: &str, dir: Option<&Path>) -> Result<Vec<u8>> {
    if let Some(rest) = uri.strip_prefix("data:") {
        let (_, data) = rest.split_once(";base64,").ok_or_else(|| anyhow!("data URI is not base64"))?;
        return b64(data);
    }
    let dir = dir.ok_or_else(|| anyhow!("external URI {uri:?} in a glTF loaded from memory"))?;
    if uri.contains("..") || uri.starts_with('/') {
        bail!("glTF URI {uri:?} leaves the file's folder");
    }
    std::fs::read(dir.join(uri.replace("%20", " "))).with_context(|| format!("glTF buffer/image {uri:?}"))
}

/// Imports a glTF file. `base` is the asset path prefix (e.g. `characters/solarcat`).
pub fn import_file(path: &Path, base: &str) -> Result<Imported> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    import(&bytes, path.parent(), base)
}

/// Imports glTF bytes; `dir` resolves relative URIs.
pub fn import(bytes: &[u8], dir: Option<&Path>, base: &str) -> Result<Imported> {
    let g = gltf::Gltf::from_slice_without_validation(bytes).context("parsing glTF")?;
    let doc = &g.document;
    check_required(doc)?;
    let json = doc.as_json();

    // Buffers.
    let mut buffers: Vec<Vec<u8>> = Vec::new();
    for b in doc.buffers() {
        let data = match b.source() {
            gltf::buffer::Source::Bin => g.blob.clone().ok_or_else(|| anyhow!("GLB has no BIN chunk"))?,
            gltf::buffer::Source::Uri(u) => load_uri(u, dir)?,
        };
        if data.len() < b.length() {
            bail!("buffer {} is {} bytes, needs {}", b.index(), data.len(), b.length());
        }
        buffers.push(data);
    }
    let view_bytes = |v: &gltf::buffer::View| -> Result<Vec<u8>> {
        let b = buffers.get(v.buffer().index()).ok_or_else(|| anyhow!("bad buffer index"))?;
        b.get(v.offset()..v.offset() + v.length()).map(|s| s.to_vec()).ok_or_else(|| anyhow!("buffer view out of range"))
    };

    // Images: decoded lazily, once each.
    let tex_json: Vec<serde_json::Value> = json.textures.iter().map(|t| serde_json::to_value(t).unwrap_or_default()).collect();
    let mut decoded: BTreeMap<usize, Rgba8> = BTreeMap::new();
    let mut image_of_texture = |ti: usize| -> Result<usize> {
        let tj = tex_json.get(ti).ok_or_else(|| anyhow!("texture {ti} out of range"))?;
        let webp = tj.pointer("/extensions/EXT_texture_webp/source").and_then(|v| v.as_u64());
        let src = tj.get("source").and_then(|v| v.as_u64());
        let img = webp.or(src).ok_or_else(|| anyhow!("texture {ti} has no image source"))? as usize;
        if img >= doc.images().len() {
            bail!("texture {ti}: image {img} out of range");
        }
        Ok(img)
    };
    let load_image = |img: usize, decoded: &mut BTreeMap<usize, Rgba8>| -> Result<()> {
        if decoded.contains_key(&img) {
            return Ok(());
        }
        let im = doc.images().nth(img).expect("checked");
        let bytes = match im.source() {
            gltf::image::Source::View { view, .. } => view_bytes(&view)?,
            gltf::image::Source::Uri { uri, .. } => load_uri(uri, dir)?,
        };
        decoded.insert(img, texture::decode_image(&bytes).with_context(|| format!("image {img}"))?);
        Ok(())
    };

    // Materials.
    let mat_json: Vec<serde_json::Value> = json.materials.iter().map(|m| serde_json::to_value(m).unwrap_or_default()).collect();
    let mut textures: Vec<(String, Texture<'static>)> = Vec::new();
    let mut tex_done: BTreeMap<String, ()> = BTreeMap::new();
    let mut materials = Vec::new();
    // Per material: the transform to bake into UVs (None = nothing to bake).
    let mut bake_xf: Vec<Option<UvXf>> = Vec::new();
    let mut baked_uv_transforms = Vec::new();
    for (mi, m) in doc.materials().enumerate() {
        let mj = &mat_json[mi];
        let pbr = m.pbr_metallic_roughness();
        let mut mat = material::simple(MaterialModel::Pbr, {
            let c = pbr.base_color_factor();
            [c[0], c[1], c[2]]
        }, pbr.metallic_factor(), pbr.roughness_factor());
        mat.double_sided = m.double_sided();
        mat.alpha_mode = match m.alpha_mode() {
            gltf::material::AlphaMode::Opaque => AlphaMode::Opaque,
            gltf::material::AlphaMode::Mask => AlphaMode::Mask,
            gltf::material::AlphaMode::Blend => AlphaMode::Blend,
        };
        mat.alpha_cutoff = m.alpha_cutoff().unwrap_or(0.5);
        // KHR_materials_transmission: no refraction yet; keep the factor and map to plain glass.
        if let Some(t) = mj.pointer("/extensions/KHR_materials_transmission/transmissionFactor").and_then(|v| v.as_f64())
            && t > 0.0
        {
            mat.params.insert(material::P_TRANSMISSION.into(), ed_pack::kinds::Param::Float(t as f32));
            mat.params.insert(material::P_ROUGHNESS.into(), ed_pack::kinds::Param::Float(pbr.roughness_factor().min(0.08)));
        }
        let infos = [
            ("base", mj.pointer("/pbrMetallicRoughness/baseColorTexture")),
            ("mr", mj.pointer("/pbrMetallicRoughness/metallicRoughnessTexture")),
            ("normal", mj.get("normalTexture")),
            ("occlusion", mj.get("occlusionTexture")),
        ];
        let xfs: Vec<UvXf> = infos.iter().filter_map(|(_, i)| i.map(read_xf)).collect();
        let uniform = xfs.windows(2).all(|w| w[0] == w[1]);
        let xf = xfs.first().copied().unwrap_or_default();
        if uniform && xf.tex_coord == 0 {
            if !xf.is_identity() {
                bake_xf.push(Some(xf));
                baked_uv_transforms.push(mi);
            } else {
                bake_xf.push(None);
            }
        } else {
            // Mixed transforms: carry the base colour's in the material (the renderer applies
            // one transform per material).
            bake_xf.push(None);
            material::set_uv(&mut mat, xf.offset, xf.scale, xf.rotation);
        }
        let tex_index = |i: Option<&serde_json::Value>| i.and_then(|v| v.get("index")).and_then(|v| v.as_u64()).map(|v| v as usize);
        let mut add = |slot: &str, asset: String, make: &mut dyn FnMut() -> Result<Texture<'static>>| -> Result<()> {
            if !tex_done.contains_key(&asset) {
                textures.push((asset.clone(), make()?));
                tex_done.insert(asset.clone(), ());
            }
            mat.textures.insert(slot.into(), AssetId::from_path(&asset));
            Ok(())
        };
        if let Some(ti) = tex_index(infos[0].1) {
            let img = image_of_texture(ti)?;
            load_image(img, &mut decoded)?;
            let asset = format!("{base}/image{img}_colour");
            let id = AssetId::from_path(&asset);
            add(T_BASE, asset, &mut || texture::bake(&decoded[&img], Role::Colour, &id))?;
        }
        if let Some(ti) = tex_index(infos[2].1) {
            let img = image_of_texture(ti)?;
            load_image(img, &mut decoded)?;
            let asset = format!("{base}/image{img}_normal");
            let id = AssetId::from_path(&asset);
            add(T_NORMAL, asset, &mut || texture::bake(&decoded[&img], Role::Normal, &id))?;
        }
        let mr = tex_index(infos[1].1).map(&mut image_of_texture).transpose()?;
        let occ = tex_index(infos[3].1).map(&mut image_of_texture).transpose()?;
        if mr.is_some() || occ.is_some() {
            for i in mr.iter().chain(occ.iter()) {
                load_image(*i, &mut decoded)?;
            }
            let asset = format!("{base}/orm_{}_{}", mr.map_or("x".into(), |i| i.to_string()), occ.map_or("x".into(), |i| i.to_string()));
            let id = AssetId::from_path(&asset);
            let orm = compose_orm(mr.map(|i| &decoded[&i]), occ.map(|i| &decoded[&i]));
            add(T_ORM, asset, &mut || texture::bake(&orm, Role::Linear, &id))?;
            // With an ORM texture the factors multiply it (glTF semantics).
        }
        materials.push((format!("{base}/material{mi}"), mat));
    }
    let default_material = format!("{base}/material_default");
    let mut need_default = false;

    // Node globals (scene graph walk from every scene root, parents first).
    let n_nodes = doc.nodes().len();
    let mut parent: Vec<Option<usize>> = vec![None; n_nodes];
    for n in doc.nodes() {
        for c in n.children() {
            parent[c.index()] = Some(n.index());
        }
    }
    let local: Vec<Mat4> = doc.nodes().map(|n| Mat4::from_cols_array_2d(&n.transform().matrix())).collect();
    let mut global: Vec<Option<Mat4>> = vec![None; n_nodes];
    fn glob(i: usize, parent: &[Option<usize>], local: &[Mat4], g: &mut Vec<Option<Mat4>>, depth: usize) -> Result<Mat4> {
        if let Some(m) = g[i] {
            return Ok(m);
        }
        if depth > 256 {
            bail!("glTF node hierarchy has a cycle");
        }
        let m = match parent[i] {
            Some(p) => glob(p, parent, local, g, depth + 1)? * local[i],
            None => local[i],
        };
        g[i] = Some(m);
        Ok(m)
    }
    for i in 0..n_nodes {
        glob(i, &parent, &local, &mut global, 0)?;
    }
    let global: Vec<Mat4> = global.into_iter().map(|m| m.expect("filled")).collect();

    // Nodes in the default scene (or all roots), depth-first, deterministic order.
    let scene_roots: Vec<usize> = match doc.default_scene().or_else(|| doc.scenes().next()) {
        Some(s) => s.nodes().map(|n| n.index()).collect(),
        None => (0..n_nodes).filter(|&i| parent[i].is_none()).collect(),
    };
    let mut order = Vec::new();
    let mut stack: Vec<usize> = scene_roots.iter().rev().copied().collect();
    while let Some(i) = stack.pop() {
        order.push(i);
        let node = doc.nodes().nth(i).expect("index");
        let ch: Vec<usize> = node.children().map(|c| c.index()).collect();
        stack.extend(ch.into_iter().rev());
    }

    let mut meshes = Vec::new();
    let mut skeletons = Vec::new();
    let mut prefab = Prefab::default();
    let mut mesh_done: BTreeMap<(usize, usize, Option<usize>), String> = BTreeMap::new();
    for &ni in &order {
        let node = doc.nodes().nth(ni).expect("index");
        let Some(mesh) = node.mesh() else { continue };
        let skin = node.skin();
        for prim in mesh.primitives() {
            if prim.mode() != gltf::mesh::Mode::Triangles {
                bail!("mesh {} primitive {}: only triangle lists are supported ({:?})", mesh.index(), prim.index(), prim.mode());
            }
            let mi = prim.material().index();
            let key = (mesh.index(), prim.index(), skin.as_ref().map(|s| s.index()));
            let asset = match mesh_done.get(&key) {
                Some(a) => a.clone(),
                None => {
                    let asset = format!("{base}/mesh{}_{}", mesh.index(), prim.index());
                    let mut build = read_primitive(&prim, &buffers).with_context(|| format!("mesh {} primitive {}", mesh.index(), prim.index()))?;
                    if let Some(Some(xf)) = mi.map(|m| bake_xf[m]) {
                        for uv in &mut build.uvs {
                            *uv = xf.apply(*uv);
                        }
                    }
                    if let Some(skin) = &skin {
                        let sk = read_skin(skin, &buffers, &parent, &global, &doc.nodes().map(|n| n.name().unwrap_or("").to_string()).collect::<Vec<_>>(), &mut build, &asset)?;
                        skeletons.push((format!("{asset}/skeleton"), sk));
                    }
                    build.validate()?;
                    let (lods, report) = build_chain(&build.positions, &build.indices, 4);
                    meshes.push(ImportedMesh { asset: asset.clone(), build, lods, report });
                    mesh_done.insert(key, asset.clone());
                    asset
                }
            };
            let material = match mi {
                Some(m) => materials[m].0.clone(),
                None => {
                    need_default = true;
                    default_material.clone()
                }
            };
            let transform = if skin.is_some() { Transform::default() } else { decompose(&global[ni]) };
            prefab.nodes.push(PrefabNode {
                parent: None,
                name: format!("{}#{}", node.name().unwrap_or("node"), prim.index()),
                transform,
                mesh: Some(AssetId::from_path(&asset)),
                material: Some(AssetId::from_path(&material)),
            });
        }
    }
    // Sockets: nodes named SOCKET_* (extras.socket_kind) become prefab connection points
    // named "socket:<kind>:<node name>", global transform, in scene order.
    for &ni in &order {
        let node = doc.nodes().nth(ni).expect("index");
        let name = node.name().unwrap_or("");
        if !name.starts_with("SOCKET_") {
            continue;
        }
        let kind = node
            .extras()
            .as_ref()
            .and_then(|r| serde_json::from_str::<serde_json::Value>(r.get()).ok())
            .and_then(|v| v.get("socket_kind").and_then(|k| k.as_str()).map(str::to_string))
            .unwrap_or_else(|| "any".into());
        prefab.points.push(ed_pack::kinds::PrefabPoint { name: format!("socket:{kind}:{name}"), kind: ed_pack::kinds::PointKind::Connection, transform: decompose(&global[ni]) });
    }
    if prefab.nodes.is_empty() {
        bail!("glTF has no mesh in its scene");
    }
    if need_default {
        materials.push((default_material, material::simple(MaterialModel::Pbr, [0.8; 3], 0.0, 0.5)));
    }
    Ok(Imported { meshes, textures, materials, skeletons, prefab: (base.to_string(), prefab), baked_uv_transforms })
}

/// One mesh primitive placed in the scene, for geometry-only imports (Megascans 3D assets:
/// materials and images are ignored; the textures come through the surface path).
#[derive(Clone, Debug)]
pub struct GeomPiece {
    pub node_name: String,
    pub mesh_name: String,
    /// Position in the depth-first scene order (stable for a given file).
    pub order: usize,
    /// Vertices in scene space (the node's global transform applied, normals included).
    pub build: MeshBuild,
}

/// Reads every triangle primitive in the default scene, transformed to scene space, without
/// touching materials or images. Skins are ignored (bind pose).
pub fn read_geometry(bytes: &[u8], dir: Option<&Path>) -> Result<Vec<GeomPiece>> {
    let g = gltf::Gltf::from_slice_without_validation(bytes).context("parsing glTF")?;
    let doc = &g.document;
    check_required(doc)?;
    let mut buffers: Vec<Vec<u8>> = Vec::new();
    for b in doc.buffers() {
        let data = match b.source() {
            gltf::buffer::Source::Bin => g.blob.clone().ok_or_else(|| anyhow!("GLB has no BIN chunk"))?,
            gltf::buffer::Source::Uri(u) => load_uri(u, dir)?,
        };
        if data.len() < b.length() {
            bail!("buffer {} is {} bytes, needs {}", b.index(), data.len(), b.length());
        }
        buffers.push(data);
    }
    let roots: Vec<gltf::Node> = match doc.default_scene().or_else(|| doc.scenes().next()) {
        Some(s) => s.nodes().collect(),
        None => {
            let mut child = vec![false; doc.nodes().len()];
            for n in doc.nodes() {
                for c in n.children() {
                    child[c.index()] = true;
                }
            }
            doc.nodes().filter(|n| !child[n.index()]).collect()
        }
    };
    let mut out = Vec::new();
    let mut stack: Vec<(gltf::Node, Mat4, usize)> = roots.into_iter().rev().map(|n| (n, Mat4::IDENTITY, 0)).collect();
    while let Some((node, parent, depth)) = stack.pop() {
        if depth > 256 {
            bail!("glTF node hierarchy is too deep (cycle?)");
        }
        let global = parent * Mat4::from_cols_array_2d(&node.transform().matrix());
        if let Some(mesh) = node.mesh() {
            let nmat = Mat4::from_mat3(glam::Mat3::from_mat4(global).inverse().transpose());
            for prim in mesh.primitives() {
                if prim.mode() != gltf::mesh::Mode::Triangles {
                    bail!("mesh {} primitive {}: only triangle lists are supported ({:?})", mesh.index(), prim.index(), prim.mode());
                }
                let mut build = read_primitive(&prim, &buffers).with_context(|| format!("mesh {} primitive {}", mesh.index(), prim.index()))?;
                build.joints.clear();
                build.weights.clear();
                for p in &mut build.positions {
                    *p = global.transform_point3(Vec3::from(*p)).to_array();
                }
                for n in &mut build.normals {
                    *n = nmat.transform_vector3(Vec3::from(*n)).normalize_or(Vec3::Y).to_array();
                }
                if global.determinant() < 0.0 {
                    for t in build.indices.as_chunks_mut::<3>().0 {
                        t.swap(1, 2);
                    }
                }
                out.push(GeomPiece { node_name: node.name().unwrap_or("").to_string(), mesh_name: mesh.name().unwrap_or("").to_string(), order: out.len(), build });
            }
        }
        let ch: Vec<gltf::Node> = node.children().collect();
        stack.extend(ch.into_iter().rev().map(|c| (c, global, depth + 1)));
    }
    Ok(out)
}

/// ORM from glTF metal-rough (G roughness, B metal) and occlusion (R). Occlusion of a
/// different size than the metal-rough image is dropped (R = 1).
fn compose_orm(mr: Option<&Rgba8>, occ: Option<&Rgba8>) -> Rgba8 {
    let (w, h) = mr.or(occ).map(|i| (i.width, i.height)).expect("one of them");
    let occ = occ.filter(|o| o.width == w && o.height == h);
    let mut d = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let m = mr.map_or([255, 255, 255, 255], |i| i.px(x, y));
            let o = occ.map_or(255, |i| i.px(x, y)[0]);
            d.extend_from_slice(&[o, m[1], m[2], 255]);
        }
    }
    Rgba8 { width: w, height: h, data: d }
}

fn decompose(m: &Mat4) -> Transform {
    let (s, r, t) = m.to_scale_rotation_translation();
    Transform { translation: t.to_array(), rotation: r.normalize().to_array(), scale: s.to_array() }
}

fn read_primitive(prim: &gltf::Primitive, buffers: &[Vec<u8>]) -> Result<MeshBuild> {
    let r = prim.reader(|b| buffers.get(b.index()).map(|v| v.as_slice()));
    let positions: Vec<[f32; 3]> = r.read_positions().ok_or_else(|| anyhow!("no POSITION"))?.collect();
    let n = positions.len();
    let indices: Vec<u32> = match r.read_indices() {
        Some(i) => i.into_u32().collect(),
        None => (0..n as u32).collect(),
    };
    if indices.iter().any(|&i| i as usize >= n) {
        bail!("index out of range");
    }
    let normals: Vec<[f32; 3]> = match r.read_normals() {
        Some(it) => it.collect(),
        None => smooth_normals(&positions, &indices),
    };
    let uvs: Vec<[f32; 2]> = r.read_tex_coords(0).map(|t| t.into_f32().collect()).unwrap_or_default();
    let mut joints: Vec<[u16; 4]> = r.read_joints(0).map(|j| j.into_u16().collect()).unwrap_or_default();
    let mut weights: Vec<[f32; 4]> = r.read_weights(0).map(|w| w.into_f32().collect()).unwrap_or_default();
    if joints.is_empty() != weights.is_empty() {
        joints.clear();
        weights.clear();
    }
    for w in &mut weights {
        let s: f32 = w.iter().sum();
        if s > 0.0 {
            for x in w.iter_mut() {
                *x /= s;
            }
        } else {
            *w = [1.0, 0.0, 0.0, 0.0];
        }
    }
    let normals = normals.into_iter().map(|n| Vec3::from(n).normalize_or(Vec3::Y).to_array()).collect();
    Ok(MeshBuild { positions, normals, uvs, joints, weights, indices })
}

fn smooth_normals(p: &[[f32; 3]], idx: &[u32]) -> Vec<[f32; 3]> {
    let mut n = vec![Vec3::ZERO; p.len()];
    for t in idx.chunks_exact(3) {
        let (a, b, c) = (Vec3::from(p[t[0] as usize]), Vec3::from(p[t[1] as usize]), Vec3::from(p[t[2] as usize]));
        let f = (b - a).cross(c - a);
        for &i in t {
            n[i as usize] += f;
        }
    }
    n.into_iter().map(|v| v.normalize_or(Vec3::Y).to_array()).collect()
}

/// Reads a skin into a parents-first skeleton and remaps the build's joint indices to it.
fn read_skin(
    skin: &gltf::Skin,
    buffers: &[Vec<u8>],
    parent: &[Option<usize>],
    global: &[Mat4],
    names: &[String],
    build: &mut MeshBuild,
    mesh_asset: &str,
) -> Result<Skeleton> {
    let joints: Vec<usize> = skin.joints().map(|j| j.index()).collect();
    if joints.is_empty() || joints.len() > ed_pack::kinds::MAX_JOINTS {
        bail!("skin {} has {} joints", skin.index(), joints.len());
    }
    let r = skin.reader(|b| buffers.get(b.index()).map(|v| v.as_slice()));
    let ibm: Vec<Mat4> = match r.read_inverse_bind_matrices() {
        Some(it) => it.map(|m| Mat4::from_cols_array_2d(&m)).collect(),
        None => vec![Mat4::IDENTITY; joints.len()],
    };
    if ibm.len() != joints.len() {
        bail!("skin {}: {} inverse bind matrices for {} joints", skin.index(), ibm.len(), joints.len());
    }
    // Joint parent = nearest ancestor node that is also a joint of this skin.
    let pos_of = |node: usize| joints.iter().position(|&j| j == node);
    let jparent: Vec<Option<usize>> = joints
        .iter()
        .map(|&j| {
            let mut p = parent[j];
            while let Some(pp) = p {
                if let Some(k) = pos_of(pp) {
                    return Some(k);
                }
                p = parent[pp];
            }
            None
        })
        .collect();
    let depth = |mut k: usize| {
        let mut d = 0;
        while let Some(p) = jparent[k] {
            k = p;
            d += 1;
            if d > joints.len() {
                break;
            }
        }
        d
    };
    let mut order: Vec<usize> = (0..joints.len()).collect();
    order.sort_by_key(|&k| (depth(k), k));
    let mut new_of = vec![0u16; joints.len()];
    for (new, &old) in order.iter().enumerate() {
        new_of[old] = new as u16;
    }
    for j in &mut build.joints {
        for x in j.iter_mut() {
            *x = *new_of.get(*x as usize).ok_or_else(|| anyhow!("JOINTS_0 index {x} beyond the skin's {} joints", joints.len()))?;
        }
    }
    let out = order
        .iter()
        .map(|&old| {
            let node = joints[old];
            let rest = match jparent[old] {
                Some(p) => global[joints[p]].inverse() * global[node],
                None => global[node],
            };
            Joint {
                parent: jparent[old].map(|p| new_of[p] as u32),
                name: names[node].clone(),
                rest: decompose(&rest),
                inverse_bind: ibm[old].to_cols_array(),
            }
        })
        .collect();
    let sk = Skeleton { mesh: AssetId::from_path(mesh_asset), joints: out };
    sk.validate().map_err(|e| anyhow!("{e}"))?;
    Ok(sk)
}

/// Helpers shared with tests and fixtures.
pub fn transform_matrix(t: &Transform) -> Mat4 {
    Mat4::from_scale_rotation_translation(Vec3::from(t.scale), Quat::from_array(t.rotation), Vec3::from(t.translation))
}

