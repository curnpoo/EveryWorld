//! The `.ewpack` file: writer and random-access reader.
//!
//! ```text
//! header (64 bytes)
//!   0  magic "EWPK"   4  version u32 (1)   8  manifest_offset u64   16 manifest_len u64
//!   24 manifest_hash [32] (BLAKE3 of the manifest bytes)            56 reserved [8] (0)
//! chunks (from byte 64): each chunk's bytes, zero-padded to 16; each distinct chunk once,
//!   ordered by the smallest key that references it
//! manifest (at manifest_offset, 16-aligned; ends the file)
//!   0  magic "EWMF"   4  entry_count u32   8  data_version u32   12 gen_version u32
//!   16 licence_count u32   20 reserved u32 (0)   24 reserved u64 (0)
//!   32 entries: entry_count x 88 bytes, strictly ascending by key bytes:
//!        key [40] (see key.rs), chunk hash [32], offset u64, len u32, reserved u32 (0)
//!   then licence ids: licence_count x (u16 len + UTF-8), sorted, unique; the union of every
//!   chunk's licence ids and any the writer added
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::sync::Arc;

use crate::chunk::{CHUNK_HEADER_LEN, Chunk, valid_licence_id};
use crate::error::{Error, Result};
use crate::hash::ChunkHash;
use crate::key::{ChunkKey, KEY_LEN, node_prefix};
use crate::wire::{ALIGN, In, Out, pad_len};
use ed_geo::NodeId;

pub const PACK_MAGIC: [u8; 4] = *b"EWPK";
pub const MANIFEST_MAGIC: [u8; 4] = *b"EWMF";
pub const PACK_VERSION: u32 = 1;
pub const PACK_HEADER_LEN: usize = 64;
pub const MANIFEST_HEADER_LEN: usize = 32;
pub const ENTRY_LEN: usize = 88;

/// Pack-level metadata from the manifest.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PackMeta {
    pub data_version: u32,
    pub gen_version: u32,
    /// Registry dataset ids of everything in the pack (`cargo xtask licence-gate` input).
    pub licences: Vec<String>,
}

/// What [`PackWriter::write_to`] produced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PackSummary {
    /// BLAKE3 of the whole file.
    pub file_hash: ChunkHash,
    pub bytes: u64,
    pub keys: usize,
    pub chunks: usize,
}

/// Collects `(key, chunk)` pairs in any order and writes one canonical file.
#[derive(Default)]
pub struct PackWriter {
    data_version: u32,
    gen_version: u32,
    extra_licences: BTreeSet<String>,
    keys: BTreeMap<[u8; KEY_LEN], (ChunkKey, ChunkHash)>,
    chunks: BTreeMap<ChunkHash, Chunk<'static>>,
}

impl PackWriter {
    pub fn new(data_version: u32, gen_version: u32) -> PackWriter {
        PackWriter { data_version, gen_version, ..Default::default() }
    }

    /// Declare a dataset id even if no chunk names it.
    pub fn add_licence(&mut self, id: &str) -> Result<()> {
        if !valid_licence_id(id) {
            return Err(Error::Invalid(format!("licence id {id:?} is not a registry id")));
        }
        self.extra_licences.insert(id.to_string());
        Ok(())
    }

    /// File `chunk` under `key`. The same key with the same chunk is a no-op; the same key with
    /// a different chunk is [`Error::DuplicateKey`] (so the result never depends on insertion
    /// order). Identical chunks under several keys are stored once.
    pub fn insert(&mut self, key: ChunkKey, chunk: Chunk<'_>) -> Result<ChunkHash> {
        key.validate()?;
        if key.kind() != chunk.kind() {
            return Err(Error::KeyKindMismatch { key, chunk: chunk.kind() });
        }
        if u32::try_from(chunk.bytes().len()).is_err() {
            return Err(Error::Invalid("chunk larger than 4 GiB".into()));
        }
        let h = chunk.hash();
        let kb = key.to_bytes();
        if let Some((_, existing)) = self.keys.get(&kb) {
            if *existing != h {
                return Err(Error::DuplicateKey { key, existing: *existing, new: h });
            }
            return Ok(h);
        }
        self.keys.insert(kb, (key, h));
        self.chunks.entry(h).or_insert_with(|| chunk.into_owned());
        Ok(h)
    }

