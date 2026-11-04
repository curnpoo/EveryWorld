//! Bevy side of the asset library (`ed_world::library`, see `docs/PLAN-engine.md`).
//!
//! Every library asset is cooked ONCE at startup (palettized mesh -> meshlet
//! mesh + collider); tiles then spawn their `Placement`s as instances that
//! share the cooked handles. `EW_NO_MESHLET=1` draws the same instances as
//! ordinary meshes for A/B comparisons.

use crate::assets::WorldMaterials;
use crate::meshconv::to_mesh;
use avian3d::prelude::*;
use bevy::pbr::experimental::meshlet::{MESHLET_DEFAULT_VERTEX_POSITION_QUANTIZATION_FACTOR, MeshletMesh, MeshletMesh3d};
use bevy::prelude::*;
use ed_world::library::{self, ColliderShape, Placement, RenderPath};
use ed_world::mesh::MeshData;

pub struct LibraryPlugin;

impl Plugin for LibraryPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, cook);
    }
}

struct Cooked {
    meshlet: Option<Handle<MeshletMesh>>,
    mesh: Handle<Mesh>,
    collider: Option<Collider>,
}

/// GPU handles for every asset of the installed library, indexed by `AssetId`.
#[derive(Resource)]
pub struct CookedLibrary {
    assets: Vec<Cooked>,
}

/// Meshlet (virtual geometry) version of a palettized mesh: position, normal, UV only.
/// Positions must be near the origin (Bevy asserts cluster bounds nest within 1e-4).
pub fn meshlet_mesh(m: &MeshData) -> Option<MeshletMesh> {
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::{Indices, PrimitiveTopology};
    if m.positions.is_empty() {
        return None;
    }
    let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, m.positions.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, m.normals.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, m.uvs.clone())
        .with_inserted_indices(Indices::U32(m.indices.clone()));
    MeshletMesh::from_mesh(&mesh, MESHLET_DEFAULT_VERTEX_POSITION_QUANTIZATION_FACTOR)
        .map_err(|e| warn!("meshlet conversion failed: {e:?}"))
        .ok()
}

pub fn meshlets_enabled() -> bool {
    std::env::var_os("EW_NO_MESHLET").is_none()
}

fn cook(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut meshlets: ResMut<Assets<MeshletMesh>>) {
    let t0 = std::time::Instant::now();
    let lib = library::get();
    let assets = lib
        .iter()
        .map(|(_, def)| {
            let mut m = (def.build)();
            m.palettize();
            let meshlet = match def.render {
                RenderPath::Meshlet if meshlets_enabled() => meshlet_mesh(&m).map(|ml| meshlets.add(ml)),
                _ => None,
            };
            let collider = match def.collider {
                ColliderShape::None => None,
                ColliderShape::Cylinder { radius, height } => {
                    Some(Collider::compound(vec![(Vec3::Y * height * 0.5, Quat::IDENTITY, Collider::cylinder(radius, height))]))
                }
            };
            Cooked { meshlet, mesh: meshes.add(to_mesh(&m)), collider }
        })
        .collect();
    info!("asset library: cooked {} assets in {:.0} ms", lib.len(), t0.elapsed().as_secs_f32() * 1e3);
    commands.insert_resource(CookedLibrary { assets });
}

/// Spawn `placements` (tile mesh space) as children of the tile entity `parent`.
pub fn spawn_placements(commands: &mut Commands, lib: &CookedLibrary, mats: &WorldMaterials, parent: Entity, placements: &[Placement]) {
    for p in placements {
        let Some(c) = lib.assets.get(p.asset.0 as usize) else { continue };
        let tf = Transform { translation: p.pos, rotation: p.rot, scale: Vec3::splat(p.scale) };
        let mut e = match &c.meshlet {
            Some(h) => commands.spawn((MeshletMesh3d(h.clone()), MeshMaterial3d(mats.palette_matte.clone()), tf, ChildOf(parent))),
            None => commands.spawn((Mesh3d(c.mesh.clone()), MeshMaterial3d(mats.matte.clone()), tf, ChildOf(parent))),
        };
        if let Some(col) = &c.collider {
            e.insert((RigidBody::Static, col.clone()));
        }
    }
}
