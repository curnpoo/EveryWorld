//! The world map: a smooth satellite globe.
//!
//! A private 3D scene (render layer 1) holds the Earth as a quadtree of EOX
//! Sentinel-2 tiles wrapped on a unit sphere, a ray-marched atmosphere, a star
//! sphere, the Sun, Moon and planets. A camera renders it into an image that
//! any `MapCanvas` UI node shows. Tile choice is `ed_world::globe`; this file
//! is only Bevy glue: input, camera, streaming, overlays.
//!
//! Globe space: radius 1, +Y north, lon 0 on +X, 90°E on -Z.

use crate::mapview::{MapCanvas, MapState, Marker, Popup, PopupTitle, SearchBar, SearchPin, SearchRing, build_overlays, scroll_notches};
use crate::menu::GameState;
use crate::sky::Clock;
use bevy::asset::{RenderAssetUsages, embedded_asset};
use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::camera::{Exposure, Hdr, RenderTarget};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::mesh::{MeshVertexBufferLayoutRef, PrimitiveTopology};
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey, MaterialPlugin};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, CompareFunction, Extent3d, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError, TextureDimension, TextureFormat,
};
use bevy::shader::ShaderRef;
use bevy::tasks::{IoTaskPool, block_on, futures_lite::future};
use bevy::text::FontSize;
use bevy::ui::{ComputedNode, RelativeCursorPosition};
use crossbeam_channel::{Receiver, Sender};
use ed_data::satellite;
use ed_geo::Geodetic;
use ed_geo::tile::TileId;
use ed_space::{BODIES, BodyId, Snapshot};
use ed_world::globe as gl;
use ed_world::ground_tex::mip_chain;
use ed_world::maprender::PlaceClass;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

/// Render layer of the globe camera: the main world camera (`camera.rs`) and the sun's shadow
/// layers never see it, so the native Metal mirror can skip these entities entirely.
pub const GLOBE_LAYER: usize = 1;
const FOV: f32 = 40.0 * std::f32::consts::PI / 180.0;
const MIN_ZOOM: f32 = -4.6;
/// Zoom at which the whole disc plus atmosphere fits the 40 deg FOV with a margin
/// (camera ~3.4 Earth radii out; the disc spans ~34 deg). Default map view.
pub const WHOLE_EARTH_ZOOM: f64 = 0.4;
/// Zoom used when the map opens from the pause menu: ~320 km up, enough to
/// see the region around the player without losing them on a whole planet.
pub const PAUSE_ZOOM: f64 = 6.0;
const MAX_ZOOM: f32 = 12.4;
/// Largest tilt of the map camera away from straight down, in radians.
const MAX_TILT: f32 = 0.72;
/// Map zoom (for `MapState::focus`) that frames an angular span of `span_deg`
/// across the canvas, with about half the screen of margin on each side.
pub fn zoom_for_span(span_deg: f64) -> f64 {
    let d = (span_deg.to_radians() / (2.0 * (FOV * 0.5).tan() as f64) / 0.55).max(1e-5);
    (3.2 / d).log2().clamp(MIN_ZOOM as f64, MAX_ZOOM as f64)
}
const EARTH_RADIUS_M: f64 = 6.371e6;
const SKY_RADIUS: f32 = 2400.0;
const BODY_DIST: f32 = 900.0;
const MAX_IN_FLIGHT: usize = 6;
const MAX_CACHED: usize = 380;
/// Radial offset per quadtree level so overlapping coarse/fine tiles never z-fight.
const LEVEL_BIAS: f32 = 1.5e-6;

/// The globe's input system; other systems that read the same pad press order against it.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct GlobeControl;

pub struct GlobePlugin;

impl Plugin for GlobePlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "globe_atmo.wesl");
        app.add_plugins(MaterialPlugin::<AtmoMaterial>::default())
            .init_resource::<PlayerPin>()
            .add_systems(Startup, setup_globe)
            .add_systems(
                Update,
                (
                    track_player,
                    attach_canvas,
                    resize_target,
                    fly_to_requests,
                    control.in_set(GlobeControl),
                    update_camera,
                    update_space,
                    stream_tiles,
                    receive_tiles,
                    apply_tiles,
                    update_overlays,
                    update_labels,
                )
                    .chain(),
            );
    }
}

/// The camera that renders the globe. Every other 3D system must skip it.
#[derive(Component)]
pub struct GlobeCamera;

#[derive(Component)]
pub struct GlobeSun;

#[derive(Component)]
struct SkySphere;

#[derive(Component)]
struct SpaceBody(BodyId);

#[derive(Component)]
struct TileEntity;

#[derive(Component)]
struct PinRoot;

#[derive(Component)]
struct PinRing;

#[derive(Component)]
struct Readout;

/// Where the player is, once they have spawned: place and compass heading.
#[derive(Resource, Default)]
struct PlayerPin {
    pos: Option<(Geodetic, f32)>,
}

// ---- atmosphere material ----------------------------------------------------

