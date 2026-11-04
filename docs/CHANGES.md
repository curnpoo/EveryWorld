# Changes

Recent work, newest first, and what still needs checking. When an entry is verified on the Mac, fold the result into `ROADMAP.md` and trim it here. Plans live in `docs/plans/`; the roadmap says what's done and what's next.

---

## 2026-11-04: species as data + genomes, tree and rock collision

Plan and next steps: `docs/plans/procedural-flora.md` (step 1 of 6 landed).

| Item | Where |
|---|---|
| `SpeciesDef` table (leaf palettes, far-card budgets, trunk colliders, genome ranges) and `Genome` (+ `sample`, `apply`) | `ed_world/src/trees/species.rs` |
| `build_genome`; `build` = default genome, tested identical | `ed_world/src/trees/mod.rs` |
| Trunk cylinders, one static compound per tree chunk, full-detail tier only | `ed_game/src/foliage.rs` `tree_colliders` |
| Rock trimesh colliders within 90 m, lazy (3 builds/frame) | `ed_game/src/near_chunks.rs` |

Needs checking on the Mac: walk into an oak, a baobab and a rock (should stop, not clip); no frame hitch entering a rock field or dense forest (compound build happens on the chunk swap); walking along a slope does not snag on trunk bases. The genome is not used per instance yet, so the look is unchanged.

---

## 2026-11-04: headless perf harness, shadow proxies, half-res clouds (measured on M3 Pro)

All numbers come from `tools/perf_headless.sh OUT [scene...]` (`--headless`: no window, no focus, no sound; 2144x1206, `--weather fair`, scenes 22-25 s). The stats skip the load phase.

| Item | Where | Notes |
|---|---|---|
| Headless mode | `headless.rs`, `config.rs` (`--headless`, `--cruise`, `--spin`, `--quit-after`) | Renders to an offscreen image. `EW_GPU_WAIT=1` logs the GPU wait per frame. `--screenshot` still quits after the capture, so don't combine it with perf runs. |
| GPU bisect | `perf.rs` `EW_BISECT=hide:cat,noshadow:cat` | Categories: bld, std, terrain, foliage, scatter, cloudsky, cloudshadow, all. Metal has no per-pass timers; this is how cost gets found. |
| Shadow proxies | `streaming.rs` (terrain, buildings), `foliage.rs` (full trees) | Detailed meshes render on `CAMERA_ONLY_LAYER` (6). The sun's shadow pass draws a cheap stand-in on `CLOUD_SHADOW_LAYER` (7). Full trees cast through their card. Cruise went from 17.4 to 34.9 fps (min 7.2 -> 25.2). |
| Half-res clouds | `cloud_half.rs`, `clouds.wesl` (`CLOUD_HALF`), `clouds_up.wesl` | The cloud raymarch runs at half resolution after the sky pass. Each 2x2 block marches against its near or far depth in a checkerboard pattern. The result is composited with a depth-aware 4-tap upsample. The material mesh is hidden; it still feeds the uniform. Still scene: 27.0 -> 24.7 ms. `EW_CLOUD_FULL=1` restores the old full-res path. |
| Vendor patch | `vendor/bevy_pbr/src/atmosphere/mod.rs` | Adds `RenderSkySet` (the sky pass as a system set) so other passes can order after it. |

**Check on Mac (windowed):** cloud edges against mountain silhouettes, with no halo or blockiness. Also check that tree trunk shadows look right on foot, since the proxy card casts in place of the full tree.

**Open (M0 exit not met yet):** still 40 fps; startup/explode spikes of 100-480 ms in cruise and origin. Next: pipeline pre-warm, then smoothing the building-detail explode.

---

## 2026-11-04: real-data ground cover, trees and biomes

Deserts no longer get lawns and dense forests are no longer sparse. Everything is deterministic and works offline (OSM-only fallback); nothing is run on Metal yet.

