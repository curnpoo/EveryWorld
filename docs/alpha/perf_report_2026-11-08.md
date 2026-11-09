# Perf report — 2026-11-08 (cap=1000 fine tiles, 2500 mph, 1000 ft)

Config: `--fly --alt 305 --heading 90 --pitch -15 --hour 15 --width 1600 --height 900 --cruise 1118 --quit-after 50` (rural) / `--quit-after 45` (Japan). `EW_RESIDENT_TILES=1000`, `RUST_LOG=info`. Binary `target/release/everyworld`. Note: `--cruise` starts immediately; there is no "sit still 20 s" delay flag, so loading and flight overlap.

## Results

| Metric | Rural America (40N,-98E) | Dense Japan (35.68N,139.69E) |
|---|---|---|
| Avg fps | 25.2 | 35.9 |
| Min fps | 8.1 | 17.7 |
| Max tris (M) | 23.79 | 19.41 |
| Max single-frame ms | 979.5 | 315.9 |
| Max native/GPU ms (per sec, summed) | 193.5 | 191.3 |
| Hitches >100 ms | 7 | 1 |
| Final loaded tiles | 231 | 49 |
| Pending queue (range) | 90-177 | 155-256 |
| Final visible meshes | 34 | 52 |

## Findings

1. **Hitches are CPU-side, not GPU.** GPU stays 20-30 ms/frame (native_ms ~190 ms summed over ~60 frames = ~3 ms/frame). The 300-980 ms single-frame stalls are not in the renderer. Timing correlates with origin rebases (every ~7.2 s at 2500 mph; log shows `rebased origin` at 12:36:43, 43+7.2, 43+14.4, ...). `apply_anchors` did not warn (>30 ms threshold), so the stall is downstream — almost certainly Avian re-syncing colliders for all static tile entities after the teleport. Rural (231 tiles → ~2000+ colliders) hitches 7×; Japan (49 tiles) hitches once.

2. **Pending queue never drains at 2500 mph.** 90-256 tiles queued throughout both runs. Tilegen cannot keep up with traversal speed; new areas always lag. Coarse tiles (z≤9, now never retired) provide a no-gap base layer, but fine detail pops in late.

3. **Tris accumulate to 20-24 M** with cap=1000 fine tiles. Retirement is 64 tiles/frame, but at 2500 mph new tiles arrive faster than old ones retire. Japan reaches 19 M tris from only 49 tiles (dense buildings ~390 k tris/tile).

4. **Japan is smoother** (avg 35.9 vs 25.2 fps, 1 hitch) because the build backlog limits loaded tiles to ~49, which keeps rebase cost low. Rural's 231 tiles make rebases expensive.

5. **No tile-gap complaints from data** (coarse base layer z≤9 holds), but pending 100-256 means fine detail is missing in newly entered areas until it streams in.

## Suspected root causes (no changes made)

- Avian collider re-sync on origin rebase (origin.rs `apply_anchors` → collider sync) — biggest hitch source.
- Tilegen throughput vs. 2500 mph traversal — permanent backlog.
- Tri count / retirement rate vs. fast travel — accumulation.

## Suggested next investigations (not implemented)

- Profile Avian collider sync on rebase; consider disabling colliders on far (coarse) tiles or rebase less frequently / in chunks.
- Lower default retention cap (already rescaled to 50/100/250 fine tiles) and test 100-fine-tile runs for a hitch-free 60 fps target.
- Measure whether reducing rebase frequency (larger `REBASE_DISTANCE_M`) reduces hitches without breaking floating-origin precision.
