# Phase 1 — Engine base and editor shell

Status: next, after Phase 0. Roadmap row: `../ROADMAP.md`. Decisions referenced by id are in
`../DECISIONS.md`; this file holds the detail for the P1-D series. Inputs are the Phase 0
contracts in `../contracts/`.

## 1. Goal

A running engine with no planet in it yet: a window, a GPU-driven renderer drawing a large
field of instanced parts with LOD, a pack format and bake tool, a fixed-step simulation, the
world's addressing and layer types, and the first stage of the editor. Everything later is
added to this base without restructuring it.

## 2. What Phase 1 proves

| Question | Proof |
|---|---|
| Does the instance-based design fix the alpha's frame cost? | 100k instances of mixed parts at 60 fps on High, per-pass timings recorded |
| Is the RHI the right shape for more than Metal? | The same scene renders on the minimal second backend |
| Can geometry be a slot? | The renderer draws through the provider interface; a stub second provider can be registered |
| Can the editor exist on this engine? | Pick an instance, move it with a gizmo, save the layer, restart, and it is where it was left |
| Is the loop sound? | Fixed-step sim with replay hash; render interpolates |
| Is the content path sound? | Blender glTF → `ew-bake` → `.ewpack` → on screen, deterministic, hot-reloadable |

## 3. What Phase 1 does not do

No planet, terrain or atmosphere (Phase 2). No world generation (Phase 3). No VSM or GI
(Phase 4). No vehicles (Phase 5). No networking (Phase 7). No cluster rendering on the
critical path (Track G may start).

## 4. Decisions in detail

### P1-D1 — Shaders
Slang as the single source, compiled at build time to each backend; `ed_gpu` also accepts
precompiled backend shaders as an escape hatch. *Why:* one renderer over three APIs.
*Revisit:* if the Slang toolchain blocks debugging on Metal, write MSL first behind the same
interface.

### P1-D2 — Metal bindings
`objc2-metal` inside a private backend module. The public RHI exposes no Metal types.
*Revisit:* add a narrow Objective-C shim inside the module if an API is missing.

### P1-D3 — Frame graph
Passes declare the resources they read and write and in which state. The graph orders
passes, aliases transient targets, emits state transitions for backends that need them, and
gives every pass a GPU timer. A pass may take manual control where needed.

### P1-D4 — Frames in flight and pacing
Two to three frames in flight; vsync toggle; uncapped mode for measurement; resize without
a stall longer than one frame.

### P1-D5 — Threading
The main thread runs input → sim → frame build → submit. `ed_runtime` pools do IO, decode,
generation and upload staging. No render thread until CPU submission is measured as a
bottleneck; with GPU-driven drawing it should not be.

### P1-D6 — Scene representation
Our own generational entity index and typed arrays. The render path consumes flat instance
pools built per frame, never entity queries. Gameplay may adopt a general ECS later, behind
the sim boundary.

### P1-D7 — Precision
f64 body-fixed truth. Chunks and instances store positions relative to their node origin.
Each view computes one camera-relative f32 origin per frame. *Never revisited.*

### P1-D8 — Depth
Reverse-Z, infinite far plane, 32-bit float depth, one shared projection helper.

### P1-D9 — Binding model
Bindless tables owned by `ed_gpu`: Metal argument buffers, descriptor indexing on Vulkan,
descriptor heaps on D3D12. Instances and materials carry indices.

### P1-D10 — Capabilities and tiers
A `Caps` struct probed at startup (argument-buffer tier, 64-bit atomics, timers, HDR, table
sizes, ray tracing, memory). Tiered features are gated by named flags the harness can force.

### P1-D11 — Pack format and geometry slot
`.ewpack`: a container of content-addressed chunks with a manifest (`contracts/pack.md`).
A mesh chunk holds a **required discrete LOD chain** (meshoptimizer simplification, with
error per level) and an **optional cluster-DAG chunk** (F3). Bakes are deterministic and
tested. Every chunk carries provenance and licence ids.

### P1-D12 — Materials v0
One material interface, two models: PBR (GGX) and toon, both reading the same lighting
inputs. Materials are data files with hot reload.

