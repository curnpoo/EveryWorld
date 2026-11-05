//! Static tile meshes that only matter near the camera (rocks, ...): kept as
//! data on one component per tile, and entities only while in range. Part
//! of "tiles are data, not entities": a far tile costs no entity for them.

use avian3d::prelude::*;
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
    /// CPU-only positions+indices for the collider. The uploaded mesh is
    /// RENDER_WORLD-only, so its data is gone after extraction and
    /// `trimesh_from_mesh(uploaded)` panics. Dropped once the collider exists.
    col_src: Option<Mesh>,
    entity: Option<Entity>,
    /// Static collision, built from the uploaded mesh when first within
    /// [`COLLIDE_M`], then kept; the entity exists only while in range.
    collider: Option<Collider>,
    col_entity: Option<Entity>,
}

impl NearMeshes {
    /// `meshes`: (chunk centre in tile space, mesh).
    pub fn new(range: f32, material: Handle<StandardMaterial>, meshes: Vec<(Vec3, Mesh)>) -> Self {
        let items = meshes.into_iter().map(|(center, m)| Item { center, col_src: Some(cpu_copy(&m)), src: Some(m), handle: None, entity: None, collider: None, col_entity: None }).collect();
        Self { range, material, items }
    }
}

/// Positions + indices only, kept in the main world for collider building.
fn cpu_copy(m: &Mesh) -> Mesh {
    use bevy::asset::RenderAssetUsages;
    let mut c = Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, RenderAssetUsages::MAIN_WORLD);
    if let Some(p) = m.attribute(Mesh::ATTRIBUTE_POSITION) {
        c.insert_attribute(Mesh::ATTRIBUTE_POSITION, p.clone());
    }
    if let Some(i) = m.indices() {
        c.insert_indices(i.clone());
    }
    c
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

/// Rocks collide only this close to the camera (m).
const COLLIDE_M: f32 = 90.0;
/// Trimesh builds per frame at most.
const COLLIDERS_PER_FRAME: usize = 3;

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
    let mut built = 0;
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
            let want_col = d < COLLIDE_M;
            if want_col && it.col_entity.is_none() {
                if it.collider.is_none() && built < COLLIDERS_PER_FRAME {
                    if let Some(m) = it.col_src.take() {
                        built += 1;
                        it.collider = Collider::trimesh_from_mesh(&m);
                    }
                }
                if let Some(c) = &it.collider {
                    it.col_entity = Some(commands.spawn((RigidBody::Static, c.clone(), Transform::default(), ChildOf(root))).id());
                }
            } else if !want_col && d > COLLIDE_M * 1.15 {
                if let Some(e) = it.col_entity.take() {
                    commands.entity(e).despawn();
                }
            }
        }
    }
}
