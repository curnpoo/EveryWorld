//! Physically based atmosphere (Phase 2 W5): the GPU side. One compute pass, "atmos", recorded
//! by [`Atmosphere::record_atmosphere`] before resolve, builds the tables of
//! `shaders/atmos_tables.slang` (transmittance and multiple scattering when the medium
//! changes; per view every frame: sky view from the camera and from the surface, the 32^3
//! aerial-perspective froxels, the ambient SH9). Resolve reads them through `FrameData.atmos`
//! (common.slang `AtmosData`): the sky, the specular environment, the sun reaching each shaded
//! point, the ambient and the aerial perspective over every opaque pixel.
//!
//! The model and the per-body constants are [`crate::atmosphere_model`] (CPU mirror, tests).
//!
//! World frame: the renderer's world is flat with y up (Phase 2 so far); the body's surface is
//! the plane y = [`AtmosphereSettings::surface_y`], local up is +Y, and the atmosphere above is
//! the body's spherical shell (the horizon dips and the far ground curves away). The same code
//! runs from 1.7 m to orbit: above the top of the atmosphere every ray starts where it enters.
//!
//! Off switch: `EW_NO_ATMOS=1` (the analytic M4 sky and sun, unchanged). Debug views:
//! `EW_ATMOS_DEBUG=transmittance` (view-ray transmittance: froxels on geometry, to the top of
//! the atmosphere on the sky, x0.5) or `=inscatter` (the in-scattered light alone).
//! Cost (M3 Pro, `ew --headless --path proving`, on vs `EW_NO_ATMOS=1`): the "atmos" pass is
//! 0.11 ms at any resolution (the tables do not scale with it); the per-pixel lookups add to
//! resolve 0.07 ms at 1280x720, 0.37 ms at 2560x1440, 0.45 ms at 2880x1800 (frame +0.2 / +0.45
//! / +0.57 ms).

use crate::terrain::CameraGround;
use crate::atmosphere_model::{self as model, Body};
use crate::graph::{FrameGraph, PassKind, Plan, Res};
use crate::gpu::{FD_ATMOS, FD_SUN_DIR, constants};
use crate::shaders::{Built, Module, ShaderSet};
use crate::{LightingSettings, Tracked};
use ed_gpu::{ComputePipelineId, Device, ResourceState, Transition, Usage};
use glam::{DVec3, Vec3};

/// `AtmosData` words (common.slang).
pub const ATMOS_WORDS: usize = 48;

const TRANS_TEXELS: usize = model::TRANS_W * model::TRANS_H;
const MS_TEXELS: usize = model::MS_N * model::MS_N;
const SKY: (u32, u32) = (192, 108);
const SURF: (u32, u32) = (96, 54);
const AP_N: u32 = 32;
/// Ambient SH buffer: 32 bands x 9 partial sums, then the 9 coefficients (atmos.slang `ATM_SH_OUT`).
const SH_FLOAT4S: usize = 32 * 9 + 9;

/// Atmosphere switches and frame placement.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AtmosphereSettings {
    /// Off falls back to the analytic M4 sky (`EW_NO_ATMOS=1`).
    pub enabled: bool,
    pub body: Body,
    /// World y (m) of the body's surface (the flat world's ground plane).
    pub surface_y: f64,
    /// Altitude (km) of that surface above the body's radius.
    pub surface_alt_km: f64,
    /// The camera's place on the curved body (see [`apply_camera_ground`]). When set it replaces
    /// the flat-world inputs: up, camera altitude and ground altitude come from it, and every
    /// ray is integrated spherically about the body's centre, `camera - up * (R + alt)`.
    pub ground: Option<CameraGround>,
    /// Froxel depth slices: exponential, `x = log(1 + t/n) / log(1 + D/n)`, inside the
    /// atmosphere (n, D) in km...
    pub ap_inside_km: (f32, f32),
    /// ...and from above it, measured from where each ray enters.
    pub ap_outside_km: (f32, f32),
    /// 0 off, 1 transmittance, 2 in-scattered light (`EW_ATMOS_DEBUG`).
    pub debug: u32,
}

impl Default for AtmosphereSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            body: Body::earth(),
            surface_y: 0.0,
            surface_alt_km: 0.0,
            ground: None,
            ap_inside_km: (0.1, 150.0),
            ap_outside_km: (1.0, 1200.0),
            debug: 0,
        }
    }
}

