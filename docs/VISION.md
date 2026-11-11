# Vision — what we are building, in the owner's words

This is the source of truth for **what** the game is. Every section pairs the owner's
**verbatim** statements (2026-11-08 session) with an explanation and the concrete systems
that deliver it. When this and an older doc disagree, this file wins.

- Locked decisions: `docs/DECISIONS.md`
- Target design: `docs/ARCHITECTURE.md` and the briefs in `docs/plans/`
- Order of work: `docs/ROADMAP.md`
- World data: `docs/data/README.md` + `docs/data/registry.md`
- Art/asset contract: `docs/solarpunk/spec.md`, `docs/solarpunk/character-spec.md`

**Name:** TBD — the owner is deciding. The repo is `EveryWorld`, the current binary is
`earthdrop`; those stay as codenames until the rename. ("Second Earth" appears only as the
title of the contributed data registry, not as a chosen name.)

---

## 1. The game in one paragraph

> "Solarpunk inspiried futurism with a persistance vibrant and lively world with endless
> activites, but not overwhelming, the player will have direction, paths, and options if they
> choose. The most important reason someone logs in is because they want to explore the worlds
> with their friends, see new places, interact with npcs to learn more about the world, earn
> new tech and vehicles and forms of transport through ranks, trust, or merit rather than
> money, and they get to establish themselves, a home base, they build it, or use existing
> structures. There might be weapons in the game, there might be a currency, but im leaning
> less and less towards violence and currency"

**What this means.** A social exploration game, not a grinder and not a shooter. Four pillars:

1. **Explore together.** The primary loop is going places with friends — on Earth, the Moon,
   Mars, and beyond. Content is *places and people*, not loot treadmills.
2. **Guided freedom.** Endless activities, but the player always has an offered direction:
   a suggested mission, a route, a person to talk to. Nothing is mandatory.
3. **Merit, not money.** Progression unlocks tech, vehicles and transport through **ranks,
   trust and merit** — doing things for people and communities. Currency and weapons are
   de-prioritized design options, not foundations; nothing in the core loop may require them.
4. **A place of your own.** Every player can establish a persistent home base — built, or
   adapted from an existing structure.

**Real systems.**
- *Direction without overwhelm* → a **mission/activity director** (`ed_server` missions
  service + HUD guidance): a small curated set of nearby offers, co-op missions, and
  "people to meet" hooks; delivery and racing are the first two activity types.
- *Ranks/trust/merit* → **progression records in `ed_store`** (per-player rank, per-community
  trust, merit flags) with unlock tables for vehicles/tech; the missions service awards them.
  No currency table in the core schema.
- *Home base* → **housing plots + building**: persistent plot records (f64 body-fixed),
  assembly from the modular solarpunk kit (`docs/solarpunk/spec.md`), or claiming/adapting an
  existing generated structure. Persisted like parked vehicles (`ARCHITECTURE.md` §10).
- *Learn the world from NPCs* → authored dialogue/lore line pools attached to places,
  schedules and missions (`ARCHITECTURE.md` §10). NPCs are the world's narrators.

---

## 2. The north-star session (what the finished game feels like)

> "dense forest with city visuals while multiplayer players are hanging out nearby, and on the
> moon, im driving from a forest thats dense and realistic, into a city, my friend sees me
> drive by, gets in the car, we drive through the city, do a mission together, then my friend
> on the moon says theres cool loot there, so we drive to the rocketport, get in the rocket,
> blast off towards the moon at super speed, get to the moon, do that thing there, get back in
> the rocket that we left on the opposite side of the moon, go back to the spaceport, get in
> my car, drive back to my place and log off. thats the whole idea, so we will just be
> building towards that, i figure getting teh ground to orbit to moon and back path is
> important as the foundation, from there we can refine graphics, and alongside refining
> graphics and creating the beautiful world, we work towards adding multiplayer features that
> give the world meaning"

**What this means.** This is the acceptance test for the whole project. It also fixes the
*order of work*: the seamless ground → orbit → Moon → back path is the foundation; visual
quality and multiplayer grow on that skeleton together, not years apart.

