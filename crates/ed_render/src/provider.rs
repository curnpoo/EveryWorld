//! Geometry providers (renderer.md §3) and provider 1, discrete LOD.
//!
//! A provider turns the shared inputs (instance pools, meshes, the view) into depth plus the
//! visibility target. [`DiscreteLod`] does it with per-instance GPU culling:
//!
//! - `cull_early`: reset the draw arguments, select each instance's LOD by projected error
//!   (with hysteresis), advance its cross-fade, and emit the instances that pass the frustum
//!   and were visible last frame.
//! - `raster_early`: draw them (visibility buffer + depth).
//! - `hzb`: build the depth pyramid from that depth.
//! - `cull_late`: frustum + occlusion against the pyramid for every instance; this is next
//!   frame's "visible last frame". Emit the ones that pass and were not drawn early.
//! - `raster_late`: draw them on top.
//!
//! Draws are bins of `DrawIndexedArgs`, one per mesh and LOD, for each phase and for solid and
//! dithered (cross-fading) instances, issued as two multi-draws per phase. Unused bins have
//! zero instances (Metal ignores the count buffer, `Caps::indirect_count == false`).

use crate::gpu::{self, ARGS_WORDS, FD_CULL, FD_HZB, MAX_HZB_LEVELS, MESH_WORDS, STAT_WORDS, constants};
use crate::graph::{FrameGraph, PassBuilder, PassKind, Plan, Res};
use crate::scene::{InstancePools, MAX_LODS};
use crate::shaders::{Built, Module, ShaderSet};
use crate::{Tracked, VIS_FORMAT};
use ed_gpu::{
    BufferId, Compare, ComputePipelineId, Cull, DepthDesc, Device, LoadOp, RenderPipelineDesc,
    RenderPipelineId, ResourceState, ShaderStage, StoreOp, Topology, Transition, Usage,
};
use glam::Vec4;
use std::cell::Cell;
use std::rc::Rc;


/// Threads per culling workgroup.
const CULL_GROUP: u32 = 64;
/// Bin capacity granularity (visible-list slots).
const BIN_ROUND: u32 = 64;
/// Readback ring length (more than the most frames in flight).
pub(crate) const RING: usize = 4;

/// Provider-independent per-frame inputs, already imported into the graph.
#[derive(Clone, Copy, Debug)]
pub struct SharedInputs {
    pub frame: (Res, u32),
    pub instances: (Res, u32),
    pub nodes: (Res, u32),
    pub verts: (Res, u32),
    pub indices: (Res, u32, BufferId),
    /// Instance slots to process (high-water mark of the pools).
    pub slots: u32,
}

/// The targets a provider writes: reverse-Z depth and the visibility buffer
/// (`Rg32Uint`: instance + 1, LOD << 28 | primitive).
#[derive(Clone, Copy, Debug)]
pub struct VisTargets {
    pub depth: Res,
    pub vis: Res,
    /// Sun shadow casters to draw after the view (M4d), or none.
    pub shadow: Option<ShadowTargets>,
}

/// The cascade atlas a provider draws sun shadow casters into: `cascades` square tiles of
/// `tile` texels, two per row (common.slang `LightData`; the cascade matrices are in FrameData).
#[derive(Clone, Copy, Debug)]
pub struct ShadowTargets {
    pub atlas: Res,
    pub cascades: u32,
    pub tile: u32,
}

/// What a provider hands to the passes after it (resolve, debug passes).
#[derive(Clone, Copy, Debug)]
pub struct ProviderOutputs {
    /// The mesh table (`GpuMesh`), for the resolve.
    pub meshes: (Res, u32),
    /// Per-instance state words (LOD, fade), for the pop detector.
    pub state: (Res, u32),
}

/// A source of geometry for the visibility buffer (renderer.md §3). Provider 1 is
/// [`DiscreteLod`]; clusters (M3) and terrain follow the same shape.
pub trait GeometryProvider {
    fn name(&self) -> &'static str;
    /// Before the graph: uploads, readbacks of earlier frames, and the provider's part of
    /// `FrameData` (`fd`, the words of the per-view block).
    /// Outside a frame (before `begin_frame`): upload the provider's tables for the current
    /// pools, so they land together with the instance data.
    fn flush(&mut self, dev: &dyn Device, pools: &InstancePools) -> Result<(), ed_gpu::Error>;
    /// Read back finished frames' results.
    fn poll(&mut self, dev: &dyn Device);
    /// Inside a frame, before the graph: per-size resources and the provider's part of
    /// `FrameData` (`fd`, the words of the per-view block).
    fn prepare(&mut self, dev: &dyn Device, pools: &InstancePools, width: u32, height: u32, fd: &mut [u32]) -> Result<(), ed_gpu::Error>;
    /// Declare the passes that fill `out`.
    fn record<'a>(&mut self, g: &mut FrameGraph<'a>, frame_index: u64, inputs: &SharedInputs, out: VisTargets) -> ProviderOutputs;
    /// After the graph ran: final resource states.
    fn finish(&mut self, plan: &Plan);
    /// Selects the view whose per-view state the next `prepare`/`record`/`finish` use
    /// (`ViewFlags::slot`), creating it on first use (outside a frame: its initial upload
    /// lands at the next frame start).
    fn set_view(&mut self, dev: &dyn Device, slot: usize) -> Result<(), ed_gpu::Error>;
    /// Timer labels of the provider's passes, in run order.
    fn pass_names(&self) -> &'static [&'static str];
    fn destroy(self: Box<Self>, dev: &dyn Device);
}

