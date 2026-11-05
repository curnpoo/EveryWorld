//! Human-flyer physics: wingsuit aerodynamics and rocket-boot thrust vectoring.
//!
//! Everything here produces *physical* forces and torques that come from the body:
//! the wingsuit turns air into lift, drag and moments depending on how the flyer
//! shapes it (arch, twist), and the rocket boots push only through the soles, so the
//! body is steered by swinging the legs (moving the thrust line off the centre of mass).
//! No invisible "attitude hold" torque: a control input changes the pose, the pose
//! changes the forces.
//!
//! Body frame (same as the player rigid body): +Y = head, +X = right hand, +Z = back.
//! In belly-to-earth flight the head points along the flight path and the back faces the sky.

use glam::{Quat, Vec3};

/// Wingsuit planform (all members inflated).
pub const WING_AREA: f32 = 1.6;
/// Reference chord (pitch / yaw moments), metres.
pub const WING_CHORD: f32 = 1.0;
/// Reference span (roll moments), metres.
pub const WING_SPAN: f32 = 1.8;
/// Neutral trim angle of attack, radians (≈ 11°): a ~35 m/s, 2.5:1 glide at sea level.
pub const TRIM_NEUTRAL: f32 = 0.20;
/// Trim range the flyer can reach by reshaping the suit (arch command ±1). Negative = the
/// flyer pushes the nose under (lift points at the ground) to pitch over into a dive; the top
/// end is a hard pull that can loop the suit when fast.
pub const TRIM_RANGE: (f32, f32) = (-0.30, 0.85);
/// Steady roll rate at full twist, rad/s (a barrel roll in under 2 s).
pub const WING_ROLL_RATE: f32 = 3.6;
/// Extra arch the auto-pilot adds while carving a turn onto the view.
const PULL: f32 = 0.9;

/// Cap on aerodynamic rate gains (1/s): damping/inertia never exceeds this, stiffness never
/// exceeds a natural frequency of half of it. Keeps the suit stable at a 64 Hz physics step.
pub const MAX_RATE_GAIN: f32 = 30.0;

/// How the flyer is holding the suit this instant.
#[derive(Debug, Clone, Copy, Default)]
pub struct WingPose {
    /// -1 = straightened / arms swept back (dive), +1 = arched, wings pulled up (flare).
    pub arch: f32,
    /// -1 = twist to roll left, +1 = twist to roll right (asymmetric wing).
    pub twist: f32,
    /// 0..1 how much of the suit is inflated (it collapses at low airspeed).
    pub open: f32,
}

impl WingPose {
    /// Trim angle of attack this pose flies at.
    pub fn trim(&self) -> f32 {
        let a = self.arch.clamp(-1.0, 1.0);
        if a >= 0.0 {
            TRIM_NEUTRAL + a * (TRIM_RANGE.1 - TRIM_NEUTRAL)
        } else {
            TRIM_NEUTRAL + a * (TRIM_NEUTRAL - TRIM_RANGE.0)
        }
    }
}

/// Aerodynamic state of the suit, mostly for HUD/debug and the pilot logic.
#[derive(Debug, Clone, Copy, Default)]
pub struct WingAero {
    /// World-space aerodynamic force, N.
    pub force: Vec3,
    /// World-space aerodynamic torque about the centre of mass, N·m.
    pub torque: Vec3,
    /// Angle of attack, radians (wind hitting the belly is positive).
    pub alpha: f32,
    /// Sideslip, radians (velocity to the right of the head is positive).
    pub beta: f32,
}

/// Lift coefficient of the suit at angle of attack `a` (flat-plate-like, peaks near 45°).
pub fn wing_cl(a: f32) -> f32 {
    1.2 * (2.0 * a).sin()
}

/// Drag coefficient at angle of attack `a` (profile + induced + broadside plate).
pub fn wing_cd(a: f32) -> f32 {
    0.10 + 1.3 * a.sin().powi(2)
}

