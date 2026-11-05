//! Physically based atmosphere (Hillaire 2020): transmittance, multiple-scattering, sky-view LUTs and an
//! aerial-perspective froxel volume, all built with Metal compute passes. Planet 6360 km, top 6460 km.
//! Game units are metres; the camera sits at `planet_radius + cam_height_m` along +Y.

use super::MetalContext;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_foundation::NSString;
use objc2_metal::*;
use std::cell::Cell;
use std::ffi::c_void;
use std::ptr::NonNull;

/// Include snippet for other shaders: constants, LUT parameterisation, `sky_radiance`, `sun_disk`,
/// `aerial_perspective`. Prepend it to your own MSL source.
pub const MSL_SKY_FUNCS: &str = include_str!("../shaders/atmosphere_funcs.metal");
const MSL_KERNELS: &str = include_str!("../shaders/atmosphere.metal");

pub const TRANSMITTANCE_SIZE: (usize, usize) = (256, 64);
pub const MULTISCATTER_SIZE: (usize, usize) = (32, 32);
pub const SKYVIEW_SIZE: (usize, usize) = (192, 108);
pub const AERIAL_SIZE: (usize, usize, usize) = (32, 32, 32);
/// Distance (m) covered by the last aerial-perspective slice.
pub const AERIAL_MAX_DIST_M: f32 = 64_000.0;

type Tex = Retained<ProtocolObject<dyn MTLTexture>>;
type Pso = Retained<ProtocolObject<dyn MTLComputePipelineState>>;

#[derive(Clone, Copy, Debug)]
pub struct AtmosphereParams {
    /// Unit vector toward the sun (world space, +Y up).
    pub sun_dir: [f32; 3],
    /// Sun illuminance at the top of the atmosphere (LUT radiance scale).
    pub sun_illuminance: [f32; 3],
    /// Camera height above ground in metres.
    pub cam_height_m: f32,
    /// Inverse of the reverse-Z view-projection (column-major, as glam `to_cols_array_2d`).
    pub view_proj_inv: [[f32; 4]; 4],
    /// Camera world position (currently unused by the LUTs; ray directions come from `view_proj_inv`).
    pub cam_pos: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct AtmoU {
    sun_h: [f32; 4],
    illum: [f32; 4],
    inv_vp: [[f32; 4]; 4],
}

pub struct Atmosphere {
    ctx_queue_device: Retained<ProtocolObject<dyn MTLDevice>>,
    pso_trans: Pso,
    pso_ms: Pso,
    pso_sky: Pso,
    pso_ap: Pso,
    transmittance: Tex,
    multiscatter: Tex,
    sky_view: Tex,
    aerial: Tex,
    static_dirty: Cell<bool>,
}

impl Atmosphere {
    pub fn new(ctx: &MetalContext) -> Option<Self> {
        let src = format!("{}\n{}", MSL_SKY_FUNCS, MSL_KERNELS);
        let lib = match ctx.library(&src) {
            Ok(l) => l,
            Err(e) => {
                log::error!("atmosphere shader compile failed: {e}");
                return None;
            }
        };
        let pso = |name: &str| -> Option<Pso> {
            let f = lib.newFunctionWithName(&NSString::from_str(name))?;
            ctx.device.newComputePipelineStateWithFunction_error(&f).ok()
        };
        let tex = |w, h, d: usize| -> Option<Tex> {
            let desc = MTLTextureDescriptor::new();
            desc.setTextureType(if d > 1 { MTLTextureType::Type3D } else { MTLTextureType::Type2D });
            desc.setPixelFormat(MTLPixelFormat::RGBA16Float);
            unsafe {
                desc.setWidth(w);
                desc.setHeight(h);
                desc.setDepth(d);
            }
            desc.setUsage(MTLTextureUsage::ShaderRead | MTLTextureUsage::ShaderWrite);
            desc.setStorageMode(MTLStorageMode::Private);
            ctx.device.newTextureWithDescriptor(&desc)
        };
        Some(Self {
            ctx_queue_device: ctx.device.clone(),
            pso_trans: pso("at_transmittance")?,
            pso_ms: pso("at_multiscatter")?,
            pso_sky: pso("at_skyview")?,
            pso_ap: pso("at_aerial")?,
            transmittance: tex(TRANSMITTANCE_SIZE.0, TRANSMITTANCE_SIZE.1, 1)?,
            multiscatter: tex(MULTISCATTER_SIZE.0, MULTISCATTER_SIZE.1, 1)?,
            sky_view: tex(SKYVIEW_SIZE.0, SKYVIEW_SIZE.1, 1)?,
            aerial: tex(AERIAL_SIZE.0, AERIAL_SIZE.1, AERIAL_SIZE.2)?,
            static_dirty: Cell::new(true),
        })
    }

