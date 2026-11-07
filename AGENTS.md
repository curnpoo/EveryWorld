# EveryWorld — Agent Guide & Project Index

Drop anywhere on real Earth; walk, drive, fly, rocket to the Moon. Real data
(OSM vector tiles, elevation, weather) decides **what** is there; procedural
rules decide **how it looks**.

This file is the map of the repository. `CLAUDE.md` is the short working memory;
`docs/ROADMAP.md` says what's done and next (Stages A-H);
`docs/CHANGES.md` lists recent changes and open checks; `docs/ARCHITECTURE.md`
is the design document; `docs/plans/` holds the plans.

---

## Direction and goals (read before planning any work)

Goal: a fast, beautiful, shared real-Earth world. The roadmap is Stages A-H in
`docs/ROADMAP.md` (A stabilize and measure, B fast, C multiplayer, D native
Metal + Nanite-like, E lighting, F play/vehicles, G space, H platforms).
**Current stage: A. Next stop: GPU measurement** (baseline table on the 6
reference scenes, per-pass GPU timers, a verdict on each performance
hypothesis; ROADMAP Stage A item 3), then M2 in Stage B.

Rules for every agent and session:

1. **Performance and looks before new gameplay.** Vehicles, missions and space
   are planned (Stages F, G) but wait. The game misses 30 fps on an M3 Pro and
   has visual glitches (`docs/BUGS.md`); that comes first.
2. **Renderer end state:** native Metal plus a Nanite-like geometry system we
   build ourselves (cluster LOD, GPU culling, visibility buffer, software
   raster) on `ed_gpu` / `ed_render`. Windows uses D3D12 with DLSS/FSR3 and RTX
   ray tracing. Bevy meshlets are not the path (measured ~60% slower). Short-term
   fixes on the Bevy renderer are fine; do not build large features that only
   work on Bevy's pipeline. New renderer code targets `ed_gpu` only, never wgpu
   or Metal directly.
3. **Use the GPU fully, and measure first.** Tools: `tools/perf_headless.sh`,
   `EW_BISECT`, `EW_GPU_WAIT`, `EW_POST`, `EW_TILE_STATS`, `--perf`. Add a dated
   `docs/CHANGES.md` entry with before/after numbers. Every feature declares its
   cost per quality tier and has an off switch.
4. **Multiplayer-safe from now on.** Every player sees the same world:
   generation is a pure function of (world seed, pinned data version, UTC time).
   Avoid `HashMap` iteration order, thread-order float reductions, unseeded RNG,
   wall-clock reads and platform-dependent hashing in generators and weather.
   Terrain, buildings, trees and weather are never sent over the network; only
   world deltas and entity state are. Keep simulation state separate from render
   state. Avian physics is not deterministic across machines.
5. **Materials and shaders** are data-driven and hot-reloadable, shaped for a
   visibility-buffer resolve (shade each pixel once).
6. **Keep the docs current.** When work lands, add a `docs/CHANGES.md` entry
   (what changed, what needs a Mac check) and update `docs/ROADMAP.md`.
   `docs/plans/engine.md` still uses the old M0-M10 names; the mapping is at the
   bottom of the roadmap.

---

## Quick start

```bash
cargo run --release                                      # play (choose drop-in from the UI)
cargo run --release -- --lat 43.53 --lon 5.45 --alt 3000 # skydive in
cargo run --release -- --fly --lat .. --lon .. --alt ..  # spectator camera
cargo run --release -- --perf                            # writes perf.csv
cargo run --release -- --lookdev out_dir                 # art-review boards
cargo run --release -- --lat .. --lon .. --screenshot out.png --frames 60
```

Tests:

```bash
cargo test --workspace          # fast, offline unit tests
cargo test -- --ignored         # hits real network data sources
cargo check --offline -p ed_game
```

Windows (PowerShell; one-time setup, batteries and the agent checklist are in
`docs/WINDOWS.md`):

```powershell
cargo test --workspace          # same suite; CI runs it on macOS + Windows
cargo build --release           # -> target\release\earthdrop.exe
cargo run --release
```

Cache: `~/Library/Caches/earthdrop` on macOS, `%LOCALAPPDATA%\earthdrop` on
Windows (override with `EARTHDROP_CACHE`). Tiles are cached forever; weather
has a TTL.

---

## Workspace layout

