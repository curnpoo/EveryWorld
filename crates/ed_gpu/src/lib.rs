//! The GPU interface (RHI): device, resources with explicit states, bindless tables, command
//! encoding, timers, capability probe. Backends are private modules; no backend type appears
//! in this API.
//!
//! Allowed workspace dependencies: none. Enforced by `cargo xtask check-deps`.
//!
//! **Status: Metal backend (Phase 1, W2) and the minimal Vulkan backend (P1-D22, M4b-2).**
//! `create_device(Backend::Metal, ..)` works on macOS. `Backend::Vulkan` works when the crate
//! is built with the `vulkan` feature and a Vulkan loader is found (on macOS: Homebrew's
//! `vulkan-loader` + `molten-vk`; `ED_GPU_VULKAN_LIB` names the loader library explicitly).
//! D3D12 returns `Unsupported`. [`default_backend`] picks the backend at run time
//! (`ED_GPU_BACKEND=metal|vulkan|d3d12`). The contract, and the walk of every item
//! below against Metal, D3D12 and Vulkan, is `docs/contracts/rhi.md` (its last section lists
//! what the Metal bring-up changed). Change the contract and this file together.
//!
//! Shader-side binding ABI: see [`shader_abi`]. Errors found while recording commands
//! (stale handles, wrong states) are collected by the encoder and returned by
//! [`Device::end_frame`], so command methods stay infallible.
//!
//! Shape, in one paragraph: resources are plain generational handles. Every texture and
//! buffer use is declared as a [`ResourceState`], and the caller (the frame graph in
//! `ed_render`) issues transitions; Metal ignores them, D3D12 and Vulkan need them. All
//! textures and buffers that shaders read live in two device-owned bindless tables, so a
//! draw carries only a pipeline and a small block of constants. There are no vertex input
//! layouts: vertex shaders pull from bindless buffers. Shaders arrive precompiled per backend.

#![forbid(unsafe_op_in_unsafe_fn)]

use std::num::NonZeroU32;

mod pool;
pub mod shader_abi;
mod track;
#[cfg(target_os = "macos")]
mod metal;
#[cfg(feature = "vulkan")]
mod vulkan;

// ---------------------------------------------------------------------------------------------
// Handles
// ---------------------------------------------------------------------------------------------

