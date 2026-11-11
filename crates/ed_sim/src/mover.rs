//! Character mover: a kinematic capsule moved by Box3D's mover queries
//! (`b3World_CollideMover` for contact planes, `b3World_CastMover` for the swept move) and our
//! port of Box3D's plane solver (`b3SolvePlanes` / `b3ClipVector`, `src/mover.c`).
//!
//! Step height, slope limit and snapping are ours, layered on those queries:
//! - While grounded, the collision capsule is lifted by `step_height`, so ledges lower than that
//!   pass under it and taller ones block it. Steep upward-facing planes are made vertical so the
//!   mover cannot climb them.
//! - After the move a narrower sphere is cast down from the capsule's bottom sphere: a walkable
//!   hit within `step_height + snap_distance` (grounded) puts the feet on it. That one probe is
//!   the step-up, the slope check and the ground snap.
//! - While airborne the capsule is not lifted, gravity integrates the vertical speed, and the
//!   probe only detects landing.
//!
//! Everything runs in Box3D's frame (y up) relative to the feet position, which is f64.

use crate::ffi::{self, B3Capsule, B3Pos, B3Vec3, EdHit, EdPlane};
use crate::frame::{dvec_from_b, pos_from_b, pos_to_b};
use crate::types::{MoverDesc, MoverInput, layers};
use glam::{DVec3, Vec3};

const MAX_PLANES: usize = 32;
const LINEAR_SLOP: f32 = 0.005;
const MOVE_ITERATIONS: usize = 5;
const MOVE_TOLERANCE: f32 = 0.01;
const PROBE_SCALE: f32 = 0.9;
const AIR_PROBE: f32 = 0.05;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MoverRt {
    pub desc: MoverDesc,
    pub input: MoverInput,
    /// Feet, region frame.
    pub feet: DVec3,
    /// Region frame; z is the integrated vertical speed.
    pub velocity: DVec3,
    pub grounded: bool,
    pub ground_normal: DVec3,
}

