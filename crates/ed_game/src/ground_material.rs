//! Ground v2 materials.
//!
//! - [`TerrainMaterial`]: splats 12 procedurally generated, tileable ground
//!   layers (texture arrays: albedo+height, normal+roughness+AO) using the
//!   per-vertex weights `ed_world::surface` writes into the terrain mesh.
//!   Height-based blending, slope-aware triplanar on cliffs, multi-scale
//!   macro variation against tiling, regional vegetation tint, wetness.
//! - [`ScatterMaterial`]: near-camera ground cover (grass, flowers, pebbles,
//!   rocks...). A custom vertex shader sways plants in the wind and shrinks
//!   each instance by its rank as density falls off with distance.
//!
//! Textures are synthesized at startup by `ed_world::ground_tex` (no files,
//! no downloads).

use bevy::asset::{RenderAssetUsages, embedded_asset};
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::mesh::{MeshVertexAttribute, MeshVertexBufferLayoutRef};
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, Extent3d, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError, TextureDimension,
    TextureFormat, TextureViewDescriptor, TextureViewDimension, VertexFormat,
};
use bevy::shader::ShaderRef;
use ed_world::ground_tex;
use ed_world::surface::LAYERS;

pub type TerrainMaterial = ExtendedMaterial<StandardMaterial, GroundExt>;
pub type ScatterMaterial = ExtendedMaterial<StandardMaterial, ScatterExt>;

/// Splat weights, 4 layers per attribute (see `ed_world::surface::Ground`).
pub const ATTRIBUTE_SPLAT0: MeshVertexAttribute = MeshVertexAttribute::new("GroundSplat0", 0x6E57_0001, VertexFormat::Unorm8x4);
pub const ATTRIBUTE_SPLAT1: MeshVertexAttribute = MeshVertexAttribute::new("GroundSplat1", 0x6E57_0002, VertexFormat::Unorm8x4);
pub const ATTRIBUTE_SPLAT2: MeshVertexAttribute = MeshVertexAttribute::new("GroundSplat2", 0x6E57_0003, VertexFormat::Unorm8x4);
/// Satellite albedo (gamma RGB + blend). Also keeps the terrain vertex stride at 64 bytes. With a
/// 60-byte stride, small meshes sharing a vertex slab were drawn from
/// misaligned offsets (long streaks across the sky).
pub const ATTRIBUTE_SPLAT_PAD: MeshVertexAttribute = MeshVertexAttribute::new("GroundSplatPad", 0x6E57_0005, VertexFormat::Unorm8x4);
/// Scatter instance pivot (xyz, mesh space) + reach.floor() + rank.
pub const ATTRIBUTE_PIVOT: MeshVertexAttribute = MeshVertexAttribute::new("ScatterPivot", 0x6E57_0004, VertexFormat::Float32x4);

const TERRAIN_SHADER: &str = "embedded://earthdrop/shaders/terrain.wesl";
const SCATTER_SHADER: &str = "embedded://earthdrop/shaders/ground_scatter.wesl";

/// Texture resolution per ground layer.
const TEX_SIZE: u32 = 512;
const MACRO_SIZE: u32 = 256;

