# Phase 0 — Freeze & Prepare (detailed)

Status: **current phase** (started 2026-11-09). Phase doc, not a plan rewrite: the roadmap
summary lives in `docs/ROADMAP.md`, the full technical plan in
`docs/plans/rearchitecture.md`, and the goals in `docs/VISION.md`.

Timebox: **~1 week of focused work**, parallelisable, and intentionally **no engine features**.
Exit gate: repo ready for parallel engine work (checklist §5).

---

## 1. Philosophy — why Phase 0 exists

### 1.1 The alpha answered "can we?"; Phase 0 answers "can we keep it?"

The Bevy alpha proved the fantasy in four days: real Earth streaming, real elevation and roads,
rocket boots, a native Metal renderer, volumetric clouds, ghost multiplayer. It also produced
exactly the debt a prototype is allowed to produce:

- three renderers that must all be changed for any visual improvement,
- per-tile merged geometry that caps density and costs 27–32 fps in Shibuya,
- two Earth representations with a hard switch at ~200 km (the core fantasy fails),
- physics baked into a nondeterministic engine, with no server authority,
- no asset pipeline, so no Megascans photorealism,
- two datasets that cannot legally ship.

None of that is a failure. It is the normal price of discovering the design at speed. Phase 0
is where we **harvest the lessons, keep the reusable core, and stop paying the interest**.

### 1.2 Four principles

1. **Evidence before architecture.** No new subsystem is adopted on reputation. Baseline
   numbers, golden hashes and dependency spikes come first. A week of measurement now is
   cheaper than a month of rebuild later.
2. **Freeze, don't delete.** The alpha stays playable on `alpha-early` forever. It is the
   reference build, the A/B comparison, and the salvage yard. Nothing is lost by leaving.
3. **One-way doors first.** Three decisions are nearly impossible to reverse later, so they
   get settled (or proven feasible) before any dependent code exists:
   - **licensing** — you cannot un-ship a restricted dataset;
   - **determinism** — retrofitting it into a float generator is a rewrite;
   - **representation** — the planet/coordinate/asset-format contracts everything sits on.
4. **Parallel-ready repo.** The project runs on many concurrent AI sessions. Phase 0 makes
   crates disjoint, dependency direction mechanically enforced, and docs authoritative, so
   parallel work cannot silently collide or diverge.

### 1.3 The brake that makes speed safe

The owner's stated risk is not slowness, it is *vibecoded sprawl*: four days produced a
prototype, and the vision is 6–10 months. Phase 0 is the deliberate brake — a short, boring,
measurable week that converts momentum into foundations. Every later phase inherits its
evidence: Phase 3 can claim "2× the alpha", Phase 6 can claim "deterministic", Phase 8 can
claim "shippable", only because Phase 0 recorded what those words mean.

---

## 2. What Phase 0 is not

- **Not engine code.** Only empty crates, CI and throwaway spikes.
- **Not feature work** of any kind, on any branch.
- **Not bug fixing.** Alpha visual bugs are replacement criteria, not tasks
  (`docs/alpha/BUGS.md`).
- **Not content.** No buildings, no missions, no art production beyond import tests.
- **Not a redesign.** Decisions are locked (`docs/MASTER.md` §1); Phase 0 validates them and
  produces fallbacks, it does not reopen them.

---

## 3. Workstreams

Seven streams, each with a purpose, concrete steps, a committed artifact and a definition of
done. They can run in parallel; **W1 must finish before W2** (baseline must be captured on the
frozen alpha).

### W1 — Branch & repository hygiene

**Why.** `main` is about to become a different engine. History must stay readable and the alpha
must stay reproducible; untracked work is how WIP silently rots.

**Steps**
1. Confirm `alpha-early` exists at the last alpha commit and contains the WIP (impostors,
   loading, player, vehicles, tilegen). **Done** (`cac7988`).
2. Tag it (`alpha-freeze-0.15`) so the reference build is addressable forever.
3. Triage untracked work: `assets/characters/Solarcat.glb` stays uncommitted by owner decision;
   anything else either lands on `alpha-early` or is deleted with a note.
