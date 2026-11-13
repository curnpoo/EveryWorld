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

use crate::collide::{enu_to_yup, yup_to_enu};
use super::flight::{FlightIn, FlightPose, FlightState, MoveMode};
use ed_planet::Anchor;
use crate::game::{GameInput, input_velocity_at};
use ed_sim::{BodyDesc, BodyKind, Entity, Filter, Material, QueryFilter, Shape, ShapeDesc, World, layers, yaw_rotation};
use glam::{DQuat, DVec3, Quat, Vec3};

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
    pub fl: FlightPose,
}

/// Where the body is relative to the planet, for altitude and height above ground. Set by the
/// caller each frame (the sim frame knows neither).
#[derive(Clone, Debug, Default)]
pub struct FlightEnv {
    pub anchor: Option<Anchor>,
    /// Terrain height under the body (m above the ellipsoid), when resident.
    pub ground_alt: Option<f64>,
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
    pub fl: FlightState,
    /// Look pitch (renderer convention: positive looks up) and the flight environment.
    pub cam_pitch: f32,
    pub env: FlightEnv,
    /// Edge-triggered keys, consumed by the next step: G nudge.
    pub nudge: bool,
    /// Height above the ground the last step measured (m).
    pub agl: f32,
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
            fl: FlightState::default(),
            cam_pitch: 0.0,
            env: FlightEnv::default(),
            nudge: false,
            agl: 0.0,
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
        let wish = input_velocity_at(input, 1.0, 1.0);
        if self.landing_recovery > 0.0 && !self.grounded {
            self.landing_recovery = 0.0;
        } else if self.landing_recovery > 0.0 {
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

        // Flight: edge keys, then the mode (rocket, skydive, parachute...) from the air state.
        let to_y = |v: DVec3| enu_to_yup(v).as_vec3();
        let alt = self.altitude(pos);
        self.agl = if self.grounded {
            self.ground_dist.unwrap_or(RIDE_HEIGHT) as f32
        } else {
            const AGL_REACH: f64 = 120.0;
            match world.cast_ray(pos, -DVec3::Z * AGL_REACH, ground_filter()) {
                Some(h) => (h.fraction as f64 * AGL_REACH) as f32,
                None => match self.env.ground_alt {
                    Some(g) => ((alt - g) as f32).max(AGL_REACH as f32),
                    None => (alt as f32).max(AGL_REACH as f32),
                },
            }
        };
        let (fwd_in, strafe_in) = (input.axes.y.clamp(-1.0, 1.0), input.axes.x.clamp(-1.0, 1.0));
        let wish_r = to_y(wish).normalize_or_zero();
        let mut fin = FlightIn {
            rot: to_render_rot(rot),
            v: to_y(v),
            omega: to_y(w),
            alt,
            agl: self.agl,
            grounded: self.grounded,
            jump_push_timer: self.push_timer,
            fwd_in,
            strafe: strafe_in,
            cam_yaw: input.yaw,
            cam_pitch: self.cam_pitch,
            space_held: input.jump,
            shift: input.sprint,
            accel_smooth: to_y(self.accel_smooth),
            wish: wish_r,
            yaw: self.yaw as f32,
            yaw_vel: 0.0,
        };
        self.fl.update_mode(&fin, dtf);
        let mode = self.fl.mode;
        let sprint = input.sprint && self.grounded;

        // Physical test nudge (G): a 3.2 m/s kick at the chest along the wish or the view.
        if std::mem::take(&mut self.nudge) {
            let dir = if wish.length() > 0.01 {
                wish
            } else {
                let (s, c) = input.yaw.sin_cos();
                yup_to_enu(DVec3::new(-s as f64, 0.0, -c as f64))
            };
            let chest = pos + rot * DVec3::Z * 0.35;
            world.apply_impulse_at(self.body, dir * (mass * 3.2), chest);
        }

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
        let crouched = self.jump_charge > 0.1;
        let max_speed = if crouched { CHARGE_MPS } else if sprint { SPRINT_MPS } else { WALK_MPS };
        let mut f_h = DVec3::ZERO;
        if self.grounded {
            if self.landing_recovery > 0.1 {
                f_h = clamp_len(-v_flat * mass * 7.5, weight * 0.8);
            } else if has_input {
                let cap = weight * if sprint { 0.6 } else { 0.35 };
                f_h = clamp_len((wish * max_speed - v_flat) * mass * SERVO_K, cap);
            } else {
                f_h = clamp_len(-v_flat * mass * BRAKE_K, weight * 0.9);
            }
        }

        // Facing: toward the wish on the ground (ccw about up; 0 faces north), toward the view
        // in the air modes.
        let prev_yaw = self.yaw;
        if mode == MoveMode::Walk {
            if self.landing_recovery <= 0.1 && has_input && wish.length() > 1e-6 {
                let target = (-wish.x).atan2(wish.y);
                self.yaw += wrap_pi(target - self.yaw) * (1.0 - (-TURN_RATE * dt).exp());
            }
        } else if let Some((target, rate)) = self.fl.yaw_target(input.yaw, has_input) {
            self.yaw += wrap_pi(target as f64 - self.yaw) * (1.0 - (-(rate as f64) * dt).exp());
        }
        let yaw_vel = wrap_pi(self.yaw - prev_yaw) / dt;
        self.strafe = input.axes.x.clamp(-1.0, 1.0);
        fin.yaw = self.yaw as f32;
        fin.yaw_vel = yaw_vel as f32;

        // Posture target: the walking lean (upright, leaning into the wish, brake and turn), or
        // the flight posture (rocket attitude, skydive, fall, parachute).
        let yaw_frame = yaw_rotation(self.yaw);
        let right_now = yaw_frame * DVec3::X;
        let brake = if !has_input || self.landing_recovery > 0.1 { -v_flat * 0.045 } else { DVec3::ZERO };
        let bank = right_now * (-yaw_vel * speed_flat * 0.04 + self.strafe as f64 * 0.20);
        let lean = clamp_len(wish * 0.14 + brake + bank, 0.48);
        let balanced = (DVec3::Z + lean).normalize();
        let mut target = DQuat::from_rotation_arc(DVec3::Z, balanced) * yaw_frame;
        let mut muscle = 1.0;
        let mut f_air = DVec3::ZERO;
        let mut t_air = DVec3::ZERO;
        let out = self.fl.forces(&fin, dtf);
        f_air += yup_to_enu(out.force.as_dvec3());
        t_air += yup_to_enu(out.torque.as_dvec3());
        for (off, f) in out.points {
            if f.length_squared() > 0.0 {
                world.apply_force_at(self.body, yup_to_enu(f.as_dvec3()), pos + yup_to_enu(off.as_dvec3()));
            }
        }
        if let Some(t) = out.target {
            target = to_sim_rot(t);
        }
        if mode != MoveMode::Walk || out.target.is_some() {
            muscle = out.muscle as f64;
        }
        world.apply_force_at(self.body, f_h + DVec3::Z * f_up + f_air, pos);

        // Upright torque PD, sized to the body's inertia.
        let err = rotation_error(rot, target);
        self.rot_err = self.rot_err.lerp(err, 1.0 - (-18.0 * dt).exp());
        let wn = if self.grounded { 7.0 } else { 6.0 };
        let (kp, kd) = (8.5 * wn * wn, 2.0 * 8.5 * wn);
        let mut torque = (err * kp - w * kd) * muscle + t_air;
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
            fl: self.fl.pose(),
        }
    }

    /// Height above the ellipsoid of a sim-frame point (the anchor's, else the sim z).
    pub fn altitude(&self, pos: DVec3) -> f64 {
        match &self.env.anchor {
            Some(a) => {
                let b = a.sim_to_body(pos);
                b.length() - a.body.radius(b.normalize())
            }
            None => pos.z,
        }
    }

    pub fn mode(&self) -> MoveMode {
        self.fl.mode
    }

    /// F: wingsuit on or off.
    pub fn toggle_wingsuit(&mut self) {
        self.fl.toggle_wingsuit();
    }

    /// P: parachute on or off (in the air).
    pub fn toggle_parachute(&mut self) {
        self.fl.toggle_parachute();
    }

    /// Camera shake 0..1 (suit buffet, Mach crossing).
    pub fn shake(&self) -> f32 {
        self.fl.shake
    }

    pub fn crouching(&self) -> bool {
        self.jump_charge > 0.1
    }
}