| Item | Where | Notes |
|---|---|---|
| Climate biomes | `ed_data::climate` (+ `data/climate10.bin`, `tools/bake_climate.py`), `biome.rs` `from_climate` | WorldClim 2.1 normals, lapse-rate corrected. New `Biome::Steppe`. Old lat/lon boxes only where the grid has no data. |
| Land cover | `ed_data::{worldcover, cog}`, `http::get_range`, `SurfaceMap::with_worldcover`, `tilegen.rs` (z >= 8) | WorldCover COG range reads fill only `Natural` cells; OSM polygons win. New `Cover::{Shrub, Bare, Moss}`. |
| Greenness gate | `SurfaceMap::splat`, `greenness` | Where the satellite is not green, grass/natural layers give way to dirt/gravel/sand. |
| Canopy-driven trees | `SurfaceMap::tree_density`, `foliage::scatter` | WorldCover tree/shrub fractions x per-biome max density. Up to 3 plants per 8 m cell (was clamped to <1). Per-tile caps raised 9k/6k/5k/4k -> 16k/10k/7k/5k. |
| Geology | `ed_data::geology` (Macrostrat), `SurfaceMap::lith` | Scattered rocks take the bedrock colour (tiles z >= 10). |
| Per-tree variation | `foliage::vigor`, `scatter`, `trees::instance_rotation` | Size from age x biome/altitude vigour, lean (a few degrees), per-crown tint jitter. Still 3 mesh variants per species. |

**Not done:** canopy comes from WorldCover tree fraction, not ETH/Hansen height (ETH unreachable; Hansen tiles are 53 MB stripped). Sand/scree ground textures are not yet tinted by lithology (shader work). More than 3 mesh variants would grow the impostor atlas.

**Check on Mac** (short runs, window focused): `--place "Zermatt"`, a Sahara drop (`--lat 23.4 --lon 25.6 --alt 800`), Manaus (`--lat -3.1 --lon -60 --alt 600`), Aix, Central Park. Expect: bare sand/gravel and almost no trees in the Sahara; thick canopy at Manaus without holes; no regression in cities. Watch tile build time (first run fetches WorldCover blocks; later runs read the cache) and tree counts per tile with `EW_TILE_STATS=1`.

---

## 2026-11-04: pause-menu map ignores drag/click (fix)

Bevy's `ui_focus_system` stops updating `RelativeCursorPosition` on hidden nodes, so the "Drop in here" popup, hidden while the cursor was on its button, kept `cursor_over = true`. `globe::control` read that as "cursor over popup" and dropped every press, drag and scroll on the map from then on (the first drop from the main menu worked; every pause map after it didn't). Fix: popups (and the time slider track) only count when `InheritedVisibility` is visible. **Check on Mac:** drop in, Esc, then drag, click to pick and scroll the map; drop again from the popup, then do it once more.

## 2026-11-04: perf milestones M0 + M1 (on `main`)

Built and unit-tested in a Linux cloud container with a software GPU. A smoke run renders Zermatt with no shader errors, but **nothing is measured on Metal yet**. The Mac pass below decides whether M0's exit criterion is met (45+ fps steady, no frame > 50 ms after load).

### What changed

**M0, in Bevy**
| Item | Where | Notes |
|---|---|---|
| 2 shadow cascades | `sky.rs` `setup_sky` | 40 m near, 700 m far (was 3). |
| Env map only on change | `sky.rs` `refresh_env_map`, patched `vendor/bevy_pbr` (`EnvironmentMapRefresh`) | The atmosphere probe and env-map filter passes run only when the sun moves > 0.3°, altitude changes > 3% (min 50 m), cloud factor changes, or the probe is re-added. 3 frames per refresh. Was every frame (~2 ms). |
| Shared terrain material | `ground_material.rs` `SatPool`, `shaders/terrain.wesl` `sample_sat` | Satellite colour lives in 3 texture arrays (512 px x 40, 256 x 96, 128 x 192; 76 MB). The layer rides on the terrain entity's `MeshTag`, so all terrain batches under one material. Also fixes ground textures jumping after > 8 km of travel (per-tile material copies missed the floating-origin UV update). |
| Budgeted, staged tile apply | `streaming.rs` `apply_tiles`, `ed_runtime::budget` | Per-frame cost budget (`EW_APPLY_BUDGET`, default 250 units) plus a 4 ms cap. A tile stays hidden until complete, so the coarse tile keeps covering. |
| Tree impostors | `ed_world::trees::impostor`, `impostors.rs`, `shaders/impostor.wesl` | User approved impostors for trees. See below. |

