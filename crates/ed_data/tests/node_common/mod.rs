//! Shared synthetic source for the node tests: an analytic height function served as slippy
//! tiles (256 px, pixel centres), with a tile cache that is never iterated.
#![allow(dead_code)]

use ed_data::node::{Raster, SourceView};
use ed_geo::node::{BodyId, NodeId};
use ed_geo::{Geodetic, WGS84_A, WGS84_B};
use std::collections::HashMap;
use std::f64::consts::PI;
use std::sync::{Arc, Mutex};

pub const TILE: usize = 256;

/// Smooth analytic terrain (metres). Second derivatives bounded by `F_PP`.
pub fn analytic(lat_deg: f64, lon_deg: f64) -> f64 {
    let (la, lo) = (lat_deg.to_radians(), lon_deg.to_radians());
    2000.0 * (2.0 * la).sin() * (3.0 * lo).cos() + 500.0 * (5.0 * lo + la).sin() - 300.0
}
/// Bound on |d2h/dtheta2| in metres per radian^2 (2000*9 + 2000*4 cross terms + 500*25..).
pub const F_PP: f64 = 2000.0 * 9.0 + 2000.0 * 4.0 + 500.0 * 26.0;

pub struct Synth {
    cache: Mutex<HashMap<(u8, u32, u32), Arc<Raster>>>,
}
impl Synth {
    pub fn new() -> Self {
        Synth { cache: Mutex::new(HashMap::new()) }
    }
}
impl SourceView for Synth {
    fn elevation_tile(&self, z: u8, x: u32, y: u32) -> Option<Arc<Raster>> {
        if let Some(t) = self.cache.lock().unwrap().get(&(z, x, y)) {
            return Some(t.clone());
        }
        let n = (1u64 << z) as f64;
        let mut heights = Vec::with_capacity(TILE * TILE);
        for py in 0..TILE {
            let fy = (y as f64 * TILE as f64 + py as f64 + 0.5) / (n * TILE as f64);
            let lat = (PI * (1.0 - 2.0 * fy)).sinh().atan().to_degrees();
            for px in 0..TILE {
                let fx = (x as f64 * TILE as f64 + px as f64 + 0.5) / (n * TILE as f64);
                heights.push(analytic(lat, fx * 360.0 - 180.0) as f32);
            }
        }
        let t = Arc::new(Raster { size: TILE, heights });
        self.cache.lock().unwrap().insert((z, x, y), t.clone());
        Some(t)
    }
}

pub struct Rng(pub u64);
impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

pub fn random_node(rng: &mut Rng, max_level: u8) -> NodeId {
    let level = rng.below(max_level as u64 + 1) as u8;
    let n = 1u64 << level;
    NodeId::new(BodyId(0), rng.below(6) as u8, level, rng.below(n) as u32, rng.below(n) as u32).unwrap()
}

/// Geodetic lat/lon of lattice point (i, j) of `id`'s 256-cell grid: the ellipsoid point
/// `d * r(d)` along the lattice direction, as `ed_planet` places it.
pub fn lat_lon(id: NodeId, i: u32, j: u32) -> (f64, f64) {
    let d = id.grid_point(i, j, 256);
    let r = 1.0 / ((d.x * d.x + d.y * d.y) / (WGS84_A * WGS84_A) + d.z * d.z / (WGS84_B * WGS84_B)).sqrt();
    let g = Geodetic::from_ecef(d * r);
    (g.lat_deg, g.lon_deg)
}
