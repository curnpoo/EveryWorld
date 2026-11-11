# Contract: pack

Owner: `ed_pack`. Decisions: F3, F4, P1-D11, P1-D13. Evidence:
`../reference/spikes/w4d-addressing.md` (sizes), `../reference/spikes/w4b-geometry.md`
(cluster data).

## 1. Model

A **chunk** is one immutable blob of one kind for one key. A **pack** (`.ewpack`) is a
container of chunks plus a manifest. A **chunk store** is anything that returns a chunk for
a hash: a pack file, a directory, the tile service, an HTTP cache.

```rust
pub struct ChunkHash(pub [u8; 32]);                // BLAKE3 of the encoded chunk bytes
pub enum ChunkKey {
    Node  { node: NodeId, kind: ChunkKind, rung: Rung },
    Asset { asset: AssetId, kind: ChunkKind },      // AssetId = BLAKE3 of the source asset path
}
pub trait ChunkSource {
    fn resolve(&self, key: ChunkKey) -> impl Future<Output = Result<ChunkHash, SourceError>>;
    fn fetch(&self, hash: ChunkHash) -> impl Future<Output = Result<Chunk, SourceError>>;
}
```

Resolution (key to hash) and fetch (hash to bytes) are separate so that a server can send a
hash and the client can fetch it from anywhere, or generate it locally and compare.

## 2. Chunk header

Every chunk starts with a fixed 64-byte header, then the body.

| Field | Type | Meaning |
|---|---|---|
| magic | `[u8; 4]` | `EWCK` |
| kind | `u16` | Chunk kind (section 4) |
| schema | `u16` | Schema version of this kind |
| gen_version | `u32` | Generator or baker version; 0 for authored assets |
| data_version | `u32` | Data version; 0 if no source data was used |
| codec | `u8` | 0 raw, 1 zstd |
| flags | `u8` | Reserved, zero |
| reserved | `[u8; 6]` | Zero |
| raw_len | `u64` | Body length after decoding |
| key | `[u8; 16]` | `NodeId` plus rung, or the first 16 bytes of the `AssetId` |
| reserved2 | `[u8; 16]` | Zero |

- The hash covers the header and the encoded body. Two chunks with the same hash are the
  same chunk.
- A reader must reject an unknown `kind` or a `schema` it does not implement; it must never
  guess.
- Bodies are plain little-endian structures with explicit offsets, readable without a
  parsing library, and are designed to be used in place after decoding (no per-item
  allocation).

## 3. Pack file

```
header      magic "EWPK", version u32, manifest_offset u64, manifest_len u64, manifest_hash [u8;32]
chunks      concatenated, each aligned to 16 bytes
manifest    sorted table of (ChunkKey bytes, ChunkHash, offset u64, len u32)
            + pack metadata: data version, generator version, licence manifest (dataset ids)
```

- The manifest is sorted by key bytes so lookup is a binary search on a memory-mapped file.
- Identical chunks (ocean nodes, repeated assets) are stored once and referenced by several
  keys.
- The licence manifest lists every registry dataset id that contributed. `cargo xtask
  licence-gate` reads it; a pack that names a RED or development-only dataset cannot be
  built in a release configuration.
- Packs are immutable. An update is a new pack; a patch pack is a pack whose manifest
  overrides keys of an earlier one, in mount order.

## 4. Chunk kinds