impl MoverRt {
    pub fn new(desc: MoverDesc) -> Self {
        MoverRt {
            desc,
            input: MoverInput::default(),
            feet: desc.position,
            velocity: DVec3::ZERO,
            grounded: false,
            ground_normal: DVec3::Z,
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Plane {
    n: Vec3,
    offset: f32,
    push: f32,
}

#[inline]
fn v(b: B3Vec3) -> Vec3 {
    Vec3::new(b.x, b.y, b.z)
}

#[inline]
fn b(v: Vec3) -> B3Vec3 {
    B3Vec3 { x: v.x, y: v.y, z: v.z }
}

/// Port of `b3SolvePlanes`: Gauss-Seidel on the plane pushes, rigid planes (no push limit).
fn solve_planes(target: Vec3, planes: &mut [Plane]) -> Vec3 {
    for p in planes.iter_mut() {
        p.push = 0.0;
    }
    let mut delta = target;
    for _ in 0..20 {
        let mut total = 0.0f32;
        for p in planes.iter_mut() {
            let separation = p.n.dot(delta) - p.offset + LINEAR_SLOP;
            let push = -separation;
            let accumulated = p.push;
            p.push = (p.push + push).clamp(0.0, f32::MAX);
            let push = p.push - accumulated;
            delta += push * p.n;
            total += push.abs();
        }
        if total < LINEAR_SLOP {
            break;
        }
    }
    delta
}

/// Port of `b3ClipVector`.
fn clip_vector(mut vel: Vec3, planes: &[Plane]) -> Vec3 {
    for p in planes {
        if p.push == 0.0 {
            continue;
        }
        vel -= vel.dot(p.n).min(0.0) * p.n;
    }
    vel
}

fn collide(w: u32, origin: B3Pos, cap: &B3Capsule, mask: u64, out: &mut [Plane; MAX_PLANES]) -> usize {
    let mut raw = [EdPlane::default(); MAX_PLANES];
    // SAFETY: pointers are to live locals; `raw` holds MAX_PLANES entries and that capacity is
    // passed, so the shim writes at most that many.
    let n = unsafe { ffi::ed_collide_mover(w, &origin, cap, layers::DEFAULT, mask, raw.as_mut_ptr(), MAX_PLANES as i32) };
    let n = (n.max(0) as usize).min(MAX_PLANES);
    for i in 0..n {
        out[i] = Plane { n: v(raw[i].normal), offset: raw[i].offset, push: 0.0 };
    }
    n
}

fn cast_mover(w: u32, origin: B3Pos, cap: &B3Capsule, delta: Vec3, mask: u64) -> f32 {
    let t = b(delta);
    // SAFETY: all pointers are to live locals for the duration of the call.
    unsafe { ffi::ed_cast_mover(w, &origin, cap, &t, layers::DEFAULT, mask) }
}

fn cast_sphere(w: u32, origin: B3Pos, center: Vec3, radius: f32, delta: Vec3, mask: u64) -> Option<EdHit> {
    let p = b(center);
    let t = b(delta);
    let mut hit = EdHit::default();
    // SAFETY: one proxy point, count 1; all pointers are to live locals.
    let r = unsafe { ffi::ed_cast_shape(w, &origin, &p, 1, radius, &t, layers::DEFAULT, mask, &mut hit) };
    (r != 0).then_some(hit)
}

fn cast_ray(w: u32, origin: B3Pos, delta: Vec3, mask: u64) -> Option<EdHit> {
    let t = b(delta);
    let mut hit = EdHit::default();
    // SAFETY: all pointers are to live locals for the duration of the call.
    let r = unsafe { ffi::ed_cast_ray(w, &origin, &t, layers::DEFAULT, mask, &mut hit) };
    (r != 0).then_some(hit)
}

#[inline]
fn offset(p: B3Pos, d: Vec3) -> B3Pos {
    B3Pos { x: p.x + d.x as f64, y: p.y + d.y as f64, z: p.z + d.z as f64 }
}

/// The body capsule, feet at the origin.
pub(crate) fn body_capsule(d: &MoverDesc) -> B3Capsule {
    capsule(d, 0.0)
}

fn capsule(d: &MoverDesc, lift: f32) -> B3Capsule {
    let r = d.radius;
    let top = d.height - r;
    let bottom = (lift + r).min(top);
    B3Capsule { center1: B3Vec3 { x: 0.0, y: bottom, z: 0.0 }, center2: B3Vec3 { x: 0.0, y: top, z: 0.0 }, radius: r }
}

/// Advances one mover by `dt`. Reads only the world's collision and the mover's own state.
pub(crate) fn update(w: u32, m: &mut MoverRt, dt: f32) {
    let d = m.desc;
    let r = d.radius;
    let mask = d.mask & !layers::MOVER;
    let cos_max = libm::cosf(d.max_slope);
    let was_grounded = m.grounded;

    let mut vy = m.velocity.z as f32;
    if was_grounded {
        if m.input.jump_speed > 0.0 {
            vy = m.input.jump_speed;
            m.input.jump_speed = 0.0;
        } else {
            vy = 0.0;
        }
    } else {
        vy -= d.gravity * dt;
    }
    let lifted = was_grounded && vy <= 0.0;
    let lift = if lifted { d.step_height } else { 0.0 };
    let cap = capsule(&d, lift);

    // Region (east, north) to Box3D (x, -z).
    let want = Vec3::new(m.input.velocity.x as f32, vy, -(m.input.velocity.y as f32));
    let target_delta = want * dt;
    let start = pos_to_b(m.feet);
    let target = offset(start, target_delta);
    let mut pos = start;

    let mut planes = [Plane::default(); MAX_PLANES];
    let mut count = 0;
    let mut steep = false;
    for _ in 0..MOVE_ITERATIONS {
        count = collide(w, pos, &cap, mask, &mut planes);
        steep = planes[..count].iter().any(|p| p.n.y >= 0.0 && p.n.y < cos_max);
        if lifted && steep {
            // Grounded: steep slopes are walls.
            for p in &mut planes[..count] {
                if p.n.y >= 0.0 && p.n.y < cos_max {
                    let h = Vec3::new(p.n.x, 0.0, p.n.z);
                    let len = h.length();
                    if len > 1e-4 {
                        p.n = h / len;
                    }
                }
            }
        }
        let remaining = Vec3::new((target.x - pos.x) as f32, (target.y - pos.y) as f32, (target.z - pos.z) as f32);
        let mut delta = solve_planes(remaining, &mut planes[..count]);
        if !lifted && steep {
            // Airborne against a steep slope: it may deflect the mover sideways or down, never
            // lift it (otherwise pushing into the slope would climb it).
            delta.y = delta.y.min(remaining.y.max(0.0));
        }
        let fraction = cast_mover(w, pos, &cap, delta, mask);
        let moved = fraction * delta;
        pos = offset(pos, moved);
        if moved.length_squared() < MOVE_TOLERANCE * MOVE_TOLERANCE {
            break;
        }
    }
    if !lifted {
        // Airborne: hitting a ceiling or landing removes the vertical speed into the surface;
        // a steep slope never adds upward speed.
        let clipped = clip_vector(want, &planes[..count]).y;
        vy = if steep { clipped.min(want.y) } else { clipped };
    }

    // Ground probe from the bottom sphere of the capsule used for the move.
    m.grounded = false;
    if vy <= 0.0 {
        let probe_r = PROBE_SCALE * r;
        let center = Vec3::new(0.0, lift + r, 0.0);
        let extra = if was_grounded { d.snap_distance } else { AIR_PROBE };
        let dist = lift + r - probe_r + extra;
        if let Some(hit) = cast_sphere(w, pos, center, probe_r, Vec3::new(0.0, -dist, 0.0), mask) {
            // On a slope the sphere touches uphill of the feet, and at an edge the cast normal is
            // the edge's. A ray straight down the capsule axis gives the ground under the feet;
            // if that finds nothing near the contact (feet still past a ledge's edge), a ray at
            // the contact gives the face being stepped onto.
            let ray_top = pos.y + center.y as f64;
            let ray_len = (ray_top - hit.point.y) as f32 + 0.1;
            let down = Vec3::new(0.0, -ray_len, 0.0);
            let axis = cast_ray(w, B3Pos { x: pos.x, y: ray_top, z: pos.z }, down, mask);
            let (ground_y, normal) = match axis {
                Some(rh) => (rh.point.y, v(rh.normal)),
                None => match cast_ray(w, B3Pos { x: hit.point.x, y: ray_top, z: hit.point.z }, down, mask) {
                    Some(rh) => (hit.point.y, v(rh.normal)),
                    None => (hit.point.y, v(hit.normal)),
                },
            };
            if normal.y >= cos_max {
                pos.y = ground_y;
                m.grounded = true;
                m.ground_normal = dvec_from_b(b(normal));
                vy = 0.0;
            }
        }
    }
    if !m.grounded {
        m.ground_normal = DVec3::Z;
    }

    let feet = pos_from_b(pos);
    let inv = 1.0 / dt as f64;
    m.velocity = DVec3::new((feet.x - m.feet.x) * inv, (feet.y - m.feet.y) * inv, vy as f64);
    m.feet = feet;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plane_solver_matches_reference_cases() {
        // Touching floor plane: the downward part is removed (to within the slop the solver
        // allows), the sideways part kept.
        let mut p = [Plane { n: Vec3::Y, offset: 0.0, push: 0.0 }];
        let d = solve_planes(Vec3::new(1.0, -1.0, 0.0), &mut p);
        assert!((d.x - 1.0).abs() < 1e-6);
        assert!((d.y + LINEAR_SLOP).abs() < 1e-6, "{d}");
        // Moving away from the plane: no push.
        let d = solve_planes(Vec3::new(0.0, 1.0, 0.0), &mut p);
        assert_eq!(d, Vec3::new(0.0, 1.0, 0.0));
        assert_eq!(p[0].push, 0.0);
        // Clip removes only the into-plane component.
        let mut p = [Plane { n: Vec3::X, offset: 0.0, push: 0.0 }];
        let _ = solve_planes(Vec3::new(-1.0, 0.0, 1.0), &mut p);
        let c = clip_vector(Vec3::new(-3.0, 0.0, 2.0), &p);
        assert_eq!(c, Vec3::new(0.0, 0.0, 2.0));
    }
}
