//! Camera rig. Stage 1: free-fly spectator (WASD + mouse, Shift = fast,
//! scroll = speed). The player/vehicle rigs (first + third person) attach
//! to this same camera entity.

use crate::config::Config;
use crate::origin::{Focus, Rebased, WorldOrigin};
use bevy::anti_alias::taa::TemporalAntiAliasing;
use bevy::camera::Exposure;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::light::AtmosphereEnvironmentMapLight;
use bevy::pbr::AtmosphereSettings;
use bevy::pbr::ScreenSpaceAmbientOcclusion;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::camera::Hdr;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_camera).add_systems(Update, fly_camera.run_if(|cfg: Res<Config>| cfg.fly && cfg.lookdev.is_none()));
    }
}

#[derive(Component)]
pub struct FlyCam {
    pub yaw: f32,
    pub pitch: f32,
    pub speed: f32,
}

fn spawn_camera(mut commands: Commands, cfg: Res<Config>, origin: Res<WorldOrigin>) {
    // Start above the drop point; streaming refines the ground under us.
    let ground = cfg.start.alt_m.max(0.0);
    let pos = origin.to_render(ed_geo::Geodetic { alt_m: ground + cfg.start_agl, ..cfg.start });
    let yaw = (-cfg.heading_deg).to_radians();
    let pitch = cfg.pitch_deg.to_radians();
    let mut cam = commands.spawn((
        Camera3d::default(),
        Hdr,
        Projection::Perspective(PerspectiveProjection { fov: 60f32.to_radians(), near: 0.1, far: 1.0e13, ..default() }),
        Tonemapping::AgX,
        Bloom::NATURAL,
        Exposure { ev100: 14.2 },
        Msaa::Off,
        TemporalAntiAliasing::default(),
        ScreenSpaceAmbientOcclusion::default(),
        Transform::from_translation(pos).with_rotation(Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0)),
        FlyCam { yaw, pitch, speed: 60.0 },
        Rebased,
    ));
    // The camera drives streaming in spectator/lookdev modes; otherwise the player does.
    if cfg.fly || cfg.lookdev.is_some() {
        cam.insert(Focus);
    }
    if cfg.atmosphere {
        cam.insert((AtmosphereSettings::default(), AtmosphereEnvironmentMapLight::default()));
    } else {
        cam.insert(DistanceFog {
            color: Color::srgb(0.7, 0.8, 0.92),
            falloff: FogFalloff::from_visibility(60_000.0),
            ..default()
        });
        commands.insert_resource(ClearColor(Color::srgb(0.55, 0.72, 0.92)));
        commands.insert_resource(GlobalAmbientLight { brightness: 8_000.0, ..default() });
    }
}

fn fly_camera(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    buttons: Res<ButtonInput<MouseButton>>,
    origin: Res<WorldOrigin>,
    mut q: Query<(&mut Transform, &mut FlyCam)>,
) {
    let Ok((mut t, mut cam)) = q.single_mut() else { return };
    let dt = time.delta_secs();
    if buttons.pressed(MouseButton::Right) || buttons.pressed(MouseButton::Left) {
        cam.yaw -= mouse.delta.x * 0.003;
        cam.pitch = (cam.pitch - mouse.delta.y * 0.003).clamp(-1.55, 1.55);
    }
    if scroll.delta.y != 0.0 {
        cam.speed = (cam.speed * 1.2f32.powf(scroll.delta.y)).clamp(2.0, 2.0e6);
    }
    // Keep "up" as local up so flying around the globe stays level.
    let up = origin.up_at(t.translation);
    let base = Quat::from_rotation_arc(Vec3::Y, up);
    t.rotation = base * Quat::from_euler(EulerRot::YXZ, cam.yaw, cam.pitch, 0.0);
    let mut dir = Vec3::ZERO;
    let fwd = t.forward().as_vec3();
    let right = t.right().as_vec3();
    if keys.pressed(KeyCode::KeyW) {
        dir += fwd;
    }
    if keys.pressed(KeyCode::KeyS) {
        dir -= fwd;
    }
    if keys.pressed(KeyCode::KeyD) {
        dir += right;
    }
    if keys.pressed(KeyCode::KeyA) {
        dir -= right;
    }
    if keys.pressed(KeyCode::KeyE) {
        dir += up;
    }
    if keys.pressed(KeyCode::KeyQ) {
        dir -= up;
    }
    let boost = if keys.pressed(KeyCode::ShiftLeft) { 10.0 } else { 1.0 };
    t.translation += dir.normalize_or_zero() * cam.speed * boost * dt;
}
