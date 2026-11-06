# Tile-load hitch: white tile, freeze, then smooth

Status: INVESTIGATION (2026-11-06). Findings from reading the code and from measurements on a Linux CPU box. **Not yet
confirmed on the Mac.** Nothing in the game was changed except this doc. Section 6 lists the exact experiments that
confirm or kill each idea.

Symptom (from the user): with every visible tile loaded the game is perfectly smooth. When a new tile arrives it shows
up **white**, the game **freezes for about a second**, then it is normal again and the tile looks right.

## 1. Short answer

The most likely cause is the **native Metal bridge uploading whole tiles on the main thread in one frame, with a heavy
CPU step per big mesh**. Every z14 tile has three or more meshes over 40,000 triangles (terrain 301k, roads 140k to
587k, merged building walls 88k to 565k). For each of those, `WorldRenderer::upload` sorts triangles into clusters and
then runs a hash-map vertex-clustering simplification **three times per cluster** (`ed_metal/src/world.rs:527-604`).
I replicated that CPU work outside Metal on realistic mesh sizes: **150 to 280 ms per big mesh, so roughly 250 to 700 ms
of main-thread time for one tile** on this machine. The frame budget system (`FrameBudget`, 4 ms) cannot see it,
because the upload happens in `native_frame` (in `Last`), after the budgeted `apply_tiles`.

The **white** is real and has a clear mechanism: a terrain draw whose satellite texture is not resident falls back to a
white texture (`world.rs:1023` and `:1150`, `unwrap_or(&self.white)`), and the bridge still draws it (`native.rs:289-314`).
Whether that is what you see, and why it comes before the freeze, needs the counter in section 6.

A second, independent candidate is a **GPU spike**: any new shadow caster changes the cache key of the far shadow
cascades, so they re-render (`world.rs:975-993`).

## 2. What a tile costs (measured)

All numbers below are for one z14 tile, built with the real data path (`tilegen::build`) on this box (4 cores, Linux,
release build). They are not Mac numbers; use the ratios and the shape, then re-measure on the Mac.

### 2.1 Worker CPU: `tilegen::build`

Measured with the existing ignored test, `cargo test --release -p ed_world real_places -- --ignored --nocapture`.
"Cold" includes network fetch and decode of elevation, vector tile and satellite; "warm" is disk cache only.

| Place | Cold | Warm (CPU) | Buildings | Trees | Roads tris |
|---|---|---|---|---|---|
| Central Park | 4.5 s | 1.6 s | 2,508 | 6,351 | 500k |
| Zermatt | 1.9 s | 1.0 s | 1,424 | 12,186 | 140k |
| Aix-en-Provence | 4.2 s | 2.0 s | 4,429 | 3,076 | 466k |
| Marrakech | 4.9 s | 3.2 s | 10,809 | 3,776 | 403k |
| Shibuya | 4.2 s | 2.8 s | 7,423 | 7,496 | 587k |

So a tile costs about **1 to 3.5 CPU-seconds of one core** on a worker, before the GPU sees it. That is fine on a
background thread. The hitch is not here, but two things are:
- the worker threads compete with the render thread for memory bandwidth (idea H7);
- cold tiles add 1 to 3 s of IO, which only affects how long the white state lasts if the IO prefetch failed (H10).

I did not split the 1 to 3.5 s by stage. A per-stage timer in `tilegen::build` is item M1 in section 6.

### 2.2 What a tile hands to the main thread

Triangle counts per part, same run (columns are triangles):

| Place | Terrain | Roads | Merged building LOD (all materials) | Tree mesh input | Placements |
|---|---|---|---|---|---|
| Central Park | 301,056 | 500,470 | 289,874 | 720,966 | 1,705 |
| Zermatt | 301,056 | 140,040 | 94,216 | 1,308,104 | 7 |
| Aix | 301,056 | 465,682 | 318,364 | 351,496 | 2,268 |
| Marrakech | 301,056 | 402,528 | 597,445 | 430,032 | 1,704 |
| Shibuya | 301,056 | 586,524 | 576,692 | 829,288 | 4,757 |

