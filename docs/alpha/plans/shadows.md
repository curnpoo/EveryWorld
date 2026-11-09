# Shadows: from 700 m to the horizon (B3)

> **Superseded (2026-11-09) — do not salvage.** This alpha shadow design (cascades + cached far
> shadow map + horizon maps) was a Metal/Bevy-era bandaid. The new engine uses **virtual shadow
> maps (VSM)** with a single virtualised source. Keep this document only as a record of the
> failure modes. See `docs/VISION.md` §11, `docs/ARCHITECTURE.md` (Rendering),
> `docs/MASTER.md` §5.2.

Status 2026-11-04: **stage 1 landed** (4 AGL-planned cascades to 2.5-12 km with a far fade). **Stage 2 prototype**: an engine-free horizon-map bake (`ed_world::horizon`, unit-tested, not yet wired into rendering). Everything below stage 2 is design.

## Recommendation

Use layers, each one cheap where it is used, and never rely on a single shadow map for "unlimited" distance:

| Layer | Range | What casts | Technique | Where it lives |
|---|---|---|---|---|
| **Near** | 0-2.5 km walking, to 12 km flying | everything: player, cars, trees, buildings, terrain | 4 cascades, even ~5.5x splits, AGL-planned, overlap blend, far fade | Bevy now (stage 1, done) |
| **Mid** | 2.5-15 km | terrain and building massing | one cached far shadow map, re-rendered only when the sun moves or tiles change | Bevy (stage 3) or `ed_render` cached shadows (M2) |
| **Far terrain** | unlimited (30 km reach per point) | terrain only | **horizon maps**: 16 horizon angles per 150 m grid point, baked per tile, shaded per pixel | `ed_world::horizon` + terrain.wesl (stage 2) |
| **Far objects** | > 2.5 km | buildings, canopy | building/canopy heights folded into the horizon bake; nothing per object | stage 4 |
| **Clouds** | to the horizon | cloud layer | existing layer-7 caster quad, unchanged | `clouds.rs` |

"No distance limit" then means this: every terrain pixel you can see knows whether a ridge up to 30 km away blocks the sun, and that costs the same at 50 km as at 500 m. Objects are an exception. Past a few km a building's shadow is just one more darker texel among many, and the honest approach is to fold those objects into the heightfield rather than shadow-map them. That is also how the engines that ship this work: UE uses distance-field shadows past the virtual shadow map range, and Horizon Zero Dawn uses heightfield terrain shadows.

Next step: wire stage 2 behind `EW_FAR_SHADOWS=1` (about one session; see "Stage 2 wiring"). It needs a coarse elevation fetch in `tilegen::prefetch`, which is in a file another session is changing, so it was left out of this pass.

## Why the old cascades looked bad (measured)

Old config (`sky.rs`): 2 cascades, first far bound 40 m, maximum 700 m, overlap 0.15, 2048² maps. Bevy fits each cascade's orthographic box to its frustum slice's bounding sphere, so the texel size is the slice diameter / 2048. Computed with `ed_runtime::shadows::texel_m` (60° FOV, 16:9), and the unit test pins these numbers:

| Config | c0 | c1 | c2 | c3 | Neighbour ratio | Reach |
|---|---|---|---|---|---|---|
| Legacy | 4.6 cm (0-40 m) | 80.5 cm (34-700 m) | | | **17x** at 34 m | hard cut at 700 m |
| Stage 1, walking (AGL < 30 m) | 1.8 cm (0-15 m) | 9.5 cm (to 82 m) | 52 cm (to 454 m) | 2.9 m (to 2.5 km) | ~5.5x | fades over 2.0-2.5 km |
| Stage 1, 1500 m AGL | 83 cm (to 724 m) | 2.1 m (to 1.8 km) | 5.3 m (to 4.6 km) | 13 m (to 11.6 km) | ~2.5x | fades over 9.3-11.6 km |

