# Rearchitecture: audit and plan (ground → orbit → Moon, one living world)

Status: **ACCEPTED DIRECTION, 2026-11-08.** Companion docs: **`docs/VISION.md`** (owner's
verbatim goals + systems, incl. progression, avatars, space stations and the fourth world),
**`docs/ROADMAP.md`** (phases), **`docs/ARCHITECTURE.md`** (target design),
**`docs/data/`** (full geospatial registry + licensing). This file supersedes the direction
of the old ROADMAP Stages A/B/D; the mapping old → new is in §6.9.

---

## 0. Decisions locked with the user (2026-11-08)

1. **Custom engine, own GPU abstraction.** Native Metal first, D3D12/Vulkan later. Bevy is
   removed completely over the course of this plan. The engine-free crates survive.
2. **One cube-sphere planet for render geometry.** Slippy Web-Mercator tiles stay as the *data*
   layer only. No separate globe mesh, no flat-plane-to-sphere switch, no loading screens.
3. **North star = the full loop** (§1). Build the ground → orbit → Moon → back foundation first,
   then grow visual quality and multiplayer together on that skeleton.
4. **Physics: deterministic, Box3D-style.** Vehicles must feel real (Assetto Corsa cars, MSFS
   planes, real boats and rockets) but tuned faster than life. Ragdolls, dynamic animation,
   destruction, knock-over trees; smoke sims only if the budget allows.
5. **Art: photoreal environment** (Megascans-heavy) with **realistic lighting everywhere —
   PBR materials, GI, real reflections**; **cartoon-proportioned characters** (Fortnite-wacky,
   per `character-spec.md`) whose **cel/toon shading is an opt-in per-avatar material model**
   (realistic PBR is equally supported); **solarpunk world dressing** per `spec.md`.
6. **Servers: region instancing + persistence.** House plots and parked vehicles persist;
   distant rocket launches are visible; dense scripted NPC life; first server is a gaming PC.
7. **Repo is/will be private; the product is a packaged game.** Megascans/Fab assets may live in
   the private repo and ship inside the game. No standalone redistribution of raw assets.
8. **Server: Ubuntu LTS**, headless; cross-platform game code, Linux-first deploy tooling.
9. **All-in on the new engine.** The playable Bevy alpha is preserved on branch
   **`alpha-early`** (crash fixes only); `main` becomes the new engine.
10. **Characters:** start from the owner's rigged GLB (retargeted); later **VRM / VRChat-style
    custom avatars** with a sandboxed material system for self-expression (toon *or* PBR).
    **NPCs are max-realism PBR, never cel** — high-fidelity humans (MetaHuman-class, license to
    verify) and robots with full facial/eye/cloth/hair animation; fidelity scales with distance
    via sim-LOD, not art style.
11. **Game feel:** guided freedom, progression through **ranks/trust/merit rather than money**,
    persistent home bases; violence/currency de-prioritized; handcrafted assets + guided
    procedural, never "all code"; space stations, Moon/Mars civilizations, travel lanes, and a
    fourth human-manufactured industrial world (concept TBD). Verbatim: `docs/VISION.md`.

---

## 1. The north-star scenario, turned into requirements

> Drive out of a dense realistic forest into a city. A friend sees you drive by, gets in the car,
> you do a mission together. Another friend on the Moon mentions loot. You drive to the
> rocketport, blast off at super speed, land on the Moon, do the thing, return to the rocket you
> left on the far side, fly back to the spaceport, get in your car, drive home, log off.

| Beat | Systems it forces | Where we are today |
|---|---|---|
| Drive out of a dense forest | Geometric foliage + cluster LOD, driveable roads, good car physics | Impostor cards + merged tiles; roads v2 unverified; Avian raycast prototype |
| Into a city | Cluster LOD + GPU culling + streaming with fixed memory | Shibuya 27–32 fps settled, unusable while streaming |
| Friend sees you drive by | Region shard, entity replication, interpolation | Ghost avatars only; no vehicle sync |
| Friend gets in the car | Vehicle seats, ownership/authority transfer | Not built |
| Co-op mission | Shared mission state, server authority | Single-player delivery planner only |
| Friend on the Moon, with loot | Multi-body worlds, cross-body presence/chat, Moon terrain | Moon is a sky object |
| Rocketport → Moon at super speed | Seamless planet renderer, time compression, rocket sim | Hard switch at ~200 km; two renderers that "share almost nothing" |
| Rocket left on far side of Moon | Parked-vessel records in f64 body-fixed coords, persistence | Designed (VISION §7 / sim-LOD), not built |
| Car where you left it | Same parked-record system on Earth | Not built |
| Drive home, log off | Housing plots, account persistence, save/load | Not built |

