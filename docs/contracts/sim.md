# Contract: simulation (`ed_sim`)

Owner: `ed_sim`. Decisions: D9, F10, P1-D16, P1-D17. Evidence:
`../reference/spikes/w4a-box3d.md`.

## 1. Step

```rust
pub const TICK_HZ: u32 = 60;
pub const SUBSTEPS: u32 = 4;

pub struct SimInput<'a> {
    pub tick: u64,
    pub commands: &'a [Command],     // sorted by (entity, sequence)
    pub utc: f64,                    // simulated UTC seconds, from the tick, never the wall clock
}
impl World {
    pub fn new(config: &SimConfig) -> World;          // seed, data version, region origin
    pub fn step(&mut self, input: &SimInput) -> StepOutput;
    pub fn snapshot(&self) -> Snapshot;               // complete, self-contained
    pub fn restore(&mut self, s: &Snapshot);
    pub fn state_hash(&self) -> u64;
}
```

- The step is exactly `1 / 60` s with 4 sub-steps. The render loop accumulates time and
  interpolates between the last two states; it never passes a variable step.
- `step` reads only `SimInput`, the world state, and collision geometry delivered through
  section 4. It must not read the clock, the frame rate, input devices, random sources other
  than the world's seeded generator, or anything from `ed_render`.
- `ed_sim` depends on no render crate (enforced by `cargo xtask check-deps`).

## 2. Handles and components

```rust
pub struct Entity { index: u32, generation: u32 }
```

- All access is through `Entity` handles and typed accessors. No Box3D type or pointer is
  public. Box3D is vendored C at a pinned commit behind a private binding.
- Entities are created in a canonical order: commands are applied sorted by
  `(entity, sequence)`, and bodies streamed in from chunks are created sorted by `ObjectId`.
  Creation order is part of determinism.
- Body kinds: static (chunk collision), kinematic (movers, platforms), dynamic. Shapes:
  box, sphere, capsule, convex hull, compound, triangle mesh, height field.
- Character mover: a capsule driven by the Box3D mover query, with step height, slope limit
  and ground snapping as parameters.
- Vehicles are our models on top of physics queries, not library vehicles: ray or shape cast
  suspension, tyre model, drivetrain. Wheel joints are available for simple wheeled props.

## 3. Space

- The sim works in a **region frame**: f64 origin at a body-fixed point, axes east, north,
  up there. Box3D is built with double-precision positions.
- A region covers the area around a player group. When players move more than 8 km from the
  origin the region is **re-based**: a new origin, all positions shifted, velocities
  unchanged. A re-base happens on a tick boundary and is recorded in the replay.
- Gravity is toward the body centre, evaluated per body, so the same code works on the
  ground, in orbit and on the Moon. Orbital motion outside physics range is `ed_space`'s.

## 4. Collision streaming

- Static collision comes from `collision` chunks (`pack.md` kind 6), one static body set per
  node, added when the node is within the collision radius of any dynamic body and removed
  when none is.
- A dynamic body may not move into a node whose collision is not loaded: it is frozen (kept
  kinematic at rest) until the chunk arrives. The streaming system prioritises those nodes.
- Add and remove happen at tick boundaries, in sorted `NodeId` order.

## 5. Detail tiers

| Tier | Where | Cost |
|---|---|---|
| Full | Near any player | Rigid bodies, joints, mover |
| Kinematic | Mid range | Follows a path or schedule; collides as kinematic |
| Parked record | Anywhere else | A row: `ObjectId`, kind, body-fixed f64 transform, state blob, owner. No body exists |
| Statistical | Crowds, traffic far away | Counts per node, no entities |

Promotion and demotion happen at tick boundaries. A parked record promoted to full and
demoted again without interaction must return the same record.

## 6. Water and environment queries

```rust
pub trait Environment {
    fn water(&self, p: DVec3, utc: f64) -> Option<WaterSample>;   // height, velocity, normal
    fn air(&self, p: DVec3, utc: f64) -> AirSample;               // density, wind
    fn ground_material(&self, p: DVec3) -> SurfaceKind;
}
```

The environment is passed to `World::step` by the caller. Implementations must be pure
functions of `(position, utc, seed, data version)` so that server and client agree. Buoyancy,
drag and tyre grip use only these queries.

## 7. Snapshots and replay

- A `Snapshot` contains everything needed to continue: bodies, joints, sleeping state,
  solver warm-start data, entity tables, the random generator, the region origin, the tick.
- **Replay format:** `SimConfig`, an initial `Snapshot`, then per tick the commands and any
  re-base or collision add and remove. Replaying must reach the same `state_hash` at every
  recorded checkpoint on the same build and machine.
- Cross-machine and cross-platform equality is measured and reported in CI; it does not gate
  a phase (F10). Multiplayer does not depend on it: the server is authoritative.
- `state_hash` hashes positions, rotations and velocities of all bodies in entity order.

## 8. Tests

| # | Test |
|---|---|
| S1 | Replay: 3,000 ticks of the reference scene (height field, 1,000 boxes, 20 vehicles) reach the recorded hashes; run twice and across processes |
| S2 | Snapshot at tick N, restore, continue: hashes equal the uninterrupted run |
| S3 | Scalar build equals SIMD build on the reference scene (it did in the spike) |
| S4 | Cross-platform hash comparison, reported not gating |
| S5 | Re-base: a scene stepped across a re-base matches the same scene without one, within 1 mm after 600 ticks |
| S6 | Collision streaming: a body approaching an unloaded node is frozen and released; add and remove order does not change the result |
| S7 | Parked round trip: demote and promote without interaction returns the identical record |
| S8 | The step makes no allocation after warm-up and stays under 2 ms for the reference scene on the M3 Pro |
| S9 | Mover: step-up, slope limit and ground snapping cases |
| S10 | `ed_sim` builds with no render crate in its dependency graph (`check-deps`) |

