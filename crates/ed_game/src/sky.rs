//! Sun, sky, Moon and planets, ocean, weather-driven fog.
//!
//! Lighting pipeline (the "filmic" look):
//!   HDR camera -> physically based atmosphere (sky + aerial perspective) ->
//!   atmosphere-generated environment map (sky-lit ambient + reflections) ->
//!   real-lux sun with cascaded shadows -> bloom -> AgX tonemapping.

use crate::config::Config;
use crate::origin::{Focus, GeoAnchored, WorldOrigin};
use bevy::light::atmosphere::ScatteringMedium;
use bevy::light::{Atmosphere, SunDisk, light_consts::lux};
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
            .add_systems(Update, (tick_clock, update_sun, update_bodies, follow_ocean, follow_ocean_globe))
            .add_systems(PostUpdate, refresh_env_map);
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

/// Whole-planet sea-level shell for views from high altitude, where the local
/// disc (280 km radius) can't reach the horizon.
#[derive(Component)]
struct OceanGlobe;

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
        DirectionalLight { illuminance: lux::RAW_SUNLIGHT, shadow_maps_enabled: !std::env::args().any(|a| a == "--no-shadows"), ..default() },
        SunDisk::EARTH,
        // Four cascades re-planned from height above ground (`shadows.rs`,
        // docs/plans/shadows.md): ~5.5x texel steps, 2.5 km reach on foot,
        // fade to lit instead of a hard cut. `EW_SHADOWS=legacy` = old 2 x 700 m.
        crate::shadows::cascade_config(cfg.start_agl as f32),
        Transform::default(),
    ));
    if cfg.atmosphere {
        // Planet center sits at ECEF origin; GeoAnchored keeps it there across rebases.
        let medium = media.add(ScatteringMedium::earth(256, 256));
        // Where no terrain tile covers the view (open ocean at low zoom) the atmosphere's
        // own ground sphere shows through; make it deep-ocean navy, not mid-grey.
        let mut atm = Atmosphere::earth(medium);
        atm.ground_albedo = Vec3::new(0.012, 0.045, 0.075);
        commands.spawn((EarthAtmosphere, atm, GeoAnchored(DAffine3::IDENTITY), Transform::default()));
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
    commands.spawn((OceanGlobe, bevy::camera::visibility::NoFrustumCulling, Mesh3d(meshes.add(Mesh::from(Plane3d::default()))), MeshMaterial3d(mats.add(StandardMaterial {
            // Orbit view: matte deep blue. A mirror-smooth sea at grazing angles only
            // reflects the hazy sky and reads as grey-brown.
            base_color: Color::srgb(0.035, 0.20, 0.42),
            perceptual_roughness: 0.5,
            reflectance: 0.12,
            ..default()
        })), Transform::default(), Visibility::Hidden, bevy::light::NotShadowCaster));
}

