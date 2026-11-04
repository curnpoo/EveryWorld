# EveryWorld (binary: `earthdrop`)

Drop anywhere on real Earth; walk, drive, fly, rocket to the Moon. Real data (OSM vector tiles, elevation, weather) decides *what* is there; procedural rules decide *how it looks*. Rust 2024 + Bevy 0.20 + Avian 0.8, Metal on macOS.
Deeper docs: `docs/ARCHITECTURE.md` (design), `docs/PLAN-engine.md` (asset library / agents / vibes direction), `docs/PLAN-perf.md` (meshlets), `docs/PLAN-native-engine.md` (target engine: own RHI, 120 fps tiers, multiplayer, space), `docs/ROADMAP.md` (phases), `docs/claude-handoff/*.md` (session handoffs), `docs/lookdev/round2/*` (art review notes).

## Read this first (saves tokens)
- Do NOT read whole big files. Use `grep -n '^\(pub \)\?fn\|^struct\|^enum'` on a file, then read ranges. Biggest: `ed_world/src/trees/mod.rs` (1.8k), `voice.rs` (1.4k), `buildings.rs` (1.4k), `archkit.rs` (1.3k), `ground_scatter.rs` (1k).
- Ignore `ed_engine_audio` unless the task is audio (a self-contained port of `vendor/engine-sound-kit`, JS is the source of truth).
- `target/`, `perf*.csv`, `*.png` are artifacts; never read them.

## Commands
```bash
cargo run --release -- --place "Zermatt"                  # play
cargo run --release -- --lat 43.53 --lon 5.45 --alt 3000  # coords
cargo run --release -- --fly --lat .. --lon .. --alt .. --heading .. --pitch .. --hour 15 --width 1280 --height 720 --frames 60 --screenshot out.png   # headless-ish shot
cargo run --release -- --lookdev out_dir [--lookdev-only name]   # art-direction boards
cargo run --release -- --perf                              # writes perf.csv (or EW_PERF=1)
cargo test --workspace          # fast, offline
cargo test -- --ignored         # hits real network
cargo check --offline -p ed_game
tools/shoot_places.sh OUT_DIR   # 6 reference scenes (Central Park, Zermatt, Aix, Marrakech, Shibuya, Manhattan)
```
Cache dir `.earthdrop-cache/` (tiles cached forever; gitignored). Dev profile: deps opt-level 3, own code 1.

## Crates (dependency order; all but ed_game are engine-free, unit-tested, thread-safe)
| Crate | Role |
|---|---|
| `ed_geo` | f64 WGS84/ECEF/ENU math (`lib.rs`), slippy tiles `TileId` (`tile.rs`), NOAA sun (`sun.rs`) |
| `ed_data` | HTTP+disk cache (`http`,`cache`), Terrarium elevation (`elevation`), hand-written MVT decoder (`mvt`), OpenFreeMap tiles (`vector`), Open-Meteo weather/geocode |
| `ed_world` | **World generator**: tile -> meshes. Details below |
| `ed_flight` | atmosphere, drag, Mach, gravity, rocket "superpower" model |
| `ed_space` | Sun/Moon/planet ephemeris, body catalog |
| `ed_engine_audio` | Rust port of engine-sound-kit (voice, mixer, cars.json). Parity tests vs JS renders |
| `ed_runtime` | M1 runtime services: `jobs` (keyed pools, priority, cancel), `budget`, `residency` (LRU), `view` (height-aware cone test), `scheduler` (build priority, hole-free retire) |
| `ed_game` | Bevy app (`earthdrop`) |

## ed_world modules (`crates/ed_world/src`)
- `tilegen.rs` **entry point**: per tile fetch elevation+MVT, build terrain, surface, water, roads, buildings (prepare -> LOD + per-chunk detail), trees, props. Worker-thread, outputs plain data. `DetailChunks` holds `BuildingSpec`s per 8x8 chunk for on-demand detail.
- `terrain.rs` `TerrainGrid` = single source of ground height (`sample(u,v)`, same triangles as render mesh + collider; quads split on 00-11 diagonal). Mesh subdivided x3 at z14 for splat.
- `project.rs` `TileProjector`: tile uv + height -> local ENU f32 (`local(u,v,h)`, `up()` is curved-Earth up at tile center).
- `buildings.rs` footprints -> `BuildingSpec` (`prepare`/`prepare_one`) -> `build_lod` (cheap massing + textured windows + simple roof + physics collider) and `build_detail` (`detail_one`: facade, plinth/cornice sweeps, roofs `flat_roof`/`pitched`/`mansard`, `lot` for houses).
- `archkit.rs` per-`Region` `Variant` tables (wall/roof palettes, `roof_forms`, `pitch`, `overhang`, floor heights, ground-floor type). `style.rs` `Region::locate` + street kits. Data tables, not code paths.
- `meshkit.rs` modeling toolkit (`sweep`, `inset`, `polygon`, `wall_with_openings`, `Basis`, `signed_area`). `bparts.rs` parts (windows, doors, balconies, chimneys...; `Mat` buckets, `Out`). `mesh.rs` `MeshData`.
- `library.rs` **asset library**: `AssetDef` (key, `RenderPath`, `ColliderShape`, generator) + `Placement`. Tiles emit `TileBuild::placements` instead of baking discrete objects; `library::get()` is global (content packs `install` before first tile). Street props are the first assets (`props::register_street_props`).
- `roads.rs`, `water.rs`, `surface.rs` (land-cover raster `SurfaceMap`, `Cover`), `biome.rs`, `foliage.rs`+`trees/` (procedural trees), `props.rs` (rocks, street furniture), `ground_scatter.rs`+`ground_tex.rs`+`noise.rs` (near-camera cover, procedural textures), `clouds.rs` (engine-free cloud volumes), `lod.rs` (terrain quadtree), `maprender.rs` (2D menu map), `missions.rs`.

