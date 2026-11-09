# Lessons from BeamNG.Earth

Status: PLAN (2026-11-06). Nothing here is implemented. Written after reading the public repo
https://github.com/TecnaGamer/BeamNG.Earth (MIT licence, ~4 weeks old, v1.8.1 beta): its README, ROADMAP,
CHANGELOG, `docs/ARCHITECTURE.md`, `docs/BRIDGE_CLEARANCE.md`, `SOURCES.md`, and the module headers and
docstrings of `server/*.py`. I did not run the mod, and I did not read every function body. Where I say "they do X",
the source is a docstring or doc in that repo, named in brackets. Treat their performance numbers as theirs, not ours.

Why they are worth studying: they target the same idea (drive anywhere on real Earth, 1:1, streamed, deterministic),
they have shipped for weeks to real players, and almost every decision is written down with the reason and the bug
report that caused it. We can copy the ideas and the data choices. We cannot copy the code (Python/numpy plus a Lua
mod for BeamNG; we are Rust plus our own renderer) and we must not copy their assets (BeamNG stock models and
textures). The MIT licence allows reusing ideas and even code; put a credit line in `CREDITS` if we port an algorithm
closely.

## 0. The one-paragraph summary

Their world looks better than ours for five reasons, in order of how much each matters:

1. **Better input data.** Aerial photos at 15-60 cm instead of 10 m Sentinel; Overture buildings (OSM plus machine-learned
   footprints and heights); OSM tags for lanes, sidewalks and parking; Meta's 1.2 m canopy height map, so trees stand where
   real trees stand; lidar-measured bridge heights.
2. **A real road model.** Every road piece has a level (ground, bridge, tunnel). A solver picks deck heights from the
   crossings underneath. Stacked interchanges, ramps, tunnels, approaches and junction geometry all follow from that.
3. **Everything agrees on one ground.** One graded elevation function is shared by physics, roads, sidewalks, buildings,
   trees and the visible mesh. Road plans are computed once per map cell and cached.
4. **Detail rules written down per country.** Markings, driving side, signs, lane colours, barrier types, building styles.
5. **A culture of measuring.** A fixed benchmark set of places, golden-hash determinism tests, and a dated doc entry with
   the user's quote for every change.

Our engine, determinism rules and renderer plan are already stronger in some places (Rust, one `TerrainGrid`, native
Metal, meshlet/Nanite direction). The gap is mostly data and road logic, not rendering.

## 1. Side by side

| Area | BeamNG.Earth | EveryWorld today | Gap |
|---|---|---|---|
| Runtime | Python server (`earthd`) + Rust kernels (`earthfast`) + Lua mod inside BeamNG | One Rust workspace, Bevy app + native Metal | We are simpler to ship; no change needed |
| Road/building/water data | Overture Maps (pinned release) + OSM US Layercake for lanes/sidewalks/surface | OpenFreeMap vector tiles (OpenMapTiles schema, OSM derived) | Large: no lanes, sidewalks, parking, width, full layer data |
| Elevation | Mapterhorn (best national DEM, 1 m in places), AWS fallback, Copernicus at poles | Mapterhorn z14 with AWS fallback | None for terrain; see section 4 for lidar bridge heights |
| Ground colour | Regional/state orthophotos 10-30 cm, NAIP 30-60 cm, Sentinel-2 cloudless fallback; de-shadowed and colour matched | Sentinel-2 cloudless 10 m (`ed_data/satellite.rs`) | Large, the biggest visual gap on the ground |
| Land cover | ESA WorldCover (masks to 8 ground materials) | WorldCover + climate + geology | Roughly equal |
| Trees | Meta/WRI canopy height model (1.2 m): each tree's real position and height; regional species | Procedural: WorldCover density + species table + genomes | Large for realism, ours is better for determinism and art control |
| Buildings | Overture footprints, parts and heights; region styles; roofs from footprint; garages you can enter | OSM footprints via vector tile, `render_height`; 17 regions in `archkit` | Medium: data coverage, parts, false-positive filtering |
| Roads | Levels, deck solver, tunnels, approaches, junctions, markings per country | Draped ribbons, bridge = straight line, no tunnels | Very large |
| Traffic / AI | Nav graph from connectors, lane flow, signals, parking spots, "distant traffic" | None | Planned in our Stage F |
| Streaming | Per-cell plan caches, revisit cache, baked packs, path look-ahead | Quadtree tiles, hole-free swaps, budgeted apply | Medium, see `tile-load-hitch.md` |

