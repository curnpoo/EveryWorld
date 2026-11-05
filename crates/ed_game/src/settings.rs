//! Player settings that survive between sessions: audio mix, graphics
//! toggles, mouse, cosmetics. Stored as JSON at
//! `~/Library/Application Support/EveryWorld/settings.json` (macOS) or
//! `$XDG_CONFIG_HOME/everyworld/settings.json`; `EW_SETTINGS=path` overrides.
//!
//! Loaded at PreStartup, saved ~1 s after the last change (and on exit).
//! Automated runs (`--screenshot`, `--perf`, `--headless`, `--lookdev`, or any
//! explicit graphics flag / `EW_GFX`) keep their CLI graphics and never write
//! the file, so perf A/B and reference shots stay reproducible.

use crate::body::Cosmetics;
use crate::config::Config;
use crate::menu::GraphicsSettings;
use crate::player::ControlSettings;
use bevy::app::AppExit;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The user's audio mix. Every gain is 0..1; `master` 0.7 reproduces the
/// tuned default levels, 1.0 is ~40 % louder.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioSettings {
    pub master: f32,
    /// Ambience: wind in trees, water, birds and insects, traffic, crowds.
    pub world: f32,
    /// The robot: footsteps, servos, landings.
    pub body: f32,
    /// Air: rushing wind, rocket boots, wingsuit, parachute.
    pub air: f32,
    /// Car and plane engines.
    pub vehicles: f32,
    /// Altimeter beeps and ground-proximity warning.
    pub alerts: f32,
    pub muted: bool,
    /// Calm ambience: only the strongest few beds and animal groups play.
    pub calm: bool,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self { master: 0.7, world: 1.0, body: 1.0, air: 1.0, vehicles: 1.0, alerts: 1.0, muted: false, calm: true }
    }
}

/// `master` at which every output plays at its tuned level.
pub const MASTER_REF: f32 = 0.7;

impl AudioSettings {
    /// Overall linear gain multiplier vs the tuned levels (0 when muted).
    /// `EW_SOUND=0..1` (old knob) still overrides the slider for testing.
    pub fn level(&self) -> f32 {
        if self.muted {
            return 0.0;
        }
        let m = env_master().unwrap_or(self.master);
        m.clamp(0.0, 1.0) / MASTER_REF
    }

    /// Per-bus gains for `ed_audio::Params::mix` (body, air, ambience, UI).
    pub fn scape_mix(&self) -> [f32; 4] {
        [self.body, self.air, self.world, self.alerts].map(|g| g.clamp(0.0, 1.0))
    }
}

fn env_master() -> Option<f32> {
    static V: std::sync::OnceLock<Option<f32>> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("EW_SOUND").ok().and_then(|s| s.parse::<f32>().ok()).map(|v| v * MASTER_REF / 0.35))
}

/// What goes in the file. Every section is optional so old/partial files load.
#[derive(Serialize, Deserialize, Default, Debug, PartialEq)]
#[serde(default)]
struct Saved {
    audio: AudioSettings,
    graphics: Option<SavedGraphics>,
    mouse_sensitivity: Option<f32>,
    invert_y: Option<bool>,
    backpack: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
struct SavedGraphics {
    shadows: bool,
    ambient_occlusion: bool,
    bloom: bool,
    aa: bool,
    render_pct: u32,
    show_stats: bool,
}

fn settings_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("EW_SETTINGS") {
        return Some(PathBuf::from(p));
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if cfg!(target_os = "macos") {
        return home.map(|h| h.join("Library/Application Support/EveryWorld/settings.json"));
    }
    let base = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).or_else(|| home.map(|h| h.join(".config")));
    base.map(|b| b.join("everyworld/settings.json"))
}

fn parse(text: &str) -> Saved {
    serde_json::from_str(text).unwrap_or_else(|e| {
        warn!("settings: ignoring unreadable settings file ({e})");
        Saved::default()
    })
}

/// Writes via a temp file + rename so a crash never leaves half a file.
fn write_atomic(path: &std::path::Path, text: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(tmp, path)
}

/// Runtime bookkeeping: where to save, whether this run may save at all.
#[derive(Resource)]
struct Store {
    path: Option<PathBuf>,
    /// False for automated runs: they read audio but never write.
    persist: bool,
    /// Graphics come from the CLI this run; keep the file's graphics untouched.
    cli_graphics: bool,
    /// Graphics as loaded, written back unchanged when `cli_graphics`.
    file_graphics: Option<SavedGraphics>,
    dirty_since: Option<f64>,
}

pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AudioSettings>()
            .init_resource::<GraphicsSettings>()
            .init_resource::<ControlSettings>()
            .init_resource::<Cosmetics>()
            .add_systems(PreStartup, load)
            .add_systems(Last, (mark_dirty, save_when_quiet).chain());
    }
}

fn automated(cfg: &Config) -> bool {
    let args: Vec<String> = std::env::args().collect();
    cfg.screenshot.is_some() || cfg.headless || cfg.lookdev.is_some() || cfg.quit_after.is_some() || args.iter().any(|a| a == "--perf") || std::env::var_os("EW_PERF").is_some()
}

fn cli_graphics(cfg: &Config) -> bool {
    cfg.no_post || std::env::args().any(|a| a == "--no-shadows" || a == "--no-post") || std::env::var_os("EW_GFX").is_some()
}

