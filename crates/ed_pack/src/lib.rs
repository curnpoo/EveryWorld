//! The `.ewpack` container: chunk kinds, content hashes, manifests, reader and writer, and the
//! override-layer text format.
//!
//! Allowed workspace dependencies: `ed_geo`. The list is enforced by
//! `cargo xtask check-deps` (rules in `xtask/deps.toml`).
//!
//! Contracts: `docs/contracts/pack.md` (container, chunk kinds; the "Decisions made during
//! implementation" section fixes what the contract left open) and `docs/contracts/layers.md`
//! (override layers, module [`layers`]).
//!
//! - [`Chunk`]: one immutable, self-verifying blob (64-byte header, provenance/licence block,
//!   payload). Typed bodies implement [`ChunkBody`] (module [`kinds`]).
//! - [`PackWriter`] / [`PackReader`]: the `.ewpack` file. Output is byte-identical for the same
//!   set of `(key, chunk)` pairs regardless of insertion order, thread count or platform.
//! - [`ChunkSource`]: blocking `resolve` / `fetch` / `get`, implemented by [`PackFileSource`]
//!   and [`PackReader`]; [`Mounts`] stacks patch packs in mount order.

pub mod chunk;
pub mod error;
pub mod hash;
pub mod key;
pub mod kinds;
pub mod layers;
pub mod pack;
pub mod source;
mod wire;

pub use chunk::{Chunk, ChunkBody, ChunkHeader, ChunkMeta, Codec, CHUNK_HEADER_LEN, CHUNK_MAGIC};
pub use error::{Error, Result};
pub use hash::{AssetId, ChunkHash};
pub use key::{ChunkKey, ChunkKind, Rung, KEY_LEN};
pub use pack::{ManifestEntry, PackMeta, PackReader, PackSummary, PackWriter, ReadAt};
pub use source::{ChunkSource, Mounts, PackFileSource, PackStorage};
pub use wire::ALIGN;
