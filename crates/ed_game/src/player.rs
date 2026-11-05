//! The robot courier: a physical self-balancing biped driven by foot forces,
//! landing recovery, and a clean 6-case rocket/flight control scheme.
//!
//! Flight & Rocket Control Matrix (in the air):
//! 1. **WASD only (no Space, no Shift):** Air manipulation & wingsuit tilting.
//!    Rotates/tilts the body in the air (`W` pitches forward/dives, `S` leans back
//!    /flares to catch air, `A`/`D` banks/rolls) without firing rockets.
//! 2. **Space only (no WASD, no Shift):** Hover & brake stabilizer.
//!    Tilts the body against any horizontal velocity (`-v_flat`) to kill lateral
//!    and forward speed, rights the robot to vertical, and climbs gently upward.
//! 3. **Space + WASD (no Shift):** Camera-relative directional rocket cruise.
//!    Rotates the feet opposite the 3D camera direction (`+cam_fwd_3d` for `W`,
//!    `-cam_fwd_3d` for `S` to retro-boost in the opposite direction of the
//!    camera, `±cam_right_3d` for `D`/`A`) and fires controlled cruise thrust.
//! 4. **Space + Shift (no WASD):** Extreme vertical human-rocket launch straight
//!    upward along local `up` while killing horizontal drift.
//! 5. **Space + Shift + WASD:** Extreme afterburner acceleration in the 3D
//!    camera-relative direction you are pressing, ramping smoothly with time.
//! 6. **Shift only (no Space, any time—even on the ground):** Raw boot thrusters!
//!    Fires the rockets straight out of whatever direction the feet are currently
//!    facing, without overriding your orientation.
//!
//! Ground & Landing Physics:
//! - When landing from a fall or high-speed flight, the robot enters a physical
//!   landing-impact / slide recovery state (`landing_recovery`): the knees absorbs
//!   the impact, the body slides/decelerates under ground friction, and it waits
//!   until it has nearly stopped before standing up and running when you press `W`.

use crate::config::Config;
use crate::origin::{Focus, Rebased, WorldOrigin};
use avian3d::prelude::*;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use ed_flight::{MachEvent, MachTracker, atmosphere, flyer};

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SonicBoom>()
            .init_resource::<ControlSettings>()
            .init_resource::<CamRig>()
            .add_systems(Startup, spawn_player)
            .init_resource::<Settling>()
            .add_systems(
                FixedUpdate,
                (ground_probe, drive_player, hold_until_ground)
                    .chain()
                    .run_if(in_state(crate::menu::GameState::Playing)),
            )
            .add_systems(
                Update,
                (
                    player_input.run_if(in_state(crate::menu::GameState::Playing)),
                    camera_follow,
                )
                    .chain(),
            );
    }
}

/// Loose spring-damper state for the third-person camera focus point.
#[derive(Resource, Default)]
pub struct CamRig {
    pub init: bool,
    pub focus_pos: Vec3,
    pub focus_vel: Vec3,
}

/// While set, the player is frozen in place until a collider exists below
/// (or the timeout passes), so arriving before tiles load doesn't mean falling.
#[derive(Resource)]
pub struct Settling {
    pub until_s: Option<f32>,
}

impl Default for Settling {
    fn default() -> Self {
        Self { until_s: Some(f32::MAX) }
    }
}

/// Height above the real ground (m) the player is re-placed at once their tile
/// has a collider, so they drop in gently rather than start inside the terrain.
const SETTLE_DROP_M: f32 = 4.0;

fn hold_until_ground(
    time: Res<Time<Real>>,
    spatial: SpatialQuery,
    origin: Res<WorldOrigin>,
    mut settling: ResMut<Settling>,
    mut commands: Commands,
    mut q: Query<(
        Entity,
        &Player,
        &Transform,
        &mut LinearVelocity,
        Option<&mut AngularVelocity>,
    )>,
) {
    let Some(deadline) = settling.until_s else { return };
    let Ok((e, p, t, mut v, ang_v)) = q.single_mut() else { return };
    let now = time.elapsed_secs();
    let deadline = if deadline == f32::MAX { now + 20.0 } else { deadline };
    // Ground-level spawns: wait for a collider under the player (looking from
    // well above, in case the spawn point ended up under the surface).
    const LOOK_FROM_M: f32 = 1000.0;
    let up = origin.up_at(t.translation);
    let hit = (p.mode == MoveMode::Fall)
        .then(|| {
            let filter = SpatialQueryFilter::from_excluded_entities([e]);
            let from = t.translation + up * LOOK_FROM_M;
            let down = Dir3::new(-up).unwrap_or(Dir3::NEG_Y);
            spatial.cast_ray(from, down, 3.0 * LOOK_FROM_M, true, &filter).map(|h| from - up * h.distance)
        })
        .flatten();
    if p.agl < 19_000.0 || now > deadline {
        settling.until_s = None;
        commands.entity(e).remove::<GravityScale>();
        // Only a ground spawn (within ~30 m of the surface) is re-placed.
        if let Some(g) = hit.filter(|g| (t.translation - *g).length() < 30.0) {
            let np = g + up * (RIDE_HEIGHT + SETTLE_DROP_M);
            // Deferred writes: `SpatialQuery` already borrows Position/Transform.
            commands.entity(e).insert((Transform { translation: np, ..*t }, Position(np)));
            v.0 = Vec3::ZERO;
        }
    } else {
        settling.until_s = Some(deadline);
        v.0 = Vec3::ZERO;
        if let Some(mut av) = ang_v {
            av.0 = Vec3::ZERO;
        }
        commands.entity(e).insert(GravityScale(0.0));
    }
}

