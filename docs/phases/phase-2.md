# Phase 2 — Planet, far field and streaming

Status: follows Phase 1. Roadmap row: `../ROADMAP.md`. Design: `../plans/world.md`,
`../plans/renderer.md` §4. Contracts: `../contracts/addressing.md`, `pack.md`,
`generation.md`. Milestones M5 to M7 in `../plans/path-to-solo.md`. Decisions in the P2-D
series below are **proposed** engineering choices; they are locked only when the owner moves
them into `../DECISIONS.md`.

## 1. Goal

One planet renderer from a street to the Moon, with nothing on it yet but ground, sea, sky
and far-field imagery, and a streaming system that shows a complete view first and sharpens
it in place. Phase 3 adds content to nodes; it must not have to touch selection, culling,
residency, uploads or precision.

## 2. What Phase 2 proves

| Question | Proof |
|---|---|
| Does one address and one height truth remove the alpha's gaps? | Scripted ground → orbit → Moon → ground path with the hole detector reporting zero uncovered pixels on every frame |
| Does coarse-first loading fix "slow to load"? | First ground image in 3 s, complete view in 10 s, from a cold process with local packs (`../reference/alpha-baseline.md` §5) |
| Does a budgeted selector fix the churn that the alpha's 200-tile cap caused? | Resident set equals wanted set at every memory setting; lowering the budget lowers detail and never causes eviction of a wanted node |
| Is precision sound from 10 cm to 400,000 km? | No vertex jitter at ground level at any longitude or at the poles; the Moon is a terrain body in the same pass |
| Can the far field be shipped? | Globe pack L0–L6 inside the P0-R2 size; city lights visible on the night side from orbit and from the Moon |
| Can three sources sit behind one loader? | The same node loads from the pack, from `ew-tiles` and from local generation, with identical chunk hashes |

## 3. What Phase 2 does not do

No buildings, roads or trees as geometry (Phase 3); they exist only as R3 tints and
emissive. No VSM, GI, volumetric clouds or waves (Phase 4). No car physics beyond a
placeholder box on wheels (Phase 5). No networking beyond the tile service's plain HTTP.

## 4. Decisions in detail (proposed)

### P2-D1 — Terrain node geometry
Every node draws the same 65 × 65 vertex grid (64 cells) plus a skirt ring, one shared
index buffer with three LOD variants. Vertex positions come from the integer lattice of
P0-R1, evaluated in f64 on the CPU for the node's four corners and its curvature terms, and
displaced in the vertex stage by the node's height page. *Why:* shared edges are
bit-identical by construction, and the mesh costs no upload. *Alternative:* clipmaps centred
on the camera; rejected because they do not key on `NodeId` (F1) and do not extend to other
bodies. *Revisit:* if the terrain pass exceeds its budget, drop to 33 × 33 for nodes above a
projected-size threshold before changing the scheme.

### P2-D2 — Geomorphing, not popping
A node's vertices morph toward the parent's lattice over the outer band of its selection
range, driven by the same projected error that selects it (CDLOD rule). Height pages carry a
one-texel border from neighbours so the morph and the normals are continuous across edges.
Skirts cover only residual cracks between levels and are tested to stay under one pixel.

### P2-D3 — One height truth
A `terrain` chunk holds a 257 × 257 height grid (256 cells, 16-bit, per-node scale and
offset, metres above the ellipsoid) and min/max. A parent's grid is the box-filtered child
grids with **edges copied, not filtered**, so contract test G4 holds. `ed_planet` exposes
`height(body, position_f64) -> (metres, level_used)`, answered from the finest resident
node; the renderer, collision chunks, the mover and every Phase 3 generator call it and
nothing else. The alpha's `TerrainGrid` triangle-diagonal rule is kept so the collider and
the picture agree.

### P2-D4 — Selection by projected error, inside a budget
The selector walks the quadtree from the roots of every visible body with a priority queue
ordered by screen-space error reduction, and stops refining when the next split would
exceed the residency budget (bytes, not node count). The result is the wanted set; it is by
construction never larger than what may be kept. Inputs: camera f64 position, velocity,
field of view, viewport, and a look-ahead position along velocity. *Why:* the alpha wanted
430 tiles and kept 200. *Revisit:* never; this is the baseline rule.

### P2-D5 — Culling order
Horizon cull (cone from camera altitude against the node's bounding sphere raised by its
maximum height), then frustum, then the previous frame's depth pyramid. Horizon and frustum
run on the CPU for nodes (hundreds), and in the Phase 1 compute cull for instances.

