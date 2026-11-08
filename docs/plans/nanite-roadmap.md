# EveryWorld — Architecture, Research, and Nanite Roadmap

Saved from session on 2026-11-08. This is the full analysis as discussed; no code changes are made by this document.

---

## 1. What's actually wrong (from the perf data)

- **Rebase hitches every ~7 s at 2500 mph.** Origin rebase moves all `GeoAnchored` entities; Avian re-syncs their colliders. Terrain colliders are the culprit (Phase 1 removed them → hitches 7→1; restoring them brings hitches back).
- **Pending backlog never drains at speed.** Tilegen (IO + CPU) can't keep up with traversal. Tiles arrive after you've passed them.
- **Pop-in.** Instant LOD swap (far massing → full buildings) and instant tile reveal. No geomorph.
- **Flat cities from far.** Coarse tiles (z≤12) have no buildings; massing on z≥13 is flat slabs. `tile_is_dense` misclassified NYC as suburb (1-2 floors) until the threshold was lowered.
- **Loading bar stalls at 75%.** `desired` set (20 km ring) never fully loads (backlog).
- **Only loads in front.** View-cone culling gates off-screen tiles behind a 2-tile prebuild ring.
- **Impostors persist near camera.** Fixed this session.

---

## 2. Full architecture (top to bottom)

### 2.1 Crate stack

```
ed_geo        → f64 WGS84/ECEF/ENU, TileId, sun (NOAA)
ed_data       → HTTP+disk cache, elevation (Mapterhorn/AWS), MVT decoder, vector (OpenFreeMap), weather, geocode
ed_world      → world generator: tilegen, terrain, buildings, roads, water, surface, biome, foliage, trees, props, meshkit, archkit/style, maprender, missions
ed_flight     → atmosphere (ISA), drag, Mach, SuperRocket
ed_space      → ephemeris (Sun/Moon/planets)
ed_engine_audio → engine sound kit (ignore unless audio)
ed_runtime    → job pools, frame budget, LRU, view cone, scheduler
ed_game       → Bevy 0.20 + Avian 0.8 app (binary: everyworld)
ed_metal      → native Metal renderer (default on macOS)
ed_gpu/ed_render → (future) target renderer
```

Dependency order: geo → data → world → (flight, space, audio, runtime) → game. Everything except `ed_game` is engine-free and unit-tested.

### 2.2 Coordinate system

- Truth is **f64 ECEF**. Render is **f32 local ENU** (x=East, y=Up, z=−North).
- Never do far-from-origin math in f32. Static things store `GeoAnchored` (ECEF placement). Moving things use `Rebased` and are carried across origin moves.
- Renderer uses reverse-Z infinite projection; camera far plane huge.
- Floating origin: rebase when camera > 8 km from origin (`REBASE_DISTANCE_M`). `apply_anchors` re-positions all `GeoAnchored` entities. Avian re-syncs their colliders.

### 2.3 Rendering (two paths)

- **Bevy (wgpu)**: default plugins, custom WESL shaders, ground splat material, sky/weather, ocean, clouds.
- **Native Metal (`ed_metal`)**: transparent Bevy window + own `CAMetalLayer`. Mirrors Bevy `Mesh3d` entities into `WorldRenderer`. Has own shadow cascades (8), SSR, AO, bloom, water, clouds, ocean FFT, post. Default on macOS (`EW_NATIVE=0` to disable).
- Native is retained-mode: GPU scene persists across frames, sparsely updated. Single large mesh buffer. Per-view GPU cull, then rasterize. (This is *already* moving toward the Nanite model.)

### 2.4 World generation (per tile)

```
tile id ─┬─> elevation tile ──> TerrainGrid (single source of truth for ground height)
         └─> vector tile ─────> SurfaceMap (land cover)
                                   │
   biome (climate) + region (architecture/street kit) from lat/lon/elevation
                                   │
   terrain | water | roads | buildings | foliage | props
```

