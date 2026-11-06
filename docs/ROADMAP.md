# Roadmap

Rewritten 2026-11-05. What's done, what's next, in what order, and why. Recent changes and open checks: `CHANGES.md`. Open bugs: `BUGS.md`. Plans: `plans/` (engine, assets and agents, buildings, world sim, space visuals, shadows, flora, sound, voxel proxies and navara). How it works: `ARCHITECTURE.md`.

## Where we are

A playable game: a good-looking real-Earth world with a flyable robot courier (rocket boots, skydive, parachute, wingsuit), teleport by place name, HUD, procedural trees/buildings/ground, clouds and atmosphere. What holds it back:

- **Performance.** Below 30 fps on an M3 Pro (baseline measured at 33.5 fps in an Ohio suburb at 67% render scale; worse in cities). M0 and M1 are built; the headless harness (`tools/perf_headless.sh`) shows real gains (shadow proxies took the cruise scene from 17.4 to 34.9 fps) but the M0 exit (45+ fps) is not met and the windowed Mac pass has not been done. The native Metal renderer *does* have per-pass GPU timers now (`ed_metal::timers`, printed by `RUST_LOG=info` runs), and the first measurements are in `plans/voxel-proxy-and-navara.md`: in a settled city (Shibuya, street level, 2144x1206) the frame is 20.8 ms of GPU - world pass 71%, ocean 25%, shadows 20% - with 15.6 M triangles submitted in 474 per-tile draws. So the GPU is the limit, and it is fed per-tile merged geometry plus an ocean FFT that runs every frame for water the camera cannot see (~4.8 ms, 23% of the frame, measured). The next stops are the cheap wins (skip the ocean FFT when the sea is not visible, shadow cache survives streaming) and then GPU-driven per-cluster culling.
- **City performance is BAD right now (2026-11-06).** Do not read the older "35-44 fps after the shadow fix" line and assume a city is fine. Measured today on the native Metal renderer in Shibuya at street level (2144x1206, headless): **27-32 fps settled, `gpu_wait` 20.8 ms, 15.6 M triangles submitted in 474 per-tile draws**; while the city streams it is far worse (`gpu_wait` 93 -> 207 ms, 1639 shadow draws, 0 cascades cached - every arriving tile invalidates all six cascades). The LOD fix stopped distant towers shredding into sails but did not cut the triangle count (23.4 M vs 26.9 M with the LOD off): in a city the triangles *are* the buildings, and `meshopt` can only remove ~17 % of them inside a sane error budget. So: below the 30 fps floor in a city, and unplayable while it loads. The four fixes in order are in `docs/CHANGES.md` (skip the ocean FFT when the sea is not visible; make the shadow cache survive streaming; a per-building far LOD; GPU-driven per-cluster culling in M2). The 6 reference scenes still need re-measuring on the same basis.
- **Visual glitches.** LOD flashes, impostor flicker, shadow cascades, road/guardrail/path geometry, atmosphere/satellite/cloud altitude transitions (`BUGS.md`).
- **Missing:** vehicles, missions, multiplayer, space beyond a prototype, other planets.

## Decisions (2026-11-05)

1. **Native Metal is the macOS backend.** G1 is no longer a go/no-go; it becomes a scope check (see Stage B).
2. **A Nanite-like geometry system is the goal** (cluster LOD + GPU culling + visibility buffer + software raster for small triangles). Bevy's meshlets measured ~60% slower on the M3 Pro, so we build our own on `ed_gpu`/`ed_render`, not on Bevy.
3. **Windows must support DLSS/FSR3 and hardware ray tracing (RTX).** `ed_gpu` is designed for it from the start: an upscaler interface, an acceleration-structure interface, a bindless resource model. A D3D12 backend is written when Stage H starts.
4. **Use the GPU fully.** Measure it first (Stage A), then fix what the measurements show. Better shaders and materials follow from a renderer that is not starved.
5. **Performance and looks come before vehicles.** Vehicles are a planned step (Stage F), but the game must run well and look good first.
6. **Multiplayer is designed in early.** The world must match for every player. Stage A lays the determinism foundation; Stage C ships ghost multiplayer.

## Stage order

