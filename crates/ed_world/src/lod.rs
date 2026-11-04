//! Which terrain tiles to show. A quadtree over Web-Mercator tiles: a tile
//! splits into its 4 children while the camera is close relative to the
//! tile's size. Result: ~2 km tiles under your feet, ~600 km tiles at the
//! horizon, a few hundred tiles total.

use ed_geo::{Geodetic, WGS84_A, tile::TileId};

#[derive(Debug, Clone, Copy)]
pub struct LodParams {
    /// Coarsest zoom considered.
    pub root_zoom: u8,
    /// Finest zoom (terrain detail limit).
    pub max_zoom: u8,
    /// Split while distance < split * tile width.
    pub split: f64,
    /// Root tiles around the camera in each direction.
    pub root_radius: i64,
}

impl Default for LodParams {
    fn default() -> Self {
        Self { root_zoom: 6, max_zoom: 14, split: 1.0, root_radius: 2 }
    }
}

pub fn horizon_m(alt_m: f64) -> f64 {
    let h = alt_m.max(2.0) + 2_000.0; // allow for mountains poking above
    (2.0 * WGS84_A * h).sqrt()
}

/// Leaf tiles to display for a camera at `cam`.
pub fn select(cam: Geodetic, ground_m: f64, p: &LodParams) -> Vec<TileId> {
    let agl = (cam.alt_m - ground_m).max(1.0);
    let horizon = horizon_m(cam.alt_m) * 1.3;
    let root = TileId::containing(cam.lat_deg, cam.lon_deg, p.root_zoom);
    let mut out = Vec::new();
    let mut stack = Vec::new();
    for dy in -p.root_radius..=p.root_radius {
        for dx in -p.root_radius..=p.root_radius {
            if let Some(t) = root.offset(dx, dy) {
                stack.push(t);
            }
        }
    }
    while let Some(t) = stack.pop() {
        let w = t.width_m();
        let d_ground = distance_to_tile(cam, t);
        if d_ground > horizon + w {
            continue;
        }
        let d = (d_ground * d_ground + agl * agl).sqrt();
        if t.z < p.max_zoom && d < p.split * w {
            stack.extend(t.children());
        } else {
            out.push(t);
        }
    }
    out
}

/// Ground distance from the camera to the nearest point of a tile.
fn distance_to_tile(cam: Geodetic, t: TileId) -> f64 {
    let (u, v) = t.geodetic_to_uv(cam.lat_deg, cam.lon_deg);
    let (cu, cv) = (u.clamp(0.0, 1.0), v.clamp(0.0, 1.0));
    if (cu - u).abs() < 1e-12 && (cv - v).abs() < 1e-12 {
        return 0.0;
    }
    cam.distance_to(t.uv_to_geodetic(cu, cv))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ground_level_selection() {
        let cam = Geodetic::new(46.02, 7.75, 1_620.0);
        let tiles = select(cam, 1_600.0, &LodParams::default());
        let under = tiles.iter().find(|t| t.geodetic_to_uv(46.02, 7.75).0.clamp(0.0, 1.0) == t.geodetic_to_uv(46.02, 7.75).0
            && (0.0..=1.0).contains(&t.geodetic_to_uv(46.02, 7.75).1)).unwrap();
        assert_eq!(under.z, 14, "finest tile under the player");
        assert!(tiles.len() < 600, "{} tiles", tiles.len());
        assert!(tiles.iter().any(|t| t.z <= 9), "far tiles are coarse");
    }

    #[test]
    fn high_altitude_is_coarser() {
        let low = select(Geodetic::new(40.7, -74.0, 300.0), 0.0, &LodParams::default());
        let high = select(Geodetic::new(40.7, -74.0, 30_000.0), 0.0, &LodParams::default());
        let max_low = low.iter().map(|t| t.z).max().unwrap();
        let max_high = high.iter().map(|t| t.z).max().unwrap();
        assert!(max_high < max_low, "{max_high} vs {max_low}");
    }
}
