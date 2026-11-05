//! Native world renderer: draws Bevy-owned meshes through Metal into a `CAMetalLayer` that lives in its
//! own `NSView` underneath the Bevy window view (Bevy keeps UI on top through a transparent window).
//!
//! Frame graph: 4 shadow cascades -> main pass (sky, opaque/mask PBR into HDR + normal/roughness + depth,
//! then sorted alpha-blended draws) -> resolve (exposure + tonemap) into the drawable.
use crate::{timers::GpuTimers, MetalContext};
use ed_gpu::PassTimes;
use glam::{Mat4, Vec3};
use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_app_kit::{NSAutoresizingMaskOptions, NSView, NSWindowOrderingMode};
use objc2_core_foundation::CGSize;
use objc2_foundation::{MainThreadMarker, NSString};
use objc2_metal::*;
use objc2_quartz_core::{CAMetalDrawable, CAMetalLayer};
use std::collections::HashMap;
use std::ffi::c_void;
use std::ptr::NonNull;

type Buf = Retained<ProtocolObject<dyn MTLBuffer>>;
type Tex = Retained<ProtocolObject<dyn MTLTexture>>;
type Pso = Retained<ProtocolObject<dyn MTLRenderPipelineState>>;

const SHADOW_RES: usize = 2048;
const NCASC: usize = 6; // 0-3 near cascades (everything), 4 = mid (everything, to MID_SHADOW m), 5 = far terrain-only
const MID_SHADOW: f32 = 4000.0;
const HDR: MTLPixelFormat = MTLPixelFormat::RGBA16Float;
const NRM: MTLPixelFormat = MTLPixelFormat::RGBA8Unorm;
const DEPTH: MTLPixelFormat = MTLPixelFormat::Depth32Float;

#[repr(C)]
#[derive(Clone, Copy)]
struct FrameU {
    vp: [[f32; 4]; 4],
    sun: [f32; 4],
    cam: [f32; 4],
    sky: [f32; 4],
    fog: [f32; 4],
    csm: [[[f32; 4]; 4]; NCASC],
    csm_info: [[f32; 4]; NCASC],
    csm_split: [f32; 4],
    csm_far: [f32; 4],
    cr: [f32; 4],
    cu: [f32; 4],
    cf: [f32; 4],
    params: [f32; 4],
    pb0: [f32; 4],
    pb1: [f32; 4],
}
#[repr(C)]
#[derive(Clone, Copy)]
struct DrawU {
    model: [[f32; 4]; 4],
    tint: [f32; 4],
    emissive: [f32; 4],
    mat: [f32; 4],
}

pub struct GpuMesh {
    vb: Buf,
    ib: Buf,
    index_count: usize,
    pub center: [f32; 3],
    pub radius: f32,
    /// Spatial sub-ranges of big meshes (empty = draw whole), culled individually per pass.
    clusters: Vec<Cluster>,
    /// Coarse proxy index lists (vertex-clustered) for distant shadow cascades.
    ib2: Option<Buf>,
    /// Per-vertex ground-layer weights (12 x u8) for the splat terrain shader.
    splat: Option<Buf>,
    /// Impostor per-vertex params(4) + axis(4) + tile uv(2), 10 floats each.
    extra: Option<Buf>,
}

#[derive(Clone, Copy)]
struct Cluster {
    first: usize,
    count: usize,
    /// (first, count) into `ib2` for the 1.5 m and 4 m proxies.
    proxy: [(usize, usize); 3],
    c: [f32; 3],
    r: f32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Blend {
    Opaque,
    /// Alpha tested against `cutoff`.
    Mask,
    /// Straight alpha blending, drawn back to front after the opaque pass.
    Alpha,
}

pub struct DrawItem {
    pub mesh: u64,
    pub model: [[f32; 4]; 4],
    pub tint: [f32; 4],
    pub emissive: [f32; 3],
    pub rough: f32,
    pub metal: f32,
    pub cutoff: f32,
    pub unlit: bool,
    /// Texture id from `upload_texture`; 0 = white.
    pub tex: u64,
    /// Terrain only: baked DEM detail (linear RGBA8, see `ed_world::detail_normal`); 0 = none.
    pub tex2: u64,
    pub blend: Blend,
    pub double_sided: bool,
    /// Passed the camera frustum test (casters outside the view may still draw into shadow maps).
    pub visible: bool,
    pub cast_shadow: bool,
    /// Terrain tile: also casts into the far (terrain-only) cascade.
    pub terrain: bool,
    /// Far-tree impostor quads (needs `upload_extra` + the atlas).
    pub impostor: bool,
}

pub struct FrameParams {
    /// Reverse-Z view-projection used for rasterising.
    pub view_proj: [[f32; 4]; 4],
    pub cam_pos: [f32; 3],
    pub cam_right: [f32; 3],
    pub cam_up: [f32; 3],
    pub cam_fwd: [f32; 3],
    pub tan_half_fov: f32,
    pub aspect: f32,
    pub near: f32,
    /// Unit vector toward the sun, and the day factor (0 night .. 1 day) in `.w`.
    pub sun: [f32; 4],
    pub sky: [f32; 3],
    pub fog_density: f32,
    pub shadow_far: f32,
    /// How far terrain shadows reach (m of view depth).
    pub terrain_shadow_far: f32,
    pub shadows: bool,
    pub exposure: f32,
    pub size: (u32, u32),
    pub scale: f64,
}

struct Targets {
    w: usize,
    h: usize,
    hdr: Tex,
    nrm: Tex,
    depth: Tex,
}

struct Cascades {
    vp: [Mat4; NCASC],
    info: [[f32; 4]; NCASC],
    splits: [f32; NCASC],
    /// Light-space (x,y) circle each cascade covers, for caster culling.
    ctr: [[f32; 2]; NCASC],
    rad: [f32; NCASC],
    v0: Mat4,
}

fn fit_cascades(p: &FrameParams) -> Cascades {
    let sun = Vec3::from_slice(&p.sun[..3]).normalize();
    let up = if sun.y.abs() > 0.98 { Vec3::X } else { Vec3::Y };
    let v0 = Mat4::look_to_rh(Vec3::ZERO, -sun, up);
    let (cam, r, u, f) = (Vec3::from(p.cam_pos), Vec3::from(p.cam_right), Vec3::from(p.cam_up), Vec3::from(p.cam_fwd));
    let near = p.near.max(0.3);
    let far = p.shadow_far.max(near * 4.0);
    let mut out = Cascades { vp: [Mat4::IDENTITY; NCASC], info: [[0.0; 4]; NCASC], splits: [0.0; NCASC], ctr: [[0.0; 2]; NCASC], rad: [0.0; NCASC], v0 };
    let mut prev = near;
    for i in 0..NCASC {
        let t = (i + 1) as f32 / 4.0;
        let log = near * (far / near).powf(t.min(1.0));
        let uni = near + (far - near) * t.min(1.0);
        let split = if i == 5 { p.terrain_shadow_far.max(MID_SHADOW * 2.0) } else if i == 4 { MID_SHADOW.max(far * 2.0) } else { uni * 0.25 + log * 0.75 };
        out.splits[i] = split;
        let hy = p.tan_half_fov;
        let hx = hy * p.aspect;
        let mut corners = [Vec3::ZERO; 8];
        let mut k = 0;
        for d in [prev, split] {
            for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                corners[k] = cam + f * d + r * (sx * hx * d) + u * (sy * hy * d);
                k += 1;
            }
        }
        let centroid = corners.iter().fold(Vec3::ZERO, |a, c| a + *c) / 8.0;
        let mut radius = corners.iter().map(|c| (*c - centroid).length()).fold(0.0, f32::max);
        radius = (radius * 16.0).ceil() / 16.0;
        let c_ls = v0.transform_point3(centroid);
        let texel = 2.0 * radius / SHADOW_RES as f32;
        let (cx, cy) = ((c_ls.x / texel).floor() * texel, (c_ls.y / texel).floor() * texel);
        let zpad = if i == 5 { 12000.0 } else if i == 4 { 6000.0 } else { 4000.0 };
        let dc = -c_ls.z;
        // Quantised depth range so the cache key only changes when the window really moves.
        let q = (radius * 0.5).max(16.0);
        let (zn, zf) = (((dc - radius - zpad) / q).floor() * q, ((dc + radius) / q).ceil() * q);
        let proj = Mat4::orthographic_rh(cx - radius, cx + radius, cy - radius, cy + radius, zn, zf);
        out.vp[i] = proj * v0;
        out.info[i] = [texel, 1.0 / (zf - zn), 0.0, 0.0];
        out.ctr[i] = [cx, cy];
        out.rad[i] = radius * 1.42;
        prev = split;
    }
    out
}

