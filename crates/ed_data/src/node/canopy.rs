//! Canopy pages from source data (trees design §0, §1; gates G0/G1): `node_canopy` builds the
//! [`CanopyPage`] of any node straight from WorldCover and the ETH canopy height (F2: any rung
//! from source), `parent_canopy` is the area-mean parent.
//!
//! Sampling is on a **global lat/lon cell grid** (cell size `3 deg / 2^k`, `k` chosen from the
//! node level, so a cell is about one lattice spacing, never finer than the 10 m source). A
//! sample takes the value of the cell that contains its lat/lon, and a cell's value depends
//! only on the cell, so neighbouring nodes (and any rung asking) agree on shared samples.
//! Antimeridian and polar nodes are out of scope until the generator needs them.

use super::heights::direction_geodetic;
use crate::cache::DiskCache;
use crate::canopy_height::{self, CanopyHeight};
use crate::worldcover::{self, LandCover};
use ed_geo::canopy_page::{decode_height, encode_height, CanopyPage};
use ed_geo::node::NodeId;
use ed_geo::{PAGE_CELLS, PAGE_N};

/// Where canopy inputs come from, per lat/lon window (the canopy twin of
/// [`SourceView`](super::SourceView)). Grids are `w` x `h`, row 0 = north edge, column 0 =
/// west edge; each cell averages `sub` x `sub` source samples. `None` = no data.
pub trait CanopySource {
    fn land_cover(&self, south: f64, west: f64, north: f64, east: f64, w: usize, h: usize, sub: usize) -> Option<LandCover>;
    fn canopy_height(&self, south: f64, west: f64, north: f64, east: f64, w: usize, h: usize, sub: usize) -> Option<CanopyHeight>;
}

/// The real source: WorldCover and ETH canopy height COG range reads through the disk cache
/// (network on a miss). A read error is logged and treated as "no data".
pub struct RealCanopySource<'a> {
    pub cache: &'a DiskCache,
}

impl CanopySource for RealCanopySource<'_> {
    fn land_cover(&self, s: f64, w: f64, n: f64, e: f64, cw: usize, ch: usize, sub: usize) -> Option<LandCover> {
        worldcover::sample_area(s, w, n, e, cw, ch, sub, self.cache).map_err(|e| log::warn!("canopy: worldcover: {e}")).ok()
    }
    fn canopy_height(&self, s: f64, w: f64, n: f64, e: f64, cw: usize, ch: usize, sub: usize) -> Option<CanopyHeight> {
        canopy_height::sample_area(s, w, n, e, cw, ch, sub, self.cache).map_err(|e| log::warn!("canopy: eth height: {e}")).ok()
    }
}

/// Offline source for tests: functions of the cell-centre lat/lon.
pub struct SyntheticCanopy {
    /// `(lat, lon) -> (WorldCover class, tree share 0..=255, shrub share 0..=255)`.
    pub cover: Box<dyn Fn(f64, f64) -> (u8, u8, u8) + Send + Sync>,
    /// `(lat, lon) -> ETH canopy height in metres`, `None` = no data.
    pub height: Box<dyn Fn(f64, f64) -> Option<f32> + Send + Sync>,
}

impl SyntheticCanopy {
    /// Trees everywhere, no height data.
    pub fn forest() -> Self {
        SyntheticCanopy { cover: Box::new(|_, _| (worldcover::TREE, 255, 0)), height: Box::new(|_, _| None) }
    }
}

fn centres(s: f64, w: f64, n: f64, e: f64, cw: usize, ch: usize) -> impl Iterator<Item = (usize, f64, f64)> {
    (0..cw * ch).map(move |k| {
        let (i, j) = (k % cw, k / cw);
        (k, n - (n - s) * (j as f64 + 0.5) / ch as f64, w + (e - w) * (i as f64 + 0.5) / cw as f64)
    })
}

