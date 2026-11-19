//! Volumetric clouds, the GPU side (Phase 2 item 5; design `docs/plans/clouds.md`). The field,
//! its lighting and the per-frame words are `ed_world::cloudscape`; this module owns the GPU
//! objects and records three passes after resolve (`shaders/clouds.slang`):
//!
//! 1. "clouds march" (compute, half res, checkerboard): a quarter of the half-res pixels per
//!    frame (1/16 of full res), jittered, with aerial perspective and a ground shadow term.
//! 2. "clouds temporal" (compute, half res): reprojects the history with the previous
//!    view-projection and fills the pixels not marched this frame.
//! 3. "clouds composite" (fullscreen raster, premultiplied blend over HDR): depth-aware
//!    upsample; the target becomes `hdr * T * shadow + L`.
//!
//! The noise textures (shape and detail volumes, weather map; Rgba16Float with mips) are
//! uploaded once from bytes the caller bakes ([`CloudNoise`], `ed_world::cloudscape::noise`),
//! then sit in ShaderRead like material textures. Per view: the `CloudData` rows (cpu_write)
//! and the half-res buffers (this frame's march, two history pairs ping-ponged).
//!
//! Off switch: `EW_CLOUDS=0`. Debug views: `EW_CLOUDS_DEBUG=transmittance|depth|shadow`
//! (packed by the caller into the words). Cost per tier: to be measured on the M3 Pro at the
//! 1440p internal resolution before any claim (CLAUDE.md rule 9); the knobs are the march's
//! max steps (words row 0) and the checkerboard (fixed at 1/4 per frame).

use crate::graph::{FrameGraph, PassKind, Plan, Res};
use crate::gpu::constants;
use crate::shaders::{Built, Module, ShaderSet};
use crate::{Tracked, HDR_FORMAT};
use ed_gpu::{
    Blend, BindlessTexture, ComputePipelineId, Cull, Device, Format, LoadOp, RenderPipelineDesc, RenderPipelineId, ResourceState, ShaderStage, StoreOp, Subresource, TextureDesc,
    TextureId, TextureKind, Topology, Usage,
};
use glam::{DVec3, Mat4};

/// `CloudData` words (`ed_world::cloudscape::gpu::CLOUD_WORDS`; a test pins them equal).
pub const CLOUD_WORDS: usize = 400;
/// First row the renderer fills: previous view-projection columns (4 rows), camera delta and
/// history-valid flag, half-res size.
const REPROJECT_ROW0: usize = 94;

/// Cloud switches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CloudSettings {
    /// `EW_CLOUDS=0` turns the passes off.
    pub enabled: bool,
}

impl Default for CloudSettings {
    fn default() -> Self {
        Self { enabled: true }
    }
}

impl CloudSettings {
    pub fn from_env() -> Self {
        Self { enabled: std::env::var("EW_CLOUDS").map_or(true, |v| v != "0") }
    }
}

/// One noise texture: size and every mip level's bytes (Rgba16Float texels, little-endian).
pub struct NoiseTexture {
    pub size: u32,
    pub mips: Vec<Vec<u8>>,
}

/// The cloud noise, as `ed_world::cloudscape::noise` bakes it (f16 bytes, all mips).
pub struct CloudNoise {
    /// 3D shape volume.
    pub shape: NoiseTexture,
    /// 3D detail volume.
    pub detail: NoiseTexture,
    /// 2D weather map.
    pub weather: NoiseTexture,
}

/// The cloud pipelines (rebuilt on hot reload with the rest).
pub(crate) struct CloudPipes {
    march: ComputePipelineId,
    temporal: ComputePipelineId,
    composite: RenderPipelineId,
}

impl CloudPipes {
    pub fn build(dev: &dyn Device, src: &ShaderSet, b: &mut Built) -> Result<Self, ed_gpu::Error> {
        let march = b.compute(dev, src, Module::Clouds, "clouds_march", [8, 8, 1])?;
        let temporal = b.compute(dev, src, Module::Clouds, "clouds_temporal", [8, 8, 1])?;
        let vs = b.shader(dev, src, Module::Clouds, "clouds vs", ShaderStage::Vertex, "vs_clouds")?;
        let fs = b.shader(dev, src, Module::Clouds, "clouds composite fs", ShaderStage::Fragment, "fs_clouds_composite")?;
        let composite = b.render(
            dev,
            &RenderPipelineDesc {
                label: "clouds composite",
                vertex: vs,
                fragment: Some(fs),
                color_formats: &[HDR_FORMAT],
                blend: Blend::Premultiplied,
                depth: None,
                cull: Cull::None,
                topology: Topology::Triangles,
                samples: 1,
            },
        )?;
        Ok(Self { march, temporal, composite })
    }
}

