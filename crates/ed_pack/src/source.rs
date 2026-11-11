//! [`ChunkSource`]: where chunks come from (pack file, directory store, tile service, cache).
//!
//! Synchronous by design: worker pools (`ed_runtime::jobs`) call [`ChunkSource::get`] and
//! block; an async front end can wrap any source. Resolution (key to hash) and fetch (hash to
//! bytes) are separate, as in `pack.md` section 1.

use std::fs::File;
use std::io;
use std::path::Path;

use crate::chunk::Chunk;
use crate::error::{Error, Result};
use crate::hash::ChunkHash;
use crate::key::ChunkKey;
use crate::pack::{PackReader, ReadAt, slice_read_at};

pub trait ChunkSource: Send + Sync {
    /// The hash filed under `key`, or [`Error::KeyNotFound`].
    fn resolve(&self, key: &ChunkKey) -> Result<ChunkHash>;
    /// The verified chunk with hash `hash`, or [`Error::ChunkNotFound`].
    fn fetch(&self, hash: &ChunkHash) -> Result<Chunk<'static>>;
    /// `resolve` then `fetch`.
    fn get(&self, key: &ChunkKey) -> Result<Chunk<'static>> {
        let h = self.resolve(key)?;
        self.fetch(&h)
    }
}

impl<S: ReadAt> ChunkSource for PackReader<S> {
    fn resolve(&self, key: &ChunkKey) -> Result<ChunkHash> {
        PackReader::resolve(self, key).ok_or(Error::KeyNotFound(*key))
    }
    fn fetch(&self, hash: &ChunkHash) -> Result<Chunk<'static>> {
        self.read_hash(hash)
    }
}

/// Backing store of a [`PackFileSource`].
pub enum PackStorage {
    Mapped(memmap2::Mmap),
    File(File),
}

impl ReadAt for PackStorage {
    fn size(&self) -> io::Result<u64> {
        match self {
            PackStorage::Mapped(m) => Ok(m.len() as u64),
            PackStorage::File(f) => f.size(),
        }
    }
    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()> {
        match self {
            PackStorage::Mapped(m) => slice_read_at(m, buf, offset),
            PackStorage::File(f) => f.read_exact_at(buf, offset),
        }
    }
    fn as_slice(&self) -> Option<&[u8]> {
        match self {
            PackStorage::Mapped(m) => Some(m),
            PackStorage::File(_) => None,
        }
    }
}

/// A `.ewpack` file opened as a chunk source.
pub struct PackFileSource {
    reader: PackReader<PackStorage>,
}

impl PackFileSource {
    /// Memory-map the file. Packs are immutable (`pack.md` section 3): replacing a pack means
    /// writing a new file, never rewriting one that is open.
    pub fn open(path: &Path) -> Result<PackFileSource> {
        let f = File::open(path)?;
        // SAFETY: the map is read-only and packs are never modified in place while mounted
        // (writers create new files). Every chunk read from it is hash-verified.
        let map = unsafe { memmap2::Mmap::map(&f)? };
        Ok(PackFileSource { reader: PackReader::open(PackStorage::Mapped(map))? })
    }
    /// Open with positional reads instead of a map.
    pub fn open_unmapped(path: &Path) -> Result<PackFileSource> {
        Ok(PackFileSource { reader: PackReader::open(PackStorage::File(File::open(path)?))? })
    }
    pub fn reader(&self) -> &PackReader<PackStorage> {
        &self.reader
    }
}

impl ChunkSource for PackFileSource {
    fn resolve(&self, key: &ChunkKey) -> Result<ChunkHash> {
        ChunkSource::resolve(&self.reader, key)
    }
    fn fetch(&self, hash: &ChunkHash) -> Result<Chunk<'static>> {
        self.reader.read_hash(hash)
    }
}

/// Sources in mount order. A later mount overrides an earlier one's keys (patch packs);
/// `fetch` asks the newest mount first and falls back to older ones.
#[derive(Default)]
pub struct Mounts {
    sources: Vec<Box<dyn ChunkSource>>,
}

impl Mounts {
    pub fn new() -> Mounts {
        Mounts::default()
    }
    pub fn mount(&mut self, source: Box<dyn ChunkSource>) {
        self.sources.push(source);
    }
    pub fn len(&self) -> usize {
        self.sources.len()
    }
    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }
}

impl ChunkSource for Mounts {
    fn resolve(&self, key: &ChunkKey) -> Result<ChunkHash> {
        for s in self.sources.iter().rev() {
            match s.resolve(key) {
                Err(e) if e.is_not_found() => continue,
                r => return r,
            }
        }
        Err(Error::KeyNotFound(*key))
    }
    fn fetch(&self, hash: &ChunkHash) -> Result<Chunk<'static>> {
        for s in self.sources.iter().rev() {
            match s.fetch(hash) {
                Err(e) if e.is_not_found() => continue,
                r => return r,
            }
        }
        Err(Error::ChunkNotFound(*hash))
    }
}
