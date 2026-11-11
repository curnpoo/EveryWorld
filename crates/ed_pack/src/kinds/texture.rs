//! `texture` (kind 18): one 2D texture (optionally an array) with its mip chain.
//!
//! Schema 1 payload:
//! ```text
//! format u8, role u8, layers u16, width u32, height u32, mip_count u8, reserved [7] (0),
//! page_id u64
//! mips: mip_count x (offset u64, len u64)     offsets relative to the payload start
//! each mip's data 16-aligned, all layers of one mip contiguous, rows tightly packed
//! ```
//! `page_id` is a stable id chosen by the baker so that Phase 2 virtual texturing can address
//! the texture as pages without changing materials (P1-D13). Virtual-texture page tables for
//! textures above 4096 are not part of schema 1.

use std::borrow::Cow;

use crate::chunk::ChunkBody;
use crate::error::{Error, Result};
use crate::key::ChunkKind;
use crate::wire::{In, Out};

wire_enum! {
    /// Stored texel format. Block formats are 4 x 4.
    pub enum TextureFormat: u8 {
        Rgba8Unorm = 1, Rgba16Float = 2, Bc7 = 3, Bc4 = 4, Bc5 = 5, R8Unorm = 6, Rg8Unorm = 7,
    }
}

impl TextureFormat {
    /// `(block width, block height, bytes per block)`.
    pub fn block(self) -> (u32, u32, u32) {
        use TextureFormat::*;
        match self {
            Rgba8Unorm => (1, 1, 4),
            Rgba16Float => (1, 1, 8),
            Bc7 | Bc5 => (4, 4, 16),
            Bc4 => (4, 4, 8),
            R8Unorm => (1, 1, 1),
            Rg8Unorm => (1, 1, 2),
        }
    }
    /// Bytes of one layer of a `w x h` image.
    pub fn image_bytes(self, w: u32, h: u32) -> u64 {
        let (bw, bh, bb) = self.block();
        (w.div_ceil(bw) as u64) * (h.div_ceil(bh) as u64) * bb as u64
    }
}

wire_enum! {
    /// How texel values are interpreted (P1-D13 "colour space per role").
    pub enum ColourRole: u8 {
        /// Colour in sRGB encoding (albedo, LDR emissive). Formats: Rgba8Unorm, Bc7.
        Srgb = 0,
        /// Linear data (roughness, metalness, occlusion, masks, heights).
        Linear = 1,
        /// Tangent-space normal map.
        Normal = 2,
        /// Linear HDR colour. Format: Rgba16Float.
        Hdr = 3,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Texture<'a> {
    pub format: TextureFormat,
    pub role: ColourRole,
    pub width: u32,
    pub height: u32,
    pub layers: u16,
    pub page_id: u64,
    /// Mip 0 first. Each holds all layers of that level.
    pub mips: Vec<Cow<'a, [u8]>>,
}

pub const MAX_TEXTURE_SIZE: u32 = 65536;

impl Texture<'_> {
    pub fn mip_size(&self, level: usize) -> (u32, u32) {
        let s = |v: u32| (v.checked_shr(level as u32).unwrap_or(0)).max(1);
        (s(self.width), s(self.height))
    }
    pub fn mip_bytes(&self, level: usize) -> u64 {
        let (w, h) = self.mip_size(level);
        self.format.image_bytes(w, h) * self.layers as u64
    }
    pub fn into_owned(self) -> Texture<'static> {
        Texture { mips: self.mips.into_iter().map(|m| Cow::Owned(m.into_owned())).collect(), ..self }
    }
    pub fn validate(&self) -> Result<()> {
        let bad = |s: String| Err(Error::Invalid(format!("texture: {s}")));
        if !(1..=MAX_TEXTURE_SIZE).contains(&self.width) || !(1..=MAX_TEXTURE_SIZE).contains(&self.height) {
            return bad(format!("size {}x{} out of range", self.width, self.height));
        }
        if self.layers == 0 {
            return bad("zero layers".into());
        }
        let max_mips = 32 - self.width.max(self.height).leading_zeros() as usize;
        if self.mips.is_empty() || self.mips.len() > max_mips {
            return bad(format!("{} mips, expected 1..={max_mips}", self.mips.len()));
        }
        let ok_role = match self.role {
            ColourRole::Srgb => matches!(self.format, TextureFormat::Rgba8Unorm | TextureFormat::Bc7),
            ColourRole::Hdr => self.format == TextureFormat::Rgba16Float,
            ColourRole::Linear | ColourRole::Normal => true,
        };
        if !ok_role {
            return bad(format!("format {:?} cannot carry role {:?}", self.format, self.role));
        }
        for (i, m) in self.mips.iter().enumerate() {
            if m.len() as u64 != self.mip_bytes(i) {
                return bad(format!("mip {i} is {} bytes, expected {}", m.len(), self.mip_bytes(i)));
            }
        }
        Ok(())
    }
}

impl<'a> ChunkBody<'a> for Texture<'a> {
    const NAME: &'static str = "Texture";
    fn kind(&self) -> ChunkKind {
        ChunkKind::Texture
    }
    fn accepts(kind: ChunkKind) -> bool {
        kind == ChunkKind::Texture
    }

    fn encode_payload(&self, buf: &mut Vec<u8>) -> Result<()> {
        self.validate()?;
        let mut o = Out::new(buf);
        o.u8(self.format.to_wire());
        o.u8(self.role.to_wire());
        o.u16(self.layers);
        o.u32(self.width);
        o.u32(self.height);
        o.u8(self.mips.len() as u8);
        o.bytes(&[0; 7]);
        o.u64(self.page_id);
        let table = o.pos();
        for _ in &self.mips {
            o.u64(0);
            o.u64(0);
        }
        for (i, m) in self.mips.iter().enumerate() {
            o.align();
            let at = o.pos() as u64;
            o.bytes(m);
            o.patch_u64(table + 16 * i, at);
            o.patch_u64(table + 16 * i + 8, m.len() as u64);
        }
        Ok(())
    }

    fn decode_payload(_: ChunkKind, p: &'a [u8]) -> Result<Texture<'a>> {
        let mut r = In::new(p);
        let format =
            TextureFormat::from_wire(r.u8()?).ok_or_else(|| Error::Corrupt("texture: bad format".into()))?;
        let role = ColourRole::from_wire(r.u8()?).ok_or_else(|| Error::Corrupt("texture: bad role".into()))?;
        let layers = r.u16()?;
        let width = r.u32()?;
        let height = r.u32()?;
        let nm = r.u8()? as usize;
        r.zeros(7, "texture reserved")?;
        let page_id = r.u64()?;
        let mut table = Vec::with_capacity(nm);
        for _ in 0..nm {
            table.push((r.u64()?, r.u64()?));
        }
        let mut mips = Vec::with_capacity(nm);
        for (i, (off, len)) in table.into_iter().enumerate() {
            r.align()?;
            if r.pos() as u64 != off {
                return Err(Error::Corrupt(format!("texture: mip {i} offset {off}, expected {}", r.pos())));
            }
            let len = usize::try_from(len).map_err(|_| Error::Corrupt("texture: mip too large".into()))?;
            mips.push(Cow::Borrowed(r.take(len)?));
        }
        r.finish()?;
        let t = Texture { format, role, width, height, layers, page_id, mips };
        t.validate().map_err(|e| Error::Corrupt(e.to_string()))?;
        Ok(t)
    }
}