/// Most per-frame counter records [`DiscreteLod::take_history`] keeps.
pub const HISTORY: usize = 1024;

/// Provider 1 settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LodConfig {
    /// Largest allowed projected geometric error, pixels.
    pub lod_px: f32,
    /// Switching to a coarser level needs the error under `lod_px * hysteresis`.
    pub hysteresis: f32,
    /// Cross-fade length in frames (0: switch at once). Max 63.
    pub fade_frames: u32,
    /// Depth-pyramid occlusion test in `cull_late`.
    pub occlusion: bool,
}

impl Default for LodConfig {
    fn default() -> Self {
        Self { lod_px: 1.0, hysteresis: 0.75, fade_frames: 16, occlusion: true }
    }
}

/// GPU culling counters of one finished frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CullStats {
    pub frame: u64,
    /// Live instances.
    pub total: u32,
    pub after_frustum: u32,
    pub after_occlusion: u32,
    pub drawn_early: u32,
    pub drawn_late: u32,
    /// Visible-list entries (a cross-fading instance has two).
    pub entries: u32,
    pub nonempty_bins: u32,
    /// Drawn instances in a LOD cross-fade.
    pub fading: u32,
    pub triangles: u32,
    /// Drawn instances whose LOD changed this frame.
    pub changed: u32,
    /// Sun shadow casters per cascade (M4d; 0 with shadows off).
    pub shadow_casters: [u32; 4],
    /// Triangles drawn into the shadow atlas.
    pub shadow_triangles: u32,
}

impl CullStats {
    pub fn drawn(&self) -> u32 {
        self.drawn_early + self.drawn_late
    }
    fn from_words(frame: u64, w: &[u32]) -> Self {
        Self {
            frame,
            total: w[0],
            after_frustum: w[1],
            after_occlusion: w[2],
            drawn_early: w[3],
            drawn_late: w[4],
            entries: w[5],
            nonempty_bins: w[6],
            fading: w[7],
            triangles: w[8],
            changed: w[9],
            shadow_casters: [w[10], w[11], w[12], w[13]],
            shadow_triangles: w[14],
        }
    }
}

/// Pixels per metre at the near side of a bounding sphere (the LOD metric; mirrors cull.slang).
pub fn pixels_per_metre(scale: f32, target_height: f32, p11: f32, centre_distance: f32, radius: f32, near: f32) -> f32 {
    scale * target_height * 0.5 * p11 / (centre_distance - radius).max(near)
}

/// LOD selection (mirrors cull.slang): the coarsest level whose error projects under
/// `threshold` pixels; a switch to a coarser level than `cur` needs `threshold * hysteresis`.
/// `init` is false the first time an instance is seen (no hysteresis).
pub fn select_lod(errors: &[f32], px_per_m: f32, cur: u32, init: bool, threshold: f32, hysteresis: f32) -> u32 {
    let n = errors.len().clamp(1, MAX_LODS) as u32;
    let mut want = 0;
    for l in 1..n {
        if errors[l as usize] * px_per_m <= threshold {
            want = l;
        }
    }
    if init && want > cur {
        let mut w = cur;
        for l in cur + 1..n {
            if errors[l as usize] * px_per_m <= threshold * hysteresis {
                w = l;
            }
        }
        return w;
    }
    want
}

/// Depth-pyramid levels for a `w` x `h` depth target: (offset in floats, width, height).
pub fn hzb_levels(w: u32, h: u32) -> Vec<(u32, u32, u32)> {
    let mut out = Vec::new();
    let (mut lw, mut lh, mut off) = ((w / 2).max(1), (h / 2).max(1), 0);
    loop {
        out.push((off, lw, lh));
        off += lw * lh;
        if (lw == 1 && lh == 1) || out.len() == MAX_HZB_LEVELS {
            break;
        }
        lw = (lw / 2).max(1);
        lh = (lh / 2).max(1);
    }
    out
}

/// A mesh as provider 1 sees it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MeshRecord {
    pub sphere: Vec4,
    pub lod_count: u32,
    pub first_index: [u32; MAX_LODS],
    pub index_count: [u32; MAX_LODS],
    pub base_vertex: i32,
    pub error: [f32; MAX_LODS],
}

/// A ring of `cpu_read` buffers written by the GPU, one per frame in flight.
pub(crate) struct Ring {
    pub bufs: Vec<Tracked>,
    pending: [Option<u64>; RING],
    pub lost: u64,
}

impl Ring {
    pub fn new(dev: &dyn Device, label: &str, size: u64) -> Result<Self, ed_gpu::Error> {
        let mut bufs = Vec::with_capacity(RING);
        for _ in 0..RING {
            bufs.push(Tracked::new(dev, label, size, Usage { cpu_read: true, shader_read: true, shader_write: true, ..Default::default() })?);
        }
        Ok(Self { bufs, pending: [None; RING], lost: 0 })
    }
    pub fn slot(&mut self, frame: u64) -> usize {
        let s = (frame % RING as u64) as usize;
        if self.pending[s].is_some() {
            self.lost += 1; // never read back (should not happen with RING > frames in flight)
        }
        self.pending[s] = Some(frame);
        s
    }
    /// Calls `f(frame, bytes)` for every finished frame not read yet, oldest first.
    pub fn poll(&mut self, dev: &dyn Device, scratch: &mut Vec<u8>, mut f: impl FnMut(u64, &[u8])) {
        let done = dev.completed_frames();
        let mut ready: Vec<(u64, usize)> = (0..RING).filter_map(|s| self.pending[s].filter(|&fr| fr < done).map(|fr| (fr, s))).collect();
        ready.sort_unstable();
        for (fr, s) in ready {
            self.pending[s] = None;
            if dev.read_buffer(self.bufs[s].id, scratch).is_ok() {
                f(fr, scratch);
            }
        }
    }
    pub fn destroy(self, dev: &dyn Device) {
        for b in self.bufs {
            b.destroy(dev);
        }
    }
}

