# Roadmap

Updated 2026-11-08. Aligned with **`docs/VISION.md`** (what we're building, verbatim goals)
and **`docs/plans/rearchitecture.md`** (audit + full technical plan). This file supersedes
the old Stages A–H roadmap; the mapping and alpha history are at the bottom.

## Where we are

- The playable Bevy alpha is frozen on branch **`alpha-early`** (crash fixes only).
- **`main` is now the custom-engine reboot.** All-in; no parallel Bevy feature work.
- **Current phase: 0 — freeze and prepare.** Next: Phase 1 (engine skeleton) and Phase 2
  (planet spike) in parallel.
- The north star is the full session loop in `VISION.md` §2: forest drive → city → friend +
  co-op mission → rocketport → Moon → back → your car → home → log off. Solo first
  (Phase 5), then multiplayer (Phase 6).

## Phases

| # | Name | Goal | Exit gate | Status |
|---|---|---|---|---|
| 0 | Freeze & prepare | Determinism tests, baselines, vendor study, crate skeleton, data registry started | Repo ready for parallel engine work | **Now** |
| 1 | Engine skeleton | `ew` app + `ed_gpu` Metal backend + fixed-step loop + bake CLI | Test scene renders natively; headless perf harness works | Next |
| 2 | Planet spike | One continuous cube-sphere Earth; ground↔orbit with no switch; procedural Moon | Unbroken ground→orbit→ground recording at budget | Next (parallel) |
| 3 | Renderer core | Cluster DAG + GPU cull + visbuffer + GI + geometric foliage + cel material | Forest + city 60 fps High tier, fixed memory, no streaming holes | |
| 4 | Sim & vehicles | Box3D, character mover + boots, real car, driveable roads v2, parked records | Forest→city drive signoff; park→orbit→return works | |
| 5 | Golden path solo | Rocket, Moon landing, save/load, housing plot | Whole north-star loop solo, recorded | |
| 6 | Multiplayer | Directory + region shards, prediction, shared vehicles, co-op mission, sky events | Two clients share car + mission; launch seen cross-region; 8-player hub | |
| 7 | Living world & polish | NPC schedules, traffic, dialogue, mission variety, destruction, VSM/DDGI, weather, water, MotionBricks eval | World feels populated on a gaming-PC server | Ongoing |
| 8 | Platforms & ship | Windows D3D12, then iOS/Android tiers, packaging, perf gates | Ships on macOS + Windows; mobile tier proven | Later |

Phases overlap; every phase ends playable. Do not start Phase 6 before Phase 5 passes.

**Pacing (owner, 2026-11-09):** this is deliberately a multi-month project — expect roughly
**6–10 months to the full vision**, and that is fine. For calibration: everything in the
`alpha-early` alpha was built in **4 days** with AI coding models, so velocity is high, but
the phase gates exist so speed never strands the project in an unplayable state. Do not
compress the gates to chase dates.

## Phase 0 tasks (this week)

Full detail — philosophy, workstreams, pass criteria, exit gate, failure branches and the
goal mapping — is in **`docs/phases/phase-0.md`**. Summary:

| # | Workstream | Artifact |
|---|---|---|
| W1 | Branch & repo hygiene | Tagged `alpha-early`, clean `main`, untracked triage |
| W2 | Forensic alpha baseline | Dated six-scene + golden-path perf report (fps/p99/VRAM/draws/tris) |
| W3 | Determinism harness | Per-subsystem golden-hash tests, macOS + Windows CI, triaged failures |
| W4 | Dependency spikes | Box3D determinism/perf decision · Solstice port map · GLB+VRM+Megascans import proven |
| W5 | Data registry & licence gate | `data/registry/*.toml`, CI gate, generated `CREDITS.md`, WorldClim/EOX decisions |
| W6 | Crate skeletons & CI | Documented empty crates, enforced dependency edges, green CI matrix |
| W7 | Interface contracts | Generation · sim · asset · persistence contracts in `docs/contracts/` |

Rules: W1 before W2; no feature work; a failed spike takes its pre-written fallback rather than
blocking the week. Exit gate is the checklist in `phase-0.md` §5.

## Phases

## Standing engineering rules

1. **Seamless is non-negotiable.** No loading screens or representation switches between
   ground, orbit and other bodies. One planet renderer, one atmosphere, one cloud field.
2. **Measure first.** Every phase records fps/p99/GPU-pass numbers on the reference scenes
   (city, forest, mountains, ocean, low flight, ascent) plus the golden-path route. Every
   feature declares its cost per tier and has an off switch.
3. **Multiplayer-safe from now on.** Generation is a pure function of (seed, pinned data
   version, UTC). Only deltas and entity state travel. Sim and render state stay separate;
   fixed-step sim, interpolated render.
4. **Deterministic sim.** Box3D replay tests in CI; server authoritative; no lockstep.
5. **Data licensing is a shipping gate.** RED never ships; attribution generated
   (`docs/data/README.md`).
6. **Docs stay current.** `docs/CHANGES.md` entry per landed work; tick this roadmap;
   `VISION.md` changes only when the owner changes the goals.

## Feature backlog by area (pull into phases)

- **World fidelity:** Overture/MS footprints + heights, lane-level roads, canopy-height
  forests, orthophoto ground, landmarks from OSM tags, interiors, bridges/tunnels, bathymetry
  and underwater, real crop/food systems, SoilGrids-driven ground.
- **Living world:** NPC home/work schedules, authored dialogue and lore, traffic, wildlife
  (start with urban birds), ambient launches, spaceport schedules, population from WorldPop.
- **Interiors:** procedural floor plans + authored interiors, instanced interior parts with
  their own streaming tier, walk-in buildings with no door-teleports (VISION §12).
- **Progression:** ranks/trust/merit unlocks for tech and transport; housing plots and
  building; activity director (guided, never overwhelming). No currency in the core loop.
- **Space:** space stations, surface bases, Moon/Mars civilizations, travel lanes with
  beacons and scheduled traffic, **a fourth human-manufactured industrial world** (concept
  TBD), procedural planets and a seeded galaxy (later), time compression.
- **Characters:** VRM avatar import, avatar material sandbox, customization, MotionBricks
  evaluation, realistic + robot NPCs.
- **Rendering:** VSM, DDGI/RT, rain/snow/wet roads, motion blur, night lights, coupled ocean,
  destruction effects and budget-gated smoke.
- **Platforms:** Windows D3D12 + DLSS/FSR3/RTX, iOS then Android, signing/packaging.

## Mapping to the old Stages A–H

| Old | New |
|---|---|
| A stabilize/measure | Phase 0 (baseline + determinism only; Bevy bug-fixing dropped) |
| B fast (M1/M2 wgpu) | Replaced by Phases 1–3 (no wgpu detour) |
| C together | Phase 6 (+ `ed_store` in Phase 5) |
| D native + Nanite-like | Phases 1 + 3 |
| E lighting | Phases 3 + 7 |
| F play | Phases 4 + 7 |
| G space | Phases 2 + 5 (+ Phase 7 for stations/civilizations) |
| H platforms | Phase 8 |

## Alpha history (summary, preserved on `alpha-early`)

Playable real-Earth world: WGS84/ECEF math, slippy streaming, Mapterhorn elevation,
OpenFreeMap vectors, procedural terrain/buildings/trees with 17 regions, roads v2 solver,
robot courier with rocket boots/skydive/wingsuit, cars and planes as prototypes, engine
audio port, volumetric clouds, atmosphere, FFT ocean, native Metal renderer with GTAO/SSR/
probe GI/MetalFX, ghost multiplayer (protocol v5), delivery mission planner, lookdev and
headless perf tooling. Known alpha measurements: Shibuya street level 27–32 fps settled,
unplayable during streaming; GPU-bound with per-tile merged geometry. Those numbers are the
baseline the new renderer must beat, not a target to keep tuning.
