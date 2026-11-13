//! ESA WorldCover 2021 v200 (10 m land cover, CC BY 4.0) read straight from
//! its public Cloud-Optimized GeoTIFFs on S3 (3°x3° tiles, 36000² px, with
//! overviews). Only the chunks a game tile needs are downloaded, then cached.
//!
//! Output is the raw class code per cell plus the fraction of samples that were
//! tree/mangrove, which doubles as a canopy-density proxy for tree placement.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use std::io::{Read, Seek, SeekFrom};

use tiff::decoder::Decoder;
use tiff::tags::Tag;

use crate::cache::DiskCache;
use crate::cog::RangeReader;

pub const TREE: u8 = 10;
pub const SHRUB: u8 = 20;
pub const GRASS: u8 = 30;
pub const CROP: u8 = 40;
pub const BUILT: u8 = 50;
pub const BARE: u8 = 60;
pub const SNOW: u8 = 70;
pub const WATER: u8 = 80;
pub const WETLAND: u8 = 90;
pub const MANGROVE: u8 = 95;
pub const MOSS: u8 = 100;

/// Per-cell result of [`sample_area`], row-major from the north-west corner.
#[derive(Debug, Clone)]
pub struct LandCover {
    pub w: usize,
    pub h: usize,
    /// Most common WorldCover class in the cell (0 = no data).
    pub class: Vec<u8>,
    /// Share of samples that were tree or mangrove, 0..=255.
    pub tree: Vec<u8>,
    /// Share of samples that were shrub, 0..=255.
    pub shrub: Vec<u8>,
}

fn url_for(name: &str) -> String {
    format!("https://esa-worldcover.s3.eu-central-1.amazonaws.com/v200/2021/map/ESA_WorldCover_10m_2021_v200_{name}_Map.tif")
}

/// Tile name from the south-west corner, e.g. `N24E027`, `S06W060`.
pub fn tile_name(lat: f64, lon: f64) -> String {
    let la = (lat / 3.0).floor() as i32 * 3;
    let lo = (lon / 3.0).floor() as i32 * 3;
    format!("{}{:02}{}{:03}", if la >= 0 { 'N' } else { 'S' }, la.abs(), if lo >= 0 { 'E' } else { 'W' }, lo.abs())
}

struct Level {
    w: u32,
    h: u32,
    cw: u32,
    ch: u32,
    offsets: Vec<u64>,
    counts: Vec<u64>,
    deflate: bool,
    predictor: u32,
}

/// The `tiff` crate refuses palette-photometric images when decoding pixels
/// (WorldCover ships a colour table), so it is used for metadata only and the
/// tiles are read and inflated here.
struct Tile {
    rd: RangeReader,
    levels: Vec<Level>,
    chunks: HashMap<(usize, u32), Arc<Vec<u8>>>,
    sw_lat: f64,
    sw_lon: f64,
}

impl Tile {
    fn open(name: &str, sw_lat: f64, sw_lon: f64, cache: &DiskCache) -> anyhow::Result<Option<Self>> {
        let key = format!("worldcover/{name}.tif");
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
            anyhow::ensure!(matches!(comp, 1 | 8 | 32946), "worldcover: unsupported compression {comp}");
            anyhow::ensure!(predictor <= 2, "worldcover: unsupported predictor {predictor}");
            levels.push(Level { w, h, cw, ch, offsets, counts, deflate: comp != 1, predictor });
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
        Ok(c.get(y * cw as usize + x).copied().unwrap_or(0))
    }

    fn load_chunk(&mut self, lv: usize, idx: u32) -> anyhow::Result<Vec<u8>> {
        let l = &self.levels[lv];
        let (off, len) = (*l.offsets.get(idx as usize).unwrap_or(&0), *l.counts.get(idx as usize).unwrap_or(&0));
        if len == 0 {
            return Ok(Vec::new()); // sparse tile: no data
        }
        let mut raw = vec![0u8; len as usize];
        self.rd.seek(SeekFrom::Start(off))?;
        self.rd.read_exact(&mut raw)?;
        let mut data = if l.deflate {
            let mut out = Vec::with_capacity((l.cw * l.ch) as usize);
            flate2::read::ZlibDecoder::new(&raw[..]).read_to_end(&mut out)?;
            out
        } else {
            raw
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

/// Land cover over a lat/lon box as a `w`x`h` grid; each cell takes `sub`x`sub`
/// samples for its class mode and tree/shrub fractions. A missing WorldCover
/// tile (ocean) yields class 0. Errors mean "network unavailable": callers
/// should fall back to OSM-only cover.
pub fn sample_area(south: f64, west: f64, north: f64, east: f64, w: usize, h: usize, sub: usize, cache: &DiskCache) -> anyhow::Result<LandCover> {
    let sub = sub.max(1);
    let cell_deg = ((east - west) / w as f64).min((north - south) / h as f64);
    let target = cell_deg / sub as f64;
    let mut out = LandCover { w, h, class: vec![0; w * h], tree: vec![0; w * h], shrub: vec![0; w * h] };
    let n = (sub * sub) as f32;
    for j in 0..h {
        for i in 0..w {
            let mut counts = [0u16; 128];
            let (mut trees, mut shrubs) = (0u32, 0u32);
            for b in 0..sub {
                for a in 0..sub {
                    let lon = west + (east - west) * ((i as f64 + (a as f64 + 0.5) / sub as f64) / w as f64);
                    let lat = north - (north - south) * ((j as f64 + (b as f64 + 0.5) / sub as f64) / h as f64);
                    let Some(t) = tile(lat, lon, cache)? else { continue };
                    let mut t = t.lock().unwrap();
                    let lv = t.level_for(target);
                    let c = t.pixel(lv, lat, lon)?;
                    counts[(c as usize).min(127)] += 1;
                    if c == TREE || c == MANGROVE {
                        trees += 1;
                    } else if c == SHRUB {
                        shrubs += 1;
                    }
                }
            }
            let k = j * w + i;
            let (best, _) = counts.iter().enumerate().skip(1).fold((0usize, 0u16), |m, (c, &n)| if n > m.1 { (c, n) } else { m });
            out.class[k] = best as u8;
            out.tree[k] = (trees as f32 / n * 255.0).round() as u8;
            out.shrub[k] = (shrubs as f32 / n * 255.0).round() as u8;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(tile_name(24.5, 27.1), "N24E027");
        assert_eq!(tile_name(-3.1, -60.0), "S06W060");
        assert_eq!(tile_name(45.0, -89.9), "N45W090");
    }

    #[test]
    #[ignore = "network"]
    fn real_tiles() {
        let cache = DiskCache::default_location();
        // Western Desert, Egypt: bare.
        let d = sample_area(25.0, 27.0, 25.02, 27.02, 4, 4, 3, &cache).unwrap();
        assert!(d.class.iter().filter(|&&c| c == BARE).count() >= 12, "{:?}", d.class);
        // Northern Wisconsin: forest.
        let f = sample_area(45.8, -90.0, 45.82, -89.98, 4, 4, 3, &cache).unwrap();
        assert!(f.tree.iter().map(|&t| t as u32).sum::<u32>() / 16 > 80, "{:?}", f.tree);
    }
}
