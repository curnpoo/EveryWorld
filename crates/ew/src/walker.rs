//! Procedural walk for the play character (P-slice S4), ported from the alpha robot
//! (`alpha-early:crates/ed_game/src/body.rs`, `solve_rig`): a footstep planner plants each foot
//! on the ground in the world and swings it to the next foothold on an arc, two-bone IK puts
//! hip, knee and ankle between the pelvis and the planted foot, the pelvis drops to whatever
//! height both legs can reach (that is the walk's bob), the torso leans with speed and twists
//! against the hips, and the arms swing against the legs. Walk and run differ by stride, step
//! time, lean, crouch and arm carriage.
//!
//! Rendering only: nothing here feeds the sim. The planner runs every rendered frame on the
//! interpolated body position, so it is smooth at any display rate.
//!
//! Frames: world positions are the sim frame (ENU metres, z up, `DVec3`). The skeleton's model
//! space is x = the character's left, y = up, z = forward; the model turns by `heading` about
//! render y (render = (E, U, -N)), and its origin is [`Walker::root`]. Joints are found by their
//! Rigify names (`thigh.L`, `shin.L`, `foot.L`, `upper_arm.L`, `forearm.L`, `spine.001..003`).

use ed_pack::kinds::Skeleton;
use glam::{DVec2, DVec3, Mat4, Quat, Vec3};

/// One foot's state in the world (sim frame).
#[derive(Clone, Copy, Debug, Default)]
struct Foot {
    /// Ground contact point while planted.
    planted: DVec3,
    from: DVec3,
    to: DVec3,
    /// Swing progress 0..1 and its duration (s).
    t: f32,
    dur: f32,
    stepping: bool,
}

#[derive(Clone, Copy, Debug)]
struct Joints {
    spine: [usize; 3],
    thigh: [usize; 2],
    shin: [usize; 2],
    foot: [usize; 2],
    arm: [usize; 2],
    fore: [usize; 2],
    hand: [Option<usize>; 2],
}

