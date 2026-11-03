//! The robot courier: a physics capsule you steer by *torso velocity*.
//!
//! Input picks a target velocity; a PD controller turns the error into a
//! force (strong on the ground, weak in the air). A ride-height spring keeps
//! the capsule floating just above the ground, which handles slopes, curbs
//! and stairs and gives a natural bounce. The visible robot is procedural
//! (see `body.rs`), driven by what the physics actually does.
//!
//! Modes: Walk, Fall, RocketShoes, Skydive, Parachute.
//! Keys: WASD move, Shift sprint/boost, Space jump / hold for rocket shoes
//! (in air), F parachute (while skydiving), V first/third person, R respawn
//! high (skydive drop), C crouch.

use crate::config::Config;
use crate::origin::{Focus, Rebased, WorldOrigin};
use avian3d::prelude::*;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use ed_flight::{MachEvent, MachTracker, SuperRocket, atmosphere};
use glam::DVec3;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SonicBoom>()
            .add_systems(Startup, spawn_player)
            .add_systems(FixedUpdate, (ground_probe, drive_player).chain())
            .add_systems(Update, (player_input, camera_follow).chain());
    }
}

pub const RIDE_HEIGHT: f32 = 1.0;
const CAPSULE_RADIUS: f32 = 0.32;
const CAPSULE_LEN: f32 = 0.9;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveMode {
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
            MoveMode::Fall => "FALLING",
            MoveMode::RocketShoes => "ROCKET SHOES",
            MoveMode::Skydive => "SKYDIVING",
            MoveMode::Parachute => "PARACHUTE",
        }
    }
}

#[derive(Component)]
pub struct Player {
    pub mode: MoveMode,
    /// Facing yaw (radians, about local up).
    pub yaw: f32,
    pub cam_yaw: f32,
    pub cam_pitch: f32,
    pub cam_dist: f32,
    pub first_person: bool,
    pub grounded: bool,
    pub ground_dist: f32,
    pub ground_normal: Vec3,
    /// Height above ground (from the long probe), meters.
    pub agl: f32,
    pub boost_stage: usize,
    pub throttle: f32,
    pub crouch: bool,
    pub mach: MachTracker,
    /// Input this frame (camera-relative, flat).
    pub wish: Vec3,
    pub sprint: bool,
    pub jump: bool,
    pub thrust_held: bool,
    pub shake: f32,
}

#[derive(Message)]
pub struct SonicBoom(pub Vec3);

impl Player {
    /// A standing, idle player (used for lookdev and spawning).
    pub fn idle(yaw: f32, mode: MoveMode) -> Self {
        Self {
            mode,
            yaw,
            cam_yaw: yaw,
            cam_pitch: -0.25,
            cam_dist: 6.0,
            first_person: false,
            grounded: true,
            ground_dist: RIDE_HEIGHT,
            ground_normal: Vec3::Y,
            agl: 0.0,
            boost_stage: 0,
            throttle: 0.0,
            crouch: false,
            mach: MachTracker::default(),
            wish: Vec3::ZERO,
            sprint: false,
            jump: false,
            thrust_held: false,
            shake: 0.0,
        }
    }
}

fn spawn_player(mut commands: Commands, cfg: Res<Config>, origin: Res<WorldOrigin>) {
    let pos = origin.to_render(ed_geo::Geodetic { alt_m: cfg.start.alt_m.max(0.0) + cfg.start_agl.max(3.0), ..cfg.start });
    let mode = if cfg.start_agl > 150.0 { MoveMode::Skydive } else { MoveMode::Fall };
    commands.spawn((
        Player { agl: cfg.start_agl as f32, grounded: false, ..Player::idle((-cfg.heading_deg).to_radians(), mode) },
        RigidBody::Dynamic,
        Collider::capsule(CAPSULE_RADIUS, CAPSULE_LEN),
        LockedAxes::ROTATION_LOCKED,
        Mass(80.0),
        Friction::new(0.0).with_combine_rule(CoefficientCombine::Min),
        Restitution::new(0.0),
        LinearVelocity::default(),
        SweptCcd::default(),
        Transform::from_translation(pos),
        Rebased,
        Focus,
        Name::new("player"),
    ));
}

/// Short ray for standing, long ray for altitude-above-ground.
fn ground_probe(spatial: SpatialQuery, origin: Res<WorldOrigin>, mut q: Query<(Entity, &Transform, &mut Player)>) {
    for (e, t, mut p) in &mut q {
        let up = origin.up_at(t.translation);
        let filter = SpatialQueryFilter::from_excluded_entities([e]);
        let down = Dir3::new(-up).unwrap_or(Dir3::NEG_Y);
        match spatial.cast_ray(t.translation, down, RIDE_HEIGHT + 0.6, true, &filter) {
            Some(hit) => {
                p.ground_dist = hit.distance;
                p.ground_normal = hit.normal;
                p.grounded = true;
            }
            None => {
                p.ground_dist = f32::MAX;
                p.grounded = false;
            }
        }
        p.agl = spatial.cast_ray(t.translation, down, 20_000.0, true, &filter).map_or(p.agl.max(20_000.0), |h| h.distance - RIDE_HEIGHT);
    }
}

