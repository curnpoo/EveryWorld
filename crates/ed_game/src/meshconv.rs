//! `ed_world::MeshData` -> `bevy::Mesh`.

use crate::ground_material::{ATTRIBUTE_PIVOT, ATTRIBUTE_SPLAT0, ATTRIBUTE_SPLAT1, ATTRIBUTE_SPLAT2, ATTRIBUTE_SPLAT_PAD};
use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;
use ed_world::ground_scatter::ScatterChunk;
use ed_world::mesh::MeshData;

pub fn to_mesh(m: &MeshData) -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, crate::native::mesh_usage())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, m.positions.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, m.normals.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, m.uvs.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, m.colors.clone())
        .with_inserted_indices(Indices::U32(m.indices.clone()));
    // Terrain: ground splat weights, 4 layers per attribute.
    if !m.splat.is_empty() && m.splat.len() == m.positions.len() {
        for (k, attr) in [ATTRIBUTE_SPLAT0, ATTRIBUTE_SPLAT1, ATTRIBUTE_SPLAT2].into_iter().enumerate() {
            let v: Vec<[u8; 4]> = m.splat.iter().map(|s| [s[k * 4], s[k * 4 + 1], s[k * 4 + 2], s[k * 4 + 3]]).collect();
            mesh.insert_attribute(attr, VertexAttributeValues::Unorm8x4(v));
        }
        mesh.insert_attribute(ATTRIBUTE_SPLAT_PAD, VertexAttributeValues::Unorm8x4(if m.sat.len() == m.positions.len() { m.sat.clone() } else { vec![[0; 4]; m.positions.len()] }));
    }
    mesh
}

/// Ground-cover chunk: the usual attributes plus per-vertex instance pivots.
pub fn to_scatter_mesh(c: &ScatterChunk) -> Mesh {
    to_mesh(&c.mesh).with_inserted_attribute(ATTRIBUTE_PIVOT, c.pivots.clone())
}