**Real systems.** The phases that deliver it are in `docs/ROADMAP.md`. The solo version
must be playable (Phase 6) before multiplayer lands (Phase 7).

---

## 3. One planet, no transitions

> "i wanna be able to see the earth from space, fly down, and its the earth on the ground, i
> dont want a flat rendered plane that we tranistion to, no mans sky does it quite well"

> "the whole earth rendering thing is wrong i think."

**What this means.** The Earth is one continuous curved surface from orbit to pebble — the
No Man's Sky / Star Citizen feeling of descending onto a real planet. The failure in the alpha
is a *representation* bug (streamed tangent tiles below ~200 km, a separate satellite sphere
above, a third globe in the menu), not a precision bug. f64 coordinates and a floating origin
stay; the separate representations go.

**Real systems.** `ed_planet`: cube-sphere quadtree per body, camera-relative f64→f32,
elevation + imagery sampled from the slippy data sources onto the same geometry at every
altitude, one atmosphere, one cloud field. See `plans/world.md` and `plans/renderer.md` §4. Acceptance is a
single unbroken ground → orbit → ground recording (ROADMAP Phase 2 gate).

---

## 4. The living, shared universe

> "my ultimate dream is to be able to drive a vehicle on the roads, see people flying above me
> in rocket boots, then i can see someone taking off in a rocket far far away. i want this to
> be a living breathing world that brings people together in a persistent universe."

> "server instancing based on region, with live states saved for where my house is placed,
> where my car is parked, i should be able to see a rocket take off in the distance from
> another player as well. persistent as well becuase i havent even mentioned this but i want
> to fill the world with npcs, ai driven cars, rockets taking off, people you can talk to in
> the world for missions, a genuine livelyness in the world like its got a dense population.
> i want to see life everywhere, i wanna see someone go from there house, to the city to work,
> then i can talk to them and its random, or maybe certain lines they speak, but like they
> live a persitent life, and only need to be rendered if someone is nearby, but mostly
> scripted so we arent running crazy ai sims on the server, since initially this will just be
> hosted on a gaming pc power server, but eventually if this gains enough traction i will have
> to switch to a more powerful system."

**What this means.** (a) One persistent world, region-instanced — your house and car are
*places*, not session props. (b) Distant events are real: a launch on another continent is
visible from your backyard. (c) Density of *life*, not density of *simulation cost*: NPCs lead
scripted persistent lives and are promoted to full fidelity only near players. The server is
a gaming PC (Ubuntu LTS) until traction forces an upgrade.

**Real systems.** Region shards + directory, the **sky-event bus** (coarse global replication
of launches/trajectories), `ed_store` persistence for plots/vehicles/NPC assignments, NPC
schedule system seeded by (npc id, UTC) with four sim tiers (statistical → kinematic → full
body → rendered). Details: `ARCHITECTURE.md` §9 and §10. Netcode: prediction
for owned entities, interpolation for the rest; vehicles support seats and authority transfer.

---

## 5. Art direction: photoreal world, cartoon people, solarpunk civilization

> "push for photoreal environment, the stylized part is the characters, wacky like fornite,
> cel shading, realism, cartoony, and then the buildings and the world will be solarpunk, so
> stylized because its not like real life, its a one with nature sustainable energy connected
> world. so glowy stuff, vines and trees everywhere. we can take advantage of quixel megascans
> since they just made all of it free and open for agents and whatver anyone wants. we will
> heavily realy on those assets to decorate the world and create the procedural world"

> "i want ultra realism, fun cartoon characters. a custom world that feels solarpunk, player
> persistence and streaming with vehicles and effects on windows and mac and eventually
> iphone, maybe android."

> "i want dense forests, trees that have a thick canopy, lush ground cover as well. i want it
> to feel like an unreal engine demo but be able to fly around on the earth, then go to space,
> moon, mars, with no loading screens."

**What this means.** Three art layers, deliberately contrasted: a **photoreal natural world**
(Megascans scans, real GI/atmosphere, dense geometric foliage); **expressive, cartoon-
proportioned characters**; and a **solarpunk built world** that is stylized by *design*
(one-with-nature, renewable, glowing accents, vines everywhere), not by realism. This refines
`docs/solarpunk/spec.md`: the spec's "avoid photorealism" clause applies to the *built
assets'* cleanliness, not to the environment's lighting/material fidelity.