| Stage | Name | Contents | Exit criteria |
|---|---|---|---|
| **A** | Stabilize and measure | P1 bugs; Mac test pass; GPU pass timers and utilization capture; `perf_gate.sh`; determinism golden-hash CI; pinned data versions | We know what limits the frame. Reference scenes glitch-free. |
| **B** | Fast | Finish M1 (GPU-resident tiles); M2 (`ed_gpu` + `ed_render` v1); G1 scope check | 60 fps steady on the 6 reference scenes (M3 Pro) |
| **C** | Together | Ghost multiplayer; `ed_store` world deltas; procedural climate synced by (seed, UTC) | Two machines see the same world and each other |
| **D** | Native and Nanite-like | Native Metal backend; cluster LOD + software raster; material and shader system v2 | 120 fps High tier on M3 Pro |
| **E** | Lighting | GTAO, froxel fog, motion blur, night lights (cheap first); virtual shadow maps; DDGI | Looks cinematic within budget; every feature has a cost per tier and an off switch |
| **F** | Play | Vehicles; water; traffic; missions; NPCs and animals | Full delivery loop with vehicles, on one machine and online |
| **G** | Space | Reference frames; orbit; Earth from orbit; Moon; Mars; procedural planets; galaxy | Rocket to the Moon, walk on it, keep your edits |
| **H** | Platforms | Windows (D3D12, DLSS/FSR3, RTX); then mobile (iOS, then Android) | Ships on other hardware with the perf gate passing |

Stages A to C are strictly ordered by dependency. D to H can overlap with different sessions on different files, but the exit criteria above are what "done" means.

---

## Stage A: Stabilize and measure (now)

Goal: stop guessing. Find the real bottleneck and make the existing game solid before any big rewrite.

1. **Open bugs** (`BUGS.md`, B1-B12). P1 first: roads on the ground and marked (B5, B6), guardrails (B7), atmosphere and satellite transitions (B9, B10), clouds at speed (B11), impostor flicker (B2), shadow cascades (B3).
2. **Mac test pass for M0 + M1** (`CHANGES.md`, about 20 minutes): fps, hitches, forests and impostors, building explode, colliders. Everything since `c42ed5e` is unmeasured on Metal.
2b. **Tile-load hitch** (white tile, ~1 s freeze when a tile arrives, smooth otherwise). Investigation and ranked hypotheses in `plans/tile-load-hitch.md`; the leading cause (measured on CPU, not yet on the Mac) is native mesh upload doing cluster + proxy simplification on the main thread, ~250-700 ms per dense tile. Confirm with M1-M5 there, then fix F1-F4. Do this before the GPU measurement table, since it pollutes every p99. **Partly done (2026-11-06):** the simplification moved to a worker, the decimation is hash-free, and preps are gated (`EW_PREP_JOBS`, default 2). In Shibuya the load phase went from 1-6 fps to 14-34 fps. The remaining cost is the main-thread apply (mesh merge + collider) and the main pass' triangle count.
2c. **BeamNG.Earth lessons** (`plans/beamng-earth-lessons.md`). **Roads v2 is implemented** (`plans/roads.md`: world-space network, solver, decks, tunnels, junctions, medians, nav graph, Overture buildings, garages) and waits for the Mac check; still open from that plan: imagery providers, canopy-height trees, retaining walls, inferred bridges, traffic drivers.
3. **GPU measurement (the next stop).** Tools that already exist: `tools/perf_headless.sh` (offscreen, repeatable scenes), `EW_BISECT=hide:cat,noshadow:cat` (cost by category), `EW_GPU_WAIT=1`, `EW_POST=...`, `EW_TILE_STATS=1`, F3/I stats, `--perf` CSV, and the pause menu's Debug tab (live probes, shadow views, renderer toggles and diagnostics). Steps:
   1. **Baseline table.** Run the headless harness on the 6 reference scenes and record fps, p99 and worst frame in `plans/engine.md` (one table, dated). This is the number every later change is judged against.
   2. **Per-pass GPU timers.** Done as a tool: Bevy's own pass timers are empty on Metal (wgpu has no in-encoder timestamps there), so `tools/gpu_passes.sh` records a Metal System Trace of a headless run and sums busy time per labelled encoder (fragment / vertex / compute). In-game F3 timers come with `ed_gpu::PassTimer` in M2 slice S1 (`plans/m2.md`).
   3. **Test the hypotheses** and write down the verdict on each: vertex bound (triangles submitted vs visible), draw/dispatch count, shadow pass cost, post stack (SSAO, TAA, bloom), clouds and sky, fill rate at 2144x1206, CPU vs GPU frame time (`EW_GPU_WAIT`).
   4. **Rank the findings** into M2's priority list. Anything that is a cheap fix on the current renderer (pipeline pre-warm, explode hitches, residual shadow cost) lands first; everything else becomes an M2 requirement.
   4c. **City LOD fixed (2026-11-06):** the vertex-clustering LOD proxies were twisting flat walls and turning distant towers into sails; the renderer now uses `meshopt` levels (`ed_gpu::cluster::simplify_levels`) and the LOD view is pixel-identical to the full geometry. `tools/lod_sheet.py` + the two dump examples render a real tile's LOD levels so the technique can be checked by eye. **Still open:** a city needs a *per building* far LOD (merged footprints, then boxes) to cut the triangle count further - see `docs/plans/far-buildings-and-nanite.md`.
   4b. **First city measurement done, and the city is BAD right now (2026-11-06, Shibuya, native renderer, street level):** steady state 24-25 fps -> 35-44 fps after the shadow-cascade reuse and the prep fix; the crash in a city is MetalFX's temporal scaler running an ANE/MPSGraph executable that aborts ~150 s after creation (auto-exposure is off by default in the macOS 27 SDK; the effect is now recycled every 90 s, which ran 240 s clean). But the city is still **below the 30 fps floor (26-27 fps settled at full res, GPU-bound: world 26 ms, shadow cascades ~16 ms, resolve 5.6 ms, water 4 ms)** and unusable while it streams: load-to-steady is ~45-80 s because a city uploads GBs of building vertex data, and fast travel freezes ~9.5 s then runs 1-4 fps for ~40 s. The upload queue re-request bug (9,000 pending preps) is fixed; per-frame uploads are budgeted. Next: per-building far LOD + vertex compression (48 B -> 28 B/vertex) + a mesh arena, then re-run the 6 reference scenes.
