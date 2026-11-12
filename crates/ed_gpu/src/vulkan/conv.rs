//! RHI enums to Vulkan values.

use ash::vk;

use crate::*;

pub fn format(f: Format) -> vk::Format {
    use Format::*;
    match f {
        Rgba8Unorm => vk::Format::R8G8B8A8_UNORM,
        Bgra8Unorm => vk::Format::B8G8R8A8_UNORM,
        Rgba8Srgb => vk::Format::R8G8B8A8_SRGB,
        Bgra8Srgb => vk::Format::B8G8R8A8_SRGB,
        Rg8Unorm => vk::Format::R8G8_UNORM,
        R8Unorm => vk::Format::R8_UNORM,
        R16Float => vk::Format::R16_SFLOAT,
        Rg16Float => vk::Format::R16G16_SFLOAT,
        Rgba16Float => vk::Format::R16G16B16A16_SFLOAT,
        R32Float => vk::Format::R32_SFLOAT,
        R32Uint => vk::Format::R32_UINT,
        Rg32Uint => vk::Format::R32G32_UINT,
        R11G11B10Float => vk::Format::B10G11R11_UFLOAT_PACK32,
        // Red in the low bits, as MTLPixelFormatRGB10A2Unorm.
        Rgb10A2Unorm => vk::Format::A2B10G10R10_UNORM_PACK32,
        Depth32Float => vk::Format::D32_SFLOAT,
        Bc4Unorm => vk::Format::BC4_UNORM_BLOCK,
        Bc5Unorm => vk::Format::BC5_UNORM_BLOCK,
        Bc6hUfloat => vk::Format::BC6H_UFLOAT_BLOCK,
        Bc7Unorm => vk::Format::BC7_UNORM_BLOCK,
        Bc7Srgb => vk::Format::BC7_SRGB_BLOCK,
        Astc4x4Unorm => vk::Format::ASTC_4X4_UNORM_BLOCK,
        Astc4x4Srgb => vk::Format::ASTC_4X4_SRGB_BLOCK,
        Astc6x6Unorm => vk::Format::ASTC_6X6_UNORM_BLOCK,
        Astc6x6Srgb => vk::Format::ASTC_6X6_SRGB_BLOCK,
    }
}

pub fn aspect(f: Format) -> vk::ImageAspectFlags {
    if f.is_depth() { vk::ImageAspectFlags::DEPTH } else { vk::ImageAspectFlags::COLOR }
}

pub fn view_type(kind: TextureKind) -> vk::ImageViewType {
    match kind {
        TextureKind::D2 => vk::ImageViewType::TYPE_2D,
        TextureKind::D2Array => vk::ImageViewType::TYPE_2D_ARRAY,
        TextureKind::Cube => vk::ImageViewType::CUBE,
        TextureKind::D3 => vk::ImageViewType::TYPE_3D,
    }
}

pub fn compare(c: Compare) -> vk::CompareOp {
    match c {
        Compare::Never => vk::CompareOp::NEVER,
        Compare::Less => vk::CompareOp::LESS,
        Compare::LessEqual => vk::CompareOp::LESS_OR_EQUAL,
        Compare::Equal => vk::CompareOp::EQUAL,
        Compare::GreaterEqual => vk::CompareOp::GREATER_OR_EQUAL,
        Compare::Greater => vk::CompareOp::GREATER,
        Compare::Always => vk::CompareOp::ALWAYS,
    }
}

pub fn filter(f: Filter) -> vk::Filter {
    match f {
        Filter::Nearest => vk::Filter::NEAREST,
        Filter::Linear => vk::Filter::LINEAR,
    }
}

pub fn mip(f: Filter) -> vk::SamplerMipmapMode {
    match f {
        Filter::Nearest => vk::SamplerMipmapMode::NEAREST,
        Filter::Linear => vk::SamplerMipmapMode::LINEAR,
    }
}

pub fn address(a: AddressMode) -> vk::SamplerAddressMode {
    match a {
        AddressMode::Repeat => vk::SamplerAddressMode::REPEAT,
        AddressMode::Clamp => vk::SamplerAddressMode::CLAMP_TO_EDGE,
        AddressMode::Mirror => vk::SamplerAddressMode::MIRRORED_REPEAT,
    }
}

