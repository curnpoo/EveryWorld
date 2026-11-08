# Perf report — 2026-11-08 (Japan 4000 ft) — full data

Config: `--fly --lat 35.68 --lon 139.69 --alt 1219 --heading 90 --pitch -15 --hour 15 --width 1600 --height 900 --cruise 1118 --quit-after 45`. `EW_RESIDENT_TILES=1000`. Binary `target/release/everyworld`. Windowed (window popped up). Cruise starts immediately (no "sit still" delay flag exists).

## Summary

| Metric | Value |
|---|---|
| Avg fps | 36.2 |
| Min fps | 14.7 |
| Max tris (M) | 23.23 |
| Max single-frame ms | 513.43 |
| Max native/GPU ms (per sec, summed) | 191.62 |
| Hitches >100 ms | 3 (t=1.0s 281.8, t=36.6s 131.3, t=43.8s 513.4) |
| Final loaded tiles | 101 |
| Pending queue (range) | 158-239 |
| Final visible meshes | 81 |
| Rebases (log) | 6 (every ~7.17 s) |

## Per-second data

```
t=   1.0s fps= 16.8 tris= 0.00M pending= 158 apply= 0.02 retire= 1.78 native=191.62 upload= 0.09 max_ms=281.76 meshes=    1 tiles=   0
t=   2.0s fps= 43.0 tris= 0.00M pending= 164 apply= 0.04 retire= 5.35 native= 13.85 upload= 0.03 max_ms= 60.72 meshes=    1 tiles=   0
t=   3.0s fps= 50.2 tris= 0.00M pending= 166 apply= 0.05 retire= 6.61 native= 28.48 upload= 0.05 max_ms= 40.15 meshes=    1 tiles=   0
t=   4.1s fps= 41.5 tris= 4.00M pending= 169 apply= 0.36 retire= 5.94 native= 24.68 upload= 5.95 max_ms= 62.79 meshes=    8 tiles=   3
t=   5.1s fps= 41.2 tris= 5.71M pending= 188 apply= 0.19 retire= 5.33 native= 97.78 upload=65.67 max_ms= 45.86 meshes=   16 tiles=   4
t=   6.1s fps= 44.7 tris= 5.84M pending= 197 apply= 0.33 retire= 4.66 native= 53.27 upload= 0.49 max_ms= 44.90 meshes=   16 tiles=   7
t=   7.1s fps= 47.5 tris= 5.96M pending= 193 apply= 0.34 retire= 4.53 native= 42.43 upload= 0.79 max_ms= 68.39 meshes=   16 tiles=  10
t=   8.1s fps= 45.6 tris= 4.16M pending= 188 apply= 1.54 retire= 5.41 native= 41.24 upload= 3.53 max_ms= 45.98 meshes=    7 tiles=  12
t=   9.1s fps= 43.8 tris= 4.16M pending= 196 apply= 0.04 retire= 6.35 native= 52.23 upload= 5.76 max_ms= 40.50 meshes=    7 tiles=  12
t=  10.1s fps= 39.8 tris= 8.13M pending= 193 apply= 0.49 retire= 5.09 native= 47.49 upload=26.52 max_ms= 45.16 meshes=   17 tiles=  15
t=  11.2s fps= 40.0 tris= 9.80M pending= 201 apply= 0.17 retire= 4.12 native= 45.19 upload=12.65 max_ms= 44.77 meshes=   19 tiles=  16
t=  12.2s fps= 43.5 tris=11.77M pending= 210 apply= 0.16 retire= 4.18 native= 50.01 upload=11.67 max_ms= 44.54 meshes=   24 tiles=  17
t=  13.2s fps= 42.0 tris=11.77M pending= 216 apply= 0.03 retire= 4.02 native= 35.44 upload= 0.04 max_ms= 40.68 meshes=   24 tiles=  17
t=  14.2s fps= 43.2 tris=14.92M pending= 218 apply= 0.36 retire= 4.18 native= 61.62 upload=33.57 max_ms= 46.21 meshes=   24 tiles=  20
t=  15.2s fps= 43.7 tris= 8.30M pending= 211 apply= 2.91 retire= 8.22 native= 71.64 upload=19.35 max_ms= 43.55 meshes=   25 tiles=  20
t=  16.2s fps= 41.8 tris= 9.04M pending= 218 apply= 0.10 retire= 4.41 native= 71.57 upload= 4.23 max_ms= 41.58 meshes=   27 tiles=  21
t=  17.2s fps= 41.1 tris=11.19M pending= 216 apply= 0.16 retire= 3.92 native=100.08 upload=10.60 max_ms= 45.05 meshes=   31 tiles=  23
t=  18.2s fps= 38.8 tris=13.18M pending= 220 apply= 0.21 retire= 3.63 native= 36.30 upload=14.96 max_ms= 46.96 meshes=   31 tiles=  25
t=  19.2s fps= 39.6 tris=14.68M pending= 226 apply= 0.34 retire= 4.09 native= 33.79 upload= 9.30 max_ms= 46.31 meshes=   31 tiles=  29
t=  20.3s fps= 36.0 tris=15.66M pending= 235 apply= 0.21 retire= 3.37 native= 31.26 upload= 9.04 max_ms= 46.28 meshes=   31 tiles=  32
t=  21.3s fps= 33.4 tris=16.64M pending= 230 apply= 0.37 retire= 3.29 native= 27.60 upload= 7.27 max_ms= 58.27 meshes=   31 tiles=  36
t=  22.3s fps= 34.7 tris=10.64M pending= 217 apply= 3.00 retire=11.31 native= 34.05 upload= 9.19 max_ms= 50.20 meshes=   48 tiles=  39
t=  23.4s fps= 35.1 tris=12.97M pending= 224 apply= 0.37 retire= 3.42 native= 53.78 upload=11.49 max_ms= 54.90 meshes=   51 tiles=  42
t=  24.4s fps= 34.6 tris=13.78M pending= 223 apply= 0.10 retire= 3.31 native= 45.78 upload= 8.19 max_ms= 55.09 meshes=   51 tiles=  44
t=  25.4s fps= 35.6 tris=14.69M pending= 225 apply= 0.22 retire= 3.27 native= 24.33 upload= 4.31 max_ms= 47.88 meshes=   53 tiles=  47
t=  26.4s fps= 34.7 tris=16.59M pending= 225 apply= 0.48 retire= 3.36 native= 32.25 upload=11.21 max_ms= 51.90 meshes=   53 tiles=  52
t=  27.5s fps= 35.0 tris=18.24M pending= 229 apply= 0.22 retire= 3.47 native= 24.51 upload= 4.29 max_ms= 50.01 meshes=   51 tiles=  54
t=  28.5s fps= 35.9 tris=19.04M pending= 228 apply= 0.31 retire= 3.51 native= 63.14 upload=18.97 max_ms= 74.38 meshes=   53 tiles=  57
t=  29.5s fps= 33.0 tris=11.99M pending= 218 apply= 3.61 retire= 5.72 native= 55.27 upload=16.40 max_ms= 53.95 meshes=   53 tiles=  55
t=  30.5s fps= 35.9 tris=14.10M pending= 224 apply= 0.39 retire= 3.56 native= 34.73 upload=11.39 max_ms= 50.18 meshes=   57 tiles=  58
t=  31.5s fps= 33.4 tris=15.18M pending= 223 apply= 0.15 retire= 3.16 native= 23.21 upload= 3.39 max_ms= 56.37 meshes=   57 tiles=  59
t=  32.5s fps= 34.6 tris=17.18M pending= 217 apply= 0.51 retire= 3.33 native= 31.38 upload= 7.77 max_ms= 47.74 meshes=   55 tiles=  64
t=  33.5s fps= 34.5 tris=18.53M pending= 229 apply= 0.27 retire= 3.24 native= 29.93 upload= 8.41 max_ms= 50.87 meshes=   59 tiles=  67
t=  34.6s fps= 31.4 tris=20.03M pending= 239 apply= 0.73 retire= 3.12 native= 31.59 upload= 9.80 max_ms= 55.24 meshes=   59 tiles=  73
t=  35.6s fps= 34.5 tris=20.96M pending= 225 apply= 0.41 retire= 3.30 native= 27.71 upload= 4.76 max_ms= 51.71 meshes=   61 tiles=  79
t=  36.6s fps= 29.2 tris=14.12M pending= 217 apply= 1.27 retire= 4.14 native= 39.44 upload=15.19 max_ms=131.25 meshes=   75 tiles=  69
t=  37.7s fps= 33.5 tris=16.13M pending= 221 apply= 0.25 retire= 3.12 native= 42.38 upload=19.14 max_ms= 53.99 meshes=   79 tiles=  72
t=  38.7s fps= 30.4 tris=17.99M pending= 215 apply= 0.51 retire= 2.92 native= 32.53 upload= 9.68 max_ms= 58.18 meshes=   77 tiles=  78
t=  39.7s fps= 30.9 tris=19.32M pending= 211 apply= 0.45 retire= 2.65 native= 34.87 upload=11.65 max_ms= 57.93 meshes=   82 tiles=  84
t=  40.8s fps= 29.6 tris=20.84M pending= 207 apply= 0.79 retire= 2.72 native= 33.07 upload= 9.05 max_ms= 61.55 meshes=   77 tiles=  94
t=  41.8s fps= 29.4 tris=22.05M pending= 205 apply= 0.29 retire= 2.74 native= 31.59 upload= 9.37 max_ms= 89.86 meshes=   82 tiles=  97
t=  42.8s fps= 28.1 tris=23.23M pending= 200 apply= 0.44 retire= 2.71 native= 30.93 upload= 7.18 max_ms= 62.46 meshes=   79 tiles= 103
t=  43.8s fps= 14.7 tris=14.19M pending= 198 apply= 0.32 retire= 2.41 native= 40.36 upload=12.06 max_ms=513.43 meshes=   76 tiles=  86
t=  44.8s fps= 27.9 tris=15.53M pending= 191 apply= 3.55 retire= 2.82 native= 33.26 upload= 7.00 max_ms= 63.75 meshes=   81 tiles= 101
```