pub const RIDE_HEIGHT: f32 = 0.95;
const CAPSULE_RADIUS: f32 = 0.25;
const CAPSULE_LEN: f32 = 0.48;
const HIP_HALF_W: f32 = 0.13;

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
    /// Distance the scroll wheel asked for; `cam_dist` eases toward it.
    pub cam_dist_goal: f32,
    pub first_person: bool,
    pub grounded: bool,
    pub was_grounded: bool,
    pub ground_dist: f32,
    pub ground_normal: Vec3,
    /// Height above ground (from the long probe), meters.
    pub agl: f32,
    pub boost_stage: usize,
    /// Current rocket thrust level (0.0 = off, ~1.0 = Space cruise, 1.5..4.0 = Shift afterburner).
    pub throttle: f32,
    /// How far the rocket (Space / RT) and afterburner (Shift / LT) are pressed, 0..1. Keys are 1.
    pub rocket_amt: f32,
    pub boost_amt: f32,
    pub afterburner: bool,
    pub crouch: bool,
    pub mach: MachTracker,
    /// Input this frame (camera-relative, flat).
    pub wish: Vec3,
    pub sprint: bool,
    pub jump: bool,
    pub thrust_held: bool,
    pub shake: f32,
    /// Raw inputs this frame.
    pub shift: bool,
    pub space_held: bool,
    pub space_just_pressed: bool,
    pub space_just_released: bool,
    pub strafe: f32,
    pub fwd_in: f32,
    /// Seconds Shift afterburner has been held (ramps up smoothly).
    pub boost_time: f32,
    /// 0..1 charge while holding Space on the ground to squat before jumping.
    pub jump_charge: f32,
    /// Remaining seconds of powerful leg-extension push against the ground during liftoff.
    pub jump_push_timer: f32,
    /// True once the player has left the ground and can hold/press Space in mid-air to fire boots.
    pub space_air_latched: bool,
    /// Landing recovery timer (> 0 when recovering from a fall/landing impact; waits until nearly stopped before running).
    pub landing_recovery: f32,
    /// Previous frame's linear velocity for computing real physical acceleration.
    pub prev_vel: Vec3,
    /// Smoothed physical linear acceleration in world space.
    pub accel_smooth: Vec3,
    /// Smoothed world-space rotation error (axis * angle) between the body and its aimed posture; drives limb effort in `body.rs`.
    pub rot_err: Vec3,
    /// Camera-relative direction the player wants to travel (zero without WASD).
    pub intent_dir: Vec3,
    /// F toggles the wingsuit (membranes + wing aerodynamics).
    pub wingsuit_on: bool,
    /// Smoothed turn demand while gliding: +1 = hard left (look left of the flight path), -1 = hard right.
    pub wing_turn: f32,
    /// Smoothed wingsuit arch: +1 = wings pulled up / body arched (flare), -1 = straightened out (dive).
    pub wing_arch: f32,
    /// Rocket legs: body-local swing of the feet away from their rest pose (x sideways, z fore/aft),
    /// which moves the boots' thrust line off the centre of mass and steers the body. Drives `body.rs`.
    pub leg_steer: Vec3,
    /// Freefall without the suit, 0..1: balled up for a somersault/barrel roll. Drives `body.rs`.
    pub fall_tuck: f32,
    /// Freefall without the suit, 0..1: spread eagle (S). Drives `body.rs`.
    pub fall_spread: f32,
    /// Radians the view still trails behind the character's own turning; eases to 0 so the camera slowly catches up.
    pub view_lag: f32,
    /// Actual world-space ankle/foot positions and planted state updated by `body.rs`.
    pub foot_world_pos: [Vec3; 2],
    /// Unit vector pointing out of the top of each foot (+Y of the shoe; sole points `-foot_up_dir`).
    pub foot_up_dir: [Vec3; 2],
    /// Whether each foot is currently planted on the ground (`true`) or stepping/airborne (`false`).
    pub foot_planted: [bool; 2],
    /// Ground height directly under each hip (stable, non-feedback raycast).
    pub foot_ground_hit: [Option<Vec3>; 2],
}

/// Mouse look sensitivity and other control settings.
#[derive(Resource, Clone)]
pub struct ControlSettings {
    pub mouse_sensitivity: f32,
    pub invert_y: bool,
}

impl Default for ControlSettings {
    fn default() -> Self {
        Self { mouse_sensitivity: 1.0, invert_y: false }
    }
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
            cam_dist_goal: 6.0,
            first_person: false,
            grounded: true,
            was_grounded: true,
            ground_dist: RIDE_HEIGHT,
            ground_normal: Vec3::Y,
            agl: 0.0,
            boost_stage: 0,
            throttle: 0.0,
            rocket_amt: 1.0,
            boost_amt: 1.0,
            afterburner: false,
            crouch: false,
            mach: MachTracker::default(),
            wish: Vec3::ZERO,
            sprint: false,
            jump: false,
            thrust_held: false,
            shake: 0.0,
            shift: false,
            space_held: false,
            space_just_pressed: false,
            space_just_released: false,
            strafe: 0.0,
            fwd_in: 0.0,
            boost_time: 0.0,
            jump_charge: 0.0,
            jump_push_timer: 0.0,
            space_air_latched: false,
            landing_recovery: 0.0,
            prev_vel: Vec3::ZERO,
            accel_smooth: Vec3::ZERO,
            rot_err: Vec3::ZERO,
            intent_dir: Vec3::ZERO,
            wingsuit_on: true,
            wing_turn: 0.0,
            wing_arch: 0.0,
            leg_steer: Vec3::ZERO,
            fall_tuck: 0.0,
            fall_spread: 0.0,
            view_lag: 0.0,
            foot_world_pos: [Vec3::new(-HIP_HALF_W, -RIDE_HEIGHT, 0.0), Vec3::new(HIP_HALF_W, -RIDE_HEIGHT, 0.0)],
            foot_up_dir: [Vec3::Y; 2],
            foot_planted: [true; 2],
            foot_ground_hit: [None; 2],
        }
    }
}

