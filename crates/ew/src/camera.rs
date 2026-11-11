//! Cameras driven from the fixed step: the free camera (WASD + mouse look) and the scripted
//! paths used by the headless harness. Both keep the previous and current state so the
//! renderer can interpolate with the step fraction.

use crate::fixed_step::Simulation;
use ed_render::{Camera, TestScene};
use glam::{DVec3, Vec2, Vec3};

/// Camera pose in the simulation: f64 position, yaw/pitch in radians.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub position: DVec3,
    pub yaw: f32,
    pub pitch: f32,
}

impl Pose {
    pub fn from_camera(c: &Camera) -> Self {
        Self { position: c.position, yaw: c.yaw, pitch: c.pitch }
    }
    pub fn lerp(a: &Pose, b: &Pose, t: f32) -> Pose {
        Pose {
            position: a.position.lerp(b.position, t as f64),
            yaw: a.yaw + (b.yaw - a.yaw) * t,
            pitch: a.pitch + (b.pitch - a.pitch) * t,
        }
    }
    pub fn camera(&self) -> Camera {
        Camera { position: self.position, yaw: self.yaw, pitch: self.pitch, fov_y: Camera::DEFAULT_FOV_Y, near: Camera::DEFAULT_NEAR }
    }
}

/// Input snapshot for one step.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CameraInput {
    /// x: right, y: up, z: forward; each -1, 0 or 1.
    pub axes: Vec3,
    /// Mouse movement since the last step, pixels (given to one step only).
    pub look: Vec2,
    pub fast: bool,
}

/// Free-flying camera.
#[derive(Clone, Debug)]
pub struct FreeCamera {
    pub prev: Pose,
    pub cur: Pose,
    pub speed: f64,
    pub fast_speed: f64,
    /// Radians per pixel of mouse movement.
    pub sensitivity: f32,
}

impl FreeCamera {
    pub fn new(start: Pose) -> Self {
        Self { prev: start, cur: start, speed: 8.0, fast_speed: 60.0, sensitivity: 0.003 }
    }
    pub fn interpolated(&self, alpha: f32) -> Camera {
        Pose::lerp(&self.prev, &self.cur, alpha).camera()
    }
}

impl Simulation for FreeCamera {
    type Input = CameraInput;
    fn step(&mut self, input: &CameraInput, dt: f64) {
        self.prev = self.cur;
        let p = &mut self.cur;
        p.yaw -= input.look.x * self.sensitivity;
        p.pitch = (p.pitch - input.look.y * self.sensitivity).clamp(-1.55, 1.55);
        let cam = p.camera();
        let dir = cam.right().as_dvec3() * input.axes.x as f64 + DVec3::Y * input.axes.y as f64 + cam.forward().as_dvec3() * input.axes.z as f64;
        if dir.length_squared() > 0.0 {
            let v = if input.fast { self.fast_speed } else { self.speed };
            p.position += dir.normalize() * v * dt;
        }
    }
}

/// Scripted camera paths for the harness. Deterministic: a pure function of the step count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathKind {
    /// Circles the test scene once every 10 s at 24 m radius, 8 m up, looking at its centre.
    Orbit,
    /// The image-test camera, not moving.
    Static,
    /// The M2 proving ground ([`crate::proving::path_pose`]): street level, rooftop pass, long
    /// sight line; 10 s at speed 1, then repeats.
    Proving,
}

#[derive(Clone, Debug)]
pub struct ScriptedPath {
    pub kind: PathKind,
    /// Path time per real second (1 = authored speed).
    pub speed: f64,
    pub step: u64,
    pub prev: Pose,
    pub cur: Pose,
}

impl ScriptedPath {
    pub fn new(kind: PathKind) -> Self {
        Self::with_speed(kind, 1.0)
    }

    pub fn with_speed(kind: PathKind, speed: f64) -> Self {
        let p = Self::pose_at(kind, 0, speed);
        Self { kind, speed, step: 0, prev: p, cur: p }
    }

    pub fn pose(kind: PathKind, step: u64) -> Pose {
        Self::pose_at(kind, step, 1.0)
    }

    /// Pose after `step` fixed steps (60 Hz) at `speed`.
    pub fn pose_at(kind: PathKind, step: u64, speed: f64) -> Pose {
        let focus = TestScene::ORIGIN + TestScene::FOCUS.as_dvec3();
        match kind {
            PathKind::Static => Pose::from_camera(&TestScene::test_camera()),
            PathKind::Orbit => {
                let a = step as f64 * std::f64::consts::TAU / 600.0;
                let pos = focus + DVec3::new(24.0 * a.sin(), 8.0, 24.0 * a.cos());
                Pose::from_camera(&Camera::look_at(pos, focus))
            }
            PathKind::Proving => crate::proving::path_pose(step as f64 / 60.0 * speed),
        }
    }

    pub fn interpolated(&self, alpha: f32) -> Camera {
        Pose::lerp(&self.prev, &self.cur, alpha).camera()
    }
}

impl Simulation for ScriptedPath {
    type Input = ();
    fn step(&mut self, _: &(), _dt: f64) {
        self.step += 1;
        self.prev = self.cur;
        self.cur = Self::pose_at(self.kind, self.step, self.speed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_camera_moves_forward_and_interpolates() {
        let start = Pose { position: DVec3::new(1e7, 2.0, 0.0), yaw: 0.0, pitch: 0.0 };
        let mut c = FreeCamera::new(start);
        c.step(&CameraInput { axes: Vec3::Z, ..Default::default() }, 0.5);
        assert!((c.cur.position.z - -4.0).abs() < 1e-9, "8 m/s forward (-Z) for 0.5 s");
        let mid = c.interpolated(0.5);
        assert!((mid.position.z - -2.0).abs() < 1e-9);
        assert_eq!(mid.position.x, 1e7);
    }

    #[test]
    fn mouse_look_turns_and_pitch_is_clamped() {
        let mut c = FreeCamera::new(Pose { position: DVec3::ZERO, yaw: 0.0, pitch: 0.0 });
        c.step(&CameraInput { look: Vec2::new(-100.0, -10_000.0), ..Default::default() }, 1.0 / 60.0);
        assert!(c.cur.yaw > 0.0, "moving the mouse left turns left");
        assert_eq!(c.cur.pitch, 1.55);
    }

    #[test]
    fn orbit_is_deterministic_and_periodic() {
        let mut a = ScriptedPath::new(PathKind::Orbit);
        let mut b = ScriptedPath::new(PathKind::Orbit);
        for _ in 0..600 {
            a.step(&(), 1.0 / 60.0);
            b.step(&(), 1.0 / 60.0);
        }
        assert_eq!(a.cur, b.cur);
        let start = ScriptedPath::pose(PathKind::Orbit, 0);
        assert!(a.cur.position.distance(start.position) < 1e-6, "back to the start after 600 steps");
    }
}
