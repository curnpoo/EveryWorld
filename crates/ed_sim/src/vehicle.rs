//! Raycast-wheel car on a dynamic box body (P-slice S5).
//!
//! The chassis is an ordinary dynamic Box3D body (category [`layers::VEHICLE`]); the wheels
//! are not bodies. Each tick, before [`World::step`], [`Vehicle::update`] casts one ray per
//! wheel down the suspension axis and applies, at the wheel, a spring-damper force along the
//! body's up axis plus tyre forces in the ground plane:
//!
//! * lateral: a slip-angle curve (linear to `slip_peak`, then falling to 80 % while sliding),
//!   never more than what cancels the sideways velocity this tick (no low-speed jitter);
//! * longitudinal: engine torque through automatic gears, brakes, handbrake (rear lock, rear
//!   grip loss), rolling resistance, and a hold that keeps a stopped car still on a slope;
//! * both clamped to a friction circle of `grip x load`.
//!
//! Anti-roll bars move load across each axle. Steering has a speed-dependent lock and eases
//! back to centre. Car frame: x right, y forward, z up; the body's origin is the chassis box
//! centre. Everything is f64 from the body state and its own fields, transcendental functions
//! through `libm`, so the update is a pure function of (world state, vehicle state, input).
//!
//! The vehicle's own state (steer angle, gear, wheel compression and spin) lives here, not in
//! world snapshots; a caller that snapshots the world keeps a copy of the [`Vehicle`] with it.

use crate::types::{BodyDesc, BodyKind, Entity, Filter, Material, QueryFilter, ShapeDesc, layers};
use crate::world::World;
use crate::{DT, SimError};
use glam::{DQuat, DVec3, Vec3};

/// Everything that defines a car. Car frame: x right, y forward, z up, metres.
#[derive(Clone, Debug, PartialEq)]
pub struct VehicleDesc {
    /// kg.
    pub mass: f64,
    /// Chassis collision box half extents (centred on the body origin).
    pub body_half: Vec3,
    /// Cabin collision box half extents and centre.
    pub cabin_half: Vec3,
    pub cabin_offset: Vec3,
    /// Centre of mass, car frame.
    pub com: Vec3,
    /// Principal moments of inertia (car axes), kg m^2.
    pub inertia: Vec3,
    pub wheel_radius: f64,
    /// Wheel mounts: half track (x), front and rear axle (y), mount height (z, top of travel).
    pub half_track: f64,
    pub front_y: f64,
    pub rear_y: f64,
    pub mount_z: f64,
    /// Suspension travel below the mount, m.
    pub rest_length: f64,
    /// Corner natural frequency (Hz) and damping ratio.
    pub spring_hz: f64,
    pub damping_ratio: f64,
    /// Rebound (extension) damping over bump damping: keeps a car that crests a rise from
    /// being flung nose-down by its own springs.
    pub rebound: f64,
    /// Pitch and roll damping while airborne (1/s): a light hand that keeps jumps level-ish.
    pub air_damping: f64,
    /// Anti-roll stiffness per axle, N per metre of compression difference.
    pub anti_roll: f64,
    /// Tyre friction coefficient and the slip angle of peak lateral force (rad).
    pub grip: f64,
    pub slip_peak: f64,
    /// Steering lock at standstill (rad), the speed (m/s) at which it halves, and the steer
    /// and return rates (rad/s).
    pub max_steer: f64,
    pub steer_half_speed: f64,
    pub steer_rate: f64,
    pub return_rate: f64,
    /// Engine: peak torque (N m) at `rpm_peak`, idle, red line.
    pub torque_peak: f64,
    pub rpm_idle: f64,
    pub rpm_peak: f64,
    pub rpm_max: f64,
    /// Forward gear ratios, reverse ratio, final drive, drivetrain efficiency.
    pub gears: Vec<f64>,
    pub reverse: f64,
    pub final_drive: f64,
    pub efficiency: f64,
    /// Up- and downshift engine speeds and the shift time (no drive while shifting), s.
    pub shift_up_rpm: f64,
    pub shift_down_rpm: f64,
    pub shift_s: f64,
    /// Share of drive torque on the front axle (0 rear drive, 1 front drive).
    pub front_drive: f64,
    /// Total brake force (N) and its front share; handbrake force per rear wheel (N).
    pub brake_force: f64,
    pub brake_front: f64,
    pub handbrake_force: f64,
    /// Rear lateral grip multiplier while the handbrake is held.
    pub handbrake_grip: f64,
    /// Reverse speed limit, m/s.
    pub reverse_max: f64,
    /// 0.5 rho Cd A (N per (m/s)^2) and the rolling resistance coefficient.
    pub drag: f64,
    pub rolling: f64,
}