macro_rules! handle {
    ($($(#[$m:meta])* $name:ident),* $(,)?) => {$(
        $(#[$m])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
        pub struct $name { index: u32, generation: NonZeroU32 }
        impl $name {
            /// For backends and tests only.
            pub fn from_parts(index: u32, generation: NonZeroU32) -> Self { Self { index, generation } }
            pub fn index(self) -> u32 { self.index }
            pub fn generation(self) -> NonZeroU32 { self.generation }
        }
    )*};
}
handle!(
    /// A GPU buffer.
    BufferId,
    /// A texture (any dimension).
    TextureId,
    SamplerId,
    ShaderId,
    RenderPipelineId,
    ComputePipelineId,
    /// A window surface with its swapchain.
    SurfaceId,
);

/// Index of a texture in the device's bindless texture table, as shaders see it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct BindlessTexture(pub u32);
/// Index of a buffer in the device's bindless buffer table, as shaders see it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct BindlessBuffer(pub u32);
/// Index of a sampler in the device's sampler table, as shaders see it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct BindlessSampler(pub u32);

// ---------------------------------------------------------------------------------------------
// Capabilities (P1-D10)
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Backend {
    Metal,
    Vulkan,
    D3D12,
}

/// Filled once at device creation. Every tiered renderer feature is gated by a field here,
/// never by a platform check.
#[derive(Clone, Debug)]
pub struct Caps {
    pub backend: Backend,
    pub device_name: String,
    /// Memory the device recommends staying under, in bytes.
    pub memory_budget: u64,
    pub unified_memory: bool,
    /// 1: bindless tables of limited size (Metal argument buffers tier 1, no descriptor
    /// indexing). 2: large tables, any resource indexed dynamically (Metal tier 2, Vulkan
    /// descriptor indexing, D3D12 resource binding tier 3).
    pub bindless_tier: u32,
    /// Sizes of the tables this device allocated (not the hardware maxima, which can be larger).
    pub max_bindless_textures: u32,
    pub max_bindless_buffers: u32,
    pub max_bindless_samplers: u32,
    pub max_texture_size: u32,
    /// Largest threads per workgroup, and workgroup shared memory in bytes.
    pub max_workgroup_threads: u32,
    pub max_workgroup_memory: u32,
    /// The backend honours the count buffer of [`CommandEncoder::draw_indexed_indirect_multi`].
    /// When false it draws `max_draws` entries, so unused entries must have zero instances.
    pub indirect_count: bool,
    /// `draw_indexed_indirect_multi` is one native command. When false the backend loops over
    /// the entries on the CPU (one indirect draw each), so its CPU cost grows with `max_draws`.
    pub native_multi_draw_indirect: bool,
    pub mesh_shaders: bool,
    pub ray_tracing: bool,
    /// 64-bit atomic operations on buffers in compute shaders (software rasteriser, Track G).
    pub atomics_64: bool,
    pub sparse_textures: bool,
    pub bc_textures: bool,
    pub astc_textures: bool,
    /// How [`CommandEncoder::begin_timer`] scopes are measured.
    pub timers: TimerSource,
    /// Surfaces can present extended-range (HDR) content.
    pub hdr_output: bool,
}

/// How the backend measures GPU timer scopes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TimerSource {
    /// No GPU timers; [`Device::timings`] stays empty.
    None,
    /// Timestamps at pass boundaries (Metal counter sample buffers, Vulkan and D3D12
    /// timestamp queries). Scopes measure the passes inside them.
    PassBoundary,
    /// The backend splits the frame into separate submissions at every scope boundary and
    /// reads each submission's GPU start and end time. Coarser, and adds submissions.
    Submission,
}

// ---------------------------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------------------------

/// How a resource is used at a point in the frame. One state at a time per subresource.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ResourceState {
    /// Contents undefined; the only legal source state for a first use, or to discard.
    Undefined,
    /// Read in any shader stage (sampled texture, read-only buffer, vertex pulling, indices).
    ShaderRead,
    /// Read and written in a compute or fragment shader.
    ShaderWrite,
    RenderTarget,
    DepthWrite,
    /// Depth test without write; may also be sampled.
    DepthRead,
    IndirectArgs,
    CopySrc,
    CopyDst,
    Present,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Format {
    Rgba8Unorm,
    Bgra8Unorm,
    Rgba8Srgb,
    Bgra8Srgb,
    Rg8Unorm,
    R8Unorm,
    R16Float,
    Rg16Float,
    Rgba16Float,
    R32Float,
    R32Uint,
    Rg32Uint,
    R11G11B10Float,
    Rgb10A2Unorm,
    Depth32Float,
    Bc4Unorm,
    Bc5Unorm,
    Bc6hUfloat,
    Bc7Unorm,
    Bc7Srgb,
    Astc4x4Unorm,
    Astc4x4Srgb,
    Astc6x6Unorm,
    Astc6x6Srgb,
}

impl Format {
    /// `(block width, block height, bytes per block)`. Uncompressed formats have 1x1 blocks.
    pub fn block(self) -> (u32, u32, u32) {
        use Format::*;
        match self {
            R8Unorm => (1, 1, 1),
            Rg8Unorm | R16Float => (1, 1, 2),
            Rgba8Unorm | Bgra8Unorm | Rgba8Srgb | Bgra8Srgb | Rg16Float | R32Float | R32Uint | R11G11B10Float
            | Rgb10A2Unorm | Depth32Float => (1, 1, 4),
            Rgba16Float | Rg32Uint => (1, 1, 8),
            Bc4Unorm => (4, 4, 8),
            Bc5Unorm | Bc6hUfloat | Bc7Unorm | Bc7Srgb | Astc4x4Unorm | Astc4x4Srgb => (4, 4, 16),
            Astc6x6Unorm | Astc6x6Srgb => (6, 6, 16),
        }
    }
    pub fn is_depth(self) -> bool {
        matches!(self, Format::Depth32Float)
    }
    /// Tightly packed bytes per row of blocks for a mip of `width` texels.
    pub fn packed_row_bytes(self, width: u32) -> u32 {
        let (bw, _, bytes) = self.block();
        width.div_ceil(bw) * bytes
    }
    /// Rows of blocks in a mip of `height` texels.
    pub fn block_rows(self, height: u32) -> u32 {
        height.div_ceil(self.block().1)
    }
    /// Row pitch to use with [`CommandEncoder::copy_texture_to_buffer`]: the packed row size
    /// rounded up to [`COPY_ROW_PITCH_ALIGN`].
    pub fn copy_row_pitch(self, width: u32) -> u32 {
        self.packed_row_bytes(width).next_multiple_of(COPY_ROW_PITCH_ALIGN)
    }
}

/// Texture-to-buffer copies use a row pitch that is a multiple of this (the D3D12 rule; the
/// strictest of the three APIs).
pub const COPY_ROW_PITCH_ALIGN: u32 = 256;

/// Size of mip `mip` of a dimension of `size` texels (never 0).
pub fn mip_size(size: u32, mip: u32) -> u32 {
    (size >> mip).max(1)
}

/// What a resource may ever be used as. Fixed at creation; states outside it are errors.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Usage {
    pub shader_read: bool,
    pub shader_write: bool,
    pub render_target: bool,
    pub depth: bool,
    pub indirect_args: bool,
    pub copy_src: bool,
    pub copy_dst: bool,
    /// The CPU writes it every frame through [`Device::fill_frame_buffer`].
    pub cpu_write: bool,
    /// The CPU reads it back (screenshots, picking, timers).
    pub cpu_read: bool,
}

