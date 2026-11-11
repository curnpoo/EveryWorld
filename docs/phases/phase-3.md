# Phase 3 — World content on the ladder

Status: follows Phase 2. Roadmap row: `../ROADMAP.md`. Design: `../plans/world.md` §2 and
§4, `../plans/streets.md`, `../plans/editor.md` (stage E3). Contracts:
`../contracts/generation.md`, `layers.md`, `persistence.md`. Milestones M8 to M10 in
`../plans/path-to-solo.md`. Decisions in the P3-D series are **proposed** until the owner
moves them into `../DECISIONS.md`.

## 1. Goal

Put the world on the planet: streets you can drive, every building present at every
distance, forests, and authored places, all as per-rung output of one pure `generate`
function, drawn as instances through the Phase 1 renderer and streamed by the Phase 2
loader. After this phase the alpha has nothing the new engine lacks except lighting
quality (Phase 4).

## 2. What Phase 3 proves

| Question | Proof |
|---|---|
| Does instancing fix the city frame cost? | Shibuya, Manhattan and Aix at 60 fps on High with under 4 M rasterised triangles in the main view (alpha: 14–23 M at 12–40 fps) |
| Does the ladder remove pop-in and empty blocks? | Scripted orbit-to-street descents with the pop detector under threshold and every source footprint represented at every rung |
| Do Overture streets remove road gaps? | Three reference routes driven by a scripted car with zero gaps and no height step over 3 cm |
| Can a generated world be edited? | A landmark placed in the editor is visible from 50 km (in R2 and R3) and survives a re-bake and a data-version bump |
| Is generation deterministic everywhere? | G1–G10 green on macOS and Windows, including the seven hashes quarantined in Phase 0 |
| Is memory fixed? | Resident bytes stay inside the budget over a 30-minute scripted tour of all reference places |

## 3. What Phase 3 does not do

No VSM, GI, wet roads or final materials (Phase 4). No car feel, traffic or pedestrians
(Phase 5); the scripted car follows the nav graph kinematically. No interiors (Phase 8);
the per-storey hook is emitted (generation contract §6). No final art: the kit is
procedural placeholders behind the real asset path, replaced by Blender prefabs as they
arrive without code changes.

## 4. Decisions in detail (proposed)

### P3-D1 — `ed_world` becomes `generate(GenInput)`
The alpha's `tilegen::build` (one slippy tile → merged meshes) is replaced by the contract
function. Existing modules are kept where their maths is sound (`roads/solve`, `junction`,
`archkit`, `style`, `trees/species`, `surface`, `biome`) and re-pointed at `SourceView` and
node space. `TileProjector`, `TerrainGrid` and every slippy type leave `ed_world`; height
comes from `ed_planet`'s query (P2-D3). The alpha entry point is deleted, not kept behind a
switch.

### P3-D2 — Buildings emit instances, not meshes
The facade generator (`buildings::detail_one`, `bparts`) stops writing triangles into
per-material buckets and writes `(part id, transform, material variant, ObjectId)` rows.
Parts are meshes in a **kit pack**: windows, doors, wall panels on the 2 m grid, cornices,
roof pieces, balconies (`../solarpunk/spec.md`). Until authored parts exist the kit pack is
baked from the existing `bparts` generators through `ew-bake`, so the content path is the
real one from the start. Non-grid geometry (irregular footprint walls, pitched roofs) is
emitted as a per-building shell mesh in the R0 chunk, shared with R1 where identical.
*Why:* this is the structural fix for the alpha's vertex-bound frame.

### P3-D2a — Sockets and living surfaces
From the owner's architecture references (`../solarpunk/reference/README.md`): planting and
energy are part of the building. Kit parts and prefabs declare **sockets** (planter, tree,
climber, solar plane, wind mount) in their manifest; the building generator fills them by
rule from the style row, emitting ordinary instances with `ObjectId`s derived from the
building. Roof and wall surfaces may be flagged to take ground cover (turf roof, green
wall), drawn by the same GPU-instanced cover as terrain. Trees on towers go through the
foliage ladder (P3-D7), so a planted skyline has cluster and proxy rungs like a forest.

### P3-D2b — Free-form hero prefabs
Shells, domes, burrows and sculpted earth buildings do not fit the 2 m panel grid. They are
authored whole in Blender as prefabs with a footprint, sockets and an optional terrain
patch, and enter the world two ways: placed in the editor, or chosen by the generator for a
parcel from a per-family list with a rarity. The grid kit remains the way ordinary streets
are filled.