#[derive(Clone, Copy, Default, ShaderType)]
struct AtmoUniform {
    sun: Vec4,
    shape: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
struct AtmoMaterial {
    #[uniform(0)]
    u: AtmoUniform,
}

impl Material for AtmoMaterial {
    fn vertex_shader() -> ShaderRef {
        "embedded://earthdrop/globe_atmo.wesl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "embedded://earthdrop/globe_atmo.wesl".into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }
    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        if let Some(ds) = descriptor.depth_stencil.as_mut() {
            ds.depth_compare = Some(CompareFunction::Always);
            ds.depth_write_enabled = Some(false);
        }
        Ok(())
    }
}

// ---- state ------------------------------------------------------------------

struct Slot {
    mesh: Handle<Mesh>,
    mat: Handle<StandardMaterial>,
    last_used: u64,
    places: Vec<Place>,
}

/// A named place from the tile's vector data, positioned on the unit globe.
struct Place {
    name: String,
    class: PlaceClass,
    rank: u32,
    pos: Vec3,
}

struct Fetched {
    id: TileId,
    data: Option<(u32, Vec<u8>, u32, Vec<Place>)>,
}

/// One text node of the per-canvas label pool.
#[derive(Component)]
struct PlaceLabel;

const LABEL_POOL: usize = 64;

#[derive(Resource)]
struct GlobeState {
    // view
    focus: Vec3,
    zoom: f32,
    goal_zoom: f32,
    fly: Option<(Vec3, f32)>,
    vel: Vec2,
    /// Cursor position (logical px in the canvas) last frame, for drag deltas.
    last_px: Option<Vec2>,
    press: Option<Vec2>,
    moved: f32,
    // camera as last placed (for picking and overlay projection)
    cam_pos: Vec3,
    cam_rot: Quat,
    aspect: f32,
    active: bool,
    cursor_pick: Option<Vec3>,
    // render target
    target: Handle<Image>,
    target_px: UVec2,
    resize_cool: f32,
    camera: Entity,
    // tiles
    loaded: HashMap<TileId, Slot>,
    pending: HashSet<TileId>,
    failed: HashSet<TileId>,
    shown: HashMap<TileId, Entity>,
    frame: u64,
    wanted_best: Vec<TileId>,
    tx: Sender<Fetched>,
    rx: Mutex<Receiver<Fetched>>,
    // space
    atmo: Handle<AtmoMaterial>,
    stars: Option<bevy::tasks::Task<Vec<u8>>>,
    sky_mat: Handle<StandardMaterial>,
    sun_dir: Vec3,
}

impl GlobeState {
    fn dist(&self) -> f32 {
        1.0 + 3.2 * 2f32.powf(-self.zoom)
    }
    fn tan_half(&self) -> f32 {
        (FOV * 0.5).tan()
    }
    /// Unit ray through a point of the canvas (-0.5..0.5, y down).
    fn ray(&self, n: Vec2) -> Vec3 {
        let t = self.tan_half();
        (self.cam_rot * Vec3::new(n.x * 2.0 * t * self.aspect, -n.y * 2.0 * t, -1.0)).normalize()
    }
    /// Canvas fraction (0..1, y down) of a globe point, if in front of the camera.
    fn project(&self, p: Vec3) -> Option<Vec2> {
        let v = self.cam_rot.inverse() * (p - self.cam_pos);
        if v.z >= -1e-6 {
            return None;
        }
        let t = self.tan_half();
        let x = v.x / -v.z / (t * self.aspect);
        let y = v.y / -v.z / t;
        Some(Vec2::new(x * 0.5 + 0.5, 0.5 - y * 0.5))
    }
    fn facing(&self, p: Vec3) -> bool {
        p.dot(self.cam_pos - p) > 0.0
    }
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn to_globe(v: glam::DVec3) -> Vec3 {
    Vec3::new(v.x as f32, v.z as f32, -v.y as f32)
}

// ---- setup ------------------------------------------------------------------

fn target_image(w: u32, h: u32) -> Image {
    Image::new_target_texture(w, h, TextureFormat::Rgba8UnormSrgb, None)
}

#[allow(clippy::too_many_arguments)]
fn setup_globe(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut atmo_mats: ResMut<Assets<AtmoMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let layers = RenderLayers::layer(GLOBE_LAYER);
    let target = images.add(target_image(1280, 720));
    let camera = commands
        .spawn((
            GlobeCamera,
            Camera3d::default(),
            Camera { is_active: false, order: -5, clear_color: ClearColorConfig::Custom(Color::BLACK), ..default() },
            RenderTarget::Image(target.clone().into()),
            Hdr,
            Projection::Perspective(PerspectiveProjection { fov: FOV, near: 1e-4, far: 6000.0, ..default() }),
            Tonemapping::TonyMcMapface,
            Exposure { ev100: 12.0 },
            Bloom::NATURAL,
            Msaa::Off,
            AmbientLight { color: Color::srgb(0.55, 0.65, 1.0), brightness: 500.0, ..default() },
            Transform::from_xyz(0.0, 0.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
            layers.clone(),
        ))
        .id();
    commands.spawn((
        GlobeSun,
        DirectionalLight { illuminance: 26_000.0, ..default() },
        Transform::default().looking_to(Vec3::NEG_X, Vec3::Y),
        layers.clone(),
    ));

    // Deep-ocean base under the tiles, so holes and offline tiles stay dark blue.
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.9996).mesh().uv(96, 48))),
        MeshMaterial3d(mats.add(StandardMaterial { base_color: Color::srgb(0.02, 0.07, 0.16), perceptual_roughness: 0.35, ..default() })),
        layers.clone(),
        NotShadowCaster,
    ));
    // Polar caps beyond the Web-Mercator limit.
    for (north, col) in [(true, Color::srgb(0.80, 0.86, 0.92)), (false, Color::srgb(0.90, 0.93, 0.96))] {
        commands.spawn((
            Mesh3d(meshes.add(crate::meshconv::to_bevy_mesh(&gl::polar_cap(north, 96, 6)))),
            MeshMaterial3d(mats.add(StandardMaterial { base_color: col, perceptual_roughness: 0.6, cull_mode: None, ..default() })),
            layers.clone(),
            NotShadowCaster,
        ));
    }

    // Atmosphere shell.
    let atmo = atmo_mats.add(AtmoMaterial { u: AtmoUniform { sun: Vec4::new(1.0, 0.0, 0.0, 3.0), shape: Vec4::new(1.075, 0.011, 0.0, 0.0) } });
    commands.spawn((
        Mesh3d(meshes.add(
            Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
                .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![[-1.0f32, -1.0, 0.0], [3.0, -1.0, 0.0], [-1.0, 3.0, 0.0]]),
        )),
        MeshMaterial3d(atmo.clone()),
        Transform::default(),
        NoFrustumCulling,
        NotShadowCaster,
        layers.clone(),
    ));

    // Stars: generated off-thread, assigned when ready.
    let (w, h) = (4096usize, 2048usize);
    let stars = bevy::tasks::AsyncComputeTaskPool::get().spawn(async move { ed_world::starfield::generate(w, h) });
    let sky_mat = mats.add(StandardMaterial { base_color: Color::BLACK, unlit: true, cull_mode: None, ..default() });
    commands.spawn((
        SkySphere,
        Mesh3d(meshes.add(sky_sphere(96, 48))),
        MeshMaterial3d(sky_mat.clone()),
        Transform::from_scale(Vec3::splat(SKY_RADIUS)),
        NoFrustumCulling,
        NotShadowCaster,
        NotShadowReceiver,
        layers.clone(),
    ));

    // Sun, Moon and planets. The Moon is true scale and distance; the rest sit
    // on a distant shell with an exaggerated angular size so they read as discs.
    for b in BODIES.iter().filter(|b| b.id != BodyId::Earth) {
        let c = Color::srgb_u8((b.color >> 16) as u8, (b.color >> 8) as u8, b.color as u8);
        let (mesh, mat) = match b.id {
            BodyId::Moon => (
                Sphere::new((b.radius_m / EARTH_RADIUS_M) as f32).mesh().uv(48, 24),
                StandardMaterial { base_color: c, perceptual_roughness: 0.95, reflectance: 0.05, ..default() },
            ),
            BodyId::Sun => (
                Sphere::new(1.0).mesh().uv(32, 16),
                StandardMaterial { base_color: Color::linear_rgb(14.0, 12.0, 9.0), unlit: true, ..default() },
            ),
            _ => (Sphere::new(1.0).mesh().uv(24, 12), {
                let l = c.to_linear();
                StandardMaterial { base_color: Color::linear_rgb(l.red * 2.5, l.green * 2.5, l.blue * 2.5), unlit: true, ..default() }
            }),
        };
        commands.spawn((
            SpaceBody(b.id),
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(mats.add(mat)),
            Transform::default(),
            NoFrustumCulling,
            NotShadowCaster,
            NotShadowReceiver,
            layers.clone(),
        ));
    }

    let (tx, rx) = crossbeam_channel::unbounded();
    let focus = gl::unit_from_latlon(22.0, 12.0);
    commands.insert_resource(GlobeState {
        focus,
        zoom: WHOLE_EARTH_ZOOM as f32,
        goal_zoom: WHOLE_EARTH_ZOOM as f32,
        fly: None,
        vel: Vec2::ZERO,
        last_px: None,
        press: None,
        moved: 0.0,
        cam_pos: Vec3::new(0.0, 0.0, 4.0),
        cam_rot: Quat::IDENTITY,
        aspect: 16.0 / 9.0,
        active: false,
        cursor_pick: None,
        target,
        target_px: UVec2::new(1280, 720),
        resize_cool: 0.0,
        camera,
        loaded: HashMap::new(),
        pending: HashSet::new(),
        failed: HashSet::new(),
        shown: HashMap::new(),
        frame: 0,
        wanted_best: Vec::new(),
        tx,
        rx: Mutex::new(rx),
        atmo,
        stars: Some(stars),
        sky_mat,
        sun_dir: Vec3::X,
    });
}

