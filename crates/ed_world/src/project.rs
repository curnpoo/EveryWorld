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
}