### P3-D3 — Rungs are independent functions
`generate` dispatches on rung to separate code paths that share only inputs and the style
row: R1 from footprint + height + style row; R2 from footprints extruded and simplified with
meshoptimizer, with a small atlas coloured from style rows; R3 from footprint and road
density rasters into the terrain layers. No rung calls a finer rung. Consistency is held by
tests G6, not by derivation.

### P3-D4 — Building source and height
Overture buildings at the pinned release (GERS ids give `ObjectId`s). Height by the
four-step rule of generation contract §4, tables as data files in `data/rules/`. Where a
region has no footprints, parcels from the street plan are filled by rule so that no block
is empty (P1-D25); filled buildings have ids from `(parcel id, generator id, sub-index)`.

### P3-D5 — Streets
Per `../plans/streets.md` and P0-R4. Ingest and normalise run in the bake tools (DuckDB over
GeoParquet) and write a regional **source-extract chunk**; the runtime and the service read
only that chunk. The alpha's roads v2 solver is ported to node space with ownership by
anchor point. Road surfaces are instanced patches plus a terrain patch so nothing floats.
The lane graph and parcels are chunk kinds of their own.

### P3-D6 — Regional styles are tables
`archkit` variants and street kits move to data files (`data/styles/*.toml`) versioned in
`GenVersion`, with hot reload in the editor. The five solarpunk families of
`../solarpunk/spec.md` are rows over the existing region table; a region maps to a family
and a variant set. Styling is two layers: the regional vernacular (what the place really
looks like) and a **retrofit table** applied on top (solar planes, planted balconies,
glazing bays, climbers, lot gardens), because the references show real buildings changed,
not replaced. Each parcel also gets lot dressing by rule: garden, path, fence, fruit trees,
and a pond where the terrain allows.

### P3-D7 — Foliage ladder
Trees are instances of a bank of meshes per species (a fixed number of `Genome` variants
per species baked into the kit pack with LOD chains and a billboard level), then clusters
at R1, canopy mesh in the R2 proxy, canopy colour and roughness in R3 layers. Placement is
from canopy height and land cover with per-cell hashing. Ground cover is GPU-instanced from
per-node parameters, not chunks of meshes. The alpha's CPU impostor atlas is not ported.

### P3-D8 — Transitions
Rung changes cross-fade by dither over a band of projected error, using the Phase 1
per-instance fade. A parent's R2 proxy and its children's R1 shells are never both fully
opaque. The **pop detector** in the harness compares consecutive frames on the scripted
descents after reprojection and reports the largest changed area that is not explained by
camera motion.

### P3-D9 — Override layers in generation
`generate` receives the layers touching the node. Order: source → `road_patch` → street
solve → `suppress` → building and foliage generation → `replace` → `place` → emit, for
every rung. A placed prefab with a declared footprint suppresses generated buildings and
parcel fill beneath it and contributes to R2 and R3. A prefab may carry a `terrain_patch`
(a berm, a terrace, a cut) that is applied with its `place`, so earth-sheltered buildings
sit in the ground at every rung. The bake reports overrides whose targets no longer exist.

### P3-D10 — Deltas
`ed_store` gets its first real use: SQLite with the single schema of
`../contracts/persistence.md`, holding deltas keyed by L12 node in the layer-operation
vocabulary. In Phase 3 the only writer is the editor's "apply as delta" test path; Phase 5
adds parked records.

### P3-D11 — Platform maths
Every generator call that can reach topology or quantised output uses `libm` (generation
contract rule 7). This closes the seven Windows hashes from Phase 0. A lint in `xtask`
(G9) forbids `f32::sin`-style calls, `HashMap` iteration, `std::env`, `Instant` and
`SystemTime` in `ed_world`.

### P3-D12 — Collision and navigation at R0 only
Collision chunks hold building footprints as extruded hulls, road and ground as the
heightfield plus road patches, and trunk capsules. They are produced only at L13–L15 and
only resident near dynamic bodies.

### P3-D13 — Reference routes
1. **Town Mountain Road, Asheville, North Carolina** (owner, 2026-11-10): from the Blue
   Ridge Parkway down the ridge into downtown Asheville. A winding two-lane forest road
   with grade, tight curves and driveways that ends in a city grid. It is the forest-to-city
   drive of the north-star loop in one real road, so it is also the Phase 5 forest-road
   feel route and the proposed setting for the home area (P3-D14).
