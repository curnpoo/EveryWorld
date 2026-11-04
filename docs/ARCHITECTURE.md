# EveryWorld architecture

Drop anywhere on the real Earth. Walk, rocket-jump, skydive, drive, fly, and launch rockets to the Moon. The world is built from public map data and made to look like *that place* through procedural rules.

## The one-sentence version

**Real data decides what is there. Procedural rules decide how it looks. A floating origin lets one engine cover everything from a pebble to Neptune.**

## Stack

| Layer | Choice | Why |
|---|---|---|
| Language | Rust | Fast, safe threading for streaming, one toolchain |
| Engine | Bevy 0.20 | ECS, modern renderer, Metal on macOS via wgpu |
| Physics | Avian 0.8 | ECS-native, continuous collision, good character/vehicle primitives |
| Renderer backend | wgpu (Metal on Mac, Vulkan elsewhere) | Native Apple Silicon, no translation layer |

## Crates

```
crates/
  ed_geo     WGS84/ECEF/ENU math, slippy tiles, real sun position         (no engine)
  ed_data    HTTP + disk cache, elevation, vector tiles (MVT), weather     (no engine)
  ed_world   terrain, ground cover, buildings, roads, trees, missions      (no engine)
  ed_flight  atmosphere, drag, Mach, gravity, "superpower" rockets         (no engine)
  ed_space   Moon + planet positions (real ephemeris), body catalog        (no engine)
  ed_engine_audio  engine sound kit port: engine voices, mixer, game adapter  (no engine)
  ed_runtime jobs (IO + CPU pools, priority, cancel), frame budgets, residency, streaming scheduler  (no engine)
  ed_game    the Bevy app: streaming, rendering, player, vehicles, HUD
```

Everything except `ed_game` is engine-free, unit tested, and runs on any thread. If we ever change engines (or add a server), the world generator comes with us.

## Coordinate systems (the "big world" problem)

`f32` has ~7 significant digits. At 10 km from the origin that's millimeter precision; at 6,371 km (Earth's radius) it's half a meter, and everything jitters. So:

1. **Truth is f64 ECEF** (Earth-centered, Earth-fixed meters). Every static thing stores its placement as an f64 affine in ECEF (`GeoAnchored`).
2. **Rendering is f32 in a local East-North-Up frame** around the player (x=East, y=Up, z=-North). Curvature is real: a tile 50 km away is genuinely below the horizon.
3. **Floating origin**: when the player is >8 km from the frame origin, the frame moves under them. Static entities recompute their transforms from ECEF; moving bodies (player, vehicles, camera) are rigidly transformed, including velocities.
4. **Space**: Moon and planets are tracked in f64 ECEF too (0.5 mm precision at the Moon). The renderer only ever draws relative to the camera, and Bevy's reverse-Z infinite projection handles the depth range.

## Data sources (all free, no API keys)

| What | Source | Notes |
|---|---|---|
| Elevation | **Mapterhorn** (Terrarium WebP, 512 px) | Copernicus GLO-30 + national lidar (swisstopo, USGS 3DEP) |
| Elevation fallback | AWS Terrain Tiles (Terrarium PNG) | SRTM/GMTED + ETOPO1 bathymetry |
| Map vectors | **OpenFreeMap** (OpenMapTiles schema) | buildings w/ heights + colour, roads, water, land cover, land use, parks, airports, POIs |
| Weather | Open-Meteo | current + 48 h hourly forecast |
| Place search | Open-Meteo geocoding | "drop me in Zermatt" |
| Moon/planets | JPL Keplerian elements + Almanac lunar series | computed locally, no network |

Planned additions (see `DATA.md`): ESA WorldCover 10 m land cover, TerraClimate (real climate instead of latitude bands), Macrostrat geology (rock color/type), NASA LOLA/MOLA for real Moon and Mars terrain, Gaia/Hipparcos stars, NASA SVS city lights.

Everything is cached on disk forever (tiles are immutable per planet build), so the second visit to a place is instant and offline.

## World generation pipeline (per tile)

```
tile id ─┬─> elevation tile ──> TerrainGrid (single source of truth for ground height)
         └─> vector tile ─────> SurfaceMap (land cover raster: wood/grass/farmland/sand/rock/ice/urban/water)
                                   │
   biome (climate) + region (architecture/street kit) from lat/lon/elevation
                                   │
   ┌───────────┬───────────┬───────┴──────┬──────────────┬──────────────┐
 terrain     water       roads         buildings       foliage        props
 mesh+       (inland;    ribbons,      footprints ->   trees by       street lights,
 collider    ocean is    markings,     walls w/ window biome, rocks   poles, signals,
             global)     sidewalks,    grid, shops,    by cover       stop signs
                         guardrails    awnings, roofs
```

- Everything that sits on the ground samples the **same** `TerrainGrid` with the same triangle interpolation as the render mesh and the physics collider. Nothing floats, nothing sinks.
- Each tile becomes a handful of merged meshes (one draw call for all trees in a tile, one for all facades, etc.).
- Generation is deterministic (hash of OSM id / tile / cell), so the world looks identical every run on every Mac.
- Typical z14 tile: 0.3 to 1 s on one core, 400k to 800k triangles.