/// Provider 1: discrete LOD meshes with per-instance GPU culling, two-phase occlusion and
/// dithered LOD cross-fades.
pub struct DiscreteLod {
    pub config: LodConfig,
    p: LodPipelines,
    meshes: Vec<MeshRecord>,
    capacity: Vec<u32>,
    layout_dirty: bool,
    bins: u32,
    mesh_table: Tracked,
    templ: Tracked,
    max_instances: u32,
    /// Visible-list bytes the current layout needs (a new view starts at this size).
    visible_bytes: u64,
    /// Per-view culling state (P1-D24), index = `ViewFlags::slot`. Nothing in here is shared
    /// between views: LOD selection, fades, the occlusion pyramid, "visible last frame" and
    /// the counters of one view never leak into another's.
    views: Vec<CullView>,
    cur: usize,
    scratch: Vec<u8>,
}

/// One view's culling state.
struct CullView {
    state: Tracked,
    args: Tracked,
    visible: Tracked,
    /// Sun shadow casters: per-cascade bins (same layout and template as the main view) and
    /// their visible list.
    shadow_args: Tracked,
    shadow_visible: Tracked,
    hzb: Option<(Tracked, u32, u32)>,
    stats_ring: Ring,
    stats: Option<CullStats>,
    history: std::collections::VecDeque<CullStats>,
    draw_loop: Rc<Cell<f64>>,
    last_draw_loop_ms: f64,
    imports: Vec<(Res, Imported)>,
    hzb_levels: Vec<(u32, u32, u32)>,
}

