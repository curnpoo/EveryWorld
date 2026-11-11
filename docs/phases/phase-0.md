# Phase 0 — Prepare

Status: **current phase.** Roadmap row: `../ROADMAP.md`. Decisions referenced by id are in
`../DECISIONS.md`.

## 1. Goal

Leave the alpha behind with everything worth keeping recorded, and answer every question the
engine's foundation depends on, so that Phase 1 is construction and not discovery.

Phase 0 writes no engine features. It produces measurements, tests, small throwaway spikes,
empty crates, CI and contracts.

## 2. Why each part exists

| Problem with the alpha or the old plan | Phase 0 answer |
|---|---|
| "Looks pretty good" is not recorded anywhere a new renderer can be compared to | W2 look-reference captures |
| "Runs not great, loads slowly, has gaps" has no numbers | W2 performance, load-time and gap baseline |
| Shared world depends on deterministic generation that was never tested | W3 golden hashes |
| The plan depended on young external code (Box3D v0.1, a week-old renderer repo) | W4 spikes with pass criteria |
| Far-field, streaming and street data had no design | W4d and W4e spikes; W7 contracts |
| Two datasets cannot ship; the registry had licence errors | W5 |
| Many parallel coding sessions can couple crates by accident | W6 enforced dependency rules |
| Two sessions can build two halves of a seam that disagree | W7 contracts written first |

## 3. Workstreams

Each has steps, an artifact and a done condition. W1 comes first. The rest can run in
parallel, except that W2 needs the frozen alpha from W1 and W7 uses the results of W4.

### W1 — Repository and branches

1. Land this documentation set on `main`.
2. Tag the alpha (`alpha-freeze-0.15`) on `alpha-early` after its pending changes
   (`ed_game/src/main.rs`, `docs/CHANGES.md`, untracked `assets/`) are committed or
   deliberately discarded by the owner. `Solarcat.glb` stays uncommitted unless the owner
   says otherwise.
3. On `main`, keep docs and engine-free crates. Remove `ed_game` and the vendored
   `bevy_pbr` once W2 is captured (they remain on `alpha-early`). Keep `ed_metal` and the
   alpha `ed_gpu` sources under `reference/alpha_metal/`, outside the workspace, as the
   shader-maths source for P1-D18.

**Artifact:** tag; clean `main`. **Done when:** `git status` is clean on `main` and
`alpha-early` builds at the tag.

### W2 — Alpha baseline: look, performance, loading, gaps

Run on the M3 Pro with the native Metal path, on the reference scenes in `../ROADMAP.md`.

1. **Look reference.** Fixed camera, fixed time of day, per scene: final image, plus the
   atmosphere, cloud, terrain-material and tone-mapping stages in isolation where the alpha
   allows. Stored in `docs/reference/look/` with the exact command for each.
2. **Performance.** Per scene: steady frame time and p99, per-pass GPU time
   (`tools/gpu_passes.sh`), triangles, draws, GPU and process memory.
3. **Loading.** Time from launch to first complete view; time to full detail at a
   standstill; worst frame while flying a fixed path at aircraft speed.
4. **Gaps.** A written list with screenshots of every hole, seam and pop on a fixed
   orbit-to-street path (terrain cracks, missing tiles, road breaks, the globe switch).
5. Write `docs/reference/alpha-baseline.md`. From it, set the Phase 2 and Phase 3 budget
   numbers for load time and worst frame.

**Artifact:** captures, CSVs, baseline report. **Done when:** every later "better than the
alpha" claim can cite a file here.

### W3 — Determinism harness

1. Canonical inputs: six real places (dense city, alpine, desert, forest, coast, farmland)
   with their source data stored as fixtures, plus two synthetic inputs that need no data.
2. Hash per subsystem (terrain heights, surface map, roads, buildings, foliage, props),
   quantised (positions to 1 mm) so real divergence is separated from float noise.
3. Run twice in one process, across two processes, and on macOS and Windows CI.
4. Classify each failure: fix now, or quarantine with a note and a named fix phase.
5. Write the audit: every source of nondeterminism found (map iteration order, thread-order
   reductions, unseeded randomness, wall clock).

**Artifact:** test suite, CI job, audit note. **Done when:** CI is green on both platforms or
every red is classified.

### W4 — Spikes

Throwaway code in `spikes/`, outside the workspace. Each has pass criteria and a written
verdict in `docs/reference/spikes/`.

**W4a — Box3D.** Build it; run a 1,000-body stack and 20 simple vehicles at 60 Hz; hash the
final state across runs, processes, and macOS versus Windows; measure step cost; confirm
character mover, heightfield collider, wheel or raycast support and continuous collision.
Compare the C library through FFI with the Rust port. *Pass:* identical hashes across runs
and processes on one machine, needed features present, step cost leaves room on the M3 Pro.
Cross-machine results are recorded and feed F10. *Decision:* vendored C or Rust port.