- z14 tile: ~0.3–1 s on one core, 400k–800k triangles. Merged into ~10 entities per tile.
- Terrain LOD can render coarser than the `TerrainGrid` buildings use (foundation margin 0.5 m).
- Everything deterministic from OSM id / tile / cell via `hash01`. OSM `building:colour` always wins over region palettes.

### 2.5 Streaming (`streaming.rs` + `ed_runtime`)

- `TileManager`: IO pool (crossbeam), CPU pool (build threads), apply budget (300 units/frame, 8 ms).
- Stages per tile: `Base` (terrain, water, roads, rocks, props, collider, nav, ground scatter, sat tex) → `Extras` (placements, meshlets) → `Buildings` (LOD, colliders) → `Trees` (impostors, chunks) → `Finish` (reveal, register in `loaded`).
- Each tile stays `Visibility::Hidden` until `Finish`, so the coarser tile it replaces keeps covering the area.
- `select_tiles` (4 Hz): `lod::select` (quadtree) → `wanted` set; view cone → `in_view`; prefetch along velocity; cancel stale; queue new; `desired` set (8 km ring).
- `retire_tiles`: retire non-wanted; hold ancestors of blocking (wanted & !loaded) wanted tiles; over-cap LRU (fine tiles only, z≥10); coarse base (z≤9) never retired; delayed despawn (4–16 frames) on far→full upgrade.
- `upgrade_far_buildings`: when a far (massing) tile enters `far_lod_m`, build full buckets, keep massing 16 frames (no pop-out hole).
- `stream_tree_chunks`: leaf cards within `CARD_RANGE_M` (~600 m), full trees within `NEAR_IN` (230 m). Impostor hidden when full trees spawn.
- `near_chunks`: rock chunks as data, entities only within 500 m.

### 2.6 LOD (`lod.rs`)

- Quadtree over Web-Mercator tiles. Root zoom by altitude (2 at 200 km, 6 at ground). `root_radius` 2. `fine_range_m` 20 km (splits to finest within). `max_distance_m` ∞. `split` 1.0.
- Result: ~2 km tiles under feet, ~600 km tiles at horizon, a few hundred tiles total.

### 2.7 Materials / shaders

- Data-driven, hot-reloadable. One material per `bparts::Mat` bucket. Ground splat material with procedural textures. Building materials per Mat bucket.
- Native renderer: visibility-buffer-ish (GBuffer populated from a 64-bit VBuffer via atomic max). Material passes decoupled from geometry.

### 2.8 Physics

- Avian 0.8. Player = capsule, locked rotation, PD velocity controller. Ground probe = raycast down. Terrain colliders = trimesh (per tile). Building colliders = trimesh (merged per tile). Origin rebase > 8 km re-syncs all colliders.

### 2.9 Terrain

- `TerrainGrid`: single source of truth for ground height. Render mesh, collider, and placed objects all sample it. `sample(u,v)`.
- Elevation: Mapterhorn (Copernicus GLO-30) primary, AWS Terrain fallback. Segments: 192 (z≥14), 128 (z=13), 96 (≤z=12).

### 2.10 Buildings

- OSM footprints → `BuildingSpec` (`prepare`). `build_lod` (merged per material + collider). `build_detail` (facades, plinth/cornice, flat/pitched/mansard roofs, foundations).
- `archkit`: per-Region `Variant` tables (wall/roof palettes, roof forms, pitch, floors, ground-floor type).
- `bparts`: windows, doors, balconies, chimneys, shopfronts, AC units, fire escapes, rooftop gear.
- `meshkit`: sweep, inset, polygon, wall_with_openings.
- Far massing (`build_massing`): extruded footprints + flat roof cap, one mesh, vertex colors. Used past `far_lod_m` (3 km). Drops tile from ~290k tris to tens of thousands.
- Thresholds: 350/500 m², 4 floors, 14 m span in `roof_plan`.

### 2.11 Trees / foliage

- Tree/shrub placement via `foliage.rs` + `trees/`. Procedural branching trees, leaf/bark textures.
- `trees/impostor.rs` bakes impostor atlas at startup, builds per-tile impostor quads.
- LOD: impostor quads (far) → leaf cards (< 800 m) → full trees (< 230 m).
- Tree chunks: entities only near camera (within 500 m for rocks).