**M1: `ed_runtime` (engine-free, 19 tests) and streaming on it**
| Module | What |
|---|---|
| `jobs` | Keyed thread pools: priority, re-rank while queued, cancel, one entry per key, panics reported. |
| `budget` | `FrameBudget`: cost units + wall clock; the first item always runs. |
| `residency` | `Lru` with a cap. |
| `view` | Cone test using the tile's real height range. Fixes tiles vanishing above ~4 km ground (Altiplano, Tibet) and when looking up at peaks. |
| `scheduler` | Build priority (visible first, prefetch along travel above ~50 m/s) and hole-free retire, linear time. |

Streaming now runs as: IO pool (8 threads, downloads into the disk cache) -> CPU pool (cores - 3, max 8, `tilegen::build` + mesh prep) -> budgeted apply -> retire. Stale queued tiles are cancelled at each select (4 Hz). Out-of-view tiles stay loaded while still in the LOD set (`EW_RESIDENT_TILES`, default 192), so turning around doesn't rebuild the street behind you.

**Tree impostors**
- Atlas: a CPU rasterizer renders all 69 species variants (23 kinds x 3) from 8x8 hemi-octahedral views. 32 px frames, 4x supersampled, albedo + tree-local normal, coverage-preserving mips. About 5 s in release, on a background thread at startup; impostors draw nothing until it lands.
- One quad per tree, one impostor mesh per tile (`impostor::build_mesh`, built on the tile worker).
- Shader: billboards toward the camera, picks the nearest view frame, thins with distance (share shown = (1000 m / d)^2), grows survivors up to 6x so canopy cover holds, fades trees under ~1.5 px. Past 600 m it blends toward the satellite colour under the tree (`TreeInstance::tint`), so far forest matches the photo instead of speckling it. Same vertex code in main pass and prepass. No shadows.
- Trees now exist on z10+ tiles (were z13+). Caps: z14 9,000; z12-13 6,000; z11 5,000; z10 4,000 (`tilegen.rs`).
- LOD chain: impostor -> leaf cards per chunk (z14, within `CARD_RANGE_M` = 800 m) -> full trees (within 230 m). The old thinned card bands are gone.
- Range: a 10 m tree is ~1 px at ~11 km (1280 px, 60° FOV). With growth, forests read as canopy texture to roughly 20-30 km; past that the satellite colour carries them, which is what forests look like from orbit.
- Tuning: `FULL_M`, `GROW_MAX`, `MIN_PX` at the top of `impostors.rs`.

**Tiles stop being entities (M1, Bevy form).** A tile is now ~10 entities whatever its content:
- terrain, water, roads;
- library placements merged into one mesh + one compound collider (`library::merge_placements`; the `EW_MESHLET=1` path is unchanged);
- building LOD merged per material + one trimesh collider (`streaming::merge_lod`). A tile explodes into per-chunk LOD entities only while building detail loads near the player, and merges back after 8 s with nothing near (`streaming::Explode`);
- one impostor entity for all its trees;
- a `TreeChunks` holder: leaf cards and full trees are entities only near the camera (`foliage::stream_tree_chunks`);
- a `NearMeshes` holder: rock chunks are entities only within 500 m (`near_chunks.rs`).

The fully GPU-resident form (no per-tile entities at all) comes with M2's own renderer.