4. Clean `main`: only docs + engine-free crates; delete Bevy-only crates **after** the salvage
   copy is verified (they remain on `alpha-early` anyway).
5. Add a short `docs/alpha/README.md` pointer explaining the archive (**done**).

**Artifact.** Tagged alpha, clean `main` status, nothing untracked except the owner's GLB.
**Done when.** `git status` on `main` is clean and `alpha-early` builds.

### W2 — Forensic baseline of the alpha

**Why.** Every later performance claim is relative. Without a recorded baseline, "faster" is an
opinion, and a regression in a surviving engine-free crate goes unnoticed.

**Steps**
1. Run `tools/perf_headless.sh` (six scenes: city, forest, mountains, ocean, low flight,
   ascent) plus the golden-path route on the alpha, on the M3 Pro.
2. Capture per scene: fps steady-state, p99 frame time, streaming-time frame time, GPU pass
   timings, VRAM, draw calls, triangles, resident tiles.
3. Record the known failure modes explicitly: Shibuya 15–23M tris / 474 draws / 27–32 fps
   settled, unplayable while streaming, hard switch at ~200 km.
4. Store the raw CSV + a written report in `docs/alpha/perf/` (dated), summarised in
   `docs/alpha/`.

**Artifact.** Dated baseline report + raw CSVs committed.
**Done when.** Every Phase 3 comparison can quote a number from this file.
**Risk retired.** "We improved it" with no evidence; losing the alpha's perf lessons.

### W3 — Determinism harness (golden hashes)

**Why.** The whole shared universe rests on generation being a pure function of
(seed, pinned data version, UTC). The alpha generator was not audited for this. Retrofitting
determinism into a large float/hash generator is one of the most expensive mistakes available,
so it gets measured before anything depends on it.

**Steps**
1. Define a canonical input set: ~6 tiles covering dense city (Shibuya), alpine (Zermatt),
   desert (Marrakech), forest, ocean/coast, and a flat agricultural tile; plus 2 synthetic
   tile ids that need no network.
2. Hash **per subsystem** (terrain heights, surface map, roads, buildings, foliage, props) so
   a failure localises instead of just "the tile differs".
3. Quantise before hashing (e.g. positions to 1 mm, colours to 8-bit) to separate genuine
   nondeterminism from harmless float noise; record the tolerance in the test.
4. Run twice in one process, twice across processes, and on macOS + Windows CI.
5. Any failure is classified: *fix now* (cheap, blocks multiplayer), *quarantine with owner*
   (documented, fixed before Phase 4), or *design intent* (e.g. UTC-driven weather, excluded
   from the hash).
6. Land the harness in `ed_world` tests + a CI job (`cargo test -p ed_world --features
   golden`) with pinned fixture data committed for the synthetic tiles.

**Artifact.** Golden-hash test suite + CI job + a short determinism audit listing every
nondeterministic source found (HashMap order, thread-order reductions, unseeded RNG, wall
clock).
**Done when.** CI is green on macOS + Windows, or every red is triaged with a dated fix owner.
**Risk retired.** A multiplayer universe that desyncs by construction.

### W4 — Dependency spikes (the three bets)

**Why.** Three external bets could invalidate the architecture if they are wrong: Box3D
(physics), Solstice (renderer techniques), and the character/asset import path. Each gets a
throwaway spike with **explicit pass/fail criteria**, before any production code.

#### 4a. Box3D (physics)

1. Clone `erincatto/box3d` (MIT), build its samples on macOS.
2. Write a throwaway 1,000-box tower + 20 simple vehicles scene; step at fixed 60 Hz; run the
   same scene twice in one process, across two processes, and record the final state hash.
3. Measure step cost single-threaded and multithreaded; check availability of character mover,
   wheel joints, heightfield colliders, CCD, recording/replay.
4. Evaluate `box3d-rust` (MIT) on the same scene for parity and ergonomics.
5. **Pass criteria:** identical state hashes across runs/processes, vehicles + character mover
   present, and a 60 Hz step budget that leaves room for gameplay on the M3 Pro.