## ed_game modules (`crates/ed_game/src`) — each is a Bevy Plugin, wired in `main.rs`
- Core: `library` (cooks every library asset once at PreStartup -> `CookedLibrary`; `spawn_placements` instances them as meshlets + static colliders), `origin` (floating origin, re-anchors at >8 km), `config` (CLI), `assets` (shared materials/textures), `building_materials` (one material per `bparts::Mat`), `meshconv` (MeshData -> bevy Mesh).
- World: `streaming` (quadtree tile set, camera-cone culling, worker builds, hole-free swaps, building detail chunk streaming), `ground_material`, `ground_scatter`, `foliage`, `sky` (sun, atmosphere, moon/planets, ocean, fog), `clouds` (+`clouds.wesl`, `shaders/`).
- Player/vehicles: `player` (capsule + PD controller), `body` (procedural robot, IK), `camera`, `vehicles` (cars/planes), `engine_audio`.
- UI/flow: `menu`, `mapview`, `spawn`, `hud`, `capture` (`--screenshot`), `perf` (`--perf`), `render_scale`, `lookdev`+`lookdev_ground` (art boards; builds buildings on a FLAT grid, so it can't show terrain bugs).
- Modes: `--fly` skips player/vehicles; `--lookdev` replaces streaming.

## Rendering / meshlets
- Bevy meshlets (virtual geometry) are OFF by default (opt in `EW_MESHLET=1`; measured ~60% slower on M3 Pro). `MeshletPlugin` stays registered in `main.rs`. Meshlet meshes take position/normal/UV only: colours go through `MeshData::palettize()` + `WorldMaterials::palette_matte` (512x512 palette LUT). MSAA must stay off on every camera.
- `vendor/bevy_pbr` is a patched copy of bevy_pbr 0.20.1 (`[patch.crates-io]`): meshlet prepass `flags`, atmosphere pipeline key, material bind group index 3, motion-vector NaN guard, `EnvironmentMapRefresh` gate on the env-map probe + filter passes. Re-apply if Bevy is bumped.
- Env knobs: `EW_MESHLET=1` (same content as meshlets, A/B), `EW_POST=bloom,taa,ssao` (subset of post), `EW_TILE_STATS=1` (per-tile tri counts), `EW_PERF=1` + `EW_PERF_OUT=path`, `EW_APPLY_BUDGET=units` (tile apply per frame, default 250), `EW_RESIDENT_TILES=n` (out-of-view tiles kept, default 192), `EW_IO_THREADS` / `EW_TILE_THREADS` (streaming pools).
- Streaming (`streaming.rs` on `ed_runtime`): select (wanted + in-view) -> IO pool `tilegen::prefetch` -> CPU pool `tilegen::build` + `ready_tile` -> staged apply under a budget (tile hidden until complete) -> retire. Terrain uses ONE shared material: satellite colour lives in `ground_material::SatPool` (3 texture arrays by size class), the layer rides on the terrain entity's `MeshTag`. The atmosphere env map only re-renders when the sun/altitude/cloud changes (`sky::refresh_env_map` -> patched `bevy_pbr` `EnvironmentMapRefresh`).
- Screenshots/perf runs need the window focused on macOS (an unfocused/occluded window renders black).

## Key invariants
- Truth is f64 ECEF; render is f32 local ENU (x=East, y=Up, z=-North). Never do far-from-origin math in f32.
- Everything on the ground samples the SAME `TerrainGrid`. Nothing may float or sink. Buildings are built against the tile's own grid, but terrain may render at a coarser LOD than the grid used for buildings (check when debugging gaps).
- Generation is deterministic (hash of OSM id/tile/cell, `crate::hash01(seed, salt)`); same world every run.
- OSM `building:colour` always wins over region palette. Heights come from `render_height`/`render_min_height` MVT props.
- Buildings: walls/roofs are authored in a facade-space where y is measured from `base_h` (ground-relative); `gy` = street level inside that space; `wall_top` = eave height.

## Conventions
- Engine-free logic goes in the engine-free crates with unit tests; Bevy glue only in `ed_game`.
- Art/layout tables stay data (archkit/style), not branches.
- Commit trailer: `Co-Authored-By: Claude`. Don't push unless asked.
- Parallel sessions touch different files (trees, ground, clouds, buildings); keep edits scoped.

## Known issues / active work
- Buildings: roof/foundation overhaul landed (see `docs/PLAN-buildings.md` for what and what's left). `buildings.rs`: `roof_plan` (Flat | Mansard | Pitched(rects)) is shared by `detail_one` and `lod_one`; `foundation()` skins walls above low terrain. Needs visual check in-game; thresholds (350/500 m2, 4 floors, 14 m span) are constants in `roof_plan`.
- Open: does terrain render at a coarser LOD than the building's `TerrainGrid`? Foundations have a 0.5 m margin only.
- M0/M1 work on `perf/m0-m1` needs a Mac measurement pass: see `docs/claude-handoff/2026-11-04-m0-m1.md`.
- Roadmap items: instancing, CDLOD, landmarks, interiors, night lighting (see `docs/ROADMAP.md`).
