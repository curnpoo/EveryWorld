# Path to the playable solo version

The map from the end of Phase 0 to the Phase 6 gate: **the north-star loop played solo in one
recording** (`../VISION.md` §2). It breaks Phases 1 to 6 of `../ROADMAP.md` into milestones,
each of which ends in a build you can run and judge. Gates, not dates (D18).

This file orders work and names what each milestone must show. The designs are in the briefs
(`world.md`, `renderer.md`, `streets.md`, `editor.md`, `water.md`), the interfaces in
`../contracts/`, the gameplay in `solastra-core-loop.md`. Where this file and a phase
document disagree on detail, the phase document wins; where they disagree on order, fix this
file.

## 1. The target

The solo loop, taken from the owner's north-star session with the friend removed:

1. Start at **home**, a plot you own, in or near a dense forest.
2. **Drive** a car on real roads from the forest into a city, with no loading and no gaps.
3. Answer a **need** in the city: one mission that asks for the car.
4. Drive to a **spaceport**, park the car, board a **rocket**.
5. **Launch**, fly to the **Moon** in one continuous view, land.
6. Do **one mission on the Moon**.
7. Fly back, land at the spaceport, **find the car where it was parked**.
8. Drive home. **Quit, restart: everything is where it was left**, and trust earned is kept.

What "playable" means for this gate: the loop runs start to finish at the High budget on the
M3 Pro, saves and restores, and is fun enough that the owner signs off car feel and flight
feel. It does not mean other players, living NPCs, interiors or destruction (Phases 7, 8).

### Gameplay scope in the solo version

From `solastra-core-loop.md`:

| Part of the core loop | In solo | Later |
|---|---|---|
| Needs reach you (pings, a board) | A needs list on the device, fed by authored missions and one generated type | NPC-driven needs at scale (Phase 8) |
| Go do it in person with the right vehicle | Yes: this is the loop | |
| Thanked, not paid | Trust per community, stored in `ed_store`; no currency | Gifts, lending, decay rules (Phase 8) |
| Trust opens the world | Two gates: the spaceport lends a rocket; the home plot can be extended | The full access ladder |
| Mission categories | **B** transport and delivery (two missions), **A** repair (one), **E** a time trial (one), **F** a first-footfall on the Moon (one) | **C** rebuild needs destruction; **D** social needs NPCs |
| Home | One plot, placed with the editor's tools in a play-mode build UI | Free settlement anywhere |

The owner's open questions in that document (one trust score or many, decay, roles) do not
block the solo build: trust is stored per community from the start, so either answer fits.

## 2. Shape of the work

Four strands run through all the milestones.

```
engine     ed_gpu ─ ed_render ─────────────── quality (VSM, GI, water) ──────────►
world      addressing ─ ed_planet ─ globe pack ─ ladder R0–R4 ─ streets ─────────►
play       ed_sim ─ mover ─ car ─ plane ─ rocket ─ missions ─ saves ─────────────►
tools      ew-bake ─ editor E1 ─ E2 ─ E3 ─ E4 ───────────────────────────────────►
```

Rules that keep it from drifting:

- **Every milestone ends runnable**, with harness numbers written to `../CHANGES.md`.
- **Riskiest first inside a milestone**: the item whose failure would change the plan goes
  before the items that only take effort.
- **Content flows through the real path from the first milestone**: packs, layers, ids. No
  demo-only loaders.
- **Play is not left to the end.** A controllable character exists from milestone 2 and a
  car from milestone 5, on placeholder art, so feel is tuned over the whole project.
- Strands that do not share files can run as parallel sessions against the contracts
  (`ed_sim`, `ed_pack` and the streets bake are the first three).

## 3. Milestones

Each milestone lists what is built, what the build lets you do, and the gate.

### Phase 1 — Engine base and editor shell

**M1. First frame.** `ew` window and main loop; Metal backend behind `ed_gpu`; shader
pipeline (Slang to Metal); frame graph; headless harness with PNG and CSV output.
*You can:* open a window, see a lit test mesh, resize, quit cleanly, run the same frame
headless. *Gate:* harness image matches a stored reference; per-pass GPU timings are in the
CSV.

**M2. Instances at scale.** Bindless resources; GPU instance culling and LOD selection;
indirect draws; the discrete-LOD geometry provider; materials v0 (PBR and toon); the
proving ground with 100,000 part instances; fly camera.
*You can:* fly through the proving ground. *Gate:* 100k mixed instances at 60 fps on High,
no per-instance CPU work, LOD cross-fades without pops.

