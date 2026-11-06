# Roads v2: remaining work (third pass)

Companion to `docs/plans/roads.md` (the v2 design and the second-pass follow-ups). The second pass landed
six items on `feature/roads-v2-followups` (retaining-walled embankments, Overture roof shapes, national
marking rules, OSM lane/sidewalk/bike/speed tags, inferred bridges, measured heights for France and
Norway). This document plans the items that are still open, in the order they should be tackled.

**Nothing from the second pass has run in the game yet.** Run the Mac checklist in `roads.md` (bottom) before
starting any of this; visual validation of the existing work is a higher-value use of time than more features.

## Phase 0 — Validate what exists (do this first)

Run the 8-item Mac checklist in `roads.md` on a real Mac with shadows on. Record results in `CHANGES.md` and
fold verified items into `ROADMAP.md`. Specific things to watch:

- Tile-border cracks/steps in roads and terrain (the thing v2 was built to remove).
- Four Level / Dallas High Five: deck heights, ramps meeting them, headroom.
- Tunnel drive-through (terrain collider hole in a low-arch section).
- `EW_TILE_STATS=1` for `ROADS v2` lines and triangle totals; `EW_ROADS=legacy` for before/after.
- Overture bake (`tools/bake_overture.py`) and garage drive-up (LAX, Shibuya).

Only after Phase 0 is green should new work begin.

## Phase 1 — Correctness

### 1.1 Norway deck matching (NVDB has no bridge flag)

**Problem.** `roads::measured` matches NVDB surveyed segments to bridge/tunnel edges by geometry. NVDB does
not flag bridges, so a bridge edge beside a parallel ground road can take the wrong height. Direction,
distance and a sanity band limit but do not remove this.

**Fix.** Tighten the match and add a guard:

- Require the surveyed midpoint height to be at least 1.5 m above the smoothed ground DEM at that point
  (a ground road's surveyed Z equals the ground; a deck's does not). This single check removes most
  false positives.
- Reject a match when the edge's already-solved height (from the solver) exceeds the surveyed height by
  more than 2.0 m (the deck cannot be lower than the ground it clears).
- When two edges of the same level compete for one surveyed segment, prefer the one whose bearing matches
  the segment within 10 degrees; only fall back to distance.
- Add a `measured_mismatch` counter to `Stats` (and to the `ROADS v2` tile line) so regressions show up.

**Effort.** Small (1-2 days). **Switch.** Already covered by `EW_ROADZ=0`.

**Tests.** Unit test with two parallel edges (one bridge, one ground) and a surveyed segment whose Z is
above ground: only the bridge edge matches. Property test: matching is invariant to swapping edge order.

### 1.2 Cut retaining walls — explicitly deferred

The second pass walled embankments only. A cut wall would stand where the slope starts, but the bench is
tied to the 6 m terrain grid cell (road half-width + ~9 m), so the wall would float above or sink into the
graded slope. Doing this properly needs the terrain grade to produce a flat bench for cuts, which is a
larger change to `grade()` and `TerrainGrid`.

**Decision.** Defer until the terrain grading is reworked for benches. Sidehill cuts stay graded (current
behaviour). Document as "not a bug, by design" in `roads.md`. Revisit in Phase 3 of the Nanite/terrain work
(Stage B/C), not before.

## Phase 2 — Fidelity, high value

### 2.1 US lidar heights (USGS 3DEP) — the biggest remaining item

**Why.** LA and Dallas still use inferred heights. US interchanges (Four Level, High Five) are the reference
scenes for stacked-deck correctness, so this is the highest-value data work.

**Needs.**
1. An EPT dataset index: map a lat/lon to a USGS 3DEP EPT collection URL. USGS publishes an index; a
   static table of known regional datasets is acceptable for v1, with a fallback to "no data" (inferred
   heights) outside coverage.
2. A LAZ decoder dependency. Options: the `laz` crate (pure Rust, supports LAZ 1.0-1.4) or `las` (LAS/LAZ).
   Add to `ed_data` behind a feature flag so the workspace stays fast to build offline.
3. A deck-surface picker for stacked interchanges: cluster points vertically within a 20 m band around the
   inferred deck height, pick the highest cluster with >= N points within +/-0.5 m, and treat its median as
   the deck Z. This is what distinguishes the upper deck from the lower deck and the ground.
