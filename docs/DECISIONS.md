# Decisions log

The single list of locked decisions. Other docs link here and do not restate decisions in
their own words. A decision changes only when the owner changes it; the change is recorded
here with a date, and the old text is struck through, not deleted.

- What the game is: `VISION.md`
- How the decisions fit together: `ARCHITECTURE.md`
- Order of work: `ROADMAP.md`

Four groups: **D** = owner decisions about the product and direction, **F** = foundation
decisions about how the engine is shaped (2026-11-10 review), **P0-R** = choices settled by
Phase 0 measurements, **P1-D** = Phase 1 engineering choices (detail and alternatives in
`phases/phase-1.md`).

---

## D — Owner decisions (2026-11-08 to 2026-11-10)

| # | Decision |
|---|---|
| D1 | **Custom engine in Rust with our own GPU abstraction.** Metal first, D3D12 and Vulkan later. No Bevy, no wgpu. Reconfirmed 2026-11-10 after an engine review. |
| D2 | **One cube-sphere planet** for render geometry. Slippy Web-Mercator tiles are only a source-data format. No separate globe, no flat-plane switch, no loading screens. |
| D3 | **North star is the full session loop** (`VISION.md` §2). Solo loop before multiplayer. |
| D4 | **Physics is Box3D** (C17, MIT) behind `ed_sim`, with custom vehicle models. |
| D5 | **Art:** photoreal environment, expressive characters, solarpunk built world. Lighting, materials, reflections and GI are realistic everywhere. Cel shading is a per-avatar material option. Handcrafted assets plus guided procedural generation. |
| D6 | **One persistent universe** on region-instanced servers. First host is an Ubuntu LTS gaming PC. Scripted NPC life, no generative-AI NPC simulation. |
| D7 | **Progression is ranks, trust and merit, not money.** |
| D8 | **Private repo, packaged commercial game.** Fab/Megascans assets ship only inside the game. |
| D9 | **All-in on the new engine.** The Bevy alpha is frozen on `alpha-early`. Its visual bugs are replacement criteria, not a fix list. |
| D10 | **Characters:** owner's GLB first, VRM custom avatars later (toon or PBR). NPCs are realistic PBR only. |
| D11 | **Alpha docs are archived** in `docs/alpha/`. |
| D12 | **Building interiors are in scope.** No door-teleports. |
| D13 | **Water is designed from scratch** (`plans/water.md`). The alpha FFT ocean is not ported. |
| D14 | **Lighting targets are virtual shadow maps and voxel/Lumen-class dynamic GI.** The alpha's probe GI and cached shadow cascades are not targets. |
| D15 | **The engine ships with an editor** (2026-11-10): load the world, place specific buildings, markers and missions. See F6. |
| D16 | **Far-field view distance is a foundation requirement** (2026-11-10): the Moon, the Earth from orbit, a country from space, and city structures from very far away must all be visible. See F2. |
| D17 | **Cluster ("Nanite-style") geometry is developed in parallel with the engine**, not as a prerequisite for it (2026-11-10). See F3. |
| D18 | **Plans carry gates, not dates** (2026-11-10). No time estimates in plan documents. |

Open owner questions: product name; public playtest timing; fourth-world concept; source of
realistic NPC human assets; when raw assets (`Solarcat.glb`) get committed. Three engineering
points are flagged for the owner in F9, F10 and F11.

---

## F — Foundation decisions (2026-11-10)

Each of these exists because the previous plan left a load-bearing part undesigned. Design
detail is in the linked brief.