Everything in this plan exists to make that row of beats work, in that order of dependency.

---

## 2. Audit

### 2.1 What is sound and survives

- **f64 ECEF truth + floating origin + camera-relative f32 rendering.** This is also how the
  big space games do it. The "flat Earth feeling" is *not* caused by this; see 2.2 S3.
- **Engine-free crates** (~27k lines, unit-tested): `ed_geo` (629), `ed_data` (2.3k),
  `ed_world` (24k), `ed_flight` (991), `ed_space` (251), `ed_runtime` (966),
  `ed_engine_audio` (3.8k) + `ed_audio` (3.3k), `ed_net` (620) as protocol reference.
  Leaving Bevy does not touch these. This is the project's biggest asset.
- **Design knowledge**, even where the code gets rewritten: `TerrainGrid` single-height-truth
  rule, roads v2 deck-solver design, `beamng-earth-lessons.md`, region/archkit data tables,
  biome/climate model, FFT ocean and probe-GI experiments, the perf-measurement culture and
  harnesses, the engine-sound port, ephemeris, the ghost-protocol rig sync lessons.
- **Determinism and data-versioning rules** (`AGENTS.md` rule 4). Still correct; now they also
  serve server-side generation and replay.

### 2.2 Shortcomings, ranked by how much they block the goals

| # | Shortcoming | Evidence | Goals it blocks |
|---|---|---|---|
| S1 | **Three renderers.** Bevy/WESL world, native Metal fed by *mirroring Bevy `Mesh3d` entities*, separate menu globe | `HANDOFF.md`, `space-visuals.md` | Every visual improvement costs 2–3×; the mirror keeps rendering entity-driven, so GPU-driven is impossible |
| S2 | **Per-tile merged geometry + impostor cards** is the wrong substrate for photoreal density | Shibuya 15–23M tris / 474 draws, 27–32 fps; `CHANGES.md` 2026-11-06/07 | Dense canopy, lush ground cover, cities, destruction, 60 fps |
| S3 | **Two Earth representations with a hard switch** (~200 km) plus a third in the menu | `space-visuals.md`, VISION §3 | The core fantasy. This is a *representation* bug, not a precision bug |
| S4 | **Bevy/Avian coupling**: 23.6k lines of `ed_game`; UI, input, physics, audio, scene all Bevy; Avian not cross-machine deterministic | crate sizes, `ROADMAP` Stage C | Custom engine, server authority, deterministic sim |
| S5 | **Generator emits merged vertex soups, not part instances** | `tilegen.rs`, `buildings.rs` | Solarpunk modular kit, GPU instancing, per-building destruction, interiors |
| S6 | **Data gaps**: OpenMapTiles drops lanes/sidewalks/widths; Sentinel-2 10 m ground colour; no real tree positions; EOX mosaic is CC BY-NC-SA (non-commercial) | `beamng-earth-lessons.md` §2 | Driveable roads, ground realism, dense real forests, selling the game |
| S7 | **Server is a stub** (361 lines); no persistence, missions service, NPC sim, sharding | `ed_server`, `ed_net` | The entire shared-universe dream |
| S8 | **No skinned-character/animation pipeline**; the avatar is a 62-part procedural robot | `body.rs`, `character-spec.md` | The character spec, cel-shaded heroes, NPCs, MotionBricks |
| S9 | **No asset pipeline**: no glTF→cluster bake, no texture compression, no scan import | Solstice has all three | Megascans-based photorealism |
| S10 | **Perf/bug effort is going into a renderer being retired** (B1–B13, M0/M1/M2 slices) | `BUGS.md`, `ROADMAP` Stage A | Opportunity cost; stop |

### 2.3 What we stop doing now

- Stop optimizing per-tile triangle counts and fighting the Bevy↔Metal mirror.
- Stop the `m2.md` wgpu-detour plan; no new Bevy-only features.
- Stop fixing P1 *visual* bugs on the Bevy path (fix crashes only). They get fixed by replacement.
- Never attempt live ground physics from orbit (parked records instead — already decided).
- No generative-AI NPC simulation on the server. Scripted schedules + authored dialogue.
- Do **not** adopt Solstice as the engine. Port its techniques (§3.3).

