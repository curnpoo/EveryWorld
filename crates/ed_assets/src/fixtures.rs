//! Generated glTF files for tests and the determinism corpus (no binary fixtures in the repo).
//!
//! - [`skinned_bar`]: a two-joint skinned bar with UVs, a WebP base colour through
//!   `EXT_texture_webp` (required), a PNG metal-rough texture, and the same
//!   `KHR_texture_transform` on both (required). The skin lists the child joint first so the
//!   importer must sort it.
//! - [`requires`]: a minimal file that requires a given extension.
//! - [`corpus_item`]: a heavier file (dense grid + large textures) for the ~100 MB corpus.

use crate::texture::{self, Rgba8};
use anyhow::Result;
use serde_json::{Value, json};

/// A GLB under construction.
#[derive(Default)]
pub struct Glb {
    pub bin: Vec<u8>,
    pub views: Vec<Value>,
    pub accessors: Vec<Value>,
}

impl Glb {
    pub fn view(&mut self, bytes: &[u8], target: Option<u32>) -> usize {
        while !self.bin.len().is_multiple_of(4) {
            self.bin.push(0);
        }
        let mut v = json!({"buffer": 0, "byteOffset": self.bin.len(), "byteLength": bytes.len()});
        if let Some(t) = target {
            v["target"] = json!(t);
        }
        self.bin.extend_from_slice(bytes);
        self.views.push(v);
        self.views.len() - 1
    }
    fn accessor(&mut self, bytes: &[u8], component: u32, count: usize, ty: &str, target: Option<u32>, minmax: Option<(Vec<f32>, Vec<f32>)>) -> usize {
        let view = self.view(bytes, target);
        let mut a = json!({"bufferView": view, "componentType": component, "count": count, "type": ty});
        if let Some((mn, mx)) = minmax {
            a["min"] = json!(mn);
            a["max"] = json!(mx);
        }
        self.accessors.push(a);
        self.accessors.len() - 1
    }
    pub fn f32s(&mut self, v: &[f32], width: usize, target: Option<u32>) -> usize {
        let ty = ["SCALAR", "VEC2", "VEC3", "VEC4"][width - 1];
        let mm = if width == 3 {
            let mut mn = vec![f32::MAX; 3];
            let mut mx = vec![f32::MIN; 3];
            for c in v.chunks(3) {
                for k in 0..3 {
                    mn[k] = mn[k].min(c[k]);
                    mx[k] = mx[k].max(c[k]);
                }
            }
            Some((mn, mx))
        } else {
            None
        };
        let b: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
        self.accessor(&b, 5126, v.len() / width, ty, target, mm)
    }
    pub fn mat4s(&mut self, v: &[f32]) -> usize {
        let b: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
        self.accessor(&b, 5126, v.len() / 16, "MAT4", None, None)
    }
    pub fn u16x4(&mut self, v: &[[u16; 4]]) -> usize {
        let b: Vec<u8> = v.iter().flatten().flat_map(|x| x.to_le_bytes()).collect();
        self.accessor(&b, 5123, v.len(), "VEC4", Some(34962), None)
    }
    pub fn u32s(&mut self, v: &[u32]) -> usize {
        let b: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
        self.accessor(&b, 5125, v.len(), "SCALAR", Some(34963), None)
    }

    /// Serialises with `doc` as the JSON (buffers, bufferViews and accessors are filled in).
    pub fn finish(mut self, mut doc: Value) -> Vec<u8> {
        while !self.bin.len().is_multiple_of(4) {
            self.bin.push(0);
        }
        doc["buffers"] = json!([{"byteLength": self.bin.len()}]);
        doc["bufferViews"] = Value::Array(self.views);
        doc["accessors"] = Value::Array(self.accessors);
        let mut j = serde_json::to_vec(&doc).expect("json");
        while !j.len().is_multiple_of(4) {
            j.push(b' ');
        }
        let total = 12 + 8 + j.len() + 8 + self.bin.len();
        let mut out = Vec::with_capacity(total);
        out.extend_from_slice(b"glTF");
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&(total as u32).to_le_bytes());
        out.extend_from_slice(&(j.len() as u32).to_le_bytes());
        out.extend_from_slice(b"JSON");
        out.extend_from_slice(&j);
        out.extend_from_slice(&(self.bin.len() as u32).to_le_bytes());
        out.extend_from_slice(b"BIN\0");
        out.extend_from_slice(&self.bin);
        out
    }
}

