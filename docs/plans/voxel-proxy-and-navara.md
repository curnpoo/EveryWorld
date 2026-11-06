# Voxel proxies, navara, and where the frame time actually goes

Written 2026-11-06. Parent docs: `docs/ROADMAP.md` (Stages A, B, D), `docs/plans/m2.md`
(slices N0-N7), `docs/plans/far-buildings-and-nanite.md` (20 km city tiers).

Two things prompted this note:

1. **navara** (<https://github.com/maplibre/navara>) - MapLibre's new 3D globe map engine.
2. **A custom engine post** (Toni Macaroni, 2026-11-03) describing an engine built for
   "a heavy amount of instances with a heavy amount of polygons": CLODs (virtual geometry)
   with **voxels for far representations, no impostors**, screenspace + voxel hybrid GI
   (no raytracing), and **proxies without instance data** for very far representations.

The note answers two questions: what is worth taking from each, and (measured, not guessed)
where the frame time actually goes today.

---

## 1. navara: what it is

A headless GIS core in Rust compiled to WASM, with rendering delegated to Three.js. Four API
tiers: declarative (sources and layers as config objects) -> plugin (bundled features such as
the photorealistic scene, first-person walking) -> API (per-feature styling by attributes,
picking, terrain sampling, standalone geodetic/ECEF math) -> shader (custom mesh/effect/light
descriptors). It streams satellite imagery, terrain, vector data and 3D city models onto a
globe. Licence: MIT / Apache-2.0.

Its crate layout is the useful part: `navara_parser` (ECS-free parse cores), `navara_mvt`,
`navara_geometry`, `navara_quadtree`, `navara_occluder` (horizon culling points),
`navara_cesium3dtiles` / `navara_cesiumtiles` / `navara_b3dm` / `navara_glb` (3D Tiles and glTF
bodies), `navara_pmtiles`.

### Verdict per piece

| Piece | Verdict | Reason |
|---|---|---|
| The whole rendering path (WASM + Three.js) | **Skip** | EveryWorld is native Metal; Bevy keeps windowing, input, UI, audio and physics. Declarative map layers solve a problem we do not have. |
| MVT decoding, tile streaming, quadtree LOD | **Skip** | We already have `ed_data::mvt`, `ed_data::vector`, `ed_world::lod`, and `ed_runtime` job pools with tile cancellation. |
| `navara_occluder` (horizon culling point) | **Read, then port ideas** | Our `ed_gpu::cull` does frustum + normal cone only. HZB occlusion is the standard answer to "15.6 M triangles submitted". MIT, so reading and porting is fine. |
| `navara_pmtiles` | **Spike** | Solves an open Stage A item: "pin data versions ... live data revisions are a silent source of mismatch". A PMTiles archive is one file with byte-range reads and a hashable SHA, so a snapshot becomes a hash. |
| `navara_cesium3dtiles` + `navara_b3dm` / `navara_glb` | **Later, optional** | A dataset-driven CLOD rung above our merged-mesh tier for landmark districts. navara's own examples use PLATEAU (Tokyo). We already took this one step with Overture buildings; PLATEAU is the next rung. |
| `navara_mvt` worker-offload pattern | **Already have it** | Their split (ECS-free parse core, decode on a worker, finalise on the main thread) is exactly `ed_runtime` + `tilegen`. Their "cancel evicted parse tasks" is `ed_runtime::jobs`. |

Net: navara is a **reference implementation to read**, not a dependency to take. Two things are
worth actually pulling across: the occluder, and PMTiles.

---

## 2. The custom-engine post: three techniques against our code

### 2a. CLODs / virtual geometry -> we are already building this

`crates/ed_gpu/src/cluster.rs` splits an indexed mesh into clusters of <=128 triangles, sorts
them by 3D Morton code of the centroid, gives each cluster a bounding sphere, a normal cone and
a geometric error, and `crates/ed_gpu/src/cull.rs` turns the survivors into indirect draws.
`crates/ed_gpu/src/caps.rs` already carries `mesh_shaders`, `atomics_64`, `ray_tracing` and
`tile_memory`. This is external validation of the Stage D decision, not a change of direction.

### 2b. Voxels for far representation, no impostors -> the genuinely new idea

Our far trees are hemi-octahedral impostors with distance thinning
(`impostors.rs`: `FULL_M` 1000 m, `GROW_MAX` 6, `MIN_PX` 2.5), and B2 reports they flicker.
The thinning hack is also why the far forest is expensive: every surviving tree grows to
stand in for its thinned neighbours.

**Correction (2026-11-06):** the impostor flicker is gone in the native Metal renderer (the
4-view blend, the wider thinning ramp and the no-sub-2-px rule fixed it there; B2 is still
open for the Bevy path). So the flicker is no longer the argument for this work. What is left:
the far forest still pays for one impostor instance per surviving tree, and the missing
`far_tree_shadow` term. That is a smaller case than it was.

Proposal - **a canopy heightfield proxy per tile**, baked from the same tree placements:

1. Voxelise crowns at ~16 m cells over the z14 tile (2.44 km square).
2. Per cell keep max canopy height, canopy density and a species tint.
3. Extract a 2.5D surface (one quad per occupied cell) as a single merged mesh.

What it buys:

- A forest tile becomes a few thousand quads in **one** mesh instead of 100 k+ billboard
  instances. No instance data -> nothing for the culler to fetch.
- The impostor thinning hack disappears, so B2's flicker has no root cause left to fix.
- It can feed cascades 4 and 5 as a caster, which also lands the missing
  `far_tree_shadow` term in the native path.
- Deterministic (tile + `hash01`), so it is multiplayer-safe.
- Cross-fade against impostors in the 300-800 m band; full trees stay inside
  `NEAR_IN`/`NEAR_OUT` (230/270 m) in `ed_game/src/foliage.rs`.

The same density field is cheap GI: feed canopy density into `ed_world::probes::bake`
(`SceneField` already exists) so forest floors stop being lit as if under open sky.

### 2c. Screenspace + voxel hybrid GI -> skip

We already have sun-aware probe GI (3D RGBA16F volume, texture slot 8, re-bake on ~4 deg of sun
movement), and Stage E plans DDGI, which is the better call on Metal. A voxel radiance cache is
the classic no-raytracing fallback; it is the right answer on a platform without RT (Stage H
mobile) and the wrong one on the Mac. Record it, do not build it. The one cheap piece worth
taking is 2b's canopy occlusion of sky in the probe bake.

### 2d. Proxies without instance data -> we are already doing this

`plans/far-buildings-and-nanite.md` Tier 1/2 (merged per-tile footprints, one indirect draw per
tile) and the shadow proxies that took the cruise scene from 17.4 to 34.9 fps are the same idea.
The post just states it as a principle: at far range, bake the geometry so there is nothing to
instance.

---

## 3. Measured: where the frame time goes

All numbers below were taken on this Mac on 2026-11-06 with the native Metal renderer
(`target/release/earthdrop`, headless, 2144x1206, `--hour 15 --weather fair`). The per-pass
figures come from the renderer's own GPU counters (`ed_metal::timers`); `gpu_wait` is wall time
for the whole frame. **Headless numbers include a framebuffer readback and are not the windowed
Mac numbers** - re-measure windowed before acting.

### 3a. Forest - Black Forest, 48.0/8.2, alt 3000 m, settled

32-34 fps, `gpu_wait` 20.0-21.4 ms, 363 draws, 5.90 M triangles submitted, 501 meshes resident.

| Pass | ms | Share |
|---|---|---|
| world (sky + opaque + clouds) | 10.1-10.9 | 52% |
| water | 5.3-5.8 | 28% |
| resolve | 1.1-1.3 | 5% |
| shadows 0-5 | 0.46 (6/6 cached) | 2% |
| cpu_encode | 0.4 | 2% |

With `EW_NATIVE_NOWATER=1` (water pass off): `gpu_wait` 19.8-21.4 ms - **unchanged**. The water
pass is off the critical path here; there is slack in the frame.

### 3b. City - Shibuya, 35.6595/139.7004, alt 12 m, settled (after ~70 s of streaming)

27-32 fps, `gpu_wait` 20.7-26.1 ms, 474 draws, **15.61 M triangles submitted**, 720 meshes
resident.

| Pass | ms | Share |
|---|---|---|
| world (sky + opaque + clouds) | 14.6-18.5 | 71% |
| water | 5.1-7.8 | 25% |
| shadows 0-5 | 4.08 | 20% |
| resolve | 1.14 | 5% |
| cpu_encode | 0.5 | 2% |

### 3c. The same city scene without the ocean FFT

`crates/ed_metal/src/ocean.rs` evolves and inverse-FFTs three 128^2 JONSWAP cascades
(600 / 85 / 12 m) **on the GPU every frame**. Two A/B runs separate it from the water shading:

| Config | fps | `gpu_wait` | world | water | shadows 0-5 |
|---|---|---|---|---|---|
| baseline | 27-32 | 20.8-26.1 ms | 14.6-18.5 | 5.1-7.8 | **4.08** |
| `EW_NATIVE_NOFFT=1` (analytic waves, water pass on) | 33-34 | 18.8-19.6 ms | 12.9-13.3 | 3.5-3.6 | **1.83** |
| `EW_NATIVE_NOWATER=1` (no water pass at all) | 34 | 19.0-19.6 ms | 13.3-13.7 | - | **0.61** |

Reading: the **FFT compute is the cost**, not the water shading.

**Correction (2026-11-06, later the same day):** the ~4.8 ms figure above was read off a
single A/B pair and does not survive repetition. Three alternating runs of the stable forest
scene with the FFT forced on and forced off (the minimum of each, which is the cleanest estimate
available) gave **19.57 ms with the FFT on and 19.71 ms with it off** - the compute is below the
run-to-run noise, which in that scene spans 19.6-32 ms. The earlier "the FFT steals 2.25 ms
from the shadow cascades" reading came from the per-pass counters shifting between frames
(`resolve_oldest` returns the oldest sample, so a counter can be several frames stale).
So the ocean FFT is **not** a 23% win; treat it as worth skipping on principle, not on a
measured number. What the numbers *do* support: the per-pass timers in this renderer are not
reliable enough to attribute milliseconds between passes without repeated runs.

The ocean disc itself is not the problem: `sky.rs:97` already spawns it with
`NotShadowCaster`, and it is only ~9 k triangles over a 280 km disc.

### 3d. Streaming is the real city problem

The same scene while tiles were still arriving: `gpu_wait` 93 ms, then 207 ms; shadow draws
82 -> **1639 with 0 cached**; resident triangles 6.3 M -> 12.8 M. Every arriving tile
invalidates all six cascades, so nothing can be cached and the frame collapses.

### 3e. Upstream triangle budget

`EW_TILE_STATS=1` on a z14 forest tile: `terrain=301056 roads=5376 trees=0 rocks=0 street=0
bld=0 total=2106910 trees_n=15798`. The printed categories do not add up to the total: the
missing ~1.8 M is `rock_chunks` + cliff slabs (`tilegen.rs:591-597`), which the `TILESTAT` line
does not print. Per z14 tile the world carries about **6x more rock triangles than terrain**, and
rocks only spawn within 500 m (`ROCK_RANGE_M`). That is a streaming-memory and tile-build cost
(and a suspect for the tile-load hitch), not a GPU cost. **Fix the `TILESTAT` line first** - it
is three lines of code and it hides the largest triangle producer in the world.

### 3f. The bisect knobs do not all work in native mode

`EW_BISECT=hide:bld` on the city changed nothing (474 draws, 15.61 M tris, world 15.5-16.4 ms -
same as baseline), and `hide:std` made the forest *slower*. `hide:terrain` and `hide:all` do
work. The bisect set was written for the Bevy renderer; in native mode the draw list comes from
`ed_game::native`, which mirrors entities but not in the way `perf.rs` assumes. Treat bisect
numbers in native mode as suspect until this is fixed.

---

## 4. The savings ranking

Ordered by measured GPU time, with the fidelity effect of the fix.

### #1 The ocean FFT: skip it when the sea is not near the camera

Three 128^2 cascades are evolved and inverse-FFT'd **every frame**, whether or not any of that
water is visible. The game now decides per frame: the FFT runs only when the tile the camera is
in has real sea in it (`TileBuild.sea_fraction`; the elevation data clamps the ocean to 0 m, so
the *area* at sea level is the signal, not the height). In a city that is off, on a beach or
over the sea it is on.

The measured saving is **not** the 4.8 ms claimed earlier - repeated runs put it below the
noise (see 3c). It is still the right thing to do: no reason to simulate waves for water that
cannot be seen, and it scales with the frame.

- Keep the spectrum (`h0`) as it is; only the evolve + IFFT needs to run per frame.
- If the FFT ever shows up in a profile, run fewer cascades by distance and put the compute on
  a lower-priority queue.

Fidelity: neutral where the sea is visible; the gate only turns the compute off when the water
is out of sight.

### #2 Per-cluster GPU culling + visibility buffer: the 14.8 ms world pass (71% of the city frame)

474 draws carry 15.61 M triangles - 33 k per draw - because tiles are merged and culled
all-or-nothing, so a tile is drawn whole as soon as any part of it is on screen. How much of
that is behind the camera or occluded is **not yet measured** (the perf CSV's visible-triangle
columns are Bevy-side and unreliable in native mode), so treat the saving as "proportional to
the hidden fraction" rather than a number. The cluster builder already exists; N2 (GPU-driven
indirect draws) and N3 (visibility buffer) turn it into per-cluster culling and shade each
pixel once.