4. **M0 leftovers:** pipeline pre-warm (confirm the AO/TAA cause with the test pass first). Clouds ½ res done (`cloud_half.rs`, -2.3 ms).
5. **Perf gate script** (`tools/perf_gate.sh`, built on `perf_headless.sh`): 6 reference routes (city, forest, mountains, ocean, low flight, rocket ascent). Fails if p99 exceeds budget by 20% or any frame is over 2x budget after load. Needed before M2 so improvements are provable.
6. **Multiplayer foundations** (small now, expensive later):
   - Golden-hash CI: each reference tile must generate bit-identical output (no platform-dependent hashing, no thread-order-dependent float reductions, f64 or fixed-point in generators).
   - Pin data versions: stamp the tile cache with the OSM / elevation / land-cover snapshot so every client builds the same world. Live data revisions are a silent source of mismatch.
   - Separate simulation state from render state in `ed_game` (fixed timestep for sim, interpolated render).
7. **Feature items that already work and just need a review:** art direction round 2 (lookdev boards).

## Stage B: Fast

**Plan:** `docs/plans/m2.md` has the design rules (what keeps the Metal switch a backend swap), the slices S0-S7 with exit criteria, and when the native Metal spike starts (right after S1, not at the end of M2).

Goal: 60 fps steady on an M3 Pro for the world as it looks today. Engine design: `plans/engine.md`.

1. **Finish M1.** Fully GPU-resident tiles (no per-tile entities) is the part M1 deferred. Without it M1 is not done.
2. **M2: `ed_gpu` + `ed_render` v1.**
   - `ed_gpu` is the render hardware interface: bindless tables, GPU-driven indirect draws, transient targets, async compute, explicit heaps for streaming pages, pipeline precompilation, an upscaler interface, an acceleration-structure interface. First backend: wgpu, so work can start on top of it.
   - `ed_render` v1: terrain clipmap, GPU-culled instancing, visibility buffer, cached shadow maps, own TAA upscaler.
   - Renderer code is written against `ed_gpu` only, never against wgpu or Metal directly.
3. **Phase 2 items that M2 delivers:** instanced building modules (windows, balconies, AC units), CDLOD geomorphing for terrain, crossfades for building detail.
4. **Exit:** 60 fps steady on all 6 reference scenes, no frame over 50 ms after load.
5. **G1 scope check** (end of M2). Native Metal is already decided. G1 now answers *how much* of the renderer goes native first: profile wgpu vs native Metal on the same scene and prioritize what the numbers show (CPU overhead above 1.5 ms per frame, emulated indirect draws or missing tile memory costing over 2 ms, features that cannot go through wgpu: mesh shaders, ray tracing, MetalFX).

## Stage C: Together