```
crates/
  ed_geo/            f64 WGS84 / ECEF / ENU math, slippy tiles, NOAA sun
  ed_data/           HTTP + disk cache, elevation, MVT decoder, vector tiles, weather/geocode
  ed_world/          world generator: tile -> meshes, instances, map tiles, missions
  ed_flight/         atmosphere, drag, Mach, gravity, rocket "superpower" model
  ed_space/          Sun/Moon/planet ephemeris and body catalog
  ed_engine_audio/   Rust port of vendor/engine-sound-kit (JS is source of truth)
  ed_runtime/        M1 runtime services: job pools, frame budget, LRU, view cone, scheduler
  ed_game/           Bevy 0.20 + Avian 0.8 app (binary: earthdrop)
tools/               perf_headless.sh (repeatable perf scenes), shoot_places.sh, lookdev boards, climate bake
vendor/              engine-sound-kit JS reference, patched bevy_pbr
docs/                ROADMAP, CHANGES, ARCHITECTURE, plans/, lookdev notes
```

Dependency order: `ed_geo` → `ed_data` → `ed_world` → (`ed_flight`, `ed_space`,
`ed_engine_audio`, `ed_runtime`) → `ed_game`. Everything except `ed_game` is engine-free,
unit-tested, and thread-safe.

---

## Crate and module index

### `crates/ed_geo` — Earth math (f64, engine-free)

| File | Contents |
|---|---|
| `lib.rs` | `Geodetic` (lat/lon/alt ↔ ECEF), distance/bearing/destination, `EnuFrame` (ENU ↔ ECEF ↔ render), WGS84 constants, `wrap_lon`, `enu_to_render` / `render_to_enu` |
| `tile.rs` | `TileId` slippy-map addressing: containing tile, uv↔geodetic, parent/ancestor/children/offset, `width_m`, `path` |
| `sun.rs` | NOAA solar position (`sun_position`, `julian_day`), ENU direction |

Render frame convention: **x = East, y = Up, z = −North**.

### `crates/ed_data` — remote data + cache (engine-free)

| File | Contents |
|---|---|
| `lib.rs` | Public exports; data-source/licence table |
| `cache.rs` | `DiskCache`: durable keyed disk cache, `get`/`put`/`get_or_fetch`, default macOS/Linux location |
| `http.rs` | Shared blocking HTTP agent (OS trust store, `HTTPS_PROXY`) |
| `elevation.rs` | Terrarium elevation decode; Mapterhorn primary, AWS Terrain fallback |
| `mvt.rs` | Hand-written Mapbox Vector Tile v2 decoder (protobuf + geometry commands), `Tile`/`Layer`/`Feature` |
| `vector.rs` | OpenFreeMap tile template + fetch; `MAX_ZOOM = 14` native vector zoom |
| `weather.rs` | Open-Meteo current + 48 h hourly forecast |
| `geocode.rs` | Open-Meteo place-name search; Nominatim reverse geocode for arrival names |

### `crates/ed_world` — procedural world generator (engine-free)

| File | Contents |
|---|---|
| `lib.rs` | Module exports; `hash01(a, b)` deterministic hash |
| `tilegen.rs` | **Entry point.** One tile start-to-finish: fetch elevation + vector data → terrain, surface, water, roads, buildings, trees, props; `DetailChunks` for per-chunk building detail |
| `terrain.rs` | `TerrainGrid`: single source of truth for ground height (`sample(u,v)`), render mesh + collider share the same triangles |
| `project.rs` | `TileProjector`: tile uv + height → local ENU f32 render space |
| `buildings.rs` | OSM footprints → `BuildingSpec` (`prepare`), `build_lod`, `build_detail` (facades, plinth/cornice, flat/pitched/mansard roofs, foundations) |
| `archkit.rs` | Per-`Region` `Variant` tables: wall/roof palettes, roof forms, pitch, floors, ground-floor type |
| `style.rs` | `Region::locate` and regional `StreetKit` data tables |
| `bparts.rs` | Building part library: windows, doors, balconies, chimneys, shopfronts, AC units, fire escapes, rooftop gear |
| `meshkit.rs` | Modeling toolkit: `sweep`, `inset`, `polygon`, `wall_with_openings`, `Basis`, `signed_area` |
| `mesh.rs` | `MeshData` plain vertex/index buffers |
| `roads.rs` | Roads/paths/rail from `transportation`, draped on terrain, markings and street furniture |
| `water.rs` | Inland lakes/rivers from `water`; ocean is global in the game |
| `surface.rs` | `SurfaceMap` land-cover raster (`Cover`) from landcover/landuse/park/water |
| `biome.rs` | Coarse climate/biome classification from latitude, altitude, deserts |
| `foliage.rs` + `trees/` | Tree/shrub placement; procedural branching trees, leaf/bark textures; `trees/impostor.rs` bakes the impostor atlas and builds per-tile impostor quads |
| `props.rs` | Rocks and street furniture baked per tile |
| `ground_scatter.rs` | Near-camera ground cover (grass, flowers, pebbles, moss, scrub, snow, ripples) |
| `ground_tex.rs` | Procedural tileable ground textures per `Ground` layer |
| `noise.rs` | Value/fBm/Worley noise, plain and periodic/tileable |
| `clouds.rs` | Engine-free cloud/weather shaping and tileable noise volumes |
| `lod.rs` | Terrain quadtree LOD selection |
| `maprender.rs` | 2D map tiles for menu/minimap: hillshade, land cover, water, parks, roads, buildings, place labels; `render_at` supports overzoom past z14 |
| `missions.rs` | Delivery mission planner over real airports/launch sites |

