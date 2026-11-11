//! Region frame (public: x east, y north, z up) to Box3D frame (x east, y up, z south).
//!
//! The map is the rotation `(e, n, u) -> (e, u, -n)`. A quaternion's vector part transforms like
//! a vector under a rotation of frame, and its scalar part is unchanged, so rotations convert by
//! permuting components: exact, no rounding.

use crate::ffi::{B3Pos, B3Quat, B3Vec3};
use glam::{DQuat, DVec3, Quat, Vec3};

#[inline]
pub(crate) fn pos_to_b(v: DVec3) -> B3Pos {
    B3Pos { x: v.x, y: v.z, z: -v.y }
}

#[inline]
pub(crate) fn pos_from_b(p: B3Pos) -> DVec3 {
    DVec3::new(p.x, -p.z, p.y)
}

#[inline]
pub(crate) fn vec_to_b(v: Vec3) -> B3Vec3 {
    B3Vec3 { x: v.x, y: v.z, z: -v.y }
}

#[inline]
pub(crate) fn dvec_to_b(v: DVec3) -> B3Vec3 {
    vec_to_b(v.as_vec3())
}

#[inline]
pub(crate) fn vec_from_b(v: B3Vec3) -> Vec3 {
    Vec3::new(v.x, -v.z, v.y)
}

#[inline]
pub(crate) fn dvec_from_b(v: B3Vec3) -> DVec3 {
    vec_from_b(v).as_dvec3()
}

/// Normalises in f64 before narrowing so the backend gets a unit quaternion.
#[inline]
pub(crate) fn dquat_to_b(q: DQuat) -> B3Quat {
    let q = q.normalize();
    B3Quat { x: q.x as f32, y: q.z as f32, z: -q.y as f32, s: q.w as f32 }
}

#[inline]
pub(crate) fn quat_to_b(q: Quat) -> B3Quat {
    let q = q.normalize();
    B3Quat { x: q.x, y: q.z, z: -q.y, s: q.w }
}

#[inline]
pub(crate) fn dquat_from_b(q: B3Quat) -> DQuat {
    DQuat::from_xyzw(q.x as f64, -q.z as f64, q.y as f64, q.s as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_roundtrip_and_consistency() {
        let q = DQuat::from_axis_angle(DVec3::new(0.3, -0.5, 0.8).normalize(), 0.7);
        let v = DVec3::new(1.0, 2.0, 3.0);
        // Rotate in the public frame, then convert, equals convert then rotate in Box3D frame.
        let a = pos_to_b(q * v);
        let qb = dquat_from_b(dquat_to_b(q)); // round trip through f32
        assert!((qb.dot(q).abs() - 1.0).abs() < 1e-6);
        let qb32 = dquat_to_b(q);
        let qb64 = DQuat::from_xyzw(qb32.x as f64, qb32.y as f64, qb32.z as f64, qb32.s as f64);
        let vb = pos_to_b(v);
        let b = qb64 * DVec3::new(vb.x, vb.y, vb.z);
        assert!((a.x - b.x).abs() < 1e-5 && (a.y - b.y).abs() < 1e-5 && (a.z - b.z).abs() < 1e-5);
        assert_eq!(pos_from_b(pos_to_b(v)), v);
        // Up is +y in Box3D.
        let up = pos_to_b(DVec3::Z);
        assert_eq!((up.x, up.y, up.z), (0.0, 1.0, 0.0));
    }
}
