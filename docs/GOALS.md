# User goals (compiled)

Compiled from the user's questions in this session plus the standing project goals in `ROADMAP.md`, `ARCHITECTURE.md`, and `plans/space-visuals.md`. This is the running "what I want the game to be" list. Keep it current: when a goal lands, move it to "Done" and add a `CHANGES.md` entry.

Last updated: 2026-11-08 (session).

---

## 1. Core fantasy: seamless sphere Earth, surface to space to other worlds

**Goal:** One continuous world. Walk on the real Earth's surface, launch into space, orbit, fly to another planet (Moon, Mars, etc.), and everything — clouds, terrain, shadows, atmosphere — is there and consistent the whole way. No "switch" between a ground game and a space game.

Status: **Partially built.** Foundation is right; the altitude transition layer is not.

Sub-goals:
- **Ground layer (done):** real terrain from Mapterhorn/AWS elevation, OSM buildings/roads/water, biomes, trees, weather, atmosphere, clouds, sun/moon.
- **Floating origin / f64 ECEF truth (done):** lets one engine cover pebble to planet without f32 jitter near the camera.
- **Orbit view of Earth (partial):** `globe.rs` renders a unit sphere with EOX Sentinel-2 quadtree tiles + `OceanGlobe` above 30 km; `globe_atmo.wesl` ray-marches an atmosphere shell. Not unified with the ground view yet.
- **Seamless climb (NOT built):** crossfade ground atmosphere → orbital atmosphere (30–100 km), terrain → satellite sphere above ~200 km, volumetric clouds → 2D cloud-cover shell, no hard switch. Planned in `plans/space-visuals.md` ("Seamless climb" goal 1).
- **One look (NOT built):** map globe and in-game view from orbit must match (same atmosphere colors, clouds, stars). Currently two renderers that "share almost nothing."
- **Clouds from orbit (NOT built):** 2D cloud-cover texture on an ~8 km shell from `ed_climate`, feeding both globes; volumetric raymarch takes over near the camera seeded from the same field.
- **City lights from orbit (NOT built):** NASA Black Marble (VIIRS) tiles on night side, masked by terminator, dimmed under clouds.
- **Moon as explorable world (partial):** `ed_space` gives true positions; procedural cratered terrain via a `TerrainGrid`-like provider on a 1737 km sphere, then NASA LOLA. Floating origin must re-anchor to the Moon's frame (not done).
- **Mars and other planets (planned, Stage G):** procedural terrain then MOLA elevation.
- **Starfield (partial):** procedural `starfield.rs`; real Hipparcos/Yale catalog planned.
- **Procedural planets and seeded galaxy (planned, Stage G).**

Related docs: `plans/space-visuals.md`, `plans/world-sim.md`, `ROADMAP.md` Stage G.

---

## 2. Terrain at altitude must not disappear / must stay consistent

**Goal:** No "terrain vanishes when I go high." The ground should transition smoothly into the globe representation rather than popping out.

Status: **Known gap.** `lod::select` picks coarse roots (zoom 2–6) at altitude and the space plan replaces ground tiles with the satellite sphere above ~200 km, but the swap is a hard switch with no crossfade, and the two renderers don't share data.

Sub-goals / fixes:
- Crossfade terrain tiles → satellite-tile sphere over an altitude band (not a hard cutoff).
- Share tile cache between map globe and in-game far Earth (`GlobeTiles` resource) instead of separate caches.
- Keep a horizon ring: hide terrain beyond horizon, show globe beyond.
- Terrain LOD must not render ground-level triangles from orbital distance (precision + performance). Use the globe representation there.

Related docs: `plans/space-visuals.md` item 4; `crates/ed_game/src/globe.rs`, `sky.rs` (`OceanGlobe`).

---

## 3. Floating-point precision at high altitude

**Goal:** No visible precision degradation when flying high / in space.

