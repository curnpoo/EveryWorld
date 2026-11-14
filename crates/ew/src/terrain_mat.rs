//! Terrain surface materials (P-slice S7 c): scanned layers from a local Megascans library,
//! packed for `ed_render`'s terrain material blend.
//!
//! The library is read at run time from `EW_MEGASCANS` (default
//! `~/Documents/Megascans/library` when it exists) and is never copied into the repository.
//! Packed, downsampled layers are cached under `<cache>/terrain_mat/` (gitignored). When the
//! library or a layer is missing, `load` returns `None` and the terrain keeps its flat tint.
//! `ED_TERRAIN_NOMAT=1` switches the blend off at draw time.

use ed_render::terrain::{MAT_LAYERS, TerrainLayer, TerrainMaterials};
use std::path::{Path, PathBuf};

/// Packed layer size (texels per side, mip 0).
pub const SIZE: u32 = 1024;
const CACHE_VERSION: u32 = 1;

/// One layer: the asset folder suffix, the metres per repeat (divides 4096) and the linear
/// albedo the layer is calibrated to (its mean; the scans are graded to physical values).
struct Spec {
    asset: &'static str,
    tile_m: f32,
    target: [f32; 3],
}

/// In `MAT_LAYERS` order: rock, scree, grass, forest, snow, soil.
const SPECS: [Spec; 6] = [
    Spec { asset: "__rock_cliff", tile_m: 4.0, target: [0.20, 0.19, 0.18] },
    Spec { asset: "__volcanic_gravel_ground", tile_m: 4.0, target: [0.22, 0.21, 0.20] },
    Spec { asset: "__grass_lawn", tile_m: 2.0, target: [0.11, 0.155, 0.05] },
    Spec { asset: "__forest_floor", tile_m: 2.0, target: [0.06, 0.07, 0.035] },
    Spec { asset: "__fresh_snow", tile_m: 2.0, target: [0.82, 0.84, 0.87] },
    Spec { asset: "__soil_dirt", tile_m: 1.0, target: [0.12, 0.10, 0.075] },
];

/// The calibrated mean albedo of a layer (`MAT_LAYERS` index): the grass carpet table's
/// references (grass G4).
pub fn layer_target(layer: usize) -> [f32; 3] {
    SPECS[layer].target
}

/// The library root, if there is one.
pub fn library_root() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("EW_MEGASCANS") {
        let p = PathBuf::from(p);
        return p.is_dir().then_some(p);
    }
    let home = std::env::var_os("HOME")?;
    let p = PathBuf::from(home).join("Documents/Megascans/library");
    p.is_dir().then_some(p)
}

/// Loads (or packs and caches) every layer. `None`: no library, or a layer is missing.
pub fn load() -> Option<TerrainMaterials> {
    debug_assert_eq!(SPECS.len(), MAT_LAYERS.len());
    let root = library_root()?;
    let cache = crate::terrain::cache_root().join("terrain_mat");
    let mut layers = Vec::with_capacity(SPECS.len());
    for s in &SPECS {
        let dir = std::fs::read_dir(&root).ok()?.flatten().map(|e| e.path()).find(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.ends_with(s.asset)))?;
        let key = format!("v{CACHE_VERSION}_{}_{SIZE}.bin", s.asset.trim_start_matches('_'));
        let path = cache.join(key);
        let (albedo, normal, mean) = match read_cache(&path) {
            Some(v) => v,
            None => {
                let v = pack(&dir)?;
                let _ = write_cache(&path, &v);
                v
            }
        };
        let gain = [0, 1, 2].map(|c| s.target[c] / mean[c].max(1e-3));
        layers.push(TerrainLayer { tile_m: s.tile_m, gain, albedo, normal });
    }
    Some(TerrainMaterials { size: SIZE, layers })
}

type Packed = (Vec<Vec<u8>>, Vec<Vec<u8>>, [f32; 3]);

fn find(dir: &Path, map: &str) -> Option<PathBuf> {
    let suffix = format!("_{map}.jpg");
    std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.ends_with(&suffix)))
}

/// A map resized to `SIZE` (box filter), as RGB8 or L8 per `channels`.
fn read_map(dir: &Path, map: &str, rgb: bool) -> Option<Vec<u8>> {
    let img = image::open(find(dir, map)?).ok()?;
    let img = img.resize_exact(SIZE, SIZE, image::imageops::FilterType::Triangle);
    Some(if rgb { img.to_rgb8().into_raw() } else { img.to_luma8().into_raw() })
}

fn srgb_to_linear(v: f32) -> f32 {
    if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}

fn linear_to_srgb(v: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    if v <= 0.0031308 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }
}

