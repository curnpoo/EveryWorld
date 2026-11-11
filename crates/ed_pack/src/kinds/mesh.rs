//! `mesh` (kind 16): vertex and index streams with a required discrete LOD chain.
//!
//! Schema 1 payload:
//! ```text
//! vertex_count u32, vertex_stride u32, index_count u32, index_format u8, attribute_count u8,
//! lod_count u16, slot_count u16, reserved u16 (0), vertex_offset u64, index_offset u64,
//! bounds (centre f32x3, radius f32, min f32x3, max f32x3)
//! attributes: attribute_count x (semantic u8, format u8, offset u16)
//! lods: lod_count x (first_index u32, index_count u32, error_m f32, section_count u32,
//!        sections: section_count x (first_index u32, index_count u32, material_slot u16, 0 u16))
//! material slots: slot_count x (u16 len + UTF-8 name)
//! zero padding to 16; vertex data at vertex_offset; padding to 16; index data at index_offset
//! ```
//! Offsets are relative to the payload start. The cluster DAG for the same mesh is a separate
//! optional `clusters` chunk (kind 17); a mesh never references it (pack.md, F3).

use std::borrow::Cow;

use super::Bounds;
use crate::chunk::ChunkBody;
use crate::error::{Error, Result};
use crate::key::ChunkKind;
use crate::wire::{In, Out};

wire_enum! {
    pub enum IndexFormat: u8 { U16 = 0, U32 = 1 }
}
impl IndexFormat {
    pub fn size(self) -> usize {
        match self {
            IndexFormat::U16 => 2,
            IndexFormat::U32 => 4,
        }
    }
}

wire_enum! {
    pub enum VertexSemantic: u8 {
        Position = 0, Normal = 1, Tangent = 2, Uv0 = 3, Uv1 = 4, Colour = 5, Joints = 6, Weights = 7,
    }
}

