# Phase 5 — Simulation and vehicles

Status: follows Phase 3; may overlap Phase 4. Roadmap row: `../ROADMAP.md`. Contracts:
`../contracts/sim.md`, `../contracts/persistence.md`. Milestones M12 and M13 in
`../plans/path-to-solo.md`. Decisions in the P5-D series are **proposed**.

## 1. Goal

Things you control that feel right, and things that stay where you leave them: a character
with rocket boots, a car, a plane and a boat on custom models over Box3D (D4), light
traffic, and persistence of parked vehicles through `ed_store`.

## 2. What Phase 5 proves

| Question | Proof |
|---|---|
| Can custom vehicle models on Box3D casts feel good? | Owner sign-off of the car on a forest road and a city street |
| Do the detail tiers of `sim.md` §5 hold? | Contract tests S6 and S7; a parked car costs no simulation |
| Is persistence one schema? | Contract tests D1 to D9 that apply to solo; park, go to orbit, return, quit, restart |
| Does the sim stay inside its step budget with vehicles and traffic? | S8 on the reference scene with 20 vehicles |

## 3. What Phase 5 does not do

No rocket vehicle or Moon landing (Phase 6); "go to orbit" in the gate uses rocket boots or
the editor camera with the simulation running. No missions (Phase 6). No networking
(Phase 7). No pedestrians or NPC drivers with schedules (Phase 8); traffic here is anonymous.

## 4. Decisions in detail (proposed)

### P5-D1 — Vehicle models live in `ed_sim`
A `vehicle` module with one trait and four models (car, plane, boat, rocket in Phase 6).
Each model reads only `SimInput` commands, body state, cast results and the `Environment`
queries of `sim.md` §6. No Box3D vehicle or joint-based wheel is used for player vehicles.

### P5-D2 — Car model
Per wheel: a shape cast for suspension with spring, damper and anti-roll; a tyre model with
combined longitudinal and lateral slip and a load-sensitive friction curve; grip scaled by
`ground_material`. Drivetrain: torque curve, clutch, gearbox, differential, brakes, taken
from the per-car data already used by `ed_engine_audio`. Four sub-steps per tick (contract).
Assists (traction, stability, steering speed sensitivity) are data and can be turned off.

### P5-D3 — Tuning workflow
Vehicle parameters are text files with hot reload. A telemetry overlay shows per-wheel
load, slip and force, speed, engine state. Input can be recorded and replayed against a
changed parameter file, so two tunings are compared on identical inputs. The owner's
sign-off sessions are recorded as replays and kept as regression inputs.

### P5-D4 — Character
The Box3D mover from Phase 1 with step height, slope limit and ground snapping (S9);
rocket boots, skydive, parachute and wingsuit on `ed_flight`; a third-person camera with
collision. Animation through `ed_anim`: locomotion blend, foot placement from the height
query. The owner's rig has no neck or head bones (P0-R7); retargeting must not assume them.

### P5-D5 — Plane and boat
Plane: aerodynamic surfaces and atmosphere from `ed_flight`, landing gear on the car's
suspension code. Boat: buoyancy at sample points against the water field of P4-D7, hull
drag, propeller thrust; test "a boat floats the same in two processes" (`water.md` §6).

### P5-D6 — Detail tiers and promotion
Exactly the four tiers of `sim.md` §5. Promotion and demotion at tick boundaries; a parked
record promoted and demoted without interaction returns the identical record (S7). Radii
are data.

### P5-D7 — Persistence
`ed_store` on SQLite with the contract's schema, extending the delta use of P3-D10 with
`parked`, `plot` and `progression`. The sim hands changes to a writer queue; a consistent
batch is written every 2 s and at shutdown (contract rule 5). Solo runs the server's
persistence code in process (rule 6).

### P5-D8 — Traffic
Vehicles in the kinematic tier follow the Phase 3 lane graph with spacing and junction
priority rules; statistical counts per node beyond that. Density is low and set by road
class and hour. Deterministic from (seed, node, tick).

### P5-D9 — Input and cameras
One action map for keyboard and mouse and for controllers, rebindable, with per-vehicle
contexts. Cameras: chase, cockpit, free; each is data.

