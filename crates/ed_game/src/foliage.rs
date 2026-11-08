//! Trees and shrubs on screen: the wind-swayed foliage material, the
//! procedural leaf atlas and bark textures, and distance LOD.
//!
//! LOD chain per tree, far to near: impostor quad (one entity per tile, see
//! [`crate::impostors`]), merged leaf-card mesh per chunk (fine tiles,
//! within [`impostor::CARD_RANGE_M`]), full-detail tree (within `NEAR_IN`)
//! sharing one mesh per species variant so Bevy instances them. Cards and
//! full trees are entities only while the camera is near
//! ([`stream_tree_chunks`]).

use crate::meshconv::to_mesh;
use bevy::asset::{RenderAssetUsages, embedded_asset};
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::camera::visibility::RenderLayers;
use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, Extent3d, TextureDimension, TextureFormat};
use bevy::shader::ShaderRef;
use ed_world::foliage::{TreeInstance, TreeKind};
use ed_world::trees::{self, BarkStyle, TileTrees, WindMesh, impostor, textures::Rgba8Image};
use std::collections::HashMap;

/// Full-detail trees within this distance of a chunk's center...
#[cfg(target_os = "windows")]
const NEAR_IN: f32 = 100.0;
#[cfg(not(target_os = "windows"))]
const NEAR_IN: f32 = 230.0;
/// ...and back to the far cards beyond this one (hysteresis).
#[cfg(target_os = "windows")]
const NEAR_OUT: f32 = 130.0;
#[cfg(not(target_os = "windows"))]
const NEAR_OUT: f32 = 270.0;
/// Chunks switched per frame (spreads spawn cost).
const SWAPS_PER_FRAME: usize = 6;

pub struct FoliagePlugin;

impl Plugin for FoliagePlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "shaders/foliage.wesl");
        embedded_asset!(app, "shaders/foliage_prepass.wesl");
        app.add_plugins((MaterialPlugin::<FoliageMaterial>::default(), crate::impostors::ImpostorPlugin))
            .insert_resource(Wind::default())
            .add_systems(PreStartup, setup_foliage)
            .add_systems(Update, (update_wind, stream_tree_chunks));
    }
}

pub type FoliageMaterial = ExtendedMaterial<StandardMaterial, WindExt>;

/// Material extension: wind uniform + the swaying vertex shaders.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct WindExt {
    /// xy: direction the wind blows toward (world x, z), z: strength, w: time.
    #[uniform(100)]
    pub wind: Vec4,
    /// x: previous frame's time (for motion vectors).
    #[uniform(100)]
    pub prev: Vec4,
}

impl MaterialExtension for WindExt {
    fn vertex_shader() -> ShaderRef {
        "embedded://everyworld/shaders/foliage.wesl".into()
    }
    fn prepass_vertex_shader() -> ShaderRef {
        "embedded://everyworld/shaders/foliage_prepass.wesl".into()
    }
    fn deferred_vertex_shader() -> ShaderRef {
        "embedded://everyworld/shaders/foliage_prepass.wesl".into()
    }
}

/// Live wind (from the weather when we have it).
#[derive(Resource, Clone, Copy)]
pub struct Wind {
    /// Horizontal render-space direction the wind blows toward (x, z).
    pub dir: Vec2,
    /// ~1.0 for a fresh 8 m/s breeze.
    pub strength: f32,
}

impl Default for Wind {
    fn default() -> Self {
        Self { dir: Vec2::new(0.8, 0.3).normalize(), strength: 0.45 }
    }
}

/// One species variant, uploaded once and shared by every instance.
#[derive(Clone)]
struct Proto {
    bark: Option<Handle<Mesh>>,
    leaves: Option<Handle<Mesh>>,
    style: BarkStyle,
}

#[derive(Resource)]
pub struct FoliageAssets {
    pub bark: Handle<FoliageMaterial>,
    pub palm_bark: Handle<FoliageMaterial>,
    pub leaves: Handle<FoliageMaterial>,
    protos: HashMap<(TreeKind, u32), Proto>,
}

impl FoliageAssets {
    fn bark_for(&self, style: BarkStyle) -> &Handle<FoliageMaterial> {
        match style {
            BarkStyle::Rough => &self.bark,
            BarkStyle::Palm => &self.palm_bark,
        }
    }