What each symptom came from:
- **"Very sharp near"**: 4.6 cm texels at 0-34 m.
- **"Blocky farther out"**: one 80 cm texel covers most of a person and all of a lamp post. Bevy's normal bias is 1.8 texels, so the 1.4 m bias in cascade 1 also detaches shadows from their casters (peter-panning).
- **"Abrupt switch"**: a 17x texel jump across an overlap only 6 m wide (34-40 m).
- **"Hard cut"**: `fetch_directional_shadow` returns 1.0 past the last cascade's far bound.
- **Distant towers cast nothing**: building chunks are shadow casters only inside `SHADOW_EXPLODE_M` = 750 m (`streaming.rs`). This is independent of the cascades and still true after stage 1. See stage 4.

## Stage 1 (landed): planned cascades + far fade

- `ed_runtime::shadows::CascadePlan::for_agl` (engine-free, 5 tests):
  - Quantize AGL to quarter octaves.
  - `max = clamp(8·agl, 2.5 km, 12 km)`
  - `first_far = clamp(0.5·agl, 15 m, max/4)`
  - 4 cascades, overlap 0.2.
  - `LEGACY` keeps the M0 numbers for A/B (`EW_SHADOWS=legacy`).
- `ed_game::shadows`: casts a ray down every 8 frames (Avian, falling back to geodetic altitude) and swaps `CascadeShadowConfig` only when the quantized plan changes. Plans therefore change a few times during a climb and never while walking, so cascades don't "breathe".
- `vendor/bevy_pbr/src/render/shadows.wesl`: the last cascade fades to lit from `0.8 × far_bound` to `far_bound` (`FAR_FADE_START`, which must match `ed_runtime::shadows::FADE_START`). There is no hard edge anywhere now.
- Verified with headless `--fly` shots (1280×720 at 2×, `--weather clear --hour 14`):
  - Zermatt, 40 m: building shadows at 60-150 m keep their outlines; they were soft blobs before.
  - Central Park, 25 m: a crisp building shadow; it was smeared before.
  - Zermatt, 1500 m: valley and hillside terrain shadows appear; legacy showed **no** shadows at that height.
- Cost (measured, noisy). Zermatt 40 m, 22 s runs, median frame time for t ≥ 10 s:

  | Config | Run 1 | Run 2 | Run 3 |
  |---|---|---|---|
  | New | 27.1 ms | 32.1 ms | 32.4 ms |
  | Legacy | no data | 30.0 ms (p99 spikes to 232) | 24.3 ms |
  | Shadows off | 26.7 ms | | |

  Run-to-run spread (about 8 ms) is larger than the difference between configs. Estimate: **0-8 ms per frame for 4 vs 2 cascades** on M3 Pro. All cascades re-render every frame. Needs the Mac A/B below before the setting is final. If it costs too much, use `num = 3` (ratio ~9x, still about 2x better than legacy) or 3 cascades plus the stage 3 cached far map.

Known stage-1 limits:
- **Rebase snap.** Bevy snaps cascade centres to texels in world space, and the floating origin re-anchors past 8 km. On that frame the snap grid moves, which can cause one frame of sub-texel shimmer. Fix later by snapping in a light space anchored to ECEF: a small vendor patch, or free in `ed_render`.
- **Shadow acne/bias.** Bevy's bias is in texels, so it grows with the cascade: about 5 m normal bias at c3. Cascade 3 is mostly terrain and massing, where that is acceptable. Watch for floating shadows on hillsides at 1-2.5 km.

## Stage 2: horizon maps for far terrain (prototype in `ed_world::horizon`)

Technique (Max 1988, "horizon mapping"; Timonen & Westerholm 2010, "Scalable height field self-shadowing"; Horizon Zero Dawn and Flight Simulator use variants):
- For each point of a 33×33 grid per tile (~150 m spacing at z13) and each of 16 compass azimuths, march outward and store the maximum elevation angle `atan((h(t) - h0 - t²/2R) / t)`.
- The `t²/2R` term is the Earth-curvature drop: 38 m at 22 km, which is enough to hide 30 m hills.
- Encoded as u8 on a sqrt curve: ≤0.35° error below 10° of sun elevation, where long shadows are.
- Shading: `lit = smoothstep(-w, w, sun_elev − horizon(sun_az))`, bilinear in space and linear in azimuth. `w` ≈ 0.5° hides the grid; the real penumbra is 0.27°.