### P2-D6 — Parent until children
A node is drawn only when all four children's required chunks are resident and uploaded;
until then the parent draws. Required chunks per rung are declared in the manifest, so
"complete" is one lookup. Upload completes a node atomically at a frame boundary.

### P2-D7 — Precision
Node origins are f64 body-fixed. Per view and frame the CPU computes
`node_origin − camera` in f64 and passes it as f32, with the node's local basis; all vertex
maths is node-local (P1-D7). Body-to-body transforms (Earth-fixed to Moon-fixed) are f64 on
the CPU, from `ed_space`.

### P2-D8 — Texture pages follow nodes
Phase 2 virtual texturing is node-paged: a page id is `(NodeId, layer)`, each resident
terrain node owns one 256² page (plus a 4-texel border) per layer in a fixed-size atlas
array, and terrain selection is texture selection. *Why:* no GPU feedback pass, no
indirection lag, and page residency is the same LRU as node residency. *Alternative:*
feedback-driven sparse VT; deferred. *Revisit:* in Phase 3 if R0 ground needs more than one
page per L15 node.

### P2-D9 — Ground material near, imagery far (F13)
Layers per node: albedo, roughness, emissive, land-cover weights, detail-height. Below a
pixel-footprint threshold the ground is synthesised from land-cover weights and tiling
materials (the alpha's splat maths, ported with a reference test, P1-D18); above it the R4
imagery albedo is used; a blend band between them is driven by pixel footprint, not
distance. Imagery is never the ground you stand on.

### P2-D10 — Atmosphere
One physically based model with precomputed transmittance and multiple-scattering tables, a
sky-view table per view, and an aerial-perspective froxel volume applied to every opaque
pixel including far rungs. Constants per body; the Moon has none. Valid at ground, aircraft
and orbital altitudes with the same code. The alpha's atmosphere maths is ported where it
matches this structure, with a capture comparison.

### P2-D11 — Clouds as one field, shell rendering
A deterministic cloud field: a pure function of (seed, UTC, body position, weather sample).
Phase 2 renders it as a lit shell with parallax and self-shadow terms, plus a cloud shadow
term on the ground. Phase 4 adds the volumetric march over the same field near the camera.

### P2-D12 — Ocean surface v0
Where a node's height is below sea level it draws the sea surface (the geoid, with a
datum offset stored per body) with an analytic water shading model: sky reflection, depth
tint from bathymetry, sun glint, a scrolling normal detail. No displacement. This is a
placeholder that `../plans/water.md` replaces in Phase 4 without changing node selection.

### P2-D13 — Globe pack
Levels L0–L6 (P0-R2), rungs R3 and R4: `terrain` and `layers` chunks for 32,766 Earth nodes
and L0–L4 for the Moon. Built by `ew-bake globe` from pinned sources, deterministic, with
provenance. Budget 2.1 GiB with BC7. Night-light emissive is normalised so brightness is
continuous with Phase 3's street-light density (generation contract §3).

### P2-D14 — Three sources, one loader
`ChunkSource` implementations: `PackFileSource` (Phase 1), `TileServiceSource` (HTTP, with a
local content-addressed cache directory), `LocalGenSource` (calls `generate`). A
`ChainSource` tries them in an order set per level range. All return bytes verified against
the manifest hash. In development `ew` embeds the service's bake function, so no network is
needed.

### P2-D15 — `ew-tiles` v1
A small HTTP service: `GET /v1/manifest/<node>` and `GET /v1/chunk/<hash>`. A missing chunk
is baked on request from pinned data, stored by hash, and served; responses are immutable
and cacheable. No accounts, no TLS termination of its own (a reverse proxy handles that
when it is hosted). It runs on the owner's server PC (D6).

### P2-D16 — Staged uploads
Decode and generation run on `ed_runtime` pools. GPU upload goes through a queue drained
each frame under a byte budget and a time budget, both configurable and recorded by the
harness. No upload happens outside the queue.

