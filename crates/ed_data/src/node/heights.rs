//! Height pages: 257 x 257 u16 samples per terrain node (P2-D3).
//!
//! Quantisation is a fixed rule, not a per-node fit: `scale` is always `height_step_for_level` and
//! `offset` is the lowest quantised sample, so every node decodes the same real height to the
//! same f32 bits. That is what makes shared edges bit-identical *after decoding* even when two
//! neighbours have different ranges (a per-node fitted scale cannot do this).

use super::source::{Raster, SourceView};
use ed_geo::node::NodeId;
use ed_geo::{WGS84_A, WGS84_B, WGS84_E2};
use std::f64::consts::PI;
use std::sync::Arc;

pub use ed_geo::{HEIGHT_STEP_FINE_M, HEIGHT_STEP_M, HeightPage, height_step_for_level, PAGE_CELLS, PAGE_N, PAGE_SAMPLES};

/// Build from quantised heights in units of `step` (global grid indices).
fn page_from_quantised(q: &[i64], step: f32) -> HeightPage {
    debug_assert_eq!(q.len(), PAGE_SAMPLES);
    let lo = *q.iter().min().unwrap();
    let hi = (*q.iter().max().unwrap()).min(lo + u16::MAX as i64);
    let samples = q.iter().map(|&v| (v - lo).min(u16::MAX as i64) as u16).collect();
    let s = step as f64;
    let offset = (lo as f64 * s) as f32;
    HeightPage { samples, scale: step, offset, min_m: offset, max_m: (hi as f64 * s) as f32 }
}

/// A page's heights in metres.
fn page_metres(p: &HeightPage) -> Vec<f64> {
    p.samples.iter().map(|&s| p.decode(s)).collect()
}

fn quantise(h: f64, step: f32) -> i64 {
    (h / step as f64).round() as i64
}

/// Cubic weights for the four taps around a cell at fraction `t` (Catmull-Rom: C1, passes
/// through the source samples, so real ridges and cliffs keep their source sharpness).
#[inline]
pub fn cubic_weights(t: f64) -> [f64; 4] {
    let (t2, t3) = (t * t, t * t * t);
    [
        0.5 * (-t3 + 2.0 * t2 - t),
        0.5 * (3.0 * t3 - 5.0 * t2 + 2.0),
        0.5 * (-3.0 * t3 + 4.0 * t2 + t),
        0.5 * (t3 - t2),
    ]
}

/// Derivatives of [`cubic_weights`] with respect to `t`.
#[inline]
pub fn cubic_dweights(t: f64) -> [f64; 4] {
    let t2 = t * t;
    [0.5 * (-3.0 * t2 + 4.0 * t - 1.0), 0.5 * (9.0 * t2 - 10.0 * t), 0.5 * (-9.0 * t2 + 8.0 * t + 1.0), 0.5 * (3.0 * t2 - 2.0 * t)]
}

/// Erosion detail on fine pages ([`super::detail`]); `EW_TERRAIN_NODETAIL=1` turns it off (A/B).
pub fn detail_enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("EW_TERRAIN_NODETAIL").is_none_or(|v| v == "0"))
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

/// Vertical quantisation of the source at zoom `z` (m): Mapterhorn stores z14 in 1/8 m steps
/// and halves the precision per zoom out (z13 1/4 m, z12 1/2 m: measured on the cached tiles,
/// see `source_steps_follow_the_zoom_rule`). Finer zooms are taken as 1/8 m: an upsampled z15
/// carries the z14 steps, and real finer data is only smoothed within its own 1/16 m.
pub fn source_step(z: u8) -> f64 {
    0.125 * (1u64 << (14 - z.min(14))) as f64
}

/// First node level whose pages are de-terraced ([`DEQUANT_PASSES`]): from here the page cell is
/// within ~2 source pixels, so the source's height steps show as terraces (on a 3 degree slope a
/// 1/8 m step every 2.4 m makes flats and risers several metres wide).
pub const DEQUANT_FROM_LEVEL: u8 = 12;
/// Smoothing passes of the de-terracing (each a 5-tap binomial, variance 1 px^2; then a clamp
/// back into the source sample's quantisation bin).
pub const DEQUANT_PASSES: usize = 12;
/// Window side above which a node skips de-terracing (only nodes reaching the Mercator clamp
/// near the poles get here).
const DEQUANT_MAX_SIDE: i64 = 4096;

