//! Volumetric clouds in the real sky.
//!
//! A full-screen triangle with a custom material raymarches a spherical cloud
//! shell around the planet (see `clouds.wesl`). It draws in the transparent
//! pass, after the physically based sky, and reads the depth prepass so
//! terrain and buildings occlude clouds correctly.
//!
//! - Shapes: tileable Perlin-Worley + Worley 3D textures (`ed_world::clouds`),
//!   sampled in ECEF modulo a fixed period, so the clouds are anchored to the
//!   planet and the floating origin can move under them without a seam.
//! - Climate: live weather (`CurrentWeather`, hourly forecast while
//!   timelapsing) picks coverage, cumulus vs stratus, storm cells and rain.
//! - Wind drifts the whole field (and the storm cells) downwind.
//! - Light: sun through an analytic atmosphere (warm low sun, Earth shadow
//!   for sunset tops), dual-lobe phase for silver linings, multi-octave
//!   multiple scattering, sky/ground ambient for blue-grey bellies, haze.
//!
//! Flags: `--weather clear|fair|overcast|rain|showers|storm|fog` overrides the
//! live report, `--cloud-quality low|medium|high` (low is picked
//! automatically on software renderers), `--no-clouds`.

use crate::origin::WorldOrigin;
use crate::sky::{Clock, CurrentWeather, Sun};
use bevy::asset::{RenderAssetUsages, embedded_asset};
use bevy::camera::visibility::NoFrustumCulling;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::camera::visibility::RenderLayers;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::mesh::{MeshVertexBufferLayoutRef, PrimitiveTopology};
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, CompareFunction, Extent3d, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError, TextureDimension,
    TextureFormat,
};
use bevy::shader::ShaderRef;
use ed_world::clouds::{self as wc, CloudParams, Conditions, StormCell};
use glam::DVec3;

pub struct CloudsPlugin;

impl Plugin for CloudsPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "clouds.wesl");
        let quality = CloudQuality::from_args();
        app.add_plugins(MaterialPlugin::<CloudMaterial>::default())
            .insert_resource(quality)
            .init_resource::<CloudState>()
            .init_resource::<CloudOverride>()
            .add_systems(Startup, setup_clouds)
            .add_systems(Update, (pick_quality, apply_lookdev_presets, update_climate, sun_sees_cloud_shadows).chain())
            .add_systems(PostUpdate, update_uniforms.after(TransformSystems::Propagate));
    }
}

/// `--flag value` from the command line (clouds keep their flags local so
/// they don't collide with other modules' config).
fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

/// `--weather <preset>`: synthetic weather instead of the live report.
pub fn weather_override() -> Option<ed_data::weather::Weather> {
    let name = arg("--weather")?;
    let w = wc::weather_preset(&name);
    if w.is_none() {
        warn!("unknown --weather {name:?} (try clear, fair, overcast, rain, showers, storm, fog)");
    }
    w
}

#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct CloudQuality {
    /// Max raymarch steps per pixel.
    pub steps: u32,
    /// Steps toward the sun per lit sample.
    pub light_steps: u32,
    /// How far out clouds are drawn (m).
    pub max_distance_m: f32,
    /// Raymarch step length multiplier (bigger = faster, coarser).
    pub step_scale: f32,
    /// User asked for a specific level (don't auto-downgrade).
    pub explicit: bool,
    pub enabled: bool,
}

impl CloudQuality {
    pub const LOW: Self = Self { steps: 64, light_steps: 4, max_distance_m: 160_000.0, step_scale: 1.8, explicit: false, enabled: true };
    pub const MEDIUM: Self = Self { steps: 110, light_steps: 5, max_distance_m: 220_000.0, step_scale: 1.2, explicit: false, enabled: true };
    pub const HIGH: Self = Self { steps: 160, light_steps: 6, max_distance_m: 300_000.0, step_scale: 0.8, explicit: false, enabled: true };

