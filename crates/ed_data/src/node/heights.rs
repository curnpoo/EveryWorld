//! Height pages: 257 x 257 u16 samples per terrain node (P2-D3).
//!
//! Quantisation is a fixed rule, not a per-node fit: `scale` is always [`HEIGHT_STEP_M`] and
//! `offset` is the lowest quantised sample, so every node decodes the same real height to the
//! same f32 bits. That is what makes shared edges bit-identical *after decoding* even when two
//! neighbours have different ranges (a per-node fitted scale cannot do this).

use super::source::{Raster, SourceView};
use ed_geo::node::NodeId;
use ed_geo::{WGS84_A, WGS84_B, WGS84_E2};
use std::f64::consts::PI;
use std::sync::Arc;

pub const PAGE_CELLS: usize = 256;
/// Samples per page side.
pub const PAGE_N: usize = PAGE_CELLS + 1;
pub const PAGE_SAMPLES: usize = PAGE_N * PAGE_N;
/// Quantisation step in metres: 65535 steps cover 32.7 km, more than sea floor to summit.
pub const HEIGHT_STEP_M: f32 = 0.5;

/// `height(i, j) = offset + samples[j * 257 + i] as f32 * scale`; `i` runs along the node's x
/// (lattice `grid_point(i, j, 256)`), `j` along its y.
#[derive(Debug, Clone, PartialEq)]
pub struct HeightPage {
    pub samples: Vec<u16>,
    pub scale: f32,
    pub offset: f32,
    pub min_m: f32,
    pub max_m: f32,
}

impl HeightPage {
    /// Decoded height at lattice index `(i, j)`; exact (multiples of 0.5 m below 2^22).
    #[inline]
    pub fn height(&self, i: usize, j: usize) -> f32 {
        self.offset + self.samples[j * PAGE_N + i] as f32 * self.scale
    }

    /// Canonical byte form (little endian) for hashing and byte-identity tests.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(self.samples.len() * 2 + 16);
        for s in &self.samples {
            b.extend_from_slice(&s.to_le_bytes());
        }
        for f in [self.scale, self.offset, self.min_m, self.max_m] {
            b.extend_from_slice(&f.to_bits().to_le_bytes());
        }
        b
    }

    /// Build from quantised heights in units of [`HEIGHT_STEP_M`] (global grid indices).
    fn from_quantised(q: &[i64]) -> HeightPage {
        debug_assert_eq!(q.len(), PAGE_SAMPLES);
        let lo = *q.iter().min().unwrap();
        let hi = *q.iter().max().unwrap();
        let samples = q.iter().map(|&v| (v - lo).min(u16::MAX as i64) as u16).collect();
        let step = HEIGHT_STEP_M as f64;
        let offset = (lo as f64 * step) as f32;
        HeightPage { samples, scale: HEIGHT_STEP_M, offset, min_m: offset, max_m: (hi as f64 * step) as f32 }
    }

    fn quantised(&self) -> Vec<i64> {
        let base = (self.offset as f64 / HEIGHT_STEP_M as f64).round() as i64;
        self.samples.iter().map(|&s| base + s as i64).collect()
    }
}

fn quantise(h: f64) -> i64 {
    (h / HEIGHT_STEP_M as f64).round() as i64
}

/// Slippy zoom used for a node level: pixel spacing of a 256 px tile at the equator is at most
/// the node's cell spacing (`2^z >= 4 * 2^level`, i.e. `z = level + 2`), capped at the finest
/// source zoom. A pure function of the level, so neighbours always agree.
pub fn source_zoom(level: u8) -> u8 {
    (level + 2).min(crate::elevation::MAX_ZOOM)
}

/// Fractional global pixel coordinates (pixel centres at +0.5) for lon/lat at zoom `z`.
/// Latitude is clamped to the Mercator limit: beyond ~85.05 degrees the top/bottom row is
/// extended (polar caps are flat in longitude-varying data; see the W2a report).
fn lonlat_to_pixel(lon: f64, lat: f64, z: u8, size: usize) -> (f64, f64) {
    let n = (1u64 << z) as f64;
    let lat = lat.clamp(-ed_geo::tile::MAX_LAT, ed_geo::tile::MAX_LAT).to_radians();
    let x = (lon + 180.0) / 360.0 * n;
    let y = (1.0 - libm::log(libm::tan(lat) + 1.0 / libm::cos(lat)) / PI) / 2.0 * n;
    (x * size as f64, y * size as f64)
}