/// Source pixels of a window, de-terraced: `DEQUANT_PASSES` rounds of a separable [1 4 6 4 1]/16
/// smoothing, each clamped to `raw +- step / 2`, so the result is a smooth surface that the source
/// would have quantised to exactly its own samples. Real relief (ridges, cliffs, breaks of slope)
/// moves by at most half a step (6 cm at z14); flats and risers of the staircase become the ramp.
///
/// Each output pixel depends only on raw pixels within `2 * DEQUANT_PASSES` of it, through the
/// same arithmetic in the same order, so any window that contains that neighbourhood gives the
/// same bits (shared node edges stay identical). The valid region shrinks by 2 per pass.
fn dequantise(raw: &[f64], w: usize, h: usize, step: f64) -> Vec<f64> {
    let half = 0.5 * step;
    let mut cur = raw.to_vec();
    let mut tmp = raw.to_vec();
    for p in 0..DEQUANT_PASSES {
        let m = 2 * (p + 1);
        if 2 * m >= w || 2 * m >= h {
            break;
        }
        // Horizontal pass on rows still needed (valid margin m - 2 vertically).
        for y in (m - 2)..(h - m + 2) {
            let r = y * w;
            for x in m..(w - m) {
                let c = &cur[r + x - 2..r + x + 3];
                tmp[r + x] = (c[0] + 4.0 * c[1] + 6.0 * c[2] + 4.0 * c[3] + c[4]) * (1.0 / 16.0);
            }
        }
        for y in m..(h - m) {
            for x in m..(w - m) {
                let at = |yy: usize| tmp[yy * w + x];
                let v = (at(y - 2) + 4.0 * at(y - 1) + 6.0 * at(y) + 4.0 * at(y + 1) + at(y + 2)) * (1.0 / 16.0);
                let r = raw[y * w + x];
                cur[y * w + x] = v.clamp(r - half, r + half);
            }
        }
    }
    cur
}