#[derive(Clone, Debug)]
pub struct BufferDesc<'a> {
    pub label: &'a str,
    pub size: u64,
    pub usage: Usage,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TextureKind {
    D2,
    D2Array,
    Cube,
    D3,
}

#[derive(Clone, Debug)]
pub struct TextureDesc<'a> {
    pub label: &'a str,
    pub kind: TextureKind,
    pub format: Format,
    pub width: u32,
    pub height: u32,
    /// Depth for `D3`, layer count for `D2Array`, 6 for `Cube`, otherwise 1.
    pub depth_or_layers: u32,
    pub mips: u32,
    pub samples: u32,
    pub usage: Usage,
}

/// One mip of one layer, or the whole resource.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Subresource {
    pub mip: u32,
    pub layer: u32,
}

#[derive(Clone, Copy, Debug)]
pub enum Transition {
    Buffer { buffer: BufferId, from: ResourceState, to: ResourceState },
    /// `sub: None` transitions every mip and layer.
    Texture { texture: TextureId, sub: Option<Subresource>, from: ResourceState, to: ResourceState },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Filter {
    Nearest,
    Linear,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AddressMode {
    Repeat,
    Clamp,
    Mirror,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Compare {
    Never,
    Less,
    LessEqual,
    Equal,
    GreaterEqual,
    Greater,
    Always,
}

#[derive(Clone, Debug)]
pub struct SamplerDesc {
    pub min_mag: Filter,
    pub mip: Filter,
    pub address: AddressMode,
    pub anisotropy: u8,
    /// Comparison sampler for shadow maps.
    pub compare: Option<Compare>,
}

// ---------------------------------------------------------------------------------------------
// Shaders and pipelines
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ShaderStage {
    Vertex,
    Fragment,
    Compute,
}

/// Precompiled code for one backend. Slang produces all of these at build time (P1-D1); the
/// device rejects code for another backend (`Caps::backend` says which one to pass).
///
/// Metal takes `MslSource` or `MetalLib`; Vulkan takes `SpirV` (the entry point is
/// [`ShaderDesc::entry`] inside the module; the binding ABI is in [`shader_abi`]).
#[derive(Clone, Copy, Debug)]
pub enum ShaderCode<'a> {
    /// Metal shading language source: the escape hatch for hand-written shaders.
    MslSource(&'a str),
    MetalLib(&'a [u8]),
    SpirV(&'a [u32]),
    Dxil(&'a [u8]),
}

#[derive(Clone, Debug)]
pub struct ShaderDesc<'a> {
    pub label: &'a str,
    pub stage: ShaderStage,
    pub entry: &'a str,
    pub code: ShaderCode<'a>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Blend {
    Opaque,
    Alpha,
    Additive,
    Premultiplied,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cull {
    None,
    Back,
    Front,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Topology {
    Triangles,
    Lines,
    Points,
}

#[derive(Clone, Debug)]
pub struct DepthDesc {
    pub format: Format,
    pub write: bool,
    /// Reverse-Z (P1-D8): the usual value is `GreaterEqual`.
    pub compare: Compare,
    pub bias: f32,
    pub slope_bias: f32,
}

/// There is no vertex layout: vertex shaders read vertex data from bindless buffers by
/// `vertex_id` and the draw constants.
#[derive(Clone, Debug)]
pub struct RenderPipelineDesc<'a> {
    pub label: &'a str,
    pub vertex: ShaderId,
    /// `None` for depth-only passes.
    pub fragment: Option<ShaderId>,
    pub color_formats: &'a [Format],
    pub blend: Blend,
    pub depth: Option<DepthDesc>,
    pub cull: Cull,
    pub topology: Topology,
    pub samples: u32,
}

#[derive(Clone, Debug)]
pub struct ComputePipelineDesc<'a> {
    pub label: &'a str,
    pub shader: ShaderId,
    /// Threads per workgroup. Metal needs it at dispatch time; on Vulkan and D3D12 it must
    /// match the size compiled into the shader (`numthreads` / `local_size`).
    pub workgroup_size: [u32; 3],
}

// ---------------------------------------------------------------------------------------------
// Passes and commands
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
pub enum LoadOp {
    Load,
    Clear([f32; 4]),
    /// Contents are not needed; cheapest on tile-based GPUs.
    Discard,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StoreOp {
    Store,
    Discard,
}

#[derive(Clone, Copy, Debug)]
pub struct Attachment {
    pub texture: TextureId,
    pub sub: Subresource,
    pub load: LoadOp,
    pub store: StoreOp,
}

#[derive(Clone, Debug)]
/// Colour attachments must be in `RenderTarget`; the depth attachment in `DepthWrite` (or
/// `DepthRead` when the pipeline does not write depth). `LoadOp::Clear` on depth clears to
/// element 0 (0.0 for reverse-Z, P1-D8).
pub struct RenderPassDesc<'a> {
    pub label: &'a str,
    pub colors: &'a [Attachment],
    pub depth: Option<Attachment>,
}

/// Size of the per-draw constant block every pipeline receives (push constants on Vulkan,
/// root constants on D3D12, a small inline buffer on Metal).
pub const DRAW_CONSTANTS_BYTES: usize = 64;

/// Layout of one entry in an indirect argument buffer for indexed draws. Identical on all
/// three backends.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct DrawIndexedArgs {
    pub index_count: u32,
    pub instance_count: u32,
    pub first_index: u32,
    pub base_vertex: i32,
    pub first_instance: u32,
}

/// Records commands for one frame on one thread. Passes do not nest. A resource must be in
/// the state a command needs; the encoder never transitions on its own.
///
/// Transitions are also the only synchronisation: a resource written by one pass and read by
/// a later one needs a transition between the two passes (also `ShaderWrite` to
/// `ShaderWrite` inside a compute pass, which is a write-after-write barrier). Transitions are
/// not allowed inside a render pass. Copies and `generate_mips` happen outside passes.
///
/// Recording never fails on the spot: the first error is kept and returned by
/// [`Device::end_frame`].
///
/// Winding: counter-clockwise triangles (in clip space, Y up) are front faces.
pub trait CommandEncoder {
    /// Index of the frame being recorded (0, 1, 2, ...). Compare with
    /// [`Device::completed_frames`] to know when its results (readback) are available.
    fn frame_index(&self) -> u64;
    fn transition(&mut self, transitions: &[Transition]);

    fn begin_render_pass(&mut self, desc: &RenderPassDesc<'_>);
    fn end_render_pass(&mut self);
    fn set_viewport(&mut self, x: f32, y: f32, w: f32, h: f32);
    fn set_scissor(&mut self, x: u32, y: u32, w: u32, h: u32);
    fn set_render_pipeline(&mut self, pipeline: RenderPipelineId);
    /// 32-bit indices only.
    fn set_index_buffer(&mut self, buffer: BufferId, offset: u64);
    fn set_draw_constants(&mut self, data: &[u8; DRAW_CONSTANTS_BYTES]);
    fn draw(&mut self, vertices: std::ops::Range<u32>, instances: std::ops::Range<u32>);
    fn draw_indexed(&mut self, indices: std::ops::Range<u32>, base_vertex: i32, instances: std::ops::Range<u32>);
    /// Draws up to `max_draws` [`DrawIndexedArgs`] from `args`. `count` names a `u32` holding
    /// the number of valid entries. Entries past the count must have `instance_count == 0`,
    /// because a backend with `Caps::indirect_count == false` draws all `max_draws`.
    /// `args` and `count` must be in `IndirectArgs`; the index buffer in `ShaderRead`.
    fn draw_indexed_indirect_multi(&mut self, args: BufferId, args_offset: u64, max_draws: u32, count: Option<(BufferId, u64)>);

    fn begin_compute_pass(&mut self, label: &str);
    fn end_compute_pass(&mut self);
    fn set_compute_pipeline(&mut self, pipeline: ComputePipelineId);
    fn set_compute_constants(&mut self, data: &[u8; DRAW_CONSTANTS_BYTES]);
    fn dispatch(&mut self, x: u32, y: u32, z: u32);
    /// `args` holds three `u32` group counts at `offset`.
    fn dispatch_indirect(&mut self, args: BufferId, offset: u64);

    fn copy_buffer(&mut self, src: BufferId, src_offset: u64, dst: BufferId, dst_offset: u64, size: u64);
    fn copy_texture(&mut self, src: TextureId, src_sub: Subresource, dst: TextureId, dst_sub: Subresource);
    /// Copies one whole subresource. Rows are `bytes_per_row` apart in the buffer, a multiple
    /// of [`COPY_ROW_PITCH_ALIGN`] (see [`Format::copy_row_pitch`]).
    fn copy_texture_to_buffer(&mut self, src: TextureId, sub: Subresource, dst: BufferId, dst_offset: u64, bytes_per_row: u32);
    /// Fills every mip below 0 from mip 0. Every subresource must be in `CopyDst` before and
    /// is in `CopyDst` after.
    fn generate_mips(&mut self, texture: TextureId);

    /// Named GPU timing scope around whole passes (not allowed inside a pass). Scopes nest.
    /// Results arrive through [`Device::timings`].
    fn begin_timer(&mut self, label: &'static str);
    fn end_timer(&mut self);
    fn debug_group(&mut self, label: &str);
    fn end_debug_group(&mut self);

    /// Submits the frame. Called by [`Device::end_frame`]; do not call it directly.
    #[doc(hidden)]
    fn finish(self: Box<Self>) -> Result<()>;
}

// ---------------------------------------------------------------------------------------------
// Device
// ---------------------------------------------------------------------------------------------

#[derive(Debug)]
pub enum Error {
    /// The backend cannot run on this machine at all.
    Unsupported(String),
    OutOfMemory,
    /// A handle was stale, or a resource was used outside its declared [`Usage`] or state.
    Invalid(String),
    /// The window surface changed size or was lost; reconfigure it.
    SurfaceLost,
    /// The device was removed or reset; every handle is dead.
    DeviceLost,
    Backend(String),
}
pub type Result<T> = std::result::Result<T, Error>;

/// A native window, passed through untouched. Each backend documents which fields it reads.
///
/// Metal reads `window` only: a `CAMetalLayer*`, or an `NSView*` that gets a new
/// `CAMetalLayer` as its layer (call `create_surface` on the main thread in that case).
#[derive(Clone, Copy, Debug)]
pub struct WindowHandle {
    /// `NSView*` or `CAMetalLayer*` on Apple platforms, `HWND` on Windows, surface or window
    /// pointer elsewhere.
    pub window: *mut std::ffi::c_void,
    /// `HINSTANCE`, display or connection, where the platform needs one.
    pub display: *mut std::ffi::c_void,
}

#[derive(Clone, Debug)]
pub struct SurfaceConfig {
    pub width: u32,
    pub height: u32,
    /// `Bgra8Srgb`/`Bgra8Unorm`/`Rgb10A2Unorm` for SDR, `Rgba16Float` for HDR.
    pub format: Format,
    pub vsync: bool,
    /// Extended-range output (needs `Caps::hdr_output` and an `Rgba16Float` format).
    pub hdr: bool,
}

/// One timer scope from a finished frame.
#[derive(Clone, Debug)]
pub struct Timing {
    pub label: &'static str,
    pub depth: u32,
    pub gpu_ms: f32,
}

#[derive(Clone, Debug)]
pub struct DeviceDesc {
    /// Enable the backend's validation (Metal API validation, Vulkan validation layers, the
    /// D3D12 debug layer). On in tests and debug builds.
    pub validation: bool,
    /// Frames the CPU may record ahead of the GPU: 2 or 3 (P1-D4).
    pub frames_in_flight: u32,
}

/// The device. One per process. Resource creation and destruction may be called from any
/// thread; frame functions from the render thread only.
pub trait Device: Send + Sync {
    fn caps(&self) -> &Caps;

    fn create_buffer(&self, desc: &BufferDesc<'_>) -> Result<BufferId>;
    fn create_texture(&self, desc: &TextureDesc<'_>) -> Result<TextureId>;
    fn create_sampler(&self, desc: &SamplerDesc) -> Result<SamplerId>;
    fn create_shader(&self, desc: &ShaderDesc<'_>) -> Result<ShaderId>;
    fn create_render_pipeline(&self, desc: &RenderPipelineDesc<'_>) -> Result<RenderPipelineId>;
    fn create_compute_pipeline(&self, desc: &ComputePipelineDesc<'_>) -> Result<ComputePipelineId>;

    /// Destruction is deferred until no frame in flight can still use the resource. A
    /// destroyed resource that is still in a bindless table is removed from it.
    fn destroy_buffer(&self, id: BufferId);
    fn destroy_texture(&self, id: TextureId);
    fn destroy_sampler(&self, id: SamplerId);
    /// Pipelines keep what they need; the shader can go once its pipelines exist.
    fn destroy_shader(&self, id: ShaderId);
    fn destroy_render_pipeline(&self, id: RenderPipelineId);
    fn destroy_compute_pipeline(&self, id: ComputePipelineId);

    /// Put a resource in the bindless table. The index is stable until removed and is what
    /// instance and material data store. Binding a bound resource returns its index. An index
    /// is not reused until every frame that could still read it has finished. Samplers live in
    /// a third, small table (`Caps::max_bindless_samplers`).
    fn bind_texture(&self, id: TextureId) -> Result<BindlessTexture>;
    fn bind_buffer(&self, id: BufferId) -> Result<BindlessBuffer>;
    fn bind_sampler(&self, id: SamplerId) -> Result<BindlessSampler>;
    fn unbind_texture(&self, index: BindlessTexture);
    fn unbind_buffer(&self, index: BindlessBuffer);
    fn unbind_sampler(&self, index: BindlessSampler);

    /// Staged uploads. Data is copied; the write lands before the next frame's commands. The
    /// destination must be in `CopyDst` or `Undefined` at that point; `Undefined` becomes
    /// `CopyDst`. Budgeted by the caller (P1-D5). Not for `cpu_write` buffers.
    fn write_buffer(&self, id: BufferId, offset: u64, data: &[u8]) -> Result<()>;
    /// `data` is one whole subresource, rows of blocks tightly packed
    /// ([`Format::packed_row_bytes`]); for `D3` textures every depth slice, one after another.
    fn write_texture(&self, id: TextureId, sub: Subresource, data: &[u8]) -> Result<()>;
    /// Fill this frame's copy of a `cpu_write` buffer. The slice is write-only memory and is
    /// valid only inside the call. Only between `begin_frame` and `end_frame`.
    fn fill_frame_buffer(&self, id: BufferId, fill: &mut dyn FnMut(&mut [u8])) -> Result<()>;
    /// Contents of a `cpu_read` buffer. The data is complete once the frame that wrote it has
    /// finished: wait until `completed_frames() > frame_index` of that frame (or `wait_idle`).
    fn read_buffer(&self, id: BufferId, out: &mut Vec<u8>) -> Result<()>;

    fn create_surface(&self, window: WindowHandle, config: &SurfaceConfig) -> Result<SurfaceId>;
    fn configure_surface(&self, surface: SurfaceId, config: &SurfaceConfig) -> Result<()>;
    fn destroy_surface(&self, surface: SurfaceId);

    /// Begin a frame. Blocks if `frames_in_flight` frames are still on the GPU.
    fn begin_frame(&self) -> Result<Box<dyn CommandEncoder + '_>>;
    /// The surface's texture for this frame, in `Undefined`. Transition to `Present` before
    /// `end_frame`.
    fn acquire(&self, surface: SurfaceId) -> Result<TextureId>;
    /// Submit, and present every surface acquired this frame.
    fn end_frame(&self, encoder: Box<dyn CommandEncoder + '_>) -> Result<()>;
    /// Block until the GPU has finished all submitted work (screenshots, tests, shutdown).
    fn wait_idle(&self);
    /// Number of frames the GPU has finished (frame `i` is done when this is `> i`).
    fn completed_frames(&self) -> u64;

    /// GPU timings of the most recent frame whose results are back (a few frames late).
    fn timings(&self, out: &mut Vec<Timing>);
    /// GPU memory this device has allocated now, in bytes (leak checks, budgets).
    fn allocated_bytes(&self) -> u64;
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unsupported(s) => write!(f, "unsupported: {s}"),
            Error::OutOfMemory => write!(f, "out of GPU memory"),
            Error::Invalid(s) => write!(f, "invalid use: {s}"),
            Error::SurfaceLost => write!(f, "surface lost"),
            Error::DeviceLost => write!(f, "device lost"),
            Error::Backend(s) => write!(f, "backend error: {s}"),
        }
    }
}
impl std::error::Error for Error {}

/// Create the device for a backend. Returns `Unsupported` for backends not compiled in or not
/// available on this machine.
pub fn create_device(backend: Backend, desc: &DeviceDesc) -> Result<Box<dyn Device>> {
    if !(2..=3).contains(&desc.frames_in_flight) {
        return Err(Error::Invalid(format!("frames_in_flight must be 2 or 3, got {}", desc.frames_in_flight)));
    }
    match backend {
        #[cfg(target_os = "macos")]
        Backend::Metal => Ok(Box::new(metal::MetalDevice::new(desc)?)),
        #[cfg(feature = "vulkan")]
        Backend::Vulkan => Ok(Box::new(vulkan::VulkanDevice::new(desc)?)),
        _ => Err(Error::Unsupported(format!("{backend:?}: backend not available in this build"))),
    }
}

/// The backend to use on this machine: `ED_GPU_BACKEND` (`metal`, `vulkan` or `d3d12`, any
/// case) when set, otherwise the platform's native one (Metal on Apple platforms, D3D12 on
/// Windows, Vulkan elsewhere). A named backend that is not compiled in still comes back here;
/// `create_device` then returns `Unsupported`.
pub fn default_backend() -> Backend {
    match std::env::var("ED_GPU_BACKEND").map(|v| v.to_ascii_lowercase()).as_deref() {
        Ok("metal") => Backend::Metal,
        Ok("vulkan") | Ok("vk") => Backend::Vulkan,
        Ok("d3d12") | Ok("dx12") => Backend::D3D12,
        _ if cfg!(any(target_os = "macos", target_os = "ios")) => Backend::Metal,
        _ if cfg!(windows) => Backend::D3D12,
        _ => Backend::Vulkan,
    }
}
