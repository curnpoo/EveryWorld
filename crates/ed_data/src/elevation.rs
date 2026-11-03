//! Global elevation in Terrarium encoding: `height_m = R*256 + G + B/256 - 32768`.
//!
//! Primary: **Mapterhorn** (512 px WebP, Copernicus GLO-30 + national lidar
//! like swisstopo / USGS 3DEP where available).
//! Fallback: **AWS Terrain Tiles** (256 px PNG; SRTM, GMTED, ETOPO1 bathymetry).
//! Both are free and keyless. Oceans have real seafloor depth.

use crate::DiskCache;
use ed_geo::tile::TileId;

pub const MAX_ZOOM: u8 = 15;
const AWS_URL: &str = "https://s3.amazonaws.com/elevation-tiles-prod/terrarium";
const MAPTERHORN_URL: &str = "https://tiles.mapterhorn.com";

/// A decoded square grid of heights (meters), row 0 = north edge.
#[derive(Debug, Clone)]
pub struct HeightTile {
    pub id: TileId,
    pub size: usize,
    pub heights: Vec<f32>,
}

impl HeightTile {
    pub fn decode_terrarium(id: TileId, png_bytes: &[u8]) -> anyhow::Result<Self> {
        let mut dec = png::Decoder::new(std::io::Cursor::new(png_bytes));
        dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = dec.read_info()?;
        let mut buf = vec![0; reader.output_buffer_size().ok_or_else(|| anyhow::anyhow!("png too large"))?];
        let info = reader.next_frame(&mut buf)?;
        let (w, h) = (info.width as usize, info.height as usize);
        anyhow::ensure!(w == h, "terrarium tile not square: {w}x{h}");
        let ch = info.color_type.samples();
        anyhow::ensure!(ch >= 3, "terrarium tile has {ch} channels");
        let heights = buf[..w * h * ch]
            .chunks_exact(ch)
            .map(|p| (p[0] as f32 * 256.0 + p[1] as f32 + p[2] as f32 / 256.0) - 32768.0)
            .collect();
        Ok(Self { id, size: w, heights })
    }

    pub fn decode_terrarium_webp(id: TileId, webp_bytes: &[u8]) -> anyhow::Result<Self> {
        let mut dec = image_webp::WebPDecoder::new(std::io::Cursor::new(webp_bytes))?;
        let (w, h) = dec.dimensions();
        let (w, h) = (w as usize, h as usize);
        anyhow::ensure!(w == h, "terrarium tile not square: {w}x{h}");
        let ch = if dec.has_alpha() { 4 } else { 3 };
        let mut buf = vec![0u8; w * h * ch];
        dec.read_image(&mut buf)?;
        let heights = buf
            .chunks_exact(ch)
            .map(|p| (p[0] as f32 * 256.0 + p[1] as f32 + p[2] as f32 / 256.0) - 32768.0)
            .collect();
        Ok(Self { id, size: w, heights })
    }

    pub fn flat(id: TileId, size: usize, h: f32) -> Self {
        Self { id, size, heights: vec![h; size * size] }
    }

    #[inline]
    pub fn at(&self, x: usize, y: usize) -> f32 {
        self.heights[y.min(self.size - 1) * self.size + x.min(self.size - 1)]
    }

    /// Bilinear sample, `u`,`v` in [0,1] across the tile (pixel-center aligned
    /// edge-to-edge so neighbouring tiles agree on shared edges).
    pub fn sample(&self, u: f64, v: f64) -> f32 {
        let s = (self.size - 1) as f64;
        let fx = (u.clamp(0.0, 1.0) * s) as f32;
        let fy = (v.clamp(0.0, 1.0) * s) as f32;
        let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
        let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
        let a = self.at(x0, y0) * (1.0 - tx) + self.at(x0 + 1, y0) * tx;
        let b = self.at(x0, y0 + 1) * (1.0 - tx) + self.at(x0 + 1, y0 + 1) * tx;
        a * (1.0 - ty) + b * ty
    }
}

/// Fetch (through the cache) and decode an elevation tile. Zooms above
/// [`MAX_ZOOM`] are served by sampling the z15 ancestor.
pub fn fetch(cache: &DiskCache, id: TileId) -> anyhow::Result<HeightTile> {
    if id.z > MAX_ZOOM {
        let (anc, u0, v0, s) = id.ancestor(MAX_ZOOM);
        let parent = fetch(cache, anc)?;
        let size = 256;
        let mut heights = Vec::with_capacity(size * size);
        for y in 0..size {
            for x in 0..size {
                let u = u0 + s * x as f64 / (size - 1) as f64;
                let v = v0 + s * y as f64 / (size - 1) as f64;
                heights.push(parent.sample(u, v));
            }
        }
        return Ok(HeightTile { id, size, heights });
    }
    let mt_key = format!("mapterhorn/{}.webp", id.path());
    let mt = cache.get_or_fetch(&mt_key, None, || {
        Ok(crate::http::get_bytes_opt(&format!("{MAPTERHORN_URL}/{}.webp", id.path()))?.unwrap_or_default())
    });
    match mt {
        Ok(bytes) if !bytes.is_empty() => match HeightTile::decode_terrarium_webp(id, &bytes) {
            Ok(t) => return Ok(t),
            Err(e) => log::warn!("mapterhorn {} undecodable, falling back: {e}", id.path()),
        },
        Ok(_) => {}
        Err(e) => log::warn!("mapterhorn {} failed, falling back: {e}", id.path()),
    }
    let key = format!("terrarium/{}.png", id.path());
    let bytes = cache.get_or_fetch(&key, None, || crate::http::get_bytes(&format!("{AWS_URL}/{}.png", id.path())))?;
    HeightTile::decode_terrarium(id, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_bilinear() {
        let mut t = HeightTile::flat(TileId::new(0, 0, 0), 2, 0.0);
        t.heights = vec![0.0, 10.0, 20.0, 30.0];
        assert_eq!(t.sample(0.0, 0.0), 0.0);
        assert_eq!(t.sample(1.0, 1.0), 30.0);
        assert_eq!(t.sample(0.5, 0.5), 15.0);
    }

    /// Hits the network. `cargo test -p ed_data -- --ignored`
    #[test]
    #[ignore]
    fn matterhorn_is_tall() {
        let cache = DiskCache::new(std::env::temp_dir().join("ed-net-test"));
        let id = TileId::containing(45.9763, 7.6586, 12);
        let t = fetch(&cache, id).unwrap();
        let (u, v) = id.geodetic_to_uv(45.9763, 7.6586);
        let h = t.sample(u, v);
        assert!((4000.0..4500.0).contains(&h), "matterhorn summit sampled at {h}");
    }
}
