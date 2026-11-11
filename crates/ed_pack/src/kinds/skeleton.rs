//! `skeleton` (kind 20): the joints of one skinned mesh (M3).
//!
//! Schema 1 payload:
//! ```text
//! mesh AssetId [32]           the skinned mesh whose Joints/Weights attributes index `joints`
//! joint_count u32
//! joints: joint_count x (parent u32 (0xFFFFFFFF = root), name str16, rest Transform (40 B),
//!                        inverse_bind f32 x 16 (column-major, mesh space -> joint space))
//! ```
//! Parents come before children. Joint transforms are relative to the parent joint (the root's
//! to the mesh space). Nothing about bone names is assumed (no neck or head is required);
//! poses are driven by index.

use super::{Transform, read_asset};
use crate::chunk::ChunkBody;
use crate::error::{Error, Result};
use crate::hash::AssetId;
use crate::key::ChunkKind;
use crate::wire::{In, Out};

const ROOT: u32 = u32::MAX;
/// Most joints one skeleton may have (the GPU palette is sized per skeleton).
pub const MAX_JOINTS: usize = 1024;

#[derive(Clone, Debug, PartialEq)]
pub struct Joint {
    pub parent: Option<u32>,
    pub name: String,
    pub rest: Transform,
    pub inverse_bind: [f32; 16],
}

#[derive(Clone, Debug, PartialEq)]
pub struct Skeleton {
    pub mesh: AssetId,
    pub joints: Vec<Joint>,
}

impl Skeleton {
    pub fn validate(&self) -> Result<()> {
        if self.joints.is_empty() || self.joints.len() > MAX_JOINTS {
            return Err(Error::Invalid(format!("skeleton: {} joints (1..={MAX_JOINTS})", self.joints.len())));
        }
        for (i, j) in self.joints.iter().enumerate() {
            if j.parent.is_some_and(|p| p as usize >= i) {
                return Err(Error::Invalid(format!("skeleton: joint {i} parent does not come before it")));
            }
            j.rest.validate()?;
            super::finite(&j.inverse_bind, "skeleton inverse bind")?;
        }
        Ok(())
    }
}

impl<'a> ChunkBody<'a> for Skeleton {
    const NAME: &'static str = "Skeleton";
    fn kind(&self) -> ChunkKind {
        ChunkKind::Skeleton
    }
    fn accepts(kind: ChunkKind) -> bool {
        kind == ChunkKind::Skeleton
    }
    fn encode_payload(&self, buf: &mut Vec<u8>) -> Result<()> {
        self.validate()?;
        let mut o = Out::new(buf);
        o.bytes(&self.mesh.0);
        o.count(self.joints.len(), "joints")?;
        for j in &self.joints {
            o.u32(j.parent.unwrap_or(ROOT));
            o.str16(&j.name)?;
            j.rest.write(&mut o);
            o.f32s(&j.inverse_bind);
        }
        Ok(())
    }
    fn decode_payload(_: ChunkKind, p: &'a [u8]) -> Result<Skeleton> {
        let mut r = In::new(p);
        let mesh = read_asset(&mut r)?;
        let n = r.count(4 + 2 + 40 + 64)?;
        let mut joints = Vec::with_capacity(n);
        for _ in 0..n {
            let parent = match r.u32()? {
                ROOT => None,
                v => Some(v),
            };
            let name = r.str16()?.to_string();
            let rest = Transform::read(&mut r)?;
            let inverse_bind = r.f32x::<16>()?;
            joints.push(Joint { parent, name, rest, inverse_bind });
        }
        r.finish()?;
        let s = Skeleton { mesh, joints };
        s.validate().map_err(|e| Error::Corrupt(e.to_string()))?;
        Ok(s)
    }
}
