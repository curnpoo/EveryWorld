# W4b — Geometry references

This spike cannot fail the phase; it supplies measured inputs to Track G. Date 2026-11-10,
Apple M3 Pro (18 GPU cores, Metal 4). Code: `spikes/geometry` (`./run.sh character.glb`).

## 1. Cluster LOD bake with meshoptimizer (`clusterlod.h`, commit `f0a684f`)

128-triangle clusters, positions only, welded first.

| Mesh | Source triangles | Clusters | Groups | Levels | Stored triangles | Bake time | Second bake |
|---|---|---|---|---|---|---|---|
| Owner's character | 30,981 | 510 | 37 | 9 | 61,556 (1.99x) | 41 ms | identical hash |
| meshoptimizer sample (pirate) | 5,010 | 86 | 10 | 7 | 9,931 (1.98x) | 5 ms | identical hash |
| Generated terrain grid | 980,000 | 16,025 | 997 | 14 | 1,959,015 (2.00x) | 936 ms | identical hash |

- **Throughput is about 1 million triangles per second on one thread**, and each level
  halves the triangle count, so a full hierarchy stores twice the source triangles.
- **The bake is repeatable** in one process. Cross-platform repeatability is not tested;
  baked clusters are shipped as data, so it is not required.
- A building was not baked: the generator does not yet emit per-building meshes (that is the
  Phase 3 change). The hard cases for buildings are known from the literature and remain to
  be measured: thin parts, many disconnected pieces, and UV or material seams, all of which
  lock vertices and slow simplification.

## 2. Apple GPU capabilities needed by a visibility-buffer path

| Capability | M3 Pro |
|---|---|
| 64-bit atomics in shaders (`atomic_ulong`, atomic max) | Compiles at Metal language versions 3.0, 3.1, 3.2 and 4.0. Execution not tested |
| Argument buffers | Tier 2 |
| Mesh shaders, ray tracing, sparse textures | Reported as supported |
| Largest single buffer | 10.0 GiB |
| Recommended working set | 13.3 GiB of the 18 GiB |

A software rasteriser for small triangles (depth and id packed in one 64-bit value, written
with atomic max) is therefore expressible on this GPU. Whether it is fast enough is a Track G
measurement.

## 3. Solstice (commit `17623cf`, 2026-11-10; reference only, F12)

Solstice is a Rust renderer on **WebGPU that runs in the browser**, not a native Metal
renderer. Its live demo was opened on the M3 Pro:

| Observation | Value |
|---|---|
| Runs on Apple GPU | Yes, WebGPU on the Metal 3 backend, 0 GPU errors |
| Scene | 21.98 M source triangles, 114 instances |
| After about 30 s of streaming | 215 K triangles drawn, 1.8 K clusters, 572 of them in the software rasteriser; 161 of 1,666 geometry pages; 41 MB downloaded |
| Textures | BC7 and BC5, 63 MB |
| **Frame rate** | **Not validly measured.** The page reported 10 fps, but the browser pane was not on screen, and a page that is not displayed is throttled. Open the demo in a visible Chrome window for a real number |

What it shows that matters to us:

1. Cluster selection, page streaming, a compute software rasteriser beside the hardware one,
   and voxel GI all run through WebGPU on Apple hardware, so nothing in the approach needs a
   feature Metal lacks.
2. It drew about 1% of the source triangles for that view. That ratio is the purpose of the
   technique and is the number Track G must reproduce.
3. It is a browser renderer with its own scene format. It is a design reference, not code to
   depend on (F12 stands).

## 4. Inputs to the Track G plan (`plans/renderer.md` section 5)

| Step | Input from this spike |
|---|---|
| Bake | Use `clusterlod.h` inside `ew-bake`. Budget: 1 M triangles per second per core; stored size 2x source triangles before compression |
| Format | Clusters of at most 128 triangles; per group a bounding sphere and error, per cluster the refined group. Goes into the geometry slot of `contracts/pack.md` |
| Runtime, first target | Hardware-rasterised clusters with per-cluster culling and error-based selection, on scanned props and terrain rock, where the gain is clearest |
| Runtime, later | Compute rasteriser for sub-pixel triangles using 64-bit atomics; needs an execution test and a timing on the M3 Pro first |
| Adoption test | A reference scene where discrete LOD needs more than the triangle budget and clusters do not; compare frame time and memory against provider 1 |
| Open measurements | A real building once Phase 3 emits them; cluster decode cost; streaming page size |
