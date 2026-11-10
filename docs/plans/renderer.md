# Renderer — `ed_gpu` and `ed_render`

Design brief. Decisions: D1, D14, F3, F7, F9, F12 and the P1-D series in `../DECISIONS.md`.
Built from Phase 1; the planet passes arrive in Phase 2, lighting targets in Phase 4, and
cluster geometry on Track G (`../ROADMAP.md`).

## Goals

1. Hold 60 fps on the High tier (M3 Pro, 1440p) in a dense city and a dense forest with
   fixed memory, including while streaming.
2. Draw everything from a pebble to the Moon in one frame with one renderer.
3. Look at least as good as the alpha's Metal path in the same scenes.
4. Accept cluster ("Nanite-style") geometry later without restructuring.
5. Run on a second GPU API without a rewrite.

## 1. What the alpha's Metal renderer taught us

It looks good and runs badly. The causes are structural, not shader quality:

| Symptom | Cause | New design |
|---|---|---|
| Low frame rate in cities | The frame is vertex-bound: about 16.5 million triangles of merged per-tile meshes, drawn again for the depth prepass and each shadow cascade | Instances of shared parts, GPU culling per instance, LOD per instance, far rungs instead of far detail (`world.md` §2) |
| Slow loading | Large merged meshes generated and uploaded whole | Small content-addressed chunks, staged uploads, coarse rungs first |
| Gaps | Render geometry mirrored from Bevy entities; tiles visible before complete | No mirror; a parent draws until children are resident |
| Every change made three times | Bevy renderer + native renderer + menu globe | One renderer |

What it got right and we keep as the **look reference**: the atmosphere, volumetric clouds,
tone mapping and exposure, MetalFX temporal upscaling, the terrain splat material. Phase 0
captures reference images of these (phase-0 W2). The shader maths is ported with tests onto
the new structure (P1-D18). The ocean FFT, probe GI and shadow caching are not ported
(D13, D14).

## 2. `ed_gpu` — the hardware interface

- Public API: device, buffers, textures, samplers, pipelines, bindless tables, command
  encoding, timers, capability probe. No backend types are visible.
- **Resource states are explicit in the API** (read, write, present, copy source, ...), even
  though Metal ignores them. The frame graph computes transitions; the Metal backend drops
  them; D3D12 and Vulkan need them. This is the reason for F7.
- **Bindless by default** (P1-D9): one table of textures and one of buffers per frame, indexed
  from instance and material data.
- **Capability probe** (P1-D10) decides tiers and feature flags at startup.
- **Second backend in Phase 1** (F7): the smallest D3D12 or Vulkan backend that renders the
  proving-ground scene. It need not be fast or complete; it exists to fail early wherever the
  interface assumed Metal.
- Shaders: Slang source compiled to each backend at build time (P1-D1).

## 3. Geometry providers (F3)

```
trait GeometryProvider {
    fn cull_and_emit(&self, views: &[View], instances: &InstancePools, out: &mut VisibilityTargets);
}
```

Shared by all providers:

- **Instance pools** on the GPU: transform relative to node origin, previous-frame transform,
  mesh id, material id, `ObjectId` low bits for picking, flags.
- **Views**: main camera, shadow views, reflection views, editor viewports.
- **Depth pyramid** from the previous frame for occlusion.
- **Outputs**: depth, a visibility buffer (instance id + primitive id), motion vectors.

| | Provider 1: discrete LOD | Provider 2: cluster DAG (Track G) |
|---|---|---|
| Pack data | LOD chain per mesh (required) | Cluster DAG chunk (optional) |
| Culling | Compute: frustum, horizon, depth pyramid, per instance | Compute: per instance, then per cluster, two-phase |
| LOD | One level per instance by projected error, dithered cross-fade | Cluster cut by projected error, watertight |
| Raster | Indirect draws grouped by mesh | Hardware raster for large triangles, compute raster for small |
| Status | Phase 1 gate | Parallel; adopted per content type when it wins on measurement |

A mesh without a cluster chunk always works through provider 1. Provider 2 can therefore be
turned on for scanned rocks first, buildings later, or never for some content.

Material shading reads the visibility buffer and shades each pixel once, regardless of which
provider drew it.

