//! Collision shapes for packs (M4b): `collision_asset` (kind 23) per mesh or prefab, in the
//! asset's own frame, and the proving node's `collision` (kind 6), every placed shape in the
//! node frame (ENU at the proving origin). The game only converts these to simulation
//! bodies; it never derives collision from render geometry itself.
//!
//! Frames: an instance transform ([`Transform`], ENU node frame) maps the asset's local
//! coordinates straight into the node frame (see [`crate::proving::to_enu`]), so placing a
//! shape is applying that transform to it ([`place`]).

use ed_pack::kinds::{CollisionShape, Shape, Transform};
use glam::{Affine3A, Quat, Vec3};

/// Surface class written into [`CollisionShape::surface`] (low byte; the rest are flags).
pub mod surface {
    pub const GROUND: u32 = 1;
    pub const BUILDING: u32 = 2;
    pub const PROP: u32 = 3;
}

/// Axis-aligned bounds of `points`, `None` when empty or not finite.
pub fn bounds(points: impl IntoIterator<Item = Vec3>) -> Option<(Vec3, Vec3)> {
    let mut lo = Vec3::splat(f32::INFINITY);
    let mut hi = Vec3::splat(f32::NEG_INFINITY);
    let mut any = false;
    for p in points {
        lo = lo.min(p);
        hi = hi.max(p);
        any = true;
    }
    (any && lo.is_finite() && hi.is_finite()).then_some((lo, hi))
}

/// A box shape covering `lo..hi` (axis-aligned in the frame of the points).
pub fn aabb_box(lo: Vec3, hi: Vec3, surface: u32) -> CollisionShape {
    let c = (lo + hi) * 0.5;
    let h = ((hi - lo) * 0.5).max(Vec3::splat(0.01));
    CollisionShape { shape: Shape::Box { centre: c.to_array(), half_extents: h.to_array(), rotation: [0.0, 0.0, 0.0, 1.0] }, surface }
}

/// `shape` (asset frame) moved by `t` into the frame `t` maps to. Scale is applied in the
/// asset frame: exact for boxes whose rotation is the identity in the asset frame (all the
/// baked ones), spheres take the largest scale axis, capsules scale their radius likewise.
pub fn place(shape: &CollisionShape, t: &Transform) -> CollisionShape {
    let s = Vec3::from(t.scale);
    let r = Quat::from_array(t.rotation);
    let a = Affine3A::from_scale_rotation_translation(s, r, Vec3::from(t.translation));
    let pt = |p: [f32; 3]| a.transform_point3(Vec3::from(p)).to_array();
    let smax = s.abs().max_element();
    let out = match &shape.shape {
        Shape::Box { centre, half_extents, rotation } => {
            let q = Quat::from_array(*rotation);
            Shape::Box { centre: pt(*centre), half_extents: (Vec3::from(*half_extents) * s.abs()).to_array(), rotation: (r * q).normalize().to_array() }
        }
        Shape::Sphere { centre, radius } => Shape::Sphere { centre: pt(*centre), radius: radius * smax },
        Shape::Capsule { a: p, b, radius } => Shape::Capsule { a: pt(*p), b: pt(*b), radius: radius * smax },
        Shape::ConvexHull { points } => Shape::ConvexHull { points: points.iter().map(|p| pt(*p)).collect() },
        Shape::TriMesh { vertices, triangles } => Shape::TriMesh { vertices: vertices.iter().map(|p| pt(*p)).collect(), triangles: triangles.clone() },
        Shape::HeightField { origin, cell_size, columns, rows, heights } => {
            // Heightfields only translate (no rotation or scale in the node frame).
            Shape::HeightField { origin: pt(*origin), cell_size: *cell_size, columns: *columns, rows: *rows, heights: heights.clone() }
        }
    };
    CollisionShape { shape: out, surface: shape.surface }
}

/// Centre of a shape (for spatial grouping).
pub fn centre(s: &Shape) -> Vec3 {
    match s {
        Shape::Box { centre, .. } | Shape::Sphere { centre, .. } => Vec3::from(*centre),
        Shape::Capsule { a, b, .. } => (Vec3::from(*a) + Vec3::from(*b)) * 0.5,
        Shape::ConvexHull { points } => bounds(points.iter().map(|p| Vec3::from(*p))).map(|(l, h)| (l + h) * 0.5).unwrap_or(Vec3::ZERO),
        Shape::TriMesh { vertices, .. } => bounds(vertices.iter().map(|p| Vec3::from(*p))).map(|(l, h)| (l + h) * 0.5).unwrap_or(Vec3::ZERO),
        Shape::HeightField { origin, .. } => Vec3::from(*origin),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proving::to_enu;

    #[test]
    fn placing_a_box_matches_transforming_its_corners() {
        let b = aabb_box(Vec3::new(-0.5, 0.0, -0.5), Vec3::new(0.5, 1.0, 0.5), surface::BUILDING);
        // A y-up placement: 20 x 30 x 10 building at (100, 0, -40), turned 30 degrees.
        let a = Affine3A::from_scale_rotation_translation(Vec3::new(20.0, 30.0, 10.0), Quat::from_rotation_y(0.5), Vec3::new(100.0, 0.0, -40.0));
        let t = to_enu(&a);
        let p = place(&b, &t);
        assert_eq!(p.surface, surface::BUILDING);
        let Shape::Box { centre, half_extents, rotation } = p.shape else { panic!() };
        // The box top centre in y-up is (100, 30, -40) -> ENU (100, 40, 30).
        let q = Quat::from_array(rotation);
        let top = Vec3::from(centre) + q * Vec3::new(0.0, half_extents[1], 0.0);
        assert!(top.abs_diff_eq(Vec3::new(100.0, 40.0, 30.0), 1e-3), "{top}");
        assert!((half_extents[0] - 10.0).abs() < 1e-5 && (half_extents[2] - 5.0).abs() < 1e-5);
    }
}
