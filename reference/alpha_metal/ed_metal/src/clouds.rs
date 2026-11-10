//! Native volumetric clouds (port of `ed_game/src/clouds.wesl`, see `shaders/clouds.metal`).
//!
//! Runs between the main pass and the screen-space post: a half-res compute raymarch of the cloud shell
//! (clipped against scene depth), a temporal resolve against the previous frame's clouds, and a depth-aware
//! bilateral composite that premultiplied-blends the result onto the HDR target.
//!
//! Cost (M-series, 1440p, HIGH quality): ~1.5-3 ms GPU. Off switch: `EW_NATIVE_NOCLOUDS`.
//! Lux -> HDR gain: `EW_NATIVE_CLOUD_GAIN` (default 6.4 / 130000, the native sun's HDR value per raw sunlight lux).

use crate::MetalContext;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_foundation::NSString;
use objc2_metal::*;
use std::ffi::c_void;
use std::ptr::NonNull;

type Tex = Retained<ProtocolObject<dyn MTLTexture>>;
type Cpso = Retained<ProtocolObject<dyn MTLComputePipelineState>>;
type Rpso = Retained<ProtocolObject<dyn MTLRenderPipelineState>>;

/// Floats in the flat cloud uniform (32 vec4s, layout of `CloudU` in the shader).
pub const CLOUD_U_FLOATS: usize = 128;

#[repr(C)]
#[derive(Clone, Copy)]
struct ViewU {
    right: [f32; 4],
    up: [f32; 4],
    fwd: [f32; 4],
    size: [f32; 4],
    proj: [f32; 4],
    full: [f32; 4],
    prev_vp: [f32; 16],
    sunv: [f32; 4],
}

pub struct CloudFrame<'a> {
    /// Flat uniform from `CloudUniform::to_flat`.
    pub uniform: &'a [f32],
    pub cam_right: [f32; 3],
    pub cam_up: [f32; 3],
    pub cam_fwd: [f32; 3],
    pub tan_half_fov: f32,
    pub aspect: f32,
    pub near: f32,
    /// Un-jittered reverse-Z view-projection of this frame (column-major).
    pub vp_unjit: [f32; 16],
    pub cam_pos: [f32; 3],
    /// Sun direction + day factor in `.w`.
    pub sun: [f32; 4],
    pub frame: u32,
}

struct Sized {
    w: usize,
    h: usize,
    cur: Tex,
    cur_aux: Tex,
    hist: [Tex; 2],
    hist_aux: [Tex; 2],
}

pub struct CloudRenderer {
    march: Cpso,
    resolve: Cpso,
    comp: Rpso,
    shape: Option<Tex>,
    detail: Option<Tex>,
    sized: Option<Sized>,
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    prev_vp: [f32; 16],
    prev_cam: Option<[f32; 3]>,
    ping: usize,
    pub on: bool,
    gain: f32,
}

impl CloudRenderer {
    pub fn new(ctx: &MetalContext, hdr_fmt: MTLPixelFormat) -> Option<Self> {
        let lib = match ctx.library(include_str!("../shaders/clouds.metal")) {
            Ok(l) => l,
            Err(e) => {
                log::error!("cloud shader compile failed: {e}");
                return None;
            }
        };
        let cpso = |name: &str| -> Option<Cpso> {
            let f = lib.newFunctionWithName(&NSString::from_str(name))?;
            match ctx.device.newComputePipelineStateWithFunction_error(&f) {
                Ok(p) => Some(p),
                Err(e) => {
                    log::error!("cloud pso {name}: {e:?}");
                    None
                }
            }
        };
        let d = MTLRenderPipelineDescriptor::new();
        let vf = lib.newFunctionWithName(&NSString::from_str("cloud_comp_vs"))?;
        let ff = lib.newFunctionWithName(&NSString::from_str("cloud_comp_fs"))?;
        d.setVertexFunction(Some(&vf));
        d.setFragmentFunction(Some(&ff));
        let ca = unsafe { d.colorAttachments().objectAtIndexedSubscript(0) };
        ca.setPixelFormat(hdr_fmt);
        ca.setBlendingEnabled(true);
        // Premultiplied "over": the shader outputs rgb already multiplied by coverage.
        ca.setSourceRGBBlendFactor(MTLBlendFactor::One);
        ca.setDestinationRGBBlendFactor(MTLBlendFactor::OneMinusSourceAlpha);
        ca.setSourceAlphaBlendFactor(MTLBlendFactor::Zero);
        ca.setDestinationAlphaBlendFactor(MTLBlendFactor::One);
        let comp = match ctx.device.newRenderPipelineStateWithDescriptor_error(&d) {
            Ok(p) => p,
            Err(e) => {
                log::error!("cloud composite pso: {e:?}");
                return None;
            }
        };
        Some(Self {
            march: cpso("cloud_march")?,
            resolve: cpso("cloud_resolve")?,
            comp,
            shape: None,
            detail: None,
            sized: None,
            device: ctx.device.clone(),
            prev_vp: [0.0; 16],
            prev_cam: None,
            ping: 0,
            on: std::env::var_os("EW_NATIVE_NOCLOUDS").is_none(),
            gain: std::env::var("EW_NATIVE_CLOUD_GAIN").ok().and_then(|v| v.parse().ok()).unwrap_or(6.4 / 130_000.0),
        })
    }