### 2.12 Ground scatter / surface

- `ground_scatter.rs`: near-camera ground cover (grass, flowers, pebbles, moss, scrub, snow, ripples) streamed as chunks.
- `surface.rs`: `SurfaceMap` land-cover raster (`Cover`) from landcover/landuse/park/water.
- `ground_tex.rs`: procedural tileable ground textures per `Ground` layer.

### 2.13 Sky / weather / space

- `sky.rs`: sun, atmosphere, Moon/planets, ocean, fog, weather, `Clock`.
- `clouds.rs` + `clouds.wesl`: volumetric cloud raymarch.
- `ed_space`: JPL Keplerian elements, planet positions, GMST.

### 2.14 Audio

- `ed_engine_audio`: Rust port of engine-sound-kit (JS reference). Engine voices, jet/steam/pod-racer/warp, mixer, cars from `data/cars.json`.

### 2.15 Menu / UI

- `menu.rs`: main menu, pause tabs (Map/Settings/Character/Controls), `GameState`, graphics settings, time slider.
- `mapview.rs`: full-screen slippy map (drag, zoom, pick, place labels).
- `hud.rs`: compass strip, mode pill, stats panel, arrival toast, celestial target rings.
- `minimap.rs`: 220 px circular panel, z16, rotated by heading, async tile workers.

---

## 3. Data flow (end to end)

```
CLI config (--lat/--lon/--alt/--fly/--perf/--screenshot/...)
  │
  ▼
main.rs: App::new, plugins (Default, Physics, Origin, Assets, Sky, Shadows, Camera, Foliage, Clouds, Perf, Native, Loading, Streaming, ...)
  │
  ▼
OriginPlugin: WorldOrigin (f64 ENU frame), GeoAnchored, Rebased, Focus
  │
  ▼
StreamingPlugin: TileManager (IO pool, CPU pool, apply budget)
  │  └─ resolve_vector_template (fetch OpenFreeMap template)
  │
  ▼
SpawnPlugin: SpawnRequest → ground height → teleport (Arrived)
  │
  ▼
select_tiles (4 Hz):
  │  lod::select(cam, ground, params) → wanted set (quadtree)
  │  view cone → in_view
  │  velocity prefetch → wanted_ahead (coarse + mid)
  │  cancel stale IO/CPU jobs
  │  queue new tiles (priority: visible first, then ahead)
  │  retire_tiles (hold ancestors, over-cap LRU, delayed despawn)
  │
  ▼
pump_jobs: IO done → CPU queue; CPU done → apply queue
  │  (tilegen::build on CPU pool: elevation + vector → terrain, surface, water, roads, buildings, trees, props)
  │
  ▼
apply_tiles (per frame, budget 300 units / 8 ms):
  │  Stage::Base → terrain, water, roads, rocks, props, collider, nav, ground scatter, sat tex (hidden)
  │  Stage::Extras → placements, meshlets
  │  Stage::Buildings → LOD buckets, colliders, road collider (or massing for far tiles)
  │  Stage::Trees → impostors, leaf cards
  │  Stage::Finish → reveal (Visibility::Inherited), register in loaded
  │
  ▼
native::native_frame (Last, if native enabled):
  │  mirror Bevy Mesh3d → ed_metal WorldRenderer
  │  retain_meshes_timed (2 ms/frame)
  │  render (shadow cascades, world, resolve, SSR, AO, bloom, water, clouds, post)
  │
  ▼
perf::record (Last, if --perf): CSV row per second
```

### What "loaded" means

A tile is in `mgr.loaded` only after `Stage::Finish`. Until then it's `pending` or `applying`. The loading screen waits for `desired` (8 km ring) to be fully in `loaded`.

---

## 4. Research: how others have done this

### 4.1 Nanite (Unreal 5) — virtualized geometry

