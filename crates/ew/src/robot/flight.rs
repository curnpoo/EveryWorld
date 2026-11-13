//! The robot's flight: a port of the alpha's `player.rs` `drive_player` air branches
//! (rocket shoes, afterburner, wingsuit skydive, plain freefall, parachute) on `ed_flight`.
//!
//! Engine-free f32 maths in the render frame (x east, y up, forward -z), exactly the frame the
//! alpha ran in, so its constants and formulas carry over verbatim. The controller converts the
//! sim state in and the forces and torques out. No clock, no randomness: a pure function of the
//! state and the inputs, stepped at the fixed rate.
//!
//! Keys (alpha): Space in the air = rocket, Shift in the air = afterburner (ramps over 10 s),
//! F = wingsuit on/off, P = parachute, G = a physical nudge. W/S tilt the aim, A/D bank, the
//! mouse aims.

use ed_flight::flyer::{self, BODY_INERTIA, FallCmd, WingCmd, WingPose};
use ed_flight::{MachEvent, MachTracker, atmosphere};
use glam::{EulerRot, Mat3, Quat, Vec3};

pub const MASS: f32 = 80.0;
/// The alpha's flight gravity constant (used for thrust sizing).
pub const G: f32 = 9.81;
/// Space rocket: 1.5 g.
pub const SPACE_G: f32 = 1.5;
/// Afterburner full thrust: 36 g after a 10 s ramp.
pub const BOOST_G: f32 = 36.0;
pub const BOOST_RAMP_S: f32 = 10.0;
/// Camera pitch that counts as level for flight aiming (the chase view looks slightly down).
pub const CAM_LEVEL_PITCH: f32 = -0.25;
/// Parachute canopy: drag area constant and glide speed (m/s) toward the wish.
const CANOPY_AREA: f32 = 28.0;
const CANOPY_GLIDE_MPS: f32 = 10.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum MoveMode {
    #[default]
    Walk,
    Fall,
    RocketShoes,
    Skydive,
    Parachute,
}

impl MoveMode {
    pub fn label(self) -> &'static str {
        match self {
            MoveMode::Walk => "WALK",
            MoveMode::Fall => "FALL",
            MoveMode::RocketShoes => "ROCKET",
            MoveMode::Skydive => "SKYDIVE",
            MoveMode::Parachute => "PARACHUTE",
        }
    }
}

/// What the rig and the view need from the flight state each frame.
#[derive(Clone, Copy, Debug)]
pub struct FlightPose {
    pub mode: MoveMode,
    pub throttle: f32,
    pub afterburner: bool,
    pub wingsuit_on: bool,
    pub fwd_in: f32,
    pub wing_arch: f32,
    pub wing_turn: f32,
    pub fall_spread: f32,
    pub fall_tuck: f32,
    pub leg_steer: Vec3,
    /// Camera-relative wished direction (render frame), for the stance.
    pub intent_dir: Vec3,
}

impl Default for FlightPose {
    fn default() -> Self {
        FlightPose { mode: MoveMode::Walk, throttle: 0.0, afterburner: false, wingsuit_on: true, fwd_in: 0.0, wing_arch: 0.0, wing_turn: 0.0, fall_spread: 0.0, fall_tuck: 0.0, leg_steer: Vec3::ZERO, intent_dir: Vec3::ZERO }
    }
}

/// One step's inputs, all in the render frame.
#[derive(Clone, Copy, Debug)]
pub struct FlightIn {
    pub rot: Quat,
    pub v: Vec3,
    pub omega: Vec3,
    /// Height above the ellipsoid, m.
    pub alt: f64,
    /// Height above the ground, m.
    pub agl: f32,
    pub grounded: bool,
    pub jump_push_timer: f32,
    /// W/S (+1 forward) and D/A (+1 right).
    pub fwd_in: f32,
    pub strafe: f32,
    pub cam_yaw: f32,
    pub cam_pitch: f32,
    pub space_held: bool,
    pub shift: bool,
    pub accel_smooth: Vec3,
    /// The unit horizontal wish direction, zero with no input.
    pub wish: Vec3,
    /// The facing yaw after this step's update, and its rate.
    pub yaw: f32,
    pub yaw_vel: f32,
}