Notes:
- Terrain is 301,056 triangles and 149,765 vertices in every z14 tile. `docs/CHANGES.md` mentions 74k for a
  "render sub=1" configuration; what the tile build returns right now is 301k, so check that the game is using the
  setting you expect.
- **Roads are the biggest mesh after terrain, and in dense tiles bigger than terrain.** 140k to 587k triangles of roads
  per tile is far more than needed. This also makes the road overhaul in `docs/plans/beamng-earth-lessons.md` a
  performance item, not only a looks item.
- Merged building LOD is almost all one `LodWall` mesh per tile (88k to 565k triangles).
- Tree input is not uploaded whole: impostor quads and near-camera card chunks are built from it (`foliage.rs`).

### 2.3 Main thread: native upload

The hot loop is `native_frame` -> `r.upload(...)` -> `WorldRenderer::upload`. For any mesh over 40,000 triangles
(`world.rs:550`) it does this on the calling thread:

```rust
// world.rs:550-598, abridged
if ntri > 40_000 {
    let n = ((ntri as f32 / 20_000.0).sqrt().ceil() as usize).clamp(2, 24);
    // 1. bin every triangle into an n x n grid by centroid      (O(tris))
    // 2. counting-sort the index buffer by cell                 (O(tris))
    for c in 0..n * n {
        // 3. per-cluster bounding box
        for (k, cell) in [0.5f32, 1.5, 4.0].iter().enumerate() {
            let simp = simplify(&idx[a..b], positions, *cell);   // 3 LOD proxies per cluster
            ...
        }
    }
}
// simplify(): two HashMap<_, _> + one HashSet<[u32;3]> per call, one entry per vertex and per triangle
```

`simplify` (`world.rs:1292`) allocates two `HashMap`s and a `HashSet` per cluster per level and inserts one entry per
vertex and per triangle. That is the slow part.

I copied that exact code into a standalone program (no Metal) and ran it on regular grid meshes of the same triangle
counts. Best of 5 runs, release build, this box:

| Mesh (real example it stands for) | Triangles | Interleave | Cluster + 3 proxies | Total |
|---|---|---|---|---|
| Zermatt LodWall | 88k | 1.1 ms | 36.6 ms | 37.7 ms |
| Terrain, every z14 tile | 301k | 4.0 ms | 142.0 ms | 147.0 ms |
| Central Park roads | 500k | 6.0 ms | 207.5 ms | 215.4 ms |
| Marrakech LodWall | 565k | 13.1 ms | 262.9 ms | 278.2 ms |
| Shibuya roads | 587k | 11.0 ms | 268.2 ms | 281.1 ms |

Roughly **0.45 ms per 1,000 triangles**, and 90 percent of it is `simplify`. Adding the real meshes of each tile:

| Tile | Terrain | Roads | Walls | Estimated main-thread upload CPU |
|---|---|---|---|---|
| Zermatt | 147 ms | ~65 ms | ~40 ms | **~250 ms** |
| Central Park | 147 ms | ~215 ms | ~125 ms | **~490 ms** |
| Marrakech | 147 ms | ~185 ms | ~278 ms | **~610 ms** |
| Shibuya | 147 ms | ~281 ms | ~265 ms | **~690 ms** |

Caveats, so nobody over-trusts this:
- The inputs are synthetic grids with the right triangle counts, not the real meshes. Real meshes have less regular
  vertex spacing, which usually makes hash maps slower, not faster, but I did not test that.
- This box is not an M3 Pro. A Mac core may be 1.3x to 2x faster. A freeze of 150 to 500 ms per tile would still match
  "freezes for a second" when two tiles arrive together, which they usually do.
- The numbers exclude the Metal calls themselves (`newBufferWithBytes`, texture creation), which are additional.