2. Tokyo: Shibuya crossing onto the elevated expressway and back to grade (stacked levels,
   ramps, tunnels). *Proposed.*
3. Aix-en-Provence: old-town loop to the A8 junction (narrow streets, one-way,
   roundabouts). *Proposed.*

The Zermatt valley road (grades, tunnels, galleries) is run as a fourth, non-gating route.
Asheville becomes a seventh reference region for source extracts and golden hashes.

### P3-D14 — Home area (proposed)
The first authored home plot is placed in the forest off Town Mountain Road, so that the
solo loop's opening drive is reference route 1. The owner may place it elsewhere; the
route stays a reference either way.

## 5. Budgets (M3 Pro, 1440p, High)

| Item | Budget | Basis |
|---|---|---|
| Steady frame rate in the three reference cities and the forest | 60 fps | Roadmap gate |
| Rasterised triangles, main view | 4 M | Alpha 14–23 M; Phase 1 proving ground measurement sets the final figure |
| Visible instances after culling | 300 k | Phase 1 gate is 100 k at 60 fps with headroom recorded |
| Resident instances | 3 M | 64 bytes per instance → about 190 MB |
| GPU memory, total | 6 GB | 18 GB machine shared with the OS and CPU caches |
| CPU memory, total | 5 GB | Same |
| Spawn to complete view in a city or forest | 15 s | Alpha baseline §5 |
| Worst frame at 60 m/s and 400 m/s | 33 ms | Alpha baseline §5 |
| `generate` R0 for one L14 node, one core | 150 ms | Alpha z14 tile took 300–1,000 ms and produced merged meshes |
| `generate` R1 for one L12 node | 50 ms | Shells only |
| `generate` R2 for one L10 node | 200 ms | Extrude and simplify |
| Road height step at any node edge | 0 (bit-identical) | Contract G4 |
| Dangling road ends | under 7% | P0-R4 measurement in Shibuya |

## 6. Workstreams

| # | Scope | Owns | Done when |
|---|---|---|---|
| W1 | Source extracts: Overture buildings and transportation, OSM tag join, canopy height, climate for the reference regions; extract chunk schema; `SourceView` over it | `tools/bake_*`, `crates/ed_data` | Extracts for six regions are pinned, hashed and pass the licence gate |
| W2 | `generate` skeleton, `GenVersion`, rung dispatch, ownership by anchor, layer application order, golden harness moved to chunk output | `crates/ed_world/src/gen*` | G2, G3, G5, G7, G8 green on a synthetic input |
| W3 | Streets: normalise, cross-section rule tables, junction and height solvers in node space, surface patches, terrain patch, lane graph, parcels, R1–R3 ribbons | `crates/ed_world/src/roads/**`, `data/rules/streets*.toml` | Three routes driven by the scripted car with zero gaps; G4 for road heights |
| W4 | Kit pack: part catalogue, procedural placeholder bake, LOD chains, material variants; prefab manifests | `crates/ed_assets`, `data/kit/` | Kit bake is deterministic; every part has at least three LOD levels and a declared error |
| W5 | Buildings R0: facade grammar to instances, shells for irregular geometry, height rule, parcel fill, `ObjectId`s, interiors hook | `crates/ed_world/src/buildings*`, `archkit`, `style`, `data/styles/` | Every Overture footprint in the six regions yields an R0 building; G10 green |
| W6 | Buildings R1, R2, R3 and rung consistency | `crates/ed_world/src/rungs*` | G6 green; a city is visible at every level from L0 to L15 |
| W7 | Foliage ladder and ground cover | `crates/ed_world/src/trees/**`, `foliage*`, `crates/ed_render/src/groundcover*` | Forest reference scene at 60 fps; canopy colour continuous from R0 to R3 |
| W8 | Renderer support: rung selection for content nodes, cross-fades, instance pool streaming, pop detector | `crates/ed_render`, `crates/ed_planet` | Descents under the pop threshold; instance upload inside the Phase 2 upload budget |
| W9 | Layers in generation, re-bake reporting, `ed_store` deltas | `crates/ed_world/src/layers*`, `crates/ed_store` | A `place`, `replace` and `suppress` each change all rungs; landmark-at-50-km test |
| W10 | Editor stage E3 | `crates/ed_editor` | Place, replace, suppress, building parameters, road and terrain patch, layer panel, undo, each with a headless UI test |
| W11 | Collision and nav chunks; scripted car on the lane graph | `crates/ed_world`, `crates/ed_sim`, `crates/ew` | The car completes each route; the mover cannot pass through any building in a 1 km walk test |
| W12 | Determinism closure: `libm`, lint G9, Windows hashes | `crates/ed_world`, `xtask` | 72 of 72 chunk hashes match across macOS and Windows |
| W13 | Authored places: home area, spaceport, Moon site as layers and placeholder prefabs | `layers/`, `data/prefabs/` | Each is reachable with go-to and visible from 50 km where applicable |
| W14 | Harness: descent paths, route runs, 30-minute tour, memory and pop columns | `crates/ew`, `tools/` | Gate runs produce the numbers for `CHANGES.md` |