impl AtmosphereSettings {
    /// Defaults with `EW_NO_ATMOS=1`, `EW_ATMOS_DEBUG=transmittance|inscatter` and
    /// `EW_ATMOS_BODY=moon` applied.
    pub fn from_env() -> Self {
        let on = |v: &str| std::env::var(v).is_ok_and(|s| s != "0" && !s.is_empty());
        let debug = match std::env::var("EW_ATMOS_DEBUG").as_deref() {
            Ok("transmittance") | Ok("1") => 1,
            Ok("inscatter") | Ok("2") => 2,
            _ => 0,
        };
        let body = if std::env::var("EW_ATMOS_BODY").as_deref() == Ok("moon") { Body::moon() } else { Body::earth() };
        Self { enabled: !on("EW_NO_ATMOS"), body, debug, ..Self::default() }
    }
}

/// The atmosphere's compute pipelines (part of the renderer's pipeline set, rebuilt on hot reload).
pub(crate) struct AtmosPipes {
    trans: ComputePipelineId,
    ms: ComputePipelineId,
    sky: ComputePipelineId,
    ap: ComputePipelineId,
    sh_partial: ComputePipelineId,
    sh_final: ComputePipelineId,
}

impl AtmosPipes {
    pub fn build(dev: &dyn Device, src: &ShaderSet, b: &mut Built) -> Result<Self, ed_gpu::Error> {
        Ok(Self {
            trans: b.compute(dev, src, Module::Atmos, "atmos_transmittance", [8, 8, 1])?,
            ms: b.compute(dev, src, Module::Atmos, "atmos_multiscatter", [8, 8, 1])?,
            sky: b.compute(dev, src, Module::Atmos, "atmos_skyview", [8, 8, 1])?,
            ap: b.compute(dev, src, Module::Atmos, "atmos_ap", [8, 8, 1])?,
            sh_partial: b.compute(dev, src, Module::Atmos, "atmos_sh_partial", [32, 1, 1])?,
            sh_final: b.compute(dev, src, Module::Atmos, "atmos_sh_final", [32, 1, 1])?,
        })
    }
}

/// Per-view tables.
struct ViewTables {
    sky: Tracked,
    surf: Tracked,
    ap_l: Tracked,
    ap_t: Tracked,
    sh: Tracked,
}

/// What one frame's "atmos" pass imported: resolve reads `reads`, [`Atmosphere::finish`]
/// stores the final states.
pub(crate) struct AtmosFrame {
    slot: usize,
    /// trans, ms, sky, surf, ap_l, ap_t, sh.
    res: [Res; 7],
}

impl AtmosFrame {
    /// Tables resolve reads.
    pub fn reads(&self) -> impl Iterator<Item = Res> + '_ {
        [0, 1, 2, 4, 5, 6].into_iter().map(|i| self.res[i])
    }
}

/// Medium inputs of the shared tables: rebuilt when these change.
type MediumKey = (Body, [f32; 4]);

pub(crate) struct Atmosphere {
    pub settings: AtmosphereSettings,
    shared: Option<(Tracked, Tracked)>,
    built_for: Option<MediumKey>,
    /// The key the current frame's words were written for (set by `prepare`).
    pending: Option<MediumKey>,
    views: Vec<Option<ViewTables>>,
}

fn rw() -> Usage {
    Usage { shader_read: true, shader_write: true, ..Default::default() }
}

impl Atmosphere {
    pub fn new(settings: AtmosphereSettings) -> Self {
        Self { settings, shared: None, built_for: None, pending: None, views: Vec::new() }
    }