**M3. Real content path.** `ed_pack` and `ew-bake`; glTF import with skinning, WebP and
texture transforms (P0-R7); meshoptimizer LOD chains; BC7 textures; hot reload; the owner's
character on screen, skinned; one Megascans surface and one Blender prefab converted.
*You can:* export from Blender and see it in the running engine. *Gate:* bake output is
deterministic and passes the licence gate; the proving ground loads only from packs.

**M4. Step, pick, save.** `ed_sim` on Box3D with the fixed step and replay hash; character
mover on the proving ground; editor stage E1 (pick, inspect, gizmo, save a layer, undo);
multi-view; the minimal Vulkan backend rendering the same scene.
*You can:* walk a character around, switch to the editor, move a prefab, restart and find it
moved. *Gate:* the Phase 1 exit gate (`../phases/phase-1.md` §7).

### Phase 2 — Planet, far field and streaming

**M5. A planet you can stand on.** `ed_planet` quadtree on `NodeId`; terrain from elevation
with one height truth; horizon and frustum culling; camera-relative rendering from ground to
orbit; collision chunks; the mover and a placeholder car on real terrain.
*You can:* walk and drive on bare terrain anywhere on Earth and fly up to orbit without a
seam. *Gate:* no cracks on a scripted ground-to-orbit path; frame time flat while moving.

**M6. A planet worth looking at.** Atmosphere and aerial perspective; cloud shell; ocean
surface v0; virtual-textured terrain material from land cover; rungs R3 and R4 (imagery,
night lights) baked into the **globe pack**; the Moon as a second body with its own terrain.
*You can:* see Earth from orbit with city lights on the night side, and fly to the Moon and
stand on it. *Gate:* the look captures for atmosphere and terrain are matched or beaten;
globe pack within its size budget (P0-R2).

**M7. It streams.** `ew-tiles` v1 serving baked chunks; the three sources (pack, service,
local generation) behind one loader; coarse-first loading with parent-until-children;
staged uploads; residency and memory budgets; editor stage E2 (world browser, go-to,
chunk inspector).
*You can:* spawn anywhere and see a complete view at once that sharpens in place. *Gate:*
the Phase 2 exit gate: one unbroken recording street → orbit → Moon view → back, no holes,
spawn-to-complete-view inside the budget from the alpha baseline: first ground image in
3 s, complete view in 10 s, no frame over 33 ms while flying (the alpha never completes the
view; `../reference/alpha-baseline.md` §5).

### Phase 3 — World content on the ladder

**M8. Streets.** Overture transportation plus OSM tags through the bake; the solver on
connectors with rule-split segments (P0-R4); lanes, junctions, heights, bridges and tunnels;
road surfaces into terrain; the nav graph; the car on real roads.
*You can:* drive three reference routes. *Gate:* no road gaps or height steps on those
routes; dangling ends under the threshold set in `streets.md`.

**M9. Buildings and the ladder.** `ed_world` refactored to emit per rung: part instances
(R0), shells (R1), merged proxies (R2); Overture buildings with the height rule; regional
styles on the modular kit; cross-fades between rungs; stable `ObjectId`s.
*You can:* fly from orbit down into Shibuya, Manhattan and Aix with the city present at
every distance. *Gate:* no pops on the scripted descents at 60 fps on High, memory fixed.

**M10. Forests and authored places.** Foliage ladder from canopy height and land cover;
ground cover; override layers applied in generation; deltas; editor stage E3 (place,
replace, suppress, road and terrain patches); the first authored places: the home area, the
spaceport, the Moon site.
*You can:* place a landmark in the editor and see it from 50 km; walk a dense forest.
*Gate:* the Phase 3 exit gate, including the landmark surviving a re-bake.

### Phases 4 and 5 — overlap by design

**M11. Lighting and materials.** Baseline lighting hardened as the Low tier; virtual shadow
maps; voxel GI; material library; foliage shading; volumetric clouds; full water.
*Gate:* reference scenes match or beat the alpha look captures at the High budget
(Phase 4 exit gate). Features that miss the budget stay on Ultra and do not block M14.

**M12. Vehicles with feel.** Car model (suspension casts, tyres, drivetrain) tuned on a
forest road and a city street; rocket boots; a plane; audio through the existing audio
crates; cameras; input for keyboard, mouse and controller; traffic on the nav graph at low
density.
*Gate:* the owner signs off car feel on both surfaces.

