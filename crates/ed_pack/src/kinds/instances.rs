//! `instances` (kind 5, R0): per-node instance table, sorted by `ObjectId`.
//!
//! Schema 1 payload:
//! ```text
//! asset_count u32, instance_count u32, reserved u64 (0)
//! assets: asset_count x AssetId [32], sorted, unique (every mesh/prefab/material referenced)
//! zero padding to 16
//! instances: instance_count x 80-byte records, sorted by object id, unique:
//!   object u128, translation f32x3, rotation f32x4 (x,y,z,w), scale f32x3,
//!   asset u32 (index into assets), material u32 (index, 0xFFFFFFFF = the asset's own),
//!   flags u32, variant u16, reserved u16 (0), reserved [8] (0)
//! ```
//! Transforms are relative to the node origin in the node frame (east, north, up).

use std::collections::BTreeSet;

use super::{Transform, read_asset};
use crate::chunk::ChunkBody;
use crate::error::{Error, Result};
use crate::hash::AssetId;
use crate::key::ChunkKind;
use crate::wire::{In, Out};
use ed_geo::ObjectId;

pub const INSTANCE_RECORD_LEN: usize = 80;
const NONE: u32 = u32::MAX;

#[derive(Clone, Debug, PartialEq)]
pub struct Instance {
    pub object: ObjectId,
    pub transform: Transform,
    /// Mesh or prefab asset.
    pub asset: AssetId,
    /// Material override; `None` keeps the asset's own.
    pub material: Option<AssetId>,
    pub variant: u16,
    pub flags: u32,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Instances {
    pub instances: Vec<Instance>,
}

impl Instances {
    /// Sort by `ObjectId` (the required order).
    pub fn sorted(mut instances: Vec<Instance>) -> Instances {
        instances.sort_by_key(|i| i.object);
        Instances { instances }
    }
    pub fn validate(&self) -> Result<()> {
        if self.instances.windows(2).any(|w| w[0].object >= w[1].object) {
            return Err(Error::Invalid("instances: not sorted by ObjectId or duplicate id".into()));
        }
        self.instances.iter().try_for_each(|i| i.transform.validate())
    }
}

impl<'a> ChunkBody<'a> for Instances {
    const NAME: &'static str = "Instances";
    fn kind(&self) -> ChunkKind {
        ChunkKind::Instances
    }
    fn accepts(kind: ChunkKind) -> bool {
        kind == ChunkKind::Instances
    }

    fn encode_payload(&self, buf: &mut Vec<u8>) -> Result<()> {
        self.validate()?;
        let table: Vec<AssetId> = self
            .instances
            .iter()
            .flat_map(|i| std::iter::once(i.asset).chain(i.material))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let index = |a: &AssetId| table.binary_search(a).map(|i| i as u32).unwrap_or(NONE);
        let mut o = Out::new(buf);
        o.count(table.len(), "assets")?;
        o.count(self.instances.len(), "instances")?;
        o.u64(0);
        for a in &table {
            o.bytes(&a.0);
        }
        o.align();
        for i in &self.instances {
            o.u128(i.object.0);
            i.transform.write(&mut o);
            o.u32(index(&i.asset));
            o.u32(i.material.as_ref().map_or(NONE, index));
            o.u32(i.flags);
            o.u16(i.variant);
            o.u16(0);
            o.u64(0);
        }
        Ok(())
    }

    fn decode_payload(_: ChunkKind, p: &'a [u8]) -> Result<Instances> {
        let mut r = In::new(p);
        let na = r.count(32)?;
        let ni = r.u32()? as usize;
        r.zeros(8, "instances reserved")?;
        let table = (0..na).map(|_| read_asset(&mut r)).collect::<Result<Vec<_>>>()?;
        if table.windows(2).any(|w| w[0] >= w[1]) {
            return Err(Error::Corrupt("instances: asset table not sorted".into()));
        }
        r.align()?;
        if ni.saturating_mul(INSTANCE_RECORD_LEN) != r.remaining() {
            return Err(Error::Corrupt(format!("instances: {ni} records do not match {} bytes", r.remaining())));
        }
        let get = |i: u32| table.get(i as usize).copied().ok_or_else(|| Error::Corrupt("instances: bad asset index".into()));
        let mut instances = Vec::with_capacity(ni);
        for _ in 0..ni {
            let object = ObjectId(r.u128()?);
            let transform = Transform::read(&mut r)?;
            let asset = get(r.u32()?)?;
            let material = match r.u32()? {
                NONE => None,
                m => Some(get(m)?),
            };
            let flags = r.u32()?;
            let variant = r.u16()?;
            r.zeros(10, "instance reserved")?;
            instances.push(Instance { object, transform, asset, material, variant, flags });
        }
        r.finish()?;
        let out = Instances { instances };
        out.validate().map_err(|e| Error::Corrupt(e.to_string()))?;
        Ok(out)
    }
}
