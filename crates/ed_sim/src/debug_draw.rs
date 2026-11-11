//! Debug geometry as plain lists in the region frame, for any renderer to draw. Built from our
//! own shape descriptions and the current body poses; no Box3D draw callbacks.

use crate::types::{BodyKind, Geometry, MoverDesc, Shape};
use crate::world::{Payload, World};
use glam::{DQuat, DVec3, Vec3};

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Rgba(pub [u8; 4]);

impl Rgba {
    pub const STATIC: Rgba = Rgba([128, 128, 128, 255]);
    pub const KINEMATIC: Rgba = Rgba([90, 160, 230, 255]);
    pub const DYNAMIC: Rgba = Rgba([230, 170, 60, 255]);
    pub const SLEEPING: Rgba = Rgba([150, 120, 80, 255]);
    pub const MOVER: Rgba = Rgba([80, 220, 120, 255]);
    pub const MOVER_GROUNDED: Rgba = Rgba([40, 255, 60, 255]);
}

#[derive(Clone, Default, Debug)]
pub struct DebugDraw {
    /// Segment endpoints, region frame.
    pub lines: Vec<([DVec3; 2], Rgba)>,
    pub triangles: Vec<([DVec3; 3], Rgba)>,
}

const CIRCLE_SEGMENTS: usize = 16;

impl DebugDraw {
    fn line(&mut self, a: DVec3, b: DVec3, c: Rgba) {
        self.lines.push(([a, b], c));
    }

    fn circle(&mut self, center: DVec3, u: DVec3, v: DVec3, r: f64, c: Rgba) {
        let mut prev = center + u * r;
        for i in 1..=CIRCLE_SEGMENTS {
            let a = i as f64 / CIRCLE_SEGMENTS as f64 * std::f64::consts::TAU;
            let p = center + (u * libm::cos(a) + v * libm::sin(a)) * r;
            self.line(prev, p, c);
            prev = p;
        }
    }

    fn capsule(&mut self, a: DVec3, b: DVec3, r: f64, c: Rgba) {
        let axis = (b - a).try_normalize().unwrap_or(DVec3::Z);
        let u = axis.any_orthonormal_vector();
        let v = axis.cross(u);
        for p in [a, b] {
            self.circle(p, u, v, r, c);
            self.circle(p, u, axis, r, c);
            self.circle(p, v, axis, r, c);
        }
        for d in [u, -u, v, -v] {
            self.line(a + d * r, b + d * r, c);
        }
    }

    /// Everything in the world.
    pub fn from_world(world: &World) -> DebugDraw {
        let mut dd = DebugDraw::default();
        for (e, st) in world.body_states() {
            let Some(kind) = world.body_kind(e) else { continue };
            let slot = &world.slots()[e.index() as usize];
            let Some(live) = &slot.live else { continue };
            match &live.payload {
                Payload::Mover(m) => {
                    let col = if m.grounded { Rgba::MOVER_GROUNDED } else { Rgba::MOVER };
                    dd.mover(&m.desc, m.feet, col);
                }
                Payload::Body(desc) => {
                    let col = match kind {
                        BodyKind::Static => Rgba::STATIC,
                        BodyKind::Kinematic => Rgba::KINEMATIC,
                        BodyKind::Dynamic => {
                            if world.is_awake(e) == Some(true) {
                                Rgba::DYNAMIC
                            } else {
                                Rgba::SLEEPING
                            }
                        }
                    };
                    for s in &desc.shapes {
                        dd.shape(world, &s.shape, s.offset, s.rotation, st.position, st.rotation, col);
                    }
                }
            }
        }
        dd
    }

    fn mover(&mut self, d: &MoverDesc, feet: DVec3, c: Rgba) {
        let r = d.radius as f64;
        let a = feet + DVec3::Z * r;
        let b = feet + DVec3::Z * (d.height as f64 - r);
        self.capsule(a, b, r, c);
    }

    #[allow(clippy::too_many_arguments)]
    fn shape(&mut self, world: &World, s: &Shape, off: Vec3, rot: glam::Quat, p: DVec3, q: DQuat, c: Rgba) {
        let local = |v: Vec3| -> DVec3 { p + q * (off.as_dvec3() + rot.as_dquat() * v.as_dvec3()) };
        let world_pt = |v: Vec3| -> DVec3 { p + q * v.as_dvec3() };
        match *s {
            Shape::Cuboid { half_extents: h } => {
                let corner = |i: usize| {
                    local(Vec3::new(
                        if i & 1 == 0 { -h.x } else { h.x },
                        if i & 2 == 0 { -h.y } else { h.y },
                        if i & 4 == 0 { -h.z } else { h.z },
                    ))
                };
                for i in 0..8usize {
                    for bit in [1usize, 2, 4] {
                        if i & bit == 0 {
                            self.line(corner(i), corner(i | bit), c);
                        }
                    }
                }
            }
            Shape::Sphere { radius } => {
                let ctr = local(Vec3::ZERO);
                let r = radius as f64;
                self.circle(ctr, q * DVec3::X, q * DVec3::Y, r, c);
                self.circle(ctr, q * DVec3::X, q * DVec3::Z, r, c);
                self.circle(ctr, q * DVec3::Y, q * DVec3::Z, r, c);
            }
            Shape::Capsule { half_height, radius } => {
                let a = local(Vec3::new(0.0, 0.0, -half_height));
                let b = local(Vec3::new(0.0, 0.0, half_height));
                self.capsule(a, b, radius as f64, c);
            }
            Shape::Hull(g) => {
                if let Some(e) = world.geom_entry(g) {
                    for [a, b] in &e.hull_edges {
                        self.line(local(*a), local(*b), c);
                    }
                }
            }
            Shape::Mesh(g) => {
                if let Some(e) = world.geom_entry(g)
                    && let Geometry::Mesh(m) = &e.geom
                {
                    for t in &m.triangles {
                        let v = t.map(|i| world_pt(m.vertices[i as usize]));
                        self.triangles.push((v, c));
                    }
                }
            }
            Shape::HeightField(g) => {
                if let Some(e) = world.geom_entry(g)
                    && let Geometry::HeightField(h) = &e.geom
                {
                    let pt = |col: u32, row: u32| {
                        world_pt(Vec3::new(col as f32 * h.spacing[0], -(row as f32) * h.spacing[1], h.at(col, row)))
                    };
                    for row in 0..h.rows - 1 {
                        for col in 0..h.columns - 1 {
                            let (a, b, cc, d) = (pt(col, row), pt(col + 1, row), pt(col + 1, row + 1), pt(col, row + 1));
                            // Counter-clockwise seen from above (z up).
                            self.triangles.push(([a, d, b], c));
                            self.triangles.push(([b, d, cc], c));
                        }
                    }
                }
            }
        }
    }
}