- **Import:** meshes analyzed, broken into hierarchical clusters of ~128 triangles. DAG (LOD tree) built: cluster triangles → group → simplify to 50% → re-cluster. Parent draws instead of children when screen-space error < 1 pixel.
- **Runtime (GPU-driven):** entire scene in one large GPU buffer. Per view: GPU instance cull → cluster cull (frustum + occlusion via HZB) → LOD selection (pick cut matching desired error) → rasterize all triangles with a single `DrawIndirect`. Software raster for micro-triangles (≤1 pixel), hardware raster for larger. 64-bit visibility buffer (depth + triangle/cluster ID), then material resolve (shade each pixel once).
- **Streaming:** clusters stored on disk; metadata (cluster hierarchy) in memory. GPU LOD pass outputs a list of needed clusters not in memory → CPU fetches from disk → GPU. Eviction by priority (visible error). Low-res parents always resident → always something to render.
- **No CPU in the hot path.** CPU only starts the pipeline and parses feedback.
- **Result:** pixel-scale detail, high object counts, no pop-in (error < 1 pixel), automatic LOD.

### 4.2 Geometry clipmaps (Losasso & Hoppe, GPU Gems 2) — terrain LOD

- Concentric rings centered on camera, each ring 2× area at half resolution. Constant vertex count (~500k) regardless of world size.
- **Geomorphing in vertex shader:** blend vertex height between LOD levels over a transition zone (outer 20% of each ring) using `smoothstep`. `morphedHeight = mix(fine, coarse, smoothstep(near, far, dist))`. Pop-free at normal speeds.
- Toroidal buffer shifts each frame; CPU updates heightmap data.

### 4.3 CDLOD (Continuous Distance LOD)

- Quadtree of patches, each a fixed grid (33×33). Per-patch LOD selected by distance. Vertex morphing between adjacent LOD levels (morph factor based on distance to threshold). No popping.

### 4.4 VDPM (Hoppe)

- Geomorphs: transform performed over several frames by gradually changing vertex geometry. Only on visible regions. Eliminates popping. Constant frame rate by adjusting screen-space error tolerance.

### 4.5 World streaming (Battlefield 3 / Frostbite)

- **Tiles** (raster tiles: heightfield, mask, color) with fixed-size pools (atlases).
- **Streaming modes:** free (tile-by-tile, slow gameplay), push-based (bundles, fast), hybrid (bundles at spawn points + free fill-in).
- **Hysteresis:** load radius vs unload radius to prevent thrashing at borders.
- **Prefetch along velocity.**
- **Even with improvements, streaming is not instant** → design around it.

### 4.6 Browser terrain (Cinevva) — modern reference

- Progressive loading: geometry <100 ms, textures <300 ms, vegetation <1 s, volumetric <3 s, objects <10 s.
- Predictive pre-fetch based on velocity (walking: 2 chunks ahead; flying: pause rendering, stream destination, resume when first frame ready).
- Memory budget: 256 MB total terrain.
- Geomorphing in vertex shader (pop-free).
- LOD rings with skirt geometry (T-junction crack prevention).

### 4.7 Key principles from research

1. **Never let a region have no geometry.** Keep a low-res parent until the child is fully applied (Nanite: low-res parents always resident).
2. **Pop-free transitions require morphing** (geomorph) or sub-pixel error (Nanite). Instant swaps pop.
3. **GPU-driven.** CPU should not be in the per-frame hot path for culling/LOD. The GPU can cull clusters in one dispatch.
4. **Async IO off the main thread.** Main thread only registers ready data. Never block the frame.
5. **Hysteresis.** Load radius > unload radius. Prevents thrashing.
6. **Prefetch along velocity.** Don't wait for the camera to arrive.
7. **Colliders decoupled from render geometry.** Physics world is separate; rebase shouldn't touch hundreds of colliders.
8. **Virtual texturing** for satellite (already partially there via SatPool).
9. **Frame budget discipline.** Every system declares its cost and has an off switch.

---

## 5. Refactoring options (you decide)

### Option A — "Nanite-lite" on ed_metal (recommended long-term)