    fn proto(&mut self, meshes: &mut Assets<Mesh>, kind: TreeKind, variant: u32) -> Proto {
        self.protos
            .entry((kind, variant))
            .or_insert_with(|| {
                let m = trees::model(kind, variant);
                Proto {
                    bark: (!m.bark.is_empty()).then(|| meshes.add(wind_mesh(&m.bark))),
                    leaves: (!m.leaves.is_empty()).then(|| meshes.add(wind_mesh(&m.leaves))),
                    style: m.bark_style,
                }
            })
            .clone()
    }
}

/// `WindMesh` -> `bevy::Mesh` with the wind data in UV1.
pub fn wind_mesh(m: &WindMesh) -> Mesh {
    to_mesh(&m.mesh).with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, m.wind.clone())
}

fn to_image(img: Rgba8Image, repeat: bool) -> Image {
    let size = Extent3d { width: img.width, height: img.height, depth_or_array_layers: 1 };
    let mut out = Image::new_uninit(size, TextureDimension::D2, TextureFormat::Rgba8UnormSrgb, crate::native::mesh_usage());
    out.texture_descriptor.mip_level_count = img.mip_levels;
    out.data = Some(img.data);
    let mode = if repeat { ImageAddressMode::Repeat } else { ImageAddressMode::ClampToEdge };
    out.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: mode,
        address_mode_v: mode,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        anisotropy_clamp: 8,
        ..default()
    });
    out
}

fn setup_foliage(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut mats: ResMut<Assets<FoliageMaterial>>,
    mut imp_mats: ResMut<Assets<crate::impostors::ImpostorMaterial>>,
) {
    let (leaf_img, bark_img, palm_img) = (trees::textures::leaf_atlas(), trees::textures::bark_texture(), trees::textures::palm_bark_texture());
    // The impostor bake renders the same textures on a background thread.
    crate::impostors::setup(&mut commands, &mut images, &mut imp_mats, leaf_img.clone(), bark_img.clone(), palm_img.clone());
    let atlas = images.add(to_image(leaf_img, false));
    let bark_tex = images.add(to_image(bark_img, true));
    let palm_tex = images.add(to_image(palm_img, true));
    let ext = WindExt::default();
    let bark = |tex: Handle<Image>| StandardMaterial {
        base_color_texture: Some(tex),
        perceptual_roughness: 0.92,
        reflectance: 0.25,
        ..default()
    };
    let leaves = StandardMaterial {
        base_color_texture: Some(atlas),
        alpha_mode: AlphaMode::Mask(0.5),
        // Two-sided without flipping normals: cards keep the baked
        // crown-outward normals on both faces.
        cull_mode: None,
        double_sided: false,
        perceptual_roughness: 0.7,
        reflectance: 0.35,
        // Light through the leaves: backlit canopies glow.
        diffuse_transmission: 0.35,
        ..default()
    };
    commands.insert_resource(FoliageAssets {
        bark: mats.add(ExtendedMaterial { base: bark(bark_tex), extension: ext.clone() }),
        palm_bark: mats.add(ExtendedMaterial { base: bark(palm_tex), extension: ext.clone() }),
        leaves: mats.add(ExtendedMaterial { base: leaves, extension: ext }),
        protos: HashMap::new(),
    });
}

/// Weather wind -> render-space wind, and push time + wind into the materials.
fn update_wind(
    time: Res<Time>,
    weather: Option<Res<crate::sky::CurrentWeather>>,
    mut wind: ResMut<Wind>,
    fa: Option<Res<FoliageAssets>>,
    mut mats: ResMut<Assets<FoliageMaterial>>,
    mut last_t: Local<f32>,
) {
    if let Some(w) = weather.as_ref().and_then(|w| w.0.as_ref()) {
        // Meteorological "from" bearing -> the way it blows, in ENU, then
        // render space (x = east, z = -north).
        let to = (w.wind_direction_deg + 180.0).to_radians();
        wind.dir = Vec2::new(to.sin(), -to.cos());
        let speed = 0.7 * w.wind_speed_ms + 0.3 * w.wind_gusts_ms;
        wind.strength = (speed / 8.0).clamp(0.12, 2.5);
    }
    let Some(fa) = fa else { return };
    // Wrap time so sin() keeps precision over long sessions.
    let t = time.elapsed_secs_wrapped() % 3600.0;
    let prev = if *last_t > t { t } else { *last_t };
    *last_t = t;
    for h in [&fa.bark, &fa.palm_bark, &fa.leaves] {
        if let Some(mut m) = mats.get_mut(h) {
            m.extension.wind = Vec4::new(wind.dir.x, wind.dir.y, wind.strength, t);
            m.extension.prev = Vec4::new(prev, 0.0, 0.0, 0.0);
        }
    }
}