fn spawn_player(mut commands: Commands, cfg: Res<Config>, origin: Res<WorldOrigin>) {
    let pos = origin.to_render(ed_geo::Geodetic {
        alt_m: cfg.start.alt_m.max(0.0) + cfg.start_agl.max(3.0),
        ..cfg.start
    });
    let up = origin.up_at(pos);
    let yaw = (-cfg.heading_deg).to_radians();
    let rot = Quat::from_rotation_arc(Vec3::Y, up) * Quat::from_rotation_y(yaw);
    let right = rot * Vec3::X;
    let mode = if cfg.start_agl > 150.0 { MoveMode::Skydive } else { MoveMode::Fall };
    let mut p = Player {
        agl: cfg.start_agl as f32,
        grounded: false,
        was_grounded: false,
        ..Player::idle(yaw, mode)
    };
    p.foot_world_pos = [
        pos - up * RIDE_HEIGHT - right * HIP_HALF_W,
        pos - up * RIDE_HEIGHT + right * HIP_HALF_W,
    ];
    p.foot_up_dir = [up; 2];

    commands.spawn((
        p,
        RigidBody::Dynamic,
        Collider::capsule(CAPSULE_RADIUS, CAPSULE_LEN),
        Mass(80.0),
        // Balanced rotational inertia + angular damping so high-thrust afterburner never wobbles or spins out!
        AngularInertia::new(Vec3::new(8.5, 3.8, 8.5)),
        AngularDamping(0.5),
        (
            Friction::new(0.0).with_combine_rule(CoefficientCombine::Min),
            Restitution::new(0.0),
            LinearVelocity::default(),
            AngularVelocity::default(),
        ),
        SweptCcd::default(),
        // Render the body between fixed physics ticks so planted feet don't jitter against a stair-stepping pelvis.
        TransformInterpolation,
        Transform::from_translation(pos).with_rotation(rot),
        Rebased,
        Focus,
        Name::new("player"),
    ));
}

/// Probes ground beneath left/right hips (fixed hip offsets, eliminating foot-position feedback jitter!).
fn ground_probe(
    spatial: SpatialQuery,
    origin: Res<WorldOrigin>,
    mut q: Query<(Entity, &Transform, &mut Player)>,
) {
    for (e, t, mut p) in &mut q {
        let up = origin.up_at(t.translation);
        let filter = SpatialQueryFilter::from_excluded_entities([e]);
        let down = Dir3::new(-up).unwrap_or(Dir3::NEG_Y);
        let base_yaw = Quat::from_rotation_arc(Vec3::Y, up) * Quat::from_rotation_y(p.yaw);
        let right = base_yaw * Vec3::X;
        let probe_max = RIDE_HEIGHT + 0.55;

        let mut sum_dist = 0.0f32;
        let mut hit_count = 0usize;
        let mut avg_normal = Vec3::ZERO;

        for (i, side) in [-1.0f32, 1.0f32].into_iter().enumerate() {
            let ray_origin = t.translation + right * (side * HIP_HALF_W);
            match spatial.cast_ray(ray_origin, down, probe_max, true, &filter) {
                Some(hit) => {
                    p.foot_ground_hit[i] = Some(ray_origin - up * hit.distance);
                    sum_dist += hit.distance;
                    hit_count += 1;
                    avg_normal += hit.normal;
                }
                None => {
                    p.foot_ground_hit[i] = None;
                }
            }
        }

        if hit_count > 0 {
            let eff_dist = sum_dist / (hit_count as f32);
            p.ground_dist = eff_dist;
            p.ground_normal = avg_normal.normalize_or(up);
            p.grounded = eff_dist < RIDE_HEIGHT + 0.28;
        } else {
            p.ground_dist = f32::MAX;
            p.ground_normal = up;
            p.grounded = false;
        }

        p.agl = spatial
            .cast_ray(t.translation, down, 20_000.0, true, &filter)
            .map_or(p.agl.max(20_000.0), |h| h.distance - RIDE_HEIGHT);
    }
}

fn player_input(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mouse: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    settings: Res<ControlSettings>,
    pad: Res<crate::gamepad::Pad>,
    mut q: Query<&mut Player>,
) {
    let Ok(mut p) = q.single_mut() else { return };
    let k = 0.0025 * settings.mouse_sensitivity;
    let dy = if settings.invert_y { -mouse.delta.y } else { mouse.delta.y };
    p.cam_yaw -= mouse.delta.x * k;
    p.cam_pitch = (p.cam_pitch - dy * k).clamp(-1.48, 1.42);
    // Right stick: same direction and invert setting as the mouse, at a rate per second.
    let rate = crate::gamepad::PAD_LOOK_RATE * settings.mouse_sensitivity * time.delta_secs();
    let pad_dy = if settings.invert_y { -pad.right.y } else { pad.right.y };
    p.cam_yaw -= pad.right.x * rate;
    p.cam_pitch = (p.cam_pitch + pad_dy * rate).clamp(-1.48, 1.42);
    let notches = crate::mapview::scroll_notches(&scroll);
    if notches != 0.0 {
        p.cam_dist_goal = (p.cam_dist_goal * 0.92f32.powf(notches)).clamp(2.0, 60.0);
    }
    let k = 1.0 - (-5.0 * time.delta_secs()).exp();
    p.cam_dist = (p.cam_dist.ln() + (p.cam_dist_goal.ln() - p.cam_dist.ln()) * k).exp();
    if keys.just_pressed(KeyCode::KeyV) {
        p.first_person = !p.first_person;
    }
    let axis = |pos: KeyCode, neg: KeyCode| {
        keys.pressed(pos) as i32 as f32 - keys.pressed(neg) as i32 as f32
    };
    // Left stick adds to WASD; both are clamped so using both never overshoots.
    p.fwd_in = (axis(KeyCode::KeyW, KeyCode::KeyS) + pad.left.y).clamp(-1.0, 1.0);
    p.strafe = (axis(KeyCode::KeyD, KeyCode::KeyA) + pad.left.x).clamp(-1.0, 1.0);
    let (s, c) = p.cam_yaw.sin_cos();
    let fwd = Vec3::new(-s, 0.0, -c);
    let right = Vec3::new(c, 0.0, -s);
    p.wish = (fwd * p.fwd_in + right * p.strafe).normalize_or_zero();
    // Left trigger is Shift; right trigger is Space. Each pad button acts like its key.
    p.shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) || pad.lt_held;
    p.space_held = keys.pressed(KeyCode::Space) || pad.rt_held;
    p.rocket_amt = if keys.pressed(KeyCode::Space) { 1.0 } else { pad.rt };
    p.boost_amt = if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) { 1.0 } else { pad.lt };
    if keys.just_pressed(KeyCode::Space) || pad.rt_just {
        p.space_just_pressed = true;
    }
    if keys.just_released(KeyCode::Space) || pad.rt_released {
        p.space_just_released = true;
    }
    p.crouch = keys.pressed(KeyCode::KeyC);
    // A: wingsuit (F). B: parachute (P).
    if keys.just_pressed(KeyCode::KeyF) || pad.a_just {
        p.wingsuit_on = !p.wingsuit_on;
    }
    if keys.just_pressed(KeyCode::KeyP) || pad.b_just {
        p.mode = match p.mode {
            MoveMode::Skydive | MoveMode::Fall => MoveMode::Parachute,
            MoveMode::Parachute => MoveMode::Skydive,
            m => m,
        };
    }
    if keys.just_pressed(KeyCode::KeyG) {
        p.jump = true;
    }
}

