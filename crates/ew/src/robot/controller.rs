//! The robot's floating-capsule controller (a port of the alpha's `player.rs` walking mode)
//! on an `ed_sim` dynamic body.
//!
//! A capsule carries the body. Two ground rays under the hips measure the ride height; a
//! spring-damper holds the capsule `RIDE_HEIGHT` above the ground, a velocity servo drives it
//! toward the wished velocity, and a torque PD keeps it upright, leaning into acceleration and
//! turns. Everything runs in the fixed step on the sim's own state: no wall clock.
//!
//! Frames: the body is in the sim frame (ENU, z up). The rig's inputs ([`RigFeed`]) are given
//! in the renderer frame (x east, y up, forward -z), where the rig works.

use crate::collide::enu_to_yup;
use crate::game::{GameInput, input_velocity_at};
use ed_sim::{BodyDesc, BodyKind, Entity, Filter, Material, QueryFilter, Shape, ShapeDesc, World, layers, yaw_rotation};
use glam::{DQuat, DVec3, Vec3};

/// Capsule ride height above the ground (pelvis to ground), m.
pub const RIDE_HEIGHT: f64 = 0.95;
pub const CAPSULE_RADIUS: f32 = 0.25;
pub const CAPSULE_HALF_HEIGHT: f32 = 0.24;
/// Half the hip spacing: where the two ground probes sit.
pub const HIP_HALF_W: f64 = 0.13;
pub const MASS: f32 = 80.0;
/// Inertia in body axes (x east, y north, z up); the alpha's (8.5, 3.8, 8.5) was y-up.
pub const INERTIA: Vec3 = Vec3::new(8.5, 8.5, 3.8);
pub const WALK_MPS: f64 = 2.2;
pub const SPRINT_MPS: f64 = 6.0;
/// Walking speed while charging a jump.
pub const CHARGE_MPS: f64 = 1.0;
/// Vertical spring and damper (N/m, N s/m), horizontal servo gains (1/s) and force caps.
const RIDE_K: f64 = 1850.0;
const RIDE_D: f64 = 780.0;
const SERVO_K: f64 = 8.5;
const BRAKE_K: f64 = 11.0;
/// Heading follow rate (1/s).
const TURN_RATE: f64 = 6.5;
const GRAVITY: f64 = 9.80665;

/// What the sim frame hands the rig and the camera each step.
#[derive(Clone, Copy, Debug, Default)]
pub struct RigFeed {
    pub airborne: bool,
    /// Facing yaw in the renderer convention (ccw about +y; 0 faces -z).
    pub yaw: f32,
    pub strafe: f32,
    pub jump_charge: f32,
    pub jump_push_timer: f32,
    pub landing_recovery: f32,
    pub rot_err: Vec3,
    pub accel_smooth: Vec3,
    pub ground_normal: Vec3,
    /// Ground points under the hips (renderer frame, relative to the node-frame origin).
    pub foot_ground_hit: [Option<Vec3>; 2],
    pub vel: Vec3,
}

/// The controller state; the body itself lives in the `World`.
pub struct Controller {
    pub body: Entity,
    /// Facing, radians (ccw about up; equal in the sim and renderer conventions).
    pub yaw: f64,
    /// Pelvis (body centre), sim frame, before and after the last step.
    pub prev: (DVec3, DQuat),
    pub cur: (DVec3, DQuat),
    pub vel: DVec3,
    pub grounded: bool,
    pub ground_dist: Option<f64>,
    jump_charge: f32,
    push_timer: f32,
    push_charge: f32,
    jump_was_held: bool,
    landing_recovery: f32,
    last_v_up: f64,
    prev_vel: DVec3,
    accel_smooth: DVec3,
    rot_err: DVec3,
    strafe: f32,
    ground_normal: Vec3,
    hits: [Option<DVec3>; 2],
}

/// The scenery the controller stands on: everything but characters and vehicles.
fn ground_filter() -> QueryFilter {
    QueryFilter { category: layers::DEFAULT, mask: layers::ALL & !(layers::MOVER | layers::VEHICLE) }
}

