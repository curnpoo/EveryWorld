//! `--screenshot out.png`: wait for the world to finish streaming around the
//! camera, save a frame, quit. Used for lookdev review and CI snapshots.

use crate::config::Config;
use crate::streaming::TileManager;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

pub struct CapturePlugin;

impl Plugin for CapturePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, capture);
    }
}

#[derive(Default)]
struct State {
    frames: u32,
    idle_frames: u32,
    shot_at: Option<u32>,
}

fn capture(
    time: Res<Time>,
    mut commands: Commands,
    cfg: Res<Config>,
    mgr: Res<TileManager>,
    mut st: Local<State>,
    mut exit: MessageWriter<AppExit>,
    headless: Option<Res<crate::headless::HeadlessTarget>>,
) {
    let Some(path) = &cfg.screenshot else { return };
    st.frames += 1;
    if mgr.idle() { st.idle_frames += 1 } else { st.idle_frames = 0 }
    match st.shot_at {
        None if (st.frames >= cfg.screenshot_frames && (st.idle_frames >= 20 || std::env::args().any(|a| a == "--no-wait"))) || time.elapsed_secs() > 480.0 => {
            info!(
                "capturing {} ({} tiles, {} buildings, {} trees, {} tris)",
                path.display(),
                mgr.loaded.len(),
                mgr.totals.buildings,
                mgr.totals.trees,
                mgr.totals.triangles
            );
            let shot = match &headless {
                Some(t) => Screenshot::image(t.0.clone()),
                None => Screenshot::primary_window(),
            };
            commands.spawn(shot).observe(save_to_disk(path.clone()));
            st.shot_at = Some(st.frames);
        }
        Some(f) if st.frames > f + 10 => {
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}
