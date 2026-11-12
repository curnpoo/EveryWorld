//! Terrain bodies: shape constants, the surface mapping and per-frame poses (P2-D18).
//!
//! The surface is an ellipsoid of revolution about body-fixed +Z. A terrain point is the
//! node lattice direction `d` scaled to the ellipsoid along that same ray and then displaced
//! **radially** by the height: `d * (radius(d) + h)`. Radial displacement keeps the height
//! query a pure function of direction (no iteration), and the renderer, collision and the
//! height query all use the same rule.

use ed_geo::{BodyId, NodeId};
use glam::{DMat3, DQuat, DVec3};

/// Shape and limits of a terrain body. Constant for the session.
#[derive(Clone, Debug, PartialEq)]
pub struct Body {
    pub id: BodyId,
    /// Equatorial semi-axis (m).
    pub equatorial_m: f64,
    /// Polar semi-axis (m), along body-fixed +Z.
    pub polar_m: f64,
    /// Lowest and highest terrain on the body (m above the ellipsoid), used for nodes whose
    /// heights are not known yet and for the horizon occluder.
    pub min_h: f32,
    pub max_h: f32,
    /// Deepest level the selector refines to.
    pub max_level: u8,
}

impl Body {
    /// WGS84 Earth, catalog index of `ed_space::BodyId::Earth`.
    pub fn earth() -> Body {
        Body {
            id: BodyId(ed_space::BodyId::Earth as u8),
            equatorial_m: ed_geo::WGS84_A,
            polar_m: ed_geo::WGS84_B,
            min_h: -11_000.0,
            max_h: 8_900.0,
            max_level: 17,
        }
    }

    /// The Moon as a sphere of the catalog radius.
    pub fn moon() -> Body {
        let r = ed_space::info(ed_space::BodyId::Moon).radius_m;
        Body { id: BodyId(ed_space::BodyId::Moon as u8), equatorial_m: r, polar_m: r, min_h: -9_200.0, max_h: 10_800.0, max_level: 16 }
    }

    /// Ellipsoid radius along unit direction `d`.
    pub fn radius(&self, d: DVec3) -> f64 {
        let (a2, b2) = (self.equatorial_m * self.equatorial_m, self.polar_m * self.polar_m);
        1.0 / ((d.x * d.x + d.y * d.y) / a2 + d.z * d.z / b2).sqrt()
    }

    /// Body-fixed point at height `h` along unit direction `d` (radial displacement).
    pub fn point(&self, d: DVec3, h: f64) -> DVec3 {
        d * (self.radius(d) + h)
    }

    /// Radius of a sphere that is inside the body's terrain everywhere: the horizon occluder.
    pub fn occluder_radius(&self) -> f64 {
        self.equatorial_m.min(self.polar_m) + self.min_h as f64
    }

    /// Ellipsoid normal at the surface point along unit direction `d`.
    pub fn up(&self, d: DVec3) -> DVec3 {
        let (a2, b2) = (self.equatorial_m * self.equatorial_m, self.polar_m * self.polar_m);
        DVec3::new(d.x / a2, d.y / a2, d.z / b2).normalize()
    }

    /// East, north, up (columns) at the surface point along `d`. At a pole east is +Y.
    pub fn frame(&self, d: DVec3) -> DMat3 {
        let up = self.up(d);
        let e = DVec3::Z.cross(up);
        let east = if e.length_squared() > 1e-24 { e.normalize() } else { DVec3::Y };
        let north = up.cross(east);
        DMat3::from_cols(east, north, up)
    }

    /// Origin of a node's frame: its centre direction on the ellipsoid (`addressing.md` §5).
    pub fn node_origin(&self, id: NodeId) -> DVec3 {
        let d = id.centre();
        self.point(d, 0.0)
    }
}

/// Where a body is this frame: body-fixed to world rotation and the body centre in the world
/// frame (f64). The world frame is whatever the camera is given in; body-to-body transforms
/// come from `ed_space` (P2-D7).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyPose {
    pub id: BodyId,
    pub rotation: DQuat,
    pub position: DVec3,
}

impl BodyPose {
    pub fn identity(id: BodyId) -> BodyPose {
        BodyPose { id, rotation: DQuat::IDENTITY, position: DVec3::ZERO }
    }
    pub fn to_body(&self, world: DVec3) -> DVec3 {
        self.rotation.inverse() * (world - self.position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn earth_radius_and_frame() {
        let e = Body::earth();
        assert!((e.radius(DVec3::X) - ed_geo::WGS84_A).abs() < 1e-6);
        assert!((e.radius(DVec3::Z) - ed_geo::WGS84_B).abs() < 1e-6);
        let f = e.frame(DVec3::new(1.0, 1.0, 1.0).normalize());
        assert!((f.x_axis.dot(f.y_axis)).abs() < 1e-12);
        assert!((f.z_axis.length() - 1.0).abs() < 1e-12);
        assert!(f.y_axis.z > 0.0, "north points to +Z");
        let p = e.frame(DVec3::Z);
        assert!((p.x_axis - DVec3::Y).length() < 1e-12);
    }
}
