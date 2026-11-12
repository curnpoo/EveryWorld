//! Metal backend (P1-D2). Private: nothing here is visible outside the crate.
//!
//! Design notes (the contract has the full list in "Changes made during the Metal bring-up"):
//! - All mutable state lives in one `Mutex<State>`; command recording locks it briefly to
//!   resolve handles and update tracked states.
//! - Bindless tables are Metal 3 argument buffers written directly from the CPU: 8-byte
//!   `MTLResourceID`s for textures and samplers, 8-byte GPU addresses for buffers. Bound
//!   resources are made resident through one `MTLResidencySet` attached to the queue.
//! - The buffer table has one copy per frame in flight, so a `cpu_write` buffer (one Metal
//!   buffer per frame in flight) has a fixed address in each copy.
//! - Synchronisation between passes comes from the caller's transitions (see `encoder.rs`).

mod conv;
mod encoder;

use std::collections::VecDeque;
use std::ptr::NonNull;
use std::sync::{Mutex, MutexGuard};

use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{ClassType, msg_send};
use objc2_core_foundation::CGSize;
use objc2_foundation::NSRange;
use objc2_metal::*;
use objc2_quartz_core::{CAMetalDrawable, CAMetalLayer};

use crate::pool::{Pool, Slots};
use crate::track::{States, usage_allows};
use crate::*;

pub(crate) type P<T> = Retained<ProtocolObject<T>>;

/// Table sizes this backend allocates (8 bytes per entry).
const TEXTURE_TABLE: u32 = 1 << 17;
const BUFFER_TABLE: u32 = 1 << 16;
/// Timed passes per frame (4 counter samples each).
const MAX_TIMED_PASSES: usize = 1024;

pub(crate) struct Buffer {
    /// One per frame in flight for `cpu_write`, otherwise one.
    raws: Vec<P<dyn MTLBuffer>>,
    size: u64,
    usage: Usage,
    states: States,
    bound: Option<u32>,
}

pub(crate) struct Texture {
    raw: P<dyn MTLTexture>,
    kind: TextureKind,
    format: Format,
    width: u32,
    height: u32,
    depth_or_layers: u32,
    mips: u32,
    usage: Usage,
    states: States,
    bound: Option<u32>,
    /// A surface's drawable texture for this frame.
    surface: bool,
}

impl Texture {
    fn layers(&self) -> u32 {
        match self.kind {
            TextureKind::D2Array | TextureKind::Cube => self.depth_or_layers,
            _ => 1,
        }
    }
    fn depth(&self) -> u32 {
        if self.kind == TextureKind::D3 { self.depth_or_layers } else { 1 }
    }
}

struct Sampler {
    raw: P<dyn MTLSamplerState>,
    bound: Option<u32>,
}

struct Shader {
    func: P<dyn MTLFunction>,
    stage: ShaderStage,
}

pub(crate) struct RenderPipe {
    pso: P<dyn MTLRenderPipelineState>,
    depth_state: Option<P<dyn MTLDepthStencilState>>,
    prim: MTLPrimitiveType,
    cull: MTLCullMode,
    bias: (f32, f32),
    clamp: bool,
    colors: Vec<Format>,
    depth: Option<(Format, bool)>,
}

pub(crate) struct ComputePipe {
    pso: P<dyn MTLComputePipelineState>,
    group: MTLSize,
}

struct Surface {
    layer: Retained<CAMetalLayer>,
    config: SurfaceConfig,
}

