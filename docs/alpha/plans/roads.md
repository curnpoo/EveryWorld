# Roads v2: world-space network, solved heights, decks, tunnels, junctions, nav graph

Status: IMPLEMENTED 2026-11-06 on `feature/roads-v2`, tested on Linux (unit and property tests, real-data runs on
six tiles). **Not seen in the game yet.** The Mac checklist is at the bottom. Inspired by BeamNG.Earth (MIT,
`docs/plans/beamng-earth-lessons.md` has the comparison); ideas, not code, were taken.

`EW_ROADS=legacy` returns to the old per-tile planner. Everything else below is on by default.

## What changed, in one paragraph

Roads used to be planned one tile at a time: a bridge was a straight line between two ground heights, tunnels were
dropped, ramps were draped on the ground, and a road crossing a tile border got two unrelated profiles. Now the
road network of the 3x3 block of vector tiles around a tile is assembled in world space, every road gets a level
(ground, bridge, tunnel), one solver picks all heights together (decks clear what runs under them, approaches
climb onto fill at a legal grade, tunnel floors run under hills), the terrain is graded to those heights, and decks,
tunnels, junctions, medians and garages are real geometry with colliders.

## Modules (`crates/ed_world/src/roads/`, engine-free, tested)

| File | Role |
|---|---|
| `mod.rs` | The old planner (`plan`, kept behind `EW_ROADS=legacy`), `grade`, `generate`, profiles, markings. `build_v2` assembles net + ground + solve |
| `net.rs` | `RoadNet::from_tiles`: each source tile's roads clipped to its own square (not the 64-unit buffer), pieces that meet on a border joined into one `Edge` (class, layer, bridge/tunnel/ramp/oneway/paved/cycleway). Also building outlines for widths and keep-out |
| `ground.rs` | Raw elevation of the 3x3 block as one function of position, and `smooth_ground` (centre + ring of 8 at 12 m). Raw data only, so the answer never depends on which tile asked |
| `solve.rs` | The height solver (below), crossing detection, tunnel sections |
| `plan2.rs` | Solved network -> `RoadPlan`: items clipped to the tile plus a 40 m halo that is graded but not drawn; junction specs; median runs; `carve_holes` |
| `structures.rs` | Closed deck section (parapets, curbs, girder underside), tunnel tube, hammerhead piers, portals, jersey barrier, `sweep_raw` |
| `junction.rs` | Curb-return fillet geometry (tangent distance `R/tan(theta/2)`), crosswalk bars |
| `width.rs` | Street width from the buildings that line it (rays to facades, median of face-to-face distances) |
| `median.rs` | Barrier runs between opposite one-way motorway/trunk carriageways, gap under 18 m |
| `keepout.rs` | Drops building footprints whose centre is inside a carriageway or on a deck |
| `marks.rs` / `osm.rs` / `infer.rs` / `measured.rs` | National marking rules; OSM tag matcher; inferred bridges; surveyed heights (second pass, below) |
| `nav.rs` | `NavTile`: nodes (stable ids from world position), edges (lanes, speed, width, level, centreline), signals and stops |

Also: `garage.rs` (drivable parking structures), `ed_data/src/overture.rs` (baked Overture buildings as a `building`
layer), `tools/bake_overture.py`, `TerrainGrid.holes`, `TileBuild::{road_collider, nav}`, and in `ed_game` the road
collider spawn and a `TileNav` component on each tile entity.

## The solver (`solve.rs`)

Every in-graph road (paved ground roads of rank 1+, every bridge, every tunnel) is resampled to 4 m.
- **Target** per sample: ground roads follow the smoothed ground (2D low-pass, then a light 1D smooth); decks take a
  chord between their abutments; tunnels take the lower convex hull of the ground.
- **Lower bounds**: at each crossing of a higher road over a lower one with no junction between them (angle over 8
  degrees, and not within 160 m of a shared node, which is a ramp forking off), the upper road is held at
  `lower height + 6.0 m headroom (6.5 over rail) + deck depth (1.5 m)` over the crossing's footprint; decks never
  come closer than 0.7 m to the ground (eased in from the abutments so the floor does not demand a steep approach).
- **Fixpoint**: heights are the lowest values at or above target and bound such that neighbouring samples never
  differ faster than the class grade (motorway 6 %, primary 7 % ... path 12 %), across junctions too. Natural ground
  slope steeper than the cap is allowed through, so a mountain road is not turned into a giant fill. This is a
  max-plus relaxation on a priority queue; raising is the only operation, so a short overpass lifts its own approaches.
- **Rounds**: three, because the upper road's bound uses the lower road's solved height. Between rounds the raised
  samples are smoothed (vertical curves) and the bounds re-applied.
- **Locality**: influence is `height difference / grade`, a few hundred metres, so tiles that share a halo compute the
  same heights at the same place (tested: translating the network gives the same answer; two centres give the same
  joined road).
