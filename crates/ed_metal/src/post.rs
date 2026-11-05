//! Screen-space post: GTAO (half res, bilateral blur + upsample) and a dual-filter HDR bloom, built from
//! `shaders/post.metal` compute kernels. `PostChain::encode` runs between the main pass and the resolve and
//! returns the AO (R8, full res) and bloom (RGBA16F, half res) textures the resolve samples.

use crate::MetalContext;
use glam::{Mat4, Vec3};
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_foundation::NSString;
use objc2_metal::*;
use std::ffi::c_void;
use std::ptr::NonNull;

type Tex = Retained<ProtocolObject<dyn MTLTexture>>;
type Pso = Retained<ProtocolObject<dyn MTLComputePipelineState>>;

const BLOOM_LEVELS: usize = 5;

#[repr(C)]
#[derive(Clone, Copy)]
struct PostU {
    inv_proj: [f32; 16],
    view: [f32; 16],
    view_inv: [f32; 16],
    size: [f32; 4],
    proj: [f32; 4],
    ao: [f32; 4],
    ao2: [f32; 4],
    ssr: [f32; 4],
    ssr2: [f32; 4],
    bloom: [f32; 4],
    fin: [f32; 4],
    fin2: [f32; 4],
    fin3: [f32; 4],
    ints: [u32; 4],
}

pub struct PostParams {
    pub cam_right: [f32; 3],
    pub cam_up: [f32; 3],
    pub cam_fwd: [f32; 3],
    pub tan_half_fov: f32,
    pub aspect: f32,
    pub near: f32,
}

struct Sized {
    w: usize,
    h: usize,
    lin: Tex,
    ao_a: Tex,
    ao_b: Tex,
    ao_full: Tex,
    down: Vec<Tex>,
    up: Vec<Tex>,
    ssr: Tex,
    bounce: Vec<Tex>,
}

pub struct PostChain {
    lin: Pso,
    gtao: Pso,
    blur: Pso,
    upsample: Pso,
    bloom0: Pso,
    bloom_down: Pso,
    bloom_up: Pso,
    ssr_trace: Pso,
    ssr_comp: Pso,
    sized: Option<Sized>,
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    pub ao_on: bool,
    pub bloom_on: bool,
    pub ssr_on: bool,
    pub bounce_on: bool,
    frame: u32,
    slices: f32,
    steps: f32,
}

impl PostChain {
    pub fn new(ctx: &MetalContext) -> Option<Self> {
        let lib = match ctx.library(include_str!("../shaders/post.metal")) {
            Ok(l) => l,
            Err(e) => {
                log::error!("post shader compile failed: {e}");
                return None;
            }
        };
        let pso = |name: &str| -> Option<Pso> {
            let f = lib.newFunctionWithName(&NSString::from_str(name))?;
            ctx.device.newComputePipelineStateWithFunction_error(&f).ok()
        };
        Some(Self {
            lin: pso("post_lin_depth")?,
            gtao: pso("post_gtao")?,
            blur: pso("post_ao_blur")?,
            upsample: pso("post_ao_upsample")?,
            bloom0: pso("post_bloom_down0")?,
            bloom_down: pso("post_bloom_down")?,
            bloom_up: pso("post_bloom_up")?,
            ssr_trace: pso("post_ssr_trace")?,
            ssr_comp: pso("post_ssr_composite")?,
            sized: None,
            device: ctx.device.clone(),
            ao_on: std::env::var("EW_NATIVE_NOAO").is_err(),
            bloom_on: std::env::var("EW_NATIVE_NOBLOOM").is_err(),
            ssr_on: std::env::var("EW_NATIVE_NOSSR").is_err(),
            bounce_on: std::env::var("EW_NATIVE_NOBOUNCE").is_err(),
            frame: 0,
            slices: std::env::var("EW_NATIVE_AO_SLICES").ok().and_then(|v| v.parse().ok()).unwrap_or(2.0),
            steps: std::env::var("EW_NATIVE_AO_STEPS").ok().and_then(|v| v.parse().ok()).unwrap_or(5.0),
        })
    }

    fn tex(&self, fmt: MTLPixelFormat, w: usize, h: usize) -> Tex {
        let d = unsafe { MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(fmt, w.max(1), h.max(1), false) };
        d.setUsage(MTLTextureUsage::ShaderRead | MTLTextureUsage::ShaderWrite);
        d.setStorageMode(MTLStorageMode::Private);
        self.device.newTextureWithDescriptor(&d).unwrap()
    }

    fn ensure(&mut self, w: usize, h: usize) {
        if self.sized.as_ref().is_some_and(|s| s.w == w && s.h == h) {
            return;
        }
        let (hw, hh) = (w.div_ceil(2), h.div_ceil(2));
        let mut down = Vec::new();
        let (mut lw, mut lh) = (hw, hh);
        for _ in 0..BLOOM_LEVELS {
            down.push(self.tex(MTLPixelFormat::RGBA16Float, lw, lh));
            lw = lw.div_ceil(2);
            lh = lh.div_ceil(2);
        }
        let up = (0..BLOOM_LEVELS - 1).map(|i| self.tex(MTLPixelFormat::RGBA16Float, down[i].width(), down[i].height())).collect();
        self.sized = Some(Sized {
            w,
            h,
            lin: self.tex(MTLPixelFormat::R32Float, w, h),
            ao_a: self.tex(MTLPixelFormat::R8Unorm, hw, hh),
            ao_b: self.tex(MTLPixelFormat::R8Unorm, hw, hh),
            ao_full: self.tex(MTLPixelFormat::R8Unorm, w, h),
            down,
            up,
            ssr: self.tex(MTLPixelFormat::RGBA16Float, hw, hh),
            bounce: (1..=3).map(|k| self.tex(MTLPixelFormat::RGBA16Float, w.div_ceil(1 << k), h.div_ceil(1 << k))).collect(),
        });
    }

