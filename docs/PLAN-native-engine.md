# Plan: the EveryWorld engine

Status: proposed, Nov 2026. This is the target architecture.

Vision set by user: a **120 fps whole-Earth sim sandbox**, extending to other planets and the wider universe. It needs:
- **Rendering:** Nanite-like geometry, DLSS-style upscaling, global illumination, motion blur, AO and a full post chain that gives the world a strong sense of grounding through lighting.
- **Gameplay systems:** NPCs, water simulation, missions, and persistent props and objects anywhere on Earth.
- **Platforms:** Windows and phones eventually, with multiplayer.
- **Ownership:** our own systems, borrowing from research and open source. Not a game built on someone else's engine.

Open decision: **Rust + wgpu**, or **full native** (our own GPU layer over Metal, Vulkan and DX12)? See §3. The plan is set up so this choice can be made with measurements rather than up front.

Related plans:
- `PLAN-engine.md`: asset library, agents, vibes.
- `PLAN-perf.md`: meshlets.
- `PLAN-world-sim.md`, `PLAN-space-visuals.md`.

---

## 1. Where we are (measured, M3 Pro, Ohio suburb, 67% render scale)

| Config | Steady fps | Notes |
|---|---|---|
| Meshlets on (old default) | 23.6 | Bevy meshlet culling and raster run once per view, 5 views |
| Ordinary meshes (current default) | 33.5 | Frame ~30 ms, very flat |
| Clouds high / medium / low / off | 33.3 / 33.3 / 33.3 / 33.5 | Clouds are **not** the limit |
| 480x270 window | same as full res | **Not** fill-rate bound |
| No post, no shadows | 45-48 | Geometry passes removed |
| Spawn, first 2 s | ~10 | 370 ms frames (tile apply plus pipeline compiles) |
| Settings change | 3-6 s stall | Pipeline/shader compile on the main path |

Scene: 89 tiles, ~16.5M tris resident, ~22k visible meshes, 56k entities, 85.8k trees, 24.5k buildings.

Metal System Trace, GPU time per frame:

| Pass | Time per frame |
|---|---|
| Main opaque | vertex 5.1 ms, fragment 7 ms |
| Depth prepass | vertex 4 ms, fragment 1.7 ms |
| Transparent (clouds) | ~9 ms fragment. It overlaps other work, so it is hidden today |
| Shadows (3 cascades) | ~2.5 ms |
| SSAO | 1.7 ms |
| Atmosphere env map, regenerated every frame | ~2 ms |
| TAA, bloom, UI, CAS | ~1.5 ms |

**Diagnosis.** Apple GPUs are tile-based deferred renderers: vertex and tiling work runs separately from fragment shading. Our frame is limited by **triangles and draws through the vertex stage** (16M tris × main + prepass + 3 cascades), not by pixels. Resolution, clouds and FSR barely move fps; removing geometry passes does.

**CPU.** The main and render threads are parked about 80% of the time, waiting on the GPU. Hitches come from main-thread spikes:
- tile apply (entity and collider inserts, material clones, texture uploads);
- buffer frees;
- pipeline compiles.

**What "Bevy holds us back" really means.** It is the renderer, not the ECS:
- Every view re-queues the whole mesh list.
- The renderer has no notion of a GPU-resident planet-scale world.
- Tiles become hundreds of entities each.
- Pipelines are specialized lazily, so new combinations compile mid-game.
- We already vendor and patch `bevy_pbr`.

---

## 2. Targets

| Tier | Hardware | Target | Features |
|---|---|---|---|
| **Ultra** | RTX 3070+/RDNA3, M3 Max+ | 120 fps at 4K output | Cluster LOD + software raster, hardware ray-traced GI and reflections, DLSS/FSR3/MetalFX quality |
| **High** | M3 Pro, RTX 2060-3060, Steam Deck-class+ | 120 fps at 1440p output (60 fps floor) | Cluster LOD, probe GI (DDGI), full post |
| **Medium** | Older laptops, iGPUs | 60 fps | Discrete LOD + impostors, probe GI at low res, reduced post |
| **Mobile** | iPhone 15 Pro+, Snapdragon 8 Gen 2+ | 60 fps (30 fps battery saver) | Impostor-heavy, baked/probe GI, MetalFX/FSR, short draw distance at ground level, full Earth from the air |