/// A fine tile's tree chunks. Leaf-card meshes and full-detail trees exist
/// as entities only near the camera ([`stream_tree_chunks`]); everything
/// else is drawn by the tile's single impostor entity.
#[derive(Component)]
pub struct TreeChunks {
    up: Vec3,
    chunks: Vec<ChunkState>,
    pub impostor: Option<Entity>,
}

impl TreeChunks {
    /// Trees within `r` of `local` (tile mesh space). Chunk-culled, so cheap
    /// enough for the soundscape to call every frame.
    pub fn trees_near(&self, local: Vec3, r: f32) -> impl Iterator<Item = &TreeInstance> {
        // Chunks are ~150 m wide; anything whose centre is within r + 120 m may hold a hit.
        let reach = r + 120.0;
        self.chunks
            .iter()
            .filter(move |c| c.center.distance_squared(local) < reach * reach)
            .flat_map(|c| c.instances.iter())
            .filter(move |t| t.pos.distance_squared(local) < r * r)
    }
}

struct ChunkState {
    /// Chunk center in the tile's mesh space.
    center: Vec3,
    /// Card mesh, converted on the worker; uploaded the first time the
    /// camera comes within range, then kept by handle.
    far_src: Option<Mesh>,
    far_mesh: Option<Handle<Mesh>>,
    instances: Vec<TreeInstance>,
    far: Option<Entity>,
    near: Vec<Entity>,
}

/// Spawn one full-detail tree under `parent` (bark and leaves entities,
/// each with the tree's transform). `variant` is normally [`trees::variant_for`].
#[allow(clippy::too_many_arguments)]
pub fn spawn_near_tree(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    fa: &mut FoliageAssets,
    parent: Entity,
    t: &TreeInstance,
    up: Vec3,
    offset: Vec3,
    variant: u32,
    out: &mut Vec<Entity>,
) {
    let p = fa.proto(meshes, t.kind, variant);
    let transform = Transform {
        translation: t.pos - offset,
        rotation: trees::instance_rotation(t, up),
        scale: Vec3::splat(t.scale),
    };
    if let Some(b) = p.bark {
        out.push(commands.spawn((Mesh3d(b), MeshMaterial3d(fa.bark_for(p.style).clone()), transform, ChildOf(parent))).id());
    }
    if let Some(l) = p.leaves {
        out.push(commands.spawn((Mesh3d(l), MeshMaterial3d(fa.leaves.clone()), transform, ChildOf(parent))).id());
    }
}

/// One compound collider of trunk cylinders for `trees` (tile space), or
/// `None` when none of them has a trunk (shrubs, ferns, grass).
fn tree_colliders(trees: &[TreeInstance], up: Vec3) -> Option<Collider> {
    let shapes: Vec<(Vec3, Quat, Collider)> = trees
        .iter()
        .filter_map(|t| {
            let trunk = trees::species::def(t.kind).trunk?;
            let rot = trees::instance_rotation(t, up);
            let h = trunk.height * t.scale;
            Some((t.pos + rot * (Vec3::Y * h * 0.5), rot, Collider::cylinder(trunk.radius * t.scale, h)))
        })
        .collect();
    (!shapes.is_empty()).then(|| Collider::compound(shapes))
}

/// A tile's tree chunk with its card mesh already converted (on the worker).
pub struct PreparedTreeChunk {
    center: Vec3,
    far: Option<Mesh>,
    instances: Vec<TreeInstance>,
}

/// A tile's trees, ready to spawn.
pub struct PreparedTrees {
    pub up: Vec3,
    /// Fine tiles only.
    pub chunks: Vec<PreparedTreeChunk>,
    /// Every tree as an impostor quad.
    pub impostors: Option<(Mesh, bevy::camera::primitives::Aabb)>,
}

/// Worker-thread half of spawning: convert every mesh up front.
pub fn prepare_trees(trees: TileTrees) -> PreparedTrees {
    let chunks = trees
        .chunks
        .into_iter()
        .map(|c| PreparedTreeChunk { center: c.center, far: (!c.far.is_empty()).then(|| wind_mesh(&c.far)), instances: c.instances })
        .collect();
    let impostors = (!trees.impostors.is_empty()).then(|| crate::impostors::to_mesh(trees.impostors));
    PreparedTrees { up: trees.up, chunks, impostors }
}