struct Tex {
    id: TextureId,
    bindless: BindlessTexture,
}

/// Per-view buffers.
struct ViewClouds {
    data: Tracked,
    cur: [Tracked; 2],
    hist: [[Tracked; 2]; 2],
    hw: u32,
    hh: u32,
    /// Which history pair holds last frame's result.
    parity: usize,
    /// Last frame's camera-relative view-projection and camera position.
    prev: Option<(Mat4, DVec3)>,
}

/// What one frame's cloud passes imported (final states stored by [`Clouds::finish`]).
pub(crate) struct CloudFrame {
    slot: usize,
    /// data, cur a, cur b, prev a, prev b, out a, out b.
    res: [Res; 7],
}

pub(crate) struct Clouds {
    pub settings: CloudSettings,
    noise: Option<[Tex; 3]>,
    /// This frame's words from the caller (rows 0..94); `None`: no clouds.
    words: Option<Vec<u32>>,
    views: Vec<Option<ViewClouds>>,
}

fn rw() -> Usage {
    Usage { shader_read: true, shader_write: true, ..Default::default() }
}

impl Clouds {
    pub fn new(settings: CloudSettings) -> Self {
        Self { settings, noise: None, words: None, views: Vec::new() }
    }

    /// Creates and uploads the noise textures; returns their ids for the renderer's
    /// CopyDst -> ShaderRead transition at the next frame start.
    pub fn set_noise(&mut self, dev: &dyn Device, n: &CloudNoise) -> Result<Vec<TextureId>, ed_gpu::Error> {
        if let Some(old) = self.noise.take() {
            for t in old {
                dev.unbind_texture(t.bindless);
                dev.destroy_texture(t.id);
            }
        }
        let make = |label: &str, kind: TextureKind, t: &NoiseTexture| -> Result<Tex, ed_gpu::Error> {
            let depth = if kind == TextureKind::D3 { t.size } else { 1 };
            let id = dev.create_texture(&TextureDesc {
                label,
                kind,
                format: Format::Rgba16Float,
                width: t.size,
                height: t.size,
                depth_or_layers: depth,
                mips: t.mips.len() as u32,
                samples: 1,
                usage: Usage { shader_read: true, copy_dst: true, ..Default::default() },
            })?;
            for (mip, bytes) in t.mips.iter().enumerate() {
                dev.write_texture(id, Subresource { mip: mip as u32, layer: 0 }, bytes)?;
            }
            Ok(Tex { id, bindless: dev.bind_texture(id)? })
        };
        let shape = make("cloud shape", TextureKind::D3, &n.shape)?;
        let detail = make("cloud detail", TextureKind::D3, &n.detail)?;
        let weather = make("cloud weather", TextureKind::D2, &n.weather)?;
        let ids = vec![shape.id, detail.id, weather.id];
        self.noise = Some([shape, detail, weather]);
        Ok(ids)
    }

    /// This frame's `CloudData` words (rows 0..94 from `ed_world::cloudscape::gpu::cloud_words`).
    pub fn set_words(&mut self, words: Option<Vec<u32>>) {
        self.words = words.filter(|w| w.len() == CLOUD_WORDS);
    }

    /// True when the passes will record this frame.
    pub fn active(&self) -> bool {
        self.settings.enabled && self.noise.is_some() && self.words.as_ref().is_some_and(|w| f32::from_bits(w[0]) != 0.0)
    }