/// Wingsuit forces and moments.
///
/// `rot` body orientation, `v` velocity relative to the air (world), `rho` air density,
/// `omega` angular velocity (world), `inertia` principal moments (body X, Y, Z).
///
/// Moments are aerodynamic: pitch stiffness about the trim AoA the pose asks for,
/// weathervane yaw stability, roll from wing twist, plus aerodynamic rate damping. Each scales
/// with dynamic pressure, so at low airspeed the suit simply has no authority.
pub fn wingsuit(rot: Quat, v: Vec3, rho: f32, omega: Vec3, inertia: Vec3, pose: WingPose) -> WingAero {
    let speed = v.length();
    if speed < 0.5 || pose.open <= 0.0 {
        return WingAero::default();
    }
    let inv = rot.inverse();
    let vb = inv * v;
    let vh = vb / speed;
    let wb = inv * omega;
    let q = 0.5 * rho * speed * speed;
    let qs = q * WING_AREA * pose.open;

    // Angle of attack in the head/back plane, sideslip in the head/right plane.
    let alpha = (-vb.z).atan2(vb.y);
    let beta = vb.x.atan2(vb.y);

    // Lift is perpendicular to the wind, on the back side of the suit; drag opposes the wind.
    let lift_dir = Vec3::X.cross(vh).normalize_or(Vec3::Z);
    let side = -(Vec3::X - vh * vh.x).normalize_or_zero() * (0.6 * beta.sin());
    let fb = lift_dir * (qs * wing_cl(alpha)) - vh * (qs * wing_cd(alpha)) + side * qs;

    // Pitch: restoring moment toward the trim AoA (sin keeps the stable/unstable points right
    // when tumbling), roll: twist, yaw: weathervane into the wind.
    let plane_yz = (vh.y * vh.y + vh.z * vh.z).sqrt();
    let plane_xy = (vh.x * vh.x + vh.y * vh.y).sqrt();
    // Real suits get stiffer with q²; the body can't follow that fast (and a 64 Hz physics step
    // can't either), so the effective gains saturate: natural frequency ≤ MAX_RATE_GAIN/2.
    let k_pitch = (qs * WING_CHORD * 0.55 * plane_yz).min(inertia.x * MAX_RATE_GAIN * MAX_RATE_GAIN * 0.25);
    let k_yaw = (qs * WING_CHORD * 0.30 * plane_xy).min(inertia.z * MAX_RATE_GAIN * MAX_RATE_GAIN * 0.25);
    let d_roll = (qs * WING_SPAN * 0.10 / 1.6).min(inertia.y * MAX_RATE_GAIN);
    let m_roll = d_roll * WING_ROLL_RATE * pose.twist.clamp(-1.0, 1.0);
    let mut mb = Vec3::new(
        k_pitch * (pose.trim() - alpha).sin(),
        m_roll,
        -k_yaw * beta.sin(),
    );
    // Aerodynamic rate damping. Pitch and yaw: near-critical for the stiffness above.
    // Roll: a steady full twist settles to WING_ROLL_RATE.
    let d_pitch = 1.6 * (k_pitch * inertia.x).max(0.0).sqrt();
    let d_yaw = 1.6 * (k_yaw * inertia.z).max(0.0).sqrt();
    mb -= Vec3::new(wb.x * d_pitch, wb.y * d_roll, wb.z * d_yaw);

    WingAero { force: rot * fb, torque: rot * mb, alpha, beta }
}

/// Thrust axis that makes the net acceleration (thrust + gravity) point along `aim`.
///
/// `thrust` and `g` are accelerations (m/s²). If the thrust can't hold the line
/// (thrust ≤ g against a climb), it falls back to the best it can do.
pub fn rocket_axis(aim: Vec3, up: Vec3, thrust: f32, g: f32) -> Vec3 {
    let d = aim.normalize_or(up);
    let du = d.dot(up);
    // |a d + g up| = thrust  ->  a = -g du + sqrt(g² du² - g² + thrust²)
    let disc = g * g * du * du - g * g + thrust * thrust;
    if disc <= 0.0 {
        return (d + up).normalize_or(up);
    }
    let a = (-g * du + disc.sqrt()).max(0.0);
    (d * a + up * g).normalize_or(up)
}

/// How the boots realise a wanted torque.
#[derive(Debug, Clone, Copy, Default)]
pub struct BootSteer {
    /// Where the legs should swing the feet, body-local offset from their rest pose (y = 0).
    pub leg_offset: Vec3,
    /// Extra sideways push per foot (body-local, ⊥ to the thrust axis) for roll about the axis:
    /// left foot gets `-roll_push`, right foot `+roll_push`. Newtons.
    pub roll_push: Vec3,
}

/// Leg swing limits, body-local (x = sideways, z = fore/aft), metres.
pub const LEG_REACH: Vec3 = Vec3::new(0.15, 0.0, 0.45);
/// Half the distance between the two soles, metres.
pub const FOOT_HALF_SPAN: f32 = 0.17;

/// Solve the leg swing that makes boot thrust `thrust_n` (N, along body +Y) produce `torque`
/// (world, N·m) about the centre of mass. The thrust line moves with the feet, so a foot offset `r`
/// gives `r × F`; roll about the thrust axis comes from the soles pushing slightly sideways in
/// opposite directions (a couple). Whatever the legs can't reach is simply not produced.
pub fn boot_steer(rot: Quat, thrust_n: f32, torque: Vec3) -> BootSteer {
    if thrust_n < 1.0 {
        return BootSteer::default();
    }
    let tb = rot.inverse() * torque;
    // Torque about X / Z (tilting the thrust axis): offset r along Z / X.  r × (F ŷ):
    //   (rx, 0, rz) × (0, F, 0) = (-rz F, 0, rx F)
    let leg_offset = Vec3::new(tb.z / thrust_n, 0.0, -tb.x / thrust_n).clamp(-LEG_REACH, LEG_REACH);
    // Roll about Y: right foot at +s pushes f, left at -s pushes -f: τ = 2 s × f.
    // s = (h, 0, 0), f = (0, 0, fz):  2 s × f = (0, -2 h fz, 0)
    let max_side = thrust_n * 0.5 * 0.25;
    let fz = (-tb.y / (2.0 * FOOT_HALF_SPAN)).clamp(-max_side, max_side);
    BootSteer { leg_offset, roll_push: Vec3::new(0.0, 0.0, fz) }
}

