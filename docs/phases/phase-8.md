# Phase 8 — Living world

Status: follows Phase 7. Roadmap row: `../ROADMAP.md`. Gameplay source:
`../plans/solastra-core-loop.md`. Decisions in the P8-D series are **proposed**.

## 1. Goal

A world that is inhabited and changes: people with homes, work and schedules, buildings you
can walk into, things that break and are rebuilt, weather that matters, and enough mission
variety that the core loop sustains itself.

## 2. What Phase 8 proves

| Question | Proof |
|---|---|
| Can a planet's population be simulated on one gaming PC? | The tier pyramid: statistical counts everywhere, entities only near players, inside the server tick budget |
| Can interiors exist without door-teleports (D12)? | Walk from a street into a building and up a stair in one continuous view, at frame budget |
| Is destruction just deltas? | A destroyed house is rows in `delta`; a rebuild mission restores it; both survive restart and replicate |
| Does the loop run without authored missions? | Needs are generated from world state (weather damage, NPC shortages) and complete as missions |

## 3. What Phase 8 does not do

No generative-AI NPC behaviour or dialogue (D6). No full fluid, fire or structural
simulation. No new platforms (Phase 9).

## 4. Decisions in detail (proposed)

### P8-D1 — Population pyramid
The four tiers of `sim.md` §5. Statistical: counts per node from open population grids and
land use, by hour. Kinematic: schedule-followers on paths within mid range. Full: movers
near players. An NPC that a player has interacted with gets an `npc_assignment` row and
keeps its identity; all others are regenerated from (seed, node, hour) and need no storage.

### P8-D2 — Schedules are data
Home, work and leisure places are chosen deterministically from parcels and building
classes; a schedule is a table of (time span, activity, place class). Behaviours are small
state machines authored as data. No learning, no language model.

### P8-D3 — Dialogue
Authored lines keyed by activity, community, trust level and mission state. Greeting by
name follows trust (`solastra-core-loop.md` §4).

### P8-D4 — Interiors
A new streaming tier keyed by building `ObjectId`: procedural floor plans from the
per-storey hook of the generation contract §6, and authored interiors for hubs through
prefabs. Interior chunks load on approach and are culled by portals at doors and windows.
The shell's openings and the interior's openings come from the same data, so they line up.

### P8-D5 — Destruction and rebuilding
Damage states are `replace` operations on an `ObjectId`; removal is `suppress`; both are
deltas, so every rung reflects them (P3-D9). Hub cities reject such deltas server-side.
A rebuild is a mission (category C) whose completion appends the inverse deltas. Debris is
short-lived local simulation, never persisted.

### P8-D6 — Weather
A deterministic climate and storm model, a pure function of (seed, UTC, position), in its
own engine-free crate. It modulates clouds, wind, water state, wetness and visibility. It
never decides what grows (climate normals and land cover do). Storms generate needs.

### P8-D7 — Wet and cold surfaces
Rain, snow accumulation and wet roads as material modifiers driven by the weather sample,
with tyre grip following through `ground_material`. Each has a tier cost and an off switch.

### P8-D8 — Need generation
Needs come from world state: storm damage, shortages in a community's statistical needs,
stranded travellers. Category D (social) missions use NPC groups and schedules. The ping
filter (range, urgency, the player's history) is data.

### P8-D9 — Trust rules completed
Loss, decay and gifts as the owner decides (`solastra-core-loop.md` §5); the storage has
been per community since Phase 6.

### P8-D10 — Wildlife and ambient life
Animals by biome on the same agent tiers; ambient launches as sky events.

### P8-D11 — Water completion
Storm seas, caustics and wake polish per `../plans/water.md` Phase 8 row.

## 5. Budgets

| Item | Budget | Basis |
|---|---|---|
| Server tick with eight players in a hub and NPCs | 12 ms | Leaves margin in a 16.6 ms tick |
| Full-tier NPCs near one player | 60 | Set by the mover's measured cost in Phase 5; adjusted then |
| Kinematic NPCs and vehicles near one player | 600 | Same |
| Client frame in a populated hub | 60 fps on High | Roadmap standing rule |
| Interior chunk load on approach | resident before the door is reached at running speed | No door-teleports |
| Statistical update per node | amortised; under 1 ms per tick in total | Server budget |
| Stored rows per interacted NPC | one | Contract table |

## 6. Workstreams

| # | Scope | Owns | Done when |
|---|---|---|---|
| W1 | Population model, schedules, tier promotion | `ed_sim/src/agents*`, `data/schedules/` | A city block shows day and night patterns by script |
| W2 | NPC movers, animation, dialogue | `ed_anim`, `ew`, `data/dialogue/` | Sixty NPCs near the player inside budget |
| W3 | Interiors: floor plans, portals, streaming tier | `ed_world/src/interiors*`, `ed_render`, `ed_planet` | Street-to-stair walk test |
| W4 | Destruction, rebuild missions | `ed_sim`, mission module, `ed_store` | Destroy, restart, rebuild, restart |
| W5 | Climate and storms | new `ed_climate` crate | Same storm on two machines for the same UTC |
| W6 | Weather rendering and surface state | `ed_render` | Calm and storm boards |
| W7 | Need generation and ping filter | mission module | An hour of play yields completable needs with no authored mission |
| W8 | Trust rules | `ed_store`, mission outcomes | Owner's rules as tests |
| W9 | Wildlife, ambient events | `ed_sim/src/agents*` | Birds in a city, a launch on the horizon |
| W10 | Water completion | `ed_render/src/water*` | `water.md` Phase 8 gate |
| W11 | Polish pass on the reference places | all | Owner review list closed |

## 7. Order and milestones

```
W1 ──> W2 ──> W7 ──> W8 ──┐
W3 ───────────────────────┼──> W11 ──> gate
W5 ──> W6 ──> W4 ─────────┤
W9, W10 ──────────────────┘
```

**M19. People.** W1, W2. *Gate:* population budget held in Shibuya.
**M20. Inside.** W3. *Gate:* street-to-stair walk.
**M21. Weather and damage.** W4 to W6, W10. *Gate:* storm, damage, rebuild, replicated.
**M22. A world that asks.** W7 to W9, W11. *Gate:* the Phase 8 exit gate.

## 8. Exit gate

- [ ] The world feels populated on the gaming-PC server (owner sign-off after a shared
      session in a hub and in the countryside).
- [ ] Population, interior and server budgets held during that session.
- [ ] Destruction and rebuilding persist and replicate.
- [ ] Generated needs sustain an hour of play.
- [ ] `CHANGES.md` has the numbers.

## 9. Failure branches

| If | Then |
|---|---|
| NPC cost exceeds the tick | Lower full-tier radius; more kinematic |
| Procedural floor plans are implausible | Authored interior templates per building class, chosen by footprint |
| Interiors blow the memory budget | One storey resident around the player; others as shells |
| Storm model is costly | Precompute per-UTC-hour fields on the server and send parameters |
| Pings feel like spam | Tighten the filter; needs still exist unpinged |

## 10. Needed from outside the code

| Need | From |
|---|---|
| Realistic NPC human assets (open owner question in `DECISIONS.md`) | Owner and art |
| Population grids and agriculture data, registered | Acquisition (`data/README.md` §7) |
| Answers to the trust and griefing questions | Owner |
| Dialogue writing | Owner or a writer |

## 11. What Phase 9 receives

The complete game on macOS and the first server, with every feature behind a tier flag.