6. **Decision recorded:** vendored C17 vs Rust port vs abstraction-only. Regardless of the
   answer, `ed_sim` exposes our own API so the choice is reversible.

**Artifact.** Spike repo/branch + measurement note + decision paragraph in
`docs/ARCHITECTURE.md` or a new `docs/alpha/spikes/box3d.md`.
**Risk retired.** Nondeterministic or unusably slow physics discovered in Phase 4 with vehicles
already built on it.

#### 4b. Solstice (renderer techniques)

1. Clone `SamG-Coder/solstice` (MIT), run the viewer/demo, profile the sample scene.
2. Read the builder and renderer; produce a **port map**: what ports conceptually as-is
   (cluster DAG, cut selection, two-phase occlusion, software raster, voxel GI, paging), what
   needs Metal-specific rewriting, and what does not exist there at all (dynamic objects,
   skinning, motion vectors, translucency, planet VT).
3. Check the specific gaps our engine needs: 64-bit atomics on Metal, bindless argument
   buffers, tier behaviour on Apple silicon, memory ceiling of the GI clipmap.
4. **Pass criteria:** the techniques are understood well enough to write a build plan for
   `ed_render`, and no blocker invalidates Metal-first.
5. **Failure branch:** if a technique proves Metal-hostile or the GI cost is out of budget at
   the High tier, choose the fallback (cached GI probes / DDGI-style alternative) now, while it
   is still a paragraph instead of a rewrite.

**Artifact.** Port map + feasibility verdict + updated risk table.
**Risk retired.** "Nanite-like on Metal" turning out to be a dead end in Phase 3.

#### 4c. Character and asset import

1. Locate the owner's `assets/characters/Solarcat.glb`; load it with a glTF crate in a test;
   print skeleton joint count, skin weight coverage, UV sets, material/texture list.
2. Retarget-test onto the intended standard humanoid skeleton; measure how broken the "terrible
   rig" actually is and what retargeting library or manual fix is needed.
3. Confirm the VRM path: read a sample VRM (humanoid bone map, MToon material, spring bones)
   and note which extensions our importer must support.
4. Download 2–3 Megascans samples (one 3D asset, one surface, one decal); prove the conversion
   path to glTF and texture compression (BC7 on desktop, ASTC later), and capture the licence
   metadata into the registry.
5. **Pass criteria:** the owner's character renders skinned in a throwaway viewer, and one
   Megascans asset survives conversion with its licence record.

**Artifact.** Import test/CLI + notes on retargeting + first registry entries for Fab assets.
**Risk retired.** Discovering in Phase 5 that the character pipeline needs a different model
format or that Megascans cannot be baked into our cluster format.

### W5 — Data registry, licence gate, P0 replacements

**Why.** Licensing is a one-way door and the current alpha cannot legally ship. The registry is
also the mechanism that keeps generation reproducible: every bake pack must name the exact
dataset versions it consumed.

**Steps**
1. Define `data/registry/*.toml` (schema in `docs/data/README.md` §2) and seed it with the
   sources already in use: OpenStreetMap/OpenFreeMap, Mapterhorn, AWS Terrain, ESA WorldCover,
   Macrostrat, Open-Meteo, JPL ephemeris.
2. Add the **licence gate**: a script/`xtask` that fails any ship manifest containing a RED
   dataset or an undeclared YELLOW one. Wire it into CI as a no-op-until-there-is-a-manifest.
3. Add the **attribution generator**: `CREDITS.md` + the resource the in-game credits screen
   will render (Solstice MIT, Box3D MIT, Fab/Megascans, every dataset).
4. **P0 decisions:** choose the WorldClim replacement (CHELSA / TerraClimate / ERA5 /
   NASA MERRA-2 + POWER) with licence URLs recorded, and write the EOX replacement plan (raw
   Sentinel-2 + NAIP/national orthophotos, colour-matched).
5. Add the version pinner concept: every future bake pack carries `dataset versions + plan
   signature`, so generation stays a pure function of pinned inputs.

