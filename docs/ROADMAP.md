# Roadmap

What's done, what's next. Recent changes and open checks: `CHANGES.md`. Plans: `plans/` (engine, assets and agents, buildings, world sim, space visuals, shadows). How it works: `ARCHITECTURE.md`.

Phases are ordered so there's always something playable. Each item is sized to be a good standalone task for a Claude Code session (one Mac, one day). Two tracks run side by side: **features** (Phases 0-5) and **engine and performance** (M0-M10, from `plans/engine.md`).

## Now

0. **Open bugs from the 2026-11-04 play session**: `BUGS.md` (B1-B12: LOD flash, impostor flicker, shadow cascades, road/guardrail/path geometry, atmosphere/satellite/cloud altitude transitions, camera lag).
1. **Mac test pass for M0 + M1** (`CHANGES.md`, about 20 minutes): fps, hitches, forests/impostors, building explode, colliders. Everything since `c42ed5e` is unmeasured on Metal.
2. **M0 leftovers**: pipeline pre-warm (confirm the AO/TAA cause with the test pass first). Clouds ½ res done (`cloud_half.rs`, -2.3 ms).
3. **M2 start**: `ed_gpu` RHI (wgpu backend) + `ed_render` v1 (terrain clipmap, GPU-culled instancing, visibility buffer, cached shadows, own TAAU).
4. Feature side: art direction round 2 review; `ed_climate` milestones 1-3.

Getting around until Phase 4: **rocket boots and teleporting**. Vehicles come after Space.

Weather is **procedural** (`ed_climate`, see `plans/world-sim.md`): a deterministic function of (world seed, UTC time), not live data. Open-Meteo stays only as a dev fallback until `ed_climate` lands. Space visuals: `plans/space-visuals.md`.

## Engine and performance track

Targets and design: `plans/engine.md`. Baseline (M3 Pro, Ohio suburb, 67% render scale): 33.5 fps, 56k entities, vertex bound.

| M | What | Status |
|---|---|---|
| **M0** | Quick wins in Bevy. Exit: 45+ fps steady, no frame > 50 ms after load | **Built, not yet measured.** Done: 2 shadow cascades; env map re-rendered only on change; one shared terrain material (satellite texture pool); staged, budgeted tile apply; tree impostors to ~20-30 km (user approved for trees); far trees out of shadows. Clouds ½ res + shadow proxies done (headless numbers in `docs/CHANGES.md`). Open: pipeline pre-warm, explode hitches. |
| **M1** | `ed_runtime` + tiles stop being entities. Exit: tile apply < 1 ms, flat entity count | **Built, not yet measured.** Done: `ed_runtime` (job pools with priority/cancel, frame budgets, residency LRU, height-aware view test, hole-free retire); IO and CPU pools; out-of-view tiles stay resident; a tile is ~10 entities (merged props, merged LOD buildings that explode near detail, one impostor mesh, cards/trees/rocks only near the camera). Entities at Zermatt village: 16,503 -> 2,166. Left for M2: fully GPU-resident tiles (no per-tile entities). |
| **M2** | `ed_gpu` RHI (wgpu) + `ed_render` v1: terrain clipmap, GPU-culled instancing, visibility buffer, cached shadows, own TAAU. Exit: 60 fps steady on the 6 reference scenes | Next |
| **G1** | Decision gate: native backends | After M2 |
| **M3** | Native Metal (if G1 says so), cluster LOD (Nanite-like) + software raster. Exit: 120 fps High on M3 Pro | |
| **M4** | Lighting: DDGI, virtual shadow maps, GTAO, froxel fog, motion blur, night lights | |
| **M5** | App shell off Bevy, `ed_store` persistence, agents framework, missions v1 | |
| **M6** | Water: FFT ocean + local shallow water, buoyancy | |
| **M7** | Windows + DLSS/FSR3 | |
| **M8** | Multiplayer | |
| **M9** | Mobile (iOS, then Android) | |
| **M10** | Space: reference frames, orbits, procedural planets, galaxy | |

## Phase 0: Foundation (done)

- [x] Workspace: `ed_geo`, `ed_data`, `ed_world`, `ed_flight`, `ed_space`, `ed_game`
- [x] WGS84/ECEF/ENU math, slippy tiles, real sun position (tested)
- [x] Data: Mapterhorn + AWS elevation, OpenFreeMap vector tiles (hand-written MVT decoder), Open-Meteo weather (dev fallback), geocoding, disk cache
- [x] World gen: terrain, land cover raster, biomes, 17 architectural regions, street kits, roads, street props, trees, rocks, inland water
- [x] Quadtree LOD, floating origin, tile streaming with hole-free swaps
- [x] Filmic pipeline: HDR, atmosphere, atmosphere env-map lighting, bloom, SSAO, TAA
- [x] Moon + planets at true positions; delivery mission planner over real airports/spaceports
- [x] Lookdev boards for art review

## Phase 1: Feel (now)