    fn from_args() -> Self {
        let mut q = match arg("--cloud-quality").as_deref() {
            Some("low") => Self { explicit: true, ..Self::LOW },
            Some("medium") => Self { explicit: true, ..Self::MEDIUM },
            Some("high") => Self { explicit: true, ..Self::HIGH },
            _ => Self::HIGH,
        };
        q.enabled = !std::env::args().any(|a| a == "--no-clouds");
        q
    }
}

/// Software rasterizers (lavapipe in CI) get the cheap settings.
fn pick_quality(mut q: ResMut<CloudQuality>, info: Option<Res<bevy::render::renderer::RenderAdapterInfo>>, mut done: Local<bool>) {
    if *done {
        return;
    }
    let Some(info) = info else { return };
    *done = true;
    if !q.explicit && format!("{:?}", info.device_type) == "Cpu" {
        info!("clouds: software renderer ({}), using low quality", info.name);
        *q = CloudQuality { enabled: q.enabled, ..CloudQuality::LOW };
    }
}

/// Forces the cloud climate (lookdev shots, scripted weather). `None` fields
/// fall back to live weather.
#[derive(Resource, Default, Clone)]
pub struct CloudOverride {
    pub params: Option<CloudParams>,
    /// Use exactly these storm cells instead of the generated field.
    pub storms: Option<Vec<StormCell>>,
}

/// Live cloud state: smoothed climate and wind drift.
#[derive(Resource)]
pub struct CloudState {
    pub params: CloudParams,
    /// How far the wind has carried the noise field (ECEF meters).
    pub drift_ecef: DVec3,
    /// Same, for the storm grid (degrees lat, lon).
    pub storm_drift_deg: [f64; 2],
    last_clock: Option<f64>,
    snapped: bool,
}

impl Default for CloudState {
    fn default() -> Self {
        Self { params: CloudParams::default(), drift_ecef: DVec3::ZERO, storm_drift_deg: [0.0; 2], last_clock: None, snapped: false }
    }
}

/// Fraction of direct sun reaching the ground under the current clouds
/// (read by `sky::update_sun`).
#[derive(Resource, Clone, Copy)]
pub struct CloudSunFactor(pub f32);

#[derive(Clone, Default, ShaderType)]
pub struct CloudUniform {
    /// Render-space direction -> noise (ECEF) axes.
    pub noise_from_render: Mat3,
    /// Camera position in noise space (ECEF minus drift, modulo period).
    pub cam_noise: Vec4,
    /// Camera position in render space.
    pub cam_pos: Vec4,
    /// xyz: local up at the camera, w: camera altitude (m above sea level).
    pub cam_up: Vec4,
    /// x: planet radius, y: max distance, z: sun white balance (0 = physical), w: brightness gain.
    pub planet: Vec4,
    /// Cumulus: base, top, coverage, density multiplier.
    pub cu: Vec4,
    /// Stratus: base, top, amount, belly darkness.
    pub st: Vec4,
    /// x: precipitation, y: storm count, z: steps, w: light steps.
    pub misc: Vec4,
    /// xyz: downwind direction (render), w: wind speed (m/s).
    pub wind: Vec4,
    /// Shell bottom, shell top (m above sea level), haze distance (m), step scale.
    pub shell: Vec4,
    /// xyz: direction to the sun, w: sun illuminance above the atmosphere (lux).
    pub sun: Vec4,
    /// Sky irradiance on an upward surface (lux, rgb).
    pub sky_ambient: Vec4,
    /// Light bounced up from the ground (lux, rgb).
    pub ground_ambient: Vec4,
    /// Horizon haze radiance (lux/sr-ish, rgb), w: glow toward the sun.
    pub haze: Vec4,
    /// Storm cells: center at sea level (render), w: core radius.
    pub storm_pos: [Vec4; MAX_STORMS],
    /// Storm cells: up at center, w: anvil top (m).
    pub storm_up: [Vec4; MAX_STORMS],
}

pub const MAX_STORMS: usize = 8;