### P2-D17 — Simulation on a round world
As `../contracts/sim.md` §3 and §4 specify: Box3D in double precision (P0-R3) works in a
**region frame** (f64 origin at a body-fixed point, axes east, north, up there), re-based
on a tick boundary when players move more than 8 km from the origin. World gravity is zero;
`ed_sim` applies gravity per body toward the body's centre. Static terrain collision is a
height-field shape per resident L13–L15 node within the collision radius of any dynamic
body, added and removed at tick boundaries in sorted `NodeId` order; a body approaching an
unloaded node is frozen until its chunk arrives. Phase 2 implements re-basing and collision
streaming and passes contract tests S5 and S6. *Riskiest item of M5:* it is tested first,
on the equator, at 60° north and at a pole, and across a re-base.

### P2-D18 — Bodies
A body catalog in `ed_space` (radius, datum, rotation, atmosphere constants, pack prefix).
The selector, renderer and height query take a body id. Earth and the Moon are both
terrain bodies from L0; the Sun is a light and a disc.

### P2-D19 — Hole detector
The harness can render with the clear colour set to a reserved value and count pixels that
keep it where a body's analytic disc says ground must be. The count is a CSV column. The
exit gate is zero on every frame of the gate path.

## 5. Budgets (M3 Pro, 1440p, High, local packs)

From `../reference/alpha-baseline.md` §5 for load and worst-frame; pass budgets split a
16.6 ms frame leaving half for Phase 3 and 4 content.

| Item | Budget |
|---|---|
| Launch to first ground image | 3 s |
| Spawn to complete view, standstill | 10 s |
| Worst frame after warm-up, standstill / 60 m/s / 400 m/s | 25 / 33 / 33 ms |
| Steady frame rate | 60 fps |
| Selector, CPU | 0.5 ms |
| Terrain depth and visibility, GPU | 2.0 ms |
| Atmosphere (tables amortised, sky, aerial perspective) | 1.5 ms |
| Cloud shell and cloud shadow | 1.0 ms |
| Ocean v0 | 1.0 ms |
| Total GPU frame on bare planet | 8 ms |
| Upload per frame | 8 MB and 2 ms |
| Terrain and layer GPU memory | 1.5 GB |
| CPU chunk cache | 1.0 GB |
| Globe pack on disk | 2.1 GiB |
| `ew-tiles` warm chunk response on the local network | 20 ms median |

A missed number is reported with its diagnosis, not adjusted.

## 6. Workstreams

| # | Scope | Owns | Done when |
|---|---|---|---|
| W1 | `ed_planet` core: quadtree on `NodeId`, budgeted selector, horizon and frustum cull, residency (LRU over bytes), parent-until-children state machine, height query | `crates/ed_planet` | Property tests: wanted ≤ budget for random cameras; no node visible without complete required chunks; height query matches chunk data to 1 mm |
| W2 | Node-space source data: `SourceView`, slippy and raster resampling into node grids, elevation and bathymetry merge, land cover | `crates/ed_data` (new `node` module); no slippy type exported | G4 edge test: neighbour and parent borders bit-identical for 1,000 random nodes on all six faces and both poles |
| W3 | Terrain rendering: node grid, height displacement, geomorph, skirts, camera-relative transforms, terrain pass in the frame graph | `crates/ed_render/src/terrain*` | Scripted ground-to-orbit path with zero hole pixels and no crack over one pixel |
| W4 | Node-paged texture atlas, layer chunks, ground material synthesis and imagery blend | `crates/ed_render/src/vt*`, `ground*` | Material reference test against the alpha capture; atlas never exceeds its pool |
| W5 | Atmosphere tables, sky, aerial perspective | `crates/ed_render/src/atmosphere*` | Captures at ground, 10 km and 400 km match the reference within tolerance; energy test on the tables |
| W6 | Cloud field (engine-free) and shell rendering; ocean v0 | `crates/ed_world/src/clouds.rs` reuse, `crates/ed_render/src/clouds*`, `ocean*` | Field is bit-identical for the same UTC on two runs; both passes inside budget |
| W7 | `ew-bake globe`: R3 and R4 for L0–L6, Moon L0–L4, pinned inputs, provenance | `crates/ed_assets` (bake), `tools/` acquisition scripts, `data/registry` pins | Pack hash stable across two bakes; size inside budget; licence gate passes |
| W8 | Loader: `TileServiceSource`, `LocalGenSource`, `ChainSource`, upload queue, priorities, look-ahead; `ew-tiles` v1 binary | `crates/ed_tiles`, `crates/ed_runtime` (queue) | Same node from three sources gives the same hashes; upload never exceeds its frame budget in the harness CSV |
| W9 | Bodies: catalog, Earth–Moon transforms, Moon terrain, Sun disc, star field | `crates/ed_space`, `crates/ed_planet` | Moon from Earth is a lit terrain body with correct phase for a given UTC; landing camera on the Moon has no jitter |
| W10 | Sim on the planet: gravity, heightfield collision chunks, mover and placeholder car on terrain | `crates/ed_sim`, `crates/ew` | Mover stands still on a slope at three latitudes; a box dropped from 100 m lands on the rendered surface within 2 cm |
| W11 | Editor stage E2: go-to, time and body, node overlays, selector freeze, streaming monitor, chunk inspector | `crates/ed_editor` | Each overlay is exercised by a headless UI test that reads back the overlay image |
| W12 | Harness: gate path script, hole detector, load-time and upload columns, alpha comparison on the same path (carried from Phase 0) | `crates/ew` harness, `tools/` | The gate path produces one CSV and one frame strip per run |

