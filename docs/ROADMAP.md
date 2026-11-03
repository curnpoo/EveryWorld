# Roadmap

Phases are ordered so there's always something playable. Each item is sized to be a good standalone task for a Claude Code session (one Mac, one day).

Getting around until Phase 4: **rocket boots and teleporting**. Vehicles come after Space.

## Phase 0: Foundation (done)

- [x] Workspace: `ed_geo`, `ed_data`, `ed_world`, `ed_flight`, `ed_space`, `ed_game`
- [x] WGS84/ECEF/ENU math, slippy tiles, real sun position (tested)
- [x] Data: Mapterhorn + AWS elevation, OpenFreeMap vector tiles (hand-written MVT decoder), Open-Meteo weather, geocoding, disk cache
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
   - [ ] Trees v2: branching trees, leaf cards, wind sway, more species (parallel session)
   - [ ] Ground v2: splat terrain material, procedural textures, near-camera grass/rocks/moss (parallel session)
   - [ ] Clouds v2: volumetric, weather driven, translucent (parallel session)
   - [ ] Round 2 review
2. **Teleport**: type a place name (or pick on a globe map), drop in from altitude and skydive down
3. **Rocket boots + skydive + parachute + wingsuit**: Mach 1 boom, vapor cone, camera shake, speed lines, wind from live weather
4. **HUD**: compass strip with waypoint pins, mode pill, minimap from vector tiles, target rings for bodies/waypoints with distance and ETA
5. Weather FX: rain and snow particles, wet roads, fog from visibility

## Phase 2: World fidelity

1. GPU instancing for repeated building modules (windows, balconies, AC units): the big triangle win for dense cities
2. CDLOD geomorphing for terrain (no popping); VisibilityRange crossfades between building detail and distant versions
3. Landmarks and special buildings from OSM tags: castles, churches, cathedrals, mosques, temples, stadiums, skyscraper crowns, old town walls
4. ESA WorldCover land cover; TerraClimate climate; Macrostrat geology for rock color
5. Building interiors (procedural floor plans, enterable ground floors)
6. Bridges and tunnels with real decks; road elevation smoothing
7. Night: lit windows, street lights, NASA SVS city lights from orbit
8. Rivers that flow downhill, waves, shoreline foam
9. Skateparks and ski resorts from OSM (`leisure=skatepark`, `piste:*`, aerialways)

## Phase 3: Space (by rocket boots)

1. Rocket boots superpower stages that reach orbit and beyond; time compression on long coasts
2. Earth from orbit: low-zoom globe tiles, atmosphere from space, cloud layer
3. The Moon: procedural cratered terrain, then NASA LOLA elevation; land and walk
4. Mars (MOLA), then the rest, with HUD target rings and approach guidance
5. Starfield from Gaia/Hipparcos

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
4. Title screen: spinning Earth with real clouds and city lights
5. Character customization (backpack and more), save/load, settings, controller support

## Engineering

- CI: `cargo test --workspace` + lookdev snapshot diff
- Move style/kit tables to `assets/styles/*.ron` with hot reload
- Tile builder: dedicated IO thread pool + cancellation for tiles that went stale
- Profiling HUD (frame time, tiles in flight, triangles, memory)
- macOS app bundle + signing