- **Audit**: `Stats.deficits` lists crossings under 5.9 m. It reports; it never steepens an approach to fake success.

## Terrain

`grade()` now takes every non-bridge, non-tunnel item including the halo items, uses each item's own width, and
`TerrainGrid.holes` removes terrain-collider quads over low-arch tunnel sections (`plan2::carve_holes`) so a car can
drive through shallow cover. The render mesh is untouched (the tube mesh is what you see and hit inside).
Decks, tunnels, medians and garages go into `TileBuild::road_collider` (trimesh, spawned with the building shell).

## Real-data results (Linux, release build, this repo's reference tiles)

Road triangles per z14 tile (legacy before the diet, then v2 with the diet) and solver statistics:

| Tile | Legacy before | v2 now | Bridges / tunnels / crossings / deficits |
|---|---|---|---|
| Central Park | 452k | 281k | 183 / 277 / 243 / 0 |
| Zermatt | 137k | 64k | 66 / 21 / 3 / 0 |
| Shibuya | 485k | 388k | 809 / 885 / 1396 / 22 |
| LA Four Level | 659k | 502k | 579 / 237 / 957 / 13 (worst 5.7 m) |
| Dallas High Five | 353k | 331k | 272 / 25 / 496 / 7 |
| Zurich Hardbruecke | 560k | 486k | 610 / 974 / 335 / 1 |

The diet: flat sweeps no longer subdivide strips (graded roads are level across), paths drop the soft edge strips and
curtains and simplify at 9 m, guardrails use half the samples and a 5-edge rail. Footpaths were 55-80 % of road
triangles in a park or village (110 km of paths in Central Park). Remaining weight: markings (12-58k per tile) and
motorways. v2 costs about 0.25-0.7 s of worker time on a dense tile for the solve and 0.1-0.3 s for planning, plus nine
vector tiles and nine elevation tiles instead of one (prefetched by the IO stage).
A plan-view plot of the LA tile shows the Four Level Interchange raised to about 20 m with ramps coming back to the
ground (`EW_DUMP=out.json EW_LL=lat,lon cargo test -p ed_world dump_roads -- --ignored`).

## Buildings, widths, garages

- `tools/bake_overture.py` bakes Overture buildings and parts (pinned release `2026-10-21.0`) to
  `<cache>/overture/<release>/14/x/y.json`; `ed_data::overture` reads them as a `building` layer, so the building
  generator needs no second path. `has_parts` becomes `hide_3d`. No file for a tile means OpenFreeMap buildings.
  `EW_OVERTURE=0` ignores the baked files. Needs `pip install duckdb`; the script ignores `AWS_*` variables on purpose.
- Machine-learned footprints on roads are dropped by `keepout.rs`, for every source.
- Street width from building facades replaces the class default where both sides are walled in agreement.
- Overture `class = parking` footprints of 400 m2 or more become garages: storeys of slab, belt beam, columns, a ramp per
  storey through a hole in the next slab (rectangular footprints), colliders.

## Off switches and knobs

`EW_ROADS=legacy` (old planner), `EW_OVERTURE=0` (OpenFreeMap buildings only), `EW_OSMTAGS=0` (ignore baked OSM tags), `EW_INFER_BRIDGES=0`, `EW_ROADZ=0` (no surveyed heights), `EW_TILE_STATS=1` (prints one
`ROADS v2` line per tile: edges, samples, bridges, tunnels, crossings, deficits, max fill and cut),
`EW_ROAD_TALLY=1` (triangles by kind and class, stage timings).

## Follow-ups (second pass, 2026-11-06, one commit each)

- **Retaining walls** (`Bed::Fill`, `plan2::mark_walled`): a ground road at least 2.5 m above the natural ground for 24 m
  or more, with no hillside reaching road level within road half width + 3.5 m on either side, becomes a solid section
  (parapet, curb, asphalt, walls down into the ground; depth = fill + 0.8 m, at most 14 m) with its own collider, and the
  terrain is not graded under it. Decided from the position-based smoothed ground so neighbouring tiles agree. Sidehill
  cuts stay graded (a wall in a cut would stand where the slope starts: the bench is road half width + ~9 m because it
  is tied to the 6 m grid cell).
- **Roof shapes** (`buildings.rs` `roof_hint`): Overture `roof:shape` (flat, gabled, hipped, pyramidal, mansard,
  saltbox, gambrel, half-hipped) overrides the regional roof table, even in regions whose table is all flat roofs; a mapped
  pitched roof may be up to twice the usual size and 6 storeys. Shapes without a counterpart (dome, onion, skillion, cone)
  keep the regional default.
