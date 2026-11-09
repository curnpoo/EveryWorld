# MASTER PLAN — the solarpunk universe (working title: EveryWorld; final name TBD)

**One file for the whole project:** vision, goals, architecture, systems, data, art, phases,
implementation and the record of the 2026-11-08/09 rearchitecture session.
Detail docs are linked throughout; if anything disagrees, `VISION.md` (goals) and this file's
§1 (decisions) win.

> **Status:** custom-engine reboot, Phase 0 (prepare). **Pacing:** multi-month on purpose —
> roughly **6–10 months to the full vision** (owner, 2026-11-09). The playable alpha was built
> in 4 days with AI coding models; speed is high, but phase gates keep the game playable.
> **Branches:** `main` = new engine. `alpha-early` = frozen playable Bevy alpha.

## Contents

1. Decisions log (locked)
2. Vision — the game, in the owner's words
3. Session record — what the audit found and what we did
4. Target architecture
5. Systems & implementation (planet, renderer, sim, characters, world, net, data)
6. Data & licensing
7. Art direction & reference imagery
8. Roadmap — Phases 0–8 with gates and indicative pacing
9. Acceptance metrics & testing
10. Risks & mitigations
11. Open questions
12. Document & reference index

---

## 1. Decisions log (locked 2026-11-08/09)

| # | Decision |
|---|---|
| D1 | **Custom engine, own GPU abstraction.** Metal first, D3D12/Vulkan later. Bevy removed completely. |
| D2 | **One cube-sphere planet** for render geometry; slippy Web-Mercator tiles remain only the data layer. No separate globe, no flat-plane switch, no loading screens. |
| D3 | **North star = the full session loop** (§2.2). Ground→orbit→Moon→back foundation first; graphics and multiplayer grow on it together. Solo loop before multiplayer. |
| D4 | **Physics: Box3D** (C17, MIT, cross-platform deterministic + replay) behind `ed_sim`; custom vehicle models (Assetto Corsa cars / MSFS planes / boats / rockets, faster than life); ragdolls, destruction, knockable trees; smoke if budget allows. |
| D5 | **Art:** photoreal environment (Megascans-heavy), cartoon characters, solarpunk built world. **Lighting/materials/reflections/GI are realistic everywhere; cel shading is a per-avatar material option, never a global style switch.** Handcrafted assets + guided procedural, never "all code". |
| D6 | **Universe:** one persistent world, **region-instanced servers**; persistent houses/vehicles; distant launches visible; dense **scripted** NPC life; first host Ubuntu LTS gaming PC. |
| D7 | **Progression:** ranks/trust/merit, **not money**; violence and currency de-prioritized; persistent home bases (build or reuse). |
| D8 | **Repo private; packaged commercial game.** Fab/Megascans assets allowed in-repo and in-game; never redistributed standalone. |
| D9 | **All-in on the new engine.** Playable alpha preserved on branch `alpha-early`; alpha visual bugs are replacement criteria, not a fix list. |
| D10 | **Characters:** owner's rigged GLB first (`Solarcat.glb`, kept uncommitted for now), retargeted; later **VRM/VRChat-style custom avatars** with sandboxed toon-or-PBR materials. **NPCs are max-realism PBR only** — high-fidelity humans + robots, never cel (MetaHuman-class licensing to verify). |
| D11 | **Alpha-era docs archived** to `docs/alpha/`; current direction lives in `VISION.md`, `ROADMAP.md`, `ARCHITECTURE.md`, `GOALS.md`, `data/`, `plans/rearchitecture.md`, and this file. |
| D12 | **Building interiors are in scope** — walk-in buildings, no door-teleports (owner note, 2026-11-09). |

Open: product name; public playtest timing; fourth-world concept (§2.8).

---

## 2. Vision — the game, in the owner's words

Full verbatim set with per-goal system mapping: `docs/VISION.md`. The essentials:

### 2.1 Identity

> "Solarpunk inspiried futurism with a persistance vibrant and lively world with endless
> activites, but not overwhelming, the player will have direction, paths, and options if they
> choose. The most important reason someone logs in is because they want to explore the worlds
> with their friends, see new places, interact with npcs to learn more about the world, earn
> new tech and vehicles and forms of transport through ranks, trust, or merit rather than
> money, and they get to establish themselves, a home base, they build it, or use existing
> structures. There might be weapons in the game, there might be a currency, but im leaning
> less and less towards violence and currency"

