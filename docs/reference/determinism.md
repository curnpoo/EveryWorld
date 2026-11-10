# Determinism audit (Phase 0, W3)

Date: 2026-11-10. Machine: Apple M3 Pro, macOS. Suite: `cargo xtask golden`
(`crates/ed_world/tests/golden.rs`).

## What is tested

Eight inputs: six real places at tile zoom 14 (Shibuya, Zermatt, Wadi Rum, Schwarzwald,
Amalfi, Beauce) built from recorded source tiles with the network refused, and two synthetic
inputs with no data at all (open ocean; a coarse zoom-9 tile), which exercise every fallback.

Nine hashes per input: terrain, surface, water, roads (mesh and nav graph), buildings far,
buildings LOD, buildings detail (every chunk built), foliage, props. Positions are quantised
to 1 mm, normals to 0.01, colours to 1/255, before hashing.

Each input is built twice in one process and compared; the result is compared to
`tests/golden/hashes.txt`, which macOS and Windows CI share.

## Results

| Check | Result |
|---|---|
| Two builds in one process | Pass, after one fix (below) |
| Two processes on one machine | Pass |
| macOS versus Windows | Runs in CI on the first push of this branch; not yet observed |

72 hashes. A full run takes about 50 s in release on the M3 Pro.

## Sources of nondeterminism

| # | Source | Where | Status |
|---|---|---|---|
| 1 | `HashMap` iteration order decided the vertex order of a merged mesh | `tilegen.rs`, far building massing (`build_massing(chunks.values()...)`) | **Fixed**: chunks are visited in sorted key order |
| 2 | Per-material mesh buckets are a `HashMap<Mat, MeshData>` | `bparts::Out` | Harmless today (each bucket is deterministic; consumers must not depend on bucket order). Replace with an ordered map when the output changes to instances in Phase 3 |
| 3 | Detail chunks are a `HashMap<(u8, u8), _>` | `tilegen::DetailSet` | Same as 2 |
| 4 | Environment variables change generator behaviour (`EW_ROADS`, `EW_OVERTURE`, `EW_OSMTAGS`, `EW_ROADZ`, `EW_INFER_BRIDGES`, `EW_SCATTER_MAX`, `EW_FAR_SHADOWS`) | `ed_world`, `ed_data` | Open. In the new engine these become fields of the generator version (contract `generation.md`), never ambient state |
| 5 | A cache miss and a network failure produce a different world from a cache hit (fallback elevation zoom, missing roads) | `tilegen::build` | Open by design in the alpha. In the new engine a bake either has all its pinned inputs or fails; there is no silent fallback (contract `generation.md`) |
| 6 | Cached files with a time-to-live (the vector tile JSON, 7 days) | `ed_data::vector::tile_template` | Avoided in the suite by storing the template. Disappears with pinned releases |
| 7 | Wall clock, unseeded randomness, thread-order reductions | searched for | None found in `ed_world` generation paths. Scatter and style choices are seeded from tile and feature ids |

Not covered yet: trees' full-detail meshes (only instances and impostor placement are
hashed), ground scatter, the sound map, and anything in the crates that are not generators.
Float results could still differ between CPU architectures in transcendental functions
(`sin`, `powf` go through the platform maths library); the Windows CI run is the first
evidence either way. If it differs, the candidates are listed in rows 4 to 6 first, then
maths-library calls in noise and projection code.

## Rule carried into the contracts

A generator may not iterate an unordered container when the order can reach its output, read
the environment, read the clock, or continue after a missing input.