#[derive(Clone, Copy, ShaderType, Debug, Default)]
pub struct GroundParams {
    /// Meters per texture repeat, one per layer (3 x vec4 = 12 layers).
    pub size: [Vec4; 3],
    /// Normal strength per layer.
    pub bump: [Vec4; 3],
    /// xy: world-space UV offset (meters) that keeps textures still across
    /// floating-origin rebases. z: global detail-normal strength. w: unused.
    pub offset: Vec4,
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct GroundExt {
    #[uniform(100)]
    pub params: GroundParams,
    #[texture(101, dimension = "2d_array")]
    #[sampler(102)]
    pub albedo: Handle<Image>,
    #[texture(103, dimension = "2d_array")]
    pub normal: Handle<Image>,
    #[texture(104)]
    #[sampler(105)]
    pub macro_tex: Handle<Image>,
    /// Satellite colour pool (gamma RGB + land/water flag in alpha), one 2D
    /// array per tile resolution class (see [`SAT_CLASSES`]). A terrain mesh
    /// picks its class and layer through its `MeshTag` (see [`SatPool`]), so
    /// every terrain tile shares this one material and batches together.
    #[texture(106, dimension = "2d_array")]
    #[sampler(107)]
    pub sat_512: Handle<Image>,
    #[texture(108, dimension = "2d_array")]
    pub sat_256: Handle<Image>,
    #[texture(109, dimension = "2d_array")]
    pub sat_128: Handle<Image>,
}

impl MaterialExtension for GroundExt {
    fn vertex_shader() -> ShaderRef {
        TERRAIN_SHADER.into()
    }
    fn fragment_shader() -> ShaderRef {
        TERRAIN_SHADER.into()
    }

    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // Prepass/shadows use Bevy's default vertex shader: leave them alone.
        if descriptor.vertex.shader_defs.iter().any(|d| matches!(d, bevy::shader::ShaderDefVal::Bool(n, true) if n == "PREPASS_PIPELINE")) {
            return Ok(());
        }
        let extra = layout.0.get_layout(&[
            ATTRIBUTE_SPLAT0.at_shader_location(8),
            ATTRIBUTE_SPLAT1.at_shader_location(9),
            ATTRIBUTE_SPLAT2.at_shader_location(10),
            ATTRIBUTE_SPLAT_PAD.at_shader_location(11),
        ])?;
        if let Some(buf) = descriptor.vertex.buffers.first_mut() {
            buf.attributes.extend(extra.attributes);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, ShaderType, Debug, Default)]
pub struct ScatterParams {
    /// Camera position in render space (xyz). w: 1 when valid.
    pub camera: Vec4,
    /// xz: wind direction (unit), y: unused, w: strength.
    pub wind: Vec4,
    /// x: time now, y: time last frame (seconds, wrapped).
    pub time: Vec4,
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone, Default)]
pub struct ScatterExt {
    #[uniform(100)]
    pub params: ScatterParams,
}

impl MaterialExtension for ScatterExt {
    fn vertex_shader() -> ShaderRef {
        SCATTER_SHADER.into()
    }
    fn prepass_vertex_shader() -> ShaderRef {
        SCATTER_SHADER.into()
    }

    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // Same inputs in every pass: our vertex shader always reads them all.
        let l = layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_NORMAL.at_shader_location(1),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(2),
            Mesh::ATTRIBUTE_COLOR.at_shader_location(5),
            ATTRIBUTE_PIVOT.at_shader_location(8),
        ])?;
        descriptor.vertex.buffers = vec![l];
        Ok(())
    }
}

#[derive(Resource, Clone)]
pub struct GroundMaterials {
    pub scatter: Handle<ScatterMaterial>,
}

/// Called from `AssetsPlugin::build` so the material types exist before
/// `PreStartup` creates the world materials.
pub fn register(app: &mut App) {
    embedded_asset!(app, "shaders/terrain.wesl");
    embedded_asset!(app, "shaders/ground_scatter.wesl");
    app.add_plugins((MaterialPlugin::<TerrainMaterial>::default(), MaterialPlugin::<ScatterMaterial>::default()))
        .init_resource::<UvOffset>()
        // After the floating origin has moved for this frame, before extraction.
        .add_systems(Last, (track_rebase, update_params).chain());
    if let Some(render_app) = app.get_sub_app_mut(bevy::render::RenderApp) {
        use bevy::render::{ExtractSchedule, Render, RenderSystems};
        render_app
            .init_resource::<RenderSatUploads>()
            .add_systems(ExtractSchedule, extract_sat_uploads)
            .add_systems(Render, write_sat_uploads.in_set(RenderSystems::PrepareResources));
    }
}

fn sampler(aniso: u16) -> ImageSampler {
    ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        anisotropy_clamp: aniso,
        ..default()
    })
}