impl CullView {
    fn new(dev: &dyn Device, max_instances: u32, bins_max: u64, visible_bytes: u64) -> Result<Self, ed_gpu::Error> {
        let rw = Usage { shader_read: true, shader_write: true, ..Default::default() };
        let mut state = Tracked::new(dev, "instance state", max_instances as u64 * 4, Usage { copy_dst: true, ..rw })?;
        dev.write_buffer(state.id, 0, &vec![0u8; max_instances as usize * 4])?;
        state.state = ResourceState::CopyDst;
        let args = Tracked::new(dev, "draw args", bins_max * ARGS_WORDS as u64 * 4, Usage { indirect_args: true, ..rw })?;
        let visible = Tracked::new(dev, "visible list", visible_bytes, rw)?;
        let shadow_args = Tracked::new(dev, "shadow draw args", bins_max * ARGS_WORDS as u64 * 4, Usage { indirect_args: true, ..rw })?;
        let shadow_visible = Tracked::new(dev, "shadow visible list", visible_bytes, rw)?;
        let stats_ring = Ring::new(dev, "cull stats", (STAT_WORDS * 4) as u64)?;
        Ok(Self {
            state,
            args,
            visible,
            shadow_args,
            shadow_visible,
            hzb: None,
            stats_ring,
            stats: None,
            history: std::collections::VecDeque::new(),
            draw_loop: Rc::new(Cell::new(0.0)),
            last_draw_loop_ms: 0.0,
            imports: Vec::new(),
            hzb_levels: Vec::new(),
        })
    }
    fn destroy(self, dev: &dyn Device) {
        for t in [self.state, self.args, self.visible, self.shadow_args, self.shadow_visible] {
            t.destroy(dev);
        }
        if let Some((h, _, _)) = self.hzb {
            h.destroy(dev);
        }
        self.stats_ring.destroy(dev);
    }
    fn poll(&mut self, dev: &dyn Device, scratch: &mut Vec<u8>) {
        let mut latest = None;
        let hist = &mut self.history;
        self.stats_ring.poll(dev, scratch, |fr, b| {
            let s = CullStats::from_words(fr, &gpu::words(b));
            if hist.len() == HISTORY {
                hist.pop_front();
            }
            hist.push_back(s);
            latest = Some(s);
        });
        if latest.is_some() {
            self.stats = latest;
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Imported {
    MeshTable,
    State,
    Args,
    Templ,
    Visible,
    ShadowArgs,
    ShadowVisible,
    Hzb,
    Stats(usize),
}

/// The accesses both culling passes declare. `bufs`: mesh table, state, args, visible, stats.
fn cull_pass<'g, 'a>(g: &'g mut FrameGraph<'a>, name: &'static str, inp: &SharedInputs, bufs: [Res; 5]) -> PassBuilder<'g, 'a> {
    let [meshes, state, args, visible, stats] = bufs;
    let sw = ResourceState::ShaderWrite;
    g.add_pass(name, PassKind::Compute)
        .read(inp.frame.0, ResourceState::ShaderRead)
        .read(inp.instances.0, ResourceState::ShaderRead)
        .read(inp.nodes.0, ResourceState::ShaderRead)
        .read(meshes, ResourceState::ShaderRead)
        .write(state, sw)
        .write(args, sw)
        .write(visible, sw)
        .write(stats, sw)
}

/// Provider 1's pipelines, rebuilt as a set on shader hot reload.
pub(crate) struct LodPipelines {
    built: Built,
    cull_reset: ComputePipelineId,
    cull_early: ComputePipelineId,
    cull_late: ComputePipelineId,
    hzb_first: ComputePipelineId,
    hzb_down: ComputePipelineId,
    vis_solid: RenderPipelineId,
    vis_dither: RenderPipelineId,
    shadow_reset: ComputePipelineId,
    shadow_cull: ComputePipelineId,
    shadow_raster: RenderPipelineId,
}

impl LodPipelines {
    /// Compiles every pipeline; on any error nothing created so far is left behind.
    pub(crate) fn new(dev: &dyn Device, src: &ShaderSet) -> Result<Self, ed_gpu::Error> {
        let mut b = Built::default();
        match Self::build(dev, src, &mut b) {
            Ok(p) => Ok(p),
            Err(e) => {
                b.destroy(dev);
                Err(e)
            }
        }
    }
    fn build(dev: &dyn Device, src: &ShaderSet, b: &mut Built) -> Result<Self, ed_gpu::Error> {
        let g = [CULL_GROUP, 1, 1];
        let cull_reset = b.compute(dev, src, Module::Cull, "cull_reset", g)?;
        let cull_early = b.compute(dev, src, Module::Cull, "cull_early", g)?;
        let cull_late = b.compute(dev, src, Module::Cull, "cull_late", g)?;
        let hzb_first = b.compute(dev, src, Module::Hzb, "hzb_first", [8, 8, 1])?;
        let hzb_down = b.compute(dev, src, Module::Hzb, "hzb_down", [8, 8, 1])?;
        let vs = b.shader(dev, src, Module::Vis, "vs_vis", ShaderStage::Vertex, "vs_vis")?;
        let fs = b.shader(dev, src, Module::Vis, "fs_vis", ShaderStage::Fragment, "fs_vis")?;
        let fsd = b.shader(dev, src, Module::Vis, "fs_vis_dither", ShaderStage::Fragment, "fs_vis_dither")?;
        let rp = |label, fragment| RenderPipelineDesc {
            label,
            vertex: vs,
            fragment: Some(fragment),
            color_formats: &[VIS_FORMAT],
            blend: ed_gpu::Blend::Opaque,
            depth: Some(DepthDesc { format: crate::DEPTH_FORMAT, write: true, compare: Compare::GreaterEqual, bias: 0.0, slope_bias: 0.0, clamp: false }),
            cull: Cull::Back,
            topology: Topology::Triangles,
            samples: 1,
        };
        let vis_solid = b.render(dev, &rp("vis solid", fs))?;
        let vis_dither = b.render(dev, &rp("vis dither", fsd))?;
        let shadow_reset = b.compute(dev, src, Module::Cull, "shadow_reset", g)?;
        let shadow_cull = b.compute(dev, src, Module::Cull, "shadow_cull", g)?;
        let vss = b.shader(dev, src, Module::Vis, "vs_shadow", ShaderStage::Vertex, "vs_shadow")?;
        // Depth only; both faces (thin and open geometry casts), light depth grows towards the sun.
        let shadow_raster = b.render(
            dev,
            &RenderPipelineDesc {
                label: "shadow raster",
                vertex: vss,
                fragment: None,
                color_formats: &[],
                blend: ed_gpu::Blend::Opaque,
                depth: Some(DepthDesc { format: crate::DEPTH_FORMAT, write: true, compare: Compare::GreaterEqual, bias: 0.0, slope_bias: 0.0, clamp: true }),
                cull: Cull::None,
                topology: Topology::Triangles,
                samples: 1,
            },
        )?;
        Ok(Self { built: std::mem::take(b), cull_reset, cull_early, cull_late, hzb_first, hzb_down, vis_solid, vis_dither, shadow_reset, shadow_cull, shadow_raster })
    }
    pub(crate) fn destroy(self, dev: &dyn Device) {
        self.built.destroy(dev);
    }
}

impl DiscreteLod {
    pub(crate) fn new(dev: &dyn Device, src: &ShaderSet, max_instances: u32, max_meshes: u32, config: LodConfig) -> Result<Self, ed_gpu::Error> {
        let p = LodPipelines::new(dev, src)?;
        let mesh_table = Tracked::new(dev, "mesh table", (max_meshes as usize * MESH_WORDS * 4) as u64, Usage { shader_read: true, copy_dst: true, ..Default::default() })?;
        let bins_max = max_meshes as u64 * MAX_LODS as u64 * 4;
        let templ = Tracked::new(dev, "draw args template", bins_max * ARGS_WORDS as u64 * 4, Usage { shader_read: true, copy_dst: true, ..Default::default() })?;
        // Every live instance can sit in one bin per kind and phase: 4 slots, plus rounding.
        let visible_bytes = (max_instances as u64 + bins_max / 4 * BIN_ROUND as u64) * 4 * 4;
        let view0 = CullView::new(dev, max_instances, bins_max, visible_bytes)?;
        Ok(Self {
            config,
            p,
            meshes: Vec::new(),
            capacity: Vec::new(),
            layout_dirty: true,
            bins: 0,
            mesh_table,
            templ,
            max_instances,
            visible_bytes,
            views: vec![view0],
            cur: 0,
            scratch: Vec::new(),
        })
    }

    fn bins_max(&self) -> u64 {
        self.mesh_table.size / (MESH_WORDS as u64 * 4) * MAX_LODS as u64 * 4
    }
    /// Views that have culling state.
    pub fn view_count(&self) -> usize {
        self.views.len()
    }
    /// Counters of view `slot`'s latest finished frame.
    pub fn stats_for(&self, slot: usize) -> Option<CullStats> {
        self.views.get(slot).and_then(|v| v.stats)
    }

    pub(crate) fn add_mesh(&mut self, m: MeshRecord) {
        self.meshes.push(m);
        self.capacity.push(0);
        self.layout_dirty = true;
    }
    /// Replaces a mesh slot's record (mesh replace, removal with an empty record, slot reuse).
    pub(crate) fn set_mesh(&mut self, slot: usize, m: MeshRecord) {
        self.meshes[slot] = m;
        self.layout_dirty = true;
    }
    pub(crate) fn mesh_count(&self) -> usize {
        self.meshes.len()
    }
    /// Swaps in a new pipeline set and returns the old one (destroy it once idle).
    pub(crate) fn swap_pipelines(&mut self, p: LodPipelines) -> LodPipelines {
        std::mem::replace(&mut self.p, p)
    }

    /// Latest finished frame's counters.
    pub fn stats(&self) -> Option<CullStats> {
        self.views[0].stats
    }
    /// Every finished frame's counters since the last call, oldest first (the newest
    /// [`HISTORY`] are kept when nobody takes them).
    pub fn take_history(&mut self) -> Vec<CullStats> {
        self.views[0].history.drain(..).collect()
    }
    /// Bins (mesh x LOD) per kind and phase; a frame issues `4 * bins()` indirect draws.
    pub fn bins(&self) -> u32 {
        self.bins
    }
    /// Indirect draws recorded per frame.
    pub fn draw_calls(&self) -> u32 {
        self.bins * 4
    }
    /// CPU time of the last frame's multi-draw loops (both raster passes), ms.
    pub fn draw_loop_ms(&self) -> f64 {
        self.views.iter().map(|v| v.last_draw_loop_ms).sum()
    }
    /// Readbacks overwritten before they were read.
    pub fn lost_readbacks(&self) -> u64 {
        self.views.iter().map(|v| v.stats_ring.lost).sum()
    }

    fn relayout(&mut self, dev: &dyn Device, pools: &InstancePools) -> Result<(), ed_gpu::Error> {
        let need = self.layout_dirty || (0..self.meshes.len()).any(|m| pools.mesh_alive(m) > self.capacity[m]);
        if !need {
            return Ok(());
        }
        self.layout_dirty = false;
        let mut base = 0u32;
        let mut bases = Vec::with_capacity(self.meshes.len());
        for (m, rec) in self.meshes.iter().enumerate() {
            let alive = pools.mesh_alive(m);
            self.capacity[m] = (alive + alive / 4).div_ceil(BIN_ROUND).max(1) * BIN_ROUND;
            bases.push(base);
            base += rec.lod_count;
        }
        self.bins = base;
        let s = base as usize;
        let mut templ = vec![0u32; s * 4 * ARGS_WORDS];
        let mut offset = 0u32;
        for range in 0..4 {
            for (m, rec) in self.meshes.iter().enumerate() {
                for l in 0..rec.lod_count as usize {
                    let e = (range * s + bases[m] as usize + l) * ARGS_WORDS;
                    templ[e..e + ARGS_WORDS].copy_from_slice(&[rec.index_count[l], 0, rec.first_index[l], rec.base_vertex as u32, offset]);
                    offset += self.capacity[m];
                }
            }
        }
        // Each (mesh, LOD, kind, phase) bin reserves the mesh's capacity: grow the visible
        // list when the layout needs more (rare: scene load or a big jump in instance count).
        // The old buffer may still be read by frames in flight, so wait for them first.
        if offset as u64 > self.visible_bytes / 4 {
            self.visible_bytes = (offset as u64).next_multiple_of(1 << 16) * 4;
        }
        if self.views.iter().any(|v| v.visible.size < self.visible_bytes) {
            dev.wait_idle();
            let rw = Usage { shader_read: true, shader_write: true, ..Default::default() };
            for v in &mut self.views {
                if v.visible.size < self.visible_bytes {
                    let grown = Tracked::new(dev, "visible list", self.visible_bytes, rw)?;
                    std::mem::replace(&mut v.visible, grown).destroy(dev);
                    let grown = Tracked::new(dev, "shadow visible list", self.visible_bytes, rw)?;
                    std::mem::replace(&mut v.shadow_visible, grown).destroy(dev);
                }
            }
        }
        let mut table = vec![0u32; self.meshes.len() * MESH_WORDS];
        for (m, rec) in self.meshes.iter().enumerate() {
            let w = &mut table[m * MESH_WORDS..(m + 1) * MESH_WORDS];
            w[..4].copy_from_slice(&rec.sphere.to_array().map(f32::to_bits));
            w[4] = rec.lod_count;
            w[5] = bases[m];
            w[6] = self.capacity[m];
            for l in 0..MAX_LODS {
                w[8 + l] = rec.first_index[l];
                w[12 + l] = rec.index_count[l];
                w[16 + l] = rec.base_vertex as u32;
                w[20 + l] = rec.error[l].to_bits();
            }
        }
        dev.write_buffer(self.mesh_table.id, 0, &gpu::bytes(&table))?;
        dev.write_buffer(self.templ.id, 0, &gpu::bytes(&templ))?;
        Ok(())
    }
}

impl GeometryProvider for DiscreteLod {
    fn name(&self) -> &'static str {
        "discrete-lod"
    }

    fn flush(&mut self, dev: &dyn Device, pools: &InstancePools) -> Result<(), ed_gpu::Error> {
        self.relayout(dev, pools)
    }

    fn poll(&mut self, dev: &dyn Device) {
        for v in &mut self.views {
            v.poll(dev, &mut self.scratch);
        }
    }

    fn set_view(&mut self, dev: &dyn Device, slot: usize) -> Result<(), ed_gpu::Error> {
        while self.views.len() <= slot {
            let v = CullView::new(dev, self.max_instances, self.bins_max(), self.visible_bytes)?;
            self.views.push(v);
        }
        self.cur = slot;
        Ok(())
    }

    fn prepare(&mut self, dev: &dyn Device, _pools: &InstancePools, width: u32, height: u32, fd: &mut [u32]) -> Result<(), ed_gpu::Error> {
        let v = &mut self.views[self.cur];
        v.last_draw_loop_ms = v.draw_loop.replace(0.0);
        let levels = hzb_levels(width, height);
        let floats: u32 = levels.iter().map(|l| l.1 * l.2).sum();
        if v.hzb.as_ref().map(|h| (h.1, h.2)) != Some((width, height)) {
            if let Some((h, _, _)) = v.hzb.take() {
                h.destroy(dev);
            }
            let t = Tracked::new(dev, "hzb", floats as u64 * 4, Usage { shader_read: true, shader_write: true, ..Default::default() })?;
            v.hzb = Some((t, width, height));
        }
        let c = &self.config;
        fd[FD_CULL] = if c.occlusion { levels.len() as u32 } else { 0 };
        fd[FD_CULL + 1] = c.fade_frames.min(63);
        for (i, l) in levels.iter().enumerate() {
            fd[FD_HZB + i * 4..FD_HZB + i * 4 + 4].copy_from_slice(&[l.0, l.1, l.2, 0]);
        }
        v.hzb_levels = levels;
        Ok(())
    }

    fn record<'a>(&mut self, g: &mut FrameGraph<'a>, frame_index: u64, inp: &SharedInputs, out: VisTargets) -> ProviderOutputs {
        let (mesh_table, templ_t, bins, config, pipes) = (&self.mesh_table, &self.templ, self.bins, self.config, &self.p);
        let v = &mut self.views[self.cur];
        v.imports.clear();
        let fin = |s| Some(s);
        let imp = |g: &mut FrameGraph<'_>, name, t: &Tracked, f: Option<ResourceState>, which: Imported, list: &mut Vec<(Res, Imported)>| {
            let r = g.import_buffer(name, t.id, t.state, f);
            list.push((r, which));
            r
        };
        let mut list = Vec::new();
        // Final states that differ from the first use force a transition (a barrier) at the
        // start of the next frame, so the GPU never overlaps two frames' culling.
        let meshes = imp(g, "mesh table", mesh_table, fin(ResourceState::CopyDst), Imported::MeshTable, &mut list);
        let templ = imp(g, "draw args template", templ_t, fin(ResourceState::CopyDst), Imported::Templ, &mut list);
        let state = imp(g, "instance state", &v.state, fin(ResourceState::ShaderRead), Imported::State, &mut list);
        let args = imp(g, "draw args", &v.args, fin(ResourceState::IndirectArgs), Imported::Args, &mut list);
        let visible = imp(g, "visible list", &v.visible, fin(ResourceState::ShaderRead), Imported::Visible, &mut list);
        let shadow_bufs = out.shadow.map(|_| {
            let a = imp(g, "shadow draw args", &v.shadow_args, fin(ResourceState::IndirectArgs), Imported::ShadowArgs, &mut list);
            let vl = imp(g, "shadow visible list", &v.shadow_visible, fin(ResourceState::ShaderRead), Imported::ShadowVisible, &mut list);
            (a, vl)
        });
        let slot = v.stats_ring.slot(frame_index);
        let stats = imp(g, "cull stats", &v.stats_ring.bufs[slot], fin(ResourceState::ShaderRead), Imported::Stats(slot), &mut list);
        let occlusion = config.occlusion;
        let (hzb_t, dw, dh) = v.hzb.as_ref().expect("prepare before record");
        let hzb = imp(g, "hzb", hzb_t, fin(ResourceState::ShaderRead), Imported::Hzb, &mut list);
        let (hzb_id, hzb_bi, dw, dh) = (hzb_t.id, hzb_t.bindless.0, *dw, *dh);
        v.imports = list;
        let draw_loop = v.draw_loop.clone();
        let hzb_levels = v.hzb_levels.clone();

        let s = bins;
        let cw = [
            inp.frame.1,
            inp.instances.1,
            mesh_table.bindless.0,
            v.state.bindless.0,
            v.args.bindless.0,
            v.visible.bindless.0,
            v.stats_ring.bufs[slot].bindless.0,
            hzb_bi,
            inp.slots,
            s,
            templ_t.bindless.0,
            inp.nodes.1,
        ];
        let cc = constants(&cw);
        let (args_id, stats_id) = (v.args.id, v.stats_ring.bufs[slot].id);
        let groups = inp.slots.div_ceil(CULL_GROUP).max(1);
        let reset_groups = (s * 4 * ARGS_WORDS as u32).max(STAT_WORDS as u32).div_ceil(CULL_GROUP);
        let (p_reset, p_early, p_late) = (pipes.cull_reset, pipes.cull_early, pipes.cull_late);
        let sw = ResourceState::ShaderWrite;
        let bufs = [meshes, state, args, visible, stats];
        cull_pass(g, "cull_early", inp, bufs).read(templ, ResourceState::ShaderRead).exec(move |ctx| {
            ctx.enc.set_compute_pipeline(p_reset);
            ctx.enc.set_compute_constants(&cc);
            ctx.enc.dispatch(reset_groups, 1, 1);
            ctx.enc.transition(&[
                Transition::Buffer { buffer: args_id, from: sw, to: sw },
                Transition::Buffer { buffer: stats_id, from: sw, to: sw },
            ]);
            ctx.enc.set_compute_pipeline(p_early);
            ctx.enc.set_compute_constants(&cc);
            ctx.enc.dispatch(groups, 1, 1);
        });

        let vc = constants(&[inp.frame.1, inp.instances.1, inp.nodes.1, v.visible.bindless.0, inp.verts.1]);
        let (solid, dither) = (pipes.vis_solid, pipes.vis_dither);
        let idx_id = inp.indices.2;
        let raster = |g: &mut FrameGraph<'a>, name, phase: u32, first: bool| {
            let timer = draw_loop.clone();
            let (cl, dl) = if first { (LoadOp::Clear([0.0; 4]), LoadOp::Clear([0.0; 4])) } else { (LoadOp::Load, LoadOp::Load) };
            g.add_pass(name, PassKind::Raster)
                .read(inp.frame.0, ResourceState::ShaderRead)
                .read(inp.instances.0, ResourceState::ShaderRead)
                .read(inp.nodes.0, ResourceState::ShaderRead)
                .read(inp.verts.0, ResourceState::ShaderRead)
                .read(inp.indices.0, ResourceState::ShaderRead)
                .read(visible, ResourceState::ShaderRead)
                .read(args, ResourceState::IndirectArgs)
                .color(out.vis, cl, StoreOp::Store)
                .depth(out.depth, dl, StoreOp::Store, true)
                .exec(move |ctx| {
                    if s == 0 {
                        return;
                    }
                    let t0 = std::time::Instant::now();
                    ctx.enc.set_index_buffer(idx_id, 0);
                    for (kind, pipe) in [(0u32, solid), (1, dither)] {
                        ctx.enc.set_render_pipeline(pipe);
                        ctx.enc.set_draw_constants(&vc);
                        let off = ((phase * 2 + kind) * s) as u64 * (ARGS_WORDS * 4) as u64;
                        ctx.enc.draw_indexed_indirect_multi(args_id, off, s, None);
                    }
                    timer.set(timer.get() + t0.elapsed().as_secs_f64() * 1000.0);
                });
        };
        raster(g, "raster_early", 0, true);

