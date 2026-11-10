# World — addressing, the representation ladder, streaming and authored layers

Design brief for the part of the engine that the previous plan left undesigned and that the
alpha got wrong: it loaded slowly, had gaps, and could not show a city without generating all
of it. Decisions: F1, F2, F4, F5 in `../DECISIONS.md`. Built in Phases 1 to 3
(`../ROADMAP.md`).

## Goals

1. Stand anywhere from a street to the Moon and see a complete world with no holes.
2. See a city's skyline from tens of kilometres, its footprint and lights from orbit.
3. Arrive somewhere new and have it appear quickly, coarse first, without stalls.
4. Place a specific building, marker or mission and have it stay there across regeneration,
   saves and multiplayer.

## 1. Addressing (F1)

### 1.1 NodeId

```
NodeId (64 bits) = body:8 | face:3 | level:5 | x:24 | y:24
```

- `body`: index into the body catalog (Earth, Moon, Mars, ...).
- `face`: one of six cube faces. `level`: quadtree depth, 0 = whole face.
- `x`, `y`: cell on that face at that level.

| Level | Node edge on Earth | Roughly slippy zoom | Typical content |
|---|---|---|---|
| L0–L3 | 10,000–1,250 km | z2–z5 | Whole-planet view |
| L4–L6 | 625–156 km | z6–z8 | Countries, coastlines, night lights |
| L7–L9 | 78–20 km | z9–z11 | Regions, city footprints |
| L10–L12 | 10–2.4 km | z12–z14 | Districts, skylines, road network |
| L13–L15 | 1.2 km–300 m | z15–z17 | Streets, buildings, parts |

The cube-to-sphere mapping is the tangent warp (P0-R1, `../contracts/addressing.md`) so nodes at
one level have similar size everywhere, including the poles.

### 1.2 What keys on NodeId

| System | Use |
|---|---|
| `ed_planet` | LOD selection, residency, height queries |
| `ed_pack` | Chunk key: `(NodeId, chunk kind, rung)` → content hash |
| `ed_tiles` | Request and cache key |
| `ed_world` | Unit of generation |
| `ed_store` | Deltas, parked records and plots are indexed by the L12 node containing them |
| `ed_net` | Interest sets and shard ownership are sets of nodes |
| `ed_editor` | Override layers are files grouped by L10 node |

### 1.3 Where slippy tiles remain

Source datasets arrive as slippy tiles, GeoParquet or rasters in their own projections.
`ed_data` reads them and resamples into node space. No type from `ed_data` that names a slippy
tile crosses into `ed_world` or above. This removes the alpha's class of gap bugs where the
render tile and the data tile disagreed about an edge.

### 1.4 Edges

