//! Coarse canopy layer (trees G3b): per 3 deg source tile, a baked raster at 1/128 deg
//! (~870 m) of `[tree fraction, mean canopy height of treed pixels, data share]`, built once
//! from a coarse overview of WorldCover and the ETH canopy height (aggregated, never
//! point-sampled), and cached as a small deflated binary (`canopy_coarse/v1/N45E006.bin`).
//! Canopy pages whose lattice spacing is [`COARSE_MIN_SPACING_M`] or more read this instead of
//! the 10 m sources (an orbit view then costs a few megabytes, not hundreds).
//!
//! In memory a tile carries a mip chain (cell `2^m` base cells, `m` up to [`MAX_MIP`]) so a page
//! sample is one bilinear read at the mip whose cell matches the page spacing. Deterministic: a
//! pure function of the sources.

use std::collections::HashMap;
use std::io::Read;
use std::sync::{Arc, Mutex, OnceLock};

use crate::cache::DiskCache;
use crate::{canopy_height, worldcover};

/// Cells per 3 deg tile side (1/128 deg each).
pub const COARSE_N: usize = 384;
/// Cell size in degrees of mip 0.
pub const COARSE_CELL_DEG: f64 = 3.0 / COARSE_N as f64;
/// Highest mip kept (cell `2^7` base cells, 3 cells per tile).
pub const MAX_MIP: usize = 7;
/// Pages with at least this lattice spacing (m) read the coarse layer. Below it the 10 m path is
/// cheap enough (a handful of chunk reads) and the cells would be coarser than the page.
pub const COARSE_MIN_SPACING_M: f64 = 400.0;
/// Approximate size (m) of a mip-0 cell at the equator, for picking the mip from a spacing.
pub const COARSE_CELL_M: f64 = 869.0;
const VERSION: u8 = 1;

/// One 3 deg tile, rows from the north edge. Planes are `u8`: `cover` 0..=255 is the tree
/// fraction, `height` is metres (0 = none or no ETH data), `data` 0..=255.
pub struct CoarseTile {
    /// `mips[m]` is a `(N >> m)` square, each plane `n * n` bytes: cover, height, data.
    mips: Vec<[Vec<u8>; 3]>,
}