Four pillars: **explore together · guided freedom · merit not money · a place of your own.**

### 2.2 The north-star session

> "...im driving from a forest thats dense and realistic, into a city, my friend sees me drive
> by, gets in the car, we drive through the city, do a mission together, then my friend on the
> moon says theres cool loot there, so we drive to the rocketport, get in the rocket, blast
> off towards the moon at super speed, get to the moon, do that thing there, get back in the
> rocket that we left on the opposite side of the moon, go back to the spaceport, get in my
> car, drive back to my place and log off. thats the whole idea..."

This loop is the project's acceptance test. Beat-by-beat system requirements:
`docs/plans/rearchitecture.md` §1.

### 2.3 One planet, no transitions

> "i wanna be able to see the earth from space, fly down, and its the earth on the ground, i
> dont want a flat rendered plane that we tranistion to, no mans sky does it quite well"

One continuous curved surface, orbit to pebble. The alpha's hard switch at ~200 km was a
representation bug, not a precision bug. → `ed_planet` (§5.1).

### 2.4 A living, shared universe

> "my ultimate dream is to be able to drive a vehicle on the roads, see people flying above me
> in rocket boots, then i can see someone taking off in a rocket far far away. i want this to
> be a living breathing world that brings people together in a persistent universe."

> "...i want to fill the world with npcs, ai driven cars, rockets taking off, people you can
> talk to in the world for missions ... they live a persitent life, and only need to be
> rendered if someone is nearby, but mostly scripted so we arent running crazy ai sims on the
> server, since initially this will just be hosted on a gaming pc power server..."

Region shards + sky-event bus + four NPC sim tiers (§5.6).

### 2.5 Art direction

> "push for photoreal environment, the stylized part is the characters, wacky like fornite,
> cel shading ... the buildings and the world will be solarpunk ... one with nature
> sustainable energy connected world. so glowy stuff, vines and trees everywhere ... we will
> heavily realy on those assets [Megascans] to decorate the world and create the procedural
> world"

Photoreal nature · expressive cartoon people (cel optional, PBR equally supported) · solarpunk
civilization (§7).

### 2.6 Characters & self-expression

> "...eventually i want to include vrchat style avatas, full custom, different shaders ...
> someone might choose to be human, someone might want to be a catgirl, someone might want to
> be a robot, someone might want to be a meme character. npcs will maybe have to be metahuman,
> or something similar, i want realistic human and robot npcs that coexist"

Owner's GLB first → VRM avatar platform + material sandbox (§5.4).

### 2.7 Handcrafted + guided procedural

> "the game should not be ALL CODE, there will be a lot of handcrafted assets, i dont want it
> to feel vibecoded, i want it to feel like a guided procedural world that is put together ...
> but data will drive a lot of what we see on earth."

Data decides what is there; procedural rules lay out the ordinary; **authored assets, hubs,
landmarks, missions and art direction provide the authorship** (§5.5, §7).

### 2.8 Beyond Earth

> "I will want space stations, bases, as well as civilizations on the moon and mars, with
> clear lanes between them. i also want a 4th world you can visit, something entirely new,
> maybe manufactured by the humans, but still beautiful, but maybe more industrial"

Stations and settlements reuse the same engine/persistence; lanes are persistent routes +
beacons + visible traffic; the fourth world is the first fully designed, non-Earth-data
world (later design exploration).

### 2.9 Interiors

> "also one more note is building interiors of course"

Walk-in buildings: procedural floor plans from the modular kit + authored interiors for
hubs, instanced interior streaming, no door-teleports (§5.5).

---

## 3. Session record — audit findings and actions (2026-11-08/09)

### 3.1 What the audit found

**Sound and kept:** f64 ECEF + floating origin; ~27k lines of engine-free, tested crates
(`ed_geo`, `ed_data`, `ed_world`, `ed_flight`, `ed_space`, `ed_runtime`, audio); determinism
and data-versioning rules; roads v2 design; region/biome tables; perf-measurement culture;
ephemeris; engine-audio port; ghost-protocol lessons.

