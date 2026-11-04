//! Bevy side of the asset library (`ed_world::library`, see `docs/PLAN-engine.md`).
//!
//! Every library asset is cooked ONCE at startup (palettized mesh -> meshlet
//! mesh + collider); tiles then spawn their `Placement`s as instances that
//! share the cooked handles. `EW_MESHLET=1` draws the same instances as meshlets instead of
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

/// Meshlets (virtual geometry) are opt-in: `EW_MESHLET=1`. On Apple GPUs the
/// meshlet cull + raster passes (run again per shadow cascade) measured ~60%
/// slower than ordinary meshes at the same content (24 vs 39 fps, Nov 2026).
pub fn meshlets_enabled() -> bool {
    std::env::var("EW_MESHLET").is_ok_and(|v| v != "0") && std::env::var_os("EW_NO_MESHLET").is_none()
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

/// A tile's placements merged into one mesh and one compound collider
/// (built on a worker), so a tile's props cost two entities, not hundreds.
pub struct MergedPlacements {
    pub mesh: Option<Mesh>,
    pub collider: Option<Collider>,
}

/// Source meshes of every library asset, palettized like the cooked ones.
fn sources() -> &'static [MeshData] {
    static S: std::sync::OnceLock<Vec<MeshData>> = std::sync::OnceLock::new();
    S.get_or_init(|| {
        library::get()
            .iter()
            .map(|(_, def)| {
                let mut m = (def.build)();
                m.palettize();
                m
            })
            .collect()
    })
}

/// Merge `placements` (tile mesh space). Worker-thread safe.
pub fn merge_placements(placements: &[Placement]) -> MergedPlacements {
    let lib = library::get();
    let src = sources();
    let mut mesh = MeshData::default();
    let mut shapes = Vec::new();
    for p in placements {
        let Some(m) = src.get(p.asset.0 as usize) else { continue };
        let base = mesh.positions.len() as u32;
        for i in 0..m.positions.len() {
            mesh.positions.push((p.rot * (Vec3::from(m.positions[i]) * p.scale) + p.pos).to_array());
            mesh.normals.push((p.rot * Vec3::from(m.normals[i])).to_array());
        }
        mesh.uvs.extend_from_slice(&m.uvs);
        mesh.colors.extend_from_slice(&m.colors);
        mesh.indices.extend(m.indices.iter().map(|i| i + base));
        if let ColliderShape::Cylinder { radius, height } = lib.get(p.asset).collider {
            let h = height * p.scale;
            shapes.push((p.pos + p.rot * (Vec3::Y * h * 0.5), p.rot, Collider::cylinder(radius * p.scale, h)));
        }
    }
    MergedPlacements {
        mesh: (!mesh.is_empty()).then(|| to_mesh(&mesh)),
        collider: (!shapes.is_empty()).then(|| Collider::compound(shapes)),
    }
}

/// Spawn merged placements under the tile entity `parent`.
pub fn spawn_merged(commands: &mut Commands, meshes: &mut Assets<Mesh>, mats: &WorldMaterials, parent: Entity, m: MergedPlacements) {
    if let Some(mesh) = m.mesh {
        commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(mats.matte.clone()), Transform::default(), ChildOf(parent)));
    }
    if let Some(col) = m.collider {
        commands.spawn((RigidBody::Static, col, Transform::default(), ChildOf(parent)));
    }
}
