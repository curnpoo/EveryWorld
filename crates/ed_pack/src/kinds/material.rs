//! `material` (kind 19): model, named parameters and texture slots.
//!
//! Schema 1 payload:
//! ```text
//! model u8, alpha_mode u8, double_sided u8 (0/1), reserved u8 (0), alpha_cutoff f32,
//! param_count u32, texture_count u32,
//! params:   param_count x (u16 len + name, type u8 (0 f32, 1 vec2, 2 vec3, 3 vec4), f32 x n)
//! textures: texture_count x (u16 len + slot name, AssetId [32])
//! ```
//! Names are sorted and unique (byte order), so one material has one encoding.

use std::collections::BTreeMap;

use super::{finite, read_asset};
use crate::chunk::ChunkBody;
use crate::error::{Error, Result};
use crate::hash::AssetId;
use crate::key::ChunkKind;
use crate::wire::{In, Out};

wire_enum! {
    pub enum MaterialModel: u8 { Pbr = 0, Toon = 1 }
}
wire_enum! {
    pub enum AlphaMode: u8 { Opaque = 0, Mask = 1, Blend = 2 }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Param {
    Float(f32),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
}

impl Param {
    fn values(&self) -> &[f32] {
        match self {
            Param::Float(v) => std::slice::from_ref(v),
            Param::Vec2(v) => v,
            Param::Vec3(v) => v,
            Param::Vec4(v) => v,
        }
    }
    fn tag(&self) -> u8 {
        self.values().len() as u8 - 1
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Material {
    pub model: MaterialModel,
    pub alpha_mode: AlphaMode,
    pub alpha_cutoff: f32,
    pub double_sided: bool,
    /// Model parameters by name (`base_colour`, `roughness`, `band_count`, ...).
    pub params: BTreeMap<String, Param>,
    /// Texture slots by name (`albedo`, `normal`, `orm`, ...), referencing texture assets.
    pub textures: BTreeMap<String, AssetId>,
}

impl Material {
    pub fn new(model: MaterialModel) -> Material {
        Material {
            model,
            alpha_mode: AlphaMode::Opaque,
            alpha_cutoff: 0.5,
            double_sided: false,
            params: BTreeMap::new(),
            textures: BTreeMap::new(),
        }
    }
    pub fn validate(&self) -> Result<()> {
        finite(&[self.alpha_cutoff], "material alpha cutoff")?;
        for (k, p) in &self.params {
            if k.is_empty() {
                return Err(Error::Invalid("material: empty parameter name".into()));
            }
            finite(p.values(), "material parameter")?;
        }
        if self.textures.keys().any(|k| k.is_empty()) {
            return Err(Error::Invalid("material: empty texture slot name".into()));
        }
        Ok(())
    }
}

impl<'a> ChunkBody<'a> for Material {
    const NAME: &'static str = "Material";
    fn kind(&self) -> ChunkKind {
        ChunkKind::Material
    }
    fn accepts(kind: ChunkKind) -> bool {
        kind == ChunkKind::Material
    }

    fn encode_payload(&self, buf: &mut Vec<u8>) -> Result<()> {
        self.validate()?;
        let mut o = Out::new(buf);
        o.u8(self.model.to_wire());
        o.u8(self.alpha_mode.to_wire());
        o.u8(self.double_sided as u8);
        o.u8(0);
        o.f32(self.alpha_cutoff);
        o.count(self.params.len(), "params")?;
        o.count(self.textures.len(), "textures")?;
        for (k, p) in &self.params {
            o.str16(k)?;
            o.u8(p.tag());
            o.f32s(p.values());
        }
        for (k, a) in &self.textures {
            o.str16(k)?;
            o.bytes(&a.0);
        }
        Ok(())
    }

    fn decode_payload(_: ChunkKind, p: &'a [u8]) -> Result<Material> {
        let mut r = In::new(p);
        let model = MaterialModel::from_wire(r.u8()?).ok_or_else(|| Error::Corrupt("material: bad model".into()))?;
        let alpha_mode =
            AlphaMode::from_wire(r.u8()?).ok_or_else(|| Error::Corrupt("material: bad alpha mode".into()))?;
        let double_sided = match r.u8()? {
            0 => false,
            1 => true,
            _ => return Err(Error::Corrupt("material: bad double_sided".into())),
        };
        r.zeros(1, "material reserved")?;
        let alpha_cutoff = r.f32()?;
        let np = r.count(7)?;
        let nt = r.count(34)?;
        let mut params = BTreeMap::new();
        let mut last: Option<&str> = None;
        for _ in 0..np {
            let k = r.str16()?;
            if last.is_some_and(|l| l >= k) {
                return Err(Error::Corrupt("material: parameter names not sorted".into()));
            }
            last = Some(k);
            let v = match r.u8()? {
                0 => Param::Float(r.f32()?),
                1 => Param::Vec2(r.f32x()?),
                2 => Param::Vec3(r.f32x()?),
                3 => Param::Vec4(r.f32x()?),
                t => return Err(Error::Corrupt(format!("material: bad parameter type {t}"))),
            };
            params.insert(k.to_string(), v);
        }
        let mut textures = BTreeMap::new();
        let mut last: Option<&str> = None;
        for _ in 0..nt {
            let k = r.str16()?;
            if last.is_some_and(|l| l >= k) {
                return Err(Error::Corrupt("material: texture slots not sorted".into()));
            }
            last = Some(k);
            textures.insert(k.to_string(), read_asset(&mut r)?);
        }
        r.finish()?;
        let m = Material { model, alpha_mode, alpha_cutoff, double_sided, params, textures };
        m.validate().map_err(|e| Error::Corrupt(e.to_string()))?;
        Ok(m)
    }
}
