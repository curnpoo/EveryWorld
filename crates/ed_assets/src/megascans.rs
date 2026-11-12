//! Megascans surface import: a folder of `<anything>_Albedo`, `_Normal`, `_Roughness`, `_AO`,
//! `_Displacement` (and optional `_Metalness`) images -> textures + one material.
//!
//! - Albedo -> base colour (BC7 sRGB); Normal -> BC5 (`normal_y = "dx"` flips green for
//!   DirectX-convention maps; Megascans' default `_Normal` export is OpenGL); AO, Roughness
//!   and Metalness -> one ORM (BC7 linear: R occlusion, G roughness, B metal); Displacement ->
//!   BC4 (`displacement` slot; not sampled by the renderer yet).
//! - Images: PNG or JPEG (EXR displacement is rejected with a message: convert to 16-bit PNG).
//! - Licence rules are enforced by the baker ([`crate::bake`]): registry id `megascans`, pack
//!   flagged game-packaged, source folder outside the repository, Fab asset id recorded in
//!   every chunk's provenance.
//! - [`import_maps`] is the shared path for every Megascans kind (3D assets, plants, decals,
//!   imperfections: `crate::megascans_lib`), with gloss -> roughness, opacity -> albedo alpha
//!   and a translucency slot.

use crate::material::{self, T_BASE, T_DISPLACEMENT, T_NORMAL, T_ORM};
use crate::texture::{self, Rgba8, Role};
use anyhow::{Context, Result, bail};
use ed_pack::AssetId;
use ed_pack::kinds::{AlphaMode, Material, MaterialModel, Param, Texture};
use std::path::{Path, PathBuf};

pub struct Surface {
    pub textures: Vec<(String, Texture<'static>)>,
    pub material: (String, Material),
    /// Source files used (for content hashes), in a fixed order.
    pub files: Vec<PathBuf>,
}

/// Finds the map whose file stem ends with `_<key>` (case-insensitive).
fn find(dir: &Path, key: &str) -> Result<Option<PathBuf>> {
    let mut hits = Vec::new();
    for e in std::fs::read_dir(dir).with_context(|| format!("Megascans folder {}", dir.display()))? {
        let p = e?.path();
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
        if stem.ends_with(&format!("_{}", key.to_ascii_lowercase())) {
            hits.push(p);
        }
    }
    hits.sort();
    if let Some(p) = hits.iter().find(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("exr"))) {
        if hits.len() == 1 {
            bail!("{}: EXR is not supported; export the {key} map as 16-bit PNG", p.display());
        }
    }
    Ok(hits.into_iter().find(|p| !p.extension().is_some_and(|x| x.eq_ignore_ascii_case("exr"))))
}

fn load(p: &Path) -> Result<Rgba8> {
    texture::decode_image(&std::fs::read(p)?).with_context(|| p.display().to_string())
}

/// Resolved map files of one Megascans asset (any of them may be missing).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Maps {
    pub albedo: Option<PathBuf>,
    pub normal: Option<PathBuf>,
    pub roughness: Option<PathBuf>,
    /// Used (inverted) only when there is no roughness map.
    pub gloss: Option<PathBuf>,
    pub ao: Option<PathBuf>,
    pub metalness: Option<PathBuf>,
    pub displacement: Option<PathBuf>,
    pub opacity: Option<PathBuf>,
    pub translucency: Option<PathBuf>,
    /// The albedo file also carries opacity in its alpha (Megascans plant `B-O`).
    pub albedo_alpha: bool,
    /// Packed glTF-convention ORM (R occlusion, G roughness, B metalness; 3D scans' `orm.png`).
    pub orm: Option<PathBuf>,
    /// Packed plant ORT (R occlusion, G roughness, B translucency; no metalness).
    pub ort: Option<PathBuf>,
    /// Billboard (impostor source) albedo+opacity and normal, kept as an extra texture pair.
    pub billboard_albedo: Option<PathBuf>,
    pub billboard_normal: Option<PathBuf>,
}

/// How [`import_maps`] builds the material.
#[derive(Clone, Debug, Default)]
pub struct MapOptions {
    /// Asset path of the material (textures go under `base`); `None` = `base`.
    pub material_asset: Option<String>,
    /// DirectX-convention normal map (flip green).
    pub normal_dx: bool,
    /// Put the opacity map into the albedo's alpha and make the material alpha-tested
    /// (`Some(cutoff)`), e.g. plant leaves.
    pub alpha_test: Option<f32>,
    /// Alpha-blended (decals): opacity into albedo alpha, `AlphaMode::Blend`.
    pub alpha_blend: bool,
    pub double_sided: bool,
    /// Albedo may be missing (imperfection maps).
    pub albedo_optional: bool,
    /// Also import a translucency map (slot [`T_TRANSLUCENCY`]).
    pub translucency: bool,
    /// Material tag ([`P_MEGASCANS_KIND`]), `None` for plain surfaces.
    pub kind_tag: Option<f32>,
}