## Rebases (from /tmp/japan2.log)

```
12:40:08.367 rebased origin to 35.67997, 139.77867
12:40:15.538 rebased origin to 35.67993, 139.86717  (+7.17s)
12:40:22.709 rebased origin to 35.67990, 139.95567  (+7.17s)
12:40:29.879 rebased origin to 35.67987, 140.04419  (+7.17s)
12:40:37.084 rebased origin to 35.67984, 140.13310  (+7.20s)
12:40:44.251 rebased origin to 35.67980, 140.22168  (+7.17s)
```

## What I see

1. **Hitches align exactly with rebases.** t=43.8s (513 ms) aligns with the 6th rebase (12:40:44.25). t=36.6s (131 ms) aligns with the 5th (12:40:37.08). t=1.0s (282 ms) is startup (first tile batch + ground textures 334 ms). The 7.17 s cadence matches REBASE_DISTANCE_M = 8000 m at 1118 m/s (7.15 s).

2. **GPU is not the bottleneck.** native_ms summed per second is 13-192 ms over ~30-45 frames = 0.3-4 ms/frame. GPU frame time is fine. The 513 ms stall is on the CPU/main thread, not in the Metal renderer.

3. **apply_anchors did not warn (>30 ms)**, so the stall is downstream of the origin change — almost certainly Avian re-syncing colliders for all static tile entities after the teleport. At 4000 ft more tiles are visible (101 loaded vs 49 at 1000 ft), so rebase cost is higher than the 1000-ft Japan run.

4. **Pending queue never drains** (158-239 throughout). Tilegen can't keep up at 2500 mph. At 4000 ft more tiles are wanted (larger view), so the backlog is worse than at 1000 ft (155-256 pending there too, but 49 loaded vs 101 here — more loaded, more tris).

5. **Tris accumulate to 23.2 M** from only 101 tiles (dense Tokyo buildings ~230 k tris/tile). Retirement is 64 tiles/frame but new tiles arrive faster. Mass retirement causes secondary hitches: t=36.6 (20.96 → 14.12 M tris freed in one second) and t=43.8 (23.23 → 14.19 M).

6. **Visible gaps likely.** With 158-239 pending and only 101 loaded, fine detail is missing in newly entered areas until it streams in. The coarse base layer (z≤9, never retired) covers the worst holes, but at 4000 ft the view radius is larger, so coarse tiles must cover more area.

7. **No changes made.** This run is measurement only.