Measured (`cargo test --release`, analytic terrain, one thread):
- Bake: **22.8 ms per tile** (33²×16 rays, 34 geometric steps per ray to 30 km).
- Size: **17 KB per tile**, which is less than a single 64² RGBA satellite mip.
- It does not depend on the sun: bake once on the CPU worker, valid at any time of day.
- Six unit tests cover:
  - flat ground is lit
  - an eastern ridge shadows morning sun but not evening sun
  - high sun clears the ridge
  - curvature hides low distant hills
  - azimuth wraps through north
  - the bake is deterministic

What the prototype found: the far march steps are about 2 km long, so the height source **must be prefiltered** to that scale. Point-sampling full-resolution data misses narrow peaks. The right source is a coarse Terrarium zoom, z9-z10 at ~150-300 m/px, max-filtered or at least bilinear. One such tile covers 40-80 km, which is all the 30 km reach needs.

### Stage 2 landed (2026-11-04, on by default; `EW_FAR_SHADOWS=0` off, `=debug` magenta tint)

Verified: Zermatt 08:00 from 6 km, debug tint marks the valley floors past the cascades; on vs off changes 3.8% of pixels (mean luma 96.0 -> 94.5; low sun, so shadowed floors were already dim). Everest 65 km 07:00: 17% of pixels, luma 119 -> 110. Not yet verified: canopy term, march, GPU cost. Far trees: impostors take the horizon term per tree (tile uv baked per vertex by `TileProjector::uv_map`, far layer in the impostor `MeshTag`, `impostor.wesl` `far_tree_shadow`); debug tints them cyan. Verified on the shaded Mattertal slope (07:30, 2.9 km). Canopy/march terms are for the floor under leaves and are not applied to crowns.

Three far terms, all multiplied into `directional_shadow_factor` on terrain, faded in over 75-100% of the last cascade's far bound:
- **Horizon map** (`ed_world::farshadow` + `horizon`): 16 azimuths. n = 65 points at z<=11, 33 at z>=12. Reach is clamp(extent/2, 30, 150 km) over a 3x3 block of a coarse Terrarium zoom (LRU cached; failures cached too). Baked once per tile on the worker, valid all day, works from orbit.
- **Canopy darkening**: the WorldCover canopy share (falls back to biome tree density) darkens the floor under trees by up to 60%, more at low sun.
- **Surface march**: a 128x128 f16 heightfield (ground, building heights from `BuildingSpec::shadow_outline` at z14, canopy at 14 m). 12 steps out to 400 m towards the sun; skipped when texels exceed 100 m (z<=12-ish).
- GPU: `ed_game::far_shadows::FarPool`, two 2D arrays sharing one layer per tile.
  - Horizon array: 260x66 RGBA8. Row 65 holds metadata: extent (f32) and n.
  - Surface array: 128x128 RGBA16F.
  - Size: 328 layers, 64 MB when on; 1 layer when off.
  - Bindings 110/111. The layer + 1 is in `MeshTag` bits 12-23 (sat layer bits 0-11, sat class + 1 bits 24-31).
- Bake cost (release, measured): 33-46 ms/tile at z8-z11, 13-16 ms at z12-z14. Shader: 2 horizon + 1 surface + up to 12 march taps per far terrain fragment, not measured on GPU yet.
- Verified headless: Everest from 65 km at 07:00. Ridges throw long west-pointing shadows that aren't there with the flag off. 17% of pixels changed; mean luma 119 -> 110.
- Not verified: canopy darkening and the march (the forest shots were either inside cascade range or overcast).
- Limits:
  - Only terrain receives the far terms. Water, impostors, cards and buildings don't.
  - The march stops at tile edges.
  - Needs sun shadow maps on (`directional_shadow_factor` is ignored otherwise).
  - The imagery still contains its own baked shadows.

### Stage 2 wiring (original plan)

