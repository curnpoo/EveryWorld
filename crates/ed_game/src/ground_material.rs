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

const TERRAIN_SHADER: &str = "embedded://everyworld/shaders/terrain.wesl";
const SCATTER_SHADER: &str = "embedded://everyworld/shaders/ground_scatter.wesl";

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
    /// Far shadows (`far_shadows.rs`): Earth centre in render space (xyz);
    /// w = 1 when enabled.
    pub far_c: Vec4,
    /// Far shadows: Earth's pole axis in render space.
    pub far_p: Vec4,
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
    /// Far-shadow pool (`far_shadows.rs`): horizon maps (RGBA8) and surface
    /// maps (RGBA16F), sharing one layer index per tile. Sampler 107.
    #[texture(110, dimension = "2d_array")]
    pub far_horizon: Handle<Image>,
    #[texture(111, dimension = "2d_array")]
    pub far_surface: Handle<Image>,
    /// Baked DEM detail (`ed_world::detail_normal`), same class/layer as the
    /// satellite colour. Sampler 107.
    #[texture(112, dimension = "2d_array")]
    pub detail_512: Handle<Image>,
    #[texture(113, dimension = "2d_array")]
    pub detail_256: Handle<Image>,
    #[texture(114, dimension = "2d_array")]
    pub detail_128: Handle<Image>,
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
/// `ed_world::tilegen` (`sat_levels`: z<=6 -> 1024, z7-9 -> 512, z>=10 -> 256).
/// Layer counts cover the ~90 resident tiles plus the coarse tiles kept during
/// hole-free swaps; if a class runs out, the tile simply renders without
/// satellite colour. The 1024 class exists so coarse tiles stay sharp from high
/// altitude. Memory is fixed: ~135 MB + 85 MB + 85 MB plus mips.
pub const SAT_CLASSES: [(u32, u32); 3] = [(1024, 32), (512, 80), (256, 320)];

/// Tag bit: the satellite layer also has baked DEM detail.
pub const DETAIL_BIT: u32 = 1 << 23;

/// Tag value for "no satellite colour".
pub const SAT_NONE: u32 = 0;

/// Fixed pool of satellite layers shared by every terrain tile.
///
/// The layer index rides on the terrain entity's `MeshTag`:
/// `(class + 1) << 24 | layer`, 0 = none. Bits 12-23 carry the far-shadow
/// layer + 1 (`far_shadows::FarPool`), so the sat layer is `tag & 0xFFF`. Uploads go straight to the GPU array
/// with `queue.write_texture` in the render world, so adding a tile never
/// creates an image or material asset and never re-uploads the whole array.
#[derive(Resource)]
pub struct SatPool {
    pub images: [Handle<Image>; 3],
    /// Baked DEM detail, one array per class, sharing the satellite layer.
    pub detail: [Handle<Image>; 3],
    free: [Vec<u32>; 3],
    uploads: Vec<LayerUpload>,
    /// CPU copies of the satellite layers for the native renderer: layer key -> (generation, size, RGBA8).
    pub cpu: std::collections::HashMap<u32, (u64, u32, std::sync::Arc<Vec<u8>>)>,
    /// Native renderer only: CPU copy of each layer's baked detail, same key as `cpu`.
    pub cpu_detail: std::collections::HashMap<u32, std::sync::Arc<Vec<u8>>>,
    gen_counter: u64,
}

/// One texture-array layer waiting for `queue.write_texture`.
pub(crate) struct LayerUpload {
    pub image: AssetId<Image>,
    pub layer: u32,
    pub width: u32,
    pub height: u32,
    pub bytes_per_px: u32,
    pub data: Vec<u8>,
}

impl SatPool {
    pub fn new(images: &mut Assets<Image>) -> Self {
        let detail = SAT_CLASSES.map(|(n, layers)| images.add(sat_array_image(n, layers)));
        let images = SAT_CLASSES.map(|(n, layers)| images.add(sat_array_image(n, layers)));
        let free = SAT_CLASSES.map(|(_, layers)| (0..layers).rev().collect());
        Self { images, detail, free, uploads: Vec::new(), cpu: Default::default(), cpu_detail: Default::default(), gen_counter: 0 }
    }