## 2. Data sources: the cheapest big win

This is where I would start, because it improves looks without touching renderer code.

### 2.1 Aerial photos instead of Sentinel-2 (ground looks)

[SOURCES.md] They try a regional orthophoto service first (Maryland 15 cm, a dozen US states, Switzerland, France, Spain,
Netherlands, Germany's states, Austria, Poland, Czechia, Japan, parts of Australia and Canada), then USGS NAIP across the
lower 48, then Sentinel-2 cloudless. Photos are then **de-shadowed** (`deshadow.py`) and **colour matched across seams**
(`colormatch.py`), with the season's colours graded in.

For us:
- `ed_data::satellite` is hard-wired to one URL. Turn it into a provider list: `trait ImagerySource { covers(tile) ->
  bool; fetch(tile) -> Image }`, tried in order, result cached forever like today.
- Start with NAIP (public domain, WMS/tiles from `imagery.nationalmap.gov`, covers the US) and Switzerland (swisstopo
  `wmts.geo.admin.ch`, Zermatt is one of our test scenes). Add others per region as data-only rows.
- Colour match: our per-tile 128/256/512 px layers will show seams where sources change. Their approach computes seam
  statistics per region, once, and caches them. We can start with a per-tile histogram match to the Sentinel base.
- De-shadowing is a research task on its own. Do it last.
- **Licence check, important:** our source comment says the EOX mosaic is **CC BY-NC-SA 4.0** (non-commercial). If this
  game is ever sold, this source must be replaced or licensed. NAIP and most national orthos are open. Their
  `CREDITS.md` lists the exact attribution each source asks for; we need the same file and an in-game credits screen.

### 2.2 Overture Maps for buildings (and more)

[`docs/research/2026-10-22-osm-vs-overture.md`, measured on six cities] Overture buildings are OSM's wherever OSM has them,
keeping height, levels, roof shape and colours, plus: machine-learned footprints where OSM has gaps (+66 % in
Florianopolis) and machine-learned heights (Hays, Kansas: 82 % of buildings have a height against 6 % in OSM). Overture's
`base` theme also carries street lamps, signals, stop signs, crossings, tree points, benches and hydrants.

For us:
- Overture ships as GeoParquet on S3/Azure with a STAC index. They read it by HTTP range requests, per map cell, through
  DuckDB. In Rust, the `parquet` + `object_store` crates or a small Python bake tool can do the same.
- Do not put this on the live path first. Bake offline (see 2.4), ship the result as the same MVT-like structure our
  generators already read, keep OpenFreeMap as the fallback. That keeps the world a pure function of (seed, pinned data
  version), which is our rule 4.
- Pin the release (they pin `2026-09-23.1`).

### 2.3 OSM tags the vector tiles drop

[same study] OSM has lane counts, sidewalks, on-street parking, street lighting, turn lanes, cycleways and smoothness.
Overture has none of these. They pull them from **OSM US Layercake** (`data.openstreetmap.us`, per-region GeoParquet). This
is what gives them lane counts (so road width is right), `sidewalk=both`, and bike lanes. Worldwide, Geofabrik extracts or
the same Layercake format are the equivalent. Without lane counts we cannot get road widths, markings or AI lanes right.

OpenMapTiles (what we use now) does keep: `class`, `subclass`, `brunnel`, `oneway`, `ramp`, `surface`, `layer`,
`service`. I have not checked that OpenFreeMap's tiles carry every one of those on every road. First task in 3.1.

### 2.4 Offline baking

[`bakepack.py`] A first visit's heavy map-only work is baked offline and shipped as one file per region (each cell's road
plans, 10-30 CPU-seconds for a dense cell). The pack is valid only for the inputs listed in its meta (data versions, plan
signature). This is exactly the "world is a pure function of (seed, pinned data version)" rule, and it is how they
support "drive through NYC at 500 mph". For us it is Stage C-adjacent; the design to copy is the `meta` check, not the
file layout.

### 2.5 Measured heights (later)