Status: **Architecture correct, implementation incomplete.** f64 ECEF truth + f32 local ENU + floating origin (>8 km rebase) is the right and standard approach. Degradation happens because ground mesh geometry is still being rendered (in a frame where it's hundreds of km away) and Avian colliders sync across rebases.

Sub-goals / fixes:
- Stop rendering ground tiles at altitude; use the globe representation (fixes the visible precision problem at its source).
- Ensure orbital rendering uses f64-derived transforms and does not store large offsets in f32.
- Terrain colliders: keep them near the player only; use `TerrainGrid` height probes for ground instead of trimesh colliders at distance (eliminates rebase hitches). See `nanite-roadmap.md` recommendation #3.
- For Moon/Mars, re-anchor the floating origin to that body's frame.

Related docs: `ARCHITECTURE.md` (coordinate systems); `nanite-roadmap.md` sections 5–6.

---

## 4. Vehicle persistence: leave a car, go to space, come back

**Goal:** If I park a vehicle on the ground and fly to space, the vehicle stays where I left it and I can return to it working.

Status: **Not built.** Currently vehicles are live Avian bodies tied to loaded tiles; flying away unloads tiles and the body/colliders go with them.

Approach (decided): **"vessel on rails" / parked state**, not live physics from orbit.
- When a vehicle is far from the active camera (or its tile unloads), it stops being a live Avian body and becomes a **parked data record**: transform, velocity, fuel, damage, etc. stored as **f64 ECEF** state (`GeoAnchored`-style), no collider, no tire raycasts.
- On return to range, rehydrate into a live body at that location and resume physics.
- Pattern already exists in codebase: `near_chunks` (rocks as data, entities only within 500 m), `TreeChunks` (entities only near camera).
- For multiplayer: only state deltas travel; the body doesn't exist on remote machines until someone is near it. (Aligns with "only world deltas and entity state are sent.")
- **Do not** attempt full Avian physics for ground vehicles from orbit: precision (f32 at planetary distance), missing colliders (tiles unloaded), rebase churn, and cost all rule it out. Avian isn't deterministic across machines anyway.

Future (separate subsystem): **simulation LOD for autonomous traffic** — distant vehicles run a simplified kinematic tier (no Avian, no colliders, f64, lower tick rate) and promote to full bodies near a player. Build on `ed_runtime`'s scheduler, not Bevy physics directly.

Related docs: `ARCHITECTURE.md` (physics); `ROADMAP.md` Stage C (multiplayer); `plans/multiplayer.md`.

---

## 5. Cubesphere terrain chunks (explored, not adopted)

**Goal (question):** Would cubesphere quadtree terrain chunks be better?

Decision: **No for ground terrain; optional polish for the globe view only.**
- Ground data sources (Mapterhorn, OpenFreeMap, WorldCover) ship slippy Web-Mercator tiles; cubesphere would require reprojecting everything and break the cache/CDN/`TileId`/`lod::select` pipeline.
- The globe is already mathematically spherical (f64 ECEF); the visible region is a flat tangent plane via floating origin. Cubesphere's benefits (uniform triangles, no poles) only matter for whole-planet rendering, which is handled by `globe.rs` (unit sphere + EOX tiles).
- Planned terrain LOD is a **geometry clipmap** (constant vertex count, geomorph in vertex shader) + later **Nanite cluster LOD** on `ed_gpu`/`ed_render` — not a different tiling scheme.
- Cubesphere could improve the *orbital globe* renderer (avoids Web-Mercator pole distortion) but is optional polish, not a rewrite.

Related docs: `plans/nanite-roadmap.md` (Option C); `plans/space-visuals.md`.

---

## 6. Raymarched clouds (already built; open work is orbital unification)

**Goal:** Clouds visible and consistent from ground to orbit.

Status: **Ground raymarch done and tuned.** `clouds.wesl` raymarches a spherical cloud shell driven by weather data. Native Metal path runs it half-res with depth-aware upsample (27.0 → 24.7 ms). Switches: `EW_CLOUD_FULL=1`, `EW_WEATHER`.

Open work (from `plans/space-visuals.md` item 5):
- 2D cloud-cover texture on an ~8 km shell for the orbital view, generated from `ed_climate`.
- Same texture feeds map globe + title globe; volumetric raymarch takes over near camera seeded from the same field so they line up.
- Moonlit clouds on night side.

Related docs: `crates/ed_game/src/clouds.wesl`, `crates/ed_metal/src/clouds.rs`, `plans/space-visuals.md`.

---

## 7. Rendering end state: native Metal + Nanite-like geometry

**Goal:** 60 fps steady (Stage B), 120 fps High tier (Stage D) on M3 Pro.

Status: **In progress.** Native Metal renderer is default on macOS (`EW_NATIVE=0` to disable). Bevy/wgpu is the short-term path.

Key decisions:
- New renderer code targets `ed_gpu`/`ed_render` only, never wgpu or Metal directly.
- Bevy meshlets measured ~60% slower; not the path.
- Windows: D3D12 with DLSS/FSR3 and RTX ray tracing (Stage H).
- Every feature declares cost per tier and has an off switch; perf gate enforces budgets.

Sub-goals:
- Finish M1 (GPU-resident tiles).
- M2: `ed_gpu` + `ed_render` v1 (terrain clipmap, GPU-culled instancing, visibility buffer, cached shadow maps, own TAA upscaler).
- Cluster LOD + software raster (Nanite-like): ~128-tri clusters, GPU culling, 64-bit visibility buffer, material resolve shades each pixel once.
- Geometry clipmap for terrain (constant vertex count, geomorph in vertex shader) — replaces tile-based terrain for the ground view.
- Instanced building modules (windows, balconies, AC units).
- CDLOD geomorphing for terrain; crossfades for building detail.
- Virtual shadow maps (near range); far-range horizon maps stay.

Related docs: `plans/nanite-roadmap.md`, `plans/m2.md`, `plans/engine.md`, `ROADMAP.md` Stages B/D.

---

## 8. Performance: measure first, then fix

**Goal:** Know what limits the frame; don't guess.

Status: **In progress (Stage A).** Tools exist: `tools/perf_headless.sh`, `EW_BISECT`, `EW_GPU_WAIT`, `EW_POST`, `EW_TILE_STATS`, `--perf` CSV, pause-menu Debug tab, `tools/gpu_passes.sh`.

Open:
- Baseline table on 6 reference scenes (fps, p99, worst frame) — not yet recorded.
- Per-pass GPU timers verdict on hypotheses (vertex bound, draw count, shadow cost, post stack, clouds, fill rate, CPU vs GPU).
- Rank findings into M2 priority list.
- `tools/perf_gate.sh` (fail if p99 exceeds budget by 20%).
- Fix tile-load hitch (white tile ~1 s freeze): simplification moved to worker; remaining is main-thread apply (mesh merge + collider) and triangle count. See `plans/tile-load-hitch.md`.
- City performance is below 30 fps floor (26–27 fps settled at full res in Shibuya); needs per-building far LOD + vertex compression + mesh arena.

Related docs: `ROADMAP.md` Stage A; `docs/CHANGES.md`; `docs/BUGS.md`.

---

## 9. Multiplayer: same world for everyone

**Goal:** Shared, persistent, deterministic world.

Status: **Ghost multiplayer baseline works** (relay sends position/movement/pose; protocol v5 shares IK rig, wing deformation, flames). Hardening in progress.

Sub-goals:
- Determinism: golden-hash CI per reference tile (no platform-dependent hashing, no thread-order float reductions, f64/fixed-point in generators).
- Pin data versions: stamp tile cache with OSM/elevation/land-cover snapshot.
- Separate sim state from render state (fixed timestep sim, interpolated render).
- `ed_store`: persistent world deltas (save files + multiplayer share format), reliable delivery.
- Procedural climate (`ed_climate`): weather as pure function of (seed, UTC), replacing Open-Meteo; deterministic spin-up/fast-forward from checkpoints.
- Netcode: authoritative server per region shard, interest management by speed (walking small, jet large, orbit coarse), prediction/reconciliation for owned, interpolation for others. Candidates: `lightyear` or `renet` over QUIC.
- Physics note: Avian not deterministic across machines → server owns vehicles/rocket; clients render/predict.
- Parked vehicles as state (see goal 4) — only state travels, no live bodies at distance.

Related docs: `plans/multiplayer.md`, `ROADMAP.md` Stage C.

---

## 10. Living world and play

**Goal:** A game worth playing on top of the renderer.

Status: **Prototypes exist** (cars, planes, delivery missions).

Sub-goals:
- Vehicles: helicopters, rockets, spaceships, boats; better handling, runways, gearing, engine audio.
- Missions: finish delivery, add racing, then more types (community input).
- Living world: NPCs, AI traffic, parked cars, biome wildlife (start with urban birds like pigeons).
- Customization: beyond backpack → character appearance/equipment; persist character + world state.
- Water/weather: coupled ocean simulation (waves, buoyancy), rain, snow, wet roads, fog, severe local storms (hurricanes, tornadoes, sandstorms).
- Building interiors (procedural floor plans), bridges/tunnels with real decks, landmarks from OSM tags.
- Wildlife, flora procedural (`plans/procedural-flora.md`).

Related docs: `ROADMAP.md` Stage F; `plans/world-sim.md`.

---

## 11. Lighting and atmosphere

**Goal:** Cinematic look within budget; every feature has cost per tier + off switch.

Status: **Base done** (HDR, atmosphere, env-map lighting, bloom, SSAO, TAA, Blender Filmic, real-lux sun, cascaded shadows, Moon phases).

Open:
- Cheap high-value: GTAO, froxel fog, motion blur, night lights (lit windows, street lights, NASA city lights from orbit).
- Virtual shadow maps (near range); far-range horizon maps stay.
- DDGI global illumination, hardware RT where available.
- Weather visual: rain, snow, wet roads, visibility fog, severe local storms.
- Atmosphere from orbit unification (see goal 1).

Related docs: `ROADMAP.md` Stage E; `plans/shadows.md`.

---

## 12. Platforms

**Goal:** Ship beyond the dev Mac.

Status: **Dev path exists** (Bevy/wgpu, Windows CI + test instructions).

Open:
- Windows: broader PC support including low-end; native D3D12 with DLSS/FSR3, RTX; Windows perf gate.
- Mobile: iOS first (shares Metal backend; tile memory is the big win), then Android (Vulkan). Low-power tier with its own budget.
- macOS app bundle + signing.

Related docs: `ROADMAP.md` Stage H; `docs/WINDOWS.md`.

---

## 13. Audio

**Goal:** Engine sound kit ported; vehicles sound right.

Status: **Port done** (`ed_engine_audio`: engine voices, jet/steam/pod-racer/warp, mixer, cars from `data/cars.json`). Engine audio streams nearest 6 voices.

Open: remote spatial movement audio, distance mix, valley plume grounding (being tuned). See `docs/lookdev/round2/engine-audio/NOTES.md`.

---

## 14. Data and world fidelity (backlog)

- Landmarks/special buildings from OSM tags (castles, churches, mosques, temples, stadiums, skyscraper crowns).
- Building interiors (procedural floor plans).
- Bridges/tunnels with real decks; road elevation smoothing.
- ESA WorldCover + Macrostrat geology landed (unverified on Mac); geology-tinted sand/scree, canopy height, SoilGrids.
- Procedural flora (`plans/procedural-flora.md`); skateparks/ski resorts from OSM.
- NASA LOLA/MOLA for real Moon/Mars terrain; Gaia/Hipparcos stars; NASA SVS city lights.
- TerraClimate (real climate instead of latitude bands).

---

## 15. Engineering hygiene

- CI: `cargo test --workspace` + lookdev snapshot diff.
- Style/kit tables to `assets/styles/*.ron` with hot reload.
- Profiling HUD: GPU pass timings, memory (partly done: F3/I stats, `--perf` CSV).
- Tile builder IO pool + cancellation (done, `ed_runtime::jobs`).
- Pinned data versions, deterministic golden-hash CI (Stage A).

---

## Notes on "am I building this wrong?"

The foundation (f64 ECEF truth, floating origin, slippy tiles, engine-free crates, native Metal target) is correct and is the standard approach for this kind of game. The pain points are concentrated in the **altitude transition layer** (ground ↔ orbit) and the **streaming/collider coupling**, both of which are unfinished rather than wrongly designed. The fix is a layer on top, not a rewrite. See `plans/space-visuals.md` and `plans/nanite-roadmap.md` for the planned work. Cubesphere is not needed. Raymarched clouds already exist. Live ground physics from orbit is not feasible and not desirable; use parked-state persistence instead.