/// Splat-terrain material: layer texture arrays, macro variation, per-layer size/bump.
struct ImpAtlas {
    al: Tex,
    nr: Tex,
}

struct Ground {
    al: Tex,
    nr: Tex,
    mt: Tex,
    u: Buf,
}

pub struct WorldRenderer {
    ctx: MetalContext,
    layer: Option<Retained<CAMetalLayer>>,
    _view: Option<Retained<NSView>>,
    host: Option<Retained<NSView>>,
    frames: u64,
    shadow_pso: Pso,
    shadow_mask_pso: Pso,
    sky_pso: Pso,
    opaque_pso: Pso,
    blend_pso: Pso,
    terrain_pso: Pso,
    ground: Option<Ground>,
    imp: Option<ImpAtlas>,
    probe: Option<Tex>,
    probe_dummy: Tex,
    probe_box: [f32; 6],
    probe_k: f32,
    impostor_pso: Pso,
    resolve_pso: Pso,
    ds_shadow: Retained<ProtocolObject<dyn MTLDepthStencilState>>,
    ds_opaque: Retained<ProtocolObject<dyn MTLDepthStencilState>>,
    ds_blend: Retained<ProtocolObject<dyn MTLDepthStencilState>>,
    ds_sky: Retained<ProtocolObject<dyn MTLDepthStencilState>>,
    shadow_map: Tex,
    white: Tex,
    neutral: Tex,
    filmic: Tex,
    targets: Option<Targets>,
    post: Option<crate::post::PostChain>,
    upscaler: Option<crate::upscale::Upscaler>,
    /// Internal render scale (1 = native, <1 = MetalFX temporal upscale). `EW_NATIVE_SCALE` sets the default.
    pub render_scale: f32,
    /// FXAA-lite at native scale (menu "Anti-aliasing").
    pub fxaa: bool,
    prev_cam: Option<[f32; 3]>,
    meshes: HashMap<u64, GpuMesh>,
    textures: HashMap<u64, Tex>,
    timers: GpuTimers,
    pub times: PassTimes,
    pub draws: usize,
    pub tris: u64,
    pub shadow_draws: usize,
    pub shadow_skipped: usize,
    shadow_keys: [u64; NCASC],
}

fn depth_state(ctx: &MetalContext, f: MTLCompareFunction, write: bool) -> Option<Retained<ProtocolObject<dyn MTLDepthStencilState>>> {
    let ds = MTLDepthStencilDescriptor::new();
    ds.setDepthCompareFunction(f);
    ds.setDepthWriteEnabled(write);
    ctx.device.newDepthStencilStateWithDescriptor(&ds)
}

impl WorldRenderer {
    /// `ns_view` is the Bevy/winit window's content view. Our view goes directly below it.
    pub fn new(ns_view: *mut c_void) -> Option<Self> {
        Self::new_inner(Some(ns_view))
    }

    /// No window or layer: only `render_offscreen` works. For `--headless` captures.
    pub fn new_headless() -> Option<Self> {
        Self::new_inner(None)
    }

    fn new_inner(ns_view: Option<*mut c_void>) -> Option<Self> {
        let ctx = MetalContext::new().ok()?;
        let probe_dummy = mk_probe_tex(&ctx.device, (1, 1, 1), &[0u16; 4])?;
        let lib = match ctx.library(include_str!("../shaders/world.metal")) {
            Ok(l) => l,
            Err(e) => {
                log::error!("world.metal: {e}");
                return None;
            }
        };
        let f = |n: &str| lib.newFunctionWithName(&NSString::from_str(n));
        let mk = |vs: &str, fs: Option<&str>, depth: bool, colors: &[MTLPixelFormat], blend: bool, mask_second: bool| -> Option<Pso> {
            let d = MTLRenderPipelineDescriptor::new();
            let v = f(vs)?;
            d.setVertexFunction(Some(&v));
            if let Some(fs) = fs {
                let fr = f(fs)?;
                d.setFragmentFunction(Some(&fr));
            }
            if depth {
                d.setDepthAttachmentPixelFormat(DEPTH);
            }
            for (i, fmt) in colors.iter().enumerate() {
                let ca = unsafe { d.colorAttachments().objectAtIndexedSubscript(i) };
                ca.setPixelFormat(*fmt);
                if i == 0 && blend {
                    ca.setBlendingEnabled(true);
                    ca.setSourceRGBBlendFactor(MTLBlendFactor::SourceAlpha);
                    ca.setDestinationRGBBlendFactor(MTLBlendFactor::OneMinusSourceAlpha);
                    ca.setSourceAlphaBlendFactor(MTLBlendFactor::Zero);
                    ca.setDestinationAlphaBlendFactor(MTLBlendFactor::One);
                }
                if i == 1 && mask_second {
                    ca.setWriteMask(MTLColorWriteMask::None);
                }
            }
            match ctx.device.newRenderPipelineStateWithDescriptor_error(&d) {
                Ok(p) => Some(p),
                Err(e) => {
                    log::error!("pso {vs}/{fs:?}: {e:?}");
                    None
                }
            }
        };
        let shadow_pso = mk("shadow_vs", None, true, &[], false, false)?;
        let shadow_mask_pso = mk("shadow_vs", Some("shadow_fs_mask"), true, &[], false, false)?;
        let sky_pso = mk("fs_vs", Some("sky_fs"), true, &[HDR, NRM], false, true)?;
        let opaque_pso = mk("world_vs", Some("world_fs"), true, &[HDR, NRM], false, false)?;
        let blend_pso = mk("world_vs", Some("world_fs_blend"), true, &[HDR, NRM], true, true)?;
        let terrain_pso = mk("terrain_vs", Some("terrain_fs"), true, &[HDR, NRM], false, false)?;
        let impostor_pso = mk("impostor_vs", Some("impostor_fs"), true, &[HDR, NRM], false, false)?;
        let post = crate::post::PostChain::new(&ctx);
        let upscaler = crate::upscale::Upscaler::new(&ctx);
        let resolve_pso = mk("fs_vs", Some("resolve_fs"), false, &[MTLPixelFormat::BGRA8Unorm], false, false)?;
        let ds_shadow = depth_state(&ctx, MTLCompareFunction::Less, true)?;
        let ds_opaque = depth_state(&ctx, MTLCompareFunction::Greater, true)?; // reverse-Z
        let ds_blend = depth_state(&ctx, MTLCompareFunction::Greater, false)?;
        let ds_sky = depth_state(&ctx, MTLCompareFunction::Always, false)?;

        let sd = unsafe { MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(DEPTH, SHADOW_RES, SHADOW_RES, false) };
        sd.setTextureType(MTLTextureType::Type2DArray);
        unsafe { sd.setArrayLength(NCASC) };
        sd.setUsage(MTLTextureUsage::RenderTarget | MTLTextureUsage::ShaderRead);
        sd.setStorageMode(MTLStorageMode::Private);
        let shadow_map = ctx.device.newTextureWithDescriptor(&sd)?;

        let wd = unsafe { MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(MTLPixelFormat::RGBA8Unorm, 1, 1, false) };
        let white = ctx.device.newTextureWithDescriptor(&wd)?;
        let px = [255u8; 4];
        unsafe {
            white.replaceRegion_mipmapLevel_withBytes_bytesPerRow(
                MTLRegion { origin: MTLOrigin { x: 0, y: 0, z: 0 }, size: MTLSize { width: 1, height: 1, depth: 1 } },
                0,
                NonNull::new(px.as_ptr() as *mut c_void).unwrap(),
                4,
            );
        }

        let neutral = ctx.device.newTextureWithDescriptor(&wd)?;
        let npx = [128u8, 128, 128, 0];
        unsafe {
            neutral.replaceRegion_mipmapLevel_withBytes_bytesPerRow(
                MTLRegion { origin: MTLOrigin { x: 0, y: 0, z: 0 }, size: MTLSize { width: 1, height: 1, depth: 1 } },
                0,
                NonNull::new(npx.as_ptr() as *mut c_void).unwrap(),
                4,
            );
        }

        // Blender Filmic 3D LUT (64^3 RGBA16F), the same table Bevy's `Tonemapping::BlenderFilmic` samples.
        let fd = unsafe { MTLTextureDescriptor::new() };
        fd.setTextureType(MTLTextureType::Type3D);
        fd.setPixelFormat(MTLPixelFormat::RGBA16Float);
        unsafe {
            fd.setWidth(64);
            fd.setHeight(64);
            fd.setDepth(64);
        }
        fd.setUsage(MTLTextureUsage::ShaderRead);
        let filmic = ctx.device.newTextureWithDescriptor(&fd)?;
        let lut: &[u8] = include_bytes!("../assets/filmic_lut_rgba16f.bin");
        unsafe {
            filmic.replaceRegion_mipmapLevel_slice_withBytes_bytesPerRow_bytesPerImage(
                MTLRegion { origin: MTLOrigin { x: 0, y: 0, z: 0 }, size: MTLSize { width: 64, height: 64, depth: 64 } },
                0,
                0,
                NonNull::new(lut.as_ptr() as *mut c_void).unwrap(),
                64 * 8,
                64 * 64 * 8,
            );
        }

        let (layer, view, host) = match ns_view {
            Some(ns_view) => {
                let mtm = MainThreadMarker::new()?;
                let (l, v, h) = Self::make_view(&ctx, mtm, ns_view)?;
                (Some(l), Some(v), Some(h))
            }
            None => (None, None, None),
        };
        let timers = GpuTimers::new(&ctx.device);
        Some(Self {
            ctx,
            layer,
            _view: view,
            host,
            frames: 0,
            shadow_pso,
            shadow_mask_pso,
            sky_pso,
            opaque_pso,
            blend_pso,
            terrain_pso,
            ground: None,
            imp: None,
            probe: None,
            probe_dummy,
            probe_box: [0.0; 6],
            probe_k: 0.0,
            impostor_pso,
            resolve_pso,
            ds_shadow,
            ds_opaque,
            ds_blend,
            ds_sky,
            shadow_map,
            white,
            neutral,
            filmic,
            post,
            upscaler,
            fxaa: true,
            render_scale: std::env::var("EW_NATIVE_SCALE").ok().and_then(|v| v.parse().ok()).unwrap_or(0.67),
            prev_cam: None,
            targets: None,
            meshes: HashMap::new(),
            textures: HashMap::new(),
            timers,
            times: PassTimes::new(),
            draws: 0,
            tris: 0,
            shadow_draws: 0,
            shadow_skipped: 0,
            shadow_keys: [0; NCASC],
        })
    }