wire_enum! {
    pub enum VertexFormat: u8 {
        F32x2 = 0, F32x3 = 1, F32x4 = 2, Unorm8x4 = 3, Snorm8x4 = 4, Unorm16x2 = 5,
        Snorm16x2 = 6, Snorm16x4 = 7, U16x4 = 8, U8x4 = 9, F16x2 = 10, F16x4 = 11,
    }
}
impl VertexFormat {
    pub fn size(self) -> usize {
        use VertexFormat::*;
        match self {
            F32x2 => 8,
            F32x3 => 12,
            F32x4 => 16,
            Unorm8x4 | Snorm8x4 | U8x4 | Unorm16x2 | Snorm16x2 | F16x2 => 4,
            Snorm16x4 | U16x4 | F16x4 => 8,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VertexAttribute {
    pub semantic: VertexSemantic,
    pub format: VertexFormat,
    /// Byte offset inside the vertex, a multiple of 4.
    pub offset: u16,
}

/// A run of indices drawn with one material slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MeshSection {
    pub first_index: u32,
    pub index_count: u32,
    pub material_slot: u16,
}

/// One level of the discrete LOD chain.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshLod {
    pub first_index: u32,
    pub index_count: u32,
    /// Geometric error of this level against LOD 0, in metres. Non-decreasing along the chain.
    pub error_m: f32,
    /// Material runs inside `first_index..first_index + index_count`. May be empty when the
    /// mesh has no material slots.
    pub sections: Vec<MeshSection>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Mesh<'a> {
    pub attributes: Vec<VertexAttribute>,
    pub vertex_stride: u32,
    pub vertex_count: u32,
    pub vertex_data: Cow<'a, [u8]>,
    pub index_format: IndexFormat,
    pub index_data: Cow<'a, [u8]>,
    pub lods: Vec<MeshLod>,
    pub bounds: Bounds,
    pub material_slots: Vec<String>,
}

impl Mesh<'_> {
    pub fn index_count(&self) -> usize {
        self.index_data.len() / self.index_format.size()
    }
    pub fn index(&self, i: usize) -> Option<u32> {
        let s = self.index_format.size();
        let b = self.index_data.get(i * s..i * s + s)?;
        Some(match self.index_format {
            IndexFormat::U16 => u16::from_le_bytes([b[0], b[1]]) as u32,
            IndexFormat::U32 => u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
        })
    }
    pub fn into_owned(self) -> Mesh<'static> {
        Mesh {
            vertex_data: Cow::Owned(self.vertex_data.into_owned()),
            index_data: Cow::Owned(self.index_data.into_owned()),
            attributes: self.attributes,
            vertex_stride: self.vertex_stride,
            vertex_count: self.vertex_count,
            index_format: self.index_format,
            lods: self.lods,
            bounds: self.bounds,
            material_slots: self.material_slots,
        }
    }

    pub fn validate(&self) -> Result<()> {
        let bad = |s: String| Err(Error::Invalid(format!("mesh: {s}")));
        let stride = self.vertex_stride as usize;
        if stride == 0 || !stride.is_multiple_of(4) {
            return bad(format!("vertex stride {stride} is not a positive multiple of 4"));
        }
        if !self.attributes.iter().any(|a| a.semantic == VertexSemantic::Position) {
            return bad("no position attribute".into());
        }
        for (i, a) in self.attributes.iter().enumerate() {
            if a.offset % 4 != 0 || a.offset as usize + a.format.size() > stride {
                return bad(format!("attribute {:?} at offset {} does not fit the stride", a.semantic, a.offset));
            }
            if self.attributes[..i].iter().any(|b| b.semantic == a.semantic) {
                return bad(format!("duplicate attribute {:?}", a.semantic));
            }
        }
        let expect = (self.vertex_count as usize).checked_mul(stride);
        if expect != Some(self.vertex_data.len()) {
            return bad(format!("vertex data is {} bytes, expected {} x {}", self.vertex_data.len(), self.vertex_count, stride));
        }
        let isz = self.index_format.size();
        if !self.index_data.len().is_multiple_of(isz) {
            return bad("index data is not a whole number of indices".into());
        }
        let n = self.index_count();
        if !n.is_multiple_of(3) || n > u32::MAX as usize {
            return bad(format!("index count {n} is not a triangle list"));
        }
        for i in 0..n {
            if self.index(i).is_none_or(|v| v >= self.vertex_count) {
                return bad(format!("index {i} out of range"));
            }
        }
        if self.lods.is_empty() || self.lods.len() > u16::MAX as usize {
            return bad("the LOD chain needs 1 to 65535 levels".into());
        }
        let mut prev_err = 0f32;
        for (l, lod) in self.lods.iter().enumerate() {
            let end = lod.first_index as u64 + lod.index_count as u64;
            if lod.index_count == 0 || lod.index_count % 3 != 0 || end > n as u64 {
                return bad(format!("LOD {l} index range is empty, not triangles, or out of range"));
            }
            if !(lod.error_m.is_finite() && lod.error_m >= prev_err) {
                return bad(format!("LOD {l} error {} is not finite and non-decreasing", lod.error_m));
            }
            prev_err = lod.error_m;
            for s in &lod.sections {
                let s_end = s.first_index as u64 + s.index_count as u64;
                if s.first_index < lod.first_index || s_end > end || s.index_count % 3 != 0 {
                    return bad(format!("LOD {l} section outside its LOD range"));
                }
                if s.material_slot as usize >= self.material_slots.len() {
                    return bad(format!("LOD {l} section names material slot {}", s.material_slot));
                }
            }
        }
        if self.attributes.len() > u8::MAX as usize || self.material_slots.len() > u16::MAX as usize {
            return bad("too many attributes or material slots".into());
        }
        self.bounds.validate()
    }
}

