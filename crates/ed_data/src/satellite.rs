//! Satellite colour: Sentinel-2 cloudless mosaic (EOX, CC BY-NC-SA 4.0,
//! contains modified Copernicus Sentinel data), as Web-Mercator XYZ JPEG
//! tiles. Used only to *sample colours* (ground tint, far-field albedo,
//! water colour), never drawn as a picture.

use crate::DiskCache;
use ed_geo::tile::TileId;

/// Native detail runs out around here; deeper tiles are just upsampled.
pub const MAX_ZOOM: u8 = 12;
const URL: &str = "https://tiles.maps.eox.at/wmts/1.0.0/s2cloudless-2020_3857/default/g";

/// A decoded 8-bit sRGB tile.
#[derive(Debug, Clone)]
pub struct SatTile {
    pub id: TileId,
    pub size: usize,
    rgb: Vec<u8>,
}

#[inline]
fn srgb_to_linear(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

impl SatTile {
    pub fn decode(id: TileId, jpeg: &[u8]) -> anyhow::Result<Self> {
        let mut dec = jpeg_decoder::Decoder::new(std::io::Cursor::new(jpeg));
        let px = dec.decode()?;
        let info = dec.info().ok_or_else(|| anyhow::anyhow!("no jpeg info"))?;
        anyhow::ensure!(info.width == info.height, "satellite tile not square");
        let size = info.width as usize;
        let rgb = match info.pixel_format {
            jpeg_decoder::PixelFormat::RGB24 => px,
            jpeg_decoder::PixelFormat::L8 => px.iter().flat_map(|&g| [g, g, g]).collect(),
            f => anyhow::bail!("unsupported jpeg format {f:?}"),
        };
        anyhow::ensure!(rgb.len() == size * size * 3, "short jpeg");
        Ok(Self { id, size, rgb })
    }

    /// Raw 8-bit sRGB pixels, row-major RGB triplets (`size * size * 3`).
    pub fn rgb(&self) -> &[u8] {
        &self.rgb
    }

    /// Bilinear sample at tile (u, v) in [0,1], returning **linear** RGB.
    pub fn sample(&self, u: f64, v: f64) -> [f32; 3] {
        let n = self.size;
        let fx = (u * n as f64 - 0.5).clamp(0.0, (n - 1) as f64);
        let fy = (v * n as f64 - 0.5).clamp(0.0, (n - 1) as f64);
        let (x0, y0) = (fx as usize, fy as usize);
        let (x1, y1) = ((x0 + 1).min(n - 1), (y0 + 1).min(n - 1));
        let (tx, ty) = ((fx - x0 as f64) as f32, (fy - y0 as f64) as f32);
        let px = |x: usize, y: usize, c: usize| srgb_to_linear(self.rgb[(y * n + x) * 3 + c]);
        let mut out = [0.0; 3];
        for (c, o) in out.iter_mut().enumerate() {
            let a = px(x0, y0, c) * (1.0 - tx) + px(x1, y0, c) * tx;
            let b = px(x0, y1, c) * (1.0 - tx) + px(x1, y1, c) * tx;
            *o = a * (1.0 - ty) + b * ty;
        }
        out
    }
}

/// Fetch (cached forever) and decode one tile. `id.z` must be <= [`MAX_ZOOM`].
pub fn fetch(cache: &DiskCache, id: TileId) -> anyhow::Result<SatTile> {
    anyhow::ensure!(id.z <= MAX_ZOOM, "satellite tiles stop at z{MAX_ZOOM}");
    let key = format!("eox-s2/{}.jpg", id.path());
    // WMTS REST order is {TileMatrix}/{TileRow}/{TileCol} = z/y/x (not slippy z/x/y).
    let url = format!("{URL}/{}/{}/{}.jpg", id.z, id.y, id.x);
    let bytes = cache.get_or_fetch(&key, None, || crate::http::get_bytes(&url))?;
    SatTile::decode(id, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hits the network. `cargo test -p ed_data -- --ignored`
    #[test]
    #[ignore]
    fn ocean_is_blue_land_is_not() {
        let cache = DiskCache::new(std::env::temp_dir().join("ed-net-test"));
        let t = fetch(&cache, TileId::containing(0.0, -30.0, 3)).unwrap(); // mid-Atlantic
        let (u, v) = t.id.geodetic_to_uv(0.0, -30.0);
        let c = t.sample(u, v);
        assert!(c[2] > c[0], "ocean should be bluer than red: {c:?}");
    }
}
