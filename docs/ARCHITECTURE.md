# Architecture (target)

This describes the **target** architecture for the custom-engine reboot on `main`
(`docs/plans/rearchitecture.md`, `docs/VISION.md`). The shipped Bevy alpha and its docs are
preserved on branch `alpha-early`; where the alpha differs, this file wins for new work.

## The one-sentence version

**Real data decides what is there. Authored assets and procedural rules decide how it looks.
One continuous planet renderer covers pebble to orbit to other worlds; a deterministic
fixed-step simulation and region-sharded servers make it one persistent, shared universe.**

## Stack

| Layer | Choice | Why |
|---|---|---|
| Language | Rust (2024) | One toolchain for client, server and tools |
| Engine | **Custom (`ew`)** — winit, own scene/sim core | Bevy's entity model and renderer were the ceiling |
| GPU | **`ed_gpu` RHI**: Metal first, D3D12 + Vulkan later | 64-bit atomics, bindless, tile memory, MetalFX/DLSS/FSR, optional HW RT |
| Renderer | **`ed_render`**: cluster-LOD DAG, GPU culling, visibility buffer, software raster, voxel-clipmap GI | Photoreal density (Solstice techniques, ported; MIT) |
| Planet | **`ed_planet`**: cube-sphere quadtree per body | No flat-plane/globe switch; poles work |
| Physics | **Box3D** (C17, MIT) behind **`ed_sim`** | Cross-platform determinism, replay, character mover, CCD |
| Net | **`ed_net`** + **`ed_server`**: directory + region shards, prediction/interpolation, sky-event bus | One persistent universe on a gaming-PC host |
| Persistence | **`ed_store`**: SQLite → Postgres; same format for SP saves | Plots, parked vessels, ranks, missions, NPC lives |
| Server OS | Ubuntu LTS (headless) | Owner's host; cheap to scale later |
| Data | `ed_data` + `data/registry/` + baked packs (`docs/data/`) | Licensed, pinned, shippable without live APIs |

## Crates

```
SURVIVE FROM THE ALPHA (engine-free, unit-tested)
  ed_geo           f64 WGS84/ECEF/ENU, TileId, sun
  ed_data          HTTP+cache, elevation, MVT, imagery providers, registry-driven sources
  ed_flight        ISA atmosphere, drag, Mach, rocket stages
  ed_space         ephemeris, body catalog
  ed_engine_audio / ed_audio   engine sound kit + mixer
  ed_runtime       job pools, budgets, LRU, scheduler
  ed_world         world RULES: regions, biomes, archkit, roads v2, foliage placement,
                   missions. Emits PART INSTANCES + heightfields + placements (refactored:
                   no more merged per-tile vertex soups)

NEW
  ed_planet        cube-sphere quadtree; slippy→planet sampling; virtual texturing;
                   body frames (Earth ECEF, Moon/Mars frames); projected-error LOD
  ed_assets        bake CLI + library: glTF/FBX (Megascans, VRM) → cluster DAG, BC7/ASTC,
                   collision, kit manifests, data bake packs
  ed_gpu           RHI: device, passes, bindless tables, heaps, timers, caps, upscaler + AS
  ed_render        cull + visbuffer + software raster, shadows/VSM, GI, atmosphere/clouds,
                   ocean, post, TAA/MetalFX/DLSS/FSR, skinning, cel/toon + PBR materials
  ed_sim           fixed-step deterministic sim: Box3D, character mover, vehicles, ragdolls,
                   destruction, parked records, sim-LOD tiers. No render types
  ed_anim          skeletons, retargeting, clips, motion matching, cloth/jiggle,
                   (optional) MotionBricks client layer
  ed_store         accounts, plots, parked vessels, ranks/trust/merit, missions, NPC lives
  ed_net           protocol, interest management, prediction/reconciliation, sky events
  ed_server        region shards, directory, hubs, missions service, NPC scheduler
  ew               the app: winit, input, UI (egui debug; game UI), audio, main loop
```

Dependency direction: sim never imports render; server never imports `ed_gpu`/`ed_render`;
client and server share `ed_world` + `ed_planet` generation with pinned data versions.

## Coordinates (the big-world problem)

- Truth is **f64 body-fixed** (Earth ECEF; Moon/Mars their own frames). Render is **f32
  camera-relative**, computed from f64 node transforms. Never do far-from-origin math in f32.
- A **floating origin** still exists for physics and local frames — precision requires it —
  but there is no *visual* switch between representations at any altitude.
- Reverse-Z infinite projection; far plane huge on purpose. Moving entities are carried
  across rebases; parked/static things store f64 placements.

## Planet rendering

- Six cube-sphere faces, quadtree per face; a node is a heightfield patch positioned on the
  ellipsoid, with geomorphing between levels. Curvature lives in the vertex transform, so
  ground, horizon, limb and orbit are the same mesh.
- **Slippy tiles remain the data layer** (`TileId`, caches, Mapterhorn/OpenFreeMap); the
  planet samples them. Render geometry and data addressing are decoupled.
- **Imagery is virtual texturing** over the planet (orthophotos + raw Sentinel-2, colour
  matched), one mip chain from orbit to boots — no satellite "layer" that turns off.
