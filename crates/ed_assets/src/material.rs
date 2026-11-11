//! Material files v0 (TOML) -> material chunks.
//!
//! ```toml
//! model = "pbr"                 # or "toon"
//! base_colour = [0.8, 0.7, 0.6] # linear RGB
//! metallic = 0.0
//! roughness = 0.6
//! shade_colour = [0.4, 0.4, 0.45]  # toon only
//! bands = 3.0                      # toon only
//! rim = 0.3                        # toon only
//! checker = false                  # node-space 2 m checker
//! alpha_mode = "opaque"            # "mask", "blend"
//! alpha_cutoff = 0.5
//! double_sided = false
//! [textures]                       # texture asset paths (chunks in this pack or a mounted one)
//! base_colour = "props/bench/albedo"
//! normal = "props/bench/normal"
//! orm = "props/bench/orm"          # R occlusion, G roughness, B metal
//! [uv]                             # texture transform (KHR_texture_transform semantics)
//! offset = [0.0, 0.0]
//! scale = [1.0, 1.0]
//! rotation = 0.0
//! ```
//!
//! Parameter names in the chunk ([`P_BASE`] ...) are what the renderer reads; unknown TOML keys
//! are an error so typos do not silently fall back to defaults.

use anyhow::{Context, Result, bail};
use ed_pack::AssetId;
use ed_pack::kinds::{AlphaMode, Material, MaterialModel, Param};
use serde::Deserialize;
use std::collections::BTreeMap;

pub const P_BASE: &str = "base_colour";
pub const P_METALLIC: &str = "metallic";
pub const P_ROUGHNESS: &str = "roughness";
pub const P_SHADE: &str = "shade_colour";
pub const P_BANDS: &str = "bands";
pub const P_RIM: &str = "rim";
pub const P_CHECKER: &str = "checker";
/// `[offset.x, offset.y, scale.x, scale.y]`.
pub const P_UV: &str = "uv_offset_scale";
pub const P_UV_ROT: &str = "uv_rotation";
/// KHR_materials_transmission factor. The M3 renderer has no refraction: a material with
/// transmission > 0 is shaded as plain glass (dark, smooth, slightly metallic-looking).
pub const P_TRANSMISSION: &str = "transmission";
/// Texture slots the renderer samples.
pub const T_BASE: &str = "base_colour";
pub const T_NORMAL: &str = "normal";
pub const T_ORM: &str = "orm";
pub const T_DISPLACEMENT: &str = "displacement";
pub const SLOTS: [&str; 4] = [T_BASE, T_NORMAL, T_ORM, T_DISPLACEMENT];

#[derive(Deserialize, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct UvFile {
    #[serde(default)]
    pub offset: Option<[f32; 2]>,
    #[serde(default)]
    pub scale: Option<[f32; 2]>,
    #[serde(default)]
    pub rotation: Option<f32>,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct MaterialFile {
    #[serde(default = "pbr")]
    pub model: String,
    #[serde(default = "grey")]
    pub base_colour: [f32; 3],
    #[serde(default)]
    pub metallic: f32,
    #[serde(default = "half")]
    pub roughness: f32,
    #[serde(default)]
    pub shade_colour: Option<[f32; 3]>,
    #[serde(default)]
    pub bands: Option<f32>,
    #[serde(default)]
    pub rim: Option<f32>,
    #[serde(default)]
    pub checker: bool,
    #[serde(default)]
    pub alpha_mode: Option<String>,
    #[serde(default)]
    pub alpha_cutoff: Option<f32>,
    #[serde(default)]
    pub double_sided: bool,
    #[serde(default)]
    pub textures: BTreeMap<String, String>,
    #[serde(default)]
    pub uv: Option<UvFile>,
}

fn pbr() -> String {
    "pbr".into()
}
fn grey() -> [f32; 3] {
    [0.5, 0.5, 0.5]
}
fn half() -> f32 {
    0.5
}

/// Parses a material file into a chunk body.
pub fn parse(src: &str) -> Result<Material> {
    let f: MaterialFile = toml::from_str(src).context("material file")?;
    from_file(&f)
}

