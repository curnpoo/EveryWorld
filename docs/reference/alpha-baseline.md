# Alpha baseline (Phase 0, W2)

The numbers every later "better than the alpha" claim cites. Measured 2026-11-10 on the
alpha tag `alpha-freeze-0.15` (cac7988), release build, native Metal path (the macOS
default).

**Machine:** MacBook Pro, Apple M3 Pro (12 CPU cores, 18 GPU cores), 18 GB, macOS 27.2.
**Settings:** the owner's saved settings, tile memory "Normal". Disk cache
`~/Library/Caches/earthdrop` warm for Zermatt (shared with the owner's alpha checkout),
partly warm elsewhere.

## 1. Performance and loading

Command: `tools/perf_headless.sh` at the tag, binary `target/release/everyworld`, which runs
each scene with `--headless --fly --width 2144 --height 1206 --hour 15 --weather fair`.
One row per second in `alpha-baseline/<scene>.csv`.

| Scene | Run | Tiles at end / wanted | Frame rate in the last second | Worst frame after 5 s | Triangles |
|---|---|---|---|---|---|
| still (Zermatt, 120 m) | 22 s | 53 / 430 | 39.6 fps | 4230 ms (one stall) | 13.9 M |
| cruise (Zermatt, 60 m/s) | 25 s | 88 / 432 | 13.4 fps | 105 ms | 18.2 M |
| spin (Zermatt, 20 m) | 25 s | 83 / 430 | 15.1 fps | 118 ms | 18.3 M |
| forest (Black Forest, 3 km) | 22 s | 66 / 426 | 36.5 fps | 46 ms | 10.3 M |
| lapaz (La Paz, 300 m) | 22 s | 68 / 355 | 40.7 fps | 136 ms | 22.8 M |
| origin (Zermatt, 400 m/s) | 25 s | 73 / 434 | 20.9 fps | 64 ms | 18.1 M |

No scene finished loading inside its run. A 200 s standstill run (`still_long.csv`) shows
why:

| Time | Resident tiles | Pending | Pending in I/O | Pending in build | Frame rate |
|---|---|---|---|---|---|
| 9 s | 24 | 406 | 405 | 1 | 40 fps |
| 25 s | 110 | 320 | 310 | 10 | 42 fps |
| 41 s | **200** | 225 | 149 | 75 | 12 fps |
| 58 s – 200 s | **200** | ~225 | 0 | ~225 | 9–26 fps, mostly 11–13 |

## 2. Findings

1. **The view never completes.** The tile selector wants about 430 tiles at street level;
   the "Normal" memory setting caps residency at 200 on macOS (96 on Windows,
   `ed_game/src/menu.rs`, `TileRetentionSetting::cap`). Once the cap is reached the streamer
   builds, evicts and rebuilds the same tiles for as long as the game runs: about 225 jobs
   stay in the build stage, CPU work never stops, and the frame rate falls from about 40 to
   about 12 fps. This one defect explains all three of the owner's complaints: slow loading
   (the view never fills), gaps (the missing 230 tiles), and "runs not great" (permanent
   build load).
2. **Before the cap, loading is fetch-bound.** For the first 30–40 s, 95% or more of pending
   tiles wait on I/O, not on generation. Throughput is 3–5 tiles per second with a warm
   cache.
3. **First image is slow.** No tiles are resident for 4–8 s after launch; the first frames
   take 0.3–1.9 s each; one 4.2 s stall occurred at the start of the standstill scene.
4. **Frame cost is high even without churn.** About 40 fps at 14–23 M triangles in the
   lightest scenes, with fewer than 50 visible meshes: the cost is in triangles per tile and
   full-screen passes, not draw count. The native renderer accounts for about 850 ms of each
   second (`native_ms`).

## 3. Gaps

Observed in this baseline: missing tiles at every distance once the cap is reached
(finding 1), and an empty world for the first 4–8 s after launch (finding 3).

Not done in Phase 0: the screenshot walk of a fixed orbit-to-street path, and per-pass GPU
timings (`tools/gpu_passes.sh`). Both move to the Phase 1 harness, which records them for
the new renderer and, with the same path, for the alpha.

## 4. Look reference

`look/<place>.png`, from `tools/shoot_places.sh` at the tag (1280×720, 60 frames minimum,
captured when streaming is idle or after 480 s). Places and cameras are in that script.
Under the 200-tile cap streaming is never idle, so these show the alpha as the owner
sees it, gaps included.

## 5. Budgets set from this baseline

M3 Pro, 1440p, High tier, local packs (no network in the measured path).

| Measure | Alpha | Phase 2 budget | Phase 3 budget |
|---|---|---|---|
| Launch to first image of the ground under the camera | 4–8 s | 3 s | 3 s |
| Spawn to complete view at a standstill (nothing pending) | never (over 200 s) | 10 s, terrain and far field | 15 s in a reference city or forest |
| Worst frame after warm-up, standstill | 4230 ms | 25 ms | 25 ms |
| Worst frame flying at 60 m/s, then at 400 m/s | 105 ms, 64 ms | 33 ms | 33 ms |
| Steady frame rate | 12–41 fps | 60 fps | 60 fps |
| Resident set versus wanted set | 200 of 430 (churn) | the selector chooses within the memory budget; nothing is evicted that is still wanted | same |

The last row is a rule, not a number: Phase 2's selector (`../plans/world.md`) picks the
finest set that fits the residency budget and falls back to coarser rungs, so a memory
setting lowers detail instead of causing churn.