fn wrap_pi(a: f64) -> f64 {
    let two = std::f64::consts::TAU;
    (a + std::f64::consts::PI).rem_euclid(two) - std::f64::consts::PI
}

/// Shortest-arc rotation vector (axis * angle) taking `from` to `to`.
fn rotation_error(from: DQuat, to: DQuat) -> DVec3 {
    let mut d = to * from.inverse();
    if d.w < 0.0 {
        d = -d;
    }
    let (axis, angle) = d.to_axis_angle();
    if angle.abs() < 1e-9 || !axis.is_finite() { DVec3::ZERO } else { axis * angle }
}

impl Controller {
    /// A robot standing with its feet at `feet` (sim frame), facing `yaw`.
    pub fn spawn(world: &mut World, feet: DVec3, yaw: f64) -> Result<Controller, String> {
        let pos = feet + DVec3::Z * RIDE_HEIGHT;
        let rot = yaw_rotation(yaw);
        let mut desc = BodyDesc::new(BodyKind::Dynamic, pos).with_rotation(rot).with_shape(
            ShapeDesc::new(Shape::Capsule { half_height: CAPSULE_HALF_HEIGHT, radius: CAPSULE_RADIUS })
                .with_material(Material { density: 1000.0, friction: 0.0, restitution: 0.0 })
                .with_filter(Filter { category: layers::MOVER, mask: layers::ALL }),
        );
        desc.angular_damping = 0.5;
        desc.can_sleep = false;
        let body = world.create_body(desc).map_err(|e| format!("robot: {e}"))?;
        world.set_mass(body, MASS, Vec3::ZERO, INERTIA);
        Ok(Controller {
            body,
            yaw,
            prev: (pos, rot),
            cur: (pos, rot),
            vel: DVec3::ZERO,
            grounded: true,
            ground_dist: Some(RIDE_HEIGHT),
            jump_charge: 0.0,
            push_timer: 0.0,
            push_charge: 0.0,
            jump_was_held: false,
            landing_recovery: 0.0,
            last_v_up: 0.0,
            prev_vel: DVec3::ZERO,
            accel_smooth: DVec3::ZERO,
            rot_err: DVec3::ZERO,
            strafe: 0.0,
            ground_normal: Vec3::Y,
            hits: [None; 2],
        })
    }

    pub fn despawn(self, world: &mut World) {
        world.despawn(self.body);
    }

    /// Feet (sim frame): under the pelvis at the ride height.
    pub fn feet(&self) -> DVec3 {
        self.cur.0 - DVec3::Z * RIDE_HEIGHT
    }

    pub fn feet_at(&self, alpha: f32) -> DVec3 {
        self.prev.0.lerp(self.cur.0, alpha as f64) - DVec3::Z * RIDE_HEIGHT
    }

    pub fn pose(&self, alpha: f32) -> (DVec3, DQuat) {
        (self.prev.0.lerp(self.cur.0, alpha as f64), self.prev.1.slerp(self.cur.1, alpha as f64))
    }

    /// Re-places the body (hard teleport), clearing its motion.
    pub fn place(&mut self, world: &mut World, feet: DVec3, yaw: f64) {
        let pos = feet + DVec3::Z * RIDE_HEIGHT;
        let rot = yaw_rotation(yaw);
        world.set_transform(self.body, pos, rot);
        world.set_velocity(self.body, DVec3::ZERO, DVec3::ZERO);
        self.yaw = yaw;
        self.prev = (pos, rot);
        self.cur = (pos, rot);
        self.vel = DVec3::ZERO;
        self.prev_vel = DVec3::ZERO;
        self.accel_smooth = DVec3::ZERO;
    }

