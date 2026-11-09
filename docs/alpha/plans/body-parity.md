# Plan: Full body.rs animation parity for remote (multiplayer) players

Status: PLAN ONLY (no implementation). Revised 2026-11-07 after a full read of
`body.rs`, `player.rs`, `net.rs`, `ed_net`, `ed_server` and every `Player`
consumer in `ed_game`.
Companions: `docs/ROADMAP.md` Stage C, `docs/plans/multiplayer.md`,
`docs/ARCHITECTURE.md`, `docs/CHANGES.md`.

## 0. Scope

**Goal.** A remote player is rendered by the *same* `body.rs` rig as the local
player: the same meshes, the same materials, the same IK solver, the same
capture-point foot planting, the same wingsuit membranes, the same boot flames.
No second, simplified rig.

**Hard constraints (user, 2026-11-07).**

1. **The local player must look exactly as it does today.** Not "similar" —
   identical. Every change here is judged against that.
2. The remote player is what gets better. The local player is the control.
3. **Ragdoll physics is coming.** The rig must be built so that a ragdolled
   character looks the *same on both ends*. This rules out "re-simulate the
   ragdoll on the receiver" (Avian is not deterministic across machines —
   `docs/plans/multiplayer.md:42`), and rules out a rig whose parts cannot be
   individually addressed. See §6.

## 1. Philosophy (unchanged in spirit, sharpened)

The remote is a **proxy**, not a second character implementation.

- **Local player:** client-side prediction, instant input. Unchanged.
- **Remote player:** snapshot interpolation. Render ~100 ms in the past,
  interpolate between the two most recent snapshots, never snap.
  (Gambetta, *Entity Interpolation*.)
- **Animation is driven by derived velocity, not raw input.** Remote inputs are
  unknown; velocity is measured from position deltas. "Better to show a player
  doing something late than doing something they never did." (SnapNet.)
- **Replicate rig *inputs*, re-run the identical solver.** Both ends run the
  same code on the same inputs, so the pose matches by construction.
- **Never replicate joint transforms for the *animated* state.** That is what
  makes the procedural rig cheap (≈100 B/actor/tick) instead of a skeleton
  stream. *Exception:* ragdoll, where physics is non-deterministic and there is
  no input set that reproduces it (§6).
- **Never run Avian physics for remotes.** Kinematic only.
- **Never use skeletal animation clips.** The rig is procedural; a clip system
  would be a second, parallel animation stack.

## 2. Current state (verified 2026-11-07)

### 2.1 `crates/ed_game/src/body.rs` (1126 lines)

| Item | Location | Note |
|---|---|---|
| `BodyPart(Spec)` | `body.rs:96` | 58 part variants + 4 backpack + 3 wings |
| `WingMesh(usize)` | `body.rs:100` | `pub(crate)`; used by `native.rs:280` |
| `Spec` | `body.rs:81` | `Limb`/`Ball`/`Rigid`/`Eye`/`JetFlame`/`Canopy` |
| `J` / `F` joint + frame enums | `body.rs:50` / `body.rs:70` | 16 joints, 7 frames — **private** |
| `Gait` | `body.rs:153` | global `Resource`, one per *game*, not per player |
| `spawn_body` | `body.rs:179` | one global set of parts, **no owner link** |
| `animate_body` | `body.rs:435` | `players.single_mut()` at `:447`; 652 lines; writes **all** `BodyPart` entities |

`animate_body` reads 25 `Player` fields (`p.mode`, `p.yaw`, `p.cam_pitch`,
`p.throttle`, `p.wingsuit_on`, `p.afterburner`, `p.fwd_in`, `p.strafe`,
`p.wing_arch`, `p.wing_turn`, `p.fall_spread`, `p.fall_tuck`, `p.jump_charge`,
`p.jump_push_timer`, `p.crouch`, `p.landing_recovery`, `p.leg_steer`,
`p.intent_dir`, `p.rot_err`, `p.accel_smooth`, `p.ground_normal`,
`p.foot_ground_hit`) and writes back `p.foot_planted`, `p.foot_world_pos`,
`p.foot_up_dir` (consumed by `drive_player`, `player.rs:809`).