/// What a step asks of the body (render frame).
#[derive(Clone, Copy, Debug, Default)]
pub struct FlightOut {
    pub force: Vec3,
    pub torque: Vec3,
    /// Boot thrust points (offset from the centre of mass) and forces.
    pub points: [(Vec3, Vec3); 2],
    /// The posture target (`None` in Walk: the controller's lean applies).
    pub target: Option<Quat>,
    /// Weight of the posture torque (rockets steer through the soles, the suit through the air).
    pub muscle: f32,
}

/// Persistent flight state.
#[derive(Clone, Debug)]
pub struct FlightState {
    pub mode: MoveMode,
    pub wingsuit_on: bool,
    pub space_latched: bool,
    space_prev: bool,
    pub boost_time: f32,
    pub boost_stage: usize,
    pub throttle: f32,
    pub afterburner: bool,
    pub thrust_held: bool,
    pub winged_boost: bool,
    pub wingsuit: bool,
    pub freefall: bool,
    pub leg_steer: Vec3,
    pub wing_arch: f32,
    pub wing_turn: f32,
    pub fall_tuck: f32,
    pub fall_spread: f32,
    pub intent_dir: Vec3,
    pub fwd_in: f32,
    /// Camera shake 0..1 (Mach crossing, suit buffet).
    pub shake: f32,
    pub mach: MachTracker,
    /// Mach 1 was crossed this step (the sonic boom hook).
    pub boom: bool,
    pub speed: f32,
}

impl Default for FlightState {
    fn default() -> Self {
        FlightState {
            mode: MoveMode::Walk,
            wingsuit_on: true,
            space_latched: false,
            space_prev: false,
            boost_time: 0.0,
            boost_stage: 0,
            throttle: 0.0,
            afterburner: false,
            thrust_held: false,
            winged_boost: false,
            wingsuit: false,
            freefall: false,
            leg_steer: Vec3::ZERO,
            wing_arch: 0.0,
            wing_turn: 0.0,
            fall_tuck: 0.0,
            fall_spread: 0.0,
            intent_dir: Vec3::ZERO,
            fwd_in: 0.0,
            shake: 0.0,
            mach: MachTracker::default(),
            boom: false,
            speed: 0.0,
        }
    }
}

impl FlightState {
    /// P: parachute on and off (only from the air modes).
    pub fn toggle_parachute(&mut self) {
        self.mode = match self.mode {
            MoveMode::Skydive | MoveMode::Fall => MoveMode::Parachute,
            MoveMode::Parachute => MoveMode::Skydive,
            m => m,
        };
    }

    /// F: wingsuit on and off.
    pub fn toggle_wingsuit(&mut self) {
        self.wingsuit_on = !self.wingsuit_on;
    }

    pub fn pose(&self) -> FlightPose {
        FlightPose {
            mode: self.mode,
            throttle: self.throttle,
            afterburner: self.afterburner,
            wingsuit_on: self.wingsuit_on,
            fwd_in: self.fwd_in,
            wing_arch: self.wing_arch,
            wing_turn: self.wing_turn,
            fall_spread: self.fall_spread,
            fall_tuck: self.fall_tuck,
            leg_steer: self.leg_steer,
            intent_dir: self.intent_dir,
        }
    }

