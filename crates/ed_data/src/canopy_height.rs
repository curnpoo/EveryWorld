//! ETH Global Canopy Height 2020, 10 m (Lang et al. 2023, CC BY 4.0; registry row in
//! `docs/data/`). Read straight from the authors' Cloud-Optimized GeoTIFFs on ETH's libdrive
//! (3 deg x 3 deg tiles named by the south-west corner, `..._N45E006_Map.tif`, uint8 metres,
//! 255 = no data, 0 = no canopy, LZW compressed, with overviews). The server honours HTTP Range, so only the
//! chunks a node window needs are downloaded, through [`RangeReader`] and the [`DiskCache`].
//! Same shape as [`crate::worldcover`].

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::sync::{Arc, Mutex, OnceLock};

use tiff::decoder::Decoder;
use tiff::tags::Tag;

use crate::cache::DiskCache;
use crate::cog::RangeReader;

/// Raw value meaning "no data" in the source.
pub const NODATA: u8 = 255;

/// Per-cell result of [`sample_area`], row-major from the north-west corner.
#[derive(Debug, Clone)]
pub struct CanopyHeight {
    pub w: usize,
    pub h: usize,
    /// Mean canopy top (m) of the cell's tree samples (samples with height >= 1 m); 0 where
    /// there are none or no data.
    pub height: Vec<f32>,
    /// Share of the cell's samples that carried data (not 255, tile present), `0..=255`.
    /// 0 means "no data here": callers fall back to allometry.
    pub data: Vec<u8>,
}

fn url_for(name: &str) -> String {
    format!("https://libdrive.ethz.ch/index.php/s/cO8or7iOe5dT2Rt/download?path=%2F3deg_cogs&files=ETH_GlobalCanopyHeight_10m_2020_{name}_Map.tif")
}

/// Tile name from the south-west corner, e.g. `N45E006`, `S06W060`.
pub fn tile_name(lat: f64, lon: f64) -> String {
    crate::worldcover::tile_name(lat, lon)
}

struct Level {
    w: u32,
    h: u32,
    cw: u32,
    ch: u32,
    offsets: Vec<u64>,
    counts: Vec<u64>,
    comp: u32,
    predictor: u32,
}

struct Tile {
    rd: RangeReader,
    levels: Vec<Level>,
    chunks: HashMap<(usize, u32), Arc<Vec<u8>>>,
    sw_lat: f64,
    sw_lon: f64,
}

impl Tile {
    fn open(name: &str, sw_lat: f64, sw_lon: f64, cache: &DiskCache) -> anyhow::Result<Option<Self>> {
        let key = format!("canopy_eth/{name}.tif");
        let Some(r) = RangeReader::open(&url_for(name), &key, cache)? else { return Ok(None) };
        let mut dec = Decoder::new(r)?;
        let rd = RangeReader::open(&url_for(name), &key, cache)?.expect("just opened");
        let mut levels = Vec::new();
        for ifd in 0..16 {
            if ifd > 0 && dec.seek_to_image(ifd).is_err() {
                break;
            }
            let (w, h) = dec.dimensions()?;
            let (cw, ch) = dec.chunk_dimensions();
            let offsets = dec.get_tag_u64_vec(Tag::TileOffsets)?;
            let counts = dec.get_tag_u64_vec(Tag::TileByteCounts)?;
            let comp = dec.get_tag_u32(Tag::Compression)?;
            let predictor = dec.find_tag_unsigned::<u32>(Tag::Predictor)?.unwrap_or(1);
            anyhow::ensure!(matches!(comp, 1 | 5 | 8 | 32946), "canopy_height: unsupported compression {comp}");
            anyhow::ensure!(predictor <= 2, "canopy_height: unsupported predictor {predictor}");
            levels.push(Level { w, h, cw, ch, offsets, counts, comp, predictor });
        }
        Ok(Some(Tile { rd, levels, chunks: HashMap::new(), sw_lat, sw_lon }))
    }

    /// Pick the coarsest level whose pixel is no bigger than `target_deg`.
    fn level_for(&self, target_deg: f64) -> usize {
        let mut best = 0;
        for (i, l) in self.levels.iter().enumerate() {
            if 3.0 / l.w as f64 <= target_deg {
                best = i;
            }
        }
        best
    }

    /// Raw byte at the pixel under `(lat, lon)`; `NODATA` for a sparse (absent) chunk.
    fn pixel(&mut self, lv: usize, lat: f64, lon: f64) -> anyhow::Result<u8> {
        let (w, h, cw, ch) = {
            let l = &self.levels[lv];
            (l.w, l.h, l.cw, l.ch)
        };
        let px = (((lon - self.sw_lon) / 3.0) * w as f64).floor().clamp(0.0, (w - 1) as f64) as u32;
        let py = (((self.sw_lat + 3.0 - lat) / 3.0) * h as f64).floor().clamp(0.0, (h - 1) as f64) as u32;
        let across = w.div_ceil(cw);
        let idx = (py / ch) * across + px / cw;
        if !self.chunks.contains_key(&(lv, idx)) {
            if self.chunks.len() > 48 {
                self.chunks.clear();
            }
            let data = self.load_chunk(lv, idx)?;
            self.chunks.insert((lv, idx), Arc::new(data));
        }
        let c = &self.chunks[&(lv, idx)];
        let (x, y) = ((px % cw) as usize, (py % ch) as usize);
        Ok(c.get(y * cw as usize + x).copied().unwrap_or(NODATA))
    }