### Track G — cluster geometry, in parallel (D17)

1. Bake: build cluster DAGs with meshoptimizer's cluster-LOD builder; store as the optional
   chunk. Determinism test on the bake.
2. Runtime: cut selection in compute; hardware raster only.
3. Compute raster for small triangles (needs 64-bit atomics or the 32-bit packed path chosen
   by the capability probe).
4. Streaming of cluster pages with fixed memory.
5. Measure against provider 1 on the reference scenes; adopt per content type.

Track G never blocks a phase gate.

## 4. Planet-scale rendering

- Terrain nodes are grids displaced on the ellipsoid in the vertex stage, positioned relative
  to the camera in f32 from f64 node origins.
- **Horizon culling** removes nodes and instances below the horizon before the frustum test;
  from the ground this removes most of the planet.
- Layers (albedo, material weights, emissive, height detail) are **virtual textures** with
  page ids that match `NodeId`.
- One **atmosphere** model (precomputed transmittance and scattering tables, aerial
  perspective applied to everything including far rungs) is correct at the ground, from an
  aircraft and from orbit.
- Clouds: one field; volumetric near the camera, a lit shell from far away.
- Other bodies render with the same passes and their own constants. The Moon from Earth is a
  terrain body at L0–L2, not a sprite.
- Depth: reverse-Z float with an infinite far plane gives usable precision from 10 cm to
  beyond the Moon in one pass.

## 5. Lighting

| Tier | Shadows | Indirect light | Reflections |
|---|---|---|---|
| Baseline (Low, and all tiers until replaced) | Cascaded sun shadows; far terrain shadow from height data | Sky ambient from the atmosphere model, GTAO | Sky + screen-space |
| High | Virtual shadow maps | Voxel dynamic GI | Screen-space, sky fallback |
| Ultra | Virtual shadow maps | Voxel dynamic GI at higher resolution | Hardware ray tracing |

The baseline exists from Phase 1 (F9). VSM and voxel GI are built in Phase 4 and become the
default on a tier when the harness shows they fit that tier's frame and memory budget on the
reference scenes. The 18 GB M3 Pro is the High tier reference; GI volume size and shadow page
pool are sized to it, not the other way round.

Far rungs are lit by the same sun and atmosphere, with baked normals and emissive; they do
not take part in GI voxelisation.

## 6. Frame

```
update instance pools → cull (compute) → depth/visibility (providers) → depth pyramid
→ shadows → lighting inputs (GI, AO) → material resolve → atmosphere + clouds + water
→ transparents → upscale/TAA → post → UI/editor
```

A frame graph (P1-D3) owns transient targets and resource states. Each pass has a GPU timer;
the harness writes per-pass times.

## 7. Implementation plan

| Phase | Work | Gate |
|---|---|---|
| 0 | Look-reference captures and perf baseline from the alpha; Solstice and meshoptimizer cluster study on the M3 Pro; RHI contract reviewed against D3D12 and Vulkan | Captures committed; study written |
| 1 | `ed_gpu` Metal + minimal second backend; frame graph; instance pools; provider 1; visibility buffer and resolve; PBR and toon v0; baseline lighting; picking; multi-view; egui; harness | 100k instances of mixed parts at 60 fps on High with per-pass timings; same scene on the second backend |
| 2 | Terrain nodes, virtual texturing, atmosphere, clouds shell, ocean surface v0, horizon culling, Moon | Unbroken orbit ↔ ground recording |
| 3 | Rungs R0–R2 drawing through provider 1, cross-fades, foliage ladder | Reference cities at 60 fps on High with fixed memory |
| 4 | VSM, voxel GI, full water, volumetric clouds, material library, hardware RT on Ultra | Look matches or beats the Phase 0 captures at budget |
| G | Track G steps 1–5 | Per-content-type adoption by measurement |

## 8. Open questions

1. D3D12 or Vulkan for the Phase 1 second backend (D3D12 serves the Windows target directly;
   Vulkan can be developed on the Mac through MoltenVK).
2. Visibility-buffer format on GPUs without 64-bit atomics (only matters for Track G step 3).
3. Whether voxel GI on High needs a reduced update rate; decided by Phase 4 measurement.