    fn run(&self, cb: &ProtocolObject<dyn MTLCommandBuffer>, pso: &Pso, texs: &[&ProtocolObject<dyn MTLTexture>], u: Option<&PostU>, dir: Option<[i32; 2]>, w: usize, h: usize) {
        let Some(enc) = cb.computeCommandEncoder() else { return };
        enc.setComputePipelineState(pso);
        unsafe {
            for (i, t) in texs.iter().enumerate() {
                enc.setTexture_atIndex(Some(*t), i);
            }
            if let Some(u) = u {
                enc.setBytes_length_atIndex(NonNull::new(u as *const PostU as *mut c_void).unwrap(), std::mem::size_of::<PostU>(), 0);
            }
            if let Some(d) = dir {
                enc.setBytes_length_atIndex(NonNull::new(d.as_ptr() as *mut c_void).unwrap(), 8, 1);
            }
        }
        enc.dispatchThreadgroups_threadsPerThreadgroup(MTLSize { width: w.div_ceil(8), height: h.div_ceil(8), depth: 1 }, MTLSize { width: 8, height: 8, depth: 1 });
        enc.endEncoding();
    }

    /// Encode SSR, AO, bounce and bloom. Returns (ao full-res R8, bloom half-res RGBA16F, 1/8-res blurred scene for bounce).
    pub fn encode(&mut self, cb: &ProtocolObject<dyn MTLCommandBuffer>, p: &PostParams, hdr: &Tex, nrm: &Tex, depth: &Tex) -> (Tex, Tex, Tex) {
        let (w, h) = (hdr.width(), hdr.height());
        self.ensure(w, h);
        self.frame = self.frame.wrapping_add(1);
        let s = self.sized.as_ref().unwrap();
        let right = Vec3::from(p.cam_right);
        let up = Vec3::from(p.cam_up);
        let back = -Vec3::from(p.cam_fwd);
        let view_inv = Mat4::from_cols(right.extend(0.0), up.extend(0.0), back.extend(0.0), glam::Vec4::W);
        let view = view_inv.transpose();
        let u = PostU {
            inv_proj: Mat4::IDENTITY.to_cols_array(),
            view: view.to_cols_array(),
            view_inv: view_inv.to_cols_array(),
            size: [w as f32, h as f32, 1.0 / w as f32, 1.0 / h as f32],
            proj: [p.near, p.tan_half_fov, p.aspect, 0.0],
            ao: [3.0, 1.0, 2.0, self.slices],
            ao2: [self.steps, 0.0, 0.0, 0.0],
            ssr: [400.0, 25.0, 40.0, 1.0],
            ssr2: [0.04, 0.55, 0.0, 0.0],
            bloom: [1.6, 0.8, 1.0, 0.0],
            fin: [0.0; 4],
            fin2: [0.0; 4],
            fin3: [0.0; 4],
            ints: [self.frame, 0, 0, 0],
        };
        let (hw, hh) = (s.ao_a.width(), s.ao_a.height());
        self.run(cb, &self.lin, &[&**depth, &*s.lin], Some(&u), None, w, h);
        if self.ao_on {
            self.run(cb, &self.gtao, &[&*s.lin, &**nrm, &*s.ao_a], Some(&u), None, hw, hh);
            self.run(cb, &self.blur, &[&*s.ao_a, &*s.lin, &*s.ao_b], Some(&u), Some([1, 0]), hw, hh);
            self.run(cb, &self.blur, &[&*s.ao_b, &*s.lin, &*s.ao_a], Some(&u), Some([0, 1]), hw, hh);
            self.run(cb, &self.upsample, &[&*s.ao_a, &*s.lin, &*s.ao_full], Some(&u), None, w, h);
        }
        if self.ssr_on {
            let (qw, qh) = (s.ssr.width(), s.ssr.height());
            self.run(cb, &self.ssr_trace, &[&**hdr, &**depth, &**nrm, &*s.ssr], Some(&u), None, qw, qh);
            self.run(cb, &self.ssr_comp, &[&**hdr, &**nrm, &**depth, &*s.ssr], Some(&u), None, w, h);
        }
        if self.bounce_on {
            self.run(cb, &self.bloom_down, &[&**hdr, &*s.bounce[0]], None, None, s.bounce[0].width(), s.bounce[0].height());
            for i in 1..3 {
                self.run(cb, &self.bloom_down, &[&*s.bounce[i - 1], &*s.bounce[i]], None, None, s.bounce[i].width(), s.bounce[i].height());
            }
        }
        if self.bloom_on {
            self.run(cb, &self.bloom0, &[&**hdr, &*s.down[0]], Some(&u), None, s.down[0].width(), s.down[0].height());
            for i in 1..BLOOM_LEVELS {
                self.run(cb, &self.bloom_down, &[&*s.down[i - 1], &*s.down[i]], None, None, s.down[i].width(), s.down[i].height());
            }
            for i in (0..BLOOM_LEVELS - 1).rev() {
                let low = if i == BLOOM_LEVELS - 2 { &s.down[i + 1] } else { &s.up[i + 1] };
                self.run(cb, &self.bloom_up, &[&*s.down[i], &**low, &*s.up[i]], None, None, s.up[i].width(), s.up[i].height());
            }
        }
        (s.ao_full.clone(), s.up[0].clone(), s.bounce[2].clone())
    }
}
