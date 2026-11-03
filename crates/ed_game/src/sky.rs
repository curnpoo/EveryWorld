//! Sun, sky, Moon and planets, ocean, weather-driven fog.
//!
//! Lighting pipeline (the "filmic" look):
//!   HDR camera -> physically based atmosphere (sky + aerial perspective) ->
//!   atmosphere-generated environment map (sky-lit ambient + reflections) ->
//!   real-lux sun with cascaded shadows -> bloom -> AgX tonemapping.

use crate::config::Config;
use crate::origin::{Focus, GeoAnchored, WorldOrigin};
use bevy::light::atmosphere::ScatteringMedium;
use bevy::light::{Atmosphere, CascadeShadowConfigBuilder, SunDisk, light_consts::lux};
use bevy::prelude::*;
use ed_space::{BODIES, BodyId, Snapshot};
use glam::{DAffine3, DVec3};

pub struct SkyPlugin;

/// Planet visuals and navigation markers are reserved for spaceflight.
pub const SPACE_ALTITUDE_M: f64 = 100_000.0;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Clock { unix_s: 0.0, rate: 1.0 })
            .add_systems(Startup, (setup_sky, fetch_weather))
            .add_systems(Update, (tick_clock, update_sun, update_bodies, follow_ocean));
    }
}

/// Game clock (real UTC by default; timelapse by changing `rate`).
#[derive(Resource)]
pub struct Clock {
    pub unix_s: f64,
    pub rate: f64,
}

#[derive(Resource, Default, Clone)]
pub struct CurrentWeather(pub Option<ed_data::weather::Weather>);

#[derive(Component)]
pub struct Sun;

#[derive(Component)]
pub struct CelestialBody(pub BodyId);

#[derive(Component)]
struct EarthAtmosphere;

#[derive(Component)]
struct Ocean;

fn setup_sky(
    mut commands: Commands,
    cfg: Res<Config>,
    mut clock: ResMut<Clock>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut media: ResMut<Assets<ScatteringMedium>>,
    wm: Res<crate::assets::WorldMaterials>,
) {
    clock.unix_s = cfg.now_unix();
    commands.spawn((
        Sun,
        DirectionalLight { illuminance: lux::RAW_SUNLIGHT, shadow_maps_enabled: true, ..default() },
        SunDisk::EARTH,
        CascadeShadowConfigBuilder { first_cascade_far_bound: 60.0, maximum_distance: 1_500.0, num_cascades: 3, ..default() }.build(),
        Transform::default(),
    ));
    if cfg.atmosphere {
        // Planet center sits at ECEF origin; GeoAnchored keeps it there across rebases.
        let medium = media.add(ScatteringMedium::earth(256, 256));
        commands.spawn((EarthAtmosphere, Atmosphere::earth(medium), GeoAnchored(DAffine3::IDENTITY), Transform::default()));
    }
    // Moon and planets at their true positions and sizes.
    let sphere = meshes.add(Sphere::new(1.0).mesh().uv(64, 32));
    for b in BODIES.iter().filter(|b| !matches!(b.id, BodyId::Earth | BodyId::Sun)) {
        let c = b.color;
        let mat = mats.add(StandardMaterial {
            base_color: Color::srgb_u8((c >> 16) as u8, (c >> 8) as u8, c as u8),
            perceptual_roughness: 0.95,
            ..default()
        });
        commands.spawn((CelestialBody(b.id), Mesh3d(sphere.clone()), MeshMaterial3d(mat), Transform::default(), bevy::light::NotShadowCaster));
    }
    commands.spawn((Ocean, Mesh3d(meshes.add(Mesh::from(Plane3d::default()))), MeshMaterial3d(wm.ocean.clone()), Transform::default(), bevy::light::NotShadowCaster));
}

fn fetch_weather(mut commands: Commands, cfg: Res<Config>) {
    let w = match ed_data::weather::fetch(&cfg.cache, cfg.start.lat_deg, cfg.start.lon_deg) {
        Ok(w) => {
            info!("weather: {} {:.0}°C, clouds {:.0}%, wind {:.0} m/s", w.describe(), w.temperature_c, w.cloud_cover_pct, w.wind_speed_ms);
            Some(w)
        }
        Err(e) => {
            warn!("weather unavailable ({e}); fair weather");
            None
        }
    };
    commands.insert_resource(CurrentWeather(w));
}

fn tick_clock(time: Res<Time>, mut clock: ResMut<Clock>, keys: Res<ButtonInput<KeyCode>>) {
    // [ and ] scrub time of day; \ resets to real time rate.
    if keys.pressed(KeyCode::BracketRight) {
        clock.unix_s += 1800.0 * time.delta_secs_f64();
    }
    if keys.pressed(KeyCode::BracketLeft) {
        clock.unix_s -= 1800.0 * time.delta_secs_f64();
    }
    clock.unix_s += time.delta_secs_f64() * clock.rate;
}