/// RGBA8 2D array texture with a full CPU-built mip chain per layer.
fn array_image(layers: &[&[u8]], size: u32, format: TextureFormat, srgb: bool) -> Image {
    let mut data = Vec::new();
    let mut levels = 1;
    for l in layers {
        let (chain, n) = ground_tex::mip_chain(l, size, srgb);
        data.extend_from_slice(&chain);
        levels = n;
    }
    let mut img = Image::new_uninit(
        Extent3d { width: size, height: size, depth_or_array_layers: layers.len() as u32 },
        TextureDimension::D2,
        format,
        RenderAssetUsages::RENDER_WORLD,
    );
    img.data = Some(data);
    img.texture_descriptor.mip_level_count = levels;
    img.texture_view_descriptor = Some(TextureViewDescriptor { dimension: Some(TextureViewDimension::D2Array), ..default() });
    img.sampler = sampler(16);
    img
}

/// Satellite texture classes: (texels per side, layers). Sizes match
/// `ed_world::tilegen` (z<=9: 512, z<=12: 256, finer: 128). Layer counts cover
/// the ~90 resident tiles plus the coarse tiles kept during hole-free swaps;
/// if a class runs out, the tile simply renders without satellite colour.
/// Memory is fixed: 40 MB + 24 MB + 12 MB.
pub const SAT_CLASSES: [(u32, u32); 3] = [(512, 40), (256, 96), (128, 192)];

/// Tag value for "no satellite colour".
pub const SAT_NONE: u32 = 0;

/// Fixed pool of satellite layers shared by every terrain tile.
///
/// The layer index rides on the terrain entity's `MeshTag`:
/// `(class + 1) << 24 | layer`, 0 = none. Uploads go straight to the GPU array
/// with `queue.write_texture` in the render world, so adding a tile never
/// creates an image or material asset and never re-uploads the whole array.
#[derive(Resource)]
pub struct SatPool {
    pub images: [Handle<Image>; 3],
    free: [Vec<u32>; 3],
    uploads: Vec<SatUpload>,
}

struct SatUpload {
    image: AssetId<Image>,
    layer: u32,
    size: u32,
    data: Vec<u8>,
}

impl SatPool {
    pub fn new(images: &mut Assets<Image>) -> Self {
        let images = SAT_CLASSES.map(|(n, layers)| images.add(sat_array_image(n, layers)));
        let free = SAT_CLASSES.map(|(_, layers)| (0..layers).rev().collect());
        Self { images, free, uploads: Vec::new() }
    }

    /// Reserve a layer for `data` (`size`^2 RGBA8) and queue its upload.
    /// Returns the `MeshTag` value, or [`SAT_NONE`] if the class is full or
    /// the size is unknown.
    pub fn alloc(&mut self, size: u32, data: Vec<u8>) -> u32 {
        let Some(class) = SAT_CLASSES.iter().position(|(n, _)| *n == size) else { return SAT_NONE };
        if data.len() != (size * size * 4) as usize {
            return SAT_NONE;
        }
        let Some(layer) = self.free[class].pop() else {
            warn!("satellite pool full for {size}px tiles; tile drawn without satellite colour");
            return SAT_NONE;
        };
        self.uploads.push(SatUpload { image: self.images[class].id(), layer, size, data });
        ((class as u32 + 1) << 24) | layer
    }

    /// Return a layer to the pool once its tile is gone.
    pub fn release(&mut self, tag: u32) {
        let class = (tag >> 24) as usize;
        if class == 0 || class > SAT_CLASSES.len() {
            return;
        }
        let layer = tag & 0x00FF_FFFF;
        // Drop a still-queued upload for this slot: the slot is free again.
        self.uploads.retain(|u| !(u.image == self.images[class - 1].id() && u.layer == layer));
        self.free[class - 1].push(layer);
    }

    pub fn free_layers(&self) -> [usize; 3] {
        [self.free[0].len(), self.free[1].len(), self.free[2].len()]
    }
}