#[allow(clippy::too_many_arguments)]
fn load(
    mut commands: Commands,
    cfg: Res<Config>,
    mut audio: ResMut<AudioSettings>,
    mut gfx: ResMut<GraphicsSettings>,
    mut controls: ResMut<ControlSettings>,
    mut cosmetics: ResMut<Cosmetics>,
) {
    let path = settings_path();
    let saved = path.as_ref().and_then(|p| std::fs::read_to_string(p).ok()).map(|t| parse(&t)).unwrap_or_default();
    let auto = automated(&cfg);
    let cli_gfx = cli_graphics(&cfg) || auto;
    *audio = saved.audio;
    if !cli_gfx && let Some(g) = saved.graphics {
        gfx.shadows = g.shadows;
        gfx.ambient_occlusion = g.ambient_occlusion;
        gfx.bloom = g.bloom;
        gfx.aa = g.aa;
        gfx.render_pct = g.render_pct.clamp(25, 100);
        gfx.show_stats = g.show_stats;
    }
    if let Some(s) = saved.mouse_sensitivity {
        controls.mouse_sensitivity = s.clamp(0.2, 3.0);
    }
    if let Some(v) = saved.invert_y {
        controls.invert_y = v;
    }
    if let Some(b) = saved.backpack {
        cosmetics.backpack = b;
    }
    if let Some(p) = &path {
        info!("settings: {} ({})", p.display(), if auto { "read-only, automated run" } else { "saved on change" });
    }
    commands.insert_resource(Store { path, persist: !auto, cli_graphics: cli_gfx, file_graphics: saved.graphics, dirty_since: None });
}

fn mark_dirty(time: Res<Time<Real>>, mut store: ResMut<Store>, audio: Res<AudioSettings>, gfx: Res<GraphicsSettings>, controls: Res<ControlSettings>, cosmetics: Res<Cosmetics>) {
    // `is_changed` is also true on the first frame (resources were just added).
    let changed = audio.is_changed() || gfx.is_changed() || controls.is_changed() || cosmetics.is_changed();
    if changed && !audio.is_added() {
        store.dirty_since = Some(time.elapsed_secs_f64());
    }
}

fn snapshot(store: &Store, audio: &AudioSettings, gfx: &GraphicsSettings, controls: &ControlSettings, cosmetics: &Cosmetics) -> Saved {
    let graphics = if store.cli_graphics {
        store.file_graphics
    } else {
        Some(SavedGraphics { shadows: gfx.shadows, ambient_occlusion: gfx.ambient_occlusion, bloom: gfx.bloom, aa: gfx.aa, render_pct: gfx.render_pct, show_stats: gfx.show_stats })
    };
    Saved { audio: *audio, graphics, mouse_sensitivity: Some(controls.mouse_sensitivity), invert_y: Some(controls.invert_y), backpack: Some(cosmetics.backpack) }
}

fn save_when_quiet(
    time: Res<Time<Real>>,
    mut store: ResMut<Store>,
    mut exits: MessageReader<AppExit>,
    audio: Res<AudioSettings>,
    gfx: Res<GraphicsSettings>,
    controls: Res<ControlSettings>,
    cosmetics: Res<Cosmetics>,
) {
    let exiting = exits.read().count() > 0;
    let Some(t) = store.dirty_since else { return };
    // Wait for a slider drag to settle instead of writing every frame.
    if !exiting && time.elapsed_secs_f64() - t < 1.0 {
        return;
    }
    store.dirty_since = None;
    let (true, Some(path)) = (store.persist, store.path.clone()) else { return };
    let text = serde_json::to_string_pretty(&snapshot(&store, &audio, &gfx, &controls, &cosmetics)).unwrap_or_default();
    if let Err(e) = write_atomic(&path, &text) {
        warn!("settings: could not save {}: {e}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let s = Saved {
            audio: AudioSettings { master: 0.4, world: 0.2, muted: true, calm: false, ..default() },
            graphics: Some(SavedGraphics { shadows: false, ambient_occlusion: true, bloom: false, aa: true, render_pct: 75, show_stats: true }),
            mouse_sensitivity: Some(1.3),
            invert_y: Some(true),
            backpack: Some(true),
        };
        assert_eq!(parse(&serde_json::to_string(&s).unwrap()), s);
    }

    #[test]
    fn partial_and_broken_files_fall_back_to_defaults() {
        let s = parse(r#"{"audio":{"master":0.2}}"#);
        assert_eq!(s.audio.master, 0.2);
        assert_eq!(s.audio.world, 1.0);
        assert!(s.graphics.is_none());
        assert_eq!(parse("not json"), Saved::default());
    }

    #[test]
    fn default_master_is_unity_and_mute_silences() {
        let a = AudioSettings::default();
        assert!((a.level() - 1.0).abs() < 1e-6 || std::env::var_os("EW_SOUND").is_some());
        assert_eq!(AudioSettings { muted: true, ..a }.level(), 0.0);
    }

    #[test]
    fn atomic_write_creates_dirs() {
        let dir = std::env::temp_dir().join(format!("ew-settings-{}", std::process::id()));
        let p = dir.join("a/b/settings.json");
        write_atomic(&p, "{}").unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "{}");
        let _ = std::fs::remove_dir_all(dir);
    }
}