Why the budget does not stop it: `apply_tiles` spends at most `EW_APPLY_BUDGET` (250 units, 4 ms) and estimates a stage's
cost as `triangles / 10_000` (`streaming.rs:678`), so the budget thinks a 301k terrain is 30 units. The expensive step runs
later, in `native_frame`, whose only limit is "48 mesh uploads per frame, 8 texture uploads per frame" (`native.rs:257,325`).
A count limit, not a time limit. One tile has 5 or more meshes, so a whole tile goes in one frame.

### 2.4 GPU memory per tile (computed from the counts, not measured)

Vertex is 12 floats = 48 bytes (`world.rs:531`), index is 4 bytes, terrain splat is 12 bytes per vertex.

| Part (Central Park) | Vertex buffer | Index buffer | Other |
|---|---|---|---|
| Terrain 149,765 v / 301,056 t | 7.2 MB | 3.6 MB | splat 1.8 MB, sat + detail textures 2 x 256 KB + mips |
| Roads 500k t (assumes ~0.5 vertices per triangle) | ~12 MB | ~6 MB | |
| Wall LOD 273k t | ~6.6 MB | ~3.3 MB | |

About **40 MB of Metal buffers for one tile**, plus the proxy index buffers. With `RenderAssetUsages::MAIN_WORLD |
RENDER_WORLD` (`native.rs:15-21`) Bevy keeps a CPU copy and also builds its own GPU copy for a camera that is switched
off, so the same tile may exist **three times** in memory. With about 90 resident tiles that is a lot of unified memory
and a lot of page faults. This is an estimate; measure with `vmmap` or Activity Monitor (item M4).

## 3. The white tile

Code path that makes white:
1. `native_frame` builds a `DrawItem` for every visible mesh. For terrain it looks the tile's satellite layer up in
   `SatPool::cpu` and uploads it, but only if `tex_uploads < 8` (`native.rs:295`, `:304`). If not, `tex` stays 0.
2. A terrain `MeshMaterial3d<TerrainMaterial>` is not a `StandardMaterial`, so `tint` stays `[1,1,1,1]` (`native.rs:228`).
3. At draw time `self.textures.get(&d.tex).unwrap_or(&self.white)` returns the white texture (`world.rs:1150`), and the
   terrain shader multiplies lighting by it. White.
4. Nothing stops the tile being revealed first: `apply_tiles` sets `Visibility::Inherited` at the end of its last
   stage (`streaming.rs:807`) and retires the coarser parent tile once children are loaded. Native has no way to say
   "this tile is not ready".

Triggers that can produce this, from most to least likely given what the code does:
- **Burst of tiles.** Each finished terrain tile wants 2 texture uploads (satellite and detail, `native.rs:295-306`). The
  cap is 8 per frame including every other texture, so 4 or more tiles in one frame leaves the rest white until the
  next frame. A frame that also does 500 ms of mesh upload means that "next frame" is a second away.
- **Pool full or size unknown.** `SatPool::alloc` returns `SAT_NONE` when a class is full or the size is not one of 512,
  256, 128 (`ground_material.rs:294-301`); the tile is then drawn without colour for as long as it lives. There is a
  `warn!`, so look for "satellite pool full" in the log.
- **Satellite source missing.** `b.sat_tex` is `None` when the satellite fetch failed in the build (`tilegen`), which also
  lands in `SAT_NONE`.

What the code cannot tell me is why you see white **before** the freeze. In the straightforward reading, a mesh and its
texture are uploaded and drawn in the same call, so the freeze would come first. Possible orderings: the first frame
after reveal draws white because the texture cap was hit, and the freeze is the next frame's uploads (several tiles
arriving together makes this natural); or the white is a different mesh (building walls and roofs are white until their
materials resolve). The counter in section 6 (M2) answers it: log every frame where a terrain draw had `tex == 0`, with
the frame time, and compare to when the long frames happen.