## 7. Order

```
        ┌─> W3 ──> W4 ──┬─> W5 ──> W6 ─────────────┐
W1 ─────┤               │                           ├─> W9 ──> gate path (W12)
W2 ─────┴─> W7 ─────────┴─> W8 ──> W11 ────────────┘
W10 after W1+W2 (needs height query and collision chunks)
W12 from the first terrain frame on
```

W1 and W2 start together and share no files. W7 (bake) and W3 (render) both need W2 and
then run in parallel. W5 and W8 run in parallel. W10 is independent of all GPU work.

M5 = W1, W2, W3, W10. M6 = W4, W5, W6, W7, W9. M7 = W8, W11, W12 and the exit gate.

## 8. Exit gate

- [ ] One unbroken recording street level → orbit → Moon view → back, with no holes (hole
      detector zero on every frame) and no representation switch.
- [ ] City lights visible on the night side from orbit.
- [ ] Spawn to first ground image within 3 s and to a complete view within 10 s; no frame
      over 33 ms while flying at 60 m/s and at 400 m/s; 60 fps steady.
- [ ] The wanted set never exceeds the residency budget at Low, High and Ultra memory
      settings; nothing wanted is evicted.
- [ ] Edge test G4 green on macOS and Windows.
- [ ] Globe pack: deterministic, inside 2.1 GiB, licence gate passing with every source
      pinned.
- [ ] A node loads from the pack, the tile service and local generation with identical
      hashes.
- [ ] The Moon is a terrain body; a camera can stand on it.
- [ ] Mover and placeholder car on real terrain at three latitudes.
- [ ] Editor E2 overlays and go-to work on the live planet.
- [ ] `CHANGES.md` has the Phase 2 numbers beside the alpha's.

## 9. Failure branches

| If | Then |
|---|---|
| Cracks remain between levels | Widen the height-page border and restrict neighbours to one level apart before considering stitching index variants |
| Jitter at high latitude or altitude | Move the remaining f32 terms into the per-node f64 CPU setup; the lattice mapping is already exact (P0-R1) |
| Complete view misses 10 s | Read the load CSV: if decode-bound, lower page resolution for L7+; if upload-bound, raise the byte budget while watching worst frame |
| Globe pack over budget | Ship L0–L5 and let the service start at L6 (Phase 0 branch) |
| Re-basing disturbs resting contacts (S5 over 1 mm) | Re-base only when every dynamic body in the region is asleep or airborne, up to a hard limit of 16 km; record the diagnosis |
| Node-paged texturing wastes memory on flat ocean | Share one constant page for uniform nodes; flag in the manifest |
| Atmosphere tables too slow to update on time-of-day change | Amortise over frames; they change slowly |
| A dataset cannot be pinned or is not GREEN/YELLOW | It does not enter the pack; use the next source in `../data/README.md` and record it |

## 10. Needed from outside the code

| Need | By | From |
|---|---|---|
| Global elevation and bathymetry at L6 resolution, imagery composite, night lights, land cover, Moon elevation: downloaded and pinned | M6 | Acquisition scripts; the owner approves each download and its size before it runs |
| A host for `ew-tiles` | M7 (the embedded service is enough for the gate) | Owner's server PC |
| Confirmation of F9 (baseline lighting floor) | M6 | Owner |

## 11. What Phase 3 receives

A planet whose nodes can be asked for height, residency and neighbours; a loader that
accepts new chunk kinds without change; a selector that already accounts for bytes; an
atmosphere that already shades far rungs; an editor that can go anywhere; and a harness
that measures load time, worst frame and holes on a fixed path.