| Id | Kind | Key | Body |
|---|---|---|---|
| 1 | `terrain` | node, R3 | Height grid 257 x 257, `u16` with per-chunk scale and offset; min and max; skirt depth |
| 2 | `layers` | node, R3 or R4 | Texture set: albedo 256 x 256, emissive 128 x 128, roughness and class 256 x 256. Supercompressed block format transcoded to BC7 or ASTC at load |
| 3 | `proxy` | node, R2 | One mesh (quantised positions, octahedral normals, atlas UVs), one atlas texture reference |
| 4 | `shells` | node, R1 | Mesh table, one entry per building: `ObjectId`, bounds, index range, facade material id, style row |
| 5 | `instances` | node, R0 | Instance table sorted by `ObjectId`: mesh or prefab `AssetId`, transform in the node frame (position f32 x 3, rotation as a quaternion, non-uniform scale), material variant, flags; interior hooks |
| 6 | `collision` | node, R0 | Height field for ground, convex hulls and boxes for parts, triangle meshes for decks and tunnels |
| 7 | `nav` | node, R0 | Lane graph: nodes, edges, lanes, speeds, connections to neighbour nodes by `ObjectId` |
| 8 | `ground` | node, R0 | Ground cover parameters and density maps |
| 9 | `source` | node, L12 | Source extract for local generation: road segments with tags, footprints, parcels, water, in node space |
| 16 | `mesh` | asset | Vertex and index data with a **discrete LOD chain** (required): per LOD an index range and an error in metres. Bounds. Material slots |
| 17 | `clusters` | asset | **Optional** cluster hierarchy for the same mesh: clusters of at most 128 triangles, per group a bounding sphere and error, per cluster the refined group. Present only where Track G adopts it |
| 18 | `texture` | asset | One texture with mips, in a supercompressed block format; virtual-texture page table if larger than 4096 |
| 19 | `material` | asset | Material model id (PBR or toon), parameters, texture references by `AssetId` |
| 20 | `skeleton` | asset | Joints, parents, inverse bind matrices, humanoid mapping with optional bones |
| 21 | `clip` | asset | Animation clip against a skeleton |
| 22 | `prefab` | asset | A tree of instances with a manifest: footprint polygon, anchor, door and connection points, rung contributions (shell mesh, proxy contribution, emissive) |
| 23 | `collision_asset` | asset | Collision shapes for a mesh or prefab |
| 32 | `layer` | L10 node + layer name | One override layer file, compiled (see `layers.md`) |

**The geometry slot (F3):** a mesh always has kind 16. Kind 17 is extra. A renderer that
ignores kind 17 must draw everything correctly. No other chunk may reference cluster data.

Texture format: stored supercompressed (target: a UASTC-class or BC7-with-rate-distortion
format under zstd) and transcoded at load. Measured consequence: the globe pack is about
1.2 GiB with this choice against about 3 GiB for raw fixed-rate blocks.

## 5. Versions and invalidation

- `schema` changes when a body layout changes. Readers support the current schema only;
  packs are rebuilt, not migrated.
- `gen_version` and `data_version` are inputs to the hash through the header, so a
  generator change produces new hashes and old cached chunks are simply never requested.
- A cache may evict anything at any time; every chunk can be fetched or generated again.

## 6. Tests

| # | Test |
|---|---|
| P1 | Write then read every chunk kind; bytes round trip; the header hash matches |
| P2 | A corrupted byte anywhere fails the hash check on read |
| P3 | Unknown kind and unknown schema are rejected with a typed error |
| P4 | Pack with 100,000 keys: lookup by binary search on a mapped file; duplicate bodies stored once |
| P5 | Patch pack overrides a key; mount order decides |
| P6 | Building the same inputs twice gives byte-identical packs |
| P7 | A mesh with and without a `clusters` chunk renders the same through provider 1 |
| P8 | Licence manifest: a pack naming a RED dataset fails the gate |
| P9 | `ChunkSource` conformance suite run against the pack file, the directory store and the tile service |

## Decisions made during implementation (`crates/ed_pack`, pack version 1)

