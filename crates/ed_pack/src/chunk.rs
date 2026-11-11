//! One chunk: 64-byte header, metadata block (licence ids and provenance), payload.
//!
//! ```text
//! 0   magic "EWCK"            4   kind u16          6   schema u16
//! 8   gen_version u32         12  data_version u32  16  codec u8   17 flags u8 (0)
//! 18  reserved [6] (0)        24  raw_len u64 (body length = meta + payload)
//! 32  key [16] (0, see pack.md decisions)           48  reserved2 [16] (0)
//! 64  meta: meta_len u32, licence_count u16, provenance_count u16,
//!           licence ids (u16 len + UTF-8, sorted, unique), provenance (same),
//!           zero padding to a multiple of 16
//! 64+meta_len  payload (16-aligned), raw_len - meta_len bytes
//! ```
//! The chunk hash is BLAKE3 over all of it.

use std::borrow::Cow;
use std::fmt;

use crate::error::{Error, Result};
use crate::hash::ChunkHash;
use crate::key::ChunkKind;
use crate::wire::{In, Out};

pub const CHUNK_MAGIC: [u8; 4] = *b"EWCK";
pub const CHUNK_HEADER_LEN: usize = 64;

/// Body codec. Only `Raw` is implemented (see pack.md decisions); `Zstd` is recognised and
/// rejected with [`Error::UnsupportedCodec`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Codec {
    Raw = 0,
    Zstd = 1,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ChunkHeader {
    pub kind: ChunkKind,
    pub schema: u16,
    pub gen_version: u32,
    pub data_version: u32,
    pub codec: Codec,
    pub raw_len: u64,
}

/// Versions and provenance carried by every chunk. Licence ids are dataset registry ids
/// (`data/registry/<id>.toml`); provenance entries are free-form source descriptions
/// (`"openstreetmap:way/123"`, `"assets/hubs/hall.glb"`). Both are stored sorted and unique.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ChunkMeta {
    pub gen_version: u32,
    pub data_version: u32,
    pub licences: Vec<String>,
    pub provenance: Vec<String>,
}

impl ChunkMeta {
    pub fn new(gen_version: u32, data_version: u32) -> ChunkMeta {
        ChunkMeta { gen_version, data_version, ..Default::default() }
    }
    pub fn licence(mut self, id: &str) -> Self {
        self.licences.push(id.to_string());
        self
    }
    pub fn provenance(mut self, p: &str) -> Self {
        self.provenance.push(p.to_string());
        self
    }
}

/// A registry dataset id: `[a-z0-9][a-z0-9._-]*`, at most 64 bytes.
pub fn valid_licence_id(id: &str) -> bool {
    let b = id.as_bytes();
    !b.is_empty()
        && b.len() <= 64
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'_' | b'-'))
}

/// A typed chunk body with a versioned binary layout.
pub trait ChunkBody<'a>: Sized {
    /// Type name used in [`Error::WrongKind`].
    const NAME: &'static str;
    /// The kind this value encodes as.
    fn kind(&self) -> ChunkKind;
    /// Kinds this type decodes.
    fn accepts(kind: ChunkKind) -> bool;
    /// Append the payload (starts 16-aligned in the chunk) to `out`, which starts empty.
    fn encode_payload(&self, out: &mut Vec<u8>) -> Result<()>;
    /// Decode and validate a payload. Must never panic on any input.
    fn decode_payload(kind: ChunkKind, payload: &'a [u8]) -> Result<Self>;
}

/// An encoded, structurally validated chunk. Borrowed from a mapped pack or owned.
#[derive(Clone)]
pub struct Chunk<'a> {
    bytes: Cow<'a, [u8]>,
    header: ChunkHeader,
    meta: ChunkMeta,
    meta_len: usize,
    hash: ChunkHash,
}

impl fmt::Debug for Chunk<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Chunk")
            .field("kind", &self.header.kind)
            .field("len", &self.bytes.len())
            .field("hash", &self.hash)
            .finish()
    }
}

impl PartialEq for Chunk<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}
impl Eq for Chunk<'_> {}

fn sorted_unique(v: &[String]) -> Vec<&str> {
    let mut s: Vec<&str> = v.iter().map(String::as_str).collect();
    s.sort_unstable();
    s.dedup();
    s
}