fn fetch_weather(mut commands: Commands, cfg: Res<Config>) {
    // `--weather <preset>` (and lookdev, which must be deterministic) skip the network.
    let explicit = std::env::args().any(|a| a == "--weather");
    let lookdev_fair = if explicit { None } else { cfg.lookdev.as_ref().and_then(|_| ed_world::clouds::weather_preset("fair")) };
    if let Some(w) = lookdev_fair.or_else(crate::clouds::weather_override) {
        info!("weather (override): {} {:.0}% clouds", w.describe(), w.cloud_cover_pct);
        commands.insert_resource(CurrentWeather(Some(w)));
        return;
    }
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

fn tick_clock(
    time: Res<Time>,
    mut clock: ResMut<Clock>,
    keys: Res<ButtonInput<KeyCode>>,
    origin: Res<WorldOrigin>,
    mut gfx: Option<ResMut<crate::menu::GraphicsSettings>>,
) {
    // [ and ] scrub time of day; \ resets to real time rate.
    let mut scrubbed = false;
    if keys.pressed(KeyCode::BracketRight) {
        clock.unix_s += 1800.0 * time.delta_secs_f64();
        scrubbed = true;
    }
    if keys.pressed(KeyCode::BracketLeft) {
        clock.unix_s -= 1800.0 * time.delta_secs_f64();
        scrubbed = true;
    }
    if scrubbed {
        // Scrubbing picks an hour, so the map clicks and drops keep it.
        if let Some(g) = gfx.as_mut() {
            g.time_live = false;
            g.solar_hour = ed_geo::sun::solar_hour(clock.unix_s, origin.frame.origin.lon_deg);
        }
    }
    clock.unix_s += time.delta_secs_f64() * clock.rate;
}

fn update_sun(
    clock: Res<Clock>,
    origin: Res<WorldOrigin>,
    focus: Query<&GlobalTransform, With<Focus>>,
    mut sun: Query<(&mut Transform, &mut DirectionalLight), With<Sun>>,
    clouds: Option<Res<crate::clouds::CloudSunFactor>>,
) {
    let Ok((mut t, mut light)) = sun.single_mut() else { return };
    let observer = focus.iter().next().map(|f| origin.to_geodetic(f.translation())).unwrap_or(origin.frame.origin);
    let s = ed_geo::sun::sun_position(observer.lat_deg, observer.lon_deg, clock.unix_s);
    let dir_enu = s.direction_enu();
    let to_sun = ed_geo::enu_to_render(dir_enu).as_vec3();
    *t = Transform::default().looking_to(-to_sun, Vec3::Y);
    // Atmosphere handles reddening; we just fade the sun out below the horizon.
    let k = ((s.elevation_deg as f32 + 4.0) / 6.0).clamp(0.0, 1.0);
    // Clouds: an overcast deck takes most of the direct sun off the ground.
    let cloud = clouds.map_or(1.0, |c| c.0);
    light.illuminance = lux::RAW_SUNLIGHT * k * cloud + 1.0;
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
    mut q: Query<(&mut Mesh3d, &mut Transform, &mut Visibility), With<Ocean>>,
    focus: Query<&GlobalTransform, With<Focus>>,
    mut built_gen: Local<Option<u64>>,
) {
    // From high up, terrain tiles carry the satellite water colour themselves;
    // the sea disc would only add mismatched patches where it clips them.
    if let Some(f) = focus.iter().next() {
        let high = origin.to_geodetic(f.translation()).alt_m > 30_000.0;
        if let Ok((_, _, mut vis)) = q.single_mut() {
            *vis = if high { Visibility::Hidden } else { Visibility::Inherited };
        }
    }
    if *built_gen == Some(origin.generation) {
        return;
    }
    let Ok((mut mesh, mut t, _)) = q.single_mut() else { return };
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

/// Sea-level ellipsoid, 1 degree cells (chord sag ~240 m, hidden under the
/// bathymetry-carrying terrain tiles). Rebuilt when the floating origin moves,
/// shown only from high altitude where it replaces the local disc.
fn follow_ocean_globe(
    origin: Res<WorldOrigin>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut q: Query<(&mut Mesh3d, &mut Visibility), With<OceanGlobe>>,
    focus: Query<&GlobalTransform, With<Focus>>,
    mut built_gen: Local<Option<u64>>,
) {
    let Ok((mut mesh, mut vis)) = q.single_mut() else { return };
    let high = focus.iter().next().is_some_and(|f| origin.to_geodetic(f.translation()).alt_m > 30_000.0);
    *vis = if high { Visibility::Inherited } else { Visibility::Hidden };
    if !high || *built_gen == Some(origin.generation) {
        return;
    }
    let (rows, cols) = (360usize, 720usize);
    let mut m = ed_world::mesh::MeshData::default();
    for r in 0..=rows {
        let lat = -90.0 + 180.0 * r as f64 / rows as f64;
        for c in 0..=cols {
            let lon = -180.0 + 360.0 * c as f64 / cols as f64;
            let p = origin.frame.geodetic_to_render(ed_geo::Geodetic { lat_deg: lat, lon_deg: lon, alt_m: 80.0 });
            let n = origin.frame.up_at_render(p);
            m.vertex(p.as_vec3().to_array(), n.as_vec3().to_array(), [0.0, 0.0], [1.0; 4]);
        }
    }
    let w = cols as u32 + 1;
    for r in 0..rows as u32 {
        for c in 0..cols as u32 {
            let (a, b) = (r * w + c, r * w + c + 1);
            let (cc, d) = (a + w, b + w);
            m.tri(a, b, d);
            m.tri(a, d, cc);
        }
    }
    // Face outward: compare a mid-latitude triangle with its vertex normal.
    let probe = (rows as u32 / 2 + 20) * cols as u32 * 6 + 12;
    let i = (probe as usize).min(m.indices.len() - 3);
    let [a, b, c] = [m.indices[i], m.indices[i + 1], m.indices[i + 2]].map(|k| Vec3::from(m.positions[k as usize]));
    let n = Vec3::from(m.normals[m.indices[i] as usize]);
    if (b - a).cross(c - a).dot(n) < 0.0 {
        for tri in m.indices.chunks_exact_mut(3) {
            tri.swap(1, 2);
        }
    }
    mesh.0 = meshes.add(crate::meshconv::to_mesh(&m));
    *built_gen = Some(origin.generation);
    info!("ocean globe built: {} verts", m.positions.len());
}

/// The atmosphere environment map (ambient light + reflections) used to be
/// re-rendered and re-filtered every frame (~2 ms on M3 Pro). The sky only
/// changes when the sun moves, the camera changes altitude, or the weather
/// dims the sun, so refresh on those and reuse the last map otherwise.
#[derive(Default)]
struct EnvRefreshState {
    to_sun: Vec3,
    alt_m: f64,
    cloud: f32,
    frames_left: u8,
    started: bool,
}

/// Sun direction change that triggers a refresh (about 0.3 degrees; at the
/// real clock rate that is roughly once a minute).
const ENV_SUN_COS: f32 = 0.999_986;

fn refresh_env_map(
    mut st: Local<EnvRefreshState>,
    mut refresh: ResMut<bevy::pbr::EnvironmentMapRefresh>,
    origin: Res<WorldOrigin>,
    sun: Query<&GlobalTransform, With<Sun>>,
    focus: Query<&GlobalTransform, With<Focus>>,
    added: Query<(), Added<bevy::light::AtmosphereEnvironmentMapLight>>,
    clouds: Option<Res<crate::clouds::CloudSunFactor>>,
) {
    let to_sun = sun.single().map(|t| -t.forward().as_vec3()).unwrap_or(Vec3::Y);
    let alt = focus.iter().next().map_or(0.0, |f| origin.to_geodetic(f.translation()).alt_m);
    let cloud = clouds.map_or(1.0, |c| c.0);
    let alt_step = (alt.abs() * 0.03).max(50.0);
    let changed = !st.started
        || !added.is_empty()
        || to_sun.dot(st.to_sun) < ENV_SUN_COS
        || (alt - st.alt_m).abs() > alt_step
        || (cloud - st.cloud).abs() > 0.02;
    if changed {
        st.started = true;
        st.to_sun = to_sun;
        st.alt_m = alt;
        st.cloud = cloud;
        // Generate on one frame, filter on the next (filtering runs first in
        // the render schedule), plus one spare frame for the new image upload.
        st.frames_left = st.frames_left.max(3);
    }
    let run = st.frames_left > 0 || std::env::var_os("EW_ENV_ALWAYS").is_some(); // TMP-DBG
    st.frames_left = st.frames_left.saturating_sub(1);
    if refresh.0 != run {
        refresh.0 = run;
    }
}