impl Default for VehicleDesc {
    /// A placeholder hatchback: 1500 kg, 4.4 x 1.8 x 1.4 m, 150 kW, rear-biased all-wheel drive.
    fn default() -> Self {
        let mass = 1500.0;
        let (w, l, h) = (1.8f32, 4.4f32, 1.0f32);
        let k = mass as f32 / 12.0;
        VehicleDesc {
            mass,
            body_half: Vec3::new(0.9, 2.2, 0.25),
            cabin_half: Vec3::new(0.78, 1.1, 0.28),
            cabin_offset: Vec3::new(0.0, -0.2, 0.53),
            com: Vec3::new(0.0, 0.05, -0.12),
            inertia: Vec3::new(k * (l * l + h * h), k * (w * w + h * h), k * (w * w + l * l)),
            wheel_radius: 0.33,
            half_track: 0.78,
            front_y: 1.38,
            rear_y: -1.32,
            mount_z: -0.05,
            rest_length: 0.30,
            spring_hz: 1.6,
            damping_ratio: 0.45,
            rebound: 1.8,
            air_damping: 1.5,
            anti_roll: 18_000.0,
            grip: 1.05,
            slip_peak: 0.14,
            max_steer: 0.55,
            steer_half_speed: 16.0,
            steer_rate: 2.4,
            return_rate: 4.0,
            torque_peak: 320.0,
            rpm_idle: 900.0,
            rpm_peak: 4200.0,
            rpm_max: 6800.0,
            gears: vec![3.5, 2.1, 1.45, 1.1, 0.88, 0.72],
            reverse: 3.3,
            final_drive: 3.6,
            efficiency: 0.85,
            shift_up_rpm: 6200.0,
            shift_down_rpm: 2600.0,
            shift_s: 0.18,
            front_drive: 0.35,
            brake_force: 14_000.0,
            brake_front: 0.62,
            handbrake_force: 3_500.0,
            handbrake_grip: 0.45,
            reverse_max: 8.0,
            drag: 0.5 * 1.2 * 0.32 * 2.2,
            rolling: 0.013,
        }
    }
}

impl VehicleDesc {
    /// Wheel mount points, car frame: front left, front right, rear left, rear right.
    pub fn mounts(&self) -> [DVec3; 4] {
        let (x, f, r, z) = (self.half_track, self.front_y, self.rear_y, self.mount_z);
        [DVec3::new(-x, f, z), DVec3::new(x, f, z), DVec3::new(-x, r, z), DVec3::new(x, r, z)]
    }
    /// Spring rate per corner, N/m.
    pub fn spring_k(&self) -> f64 {
        let w = std::f64::consts::TAU * self.spring_hz;
        self.mass / 4.0 * w * w
    }
    /// Damper rate per corner, N s/m.
    pub fn damper_c(&self) -> f64 {
        2.0 * self.damping_ratio * libm::sqrt(self.spring_k() * self.mass / 4.0)
    }
    /// Height of the body origin above flat ground when settled.
    pub fn ride_height(&self) -> f64 {
        let sag = self.mass * 9.80665 / 4.0 / self.spring_k();
        -self.mount_z + (self.rest_length - sag).max(0.0) + self.wheel_radius
    }
}

/// One tick of driver input.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VehicleInput {
    /// -1 .. 1: forward is throttle (or brake while rolling backwards), back is brake (or
    /// reverse once nearly stopped).
    pub drive: f32,
    /// -1 .. 1, positive steers right.
    pub steer: f32,
    pub handbrake: bool,
}

