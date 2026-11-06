//! Tree impostors on screen: the far end of the tree LOD chain.
//!
//! The atlas is baked on a background thread at startup
//! ([`ed_world::trees::impostor::bake`], a few seconds); until it lands the
//! material samples an empty texture and impostors draw nothing. Each tile
//! with trees gets ONE impostor entity (a quad per tree, see
//! [`ed_world::trees::impostor::build_mesh`]) that the vertex shader
//! billboards, thins and fades with distance.

use bevy::asset::{RenderAssetUsages, embedded_asset};
use bevy::camera::primitives::Aabb;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, MeshVertexAttribute, MeshVertexBufferLayoutRef, PrimitiveTopology};
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, Extent3d, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError, TextureDimension,
    TextureFormat, TextureViewDescriptor, TextureViewDimension, VertexFormat,
};
use bevy::shader::ShaderRef;
use ed_world::trees::{
    impostor::{self, ImpostorAtlas, ImpostorMesh},
    textures::Rgba8Image,
};
use std::sync::{Arc, Mutex};

const SHADER: &str = "embedded://earthdrop/shaders/impostor.wesl";

/// Every tree shows up to this distance (m); beyond it the share falls as
/// (FULL_M / d)^2 while the kept ones grow.
pub const FULL_M: f32 = 1000.0;
/// Cap on how much a kept tree grows to stand in for its thinned neighbours.
pub const GROW_MAX: f32 = 6.0;
/// Trees smaller than this on screen (px) fade out (ramp 0.8x..1.3x; sub-2 px quads flicker).
pub const MIN_PX: f32 = 2.5;

pub const ATTRIBUTE_IMP_PARAMS: MeshVertexAttribute = MeshVertexAttribute::new("ImpostorParams", 0x1A7E_0001, VertexFormat::Float32x4);
pub const ATTRIBUTE_IMP_AXIS: MeshVertexAttribute = MeshVertexAttribute::new("ImpostorAxis", 0x1A7E_0002, VertexFormat::Float32x4);
/// Tile (u, v) of the tree's base: far-shadow lookups.
pub const ATTRIBUTE_IMP_TILE_UV: MeshVertexAttribute = MeshVertexAttribute::new("ImpostorTileUv", 0x1A7E_0003, VertexFormat::Float32x2);

pub struct ImpostorPlugin;

impl Plugin for ImpostorPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "shaders/impostor.wesl");
        app.add_plugins(MaterialPlugin::<ImpostorMaterial>::default()).add_systems(Update, install_atlas);
        app.add_systems(Update, imp_dbg_hide); // TEMP debug
    }
}

// TEMP debug: EW_HIDE_IMP=1 hides every impostor entity.
fn imp_dbg_hide(mut q: Query<&mut Visibility, With<MeshMaterial3d<ImpostorMaterial>>>) {
    if std::env::var("EW_HIDE_IMP").is_err() {
        return;
    }
    for mut v in &mut q {
        *v = Visibility::Hidden;
    }
}

#[derive(Clone, Copy, ShaderType, Debug)]
pub struct ImpostorParams {
    /// x: FULL_M, y: GROW_MAX, z: MIN_PX.
    pub tune: Vec4,
    /// Far shadows (`far_shadows.rs`): Earth centre in render space (w = on)
    /// and pole (w = debug tint). Same values as the terrain material.
    pub far_c: Vec4,
    pub far_p: Vec4,
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct ImpostorExt {
    #[uniform(100)]
    pub params: ImpostorParams,
    #[texture(101, dimension = "2d_array")]
    #[sampler(102)]
    pub albedo: Handle<Image>,
    #[texture(103, dimension = "2d_array")]
    pub normal: Handle<Image>,
    /// The far-shadow horizon pool (`FarPool::horizon`); the tree's tile
    /// layer rides on the impostor entity's `MeshTag`, as on terrain.
    #[texture(110, dimension = "2d_array")]
    #[sampler(111)]
    pub far_horizon: Handle<Image>,
}

impl MaterialExtension for ImpostorExt {
    fn vertex_shader() -> ShaderRef {
        SHADER.into()
    }
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
    fn prepass_vertex_shader() -> ShaderRef {
        SHADER.into()
    }
    fn prepass_fragment_shader() -> ShaderRef {
        SHADER.into()
    }

    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // Same inputs in every pass: the vertex shader always reads them all.
        let l = layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_NORMAL.at_shader_location(1),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(2),
            Mesh::ATTRIBUTE_COLOR.at_shader_location(5),
            ATTRIBUTE_IMP_PARAMS.at_shader_location(8),
            ATTRIBUTE_IMP_AXIS.at_shader_location(9),
            ATTRIBUTE_IMP_TILE_UV.at_shader_location(10),
        ])?;
        descriptor.vertex.buffers = vec![l];
        Ok(())
    }
}

pub type ImpostorMaterial = ExtendedMaterial<StandardMaterial, ImpostorExt>;

/// The shared impostor material plus the atlas being baked.
#[derive(Resource)]
pub struct Impostors {
    pub material: Handle<ImpostorMaterial>,
    pending: Option<Arc<Mutex<Option<ImpostorAtlas>>>>,
}

