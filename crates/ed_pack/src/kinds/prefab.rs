//! `prefab` (kind 22): a tree of mesh + material references with local transforms, plus the
//! prefab manifest (footprint, anchor/door/connection points, per-rung contributions).
//!
//! Schema 1 payload:
//! ```text
//! node_count u32, nodes: (parent u32 (0xFFFFFFFF = root), u16 len + name, transform (40 bytes),
//!                         optional mesh AssetId, optional material AssetId)
//!   optional AssetId = flag u8 (0/1) then [32] if 1; a parent index is always < its child's
//! footprint_count u32, footprint: f32x2 each (x east, y north in the prefab frame, metres)
//! point_count u32, points: (u16 len + name, kind u8, transform)
//! rung_count u32, rung contributions: (rung u8, AssetId [32]) sorted by rung, unique
//! ```

use super::{Transform, finite, read_asset, read_opt_asset, write_opt_asset};
use crate::chunk::ChunkBody;
use crate::error::{Error, Result};
use crate::hash::AssetId;
use crate::key::{ChunkKind, Rung};
use crate::wire::{In, Out};

wire_enum! {
    pub enum PointKind: u8 { Anchor = 0, Door = 1, Connection = 2, Other = 3 }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PrefabNode {
    /// Index of the parent node; `None` for a root. Always less than this node's index.
    pub parent: Option<u32>,
    pub name: String,
    /// Relative to the parent (or the prefab origin for roots).
    pub transform: Transform,
    /// A mesh asset, or a nested prefab asset.
    pub mesh: Option<AssetId>,
    pub material: Option<AssetId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PrefabPoint {
    pub name: String,
    pub kind: PointKind,
    pub transform: Transform,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Prefab {
    pub nodes: Vec<PrefabNode>,
    /// Footprint polygon (empty, or at least 3 points).
    pub footprint: Vec<[f32; 2]>,
    pub points: Vec<PrefabPoint>,
    /// What the prefab contributes to coarser rungs (shell mesh for R1, proxy for R2, ...).
    pub rung_meshes: Vec<(Rung, AssetId)>,
}

const ROOT: u32 = u32::MAX;

impl Prefab {
    pub fn validate(&self) -> Result<()> {
        let bad = |s: String| Err(Error::Invalid(format!("prefab: {s}")));
        for (i, n) in self.nodes.iter().enumerate() {
            if n.parent.is_some_and(|p| p as usize >= i) {
                return bad(format!("node {i} has parent {:?}, which does not come before it", n.parent));
            }
            n.transform.validate()?;
        }
        if self.footprint.len() == 1 || self.footprint.len() == 2 {
            return bad("footprint needs 0 or at least 3 points".into());
        }
        finite(self.footprint.as_flattened(), "prefab footprint")?;
        for p in &self.points {
            p.transform.validate()?;
        }
        if self.rung_meshes.windows(2).any(|w| w[0].0 >= w[1].0) {
            return bad("rung contributions must be sorted by rung and unique".into());
        }
        Ok(())
    }
}

impl<'a> ChunkBody<'a> for Prefab {
    const NAME: &'static str = "Prefab";
    fn kind(&self) -> ChunkKind {
        ChunkKind::Prefab
    }
    fn accepts(kind: ChunkKind) -> bool {
        kind == ChunkKind::Prefab
    }

    fn encode_payload(&self, buf: &mut Vec<u8>) -> Result<()> {
        self.validate()?;
        let mut o = Out::new(buf);
        o.count(self.nodes.len(), "nodes")?;
        for n in &self.nodes {
            o.u32(n.parent.unwrap_or(ROOT));
            o.str16(&n.name)?;
            n.transform.write(&mut o);
            write_opt_asset(&mut o, &n.mesh);
            write_opt_asset(&mut o, &n.material);
        }
        o.count(self.footprint.len(), "footprint points")?;
        o.f32s(self.footprint.as_flattened());
        o.count(self.points.len(), "points")?;
        for p in &self.points {
            o.str16(&p.name)?;
            o.u8(p.kind.to_wire());
            p.transform.write(&mut o);
        }
        o.count(self.rung_meshes.len(), "rung contributions")?;
        for (rung, a) in &self.rung_meshes {
            o.u8(*rung as u8);
            o.bytes(&a.0);
        }
        Ok(())
    }

    fn decode_payload(_: ChunkKind, p: &'a [u8]) -> Result<Prefab> {
        let mut r = In::new(p);
        let n = r.count(48)?;
        let mut nodes = Vec::with_capacity(n);
        for _ in 0..n {
            let parent = match r.u32()? {
                ROOT => None,
                v => Some(v),
            };
            let name = r.str16()?.to_string();
            let transform = Transform::read(&mut r)?;
            nodes.push(PrefabNode { parent, name, transform, mesh: read_opt_asset(&mut r)?, material: read_opt_asset(&mut r)? });
        }
        let nf = r.count(8)?;
        let footprint = (0..nf).map(|_| r.f32x()).collect::<Result<_>>()?;
        let np = r.count(43)?;
        let mut points = Vec::with_capacity(np);
        for _ in 0..np {
            let name = r.str16()?.to_string();
            let kind = PointKind::from_wire(r.u8()?).ok_or_else(|| Error::Corrupt("prefab: bad point kind".into()))?;
            points.push(PrefabPoint { name, kind, transform: Transform::read(&mut r)? });
        }
        let nr = r.count(33)?;
        let mut rung_meshes = Vec::with_capacity(nr);
        for _ in 0..nr {
            let rung = Rung::from_u8(r.u8()?).ok_or_else(|| Error::Corrupt("prefab: bad rung".into()))?;
            rung_meshes.push((rung, read_asset(&mut r)?));
        }
        r.finish()?;
        let pf = Prefab { nodes, footprint, points, rung_meshes };
        pf.validate().map_err(|e| Error::Corrupt(e.to_string()))?;
        Ok(pf)
    }
}