fn update_sun(
    clock: Res<Clock>,
    origin: Res<WorldOrigin>,
    focus: Query<&GlobalTransform, With<Focus>>,
    mut sun: Query<(&mut Transform, &mut DirectionalLight), With<Sun>>,
) {
    let Ok((mut t, mut light)) = sun.single_mut() else { return };
    let observer = focus.iter().next().map(|f| origin.to_geodetic(f.translation())).unwrap_or(origin.frame.origin);
    let s = ed_geo::sun::sun_position(observer.lat_deg, observer.lon_deg, clock.unix_s);
    let dir_enu = s.direction_enu();
    let to_sun = ed_geo::enu_to_render(dir_enu).as_vec3();
    *t = Transform::default().looking_to(-to_sun, Vec3::Y);
    // Atmosphere handles reddening; we just fade the sun out below the horizon.
    let k = ((s.elevation_deg as f32 + 4.0) / 6.0).clamp(0.0, 1.0);
    light.illuminance = lux::RAW_SUNLIGHT * k + 1.0;
}

fn update_bodies(
    clock: Res<Clock>,
    origin: Res<WorldOrigin>,
    focus: Query<&GlobalTransform, With<Focus>>,
    mut bodies: Query<(&CelestialBody, &mut Transform, &mut Visibility)>,
) {
    let snap = Snapshot::at(clock.unix_s);
    let focus_pos = focus.iter().next().map(|f| f.translation());
    let in_space = focus_pos
        .map(|p| origin.to_geodetic(p).alt_m >= SPACE_ALTITUDE_M)
        .unwrap_or(false);
    let cam = focus_pos.map(|p| p.as_dvec3()).unwrap_or_default();
    for (b, mut t, mut visibility) in &mut bodies {
        *visibility = if in_space { Visibility::Visible } else { Visibility::Hidden };
        let p = origin.ecef_to_render(snap.ecef(b.0));
        let info = ed_space::info(b.0);
        let dist = (p - cam).length();
        // Never smaller than ~1.5 arcmin so planets read as bright points.
        let r = info.radius_m.max(dist * 0.000_45);
        t.translation = p.as_vec3();
        t.scale = Vec3::splat(r as f32);
    }
}

/// The ocean: a curved sea-level disc centered under the player, rebuilt
/// whenever the origin moves. Exponentially spaced rings = fine detail near
/// the coast you're standing on, cheap at the horizon.
fn follow_ocean(
    origin: Res<WorldOrigin>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut q: Query<(&mut Mesh3d, &mut Transform), With<Ocean>>,
    mut built_gen: Local<Option<u64>>,
) {
    if *built_gen == Some(origin.generation) {
        return;
    }
    let Ok((mut mesh, mut t)) = q.single_mut() else { return };
    let mut m = ed_world::mesh::MeshData::default();
    let (rings, seg) = (48usize, 96usize);
    let center = origin.frame.origin;
    m.vertex(origin.to_render(ed_geo::Geodetic { alt_m: 0.0, ..center }).to_array(), [0.0, 1.0, 0.0], [0.0, 0.0], [1.0; 4]);
    for r in 1..=rings {
        let dist = 20.0 * (1.22f64).powi(r as i32); // up to ~280 km
        for s in 0..seg {
            let g = center.destination(s as f64 / seg as f64 * 360.0, dist);
            let p = origin.frame.geodetic_to_render(ed_geo::Geodetic { alt_m: 0.0, ..g });
            let n = origin.frame.up_at_render(p);
            m.vertex(p.as_vec3().to_array(), n.as_vec3().to_array(), [0.0, 0.0], [1.0; 4]);
        }
    }
    for s in 0..seg as u32 {
        let (a, b) = (1 + s, 1 + (s + 1) % seg as u32);
        m.tri(0, b, a);
    }
    for r in 0..(rings - 1) as u32 {
        for s in 0..seg as u32 {
            let i0 = 1 + r * seg as u32;
            let (a, b) = (i0 + s, i0 + (s + 1) % seg as u32);
            let (c, d) = (a + seg as u32, b + seg as u32);
            m.tri(a, b, d);
            m.tri(a, d, c);
        }
    }
    // Fix winding to face up (compare first triangle against local up).
    let up = Vec3::Y;
    let [a, b, c] = [m.indices[0], m.indices[1], m.indices[2]].map(|i| Vec3::from(m.positions[i as usize]));
    if (b - a).cross(c - a).dot(up) < 0.0 {
        for tri in m.indices.chunks_exact_mut(3) {
            tri.swap(1, 2);
        }
    }
    mesh.0 = meshes.add(crate::meshconv::to_mesh(&m));
    *t = Transform::default();
    *built_gen = Some(origin.generation);
    let _ = DVec3::ZERO;
}