---

## 3. Target architecture

### 3.1 Crate map

```
SURVIVE, mostly unchanged
  ed_geo           f64 WGS84/ECEF/ENU, TileId, sun
  ed_data          HTTP+cache, elevation, MVT, imagery providers, new sources (§3.6)
  ed_flight        ISA atmosphere, drag, Mach, rocket stages
  ed_space         ephemeris, body catalog
  ed_engine_audio / ed_audio   engine sound kit + mixer
  ed_runtime       job pools, budgets, LRU, scheduler

SURVIVE, refactored output
  ed_world         world rules: regions, biomes, archkit, roads v2, foliage placement,
                   missions planner. Emits PART INSTANCES + heightfields + placement data,
                   not merged per-tile meshes (S5).

NEW
  ed_planet        cube-sphere quadtree per body; slippy→planet sampling; virtual texturing;
                   body frames (Earth ECEF, Moon/Mars fixed frames); LOD by projected error
  ed_assets        bake CLI + library: glTF/FBX (Megascans) → cluster-LOD DAG, BC7/ASTC
                   textures, collision meshes, modular-kit manifests, data bake packs
  ed_gpu           RHI: device, passes, bindless tables, heaps, timers, caps, upscaler + AS
                   interfaces. Backends: Metal (first), D3D12, Vulkan (later)
  ed_render        cluster cull + visibility buffer, software raster, shadows/VSM, voxel-
                   clipmap GI, atmosphere/clouds, ocean/water, post, TAA/MetalFX/DLSS/FSR,
                   skinning, toon/cel material model, translucency, emissives
  ed_sim           fixed-step deterministic sim: Box3D binding, character mover, vehicles,
                   ragdolls, destruction, parked records, sim-LOD tiers. No render types
  ed_anim          skeletons, clips, motion matching, cloth/jiggle, (optional) MotionBricks
  ed_store         persistence: accounts, plots, parked vessels, inventories, missions, NPC
                   homes. SQLite first, Postgres when hosted; same format for SP saves
  ed_net           protocol, interest management, prediction/reconciliation, sky-event bus
  ed_server        region shards, directory, hubs, missions service, NPC scheduler
  ew               the app: winit window/input, UI (egui for debug; game UI retained custom
                   or egui), audio glue, main loop: fixed sim + interpolated render
```

Sim never imports render. Server imports `ed_world`, `ed_sim`, `ed_store`, `ed_net` — never
`ed_gpu`/`ed_render`. Client generation and server generation share `ed_world` + `ed_planet`
with pinned data versions.

### 3.2 Planet: one continuous surface (kills S3)

- **Geometry:** six cube-sphere faces, a quadtree per face. A node is a heightfield patch whose
  vertices are positioned on the ellipsoid with f64 node transforms, rendered camera-relative.
  Curvature is in the vertex transform, so the horizon, the limb and the round planet are the
  same mesh at every altitude. There is no globe object and no transition altitude.
- **Data stays slippy:** elevation (Mapterhorn/AWS), imagery (ortho providers, Sentinel-2
  fallback), OSM/Overture vectors keep `TileId` addressing and the existing caches. `ed_planet`
  samples them into planet space. This supersedes the old GOALS decision #5: the old decision was
  right about data addressing and wrong if applied to render geometry. The two are now decoupled.
- **Imagery = virtual texturing** over the planet, with per-tile colour matching toward the
  Sentinel base (BeamNG §2.1). Ground texture never "turns off" at altitude; it is the same VT
  mip chain from orbit to boots.
- **Objects:** roads, buildings, trees, props anchor through the same height function
  (`TerrainGrid` rule preserved conceptually: one height truth per node, render and collision
  sample the same function).
- **LOD:** nodes split/merge on projected geometric error with geomorphing; object detail comes
  from the cluster DAG (§3.3), so LOD is one continuous error metric, not tile-tier swaps.
- **Ocean:** sea-level ellipsoid shell, FFT detail near the camera (port `ed_metal` ocean work),
  shoaling from scene depth. **Atmosphere:** one Hillaire-style table set used for both
  ground aerial perspective and the orbital limb. **Clouds:** one climate field; volumetric
  raymarch near, 2D shell far, same source. **Night:** Black Marble city lights on the night
  side. **Moon/Mars:** same `ed_planet` code with different body constants, procedural terrain
  first, LOLA/MOLA elevation later; floating origin re-anchors to the body frame.
