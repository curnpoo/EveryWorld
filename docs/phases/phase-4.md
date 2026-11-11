# Phase 4 — Rendering quality

Status: follows Phase 3; may overlap Phase 5. Roadmap row: `../ROADMAP.md`. Design:
`../plans/renderer.md` §5, `../plans/water.md`. Milestone M11 in
`../plans/path-to-solo.md`. Decisions in the P4-D series are **proposed** until the owner
moves them into `../DECISIONS.md`.

## 1. Goal

Light the complete world of Phase 3 so that every reference scene clearly exceeds its alpha
look capture (P1-D25), at 60 fps on High. Each advanced feature is added beside the baseline
path of F9 and adopted on a tier only when measured to fit.

## 2. What Phase 4 proves

| Question | Proof |
|---|---|
| Do virtual shadow maps fit an 18 GB machine? | VSM on in all reference scenes inside its time and page-pool budget; otherwise it stays on Ultra |
| Does dynamic GI fit? | Voxel GI inside its budget with the update rate chosen by measurement |
| Is the look above the alpha floor? | The comparison procedure of P4-D2, signed by the owner for all six places |
| Is water one system from orbit to underwater? | The acceptance list in `../plans/water.md` §7, items for Phase 4 |
| Does the baseline survive as Low? | Every reference scene at 60 fps on the Low tier settings with all advanced flags off |

## 3. What Phase 4 does not do

No buoyancy or boats (Phase 5). No storms, caustics or wet-weather surfaces (Phase 8). No
D3D12, DLSS or FSR (Phase 9); the upscaler sits behind an interface. No cluster geometry on
the critical path (Track G). No new world content except materials and the authored kit.

## 4. Decisions in detail (proposed)

### P4-D1 — Adoption by measurement
Each feature has a caps flag, a cost per tier and an off switch (P1-D10). A feature becomes
the default on a tier when the harness shows the tier's frame and memory budget held on all
reference scenes with it on. A feature that misses High stays Ultra and does not block the
gate (path-to-solo M11).

### P4-D2 — How "clearly exceeds" is judged
For each of the six places in `../reference/alpha-baseline.md` §4: the same position,
heading, pitch, solar hour and date (2026-11-10) are rendered by `ew --headless` at 2560 ×
1440 on High after the view is complete. A board places the alpha capture and the new
render side by side. The owner marks five criteria per place, each as worse, equal or
clearly better: detail and fidelity; lighting (including ambient light in shadow); clouds;
building fill; architecture. The gate needs "clearly better" on all five for all six, with
the frame inside the High budget. Supporting numbers on the board, not substitutes for the
judgement: fraction of source footprints drawn (must be 100%), frame time, per-pass times.
A second board at dusk and at night is recorded but does not gate.

### P4-D3 — Baseline hardened first
Cascaded sun shadows with a far terrain-shadow term from height data, sky ambient from the
atmosphere tables, GTAO, screen-space reflections with sky fallback. This is completed and
budgeted for Low before any advanced feature starts, so there is always a shippable frame.

### P4-D4 — Virtual shadow maps
A sun clipmap of virtual pages, 128² texels each, backed by one physical depth atlas: 8192²
on High (256 MB), 16384² on Ultra. Pages are requested from the depth buffer of the
previous frame, rendered through provider 1 with per-page culling, and cached while no
instance or light moves over them. Far rungs (R2 and above) cast through the terrain-shadow
term, not pages. *Riskiest item of the phase; done first.* *Revisit:* if page rendering
exceeds budget in a city, cap pages rendered per frame and accept one-frame-late shadows.

### P4-D5 — Voxel GI
Camera-centred clipmap cascades of radiance voxels. Voxelisation reads R1 shells, the R2
proxy and terrain, never R0 parts, so its cost does not follow instance count. One cascade
is refreshed per frame. Final gather is at half resolution with temporal accumulation. Far
rungs are lit by sun and sky only (`renderer.md` §5). Cascade count, resolution and update
rate per tier are set by the first measurement on Shibuya and the forest.

### P4-D6 — Reflections
Screen-space trace with the atmosphere as fallback on Low and High. Hardware ray tracing on
Ultra where `Caps` reports it, against R1 shells and terrain.

### P4-D7 — Water
The recommendations of `../plans/water.md` §4 are taken as the design: water patches on
planet nodes with a camera-local detail grid; a low-frequency Gerstner set plus tiling
spectrum detail; reflections per P4-D6; refraction from scene colour and depth with
Beer–Lambert absorption; procedural foam from steepness, shore distance and wind; lakes at
terrain height and rivers as flow-map ribbons; an underwater absorption volume. The
low-frequency field is one engine-free function used by the renderer now and by `ed_sim` in
Phase 5. It replaces Phase 2's ocean v0 without changing node selection.

### P4-D8 — Volumetric clouds
A ray march of the Phase 2 cloud field within a range of the camera, at half resolution
with temporal reprojection, blended into the shell beyond it. One field, two renderings;
the shell and the volume must agree in coverage and lighting at the blend.

### P4-D9 — Material library
Materials are data files (P1-D12). Megascans surfaces enter through `ew-bake` with a
registry entry each; they ship only inside packs (D8) and the licence gate enforces it.
Ground materials are chosen by land cover, biome and lithology tables. Texture pages stream
under a fixed pool.

### P4-D10 — Foliage shading
Two-sided transmission, per-instance tint from the genome, wind from a deterministic field,
and the canopy colour of R3 matched to the lit colour of R0 at the switch distance.