    fn make_view(ctx: &MetalContext, mtm: MainThreadMarker, ns_view: *mut c_void) -> Option<(Retained<CAMetalLayer>, Retained<NSView>, Retained<NSView>)> {
        let host: &NSView = unsafe { (ns_view as *const NSView).as_ref()? };
        let layer = unsafe { CAMetalLayer::new() };
        layer.setDevice(Some(&ctx.device));
        layer.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
        unsafe {
            let _: () = msg_send![&layer, setDisplaySyncEnabled: false];
            let _: () = msg_send![&layer, setMaximumDrawableCount: 3usize];
        }
        let view = NSView::new(mtm);
        view.setWantsLayer(true);
        view.setLayer(Some(&layer));
        view.setFrame(host.frame());
        view.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable);
        unsafe {
            // Make sure everything above our view is see-through: host layer, window and its background.
            if let Some(hl) = host.layer() {
                let _: () = msg_send![&hl, setOpaque: false];
            }
            if let Some(win) = host.window() {
                let _: () = msg_send![&win, setOpaque: false];
                let clear: *mut objc2::runtime::AnyObject = msg_send![objc2::class!(NSColor), clearColor];
                let _: () = msg_send![&win, setBackgroundColor: clear];
            }
        }
        if let Some(parent) = unsafe { host.superview() } {
            parent.addSubview_positioned_relativeTo(&view, NSWindowOrderingMode::Below, Some(host));
        } else {
            return None;
        }
        if std::env::var_os("EW_NATIVE_HIDE_HOST").is_some() {
            host.setHidden(true);
        }
        Some((layer, view, unsafe { Retained::retain(host as *const NSView as *mut NSView)? }))
    }

    pub fn set_hidden(&self, hidden: bool) {
        if let Some(v) = &self._view {
            if v.isHidden() != hidden {
                v.setHidden(hidden);
            }
        }
    }

    pub fn has_mesh(&self, id: u64) -> bool {
        self.meshes.contains_key(&id)
    }
    pub fn mesh(&self, id: u64) -> Option<&GpuMesh> {
        self.meshes.get(&id)
    }
    pub fn retain_meshes(&mut self, mut keep: impl FnMut(u64) -> bool) {
        self.meshes.retain(|k, _| keep(*k));
    }
    pub fn mesh_count(&self) -> usize {
        self.meshes.len()
    }
    pub fn has_texture(&self, id: u64) -> bool {
        self.textures.contains_key(&id)
    }
    pub fn texture_count(&self) -> usize {
        self.textures.len()
    }
    pub fn retain_textures(&mut self, mut keep: impl FnMut(u64) -> bool) {
        self.textures.retain(|k, _| keep(*k));
    }

    pub fn upload(&mut self, id: u64, positions: &[[f32; 3]], normals: &[[f32; 3]], uvs: &[[f32; 2]], colors: &[[f32; 4]], indices: &[u32]) {
        if positions.is_empty() || indices.is_empty() {
            return;
        }
        let mut data: Vec<f32> = Vec::with_capacity(positions.len() * 12);
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for (i, p) in positions.iter().enumerate() {
            let n = normals.get(i).copied().unwrap_or([0.0, 1.0, 0.0]);
            let uv = uvs.get(i).copied().unwrap_or([0.0; 2]);
            let c = colors.get(i).copied().unwrap_or([1.0; 4]);
            data.extend_from_slice(&[p[0], p[1], p[2], n[0], n[1], n[2], uv[0], uv[1], c[0], c[1], c[2], c[3]]);
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        let center = [(lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5, (lo[2] + hi[2]) * 0.5];
        let radius = ((hi[0] - lo[0]).powi(2) + (hi[1] - lo[1]).powi(2) + (hi[2] - lo[2]).powi(2)).sqrt() * 0.5;
        // Big merged meshes: sort triangles into an xz grid so passes can cull sub-ranges.
        let mut idx: Vec<u32> = indices.to_vec();
        let mut clusters: Vec<Cluster> = Vec::new();
        let mut pidx: Vec<u32> = Vec::new();
        let ntri = indices.len() / 3;
        if ntri > 40_000 {
            let n = ((ntri as f32 / 20_000.0).sqrt().ceil() as usize).clamp(2, 24);
            let (sx, sz) = ((hi[0] - lo[0]).max(1e-3), (hi[2] - lo[2]).max(1e-3));
            let cell_of = |t: usize| -> usize {
                let (a, b, c) = (positions[indices[t * 3] as usize], positions[indices[t * 3 + 1] as usize], positions[indices[t * 3 + 2] as usize]);
                let cx = (a[0] + b[0] + c[0]) / 3.0;
                let cz = (a[2] + b[2] + c[2]) / 3.0;
                let ix = (((cx - lo[0]) / sx * n as f32) as usize).min(n - 1);
                let iz = (((cz - lo[2]) / sz * n as f32) as usize).min(n - 1);
                iz * n + ix
            };
            let mut counts = vec![0usize; n * n + 1];
            let cells: Vec<usize> = (0..ntri).map(|t| cell_of(t)).collect();
            for &c in &cells {
                counts[c + 1] += 1;
            }
            for i in 0..n * n {
                counts[i + 1] += counts[i];
            }
            let starts = counts.clone();
            let mut cur = counts;
            for (t, &c) in cells.iter().enumerate() {
                let o = cur[c] * 3;
                idx[o..o + 3].copy_from_slice(&indices[t * 3..t * 3 + 3]);
                cur[c] += 1;
            }
            for c in 0..n * n {
                let (a, b) = (starts[c] * 3, starts[c + 1] * 3);
                if a == b {
                    continue;
                }
                let (mut l, mut h) = ([f32::MAX; 3], [f32::MIN; 3]);
                for &vi in &idx[a..b] {
                    let p = positions[vi as usize];
                    for k in 0..3 {
                        l[k] = l[k].min(p[k]);
                        h[k] = h[k].max(p[k]);
                    }
                }
                let cc = [(l[0] + h[0]) * 0.5, (l[1] + h[1]) * 0.5, (l[2] + h[2]) * 0.5];
                let r = ((h[0] - l[0]).powi(2) + (h[1] - l[1]).powi(2) + (h[2] - l[2]).powi(2)).sqrt() * 0.5;
                let mut proxy = [(0usize, 0usize); 3];
                for (k, cell) in [0.5f32, 1.5, 4.0].iter().enumerate() {
                    let simp = simplify(&idx[a..b], positions, *cell);
                    proxy[k] = (pidx.len(), simp.len());
                    pidx.extend_from_slice(&simp);
                }
                clusters.push(Cluster { first: a, count: b - a, proxy, c: cc, r });
            }
        }
        let mk = |ptr: *const u8, len: usize| unsafe { self.ctx.device.newBufferWithBytes_length_options(NonNull::new(ptr as *mut c_void).unwrap(), len, MTLResourceOptions::StorageModeShared) };
        let (Some(vb), Some(ib)) = (mk(data.as_ptr() as *const u8, data.len() * 4), mk(idx.as_ptr() as *const u8, idx.len() * 4)) else { return };
        let ib2 = if pidx.is_empty() { None } else { mk(pidx.as_ptr() as *const u8, pidx.len() * 4) };
        self.meshes.insert(id, GpuMesh { vb, ib, index_count: indices.len(), center, radius, clusters, ib2, splat: None, extra: None });
    }

    /// Per-vertex splat weights (12 bytes per vertex: 12 layer weights) for an uploaded terrain mesh.
    pub fn upload_splat(&mut self, id: u64, weights: &[[u8; 12]]) {
        let Some(m) = self.meshes.get_mut(&id) else { return };
        if weights.is_empty() {
            return;
        }
        m.splat = unsafe { self.ctx.device.newBufferWithBytes_length_options(NonNull::new(weights.as_ptr() as *mut c_void).unwrap(), weights.len() * 12, MTLResourceOptions::StorageModeShared) };
    }

    /// Impostor vertex stream: `params` (radius, layer, kept share, near), `axes` (xyz + thinning rank), tile uv.
    pub fn upload_extra(&mut self, id: u64, params: &[[f32; 4]], axes: &[[f32; 4]], tile_uv: &[[f32; 2]]) {
        let Some(m) = self.meshes.get_mut(&id) else { return };
        let n = params.len().min(axes.len()).min(tile_uv.len());
        if n == 0 {
            return;
        }
        let mut d: Vec<f32> = Vec::with_capacity(n * 10);
        for i in 0..n {
            d.extend_from_slice(&params[i]);
            d.extend_from_slice(&axes[i]);
            d.extend_from_slice(&tile_uv[i]);
        }
        // Quads must never be simplified, and cluster bounds only cover the tree centres: pad by the grown radius.
        m.ib2 = None;
        let pad = params.iter().take(n).map(|q| q[0]).fold(0.0f32, f32::max) * 6.6 + 1.0;
        for c in &mut m.clusters {
            c.r += pad;
        }
        m.extra = unsafe { self.ctx.device.newBufferWithBytes_length_options(NonNull::new(d.as_ptr() as *mut c_void).unwrap(), d.len() * 4, MTLResourceOptions::StorageModeShared) };
    }

    /// Probe GI volume (RGBA16F 3D texture, `texels` are f16 bit patterns, x fastest). `box_min`/`box_max`
    /// are the world-space corners. `k` is the bounce intensity (0 hides the volume).
    pub fn upload_probe_volume(&mut self, dims: (usize, usize, usize), texels: &[u16], box_min: [f32; 3], box_max: [f32; 3]) {
        let Some(t) = mk_probe_tex(&self.ctx.device, dims, texels) else { return };
        self.probe = Some(t);
        self.probe_box = [box_min[0], box_min[1], box_min[2], box_max[0], box_max[1], box_max[2]];
    }

    pub fn set_probe_intensity(&mut self, k: f32) {
        self.probe_k = k;
    }

    pub fn has_atlas(&self) -> bool {
        self.imp.is_some()
    }

    /// Hemi-octahedral impostor atlas: `layers` slices of `size`^2, `mips` levels each, layer-major
    /// (all mips of layer 0, then layer 1, ...). Albedo is sRGB, normals linear.
    pub fn upload_impostor_atlas(&mut self, size: usize, layers: usize, mips: usize, albedo: &[u8], normal: &[u8]) {
        let mk = |fmt: MTLPixelFormat, data: &[u8]| -> Option<Tex> {
            let d = unsafe { MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(fmt, size, size, true) };
            d.setTextureType(MTLTextureType::Type2DArray);
            unsafe {
                d.setArrayLength(layers);
                d.setMipmapLevelCount(mips);
            }
            d.setUsage(MTLTextureUsage::ShaderRead);
            let t = self.ctx.device.newTextureWithDescriptor(&d)?;
            let mut off = 0usize;
            for l in 0..layers {
                for m in 0..mips {
                    let w = (size >> m).max(1);
                    let bytes = w * w * 4;
                    if off + bytes > data.len() {
                        return Some(t);
                    }
                    unsafe {
                        t.replaceRegion_mipmapLevel_slice_withBytes_bytesPerRow_bytesPerImage(
                            MTLRegion { origin: MTLOrigin { x: 0, y: 0, z: 0 }, size: MTLSize { width: w, height: w, depth: 1 } },
                            m,
                            l,
                            NonNull::new(data[off..].as_ptr() as *mut c_void).unwrap(),
                            w * 4,
                            bytes,
                        );
                    }
                    off += bytes;
                }
            }
            Some(t)
        };
        let (Some(al), Some(nr)) = (mk(MTLPixelFormat::RGBA8Unorm_sRGB, albedo), mk(MTLPixelFormat::RGBA8Unorm, normal)) else { return };
        self.imp = Some(ImpAtlas { al, nr });
    }

    /// Ground layers: `al`/`nr` are 12 RGBA8 layers of `size`^2 (level 0 only; mips are generated),
    /// `mt` the macro-variation texture. `sizes`/`bump` are per layer. `strength` scales all normals.
    pub fn upload_ground(&mut self, size: usize, al: &[&[u8]], nr: &[&[u8]], mt: &[u8], msize: usize, sizes: &[f32; 12], bump: &[f32; 12], strength: f32) {
        let arr = |fmt: MTLPixelFormat, layers: &[&[u8]]| -> Option<Tex> {
            let d = unsafe { MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(fmt, size, size, true) };
            d.setTextureType(MTLTextureType::Type2DArray);
            unsafe { d.setArrayLength(layers.len()) };
            d.setUsage(MTLTextureUsage::ShaderRead);
            let t = self.ctx.device.newTextureWithDescriptor(&d)?;
            for (i, l) in layers.iter().enumerate() {
                unsafe {
                    t.replaceRegion_mipmapLevel_slice_withBytes_bytesPerRow_bytesPerImage(
                        MTLRegion { origin: MTLOrigin { x: 0, y: 0, z: 0 }, size: MTLSize { width: size, height: size, depth: 1 } },
                        0,
                        i,
                        NonNull::new(l.as_ptr() as *mut c_void).unwrap(),
                        size * 4,
                        size * size * 4,
                    );
                }
            }
            Some(t)
        };
        let (Some(a), Some(n)) = (arr(MTLPixelFormat::RGBA8Unorm_sRGB, al), arr(MTLPixelFormat::RGBA8Unorm, nr)) else { return };
        let md = unsafe { MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(MTLPixelFormat::RGBA8Unorm, msize, msize, true) };
        md.setUsage(MTLTextureUsage::ShaderRead);
        let Some(m) = self.ctx.device.newTextureWithDescriptor(&md) else { return };
        unsafe {
            m.replaceRegion_mipmapLevel_withBytes_bytesPerRow(
                MTLRegion { origin: MTLOrigin { x: 0, y: 0, z: 0 }, size: MTLSize { width: msize, height: msize, depth: 1 } },
                0,
                NonNull::new(mt.as_ptr() as *mut c_void).unwrap(),
                msize * 4,
            );
        }
        if let Some(cb) = self.ctx.queue.commandBuffer() {
            if let Some(blit) = cb.blitCommandEncoder() {
                blit.generateMipmapsForTexture(&a);
                blit.generateMipmapsForTexture(&n);
                blit.generateMipmapsForTexture(&m);
                blit.endEncoding();
            }
            cb.commit();
        }
        let mut u = [0f32; 28];
        u[..12].copy_from_slice(sizes);
        u[12..24].copy_from_slice(bump);
        u[24] = strength;
        u[25] = 1.0;
        let Some(ub) = (unsafe { self.ctx.device.newBufferWithBytes_length_options(NonNull::new(u.as_ptr() as *mut c_void).unwrap(), u.len() * 4, MTLResourceOptions::StorageModeShared) }) else { return };
        self.ground = Some(Ground { al: a, nr: n, mt: m, u: ub });
    }

    /// Upload an RGBA8 image (tightly packed rows) with a full mip chain. `srgb` picks the view format.
    pub fn upload_texture(&mut self, id: u64, w: usize, h: usize, rgba: &[u8], srgb: bool) {
        if w == 0 || h == 0 || rgba.len() < w * h * 4 {
            return;
        }
        let fmt = if srgb { MTLPixelFormat::RGBA8Unorm_sRGB } else { MTLPixelFormat::RGBA8Unorm };
        let d = unsafe { MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(fmt, w, h, true) };
        d.setUsage(MTLTextureUsage::ShaderRead);
        let Some(t) = self.ctx.device.newTextureWithDescriptor(&d) else { return };
        unsafe {
            t.replaceRegion_mipmapLevel_withBytes_bytesPerRow(
                MTLRegion { origin: MTLOrigin { x: 0, y: 0, z: 0 }, size: MTLSize { width: w, height: h, depth: 1 } },
                0,
                NonNull::new(rgba.as_ptr() as *mut c_void).unwrap(),
                w * 4,
            );
        }
        if let Some(cb) = self.ctx.queue.commandBuffer() {
            if let Some(blit) = cb.blitCommandEncoder() {
                blit.generateMipmapsForTexture(&t);
                blit.endEncoding();
            }
            cb.commit();
        }
        self.textures.insert(id, t);
    }

    fn targets(&mut self, w: usize, h: usize) -> (Tex, Tex, Tex) {
        if let Some(t) = &self.targets {
            if t.w == w && t.h == h {
                return (t.hdr.clone(), t.nrm.clone(), t.depth.clone());
            }
        }
        let mk = |fmt: MTLPixelFormat, usage: MTLTextureUsage| {
            let d = unsafe { MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(fmt, w, h, false) };
            d.setUsage(usage);
            d.setStorageMode(MTLStorageMode::Private);
            self.ctx.device.newTextureWithDescriptor(&d).unwrap()
        };
        let u = MTLTextureUsage::RenderTarget | MTLTextureUsage::ShaderRead | MTLTextureUsage::ShaderWrite;
        let t = Targets { w, h, hdr: mk(HDR, u), nrm: mk(NRM, u), depth: mk(DEPTH, u) };
        let r = (t.hdr.clone(), t.nrm.clone(), t.depth.clone());
        self.targets = Some(t);
        r
    }

    /// wgpu adds its own CAMetalLayer under the host view's layer and may mark it opaque; keep every
    /// layer above the native view see-through so the world shows through.
    fn clear_host_opacity(&self) {
        fn walk(l: &objc2_quartz_core::CALayer) {
            unsafe {
                let _: () = msg_send![l, setOpaque: false];
                if let Some(subs) = l.sublayers() {
                    for s in subs.iter() {
                        walk(&s);
                    }
                }
            }
        }
        if let Some(h) = &self.host {
            if let Some(l) = unsafe { h.layer() } {
                walk(&l);
            }
        }
    }

    pub fn render(&mut self, p: &FrameParams, draws: &[DrawItem]) {
        let Some(layer) = self.layer.clone() else { return };
        if self.frames % 30 == 0 {
            self.clear_host_opacity();
        }
        layer.setDrawableSize(CGSize::new(p.size.0 as f64, p.size.1 as f64));
        unsafe {
            let _: () = msg_send![&layer, setContentsScale: p.scale];
        }
        let Some(drawable) = layer.nextDrawable() else { return };
        let dtex = drawable.texture();
        let cb = self.encode(p, draws, &dtex);
        cb.presentDrawable(ProtocolObject::from_ref(&*drawable));
        self.finish(&cb);
    }

    /// Render one frame into a CPU-readable texture and return tightly packed RGBA8 rows (w*h*4).
    /// Menu graphics options (AO / bloom are env-overridable defaults in `PostChain`).
    pub fn set_options(&mut self, ao: bool, bloom: bool, aa: bool, render_scale: f32) {
        if let Some(pc) = self.post.as_mut() {
            pc.ao_on = ao && std::env::var_os("EW_NATIVE_NOAO").is_none();
            pc.bloom_on = bloom && std::env::var_os("EW_NATIVE_NOBLOOM").is_none();
        }
        self.fxaa = aa;
        self.render_scale = render_scale.clamp(0.4, 1.0);
    }

    pub fn render_offscreen(&mut self, p: &FrameParams, draws: &[DrawItem]) -> Option<Vec<u8>> {
        let (w, h) = (p.size.0 as usize, p.size.1 as usize);
        let d = unsafe { MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(MTLPixelFormat::BGRA8Unorm, w, h, false) };
        d.setUsage(MTLTextureUsage::RenderTarget);
        d.setStorageMode(MTLStorageMode::Shared);
        let tex = self.ctx.device.newTextureWithDescriptor(&d)?;
        let cb = self.encode(p, draws, &tex);
        self.finish(&cb);
        cb.waitUntilCompleted();
        let mut px = vec![0u8; w * h * 4];
        unsafe {
            tex.getBytes_bytesPerRow_fromRegion_mipmapLevel(
                NonNull::new(px.as_mut_ptr() as *mut c_void)?,
                w * 4,
                MTLRegion { origin: MTLOrigin { x: 0, y: 0, z: 0 }, size: MTLSize { width: w, height: h, depth: 1 } },
                0,
            );
        }
        for c in px.chunks_exact_mut(4) {
            c.swap(0, 2);
        }
        Some(px)
    }

    fn finish(&mut self, cb: &ProtocolObject<dyn MTLCommandBuffer>) {
        cb.commit();
        self.timers.end_frame();
        for (name, ms) in self.timers.resolve_oldest() {
            self.times.record(&name, ms);
        }
    }

    fn encode(&mut self, p: &FrameParams, draws: &[DrawItem], dtex: &Tex) -> Retained<ProtocolObject<dyn MTLCommandBuffer>> {
        self.frames += 1;
        let (ow, oh) = (dtex.width(), dtex.height());
        let up_on = self.render_scale < 0.99 && self.upscaler.is_some();
        let (iw, ih) = if up_on { (((ow as f32 * self.render_scale) as usize).max(64) & !1, ((oh as f32 * self.render_scale) as usize).max(64) & !1) } else { (ow, oh) };
        let (hdr, nrm, depth) = self.targets(iw, ih);
        let vp_unjit = Mat4::from_cols_array_2d(&p.view_proj);
        let jit = if up_on { crate::upscale::jitter(self.frames as u32) } else { (0.0, 0.0) };
        let mut vp_arr = p.view_proj;
        if up_on {
            for col in vp_arr.iter_mut() {
                col[0] += (2.0 * jit.0 / iw as f32) * col[3];
                col[1] += (-2.0 * jit.1 / ih as f32) * col[3];
            }
        }
        let cam_jump = self.prev_cam.is_some_and(|c| (Vec3::from(c) - Vec3::from(p.cam_pos)).length() > 300.0);
        self.prev_cam = Some(p.cam_pos);

        let casc = fit_cascades(p);
        let mut fu = FrameU {
            vp: vp_arr,
            sun: p.sun,
            cam: [p.cam_pos[0], p.cam_pos[1], p.cam_pos[2], 0.0],
            sky: [p.sky[0], p.sky[1], p.sky[2], 1.0],
            fog: [p.fog_density, if up_on || !self.fxaa || std::env::var_os("EW_NATIVE_NOAA").is_some() { 1.0 } else { 0.0 }, ih as f32 / p.tan_half_fov.max(1e-4), 0.0],
            csm: [[[0.0; 4]; 4]; NCASC],
            csm_info: casc.info,
            csm_split: [casc.splits[0], casc.splits[1], casc.splits[2], casc.splits[3]],
            csm_far: [casc.splits[5], casc.splits[4], 0.0, 0.0],
            cr: [0.0; 4],
            cu: [0.0; 4],
            cf: [p.cam_fwd[0], p.cam_fwd[1], p.cam_fwd[2], 0.0],
            pb0: if self.probe.is_some() { [self.probe_box[0], self.probe_box[1], self.probe_box[2], self.probe_k] } else { [0.0; 4] },
            pb1: { let b = self.probe_box; [1.0 / (b[3] - b[0]).max(1e-3), 1.0 / (b[4] - b[1]).max(1e-3), 1.0 / (b[5] - b[2]).max(1e-3), 0.0] },
            params: [p.exposure, if p.shadows { 1.0 } else { 0.0 }, 1.0 / SHADOW_RES as f32, std::env::var("EW_NATIVE_DBG").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0)],
        };
        for i in 0..NCASC {
            fu.csm[i] = casc.vp[i].to_cols_array_2d();
        }
        let tx = p.tan_half_fov * p.aspect;
        for k in 0..3 {
            fu.cr[k] = p.cam_right[k] * tx;
            fu.cu[k] = p.cam_up[k] * p.tan_half_fov;
        }

        // World-space bounding spheres, computed once for all passes.
        struct B { c: Vec3, r: f32, mm: Mat4, s: f32 }
        let bounds: Vec<Option<B>> = draws
            .iter()
            .map(|d| {
                let m = self.meshes.get(&d.mesh)?;
                let mm = Mat4::from_cols_array_2d(&d.model);
                let s = mm.x_axis.truncate().length().max(mm.y_axis.truncate().length()).max(mm.z_axis.truncate().length());
                Some(B { c: mm.transform_point3(Vec3::from(m.center)), r: m.radius * s, mm, s })
            })
            .collect();

        self.timers.begin_frame();
        let cb = self.ctx.queue.commandBuffer().unwrap();
        let mut shadow_draws = 0usize;

        // ---- shadow cascades ----
        if p.shadows {
            self.shadow_skipped = 0;
            for ci in 0..NCASC {
                // Caster set for this cascade: in range, and big enough to matter at this texel size.
                let texel = casc.info[ci][0];
                let min_r = if ci == 0 || ci == 5 { 0.0 } else { texel * 1.5 };
                let mut list: Vec<usize> = Vec::new();
                let mut key: u64 = 0xcbf29ce484222325;
                let mix = |k: u64, v: u64| (k ^ v).wrapping_mul(0x100000001b3);
                for (i, (d, b)) in draws.iter().zip(&bounds).enumerate() {
                    let Some(b) = b else { continue };
                    if !d.cast_shadow || d.blend == Blend::Alpha || b.r < min_r || (ci == 5 && !d.terrain) {
                        continue;
                    }
                    let l = casc.v0.transform_point3(b.c);
                    let dx = l.x - casc.ctr[ci][0];
                    let dy = l.y - casc.ctr[ci][1];
                    let lim = casc.rad[ci] + b.r;
                    if dx * dx + dy * dy > lim * lim {
                        continue;
                    }
                    list.push(i);
                    key = mix(key, d.mesh);
                    key = mix(key, (d.model[3][0] / texel).round() as i64 as u64);
                    key = mix(key, (d.model[3][1] / texel).round() as i64 as u64);
                    key = mix(key, (d.model[3][2] / texel).round() as i64 as u64);
                }
                for v in casc.vp[ci].to_cols_array() {
                    key = mix(key, v.to_bits() as u64);
                }
                if key == self.shadow_keys[ci] {
                    self.shadow_skipped += 1;
                    continue;
                }
                self.shadow_keys[ci] = key;
                let pass = MTLRenderPassDescriptor::new();
                let da = pass.depthAttachment();
                da.setTexture(Some(&self.shadow_map));
                da.setSlice(ci);
                da.setLoadAction(MTLLoadAction::Clear);
                da.setStoreAction(MTLStoreAction::Store);
                da.setClearDepth(1.0);
                self.timers.attach(&pass, &format!("shadow{ci}"));
                let re = cb.renderCommandEncoderWithDescriptor(&pass).unwrap();
                re.setDepthStencilState(Some(&self.ds_shadow));
                re.setCullMode(MTLCullMode::None);
                re.setFrontFacingWinding(MTLWinding::CounterClockwise);
                re.setDepthBias_slopeScale_clamp(0.0, 1.5, 0.0);
                let lvp = casc.vp[ci].to_cols_array_2d();
                unsafe { re.setVertexBytes_length_atIndex(NonNull::new(&lvp as *const _ as *mut c_void).unwrap(), 64, 1) };
                let mut cur_mask: Option<bool> = None;
                let no_proxy = std::env::var_os("EW_NATIVE_NOPROXY").is_some();
                for &i in &list {
                    let d = &draws[i];
                    let mask = d.blend == Blend::Mask;
                    if cur_mask != Some(mask) {
                        re.setRenderPipelineState(if mask { &self.shadow_mask_pso } else { &self.shadow_pso });
                        cur_mask = Some(mask);
                    }
                    let m = &self.meshes[&d.mesh];
                    let du = DrawU { model: d.model, tint: d.tint, emissive: [0.0; 4], mat: [d.rough, d.metal, d.cutoff, 0.0] };
                    unsafe {
                        re.setVertexBuffer_offset_atIndex(Some(&m.vb), 0, 0);
                        re.setVertexBytes_length_atIndex(NonNull::new(&du as *const DrawU as *mut c_void).unwrap(), std::mem::size_of::<DrawU>(), 2);
                        if mask {
                            re.setFragmentBytes_length_atIndex(NonNull::new(&du as *const DrawU as *mut c_void).unwrap(), std::mem::size_of::<DrawU>(), 2);
                            re.setFragmentTexture_atIndex(Some(self.textures.get(&d.tex).unwrap_or(&self.white)), 0);
                        }
                        if m.clusters.is_empty() {
                            re.drawIndexedPrimitives_indexCount_indexType_indexBuffer_indexBufferOffset(MTLPrimitiveType::Triangle, m.index_count, MTLIndexType::UInt32, &m.ib, 0);
                        } else {
                            let b = bounds[i].as_ref().unwrap();
                            for cl in &m.clusters {
                                let r = cl.r * b.s;
                                if r < min_r {
                                    continue;
                                }
                                let l = casc.v0.transform_point3(b.mm.transform_point3(Vec3::from(cl.c)));
                                let (dx, dy) = (l.x - casc.ctr[ci][0], l.y - casc.ctr[ci][1]);
                                let lim = casc.rad[ci] + r;
                                if dx * dx + dy * dy > lim * lim {
                                    continue;
                                }
                                let far0 = ci == 0 && (l_dist(&b.mm, cl, p.cam_pos)) > 60.0 + r;
                                match (if far0 { 1 } else { ci }, if no_proxy { &None } else { &m.ib2 }) {
                                    (0, _) | (_, None) => re.drawIndexedPrimitives_indexCount_indexType_indexBuffer_indexBufferOffset(MTLPrimitiveType::Triangle, cl.count, MTLIndexType::UInt32, &m.ib, cl.first * 4),
                                    (c, Some(ib2)) => {
                                        let (f, n) = cl.proxy[(c - 1).min(1)];
                                        if n > 0 {
                                            re.drawIndexedPrimitives_indexCount_indexType_indexBuffer_indexBufferOffset(MTLPrimitiveType::Triangle, n, MTLIndexType::UInt32, ib2, f * 4)
                                        }
                                    }
                                }
                                shadow_draws += 1;
                            }
                        }
                    }
                    shadow_draws += 1;
                }
                re.endEncoding();
            }
        }

        // ---- main pass: sky, opaque/mask, blended ----
        let pass = MTLRenderPassDescriptor::new();
        let ca = unsafe { pass.colorAttachments().objectAtIndexedSubscript(0) };
        ca.setTexture(Some(&hdr));
        ca.setLoadAction(MTLLoadAction::DontCare);
        ca.setStoreAction(MTLStoreAction::Store);
        let cn = unsafe { pass.colorAttachments().objectAtIndexedSubscript(1) };
        cn.setTexture(Some(&nrm));
        cn.setLoadAction(MTLLoadAction::Clear);
        cn.setClearColor(MTLClearColor { red: 0.0, green: 0.0, blue: 0.0, alpha: 0.0 });
        cn.setStoreAction(MTLStoreAction::Store);
        let da = pass.depthAttachment();
        da.setTexture(Some(&depth));
        da.setLoadAction(MTLLoadAction::Clear);
        da.setStoreAction(MTLStoreAction::Store);
        da.setClearDepth(0.0);
        self.timers.attach(&pass, "world");
        let re = cb.renderCommandEncoderWithDescriptor(&pass).unwrap();
        re.setFrontFacingWinding(MTLWinding::CounterClockwise);
        let fbytes = NonNull::new(&fu as *const FrameU as *mut c_void).unwrap();
        unsafe {
            re.setVertexBytes_length_atIndex(fbytes, std::mem::size_of::<FrameU>(), 1);
            re.setFragmentBytes_length_atIndex(fbytes, std::mem::size_of::<FrameU>(), 1);
            re.setFragmentTexture_atIndex(Some(&self.shadow_map), 1);
            re.setFragmentTexture_atIndex(Some(self.probe.as_ref().unwrap_or(&self.probe_dummy)), 8);
        }
        // sky
        re.setRenderPipelineState(&self.sky_pso);
        re.setDepthStencilState(Some(&self.ds_sky));
        re.setCullMode(MTLCullMode::None);
        unsafe { re.drawPrimitives_vertexStart_vertexCount(MTLPrimitiveType::Triangle, 0, 3) };

        let mut tris = 0u64;
        let mut n = 0usize;
        let (vcam, vf, vr, vu) = (Vec3::from(p.cam_pos), Vec3::from(p.cam_fwd), Vec3::from(p.cam_right), Vec3::from(p.cam_up));
        let (vtx, vty) = (p.tan_half_fov * p.aspect, p.tan_half_fov);
        let px_per_m = p.size.1 as f32 / (2.0 * p.tan_half_fov);
        let in_view = move |c: Vec3, r: f32| -> bool {
            let v = c - vcam;
            let z = v.dot(vf);
            if z + r < 0.0 {
                return false;
            }
            if v.dot(vr).abs() - r * (1.0 + vtx * vtx).sqrt() > z * vtx {
                return false;
            }
            v.dot(vu).abs() - r * (1.0 + vty * vty).sqrt() <= z * vty
        };
        let mut draw_one = |re: &ProtocolObject<dyn MTLRenderCommandEncoder>, d: &DrawItem, m: &GpuMesh, tex_cache: &mut u64, cur_t: &mut u8| {
            let du = DrawU {
                model: d.model,
                tint: d.tint,
                emissive: [d.emissive[0], d.emissive[1], d.emissive[2], if d.terrain { 1.0 } else { 0.0 }],
                mat: [d.rough, d.metal, if d.blend == Blend::Mask { d.cutoff.max(0.01) } else { 0.0 }, if d.unlit { 1.0 } else { 0.0 }],
            };
            let use_t = d.terrain && d.blend != Blend::Alpha && m.splat.is_some() && self.ground.is_some();
            let use_i = d.impostor && d.blend != Blend::Alpha && m.extra.is_some() && self.imp.is_some();
            let mode: u8 = if use_i { 2 } else if use_t { 1 } else { 0 };
            unsafe {
                if d.blend != Blend::Alpha && mode != *cur_t {
                    *cur_t = mode;
                    if use_i {
                        let a = self.imp.as_ref().unwrap();
                        re.setRenderPipelineState(&self.impostor_pso);
                        re.setFragmentTexture_atIndex(Some(&a.al), 6);
                        re.setFragmentTexture_atIndex(Some(&a.nr), 7);
                    } else if use_t {
                        let g = self.ground.as_ref().unwrap();
                        re.setRenderPipelineState(&self.terrain_pso);
                        re.setFragmentTexture_atIndex(Some(&g.al), 3);
                        re.setFragmentTexture_atIndex(Some(&g.nr), 4);
                        re.setFragmentTexture_atIndex(Some(&g.mt), 5);
                        re.setFragmentBuffer_offset_atIndex(Some(&g.u), 0, 3);
                    } else {
                        re.setRenderPipelineState(&self.opaque_pso);
                    }
                }
                if use_t {
                    re.setVertexBuffer_offset_atIndex(m.splat.as_deref(), 0, 3);
                }
                if use_i {
                    re.setVertexBuffer_offset_atIndex(m.extra.as_deref(), 0, 3);
                }
                re.setCullMode(if use_i { MTLCullMode::None } else if d.double_sided || d.blend != Blend::Opaque { MTLCullMode::None } else { MTLCullMode::Back });
                re.setVertexBuffer_offset_atIndex(Some(&m.vb), 0, 0);
                let ptr = NonNull::new(&du as *const DrawU as *mut c_void).unwrap();
                re.setVertexBytes_length_atIndex(ptr, std::mem::size_of::<DrawU>(), 2);
                re.setFragmentBytes_length_atIndex(ptr, std::mem::size_of::<DrawU>(), 2);
                let key = d.tex.wrapping_add(d.tex2.wrapping_mul(0x9E37_79B9_7F4A_7C15));
                if *tex_cache != key {
                    re.setFragmentTexture_atIndex(Some(self.textures.get(&d.tex).unwrap_or(&self.white)), 0);
                    re.setFragmentTexture_atIndex(Some(self.textures.get(&d.tex2).unwrap_or(&self.neutral)), 2);
                    *tex_cache = key;
                }
                if m.clusters.is_empty() {
                    re.drawIndexedPrimitives_indexCount_indexType_indexBuffer_indexBufferOffset(MTLPrimitiveType::Triangle, m.index_count, MTLIndexType::UInt32, &m.ib, 0);
                    tris += (m.index_count / 3) as u64;
                } else {
                    let mm = Mat4::from_cols_array_2d(&d.model);
                    let sc = mm.x_axis.truncate().length().max(mm.y_axis.truncate().length()).max(mm.z_axis.truncate().length());
                    for cl in &m.clusters {
                        let cw = mm.transform_point3(Vec3::from(cl.c));
                        if !in_view(cw, cl.r * sc) {
                            continue;
                        }
                        let dist = ((cw - vcam).length() - cl.r * sc * 0.5).max(1.0);
                        let lvl = if m.ib2.is_none() { None } else if 4.0 * px_per_m / dist < 4.0 { Some(2) } else if 1.5 * px_per_m / dist < 4.0 { Some(1) } else if 0.5 * px_per_m / dist < 4.0 { Some(0) } else { None };
                        match (lvl, &m.ib2) {
                            (Some(k), Some(ib2)) => {
                                let (f, n) = cl.proxy[k];
                                if n > 0 {
                                    re.drawIndexedPrimitives_indexCount_indexType_indexBuffer_indexBufferOffset(MTLPrimitiveType::Triangle, n, MTLIndexType::UInt32, ib2, f * 4);
                                    tris += (n / 3) as u64;
                                }
                            }
                            _ => {
                                re.drawIndexedPrimitives_indexCount_indexType_indexBuffer_indexBufferOffset(MTLPrimitiveType::Triangle, cl.count, MTLIndexType::UInt32, &m.ib, cl.first * 4);
                                tris += (cl.count / 3) as u64;
                            }
                        }
                    }
                }
            }
            n += 1;
        };
        re.setRenderPipelineState(&self.opaque_pso);
        re.setDepthStencilState(Some(&self.ds_opaque));
        let mut tc = u64::MAX;
        let mut cur_t = 0u8;
        for d in draws.iter().filter(|d| d.visible && d.blend != Blend::Alpha) {
            if let Some(m) = self.meshes.get(&d.mesh) {
                draw_one(&re, d, m, &mut tc, &mut cur_t);
            }
        }
        // blended, far to near
        let cam = Vec3::from(p.cam_pos);
        let mut blended: Vec<(f32, usize)> = draws
            .iter()
            .enumerate()
            .zip(&bounds)
            .filter_map(|((i, d), b)| if d.visible && d.blend == Blend::Alpha { b.as_ref().map(|b| ((b.c - cam).length_squared(), i)) } else { None })
            .collect();
        if !blended.is_empty() {
            blended.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
            re.setRenderPipelineState(&self.blend_pso);
            re.setDepthStencilState(Some(&self.ds_blend));
            tc = u64::MAX;
            for (_, i) in blended {
                let d = &draws[i];
                if let Some(m) = self.meshes.get(&d.mesh) {
                    draw_one(&re, d, m, &mut tc, &mut cur_t);
                }
            }
        }
        re.endEncoding();

        // ---- screen-space post: GTAO + bloom ----
        let post_tex = self.post.as_mut().map(|pc| {
            let pp = crate::post::PostParams { cam_right: p.cam_right, cam_up: p.cam_up, cam_fwd: p.cam_fwd, tan_half_fov: p.tan_half_fov, aspect: p.aspect, near: p.near };
            let t = pc.encode(&cb, &pp, &hdr, &nrm, &depth);
            (t, pc.ao_on, pc.bloom_on, pc.bounce_on)
        });

        // ---- MetalFX temporal upscale (internal -> output resolution) ----
        let hdr_final: Tex = if up_on {
            match self.upscaler.as_mut().and_then(|u| u.encode(&cb, &hdr, &depth, ow, oh, vp_unjit, jit, cam_jump)) {
                Some(t) => t,
                None => hdr.clone(),
            }
        } else {
            hdr.clone()
        };

        // ---- resolve to the drawable ----
        let pass = MTLRenderPassDescriptor::new();
        let ca = unsafe { pass.colorAttachments().objectAtIndexedSubscript(0) };
        ca.setTexture(Some(dtex));
        ca.setLoadAction(MTLLoadAction::DontCare);
        ca.setStoreAction(MTLStoreAction::Store);
        self.timers.attach(&pass, "resolve");
        let re = cb.renderCommandEncoderWithDescriptor(&pass).unwrap();
        re.setRenderPipelineState(&self.resolve_pso);
        unsafe {
            re.setFragmentBytes_length_atIndex(fbytes, std::mem::size_of::<FrameU>(), 1);
            re.setFragmentTexture_atIndex(Some(&hdr_final), 0);
            re.setFragmentTexture_atIndex(Some(&self.filmic), 1);
            let flags: [f32; 4] = match &post_tex {
                Some(((ao, bl, bo), a, b, c)) => {
                    re.setFragmentTexture_atIndex(Some(ao), 2);
                    re.setFragmentTexture_atIndex(Some(bl), 3);
                    re.setFragmentTexture_atIndex(Some(bo), 4);
                    [if *a { 1.0 } else { 0.0 }, if *b { 1.0 } else { 0.0 }, if std::env::var("EW_NATIVE_AOVIS").is_ok() { 1.0 } else { 0.0 }, if *c { 1.0 } else { 0.0 }]
                }
                None => [0.0; 4],
            };
            re.setFragmentBytes_length_atIndex(NonNull::new(flags.as_ptr() as *mut c_void).unwrap(), 16, 2);
            re.drawPrimitives_vertexStart_vertexCount(MTLPrimitiveType::Triangle, 0, 3);
        }
        re.endEncoding();

        self.draws = n;
        self.tris = tris;
        self.shadow_draws = shadow_draws;
        cb
    }

    pub fn recalibrate(&mut self) {
        self.timers.recalibrate(&self.ctx.device);
    }
}


