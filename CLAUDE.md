# EveryWorld (alpha binary: `earthdrop`)

A persistent solarpunk universe on a twin of the real Earth, on a custom Rust engine with
its own editor. Real data decides what is there; authored assets and rules decide how it
looks.

## Direction (read before planning any work)

`main` is the custom-engine rebuild. The Bevy alpha is frozen on branch `alpha-early`.
**Current phase: 0 — Prepare** (`docs/phases/phase-0.md`). Read in this order:
`docs/MASTER.md` (overview and document map) → `docs/VISION.md` (goals) →
`docs/DECISIONS.md` (locked decisions, by id) → `docs/ARCHITECTURE.md` → `docs/ROADMAP.md`
→ the brief in `docs/plans/` for the system you are touching.

Rules for every session:

1. **Custom engine only (D1, D9).** No Bevy, no wgpu, no engine swap. No feature or visual
   work on the alpha; crash fixes only, on `alpha-early`.
2. **One address (F1).** Everything above `ed_data` keys on `NodeId`. Slippy `TileId` stays
   inside `ed_data`. Objects have stable `ObjectId`s.
3. **Far field is designed in (F2).** Generators emit per-rung output (parts, shells, node
   proxy, terrain-baked layers, imagery). Never merged per-tile meshes. Any rung must be
   producible straight from source data.
4. **Chunks, not ad-hoc loading (F4).** Content reaches the game as `.ewpack` chunks keyed
   by `NodeId`. A parent node draws until its children are resident.
5. **Authored content is override layers (F5, F6).** Never edit generated output. The
   editor is a mode of `ew`; assets come from Blender as glTF.
6. **Renderer (F3, F7, F9).** New GPU code targets `ed_gpu` only. Drawing is GPU-driven
   through the geometry-provider interface; discrete LOD is the base, cluster geometry is
   the parallel Track G. Baseline lighting always works; VSM and voxel GI are the High and
   Ultra targets (D14). Water is a from-scratch design (`docs/plans/water.md`).
7. **Deterministic generation.** A pure function of (seed, pinned data version, node, rung).
   No map-iteration order, thread-order float reductions, unseeded randomness, wall-clock
   reads or platform-dependent hashing in generators. Geometry is never sent over the
   network.
8. **Sim is separate.** `ed_sim` and `ed_server` never depend on GPU, render, animation or
   editor crates. Fixed step; the sim never reads the wall clock.
9. **Measure first.** Changes that claim a performance or loading gain record before and
   after numbers in `docs/CHANGES.md`. Every rendering feature has a cost per tier and an
   off switch.
10. **Data licensing.** Registry entry before use; RED never ships (`docs/data/README.md`).
11. **Docs.** Decisions change only in `docs/DECISIONS.md`. Plans carry gates, not dates
    (D18). Add a `docs/CHANGES.md` entry when work lands. Do not commit or push unless asked.

---

# Alpha codebase notes

Everything below describes the alpha code: the engine-free crates that carry over (`ed_geo`,
`ed_data`, `ed_world`, `ed_flight`, `ed_space`, `ed_runtime`, audio) and the Bevy app that
does not. Use it to find and read existing code. It is not direction for new work; what
happens to each piece is in `docs/plans/rearchitecture.md` §4. Stage and milestone names
below (Stages A–H, M0–M10) are the alpha's.

## Read this first (saves tokens)
- Do NOT read whole big files. Use `grep -n '^\(pub \)\?fn\|^struct\|^enum'` on a file, then read ranges. Biggest: `ed_world/src/trees/mod.rs` (1.8k), `voice.rs` (1.4k), `buildings.rs` (1.4k), `archkit.rs` (1.3k), `ground_scatter.rs` (1k).
- Ignore `ed_engine_audio` unless the task is audio (a self-contained port of `vendor/engine-sound-kit`, JS is the source of truth).
- `target/`, `perf*.csv`, `*.png` are artifacts; never read them.

## Commands
```bash
cargo run --release                                      # play (choose drop-in from the UI)
cargo run --release -- --lat 43.53 --lon 5.45 --alt 3000  # coords
cargo run --release -- --fly --lat .. --lon .. --alt .. --heading .. --pitch .. --hour 15 --width 1280 --height 720 --frames 60 --screenshot out.png   # headless-ish shot
cargo run --release -- --lookdev out_dir [--lookdev-only name]   # art-direction boards
cargo run --release -- --perf                              # writes perf.csv (or EW_PERF=1)
cargo test --workspace          # fast, offline
cargo test -- --ignored         # hits real network
cargo check --offline -p ed_game
tools/shoot_places.sh OUT_DIR   # 6 reference scenes (Central Park, Zermatt, Aix, Marrakech, Shibuya, Manhattan)
```
Windows (PowerShell): same `cargo` commands; setup, the screenshot/perf
batteries and the agent checklist are in `docs/alpha/WINDOWS.md`. Set
`$env:RUST_LOG = "info,wgpu=error,naga=warn,bevy_render=warn"` to see the
`GPU adapter` line (Dx12 or Vulkan) and other info logs.
Cache dir `.earthdrop-cache/` (tiles cached forever; gitignored). Dev profile: deps opt-level 3, own code 1.