    pub fn key_count(&self) -> usize {
        self.keys.len()
    }
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Write the pack. Deterministic: depends only on the set of `(key, chunk)` pairs, the
    /// versions and the declared licences.
    pub fn write_to<W: Write>(&self, w: W) -> Result<PackSummary> {
        // Layout: chunks in order of the smallest key that references them.
        let mut placed: BTreeMap<ChunkHash, (u64, u32)> = BTreeMap::new();
        let mut order: Vec<&Chunk<'static>> = Vec::with_capacity(self.chunks.len());
        let mut pos = PACK_HEADER_LEN as u64;
        for (_, h) in self.keys.values() {
            if placed.contains_key(h) {
                continue;
            }
            let c = &self.chunks[h];
            let len = c.bytes().len() as u64;
            placed.insert(*h, (pos, len as u32));
            order.push(c);
            pos += len;
            pos += pad_len(pos as usize % ALIGN) as u64;
        }
        let manifest_offset = pos;

        let mut licences: BTreeSet<&str> = self.extra_licences.iter().map(String::as_str).collect();
        for c in self.chunks.values() {
            licences.extend(c.licences().iter().map(String::as_str));
        }

        let mut manifest = Vec::with_capacity(MANIFEST_HEADER_LEN + ENTRY_LEN * self.keys.len());
        let mut m = Out::new(&mut manifest);
        m.bytes(&MANIFEST_MAGIC);
        m.count(self.keys.len(), "keys")?;
        m.u32(self.data_version);
        m.u32(self.gen_version);
        m.count(licences.len(), "licences")?;
        m.u32(0);
        m.u64(0);
        for (kb, (_, h)) in &self.keys {
            let (off, len) = placed[h];
            m.bytes(kb);
            m.bytes(&h.0);
            m.u64(off);
            m.u32(len);
            m.u32(0);
        }
        for l in &licences {
            m.str16(l)?;
        }
        let manifest_hash = ChunkHash::of(&manifest);

        let mut header = Vec::with_capacity(PACK_HEADER_LEN);
        let mut o = Out::new(&mut header);
        o.bytes(&PACK_MAGIC);
        o.u32(PACK_VERSION);
        o.u64(manifest_offset);
        o.u64(manifest.len() as u64);
        o.bytes(&manifest_hash.0);
        o.u64(0);

        let mut hw = HashingWriter { inner: w, hasher: blake3::Hasher::new(), bytes: 0 };
        hw.write_all(&header)?;
        let zeros = [0u8; ALIGN];
        for c in order {
            hw.write_all(c.bytes())?;
            let pad = pad_len(hw.bytes as usize % ALIGN);
            hw.write_all(&zeros[..pad])?;
        }
        debug_assert_eq!(hw.bytes, manifest_offset);
        hw.write_all(&manifest)?;
        hw.flush()?;
        Ok(PackSummary {
            file_hash: ChunkHash(*hw.hasher.finalize().as_bytes()),
            bytes: hw.bytes,
            keys: self.keys.len(),
            chunks: self.chunks.len(),
        })
    }

    pub fn to_bytes(&self) -> Result<(Vec<u8>, PackSummary)> {
        let mut v = Vec::new();
        let s = self.write_to(&mut v)?;
        Ok((v, s))
    }

    /// Write to `path` (created or truncated).
    pub fn write_file(&self, path: &Path) -> Result<PackSummary> {
        let f = File::create(path)?;
        let s = self.write_to(BufWriter::with_capacity(1 << 20, &f))?;
        f.sync_all()?;
        Ok(s)
    }
}