**W4b — Geometry references.** Run Solstice on the M3 Pro and record what works, frame
times and memory (F12). Build a cluster LOD for one scanned mesh and one building with
meshoptimizer's cluster-LOD builder; record bake time and determinism. Check 64-bit atomics
and argument-buffer tier on the M3 Pro. *Output:* a Track G plan with measured numbers; a
list of ideas worth porting. This spike cannot fail the phase.

**W4c — Asset import.** Load the owner's GLB (skeleton, weights, materials); test retargeting
to the standard humanoid; read a sample VRM; convert two or three Megascans assets to glTF
and compressed textures with licence metadata. Export one building prefab from Blender with a
manifest (footprint, anchor). *Pass:* the character renders skinned in a throwaway viewer;
one scan and one prefab convert with provenance.

**W4d — Addressing and far field.** Implement candidate cube-to-sphere mappings in a test;
measure node area and shape variation; choose one. Test edge agreement between neighbouring
nodes and faces. Bake R3/R4 layers (terrain height, albedo, night-light emissive) for a
sample of L0–L6 nodes including dense, empty and ocean nodes; extrapolate the globe pack
size. *Pass:* a chosen mapping with tests; a measured pack-size estimate; a decision on
layer resolution per level.

**W4e — Street data.** Extract one city from the pinned Overture release with DuckDB; join
OSM tags; convert to the roads v2 input; compare against the vector-tile input for
continuity across tile edges and for tags gained. Record extract size per km². *Pass:* the
existing solver runs on the Overture graph for that city; comparison note written.

**W4f — RHI shape.** Draft the `ed_gpu` public interface as a Rust trait file with no
implementation. Walk it against Metal, D3D12 and Vulkan for: resource states, bindless
tables, indirect draws with count, compute, timers, presentation. *Output:* the RHI contract
(W7) and the choice of second backend for Phase 1.

### W5 — Data registry and licence gate

1. Create `data/registry/*.toml` (schema in `../data/README.md` §3) for every source in use
   and every source the briefs name.
2. Licence gate: a check that fails any pack manifest naming a RED dataset or a YELLOW one
   without review notes. Wired into CI.
3. Attribution generator: `CREDITS.md` and the in-game credits resource.
4. P0 choices recorded with licence URLs: climate normals to replace WorldClim; far-field
   imagery to replace the EOX mosaic; night lights; the building-height rule.
5. ODbL position for baked chunks written down for legal review.
6. Pin one Overture release and one elevation snapshot as "data version 1".

**Artifact:** registry, gate, `CREDITS.md`, decision notes. **Done when:** the gate runs in
CI and no planned shipping path names a RED source.

### W6 — Workspace, crates, CI

1. Create the new crates from `../ARCHITECTURE.md` §3 as empty libraries, each with a doc
   comment stating its one responsibility and allowed dependencies.
2. A dependency check in CI that fails on a forbidden edge (prove it once by adding one).
3. Shared lints, formatting, macOS and Windows build and test; tests stay fast and offline.
4. `xtask` commands: `check-deps`, `licence-gate`, `credits`, `golden`.
5. Move slippy `TileId` use behind `ed_data`'s boundary where engine-free crates expose it,
   or list the call sites as Phase 3 work.

**Artifact:** building workspace, green CI. **Done when:** `cargo build --workspace` passes
and the dependency check has been seen to fail on purpose.

### W7 — Contracts

Short documents in `docs/contracts/`, each with types, invariants and a test list. They are
the review checklist for Phases 1 to 3.

| Contract | Content | Source |
|---|---|---|
| `addressing.md` | `NodeId`, `ObjectId`, cube mapping, edge ownership, frame tree | `plans/world.md` §1, §4.1; W4d |
| `generation.md` | What `ed_world` emits per rung; determinism rules; the building-height rule; interior hooks | `plans/world.md` §2 |
| `pack.md` | `.ewpack` container, chunk kinds, versions, hashing, the geometry slot, source extracts | `plans/world.md` §3.3; `plans/renderer.md` §3 |
| `layers.md` | Override layer file format and operations; delta vocabulary | `plans/world.md` §4 |
| `rhi.md` | `ed_gpu` public interface | W4f |
| `sim.md` | Fixed step, handles, snapshots, replay, parked records, water query | `../ARCHITECTURE.md` §9 |
| `persistence.md` | `ed_store` records; solo save equals server schema | `../ARCHITECTURE.md` §10 |

**Done when:** Phase 1 and Phase 2 work can be split across sessions without either needing
to ask the other a question the contracts do not answer.

## 4. Order

