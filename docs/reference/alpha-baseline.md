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

Six places, captured 2026-11-10 at the tag, in `look/<place>.png` (window capture at
1280×720 logical, 2560×1440 on the Retina panel, plus the title bar). Each place opens a
window, waits, takes the window image with `screencapture -l<window id>`, and quits. The
alpha never reaches "streaming idle" under the 200-tile cap (finding 1), so a fixed wait
replaces the alpha's own idle-triggered `--screenshot`: 15 s at street level, 45 s for the
1500 m aerial, which needs far more tiles. The images show what the alpha draws after that
wait, gaps included.

Common arguments: `--fly --weather fair --no-hud --quit-after 30 --width 1280 --height 720`.
`--quit-after` matters beyond closing the window: it marks the run as automated, which
stops the alpha auto-joining the multiplayer server saved in the owner's settings. That
server sets the clock to real time and overrides `--hour`.

| Place | `--lat` | `--lon` | `--alt` | `--heading` | `--pitch` | `--hour` (solar) | Wait |
|---|---|---|---|---|---|---|---|
| central_park | 40.7700 | -73.9740 | 120 | 20 | -14 | 10.5 | 15 s |
| zermatt | 46.0150 | 7.7400 | 300 | 230 | -6 | 11.0 | 15 s |
| aix_en_provence | 43.5270 | 5.4450 | 90 | 340 | -18 | 14.5 | 15 s |
| marrakech | 31.6260 | -7.9890 | 80 | 40 | -20 | 9.5 | 15 s |
| shibuya | 35.6590 | 139.7000 | 110 | 300 | -16 | 15.0 | 15 s |
| manhattan_aerial | 40.7000 | -74.0300 | 1500 | 30 | -22 | 14.0 | 45 s, `--quit-after 70` |

The date was the capture day (November), so the sun is low; pass `--date 2026-11-10` to
reproduce the same light. These are a floor, not a target (P1-D25). Phase 4 must clearly exceed them in detail and
fidelity, lighting, clouds and building fill, with solarpunk architecture in place of the
alpha's plain boxes. What they show that must not survive: missing buildings and empty
blocks, flat untextured facades, plain box massing, thin cloud layers, and flat ambient
light in shadow.

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