    /// Fills view `slot`'s words for a `w` x `h` target and camera at `cam` with camera-relative
    /// view-projection `vpm`. Call once per frame per view, before recording.
    pub fn prepare(&mut self, dev: &dyn Device, slot: usize, w: u32, h: u32, cam: DVec3, vpm: Mat4) -> Result<(), ed_gpu::Error> {
        if !self.active() {
            return Ok(());
        }
        let (hw, hh) = (w.div_ceil(2), h.div_ceil(2));
        if self.views.len() <= slot {
            self.views.resize_with(slot + 1, || None);
        }
        if self.views[slot].as_ref().is_some_and(|v| (v.hw, v.hh) != (hw, hh)) {
            dev.wait_idle();
            if let Some(v) = self.views[slot].take() {
                v.destroy(dev);
            }
        }
        if self.views[slot].is_none() {
            let bytes = hw as u64 * hh as u64 * 16;
            let b = |l: &str| Tracked::new(dev, l, bytes, rw());
            self.views[slot] = Some(ViewClouds {
                data: Tracked::new(dev, "cloud data", (CLOUD_WORDS * 4) as u64, Usage { shader_read: true, cpu_write: true, ..Default::default() })?,
                cur: [b("cloud march a")?, b("cloud march b")?],
                hist: [[b("cloud history 0a")?, b("cloud history 0b")?], [b("cloud history 1a")?, b("cloud history 1b")?]],
                hw,
                hh,
                parity: 0,
                prev: None,
            });
        }
        let v = self.views[slot].as_mut().expect("allocated");
        let mut words = self.words.clone().expect("active");
        let put = |w: &mut [u32], row: usize, v: [f32; 4]| {
            for (i, x) in v.iter().enumerate() {
                w[row * 4 + i] = x.to_bits();
            }
        };
        // Reprojection: last frame's matrix and the camera's move since (a jump of more than
        // 2 km, a cut, drops the history).
        let (pvm, delta, valid) = match v.prev {
            Some((m, p)) if (cam - p).length() < 2_000.0 => (m, (cam - p).as_vec3(), 1.0),
            _ => (vpm, glam::Vec3::ZERO, 0.0),
        };
        for c in 0..4 {
            put(&mut words, REPROJECT_ROW0 + c, pvm.col(c).to_array());
        }
        put(&mut words, REPROJECT_ROW0 + 4, [delta.x, delta.y, delta.z, valid]);
        put(&mut words, REPROJECT_ROW0 + 5, [hw as f32, hh as f32, 0.0, 0.0]);
        v.prev = Some((vpm, cam));
        let bytes = crate::gpu::bytes(&words);
        dev.fill_frame_buffer(v.data.id, &mut |dst| dst[..bytes.len()].copy_from_slice(&bytes))
    }