```
W1 ──> W2 ──────────────────────────────┐
  └──> W3, W5, W6 (independent)         ├──> exit review
  └──> W4a..f ──> W7 ───────────────────┘
```

A spike that fails its pass criteria is recorded and its branch in §6 is taken. It is not
extended until it passes.

## 5. Exit gate

**Phase 0 closed on 2026-11-10.** `[x]` done; items that moved are named with where they
went. Work is on the `phase-0` branch.

- [x] Docs landed on `main`; alpha tagged; `main` clean. *Tag `alpha-freeze-0.15` is the
      last commit of `alpha-early` (cac7988). The owner's uncommitted crash fix in their
      checkout is not in the tag; it stays the owner's to commit.*
- [x] Performance, load-time and gap baselines committed; Phase 2 and 3 budgets set from
      them (`../reference/alpha-baseline.md`). *Look-reference images for six places are in
      `../reference/look/`. Main finding: a
      200-tile residency cap below a 430-tile selection makes the alpha rebuild tiles
      forever. The orbit-to-street gap walk and per-pass GPU timings move to the Phase 1
      harness.*
- [x] Golden-hash suite in CI on macOS and Windows; every failure classified. *macOS passes.
      Windows matches on 65 of 72 hashes, all terrain included; the 7 street and foliage
      hashes are quarantined to Phase 3 with a cause (`../reference/determinism.md`).*
- [x] Box3D verdict with determinism and cost numbers; backend form chosen (P0-R3).
      *Cross-machine comparison moves to Phase 7, under F10.*
- [x] Geometry reference study written; Track G plan has measured inputs (P0-R6).
      *Solstice frame rate not measured; it is reference only (F12).*
- [x] Owner's character inspected; import requirements recorded (P0-R7). *Viewer, scan and
      prefab conversion move to Phase 1 with `ed_assets` (milestone M3).*
- [x] Cube mapping chosen and tested; globe pack size measured (P0-R1, P0-R2).
- [x] Overture street extract runs through the existing solver for one city (P0-R4).
- [x] RHI interface drafted and reviewed against three APIs; second backend chosen (P0-R5).
- [x] Registry populated; licence gate and `CREDITS.md` in CI; P0 data choices recorded;
      data version 1 defined. *28 of 31 licences checked against their sources, with
      corrections to seven entries. Overture and GEBCO are pinned; the rest are pinned when
      acquired in Phases 2 and 3, and the strict gate (`EW_GATE_SHIP=1`) blocks any pack
      built before that.*
- [x] Workspace builds with the new crates; dependency check enforced.
- [x] Seven contracts written.
- [x] `ROADMAP.md` marks Phase 0 complete.

**Carried forward**

| Item | Owner of the next step | Where |
|---|---|---|
| Legal review of the ODbL position and the JPL element table | Owner, with counsel, before any public build | `../data/odbl-position.md`, `data/registry/jpl-elements.toml` |
| F9, F10, F11 | Owner confirmation | `../DECISIONS.md` |
| Uncommitted crash fix on `alpha-early` | Owner | Owner's checkout |
| Character viewer, scan and prefab conversion | Phase 1, M3 | `phase-1.md` |
| Gap walk and per-pass GPU timings, alpha and new renderer on one path | Phase 1 harness | `phase-1.md` |
| Windows-only street and foliage hashes | Phase 3 generator refactor | `../reference/determinism.md` row 8 |
| Release pins for the remaining datasets | At acquisition, Phases 2 and 3 | `../data/README.md` §7 |

## 6. Failure branches

| If | Then |
|---|---|
| Box3D is not repeatable on one machine, or too slow | Keep the `ed_sim` interface; evaluate Jolt (C++) and Rapier behind it; record the cost |
| Box3D differs across machines only | Proceed. Server authority covers it (F10); note it for Phase 7 |
| Solstice does not run or underperforms on Apple GPUs | No change to the plan; Track G proceeds from meshoptimizer and published techniques |
| Golden hashes show widespread nondeterminism | Fix the generators at the start of Phase 3, where they are refactored anyway; fix anything Phase 2 terrain depends on immediately |
| The globe pack is much larger than 2 GB | Ship fewer levels and let the tile service start at a coarser level; or lower layer resolution per level |
| Overture extract loses information the vector tiles had | Keep Overture as the graph and add the missing fields from OSM tags; record which |
| The owner's rig cannot retarget | Use it as art reference; re-skin onto the standard skeleton |
| The RHI draft cannot express something D3D12 or Vulkan requires | Change the draft now; that is the purpose of W4f |

## 7. What Phase 1 receives

A tagged alpha and a baseline to beat; a deterministic generator or a list of where it is
not; verdicts on physics, geometry references and asset import; a chosen node mapping and a
measured globe pack; a street data path; a reviewed RHI interface; a licence-clean data
version; an enforced crate graph; and seven contracts.
