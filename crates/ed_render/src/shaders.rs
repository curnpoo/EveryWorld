//! Shader sources and hot reload (M3, M4c). Every renderer shader is one Slang module in
//! `crates/ed_render/shaders/*.slang` (P1-D1), compiled by `tools/shaders/build.sh` to MSL
//! (`shaders/gen/msl/<module>.metal`) and SPIR-V (`shaders/gen/spv/<module>.<entry>.spv`).
//! The outputs are checked in and embedded, so a build needs no shader compiler; the
//! `ed_gpu` test `slang_fresh` fails when they are stale. The renderer picks MSL or SPIR-V by
//! the device's backend.
//!
//! [`ShaderSources`] holds the Slang text. The built-in sources use the embedded outputs; any
//! other sources (an edited copy, or [`ShaderSources::load`] of a shader directory for hot
//! reload) are compiled at run time with slangc (`ed_gpu::slang`), which must then be
//! installed. [`crate::Renderer::reload_shaders`] compiles a complete new set first and swaps
//! it in only when every pipeline built, so a broken edit leaves the running pipelines
//! untouched and reports the compiler error.

use ed_gpu::{
    Backend, ComputePipelineDesc, ComputePipelineId, Device, RenderPipelineDesc, RenderPipelineId,
    ShaderCode, ShaderDesc, ShaderId, ShaderStage,
};
use std::borrow::Cow;
use std::path::{Path, PathBuf};

/// The Slang source of every renderer module. `common` and `lighting` are included by the
/// others, and `abi` is ed_gpu's binding ABI (`shader_abi.slang`), which every module includes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShaderSources {
    pub abi: String,
    pub common: String,
    pub lighting: String,
    pub ao: String,
    pub resolve: String,
    pub tonemap: String,
    pub debug: String,
    pub skin: String,
    pub cull: String,
    pub hzb: String,
    pub vis: String,
    pub overlay: String,
}

/// A renderer shader module (one `.slang` file with entry points).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Module {
    Resolve,
    Tonemap,
    Debug,
    Skin,
    Cull,
    Hzb,
    Vis,
    Overlay,
    Ao,
}

impl Module {
    const ALL: [Module; 9] = [
        Module::Resolve,
        Module::Tonemap,
        Module::Debug,
        Module::Skin,
        Module::Cull,
        Module::Hzb,
        Module::Vis,
        Module::Overlay,
        Module::Ao,
    ];

    /// File stem in `shaders/` (and of the generated outputs).
    fn stem(self) -> &'static str {
        match self {
            Module::Resolve => "resolve",
            Module::Tonemap => "tonemap",
            Module::Debug => "debug",
            Module::Skin => "skin",
            Module::Cull => "cull",
            Module::Hzb => "hzb",
            Module::Vis => "visibility",
            Module::Overlay => "overlay",
            Module::Ao => "ao",
        }
    }

    fn source(self, s: &ShaderSources) -> &str {
        match self {
            Module::Resolve => &s.resolve,
            Module::Tonemap => &s.tonemap,
            Module::Debug => &s.debug,
            Module::Skin => &s.skin,
            Module::Cull => &s.cull,
            Module::Hzb => &s.hzb,
            Module::Vis => &s.vis,
            Module::Overlay => &s.overlay,
            Module::Ao => &s.ao,
        }
    }
}

impl ShaderSources {
    /// File names in a shader directory, in field order (after `abi`, which lives in ed_gpu:
    /// see [`Self::abi_path`]).
    pub const FILES: [&'static str; 11] = [
        "common.slang",
        "lighting.slang",
        "ao.slang",
        "resolve.slang",
        "tonemap.slang",
        "debug.slang",
        "skin.slang",
        "cull.slang",
        "hzb.slang",
        "visibility.slang",
        "overlay.slang",
    ];