4. Fetch via HTTP range requests on the EPT (entwine) tile index; cache under
   `<cache>/roadz/v1/usgs/` keyed by z14 tile, same as IGN/NVDB.
5. Match to bridge/tunnel edges exactly like `measured.rs` (8 m, same direction, sanity band).

**Off switch.** Covered by `EW_ROADZ=0`; add `EW_ROADZ_US=0` to disable only the US source while keeping
France/Norway.

**Effort.** Large (1-2 weeks). **Risk.** LAZ decoding and EPT range requests are fiddly; budget time for
the decoder integration. If the `laz` crate does not build on the toolchain, fall back to shelling out to
`pdal` (not preferred, adds a runtime dependency).

**Tests.** Integration test (ignored, network) fetching one LA tile and asserting >= 1 deck match with
height within 2 m of a known value. Unit test for the cluster picker on synthetic point clouds (two decks
5 m apart).

### 2.2 Bridge types beyond the beam deck

**Current state.** v2 builds every bridge as a beam deck (`deck_profile`) + hammerhead piers. The old
single-file planner (`roads.rs`, pre-v2) had a `bridge_style`/`bridge_structure` with ThroughArch, StoneArch,
Truss and Suspension, but that code is not wired into v2.

**Work.** Port the structure types into `roads::structures` so each bridge's `Bed::Bridge` item carries a
`BridgeKind` (Beam, ThroughArch, StoneArch, Truss, Suspension) chosen by `bridge_style` (flavor x class x
length x over-water, already defined in the old code). For each non-beam kind, add geometry:

- **ThroughArch:** two arch ribs (swept along a parabolic/circular curve rising above the deck) plus vertical
  hangers at 4-6 m spacing. Deck stays `deck_profile`.
- **StoneArch:** a single masonry arch (extruded curve) with spandrel walls; piers become abutments.
- **Truss:** open-web triangular members (diagonals + verticals) between two chords (top chord = parapet
  rail, bottom chord = girder).
- **Suspension:** two towers, main cables (catenary), vertical hangers, deck stiffening girder. Only for
  spans >= 150 m over water.

**Effort.** Medium-large (1 week). **Priority order.** ThroughArch and StoneArch first (most visible in
cities and historic towns), then Truss, then Suspension (rare, long spans).

**Switch.** None needed; structure is chosen by data. Add `EW_BRIDGE_BEAM=1` to force all bridges to beam
for A/B comparison.

**Tests.** Geometry non-empty and closed-mesh (winding) tests per kind, mirroring the existing
`deck_section_is_closed_and_faces_outward` test. Visual check on the Bridges lookdev board
(`--lookdev ... --only bridge`).

### 2.3 Parking lanes

**Work.** Where the road has a mapped parking tag (OSM `parking:lane` / Overture) or where the facade-
measured width leaves >= 2.5 m spare on one side, draw a narrow parking lane strip (2.0 m) along the curb
with a dashed line. Add `parking_lane_w: f32` to `MarkStyle` (default 2.0, 0.0 = disabled) and a
`parking: Option<Side>` field on the road item.

**Effort.** Small (2-3 days). **Depends on.** 2.1 is not required; OSM tags (already landed) provide
`parking:lane` in some cities.

### 2.4 Stop lines and lane arrows

**Current state.** `junction.rs` builds zebra crosswalk bars. Stop lines and lane arrows are not drawn.

**Work.**
- Stop line: a solid bar of width `MarkStyle::stop_w` across each branch of an urban junction, placed
  `stop_setback` (default 1.0 m) behind the crosswalk (or at the stop point if no crosswalk). Add to
  `junction::crosswalk_bars`'s caller or a new `junction::stop_lines`.
- Lane arrows: chevron/triangle decals on the asphalt for through/left/right lanes on approaches to
  junctions with 3+ branches. Place at `arrow_setback` (default 8 m) before the junction, one per lane,
  rotated to the lane direction. Geometry: a small filled triangle or two chevrons (~1.5 m wide).
- Add `arrow_setback: f32` and `stop_setback: f32` to `MarkStyle`.