- **What "no flat plane" means:** local frames and floating origin stay (precision requires
  them), but there is never a visual switch between representations. Acceptance is a recording
  of ground → orbit → ground with no cut, pop or texture regime change.

### 3.3 Renderer: Solstice techniques, plus what Solstice doesn't have

**Port (MIT, credit in CREDITS):** offline cluster-DAG builder (128-tri clusters, locked-boundary
group simplification, watertight cuts), per-frame GPU cut selection, two-phase occlusion culling,
software rasterizer for small clusters, visibility buffer + resolve, cached shadow cascades,
voxel-clipmap GI with multi-bounce + screen traces, the Horizon/UE cloud model, page streaming
with no holes and fixed GPU memory, BC texture pipeline, geometric-foliage thinning.

**Build new (the reasons we are not just forking Solstice):** dynamic objects and previous-frame
transforms, skinned meshes + motion vectors, 64-bit-atomic visibility buffer (Metal, cap-gated),
bindless argument buffers, MetalFX / DLSS / FSR3 upscalers, optional hardware RT, translucent +
emissive materials, **two shading models in one lighting pipeline** — PBR (default, everything)
and **cel/toon as an opt-in character material** (sun/GI/reflection-aware so a cel hero sits in
the photoreal world beside a realistic NPC), destruction mesh swapping, planetary virtual
texturing, VSM later.

**Foliage:** near/mid = geometric leaves/blades with cluster thinning (Solstice-style), sourced
from Megascans/Megaplants conversions + `spec.md` vegetation library + Solstice-style `.species`
procedural trees. Far = billboard clusters *inside the same DAG*, so the separate impostor
renderer and its flicker bugs (B2) die. Ground cover: geometric near, shaded detail far.

**Tiers (every feature has a cost + off switch, rule kept):**

| Tier | Target | Budget |
|---|---|---|
| Ultra | Desktop RTX, 1440p+, DLSS/FSR, RT shadows/GI optional | 60+ fps |
| High | M3 Pro, 1440p output, MetalFX | 60 fps steady, 120 stretch |
| Low | iPhone/mobile, reduced GI/foliage/draw distance | 30 fps |

Perf gates per tier on the reference scenes (city, forest, mountains, ocean, low flight, ascent)
are rebuilt in `ew` from `tools/perf_headless.sh` concepts, plus ground→orbit→Moon routes.

### 3.4 Simulation and vehicles (kills S4, supports destruction)

- **Box3D is the right base** (verified 2026-11-08): Erin Catto, C17, MIT; soft-step solver,
  CCD, character mover, heightfields, wheel joints, island sleep, **cross-platform determinism,
  multithreading determinism, recording/replay**. A pure-Rust port (`box3d-rust`, MIT) claims
  bit-exact parity and has a double-precision feature. It is young (upstream v0.1, June 2026) —
  isolate it behind `ed_sim` so Rapier or a custom solver is a swap, not a rewrite.
  Recommendation: **bind the C17 library** (best perf, upstream momentum) behind `ed_sim`;
  evaluate the Rust port as the long-term answer.
- **Determinism model:** server-authoritative snapshots + client prediction; determinism buys
  replay, regression tests and cheap reconciliation — we do **not** do lockstep networking.
- **Vehicles are custom on top of Box3D queries:** raycast suspension, Pacejka-style tire model
  with combined slip, torque curve + gearbox + assists for cars; per-surface lift/drag for
  planes; buoyancy + wave coupling for boats; `ed_flight` staged rockets. One "simcade" tuning
  table per vehicle (everything a bit faster than life). Car feel is a signoff gate, not an
  afterthought.
- **Character:** Box3D character mover + rocket-boots flight model (existing `ed_flight`).
  Ragdolls from joints; dynamic animation in `ed_anim`.
- **Destruction/trees:** pre-fractured building chunks and hinged trees become dynamic bodies
  only inside player bubbles, server-owned, persisted as deltas. Smoke: GPU particles, tier-gated.
- **Sim-LOD:** full bodies near players → kinematic f64 tier (traffic, vessels on rails) →
  parked records (VISION §7) → statistical (NPC population). Promotion/demotion is one code
  path shared by SP and MP.

### 3.5 Characters and animation

- `ed_anim`: glTF skinned import, skeleton/clip runtime, motion matching as the baseline,
  cloth/jiggle for ears, hair, straps, terrarium (`character-spec.md`), customization slots.
