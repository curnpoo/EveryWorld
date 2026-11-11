//! Shader sources and hot reload (M3). The renderer compiles every pipeline from a
//! [`ShaderSources`]: the built-in `shaders/*.metal` (embedded at build time) or the same
//! files loaded from a directory. [`crate::Renderer::reload_shaders`] compiles a complete new
//! set first and swaps it in only when every pipeline built, so a broken edit leaves the
//! running pipelines untouched and reports the compiler error.

use ed_gpu::{ComputePipelineDesc, ComputePipelineId, Device, RenderPipelineDesc, RenderPipelineId, ShaderCode, ShaderDesc, ShaderId, ShaderStage};
use std::path::Path;

/// Every MSL source the renderer compiles. Each is prefixed with `ed_gpu::shader_abi::MSL_PRELUDE`
/// and `common`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShaderSources {
    pub common: String,
    pub resolve: String,
    pub tonemap: String,
    pub debug: String,
    pub skin: String,
    pub cull: String,
    pub hzb: String,
    pub vis: String,
}

impl ShaderSources {
    /// File names in a shader directory, in field order.
    pub const FILES: [&'static str; 8] = ["common.metal", "resolve.metal", "tonemap.metal", "debug.metal", "skin.metal", "cull.metal", "hzb.metal", "visibility.metal"];

    /// The sources compiled into the crate.
    pub fn builtin() -> Self {
        Self {
            common: include_str!("../shaders/common.metal").into(),
            resolve: include_str!("../shaders/resolve.metal").into(),
            tonemap: include_str!("../shaders/tonemap.metal").into(),
            debug: include_str!("../shaders/debug.metal").into(),
            skin: include_str!("../shaders/skin.metal").into(),
            cull: include_str!("../shaders/cull.metal").into(),
            hzb: include_str!("../shaders/hzb.metal").into(),
            vis: include_str!("../shaders/visibility.metal").into(),
        }
    }

    /// Loads [`Self::FILES`] from `dir` (for hot reload, normally `crates/ed_render/shaders`).
    pub fn load(dir: &Path) -> std::io::Result<Self> {
        let r = |f: &str| std::fs::read_to_string(dir.join(f));
        Ok(Self { common: r("common.metal")?, resolve: r("resolve.metal")?, tonemap: r("tonemap.metal")?, debug: r("debug.metal")?, skin: r("skin.metal")?, cull: r("cull.metal")?, hzb: r("hzb.metal")?, vis: r("visibility.metal")? })
    }

    /// The crate's own shader directory (source checkout), for development hot reload.
    pub fn source_dir() -> &'static Path {
        Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/shaders"))
    }
}

/// GPU objects created while building a pipeline set: destroyed together on failure or swap.
#[derive(Default)]
pub(crate) struct Built {
    shaders: Vec<ShaderId>,
    compute: Vec<ComputePipelineId>,
    render: Vec<RenderPipelineId>,
}

impl Built {
    pub fn shader(&mut self, dev: &dyn Device, src: &ShaderSources, body: &str, label: &str, stage: ShaderStage, entry: &str) -> Result<ShaderId, ed_gpu::Error> {
        let code = format!("{}\n{}\n{}", ed_gpu::shader_abi::MSL_PRELUDE, src.common, body);
        let s = dev.create_shader(&ShaderDesc { label, stage, entry, code: ShaderCode::MslSource(&code) })?;
        self.shaders.push(s);
        Ok(s)
    }
    pub fn compute(&mut self, dev: &dyn Device, src: &ShaderSources, body: &str, entry: &str, group: [u32; 3]) -> Result<ComputePipelineId, ed_gpu::Error> {
        let s = self.shader(dev, src, body, entry, ShaderStage::Compute, entry)?;
        let p = dev.create_compute_pipeline(&ComputePipelineDesc { label: entry, shader: s, workgroup_size: group })?;
        self.compute.push(p);
        Ok(p)
    }
    pub fn render(&mut self, dev: &dyn Device, d: &RenderPipelineDesc) -> Result<RenderPipelineId, ed_gpu::Error> {
        let p = dev.create_render_pipeline(d)?;
        self.render.push(p);
        Ok(p)
    }
    pub fn destroy(self, dev: &dyn Device) {
        for p in self.compute {
            dev.destroy_compute_pipeline(p);
        }
        for p in self.render {
            dev.destroy_render_pipeline(p);
        }
        for s in self.shaders {
            dev.destroy_shader(s);
        }
    }
}
