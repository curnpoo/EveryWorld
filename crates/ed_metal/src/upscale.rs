//! MetalFX temporal upscaler: the world renders at a reduced internal resolution with a jittered projection,
//! camera motion vectors are derived from depth, and MetalFX reconstructs the full-resolution HDR image
//! (which the resolve pass then grades). Off when `render_scale >= 1`; `EW_NATIVE_SCALE=0.67` sets the scale.

use crate::MetalContext;
use glam::Mat4;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_foundation::NSString;
use objc2_metal::*;
use objc2_metal_fx::{MTLFXTemporalScaler, MTLFXTemporalScalerBase, MTLFXTemporalScalerDescriptor};
use std::ffi::c_void;
use std::ptr::NonNull;

type Tex = Retained<ProtocolObject<dyn MTLTexture>>;

#[repr(C)]
struct Mu {
    inv_cur: [f32; 16],
    prev: [f32; 16],
    size: [f32; 4],
}

pub struct Upscaler {
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    motion_pso: Retained<ProtocolObject<dyn MTLComputePipelineState>>,
    scaler: Option<Retained<ProtocolObject<dyn MTLFXTemporalScaler>>>,
    dims: (usize, usize, usize, usize),
    motion: Option<Tex>,
    out: Option<Tex>,
    prev_vp: Option<Mat4>,
    pub frame: u32,
}

/// Halton(2,3) sample `i` (1-based) centred on 0, in pixels.
pub fn jitter(i: u32) -> (f32, f32) {
    fn h(mut i: u32, b: u32) -> f32 {
        let (mut f, mut r) = (1.0, 0.0);
        while i > 0 {
            f /= b as f32;
            r += f * (i % b) as f32;
            i /= b;
        }
        r
    }
    let k = (i % 32) + 1;
    (h(k, 2) - 0.5, h(k, 3) - 0.5)
}

impl Upscaler {
    pub fn new(ctx: &MetalContext) -> Option<Self> {
        let desc = unsafe { MTLFXTemporalScalerDescriptor::new() };
        if !unsafe { MTLFXTemporalScalerDescriptor::supportsDevice(&ctx.device) } {
            log::warn!("MetalFX temporal scaler unsupported on this device");
            return None;
        }
        drop(desc);
        let lib = ctx.library(include_str!("../shaders/upscale.metal")).map_err(|e| log::error!("upscale shader: {e}")).ok()?;
        let f = lib.newFunctionWithName(&NSString::from_str("motion_cs"))?;
        let motion_pso = ctx.device.newComputePipelineStateWithFunction_error(&f).ok()?;
        Some(Self { device: ctx.device.clone(), motion_pso, scaler: None, dims: (0, 0, 0, 0), motion: None, out: None, prev_vp: None, frame: 0 })
    }

    fn tex(&self, fmt: MTLPixelFormat, w: usize, h: usize, usage: MTLTextureUsage) -> Tex {
        let d = unsafe { MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(fmt, w, h, false) };
        d.setUsage(usage);
        d.setStorageMode(MTLStorageMode::Private);
        self.device.newTextureWithDescriptor(&d).unwrap()
    }

    /// Upscale `hdr` (internal res) to `(ow, oh)`. `vp` is the UNJITTERED view-projection; `jit` the pixel jitter
    /// applied this frame. Returns the output texture.
    #[allow(clippy::too_many_arguments)]
    pub fn encode(&mut self, cb: &ProtocolObject<dyn MTLCommandBuffer>, hdr: &Tex, depth: &Tex, ow: usize, oh: usize, vp: Mat4, jit: (f32, f32), cam_jump: bool) -> Option<Tex> {
        let (iw, ih) = (hdr.width(), hdr.height());
        let mut reset = cam_jump || self.prev_vp.is_none();
        if self.dims != (iw, ih, ow, oh) || self.scaler.is_none() {
            let d = unsafe { MTLFXTemporalScalerDescriptor::new() };
            unsafe {
                d.setInputWidth(iw);
                d.setInputHeight(ih);
                d.setOutputWidth(ow);
                d.setOutputHeight(oh);
                d.setColorTextureFormat(MTLPixelFormat::RGBA16Float);
                d.setOutputTextureFormat(MTLPixelFormat::RGBA16Float);
                d.setDepthTextureFormat(MTLPixelFormat::Depth32Float);
                d.setMotionTextureFormat(MTLPixelFormat::RG16Float);
                d.setAutoExposureEnabled(true);
            }
            self.scaler = unsafe { d.newTemporalScalerWithDevice(&self.device) };
            self.scaler.as_ref()?;
            self.motion = Some(self.tex(MTLPixelFormat::RG16Float, iw, ih, MTLTextureUsage::ShaderRead | MTLTextureUsage::ShaderWrite));
            self.out = Some(self.tex(MTLPixelFormat::RGBA16Float, ow, oh, MTLTextureUsage::ShaderRead | MTLTextureUsage::ShaderWrite | MTLTextureUsage::RenderTarget));
            self.dims = (iw, ih, ow, oh);
            reset = true;
        }
        let (motion, out, scaler) = (self.motion.clone()?, self.out.clone()?, self.scaler.clone()?);
        let prev = self.prev_vp.unwrap_or(vp);
        let u = Mu { inv_cur: vp.inverse().to_cols_array(), prev: prev.to_cols_array(), size: [iw as f32, ih as f32, 1.0 / iw as f32, 1.0 / ih as f32] };
        if let Some(enc) = cb.computeCommandEncoder() {
            enc.setComputePipelineState(&self.motion_pso);
            unsafe {
                enc.setTexture_atIndex(Some(&**depth), 0);
                enc.setTexture_atIndex(Some(&*motion), 1);
                enc.setBytes_length_atIndex(NonNull::new(&u as *const Mu as *mut c_void).unwrap(), std::mem::size_of::<Mu>(), 0);
            }
            enc.dispatchThreadgroups_threadsPerThreadgroup(MTLSize { width: iw.div_ceil(8), height: ih.div_ceil(8), depth: 1 }, MTLSize { width: 8, height: 8, depth: 1 });
            enc.endEncoding();
        }
        unsafe {
            scaler.setColorTexture(Some(&**hdr));
            scaler.setDepthTexture(Some(&**depth));
            scaler.setMotionTexture(Some(&*motion));
            scaler.setOutputTexture(Some(&*out));
            scaler.setInputContentWidth(iw);
            scaler.setInputContentHeight(ih);
            scaler.setJitterOffsetX(jit.0);
            scaler.setJitterOffsetY(jit.1);
            scaler.setMotionVectorScaleX(-(iw as f32));
            scaler.setMotionVectorScaleY(-(ih as f32));
            scaler.setDepthReversed(true);
            scaler.setReset(reset);
            scaler.encodeToCommandBuffer(cb);
        }
        self.prev_vp = Some(vp);
        self.frame = self.frame.wrapping_add(1);
        Some(out)
    }
}
