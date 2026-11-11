//! Engine-free mesh data as the baker builds it, and its conversion to and from the pack's
//! `mesh` chunk.
//!
//! Pack vertex layout written by the baker (every attribute 4-byte aligned, f32 unless noted):
//! `Position F32x3 @0, Normal F32x3 @12`, then if present `Uv0 F32x2`, then if skinned
//! `Joints U16x4, Weights F32x4`. Indices are always `U32`. One section per LOD (slot 0).

use std::borrow::Cow;

use anyhow::{Result, bail};
use ed_pack::kinds::{Bounds, IndexFormat, Mesh, MeshLod, MeshSection, VertexAttribute, VertexFormat, VertexSemantic};

/// A triangle mesh with optional UVs and skin weights.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeshBuild {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    /// Empty, or one per vertex.
    pub uvs: Vec<[f32; 2]>,
    /// Empty, or one per vertex (joint indices into the skeleton).
    pub joints: Vec<[u16; 4]>,
    /// Empty, or one per vertex (normalised to sum 1).
    pub weights: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}

/// One LOD level of a baked mesh.
#[derive(Clone, Debug, PartialEq)]
pub struct Lod {
    pub indices: Vec<u32>,
    /// Geometric error against LOD 0 in mesh units (metres), measured on the result.
    pub error: f32,
}

impl MeshBuild {
    pub fn validate(&self) -> Result<()> {
        let n = self.positions.len();
        if n == 0 || self.indices.is_empty() || !self.indices.len().is_multiple_of(3) {
            bail!("mesh has no vertices or is not a triangle list");
        }
        if self.normals.len() != n {
            bail!("mesh has {} normals for {} vertices", self.normals.len(), n);
        }
        if !self.uvs.is_empty() && self.uvs.len() != n {
            bail!("mesh has {} UVs for {} vertices", self.uvs.len(), n);
        }
        if self.joints.len() != self.weights.len() || (!self.joints.is_empty() && self.joints.len() != n) {
            bail!("mesh skin streams do not match its vertex count");
        }
        if self.indices.iter().any(|&i| i as usize >= n) {
            bail!("mesh index out of range");
        }
        if self.positions.iter().flatten().chain(self.normals.iter().flatten()).chain(self.uvs.iter().flatten()).any(|v| !v.is_finite()) {
            bail!("mesh has non-finite vertex data");
        }
        Ok(())
    }

    pub fn is_skinned(&self) -> bool {
        !self.joints.is_empty()
    }

    /// Bounding sphere (centre of the box, radius to the farthest vertex) and box.
    pub fn bounds(&self) -> Bounds {
        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];
        for p in &self.positions {
            for k in 0..3 {
                min[k] = min[k].min(p[k]);
                max[k] = max[k].max(p[k]);
            }
        }
        let c = [(min[0] + max[0]) * 0.5, (min[1] + max[1]) * 0.5, (min[2] + max[2]) * 0.5];
        let r = self.positions.iter().map(|p| ((p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2) + (p[2] - c[2]).powi(2)).sqrt()).fold(0.0f32, f32::max);
        Bounds { centre: c, radius: r, min, max }
    }

    /// The pack chunk body for this mesh with its LOD chain (LOD 0 first).
    pub fn to_pack(&self, lods: &[Lod]) -> Result<Mesh<'static>> {
        self.validate()?;
        let mut attributes = vec![
            VertexAttribute { semantic: VertexSemantic::Position, format: VertexFormat::F32x3, offset: 0 },
            VertexAttribute { semantic: VertexSemantic::Normal, format: VertexFormat::F32x3, offset: 12 },
        ];
        let mut stride = 24u16;
        if !self.uvs.is_empty() {
            attributes.push(VertexAttribute { semantic: VertexSemantic::Uv0, format: VertexFormat::F32x2, offset: stride });
            stride += 8;
        }
        if self.is_skinned() {
            attributes.push(VertexAttribute { semantic: VertexSemantic::Joints, format: VertexFormat::U16x4, offset: stride });
            attributes.push(VertexAttribute { semantic: VertexSemantic::Weights, format: VertexFormat::F32x4, offset: stride + 8 });
            stride += 24;
        }
        let mut vd = Vec::with_capacity(self.positions.len() * stride as usize);
        for i in 0..self.positions.len() {
            for v in self.positions[i].iter().chain(&self.normals[i]) {
                vd.extend_from_slice(&v.to_le_bytes());
            }
            if let Some(uv) = self.uvs.get(i) {
                vd.extend_from_slice(&uv[0].to_le_bytes());
                vd.extend_from_slice(&uv[1].to_le_bytes());
            }
            if self.is_skinned() {
                for j in self.joints[i] {
                    vd.extend_from_slice(&j.to_le_bytes());
                }
                for w in self.weights[i] {
                    vd.extend_from_slice(&w.to_le_bytes());
                }
            }
        }
        let mut id = Vec::new();
        let mut out_lods = Vec::with_capacity(lods.len());
        for l in lods {
            let first = (id.len() / 4) as u32;
            for &i in &l.indices {
                id.extend_from_slice(&i.to_le_bytes());
            }
            let n = l.indices.len() as u32;
            out_lods.push(MeshLod { first_index: first, index_count: n, error_m: l.error, sections: vec![MeshSection { first_index: first, index_count: n, material_slot: 0 }] });
        }
        let m = Mesh {
            attributes,
            vertex_stride: stride as u32,
            vertex_count: self.positions.len() as u32,
            vertex_data: Cow::Owned(vd),
            index_format: IndexFormat::U32,
            index_data: Cow::Owned(id),
            lods: out_lods,
            bounds: self.bounds(),
            material_slots: vec!["default".into()],
        };
        m.validate().map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(m)
    }
}