/// Inverted UV sphere whose UVs are equatorial (RA, Dec) for the star texture.
/// Positions are in globe axes but *not* spun by sidereal time; the entity is.
fn sky_sphere(seg: usize, rings: usize) -> Mesh {
    let mut pos = Vec::new();
    let mut uv = Vec::new();
    let mut idx = Vec::new();
    for j in 0..=rings {
        let dec = 90.0 - 180.0 * j as f32 / rings as f32;
        for i in 0..=seg {
            let ra = 360.0 * i as f32 / seg as f32;
            let d = ed_world::starfield::eci_dir(ra, dec);
            pos.push([d.x, d.z, -d.y]);
            uv.push([i as f32 / seg as f32, j as f32 / rings as f32]);
        }
    }
    let row = seg as u32 + 1;
    for j in 0..rings as u32 {
        for i in 0..seg as u32 {
            let (a, b, c, d) = (j * row + i, j * row + i + 1, (j + 1) * row + i, (j + 1) * row + i + 1);
            idx.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }
    let normals: Vec<[f32; 3]> = pos.iter().map(|p| [-p[0], -p[1], -p[2]]).collect();
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
        .with_inserted_indices(bevy::mesh::Indices::U32(idx))
}

// ---- player ------------------------------------------------------------------

fn track_player(
    state: Res<State<GameState>>,
    origin: Res<crate::origin::WorldOrigin>,
    cams: Query<&Transform, (With<Camera3d>, Without<GlobeCamera>)>,
    mut pin: ResMut<PlayerPin>,
) {
    if *state.get() != GameState::Playing {
        return;
    }
    let Ok(cam) = cams.single() else { return };
    let g = origin.to_geodetic(cam.translation);
    let up = origin.up_at(cam.translation);
    let fwd = cam.forward().as_vec3().reject_from_normalized(up);
    let heading = if fwd.length_squared() > 1e-6 {
        let enu = ed_geo::render_to_enu(fwd.normalize().as_dvec3());
        (enu.x.atan2(enu.y).to_degrees() as f32).rem_euclid(360.0)
    } else {
        pin.pos.map_or(0.0, |p| p.1)
    };
    pin.pos = Some((g, heading));
}

// ---- UI attachment --------------------------------------------------------------

fn attach_canvas(mut commands: Commands, canvases: Query<Entity, Added<MapCanvas>>, g: Res<GlobeState>) {
    for c in &canvases {
        commands.spawn((
            Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
            ImageNode::new(g.target.clone()),
            ZIndex(0),
            Pickable::IGNORE,
            ChildOf(c),
        ));
        build_overlays(&mut commands, c);
        // Player pin: the minimap's dot and needle, rotated to the heading.
        commands
            .spawn((
                PinRoot,
                Node { position_type: PositionType::Absolute, width: Val::Px(44.0), height: Val::Px(44.0), ..default() },
                UiTransform::IDENTITY,
                Visibility::Hidden,
                ZIndex(2),
                Pickable::IGNORE,
                ChildOf(c),
            ))
            .with_children(|p| {
                p.spawn((
                    PinRing,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(8.0),
                        top: Val::Px(8.0),
                        width: Val::Px(28.0),
                        height: Val::Px(28.0),
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::MAX,
                        ..default()
                    },
                    BorderColor::all(Color::srgba(0.98, 0.42, 0.1, 0.8)),
                    UiTransform::IDENTITY,
                ));
                p.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(16.0),
                        top: Val::Px(16.0),
                        width: Val::Px(12.0),
                        height: Val::Px(12.0),
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::MAX,
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.98, 0.42, 0.1)),
                    BorderColor::all(Color::WHITE),
                    ZIndex(1),
                ));
                p.spawn((
                    Text::new("▲"),
                    TextFont { font_size: FontSize::Px(15.0), ..default() },
                    TextColor(Color::WHITE),
                    Node { position_type: PositionType::Absolute, left: Val::Px(16.0), top: Val::Px(-2.0), ..default() },
                    ZIndex(2),
                ));
            });
        // Place-name pool, filled each frame by `update_labels`.
        for _ in 0..LABEL_POOL {
            commands.spawn((
                PlaceLabel,
                Text::new(""),
                TextFont { font_size: FontSize::Px(12.0), ..default() },
                TextColor(Color::WHITE),
                TextShadow { offset: Vec2::new(0.0, 1.0), color: Color::srgba(0.0, 0.0, 0.0, 0.85) },
                TextLayout::no_wrap(),
                Node { position_type: PositionType::Absolute, ..default() },
                Visibility::Hidden,
                ZIndex(1),
                Pickable::IGNORE,
                ChildOf(c),
            ));
        }
        // Readout and hint.
        commands.spawn((
            Readout,
            Text::new(""),
            TextFont { font_size: FontSize::Px(12.0), ..default() },
            TextColor(Color::srgba(1.0, 1.0, 1.0, 0.75)),
            Node { position_type: PositionType::Absolute, left: Val::Px(14.0), bottom: Val::Px(10.0), ..default() },
            ZIndex(1),
            Pickable::IGNORE,
            ChildOf(c),
        ));
        commands.spawn((
            Text::new("Drag to spin  ·  Scroll to zoom  ·  Click to pick a place"),
            TextFont { font_size: FontSize::Px(12.0), ..default() },
            TextColor(Color::srgba(1.0, 1.0, 1.0, 0.4)),
            Node { position_type: PositionType::Absolute, right: Val::Px(14.0), bottom: Val::Px(10.0), ..default() },
            ZIndex(1),
            Pickable::IGNORE,
            ChildOf(c),
        ));
    }
}

/// The visible canvas, if any: its logical size and cursor state.
fn active_canvas<'a>(q: impl Iterator<Item = (&'a RelativeCursorPosition, &'a ComputedNode, &'a InheritedVisibility)>) -> Option<(Vec2, Vec2, Option<Vec2>, bool)> {
    for (rel, node, vis) in q {
        if vis.get() && node.size().x > 8.0 {
            let logical = node.size() * node.inverse_scale_factor();
            return Some((logical, node.size(), rel.normalized, rel.cursor_over()));
        }
    }
    None
}

fn resize_target(
    time: Res<Time>,
    mut g: ResMut<GlobeState>,
    canvases: Query<(&RelativeCursorPosition, &ComputedNode, &InheritedVisibility), With<MapCanvas>>,
    mut images: ResMut<Assets<Image>>,
    mut cams: Query<(&mut Camera, &mut RenderTarget), With<GlobeCamera>>,
    mut nodes: Query<&mut ImageNode>,
) {
    g.resize_cool -= time.delta_secs();
    let canvas = active_canvas(canvases.iter());
    g.active = canvas.is_some();
    if let Ok((mut cam, mut rt)) = cams.single_mut() {
        if cam.is_active != g.active {
            cam.is_active = g.active;
        }
        let Some((_, phys, _, _)) = canvas else { return };
        // Render at 1.5x for smooth edges; the UI filters it back down.
        // Scale uniformly so the cap never changes the aspect (that pillarboxed the map).
        let s = (1.5f32).min(3000.0 / phys.max_element().max(1.0));
        let want = (phys * s).as_uvec2().max(UVec2::splat(64));
        let cur = g.target_px;
        if (want.x.abs_diff(cur.x) > 3 || want.y.abs_diff(cur.y) > 3) && g.resize_cool <= 0.0 {
            let handle = images.add(target_image(want.x, want.y));
            *rt = RenderTarget::Image(handle.clone().into());
            for mut n in &mut nodes {
                if n.image == g.target {
                    n.image = handle.clone();
                }
            }
            g.target = handle;
            g.target_px = want;
            g.resize_cool = 0.15;
        }
        g.aspect = g.target_px.x as f32 / g.target_px.y as f32;
    }
}

