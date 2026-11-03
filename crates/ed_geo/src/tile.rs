//! Web-Mercator slippy-map tile addressing (the z/x/y scheme used by
//! OpenMapTiles, Terrarium elevation, and basically every public map source).

use crate::Geodetic;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/// Mercator's latitude limit.
pub const MAX_LAT: f64 = 85.051_128_779_806_59;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TileId {
    pub z: u8,
    pub x: u32,
    pub y: u32,
}

impl TileId {
    pub const fn new(z: u8, x: u32, y: u32) -> Self {
        Self { z, x, y }
    }

    pub fn n(z: u8) -> u32 {
        1u32 << z
    }

    /// Tile containing a lat/lon at zoom `z`.
    pub fn containing(lat_deg: f64, lon_deg: f64, z: u8) -> Self {
        let (fx, fy) = lonlat_to_tile_f(lon_deg, lat_deg, z);
        let n = Self::n(z);
        Self::new(z, (fx.floor() as i64).clamp(0, n as i64 - 1) as u32, (fy.floor() as i64).clamp(0, n as i64 - 1) as u32)
    }

    /// Geodetic position (alt 0) of a point inside the tile, `u`,`v` in [0,1],
    /// v=0 at the tile's north edge.
    pub fn uv_to_geodetic(&self, u: f64, v: f64) -> Geodetic {
        let n = Self::n(self.z) as f64;
        let lon = (self.x as f64 + u) / n * 360.0 - 180.0;
        let my = PI * (1.0 - 2.0 * (self.y as f64 + v) / n);
        let lat = my.sinh().atan().to_degrees();
        Geodetic::new(lat, lon, 0.0)
    }

    /// Fractional position of a lat/lon inside this tile (may be outside [0,1]).
    pub fn geodetic_to_uv(&self, lat_deg: f64, lon_deg: f64) -> (f64, f64) {
        let (fx, fy) = lonlat_to_tile_f(lon_deg, lat_deg, self.z);
        (fx - self.x as f64, fy - self.y as f64)
    }

    pub fn center(&self) -> Geodetic {
        self.uv_to_geodetic(0.5, 0.5)
    }

    /// Approximate ground width of the tile in meters (east-west, at its center).
    pub fn width_m(&self) -> f64 {
        let c = self.center();
        2.0 * PI * crate::WGS84_A * c.lat_deg.to_radians().cos() / Self::n(self.z) as f64
    }

    pub fn parent(&self) -> Option<TileId> {
        (self.z > 0).then(|| TileId::new(self.z - 1, self.x / 2, self.y / 2))
    }

    /// Ancestor at a lower zoom, plus this tile's (u0, v0, scale) inside it.
    pub fn ancestor(&self, z: u8) -> (TileId, f64, f64, f64) {
        assert!(z <= self.z);
        let d = self.z - z;
        let k = 1u32 << d;
        let a = TileId::new(z, self.x / k, self.y / k);
        let scale = 1.0 / k as f64;
        let u0 = (self.x % k) as f64 * scale;
        let v0 = (self.y % k) as f64 * scale;
        (a, u0, v0, scale)
    }

    pub fn children(&self) -> [TileId; 4] {
        let (z, x, y) = (self.z + 1, self.x * 2, self.y * 2);
        [TileId::new(z, x, y), TileId::new(z, x + 1, y), TileId::new(z, x, y + 1), TileId::new(z, x + 1, y + 1)]
    }

    /// Neighbour offset by (dx, dy), wrapping in x, clamped in y.
    pub fn offset(&self, dx: i64, dy: i64) -> Option<TileId> {
        let n = Self::n(self.z) as i64;
        let y = self.y as i64 + dy;
        if y < 0 || y >= n {
            return None;
        }
        let x = (self.x as i64 + dx).rem_euclid(n);
        Some(TileId::new(self.z, x as u32, y as u32))
    }

    pub fn path(&self) -> String {
        format!("{}/{}/{}", self.z, self.x, self.y)
    }
}

fn lonlat_to_tile_f(lon_deg: f64, lat_deg: f64, z: u8) -> (f64, f64) {
    let n = TileId::n(z) as f64;
    let lat = lat_deg.clamp(-MAX_LAT, MAX_LAT).to_radians();
    let x = (lon_deg + 180.0) / 360.0 * n;
    let y = (1.0 - (lat.tan() + 1.0 / lat.cos()).ln() / PI) / 2.0 * n;
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_tile() {
        // Zermatt at z14 per any slippy-map calculator.
        let t = TileId::containing(46.0207, 7.7491, 14);
        assert_eq!((t.x, t.y), (8544, 5827));
    }

    #[test]
    fn uv_roundtrip() {
        let t = TileId::new(12, 1205, 1539);
        let g = t.uv_to_geodetic(0.25, 0.75);
        let (u, v) = t.geodetic_to_uv(g.lat_deg, g.lon_deg);
        assert!((u - 0.25).abs() < 1e-9 && (v - 0.75).abs() < 1e-9);
    }

    #[test]
    fn ancestor_math() {
        let t = TileId::new(14, 8545, 5851);
        let (a, u0, v0, s) = t.ancestor(12);
        assert_eq!(a, TileId::new(12, 2136, 1462));
        assert_eq!((u0, v0, s), (0.25, 0.75, 0.25));
    }

    #[test]
    fn width() {
        let w = TileId::containing(0.0, 0.0, 14).width_m();
        assert!((w - 2446.0).abs() < 5.0, "{w}");
    }
}