## Crates (dependency order; all but ed_game are engine-free, unit-tested, thread-safe)
| Crate | Role |
|---|---|
| `ed_geo` | f64 WGS84/ECEF/ENU math (`lib.rs`), slippy tiles `TileId` (`tile.rs`), NOAA sun (`sun.rs`) |
| `ed_data` | HTTP+disk cache (`http`,`cache`), Terrarium elevation (`elevation`), hand-written MVT decoder (`mvt`), OpenFreeMap tiles (`vector`), Open-Meteo weather/geocode; real-data layers: `climate` (embedded WorldClim normals, `tools/bake_climate.py`), `worldcover`+`cog` (ESA land cover range reads), `geology` (Macrostrat lithology) |
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
- `roads/` (v2, see `docs/alpha/plans/roads.md`: `net` world-space network from 3x3 tiles, `solve` height solver, `plan2`, `structures` decks/tunnels, `junction`, `width`, `median`, `keepout`, `nav` graph; `mod.rs` keeps the legacy planner behind `EW_ROADS=legacy`), `garage.rs`, `water.rs`, `surface.rs` (land-cover raster `SurfaceMap`, `Cover`; WorldCover canopy/shrub rasters and lithology drive plant density and colour), `biome.rs` (`Biome::from_climate`, real climate with heuristic fallback), `foliage.rs`+`trees/` (procedural trees; `trees/species.rs` = `SpeciesDef` table + `Genome`: add species as rows, don't hard-code palettes/budgets in `mod.rs`; `trees/impostor.rs` CPU-bakes the impostor atlas and builds per-tile impostor quads), `props.rs` (rocks, street furniture), `ground_scatter.rs`+`ground_tex.rs`+`noise.rs` (near-camera cover, procedural textures), `clouds.rs` (engine-free cloud volumes), `horizon.rs` (terrain horizon-map bake for far shadows, not yet wired), `lod.rs` (terrain quadtree), `maprender.rs` (2D menu map), `missions.rs`.

## ed_game modules (`crates/ed_game/src`) — each is a Bevy Plugin, wired in `main.rs`
- Core: `library` (cooks every library asset once at PreStartup -> `CookedLibrary`; `spawn_placements` instances them as meshlets + static colliders), `origin` (floating origin, re-anchors at >8 km), `config` (CLI), `assets` (shared materials/textures), `building_materials` (one material per `bparts::Mat`), `meshconv` (MeshData -> bevy Mesh).
- World: `streaming` (quadtree tile set, camera-cone culling, worker builds, hole-free swaps, building detail chunk streaming with LOD explode/implode), `near_chunks` (rock chunks as data, entities only near the camera), `ground_material`, `ground_scatter`, `foliage` (cards + full trees near the camera), `impostors` (tree impostor material, atlas bake at startup, `shaders/impostor.wesl`), `sky` (sun, atmosphere, moon/planets, ocean, fog), `shadows` (sun cascades re-planned from camera AGL via `ed_runtime::shadows`), `clouds` (+`clouds.wesl`, `shaders/`).
- Player/vehicles: `player` (capsule + PD controller), `body` (procedural robot, IK), `camera`, `vehicles` (cars/planes), `engine_audio`.
- UI/flow: `menu`, `mapview`, `spawn`, `hud`, `capture` (`--screenshot`), `perf` (`--perf`), `render_scale`, `lookdev`+`lookdev_ground` (art boards; builds buildings on a FLAT grid, so it can't show terrain bugs).
- Modes: `--fly` skips player/vehicles; `--lookdev` replaces streaming.

## Rendering / meshlets
- Bevy meshlets (virtual geometry) are OFF by default (opt in `EW_MESHLET=1`; measured ~60% slower on M3 Pro). `MeshletPlugin` stays registered in `main.rs`. Meshlet meshes take position/normal/UV only: colours go through `MeshData::palettize()` + `WorldMaterials::palette_matte` (512x512 palette LUT). MSAA must stay off on every camera.
- `vendor/bevy_pbr` is a patched copy of bevy_pbr 0.20.1 (`[patch.crates-io]`): meshlet prepass `flags`, atmosphere pipeline key, material bind group index 3, motion-vector NaN guard, `EnvironmentMapRefresh` gate on the env-map probe + filter passes, `render/shadows.wesl` last-cascade far fade (`FAR_FADE_START`, = `ed_runtime::shadows::FADE_START`). Re-apply if Bevy is bumped.
- Env knobs: `EW_MESHLET=1` (same content as meshlets, A/B), `EW_POST=bloom,taa,ssao` (subset of post), `EW_TILE_STATS=1` (per-tile tri counts), `EW_PERF=1` + `EW_PERF_OUT=path`, `EW_APPLY_BUDGET=units` (tile apply per frame, default 250), `EW_RESIDENT_TILES=n` (out-of-view tiles kept, default 192), `EW_IO_THREADS` / `EW_TILE_THREADS` (streaming pools), `EW_SHADOWS=legacy` (old 2-cascade 700 m config, A/B), `EW_FAR_SHADOWS` (far terrain shadows, on by default; `=0` off, `=debug` tints far-shadowed ground magenta: horizon maps + canopy + surface march; `ed_world::farshadow`, `ed_game::far_shadows`, terrain bindings 110/111, far layer in `MeshTag` bits 12-23).
- Roads/buildings knobs: `EW_ROADS=legacy`, `EW_OSMTAGS=0` (baked OSM tags from `tools/bake_osm_tags.py`), `EW_INFER_BRIDGES=0`, `EW_ROADZ=0` (IGN/NVDB heights), `EW_OVERTURE=0` (baked Overture buildings from `tools/bake_overture.py`), `EW_ROAD_TALLY=1` (road triangles by kind, stage times), `EW_TILE_STATS=1` also prints one `ROADS v2` line per tile.
- Streaming (`streaming.rs` on `ed_runtime`): select (wanted + in-view) -> IO pool `tilegen::prefetch` -> CPU pool `tilegen::build` + `ready_tile` -> staged apply under a budget (tile hidden until complete) -> retire. Terrain uses ONE shared material: satellite colour lives in `ground_material::SatPool` (3 texture arrays by size class), the layer rides on the terrain entity's `MeshTag`. The atmosphere env map only re-renders when the sun/altitude/cloud changes (`sky::refresh_env_map` -> patched `bevy_pbr` `EnvironmentMapRefresh`).
- Tiles are data, not entity trees: a tile is ~10 entities (terrain, water, roads, merged props + one compound collider, merged LOD buildings per material + one collider, one impostor mesh for all its trees, a `TreeChunks` and a `NearMeshes` holder). Leaf-card chunks, full trees and rocks become entities only near the camera; building LOD explodes into per-chunk entities only on tiles where detail is loading (`streaming::Explode`).
- Trees, far to near: impostor quad (z10+ tiles, hemi-octahedral atlas `ed_world::trees::impostor`, shader thins by distance and grows survivors, fades below ~1.5 px) -> merged leaf cards per chunk (z14, < `CARD_RANGE_M` 800 m) -> full trees (< 230 m). Tuning: `impostors.rs` `FULL_M`, `GROW_MAX`, `MIN_PX`; tree caps per zoom in `tilegen.rs`.
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
- Buildings: roof/foundation overhaul landed (see `docs/alpha/plans/buildings.md` for what and what's left). `buildings.rs`: `roof_plan` (Flat | Mansard | Pitched(rects)) is shared by `detail_one` and `lod_one`; `foundation()` skins walls above low terrain. Needs visual check in-game; thresholds (350/500 m2, 4 floors, 14 m span) are constants in `roof_plan`.
- Open: does terrain render at a coarser LOD than the building's `TerrainGrid`? Foundations have a 0.5 m margin only.
- M0/M1 (env map, cascades, shared terrain material, budgeted apply, `ed_runtime`, tree impostors, tiles as data) is on `main` but unmeasured on the Mac: test pass in `docs/CHANGES.md`. Clouds ½ res + shadow proxies done (headless numbers in `docs/CHANGES.md`). Open: pipeline pre-warm, explode hitches. These are alpha-only; the measurement that matters now is the Phase 0 alpha baseline (`docs/phases/phase-0.md` W2).

## Flora and collision (read before adding species or plant variety)
- Species are rows in `trees/species.rs` `SPECIES` (same order as `TreeKind`, tested). Per-tree variety goes through `Genome` (`Genome::sample` -> `build_genome`), never new hard-coded variants. Roadmap of the remaining steps (niche selection, per-instance genomes, lazy banks, ground cover/rocks on the same system): `docs/alpha/plans/procedural-flora.md`.
- Climate/biome/land cover/geology are static real data (`ed_data`), separate from the future procedural weather sim (`ed_climate`); weather modulates, never decides what grows.
- Collision: trunk cylinders (one static compound per tree chunk, only the full-detail tier) and rock trimeshes (within 90 m, `near_chunks.rs`). Cards and impostors never collide.

## Native Metal renderer (alpha; the look reference for the new renderer)
`ed_game::native` mirrors Bevy `Mesh3d` entities into `ed_metal::world::WorldRenderer` (own CAMetalLayer under a transparent Bevy window). `EW_NATIVE=0` returns to the Bevy renderer. Headless check: `./target/release/earthdrop --headless --fly --lat .. --lon .. --alt .. --heading .. --pitch .. --hour 15 --width 1600 --height 900 --frames N --no-wait --screenshot out.png`, filter log with `grep "native: [0-9]* fps"`. Switches: `EW_NATIVE_NOSSR/NOBOUNCE/NOAO/NOBLOOM/NOGROUND/NOWATER/NOFFT`, `EW_HOLD_SPACE=1`, `EW_NATIVE_DBG`. Full state, switches (incl. `EW_CONTACT`, `EW_PROBE_GI`, `EW_NATIVE_SCALE`, `EW_NATIVE_NOAA`, `EW_WEATHER`) and prioritised open work: `docs/alpha/HANDOFF.md`; water: the new design brief is `docs/plans/water.md` (the alpha `ultra-water-port.md` is a superseded post-mortem); per-change detail in `docs/CHANGES.md`.