impl CanopySource for SyntheticCanopy {
    fn land_cover(&self, s: f64, w: f64, n: f64, e: f64, cw: usize, ch: usize, _sub: usize) -> Option<LandCover> {
        let mut o = LandCover { w: cw, h: ch, class: vec![0; cw * ch], tree: vec![0; cw * ch], shrub: vec![0; cw * ch] };
        for (k, lat, lon) in centres(s, w, n, e, cw, ch) {
            (o.class[k], o.tree[k], o.shrub[k]) = (self.cover)(lat, lon);
        }
        Some(o)
    }
    fn canopy_height(&self, s: f64, w: f64, n: f64, e: f64, cw: usize, ch: usize, _sub: usize) -> Option<CanopyHeight> {
        let mut o = CanopyHeight { w: cw, h: ch, height: vec![0.0; cw * ch], data: vec![0; cw * ch] };
        for (k, lat, lon) in centres(s, w, n, e, cw, ch) {
            if let Some(m) = (self.height)(lat, lon) {
                (o.height[k], o.data[k]) = (m, 255);
            }
        }
        Some(o)
    }
}

// ---------------------------------------------------------------------------------------------
// Climate proxies (placeholders until the CHELSA extract; data tables, not branches)
// ---------------------------------------------------------------------------------------------

/// Piecewise-linear lookup in a sorted `(x, y)` table, clamped at the ends.
fn lut(t: &[(f32, f32)], x: f32) -> f32 {
    if x <= t[0].0 {
        return t[0].1;
    }
    for w in t.windows(2) {
        if x <= w[1].0 {
            return w[0].1 + (w[1].1 - w[0].1) * (x - w[0].0) / (w[1].0 - w[0].0);
        }
    }
    t[t.len() - 1].1
}

/// Treeline altitude (m) by absolute latitude. About 2200 m at 46 N (Alps), 1700 m at 60 N,
/// falling to sea level near the polar limit; high in the tropics. A smooth stand-in for the
/// CHELSA warm-season-temperature treeline, which replaces it (aspect and exposure shift it
/// then).
pub const TREELINE_M: [(f32, f32); 9] = [(0.0, 4100.0), (20.0, 4000.0), (30.0, 3700.0), (40.0, 3000.0), (46.0, 2200.0), (60.0, 1700.0), (66.0, 800.0), (72.0, 200.0), (90.0, 0.0)];
/// Width (m) of the krummholz band below the treeline where cover and height taper.
pub const TREELINE_BAND_M: f32 = 200.0;
/// Fallback canopy top (m) of a closed warm-lowland stand by absolute latitude: tall in the
/// tropics and temperate lowlands (25-35 m), shrinking toward the boreal edge.
pub const LOWLAND_HEIGHT_M: [(f32, f32); 6] = [(0.0, 35.0), (25.0, 31.0), (40.0, 28.0), (50.0, 25.0), (60.0, 18.0), (70.0, 7.0)];
/// Height (m) of krummholz at the bottom of the treeline band's upper edge.
pub const KRUMMHOLZ_M: f32 = 3.0;
/// Conifer share by a warm-season temperature proxy `T = 30 - 0.25 |lat| - 0.0065 alt` (C):
/// Alps broadleaf below ~1000 m, mixed around there, conifer/larch toward the treeline.
pub const CONIFER_BY_T: [(f32, f32); 7] = [(5.0, 1.0), (8.0, 0.9), (10.0, 0.7), (12.0, 0.35), (14.0, 0.2), (16.0, 0.1), (25.0, 0.0)];
/// Extra conifer share toward the boreal belt: 0 at 50 N rising to this at 65 N and beyond.
pub const BOREAL_CONIFER: f32 = 0.6;

fn temp_proxy(lat: f64, alt: f64) -> f32 {
    (30.0 - 0.25 * lat.abs() - 0.0065 * alt) as f32
}