A generator for node N may read source features within a margin outside N, and must produce
output that is identical along the shared edge to what the neighbour produces. Roads,
buildings and parcels that cross an edge are owned by exactly one node (the one containing
the feature's anchor point); the other node does not emit them. Terrain skirts and
geomorphing hide level differences between neighbours.

## 2. The representation ladder (F2)

| Rung | Content | Chunk kinds | Built from |
|---|---|---|---|
| R0 parts | Instances of kit parts, props, trees; collision; nav graph | `instances`, `collision`, `nav` | Street plan, footprints, rules |
| R1 shells | One low-poly mesh per building with a baked or procedural facade material; tree clusters | `shells` | Footprints + height + style row |
| R2 proxy | One merged simplified mesh per node, one small atlas | `proxy` | Footprints + heights, extruded and simplified; or simplification of R1 |
| R3 terrain-baked | No object meshes. Buildings and roads become a height bump, an albedo tint and an emissive mask in the node's terrain layers | `terrain`, `layers` | Footprint density rasters |
| R4 imagery | Albedo from imagery, night-light emissive | `layers` | Sentinel-2 composite, night lights |

Rules:

- **Selection** is by projected error in pixels. Each rung declares a geometric error per
  node level; the selector picks the coarsest rung under the pixel threshold. Distances in
  any doc are illustrations.
- **Independence.** R2, R3 and R4 are baked directly from footprints and rasters. Showing
  Tokyo from 80 km never runs the facade generator.
- **Consistency.** Rungs agree on silhouette and colour: R1's style row decides the colour
  that R2's atlas and R3's albedo tint use, so a city does not change colour as you approach.
- **Transitions** cross-fade (dither over a short screen-error band) and are tested by flying
  one fixed path from orbit to street.
- **Lights.** R3 and R4 emissive come from the same density data that drives R0 street
  lights, scaled so brightness is continuous. A city is visible on the night side from the
  Moon as a few bright pixels because the emissive survives in the coarsest mips.
- **Trees** follow the same ladder: individual trees → clusters → canopy mesh in the proxy →
  canopy colour and roughness in terrain layers.

What you can see from where (1440p, 60° vertical field of view):

| Viewpoint | Pixel footprint on the ground | Rung in view |
|---|---|---|
| Street | centimetres | R0 near, R1 down the street, R2 skyline |
| Aircraft, 10 km | about 7 m | R1/R2 below, R3 to the horizon (about 357 km away) |
| Low orbit, 400 km | about 320 m | R3 and R4; a country fills the screen |
| Moon, 384,000 km | about 300 km | R4; Earth is about 40 pixels across |

## 3. Streaming (F4)

### 3.1 Why the alpha loaded slowly

Every node was fetched as raw source tiles and generated on the player's machine at full
detail before anything was shown, and merged meshes were then uploaded in one piece. The fix
is to move work earlier (bake), show coarse rungs first, and make uploads small.

### 3.2 Three sources, one interface

```
trait ChunkSource { fn get(&self, key: ChunkKey) -> Future<Chunk>; }
```

| Source | Levels | Notes |
|---|---|---|
| **Globe pack** (shipped) | L0–L6, rungs R3/R4; plus R2 proxies for the largest cities | About 33,000 nodes. Size is measured in Phase 0 (W4d); the working assumption is 1–2 GB |
| **Tile service** (`ew-tiles`) | L7–L12, rungs R1–R4 | Bakes a chunk on first request from pinned source data, stores it by content hash, serves it over HTTP. Any static file host or CDN can sit in front of it |
| **Local generation** | L13–L15, rung R0 | The client runs `ed_world` on a regional source extract (roads, footprints, tags) that the tile service delivers as a chunk |

For development, all three run on one machine: `ew` embeds the same code the service runs,
so no network is needed. For offline play, a region can be pre-baked into a pack.

### 3.3 Chunk format

`.ewpack` is a container of chunks. Each chunk has a kind, a schema version, the generator
version, the pinned data version and a content hash. The manifest maps
`(NodeId, kind, rung)` → hash. Identical content is stored once.

Because generation is deterministic (invariant 3), the server can tell a client "node N,
rung R1, hash H" and the client may fetch it from any source, or generate it and verify.

### 3.4 Loading order and budgets

1. The selector produces the wanted set from camera position, velocity and the look-ahead
   along the direction of travel.
2. Requests are prioritised by screen-space benefit; coarse rungs first.
3. Decode and generation run on `ed_runtime` pools. GPU upload is staged under a per-frame
   byte budget, so a frame never stalls on an upload.
4. A node becomes visible only when complete; until then its parent draws (invariant 5).
5. Residency is bounded: fixed GPU and CPU memory budgets, least-recently-used eviction.

Measured targets (set from the Phase 0 baseline, held by the harness): time from spawn to a
complete coarse view; time to full detail at a standstill; worst frame while flying at
aircraft and at rocket speed.

## 4. Stable ids, override layers and deltas (F5)

### 4.1 ObjectId

```
ObjectId (128 bits) = hash(source dataset, source feature id, generator id, sub-index)
```

- For a building from Overture, the feature id is its GERS id, which is stable across
  Overture releases. A part of that building adds the generator id and a sub-index.
- Procedural fill with no source feature uses (parcel id, generator id, sub-index).
- Authored objects get a random id at creation, stored in the layer file.

An `ObjectId` does not depend on which node or rung the object is in, so it survives
re-baking, a data-version bump and LOD changes. Picking in the editor returns it.

### 4.2 Override layers

Text files (TOML), one directory per layer, one file per L10 node, committed to the repo.

| Operation | Meaning |
|---|---|
| `place` | Add a prefab at a body-fixed position and orientation |
| `replace` | Swap a generated object (by `ObjectId`) for a prefab, or change its parameters (style, height, colour) |
| `suppress` | Remove generated objects by id or inside a volume |
| `marker` | A named point or volume for gameplay (spawn, landing pad, point of interest) |
| `mission` | A mission definition bound to markers |
| `road_patch` | Edit to the street graph: add, remove or re-profile a segment |
| `terrain_patch` | Local height or surface edit with a falloff |

Layers have an order and can be toggled (for example `base_hubs`, `spaceports`,
`mission_pack_1`). Generation for a node takes the node's source data and the layers that
touch it, so placing a hub also clears the generated buildings under it and re-routes the
street generator around it.

Every rung honours layers: a placed landmark tower contributes to its node's R2 proxy and R3
emissive, so authored landmarks are visible from far away.

### 4.3 Live deltas

Runtime changes (a player-built house, a felled tree, a parked car) are rows in `ed_store`
with the same vocabulary as layer operations, keyed by node. The world at any moment is:

```
generate(seed, data version, node, rung)  ∘  override layers  ∘  deltas
```

Solo saves, server persistence and network replication all carry only the third term.

## 5. Implementation plan

| Phase | Work | Proof |
|---|---|---|
| 0 | Contracts: addressing, generation outputs per rung, pack format, layer format. Spikes: cube mapping choice and edge test; globe pack size measurement | `docs/contracts/*`; measured pack size |
| 1 | `NodeId`/`ObjectId` types in `ed_geo`; `ed_pack` reader/writer; layer file reader; all exercised by the proving-ground scene, which is described as nodes and a layer | Proving ground loads from a pack and a layer file |
| 2 | `ed_planet` selection, horizon culling, residency; R3/R4 bake for the globe pack; `ew-tiles` v1; staged upload budgets; Moon as a second body | Unbroken orbit → ground → orbit recording with no holes; city lights from orbit |
| 3 | `ed_world` refactor to emit R0–R2 per node; cross-fade transitions; layer operations applied in generation; deltas in `ed_store` | Fly-in to three reference cities with no pops; a placed landmark visible from 50 km |

## 6. Open questions

1. Globe pack contents beyond L6: how many cities get shipped R2 proxies.
2. Whether R1 shells use baked facade textures or a procedural shell material (measure
   memory against quality in Phase 3).
3. Regional source-extract format for local generation (compact binary of the street graph
   and footprints; schema in the Phase 0 pack contract).