impl Chunk<'static> {
    /// Build a chunk of `kind` (at the kind's current schema) around an encoded payload.
    pub fn build(kind: ChunkKind, meta: &ChunkMeta, payload: &[u8]) -> Result<Chunk<'static>> {
        let licences = sorted_unique(&meta.licences);
        let provenance = sorted_unique(&meta.provenance);
        if let Some(bad) = licences.iter().find(|l| !valid_licence_id(l)) {
            return Err(Error::Invalid(format!("licence id {bad:?} is not a registry id")));
        }
        if provenance.iter().any(|p| p.is_empty()) {
            return Err(Error::Invalid("empty provenance entry".into()));
        }
        let (nl, np) = (u16::try_from(licences.len()), u16::try_from(provenance.len()));
        let (Ok(nl), Ok(np)) = (nl, np) else {
            return Err(Error::Invalid("more than 65535 licence or provenance entries".into()));
        };

        let mut buf = Vec::with_capacity(CHUNK_HEADER_LEN + 64 + payload.len());
        let mut o = Out::new(&mut buf);
        o.bytes(&CHUNK_MAGIC);
        o.u16(kind.id());
        o.u16(kind.schema());
        o.u32(meta.gen_version);
        o.u32(meta.data_version);
        o.u8(Codec::Raw as u8);
        o.u8(0);
        o.bytes(&[0; 6]);
        o.u64(0); // raw_len, patched below
        o.bytes(&[0; 32]); // key + reserved2
        debug_assert_eq!(o.pos(), CHUNK_HEADER_LEN);
        o.u32(0); // meta_len, patched below
        o.u16(nl);
        o.u16(np);
        for s in licences.iter().chain(provenance.iter()) {
            o.str16(s)?;
        }
        o.align();
        let meta_len = o.pos() - CHUNK_HEADER_LEN;
        o.patch_u32(CHUNK_HEADER_LEN, meta_len as u32);
        o.bytes(payload);
        let raw_len = (o.pos() - CHUNK_HEADER_LEN) as u64;
        o.patch_u64(24, raw_len);
        let hash = ChunkHash::of(&buf);
        let meta = ChunkMeta {
            gen_version: meta.gen_version,
            data_version: meta.data_version,
            licences: licences.iter().map(|s| s.to_string()).collect(),
            provenance: provenance.iter().map(|s| s.to_string()).collect(),
        };
        let header = ChunkHeader {
            kind,
            schema: kind.schema(),
            gen_version: meta.gen_version,
            data_version: meta.data_version,
            codec: Codec::Raw,
            raw_len,
        };
        Ok(Chunk { bytes: Cow::Owned(buf), header, meta, meta_len, hash })
    }

    /// Encode a typed body.
    pub fn encode<'b, B: ChunkBody<'b>>(body: &B, meta: &ChunkMeta) -> Result<Chunk<'static>> {
        let mut payload = Vec::new();
        body.encode_payload(&mut payload)?;
        Chunk::build(body.kind(), meta, &payload)
    }
}