    /// Reserve a layer for `data` (`size`^2 RGBA8) and queue its upload.
    /// Returns the `MeshTag` value, or [`SAT_NONE`] if the class is full or
    /// the size is unknown.
    /// `detail` (same size) fills the matching layer of the detail array;
    /// without it that layer keeps stale data, so the shader is told via
    /// bit 23 of the tag (set = detail present).
    pub fn alloc(&mut self, size: u32, data: Vec<u8>, detail: Option<Vec<u8>>) -> u32 {
        let Some(class) = SAT_CLASSES.iter().position(|(n, _)| *n == size) else { return SAT_NONE };
        if data.len() != (size * size * 4) as usize {
            return SAT_NONE;
        }
        let Some(layer) = self.free[class].pop() else {
            warn!("satellite pool full for {size}px tiles; tile drawn without satellite colour");
            return SAT_NONE;
        };
        if crate::native::enabled() {
            self.gen_counter += 1;
            self.cpu.insert(((class as u32 + 1) << 12) | layer, (self.gen_counter, size, std::sync::Arc::new(data.clone())));
        }
        self.uploads.push(LayerUpload { image: self.images[class].id(), layer, width: size, height: size, bytes_per_px: 4, data });
        let mut tag = ((class as u32 + 1) << 24) | layer;
        if let Some(d) = detail.filter(|d| d.len() == (size * size * 4) as usize) {
            if crate::native::enabled() {
                self.cpu_detail.insert(((class as u32 + 1) << 12) | layer, std::sync::Arc::new(d.clone()));
            }
            self.uploads.push(LayerUpload { image: self.detail[class].id(), layer, width: size, height: size, bytes_per_px: 4, data: d });
            tag |= DETAIL_BIT;
        }
        tag
    }

    /// Return a layer to the pool once its tile is gone.
    pub fn release(&mut self, tag: u32) {
        let class = (tag >> 24) as usize;
        if class == 0 || class > SAT_CLASSES.len() {
            return;
        }
        let layer = tag & 0xFFF;
        self.cpu.remove(&(((class as u32) << 12) | layer));
        self.cpu_detail.remove(&(((class as u32) << 12) | layer));
        // Drop a still-queued upload for this slot: the slot is free again.
        let (img, det) = (self.images[class - 1].id(), self.detail[class - 1].id());
        self.uploads.retain(|u| !((u.image == img || u.image == det) && u.layer == layer));
        self.free[class - 1].push(layer);
    }

    pub fn free_layers(&self) -> [usize; 3] {
        [self.free[0].len(), self.free[1].len(), self.free[2].len()]
    }
}

/// Render-world queue of satellite layers waiting for their GPU array.
#[derive(Resource, Default)]
struct RenderSatUploads(Vec<LayerUpload>);

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
    write_layers(&mut pending.0, &gpu_images, &queue);
}

/// Write queued layers whose array is on the GPU; the rest wait.
pub(crate) fn write_layers(
    pending: &mut Vec<LayerUpload>,
    gpu_images: &bevy::render::render_asset::RenderAssets<bevy::render::texture::GpuImage>,
    queue: &bevy::render::renderer::RenderQueue,
) {
    use bevy::render::render_resource::{Origin3d, TexelCopyBufferLayout, TexelCopyTextureInfo, TextureAspect};
    // Uploads whose array isn't on the GPU yet (first frames) wait.
    pending.retain(|u| {
        let Some(gpu) = gpu_images.get(u.image) else { return true };
        queue.write_texture(
            TexelCopyTextureInfo { texture: &gpu.texture, mip_level: 0, origin: Origin3d { x: 0, y: 0, z: u.layer }, aspect: TextureAspect::All },
            &u.data,
            TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(u.width * u.bytes_per_px), rows_per_image: Some(u.height) },
            Extent3d { width: u.width, height: u.height, depth_or_array_layers: 1 },
        );
        false
    });
}