[`roadz.py`] Optional override of bridge and tunnel heights from real surveys: USGS 3DEP lidar in the US (public domain,
read from EPT octrees on AWS, ~0.3 MB a node), IGN BD TOPO in France, NVDB in Norway. Their own README says they
fall back to inference everywhere else, and inference is the real workhorse. Put this after the solver in section 3 works.

## 3. Roads (the part you asked about first)

### 3.1 What we have

`crates/ed_world/src/roads.rs` (1085 lines). Facts from reading it:
- `plan()` drops tunnels at line 298 (`filter(|f| f.str("brunnel") != Some("tunnel"))`).
- A bridge is `Bed::Bridge`: heights are a straight line between the ground heights at the two ends (lines 331-335). No
  clearance, no grade limit, no relation to what runs underneath. Nothing reads `layer` or `ramp`.
- `Bed::Graded` roads smooth their ground heights (box filter, radius 10, 3 passes) and `grade()` cuts/fills the terrain
  grid around them (cut slope `1.5 + 1.2*depth` capped at 8, fill `3 + 2.5*depth` capped at 14).
- Junctions are the count of road endpoints at integer tile-pixel positions (line 313). That is enough for a signal or
  stop sign, not for geometry.
- Everything is built per tile. A deck crossing a tile edge is two independent profiles.
- Known open bugs B5 (no markings), B6 (roads not on ground), B7 (guardrails) in `docs/BUGS.md`.

### 3.2 What they do

**Levels.** [`roads.level_ranges`, `tag_level_ranges`, `gradesep.py`] Each road feature is split into ranges: on the ground,
on a bridge, in a tunnel. Ranges come first from the map's `bridge`/`tunnel`/`layer` tags, then are extended by rules:
a deck is lengthened over roads that cross under it (`_skew_decks`), short ground gaps between two decks become deck
(`_deck_gaps`), and where two ground lines cross with no junction node and no bridge tag, the one whose surrounding
ground stands at least 4 m higher (and 4 m higher on each side) gets a bridge (`gradesep.py`, `SEP_M = 4.0`). That last
rule is expensive (reads the 8 neighbouring cells), so they ship it **off by default** except for the tunnel case. Take
that as a warning: measure the cost before we turn it on.

**Deck solver.** [`deck_curve`, `curve_profile`, `deck_profile_once`] Given a chain of deck pieces they compute heights:
- a requirement array: ground plus a lift, and 6 m of usable clearance under the girder at every crossing with a lower
  road, held for 20 m either side (corridor, not point) [`docs/BRIDGE_CLEARANCE.md`];
- a grade cap from each abutment (`DECK_MAX_GRADE`), so a deck cannot climb steeper than the roads that feed it;
- a "design grade" ramp up to the requirement and parabolic vertical curves through it;
- never below the straight chord between abutments, never below ground + lift;
- joined decks (a ramp forking off a mainline, side-by-side carriageways) are solved as chains so they share heights.
It logs `bridge clearance below 6.0 m target` rather than silently pretending it succeeded or steepening the approach.

**Stacked levels.** [`_deck_stacks`] Every 16 m along a higher-level piece, find lower-level pieces overlapping it in
plan (overlap means at least half the narrower deck). Each hit is a clearance requirement. Pieces that share an end are
exempt near the shared end (a ramp overlaps the mainline at the fork; without the exemption the ramp got a 2.2 m ledge,
their I-83 bug). This is the piece that makes real interchanges and double-deck bridges (Queensboro) work.

**Chains across cells.** [`ChainStore`, `ChainView`] Pieces that connect are solved together across cell borders, so one
deck gets one profile no matter which cell asked.

**Approaches.** [`grading.py`] Ground under a road is graded by a plan computed once per map cell: DEM sampled every
`PROFILE_STEP`, Gaussian smoothed, flat across the road core plus shoulder (or plus sidewalk), then a 1:2 cut/fill
transition clamped to `[T_MIN, T_MAX]`; several roads at one point blend by inverse distance. Bridge approaches are lifted
to meet the deck, with retaining walls where a road is cut into a hill. Plans do not depend on the player's anchor (they
use a region frame), so every player gets the same graded ground.

