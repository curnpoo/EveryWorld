# Architecture (target)

The target design of the custom engine on `main`. Locked choices are in `DECISIONS.md` and are
referred to here by id (D1, F2, P1-D7). Per-system detail is in the briefs under `plans/`.
The frozen Bevy alpha (`alpha-early`) is described in `plans/rearchitecture.md`.

## 1. The design in one paragraph

Real data decides what is there; authored assets and rules decide how it looks. The planet is
a cube-sphere quadtree whose nodes share **one address** (F1). Each node can be shown at any
rung of a **ladder of representations**, from individual building parts down to night lights
seen from the Moon (F2). Node content arrives as **content-addressed chunks** from a shipped
globe pack, our tile service, or local generation (F4). Authored content is **override
layers** on top of deterministic generation (F5), edited in the engine's own **editor mode**
(F6). A GPU-driven renderer draws instances through a **geometry-provider slot** (F3). A
fixed-step simulation and region servers make it one persistent shared world.

## 2. Stack

| Layer | Choice | Decision |
|---|---|---|
| Language | Rust 2024 for client, server and tools | D1 |
| App | `ew`: winit window, input, main loop, game and editor modes | D1, F6 |
| GPU | `ed_gpu`: Metal first; D3D12 and Vulkan later, one of them minimal in Phase 1 | D1, F7 |
| Shaders | Slang, with a backend-native escape hatch | P1-D1 |
| Renderer | `ed_render`: frame graph, GPU instance culling, geometry providers, visibility buffer, lighting tiers | F3, F9, D14 |
| Geometry tools | meshoptimizer (simplification, meshlets, cluster LOD) | P1-D11 |
| Planet | `ed_planet`: cube-sphere quadtree per body | D2, F1 |
| Packs | `ed_pack`: `.ewpack` content-addressed chunk container | F4, P1-D11 |
| Physics | Box3D behind `ed_sim` | D4, F10 |
| Developer UI and editor | egui drawn through `ed_gpu` | P1-D14, F6 |
| Network | QUIC (`quinn`) in `ed_net` | F11 |
| Persistence | `ed_store`: SQLite, then Postgres; same schema for solo saves | D6 |
| Source data | `ed_data` + `data/registry/`; DuckDB/GeoParquet in bake tools | F8, F13 |
| Asset authoring | Blender → glTF → `ew-bake` | F6 |

Every component above exists today as a released library or a documented API. The two young
ones are Box3D (v0.1.0, mid-2026) and Solstice (reference only, F12); both are tested in
Phase 0 before anything depends on them.

## 3. Crates

```
KEPT FROM THE ALPHA (engine-free, unit-tested)
  ed_geo           f64 WGS84/ECEF/ENU maths, sun; gains cube-sphere and NodeId maths
  ed_data          source-data readers: elevation, land cover, climate, Overture, OSM tags.
                   Slippy TileId lives only here
  ed_world         world RULES: regions, biomes, archkit, streets, buildings, foliage placement.
                   Output changes from merged meshes to instances, shells, proxies and layers
  ed_flight        atmosphere, drag, Mach, rocket stages
  ed_space         ephemeris, body catalog, body frames
  ed_engine_audio, ed_audio
  ed_runtime       job pools, budgets, residency (LRU), scheduling

NEW
  ed_pack          .ewpack format: chunk kinds, content hashes, manifests, reader/writer
  ed_assets        ew-bake: glTF → meshes (LOD chain, optional clusters), textures, collision,
                   prefabs; world bake: node chunks for any ladder rung
  ed_planet        quadtree selection, horizon culling, node residency, height truth,
                   virtual texture pages, chunk requests
  ed_gpu           RHI: device, resources with states, bindless tables, frame graph support,
                   timers, capability probe. Backends are private modules
  ed_render        instance pools, GPU culling, geometry providers, visibility/shading,
                   shadows, GI, atmosphere, clouds, water, post, upscalers, picking
  ed_sim           fixed-step sim: Box3D, character mover, vehicles, water query. No render types
  ed_anim          skeletons, retargeting, clips, cloth
  ed_editor        editor mode: tools, gizmos, overlays, override-layer editing, undo
  ed_store         accounts, plots, parked vessels, progression, missions, world deltas
  ed_net           protocol, interest by NodeId, prediction, sky events
  ed_tiles         ew-tiles: the tile service (bake on request, cache, serve chunks)
  ed_server        directory, region shards, missions, NPC scheduler
  ew               the app
```