type CachedTile = ((u32, u32), Option<Arc<Raster>>);

/// Per-call tile cache (linear scan; no hash iteration, order is insertion order).
struct Tiles<'a> {
    src: &'a dyn SourceView,
    z: u8,
    cache: Vec<CachedTile>,
}

impl Tiles<'_> {
    fn get(&mut self, x: u32, y: u32) -> Option<Arc<Raster>> {
        if let Some((_, t)) = self.cache.iter().find(|(k, _)| *k == (x, y)) {
            return t.clone();
        }
        let t = self.src.elevation_tile(self.z, x, y);
        self.cache.push(((x, y), t.clone()));
        t
    }

    /// Height at integer global pixel `(px, py)`; longitude wraps, latitude clamps.
    fn pixel(&mut self, px: i64, py: i64, size: usize) -> f64 {
        let n = 1i64 << self.z;
        let w = n * size as i64;
        let px = px.rem_euclid(w);
        let py = py.clamp(0, w - 1);
        let (tx, ty) = ((px / size as i64) as u32, (py / size as i64) as u32);
        match self.get(tx, ty) {
            Some(t) if t.size == size => t.heights[(py % size as i64) as usize * size + (px % size as i64) as usize] as f64,
            Some(t) => {
                // Foreign tile size: nearest-neighbour remap into this call's pixel grid.
                let (sx, sy) = ((px % size as i64) as usize * t.size / size, (py % size as i64) as usize * t.size / size);
                t.heights[sy * t.size + sx] as f64
            }
            None => 0.0,
        }
    }
}

/// Heights for `id`'s 257 x 257 lattice, sampled bilinearly in tile pixel space.
///
/// Sample positions are `NodeId::grid_point(i, j, 256)` (f64, shared bit for bit with
/// neighbours), turned into geodetic lat/lon on the WGS84 ellipsoid (unit vector scaled by the
/// radii, P0-R1). Pure function of `(id, source)`.
pub fn node_heights(src: &dyn SourceView, id: NodeId) -> HeightPage {
    let z = source_zoom(id.level());
    let mut tiles = Tiles { src, z, cache: Vec::new() };
    // Tile size is learned from the first available tile; 256 if there is none.
    let size = {
        let g = lattice_geodetic(id, 0, 0);
        let (px, py) = lonlat_to_pixel(g.lon_deg, g.lat_deg, z, 1);
        let n = 1i64 << z;
        match tiles.get((px.floor() as i64).rem_euclid(n) as u32, (py.floor() as i64).clamp(0, n - 1) as u32) {
            Some(t) => t.size,
            None => 256,
        }
    };
    let mut q = Vec::with_capacity(PAGE_SAMPLES);
    for j in 0..PAGE_N {
        for i in 0..PAGE_N {
            let g = lattice_geodetic(id, i, j);
            let (gx, gy) = lonlat_to_pixel(g.lon_deg, g.lat_deg, z, size);
            let (fx, fy) = (gx - 0.5, gy - 0.5);
            let (x0, y0) = (fx.floor(), fy.floor());
            let (tx, ty) = (fx - x0, fy - y0);
            let (x0, y0) = (x0 as i64, y0 as i64);
            let a = tiles.pixel(x0, y0, size) * (1.0 - tx) + tiles.pixel(x0 + 1, y0, size) * tx;
            let b = tiles.pixel(x0, y0 + 1, size) * (1.0 - tx) + tiles.pixel(x0 + 1, y0 + 1, size) * tx;
            q.push(quantise(a * (1.0 - ty) + b * ty));
        }
    }
    HeightPage::from_quantised(&q)
}

/// Geodetic lat/lon of a lattice point: unit vector scaled by the WGS84 radii (P0-R1).
fn lattice_geodetic(id: NodeId, i: usize, j: usize) -> LatLon {
    let p = id.grid_point(i as u32, j as u32, PAGE_CELLS as u32);
    ecef_to_latlon(p.x * WGS84_A, p.y * WGS84_A, p.z * WGS84_B)
}