### P5-D10 — Audio
`ed_audio` and `ed_engine_audio` are driven by vehicle state (engine speed, load, slip,
impacts) through a small event interface; audio never feeds back into the sim.

### P5-D11 — Editor stage E4, first half
Markers, vehicle spawners and nav-graph visualisation (`../plans/editor.md`). Mission
graphs are Phase 6.

## 5. Budgets

| Item | Budget | Basis |
|---|---|---|
| Sim step, reference scene (height field, 1,000 boxes, 20 vehicles) | 2 ms, no allocation after warm-up | Contract S8 |
| Sim step, worst case in a city with traffic | 4 ms | Quarter of a frame |
| One player car | 0.05 ms | 20 vehicles inside S8 (inferred) |
| Store batch write | off the sim thread; never blocks a tick | Contract §2 |
| Load one node's deltas and parked rows | 5 ms | Contract D4 |
| Parked position after restart | within 1 mm | Contract D2 |
| Traffic vehicles in the kinematic tier | 200 | Set against the 4 ms step; adjusted by measurement |

## 6. Workstreams

| # | Scope | Owns | Done when |
|---|---|---|---|
| W1 | Car model, data files, telemetry, input replay | `ed_sim/src/vehicle/car*`, `data/vehicles/` | Drivable on the three Phase 3 routes; replay A/B works |
| W2 | Character: mover, boots, flight modes, camera, animation | `ed_sim/src/character*`, `ed_anim`, `ew` | S9; walk test through a city without passing through geometry |
| W3 | Plane | `ed_sim/src/vehicle/plane*` | Take-off, circuit and landing by script |
| W4 | Boat and buoyancy | `ed_sim/src/vehicle/boat*` | Two-process float test |
| W5 | Detail tiers, promotion, collision streaming under vehicles | `ed_sim/src/tiers*` | S6, S7 |
| W6 | `ed_store`: parked, plots, progression, writer queue, crash safety | `ed_store` | D1 (SQLite cases), D2, D3, D4, D6, D7, D9 |
| W7 | Traffic | `ed_sim/src/traffic*` | 200 vehicles follow lanes with no collisions over a 10-minute run |
| W8 | Input, cameras, audio hookup | `ew`, `ed_audio` | Controller and keyboard both complete the routes |
| W9 | Editor E4 first half | `ed_editor` | Spawner placed in the editor produces a car in play |
| W10 | Feel sessions | data only | Owner sign-off recorded with its replay files |

## 7. Order

```
W1 ──> W10 (repeats)          W5 ──> W6 ──> gate (park, orbit, return, restart)
W2, W3, W4 in parallel        W7 after W1 and W5
W8 with W1                    W9 after W5
```

M12 = W1 to W4, W7, W8, W10. M13 = W5, W6, W9 and the exit gate.

## 8. Exit gate

- [ ] Car feel signed off on a forest road and a city street.
- [ ] Park a car, go to orbit, return, and it is there; quit and restart with the same
      result.
- [ ] Contract tests S1, S2, S5 to S9 and the solo cases of D1 to D9 green.
- [ ] Step budget held with 20 vehicles and with city traffic.
- [ ] Plane and boat pass their scripted tests.
- [ ] `CHANGES.md` has the sim and store numbers.

## 9. Failure branches

| If | Then |
|---|---|
| Car feel takes many passes | Expected; a drivable car exists from M5, and W10 repeats until sign-off |
| Cast suspension is unstable at speed over node edges | Cast against the height query directly for terrain, shapes only for objects |
| Box3D destabilises with vehicles | Keep vehicles as our own integrated bodies colliding through casts and contact callbacks; file the diagnosis |
| Batch writes stall on large saves | Split by node; compaction offline (contract rule 1) |
| Traffic exceeds the step budget | Lower kinematic radius; more statistical |

## 10. Needed from outside the code

| Need | From |
|---|---|
| Car, plane and boat models with interiors visible from the seat | Art |
| A locomotion animation set on the standard skeleton | Art |
| Feel sessions and sign-off | Owner |
| Confirmation of F10 | Owner |

## 11. What Phase 6 receives

Vehicles on one model interface, a store that already holds parked records, plots and
progression, tiers that let a vehicle be left anywhere, and editor markers and spawners.