/// Heights for `id`'s 257 x 257 lattice, sampled Catmull-Rom in tile pixel space from the
/// de-terraced source ([`dequantise`], from [`DEQUANT_FROM_LEVEL`]).
///
/// Sample positions are `NodeId::grid_point(i, j, 256)` (f64, shared bit for bit with
/// neighbours), turned into geodetic lat/lon of the ellipsoid point `d * r(d)` (P0-R1). Pure function of `(id, source)`.
pub fn node_heights(src: &dyn SourceView, id: NodeId) -> HeightPage {
    let z = source_zoom(id.level());
    let step = height_step_for_level(id.level());
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
    // Lattice points in fractional pixel coordinates (centres at integers), x unwrapped around
    // the first point so a node across the antimeridian has one compact window.
    let world = ((1i64 << z) * size as i64) as f64;
    let mut pts: Vec<(f64, f64)> = Vec::with_capacity(PAGE_SAMPLES);
    let detail_on = id.level() >= super::detail::first_detail_level() && detail_enabled();
    // Per lattice point, for the erosion detail: ellipsoid point (ECEF m), lat, lon (rad).
    let mut geo: Vec<([f64; 3], f64, f64)> = Vec::with_capacity(if detail_on { PAGE_SAMPLES } else { 0 });
    let mut clamped = false;
    for j in 0..PAGE_N {
        for i in 0..PAGE_N {
            let d = id.grid_point(i as u32, j as u32, PAGE_CELLS as u32);
            let r = ellipsoid_radius(d.x, d.y, d.z);
            let g = ecef_to_latlon(d.x * r, d.y * r, d.z * r);
            if detail_on {
                geo.push(([d.x * r, d.y * r, d.z * r], g.lat_deg.to_radians(), g.lon_deg.to_radians()));
            }
            clamped |= g.lat_deg.abs() > ed_geo::tile::MAX_LAT;
            let (gx, gy) = lonlat_to_pixel(g.lon_deg, g.lat_deg, z, size);
            let mut fx = gx - 0.5;
            if let Some(&(x0, _)) = pts.first() {
                fx -= world * ((fx - x0) / world).round();
            }
            pts.push((fx, gy - 0.5));
        }
    }
    let (mut x_lo, mut x_hi, mut y_lo, mut y_hi) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
    for &(x, y) in &pts {
        (x_lo, x_hi, y_lo, y_hi) = (x_lo.min(x), x_hi.max(x), y_lo.min(y), y_hi.max(y));
    }
    let pad = 2 + 2 * DEQUANT_PASSES as i64;
    let (wx0, wy0) = (x_lo.floor() as i64 - pad, y_lo.floor() as i64 - pad);
    let (ww, wh) = ((x_hi.floor() as i64 + 1 + pad - wx0 + 1) as usize, (y_hi.floor() as i64 + 1 + pad - wy0 + 1) as usize);
    let smooth = id.level() >= DEQUANT_FROM_LEVEL && !clamped && (ww as i64) <= DEQUANT_MAX_SIDE && (wh as i64) <= DEQUANT_MAX_SIDE;
    let win: Option<Vec<f64>> = smooth.then(|| {
        let mut raw = Vec::with_capacity(ww * wh);
        for y in 0..wh {
            for x in 0..ww {
                raw.push(tiles.pixel(wx0 + x as i64, wy0 + y as i64, size));
            }
        }
        dequantise(&raw, ww, wh, source_step(z))
    });
    let mut q = Vec::with_capacity(PAGE_SAMPLES);
    for (s, &(fx, fy)) in pts.iter().enumerate() {
        let (x0, y0) = (fx.floor(), fy.floor());
        let (wx, wy) = (cubic_weights(fx - x0), cubic_weights(fy - y0));
        let (dwx, dwy) = (cubic_dweights(fx - x0), cubic_dweights(fy - y0));
        let (x0, y0) = (x0 as i64, y0 as i64);
        let (mut h, mut hx, mut hy) = (0.0, 0.0, 0.0);
        for m in 0..4 {
            let (mut row, mut drow) = (0.0, 0.0);
            for k in 0..4 {
                let (px, py) = (x0 - 1 + k as i64, y0 - 1 + m as i64);
                let v = match &win {
                    Some(win) => win[(py - wy0) as usize * ww + (px - wx0) as usize],
                    None => tiles.pixel(px, py, size),
                };
                row += wx[k] * v;
                drow += dwx[k] * v;
            }
            h += wy[m] * row;
            hx += wy[m] * drow;
            hy += dwy[m] * row;
        }
        if detail_on && !clamped {
            // Source gradient in metres per metre (Mercator pixels are square; y runs south).
            let (p, lat, lon) = geo[s];
            let pix_m = 2.0 * PI * WGS84_A * libm::cos(lat) / world;
            let grad = [hx / pix_m, -hy / pix_m];
            h += super::detail::erosion_detail(p, lat, lon, h, grad, id.level());
        }
        q.push(quantise(h, step));
    }
    page_from_quantised(&q, step)
}

/// Geodetic lat/lon of a lattice point: the ECEF point `d * r(d)` where `r(d)` is the WGS84
/// ellipsoid radius along the unit direction `d`. This is exactly where `ed_planet` places the
/// lattice sample (`Body::point`), so sampled terrain and rendered terrain agree.
fn lattice_geodetic(id: NodeId, i: usize, j: usize) -> LatLon {
    let d = id.grid_point(i as u32, j as u32, PAGE_CELLS as u32);
    direction_geodetic(d.x, d.y, d.z)
}

/// Geodetic lat/lon of the ellipsoid point along unit direction `(x, y, z)`.
pub(crate) fn direction_geodetic(x: f64, y: f64, z: f64) -> LatLon {
    let r = ellipsoid_radius(x, y, z);
    ecef_to_latlon(x * r, y * r, z * r)
}