/// The JSON document of a GLB (for tests that tweak a fixture).
pub fn split_glb(glb: &[u8]) -> Result<Value> {
    anyhow::ensure!(glb.len() >= 20 && &glb[0..4] == b"glTF", "not a GLB");
    let jl = u32::from_le_bytes(glb[12..16].try_into()?) as usize;
    Ok(serde_json::from_slice(&glb[20..20 + jl])?)
}

/// `glb` with its JSON replaced by `doc` (the BIN chunk is kept as is).
pub fn rejoin_glb(glb: &[u8], doc: &Value) -> Vec<u8> {
    let jl = u32::from_le_bytes(glb[12..16].try_into().expect("glb")) as usize;
    let bin = &glb[20 + jl..];
    let mut j = serde_json::to_vec(doc).expect("json");
    while !j.len().is_multiple_of(4) {
        j.push(b' ');
    }
    let total = 12 + 8 + j.len() + bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(j.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&j);
    out.extend_from_slice(bin);
    out
}

/// A test image: gradient plus a hash pattern.
pub fn pattern(w: u32, h: u32, seed: u32) -> Rgba8 {
    let mut d = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let v = x.wrapping_mul(2654435761).wrapping_add(y.wrapping_mul(40503)).wrapping_add(seed.wrapping_mul(97));
            d.extend_from_slice(&[(x * 255 / w.max(1)) as u8, (y * 255 / h.max(1)) as u8, (v >> 24) as u8, 255]);
        }
    }
    Rgba8 { width: w, height: h, data: d }
}

/// The texture transform used by [`skinned_bar`].
pub const BAR_UV_OFFSET: [f32; 2] = [0.25, 0.5];
pub const BAR_UV_SCALE: [f32; 2] = [2.0, 4.0];

/// Two-joint skinned bar, 1 x 2 x 1 m, 4 rings of 4 vertices. See the module docs.
pub fn skinned_bar() -> Result<Vec<u8>> {
    let mut g = Glb::default();
    let mut pos = Vec::new();
    let mut uv = Vec::new();
    let mut joints = Vec::new();
    let mut weights = Vec::new();
    let mut nrm = Vec::new();
    for ring in 0..4 {
        let y = ring as f32 * 2.0 / 3.0;
        for (k, (x, z)) in [(-0.5f32, -0.5f32), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)].into_iter().enumerate() {
            pos.extend_from_slice(&[x, y, z]);
            let l = (x * x + z * z).sqrt();
            nrm.extend_from_slice(&[x / l, 0.0, z / l]);
            uv.extend_from_slice(&[k as f32 / 4.0, ring as f32 / 3.0]);
            // Skin joint list is [child, root]: index 1 is the root, 0 the child.
            let t = y / 2.0;
            joints.push([1u16, 0, 0, 0]);
            weights.extend_from_slice(&[1.0 - t, t, 0.0, 0.0]);
        }
    }
    let mut idx = Vec::new();
    for ring in 0..3u32 {
        for k in 0..4u32 {
            let (a, b) = (ring * 4 + k, ring * 4 + (k + 1) % 4);
            let (c, d) = (a + 4, b + 4);
            idx.extend_from_slice(&[a, d, b, a, c, d]);
        }
    }
    let a_pos = g.f32s(&pos, 3, Some(34962));
    let a_nrm = g.f32s(&nrm, 3, Some(34962));
    let a_uv = g.f32s(&uv, 2, Some(34962));
    let a_j = g.u16x4(&joints);
    let a_w = g.f32s(&weights, 4, Some(34962));
    let a_i = g.u32s(&idx);
    // Inverse binds: root at the origin, child at y = 1.
    let mut ibm = Vec::new();
    let child = glam::Mat4::from_translation(glam::Vec3::new(0.0, -1.0, 0.0));
    ibm.extend_from_slice(&child.to_cols_array());
    ibm.extend_from_slice(&glam::Mat4::IDENTITY.to_cols_array());
    let a_ibm = g.mat4s(&ibm);
    let webp = texture::encode_webp(&pattern(16, 16, 1))?;
    let png = texture::encode_png(&pattern(8, 8, 2))?;
    let v_webp = g.view(&webp, None);
    let v_png = g.view(&png, None);
    let xf = json!({"KHR_texture_transform": {"offset": BAR_UV_OFFSET, "scale": BAR_UV_SCALE}});
    let doc = json!({
        "asset": {"version": "2.0", "generator": "ed_assets fixtures"},
        "extensionsUsed": ["EXT_texture_webp", "KHR_texture_transform"],
        "extensionsRequired": ["EXT_texture_webp", "KHR_texture_transform"],
        "scene": 0,
        "scenes": [{"nodes": [0, 3]}],
        "nodes": [
            {"name": "root", "children": [1], "translation": [0.0, 0.0, 0.0]},
            {"name": "child", "translation": [0.0, 1.0, 0.0]},
            {"name": "unused"},
            {"name": "bar", "mesh": 0, "skin": 0, "translation": [5.0, 0.0, 0.0]}
        ],
        "skins": [{"joints": [1, 0], "inverseBindMatrices": a_ibm}],
        "meshes": [{"primitives": [{
            "attributes": {"POSITION": a_pos, "NORMAL": a_nrm, "TEXCOORD_0": a_uv, "JOINTS_0": a_j, "WEIGHTS_0": a_w},
            "indices": a_i, "material": 0
        }]}],
        "materials": [{
            "name": "bar",
            "pbrMetallicRoughness": {
                "baseColorFactor": [1.0, 1.0, 1.0, 1.0],
                "baseColorTexture": {"index": 0, "extensions": xf.clone()},
                "metallicRoughnessTexture": {"index": 1, "extensions": xf},
                "metallicFactor": 0.0, "roughnessFactor": 1.0
            }
        }],
        "textures": [
            {"sampler": 0, "extensions": {"EXT_texture_webp": {"source": 0}}},
            {"sampler": 0, "source": 1}
        ],
        "samplers": [{"magFilter": 9729, "minFilter": 9987}],
        "images": [
            {"bufferView": v_webp, "mimeType": "image/webp"},
            {"bufferView": v_png, "mimeType": "image/png"}
        ]
    });
    Ok(g.finish(doc))
}