A frame budget at **120 fps is 8.3 ms**. That only fits with a GPU-driven renderer, upscaling from ~50-67% internal resolution, and hard per-system budgets.

**Engine rule: every feature declares its cost per tier and has an off switch.** The perf gate (§13) enforces each tier's budget.

---

## 3. The big decision: Rust + wgpu vs full native

### What "native" means here
The game is already native code (Rust compiles to machine code). The real question is **who owns the GPU layer**, the RHI (render hardware interface).

| Option | GPU layer | Language |
|---|---|---|
| A. Rust + wgpu | wgpu (the same layer Bevy uses) | Rust |
| B. Rust + wgpu + native escape hatches | wgpu, plus direct Metal/Vulkan/DX12 for specific features through `wgpu::hal` interop | Rust |
| C. **Full native, in Rust** | Our own RHI with backends written directly against Metal (`objc2-metal`), Vulkan (`ash`) and D3D12 (`windows-rs`) | Rust |
| D. Full native, in C++ | Same as C, or an existing engine | C++ |

Rust stays the language in every serious option. Rust *is* native: crates such as `objc2-metal`, `ash` and `windows-rs` give direct API access with no overhead. Going to C++ would buy nothing except the option to use Unreal.

### What full native actually unlocks (the "proper optimizations")
These are the real optimizations a native backend can use and wgpu either can't express today or only partly can. Check each against the current wgpu release before deciding; wgpu moves quickly.

| Optimization | Why it matters for us | Metal | Vulkan / DX12 | wgpu today |
|---|---|---|---|---|
| **Mesh / object shaders** | Nanite-style cluster culling and LOD in one GPU pass, no index-buffer expansion | ✓ (Apple7+) | ✓ | Experimental |
| **Indirect command buffers** (GPU writes the draw list itself) | Thousands of draws for the cost of one, culled on the GPU. The key to 22k draws becoming "free" | ✓ ICB | ✓ (`DGC` / ExecuteIndirect) | `multi_draw_indirect` is emulated on Metal as a loop |
| **Full bindless** (argument buffers / descriptor heaps) | Every texture and material reachable from one bind, so material count doesn't cost draws | ✓ Tier 2 | ✓ | Partial (binding arrays, limits) |
| **Tile memory** (tile shaders, imageblocks, memoryless targets) | On Apple and phone GPUs, G-buffer, depth and MSAA can stay in on-chip memory and never touch RAM. **Huge for mobile bandwidth and battery** | ✓ | Vulkan subpasses / `VK_KHR_dynamic_rendering_local_read` | Transient attachments only, no tile shaders |
| **Async compute** (separate GPU queues) | Clouds, GI, culling and water overlap with the raster passes | ✓ | ✓ | Single queue |
| **Explicit memory** (heaps, aliasing, placement) | Fixed GPU memory pools for streamed tiles. No alloc/free hitches; transient targets share memory | ✓ | ✓ | Allocator managed by wgpu |
| **Sparse / tiled resources** | Virtual texturing for Earth-scale satellite and terrain textures | ✓ | ✓ | No |
| **Hardware ray tracing** | Ray-traced GI, reflections and shadows on Ultra | ✓ (M3+, A17 Pro+) | ✓ | Experimental ray query |
| **64-bit atomics** | Nanite-style software rasterizer (depth+id packed in 64 bits) | ✓ (Apple8+/M2+) | ✓ | Feature-gated, limited |
| **Pipeline binary archives / PSO caches** | Zero shader compile at runtime. Ships precompiled per device | ✓ | ✓ | Pipeline cache (Vulkan only) |
| **Vendor upscalers** (DLSS, FSR3, XeSS, MetalFX) | Your DLSS-style requirement | MetalFX | DLSS/FSR3/XeSS SDKs | Only through hal interop |
| **Precise GPU timing and debug markers** | Profiling per pass on every platform | ✓ | ✓ | Timestamp queries (not inside passes on Metal) |
| **Validation overhead** | wgpu validates and tracks every resource for safety (~CPU cost per draw/bind) | — | — | Always on |