pub struct Walker {
    sk: Skeleton,
    rest_t: Vec<Vec3>,
    rest_r: Vec<Quat>,
    rest_s: Vec<Vec3>,
    /// Global rest rotation and position of each joint (model space).
    g_rot: Vec<Quat>,
    j: Joints,
    hip_y: f32,
    ankle_h: f32,
    thigh: f32,
    shin: f32,
    arm: [(f32, f32); 2],
    feet: [Foot; 2],
    init: bool,
    was_air: bool,
    vs: DVec2,
    moving: bool,
    cooldown: f32,
    idle: f32,
    air_t: f32,
    air: f32,
    /// Pelvis height above the ground under the body (smoothed).
    pelvis_off: f64,
    lean: f32,
    twist: f32,
    sway: f32,
    wrist: [Vec3; 2],
    heading: f32,
    time: f32,
    locals: Vec<Mat4>,
    /// Model-space origin in the world (sim frame): the render node goes here.
    pub root: DVec3,
    /// Largest miss (m) between a planted foot's ankle target and where the leg put it this
    /// frame (0 = the foot is exactly on its foothold).
    pub slip: f32,
    /// Steps taken (diagnostics).
    pub steps: u64,
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Two-bone IK (from the alpha): knee (or elbow) and end positions for a chain of `l1`, `l2`
/// from `root` toward `target`, bending toward `pole`.
pub fn two_bone(root: Vec3, target: Vec3, l1: f32, l2: f32, pole: Vec3) -> (Vec3, Vec3) {
    let to = target - root;
    let d = to.length().clamp((l1 - l2).abs() + 0.02, l1 + l2 - 0.002);
    let dir = to.normalize_or(Vec3::NEG_Y);
    let end = root + dir * d;
    let cos_a = ((l1 * l1 + d * d - l2 * l2) / (2.0 * l1 * d)).clamp(-1.0, 1.0);
    let a = cos_a.acos();
    let bend = pole.reject_from_normalized(dir).normalize_or(Vec3::Z);
    let mid = root + (dir * a.cos() + bend * a.sin()) * l1;
    (mid, end)
}

/// Sim-frame direction of a model-space vector for a character facing `heading`.
fn model_dir_to_sim(heading: f32, m: Vec3) -> DVec3 {
    let r = Quat::from_rotation_y(heading) * m;
    DVec3::new(r.x as f64, -r.z as f64, r.y as f64)
}

/// Model-space direction of a sim-frame vector.
fn sim_dir_to_model(heading: f32, d: DVec3) -> Vec3 {
    Quat::from_rotation_y(-heading) * Vec3::new(d.x as f32, d.z as f32, -d.y as f32)
}

const WALK_SPEED: f32 = 2.2;
const RUN_SPEED: f32 = 5.0;

impl Walker {
    /// `None` when the skeleton lacks the legs (the caller keeps the rest pose).
    pub fn new(sk: &Skeleton) -> Option<Walker> {
        let find = |n: &str| sk.joints.iter().position(|j| j.name.eq_ignore_ascii_case(n));
        let j = Joints {
            spine: [find("spine.001")?, find("spine.002")?, find("spine.003")?],
            thigh: [find("thigh.L")?, find("thigh.R")?],
            shin: [find("shin.L")?, find("shin.R")?],
            foot: [find("foot.L")?, find("foot.R")?],
            arm: [find("upper_arm.L")?, find("upper_arm.R")?],
            fore: [find("forearm.L")?, find("forearm.R")?],
            hand: [find("hand.L"), find("hand.R")],
        };
        let n = sk.joints.len();
        let mut rest_t = Vec::with_capacity(n);
        let mut rest_r = Vec::with_capacity(n);
        let mut rest_s = Vec::with_capacity(n);
        let mut g_rot: Vec<Quat> = Vec::with_capacity(n);
        let mut g_pos: Vec<Vec3> = Vec::with_capacity(n);
        for jt in &sk.joints {
            let t = Vec3::from(jt.rest.translation);
            let r = Quat::from_array(jt.rest.rotation).normalize();
            rest_t.push(t);
            rest_r.push(r);
            rest_s.push(Vec3::from(jt.rest.scale));
            match jt.parent.map(|p| p as usize).filter(|p| *p < g_rot.len()) {
                Some(p) => {
                    g_pos.push(g_pos[p] + g_rot[p] * t);
                    g_rot.push(g_rot[p] * r);
                }
                None => {
                    g_pos.push(t);
                    g_rot.push(r);
                }
            }
        }
        let hip_y = 0.5 * (g_pos[j.thigh[0]].y + g_pos[j.thigh[1]].y);
        let ankle_h = (0.5 * (g_pos[j.foot[0]].y + g_pos[j.foot[1]].y)).max(0.02);
        let thigh = g_pos[j.shin[0]].distance(g_pos[j.thigh[0]]);
        let shin = g_pos[j.foot[0]].distance(g_pos[j.shin[0]]);
        let arm = [0, 1].map(|i| {
            let wrist = j.hand[i].map(|h| g_pos[h]).unwrap_or(g_pos[j.fore[i]] + (g_pos[j.fore[i]] - g_pos[j.arm[i]]));
            (g_pos[j.fore[i]].distance(g_pos[j.arm[i]]), wrist.distance(g_pos[j.fore[i]]))
        });
        Some(Walker {
            sk: sk.clone(),
            rest_t,
            rest_r,
            rest_s,
            g_rot,
            j,
            hip_y,
            ankle_h,
            thigh,
            shin,
            arm,
            feet: [Foot::default(); 2],
            init: false,
            was_air: false,
            vs: DVec2::ZERO,
            moving: false,
            cooldown: 0.0,
            idle: 0.0,
            air_t: 0.0,
            air: 0.0,
            pelvis_off: hip_y as f64,
            lean: 0.0,
            twist: 0.0,
            sway: 0.0,
            wrist: [Vec3::ZERO; 2],
            heading: 0.0,
            time: 0.0,
            locals: ed_anim::rest_locals(sk),
            root: DVec3::ZERO,
            slip: 0.0,
            steps: 0,
        })
    }

