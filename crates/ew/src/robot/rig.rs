//! The robot's procedural gait and pose solver (a port of the alpha's `body::solve_rig`,
//! walking and falling only). Engine-free: glam only, no drawing, no physics.
//!
//! The rig takes the body's pose and motion and returns joint positions and rigid frames
//! ([`Pose`]). What draws them ([`super::view`]) is separate, so another character can be
//! driven from the same joints later.
//!
//! Frame: the render frame, as the alpha used (x east, y up, forward is -z). Positions are
//! relative to the anchor. The solver is a pure function of its inputs and the [`Gait`] state:
//! no clock, no randomness (blinks and idle sway take the caller's time `t_sec`).
//!
//! Constants are the alpha's: limb lengths, hip and shoulder widths, step timing, the
//! spring-damper gains and the anatomical clamps.

use super::flight::{FlightPose, MoveMode};
use glam::{EulerRot, Quat, Vec3};

pub const THIGH: f32 = 0.44;
pub const SHIN: f32 = 0.44;
pub const UPPER_ARM: f32 = 0.29;
pub const FOREARM: f32 = 0.27;
pub const HIP_W: f32 = 0.13;
pub const SHOULDER_W: f32 = 0.27;
pub const SPINE: f32 = 0.42;
pub const BASE_STEP_TIME: f32 = 0.20;
/// Ankle joint height above the ground.
pub const ANKLE_H: f32 = 0.11;

/// Joint ids (indices into [`Pose::joints`]).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum J {
    Pelvis = 0,
    Chest = 1,
    Neck = 2,
    HipL = 3,
    HipR = 4,
    KneeL = 5,
    KneeR = 6,
    AnkleL = 7,
    AnkleR = 8,
    ShoulderL = 9,
    ShoulderR = 10,
    ElbowL = 11,
    ElbowR = 12,
    WristL = 13,
    WristR = 14,
}

/// Rigid frames (position and orientation; indices into [`Pose::frames`]).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum F {
    Pelvis = 0,
    Chest = 1,
    Head = 2,
    FootL = 3,
    FootR = 4,
    HandL = 5,
    HandR = 6,
}

#[derive(Default, Clone, Copy, Debug)]
pub struct Foot {
    pub planted: Vec3,
    pub from: Vec3,
    pub to: Vec3,
    pub t: f32,
    pub step_dur: f32,
    pub stepping: bool,
}

/// A 3D spring-damper state in pelvis-local coordinates.
#[derive(Default, Clone, Copy, Debug)]
pub struct SpringVec3 {
    pub pos: Vec3,
    pub vel: Vec3,
}

impl SpringVec3 {
    pub fn step(&mut self, target: Vec3, external_accel: Vec3, kp: f32, kd: f32, dt: f32) {
        let force = (target - self.pos) * kp - self.vel * kd + external_accel;
        self.vel += force * dt;
        self.pos += self.vel * dt;
    }
}

/// The gait's persistent state (feet, spine, head and hand springs).
#[derive(Default, Clone, Copy, Debug)]
pub struct Gait {
    pub feet: [Foot; 2],
    pub init: bool,
    /// Which foot stepped last (0 or 1), so strides alternate.
    pub last_step_foot: usize,
    /// Seconds until another step may start (prevents rapid step-spam jitter).
    pub step_cooldown: f32,
    /// Low-pass filtered horizontal velocity used for footstep placement.
    pub vs: Vec3,
    /// Hysteretic 'walking vs standing' flag so footstep rules don't flip at a threshold.
    pub moving: bool,
    /// Whether the previous frame was airborne (to detect touchdown).
    pub was_airborne: bool,
    pub phase: f32,
    /// Spine pitch/roll deflection in pelvis-local space (x = lateral roll, z = forward/back pitch).
    pub spine_bend: SpringVec3,
    /// Torso twist: x = chest yaw relative to the pelvis, y = pelvis yaw counter-twist.
    pub spine_twist: SpringVec3,
    /// Head pitch/yaw/roll deflection relative to the chest.
    pub head_angles: SpringVec3,
    /// Pelvis-local ankle positions (spring-damper while airborne, plus anatomical clamps).
    pub local_ankles: [SpringVec3; 2],
    /// Pelvis-local wrist positions (spring-damper plus anatomical clamps).
    pub local_wrists: [SpringVec3; 2],
}