/// Camera pitch that counts as "level" for flight aiming (the default chase view looks slightly down).
const CAM_LEVEL_PITCH: f32 = -0.25;

/// Computes shortest-arc rotation error vector in world space from `current` to `target`.
pub fn rotation_error_vec(current: Quat, target: Quat) -> Vec3 {
    let mut dq = target * current.inverse();
    if dq.w < 0.0 {
        dq = -dq;
    }
    let (axis, angle) = dq.to_axis_angle();
    if angle.is_finite() { axis * angle } else { Vec3::ZERO }
}

#[allow(clippy::type_complexity)]
fn drive_player(
    time: Res<Time>,
    origin: Res<WorldOrigin>,
    mut booms: MessageWriter<SonicBoom>,
    mut q: Query<(&Transform, &mut Player, Forces)>,
) {
    let dt = time.delta_secs().clamp(1e-4, 0.05);
    for (t, mut p, mut forces) in &mut q {
        let up = origin.up_at(t.translation);
        let mass = 80.0f32;
        let g = 9.81f32;
        let weight = mass * g;

        let v = forces.linear_velocity();
        let ang_v = forces.angular_velocity();
        let v_up_scalar = v.dot(up);
        let v_up = up * v_up_scalar;
        let v_flat = v - v_up;
        let speed_flat = v_flat.length();
        let speed_total = v.length();
        let alt = origin.to_geodetic(t.translation).alt_m;
        let air = atmosphere(alt);
        let rho = air.density as f32;

        let raw_accel = (v - p.prev_vel) / dt;
        let prev_v_up = p.prev_vel.dot(up);
        p.prev_vel = v;
        p.accel_smooth = p.accel_smooth.lerp(raw_accel.clamp_length_max(150.0), 1.0 - (-14.0 * dt).exp());

        let base_globe = Quat::from_rotation_arc(Vec3::Y, up);
        let wish_world = base_globe * p.wish;
        let has_dir_input = p.fwd_in.abs() > 0.05 || p.strafe.abs() > 0.05;

        // Physical test nudge (`G` key):
        if p.jump {
            p.jump = false;
            let (s, c) = p.cam_yaw.sin_cos();
            let nudge_dir = if p.wish.length_squared() > 0.01 {
                wish_world
            } else {
                base_globe * Vec3::new(-s, 0.0, -c)
            };
            let chest_pt = t.translation + (t.rotation * Vec3::Y) * 0.35;
            forces.apply_linear_impulse_at_point(nudge_dir * (mass * 3.2), chest_pt);
        }

        // Hysteresis: easy to stay grounded, harder to become grounded, so the controller never
        // flickers between ground/air modes right at the threshold.
        let near_ground = if p.was_grounded {
            p.ground_dist < RIDE_HEIGHT + 0.45 && v_up_scalar < 4.0
        } else {
            p.ground_dist < RIDE_HEIGHT + 0.18 && v_up_scalar < 2.0
        };
        let grounded = near_ground;

        // --- Landing Impact & Recovery State ---
        // When touching down from a fall or fast flight, absorb the landing in a deep crouch/slide
        // and wait until horizontal speed has nearly stopped before allowing the player to get up and run!
        if grounded && !p.was_grounded {
            let impact_speed = (-prev_v_up).max(0.0);
            if impact_speed > 5.0 || speed_flat > 6.5 {
                p.landing_recovery = ((impact_speed - 3.5) * 0.12 + speed_flat * 0.06).clamp(0.45, 1.8);
            }
        }
        p.was_grounded = grounded;

        if p.landing_recovery > 0.0 {
            if !grounded {
                p.landing_recovery = 0.0;
            } else if speed_flat > 1.8 {
                // Keep recovering as long as we're still sliding/moving fast from the landing!
                p.landing_recovery = (p.landing_recovery - dt * 0.35).max(0.25);
            } else {
                // Once nearly stopped (`speed_flat <= 1.8`), count down quickly and get up when W is pressed!
                let get_up_rate = if has_dir_input { 2.2 } else { 1.2 };
                p.landing_recovery = (p.landing_recovery - dt * get_up_rate).max(0.0);
            }
        }

        // --- Space Jump Charge vs. Mid-Air Space Rocket Logic ---
        if grounded {
            p.space_air_latched = false;
            if p.space_held && p.jump_push_timer <= 0.0 && p.landing_recovery <= 0.2 {
                p.jump_charge = (p.jump_charge + dt * 3.8).min(1.0);
                if p.jump_charge >= 0.95 {
                    p.jump_push_timer = 0.17;
                }
            } else if p.space_just_released && p.jump_charge > 0.12 && p.jump_push_timer <= 0.0 {
                p.jump_push_timer = 0.14;
            } else if !p.space_held && p.jump_push_timer <= 0.0 {
                p.jump_charge = (p.jump_charge - dt * 6.0).max(0.0);
            }
        } else {
            p.jump_charge = 0.0;
            // In the air: pressing Space (or holding Space after the jump peak) engages the Space rocket controller!
            if p.space_just_pressed || (p.space_held && v_up_scalar < 1.0 && p.jump_push_timer <= 0.0) {
                p.space_air_latched = true;
            }
            if !p.space_held {
                p.space_air_latched = false;
            }
        }
        p.space_just_pressed = false;
        p.space_just_released = false;

        // --- Rocket Activation ---
        let space_rocket = !grounded && p.space_air_latched;
        // Shift is sprint on the ground and only becomes the afterburner once airborne.
        p.afterburner = p.shift && !grounded;
        p.sprint = p.shift && grounded;
        p.thrust_held = p.afterburner || space_rocket;

        // Smooth ramp-up when holding Shift so thrust builds progressively over 2.5-4 seconds instead of jerking!
        if p.afterburner {
            p.boost_time += dt;
        } else {
            p.boost_time = (p.boost_time - dt * 2.5).max(0.0);
        }
        p.boost_stage = if p.afterburner {
            (1 + (p.boost_time / 4.0) as usize).min(3)
        } else {
            0
        };

        // Mode transitions:
        // Wingsuit on and already flying: the rockets just add thrust to the suit, no mode change.
        let winged_boost = p.thrust_held
            && p.wingsuit_on
            && !grounded
            && (speed_total > 12.0 || (p.mode == MoveMode::Skydive && speed_total > 6.0));
        let wants_glide = !grounded && (!p.thrust_held || winged_boost) && (speed_flat > 10.0 || (p.agl > 8.0 && v_up_scalar < -8.0));
        p.mode = match p.mode {
            _ if winged_boost => MoveMode::Skydive,
            _ if p.thrust_held => MoveMode::RocketShoes,
            _ if grounded => MoveMode::Walk,
            MoveMode::Walk if !grounded => {
                if wants_glide { MoveMode::Skydive } else { MoveMode::Fall }
            }
            MoveMode::RocketShoes if !p.thrust_held => {
                if grounded {
                    MoveMode::Walk
                } else if wants_glide {
                    MoveMode::Skydive
                } else {
                    MoveMode::Fall
                }
            }
            MoveMode::Fall if wants_glide => MoveMode::Skydive,
            MoveMode::Skydive if !wants_glide && p.agl < 20.0 && speed_flat < 5.0 => MoveMode::Fall,
            m => m,
        };

        // --- Plane-style stick. Only the mouse moves the view; W/S/A/D tilt the body's aim
        // relative to it (see the rocket and wingsuit sections), never the camera. ---
        let wingsuit = p.wingsuit_on && matches!(p.mode, MoveMode::Skydive | MoveMode::Fall) && (!p.thrust_held || winged_boost) && !grounded;
        // Suit off: a plain skydiver. W somersaults, A/D barrel roll, S spreads eagle.
        let freefall = !p.wingsuit_on && p.mode == MoveMode::Skydive && !p.thrust_held && !grounded;
        {
            let (tuck, spread) = if freefall {
                let spin = p.fwd_in.max(0.0).max(p.strafe.abs() * 0.5);
                (spin, (-p.fwd_in).max(0.0))
            } else {
                (0.0, 0.0)
            };
            let k = 1.0 - (-6.0 * dt).exp();
            p.fall_tuck += (tuck - p.fall_tuck) * k;
            p.fall_spread += (spread - p.fall_spread) * k;
        }

        // --- Facing Yaw ---
        let prev_yaw = p.yaw;
        let target_yaw = match p.mode {
            MoveMode::Walk => {
                if p.landing_recovery <= 0.1 && p.wish.length_squared() > 0.01 {
                    (-p.wish.x).atan2(-p.wish.z)
                } else {
                    p.yaw
                }
            }
            MoveMode::RocketShoes | MoveMode::Skydive => p.cam_yaw,
            _ => {
                if has_dir_input {
                    p.cam_yaw
                } else {
                    p.yaw
                }
            }
        };
        let dyaw = ((target_yaw - p.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)) - std::f32::consts::PI;
        let yaw_rate = match p.mode {
            MoveMode::Walk => 6.5,
            MoveMode::RocketShoes => 6.0,
            MoveMode::Skydive => 4.0,
            _ => 3.5,
        };
        p.yaw += dyaw * (1.0 - (-yaw_rate * dt).exp());
        let yaw_vel = (p.yaw - prev_yaw) / dt;

        let yaw_frame = base_globe * Quat::from_rotation_y(p.yaw);
        let _fwd_world = yaw_frame * Vec3::NEG_Z;
        let right_world = yaw_frame * Vec3::X;

        // =========================================================================
        // 1. PURE AERODYNAMIC DRAG & WINGSUIT LIFT ON THE BODY
        // =========================================================================
        let q_dyn = 0.5 * rho * speed_total * speed_total;
        let wind_dir = (-v).normalize_or(up);
        let body_head = t.rotation * Vec3::Y;

        // Real drag only: a streamlined body (head/feet along the wind) has a tiny
        // cross-section; tumbling or lying belly-to-wind exposes the full ~0.7 m^2.
        let axial = body_head.dot(wind_dir).abs().clamp(0.0, 1.0);
        let broadside_sq = 1.0 - axial * axial;
        // The inflated suit carries its own drag (flyer::wingsuit); the bare-body drag fades out as it opens.
        let wing_open = if wingsuit { ((speed_total - 4.0) / 10.0).clamp(0.0, 1.0) } else { 0.0 };
        if p.mode != MoveMode::Parachute {
            // Balled up presents less area; spread eagle more.
            let pose_area = 1.0 + 0.4 * p.fall_spread - 0.35 * p.fall_tuck;
            let cd_area = (0.04 + 0.62 * broadside_sq) * pose_area;
            forces.apply_force(wind_dir * (q_dyn * cd_area * (1.0 - wing_open)));
        } else {
            let k = 0.5 * rho * 28.0;
            let drag_v = -v_up * (k * v_up.length() * 1.5);
            let glide = (wish_world * 10.0 - v_flat) * (mass * 1.5);
            forces.apply_force(drag_v + glide);
        }

        // Wingsuit: the flyer only reshapes the suit (arch for S/W and looking up/down, twist for
        // A/D and for banking toward the view); lift, drag and every moment come from the air.
        let mut wing_shake = 0.0f32;
        if wingsuit {
            let (s, c) = p.cam_yaw.sin_cos();
            let cmd = flyer::WingCmd {
                pitch: -p.fwd_in,
                roll: p.strafe,
                heading: if p.strafe.abs() > 0.05 { Vec3::ZERO } else { base_globe * Vec3::new(-s, 0.0, -c) },
                arch_bias: ((p.cam_pitch - CAM_LEVEL_PITCH) * 1.2).clamp(-0.6, 0.6),
            };
            let want = flyer::wing_pilot(t.rotation, v, up, ang_v, cmd);
            // Limbs move at a finite rate: the suit responds loosely, not instantly.
            let k = 1.0 - (-10.0 * dt).exp();
            p.wing_arch += (want.arch - p.wing_arch) * k;
            let twist = -p.wing_turn + (want.twist + p.wing_turn) * k;
            p.wing_turn = -twist;
            let pose = flyer::WingPose { arch: p.wing_arch, twist, open: wing_open };
            let aero = flyer::wingsuit(t.rotation, v, rho, ang_v, flyer::BODY_INERTIA, pose);
            forces.apply_force(aero.force);
            forces.apply_torque(aero.torque);
            wing_shake = 0.12 * (speed_total / 70.0).clamp(0.0, 1.0) * wing_open;
        } else {
            let k = (1.0 - 4.0 * dt).max(0.0);
            p.wing_turn *= k;
            p.wing_arch *= k;
        }

        // =========================================================================
        // 2. GROUND STANDING, LANDING RECOVERY, STEPPING & JUMP PUSH
        // =========================================================================
        if grounded || p.jump_push_timer > 0.0 {
            let landing_crouch = (p.landing_recovery * 0.35).min(0.32);
            let crouch_depth = if p.crouch {
                0.30
            } else if p.jump_charge > 0.0 && p.jump_push_timer <= 0.0 {
                p.jump_charge * 0.34
            } else {
                landing_crouch
            };
            let target_leg_h = RIDE_HEIGHT - crouch_depth;

            let is_jumping = p.jump_push_timer > 0.0;
            if is_jumping {
                p.jump_push_timer = (p.jump_push_timer - dt).max(0.0);
                if p.jump_push_timer <= 0.0 {
                    p.jump_charge = 0.0;
                }
            }

            // Vertical leg support force:
            // Both legs share the vertical support smoothly across `foot_ground_hit` so stepping one foot
            // never drops or spikes the spring stiffness!
            let h_err = target_leg_h - p.ground_dist;
            let total_f_up = if is_jumping {
                let charge_scale = 0.75 + 0.45 * p.jump_charge.max(0.5);
                weight * 3.8 * charge_scale
            } else {
                (weight + h_err * 1850.0 - v_up_scalar * 780.0).clamp(0.0, weight * 14.0)
            };

            // Horizontal ground force:
            // - If in `landing_recovery`, apply strong braking friction until the robot has basically stopped!
            // - Only allow walking/running propulsion once `landing_recovery <= 0.1` and the player presses WASD!
            let f_horiz_total = if p.landing_recovery > 0.1 {
                (-v_flat * (mass * 7.5)).clamp_length_max(weight * 0.8)
            } else if has_dir_input {
                let max_walk_speed = if p.crouch || p.jump_charge > 0.1 { 1.0 } else if p.sprint { 6.0 } else { 2.2 };
                let target_v_flat = wish_world * max_walk_speed;
                let v_err_flat = target_v_flat - v_flat;
                (v_err_flat * (mass * 8.5)).clamp_length_max(weight * if p.sprint { 0.6 } else { 0.35 })
            } else {
                // Standing still: grip the ground so we never slide down slopes.
                (-v_flat * (mass * 11.0)).clamp_length_max(weight * 0.9)
            };

            // Distribute through the two feet with a gentle weight-shift lever arm:
            // Vertical load is shared; horizontal (walking) force only goes through planted feet,
            // so propulsion comes from a foot pushing off the ground, not from a swing leg.
            let n_planted = p.foot_planted.iter().filter(|&&b| b).count();
            for i in 0..2 {
                let share = match (p.foot_planted[0], p.foot_planted[1]) {
                    (true, false) => if i == 0 { 0.75 } else { 0.25 },
                    (false, true) => if i == 1 { 0.75 } else { 0.25 },
                    _ => 0.5,
                };
                let share_h = match n_planted {
                    0 => 0.5,
                    n if p.foot_planted[i] => 1.0 / n as f32,
                    _ => 0.0,
                };
                let foot_force = up * total_f_up * share + f_horiz_total * share_h;
                let app_point = t.translation;
                forces.apply_force_at_point(foot_force, app_point);
            }
        }
        // Rockets off (including the frame we touch down) means no flame, whatever branch ran above.
        if !p.thrust_held {
            p.throttle = 0.0;
        }

        // =========================================================================
        // 3. ROCKET BOOTS & 6-CASE FLIGHT CONTROL MATRIX
        // =========================================================================
        let cam_rot = base_globe * Quat::from_euler(EulerRot::YXZ, p.cam_yaw, p.cam_pitch, 0.0);
        let cam_fwd_3d = cam_rot * Vec3::NEG_Z;
        let cam_right_3d = cam_rot * Vec3::X;
        let cam_up_3d = cam_rot * Vec3::Y;
        let cmd_dir_3d = (cam_fwd_3d * p.fwd_in + cam_right_3d * p.strafe).normalize_or_zero();

        p.intent_dir = cmd_dir_3d;
        let target_rot: Quat;
        // Weight of the posture "muscles" PD below. Rockets steer only through the soles and the
        // wingsuit only through the air, so for them it fades out.
        let mut muscle = 1.0f32;

        if winged_boost {
            // (thrust only; orientation falls through to the normal wingsuit code below)
            // Winged: same boost strength, but it just pushes along the head; the suit and the
            // normal wingsuit posture stay exactly as they are.
            let ramp = flyer::boost_ramp(p.boost_time, 10.0);
            let (rk, bo) = (p.rocket_amt.clamp(0.0, 1.0), p.boost_amt.clamp(0.0, 1.0));
            let thrust_acc = if p.afterburner { g * (1.5 * rk + (36.0 - 1.5) * ramp * bo) } else { g * 1.5 * rk };
            p.throttle = if p.afterburner { (0.65 * rk + ramp * 2.5 * bo + 0.35 * bo).max(0.1) } else { 0.65 * rk };
            forces.apply_force(t.rotation * Vec3::Y * (mass * thrust_acc));
        }
        if p.thrust_held && !winged_boost {
            // Space = gentle boost (1.5 g: hovers with room to climb, ~1.1 g along a level aim).
            // Shift = extra boost, ramping slowly from there to 36 g over 10 s.
            const SPACE_G: f32 = 1.5;
            let ramp = flyer::boost_ramp(p.boost_time, 10.0);
            // Trigger travel scales both: a light RT is a gentle push, a light LT a small boost.
            let (rk, bo) = (p.rocket_amt.clamp(0.0, 1.0), p.boost_amt.clamp(0.0, 1.0));
            let thrust_acc = if p.afterburner { g * (SPACE_G * rk + (36.0 - SPACE_G) * ramp * bo) } else { g * SPACE_G * rk };
            p.throttle = if p.afterburner { (0.65 * rk + ramp * 2.5 * bo + 0.35 * bo).max(0.1) } else { 0.65 * rk };

            // Aim = the view (offset so the default slightly-down camera means level flight),
            // tilted by the held keys: S noses up, W down, A/D swing left/right.
            let aim_pitch = (p.cam_pitch - CAM_LEVEL_PITCH - p.fwd_in * 0.6).clamp(-1.5, 1.5);
            let aim_yaw = p.cam_yaw - p.strafe * 0.4;
            let aim = base_globe * Quat::from_euler(EulerRot::YXZ, aim_yaw, aim_pitch, 0.0) * Vec3::NEG_Z;
            // Point the boots so thrust + gravity accelerates along the aim (no altitude lost in turns).
            let axis = flyer::rocket_axis(aim, up, thrust_acc, g);
            // Bank: D rolls right, A left, plus a lean into whatever turn the view is making.
            let roll = (p.strafe * 0.55 - yaw_vel * 0.2).clamp(-0.8, 0.8);
            target_rot = flyer::rocket_attitude(axis, cam_up_3d, up, roll);

            // The boots throttle back while the body is still swinging round (never blast the
            // wrong way), but keep a floor: the thrust is also what turns us.
            let gate = (((t.rotation * Vec3::Y).dot(axis) - 0.3) / 0.5).clamp(0.25, 1.0);
            let thrust_n = mass * thrust_acc * gate;
            // A loose spring says how the body would like to turn; the legs swing the feet so the
            // thrust line passes beside the centre of mass and makes that torque (r x F).
            let want = flyer::rocket_want_torque(t.rotation, target_rot, ang_v);
            let steer = flyer::boot_steer(t.rotation, thrust_n, want);
            let leg = p.leg_steer;
            p.leg_steer = leg + (steer.leg_offset - leg) * (1.0 - (-15.0 * dt).exp());
            for (pt, f) in flyer::boot_forces(t.rotation, t.translation, thrust_n, p.leg_steer, steer.roll_push) {
                forces.apply_force_at_point(f, pt);
            }
            muscle = 0.0;
        } else {
            // =====================================================================
            // 4. NON-ROCKET ORIENTATION (WALK, FALL, SKYDIVE, PARACHUTE)
            // =====================================================================
            match p.mode {
                MoveMode::Walk => {
                    let brake_lean = if !has_dir_input || p.landing_recovery > 0.1 {
                        -v_flat * 0.045
                    } else {
                        Vec3::ZERO
                    };
                    let turn_bank = right_world * (-yaw_vel * speed_flat * 0.04 + p.strafe * 0.20);
                    let wish_lean = if p.landing_recovery > 0.1 {
                        brake_lean
                    } else {
                        wish_world * 0.14 + brake_lean + turn_bank
                    };

                    let balanced_up = (up + wish_lean.clamp_length_max(0.48)).normalize_or(up);
                    target_rot = Quat::from_rotation_arc(up, balanced_up) * yaw_frame;
                }

                MoveMode::Skydive => {
                    let base_dive_pitch = -std::f32::consts::FRAC_PI_2 * 0.88;
                    let cam_pitch_influence = p.cam_pitch.clamp(-1.1, 0.6) * 0.42;
                    let ws_pitch_offset = -p.fwd_in * 0.42;
                    let total_pitch = (base_dive_pitch + cam_pitch_influence + ws_pitch_offset).clamp(-1.50, -0.30);
                    let pitch_pose = yaw_frame * Quat::from_rotation_x(total_pitch);

                    // Real wingsuit posture: head along the flight path with a small angle of attack,
                    // belly toward the ground, banking into turns. Blends in as horizontal speed builds.
                    let dive = p.fwd_in.max(0.0);
                    let flare = (-p.fwd_in).max(0.0);
                    let h = (v.normalize_or(up) + up * (0.18 + 0.4 * flare - 0.45 * dive)).normalize_or(up);
                    let back = (up - h * up.dot(h)).normalize_or(yaw_frame * Vec3::Z);
                    let bank = (-p.wing_turn * 0.95 + (-yaw_vel * 0.15).clamp(-0.5, 0.5)).clamp(-1.2, 1.2);
                    let flight = Quat::from_mat3(&Mat3::from_cols(h.cross(back), h, back)) * Quat::from_rotation_y(bank);
                    let blend = ((speed_flat - 6.0) / 14.0).clamp(0.0, 1.0);
                    target_rot = pitch_pose.slerp(flight, blend);
                }

                MoveMode::Fall => {
                    // In the air with no Space/Shift: stays upright by default, and WASD tilts the body!
                    // - W tilts forward, S tilts backward, A/D tilts sideways.
                    let pitch_in = p.fwd_in * 0.55;
                    let roll_in = -p.strafe * 0.48;
                    target_rot = yaw_frame * Quat::from_euler(EulerRot::YXZ, 0.0, -pitch_in, roll_in);
                }

                MoveMode::Parachute | MoveMode::RocketShoes => {
                    let swing_up = (up - p.accel_smooth * 0.03 + right_world * (-p.strafe * 0.2)).normalize_or(up);
                    target_rot = Quat::from_rotation_arc(up, swing_up) * yaw_frame;
                }
            }
        }

        if wingsuit {
            // Only while the suit is still collapsed does the flyer hold a posture by muscle.
            muscle = (1.0 - wing_open) * (1.0 - wing_open);
        }
        if freefall {
            // Spins and the box position come from limbs in the airflow (flyer::freefall_torque),
            // not from a posture spring. Belly down, head toward the view.
            let belly_down = yaw_frame * Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
            let cmd = flyer::FallCmd { flip: p.fwd_in.max(0.0), roll: p.strafe, spread: p.fall_spread };
            forces.apply_torque(flyer::freefall_torque(t.rotation, ang_v, speed_total, belly_down, cmd));
            muscle = 0.0;
        }
        if !p.thrust_held || winged_boost {
            p.leg_steer *= (1.0 - 6.0 * dt).max(0.0);
        }
        let rot_err = rotation_error_vec(t.rotation, target_rot);
        p.rot_err = p.rot_err.lerp(rot_err, 1.0 - (-18.0 * dt).exp());
        // Critically damped PD sized to the body's inertia (I ~ 8.5): natural frequency w,
        // kp = I*w^2, kd = 2*I*w.
        let w = if grounded { 7.0 } else { 6.0 };
        let (kp_rot, kd_rot) = (8.5 * w * w, 2.0 * 8.5 * w);
        let mut torque = (rot_err * kp_rot - ang_v * kd_rot) * muscle;
        // Hard guard against runaway spin from a bad contact: extra damping above 9 rad/s.
        let spin_excess = (ang_v.length() - 14.0).max(0.0);
        torque -= ang_v.normalize_or_zero() * spin_excess * 8.5 * 30.0;
        let torque = torque.clamp_length_max(1800.0);
        forces.apply_torque(torque);

        if let Some(MachEvent::BrokeBarrier) = p.mach.update(speed_total as f64, alt) {
            booms.write(SonicBoom(t.translation));
            p.shake = 1.0;
        }
        p.shake = (p.shake - dt * 1.5).max(wing_shake);
    }
}

