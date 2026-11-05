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
        app.add_systems(Startup, spawn_camera).add_systems(Update, atmosphere_by_altitude).add_systems(Update, fly_camera.run_if(|cfg: Res<Config>| cfg.fly && cfg.lookdev.is_none()));
    }
}

#[derive(Component)]
pub struct FlyCam {
    pub yaw: f32,
    pub pitch: f32,
    pub speed: f32,
}

/// Lookup-texture atmosphere is exact and cheap near the ground, but its
/// aerial-perspective volume only spans 32 km. From altitude we switch to the
/// ray-marched model, which integrates the true path through the shell, so the
/// limb glow, blue haze over land and ocean extinction appear from orbit.
fn atmosphere_by_altitude(
    origin: Res<WorldOrigin>,
    mut cams: Query<(&GlobalTransform, &mut AtmosphereSettings), (With<Camera3d>, Without<crate::globe::GlobeCamera>)>,
) {
    for (gt, mut s) in &mut cams {
        let alt = origin.to_geodetic(gt.translation()).alt_m;
        let high = alt > 15_000.0;
        let mode = if high { bevy::pbr::AtmosphereMode::Raymarched } else { bevy::pbr::AtmosphereMode::LookupTexture };
        if s.rendering_method as u32 != mode as u32 {
            s.rendering_method = mode;
            s.sky_max_samples = if high { 48 } else { 16 };
        }
    }
}

fn spawn_camera(mut commands: Commands, cfg: Res<Config>, origin: Res<WorldOrigin>, mut images: ResMut<Assets<Image>>) {
    // Start above the drop point; streaming refines the ground under us.
    let ground = cfg.start.alt_m.max(0.0);
    let pos = origin.to_render(ed_geo::Geodetic { alt_m: ground + cfg.start_agl, ..cfg.start });
    let yaw = (-cfg.heading_deg).to_radians();
    let pitch = cfg.pitch_deg.to_radians();
    let mut cam = commands.spawn((
        Camera3d::default(),
        Hdr,
        Projection::Perspective(PerspectiveProjection { fov: 60f32.to_radians(), near: 0.1, far: 1.0e13, ..default() }),
        Tonemapping::BlenderFilmic,
        Exposure { ev100: std::env::var("EW_EV100").ok().and_then(|v| v.parse().ok()).unwrap_or(13.2) },
        Msaa::Off,
        Transform::from_translation(pos).with_rotation(Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0)),
        FlyCam { yaw, pitch, speed: 60.0 },
        Rebased,
        crate::cloud_half::CloudView,
        // Layer 0 plus the camera-only layer (meshes the sun must not see).
        bevy::camera::visibility::RenderLayers::from_layers(&[0, CAMERA_ONLY_LAYER]),
    ));
    if cfg.headless {
        let target = crate::headless::make_target(&mut images, cfg.width, cfg.height);
        cam.insert(bevy::camera::RenderTarget::Image(target.clone().into()));
        cam.commands().insert_resource(crate::headless::HeadlessTarget(target));
    }
    if !cfg.no_post {
        // `EW_POST=bloom,taa,ssao` (any subset) to bisect post effects; default all.
        let only = std::env::var("EW_POST").ok();
        let on = |k: &str| only.as_deref().map_or(true, |v| v.split(',').any(|x| x == k));
        if on("bloom") { cam.insert(Bloom::NATURAL); }
        if on("taa") { cam.insert(TemporalAntiAliasing::default()); }
        if on("ssao") { cam.insert(ScreenSpaceAmbientOcclusion::default()); }
        if only.as_deref().is_some_and(|v| v.contains("mv")) { cam.insert((bevy::core_pipeline::prepass::DepthPrepass, bevy::core_pipeline::prepass::MotionVectorPrepass)); }
    }
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
    cfg: Res<Config>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    buttons: Res<ButtonInput<MouseButton>>,
    origin: Res<WorldOrigin>,
    spatial: avian3d::prelude::SpatialQuery,
    mut q: Query<(&mut Transform, &mut FlyCam)>,
) {
    let Ok((mut t, mut cam)) = q.single_mut() else { return };
    let dt = time.delta_secs();
    if buttons.pressed(MouseButton::Right) || buttons.pressed(MouseButton::Left) {
        cam.yaw -= mouse.delta.x * 0.003;
        cam.pitch = (cam.pitch - mouse.delta.y * 0.003).clamp(-1.55, 1.55);
    }
    // Scripted route (`--spin`, `--cruise`) for unattended perf runs.
    if let Some(spin) = cfg.spin {
        cam.yaw -= spin.to_radians() * dt;
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
    if let Some(v) = cfg.cruise {
        // Level flight along the heading, so AGL stays roughly constant over flat ground.
        let level = (fwd - up * fwd.dot(up)).normalize_or_zero();
        t.translation += level * v * dt;
        // Terrain follow: hold the start AGL over the colliders (ground + roofs),
        // so a long route never flies into a mountain. Eased, so it stays smooth.
        let top = t.translation + up * 4000.0;
        if let Ok(down) = Dir3::new(-up) {
            if let Some(hit) = spatial.cast_ray(top, down, 8000.0, true, &avian3d::prelude::SpatialQueryFilter::default()) {
                let agl = hit.distance - 4000.0;
                let err = cfg.start_agl as f32 - agl;
                t.translation += up * err * (1.0 - (-2.0 * dt).exp());
            }
        }
    }
}

/// Render layer seen by the main camera but not the sun: meshes drawn without
/// casting shadows while a cheaper stand-in on `clouds::CLOUD_SHADOW_LAYER`
/// (sun only) casts for them. Used for full-detail building chunks.
pub const CAMERA_ONLY_LAYER: usize = 6;
