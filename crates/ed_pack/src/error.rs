//! Errors for chunks, packs and sources. Corrupt input always becomes one of these.

use crate::{ChunkHash, ChunkKey, ChunkKind};

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("i/o: {0}")]
    Io(#[from] std::io::Error),
    #[error("truncated at byte {offset}: needed {needed} bytes, {available} available")]
    Truncated { offset: usize, needed: usize, available: usize },
    #[error("bad magic: expected {expected:?}")]
    BadMagic { expected: [u8; 4] },
    #[error("unsupported pack version {0}")]
    UnsupportedPackVersion(u32),
    #[error("unknown chunk kind {0}")]
    UnknownKind(u16),
    #[error("chunk kind {kind:?} has schema {found}; this reader implements schema {supported}")]
    UnsupportedSchema { kind: ChunkKind, found: u16, supported: u16 },
    #[error("wrong chunk kind: a {found:?} chunk cannot be decoded as {expected}")]
    WrongKind { expected: &'static str, found: ChunkKind },
    #[error("unsupported codec {0}")]
    UnsupportedCodec(u8),
    #[error("hash mismatch: expected {expected}, got {actual}")]
    HashMismatch { expected: ChunkHash, actual: ChunkHash },
    #[error("corrupt: {0}")]
    Corrupt(String),
    #[error("invalid: {0}")]
    Invalid(String),
    #[error("key {key:?} already maps to {existing}, refusing {new}")]
    DuplicateKey { key: ChunkKey, existing: ChunkHash, new: ChunkHash },
    #[error("key {key:?} names kind {:?} but the chunk is {chunk:?}", key.kind())]
    KeyKindMismatch { key: ChunkKey, chunk: ChunkKind },
    #[error("no chunk for key {0:?}")]
    KeyNotFound(ChunkKey),
    #[error("no chunk with hash {0}")]
    ChunkNotFound(ChunkHash),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    /// True for "not here" answers, which a [`crate::Mounts`] stack treats as "ask the next".
    pub fn is_not_found(&self) -> bool {
        matches!(self, Error::KeyNotFound(_) | Error::ChunkNotFound(_))
    }
}