Build a real retained-mode GPU renderer on `ed_gpu`/`ed_render` (the stated end state):

- **Cluster DAG** for all meshes (buildings, terrain, props): group triangles into ~128-tri clusters, build parent simplifications (50% tris), stream from disk on demand.
- **GPU cluster culling:** frustum + HZB occlusion in one compute dispatch. LOD selection by screen-space error (sub-pixel = no pop).
- **Visibility buffer:** 64-bit atomic max (depth + triangle/cluster ID). Material resolve shades each pixel once.
- **Software raster** for micro-triangles (≤1 pixel) — avoids hardware raster overhead.
- **Terrain as heightfield clipmap** (constant vertex count, geomorph in vertex shader) instead of tile meshes.
- **Colliders:** separate physics world, only near player, Avian sync avoided on rebase.
- **Streaming:** cluster pages from disk, CPU parses feedback, fetches needed clusters, evicts by priority. Low-res parents always resident.
- **Target 60 fps:** GPU does all culling/LOD; CPU only manages streaming feedback and game logic.
- *Cost:* large undertaking (months). But this is the stated end state (native Metal + Nanite-like). Short-term Bevy fixes are fine but don't build large features on Bevy's pipeline.

### Option B — "Fix the Bevy path" (medium-term, less invasive)

Keep Bevy but fix the specific pain points:

1. **Geomorph for tile swaps.** Instead of instant swap (far massing → full buildings), blend over N frames by interpolating vertex positions in the shader (requires storing both LOD meshes + a morph factor). Or use geomorph on the terrain only (clipmap).
2. **Terrain as geometry clipmap.** Replace tile-based terrain with a clipmap (concentric rings, constant vertex count, geomorph in vertex shader). Eliminates terrain tile streaming entirely. Buildings/props still tile-based.
3. **360 loading with hysteresis.** Load radius 20 km, unload radius 25 km. Prefetch along velocity. Never retire a parent until children applied.
4. **Colliders:** terrain colliders only within 2 km, registered lazily on worker thread (not main thread). Or use TerrainGrid for ground height (no colliders) + building colliders only near player.
5. **Reduce tri count:** simplify roads (140–587 k → ~20 k), push building detail to 800 m, use impostors for trees beyond 230 m (already).
6. **Loading screen:** release when near ring (8 km) is loaded OR 15 s timeout. Show progress honestly.
- *Cost:* weeks. Improves feel significantly but doesn't reach Nanite-level.

### Option C — "Hybrid: clipmap terrain + tile buildings + Nanite later"

- Terrain: geometry clipmap (no tiles, no pop-in, constant cost, geomorph). Immediate visual win, moderate effort.
- Buildings: keep tile-based but fix LOD (massing → full with geomorph or delayed cross-fade). Prefetch + 360 loading.
- Colliders: TerrainGrid for ground, building colliders near player only.
- Plan Nanite for buildings/props as Stage B (M2) work.
- *Cost:* weeks for terrain clipmap, then Nanite later.

### Option D — "Accept current architecture, tune aggressively"

- Increase `EW_APPLY_BUDGET`, IO threads, CPU threads.
- Reduce far_lod_m to 2 km (less massing, more full buildings closer).
- Disable view cone (360 loading) — already default now.
- Increase retention caps.
- Accept hitches on rebase (terrain colliders stay).
- *Cost:* days. Marginal improvement. Doesn't fix root causes.

---

## 6. My recommendation

Given the frustration and the stated roadmap (Stage A: bugs + GPU measurement; Stage B: ed_gpu + ed_render), I recommend:

1. **Do not chase 60 fps on the Bevy path.** It won't get there (Bevy meshlets measured ~60% slower; no GPU-driven culling). Short-term fixes are fine, but don't build large features on Bevy's pipeline.
2. **Prioritize the terrain clipmap (Option C first step).** Terrain is the biggest source of pop-in and tile backlog. A clipmap gives constant vertex count, geomorph (pop-free), no tile streaming for ground. This is a contained, high-impact change (~1-2 weeks) that doesn't block Nanite later.
3. **Decouple colliders from tiles immediately.** Use TerrainGrid for ground height (no terrain colliders). Building colliders only within 500 m, registered on worker thread. This eliminates rebase hitches without sacrificing foot feel. The TerrainGrid normal fix needs the correct ENU transform — fixable.
4. **Keep impostor fix, dense-city fix, massing height fix** (all good, low risk).
5. **Revert the 360-loading default if perf is unacceptable** — make it opt-in (`EW_VIEW_CONE=0`) rather than default, since 360 loading at 2500 mph creates a 185-460 pending backlog. Or keep 360 but with hysteresis (load 20 km, unload 25 km) and a smaller prefetch cone.
6. **Loading screen:** release at 8 km ring OR 15 s timeout (already done). Show honest progress.
7. **Then Stage B:** start `ed_gpu` + `ed_render` with cluster LOD, targeting the Nanite model. This is where 60 fps comes from.

---

## 7. What was changed this session (summary)

| Change | File | Status |
|---|---|---|
| Terrain colliders removed (TerrainGrid probe) | streaming.rs, player.rs | Done, then reverted terrain colliders per user request |
| Impostor hide when full trees spawn | foliage.rs, impostors.rs | Done |
| Dense city detection (NYC etc.) | buildings.rs, tilegen.rs | Done |
| Massing height +0.5 | buildings.rs | Done |
| Coarse base layer never retires (z≤9) | streaming.rs | Done |
| Separate fine-tile cap | streaming.rs, menu.rs | Done |
| DelayedDespawn 4→16 frames | streaming.rs | Done |
| Coarse prefetch (z≤11, 3 s ahead) | streaming.rs | Done |
| Coarse priority boost | streaming.rs | Done |
| 360 loading default | streaming.rs | Done |
| Loading bar 8 km ring | streaming.rs, loading.rs | Done |
| far_lod_m 1500→3000 | streaming.rs | Done |
| fine_range_m 16k→20k | streaming.rs | Done |
| Terrain normal sign fix | player.rs | Done |
| tile_is_dense threshold lowered | buildings.rs | Done |

---

## 8. Open questions for you

1. Do you want me to proceed with the **terrain clipmap** (Option C first step) next, or **fix colliders properly** (TerrainGrid-only, no rebase hitch) first?
2. Is the **360 loading** worth the pending backlog, or should I make it opt-in?
3. Should I **revert the terrain colliders** (accept the TerrainGrid probe + fix its normal) to eliminate rebase hitches, or keep colliders for foot feel?

I'll wait for your decision before changing anything.

---

## 9. Key references

- **Solstice:** https://github.com/SamG-Coder/solstice (MIT, Rust, WebGPU, Nanite+Lumen)
- **Nanite SIGGRAPH:** https://www.advances.realtimerendering.com/s2021/Karis_Nanite_SIGGRAPH_Advances_2021_final.pdf
- **Nanite GPU-Driven Materials (GDC):** https://media.gdcvault.com/gdc2024/Slides/GDC+slide+presentations/Nanite+GPU+Driven+Materials.pdf
- **Geometry clipmaps (Hoppe, GPU Gems 2):** concentric rings + geomorph
- **VDPM (Hoppe):** geomorphs over several frames
- **CDLOD:** continuous distance LOD with vertex morphing
- **Battlefield 3 terrain (Widmark):** tile streaming, bundles, hysteresis
- **Cinevva browser terrain guide:** clipmaps, geomorphing, progressive loading, predictive prefetch
- **Solstice streaming doc:** https://github.com/SamG-Coder/solstice/blob/main/docs/streaming.md

---

## 10. Note on the globe

It IS a globe — the math is spherical (f64 ECEF / WGS84 ellipsoid), and tiles are slippy-map tiles on the sphere. But the render region is a flat tangent plane (local ENU, ~8 km across) that re-centers as you fly. So over the area you see, it's effectively flat; the curvature is in the coordinate math, not in the visible terrain mesh. Nanite doesn't care about globe vs flat — it just renders clusters.
EOF
echo "written"