/// Sim-frame rotation to the renderer frame and back (conjugation by the proper rotation
/// (x, y, z) -> (x, z, -y)).
pub fn to_render_rot(q: DQuat) -> Quat {
    DQuat::from_xyzw(q.x, q.z, -q.y, q.w).as_quat()
}

fn to_sim_rot(q: Quat) -> DQuat {
    DQuat::from_xyzw(q.x as f64, -q.z as f64, q.y as f64, q.w as f64).normalize()
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

    fn spawn_at(z: f64) -> (World, Controller) {
        let mut w = world_with_floor();
        let c = Controller::spawn(&mut w, DVec3::new(0.0, 0.0, z), 0.0).unwrap();
        (w, c)
    }

    #[test]
    fn holding_space_in_the_air_lights_the_rocket_and_climbs() {
        let (mut w, mut c) = spawn_at(0.0);
        for _ in 0..60 {
            tick(&mut w, &mut c, &GameInput::default());
        }
        let hold = GameInput { jump: true, ..Default::default() };
        let mut rocket = false;
        for _ in 0..240 {
            tick(&mut w, &mut c, &hold);
            rocket |= c.mode() == MoveMode::RocketShoes;
        }
        assert!(rocket, "mode {:?}", c.mode());
        assert!(c.cur.0.z > 15.0 && c.vel.z > 2.0, "z {} vz {}", c.cur.0.z, c.vel.z);
        assert!(c.fl.throttle > 0.0);
    }

    fn fall_from(z: f64, chute_after: Option<usize>, ticks: usize) -> (World, Controller, bool, f64) {
        let (mut w, mut c) = spawn_at(z);
        let mut skydived = false;
        let mut peak = 0.0f64;
        for k in 0..ticks {
            if chute_after == Some(k) {
                c.toggle_parachute();
            }
            tick(&mut w, &mut c, &GameInput::default());
            skydived |= c.mode() == MoveMode::Skydive;
            peak = peak.max(-c.vel.z);
        }
        (w, c, skydived, peak)
    }

    #[test]
    fn a_long_fall_becomes_a_skydive_and_the_parachute_is_slower() {
        // The wingsuit glides (it trades the dive for forward speed), so the skydive is compared
        // by its peak sink rate and the parachute by its steady one.
        let (_w, dive, skydived, vd) = fall_from(3000.0, None, 60 * 12);
        assert!(skydived, "mode {:?}", dive.mode());
        let (_w, chute, _, _) = fall_from(3000.0, Some(60 * 3), 60 * 12);
        assert_eq!(chute.mode(), MoveMode::Parachute);
        let vc = -chute.vel.z;
        assert!(vd > 25.0, "skydive {vd}");
        assert!(vc < vd * 0.5 && vc > 3.0, "parachute {vc} vs skydive {vd}");
    }

    #[test]
    fn parachute_lands_back_in_walk() {
        let (mut w, mut c) = spawn_at(100.0);
        for k in 0..60 * 40 {
            if k == 30 {
                c.toggle_parachute();
            }
            tick(&mut w, &mut c, &GameInput::default());
        }
        assert!(c.grounded, "z {}", c.cur.0.z);
        assert_eq!(c.mode(), MoveMode::Walk);
    }
}