Fidelity: **improves**. Visibility-buffer resolve gives correct per-pixel normals and AO on
building facades (forward + MRT cannot), and CDLOD geomorphing removes LOD pops (B1).

### #3 Shadow-cache survival during streaming: 207 ms frames -> ~25 ms

1639 shadow draws with 0 cache hits while tiles arrive. The append/defer machinery exists
(`EW_NATIVE_NOAPPEND`, `EW_NATIVE_NODEFER`, `EW_NATIVE_NOPROXY`): the problem is that a new
tile invalidates the *whole* caster list for all six cascades.

- Make a cascade dirty only for the casters that actually entered its region.
- While `pending > 0`, defer cascades 4 and 5 (they cover 4-20 km and are cached anyway).

Fidelity: **improves**. Stable far shadows instead of re-rasterising every frame; this is also
where B3's "shadow cascades are irregular and hard-cut" comes from.

### #4 The upstream triangle budget: rock chunks (memory and hitch, not GPU)

1.8 M rock triangles per z14 tile, of which only the chunks within 500 m are ever drawn.
Bake rocks at a coarser chunk size away from the camera, or decimate at bake time. Also fix the
`TILESTAT` line so the cost is visible.

Fidelity: neutral, unless the decimation is visible up close.

### #5 Road markings are geometry

