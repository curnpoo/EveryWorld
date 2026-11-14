//! Ground pages (grass plan G2): per node, `PAGE_N` x `PAGE_N` samples of land-cover **shares**
//! `[herbaceous, cropland, shrub/heath, wet]` (0..=255 each; bare is what trees, these four,
//! snow and water leave). Built beside the canopy page (`canopy.rs`) with the same lattice,
//! the same global lat/lon cell grid (so neighbours agree on shared samples and any level can be
//! built straight from source) and the same two regimes: 10 m land-cover reads in the fine
//! regime, the `ground_coarse` layer from [`super::canopy_coarse::COARSE_MIN_SPACING_M`] up.
//! Shares, not class ids, so [`parent_ground`] averages correctly.

use super::canopy::{geo_of, page_raw, CanopySource, Raw};
use ed_geo::node::NodeId;
use ed_geo::{PAGE_CELLS, PAGE_N};

pub use ed_geo::GroundPage;

/// Quantised shares of a raw lattice read.
pub(super) fn ground_from_raw(raw: &[Raw]) -> GroundPage {
    let q = |x: f32| libm::roundf(x.clamp(0.0, 1.0) * 255.0) as u8;
    GroundPage { samples: raw.iter().map(|r| r.ground.map(q)).collect() }
}

/// Builds the ground page of `id` from `src`: a sample takes the shares of the global cell that
/// contains it (fine regime) or a bilinear read of the coarse layer (coarse regime). Pure
/// function of `(id, src)`. When the canopy page is wanted too, use
/// [`node_canopy_ground`](super::node_canopy_ground), which reads the sources once.
pub fn node_ground(src: &dyn CanopySource, id: NodeId) -> GroundPage {
    let geo = geo_of(id);
    let (raw, _) = page_raw(src, id, &geo, false, true);
    ground_from_raw(&raw)
}