/// A one-triangle file that lists `ext` in `extensionsRequired`.
pub fn requires(ext: &str) -> Vec<u8> {
    let mut g = Glb::default();
    let a = g.f32s(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0], 3, Some(34962));
    let doc = json!({
        "asset": {"version": "2.0"},
        "extensionsUsed": [ext],
        "extensionsRequired": [ext],
        "scenes": [{"nodes": [0]}],
        "nodes": [{"mesh": 0}],
        "meshes": [{"primitives": [{"attributes": {"POSITION": a}}]}]
    });
    g.finish(doc)
}

/// A heavier file for the determinism corpus: a `grid` x `grid` displaced sheet (two
/// primitives, two materials) with `tex` x `tex` base colour and metal-rough PNG textures.
pub fn corpus_item(seed: u32, grid: u32, tex: u32) -> Result<Vec<u8>> {
    let mut g = Glb::default();
    let mut prims = Vec::new();
    for half in 0..2u32 {
        let mut pos = Vec::new();
        let mut uv = Vec::new();
        for j in 0..=grid {
            for i in 0..=grid {
                let (u, v) = (i as f32 / grid as f32, j as f32 / grid as f32);
                let h = ((u * 9.0 + seed as f32).sin() * (v * 7.0 + half as f32).cos()) * 0.3;
                pos.extend_from_slice(&[u * 10.0 + half as f32 * 10.5, h, v * 10.0]);
                uv.extend_from_slice(&[u, v]);
            }
        }
        let mut idx = Vec::new();
        for j in 0..grid {
            for i in 0..grid {
                let a = j * (grid + 1) + i;
                idx.extend_from_slice(&[a, a + grid + 1, a + 1, a + 1, a + grid + 1, a + grid + 2]);
            }
        }
        let ap = g.f32s(&pos, 3, Some(34962));
        let au = g.f32s(&uv, 2, Some(34962));
        let ai = g.u32s(&idx);
        prims.push(json!({"attributes": {"POSITION": ap, "TEXCOORD_0": au}, "indices": ai, "material": half}));
    }
    let mut images = Vec::new();
    for k in 0..4u32 {
        let png = texture::encode_png(&pattern(tex, tex, seed * 8 + k))?;
        let v = g.view(&png, None);
        images.push(json!({"bufferView": v, "mimeType": "image/png"}));
    }
    let doc = json!({
        "asset": {"version": "2.0"},
        "scenes": [{"nodes": [0]}],
        "nodes": [{"mesh": 0}],
        "meshes": [{"primitives": prims}],
        "materials": [
            {"pbrMetallicRoughness": {"baseColorTexture": {"index": 0}, "metallicRoughnessTexture": {"index": 1}}},
            {"pbrMetallicRoughness": {"baseColorTexture": {"index": 2}, "metallicRoughnessTexture": {"index": 3}}}
        ],
        "textures": [{"source": 0}, {"source": 1}, {"source": 2}, {"source": 3}],
        "images": images
    });
    Ok(g.finish(doc))
}