    /// Allocates view `slot`'s tables and fills `FrameData.atmos` (and the sun colour at the
    /// camera) for a camera at world `cam` with the sun towards `sun`. Leaves the block zero
    /// (atmosphere off in the shaders) when disabled.
    pub fn prepare(&mut self, dev: &dyn Device, slot: usize, cam: DVec3, sun: Vec3, lighting: &LightingSettings, fd: &mut [u32]) -> Result<(), ed_gpu::Error> {
        self.pending = None;
        if !self.settings.enabled {
            return Ok(());
        }
        if self.shared.is_none() {
            let t = Tracked::new(dev, "atmos transmittance", (TRANS_TEXELS * 16) as u64, rw())?;
            let m = Tracked::new(dev, "atmos multiscatter", (MS_TEXELS * 16) as u64, rw())?;
            self.shared = Some((t, m));
            self.built_for = None;
        }
        if self.views.len() <= slot {
            self.views.resize_with(slot + 1, || None);
        }
        if self.views[slot].is_none() {
            let b = |label, n: u64| Tracked::new(dev, label, n * 16, rw());
            self.views[slot] = Some(ViewTables {
                sky: b("atmos sky view", (SKY.0 * SKY.1) as u64)?,
                surf: b("atmos surface sky", (SURF.0 * SURF.1) as u64)?,
                ap_l: b("atmos froxel scatter", (AP_N * AP_N * AP_N) as u64)?,
                ap_t: b("atmos froxel transmittance", (AP_N * AP_N * AP_N) as u64)?,
                sh: b("atmos ambient sh", SH_FLOAT4S as u64)?,
            });
        }
        let s = &self.settings;
        let body = s.body;
        let ht = body.top_km - body.radius_km;
        let sun = sun.normalize_or(Vec3::Y);
        // Flat world: up is +Y and heights come from the plane. Curved body: from the pose.
        let (h_cam, h_surf, up) = match s.ground {
            None => (((cam.y - s.surface_y) * 1e-3 + s.surface_alt_km).max(0.0005), s.surface_alt_km.max(0.0) + 0.002, Vec3::Y),
            Some(g) => (
                (g.camera_alt_m * 1e-3).max(0.0005),
                g.ground_alt_m.map_or(0.0, |m| m * 1e-3).max(0.0) + 0.002,
                g.up.normalize_or(Vec3::Y),
            ),
        };
        let mu_s_cam = if s.ground.is_some() { up.dot(sun) } else { sun.y };
        let (n, d) = if h_cam > ht { s.ap_outside_km } else { s.ap_inside_km };
        let a = lighting.bounce_albedo;
        let sunlit = lighting.bounce_sunlit.clamp(0.0, 1.0);
        let (shared, view) = (self.shared.as_ref().expect("allocated"), self.views[slot].as_ref().expect("allocated"));
        let f = |v: [f64; 3], w: f64| [v[0] as f32, v[1] as f32, v[2] as f32, w as f32];
        let rows: [[f32; 4]; 9] = [
            [body.radius_km as f32, body.top_km as f32, h_cam as f32, h_surf as f32],
            f(body.rayleigh, body.rayleigh_h_km),
            f(body.mie_scattering, body.mie_h_km),
            f(body.mie_absorption, body.mie_g),
            f(body.ozone, body.ozone_peak_km),
            [a.x, a.y, a.z, body.ozone_half_km as f32],
            f(body.sun_irradiance, body.sun_radius.cos()),
            [up.x, up.y, up.z, mu_s_cam],
            [n, d, sunlit, 0.0],
        ];
        let w = &mut fd[FD_ATMOS..FD_ATMOS + ATMOS_WORDS];
        for (i, r) in rows.iter().enumerate() {
            for (j, x) in r.iter().enumerate() {
                w[i * 4 + j] = x.to_bits();
            }
        }
        w[36..40].copy_from_slice(&[shared.0.bindless.0, shared.1.bindless.0, view.sky.bindless.0, view.surf.bindless.0]);
        w[40..44].copy_from_slice(&[view.ap_l.bindless.0, view.ap_t.bindless.0, view.sh.bindless.0, 0]);
        w[44..48].copy_from_slice(&[1, s.debug, 0, 0]);
        // The sun at the camera, through the atmosphere (what the analytic model's warm sun
        // stood for); resolve evaluates it per shaded point.
        let sc = model::sun_at(&body, h_cam, mu_s_cam as f64);
        for i in 0..3 {
            fd[FD_SUN_DIR + 4 + i] = (sc[i] as f32).to_bits();
        }
        self.pending = Some((body, [a.x, a.y, a.z, sunlit]));
        Ok(())
    }

