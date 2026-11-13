//! Erosion detail (P-slice S7 b) on steep synthetic mountains: shared borders stay bit-identical
//! at every detail level, the detail is really there on steep ground, and it stays within its
//! bound of the smooth source.

mod node_common;
use ed_data::node::{HeightPage, Raster, SourceView, max_detail_m, node_heights};
use ed_geo::node::{BodyId, NodeId};
use node_common::Rng;
use std::collections::HashMap;
use std::f64::consts::PI;
use std::sync::{Arc, Mutex};

/// Steep analytic relief (slopes up to ~1.7, i.e. 60 degrees) around Zermatt, gentle elsewhere.
fn steep(lat: f64, lon: f64) -> f64 {
    let (y, x) = ((lat - 46.0) * 111_000.0, (lon - 7.7) * 77_000.0);
    2500.0 + 420.0 * (x / 260.0).sin() * (y / 330.0).cos() + 150.0 * ((x + y) / 120.0).sin()
}

struct Steep(Mutex<HashMap<(u8, u32, u32), Arc<Raster>>>);
impl SourceView for Steep {
    fn elevation_tile(&self, z: u8, x: u32, y: u32) -> Option<Arc<Raster>> {
        if let Some(t) = self.0.lock().unwrap().get(&(z, x, y)) {
            return Some(t.clone());
        }
        let n = (1u64 << z) as f64;
        let mut heights = Vec::with_capacity(256 * 256);
        for py in 0..256 {
            let fy = (y as f64 * 256.0 + py as f64 + 0.5) / (n * 256.0);
            let lat = (PI * (1.0 - 2.0 * fy)).sinh().atan().to_degrees();
            for px in 0..256 {
                let fx = (x as f64 * 256.0 + px as f64 + 0.5) / (n * 256.0);
                heights.push(steep(lat, fx * 360.0 - 180.0) as f32);
            }
        }
        let t = Arc::new(Raster { size: 256, heights });
        self.0.lock().unwrap().insert((z, x, y), t.clone());
        Some(t)
    }
}

fn zermatt(level: u8) -> NodeId {
    let d = ed_geo::Geodetic::new(46.0, 7.7, 0.0).to_ecef().normalize();
    NodeId::from_unit(BodyId(0), d, level)
}

fn edge(p: &HeightPage, k: usize, side: u8) -> f32 {
    let (i, j) = match side {
        0 => (0, k),
        1 => (256, k),
        2 => (k, 0),
        _ => (k, 256),
    };
    p.height(i, j) as f32
}

#[test]
fn detail_borders_bit_identical_on_steep_ground() {
    let src = Steep(Mutex::new(HashMap::new()));
    let mut rng = Rng(0xDE7A_11);
    for level in 13..=18u8 {
        let base = zermatt(level);
        for _ in 0..3 {
            let id = base.neighbour(rng.below(5) as i32 - 2, rng.below(5) as i32 - 2);
            let pa = node_heights(&src, id);
            for (dx, dy, sa, sb) in [(1, 0, 1u8, 0u8), (0, 1, 3, 2)] {
                let nb = id.neighbour(dx, dy);
                let pb = node_heights(&src, nb);
                // Same face here: the shared edge is i = 256 / i = 0 (or j), sample for sample.
                for k in 0..257 {
                    assert_eq!(id.grid_point(if sa == 1 { 256 } else { k as u32 }, if sa == 3 { 256 } else { k as u32 }, 256), nb.grid_point(if sb == 0 { 0 } else { k as u32 }, if sb == 2 { 0 } else { k as u32 }, 256));
                    assert_eq!(edge(&pa, k, sa).to_bits(), edge(&pb, k, sb).to_bits(), "level {level} {id} / {nb} at {k}");
                }
            }
        }
    }
}

#[test]
fn detail_is_present_and_bounded() {
    let src = Steep(Mutex::new(HashMap::new()));
    let id = zermatt(17);
    let p = node_heights(&src, id);
    // Residual against the smooth field: second differences along i (the source's own curvature
    // over 0.3 m is ~1e-3 m; the detail's is centimetres).
    let mut rough = 0.0f64;
    for j in (0..257).step_by(8) {
        for i in 1..256 {
            rough = rough.max((p.height(i - 1, j) - 2.0 * p.height(i, j) + p.height(i + 1, j)).abs());
        }
    }
    assert!(rough > 0.01, "no fine relief on steep ground: {rough}");
    // Deviation from the analytic source stays within the detail bound plus interpolation error.
    let mut worst = 0.0f64;
    for j in (0..257usize).step_by(16) {
        for i in (0..257usize).step_by(16) {
            // Against the page's own smooth neighbourhood (mean of a 9 x 9 block, 2.4 m wide).
            let (lo, hi) = (i.saturating_sub(4), (i + 4).min(256));
            let (jl, jh) = (j.saturating_sub(4), (j + 4).min(256));
            let mut s = 0.0;
            let mut n = 0.0;
            for jj in jl..=jh {
                for ii in lo..=hi {
                    s += p.height(ii, jj);
                    n += 1.0;
                }
            }
            worst = worst.max((p.height(i, j) - s / n).abs());
        }
    }
    assert!(worst < max_detail_m() + 1.0, "detail moved ground by {worst} m (bound {})", max_detail_m());
}
