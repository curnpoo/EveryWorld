//! Coarse ground layer (grass plan G2): the ground-page twin of `canopy_coarse`. Per 3 deg
//! source tile a baked raster (same `COARSE_N` grid, same mip chain, same bilinear read) of four
//! land-cover **shares**: herbaceous, cropland, shrub/heath, wet. Built only from the WorldCover
//! overview (`worldcover::sample_area`), cached as `ground_coarse/v1/N45E006.bin`. Its own file
//! and key, so the canopy layer (and its slow ETH sweep) is never rebuilt. Deterministic.

use std::collections::HashMap;
use std::io::Read;
use std::sync::{Arc, Mutex, OnceLock};

use super::canopy_coarse::{COARSE_CELL_DEG, COARSE_N, MAX_MIP};
use crate::cache::DiskCache;
use crate::worldcover;

const VERSION: u8 = 1;
const PLANES: usize = 4;

/// One 3 deg tile, rows from the north edge; planes are shares 0..=255 of
/// `[herbaceous, cropland, shrub, wet]`.
pub struct GroundCoarseTile {
    /// `mips[m]` is a `(N >> m)` square; each plane `n * n` bytes.
    mips: Vec<[Vec<u8>; PLANES]>,
}

impl GroundCoarseTile {
    pub(crate) fn from_planes(base: [Vec<u8>; PLANES]) -> GroundCoarseTile {
        let mut mips = vec![base];
        for m in 1..=MAX_MIP {
            let n = COARSE_N >> m;
            let sn = n * 2;
            let src = &mips[m - 1];
            let mut out: [Vec<u8>; PLANES] = std::array::from_fn(|_| vec![0u8; n * n]);
            for j in 0..n {
                for i in 0..n {
                    for p in 0..PLANES {
                        let mut s = 0u32;
                        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                            s += src[p][(2 * j + dy) * sn + 2 * i + dx] as u32;
                        }
                        out[p][j * n + i] = ((s + 2) / 4) as u8;
                    }
                }
            }
            mips.push(out);
        }
        GroundCoarseTile { mips }
    }

    /// Cells per side at mip `m`.
    pub fn n(m: usize) -> usize {
        COARSE_N >> m
    }

    /// Shares of cell `(i, j)` (row from the north) at mip `m`.
    #[inline]
    pub fn cell(&self, m: usize, i: usize, j: usize) -> [u8; PLANES] {
        let k = j * Self::n(m) + i;
        let p = &self.mips[m];
        [p[0][k], p[1][k], p[2][k], p[3][k]]
    }

    /// Cached form: `[VERSION, N lo, N hi]` then the four base planes, deflated.
    pub fn to_bytes(&self) -> Vec<u8> {
        use flate2::{write::ZlibEncoder, Compression};
        use std::io::Write;
        let mut raw = Vec::with_capacity(PLANES * COARSE_N * COARSE_N);
        for p in &self.mips[0] {
            raw.extend_from_slice(p);
        }
        let mut e = ZlibEncoder::new(vec![VERSION, (COARSE_N & 255) as u8, (COARSE_N >> 8) as u8], Compression::best());
        e.write_all(&raw).expect("in-memory write");
        e.finish().expect("in-memory finish")
    }

    pub fn from_bytes(b: &[u8]) -> Option<GroundCoarseTile> {
        if b.len() < 3 || b[0] != VERSION || b[1] as usize | (b[2] as usize) << 8 != COARSE_N {
            return None;
        }
        let mut raw = Vec::new();
        flate2::read::ZlibDecoder::new(&b[3..]).read_to_end(&mut raw).ok()?;
        let pn = COARSE_N * COARSE_N;
        if raw.len() != PLANES * pn {
            return None;
        }
        Some(GroundCoarseTile::from_planes(std::array::from_fn(|p| raw[p * pn..(p + 1) * pn].to_vec())))
    }

    /// Builds the tile with south-west corner `(sw_lat, sw_lon)` from the WorldCover overview
    /// (4 samples per cell). `Err` = unreadable (offline): nothing is cached. Open ocean is an
    /// all-zero tile.
    pub fn build(sw_lat: f64, sw_lon: f64, cache: &DiskCache) -> anyhow::Result<GroundCoarseTile> {
        let (s, w, n, e) = (sw_lat, sw_lon, sw_lat + 3.0, sw_lon + 3.0);
        let pn = COARSE_N * COARSE_N;
        let mut planes: [Vec<u8>; PLANES] = std::array::from_fn(|_| vec![0u8; pn]);
        if worldcover::tile_exists(sw_lat + 1.5, sw_lon + 1.5, cache)? {
            // Coarse probe first: an unreachable source fails before the long pass.
            worldcover::sample_area(s, w, n, e, 32, 32, 1, cache)?;
            let lc = worldcover::sample_area(s, w, n, e, COARSE_N, COARSE_N, 2, cache)?;
            planes[0].copy_from_slice(&lc.grass);
            planes[1].copy_from_slice(&lc.crop);
            planes[2].copy_from_slice(&lc.shrub);
            planes[3].copy_from_slice(&lc.wet);
        }
        Ok(GroundCoarseTile::from_planes(planes))
    }

    /// Reads the cached tile or builds (and caches) it.
    pub fn load(sw_lat: f64, sw_lon: f64, cache: &DiskCache) -> anyhow::Result<GroundCoarseTile> {
        let key = format!("ground_coarse/v{VERSION}/{}.bin", worldcover::tile_name(sw_lat + 0.5, sw_lon + 0.5));
        if let Some(t) = cache.get(&key, None).and_then(|b| GroundCoarseTile::from_bytes(&b)) {
            return Ok(t);
        }
        let t = GroundCoarseTile::build(sw_lat, sw_lon, cache)?;
        let _ = cache.put(&key, &t.to_bytes());
        Ok(t)
    }
}