Goal: players share the same world. Design: `plans/engine.md` section 9.

- **Principle:** terrain, buildings, trees and weather are *never sent*. Every client generates them from the same inputs (world seed, pinned data version, UTC time). Only deltas (edits) and entity state (players, vehicles, NPCs) travel.
- **Ghost multiplayer first:** a thin relay sends player position, pose and boot state at 20-30 Hz; each client generates the world. The test: two clients at the same coordinates see the same buildings, trees, clouds and sun, and see each other.
- **`ed_store`:** persistent world deltas (save files and multiplayer share the format). Reliable delivery.
- **Procedural climate** (`ed_climate` milestones 1-3, `plans/world-sim.md`): weather is a pure function of (seed, UTC), replacing Open-Meteo. Deterministic spin-up and fast-forward from checkpoints so all clients reach the same state.
- **Netcode:** authoritative server per region shard, interest management by speed (walking small, jet large, orbit coarse), prediction and reconciliation for owned players and vehicles, interpolation for others. Candidates: `lightyear` or `renet` over QUIC (`quinn`).
- **Physics note:** Avian is not deterministic across machines. The server owns vehicles and the rocket; clients render and predict them.

## Stage D: Native and Nanite-like

Goal: use the GPU fully on Metal, with a geometry system that scales to dense cities. Target: 120 fps High tier on M3 Pro at 1440p output.

1. **Native Metal backend** for `ed_gpu`: indirect command buffers, tile memory, binary archives, argument buffers, MetalFX upscaling, hardware ray tracing on supported GPUs.
2. **Cluster LOD + software raster** (Nanite-like): offline/online cluster hierarchy per mesh, GPU culling and selection, visibility buffer, 64-bit atomic software rasterizer for small triangles (Apple8+).
3. **Material and shader system v2.** This is where "better shaders and materials" lands:
   - material graph data model (data-driven, not hand-written pipelines);
   - visibility-buffer material resolve (one pass shades each pixel once);
   - layered materials for terrain and buildings, procedural detail at the shading stage;
   - hot reload for shaders and material definitions.
4. **Phase 2 work that depends on this:** building interiors streaming, bridges and tunnels with real decks, landmarks from OSM tags.

## Stage E: Lighting

Goal: make it look great without breaking the frame budget. **Engine rule: every feature declares its cost per tier and has an off switch.** The perf gate enforces each tier's budget.

1. Cheap, high-value first: GTAO, froxel fog, motion blur, night lights (lit windows, street lights, NASA city lights from orbit).
2. Virtual shadow maps (replaces the cascade work in `plans/shadows.md` for near range; far-range horizon maps stay).
3. DDGI global illumination, with hardware ray tracing used where available.
4. Phase 1.6 weather effects land here: rain and snow particles, wet roads, fog from visibility; later local storms (hurricane, tornado, sandstorm) per `plans/world-sim.md`.

## Stage F: Play

Goal: a game loop worth playing, built on a renderer that holds frame rate.

1. **Vehicles** (Phase 4): car (raycast suspension, tire model, torque curve, gearbox), engine audio (done: `ed_engine_audio`; next Doppler, rocket generator, per-car gearing), plane (per-surface lift/drag, runways from OSM aeroways), helicopter, rocket ship (launch from real sites).
2. **Water:** FFT ocean and local shallow water, buoyancy, rivers that flow downhill, waves, shoreline foam (boats come with this).
3. **Traffic:** AI cars on road centerlines (already exported).
4. **Missions:** delivery planner (pick up, multi-leg route, timer, scoring), missions v1 online.
5. **NPCs and animals** by biome; photo mode and postcards; character customization, save/load, settings, controller support.

## Stage G: Space

Goal: rocket to the Moon and beyond, with the same world rules (determinism plus deltas).

1. Reference frames (galaxy, system, planet) and orbital mechanics.
2. Earth from orbit: low-zoom globe tiles, atmosphere from space, cloud layer from `ed_climate` (`plans/space-visuals.md`).
3. The Moon: procedural cratered terrain, then NASA LOLA elevation; land and walk. Mars (MOLA), then the rest, with HUD target rings and approach guidance.
4. Starfield: procedural sky done (`starfield.rs`); next a real Hipparcos/Yale catalog.
5. Procedural planets and a seeded galaxy with real nearby stars (Gaia/HYG). Systems are generated on demand; a flag planted on a moon persists via `ed_store`.
6. Time compression on long coasts. The Moon and Mars work on the current f64 positions; only galaxy scale needs the new reference frames.