- **MotionBricks (verified):** real-time generative in-betweener (4 recent poses + 1–4 target
  poses), Apache-2.0 code, commercial-use weights, **but the runtime is TensorRT =
  Windows/Linux NVIDIA only**. Role: *optional client-side visual layer* for local + remote
  avatars on capable PCs; the wire format stays inputs/target poses (a few floats), never neural
  state, so Mac/iOS clients fall back to clips/motion matching and every client sees equivalent
  motion. Server never runs it. Evaluate in Phase 7, never a dependency.
- NPC dialogue = authored line pools + mission hooks; schedules in §3.7.

### 3.6 World content and data

- **Megascans (verified free again 2026-11-05, Fab Standard License):** commercial use, any
  engine, no attribution required, AI training allowed. **No standalone redistribution** — fine
  for a private repo + packaged game; never push raw scans to a public repo or asset share.
  Archive the EULA at bake time; keep Poly Haven / ambientCG (CC0) as fallback. Note Megaplants
  ship as `.uasset` (UE-only) → prefer Megascans FBX/glTF + our procedural species; write a
  conversion path only if a needed plant is uasset-only.
- **Solarpunk kit (`spec.md` is the contract):** 2 m / 0.5 m grid, `BIO_RGN_ASSET_VARIANT`
  naming, five families mapped onto the existing 17-region tables. `ed_world` emits *part
  instances* against the kit; `ed_assets` bakes parts into cluster DAGs; `ed_render` instances
  them. This simultaneously delivers the spec, GPU instancing, destruction and interiors.
- **Roads:** finish roads v2 (levels, deck solver, tunnels, junctions, medians, nav graph) and
  add lane/sidewalk/width data. Acceptance is driveability: no floating, grade limits, collision
  == visual, Assetto-grade car can use it.
- **Data additions:** the full master catalog with licensing traffic lights now lives in
  **`docs/data/registry.md`**; acquisition phases, the machine-readable registry and the
  current-source audit (incl. the **WorldClim** and **EOX** replacements, both P0) are in
  **`docs/data/README.md`**. Key sources: Overture Maps (pinned release, ML footprints +
  heights) + Microsoft ML footprints (CDLA Permissive 2.0), OSM lanes/sidewalks via
  Geofabrik/Layercake, NAIP + national orthophotos (15–60 cm) + raw Sentinel-2, Meta/WRI
  1.2 m canopy height + NASA GEDI (real tree positions), USGS 3DEP EPT lidar (bridge
  clearance), GEBCO (bathymetry), SoilGrids, WorldPop (NPC/population reference), NASA Black
  Marble (night lights), GeoNames (packaged gazetteer), CHELSA/TerraClimate/ERA5/MERRA-2
  (WorldClim replacement), Yale BSC/Gaia (stars), NASA LOLA/MOLA (Moon/Mars). Offline bake
  packs with a `meta` signature (data versions + plan signature) so heavy map work ships
  precomputed and deterministic.

### 3.7 Multiplayer, persistence, living world

- **Topology:** global directory + **region shards** (geographic cells with handoff), always-on
  hub instances, separate Moon/Mars shard processes. One universe, instanced by region (user
  decision 6). Start single-process multi-region on the gaming PC; split as load grows.
- **Sky-event bus:** coarse global replication of launches/trajectories so a rocket taking off
  far away — or from the Moon — is visible as a real object anywhere on Earth. Cheap: a handful
  of f64 state updates per second per active vessel.
- **Persistence (`ed_store`):** accounts, housing plots, parked vessels (f64 body-fixed state),
  inventories, **ranks/trust/merit progression** (no currency in the core loop, VISION §1),
  mission progress, NPC home/work assignments. SQLite now, Postgres later; SP save games use
  the same format. First host: **Ubuntu LTS** gaming PC.
- **Netcode:** interest management by speed/altitude; ~20 Hz near / 5 Hz mid / 1 Hz far;
  prediction + reconciliation for owned players/vehicles, interpolation for the rest; seat and
  authority transfer for shared vehicles. Port the protocol-v5 rig lessons, not the Bevy code.
- **NPC life, scripted:** home → work → POI schedules seeded by (npc id, UTC), dialogue from
  authored pools, traffic on the road nav graph, ambient spaceport launches feeding the sky-event
  bus. Simulation tiers: statistical (whole population) → kinematic (same region) → full body
  (near players) → rendered (visible). Density is data-driven per settlement so a gaming-PC
  server stays inside budget. Initial budget target: tens of full-fidelity players, hundreds of
  kinematic agents, thousands of statistical NPCs per shard — measured, not assumed.