### Costs of full native
- **Three backends to write and maintain:** Metal (Mac and iOS), Vulkan (Android, Linux, Steam Deck) and D3D12 (Windows). Plus synchronization, barriers, memory, swapchains, device loss and driver bugs, which wgpu handles today.
- **Shader cross-compilation:** one source language (WGSL or Slang) → MSL / SPIR-V / DXIL. Tools: `naga`, Slang, or `spirv-cross`.
- **No web build.** wgpu also targets WebGPU; full native gives that up unless we keep a wgpu backend.
- Rough effort: a usable RHI with one backend takes about 2-3 months for one experienced person. All three take about 6-9 months, before any rendering features.

### Recommendation: build our own RHI now; wgpu is backend #1, native Metal is backend #2
This gets the control of full native without betting the project on it before we have data.

1. **`ed_gpu` crate: our own RHI trait.** It is designed around *modern native concepts*, not around wgpu's API:
   - bindless resource tables;
   - GPU-driven indirect draws;
   - transient/memoryless targets;
   - queues (graphics + async compute);
   - explicit heaps for streaming pages;
   - pipeline precompilation;
   - the upscaler interface.
2. **Backend 1: `wgpu`.** It brings up every platform fast (Windows, Mac, Linux, Android, iOS, web). Features it lacks fall back (ICB → multi-draw loop, async compute → same queue, etc.).
3. **Backend 2: native Metal.** It is our dev platform and covers iPhone. It is written once profiling on the wgpu backend shows a ceiling (decision gate below). This is where tile memory, ICBs, MetalFX, ray tracing and binary archives come in.
4. **Backends 3 and 4: D3D12 and Vulkan,** only when Windows and Android ship. With D3D12, DLSS and FSR3 integrate natively. Until then wgpu covers those platforms.
5. Renderer code (`ed_render`) is written against `ed_gpu` only, **never** against wgpu or Metal directly. Swapping a backend is then a backend job, not a rewrite.

**Decision gate G1, end of M2.** Profile the same scene on the wgpu and Metal paths. Write the remaining backends natively if any of the following holds:
- wgpu CPU overhead per frame is > 1.5 ms;
- emulated indirect draws or missing tile memory cost > 2 ms on GPU;
- an Ultra-tier feature we need (mesh shaders, RT, vendor upscalers) can't go through hal interop.

Otherwise keep wgpu as the portable backend and native only where it pays.

---

## 4. Engine architecture: the persistent Earth at the centre