/// What the solver needs to know each frame (everything in the render frame).
#[derive(Clone, Copy, Debug)]
pub struct RigInput {
    /// In the air (the alpha's Fall mode); otherwise walking.
    pub airborne: bool,
    /// Facing yaw (rotation about +y; 0 faces -z), the controller's heading.
    pub yaw: f32,
    pub cam_pitch: f32,
    /// Sideways input, -1..1 (leans and head turn).
    pub strafe: f32,
    pub jump_charge: f32,
    pub jump_push_timer: f32,
    pub landing_recovery: f32,
    /// Smoothed orientation error vector (world), for torso twist.
    pub rot_err: Vec3,
    /// Smoothed acceleration (world), for inertial sway.
    pub accel_smooth: Vec3,
    pub ground_normal: Vec3,
    /// Ground under each hip from the controller's probes.
    pub foot_ground_hit: [Option<Vec3>; 2],
    pub vel: Vec3,
    /// Flight state (mode, wing and fall poses, throttle); walking by default.
    pub fl: FlightPose,
}

impl Default for RigInput {
    fn default() -> Self {
        RigInput {
            airborne: false,
            yaw: 0.0,
            cam_pitch: 0.0,
            strafe: 0.0,
            jump_charge: 0.0,
            jump_push_timer: 0.0,
            landing_recovery: 0.0,
            rot_err: Vec3::ZERO,
            accel_smooth: Vec3::ZERO,
            ground_normal: Vec3::Y,
            foot_ground_hit: [None; 2],
            vel: Vec3::ZERO,
            fl: FlightPose::default(),
        }
    }
}

/// The solved pose.
#[derive(Clone, Copy, Debug)]
pub struct Pose {
    pub pelvis_c: Vec3,
    pub base: Quat,
    pub blink: f32,
    pub joints: [Vec3; 15],
    pub frames: [(Vec3, Quat); 7],
    /// Where each foot stands (planted point, or under the ankle in the air).
    pub foot_world_pos: [Vec3; 2],
    pub foot_planted: [bool; 2],
    pub body_up: Vec3,
    /// The wingsuit is out: draw the wing panels.
    pub flying: bool,
    /// The parachute canopy is open.
    pub canopy: bool,
    /// Wing panels (left arm, right arm, legs): outline points and the centroid lift.
    pub wing_polys: [([Vec3; 7], Vec3); 3],
    /// Boot nozzles: position, thrust direction and forward, per foot; the throttle (0 = off).
    pub nozzle_pos: [Vec3; 2],
    pub nozzle_dir: [Vec3; 2],
    pub throttle: f32,
    /// Seconds, for flame flicker.
    pub time: f32,
}

impl Pose {
    pub fn joint(&self, j: J) -> Vec3 {
        self.joints[j as usize]
    }

    pub fn frame(&self, f: F) -> (Vec3, Quat) {
        self.frames[f as usize]
    }
}

/// Two-bone IK: returns (middle joint, end) given root, target, lengths and a pole direction
/// (which way the knee or elbow points).
fn two_bone(root: Vec3, target: Vec3, l1: f32, l2: f32, pole: Vec3) -> (Vec3, Vec3) {
    let to = target - root;
    let d = to.length().clamp(0.12, l1 + l2 - 0.002);
    let dir = to.normalize_or(Vec3::NEG_Y);
    let end = root + dir * d;
    let cos_a = ((l1 * l1 + d * d - l2 * l2) / (2.0 * l1 * d)).clamp(-1.0, 1.0);
    let a = cos_a.acos();
    let bend = pole.reject_from_normalized(dir).normalize_or(Vec3::NEG_Z);
    let mid = root + (dir * a.cos() + bend * a.sin()) * l1;
    (mid, end)
}

/// Hard anatomical limits on an ankle in pelvis-local space (`hip` = local hip position):
/// legs never fold into the torso or cross the midline.
fn clamp_ankle_anatomical(hip: Vec3, mut ankle: Vec3, side: f32) -> Vec3 {
    let max_reach = THIGH + SHIN - 0.006;
    let min_reach = 0.26; // knee max bend ~135 deg
    ankle.y = ankle.y.min(hip.y - 0.16);
    if side < 0.0 {
        ankle.x = ankle.x.clamp(hip.x - 0.42, -0.03);
    } else {
        ankle.x = ankle.x.clamp(0.03, hip.x + 0.42);
    }
    // Forward/backward hip swing limit (-z is forward).
    ankle.z = ankle.z.clamp(-0.66, 0.62);
    let rel = ankle - hip;
    let len = rel.length();
    if len > max_reach {
        ankle = hip + rel * (max_reach / len);
    } else if len < min_reach {
        ankle = hip + rel.normalize_or(Vec3::NEG_Y) * min_reach;
    }
    ankle
}

