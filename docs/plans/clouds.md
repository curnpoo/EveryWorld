# Clouds: volumetric, layered, weather-driven

Status: **engine-free field, lighting and CPU reference landed; GPU pass written, behind the
`clouds` cargo feature until its shaders are generated and it is measured on the Mac.**
Phase 2 item 5 (`../phases/phase-2.md` §7.1). Supersedes the P2-D11 cloud shell, as the owner
decided on 2026-11-12: Nubis-style raymarched volumes, procedural noise and weather maps, layers
(low cumulus, mid sheets and patches, high cirrus), weather sets the mix, lit from above and below.

Owner ask (2026-11-19): clouds in every world; no weird noise; smooth; thick and tall; driven by a
complex weather and wind system; multilayered; Star Citizen-like towering thunderclouds; thick and
fluffy, light and wispy, or dark and towering.

## 1. Where the code is

| Part | File | Verified here |
|---|---|---|
| Weather contract (what the climate sim fills) | `crates/ed_world/src/cloudscape/weather.rs` | Unit tests |
| Layers (band, coverage, type, wind, shear, rain) | `cloudscape/layer.rs` | Unit tests |
| Thunderstorm cells (drift, life cycle, anvil, rain) | `cloudscape/storm.rs` | Unit tests |
| Noise bank (f16 3D shape and detail, 2D weather, mips, coverage quantiles) | `cloudscape/noise.rs` | Unit tests |
| Density field | `cloudscape/field.rs` | Unit tests + previews |
| Lighting and reference march (`Stepper`) | `cloudscape/light.rs` | Unit tests + previews |
| Per-frame GPU words | `cloudscape/gpu.rs` | Unit tests, f32-vs-f64 parity, shader constant parity |
| Look reference (CPU, real atmosphere and tonemap) | `crates/ed_render/examples/cloud_preview.rs` | Renders |
| GPU passes | `crates/ed_render/shaders/clouds.slang`, `src/clouds.rs` | Type-checked only (no slangc, no GPU here) |
| Game glue (every world: play and headless) | `crates/ew/src/clouds.rs` | Type-checked, unit test |

## 2. How it works

**Weather to layers.** `CloudWeather` holds physical quantities only: humidity at three levels,
instability (a CAPE proxy), dew-point spread, ground altitude, tropopause, frontal lift, winds at
three levels, precipitation, seed. `layers()` is the meteorology:

- Cloud base = lifting condensation level, 125 m per degree of dew-point spread above the ground.
- Convective depth grows with instability: humilis ~0.7 km, mediocris ~2 km, congestus to 6.5 km.
  Cumulonimbus are storm cells, which grow from the low layer to the tropopause.
- Stable and saturated makes a stratocumulus deck; frontal lift makes altostratus, cirrostratus
  and steady rain.
- Winds aloft move each layer; the difference between levels is the shear that leans towers.
- `lerp` blends two weathers continuously (fronts arriving, storms building);
  `with_surface_wind` ties the low wind to the trees' and grass's `WindField` (O3-W1).

Presets stand in for the sim: clear, fair, scattered, towering, thunderstorm, overcast, frontal,
cirrus, layered. In game: `EW_WEATHER=towering` or `EW_WEATHER=fair>thunderstorm:900`.

**Density** (per layer, at a point):
1. Weather map: two coverage fields drift with the layer wind and crossfade over 25 min,
   variance-preserving, so clouds form and dissolve as they travel. A regional channel clusters
   them. The threshold comes from a quantile table, so coverage 0.4 draws 40% of the sky
   (measured: fair 15% -> 14%, scattered 41% -> 43%, towering 66% -> 69%, overcast 100% -> 97%).
2. Column height from a tower channel: a few big clouds among many small ones.
3. Profile: presence is radial (0 at the edge, 1 at the core) and the presence needed rises with
   height, so heaps are domes with flat 90 m condensation bases, not extruded walls.
4. Shape: Perlin-Worley carved by the envelope; the carve threshold never drops below 0.45, so
   interiors stay lumpy, and density ramps up over a short band (sharp cumulus edges).
5. Detail: Worley fBm (billowy tops) and Perlin (wispy bases, cirrus fibres), warped by the shape
   noise, boiling upward slowly, faded out with distance (18-45 km) before it can alias.
6. Storms: core, overshooting dome, anvil spreading downwind on the upper wind.
7. Rain shafts under raining cores, down to the ground.

**Lighting**: sun through a jittered 8-step light march (two long coarse steps so tall towers
shade their lee); dual-lobe Henyey-Greenstein; 4 multiple-scattering octaves (no black cores);
powder (lumpy, solid look with the sun behind you); sky ambient from above and ground bounce from
below, each through the cloud's optical depth above or below via two-stream diffuse transmittance
(dark bases under thick clouds, bright tops). Energy-conserving integration per step.

