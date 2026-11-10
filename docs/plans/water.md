# Water — design brief (from scratch)

Status: **design brief, not implemented.** Target: first real water in Phase 2 (planet surface),
full system Phase 4, buoyancy and boats Phase 5, polish Phase 8 (`docs/ROADMAP.md`). Supersedes
`docs/alpha/plans/ultra-water-port.md` (the alpha FFT port is a post-mortem, not a plan).

Owner direction (2026-11-09): *"i want to rethink and redo the water because it sucks right
now… i dont want to pull all of those things since those systems were bandaids on
metal/bevy."* The alpha ocean is therefore a **list of failure modes**, not a code source.

Related: `docs/VISION.md` §5, `docs/DECISIONS.md` D13, `docs/ARCHITECTURE.md` §6,
`docs/plans/renderer.md`, `docs/phases/phase-1.md` P1-D18.

---

## 1. Why the alpha water failed

Not "the FFT was slightly wrong" — the *shape of the system* was wrong:

| Failure | Consequence |
|---|---|
| A flat FFT shell bolted onto a separate globe/ocean object | Hard representation switch, no continuity from orbit to shore, visible seam at altitude |
| One wave scale tuned for sea level | Waves vanish or alias at altitude; no swell-to-ripple range |
| No depth/shore coupling | No shoaling colour, no shoreline foam, water meets land as a hard edge |
| Fake/poor reflections | Water reads as a shiny plane, not a lit volume |
| No refraction/absorption | Underwater is a camera clip, not a medium |
| No simulation hand-off | Boats/flotsam cannot float; buoyancy would be faked |
| Weather-independent | Storms and calm look identical |

The new system must be a **planet surface**, a **lit volume**, and a **deterministic simulation
input** at the same time.

---

## 2. Requirements

### 2.1 Continuity (the north star)

- The ocean is part of the **cube-sphere planet surface**, not a separate globe or plane.
  It must be visible and correct in the same frame as terrain from orbit, aircraft, boat and
  first person.
- **No representation/LOD pop** on a recording from orbit → coast → underwater. Water LOD
  changes may change *detail*, never *shape* or *colour regime*.
- Horizon curvature and the planet limb are correct at altitude; the waterline stays continuous
  where terrain meets sea.

### 2.2 Shape and motion

- **Multi-scale waves**: ocean swell (100 m+), wind sea (1–30 m), ripples (cm–m). Detail is
  filtered by distance and screen footprint; no shimmer, no popping.
- **Deterministic low-frequency field** for gameplay and networking: water height + surface
  velocity are a pure function of `(world seed, pinned data version, UTC, position)` — the same
  contract as terrain and weather. High-frequency visual detail may be render-only, but it must
  not affect buoyancy.
- **Weather coupling**: wind speed/direction and storm state drive amplitude, direction and
  whitecaps. Calm, breeze, gale and storm are visibly different.
- **Rivers and lakes**: inland water at terrain height, downhill flow, advected detail, foam at
  rocks and drops; waterfalls are a later extension.

### 2.3 Light and medium

- **Shading**: energy-consistent water BRDF (GGX + Fresnel), sun glitter, sky/atmosphere
  reflection, subsurface/translucency on crests, foam BRDF.
- **Reflections**: screen-space trace of terrain/objects/sky with a correct sky/atmosphere
  fallback; hardware ray tracing where available (Ultra). Reflections must not show a flat
  mirror or a stale probe.
- **Refraction + absorption**: depth-based refraction, Beer–Lambert absorption, scattering;
  shallow water shows the bed, deep water goes opaque. Underwater adds fog, colour shift and
  (later) caustics; the transition at the surface is continuous, not a clip.
- **Shoaling and shore**: depth-based colour ramp, shoreline foam, wet-sand band, wave run-up
  visual. Water must react to the actual `TerrainGrid`/planet height, not a guessed plane.
- **Foam/whitecaps**: wave-steepness and wind driven; boat wakes and prop wash later.

### 2.4 Simulation hand-off

- `ed_sim` exposes a **water query**: height, normal, surface velocity at a body-fixed point,
  valid on client and server. Boats, debris, swimmers and ragdolls use it.
- Physics must be deterministic and cheap: evaluate the analytic low-frequency field, not the
  render mesh.
- Parked boats and floating records persist through `ed_store`; a region reload must restore
  the same float state.

### 2.5 Performance

Every feature has a cost per tier and an off switch. No water pass may allocate per frame.
Targets (M3 Pro, 1440p, High): water shading + reflection pass within the Phase 4 frame budget;
Low tier degrades to cheap normals + sky reflection + no refraction. Water must never be the
reason a scene misses 60 fps without appearing in the profiling breakdown by name.

---

## 3. Non-goals

- **Not** porting the alpha FFT ocean, its shell mesh, its materials or its switches.
- **Not** a full fluid simulation of the ocean. Large-scale flow is analytic/spectral; local
  interaction (boat wakes, splashes) is a bounded effect system.
- **Not** a planar-reflection-of-everything system. Local planar reflections are a possible
  quality option for calm lakes, not the planet ocean.
- **Not** a second water renderer per body: Moon/Mars use the same system with different
  constants (or no water), and the fourth world may use authored industrial coolant/water
  volumes through the same interface.
- **Not** gameplay water before the shell renders correctly (Phase 2 gate).

---

## 4. Candidate techniques to evaluate (decided at the start of Phase 2)

The spike chooses a concrete set; this list is the menu, with a recommended default.