pub fn cull(c: Cull) -> vk::CullModeFlags {
    match c {
        Cull::None => vk::CullModeFlags::NONE,
        Cull::Back => vk::CullModeFlags::BACK,
        Cull::Front => vk::CullModeFlags::FRONT,
    }
}

pub fn topology(t: Topology) -> vk::PrimitiveTopology {
    match t {
        Topology::Triangles => vk::PrimitiveTopology::TRIANGLE_LIST,
        Topology::Lines => vk::PrimitiveTopology::LINE_LIST,
        Topology::Points => vk::PrimitiveTopology::POINT_LIST,
    }
}

pub fn blend(b: Blend) -> vk::PipelineColorBlendAttachmentState {
    use vk::BlendFactor as F;
    let s = vk::PipelineColorBlendAttachmentState::default().color_write_mask(vk::ColorComponentFlags::RGBA);
    let (src, dst, sa, da) = match b {
        Blend::Opaque => return s,
        Blend::Alpha => (F::SRC_ALPHA, F::ONE_MINUS_SRC_ALPHA, F::ONE, F::ONE_MINUS_SRC_ALPHA),
        Blend::Additive => (F::ONE, F::ONE, F::ONE, F::ONE),
        Blend::Premultiplied => (F::ONE, F::ONE_MINUS_SRC_ALPHA, F::ONE, F::ONE_MINUS_SRC_ALPHA),
    };
    s.blend_enable(true)
        .src_color_blend_factor(src)
        .dst_color_blend_factor(dst)
        .color_blend_op(vk::BlendOp::ADD)
        .src_alpha_blend_factor(sa)
        .dst_alpha_blend_factor(da)
        .alpha_blend_op(vk::BlendOp::ADD)
}

pub fn load(l: LoadOp) -> vk::AttachmentLoadOp {
    match l {
        LoadOp::Load => vk::AttachmentLoadOp::LOAD,
        LoadOp::Clear(_) => vk::AttachmentLoadOp::CLEAR,
        LoadOp::Discard => vk::AttachmentLoadOp::DONT_CARE,
    }
}

pub fn store(s: StoreOp) -> vk::AttachmentStoreOp {
    match s {
        StoreOp::Store => vk::AttachmentStoreOp::STORE,
        StoreOp::Discard => vk::AttachmentStoreOp::DONT_CARE,
    }
}

/// How a resource in a state is accessed: pipeline stages, access mask, image layout.
#[derive(Clone, Copy)]
pub struct Access {
    pub stages: vk::PipelineStageFlags2,
    pub access: vk::AccessFlags2,
    pub layout: vk::ImageLayout,
}

const WRITES: vk::AccessFlags2 = vk::AccessFlags2::from_raw(
    vk::AccessFlags2::SHADER_WRITE.as_raw()
        | vk::AccessFlags2::SHADER_STORAGE_WRITE.as_raw()
        | vk::AccessFlags2::COLOR_ATTACHMENT_WRITE.as_raw()
        | vk::AccessFlags2::DEPTH_STENCIL_ATTACHMENT_WRITE.as_raw()
        | vk::AccessFlags2::TRANSFER_WRITE.as_raw()
        | vk::AccessFlags2::HOST_WRITE.as_raw()
        | vk::AccessFlags2::MEMORY_WRITE.as_raw(),
);

impl Access {
    /// The access to make available when leaving this state (writes only).
    pub fn src_access(self) -> vk::AccessFlags2 {
        self.access & WRITES
    }
}

/// The resource a state is resolved for: buffers have no layout; a texture's `ShaderRead`
/// layout depends on its usage (see `sampled_layout`).
#[derive(Clone, Copy)]
pub enum Res {
    Buffer,
    Texture { sampled_layout: vk::ImageLayout, surface: bool },
}

