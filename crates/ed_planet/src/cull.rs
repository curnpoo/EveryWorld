//! CPU culls for nodes (P2-D5): horizon first, then frustum. f64 throughout.

use glam::{DQuat, DVec3};

/// Horizon occluder seen from one eye, both in the body-fixed frame.
#[derive(Clone, Copy, Debug)]
pub struct Horizon {
    eye: DVec3,
    dir: DVec3,
    dist: f64,
    /// `ro² / |eye|`: the plane through the tangent circle, along `dir`.
    plane: f64,
    /// Half angle of the tangent cone.
    alpha: f64,
    active: bool,
}

impl Horizon {
    /// `occluder` is the radius of a sphere at the body centre that is inside the terrain
    /// everywhere.
    pub fn new(eye: DVec3, occluder: f64) -> Horizon {
        let dist = eye.length();
        let active = dist > occluder * (1.0 + 1e-9) && occluder > 0.0;
        let dir = if dist > 0.0 { eye / dist } else { DVec3::Z };
        let (plane, alpha) = if active { (occluder * occluder / dist, libm::asin(occluder / dist)) } else { (0.0, 0.0) };
        Horizon { eye, dir, dist, plane, alpha, active }
    }

    /// True if the sphere is entirely hidden behind the occluder.
    ///
    /// A point is hidden when it lies inside the tangent cone from the eye and beyond the
    /// plane of the tangent circle: the ray to it then enters the occluder first.
    pub fn hidden(&self, centre: DVec3, radius: f64) -> bool {
        if !self.active || centre.dot(self.dir) + radius >= self.plane {
            return false;
        }
        let v = centre - self.eye;
        let l = v.length();
        if l <= radius {
            return false;
        }
        let cos_b = (v.dot(-self.dir) / l).clamp(-1.0, 1.0);
        let beta = libm::acos(cos_b);
        beta + libm::asin(radius / l) <= self.alpha && self.dist > 0.0
    }
}

/// Four side planes of a perspective frustum (no near or far plane: the near plane is
/// centimetres and the far plane is infinite), in camera space where the camera looks
/// down -Z with +Y up.
#[derive(Clone, Copy, Debug)]
pub struct Frustum {
    planes: [DVec3; 4],
    /// World (or any frame) to camera rotation.
    to_cam: DQuat,
}

impl Frustum {
    /// `rotation` is camera to frame; `fov_y` in radians; `aspect` = width / height.
    pub fn new(rotation: DQuat, fov_y: f64, aspect: f64) -> Frustum {
        let a = fov_y * 0.5;
        let b = libm::atan(libm::tan(a) * aspect);
        let (ca, sa) = (libm::cos(a), libm::sin(a));
        let (cb, sb) = (libm::cos(b), libm::sin(b));
        let planes = [DVec3::new(0.0, ca, sa), DVec3::new(0.0, -ca, sa), DVec3::new(cb, 0.0, sb), DVec3::new(-cb, 0.0, sb)];
        Frustum { planes, to_cam: rotation.inverse() }
    }

    /// `rel` is the sphere centre minus the eye, in the frame `rotation` maps into.
    pub fn visible(&self, rel: DVec3, radius: f64) -> bool {
        let c = self.to_cam * rel;
        self.planes.iter().all(|n| n.dot(c) <= radius)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horizon_hides_far_side() {
        let r = 6_356_000.0;
        let h = Horizon::new(DVec3::new(r + 1000.0, 0.0, 0.0), r);
        assert!(h.hidden(DVec3::new(-r, 0.0, 0.0), 1000.0));
        assert!(h.hidden(DVec3::new(0.0, r, 0.0), 1000.0));
        assert!(!h.hidden(DVec3::new(r, 0.0, 0.0), 10.0));
        // 100 km away at the surface: beyond the ~113 km horizon? not yet.
        let ang = 100_000.0 / r;
        assert!(!h.hidden(DVec3::new(r * ang.cos(), r * ang.sin(), 0.0) * 1.0001, 10.0));
        // 400 km away: hidden.
        let ang = 400_000.0 / r;
        assert!(h.hidden(DVec3::new(r * ang.cos(), r * ang.sin(), 0.0), 10.0));
        // Inside the occluder nothing is culled.
        assert!(!Horizon::new(DVec3::new(r * 0.5, 0.0, 0.0), r).hidden(DVec3::new(-r, 0.0, 0.0), 1.0));
    }

    #[test]
    fn frustum_sides() {
        let f = Frustum::new(DQuat::IDENTITY, 60f64.to_radians(), 16.0 / 9.0);
        assert!(f.visible(DVec3::new(0.0, 0.0, -10.0), 0.1));
        assert!(!f.visible(DVec3::new(0.0, 0.0, 10.0), 0.1));
        assert!(!f.visible(DVec3::new(0.0, 10.0, -1.0), 0.1));
        assert!(f.visible(DVec3::new(0.0, 0.0, 0.0), 0.1));
        // Rotated 180° about Y: looks down +Z.
        let g = Frustum::new(DQuat::from_rotation_y(std::f64::consts::PI), 1.0, 1.0);
        assert!(g.visible(DVec3::new(0.0, 0.0, 10.0), 0.1));
    }
}