    fn load_chunk(&mut self, lv: usize, idx: u32) -> anyhow::Result<Vec<u8>> {
        let l = &self.levels[lv];
        let (off, len) = (*l.offsets.get(idx as usize).unwrap_or(&0), *l.counts.get(idx as usize).unwrap_or(&0));
        if len == 0 {
            return Ok(Vec::new());
        }
        let mut raw = vec![0u8; len as usize];
        self.rd.seek(SeekFrom::Start(off))?;
        self.rd.read_exact(&mut raw)?;
        let mut data = match l.comp {
            5 => weezl::decode::Decoder::with_tiff_size_switch(weezl::BitOrder::Msb, 8)
                .decode(&raw)
                .map_err(|e| anyhow::anyhow!("canopy_height: lzw: {e}"))?,
            8 | 32946 => {
                let mut out = Vec::with_capacity((l.cw * l.ch) as usize);
                flate2::read::ZlibDecoder::new(&raw[..]).read_to_end(&mut out)?;
                out
            }
            _ => raw,
        };
        if l.predictor == 2 {
            for row in data.chunks_mut(l.cw as usize) {
                for i in 1..row.len() {
                    row[i] = row[i].wrapping_add(row[i - 1]);
                }
            }
        }
        Ok(data)
    }
}

type Slot = Option<Arc<Mutex<Tile>>>;

fn tiles() -> &'static Mutex<HashMap<String, Slot>> {
    static T: OnceLock<Mutex<HashMap<String, Slot>>> = OnceLock::new();
    T.get_or_init(Default::default)
}

fn tile(lat: f64, lon: f64, cache: &DiskCache) -> anyhow::Result<Slot> {
    // Per-thread memo keyed by the 3 deg cell: bulk sampling asks per sample, and the global
    // lock plus name formatting dominated (and serialised) the coarse-layer build.
    thread_local! {
        static MEMO: std::cell::RefCell<HashMap<(i32, i32), Slot>> = Default::default();
    }
    let key = ((lat / 3.0).floor() as i32, (lon / 3.0).floor() as i32);
    if let Some(s) = MEMO.with(|m| m.borrow().get(&key).cloned()) {
        return Ok(s);
    }
    let name = tile_name(lat, lon);
    if let Some(s) = tiles().lock().unwrap().get(&name) {
        let s = s.clone();
        MEMO.with(|m| m.borrow_mut().insert(key, s.clone()));
        return Ok(s);
    }
    let sw_lat = (lat / 3.0).floor() * 3.0;
    let sw_lon = (lon / 3.0).floor() * 3.0;
    let slot = Tile::open(&name, sw_lat, sw_lon, cache)?.map(|t| Arc::new(Mutex::new(t)));
    let s = tiles().lock().unwrap().entry(name).or_insert(slot).clone();
    MEMO.with(|m| m.borrow_mut().insert(key, s.clone()));
    Ok(s)
}

/// Whether the 3 deg source tile under `(lat, lon)` exists (`false` for open ocean); an error
/// means the network is unavailable. Cheap after the first call: callers skip sampling a tile
/// that is not there.
pub fn tile_exists(lat: f64, lon: f64, cache: &DiskCache) -> anyhow::Result<bool> {
    Ok(tile(lat, lon, cache)?.is_some())
}

/// Canopy height (m) over a lat/lon box as a `w` x `h` grid; each cell averages `sub` x `sub`
/// samples. Cells with no data have `data == 0` and `height == 0`. Errors mean "network
/// unavailable": callers fall back to allometry.
pub fn sample_area(south: f64, west: f64, north: f64, east: f64, w: usize, h: usize, sub: usize, cache: &DiskCache) -> anyhow::Result<CanopyHeight> {
    let sub = sub.max(1);
    let cell_deg = ((east - west) / w as f64).min((north - south) / h as f64);
    let target = cell_deg / sub as f64;
    let mut out = CanopyHeight { w, h, height: vec![0.0; w * h], data: vec![0; w * h] };
    for j in 0..h {
        for i in 0..w {
            let (mut sum, mut trees, mut valid) = (0u32, 0u32, 0u32);
            for b in 0..sub {
                for a in 0..sub {
                    let lon = west + (east - west) * ((i as f64 + (a as f64 + 0.5) / sub as f64) / w as f64);
                    let lat = north - (north - south) * ((j as f64 + (b as f64 + 0.5) / sub as f64) / h as f64);
                    let Some(t) = tile(lat, lon, cache)? else { continue };
                    let mut t = t.lock().unwrap();
                    let lv = t.level_for(target);
                    let c = t.pixel(lv, lat, lon)?;
                    if c == NODATA {
                        continue;
                    }
                    valid += 1;
                    if c >= 1 {
                        sum += c as u32;
                        trees += 1;
                    }
                }
            }
            let k = j * w + i;
            out.data[k] = ((valid * 255 + (sub * sub) as u32 / 2) / (sub * sub) as u32) as u8;
            out.height[k] = if trees > 0 { sum as f32 / trees as f32 } else { 0.0 };
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(tile_name(46.02, 7.75), "N45E006");
        assert_eq!(tile_name(37.9, -78.9), "N36W081");
    }

    #[test]
    #[ignore = "network"]
    fn real_zermatt_and_blue_ridge() {
        let cache = DiskCache::default_location();
        // Zermatt valley slopes: forest 15-40 m somewhere in a 4 km window.
        let z = sample_area(46.00, 7.72, 46.04, 7.78, 16, 16, 2, &cache).unwrap();
        let tall = z.height.iter().filter(|&&h| (15.0..=40.0).contains(&h)).count();
        assert!(tall >= 8, "zermatt tall cells {tall}: {:?}", z.height);
        // Blue Ridge, Shenandoah NP west of Skyline Drive.
        let b = sample_area(38.50, -78.40, 38.54, -78.34, 16, 16, 2, &cache).unwrap();
        let tall = b.height.iter().filter(|&&h| h >= 15.0).count();
        assert!(tall >= 8, "blue ridge tall cells {tall}: {:?}", b.height);
    }
}