type Slot = Arc<OnceLock<Option<Arc<GroundCoarseTile>>>>;

/// The tile with indices `(tj, ti)` (south-west corner `3 tj`, `3 ti` degrees), shared by every
/// thread; a failed read is remembered for the process (`None`).
pub fn shared_tile(tj: i64, ti: i64, cache: &DiskCache) -> Option<Arc<GroundCoarseTile>> {
    static T: OnceLock<Mutex<HashMap<(i64, i64), Slot>>> = OnceLock::new();
    let slot = T.get_or_init(Default::default).lock().unwrap().entry((tj, ti)).or_default().clone();
    slot.get_or_init(|| match GroundCoarseTile::load(tj as f64 * 3.0, ti as f64 * 3.0, cache) {
        Ok(t) => Some(Arc::new(t)),
        Err(e) => {
            log::warn!("ground_coarse: tile ({}, {}): {e}", tj * 3, ti * 3);
            None
        }
    })
    .clone()
}

/// Bilinear shares `0..=1` of `[herb, crop, shrub, wet]` at `(lat, lon)` and mip `m` over the
/// global coarse grid (same taps as `canopy_coarse::sample`). Taps on missing tiles drop out and
/// the rest renormalise; `None` when no tap exists.
pub fn sample(tile: &mut dyn FnMut(i64, i64) -> Option<Arc<GroundCoarseTile>>, lat: f64, lon: f64, m: usize) -> Option<[f32; PLANES]> {
    let n = GroundCoarseTile::n(m) as i64;
    let cs = COARSE_CELL_DEG * (1u64 << m) as f64;
    let (fx, fy) = (lon / cs - 0.5, lat / cs - 0.5);
    let (gx, gy) = (fx.floor(), fy.floor());
    let (tx, ty) = ((fx - gx) as f32, (fy - gy) as f32);
    let (gx, gy) = (gx as i64, gy as i64);
    let (mut acc, mut wsum) = ([0.0f32; PLANES], 0.0f32);
    for k in 0..4 {
        let (dx, dy) = ((k & 1) as i64, (k >> 1) as i64);
        let w = (if dx == 1 { tx } else { 1.0 - tx }) * (if dy == 1 { ty } else { 1.0 - ty });
        let (cx, cy) = (gx + dx, gy + dy);
        let Some(t) = tile(cy.div_euclid(n), cx.div_euclid(n)) else { continue };
        let c = t.cell(m, cx.rem_euclid(n) as usize, (n - 1 - cy.rem_euclid(n)) as usize);
        for p in 0..PLANES {
            acc[p] += w * c[p] as f32 / 255.0;
        }
        wsum += w;
    }
    if wsum <= 0.0 {
        return None;
    }
    Some(acc.map(|a| a / wsum))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_round_trip_and_mips() {
        let pn = COARSE_N * COARSE_N;
        let a: Vec<u8> = (0..pn).map(|k| (k * 7 % 251) as u8).collect();
        let t = GroundCoarseTile::from_planes([a, vec![10; pn], vec![0; pn], vec![255; pn]]);
        let u = GroundCoarseTile::from_bytes(&t.to_bytes()).unwrap();
        for m in 0..=MAX_MIP {
            assert_eq!(t.mips[m][0], u.mips[m][0]);
        }
        assert_eq!(u.cell(MAX_MIP, 0, 0), [u.cell(MAX_MIP, 0, 0)[0], 10, 0, 255]);
        assert!(GroundCoarseTile::from_bytes(&t.to_bytes()[1..]).is_none());
    }
}