**M13. Things stay put.** `ed_store` on SQLite with the single schema; parked records;
sim detail tiers; saves at an interval and on quit; editor stage E4 (markers, spawners,
mission graphs).
*Gate:* the Phase 5 exit gate: park a car, go to orbit, return, and it is there; quit and
restart with the same result.

### Phase 6 — Golden path, solo

**M14. Space leg.** Rocket vehicle and flight model; launch, ascent and orbit with
`ed_space`; transfer to the Moon at accelerated but continuous time; descent and landing;
boarding with a vehicle carried aboard or left behind; the spaceport as a working place.
*You can:* fly spaceport → Moon → spaceport by hand with guidance aids. *Gate:* the round
trip in one recording with no loading.

**M15. A game around it.** Missions as data, authored in the editor; the needs list and
pings; trust per community with the two access gates; the home plot with a play-mode build
UI; HUD and menus on the authored UI layer; a first-run sequence; the five solo missions of
§1.
*You can:* play the whole loop. *Gate:* the Phase 6 exit gate: the north-star loop, solo,
in one recording, then quit, restart, and continue from the saved state.

## 4. What each milestone needs from outside the code

| Needs | By | From |
|---|---|---|
| Alpha look and performance baseline | M6, M7, M11 | Phase 0 (`../reference/alpha-baseline.md`) |
| Global elevation, bathymetry, land cover, imagery composite, night lights, Moon elevation, each pinned | M5, M6 | Acquisition per `../data/README.md` §7 |
| Tile service host (the owner's server PC at first) | M7 | Owner |
| Overture buildings and transportation, OSM tags, canopy height, climate normals for the reference regions | M8 to M10 | Acquisition |
| Modular kit, first regional family; spaceport, home and Moon-site prefabs | M9, M10 | Art, through Blender and the editor |
| Owner's character with a standard skeleton and a locomotion set | M3 (static), M12 (animated) | Art; the rig lacks neck and head bones (P0-R7) |
| Car, plane and rocket models with interiors visible from the seat | M12, M14 | Art |
| Legal review of the ODbL position | Before any public build, not before M15 | Owner |
| Answers to F9, F10, F11 | M4 (F10), M11 (F9); F11 only matters after M15 | Owner |

## 5. Order and what can run in parallel

```
M1 → M2 → M3 → M4 → M5 → M6 → M7 → M8 → M9 → M10 ─┬─ M11 ─┐
                │                                  ├─ M12 ─┼─ M14 → M15
                └ ed_sim, ed_pack (parallel)       └─ M13 ─┘
```

- From M3: `ed_sim` and `ed_pack` are separate sessions against their contracts.
- From M5: data acquisition and the bake tools run ahead of the engine milestones that
  consume them. The streets bake (M8) can start as soon as M3 delivers the pack writer.
- M11, M12 and M13 share little and may run together.
- Track G (cluster geometry) runs beside everything from M2 and never blocks a gate.
- Phase 7 does not start before M15 passes (roadmap rule).

## 6. Where the plan is most likely to bend

| Risk | Shows up at | Response already in the plan |
|---|---|---|
| The GPU interface is wrong for explicit APIs | M4, on the Vulkan backend | Change the interface then; that is why the backend is in Phase 1 (F7) |
| Terrain cracks or precision loss between ground and orbit | M5 | Integer-lattice mapping and node-local frames are already tested (P0-R1); the scripted path is the gate |
| Globe pack or tile service too large or too slow | M6, M7 | Fewer levels, coarser layers, service starts coarser (Phase 0 failure branch) |
| Road solver does not scale past one city | M8 | It runs per owner node with a fixed margin; widen the margin before changing the solver |
| City frame rate misses 60 fps on High | M9 | Rung distances are data; proxies take over earlier; Track G is the longer answer |
| VSM or voxel GI do not fit 18 GB | M11 | They stay on Ultra; the baseline path ships on High (F9) |
| Car feel takes many passes | M12 | A drivable car exists from M5 so tuning time is spread out |
| The Moon transfer is dull or too long | M14 | Time acceleration is a design parameter; the view stays continuous either way |
| The loop works but is not fun | M15 | Missions are data and authored in the editor, so iteration needs no engine work |

## 7. After solo

Phase 7 adds other players to a loop that already works: shards, QUIC, prediction, shared
vehicles, a co-operative mission, launches seen from other regions. Phase 8 adds the living
world that the rest of `solastra-core-loop.md` needs: NPCs with needs, destruction and
rebuilds, social missions, trust that is given and lost. Phase 9 ships.