```
 ┌────────────────────── persistent universe (truth, f64 + frames) ──────────────────────┐
 │ ed_geo    ECEF/ENU, tiles, sun; + reference frames (galaxy/system/planet, §10)        │
 │ ed_data   OSM / DEM / weather / cache                                                 │
 │ ed_world  generators: cell -> TileBuild (terrain, placements, scatter, water)         │
 │ ed_store  world-delta DB: edits, damage, placed props, NPC/vehicle/mission state      │
 └──────────────────────────────────────┬─────────────────────────────────────────────────┘
                                        │ cells: deterministic base + deltas
 ┌────────────────────── ed_runtime (engine-free) ────────────────────────────────────────┐
 │ job system (work-stealing pool) · streaming scheduler (screen error, velocity          │
 │ prefetch) · residency (CPU cache -> GPU pages, LRU) · per-system frame budgets         │
 │ floating origin · fixed-tick sim (agents, vehicles, water, weather, time)              │
 │ net: replication of deltas + entity state (§9)                                          │
 └──────┬──────────────────┬───────────────────┬─────────────────┬──────────────────────────┘
        │                  │                   │                 │
 ┌──────▼──────┐   ┌───────▼───────┐   ┌───────▼──────┐   ┌──────▼──────────────────┐
 │ ed_render   │   │ ed_physics    │   │ ed_audio     │   │ app shell: winit window, │
 │ on ed_gpu   │   │ (Avian/Rapier │   │ (engine      │   │ input, UI, platform glue │
 │ (§5)        │   │ behind trait) │   │ audio + mix) │   │ (desktop / iOS / Android)│
 └──────┬──────┘   └───────────────┘   └──────────────┘   └──────────────────────────┘
 ┌──────▼──────────────────────────────────────┐
 │ ed_gpu RHI: wgpu │ Metal │ D3D12 │ Vulkan    │
 └─────────────────────────────────────────────┘
```

Rules:
1. **Tiles are data, not entities.** A tile apply is a copy into GPU pages plus instance-list appends: O(1) on the main thread. The ECS holds only things with behaviour.
2. **Truth is deterministic generation + deltas.** The procedural base is never stored or sent. Only changes are (save files and multiplayer alike).
3. **Hard frame budgets per system.** Work that doesn't fit waits for the next frame. Nothing big ever runs synchronously on the main thread.
4. **Pipelines are known up front** and precompiled (binary archives / PSO caches). Settings toggle uniforms and passes, never trigger compiles.
5. **Engine-free by default.** Everything except `ed_render`, `ed_gpu` and the app shell is unit-tested logic with no GPU dependency. The server runs the same crates headless.
6. **ECS:** keep `bevy_ecs` as a standalone crate (it is excellent and engine-agnostic) or move to `hecs`/`flecs`-style later. The ECS is not the bottleneck. Bevy's app, render and PBR layers are what we leave.

---

## 5. ed_render: GPU-driven, built for Earth

### 5.1 Geometry: Nanite-like
- **Cluster LOD.** Each mesh is built into 128-triangle clusters with a LOD DAG (cluster groups simplified with locked borders, so there are no cracks). Build on worker threads with **meshoptimizer** (`meshopt` crate: `build_meshlets`, `simplify`). Cache to disk per asset or tile.
- **GPU culling pipeline:** instance cull (frustum + two-pass Hi-Z occlusion) → cluster cull and LOD select by screen error → raster.
- **Raster path:**
  - mesh shaders (native) or compute-expanded indirect index buffers (wgpu) for big triangles;
  - a **compute software rasterizer** with 64-bit atomics for pixel-size triangles (Ultra/High);
  - discrete LOD plus impostors on Mobile/Medium.
- **Visibility buffer.** Geometry is written once (instance+cluster+triangle id). Materials are resolved in a compute/fullscreen pass per material bin. This removes the depth prepass and makes shading cost independent of geometry.
- **Impostors.** Octahedral impostors for trees and props; merged facade-atlas boxes for distant city blocks. They are the bottom of every LOD chain.
- **Terrain.** CDLOD / clipmap with height and normal textures. One draw, continuous LOD, the same height function as the physics `TerrainGrid`. Satellite and splat textures through **virtual texturing** (sparse native, an indirection-table fallback otherwise).
- **Streaming pools.** Fixed-size GPU pages for clusters, instances and textures. Allocation never happens per tile.