/// Spawn a tile's trees under `parent` (the tile entity): one impostor
/// entity, plus one chunk-set entity on fine tiles. Returns the entity count.
pub fn spawn_tile_trees(commands: &mut Commands, meshes: &mut Assets<Mesh>, imp: &crate::impostors::Impostors, parent: Entity, p: PreparedTrees, far_tag: u32) -> u32 {
    let mut n = 0;
    let imp_ent = if let Some((mesh, aabb)) = p.impostors {
        let e = crate::impostors::spawn(commands, meshes, imp, parent, mesh, aabb, far_tag);
        n += 1;
        Some(e)
    } else {
        None
    };
    if !p.chunks.is_empty() {
        let chunks = p
            .chunks
            .into_iter()
            .map(|c| ChunkState { center: c.center, far_src: c.far, far_mesh: None, instances: c.instances, far: None, near: Vec::new() })
            .collect();
        commands.spawn((TreeChunks { up: p.up, chunks, impostor: imp_ent }, Transform::default(), Visibility::default(), ChildOf(parent)));
        n += 1;
    }
    n
}

/// Card meshes spawned per frame at most (each may upload a mesh).
const CARD_SPAWNS_PER_FRAME: usize = 24;

/// Near the camera, turn chunks into entities: leaf cards within
/// [`impostor::CARD_RANGE_M`], full-detail trees within `NEAR_IN`.
pub fn stream_tree_chunks(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    fa: Option<ResMut<FoliageAssets>>,
    cam: Query<&GlobalTransform, (With<Camera3d>, Without<crate::globe::GlobeCamera>)>,
    mut sets: Query<(Entity, &GlobalTransform, &mut TreeChunks)>,
) {
    let Some(mut fa) = fa else { return };
    let Some(cam) = cam.iter().next().map(|c| c.translation()) else { return };
    let (mut swaps, mut cards) = (0, 0);
    for (root, gt, mut set) in &mut sets {
        let up = set.up;
        for c in set.chunks.iter_mut() {
            let d = gt.transform_point(c.center).distance(cam);
            let card_range = impostor::CARD_RANGE_M + if c.far.is_some() { 60.0 } else { 0.0 };
            if d < card_range {
                if c.far.is_none() && cards < CARD_SPAWNS_PER_FRAME {
                    if c.far_mesh.is_none() {
                        c.far_mesh = c.far_src.take().map(|m| meshes.add(m));
                    }
                    if let Some(h) = &c.far_mesh {
                        cards += 1;
                        let e = commands.spawn((Mesh3d(h.clone()), MeshMaterial3d(fa.leaves.clone()), Transform::from_translation(c.center), ChildOf(root))).id();
                        if !c.near.is_empty() {
                            commands.entity(e).insert(RenderLayers::layer(crate::clouds::CLOUD_SHADOW_LAYER));
                        }
                        c.far = Some(e);
                    }
                }
            } else if let Some(e) = c.far.take() {
                commands.entity(e).despawn();
            }
            if c.near.is_empty() && d < NEAR_IN && swaps < SWAPS_PER_FRAME {
                swaps += 1;
                let mut near = Vec::with_capacity(c.instances.len() * 2);
                for t in &c.instances {
                    spawn_near_tree(&mut commands, &mut meshes, &mut fa, root, t, up, Vec3::ZERO, trees::variant_for(t), &mut near);
                }
                // Trunk colliders: one static compound per chunk, so the
                // player and vehicles can't walk through full-detail trees.
                if let Some(col) = tree_colliders(&c.instances, up) {
                    near.push(commands.spawn((RigidBody::Static, col, Transform::default(), ChildOf(root))).id());
                }
                // Full trees draw for the camera only; the chunk's leaf cards
                // (one merged mesh) cast their shadow from the sun-only layer.
                // Thousands of bark/leaf draws per cascade become one.
                for &e in &near {
                    commands.entity(e).insert(RenderLayers::layer(crate::camera::CAMERA_ONLY_LAYER));
                }
                c.near = near;
                if let Some(e) = c.far {
                    commands.entity(e).insert(RenderLayers::layer(crate::clouds::CLOUD_SHADOW_LAYER));
                }
            } else if !c.near.is_empty() && d > NEAR_OUT && swaps < SWAPS_PER_FRAME {
                swaps += 1;
                for n in c.near.drain(..) {
                    commands.entity(n).despawn();
                }
                if let Some(e) = c.far {
                    commands.entity(e).remove::<RenderLayers>();
                }
            }
        }
        if let Some(imp) = set.impostor {
            let has_near = set.chunks.iter().any(|c| !c.near.is_empty());
            if has_near {
                commands.entity(imp).insert(Visibility::Hidden);
            } else {
                commands.entity(imp).insert(Visibility::Inherited);
            }
        }
    }
}