fn player_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut q: Query<&mut Player>,
) {
    let Ok(mut p) = q.single_mut() else { return };
    if buttons.pressed(MouseButton::Right) || buttons.pressed(MouseButton::Left) || p.first_person {
        p.cam_yaw -= mouse.delta.x * 0.0035;
        p.cam_pitch = (p.cam_pitch - mouse.delta.y * 0.0035).clamp(-1.45, 1.35);
    }
    if scroll.delta.y != 0.0 {
        p.cam_dist = (p.cam_dist * 0.85f32.powf(scroll.delta.y)).clamp(2.0, 60.0);
    }
    if keys.just_pressed(KeyCode::KeyV) {
        p.first_person = !p.first_person;
    }
    let mut wish = Vec3::ZERO;
    let (s, c) = p.cam_yaw.sin_cos();
    let fwd = Vec3::new(-s, 0.0, -c);
    let right = Vec3::new(c, 0.0, -s);
    if keys.pressed(KeyCode::KeyW) {
        wish += fwd;
    }
    if keys.pressed(KeyCode::KeyS) {
        wish -= fwd;
    }
    if keys.pressed(KeyCode::KeyD) {
        wish += right;
    }
    if keys.pressed(KeyCode::KeyA) {
        wish -= right;
    }
    p.wish = wish.normalize_or_zero();
    p.sprint = keys.pressed(KeyCode::ShiftLeft);
    p.crouch = keys.pressed(KeyCode::KeyC);
    if keys.just_pressed(KeyCode::Space) {
        p.jump = true;
    }
    p.thrust_held = keys.pressed(KeyCode::Space);
    if keys.just_pressed(KeyCode::KeyF) {
        p.mode = match p.mode {
            MoveMode::Skydive | MoveMode::Fall => MoveMode::Parachute,
            MoveMode::Parachute => MoveMode::Skydive,
            m => m,
        };
    }
    // Boost stages climb while Shift is held in rocket mode.
    if p.mode == MoveMode::RocketShoes {
        if keys.just_pressed(KeyCode::ShiftLeft) {
            p.boost_stage = (p.boost_stage + 1).min(3);
        }
        if keys.just_pressed(KeyCode::ControlLeft) {
            p.boost_stage = p.boost_stage.saturating_sub(1);
        }
    }
}