/// One wheel after the last update.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Wheel {
    pub contact: bool,
    /// Suspension length below the mount (0 .. rest), m.
    pub length: f64,
    pub compression: f64,
    /// Steer angle (rad, positive right) and spin angle (rad, forward rolling positive).
    pub steer: f64,
    pub spin: f64,
    /// Ground load (N) and sideways slip speed (m/s), for effects.
    pub load: f64,
    pub slip: f64,
}

/// A car: its body and controller state.
#[derive(Clone, Debug, PartialEq)]
pub struct Vehicle {
    pub body: Entity,
    pub desc: VehicleDesc,
    pub wheels: [Wheel; 4],
    /// Current steer angle, rad (positive right).
    pub steer: f64,
    /// 1.. forward gears, -1 reverse.
    pub gear: i32,
    pub rpm: f64,
    shift_timer: f64,
}

fn rotate_about(v: DVec3, axis: DVec3, angle: f64) -> DVec3 {
    let (s, c) = (libm::sin(angle), libm::cos(angle));
    v * c + axis.cross(v) * s + axis * axis.dot(v) * (1.0 - c)
}

/// Rotation about region z by `yaw` (rad, counter-clockwise; 0 faces north), via `libm`.
pub fn yaw_rotation(yaw: f64) -> DQuat {
    DQuat::from_xyzw(0.0, 0.0, libm::sin(yaw * 0.5), libm::cos(yaw * 0.5))
}

impl Vehicle {
    /// Spawns the body with its origin at `position`, facing `yaw` (0 = north, CCW).
    pub fn spawn(world: &mut World, desc: VehicleDesc, position: DVec3, yaw: f64) -> Result<Vehicle, SimError> {
        let filter = Filter { category: layers::VEHICLE, mask: layers::ALL & !layers::MOVER };
        let mat = Material { density: 300.0, friction: 0.3, restitution: 0.05 };
        let mut d = BodyDesc::new(BodyKind::Dynamic, position).with_rotation(yaw_rotation(yaw));
        d.shapes.push(ShapeDesc::cuboid(desc.body_half).with_material(mat).with_filter(filter));
        d.shapes.push(ShapeDesc::cuboid(desc.cabin_half).at(desc.cabin_offset, glam::Quat::IDENTITY).with_material(mat).with_filter(filter));
        d.can_sleep = false;
        d.angular_damping = 0.05;
        let body = world.create_body(d)?;
        if !world.set_mass(body, desc.mass as f32, desc.com, desc.inertia) {
            world.despawn(body);
            return Err(SimError::InvalidDesc("vehicle mass properties"));
        }
        let rest = desc.rest_length;
        Ok(Vehicle { body, desc, wheels: [Wheel { length: rest, ..Default::default() }; 4], steer: 0.0, gear: 1, rpm: 0.0, shift_timer: 0.0 })
    }

    /// Speed along the car's forward axis, m/s (negative reversing).
    pub fn forward_speed(&self, world: &World) -> f64 {
        world.body_state(self.body).map_or(0.0, |s| s.linear_velocity.dot(s.rotation * DVec3::Y))
    }

