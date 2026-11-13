//! The robot's third-person camera rig (a port of the alpha's `camera_follow`): a pivot above
//! the pelvis that follows rigidly on the flat but rides steps and bumps through a critically
//! damped spring, a pull-in against terrain, and a field of view that widens with speed.
//!
//! Mouse look and zoom stay in [`crate::play::OrbitCam`] (yaw, pitch, distance); this holds
//! only the follow state. Sim frame (ENU, z up) in, render-frame [`Camera`] out.

use ed_planet::Anchor;
use ed_render::Camera;
use glam::DVec3;

/// Pivot above the pelvis, along world up and along the body's up, m.
pub const PIVOT_UP: f64 = 0.55;
pub const PIVOT_BODY_UP: f64 = 0.20;
/// Vertical spring natural frequency (rad/s).
pub const SPRING_W: f64 = 11.0;
/// Pull-in margin and minimum distance, m.
pub const MARGIN: f64 = 0.3;
pub const MIN_DIST: f64 = 0.5;
/// Zoom limits and the alpha's default distance, m.
pub const DIST_MIN: f64 = 2.0;
pub const DIST_MAX: f64 = 60.0;
pub const DIST_DEFAULT: f64 = 6.0;
/// Camera clearance above the ground under it, m.
pub const GROUND_CLEAR: f64 = 0.3;

#[derive(Clone, Copy, Debug)]
pub struct RobotCam {
    /// Pivot minus anchor, vertical (m) and its rate (m/s): the lag the spring removes.
    off: f64,
    vel: f64,
    last_anchor: Option<DVec3>,
    /// Smoothed distance, eased in log space.
    dist: f64,
    /// Smoothed vertical field of view, radians.
    fov: f64,
    /// Pulled-in distance (pulls in at once, eases back out).
    cur: f64,
}

impl Default for RobotCam {
    fn default() -> Self {
        RobotCam { off: 0.0, vel: 0.0, last_anchor: None, dist: DIST_DEFAULT, fov: 62f64.to_radians(), cur: DIST_DEFAULT }
    }
}

impl RobotCam {
    /// Forgets the lag (after a teleport or getting out of the car).
    pub fn reset(&mut self) {
        self.off = 0.0;
        self.vel = 0.0;
        self.last_anchor = None;
    }

    /// `look` carries the yaw and pitch (renderer convention); `dist_goal` is the wanted
    /// distance. `blocked(from, d)` is the free fraction of the ray `from -> from + d`,
    /// `ground(p)` the terrain height (sim z) under `p`.
    #[allow(clippy::too_many_arguments)]
    pub fn update(&mut self, look: &Camera, dist_goal: f64, pelvis: DVec3, body_up: DVec3, speed: f64, grounded: bool, dt: f64, blocked: impl Fn(DVec3, DVec3) -> Option<f64>, ground: impl Fn(DVec3) -> Option<f64>) -> Camera {
        let anchor = pelvis + DVec3::Z * PIVOT_UP + body_up * PIVOT_BODY_UP;
        match self.last_anchor {
            Some(prev) if (anchor - prev).length() <= 25.0 + speed * dt * 2.0 => {
                if grounded {
                    // The pivot stays where it was; the anchor's vertical move becomes lag.
                    self.off -= anchor.z - prev.z;
                    let w = SPRING_W;
                    let e = (-w * dt).exp();
                    let (x0, v0) = (self.off, self.vel);
                    self.off = (x0 + (v0 + w * x0) * dt) * e;
                    self.vel = (v0 - w * (v0 + w * x0) * dt) * e;
                } else {
                    self.off = 0.0;
                    self.vel = 0.0;
                }
            }
            _ => {
                self.off = 0.0;
                self.vel = 0.0;
            }
        }
        self.last_anchor = Some(anchor);

        let k_d = 1.0 - (-5.0 * dt).exp();
        self.dist = (self.dist.ln() + (dist_goal.clamp(DIST_MIN, DIST_MAX).ln() - self.dist.ln()) * k_d).exp();
        let fov_goal = (62.0 + (speed / 24.0).min(26.0)).to_radians();
        self.fov += (fov_goal - self.fov) * (1.0 - (-4.0 * dt).exp());

        let pivot = DVec3::new(anchor.x, anchor.y, anchor.z + self.off);
        let fwd = Anchor::render_to_sim(look.forward().as_dvec3());
        let back = -fwd * self.dist;
        let free = blocked(pivot, back).map_or(self.dist, |f| (f * self.dist - MARGIN).max(MIN_DIST));
        self.cur = if free < self.cur { free } else { self.cur + (free - self.cur) * (1.0 - (-dt / 0.25).exp()) };
        let mut eye = pivot - fwd * self.cur;
        if let Some(g) = ground(eye) {
            eye.z = eye.z.max(g + GROUND_CLEAR);
        }
        Camera { position: Anchor::sim_to_render(eye), fov_y: self.fov as f32, ..*look }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn look(yaw: f32) -> Camera {
        Camera { position: DVec3::ZERO, yaw, pitch: -0.2, fov_y: 1.0, near: 0.1 }
    }

    fn run(pelvis_z: impl Fn(usize) -> f64, grounded: bool) -> Vec<f64> {
        let mut c = RobotCam::default();
        (0..120)
            .map(|k| {
                let cam = c.update(&look(0.0), 6.0, DVec3::new(0.0, 0.0, pelvis_z(k)), DVec3::Z, 0.0, grounded, 1.0 / 60.0, |_, _| None, |_| None);
                Anchor::render_to_sim(cam.position.into()).z
            })
            .collect()
    }

    #[test]
    fn a_step_up_is_eased_not_snapped() {
        let z = run(|k| if k < 30 { 1.0 } else { 1.3 }, true);
        // Right after the 0.3 m step the camera has moved only a little; later it has all of it.
        assert!(z[31] - z[29] < 0.1, "{} {}", z[29], z[31]);
        assert!((z[119] - z[29] - 0.3).abs() < 0.01);
        assert!(z.windows(2).all(|w| w[1] >= w[0] - 1e-9), "monotone rise");
    }

    #[test]
    fn airborne_follows_rigidly() {
        let z = run(|k| if k < 30 { 1.0 } else { 1.3 }, false);
        assert!((z[30] - z[29] - 0.3).abs() < 1e-9);
    }

    #[test]
    fn fov_widens_with_speed() {
        let mut c = RobotCam::default();
        for _ in 0..300 {
            c.update(&look(0.0), 6.0, DVec3::ZERO, DVec3::Z, 24.0, true, 1.0 / 60.0, |_, _| None, |_| None);
        }
        assert!((c.fov.to_degrees() - 63.0).abs() < 0.05);
    }
}