    /// RGBA16F 256x64: rgb = transmittance, indexed by (view angle, height).
    pub fn transmittance(&self) -> &ProtocolObject<dyn MTLTexture> {
        &self.transmittance
    }
    /// RGBA16F 32x32: multiple-scattering transfer (unit sun illuminance).
    pub fn multiscatter(&self) -> &ProtocolObject<dyn MTLTexture> {
        &self.multiscatter
    }
    /// RGBA16F 192x108: rgb = sky luminance (scaled by sun illuminance), a = mean view transmittance.
    pub fn sky_view(&self) -> &ProtocolObject<dyn MTLTexture> {
        &self.sky_view
    }
    /// RGBA16F 3D 32x32x32: rgb = in-scattered luminance, a = mean transmittance, up to 64 km.
    pub fn aerial(&self) -> &ProtocolObject<dyn MTLTexture> {
        &self.aerial
    }

    /// Force transmittance + multi-scatter LUTs to be rebuilt on the next `encode_luts`.
    pub fn mark_static_dirty(&self) {
        self.static_dirty.set(true);
    }

    /// Encode everything needed this frame: static LUTs when dirty, then sky-view and aerial perspective.
    pub fn encode_luts(&self, cb: &ProtocolObject<dyn MTLCommandBuffer>, p: &AtmosphereParams) {
        if self.static_dirty.replace(false) {
            self.encode_transmittance(cb);
            self.encode_multiscatter(cb);
        }
        self.encode_sky_view(cb, p);
        self.encode_aerial(cb, p);
    }

    pub fn encode_transmittance(&self, cb: &ProtocolObject<dyn MTLCommandBuffer>) {
        self.dispatch(cb, &self.pso_trans, &[&self.transmittance], None, TRANSMITTANCE_SIZE.0, TRANSMITTANCE_SIZE.1);
    }
    pub fn encode_multiscatter(&self, cb: &ProtocolObject<dyn MTLCommandBuffer>) {
        self.dispatch(cb, &self.pso_ms, &[&self.multiscatter, &self.transmittance], None, MULTISCATTER_SIZE.0, MULTISCATTER_SIZE.1);
    }
    pub fn encode_sky_view(&self, cb: &ProtocolObject<dyn MTLCommandBuffer>, p: &AtmosphereParams) {
        let u = Self::uniform(p);
        self.dispatch(cb, &self.pso_sky, &[&self.sky_view, &self.transmittance, &self.multiscatter], Some(&u), SKYVIEW_SIZE.0, SKYVIEW_SIZE.1);
    }
    pub fn encode_aerial(&self, cb: &ProtocolObject<dyn MTLCommandBuffer>, p: &AtmosphereParams) {
        let u = Self::uniform(p);
        self.dispatch(cb, &self.pso_ap, &[&self.aerial, &self.transmittance, &self.multiscatter], Some(&u), AERIAL_SIZE.0, AERIAL_SIZE.1);
    }

    fn uniform(p: &AtmosphereParams) -> AtmoU {
        let s = p.sun_dir;
        let l = (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt().max(1e-6);
        let i = p.sun_illuminance;
        AtmoU {
            sun_h: [s[0] / l, s[1] / l, s[2] / l, p.cam_height_m.max(0.0) * 0.001],
            illum: [i[0], i[1], i[2], 0.0],
            inv_vp: p.view_proj_inv,
        }
    }

    fn dispatch(&self, cb: &ProtocolObject<dyn MTLCommandBuffer>, pso: &Pso, texs: &[&Tex], u: Option<&AtmoU>, w: usize, h: usize) {
        let Some(enc) = cb.computeCommandEncoder() else { return };
        enc.setComputePipelineState(pso);
        unsafe {
            for (i, t) in texs.iter().enumerate() {
                enc.setTexture_atIndex(Some(&***t), i);
            }
            if let Some(u) = u {
                enc.setBytes_length_atIndex(NonNull::new(u as *const AtmoU as *mut c_void).unwrap(), std::mem::size_of::<AtmoU>(), 0);
            }
        }
        let tg = MTLSize { width: 8, height: 8, depth: 1 };
        enc.dispatchThreadgroups_threadsPerThreadgroup(MTLSize { width: w.div_ceil(8), height: h.div_ceil(8), depth: 1 }, tg);
        enc.endEncoding();
    }

    pub fn device(&self) -> &ProtocolObject<dyn MTLDevice> {
        &self.ctx_queue_device
    }
}