    fn leg(&self) -> f32 {
        self.thigh + self.shin
    }

    /// Model-space x of the hips (left positive).
    fn hip_x(&self, i: usize) -> f32 {
        let x = self.rest_t[self.j.thigh[i]].x;
        if i == 0 { x.abs() } else { -x.abs() }
    }

    /// Where foot `i` stands at rest, on the ground plane under the body (sim frame, z = body z).
    fn home(&self, body: DVec3, i: usize) -> DVec3 {
        body + model_dir_to_sim(self.heading, Vec3::new(self.hip_x(i) * 0.82, 0.0, 0.0))
    }

    /// Advances the gait by `dt` (s) and poses the skeleton. `body` is the interpolated sim
    /// feet position, `vel` the sim velocity, `heading` the drawn facing, `ground` the terrain
    /// height (sim z) at a horizontal position, matching what is drawn.
    pub fn update(&mut self, dt: f32, body: DVec3, heading: f32, vel: DVec3, grounded: bool, ground: &dyn Fn(DVec2) -> Option<f64>) {
        let dt = dt.clamp(0.0, 0.1);
        self.time += dt;
        self.heading = heading;
        let up = DVec3::Z;
        let leg = self.leg();
        let ground_at = |p: DVec3| ground(p.truncate()).map(|z| DVec3::new(p.x, p.y, z));
        let g_body = ground(body.truncate()).unwrap_or(body.z);
        self.air_t = if grounded { 0.0 } else { self.air_t + dt };
        let airborne = self.air_t > 0.12;
        let ka = 1.0 - (-dt / if airborne { 0.10 } else { 0.05 }).exp();
        self.air += (airborne as u8 as f32 - self.air) * ka;
        if !self.init {
            for i in 0..2 {
                let h = self.home(body, i);
                self.feet[i] = Foot { planted: ground_at(h).unwrap_or(h), dur: 0.3, ..Default::default() };
            }
            self.pelvis_off = (body.z - g_body).max(0.0) + self.hip_y as f64 * 0.98;
            self.init = true;
        }

        // The footstep planner (alpha `solve_rig`, ground branch), with human step timing.
        let v_flat = DVec2::new(vel.x, vel.y);
        self.vs = self.vs.lerp(v_flat, 1.0 - (-9.0 * dt as f64).exp());
        // Step timing and footholds use the faster of the smoothed and the current speed, so
        // the first step from standing is sized for where the body is going.
        let speed = self.vs.length().max(v_flat.length()) as f32;
        self.moving = if self.moving { speed > 0.2 } else { speed > 0.45 };
        let run = smooth((speed - WALK_SPEED) / (RUN_SPEED - WALK_SPEED));
        let fwd = model_dir_to_sim(heading, Vec3::Z);
        let right = model_dir_to_sim(heading, Vec3::NEG_X);
        if airborne {
            self.was_air = true;
            for f in &mut self.feet {
                f.stepping = false;
            }
        } else {
            if self.was_air {
                for i in 0..2 {
                    let h = self.home(body, i);
                    self.feet[i] = Foot { planted: ground_at(h).unwrap_or(h), dur: 0.3, ..Default::default() };
                }
                self.cooldown = 0.0;
                self.was_air = false;
            }
            let travelling = self.moving;
            let dir = self.vs.lerp(v_flat, 0.5).normalize_or_zero().extend(0.0);
            let vs3 = dir * speed as f64;
            // Half a stride (how far a foot trails before it steps) and the swing time.
            let s_half = (0.16 + 0.15 * speed).clamp(0.16, 0.62) as f64;
            // One foot swings at a time, so a swing must end before the planted foot trails
            // out of reach: the body may cover at most ~1.85 half-strides during it.
            let step_dur = (0.52 - 0.06 * speed).clamp(0.22, 0.42).min(1.8 * s_half as f32 / speed.max(0.1)).max(0.12);
            self.cooldown = (self.cooldown - dt).max(0.0);
            self.idle = if travelling { 0.0 } else { self.idle + dt };
            let pelvis_z = g_body + self.pelvis_off;
            let any_stepping = self.feet[0].stepping || self.feet[1].stepping;
            // A planted foot that the body is about to leave behind steps even while the other
            // is still in the air (a run's overlap; also the first step of a sprint).
            if (!any_stepping && self.cooldown <= 0.0) || (any_stepping && self.feet.iter().any(|f| !f.stepping)) {
                let mut need = [0.0f64; 2];
                let mut settle = [0.0f64; 2];
                for (i, n) in need.iter_mut().enumerate() {
                    let side = if i == 0 { -1.0 } else { 1.0 }; // -1: left
                    let home = self.home(body, i);
                    let rel = self.feet[i].planted - home;
                    let rel = DVec3::new(rel.x, rel.y, 0.0);
                    let rel_x = rel.dot(right);
                    let rel_z = rel.dot(fwd);
                    let inward = -side * rel_x;
                    let comfort_lat = if inward > 0.0 { inward / 0.10 } else { -inward / 0.30 };
                    let hip_w = DVec3::new(home.x, home.y, pelvis_z);
                    let reach = (hip_w - self.feet[i].planted).length() / (leg as f64 * 1.02);
                    let comfort = comfort_lat.max(reach).max(rel_z.abs() / if travelling { 0.75 } else { 0.30 });
                    *n = comfort.max(if travelling {
                        let behind = -rel.dot(dir);
                        let lateral = (rel - dir * rel.dot(dir)).length();
                        (behind / s_half).max(lateral / 0.30)
                    } else {
                        rel.length() / 0.34
                    });
                    settle[i] = rel.length();
                    if self.feet[i].stepping {
                        *n = -1.0;
                        settle[i] = -1.0;
                    }
                }
                let pick = if need[0] >= need[1] { 0 } else { 1 };
                let thresh = if any_stepping { 1.5 } else if travelling { 1.0 } else { 1.25 };
                // Standing still a moment: bring a stray foot back under the hip.
                let settle_pick = if settle[0] >= settle[1] { 0 } else { 1 };
                let (pick, go) = if need[pick] > thresh {
                    (pick, true)
                } else if !any_stepping && !travelling && self.idle > 0.25 && settle[settle_pick] > 0.09 {
                    (settle_pick, true)
                } else {
                    (pick, false)
                };
                if go {
                    let dur = if travelling { step_dur } else { 0.32 };
                    let home = self.home(body, pick);
                    let target = if travelling { home + vs3 * (dur as f64 * 0.9) + dir * (s_half * 0.9) } else { home };
                    let to = ground_at(target).unwrap_or(DVec3::new(target.x, target.y, g_body));
                    let f = &mut self.feet[pick];
                    *f = Foot { from: f.planted, to, t: 0.0, dur, stepping: true, planted: f.planted };
                    self.cooldown = if travelling { dur * 0.1 } else { 0.15 };
                    self.steps += 1;
                }
            }
            for f in &mut self.feet {
                if f.stepping {
                    f.t += dt / f.dur.max(0.08);
                    // The foothold may be refined while the foot is in the air.
                    if let Some(g) = ground_at(f.to) {
                        f.to = g;
                    }
                    if f.t >= 1.0 {
                        f.stepping = false;
                        f.planted = f.to;
                    }
                } else if let Some(g) = ground_at(f.planted) {
                    f.planted = g;
                }
            }
        }

        // Foot points, foot pitch and ground normals (world).
        let mut ankle_w = [DVec3::ZERO; 2];
        let mut pitch = [0.0f32; 2];
        let mut normal = [Vec3::Y; 2];
        let mut planted = [false; 2];
        for i in 0..2 {
            let f = self.feet[i];
            let (pt, p) = if f.stepping {
                let t = f.t.clamp(0.0, 1.0);
                let s = t * t * (3.0 - 2.0 * t);
                let span = (f.to - f.from).truncate().length() as f32;
                let lift = (0.07 + span * 0.12 + 0.06 * run).clamp(0.06, 0.24);
                let arc = (t * std::f32::consts::PI).sin();
                // Toe down as the heel leaves, toe up before the heel strikes.
                let p = 0.55 * (1.0 - t).powi(3) * arc * 2.0 - 0.35 * t * t * arc;
                (f.from.lerp(f.to, s as f64) + up * (arc * lift) as f64, p)
            } else {
                planted[i] = !airborne;
                // A planted foot well behind the hip rolls onto its toe (heel off).
                let rel = (f.planted - self.home(body, i)).dot(fwd) as f32;
                let heel = if self.moving { ((-rel - 0.14) * 0.9).clamp(0.0, 0.35) } else { 0.0 };
                (f.planted, heel)
            };
            // Ground normal under the foot from the terrain (only near the ground).
            let n = {
                let e = 0.18;
                match (ground(pt.truncate() + DVec2::X * e), ground(pt.truncate() - DVec2::X * e), ground(pt.truncate() + DVec2::Y * e), ground(pt.truncate() - DVec2::Y * e)) {
                    (Some(a), Some(b), Some(c), Some(d)) => DVec3::new(-(a - b) / (2.0 * e), -(c - d) / (2.0 * e), 1.0).normalize(),
                    _ => up,
                }
            };
            let nm = sim_dir_to_model(heading, n);
            // Soften the tilt on steep ground (an ankle bends ~25 degrees).
            let nm = Vec3::Y.lerp(nm, 0.8).normalize();
            normal[i] = nm;
            pitch[i] = p;
            // Toe-off lifts the ankle by the heel's rise.
            let heel_rise = if f.stepping { 0.0 } else { p.sin() * 0.16 };
            ankle_w[i] = pt + n * (self.ankle_h as f64) + up * heel_rise as f64;
        }

        // Pelvis height: as high as standing allows, no higher than both planted legs reach.
        let crouch = (0.025 + 0.05 * run) * smooth(speed / 1.0);
        let mut target = self.hip_y as f64 * 0.985 - crouch as f64;
        for i in 0..2 {
            if !planted[i] {
                continue;
            }
            let hip = self.home(body, i);
            let dh = (ankle_w[i] - hip).truncate().length();
            let l = leg as f64 * 0.985;
            if dh < l {
                target = target.min(ankle_w[i].z + (l * l - dh * dh).sqrt() - g_body);
            }
        }
        if airborne {
            self.pelvis_off = body.z - g_body + self.hip_y as f64;
        } else {
            // Drop fast (a leg must reach its foot), rise slower (push-off).
            let rate = if target < self.pelvis_off { 28.0 } else { 10.0 };
            self.pelvis_off += (target - self.pelvis_off) * (1.0 - (-rate * dt as f64).exp());
            self.pelvis_off = self.pelvis_off.min(target + 0.002).max(target - 0.12);
        }
        let pelvis_z = if airborne { body.z + self.hip_y as f64 } else { g_body + self.pelvis_off };
        self.root = DVec3::new(body.x, body.y, pelvis_z - self.hip_y as f64);

        // Torso: lean with speed and acceleration, twist against the hips, sway over the stance foot.
        let accel = ((v_flat - self.vs).dot(DVec2::new(fwd.x, fwd.y)) as f32).clamp(-2.0, 2.0);
        let lean_t = (0.04 + 0.16 * run) * smooth(speed / 1.2) + 0.04 * accel + 0.12 * self.air;
        self.lean += (lean_t - self.lean) * (1.0 - (-8.0 * dt).exp());
        let am: [Vec3; 2] = [0, 1].map(|i| self.to_model(ankle_w[i]));
        let twist_t = if airborne { 0.0 } else { (-0.35 * (am[0].z - am[1].z)).clamp(-0.22, 0.22) * smooth(speed / 1.0) };
        self.twist += (twist_t - self.twist) * (1.0 - (-14.0 * dt).exp());
        let sway_t = match (self.feet[0].stepping, self.feet[1].stepping) {
            (true, false) => -0.022,
            (false, true) => 0.022,
            _ => 0.0,
        } * (1.0 - run) * (1.0 - self.air);
        self.sway += (sway_t - self.sway) * (1.0 - (-7.0 * dt).exp());

        self.pose(&am, &normal, &pitch, &planted, run, speed, dt);
    }