fn array_image(size: u32, layers: u32, mips: u32, format: TextureFormat, data: Vec<u8>) -> Image {
    let mut img = Image::new_uninit(
        Extent3d { width: size, height: size, depth_or_array_layers: layers },
        TextureDimension::D2,
        format,
        crate::native::mesh_usage(),
    );
    img.texture_descriptor.mip_level_count = mips;
    img.data = Some(data);
    img.texture_view_descriptor = Some(TextureViewDescriptor { dimension: Some(TextureViewDimension::D2Array), ..default() });
    img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        ..default()
    });
    img
}

/// Create the material (empty atlas) and start the bake. The source
/// textures are the ones the tree materials use.
pub fn setup(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    mats: &mut Assets<ImpostorMaterial>,
    leaves: Rgba8Image,
    bark: Rgba8Image,
    palm_bark: Rgba8Image,
) {
    let mut empty = |f| images.add(array_image(1, 1, 1, f, vec![0; 4]));
    let material = mats.add(ExtendedMaterial {
        base: StandardMaterial {
            alpha_mode: AlphaMode::Mask(0.5),
            // A canopy seen from afar is a rough, self-shadowed volume: no
            // sky sheen (bright speckles at low sun otherwise).
            perceptual_roughness: 0.95,
            reflectance: 0.1,
            diffuse_transmission: 0.2,
            ..default()
        },
        extension: ImpostorExt {
            params: ImpostorParams { tune: Vec4::new(FULL_M, GROW_MAX, MIN_PX, 0.0), far_c: Vec4::ZERO, far_p: Vec4::ZERO },
            albedo: empty(TextureFormat::Rgba8UnormSrgb),
            normal: empty(TextureFormat::Rgba8Unorm),
            far_horizon: empty(TextureFormat::Rgba8Unorm),
        },
    });
    let slot = Arc::new(Mutex::new(None));
    let out = slot.clone();
    std::thread::Builder::new()
        .name("impostor-bake".into())
        .spawn(move || {
            let t0 = std::time::Instant::now();
            let a = impostor::bake(&impostor::BakeSources { leaves: &leaves, bark: &bark, palm_bark: &palm_bark });
            info!("impostor atlas: {} layers baked in {:.1}s", a.layers, t0.elapsed().as_secs_f32());
            *out.lock().unwrap() = Some(a);
        })
        .expect("spawn impostor bake");
    commands.insert_resource(Impostors { material, pending: Some(slot) });
}

fn install_atlas(imp: Option<ResMut<Impostors>>, mut images: ResMut<Assets<Image>>, mut mats: ResMut<Assets<ImpostorMaterial>>) {
    let Some(mut imp) = imp else { return };
    let Some(slot) = &imp.pending else { return };
    let Some(a) = slot.lock().unwrap().take() else { return };
    let albedo = images.add(array_image(a.size, a.layers, a.mip_levels, TextureFormat::Rgba8UnormSrgb, a.albedo));
    let normal = images.add(array_image(a.size, a.layers, a.mip_levels, TextureFormat::Rgba8Unorm, a.normal));
    if let Some(mut m) = mats.get_mut(&imp.material) {
        m.extension.albedo = albedo;
        m.extension.normal = normal;
    }
    imp.pending = None;
}

/// Engine-free quads -> Bevy mesh plus a bounding box that covers every
/// quad at full growth (positions are sphere centres only).
pub fn to_mesh(m: ImpostorMesh) -> (Mesh, Aabb) {
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    let mut r = 0.0f32;
    for (p, q) in m.positions.iter().zip(&m.params) {
        lo = lo.min(Vec3::from(*p));
        hi = hi.max(Vec3::from(*p));
        r = r.max(q[0]);
    }
    let pad = Vec3::splat(r * GROW_MAX * 1.1 + 1.0);
    let aabb = Aabb::from_min_max(lo - pad, hi + pad);
    let mesh = Mesh::new(PrimitiveTopology::TriangleList, crate::native::world_mesh_usage())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, m.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, m.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, m.corners)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, m.colors)
        .with_inserted_attribute(ATTRIBUTE_IMP_PARAMS, m.params)
        .with_inserted_attribute(ATTRIBUTE_IMP_AXIS, m.axes)
        .with_inserted_attribute(ATTRIBUTE_IMP_TILE_UV, m.tile_uv)
        .with_inserted_indices(Indices::U32(m.indices));
    (mesh, aabb)
}

/// Spawn a tile's impostor entity under `parent`. `far_tag` = the tile's
/// far-shadow layer bits (`FarPool::alloc`), 0 for none.
pub fn spawn(commands: &mut Commands, meshes: &mut Assets<Mesh>, imp: &Impostors, parent: Entity, mesh: Mesh, aabb: Aabb, far_tag: u32) {
    commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(imp.material.clone()), bevy::mesh::MeshTag::new(far_tag), Transform::default(), aabb, NotShadowCaster, ChildOf(parent)));
}