**Artifact.** Registry files, gate, generated `CREDITS.md`, two P0 decision notes.
**Done when.** `CREDITS.md` exists, the gate runs in CI, and no shipping path references a RED
source.
**Risk retired.** A commercial launch blocked by a dataset we shipped for two years.

### W6 — Crate skeletons, dependency rules, CI

**Why.** Many parallel sessions; architecture survives only if it is mechanically enforced.
An empty crate with the wrong dependency edge is how "sim never imports render" quietly dies.

**Steps**
1. Create `ed_gpu`, `ed_planet`, `ed_assets`, `ed_sim`, `ed_anim`, `ed_store`, `ed_net`
   (minimal), and the `ew` app binary; each with a doc comment stating its single
   responsibility and what it may depend on.
2. Encode dependency direction: `ed_sim` must not see render/GPU types; `ed_server` must not
   depend on `ed_gpu`/`ed_render`/`ed_anim`; client and server share `ed_world` + `ed_planet`.
   Enforce with a small `xtask`/CI check (grep the dependency graph), not discipline.
3. Workspace hygiene: edition 2024, shared lints, `rustfmt`/`clippy` in CI, macOS + Windows
   matrix, `cargo test --workspace` stays fast and offline.
4. Quality-tier feature flags (`tier-low`, `tier-high`, `tier-ultra`) defined once in the
   workspace, not per crate.
5. Keep the surviving engine-free crates in place; delete Bevy-only crates only after W1's
   salvage check.

**Artifact.** Building workspace with empty-but-documented crates, green CI matrix, dependency
check.
**Done when.** `cargo build --workspace` and CI pass, and the dependency rules are enforced by
a failing check (prove it by trying a forbidden edge).
**Risk retired.** Parallel sessions coupling crates until the server needs a GPU.

### W7 — Interface contracts (the documents parallel work agrees on)

**Why.** The most expensive parallel-AI failure is not a merge conflict, it is two sessions
building compatible-looking halves of a seam that disagree. Contracts must be written before
the code.

**Steps**
1. **Generation contract:** what `ed_world` emits — heightfields, part instances (kit asset id,
   transform, per-instance params), placements (vegetation, props, roads, nav graph), and what
   it explicitly does *not* emit (merged meshes). Include the determinism requirements and the
   interior hook (door/window module ids).
2. **Sim contract:** `ed_sim` public surface — fixed step, body/handle types, snapshots,
   replay/record, sim-LOD promotion/demotion, parked-record format. No render types anywhere.
3. **Asset contract:** the bake format — cluster DAG metadata, material graph schema (PBR +
   toon variants), collision, LOD/cut rules, licence/provenance fields, kit manifest.
4. **Persistence contract:** `ed_store` records for accounts, plots, vessels, progression
   (ranks/trust/merit), missions, NPC lives — SQLite now, Postgres later, same shape for SP
   saves.
5. Link these from `docs/ARCHITECTURE.md`; they become the review checklist for Phases 1–2.

**Artifact.** Four short contract docs (`docs/contracts/`).
**Done when.** Phase 1 and Phase 2 can start without asking each other questions the docs do
not answer.

---

## 4. Ordering and timeboxing

| Day | Parallel streams |
|---|---|
| 1 | W1 hygiene + tag; start W4a (Box3D build) and W4b (Solstice clone) |
| 2 | W2 baseline capture (needs frozen alpha); W4c GLB import; W5 registry schema |
| 3 | W3 determinism harness; W4 spikes finish + decisions; W6 crate skeletons |
| 4 | W7 contracts; W5 gate + CREDITS + P0 decisions; CI green |
| 5 | Buffer: triage failures, write the Phase 0 report, update ROADMAP/MASTER |

Rule: if a spike fails its pass criteria, **do not** push on. Record the failure, take the
fallback branch (§6), and continue — the point of a spike is to fail cheaply.

---

## 5. Exit gate (all must be true)