    fn to_model(&self, p: DVec3) -> Vec3 {
        sim_dir_to_model(self.heading, p - self.root)
    }

    #[allow(clippy::too_many_arguments)]
    fn pose(&mut self, ankle: &[Vec3; 2], normal: &[Vec3; 2], pitch: &[f32; 2], planted: &[bool; 2], run: f32, speed: f32, dt: f32) {
        let j = self.j;
        let n = self.sk.joints.len();
        let mut q = vec![Quat::IDENTITY; n];
        let mut pos = vec![Vec3::ZERO; n];
        let leg = self.leg();
        let pelvis_c = self.rest_t[j.spine[0]];
        let twist = Quat::from_rotation_y(self.twist);
        let sway = Vec3::new(self.sway, 0.0, 0.0);
        // Legs first (model space), so the chain pass below only aims bones.
        let mut knee = [Vec3::ZERO; 2];
        let mut foot = [Vec3::ZERO; 2];
        let mut hip = [Vec3::ZERO; 2];
        self.slip = 0.0;
        for i in 0..2 {
            let side = if i == 0 { 1.0 } else { -1.0 }; // model x: left positive
            let h0 = self.rest_t[j.thigh[i]];
            hip[i] = pelvis_c + twist * (h0 - pelvis_c) + sway;
            let tuck = hip[i] + Vec3::new(side * 0.03, -leg * 0.88, 0.10);
            let mut a = ankle[i].lerp(tuck, self.air);
            a.y = a.y.min(hip[i].y - 0.2);
            let pole = Vec3::new(side * 0.12, 0.0, 1.0);
            let (k, e) = two_bone(hip[i], a, self.thigh, self.shin, pole);
            if planted[i] {
                self.slip = self.slip.max(e.distance(a));
            }
            knee[i] = k;
            foot[i] = e;
        }
        // Arms: hang, swing against the legs; elbows bend and carry higher when running.
        let w = smooth(speed / 1.0) * (1.0 - self.air);
        let mut wrist_t = [Vec3::ZERO; 2];
        for i in 0..2 {
            let side = if i == 0 { 1.0 } else { -1.0 };
            let (l1, l2) = self.arm[i];
            let swing = ((ankle[1 - i].z - ankle[i].z) * (0.45 + 0.25 * run) * w).clamp(-0.4, 0.4);
            let walk = Vec3::new(side * 0.05, -(l1 + l2) * 0.93, 0.03 + swing);
            let runp = Vec3::new(side * 0.02, -l1 * 0.9, l2 * 0.75 + swing * 0.9);
            let air = Vec3::new(side * 0.28, -(l1 + l2) * 0.6, 0.08);
            wrist_t[i] = walk.lerp(runp, run * w).lerp(air, self.air);
        }
        if self.wrist == [Vec3::ZERO; 2] {
            self.wrist = wrist_t;
        }
        let k = 1.0 - (-16.0 * dt).exp();
        for i in 0..2 {
            self.wrist[i] = self.wrist[i].lerp(wrist_t[i], k);
        }
        let breathe = 0.015 * (1.0 - w) * (self.time * std::f32::consts::TAU * 0.25).sin();

        for idx in 0..n {
            let parent = self.sk.joints[idx].parent.map(|p| p as usize).filter(|p| *p < idx);
            let (pq, pp) = parent.map(|p| (q[p], pos[p])).unwrap_or((Quat::IDENTITY, Vec3::ZERO));
            let inh = pq * self.rest_r[idx];
            let mut here = pp + pq * self.rest_t[idx];
            let aim = |inh: Quat, from: Vec3, to: Vec3| Quat::from_rotation_arc(inh * Vec3::Y, (to - from).normalize_or(inh * Vec3::Y)) * inh;
            let r = if idx == j.spine[0] {
                here += sway;
                Quat::from_rotation_y(self.twist * 0.5) * Quat::from_rotation_x(self.lean * 0.5) * inh
            } else if idx == j.spine[1] {
                Quat::from_rotation_x(self.lean * 0.3 + breathe) * inh
            } else if idx == j.spine[2] {
                Quat::from_rotation_y(-self.twist * 1.6) * Quat::from_rotation_x(self.lean * 0.2 - breathe) * inh
            } else if let Some(i) = j.thigh.iter().position(|t| *t == idx) {
                here = hip[i];
                aim(inh, hip[i], knee[i])
            } else if let Some(i) = j.shin.iter().position(|t| *t == idx) {
                aim(inh, knee[i], foot[i])
            } else if let Some(i) = j.foot.iter().position(|t| *t == idx) {
                Quat::from_rotation_arc(Vec3::Y, normal[i]) * Quat::from_rotation_x(pitch[i]) * self.g_rot[idx]
            } else if let Some(i) = j.arm.iter().position(|t| *t == idx) {
                let (l1, l2) = self.arm[i];
                let side = if i == 0 { 1.0 } else { -1.0 };
                let (elbow, _) = two_bone(here, here + self.wrist[i], l1, l2, Vec3::new(side * 0.35, 0.0, -1.0));
                aim(inh, here, elbow)
            } else if let Some(i) = j.fore.iter().position(|t| *t == idx) {
                let sh = pos[j.arm[i]];
                let (l1, l2) = self.arm[i];
                let side = if i == 0 { 1.0 } else { -1.0 };
                let (_, wr) = two_bone(sh, sh + self.wrist[i], l1, l2, Vec3::new(side * 0.35, 0.0, -1.0));
                aim(inh, here, wr)
            } else {
                inh
            };
            q[idx] = r;
            pos[idx] = here;
            let local_r = pq.inverse() * r;
            let local_t = match parent {
                Some(_) => self.rest_t[idx],
                None => here,
            };
            self.locals[idx] = Mat4::from_scale_rotation_translation(self.rest_s[idx], local_r.normalize(), local_t);
        }
    }

