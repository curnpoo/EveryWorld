//! `clusters` (kind 17): the optional cluster-DAG slot for a mesh (Track G). Opaque for now.
//!
//! Schema 1 payload: `mesh ChunkHash [32]` (the exact `mesh` chunk this refines), `len u64`,
//! zero padding to 16, then `len` opaque bytes. The mesh never references this chunk; a
//! renderer that ignores kind 17 draws everything from the mesh's LOD chain (pack.md, F3).

use std::borrow::Cow;

use crate::chunk::ChunkBody;
use crate::error::{Error, Result};
use crate::hash::ChunkHash;
use crate::key::ChunkKind;
use crate::wire::{In, Out};

#[derive(Clone, Debug, PartialEq)]
pub struct Clusters<'a> {
    pub mesh: ChunkHash,
    pub data: Cow<'a, [u8]>,
}

impl<'a> ChunkBody<'a> for Clusters<'a> {
    const NAME: &'static str = "Clusters";
    fn kind(&self) -> ChunkKind {
        ChunkKind::Clusters
    }
    fn accepts(kind: ChunkKind) -> bool {
        kind == ChunkKind::Clusters
    }
    fn encode_payload(&self, buf: &mut Vec<u8>) -> Result<()> {
        let mut o = Out::new(buf);
        o.bytes(&self.mesh.0);
        o.u64(self.data.len() as u64);
        o.align();
        o.bytes(&self.data);
        Ok(())
    }
    fn decode_payload(_: ChunkKind, p: &'a [u8]) -> Result<Clusters<'a>> {
        let mut r = In::new(p);
        let mesh = ChunkHash(r.arr()?);
        let len = usize::try_from(r.u64()?).map_err(|_| Error::Corrupt("clusters: length".into()))?;
        r.align()?;
        let data = r.take(len)?;
        r.finish()?;
        Ok(Clusters { mesh, data: Cow::Borrowed(data) })
    }
}