**March**: curved-planet shells about the camera; strides through clear air, fine steps inside a
cloud's envelope, and a back-up after a stride that lands in cloud (`Stepper`). Detail and texture
LOD by distance and pixel footprint.

**GPU frame** (`clouds.slang`): half resolution, checkerboard (a quarter of the half-res pixels
marched per frame, 1/16 of full res), temporal reprojection with the cloud's own depth and a
neighbourhood clamp, then a depth-aware upsample composited over HDR with premultiplied blend:
`hdr * T * shadow + L`, aerial perspective from the atmosphere froxels, sun colour from the
atmosphere at the cloud's altitude (sunset colours), cloud shadow on the ground.

## 3. "No weird noise": what caused it and what fixed it

Each of these showed in the previews and is fixed at the source, so the GPU inherits the fix.

| Artifact | Cause | Fix |
|---|---|---|
| Contour ripples on smooth cloud tops | Jitter covered one step, strides are two: half the stride phase never sampled, so where rays entered cloud followed the stride grid | Jitter spans a whole stride; back-up after a stride that lands in cloud |
| See-through hairlines along cloud undersides | Back-up rewound to a clear sample from before the previous cloud, re-walked it, looped until the step budget ran out | `Stepper` backs up only right after a stride, to the sample before it; `stepper_never_rewinds...` test fails on the old rule |
| Terraces under grazing sun | 8-bit noise: under one step per texel, bilinear interpolation of equal bytes is flat | f16 textures (8x finer steps) |
| Moire in the noise itself | Finest Worley octaves at 1-2 texels per cell | Detail 64^3; every octave at 2.7 texels per cell or more |
| Light-march banding | Fixed light-step distances quantised the sun's optical depth | Light steps scaled 0.7-1.3 by a per-step jitter |
| Stipple on far towers | Big far steps hit or miss thin structure | Fine steps inside envelopes |
| Shimmer far away | Detail aliasing | Detail fades 18-45 km; texture mips by footprint |
| Extruded walls, flat-cut cliffs | Presence was a hard footprint filled at every height | Radial presence + dome profile |

## 4. Turning the GPU pass on

The renderer embeds generated MSL and SPIR-V; this session could not run `slangc` (a download of
the pinned compiler was not permitted), so `clouds.slang` has no generated outputs yet.

1. `tools/shaders/build.sh` (slangc 2026.20) generates `gen/msl/clouds.metal` and
   `gen/spv/clouds.*.spv`. Fix any compile errors in `clouds.slang` (written against the existing
   modules' conventions, never compiled).
2. Build with the feature: `cargo run --release -p ew --features clouds -- --play zermatt`.
3. Check with `EW_CLOUDS_DEBUG=transmittance|depth|shadow`, then compare against the preview:
   `cargo run --release -p ed_render --example cloud_preview -- OUT scattered --view ground`.
4. Measure (rule 9) and record in `CHANGES.md`: "clouds march", "clouds temporal", "clouds
   composite" pass times at 1440p, at eye level under scattered and thunderstorm, and from 11 km.
   Knob: `EW_CLOUDS_STEPS` (default 160). Off: `EW_CLOUDS=0`.
5. When it holds the budget, make `clouds` a default feature.

## 5. Gate

- Zermatt and Blue Ridge at dawn, midday and dusk, from the ground, 3 km and 11 km, under
  fair, scattered, towering, thunderstorm, overcast and cirrus: the owner judges against photos
  (`../reference/look-reference.md`).
- No grain, banding, ripples, hairlines or shimmer when the camera is still or moving
  (temporal accumulation converged within ~16 frames).
- Clouds the same on two runs with the same seed and time (deterministic field; tests).
- Cost at the High tier inside the frame budget, measured on the M3 Pro at 1440p internal.

## 6. Open

- Cloud shadows on terrain are approximated in the composite (60% of lit ground treated as
  direct sun). The proper term is a cloud shadow map read by resolve's sun term (also dims the
  sky ambient under overcast).
- Rain is a density term only: no rain particles, wet ground or lightning yet (O3-W5 wants rain
  from clouds: the shafts already come from the raining cores).
- Weather is uniform around the camera except storms. The climate sim's synoptic grid (fronts,
  lows) should vary `CloudWeather` in space; the field already takes it per layer.
- A small library of authored cloud volumes (VDB-style hero clouds, owner interest) can enter as
  extra envelopes like storm cells.
- Far field: beyond ~140 km clouds stop; from orbit a cheap shell from the same weather map.
- Quality tiers: Low (quarter res, 64 steps), High (this), Ultra (full-res history, more light
  steps). Costs measured per tier before naming them.