pub fn from_file(f: &MaterialFile) -> Result<Material> {
    let model = match f.model.as_str() {
        "pbr" => MaterialModel::Pbr,
        "toon" => MaterialModel::Toon,
        m => bail!("material: unknown model {m:?} (pbr, toon)"),
    };
    let mut m = Material::new(model);
    m.alpha_mode = match f.alpha_mode.as_deref().unwrap_or("opaque") {
        "opaque" => AlphaMode::Opaque,
        "mask" => AlphaMode::Mask,
        "blend" => AlphaMode::Blend,
        a => bail!("material: unknown alpha_mode {a:?}"),
    };
    m.alpha_cutoff = f.alpha_cutoff.unwrap_or(0.5);
    m.double_sided = f.double_sided;
    m.params.insert(P_BASE.into(), Param::Vec3(f.base_colour));
    m.params.insert(P_METALLIC.into(), Param::Float(f.metallic));
    m.params.insert(P_ROUGHNESS.into(), Param::Float(f.roughness));
    if model == MaterialModel::Toon {
        m.params.insert(P_SHADE.into(), Param::Vec3(f.shade_colour.unwrap_or([0.45; 3])));
        m.params.insert(P_BANDS.into(), Param::Float(f.bands.unwrap_or(3.0)));
        m.params.insert(P_RIM.into(), Param::Float(f.rim.unwrap_or(0.0)));
    } else if f.shade_colour.is_some() || f.bands.is_some() || f.rim.is_some() {
        bail!("material: shade_colour/bands/rim are toon parameters");
    }
    if f.checker {
        m.params.insert(P_CHECKER.into(), Param::Float(1.0));
    }
    for (slot, path) in &f.textures {
        if !SLOTS.contains(&slot.as_str()) {
            bail!("material: unknown texture slot {slot:?} (one of {SLOTS:?})");
        }
        if path.is_empty() {
            bail!("material: texture slot {slot:?} has an empty asset path");
        }
        m.textures.insert(slot.clone(), AssetId::from_path(path));
    }
    if let Some(uv) = &f.uv {
        set_uv(&mut m, uv.offset.unwrap_or([0.0; 2]), uv.scale.unwrap_or([1.0; 2]), uv.rotation.unwrap_or(0.0));
    }
    m.validate().map_err(|e| anyhow::anyhow!("material: {e}"))?;
    Ok(m)
}

/// Stores a texture transform (identity transforms are not stored).
pub fn set_uv(m: &mut Material, offset: [f32; 2], scale: [f32; 2], rotation: f32) {
    if offset != [0.0; 2] || scale != [1.0; 2] {
        m.params.insert(P_UV.into(), Param::Vec4([offset[0], offset[1], scale[0], scale[1]]));
    }
    if rotation != 0.0 {
        m.params.insert(P_UV_ROT.into(), Param::Float(rotation));
    }
}

/// Texture asset paths a material file references (bake order: textures before materials).
pub fn texture_paths(src: &str) -> Result<Vec<String>> {
    let f: MaterialFile = toml::from_str(src).context("material file")?;
    Ok(f.textures.values().cloned().collect())
}

/// A simple PBR or toon material (the proving ground's tables and glTF defaults).
pub fn simple(model: MaterialModel, base: [f32; 3], metallic: f32, roughness: f32) -> Material {
    let mut m = Material::new(model);
    m.params.insert(P_BASE.into(), Param::Vec3(base));
    m.params.insert(P_METALLIC.into(), Param::Float(metallic));
    m.params.insert(P_ROUGHNESS.into(), Param::Float(roughness));
    m
}

pub fn toon(base: [f32; 3], shade: [f32; 3], bands: f32, rim: f32) -> Material {
    let mut m = simple(MaterialModel::Toon, base, 0.0, 1.0);
    m.params.insert(P_SHADE.into(), Param::Vec3(shade));
    m.params.insert(P_BANDS.into(), Param::Float(bands));
    m.params.insert(P_RIM.into(), Param::Float(rim));
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_textures_uv_and_rejects_typos() {
        let m = parse(
            r#"
model = "pbr"
base_colour = [1.0, 0.5, 0.25]
roughness = 0.7
[textures]
base_colour = "a/albedo"
orm = "a/orm"
[uv]
scale = [2.0, 2.0]
"#,
        )
        .unwrap();
        assert_eq!(m.params[P_ROUGHNESS], Param::Float(0.7));
        assert_eq!(m.textures[T_BASE], AssetId::from_path("a/albedo"));
        assert_eq!(m.params[P_UV], Param::Vec4([0.0, 0.0, 2.0, 2.0]));
        assert!(parse("rougness = 0.3").is_err(), "typo is an error");
        assert!(parse("[textures]\nalbedo = \"x\"").is_err(), "unknown slot");
        assert!(parse("model = \"pbr\"\nbands = 3.0").is_err(), "toon param on pbr");
        let t = parse("model = \"toon\"\nbands = 4.0").unwrap();
        assert_eq!(t.params[P_BANDS], Param::Float(4.0));
    }
}