Dependency rules, enforced by a CI check:

- `ed_sim` and `ed_server` never depend on `ed_gpu`, `ed_render`, `ed_anim` or `ed_editor`.
- Only `ed_data` knows slippy tiles. Everything above it speaks `NodeId`.
- Client, tile service and server share `ed_world`, `ed_planet` and `ed_pack`, so a node
  generated in any of the three places is the same bytes.
- New renderer code targets `ed_gpu` only.

## 4. Coordinates and addressing

- Truth is **f64 body-fixed** (Earth ECEF; Moon and Mars their own frames). Rendering is
  **f32 relative to the camera**, computed once per view per frame (P1-D7). No asset, chunk or
  shader stores a world-space f32 position; chunks store positions relative to their node.
- Bodies sit in an explicit **frame tree** (Sun → Earth → Moon; stations under their parent).
  Crossing between bodies changes the camera's parent frame; it does not change the renderer.
- Reverse-Z with an infinite far plane (P1-D8). A local origin still exists for physics.
- **`NodeId`** is the one address (F1). Node level `L` corresponds roughly to slippy zoom
  `L + 2`: L6 nodes are about 150 km across, L12 about 2.4 km, L15 about 300 m.
- **`ObjectId`** names one generated or authored object and survives regeneration (F5).

Detail: `plans/world.md` §1 and §4.

## 5. World: ladder, streaming, authored layers

| Rung | What a node shows | Indicative use |
|---|---|---|
| R0 | Part instances from the kit, props, individual trees | On foot, driving |
| R1 | One shell mesh per building; tree clusters | Across a district |
| R2 | One merged, simplified proxy mesh per node with a small atlas | Skyline from kilometres away |
| R3 | No meshes: features baked into the node's terrain layers (height bump, albedo, emissive) | From aircraft and low orbit |
| R4 | Imagery and night lights only | Orbit and beyond |

Rungs are chosen by projected screen error, not fixed distances. A parent node draws until
its children are resident, so there are no holes. Every rung can be baked straight from
source data, which is why a city can be visible from 100 km without generating its interiors.

Chunks for a node come from three places behind one interface (F4): the shipped **globe
pack** (L0 to L6, all rungs that apply), the **tile service** (L7 to L12), and **local
generation** (L13 to L15, from regional source extracts the service also delivers). Because
generation is deterministic, a chunk's content hash is the same wherever it was produced.

The world a player sees is **base generation + override layers + live deltas** (F5).
Override layers are text files in the repo, grouped by node. Deltas are rows in `ed_store`.

Detail: `plans/world.md`. Streets: `plans/streets.md`.

## 6. Renderer

- **GPU-driven from the start.** Instances live in GPU-resident pools with stable ids and
  previous-frame transforms. Culling (frustum, horizon, hierarchical depth) runs in compute.
  The CPU never loops over visible objects.
- **Geometry providers** (F3). Provider 1 picks a discrete LOD per instance and emits
  indirect draws. Provider 2 (Track G) selects cluster cuts and rasterises small triangles in
  compute. Both write the same visibility and depth targets.
- **Lighting tiers** (F9, D14). Baseline: cascaded shadows, sky ambient, GTAO. Target: virtual
  shadow maps and voxel dynamic GI on High and Ultra; hardware ray tracing on Ultra.
- **Planet passes:** terrain nodes with virtual-textured layers, one atmosphere model valid
  from ground to orbit, one cloud field, ocean as a layer of the planet surface (D13).
- **Multi-view and picking are core** (P1-D24): the editor, shadow views and reflection views
  all use the same view abstraction; an id buffer answers "what is under the cursor".