/// A lumpy "rock" as a supplied LOD chain (Megascans-style): one node and mesh per level,
/// named `<name>_LOD<k>`, each a UV sphere with `levels[k]` = (segments, rings), positions
/// times `scale` (1 = metres, 100 = a centimetre file). Normals and UV0 included.
pub fn lod_chain_glb(name: &str, levels: &[(u32, u32)], scale: f32) -> Vec<u8> {
    let mut g = Glb::default();
    let mut meshes = Vec::new();
    let mut nodes = Vec::new();
    for (k, &(segs, rings)) in levels.iter().enumerate() {
        let (mut pos, mut nrm, mut uv) = (Vec::new(), Vec::new(), Vec::new());
        for r in 0..=rings {
            let th = r as f32 / rings as f32 * std::f32::consts::PI;
            for s in 0..=segs {
                let ph = s as f32 / segs as f32 * std::f32::consts::TAU;
                let d = [th.sin() * ph.cos(), th.cos(), th.sin() * ph.sin()];
                let rad = 0.5 * (1.0 + 0.15 * (3.0 * d[0] + 1.0).sin() * (2.0 * d[1]).cos() + 0.1 * (5.0 * d[2]).sin());
                pos.extend_from_slice(&[d[0] * rad * scale, (d[1] * rad * 0.8 + 0.4) * scale, d[2] * rad * scale]);
                nrm.extend_from_slice(&d);
                uv.extend_from_slice(&[s as f32 / segs as f32, r as f32 / rings as f32]);
            }
        }
        let mut idx = Vec::new();
        for r in 0..rings {
            for s in 0..segs {
                let a = r * (segs + 1) + s;
                let b = a + segs + 1;
                idx.extend_from_slice(&[a, a + 1, b, a + 1, b + 1, b]);
            }
        }
        let ap = g.f32s(&pos, 3, Some(34962));
        let an = g.f32s(&nrm, 3, Some(34962));
        let au = g.f32s(&uv, 2, Some(34962));
        let ai = g.u32s(&idx);
        meshes.push(json!({"name": format!("{name}_LOD{k}"), "primitives": [{"attributes": {"POSITION": ap, "NORMAL": an, "TEXCOORD_0": au}, "indices": ai}]}));
        nodes.push(json!({"name": format!("{name}_LOD{k}"), "mesh": k}));
    }
    let roots: Vec<usize> = (0..levels.len()).collect();
    let doc = json!({
        "asset": {"version": "2.0"},
        "scenes": [{"nodes": roots}],
        "scene": 0,
        "nodes": nodes,
        "meshes": meshes,
    });
    g.finish(doc)
}

/// A glTF with an EXTERNAL buffer (`json`, `bin`), built from [`lod_chain_glb`]: `levels` as
/// `<name>_LOD<k>` nodes, `bin_uri` as the buffer URI (`"../x.bin"` mimics Fab's
/// `standard/*_nonUE.gltf`), and, with `billboard_last`, the last level drawn with a material
/// named `..._Billboard` (Fab's camera-facing LOD).
pub fn external_gltf(name: &str, levels: &[(u32, u32)], bin_uri: &str, billboard_last: bool) -> (Vec<u8>, Vec<u8>) {
    let glb = lod_chain_glb(name, levels, 1.0);
    let mut doc = split_glb(&glb).expect("glb");
    let jl = u32::from_le_bytes(glb[12..16].try_into().expect("len")) as usize;
    let bin = glb[20 + jl + 8..].to_vec();
    doc["buffers"] = json!([{"byteLength": bin.len(), "uri": bin_uri}]);
    doc["materials"] = json!([{"name": format!("MI_{name}")}, {"name": format!("MI_{name}_Billboard")}]);
    let n = levels.len();
    for (k, m) in doc["meshes"].as_array_mut().expect("meshes").iter_mut().enumerate() {
        m["primitives"][0]["material"] = json!(if billboard_last && k + 1 == n { 1 } else { 0 });
    }
    (serde_json::to_vec(&doc).expect("json"), bin)
}