/// Objects kept alive until the frames that may use them have finished.
enum Deferred {
    Buffer(#[allow(dead_code)] Buffer),
    Texture(#[allow(dead_code)] Texture),
    Sampler(#[allow(dead_code)] Sampler),
    RenderPipe(#[allow(dead_code)] RenderPipe),
    ComputePipe(#[allow(dead_code)] ComputePipe),
}

/// One timed pass: base index of its counter samples, and how many (2 or 4).
#[derive(Clone, Copy)]
pub(crate) struct TimedPass {
    base: usize,
    samples: usize,
}

/// A finished timer scope, in passes (counter path) or command buffers (submission path).
#[derive(Clone)]
pub(crate) struct Scope {
    label: &'static str,
    depth: u32,
    first: usize,
    end: usize,
}

struct FrameRec {
    index: u64,
    cbs: Vec<P<dyn MTLCommandBuffer>>,
    slot: usize,
    passes: Vec<TimedPass>,
    scopes: Vec<Scope>,
}

enum Upload {
    Buffer { staging: P<dyn MTLBuffer>, dst: BufferId, offset: u64 },
    Texture { staging: P<dyn MTLBuffer>, dst: TextureId, sub: Subresource },
}

struct Tables {
    textures: P<dyn MTLBuffer>,
    tex_slots: Slots,
    tex_res: Vec<Option<P<dyn MTLTexture>>>,
    tex_owner: Vec<Option<TextureId>>,
    /// One per frame in flight.
    buffers: Vec<P<dyn MTLBuffer>>,
    buf_slots: Slots,
    buf_res: Vec<Option<Vec<P<dyn MTLBuffer>>>>,
    buf_owner: Vec<Option<BufferId>>,
    samplers: P<dyn MTLBuffer>,
    smp_slots: Slots,
    smp_res: Vec<Option<P<dyn MTLSamplerState>>>,
    smp_owner: Vec<Option<SamplerId>>,
}

pub(crate) struct State {
    buffers: Pool<Buffer>,
    textures: Pool<Texture>,
    samplers: Pool<Sampler>,
    shaders: Pool<Shader>,
    rpipes: Pool<RenderPipe>,
    cpipes: Pool<ComputePipe>,
    surfaces: Pool<Surface>,
    tables: Tables,
    residency: P<dyn MTLResidencySet>,
    residency_dirty: bool,
    frames_begun: u64,
    completed: u64,
    frame_open: bool,
    in_flight: VecDeque<FrameRec>,
    deferred: Vec<(u64, Deferred)>,
    uploads: Vec<Upload>,
    /// Drawables acquired in the open frame.
    acquired: Vec<(P<dyn CAMetalDrawable>, TextureId)>,
    /// Fences per frame slot, one per encoder (grown on demand).
    fences: Vec<Vec<P<dyn MTLFence>>>,
    /// Encoders of the previous frame whose fences nobody has waited on yet.
    tail_fences: Vec<P<dyn MTLFence>>,
    counters: Vec<P<dyn MTLCounterSampleBuffer>>,
    /// (cpu ns, gpu ticks) at creation, and the derived ns per GPU tick.
    clock0: (u64, u64),
    ns_per_tick: f64,
    timings: Vec<Timing>,
}

/// `State` holds Metal objects, which Apple documents as safe to use from any thread; all
/// access goes through the mutex.
struct Locked(Mutex<State>);
// SAFETY: every Metal object in `State` is only touched while the mutex is held, and Metal
// resource, pipeline, layer and command objects may be used from any thread one at a time.
unsafe impl Send for Locked {}
// SAFETY: as above; shared access always goes through the mutex.
unsafe impl Sync for Locked {}

pub(crate) struct MetalDevice {
    caps: Caps,
    device: P<dyn MTLDevice>,
    queue: P<dyn MTLCommandQueue>,
    validation: bool,
    frames: u32,
    state: Locked,
    /// Zeroed constants bound at every encoder start.
    zero_constants: [u8; DRAW_CONSTANTS_BYTES],
}

fn invalid(what: impl Into<String>) -> Error {
    Error::Invalid(what.into())
}

fn shared_buffer(device: &ProtocolObject<dyn MTLDevice>, size: usize, label: &str) -> Result<P<dyn MTLBuffer>> {
    let b = device
        .newBufferWithLength_options(size.max(4), MTLResourceOptions::StorageModeShared)
        .ok_or(Error::OutOfMemory)?;
    b.setLabel(Some(&conv::ns(label)));
    Ok(b)
}

/// Write one 8-byte table entry.
fn write_entry(table: &ProtocolObject<dyn MTLBuffer>, index: u32, value: u64) {
    let p = table.contents().as_ptr() as *mut u64;
    debug_assert!((index as usize + 1) * 8 <= table.length());
    // SAFETY: `index` is below the table capacity the buffer was allocated for (slot
    // allocators never exceed it); the memory is shared and CPU-writable. In-flight frames
    // never read this entry (slots are reused only after their frames finished).
    unsafe { p.add(index as usize).write(value) };
}

impl MetalDevice {
    pub fn new(desc: &DeviceDesc) -> Result<Self> {
        let device = MTLCreateSystemDefaultDevice().ok_or_else(|| Error::Unsupported("no Metal device".into()))?;
        if device.argumentBuffersSupport() != MTLArgumentBuffersTier::Tier2 {
            return Err(Error::Unsupported("Metal argument buffers tier 2 is required for bindless tables".into()));
        }
        let queue = device.newCommandQueue().ok_or_else(|| Error::Backend("newCommandQueue failed".into()))?;
        queue.setLabel(Some(&conv::ns("ed_gpu queue")));

        let frames = desc.frames_in_flight;
        let max_samplers = (device.maxArgumentBufferSamplerCount() as u32).min(4096);
        let tables = Tables {
            textures: shared_buffer(&device, TEXTURE_TABLE as usize * 8, "bindless textures")?,
            tex_slots: Slots::new(TEXTURE_TABLE),
            tex_res: Vec::new(),
            tex_owner: Vec::new(),
            buffers: (0..frames)
                .map(|f| shared_buffer(&device, BUFFER_TABLE as usize * 8, &format!("bindless buffers (frame slot {f})")))
                .collect::<Result<_>>()?,
            buf_slots: Slots::new(BUFFER_TABLE),
            buf_res: Vec::new(),
            buf_owner: Vec::new(),
            samplers: shared_buffer(&device, max_samplers as usize * 8, "bindless samplers")?,
            smp_slots: Slots::new(max_samplers),
            smp_res: Vec::new(),
            smp_owner: Vec::new(),
        };

        let rdesc = MTLResidencySetDescriptor::new();
        rdesc.setLabel(Some(&conv::ns("ed_gpu bindless residency")));
        let residency = device
            .newResidencySetWithDescriptor_error(&rdesc)
            .map_err(|e| Error::Unsupported(format!("residency sets (macOS 15+) are required: {}", e.localizedDescription())))?;
        queue.addResidencySet(&residency);

        // GPU timers: counter sample buffers at stage boundaries, else per-submission times.
        let force_submission = std::env::var("ED_GPU_TIMERS").is_ok_and(|v| v == "submission");
        let mut counters = Vec::new();
        if !force_submission
            && device.supportsCounterSampling(MTLCounterSamplingPoint::AtStageBoundary)
            && let Some(set) = device.counterSets().and_then(|sets| {
                (0..sets.count()).map(|i| sets.objectAtIndex(i)).find(|s| s.name().to_string() == "timestamp")
            })
        {
            for f in 0..frames {
                let d = MTLCounterSampleBufferDescriptor::new();
                d.setCounterSet(Some(&set));
                d.setStorageMode(MTLStorageMode::Shared);
                // SAFETY: plain count; Metal validates the limit and errors below.
                unsafe { d.setSampleCount(MAX_TIMED_PASSES * 4) };
                d.setLabel(&conv::ns(&format!("ed_gpu timers (frame slot {f})")));
                match device.newCounterSampleBufferWithDescriptor_error(&d) {
                    Ok(b) => counters.push(b),
                    Err(_) => {
                        counters.clear();
                        break;
                    }
                }
            }
        }
        let timers = if !counters.is_empty() { TimerSource::PassBoundary } else { TimerSource::Submission };
        let clock0 = sample_clocks(&device);

        let caps = probe_caps(&device, max_samplers, timers);
        let state = State {
            buffers: Pool::default(),
            textures: Pool::default(),
            samplers: Pool::default(),
            shaders: Pool::default(),
            rpipes: Pool::default(),
            cpipes: Pool::default(),
            surfaces: Pool::default(),
            tables,
            residency,
            residency_dirty: false,
            frames_begun: 0,
            completed: 0,
            frame_open: false,
            in_flight: VecDeque::new(),
            deferred: Vec::new(),
            uploads: Vec::new(),
            acquired: Vec::new(),
            fences: (0..frames).map(|_| Vec::new()).collect(),
            tail_fences: Vec::new(),
            counters,
            clock0,
            ns_per_tick: 1.0,
            timings: Vec::new(),
        };
        Ok(Self {
            caps,
            device,
            queue,
            validation: desc.validation,
            frames,
            state: Locked(Mutex::new(state)),
            zero_constants: [0; DRAW_CONSTANTS_BYTES],
        })
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        // A panic while holding the lock leaves plain bookkeeping behind; keep going.
        self.state.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Frames that must finish before something released now can be freed.
    fn retire_after(st: &State) -> u64 {
        st.frames_begun
    }

    fn defer(&self, st: &mut State, d: Deferred) {
        let after = Self::retire_after(st);
        if after <= st.completed {
            drop(d);
        } else {
            st.deferred.push((after, d));
        }
    }

    /// Retire finished frames: timings, deferred destruction, bindless slot reuse.
    fn retire(&self, st: &mut State) {
        while let Some(f) = st.in_flight.front() {
            let done = f.cbs.iter().all(|cb| matches!(cb.status(), MTLCommandBufferStatus::Completed | MTLCommandBufferStatus::Error));
            if !done {
                break;
            }
            let f = st.in_flight.pop_front().expect("front exists");
            self.resolve_timings(st, &f);
            st.completed = f.index + 1;
        }
        let completed = st.completed;
        st.deferred.retain(|(after, _)| *after > completed);
        let mut freed = Vec::new();
        st.tables.tex_slots.retire(completed, &mut freed);
        for s in freed.drain(..) {
            write_entry(&st.tables.textures, s, 0);
            if let Some(t) = st.tables.tex_res[s as usize].take() {
                st.residency.removeAllocation(ProtocolObject::from_ref(&*t));
                st.residency_dirty = true;
            }
        }
        st.tables.buf_slots.retire(completed, &mut freed);
        for s in freed.drain(..) {
            for t in &st.tables.buffers {
                write_entry(t, s, 0);
            }
            if let Some(bs) = st.tables.buf_res[s as usize].take() {
                for b in bs {
                    st.residency.removeAllocation(ProtocolObject::from_ref(&*b));
                }
                st.residency_dirty = true;
            }
        }
        st.tables.smp_slots.retire(completed, &mut freed);
        for s in freed.drain(..) {
            write_entry(&st.tables.samplers, s, 0);
            st.tables.smp_res[s as usize] = None;
        }
    }

    fn resolve_timings(&self, st: &mut State, f: &FrameRec) {
        if f.scopes.is_empty() {
            return;
        }
        let mut out = Vec::with_capacity(f.scopes.len());
        if st.counters.is_empty() {
            // Submission path: scope = command buffers [first, end).
            for s in &f.scopes {
                let mut t0 = f64::MAX;
                let mut t1 = 0f64;
                for cb in &f.cbs[s.first.min(f.cbs.len())..s.end.min(f.cbs.len())] {
                    let (a, b) = (cb.GPUStartTime(), cb.GPUEndTime());
                    if a > 0.0 && b >= a {
                        t0 = t0.min(a);
                        t1 = t1.max(b);
                    }
                }
                let ms = if t1 >= t0 { ((t1 - t0) * 1000.0) as f32 } else { 0.0 };
                out.push(Timing { label: s.label, depth: s.depth, gpu_ms: ms });
            }
        } else {
            let n = f.passes.last().map(|p| p.base + p.samples).unwrap_or(0);
            let mut stamps: Vec<u64> = Vec::new();
            if n > 0 {
                // SAFETY: the frame finished, so the samples are written; the range is within
                // the sample count allocated.
                if let Some(data) = unsafe { st.counters[f.slot].resolveCounterRange(NSRange { location: 0, length: n }) } {
                    // SAFETY: the NSData is alive for this block and not mutated.
                    let bytes = unsafe { data.as_bytes_unchecked() };
                    stamps = bytes.as_chunks::<8>().0.iter().map(|c| u64::from_ne_bytes(*c)).collect();
                }
            }
            self.recalibrate(st);
            for s in &f.scopes {
                let mut t0 = u64::MAX;
                let mut t1 = 0u64;
                for p in &f.passes[s.first.min(f.passes.len())..s.end.min(f.passes.len())] {
                    // Starts: first sample(s); ends: last sample(s). Invalid samples are
                    // u64::MAX (MTLCounterErrorValue) or 0.
                    let ok = |v: u64| v != u64::MAX && v != 0;
                    let half = p.samples / 2;
                    for i in 0..half {
                        if let Some(&v) = stamps.get(p.base + i).filter(|v| ok(**v)) {
                            t0 = t0.min(v);
                        }
                    }
                    for i in half..p.samples {
                        if let Some(&v) = stamps.get(p.base + i).filter(|v| ok(**v)) {
                            t1 = t1.max(v);
                        }
                    }
                }
                let ms = if t1 > t0 { ((t1 - t0) as f64 * st.ns_per_tick / 1.0e6) as f32 } else { 0.0 };
                out.push(Timing { label: s.label, depth: s.depth, gpu_ms: ms });
            }
        }
        st.timings = out;
    }

    /// GPU timestamp ticks -> ns, from the CPU (ns) and GPU clocks sampled together.
    fn recalibrate(&self, st: &mut State) {
        let now = sample_clocks(&self.device);
        let dc = now.0.saturating_sub(st.clock0.0);
        let dg = now.1.saturating_sub(st.clock0.1);
        if dc > 10_000_000 && dg > 0 {
            st.ns_per_tick = dc as f64 / dg as f64;
        }
    }

    fn fence(&self, st: &mut State, slot: usize, i: usize) -> Result<P<dyn MTLFence>> {
        while st.fences[slot].len() <= i {
            let f = self.device.newFence().ok_or_else(|| Error::Backend("newFence failed".into()))?;
            f.setLabel(Some(&conv::ns(&format!("ed_gpu fence {slot}.{}", st.fences[slot].len()))));
            st.fences[slot].push(f);
        }
        Ok(st.fences[slot][i].clone())
    }
}

fn sample_clocks(device: &ProtocolObject<dyn MTLDevice>) -> (u64, u64) {
    let (mut cpu, mut gpu) = (0u64, 0u64);
    // SAFETY: both pointers are to live, writable u64s on this stack frame.
    unsafe { device.sampleTimestamps_gpuTimestamp(NonNull::from(&mut cpu), NonNull::from(&mut gpu)) };
    (cpu, gpu)
}

/// Whether a kernel using 64-bit atomics on a buffer compiles and links on this device.
fn probe_atomics_64(device: &ProtocolObject<dyn MTLDevice>) -> bool {
    const SRC: &str = "#include <metal_stdlib>\nusing namespace metal;\nkernel void k(device atomic_ulong* a [[buffer(0)]], uint i [[thread_position_in_grid]]) { atomic_max_explicit(a, ulong(i), memory_order_relaxed); }\n";
    let Ok(lib) = device.newLibraryWithSource_options_error(&conv::ns(SRC), None) else { return false };
    let Some(f) = lib.newFunctionWithName(&conv::ns("k")) else { return false };
    device.newComputePipelineStateWithFunction_error(&f).is_ok()
}

fn probe_caps(device: &ProtocolObject<dyn MTLDevice>, max_samplers: u32, timers: TimerSource) -> Caps {
    let fam = |f| device.supportsFamily(f);
    let tg = device.maxThreadsPerThreadgroup();
    Caps {
        backend: Backend::Metal,
        device_name: device.name().to_string(),
        memory_budget: device.recommendedMaxWorkingSetSize(),
        unified_memory: device.hasUnifiedMemory(),
        bindless_tier: if device.argumentBuffersSupport() == MTLArgumentBuffersTier::Tier2 { 2 } else { 1 },
        max_bindless_textures: TEXTURE_TABLE,
        max_bindless_buffers: BUFFER_TABLE,
        max_bindless_samplers: max_samplers,
        // Apple3+ and Mac2 GPUs: 16384 (Metal feature set tables; there is no query).
        max_texture_size: if fam(MTLGPUFamily::Apple3) || fam(MTLGPUFamily::Mac2) { 16384 } else { 8192 },
        max_workgroup_threads: (tg.width as u32).min(tg.height as u32 * tg.depth as u32).max(tg.width as u32),
        max_workgroup_memory: device.maxThreadgroupMemoryLength() as u32,
        indirect_count: false,
        native_multi_draw_indirect: false,
        mesh_shaders: fam(MTLGPUFamily::Metal3),
        ray_tracing: device.supportsRaytracing(),
        atomics_64: probe_atomics_64(device),
        sparse_textures: fam(MTLGPUFamily::Apple6) && device.sparseTileSizeInBytes() > 0,
        bc_textures: device.supportsBCTextureCompression(),
        astc_textures: fam(MTLGPUFamily::Apple2),
        timers,
        // CAMetalLayer supports extended-range output on every macOS this backend runs on;
        // whether the current display shows it is a per-display question for the app.
        hdr_output: true,
    }
}

impl Drop for MetalDevice {
    fn drop(&mut self) {
        self.wait_idle();
    }
}

impl Device for MetalDevice {
    fn caps(&self) -> &Caps {
        &self.caps
    }

    fn create_buffer(&self, desc: &BufferDesc<'_>) -> Result<BufferId> {
        if desc.size == 0 {
            return Err(invalid(format!("buffer '{}': size 0", desc.label)));
        }
        if desc.usage.cpu_write && desc.usage.cpu_read {
            return Err(invalid(format!("buffer '{}': cpu_write and cpu_read together are not supported", desc.label)));
        }
        let shared = desc.usage.cpu_write || desc.usage.cpu_read;
        let opts = if shared { MTLResourceOptions::StorageModeShared } else { MTLResourceOptions::StorageModePrivate };
        let copies = if desc.usage.cpu_write { self.frames } else { 1 };
        let raws = autoreleasepool(|_| {
            (0..copies)
                .map(|i| {
                    let b = self.device.newBufferWithLength_options(desc.size as usize, opts).ok_or(Error::OutOfMemory)?;
                    let label = if copies > 1 { format!("{} [{i}]", desc.label) } else { desc.label.to_string() };
                    b.setLabel(Some(&conv::ns(&label)));
                    Ok(b)
                })
                .collect::<Result<Vec<_>>>()
        })?;
        let mut st = self.lock();
        let (i, g) = st.buffers.insert(Buffer { raws, size: desc.size, usage: desc.usage, states: States::new(1, 1), bound: None });
        Ok(BufferId::from_parts(i, g))
    }

    fn create_texture(&self, desc: &TextureDesc<'_>) -> Result<TextureId> {
        let layers = match desc.kind {
            TextureKind::Cube if desc.depth_or_layers != 6 => return Err(invalid(format!("texture '{}': cube needs depth_or_layers 6", desc.label))),
            TextureKind::D2 if desc.depth_or_layers != 1 => return Err(invalid(format!("texture '{}': D2 needs depth_or_layers 1", desc.label))),
            TextureKind::D2Array | TextureKind::Cube => desc.depth_or_layers,
            _ => 1,
        };
        let max_mips = 32 - desc.width.max(desc.height).max(1).leading_zeros();
        if desc.width == 0 || desc.height == 0 || desc.depth_or_layers == 0 || desc.mips == 0 || desc.mips > max_mips {
            return Err(invalid(format!("texture '{}': bad size or mip count", desc.label)));
        }
        if desc.usage.cpu_read || desc.usage.cpu_write {
            return Err(invalid(format!("texture '{}': CPU access goes through buffers", desc.label)));
        }
        if desc.usage.depth != desc.format.is_depth() {
            return Err(invalid(format!("texture '{}': usage.depth must match a depth format", desc.label)));
        }
        let raw = autoreleasepool(|_| {
            let d = MTLTextureDescriptor::new();
            d.setTextureType(conv::texture_type(desc.kind, desc.samples));
            d.setPixelFormat(conv::format(desc.format));
            // SAFETY: plain sizes, validated above; Metal validates the limits.
            unsafe {
                d.setWidth(desc.width as usize);
                d.setHeight(desc.height as usize);
                d.setDepth(if desc.kind == TextureKind::D3 { desc.depth_or_layers as usize } else { 1 });
                d.setArrayLength(if desc.kind == TextureKind::D2Array { desc.depth_or_layers as usize } else { 1 });
                d.setMipmapLevelCount(desc.mips as usize);
                d.setSampleCount(desc.samples.max(1) as usize);
            }
            d.setStorageMode(MTLStorageMode::Private);
            let mut u = MTLTextureUsage::Unknown;
            if desc.usage.shader_read || desc.usage.depth {
                u |= MTLTextureUsage::ShaderRead;
            }
            if desc.usage.shader_write {
                u |= MTLTextureUsage::ShaderWrite;
            }
            if desc.usage.render_target || desc.usage.depth {
                u |= MTLTextureUsage::RenderTarget;
            }
            d.setUsage(u);
            let t = self.device.newTextureWithDescriptor(&d).ok_or(Error::OutOfMemory)?;
            t.setLabel(Some(&conv::ns(desc.label)));
            Ok::<_, Error>(t)
        })?;
        let tex = Texture {
            raw,
            kind: desc.kind,
            format: desc.format,
            width: desc.width,
            height: desc.height,
            depth_or_layers: desc.depth_or_layers,
            mips: desc.mips,
            usage: desc.usage,
            states: States::new(desc.mips, layers),
            bound: None,
            surface: false,
        };
        let (i, g) = self.lock().textures.insert(tex);
        Ok(TextureId::from_parts(i, g))
    }

    fn create_sampler(&self, desc: &SamplerDesc) -> Result<SamplerId> {
        let raw = autoreleasepool(|_| {
            let d = MTLSamplerDescriptor::new();
            d.setMinFilter(conv::min_mag(desc.min_mag));
            d.setMagFilter(conv::min_mag(desc.min_mag));
            d.setMipFilter(conv::mip(desc.mip));
            let a = conv::address(desc.address);
            d.setSAddressMode(a);
            d.setTAddressMode(a);
            d.setRAddressMode(a);
            d.setMaxAnisotropy(desc.anisotropy.clamp(1, 16) as usize);
            if let Some(c) = desc.compare {
                d.setCompareFunction(conv::compare(c));
            }
            d.setSupportArgumentBuffers(true);
            self.device.newSamplerStateWithDescriptor(&d).ok_or_else(|| Error::Backend("newSamplerState failed".into()))
        })?;
        let (i, g) = self.lock().samplers.insert(Sampler { raw, bound: None });
        Ok(SamplerId::from_parts(i, g))
    }

    fn create_shader(&self, desc: &ShaderDesc<'_>) -> Result<ShaderId> {
        let lib = autoreleasepool(|_| match desc.code {
            ShaderCode::MslSource(src) => {
                self.device.newLibraryWithSource_options_error(&conv::ns(src), None).map_err(|e| invalid(format!("shader '{}': MSL compile failed: {}", desc.label, e.localizedDescription())))
            }
            ShaderCode::MetalLib(bytes) => {
                let data = dispatch2::DispatchData::from_bytes(bytes);
                self.device.newLibraryWithData_error(&data).map_err(|e| invalid(format!("shader '{}': metallib load failed: {}", desc.label, e.localizedDescription())))
            }
            ShaderCode::SpirV(_) | ShaderCode::Dxil(_) => Err(invalid(format!("shader '{}': the Metal backend takes MslSource or MetalLib, not SPIR-V or DXIL", desc.label))),
        })?;
        lib.setLabel(Some(&conv::ns(desc.label)));
        let func = lib.newFunctionWithName(&conv::ns(desc.entry)).ok_or_else(|| invalid(format!("shader '{}': no entry point '{}'", desc.label, desc.entry)))?;
        let want = match desc.stage {
            ShaderStage::Vertex => MTLFunctionType::Vertex,
            ShaderStage::Fragment => MTLFunctionType::Fragment,
            ShaderStage::Compute => MTLFunctionType::Kernel,
        };
        if func.functionType() != want {
            return Err(invalid(format!("shader '{}': entry '{}' is not a {:?} function", desc.label, desc.entry, desc.stage)));
        }
        func.setLabel(Some(&conv::ns(desc.label)));
        let (i, g) = self.lock().shaders.insert(Shader { func, stage: desc.stage });
        Ok(ShaderId::from_parts(i, g))
    }

    fn create_render_pipeline(&self, desc: &RenderPipelineDesc<'_>) -> Result<RenderPipelineId> {
        desc.validate()?;
        let (vs, fs) = {
            let st = self.lock();
            let vs = st.shaders.get(desc.vertex.index(), desc.vertex.generation()).ok_or_else(|| invalid(format!("pipeline '{}': stale vertex shader", desc.label)))?;
            if vs.stage != ShaderStage::Vertex {
                return Err(invalid(format!("pipeline '{}': vertex shader has stage {:?}", desc.label, vs.stage)));
            }
            let fs = match desc.fragment {
                Some(f) => {
                    let s = st.shaders.get(f.index(), f.generation()).ok_or_else(|| invalid(format!("pipeline '{}': stale fragment shader", desc.label)))?;
                    if s.stage != ShaderStage::Fragment {
                        return Err(invalid(format!("pipeline '{}': fragment shader has stage {:?}", desc.label, s.stage)));
                    }
                    Some(s.func.clone())
                }
                None => None,
            };
            (vs.func.clone(), fs)
        };
        if desc.color_formats.len() > 8 {
            return Err(invalid(format!("pipeline '{}': more than 8 colour targets", desc.label)));
        }
        let (prim, class) = conv::topology(desc.topology);
        let pso = autoreleasepool(|_| {
            let d = MTLRenderPipelineDescriptor::new();
            d.setLabel(Some(&conv::ns(desc.label)));
            d.setVertexFunction(Some(&vs));
            d.setFragmentFunction(fs.as_deref());
            // SAFETY: the topology class matches the primitive type used by draws.
            unsafe { d.setInputPrimitiveTopology(class) };
            d.setRasterSampleCount(desc.samples.max(1) as usize);
            for (i, f) in desc.color_formats.iter().enumerate() {
                // SAFETY: i < 8, within Metal's colour attachment array.
                let a = unsafe { d.colorAttachments().objectAtIndexedSubscript(i) };
                a.setPixelFormat(conv::format(*f));
                if let Some((s, dst, sa, da)) = conv::blend(desc.blend) {
                    a.setBlendingEnabled(true);
                    a.setSourceRGBBlendFactor(s);
                    a.setDestinationRGBBlendFactor(dst);
                    a.setSourceAlphaBlendFactor(sa);
                    a.setDestinationAlphaBlendFactor(da);
                }
            }
            if let Some(dd) = &desc.depth {
                d.setDepthAttachmentPixelFormat(conv::format(dd.format));
            }
            self.device.newRenderPipelineStateWithDescriptor_error(&d).map_err(|e| invalid(format!("pipeline '{}': {}", desc.label, e.localizedDescription())))
        })?;
        let depth_state = match &desc.depth {
            Some(dd) => {
                if !dd.format.is_depth() {
                    return Err(invalid(format!("pipeline '{}': depth format {:?} is not a depth format", desc.label, dd.format)));
                }
                let d = MTLDepthStencilDescriptor::new();
                d.setDepthCompareFunction(conv::compare(dd.compare));
                d.setDepthWriteEnabled(dd.write);
                d.setLabel(Some(&conv::ns(desc.label)));
                Some(self.device.newDepthStencilStateWithDescriptor(&d).ok_or_else(|| Error::Backend("newDepthStencilState failed".into()))?)
            }
            None => None,
        };
        let pipe = RenderPipe {
            pso,
            depth_state,
            prim,
            cull: conv::cull(desc.cull),
            bias: desc.depth.as_ref().map(|d| (d.bias, d.slope_bias)).unwrap_or((0.0, 0.0)),
            clamp: desc.depth.as_ref().is_some_and(|d| d.clamp),
            colors: desc.color_formats.to_vec(),
            depth: desc.depth.as_ref().map(|d| (d.format, d.write)),
        };
        let (i, g) = self.lock().rpipes.insert(pipe);
        Ok(RenderPipelineId::from_parts(i, g))
    }

    fn create_compute_pipeline(&self, desc: &ComputePipelineDesc<'_>) -> Result<ComputePipelineId> {
        let func = {
            let st = self.lock();
            let s = st.shaders.get(desc.shader.index(), desc.shader.generation()).ok_or_else(|| invalid(format!("pipeline '{}': stale shader", desc.label)))?;
            if s.stage != ShaderStage::Compute {
                return Err(invalid(format!("pipeline '{}': shader has stage {:?}", desc.label, s.stage)));
            }
            s.func.clone()
        };
        let [x, y, z] = desc.workgroup_size;
        let threads = x as u64 * y as u64 * z as u64;
        if threads == 0 || threads > self.caps.max_workgroup_threads as u64 {
            return Err(invalid(format!("pipeline '{}': workgroup size {:?}", desc.label, desc.workgroup_size)));
        }
        let pso = autoreleasepool(|_| {
            let d = MTLComputePipelineDescriptor::new();
            d.setLabel(Some(&conv::ns(desc.label)));
            d.setComputeFunction(Some(&func));
            self.device
                .newComputePipelineStateWithDescriptor_options_reflection_error(&d, MTLPipelineOption::None, None)
                .map_err(|e| invalid(format!("pipeline '{}': {}", desc.label, e.localizedDescription())))
        })?;
        if (pso.maxTotalThreadsPerThreadgroup() as u64) < threads {
            return Err(invalid(format!("pipeline '{}': workgroup of {threads} exceeds this kernel's limit {}", desc.label, pso.maxTotalThreadsPerThreadgroup())));
        }
        let group = MTLSize { width: x as usize, height: y as usize, depth: z as usize };
        let (i, g) = self.lock().cpipes.insert(ComputePipe { pso, group });
        Ok(ComputePipelineId::from_parts(i, g))
    }

    fn destroy_buffer(&self, id: BufferId) {
        let mut st = self.lock();
        if let Some(b) = st.buffers.remove(id.index(), id.generation()) {
            if let Some(slot) = b.bound {
                st.tables.buf_owner[slot as usize] = None;
                let after = Self::retire_after(&st);
                st.tables.buf_slots.free_after(slot, after);
            }
            self.defer(&mut st, Deferred::Buffer(b));
            self.retire(&mut st);
        }
    }

    fn destroy_texture(&self, id: TextureId) {
        let mut st = self.lock();
        if let Some(t) = st.textures.get(id.index(), id.generation())
            && t.surface
        {
            return; // Surface textures belong to the frame.
        }
        if let Some(t) = st.textures.remove(id.index(), id.generation()) {
            if let Some(slot) = t.bound {
                st.tables.tex_owner[slot as usize] = None;
                let after = Self::retire_after(&st);
                st.tables.tex_slots.free_after(slot, after);
            }
            self.defer(&mut st, Deferred::Texture(t));
            self.retire(&mut st);
        }
    }

    fn destroy_sampler(&self, id: SamplerId) {
        let mut st = self.lock();
        if let Some(s) = st.samplers.remove(id.index(), id.generation()) {
            if let Some(slot) = s.bound {
                st.tables.smp_owner[slot as usize] = None;
                let after = Self::retire_after(&st);
                st.tables.smp_slots.free_after(slot, after);
            }
            self.defer(&mut st, Deferred::Sampler(s));
            self.retire(&mut st);
        }
    }

    fn destroy_shader(&self, id: ShaderId) {
        self.lock().shaders.remove(id.index(), id.generation());
    }

    fn destroy_render_pipeline(&self, id: RenderPipelineId) {
        let mut st = self.lock();
        if let Some(p) = st.rpipes.remove(id.index(), id.generation()) {
            self.defer(&mut st, Deferred::RenderPipe(p));
        }
    }

    fn destroy_compute_pipeline(&self, id: ComputePipelineId) {
        let mut st = self.lock();
        if let Some(p) = st.cpipes.remove(id.index(), id.generation()) {
            self.defer(&mut st, Deferred::ComputePipe(p));
        }
    }

    fn bind_texture(&self, id: TextureId) -> Result<BindlessTexture> {
        let mut st = self.lock();
        let st = &mut *st;
        let t = st.textures.get_mut(id.index(), id.generation()).ok_or_else(|| invalid("bind_texture: stale texture"))?;
        if let Some(s) = t.bound {
            return Ok(BindlessTexture(s));
        }
        if t.surface {
            return Err(invalid("bind_texture: surface textures cannot be bound"));
        }
        if !(t.usage.shader_read || t.usage.shader_write || t.usage.depth) {
            return Err(invalid("bind_texture: texture has neither shader_read nor shader_write usage"));
        }
        let slot = st.tables.tex_slots.alloc().ok_or_else(|| invalid("bind_texture: texture table full"))?;
        t.bound = Some(slot);
        write_entry(&st.tables.textures, slot, t.raw.gpuResourceID().to_raw());
        if st.tables.tex_res.len() <= slot as usize {
            st.tables.tex_res.resize(slot as usize + 1, None);
            st.tables.tex_owner.resize(slot as usize + 1, None);
        }
        st.tables.tex_owner[slot as usize] = Some(id);
        st.residency.addAllocation(ProtocolObject::from_ref(&*t.raw));
        st.tables.tex_res[slot as usize] = Some(t.raw.clone());
        st.residency_dirty = true;
        Ok(BindlessTexture(slot))
    }

    fn bind_buffer(&self, id: BufferId) -> Result<BindlessBuffer> {
        let mut st = self.lock();
        let st = &mut *st;
        let b = st.buffers.get_mut(id.index(), id.generation()).ok_or_else(|| invalid("bind_buffer: stale buffer"))?;
        if let Some(s) = b.bound {
            return Ok(BindlessBuffer(s));
        }
        if !(b.usage.shader_read || b.usage.shader_write || b.usage.cpu_write) {
            return Err(invalid("bind_buffer: buffer has no shader usage"));
        }
        let slot = st.tables.buf_slots.alloc().ok_or_else(|| invalid("bind_buffer: buffer table full"))?;
        b.bound = Some(slot);
        for (f, table) in st.tables.buffers.iter().enumerate() {
            let raw = &b.raws[f % b.raws.len()];
            write_entry(table, slot, raw.gpuAddress());
        }
        for raw in &b.raws {
            st.residency.addAllocation(ProtocolObject::from_ref(&**raw));
        }
        if st.tables.buf_res.len() <= slot as usize {
            st.tables.buf_res.resize(slot as usize + 1, None);
            st.tables.buf_owner.resize(slot as usize + 1, None);
        }
        st.tables.buf_owner[slot as usize] = Some(id);
        st.tables.buf_res[slot as usize] = Some(b.raws.clone());
        st.residency_dirty = true;
        Ok(BindlessBuffer(slot))
    }

    fn bind_sampler(&self, id: SamplerId) -> Result<BindlessSampler> {
        let mut st = self.lock();
        let st = &mut *st;
        let s = st.samplers.get_mut(id.index(), id.generation()).ok_or_else(|| invalid("bind_sampler: stale sampler"))?;
        if let Some(i) = s.bound {
            return Ok(BindlessSampler(i));
        }
        let slot = st.tables.smp_slots.alloc().ok_or_else(|| invalid("bind_sampler: sampler table full"))?;
        s.bound = Some(slot);
        write_entry(&st.tables.samplers, slot, s.raw.gpuResourceID().to_raw());
        if st.tables.smp_res.len() <= slot as usize {
            st.tables.smp_res.resize(slot as usize + 1, None);
            st.tables.smp_owner.resize(slot as usize + 1, None);
        }
        st.tables.smp_owner[slot as usize] = Some(id);
        st.tables.smp_res[slot as usize] = Some(s.raw.clone());
        Ok(BindlessSampler(slot))
    }

    fn unbind_texture(&self, index: BindlessTexture) {
        let mut st = self.lock();
        let after = Self::retire_after(&st);
        let st = &mut *st;
        if let Some(id) = st.tables.tex_owner.get_mut(index.0 as usize).and_then(Option::take) {
            if let Some(t) = st.textures.get_mut(id.index(), id.generation()) {
                t.bound = None;
            }
            st.tables.tex_slots.free_after(index.0, after);
        }
        self.retire(st);
    }

    fn unbind_buffer(&self, index: BindlessBuffer) {
        let mut st = self.lock();
        let after = Self::retire_after(&st);
        let st = &mut *st;
        if let Some(id) = st.tables.buf_owner.get_mut(index.0 as usize).and_then(Option::take) {
            if let Some(b) = st.buffers.get_mut(id.index(), id.generation()) {
                b.bound = None;
            }
            st.tables.buf_slots.free_after(index.0, after);
        }
        self.retire(st);
    }

    fn unbind_sampler(&self, index: BindlessSampler) {
        let mut st = self.lock();
        let after = Self::retire_after(&st);
        let st = &mut *st;
        if let Some(id) = st.tables.smp_owner.get_mut(index.0 as usize).and_then(Option::take) {
            if let Some(s) = st.samplers.get_mut(id.index(), id.generation()) {
                s.bound = None;
            }
            st.tables.smp_slots.free_after(index.0, after);
        }
        self.retire(st);
    }

    fn write_buffer(&self, id: BufferId, offset: u64, data: &[u8]) -> Result<()> {
        if data.is_empty() {
            return Ok(());
        }
        {
            let st = self.lock();
            let b = st.buffers.get(id.index(), id.generation()).ok_or_else(|| invalid("write_buffer: stale buffer"))?;
            if !b.usage.copy_dst {
                return Err(invalid("write_buffer: buffer lacks copy_dst usage"));
            }
            if b.usage.cpu_write {
                return Err(invalid("write_buffer: use fill_frame_buffer for cpu_write buffers"));
            }
            if offset + data.len() as u64 > b.size || !offset.is_multiple_of(4) {
                return Err(invalid(format!("write_buffer: range {offset}+{} outside size {} or unaligned", data.len(), b.size)));
            }
        }
        let staging = self.staging(data)?;
        self.lock().uploads.push(Upload::Buffer { staging, dst: id, offset });
        Ok(())
    }

    fn write_texture(&self, id: TextureId, sub: Subresource, data: &[u8]) -> Result<()> {
        {
            let st = self.lock();
            let t = st.textures.get(id.index(), id.generation()).ok_or_else(|| invalid("write_texture: stale texture"))?;
            if !t.usage.copy_dst {
                return Err(invalid("write_texture: texture lacks copy_dst usage"));
            }
            if sub.mip >= t.mips || sub.layer >= t.layers() {
                return Err(invalid("write_texture: subresource out of range"));
            }
            let (w, h) = (mip_size(t.width, sub.mip), mip_size(t.height, sub.mip));
            let d = if t.kind == TextureKind::D3 { mip_size(t.depth(), sub.mip) } else { 1 };
            let need = t.format.packed_row_bytes(w) as usize * t.format.block_rows(h) as usize * d as usize;
            if data.len() != need {
                return Err(invalid(format!("write_texture: {} bytes given, subresource needs {need}", data.len())));
            }
        }
        let staging = self.staging(data)?;
        self.lock().uploads.push(Upload::Texture { staging, dst: id, sub });
        Ok(())
    }

    fn fill_frame_buffer(&self, id: BufferId, fill: &mut dyn FnMut(&mut [u8])) -> Result<()> {
        let (raw, size) = {
            let st = self.lock();
            if !st.frame_open {
                return Err(invalid("fill_frame_buffer: no frame is open"));
            }
            let b = st.buffers.get(id.index(), id.generation()).ok_or_else(|| invalid("fill_frame_buffer: stale buffer"))?;
            if !b.usage.cpu_write {
                return Err(invalid("fill_frame_buffer: buffer lacks cpu_write usage"));
            }
            let slot = ((st.frames_begun - 1) % self.frames as u64) as usize;
            (b.raws[slot].clone(), b.size as usize)
        };
        // SAFETY: shared storage of `size` bytes; this frame slot's previous frame has finished
        // (begin_frame waited), so the GPU is not reading it; the slice lives only for the call.
        let bytes = unsafe { std::slice::from_raw_parts_mut(raw.contents().as_ptr() as *mut u8, size) };
        fill(bytes);
        Ok(())
    }

    fn read_buffer(&self, id: BufferId, out: &mut Vec<u8>) -> Result<()> {
        let st = self.lock();
        let b = st.buffers.get(id.index(), id.generation()).ok_or_else(|| invalid("read_buffer: stale buffer"))?;
        if !b.usage.cpu_read {
            return Err(invalid("read_buffer: buffer lacks cpu_read usage"));
        }
        out.clear();
        // SAFETY: shared storage of `size` bytes, alive while the lock holds the buffer.
        let bytes = unsafe { std::slice::from_raw_parts(b.raws[0].contents().as_ptr() as *const u8, b.size as usize) };
        out.extend_from_slice(bytes);
        Ok(())
    }

    fn create_surface(&self, window: WindowHandle, config: &SurfaceConfig) -> Result<SurfaceId> {
        let obj = NonNull::new(window.window as *mut AnyObject).ok_or_else(|| invalid("create_surface: null window"))?;
        // SAFETY: the caller passes a live NSView* or CAMetalLayer* (WindowHandle contract).
        let obj: &AnyObject = unsafe { obj.as_ref() };
        // SAFETY: isKindOfClass: is defined on every NSObject.
        let is_layer: bool = unsafe { msg_send![obj, isKindOfClass: CAMetalLayer::class()] };
        let layer = if is_layer {
            // SAFETY: just checked the class; retaining keeps it alive for the surface.
            unsafe { Retained::retain(obj as *const AnyObject as *mut CAMetalLayer) }.ok_or_else(|| invalid("create_surface: null layer"))?
        } else {
            let layer = CAMetalLayer::new();
            // SAFETY: `obj` is an NSView (contract); these are main-thread NSView setters.
            unsafe {
                let _: () = msg_send![obj, setWantsLayer: true];
                let _: () = msg_send![obj, setLayer: &*layer];
            }
            layer
        };
        layer.setDevice(Some(&self.device));
        self.apply_surface_config(&layer, config)?;
        let (i, g) = self.lock().surfaces.insert(Surface { layer, config: config.clone() });
        Ok(SurfaceId::from_parts(i, g))
    }

    fn configure_surface(&self, surface: SurfaceId, config: &SurfaceConfig) -> Result<()> {
        let layer = {
            let st = self.lock();
            let s = st.surfaces.get(surface.index(), surface.generation()).ok_or_else(|| invalid("configure_surface: stale surface"))?;
            if st.frame_open && !st.acquired.is_empty() {
                return Err(invalid("configure_surface: reconfigure between frames"));
            }
            s.layer.clone()
        };
        self.apply_surface_config(&layer, config)?;
        if let Some(s) = self.lock().surfaces.get_mut(surface.index(), surface.generation()) {
            s.config = config.clone();
        }
        Ok(())
    }

    fn destroy_surface(&self, surface: SurfaceId) {
        let mut st = self.lock();
        st.surfaces.remove(surface.index(), surface.generation());
    }

    fn begin_frame(&self) -> Result<Box<dyn CommandEncoder + '_>> {
        // Pace: wait (without the lock) until fewer than `frames` frames are in flight.
        loop {
            let wait_on = {
                let mut st = self.lock();
                if st.frame_open {
                    return Err(invalid("begin_frame: the previous frame was not ended"));
                }
                self.retire(&mut st);
                if st.in_flight.len() < self.frames as usize {
                    break;
                }
                st.in_flight.front().and_then(|f| f.cbs.last().cloned())
            };
            if let Some(cb) = wait_on {
                cb.waitUntilCompleted();
            }
        }
        encoder::MetalEncoder::begin(self).map(|e| Box::new(e) as Box<dyn CommandEncoder + '_>)
    }

    fn acquire(&self, surface: SurfaceId) -> Result<TextureId> {
        let (layer, config) = {
            let st = self.lock();
            if !st.frame_open {
                return Err(invalid("acquire: no frame is open"));
            }
            let s = st.surfaces.get(surface.index(), surface.generation()).ok_or_else(|| invalid("acquire: stale surface"))?;
            (s.layer.clone(), s.config.clone())
        };
        let drawable = autoreleasepool(|_| layer.nextDrawable()).ok_or(Error::SurfaceLost)?;
        let raw = drawable.texture();
        if raw.width() as u32 != config.width || raw.height() as u32 != config.height {
            return Err(Error::SurfaceLost);
        }
        let tex = Texture {
            raw,
            kind: TextureKind::D2,
            format: config.format,
            width: config.width,
            height: config.height,
            depth_or_layers: 1,
            mips: 1,
            usage: Usage { render_target: true, copy_src: true, copy_dst: true, shader_read: true, ..Default::default() },
            states: States::new(1, 1),
            bound: None,
            surface: true,
        };
        let mut st = self.lock();
        let (i, g) = st.textures.insert(tex);
        let id = TextureId::from_parts(i, g);
        st.acquired.push((drawable, id));
        Ok(id)
    }

    fn end_frame(&self, encoder: Box<dyn CommandEncoder + '_>) -> Result<()> {
        encoder.finish()
    }

    fn wait_idle(&self) {
        let cbs: Vec<_> = self.lock().in_flight.iter().flat_map(|f| f.cbs.iter().cloned()).collect();
        for cb in cbs {
            cb.waitUntilCompleted();
        }
        let mut st = self.lock();
        self.retire(&mut st);
    }

    fn completed_frames(&self) -> u64 {
        let mut st = self.lock();
        self.retire(&mut st);
        st.completed
    }

    fn timings(&self, out: &mut Vec<Timing>) {
        let mut st = self.lock();
        self.retire(&mut st);
        out.clear();
        out.extend_from_slice(&st.timings);
    }

    fn allocated_bytes(&self) -> u64 {
        self.device.currentAllocatedSize() as u64
    }
}

impl MetalDevice {
    fn staging(&self, data: &[u8]) -> Result<P<dyn MTLBuffer>> {
        let b = shared_buffer(&self.device, data.len(), "ed_gpu upload staging")?;
        // SAFETY: the new shared buffer has at least `data.len()` bytes and no other user.
        unsafe { std::ptr::copy_nonoverlapping(data.as_ptr(), b.contents().as_ptr() as *mut u8, data.len()) };
        Ok(b)
    }

    fn apply_surface_config(&self, layer: &CAMetalLayer, c: &SurfaceConfig) -> Result<()> {
        if c.width == 0 || c.height == 0 {
            return Err(invalid("surface size 0"));
        }
        let ok = matches!(c.format, Format::Bgra8Srgb | Format::Bgra8Unorm | Format::Rgb10A2Unorm | Format::Rgba16Float);
        if !ok {
            return Err(invalid(format!("surface format {:?} not presentable", c.format)));
        }
        if c.hdr && c.format != Format::Rgba16Float {
            return Err(invalid("HDR surfaces need Rgba16Float"));
        }
        layer.setPixelFormat(conv::format(c.format));
        layer.setDrawableSize(CGSize { width: c.width as f64, height: c.height as f64 });
        layer.setDisplaySyncEnabled(c.vsync);
        layer.setMaximumDrawableCount(self.frames.clamp(2, 3) as usize);
        layer.setFramebufferOnly(false);
        layer.setWantsExtendedDynamicRangeContent(c.hdr);
        let space = if c.hdr {
            // SAFETY: reading an immutable CoreGraphics constant.
            objc2_core_graphics::CGColorSpace::with_name(Some(unsafe { objc2_core_graphics::kCGColorSpaceExtendedLinearSRGB }))
        } else {
            None
        };
        layer.setColorspace(space.as_deref());
        Ok(())
    }
}

#[allow(dead_code)]
fn _assert_send_sync() {
    fn f<T: Send + Sync>() {}
    f::<MetalDevice>();
}
