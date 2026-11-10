# Editor — a mode of the engine

Design brief. Decisions D15, F5, F6, P1-D14, P1-D24 in `../DECISIONS.md`.

Owner requirement (2026-11-10): *"I would like a game engine editor like thing... I want to
be able to go into the game engine. I want to be able to load the world, place specific
buildings, markers, missions, all that stuff."*

## Goals

1. Open the real world at any place, at any altitude, in seconds.
2. Understand what is on screen: which node, which rung, which source feature and rule
   produced an object, and what it costs.
3. Place and edit specific things (buildings, props, markers, missions, road changes) that
   persist and merge cleanly in git.
4. Press play at the cursor and be in the game there.

## Non-goals

- Not a mesh, material-graph or animation editor. Assets and prefabs are made in Blender and
  exported as glTF.
- Not a separate application or a second renderer.
- Not a general scene-graph editor. The world is generated; the editor edits the rules'
  inputs and the override layers.

## 1. Shape

`ew --editor` (or a key in a development build) switches the same process into editor mode.
The simulation pauses or runs; the renderer, streaming and world are the ones the game uses.
The UI is egui drawn through `ed_gpu`.

What the engine must provide for this, which is why the editor starts in Phase 1:

| Need | Engine feature |
|---|---|
| Click an object | GPU id buffer → `ObjectId` (P1-D24) |
| Know where an object came from | Generators record (source feature, generator, rule row) per `ObjectId` in a debug chunk |
| Edits that survive regeneration | Override layers keyed by `ObjectId` and `NodeId` (F5) |
| See the edit immediately | Per-node regeneration on layer change; hot reload |
| Several viewports, gizmos | Multi-view rendering; an overlay pass for lines and handles |
| Undo | Every edit is a command over layer data |

## 2. Features by stage

### Stage E1 — viewer (Phase 1)

- Free camera with speed scaling from centimetres to orbital.
- Stats: frame and per-pass times, instance counts, memory.
- Pick any instance in the proving ground and see its id, mesh, material and LOD.
- Hot reload of shaders, materials and prefabs.
- Layer file loaded at start; a transform gizmo that moves a placed prefab and writes the
  layer back.

### Stage E2 — world browser (Phase 2)

- Go to latitude/longitude, a place name (packaged gazetteer), or a `NodeId`.
- Time of day and date; body selection (Earth, Moon).
- Overlays: node bounds coloured by level, rung, chunk source (pack, service, local),
  residency, load state; freeze the selector to inspect LOD from another viewpoint.
- Streaming monitor: request queue, upload budget use, worst frame.

### Stage E3 — world editing (Phase 3)

- Pick → source feature (Overture id, OSM tags), generator and rule row.
- Prefab browser; place, move, rotate, scale with snapping to terrain, parcels and the kit
  grid; `replace` and `suppress` on generated objects; volume suppress.
- Building parameters (style row, height, colour) as a `replace` override.
- Road patches: add, remove and re-profile segments on the street graph (`streets.md` §4).
- Terrain patches with falloff.
- Layer panel: create, order, toggle, and see a layer's diff.
- Undo and redo.

### Stage E4 — gameplay authoring (Phases 5 and 6)

- Markers: spawn points, landing pads, points of interest, trigger volumes.
- Missions: definition files bound to markers; validation that referenced markers and
  prefabs exist; play-from-here with a chosen mission state.
- Vehicle and NPC spawners; nav graph visualisation.

### Stage E5 — live and shared (Phase 7 onward)

- Connect to a development server; view other players and shard boundaries.
- Inspect and revert live deltas.
- Later: restricted in-game building tools for players reuse the placement code.

## 3. Prefabs and assets

- A prefab is a glTF scene exported from Blender plus a small manifest (anchor, footprint,
  collision, sockets for doors, kit family). `ew-bake` turns it into pack chunks with a LOD
  chain; the editor watches the source folder and re-bakes on change.
- FBX sources are converted to glTF in Blender; the engine reads only glTF.
- Prefabs that are buildings declare a footprint so placing one suppresses generated
  buildings and parcels fill underneath it, and so far rungs can include it.

## 4. Data written

Only override layer files (`world.md` §4.2), prefab manifests, rule tables and mission
definitions. All are text, diff well, and are loaded by the game with no export step.

## 5. Open questions

1. Whether mission logic is data only (states and conditions) or needs a scripting language;
   decide when the first three missions are written by hand.
2. Collaborative editing: git merges of layer files are assumed sufficient at first.