Key invariants:
- Everything on the ground samples the **same** `TerrainGrid`; nothing floats or sinks.
- Generation is deterministic from OSM id / tile / cell via `hash01`.
- OSM `building:colour` always wins over region palettes.
- Buildings are authored in facade space: `y` is ground-relative, `gy` is street
  level, `wall_top` is eave height.

### `crates/ed_flight` — flight physics (engine-free)

`lib.rs`: `atmosphere(alt)` (ISA), `gravity`, `drag`, `MachTracker` /
`MachEvent`, `SuperRocket` (rocket shoes / rocket ship stages), `Body` integrator.

### `crates/ed_space` — real sky (engine-free)

`lib.rs`: `BodyId` / `BodyInfo` catalog, `Snapshot::at(unix_s)`, planet ECEF
positions, observer-relative sky direction/distance, GMST, `sun_dir_ecef`.

### `crates/ed_engine_audio` — engine sound kit port (engine-free)

| File | Contents |
|---|---|
| `lib.rs` | Crate docs and exports |
| `voice.rs` | `EngineVoice`: sample-accurate port of `synth.js` (cylinders, headers, tailpipe, muffler, resonances) |
| `special.rs` | Jet / steam / pod-racer / warp generators from `special.js` |
| `dsp.rs` | Shared filters/oscillators/noise/arithmetic |
| `mixer.rs` | Multi-voice mixer and game-thread `MixerHandle` commands |
| `driver.rs` | Vehicle state → voice controls/events |
| `cars.rs` | Vehicle data loaded from generated `data/cars.json` |
| `controls.rs` | Replay JS reference control streams |
| `analysis.rs` | RMS, spectral centroid, octave bands, pitch tracking |
| `wav.rs` | Minimal 16-bit PCM WAV read/write |
| `tests/parity.rs` | Rust vs JS reference parity tests |
| `examples/` | `render`, `compare`, `bench` |

Ignore this crate unless the task is audio.

### `crates/ed_game` — Bevy app

`main.rs` wires plugins; each module below is (usually) one Bevy plugin.

