# Phase 7 — Multiplayer and persistence

Status: starts only after the Phase 6 gate passes (roadmap rule). Roadmap row:
`../ROADMAP.md`. Design: `../ARCHITECTURE.md` §10. Contracts: `../contracts/sim.md`,
`../contracts/persistence.md`. Decisions in the P7-D series are **proposed**. F11 (QUIC)
and F10 (determinism not gating) are still flagged for owner confirmation; this plan
assumes both.

## 1. Goal

Other players in the loop that already works: a server-authoritative shared world on region
shards, where two people drive one car and do one mission, a launch is seen from far away,
and eight players share a hub on the owner's first server (D6).

## 2. What Phase 7 proves

| Question | Proof |
|---|---|
| Does one sim and one schema serve solo and shared play? | The server binary runs the Phase 6 game code headless; a solo save opens as a server database (D8) |
| Is predicted movement acceptable under real latency? | Mover and car driven at 80 ms round trip and 2% loss with corrections under the budget |
| Can the world be shared without sending it? | No geometry in any packet capture; clients build the world from hashes, layers and deltas |
| Do shards hand over cleanly? | A car crosses a shard boundary with two occupants and no visible discontinuity |
| Does it fit the first server? | Eight players in a hub inside the tick and bandwidth budgets on the Ubuntu gaming PC |

## 3. What Phase 7 does not do

No NPC life (Phase 8). No public launch, accounts at scale, payments or matchmaking. No
anti-cheat beyond server authority and validation. No PostgreSQL unless shards are split
into processes within this phase (contract §1).

## 4. Decisions in detail (proposed)

### P7-D1 — The alpha protocol is replaced
`ed_net` at protocol version 5 (hand-written pose messages over a socket) and the single-file
`ed_server` relay were a ghost test. Their message set is not extended. What carries over:
distance-based send rates and the lesson that clocks must come from the server tick.

### P7-D2 — Transport
`quinn` (F11). One connection per client: a reliable stream for session control, one for
world state (delta rows, parked and plot rows, layer hashes), unreliable datagrams for
entity snapshots and inputs. Chunks are fetched from `ew-tiles` over HTTP, not through the
game connection.

### P7-D3 — Authority and tick
The server runs `ed_sim` at 60 Hz and is the only authority. Clients send input commands
stamped with the tick they apply to; the server applies them in `(entity, sequence)` order
(`sim.md` §2). Snapshots go out at a rate that falls with distance and rises with speed,
from 30 Hz near to 2 Hz far.

### P7-D4 — Prediction and reconciliation
The client runs the same `ed_sim` for the entities it owns (its mover, the vehicle it
drives), keeps its unacknowledged inputs, and on each server snapshot restores the
acknowledged state and replays them (snapshot and restore are contract features). Small
errors are blended over a few frames in the render state only. Other entities are drawn
from an interpolation buffer. Cross-machine float equality is not assumed (F10); the server
state always wins.

### P7-D5 — Shared vehicles
A vehicle has one driver whose inputs the server simulates; passengers are attached and
predict nothing but their own camera. Seats, entering and leaving are commands validated by
the server. The driver's client predicts the vehicle.

### P7-D6 — Interest by node
A client's interest set is a set of L12 nodes sized by its speed and altitude: small on
foot, large in a jet, coarse in orbit. Entity snapshots, delta rows and parked rows are
sent only for nodes in the set (contract §5).

### P7-D7 — Directory and shards
A directory service maps `NodeId` sets to shards and accounts to their current shard.
Shards start as regions inside one server process and split into processes when load
requires (ARCHITECTURE §10). Space is a shard of its own for vessels on rails.

### P7-D8 — Handoff
An entity crossing a shard boundary is transferred at a tick boundary by a transaction that
moves its ownership record; for a moment it is a parked-form record owned by exactly one
side (contract rule 2, test D5). Clients connect to the destination shard before the
crossing and switch streams without a pause.

### P7-D9 — Sky events
Launches and re-entries are rows in `sky_event` with trajectory elements, replicated to all
shards at low rate, so any client can draw a launch on the horizon from the elements alone.

### P7-D10 — Accounts
A display name and an opaque auth subject (contract rule 7). The first server uses invite
tokens issued by the owner; the identity provider for a public build is an owner decision
deferred to Phase 9.

### P7-D11 — Trust boundary
Clients are untrusted: every command is range-, rate- and ownership-checked; a client's
proposed delta is validated before `append_delta`; speed and teleport limits per vehicle
kind are enforced from the vehicle data files.