### 3.8 Platforms

macOS/Metal first (dev machine), Windows D3D12 second (DLSS/FSR3, RTX RT), iOS Metal Low tier
later (ASTC bakes, tile memory), Android Vulkan last. Input, UI and audio are written
cross-platform in `ew` from day one. macOS bundle/signing at ship time.

---

## 4. What changes for the current codebase, concretely

| Current | Fate |
|---|---|
| `ed_geo`, `ed_data`, `ed_flight`, `ed_space`, `ed_runtime`, audio crates | Keep; extend |
| `ed_world` | Keep rules; replace mesh-merging output with part instances + heightfields |
| `ed_game` (Bevy app) | Retired module by module as `ew` gains parity; then deleted |
| `ed_metal` | Salvage device/layer/timers, ocean FFT, probe-GI bake, shader maths into `ed_gpu`/`ed_render`; the entity mirror is deleted |
| Bevy WESL shaders | Reference only |
| Avian | Replaced by Box3D behind `ed_sim` |
| `body.rs` procedural robot | Replaced by skinned characters; IK lessons move to `ed_anim` |
| `globe.rs`, `OceanGlobe`, `globe_atmo.wesl`, menu globe | Replaced by `ed_planet` (one representation, also used by the menu) |
| `ed_net` ghost protocol | Reference for `ed_net` vNext |
| `ed_server` | Rebuilt as directory + shards + services |
| BUGS.md B1–B13 | Closed by replacement as each subsystem lands; crashes only until then |

---

## 5. Acceptance metrics

- **Seamless:** one continuous recording forest-street → orbit → Moon surface → Earth street, no
  cuts, no representation swap, no texture regime pop, no >100 ms frame after warm-up.
- **Perf:** High tier 60 fps steady on the six reference scenes + the golden-path route on M3
  Pro; fixed GPU memory while streaming; p99 gate enforced in CI-like harness.
- **Persistence:** park car → orbit → Moon → log off → log in: car, rocket, house, inventory and
  mission state intact.
- **Multiplayer:** two clients, one shared car, one co-op mission, one launch visible from
  another region; 8-player hub test on the gaming-PC server.
- **Feel:** car signoff on forest road + city street; character spec readable against photoreal
  world at noon and dusk.
- **Determinism:** golden-hash generation CI + Box3D replay tests on macOS + Windows.

---

## 6. Phased plan

Phases overlap; each ends playable. Sized for one developer directing frontier-model sessions,
with crate ownership kept disjoint between parallel sessions (`AGENTS.md` convention).

**Phase 0 — Freeze and prepare (now, days, pre-reset).** Commit WIP; branch `reboot`. Freeze
Bevy features/visual-bug fixes. Record the six-scene baseline for history. Add golden-hash
generation tests. Vendor + build Box3D samples; clone Solstice and run its native viewer for
study. Collect the map-source list. Start `CREDITS.md` (Solstice MIT, Box3D MIT, Megascans/Fab,
EOX non-commercial warning). Skeleton crates.

**Phase 1 — Engine skeleton (wk 1–3).** `ew` winit app, `ed_gpu` Metal backend (salvaged from
`ed_metal`), fixed-step sim + interpolated render loop, debug UI, jobs, headless screenshot +
perf harness, `ed_assets` bake CLI v1 (glTF → cluster DAG, BC7) with the Solstice builder ported.

**Phase 2 — Planet spike (wk 2–5).** `ed_planet` terrain + imagery VT + unified atmosphere;
ground ↔ orbit continuous flight; procedural Moon. **Gate:** the seamless recording + perf. If
it fails, fall back to improving the old representation before building more on it.

**Phase 3 — Renderer core (wk 4–8).** GPU cull, visibility buffer, software raster, cached
shadows, GI v1, real reflections, post + MetalFX, geometric foliage + Megascans dressing,
PBR + optional cel material model.
**Gate:** forest + city at 60 fps High, fixed memory, no streaming holes.

**Phase 4 — Sim and vehicles (wk 6–9).** Box3D, character mover + boots, car v1 (real tires),
roads v2 driveability, parked records. **Gate:** forest→city drive signoff; park→orbit→return.