    /// The sources compiled into the crate.
    pub fn builtin() -> Self {
        Self {
            abi: ed_gpu::slang::ABI_SOURCE.into(),
            common: include_str!("../shaders/common.slang").into(),
            lighting: include_str!("../shaders/lighting.slang").into(),
            ao: include_str!("../shaders/ao.slang").into(),
            resolve: include_str!("../shaders/resolve.slang").into(),
            tonemap: include_str!("../shaders/tonemap.slang").into(),
            debug: include_str!("../shaders/debug.slang").into(),
            skin: include_str!("../shaders/skin.slang").into(),
            cull: include_str!("../shaders/cull.slang").into(),
            hzb: include_str!("../shaders/hzb.slang").into(),
            vis: include_str!("../shaders/visibility.slang").into(),
            overlay: include_str!("../shaders/overlay.slang").into(),
        }
    }

    /// Loads [`Self::FILES`] from `dir` (for hot reload, normally `crates/ed_render/shaders`)
    /// and the binding ABI from [`Self::abi_path`] (the built-in ABI when there is none).
    pub fn load(dir: &Path) -> std::io::Result<Self> {
        let r = |f: &str| std::fs::read_to_string(dir.join(f));
        let abi = match Self::abi_path(dir) {
            Some(p) => std::fs::read_to_string(p)?,
            None => ed_gpu::slang::ABI_SOURCE.into(),
        };
        Ok(Self {
            abi,
            common: r("common.slang")?,
            lighting: r("lighting.slang")?,
            ao: r("ao.slang")?,
            resolve: r("resolve.slang")?,
            tonemap: r("tonemap.slang")?,
            debug: r("debug.slang")?,
            skin: r("skin.slang")?,
            cull: r("cull.slang")?,
            hzb: r("hzb.slang")?,
            vis: r("visibility.slang")?,
            overlay: r("overlay.slang")?,
        })
    }

    /// The binding ABI a shader directory compiles against: `dir/shader_abi.slang` when
    /// present, else ed_gpu's source next to a source checkout
    /// (`dir/../../ed_gpu/src/shader_abi.slang`), else none (the built-in ABI).
    pub fn abi_path(dir: &Path) -> Option<PathBuf> {
        [dir.join("shader_abi.slang"), dir.join("../../ed_gpu/src/shader_abi.slang")].into_iter().find(|p| p.is_file())
    }

    /// Every file a change of which should reload the shaders of `dir`: [`Self::FILES`] and
    /// the ABI ([`Self::abi_path`]).
    pub fn watched_files(dir: &Path) -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = Self::FILES.iter().map(|f| dir.join(f)).collect();
        v.extend(Self::abi_path(dir));
        v
    }

    /// The crate's own shader directory (source checkout), for development hot reload.
    pub fn source_dir() -> &'static Path {
        Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/shaders"))
    }
}

/// Compiled modules: MSL text per module, SPIR-V words per entry point.
pub(crate) struct ShaderSet {
    msl: Vec<(Module, Cow<'static, str>)>,
    spirv: Vec<(Module, String, Vec<u32>)>,
}

macro_rules! builtin_set {
    ($( $module:ident $stem:literal [$($entry:literal),*] ),* $(,)?) => {
        fn builtin_set() -> ShaderSet {
            ShaderSet {
                msl: vec![$( (Module::$module, Cow::Borrowed(include_str!(concat!("../shaders/gen/msl/", $stem, ".metal")))) ),*],
                spirv: vec![$($( (Module::$module, $entry.to_string(), words(include_bytes!(concat!("../shaders/gen/spv/", $stem, ".", $entry, ".spv")))) ),*),*],
            }
        }
    };
}

// Every entry point of every module (a missing generated file fails the build).
builtin_set! {
    Resolve "resolve" ["vs_resolve", "fs_resolve"],
    Tonemap "tonemap" ["vs_fullscreen", "fs_tonemap"],
    Debug "debug" ["pick", "pop_stats"],
    Skin "skin" ["skin"],
    Cull "cull" ["cull_reset", "cull_early", "cull_late", "shadow_reset", "shadow_cull"],
    Hzb "hzb" ["hzb_first", "hzb_down"],
    Vis "visibility" ["vs_vis", "fs_vis", "fs_vis_dither", "vs_shadow"],
    Overlay "overlay" ["vs_overlay", "fs_overlay"],
    Ao "ao" ["ao_main"],
}

fn words(bytes: &[u8]) -> Vec<u32> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_le_bytes(*c))
        .collect()
}