### P7-D12 — Editor stage E5
Connect the editor to a development server: see players and shard boundaries, inspect and
revert live deltas.

## 5. Budgets

| Item | Budget | Basis |
|---|---|---|
| Server tick, eight players in a hub with vehicles | 8 ms on one core | Half the tick period |
| Downstream per player in that hub | 8 KB/s average, 24 KB/s peak | Seven remote players and vehicles at 30 Hz with quantised deltas (inferred; fixed by the first capture) |
| Upstream per player | 2 KB/s | Inputs at 60 Hz, redundantly packed |
| Visible correction of an owned entity at 80 ms round trip, 2% loss | under 10 cm, blended | Feel |
| Remote entity delay | 100 ms interpolation buffer | Two snapshots at 30 Hz plus jitter |
| Join to playable | 15 s plus chunk fetch | Phase 3 complete-view budget |
| Shard handoff | no frame over 33 ms on any client | Flight budget |
| Soak | 8 scripted clients for 12 hours of simulated play with no memory growth and no desync count | First-server confidence |

## 6. Workstreams

| # | Scope | Owns | Done when |
|---|---|---|---|
| W1 | Protocol: messages, quantisation, versioning, capture tooling | `ed_net` (rewritten) | Round-trip and fuzz tests; a packet capture decoder |
| W2 | Headless server on the game code, tick loop, command intake | `ed_server` | Runs the Phase 6 loop harness with a scripted client |
| W3 | Prediction, reconciliation, interpolation | `ew` client netcode, `ed_sim` snapshot use | Latency and loss test inside budget |
| W4 | Interest management and state streams | `ed_server`, `ed_store` | No row leaves its interest set in a capture |
| W5 | Shared vehicles and co-operative mission state | `ed_sim`, mission module | Two clients complete one mission in one car |
| W6 | Directory, shards, handoff | `ed_server` | D5; boundary crossing test |
| W7 | Sky events | `ed_server`, `ed_space`, `ed_render` | A launch seen from another region |
| W8 | Accounts, tokens, validation | `ed_server`, `ed_store` | Forged commands are rejected in tests |
| W9 | Network simulator and soak harness | `tools/`, `ew --bot` | The soak budget row |
| W10 | Deployment to the first server | `tools/setup_server.sh`, service units | Eight real clients in a hub |
| W11 | Editor E5 | `ed_editor` | Revert a live delta from the editor |

## 7. Order and milestones

```
W1 ──> W2 ──> W3 ──> W5 ──┐
              W4 ─────────┼──> W6 ──> W7 ──> W10 ──> gate
W8, W9 alongside ─────────┘        W11 after W4
```

**M16. Two in one place.** W1 to W4. *You can:* see a friend walk and drive. *Gate:*
latency test inside budget; no geometry on the wire.
**M17. Together.** W5, W8. *You can:* share a car and finish a mission together. *Gate:*
the co-operative mission by two scripted clients.
**M18. A world of regions.** W6, W7, W9 to W11. *Gate:* the Phase 7 exit gate.

## 8. Exit gate

- [ ] Two clients share a car and a mission.
- [ ] A launch is seen from another region.
- [ ] Eight players in a hub on the first server, inside the tick and bandwidth budgets.
- [ ] Persistence conformance D1 to D9 green, including the server cases.
- [ ] Soak run clean.
- [ ] `CHANGES.md` has the network and server numbers.

## 9. Failure branches

| If | Then |
|---|---|
| Car prediction diverges often between machines | Predict with a simplified model and correct harder; the server stays authoritative (F10) |
| QUIC datagram loss or size limits hurt | Fragment snapshots by priority; keep `quinn`; record the limits |
| Bandwidth over budget | Lower far snapshot rates first, then quantisation |
| The first server cannot hold eight | Profile the tick; lower sim radius per player before buying hardware |
| Handoff is visible | Widen the overlap band so both shards simulate the entity as kinematic during transfer |
| The owner rejects F11 | The transport sits behind `ed_net`'s interface; replace it there |

## 10. Needed from outside the code

| Need | From |
|---|---|
| Confirmation of F10 and F11 | Owner |
| The Ubuntu server, a reachable address and a port | Owner |
| Seven more people for the hub test | Owner |

## 11. What Phase 8 receives

A shared world with server-side deltas, interest sets and shards, in which NPCs, traffic
with schedules, destruction and weather are new kinds of rows and entities, not new systems.
