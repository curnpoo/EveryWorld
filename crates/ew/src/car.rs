//! The placeholder car in play mode (P-slice S5): sim state with render interpolation, box
//! drawing, the chase camera, keyboard mapping and the `EW_AUTODRIVE` script.
//!
//! Physics is [`ed_sim::Vehicle`]; this module only feeds it input once per fixed step and
//! draws the result between steps.

use ed_planet::Anchor;
use ed_render::scene::{InstanceDesc, InstanceId, Material, MeshAsset, MeshLod, NodeSlot};
use ed_render::{Camera, Renderer};
use ed_sim::{QueryFilter, Vehicle, VehicleDesc, VehicleInput, Wheel, World, layers};
use glam::{Affine3A, DQuat, DVec3, Quat, Vec2, Vec3};
use std::collections::BTreeSet;
use winit::keyboard::KeyCode;

/// How close (m, from the car's footprint) the character must be to get in.
pub const ENTER_RANGE_M: f64 = 2.2;
/// A parked car further than this from the collision centre is held in place (its ground may
/// be gone).
pub const PIN_RANGE_M: f64 = 120.0;

/// Chase camera: distance behind the look point and its height above the car, metres; the
/// default pitch (rad, negative looks down); lag time constants (s).
pub const CHASE_DIST: f64 = 6.2;
pub const CHASE_DIST_PER_MPS: f64 = 0.06;
pub const CHASE_HEIGHT: f64 = 1.35;
pub const CHASE_PITCH: f32 = -0.22;
pub const CHASE_YAW_LAG_S: f64 = 0.28;
pub const CHASE_Z_LAG_S: f64 = 0.18;
/// Seconds after the last mouse move before the camera swings back behind the car, and the
/// swing's time constant.
pub const CHASE_RETURN_AFTER_S: f64 = 1.2;
pub const CHASE_RETURN_S: f64 = 0.45;
/// Field of view widens with speed (degrees at rest, extra degrees at `FOV_FULL_MPS`).
pub const FOV_REST_DEG: f32 = 60.0;
pub const FOV_EXTRA_DEG: f32 = 9.0;
pub const FOV_FULL_MPS: f64 = 35.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CarPose {
    pub position: DVec3,
    pub rotation: DQuat,
}

/// The car's sim side plus the last two poses for drawing.
pub struct CarSim {
    pub veh: Vehicle,
    pub prev: (CarPose, [Wheel; 4]),
    pub cur: (CarPose, [Wheel; 4]),
    /// Held pose while parked out of collision range.
    pinned: Option<CarPose>,
}

impl CarSim {
    /// Spawns the car with its wheels on the ground at sim point `ground`, facing `yaw`.
    pub fn spawn(world: &mut World, ground: DVec3, yaw: f64) -> Result<CarSim, String> {
        let desc = VehicleDesc::default();
        let p = ground + DVec3::Z * (desc.ride_height() + 0.05);
        let veh = Vehicle::spawn(world, desc, p, yaw).map_err(|e| format!("car: {e}"))?;
        let pose = CarPose { position: p, rotation: ed_sim::yaw_rotation(yaw) };
        let w = veh.wheels;
        Ok(CarSim { veh, prev: (pose, w), cur: (pose, w), pinned: None })
    }

    /// Puts the car on the ground at `ground` facing `yaw`, at rest (summoned by the player).
    pub fn place(&mut self, world: &mut World, ground: DVec3, yaw: f64) {
        let p = ground + DVec3::Z * (self.veh.desc.ride_height() + 0.05);
        let rotation = ed_sim::yaw_rotation(yaw);
        world.set_transform(self.veh.body, p, rotation);
        world.set_velocity(self.veh.body, DVec3::ZERO, DVec3::ZERO);
        let pose = CarPose { position: p, rotation };
        self.prev.0 = pose;
        self.cur.0 = pose;
        self.pinned = None;
    }

    /// Before the world step: forces from `input`, or the hold when parked far away.
    pub fn pre_step(&mut self, world: &mut World, input: &VehicleInput, focus: DVec3, driving: bool) {
        let far = !driving && (self.cur.0.position - focus).truncate().length() > PIN_RANGE_M;
        if far {
            let pin = *self.pinned.get_or_insert(self.cur.0);
            world.set_transform(self.veh.body, pin.position, pin.rotation);
            world.set_velocity(self.veh.body, DVec3::ZERO, DVec3::ZERO);
            return;
        }
        self.pinned = None;
        self.veh.update(world, input);
    }