1. `tilegen::prefetch`: also fetch the z(z-4) Terrarium parent (cached forever like every tile). `tilegen::build`: `HorizonMap::bake` with a height function that blends the tile's own `TerrainGrid` near the point with the coarse parent farther out. Store the result in `TileBuild` as plain data.
2. GPU: a **texture**, not vertex attributes. Terrain is vertex bound on the M3 Pro, and 16 more bytes per vertex (an 80-byte stride) costs exactly where we are tight. Use a pooled 2D-array texture:
   - One layer per tile: 33×33 RG8x8 = 4 RGBA8 texels per point, so 132×33 per layer.
   - The layer index rides on the terrain `MeshTag` next to the satellite layer, the same pattern as `ground_material::SatPool`.
   - 512 layers = 9 MB.
3. `terrain.wesl`, one clearly separated block before `apply_pbr_lighting`:
   - Sun azimuth and elevation come from `lights.directional_lights[0].direction_to_light`. It is fragment-visible and must be rotated into the tile's local ENU frame (the tile `up()` differs from render Y by up to ~0.5° over 50 km). The 0.5° does not matter for shadows; the azimuth does.
   - Compute `pbr_input.directional_shadow_factor *= lit`.
   - Gate on a `FAR_SHADOWS` shader def from `EW_FAR_SHADOWS=1`.
4. Cost estimate:
   - 8 texel fetches and ~20 ALU per terrain fragment: about 0.1-0.3 ms at 2560×1440.
   - Bake: +23 ms CPU per tile on the worker, roughly +10-20% of a z13 tile build. Its apply cost is one layer upload.
   - Stays coherent across rebase: everything is tile-relative.
5. Combining with cascades: inside cascade range, use `min(cascade, horizon)` rather than replacing. The horizon term never shadows wrongly where cascades are good, because it ignores objects, and the coarse grid only makes it softer. Past the fade it is the only term.

## Stage 3: cached mid map (2.5-15 km)

- One 4096² orthographic map centred on the camera, covering 15 km, holding terrain and building LOD massing. 3.7 m texels: houses as blobs, towers as real shapes.
- Re-render only when one of these happens:
  - the sun moves more than 0.05° (about every 12 s of game time at normal speed)
  - a tile applies or retires inside it (`streaming` already knows; hook the staged apply and retire)
  - the camera moves more than 1/8 of its extent (recentre with a texel-snapped offset)
- Explode/implode should **not** dirty it: LOD and detail share the same massing.
- Bevy has no cached cascade, so this needs either a second directional light with a custom render-layer and a manual pass, or `ed_render`. Recommendation: **build it in `ed_render` (M2 "cached shadows")** rather than fighting Bevy's light pipeline. Until then, stage 1 cascades plus stage 2 horizons cover the case: terrain is what dominates past 2.5 km.

## Stage 4: far objects

- Building casters past 750 m: `SHADOW_EXPLODE_M` in `streaming.rs` limits casting to exploded detail chunks. The merged LOD meshes could cast directly in cascades 2-3 (at 0.5-3 m texels a massing box is exact enough). The cost is LOD vertices in 2 more shadow passes. Measure before turning it on, and coordinate with the streaming owner.
- Fold building tops (`render_height`) and, later, canopy height (ETH Global Canopy Height, `procedural-flora.md`) into the horizon bake's height function. Cost and plausibility:
  - In a city at low sun this gives plausible block-scale darkening at 2-10 km for free.
  - A 300 m tower's 8.6 km shadow at 2° of sun is too thin for a 150 m grid; the grid smears it into a soft streak. Plausible, not exact.
- Blob/stamp shadows (projected footprints as decals): **not recommended**. They are wrong at low sun, which is exactly when far shadows matter, and they add per-object entities.

## Low sun and maximum shadow length

`length = height / tan(elevation)`

| Caster | 10° | 5° | 2° | 0.5° |
|---|---|---|---|---|
| 20 m tree | 113 m | 229 m | 573 m | 2.3 km |
| 300 m tower | 1.7 km | 3.4 km | 8.6 km | 34 km |
| 2000 m ridge (Zermatt) | 11 km | 23 km | 57 km | (curvature-limited) |