#[allow(clippy::type_complexity)]
fn camera_follow(
    time: Res<Time>,
    origin: Res<WorldOrigin>,
    spatial: SpatialQuery,
    mut rig: ResMut<CamRig>,
    mut players: Query<(Entity, &Transform, &mut Player, &LinearVelocity), Without<Camera3d>>,
    mut cams: Query<(&mut Transform, &mut Projection), (With<Camera3d>, Without<crate::globe::GlobeCamera>)>,
    driving: Option<Res<crate::vehicles::Driving>>,
) {
    if driving.is_some_and(|d| d.0.is_some()) {
        return;
    }
    let Ok((pe, pt, mut p, vel)) = players.single_mut() else { return };
    let Ok((mut ct, mut proj)) = cams.single_mut() else { return };
    // Real frame time (no 50 ms clamp): the exact solvers below stay stable, and a clamp would make the rig run slow-motion at low fps.
    let dt = time.delta_secs().clamp(1e-4, 0.25);
    let up = origin.up_at(pt.translation);
    let base = Quat::from_rotation_arc(Vec3::Y, up);
    // Mouse look is applied to cam_yaw the same frame (never smoothed); only the character's own turning trails.
    p.view_lag *= (-dt / 0.8).exp();
    let look = base * Quat::from_euler(EulerRot::YXZ, p.cam_yaw + p.view_lag, p.cam_pitch, 0.0);

    let speed = vel.0.length();
    let fov = (62.0 + (speed / 24.0).min(26.0)).to_radians();
    if let Projection::Perspective(pp) = &mut *proj {
        pp.fov += (fov - pp.fov) * (1.0 - (-4.0 * dt).exp());
    }
    let shake = if p.shake > 0.0 {
        let t = time.elapsed_secs() * 47.0;
        Vec3::new(t.sin(), (t * 1.3).cos(), (t * 0.7).sin()) * p.shake * 0.25
    } else {
        Vec3::ZERO
    };

    let body_up = pt.rotation * Vec3::Y;
    if p.first_person {
        let head = pt.translation + body_up * 0.65;
        ct.translation = head + shake;
        ct.rotation = look;
        rig.focus_pos = head;
        rig.focus_vel = vel.0;
        return;
    }

    // Locked to the body: no velocity lead (it put the pivot metres ahead of a fast flyer).
    let target_anchor = pt.translation + up * 0.55 + body_up * 0.20;
    // Lag is bounded by speed, so only a genuine teleport (spawn, re-anchor) snaps the rig.
    let snap_dist = 25.0 + vel.0.length() * dt * 2.0;
    if !rig.init || (target_anchor - rig.focus_pos).length() > snap_dist {
        rig.focus_pos = target_anchor;
        rig.focus_vel = vel.0;
        rig.init = true;
    } else {
        // Critically damped spring on the offset *relative to the moving anchor*, solved exactly:
        // unconditionally stable at any dt or speed, so no overshoot, jitter or blow-up when
        // frames are slow or the player is fast.
        let x = rig.focus_pos - target_anchor;
        let u = rig.focus_vel - vel.0;
        let x_up = up * x.dot(up);
        let u_up = up * u.dot(up);
        let solve = |x: Vec3, u: Vec3, w: f32| {
            let e = (-w * dt).exp();
            let tmp = (u + x * w) * dt;
            ((x + tmp) * e, (u - tmp * w) * e)
        };
        // Only the vertical offset is eased, and only on the ground (steps, landings). Sideways,
        // and everything in the air, is rigid: the character never slides out of frame.
        let grounded = matches!(p.mode, MoveMode::Walk);
        let (xv, uv) = if grounded { solve(x_up, u_up, 11.0) } else { (Vec3::ZERO, Vec3::ZERO) };
        rig.focus_pos = target_anchor + xv;
        rig.focus_vel = vel.0 + uv;
    }

    let pivot = rig.focus_pos;
    let back = look * Vec3::Z;
    let filter = SpatialQueryFilter::from_excluded_entities([pe]);
    let dist = Dir3::new(back)
        .ok()
        .and_then(|d| spatial.cast_ray(pivot, d, p.cam_dist, true, &filter))
        .map_or(p.cam_dist, |h| (h.distance - 0.3).max(0.5));
    ct.translation = pivot + back * dist + shake;
    ct.rotation = look;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_error_zero_when_aligned() {
        let q = Quat::from_euler(EulerRot::YXZ, 0.7, -0.4, 0.2);
        let err = rotation_error_vec(q, q);
        assert!(err.length() < 1e-5);
        let err_neg = rotation_error_vec(q, -q);
        assert!(err_neg.length() < 1e-5);
    }

    #[test]
    fn rotation_error_points_along_axis() {
        let q0 = Quat::IDENTITY;
        let q1 = Quat::from_rotation_x(0.5);
        let err = rotation_error_vec(q0, q1);
        assert!((err.x - 0.5).abs() < 1e-4);
        assert!(err.y.abs() < 1e-4 && err.z.abs() < 1e-4);
    }
}
