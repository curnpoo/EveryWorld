//! Ground height across tile borders, in the road network's metre frame.
//!
//! The road solver must not depend on which tile asked, so it reads the raw elevation data (no per-tile erosion
//! relief, no water carving, no grading) of the centre tile and its neighbours through one function of position.

use ed_data::{DiskCache, elevation};
use ed_geo::tile::TileId;
use std::collections::HashMap;
use std::sync::Arc;

pub type GroundFn = Arc<dyn Fn(f64, f64) -> f32 + Send + Sync>;

/// Smooth ground target under a point: a weighted low-pass of the raw heights (centre and a ring of 8 at 12 m). It removes the stair-steps and bumps of the source grid, and being a function of position alone it
/// agrees between every edge that passes through a point, which keeps junctions level.
pub fn smooth_ground(g: &dyn Fn(f64, f64) -> f32, x: f64, y: f64) -> f32 {
    let ring = |r: f64| -> f32 {
        let mut s = 0.0;
        for k in 0..8 {
            let a = k as f64 * std::f64::consts::FRAC_PI_4;
            s += g(x + r * a.cos(), y + r * a.sin());
        }
        s / 8.0
    };
    0.3 * g(x, y) + 0.7 * ring(12.0)
}

/// The elevation sources for the 3x3 block around `tile`. A missing neighbour falls back to the centre tile's
/// clamped edge (so the solver never invents a cliff); a missing centre falls back to flat 0.
pub fn load(cache: &DiskCache, tile: TileId, wm: f64) -> GroundFn {
    let ez = crate::tilegen::elevation_zoom(tile.z);
    let mut sources: HashMap<TileId, Option<Arc<elevation::HeightTile>>> = HashMap::new();
    // (dx, dy) -> (source id, u0, v0, scale)
    let mut map: HashMap<(i32, i32), (TileId, f64, f64, f64)> = HashMap::new();
    for dy in -1..=1i32 {
        for dx in -1..=1i32 {
            let Some(t) = tile.offset(dx as i64, dy as i64) else {
                continue;
            };
            let (anc, u0, v0, s) = t.ancestor(ez);
            map.insert((dx, dy), (anc, u0, v0, s));
            sources
                .entry(anc)
                .or_insert_with(|| elevation::fetch(cache, anc).ok().map(Arc::new));
        }
    }
    from_sources(wm, map, sources)
}

fn from_sources(
    wm: f64,
    map: HashMap<(i32, i32), (TileId, f64, f64, f64)>,
    sources: HashMap<TileId, Option<Arc<elevation::HeightTile>>>,
) -> GroundFn {
    Arc::new(move |x: f64, y: f64| {
        let (cx, cy) = ((x / wm).floor(), (y / wm).floor());
        let key = (cx.clamp(-1.0, 1.0) as i32, cy.clamp(-1.0, 1.0) as i32);
        // Outside the block: clamp onto its edge tile.
        let (u, v) = (
            ((x / wm) - key.0 as f64).clamp(0.0, 1.0),
            ((y / wm) - key.1 as f64).clamp(0.0, 1.0),
        );
        let Some((anc, u0, v0, s)) = map.get(&key).or_else(|| map.get(&(0, 0))) else {
            return 0.0;
        };
        match sources.get(anc).and_then(|o| o.as_ref()) {
            // Bilinear: the low-pass in `smooth_ground` averages 17 of these per sample, so cubic buys nothing.
            Some(h) => h.sample(u0 + s * u, v0 + s * v),
            None => 0.0,
        }
    })
}

/// A ground function from a closure (tests, tools).
pub fn from_fn(f: impl Fn(f64, f64) -> f32 + Send + Sync + 'static) -> GroundFn {
    Arc::new(f)
}

pub fn prefetch(cache: &DiskCache, tile: TileId) {
    let ez = crate::tilegen::elevation_zoom(tile.z);
    for dy in -1..=1i64 {
        for dx in -1..=1i64 {
            if let Some(t) = tile.offset(dx, dy) {
                let _ = elevation::prefetch(cache, t.ancestor(ez).0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoothing_keeps_a_plane_and_flattens_a_spike() {
        let plane = |x: f64, _y: f64| (x * 0.05) as f32;
        let s = smooth_ground(&plane, 100.0, 50.0);
        assert!((s - 5.0).abs() < 0.2, "{s}");
        let spike = |x: f64, y: f64| if x.hypot(y) < 1.0 { 10.0 } else { 0.0 };
        assert!(smooth_ground(&spike, 0.0, 0.0) < 3.5);
    }
}
