//! Streams near-camera ground cover. Tiles that carry a [`GroundCover`]
//! (finest zoom only) are cut into chunks; chunks within reach of the
//! camera are generated on worker threads by `ed_world::ground_scatter` and
//! spawned as merged meshes under the tile, with a distance band that
//! decides how many instances they carry. The vertex shader does the
//! smooth per-instance fade, so swapping a chunk's band never pops.

use crate::ground_material::{GroundMaterials, ScatterMaterial};
use crate::meshconv::to_scatter_mesh;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use bevy::tasks::AsyncComputeTaskPool;
use crossbeam_channel::{Receiver, Sender};
use ed_world::ground_scatter::{CHUNKS, GroundSource, MAX_REACH, ScatterChunk};
use std::collections::HashMap;
use std::sync::Arc;

pub struct GroundScatterPlugin;

impl Plugin for GroundScatterPlugin {
    fn build(&self, app: &mut App) {
        let (tx, rx) = crossbeam_channel::unbounded();
        app.insert_resource(ScatterState { tx, rx, chunks: HashMap::new(), in_flight: 0, stats: ScatterStats::default() })
            .add_systems(Update, (plan_chunks, receive_chunks).chain());
    }
}

/// Ground-cover source for a tile entity (meshes are relative to the tile).
#[derive(Component, Clone)]
pub struct GroundCover(pub Arc<GroundSource>);

#[derive(Component)]
struct ScatterChunkMesh;

type Key = (Entity, u16, u16);

struct ChunkState {
    entity: Option<Entity>,
    band: u8,
    pending: Option<u8>,
}

#[derive(Default, Debug, Clone, Copy)]
pub struct ScatterStats {
    pub chunks: usize,
}

#[derive(Resource)]
pub struct ScatterState {
    tx: Sender<(Key, u8, ScatterChunk)>,
    rx: Receiver<(Key, u8, ScatterChunk)>,
    chunks: HashMap<Key, ChunkState>,
    in_flight: usize,
    pub stats: ScatterStats,
}

/// Distance bands (m). A chunk carries every instance visible from the
/// near edge of its band.
const BANDS: [f32; 6] = [0.0, 10.0, 20.0, 34.0, 55.0, 80.0];
const MAX_IN_FLIGHT: usize = 8;

fn band_for(d: f32) -> u8 {
    BANDS.iter().rposition(|&b| d >= b).unwrap_or(0) as u8
}

fn plan_chunks(
    mut commands: Commands,
    mut state: ResMut<ScatterState>,
    lookdev: Option<Res<crate::lookdev::Lookdev>>,
    tiles: Query<(Entity, &GroundCover, &GlobalTransform)>,
    cam: Query<&GlobalTransform, (With<Camera3d>, Without<crate::globe::GlobeCamera>)>,
) {
    let Ok(cam) = cam.single() else { return };
    let cam_w = cam.translation();
    let mut desired: HashMap<Key, (u8, f32, Arc<GroundSource>)> = HashMap::new();
    for (e, gc, gt) in &tiles {
        let src = &gc.0;
        let local = gt.affine().inverse().transform_point3(cam_w);
        let (u, v) = src.approx_uv(local);
        let size = src.chunk_size_m();
        let r = (MAX_REACH / size).ceil() as i64 + 1;
        let (cu, cv) = ((u * CHUNKS as f64).floor() as i64, (v * CHUNKS as f64).floor() as i64);
        if cu < -r || cv < -r || cu > CHUNKS as i64 + r || cv > CHUNKS as i64 + r {
            continue;
        }
        let half_diag = size * std::f32::consts::FRAC_1_SQRT_2;
        for cj in (cv - r).max(0)..=(cv + r).min(CHUNKS as i64 - 1) {
            for ci in (cu - r).max(0)..=(cu + r).min(CHUNKS as i64 - 1) {
                let c = src.chunk_center(ci as usize, cj as usize);
                let nearest = (c.distance(local) - half_diag).max(0.0);
                if nearest < MAX_REACH {
                    desired.insert((e, ci as u16, cj as u16), (band_for(nearest), nearest, src.clone()));
                }
            }
        }
    }
    // Retire chunks that are out of reach or whose tile is gone.
    let gone: Vec<Key> = state.chunks.keys().filter(|k| !desired.contains_key(k)).copied().collect();
    for k in gone {
        if let Some(cs) = state.chunks.remove(&k)
            && let Some(ent) = cs.entity
        {
            commands.entity(ent).try_despawn();
        }
    }
    // Request (re)generation: missing chunks, chunks that now need more
    // instances, and chunks that could shed a lot. Nearest first.
    let mut todo: Vec<(f32, Key, u8, Arc<GroundSource>)> = desired
        .iter()
        .filter_map(|(k, (band, d, src))| {
            let need = match state.chunks.get(k) {
                None => true,
                Some(cs) if cs.pending.is_some() => false,
                Some(cs) => *band < cs.band || *band >= cs.band + 2,
            };
            need.then(|| (*d, *k, *band, src.clone()))
        })
        .collect();
    todo.sort_by(|a, b| a.0.total_cmp(&b.0));
    let sync = lookdev.is_some();
    let pool = AsyncComputeTaskPool::get();
    for (_, key, band, src) in todo {
        if !sync && state.in_flight >= MAX_IN_FLIGHT {
            break;
        }
        let cs = state.chunks.entry(key).or_insert(ChunkState { entity: None, band, pending: None });
        cs.pending = Some(band);
        let near = BANDS[band as usize];
        if sync {
            // Lookdev: build right away so every shot is complete.
            let chunk = src.build_chunk(key.1 as usize, key.2 as usize, near);
            let _ = state.tx.send((key, band, chunk));
        } else {
            state.in_flight += 1;
            let tx = state.tx.clone();
            pool.spawn(async move {
                let chunk = src.build_chunk(key.1 as usize, key.2 as usize, near);
                let _ = tx.send((key, band, chunk));
            })
            .detach();
        }
    }
}

fn receive_chunks(
    mut commands: Commands,
    mut state: ResMut<ScatterState>,
    mut meshes: ResMut<Assets<Mesh>>,
    gm: Option<Res<GroundMaterials>>,
    lookdev: Option<Res<crate::lookdev::Lookdev>>,
) {
    let Some(gm) = gm else { return };
    let budget = if lookdev.is_some() { usize::MAX } else { 6 };
    let mut done = 0;
    while done < budget {
        let Ok((key, band, chunk)) = state.rx.try_recv() else { break };
        done += 1;
        if lookdev.is_none() {
            state.in_flight = state.in_flight.saturating_sub(1);
        }
        let Some(cs) = state.chunks.get_mut(&key) else { continue }; // retired meanwhile
        if cs.pending != Some(band) {
            continue; // superseded
        }
        cs.pending = None;
        cs.band = band;
        let old = cs.entity.take();
        if !chunk.mesh.is_empty() && commands.get_entity(key.0).is_ok() {
            let mut e = commands.spawn((
                ScatterChunkMesh,
                Mesh3d(meshes.add(to_scatter_mesh(&chunk))),
                MeshMaterial3d::<ScatterMaterial>(gm.scatter.clone()),
                Transform::default(),
                ChildOf(key.0),
            ));
            if band >= 4 {
                e.insert(NotShadowCaster);
            }
            cs.entity = Some(e.id());
        }
        if let Some(o) = old {
            commands.entity(o).try_despawn();
        }
    }
    state.stats.chunks = state.chunks.values().filter(|c| c.entity.is_some()).count();
}
