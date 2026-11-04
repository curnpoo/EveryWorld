//! Static tile meshes that only matter near the camera (rocks, ...): kept as
//! data on one component per tile, and entities only while in range. Part
//! of "tiles are data, not entities": a far tile costs no entity for them.

use bevy::prelude::*;

pub struct NearChunksPlugin;

impl Plugin for NearChunksPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, stream_near_meshes);
    }
}

/// Chunks of one tile. The holder entity sits at the tile origin, so chunk
/// meshes keep their tile-space positions.
#[derive(Component)]
pub struct NearMeshes {
    range: f32,
    material: Handle<StandardMaterial>,
    items: Vec<Item>,
}

struct Item {
    center: Vec3,
    /// Uploaded on first approach, then kept by handle.
    src: Option<Mesh>,
    handle: Option<Handle<Mesh>>,
    entity: Option<Entity>,
}

impl NearMeshes {
    /// `meshes`: (chunk centre in tile space, mesh).
    pub fn new(range: f32, material: Handle<StandardMaterial>, meshes: Vec<(Vec3, Mesh)>) -> Self {
        let items = meshes.into_iter().map(|(center, m)| Item { center, src: Some(m), handle: None, entity: None }).collect();
        Self { range, material, items }
    }
}

/// Centre of a mesh's bounding box (worker side, to place a chunk).
pub fn center_of(positions: &[[f32; 3]]) -> Vec3 {
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for p in positions {
        lo = lo.min(Vec3::from(*p));
        hi = hi.max(Vec3::from(*p));
    }
    if lo.x > hi.x { Vec3::ZERO } else { (lo + hi) * 0.5 }
}

/// Spawns per frame at most (each may upload a mesh).
const SPAWNS_PER_FRAME: usize = 32;

fn stream_near_meshes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    cam: Query<&GlobalTransform, (With<Camera3d>, Without<crate::globe::GlobeCamera>)>,
    mut sets: Query<(Entity, &GlobalTransform, &mut NearMeshes)>,
) {
    let Some(cam) = cam.iter().next().map(|c| c.translation()) else { return };
    let mut spawned = 0;
    for (root, gt, mut set) in &mut sets {
        let (range, mat) = (set.range, set.material.clone());
        for it in set.items.iter_mut() {
            let d = gt.transform_point(it.center).distance(cam);
            let r = range + if it.entity.is_some() { range * 0.1 } else { 0.0 };
            if d < r {
                if it.entity.is_none() && spawned < SPAWNS_PER_FRAME {
                    if it.handle.is_none() {
                        it.handle = it.src.take().map(|m| meshes.add(m));
                    }
                    if let Some(h) = &it.handle {
                        spawned += 1;
                        it.entity = Some(commands.spawn((Mesh3d(h.clone()), MeshMaterial3d(mat.clone()), Transform::default(), ChildOf(root))).id());
                    }
                }
            } else if let Some(e) = it.entity.take() {
                commands.entity(e).despawn();
            }
        }
    }
}