**Shortcomings, ranked (S1–S10):**

| # | Finding | Consequence |
|---|---|---|
| S1 | Three renderers (Bevy/WESL, native Metal fed by mirroring Bevy entities, separate menu globe) | Every visual improvement costs 2–3×; entity-mirror blocks GPU-driven rendering |
| S2 | Per-tile merged geometry + impostor cards | Shibuya 15–23M tris/474 draws, 27–32 fps settled, unplayable while streaming; no photoreal density |
| S3 | Two Earth representations + hard switch ~200 km + third globe in menu | Fails the core fantasy |
| S4 | Bevy/Avian coupling (23.6k lines `ed_game`); Avian nondeterministic | Blocks custom engine + server authority |
| S5 | Generator emits merged vertex soups, not part instances | Blocks modular kit, instancing, destruction, interiors |
| S6 | Data gaps: no lanes/sidewalks, 10 m imagery, no real tree positions; EOX is CC BY-NC-SA; WorldClim redistribution-restricted | Roads, realism, forests, and commercial shipping |
| S7 | Server is a 361-line stub; no persistence/missions/NPC sim | The shared-universe dream |
| S8 | No skinned-character/animation pipeline | Character spec, avatars, NPCs |
| S9 | No asset pipeline (glTF→cluster bake, texture compression, scan import) | Megascans photorealism |
| S10 | Perf/bug effort going into a renderer being retired | Opportunity cost — stopped |

### 3.2 External tech evaluation (verified)

- **Solstice** (MIT): cluster-DAG builder, two-phase occlusion, software raster, voxel-clipmap
  GI, geometric foliage, no-hole page streaming. **Port techniques; do not adopt as engine**
  (WebGPU-shaped limits; static scenes only; no skinning; untested on Mac).
- **Box3D** (MIT, Erin Catto, June 2026): soft-step solver, character mover, CCD, wheel
  joints, heightfields, **cross-platform + multithreading determinism, recording/replay**.
  Pure-Rust port exists (`box3d-rust`, MIT). Young → isolate behind `ed_sim`.
- **Megascans**: free again 2026-11-05 (Fab Standard License: any engine, commercial OK, no
  standalone redistribution; AI training allowed). Megaplants are `.uasset` (UE-only) →
  prefer Megascans FBX/glTF + our procedural species.
- **MotionBricks** (NVIDIA): real-time generative in-betweener; Apache-2.0 code, commercial
  weights, **TensorRT = NVIDIA-only** → optional client-side layer; wire carries inputs/target
  poses only; never on the server.

### 3.3 Actions taken

- `alpha-early` branch created; alpha code WIP (impostors/loading/player/vehicles/tilegen)
  committed there; `Solarcat.glb` deliberately left uncommitted.
- New docs: `VISION.md`, `plans/rearchitecture.md`, `data/registry.md`, `data/README.md`,
  this file. Rewritten: `ROADMAP.md`, `ARCHITECTURE.md`, `GOALS.md`, `README.md`, `AGENTS.md`,
  `CLAUDE.md`. Alpha docs moved to `docs/alpha/` with superseded banners.
- No engine code started (owner instruction).

---

## 4. Target architecture

**Real data decides what is there. Authored assets + procedural rules decide how it looks.
One planet renderer, one deterministic sim, region-sharded persistent servers.**

### 4.1 Stack

Rust 2024 · custom app `ew` (winit, own scene/sim core) · `ed_gpu` RHI (Metal → D3D12/Vulkan)
· `ed_render` cluster-LOD + visibility buffer + voxel GI · `ed_planet` cube-sphere · Box3D
behind `ed_sim` · `ed_store` SQLite→Postgres · `ed_server` on Ubuntu LTS · data packs with
license gating.

### 4.2 Crate map