    /// Before the world step: probes, state, forces.
    pub fn pre_step(&mut self, world: &mut World, input: &GameInput, dt: f64) {
        let Some(s) = world.body_state(self.body) else { return };
        let (pos, rot, v, w) = (s.position, s.rotation, s.linear_velocity, s.angular_velocity);
        let mass = MASS as f64;
        let weight = mass * GRAVITY;
        let right = yaw_rotation(self.yaw) * DVec3::X;

        // Ground probes under the hips.
        let reach = RIDE_HEIGHT + 0.55;
        let mut dist_sum = 0.0;
        let mut n_sum = DVec3::ZERO;
        let mut count = 0;
        for (i, side) in [-1.0f64, 1.0].into_iter().enumerate() {
            let o = pos + right * (side * HIP_HALF_W);
            match world.cast_ray(o, -DVec3::Z * reach, ground_filter()) {
                Some(h) => {
                    dist_sum += h.fraction as f64 * reach;
                    n_sum += h.normal.as_dvec3();
                    count += 1;
                    self.hits[i] = Some(h.point);
                }
                None => self.hits[i] = None,
            }
        }
        self.ground_dist = (count > 0).then(|| dist_sum / count as f64);
        let normal = if count > 0 { n_sum.normalize_or(DVec3::Z) } else { DVec3::Z };
        self.ground_normal = enu_to_yup(normal).as_vec3();

        let v_up = v.z;
        let was = self.grounded;
        self.grounded = match self.ground_dist {
            Some(d) if was => d < RIDE_HEIGHT + 0.45 && v_up < 4.0,
            Some(d) => d < RIDE_HEIGHT + 0.18 && v_up < 2.0,
            None => false,
        };
        let v_flat = DVec3::new(v.x, v.y, 0.0);
        let speed_flat = v_flat.length();
        let dtf = dt as f32;

        // Touchdown: crouch and slide a moment after a hard landing.
        if self.grounded && !was {
            let impact = (-self.last_v_up).max(0.0);
            if impact > 5.0 || speed_flat > 6.5 {
                self.landing_recovery = (((impact - 3.5) * 0.12 + speed_flat * 0.06) as f32).clamp(0.45, 1.8);
            }
        }
        self.last_v_up = v_up;
        let has_input = input.axes.length() > 0.05;
        if self.landing_recovery > 0.0 {
            let rate = if speed_flat > 1.8 {
                0.35
            } else if has_input {
                2.2
            } else {
                1.2
            };
            self.landing_recovery = (self.landing_recovery - rate * dtf).max(if speed_flat > 1.8 { 0.25 } else { 0.0 });
        }

        // Jump: hold to crouch, release (or full charge) to push off.
        let held = input.jump;
        if held && self.grounded && self.push_timer <= 0.0 && self.landing_recovery <= 0.2 {
            self.jump_charge = (self.jump_charge + dtf * 3.8).min(1.0);
            if self.jump_charge >= 0.95 {
                self.push_timer = 0.17;
                self.push_charge = self.jump_charge;
                self.jump_charge = 0.0;
            }
        } else if !held && self.jump_was_held && self.jump_charge > 0.12 && self.grounded {
            self.push_timer = 0.14;
            self.push_charge = self.jump_charge;
            self.jump_charge = 0.0;
        } else if !held {
            self.jump_charge = (self.jump_charge - dtf * 6.0).max(0.0);
        }
        self.jump_was_held = held;

        // Vertical: ride-height spring, or the push-off.
        let mut f_up = 0.0;
        if self.push_timer > 0.0 {
            f_up = weight * 3.8 * (0.75 + 0.45 * self.push_charge.max(0.5) as f64);
            self.push_timer -= dtf;
        } else if self.grounded {
            let crouch = if self.jump_charge > 0.0 { self.jump_charge as f64 * 0.34 } else { (self.landing_recovery as f64 * 0.35).min(0.32) };
            let target_h = RIDE_HEIGHT - crouch;
            let h_err = target_h - self.ground_dist.unwrap_or(RIDE_HEIGHT);
            f_up = (weight + h_err * RIDE_K - v_up * RIDE_D).clamp(0.0, weight * 14.0);
        }

        // Horizontal: servo toward the wished velocity; brake with no input.
        let wish = input_velocity_at(input, 1.0, 1.0);
        let crouched = self.jump_charge > 0.1;
        let max_speed = if crouched { CHARGE_MPS } else if input.sprint { SPRINT_MPS } else { WALK_MPS };
        let mut f_h = DVec3::ZERO;
        if self.grounded {
            if self.landing_recovery > 0.1 {
                f_h = clamp_len(-v_flat * mass * 7.5, weight * 0.8);
            } else if has_input {
                let cap = weight * if input.sprint { 0.6 } else { 0.35 };
                f_h = clamp_len((wish * max_speed - v_flat) * mass * SERVO_K, cap);
            } else {
                f_h = clamp_len(-v_flat * mass * BRAKE_K, weight * 0.9);
            }
        }
        world.apply_force_at(self.body, f_h + DVec3::Z * f_up, pos);

        // Facing: toward the wish direction (ccw about up; 0 faces north).
        let prev_yaw = self.yaw;
        if self.landing_recovery <= 0.1 && has_input && wish.length() > 1e-6 {
            let target = (-wish.x).atan2(wish.y);
            self.yaw += wrap_pi(target - self.yaw) * (1.0 - (-TURN_RATE * dt).exp());
        }
        let yaw_vel = wrap_pi(self.yaw - prev_yaw) / dt;
        self.strafe = input.axes.x.clamp(-1.0, 1.0);

        // Upright torque PD: lean into the wish, brake lean, bank into turns.
        let yaw_frame = yaw_rotation(self.yaw);
        let right_now = yaw_frame * DVec3::X;
        let brake = if !has_input || self.landing_recovery > 0.1 { -v_flat * 0.045 } else { DVec3::ZERO };
        let bank = right_now * (-yaw_vel * speed_flat * 0.04 + self.strafe as f64 * 0.20);
        let lean = clamp_len(wish * 0.14 + brake + bank, 0.48);
        let balanced = (DVec3::Z + lean).normalize();
        let target = DQuat::from_rotation_arc(DVec3::Z, balanced) * yaw_frame;
        let err = rotation_error(rot, target);
        self.rot_err = self.rot_err.lerp(err, 1.0 - (-18.0 * dt).exp());
        let wn = if self.grounded { 7.0 } else { 6.0 };
        let (kp, kd) = (8.5 * wn * wn, 2.0 * 8.5 * wn);
        let mut torque = err * kp - w * kd;
        let spin = w.length();
        if spin > 14.0 {
            torque -= w / spin * (spin - 14.0) * 8.5 * 30.0;
        }
        world.apply_torque(self.body, clamp_len(torque, 1800.0));
    }

