//! Interface enums -> Metal enums.

use crate::*;
use objc2_metal::*;

pub fn format(f: Format) -> MTLPixelFormat {
    use Format::*;
    match f {
        Rgba8Unorm => MTLPixelFormat::RGBA8Unorm,
        Bgra8Unorm => MTLPixelFormat::BGRA8Unorm,
        Rgba8Srgb => MTLPixelFormat::RGBA8Unorm_sRGB,
        Bgra8Srgb => MTLPixelFormat::BGRA8Unorm_sRGB,
        Rg8Unorm => MTLPixelFormat::RG8Unorm,
        R8Unorm => MTLPixelFormat::R8Unorm,
        R16Float => MTLPixelFormat::R16Float,
        Rg16Float => MTLPixelFormat::RG16Float,
        Rgba16Float => MTLPixelFormat::RGBA16Float,
        R32Float => MTLPixelFormat::R32Float,
        R32Uint => MTLPixelFormat::R32Uint,
        Rg32Uint => MTLPixelFormat::RG32Uint,
        R11G11B10Float => MTLPixelFormat::RG11B10Float,
        Rgb10A2Unorm => MTLPixelFormat::RGB10A2Unorm,
        Depth32Float => MTLPixelFormat::Depth32Float,
        Bc4Unorm => MTLPixelFormat::BC4_RUnorm,
        Bc5Unorm => MTLPixelFormat::BC5_RGUnorm,
        Bc6hUfloat => MTLPixelFormat::BC6H_RGBUfloat,
        Bc7Unorm => MTLPixelFormat::BC7_RGBAUnorm,
        Bc7Srgb => MTLPixelFormat::BC7_RGBAUnorm_sRGB,
        Astc4x4Unorm => MTLPixelFormat::ASTC_4x4_LDR,
        Astc4x4Srgb => MTLPixelFormat::ASTC_4x4_sRGB,
        Astc6x6Unorm => MTLPixelFormat::ASTC_6x6_LDR,
        Astc6x6Srgb => MTLPixelFormat::ASTC_6x6_sRGB,
    }
}

pub fn compare(c: Compare) -> MTLCompareFunction {
    match c {
        Compare::Never => MTLCompareFunction::Never,
        Compare::Less => MTLCompareFunction::Less,
        Compare::LessEqual => MTLCompareFunction::LessEqual,
        Compare::Equal => MTLCompareFunction::Equal,
        Compare::GreaterEqual => MTLCompareFunction::GreaterEqual,
        Compare::Greater => MTLCompareFunction::Greater,
        Compare::Always => MTLCompareFunction::Always,
    }
}

pub fn min_mag(f: Filter) -> MTLSamplerMinMagFilter {
    match f {
        Filter::Nearest => MTLSamplerMinMagFilter::Nearest,
        Filter::Linear => MTLSamplerMinMagFilter::Linear,
    }
}

pub fn mip(f: Filter) -> MTLSamplerMipFilter {
    match f {
        Filter::Nearest => MTLSamplerMipFilter::Nearest,
        Filter::Linear => MTLSamplerMipFilter::Linear,
    }
}

pub fn address(a: AddressMode) -> MTLSamplerAddressMode {
    match a {
        AddressMode::Repeat => MTLSamplerAddressMode::Repeat,
        AddressMode::Clamp => MTLSamplerAddressMode::ClampToEdge,
        AddressMode::Mirror => MTLSamplerAddressMode::MirrorRepeat,
    }
}

pub fn cull(c: Cull) -> MTLCullMode {
    match c {
        Cull::None => MTLCullMode::None,
        Cull::Back => MTLCullMode::Back,
        Cull::Front => MTLCullMode::Front,
    }
}

pub fn topology(t: Topology) -> (MTLPrimitiveType, MTLPrimitiveTopologyClass) {
    match t {
        Topology::Triangles => (MTLPrimitiveType::Triangle, MTLPrimitiveTopologyClass::Triangle),
        Topology::Lines => (MTLPrimitiveType::Line, MTLPrimitiveTopologyClass::Line),
        Topology::Points => (MTLPrimitiveType::Point, MTLPrimitiveTopologyClass::Point),
    }
}

/// (enabled, src rgb, dst rgb, src alpha, dst alpha)
pub fn blend(b: Blend) -> Option<(MTLBlendFactor, MTLBlendFactor, MTLBlendFactor, MTLBlendFactor)> {
    use MTLBlendFactor as F;
    match b {
        Blend::Opaque => None,
        Blend::Alpha => Some((F::SourceAlpha, F::OneMinusSourceAlpha, F::One, F::OneMinusSourceAlpha)),
        Blend::Additive => Some((F::One, F::One, F::One, F::One)),
        Blend::Premultiplied => Some((F::One, F::OneMinusSourceAlpha, F::One, F::OneMinusSourceAlpha)),
    }
}

pub fn load(l: LoadOp) -> MTLLoadAction {
    match l {
        LoadOp::Load => MTLLoadAction::Load,
        LoadOp::Clear(_) => MTLLoadAction::Clear,
        LoadOp::Discard => MTLLoadAction::DontCare,
    }
}

pub fn store(s: StoreOp) -> MTLStoreAction {
    match s {
        StoreOp::Store => MTLStoreAction::Store,
        StoreOp::Discard => MTLStoreAction::DontCare,
    }
}

pub fn texture_type(kind: TextureKind, samples: u32) -> MTLTextureType {
    match kind {
        TextureKind::D2 if samples > 1 => MTLTextureType::Type2DMultisample,
        TextureKind::D2 => MTLTextureType::Type2D,
        TextureKind::D2Array => MTLTextureType::Type2DArray,
        TextureKind::Cube => MTLTextureType::TypeCube,
        TextureKind::D3 => MTLTextureType::Type3D,
    }
}

pub fn ns(s: &str) -> objc2::rc::Retained<objc2_foundation::NSString> {
    objc2_foundation::NSString::from_str(s)
}