#[derive(Asset, TypePath, AsBindGroup, Clone)]
#[bind_group_data(CloudKey)]
pub struct CloudMaterial {
    /// The sky layer (false) or the shadow-only caster plane (true).
    pub shadow_caster: bool,
    #[uniform(0)]
    pub u: CloudUniform,
    #[texture(1, dimension = "3d")]
    #[sampler(2)]
    pub shape: Handle<Image>,
    #[texture(3, dimension = "3d")]
    #[sampler(4)]
    pub detail: Handle<Image>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct CloudKey {
    shadow_caster: bool,
}

impl From<&CloudMaterial> for CloudKey {
    fn from(m: &CloudMaterial) -> Self {
        Self { shadow_caster: m.shadow_caster }
    }
}

impl Material for CloudMaterial {
    fn vertex_shader() -> ShaderRef {
        "embedded://earthdrop/clouds.wesl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "embedded://earthdrop/clouds.wesl".into()
    }
    fn prepass_fragment_shader() -> ShaderRef {
        "embedded://earthdrop/clouds.wesl".into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        if self.shadow_caster { AlphaMode::Mask(0.5) } else { AlphaMode::Premultiplied }
    }
    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if key.bind_group_data.shadow_caster {
            return Ok(());
        }
        // A screen-covering triangle: never culled, never depth tested
        // (the shader clips the march against the depth prepass itself).
        descriptor.primitive.cull_mode = None;
        if let Some(ds) = descriptor.depth_stencil.as_mut() {
            ds.depth_compare = Some(CompareFunction::Always);
            ds.depth_write_enabled = Some(false);
        }
        Ok(())
    }
}

#[derive(Component)]
pub(crate) struct CloudLayer;

fn noise_image(size: usize, data: Vec<u8>) -> Image {
    let s = size as u32;
    let mut img = Image::new(
        Extent3d { width: s, height: s, depth_or_array_layers: s },
        TextureDimension::D3,
        data,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        address_mode_w: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        ..default()
    });
    img
}

pub const SHAPE_RES: usize = 128;
pub const DETAIL_RES: usize = 64;

fn setup_clouds(
    mut commands: Commands,
    q: Res<CloudQuality>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<CloudMaterial>>,
) {
    commands.insert_resource(CloudSunFactor(1.0));
    if !q.enabled {
        return;
    }
    let t0 = std::time::Instant::now();
    let (shape, detail) = std::thread::scope(|s| {
        let a = s.spawn(|| wc::shape_noise(SHAPE_RES));
        let b = s.spawn(|| wc::detail_noise(DETAIL_RES));
        (a.join().unwrap(), b.join().unwrap())
    });
    info!("clouds: noise volumes built in {:.2} s", t0.elapsed().as_secs_f32());
    let sky = CloudMaterial {
        shadow_caster: false,
        u: CloudUniform::default(),
        shape: images.add(noise_image(SHAPE_RES, shape)),
        detail: images.add(noise_image(DETAIL_RES, detail)),
    };
    let caster = CloudMaterial { shadow_caster: true, ..sky.clone() };
    let mat = mats.add(sky);
    let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![[-1.0f32, -1.0, 0.0], [3.0, -1.0, 0.0], [-1.0, 3.0, 0.0]]);
    commands.spawn((
        CloudLayer,
        Name::new("cloud layer"),
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(mat),
        Transform::default(),
        NoFrustumCulling,
        NotShadowCaster,
    ));
    // Shadow caster: a big flat quad that follows the camera at cloud base,
    // on a render layer only the sun sees (see `shadow_discard` in the shader).
    let h = CLOUD_SHADOW_PLANE_HALF_M;
    let quad = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![[-h, 0.0, -h], [-h, 0.0, h], [h, 0.0, h], [h, 0.0, -h]])
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 1.0, 0.0]; 4])
        .with_inserted_indices(bevy::mesh::Indices::U32(vec![0, 1, 2, 0, 2, 3]));
    commands.spawn((
        CloudShadowCaster,
        Name::new("cloud shadow caster"),
        Mesh3d(meshes.add(quad)),
        MeshMaterial3d(mats.add(caster)),
        Transform::default(),
        NoFrustumCulling,
        NotShadowReceiver,
        RenderLayers::layer(CLOUD_SHADOW_LAYER),
    ));
}