- **The alpha Metal renderer is the look reference** (P1-D18). Its atmosphere, clouds,
  tone-mapping and MetalFX integration are re-implemented on the new structure and compared
  against reference captures taken in Phase 0.

Quality tiers: Ultra (desktop RTX-class), High (M3 Pro, 1440p), Low (mobile-class). Every
feature declares its cost per tier and has an off switch.

Detail: `plans/renderer.md`. Water: `plans/water.md`.

## 7. Editor

Editor mode runs inside `ew` (F6): free camera, go-to by coordinate or place name, time of
day, debug overlays for nodes and rungs, picking that reports the source feature and
generator of any object, prefab placement with gizmos and undo, markers, mission volumes,
play-from-here, and hot reload of materials, shaders, rules and prefabs. Everything the
editor writes is an override layer.

Detail: `plans/editor.md`.

## 8. Generation pipeline

```
pinned source data (registry) ──> ed_data readers ──> regional extracts
                                        │
                 ed_world rules: region, biome, streets, buildings, foliage
                                        │
        per node, per rung: terrain layers | instances | shells | proxy | nav graph
                                        │
                       + override layers (authored)   + deltas (live)
                                        │
                          .ewpack chunks keyed by NodeId
```

- One height truth per node, sampled identically by rendering, placement and collision.
- Generators emit placements of authored parts (`solarpunk/spec.md`), never merged meshes,
  at R0. Higher rungs are produced by dedicated, cheaper generators from the same inputs.
- Generation is a pure function of (seed, pinned data version, node, rung). Time-dependent
  state (weather, NPC schedules) additionally takes UTC and is never baked.

## 9. Simulation, characters, vehicles

- Fixed 60 Hz step with interpolation (P1-D16); the sim reads only its declared inputs
  (P1-D17). Replay tests run in CI from Phase 1; bit-exact cross-machine results are measured
  and reported but do not gate phases (F10).
- Vehicles are our models over Box3D queries: raycast suspension, tyre model and gearbox for
  cars; per-surface aerodynamics for planes; buoyancy from the water query; staged rockets
  from `ed_flight`.
- Sim detail tiers: full bodies near players → kinematic → parked records (f64 body-fixed) →
  statistical. Parked vehicles and houses are `ed_store` rows keyed by `NodeId`.
- Characters: glTF/VRM import, retargeting to one standard humanoid, clips and motion
  matching. Players choose toon or PBR; NPCs are PBR (D10).

## 10. Multiplayer and persistence

- Global directory plus region shards. Shards start as regions inside one server process and
  split into processes when load requires it.
- Interest management, shard ownership and chunk requests all key on `NodeId` (F1).
- QUIC transport (F11): streams for reliable state, chunks and override layers; datagrams
  for entity updates. Prediction and reconciliation for owned entities, interpolation for
  others. A sky-event channel replicates launches globally at low rate.
- Terrain, buildings, trees and weather are never sent as geometry. Clients receive chunk
  hashes, override layers and deltas.
- `ed_store` holds accounts, progression, plots, parked vessels, missions, NPC assignments
  and world deltas. The same schema is the solo save.

## 11. Platforms

macOS and Metal first. Windows D3D12 second (its minimal form arrives in Phase 1, F7). iOS on
the Low tier later. Android Vulkan last. Input, UI and audio are cross-platform from the start.

## 12. Invariants

1. Truth is f64; rendering is camera-relative f32.
2. One address (`NodeId`) and one height truth per node. Nothing floats or sinks.
3. Generation is deterministic from (seed, pinned data version, node, rung). No hash-order,
   thread-order or wall-clock dependence. Golden hashes in CI.
4. Geometry is never sent over the network; only chunk hashes, override layers, deltas and
   entity state.
5. A parent node draws until its children are resident. No holes, at any altitude.
6. Authored content is override layers keyed by `ObjectId` and `NodeId`. Nothing authored is
   edited into generated output.
7. Sim and render state are separate. `ed_sim` and `ed_server` build without a GPU.
8. Every rendering feature declares its cost per tier and has an off switch.
9. RED-licensed data never ships; attribution is generated (`data/README.md`).
