# W4d — Addressing and far field

Verdict: **pass.** Date 2026-11-10, Apple M3 Pro. Code: `spikes/addressing`
(`cargo run --release`), `spikes/globe_pack/measure.py`.

## 1. Cube-to-sphere mapping

| Mapping | Node area max/min | Aspect max | Edge length max/min |
|---|---|---|---|
| Gnomonic (plain normalise) | 5.03 (L6), 5.12 (L12) | 1.41 | 2.09 to 2.11 |
| S2 quadratic | 2.02 (L6), 2.07 (L12) | 1.41 to 1.44 | 1.74 to 1.80 |
| **Tangent warp**, `tan(s * pi/4)` | **1.40** (L6), **1.41** (L12) | 1.38 to 1.41 | **1.38 to 1.41** |

**Chosen: the tangent warp.** Node area varies by 1.4x across the planet instead of 5x, so
one screen-error rule and one texel density serve every node.

Mean node edge: 144 km at L6, 2.3 km at L12, 0.3 km at L15 (the 156 km and 2.4 km figures
in the briefs are equator-of-face values; both are correct).

## 2. Edge agreement

With 1,025 lattice points per edge, every boundary point of every face has a bit-identical
twin on a neighbouring face for all three mappings, and parent and child lattice points agree
bit-for-bit from L0 to L15. Three rules make this true, and they go into
`contracts/addressing.md`:

1. **Evaluate the warp from integer lattice indices**, never from a float coordinate. Return
   exactly -1, 0 and +1 at the ends and centre, and compute the upper half as the negation of
   the lower half. (`tan(pi/4)` is 0.9999999999999999 in f64; evaluating at a float 1.0 would
   crack every face edge.)
2. **Normalise in body axes** (x, y, z order), not in face axes, so two faces sum the same
   squares in the same order.
3. **Canonicalise negative zero** (`+ 0.0`) after normalising. Faces that negate an axis
   otherwise produce -0.0 where their neighbour produces 0.0. The first run of the spike
   found exactly this at the 8 edge midpoints that have a zero coordinate.

Open point for the contract: `tan` comes from the platform maths library and is not
guaranteed identical on macOS, Windows and Linux. The engine uses a software `tan`
(the `libm` crate) for the warp so that baked chunks and clients agree everywhere.

## 3. Globe pack size

80 points sampled uniformly on the sphere (62% ocean). One 256 x 256 tile at each, at about
600 m per sample, which is the L6 node resolution. Development-only sources, used for
measurement and not kept.

| Layer | Encoding measured | Mean per node |
|---|---|---|
| Height | 16-bit, 1 m steps, left/up prediction, best of zlib and LZMA | 29.9 KiB |
| Colour | JPEG quality 90 | 6.4 KiB |
| Night lights | 128 x 128, 8-bit, zlib | 3.0 KiB |

Extrapolated to all 32,766 nodes of L0 to L6:

| Colour storage | Globe pack |
|---|---|
| Transcoded at load from a JPEG-class or supercompressed format | **1.2 GiB** |
| BC7/ASTC plus general compression (assumed 55% of fixed rate on land, 5% on flat nodes) | 2.1 GiB |
| BC7/ASTC fixed rate, stored raw | 3.0 GiB |

Findings:

- The target of about 2 GB holds. No level needs to be dropped.
- **Height is the largest layer, and most of it is ocean floor**: bathymetry at 1 m steps
  costs 15 to 22 KiB per ocean node. Storing depth below sea level at coarser steps (4 m) in
  L0 to L6 would cut the pack further if needed; nobody walks the sea bed at 600 m sampling.
- The middle row is an assumption, not a measurement. The pack contract should prefer a
  supercompressed colour format that transcodes to the GPU format at load.

## 4. Decisions

| Item | Decision |
|---|---|
| Mapping | Tangent warp, evaluated per the three rules above, with a software `tan` |
| Layer resolution, L0 to L6 | Height 257 x 257 (shared edges), colour 256 x 256, night lights 128 x 128, the same at every level |
| Globe pack levels | L0 to L6 as planned (F4) |
| Colour format | Supercompressed, transcoded at load; decided finally in `contracts/pack.md` |