`ed_world/src/roads/mod.rs:1460` builds lane lines, edge lines and crosswalks as swept
geometry. Road triangles per z14 tile are 281 k (Central Park) to 502 k (LA). Markings belong in
the road material (the splat already exists) or as decals: fewer triangles *and* crisper lines
(mipmaps and anisotropy, no z-fight).

Fidelity: **improves**.

---

## 5. Ordered plan

Stage A is not finished, so the first item is still the baseline table. Everything below is
ordered so that the cheap, measurable wins come first.

| # | Item | Where | Cost |
|---|---|---|---|
| A1 | Baseline table on the 6 reference scenes, windowed and headless, with per-pass GPU ms | `tools/perf_headless.sh`, `tools/gpu_passes.sh` | 1 session |
| A2 | Skip the ocean FFT when the sea is not visible; recompute the spectrum only on wind change | `ed_metal/src/ocean.rs`, `ed_game/src/native.rs` | small |
| A3 | Print `rock_chunks` and impostor triangles in `TILESTAT` | `ed_world/src/tilegen.rs:878` | 3 lines |
| A4 | Fix the native bisect mapping (`hide:bld`, `hide:std`) | `ed_game/src/perf.rs` | small |
| B1 | Shadow cache: per-caster dirty tracking, defer cascades 4/5 while streaming | `ed_metal/src/world.rs` | medium |
| B2 | Markings into the road material instead of geometry | `ed_world/src/roads/mod.rs` | medium |
| B3 | Canopy heightfield proxy per tile, cross-faded with impostors | new `ed_world/src/trees/canopy.rs` | large |
| B4 | Rock chunks: coarser chunking / decimation at bake | `ed_world/src/props.rs`, `ed_world/src/cliffs.rs` | medium |
| B5 | Clipmap / CDLOD terrain (2 m grid per z14 tile is the terrain cost) | `ed_gpu/src/clipmap.rs` into the native path | large |
| D1 | Per-cluster GPU culling + visibility buffer (N2, N3) | `ed_gpu` | large |
| E1 | Canopy density into the probe bake | `ed_world/src/probes.rs` | small |
| H1 | PMTiles snapshot for pinned data versions | `ed_data` | medium |
| H2 | 3D Tiles / PLATEAU for landmark districts | `ed_data`, `ed_world` | large |

Read-only, no code: `navara_occluder` and `navara_quadtree` before N2 is designed.

---

## 6. Raw commands

```bash
# per-pass GPU times (needs RUST_LOG=info; the default Bevy filter hides info!)
RUST_LOG=info target/release/earthdrop --headless --fly --width 2144 --height 1206 \
  --hour 15 --weather fair --lat 35.6595 --lon 139.7004 --alt 12 --pitch -8 --quit-after 75 \
  2>&1 | grep -E "native:" | tail -3

# the same without the water pass
RUST_LOG=info EW_NATIVE_NOWATER=1 target/release/earthdrop --headless --fly ...

# per-tile triangle budget
EW_TILE_STATS=1 target/release/earthdrop --headless --fly --lat 47.55 --lon -123.65 --alt 300 ...
```

The `native:` line format:
`native: N fps | D draws T M tris | M meshes resident | gpu gpu_wait .., world .., water ..,
resolve .., shadow0..5 .., cpu_encode .. | shadow draws N (+N cached, N deferred)`

