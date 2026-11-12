//! A local frame on a body (P-slice): one anchor on the ellipsoid that both the renderer and
//! the sim use, so a point rendered at `r` and collided at `s` are the same body point.
//!
//! * Body frame: body-fixed f64 (`Body::point`).
//! * Sim frame (ENU, z up): `s = F^T (b - p0)`, `F = Body::frame(d0)` (east, north, up columns),
//!   `p0 = Body::point(d0, 0)`. This is the `ed_sim` region frame.
//! * Render frame (x = east, y = up, z = -north): `r = (s.x, s.z, -s.y)`.
//!
//! The renderer receives the body through [`Anchor::render_pose`]: `render_pose().to_body(r)`
//! gives the body point of render position `r`.

use crate::body::{Body, BodyPose};
use glam::{DMat3, DQuat, DVec3};

#[derive(Clone, Debug, PartialEq)]
pub struct Anchor {
    pub body: Body,
    /// Unit direction of the anchor (body frame).
    pub dir: DVec3,
    /// Anchor point on the ellipsoid (body frame).
    pub origin: DVec3,
    /// East, north, up columns at the anchor (body frame).
    pub enu: DMat3,
}

/// Sim (ENU) to render axes: (e, n, u) -> (e, u, -n).
const SIM_TO_RENDER: DMat3 = DMat3::from_cols(DVec3::new(1.0, 0.0, 0.0), DVec3::new(0.0, 0.0, -1.0), DVec3::new(0.0, 1.0, 0.0));

impl Anchor {
    /// Anchor at body direction `dir` (normalised here).
    pub fn new(body: Body, dir: DVec3) -> Anchor {
        let dir = dir.normalize();
        let (origin, enu) = (body.point(dir, 0.0), body.frame(dir));
        Anchor { body, dir, origin, enu }
    }

    /// Anchor under an Earth-fixed ECEF point (e.g. `Geodetic::to_ecef` at h = 0): the body
    /// direction is the point's direction, the rule `ed_data` uses for node heights (`d·r(d)`).
    pub fn from_ecef(body: Body, ecef: DVec3) -> Anchor {
        Anchor::new(body, ecef)
    }

    pub fn body_to_sim(&self, b: DVec3) -> DVec3 {
        self.enu.transpose() * (b - self.origin)
    }
    pub fn sim_to_body(&self, s: DVec3) -> DVec3 {
        self.origin + self.enu * s
    }
    pub fn sim_to_render(s: DVec3) -> DVec3 {
        SIM_TO_RENDER * s
    }
    pub fn render_to_sim(r: DVec3) -> DVec3 {
        SIM_TO_RENDER.transpose() * r
    }
    pub fn body_to_render(&self, b: DVec3) -> DVec3 {
        Self::sim_to_render(self.body_to_sim(b))
    }
    pub fn render_to_body(&self, r: DVec3) -> DVec3 {
        self.sim_to_body(Self::render_to_sim(r))
    }

    /// Body-to-render pose for the renderer: `pose.rotation * b + pose.position = r`.
    pub fn render_pose(&self) -> BodyPose {
        let rot = DQuat::from_mat3(&(SIM_TO_RENDER * self.enu.transpose()));
        BodyPose { id: self.body.id, rotation: rot, position: -(rot * self.origin) }
    }

    /// Local up (ellipsoid normal) at a sim position, in the sim frame.
    pub fn up_sim(&self, s: DVec3) -> DVec3 {
        let b = self.sim_to_body(s);
        self.enu.transpose() * self.body.up(b.normalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_agree() {
        let ecef = ed_geo::Geodetic::new(46.0207, 7.7491, 0.0).to_ecef();
        let a = Anchor::from_ecef(Body::earth(), ecef);
        assert!(a.body_to_sim(a.origin).length() < 1e-6);
        let up = a.body_to_sim(a.origin + a.enu.col(2) * 100.0);
        assert!((up - DVec3::new(0.0, 0.0, 100.0)).length() < 1e-6);
        let pose = a.render_pose();
        for b in [a.origin, a.origin + DVec3::new(1234.0, -567.0, 89.0), a.body.point(a.dir, 1600.0)] {
            let r = a.body_to_render(b);
            assert!((pose.rotation * b + pose.position - r).length() < 1e-6, "{r}");
            assert!((pose.to_body(r) - b).length() < 1e-6);
            assert!((a.render_to_body(r) - b).length() < 1e-6);
        }
        // Up at the anchor is render +Y.
        let r_up = a.body_to_render(a.origin + a.enu.col(2));
        assert!((r_up - DVec3::Y).length() < 1e-9);
    }
}