/// Hard anatomical limits on a wrist in pelvis-local space (`shoulder` = local shoulder):
/// arms never clip into the chest box.
fn clamp_wrist_anatomical(shoulder: Vec3, mut wrist: Vec3, side: f32) -> Vec3 {
    let max_reach = UPPER_ARM + FOREARM - 0.008;
    let min_reach = 0.15;
    if wrist.y < shoulder.y + 0.05 && wrist.y > shoulder.y - 0.48 && wrist.z.abs() < 0.17 {
        if side < 0.0 {
            wrist.x = wrist.x.min(-0.22);
        } else {
            wrist.x = wrist.x.max(0.22);
        }
    }
    let rel = wrist - shoulder;
    let len = rel.length();
    if len > max_reach {
        wrist = shoulder + rel * (max_reach / len);
    } else if len < min_reach {
        wrist = shoulder + rel.normalize_or(Vec3::X * side) * min_reach;
    }
    wrist
}

/// Solves the pose for a body whose pelvis is at `pelvis` with orientation `base` (render
/// frame). `ground(origin, max)` casts a ray straight down from `origin` and returns the
/// distance to the ground within `max`, if any.
pub fn solve(input: &RigInput, gait: &mut Gait, pelvis: Vec3, base: Quat, dt: f32, t_sec: f32, ground: &dyn Fn(Vec3, f32) -> Option<f32>) -> Pose {
    let up = Vec3::Y;
    let inv_base = base.inverse();
    let yaw_only = Quat::from_rotation_y(input.yaw);
    let right_yaw = yaw_only * Vec3::X;
    let fwd_yaw = yaw_only * Vec3::NEG_Z;

    let v = input.vel;
    let v_flat = v - up * v.dot(up);
    let speed = v_flat.length();
    let airborne = input.airborne;

    let local_accel = inv_base * input.accel_smooth;
    let local_vel = inv_base * v;
    let local_wind = -local_vel;
    let local_err = inv_base * input.rot_err;
    let fl = input.fl;
    let mode = if airborne && fl.mode == MoveMode::Walk { MoveMode::Fall } else { fl.mode };
    let il = inv_base * fl.intent_dir;
    let wing_amp = if matches!(mode, MoveMode::Skydive | MoveMode::Fall) { ((v.length() - 4.0) / 10.0).clamp(0.0, 1.0) } else { 0.0 };
    let rot_effort = local_err.length().min(2.5);

    let leg = THIGH + SHIN;
    let pelvis_c = pelvis;
    let pelvis_twist = Quat::from_rotation_y(gait.spine_twist.pos.y);
    let hips_local = [pelvis_twist * Vec3::new(-HIP_W, -0.04, 0.0), pelvis_twist * Vec3::new(HIP_W, -0.04, 0.0)];

    if !gait.init {
        for i in 0..2 {
            let side = if i == 0 { -1.0 } else { 1.0 };
            let default_ankle = Vec3::new(side * HIP_W, -(leg * 0.94), 0.0);
            gait.local_ankles[i].pos = default_ankle;
            gait.feet[i].planted = pelvis_c + base * (default_ankle - Vec3::Y * ANKLE_H);
            gait.feet[i].step_dur = BASE_STEP_TIME;
            gait.local_wrists[i].pos = Vec3::new(side * (SHOULDER_W + 0.06), -0.12, 0.0);
        }
        gait.init = true;
    }

    let ground_at = |world_pt: Vec3| -> Option<Vec3> { ground(world_pt + up * 0.9, 2.5).map(|d| world_pt + up * (0.9 - d)) };

    let mut target_ankles_local = [Vec3::ZERO; 2];
    let mut foot_planted = [false; 2];

    if airborne {
        gait.was_airborne = true;
        gait.moving = false;
        for i in 0..2 {
            let side = if i == 0 { -1.0f32 } else { 1.0f32 };
            gait.feet[i].stepping = false;
            gait.feet[i].planted = pelvis_c + base * (gait.local_ankles[i].pos - Vec3::Y * ANKLE_H);
            foot_planted[i] = false;
            target_ankles_local[i] = match mode {
                MoveMode::RocketShoes => {
                    let stance_w = if fl.afterburner && fl.fwd_in.abs() > 0.1 { HIP_W * 0.80 } else { HIP_W + 0.03 };
                    let tuck = rot_effort * 0.07;
                    let knee = Vec3::new(il.x, 0.0, il.z).length().min(1.0) * 0.08;
                    Vec3::new(side * (stance_w + rot_effort * 0.04), -(leg * 0.96) + tuck + knee, 0.02) + fl.leg_steer
                }
                MoveMode::Skydive => {
                    let spread = 0.09 - 0.04 * (-fl.wing_arch).max(0.0);
                    let knee_bend = 0.03 + 0.12 * fl.wing_arch.max(0.0);
                    let suit = Vec3::new(side * (HIP_W + spread), -(leg * (0.94 - knee_bend)), 0.12 + knee_bend * 0.4);
                    let eagle = Vec3::new(side * (HIP_W + 0.30), -(leg * 0.82), 0.16);
                    let ball = Vec3::new(side * (HIP_W + 0.02), -(leg * 0.42), -leg * 0.38);
                    suit.lerp(eagle, fl.fall_spread).lerp(ball, fl.fall_tuck)
                }
                MoveMode::Parachute => Vec3::new(side * (HIP_W + 0.03), -(leg * 0.90), 0.06),
                _ => Vec3::new(side * (HIP_W + 0.05), -(leg * 0.84), 0.08),
            };
        }
    } else {
        let k_v = 1.0 - (-9.0 * dt).exp();
        gait.vs = gait.vs.lerp(v_flat, k_v);
        let vs = gait.vs;
        let vs_speed = vs.length();
        gait.moving = if gait.moving { vs_speed > 0.2 } else { vs_speed > 0.45 };
        let travelling = gait.moving;
        let dir = vs.normalize_or_zero();

        let s_half = (0.10 + 0.11 * vs_speed).clamp(0.10, 0.46);
        let step_dur = (0.27 - 0.04 * vs_speed).clamp(0.13, 0.24);
        let sliding = input.landing_recovery > 0.1;
        gait.step_cooldown = (gait.step_cooldown - dt).max(0.0);

        let hip_ground = |side: f32| pelvis_c + right_yaw * (side * HIP_W);
        let hips_world_est = |i: usize| pelvis_c + base * hips_local[i];

        if gait.was_airborne {
            for i in 0..2 {
                let side = if i == 0 { -1.0f32 } else { 1.0f32 };
                if let Some(g) = ground_at(hip_ground(side)) {
                    gait.feet[i].planted = g;
                }
                gait.feet[i].stepping = false;
                gait.local_ankles[i].vel = Vec3::ZERO;
            }
            gait.step_cooldown = 0.0;
            gait.was_airborne = false;
        }

        if sliding {
            for f in gait.feet.iter_mut() {
                f.stepping = false;
                f.planted += v_flat * dt;
            }
        }

        let any_stepping = gait.feet[0].stepping || gait.feet[1].stepping;
        if !any_stepping && !sliding && gait.step_cooldown <= 0.0 {
            let mut need = [0.0f32; 2];
            for i in 0..2 {
                let side = if i == 0 { -1.0f32 } else { 1.0f32 };
                let rel = (gait.feet[i].planted - hip_ground(side)).reject_from_normalized(up);
                let rel_x = rel.dot(right_yaw);
                let rel_z = rel.dot(fwd_yaw);
                let inward = -side * rel_x;
                let comfort_lat = if inward > 0.0 { inward / 0.10 } else { -inward / 0.30 };
                let reach = (hips_world_est(i) - gait.feet[i].planted).length() / (leg * 0.97);
                let comfort = comfort_lat.max(reach).max(rel_z.abs() / if travelling { 0.55 } else { 0.30 });
                need[i] = comfort.max(if travelling {
                    let behind = -rel.dot(dir);
                    let lateral = (rel - dir * rel.dot(dir)).length();
                    (behind / s_half).max(lateral / 0.30)
                } else {
                    rel.length() / 0.34
                });
            }
            let pick = if need[0] >= need[1] { 0 } else { 1 };
            let thresh = if travelling { 1.0 } else { 1.25 };
            if need[pick] > thresh {
                let side = if pick == 0 { -1.0f32 } else { 1.0f32 };
                let hip_pred = hip_ground(side) + vs * (step_dur * 0.9);
                let target_xz = if travelling { hip_pred + dir * (s_half * 0.9) } else { hip_ground(side) };
                let landing_pt = ground_at(target_xz).or(input.foot_ground_hit[pick]).unwrap_or(target_xz - up * (leg * 0.94 + ANKLE_H));
                let f = &mut gait.feet[pick];
                f.stepping = true;
                f.t = 0.0;
                f.step_dur = step_dur;
                f.from = f.planted;
                f.to = landing_pt;
                gait.last_step_foot = pick;
                gait.step_cooldown = if travelling { step_dur * 0.35 } else { 0.6 };
            }
        }

        for i in 0..2 {
            let f = &mut gait.feet[i];
            if f.stepping {
                f.t += dt / f.step_dur.max(0.08);
                if f.t >= 1.0 {
                    f.stepping = false;
                    f.planted = f.to;
                }
            }

            let foot_world = if f.stepping {
                foot_planted[i] = false;
                let s_t = f.t.clamp(0.0, 1.0);
                let s = s_t * s_t * (3.0 - 2.0 * s_t);
                let step_span = (f.to - f.from).length();
                let lift_h = (0.09 + step_span * 0.22).clamp(0.08, 0.28);
                f.from.lerp(f.to, s) + up * ((s_t * std::f32::consts::PI).sin() * lift_h)
            } else {
                foot_planted[i] = true;
                if let Some(g_pt) = ground_at(f.planted) {
                    f.planted = g_pt;
                }
                f.planted
            };

            let ankle_world = foot_world + up * ANKLE_H;
            target_ankles_local[i] = inv_base * (ankle_world - pelvis_c);
        }
    }

    gait.phase += speed * dt * 2.2;

    // Spine: lean into the run, squat into a jump charge, roll with turns, sway with inertia.
    let spine_target = if airborne {
        match mode {
            MoveMode::RocketShoes => Vec3::new((il.x * 0.45).clamp(-0.34, 0.34), 0.0, (il.z * 0.45).clamp(-0.42, 0.38)),
            MoveMode::Skydive => {
                let arch = fl.wing_arch * 0.24 + 0.14;
                let suit = Vec3::new(-fl.wing_turn * 0.22, 0.0, arch);
                suit.lerp(Vec3::new(0.0, 0.0, 0.30), fl.fall_spread).lerp(Vec3::new(0.0, 0.0, -0.35), fl.fall_tuck)
            }
            _ => Vec3::new(input.strafe * 0.10, 0.0, 0.06),
        }
    } else {
        let run_lean_fwd = -(speed / 7.5).min(1.0) * 0.22;
        let squat_lean = if input.jump_charge > 0.05 { -0.28 * input.jump_charge } else { 0.0 };
        Vec3::new(input.strafe * 0.14, 0.0, run_lean_fwd + squat_lean)
    };
    let spine_inertial = Vec3::new((-local_accel.x * 0.018).clamp(-0.45, 0.45) + (t_sec * 17.0).sin() * 0.05 * wing_amp, 0.0, (-local_accel.z * 0.020).clamp(-0.45, 0.45) + (t_sec * 13.0).cos() * 0.05 * wing_amp);
    gait.spine_bend.step(spine_target, spine_inertial * 20.0, 85.0, 13.0, dt);
    gait.spine_bend.pos.x = gait.spine_bend.pos.x.clamp(-0.34, 0.34);
    gait.spine_bend.pos.z = gait.spine_bend.pos.z.clamp(-0.42, 0.38);

    let spine_dir_local = Vec3::new(gait.spine_bend.pos.x, 1.0, gait.spine_bend.pos.z).normalize();
    let chest_local = spine_dir_local * SPINE;
    let chest = pelvis_c + base * chest_local;

    let twist_target = Vec3::new(
        (local_err.y * 0.55 + input.strafe * if airborne { 0.22 } else { 0.0 }).clamp(-0.75, 0.75),
        (-local_err.y * 0.28).clamp(-0.4, 0.4),
        0.0,
    );
    gait.spine_twist.step(twist_target, Vec3::ZERO, 90.0, 12.0, dt);
    gait.spine_twist.pos.x = gait.spine_twist.pos.x.clamp(-0.8, 0.8);
    gait.spine_twist.pos.y = gait.spine_twist.pos.y.clamp(-0.45, 0.45);
    let chest_twist = Quat::from_rotation_y(gait.spine_twist.pos.x);
    let chest_rot = base * Quat::from_rotation_arc(Vec3::Y, spine_dir_local) * chest_twist;
    let chest_up = chest_rot * Vec3::Y;

    // Head: looks a little with the camera, idles, and lags the body's accelerations.
    let head_target = if airborne {
        match mode {
            MoveMode::Skydive => Vec3::new(0.60 + fl.wing_arch * 0.15, fl.wing_turn * 0.28, fl.wing_turn * 0.15),
            MoveMode::RocketShoes => {
                let pitch_look = if fl.fwd_in > 0.1 {
                    0.55
                } else if fl.fwd_in < -0.1 {
                    -0.25
                } else {
                    0.15
                };
                Vec3::new(pitch_look, -input.strafe * 0.25, -input.strafe * 0.15)
            }
            _ => Vec3::new(-0.42, 0.0, (t_sec * 1.2).sin() * 0.04),
        }
    } else {
        Vec3::new((input.cam_pitch * 0.35).clamp(-0.4, 0.4) + (t_sec * 0.7).sin() * 0.025, -input.strafe * 0.15, (t_sec * 0.45).sin() * 0.03)
    };
    let head_inertial = Vec3::new((local_accel.z * 0.024).clamp(-0.55, 0.55), 0.0, (-local_accel.x * 0.024).clamp(-0.55, 0.55));
    gait.head_angles.step(head_target, head_inertial * 16.0, 95.0, 14.0, dt);
    gait.head_angles.pos.x = gait.head_angles.pos.x.clamp(-0.68, 0.85);
    gait.head_angles.pos.y = gait.head_angles.pos.y.clamp(-0.65, 0.65);
    gait.head_angles.pos.z = gait.head_angles.pos.z.clamp(-0.48, 0.48);

    let neck = chest + chest_up * 0.10;
    let head_c = neck + chest_up * 0.21;
    let head_rot = chest_rot * Quat::from_euler(EulerRot::YXZ, gait.head_angles.pos.y, gait.head_angles.pos.x, gait.head_angles.pos.z);

    let leg_wind_drag = (local_wind * 0.007).clamp_length_max(0.40);
    let leg_inertia = (-local_accel * 0.007).clamp_length_max(0.40);
    let mut ankles_world = [Vec3::ZERO; 2];
    let mut knees_world = [Vec3::ZERO; 2];
    let mut hips_world = [Vec3::ZERO; 2];
    let mut foot_world_pos = [Vec3::ZERO; 2];

    let body_up = base * Vec3::Y;
    for i in 0..2 {
        let side = if i == 0 { -1.0f32 } else { 1.0f32 };
        if airborne {
            gait.local_ankles[i].step(target_ankles_local[i], (leg_wind_drag + leg_inertia) * 14.0, 125.0, 19.0, dt);
        } else {
            let k = 1.0 - (-45.0 * dt).exp();
            let cur = gait.local_ankles[i].pos;
            gait.local_ankles[i].pos = cur.lerp(target_ankles_local[i], k);
            gait.local_ankles[i].vel = Vec3::ZERO;
        }
        gait.local_ankles[i].pos = clamp_ankle_anatomical(hips_local[i], gait.local_ankles[i].pos, side);

        let pole_local = Vec3::new(side * 0.08, 0.10, -1.0).normalize();
        let (knee_loc, ankle_loc) = two_bone(hips_local[i], gait.local_ankles[i].pos, THIGH, SHIN, pole_local);
        hips_world[i] = pelvis_c + base * hips_local[i];
        knees_world[i] = pelvis_c + base * knee_loc;
        ankles_world[i] = pelvis_c + base * ankle_loc;

        foot_world_pos[i] = if !airborne && !gait.feet[i].stepping { gait.feet[i].planted } else { ankles_world[i] - body_up * ANKLE_H };
    }

    let shoulders_local = [chest_local + chest_twist * Vec3::new(-SHOULDER_W, -0.06, 0.0), chest_local + chest_twist * Vec3::new(SHOULDER_W, -0.06, 0.0)];
    let foot_z_diff = (gait.local_ankles[0].pos.z - gait.local_ankles[1].pos.z).clamp(-0.45, 0.45);
    let arm_wind_buffet = Vec3::new((t_sec * 19.0).sin() * 0.04, (t_sec * 23.0).cos() * 0.04, (t_sec * 17.0).sin() * 0.04)
        * (local_wind.length() / 35.0).clamp(0.0, 1.5)
        * if matches!(mode, MoveMode::Skydive | MoveMode::Fall) { 2.5 } else { 1.0 };
    let arm_inertia = (-local_accel * 0.011 + local_wind * 0.005).clamp_length_max(0.55);

    let mut shoulders_world = [Vec3::ZERO; 2];
    let mut elbows_world = [Vec3::ZERO; 2];
    let mut wrists_world = [Vec3::ZERO; 2];

    for i in 0..2 {
        let side = if i == 0 { -1.0f32 } else { 1.0f32 };
        let sh = shoulders_local[i];
        let target_wrist = if airborne {
            match mode {
                MoveMode::RocketShoes => {
                    if fl.fwd_in > 0.1 {
                        let reach = if fl.afterburner { 0.34 } else { 0.26 };
                        sh + Vec3::new(side * 0.16, -0.18, -reach)
                    } else if fl.fwd_in < -0.1 {
                        sh + Vec3::new(side * 0.36, -0.16, 0.24)
                    } else if input.strafe.abs() > 0.1 {
                        let asym = side * input.strafe * 0.14;
                        sh + Vec3::new(side * 0.34, -0.22 + asym, -0.06)
                    } else if fl.afterburner {
                        sh + Vec3::new(side * 0.08, -0.48, 0.05)
                    } else {
                        sh + Vec3::new(side * 0.25, -0.36, 0.04)
                    }
                }
                MoveMode::Skydive => {
                    let dive = (-fl.wing_arch).max(0.0);
                    let sweep_back = dive * 0.24;
                    let tuck_in = dive * 0.14;
                    let flare_z = -fl.wing_arch.max(0.0) * 0.20;
                    let bank_asym = (-side) * fl.wing_turn * 0.16;
                    let suit = Vec3::new(side * (0.50 - tuck_in), -0.06 - sweep_back + bank_asym, 0.06 + sweep_back * 0.5 + flare_z);
                    let eagle = Vec3::new(side * 0.58, 0.18, 0.08);
                    let ball = Vec3::new(side * 0.16, -0.42, -0.30);
                    sh + suit.lerp(eagle, fl.fall_spread).lerp(ball, fl.fall_tuck)
                }
                MoveMode::Parachute => sh + Vec3::new(side * 0.08, 0.46, -0.08),
                _ => sh + Vec3::new(side * 0.44, -0.10 + (-side) * fl.wing_turn * 0.16, -0.05),
            }
        } else if input.jump_charge > 0.05 {
            sh + Vec3::new(side * 0.12, -0.38, 0.28 * input.jump_charge)
        } else if input.jump_push_timer > 0.0 {
            sh + Vec3::new(side * 0.14, -0.15, -0.28)
        } else {
            // Arms swing against the legs.
            let swing_z = side * foot_z_diff * 0.95;
            sh + Vec3::new(side * 0.08, -0.46 + swing_z.abs() * 0.08, swing_z)
        };
        let fling = if mode == MoveMode::RocketShoes { (-local_err.cross(Vec3::new(side * 0.45, -0.25, 0.0)) * 0.55).clamp_length_max(0.5) } else { Vec3::ZERO };
        let target_wrist = target_wrist + fling;
        gait.local_wrists[i].step(target_wrist, (arm_inertia + arm_wind_buffet) * 16.0, 135.0, 18.0, dt);
        gait.local_wrists[i].pos = clamp_wrist_anatomical(sh, gait.local_wrists[i].pos, side);

        let elbow_pole = Vec3::new(side * 0.35, -0.20, 1.0).normalize();
        let (elbow_loc, wrist_loc) = two_bone(sh, gait.local_wrists[i].pos, UPPER_ARM, FOREARM, elbow_pole);
        shoulders_world[i] = pelvis_c + base * sh;
        elbows_world[i] = pelvis_c + base * elbow_loc;
        wrists_world[i] = pelvis_c + base * wrist_loc;
    }

    let joints = [
        pelvis_c,
        chest,
        neck,
        hips_world[0],
        hips_world[1],
        knees_world[0],
        knees_world[1],
        ankles_world[0],
        ankles_world[1],
        shoulders_world[0],
        shoulders_world[1],
        elbows_world[0],
        elbows_world[1],
        wrists_world[0],
        wrists_world[1],
    ];

    let hand_rot = |e: Vec3, w: Vec3| Quat::from_rotation_arc(-body_up, (w - e).normalize_or(-body_up)) * base;
    let foot_rot = |h: Vec3, a: Vec3| {
        if !airborne {
            let n = input.ground_normal.normalize_or(up);
            Quat::from_rotation_arc(Vec3::Y, n) * Quat::from_rotation_y(input.yaw)
        } else {
            Quat::from_rotation_arc(-body_up, (a - h).normalize_or(-body_up)) * base
        }
    };

    let frames = [
        (pelvis_c, base * pelvis_twist),
        (chest, chest_rot),
        (head_c, head_rot),
        (ankles_world[0], foot_rot(hips_world[0], ankles_world[0])),
        (ankles_world[1], foot_rot(hips_world[1], ankles_world[1])),
        (wrists_world[0], hand_rot(elbows_world[0], wrists_world[0])),
        (wrists_world[1], hand_rot(elbows_world[1], wrists_world[1])),
    ];

    let blink = if (t_sec % 3.7) < 0.12 { 0.15 } else { 1.0 };
    let flying = fl.wingsuit_on && !matches!(mode, MoveMode::Walk | MoveMode::Parachute);

    let back = base * Vec3::Z;
    let bulge = 0.05 + wing_amp * 0.03 + (0.012 + 0.02 * wing_amp) * (t_sec * 38.0).sin();
    let side_wing = |i: usize| {
        let (wrist, ankle) = (wrists_world[i], ankles_world[i]);
        let trail = (wrist + ankle) * 0.5 + back * (bulge * 0.6);
        ([shoulders_world[i], elbows_world[i], wrist, trail, ankle, knees_world[i], hips_world[i]], back * bulge)
    };
    let crotch = (hips_world[0] + hips_world[1]) * 0.5 - body_up * 0.04;
    let wing_polys = [
        side_wing(0),
        side_wing(1),
        ([hips_world[0], knees_world[0], ankles_world[0], ankles_world[1], knees_world[1], hips_world[1], crotch], back * (bulge * 0.7)),
    ];
    let (p0_fl, q_fl) = frames[F::FootL as usize];
    let (p0_fr, q_fr) = frames[F::FootR as usize];
    let nozzle_pos = [p0_fl + q_fl * Vec3::new(0.0, -0.10, -0.03), p0_fr + q_fr * Vec3::new(0.0, -0.10, -0.03)];
    let nozzle_dir = [q_fl * Vec3::NEG_Y, q_fr * Vec3::NEG_Y];
    Pose { pelvis_c, base, blink, joints, frames, foot_world_pos, foot_planted, body_up, flying, canopy: mode == MoveMode::Parachute, wing_polys, nozzle_pos, nozzle_dir, throttle: fl.throttle, time: t_sec }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PELVIS_H: f32 = 0.95;

    /// Flat ground at y = 0.
    fn flat(origin: Vec3, max: f32) -> Option<f32> {
        let d = origin.y;
        (0.0..=max).contains(&d).then_some(d)
    }

    /// Runs the rig for `secs` at 60 Hz with the pelvis moving at `speed` along -z (yaw 0).
    fn run(speed: f32, secs: f32, gait: &mut Gait) -> Vec<Pose> {
        let dt = 1.0 / 60.0;
        let mut out = Vec::new();
        let mut z = 0.0f32;
        for k in 0..(secs / dt) as usize {
            z -= speed * dt;
            let input = RigInput { vel: Vec3::new(0.0, 0.0, -speed), ground_normal: Vec3::Y, ..Default::default() };
            out.push(solve(&input, gait, Vec3::new(0.0, PELVIS_H, z), Quat::IDENTITY, dt, k as f32 * dt, &flat));
        }
        out
    }

    #[test]
    fn no_nan_at_zero_speed_and_after() {
        let mut g = Gait::default();
        for p in run(0.0, 10.0, &mut g).into_iter().chain(run(2.2, 3.0, &mut g)).chain(run(0.0, 3.0, &mut g)) {
            assert!(p.joints.iter().all(|j| j.is_finite()), "{:?}", p.joints);
            assert!(p.frames.iter().all(|(a, q)| a.is_finite() && q.is_finite() && (q.length() - 1.0).abs() < 1e-3));
        }
    }

    #[test]
    fn deterministic() {
        let (mut a, mut b) = (Gait::default(), Gait::default());
        let (pa, pb) = (run(3.0, 4.0, &mut a), run(3.0, 4.0, &mut b));
        for (x, y) in pa.iter().zip(&pb) {
            assert_eq!(x.joints, y.joints);
            assert_eq!(x.frames, y.frames);
        }
    }

    #[test]
    fn feet_stay_planted_during_stance_and_steps_alternate() {
        let mut g = Gait::default();
        let poses = run(2.2, 6.0, &mut g);
        let mut steps = [0u32; 2];
        let mut order = Vec::new();
        for w in poses.windows(2).skip(60) {
            for i in 0..2 {
                if w[0].foot_planted[i] && w[1].foot_planted[i] {
                    let d = (w[1].foot_world_pos[i] - w[0].foot_world_pos[i]).length();
                    assert!(d < 1e-4, "foot {i} slid {d} m while planted");
                    // The planted foot is on the ground.
                    assert!(w[1].foot_world_pos[i].y.abs() < 1e-4);
                }
                if w[0].foot_planted[i] && !w[1].foot_planted[i] {
                    steps[i] += 1;
                    order.push(i);
                }
            }
        }
        assert!(steps[0] >= 4 && steps[1] >= 4, "steps {steps:?}");
        // One foot at a time: never both lifted together.
        assert!(poses.iter().all(|p| p.foot_planted[0] || p.foot_planted[1]), "both feet airborne at once");
        assert!(!order.is_empty());
    }

    #[test]
    fn legs_reach_the_ground_and_keep_their_lengths() {
        let mut g = Gait::default();
        for p in run(2.2, 5.0, &mut g).iter().skip(30) {
            for i in 0..2 {
                let (hip, knee, ankle) = (p.joints[3 + i], p.joints[5 + i], p.joints[7 + i]);
                assert!(((hip - knee).length() - THIGH).abs() < 1e-3);
                assert!(((knee - ankle).length() - SHIN).abs() < 1e-3);
                if p.foot_planted[i] && p.foot_world_pos[i].y.abs() < 1e-4 && (p.joints[7 + i].y - ANKLE_H).abs() < 0.2 {
                    // Ankle sits ANKLE_H above the planted sole; the leg reaches it.
                    assert!((ankle.y - ANKLE_H).abs() < 0.2, "ankle {} above ground", ankle.y);
                }
            }
        }
    }

    #[test]
    fn standing_still_settles_with_both_feet_down() {
        let mut g = Gait::default();
        let poses = run(0.0, 3.0, &mut g);
        let p = poses.last().unwrap();
        assert!(p.foot_planted == [true, true]);
        for i in 0..2 {
            assert!((p.joints[7 + i].y - ANKLE_H).abs() < 0.04);
        }
    }
}