- [ ] `alpha-early` tagged and buildable; `main` clean; nothing untracked except the owner's GLB.
- [ ] Alpha baseline report committed with per-scene fps/p99/VRAM/draws/tris.
- [ ] Golden-hash determinism suite in CI on macOS + Windows, with every red triaged.
- [ ] Box3D decision recorded with determinism + perf evidence.
- [ ] Solstice port map + feasibility verdict recorded, fallback named if needed.
- [ ] Owner's GLB loads skinned in a test; retargeting plan chosen; one Megascans asset
      converts with its licence recorded.
- [ ] `data/registry/` populated, licence gate in CI, `CREDITS.md` generated, WorldClim
      replacement chosen, EOX plan written.
- [ ] Workspace builds with the new crates; dependency rules enforced by a failing check;
      CI matrix green on macOS + Windows.
- [ ] Four interface contracts written and linked from `ARCHITECTURE.md`.
- [ ] `docs/ROADMAP.md` marks Phase 0 complete; Phase 1 and Phase 2 tasks are unblocked.

---

## 6. Failure branches (decided in advance)

| If… | Then… |
|---|---|
| Box3D is not deterministic on macOS/Windows, or too slow | Keep the `ed_sim` API, switch the backend to Rapier plus our own determinism layer, and record the cost now |
| Solstice techniques are Metal-hostile or over budget at High tier | Keep the cluster-DAG + visibility-buffer design, choose the cheaper GI (cached probes / DDGI-style), and gate hardware RT as Ultra-only |
| The owner's GLB rig cannot retarget cleanly | Use it as the art reference and start from a clean standard skeleton; re-skin, do not re-rig the engine around it |
| Golden hashes show pervasive nondeterminism | Insert "Phase 0.5: generator determinism" before Phase 2 and fix the generator — multiplayer cannot be built on a nondeterministic base |
| Megascans conversion is lossy or licence metadata is hard to track | Prefer authored/procedural assets for the kit and use scans only where they survive the pipeline; keep provenance in the registry |
| Registry/gate work grows beyond a week | Ship schema + gate + the six in-use sources; defer the long tail to Phase 4/5. The gate matters more than the catalogue |

---

## 7. How Phase 0 serves the goals (`docs/VISION.md`)

| Goal | What Phase 0 does for it |
|---|---|
| §1 Guided freedom, merit progression, home bases | Nothing directly — but the interface contracts fix the persistence shape (ranks/trust/merit, plots) so later systems do not have to be rewritten |
| §2 The north-star session loop | The baseline records exactly where the alpha breaks that loop; the spikes prove the physics and renderer can carry it |
| §3 One continuous planet | The alpha's hard switch is documented as a baseline failure, and the Solstice port map proves the replacement renderer is feasible before Phase 2 commits |
| §4 Persistent shared universe + NPC life | Golden-hash determinism is the precondition; the licence/version pinner makes generation reproducible for every client |
| §5 Photoreal world + cel characters | The Megascans import spike proves the photoreal pipeline; the material contract defines PBR + optional toon; the baseline sets the quality bar to beat |
| §6 Owner GLB → VRM avatars, realistic NPCs | W4c proves import/retarget now, so `ed_anim` is designed against a real skeleton instead of assumptions |
| §7 Real vehicles, destruction, knockable trees | Box3D's determinism/performance/API is proven before any vehicle code exists |
| §8 Handcrafted + guided procedural | The generation contract makes authored parts first-class (part instances, not merged soup), which is what enables authored kits and interiors |
| §9 Stations, Moon/Mars, fourth world | The planet and persistence contracts are body-agnostic from day one, so stations and the fourth world need no architectural change |
| §10 Twin-Earth data library, licensed | W5 is entirely this: registry, gate, attribution, and the two P0 replacements |
| §12 Interiors | The kit contract (door/window module ids, part instances) is fixed now so exteriors never have to be rebuilt to admit interiors |
| §11 Locked decisions | Phase 0 is where they turn from prose into tests, tags, CI checks and contracts |

**One sentence:** Phase 0 is the week where the project stops being a convincing prototype and
becomes a foundation — evidence instead of opinions, contracts instead of assumptions, and a
legal, deterministic, measurable base that every later phase can build on without looking back.