Cascades cannot follow these numbers, and they don't need to: by 2° the long shadows are terrain, and the horizon map handles them with a 30 km reach. Below about 1° the scattered sky light dominates and shadows fade anyway. The reach could go to 60 km for Alpine scenes if the coarse parent is z8.

## How other engines do it

- **UE5 Virtual Shadow Maps.**
  - A 16k² virtual page table per clipmap level, with only pages touched by visible pixels allocated.
  - Pages are cached and re-rendered only when invalidated (moving casters, sun change).
  - Needs a visibility-buffer-style page-marking pass and Nanite-style cheap re-rendering.
  - Past the VSM range, **distance-field shadows** (mesh SDFs and a global SDF) or a heightfield trace take over.
  - Lesson for us: caching plus a separate far technique. This is the M4 target in `engine.md` and needs `ed_render`.
- **Frostbite / Unity HDRP.**
  - 4 cascades with PSSM log/uniform split blending.
  - Blending across cascades, texel snapping, a cached static-only far cascade, and contact shadows for the near gap.
  - Stage 1 here is that recipe minus caching.
- **Horizon Zero Dawn / Forbidden West.**
  - Cascades near, plus a heightfield-traced terrain shadow far out.
  - Large draw distances with cheap far terrain shadowing.
  - That is stage 2.
- **Flight simulators (MSFS).** Terrain horizon/heightfield shadowing at altitude, with object shadows only near the camera.

## Fit with engine.md (M2/M4)

- Now in Bevy: stage 1 (done) and stage 2. Stage 2 is engine-agnostic by design: the bake is in `ed_world`, the shader block is a few lines, and both port to `ed_render` unchanged.
- M2 `ed_render` v1 "cached shadows": stage 3. Cascades snapped in an ECEF-anchored frame fix the rebase snap.
- M4: virtual shadow maps replace stages 1+3 on High/Ultra. Stage 2 stays as the far layer, the way UE keeps distance-field shadows. Medium/Mobile keep 2-3 cascades, the cached far map and horizons.

## Risks

| Risk | Mitigation |
|---|---|
| 4 cascades cost too much on M3 Pro (vertex bound: every caster is redrawn 4×) | Mac A/B below; fall back to 3 cascades; stage 3 caching; far trees are already out of shadows |
| Shadow bias at c3 (≈5 m normal bias) floats shadows on slopes at 1-2.5 km | Within the far fade; if visible, lower the far max on the ground to 2 km or bias per cascade (vendor patch) |
| Rebase shimmer for one frame | Rare (8 km moves); ECEF-anchored snapping later |
| Horizon bake needs data beyond the tile | Coarse parent zoom; until it loads, bake against the tile only (a short-reach horizon) and re-bake when the parent arrives |
| Horizon grid too coarse near the camera | `min()` with cascades, which own the near range; horizons only matter past the fade |
| Vendor patch drift on a Bevy bump | One `else` branch plus one const, listed in `CLAUDE.md` |
| Concurrent sessions in `streaming.rs`/`tilegen.rs` | Stage 1 touched neither; stage 2 touches `tilegen` prefetch/build only |

## Mac checks

1. **A/B timing**, window focused, about 25 s each, run each config twice in alternating order:
   - `--place Zermatt`, walking, `EW_PERF=1`, comparing default, `EW_SHADOWS=legacy` and `--no-shadows`.
   - Then the same at 1500 m.
   - Report the median `avg_ms` for t ≥ 10 s and p99.
2. Visual, Central Park at 2 m, sun at 14:00:
   - no visible cascade seams at about 15, 80 or 450 m
   - shadows fade out smoothly around 2-2.5 km, with no line
   - no floating shadows on the lawn
3. Rocket up from Central Park to 3 km: plans change a few times, with no flashing on the switch frames.
4. Zermatt `--hour 17` (low sun): long valley shadows within cascade reach. Past 2.5 km there are none yet (stage 2).
5. Walk more than 8 km in a straight line (rebase): watch the near shadows for a one-frame shimmer.