### 5.2 Lighting and grounding
| System | Approach | Tiers |
|---|---|---|
| Sun / sky / atmosphere | Hillaire LUT atmosphere (have it). Aerial perspective as a froxel volume | All |
| Shadows | **Virtual shadow maps** (page-based, cached; only dirty pages re-render) on Ultra/High. 2 cascades + cached far map on Medium/Mobile. Contact shadows for grounding | All |
| Diffuse GI | **DDGI-style probe grid** that follows the ground (our `probe_gi` grows into this): ray-traced probe updates (hardware RT where available, else SDF/heightfield + voxel tracing), with a moving update budget | All (density/rate per tier) |
| High-end GI | **ReSTIR GI / screen-space + world-space radiance cache** (Lumen-like), hardware RT | Ultra |
| Reflections | SSR + probe fallback. RT reflections on Ultra. Planar/SSR for water | Per tier |
| AO | GTAO (visibility-bitmask variant) at half res, temporally filtered | All but low mobile |
| Night lighting | Clustered/tiled lights for streetlights, windows and vehicles. Emissive window masks from building generation | All |
| Volumetrics | Clouds (have them) at ¼ res + temporal reprojection. Froxel fog for the god-ray/haze "grounding" | Per tier |

### 5.3 Post and image quality
- **Upscaler interface** (`ed_gpu::Upscaler`). Inputs: color, depth, motion vectors, jitter, exposure, reactive mask. Backends:
  - **DLSS** (NVIDIA);
  - **FSR 3** (all Windows/Android);
  - **XeSS** (Intel);
  - **MetalFX temporal** (Mac/iPhone);
  - our own **TAAU** fallback (portable, works on wgpu).

  Frame generation (DLSS FG / FSR FG / MetalFX frame interpolation) is an Ultra option for 120 fps output.
- **Motion vectors** for everything, including camera, skinned, wind-animated and floating-origin re-anchors (zero out the origin jump). They are required by the upscalers, TAA and motion blur.
- **Post chain** (compute, half-res where possible):
  - motion blur (per-object, McGuire tile-max);
  - depth of field (bokeh, for photo mode / cutscenes);
  - bloom (dual-filter);
  - auto exposure (histogram);
  - local tone mapping (AgX/filmic) + color grading LUT;
  - lens effects (flare, chromatic aberration, vignette, grain), subtle by default;
  - sharpening (RCAS/CAS).
- **Dynamic resolution** driven by GPU frame time keeps the tier's fps target. The upscaler hides it.

### 5.4 Frame budget (High tier, M3 Pro, 120 fps = 8.3 ms)
| Item | Budget |
|---|---|
| Culling + visibility buffer raster (incl. terrain) | 2.0 ms |
| Material resolve | 1.2 ms |
| Shadows (virtual, cached) | 0.8 ms |
| GI probes + AO | 1.0 ms (async compute) |
| Sky, clouds, volumetrics | 1.0 ms (partly async) |
| Upscale + post | 1.2 ms |
| UI | 0.3 ms |
| Headroom | 0.8 ms |

---

## 6. Simulation systems

- **Agents** (NPCs, animals, traffic). One framework (`PLAN-engine.md` §2).
  - Full sim within ~1 km of any player; simplified "statistical" sim per cell beyond that.
  - Navigation on OSM road and footpath graphs plus a navmesh near the player.
  - Animation: GPU skinning, crowd impostors at distance.
  - Behaviours are data (schedules, needs, routes).
- **Water.**
  - Open ocean: FFT spectrum (Tessendorf / JONSWAP), cascaded. Foam from Jacobian. Buoyancy from the same displacement on the CPU (readback or a CPU copy at low res).
  - Rivers, lakes and floods: GPU shallow-water simulation in a moving window around the player, seeded from OSM water and DEM flow.
  - Interaction: wakes, splashes, vehicles in water.
- **Weather.** Live Open-Meteo drives clouds, rain, wind, snow and wet surfaces. It is deterministic per time, so all players see the same weather.
- **Missions.** Data-driven (a script or graph DSL) on top of placements, agents and `ed_store`. A mission is a set of goals bound to real places (OSM POIs). Generated missions come from the world's data ("deliver to this real address").
- **Vehicles and flight.** `ed_flight` is already engine-free. Vehicles run on the physics trait.
- **Physics.**
  - A collision world near players only: height-field terrain, building boxes and props. Built on workers.
  - Fixed tick (60 Hz sim, rendered at 120 with interpolation).
  - Deterministic enough for prediction (§9).