impl<'a> ChunkBody<'a> for Mesh<'a> {
    const NAME: &'static str = "Mesh";
    fn kind(&self) -> ChunkKind {
        ChunkKind::Mesh
    }
    fn accepts(kind: ChunkKind) -> bool {
        kind == ChunkKind::Mesh
    }

    fn encode_payload(&self, buf: &mut Vec<u8>) -> Result<()> {
        self.validate()?;
        let mut o = Out::new(buf);
        o.u32(self.vertex_count);
        o.u32(self.vertex_stride);
        o.u32(self.index_count() as u32);
        o.u8(self.index_format.to_wire());
        o.u8(self.attributes.len() as u8);
        o.u16(self.lods.len() as u16);
        o.u16(self.material_slots.len() as u16);
        o.u16(0);
        let offsets_at = o.pos();
        o.u64(0);
        o.u64(0);
        self.bounds.write(&mut o);
        for a in &self.attributes {
            o.u8(a.semantic.to_wire());
            o.u8(a.format.to_wire());
            o.u16(a.offset);
        }
        for lod in &self.lods {
            o.u32(lod.first_index);
            o.u32(lod.index_count);
            o.f32(lod.error_m);
            o.count(lod.sections.len(), "sections")?;
            for s in &lod.sections {
                o.u32(s.first_index);
                o.u32(s.index_count);
                o.u16(s.material_slot);
                o.u16(0);
            }
        }
        for s in &self.material_slots {
            o.str16(s)?;
        }
        o.align();
        let vo = o.pos() as u64;
        o.bytes(&self.vertex_data);
        o.align();
        let io = o.pos() as u64;
        o.bytes(&self.index_data);
        o.patch_u64(offsets_at, vo);
        o.patch_u64(offsets_at + 8, io);
        Ok(())
    }

    fn decode_payload(_: ChunkKind, p: &'a [u8]) -> Result<Mesh<'a>> {
        let mut r = In::new(p);
        let vertex_count = r.u32()?;
        let vertex_stride = r.u32()?;
        let index_count = r.u32()? as usize;
        let index_format =
            IndexFormat::from_wire(r.u8()?).ok_or_else(|| Error::Corrupt("mesh: bad index format".into()))?;
        let na = r.u8()? as usize;
        let nl = r.u16()? as usize;
        let ns = r.u16()? as usize;
        r.zeros(2, "mesh reserved")?;
        let vo = r.u64()?;
        let io = r.u64()?;
        let bounds = Bounds::read(&mut r)?;
        let mut attributes = Vec::with_capacity(na);
        for _ in 0..na {
            let semantic = VertexSemantic::from_wire(r.u8()?)
                .ok_or_else(|| Error::Corrupt("mesh: bad vertex semantic".into()))?;
            let format =
                VertexFormat::from_wire(r.u8()?).ok_or_else(|| Error::Corrupt("mesh: bad vertex format".into()))?;
            attributes.push(VertexAttribute { semantic, format, offset: r.u16()? });
        }
        let mut lods = Vec::with_capacity(nl.min(r.remaining() / 16));
        for _ in 0..nl {
            let first_index = r.u32()?;
            let index_count = r.u32()?;
            let error_m = r.f32()?;
            let n = r.count(12)?;
            let mut sections = Vec::with_capacity(n);
            for _ in 0..n {
                let s = MeshSection { first_index: r.u32()?, index_count: r.u32()?, material_slot: r.u16()? };
                r.zeros(2, "section reserved")?;
                sections.push(s);
            }
            lods.push(MeshLod { first_index, index_count, error_m, sections });
        }
        let mut material_slots = Vec::with_capacity(ns.min(r.remaining() / 2));
        for _ in 0..ns {
            material_slots.push(r.str16()?.to_string());
        }
        r.align()?;
        if r.pos() as u64 != vo {
            return Err(Error::Corrupt(format!("mesh: vertex offset {vo}, expected {}", r.pos())));
        }
        let vlen = (vertex_count as usize)
            .checked_mul(vertex_stride as usize)
            .ok_or_else(|| Error::Corrupt("mesh: vertex size overflow".into()))?;
        let vertex_data = r.take(vlen)?;
        r.align()?;
        if r.pos() as u64 != io {
            return Err(Error::Corrupt(format!("mesh: index offset {io}, expected {}", r.pos())));
        }
        let ilen = index_count
            .checked_mul(index_format.size())
            .ok_or_else(|| Error::Corrupt("mesh: index size overflow".into()))?;
        let index_data = r.take(ilen)?;
        r.finish()?;
        let m = Mesh {
            attributes,
            vertex_stride,
            vertex_count,
            vertex_data: Cow::Borrowed(vertex_data),
            index_format,
            index_data: Cow::Borrowed(index_data),
            lods,
            bounds,
            material_slots,
        };
        m.validate().map_err(|e| Error::Corrupt(e.to_string()))?;
        Ok(m)
    }
}