/// One satellite class: an uninitialised `n`x`n`x`layers` array, filled per
/// layer by [`write_sat_uploads`].
fn sat_array_image(n: u32, layers: u32) -> Image {
    layer_array_image(n, n, layers, TextureFormat::Rgba8Unorm)
}

/// An uninitialised 2D array (clamped, linear), filled per layer by [`write_layers`].
pub(crate) fn layer_array_image(w: u32, h: u32, layers: u32, format: TextureFormat) -> Image {
    let mut img = Image::new_uninit(
        Extent3d { width: w, height: h, depth_or_array_layers: layers },
        TextureDimension::D2,
        format,
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

/// CPU copy of the ground layer data for the native Metal renderer (uploaded once).
pub struct GroundCpu {
    pub size: usize,
    pub albedo: Vec<Vec<u8>>,
    pub normal: Vec<Vec<u8>>,
    pub macro_tex: Vec<u8>,
    pub macro_size: usize,
    pub sizes: [f32; 12],
    pub bump: [f32; 12],
}
pub static GROUND_CPU: std::sync::OnceLock<GroundCpu> = std::sync::OnceLock::new();

/// Build the terrain + scatter materials (synthesizes the ground textures).
pub fn create(
    images: &mut Assets<Image>,
    terrain_mats: &mut Assets<TerrainMaterial>,
    scatter_mats: &mut Assets<ScatterMaterial>,
    pool: &SatPool,
    far: &crate::far_shadows::FarPool,
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
    if crate::native::enabled() {
        let mut sz = [0f32; 12];
        let mut bp = [0f32; 12];
        sz.copy_from_slice(&size[..12]);
        bp.copy_from_slice(&bump[..12]);
        let _ = GROUND_CPU.set(GroundCpu {
            size: TEX_SIZE as usize,
            albedo: ah.iter().map(|l| l.to_vec()).collect(),
            normal: nr.iter().map(|l| l.to_vec()).collect(),
            macro_tex: ground_tex::macro_texture(MACRO_SIZE),
            macro_size: MACRO_SIZE as usize,
            sizes: sz,
            bump: bp,
        });
    }
    let v4 = |a: &[f32; LAYERS], i: usize| Vec4::new(a[i * 4], a[i * 4 + 1], a[i * 4 + 2], a[i * 4 + 3]);
    let params = GroundParams {
        size: [v4(&size, 0), v4(&size, 1), v4(&size, 2)],
        bump: [v4(&bump, 0), v4(&bump, 1), v4(&bump, 2)],
        offset: Vec4::new(0.0, 0.0, 1.0, 0.0),
        far_c: Vec4::ZERO,
        far_p: Vec4::Y,
    };
    let terrain = terrain_mats.add(ExtendedMaterial {
        base: StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.95, reflectance: 0.3, ..default() },
        extension: GroundExt { params, albedo, normal, macro_tex, sat_512: pool.images[0].clone(), sat_256: pool.images[1].clone(), sat_128: pool.images[2].clone(),
            far_horizon: far.horizon.clone(),
            far_surface: far.surface.clone(),
            detail_512: pool.detail[0].clone(), detail_256: pool.detail[1].clone(), detail_128: pool.detail[2].clone(),
        },
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
    origin: Res<crate::origin::WorldOrigin>,
    mut last_gen: Local<Option<u64>>,
) {
    // Far shadows build a local up/north per fragment from the Earth's centre
    // and pole in render space; both move only when the origin re-anchors.
    if let Some(wm) = &wm
        && *last_gen != Some(origin.generation)
        && let Some(mut m) = terrain_mats.get_mut(&wm.terrain)
    {
        let (c, p) = ed_world::farshadow::earth_axes(origin.frame.ecef_to_render());
        let on = if ed_world::farshadow::enabled() { 1.0 } else { 0.0 };
        m.extension.params.far_c = c.as_vec3().extend(on);
        let dbg = if crate::far_shadows::DEBUG.load(std::sync::atomic::Ordering::Relaxed) { 1.0 } else { 0.0 };
        m.extension.params.far_p = p.as_vec3().extend(dbg);
        *last_gen = Some(origin.generation);
    }
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