---

## 7. Persistence (`ed_store`)
- **Key:** `(cell_id, object_id)`. `cell_id` is an S2 cell or our tile quadtree id. `object_id` is a stable hash of OSM id + generator salt, or a fresh UUID for player-placed objects.
- **Record:** an op over the procedural base: `Add`, `Remove`, `Transform`, `SetState`, `Damage`, ...
- **Storage:** an append-only log per cell, compacted to snapshots. Local: SQLite or an embedded KV (`redb`). Server: the same format in Postgres or a KV store.
- When a cell loads: generate the base, apply the deltas, then hand off to runtime residency.
- **Versioning.** Generator version is stamped per cell. If a generator changes, deltas re-anchor by object id. Removes and edits keyed to OSM ids survive regeneration.

---

## 8. Platforms

| Platform | GPU backend | Upscaler | Notes |
|---|---|---|---|
| macOS (dev) | Metal (native, later), wgpu now | MetalFX / TAAU | Primary profiling target |
| Windows | D3D12 (native, later), wgpu/Vulkan now | DLSS / FSR3 / XeSS | Biggest player base; DLSS needs D3D12 or Vulkan |
| Linux / Steam Deck | Vulkan | FSR3 | Via wgpu at first |
| iOS / iPadOS | Metal | MetalFX | Tile memory matters most here. Touch controls, thermal-aware dynamic resolution |
| Android | Vulkan | FSR / own TAAU | Wide driver variance: wgpu backend first |
| Web (optional) | WebGPU through wgpu | TAAU | Demo/viewer only; only possible if we keep the wgpu backend |

Shaders: one source language. **Decision:** stay on WGSL (naga cross-compiles to MSL, SPIR-V and HLSL) until a native backend needs features WGSL lacks. Then move to **Slang**, which targets all of them and has better generics and modules.

Mobile specifics:
- Streaming radius and cache size per device memory.
- Battery and thermal governors.
- Satellite imagery at lower mip.
- Smaller cluster budgets.
- Content is the same Earth.

---

## 9. Multiplayer
- **Authority.** An authoritative server per region shard. Positions in f64 on the server. Clients render relative to their own floating origin.
- **Interest management.** Clients subscribe to cells near them (radius by speed: walking → small, jet → large, orbit → coarse).
- **What syncs:**
  - world deltas (`ed_store` ops), reliably;
  - entity state (players, vehicles, NPCs near players), unreliably at 20-30 Hz with snapshot compression and delta encoding;
  - mission state.

  **The terrain and world are never sent.** Determinism generates them on every client.
- **Netcode.** Client-side prediction + server reconciliation for owned vehicles and players; interpolation for others. Rust references: `lightyear`, `renet`. Transport: QUIC (`quinn`), WebTransport for the web.
- **Determinism tests.** Generators must give bit-identical output on x86 and ARM, Windows, Mac and phones:
  - no platform-dependent hashing;
  - no float reductions that depend on thread order;
  - fixed-point or careful f64 in generators.

  CI runs a golden-hash test per reference tile on every platform.
- **Scale.** Shards by region with handoff at borders. A global persistent store. Space is its own shard space.

---

## 10. Planets and the universe
- **Reference frames** (the f64 limit is reached past roughly solar-system scale):
  1. `GalaxyPos { sector: IVec3 (i64), offset: DVec3 }`;
  2. star system: heliocentric f64;
  3. planet: body-fixed f64 (our ECEF);
  4. render: camera-relative f32.

  All physics and render work happens in the nearest frame. Transitions are explicit, as when the floating origin re-anchors.
