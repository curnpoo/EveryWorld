# Roadmap

Phases are ordered so there's always something playable. Each item is sized to be a good standalone task for a Claude Code session (one Mac, one day).

## Phase 0: Foundation (done in session 1)

- [x] Workspace: `ed_geo`, `ed_data`, `ed_world`, `ed_flight`, `ed_space`, `ed_game`
- [x] WGS84/ECEF/ENU math, slippy tiles, real sun position (tested)
- [x] Data: Mapterhorn + AWS elevation, OpenFreeMap vector tiles (hand-written MVT decoder), Open-Meteo weather, geocoding, disk cache
- [x] World gen: terrain grid/mesh/collider, land cover raster, biomes, 17 architectural regions, street kits, buildings (windows, shopfronts, awnings, gable/hip/flat roofs), roads (markings, sidewalks, guardrails), street props, trees (8 species), rocks, inland water
- [x] Quadtree LOD, floating origin, tile streaming with hole-free swaps
- [x] Filmic pipeline: HDR, atmosphere, atmosphere env-map lighting, AgX, bloom, SSAO, TAA
- [x] Moon + planets at true positions; delivery mission planner over real airports/spaceports
- [x] Lookdev boards for art review

## Phase 1: Feel (next)

1. **Art direction pass** driven by lookdev review: palettes, tree shapes, facade textures, cloud style, ground swatches
2. **Robot courier**: physics capsule + torso-velocity PD controller; procedural robot body with foot planting, two-bone leg/arm IK, head bob; first/third person camera
3. **Rocket shoes + skydive + parachute + wingsuit**; Mach 1 boom, vapor cone, camera shake, speed lines
4. **HUD**: compass strip with waypoint pins, mode pill (mode, speed, altitude, vertical speed), minimap from vector tiles, target rings for bodies/waypoints with distance and ETA
5. **Clouds in the sky**: weather-driven coverage and type (puffy / towering / stratus), wind drift
6. Weather FX: rain and snow particles, wet roads, fog from visibility

## Phase 2: Vehicles

1. Car: raycast suspension, tire model, torque curve, gearbox; 3 body styles
2. `ed_audio`: per-cylinder engine synthesis (firing order, exhaust resonator, intake), I4/V8/flat-6/turbine/rocket profiles
3. Plane: per-surface lift/drag, thrust, flaps, landing gear; runways from OSM aeroways
4. Helicopter
5. Rocket ship with superpower stages; launch from real launch sites
6. Traffic: AI cars on road centerlines (we already export them)

## Phase 3: World fidelity

1. CDLOD geomorphing for terrain (no popping)
2. VisibilityRange crossfades + impostors for buildings and trees
3. Procedural micro-detail under the camera: displacement noise, pebbles, grass blades by ground type
4. ESA WorldCover land cover; TerraClimate climate; Macrostrat geology for rock color
5. More building types from OSM tags: churches, stadiums, skyscraper crowns, houses vs apartments, industrial sheds; landmark overrides
6. Building interiors (procedural floor plans, enterable ground floors)
7. Bridges and tunnels with real decks; road elevation smoothing
8. Night: lit windows, street lights, NASA SVS city lights from orbit
9. Rivers that flow downhill (per-segment water levels), waves, shoreline foam
10. Skateparks and ski resorts from OSM (`leisure=skatepark`, `piste:*`, aerialways)

## Phase 4: Space

1. Earth from orbit: low-zoom globe tiles + atmosphere from space + cloud layer
2. Superpower rocket to the Moon; procedural cratered Moon terrain, then NASA LOLA
3. Mars (MOLA), then the rest with HUD target rings and approach guidance
4. Starfield from Gaia/Hipparcos

## Phase 5: Game

1. Delivery missions with the planner: pick up, multi-leg route, timer, scoring
2. NPCs (pedestrian robots) on sidewalks, animals by biome (deer, camels, penguins...)
3. Photo mode + postcards ("Greetings from Zermatt")
4. Title screen: spinning Earth with real clouds and city lights
5. Save/load, settings, controller support

## Engineering

- CI: `cargo test --workspace` + lookdev snapshot diff
- Move style/kit tables to `assets/styles/*.ron` with hot reload
- Tile builder: dedicated IO thread pool + cancellation for tiles that went stale
- Profiling HUD (frame time, tiles in flight, triangles, memory)
- macOS app bundle + signing