// ---- input and camera -------------------------------------------------------------


/// Aim the clock at the hour the player has chosen (3 pm by default) for the
/// place they just picked: the globe's sun swings round so that spot is in
/// that daylight, and the drop-in keeps the same hour.
fn retarget_time(map: &mut MapState, gfx: &mut crate::menu::GraphicsSettings, clock: &crate::sky::Clock, dest_lon: f64) {
    // A live clock is absolute: leave it on real time.
    if gfx.time_live {
        return;
    }
    let hour = gfx.solar_hour;
    // The absolute time at which `dest_lon` sees local solar hour `hour`.
    let target = ed_geo::sun::unix_at_solar_hour(clock.unix_s, dest_lon, hour);
    let mut delta = (target - clock.unix_s) / 3600.0;
    // Shortest way round the dial.
    delta -= 24.0 * (delta / 24.0).round();
    gfx.time_live = false;
    map.time_target_offset = Some(gfx.hour_offset + delta);
}

fn fly_to_requests(mut map: ResMut<MapState>, mut g: ResMut<GlobeState>, clock: Res<crate::sky::Clock>, mut gfx: ResMut<crate::menu::GraphicsSettings>) {
    if let Some((place, zoom)) = map.focus_req.take() {
        let goal = gl::unit_from_latlon(place.lat_deg, place.lon_deg);
        g.fly = Some((goal, (zoom as f32).clamp(MIN_ZOOM, MAX_ZOOM)));
        g.vel = Vec2::ZERO;
        // The searched place gets the chosen hour, like a click does.
        retarget_time(&mut map, &mut gfx, &clock, place.lon_deg);
    }
}

#[allow(clippy::too_many_arguments)]
fn control(
    time: Res<Time>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    pad: Res<crate::gamepad::Pad>,
    mode: Res<crate::gamepad::InputMode>,
    menu_focus: Res<crate::menu::MenuFocus>,
    canvases: Query<(&RelativeCursorPosition, &ComputedNode, &InheritedVisibility), With<MapCanvas>>,
    popups: Query<(&RelativeCursorPosition, &InheritedVisibility), (Or<(With<Popup>, With<SearchBar>)>, Without<MapCanvas>)>,
    mut map: ResMut<MapState>,
    mut g: ResMut<GlobeState>,
    clock: Res<crate::sky::Clock>,
    mut gfx: ResMut<crate::menu::GraphicsSettings>,
) {
    let dt = time.delta_secs().min(0.05);
    let Some((size, _, norm, over)) = active_canvas(canvases.iter()) else {
        g.press = None;
        g.last_px = None;
        return;
    };
    // With a controller the canvas counts as hovered and the pick is the centre crosshair.
    let pad_mode = *mode == crate::gamepad::InputMode::Controller || pad.a_just || pad.b_just;
    let over = over || pad_mode;
    // Cursor movement in logical pixels since last frame. Prefer the cursor
    // position (works for trackpads, tablets and synthetic events, and is in
    // the same units as the canvas); fall back to raw motion if the cursor is
    // unknown (e.g. it left the window mid-drag).
    let cur_px = norm.map(|n| n * size);
    let cursor_delta = match (cur_px, g.last_px) {
        (Some(a), Some(b)) => a - b,
        _ => motion.delta,
    };
    g.last_px = cur_px;
    // Bevy stops updating RelativeCursorPosition on hidden nodes, so a popup
    // hidden while hovered (e.g. after "Drop from the sky") keeps a stale
    // `cursor_over = true` forever. Only a visible popup may block the globe.
    let over_popup = popups.iter().any(|(p, vis)| vis.get() && p.cursor_over());
    let hover = over && !over_popup;
    // `ray` takes canvas offsets from the centre (-0.5..0.5), so the crosshair is ZERO.
    let aim = if pad_mode { Some(Vec2::ZERO) } else { norm };
    g.cursor_pick = aim.filter(|_| hover).and_then(|n| gl::ray_sphere(g.cam_pos, g.ray(n)));

    // Drag to spin.
    if buttons.just_pressed(MouseButton::Left) && hover {
        g.press = Some(norm.unwrap_or_default());
        g.moved = 0.0;
        g.vel = Vec2::ZERO;
        g.fly = None;
    }
    let dragging = g.press.is_some() && buttons.pressed(MouseButton::Left);
    let d = g.dist();
    let ppr = size.y / (2.0 * g.tan_half());
    if dragging {
        let px = cursor_delta;
        g.moved += px.length();
        // Move the surface with the cursor: screen pixels -> radians of arc.
        let ang = px * ((d - 1.0).max(1e-4) / ppr);
        let (east, north) = (gl::east_at(g.focus), gl::north_at(g.focus));
        let step = (-east * ang.x + north * ang.y) * 1.0;
        g.focus = (g.focus + step).normalize();
        if dt > 0.0 {
            let v = Vec2::new(-ang.x, ang.y) / dt;
            g.vel = g.vel.lerp(v, 0.35);
        }
    } else {
        if buttons.just_released(MouseButton::Left) {
            if let Some(start) = g.press.take() {
                let end = norm.unwrap_or(start);
                if g.moved < 5.0 && hover && (end - start).length() < 0.02 {
                    if let Some(p) = g.cursor_pick {
                        let (lat, lon) = gl::latlon_from_unit(p);
                        map.select(Geodetic::new(lat, lon, 0.0));
                        retarget_time(&mut map, &mut gfx, &clock, lon);
                        // Bring the picked place to the middle of the view.
                        g.fly = Some((p, g.goal_zoom));
                    }
                }
            }
        } else {
            g.press = None;
        }
        // Inertia.
        if g.vel.length_squared() > 1e-8 {
            let (east, north) = (gl::east_at(g.focus), gl::north_at(g.focus));
            let v = g.vel;
            g.focus = (g.focus + (east * v.x + north * v.y) * dt).normalize();
            g.vel *= (-dt * 3.0).exp();
            }
    }

    // Any controller steering cancels a pending fly-to, so the map never snaps back to the player.
    if pad_mode && (pad.left != Vec2::ZERO || pad.right != Vec2::ZERO || pad.a_just) {
        g.fly = None;
    }
    // Controller: left stick pans like a drag, right stick zooms, A picks the crosshair.
    let mut pad_zoom_in = false;
    if pad_mode && hover && !dragging {
        if pad.left != Vec2::ZERO {
            // Pan by a fraction of the visible width, so it feels the same at any height.
            let span = 2.0 * (d - 1.0).max(1e-3) * g.tan_half();
            let step = crate::gamepad::PAD_PAN_SCREENS_PER_S * dt * span;
            let (east, north) = (gl::east_at(g.focus), gl::north_at(g.focus));
            g.focus = (g.focus + (east * pad.left.x + north * pad.left.y) * step).normalize();
            g.vel = Vec2::ZERO;
            g.fly = None;
        }
        if pad.right.y != 0.0 {
            g.goal_zoom = (g.goal_zoom + pad.right.y * crate::gamepad::PAD_ZOOM_LEVELS_PER_S * dt).clamp(MIN_ZOOM, MAX_ZOOM);
            g.fly = None;
            pad_zoom_in = pad.right.y > 0.0;
        }
    }
    // While the popup is open, `mapview::pad_popup` owns A / B / D-pad.
    let popup_open = map.selected.is_some();
    // Otherwise A picks the spot under the crosshair.
    // A presses a focused menu button instead, when the D-pad is on one.
    if pad_mode && pad.a_just && hover && !popup_open && menu_focus.0.is_none() {
        if let Some(p) = g.cursor_pick {
            let (lat, lon) = gl::latlon_from_unit(p);
            map.select(Geodetic::new(lat, lon, 0.0));
            retarget_time(&mut map, &mut gfx, &clock, lon);
            g.fly = Some((p, g.goal_zoom));
        }
    }

    // Wheel zoom, towards the cursor.
    let notches = if hover { scroll_notches(&scroll) } else { 0.0 };
    if notches != 0.0 {
        g.goal_zoom = (g.goal_zoom + notches * 0.45).clamp(MIN_ZOOM, MAX_ZOOM);
        g.fly = None;
    }
    let before = g.zoom;
    let k = 1.0 - (-dt * 7.0).exp();
    g.zoom += (g.goal_zoom - g.zoom) * k;
    let dz = g.zoom - before;
    if dz > 0.0 && (notches > 0.0 || pad_zoom_in) {
        if let Some(p) = g.cursor_pick {
            // Walk the view all the way onto the spot under the cursor, so
            // the place you zoom into ends up in the middle of the screen.
            let w = (dz * 1.8).clamp(0.0, 1.0);
            g.focus = g.focus.lerp(p, w).normalize();
        }
    }

    // Fly-to.
    if let Some((goal, zoom)) = g.fly {
        let kf = 1.0 - (-dt * 1.5).exp();
        g.focus = g.focus.lerp(goal, kf).normalize();
        g.goal_zoom += (zoom - g.goal_zoom) * kf;
        if g.focus.dot(goal) > 0.999_999 && (g.goal_zoom - zoom).abs() < 0.01 {
            g.goal_zoom = zoom;
            g.fly = None;
        }
    }
}