impl CoarseTile {
    fn from_planes(base: [Vec<u8>; 3]) -> CoarseTile {
        let mut mips = vec![base];
        for m in 1..=MAX_MIP {
            let n = COARSE_N >> m;
            let src = &mips[m - 1];
            let sn = n * 2;
            let (mut c, mut h, mut d) = (vec![0u8; n * n], vec![0u8; n * n], vec![0u8; n * n]);
            for j in 0..n {
                for i in 0..n {
                    let (mut cs, mut hs, mut hw, mut ds) = (0u32, 0u32, 0u32, 0u32);
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let k = (2 * j + dy) * sn + 2 * i + dx;
                        cs += src[0][k] as u32;
                        hs += src[0][k] as u32 * src[1][k] as u32;
                        hw += src[0][k] as u32;
                        ds += src[2][k] as u32;
                    }
                    let k = j * n + i;
                    c[k] = ((cs + 2) / 4) as u8;
                    h[k] = if hw > 0 { ((hs + hw / 2) / hw) as u8 } else { 0 };
                    d[k] = ((ds + 2) / 4) as u8;
                }
            }
            mips.push([c, h, d]);
        }
        CoarseTile { mips }
    }

    /// Cells per side at mip `m`.
    pub fn n(m: usize) -> usize {
        COARSE_N >> m
    }

    /// `[cover, height_m, data]` of cell `(i, j)` (row from the north) at mip `m`.
    #[inline]
    pub fn cell(&self, m: usize, i: usize, j: usize) -> [u8; 3] {
        let n = Self::n(m);
        let k = j * n + i;
        let p = &self.mips[m];
        [p[0][k], p[1][k], p[2][k]]
    }

    /// Cached form: `[VERSION, N lo, N hi]` then the three base planes, deflated.
    pub fn to_bytes(&self) -> Vec<u8> {
        use flate2::{write::ZlibEncoder, Compression};
        use std::io::Write;
        let mut raw = Vec::with_capacity(3 * COARSE_N * COARSE_N);
        for p in &self.mips[0] {
            raw.extend_from_slice(p);
        }
        let mut e = ZlibEncoder::new(vec![VERSION, (COARSE_N & 255) as u8, (COARSE_N >> 8) as u8], Compression::best());
        e.write_all(&raw).expect("in-memory write");
        e.finish().expect("in-memory finish")
    }

    pub fn from_bytes(b: &[u8]) -> Option<CoarseTile> {
        if b.len() < 3 || b[0] != VERSION || b[1] as usize | (b[2] as usize) << 8 != COARSE_N {
            return None;
        }
        let mut raw = Vec::new();
        flate2::read::ZlibDecoder::new(&b[3..]).read_to_end(&mut raw).ok()?;
        let pn = COARSE_N * COARSE_N;
        if raw.len() != 3 * pn {
            return None;
        }
        Some(CoarseTile::from_planes([raw[..pn].to_vec(), raw[pn..2 * pn].to_vec(), raw[2 * pn..].to_vec()]))
    }

    /// Builds the tile with south-west corner `(sw_lat, sw_lon)` from the overviews of WorldCover
    /// and the ETH canopy height. `Err` = a source could not be read (offline): nothing is
    /// cached. A tile with no WorldCover (open ocean) is an all-zero tile with data 255.
    pub fn build(sw_lat: f64, sw_lon: f64, cache: &DiskCache) -> anyhow::Result<CoarseTile> {
        // sub 2 reads the ~300 m overview (the coarsest one whose pixel is under half a cell),
        // four samples per cell.
        let (s, w, n, e) = (sw_lat, sw_lon, sw_lat + 3.0, sw_lon + 3.0);
        let pn = COARSE_N * COARSE_N;
        let (mut c, mut h, d) = (vec![0u8; pn], vec![0u8; pn], vec![255u8; pn]);
        // A source tile that does not exist (open ocean) is skipped, not sampled a million times.
        let (mid_lat, mid_lon) = (sw_lat + 1.5, sw_lon + 1.5);
        // Both sources are probed first, so an unreachable one fails before any sampling work.
        let wc_ok = worldcover::tile_exists(mid_lat, mid_lon, cache)?;
        let eth_ok = canopy_height::tile_exists(mid_lat, mid_lon, cache)?;
        if wc_ok {
            // A coarse probe (32x32 samples over the whole tile) touches every overview chunk, so a
            // source that is not fully reachable fails here, not after a long sampling pass.
            worldcover::sample_area(s, w, n, e, 32, 32, 1, cache)?;
            if eth_ok {
                canopy_height::sample_area(s, w, n, e, 32, 32, 1, cache)?;
            }
            let lc = worldcover::sample_area(s, w, n, e, COARSE_N, COARSE_N, 2, cache)?;
            c.copy_from_slice(&lc.tree);
            if eth_ok {
                let eth = canopy_height::sample_area(s, w, n, e, COARSE_N, COARSE_N, 2, cache)?;
                for k in 0..pn {
                    if eth.data[k] > 127 && eth.height[k] >= 1.0 {
                        h[k] = libm::roundf(eth.height[k]).min(254.0) as u8;
                    }
                }
            }
        }
        Ok(CoarseTile::from_planes([c, h, d]))
    }

    /// Reads the cached tile or builds (and caches) it.
    pub fn load(sw_lat: f64, sw_lon: f64, cache: &DiskCache) -> anyhow::Result<CoarseTile> {
        let key = format!("canopy_coarse/v{VERSION}/{}.bin", worldcover::tile_name(sw_lat + 0.5, sw_lon + 0.5));
        if let Some(t) = cache.get(&key, None).and_then(|b| CoarseTile::from_bytes(&b)) {
            return Ok(t);
        }
        let t = CoarseTile::build(sw_lat, sw_lon, cache)?;
        let _ = cache.put(&key, &t.to_bytes());
        Ok(t)
    }
}

type Slot = Arc<OnceLock<Option<Arc<CoarseTile>>>>;

/// The coarse tile with tile indices `(tj, ti)` (south-west corner `3 tj`, `3 ti` degrees),
/// shared by every thread; a failed read is remembered for the process (`None`).
pub fn shared_tile(tj: i64, ti: i64, cache: &DiskCache) -> Option<Arc<CoarseTile>> {
    static T: OnceLock<Mutex<HashMap<(i64, i64), Slot>>> = OnceLock::new();
    let slot = T.get_or_init(Default::default).lock().unwrap().entry((tj, ti)).or_default().clone();
    slot.get_or_init(|| match CoarseTile::load(tj as f64 * 3.0, ti as f64 * 3.0, cache) {
        Ok(t) => Some(Arc::new(t)),
        Err(e) => {
            log::warn!("canopy_coarse: tile ({}, {}): {e}", tj * 3, ti * 3);
            None
        }
    })
    .clone()
}