        if occlusion {
            let (pf, pd) = (pipes.hzb_first, pipes.hzb_down);
            let levels = hzb_levels.clone();
            g.add_pass("hzb", PassKind::Compute).read(out.depth, ResourceState::ShaderRead).write(hzb, sw).exec(move |ctx| {
                let depth_bi = ctx.bindless(out.depth).0;
                let (mut sw_, mut sh_, mut soff) = (dw, dh, 0u32);
                for (i, &(off, w, h)) in levels.iter().enumerate() {
                    if i > 0 {
                        ctx.enc.transition(&[Transition::Buffer { buffer: hzb_id, from: sw, to: sw }]);
                    }
                    ctx.enc.set_compute_pipeline(if i == 0 { pf } else { pd });
                    ctx.enc.set_compute_constants(&constants(&[hzb_bi, depth_bi, soff, sw_, sh_, off, w, h]));
                    ctx.enc.dispatch(w.div_ceil(8), h.div_ceil(8), 1);
                    (sw_, sh_, soff) = (w, h, off);
                }
            });
        }

        let late = cull_pass(g, "cull_late", inp, bufs);
        let late = if occlusion { late.read(hzb, ResourceState::ShaderRead) } else { late };
        late.exec(move |ctx| {
            ctx.enc.set_compute_pipeline(p_late);
            ctx.enc.set_compute_constants(&cc);
            ctx.enc.dispatch(groups, 1, 1);
        });
        raster(g, "raster_late", 1, false);