fn update_camera(mut g: ResMut<GlobeState>, mut cams: Query<(&mut Transform, &mut Projection), With<GlobeCamera>>) {
    let Ok((mut tf, mut proj)) = cams.single_mut() else { return };
    let h = g.dist() - 1.0;
    // Tip the view towards the horizon as we get close, so mountains and
    // coastlines have some depth. The tip eases off again at the deepest
    // zoom, where looking straight down at a place reads better.
    let mut tilt = smoothstep(4.5, 9.0, g.zoom) * MAX_TILT;
    tilt *= 1.0 - 0.4 * smoothstep(10.0, 12.4, g.zoom);
    let w = (tilt / MAX_TILT).min(1.0);
    let focus = g.focus;
    let north = gl::north_at(focus);
    let pos = focus + (focus * tilt.cos() - north * tilt.sin()) * h;
    let up = (north * (1.0 - w) + focus * w).normalize();
    *tf = Transform::from_translation(pos).looking_at(focus, up);
    g.cam_pos = pos;
    g.cam_rot = tf.rotation;
    if let Projection::Perspective(p) = &mut *proj {
        p.near = (h * 0.04).clamp(2e-6, 0.05);
    }
}

// ---- sun, moon, planets, stars -------------------------------------------------------

fn update_space(
    clock: Res<Clock>,
    mut g: ResMut<GlobeState>,
    mut atmo_mats: ResMut<Assets<AtmoMaterial>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut sun: Query<&mut Transform, (With<GlobeSun>, Without<SkySphere>, Without<SpaceBody>)>,
    mut sky: Query<&mut Transform, (With<SkySphere>, Without<SpaceBody>, Without<GlobeSun>)>,
    mut bodies: Query<(&SpaceBody, &mut Transform), (Without<SkySphere>, Without<GlobeSun>)>,
) {
    // Star texture arrives once.
    if let Some(task) = g.stars.as_mut() {
        if let Some(data) = block_on(future::poll_once(task)) {
            let mut img = Image::new(
                Extent3d { width: 4096, height: 2048, depth_or_array_layers: 1 },
                TextureDimension::D2,
                data,
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::RENDER_WORLD,
            );
            img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                address_mode_u: ImageAddressMode::Repeat,
                address_mode_v: ImageAddressMode::ClampToEdge,
                mag_filter: ImageFilterMode::Linear,
                min_filter: ImageFilterMode::Linear,
                ..default()
            });
            if let Some(mut m) = mats.get_mut(&g.sky_mat) {
                m.base_color = Color::linear_rgb(1.6, 1.6, 1.6);
                m.base_color_texture = Some(images.add(img));
            }
            g.stars = None;
        }
    }
    if !g.active {
        return;
    }
    let snap = Snapshot::at(clock.unix_s);
    let to_unit = |id: BodyId| to_globe(snap.ecef(id));
    let sun_dir = to_unit(BodyId::Sun).normalize_or_zero();
    if sun_dir != Vec3::ZERO {
        g.sun_dir = sun_dir;
    }
    let sd = g.sun_dir;
    if let Ok(mut t) = sun.single_mut() {
        let up = if sd.y.abs() > 0.99 { Vec3::X } else { Vec3::Y };
        *t = Transform::default().looking_to(-sd, up);
    }
    if let Some(mut m) = atmo_mats.get_mut(&g.atmo) {
        m.u.sun = Vec4::new(sd.x, sd.y, sd.z, 3.0);
    }
    if let Ok(mut t) = sky.single_mut() {
        // Inertial -> Earth-fixed is a rotation of -GMST about the pole.
        t.rotation = Quat::from_rotation_y(-ed_space::gmst(clock.unix_s) as f32);
    }
    for (b, mut t) in &mut bodies {
        let p = to_unit(b.0);
        let dir = p.normalize_or_zero();
        let info = BODIES.iter().find(|i| i.id == b.0);
        match b.0 {
            BodyId::Moon => {
                t.translation = p / EARTH_RADIUS_M as f32;
                t.rotation = Quat::IDENTITY;
            }
            _ => {
                // Real size, but never smaller than a readable disc.
                let real = info.map_or(0.0, |i| (i.radius_m as f32 / p.length()).atan());
                let min = if b.0 == BodyId::Sun { 0.016 } else { 0.0042 };
                let ang = real.max(min);
                t.translation = dir * BODY_DIST;
                t.scale = Vec3::splat(BODY_DIST * ang.tan());
            }
        }
    }
}

// ---- tile streaming -------------------------------------------------------------------

fn level_mesh_n(z: u8) -> usize {
    match z {
        0..=1 => 64,
        2..=3 => 32,
        4..=6 => 16,
        _ => 8,
    }
}