Parts are **top-level** entities with **absolute world-space** transforms,
recomputed every frame from the owner's transform. They carry no `Rebased` and
no `GeoAnchored` — they follow the owner implicitly.

### 2.2 `crates/ed_game/src/net.rs` (893 lines)

A second, hand-written rig: `RJ`/`RF`/`RSpec`/`RPart`/`RWing`
(`net.rs:328-343`), spawned as **children** of a `RemoteAvatar` root
(`net.rs:371`), posed by `net_update_avatars` (`net.rs:706`) with a sine-based
walk cycle and a `RemotePhase` accumulator (`net.rs:322`). It has no joints, no
capture-point stepping, no wingsuit membranes that follow real arms.

### 2.3 `crates/ed_net/src/lib.rs`

Protocol **v4** (`PROTOCOL_VERSION = 4`, `ed_net/src/lib.rs:5`).
`C2S::State`/`S2C::State` (`ed_net/src/lib.rs:16`, `:26`) carry: `pos` (ECEF
f64×3), `yaw`, `pitch`, `boot` (mode), `throttle`, `wingsuit`, `t`. **No
orientation beyond yaw, no velocity, no rig modifiers, no pose channel.**

### 2.4 `crates/ed_server/src/main.rs`

`StoredState` (`ed_server/src/main.rs:13`) mirrors the wire state and is
re-broadcast verbatim (lines 71-77, 121). Interest management: 60 Hz < 1 km,
15 Hz < 10 km, 5 Hz beyond.

## 3. Hard constraints found in the code (these reshape the plan)

These are the findings that a naive "just add a `Player` to the remote"
implementation gets wrong. Each one is a hard blocker, not a nit.

### 3.1 `Player` is a *single-player* component — 15 call sites would break

`Player` is treated as "the one local player" everywhere:

| Site | Call | Breaks if a remote also has `Player` |
|---|---|---|
| `player.rs:430` | `q.single_mut()` (`Query<&mut Player>`) | **local input dies** — no walking, no jumping |
| `player.rs:996` | `players.single_mut()` | camera stops following the player |
| `player.rs:108` | `q.single_mut()` | spawn settling never completes |
| `net.rs:291` | `q.single()` | **we stop sending our own state** — the whole game goes quiet |
| `hud.rs:225` | `players.single()` | HUD pill/stats read the wrong player |
| `boot_plume.rs:197` | `players.single()` | exhaust plume loses its velocity/ground source |
| `soundscape.rs:203` | `players.single().ok()` | wind sound source becomes the camera |
| `menu.rs:1050,1199,1274,1316,1460` | `players.single()` | map/pause focus jumps to the wrong place |
| `loading.rs:211,249` | `players.single()` | loading screen misbehaves |
| `vehicles.rs:83,149` | `players.single()` | vehicles spawn/enter/exit break |
| `spawn.rs:232` | `players.single_mut()` | teleport moves the wrong entity |
| `body.rs:447` | `players.single_mut()` | the rig stops animating |

`Query::single()` returns `Err(QuerySingleError::MultipleEntities)` for 2+
matches, and every one of these sites is written `let Ok(..) else { return; }`
— so they fail **silently**.

**Decision: remote proxies must NOT carry a `Player` component.** They get a
dedicated component. This is the single most important correction to the
original sketch.

### 3.2 Wing meshes cannot be shared between owners

`animate_body` rewrites wing **vertices in world space** every frame
(`body.rs:1079-1082`, `meshes.get_mut(&m3.0)` →
`insert_attribute(POSITION, ...)`).
With two rigs sharing the three wing meshes they would overwrite each other
every frame. **Each owner needs its own three wing meshes**; everything else
(cyl/ball/cube/dome/cone meshes and all `StandardMaterial`s) can be shared.

### 3.3 The boot plume is a single global

`BootNozzles` (`boot_plume.rs:49`) holds *two* nozzle slots for *one* actor,
and `animate_body` writes them from whichever actor it processes last
(`body.rs:1013-1019`). With N rigs, the local player's plume would follow the
remote's feet. The `nozzles` write must be **local-player only**.

