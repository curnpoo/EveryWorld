# Track G — Cluster geometry (parallel)

Status: may start during Phase 1; never blocks a phase gate (D17, F3). Design:
`../plans/renderer.md` §3. Evidence: `../reference/spikes/w4b-geometry.md` (P0-R6).
Decisions in the G-D series are **proposed**.

## 1. Goal

A second geometry provider that draws meshes as a cluster hierarchy with a continuous,
crack-free level of detail, plugged into the same slot as the discrete-LOD provider, and
adopted per content type only where it measures better.

## 2. What Track G proves

| Question | Proof |
|---|---|
| Can cluster geometry enter without changing the engine? | Provider 2 registers through `GeometryProvider`; material resolve, shadows and picking are untouched |
| Is it faster or better on our content? | The adoption protocol of G-D6 on the reference scenes |
| Does it run on every supported GPU? | A hardware-raster path everywhere; the compute raster chosen by `Caps` |

## 3. What Track G does not do

It does not replace provider 1, which every mesh keeps (P1-D11). It does not change pack
keys, instance pools or the visibility buffer format seen by material resolve.

## 4. Decisions in detail (proposed)

### G-D1 — Bake
`ew-bake` builds the hierarchy with meshoptimizer's `clusterlod` (P0-R6) and stores it as
the optional cluster chunk of a mesh: clusters of bounded triangle count, group bounds and
errors, parent links, pages. The bake is deterministic and tested by hash; the spike
measured about 1 M triangles per second and 2.0× stored triangles.

### G-D2 — Cut selection on the GPU
Per view, a compute pass culls instances, then cluster groups (frustum, then the depth
pyramid in two phases), and selects the cut where a group's projected error is under the
pixel threshold and its parent's is not. Output is a visible-cluster list.

### G-D3 — Hardware raster first
Step 2 draws the visible clusters with indirect draws through the hardware rasteriser into
the same depth and visibility targets as provider 1. This alone must be correct and
crack-free before any software raster exists.

### G-D4 — Compute raster for small triangles
Clusters whose triangles are near pixel size go to a compute rasteriser writing depth and
id with 64-bit atomics where `Caps` reports them (Apple9 and later on Metal), and through a
32-bit packed two-pass path elsewhere (`renderer.md` open question 2). Large triangles stay
on hardware raster.

### G-D5 — Streaming
Cluster pages are chunks requested from GPU feedback (the cut wanted finer pages than are
resident), loaded through the Phase 2 loader under its upload budget, in a fixed-size page
pool with eviction by error contribution. The coarsest pages of every resident mesh are
always resident, so there is always something to draw.

### G-D6 — Adoption protocol
Per content type (scanned rocks, kit parts, building shells, terrain features, foliage
trunks), on the reference scenes, with the harness: provider 2 replaces provider 1 for that
type only if frame time **and** memory are both no worse and image error against a
ground-truth render at full detail is equal or better. The result per type is recorded in
`CHANGES.md` either way. Types that lose stay on provider 1.

### G-D7 — Shadows and other views
Provider 2 serves shadow views through the same cut selection with the shadow view's error
metric; until that is measured, shadow views use provider 1 for all content.

## 5. Budgets

| Item | Budget | Basis |
|---|---|---|
| Bake throughput | 1 M triangles per second | Spike measurement |
| Stored size | 2.0× source triangles | Spike measurement |
| Cut selection | 1 ms | Same as the instance cull budget (P1-D20) |
| Cluster page pool | 512 MB on High | Inside the Phase 3 GPU total |
| Frame and memory in an adopted type | no worse than provider 1 | G-D6 |

## 6. Steps

| # | Step | Owns | Done when |
|---|---|---|---|
| G1 | Bake into the optional chunk; determinism test | `ed_assets`, `ed_pack` chunk payload | Same hash on two runs and two platforms |
| G2 | Stub provider drawing the coarsest cut | `ed_render/src/provider2*` | Proving ground renders identically within tolerance |
| G3 | Cut selection and hardware raster | same | No cracks on a turntable test at every error threshold |
| G4 | Compute raster, both atomics paths | same, `ed_gpu` caps | Matches hardware raster output within one depth step |
| G5 | Page streaming with a fixed pool | `ed_render`, `ed_tiles` loader | Fly-through with the pool never exceeded and no missing geometry |
| G6 | Measurement and adoption per type | harness | A table per content type in `CHANGES.md` |

Order: G1 → G2 → G3 → (G4, G5 in parallel) → G6. G1 and G2 can begin once Phase 1's M3
delivers the pack writer and M2 the provider interface.

## 7. Exit

Track G has no single gate. It ends when every content type has a recorded adoption
decision.

## 8. Failure branches

| If | Then |
|---|---|
| Hardware raster of clusters is slower than discrete LOD at equal error | Expected for coarse content; record it and adopt only for dense scanned meshes |
| The 32-bit compute path is too slow | Compute raster only where 64-bit atomics exist; hardware raster elsewhere |
| Page feedback causes visible late detail | Bias the cut one level coarser while pages load; coarsest pages stay resident |
| No content type wins | Provider 1 remains the only provider; the chunk stays optional and unbaked |