If the Bevy renderer is in use instead (`EW_NATIVE=0` or not on macOS) the white mechanism is different: the satellite
array layer is written by `queue.write_texture` in the render world (`ground_material.rs:350-375`) and may be written a
frame after the mesh is drawn. First check which renderer the Mac session runs (`native: Metal world renderer attached`
in the log).

## 4. Ranked hypotheses

Evidence levels: **Measured** (I ran it), **Code** (I read the path, did not run it), **Guess** (plausible, no evidence yet).

| # | Idea | Evidence | Predicted signature | Fix |
|---|---|---|---|---|
| H1 | Mesh upload CPU (clustering + 3 simplify passes) on the main thread, uncapped by time | **Measured** 150-280 ms per big mesh, 250-700 ms per tile | Long frame whose time is in `WorldRenderer::upload`/`simplify`, grows with triangle count; dense cities worse than Zermatt | F1, F2 |
| H2 | GPU: far shadow cascades re-render when the caster set changes | **Code** (`world.rs:957-993`, key includes every caster's mesh id) | GPU time of `shadow4`/`shadow5` timers jumps on frames when a tile appears; `shadow_skipped` drops; the CPU waits in `nextDrawable` | F3 |
| H3 | White: terrain drawn before its satellite texture is resident | **Code** (`native.rs:295`, `world.rs:1150`) | Counter of terrain draws with `tex == 0` is non-zero on the white frames | F4 |
| H4 | Texture upload path: 2 textures per tile, each `newTexture` + `replaceRegion` + its own command buffer + blit for mips + `commit`; plus two 256 KB `data.clone()` in `SatPool::alloc`; plus Bevy still queues the same bytes for its own GPU array | **Code** | A few ms per tile, sits next to H1 | F5 |
| H5 | Bevy builds a second GPU copy of every mesh for a disabled camera (`RENDER_WORLD` kept) and clones meshes on extraction | **Code** (`native.rs:15-21`) | Render-thread time and memory grow with tile arrival; disappears with `MAIN_WORLD` only | F5 |
| H6 | Apply stage: entity spawning, collider insertion, `recount()`; one oversized item always runs because the first item per frame is always allowed (`budget.rs:30-37`) | **Code** | Hitch is in `apply_tiles` (in `Update`), not in `Last` | Split big items; measure first |
| H7 | Memory bandwidth contention from tile worker threads (BeamNG.Earth measured the shadow pass going 3.4 ms to 19 ms from one background numpy thread) | **Guess**, supported by their measurement | GPU time rises while workers run, even with no main-thread stall; `EW_TILE_THREADS=1` helps | Duty-cycle workers (F6) |
| H8 | Physics: Avian static trimesh insertion (broad phase, BVH). Colliders are built on the worker, so only the insert is main-thread | **Guess** | Spike in `PhysicsSchedule`; goes away with colliders off | Defer insert, spread over frames |
| H9 | Periodic cleanup: `native_frame` does `retain_meshes`, `retain_textures` and builds a `HashSet` of all mesh ids every 240th frame (`native.rs:460-469`); frees many MB at once | **Code** | A smaller spike exactly every 240 frames, independent of tile arrival | Incremental retire |
| H10 | IO: the IO prefetch swallows errors and the CPU stage retries the network inline (`tilegen.rs:84-90`), so a tile can arrive late or without a satellite layer | **Code** | Long delay before the tile appears; white lasts for seconds, not one frame | Retry policy, show parent colour |
| H11 | Pipeline compile on first use | **Ruled out for the native renderer**: all pipelines are created at init (`newRenderPipelineStateWithDescriptor_error` appears only in `world.rs:327`, `clouds.rs:111`, and the post passes' setup). Still a candidate on the Bevy renderer (`ROADMAP.md` lists "pipeline pre-warm") | | |
| H12 | The CPU stall is partly waiting for the GPU: `nextDrawable()` blocks when the GPU is more than a couple of frames behind (`world.rs:841`) | **Code** | CPU frame time is long but `Instruments` shows idle in `nextDrawable` | Follows from H2 |

H1, H2 and H3 explain the full symptom between them; I would expect H1 to be the freeze, H3 the white, H2 the aftershock.

## 5. Why only new tiles hitch

All of the expensive work scales with *arrival events*, not with how much is on screen. Once the tiles are resident the
per-frame cost is the draw list, which is why you see no slowdown. Every hypothesis above fits this. It is also why the
bug is invisible in headless runs that load once at the start and then measure (`tools/perf_headless.sh` skips the
load phase for its summary, by design).

## 6. How to confirm each idea (do these in order)

Cheap instrumentation first; each item is small and can be committed behind `EW_TILE_TRACE=1`.

| ID | Do | What you get |
|---|---|---|
| M1 | `Instant` timers in `tilegen::build` per stage (elevation, MVT, surface, roads, buildings, trees, props, mesh build) and in `ready_tile` | Worker CPU split. Print one line per tile |
| M2 | In `native_frame` log, per frame: ms in `r.upload`, count and triangles of uploads, ms in texture uploads, count of terrain `DrawItem`s with `tex == 0`, count dropped by the 8 and 48 caps, frame number. Print only when any of them is non-zero | Confirms H1 and H3 and the order of white versus freeze |
| M3 | Log `r.times` GPU timers (already collected, `native.rs:474`) every frame during a tile arrival instead of every 3 s; include `shadow_skipped` | Confirms H2 |
| M4 | `vmmap <pid>` before and after loading 10 tiles; Activity Monitor memory and "page faults" column | Confirms H5 and the memory estimate |
| M5 | Instruments, Time Profiler, main thread only, while flying toward unloaded land at 100 m/s; look at the longest frame | Names the function with no guessing. If it is not `upload`/`simplify`, H1 is wrong |

A/B runs with switches that already exist (run `--fly --cruise 60` toward new land, compare `max_ms` and `p99_ms` in the
perf CSV after the load phase; use `EW_PERF=1 EW_PERF_OUT=...`):

| Run | Switch | If the hitch shrinks, it was |
|---|---|---|
| A0 | baseline | |
| A1 | `EW_NATIVE_NOSHADOW=1` | H2 (GPU shadow cascades) |
| A2 | `EW_NATIVE_NOPROXY=1` | proxy draws, not upload (weaker test) |
| A3 | `EW_TILE_THREADS=1` | H7 (worker contention) |
| A4 | `EW_APPLY_BUDGET=60` | H6 (apply chunking) |
| A5 | `EW_NATIVE=0` | native-bridge specific (H1, H3, H4) versus tile pipeline in general |
| A6 | roads off (temporary `hidden_part("Roads")` is already supported in `streaming.rs:597`) | roads' share of H1 |

A decisive negative for H1: M5 shows the long frame spent in Bevy's `apply_tiles` or in Avian, and `r.upload` is under
10 ms. Then look at H6 and H8.

## 7. Fix plan (after confirmation)

In order of expected payoff per effort. None of these is done.

**F1. Move all CPU work out of the upload.** `WorldRenderer::upload` should only create buffers.
- Split it into a pure function `prepare_mesh(positions, normals, uvs, colors, indices) -> PreparedMesh` (interleaved
  vertex bytes, reordered indices, clusters, proxy indices), engine-free and `Send`, called from `ready_tile` on the tile
  worker. The main thread does `newBufferWithBytes` on already-built bytes: a few ms.
- Replace the three hash-map `simplify` passes with a cheaper scheme: quantise vertices to the cell grid, sort once
  (radix/sort_unstable by cell id), dedupe with a sorted pass instead of a `HashSet`. A meshoptimizer-style simplifier
  is the better long-term option and matches the Nanite-like direction in `docs/plans/engine.md`.
- **Chunk at generation time**: terrain into a 4x4 set of meshes, roads and walls per chunk. Then no mesh is above
  40k triangles, per-chunk culling comes for free, and uploads are naturally spreadable. This also removes the need for
  the cluster step for most meshes.
- Expected: main-thread cost per tile from 250 to 700 ms to under 15 ms. Cost moves to workers (add roughly 0.5 s of CPU per
  tile there; it is background work).

**F2. A time budget for native uploads.** Replace "48 meshes and 8 textures per frame" with "about 2 ms per frame", and
count the real cost. Keep the first item rule (so nothing starves) but upload the cheapest first (textures before big
meshes).

**F3. Do not invalidate the far shadow cascades for every new caster.**
- Cascade 5 (terrain only, 20 km) and cascade 4 (4 km) should key on **tile identity + hidden/visible**, not on every
  mesh id, and ignore casters below the texel size they can resolve.
- Better: render only the new tile into the cached cascade without clearing it (depth test LESS keeps older terrain),
  instead of re-rendering the whole cascade.

**F4. Never draw terrain white.**
- Short term: for a terrain item whose texture is not resident, skip the draw if its coarser parent is still visible, or
  tint it with the tile's average satellite colour (the average is already available in the build) so a late texture is a
  slight colour change, not white.
- Proper: native reports residency back (a `NativeResident(TileId)` message), and `apply_tiles`/`retire_tiles` wait for it
  before revealing the tile and retiring its parent. This closes the loop the code is missing today.

**F5. One copy of each mesh.** When native is on, load meshes with `RenderAssetUsages::MAIN_WORLD` only (native needs CPU
data, Bevy's disabled camera does not need GPU data), drop the Bevy satellite `write_texture` path, and avoid the
`clone()`s in `SatPool::alloc` and in `native_frame` (`nor`, `uv`, `col`, `idx` are copied before the interleave).

**F6. Calm the workers.** Run tile workers at lower priority and, if A3 shows contention, apply the duty-cycle idea from
BeamNG.Earth (`pacer.py`: a worker sleeps after each 50 ms slice so it runs at most N percent of the time; theirs defaults
to 25 percent for background analysis, urgent work exempt).

**F7. Put a diet on roads.** 140k to 587k triangles per tile is the biggest controllable mesh. Targets: fewer cross-section
vertices per sample, markings as flat decals only near the camera, guardrail posts only in the near ring, and a coarser
`STEP_M` far away. This is part of the road work in `beamng-earth-lessons.md` (R0, R10).

Exit criteria for the whole fix (use the numbers already in `CLAUDE.md`'s perf workflow): in `--cruise 60` toward unloaded
land, `max_ms` after the load phase under 25 ms and `p99_ms` under 12 ms on the M3 Pro; zero frames with a terrain draw
whose `tex == 0`; `docs/CHANGES.md` entry with before and after tables.

## 8. What I could not do

- No Mac, no Metal, no GPU: nothing here was run through the real renderer. H2, H3 and the memory figures come from
  reading code; only H1's CPU cost and the worker times were measured.
- The upload benchmark used regular grid meshes. Re-run it with the real meshes (dump them from `TileBuild`) for a
  trustworthy number.
- I did not read `foliage.rs`, `near_chunks.rs`, `library.rs` merge costs or the Avian insertion path in detail, so H6 and H8
  are open.
- The tile build was run in the test harness (one test thread, dev machine); inside the game several workers run at once
  and the OS schedules them against the render thread.

## 9. Why BeamNG.Earth is relevant here

Their changelog is full of the same class of bug and its fixes, which is a good list of things to look for: a texture set
*before* the swap, work spread over frames with a time budget, one new object per frame for forests, pre-touching kinds of
props so the first use does not hitch, background analysis duty-cycled because memory bandwidth, not CPU, was what hurt the
render thread, urgent work served ahead of background work, and a revisit cache so a place seen before skips the build. They
also solved the "ground is not ready when the car arrives" problem by looking ahead along the predicted path and building
in the order the car will reach tiles. See section 9 of `beamng-earth-lessons.md`.
