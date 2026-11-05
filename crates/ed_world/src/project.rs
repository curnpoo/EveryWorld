//! Projecting tile-space coordinates onto the curved Earth in render space.

use ed_geo::{EnuFrame, Geodetic, tile::TileId};
use glam::{DVec3, Vec3};

/// Maps a tile's (u, v) and height onto render space, relative to `anchor`
/// (the tile center at sea level). Keeping vertices relative to a per-tile
/// anchor keeps `f32` precision at millimeters even on big tiles.
#[derive(Debug, Clone, Copy)]
pub struct TileProjector {
    pub tile: TileId,
    pub frame: EnuFrame,
    pub anchor: DVec3,
}

impl TileProjector {
    pub fn new(tile: TileId, frame: EnuFrame) -> Self {
        let anchor = frame.geodetic_to_render(tile.center());
        Self { tile, frame, anchor }
    }

    pub fn geodetic(&self, u: f64, v: f64, h: f64) -> Geodetic {
        let g = self.tile.uv_to_geodetic(u, v);
        Geodetic { alt_m: h, ..g }
    }

    /// Render-space position relative to the anchor.
    pub fn local(&self, u: f64, v: f64, h: f64) -> Vec3 {
        (self.frame.geodetic_to_render(self.geodetic(u, v, h)) - self.anchor).as_vec3()
    }

    /// Local "up" at the tile center (good enough across one tile).
    pub fn up(&self) -> Vec3 {
        self.frame.up_at_render(self.anchor).as_vec3()
    }

    /// Approximate inverse of [`Self::local`]: anchor-relative position ->
    /// tile (u, v), as an affine map fitted to three corners (height drops
    /// out: it is ~perpendicular to the tile plane). Good to a fraction of a
    /// percent of the tile, plenty for per-tile lookup textures.
    pub fn uv_map(&self) -> UvMap {
        let o = self.local(0.0, 0.0, 0.0);
        let a = self.local(1.0, 0.0, 0.0) - o;
        let b = self.local(0.0, 1.0, 0.0) - o;
        let (aa, ab, bb) = (a.dot(a), a.dot(b), b.dot(b));
        let det = (aa * bb - ab * ab).max(1e-12);
        UvMap { o, a, b, inv: [bb / det, -ab / det, aa / det] }
    }
}

/// See [`TileProjector::uv_map`].
#[derive(Debug, Clone, Copy)]
pub struct UvMap {
    o: Vec3,
    a: Vec3,
    b: Vec3,
    /// Inverse Gram matrix [00, 01 = 10, 11].
    inv: [f32; 3],
}

impl UvMap {
    pub fn uv(&self, p: Vec3) -> [f32; 2] {
        let d = p - self.o;
        let (da, db) = (d.dot(self.a), d.dot(self.b));
        [self.inv[0] * da + self.inv[1] * db, self.inv[1] * da + self.inv[2] * db]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uv_map_inverts_local() {
        let tile = TileId::containing(46.0, 7.7, 11);
        let frame = EnuFrame::new(Geodetic { lat_deg: 45.5, lon_deg: 7.0, alt_m: 0.0 });
        let p = TileProjector::new(tile, frame);
        let m = p.uv_map();
        for (u, v, h) in [(0.0, 0.0, 0.0), (0.5, 0.5, 3000.0), (0.9, 0.2, 1500.0), (0.1, 1.0, 4000.0)] {
            let [mu, mv] = m.uv(p.local(u, v, h));
            assert!((mu as f64 - u).abs() < 0.005 && (mv as f64 - v).abs() < 0.005, "{u},{v},{h} -> {mu},{mv}");
        }
    }
}