| File | Plugin / role |
|---|---|
| `main.rs` | App setup, CLI config, plugin wiring, window/log settings |
| `config.rs` | `Config`: `--place`, `--lat/--lon`, `--alt`, `--heading/--pitch`, `--hour`, `--fly`, `--lookdev`, `--perf`, `--screenshot`, `--scale`, `--no-post`, `--no-atmosphere` |
| `origin.rs` | Floating origin: `WorldOrigin`, `GeoAnchored`, `Rebased`, `Focus`; rebase >8 km |
| `assets.rs` | Shared materials and procedural textures |
| `building_materials.rs` | One material per `ed_world::bparts::Mat` bucket |
| `meshconv.rs` | `ed_world::MeshData` → Bevy `Mesh` |
| `streaming.rs` | Tile streaming on `ed_runtime` (IO pool, CPU pool, budgeted staged apply, hole-free retire); tiles are ~10 merged entities; building LOD explodes into chunks only while detail loads (`Explode`) |
| `ground_material.rs` | Terrain splat material and procedural ground textures |
| `ground_scatter.rs` | Near-camera ground-cover chunk streaming |
| `foliage.rs` | Foliage materials; `TreeChunks`: leaf cards (< 800 m) and full trees (< 230 m) become entities only near the camera |
| `impostors.rs` + `shaders/impostor.wesl` | Tree impostor material, atlas bake at startup, distance thinning and satellite tint |
| `near_chunks.rs` | `NearMeshes`: rock chunks as data, entities only within 500 m |
| `sky.rs` | Sun, atmosphere, Moon/planets, ocean, fog, weather, `Clock` |
| `clouds.rs` + `clouds.wesl` | Volumetric cloud raymarch and weather-driven sky |
| `shaders/` | Custom WESL shaders |
| `camera.rs` | Camera spawn and free-fly spectator controls |
| `player.rs` | Physics capsule, PD controller, ground probe, camera follow, `MoveMode`, input |
| `body.rs` | Procedural robot body: feet, IK limbs, head, cosmetics; `Cosmetics`, `Gait` |
| `vehicles.rs` | Cars, planes, raycast wheels, vehicle camera, `Driving` |
| `engine_audio.rs` | Streams nearest engine voices on the audio thread |
| `hud.rs` | Compass strip, mode pill, toggleable extended stats panel, arrival toast, celestial target rings |
| `minimap.rs` | Bottom-right minimap: `ed_world::maprender` tiles, player dot, look-direction rotation, north marker |
| `sonic_boom.rs` | Procedural Mach-1 boom audio, played from `SonicBoom` messages |
| `menu.rs` | Main menu, pause tabs (Map/Settings/Character/Controls), `GameState`, graphics settings, time slider |
| `mapview.rs` | Full-screen slippy map for menu/pause: drag, zoom, pick, place labels, drop popup |
| `spawn.rs` | `SpawnRequest` / `Spawner` / `Arrived`: drop point → ground height → teleport |
| `capture.rs` | `--screenshot` capture after streaming settles |
| `perf.rs` | `--perf` / `EW_PERF=1` CSV logger |
| `render_scale.rs` | 3D-only render percentage via offscreen image + UI camera |
| `lookdev.rs` / `lookdev_ground.rs` | Art-review board stages and shot lists |

---

## Coordinate and rendering invariants

- Truth is **f64 ECEF**; render is **f32 local ENU** (`x=East, y=Up, z=−North`).
- Never do far-from-origin math in f32. Static things store `GeoAnchored` ECEF
  placements; moving things use `Rebased` and are carried across origin moves.
- Renderer uses reverse-Z infinite projection; camera far plane is huge on purpose.
- Physics uses Avian 0.8; the player is a capsule with locked rotation and a PD
  velocity controller.
- Terrain LOD can render coarser than the `TerrainGrid` used by buildings; check
  this when debugging gaps between ground and foundations.

---

## World generation pipeline (per tile)

```
tile id ─┬─> elevation tile ──> TerrainGrid
         └─> vector tile ─────> SurfaceMap (land cover)
                                   │
   biome (climate) + region (architecture/street kit) from lat/lon/elevation
                                   │
   terrain | water | roads | buildings | foliage | props
```

Typical z14 tile: ~0.3–1 s on one core, 400k–800k triangles. Tiles are merged
into a handful of meshes per category.

---

## Map rendering (menu map + minimap)

`ed_world::maprender` renders 256×256 RGBA map tiles from the game's own data:

- hillshade/hypsometric tint from elevation
- landcover/landuse/park/water polygons
- building footprints
- transportation lines (casing + fill, by class)
- admin boundaries
- place labels (`Label` + `PlaceClass`)

`maprender::render(id, vt, heights)` renders at native coordinates.
`maprender::render_at(id, vt, src, heights)` renders a sub-rect of a parent
tile and is used for **overzoom** past the vector source limit (z14).
`fetch_and_render` now overzooms automatically, so z15–16 menu/minimap tiles
still show roads, buildings and water.

The **in-game minimap** (`ed_game/src/minimap.rs`):

- 220 px rounded panel, bottom-right, `ZIndex(5)`, hidden outside `Playing`.
- Fixed map zoom `z16`, tiles displayed 1:1 at 256 px.
- Centers on the `Camera3d`; heading comes from the camera forward projected
  onto local up.
