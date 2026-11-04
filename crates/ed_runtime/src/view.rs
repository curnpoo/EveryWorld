//! Conservative "could this be on screen" test for streaming.
//!
//! The old test in `streaming.rs` put every tile's bounding sphere at sea
//! level with 500 m of slack. In high places (Zermatt, La Paz, the Tibetan
//! plateau) the ground sits kilometres above that sphere, so tiles in view
//! were dropped. [`TileBounds`] uses the tile's real height range.

use glam::Vec3;

/// A camera's view cone in render space.
#[derive(Debug, Clone, Copy)]
pub struct ViewCone {
    pub pos: Vec3,
    pub forward: Vec3,
    pub right: Vec3,
    pub up: Vec3,
    /// tan(vertical fov / 2)
    pub tan_half_fov: f32,
    pub aspect: f32,
}

impl ViewCone {
    /// Whether a sphere may intersect the view pyramid (no near/far planes).
    /// Conservative: it errs on "visible".
    pub fn sees_sphere(&self, center: Vec3, radius: f32) -> bool {
        let to = center - self.pos;
        if to.length_squared() <= radius * radius {
            return true;
        }
        let depth = to.dot(self.forward);
        if depth + radius < 0.0 {
            return false;
        }
        let half_depth = depth.max(0.0) + radius;
        let horizontal = to.dot(self.right).abs();
        let vertical = to.dot(self.up).abs();
        horizontal <= half_depth * self.tan_half_fov * self.aspect + radius && vertical <= half_depth * self.tan_half_fov + radius
    }
}

/// Bounds of a tile column in render space.
#[derive(Debug, Clone, Copy)]
pub struct TileBounds {
    /// Tile centre at the bottom of its height range.
    pub bottom: Vec3,
    /// Tile centre at the top of its height range (plus building allowance).
    pub top: Vec3,
    pub half_width: f32,
}

impl TileBounds {
    /// Bounding sphere of the column: centred halfway up, wide enough for the
    /// footprint corners at either end.
    pub fn sphere(&self) -> (Vec3, f32) {
        let center = (self.bottom + self.top) * 0.5;
        let half_h = (self.top - self.bottom).length() * 0.5;
        let corner = self.half_width * std::f32::consts::SQRT_2;
        (center, (corner * corner + half_h * half_h).sqrt())
    }
}

/// Height range to assume for a tile, in metres.
///
/// `known` is the tile's own (or its nearest loaded ancestor's) min/max
/// ground height. Without it, fall back to the ground under the camera with
/// generous slack. `building_m` is added on top for buildings and trees.
pub fn height_range(known: Option<(f32, f32)>, ground_estimate: f32, building_m: f32) -> (f32, f32) {
    let (lo, hi) = known.unwrap_or((ground_estimate - 1_500.0, ground_estimate + 1_500.0));
    (lo.min(hi) - 50.0, lo.max(hi) + building_m)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cam(pitch_deg: f32, height: f32) -> ViewCone {
        let p = pitch_deg.to_radians();
        let forward = Vec3::new(0.0, p.sin(), -p.cos());
        let right = Vec3::X;
        ViewCone { pos: Vec3::new(0.0, height, 0.0), forward, right, up: right.cross(forward), tan_half_fov: (45f32.to_radians() * 0.5).tan(), aspect: 16.0 / 9.0 }
    }

    /// The old streaming test: sphere at sea level, radius = corner + 500 m.
    fn old_sees(c: &ViewCone, ahead: f32, half_width: f32) -> bool {
        c.sees_sphere(Vec3::new(0.0, 0.0, -ahead), half_width * std::f32::consts::SQRT_2 + 500.0)
    }

    #[test]
    fn behind_is_culled_ahead_is_kept() {
        let c = cam(0.0, 2.0);
        assert!(c.sees_sphere(Vec3::new(0.0, 0.0, -5_000.0), 100.0));
        assert!(!c.sees_sphere(Vec3::new(0.0, 0.0, 5_000.0), 100.0));
        assert!(c.sees_sphere(Vec3::new(0.0, 0.0, 50.0), 100.0), "camera inside the sphere");
    }

    /// Altiplano / Tibet: camera on 4,500 m ground looking level, z14 tile
    /// 2.5 km ahead (beyond the old 1.5 km keep radius). The old sea-level
    /// sphere sits 4.5 km below the camera and falls outside the view.
    #[test]
    fn high_plateau_tile_ahead_is_visible() {
        let c = cam(0.0, 4_502.0);
        let half_width = 1_150.0;
        assert!(!old_sees(&c, 2_500.0, half_width), "old sea-level test drops it");
        let (lo, hi) = height_range(Some((4_450.0, 4_600.0)), 4_500.0, 500.0);
        let new = TileBounds { bottom: Vec3::new(0.0, lo, -2_500.0), top: Vec3::new(0.0, hi, -2_500.0), half_width };
        let (nc, nr) = new.sphere();
        assert!(c.sees_sphere(nc, nr));
    }

    /// Zermatt (1,620 m) looking 30 degrees up at a 4,478 m summit 5 km away.
    #[test]
    fn looking_up_at_a_peak() {
        let c = cam(30.0, 1_620.0);
        let half_width = 850.0;
        assert!(!old_sees(&c, 5_000.0, half_width), "old sea-level test drops it");
        let (lo, hi) = height_range(Some((2_400.0, 4_478.0)), 1_620.0, 0.0);
        let b = TileBounds { bottom: Vec3::new(0.0, lo, -5_000.0), top: Vec3::new(0.0, hi, -5_000.0), half_width };
        let (sc, sr) = b.sphere();
        assert!(c.sees_sphere(sc, sr));
    }

    /// Lowland behaviour is unchanged in spirit: a tile far behind stays culled.
    #[test]
    fn lowland_behind_still_culled() {
        let c = cam(0.0, 20.0);
        let (lo, hi) = height_range(Some((0.0, 30.0)), 10.0, 500.0);
        let b = TileBounds { bottom: Vec3::new(0.0, lo, 8_000.0), top: Vec3::new(0.0, hi, 8_000.0), half_width: 1_200.0 };
        let (sc, sr) = b.sphere();
        assert!(!c.sees_sphere(sc, sr));
    }

    #[test]
    fn unknown_height_falls_back_to_camera_ground() {
        assert_eq!(height_range(None, 1_000.0, 0.0), (-550.0, 2_500.0));
    }
}