- **National marking rules** (`roads/marks.rs`): bounding-box country detection and a `MarkStyle` per country from the
  national manuals (values and sources are in the file): centre colour and dash by town or country, lane lines between
  lanes, edge widths, dashed edges (France, Netherlands), yellow edges (Ireland), yellow left edge on one-way
  carriageways (Americas), double centre lines (US, Canada), no centre line under the manual's minimum width, crosswalk bar
  width and gap. Spain, Italy, Korea and the rest use the neutral default; China and Latin America are marked UNVERIFIED
  in the table. Country boxes are approximate within a few tens of km of a border.
- **OSM lane, sidewalk, bike-lane and speed tags** (`tools/bake_osm_tags.py` from Geofabrik Postpass,
  `ed_data::osmtags`, `roads::osm`): baked per z14 tile under `<cache>/osmtags/v1/` (bake the 3x3 block, `--radius 1`,
  the default). Ways are matched to edges by geometry (votes every 10 m within 8 m, class check, at least half the samples);
  left and right are swapped when the edge runs against the way. Mapped `width`, else lanes x lane width, else the
  facade-measured width, else the class default; sidewalks per side (a side without one gets a shoulder, a street with
  mapped sidewalks outside town gets the town profile); painted cycle lanes in the national colour (green in the US,
  red-brown elsewhere) with a white line; lane counts drive the lane lines; the nav graph takes lanes and speed limits.
  `EW_OSMTAGS=0` ignores them. Shibuya: 2054 of 16.5k edges carry tags (most OSM ways have none). Loading nine baked tiles
  of a dense city costs about 0.3 s of worker time; parsing could be cached. Parking lanes are not drawn.
- **Inferred bridges** (`roads/infer.rs`): two ground-level roads (or a road and a railway) that cross with no junction
  within 15 m of the crossing, at least 25 degrees apart, where the median ground 40 m and 80 m out on either side along
  one line is at least 4 m above the other's and that line's ground 40 m out is on both sides above the crossing, get a
  bridge on the upper one (span = lower width / (2 sin angle) + 20 m, 25-90 m). The edge is split and the middle piece
  becomes a bridge for the solver. Conservative: 0 to 7 spans on the reference tiles, 10-40 ms. `EW_INFER_BRIDGES=0` off.
- **Measured heights** (`ed_data::roadz`, `roads::measured`): France (IGN BD TOPO: Z per vertex, bridge/tunnel position,
  lanes and width) and Norway (NVDB: Z per vertex, no bridge flag). Segments are fetched per z14 tile, cached under
  `roadz/v1/<source>/`, matched to bridge and tunnel edges (8 m, same direction, same level where flagged, datum brought
  onto our DEM by the median offset), cleaned (gaps filled, median-of-3, 12 % grade limit) and used as the edge's target.
  A surveyed deck is exempt from the headroom requirement and its approaches are lifted to meet it. Paris tile: 133 of
  1663 decks and tunnels matched; Oslo: 349 of 660; Lyon: 208 of 1177. France's lanes and width fill untagged edges.
  `EW_ROADZ=0` off. NVDB cannot tell a deck from the road under it, so a bridge edge beside a parallel ground road could
  take the wrong height (direction, distance and a sanity band against the ground limit this, not eliminate it).
  **US lidar (USGS 3DEP) is not implemented**: it needs an EPT dataset index and a LAZ decoder dependency, and a
  deck-surface picker for stacked decks; it is the next step for US interchanges.

## Not done (honest list)

- **Cut retaining walls** (see above), **bridge types** beyond the beam deck with piers (truss, arch, suspension,
  cable-stayed), **US lidar heights**, **parking lanes**, the national **stop-line and arrow markings**, **tunnel
  lights**, detailed portals, garage skins and stair towers.
- **Traffic and AI drivers**: only the graph data exists (`NavTile`, `TileNav`).
- Overture roof levels and building parts' own roof shapes beyond the hint.

## Mac checklist (nothing here has run in the game)

1. Default run at Zermatt, Central Park, Shibuya, then LA (34.0561, -118.2358) and Dallas (32.9056, -96.7693): do roads
   sit on the ground, do tile borders show cracks or steps in roads or terrain (the thing v2 was built to remove)?
2. Fly over the Four Level Interchange: decks at different heights, ramps meeting them, headroom under each deck.
3. Drive onto and under an overpass: deck collider holds a car, nothing collides with the terrain under the deck.
4. A tunnel (Zermatt area, Central Park transverse roads): tube visible, portals, a car can drive through a low-arch
   section (terrain collider hole), no hole in the visible terrain.
5. `EW_ROADS=legacy` for before and after; `EW_TILE_STATS=1` for the `ROADS v2` lines and tile triangle totals.
6. Hitch check: v2 adds worker CPU, not main-thread work, but the road meshes are now different sizes; compare with
   `docs/plans/tile-load-hitch.md`.
7. Overture: `python3 tools/bake_overture.py --lat .. --lon .. --radius 1 --cache .earthdrop-cache`, then compare
   building counts and look for buildings that should no longer sit on roads.
8. Garages (LAX 33.9425, -118.4081 has six, Shibuya four after baking): can you drive up the ramps?