fn stream_tiles(mut g: ResMut<GlobeState>, cfg: Res<crate::config::Config>) {
    if !g.active {
        return;
    }
    g.frame += 1;
    let frame = g.frame;
    let cam = g.cam_pos;
    let view = (g.focus - cam).normalize();
    let ppr = g.target_px.y as f32 / (2.0 * g.tan_half());
    let mut texel = 0.85;
    let mut wanted = gl::select_tiles(cam, view, FOV * 0.5 * g.aspect.max(1.0), ppr, satellite::MAX_ZOOM, texel);
    for _ in 0..4 {
        if wanted.len() <= 240 {
            break;
        }
        texel *= 1.4;
        wanted = gl::select_tiles(cam, view, FOV * 0.5 * g.aspect.max(1.0), ppr, satellite::MAX_ZOOM, texel);
    }

    // For each wanted tile show the finest loaded ancestor; ask for the
    // coarsest missing one first so something always covers the area.
    let mut best: Vec<TileId> = Vec::new();
    let mut want_fetch: Vec<(u8, f32, TileId)> = Vec::new();
    let mut seen = HashSet::new();
    for t in &wanted {
        let c = gl::tile_center(*t);
        let dist = c.dot(g.focus);
        let mut chain = vec![*t];
        let mut cur = *t;
        while let Some(p) = cur.parent() {
            chain.push(p);
            cur = p;
        }
        chain.reverse(); // z = 0 first
        let mut deepest_loaded = None;
        for id in &chain {
            if g.loaded.contains_key(id) {
                deepest_loaded = Some(*id);
            } else if g.failed.contains(id) {
                break;
            } else {
                if !g.pending.contains(id) && seen.insert(*id) {
                    want_fetch.push((id.z, -dist, *id));
                }
                break;
            }
        }
        if let Some(id) = deepest_loaded {
            if !best.contains(&id) {
                best.push(id);
            }
        }
    }
    for id in &best {
        if let Some(s) = g.loaded.get_mut(id) {
            s.last_used = frame;
        }
    }
    g.wanted_best = best;

    want_fetch.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
    for (_, _, id) in want_fetch {
        if g.pending.len() >= MAX_IN_FLIGHT {
            break;
        }
        g.pending.insert(id);
        let tx = g.tx.clone();
        let cache = cfg.cache.clone();
        IoTaskPool::get()
            .spawn(async move {
                let data = satellite::fetch(&cache, id).ok().map(|t| {
                    // Roads need more texels than the imagery once it is magnified.
                    let (sz, mut rgb) =
                        if id.z >= 10 { ed_world::maprender::upsample2(t.size, t.rgb()) } else { (t.size, t.rgb().to_vec()) };
                    let places = overlay_linework(&cache, id, sz, &mut rgb);
                    let size = sz as u32;
                    let mut rgba = Vec::with_capacity(rgb.len() / 3 * 4);
                    for p in rgb.chunks_exact(3) {
                        rgba.extend_from_slice(&[p[0], p[1], p[2], 255]);
                    }
                    let (chain, levels) = mip_chain(&rgba, size, true);
                    (size, chain, levels, places)
                });
                let _ = tx.send(Fetched { id, data });
            })
            .detach();
    }
}

/// Roads and borders burned into a satellite tile (worker thread), plus the
/// tile's place names. Missing vector data just leaves the plain imagery.
fn overlay_linework(cache: &ed_data::DiskCache, id: TileId, size: usize, rgb: &mut [u8]) -> Vec<Place> {
    static TEMPLATE: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    let Some(tpl) = TEMPLATE.get_or_init(|| ed_data::vector::tile_template(cache).ok()) else { return Vec::new() };
    let (anc, u0, v0, s) = id.ancestor(id.z.min(ed_data::vector::MAX_ZOOM));
    let Ok(vt) = ed_data::vector::fetch(cache, tpl, anc) else { return Vec::new() };
    ed_world::maprender::overlay_on_imagery(id, size, rgb, &vt, (u0, v0, s));
    ed_world::maprender::place_labels(&vt, (u0, v0, s))
        .into_iter()
        .map(|l| {
            let geo = id.uv_to_geodetic(l.u as f64, l.v as f64);
            Place { name: l.name, class: l.class, rank: l.rank, pos: gl::unit_from_latlon(geo.lat_deg, geo.lon_deg) }
        })
        .collect()
}

fn receive_tiles(
    mut g: ResMut<GlobeState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let batch: Vec<Fetched> = {
        let rx = g.rx.lock().unwrap();
        rx.try_iter().take(6).collect()
    };
    for f in batch {
        g.pending.remove(&f.id);
        let Some((size, data, levels, places)) = f.data else {
            g.failed.insert(f.id);
            continue;
        };
        let mut img = Image::new_uninit(
            Extent3d { width: size, height: size, depth_or_array_layers: 1 },
            TextureDimension::D2,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD,
        );
        img.data = Some(data);
        img.texture_descriptor.mip_level_count = levels;
        img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::ClampToEdge,
            address_mode_v: ImageAddressMode::ClampToEdge,
            mag_filter: ImageFilterMode::Linear,
            min_filter: ImageFilterMode::Linear,
            mipmap_filter: ImageFilterMode::Linear,
            anisotropy_clamp: 8,
            ..default()
        });
        let mesh = meshes.add(crate::meshconv::to_bevy_mesh(&gl::tile_mesh(f.id, level_mesh_n(f.id.z))));
        let mat = mats.add(StandardMaterial {
            base_color_texture: Some(images.add(img)),
            perceptual_roughness: 0.82,
            reflectance: 0.12,
            ..default()
        });
        let frame = g.frame;
        g.loaded.insert(f.id, Slot { mesh, mat, last_used: frame, places });
    }
}

fn apply_tiles(mut commands: Commands, mut g: ResMut<GlobeState>) {
    if !g.active {
        return;
    }
    let best: HashSet<TileId> = g.wanted_best.iter().copied().collect();
    // Retire tiles no longer chosen.
    let gone: Vec<TileId> = g.shown.keys().filter(|id| !best.contains(id)).copied().collect();
    for id in gone {
        if let Some(e) = g.shown.remove(&id) {
            commands.entity(e).despawn();
        }
    }
    // Show newly chosen ones.
    let add: Vec<TileId> = best.iter().filter(|id| !g.shown.contains_key(id)).copied().collect();
    for id in add {
        let Some(slot) = g.loaded.get(&id) else { continue };
        let e = commands
            .spawn((
                TileEntity,
                Mesh3d(slot.mesh.clone()),
                MeshMaterial3d(slot.mat.clone()),
                Transform::from_scale(Vec3::splat(1.0 + id.z as f32 * LEVEL_BIAS)),
                NoFrustumCulling,
                NotShadowCaster,
                RenderLayers::layer(GLOBE_LAYER),
            ))
            .id();
        g.shown.insert(id, e);
    }
    // Evict the least recently used when the cache is full.
    if g.loaded.len() > MAX_CACHED {
        let mut order: Vec<(u64, TileId)> = g.loaded.iter().filter(|(id, _)| !g.shown.contains_key(id)).map(|(id, s)| (s.last_used, *id)).collect();
        order.sort();
        let excess = g.loaded.len() - MAX_CACHED * 3 / 4;
        for (_, id) in order.into_iter().take(excess) {
            g.loaded.remove(&id);
        }
    }
}

