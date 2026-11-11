//! Reserved kinds (terrain, layers, proxy, shells, nav, ground, source, skeleton, clip):
//! schema 0, opaque payload, until their generators define a layout.

use std::borrow::Cow;

use crate::chunk::ChunkBody;
use crate::error::{Error, Result};
use crate::key::ChunkKind;

#[derive(Clone, Debug, PartialEq)]
pub struct Opaque<'a> {
    pub kind: ChunkKind,
    pub data: Cow<'a, [u8]>,
}

impl<'a> ChunkBody<'a> for Opaque<'a> {
    const NAME: &'static str = "Opaque";
    fn kind(&self) -> ChunkKind {
        self.kind
    }
    fn accepts(kind: ChunkKind) -> bool {
        kind.is_reserved()
    }
    fn encode_payload(&self, buf: &mut Vec<u8>) -> Result<()> {
        if !self.kind.is_reserved() {
            return Err(Error::Invalid(format!("{:?} has a typed body; it cannot be encoded as opaque", self.kind)));
        }
        buf.extend_from_slice(&self.data);
        Ok(())
    }
    fn decode_payload(kind: ChunkKind, p: &'a [u8]) -> Result<Opaque<'a>> {
        Ok(Opaque { kind, data: Cow::Borrowed(p) })
    }
}