fn pack(dir: &Path) -> Option<Packed> {
    let n = SIZE as usize;
    let base = read_map(dir, "BaseColor", true)?;
    let disp = read_map(dir, "Displacement", false)?;
    let nrm = read_map(dir, "Normal", true)?;
    let rough = read_map(dir, "Roughness", false).unwrap_or_else(|| vec![200; n * n]);
    let ao = read_map(dir, "AO", false).unwrap_or_else(|| vec![255; n * n]);
    let mut a0 = vec![0u8; n * n * 4];
    let mut n0 = vec![0u8; n * n * 4];
    let mut sum = [0f64; 3];
    for i in 0..n * n {
        for c in 0..3 {
            a0[4 * i + c] = base[3 * i + c];
            sum[c] += srgb_to_linear(base[3 * i + c] as f32 / 255.0) as f64;
        }
        a0[4 * i + 3] = disp[i];
        n0[4 * i] = nrm[3 * i];
        n0[4 * i + 1] = nrm[3 * i + 1];
        n0[4 * i + 2] = rough[i];
        n0[4 * i + 3] = ao[i];
    }
    let mean = sum.map(|s| (s / (n * n) as f64) as f32);
    Some((mips(a0, true), mips(n0, false), mean))
}

/// The full mip chain of an RGBA8 square image (2x2 box; colour averaged in linear light).
fn mips(top: Vec<u8>, srgb: bool) -> Vec<Vec<u8>> {
    let mut out = vec![top];
    let mut s = SIZE as usize;
    while s > 1 {
        let h = s / 2;
        let prev = out.last().unwrap();
        let mut next = vec![0u8; h * h * 4];
        for y in 0..h {
            for x in 0..h {
                for c in 0..4 {
                    let at = |dx: usize, dy: usize| prev[4 * ((2 * y + dy) * s + 2 * x + dx) + c] as f32 / 255.0;
                    let q = [at(0, 0), at(1, 0), at(0, 1), at(1, 1)];
                    let v = if srgb && c < 3 { linear_to_srgb(q.iter().map(|&v| srgb_to_linear(v)).sum::<f32>() * 0.25) } else { q.iter().sum::<f32>() * 0.25 };
                    next[4 * (y * h + x) + c] = (v * 255.0 + 0.5) as u8;
                }
            }
        }
        out.push(next);
        s = h;
    }
    out
}

fn read_cache(path: &Path) -> Option<Packed> {
    let b = std::fs::read(path).ok()?;
    let mut mean = [0f32; 3];
    for (c, m) in mean.iter_mut().enumerate() {
        *m = f32::from_le_bytes(b.get(4 * c..4 * c + 4)?.try_into().ok()?);
    }
    let mut at = 12;
    let mut take = |len: usize| -> Option<Vec<u8>> {
        let v = b.get(at..at + len)?.to_vec();
        at += len;
        Some(v)
    };
    let sizes: Vec<usize> = (0..=SIZE.trailing_zeros()).map(|l| ((SIZE >> l) as usize).pow(2) * 4).collect();
    let albedo = sizes.iter().map(|&l| take(l)).collect::<Option<Vec<_>>>()?;
    let normal = sizes.iter().map(|&l| take(l)).collect::<Option<Vec<_>>>()?;
    (at == b.len()).then_some((albedo, normal, mean))
}

fn write_cache(path: &Path, (albedo, normal, mean): &Packed) -> std::io::Result<()> {
    std::fs::create_dir_all(path.parent().unwrap_or(Path::new(".")))?;
    let mut b = Vec::new();
    for m in mean {
        b.extend_from_slice(&m.to_le_bytes());
    }
    for l in albedo.iter().chain(normal) {
        b.extend_from_slice(l);
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, b)?;
    std::fs::rename(tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mip_chain_is_complete_and_preserves_flat_colour() {
        let n = SIZE as usize;
        let m = mips(vec![128u8; n * n * 4], true);
        assert_eq!(m.len(), SIZE.trailing_zeros() as usize + 1);
        assert_eq!(m.last().unwrap().len(), 4);
        assert!(m.last().unwrap().iter().all(|&v| v.abs_diff(128) <= 1));
    }

    #[test]
    fn layer_tiles_divide_the_texture_period() {
        for s in &SPECS {
            let k = ed_render::terrain::TEX_PERIOD_M / s.tile_m as f64;
            assert_eq!(k, k.round(), "{}", s.asset);
            // The 4x anti-tiling copy too.
            let k4 = k / 4.0;
            assert_eq!(k4, k4.round(), "{}", s.asset);
        }
    }
}