    /// Applies this tick's suspension, tyre, drive and drag forces. Call once per tick, right
    /// before [`World::step`].
    pub fn update(&mut self, world: &mut World, input: &VehicleInput) {
        let Some(s) = world.body_state(self.body) else { return };
        let d = &self.desc;
        let dt = DT;
        let q = s.rotation;
        let (fwd, up) = (q * DVec3::Y, q * DVec3::Z);
        let gravity = world.config().gravity;
        let com = s.position + q * d.com.as_dvec3();
        let (v, w) = (s.linear_velocity, s.angular_velocity);
        let vel_at = |p: DVec3| v + w.cross(p - com);
        let speed = v.dot(fwd);
        let drive = (input.drive as f64).clamp(-1.0, 1.0);
        let steer_in = (input.steer as f64).clamp(-1.0, 1.0);

        // Steering: speed-dependent lock; back toward centre faster than away from it.
        let lock = d.max_steer / (1.0 + speed.abs() / d.steer_half_speed);
        let target = steer_in * lock;
        let rate = if target.abs() < self.steer.abs() || target * self.steer < 0.0 { d.return_rate } else { d.steer_rate };
        self.steer += (target - self.steer).clamp(-rate * dt, rate * dt);

        // Driver intent: throttle, brake, gear.
        let (mut throttle, mut brake) = (0.0, 0.0);
        if drive > 0.0 {
            if self.gear < 0 && speed < -0.8 {
                brake = drive;
            } else {
                if self.gear < 0 {
                    self.gear = 1;
                }
                throttle = drive;
            }
        } else if drive < 0.0 {
            if self.gear > 0 && speed > 0.8 {
                brake = -drive;
            } else {
                self.gear = -1;
                throttle = -drive;
            }
        }
        let hold = drive == 0.0 && speed.abs() < 0.6;

        // Engine and gearbox (automatic).
        let r = d.wheel_radius;
        let wheel_rpm = speed.abs() / r * 60.0 / std::f64::consts::TAU;
        let ratio = |g: i32| if g < 0 { d.reverse } else { d.gears[(g - 1) as usize] } * d.final_drive;
        self.shift_timer = (self.shift_timer - dt).max(0.0);
        if self.gear > 0 && self.shift_timer == 0.0 {
            let rpm = wheel_rpm * ratio(self.gear);
            if rpm > d.shift_up_rpm && (self.gear as usize) < d.gears.len() && throttle > 0.0 {
                self.gear += 1;
                self.shift_timer = d.shift_s;
            } else if self.gear > 1 && rpm < d.shift_down_rpm {
                self.gear -= 1;
            }
        }
        self.rpm = (wheel_rpm * ratio(self.gear)).max(d.rpm_idle);
        let torque = if self.rpm >= d.rpm_max {
            0.0
        } else if self.rpm < d.rpm_peak {
            d.torque_peak * (0.7 + 0.3 * (self.rpm - d.rpm_idle) / (d.rpm_peak - d.rpm_idle))
        } else {
            d.torque_peak * (1.0 - 0.35 * (self.rpm - d.rpm_peak) / (d.rpm_max - d.rpm_peak))
        };
        let mut drive_force = torque * throttle * ratio(self.gear) * d.efficiency / r;
        if self.shift_timer > 0.0 || (self.gear < 0 && speed < -d.reverse_max) {
            drive_force = 0.0;
        }
        if self.gear < 0 {
            drive_force = -drive_force;
        }

        // Suspension: rays down the body's up axis.
        let filter = QueryFilter { category: layers::VEHICLE, mask: layers::ALL & !(layers::MOVER | layers::VEHICLE) };
        let reach = d.rest_length + r;
        let (k, c) = (d.spring_k(), d.damper_c());
        let mounts = d.mounts();
        let mut hits = [None; 4];
        let mut load = [0.0; 4];
        for i in 0..4 {
            let m = s.position + q * mounts[i];
            let prev = self.wheels[i];
            let wh = &mut self.wheels[i];
            match world.cast_ray(m, -up * reach, filter) {
                Some(h) => {
                    let dist = h.fraction as f64 * reach;
                    wh.length = (dist - r).clamp(0.0, d.rest_length);
                    wh.compression = d.rest_length - wh.length;
                    let rate = if prev.contact { ((wh.compression - prev.compression) / dt).clamp(-4.0, 4.0) } else { 0.0 };
                    // Bump stop when the tyre itself is squashed.
                    let bump = if dist < r { (r - dist) * k * 6.0 } else { 0.0 };
                    let damp = if rate < 0.0 { c * d.rebound } else { c };
                    load[i] = (k * wh.compression + damp * rate + bump).max(0.0);
                    wh.contact = true;
                    hits[i] = Some((h.point, h.normal.as_dvec3().normalize_or(up), m - up * wh.length));
                }
                None => {
                    wh.length = d.rest_length;
                    wh.compression = 0.0;
                    wh.contact = false;
                }
            }
        }
        // Anti-roll bars: move load toward the more compressed side of each axle.
        for (a, b) in [(0, 1), (2, 3)] {
            if hits[a].is_some() && hits[b].is_some() {
                let f = d.anti_roll * (self.wheels[a].compression - self.wheels[b].compression);
                load[a] = (load[a] + f).max(0.0);
                load[b] = (load[b] - f).max(0.0);
            }
        }

        // Tyres.
        let m_share = d.mass / 4.0;
        for i in 0..4 {
            let front = i < 2;
            let steer = if front { self.steer } else { 0.0 };
            let wh = &mut self.wheels[i];
            wh.steer = steer;
            wh.load = load[i];
            let Some((point, n, hub)) = hits[i] else {
                wh.slip = 0.0;
                continue;
            };
            world.apply_force_at(self.body, up * load[i], hub);
            let fz = load[i];
            let wf = rotate_about(fwd, up, -steer);
            let fw = (wf - n * wf.dot(n)).normalize_or(fwd);
            let side = fw.cross(n);
            let vc = vel_at(point);
            let (vl, vs) = (vc.dot(fw), vc.dot(side));
            wh.slip = vs;
            let mu = d.grip;
            // The force that stops this corner sliding along `dir` within a few ticks, gravity's
            // pull along the ground included (so a parked car does not creep down a slope).
            let cancel = |vel: f64, dir: DVec3| -(vel * 0.6 / dt + gravity.dot(dir)) * m_share;
            // Lateral.
            let alpha = libm::atan2(vs.abs(), vl.abs().max(0.5)) / d.slip_peak;
            let curve = if alpha < 1.0 { alpha * (1.5 - 0.5 * alpha * alpha) } else { 1.0 - 0.2 * (alpha - 1.0).min(1.0) };
            let mut lat_grip = mu * fz * curve;
            let hand = input.handbrake && !front;
            if hand {
                lat_grip *= d.handbrake_grip;
            }
            let lat_cancel = cancel(vs, side);
            let f_lat = lat_cancel.clamp(-lat_grip, lat_grip);
            // Longitudinal.
            let share = if front { d.front_drive } else { 1.0 - d.front_drive } * 0.5;
            let mut f_long = drive_force * share;
            let long_cancel = cancel(vl, fw);
            let mut stop = 0.0;
            if brake > 0.0 {
                stop += brake * d.brake_force * if front { d.brake_front } else { 1.0 - d.brake_front } * 0.5;
            }
            if hand {
                stop += d.handbrake_force;
                f_long = 0.0;
            }
            if hold {
                stop += mu * fz;
                f_long = 0.0;
            }
            stop += d.rolling * fz;
            f_long += long_cancel.clamp(-stop, stop);
            // Friction circle.
            let max = mu * fz;
            let mag = libm::sqrt(f_long * f_long + f_lat * f_lat);
            let scale = if mag > max && mag > 0.0 { max / mag } else { 1.0 };
            world.apply_force_at(self.body, (fw * f_long + side * f_lat) * scale, point);
            // Spin for drawing: rolling speed, locked under the handbrake.
            if !hand {
                wh.spin = (wh.spin + vl / r * dt).rem_euclid(std::f64::consts::TAU);
            }
        }

        // In the air: damp pitch and roll (not yaw) toward still.
        if hits.iter().filter(|h| h.is_some()).count() <= 1 {
            let wl = q.inverse() * w;
            let inertia = d.inertia.as_dvec3();
            let t = DVec3::new(-wl.x * inertia.x, -wl.y * inertia.y, 0.0) * d.air_damping;
            world.apply_torque(self.body, q * t);
        }

        // Aerodynamic drag at the centre of mass.
        let sp = v.length();
        if sp > 0.0 {
            world.apply_force_at(self.body, -v * (d.drag * sp), com);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Command, SimConfig, SimInput};

    fn world_with_ground(slope_deg: f64) -> World {
        let mut w = World::new(&SimConfig::default());
        // A big box whose top face passes through the origin, tilted about y (side slope for a
        // car facing north).
        let a = slope_deg.to_radians();
        let rot = DQuat::from_xyzw(0.0, libm::sin(a * 0.5), 0.0, libm::cos(a * 0.5));
        let centre = rot * DVec3::new(0.0, 0.0, -5.0);
        let d = BodyDesc::new(BodyKind::Static, centre).with_rotation(rot).with_shape(ShapeDesc::cuboid(Vec3::new(3000.0, 3000.0, 5.0)));
        w.create_body(d).unwrap();
        w
    }

    fn step(w: &mut World, car: &mut Vehicle, input: VehicleInput) {
        car.update(w, &input);
        let tick = w.tick();
        let cmds: [Command; 0] = [];
        w.step(&SimInput { tick, commands: &cmds, utc: 0.0 });
    }

    fn spawn(w: &mut World, z: f64) -> Vehicle {
        let d = VehicleDesc::default();
        let h = d.ride_height();
        Vehicle::spawn(w, d, DVec3::new(0.0, 0.0, z + h + 0.1), 0.0).unwrap()
    }

    fn secs(s: f64) -> usize {
        (s / DT).round() as usize
    }

    #[test]
    fn comes_to_rest_level() {
        let mut w = world_with_ground(0.0);
        let mut car = spawn(&mut w, 0.0);
        for _ in 0..secs(3.0) {
            step(&mut w, &mut car, VehicleInput::default());
        }
        let s = w.body_state(car.body).unwrap();
        assert!(s.linear_velocity.length() < 0.02, "still moving: {:?}", s.linear_velocity);
        assert!((s.rotation * DVec3::Z).z > 0.999, "not level");
        assert!((s.position.z - car.desc.ride_height()).abs() < 0.03, "ride height {} vs {}", s.position.z, car.desc.ride_height());
        assert!(car.wheels.iter().all(|w| w.contact));
    }

    fn accelerate(w: &mut World, car: &mut Vehicle, to: f64, max_s: f64) -> Option<f64> {
        for i in 0..secs(max_s) {
            step(w, car, VehicleInput { drive: 1.0, ..Default::default() });
            if car.forward_speed(w) >= to {
                return Some(i as f64 * DT);
            }
        }
        None
    }

    #[test]
    fn zero_to_hundred_then_stops() {
        let mut w = world_with_ground(0.0);
        let mut car = spawn(&mut w, 0.0);
        for _ in 0..secs(1.0) {
            step(&mut w, &mut car, VehicleInput::default());
        }
        let t = accelerate(&mut w, &mut car, 100.0 / 3.6, 20.0).expect("never reached 100 km/h");
        assert!((5.0..11.0).contains(&t), "0-100 km/h in {t:.1} s");
        assert!(car.gear >= 2, "gear {}", car.gear);
        eprintln!("0-100 km/h {t:.2} s, gear {}", car.gear);
        let y0 = w.body_state(car.body).unwrap().position.y;
        let mut stopped = None;
        for i in 0..secs(10.0) {
            step(&mut w, &mut car, VehicleInput { drive: -1.0, ..Default::default() });
            if car.forward_speed(&w) < 0.3 {
                stopped = Some(i);
                break;
            }
        }
        assert!(stopped.is_some(), "did not stop");
        let dist = w.body_state(car.body).unwrap().position.y - y0;
        eprintln!("100-0 km/h in {dist:.1} m");
        assert!((25.0..60.0).contains(&dist), "100-0 in {dist:.1} m");
        let s = w.body_state(car.body).unwrap();
        assert!(s.position.x.abs() < 1.0, "pulled sideways {:.2} m", s.position.x);
        // Releasing: stays put.
        for _ in 0..secs(2.0) {
            step(&mut w, &mut car, VehicleInput::default());
        }
        assert!(car.forward_speed(&w).abs() < 0.05);
    }

    #[test]
    fn reverses_after_stopping() {
        let mut w = world_with_ground(0.0);
        let mut car = spawn(&mut w, 0.0);
        for _ in 0..secs(4.0) {
            step(&mut w, &mut car, VehicleInput { drive: -1.0, ..Default::default() });
        }
        let v = car.forward_speed(&w);
        assert!(car.gear == -1 && v < -4.0 && v > -car.desc.reverse_max - 0.5, "reverse at {v:.2} m/s");
    }

    #[test]
    fn turning_circle() {
        let mut w = world_with_ground(0.0);
        let mut car = spawn(&mut w, 0.0);
        accelerate(&mut w, &mut car, 4.0, 5.0).unwrap();
        // Hold ~4 m/s with full right lock.
        let mut radius = Vec::new();
        for _ in 0..secs(8.0) {
            let v = car.forward_speed(&w);
            let drive = if v < 4.0 { 0.4 } else { 0.0 };
            step(&mut w, &mut car, VehicleInput { drive, steer: 1.0, handbrake: false });
            let s = w.body_state(car.body).unwrap();
            radius.push(s.linear_velocity.length() / s.angular_velocity.z.abs().max(1e-6));
        }
        let s = w.body_state(car.body).unwrap();
        assert!(s.angular_velocity.z < 0.0, "steering right must turn clockwise");
        let r = radius[radius.len() - 1];
        eprintln!("turn radius at 4 m/s, full lock: {r:.2} m");
        assert!((4.0..8.0).contains(&r), "turn radius {r:.2} m");
        assert!((s.rotation * DVec3::Z).z > 0.98);
    }

    #[test]
    fn grips_at_speed_and_slides_with_handbrake() {
        let mut w = world_with_ground(0.0);
        let mut car = spawn(&mut w, 0.0);
        accelerate(&mut w, &mut car, 20.0, 10.0).unwrap();
        for _ in 0..secs(2.0) {
            step(&mut w, &mut car, VehicleInput { drive: 0.3, steer: 0.5, handbrake: false });
        }
        let s = w.body_state(car.body).unwrap();
        assert!((s.rotation * DVec3::Z).z > 0.95, "rolled over in a 20 m/s turn");
        let slip = car.wheels[2].slip.abs();
        for _ in 0..secs(0.6) {
            step(&mut w, &mut car, VehicleInput { drive: 0.0, steer: 0.5, handbrake: true });
        }
        assert!(car.wheels[2].slip.abs() > slip + 0.5, "handbrake should make the rear slide ({slip:.2} -> {:.2})", car.wheels[2].slip);
    }

    #[test]
    fn side_slope_does_not_flip() {
        let mut w = world_with_ground(15.0);
        let d = VehicleDesc::default();
        let h = d.ride_height();
        // On the slope facing north (along the contour): body up is the slope normal.
        let a = 15f64.to_radians();
        let tilt = DQuat::from_xyzw(0.0, libm::sin(a * 0.5), 0.0, libm::cos(a * 0.5));
        let mut car = Vehicle::spawn(&mut w, d, tilt * DVec3::new(0.0, 0.0, h + 0.1), 0.0).unwrap();
        w.set_transform(car.body, tilt * DVec3::new(0.0, 0.0, h + 0.1), tilt);
        for _ in 0..secs(3.0) {
            step(&mut w, &mut car, VehicleInput::default());
        }
        let s = w.body_state(car.body).unwrap();
        assert!(s.linear_velocity.length() < 0.05, "creeping on the slope: {:?}", s.linear_velocity);
        for _ in 0..secs(4.0) {
            step(&mut w, &mut car, VehicleInput { drive: 0.4, steer: 0.3, handbrake: false });
        }
        let s = w.body_state(car.body).unwrap();
        assert!((s.rotation * DVec3::Z).z > 0.8, "flipped on a 15 degree side slope");
        assert!(car.wheels.iter().filter(|w| w.contact).count() >= 3);
    }

    #[test]
    fn deterministic() {
        let run = || {
            let mut w = world_with_ground(0.0);
            let mut car = spawn(&mut w, 0.0);
            for i in 0..secs(6.0) {
                let steer = if i % 120 < 60 { 0.7 } else { -0.4 };
                step(&mut w, &mut car, VehicleInput { drive: 1.0, steer, handbrake: i % 200 > 180 });
            }
            (w.state_hash(), car)
        };
        let (a, ca) = run();
        let (b, cb) = run();
        assert_eq!(a, b);
        assert_eq!(ca, cb);
    }

    #[test]
    fn mass_properties_apply() {
        let mut w = world_with_ground(0.0);
        let car = spawn(&mut w, 0.0);
        let (m, c) = w.mass(car.body).unwrap();
        assert!((m - 1500.0).abs() < 1e-3);
        assert!((c - car.desc.com).length() < 1e-5, "com {c:?}");
    }
}