    /// After the world step: the new pose and the smoothed acceleration.
    pub fn post_step(&mut self, world: &World, dt: f64) {
        self.prev = self.cur;
        if let Some(s) = world.body_state(self.body) {
            self.cur = (s.position, s.rotation);
            self.vel = s.linear_velocity;
            let raw = (s.linear_velocity - self.prev_vel) / dt;
            self.prev_vel = s.linear_velocity;
            self.accel_smooth = self.accel_smooth.lerp(clamp_len(raw, 150.0), 1.0 - (-14.0 * dt).exp());
        }
    }

    /// The rig's inputs, in the renderer frame (relative to the node-frame origin).
    pub fn feed(&self) -> RigFeed {
        // The rotation error's yaw part drives the torso twist: sim z up becomes render y up,
        // and a ccw yaw error stays ccw.
        let to_y = |v: DVec3| enu_to_yup(v).as_vec3();
        RigFeed {
            airborne: !self.grounded,
            yaw: self.yaw as f32,
            strafe: self.strafe,
            jump_charge: self.jump_charge,
            jump_push_timer: self.push_timer.max(0.0),
            landing_recovery: self.landing_recovery,
            rot_err: to_y(self.rot_err),
            accel_smooth: to_y(self.accel_smooth),
            ground_normal: self.ground_normal,
            foot_ground_hit: [self.hits[0].map(|p| to_y(p)), self.hits[1].map(|p| to_y(p))],
            vel: to_y(self.vel),
        }
    }