### P1-D13 — Textures
BC7 on desktop, ASTC later for mobile; mips at bake; colour space per role; textures are
addressed as pages with stable ids so virtual texturing in Phase 2 needs no material changes.

### P1-D14 — Developer UI and editor UI
egui, drawn by a small painter written on `ed_gpu`, is used for developer overlays and for
the editor (F6). The game's HUD and menus are a separate authored layer later.

### P1-D15 — Harness
`ew --headless` renders offscreen along a scripted camera path and writes PNG and CSV (frame
times, p99, per-pass GPU time, memory, instance counts). Hosted CI runs CPU tests only. GPU
smoke and perf runs are local or on a self-hosted Mac, and are labelled as such.

### P1-D16 — Sim loop
Fixed 60 Hz with an accumulator, a step clamp and render interpolation. Box3D behind
`ed_sim` with a minimal world: ground, a stack of boxes, a character-mover probe. Replay hash
test in CI (F10).

### P1-D17 — Time boundary
The sim reads only (fixed dt, input snapshot, seed, data version, UTC). Rendering, UI and
audio may read the wall clock. Interpolation uses the accumulator fraction.

### P1-D18 — Salvage from the alpha Metal path
The alpha's Metal renderer is the **look reference**, using the Phase 0 captures. Its
structure (entity mirror, merged meshes, per-tile draws) is not reused. Pure shader maths
(atmosphere, clouds, tone mapping, terrain material, MetalFX setup) is ported with a test
that compares against the reference capture. The ocean FFT, probe GI and shadow caching are
not ported (D13, D14).

### P1-D19 — Proving-ground scene
Described the way the world will be: a few nodes, a pack, a layer file. Contents: a ground
patch; a PBR material grid; one scanned asset; **100,000 instances of 20 to 50 distinct
parts** (windows, doors, wall panels, props, trees) laid out as blocks with long sight lines
and occluders; a skinned character; sun and analytic sky; a scripted camera path.

### P1-D20 — Budgets (M3 Pro, 1440p, High)

| Item | Target |
|---|---|
| Empty frame, GPU | under 2 ms |
| Proving ground, 100k instances | 60 fps; no frame over 25 ms after warm-up |
| Culling pass | under 1 ms |
| CPU frame build with 100k instances | under 1 ms (nothing per-instance on the CPU) |
| Bake 100 MB of glTF | deterministic hash across runs |
| Sim replay, 60 s, 1,000 bodies | identical hash across runs and processes |
| Shader or material hot reload | no restart, no leak |

Numbers are recorded by the harness. A missed number is reported with its diagnosis.

### P1-D21 — Geometry-provider interface (F3)
`ed_render` draws opaque geometry only through `GeometryProvider`
(`../plans/renderer.md` §3). Provider 1 (discrete LOD, compute culling, indirect draws,
dithered LOD cross-fade) is the Phase 1 gate. The shared inputs are built now: instance
pools with stable ids and previous-frame transforms, the depth pyramid, visibility targets.

### P1-D22 — Minimal second backend (F7)
The backend chosen in Phase 0 (W4f) renders the proving ground: resources with states,
bindless, indirect draws, compute culling, present. No performance target.

### P1-D23 — Addressing and layers before the planet (F1, F5)
`NodeId`, `ObjectId`, the pack manifest keyed by node, and the override-layer reader and
writer exist and are used by the proving ground. Phase 2 adds a planet under these types; it
does not introduce them.

### P1-D24 — Multi-view and picking
A `View` abstraction (camera, target, flags) used by the main camera, shadow cascades and
editor viewports. An id target and a readback path return the `ObjectId` under a pixel.

## 5. Workstreams

### W1 — `ew` shell
winit window, input mapping, configuration, game and editor modes, fixed-step main loop,
clean shutdown. *Done when:* the loop runs sim and render at independent rates with the
interpolation fraction exposed.

### W2 — `ed_gpu`
Public interface from `contracts/rhi.md`. Metal backend: device, resources, bindless
tables, pipelines from Slang output, compute, indirect draws, timers, capability probe, HDR
presentation. Then the minimal second backend. *Done when:* both backends pass the same RHI
test suite subset and render the proving ground.

