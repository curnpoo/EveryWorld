//! `--headless`: run the full game with no window.
//!
//! On macOS an occluded or unfocused window renders black, and a normal run
//! steals focus. Headless runs replace winit with a plain loop (no
//! NSApplication, so no Dock icon and no focus change), render the 3D camera
//! into an offscreen image (`HeadlessTarget`), and mute audio.
//!
//! Frame timing: with no swapchain nothing throttles the CPU against the GPU,
//! so frames would queue up and `perf.csv` would only show CPU time. Each render
//! frame therefore starts by waiting for the previous frame's GPU work: one
//! frame in flight, the same as the windowed `desired_maximum_frame_latency = 1`.
//! Frame time is then max(CPU, GPU) with the usual overlap.

use crate::config::Config;
use crate::spawn::{SpawnHeight, SpawnRequest};
use bevy::prelude::*;
use bevy::render::renderer::RenderDevice;
use bevy::render::{Render, RenderApp, RenderSystems};

pub struct HeadlessPlugin;

/// The offscreen image the 3D camera renders into in headless mode.
#[derive(Resource, Clone)]
pub struct HeadlessTarget(pub Handle<Image>);

impl Plugin for HeadlessPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GlobalVolume::new(bevy::audio::Volume::SILENT));
        if let Some(render) = app.get_sub_app_mut(RenderApp) {
            render.add_systems(Render, wait_for_gpu.in_set(RenderSystems::ExtractCommands));
        }
        app.add_systems(Update, scripted_hops);
    }
}

/// `EW_HOP="t:lat:lon,..."`: scripted fast travels at elapsed seconds (perf runs).
/// The ground height is looked up from the cache so the camera lands on the street.
pub fn scripted_hops(
    time: Res<Time<Real>>,
    cfg: Res<Config>,
    mut reqs: MessageWriter<SpawnRequest>,
    mut plan: Local<Option<Vec<(f32, f64, f64)>>>,
) {
    let plan = plan.get_or_insert_with(|| {
        let Ok(spec) = std::env::var("EW_HOP") else { return Vec::new() };
        spec.split(',')
            .filter_map(|part| {
                let mut it = part.split(':');
                let t = it.next()?.parse().ok()?;
                let lat = it.next()?.parse().ok()?;
                let lon = it.next()?.parse().ok()?;
                Some((t, lat, lon))
            })
            .collect()
    });
    let now = time.elapsed_secs();
    while plan.first().is_some_and(|(t, _, _)| now >= *t) {
        let (t, lat, lon) = plan.remove(0);
        let mut pos = ed_geo::Geodetic::new(lat, lon, 0.0);
        let tile = ed_geo::tile::TileId::containing(lat, lon, 12);
        let t_fetch = std::time::Instant::now();
        if let Ok(h) = ed_data::elevation::fetch(&cfg.cache, tile) {
            let (u, v) = tile.geodetic_to_uv(lat, lon);
            pos.alt_m = h.sample(u, v).max(0.0) as f64;
        }
        info!("hop at {t:.0}s -> {lat:.4},{lon:.4} ({:.0} m, elevation lookup {} ms)", pos.alt_m, t_fetch.elapsed().as_millis());
        reqs.write(SpawnRequest::At { pos, name: String::new(), height: SpawnHeight::Ground });
    }
}

fn wait_for_gpu(device: Res<RenderDevice>, mut acc: Local<(f64, u32, Option<std::time::Instant>)>) {
    let t0 = std::time::Instant::now();
    let _ = device.poll(bevy::render::render_resource::PollType::wait_indefinitely());
    // `EW_GPU_WAIT=1`: log the mean time the render thread blocks on the GPU.
    // Near zero = CPU-bound; most of the frame = GPU-bound.
    if std::env::var_os("EW_GPU_WAIT").is_none() {
        return;
    }
    acc.0 += t0.elapsed().as_secs_f64() * 1000.0;
    acc.1 += 1;
    let start = *acc.2.get_or_insert(t0);
    if start.elapsed().as_secs_f64() > 2.0 {
        info!("gpu wait {:.1} ms/frame over {} frames", acc.0 / acc.1 as f64, acc.1);
        *acc = (0.0, 0, None);
    }
}

/// Creates the offscreen colour target (sRGB, copyable for screenshots).
pub fn make_target(images: &mut Assets<Image>, w: u32, h: u32) -> Handle<Image> {
    let mut img = Image::new_target_texture(w.max(64), h.max(64), bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb, None);
    img.texture_descriptor.usage |= bevy::render::render_resource::TextureUsages::COPY_SRC;
    images.add(img)
}

/// `--quit-after <s>` (also works with a window).
pub fn quit_after(time: Res<Time<Real>>, cfg: Res<Config>, mut exit: MessageWriter<AppExit>) {
    if cfg.quit_after.is_some_and(|s| time.elapsed_secs() > s) {
        info!("quit-after reached ({:.0} s)", time.elapsed_secs());
        exit.write(AppExit::Success);
    }
}