    /// After the world step: the new pose.
    pub fn post_step(&mut self, world: &World) {
        self.prev = self.cur;
        if let Some(s) = world.body_state(self.veh.body) {
            self.cur = (CarPose { position: s.position, rotation: s.rotation }, self.veh.wheels);
        }
    }

    /// Pose between the last two steps.
    pub fn pose(&self, alpha: f32) -> CarPose {
        let a = alpha as f64;
        CarPose { position: self.prev.0.position.lerp(self.cur.0.position, a), rotation: self.prev.0.rotation.slerp(self.cur.0.rotation, a) }
    }

    /// Wheels between the last two steps.
    pub fn wheels(&self, alpha: f32) -> [Wheel; 4] {
        let a = alpha as f64;
        let mut out = self.cur.1;
        for (i, w) in out.iter_mut().enumerate() {
            let p = self.prev.1[i];
            w.length = p.length + (w.length - p.length) * a;
            w.steer = p.steer + (w.steer - p.steer) * a;
            let d = (w.spin - p.spin + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
            w.spin = p.spin + d * a;
        }
        out
    }

    /// Speed along the car's forward axis, m/s.
    pub fn speed(&self) -> f64 {
        (self.cur.0.position - self.prev.0.position).dot(self.cur.0.rotation * DVec3::Y) / ed_sim::DT
    }

    /// Distance from sim point `p` to the car's footprint (0 inside it), horizontal.
    pub fn footprint_distance(&self, p: DVec3) -> f64 {
        let pose = self.cur.0;
        let l = pose.rotation.inverse() * (p - pose.position);
        let h = self.veh.desc.body_half.as_dvec3();
        let dx = (l.x.abs() - h.x).max(0.0);
        let dy = (l.y.abs() - h.y).max(0.0);
        (dx * dx + dy * dy).sqrt()
    }

    /// Yaw of the car's forward axis (0 north, counter-clockwise), the camera's convention.
    pub fn yaw(rot: DQuat) -> f64 {
        let f = rot * DVec3::Y;
        (-f.x).atan2(f.y)
    }

    /// Where the character gets out: beside the driver's door, else the passenger side, else
    /// behind. `free(from, to)` says the straight line is clear; `ground(p)` the terrain z.
    pub fn exit_point(&self, free: impl Fn(DVec3, DVec3) -> bool, ground: impl Fn(DVec3) -> Option<f64>) -> Option<DVec3> {
        let pose = self.cur.0;
        let h = self.veh.desc.body_half.as_dvec3();
        let seat = pose.position + pose.rotation * DVec3::new(-0.35, 0.1, 0.3);
        for off in [DVec3::new(-h.x - 0.75, 0.2, 0.0), DVec3::new(h.x + 0.75, 0.2, 0.0), DVec3::new(0.0, -h.y - 0.9, 0.0)] {
            let mut p = pose.position + pose.rotation * off;
            let Some(g) = ground(p) else { continue };
            p.z = g + 1.0;
            if free(seat, p) {
                return Some(DVec3::new(p.x, p.y, g + 0.05));
            }
        }
        None
    }
}

/// Car input from held keys: W/S drive (S brakes, then reverses), A/D steer, Space handbrake.
pub fn drive_input(keys: &BTreeSet<KeyCode>) -> VehicleInput {
    let k = |c| if keys.contains(&c) { 1.0f32 } else { 0.0 };
    let up = k(KeyCode::KeyW).max(k(KeyCode::ArrowUp));
    let down = k(KeyCode::KeyS).max(k(KeyCode::ArrowDown));
    let right = k(KeyCode::KeyD).max(k(KeyCode::ArrowRight));
    let left = k(KeyCode::KeyA).max(k(KeyCode::ArrowLeft));
    VehicleInput { drive: up - down, steer: right - left, handbrake: keys.contains(&KeyCode::Space) }
}

// ---------------------------------------------------------------------------------------------
// Chase camera

/// Behind and above the car, swinging after it with lag; the mouse orbits it and the view
/// returns behind the car shortly after the mouse stops.
#[derive(Clone, Copy, Debug)]
pub struct ChaseCam {
    /// Lagged yaw behind the car, and the mouse orbit offset on top of it.
    pub yaw: f64,
    pub orbit_yaw: f64,
    pub orbit_pitch: f64,
    pub since_look: f64,
    /// Lagged look-point height.
    pub z: Option<f64>,
    /// Current (pulled-in) distance.
    pub cur_dist: f64,
    pub zoom: f64,
}

fn wrap(a: f64) -> f64 {
    (a + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI
}

impl ChaseCam {
    pub fn new(yaw: f64) -> ChaseCam {
        ChaseCam { yaw, orbit_yaw: 0.0, orbit_pitch: 0.0, since_look: 10.0, z: None, cur_dist: CHASE_DIST, zoom: 1.0 }
    }

    /// The whole camera yaw and pitch now (for handing over to the walking camera).
    pub fn view(&self) -> (f32, f32) {
        (wrap(self.yaw + self.orbit_yaw) as f32, (CHASE_PITCH as f64 + self.orbit_pitch) as f32)
    }

    /// Advances the camera for a frame. `look` is mouse pixels (radians per pixel `rad_px`),
    /// `pose` the interpolated car, `speed` its forward speed. `blocked(from, d)` is the free
    /// fraction of a ray; `ground(p)` the terrain z under `p`.
    #[allow(clippy::too_many_arguments)]
    pub fn update(&mut self, pose: CarPose, speed: f64, look: Vec2, zoom: f32, rad_px: f32, dt: f64, blocked: impl Fn(DVec3, DVec3) -> Option<f64>, ground: impl Fn(DVec3) -> Option<f64>) -> Camera {
        // Mouse orbit, then the swing back behind the car.
        if look != Vec2::ZERO {
            self.orbit_yaw = wrap(self.orbit_yaw - (look.x * rad_px) as f64);
            self.orbit_pitch = (self.orbit_pitch - (look.y * rad_px) as f64).clamp(-0.9, 0.9);
            self.since_look = 0.0;
        } else {
            self.since_look += dt;
        }
        if self.since_look > CHASE_RETURN_AFTER_S {
            let k = 1.0 - (-dt / CHASE_RETURN_S).exp();
            self.orbit_yaw -= self.orbit_yaw * k;
            self.orbit_pitch -= self.orbit_pitch * k;
        }
        self.zoom = (self.zoom * (1.0 - zoom as f64 * 0.1)).clamp(0.5, 3.0);
        // Yaw follows the car's heading (its nose, also when reversing), lagged.
        let car_yaw = CarSim::yaw(pose.rotation);
        let k = 1.0 - (-dt / CHASE_YAW_LAG_S).exp();
        self.yaw = wrap(self.yaw + wrap(car_yaw - self.yaw) * k);
        // Look point: exact in xy (no judder), lagged in z (bumps).
        let want_z = pose.position.z + CHASE_HEIGHT;
        let z = match self.z {
            None => want_z,
            Some(z) => {
                let n = z + (want_z - z) * (1.0 - (-dt / CHASE_Z_LAG_S).exp());
                n.clamp(want_z - 0.6, want_z + 0.6)
            }
        };
        self.z = Some(z);
        let target = DVec3::new(pose.position.x, pose.position.y, z);
        let (yaw, pitch) = self.view();
        let fov = (FOV_REST_DEG + FOV_EXTRA_DEG * (speed.abs() / FOV_FULL_MPS).min(1.0) as f32).to_radians();
        let look = Camera { position: DVec3::ZERO, yaw, pitch, fov_y: fov, near: Camera::DEFAULT_NEAR };
        let fwd = Anchor::render_to_sim(look.forward().as_dvec3());
        let dist = (CHASE_DIST + CHASE_DIST_PER_MPS * speed.abs().min(40.0)) * self.zoom;
        let back = -fwd * dist;
        let free = blocked(target, back).map_or(dist, |f| (f * dist - 0.4).max(1.0));
        self.cur_dist = if free < self.cur_dist { free } else { self.cur_dist + (free - self.cur_dist) * (1.0 - (-dt / 0.3).exp()) };
        let mut eye = target - fwd * self.cur_dist;
        if let Some(g) = ground(eye) {
            eye.z = eye.z.max(g + 0.6);
        }
        Camera { position: Anchor::sim_to_render(eye), ..look }
    }
}

// ---------------------------------------------------------------------------------------------
// Drawing

/// A cylinder along x (radius 0.5, length 1), flat-shaded sides so its turning shows.
fn wheel_mesh(sides: u32) -> MeshAsset {
    let mut m = MeshAsset::default();
    let mut idx = Vec::new();
    let ring = |i: u32| {
        let a = i as f32 / sides as f32 * std::f32::consts::TAU;
        (a.cos() * 0.5, a.sin() * 0.5)
    };
    for i in 0..sides {
        let ((y0, z0), (y1, z1)) = (ring(i), ring(i + 1));
        let n = Vec3::new(0.0, y0 + y1, z0 + z1).normalize();
        let b = m.positions.len() as u32;
        for p in [[-0.5, y0, z0], [-0.5, y1, z1], [0.5, y1, z1], [0.5, y0, z0]] {
            m.positions.push(p);
            m.normals.push(n.to_array());
        }
        idx.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
        for (x, nx) in [(-0.5f32, -1.0f32), (0.5, 1.0)] {
            let b = m.positions.len() as u32;
            m.positions.push([x, 0.0, 0.0]);
            m.positions.push([x, y0, z0]);
            m.positions.push([x, y1, z1]);
            for _ in 0..3 {
                m.normals.push([nx, 0.0, 0.0]);
            }
            if nx > 0.0 { idx.extend_from_slice(&[b, b + 1, b + 2]) } else { idx.extend_from_slice(&[b, b + 2, b + 1]) }
        }
    }
    m.lods.push(MeshLod { indices: idx, error: 0.0 });
    m
}

/// Render-frame rotation of a sim-frame rotation.
fn to_render(q: DQuat) -> Quat {
    DQuat::from_xyzw(q.x, q.z, -q.y, q.w).as_quat()
}

/// Car-frame size (x right, y forward, z up) to the scale of a render-axis unit cube placed
/// with [`to_render`].
fn scale(s: Vec3) -> Vec3 {
    Vec3::new(s.x, s.z, s.y)
}

/// The car's instances: body, cabin, bumpers, lights, four tyres and hubs.
pub struct CarView {
    node: NodeSlot,
    parts: Vec<(InstanceId, Vec3, Vec3)>,
    wheels: [(InstanceId, InstanceId); 4],
}

impl CarView {
    pub fn new(r: &mut Renderer, dev: &dyn ed_gpu::Device, desc: &VehicleDesc) -> Result<CarView, String> {
        let cube = r.add_mesh(dev, &ed_render::cube_mesh()).map_err(|e| format!("car mesh: {e}"))?;
        let cyl = r.add_mesh(dev, &wheel_mesh(14)).map_err(|e| format!("car mesh: {e}"))?;
        let node = r.instances().add_node(DVec3::ZERO).ok_or("car: node table full")?;
        let mut mat = |m: Material| r.add_material(m).ok_or("car: material table full");
        let paint = mat(Material::pbr(Vec3::new(0.62, 0.30, 0.08), 0.0, 0.35))?;
        let glass = mat(Material::pbr(Vec3::new(0.03, 0.04, 0.05), 0.0, 0.08))?;
        let trim = mat(Material::pbr(Vec3::new(0.05, 0.05, 0.05), 0.0, 0.7))?;
        let tyre = mat(Material::pbr(Vec3::new(0.025, 0.025, 0.025), 0.0, 0.9))?;
        let hub = mat(Material::pbr(Vec3::new(0.55, 0.56, 0.58), 0.9, 0.3))?;
        let lamp = mat(Material::pbr(Vec3::new(0.95, 0.93, 0.85), 0.0, 0.1))?;
        let tail = mat(Material::pbr(Vec3::new(0.6, 0.03, 0.02), 0.0, 0.2))?;
        let b = desc.body_half * 2.0;
        let c = desc.cabin_half * 2.0;
        let co = desc.cabin_offset;
        // (material, car-frame centre, car-frame size)
        let boxes: Vec<(_, Vec3, Vec3)> = vec![
            (paint, Vec3::new(0.0, 0.0, 0.0), b),
            (glass, co, c),
            (paint, co + Vec3::new(0.0, 0.0, c.z * 0.5 + 0.02), Vec3::new(c.x * 0.96, c.y * 0.9, 0.04)),
            (trim, Vec3::new(0.0, b.y * 0.5 + 0.03, -b.z * 0.3), Vec3::new(b.x * 0.98, 0.1, b.z * 0.35)),
            (trim, Vec3::new(0.0, -b.y * 0.5 - 0.03, -b.z * 0.3), Vec3::new(b.x * 0.98, 0.1, b.z * 0.35)),
            (lamp, Vec3::new(-b.x * 0.36, b.y * 0.5 + 0.01, b.z * 0.18), Vec3::new(0.32, 0.04, 0.1)),
            (lamp, Vec3::new(b.x * 0.36, b.y * 0.5 + 0.01, b.z * 0.18), Vec3::new(0.32, 0.04, 0.1)),
            (tail, Vec3::new(-b.x * 0.38, -b.y * 0.5 - 0.01, b.z * 0.2), Vec3::new(0.3, 0.04, 0.09)),
            (tail, Vec3::new(b.x * 0.38, -b.y * 0.5 - 0.01, b.z * 0.2), Vec3::new(0.3, 0.04, 0.09)),
        ];
        let mut n = 0u128;
        let mut add = |r: &mut Renderer, mesh, material| {
            n += 1;
            r.instances()
                .add(&InstanceDesc { node, transform: Affine3A::IDENTITY, mesh, material, object: ed_geo::ObjectId(0xCA2_0000 + n), flags: 0 })
                .ok_or("car: instance pool full")
        };
        let mut parts = Vec::new();
        for (m, p, s) in boxes {
            parts.push((add(r, cube, m)?, p, s));
        }
        let mut wheels = Vec::new();
        for _ in 0..4 {
            wheels.push((add(r, cyl, tyre)?, add(r, cube, hub)?));
        }
        Ok(CarView { node, parts, wheels: [wheels[0], wheels[1], wheels[2], wheels[3]] })
    }

    /// Places every part for the interpolated pose and wheels.
    pub fn draw(&self, r: &mut Renderer, desc: &VehicleDesc, pose: CarPose, wheels: &[Wheel; 4]) {
        r.instances().set_node(self.node, Anchor::sim_to_render(pose.position));
        let q = to_render(pose.rotation);
        let qs = pose.rotation.as_quat();
        let place = |p: Vec3| Anchor::sim_to_render((qs * p).as_dvec3()).as_vec3();
        for (id, p, s) in &self.parts {
            let t = Affine3A::from_scale_rotation_translation(scale(*s), q, place(*p));
            r.instances().set_transform(*id, &t);
        }
        let mounts = desc.mounts();
        let rad = desc.wheel_radius as f32;
        for (i, (tyre, hub)) in self.wheels.iter().enumerate() {
            let w = wheels[i];
            let centre = mounts[i].as_vec3() - Vec3::Z * w.length as f32;
            // Car frame: steer about z (positive right = clockwise), roll forward about -x.
            let part = Quat::from_rotation_z(-w.steer as f32) * Quat::from_rotation_x(-w.spin as f32);
            let rot = q * to_render(part.as_dquat());
            let tt = Affine3A::from_scale_rotation_translation(Vec3::new(0.24, rad * 2.0, rad * 2.0), rot, place(centre));
            r.instances().set_transform(*tyre, &tt);
            let side = if mounts[i].x < 0.0 { -1.0 } else { 1.0 };
            let hub_c = centre + (Quat::from_rotation_z(-w.steer as f32) * Vec3::new(side * 0.125, 0.0, 0.0));
            let th = Affine3A::from_scale_rotation_translation(Vec3::new(0.02, rad * 1.1, rad * 0.35), rot, place(hub_c));
            r.instances().set_transform(*hub, &th);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Auto-drive

/// What the auto-drive script does at a moment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AutoPhase {
    WalkToCar,
    Enter,
    Drive,
    Turn,
    Brake,
    Exit,
    Done,
}

/// Phase at script time `t` of `total` seconds.
pub fn auto_phase(t: f64, total: f64) -> AutoPhase {
    let f = t / total;
    if t < 1.0 {
        AutoPhase::WalkToCar
    } else if t < 1.2 {
        AutoPhase::Enter
    } else if f < 0.55 {
        AutoPhase::Drive
    } else if f < 0.72 {
        AutoPhase::Turn
    } else if f < 0.86 {
        AutoPhase::Brake
    } else if f < 0.9 {
        AutoPhase::Exit
    } else {
        AutoPhase::Done
    }
}

/// The flattest heading (camera yaw convention) within `spread` radians of `around`, seen from
/// sim point `p`: least height change over the next ~100 m, straight on preferred a little.
/// `ground(p)` is the terrain z under sim point `p`. For the auto-drive script.
pub fn flattest_heading(p: DVec3, around: f64, spread: f64, ground: impl Fn(DVec3) -> Option<f64>) -> f64 {
    let z0 = ground(p).unwrap_or(p.z);
    let n = 24;
    let mut best = (f64::MAX, around);
    for i in 0..=n {
        let a = -spread + 2.0 * spread * i as f64 / n as f64;
        let yaw = around + a;
        let dir = DVec3::new(-yaw.sin(), yaw.cos(), 0.0);
        let mut cost = a.abs() * 0.5;
        let mut prev = z0;
        for d in [15.0, 30.0, 50.0, 75.0, 100.0] {
            match ground(p + dir * d) {
                Some(z) => {
                    cost += (z - z0).abs() / d * 10.0 + (z - prev).abs() * 0.3;
                    prev = z;
                }
                None => cost += 20.0,
            }
        }
        if cost < best.0 {
            best = (cost, yaw);
        }
    }
    wrap(best.1)
}

/// Steering (positive right) that turns the car at `pose` toward `heading`.
pub fn steer_toward(pose: CarPose, heading: f64) -> f32 {
    // Heading is counter-clockwise; steer is positive clockwise.
    (-wrap(heading - CarSim::yaw(pose.rotation)) * 2.5).clamp(-1.0, 1.0) as f32
}

/// The query filter that sees terrain and other bodies but not cars or movers (camera rays).
pub fn scenery_filter() -> QueryFilter {
    QueryFilter { category: layers::DEFAULT, mask: layers::ALL & !(layers::MOVER | layers::VEHICLE) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_rotation_matches_anchor_frame() {
        let q = ed_sim::yaw_rotation(0.7) * DQuat::from_rotation_x(0.3);
        let v = DVec3::new(0.3, -1.2, 0.8);
        let a = Anchor::sim_to_render(q * v);
        let b = to_render(q).as_dquat() * Anchor::sim_to_render(v);
        assert!((a - b).length() < 1e-5, "{a:?} vs {b:?}");
    }

    #[test]
    fn car_yaw_matches_camera_yaw() {
        for yaw in [0.0, 0.8, -2.0, 3.0] {
            let rot = ed_sim::yaw_rotation(yaw);
            assert!((CarSim::yaw(rot) - yaw).abs() < 1e-9);
            let cam = Camera { position: DVec3::ZERO, yaw: yaw as f32, pitch: 0.0, fov_y: 1.0, near: 0.1 };
            let f = Anchor::render_to_sim(cam.forward().as_dvec3());
            assert!((f - rot * DVec3::Y).length() < 1e-5);
        }
    }

    #[test]
    fn wheel_mesh_is_closed_and_sized() {
        let m = wheel_mesh(14);
        assert_eq!(m.positions.len(), m.normals.len());
        assert_eq!(m.lods[0].indices.len(), 14 * 12);
        assert!(m.positions.iter().all(|p| p[0].abs() <= 0.5 && (p[1] * p[1] + p[2] * p[2]).sqrt() <= 0.5 + 1e-6));
    }

    #[test]
    fn drive_keys() {
        let keys: BTreeSet<KeyCode> = [KeyCode::KeyW, KeyCode::KeyD, KeyCode::Space].into_iter().collect();
        let i = drive_input(&keys);
        assert_eq!((i.drive, i.steer, i.handbrake), (1.0, 1.0, true));
    }

    #[test]
    fn autodrive_phases_in_order() {
        let mut last = AutoPhase::WalkToCar;
        let order = [AutoPhase::WalkToCar, AutoPhase::Enter, AutoPhase::Drive, AutoPhase::Turn, AutoPhase::Brake, AutoPhase::Exit, AutoPhase::Done];
        for i in 0..200 {
            let p = auto_phase(i as f64 * 0.1, 20.0);
            let (a, b) = (order.iter().position(|x| *x == last).unwrap(), order.iter().position(|x| *x == p).unwrap());
            assert!(b >= a);
            last = p;
        }
        assert_eq!(last, AutoPhase::Done);
    }
}