```
SURVIVE (engine-free, tested)
  ed_geo  ed_data  ed_flight  ed_space  ed_runtime  ed_engine_audio  ed_audio
  ed_world        — rules only: regions/biomes/archkit/roads v2/foliage/missions;
                    refactored to emit PART INSTANCES + heightfields + placements

NEW
  ed_planet   cube-sphere quadtree/body, slippy→planet sampling, virtual texturing
  ed_assets   bake CLI: glTF/FBX/VRM (Megascans) → cluster DAG, BC7/ASTC, collision,
              kit manifests, data packs
  ed_gpu      RHI: passes, bindless, heaps, timers, caps, upscaler + AS interfaces
  ed_render   cull/visbuffer/software raster, shadows→VSM, GI, atmosphere/clouds,
              ocean, post/upscalers, skinning, PBR + cel materials
  ed_sim      fixed-step deterministic sim: Box3D, character mover, vehicles,
              ragdolls, destruction, parked records, sim-LOD
  ed_anim     skeletons, retargeting, clips, motion matching, cloth/jiggle,
              optional MotionBricks client layer
  ed_store    accounts, plots, vessels, ranks/trust/merit, missions, NPC lives
  ed_net      protocol, interest tiers, prediction/reconciliation, sky-event bus
  ed_server   directory + region shards + hubs, missions service, NPC scheduler
  ew          app: window/input/UI/audio; fixed sim + interpolated render
```

Sim never imports render; server never imports GPU crates; client and server share
generation with pinned data versions.

### 4.3 Coordinates & invariants

- Truth **f64 body-fixed** (Earth ECEF; Moon/Mars own frames); render **f32 camera-relative**;
  floating origin for physics/local frames; reverse-Z infinite projection.
- One height truth per node; nothing floats or sinks.
- Generation pure in (seed, pinned data version, UTC); golden-hash CI; only deltas + entity
  state on the wire; fixed-step sim, interpolated render.
- Every rendering feature: cost per tier + off switch; RED-licensed data never ships.

---

## 5. Systems & implementation

### 5.1 Planet (`ed_planet`) — kills S3

- Six cube-sphere faces, quadtree per face; nodes are ellipsoid heightfield patches with
  geomorphing; curvature in the vertex transform. Ground, horizon, limb, orbit = same mesh.
- Slippy tiles stay as **data addressing** (`TileId`, caches, Mapterhorn/AWS, OpenFreeMap);
  planet samples them. Data and geometry are decoupled (old cubesphere rejection superseded).
- **Virtual texturing** for imagery (orthophotos + raw Sentinel-2, colour-matched), one mip
  chain orbit→boots. One atmosphere (Hillaire tables), one cloud field (volumetric near /
  2D shell far), night lights, ocean shell + FFT near camera.
- Moon/Mars: same code, different body constants; procedural terrain → LOLA/MOLA. Stations
  and the fourth world: instanced structure worlds in orbit/body frames.
- **Acceptance:** unbroken ground→orbit→ground recording, no representation/texture switch.

### 5.2 Renderer (`ed_gpu` + `ed_render`) — kills S1/S2

- **Port from Solstice (MIT):** cluster-DAG builder (128-tri clusters, watertight cuts), GPU
  cut selection, two-phase occlusion, software raster, visibility buffer + resolve, cached
  shadows, voxel-clipmap GI + screen traces, cloud model, no-hole fixed-memory paging, BC
  textures, geometric foliage thinning.
- **Build new:** dynamic objects + previous transforms, skinning + motion vectors, 64-bit
  atomic visbuffer (cap-gated), bindless argument buffers, MetalFX/DLSS/FSR3, optional HW RT,
  translucent/emissive materials, destruction swaps, planetary VT, VSM later.
- **One lighting model, two shading models.** Realistic PBR/GGX with physics-based
  atmosphere/sun/shadows, **voxel-clipmap GI with multi-bounce + screen traces**, and **real
  reflections** (SSR; HW ray tracing where available) is the default for *everything*.
  **Cel/toon is an opt-in material model for characters** (VRM/MToon-style: banded response +
  outlines) that samples the same sun/GI/reflection data, so a cel hero is lit by the same
  world as a photoreal NPC. Cel is per-avatar, never a global switch — a player who wants a
  realistic human avatar gets the full PBR path.
- **Foliage:** geometric near/mid; billboard clusters *inside the same DAG* far (the separate
  impostor renderer and its flicker bugs die). Lush geometric ground cover near.
- **Tiers:** Ultra (RTX desktop) / High (M3 Pro, 60 fps 1440p) / Low (mobile); perf gates on
  six reference scenes + golden-path route.