struct HashingWriter<W> {
    inner: W,
    hasher: blake3::Hasher,
    bytes: u64,
}
impl<W: Write> Write for HashingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.hasher.update(&buf[..n]);
        self.bytes += n as u64;
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// Positional reads from pack storage. Implemented for files (`pread` / `seek_read`, no shared
/// cursor, so one reader serves many threads), memory maps and in-memory buffers.
pub trait ReadAt: Send + Sync {
    fn size(&self) -> io::Result<u64>;
    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()>;
    /// The whole storage as one slice, when it is mapped or in memory (enables zero-copy).
    fn as_slice(&self) -> Option<&[u8]> {
        None
    }
}

pub(crate) fn slice_read_at(s: &[u8], buf: &mut [u8], offset: u64) -> io::Result<()> {
    let start = usize::try_from(offset).map_err(|_| io::ErrorKind::UnexpectedEof)?;
    let src = start
        .checked_add(buf.len())
        .and_then(|end| s.get(start..end))
        .ok_or(io::ErrorKind::UnexpectedEof)?;
    buf.copy_from_slice(src);
    Ok(())
}

impl ReadAt for File {
    fn size(&self) -> io::Result<u64> {
        Ok(self.metadata()?.len())
    }
    #[cfg(unix)]
    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()> {
        std::os::unix::fs::FileExt::read_exact_at(self, buf, offset)
    }
    #[cfg(windows)]
    fn read_exact_at(&self, mut buf: &mut [u8], mut offset: u64) -> io::Result<()> {
        use std::os::windows::fs::FileExt;
        while !buf.is_empty() {
            match self.seek_read(buf, offset) {
                Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
                Ok(n) => {
                    buf = &mut buf[n..];
                    offset += n as u64;
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}
impl ReadAt for Vec<u8> {
    fn size(&self) -> io::Result<u64> {
        Ok(self.len() as u64)
    }
    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()> {
        slice_read_at(self, buf, offset)
    }
    fn as_slice(&self) -> Option<&[u8]> {
        Some(self)
    }
}
impl ReadAt for memmap2::Mmap {
    fn size(&self) -> io::Result<u64> {
        Ok(self.len() as u64)
    }
    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()> {
        slice_read_at(self, buf, offset)
    }
    fn as_slice(&self) -> Option<&[u8]> {
        Some(self)
    }
}
impl<T: ReadAt + ?Sized> ReadAt for Arc<T> {
    fn size(&self) -> io::Result<u64> {
        (**self).size()
    }
    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()> {
        (**self).read_exact_at(buf, offset)
    }
    fn as_slice(&self) -> Option<&[u8]> {
        (**self).as_slice()
    }
}
impl<T: ReadAt + ?Sized> ReadAt for Box<T> {
    fn size(&self) -> io::Result<u64> {
        (**self).size()
    }
    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()> {
        (**self).read_exact_at(buf, offset)
    }
    fn as_slice(&self) -> Option<&[u8]> {
        (**self).as_slice()
    }
}

/// One manifest row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ManifestEntry {
    pub key: ChunkKey,
    pub hash: ChunkHash,
    /// Byte offset of the chunk in the file (16-aligned).
    pub offset: u64,
    pub len: u32,
    key_bytes: [u8; KEY_LEN],
}

/// An opened pack. Opening reads the 64-byte header and the manifest only; chunks are read on
/// demand with positional reads (or sliced from a map) and verified against their hash.
pub struct PackReader<S> {
    storage: S,
    entries: Vec<ManifestEntry>,
    /// Distinct chunks, sorted by hash: `(hash, offset, len)`.
    by_hash: Vec<(ChunkHash, u64, u32)>,
    meta: PackMeta,
    manifest_offset: u64,
}

impl<S: ReadAt> PackReader<S> {
    pub fn open(storage: S) -> Result<PackReader<S>> {
        let size = storage.size()?;
        if size < PACK_HEADER_LEN as u64 {
            return Err(Error::Truncated { offset: 0, needed: PACK_HEADER_LEN, available: size as usize });
        }
        let mut hb = [0u8; PACK_HEADER_LEN];
        storage.read_exact_at(&mut hb, 0)?;
        let mut r = In::new(&hb);
        if r.arr::<4>()? != PACK_MAGIC {
            return Err(Error::BadMagic { expected: PACK_MAGIC });
        }
        let version = r.u32()?;
        if version != PACK_VERSION {
            return Err(Error::UnsupportedPackVersion(version));
        }
        let mo = r.u64()?;
        let ml = r.u64()?;
        let mh = ChunkHash(r.arr()?);
        r.zeros(8, "pack header reserved")?;
        if mo < PACK_HEADER_LEN as u64 || mo % ALIGN as u64 != 0 || mo.checked_add(ml) != Some(size) {
            return Err(Error::Corrupt(format!(
                "manifest at {mo} (+{ml}) does not end a {size}-byte file at an aligned offset"
            )));
        }
        let ml = usize::try_from(ml).map_err(|_| Error::Corrupt("manifest too large".into()))?;
        let mut mb = vec![0u8; ml];
        storage.read_exact_at(&mut mb, mo)?;
        let actual = ChunkHash::of(&mb);
        if actual != mh {
            return Err(Error::HashMismatch { expected: mh, actual });
        }

        let mut r = In::new(&mb);
        if r.arr::<4>()? != MANIFEST_MAGIC {
            return Err(Error::BadMagic { expected: MANIFEST_MAGIC });
        }
        let n = r.u32()? as usize;
        let data_version = r.u32()?;
        let gen_version = r.u32()?;
        let nl = r.u32()? as usize;
        r.zeros(12, "manifest reserved")?;
        if n.saturating_mul(ENTRY_LEN) > r.remaining() {
            return Err(Error::Corrupt(format!("manifest claims {n} entries")));
        }
        let mut entries: Vec<ManifestEntry> = Vec::with_capacity(n);
        for _ in 0..n {
            let key_bytes: [u8; KEY_LEN] = r.arr()?;
            let key = ChunkKey::from_bytes(&key_bytes)?;
            let hash = ChunkHash(r.arr()?);
            let offset = r.u64()?;
            let len = r.u32()?;
            r.zeros(4, "entry reserved")?;
            if entries.last().is_some_and(|p| p.key_bytes >= key_bytes) {
                return Err(Error::Corrupt("manifest keys not strictly ascending".into()));
            }
            if offset < PACK_HEADER_LEN as u64
                || offset % ALIGN as u64 != 0
                || (len as usize) < CHUNK_HEADER_LEN
                || offset + len as u64 > mo
            {
                return Err(Error::Corrupt(format!("entry {key:?}: bad chunk range {offset}+{len}")));
            }
            entries.push(ManifestEntry { key, hash, offset, len, key_bytes });
        }
        let mut licences: Vec<String> = Vec::with_capacity(nl.min(r.remaining() / 2));
        for _ in 0..nl {
            let s = r.str16()?;
            if !valid_licence_id(s) || licences.last().is_some_and(|p| p.as_str() >= s) {
                return Err(Error::Corrupt(format!("bad or unsorted licence id {s:?}")));
            }
            licences.push(s.to_string());
        }
        r.finish()?;

        let mut by_hash: Vec<(ChunkHash, u64, u32)> = entries.iter().map(|e| (e.hash, e.offset, e.len)).collect();
        by_hash.sort_unstable();
        by_hash.dedup();
        if by_hash.windows(2).any(|w| w[0].0 == w[1].0) {
            return Err(Error::Corrupt("one hash stored at two locations".into()));
        }
        Ok(PackReader {
            storage,
            entries,
            by_hash,
            meta: PackMeta { data_version, gen_version, licences },
            manifest_offset: mo,
        })
    }

    pub fn meta(&self) -> &PackMeta {
        &self.meta
    }
    pub fn storage(&self) -> &S {
        &self.storage
    }
    /// All manifest rows, sorted by key.
    pub fn entries(&self) -> &[ManifestEntry] {
        &self.entries
    }
    pub fn key_count(&self) -> usize {
        self.entries.len()
    }
    /// Distinct chunks stored.
    pub fn chunk_count(&self) -> usize {
        self.by_hash.len()
    }
    pub fn manifest_offset(&self) -> u64 {
        self.manifest_offset
    }

    pub fn entry(&self, key: &ChunkKey) -> Option<&ManifestEntry> {
        let kb = key.to_bytes();
        self.entries.binary_search_by(|e| e.key_bytes.cmp(&kb)).ok().map(|i| &self.entries[i])
    }
    pub fn resolve(&self, key: &ChunkKey) -> Option<ChunkHash> {
        self.entry(key).map(|e| e.hash)
    }
    pub fn contains_hash(&self, h: &ChunkHash) -> bool {
        self.locate(h).is_some()
    }

    /// Every chunk filed under `node` (all kinds, rungs and layers), sorted by key.
    pub fn node_entries(&self, node: NodeId) -> &[ManifestEntry] {
        let p = node_prefix(node);
        let lo = self.entries.partition_point(|e| e.key_bytes[..9] < p[..]);
        let hi = self.entries.partition_point(|e| e.key_bytes[..9] <= p[..]);
        &self.entries[lo..hi]
    }
    /// The children of `node` that have at least one chunk in this pack, in child order.
    pub fn children_present(&self, node: NodeId) -> Vec<NodeId> {
        node.children()
            .map(|c| c.into_iter().filter(|c| !self.node_entries(*c).is_empty()).collect())
            .unwrap_or_default()
    }

    fn locate(&self, h: &ChunkHash) -> Option<(u64, u32)> {
        self.by_hash.binary_search_by(|e| e.0.cmp(h)).ok().map(|i| (self.by_hash[i].1, self.by_hash[i].2))
    }

    /// Read and verify one chunk by hash (an owned copy).
    pub fn read_hash(&self, h: &ChunkHash) -> Result<Chunk<'static>> {
        let (off, len) = self.locate(h).ok_or(Error::ChunkNotFound(*h))?;
        let mut buf = vec![0u8; len as usize];
        self.storage.read_exact_at(&mut buf, off)?;
        Chunk::parse_verified(buf, h)
    }
    /// Read and verify the chunk filed under `key`.
    pub fn read(&self, key: &ChunkKey) -> Result<Chunk<'static>> {
        let h = self.resolve(key).ok_or(Error::KeyNotFound(*key))?;
        self.read_hash(&h)
    }
    /// Like [`PackReader::read`], but borrows from the storage when it is mapped or in memory,
    /// so payload bytes can be handed on (to the GPU) without a copy. Still verified.
    pub fn read_borrowed(&self, key: &ChunkKey) -> Result<Chunk<'_>> {
        let e = self.entry(key).ok_or(Error::KeyNotFound(*key))?;
        match self.storage.as_slice() {
            Some(s) => {
                let bytes = s
                    .get(e.offset as usize..e.offset as usize + e.len as usize)
                    .ok_or_else(|| Error::Corrupt("chunk outside the file".into()))?;
                Chunk::parse_verified(bytes, &e.hash)
            }
            None => self.read_hash(&e.hash),
        }
    }

    /// Read every chunk, check every hash, and check that the pack's licence manifest covers
    /// every chunk's licence ids.
    pub fn verify_all(&self) -> Result<()> {
        for (h, _, _) in &self.by_hash {
            let c = self.read_hash(h)?;
            if let Some(l) = c.licences().iter().find(|l| self.meta.licences.binary_search(l).is_err()) {
                return Err(Error::Corrupt(format!("chunk {h} names licence {l:?} missing from the manifest")));
            }
        }
        for e in &self.entries {
            let mut hb = [0u8; 6];
            self.storage.read_exact_at(&mut hb, e.offset)?;
            let kind = u16::from_le_bytes([hb[4], hb[5]]);
            if kind != e.key.kind().id() {
                return Err(Error::Corrupt(format!("{:?} points at a chunk of kind {kind}", e.key)));
            }
        }
        Ok(())
    }
}

impl PackReader<File> {
    /// Open with positional reads (no mapping).
    pub fn open_file(path: &Path) -> Result<PackReader<File>> {
        PackReader::open(File::open(path)?)
    }
}
