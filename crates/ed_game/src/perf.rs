//! Opt-in performance logger: run with `--perf` (or `EW_PERF=1`).
//!
//! Writes `perf.csv` (one row per second) and prints the GPU backend once.
//! Columns: frame-time stats in ms, tile streaming state, entity count.

use crate::streaming::TileManager;
use crate::foliage::FoliageMaterial;
use crate::ground_material::{ScatterMaterial, TerrainMaterial};
use bevy::prelude::*;
use bevy::render::renderer::RenderAdapterInfo;
use std::io::Write;

pub struct PerfPlugin;

impl Plugin for PerfPlugin {
    fn build(&self, app: &mut App) {
        if !(std::env::args().any(|a| a == "--perf") || std::env::var_os("EW_PERF").is_some()) {
            return;
        }
        let path = std::env::var("EW_PERF_OUT").unwrap_or_else(|_| "perf.csv".into());
        let mut f = std::fs::File::create(&path).expect("create perf csv");
        let _ = writeln!(f, "t_s,fps,avg_ms,p99_ms,max_ms,tiles,pending,desired,tris_m,entities,vis_terrain_m,vis_scatter_m,vis_foliage_m,vis_other_m,meshes_vis");
        info!("perf logging to {path}");
        app.insert_resource(PerfLog { file: f, frames: Vec::new(), since: 0.0, reported_gpu: false })
            .add_systems(Last, record);
    }
}

#[derive(Resource)]
struct PerfLog {
    file: std::fs::File,
    frames: Vec<f32>,
    since: f32,
    reported_gpu: bool,
}

fn record(
    time: Res<Time<Real>>,
    mut log: ResMut<PerfLog>,
    mgr: Option<Res<TileManager>>,
    adapter: Option<Res<RenderAdapterInfo>>,
    entities: Query<Entity>,
    meshes: Res<Assets<Mesh>>,
    mut events: MessageReader<AssetEvent<Mesh>>,
    mut tri_cache: Local<std::collections::HashMap<AssetId<Mesh>, usize>>,
    terrain: Query<(&Mesh3d, &ViewVisibility), With<MeshMaterial3d<TerrainMaterial>>>,
    scatter: Query<(&Mesh3d, &ViewVisibility), With<MeshMaterial3d<ScatterMaterial>>>,
    foliage: Query<(&Mesh3d, &ViewVisibility), With<MeshMaterial3d<FoliageMaterial>>>,
    other: Query<(&Mesh3d, &ViewVisibility), With<MeshMaterial3d<StandardMaterial>>>,
) {
    // Mesh CPU data is dropped after GPU extraction, so remember triangle counts on add.
    for e in events.read() {
        if let AssetEvent::Added { id } | AssetEvent::Modified { id } = e {
            if let Some(m) = meshes.get(*id) {
                if let Ok(i) = m.try_indices() { tri_cache.insert(*id, i.len() / 3); }
            }
        }
        if let AssetEvent::Removed { id } = e { tri_cache.remove(id); }
    }
    if !log.reported_gpu {
        if let Some(a) = adapter {
            // Backend should read `Metal` on macOS.
            info!("GPU adapter: {:?}", &**a);
            log.reported_gpu = true;
        }
    }
    log.frames.push(time.delta_secs() * 1000.0);
    log.since += time.delta_secs();
    if log.since < 1.0 {
        return;
    }
    let mut v = std::mem::take(&mut log.frames);
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = v.len().max(1);
    let avg = v.iter().sum::<f32>() / n as f32;
    let p99 = v[((n as f32 * 0.99) as usize).min(n - 1)];
    let max = v.last().copied().unwrap_or(0.0);
    let (tiles, pending, desired, tris) = mgr.map_or((0, 0, 0, 0.0), |m| (m.loaded.len(), m.pending.len(), m.desired.len(), m.totals.triangles as f32 / 1e6));
    let t = time.elapsed_secs();
    // Visible triangles per material family (CPU-side frustum result of last frame).
    let count = |it: &mut dyn Iterator<Item = (&Mesh3d, &ViewVisibility)>| -> (f32, usize) {
        let (mut t, mut n) = (0usize, 0usize);
        for (m, vis) in it {
            if vis.get() {
                n += 1;
                t += tri_cache.get(&m.0.id()).copied().unwrap_or(0);
            }
        }
        (t as f32 / 1e6, n)
    };
    let (vt, nt) = count(&mut terrain.iter());
    let (vs, ns) = count(&mut scatter.iter());
    let (vf, nf) = count(&mut foliage.iter());
    let (vo, no) = count(&mut other.iter());
    let _ = writeln!(log.file, "{t:.1},{:.1},{avg:.2},{p99:.2},{max:.2},{tiles},{pending},{desired},{tris:.2},{},{vt:.2},{vs:.2},{vf:.2},{vo:.2},{}", 1000.0 / avg.max(1e-3), entities.iter().count(), nt + ns + nf + no);
    let _ = log.file.flush();
    log.since = 0.0;
}
