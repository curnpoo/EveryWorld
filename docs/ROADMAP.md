# Roadmap

The order of work and the gate each phase must pass. Gates, not dates (D18). Goals are in
`VISION.md`, decisions in `DECISIONS.md`, the design in `ARCHITECTURE.md`.

## Where we are

- `main` is the custom-engine rebuild. The Bevy alpha is frozen on `alpha-early`.
- Phase 0 — Prepare: **complete** (2026-11-10). Exit record and carried items:
  `phases/phase-0.md` §5; budgets from the alpha: `reference/alpha-baseline.md`.
- Phase 1 — Engine base and editor shell: **complete** (2026-11-12), all 12 gate items
  met (`phases/phase-1.md` §7).
- **Current phase: 2 — Planet, far field and streaming** (started 2026-11-12). Detail and
  progress: `phases/phase-2.md` §7.
- Milestones to the playable solo loop: `plans/path-to-solo.md`.

## Why this order

The alpha looked good, ran badly, loaded slowly and had gaps. Those are foundation problems,
so the foundation is built first and in dependency order: the engine base (1), then the
planet with far-field and streaming (2), then world content on the ladder (3). Rendering
quality (4) comes after there is a complete world to light. Cluster geometry runs alongside
as Track G and never blocks a gate. The editor grows a stage in each phase.

## Phases

| # | Name | Builds | Exit gate |
|---|---|---|---|
| 0 | **Prepare** | Frozen alpha with look and perf baselines; determinism harness; spikes; data registry and licence gate; crate skeletons and CI; contracts | Every later phase can start without an unanswered foundation question (`phases/phase-0.md` §5) |
| 1 | **Engine base + editor shell** | `ew`, `ed_gpu` (Metal + minimal second backend), frame graph, GPU instance renderer with the discrete-LOD provider, `ed_pack` and `ew-bake`, fixed-step loop with Box3D, addressing and layer types, editor stage E1, harness | 100k mixed instances at 60 fps on High with per-pass timings; same scene on the second backend; pick and move a prefab and reload it from its layer file |
| 2 | **Planet, far field, streaming and the natural world** (O2, `phases/phase-2.md` §7.1) | `ed_planet` quadtree, horizon culling, virtual-textured terrain, erosion detail, ground material library, trees and ground cover, atmosphere iterated against references, layered volumetric clouds, real satellite imagery, ocean surface v0, sun virtual shadow maps with contact hardening and a far height term (P2-D20), AgX + MetalFX, R3/R4 bake and globe pack, `ew-tiles` v1, staged uploads, editor stage E2 (Moon terrain body moved to 6) | One unbroken recording street-level → orbit → Moon view → back with no holes or representation switch; city lights visible from orbit; spawn-to-complete-view within the budget set in Phase 0 |
| 3 | **World content on the ladder** | `ed_world` refactor to R0–R2 output, streets pipeline, buildings from Overture with the height rule, street trees and park planting on the foliage ladder from Phase 2, override layers applied in generation, deltas, editor stage E3 | Fly from orbit into three reference cities with no pops at 60 fps on High and fixed memory; drive three routes with no road gaps; a landmark placed in the editor is visible from 50 km and survives a re-bake |
| 4 | **Rendering quality** | Local-light VSM, voxel GI, full water, cloud and foliage shading polish, building material library, hardware RT on Ultra | Reference scenes clearly exceed the Phase 0 look captures at the High budget (detail, lighting, clouds, full building fill, solarpunk architecture; P1-D25) |
| 5 | **Simulation and vehicles** | Character mover and rocket boots, car model, planes, boats, parked records, traffic on the nav graph, editor stage E4 (markers, spawners) | Car feel sign-off on a forest road and a city street; park a car, go to orbit, return, and it is there |
| 6 | **Golden path, solo** | Rocket, Moon as a terrain body and Moon landing, saves, housing plot, first missions authored in the editor | The whole north-star loop (`VISION.md` §2) played solo in one recording |
| 7 | **Multiplayer and persistence** | Directory and shards, QUIC protocol, prediction, shared vehicles, co-op mission, sky events, server-side deltas, editor stage E5 | Two clients share a car and a mission; a launch is seen from another region; eight players in a hub on the first server |
| 8 | **Living world** | NPC schedules and dialogue, interiors, destruction, weather, mission variety, polish | The world feels populated on the gaming-PC server |
| 9 | **Platforms and ship** | Full Windows D3D12, iOS Low tier, Android, packaging, attribution, performance gates | Ships on macOS and Windows; mobile tier proven |
| G | **Track G: cluster geometry** (parallel from Phase 1) | Cluster DAG bake, cut selection, compute raster, cluster streaming (`plans/renderer.md` §3) | Adopted per content type when it beats the discrete-LOD provider on the reference scenes |

Phases 1 to 6 are broken into fifteen runnable milestones in `plans/path-to-solo.md`.

Rules: every phase ends runnable. Do not start Phase 7 before Phase 6 passes. Phases 4 and 5
may overlap. A failed gate is recorded with its diagnosis; the gate is not edited to pass.

## Reference scenes and routes

Used by every perf and look comparison from Phase 0 on: Shibuya (dense city), Manhattan
(tall city), Aix-en-Provence (old town), Marrakech (arid), Zermatt (alpine), a dense forest,
open ocean and coast, low flight, the ascent to orbit, and the golden-path route.

## Standing rules

1. **No holes, no switches.** One planet renderer from ground to other bodies.
2. **Measure first.** Every phase records frame time, p99, per-pass GPU time, memory and
   load times on the reference scenes. Every feature has a cost per tier and an off switch.
3. **Deterministic generation.** Pure function of (seed, pinned data version, node, rung).
4. **One address, one height truth.** `NodeId` everywhere above `ed_data`.
5. **Authored content is layers.** Never edit generated output.
6. **Licensing is a shipping gate.** RED data never ships.
7. **Docs stay current.** A `CHANGES.md` entry per landed piece of work; decisions change
   only in `DECISIONS.md`.

## Backlog by area (pulled into phases as they open)

- **World fidelity:** lane-level roads, canopy-height forests, landmarks from tags, bridges
  and tunnels, bathymetry, crops and food systems, soil-driven ground.
- **Living world:** NPC home and work schedules, authored dialogue, wildlife, ambient
  launches, population from open population grids.
- **Interiors:** procedural floor plans and authored interiors with their own streaming tier.
- **Progression:** ranks, trust and merit unlocks; housing plots; activity director.
- **Space:** stations, surface bases, Moon and Mars settlements, travel lanes, the fourth
  world, time compression.
- **Characters:** VRM avatars, customization, realistic and robot NPCs.
- **Rendering:** rain, snow and wet roads, motion blur, caustics, smoke.

## Mapping from the previous roadmap (Phases 0–8 of 2026-11-08)

| Previous | Now |
|---|---|
| 0 Freeze & prepare | 0, with look baseline, far-field and streets spikes, and more contracts |
| 1 Engine skeleton | 1, plus editor shell, instance renderer, second backend, addressing types |
| 2 Planet spike | 2, widened to far field and streaming |
| 3 Renderer core | Split: 3 (world on the ladder), 4 (lighting and water), Track G (cluster geometry) |
| 4 Sim & vehicles | 5 |
| 5 Golden path solo | 6 |
| 6 Multiplayer | 7 |
| 7 Living world & polish | 8 |
| 8 Platforms & ship | 9 |

The older Stages A–H and milestones M0–M10 appear only in `docs/alpha/`.
