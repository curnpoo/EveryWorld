//! Native FFT ocean (see `shaders/ocean.metal`, docs/plans/ultra-water-port.md stage 3).
//!
//! Three band-limited cascades (600 / 85 / 12 m, 128^2) of a seeded JONSWAP spectrum, evolved and inverse-FFT'd on
//! the GPU every frame into an RGBA16F texture array (slope x, slope z, Jacobian, height) that `water_fs` samples.
//! The spectrum is generated from a fixed seed with deterministic maths, so a CPU wave model for buoyancy
//! (phase 6) can rebuild the same `h0` on every client.

use crate::MetalContext;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_foundation::NSString;
use objc2_metal::*;
use std::ffi::c_void;
use std::ptr::NonNull;

type Tex = Retained<ProtocolObject<dyn MTLTexture>>;
type Buf = Retained<ProtocolObject<dyn MTLBuffer>>;
type Cpso = Retained<ProtocolObject<dyn MTLComputePipelineState>>;

pub const N: usize = 128;
pub const NC: usize = 3;
pub const LEN: [f32; 4] = [600.0, 85.0, 12.0, 1.0];
pub const LOOP_S: f32 = 300.0;
const SEED: u64 = 0x5EED_0CEA_4001;

#[repr(C)]
#[derive(Clone, Copy)]
struct OceanU {
    len: [f32; 4],
    misc: [f32; 4],
}

pub struct Ocean {
    spec: Cpso,
    fft: Cpso,
    pack: Cpso,
    h0: Buf,
    work: Buf,
    pub tex: Tex,
    /// Slope variance of each cascade's band (for the roughness that mip averaging removes).
    pub variance: [f32; 4],
    pub choppy: f32,
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        ((self.0 >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }
    fn gauss(&mut self) -> (f64, f64) {
        let (a, b) = (self.next(), self.next());
        let r = (-2.0 * a.ln()).sqrt();
        (r * (std::f64::consts::TAU * b).cos(), r * (std::f64::consts::TAU * b).sin())
    }
}

/// JONSWAP 2-D spectrum density F(k vec) for wind speed `u` (m/s), fetch `fetch` (m), direction `wd` (unit xz).
pub fn jonswap_f(kx: f64, kz: f64, u: f64, fetch: f64, wd: (f64, f64)) -> f64 {
    let g = 9.81;
    let k = (kx * kx + kz * kz).sqrt();
    if k < 1e-6 {
        return 0.0;
    }
    let w = (g * k + 7.4e-5 * k * k * k).sqrt();
    let dwdk = (g + 3.0 * 7.4e-5 * k * k) / (2.0 * w);
    let alpha = 0.076 * (g * fetch / (u * u)).powf(-0.22);
    let wp = 22.0 * (u * fetch / (g * g)).powf(-0.33);
    let sigma = if w <= wp { 0.07 } else { 0.09 };
    let r = (-(w - wp).powi(2) / (2.0 * sigma * sigma * wp * wp)).exp();
    let s = alpha * g * g / w.powi(5) * (-1.25 * (wp / w).powi(4)).exp() * 3.3f64.powf(r);
    let cosd = (kx * wd.0 + kz * wd.1) / k;
    let d = if cosd > 0.0 { 2.0 / std::f64::consts::PI * cosd * cosd } else { 0.0 };
    s * dwdk * d / k
}

impl Ocean {
    pub fn new(ctx: &MetalContext) -> Option<Self> {
        let lib = ctx.library(include_str!("../shaders/ocean.metal")).map_err(|e| log::error!("ocean shader: {e}")).ok()?;
        let pso = |n: &str| -> Option<Cpso> {
            let f = lib.newFunctionWithName(&NSString::from_str(n))?;
            ctx.device.newComputePipelineStateWithFunction_error(&f).ok()
        };
        // Band limits so cascades do not double count: [0, .62], [.62, 4.4], [4.4, inf).
        let bands = [(0.0f64, 0.62f64), (0.62, 4.4), (4.4, 1e9)];
        let wd = (0.8f64, 0.6f64);
        let mut rng = Rng(SEED);
        let mut h0 = vec![0f32; NC * N * N * 4];
        let mut variance = [0f32; 4];
        for c in 0..NC {
            let l = LEN[c] as f64;
            let dk = std::f64::consts::TAU / l;
            let mut cplx = vec![(0f64, 0f64); N * N];
            for y in 0..N {
                for x in 0..N {
                    let (kx, kz) = (((x as f64) - (N / 2) as f64) * dk, ((y as f64) - (N / 2) as f64) * dk);
                    let k = (kx * kx + kz * kz).sqrt();
                    let (g0, g1) = rng.gauss();
                    if k < bands[c].0 || k >= bands[c].1 {
                        continue;
                    }
                    let f = jonswap_f(kx, kz, 9.0, 120_000.0, wd);
                    let amp = (0.5 * f).sqrt() * dk;
                    cplx[y * N + x] = (g0 * amp / 2f64.sqrt(), g1 * amp / 2f64.sqrt());
                    variance[c] += (k * k * f * dk * dk) as f32;
                }
            }
            for y in 0..N {
                for x in 0..N {
                    let a = cplx[y * N + x];
                    let (mx, my) = ((N - x) % N, (N - y) % N);
                    // -k partner; the Nyquist row/column has none and stays zero.
                    let b = if x == 0 || y == 0 { (0.0, 0.0) } else { cplx[my * N + mx] };
                    let o = ((c * N + y) * N + x) * 4;
                    h0[o] = a.0 as f32;
                    h0[o + 1] = a.1 as f32;
                    h0[o + 2] = b.0 as f32;
                    h0[o + 3] = -b.1 as f32; // conj
                }
            }
        }
        let bytes = h0.len() * 4;
        let hb = unsafe { ctx.device.newBufferWithBytes_length_options(NonNull::new(h0.as_ptr() as *mut c_void)?, bytes, MTLResourceOptions::StorageModeShared)? };
        let work = ctx.device.newBufferWithLength_options(NC * 4 * N * N * 8, MTLResourceOptions::StorageModePrivate)?;
        let d = unsafe { MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(MTLPixelFormat::RGBA16Float, N, N, true) };
        d.setTextureType(MTLTextureType::Type2DArray);
        unsafe { d.setArrayLength(NC) };
        d.setUsage(MTLTextureUsage::ShaderRead | MTLTextureUsage::ShaderWrite);
        d.setStorageMode(MTLStorageMode::Private);
        let tex = ctx.device.newTextureWithDescriptor(&d)?;
        log::info!("ocean fft ready: cascades {:?} m, slope variance {:?}", &LEN[..3], &variance[..3]);
        Some(Self { spec: pso("ocean_spec")?, fft: pso("ocean_fft")?, pack: pso("ocean_pack")?, h0: hb, work, tex, variance, choppy: 0.85 })
    }