    pub fn crouching(&self) -> bool {
        self.jump_charge > 0.1
    }
}

fn clamp_len(v: DVec3, max: f64) -> DVec3 {
    let l = v.length();
    if l > max { v * (max / l) } else { v }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed_sim::{SimConfig, SimInput};
    use glam::Vec2;

    /// A big flat static slab, top at z = 0.
    fn world_with_floor() -> World {
        let mut w = World::new(&SimConfig::default());
        let floor = BodyDesc::new(BodyKind::Static, DVec3::new(0.0, 0.0, -1.0)).with_shape(ShapeDesc::cuboid(Vec3::new(200.0, 200.0, 1.0)));
        w.create_body(floor).unwrap();
        w
    }

    fn step(w: &mut World) {
        let tick = w.tick();
        w.step(&SimInput { tick, commands: &[], utc: 0.0 });
    }

    fn tick(w: &mut World, c: &mut Controller, input: &GameInput) {
        c.pre_step(w, input, ed_sim::DT);
        step(w);
        c.post_step(w, ed_sim::DT);
    }

    #[test]
    fn stands_at_ride_height_and_stays_put() {
        let mut w = world_with_floor();
        let mut c = Controller::spawn(&mut w, DVec3::new(0.0, 0.0, 0.3), 0.0).unwrap();
        for _ in 0..240 {
            tick(&mut w, &mut c, &GameInput::default());
        }
        assert!(c.grounded);
        assert!((c.cur.0.z - RIDE_HEIGHT).abs() < 0.03, "z {}", c.cur.0.z);
        assert!(c.cur.0.truncate().length() < 0.05);
    }

    #[test]
    fn walks_at_the_alpha_speed_and_sprints_faster() {
        let mut w = world_with_floor();
        let mut c = Controller::spawn(&mut w, DVec3::ZERO, 0.0).unwrap();
        let fwd = GameInput { axes: Vec2::new(0.0, 1.0), ..Default::default() };
        for _ in 0..180 {
            tick(&mut w, &mut c, &fwd);
        }
        let v = c.vel.truncate().length();
        assert!((v - WALK_MPS).abs() < 0.3, "walk {v}");
        assert!(c.cur.0.y > 3.0, "walks north: {:?}", c.cur.0);
        let run = GameInput { sprint: true, ..fwd };
        for _ in 0..240 {
            tick(&mut w, &mut c, &run);
        }
        assert!(c.vel.truncate().length() > 4.5, "sprint {}", c.vel.truncate().length());
        assert!(c.grounded);
    }

    #[test]
    fn jump_leaves_the_ground_and_lands() {
        let mut w = world_with_floor();
        let mut c = Controller::spawn(&mut w, DVec3::ZERO, 0.0).unwrap();
        for _ in 0..60 {
            tick(&mut w, &mut c, &GameInput::default());
        }
        let hold = GameInput { jump: true, ..Default::default() };
        for _ in 0..40 {
            tick(&mut w, &mut c, &hold);
        }
        let mut peak = 0.0f64;
        for _ in 0..90 {
            tick(&mut w, &mut c, &GameInput::default());
            peak = peak.max(c.cur.0.z);
        }
        assert!(peak > RIDE_HEIGHT + 0.25, "peak {peak}");
        for _ in 0..120 {
            tick(&mut w, &mut c, &GameInput::default());
        }
        assert!(c.grounded && (c.cur.0.z - RIDE_HEIGHT).abs() < 0.05);
    }

    #[test]
    fn deterministic() {
        let run = || {
            let mut w = world_with_floor();
            let mut c = Controller::spawn(&mut w, DVec3::ZERO, 0.3).unwrap();
            for k in 0..300 {
                let i = GameInput { axes: Vec2::new(((k / 50) % 3) as f32 - 1.0, 1.0), jump: k % 97 < 30, sprint: k > 150, yaw: 0.2 };
                tick(&mut w, &mut c, &i);
            }
            (c.cur.0, c.yaw)
        };
        assert_eq!(run(), run());
    }
}