**Hash and verification**
- Chunk hash: BLAKE3-256 over the whole encoded chunk (header, metadata block, payload,
  excluding the pack's padding between chunks). `PackReader::read*` always checks it.
- The manifest has its own BLAKE3 hash in the pack header; `open` checks it.
- The file hash (`PackSummary::file_hash`) is BLAKE3 of the whole file. It is used for golden
  tests and release identity.

**Chunk header `key` field is written as zero.** Section 2 says the hash covers the header
and that identical chunks are stored once. Both cannot hold if the key is in the header: an
ocean terrain body under 10,000 node keys would get 10,000 hashes. So the key lives only in
the manifest, and the header field is reserved and must be zero. A chunk's identity is then
its content alone. Readers reject a non-zero key, flags or reserved field.

**Provenance and licences are in the chunk.** After the 64-byte header comes a metadata
block:
- `meta_len u32` (a multiple of 16, including itself), then `licence_count u16` and
  `provenance_count u16`;
- then the licence ids and provenance strings, each as `u16` length plus UTF-8, sorted and
  unique;
- then zero padding.

`raw_len` covers the metadata and the payload. Because the metadata is hashed, the licence
of every chunk can be verified, and the pack's licence manifest is the union over its chunks
plus any the writer adds explicitly. Licence ids must match `[a-z0-9][a-z0-9._-]*` (registry
ids, at most 64 bytes). Provenance strings are free-form, such as `osm:way/123` or
`baker:terrain@7`.

**Codec.** Only `codec = 0` (raw) is implemented. `1` (zstd) is recognised and rejected with
`Error::UnsupportedCodec`. Raw bodies can be used in place from a memory map, with no
decode buffer. Supercompressed textures (section 4) will land as a new codec value, or as a
texture schema 2, when the transcoder exists. Nothing in the format blocks that.

**Alignment** is 16 bytes throughout:
- every chunk starts on a 16-byte file offset;
- the payload starts 16-aligned within its chunk;
- inside typed payloads, vertex and index streams, texture mips and instance tables are
  16-aligned.

That lets a mapped file be handed to the GPU as one buffer plus offsets, which is what
"zero-copy" means here. Page alignment (4 or 16 KiB) is not used, because it would inflate
small chunks. Uploads that need page alignment should map the whole file.

**Endianness.** Every integer is little-endian except manifest key bytes, which are
big-endian so that byte order equals logical order.

**Manifest key bytes** (40 bytes, fixed length):
- Node: `00`, NodeId (u64 BE), kind (u16 BE), rung (u8), then 8 zero bytes.
- Layer: `00`, NodeId (BE), kind 32 (BE), `FF`, then the layer id (u64 BE). The layer id is
  the first 8 bytes of BLAKE3 of the layer name. This is a third key variant beside Node and
  Asset, because section 4 keys a layer chunk by "L10 node + layer name".
- Asset: `01`, AssetId (32 bytes), kind (u16 BE), then 5 zero bytes.

Consequences:
- All of one node's chunks are a contiguous run sorted by (kind, rung, layer), so
  `node_entries` is two binary searches.
- `children_present` is four prefix searches.
- Node keys sort before asset keys.

Decoding validates strictly: a node key needs a node kind or kind 32, a rung of R0..R4 (or
`FF` for a layer), and zero padding. An asset key needs an asset kind.

**Pack layout.**
- A 64-byte header: `EWPK`, version 1, `manifest_offset`, `manifest_len`, `manifest_hash`,
  and 8 reserved bytes.
- Then the chunks, each distinct chunk once, in order of the smallest key that references
  it.
- Then the manifest at the end. It has a 32-byte header (`EWMF`, `entry_count`,
  `data_version`, `gen_version`, `licence_count`, reserved), then 88-byte entries (key[40],
  hash[32], offset u64, len u32, reserved u32) in strictly ascending key order, then the
  licence ids.

The manifest is written last, so the writer streams chunks without seeking back. The reader
reads the header and the manifest (one read each) and nothing else until a chunk is asked
for. The 200 MB test shows that open plus one read touches only the header, the manifest and
that chunk.

**Determinism.**
- The writer keeps `BTreeMap`s keyed by key bytes and hash, so its output depends only on the
  set of (key, chunk) pairs, the versions and the explicit licences.
- Inserting the same key twice with the same chunk is a no-op; with a different chunk it is
  `Error::DuplicateKey`. Last-insert-wins would make the output depend on insertion order.
- There are no timestamps, no paths and no platform-dependent sizes.
- The golden test pins the file hash of a fixed small pack.

**Manifest in memory.** `open` copies the manifest into a sorted `Vec`, which costs about 88 B
per key: 100,000 keys is about 9 MB. Lookups binary-search that vector. Measured at about
130 ns per key on 100,000 keys in a debug build. Binary-searching the mapped bytes directly
would avoid the copy; if multi-million-key packs need it, it can be done without a format
change.

**Asset references are `AssetId`, not content hashes.** This applies to materials referencing
textures, prefabs referencing meshes and materials, and instance tables, and follows
section 4 ("texture references by `AssetId`"). A content hash would cascade: re-baking one
texture would re-hash every material, prefab and node that uses it. Content addressing
applies at the chunk level through the manifest, and the patch-pack mount order decides which
version of an asset is current.

**Clusters.** The optional cluster DAG is a separate kind-17 chunk keyed by the same
`AssetId`. Its body starts with the hash of the kind-16 mesh it was built from, followed by
opaque data. The mesh never references it (F3). A stale cluster chunk can be detected by
comparing the hashes.

**Typed kinds in schema 1:** instances (5), collision (6), mesh (16), clusters (17),
texture (18), material (19), prefab (22), collision_asset (23) and layer (32).
- Kinds 1-4 and 7-9, 20 and 21 are reserved: schema 0, with an opaque body.
- A reader rejects any other schema (`UnsupportedSchema`), a kind it does not know
  (`UnknownKind`), and decoding a chunk as the wrong type (`WrongKind`). It never panics,
  which is fuzz-tested.

Layouts per kind (all little-endian, 16-aligned streams):
- **Mesh**:
  - Fields: attributes (semantic, format, offset), stride, vertex count, index format
    (u16 or u32), bounding sphere plus AABB, material slot names.
  - LOD chain: required and non-empty, ordered fine to coarse, error in metres
    non-decreasing. Each LOD has an index range and per-material sections inside it.
  - Every index is checked against the vertex count on encode and decode.
- **Texture**:
  - Formats: `Rgba8Unorm`, `Rgba16Float`, `Bc7`, `Bc4`, `Bc5`, `R8Unorm`, `Rg8Unorm`.
  - Colour role: srgb, linear, normal or hdr. Srgb is allowed only with Rgba8 and Bc7, and
    hdr only with Rgba16F.
  - Size: width, height and layers (at most 65536 each), plus a stable `page_id` (u64) for
    the virtual-texture page table.
  - Mips: a table of (offset, length), with each length checked against the format and
    size.
- **Material**: model (PBR or toon), alpha mode and cutoff, double-sided, named parameters
  (float or vec2-4) and named texture slots (`AssetId`), sorted by name.
- **Collision**:
  - Shape kinds: box, sphere, capsule, convex hull, triangle mesh and height field, each
    with a surface id.
  - The same body is used for node collision (6) and collision_asset (23).
- **Prefab**:
  - Nodes: parent index, name, transform, optional mesh and material. A parent's index
    must be lower than its child's.
  - Footprint polygon (2D, in metres); named points (door, connection, anchor, spawn,
    light).
  - Rung contributions as (rung, mesh `AssetId`).
- **Instances**:
  - A sorted table of the assets used, then 80-byte records: ObjectId u128, transform
    (translation, quaternion, scale as f32), asset index, material index or none, variant
    u16, flags u32.
  - Records must be sorted by ObjectId with no duplicates.
- **Layer**: the canonical TOML text (`layers.md`) as UTF-8.

**`ChunkSource` is synchronous:**
```rust
trait ChunkSource: Send + Sync {
    fn resolve(&self, &ChunkKey) -> Result<ChunkHash>;
    fn fetch(&self, &ChunkHash) -> Result<Chunk<'static>>;
    fn get(&self, &ChunkKey) -> Result<Chunk<'static>>;
}
```
- This departs from the async sketch in section 1. Streaming runs on blocking worker pools
  (`ed_runtime::jobs`), so a synchronous trait avoids tying `ed_pack` to an async runtime; a
  network source can wrap it later.
- `PackFileSource` memory-maps the file, or uses positional reads with `open_unmapped`.
- `Mounts` composes sources: the newest mount wins for `resolve` (patch packs), and
  "not found" falls through to older mounts.

**Limits.** A chunk is at most 4 GiB (the manifest's `len` is u32), and a pack holds at most
2^32 - 1 keys.