| # | Decision | Why | Brief |
|---|---|---|---|
| F1 | **One node address for everything.** `NodeId = (body, cube face, level, x, y)`, packed in 64 bits. The renderer, streaming, bake packs, persistence, network interest and the editor all key on it. Slippy `TileId` is used only inside `ed_data` to read source data. | The alpha addressed render geometry, data and caches differently, which caused the gaps between layers. One key means "is this node loaded, baked, saved, replicated" is one question. | `plans/world.md` §1 |
| F2 | **A ladder of representations per node**, R0 to R4: part instances → one shell per building → one merged proxy per node → features baked into terrain layers (height, albedo, night-light emissive) → imagery and night lights. Any rung can be produced directly from source data without building the rungs below it. | Far cities need a representation that is cheap to produce and draw. The alpha could only show a city by generating all of it. | `plans/world.md` §2 |
| F3 | **Geometry is a slot.** Every mesh in a pack has a required discrete LOD chain and an optional cluster-DAG chunk. The renderer draws through a geometry-provider interface. Provider 1 (discrete LOD, GPU instance culling) is the base. Provider 2 (cluster DAG, software raster) is Track G and plugs into the same slot. | Lets the engine reach a full planet without waiting on cluster rendering, and lets cluster rendering land without changing the engine. | `plans/renderer.md` §3 |
| F4 | **Streaming has three sources behind one chunk format.** A shipped globe pack (coarse levels of the whole planet), our own tile service (middle levels, baked on first request and cached), and runtime generation on the client (finest levels). All are content-addressed chunks keyed by `NodeId`. | The alpha loaded slowly because every node was generated from raw data on the player's machine. | `plans/world.md` §3 |
| F5 | **Stable object ids and override layers.** Every generated object has an id derived from its source feature and generator, not from the node it sits in. Authored changes are text override layers (place, replace, suppress, marker, mission, road patch). The world is base generation + override layers + live deltas. | This is what makes an editor possible on a generated planet, and it is the same mechanism persistence and multiplayer deltas need. | `plans/world.md` §4 |
| F6 | **The editor is a mode of the `ew` binary**, started with the engine in Phase 1 and grown each phase. Blender (glTF) is the asset and prefab authoring tool; we do not build a mesh editor. | An editor built after the engine needs picking, ids, hot reload and multi-view rendering retrofitted. Built with it, those are just engine features. | `plans/editor.md` |
| F7 | **A second GPU backend is brought up to a minimal level in Phase 1.** | Metal has no explicit resource-state transitions. An RHI shaped only by Metal will be wrong for D3D12 and Vulkan. A small second backend fixes the interface while it is cheap. | `plans/renderer.md` §2 |
| F8 | **Streets come from Overture transportation plus raw OSM tags**, not OpenMapTiles vector tiles. A rule table infers cross-sections; a solver produces junctions and heights; output includes a nav graph and parcels. | Vector tiles are clipped, simplified and drop most tags. | `plans/streets.md` |
| F9 | **A baseline lighting path exists from Phase 1 and stays as the Low tier:** cascaded sun shadows, sky ambient and GTAO. VSM and voxel GI (D14) replace it on High and Ultra when they are measured to fit. *Flagged for owner:* this is a floor, not a return to the alpha's probe GI. | The dev machine is an M3 Pro with 18 GB. Every phase must stay runnable and measurable while D14 is being built, and mobile needs a cheap path anyway. | `plans/renderer.md` §5 |
| F10 | **Sim replay tests run in CI from Phase 1, but cross-machine bit-exact determinism does not gate a phase.** The server is authoritative, so the game works without it. *Flagged for owner.* | Box3D is at v0.1. Its determinism claim should be tested, not assumed. | `ARCHITECTURE.md` §9 |
| F11 | **Network transport is QUIC** (`quinn`): reliable streams for state and chunks, datagrams for entity updates. *Proposed; flagged for owner.* The previous plan named no transport. | One library, encrypted, works through NAT, and has both delivery modes. | `ARCHITECTURE.md` §10 |
| F12 | **External renderers are references, not dependencies.** Solstice is a single-author project published 2026-11-07 and untested on Apple GPUs. We read it, measure it on the M3 Pro in Phase 0, and port ideas that survive measurement. | A plan must not depend on a week-old repository behaving as advertised. | `phases/phase-0.md` W4 |
| F13 | **Data corrections.** Overture buildings and transportation are ODbL. Google Photorealistic 3D Tiles are excluded (no caching or offline use). Imagery is a far-field source only; near-field ground is synthesized from land cover and materials. Building heights are inferred where missing, by a written rule. | Licence errors are one-way doors. Present-day orthophotos up close would show parking lots in a solarpunk world. | `data/README.md` |

---

## P0-R — Settled by Phase 0 measurement (2026-11-10)

Engineering choices made from spike results. Each is reversible behind its contract.