/// Geodetic latitude and longitude in degrees.
pub(crate) struct LatLon {
    pub lat_deg: f64,
    pub lon_deg: f64,
}

/// `ed_geo::Geodetic::from_ecef` (Bowring) with libm transcendentals, so the result is the
/// same bits on every platform. Altitude is not needed.
pub(crate) fn ecef_to_latlon(x: f64, y: f64, z: f64) -> LatLon {
    let lon = libm::atan2(y, x);
    let r = (x * x + y * y).sqrt();
    if r < 1e-6 {
        return LatLon { lat_deg: if z >= 0.0 { 90.0 } else { -90.0 }, lon_deg: 0.0 };
    }
    let ep2 = (WGS84_A * WGS84_A - WGS84_B * WGS84_B) / (WGS84_B * WGS84_B);
    let theta = libm::atan2(z * WGS84_A, r * WGS84_B);
    let (st, ct) = libm::sincos(theta);
    let lat = libm::atan2(z + ep2 * WGS84_B * st * st * st, r - WGS84_E2 * WGS84_A * ct * ct * ct);
    LatLon { lat_deg: lat.to_degrees(), lon_deg: lon.to_degrees() }
}

/// Parent page from its four children in `NodeId::children` order ((0,0) (1,0) (0,1) (1,1)).
///
/// Interior samples are a separable [1 2 1] / 4 filter of the combined 513 x 513 child grid
/// centred on the parent lattice point, snapped to the global quantisation grid. Samples on the
/// page border are **copied** from the children (index `2i`), never filtered, so parent borders
/// match the children and neighbouring parents bit for bit.
pub fn parent_heights(children: [&HeightPage; 4]) -> HeightPage {
    const W: usize = 2 * PAGE_CELLS + 1;
    let mut g = vec![0i64; W * W];
    for (c, page) in children.iter().enumerate() {
        let (ox, oy) = ((c & 1) * PAGE_CELLS, (c >> 1) * PAGE_CELLS);
        let q = page.quantised();
        for j in 0..PAGE_N {
            for i in 0..PAGE_N {
                // Shared child borders are identical by G4; later children overwrite equal values.
                g[(oy + j) * W + ox + i] = q[j * PAGE_N + i];
            }
        }
    }
    let at = |x: usize, y: usize| g[y * W + x] as f64;
    let mut q = Vec::with_capacity(PAGE_SAMPLES);
    for j in 0..PAGE_N {
        for i in 0..PAGE_N {
            let (x, y) = (2 * i, 2 * j);
            if i == 0 || j == 0 || i == PAGE_CELLS || j == PAGE_CELLS {
                q.push(g[y * W + x]);
                continue;
            }
            let row = |yy: usize| at(x - 1, yy) + 2.0 * at(x, yy) + at(x + 1, yy);
            let v = (row(y - 1) + 2.0 * row(y) + row(y + 1)) / 16.0;
            q.push(v.round() as i64);
        }
    }
    HeightPage::from_quantised(&q)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed_geo::Geodetic;

    #[test]
    fn libm_geodetic_agrees_with_ed_geo() {
        let mut s = 0x1234_5678_9abc_def0u64;
        let mut next = || {
            s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = s;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            (z ^ (z >> 31)) as f64 / u64::MAX as f64
        };
        for _ in 0..20_000 {
            let (lat, lon) = (next() * 180.0 - 90.0, next() * 360.0 - 180.0);
            let p = Geodetic::new(lat, lon, 0.0).to_ecef();
            let (a, b) = (Geodetic::from_ecef(p), ecef_to_latlon(p.x, p.y, p.z));
            assert!((a.lat_deg - b.lat_deg).abs() < 1e-9, "lat {lat}");
            assert!((a.lon_deg - b.lon_deg).abs() < 1e-9, "lon {lon}");
            // 1e-9 deg is ~1e-4 m on the ground; metre-scale agreement check on the surface point.
            let q = Geodetic::new(b.lat_deg, b.lon_deg, 0.0).to_ecef();
            assert!((q - p).length() < 1e-6 * 1e3, "surface");
        }
    }
}
