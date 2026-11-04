//! Half-resolution cloud march (M0).
//!
//! The cloud raymarch costs ~5 ms at 2144x1206 on an M3 Pro and scales with
//! pixel count. Instead of the full-screen material in the transparent pass,
//! the main camera (`CloudView`) runs two fullscreen passes after the sky:
//! 1. `clouds.wesl` with `CLOUD_HALF` marches one texel per 2x2 block into a
//!    half-res colour target, and writes the depth it marched against.
//! 2. `clouds_up.wesl` upsamples it depth-aware onto the HDR view target.
//!
//! The uniform and noise volumes are read from the sky `CloudMaterial`, which
//! `clouds::update_uniforms` keeps current; that entity is hidden here.
//! `EW_CLOUD_FULL=1` keeps the old full-res material path (A/B).

use crate::clouds::{CloudLayer, CloudMaterial, CloudUniform};
use bevy::asset::embedded_asset;
use bevy::core_pipeline::core_3d::main_transparent_pass_3d;
use bevy::core_pipeline::schedule::Core3d;
use bevy::core_pipeline::FullscreenShader;
use bevy::prelude::*;
use bevy::render::camera::ExtractedCamera;
use bevy::render::extract_component::{ExtractComponent, ExtractComponentPlugin};
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::binding_types::{sampler, texture_2d, texture_3d, texture_depth_2d, uniform_buffer};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery};
use bevy::render::texture::{CachedTexture, GpuImage, TextureCache};
use bevy::render::view::{ExtractedView, ViewDepthStencilTexture, ViewTarget, ViewUniform, ViewUniformOffset, ViewUniforms};
use bevy::render::{Extract, ExtractSchedule, Render, RenderApp, RenderStartup, RenderSystems};

/// On the camera that gets half-res clouds.
#[derive(Component, Clone, Copy, ExtractComponent)]
#[extract_app(RenderApp)]
pub struct CloudView;

pub fn enabled() -> bool {
    std::env::var_os("EW_CLOUD_FULL").is_none()
}

pub struct CloudHalfPlugin;

impl Plugin for CloudHalfPlugin {
    fn build(&self, app: &mut App) {
        if !enabled() {
            return;
        }
        embedded_asset!(app, "clouds_up.wesl");
        app.add_plugins(ExtractComponentPlugin::<CloudView>::default()).add_systems(PostUpdate, hide_material_layer);
        let Some(render) = app.get_sub_app_mut(RenderApp) else { return };
        render
            .init_resource::<CloudFrame>()
            .add_systems(RenderStartup, init_pipelines)
            .add_systems(ExtractSchedule, extract_clouds)
            .add_systems(Render, (prepare_textures.in_set(RenderSystems::PrepareResources), prepare_bind_groups.in_set(RenderSystems::PrepareBindGroups)))
            .add_systems(Core3d, cloud_pass.after(bevy::pbr::RenderSkySet).before(main_transparent_pass_3d));
    }
}

/// The sky material entity stays (its asset carries the uniform) but is not drawn.
fn hide_material_layer(mut q: Query<&mut Visibility, (With<CloudLayer>, Added<CloudLayer>)>) {
    for mut v in &mut q {
        *v = Visibility::Hidden;
    }
}

/// This frame's cloud uniform and noise volumes, copied from the sky material.
#[derive(Resource, Default)]
struct CloudFrame {
    u: Option<CloudUniform>,
    shape: AssetId<Image>,
    detail: AssetId<Image>,
    buffer: UniformBuffer<CloudUniform>,
}

fn extract_clouds(
    mut frame: ResMut<CloudFrame>,
    layer: Extract<Query<&MeshMaterial3d<CloudMaterial>, With<CloudLayer>>>,
    mats: Extract<Res<Assets<CloudMaterial>>>,
) {
    frame.u = None;
    let Some(m) = layer.iter().next().and_then(|h| mats.get(&h.0)) else { return };
    frame.u = Some(m.u.clone());
    frame.shape = m.shape.id();
    frame.detail = m.detail.id();
}

// The main texture of an `Hdr` camera.
const HDR_FORMAT: TextureFormat = TextureFormat::Rgba16Float;
const HALF_FORMAT: TextureFormat = TextureFormat::Rgba16Float;
const DEPTH_FORMAT: TextureFormat = TextureFormat::R32Float;

#[derive(Resource)]
struct CloudPipelines {
    view_layout: BindGroupLayoutDescriptor,
    cloud_layout: BindGroupLayoutDescriptor,
    up_layout: BindGroupLayoutDescriptor,
    half: CachedRenderPipelineId,
    up: CachedRenderPipelineId,
}

