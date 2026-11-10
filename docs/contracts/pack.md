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