const SHADERS: vk::PipelineStageFlags2 = vk::PipelineStageFlags2::from_raw(
    vk::PipelineStageFlags2::VERTEX_SHADER.as_raw() | vk::PipelineStageFlags2::FRAGMENT_SHADER.as_raw() | vk::PipelineStageFlags2::COMPUTE_SHADER.as_raw(),
);
const TESTS: vk::PipelineStageFlags2 =
    vk::PipelineStageFlags2::from_raw(vk::PipelineStageFlags2::EARLY_FRAGMENT_TESTS.as_raw() | vk::PipelineStageFlags2::LATE_FRAGMENT_TESTS.as_raw());

/// Layout a texture with this usage is in while in `ShaderRead`, and the layout its
/// sampled-image descriptor is written with. Writable textures stay in `GENERAL` so one
/// descriptor serves both `ShaderRead` and `ShaderWrite`; depth textures are sampled in
/// `DEPTH_STENCIL_READ_ONLY_OPTIMAL` (no separateDepthStencilLayouts needed), which is also `DepthRead`.
pub fn sampled_layout(usage: &Usage, format: Format) -> vk::ImageLayout {
    if usage.shader_write {
        vk::ImageLayout::GENERAL
    } else if format.is_depth() {
        vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL
    } else {
        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL
    }
}

pub fn access(state: ResourceState, res: Res) -> Access {
    use ResourceState::*;
    use vk::{AccessFlags2 as A, ImageLayout as L, PipelineStageFlags2 as S};
    let (stages, access, layout) = match (state, res) {
        // A fresh swapchain image: the acquire semaphore is waited at ALL_COMMANDS, so the first
        // barrier chains to it through the same stage.
        (Undefined, Res::Texture { surface: true, .. }) => (S::ALL_COMMANDS, A::NONE, L::UNDEFINED),
        (Undefined, _) => (S::NONE, A::NONE, L::UNDEFINED),
        (ShaderRead, Res::Buffer) => (SHADERS | S::INDEX_INPUT, A::SHADER_READ | A::INDEX_READ, L::UNDEFINED),
        (ShaderRead, Res::Texture { sampled_layout, .. }) => (SHADERS, A::SHADER_READ, sampled_layout),
        (ShaderWrite, _) => (S::FRAGMENT_SHADER | S::COMPUTE_SHADER, A::SHADER_READ | A::SHADER_WRITE, L::GENERAL),
        (RenderTarget, _) => (S::COLOR_ATTACHMENT_OUTPUT, A::COLOR_ATTACHMENT_READ | A::COLOR_ATTACHMENT_WRITE, L::COLOR_ATTACHMENT_OPTIMAL),
        (DepthWrite, _) => (TESTS, A::DEPTH_STENCIL_ATTACHMENT_READ | A::DEPTH_STENCIL_ATTACHMENT_WRITE, L::DEPTH_STENCIL_ATTACHMENT_OPTIMAL),
        (DepthRead, _) => (TESTS | S::FRAGMENT_SHADER | S::COMPUTE_SHADER, A::DEPTH_STENCIL_ATTACHMENT_READ | A::SHADER_READ, L::DEPTH_STENCIL_READ_ONLY_OPTIMAL),
        (IndirectArgs, _) => (S::DRAW_INDIRECT, A::INDIRECT_COMMAND_READ, L::UNDEFINED),
        (CopySrc, _) => (S::ALL_TRANSFER, A::TRANSFER_READ, L::TRANSFER_SRC_OPTIMAL),
        (CopyDst, _) => (S::ALL_TRANSFER, A::TRANSFER_WRITE, L::TRANSFER_DST_OPTIMAL),
        (Present, _) => (S::NONE, A::NONE, L::PRESENT_SRC_KHR),
    };
    Access { stages, access, layout }
}

pub fn result(r: vk::Result) -> Error {
    match r {
        vk::Result::ERROR_OUT_OF_DEVICE_MEMORY | vk::Result::ERROR_OUT_OF_HOST_MEMORY | vk::Result::ERROR_OUT_OF_POOL_MEMORY => Error::OutOfMemory,
        vk::Result::ERROR_DEVICE_LOST => Error::DeviceLost,
        vk::Result::ERROR_OUT_OF_DATE_KHR | vk::Result::ERROR_SURFACE_LOST_KHR => Error::SurfaceLost,
        r => Error::Backend(format!("{r:?}")),
    }
}