- **Depth.** Reversed-Z float depth (already needed for Earth) plus logarithmic depth or multi-frustum for orbit-to-ground views.
- **Other planets reuse the Earth pipeline** with procedural "data" instead of OSM:
  - elevation from noise + erosion simulation;
  - biomes from climate rules;
  - atmosphere parameters per body (Hillaire LUT generalises: Mars, Titan);
  - cloud and weather parameters;
  - ocean of any liquid.

  `ed_space` already has the ephemeris and body catalog.
- **Beyond the solar system.** A procedural galaxy (seeded star catalog plus real nearby stars from Gaia/HYG data). Stars are points or impostors until approached. A system is generated on demand. Same determinism and same delta store, so a flag planted on a moon persists.
- **Travel.** `ed_flight` rocket model → orbital mechanics (patched conics first, n-body later) → time warp → "superpower" FTL as a game mechanic.

---

## 11. Research and open source to borrow from

| Area | Sources |
|---|---|
| Cluster LOD / virtual geometry | Karis, "Nanite" (SIGGRAPH 2021 Advances); meshoptimizer (clusterizer, simplifier); Bevy meshlet implementation (Rust reference); Zeux's "meshoptimizer cluster LOD" posts; AMD/Nvidia mesh shader samples |
| Visibility buffer | Burns & Hunt (2013); "Visibility Buffer Rendering with Material Graphs" (Filament/Wihlidal talks) |
| GI | Majercik et al., DDGI (JCGT 2019); ReSTIR GI (Ouyang 2021); Lumen talks (Wright, SIGGRAPH 2022); kajiya (Embark, Rust research renderer); "Radiance Cascades" (Sannikov) |
| Shadows | Virtual Shadow Maps (UE5 docs/talks); cached shadow maps |
| Sky / clouds / fog | Hillaire 2020 atmosphere (in use); Schneider, Horizon Zero Dawn clouds; Hillaire 2015 froxel volumetrics |
| Terrain | Strugar, CDLOD; Asirvatham & Hoppe geometry clipmaps; Cesium 3D Tiles; Outerra and MSFS streaming talks; virtual texturing (van Waveren, id Software) |
| Water | Tessendorf, "Simulating Ocean Water"; Crest ocean system (open source); GPU shallow water (Mei et al.) |
| AO / post | GTAO (Jimenez 2016) + visibility bitmask (Therrien 2023); McGuire motion blur; Jimenez "Next-gen post processing in CoD:AW"; AgX tonemapping |
| Upscalers | DLSS SDK, FSR 3 (open source), XeSS SDK, MetalFX docs |
| RHI design | wgpu/wgpu-hal source; The Forge; NVRHI (Nvidia); Sokol; "Designing a modern GPU API abstraction" talks; SDL3 GPU |
| Networking | `lightyear`, `renet`, `quinn`; Glenn Fiedler's articles; Overwatch GDC netcode talk |
| ECS / jobs | `bevy_ecs` standalone, `hecs`, `rayon`; Naughty Dog fiber job system talk |
| Whole-Earth references | Outerra, Microsoft Flight Simulator, Google Earth, Cesium |

---

## 12. Milestones

| M | What | Exit criterion |
|---|---|---|
| **M0** (now, in Bevy, 1-2 wk) | Tree impostors, far trees out of shadows, 2 cascades, env map on change, clouds ½ res, pipeline pre-warm, time-budgeted apply, shared terrain material | 45+ fps steady, no frame > 50 ms after load |
| **M1** (3-4 wk) | `ed_runtime`: jobs, residency, budgets, streaming scheduler out of `streaming.rs`. Tiles stop being entities | Tile apply < 1 ms; flat entity count |