    /// Space latch, afterburner, boost ramp, mode transitions, freefall pose blend.
    pub fn update_mode(&mut self, i: &FlightIn, dt: f32) {
        let v_up = i.v.y;
        let v_flat = Vec3::new(i.v.x, 0.0, i.v.z);
        let speed_flat = v_flat.length();
        let speed_total = i.v.length();
        self.speed = speed_total;
        self.fwd_in = i.fwd_in;
        let just_pressed = i.space_held && !self.space_prev;
        self.space_prev = i.space_held;

        if i.grounded {
            self.space_latched = false;
        } else {
            // In the air: pressing Space (or holding it past the jump peak) engages the rocket.
            if just_pressed || (i.space_held && v_up < 1.0 && i.jump_push_timer <= 0.0) {
                self.space_latched = true;
            }
            if !i.space_held {
                self.space_latched = false;
            }
        }
        let space_rocket = !i.grounded && self.space_latched;
        // Shift is sprint on the ground and the afterburner in the air.
        self.afterburner = i.shift && !i.grounded;
        self.thrust_held = self.afterburner || space_rocket;
        if self.afterburner {
            self.boost_time += dt;
        } else {
            self.boost_time = (self.boost_time - dt * 2.5).max(0.0);
        }
        self.boost_stage = if self.afterburner { (1 + (self.boost_time / 4.0) as usize).min(3) } else { 0 };

        // Wingsuit on and already flying: the rockets only add thrust to the suit.
        self.winged_boost = self.thrust_held && self.wingsuit_on && !i.grounded && (speed_total > 12.0 || (self.mode == MoveMode::Skydive && speed_total > 6.0));
        let wants_glide = !i.grounded && (!self.thrust_held || self.winged_boost) && (speed_flat > 10.0 || (i.agl > 8.0 && v_up < -8.0));
        let was = self.mode;
        self.mode = if self.winged_boost {
            MoveMode::Skydive
        } else if self.thrust_held {
            MoveMode::RocketShoes
        } else if i.grounded {
            MoveMode::Walk
        } else {
            match was {
                MoveMode::Walk => {
                    if wants_glide {
                        MoveMode::Skydive
                    } else {
                        MoveMode::Fall
                    }
                }
                MoveMode::RocketShoes => {
                    if wants_glide {
                        MoveMode::Skydive
                    } else {
                        MoveMode::Fall
                    }
                }
                MoveMode::Fall if wants_glide => MoveMode::Skydive,
                MoveMode::Skydive if !wants_glide && i.agl < 20.0 && speed_flat < 5.0 => MoveMode::Fall,
                m => m,
            }
        };

        self.wingsuit = self.wingsuit_on && matches!(self.mode, MoveMode::Skydive | MoveMode::Fall) && (!self.thrust_held || self.winged_boost) && !i.grounded;
        // Suit off: a plain skydiver. W somersaults, A/D barrel roll, S spreads eagle.
        self.freefall = !self.wingsuit_on && self.mode == MoveMode::Skydive && !self.thrust_held && !i.grounded;
        let (tuck, spread) = if self.freefall { (i.fwd_in.max(0.0).max(i.strafe.abs() * 0.5), (-i.fwd_in).max(0.0)) } else { (0.0, 0.0) };
        let k = 1.0 - (-6.0 * dt).exp();
        self.fall_tuck += (tuck - self.fall_tuck) * k;
        self.fall_spread += (spread - self.fall_spread) * k;
    }

    /// The facing target and follow rate for the air modes (`None`: keep the facing).
    pub fn yaw_target(&self, cam_yaw: f32, has_dir_input: bool) -> Option<(f32, f32)> {
        match self.mode {
            MoveMode::Walk => None,
            MoveMode::RocketShoes => Some((cam_yaw, 6.0)),
            MoveMode::Skydive => Some((cam_yaw, 4.0)),
            _ => has_dir_input.then_some((cam_yaw, 3.5)),
        }
    }