/// Material texture slot for a plant's translucency map (not sampled by the renderer yet).
pub const T_TRANSLUCENCY: &str = "translucency";
/// Material slots for a plant's billboard textures (kept for later impostors).
pub const T_BILLBOARD_ALBEDO: &str = "billboard_albedo";
pub const T_BILLBOARD_NORMAL: &str = "billboard_normal";
/// Material parameter tagging the Megascans asset kind (see `megascans_lib::Kind::code`).
pub const P_MEGASCANS_KIND: &str = "megascans_kind";

/// Imports a surface folder as asset `base` (`<base>/albedo`, `<base>/normal`, `<base>/orm`,
/// `<base>/displacement`, material `<base>`).
pub fn import_dir(dir: &Path, base: &str, normal_dx: bool) -> Result<Surface> {
    let maps = Maps {
        albedo: find(dir, "Albedo")?.or(find(dir, "BaseColor")?),
        normal: find(dir, "Normal")?,
        roughness: find(dir, "Roughness")?,
        ao: find(dir, "AO")?,
        metalness: find(dir, "Metalness")?,
        displacement: find(dir, "Displacement")?,
        ..Default::default()
    };
    if maps.albedo.is_none() {
        bail!("{}: no *_Albedo map", dir.display());
    }
    import_maps(&maps, base, &MapOptions { normal_dx, ..Default::default() }).with_context(|| dir.display().to_string())
}

