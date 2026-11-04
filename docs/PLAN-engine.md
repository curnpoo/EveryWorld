# Plan: EveryWorld as an engine (assets, agents, vibes)

Status: E1 in progress. Goal set by user: this codebase is the base for many future things, so adding a new asset, animal, car, NPC or "world vibe" must be a data entry plus (at most) one generator, never surgery in streaming/tilegen.

## Principle: facts -> placements -> library -> backends
```
real data (OSM, DEM, weather)          ed_data
        |  what is here
world rules (biome, region, roads)     ed_world::{tilegen, biome, roads, archkit}
        |  emits PLACEMENTS: (asset key, transform, tint, seed) in tile space
asset library (defs + generators)      ed_world::library   (engine-free, data-driven)
        |  cooked once per asset: mesh, collider, render path
render/physics backends                ed_game::library    (Bevy)
   Meshlet (static opaque)  | Instanced (wind, alpha: grass, leaves)
   Skinned (animals, NPCs)  | Dynamic (vehicles: meshlet body + moving parts)
```
- Tiles stop baking discrete objects into tile meshes. They emit placements. The game instantiates library assets (one GPU mesh, many instances; meshlets cull and LOD per cluster per instance).
- Exception, by design: terrain-conforming scatter (pebbles, small rocks, grass) stays per-chunk geometry built on the worker, because each piece must sit on the `TerrainGrid` and a shared clump would float. Rule of thumb: an object you could pick up and move is a library asset; a texture of objects on the ground is scatter.

## 1. Asset library (`ed_world::library`, `ed_game::library`)
- `AssetKey` = stable string ("prop/street_light", "rock/boulder_a", "animal/deer", "car/hatchback"). Interned to `AssetId(u16)` at startup; placements carry the id.
- `AssetDef { key, render: RenderPath, source, collider, tags }`.
  - `source`: `Procedural(fn(seed) -> MeshData)` now; `Gltf(path)` later (imported models go through the same cook).
  - `collider`: none | box | capsule | convex hull of mesh.
  - `tags`: biome/region/role hints used by spawn rules.
- Cook: MeshData -> palettized -> `MeshletMesh` (+ collider). Cached on disk in `.earthdrop-cache/cooked/<key>-<hash>.meshlet_mesh` once glTF/large assets arrive (Bevy has a meshlet asset format), so startup stays fast.
- Content packs: a plugin calls `library.register(def)`. "Animals pack", "EU cars pack" = one file each. Spawn rules (below) reference keys, never code.

## 2. Agents (animals, NPCs, traffic, player vehicles)
One framework for everything that moves on its own:
- `AgentDef` (data): body asset, locomotion (`Biped | Quadruped | Wheeled | Flying | Swimming`), size/mass, speeds, brain kind, sounds, spawn rule.
- Components: `Agent`, `Locomotion`, `Brain` (small state machine: idle, wander, flee, follow-path, drive), `Perception` (nearby player/agents via spatial hash).
- Movement sits on the same ground truth: `TerrainGrid` height, OSM road centerlines (`RoadOutput::centerlines` already exists) for traffic and sidewalks for pedestrians, water polygons for boats/birds landing.
- Player vehicles become `VehicleDef`s (today `vehicles.rs` hard-codes cuboids): the same def drives traffic AI and the player car; physics model (raycast wheels, plane aero) is selected by def.
- **Population** is deterministic: density per cell = f(biome, `Cover`, region, road class, hour, weather, seed). Deer at forest edges at dusk, pigeons in plazas, gulls on coasts, cars by road class and hour. Agents spawn in a ring around the player and despawn far away; beyond that, the world only keeps counts (simulation LOD, not render LOD: nothing visible pops, agents are simply not simulated where nobody can see them).
- Animation: procedural first (the robot `body.rs` IK approach generalises to quadrupeds: gait phase + foot IK on the TerrainGrid). Skinned glTF later; skinned meshes use the standard (non-meshlet) path.

## 3. World vibes (`WorldMood`)
A resource computed every frame from (time, sun, weather, region, biome, season, altitude):
- grading: exposure, fog color/density, sky tint, bloom; per-region tables like `archkit` (Mediterranean warm haze, Alpine crisp, Tokyo neon at night).
- ambience: audio beds by biome/hour (birds dawn chorus, crickets at night, city hum, surf), wind loudness from weather.
- particles: pollen, dust, falling leaves, rain/snow (later fed by `ed_climate`).
- life density: scales the population rules (empty streets at 4 am, busy at noon).
Everything that wants mood reads `WorldMood`; nothing reads raw weather/time directly. Tables, not branches.

## 4. Plugin contract
- Each feature is a Bevy plugin with: its defs (data), its systems, and no knowledge of other features except shared resources (`AssetLibrary`, `WorldMood`, `TerrainQuery`, `RoadGraph`, `WorldOrigin`).
- Engine-free logic (rules, generators, brains, population math) lives in engine-free crates with unit tests, as today.
- `TerrainQuery` resource: height/normal at any world position from loaded tiles' `TerrainGrid`s, so agents/vehicles/props never re-derive ground.

## Milestones
- **E1 (now)**: `library` with street props as first assets (meshlet instances instead of baked tile mesh). Proves registry + cook + placement + meshlet instancing.
- E2: boulders (large rocks) and tree trunks as library assets; street furniture variety (benches, bins, hydrants, bus stops) by region.
- E3: `TerrainQuery` + `RoadGraph` resources; vehicles.rs -> `VehicleDef`; ambient traffic on roads.
- E4: agent framework + first animal (birds: flocking, cheap, everywhere) and pedestrians.
- E5: `WorldMood` + ambience audio beds + region grading tables.
- E6: glTF import through the cook path + disk cache of cooked meshlets.
- Perf tracks (PLAN-perf.md) continue in parallel: instanced foliage (Track A), own cluster renderer for leaves/buildings (Track C), terrain CDLOD.
