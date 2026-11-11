//! Object transforms in the editor: f64 body-fixed position, world rotation, scale.
//!
//! "World" is the renderer's f64 frame, which is the body-fixed frame (CLAUDE.md: truth is
//! f64). Layer files store rotations relative to local east, north, up at the position
//! (`layers.md` section 2); [`EnuFrame`] is how the editor converts between the two. The host
//! supplies the ENU basis because only it knows the body's shape and the render axes.

use glam::{DMat3, DQuat, DVec3};

/// Where an object is: body-fixed f64 position, rotation in the world frame, per-axis scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub position: DVec3,
    pub rotation: DQuat,
    pub scale: DVec3,
}

impl Transform {
    pub const IDENTITY: Transform = Transform {
        position: DVec3::ZERO,
        rotation: DQuat::IDENTITY,
        scale: DVec3::ONE,
    };

    pub fn from_position(position: DVec3) -> Self {
        Transform {
            position,
            ..Self::IDENTITY
        }
    }

    /// Largest difference in position (metres), rotation (quaternion components, sign
    /// independent) and scale.
    pub fn max_diff(&self, o: &Transform) -> f64 {
        let dq = (glam::DVec4::from(self.rotation) - glam::DVec4::from(o.rotation))
            .abs()
            .max_element()
            .min(
                (glam::DVec4::from(self.rotation) + glam::DVec4::from(o.rotation))
                    .abs()
                    .max_element(),
            );
        (self.position - o.position)
            .abs()
            .max_element()
            .max(dq)
            .max((self.scale - o.scale).abs().max_element())
    }
}

/// The local east, north, up basis at a position, as the rotation that maps ENU vectors
/// (x = east, y = north, z = up) to world vectors.
pub trait EnuFrame {
    fn enu_to_world(&self, position: DVec3) -> DQuat;
}

/// The proving ground's flat frame: x = east, y = up, z = -north (the render convention).
pub fn flat_y_up_enu() -> DQuat {
    // Columns are where east, north and up land in the world.
    DQuat::from_mat3(&DMat3::from_cols(DVec3::X, DVec3::NEG_Z, DVec3::Y))
}

/// A frame that is the same everywhere.
#[derive(Clone, Copy, Debug)]
pub struct ConstFrame(pub DQuat);

impl EnuFrame for ConstFrame {
    fn enu_to_world(&self, _position: DVec3) -> DQuat {
        self.0
    }
}

/// Spherical body centred at the origin with the rotation axis along `pole`: up is the radial
/// direction, east is `pole x up`. Undefined exactly at the poles (east falls back to any
/// perpendicular).
#[derive(Clone, Copy, Debug)]
pub struct SphereFrame {
    pub pole: DVec3,
}

impl EnuFrame for SphereFrame {
    fn enu_to_world(&self, position: DVec3) -> DQuat {
        let up = position.normalize_or(DVec3::Z);
        let mut east = self.pole.cross(up);
        if east.length_squared() < 1e-24 {
            east = up.any_orthonormal_vector();
        }
        let east = east.normalize();
        let north = up.cross(east);
        DQuat::from_mat3(&DMat3::from_cols(east, north, up))
    }
}

/// World rotation from an ENU-relative rotation at `position`.
pub fn world_rotation(frame: &dyn EnuFrame, position: DVec3, enu_relative: DQuat) -> DQuat {
    (frame.enu_to_world(position) * enu_relative).normalize()
}

/// ENU-relative rotation (what a layer stores) from a world rotation at `position`.
pub fn enu_rotation(frame: &dyn EnuFrame, position: DVec3, world: DQuat) -> DQuat {
    (frame.enu_to_world(position).inverse() * world).normalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_frame_maps_enu_to_render_axes() {
        let q = flat_y_up_enu();
        assert!((q * DVec3::X - DVec3::X).length() < 1e-12);
        assert!((q * DVec3::Y - DVec3::NEG_Z).length() < 1e-12);
        assert!((q * DVec3::Z - DVec3::Y).length() < 1e-12);
    }

    #[test]
    fn rotation_round_trip() {
        let f = SphereFrame { pole: DVec3::Z };
        let p = DVec3::new(4_581_204.118, 381_377.902, 4_407_066.54);
        let r = DQuat::from_rotation_z(0.3) * DQuat::from_rotation_x(-0.2);
        let back = world_rotation(&f, p, enu_rotation(&f, p, r));
        assert!(
            Transform {
                rotation: back,
                ..Transform::IDENTITY
            }
            .max_diff(&Transform {
                rotation: r,
                ..Transform::IDENTITY
            }) < 1e-12
        );
        // Up at p is radial.
        let up = f.enu_to_world(p) * DVec3::Z;
        assert!((up - p.normalize()).length() < 1e-12);
    }
}