    /// The skinning palette for the current pose.
    pub fn palette(&self) -> Vec<Mat4> {
        ed_anim::palette(&self.sk, &self.locals)
    }

    /// World (sim) ankle positions of the current pose, for checks.
    pub fn planted_feet(&self) -> [Option<DVec3>; 2] {
        self.feet.map(|f| (!f.stepping && self.init).then_some(f.planted))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed_pack::kinds::{Joint, Transform};

    fn joint(name: &str, parent: Option<u32>, t: [f32; 3], r: Quat) -> Joint {
        Joint { parent, name: name.into(), rest: Transform { translation: t, rotation: r.to_array(), scale: [1.0; 3] }, inverse_bind: Mat4::IDENTITY.to_cols_array() }
    }

    /// A Rigify-shaped skeleton: legs straight down, arms out sideways.
    fn rig() -> Skeleton {
        let down = Quat::from_rotation_x(std::f32::consts::PI);
        let left = Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2);
        let right = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let joints = vec![
            joint("spine.001", None, [0.0, 1.16, 0.0], Quat::IDENTITY),
            joint("thigh.L", None, [0.13, 1.07, 0.0], down),
            joint("thigh.R", None, [-0.13, 1.07, 0.0], down),
            joint("spine.002", Some(0), [0.0, 0.13, 0.0], Quat::IDENTITY),
            joint("shin.L", Some(1), [0.0, 0.44, 0.0], Quat::IDENTITY),
            joint("shin.R", Some(2), [0.0, 0.44, 0.0], Quat::IDENTITY),
            joint("spine.003", Some(3), [0.0, 0.18, 0.0], Quat::IDENTITY),
            joint("foot.L", Some(4), [0.0, 0.57, 0.0], Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
            joint("foot.R", Some(5), [0.0, 0.57, 0.0], Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
            joint("upper_arm.L", Some(6), [0.17, 0.14, 0.0], left),
            joint("upper_arm.R", Some(6), [-0.17, 0.14, 0.0], right),
            joint("forearm.L", Some(9), [0.0, 0.33, 0.0], Quat::IDENTITY),
            joint("forearm.R", Some(10), [0.0, 0.33, 0.0], Quat::IDENTITY),
            joint("hand.L", Some(11), [0.0, 0.30, 0.0], Quat::IDENTITY),
            joint("hand.R", Some(12), [0.0, 0.30, 0.0], Quat::IDENTITY),
        ];
        Skeleton { mesh: Default::default(), joints }
    }

    /// Walks up a 38 degree slope at walking pace: planted feet never move in the world, the
    /// legs always reach them, and feet alternate.
    fn walk_on(slope_deg: f64, speed: f64) -> (f32, f64, u64) {
        let tan = slope_deg.to_radians().tan();
        let ground = move |p: DVec2| Some(p.y * tan);
        let mut w = Walker::new(&rig()).unwrap();
        let dt = 1.0 / 120.0;
        let heading = std::f32::consts::PI; // facing +y (north) in the sim frame
        assert!((model_dir_to_sim(heading, Vec3::Z) - DVec3::Y).length() < 1e-6);
        let mut body = DVec3::ZERO;
        let mut worst_slip = 0.0f32;
        let mut worst_drift = 0.0f64;
        let mut last = [None; 2];
        for k in 0..(6.0 / dt) as usize {
            let v = if k < 30 { DVec3::ZERO } else { DVec3::new(0.0, speed, speed * tan) };
            body += v * dt as f64;
            body.z = body.y * tan;
            w.update(dt as f32, body, heading, v, true, &ground);
            worst_slip = worst_slip.max(w.slip);
            let now = w.planted_feet();
            for i in 0..2 {
                if let (Some(a), Some(b)) = (last[i], now[i]) {
                    let d: DVec3 = a - b;
                    worst_drift = worst_drift.max(d.length());
                }
                if let Some(p) = now[i] {
                    assert!((p.z - p.y * tan).abs() < 1e-6, "planted foot off the ground");
                }
            }
            last = now;
            let _ = w.palette();
        }
        (worst_slip, worst_drift, w.steps)
    }

    #[test]
    fn feet_stay_planted_walking_and_running() {
        for (slope, speed) in [(0.0, 1.5), (0.0, 2.6), (0.0, 6.0), (20.0, 1.9), (38.0, 1.9), (38.0, 1.2), (30.0, 5.5), (0.0, 1.9)] {
            let (slip, drift, steps) = walk_on(slope, speed);
            assert!(slip < 0.03, "slope {slope} speed {speed}: leg missed its foothold by {slip:.3} m");
            assert!(drift < 1e-9, "slope {slope} speed {speed}: planted foot slid {drift:.4} m");
            let dist = speed * 5.75;
            // A step every 0.5..1.2 m travelled.
            assert!(steps as f64 > dist / 1.2 && (steps as f64) < dist / 0.4 + 4.0, "slope {slope} speed {speed}: {steps} steps over {dist:.1} m");
        }
    }

    #[test]
    fn standing_settles_feet_under_the_hips() {
        let ground = |_: DVec2| Some(0.0);
        let mut w = Walker::new(&rig()).unwrap();
        let mut body = DVec3::ZERO;
        for k in 0..600 {
            let v = if k < 240 { DVec3::new(2.0, 0.0, 0.0) } else { DVec3::ZERO };
            body += v / 120.0;
            w.update(1.0 / 120.0, body, 0.0, v, true, &ground);
        }
        for f in w.planted_feet() {
            let f = f.expect("both feet planted when standing");
            assert!((f - body).truncate().length() < 0.25, "foot {f:?} body {body:?}");
        }
    }
}