/// Mip whose cell best matches a page lattice spacing (m): the largest cell not above it.
pub fn mip_for_spacing(spacing_m: f64) -> usize {
    ((spacing_m / COARSE_CELL_M).max(1.0).log2().floor() as usize).min(MAX_MIP)
}

/// Bilinear `[cover 0..=1, height m, data 0..=1]` at `(lat, lon)` and mip `m` over the global
/// coarse grid. `tile(tj, ti)` supplies tiles; taps on missing tiles drop out, their weight
/// leaving `data` (so a data border fades over one cell). `None` when no tap has data. Height
/// is cover-weighted. Taps never straddle a tile (global cell indices), so tile borders agree.
pub fn sample(tile: &mut dyn FnMut(i64, i64) -> Option<Arc<CoarseTile>>, lat: f64, lon: f64, m: usize) -> Option<[f32; 3]> {
    let n = CoarseTile::n(m) as i64;
    let cs = COARSE_CELL_DEG * (1u64 << m) as f64;
    let (fx, fy) = (lon / cs - 0.5, lat / cs - 0.5);
    let (gx, gy) = (fx.floor(), fy.floor());
    let (tx, ty) = ((fx - gx) as f32, (fy - gy) as f32);
    let (gx, gy) = (gx as i64, gy as i64);
    let (mut cover, mut hs, mut hw, mut data, mut wsum) = (0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32);
    for k in 0..4 {
        let (dx, dy) = ((k & 1) as i64, (k >> 1) as i64);
        let w = (if dx == 1 { tx } else { 1.0 - tx }) * (if dy == 1 { ty } else { 1.0 - ty });
        let (cx, cy) = (gx + dx, gy + dy);
        let Some(t) = tile(cy.div_euclid(n), cx.div_euclid(n)) else { continue };
        let c = t.cell(m, cx.rem_euclid(n) as usize, (n - 1 - cy.rem_euclid(n)) as usize);
        let cf = c[0] as f32 / 255.0;
        cover += w * cf;
        hs += w * cf * c[1] as f32;
        hw += w * cf;
        data += w * c[2] as f32 / 255.0;
        wsum += w;
    }
    if wsum <= 0.0 {
        return None;
    }
    Some([cover / wsum, if hw > 0.0 { hs / hw } else { 0.0 }, data])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(cover: u8, height: u8) -> Arc<CoarseTile> {
        let pn = COARSE_N * COARSE_N;
        Arc::new(CoarseTile::from_planes([vec![cover; pn], vec![height; pn], vec![255; pn]]))
    }

    #[test]
    fn bytes_round_trip_and_mips() {
        let pn = COARSE_N * COARSE_N;
        let c: Vec<u8> = (0..pn).map(|k| (k * 7 % 251) as u8).collect();
        let t = CoarseTile::from_planes([c, vec![20; pn], vec![255; pn]]);
        let b = t.to_bytes();
        let u = CoarseTile::from_bytes(&b).unwrap();
        for m in 0..=MAX_MIP {
            assert_eq!(t.mips[m][0], u.mips[m][0]);
        }
        assert!(CoarseTile::from_bytes(&b[1..]).is_none());
        assert_eq!(u.cell(MAX_MIP, 0, 0)[1], 20);
    }

    #[test]
    fn sample_is_continuous_across_tiles_and_drops_missing() {
        let (a, b) = (flat(255, 30), flat(0, 0));
        let mut f = |tj: i64, ti: i64| match (tj, ti) {
            (15, 2) => Some(a.clone()),
            (15, 3) => Some(b.clone()),
            _ => None,
        };
        // Tile (15, 2) is lat 45..48, lon 6..9; (15, 3) lon 9..12. Across lon 9 cover ramps.
        let lat = 46.5;
        let (l, m, r) = (sample(&mut f, lat, 8.99, 0).unwrap(), sample(&mut f, lat, 9.0, 0).unwrap(), sample(&mut f, lat, 9.01, 0).unwrap());
        assert!(l[0] > m[0] && m[0] > r[0], "{l:?} {m:?} {r:?}");
        assert!((m[0] - 0.5).abs() < 0.02 && (m[1] - 30.0).abs() < 1e-3);
        // Missing neighbour: data fades, cover/height stay those of the present tile.
        let e = sample(&mut f, lat, 5.999, 0).unwrap();
        assert!(e[2] < 0.6 && (e[0] - 1.0).abs() < 1e-4, "{e:?}");
        assert!(sample(&mut f, 10.0, 10.0, 0).is_none());
        assert_eq!(mip_for_spacing(610.0), 0);
        assert_eq!(mip_for_spacing(2500.0), 1);
        assert_eq!(mip_for_spacing(1.0e6), MAX_MIP);
    }
}