/// `splitmix64` of two integers and a salt: the village-tree hash (platform independent).
fn hash01(a: i64, b: i64, salt: u64) -> f32 {
    let mut z = (a as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (b as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ salt;
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 40) as f32 / (1u32 << 24) as f32
}

#[inline]
fn q8(x: f32) -> u8 {
    libm::roundf(x.clamp(0.0, 1.0) * 255.0) as u8
}

/// Cell-size exponent `k` (`cell = 3 deg / 2^k`) for `level`: about one lattice spacing, at
/// most 15 (~10 m, the source resolution).
fn cell_exp(level: u8) -> u32 {
    let spacing_m = 10_018_000.0 / (1u64 << level) as f64 / PAGE_CELLS as f64;
    ((333_585.0 / spacing_m).log2().floor().max(0.0) as u32).min(15)
}

/// Builds the canopy page of `id` from `src`. `alt(i, j)` is the terrain altitude (m) at lattice
/// sample `(i, j)`, normally `|i, j| heights.height(i, j)` from the node's `HeightPage`.
///
/// Rules (trees §1): cover is the WorldCover tree/mangrove share; built-up cells take a low
/// stochastic cover 0.05-0.15 (village trees) hashed from the global cell; height is the ETH
/// value where present, else allometry from latitude and altitude; cover and fallback height
/// taper to krummholz over [`TREELINE_BAND_M`] below the treeline (fallback path only: real
/// data already knows where trees stop); mix comes from the altitude/latitude temperature
/// proxy. Pure function of `(id, alt, src)`.
pub fn node_canopy(src: &dyn CanopySource, id: NodeId, alt: &dyn Fn(usize, usize) -> f64) -> CanopyPage {
    // Geodetic position of every lattice sample.
    let mut geo = Vec::with_capacity(PAGE_N * PAGE_N);
    for j in 0..PAGE_N {
        for i in 0..PAGE_N {
            let d = id.grid_point(i as u32, j as u32, PAGE_CELLS as u32);
            let g = direction_geodetic(d.x, d.y, d.z);
            geo.push((g.lat_deg, g.lon_deg));
        }
    }
    let mut k = cell_exp(id.level());
    let (mut cd, mut ci0, mut cj0, mut w, mut h);
    loop {
        cd = 3.0 / (1u64 << k) as f64;
        let (mut i_lo, mut i_hi, mut j_lo, mut j_hi) = (i64::MAX, i64::MIN, i64::MAX, i64::MIN);
        for &(lat, lon) in &geo {
            let (ci, cj) = ((lon / cd).floor() as i64, (lat / cd).floor() as i64);
            (i_lo, i_hi, j_lo, j_hi) = (i_lo.min(ci), i_hi.max(ci), j_lo.min(cj), j_hi.max(cj));
        }
        (ci0, cj0, w, h) = (i_lo, j_lo, (i_hi - i_lo + 1) as usize, (j_hi - j_lo + 1) as usize);
        if (w <= 1024 && h <= 1024) || k == 0 {
            break;
        }
        k -= 1;
    }
    let sub = if k >= 15 { 1 } else { 2 };
    let (south, west) = (cj0 as f64 * cd, ci0 as f64 * cd);
    let (north, east) = (south + h as f64 * cd, west + w as f64 * cd);
    let lc = src.land_cover(south, west, north, east, w, h, sub);
    let eth = src.canopy_height(south, west, north, east, w, h, sub);

    let mut out = Vec::with_capacity(PAGE_N * PAGE_N);
    for j in 0..PAGE_N {
        for i in 0..PAGE_N {
            let (lat, lon) = geo[j * PAGE_N + i];
            let (ci, cj) = ((lon / cd).floor() as i64, (lat / cd).floor() as i64);
            let (cx, cy) = ((ci - ci0) as usize, (cj - cj0) as usize);
            let idx = (h - 1 - cy) * w + cx; // row 0 = north
            let (class, tree, shrub) = match &lc {
                Some(l) => (l.class[idx], l.tree[idx] as f32 / 255.0, l.shrub[idx] as f32 / 255.0),
                None => (0, 0.0, 0.0),
            };
            let (eth_m, eth_ok) = match &eth {
                Some(e) if e.data[idx] > 127 => (e.height[idx], true),
                _ => (0.0, false),
            };
            let altm = alt(i, j);
            let abs_lat = lat.abs() as f32;

            let mut cover = tree;
            let mut under = shrub;
            under += match class {
                worldcover::GRASS | worldcover::MOSS => 0.35,
                worldcover::CROP => 0.2,
                worldcover::WETLAND => 0.4,
                _ => 0.0,
            };
            under += 0.3 * tree;
            let village = class == worldcover::BUILT;
            if village {
                cover = 0.05 + 0.10 * hash01(ci, cj, 0x7A11_A6E5);
                under += 0.15;
            }

            // Treeline band (fallback only).
            let tl = lut(&TREELINE_M, abs_lat);
            let s = ((tl - altm as f32) / TREELINE_BAND_M).clamp(0.0, 1.0);
            let s = s * s * (3.0 - 2.0 * s);
            let have_height = eth_ok && eth_m >= 1.0 && !village;
            if !have_height {
                cover *= s;
            }

            // Height.
            let height = if cover <= 0.0 {
                0.0
            } else if have_height {
                eth_m
            } else {
                let base = lut(&LOWLAND_HEIGHT_M, abs_lat) * (1.0 - 0.4 * (altm as f32 / tl.max(1.0)).clamp(0.0, 1.0));
                let hmax = KRUMMHOLZ_M + (base - KRUMMHOLZ_M) * s;
                if village { hmax * 0.4 } else { hmax }
            };

            // Species mix.
            let t = temp_proxy(lat, altm);
            let boreal = ((abs_lat - 50.0) / 15.0).clamp(0.0, 1.0) * BOREAL_CONIFER;
            let mix = (lut(&CONIFER_BY_T, t) + boreal).min(1.0);

            out.push([q8(cover), encode_height(height), if cover > 0.0 { q8(mix) } else { 0 }, q8(under)]);
        }
    }
    CanopyPage { samples: out }
}

/// Parent page from its four children in `NodeId::children` order ((0,0) (1,0) (0,1) (1,1)):
/// the same separable [1 2 1] / 4 area filter as `parent_heights` over the combined 513 x 513
/// child grid, border samples copied (so parent borders match the children and neighbouring
/// parents bit for bit). Cover and understory average directly; height and mix average
/// weighted by cover, so open ground does not dilute a stand's height.
pub fn parent_canopy(children: [&CanopyPage; 4]) -> CanopyPage {
    const W: usize = 2 * PAGE_CELLS + 1;
    let mut g = vec![[0u8; 4]; W * W];
    for (c, page) in children.iter().enumerate() {
        let (ox, oy) = ((c & 1) * PAGE_CELLS, (c >> 1) * PAGE_CELLS);
        for j in 0..PAGE_N {
            for i in 0..PAGE_N {
                g[(oy + j) * W + ox + i] = page.get(i, j);
            }
        }
    }
    let mut out = Vec::with_capacity(PAGE_N * PAGE_N);
    for j in 0..PAGE_N {
        for i in 0..PAGE_N {
            let (x, y) = (2 * i, 2 * j);
            if i == 0 || j == 0 || i == PAGE_CELLS || j == PAGE_CELLS {
                out.push(g[y * W + x]);
                continue;
            }
            // Integer weights 1 2 1 / 2 4 2 / 1 2 1 (sum 16).
            let (mut cov, mut und, mut cw, mut hs, mut ms) = (0u32, 0u32, 0u32, 0.0f32, 0.0f32);
            for (dy, wy) in [(-1i32, 1u32), (0, 2), (1, 1)] {
                for (dx, wx) in [(-1i32, 1u32), (0, 2), (1, 1)] {
                    let p = g[(y as i32 + dy) as usize * W + (x as i32 + dx) as usize];
                    let wt = wx * wy;
                    cov += wt * p[0] as u32;
                    und += wt * p[3] as u32;
                    let wc = wt * p[0] as u32;
                    cw += wc;
                    hs += wc as f32 * decode_height(p[1]);
                    ms += wc as f32 * p[2] as f32;
                }
            }
            let (h, m) = if cw > 0 { (hs / cw as f32, ms / cw as f32) } else { (0.0, 0.0) };
            out.push([((cov + 8) / 16) as u8, encode_height(h), libm::roundf(m) as u8, ((und + 8) / 16) as u8]);
        }
    }
    CanopyPage { samples: out }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed_geo::node::BodyId;

    fn node_at(lat: f64, lon: f64, level: u8) -> NodeId {
        let d = ed_geo::Geodetic::new(lat, lon, 0.0).to_ecef().normalize();
        NodeId::from_unit(BodyId(0), d, level)
    }

    /// Altitude as a smooth function of latitude: 600 m at 45.5 N rising 4000 m per degree.
    fn alt_of(id: NodeId) -> impl Fn(usize, usize) -> f64 {
        move |i, j| {
            let d = id.grid_point(i as u32, j as u32, PAGE_CELLS as u32);
            let g = direction_geodetic(d.x, d.y, d.z);
            600.0 + (g.lat_deg - 45.5) * 4000.0
        }
    }

    fn src() -> SyntheticCanopy {
        SyntheticCanopy {
            cover: Box::new(|lat, lon| {
                // Patchy forest and a village; deterministic in position.
                let v = hash01((lat * 1e4) as i64 / 3, (lon * 1e4) as i64 / 3, 1);
                if v < 0.1 { (worldcover::BUILT, 0, 0) } else if v < 0.3 { (worldcover::GRASS, 0, 0) } else { (worldcover::TREE, 230, 10) }
            }),
            height: Box::new(|lat, lon| if lat > 45.6 { None } else { Some(18.0 + 10.0 * ((lon * 300.0).sin() as f32)) }),
        }
    }

    #[test]
    fn deterministic_bit_identical() {
        let id = node_at(45.5, 7.5, 12);
        let a = node_canopy(&src(), id, &alt_of(id));
        let b = node_canopy(&src(), id, &alt_of(id));
        assert_eq!(a.to_bytes(), b.to_bytes());
        assert!(a.samples.iter().any(|s| s[0] > 100 && s[1] > 0), "page is not empty");
    }

    #[test]
    fn neighbours_agree_on_shared_border() {
        let id = node_at(45.5, 7.5, 12);
        let e = id.neighbour(1, 0);
        let s = id.neighbour(0, 1);
        let (pa, pe, ps) = (node_canopy(&src(), id, &alt_of(id)), node_canopy(&src(), e, &alt_of(e)), node_canopy(&src(), s, &alt_of(s)));
        for k in 0..PAGE_N {
            assert_eq!(pa.get(PAGE_CELLS, k), pe.get(0, k), "x edge at {k}");
            assert_eq!(pa.get(k, PAGE_CELLS), ps.get(k, 0), "y edge at {k}");
        }
    }

    #[test]
    fn cover_falls_above_treeline() {
        // Same node, altitude ramp across 1400..3200 m, no ETH data: treeline at 46 N is 2200.
        let id = node_at(46.0, 7.7, 12);
        let src = SyntheticCanopy::forest();
        let page = node_canopy(&src, id, &|i, _| 1400.0 + i as f64 * (1800.0 / 256.0));
        let col = |i: usize| (0..PAGE_N).map(|j| page.cover(i, j)).sum::<f32>() / PAGE_N as f32;
        assert!(col(10) > 0.7, "montane forest {}", col(10));
        let ti = ((2200.0 - 1400.0) / (1800.0 / 256.0)) as usize;
        assert!(col(ti - 40) > 0.3, "inside the band");
        assert!(col(ti + 5) < 0.02, "above the line {}", col(ti + 5));
        assert!(col(250) < 0.001);
        // Heights shrink toward krummholz inside the band; conifer share grows with altitude.
        assert!(page.height_m(10, 10) > 15.0, "lowland height {}", page.height_m(10, 10));
        let hb = page.height_m(ti - 5, 10);
        assert!(hb < 12.0 && hb >= KRUMMHOLZ_M * 0.9, "band height {hb}");
        assert!(page.mix(10, 10) < page.mix(ti - 40, 10) && page.mix(ti - 40, 10) > 0.8);
    }

    #[test]
    fn village_trees_are_sparse_and_eth_height_wins() {
        let id = node_at(45.5, 7.5, 12);
        let built = SyntheticCanopy { cover: Box::new(|_, _| (worldcover::BUILT, 0, 0)), height: Box::new(|_, _| None) };
        let p = node_canopy(&built, id, &|_, _| 600.0);
        let (lo, hi) = p.samples.iter().fold((255u8, 0u8), |(l, h), s| (l.min(s[0]), h.max(s[0])));
        assert!(lo >= q8(0.05) && hi <= q8(0.15), "{lo} {hi}");
        let tall = SyntheticCanopy { cover: Box::new(|_, _| (worldcover::TREE, 255, 0)), height: Box::new(|_, _| Some(31.0)) };
        let p = node_canopy(&tall, id, &|_, _| 600.0);
        assert!((p.height_m(100, 100) - 31.0).abs() < 0.5);
    }

    #[test]
    fn parent_is_area_mean() {
        // Linear cover/under ramps: the tent filter reproduces them; borders are copied.
        let ramp = |ox: usize, oy: usize| CanopyPage::from_fn(|i, j| [((ox + i) / 3) as u8, encode_height(20.0), 128, ((oy + j) / 3) as u8]);
        let kids = [ramp(0, 0), ramp(256, 0), ramp(0, 256), ramp(256, 256)];
        let p = parent_canopy([&kids[0], &kids[1], &kids[2], &kids[3]]);
        for &(i, j) in &[(0usize, 0usize), (1, 1), (100, 57), (128, 128), (200, 3), (256, 256), (255, 1)] {
            let want_c = (2 * i) as f64 / 3.0;
            let want_u = (2 * j) as f64 / 3.0;
            let s = p.get(i, j);
            assert!((s[0] as f64 - want_c).abs() <= 1.5, "cover {i},{j}: {} vs {want_c}", s[0]);
            assert!((s[3] as f64 - want_u).abs() <= 1.5, "under {i},{j}: {} vs {want_u}", s[3]);
            assert!((decode_height(s[1]) - 20.0).abs() < 0.5);
            if s[0] > 0 {
                assert!((s[2] as i32 - 128).abs() <= 1);
            }
        }
        // Border samples copied from the children.
        assert_eq!(p.get(256, 40), kids[1].get(256, 80));
        assert_eq!(p.get(40, 0), kids[0].get(80, 0));
    }

    #[test]
    #[ignore = "network"]
    fn real_zermatt_page() {
        let cache = DiskCache::default_location();
        let id = node_at(46.02, 7.756, 12);
        let page = node_canopy(&RealCanopySource { cache: &cache }, id, &|_, _| 1600.0);
        let n = page.samples.iter().filter(|s| s[0] > 128 && s[1] > 0).count();
        assert!(n > 2000, "treed samples {n}");
    }

    /// Trees gate G3 (the density strip, numerically): the forest share of one forested
    /// Zermatt patch (an L12 node, ~2.4 km) read from every rung's pages, L8 (what a 50 km view
    /// draws) to L14 (50 m), with the far rung's forest weight (resolve.slang `terrain_material`:
    /// a steep edge on fine pages, linear in cover on coarse ones). Each level stays within 10%
    /// of the finest.
    #[test]
    #[ignore = "network"]
    fn g3_density_strip_zermatt() {
        let cache = DiskCache::default_location();
        let patch = node_at(46.012, 7.765, 12);
        let src = RealCanopySource { cache: &cache };
        let ss = |e0: f32, e1: f32, x: f32| {
            let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        };
        const N: u32 = 48;
        let mut share = Vec::new();
        for level in 8..=14u8 {
            let mut pages: std::collections::BTreeMap<u64, CanopyPage> = std::collections::BTreeMap::new();
            let mut sum = 0.0f64;
            for j in 0..N {
                for i in 0..N {
                    let p = patch.grid_point(2 * i + 1, 2 * j + 1, 2 * N);
                    let id = NodeId::from_unit(BodyId(0), p, level);
                    let page = pages.entry(id.bits()).or_insert_with(|| node_canopy(&src, id, &|_, _| 1600.0));
                    let (u, v) = id.local_uv(p);
                    // Page spacing (m) at this level, and the shader's level-aware weight.
                    let sp = (id.grid_point(0, 0, PAGE_CELLS as u32) - id.grid_point(1, 0, PAGE_CELLS as u32)).length() * 6.371e6;
                    let c = page.bilinear(u, v)[0];
                    let k = ss(80.0, 20.0, sp as f32);
                    sum += (c + (ss(0.03, 0.5, c) - c) * k) as f64;
                }
            }
            share.push((level, sum / (N * N) as f64));
        }
        let fine = share.last().unwrap().1;
        eprintln!("G3 density strip (forest share by level): {share:?}");
        assert!(fine > 0.2, "patch is forested: {fine}");
        for &(level, s) in &share {
            assert!((s - fine).abs() <= 0.1 * fine, "L{level}: {s:.3} vs L14 {fine:.3}");
        }
    }
}