    /// Records the three passes for view `slot` onto `hdr` (after resolve). `fd` is the frame
    /// data buffer and its bindless index, `depth` the scene depth, `smp` the renderer's
    /// linear repeat sampler.
    #[allow(clippy::too_many_arguments)]
    pub fn record(&mut self, g: &mut FrameGraph<'_>, slot: usize, fd: (Res, u32), depth: Res, hdr: Res, smp: u32, pipes: &CloudPipes) -> Option<CloudFrame> {
        if !self.active() {
            return None;
        }
        let noise = self.noise.as_ref()?;
        let v = self.views.get_mut(slot)?.as_mut()?;
        let (p, q) = (v.parity, 1 - v.parity);
        v.parity = q;
        let imp = |g: &mut FrameGraph<'_>, name, t: &Tracked| g.import_buffer(name, t.id, t.state, None);
        let res = [
            imp(g, "cloud data", &v.data),
            imp(g, "cloud march a", &v.cur[0]),
            imp(g, "cloud march b", &v.cur[1]),
            imp(g, "cloud history a", &v.hist[p][0]),
            imp(g, "cloud history b", &v.hist[p][1]),
            imp(g, "cloud history next a", &v.hist[q][0]),
            imp(g, "cloud history next b", &v.hist[q][1]),
        ];
        let (sr, sw) = (ResourceState::ShaderRead, ResourceState::ShaderWrite);
        let (hw, hh) = (v.hw, v.hh);
        let words = [
            fd.1,
            v.data.bindless.0,
            0, // depth: a transient, known in the pass
            smp,
            noise[0].bindless.0,
            noise[1].bindless.0,
            noise[2].bindless.0,
            v.cur[0].bindless.0,
            v.cur[1].bindless.0,
            v.hist[p][0].bindless.0,
            v.hist[p][1].bindless.0,
            v.hist[q][0].bindless.0,
            v.hist[q][1].bindless.0,
            hw,
            hh,
            0,
        ];
        let with_depth = move |ctx: &crate::graph::PassCtx<'_>| {
            let mut w = words;
            w[2] = ctx.bindless(depth).0;
            constants(&w)
        };
        let march = pipes.march;
        g.add_pass("clouds march", PassKind::Compute)
            .read(fd.0, sr)
            .read(res[0], sr)
            .read(depth, sr)
            .write(res[1], sw)
            .write(res[2], sw)
            .exec(move |ctx| {
                let c = with_depth(ctx);
                ctx.enc.set_compute_pipeline(march);
                ctx.enc.set_compute_constants(&c);
                ctx.enc.dispatch(hw.div_ceil(2).div_ceil(8), hh.div_ceil(2).div_ceil(8), 1);
            });
        let temporal = pipes.temporal;
        g.add_pass("clouds temporal", PassKind::Compute)
            .read(fd.0, sr)
            .read(res[0], sr)
            .read(res[1], sr)
            .read(res[2], sr)
            .read(res[3], sr)
            .read(res[4], sr)
            .write(res[5], sw)
            .write(res[6], sw)
            .exec(move |ctx| {
                let c = with_depth(ctx);
                ctx.enc.set_compute_pipeline(temporal);
                ctx.enc.set_compute_constants(&c);
                ctx.enc.dispatch(hw.div_ceil(8), hh.div_ceil(8), 1);
            });
        let composite = pipes.composite;
        g.add_pass("clouds composite", PassKind::Raster)
            .read(fd.0, sr)
            .read(res[0], sr)
            .read(depth, sr)
            .read(res[5], sr)
            .read(res[6], sr)
            .color(hdr, LoadOp::Load, StoreOp::Store)
            .exec(move |ctx| {
                let c = with_depth(ctx);
                ctx.enc.set_render_pipeline(composite);
                ctx.enc.set_draw_constants(&c);
                ctx.enc.draw(0..3, 0..1);
            });
        Some(CloudFrame { slot, res })
    }

    /// Stores the imported buffers' final states after the graph ran.
    pub fn finish(&mut self, plan: &Plan, f: CloudFrame) {
        let Some(Some(v)) = self.views.get_mut(f.slot) else { return };
        let st = |r: Res| plan.final_states[r.index()];
        // `parity` already flipped in `record`: the pair written this frame is `parity`.
        let (q, p) = (v.parity, 1 - v.parity);
        v.data.state = st(f.res[0]);
        v.cur[0].state = st(f.res[1]);
        v.cur[1].state = st(f.res[2]);
        v.hist[p][0].state = st(f.res[3]);
        v.hist[p][1].state = st(f.res[4]);
        v.hist[q][0].state = st(f.res[5]);
        v.hist[q][1].state = st(f.res[6]);
    }

    pub fn destroy(self, dev: &dyn Device) {
        for t in self.noise.into_iter().flatten() {
            dev.unbind_texture(t.bindless);
            dev.destroy_texture(t.id);
        }
        for v in self.views.into_iter().flatten() {
            v.destroy(dev);
        }
    }
}

impl ViewClouds {
    fn destroy(self, dev: &dyn Device) {
        self.data.destroy(dev);
        for t in self.cur.into_iter().chain(self.hist.into_iter().flatten()) {
            t.destroy(dev);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The renderer's view of the `CloudData` layout is ed_world's.
    #[test]
    fn layout_matches_ed_world() {
        use ed_world::cloudscape::gpu as g;
        assert_eq!(CLOUD_WORDS, g::CLOUD_WORDS);
        assert_eq!(REPROJECT_ROW0, g::REPROJECT_ROW0);
        assert_eq!(REPROJECT_ROW0 + 6, g::CLOUD_ROWS);
    }

    #[test]
    fn off_without_noise_or_words() {
        let mut c = Clouds::new(CloudSettings::default());
        assert!(!c.active());
        c.set_words(Some(vec![0; 3]));
        assert!(c.words.is_none(), "wrong-length words are refused");
    }
}
