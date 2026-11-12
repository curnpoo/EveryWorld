//! Hot reload (M3): `.metal` shaders and packs, polled by file stamp (no watcher thread).
//!
//! - Shaders: [`ed_render::shaders::ShaderSources::FILES`] in the renderer's source folder (or
//!   `EW_SHADER_DIR`). A change rebuilds every pipeline; a compile error is printed and the
//!   old pipelines keep drawing.
//! - Packs: [`crate::content::Content::poll_reload`] (manifest diff, only changed chunks).
//!   `ew-bake watch` rebuilds a pack when its sources change, so editing a Blender export or
//!   a material TOML reaches the running app in two hops.

use crate::content::{Content, LoadStats};
use ed_gpu::Device;
use ed_render::Renderer;
use ed_render::shaders::ShaderSources;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

/// Seconds between polls.
pub const POLL: Duration = Duration::from_millis(400);

fn stamps(files: &[PathBuf]) -> Vec<Option<SystemTime>> {
    files.iter().map(|f| std::fs::metadata(f).and_then(|m| m.modified()).ok()).collect()
}

/// What a poll did.
#[derive(Debug, Default)]
pub struct HotEvents {
    pub shaders: Option<Result<(), String>>,
    pub packs: Option<Result<LoadStats, String>>,
}

pub struct HotReload {
    shader_dir: PathBuf,
    shader_files: Vec<PathBuf>,
    shader_stamps: Vec<Option<SystemTime>>,
    last: Instant,
}

impl HotReload {
    pub fn new() -> Self {
        let dir = std::env::var_os("EW_SHADER_DIR").map(PathBuf::from).unwrap_or_else(|| ShaderSources::source_dir().to_path_buf());
        Self::with_shader_dir(&dir)
    }

    pub fn with_shader_dir(dir: &Path) -> Self {
        let files: Vec<PathBuf> = ShaderSources::FILES.iter().map(|f| dir.join(f)).collect();
        let st = stamps(&files);
        Self { shader_dir: dir.to_path_buf(), shader_files: files, shader_stamps: st, last: Instant::now() }
    }

    /// Polls at most every [`POLL`]; `force` polls now.
    pub fn poll(&mut self, r: &mut Renderer, dev: &dyn Device, content: Option<&mut Content>, force: bool) -> HotEvents {
        let mut ev = HotEvents::default();
        if !force && self.last.elapsed() < POLL {
            return ev;
        }
        self.last = Instant::now();
        let st = stamps(&self.shader_files);
        if st != self.shader_stamps {
            self.shader_stamps = st;
            ev.shaders = Some(
                ShaderSources::load(&self.shader_dir)
                    .map_err(|e| format!("reading {}: {e}", self.shader_dir.display()))
                    .and_then(|s| r.reload_shaders(dev, &s).map_err(|e| format!("{e}"))),
            );
        }
        if let Some(c) = content {
            match c.poll_reload(r, dev) {
                Ok(Some(s)) => ev.packs = Some(Ok(s)),
                Ok(None) => {}
                Err(e) => ev.packs = Some(Err(e)),
            }
        }
        ev
    }
}

impl Default for HotReload {
    fn default() -> Self {
        Self::new()
    }
}

/// A poll's events as (is_error, text) lines (the editor log shows them too).
pub fn lines(ev: &HotEvents) -> Vec<(bool, String)> {
    let mut v = Vec::new();
    match &ev.shaders {
        Some(Ok(())) => v.push((false, "shaders reloaded".to_string())),
        Some(Err(e)) => v.push((true, format!("shader reload failed, keeping the old pipelines: {e}"))),
        None => {}
    }
    match &ev.packs {
        Some(Ok(s)) => v.push((
            false,
            format!(
                "packs reloaded: {} changed keys ({} metadata-only skipped) -> {} textures ({} B), {} materials, {} meshes, {} skinned, {} prefabs, {} placements respawned",
                s.changed_keys, s.metadata_only, s.textures, s.texture_bytes, s.materials, s.meshes, s.skinned_meshes, s.prefabs, s.placements
            ),
        )),
        Some(Err(e)) => v.push((true, format!("pack reload failed: {e}"))),
        None => {}
    }
    v
}

/// Prints a poll's events.
pub fn report(ev: &HotEvents) {
    for (err, l) in lines(ev) {
        if err {
            eprintln!("ew: {l}");
        } else {
            println!("ew: {l}");
        }
    }
}
