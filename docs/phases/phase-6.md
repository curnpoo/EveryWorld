# Phase 6 — Golden path, solo

Status: follows Phases 4 and 5. Roadmap row: `../ROADMAP.md`. Target and scope:
`../plans/path-to-solo.md` §1; gameplay: `../plans/solastra-core-loop.md`. Milestones M14
and M15. Decisions in the P6-D series are **proposed**.

## 1. Goal

The north-star loop of `../VISION.md` §2, played solo from start to finish in one sitting:
home, drive to a city, a mission, the spaceport, the Moon, a mission there, back to the
parked car, home, quit, restart, continue.

## 2. What Phase 6 proves

| Question | Proof |
|---|---|
| Is the journey continuous from a driveway to the Moon? | One recording with no loading screen and no representation switch (D2) |
| Can missions be authored as data in the editor? | The five solo missions exist only as files made in editor stage E4 |
| Does progress without money work as a loop? | Trust earned opens the two gates of path-to-solo §1 |
| Does everything persist? | Quit at any step of the loop; restart continues from the saved state |

## 3. What Phase 6 does not do

No other players (Phase 7). No living NPCs, destruction or social missions (Phase 8). No
free settlement; one home plot. Mars and the fourth world are not required.

## 4. Decisions in detail (proposed)

### P6-D1 — Two flight regimes, one view
Inside physics range a rocket is a vehicle model in `ed_sim` (P5-D1) with thrust, mass
flow, staging and atmosphere from `ed_flight`. Outside it, motion is `ed_space`'s: the
vessel follows conic sections under one body at a time, switching at the sphere of
influence (`sim.md` §3). The hand-over in both directions happens at a tick boundary and
preserves position and velocity; the picture is the same planet renderer throughout.

### P6-D2 — Time acceleration
Allowed only on rails (no live physics bodies near the player), at fixed multiples, with
the view continuous. The Moon transfer's length is a design parameter tuned with the owner.
Simulated UTC advances with it, so sun, Moon phase and clouds stay correct.

### P6-D3 — Guidance aids
A navigation display with the predicted path, the target's position at arrival, burn
markers and a landing-site marker; an optional hold-attitude assist. The round trip must be
flyable by hand by someone who has not studied orbital mechanics.

### P6-D4 — Boarding and carried vehicles
A vehicle driven into a rocket's bay becomes cargo: its parked record gains the carrier's
`ObjectId` as parent and leaves the node index until unloaded. A vehicle left at the
spaceport is an ordinary parked record. Exclusivity is the contract's rule 2.

### P6-D5 — Missions are data
A mission is a text file bound to markers: steps with conditions (arrive at marker, carry
item, interact, within time, with vehicle kind), outcomes (trust change, unlock, spawn) and
text. No scripting language unless three of the five hand-written missions cannot be
expressed (`editor.md` open question 1); if so that finding is recorded and the language
chosen then. State is the `mission_state` table.

### P6-D6 — Messages: the needs log
Owner, 2026-11-10: requests arrive as messages in a log on the device that the player opens
when they choose. Only a message marked urgent raises a notification. Requests come from
people within about 10 to 20 km, or from walking up to someone. In this phase the log is fed
by the five authored missions and one generated type: a delivery between two real places
chosen deterministically from the gazetteer.

### P6-D7 — Trust
Owner, 2026-11-10: one **global** trust and one trust **per hub** (a hub is any very large
city; other places count toward the nearest hub). Both are rows in `progression`. A played-
time counter per save is stored from the start, because decay (Phase 8) runs on played time
only and never on the wall clock. Two gates in this phase: the spaceport lends a rocket;
the home plot can be extended. Trust changes only through mission outcomes in this phase.
Hubs are listed in a data file derived from the gazetteer by population.

### P6-D7a — Skills
Owner, 2026-11-10: there are no classes. Actions feed **skill types** (driving, flying,
repair, building, growing and others; the table is data and meant to be wide). Phase 6
records skill progress from mission steps in `progression` and shows it on the device. What
a higher skill changes is an open owner question, so nothing reads the values yet.

### P6-D8 — Home
One `plot` row. A play-mode build UI reuses the editor's placement code with a restricted
prefab list and writes deltas, not layers.

### P6-D9 — The device: menus live on a phone and a tablet in the world
Owner direction, 2026-11-10: in-game interaction menus are on a phone or tablet the
character carries, in the manner of a wrist or hand-held device in a space sim, not on
full-screen overlays.

- **One UI, two bodies.** A phone the character raises (one hand, quick tasks, usable
  while walking) and a tablet (two hands or docked in a vehicle; map, mission detail,
  building). Both show the same apps at different layouts.
- **It is an object in the scene.** The UI is drawn to a texture through `ed_gpu` and shown
  on the device's screen mesh, lit and reflected like anything else. Raising it plays an
  animation and eases the camera toward it; the world keeps running. Nothing pauses, which
  is also what Phase 7 needs.
- **Apps:** Messages (the needs log, P6-D6), Skills (P6-D7a), Map and navigation, Missions, People and trust,
  Vehicles (where things are parked, summon none), Home and build (P6-D8), Camera,
  Settings. A notification is a sound and a glance at the device, never a modal, and only
  urgent messages raise one.
- **Interaction in the world.** Looking at something usable shows a small anchored prompt;
  one press does the default action; holding opens the choices on the phone. Anything with
  more than a few choices is a device screen.