| Concern | Candidates | Recommendation |
|---|---|---|
| **Surface geometry** | Flat clipmap quads on the ellipsoid · screen-space projected grid · planet-node water patches with geomorphing | **Planet-node water patches** (reuse `ed_planet` LOD) + a small camera-local detail grid; projected grid only if node seams fight us |
| **Wave model** | Sum of Gerstner waves · tiling wave-spectrum atlas (FFT-baked offline or at runtime) · stochastic tiling normals · hybrid | **Gerstner low-frequency set + tiling spectrum detail**. A fresh FFT may be used as an *offline-baked detail atlas* if proven; this is a new decision under P1-D18, not salvage |
| **Deterministic gameplay field** | Analytic Gerstner sum · precomputed spectrum LUT sampled by UTC | **Analytic low-frequency Gerstner sum** shared by render and `ed_sim`; cheap and exact on both sides |
| **Reflections** | SSR + sky fallback · hardware RT · planar for lakes | **SSR + atmosphere sky fallback** now; **RT on Ultra**; planar only for authored calm water |
| **Refraction** | Scene colour + depth buffer · refraction pass with roughness-aware blur | **Scene colour + depth**, roughness-aware; no separate full-res pass |
| **Foam** | Texture masks · procedural steepness + shore distance · wake render targets | **Procedural steepness + shore + wind**, with a small wake buffer later |
| **Underwater** | Fullscreen volume + fog · per-pixel absorption · caustic projection | **Absorption/scattering volume + fog**; caustics Phase 8 |
| **Rivers** | Spline ribbon mesh · terrain-carved channel + flow map | **Flow-map ribbon on the `ed_world` river plan**, depth/foam from slope |
| **Buoyancy** | Height query only · height + velocity + pressure | **Height + normal + velocity** from the analytic field; enough for boats and swimmers |

Key constraint: the **simulation field and the render field must agree at low frequency**. The
renderer may add detail the sim does not know, but a boat must sit on the same swell the player
sees.

---

## 5. Integration points

- **`ed_planet`**: supplies the ellipsoid surface, node LOD, terrain height and the water mask;
  water is a surface layer on the same nodes, not a second planet object.
- **`ed_world`**: emits water bodies (sea level, lakes, river splines, flow direction, width)
  and shoreline/landcover hints; deterministic from the same seed/data version.
- **`ed_sim`**: `WaterField` query (height, normal, velocity at time `t`), used by boats,
  debris, swimmers, ragdolls; deterministic replay tests include a floating body.
- **`ed_render::water`**: patch generation/LOD, wave evaluation, shading, reflections,
  refraction, foam, underwater; reads sim field + render-only detail.
- **`ed_gpu`**: needs nothing special beyond the existing RHI (compute for detail if used,
  depth/scene colour access, optional RT). Any new capability is added to the caps probe.
- **`ed_data`/registry**: bathymetry is not required for v1 (depth comes from terrain/sea
  level); if a real bathymetry source is added, it is registered and licence-gated like any
  other dataset.
- **Weather**: wind/storm state is already deterministic; water reads it rather than inventing
  its own.

---

## 6. Phases and gates

| Phase | Water work | Gate |
|---|---|---|
| **1** | Interface stubs: `ed_sim` water query, `ed_render` water module, flat lit patch in the proving ground | Correct sun reflection on a flat patch |
| **2** | Technique choice from §4; ocean as a layer of the planet surface; orbit → shore continuity; sea level; horizon | **Recording: orbit → coast → surface with no representation pop** |
| **4** | Full shading, screen-space reflections with sky fallback, refraction and absorption, foam, lakes and rivers v1, underwater v1 | Coast scene at the High budget; water reads correctly from orbit |
| **5** | Buoyancy, boats and wakes, float replay test | A boat floats the same on client and server; a parked boat survives reload |
| **8** | Storms, caustics, ray-traced reflections on Ultra, wake polish | Storm and calm read clearly |

---

## 7. Acceptance criteria

- [ ] Orbit → coast → underwater recording: no seam, pop, texture-regime switch or waterline break.
- [ ] The planet limb/horizon is correct from altitude; ocean meets land along the real terrain.
- [ ] Swell, wind sea and ripples are all present at the right distances; no shimmer/aliasing.
- [ ] Sun glitter, sky reflection and terrain/object reflections read correctly; no mirror plane.
- [ ] Shallow water shows the bed; deep water absorbs; underwater transition is continuous.
- [ ] Shoreline and crest foam appear from wave/wind/shore state, not a static mask.
- [ ] Rivers/lakes flow downhill with foam at slope breaks; lakes sit at terrain height.
- [ ] `ed_sim` water query is deterministic and replay-tested; a boat floats on the visible swell.
- [ ] Weather changes wind/storm state and the water responds.
- [ ] Every effect has a tier cost and an off switch; water is broken out in the profiler by pass.
- [ ] No alpha FFT/ocean code is ported; the old `ultra-water-port.md` remains a post-mortem only.

---

## 8. Open questions

1. **Wave detail source**: offline-baked spectrum atlas vs runtime analytic + tiling normals —
   decide on visual quality per memory/bandwidth, not on "FFT is bad".
2. **Patch seams**: planet-node water LOD vs a camera-local projected grid — whichever keeps
   the orbit→shore recording clean with the least custom code.
3. **Rivers**: ribbon mesh vs carved channel + flow map for the first playable version.
4. **Bathymetry**: is real bathymetry worth a dataset, or is terrain-below-sea-level enough?
5. **Underwater caustics**: projected texture vs compute in Phase 8; likely tier-gated.
6. **Boat wake model**: render-only for v1, sim-visible later only if gameplay needs it.
7. **Moon/Mars/fourth world**: which bodies get water/liquid, and with what constants?