impl ShaderSet {
    /// The embedded outputs for [`ShaderSources::builtin`]; anything else is compiled with
    /// slangc (an error when no slangc of the pinned version is installed).
    pub fn for_sources(src: &ShaderSources) -> Result<Self, ed_gpu::Error> {
        if *src == ShaderSources::builtin() {
            return Ok(builtin_set());
        }
        let slangc = ed_gpu::slang::find_slangc().map_err(|e| {
            ed_gpu::Error::Invalid(format!("compiling edited shaders needs slangc: {e}"))
        })?;
        let mut set = ShaderSet {
            msl: Vec::new(),
            spirv: Vec::new(),
        };
        for m in Module::ALL {
            let file = format!("{}.slang", m.stem());
            // `shader_abi.slang` here overrides the built-in copy compile() writes first.
            let o = ed_gpu::slang::compile(
                &slangc,
                &[
                    (&file, m.source(src)),
                    ("common.slang", &src.common),
                    ("lighting.slang", &src.lighting),
                    ("shader_abi.slang", &src.abi),
                ],
            )
            .map_err(ed_gpu::Error::Invalid)?;
            set.msl.push((m, Cow::Owned(o.msl)));
            set.spirv
                .extend(o.spirv.into_iter().map(|(e, b)| (m, e, words(&b))));
        }
        Ok(set)
    }

    fn code(
        &self,
        backend: Backend,
        module: Module,
        entry: &str,
    ) -> Result<ShaderCode<'_>, ed_gpu::Error> {
        let missing =
            || ed_gpu::Error::Invalid(format!("no entry point {entry} in {}.slang", module.stem()));
        Ok(match backend {
            Backend::Vulkan => ShaderCode::SpirV(
                self.spirv
                    .iter()
                    .find(|(m, e, _)| *m == module && e == entry)
                    .map(|(_, _, w)| w.as_slice())
                    .ok_or_else(missing)?,
            ),
            _ => ShaderCode::MslSource(
                self.msl
                    .iter()
                    .find(|(m, _)| *m == module)
                    .map(|(_, s)| s.as_ref())
                    .ok_or_else(missing)?,
            ),
        })
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
    pub fn shader(
        &mut self,
        dev: &dyn Device,
        set: &ShaderSet,
        module: Module,
        label: &str,
        stage: ShaderStage,
        entry: &str,
    ) -> Result<ShaderId, ed_gpu::Error> {
        let code = set.code(dev.caps().backend, module, entry)?;
        let s = dev.create_shader(&ShaderDesc {
            label,
            stage,
            entry,
            code,
        })?;
        self.shaders.push(s);
        Ok(s)
    }
    pub fn compute(
        &mut self,
        dev: &dyn Device,
        set: &ShaderSet,
        module: Module,
        entry: &str,
        group: [u32; 3],
    ) -> Result<ComputePipelineId, ed_gpu::Error> {
        let s = self.shader(dev, set, module, entry, ShaderStage::Compute, entry)?;
        let p = dev.create_compute_pipeline(&ComputePipelineDesc {
            label: entry,
            shader: s,
            workgroup_size: group,
        })?;
        self.compute.push(p);
        Ok(p)
    }
    pub fn render(
        &mut self,
        dev: &dyn Device,
        d: &RenderPipelineDesc,
    ) -> Result<RenderPipelineId, ed_gpu::Error> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hot_reload_watches_the_abi() {
        let w = ShaderSources::watched_files(ShaderSources::source_dir());
        let abi = w.iter().find(|p| p.ends_with("shader_abi.slang")).expect("ABI watched");
        assert_eq!(std::fs::read_to_string(abi).unwrap(), ed_gpu::slang::ABI_SOURCE);
        assert!(ShaderSources::FILES.iter().all(|f| w.iter().any(|p| p.ends_with(f))));
    }

    #[test]
    fn source_dir_matches_builtin_and_every_entry_is_embedded() {
        let src = ShaderSources::builtin();
        assert_eq!(
            ShaderSources::load(ShaderSources::source_dir()).unwrap(),
            src
        );
        let set = builtin_set();
        for m in Module::ALL {
            for (entry, _) in ed_gpu::slang::entry_points(m.source(&src)) {
                assert!(
                    set.code(Backend::Vulkan, m, &entry).is_ok(),
                    "{entry} of {m:?} not embedded"
                );
            }
        }
    }
}