- **What stays off the device:** a minimal HUD (compass, speed and altitude in vehicles,
  the navigation line) that can be switched off, and the title screen.
- **Input:** pointer and keyboard, controller focus navigation, and touch. A touch screen
  maps onto the device directly, which is what the mobile tier of Phase 9 wants.
- **Vehicles** have a dock: the tablet becomes the centre display, so the same map is in
  the car, the plane and the rocket.

The UI layer stays separate from egui (P1-D14) and is described by data. The aero-glass
tokens of `../ui/README.md` are kept for the HUD; the device screens need their own design
pass, and the existing full-screen menu concept is superseded by this decision.

### P6-D10 — Saves and first run
Saves are the store (P5-D7); there is no separate save format. A first-run sequence places
the player at the home plot with the first message already in the log. Restart resumes at the
last consistent batch.

### P6-D11 — The five missions
Per path-to-solo §1: two transport or delivery (one needing the car, one multi-modal to the
spaceport), one repair, one time trial, one first footfall on the Moon. Each names its
markers, vehicle and trust outcome in its file.

### P6-D12 — Editor stage E4, second half
Mission files created and validated in the editor (markers and prefabs exist; steps
reachable), and play-from-here with a chosen mission state.

## 5. Budgets

| Item | Budget | Basis |
|---|---|---|
| Frame rate through the whole loop | 60 fps on High | Roadmap |
| Worst frame during ascent, transfer and descent | 33 ms | Phase 2 flight budget |
| Regime hand-over error | under 1 cm and 1 mm/s | Must not be visible |
| Quit to desktop | 2 s, with a consistent batch written | Contract rule 5 |
| Restart to playable at the saved place | 15 s | Phase 3 complete-view budget |
| Save size after the full loop | reported; expected to be megabytes since geometry is never stored | Contract §1 |

## 6. Workstreams

| # | Scope | Owns | Done when |
|---|---|---|---|
| W1 | Rocket model, staging, regime hand-over | `ed_sim/src/vehicle/rocket*`, `ed_space` | Scripted launch to orbit and back with hand-over inside budget |
| W2 | Transfer, time acceleration, guidance display | `ed_space`, `ew` | Scripted Earth–Moon–Earth round trip |
| W3 | Moon descent and landing; Moon site | `ed_sim`, `layers/` | Landing by hand on the authored site |
| W4 | Boarding and cargo | `ed_sim/src/tiers*`, `ed_store` | Car carried to the Moon and back; car left behind is found |
| W5 | Mission runtime, needs list, generated delivery | new `mission` module in `ew` or `ed_sim` per `check-deps` | Five missions complete by script |
| W6 | Trust and gates | `ed_store`, mission outcomes | Rocket refused before, lent after |
| W7 | Home plot and build UI | `ew`, `ed_editor` placement code | A placed object survives restart |
| W8 | Device UI: render-to-texture UI layer, phone and tablet models and animations, apps, world prompts, vehicle dock, minimal HUD | new `ed_ui` crate (inferred; subject to `check-deps`), `ed_anim`, `ew` | The whole loop is played with no full-screen menu after the title screen |
| W9 | Editor E4 second half | `ed_editor` | A new mission authored and played without leaving `ew` |
| W10 | Spaceport as a working place | `layers/`, prefabs | Pad, bay, parking, markers |
| W11 | Loop harness | `ew` | The whole loop runs from a script, with a restart in the middle |

## 7. Order

```
W1 ──> W2 ──> W3 ──> W4 ──┐
W5 ──> W6 ──> W9 ─────────┼──> W11 ──> recording
W8 ───────────────────────┤
W7, W10 ──────────────────┘
```

M14 = W1 to W4, W10. M15 = W5 to W9, W11 and the exit gate. W1, W5 and W8 start together.

## 8. Exit gate

- [ ] The whole north-star loop (`VISION.md` §2) played solo in one recording.
- [ ] Quit, restart, and continue from the saved state.
- [ ] No loading screen and no representation switch anywhere in the recording.
- [ ] The five missions exist only as editor-authored data.
- [ ] The owner signs off flight feel.
- [ ] `CHANGES.md` has the loop's numbers.

## 9. Failure branches

| If | Then |
|---|---|
| The transfer is dull or too long | Change the acceleration multiples; the view stays continuous (path-to-solo §6) |
| Hand-over error is visible | Hand over only when no other body is in physics range; blend over a few ticks |
| A mission cannot be expressed as data | Record which; add the smallest condition type; a scripting language only on the rule of P6-D5 |
| The loop works but is not fun | Missions are data; iterate without engine work |
| The game UI layer is not ready | Ship the loop on egui panels and hold the gate open on UI only |

## 10. Needed from outside the code

| Need | From |
|---|---|
| Rocket model with a cargo bay and a cockpit | Art |
| Spaceport, home and Moon-site prefabs (final) | Art |
| Gazetteer, airports and launch sites, pinned and registered | Acquisition (`data/README.md` §7) |
| What a higher skill changes; how global trust is derived (`solastra-core-loop.md` §6) | Owner |
| Phone and tablet models, a raise-and-hold animation set, and screen designs for the apps | Art and owner |

## 11. What Phase 7 receives

A complete solo game whose persistence already runs the server's code, whose world state is
generation plus layers plus deltas, and whose missions and trust are rows in the shared
schema.