    pub fn encode(&self, cb: &ProtocolObject<dyn MTLCommandBuffer>, t: f32) {
        let u = OceanU { len: LEN, misc: [t.rem_euclid(LOOP_S), self.choppy, LOOP_S, 0.0] };
        let up = NonNull::new(&u as *const OceanU as *mut c_void).unwrap();
        if let Some(e) = cb.computeCommandEncoder() {
            unsafe {
                e.setComputePipelineState(&self.spec);
                e.setBuffer_offset_atIndex(Some(&self.h0), 0, 0);
                e.setBuffer_offset_atIndex(Some(&self.work), 0, 1);
                e.setBytes_length_atIndex(up, std::mem::size_of::<OceanU>(), 2);
                e.dispatchThreadgroups_threadsPerThreadgroup(MTLSize { width: N / 8, height: N / 8, depth: NC }, MTLSize { width: 8, height: 8, depth: 1 });
                e.setComputePipelineState(&self.fft);
                e.setBuffer_offset_atIndex(Some(&self.work), 0, 0);
                for pass in 0u32..2 {
                    e.setBytes_length_atIndex(NonNull::new(&pass as *const u32 as *mut c_void).unwrap(), 4, 1);
                    e.dispatchThreadgroups_threadsPerThreadgroup(MTLSize { width: N, height: 4, depth: NC }, MTLSize { width: N, height: 1, depth: 1 });
                }
                e.setComputePipelineState(&self.pack);
                e.setBuffer_offset_atIndex(Some(&self.work), 0, 0);
                e.setTexture_atIndex(Some(&self.tex), 0);
                e.setBytes_length_atIndex(up, std::mem::size_of::<OceanU>(), 2);
                e.dispatchThreadgroups_threadsPerThreadgroup(MTLSize { width: N / 8, height: N / 8, depth: NC }, MTLSize { width: 8, height: 8, depth: 1 });
            }
            e.endEncoding();
        }
        if let Some(b) = cb.blitCommandEncoder() {
            b.generateMipmapsForTexture(&self.tex);
            b.endEncoding();
        }
    }
}