fn f32_at(b: &[u8], at: usize) -> f32 {
    f32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// Decoded streams of a pack mesh (the runtime's view). Missing UVs come back empty.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DecodedMesh {
    pub build: MeshBuild,
    pub lods: Vec<Lod>,
}

/// Reads a pack mesh written by [`MeshBuild::to_pack`] (or any mesh with F32 positions and
/// normals, optional F32x2 UVs and U16x4 / F32x4 skin streams).
pub fn decode(m: &Mesh<'_>) -> Result<DecodedMesh> {
    let stride = m.vertex_stride as usize;
    let find = |s: VertexSemantic| m.attributes.iter().find(|a| a.semantic == s).copied();
    let (Some(p), Some(n)) = (find(VertexSemantic::Position), find(VertexSemantic::Normal)) else {
        bail!("mesh without position or normal");
    };
    if p.format != VertexFormat::F32x3 || n.format != VertexFormat::F32x3 {
        bail!("unsupported position/normal format");
    }
    let uv = find(VertexSemantic::Uv0).filter(|a| a.format == VertexFormat::F32x2);
    let joints = find(VertexSemantic::Joints).filter(|a| a.format == VertexFormat::U16x4);
    let weights = find(VertexSemantic::Weights).filter(|a| a.format == VertexFormat::F32x4);
    let vd = &m.vertex_data;
    let mut b = MeshBuild::default();
    for v in 0..m.vertex_count as usize {
        let at = v * stride;
        let r3 = |o: u16| [f32_at(vd, at + o as usize), f32_at(vd, at + o as usize + 4), f32_at(vd, at + o as usize + 8)];
        b.positions.push(r3(p.offset));
        b.normals.push(r3(n.offset));
        if let Some(a) = uv {
            let o = at + a.offset as usize;
            b.uvs.push([f32_at(vd, o), f32_at(vd, o + 4)]);
        }
        if let (Some(j), Some(w)) = (joints, weights) {
            let o = at + j.offset as usize;
            b.joints.push([0, 1, 2, 3].map(|k| u16::from_le_bytes([vd[o + 2 * k], vd[o + 2 * k + 1]])));
            let o = at + w.offset as usize;
            b.weights.push([0, 1, 2, 3].map(|k| f32_at(vd, o + 4 * k)));
        }
    }
    let all: Vec<u32> = (0..m.index_count()).map(|i| m.index(i).unwrap_or(0)).collect();
    let lods = m
        .lods
        .iter()
        .map(|l| Lod { indices: all[l.first_index as usize..(l.first_index + l.index_count) as usize].to_vec(), error: l.error_m })
        .collect::<Vec<_>>();
    b.indices = lods.first().map(|l| l.indices.clone()).unwrap_or_default();
    Ok(DecodedMesh { build: b, lods })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_round_trip_keeps_every_stream() {
        let b = MeshBuild {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
            joints: vec![[0, 1, 0, 0]; 3],
            weights: vec![[0.75, 0.25, 0.0, 0.0]; 3],
            indices: vec![0, 1, 2],
        };
        let m = b.to_pack(&[Lod { indices: b.indices.clone(), error: 0.0 }]).unwrap();
        assert_eq!(m.vertex_stride, 56);
        let d = decode(&m).unwrap();
        assert_eq!(d.build, b);
    }
}
