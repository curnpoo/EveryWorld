# Phase 1 — Engine skeleton (detailed)

Status: next phase (starts when Phase 0's exit gate is green). Phase doc; roadmap summary in
`docs/ROADMAP.md`, plan in `docs/plans/rearchitecture.md`, goals in `docs/VISION.md`, Phase 0
in `docs/phases/phase-0.md`.

Timebox: **~3 weeks**. Exit gate: **a real Metal window renders the test scene at budget, and
the headless harness can measure it** — not a triangle, not a screenshot of a triangle.

---

## 1. Philosophy — why Phase 1 exists

### 1.1 Build the spine before the body

Every expensive thing we want later hangs off four cheap things:

1. a window that presents,
2. a frame loop that submits work to the GPU,
3. a **fixed-step simulation** with an explicit boundary to rendering,
4. an **asset pipeline** that turns authored content into data the renderer can stream.

If any of those four is shaped wrong, everything above it is wrong: the planet renderer inherits
the frame graph, the vehicles inherit the step model, the world inherits the asset format, the
server inherits the determinism boundary. Phase 1 is where those seams are cast. Later phases
fill them in; they do not re-cut them.

### 1.2 The smallest honest vertical slice

Phase 1 is deliberately **thin but complete**: for each subsystem, the smallest version that is
still *the real subsystem* and can fail in a way we can measure. One textured PBR mesh (not a
triangle). One cluster-LOD asset with visible cut transitions (not a placeholder cube). One
simulated body with a replay hash (not a moving box with no determinism story). One bake CLI
that produces a versioned, content-addressed pack (not a loader hack).

"Thin but complete" is the whole point: a triangle proves nothing, and a half-built renderer
proves the wrong things.

### 1.3 Retire the last two engineering unknowns

Phase 0 retires the *dependency* bets (Box3D, Solstice techniques, import paths). Phase 1 retires
the *engineering* bets:

- **Can our own Metal RHI express what the renderer needs?** Bindless/argument buffers,
  transient resources, GPU timestamps, HDR presentation, a visibility-buffer-capable pipeline,
  shader hot reload. If the RHI cannot, we find out in week 1, not in Phase 3.
- **Can a bake pipeline turn real authored assets into the cluster DAG and materials the
  renderer expects, deterministically and fast?** If baking a Megascans-converted asset takes
  ten minutes or produces nondeterministic clusters, the art plan is wrong.

### 1.4 Prove the rules, not just the code

Phase 1 is the first place the standing rules (`ROADMAP.md`) become executable:

- measure first → harness exists before features;
- determinism → replay hash + bake hash from day one;
- sim/render separation → `ed_sim` cannot name a render type, enforced by CI;
- licensed data → bake packs carry provenance, and the test scene uses cleared assets only;
- every feature has an off switch → caps/tier flags wired into the first pass.

---

## 2. What Phase 1 is not

- Not the planet (Phase 2), not GI or shadows (Phase 3), not vehicles/physics gameplay
  (Phase 4), not multiplayer (Phase 6).
- Not the final material system: PBR v0 + the toon interface only, no layered graphs, no
  wetness/subsurface yet.
- Not the final UI: egui is a dev tool, the game HUD stays a later authored layer.
- Not the D3D12/Vulkan backends: the RHI must *allow* them, but only Metal ships now.
- Not performance tuning: Phase 1 sets budgets and instrumentation; it does not chase them.

---

## 3. Key decisions

These are the decisions Phase 1 has to make. Each lists the alternatives, the choice this plan
recommends, the reason it follows from the locked decisions, and when it can be revisited.
A decision that turns out wrong here is cheap now and expensive in Phase 3 — that is why each
is explicit rather than emergent.

### P1-D1 — Shader authoring and pipeline

| | |
|---|---|
| **Options** | (a) MSL first, HLSL/SPIR-V written separately later · (b) **Slang** single source → MSL/HLSL/SPIR-V · (c) WGSL/WESL via naga → MSL |
| **Choice** | **Slang as the primary language, with an MSL escape hatch.** `ed_gpu` accepts precompiled MSL or SPIR-V/HLSL, so the language is a toolchain detail, not an architectural lock. |
| **Why** | We are building one renderer over three backends, with bindless, atomics, subgroups and tier-gated features. Duplicating shaders per backend is a maintenance tax we cannot afford; WGSL/naga cannot express the Metal-specific paths cleanly. The alpha's WESL shaders stay as reference, not as the new pipeline. |
| **Revisit** | If Slang's toolchain blocks CI or debugging, fall back to MSL-first behind the same interface; portable shaders can be translated later. |

### P1-D2 — Metal binding layer

| | |
|---|---|
| **Options** | `objc2`/`objc2-metal` · `metal-rs` · hand-written Objective-C shim |
| **Choice** | **`objc2`-based bindings inside a private `ed_gpu_metal` backend crate**; the public RHI never exposes Metal types. |
| **Why** | Fastest safe path to a working device/queue/layer; the crate boundary means binding problems never leak into the renderer. Phase 0 already established that `ed_metal` is a salvage reference, not a foundation. |
| **Revisit** | If a needed API (argument-buffer tiers, counter sample buffers, capture) is missing or unsound, add a narrow C shim inside the same crate. |

### P1-D3 — Frame graph vs manual passes

| | |
|---|---|
| **Options** | Manual encoder order per frame · explicit render graph with barriers/aliasing |
| **Choice** | **Explicit render graph** (declared passes, read/write resources, automatic hazards and transient aliasing) with a manual escape hatch. |
| **Why** | Phase 3 lands culling → shadow → visbuffer → resolve → GI → atmosphere → post; that is 15–25 passes with transient targets on a fixed memory budget. Hand-ordering that is how holes and leaks happen. The graph also gives us per-pass timers for free (P1-D15). |
| **Revisit** | Keep the escape hatch: a pass may own its encoders directly where the graph gets in the way (e.g. Metal capture, compute barriers). |

### P1-D4 — Frames in flight, pacing and presentation

- **Choice:** 2–3 frames in flight, mailbox-style present, vsync toggle, uncapped mode for
  measurement, one swapchain, resize without stalling more than one frame.
- **Why:** latency and determinism are unrelated to presentation depth, but input latency and
  throughput are. Uncapped + timestamps is what makes the harness meaningful.
- **Revisit:** frame pacing/latency work belongs to Phase 3/7, after there is enough GPU work to
  pace.

### P1-D5 — Threading model

| | |
|---|---|
| **Options** | Single main thread · dedicated render thread · job graph across both |
| **Choice** | **Main thread orchestrates (input → sim → submit), `ed_runtime` worker pool for IO, baking and upload staging; one graphics queue + one copy queue; no dedicated render thread yet.** |
| **Why** | Simplicity now buys determinism and low input latency; the GPU-driven renderer in Phase 3 removes most CPU submission cost anyway. A render thread is a performance tool, not an architecture. |
| **Revisit** | Add a render thread only if CPU submission is measured as the bottleneck on the High tier. |

### P1-D6 — Scene/entity representation (no general ECS)

| | |
|---|---|
| **Options** | Adopt `hecs`/`bevy_ecs` · own generational entity index + typed SoA stores |
| **Choice** | **Own minimal entity index + typed stores.** Gameplay may adopt a general ECS later; the **render path consumes flat, frame-built draw data, never an ECS query.** |
| **Why** | Bevy's ECS was never the bottleneck; the renderer was. Deterministic sim wants explicit, ordered, contiguous arrays; the GPU wants flat buffers; streaming wants ownership it can count. A general ECS adds reflection/dynamic dispatch we do not need yet. |
| **Revisit** | Phase 4 can introduce `hecs` for gameplay entities if systems get numerous — behind the sim/scene boundary, never in the renderer. |

### P1-D7 — Precision and transform model

- **Choice:** truth is **f64 body-fixed**; assets and sim store f64 node transforms; the render
  boundary computes **one camera-relative f32 origin per view per frame**; nothing in an asset
  or a shader stores a world-space f32 position.
- **Why:** this is the decision that lets Phase 2 orbit ground geometry and Phase 5 Moon
  geometry share one renderer. If Phase 1 bakes f32 world positions into the asset format, the
  planet spike inherits a precision bug it cannot fix.
- **Revisit:** never — this is a one-way door.

### P1-D8 — Depth and projection convention

- **Choice:** reverse-Z, infinite far plane, 32-bit depth, documented clip-space convention,
  one shared `ed_gpu` view-projection helper. Alpha already proved this works; Phase 1 makes it
  the only path.
- **Why:** orbit-scale depth ranges are impossible with a forward-Z near/far pair; the alpha's
  switch existed partly to hide that.

### P1-D9 — Binding model

- **Choice:** **Metal 3 argument-buffer bindless where the tier supports it**, with a
  descriptor-index fallback; resource tables owned by `ed_gpu`; caps queried at startup
  (P1-D10).
- **Why:** cluster DAG, instancing and visbuffer resolve all want large, dynamic bind sets. If
  Phase 1 ships a per-draw `setTexture` style API, Phase 3 rewrites it.
- **Revisit:** tier 1 (no argument buffers) is a Low-tier path, decided by caps, not code forks.

### P1-D10 — Capability probe and tier gating

- **Choice:** a `Caps` struct probed at startup: argument-buffer tier, **64-bit integer
  atomics**, counter sample buffers, EDR, max bindless table sizes, optional ray tracing, and
  GPU family. Every feature below `Ultra` is gated by a named flag the harness can force on/off.
- **Why:** the visibility buffer wants a 64-bit packed depth+id; if the hardware cannot, the
  32-bit packed path is a *design branch decided by probe*, not a Phase 3 emergency. Same for
  GI and upscalers later.
- **Revisit:** caps are data; adding a cap is cheap. Guessing a cap is not.

### P1-D11 — Asset format and cluster builder

| | |
|---|---|
| **Options** | Load glTF at runtime · adopt Solstice's format wholesale · own container + own DAG |
| **Choice** | **Own versioned `.ewpack` container** (binary header + TOML/JSON manifest + content-hashed blobs: clusters, meshes, textures, materials, collision, provenance/licence) and a **DAG builder using `meshoptimizer` primitives** (meshlets, simplification, partitioning) with our own grouping/cut rules and determinism tests. |
| **Why** | The renderer needs clusters and cuts, not glTF; the asset pack is also where licence/provenance and version pinning live (`docs/data/README.md`). Solstice's builder is a reference for the algorithm, not a format to inherit. `meshoptimizer` is mature, fast and deterministic enough to be tested. |
| **Revisit** | If the builder cannot hit bake-time budgets on real Megascans meshes, add offline parallel bake shards — not a different format. |

### P1-D12 — Material model v0 (PBR + toon interface)

- **Choice:** one material interface with two implementations: **PBR/GGX** (base colour,
  metal/rough, normal, AO, emissive, transmission later) and **Toon** (banded response +
  outlines) sampling the *same* lighting inputs. Material graphs are **data** (schema + hot
  reload), not code. One material system, no second renderer.
- **Why:** VISION §5/§6 — realistic lighting everywhere, cel as an opt-in character material,
  NPCs PBR-only. Phase 1 exists to prove the interface can express both before either is
  polished.
- **Revisit:** the node-graph editor is later; Phase 1 only needs the schema + a fixed set of
  nodes the test scene uses.

### P1-D13 — Texture pipeline and virtual-texture readiness

- **Choice:** BC7 for desktop (ASTC for mobile later), mips generated at bake, explicit colour
  space per texture role, and **texture pages addressed the way virtual texturing will need**
  (stable page ids + indirection table) even though the VT streaming is Phase 2.
- **Why:** Phase 2's imagery VT cannot be bolted onto a "one texture per material, bound
  directly" assumption. Naming pages now costs nothing; renaming them later touches every
  material.

### P1-D14 — Dev UI vs game UI

- **Choice:** **egui** for dev overlays (stats, culling, pass timers, material tweaks); the
  game HUD/menu stays a separate authored layer (`ed_ui`, later) that consumes game state.
- **Why:** debug UI must be fast to change and must never define the game's look. Coupling them
  is how a shipped game ends up looking like an editor.

### P1-D15 — Headless harness and CI GPU strategy

- **Choice:** `--headless` renders offscreen with a deterministic camera path, writes PNG +
  CSV (frame times, p99, per-pass GPU timestamps), and can trigger a Metal capture; CI compiles
  and runs CPU-only tests (bake determinism, loop math, asset validation); **GPU smoke and perf
  gates run on a self-hosted macOS runner or locally via `xtask gpu-smoke`** as an explicit,
  documented step.
- **Why:** "measure first" is a standing rule, and hosted macOS CI is not a reliable Metal GPU.
  Pretending CI measures GPU performance would corrupt every later number.

### P1-D16 — Sim loop and Box3D integration depth

| | |
|---|---|
| **Choice** | **Fixed 60 Hz sim with accumulator, max-steps clamp, interpolated render, input sampled at step boundaries, and a replay hash.** Box3D is integrated behind `ed_sim` with a **minimal** world: a heightfield ground, a stack of boxes, and a character-mover probe — **no vehicles**. |
| **Why** | Phase 0 proved the backend; Phase 1 proves the *plumbing* (fixed step, replay, sim→render interpolation, determinism boundary) on the cheapest possible content. Vehicles in Phase 4 then inherit a trusted loop, and a replay that desyncs is caught in week 2, not month 4. |
| **Revisit** | Step rate (60 vs 120 Hz) is a data-driven choice after Phase 4 feel tests; the loop must allow both. |

### P1-D17 — Time and determinism boundaries

- **Choice:** sim consumes only `(fixed dt, input snapshot, world seed, pinned data version,
  UTC)`; render, UI, audio and logging may use wall-clock freely. Render interpolation uses an
  explicit `alpha` from the accumulator, never wall-clock.
- **Why:** the network model depends on replayable sim; anything that reads the wall clock in
  sim is a desync. Making the boundary explicit in Phase 1 means Phase 6 does not have to hunt
  for it.

### P1-D18 — Salvage policy from `ed_metal`

- **Choice:** **reimplement** device/layer/passes/timers against the new RHI, **copy with tests**
  only pure, self-contained code (ocean FFT, probe-GI bake math, shadow-cache maths, shader
  helper functions), and **never** copy the Bevy↔Metal entity mirror.
- **Why:** the mirror is the reason the alpha had three renderers (S1). Salvage the knowledge,
  not the coupling.

### P1-D19 — Test scene definition

- **Choice:** procedural **"renderer proving ground"**: ground plane, PBR material grid
  (dielectric/metal/roughness sweep), one Megascans-converted scanned asset (from Phase 0 W4c),
  one cluster-LOD asset shown at three distances with cut transitions, a simple sun + analytic
  sky, a moving camera path, and an egui overlay. No planet, no city, no gameplay.
- **Why:** isolates renderer validation from world generation; every future renderer change can
  be A/B'd on a scene that never changes except when we mean it to.

### P1-D20 — Phase 1 budgets (numbers to hold the phase to)

| Item | Target (M3 Pro, 1440p, High) |
|---|---|
| Empty frame (clear + UI) GPU time | < 2 ms |
| Test scene, 1M tris + 100 clusters | 60 fps, no frame > 25 ms after warm-up |
| Loop overhead (sim + submit, no GPU wait) | < 0.5 ms/frame |
| Bake: 100 MB source glTF → pack | < 60 s, deterministic hash across runs |
| Sim replay: 60 s, 1k boxes | state hash identical across runs/processes |
| Shader hot reload | < 1.5 s, no restart, no leak |
| Asset hot reload | < 2 s for a material/texture change |

Budgets are recorded in the harness output, not asserted in prose.

---

## 4. Workstreams

### W1 — `ew` app shell (window, input, loop, config)

**Steps:** winit window + macOS layer wiring; CLI/config (`--headless`, `--scene`, `--frames`,
`--capture`, `--no-vsync`, tier flags); logging/tracing; input abstraction; fixed-step
accumulator + interpolation; pause/time-scale; clean shutdown; panic handling that still frees
GPU resources.
**Artifact:** `ew` opens a Metal window, runs the loop, resizes and closes cleanly.
**Done when:** loop determinism tests pass (step counts independent of frame rate) and input is
sampled at step boundaries.

### W2 — `ed_gpu` Metal backend

**Steps:** device/queue/layer; swapchain + resize + EDR/colour-space; buffers/textures with
staging + residency; pipeline cache (render/compute); argument-buffer bindless + fallback;
samplers; fence/semaphore model with N frames in flight; timers (counter sample buffers with
CPU fallback); capture hooks; debug labels; caps probe; shader compile + hot reload (P1-D1/D2
toolchain).
**Artifact:** a backend that can run a trivial lit pass and a compute pass with timestamps.
**Done when:** caps print correctly on the M3 Pro and the test scene can be rendered through
the public RHI without Metal types outside `ed_gpu_metal`.

### W3 — `ed_render` bootstrap (frame graph, passes, test scene)

**Steps:** render graph with transient aliasing + per-pass timers; depth/reverse-Z helpers;
mesh + cluster upload path; PBR v0 + toon interface; analytic sky/sun and a simple shadow pass
(placeholder for Phase 3); egui overlay; the proving-ground scene (P1-D19).
**Artifact:** the test scene at budget with an egui stats panel and per-pass timings.
**Done when:** budgets P1-D20 hold or the gap is documented with a plan.

### W4 — `ed_assets` bake CLI v1

**Steps:** glTF (and FBX→glTF converted) ingestion with errors that name the asset; meshlet
clustering + simplification + DAG grouping; cut generation + watertightness check; material
extraction into the schema; texture compression BC7 + mips; collision placeholder; `.ewpack`
writer with manifest, content hash, provenance/licence fields; deterministic bake test in CI.
**Artifact:** `ew-bake asset.gltf -o asset.ewpack` + a validation report.
**Done when:** the Phase 0 Megascans sample and a synthetic 1M-tri mesh both bake, load and
render with stable hashes.

### W5 — `ed_sim` loop integration

**Steps:** `ed_sim` public API (world, bodies, step, snapshot, replay record/play); Box3D
backend behind it with the minimal Phase 1 scene (heightfield + box stack + character mover
probe); determinism/replay tests across runs and processes; sim→render interpolation data.
**Artifact:** replay hash test in CI + a box stack that falls identically every run.
**Done when:** CI proves identical hashes on macOS and Windows (CPU-only), and the renderer can
draw the interpolated state.

### W6 — Observability and the harness

**Steps:** `--headless` offscreen path; deterministic camera path; CSV schema (frame, cpu, gpu,
passes, memory, draws, tris); p99 and hitch detection; PNG snapshot; Metal capture trigger;
`xtask gpu-smoke` and `xtask bake-check`; documented local/self-hosted GPU CI procedure.
**Artifact:** one command produces the Phase 1 performance report.
**Done when:** the same command reproduces numbers within noise across two runs.

### W7 — Contracts implemented, CI wired

**Steps:** implement the Phase 0 contracts at the seam (asset manifest, sim API, generation
placeholder that emits part instances, persistence stub not yet used); enforce dependency rules
in CI (`ed_sim` must not depend on `ed_gpu`/`ed_render`; server crates must not depend on GPU
crates); license gate active on every pack; `CHANGES.md` entry with the first real numbers.
**Artifact:** green CI matrix + failing check demonstrated for a forbidden edge.
**Done when:** the rules from `ROADMAP.md` are executable, not aspirational.

---

## 5. Week by week (indicative)

| Week | Focus | Checkpoint |
|---|---|---|
| 1 | W1 + W2: window, loop, device, swapchain, one lit pass, timers, caps probe | A textured mesh presents at 60 fps with timestamps |
| 2 | W3 + W4 start: frame graph, test scene, bake CLI ingestion/clustering | Test scene renders; a glTF bakes to a pack and loads |
| 3 | W4 finish + W5 + W6 + W7: textures/DAG cuts, Box3D loop, harness, CI | Exit gate + first performance report |

If W2 (RHI) is late, W4 (bake) proceeds in parallel because it has no GPU dependency — the two
halves of the phase are deliberately decoupled so a backend problem cannot stall the asset
pipeline.

---

## 6. Exit gate (all must be true)

- [ ] `ew` presents a Metal window; resize, EDR/colour management and clean shutdown work.
- [ ] The proving-ground scene renders through the public RHI at the P1-D20 budgets, with
      per-pass timestamps and an egui stats panel.
- [ ] Frame graph owns transient resources; no pass leaks, no manual barrier bugs on resize.
- [ ] Cluster-LOD asset renders with visible, watertight cuts at three distances.
- [ ] PBR v0 and the toon interface both render on the same scene and the same lighting inputs.
- [ ] `ew-bake` produces a deterministic `.ewpack` with manifest, content hash and licence
      provenance; validated by a CI test.
- [ ] Fixed-step loop + Box3D replay hash is identical across runs and processes; CI green on
      macOS + Windows for the CPU parts.
- [ ] Headless harness writes PNG + CSV with p99, per-pass GPU time and memory; capture works.
- [ ] Dependency rules enforced by a failing check; licence gate runs on packs.
- [ ] `docs/CHANGES.md` has the first real Phase 1 numbers; Phase 2 tasks are unblocked.

---

## 7. Failure branches (decided in advance)

| If… | Then… |
|---|---|
| Slang integration blocks CI/debugging | Ship MSL-first behind the `ed_gpu` shader interface; add Slang when the toolchain is proven. No renderer code changes. |
| `objc2`/`metal-rs` lacks a needed API or is unsound | Add a narrow Objective-C shim inside `ed_gpu_metal`; never widen the public RHI. |
| 64-bit atomics unavailable on target GPUs | Use the 32-bit packed visibility buffer path (probe-gated), and record the quality/perf trade in the caps matrix. |
| Frame graph over-engineers early passes | Keep manual pass escape hatch; ship the graph with the subset the test scene needs, grow it in Phase 3. |
| Cluster builder too slow on Megascans meshes | Parallel bake shards + aggressive simplification pre-pass; keep the format. |
| Metal capture/timestamps unreliable in CI | CPU frame timing remains authoritative for CI; GPU timings are local/self-hosted and labelled as such. |
| Box3D integration destabilises the loop | Keep the loop's step/replay interface, ship Phase 1 with a stub backend (kinematic boxes), integrate Box3D in Phase 4 — the determinism plumbing is what Phase 1 must prove. |
| Test-scene budgets are missed | Do not tune features that Phase 3 replaces; record the gap, identify whether it is submission, bandwidth or the RHI, and gate the phase on the *diagnosis*, not the number. |

---

## 8. How Phase 1 serves the goals (`docs/VISION.md`)

| Goal | What Phase 1 does for it |
|---|---|
| §2 north-star loop | Delivers the loop skeleton (window → fixed step → submit → interpolate) that the whole loop will run on |
| §3 one continuous planet | Casts the f64-camera-relative seam and reverse-Z depth the planet renderer will live inside; no f32 world positions in assets |
| §4 shared universe | Executable determinism boundary + replay hash + licence-stamped bake packs |
| §5 photoreal + optional cel | Material interface proven for both PBR and toon; BC7/pages ready for the VT imagery of Phase 2 |
| §6 avatars/NPCs | Asset pipeline ingests the owner's GLB and skinned content; the render path accepts skinned meshes (basic) |
| §7 vehicles/destruction | Box3D behind `ed_sim` with a trusted fixed-step/replay loop before any vehicle exists |
| §8 handcrafted + procedural | Bake pipeline treats authored assets as first-class content; part-instance contract scaffolded |
| §9 stations/Moon/fourth world | Body-agnostic asset/transform contracts; no Earth-specific assumptions in the RHI |
| §10 data licensing | Every pack carries provenance; the licence gate runs on the first pack |
| §12 interiors | Asset/material contracts support interior parts; no architectural decision blocks them |
| §11 locked decisions | Metal-first RHI, no Bevy/wgpu, deterministic sim, own formats — all become code |

**One sentence:** Phase 1 builds the spine — window, frame, fixed step, bake pack — thin enough
to finish in three weeks and real enough that every later phase is an extension rather than a
rewrite.

---

## 9. Handoff to Phase 2 (what Phase 2 may rely on)

- A camera-relative f32 renderer with f64 truth, reverse-Z, and a frame graph that can add
  planet passes without restructuring.
- A working RHI with bindless, timers, caps probing and HDR presentation.
- A deterministic bake pipeline and asset format that can carry planet tiles, imagery pages and
  materials.
- A fixed-step sim with replay hashes and an explicit render interpolation seam.
- A harness that measures every later claim.

**What Phase 1 must *not* pre-empt:** the planet quadtree and virtual-texturing *internals*
(Phase 2), the occlusion/visbuffer/GI design (Phase 3), vehicle models (Phase 4), the netcode
(Phase 6). The seams exist; the implementations belong to their phases.