## 7. Order

```
W1 ─┬─> W3 ──────────────┬─> W11 ─────────────┐
    │                    │                    │
W2 ─┼─> W5 ──> W6 ───────┼─> W8 ──> W14 ──────┼─> exit
    │     ▲              │                    │
W4 ─┴─────┘      W7 ─────┘     W9 ──> W10 ──> W13
W12 alongside W3, W5, W7 (it touches their maths calls)
```

W1, W2 and W4 start together and share no files. W3, W5 and W7 then run in parallel
(streets, buildings, foliage are separate module trees); W5 needs parcels from W3 only for
parcel fill, which is its last step. W9 needs W2. W10 needs W9 and W5.

M8 = W1 (streets part), W2, W3, W11. M9 = W4, W5, W6, W8, W12. M10 = W7, W9, W10, W13, W14
and the exit gate.

## 8. Exit gate

- [ ] Fly from orbit into Shibuya, Manhattan and Aix with no pops, at 60 fps on High, with
      memory fixed.
- [ ] Drive three routes with no road gaps and no height steps.
- [ ] A landmark placed in the editor is visible from 50 km and survives a re-bake.
- [ ] Every source footprint is represented at every rung; no empty blocks in the six
      reference places.
- [ ] G1–G10 green on macOS and Windows; no quarantined hashes.
- [ ] Complete view within 15 s in a city and in a forest; worst frame within 33 ms in
      flight.
- [ ] 30-minute tour with resident memory inside budget and no growth.
- [ ] Override operations `place`, `replace`, `suppress`, `road_patch`, `terrain_patch`
      affect every rung.
- [ ] Editor E3 complete, with undo.
- [ ] Home area, spaceport and Moon site exist as layers.
- [ ] `CHANGES.md` has the Phase 3 numbers beside the alpha's.

## 9. Failure branches

| If | Then |
|---|---|
| City frame rate misses 60 fps | Read pass timers. Vertex-bound: lower part LOD error thresholds, move the R0→R1 switch nearer (rung distances are data). Cull-bound: coarser instance grouping per building. Track G is the longer answer and does not gate |
| Instance count per city exceeds the pool | Merge parts per facade bay into baked bay meshes in the kit; fewer, larger instances |
| Road solver does not scale past one city | Widen the per-node margin before changing the solver (path-to-solo §6) |
| R2 proxies look wrong against R1 | Derive R2 by simplifying R1 for hero cities only; keep the independent path elsewhere |
| Parcel fill produces implausible blocks | Restrict fill to parcels with a land-use class; leave others as planted ground, never empty paving |
| Windows hashes still differ after `libm` | Quantise the affected intermediate values and record the tolerance; do not quarantine again without a cause |
| Overture extract lacks a field the vector tiles had | Take it from OSM tags and record it (Phase 0 branch) |
| No authored kit parts arrive | The gate passes on the procedural kit; P1-D25's architecture requirement is judged at the Phase 4 gate |

## 10. Needed from outside the code

| Need | By | From |
|---|---|---|
| Overture buildings and transportation with OSM tags, canopy height, climate normals for the six regions, pinned | M8 (streets), M9 (buildings), M10 (canopy) | Acquisition; the owner approves each download |
| First regional kit family as Blender prefabs | M9 if available; otherwise Phase 4 | Art |
| Spaceport, home and Moon-site prefabs | M10 (placeholders acceptable) | Art |
| Confirmation of routes 2 and 3 and of the home area (P3-D13, P3-D14) | M8 | Owner |
| Free-form hero prefabs per family, with sockets (P3-D2b) | M10 if available; otherwise Phase 4 | Art |
| Legal review of baked ODbL chunks | Before any public build | Owner |

## 11. What Phase 4 receives

A complete world at every distance, drawn as instances with stable ids, with consistent
colour and light across rungs; reference scenes and routes that run from a script; and a
frame with measured headroom for shadows, global illumination and water.