/// Parent page from its four children in `NodeId::children` order: the separable [1 2 1] / 4
/// area filter over the combined 513 x 513 child grid (plain averages of shares), border samples
/// copied so parent borders match the children and neighbouring parents bit for bit.
pub fn parent_ground(children: [&GroundPage; 4]) -> GroundPage {
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
    GroundPage::from_fn(|i, j| {
        let (x, y) = (2 * i, 2 * j);
        if i == 0 || j == 0 || i == PAGE_CELLS || j == PAGE_CELLS {
            return g[y * W + x];
        }
        let mut acc = [0u32; 4];
        for (dy, wy) in [(-1i32, 1u32), (0, 2), (1, 1)] {
            for (dx, wx) in [(-1i32, 1u32), (0, 2), (1, 1)] {
                let p = g[(y as i32 + dy) as usize * W + (x as i32 + dx) as usize];
                for c in 0..4 {
                    acc[c] += wx * wy * p[c] as u32;
                }
            }
        }
        acc.map(|a| ((a + 8) / 16) as u8)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::canopy_coarse::{CoarseTile, COARSE_N};
    use crate::node::ground_coarse::GroundCoarseTile;
    use crate::node::{CanopySource, SyntheticCanopy};
    use crate::{canopy_height::CanopyHeight, worldcover};
    use ed_geo::node::BodyId;
    use std::sync::Arc;

    fn node_at(lat: f64, lon: f64, level: u8) -> NodeId {
        let d = ed_geo::Geodetic::new(lat, lon, 0.0).to_ecef().normalize();
        NodeId::from_unit(BodyId(0), d, level)
    }

    /// Splitmix64 stream for seeded positions.
    fn rng(seed: u64) -> impl FnMut() -> f64 {
        let mut s = seed;
        move || {
            s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = s;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
        }
    }

    /// A different class per 0.01 deg block (and per fine cell inside it for trees/shrub).
    fn src() -> SyntheticCanopy {
        SyntheticCanopy {
            cover: Box::new(|lat, lon| {
                let (a, b) = ((lat * 100.0).floor() as i64, (lon * 100.0).floor() as i64);
                let v = (a.wrapping_mul(31).wrapping_add(b.wrapping_mul(17))).rem_euclid(7);
                match v {
                    0 => (worldcover::GRASS, 0, 0),
                    1 => (worldcover::CROP, 0, 0),
                    2 => (worldcover::WETLAND, 40, 20),
                    3 => (worldcover::MOSS, 0, 100),
                    4 => (worldcover::SHRUB, 0, 200),
                    5 => (worldcover::TREE, 230, 10),
                    _ => (worldcover::GRASS, 60, 60),
                }
            }),
            height: Box::new(|_, _| None),
        }
    }

    /// Same bit-exact check on the fine regime, many nodes and levels.
    #[test]
    fn borders_agree_with_neighbours() {
        let mut r = rng(7);
        for level in [8u8, 9, 10, 11, 12, 13, 14] {
            for _ in 0..3 {
                let (lat, lon) = (r() * 120.0 - 60.0, r() * 300.0 - 150.0);
                let id = node_at(lat, lon, level);
                let (e, s) = (id.neighbour(1, 0), id.neighbour(0, 1));
                let (pa, pe, ps) = (node_ground(&src(), id), node_ground(&src(), e), node_ground(&src(), s));
                for k in 0..PAGE_N {
                    assert_eq!(pa.get(PAGE_CELLS, k), pe.get(0, k), "L{level} x edge at {k}");
                    assert_eq!(pa.get(k, PAGE_CELLS), ps.get(k, 0), "L{level} y edge at {k}");
                }
            }
        }
    }

    /// A parent built by [`parent_ground`] agrees with `node_ground` of that parent on its
    /// border samples. The source is constant over 0.5 deg blocks so a coarser cell grid sees the
    /// same values; nodes are chosen inside one block.
    #[test]
    fn parent_border_matches_direct() {
        let blocky = SyntheticCanopy {
            cover: Box::new(|lat, lon| match ((lat * 2.0).floor() as i64 * 5 + (lon * 2.0).floor() as i64 * 3).rem_euclid(4) {
                0 => (worldcover::GRASS, 0, 0),
                1 => (worldcover::CROP, 0, 0),
                2 => (worldcover::WETLAND, 30, 0),
                _ => (worldcover::SHRUB, 0, 255),
            }),
            height: Box::new(|_, _| None),
        };
        let mut r = rng(11);
        let mut tested = 0;
        for level in [9u8, 10, 11, 12] {
            for _ in 0..6 {
                let id = node_at(r() * 100.0 - 50.0, r() * 300.0 - 150.0, level);
                let parent = id.parent().unwrap();
                let pc = parent.children().unwrap();
                let kids: Vec<GroundPage> = pc.iter().map(|&c| node_ground(&blocky, c)).collect();
                let p = parent_ground([&kids[0], &kids[1], &kids[2], &kids[3]]);
                let d = node_ground(&blocky, parent);
                // Skip parents straddling a 0.5 deg block edge (corner lat/lon span).
                let uniform = d.samples.iter().all(|s| *s == d.samples[0]) || {
                    let g = geo_of(parent);
                    let blk = |(la, lo): (f64, f64)| ((la * 2.0).floor() as i64, (lo * 2.0).floor() as i64);
                    g.iter().all(|&x| blk(x) == blk(g[0]))
                };
                if !uniform {
                    continue;
                }
                tested += 1;
                for k in 0..PAGE_N {
                    for (i, j) in [(0, k), (PAGE_CELLS, k), (k, 0), (k, PAGE_CELLS)] {
                        assert_eq!(p.get(i, j), d.get(i, j), "L{level} border ({i},{j})");
                    }
                }
            }
        }
        assert!(tested >= 8, "only {tested} parents tested");
    }

    #[test]
    fn parent_is_area_mean() {
        let ramp = |ox: usize, oy: usize| GroundPage::from_fn(|i, j| [((ox + i) / 3) as u8, ((oy + j) / 3) as u8, 7, 0]);
        let k = [ramp(0, 0), ramp(256, 0), ramp(0, 256), ramp(256, 256)];
        let p = parent_ground([&k[0], &k[1], &k[2], &k[3]]);
        let s = p.get(100, 57);
        assert!((s[0] as f64 - 200.0 / 3.0).abs() <= 1.5 && (s[1] as f64 - 114.0 / 3.0).abs() <= 1.5 && s[2] == 7);
        assert_eq!(p.get(256, 40), k[1].get(256, 80));
    }

    #[test]
    fn shares_sum_within_255() {
        let mut r = rng(3);
        for level in [6u8, 8, 11, 13] {
            let id = node_at(r() * 100.0 - 50.0, r() * 300.0 - 150.0, level);
            let p = node_ground(&src(), id);
            let mut seen = [false; 4];
            for s in &p.samples {
                assert!(s.iter().map(|&v| v as u32).sum::<u32>() <= 255 + 2, "{s:?}");
                for c in 0..4 {
                    seen[c] |= s[c] > 0;
                }
            }
            if level >= 11 {
                assert!(seen.iter().filter(|&&b| b).count() >= 2, "L{level} {seen:?}");
            }
        }
    }

    /// A source with the coarse layers on: tiles are a ramp of grass and flat others.
    struct WithCoarse(SyntheticCanopy);
    impl CanopySource for WithCoarse {
        fn land_cover(&self, s: f64, w: f64, n: f64, e: f64, cw: usize, ch: usize, sub: usize) -> Option<crate::worldcover::LandCover> {
            self.0.land_cover(s, w, n, e, cw, ch, sub)
        }
        fn canopy_height(&self, s: f64, w: f64, n: f64, e: f64, cw: usize, ch: usize, sub: usize) -> Option<CanopyHeight> {
            self.0.canopy_height(s, w, n, e, cw, ch, sub)
        }
        fn has_coarse(&self) -> bool {
            true
        }
        fn coarse_tile(&self, _tj: i64, _ti: i64) -> Option<Arc<CoarseTile>> {
            let pn = COARSE_N * COARSE_N;
            Some(Arc::new(CoarseTile::from_planes([vec![100; pn], vec![10; pn], vec![255; pn]])))
        }
        fn ground_coarse_tile(&self, tj: i64, ti: i64) -> Option<Arc<GroundCoarseTile>> {
            if (tj + ti) % 5 == 0 {
                return None; // missing tiles drop out
            }
            let pn = COARSE_N * COARSE_N;
            let ramp: Vec<u8> = (0..pn).map(|k| (k % COARSE_N * 255 / COARSE_N) as u8).collect();
            Some(Arc::new(GroundCoarseTile::from_planes([ramp, vec![20; pn], vec![30; pn], vec![40; pn]])))
        }
    }

    #[test]
    fn regime_switch_and_coarse_borders() {
        let s = WithCoarse(src());
        let mut r = rng(5);
        for level in 0..=14u8 {
            let id = node_at(r() * 100.0 - 50.0, r() * 300.0 - 150.0, level);
            let p = node_ground(&s, id);
            assert_eq!(p.samples.len(), PAGE_N * PAGE_N);
            assert_eq!(p, node_ground(&s, id), "deterministic");
            let e = id.neighbour(1, 0);
            let pe = node_ground(&s, e);
            for k in 0..PAGE_N {
                assert_eq!(p.get(PAGE_CELLS, k), pe.get(0, k), "L{level} edge {k}");
            }
        }
        // Coarse regime reads the layer (level 3 is ~100 km spacing): flat planes show up.
        let id = node_at(46.0, 7.7, 3);
        let p = node_ground(&s, id);
        assert!(p.samples.iter().any(|s| s[1] == 20 && s[2] == 30 && s[3] == 40));
        // The combined build equals the two builds apart.
        let alt = |_: usize, _: usize| 500.0;
        for level in [3u8, 12] {
            let id = node_at(46.0, 7.7, level);
            let (c, g) = crate::node::node_canopy_ground(&s, id, &alt);
            assert_eq!(g, node_ground(&s, id));
            assert_eq!(c.to_bytes(), crate::node::node_canopy(&s, id, &alt).to_bytes());
        }
    }

    #[test]
    #[ignore = "network (WorldCover, cached for the PLACES)"]
    fn real_places_shares() {
        use crate::node::RealCanopySource;
        let cache = crate::DiskCache::default_location();
        let real = RealCanopySource { cache: &cache };
        let places: [(&str, f64, f64); 9] = [
            ("dunes", 31.15, -3.99),
            ("sonoran", 32.25, -111.17),
            ("jungle", 10.43, -84.74),
            ("everglades", 25.75, -80.85),
            ("tundra", 68.63, -149.60),
            ("siusi", 46.54, 11.62),
            ("prairie", 38.60, -96.50),
            ("iowa", 41.95, -93.65),
            ("smokies", 35.60, -83.80),
        ];
        eprintln!("place      level      herb   crop  shrub    wet   tree   ms");
        for (name, lat, lon) in places {
            for level in [12u8, 8, 6] {
                let id = node_at(lat, lon, level);
                let t = std::time::Instant::now();
                let (c, g) = crate::node::node_canopy_ground(&real, id, &|_, _| 100.0);
                let ms = t.elapsed().as_secs_f64() * 1e3;
                // Warm re-reads, to split the cost: canopy alone, ground alone.
                let t = std::time::Instant::now();
                let _ = crate::node::node_canopy(&real, id, &|_, _| 100.0);
                let (ms_c, t) = (t.elapsed().as_secs_f64() * 1e3, std::time::Instant::now());
                let _ = node_ground(&real, id);
                let ms_g = t.elapsed().as_secs_f64() * 1e3;
                let n = (PAGE_N * PAGE_N) as f64 * 255.0;
                let m = |k: usize| g.samples.iter().map(|s| s[k] as f64).sum::<f64>() / n;
                let tree = c.samples.iter().map(|s| s[0] as f64).sum::<f64>() / n;
                eprintln!("{name:<10} L{level:<2} {:>10.2} {:>6.2} {:>6.2} {:>6.2} {:>6.2} {ms:>6.0} (warm: canopy {ms_c:.0}, ground {ms_g:.0})", m(0), m(1), m(2), m(3), tree);
            }
        }
    }
}