**Effort.** Medium (3-5 days). **Tests.** Unit test that a 4-way junction produces 4 stop lines and that
arrow count matches lane count.

## Phase 3 — Fidelity, medium

### 3.1 Tunnel lights

Emissive strips on the tunnel ceiling (a thin bright quad or a line of small emissive boxes) plus, in the
native renderer, a row of point lights. Bevy path: emissive material only (no real lights, matches current
cost). Native path: add point lights to the tunnel light list uploaded to `WorldRenderer`.

**Effort.** Small (mesh) + medium (native light list). **Switch.** `EW_TUNNEL_LIGHTS=0`.

### 3.2 Detailed portals

Current portal is a concrete lintel + two cheek walls. Add: approach pavement (a short paved flare), a
signage board (a flat quad with a road number), and (optional) a light fixture. Keep the boxy style.

**Effort.** Small-medium (2-3 days).

### 3.3 Garage skins and stair towers

Garages are currently slab/column/ramp. Add an exterior skin (wall panels with openings for the ramp and
pedestrian doors) and a stair tower at one corner. Affects `garage.rs`.

**Effort.** Medium (3-5 days).

### 3.4 Overture roof levels and part roof shapes

`buildings::roof_hint` already maps Overture `roof:shape`. Two gaps:
- `roof:levels` (number of roof storeys) is ignored; apply it to scale the roof height.
- Building parts (towers, annexes) carry their own `roof:shape` in Overture parts; the part generator
  uses the parent's roof. Pass the part's shape through.

**Effort.** Small (1-2 days). **Depends on.** Overture bake being run (Phase 0).

## Phase 4 — Gameplay (deferred to Stage F)

### 4.1 Traffic and AI drivers

Only `NavTile`/`TileNav` graph data exists. Full traffic needs: vehicle spawning on nav edges, lane
following with speed limits, traffic signals at junctions, intersection priority, collision avoidance,
despawning. This is Stage F work and should not start until the renderer (Stage A/B) is solid; it depends
on the vehicle physics (Stage F) which is itself deferred.

**Decision.** Keep as graph-data-only until Stage F. No new work here now. Document the interface
(`NavTile` -> spawn request) so Stage F can pick it up.

## Phase 5 — Broader project (not roads)

These are the actual Stage A priorities per `ROADMAP.md` and should run in parallel with the roads work
above, not after it:

- Mac test pass for M0+M1 (fps, hitches, forests, impostors, building explode, colliders).
- GPU measurement: baseline table on 6 reference scenes, per-pass GPU timers, hypothesis verdicts.
- Perf gate script (`tools/perf_gate.sh`).
- Determinism golden-hash CI + pinned data versions.
- M0 leftovers: pipeline pre-warm (confirm AO/TAA cause first).
- Art direction round 2 (lookdev boards).

## Recommended order (commit-sized chunks)

1. Phase 0 (Mac validation of existing roads work).
2. 1.1 Norway matching fix.
3. 2.2 ThroughArch + StoneArch (highest visual payoff for effort).
4. 2.4 Stop lines + arrows.
5. 2.3 Parking lanes.
6. 3.4 Overture roof levels.
7. 2.1 US lidar (start the EPT/decoder spike early; it has the longest lead time and unknowns).
8. 2.2 Truss + Suspension.
9. 3.1-3.3 tunnel lights, portals, garage skins.
10. Phase 5 in parallel throughout.

## Off switches (cumulative)

`EW_ROADS=legacy`, `EW_OVERTURE=0`, `EW_OSMTAGS=0`, `EW_INFER_BRIDGES=0`, `EW_ROADZ=0`,
`EW_ROADZ_US=0` (new), `EW_BRIDGE_BEAM=1` (new), `EW_TUNNEL_LIGHTS=0` (new).

## Definition of done per item

- Unit tests in the engine-free crate pass (`cargo test -p ed_world`).
- `cargo check --offline -p ed_game` passes.
- A dated `CHANGES.md` entry with what changed and what needs a Mac check.
- `ROADMAP.md` updated if the item changes stage exit criteria.
- Visual check on at least one reference scene (Zermatt, Central Park, Shibuya, LA, Dallas) noted in
  the CHANGES entry.