/// Vertex-clustering decimation: each vertex snaps to the first vertex seen in its `cell`-sized grid cell
/// (so it stays on the real surface); collapsed and duplicate triangles are dropped.
fn simplify(idx: &[u32], pos: &[[f32; 3]], cell: f32) -> Vec<u32> {
    use std::collections::{HashMap, HashSet};
    let inv = 1.0 / cell;
    let mut rep_of_cell: HashMap<(i32, i32, i32), u32> = HashMap::new();
    let mut rep: HashMap<u32, u32> = HashMap::new();
    let mut map = |v: u32| -> u32 {
        *rep.entry(v).or_insert_with(|| {
            let p = pos[v as usize];
            *rep_of_cell.entry(((p[0] * inv).floor() as i32, (p[1] * inv).floor() as i32, (p[2] * inv).floor() as i32)).or_insert(v)
        })
    };
    let mut seen: HashSet<[u32; 3]> = HashSet::new();
    let mut out = Vec::new();
    for t in idx.chunks_exact(3) {
        let (a, b, c) = (map(t[0]), map(t[1]), map(t[2]));
        if a == b || b == c || a == c {
            continue;
        }
        let mut k = [a, b, c];
        k.sort_unstable();
        if seen.insert(k) {
            out.extend_from_slice(&[a, b, c]);
        }
    }
    out
}