Related: `Spec::JetFlame` hides its cone whenever the plume resource exists
(`body.rs:1021`, `p.throttle <= 0.01 || nozzles.is_some()`), because the
particle plume replaces the cone. A remote has no particle plume — if it used
the same rule it would show **no flame at all**. So flame rendering mode is
per-actor: `ParticlePlume` (local, when the plume plugin is on) vs `Cone`
(remote, and local in `--fly` where the plugin is absent).

### 3.4 The local rig's parts are top-level and absolute

Local parts are root entities with absolute world transforms; they are neither
`Rebased` nor `GeoAnchored`, so they follow the owner only because the solver
recomputes them each frame. Remote parts must use the same convention, or the
two rigs diverge in how they survive an origin rebase (a `Rebased` root moves
its children; a top-level absolute part does not).

### 3.5 Lookdev owns a second `Player`

`lookdev.rs:232` spawns `Player::idle(0.5, Walk)` for the character board,
and `main.rs:108` adds `body::BodyPlugin` **without** `player::PlayerPlugin`.
Today `spawn_body` runs unconditionally at Startup and `animate_body` finds
the lookdev dummy through `players.single_mut()`. Moving part spawning into
`player::spawn_player` would **delete the lookdev character**. The lookdev
dummy needs its own owned rig too.

### 3.6 `Cosmetics` is global

`cosmetics.backpack` decides backpack visibility at spawn
(`body.rs:338`) and is re-applied every frame (`body.rs:978-981`). A remote
would inherit the *local* player's backpack setting. Remotes get their own
cosmetics value (default: no backpack) until cosmetics are replicated.

### 3.7 `branches`: `main` and `windows` have diverged

Merge base `3dc96a7`. `windows` carries Windows-specific work (arboard
clipboard instead of `bevy_clipboard`, view-distance cuts, no meshlet plugin,
lite graphics defaults, HUD/menu text-write guards) in
`net.rs`, `hud.rs`, `menu.rs`, `streaming.rs`, `camera.rs`, `far_shadows.rs`,
`render_scale.rs`, `ed_world/src/lod.rs`, `ed_game/Cargo.toml`,
`docs/CHANGES.md`. The `net.rs` divergence is confined to `join_input`
(`net.rs:508`), adjacent to `net_spawn_avatars` — expect a conflict there and
resolve by keeping `windows`' arboard version.

## 4. Target architecture

```
                       ┌──────────────────────────────────────────┐
                       │              body.rs  (rig)              │
                       │                                          │
   local player  ─────►│  Player ──┐                              │
   (has Player)        │           ├─► RigState ─► solve_rig()   │
                       │           │        │         │           │
   remote proxy  ─────►│  RigState ─┘        │         ▼           │
   (has RigState)      │                    │      Pose (joints)  │
                       │                    │         │           │
                       │                    │         ▼           │
                       │            BodyOwner(owner) ──► parts    │
                       │            (58+4+3 entities)            │
                       └──────────────────────────────────────────┘
```

### 4.1 `BodyOwner`

```rust
/// Links a body piece (part, wing, flame) to the actor it belongs to.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BodyOwner(pub Entity);
```

Every part, accessory and wing carries it. `animate_body` filters parts by
owner instead of touching all of them.

### 4.2 `RigState` — the solver's input set

The solver stops reading `Player` directly and reads a plain struct. One
definition, two producers, no drift.

```rust
/// Everything the rig solver needs to pose one actor. The local player fills
/// it from `Player`; a remote fills it from the network snapshot.
#[derive(Clone, Copy, Debug)]
pub struct RigState {
    pub mode: MoveMode,
    pub yaw: f32,            // facing yaw about local up
    pub cam_pitch: f32,       // view pitch (head aim)
    pub throttle: f32,
    pub wingsuit_on: bool,
    pub afterburner: bool,
    pub fwd_in: f32,
    pub strafe: f32,
    pub wing_arch: f32,
    pub wing_turn: f32,
    pub fall_spread: f32,
    pub fall_tuck: f32,
    pub jump_charge: f32,
    pub jump_push_timer: f32,
    pub crouch: bool,
    pub landing_recovery: f32,
    pub leg_steer: Vec3,
    pub intent_dir: Vec3,
    pub rot_err: Vec3,
    pub accel_smooth: Vec3,
    pub ground_normal: Vec3,
    pub foot_ground_hit: [Option<Vec3>; 2],
    pub first_person: bool,
    pub vel: Vec3,            // world-space linear velocity
}
```