/// Render layer seen only by the sun (for the cloud shadow caster).
pub const CLOUD_SHADOW_LAYER: usize = 7;
const CLOUD_SHADOW_PLANE_HALF_M: f32 = 20_000.0;

#[derive(Component)]
struct CloudShadowCaster;

/// Let the sun see the shadow-caster layer as well as the default one.
fn sun_sees_cloud_shadows(mut commands: Commands, sun: Query<Entity, (With<Sun>, Without<RenderLayers>)>) {
    for e in &sun {
        commands.entity(e).insert(RenderLayers::from_layers(&[0, CLOUD_SHADOW_LAYER]));
    }
}

/// Pick the target climate from weather (or an override) and ease toward it.
fn update_climate(
    time: Res<Time>,
    clock: Res<Clock>,
    weather: Option<Res<CurrentWeather>>,
    over: Res<CloudOverride>,
    mut st: ResMut<CloudState>,
    mut sun_factor: ResMut<CloudSunFactor>,
) {
    let target = if let Some(p) = over.params {
        p
    } else {
        match weather.as_ref().and_then(|w| w.0.as_ref()) {
            Some(w) => CloudParams::from_conditions(&Conditions::at(w, clock.unix_s)),
            None => CloudParams::default(),
        }
    };
    // Snap on the first frame and whenever an override is in force (lookdev
    // cuts); otherwise drift toward the new weather over a few minutes of
    // game time so fronts roll in instead of popping.
    if !st.snapped || over.is_changed() {
        st.params = target;
        st.snapped = true;
    } else {
        let game_dt = st.last_clock.map(|c| (clock.unix_s - c).abs()).unwrap_or(0.0) as f32;
        let k = 1.0 - (-(game_dt.max(time.delta_secs())) / 240.0).exp();
        st.params = st.params.lerp(&target, k);
    }
    sun_factor.0 = st.params.sun_fraction();
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Rough sky lighting for a sun elevation (lux). The atmosphere renders the
/// actual sky; this only has to light the clouds consistently with it.
fn sky_light(sun_elev_deg: f32, sun_lux: f32) -> (Vec3, Vec3, Vec3, f32) {
    let e = sun_elev_deg;
    let s = |a: f32, b: f32, x: f32| ((x - a) / (b - a)).clamp(0.0, 1.0);
    let smooth = |a: f32, b: f32, x: f32| {
        let t = s(a, b, x);
        t * t * (3.0 - 2.0 * t)
    };
    let day = smooth(-7.0, 40.0, e);
    let golden = smooth(-4.0, 3.0, e) * (1.0 - smooth(4.0, 12.0, e));
    // Sky irradiance: ~20 klx at high sun, ~1 klx at sunset, a little twilight.
    let sky_lux = 20_000.0 * day.powf(1.4) + 2_500.0 * smooth(-10.0, 0.0, e) * (1.0 - smooth(5.0, 20.0, e));
    let blue = Vec3::new(0.60, 0.74, 1.0);
    let dusk = Vec3::new(0.95, 0.72, 0.80);
    let sky_col = blue.lerp(dusk, golden * 0.7);
    let sky = sky_col / sky_col.max_element() * sky_lux;
    // Ground bounce: sun on the ground times a green-brown albedo.
    let sun_ground = sun_lux * (e.max(0.0).to_radians().sin()) * (0.25 + 0.75 * smooth(0.0, 15.0, e));
    let ground = Vec3::new(0.15, 0.15, 0.14) * sun_ground * 0.6 + sky * 0.12;
    // Horizon haze radiance (pre-exposure, lux/sr-ish): brighter than the zenith.
    let haze_col = Vec3::new(0.78, 0.84, 0.96).lerp(Vec3::new(1.0, 0.72, 0.55), golden);
    let haze = haze_col * (sky_lux / std::f32::consts::PI) * 3.0;
    (sky, ground, haze, 0.5 + 6.0 * golden)
}

#[allow(clippy::too_many_arguments)]
fn update_uniforms(
    clock: Res<Clock>,
    origin: Res<WorldOrigin>,
    q: Res<CloudQuality>,
    over: Res<CloudOverride>,
    mut st: ResMut<CloudState>,
    cams: Query<&GlobalTransform, (With<Camera3d>, Without<crate::globe::GlobeCamera>)>,
    sun: Query<&GlobalTransform, With<Sun>>,
    layers: Query<&MeshMaterial3d<CloudMaterial>>,
    mut caster: Query<&mut Transform, With<CloudShadowCaster>>,
    mut mats: ResMut<Assets<CloudMaterial>>,
) {
    let Ok(cam) = cams.single() else { return };
    let cam_pos = cam.translation();
    let geo = origin.to_geodetic(cam_pos);
    let p = st.params;

    // Wind drift, in game time (timelapse moves clouds faster).
    let dt = st.last_clock.map(|c| (clock.unix_s - c).clamp(-7_200.0, 7_200.0)).unwrap_or(0.0);
    st.last_clock = Some(clock.unix_s);
    let [we, wn] = p.wind_dir_en();
    let up = origin.up_at(cam_pos);
    // Render frame: x = east, -z = north (tilted with the planet).
    let east = Vec3::X - up * up.x;
    let north = Vec3::NEG_Z - up * -up.z;
    let downwind = (east.normalize() * we + north.normalize() * wn).normalize_or_zero();
    let r2e = origin.frame.render_to_ecef();
    let wind_ecef = r2e.matrix3 * (downwind.as_dvec3() * p.wind_speed_ms as f64);
    st.drift_ecef += wind_ecef * dt;
    let wind_en = [we * p.wind_speed_ms, wn * p.wind_speed_ms];
    st.storm_drift_deg = wc::advect_deg(geo.lat_deg, st.storm_drift_deg, wind_en, dt);

    let cam_noise = wc::noise_coord(r2e.transform_point3(cam_pos.as_dvec3()), st.drift_ecef);

    // Storm cells near the camera, nearest first.
    let cells = match &over.storms {
        Some(s) => s.clone(),
        None => wc::storm_cells(geo.lat_deg, geo.lon_deg, p.storm, st.storm_drift_deg[0], st.storm_drift_deg[1], 180_000.0),
    };
    let mut storm_pos = [Vec4::ZERO; MAX_STORMS];
    let mut storm_up = [Vec4::ZERO; MAX_STORMS];
    for (i, c) in cells.iter().take(MAX_STORMS).enumerate() {
        let at = origin.to_render(ed_geo::Geodetic::new(c.lat_deg, c.lon_deg, 0.0));
        storm_pos[i] = at.extend(c.radius_m);
        storm_up[i] = origin.up_at(at).extend(c.top_m);
    }
    let n_storms = cells.len().min(MAX_STORMS);

    // Sun.
    let to_sun = sun.single().map(|t| t.back().as_vec3()).unwrap_or(Vec3::Y);
    let sun_elev = to_sun.dot(up).clamp(-1.0, 1.0).asin().to_degrees();
    let sun_lux = bevy::light::light_consts::lux::RAW_SUNLIGHT;
    let (sky, ground, haze, glow) = sky_light(sun_elev, sun_lux);

    let ceiling = p.ceiling_m();
    let floor = p.cumulus_base_m.min(if p.stratus > 0.0 { p.stratus_base_m } else { f32::MAX }) - 150.0;
    let visibility = 60_000.0 * (1.0 - 0.5 * p.stratus.max(p.precipitation));

    let u = CloudUniform {
        noise_from_render: Mat3::from(r2e.matrix3.as_mat3()),
        cam_noise: cam_noise.as_vec3().extend(0.0),
        cam_pos: cam_pos.extend(1.0),
        cam_up: up.extend(geo.alt_m as f32),
        planet: Vec4::new(6_371_000.0, q.max_distance_m, 0.45 * smoothstep(4.0, 25.0, sun_elev), 1.0),
        cu: Vec4::new(p.cumulus_base_m, p.cumulus_top_m, p.coverage, p.density),
        st: Vec4::new(p.stratus_base_m, p.stratus_top_m, p.stratus, p.darkness),
        misc: Vec4::new(p.precipitation, n_storms as f32, q.steps as f32, q.light_steps as f32),
        wind: downwind.extend(p.wind_speed_ms),
        shell: Vec4::new(floor.max(0.0), ceiling, visibility, q.step_scale),
        sun: to_sun.extend(sun_lux),
        sky_ambient: sky.extend(0.0),
        ground_ambient: ground.extend(0.0),
        haze: haze.extend(glow),
        storm_pos,
        storm_up,
    };
    for handle in &layers {
        if let Some(mut m) = mats.get_mut(&handle.0) {
            m.u = u.clone();
        }
    }
    // Keep the shadow caster under the cloud base, centered on the camera.
    if let Ok(mut t) = caster.single_mut() {
        let plane_alt = floor.max(0.0) + 150.0;
        t.translation = cam_pos + up * (plane_alt - geo.alt_m as f32);
        t.rotation = Quat::from_rotation_arc(Vec3::Y, up);
    }
}

// ---------------------------------------------------------------------------
// Lookdev: cloud shots and the weather/time each one needs.
// ---------------------------------------------------------------------------

/// A cloud review shot, placed relative to the lookdev stage.
pub struct CloudShot {
    pub name: &'static str,
    /// Compass heading and pitch of the camera, degrees.
    pub heading_deg: f32,
    pub pitch_deg: f32,
    /// Eye height above the stage ground, meters.
    pub height_m: f32,
}

pub const LOOKDEV_SHOTS: [CloudShot; 7] = [
    CloudShot { name: "clouds_fair_cumulus", heading_deg: 75.0, pitch_deg: 16.0, height_m: 2.0 },
    CloudShot { name: "clouds_overcast_stratus", heading_deg: 20.0, pitch_deg: 14.0, height_m: 2.0 },
    CloudShot { name: "clouds_distant_thunderstorm", heading_deg: 110.0, pitch_deg: 10.0, height_m: 2.0 },
    CloudShot { name: "clouds_sunset", heading_deg: 75.0, pitch_deg: 14.0, height_m: 2.0 },
    CloudShot { name: "clouds_sunset_backlit", heading_deg: 238.0, pitch_deg: 9.0, height_m: 2.0 },
    CloudShot { name: "clouds_above_deck_4000m", heading_deg: 100.0, pitch_deg: -16.0, height_m: 4_000.0 },
    CloudShot { name: "clouds_ground_shadows", heading_deg: 285.0, pitch_deg: -27.0, height_m: 650.0 },
];

/// Render-space view direction for a compass heading and pitch (x = east,
/// -z = north at the stage).
pub fn heading_pitch_dir(heading_deg: f32, pitch_deg: f32) -> Vec3 {
    let (h, p) = (heading_deg.to_radians(), pitch_deg.to_radians());
    Vec3::new(h.sin() * p.cos(), p.sin(), -h.cos() * p.cos())
}

enum ShotTime {
    Hour(f64),
    /// Local solar afternoon hour when the sun is this high (degrees).
    SunElevation(f64),
}

struct ShotPreset {
    weather: &'static str,
    time: ShotTime,
    tweak: fn(&mut CloudParams),
    /// One storm cell this far out along the shot's heading (m).
    storm_at_m: Option<f64>,
}

fn preset_for(shot: &str) -> Option<ShotPreset> {
    let p = match shot {
        "clouds_ground_shadows" => ShotPreset {
            weather: "fair",
            time: ShotTime::Hour(13.0),
            tweak: |p| {
                p.coverage = 0.42;
                p.cumulus_base_m = 1_100.0;
            },
            storm_at_m: None,
        },
        "clouds_fair_cumulus" => ShotPreset { weather: "fair", time: ShotTime::Hour(14.0), tweak: |_| {}, storm_at_m: None },
        "clouds_overcast_stratus" => ShotPreset { weather: "overcast", time: ShotTime::Hour(13.0), tweak: |_| {}, storm_at_m: None },
        "clouds_distant_thunderstorm" => ShotPreset {
            weather: "fair",
            time: ShotTime::Hour(15.5),
            tweak: |p| {
                p.coverage = 0.28;
                p.precipitation = 1.0;
                p.wind_from_deg = 200.0;
            },
            storm_at_m: Some(40_000.0),
        },
        "clouds_sunset" | "clouds_sunset_backlit" => ShotPreset {
            weather: "fair",
            time: ShotTime::SunElevation(5.0),
            tweak: |p| {
                p.coverage = 0.26;
                p.cumulus_top_m = 3_800.0;
            },
            storm_at_m: None,
        },
        "clouds_above_deck_4000m" => ShotPreset {
            weather: "fair",
            time: ShotTime::Hour(15.0),
            tweak: |p| {
                // Broken stratocumulus with cumulus heads poking through.
                p.coverage = 0.45;
                p.stratus = 0.55;
                p.stratus_base_m = 1_300.0;
                p.stratus_top_m = 2_000.0;
                p.cumulus_top_m = 3_000.0;
            },
            storm_at_m: None,
        },
        _ => return None,
    };
    Some(p)
}

/// Unix time today at a local solar hour (same rule as `--hour`).
fn unix_at_solar_hour(lon_deg: f64, hour: f64) -> f64 {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
    (now / 86_400.0).floor() * 86_400.0 + (hour - lon_deg / 15.0) * 3600.0
}

/// Afternoon time when the sun drops to `elev_deg` (bisection, 12h..21h).
fn unix_at_sun_elevation(lat: f64, lon: f64, elev_deg: f64) -> f64 {
    let (mut a, mut b) = (unix_at_solar_hour(lon, 12.0), unix_at_solar_hour(lon, 21.0));
    for _ in 0..40 {
        let m = 0.5 * (a + b);
        if ed_geo::sun::sun_position(lat, lon, m).elevation_deg > elev_deg {
            a = m;
        } else {
            b = m;
        }
    }
    0.5 * (a + b)
}

/// Lookdev only: apply each cloud shot's weather, time and storm placement
/// when it becomes active, and restore the stage defaults afterwards.
pub fn apply_lookdev_presets(
    shot: Option<Res<crate::lookdev::ActiveShot>>,
    origin: Res<WorldOrigin>,
    mut clock: ResMut<Clock>,
    mut over: ResMut<CloudOverride>,
    mut state: ResMut<CloudState>,
    mut saved_clock: Local<Option<f64>>,
) {
    let Some(shot) = shot else { return };
    if !shot.is_changed() {
        return;
    }
    let Some(preset) = preset_for(&shot.0) else {
        if over.params.is_some() || over.storms.is_some() {
            *over = CloudOverride::default();
        }
        if let Some(t) = saved_clock.take() {
            clock.unix_s = t;
        }
        return;
    };
    let def = LOOKDEV_SHOTS.iter().find(|s| s.name == shot.0).expect("preset without a shot");
    // Same cloud field every run: no wind drift carried over from the clock
    // jumps between shots.
    state.drift_ecef = DVec3::ZERO;
    state.storm_drift_deg = [0.0; 2];
    state.last_clock = None;
    let stage = origin.frame.origin;
    saved_clock.get_or_insert(clock.unix_s);
    clock.unix_s = match preset.time {
        ShotTime::Hour(h) => unix_at_solar_hour(stage.lon_deg, h),
        ShotTime::SunElevation(e) => unix_at_sun_elevation(stage.lat_deg, stage.lon_deg, e),
    };
    let w = wc::weather_preset(preset.weather).expect("known preset");
    let mut params = CloudParams::from_conditions(&Conditions::current(&w));
    (preset.tweak)(&mut params);
    over.storms = Some(
        preset
            .storm_at_m
            .map(|d| {
                let at = stage.destination(def.heading_deg as f64, d);
                vec![StormCell { lat_deg: at.lat_deg, lon_deg: at.lon_deg, radius_m: 5_500.0, top_m: 12_000.0, strength: 1.0 }]
            })
            .unwrap_or_default(),
    );
    if preset.storm_at_m.is_some() {
        params.storm = 1.0;
    }
    over.params = Some(params);
}