// ---- overlays ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn update_overlays(
    time: Res<Time>,
    g: Res<GlobeState>,
    mut map: ResMut<MapState>,
    pin: Res<PlayerPin>,
    canvases: Query<(&RelativeCursorPosition, &ComputedNode, &InheritedVisibility), With<MapCanvas>>,
    mut marker: Query<(&mut Node, &mut Visibility), (With<Marker>, Without<Popup>, Without<PinRoot>, Without<SearchPin>, Without<SearchRing>)>,
    mut popup: Query<(&mut Node, &mut Visibility, &ComputedNode), (With<Popup>, Without<Marker>, Without<PinRoot>, Without<SearchPin>, Without<SearchRing>)>,
    mut title: Query<&mut Text, (With<PopupTitle>, Without<Readout>)>,
    mut pins: Query<(&mut Node, &mut Visibility, &mut UiTransform), (With<PinRoot>, Without<Marker>, Without<Popup>, Without<PinRing>)>,
    mut rings: Query<(&mut UiTransform, &mut BorderColor), With<PinRing>>,
    mut readout: Query<&mut Text, (With<Readout>, Without<PopupTitle>)>,
    mut drop: Query<(&mut Node, &mut Visibility, &mut UiTransform), (With<SearchPin>, Without<Marker>, Without<Popup>, Without<PinRoot>, Without<SearchRing>, Without<PinRing>)>,
    mut ripple: Query<(&mut Node, &mut Visibility, &mut UiTransform, &mut BorderColor), (With<SearchRing>, Without<Marker>, Without<Popup>, Without<PinRoot>, Without<SearchPin>, Without<PinRing>)>,
) {
    let Some((size, _, norm, over)) = active_canvas(canvases.iter()) else { return };
    let at = |f: Vec2| f * size;

    // The search pin: fall onto the searched place, then hand the highlight to
    // the marker/popup flow just below, which opens the drop popup next to it.
    let secs = time.elapsed_secs();
    const DROP_T: f32 = 0.6;
    const DROP_H: f32 = 170.0;
    const BOUNCE_T: f32 = 0.35;
    let mut drop_state: Option<(Vec2, f32)> = None;
    if let Some(l) = map.landing.as_mut() {
        let p = gl::unit_from_latlon(l.place.lat_deg, l.place.lon_deg);
        let goal = p;
        // Wait until the fly-to has centred the place (or a couple of seconds
        // as a fallback), so the pin never lands on a half-flown frame where
        // the spot is still drifting toward the middle of the view.
        // Wait until the fly-to has fully finished (camera centred and zoomed)
        // before dropping the pin, so it lands on the place, not mid-flight.
        let done = g.fly.is_none();
        if l.started.is_none() && (done || secs - l.created > 3.5) {
            l.started = Some(secs);
            if g.facing(p) {
                drop_state = Some((Vec2::new(size.x * 0.5, size.y * 0.5), 0.0));
            }
        }
    }
    if drop_state.is_none() {
        if let Some(l) = map.landing.as_ref().filter(|l| l.started.is_some()) {
            let p = gl::unit_from_latlon(l.place.lat_deg, l.place.lon_deg);
            if g.facing(p) {
                drop_state = Some((Vec2::new(size.x * 0.5, size.y * 0.5), secs - l.started.unwrap_or(secs)));
            }
        }
    }
    for (mut n, mut v, mut t) in &mut drop {
        match drop_state {
            Some((px, t_land)) if t_land < DROP_T + BOUNCE_T => {
                let y_off = if t_land < DROP_T {
                    // Accelerating fall: d(1-e²)/dt grows with e.
                    let e = (t_land / DROP_T).clamp(0.0, 1.0);
                    -DROP_H * (1.0 - e * e)
                } else {
                    let u = (t_land - DROP_T) / BOUNCE_T;
                    -26.0 * (1.0 - 0.4 * u) * (std::f32::consts::PI * u).sin()
                };
                t.scale = if t_land < DROP_T {
                    Vec2::ONE
                } else {
                    let u = (t_land - DROP_T) / BOUNCE_T;
                    let s = (std::f32::consts::PI * u).sin();
                    Vec2::new(1.0 + 0.2 * s, 1.0 - 0.3 * s)
                };
                n.left = Val::Px(px.x - 13.0);
                n.top = Val::Px(px.y - 36.0 + y_off);
                *v = Visibility::Inherited;
            }
            _ => *v = Visibility::Hidden,
        }
    }
    // One expanding ripple while the tip is on the ground.
    for (mut n, mut v, mut t, mut bc) in &mut ripple {
        let hit = drop_state
            .map(|(px, t_land)| (px, (t_land - DROP_T) / BOUNCE_T))
            .filter(|(_, u)| (0.0..1.0).contains(u));
        match hit {
            Some((px, u)) => {
                n.left = Val::Px(px.x - 15.0);
                n.top = Val::Px(px.y - 15.0);
                t.scale = Vec2::splat(0.4 + 1.5 * u);
                *bc = BorderColor::all(Color::srgba(0.98, 0.42, 0.1, 0.85 * (1.0 - u)));
                *v = Visibility::Inherited;
            }
            None => *v = Visibility::Hidden,
        }
    }
    // Landed? The drop popup follows right after the bounce.
    if drop_state.is_some_and(|(_, t_land)| t_land >= DROP_T + BOUNCE_T) {
        map.land();
    }

    // Pick marker and popup.
    let sel = map.selected.map(|s| (s, gl::unit_from_latlon(s.lat_deg, s.lon_deg)));
    let sel_px = sel.and_then(|(_, p)| g.facing(p).then(|| g.project(p)).flatten()).map(at);
    for (mut n, mut v) in &mut marker {
        match sel_px {
            Some(px) => {
                n.left = Val::Px(px.x - 9.0);
                n.top = Val::Px(px.y - 9.0);
                *v = Visibility::Inherited;
            }
            None => *v = Visibility::Hidden,
        }
    }
    for (mut n, mut v, c) in &mut popup {
        match sel_px {
            Some(px) => {
                let ph = c.size().y * c.inverse_scale_factor();
                let pw = 260.0;
                let left = if px.x + 28.0 + pw > size.x { px.x - 28.0 - pw } else { px.x + 28.0 };
                n.left = Val::Px(left.max(6.0));
                n.top = Val::Px((px.y - ph * 0.5).clamp(6.0, (size.y - ph - 6.0).max(6.0)));
                *v = Visibility::Inherited;
            }
            None => *v = Visibility::Hidden,
        }
    }
    if let Some((s, _)) = sel {
        for mut t in &mut title {
            let ns = if s.lat_deg >= 0.0 { 'N' } else { 'S' };
            let ew = if s.lon_deg >= 0.0 { 'E' } else { 'W' };
            let coords = format!("{:.3}° {ns}, {:.3}° {ew}", s.lat_deg.abs(), s.lon_deg.abs());
            *t = Text::new(if map.selected_name.is_empty() { coords } else { format!("{}\n{}", map.selected_name, coords) });
        }
    }

    // Player pin.
    let pin_px = pin.pos.and_then(|(geo, heading)| {
        let p = gl::unit_from_latlon(geo.lat_deg, geo.lon_deg);
        if !g.facing(p) {
            return None;
        }
        let a = g.project(p)?;
        let ahead = (p + gl::tangent_for_heading(p, heading) * 0.02).normalize();
        let b = g.project(ahead)?;
        let d = (b - a) * size;
        Some((a * size, d.x.atan2(-d.y)))
    });
    for (mut n, mut v, mut t) in &mut pins {
        match pin_px {
            Some((px, rot)) => {
                n.left = Val::Px(px.x - 22.0);
                n.top = Val::Px(px.y - 22.0);
                t.rotation = Rot2::radians(rot);
                *v = Visibility::Inherited;
            }
            None => *v = Visibility::Hidden,
        }
    }
    let phase = (secs * 0.9).fract();
    for (mut t, mut border) in &mut rings {
        let s = 0.55 + phase * 1.1;
        t.scale = Vec2::splat(s);
        *border = BorderColor::all(Color::srgba(0.98, 0.42, 0.1, 0.85 * (1.0 - phase)));
    }

    // Readout: what is under the cursor, or the view centre.
    let (p, label) = match (g.cursor_pick, over, norm) {
        (Some(p), true, _) => (p, "cursor"),
        _ => (g.focus, "centre"),
    };
    let (lat, lon) = gl::latlon_from_unit(p);
    let alt_km = (g.dist() - 1.0) as f64 * EARTH_RADIUS_M / 1000.0;
    for mut t in &mut readout {
        *t = Text::new(format!(
            "{label}  {:.2}° {}, {:.2}° {}    altitude {}",
            lat.abs(),
            if lat >= 0.0 { 'N' } else { 'S' },
            lon.abs(),
            if lon >= 0.0 { 'E' } else { 'W' },
            if alt_km >= 10.0 { format!("{:.0} km", alt_km) } else { format!("{:.1} km", alt_km) }
        ));
    }
}