**Phase 5 — Golden path, single player (wk 9–12).** Rocket, Moon landing, placeholder loot,
save/load, housing plot. **Gate:** the whole north-star loop solo, recorded.

**Phase 6 — Multiplayer (wk 12–16).** Directory + shards, prediction, shared car + seats,
co-op mission, sky events, chat, server persistence. **Gate:** §5 multiplayer criteria.

**Phase 7 — Living world and polish (ongoing).** NPC schedules, traffic, dialogue, mission
variety, destruction/ragdolls, MotionBricks evaluation, VSM, DDGI/RT, weather, coupled water,
smoke if budget allows.

**Phase 8 — Platforms and ship.** Windows/D3D12, then iOS/Android tiers, packaging, perf gates.

### 6.9 Mapping to the old roadmap

| Old | New |
|---|---|
| Stage A stabilize/measure | Phase 0 (baseline + determinism only; Bevy bug-fixing dropped) |
| Stage B M1/M2 wgpu | Replaced by Phases 1–3 (no wgpu detour) |
| Stage C Together | Phase 6 (+ `ed_store` in Phase 5) |
| Stage D native + Nanite-like | Phases 1 + 3 |
| Stage E lighting | Phases 3 + 7 |
| Stage F play | Phases 4 + 7 |
| Stage G space | Phases 2 + 5 |
| Stage H platforms | Phase 8 |

---

## 7. Risks and honest costs

- **Scope.** This is a multi-year vision for a small team. The phases are ordered so that
  failure at any gate still leaves a playable, better game. Do not start Phase 6 before Phase 5.
- **Custom renderer is the biggest single cost.** Mitigated by porting Solstice's builder/GI and
  salvaging `ed_metal`, and by gating Phase 2/3 before depending on them.
- **Box3D is young.** Behind `ed_sim`; Rapier is the fallback; replay tests from day one.
- **Cube-sphere reprojection work** invalidates some caches and LOD code; bake packs + the
  Phase 2 gate contain it.
- **Licence drift** (Megascans terms have changed twice): archive the EULA, keep CC0 fallbacks,
  never redistribute raw assets. EOX Sentinel-2 is CC BY-NC-SA — must be replaced or licensed
  before selling; national orthos/NAIP are the path.
- **MotionBricks is NVIDIA-only**; treat as an optional client enhancement only.
- **Gaming-PC server**: NPC ambitions are bounded by sim tiers; shard splitting is the scaling
  story, not bigger hardware first.
- **iOS memory**: ASTC bakes, Low tier, reduced GI volume — planned, not promised.

## 8. Open questions for the user

Resolved 2026-11-08: map-data sources (now `docs/data/registry.md`); server OS (**Ubuntu
LTS**); branch strategy (**all-in**, alpha preserved on `alpha-early`); characters (owner's
GLB first, VRM/custom avatars later, realistic + robot NPCs).

Remaining:
1. **Product name** — owner is deciding; `EveryWorld`/binary `earthdrop` stay codenames.
2. Public playtest timeline — informs when Phase 6 hardening must be production-grade.
3. Fourth-world concept (manufactured, beautiful, more industrial) — design exploration after
   Phase 5.

## 9. Immediate next actions

1. **Phase 0** — detailed workstreams, pass criteria and exit gate in `docs/phases/phase-0.md`;
   `ROADMAP.md`, `GOALS.md`, `ARCHITECTURE.md` and `VISION.md` are aligned with this plan.
2. Then **Phase 1** (`ew` + `ed_gpu` Metal skeleton) and **Phase 2** (cube-sphere planet spike)
   start in parallel.

## References

- Solstice: https://github.com/SamG-Coder/solstice (MIT) — cluster DAG, GI, streaming, foliage.
- Box3D: https://github.com/erincatto/box3d (MIT), announcement https://box2d.org/posts/2026/06/announcing-box3d;
  Rust port https://github.com/larsbrubaker/box3d-rust (MIT).
- Megascans free + AI terms: https://quixel.com/news/update-megascans-are-going-free-again;
  license https://www.fab.com/eula (Standard: any engine, no standalone redistribution).
- MotionBricks: https://nvlabs.github.io/motionbricks/, https://github.com/NVlabs/motionbricks
  (Apache-2.0 code; weights NVIDIA Open Model Agreement; TensorRT runtime).
- BeamNG.Earth lessons: `docs/alpha/plans/beamng-earth-lessons.md`; roads v2 `docs/alpha/plans/roads.md`.