**Status (Nov 4, `perf/m0-m1`, not yet measured on the Mac; see `docs/claude-handoff/2026-11-04-m0-m1.md`):**
- M0 done: 2 cascades; env map refreshed only on change (patched `bevy_pbr`, `EnvironmentMapRefresh`); shared terrain material (satellite texture pool + `MeshTag`); staged, budgeted tile apply. Far leaf cards were already out of shadows.
- M0 open: pipeline pre-warm (cause of the settings stall unconfirmed; AO/TAA toggles change every material's pipeline key), tree impostors (conflicts with PLAN-perf "no impostors": needs a decision), clouds ½ res (deferred: measured no gain while vertex bound).
- M1 started: `ed_runtime` crate with IO and CPU job pools (priority, re-rank, cancel), frame budgets, residency LRU, height-aware view test, linear-time hole-free retire. `streaming.rs` runs on it. Open: tiles are still entity trees (the "tiles stop being entities" half of M1).
| **M2** (6-8 wk) | `ed_gpu` RHI (wgpu backend) + `ed_render` v1: terrain clipmap, GPU-culled instancing, impostors, visibility buffer, cached shadows, own TAAU. Bevy keeps app/UI/player only | **60 fps steady** on the 6 reference scenes, High tier |
| **G1** | **Decision gate: native backends** (§3) | Profile data written up |
| **M3** (8-12 wk) | Native Metal backend (if G1 says so): ICBs, tile memory, MetalFX, binary archives. Cluster LOD (Nanite-like) + software raster | **120 fps** High tier on M3 Pro, 1440p output |
| **M4** (8 wk) | Lighting: DDGI probes, virtual shadow maps, GTAO, froxel fog, motion blur, full post chain, night lights | Lookdev boards pass art review at 120 fps |
| **M5** (ongoing) | App shell off Bevy (winit + own loop + `bevy_ecs` standalone). `ed_store` persistence, agents framework, missions v1 | A world edit survives a restart |
| **M6** | Water: FFT ocean + local shallow-water sim; buoyancy | Boats/planes on water at budget |
| **M7** | Windows (D3D12 native or wgpu) + DLSS/FSR3; perf gate on Windows | 120 fps High on RTX 3060 |
| **M8** | Multiplayer: server, interest management, delta replication, prediction | 2+ players see the same edits and vehicles across a city |
| **M9** | Mobile: iOS (Metal backend reuse), then Android (Vulkan); touch UI, thermal dynamic resolution | 60 fps on iPhone 15 Pro |
| **M10** | Space: reference frames, orbital mechanics, procedural planets on the Earth pipeline, galaxy | Fly Earth → Moon → Mars surface with no load screen |

---

## 13. Perf discipline (from now on)
- `tools/perf_gate.sh`: 6 reference routes (city, forest, mountains, ocean, low flight, rocket ascent) per tier. Fails if p99 > budget × 1.2 or any frame > 2× budget after load.
- GPU timestamps per pass in the HUD on every backend (Metal counters, D3D12/Vulkan timestamps). Also the Metal System Trace recipe:
  ```
  xcrun xctrace record --template 'Metal System Trace' --attach PID --time-limit 3s --output X.trace
  ```
  Export the `metal-gpu-intervals` table and sum time per encoder label.
- Every new feature lands with its cost per tier in the PR description.

## 14. What carries over unchanged
- All engine-free crates and their tests: `ed_geo`, `ed_data`, `ed_world`, `ed_flight`, `ed_space`, `ed_engine_audio` (~16k lines, deterministic generation).
- WGSL shaders: terrain, foliage, ground scatter, clouds, FSR EASU, probe GI. They port into `ed_render`.
- Placements and the asset library: they already are the "one buffer per asset class" model.

## 15. Risks
- **Scope.** This is a multi-year engine. Mitigation: every milestone ships a playable, faster game, and the old path stays as a fallback until the new one beats it on all reference scenes.
- **Native backend maintenance.** Mitigation: the `ed_gpu` trait lets us keep wgpu wherever native doesn't pay. Write native only behind the G1 gate.
- **Nanite-style rendering on Apple and mobile.** Mitigation: tiered fallback (discrete LOD + impostors) is built first and always kept.
- **Determinism across platforms.** Mitigation: golden-hash CI from M1 onward.
- **Vendor SDK licensing** (DLSS, XeSS). Mitigation: TAAU and FSR (open source) are always available.
