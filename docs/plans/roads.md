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

`EW_ROADS=legacy` (old planner), `EW_OVERTURE=0` (OpenFreeMap buildings only), `EW_TILE_STATS=1` (prints one
`ROADS v2` line per tile: edges, samples, bridges, tunnels, crossings, deficits, max fill and cut),
`EW_ROAD_TALLY=1` (triangles by kind and class, stage timings).

## Not done (honest list)

- **Retaining walls** along cuts and fills. The bench width is tied to the grid cell (about road half width + 9 m), so a
  wall would stand where the slope starts; needs a finer road-local mesh first.
- **Inferred bridges** where the map has none (BeamNG.Earth's `gradesep`, off by default there too for cost).
- **Bridge types** beyond the beam deck with piers (truss, arch, suspension, cable-stayed).
- **Measured heights** (lidar, national road databases).
- **Lanes, sidewalks and parking from OSM tags** (the vector tiles do not carry them); sidewalks are still the urban
  profile; bike lanes are only the coloured cycleway paths.
- **Markings** are the old set with per-region dash lengths and a yellow left edge on one-way carriageways in the
  Americas, not the full national-manual tables.
- **Tunnel lights**, detailed portals. Garage skins (one concrete look), stair towers.
- **Traffic and AI drivers**: only the graph data exists (`NavTile`, `TileNav`).
- Roof shape and levels from Overture are in the props (`roof:shape`) but the building generator does not read them yet.

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