`Player` → `RigState` conversion lives in `body.rs` as
`impl From<&Player> for RigState` (and `impl From<&mut Player> for RigState`
for the fields the solver writes back). The **local path keeps computing
exactly the same values**; only the plumbing changes.

### 4.3 `solve_rig` / `Pose`

Extract the body of `animate_body` verbatim into

```rust
fn solve_rig(
    state: &RigState,
    gait: &mut Gait,
    up: Vec3,
    spatial: &SpatialQuery,
    dt: f32,
    t_sec: f32,
) -> Pose;
```

`Pose` carries the joint/frame results the part-writing loop needs. The
solver's maths is **not touched** — that is what keeps the local player
identical (§8.1).

The foot-state write-back (`foot_planted`, `foot_world_pos`, `foot_up_dir`)
moves out of the solver into the caller, which copies it into `Player` for the
local actor only.

### 4.4 `Gait` becomes per-actor

```rust
#[derive(Resource, Default)]
pub struct GaitStore(pub HashMap<Entity, Gait>);
```

Keyed by owner entity. `Gait` gains `#[derive(Default)]` (it already has it)
and stays `Copy`. Entries are removed when an actor despawns (remote leaves).

### 4.5 `BodyAssets` — shared meshes and materials

`spawn_body` currently builds its meshes and materials inline, so calling it
per actor would duplicate ~5 meshes + ~14 materials per player. Split it:

```rust
#[derive(Resource, Clone)]
pub struct BodyAssets {
    pub cyl: Handle<Mesh>, pub ball: Handle<Mesh>, pub cube: Handle<Mesh>,
    pub dome: Handle<Mesh>, pub cone: Handle<Mesh>,
    pub shell: Handle<StandardMaterial>, /* ...13 more... */,
}
```

Built **once** at Startup. Parts clone the handles; only the three wing meshes
are created per actor (§3.2).

### 4.6 `spawn_body_parts`

```rust
pub fn spawn_body_parts(
    commands: &mut Commands,
    assets: &BodyAssets,
    meshes: &mut Assets<Mesh>,      // only for the per-actor wing meshes
    owner: Entity,
    cosmetics: &Cosmetics,
);
```

Spawns 58 parts + 4 backpack + 3 wing entities, each tagged `BodyOwner(owner)`.
Wing meshes are created fresh here.

### 4.7 Remote proxy component

Lives in `net.rs` (networking owns it), holds the replicated state:

```rust
#[derive(Component)]
pub struct RemoteProxy {
    pub id: u64,
    pub state: RigState,      // replicated rig inputs
    pub quat: Quat,            // body orientation, render space
    pub pose_mode: PoseMode,   // Procedural | Replicated
}
```

No `Player`, no `RigidBody`, no `Collider`, no `Focus`.