**Realistic lighting is the foundation — everywhere.** One physically based lighting model
covers the whole game: PBR/GGX materials with correct energy, **dynamic global illumination
in the voxel-GI / Lumen class** (voxelized scene or SDF traces, multi-bounce, screen-space
fallback so off-screen light still arrives), **virtual shadow maps** for consistent
high-resolution sun/shadow detail at every distance, real **reflections** (screen trace
first, hardware ray tracing where available), physically based atmosphere/sun, real exposure
and tone mapping. Cel shading is **not** a separate unlit renderer and **not** a global art
style switch — it is an optional *material model* that characters (and only characters, by
choice) can use, still lit by the same sun, GI and reflections so they sit inside the
photoreal world instead of looking pasted on.

**Cartoon ≠ cel.** The characters are stylized primarily by *design* (proportions, silhouettes,
expressive shapes, owner's `Solarcat.glb` style). Their shading is a per-avatar choice:

| Character shading | Who uses it | How it renders |
|---|---|---|
| **Realistic PBR** | Players who want to look like people (and realistic NPCs) | Full GGX + GI + reflections, same as the world |
| **Cel / toon (MToon-style)** | Players who want the Fortnite/anime look (default for the owner's first hero) | Quantized bands + outlines, but sampled from the *same* light/GI/reflection data |

So: the *environment* is always photoreal, the *lighting is always realistic*, and any
individual character can opt into cel shading. Nothing in the pipeline makes cel the default
for the whole game.

**Real systems.** `ed_assets` Megascans bake pipeline (license: free on Fab again as of
2026-11-05, any engine, no standalone redistribution — private repo + packaged game is fine);
cluster-LOD geometric foliage with far billboards inside the same DAG; **two material models
in one lighting pipeline** — `PBR` (world, vehicles, realistic characters) and `Toon` (cel
characters, sun/GI/reflection-aware) — selected per material/avatar, not per build; the
modular solarpunk kit with region tables and vegetation dressing. See `plans/renderer.md`
and `ARCHITECTURE.md` §8.

---

## 6. Characters and self-expression

> "i have a glb character thats rigged (albiet a terrible rig) but that style of character to
> start, eventually i want to include vrchat style avatas, full custom, different shaders,
> people can show off their avatars, its about self expression, so someone might choose to be
> human, someone might choose to be a catgirl, someone might want to be a robot, someone might
> want to be a meme character. npcs will maybe have to be metahuman, or something similar, i
> want realistic human and robot npcs that coexist"

**What this means.** Start with the owner's rigged GLB (retargeted onto our standard
humanoid skeleton). The endgame is **player-authored avatars as self-expression** — VRChat-
style variety including custom shaders — while NPCs are **as realistic as we can make them**:
high-fidelity humans and robots rendered with the full PBR/GI/reflection path, never cartoon
or cel. The contrast is the point: the *inhabitants of the world* look real, and players show
themselves however they like beside them.

**Real systems.**
- `ed_anim`: GLB/glTF skinned import, **retargeting** (weak rigs are expected), clips +
  motion matching, cloth/jiggle for ears/hair/straps per `character-spec.md`.
- **Avatar format: VRM** (the VRChat ecosystem's glTF-based standard, incl. MToon) alongside
  plain glTF; avatar validation + a **sandboxed shader/material model** (users pick from
  parameterized **toon *or* realistic PBR** material graphs — real/catgirl/robot/meme all
  supported, and a realistic-looking avatar is a first-class choice, not a downgrade;
  arbitrary user shader code is not executed). Both models are lit by the same sun/GI/
  reflections. Avatar data is cosmetic-only on the wire; server enforces size/budget limits.
- **NPCs are max-realism, PBR-only** (cel/toon is never used for NPCs): scanned-style human
  assets — "MetaHuman or something similar", subject to a **license check** since MetaHuman
  content has historically been UE-restricted (evaluate Reallusion Character Creator,
  CC0/MakeHuman-based pipelines, or purpose-built scans) — plus procedural robots with real
  materials, wear and emissives. They share `ed_anim`, full skeletal animation, facial/
  eye animation and cloth/hair, and the same skin/eye/hair shading models as realistic player
  avatars. Fidelity is tiered by distance (sim tiers in `ARCHITECTURE.md` §9), not by
  art style: an NPC is never "downgraded to cartoon" to save cost.
- MotionBricks is an optional NVIDIA-only client-side motion layer, never a server dependency
  (`ARCHITECTURE.md` §9). It is most valuable on NPC crowds.

---

## 7. Vehicles, physics and destruction

> "the cars and vehicles i have right now dont matter, but i do want as close to assetto
> corsa cars, microsoft flight sim planes, realistic boats, and rockets should feel real. but
> everything can be a little faster than normal. there will also be flying cars, ragdoll
> physics and dynamic animations. nvidia motionbricks would be amazing to use for the
> characters. so look into that as well, but i want to make sure that it translates over the
> server well. also this will lead to building destruction, and i can knock over trees, i
> want smoke sims as well if the perfomance budget can handle it, so if rapier can help
> great. but i think box3d phyics plus a custom engine with deterministic phyics is the way
> to go. but cars have to handle well"

**What this means.** Simulation-grade handling with simcade pacing, on a deterministic Box3D
base inside the custom engine. Existing vehicle code is disposable; the *feel* targets are
not. Destruction and knockable trees are systemic, not scripted set pieces. Smoke is
budget-gated.

**Real systems.** `ed_sim` (Box3D C17 behind an abstraction; cross-platform determinism +
replay), custom tire/suspension/aero/buoyancy/rocket models, sim-LOD + parked records,
pre-fractured buildings and hinged trees activated only near players, server-owned.
`ARCHITECTURE.md` §9. Car feel is an explicit Phase 5 sign-off.

---

## 8. Handcrafted + guided procedural, never "vibecoded"

> "the game should not be ALL CODE, there will be a lot of handcrafted assets, i dont want it
> to feel vibecoded, i want it to feel like a guided procedural world that is put together. i
> will have a lot of help. in creating the right vibe for the game, but data will drive a lot
> of what we see on earth."

**What this means.** Data decides *what is there*; procedural rules lay out the *ordinary*
world; **handcrafted assets, hubs, landmarks, missions and art direction provide the
authorship**. Procedural output is the canvas, not the finished painting. Humans (with AI
assistance as a tool, not an aesthetic) own the vibe.

**Real systems.** The modular kit + Megascans library are authored assets by definition;
`ed_world` emits *placements of authored parts*, not invented geometry. Hand-authored hubs,
spaceports and landmarks are placed into the generated world as first-class content (scene/
prefab format in `ed_assets`). Lookdev boards and, later, an in-engine scene editor are the
designers' tools. Mission and dialogue writing is authored content in data files.

---

## 9. Beyond Earth: stations, civilizations, lanes, and a fourth world

> "I will want space stations, bases, as well as civilizations on the moon and mars, with
> clear lanes between them. i also want a 4th world you can visit, something entirely new,
> maybe manufactured by the humans, but still beautiful, but maybe more industrial"

**What this means.** Space is inhabited, not empty: orbital stations, surface bases, real
settlements on the Moon and Mars, and legible **travel lanes** connecting them (beacon routes,
scheduled traffic, visible contrails of other players). The fourth world is a *designed*
place — human-manufactured, beautiful, more industrial (e.g. an orbital habitat / megastructure;
concept TBD) — and is the first world not bound to real Earth data.

**Real systems.** `ed_planet` generalizes to any body (Moon/Mars first, procedural terrain →
LOLA/MOLA). Stations and the fourth world are **instanced structure worlds** in body/orbit
frames using the same renderer, sim and persistence — not a separate game. Lanes = persistent
route records + nav beacons + sky-event traffic. Settlements reuse the solarpunk kit plus new
industrial asset families (authored, §8). Scheduled in ROADMAP Phase 8 onward; the fourth world is
a design exploration after the golden path works.

---

## 10. The data vision: a twin Earth that grew into solarpunk abundance

> "here are all the sources we could use for the data ... to create the full vision im looking
> for, like a twin earth thats evolved beyond what we have now into solarpunk abundance and
> tech meshing with nature"

**What this means.** Two jobs for data: reconstruct the real Earth's physical foundation, then
transform it into a rewilded, technologically abundant civilization. Three states, kept
separate: **Real Earth reference → Solarpunk transformation → Living planet simulation.**

**Real systems.** The full source catalog with licensing traffic lights is
`docs/data/registry.md`; acquisition phases, the dataset-registry schema, storage and
distribution rules are `docs/data/README.md`. Two immediate consequences: the alpha's
**WorldClim** climate input is license-RED for a commercial packaged game and must be
replaced, and the **EOX Sentinel-2** mosaic is CC BY-NC-SA and must be replaced or licensed
before selling.

---

## 11. Technical decisions

The locked technical decisions are kept in one place, **`docs/DECISIONS.md`**, so they cannot
drift between documents. In short: a custom Rust engine with its own GPU interface and an
editor; one cube-sphere planet; Box3D physics; region servers on Ubuntu; water designed from
scratch; virtual shadow maps and voxel dynamic GI as lighting targets; the Bevy alpha frozen
on `alpha-early`; a private repo and a packaged commercial game. The name is undecided.

---

## 12. Building interiors

> "also one more note is building interiors of course" (2026-11-09)

**What this means.** Buildings are not hollow facades: players go inside — homes, shops,
community spaces, transit hubs, and the player's own base. Interiors are part of the world,
not separate levels; no loading door-teleports for ordinary buildings.

**Real systems.** Procedural floor plans from the modular kit (2 m grid, door/window modules
align by construction, `docs/solarpunk/spec.md`), authored interiors for hubs and landmarks,
instanced interior parts with their own streaming/LOD tier (interior detail loads only when
near/inside), collision == visual, and interior lighting that works with the GI system.
Home-base building (VISION §1) uses the same interior assembly. Scheduled in ROADMAP
Phase 8, with the kit's door/window/wall contract enforced from Phase 3 so exteriors never
have to be rebuilt to admit interiors.

---

## 13. The engine itself: editor, far field, streets (owner, 2026-11-10)

> "What I ultimately want is to make sure that we have a persistent streaming world. I want
> to be able to do LOD culling so that I can reduce the amount of triangles on screen. I want
> to make sure we can have near infinite view distance so I can see the moon. I can see the
> Earth, so I can see Zimbabwe from space. I want to be able to see city structures from
> very, very, very far away."

> "I want to be able to procedurally understand the street data that we're using... I want
> to choose the right data sources."

> "I would like a game engine editor like thing... I want to be able to go into the game
> engine. I want to be able to load the world, place specific buildings, markers, missions,
> all that stuff."

> "Nanite system can be figured out in parallel to the engine. The engine needs to be built
> with far field distance in mind, nanite helps with far cities, but also we can do LOD
> options... I want to make sure the foundation is correct."

> "What is currently built for metal runs not great, but looks pretty good, loads slowly,
> has gaps... What I want is a very solid base to build off of... a game engine with editor."

**What this means.** The engine is a product goal in its own right, not only a means. It
must stream a persistent world without gaps, keep triangle counts under control with LOD and
culling, show everything from a street to another planet in one view, understand streets as
structured data, and include an editor for placing authored content on the generated world.
The alpha's look is the floor, not the goal (P1-D25): the rebuild must look far better (more
detail, better lighting and clouds, every building filled in, solarpunk architecture) as well
as fix its speed, loading and gaps.

**Real systems.** One node address and a ladder of representations per node for far-field
views; chunk streaming from a globe pack, a tile service and local generation
(`plans/world.md`). A GPU-driven instance renderer with discrete LOD as the base and cluster
geometry as a parallel track (`plans/renderer.md`). Streets from Overture and OpenStreetMap
tags (`plans/streets.md`). Editor mode with stable object ids and override layers
(`plans/editor.md`). Decisions D15–D18 and F1–F13 in `DECISIONS.md`.
