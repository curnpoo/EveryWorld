//! `layer` (kind 32): one override-layer file for one L10 node, compiled.
//!
//! Schema 1 payload: the canonical TOML text written by
//! [`crate::layers::LayerFile::to_toml`], UTF-8, no length prefix. Canonical text makes the
//! chunk hash a function of the operations, not of the author's formatting.

use std::borrow::Cow;

use crate::chunk::{Chunk, ChunkBody, ChunkMeta};
use crate::error::{Error, Result};
use crate::key::ChunkKind;
use crate::layers::LayerFile;

#[derive(Clone, Debug, PartialEq)]
pub struct LayerText<'a> {
    pub text: Cow<'a, str>,
}

impl LayerText<'_> {
    /// Parse the text back into operations.
    pub fn parse(&self) -> Result<LayerFile> {
        LayerFile::parse(&self.text).map_err(|e| Error::Corrupt(format!("layer chunk: {e}")))
    }
}

impl LayerFile {
    /// Compile to a `layer` chunk (canonical text).
    pub fn to_chunk(&self, meta: &ChunkMeta) -> Result<Chunk<'static>> {
        Chunk::encode(&LayerText { text: Cow::Owned(self.to_toml()) }, meta)
    }
}

impl<'a> ChunkBody<'a> for LayerText<'a> {
    const NAME: &'static str = "LayerText";
    fn kind(&self) -> ChunkKind {
        ChunkKind::Layer
    }
    fn accepts(kind: ChunkKind) -> bool {
        kind == ChunkKind::Layer
    }
    fn encode_payload(&self, buf: &mut Vec<u8>) -> Result<()> {
        buf.extend_from_slice(self.text.as_bytes());
        Ok(())
    }
    fn decode_payload(_: ChunkKind, p: &'a [u8]) -> Result<LayerText<'a>> {
        let text = std::str::from_utf8(p).map_err(|_| Error::Corrupt("layer chunk: invalid UTF-8".into()))?;
        Ok(LayerText { text: Cow::Borrowed(text) })
    }
}