// ---- place labels -----------------------------------------------------------------------

/// Font size, colour, upper-casing and the globe-zoom window for a class.
/// Big areas (continents, countries) fade out as you zoom in; small towns
/// only appear once you are close.
fn label_style(class: PlaceClass) -> (f32, Color, bool, f32, f32) {
    match class {
        PlaceClass::Continent => (15.0, Color::srgba(1.0, 1.0, 1.0, 0.55), true, 0.0, 2.6),
        PlaceClass::Country => (13.5, Color::srgba(1.0, 0.97, 0.9, 0.92), true, 1.2, 8.5),
        PlaceClass::State => (11.0, Color::srgba(0.85, 0.9, 1.0, 0.7), false, 4.0, 10.5),
        PlaceClass::City => (13.0, Color::WHITE, false, 3.2, 99.0),
        PlaceClass::Town => (11.5, Color::srgba(1.0, 1.0, 1.0, 0.9), false, 6.5, 99.0),
        PlaceClass::Village => (10.5, Color::srgba(1.0, 1.0, 1.0, 0.8), false, 9.0, 99.0),
    }
}

/// Lay out place names from the tiles on screen: most important first,
/// skipping any that would overlap one already placed or sit on the limb.
fn update_labels(
    g: Res<GlobeState>,
    canvases: Query<(Entity, &RelativeCursorPosition, &ComputedNode, &InheritedVisibility), With<MapCanvas>>,
    mut labels: Query<(&ChildOf, &ComputedNode, &mut Text, &mut TextFont, &mut TextColor, &mut Node, &mut Visibility), With<PlaceLabel>>,
) {
    let Some((canvas, size)) = canvases.iter().find(|(_, _, n, v)| v.get() && n.size().x > 8.0).map(|(e, _, n, _)| (e, n.size() * n.inverse_scale_factor()))
    else {
        return;
    };
    let zoom = g.zoom;
    let mut cands: Vec<&Place> = Vec::new();
    let mut seen = HashSet::new();
    for id in &g.wanted_best {
        let Some(slot) = g.loaded.get(id) else { continue };
        for p in &slot.places {
            let (_, _, _, lo, hi) = label_style(p.class);
            if zoom < lo || zoom > hi || !seen.insert(p.name.as_str()) {
                continue;
            }
            cands.push(p);
        }
    }
    cands.sort_by_key(|p| (p.class as u8, p.rank));

    let mut boxes: Vec<Rect> = Vec::new();
    let mut placed: Vec<(&Place, Vec2)> = Vec::new();
    for p in cands {
        if placed.len() >= LABEL_POOL {
            break;
        }
        // Skip points near the limb, where labels pile up and slide fast.
        let to_cam = (g.cam_pos - p.pos).normalize_or_zero();
        if p.pos.dot(to_cam) < 0.2 {
            continue;
        }
        let Some(f) = g.project(p.pos) else { continue };
        let px = f * size;
        let (fs, _, upper, _, _) = label_style(p.class);
        let w = p.name.chars().count() as f32 * fs * if upper { 0.68 } else { 0.56 } + 6.0;
        let h = fs + 6.0;
        let r = Rect::from_center_size(px, Vec2::new(w, h));
        if r.min.x < 4.0 || r.min.y < 4.0 || r.max.x > size.x - 4.0 || r.max.y > size.y - 28.0 {
            continue;
        }
        if boxes.iter().any(|b| !b.intersect(r).is_empty()) {
            continue;
        }
        boxes.push(r);
        placed.push((p, px));
    }

    let mut it = placed.into_iter();
    for (parent, measured, mut text, mut font, mut color, mut node, mut vis) in &mut labels {
        if parent.parent() != canvas {
            continue;
        }
        let Some((p, px)) = it.next() else {
            *vis = Visibility::Hidden;
            continue;
        };
        let (fs, col, upper, _, _) = label_style(p.class);
        let name = if upper { p.name.to_uppercase() } else { p.name.clone() };
        let same = text.0 == name && font.font_size == FontSize::Px(fs);
        if text.0 != name {
            text.0 = name;
        }
        if font.font_size != FontSize::Px(fs) {
            font.font_size = FontSize::Px(fs);
        }
        color.0 = col;
        // Centre on the point using last frame's measured width (falls back
        // to an estimate on the first frame a new name is shown).
        let mw = measured.size().x * measured.inverse_scale_factor();
        let w = if same && mw > 1.0 { mw } else { p.name.chars().count() as f32 * fs * if upper { 0.68 } else { 0.56 } };
        node.left = Val::Px(px.x - w * 0.5);
        node.top = Val::Px(px.y - fs * 0.6);
        *vis = Visibility::Inherited;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The clock after the map ease has run out: the offset is applied to
    /// the clock in hours, like `menu::apply_graphics` does.
    fn eased_clock(clock: &crate::sky::Clock, gfx: &crate::menu::GraphicsSettings, target: f64) -> f64 {
        clock.unix_s + (target - gfx.hour_offset) * 3600.0
    }

    #[test]
    fn clicking_a_place_puts_the_chosen_hour_there() {
        let mut map = MapState::default();
        let mut gfx = crate::menu::GraphicsSettings::default();
        let clock = crate::sky::Clock { unix_s: 1_710_892_800.0, rate: 1.0 };
        // 3 pm by default, wherever on Earth you click.
        for lon in [139.7, -74.0, 7.75, 151.2] {
            let mut map = MapState::default();
            retarget_time(&mut map, &mut gfx, &clock, lon);
            let target = map.time_target_offset.expect("the map sets a time target");
            let unix = eased_clock(&clock, &gfx, target);
            assert!((ed_geo::sun::solar_hour(unix, lon) - 15.0).abs() < 1e-6, "lon {lon}");
        }
    }

    #[test]
    fn a_hour_the_player_set_is_the_one_the_click_uses() {
        let mut map = MapState::default();
        let mut gfx = crate::menu::GraphicsSettings::default();
        gfx.solar_hour = 1.0;
        let clock = crate::sky::Clock { unix_s: 1_710_892_800.0, rate: 1.0 };
        retarget_time(&mut map, &mut gfx, &clock, 139.7);
        let target = map.time_target_offset.expect("the map sets a time target");
        let unix = eased_clock(&clock, &gfx, target);
        assert!((ed_geo::sun::solar_hour(unix, 139.7) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn live_time_is_left_alone() {
        let mut map = MapState::default();
        let mut gfx = crate::menu::GraphicsSettings::default();
        gfx.time_live = true;
        let clock = crate::sky::Clock { unix_s: 1_710_892_800.0, rate: 1.0 };
        retarget_time(&mut map, &mut gfx, &clock, 139.7);
        assert!(map.time_target_offset.is_none());
    }
}