**Tunnels.** [`tunnel_sections`, `TunnelProfiles`, `tunnel_hole_mask`] Tunnel floor = lower convex hull of the ground
profile. Cover depth classifies each sample as open road, low arch (a covered cut) or full tube. A hole is cut in the
terrain over the tube, portals are added, and a "tunnel" tagged for 683 m in Central Park is correctly split into
arches and rock tube.

**Bridge look.** [`bridgetypes.py`, `pier_sites`] Beam, truss, arch, cable-stayed, suspension from OSM `bridge:structure`;
piers placed so they keep clear of roads below; headroom enforced; two decks of a divided road joined across a narrow
median; barrier types by country practice [`medians.py`].

**Junction geometry.** [`junctions.py`, `roadcorners.py`] Branches at a node are sorted by angle; between neighbours the
corner is a curb-return arc (radius by road class: residential 3 m, secondary 6, trunk 8), sidewalks cut exactly at
the tangent points, asphalt filled to the arc; crosswalks 0.5 m past the tangent; curb ramps with tactile pads; stop line
1 m before. Acute corners shrink the radius. Rural junctions get a rounded pavement flare (6 m, 12 m where a major road
meets) without curbs.

**Markings.** [`markings.py`] A per-country `Style` (colour of centre line, widths, dash patterns, edge-line rules, whether
a town's two-way arterial gets a double centre line) taken from the national manuals (MUTCD, TSRGD, RMS, MLIT, AS 1742).
Dashes are real geometry cut from the line, not a repeating texture, so they do not restart at tile edges.

**Widths.** [`roadwidth.py`] When a street has no mapped width they measure it: cast rays from the street centreline to the
nearest building wall each side, every few metres; if both sides hit and agree, the right-of-way is that distance, split
into sidewalks and carriageway. Free and surprisingly effective. We have building footprints already, so we can do this
with no new data.

**Dual carriageways, lanes through merges.** [`dual_carriageways`, `lane_flow`] Two one-way roads side by side become one
divided highway with a median; where a lane count is mapped for some pieces and not others, unmapped pieces take theirs
from the lanes flowing in and out (4 lanes plus an on-ramp continues as 5).

### 3.3 What we should build, in order

Each step is independently shippable, behind an off switch, with a CHANGES entry and a before/after screenshot.

| # | Step | Why first | Size | Exit check |
|---|---|---|---|---|
| R0 | **Fix B6 (roads on the ground) and B5/B7 basics.** Roads must sit on `TerrainGrid` before anything lifts off it. | Everything else builds on it | S-M | Zermatt, Aix, Manhattan: no floating or buried road in `tools/shoot_places.sh` |
| R1 | **Tile dump of road attributes.** Print which of `layer`, `ramp`, `brunnel`, `oneway`, `surface`, `service`, `subclass` OpenFreeMap really sends, with counts per class, for the 6 reference places. | Decides whether we need OSM/Overture for road tags | S | A table in this doc |
| R2 | **Road graph with stable ids.** Build nodes at shared endpoints (exact, from tile pixel positions across neighbouring tiles), edges with class, level, oneway, lanes. Pure function of tile data. This is also the base of the nav graph (section 8). | Needed by levels, junctions, AI | M | Same graph from any tile load order (hash test) |
| R3 | **Level model.** Read `brunnel` + `layer`. Add the "crossing with no node" detector as a **flagged, cached, tile-neighbour-aware** pass, measured for cost. | Interchanges cannot exist without levels | M | Overpass in Los Angeles / Dallas shows two levels |
| R4 | **Deck solver in Rust** (`ed_world/src/roads/deck.rs`): requirement array, grade cap, parabolic curves, chains. Start with the simple version: lower-bound envelope, then a two-pass grade-limited relaxation, then curve smoothing. Unit-test with property tests (clearance >= target where feasible, grade <= cap, endpoints exact, no NaN). | Core of bridges, ramps, overpasses | L | Test suite plus a stacked-interchange fixture |
| R5 | **World-space chain profiles.** Compute a chain's heights from world-space data so every tile that touches the chain derives the same heights. | Removes seam mismatches; matches determinism rule 4 | M | Hash of deck heights identical from two anchors |
| R6 | **Approach embankments and retaining walls** replacing the 14 m fill slope for decks. | Visible quality jump on ramps | M | Ramp meets deck without a wedge or step |
| R7 | **Tunnels.** Stop filtering them; floor by convex hull; open/arch/tube sections; portals; terrain hole mask in `TerrainGrid` and in the collider. The hole mask is the hard part because mesh, collider and the native renderer all need it. | Mountain and city tunnels | L | Drive through the Gotthard-type tunnel at Zermatt-side without seeing the tube from outside |
| R8 | **Bridge visuals:** girders, piers placed clear of lower roads, parapets, and the types table. Start with beam and arch. | Looks | M | Golden Gate (suspension) reads as a bridge |
| R9 | **Junction geometry:** curb returns, crosswalks, curb ramps, stop lines, replace the "endpoint count" signals. | Urban look and racing lines | M | Shibuya crossing |
| R10 | **Markings by country** as a data table (`StreetKit` already exists; add centre/edge/lane styles, dash lengths, double lines), real dash geometry, bike lanes and crossings. | Your "usable" look | M | Per-country screenshots |
| R11 | **Width from buildings** (rays to walls), then OSM lanes when available. | Right-sized streets everywhere | S-M | Thames St-style plaza is wider than a default street |
| R12 | **Dual carriageways, medians, barriers.** | Highways read as highways | M | Divided motorway shot |
| R13 | **Parking garages** with ramps and drivable levels, parking lot stalls. | You listed garages | M | Drive into a garage |

R0 to R5 are the foundation and should land first. R6 to R9 give the visible jump. R13 depends on the building work in
section 6.

### 3.4 Racing specifically

Things they handle that matter for a racing game and that we should plan from the start:
- **Physics surface == visible surface.** Their rule: tyres, roads and the visible ground sample one elevation function.
  For us: decks need their own colliders (the terrain grid is only the collider for ground roads). Use thin trimesh or
  a heightfield strip, a convex-decomposed compound is too heavy per tile.
- **Banking and super-elevation.** They preserve it on raceways (`UNGRADED` class so grading does not flatten an oval's
  banking) [`roads.STYLE["raceway"]`, `docs/research/2026-10-22-racetrack-data.md`]. Add a per-class "do not flatten"
  flag and a bank angle along curves for roads over a speed.
- **Surface per segment** (asphalt, dirt, cobble, sand) as a physics tag from OSM `surface`, not just a colour.
- **High-speed look-ahead.** See section 9.

## 4. Terrain and the ground around roads and buildings

What they do [`grading.py`, `surface.py`, `seacarve.py`, `rockface.py`]:
- Ground under every road is a function of the road, graded once per map cell and cached (section 3.2 approaches).
  Several roads blend by inverse distance.
- Surface masks: every 2 m sample takes one of eight materials from WorldCover and slope (rock above a slope threshold).
  Roads are painted into the ground photo and into the surface masks, so tyres feel asphalt versus dirt.
- Sea carving: real bathymetry (NOAA, 3 m along US coasts) so cars sink at the right depth, coasts get a shaped floor.
- Cliffs drawn as rock.

What we already have: `TerrainGrid` as the single height source (stronger than their multi-source setup), graded roads,
WorldCover and geology. What to take:
1. **Region-framed plans.** They plan graded ground in a fixed region frame (not the player's anchor) so every player gets
   identical heights. Our `TerrainGrid` is per tile with its own frame; when roads span tiles (R5) the grade plan must be
   computed from a neighbourhood and cached by (tile, data version). Same fix as R5.
2. **Blend where roads meet** by inverse distance. Our `grade()` takes the nearest road per vertex (`best` map), so two
   roads at different heights fight at a junction. Blend instead.
3. **Cut/fill walls.** They stand retaining walls where a cut exceeds a limit (dry stone in mountains and towns, concrete on
   major roads) and stone parapets on mountain roads. We have `foundation()` for buildings; a `retaining_wall()` sweep from
   `meshkit::sweep` is the same shape.
4. **Ground around buildings.** Their buildings go to `FOUNDATION_M` under the lowest ground plus a `SKIRT_M` (3 m near,
   10 m for the lite mesh) so a coarse far mesh cannot show daylight underneath [`buildings.py` header, from the user's
   "buildings are floating" report]. Our open question in `CLAUDE.md` (terrain at a coarser LOD than the building's grid)
   is the same problem; the skirt is their answer, and ours is a 0.5 m margin. Raise the skirt for the lite mesh as
   they do.
5. **Bathymetry** is a good later item (NOAA coastal).

## 5. Ground textures and biomes

| Idea | Source in their repo | For us |
|---|---|---|
| Photo is the ground colour, then de-shadowed and colour matched; season graded in | `imagery.py`, `deshadow.py`, `colormatch.py`, `season.py` | See 2.1. Our `SatPool` already supports per-tile arrays; the provider list is what is missing |
| Photo plus 8 material masks decide physics and detail layers | `surface.py` | We have splat weights from WorldCover; keep, add the "paved" mask from roads so roads sit in the photo |
| Köppen climate zone selects sounds, species, driving defaults | `climate.py` | We embed WorldClim already (`ed_data::climate`); add Köppen as a table for species and ambience |
| Regional species by latitude/climate | `vegetation.py` regional rules | Our `trees/species.rs` niche selection (`docs/plans/procedural-flora.md`) covers this |
| Autumn colour in season, bare trees in winter | `season.py` | Needs a UTC-date input to flora colour; deterministic if date comes from the world clock |
| Orchards in rows, shrub lines between crop rows | `vegetation.py` v21-22 | Add as land-use rules once land use data is read |
| Keep trees off roads, sidewalks, bridge decks, rounded junction corners | `vegetation.py` `_mask_decks`, versions 16-20 | We need the same exclusion masks from the road output. Easy once R2 exists |

**Trees from real canopy heights.** They run local-maximum detection on Meta/WRI CHMv2 (1.2 m resolution, global, AWS public
bucket) to place each tree at its real crown with its real height, one tree per flat crown (`thin_tops`), and keep out
building roofs the CHM counts. Result: streets, parks and field hedgerows have trees where they really are. Ours places
trees by density, which looks plausible but not real. The two compose well: use CHM for **position and height**, keep our
`SpeciesDef` and `Genome` system for **shape and species**. It is a COG range-read like our WorldCover reader
(`ed_data::cog` exists). Worth doing; expect a large data volume and a bake step.

## 6. Buildings: how theirs differ from ours

I compared headers and structure, not output. Our `buildings.rs` (1.4k lines) plus `archkit.rs` is already the same
overall shape as their `buildings.py` (4.3k lines): per-region style tables, facade bands, roof forms, foundation. The
differences that matter:

| Topic | Theirs | Ours | Take |
|---|---|---|---|
| Data | Overture footprints, parts, ML heights; predicted from footprint when missing, deterministic by hashing the footprint position | OSM via vector tile, `render_height` | Section 2.2 |
| Heights when unknown | Predicted from area, narrowness and class (towns 50-80 % known, Fells Point 2 %) | Defaults | Add the predictor as a table in `archkit`, hashed |
| Facade | Stock window sheets, each floor one band, whole number of bays per wall (no window cut at a corner) | Textured windows in `bparts` | Adopt "whole bays per wall" if we do not already. Cheap, visible |
| Roofs | Chosen from footprint; pyramid on plinth for monument-sized | `roof_plan` shared by LOD and detail (good) | Same |
| False positives | `_ml_false`, `_ml_steep`: drop ML-predicted buildings that sit on bridges, water, roads, medians, or steep ground | None | **Needed the moment we adopt Overture**, since ML footprints include mistakes |
| Buildings over roads | `_passages`, `road_through`, `road_across`: a building a road runs through gets an arch or is split | None | Gives arcades and tunnels under buildings; later |
| Shared walls | `_shared_walls`: adjacent rows of terrace houses share a wall, no doubled faces | None | Cuts triangles and z-fighting in dense old towns |
| Garages | `_parking_deck`: open storeys with a ramp, skin variants (reports #51/#52 "all garages look the same"), stair tower; driveable | Solid | R13 |
| Landmarks | `landmarks.py`: a name-token table of famous buildings | `missions.rs` only | Plan only after Overture; matches `ROADMAP` Phase 2 |
| Steel frame, stoops, porches, sloped plinths | `_steel_frame`, `_porch`, `_stoop`, `base_z` | `foundation()` | Add porches/stoops via `bparts` as data |
| Road width from walls | `roadwidth.py` | None | R11 |

The general lesson: **most of their building quality is data plus filtering plus a lot of small corrections**, each tied to
a bug report, not a clever algorithm. Adopt the report-driven loop (section 10).

## 7. Streets as a whole (sidewalks, furniture, signs)

[`sidewalks.py`, `furniture.py`, `highwaysigns.py`, `speedsigns.py`, `routeshields.py`, `milemarkers.py`, `guardrails.py`,
`medians.py`, `plazas.py`, `picnic.py`, `shelters.py`, `gasstations.py`, `powerlines.py`, `oilwells.py`, `ships.py`]

- Sidewalks are **one swept mesh per tile** with a curb profile (face, nose, top, slab, back, with end caps), 15 cm curb
  height, a cross slope so water runs off, back falling at 1:2 to the ground. They studied the stock West Coast USA map
  and copied its structure: one draw call per material per district. Ours: `StreetKit` has a curb height; the sweep
  profile approach maps directly onto `sweep()` in `roads.rs`.
- Highway guide signs from OSM destination tags and milestones, rebuilt weekly in their data repo. Large effort. Speed-limit
  signs are cheap and high value: the mapped limit or the class default, in the country's sign style.
- Street lamps spaced by national design standards, or OSM's mapped lamps. Night lighting needs this.
- Power lines along mapped routes with towers and conductors, gas stations in regional style, oil pump jacks, ships. Good
  "alive" items, all driven by OSM/Overture tags. Low priority until the road model exists.

## 8. Navigation graph, traffic, dense traffic

You want AI, racing and dense traffic. Their approach [`navgraph.py`, `parking.py`, `distantTraffic` doc, `lane_flow`]:
- **Nodes are connectors with stable ids** (Overture connector ids), edges carry half-width, lanes per direction, one-way,
  a driving target speed (min of mapped limit and a class default, with per-country defaults) and a drivability weight
  (motorway 1.0 down to service 0.4). Sub-classes like driveway, parking_aisle, sidewalk, crosswalk are skipped.
- **Signals and stop signs** are placed at real junctions from the map's tags.
- **Parking spots** along curbs (6.5 m long, 2.5 m wide, facing traffic on their side) and in parking-lot polygons.
- They hand this graph to BeamNG's own AI and traffic. We have to write our own driver. What carries over is the **graph
  data** and the rules.
- There is also `docs/DISTANT_TRAFFIC.md` and `docs/research/2026-11-01-distant-traffic-pr.md`, which I did not read. That
  is the right next read before designing dense traffic.

Our plan (matches Stage F in `ROADMAP.md`):
1. R2's road graph carries lanes, oneway, speed and signal/stop info. It lives in `ed_world` (engine-free, tested).
2. A simulation crate (`ed_traffic`, engine-free) holds vehicle state separate from render state (rule 4). Cars follow
   lanes with a simple car-following model (IDM), choose turns at nodes, obey signals. Determinism: each car's seed is a
   hash of (cell, slot, UTC time bucket).
3. Dense traffic cost: simulate only within a radius of the player, with a cheap "far" mode (fluid flow per edge, spawn
   cars when they enter the near radius). Render with instancing, one mesh per vehicle type. This needs the renderer
   plan in `engine.md` (GPU-driven draws), so it cannot be dense before Stage D.
4. Racing AI is the same graph plus a racing line per lane (curvature-limited speed profile).

## 9. Streaming lessons

[`ARCHITECTURE.md`, `blockworker.py`, `urgency.py`, `pacer.py`, `plancache.py`, `revisit.py`, `CHANGELOG`]

- **Plan once, cache by inputs.** Road plans per cell are a pure function of (cell, data versions, settings), shared between
  processes, with lock files to avoid planning the same cell twice. On a Steam Deck a first visit spent ~300 CPU-s planning
  the same cells up to four times before this.
- **Revisit cache.** What was built for a place is stored on disk keyed by the place and the code's hash; only a
  *complete* build is stored (a build during which any read failed is not).
- **Urgent work first.** When the car is about to run out of loaded ground, background jobs wait (up to 3 s each) so urgent
  ones get the interpreter. For us: `ed_runtime::jobs` has priorities; make "tile in the camera's path within N seconds"
  the top class.
- **Duty-cycle background work.** Memory-bandwidth heavy analysis threads made the game's shadow pass 5-6x slower (3.4 ms to
  19 ms) even though a CPU-only loop changed nothing. They throttle background jobs to a duty cycle (25 % by default).
  This is directly relevant to our tile hitch, see `docs/plans/tile-load-hitch.md`.
- **Look-ahead along the path, not around the player.** Above 25 m/s, tiles within 160 m of the predicted path, up to 30 s
  ahead (cap 5 km), are wanted too and built in the order the car will reach them.
- **Fewer, larger writes.** (BeamNG specific: the game watches its user folder, so file writes caused frame hitches. They
  write to a staging folder and rename.) Not needed for us, but the idea "any per-tile work that touches a global system
  every tile will eventually hitch" is the same thing as our per-tile texture/mesh uploads.
- **Bug trail.** Their CHANGELOG lists "fewer stutters" fixes repeatedly: ground photo set before the swap, spread work over
  frames with a time budget, one new forest object per frame, pre-touch kinds of props so the first one does not hitch.
  Every item is a lesson in where hitches come from.

## 10. Process lessons worth copying

1. **Benchmark set of places** [`docs/PLACES.md`, `docs/benchmarks.json`, `tools/benchmark.py`]: all continents and biomes,
   data-rich and data-poor, flat and steep, plus a held-out set not used for tuning. Our `tools/shoot_places.sh` has 6 scenes;
   grow to ~30 with fixed cameras and run on every visual change.
2. **No location-specific code.** Test cases are never special cases. Matches our "data tables, not code paths" rule.
3. **Golden-hash determinism test**: generate the same area from different anchors, paths and load orders, hash every placed
   object. We should add this to CI (Stage A item). Their rule: never depend on anchor, tile key, LOD level, load order,
   thread timing, time of day or unseeded RNG.
4. **Every change has a dated entry that quotes the report** and a test. Our `docs/CHANGES.md` already follows this;
   add the user quote and the place.
5. **Off switches on everything** (`BNE_ROADZ=off`, `BNE_GRADESEP=...`, `BNE_URGENCY=off`), matching our rule 3.
6. **Estimates must be honest.** Their bridge-clearance doc says what the check does not prove and what must be driven in
   the game before calling it a guarantee. Copy that habit.

## 11. What not to copy

- BeamNG's stock assets (tree models, window sheets, sign shapes). We need our own, which is also what makes our
  look different, as you said you want.
- The Python server architecture, DecalRoads, TSStatic, Lua extensions.
- Their physics block (a 512 m physics-only terrain following the car), which exists because BeamNG's TerrainBlock is the only
  collidable ground in that engine. We use Avian with a collider per tile.
- Anything from the stock-map-matching strategy: their goal is to look like the official BeamNG maps; ours is a different art
  direction (per your note).

## 12. Proposed order of work (and where it fits `ROADMAP.md`)

CLAUDE.md says performance and looks come before new gameplay, and the next stop is GPU measurement. This plan respects that:

**Do now, no renderer risk (Stage A).**
- Fix the tile-load hitch (`docs/plans/tile-load-hitch.md`). It is the thing you feel most.
- R0, R1: road bugs B5/B6/B7 and the road-attribute table.
- Determinism hash test in CI.
- Licence audit of EOX Sentinel-2; start a `CREDITS.md`.

**Stage B/F-lite (roads become real).**
- R2 to R5 (graph, levels, deck solver, world-space chains), then R6, R8, R9, R10.
- Imagery provider list with NAIP and Switzerland (2.1), colour match.

**Stage D/E (with the new renderer and lighting).**
- R7 tunnels (hole mask needs renderer cooperation), R12, R13.
- Overture bake pipeline and false-positive filters; CHM trees.

**Stage F (play/vehicles).**
- Road graph to traffic crate; racing line; dense traffic after GPU-driven draws.

## 13. Open questions I could not answer from the repo alone

- Which road tags OpenFreeMap really exposes (R1 answers this).
- Whether we want a baked-data repo like theirs (`bakepack`) or generate everything on demand. A bake repo is a build and
  hosting commitment.
- Licence of every dataset for a commercial release (EOX is non-commercial; Overture is ODbL/CDLA depending on theme; NAIP is
  public domain; WorldCover is CC BY 4.0). Needs a proper pass.
- How their output actually looks next to ours at street level. Running their mod needs BeamNG.drive, which we do not have in
  this environment; compare screenshots from their README or Discord instead.