    pub fn has_noise(&self) -> bool {
        self.shape.is_some() && self.detail.is_some()
    }

    /// Upload the tileable RGBA8 noise volumes (shape `ns`^3, detail `nd`^3) and build their mip chains.
    pub fn upload_noise(&mut self, queue: &ProtocolObject<dyn MTLCommandQueue>, shape: &[u8], ns: usize, detail: &[u8], nd: usize) {
        let mk = |data: &[u8], n: usize| -> Option<Tex> {
            if data.len() < n * n * n * 4 {
                return None;
            }
            let d = unsafe { MTLTextureDescriptor::new() };
            d.setTextureType(MTLTextureType::Type3D);
            d.setPixelFormat(MTLPixelFormat::RGBA8Unorm);
            let levels = (n as f32).log2().floor() as usize + 1;
            unsafe {
                d.setWidth(n);
                d.setHeight(n);
                d.setDepth(n);
                d.setMipmapLevelCount(levels);
            }
            d.setUsage(MTLTextureUsage::ShaderRead);
            let t = self.device.newTextureWithDescriptor(&d)?;
            unsafe {
                t.replaceRegion_mipmapLevel_slice_withBytes_bytesPerRow_bytesPerImage(
                    MTLRegion { origin: MTLOrigin { x: 0, y: 0, z: 0 }, size: MTLSize { width: n, height: n, depth: n } },
                    0,
                    0,
                    NonNull::new(data.as_ptr() as *mut c_void)?,
                    n * 4,
                    n * n * 4,
                );
            }
            Some(t)
        };
        let (Some(a), Some(b)) = (mk(shape, ns), mk(detail, nd)) else {
            log::error!("cloud noise upload failed");
            return;
        };
        if let Some(cb) = queue.commandBuffer() {
            if let Some(blit) = cb.blitCommandEncoder() {
                blit.generateMipmapsForTexture(&a);
                blit.generateMipmapsForTexture(&b);
                blit.endEncoding();
            }
            cb.commit();
            cb.waitUntilCompleted();
        }
        self.shape = Some(a);
        self.detail = Some(b);
    }

    fn tex(&self, fmt: MTLPixelFormat, w: usize, h: usize) -> Tex {
        let d = unsafe { MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(fmt, w.max(1), h.max(1), false) };
        d.setUsage(MTLTextureUsage::ShaderRead | MTLTextureUsage::ShaderWrite);
        d.setStorageMode(MTLStorageMode::Private);
        self.device.newTextureWithDescriptor(&d).unwrap()
    }