impl<'a> Chunk<'a> {
    /// Parse and structurally validate chunk bytes, computing their hash.
    pub fn parse(bytes: impl Into<Cow<'a, [u8]>>) -> Result<Chunk<'a>> {
        let bytes = bytes.into();
        let hash = ChunkHash::of(&bytes);
        Self::parse_with_hash(bytes, hash)
    }

    /// Check the hash first (a flipped bit anywhere fails here), then parse.
    pub fn parse_verified(bytes: impl Into<Cow<'a, [u8]>>, expected: &ChunkHash) -> Result<Chunk<'a>> {
        let bytes = bytes.into();
        let actual = ChunkHash::of(&bytes);
        if actual != *expected {
            return Err(Error::HashMismatch { expected: *expected, actual });
        }
        Self::parse_with_hash(bytes, actual)
    }

    fn parse_with_hash(bytes: Cow<'a, [u8]>, hash: ChunkHash) -> Result<Chunk<'a>> {
        let (header, meta, meta_len) = parse_structure(&bytes)?;
        Ok(Chunk { bytes, header, meta, meta_len, hash })
    }

    pub fn hash(&self) -> ChunkHash {
        self.hash
    }
    pub fn header(&self) -> &ChunkHeader {
        &self.header
    }
    pub fn kind(&self) -> ChunkKind {
        self.header.kind
    }
    pub fn schema(&self) -> u16 {
        self.header.schema
    }
    pub fn meta(&self) -> &ChunkMeta {
        &self.meta
    }
    pub fn licences(&self) -> &[String] {
        &self.meta.licences
    }
    pub fn provenance(&self) -> &[String] {
        &self.meta.provenance
    }
    /// The whole encoded chunk, as stored in a pack and as hashed.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// The body payload. Starts at a 16-aligned offset in the chunk (and so in a pack file).
    pub fn payload(&self) -> &[u8] {
        &self.bytes[CHUNK_HEADER_LEN + self.meta_len..]
    }
    /// Offset of the payload from the start of the chunk.
    pub fn payload_offset(&self) -> usize {
        CHUNK_HEADER_LEN + self.meta_len
    }
    pub fn is_borrowed(&self) -> bool {
        matches!(self.bytes, Cow::Borrowed(_))
    }
    pub fn into_owned(self) -> Chunk<'static> {
        Chunk {
            bytes: Cow::Owned(self.bytes.into_owned()),
            header: self.header,
            meta: self.meta,
            meta_len: self.meta_len,
            hash: self.hash,
        }
    }

    /// Decode the payload as `B`. Rejects other kinds with [`Error::WrongKind`]; the schema
    /// was already checked by [`Chunk::parse`].
    pub fn decode<'b, B: ChunkBody<'b>>(&'b self) -> Result<B> {
        if !B::accepts(self.kind()) {
            return Err(Error::WrongKind { expected: B::NAME, found: self.kind() });
        }
        B::decode_payload(self.kind(), self.payload())
    }
}

fn parse_structure(b: &[u8]) -> Result<(ChunkHeader, ChunkMeta, usize)> {
    let mut r = In::new(b);
    if r.arr::<4>()? != CHUNK_MAGIC {
        return Err(Error::BadMagic { expected: CHUNK_MAGIC });
    }
    let kind_raw = r.u16()?;
    let kind = ChunkKind::from_u16(kind_raw).ok_or(Error::UnknownKind(kind_raw))?;
    let schema = r.u16()?;
    if schema != kind.schema() {
        return Err(Error::UnsupportedSchema { kind, found: schema, supported: kind.schema() });
    }
    let gen_version = r.u32()?;
    let data_version = r.u32()?;
    let codec = match r.u8()? {
        0 => Codec::Raw,
        c => return Err(Error::UnsupportedCodec(c)),
    };
    if r.u8()? != 0 {
        return Err(Error::Corrupt("chunk flags must be zero".into()));
    }
    r.zeros(6, "chunk reserved field")?;
    let raw_len = r.u64()?;
    r.zeros(32, "chunk key/reserved field")?;
    let body_len = (b.len() - CHUNK_HEADER_LEN) as u64;
    if raw_len != body_len {
        return Err(Error::Corrupt(format!("raw_len {raw_len} but body is {body_len} bytes")));
    }
    let meta_len = r.u32()? as usize;
    if meta_len < 8 || !meta_len.is_multiple_of(16) || meta_len as u64 > raw_len {
        return Err(Error::Corrupt(format!("bad meta_len {meta_len}")));
    }
    let nl = r.u16()? as usize;
    let np = r.u16()? as usize;
    let mut read_list = |n: usize, what: &str| -> Result<Vec<String>> {
        let mut v: Vec<String> = Vec::with_capacity(n.min(r.remaining() / 2));
        for _ in 0..n {
            let s = r.str16()?;
            if v.last().is_some_and(|p| p.as_str() >= s) {
                return Err(Error::Corrupt(format!("{what} not sorted and unique")));
            }
            v.push(s.to_string());
        }
        Ok(v)
    };
    let licences = read_list(nl, "licence ids")?;
    let provenance = read_list(np, "provenance")?;
    if let Some(l) = licences.iter().find(|l| !valid_licence_id(l)) {
        return Err(Error::Corrupt(format!("invalid licence id {l:?}")));
    }
    if r.pos() > CHUNK_HEADER_LEN + meta_len {
        return Err(Error::Corrupt("meta block overruns meta_len".into()));
    }
    r.zeros(CHUNK_HEADER_LEN + meta_len - r.pos(), "meta padding")?;
    let header = ChunkHeader { kind, schema, gen_version, data_version, codec, raw_len };
    Ok((header, ChunkMeta { gen_version, data_version, licences, provenance }, meta_len))
}