fn l_dist(mm: &Mat4, cl: &Cluster, cam: [f32; 3]) -> f32 {
    (mm.transform_point3(Vec3::from(cl.c)) - Vec3::from(cam)).length()
}

fn mk_probe_tex(device: &ProtocolObject<dyn MTLDevice>, dims: (usize, usize, usize), texels: &[u16]) -> Option<Tex> {
    let d = unsafe { MTLTextureDescriptor::new() };
    d.setTextureType(MTLTextureType::Type3D);
    d.setPixelFormat(MTLPixelFormat::RGBA16Float);
    unsafe {
        d.setWidth(dims.0);
        d.setHeight(dims.1);
        d.setDepth(dims.2);
    }
    d.setUsage(MTLTextureUsage::ShaderRead);
    let t = device.newTextureWithDescriptor(&d)?;
    unsafe {
        t.replaceRegion_mipmapLevel_slice_withBytes_bytesPerRow_bytesPerImage(
            MTLRegion { origin: MTLOrigin { x: 0, y: 0, z: 0 }, size: MTLSize { width: dims.0, height: dims.1, depth: dims.2 } },
            0,
            0,
            NonNull::new(texels.as_ptr() as *mut c_void)?,
            dims.0 * 8,
            dims.0 * dims.1 * 8,
        );
    }
    Some(t)
}