/// Smooth 0..1 boost ramp: slow start, long build. `t` seconds of afterburner held.
pub fn boost_ramp(t: f32, full_s: f32) -> f32 {
    let x = (t / full_s).clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// What the player is asking the wingsuit for this instant.
#[derive(Debug, Clone, Copy, Default)]
pub struct WingCmd {
    /// +1 = S (arch, pull the wings up: nose up), -1 = W (straighten out: dive).
    pub pitch: f32,
    /// +1 = D (twist to bank right), -1 = A.
    pub roll: f32,
    /// Horizontal direction the view points (the glide heads there by banking), or zero.
    pub heading: Vec3,
    /// Extra arch from looking up/down, already scaled to roughly -1..1.
    pub arch_bias: f32,
}

/// Signed bank angle, + = right wing down (radians).
pub fn bank_angle(rot: Quat, up: Vec3) -> f32 {
    (-(rot * Vec3::X).dot(up)).clamp(-1.0, 1.0).asin()
}

/// Bank measured about the flight path (where the lift can actually point), + = right wing
/// down. World-up `bank_angle` saturates in a dive: at 60 deg down, rolling only moves it
/// +-30 deg, so a turn asking for more spun the body forever. Reference "up" is world up
/// projected off the path; as the dive goes vertical it blends to the view direction, so a
/// zero bank there means "back (lift) toward the view" and the dive curves onto it.
fn path_bank(rot: Quat, v: Vec3, up: Vec3, heading: Vec3, steep: f32) -> f32 {
    let n = v.normalize_or(rot * Vec3::Y);
    let off = |d: Vec3| d - n * d.dot(n);
    let view = off(heading).normalize_or_zero();
    let reference = (off(up) + view * steep * 0.8).normalize_or(off(rot * Vec3::Z).normalize_or(up));
    let back = off(rot * Vec3::Z).normalize_or(reference);
    back.dot(n.cross(reference)).atan2(back.dot(reference))
}

/// The flyer's "hands": turn the command into a suit pose. Nothing here touches the body
/// directly; it only reshapes the suit (arch, twist), and the air does the rest.
/// Heading changes come from banking (the tilted lift curves the path), like a glider.
pub fn wing_pilot(rot: Quat, v: Vec3, up: Vec3, omega: Vec3, cmd: WingCmd) -> WingPose {
    let roll_rate = omega.dot(rot * Vec3::Y);
    let v_flat = v - up * v.dot(up);
    // Flight-path angle (- = descending). Near vertical the flat heading is noise.
    let path = (v.normalize_or(-up).dot(up)).clamp(-1.0, 1.0).asin();
    let steep = ((-path - 1.0) / 0.35).clamp(0.0, 1.0);
    let bank = path_bank(rot, v, up, cmd.heading, steep);
    let heading_err = (1.0 - steep) * if v_flat.length() > 4.0 && cmd.heading.length_squared() > 0.01 {
        let vd = v_flat.normalize();
        let hd = (cmd.heading - up * cmd.heading.dot(up)).normalize_or(vd);
        // + = the view is to the right of the path.
        (-up.dot(vd.cross(hd))).atan2(vd.dot(hd))
    } else {
        0.0
    };
    let roll_in = cmd.roll.clamp(-1.0, 1.0);
    let twist = if roll_in.abs() > 0.05 {
        // A/D work like ailerons: the twist IS the key, so holding it keeps rolling (a barrel
        // roll in under 2 s), tapping it banks.
        roll_in
    } else {
        // Otherwise bank toward the view and hold it there.
        let bank_target = (heading_err * 3.5).clamp(-1.25, 1.25);
        ((bank_target - bank) * 3.0 - roll_rate * 0.5).clamp(-1.0, 1.0)
    };
    // A banked wing needs more lift to hold the glide: pull (arch) into the turn, only while
    // right side up (upside down mid-roll, pulling would just dive).
    let upright = (rot * Vec3::Z).dot(up).max(0.0);
    // While there is still heading to make up, pull harder on top: that tightens the turn,
    // and letting go of it as the view comes round is what stops the spiral.
    let banked = (bank.abs() / 0.6).min(1.0);
    let turn_arch = ((1.0 / bank.cos().max(0.35) - 1.0) * 0.8 + (heading_err.abs() * 2.0).min(PULL) * banked) * upright;
    // Unless W is held, a near-vertical dive (the exit, or after a roll) is flown out first:
    // arch to pull back into the glide, then turn.
    let recover = steep * (1.0 + cmd.pitch.min(0.0) * 2.0).max(0.0) * upright.max(0.3);
    let mut arch = (cmd.pitch + cmd.arch_bias + turn_arch + recover * 0.6).clamp(-1.0, 1.0);
    // Pushing over (negative trim) steepens the dive; once near vertical, ease off to zero lift
    // so W holds a straight dive instead of bunting onto the back.
    let zero_lift_arch = -TRIM_NEUTRAL / (TRIM_NEUTRAL - TRIM_RANGE.0);
    // Turning needs lift on the BACK side: pushing (negative trim) flips the lift to the belly,
    // so banking toward the view would steer away from it and the body slowly corkscrews.
    // While there is heading to make up (or, straight down, a view off the path), hold a
    // little positive lift instead.
    let n = v.normalize_or(-up);
    let view_off = if cmd.heading.length_squared() > 0.01 {
        (cmd.heading - n * cmd.heading.dot(n)).length().min(1.0)
    } else {
        0.0
    };
    let want_turn = ((heading_err.abs() / 0.4).min(1.0)).max(steep * view_off);
    let floor = -1.0 + (1.0 + zero_lift_arch) * steep.max(want_turn) + 0.4 * want_turn;
    arch = arch.max(floor.min(1.0));
    WingPose { arch, twist, open: 1.0 }
}

/// Freefall without the suit: what the keys ask the body to do.
#[derive(Clone, Copy, Debug, Default)]
pub struct FallCmd {
    /// +1 = W: somersault forward (head down over the belly).
    pub flip: f32,
    /// +1 = D: barrel roll right, about the head-to-toe axis.
    pub roll: f32,
    /// 0..1 = S: spread eagle (stable, belly to the wind, most drag).
    pub spread: f32,
}

/// Spin rates a skydiver reaches by throwing limbs into the wind, rad/s.
pub const FLIP_RATE: f32 = 4.5;
pub const BARREL_RATE: f32 = 5.5;

/// Torque (world) a skydiver without a suit makes by moving arms and legs in the airflow.
/// W/A/D drive a spin about body X / Y (a tuck or a dropped shoulder: the other axes are
/// damped so the spin stays clean); with no spin asked the body arches back to `belly_down`
/// (the box position), harder when spread eagle. Authority grows with airspeed: at the exit,
/// barely moving, the limbs have little air to push on.
pub fn freefall_torque(rot: Quat, omega: Vec3, airspeed: f32, belly_down: Quat, cmd: FallCmd) -> Vec3 {
    let authority = (airspeed / 25.0).clamp(0.2, 1.0);
    let w_body = rot.inverse() * omega;
    let flip = cmd.flip.clamp(-1.0, 1.0);
    let roll = cmd.roll.clamp(-1.0, 1.0);
    if flip.abs() > 0.05 || roll.abs() > 0.05 {
        // Body +X = right: a forward flip rotates the head (+Y) toward the belly (-Z), i.e. -X.
        // Body +Y = head: rolling right sends the right side (+X) toward the belly, i.e. +Y.
        let want = Vec3::new(-flip * FLIP_RATE, roll * BARREL_RATE, 0.0);
        let t_body = (want - w_body) * BODY_INERTIA * 5.0;
        rot * t_body * authority
    } else {
        let w = 3.5 + 2.0 * cmd.spread.clamp(0.0, 1.0);
        let err = rot_error(rot, belly_down);
        let i = BODY_INERTIA.x;
        (err * i * w * w - omega * 2.0 * i * w * 0.8) * authority
    }
}

/// Inertia the player body uses (principal, body X/Y/Z), kg·m².
pub const BODY_INERTIA: Vec3 = Vec3::new(8.5, 3.8, 8.5);

/// Target posture for rocket flight: body +Y along the thrust `axis`, back toward `back_ref`
/// (the camera's up, so the flyer is seen from behind), rolled by `roll` (+ = right wing down).
pub fn rocket_attitude(axis: Vec3, back_ref: Vec3, fallback_back: Vec3, roll: f32) -> Quat {
    let y = axis.normalize_or(Vec3::Y);
    let mut z = back_ref - y * back_ref.dot(y);
    if z.length_squared() < 1e-4 {
        z = fallback_back - y * fallback_back.dot(y);
    }
    let z = z.normalize_or(y.any_orthonormal_vector());
    let x = y.cross(z);
    Quat::from_mat3(&glam::Mat3::from_cols(x, y, z)) * Quat::from_rotation_y(roll)
}

/// Shortest-arc rotation error (world axis × angle) from `current` to `target`.
pub fn rot_error(current: Quat, target: Quat) -> Vec3 {
    let mut dq = target * current.inverse();
    if dq.w < 0.0 {
        dq = -dq;
    }
    let (axis, angle) = dq.to_axis_angle();
    if angle.is_finite() { axis * angle } else { Vec3::ZERO }
}

/// The torque the flyer *wants* from the boots: a loose, slightly underdamped spring
/// (≈ 0.7 Hz, ζ 0.65) so the body swings onto the aim with some overshoot instead of snapping.
pub fn rocket_want_torque(rot: Quat, target: Quat, omega: Vec3) -> Vec3 {
    let (w, zeta) = (4.5f32, 0.65f32);
    let i = BODY_INERTIA.x;
    rot_error(rot, target) * (i * w * w) - omega * (2.0 * zeta * i * w)
}

/// Where and how hard each sole pushes, given the (already filtered) leg swing.
/// Returns `[(point, force); 2]` for left, right, in world space, pushing along the body axis.
pub fn boot_forces(rot: Quat, com: Vec3, thrust_n: f32, leg_offset: Vec3, roll_push: Vec3) -> [(Vec3, Vec3); 2] {
    let axis = rot * Vec3::Y;
    let mut out = [(com, Vec3::ZERO); 2];
    for (i, side) in [-1.0f32, 1.0].into_iter().enumerate() {
        let local = Vec3::new(side * FOOT_HALF_SPAN, -0.9, 0.0) + leg_offset;
        out[i] = (com + rot * local, axis * (thrust_n * 0.5) + rot * (roll_push * side));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal rigid body for closed-loop tests.
    struct Body {
        pos: Vec3,
        vel: Vec3,
        rot: Quat,
        omega: Vec3,
    }

    impl Body {
        fn step(&mut self, force: Vec3, torque: Vec3, dt: f32) {
            let m = 80.0;
            self.vel += (force / m + Vec3::NEG_Y * 9.81) * dt;
            self.pos += self.vel * dt;
            let inv = self.rot.inverse();
            let wb = inv * self.omega;
            let tb = inv * torque;
            let iw = I * wb;
            let wdot = (tb - wb.cross(iw)) / I;
            let wb = wb + wdot * dt;
            self.rot = (self.rot * Quat::from_scaled_axis(wb * dt)).normalize();
            self.omega = self.rot * wb;
        }
    }

    fn fly_wingsuit(cmd: WingCmd, secs: f32) -> Body {
        // Step off belly-down, nearly still, head north.
        let mut b = Body { pos: Vec3::ZERO, vel: Vec3::NEG_Y * 2.0, rot: level_rot(), omega: Vec3::ZERO };
        let dt: f32 = 1.0 / 64.0;
        let mut pose = WingPose::default();
        for _ in 0..(secs / dt) as usize {
            let want = wing_pilot(b.rot, b.vel, Vec3::Y, b.omega, cmd);
            // Limbs move at a finite rate.
            let k = 1.0 - (-10.0 * dt).exp();
            pose.arch += (want.arch - pose.arch) * k;
            pose.twist += (want.twist - pose.twist) * k;
            pose.open = ((b.vel.length() - 4.0) / 10.0).clamp(0.0, 1.0);
            let a = wingsuit(b.rot, b.vel, 1.1, b.omega, I, pose);
            b.step(a.force, a.torque, dt);
            assert!(b.vel.is_finite() && b.omega.length() < 20.0, "blew up: {:?}", b.omega);
        }
        b
    }

    #[test]
    fn wingsuit_settles_into_a_glide() {
        let b = fly_wingsuit(WingCmd { heading: Vec3::NEG_Z, ..Default::default() }, 30.0);
        let h = Vec3::new(b.vel.x, 0.0, b.vel.z).length();
        let sink = -b.vel.y;
        assert!(h > 15.0 && sink > 3.0, "vel {:?}", b.vel);
        assert!(h / sink > 1.5, "glide {}", h / sink);
        assert!(b.omega.length() < 0.3, "still rotating {:?}", b.omega);
        // Head along the path, back to the sky.
        assert!((b.rot * Vec3::Y).dot(b.vel.normalize()) > 0.85);
        assert!((b.rot * Vec3::Z).y > 0.6);
    }

    #[test]
    fn wingsuit_flare_glides_slower_and_dive_faster() {
        let base = fly_wingsuit(WingCmd { heading: Vec3::NEG_Z, ..Default::default() }, 25.0).vel.length();
        let flare = fly_wingsuit(WingCmd { heading: Vec3::NEG_Z, pitch: 1.0, ..Default::default() }, 25.0).vel.length();
        let dive = fly_wingsuit(WingCmd { heading: Vec3::NEG_Z, pitch: -1.0, ..Default::default() }, 25.0).vel.length();
        assert!(flare < base && base < dive, "flare {flare} base {base} dive {dive}");
    }

    #[test]
    fn wingsuit_turns_toward_the_view_by_banking() {
        // Look east while gliding north.
        let b = fly_wingsuit(WingCmd { heading: Vec3::X, ..Default::default() }, 30.0);
        let vd = Vec3::new(b.vel.x, 0.0, b.vel.z).normalize();
        assert!(vd.dot(Vec3::X) > 0.9, "heading {vd:?}");
    }

    /// Seconds a settled glide takes to swing 90 degrees onto a view to its left
    /// (and, with `settle`, keeps flying and returns the final heading error instead).
    fn time_to_turn_left() -> f32 {
        turn_left(false)
    }

    fn turn_left(settle: bool) -> f32 {
        let mut b = fly_wingsuit(WingCmd { heading: Vec3::NEG_Z, ..Default::default() }, 15.0);
        let dt: f32 = 1.0 / 64.0;
        let mut pose = WingPose { open: 1.0, ..Default::default() };
        let cmd = WingCmd { heading: Vec3::NEG_X, ..Default::default() };
        for i in 0..(20.0 / dt) as usize {
            let want = wing_pilot(b.rot, b.vel, Vec3::Y, b.omega, cmd);
            let k = 1.0 - (-10.0 * dt).exp();
            pose.arch += (want.arch - pose.arch) * k;
            pose.twist += (want.twist - pose.twist) * k;
            let a = wingsuit(b.rot, b.vel, 1.1, b.omega, I, pose);
            b.step(a.force, a.torque, dt);
            assert!(b.vel.is_finite() && b.omega.length() < 20.0, "blew up: {:?}", b.omega);
            let d = Vec3::new(b.vel.x, 0.0, b.vel.z).normalize().dot(Vec3::NEG_X);
            if !settle && d > 0.9 {
                return i as f32 * dt;
            }
            if settle && i as f32 * dt > 12.0 {
                return d.clamp(-1.0, 1.0).acos();
            }
        }
        f32::INFINITY
    }

    #[test]
    fn wingsuit_settles_on_the_view_without_swinging_past() {
        let err = turn_left(true);
        assert!(err < 0.1, "heading error after 12 s: {err} rad");
    }

    #[test]
    fn wingsuit_turns_onto_the_view_briskly() {
        let t = time_to_turn_left();
        eprintln!("90 deg turn in {t:.2} s");
        assert!(t < 3.0, "90 deg turn took {t} s");
    }

    /// Belly-down at terminal speed, then hold `cmd` for `secs`.
    fn fall(cmd: FallCmd, secs: f32) -> Body {
        let belly = level_rot();
        let mut b = Body { pos: Vec3::ZERO, vel: Vec3::NEG_Y * 50.0, rot: belly, omega: Vec3::ZERO };
        let dt: f32 = 1.0 / 64.0;
        for _ in 0..(secs / dt) as usize {
            let t = freefall_torque(b.rot, b.omega, 50.0, belly, cmd);
            b.step(Vec3::ZERO, t, dt);
            assert!(b.omega.is_finite() && b.omega.length() < 12.0, "spun out: {:?}", b.omega);
        }
        b
    }

    #[test]
    fn freefall_w_somersaults_forward() {
        let b = fall(FallCmd { flip: 1.0, ..Default::default() }, 0.25);
        // The head (+Y, pointing north) has started dropping toward the ground.
        assert!((b.rot * Vec3::Y).y < -0.1, "head {:?}", b.rot * Vec3::Y);
        let b = fall(FallCmd { flip: 1.0, ..Default::default() }, 3.0);
        let w = b.rot.inverse() * b.omega;
        assert!((w.x + FLIP_RATE).abs() < 0.5 && w.y.abs() < 0.3, "body rates {w:?}");
    }

    #[test]
    fn freefall_d_barrel_rolls_right() {
        let b = fall(FallCmd { roll: 1.0, ..Default::default() }, 0.25);
        // The right side (+X) dips toward the ground.
        assert!((b.rot * Vec3::X).y < -0.1, "right {:?}", b.rot * Vec3::X);
        let w = fall(FallCmd { roll: 1.0, ..Default::default() }, 3.0);
        let wb = w.rot.inverse() * w.omega;
        assert!((wb.y - BARREL_RATE).abs() < 0.5 && wb.x.abs() < 0.3, "body rates {wb:?}");
    }

    #[test]
    fn freefall_lets_go_and_settles_belly_down() {
        // Tumble, then release: the box position catches it.
        let belly = level_rot();
        let mut b = fall(FallCmd { flip: 1.0, roll: 0.6, spread: 0.0 }, 2.0);
        let dt: f32 = 1.0 / 64.0;
        for _ in 0..(3.0 / dt) as usize {
            let t = freefall_torque(b.rot, b.omega, 50.0, belly, FallCmd { spread: 1.0, ..Default::default() });
            b.step(Vec3::ZERO, t, dt);
        }
        assert!(rot_error(b.rot, belly).length() < 0.1 && b.omega.length() < 0.3, "err {:?}", rot_error(b.rot, belly));
    }

    #[test]
    fn wingsuit_w_pushes_over_into_a_steep_dive() {
        let mut b = fly_wingsuit(WingCmd { heading: Vec3::NEG_Z, ..Default::default() }, 15.0);
        let dt: f32 = 1.0 / 64.0;
        let mut pose = WingPose { open: 1.0, ..Default::default() };
        let cmd = WingCmd { pitch: -1.0, heading: Vec3::NEG_Z, ..Default::default() };
        let mut t_steep = f32::INFINITY;
        for i in 0..(8.0 / dt) as usize {
            let want = wing_pilot(b.rot, b.vel, Vec3::Y, b.omega, cmd);
            let k = 1.0 - (-10.0 * dt).exp();
            pose.arch += (want.arch - pose.arch) * k;
            pose.twist += (want.twist - pose.twist) * k;
            let a = wingsuit(b.rot, b.vel, 1.1, b.omega, I, pose);
            b.step(a.force, a.torque, dt);
            let path = b.vel.normalize().y.asin();
            if path < -0.9 && t_steep.is_infinite() {
                t_steep = i as f32 * dt;
            }
            // Never tucks under onto the back.
            assert!((b.rot * Vec3::Z).y > -0.3, "went inverted at {i}");
        }
        eprintln!("W: 50 deg dive after {t_steep:.2} s");
        assert!(t_steep < 2.0, "dive took {t_steep} s");
    }

    #[test]
    fn wingsuit_fast_dive_looking_sideways_does_not_corkscrew() {
        let mut b = fly_wingsuit(WingCmd { heading: Vec3::NEG_Z, ..Default::default() }, 15.0);
        let dt: f32 = 1.0 / 64.0;
        let mut pose = WingPose { open: 1.0, ..Default::default() };
        let mut rolled = 0.0f32;
        let mut worst = 0.0f32;
        for i in 0..(10.0 / dt) as usize {
            // Hold W the whole time; look east (90 deg off the path) after 2 s of diving.
            let heading = if i < 128 { Vec3::NEG_Z } else { Vec3::X };
            let want = wing_pilot(b.rot, b.vel, Vec3::Y, b.omega, WingCmd { pitch: -1.0, heading, ..Default::default() });
            let k = 1.0 - (-10.0 * dt).exp();
            pose.arch += (want.arch - pose.arch) * k;
            pose.twist += (want.twist - pose.twist) * k;
            let a = wingsuit(b.rot, b.vel, 1.1, b.omega, I, pose);
            b.step(a.force, a.torque, dt);
            if i >= 128 {
                rolled += b.omega.dot(b.rot * Vec3::Y) * dt;
                worst = worst.max(rolled.abs());
            }
        }
        eprintln!("dive+look: rolled {worst:.2} rad, final vel {:.1?}", b.vel);
        assert!(worst < 2.5, "corkscrewed {worst} rad");
    }

    #[test]
    fn wingsuit_holding_d_barrel_rolls() {
        let mut b = fly_wingsuit(WingCmd { heading: Vec3::NEG_Z, ..Default::default() }, 15.0);
        let dt: f32 = 1.0 / 64.0;
        let mut pose = WingPose { open: 1.0, ..Default::default() };
        let mut turned = 0.0f32;
        for _ in 0..(2.0 / dt) as usize {
            let want = wing_pilot(b.rot, b.vel, Vec3::Y, b.omega, WingCmd { roll: 1.0, ..Default::default() });
            let k = 1.0 - (-10.0 * dt).exp();
            pose.twist += (want.twist - pose.twist) * k;
            pose.arch += (want.arch - pose.arch) * k;
            let a = wingsuit(b.rot, b.vel, 1.1, b.omega, I, pose);
            b.step(a.force, a.torque, dt);
            turned += b.omega.dot(b.rot * Vec3::Y) * dt;
        }
        assert!(turned > std::f32::consts::TAU * 0.9, "rolled {turned} rad in 2 s");
    }

    #[test]
    fn wingsuit_roll_key_banks() {
        let mut b = fly_wingsuit(WingCmd { heading: Vec3::NEG_Z, ..Default::default() }, 20.0);
        let dt: f32 = 1.0 / 64.0;
        let mut pose = WingPose { open: 1.0, ..Default::default() };
        // A tap: within a third of a second the wing is visibly banked.
        for _ in 0..21 {
            let want = wing_pilot(b.rot, b.vel, Vec3::Y, b.omega, WingCmd { roll: 1.0, ..Default::default() });
            pose.twist += (want.twist - pose.twist) * 0.15;
            pose.arch = want.arch;
            let a = wingsuit(b.rot, b.vel, 1.1, b.omega, I, pose);
            b.step(a.force, a.torque, dt);
        }
        assert!(bank_angle(b.rot, Vec3::Y) > 0.4, "bank {}", bank_angle(b.rot, Vec3::Y));
    }

    #[test]
    fn rocket_swings_onto_the_aim_using_only_the_feet() {
        let g = 9.81;
        let mut b = Body { pos: Vec3::ZERO, vel: Vec3::ZERO, rot: Quat::IDENTITY, omega: Vec3::ZERO };
        let aim = Vec3::new(0.0, 0.2, -1.0).normalize();
        let thrust_acc = 2.4 * g;
        let dt: f32 = 1.0 / 64.0;
        let mut leg = Vec3::ZERO;
        let mut max_rate: f32 = 0.0;
        for _ in 0..(6.0 / dt) as usize {
            let axis = rocket_axis(aim, Vec3::Y, thrust_acc, g);
            let target = rocket_attitude(axis, Vec3::Y, Vec3::Z, 0.0);
            let gate = (((b.rot * Vec3::Y).dot(axis) - 0.3) / 0.5).clamp(0.25, 1.0);
            let f = 80.0 * thrust_acc * gate;
            let s = boot_steer(b.rot, f, rocket_want_torque(b.rot, target, b.omega));
            leg += (s.leg_offset - leg) * (1.0 - (-15.0 * dt).exp());
            let (mut force, mut torque) = (Vec3::ZERO, Vec3::ZERO);
            for (pt, fo) in boot_forces(b.rot, b.pos, f, leg, s.roll_push) {
                force += fo;
                torque += (pt - b.pos).cross(fo);
            }
            b.step(force, torque, dt);
            max_rate = max_rate.max(b.omega.length());
        }
        let axis = rocket_axis(aim, Vec3::Y, thrust_acc, g);
        assert!((b.rot * Vec3::Y).dot(axis) > 0.99, "axis {:?}", b.rot * Vec3::Y);
        assert!(b.omega.length() < 0.2);
        assert!(max_rate < 6.0, "swung too hard {max_rate}");
        // Net motion follows the aim.
        assert!(b.vel.normalize().dot(aim) > 0.95, "vel {:?}", b.vel);
    }

    const I: Vec3 = Vec3::new(8.5, 3.8, 8.5);

    /// Belly-down, head along -Z (north-ish), back to the sky.
    fn level_rot() -> Quat {
        Quat::from_mat3(&glam::Mat3::from_cols(Vec3::X, Vec3::NEG_Z, Vec3::Y))
    }

    #[test]
    fn glide_at_trim_lifts_and_has_sane_ratio() {
        let rot = level_rot();
        let a = TRIM_NEUTRAL;
        // Velocity along the head, tilted so the wind hits the belly at trim AoA.
        let v = (Vec3::NEG_Z * a.cos() + Vec3::NEG_Y * a.sin()) * 35.0;
        let aero = wingsuit(rot, v, 1.225, Vec3::ZERO, I, WingPose { open: 1.0, ..Default::default() });
        assert!((aero.alpha - a).abs() < 1e-3, "alpha {}", aero.alpha);
        let lift = aero.force.dot((Vec3::X.cross(v)).normalize());
        let drag = -aero.force.dot(v.normalize());
        assert!(lift > 400.0 && lift < 1100.0, "lift {lift}");
        let ld = lift / drag;
        assert!(ld > 2.0 && ld < 4.0, "L/D {ld}");
        // At trim the pitch moment vanishes.
        assert!(aero.torque.length() < 1.0, "torque {:?}", aero.torque);
    }

    #[test]
    fn flare_pitches_nose_up_and_dive_nose_down() {
        let rot = level_rot();
        let a = TRIM_NEUTRAL;
        let v = (Vec3::NEG_Z * a.cos() + Vec3::NEG_Y * a.sin()) * 35.0;
        let right = rot * Vec3::X;
        let flare = wingsuit(rot, v, 1.225, Vec3::ZERO, I, WingPose { arch: 1.0, open: 1.0, twist: 0.0 });
        let dive = wingsuit(rot, v, 1.225, Vec3::ZERO, I, WingPose { arch: -1.0, open: 1.0, twist: 0.0 });
        // Nose up = rotation about +right (head toward the back / sky).
        assert!(flare.torque.dot(right) > 50.0);
        assert!(dive.torque.dot(right) < -50.0);
    }

    #[test]
    fn twist_rolls_toward_its_side() {
        let rot = level_rot();
        let v = Vec3::NEG_Z * 35.0 + Vec3::NEG_Y * 7.0;
        let head = rot * Vec3::Y;
        let r = wingsuit(rot, v, 1.225, Vec3::ZERO, I, WingPose { twist: 1.0, open: 1.0, arch: 0.0 });
        // +Y rotation drops the right wing (X -> -Z = toward the ground in this pose).
        assert!(r.torque.dot(head) > 20.0);
        let dropped = Quat::from_axis_angle(head, 0.3) * (rot * Vec3::X);
        assert!(dropped.y < 0.0);
    }

    #[test]
    fn weathervane_turns_nose_into_sideslip() {
        let rot = level_rot();
        // Moving forward and to the right.
        let v = Vec3::NEG_Z * 35.0 + Vec3::X * 8.0 + Vec3::NEG_Y * 6.0;
        let r = wingsuit(rot, v, 1.225, Vec3::ZERO, I, WingPose { open: 1.0, ..Default::default() });
        assert!(r.beta > 0.1);
        let yaw = r.torque.dot(rot * Vec3::Z);
        // Yawing right is rotation about -back.
        assert!(yaw < 0.0, "yaw {yaw}");
    }

    #[test]
    fn rates_are_damped() {
        let rot = level_rot();
        let v = Vec3::NEG_Z * 35.0 + Vec3::NEG_Y * 7.0;
        let spin = (rot * Vec3::Y) * 2.0;
        let r = wingsuit(rot, v, 1.225, spin, I, WingPose { open: 1.0, ..Default::default() });
        assert!(r.torque.dot(rot * Vec3::Y) < 0.0);
    }

    #[test]
    fn no_air_no_forces() {
        let r = wingsuit(level_rot(), Vec3::NEG_Z * 50.0, 0.0, Vec3::ZERO, I, WingPose { open: 1.0, ..Default::default() });
        assert!(r.force.length() < 1e-6 && r.torque.length() < 1e-6);
    }

    #[test]
    fn rocket_axis_cancels_gravity() {
        let up = Vec3::Y;
        let g = 9.81;
        for aim in [Vec3::NEG_Z, Vec3::new(0.0, 0.5, -1.0), Vec3::new(1.0, -0.4, 0.0), Vec3::Y] {
            let t = 2.4 * g;
            let axis = rocket_axis(aim, up, t, g);
            let net = axis * t - up * g;
            let along = net.normalize().dot(aim.normalize());
            assert!(along > 0.999, "aim {aim:?} net {net:?}");
        }
    }

    #[test]
    fn boot_steer_produces_wanted_torque() {
        let rot = Quat::from_euler(glam::EulerRot::YXZ, 0.6, -1.1, 0.2);
        let f = 2000.0;
        let want = rot * Vec3::new(120.0, 30.0, -80.0);
        let s = boot_steer(rot, f, want);
        let axis = rot * Vec3::Y;
        let right = rot * Vec3::X;
        let foot = [-right * FOOT_HALF_SPAN, right * FOOT_HALF_SPAN];
        let push = [-(rot * s.roll_push), rot * s.roll_push];
        let mut tau = Vec3::ZERO;
        for i in 0..2 {
            // Nominal feet ~0.9 m down the axis; the axial lever adds nothing with an axial force.
            let r = foot[i] + rot * s.leg_offset - axis * 0.9;
            tau += r.cross(axis * (f * 0.5) + push[i]);
        }
        // The couple's push also acts at the -0.9 m lever, which tilts a little; tolerance covers it.
        let err = (tau - want).length();
        assert!(err < 40.0, "torque {tau:?} want {want:?}");
    }

    #[test]
    fn boot_steer_is_limited_by_leg_reach() {
        let s = boot_steer(Quat::IDENTITY, 500.0, Vec3::new(5000.0, 0.0, 5000.0));
        assert!(s.leg_offset.x.abs() <= LEG_REACH.x + 1e-6);
        assert!(s.leg_offset.z.abs() <= LEG_REACH.z + 1e-6);
    }

    #[test]
    fn ramp_is_slow_then_full() {
        assert!(boost_ramp(1.0, 10.0) < 0.05);
        assert!((boost_ramp(10.0, 10.0) - 1.0).abs() < 1e-6);
        assert!(boost_ramp(5.0, 10.0) > 0.45 && boost_ramp(5.0, 10.0) < 0.55);
    }
}