    /// Aerodynamics, thrust, wingsuit and the posture target for this step.
    pub fn forces(&mut self, i: &FlightIn, dt: f32) -> FlightOut {
        let mut out = FlightOut { muscle: 1.0, ..Default::default() };
        let up = Vec3::Y;
        let mass = MASS;
        let v = i.v;
        let v_up_scalar = v.dot(up);
        let v_flat = v - up * v_up_scalar;
        let speed_flat = v_flat.length();
        let speed_total = v.length();
        let rho = atmosphere(i.alt).density as f32;
        let yaw_frame = Quat::from_rotation_y(i.yaw);
        let right_world = yaw_frame * Vec3::X;
        let (s, c) = i.cam_yaw.sin_cos();
        let cam_fwd_flat = Vec3::new(-s, 0.0, -c);

        // 1. Pure aerodynamic drag and the wingsuit's lift.
        let q_dyn = 0.5 * rho * speed_total * speed_total;
        let wind_dir = (-v).normalize_or(up);
        let body_head = i.rot * Vec3::Y;
        let axial = body_head.dot(wind_dir).abs().clamp(0.0, 1.0);
        let broadside_sq = 1.0 - axial * axial;
        let wing_open = if self.wingsuit { ((speed_total - 4.0) / 10.0).clamp(0.0, 1.0) } else { 0.0 };
        if self.mode != MoveMode::Parachute {
            let pose_area = 1.0 + 0.4 * self.fall_spread - 0.35 * self.fall_tuck;
            let cd_area = (0.04 + 0.62 * broadside_sq) * pose_area;
            out.force += wind_dir * (q_dyn * cd_area * (1.0 - wing_open));
        } else {
            let k = 0.5 * rho * CANOPY_AREA;
            let drag_v = -(up * v_up_scalar) * (k * v_up_scalar.abs() * 1.5);
            let glide = (i.wish * CANOPY_GLIDE_MPS - v_flat) * (mass * 1.5);
            out.force += drag_v + glide;
        }

        let mut wing_shake = 0.0f32;
        if self.wingsuit {
            let cmd = WingCmd {
                pitch: -i.fwd_in,
                roll: i.strafe,
                heading: if i.strafe.abs() > 0.05 { Vec3::ZERO } else { cam_fwd_flat },
                arch_bias: ((i.cam_pitch - CAM_LEVEL_PITCH) * 1.2).clamp(-0.6, 0.6),
            };
            let want = flyer::wing_pilot(i.rot, v, up, i.omega, cmd);
            // Limbs move at a finite rate: the suit responds loosely, not instantly.
            let k = 1.0 - (-10.0 * dt).exp();
            self.wing_arch += (want.arch - self.wing_arch) * k;
            let twist = -self.wing_turn + (want.twist + self.wing_turn) * k;
            self.wing_turn = -twist;
            let pose = WingPose { arch: self.wing_arch, twist, open: wing_open };
            let aero = flyer::wingsuit(i.rot, v, rho, i.omega, BODY_INERTIA, pose);
            out.force += aero.force;
            out.torque += aero.torque;
            wing_shake = 0.12 * (speed_total / 70.0).clamp(0.0, 1.0) * wing_open;
        } else {
            let k = (1.0 - 4.0 * dt).max(0.0);
            self.wing_turn *= k;
            self.wing_arch *= k;
        }
        // Rockets off (including the frame we touch down) means no flame.
        if !self.thrust_held {
            self.throttle = 0.0;
        }

        // 3. Rocket boots and the posture target.
        let cam_rot = Quat::from_euler(EulerRot::YXZ, i.cam_yaw, i.cam_pitch, 0.0);
        let cam_fwd_3d = cam_rot * Vec3::NEG_Z;
        let cam_right_3d = cam_rot * Vec3::X;
        let cam_up_3d = cam_rot * Vec3::Y;
        self.intent_dir = (cam_fwd_3d * i.fwd_in + cam_right_3d * i.strafe).normalize_or_zero();
        let mut muscle = 1.0f32;
        let target: Option<Quat>;
        let (rk, bo) = (1.0f32, if i.shift { 1.0f32 } else { 0.0 });

        if self.winged_boost {
            // Thrust only along the head; the suit and its posture stay as they are.
            let ramp = flyer::boost_ramp(self.boost_time, BOOST_RAMP_S);
            let thrust_acc = if self.afterburner { G * (SPACE_G * rk + (BOOST_G - SPACE_G) * ramp * bo) } else { G * SPACE_G * rk };
            self.throttle = if self.afterburner { (0.65 * rk + ramp * 2.5 * bo + 0.35 * bo).max(0.1) } else { 0.65 * rk };
            out.force += i.rot * Vec3::Y * (mass * thrust_acc);
        }
        if self.thrust_held && !self.winged_boost {
            let ramp = flyer::boost_ramp(self.boost_time, BOOST_RAMP_S);
            let thrust_acc = if self.afterburner { G * (SPACE_G * rk + (BOOST_G - SPACE_G) * ramp * bo) } else { G * SPACE_G * rk };
            self.throttle = if self.afterburner { (0.65 * rk + ramp * 2.5 * bo + 0.35 * bo).max(0.1) } else { 0.65 * rk };

            // Aim = the view (offset so the default slightly-down camera means level flight),
            // tilted by the held keys: S noses up, W down, A/D swing left/right.
            let aim_pitch = (i.cam_pitch - CAM_LEVEL_PITCH - i.fwd_in * 0.6).clamp(-1.5, 1.5);
            let aim_yaw = i.cam_yaw - i.strafe * 0.4;
            let aim = Quat::from_euler(EulerRot::YXZ, aim_yaw, aim_pitch, 0.0) * Vec3::NEG_Z;
            // Point the boots so thrust + gravity accelerates along the aim.
            let axis = flyer::rocket_axis(aim, up, thrust_acc, G);
            let roll = (i.strafe * 0.55 - i.yaw_vel * 0.2).clamp(-0.8, 0.8);
            let tgt = flyer::rocket_attitude(axis, cam_up_3d, up, roll);
            // The boots throttle back while the body swings round, but keep a floor.
            let gate = (((i.rot * Vec3::Y).dot(axis) - 0.3) / 0.5).clamp(0.25, 1.0);
            let thrust_n = mass * thrust_acc * gate;
            // A loose spring says how the body would like to turn; the legs swing the feet so the
            // thrust line passes beside the centre of mass and makes that torque.
            let want = flyer::rocket_want_torque(i.rot, tgt, i.omega);
            let steer = flyer::boot_steer(i.rot, thrust_n, want);
            let leg = self.leg_steer;
            self.leg_steer = leg + (steer.leg_offset - leg) * (1.0 - (-15.0 * dt).exp());
            out.points = flyer::boot_forces(i.rot, Vec3::ZERO, thrust_n, self.leg_steer, steer.roll_push);
            muscle = 0.0;
            target = Some(tgt);
        } else {
            target = match self.mode {
                MoveMode::Walk => None,
                MoveMode::Skydive => {
                    let base_dive_pitch = -std::f32::consts::FRAC_PI_2 * 0.88;
                    let cam_pitch_influence = i.cam_pitch.clamp(-1.1, 0.6) * 0.42;
                    let ws_pitch_offset = -i.fwd_in * 0.42;
                    let total_pitch = (base_dive_pitch + cam_pitch_influence + ws_pitch_offset).clamp(-1.50, -0.30);
                    let pitch_pose = yaw_frame * Quat::from_rotation_x(total_pitch);
                    // Head along the flight path with a small angle of attack, belly toward the
                    // ground, banking into turns; blends in as horizontal speed builds.
                    let dive = i.fwd_in.max(0.0);
                    let flare = (-i.fwd_in).max(0.0);
                    let h = (v.normalize_or(up) + up * (0.18 + 0.4 * flare - 0.45 * dive)).normalize_or(up);
                    let back = (up - h * up.dot(h)).normalize_or(yaw_frame * Vec3::Z);
                    let bank = (-self.wing_turn * 0.95 + (-i.yaw_vel * 0.15).clamp(-0.5, 0.5)).clamp(-1.2, 1.2);
                    let flight = Quat::from_mat3(&Mat3::from_cols(h.cross(back), h, back)) * Quat::from_rotation_y(bank);
                    let blend = ((speed_flat - 6.0) / 14.0).clamp(0.0, 1.0);
                    Some(pitch_pose.slerp(flight, blend))
                }
                MoveMode::Fall => {
                    // W tilts forward, S backward, A/D sideways; upright otherwise.
                    let pitch_in = i.fwd_in * 0.55;
                    let roll_in = -i.strafe * 0.48;
                    Some(yaw_frame * Quat::from_euler(EulerRot::YXZ, 0.0, -pitch_in, roll_in))
                }
                MoveMode::Parachute | MoveMode::RocketShoes => {
                    let swing_up = (up - i.accel_smooth * 0.03 + right_world * (-i.strafe * 0.2)).normalize_or(up);
                    Some(Quat::from_rotation_arc(up, swing_up) * yaw_frame)
                }
            };
        }
        if self.wingsuit {
            // Only while the suit is still collapsed does the flyer hold a posture by muscle.
            muscle = (1.0 - wing_open) * (1.0 - wing_open);
        }
        if self.freefall {
            // Spins and the box position come from limbs in the airflow, not a posture spring.
            let belly_down = yaw_frame * Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
            let cmd = FallCmd { flip: i.fwd_in.max(0.0), roll: i.strafe, spread: self.fall_spread };
            out.torque += flyer::freefall_torque(i.rot, i.omega, speed_total, belly_down, cmd);
            muscle = 0.0;
        }
        if !self.thrust_held || self.winged_boost {
            self.leg_steer *= (1.0 - 6.0 * dt).max(0.0);
        }
        out.target = target;
        out.muscle = muscle;

        self.boom = false;
        if let Some(MachEvent::BrokeBarrier) = self.mach.update(speed_total as f64, i.alt) {
            self.boom = true;
            self.shake = 1.0;
        }
        self.shake = (self.shake - dt * 1.5).max(wing_shake);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn air(v: Vec3, alt: f64) -> FlightIn {
        FlightIn {
            rot: Quat::IDENTITY,
            v,
            omega: Vec3::ZERO,
            alt,
            agl: alt as f32,
            grounded: false,
            jump_push_timer: 0.0,
            fwd_in: 0.0,
            strafe: 0.0,
            cam_yaw: 0.0,
            cam_pitch: 0.0,
            space_held: false,
            shift: false,
            accel_smooth: Vec3::ZERO,
            wish: Vec3::ZERO,
            yaw: 0.0,
            yaw_vel: 0.0,
        }
    }

    #[test]
    fn walk_to_fall_to_rocket_and_back() {
        let mut f = FlightState::default();
        let mut i = air(Vec3::new(0.0, 2.0, 0.0), 50.0);
        f.update_mode(&i, 1.0 / 60.0);
        assert_eq!(f.mode, MoveMode::Fall);
        i.space_held = true;
        f.update_mode(&i, 1.0 / 60.0);
        assert_eq!(f.mode, MoveMode::RocketShoes);
        i.space_held = false;
        f.update_mode(&i, 1.0 / 60.0);
        assert_eq!(f.mode, MoveMode::Fall);
        i.grounded = true;
        f.update_mode(&i, 1.0 / 60.0);
        assert_eq!(f.mode, MoveMode::Walk);
    }

    #[test]
    fn fast_fall_becomes_a_skydive_and_parachute_toggles() {
        let mut f = FlightState::default();
        let i = air(Vec3::new(0.0, -20.0, -15.0), 800.0);
        f.update_mode(&i, 1.0 / 60.0);
        f.update_mode(&i, 1.0 / 60.0);
        assert_eq!(f.mode, MoveMode::Skydive);
        f.toggle_parachute();
        assert_eq!(f.mode, MoveMode::Parachute);
        f.toggle_parachute();
        assert_eq!(f.mode, MoveMode::Skydive);
    }

    #[test]
    fn shift_in_the_air_is_the_afterburner_and_ramps() {
        let mut f = FlightState::default();
        let mut i = air(Vec3::ZERO, 100.0);
        i.shift = true;
        for _ in 0..120 {
            f.update_mode(&i, 1.0 / 60.0);
        }
        assert!(f.afterburner && f.mode == MoveMode::RocketShoes);
        assert!((f.boost_time - 2.0).abs() < 0.05);
        let out = f.forces(&i, 1.0 / 60.0);
        assert!(out.points[0].1.length() > 0.0 && f.throttle > 0.5);
    }
}