| ID | Choice | Measurement | Record |
|---|---|---|---|
| P0-R1 | **Cube mapping is the tangent warp**, evaluated from integer lattice indices with a software `tan`. | Node area ratio 1.40 (gnomonic 5.03, S2 quadratic 2.02); shared edges bit-identical across faces | `reference/spikes/w4d-addressing.md`, `contracts/addressing.md` |
| P0-R2 | **Globe pack ships levels 0–6** (32,766 nodes). | About 1.2 GiB with JPEG-class colour, 2.1 GiB estimated with BC7; height is dominated by ocean bathymetry | `reference/spikes/w4d-addressing.md`, `contracts/pack.md` |
| P0-R3 | **Box3D is used as vendored C at a pinned commit behind our own binding in `ed_sim`**, built with double-precision positions. | Identical state hashes across three processes and across SIMD and scalar builds; about 0.5 ms per step for 1,100 bodies. Cross-machine not yet run | `reference/spikes/w4a-box3d.md`, `contracts/sim.md` |
| P0-R4 | **Roads come from Overture release 2026-10-21.0**; connectors are junction nodes; segments are split at rule boundaries; lane counts come from OSM tags. | Dangling road ends fall from 44% (vector tiles) to 7% in Shibuya | `reference/spikes/w4e-streets.md` |
| P0-R5 | **The Phase 1 second backend is Vulkan through `ash`**, developed on the Mac with MoltenVK and validation layers. D3D12 follows in Phase 9. | The interface draft was walked against Metal, Vulkan and D3D12; Vulkan is the only one of the two that can be run and validated on the development machine | `contracts/rhi.md` |
| P0-R6 | **Cluster geometry (Track G) starts from meshoptimizer's `clusterlod`**; Solstice stays a reading reference. | About 1 M triangles per second build, 2.0× stored triangles, deterministic; M3 Pro compiles 64-bit atomics. Solstice ran on the M3 Pro but its frame rate was not measured | `reference/spikes/w4b-geometry.md` |
| P0-R7 | **Asset import must handle WebP textures and `KHR_texture_transform`**, and retargeting cannot assume neck and head bones. | The owner's character: 16 joints, 15 of 17 humanoid bones matched, no animations | `reference/spikes/w4c-asset-import.md` |

---

## P1-D — Phase 1 engineering decisions

Summary only. Alternatives, reasons and revisit conditions are in `phases/phase-1.md` §3.

| # | Choice |
|---|---|
| P1-D1 | Shaders in Slang with an MSL escape hatch; `ed_gpu` accepts precompiled backend shaders |
| P1-D2 | `objc2-metal` bindings inside a private Metal backend module; the RHI exposes no Metal types |
| P1-D3 | Explicit frame graph with declared resource states and a manual escape hatch |
| P1-D4 | Two to three frames in flight; uncapped mode for measurement |
| P1-D5 | Main thread orchestrates; `ed_runtime` worker pools for IO, generation and upload staging |
| P1-D6 | Own entity index and typed stores; the render path reads flat frame data, never an ECS |
| P1-D7 | f64 body-fixed truth; one camera-relative f32 origin per view per frame |
| P1-D8 | Reverse-Z, infinite far plane, 32-bit float depth |
| P1-D9 | Bindless resource tables owned by `ed_gpu` (Metal argument buffers; descriptor indexing elsewhere) |
| P1-D10 | Capability probe at startup; every tiered feature gated by a named flag |
| P1-D11 | Own `.ewpack` container of content-addressed chunks; meshes carry a discrete LOD chain and an optional cluster chunk (F3); built with meshoptimizer |
| P1-D12 | One material interface, two models (PBR and toon) on the same lighting inputs; materials are data |
| P1-D13 | BC7 on desktop, ASTC on mobile; texture pages addressed for virtual texturing from the start |
| P1-D14 | egui for developer UI **and the editor**; game HUD and menus are a separate authored layer |
| P1-D15 | Headless harness writes PNG and CSV; GPU measurements are local or self-hosted, never hosted CI |
| P1-D16 | Fixed 60 Hz sim with accumulator and interpolation; minimal Box3D world |
| P1-D17 | Sim reads only (fixed dt, inputs, seed, data version, UTC); never the wall clock |
| P1-D18 | Salvage policy: the alpha Metal path is the look reference; structure is rebuilt; pure shader maths is ported with tests |
| P1-D19 | Proving-ground test scene, including a field of 100k part instances |
| P1-D20 | Phase 1 budgets, recorded by the harness |
| P1-D21 | Geometry-provider interface with the discrete-LOD provider as the Phase 1 gate (F3) |
| P1-D22 | Minimal second backend renders the proving ground (F7) |
| P1-D23 | `NodeId`, `ObjectId` and the override-layer reader exist in Phase 1, before any planet (F1, F5) |
| P1-D24 | Multi-view rendering and GPU picking are core renderer features (F6) |
| P1-D25 | The alpha look captures are a floor, not the target (owner, 2026-11-10). Every reference scene must clearly exceed them in detail and fidelity, lighting, clouds and building fill (every building present, no empty blocks), and show the solarpunk architecture of `solarpunk/spec.md` instead of the alpha's plain boxes. Refines P1-D18: the alpha's shader maths may be ported, but its look is not the goal |