### 4.8 `PoseMode` — the seam ragdoll needs

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PoseMode { Procedural, Replicated }
```

`Procedural`: the rig solver produces the pose (today's behaviour, and what a
remote always does while alive).
`Replicated`: the pose arrives over the wire and is written straight into the
parts (§6).

---

## 5. What the network must carry

### 5.1 The frame problem (do not skip this)

Each client's render frame is ENU at *its own* origin. A remote 100 km away
sits in a frame whose "north" and "up" differ by up to ~180° in longitude. A
world-space vector sent raw is therefore meaningless to the receiver.

Everything spatial goes over the wire in **ECEF** (the repo's rule: "truth is
f64 ECEF"), and the receiver converts into its own render frame, exactly as
`net.rs:752` already does for position:

```rust
let render = origin.frame.ecef_to_render().transform_point3(ecef);
```

Conversions needed:

| Direction | Helper | Exists? |
|---|---|---|
| ECEF point → render | `EnuFrame::ecef_to_render()` (`ed_geo:183`) | yes |
| render point → ECEF | `EnuFrame::render_to_ecef()` (`ed_geo:175`) | yes |
| ECEF direction → render direction | `EnuFrame::dir_ecef_to_enu()` (`ed_geo:158`) + `enu_to_render()` (`ed_geo:197`) | yes |
| render direction → ECEF direction | `EnuFrame::dir_enu_to_ecef()` (`ed_geo:162`) + `render_to_enu()` (`ed_geo:202`) | yes |
| render quaternion → ECEF quaternion | conjugate by the frame rotation (`render_to_ecef().to_scale_rotation_translation().1`) | **no — add it** |

The quaternion helper is ~15 lines of `DMat3`/quaternion plumbing in
`ed_geo`, which is already engine-free and f64-first. Keep it there rather
than in `ed_game`: it is the same kind of frame math as the rest of `ed_geo`.

### 5.2 Protocol v5 — the animated-state channel

Extend `C2S::State` / `S2C::State`. Per actor per tick:

| Field | Type | Bytes | Why |
|---|---|---|---|
| `pos` | `[f64;3]` ECEF | 24 | existing |
| `orient` | `[f32;4]` quat, ECEF | 16 | **body attitude** — replaces yaw-only fidelity; carries skydive pitch, rocket aim, barrel roll |
| `yaw` | `f32` | 4 | facing yaw about local up (feet, `yaw_only`) |
| `cam_pitch` | `f32` | 4 | head aim |
| `mode` | `u8` | 1 | `MoveMode` (existing `boot`) |
| `throttle` | `f32` | 4 | flame size |
| `flags` | `u8` | 1 | bit0 `wingsuit_on`, bit1 `afterburner`, bit2 `crouch` |
| `wing_arch` | `f32` | 4 | skydive flare |
| `wing_turn` | `f32` | 4 | skydive bank |
| `fall_spread` | `f32` | 4 | freefall spread-eagle |
| `fall_tuck` | `f32` | 4 | freefall somersault |
| `strafe` | `f32` | 4 | rocket arm pose |
| `fwd_in` | `f32` | 4 | rocket arm pose |
| `jump_charge` | `f32` | 4 | squat arm swing |
| `jump_push_timer` | `f32` | 4 | jump-push arm swing |
| `landing_recovery` | `f32` | 4 | slide pose |
| `rot_err` | `[f32;3]` ECEF dir | 12 | spine twist + limb effort |
| `intent_dir` | `[f32;3]` ECEF dir | 12 | rocket leg/spine aim |
| `accel_smooth` | `[f32;3]` ECEF dir | 12 | spine/limb inertia |
| `leg_steer` | `[f32;3]` body-local | 12 | boot steering offset |
| `pose_mode` | `u8` | 1 | `Procedural` / `Replicated` |
| `t` | `f64` | 8 | existing |

**≈150 B/actor/tick.** At the server's 60 Hz near-field rate that is ~9 kB/s
per remote actor — fine on LAN, and the server already rate-limits by
distance. Quantize the small floats (`wing_arch`, `strafe`, `fwd_in`, …) to
`i16` over their natural range later if it matters; do not do it in the first
pass — it complicates the encoder for little gain.

`leg_steer` is body-local by definition (`player.rs:234`, added straight to
the pelvis-local foot target at `body.rs:535`), so it ships as-is. The other
three vectors are world-space and ship in ECEF; the receiver rotates them
into its render frame, and because the rig works in body-local space
(`inv_base * p.rot_err` at `body.rs:471`) the result is identical to the
owner's.

**`StoredState` in `ed_server` grows the same fields** and is re-broadcast
verbatim. Bump `PROTOCOL_VERSION` to 5. The server rejects mismatched
protocols already (`ed_server/main.rs:53`), so an old client is refused
cleanly rather than mis-rendering.

### 5.3 Interpolation

`net_update_avatars` keeps its jitter-adaptive buffer
(`net.rs:722-735`). Extend it:

- position: linear (as today),
- orientation: **slerp** on the quaternion (in ECEF), shortest-arc,
- scalars: linear,
- `mode` / `flags` / `pose_mode`: step at `alpha > 0.5` (as `boot` does today,
  `net.rs:748`),
- velocity: keep the existing derived velocity (`net.rs:736-740`) rather than
  replicating it — the plan's §0.2 rule.

---

## 6. Ragdoll readiness

Ragdoll is the reason the rig is built as many small, owned entities.

### 6.1 Why the remote must *follow*, not re-simulate

A ragdoll's pose is the result of impulses against a solver. Avian is not
deterministic across machines (`docs/plans/multiplayer.md:42`), so a receiver
re-simulating the ragdoll produces a *different* flop. The only way both ends
show the same ragdoll is for the owner to be authoritative for it and the
receiver to **replicate the pose**.

This inverts the animated-state rule from §1 — deliberately, and only for
ragdoll.

### 6.2 The pose channel

New message type, e.g. `C2S::Pose { id, seq, parts: Vec<PosePart> }` and
`S2C::Pose`. `PosePart` carries one part's transform, expressed **relative to
the owner's root**: position offset in the owner's local frame, orientation
quaternion relative to the owner's orientation. Root-relative is both cheap and
frame-independent (the root orientation is itself replicated in ECEF).

```rust
pub struct PosePart {
    pub index: u16,        // index into the canonical part list
    pub pos: [f32;3],      // owner-local metres
    pub rot: [f32;4],      // owner-local quaternion
}
```

Both ends need a **stable part index**. `Spec` (`body.rs:81`) is already a
flat, deterministic list; assign each spawned part a `PartIndex(u16)` in
spawn order and keep that order frozen (a unit test asserts the count and the
`Spec` → index mapping is stable).

Bandwidth: 58 parts × 28 B ≈ 1.6 kB per pose packet. At 20 Hz that is
~33 kB/s **per ragdolling actor** — only while someone is actually ragdolling,
and only for actors in view. Quantize (i16 position, 8-bit quaternion) and
delta-encode against the previous packet to bring it down ~4×.

### 6.3 Switching modes

- Owner: `pose_mode = Procedural` while alive; on ragdoll, `pose_mode =
  Replicated` and the owner starts sending pose packets.
- Receiver: on the first `Replicated` packet for an actor, it stops running
  `solve_rig` for that actor and writes incoming part transforms directly.
  On `Procedural` again, it resumes.
- The pose packets and the animated-state packets are independent; a
  ragdolling actor's `RigState` stops mattering.

### 6.4 What this plan must leave behind *now*

1. **Parts are per-owner, individually addressable entities** (§4.1, §4.6) —
   so a pose can address them.
2. **A stable part index** (§6.2).
3. **A pose-application path separate from the solver** — i.e. `solve_rig`
   produces a `Pose` and a separate step applies a `Pose` to parts. Today the
   solver writes transforms directly into the parts; that coupling has to be
   broken for §6.2 to work.
4. **`PoseMode` in the snapshot** so the receiver knows which path to take.
5. **Part entities must not be children of the actor** (§3.4) if the pose is
   to address them independently — a parented part inherits the parent's
   transform on top of the pose we write.

Point 3 is the only one that costs anything today, and it is the reason to do
the `solve_rig` → `Pose` split now rather than later.

### 6.5 Not in this plan

The ragdoll *rig* (which parts become rigid bodies, the joint graph, muscle
torques, how the ragdoll is triggered) is a separate plan. This plan only
guarantees the architecture it needs.

---

## 7. Implementation phases

Each phase is independently shippable and must keep the local player
pixel-identical (§8.1).

### Phase 1 — rig ownership (no networking)

1. `body.rs`: add `BodyOwner`, `BodyAssets`, `GaitStore`.
2. `body.rs`: split `spawn_body` into `init_body_assets` (once) +
   `spawn_body_parts` (per owner).
3. `body.rs`: extract `solve_rig(state, gait, ...) -> Pose`; keep the maths
   byte-identical; move the foot write-back to the caller.
4. `body.rs`: `animate_body` becomes a loop over actors: the local player
   (`Player` → `RigState`) and every proxy with `RigState`.
5. `player.rs`: `spawn_player` spawns the local rig via `spawn_body_parts`
   with the player's own entity as owner.
6. `lookdev.rs`: the dummy `Player` gets its own rig.
7. `body.rs`: the `BootNozzles` write and the flame-mode choice become
   per-actor (local → particle plume; everyone else → cone).

Nothing in `net.rs` changes yet. The remote still uses the old hand-written
rig. **This phase is where the local player is at risk** — it is also the
phase with the strongest verification story (§8).

### Phase 2 — remote proxies use the shared rig

8. `net.rs`: delete `RJ`, `RF`, `RSpec`, `RPart`, `RWing`, `RemotePhase`, and
   the pose half of `net_update_avatars`.
9. `net.rs`: `net_spawn_avatars` spawns a `RemoteProxy` root (Transform,
   `Rebased`, `RemoteAvatar`, `RemoteProxy`) **and** a full owned rig via
   `spawn_body_parts`. Despawn both on leave (§3.4 — remote parts are
   top-level, so they are not reaped automatically; query `BodyOwner` and
   despawn).
10. `net.rs`: `net_update_avatars` fills `RemoteProxy.state` from the
    snapshot and sets the root transform; `body.rs` poses it.
11. `net.rs`: `net_update_nametags` and the minimap/globe dots keep using
    `RemotePlayers` / `RemoteAvatar` — unchanged.

### Phase 3 — protocol v5

12. `ed_net`: extend `C2S::State`/`S2C::State` per §5.2, bump
    `PROTOCOL_VERSION`.
13. `ed_server`: extend `StoredState` and the broadcast.
14. `net.rs`: encode the new fields; interpolate per §5.3; convert ECEF ↔
    render per §5.1.
15. `ed_geo`: add the render-quaternion ↔ ECEF-quaternion helper (§5.1).

### Phase 4 — pose channel (ragdoll plumbing)

16. `ed_net`/`ed_server`: `Pose` message type.
17. `body.rs`: `PartIndex`, stable part list, `Pose` → parts application
    path, `PoseMode` handling.

### Phase 5 — polish

18. Remote cosmetics (backpack) replication, if wanted.
19. Bandwidth: quantize the animated-state floats; delta-encode poses.
20. Distance-based rig LOD: stop solving rigs beyond the render distance and
    show a cheap marker instead (the solver is cheap, but the *parts* are 65
    entities per actor).

---

## 8. Verification

### 8.1 The local player must not change (the acceptance gate)

The rig is 58 parts × N frames of IK. Any accidental change is invisible in
code review and obvious on screen. Three gates, all mechanical:

1. **Lookdev character shots, byte-compared.**
   ```
   cargo run --release -- --lookdev /tmp/lb_before
   # ...change...
   cargo run --release -- --lookdev /tmp/lb_after
   cmp /tmp/lb_before/character_front.png /tmp/lb_after/character_front.png
   cmp /tmp/lb_before/character_back.png  /tmp/lb_after/character_back.png
   ```
   These shots frame the rig directly (`lookdev.rs:239-240`).
2. **Part-transform dump, numerically compared.** Add
   `EW_BODY_DUMP=1`: after `animate_body`, write every local part's
   translation/rotation/scale to a file. Before/after must match to the bit
   for the same scripted input. This catches changes a screenshot hides.
3. **In-game screenshot, same spot, same frame count.**
   ```
   cargo run --release -- --lat 43.53 --lon 5.45 --alt 3000 \
     --screenshot /tmp/ig.png --frames 60
   ```

Gate 2 is the one that actually proves "unchanged"; 1 and 3 are the
human-visible confirmation.

### 8.2 Remote parity

- Two clients on localhost, one walks/runs/jumps/flies; the observer compares
  the remote's rig against the observer's own rig doing the same input.
- Every `MoveMode` covered: walk, run, jump, fall, skydive, parachute, rocket
  shoes, wingsuit glide.
- The remote must show the *same* silhouette, not a similar one: same limb
  angles at the same phase.

### 8.3 Repo gates

- `cargo test --workspace` stays fast and offline.
- `cargo check --offline -p ed_game` clean.
- New unit tests: `BodyOwner` filtering, per-actor `Gait` isolation, part
  index stability, `RigState` ↔ `Player` round-trip.

### 8.4 Branch rollout

`main` and `windows` have diverged (§3.7). Land Phase 1 on `main`, verify, then
carry the work across:

```
git checkout main      # land + verify phases here
git checkout windows
git merge main         # resolve net.rs join_input by keeping windows' arboard
```

Resolve any `net.rs` conflict by keeping the `windows` clipboard code and
taking the `main` rig changes. Keep `docs/CHANGES.md` entries on both
branches, matching each branch's existing style.

---

## 9. Risks

| Risk | Mitigation |
|---|---|
| Local player changes during the rig split | §8.1 gates 1-3; `solve_rig` maths copied verbatim; `RigState` conversion is exhaustive and unit-tested |
| A `Player` sneaks onto a remote and breaks a `.single()` site | `RemoteProxy` is a distinct type; add a test that asserts no entity carries both `Player` and `RemoteProxy` |
| Two rigs share wing meshes and flicker | Per-actor wing meshes, created in `spawn_body_parts` (§3.2) |
| Remote's boot plume hijacks the local's | `BootNozzles` written by the local actor only (§3.3) |
| Remote shows no rocket flame | Per-actor flame mode: `Cone` for remotes (§3.3) |
| Remote inherits the local's backpack | Per-actor cosmetics (§3.6) |
| Remote rig parts leak when a player leaves | Explicit `BodyOwner` sweep on leave (§3.4) |
| Frame mismatch makes replicated vectors nonsense | Everything spatial in ECEF (§5.1) |
| Bandwidth grows | ~150 B/tick now; quantize + delta-encode later (§5.2, §6.2) |
| Ragdoll needs a different rig shape | §6.4 — parts are already per-owner entities with stable indices |
| Ground probes cost per actor | `ground_at` already raycasts; remote actors far away have no loaded collider, so feet float — acceptable, and it avoids N× raycasts |
| 65 extra entities per remote actor | §8.4 phase 5 LOD |

---

## 10. Open questions → decisions

| Question | Decision |
|---|---|
| Reuse `Player` for remotes? | **No.** 15 `.single()` sites break silently (§3.1). Use `RemoteProxy`. |
| Share wing meshes? | **No** — vertices are rewritten per actor in world space (§3.2). |
| Where do remotes' body parts live? | Top-level, absolute world transforms, like the local rig (§3.4). |
| Interpolation: hard linear or exponential? | Keep hard linear + jitter buffer (§5.3); revisit if jitter is visible. |
| Render delay 100 ms or 50 ms? | Keep the current jitter-adaptive 16-100 ms. |
| Barrel roll needs `roll` in the protocol? | Not as a separate field — ship the **orientation quaternion** (§5.2), which carries roll and everything else. |
| Which state does the remote need? | The solver's full input set (§4.2), not just mode/throttle. |
| Replicate the rig, or the skeleton? | Replicate rig *inputs* and re-run the solver. Exception: ragdoll (§6). |
| Ragdoll: re-simulate or replicate? | Replicate the pose; Avian is not deterministic (§6.1). |
| Does the remote need Avian physics? | No — kinematic, no collider, no `RigidBody`. |
| Protocol version | Bump to 5. The server already refuses mismatched clients. |
| Where does `Cosmetics` live? | Per actor; local keeps the global resource, remotes get a default until replicated. |

---

## 11. References

- Gambetta, *Entity Interpolation* — render-delay interpolation for proxies.
- SnapNet docs — derive velocity, do not predict remote inputs.
- Photon Fusion 2, *Animation Synchronization* — pose at the interpolated time.
- `docs/plans/multiplayer.md` — ghost→Stage C; the non-determinism note.
- `docs/ROADMAP.md` Stage C — authoritative server, interest management.