#[allow(clippy::type_complexity)]
fn drive_player(
    time: Res<Time>,
    origin: Res<WorldOrigin>,
    mut booms: MessageWriter<SonicBoom>,
    mut q: Query<(&Transform, &mut LinearVelocity, &mut Player, Forces)>,
) {
    let dt = time.delta_secs();
    for (t, mut vel, mut p, mut forces) in &mut q {
        let up = origin.up_at(t.translation);
        let mass = 80.0;
        let v = vel.0;
        let v_up = up * v.dot(up);
        let v_flat = v - v_up;
        let alt = origin.to_geodetic(t.translation).alt_m;

        // Mode transitions.
        let grounded = p.grounded && p.ground_dist < RIDE_HEIGHT + 0.25 && v.dot(up) < 3.0;
        p.mode = match p.mode {
            _ if grounded && p.mode != MoveMode::RocketShoes => MoveMode::Walk,
            MoveMode::RocketShoes if grounded && !p.thrust_held => MoveMode::Walk,
            MoveMode::Walk if !p.grounded => MoveMode::Fall,
            MoveMode::Fall if p.thrust_held && !p.jump => MoveMode::RocketShoes,
            MoveMode::Skydive if p.thrust_held && !p.jump => MoveMode::RocketShoes,
            MoveMode::RocketShoes if !p.thrust_held && p.agl > 300.0 => MoveMode::Skydive,
            MoveMode::RocketShoes if !p.thrust_held => MoveMode::Fall,
            MoveMode::Fall if p.agl > 300.0 && v.dot(up) < -25.0 => MoveMode::Skydive,
            m => m,
        };

        // Face where we're going (or where the camera looks when thrusting).
        let target_yaw = if p.wish.length_squared() > 0.01 { (-p.wish.x).atan2(-p.wish.z) } else { p.yaw };
        let dyaw = ((target_yaw - p.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)) - std::f32::consts::PI;
        p.yaw += dyaw * (1.0 - (-10.0 * dt).exp());

        // Express wish in the local tangent frame.
        let base = Quat::from_rotation_arc(Vec3::Y, up);
        let wish = base * p.wish;

        let mut accel = Vec3::ZERO;
        match p.mode {
            MoveMode::Walk => {
                let speed = if p.crouch { 1.6 } else if p.sprint { 8.5 } else { 4.2 };
                let target = wish * speed;
                // Torso-velocity PD controller (P on velocity error, capped).
                let err = target - v_flat;
                accel += (err * 12.0).clamp_length_max(45.0);
                // Ride-height spring-damper keeps us floating over the ground.
                let ride = if p.crouch { RIDE_HEIGHT - 0.35 } else { RIDE_HEIGHT };
                let x = ride - p.ground_dist;
                accel += up * (x * 300.0 - v.dot(up) * 28.0);
                accel += up * 9.81; // cancel gravity while supported
                if p.jump {
                    vel.0 += up * 6.5;
                }
            }
            MoveMode::Fall => {
                accel += (wish * 6.0 - v_flat).clamp_length_max(5.0);
                if p.jump && p.agl < 2.0 {
                    vel.0 += up * 6.5;
                }
            }
            MoveMode::RocketShoes => {
                // Thrust along camera look, pitched up a bit, with superpower stages.
                let (s, c) = p.cam_yaw.sin_cos();
                let look = Vec3::new(-s * p.cam_pitch.cos(), (p.cam_pitch + 0.35).sin().max(0.15), -c * p.cam_pitch.cos()).normalize();
                let facing = (base * look).normalize();
                let g = 9.81;
                let a = SuperRocket::ROCKET_SHOES.accel(
                    DVec3::from(v.as_dvec3()),
                    facing.as_dvec3(),
                    1.0,
                    p.boost_stage,
                    alt,
                    g,
                );
                accel += a.as_vec3();
            }
            MoveMode::Skydive => {
                // Belly-to-earth: lots of drag (~55 m/s terminal), steer by leaning.
                let air = atmosphere(alt);
                let drag = -v * (0.5 * air.density as f32 * v.length() * 0.6 / mass);
                accel += drag + wish * 6.0;
            }
            MoveMode::Parachute => {
                // Canopy: ~5 m/s sink, ~10 m/s forward glide toward wish.
                let air = atmosphere(alt);
                let k = 0.5 * air.density as f32 * 28.0 / mass;
                let drag_v = -v_up * (k * v_up.length() * 1.5);
                let glide = (wish * 10.0 - v_flat) * 1.5;
                accel += drag_v + glide;
            }
        }
        p.jump = false;
        forces.apply_force(accel * mass);

        if let Some(MachEvent::BrokeBarrier) = p.mach.update(v.length() as f64, alt) {
            booms.write(SonicBoom(t.translation));
            p.shake = 1.0;
        }
        p.shake = (p.shake - dt * 1.5).max(0.0);
    }
}

#[allow(clippy::type_complexity)]
fn camera_follow(
    time: Res<Time>,
    origin: Res<WorldOrigin>,
    spatial: SpatialQuery,
    players: Query<(Entity, &Transform, &Player, &LinearVelocity), Without<Camera3d>>,
    mut cams: Query<(&mut Transform, &mut Projection), With<Camera3d>>,
    driving: Option<Res<crate::vehicles::Driving>>,
) {
    if driving.is_some_and(|d| d.0.is_some()) {
        return;
    }
    let Ok((pe, pt, p, vel)) = players.single() else { return };
    let Ok((mut ct, mut proj)) = cams.single_mut() else { return };
    let up = origin.up_at(pt.translation);
    let base = Quat::from_rotation_arc(Vec3::Y, up);
    let look = base * Quat::from_euler(EulerRot::YXZ, p.cam_yaw, p.cam_pitch, 0.0);
    // Speed widens the FOV for the "superpower" feeling.
    let speed = vel.0.length();
    let fov = (62.0 + (speed / 20.0).min(28.0)).to_radians();
    if let Projection::Perspective(pp) = &mut *proj {
        pp.fov += (fov - pp.fov) * (1.0 - (-4.0 * time.delta_secs()).exp());
    }
    let shake = if p.shake > 0.0 {
        let t = time.elapsed_secs() * 47.0;
        Vec3::new(t.sin(), (t * 1.3).cos(), (t * 0.7).sin()) * p.shake * 0.25
    } else {
        Vec3::ZERO
    };
    if p.first_person {
        let head = pt.translation + up * 0.65;
        ct.translation = head + shake;
        ct.rotation = look;
        return;
    }
    let pivot = pt.translation + up * 0.8;
    let back = look * Vec3::Z;
    let filter = SpatialQueryFilter::from_excluded_entities([pe]);
    let dist = Dir3::new(back)
        .ok()
        .and_then(|d| spatial.cast_ray(pivot, d, p.cam_dist, true, &filter))
        .map_or(p.cam_dist, |h| (h.distance - 0.3).max(0.5));
    ct.translation = pivot + back * dist + shake;
    ct.rotation = look;
}