- One **atmosphere** (Hillaire-style tables) for ground and orbital views; one **cloud
  field** (volumetric near, 2D shell far, same climate source); **night lights** on the dark
  side; **ocean** as a sea-level shell with FFT detail near the camera.
- Moon/Mars/fourth world use the same code with different body constants; stations and the
  manufactured world are instanced structure worlds in orbit/body frames.

## World generation pipeline

```
reference data (pinned) ──> ed_data decode ──> ed_planet sampling (height/imagery)
                                   │
              biome + region + settlement rules (ed_world)
                                   │
        terrain heights | water | road plans | PART INSTANCES | plant placements
                                   │
        authored kit + Megascans assets (ed_assets bakes: cluster DAG, collision, LOD)
                                   │
        state 03: climate/weather/ecosystem simulation (seed, UTC) + persisted deltas
```

- One height truth per node, sampled identically by render, placement and collision.
- Generators emit **placements of authored parts** (solarpunk kit, `docs/solarpunk/spec.md`),
  enabling GPU instancing, destruction and interiors. Handcrafted hubs/landmarks/missions are
  first-class content inserted into the generated world (VISION §8).
- Deterministic from (seed, pinned data version, UTC); golden-hash CI.

## Rendering

- **Geometry:** offline cluster-LOD DAG (128-tri clusters, watertight cuts), per-frame GPU cut
  selection, two-phase occlusion culling, software raster for small clusters, one visibility
  buffer, material resolve shades each pixel once.
- **Streaming:** paged geometry/textures with fixed GPU memory; a parent draws until its
  children are resident — no holes.
- **Lighting:** physically based atmosphere + sun; cached shadows first, VSM later;
  voxel-clipmap GI with multi-bounce + screen traces; GTAO/SSR; HDR + TAA/upscalers; bloom,
  exposure, filmic/ACES tonemap.
- **Materials:** data-driven, hot-reloadable; PBR for the world, **cel/toon for characters**
  (sun/GI-aware so they sit inside the realistic lighting), translucency, emissives.
- **Foliage:** geometric leaves/blades with cluster thinning near; billboard clusters inside
  the same DAG far; lush ground cover near, shaded detail far.
- **Tiers:** Ultra (desktop RTX), High (M3 Pro, 60 fps at 1440p), Low (mobile). Every feature
  has a cost per tier and an off switch; perf gates on the reference scenes + golden path.

## Characters, vehicles, physics

- **Characters:** glTF/VRM skinned import + retargeting onto a standard humanoid; clips +
  motion matching; cloth/jiggle per `character-spec.md`; avatar material sandbox for player
  self-expression; realistic human + robot NPCs. MotionBricks is an optional NVIDIA-only
  client-side layer; the wire carries inputs/target poses, never neural state.
- **Vehicles:** custom models over Box3D queries — raycast suspension + Pacejka-style tires +
  gearbox for cars (Assetto-grade feel), per-surface aero for planes, buoyancy for boats,
  staged rockets from `ed_flight`. Simcade tuning: faster than life, signoff-gated.
- **Sim-LOD:** full bodies near players → kinematic f64 tier → parked records (f64 body-fixed
  state) → statistical population. Parked vessels/houses persist in `ed_store`; no live
  ground physics from orbit.
- **Destruction:** pre-fractured buildings and hinged trees become dynamic only inside player
  bubbles, server-owned, persisted as deltas. Smoke is tier-gated GPU particles.

## Multiplayer, persistence, living world

- **Topology:** global directory + region shards (geographic cells with handoff), always-on
  hubs, separate Moon/Mars processes; single-process multi-region on the first Ubuntu host.
- **Sky-event bus:** coarse global replication of launches/trajectories so distant takeoffs
  are visible anywhere.
- **Netcode:** interest by speed/altitude (~20/5/1 Hz tiers), prediction + reconciliation for
  owned entities, interpolation otherwise; seats + authority transfer for shared vehicles.
  Determinism aids replay/reconciliation; no lockstep.
- **Persistence:** accounts, ranks/trust/merit progression (no currency in the core loop),
  housing plots, parked vessels, inventories, mission state, NPC home/work assignments.
- **NPC life:** scripted schedules seeded by (npc id, UTC), authored dialogue/lore, traffic
  on the road nav graph, ambient spaceport launches. Statistical → kinematic → full →
  rendered tiers keep a gaming-PC server inside budget.

## Platforms

macOS/Metal first; Windows D3D12 (DLSS/FSR3, RTX) second; iOS Metal Low tier later (ASTC,
tile memory); Android Vulkan last. Input/UI/audio are cross-platform in `ew` from day one.

## Invariants (carried forward, still binding)

1. Truth is f64; render is camera-relative f32. No far-from-origin f32 math.
2. One height truth per node; nothing floats or sinks.
3. Generation deterministic from (seed, pinned data version, UTC); no hash-order/float-
   reduction/thread-order nondeterminism; golden-hash CI.
4. Terrain/buildings/trees/weather are never sent over the network — only deltas and entity
   state. Sim and render state are separate; fixed-step sim, interpolated render.
5. Every rendering feature declares cost per tier and has an off switch.
6. Data licensing: RED never ships; attribution is generated (`docs/data/README.md`).