### P4-D11 — Anti-aliasing and upscaling
Temporal anti-aliasing with MetalFX temporal upscaling on Metal, behind an `Upscaler`
interface so D3D12 back ends can add theirs in Phase 9. Render scale is a tier setting.

### P4-D12 — Exposure and tone
Automatic exposure from a luminance histogram with limits per time of day; the tone curve
ported in Phase 1; bloom. Night scenes must show emissive without crushing shadow detail.

### P4-D13 — Authored architecture
The first regional kit family replaces the procedural kit by id through the kit pack
(P3-D2), with its hero prefabs and sockets (P3-D2a, P3-D2b). The architecture criterion of
P4-D2 is judged against the owner's references in `../solarpunk/reference/`: buildings
that sit in the ground, planting on the building, visible energy, curves beside timber and
glass. It cannot pass on the procedural placeholders.

## 5. Budgets (M3 Pro, 1440p output)

Derived by splitting 16.6 ms; the geometry figure comes from the Phase 3 gate measurement.

| Pass | High (ms) | Low (ms) |
|---|---|---|
| Culling, depth and visibility | 3.0 | 3.0 |
| Shadows | 2.5 (VSM) | 1.5 (cascades) |
| Indirect light | 2.5 (voxel GI) | 0 (sky ambient) |
| Ambient occlusion | 0.7 | 0.7 |
| Material resolve and lighting | 2.5 | 2.0 |
| Atmosphere and clouds | 2.5 | 1.5 (shell only) |
| Water | 1.5 | 0.8 |
| Upscale, anti-aliasing, post | 1.2 | 1.0 |
| **Total** | **16.4** | **10.5** |

| Memory | High | Ultra |
|---|---|---|
| Shadow page atlas | 256 MB | 1 GB |
| GI volumes | 300 MB | 800 MB |
| Material textures | 2.5 GB | 4 GB |
| GPU total | 8 GB | not bound to the 18 GB machine |

## 6. Workstreams

| # | Scope | Owns | Done when |
|---|---|---|---|
| W1 | Baseline hardening and Low tier | `ed_render/src/shadows_csm*`, `gtao*`, `ssr*` | Low budget held on all scenes |
| W2 | Virtual shadow maps | `ed_render/src/vsm*` | Page pool never exhausted on the reference paths; budget held or feature parked on Ultra with numbers |
| W3 | Voxel GI | `ed_render/src/gi*` | Same |
| W4 | Water | `ed_render/src/water*`, `ed_world/src/water*` (bodies, river plan), an engine-free `waterfield` module | Phase 4 items of `water.md` §7 |
| W5 | Volumetric clouds | `ed_render/src/clouds*` | Shell and volume agree at the blend in a capture test |
| W6 | Material library and Megascans ingestion | `ed_assets`, `data/materials/`, `data/registry` | Licence gate passes; pool never exceeded |
| W7 | Foliage shading and ground cover lighting | `ed_render/src/foliage*` | Forest capture passes P4-D2 |
| W8 | Upscaler, anti-aliasing, exposure, post | `ed_render/src/post*` | No ghosting on the scripted fast-pan test |
| W9 | Kit family 1 integration | `data/kit/`, `data/styles/` | Every part id of the procedural kit has an authored replacement or a recorded reason |
| W10 | Hardware ray-traced reflections (Ultra) | `ed_gpu` caps and acceleration structures, `ed_render/src/rt*` | Runs where `Caps` allows; off elsewhere |
| W11 | Look boards and tier harness | `tools/`, `ew` harness | One command produces the boards and the tier table |

## 7. Order

```
W1 ──> W2 ──> W3 ──┐
W4 ───────────────┤
W5 ───────────────┼──> W11 ──> owner review ──> gate
W6 ──> W7, W9 ────┤
W8 ───────────────┘        W10 after W3, never gating
```

W2, W4, W5, W6 and W8 touch separate files and can run as parallel sessions once W1 lands.

## 8. Exit gate

- [ ] Reference scenes clearly exceed the Phase 0 look captures at the High budget (detail,
      lighting, clouds, full building fill, solarpunk architecture; P1-D25), judged by
      P4-D2.
- [ ] 60 fps on High in all reference scenes with the adopted features on.
- [ ] Low tier at 60 fps with every advanced flag off.
- [ ] Each feature has a recorded cost per tier and an off switch.
- [ ] Water passes its Phase 4 acceptance items.
- [ ] Licence gate passes with the material library in the packs.
- [ ] `CHANGES.md` has the tier table and the boards.

## 9. Failure branches

| If | Then |
|---|---|
| VSM misses High | Cascades stay on High with the far terrain-shadow term; VSM on Ultra |
| Voxel GI misses High | Lower update rate, then resolution; if still over, sky ambient plus GTAO stays on High |
| Water reflections cost too much | Half-resolution trace; sky-only on Low |
| Clouds volume flickers under reprojection | Longer history with motion rejection; widen the shell blend |
| No authored kit arrives | The gate stays open on the architecture criterion; everything else is recorded as passed |
| Texture pool exceeded | Lower the top mip per material class before lowering the pool |

## 10. Needed from outside the code

| Need | From |
|---|---|
| Megascans access under the Fab licence, and the list of surfaces | Owner |
| First regional kit family as Blender prefabs on the 2 m grid | Art |
| Owner's judgement on the boards | Owner |
| Confirmation of F9 | Owner |

## 11. What Phase 5 and 6 receive

A lit world at budget on two tiers, a water field the simulation can query, and a frame
whose per-pass costs are known, so gameplay effects can be costed against it.