fn init_pipelines(mut commands: Commands, assets: Res<AssetServer>, fullscreen: Res<FullscreenShader>, cache: Res<PipelineCache>) {
    let view_layout = BindGroupLayoutDescriptor::new(
        "cloud_half_view",
        &BindGroupLayoutEntries::sequential(ShaderStages::FRAGMENT, (uniform_buffer::<ViewUniform>(true), texture_depth_2d())),
    );
    let cloud_layout = BindGroupLayoutDescriptor::new(
        "cloud_half_cloud",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                uniform_buffer::<CloudUniform>(false),
                texture_3d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                texture_3d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
            ),
        ),
    );
    let up_layout = BindGroupLayoutDescriptor::new(
        "cloud_half_up",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                uniform_buffer::<ViewUniform>(true),
                texture_depth_2d(),
                texture_2d(TextureSampleType::Float { filterable: false }),
                texture_2d(TextureSampleType::Float { filterable: false }),
            ),
        ),
    );
    let half = cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("cloud_half".into()),
        layout: vec![view_layout.clone(), cloud_layout.clone()],
        vertex: fullscreen.to_vertex_state(),
        fragment: Some(FragmentState {
            shader: assets.load("embedded://earthdrop/clouds.wesl"),
            shader_defs: vec!["CLOUD_HALF".into()],
            entry_point: Some("fragment_half".into()),
            constants: default(),
            targets: vec![
                Some(ColorTargetState { format: HALF_FORMAT, blend: None, write_mask: ColorWrites::ALL }),
                Some(ColorTargetState { format: DEPTH_FORMAT, blend: None, write_mask: ColorWrites::ALL }),
            ],
        }),
        ..default()
    });
    let up = cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("cloud_half_up".into()),
        layout: vec![up_layout.clone()],
        vertex: fullscreen.to_vertex_state(),
        fragment: Some(FragmentState {
            shader: assets.load("embedded://earthdrop/clouds_up.wesl"),
            targets: vec![Some(ColorTargetState {
                format: HDR_FORMAT,
                blend: Some(BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: ColorWrites::ALL,
            })],
            ..default()
        }),
        ..default()
    });
    commands.insert_resource(CloudPipelines { view_layout, cloud_layout, up_layout, half, up });
}

#[derive(Component)]
struct CloudTextures {
    color: CachedTexture,
    depth: CachedTexture,
}

fn prepare_textures(
    mut commands: Commands,
    device: Res<RenderDevice>,
    mut cache: ResMut<TextureCache>,
    views: Query<(Entity, &ExtractedCamera, &ExtractedView), With<CloudView>>,
) {
    for (e, cam, view) in &views {
        let Some(size) = cam.physical_viewport_size else { continue };
        // The composite targets the HDR main texture only.
        if view.target_format != HDR_FORMAT {
            continue;
        }
        let size = Extent3d { width: size.x.div_ceil(2).max(1), height: size.y.div_ceil(2).max(1), depth_or_array_layers: 1 };
        let tex = |label: &'static str, format| TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        commands.entity(e).insert(CloudTextures {
            color: cache.get(&device, tex("cloud_half_color", HALF_FORMAT)),
            depth: cache.get(&device, tex("cloud_half_depth", DEPTH_FORMAT)),
        });
    }
}

#[derive(Component)]
struct CloudBindGroups {
    view: BindGroup,
    cloud: BindGroup,
    up: BindGroup,
}

fn prepare_bind_groups(
    mut commands: Commands,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    pipelines: Res<CloudPipelines>,
    cache: Res<PipelineCache>,
    mut frame: ResMut<CloudFrame>,
    images: Res<RenderAssets<GpuImage>>,
    view_uniforms: Res<ViewUniforms>,
    views: Query<(Entity, &CloudTextures, &ViewDepthStencilTexture), With<CloudView>>,
) {
    let frame = &mut *frame;
    let Some(u) = frame.u.clone() else { return };
    let (Some(shape), Some(detail)) = (images.get(frame.shape), images.get(frame.detail)) else { return };
    let Some(view_binding) = view_uniforms.uniforms.binding() else { return };
    frame.buffer.set(u);
    frame.buffer.write_buffer(&device, &queue);
    let Some(cloud_binding) = frame.buffer.binding() else { return };
    let cloud = device.create_bind_group(
        "cloud_half_cloud",
        &cache.get_bind_group_layout(&pipelines.cloud_layout),
        &BindGroupEntries::sequential((cloud_binding, &shape.texture_view, &shape.sampler, &detail.texture_view, &detail.sampler)),
    );
    for (e, tex, depth) in &views {
        let Some(depth_view) = depth.attachment.depth_stencil_views().depth_only_view() else { continue };
        let view = device.create_bind_group(
            "cloud_half_view",
            &cache.get_bind_group_layout(&pipelines.view_layout),
            &BindGroupEntries::sequential((view_binding.clone(), depth_view)),
        );
        let up = device.create_bind_group(
            "cloud_half_up",
            &cache.get_bind_group_layout(&pipelines.up_layout),
            &BindGroupEntries::sequential((view_binding.clone(), depth_view, &tex.color.default_view, &tex.depth.default_view)),
        );
        commands.entity(e).insert(CloudBindGroups { view, cloud: cloud.clone(), up });
    }
}

fn cloud_pass(
    view: ViewQuery<(&ViewTarget, &CloudTextures, &CloudBindGroups, &ViewUniformOffset)>,
    pipelines: Res<CloudPipelines>,
    cache: Res<PipelineCache>,
    mut ctx: RenderContext,
) {
    let (target, tex, groups, offset) = view.into_inner();
    let (Some(half), Some(up)) = (cache.get_render_pipeline(pipelines.half), cache.get_render_pipeline(pipelines.up)) else { return };
    let encoder = ctx.command_encoder();
    {
        let attach = |v| Some(RenderPassColorAttachment { view: v, depth_slice: None, resolve_target: None, ops: Operations { load: LoadOp::Clear(default()), store: StoreOp::Store } });
        let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("cloud_half"),
            color_attachments: &[attach(&tex.color.default_view), attach(&tex.depth.default_view)],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(half);
        pass.set_bind_group(0, &groups.view, &[offset.offset]);
        pass.set_bind_group(1, &groups.cloud, &[]);
        pass.draw(0..3, 0..1);
    }
    let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
        label: Some("cloud_half_up"),
        color_attachments: &[Some(target.get_color_attachment())],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_pipeline(up);
    pass.set_bind_group(0, &groups.up, &[offset.offset]);
    pass.draw(0..3, 0..1);
}