        // Sun shadow casters (M4d): cull every instance against every cascade on the GPU, then
        // one depth pass that draws each cascade's bins into its atlas tile.
        if let (Some(sh), Some((sargs, svis))) = (out.shadow, shadow_bufs) {
            let mut scw = cw;
            scw[4] = v.shadow_args.bindless.0;
            scw[5] = v.shadow_visible.bindless.0;
            let scc = constants(&scw);
            let sargs_id = v.shadow_args.id;
            let (p_sreset, p_scull, p_sraster) = (pipes.shadow_reset, pipes.shadow_cull, pipes.shadow_raster);
            let copy_groups = (s * 4 * ARGS_WORDS as u32).max(1).div_ceil(CULL_GROUP);
            cull_pass(g, "shadow_cull", inp, [meshes, state, sargs, svis, stats]).read(templ, ResourceState::ShaderRead).exec(move |ctx| {
                ctx.enc.set_compute_pipeline(p_sreset);
                ctx.enc.set_compute_constants(&scc);
                ctx.enc.dispatch(copy_groups, 1, 1);
                ctx.enc.transition(&[Transition::Buffer { buffer: sargs_id, from: sw, to: sw }]);
                ctx.enc.set_compute_pipeline(p_scull);
                ctx.enc.set_compute_constants(&scc);
                ctx.enc.dispatch(groups, 1, 1);
            });
            let svc_base = [inp.frame.1, inp.instances.1, inp.nodes.1, v.shadow_visible.bindless.0, inp.verts.1];
            let (n, tile) = (sh.cascades.min(4), sh.tile);
            g.add_pass("shadow_raster", PassKind::Raster)
                .read(inp.frame.0, ResourceState::ShaderRead)
                .read(inp.instances.0, ResourceState::ShaderRead)
                .read(inp.nodes.0, ResourceState::ShaderRead)
                .read(inp.verts.0, ResourceState::ShaderRead)
                .read(inp.indices.0, ResourceState::ShaderRead)
                .read(svis, ResourceState::ShaderRead)
                .read(sargs, ResourceState::IndirectArgs)
                .depth(sh.atlas, LoadOp::Clear([0.0; 4]), StoreOp::Store, true)
                .exec(move |ctx| {
                    if s == 0 {
                        return;
                    }
                    ctx.enc.set_index_buffer(idx_id, 0);
                    ctx.enc.set_render_pipeline(p_sraster);
                    for c in 0..n {
                        let (x, y) = ((c % 2) * tile, (c / 2) * tile);
                        ctx.enc.set_viewport(x as f32, y as f32, tile as f32, tile as f32);
                        ctx.enc.set_scissor(x, y, tile, tile);
                        let mut w = [0u32; 6];
                        w[..5].copy_from_slice(&svc_base);
                        w[5] = c;
                        ctx.enc.set_draw_constants(&constants(&w));
                        let off = (c * s) as u64 * (ARGS_WORDS * 4) as u64;
                        ctx.enc.draw_indexed_indirect_multi(sargs_id, off, s, None);
                    }
                });
        }
        ProviderOutputs { meshes: (meshes, mesh_table.bindless.0), state: (state, v.state.bindless.0) }
    }

    fn finish(&mut self, plan: &Plan) {
        let v = &mut self.views[self.cur];
        for &(r, which) in &v.imports {
            let s = plan.final_states[r.index()];
            match which {
                Imported::MeshTable => self.mesh_table.state = s,
                Imported::State => v.state.state = s,
                Imported::Args => v.args.state = s,
                Imported::Templ => self.templ.state = s,
                Imported::Visible => v.visible.state = s,
                Imported::ShadowArgs => v.shadow_args.state = s,
                Imported::ShadowVisible => v.shadow_visible.state = s,
                Imported::Hzb => {
                    if let Some(h) = v.hzb.as_mut() {
                        h.0.state = s;
                    }
                }
                Imported::Stats(i) => v.stats_ring.bufs[i].state = s,
            }
        }
    }

    fn pass_names(&self) -> &'static [&'static str] {
        if self.config.occlusion {
            &["cull_early", "raster_early", "hzb", "cull_late", "raster_late"]
        } else {
            &["cull_early", "raster_early", "cull_late", "raster_late"]
        }
    }

    fn destroy(self: Box<Self>, dev: &dyn Device) {
        let s = *self;
        s.mesh_table.destroy(dev);
        s.templ.destroy(dev);
        for v in s.views {
            v.destroy(dev);
        }
        s.p.destroy(dev);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lod_selection_and_hysteresis() {
        let e = [0.0, 0.01, 0.05, 0.2];
        // 1 px threshold: level l is allowed when error * k <= 1.
        assert_eq!(select_lod(&e, 200.0, 0, false, 1.0, 0.75), 0);
        assert_eq!(select_lod(&e, 90.0, 0, false, 1.0, 0.75), 1);
        assert_eq!(select_lod(&e, 19.0, 0, false, 1.0, 0.75), 2);
        assert_eq!(select_lod(&e, 4.0, 0, false, 1.0, 0.75), 3);
        // Hysteresis: at k = 90 level 1 is allowed (0.9 px) but not under 0.75 px.
        assert_eq!(select_lod(&e, 90.0, 0, true, 1.0, 0.75), 0);
        assert_eq!(select_lod(&e, 70.0, 0, true, 1.0, 0.75), 1);
        // Going finer is immediate.
        assert_eq!(select_lod(&e, 200.0, 2, true, 1.0, 0.75), 0);
    }

    #[test]
    fn pyramid_levels_end_at_one_texel() {
        let l = hzb_levels(2560, 1440);
        assert_eq!(l[0], (0, 1280, 720));
        assert_eq!(*l.last().unwrap(), (l.last().unwrap().0, 1, 1));
        assert_eq!(l[1].0, 1280 * 720);
        assert_eq!(hzb_levels(1, 1), vec![(0, 1, 1)]);
        assert_eq!(hzb_levels(5, 3), vec![(0, 2, 1), (2, 1, 1)]);
    }
}
