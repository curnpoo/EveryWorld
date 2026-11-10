# W4d — Addressing and far field (in progress)

Spike: `spikes/addressing` (`cargo run --release`). First run 2026-11-10, Apple M3 Pro.

## Measured so far

| Mapping | Node area max/min | Aspect max | Edge length max/min |
|---|---|---|---|
| Gnomonic (plain normalise) | 5.03 (L6), 5.12 (L12) | 1.41 | 2.09 to 2.11 |
| Tangent warp, `tan(s * pi/4)` | 1.40 (L6), 1.41 (L12) | 1.38 to 1.41 | 1.38 to 1.41 |

- The tangent warp is the leading candidate: node area varies by 1.4x instead of 5x.
- Parent and child lattice points agree bit-for-bit from L0 to L15 when the warp is
  evaluated from integer lattice indices with exact values at the ends and centre, and odd
  symmetry about the centre. `tan(pi/4)` is not exactly 1 in f64, so evaluating the warp from
  a float coordinate would open cracks at face edges.
- Mean node edge: 144 km at L6, 2.3 km at L12, 0.3 km at L15.

## Not yet trustworthy

- The S2 quadratic row in the spike output is wrong (the formula in the spike is incorrect);
  ignore it until fixed.
- The edge test reports 8 boundary points in total without a bit-identical twin on another
  face. Not yet investigated; most likely an error in the spike's face axis table at some
  cube corners. The mapping is not chosen until this reads 0.

## Still to do

- Fix the two items above and choose the mapping.
- Bake sample R3/R4 layers for L0 to L6 nodes and extrapolate the globe pack size.
- Decide layer resolution per level.