**Fixes found along the way**
- `terrain.wesl` used `class` as a variable name (reserved in WGSL), so the terrain shader failed at runtime. Caught by the software-GPU smoke run.

### Measured in the cloud box (software GPU: counts only, fps meaningless)
Same camera, old build vs new:

| Scene | Entities | Visible meshes | Load to idle |
|---|---|---|---|
| Zermatt village, 120 m AGL | 16,503 -> 2,166 | 4,233 -> 890 | 178 s -> 73 s |
| Zermatt, 4 km | 12,892 -> 1,334 | 2,188 -> 117 | 146 s -> 50 s |
| Black Forest, 3 km | 7,397 -> 1,247 | 96 -> 108 | 82 s -> 26 s |

Screenshots match the old build, plus more distant trees. They were taken at 640x360 with a software renderer, so they look soft; that's the capture setup, not the game.

### Not done, on purpose
- **Pipeline pre-warm** (M0). Likely cause of the 3-6 s settings stall: the AO and TAA toggles (`menu.rs` `apply_graphics`) add/remove camera components, which changes every material's pipeline key. Pre-warming needs a second hidden camera with the other key (21 systems assume one `Camera3d`) or a `bevy_pbr` patch that keeps SSAO bound and skips its dispatch. Confirm the cause first (check 4).
- **Clouds ½ res** (M0). Measured 33.3 vs 33.5 fps with clouds off: no gain while vertex bound. Revisit after M2.

### Mac test pass (about 20 minutes)
Use Zermatt plus the plan's Ohio suburb at 67% render scale, and compare against `c42ed5e` (before this work) at the same spot.

```bash
EW_PERF=1 EW_PERF_OUT=perf_m0.csv cargo run --release -- --place "Zermatt"
```

1. **Steady fps**: stand still 30 s after load; `fps` and `p99_ms` in the CSV. Target 45+.
2. **Streaming hitches**: walk/fly 60 s; `max_ms` under 50 after load. Spikes: lower `EW_APPLY_BUDGET` (try 120). Slow loading: raise it (try 500). `io_jobs`, `build_jobs`, `applying` show where tiles wait. `entities` should stay roughly flat.
3. **Turn around** at street level: tiles behind you don't reload (F3: `tiles N loaded` flat, `pending` ~0).
4. **Settings stall**: toggle only AO, then only Bloom, then only Shadows. If only AO (and TAA) stall, the pipeline-key theory holds.
5. **Forests**: fly over Zermatt and the Black Forest (`--lat 48.0 --lon 8.2`) at 300 m, 3 km, 10 km AGL. Trees fade into the satellite colour without popping or bald hills. No ring at 650-950 m where trees vanish or double in size (card/impostor seam). Impostors appear a few seconds after start (atlas bake).
6. **Buildings**: walk into town; detail swaps in, no flicker when a tile explodes or merges back.
7. **Physics**: drive into a street light and a building on a freshly loaded tile (merged colliders).
8. **Visuals from M0**: satellite colour on distant terrain and water as before (F3 shows `satellite layers free a / b / c`; if one hits 0, raise it in `SAT_CLASSES`). Ambient light follows time scrubbing (`[` `]`), no black ambient after spawn or the pause menu. Near shadows still crisp with 2 cascades.
9. **High ground**: La Paz (`--lat -16.5 --lon -68.15`) and the Altiplano above 4 km; look level and up at peaks. No missing tiles.
10. **Floating origin**: fly > 8 km in one direction; ground textures don't jump.

If something looks wrong, the pieces are separate commits (`dfa506c`, `0598c84`, `fd3d90b`, `1b73019`, `4a27765`, `0425426`) and can be reverted one by one.

---

## 2026-11-03: tile culling and space targets

- `streaming.rs`: the desired tile set is filtered against the camera's view cone before builds start. Superseded on Nov 4 by `ed_runtime::view` (height-aware) and the residency cap.
- `sky.rs`, `hud.rs`: celestial body meshes and their HUD rings/labels appear only above 100 km altitude.
