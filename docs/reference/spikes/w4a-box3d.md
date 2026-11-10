# W4a — Box3D

Verdict: **pass. Use Box3D, as vendored C behind our own binding in `ed_sim`.**
Date 2026-11-10, Apple M3 Pro. Code: `spikes/box3d` (`./run.sh`). Upstream commit
`3362633` (2026-10-27).

## Scene

A 65 x 65 height field (4 m cells), 1,000 boxes dropped as a pile, and 20 four-wheel
vehicles (hull chassis, sphere wheels on wheel joints with suspension, limits and spin
motors) driving over the hills. 1,100 bodies. 60 Hz, 4 sub-steps, one thread.

## Results

| Check | Result |
|---|---|
| Same hash of every body transform, three separate processes | Identical (`4d2548b348814593` at step 600) |
| NEON build versus `BOX3D_DISABLE_SIMD` build | **Identical hash** |
| Step cost, 1,100 bodies, pile settling | 0.47 to 0.51 ms mean, 2 to 5 ms worst (the frame the pile lands) |
| Step cost over 3,000 steps (pile asleep, 100 vehicle bodies awake) | 0.17 ms mean |
| Scalar build | 0.68 ms mean |
| Double-precision build (`BOX3D_DOUBLE_PRECISION`) | Works; positions are f64; 0.53 ms mean |
| macOS versus Windows | Not run (no Windows machine in this session). Feeds F10 |

The 16.7 ms frame has room for this many times over on one thread; the library also takes a
task system for threading, which was not used.

The scalar and NEON builds agreeing bit-for-bit is the strongest sign available on one
machine that results are not tied to an instruction set. It is not proof of cross-machine
agreement; F10 stands (replays are tested, cross-machine exactness does not gate a phase).

## Features needed

| Need | Present |
|---|---|
| Character mover | Yes: `b3World_CastMover`, mover joint (added upstream on 2026-10-27) |
| Height field collider | Yes, used in the spike |
| Triangle mesh, convex hull, compound | Yes |
| Wheel joint with suspension, steering and spin motor | Yes, used in the spike |
| Ray and shape casts (raycast wheels, picking) | Yes |
| Continuous collision | Yes (`enableContinuous`, `isBullet`) |
| Large worlds | Yes: double-precision positions as a build option |
| Recording | A recording API exists; not evaluated |

## Form: vendored C or a Rust port

Crates on 2026-11-10: `box3d-rust` 0.5.0 (a pure Rust port), `box3d-sys` / `box3d` and
`boxddd-sys` / `boxddd` (bindings). **Not benchmarked here**; the choice is made on
maintenance grounds:

- Upstream changes weekly and is the only version its author tests. A port is a second
  implementation that can drift in behaviour and in exact results.
- The binding crates last published in August to September; upstream has added the mover
  joint since.
- The surface `ed_sim` needs is small (world, bodies, a few shapes, wheel and mover joints,
  casts), so our own binding over a pinned commit is little code and pins exact behaviour.

**Decision:** vendor the C source at a pinned commit, build it with `cc` from `ed_sim`'s
build script, and write a thin private binding. `ed_sim` exposes handles, not Box3D types
(contract `sim.md`). Revisit the Rust port only if the C build blocks a platform.

## Carried into Phase 1 and the contracts

- Use the double-precision build with a per-region origin, so the sim frame can sit anywhere
  on the planet (decide single versus double in `sim.md`; the measured cost is about 10%).
- Fixed 60 Hz step and fixed sub-step count are part of the replay format.
- Body creation order is part of determinism: create bodies in a canonical order
  (sorted by `ObjectId`).