/// Ellipsoid radius along unit direction `d`; same formula as `ed_planet::body::Body::radius`
/// (crates/ed_planet/src/body.rs), copied because ed_data must not depend on ed_planet.
pub(crate) fn ellipsoid_radius(x: f64, y: f64, z: f64) -> f64 {
    let (a2, b2) = (WGS84_A * WGS84_A, WGS84_B * WGS84_B);
    1.0 / ((x * x + y * y) / a2 + z * z / b2).sqrt()
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
    // The parent keeps the children's step (all four share a level, hence a step).
    let step = children[0].scale;
    let mut g = vec![0i64; W * W];
    for (c, page) in children.iter().enumerate() {
        let (ox, oy) = ((c & 1) * PAGE_CELLS, (c >> 1) * PAGE_CELLS);
        let q: Vec<i64> = page_metres(page).iter().map(|&m| quantise(m, step)).collect();
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
    page_from_quantised(&q, step)
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

    #[test]
    fn sampled_point_lies_on_lattice_ray() {
        let mut dirs: Vec<[f64; 3]> = vec![
            [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, -1.0],
        ];
        for lat in [0.0f64, 46.0, 80.0, -46.0, -80.0, 89.9, -89.9] {
            for lon in [0.0f64, 37.0, 170.0, -123.0] {
                let (la, lo) = (lat.to_radians(), lon.to_radians());
                dirs.push([la.cos() * lo.cos(), la.cos() * lo.sin(), la.sin()]);
            }
        }
        for d in dirs {
            let g = direction_geodetic(d[0], d[1], d[2]);
            let p = Geodetic::new(g.lat_deg, g.lon_deg, 0.0).to_ecef();
            let pl = (p.x * p.x + p.y * p.y + p.z * p.z).sqrt();
            let u = [p.x / pl, p.y / pl, p.z / pl];
            let dot = u[0] * d[0] + u[1] * d[1] + u[2] * d[2];
            let cr = [u[1] * d[2] - u[2] * d[1], u[2] * d[0] - u[0] * d[2], u[0] * d[1] - u[1] * d[0]];
            let ang = (cr[0] * cr[0] + cr[1] * cr[1] + cr[2] * cr[2]).sqrt().atan2(dot);
            assert!(ang < 1e-9, "dir {d:?} angle {ang}");
        }
    }

    #[test]
    fn dequantise_turns_a_staircase_into_its_ramp_and_keeps_a_ridge() {
        let (w, h, q) = (120usize, 40usize, 0.125);
        // A 2 % ramp at 3.3 m pixels quantised to 1/8 m (flats ~2 px wide), plus a sharp ridge.
        let truth: Vec<f64> = (0..w * h).map(|k| 0.02 * 3.3 * (k % w) as f64 + if k % w == 90 { 6.0 } else { 0.0 }).collect();
        let raw: Vec<f64> = truth.iter().map(|&t| (t / q).round() * q).collect();
        let out = dequantise(&raw, w, h, q);
        let m = 2 + 2 * DEQUANT_PASSES;
        let (mut ramp_err, mut slope_dev) = (0.0f64, 0.0f64);
        for y in m..h - m {
            for x in m..80 {
                let k = y * w + x;
                ramp_err = ramp_err.max((out[k] - truth[k]).abs());
                slope_dev = slope_dev.max((out[k + 1] - out[k] - 0.066).abs());
            }
            // Every output stays inside its source sample's bin: the ridge keeps its height.
            for x in m..w - m {
                assert!((out[y * w + x] - raw[y * w + x]).abs() <= 0.5 * q + 1e-12);
            }
        }
        let raw_dev = (0..79).map(|x| ((raw[x + 1] - raw[x]) - 0.066f64).abs()).fold(0.0, f64::max);
        assert!(ramp_err < 0.03, "ramp error {ramp_err}");
        assert!(slope_dev < 0.25 * raw_dev, "per-pixel slope deviation {slope_dev} vs staircase {raw_dev}");
    }

    #[test]
    fn source_steps_follow_the_zoom_rule() {
        assert_eq!([source_step(12), source_step(13), source_step(14), source_step(15)], [0.5, 0.25, 0.125, 0.125]);
    }

    #[test]
    fn geodetic_46n_round_trips() {
        let p = Geodetic::new(46.0, 7.0, 0.0).to_ecef();
        let g = ecef_to_latlon(p.x, p.y, p.z);
        assert!((g.lat_deg - 46.0).abs() < 1e-9 && (g.lon_deg - 7.0).abs() < 1e-9);
    }
}