/// Builds textures and one material from resolved maps (the surface path, shared by every
/// Megascans kind).
pub fn import_maps(maps: &Maps, base: &str, o: &MapOptions) -> Result<Surface> {
    if maps.albedo.is_none() && !o.albedo_optional {
        bail!("no Albedo map");
    }
    let mut files = Vec::new();
    let mut textures = Vec::new();
    let mut mat = material::simple(MaterialModel::Pbr, [1.0; 3], if maps.metalness.is_some() { 1.0 } else { 0.0 }, 1.0);
    let mut put = |slot: &str, name: &str, img: &Rgba8, role: Role, mat: &mut Material| -> Result<()> {
        let asset = format!("{base}/{name}");
        textures.push((asset.clone(), texture::bake(img, role, &AssetId::from_path(&asset))?));
        mat.textures.insert(slot.into(), AssetId::from_path(&asset));
        Ok(())
    };
    let alpha = o.alpha_test.is_some() || o.alpha_blend;
    if let Some(albedo) = &maps.albedo {
        files.push(albedo.clone());
        let mut img = load(albedo)?;
        if alpha && !maps.albedo_alpha && let Some(op) = &maps.opacity {
            files.push(op.clone());
            let a = load(op)?;
            if (a.width, a.height) != (img.width, img.height) {
                bail!("{}: Opacity must have the Albedo's size ({}x{})", op.display(), img.width, img.height);
            }
            for (px, m) in img.data.as_chunks_mut::<4>().0.iter_mut().zip(a.data.as_chunks::<4>().0) {
                px[3] = m[0];
            }
        }
        put(T_BASE, "albedo", &img, Role::Colour, &mut mat)?;
    }
    if let Some(n) = &maps.normal {
        files.push(n.clone());
        let mut img = load(n)?;
        if o.normal_dx {
            for px in img.data.as_chunks_mut::<4>().0 {
                px[1] = 255 - px[1];
            }
        }
        put(T_NORMAL, "normal", &img, Role::Normal, &mut mat)?;
    }
    // Roughness, or gloss inverted when there is no roughness map.
    let (rough, invert) = match (&maps.roughness, &maps.gloss) {
        (Some(r), _) => (Some(r), false),
        (None, Some(g)) => (Some(g), true),
        _ => (None, false),
    };
    let (ao, metal) = (&maps.ao, &maps.metalness);
    // Packed maps: ORM as is; ORT (plants) with B = translucency, not metal.
    let mut packed: Option<Rgba8> = None;
    let mut trans_from_ort: Option<Rgba8> = None;
    if let Some(p) = &maps.orm {
        files.push(p.clone());
        packed = Some(load(p)?);
    } else if let Some(p) = &maps.ort {
        files.push(p.clone());
        let mut img = load(p)?;
        if o.translucency && maps.translucency.is_none() {
            let mut t = img.clone();
            for px in t.data.as_chunks_mut::<4>().0 {
                *px = [px[2], px[2], px[2], 255];
            }
            trans_from_ort = Some(t);
        }
        for px in img.data.as_chunks_mut::<4>().0 {
            px[2] = 0;
        }
        packed = Some(img);
    }
    if let Some(mut img) = packed {
        for px in img.data.as_chunks_mut::<4>().0 {
            px[3] = 255;
        }
        put(T_ORM, "orm", &img, Role::Linear, &mut mat)?;
    } else if rough.is_some() || ao.is_some() || metal.is_some() {
        let srcs = [ao.as_ref(), rough, metal.as_ref()];
        let mut maps_img: Vec<Option<Rgba8>> = srcs.iter().map(|p| p.map(|p| load(p)).transpose()).collect::<Result<_>>()?;
        if invert && let Some(g) = maps_img[1].as_mut() {
            for px in g.data.as_chunks_mut::<4>().0 {
                px[0] = 255 - px[0];
            }
        }
        for p in srcs.into_iter().flatten() {
            files.push(p.clone());
        }
        let (w, h) = maps_img.iter().flatten().map(|m| (m.width, m.height)).next().expect("one map");
        if maps_img.iter().flatten().any(|m| (m.width, m.height) != (w, h)) {
            bail!("AO, Roughness and Metalness must have the same size");
        }
        let mut d = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                let ch = |m: &Option<Rgba8>, def: u8| m.as_ref().map_or(def, |m| m.px(x, y)[0]);
                d.extend_from_slice(&[ch(&maps_img[0], 255), ch(&maps_img[1], 255), ch(&maps_img[2], 0), 255]);
            }
        }
        put(T_ORM, "orm", &Rgba8 { width: w, height: h, data: d }, Role::Linear, &mut mat)?;
    }
    if let Some(p) = &maps.displacement {
        files.push(p.clone());
        put(T_DISPLACEMENT, "displacement", &load(p)?, Role::Single, &mut mat)?;
    }
    if o.translucency && let Some(p) = &maps.translucency {
        files.push(p.clone());
        put(T_TRANSLUCENCY, "translucency", &load(p)?, Role::Colour, &mut mat)?;
    } else if let Some(t) = &trans_from_ort {
        put(T_TRANSLUCENCY, "translucency", t, Role::Colour, &mut mat)?;
    }
    if let Some(p) = &maps.billboard_albedo {
        files.push(p.clone());
        put(T_BILLBOARD_ALBEDO, "billboard_albedo", &load(p)?, Role::Colour, &mut mat)?;
    }
    if let Some(p) = &maps.billboard_normal {
        files.push(p.clone());
        put(T_BILLBOARD_NORMAL, "billboard_normal", &load(p)?, Role::Normal, &mut mat)?;
    }
    if let Some(c) = o.alpha_test {
        mat.alpha_mode = AlphaMode::Mask;
        mat.alpha_cutoff = c;
    } else if o.alpha_blend {
        mat.alpha_mode = AlphaMode::Blend;
    }
    mat.double_sided = o.double_sided;
    if let Some(k) = o.kind_tag {
        mat.params.insert(P_MEGASCANS_KIND.into(), Param::Float(k));
    }
    if mat.textures.is_empty() {
        bail!("no usable maps");
    }
    let name = o.material_asset.clone().unwrap_or_else(|| base.to_string());
    Ok(Surface { textures, material: (name, mat), files })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::pattern;

    /// Synthetic Megascans-style folder (no real Megascans data in tests).
    #[test]
    fn imports_a_synthetic_surface() {
        let dir = std::env::temp_dir().join(format!("ew-megascans-fixture-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for (name, seed) in [("Albedo", 1), ("Normal", 2), ("Roughness", 3), ("AO", 4), ("Displacement", 5)] {
            std::fs::write(dir.join(format!("ab12c_2K_{name}.png")), texture::encode_png(&pattern(32, 32, seed)).unwrap()).unwrap();
        }
        let s = import_dir(&dir, "surfaces/test", false).unwrap();
        let names: Vec<&str> = s.textures.iter().map(|t| t.0.as_str()).collect();
        assert_eq!(names, ["surfaces/test/albedo", "surfaces/test/normal", "surfaces/test/orm", "surfaces/test/displacement"]);
        assert_eq!(s.material.1.textures.len(), 4);
        assert_eq!(s.textures[0].1.format, ed_pack::kinds::TextureFormat::Bc7);
        assert_eq!(s.textures[1].1.format, ed_pack::kinds::TextureFormat::Bc5);
        assert_eq!(s.textures[3].1.format, ed_pack::kinds::TextureFormat::Bc4);
        assert_eq!(s.files.len(), 5);
        std::fs::remove_dir_all(&dir).ok();
    }
}