### W3 — `ed_render` base
Frame graph; views; instance pools; compute culling (frustum, depth pyramid); provider 1;
visibility buffer and material resolve; PBR and toon v0; baseline lighting (cascaded
shadows, sky ambient, GTAO); tone mapping ported from the alpha and compared to the
reference capture; skinned mesh path (basic); id target and picking. *Done when:* the
proving ground meets P1-D20.

### W4 — `ed_pack` and `ew-bake`
Container, chunk kinds for mesh, texture, material, collision, prefab and instances;
manifest keyed by `NodeId`; glTF import; LOD chain build; texture compression; prefab
manifests; watch mode for hot reload; determinism tests. *Done when:* a Blender export
appears in the running proving ground after save, and the bake hash is stable.

### W5 — `ed_sim`
Box3D integration in the form chosen in Phase 0; fixed step; snapshots; replay record and
hash; debug draw through the overlay pass. *Done when:* the replay test is green in CI.

### W6 — Editor stage E1 (`ed_editor`)
egui painter on `ed_gpu`; free camera with wide speed range; stats and pass-timing panels;
pick and inspect; transform gizmo; layer file load and save; undo for transform edits; hot
reload wiring. *Done when:* the pick-move-save-restart check passes.

### W7 — Harness and CI
Headless mode, scripted paths, CSV and PNG output, perf comparison script, the first
`CHANGES.md` entry with real numbers; dependency check and licence gate running on packs.

### Track G (optional start)
Cluster-DAG bake as the optional chunk and a stub provider 2 that draws the DAG's coarsest
cut. Does not gate Phase 1.

## 6. Order

```
W1 ─┬─> W2 (Metal) ─┬─> W3 ──> W6 ──> exit
    │               └─> W2 (second backend, after W3's needs are known)
    ├─> W4 ───────────────┘ (feeds W3 with real content)
    ├─> W5
    └─> W7 (from the first frame on)
```

W4 and W5 do not depend on the GPU work and can proceed in parallel sessions against the
contracts.

## 7. Exit gate

- [ ] `ew` opens a Metal window; resize, HDR and shutdown are clean.
- [ ] Proving ground renders through the public RHI at the P1-D20 budgets, with per-pass
      timings in the harness output.
- [ ] No per-instance CPU work in the frame; culling and LOD selection are on the GPU.
- [ ] LOD cross-fades are free of popping on the scripted path.
- [ ] The same scene renders on the second backend.
- [ ] PBR and toon materials render under the same lighting; tone mapping matches the
      alpha reference capture within the agreed tolerance.
- [ ] `ew-bake` output is deterministic, carries provenance, and passes the licence gate.
- [ ] Blender export → on screen without restarting.
- [ ] Sim replay hash is stable across runs and processes; CI green on macOS and Windows
      for CPU tests.
- [ ] Editor E1: pick, inspect, move, save layer, restart, object is in place.
- [ ] `NodeId`, `ObjectId`, pack manifest and layer files are the proving ground's actual
      data path.
- [ ] `CHANGES.md` has the Phase 1 numbers.

## 8. Failure branches

| If | Then |
|---|---|
| Slang blocks progress on Metal | MSL first behind the shader interface; add Slang when the second backend starts |
| The second backend exposes an RHI flaw | Fix the interface now; this is the intended outcome |
| 100k instances miss budget | Diagnose with pass timers: culling, vertex cost, overdraw or bandwidth. Adjust LOD error thresholds and part budgets before touching architecture |
| Box3D integration destabilises the loop | Ship the loop with a kinematic stub behind `ed_sim`; integrate Box3D at the start of Phase 5 |
| egui painter on the RHI is costly | Keep it; it is developer UI. Limit redraw rate |
| Picking readback stalls | Read back asynchronously, one frame late |

## 9. What Phase 2 receives

A renderer that accepts new passes and new instance sources without restructuring; an RHI
proven on two APIs; a chunk format and bake tool; node and object ids in use; a harness that
measures load time and frame time; an editor that Phase 2 extends with go-to and node
overlays.