1. **Art direction rounds** from lookdev review
   - [x] Round 1 review; Blender Filmic picked
   - [x] Robot courier v2: connected cylinder/ball-joint body, CRT TV head, backpack as a cosmetic
   - [x] Buildings v2: modeled facades fitted to each wall, party walls, doors/stoops/shopfronts, balconies, shutters, AC units, fire escapes, cornices, flat/pitched/mansard roofs with rooftop gear, house lots; per-region variants; chunked detail streaming
   - [x] Trees v2: branching trees, leaf cards, wind sway, more species
   - [x] Ground v2: splat terrain material, procedural textures, near-camera grass/rocks/moss
   - [x] Clouds v2: volumetric, translucent (driven by live weather for now; switch to `WeatherSample` from `ed_climate`)
   - [ ] Round 2 review
2. [x] **Teleport**: type a place name (or pick on the satellite globe map with roads/borders), drop in from altitude and skydive down
3. **Rocket boots + skydive + parachute + wingsuit**: done: boots, skydive, parachute, wingsuit, Mach 1 boom, camera shake. Left: vapor cone, speed lines, wind from `ed_climate`
4. [x] **HUD**: compass strip with waypoint pins, mode pill, minimap from vector tiles, target rings for bodies/waypoints with distance and ETA
5. **Procedural climate** (`ed_climate` milestones 1-3): climate base (insolation, lapse rate, wind belts, orographic rain), deterministic synoptic grid, storm records; `WeatherSample` replaces `CurrentWeather` in clouds/foliage/sky
6. Weather FX from `WeatherSample`: rain and snow particles, wet roads, fog from visibility; later local storms (hurricane, tornado, sandstorm) per `plans/world-sim.md`

## Phase 2: World fidelity

1. GPU instancing for repeated building modules (windows, balconies, AC units): the big triangle win for dense cities (lands with M2's GPU-culled instancing)
2. CDLOD geomorphing for terrain (no popping; M2 terrain clipmap); crossfades between building detail and distant versions
3. Landmarks and special buildings from OSM tags: castles, churches, cathedrals, mosques, temples, stadiums, skyscraper crowns, old town walls
4. ~~ESA WorldCover land cover; Macrostrat geology for rock color~~ landed 2026-11-04 (unverified on Mac, see CHANGES). Procedural flora (species as data, per-tree genomes, tiers): `plans/procedural-flora.md`, step 1 + tree/rock collision landed. Left: geology-tinted sand/scree textures, ETH/Hansen canopy height, SoilGrids; weather stays a separate procedural sim (Phase 1.5)
5. Building interiors (procedural floor plans, enterable ground floors)
6. Bridges and tunnels with real decks; road elevation smoothing
7. Night: lit windows, street lights, NASA SVS city lights from orbit
8. Rivers that flow downhill, waves, shoreline foam
9. Skateparks and ski resorts from OSM (`leisure=skatepark`, `piste:*`, aerialways)

## Phase 3: Space (by rocket boots)

1. Rocket boots superpower stages that reach orbit and beyond (boost stages exist); left: time compression on long coasts
2. Earth from orbit: low-zoom globe tiles, atmosphere from space, cloud layer from `ed_climate` (planet shell exists; see `plans/space-visuals.md`)
3. The Moon: procedural cratered terrain, then NASA LOLA elevation; land and walk
4. Mars (MOLA), then the rest, with HUD target rings and approach guidance
5. Starfield: procedural sky done (`starfield.rs`); next a real Hipparcos/Yale catalog

## Phase 4: Vehicles

1. Car: raycast suspension, tire model, torque curve, gearbox; body styles (basic version exists)
2. Engine audio: done (`ed_engine_audio`, the engine sound kit port); next: Doppler, a rocket generator, per-car gearing
3. Plane: per-surface lift/drag, thrust, flaps, landing gear; runways from OSM aeroways
4. Helicopter
5. Rocket ship with superpower stages; launch from real launch sites
6. Traffic: AI cars on road centerlines (already exported)

## Phase 5: Game

1. Delivery missions with the planner: pick up, multi-leg route, timer, scoring
2. NPCs (pedestrian robots) on sidewalks, animals by biome (deer, camels, penguins...)
3. Photo mode + postcards ("Greetings from Zermatt")
4. Title screen: spinning Earth (done: `globe.rs`, satellite tiles, atmosphere, stars); left: procedural clouds and city lights
5. Character customization (backpack and more), save/load, settings, controller support

## Engineering

- CI: `cargo test --workspace` + lookdev snapshot diff
- Move style/kit tables to `assets/styles/*.ron` with hot reload
- [x] Tile builder: dedicated IO thread pool + cancellation for tiles that went stale (`ed_runtime::jobs`, M1)
- Profiling HUD (frame time, tiles in flight, triangles, memory): partly done (F3/I debug stats with pipeline depth, `--perf` CSV with entities and pipeline columns); left: GPU pass timings, memory
- Perf gate script (`tools/perf_gate.sh`, 6 reference routes; `plans/engine.md` §13)
- macOS app bundle + signing