## Stage H: Platforms

Goal: ship beyond the dev Mac.

1. **Windows:** D3D12 backend (native), DLSS and FSR3 through the upscaler interface, hardware ray tracing (RTX) through the acceleration-structure interface, perf gate on Windows. Target: 120 fps High on RTX 3060. Dev/test on Windows through the existing Bevy/wgpu path is already in place (CI on macOS + Windows, `docs/WINDOWS.md` with test batteries and an agent checklist, 2026-11-06); the native backend above is still this stage's job.
2. **Mobile:** iOS first (shares the Metal backend; tile memory is the big win), then Android (Vulkan backend). Low-power tier with its own budget.
3. macOS app bundle and signing.

---

## Done: Phase 0 Foundation

- [x] Workspace: `ed_geo`, `ed_data`, `ed_world`, `ed_flight`, `ed_space`, `ed_game`
- [x] WGS84/ECEF/ENU math, slippy tiles, real sun position (tested)
- [x] Data: Mapterhorn + AWS elevation, OpenFreeMap vector tiles (hand-written MVT decoder), Open-Meteo weather (dev fallback), geocoding, disk cache
- [x] World gen: terrain, land cover raster, biomes, 17 architectural regions, street kits, roads, street props, trees, rocks, inland water
- [x] Quadtree LOD, floating origin, tile streaming with hole-free swaps
- [x] Filmic pipeline: HDR, atmosphere, atmosphere env-map lighting, bloom, SSAO, TAA
- [x] Moon + planets at true positions; delivery mission planner over real airports/spaceports
- [x] Lookdev boards for art review

## Done: Phase 1 Feel (mostly)

- [x] Art direction round 1 review; Blender Filmic picked
- [x] Robot courier v2 (connected body, CRT TV head, backpack as cosmetic)
- [x] Buildings v2: modeled facades, party walls, doors/stoops/shopfronts, balconies, shutters, AC units, fire escapes, cornices, roof forms, house lots; per-region variants; chunked detail streaming
- [x] Trees v2: branching trees, leaf cards, wind sway, more species
- [x] Ground v2: splat terrain material, procedural textures, near-camera grass/rocks/moss
- [x] Clouds v2: volumetric, translucent (live weather for now; switch to `WeatherSample` from `ed_climate`)
- [x] Teleport: type a place name or pick on the satellite globe map; drop in from altitude and skydive down; every drop-in lands in the chosen local solar hour (3 pm by default, changed in the pause menu)
- [x] Rocket boots, skydive, parachute, wingsuit, Mach 1 boom, camera shake (left: vapor cone, speed lines, wind from `ed_climate`)
- [x] HUD: compass strip with waypoint pins, mode pill, minimap from vector tiles, target rings
- [x] M0 quick wins in Bevy (built, unmeasured), M1 `ed_runtime` and tiles as data (built, unmeasured, GPU-resident tiles left)
- [ ] Art direction round 2 review

## Feature backlog by area (pull into stages above)

- **World fidelity (Phase 2):** landmarks and special buildings from OSM tags (castles, churches, mosques, temples, stadiums, skyscraper crowns); building interiors (procedural floor plans); bridges and tunnels with real decks; road elevation smoothing; ESA WorldCover and Macrostrat geology landed 2026-11-04 (unverified on Mac); procedural flora (`plans/procedural-flora.md`); skateparks and ski resorts from OSM; geology-tinted sand/scree textures, canopy height, SoilGrids.
- **Engineering:** CI with `cargo test --workspace` plus lookdev snapshot diff; style/kit tables to `assets/styles/*.ron` with hot reload; profiling HUD (partly done: F3/I stats, `--perf` CSV; left: GPU pass timings, memory); tile builder IO pool and cancellation (done, `ed_runtime::jobs`).

## Old milestone names

`plans/engine.md` still uses M0-M10. Mapping:

| Old | Now |
|---|---|
| M0, M1 | Stage A and B (finish M1 in B) |
| M2, G1 | Stage B |
| M3 | Stage D |
| M4 | Stage E |
| M5 (app shell, `ed_store`, agents, missions v1) | `ed_store` in C; missions v1 in F; app shell off Bevy as part of D |
| M6 water | Stage F |
| M7 Windows + DLSS/FSR3 | Stage H |
| M8 multiplayer | Foundations in A, playable in C |
| M9 mobile | Stage H |
| M10 space | Stage G |