## Decisions made during implementation

Phase 1, W5 (`ed_sim` on Box3D). Appended; sections above are unchanged.

- **Backend binding.** Box3D is vendored at `3362633` in `vendor/box3d` (see `VENDORED.md`) and
  compiled by `crates/ed_sim/build.rs` with `cc` (no CMake): `BOX3D_DOUBLE_PRECISION`,
  `-ffp-contract=off` (`/fp:precise` on MSVC), C17, `-O3` in every profile, `wide8.c` with
  `/arch:AVX2` on MSVC x86/x64. Rust never sees Box3D structs: a C shim
  (`crates/ed_sim/csrc/ed_box3d_shim.c`) exposes flat functions over small boundary structs,
  and `ffi::layout_guard` compares every size, alignment and offset against the C compiler.
- **One worker, no task callbacks.** The step runs serially. Box3D's global world table makes
  world creation and destruction non-thread-safe; `ed_sim` serialises them behind a mutex.
  At most 128 worlds may exist at once (Box3D limit).
- **Frame.** Public API is the region frame (x east, y north, z up), f64 positions, f32
  velocities. Box3D is y-up; the shim boundary maps `(e, n, u) -> (e, u, -n)`, a proper rotation,
  so quaternions convert by permuting components and mesh winding is preserved.
- **Height fields.** Row 0 is the north edge, rows run south, columns run east, the body origin
  is the north-west corner; heights are local z. Box3D quantises heights between the field's
  own min and max.
- **Meshes and height fields** are accepted on static bodies only and take no local offset;
  everything else (box, sphere, capsule along local z, hull, compound = several shapes) on
  any body kind.
- **`step` signature.** `step(&SimInput) -> StepOutput`; `SimInput.tick` must equal
  `World::tick()` and commands must be sorted, both asserted. `StepOutput` lists spawned
  entities `(issuer, sequence, entity)` and rejected commands with a reason. The
  `Environment` parameter is not added yet (no water, air or ground-material queries exist).
- **Spawning.** During play, bodies and movers are created by `Spawn` / `SpawnMover`
  commands (issuer may be `Entity::NONE`). `World::create_body` / `create_mover` exist for scene
  setup before recording starts; calling them mid-recording would not be replayed.
- **Entities.** Slot table with a LIFO free list; despawn bumps the generation. Deterministic
  because commands are applied in sorted order.
- **`state_hash`.** FNV-1a 64 over explicit little-endian bytes: tick, generator state, then in
  entity index order each slot's generation, kind, and the backend's exact bits for position
  (f64), rotation, linear and angular velocity (f32), plus mover state. Static bodies included.
- **Snapshots.** Our tables are cloned; Box3D's world is captured with its own internal
  serializer (`b3SerializeWorld`) and restored in place with `b3DeserializeIntoShell`, the path
  Box3D's replay player uses for keyframes. That image includes solver sets, contacts with
  warm-start impulses, islands, the constraint graph, broad-phase trees and sleep timers.
  Mesh, height-field and compound data referenced by restored shapes is copied into a
  per-world arena in the shim. `Snapshot::to_bytes` carries a build id (hash of the FFI
  layout); Box3D's image carries its own layout hash and precision flag. **Limit:** a snapshot
  restores only on the same build and architecture (raw struct images). Restore returns
  `Result` (the contract sketch had no return value): malformed input leaves the world
  untouched; a Box3D rejection after it began leaves the world unusable and stepping panics.
- **Replay.** Bytes = magic, initial `Snapshot` (which carries `SimConfig`), then per tick
  `(tick, utc, commands, hash after the tick)`. `Replay::run` reports the first diverging tick.
  Re-base and collision add/remove records are not in the format yet (neither exists yet).
- **Character mover.** Ours, on Box3D's mover queries: `b3World_CollideMover` planes, a Rust
  port of `b3SolvePlanes` / `b3ClipVector`, `b3World_CastMover` for the swept move (5
  iterations, as Box3D's sample). Step height: while grounded the collision capsule is lifted
  by `step_height`; steep upward planes are made vertical. Ground: a 0.9 r sphere cast down from
  the capsule's bottom sphere, then a ray down the capsule axis for the height and normal;
  walkable if within `max_slope`; reach `step_height + snap_distance` while grounded. Airborne:
  gravity integrates, steep planes may not lift the mover. The kinematic capsule body follows
  with `b3Body_SetTargetTransform`, so dynamic bodies are pushed; movers use category
  `layers::MOVER` and their queries exclude it, so movers do not block each other.
- **Not built yet:** re-basing, per-body radial gravity (gravity is uniform from
  `SimConfig`), collision streaming, detail tiers, parked records, joints, vehicles, the
  `Environment` trait. The seeded generator exists in state and hash but nothing draws from it.
- **Clock reads.** None in `ed_sim`. Box3D's internal profiler reads a timer during the step for
  its own `b3Profile` numbers only; those never feed back into the simulation.
- **Lockfile.** `ed_sim` gains `cc` (build), `glam` and `libm`, all already in the lock.