/// Render-world queue of satellite layers waiting for their GPU array.
#[derive(Resource, Default)]
struct RenderSatUploads(Vec<SatUpload>);

fn extract_sat_uploads(mut main: ResMut<bevy::render::MainWorld>, mut out: ResMut<RenderSatUploads>) {
    if let Some(mut pool) = main.get_resource_mut::<SatPool>()
        && !pool.uploads.is_empty()
    {
        out.0.append(&mut pool.uploads);
    }
}

fn write_sat_uploads(
    mut pending: ResMut<RenderSatUploads>,
    gpu_images: Res<bevy::render::render_asset::RenderAssets<bevy::render::texture::GpuImage>>,
    queue: Res<bevy::render::renderer::RenderQueue>,
) {
    use bevy::render::render_resource::{Origin3d, TexelCopyBufferLayout, TexelCopyTextureInfo, TextureAspect};
    // Uploads whose array isn't on the GPU yet (first frames) wait.
    pending.0.retain(|u| {
        let Some(gpu) = gpu_images.get(u.image) else { return true };
        queue.write_texture(
            TexelCopyTextureInfo { texture: &gpu.texture, mip_level: 0, origin: Origin3d { x: 0, y: 0, z: u.layer }, aspect: TextureAspect::All },
            &u.data,
            TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(u.size * 4), rows_per_image: Some(u.size) },
            Extent3d { width: u.size, height: u.size, depth_or_array_layers: 1 },
        );
        false
    });
}

/// One satellite class: an uninitialised `n`x`n`x`layers` array, filled per
/// layer by [`write_sat_uploads`].
fn sat_array_image(n: u32, layers: u32) -> Image {
    let mut img = Image::new_uninit(
        Extent3d { width: n, height: n, depth_or_array_layers: layers },
        TextureDimension::D2,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    img.texture_view_descriptor = Some(TextureViewDescriptor { dimension: Some(TextureViewDimension::D2Array), ..default() });
    img.sampler = sat_sampler();
    img
}

fn sat_sampler() -> ImageSampler {
    bevy::image::ImageSampler::Descriptor(bevy::image::ImageSamplerDescriptor {
        address_mode_u: bevy::image::ImageAddressMode::ClampToEdge,
        address_mode_v: bevy::image::ImageAddressMode::ClampToEdge,
        mag_filter: bevy::image::ImageFilterMode::Linear,
        min_filter: bevy::image::ImageFilterMode::Linear,
        ..default()
    })
}

/// Build the terrain + scatter materials (synthesizes the ground textures).
pub fn create(
    images: &mut Assets<Image>,
    terrain_mats: &mut Assets<TerrainMaterial>,
    scatter_mats: &mut Assets<ScatterMaterial>,
    pool: &SatPool,
) -> (Handle<TerrainMaterial>, GroundMaterials) {
    let t0 = std::time::Instant::now();
    let layers = ground_tex::all_layers(TEX_SIZE);
    let ah: Vec<&[u8]> = layers.iter().map(|l| l.albedo_height.as_slice()).collect();
    let nr: Vec<&[u8]> = layers.iter().map(|l| l.normal_rough_ao.as_slice()).collect();
    let albedo = images.add(array_image(&ah, TEX_SIZE, TextureFormat::Rgba8UnormSrgb, true));
    let normal = images.add(array_image(&nr, TEX_SIZE, TextureFormat::Rgba8Unorm, false));
    let (mchain, mlevels) = ground_tex::mip_chain(&ground_tex::macro_texture(MACRO_SIZE), MACRO_SIZE, false);
    let mut macro_img = Image::new_uninit(
        Extent3d { width: MACRO_SIZE, height: MACRO_SIZE, depth_or_array_layers: 1 },
        TextureDimension::D2,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    macro_img.data = Some(mchain);
    macro_img.texture_descriptor.mip_level_count = mlevels;
    macro_img.sampler = sampler(4);
    let macro_tex = images.add(macro_img);
    info!("ground textures: {} layers at {TEX_SIZE}px in {:?}", LAYERS, t0.elapsed());

    let (size, bump) = ground_tex::layer_params();
    let v4 = |a: &[f32; LAYERS], i: usize| Vec4::new(a[i * 4], a[i * 4 + 1], a[i * 4 + 2], a[i * 4 + 3]);
    let params = GroundParams {
        size: [v4(&size, 0), v4(&size, 1), v4(&size, 2)],
        bump: [v4(&bump, 0), v4(&bump, 1), v4(&bump, 2)],
        offset: Vec4::new(0.0, 0.0, 1.0, 0.0),
    };
    let terrain = terrain_mats.add(ExtendedMaterial {
        base: StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.95, reflectance: 0.3, ..default() },
        extension: GroundExt { params, albedo, normal, macro_tex, sat_512: pool.images[0].clone(), sat_256: pool.images[1].clone(), sat_128: pool.images[2].clone() },
    });
    let scatter = scatter_mats.add(ExtendedMaterial {
        base: StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.82,
            reflectance: 0.25,
            cull_mode: None,
            // Soft light through blades and petals.
            diffuse_transmission: 0.25,
            ..default()
        },
        extension: ScatterExt {
            params: ScatterParams { wind: Vec4::new(0.8, 0.0, 0.6, 1.0), ..default() },
        },
    });
    (terrain, GroundMaterials { scatter })
}

