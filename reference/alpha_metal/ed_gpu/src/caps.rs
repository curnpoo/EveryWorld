//! Backend capabilities. `ed_render` asks `Caps`, never the backend type.

/// Which backend produced these caps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendKind {
    Wgpu,
    Metal,
    Vulkan,
    D3D12,
}

/// Features with a portable fallback must be checked here, never assumed (m2.md rule 4).
#[derive(Clone, Debug)]
pub struct Caps {
    pub backend: BackendKind,
    /// Timestamps on render/compute pass descriptors (works on Metal via wgpu).
    pub pass_timestamps: bool,
    /// GPU-written indirect draw lists (multi-draw-indirect or ICB).
    pub indirect_draw: bool,
    /// Count read from a GPU buffer (`multi_draw_indirect_count`); false means draw a fixed
    /// maximum with zeroed entries.
    pub indirect_count: bool,
    /// Bindless textures/buffers (binding arrays or argument buffers).
    pub bindless: bool,
    /// 64-bit atomics (software rasterised small triangles).
    pub atomics_64: bool,
    /// Mesh/task shaders (hardware clusters).
    pub mesh_shaders: bool,
    /// Hardware ray tracing (RTX path, Windows).
    pub ray_tracing: bool,
    /// Tile memory / programmable blending (Metal, Apple GPUs).
    pub tile_memory: bool,
    /// Max size of a single buffer in bytes.
    pub max_buffer: u64,
}

impl Caps {
    /// The conservative floor every backend meets; used by tests and as a fallback.
    pub fn minimum(backend: BackendKind) -> Self {
        Caps {
            backend,
            pass_timestamps: false,
            indirect_draw: false,
            indirect_count: false,
            bindless: false,
            atomics_64: false,
            mesh_shaders: false,
            ray_tracing: false,
            tile_memory: false,
            max_buffer: 128 << 20,
        }
    }
}
