# W4e — Street data

Verdict: **pass. The existing roads v2 solver runs on the Overture graph, and the graph is
better connected than the vector-tile input.** Date 2026-11-10.
Code: `spikes/streets/extract.py`, `crates/ed_world/tests/overture_streets.rs`.

## Extract

Overture release **2026-10-21.0** (the release the alpha's building bake already pins),
transportation theme, read from the public bucket with DuckDB, for the 3 x 3 block of
zoom-14 tiles around Shibuya (35.5 km2).

| Measure | Value |
|---|---|
| Query time (cold, over the network) | 67 s |
| Segments returned | 22,332 (21,987 roads) |
| Extract size | 7.6 MiB as JSON; 213 KiB per km2. A binary chunk will be several times smaller; this is the dense-city upper end |

Attributes present on road segments in this block:

| Attribute | Share of segments |
|---|---|
| Class | 100% (16 classes, from motorway to steps) |
| Surface | 40% |
| Subclass rules (sidewalk, crosswalk, link, alley...) | 30% |
| Access restrictions (includes one-way) | 20% |
| Names | 17% |
| Speed limits | 14% |
| Road flags (bridge, tunnel, link) | 9% |
| Level rules | 7% |
| Width rules | 1% |
| **Lane count** | **not in the schema** |

Two attributes Overture has that the vector tiles do not: flags and levels that apply to
**part of a segment** (a linear-reference range), and **connectors**, the explicit points
where segments join.

## Solver run

Both inputs went through `RoadNet::from_tiles` and `solve::solve_with` unchanged, on the same
elevation fixtures.

| Input | Edges | Centreline | Bridges | Tunnels | Crossings | Clearance deficits | Endpoints in centre tile | of which dangling | Solve time |
|---|---|---|---|---|---|---|---|---|---|
| Vector tiles | 16,522 | 1,744 km | 809 | 885 | 1,396 | 19 | 2,955 | 1,287 (44%) | 0.28 s |
| Overture | 21,991 | 1,576 km | 632 | 984 | 1,170 | 51 | 4,161 | 286 (7%) | 0.29 s |

"Dangling" means no other edge has a vertex within 0.5 m of the endpoint. Some are real dead
ends, so the number is an upper bound on breaks, but the comparison is like for like.

Reading:

- **Continuity:** dangling endpoints fall from 44% to 7%. Vector tiles simplify geometry per
  tile, so neighbours stop meeting; Overture segments are whole.
- **Bridge and tunnel counts differ** because a vector tile splits one bridge into a piece
  per tile and per zoom-simplified part; Overture counts are per segment.
- **Clearance deficits rise from 19 to 51.** The spike applies a segment's bridge, tunnel
  and level flags to the whole segment and ignores the part-of-segment ranges. A segment
  that is a bridge for 30 m of its 200 m is therefore lifted along its whole length, or not
  at all. Splitting segments at the rule ranges is the fix and is the first ingest task.

## Decisions

| Item | Decision |
|---|---|
| Road graph source | Overture transportation, release 2026-10-21.0 for data version 1 (F8 confirmed) |
| Lanes, turn lanes, sidewalks as tags, lighting, kerbs | From raw OSM tags joined by OSM way id (Overture `sources` carries it), as `plans/streets.md` says. Overture alone cannot supply lanes |
| Junctions | Use Overture connectors as the junction nodes instead of matching endpoints by distance |
| Partial-segment rules | Split segments at every rule boundary during ingest, before the solver |
| Solver | Kept. Its input changes from per-tile clipped lines to whole segments in node space; `from_tiles` clipping is removed in Phase 3 |
| Failure branch "Overture loses information" | Not taken. The only loss is lanes, which the plan already sources from OSM |