### Making places look like themselves

`style.rs` and `roads.rs` hold data tables, not code paths:

- **Region** (from lat/lon/elevation/density): `NorthAmericaCity`, `Paris`, `Mediterranean`, `Alpine`, `Nordic`, `Japan`, `NorthAfricaMiddleEast`, ... Each has wall/roof palettes, roof shapes and pitch, floor height, default storeys, shopfront odds, awning colors.
- **StreetKit** per region: drive side, center-line style (double yellow vs dashed white), utility poles (wood/concrete/none), signal layout (vertical vs Japanese horizontal), sidewalk color, curb height, stop-sign text.
- **Biome** (climate): drives ground palette, tree species mix (palms, conifers, acacias, cacti, cypress...), snowline, tree line, rock density.
- OSM `building:colour` always wins when present.

Adding "Kyoto machiya" or "Lagos compound" is one table entry. These tables move to `assets/styles/*.ron` so art direction can be tuned without recompiling.

## Level of detail ("never see it pop")

- **Terrain**: quadtree over map tiles, ~2 km tiles under you, ~600 km tiles at the horizon, a few hundred total. Skirts hide cracks. Next: CDLOD geomorphing so vertices slide between levels instead of popping.
- **Buildings/props/trees**: merged per tile today; next is Bevy `VisibilityRange` dithered crossfades between full detail, impostor and nothing.
- **Nanite-style virtual geometry**: Bevy has experimental *meshlets*. Tracked as an experiment once it's proven on Metal; the pipeline already produces dense static meshes it can ingest.
- **Down to pebbles**: data stops at ~5 m resolution, so below that we add procedural micro-detail (noise displacement + scattered pebbles/grass near the camera, by ground type).
- **Up to planets**: a body is a dot (HUD marker), then a sphere, then a quadtree of terrain tiles. Same LOD system at every scale.

## Lighting ("filmic, not photoreal")

- HDR camera, physically based **Atmosphere** (sky, sunsets, aerial perspective from space down to the street)
- **AtmosphereEnvironmentMapLight**: the sky generates the environment map, so ambient light and reflections match time of day
- Real-lux sun at its true position (NOAA algorithm) with cascaded shadows; Moon lit by the same sun, so phases are correct
- SSAO, TAA, bloom, **Blender Filmic** tone mapping (picked in lookdev round 1)
- Next: irradiance-volume light probes for interiors and street canyons, screen-space reflections on water and glass, volumetric clouds driven by weather

## Characters, vehicles, physics

- **Player**: physics capsule; input sets *target torso velocity*, a PD controller applies forces. The robot body is procedural on top: ground raycasts pick foot targets, two-bone IK for legs and arms, head bob feeds the first-person camera. First and third person share the rig.
- **Rocket shoes / ships**: real forces (gravity falls off with altitude, ISA atmosphere, quadratic drag, Mach from real speed of sound) with "superpower" thrust stages and inertial-damper assist. Breaking Mach 1 fires a boom, vapor cone and camera shake.
- **Cars**: raycast suspension per wheel (spring + damper), tire friction model, engine torque curve.
- **Planes**: lift/drag from angle of attack per surface, thrust, control surfaces.
- **Engine audio** (`ed_engine_audio`): the owner's engine sound kit (`vendor/engine-sound-kit`) ported to Rust: every cylinder fires in its firing order through header delays, tailpipe waveguides, muffler and body resonances, with turbo, blow-off, pops and bangs; 31 vehicles including a jet. `ed_game/src/engine_audio.rs` streams the nearest six on the audio thread with distance gain and pan. See `docs/lookdev/round2/engine-audio/NOTES.md`.

## Space

- `ed_space` gives true positions of the Sun, Moon and planets for any time (tested against the April 2024 eclipse and the Jan 2025 Mars approach).
- HUD target rings for bodies and waypoints, with distance and ETA, from "blip in the sky" to landing.
- Moon/Mars terrain: procedural cratering first, then NASA LOLA/MOLA elevation through the same tile pipeline.

## Missions

`missions.rs` plans multi-modal deliveries over real hubs: drive to an airport, fly to a launch site (they have runways), rocket to the launch site nearest the destination, fly, drive to the door. Brooklyn to Kyoto routes JFK, Kennedy Space Center, Tanegashima, Osaka Itami. Airports also stream in from map tiles.

## Running on Mac

```bash
cargo run --release -- --place "Zermatt"
cargo run --release -- --lat 43.53 --lon 5.45 --alt 3000
cargo run --release -- --place "Shibuya" --hour 18.5
```

## Testing

- `cargo test --workspace` runs fast, offline unit tests (geo math, sun, ephemeris, MVT decode, meshes face the right way, regions, missions).
- `cargo test -- --ignored` hits the real data sources.
- `--screenshot out.png` renders a frame after streaming settles; used for lookdev boards and visual regression.