- Whole tile container is rotated by `−heading` so forward is up.
- Orange dot at centre, white forward needle, “N” marker on the rim.
- Async tile workers via crossbeam channel; at most 6 in flight, 96 cached.
- Tunables: `SIZE_PX`, `TILE_PX`, `ZOOM`, `MAX_IN_FLIGHT`, `MAX_CACHED`.

---

## Data sources and cache

| Data | Source |
|---|---|
| Elevation | Mapterhorn (Copernicus GLO-30 + national lidar), AWS Terrain fallback |
| Vector map | OpenFreeMap, OpenMapTiles schema |
| Weather | Open-Meteo |
| Geocoding | Open-Meteo |
| Moon/planets | JPL Keplerian elements + lunar almanac, computed locally |

All network access goes through `ed_data` and is cached on disk. Tiles are
immutable per planet build. Do not read `target/`, `perf*.csv`, or `*.png`
artifacts as source.

---

## Testing and verification

- Unit tests live beside the code. Engine-free crates are the best place for
  logic tests.
- `cargo test --workspace` must stay fast and offline.
- `#[ignore]` tests touch the network (`cargo test -- --ignored`).
- Windows: the same `cargo test --workspace` (CI runs macOS + Windows on every
  push). The `.sh` batteries are macOS-only; PowerShell versions of the
  screenshot smoke and six-scene perf battery live in `docs/WINDOWS.md`.
- Visual checks: `tools/shoot_places.sh OUT_DIR` (Central Park, Zermatt, Aix,
  Marrakech, Shibuya, Manhattan); `--lookdev` boards.
- `--screenshot` may produce a black image in some two-camera/render-scale
  configurations; when debugging UI, capture the live window instead.

---

## Conventions for agents

- Engine-free logic belongs in the engine-free crates with unit tests; Bevy
  glue only in `ed_game`.
- Art/layout choices stay **data tables** (`archkit.rs`, `style.rs`), not
  branches.
- Follow existing API patterns; avoid adding comments unless the surrounding
  code uses them.
- Keep edits scoped: parallel sessions work on trees, ground, clouds and
  buildings.
- Commit trailer when asked: `Co-Authored-By: Claude`.
- Do not commit or push unless the user asks.

---

## Current status / known issues

- Status lives in `docs/ROADMAP.md` and `docs/CHANGES.md`; read those first.
  M0/M1 perf work (tree impostors, tiles as data, `ed_runtime`) is unmeasured
  on the Mac.
- Building roof/foundation overhaul landed (`docs/plans/buildings.md`); needs
  in-game visual checks. Threshold constants (350/500 m², 4 floors, 14 m span)
  live in `roof_plan`.
- Open question: does terrain render at a coarser LOD than the building's
  `TerrainGrid`? Foundations currently have a 0.5 m margin.
- Active uncommitted work often includes perf logger, render scale, streaming,
  camera and menu tweaks; always check `git status` first.
- Roadmap next: Stage A (bugs, Mac test pass, GPU measurement, perf gate,
  determinism CI), then Stage B (finish M1, M2 `ed_gpu` + `ed_render`). Instancing,
  CDLOD and the other Phase 2 items arrive with M2. See `docs/ROADMAP.md`.

## Native Metal renderer (default on macOS)
`ed_game::native` mirrors Bevy `Mesh3d` entities into `ed_metal::world::WorldRenderer` (own CAMetalLayer under a transparent Bevy window). `EW_NATIVE=0` returns to the Bevy renderer. Headless check: `./target/release/earthdrop --headless --fly --lat .. --lon .. --alt .. --heading .. --pitch .. --hour 15 --width 1600 --height 900 --frames N --no-wait --screenshot out.png`, filter log with `grep "native: [0-9]* fps"`. Switches: `EW_NATIVE_NOSSR/NOBOUNCE/NOAO/NOBLOOM/NOGROUND/NOWATER/NOFFT`, `EW_HOLD_SPACE=1`, `EW_NATIVE_DBG`. Full state, switches (incl. `EW_CONTACT`, `EW_PROBE_GI`, `EW_NATIVE_SCALE`, `EW_NATIVE_NOAA`, `EW_WEATHER`) and prioritised open work: `docs/HANDOFF.md`; water design, status and the not-yet-built phases 6 (CPU wave model/buoyancy) and 7 (underwater): `docs/plans/ultra-water-port.md`; per-change detail in `docs/CHANGES.md`.
