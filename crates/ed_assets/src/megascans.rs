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

use crate::material::{self, T_BASE, T_DISPLACEMENT, T_NORMAL, T_ORM};
use crate::texture::{self, Rgba8, Role};
use anyhow::{Context, Result, bail};
use ed_pack::AssetId;
use ed_pack::kinds::{Material, MaterialModel, Texture};
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

/// Imports a surface folder as asset `base` (`<base>/albedo`, `<base>/normal`, `<base>/orm`,
/// `<base>/displacement`, material `<base>`).
pub fn import_dir(dir: &Path, base: &str, normal_dx: bool) -> Result<Surface> {
    let albedo = find(dir, "Albedo")?.or(find(dir, "BaseColor")?).with_context(|| format!("{}: no *_Albedo map", dir.display()))?;
    let normal = find(dir, "Normal")?;
    let rough = find(dir, "Roughness")?;
    let ao = find(dir, "AO")?;
    let metal = find(dir, "Metalness")?;
    let disp = find(dir, "Displacement")?;
    let mut files = vec![albedo.clone()];
    let mut textures = Vec::new();
    let mut mat = material::simple(MaterialModel::Pbr, [1.0; 3], if metal.is_some() { 1.0 } else { 0.0 }, 1.0);
    let mut put = |slot: &str, name: &str, img: &Rgba8, role: Role, mat: &mut Material| -> Result<()> {
        let asset = format!("{base}/{name}");
        textures.push((asset.clone(), texture::bake(img, role, &AssetId::from_path(&asset))?));
        mat.textures.insert(slot.into(), AssetId::from_path(&asset));
        Ok(())
    };
    put(T_BASE, "albedo", &load(&albedo)?, Role::Colour, &mut mat)?;
    if let Some(n) = &normal {
        files.push(n.clone());
        let mut img = load(n)?;
        if normal_dx {
            for px in img.data.chunks_exact_mut(4) {
                px[1] = 255 - px[1];
            }
        }
        put(T_NORMAL, "normal", &img, Role::Normal, &mut mat)?;
    }
    if rough.is_some() || ao.is_some() || metal.is_some() {
        let maps: Vec<Option<Rgba8>> = [&ao, &rough, &metal].iter().map(|p| p.as_ref().map(|p| load(p)).transpose()).collect::<Result<_>>()?;
        for p in [&ao, &rough, &metal].into_iter().flatten() {
            files.push(p.clone());
        }
        let (w, h) = maps.iter().flatten().map(|m| (m.width, m.height)).next().expect("one map");
        if maps.iter().flatten().any(|m| (m.width, m.height) != (w, h)) {
            bail!("{}: AO, Roughness and Metalness must have the same size", dir.display());
        }
        let mut d = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                let ch = |m: &Option<Rgba8>, def: u8| m.as_ref().map_or(def, |m| m.px(x, y)[0]);
                d.extend_from_slice(&[ch(&maps[0], 255), ch(&maps[1], 255), ch(&maps[2], 0), 255]);
            }
        }
        put(T_ORM, "orm", &Rgba8 { width: w, height: h, data: d }, Role::Linear, &mut mat)?;
    }
    if let Some(p) = &disp {
        files.push(p.clone());
        put(T_DISPLACEMENT, "displacement", &load(p)?, Role::Single, &mut mat)?;
    }
    Ok(Surface { textures, material: (base.to_string(), mat), files })
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
