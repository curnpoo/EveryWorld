//! Trees and shrubs on screen: the wind-swayed foliage material, the
//! procedural leaf atlas and bark textures, and distance LOD.
//!
//! Each tile's trees arrive as chunks ([`ed_world::trees::TileTrees`]). A
//! chunk always has a merged far-LOD mesh (big leaf cards, simple trunks).
//! When the camera comes close, [`stream_near_trees`] hides it and spawns
//! full-detail trees as entities sharing one mesh per species variant, so
//! Bevy batches them into a few instanced draws; walking away reverses it.

use crate::meshconv::to_mesh;
use bevy::asset::{RenderAssetUsages, embedded_asset};
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, Extent3d, TextureDimension, TextureFormat};
use bevy::shader::ShaderRef;
use ed_world::foliage::{TreeInstance, TreeKind};
use ed_world::trees::{self, BarkStyle, TileTrees, WindMesh, textures::Rgba8Image};
use std::collections::HashMap;

/// Full-detail trees within this distance of a chunk's center...
const NEAR_IN: f32 = 230.0;
/// ...and back to the far cards beyond this one (hysteresis).
const NEAR_OUT: f32 = 270.0;
/// Chunks switched per frame (spreads spawn cost).
const SWAPS_PER_FRAME: usize = 6;

pub struct FoliagePlugin;

impl Plugin for FoliagePlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "shaders/foliage.wesl");
        embedded_asset!(app, "shaders/foliage_prepass.wesl");
        app.add_plugins(MaterialPlugin::<FoliageMaterial>::default())
            .insert_resource(Wind::default())
            .add_systems(PreStartup, setup_foliage)
            .add_systems(Update, (update_wind, stream_near_trees));
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
        "embedded://earthdrop/shaders/foliage.wesl".into()
    }
    fn prepass_vertex_shader() -> ShaderRef {
        "embedded://earthdrop/shaders/foliage_prepass.wesl".into()
    }
    fn deferred_vertex_shader() -> ShaderRef {
        "embedded://earthdrop/shaders/foliage_prepass.wesl".into()
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
    let mut out = Image::new_uninit(size, TextureDimension::D2, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::RENDER_WORLD);
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

fn setup_foliage(mut commands: Commands, mut images: ResMut<Assets<Image>>, mut mats: ResMut<Assets<FoliageMaterial>>) {
    let atlas = images.add(to_image(trees::textures::leaf_atlas(), false));
    let bark_tex = images.add(to_image(trees::textures::bark_texture(), true));
    let palm_tex = images.add(to_image(trees::textures::palm_bark_texture(), true));
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

/// A chunk of a tile's trees, switching between far cards and full detail.
#[derive(Component)]
pub struct TreeChunkLod {
    instances: Vec<TreeInstance>,
    up: Vec3,
    /// Chunk center in the tile's mesh space (instances are relative to the tile).
    center: Vec3,
    far: Entity,
    near: Vec<Entity>,
}

/// Spawn one full-detail tree under `parent`; `offset` is subtracted from
/// the instance position (the parent's position in the same space).
/// `variant` is normally [`trees::variant_for`].
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
) -> Entity {
    let p = fa.proto(meshes, t.kind, variant);
    let transform = Transform {
        translation: t.pos - offset,
        rotation: trees::instance_rotation(t, up),
        scale: Vec3::splat(t.scale),
    };
    let root = commands.spawn((transform, Visibility::default(), ChildOf(parent))).id();
    if let Some(b) = p.bark {
        commands.spawn((Mesh3d(b), MeshMaterial3d(fa.bark_for(p.style).clone()), Transform::default(), ChildOf(root)));
    }
    if let Some(l) = p.leaves {
        commands.spawn((Mesh3d(l), MeshMaterial3d(fa.leaves.clone()), Transform::default(), ChildOf(root)));
    }
    root
}

/// Spawn a tile's trees under `parent` (the tile entity, whose mesh space
/// the chunks are in). Coarse tiles (one chunk) only ever show far cards.
pub fn spawn_tile_trees(commands: &mut Commands, meshes: &mut Assets<Mesh>, fa: &FoliageAssets, parent: Entity, trees: &TileTrees) {
    let lod = trees.chunks.len() > 1;
    for c in &trees.chunks {
        let holder = commands.spawn((Transform::from_translation(c.center), Visibility::default(), ChildOf(parent))).id();
        let far = commands
            .spawn((Mesh3d(meshes.add(wind_mesh(&c.far))), MeshMaterial3d(fa.leaves.clone()), Transform::default(), ChildOf(holder)))
            .id();
        if lod {
            commands.entity(holder).insert(TreeChunkLod { instances: c.instances.clone(), up: trees.up, center: c.center, far, near: Vec::new() });
        }
    }
}

/// Swap chunks between far cards and full-detail trees by camera distance.
pub fn stream_near_trees(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    fa: Option<ResMut<FoliageAssets>>,
    cam: Query<&GlobalTransform, With<Camera3d>>,
    mut chunks: Query<(Entity, &GlobalTransform, &mut TreeChunkLod)>,
) {
    let Some(mut fa) = fa else { return };
    let Some(cam) = cam.iter().next().map(|c| c.translation()) else { return };
    let mut swaps = 0;
    for (e, gt, mut c) in &mut chunks {
        let d = gt.translation().distance(cam);
        if c.near.is_empty() && d < NEAR_IN && swaps < SWAPS_PER_FRAME {
            swaps += 1;
            let (up, center) = (c.up, c.center);
            let near: Vec<Entity> = c.instances.iter().map(|t| spawn_near_tree(&mut commands, &mut meshes, &mut fa, e, t, up, center, trees::variant_for(t))).collect();
            c.near = near;
            commands.entity(c.far).insert(Visibility::Hidden);
        } else if !c.near.is_empty() && d > NEAR_OUT && swaps < SWAPS_PER_FRAME {
            swaps += 1;
            for n in c.near.drain(..) {
                commands.entity(n).despawn();
            }
            commands.entity(c.far).insert(Visibility::Inherited);
        }
    }
}