    /// Records the "atmos" pass for view `slot` (after [`Self::prepare`] of the same frame);
    /// `None` when the atmosphere is off. `fd` is the frame-data buffer and its bindless index.
    pub fn record_atmosphere(&mut self, g: &mut FrameGraph<'_>, slot: usize, fd: (Res, u32), pipes: &AtmosPipes) -> Option<AtmosFrame> {
        let key = self.pending?;
        let (shared, view) = (self.shared.as_ref()?, self.views.get(slot)?.as_ref()?);
        let rebuild = self.built_for != Some(key);
        self.built_for = Some(key);
        let imp = |g: &mut FrameGraph<'_>, name, t: &Tracked| g.import_buffer(name, t.id, t.state, None);
        let res = [
            imp(g, "atmos transmittance", &shared.0),
            imp(g, "atmos multiscatter", &shared.1),
            imp(g, "atmos sky view", &view.sky),
            imp(g, "atmos surface sky", &view.surf),
            imp(g, "atmos froxel scatter", &view.ap_l),
            imp(g, "atmos froxel transmittance", &view.ap_t),
            imp(g, "atmos ambient sh", &view.sh),
        ];
        let (sw, sr) = (ResourceState::ShaderWrite, ResourceState::ShaderRead);
        let mut p = g.add_pass("atmos", PassKind::Compute).read(fd.0, sr);
        p = if rebuild { p.write(res[0], sw).write(res[1], sw) } else { p.read(res[0], sr).read(res[1], sr) };
        for &r in &res[2..] {
            p = p.write(r, sw);
        }
        let ids = (shared.0.id, shared.1.id, view.surf.id, view.sh.id);
        let pp = (pipes.trans, pipes.ms, pipes.sky, pipes.ap, pipes.sh_partial, pipes.sh_final);
        let fd_bi = fd.1;
        p.exec(move |ctx| {
            let c = |mode: u32| constants(&[fd_bi, mode]);
            let barrier = |ctx: &mut crate::graph::PassCtx<'_>, b| ctx.enc.transition(&[Transition::Buffer { buffer: b, from: sw, to: sw }]);
            if rebuild {
                ctx.enc.set_compute_pipeline(pp.0);
                ctx.enc.set_compute_constants(&c(0));
                ctx.enc.dispatch((model::TRANS_W as u32).div_ceil(8), (model::TRANS_H as u32).div_ceil(8), 1);
                barrier(ctx, ids.0);
                ctx.enc.set_compute_pipeline(pp.1);
                ctx.enc.set_compute_constants(&c(0));
                ctx.enc.dispatch((model::MS_N as u32).div_ceil(8), (model::MS_N as u32).div_ceil(8), 1);
                barrier(ctx, ids.1);
            }
            ctx.enc.set_compute_pipeline(pp.2);
            ctx.enc.set_compute_constants(&c(0));
            ctx.enc.dispatch(SKY.0.div_ceil(8), SKY.1.div_ceil(8), 1);
            ctx.enc.set_compute_constants(&c(1));
            ctx.enc.dispatch(SURF.0.div_ceil(8), SURF.1.div_ceil(8), 1);
            ctx.enc.set_compute_pipeline(pp.3);
            ctx.enc.set_compute_constants(&c(0));
            ctx.enc.dispatch(AP_N.div_ceil(8), AP_N.div_ceil(8), 1);
            barrier(ctx, ids.2);
            ctx.enc.set_compute_pipeline(pp.4);
            ctx.enc.set_compute_constants(&c(0));
            ctx.enc.dispatch(1, 1, 1);
            barrier(ctx, ids.3);
            ctx.enc.set_compute_pipeline(pp.5);
            ctx.enc.set_compute_constants(&c(0));
            ctx.enc.dispatch(1, 1, 1);
        });
        Some(AtmosFrame { slot, res })
    }

    /// Stores the buffers' final states after the graph ran.
    pub fn finish(&mut self, plan: &Plan, f: AtmosFrame) {
        let st = |r: Res| plan.final_states[r.index()];
        if let Some((t, m)) = self.shared.as_mut() {
            t.state = st(f.res[0]);
            m.state = st(f.res[1]);
        }
        if let Some(Some(v)) = self.views.get_mut(f.slot) {
            v.sky.state = st(f.res[2]);
            v.surf.state = st(f.res[3]);
            v.ap_l.state = st(f.res[4]);
            v.ap_t.state = st(f.res[5]);
            v.sh.state = st(f.res[6]);
        }
    }

    pub fn destroy(self, dev: &dyn Device) {
        if let Some((t, m)) = self.shared {
            t.destroy(dev);
            m.destroy(dev);
        }
        for v in self.views.into_iter().flatten() {
            for t in [v.sky, v.surf, v.ap_l, v.ap_t, v.sh] {
                t.destroy(dev);
            }
        }
    }
}

/// Places the camera on the curved body for the next frame: call each frame with
/// `TerrainProvider::camera_ground(..)`. Atmosphere up, camera altitude and ground altitude then
/// follow the planet instead of the flat plane (output without it is unchanged).
pub fn apply_camera_ground(r: &mut crate::Renderer, g: CameraGround) {
    let mut a = *r.atmosphere();
    a.ground = Some(g);
    r.set_atmosphere(a);
}
