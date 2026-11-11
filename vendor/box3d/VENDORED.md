# Box3D (vendored)

- Upstream: https://github.com/erincatto/box3d
- Commit: `336263320469e5e9febfe18697ab562488f7e97a` (2026-10-27), the commit measured by the
  Phase 0 spike (`spikes/box3d`, `docs/reference/spikes/w4a-box3d.md`).
- Licence: MIT (`LICENSE`, copyright 2026 Erin Catto).
- Copied from the spike checkout `spikes/box3d/src` (clean at that commit) on 2026-11-10.

## What is here

`include/box3d/*.h` and `src/*.c|*.h|*.inl` unchanged, plus `LICENSE` and `README.md`.
Left out: `samples/`, `benchmark/`, `docs/`, `test/`, `shared/`, `extern/`, `data/`, the CMake
and IDE files (`CMakeLists.txt`, presets, `build*.sh|bat`, `box3d.natvis`, `box3d.pc.in`).
Do not edit these files; bump by copying a new upstream commit over them and updating this
note, then run `cargo test -p ed_sim` (the FFI layout guard and the snapshot layout hash catch
ABI drift).

## How it is built

`crates/ed_sim/build.rs` compiles every `src/*.c` with the `cc` crate (no CMake), as one
static library, together with our shim `crates/ed_sim/csrc/ed_box3d_shim.c`.

Defines and flags (mirroring upstream `CMakeLists.txt` at this commit, plus double precision):

| Setting | Value | Why |
|---|---|---|
| `BOX3D_DOUBLE_PRECISION` | defined | f64 world positions (P0-R3, `sim.md` section 3) |
| `NDEBUG` | defined unless Rust debug assertions are on | Box3D asserts follow the Rust profile |
| `BOX3D_VALIDATE` | not defined | heavy validation off |
| `BOX3D_DISABLE_SIMD` | not defined | NEON / SSE2 / AVX2 as upstream picks; the spike showed scalar == NEON hashes |
| `-ffp-contract=off` (gcc, clang), `/clang:-ffp-contract=off` (clang-cl), `/fp:precise` (MSVC) | on | no FMA contraction: upstream's deterministic-math setting |
| C standard | `-std=gnu17` (gcc, clang), `/std:c17` (MSVC) | upstream uses C17 with extensions |
| `wide8.c` on MSVC x86/x64 | `/arch:AVX2`, compiled separately | upstream sets this per file; runtime dispatch picks the path |
| Optimisation | `-O3` / `/O2` in every Cargo profile | the physics is slow unoptimised; results do not depend on it (no fast-math) |
| Threads | one worker, no task callbacks (set at world creation) | determinism |