/// Accumulated texture-space offset so ground textures don't jump when the
/// floating origin moves. Meters, wrapped to a multiple of every layer's
/// repeat size.
#[derive(Resource, Default)]
struct UvOffset {
    offset: glam::DVec2,
    generation: u64,
    frame: Option<ed_geo::EnuFrame>,
}

fn track_rebase(origin: Res<crate::origin::WorldOrigin>, mut uv: ResMut<UvOffset>) {
    if uv.frame.is_none() {
        uv.frame = Some(origin.frame);
        uv.generation = origin.generation;
        return;
    }
    if origin.generation == uv.generation {
        return;
    }
    let old = uv.frame.unwrap();
    // Where the new origin sat in the old frame: texture coordinates there
    // must stay the same after the switch.
    let new_origin_ecef = origin.frame.render_to_ecef().transform_point3(glam::DVec3::ZERO);
    let p_old = old.ecef_to_render().transform_point3(new_origin_ecef);
    uv.offset += glam::DVec2::new(p_old.x, p_old.z);
    // 96 m is a multiple of every layer size (3, 4, 4.8, 6, 8 m).
    let wrap = 96.0 * 64.0;
    uv.offset = glam::DVec2::new(uv.offset.x.rem_euclid(wrap), uv.offset.y.rem_euclid(wrap));
    uv.frame = Some(origin.frame);
    uv.generation = origin.generation;
}

fn update_params(
    time: Res<Time>,
    uv: Res<UvOffset>,
    wm: Option<Res<crate::assets::WorldMaterials>>,
    gm: Option<Res<GroundMaterials>>,
    mut terrain_mats: ResMut<Assets<TerrainMaterial>>,
    mut scatter_mats: ResMut<Assets<ScatterMaterial>>,
    cam: Query<&GlobalTransform, (With<Camera3d>, Without<crate::globe::GlobeCamera>)>,
    mut last_offset: Local<Option<glam::DVec2>>,
) {
    if let Some(wm) = wm
        && *last_offset != Some(uv.offset)
        && let Some(mut m) = terrain_mats.get_mut(&wm.terrain)
    {
        m.extension.params.offset.x = uv.offset.x as f32;
        m.extension.params.offset.y = uv.offset.y as f32;
        *last_offset = Some(uv.offset);
    }
    let (Some(gm), Ok(cam)) = (gm, cam.single()) else { return };
    if let Some(mut m) = scatter_mats.get_mut(&gm.scatter) {
        let t = time.elapsed_secs_wrapped();
        let p = &mut m.extension.params;
        p.camera = cam.translation().extend(1.0);
        p.time = Vec4::new(t, p.time.x, 0.0, 0.0);
    }
}