- **Salvage from `ed_metal`:** device/layer/timers, ocean FFT, probe-GI bake, shadow caching,
  shader maths. The entity mirror is deleted.

### 5.3 Simulation & vehicles (`ed_sim`) — kills S4

- Box3D C17 behind an abstraction (Rapier fallback); deterministic replay tests in CI;
  server-authoritative snapshots + client prediction (no lockstep).
- Custom vehicles on Box3D queries: raycast suspension + Pacejka-style tires + gearbox;
  per-surface aero; buoyancy; staged rockets (`ed_flight`). Simcade tuning tables; **car feel
  is a Phase 4 signoff**.
- Character mover + rocket-boots flight; ragdolls; pre-fractured buildings and hinged trees
  activate only near players, server-owned, persisted as deltas; smoke = tier-gated particles.
- **Sim-LOD:** full bodies → kinematic f64 → parked records (f64 body-fixed) → statistical.
  No live ground physics from orbit.

### 5.4 Characters & animation (`ed_anim`)

- glTF/VRM skinned import + **retargeting** (owner's `Solarcat.glb` first); clips + motion
  matching; cloth/jiggle (ears, hair, straps, terrarium per `character-spec.md`).
- **Avatar platform:** VRM + glTF, parameterized **toon *or* PBR** material sandbox (no
  arbitrary user shader code) — self-expression includes looking realistic, cel-shaded, or
  anything between; server-side budget limits; cosmetic-only on the wire.
- **NPCs are max-realism and PBR-only — cel/toon is never used for them.** Scanned-style
  humans (MetaHuman-class, **license check for non-UE use**; Reallusion Character Creator /
  CC0 / MakeHuman-based pipelines as alternatives) + procedural robots with real materials,
  wear and emissives. Full facial/eye/cloth/hair animation on the same `ed_anim`; fidelity is
  reduced with distance (sim-LOD), never by art style.
- MotionBricks optional on NVIDIA clients; inputs/target poses only on the wire; most valuable
  on NPC crowds.

### 5.5 World generation, assets & interiors (`ed_world` + `ed_assets`) — kills S5/S9

- Generators emit **placements of authored parts** (solarpunk kit, 2 m/0.5 m grid,
  `BIO_RGN_ASSET_VARIANT`), not merged meshes → instancing, destruction and interiors all
  become possible. 17 alpha regions map onto the spec's five solarpunk families + regional
  variants.
- `ed_assets` bakes glTF/FBX → cluster DAG + BC7/ASTC + collision + kit manifests; handcrafted
  hubs/landmarks/missions are first-class content inserted into the generated world.
- **Roads:** finish roads v2 (levels, deck solver, tunnels, junctions, medians, nav graph) +
  lanes/sidewalks from Overture/Layercake; acceptance = driveability (collision == visual,
  grade limits, no floating).
- **Interiors (D12):** procedural floor plans from the kit (doors/windows align by
  construction), authored interiors for hubs, instanced interior parts with their own
  streaming tier (detail loads near/inside), GI-correct interior lighting; player home-base
  building uses the same assembly. Kit contract enforced from Phase 3 so exteriors never
  block interiors.

### 5.6 Multiplayer, persistence & NPC life (`ed_net` + `ed_server` + `ed_store`) — kills S7

- Directory + **region shards** (geographic handoff), always-on hubs, Moon/Mars processes;
  single-process multi-region on the first Ubuntu host, split as load grows.
- **Sky-event bus:** coarse global replication of launches/trajectories — a rocket taking off
  anywhere is visible from your backyard, including from the Moon.
- Interest tiers ~20/5/1 Hz by speed/altitude; prediction + reconciliation for owned,
  interpolation otherwise; vehicle seats + authority transfer.
- **Persistence:** accounts, **ranks/trust/merit**, housing plots, parked vessels,
  inventories, missions, NPC home/work assignments; SQLite→Postgres; SP saves share format.
- **NPC life:** scripted schedules seeded by (npc id, UTC); authored dialogue/lore; traffic
  on the nav graph; ambient spaceport launches. Statistical → kinematic → full → rendered
  tiers keep a gaming-PC server alive. No generative-AI NPC sims on the server.
- **Activity director:** a few curated offers nearby + co-op missions + people to meet —
  endless activities, never overwhelming (VISION §1).

### 5.7 Platforms

macOS/Metal first; Windows D3D12 (DLSS/FSR3, RTX) second; iOS Metal Low tier later (ASTC,
tile memory); Android Vulkan last. Cross-platform input/UI/audio in `ew` from day one.

---

## 6. Data & licensing

Full catalog (20 categories, satellite programs, formats, acquisition priorities):
**`docs/data/registry.md`**. Implementation rules: **`docs/data/README.md`**.

### 6.1 Three states, never mixed

**01 Real Earth reference** (`ed_data`, pinned) → **02 Solarpunk transformation** (`ed_world`
+ `ed_planet` + authored assets) → **03 Living simulation** (`ed_climate`/`ed_sim`/server +
persisted deltas).

### 6.2 P0 licensing issues (must fix before any commercial build)

| Source | Problem | Action |
|---|---|---|
| **WorldClim** (baked climate normals) | Commercial + redistribution restricted | Replace: CHELSA / TerraClimate / ERA5 / NASA MERRA-2 + POWER |
| **EOX Sentinel-2 mosaic** | CC BY-NC-SA 4.0 (noncommercial) | Replace with raw Sentinel-2 + NAIP/national orthophotos, or license |
| HydroSHEDS (not yet used) | RED | Use JRC Global Surface Water / OSM water / NASA SWOT instead |
| OSM (ODbL) | Share-alike + attribution | Document derivative-database structure; generated attribution |
| Megascans/Fab | No standalone redistribution | Private repo + packaged game only (D8) |

### 6.3 Registry & gates

One TOML per dataset in `data/registry/` (fields: ids, endpoints, coverage, resolution,
formats, license + URLs, permissions, attribution, screening GREEN/YELLOW/RED, version,
consuming systems). Tooling: **license gate** (RED never ships; YELLOW needs review notes),
**attribution generator** (`CREDITS.md` + in-game credits), **version pinner** (bake-pack
`meta` signatures).

### 6.4 Acquisition order (aligned to phases)

- **Phase 0–2:** SRTM/Copernicus (have), ETOPO + GEBCO, ESA WorldCover (have), SoilGrids,
  Natural Earth, WorldClim replacement, raw Sentinel-2 + NAIP/national orthos, licensed
  hydrography.
- **Phase 3–4:** Overture (pinned) + Microsoft ML footprints (CDLA Permissive), lanes via
  Geofabrik/Layercake/TIGER, Meta/WRI canopy height + GEDI, PLATEAU/3D BAG, 3DEP lidar.
- **Phase 5–6:** WorldPop + GHSL, Black Marble/VIIRS, GeoNames gazetteer, agriculture (CDL,
  WorldCereal, MapSPAM).
- **Phase 7+:** ERA5/GPM, World Ocean Atlas + Argo + HYCOM, MODIS, GBIF (CC0/CC BY only),
  Yale BSC/Gaia, LOLA/MOLA. Later: pollution/hazards/cryosphere/historical Earth.
- **Storage:** hierarchical streaming global→local; master dev library separate from shipped
  regional packs; **no live API required for basic gameplay** in the shipped game.

---

## 7. Art direction & reference imagery

### 7.1 The three art layers

1. **Photoreal nature:** Megascans scans, physical atmosphere, **real GI, real reflections,
   PBR materials**, dense geometric foliage, lush ground cover — "an Unreal demo you can fly
   off of." The lighting/materials pipeline is realistic for the entire game; nothing gets a
   cheaper unlit look.
2. **Expressive characters:** wacky, cartoon-proportioned heroes per
   `docs/solarpunk/character-spec.md`. For **players**, shading is a per-avatar choice:
   **cel/toon** (banded, outlined, MToon-style — the owner's first hero) *or* **realistic PBR**
   (players who want to look human). **NPCs are always max-realism PBR** — high-fidelity humans
   and robots, never cel, so the world's inhabitants read as real people beside however players
   present themselves. Everything is lit by the same sun/GI/reflections.
3. **Solarpunk civilization:** modular, one-with-nature architecture per
   `docs/solarpunk/spec.md` — five families (temperate forest, urban rewilding, arid desert,
   tropical/coastal, arctic/alpine), 2 m/0.5 m grid, renewable tech, vines and glow accents.
   Stylized by *design*, rendered with full physical materials and lighting.

### 7.2 Solarpunk reference boards (in repo)

![Solarpunk World Lookbook](solarpunk/Solarpunk%20World%20Lookbook.png)

![Solarpunk Game Asset Guide](solarpunk/Solarpunk%20Game%20Asset%20Guide.png)

### 7.3 Character reference (`character-spec.md`)

High-fidelity stylized anime eco-explorer: nekomimi, cybernetic arm with cyan emissives,
utility flight suit, chunky boots, solar-wing jetpack with terrarium, goggles. Rigged for
ear/hair/strap dynamics.

![Character reference](solarpunk/image_20261107_212618.jpg)

### 7.4 UI direction (aero-glass design system, `docs/ui/`)

![HUD concept](ui/everyworld_hud_concept.jpg)

![Menu concept](ui/everyworld_menu_concept.jpg)

### 7.5 Alpha lookdev screenshots (what the alpha already achieved)

These are the *baseline to beat*, from `docs/lookdev/round2/`:

| Trees | Clouds | Ground |
|---|---|---|
| ![Forest closeup](lookdev/round2/trees/trees_forest_closeup.png) | ![Clouds sunset](lookdev/round2/clouds/clouds_sunset.png) | ![Forest floor](lookdev/round2/ground/ground_forest_floor_stand.png) |

More boards: `lookdev/round2/{trees,clouds,ground}/`. In-game alpha bug screenshots were
reporter Desktop captures and are not in the repo; new captures should go to `docs/images/`.

---

## 8. Roadmap — Phases 0–8

Full version: `docs/ROADMAP.md`. Indicating pacing only (not commitments): the owner's
expectation is **6–10 months total**; gates matter more than dates.

| # | Phase | Key work | Exit gate | Indicative |
|---|---|---|---|---|
| 0 | Freeze & prepare | Branch hygiene, alpha baseline, golden-hash tests, vendor Box3D/Solstice study, `data/registry` + license gate, crate skeletons, GLB import check | Repo ready for parallel engine work | Week 1 |
| 1 | Engine skeleton | `ew` winit app, `ed_gpu` Metal (salvaged), fixed-step loop, debug UI, headless perf harness, `ed_assets` bake CLI v1 (glTF→cluster DAG, BC7) | Native test scene + harness + bake works | Month 1 |
| 2 | Planet spike | `ed_planet` terrain + VT + unified atmosphere; ground↔orbit; procedural Moon | **Unbroken ground→orbit→ground recording at budget** (fallback: improve old representation before going further) | Months 1–2 |
| 3 | Renderer core | GPU cull, visbuffer, software raster, cached shadows, GI v1, real reflections, post + MetalFX, geometric foliage + Megascans, PBR + optional cel material, kit contract for interiors | **Forest + city 60 fps High, fixed memory, no holes** | Months 2–3 |
| 4 | Sim & vehicles | Box3D, character mover + boots, car v1, roads v2 driveability, parked records | **Forest→city drive signoff; park→orbit→return** | Months 3–4 |
| 5 | Golden path solo | Rocket, Moon landing, placeholder loot, save/load, housing plot | **Whole north-star loop solo, recorded** | Months 4–5 |
| 6 | Multiplayer | Directory + shards, prediction, shared car + seats, co-op mission, sky events, chat, server persistence | **Two clients share car + mission; cross-region launch visible; 8-player hub** | Months 5–6 |
| 7 | Living world & polish | NPC schedules, traffic, dialogue, mission variety, interiors, destruction/ragdolls, MotionBricks eval, VSM/DDGI/RT, weather, coupled water, smoke | World feels populated on the gaming-PC server | Months 6–10 |
| 8 | Platforms & ship | Windows D3D12, iOS/Android tiers, packaging, perf gates | Ships macOS + Windows; mobile tier proven | Months 8–10+ |

Rules: phases overlap; each ends playable; **no Phase 6 before Phase 5 passes**; parallel AI
sessions own disjoint crates; every landed change gets a `CHANGES.md` entry with numbers.

**Phase 0 detail:** `docs/phases/phase-0.md` — why the phase exists, seven workstreams with
pass criteria and artifacts, ordering, exit gate, pre-decided failure branches, and how each
workstream serves the VISION goals.

**Phase 1 detail:** `docs/phases/phase-1.md` — the spine (window, frame, fixed step, bake
pack), the 20 key decisions with alternatives and rationale, workstreams, week plan, budgets,
exit gate and failure branches.

---

## 9. Acceptance metrics & testing

- **Seamless:** one continuous recording forest street → orbit → Moon surface → Earth street;
  no cuts, representation swaps, texture regime pops, or >100 ms frames after warm-up.
- **Perf:** High tier 60 fps steady on six reference scenes + golden path; fixed GPU memory
  while streaming; p99 gate in the harness.
- **Persistence:** park car → orbit → Moon → log off → log in: car, rocket, house, inventory,
  ranks and missions intact.
- **Multiplayer:** shared car, co-op mission, cross-region launch visibility, 8-player hub.
- **Feel:** car signoff on forest road + city street; character readable against the photoreal
  world at noon and dusk; interiors walk-in without teleports.
- **Determinism:** golden-hash generation CI + Box3D replay tests on macOS + Windows.
- **Licensing:** license gate passes on every ship manifest; attribution generated.

---

## 10. Risks & mitigations

| Risk | Mitigation |
|---|---|
| Scope (multi-year vision, small team) | Phase gates; each phase playable; no Phase 6 before 5; 6–10 month pacing accepted |
| Custom renderer cost | Port Solstice builder/GI; salvage `ed_metal`; Phase 2/3 gates before depending on them |
| Box3D is young | Behind `ed_sim`; Rapier fallback; replay tests from day one |
| Cube-sphere reprojection invalidates caches/LOD | Bake packs + Phase 2 gate |
| Licence drift (Megascans changed terms twice) | Archive EULAs; CC0 fallbacks (Poly Haven/ambientCG); no raw redistribution |
| WorldClim/EOX currently non-shippable | P0 replacements in `data/README.md` |
| MotionBricks NVIDIA-only | Optional client layer; clips/motion matching fallback everywhere |
| Gaming-PC server load | NPC sim tiers; shard splitting before bigger hardware |
| iOS memory | ASTC bakes, Low tier, reduced GI volume |
| "Vibecoded" feel | Authored kit/hubs/missions; lookdev review; procedural only lays out the ordinary |

---

## 11. Open questions

1. **Product name** (owner deciding; `EveryWorld`/binary `earthdrop` are codenames).
2. Public playtest timing → when Phase 6 must be production-grade.
3. Fourth-world concept (manufactured, beautiful, industrial) → design exploration post-Phase 5.
4. NPC realistic-human asset source (MetaHuman license vs Reallusion/CC0/MakeHuman).
5. When `Solarcat.glb` and future raw assets get committed (kept uncommitted for now).

---

## 12. Document & reference index

**Current direction:** `docs/VISION.md` (verbatim goals + systems) · `docs/ROADMAP.md` ·
`docs/ARCHITECTURE.md` · `docs/GOALS.md` (status board) · `docs/plans/rearchitecture.md`
(full audit + plan) · `docs/data/` (registry + rules) · `docs/solarpunk/` (asset + character
specs) · `docs/CHANGES.md` · this file.

**Alpha archive:** `docs/alpha/` (BUGS as replacement criteria, HANDOFF salvage map, m2/engine/
nanite/space plans, roads, flora, buildings, beamng lessons, water, shadows, perf reports).
**Agent guides:** `AGENTS.md`, `CLAUDE.md`.

**External:** Solstice https://github.com/SamG-Coder/solstice (MIT) · Box3D
https://github.com/erincatto/box3d (MIT) + https://github.com/larsbrubaker/box3d-rust ·
Megascans https://quixel.com/news/update-megascans-are-going-free-again + Fab EULA ·
MotionBricks https://nvlabs.github.io/motionbricks/ · BeamNG.Earth lessons in
`docs/alpha/plans/beamng-earth-lessons.md`.