    /// Encode the cloud passes. Draws onto `hdr` (RGBA16F, already holding sky + scene) using `depth` (reverse-Z).
    pub fn encode(&mut self, cb: &ProtocolObject<dyn MTLCommandBuffer>, f: &CloudFrame, hdr: &Tex, depth: &Tex) {
        let (Some(shape), Some(detail)) = (self.shape.clone(), self.detail.clone()) else { return };
        if f.uniform.len() < CLOUD_U_FLOATS {
            return;
        }
        let (w, h) = (hdr.width(), hdr.height());
        let (hw, hh) = (w.div_ceil(2), h.div_ceil(2));
        let mut reset = false;
        if !self.sized.as_ref().is_some_and(|s| s.w == w && s.h == h) {
            self.sized = Some(Sized {
                w,
                h,
                cur: self.tex(MTLPixelFormat::RGBA16Float, hw, hh),
                cur_aux: self.tex(MTLPixelFormat::RGBA32Float, hw, hh),
                hist: [self.tex(MTLPixelFormat::RGBA16Float, hw, hh), self.tex(MTLPixelFormat::RGBA16Float, hw, hh)],
                hist_aux: [self.tex(MTLPixelFormat::RGBA32Float, hw, hh), self.tex(MTLPixelFormat::RGBA32Float, hw, hh)],
            });
            reset = true;
        }
        // Teleport / floating-origin shift: reprojecting through a different render origin would smear.
        if self.prev_cam.is_none_or(|c| {
            let d = [c[0] - f.cam_pos[0], c[1] - f.cam_pos[1], c[2] - f.cam_pos[2]];
            (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() > 300.0
        }) {
            reset = true;
        }
        self.prev_cam = Some(f.cam_pos);
        let s = self.sized.as_ref().unwrap();

        let tx = f.tan_half_fov * f.aspect;
        let sc = |v: [f32; 3], k: f32| [v[0] * k, v[1] * k, v[2] * k, 0.0];
        let v = ViewU {
            right: sc(f.cam_right, tx),
            up: sc(f.cam_up, f.tan_half_fov),
            fwd: sc(f.cam_fwd, 1.0),
            size: [hw as f32, hh as f32, 1.0 / hw as f32, 1.0 / hh as f32],
            proj: [f.near, f.frame as f32, self.gain, if reset { 1.0 } else { 0.0 }],
            full: [w as f32, h as f32, 0.0, 0.0],
            prev_vp: self.prev_vp,
            sunv: f.sun,
        };
        let vb = NonNull::new(&v as *const ViewU as *mut c_void).unwrap();
        let ub = NonNull::new(f.uniform.as_ptr() as *mut c_void).unwrap();
        let ulen = CLOUD_U_FLOATS * 4;
        let grid = MTLSize { width: hw.div_ceil(8), height: hh.div_ceil(8), depth: 1 };
        let tgs = MTLSize { width: 8, height: 8, depth: 1 };

        let (cur_h, prev_h) = (self.ping, 1 - self.ping);
        if let Some(enc) = cb.computeCommandEncoder() {
            enc.setComputePipelineState(&self.march);
            unsafe {
                enc.setTexture_atIndex(Some(&*s.cur), 0);
                enc.setTexture_atIndex(Some(&*s.cur_aux), 1);
                enc.setTexture_atIndex(Some(&**depth), 2);
                enc.setTexture_atIndex(Some(&*shape), 3);
                enc.setTexture_atIndex(Some(&*detail), 4);
                enc.setBytes_length_atIndex(ub, ulen, 0);
                enc.setBytes_length_atIndex(vb, std::mem::size_of::<ViewU>(), 1);
            }
            enc.dispatchThreadgroups_threadsPerThreadgroup(grid, tgs);
            enc.endEncoding();
        }
        if let Some(enc) = cb.computeCommandEncoder() {
            enc.setComputePipelineState(&self.resolve);
            unsafe {
                enc.setTexture_atIndex(Some(&*s.cur), 0);
                enc.setTexture_atIndex(Some(&*s.cur_aux), 1);
                enc.setTexture_atIndex(Some(&*s.hist[prev_h]), 2);
                enc.setTexture_atIndex(Some(&*s.hist_aux[prev_h]), 3);
                enc.setTexture_atIndex(Some(&*s.hist[cur_h]), 4);
                enc.setTexture_atIndex(Some(&*s.hist_aux[cur_h]), 5);
                enc.setBytes_length_atIndex(ub, ulen, 0);
                enc.setBytes_length_atIndex(vb, std::mem::size_of::<ViewU>(), 1);
            }
            enc.dispatchThreadgroups_threadsPerThreadgroup(grid, tgs);
            enc.endEncoding();
        }
        // Composite onto the HDR target (load existing contents, blend over).
        let pass = MTLRenderPassDescriptor::new();
        let ca = unsafe { pass.colorAttachments().objectAtIndexedSubscript(0) };
        ca.setTexture(Some(&**hdr));
        ca.setLoadAction(MTLLoadAction::Load);
        ca.setStoreAction(MTLStoreAction::Store);
        if let Some(re) = cb.renderCommandEncoderWithDescriptor(&pass) {
            re.setRenderPipelineState(&self.comp);
            unsafe {
                re.setFragmentTexture_atIndex(Some(&*s.hist[cur_h]), 0);
                re.setFragmentTexture_atIndex(Some(&*s.hist_aux[cur_h]), 1);
                re.setFragmentTexture_atIndex(Some(&**depth), 2);
                re.setFragmentBytes_length_atIndex(vb, std::mem::size_of::<ViewU>(), 1);
                re.drawPrimitives_vertexStart_vertexCount(MTLPrimitiveType::Triangle, 0, 3);
            }
            re.endEncoding();
        }
        self.ping = prev_h;
        self.prev_vp = f.vp_unjit;
    }
}
