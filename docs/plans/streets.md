# Streets — from source data to driveable roads

Design brief. Decision F8 in `../DECISIONS.md`. Data rules in `../data/README.md`. Built in
Phase 3 on the alpha's roads v2 solver (`../alpha/plans/roads.md`), which stays as the
reference for the height and junction maths.

## Goals

1. The engine understands a street the way a driver does: lanes, direction, sidewalks,
   junction shape, bridges and tunnels, not a centre line with a class.
2. Roads are continuous across node edges, with no gaps or doubled geometry.
3. The same plan feeds rendering, collision, vehicle and pedestrian navigation, building
   orientation and the far-field rungs.
4. The solarpunk transformation of streets is a rule table, not code.

## 1. Why the source changes

The alpha read roads from OpenMapTiles vector tiles. Those are made for drawing maps: lines
are clipped at tile edges, simplified per zoom, and most tags are dropped (lanes, sidewalks,
widths, turn lanes). Stitching clipped lines back together is where many of the alpha's road
gaps came from.

## 2. Sources

| Source | Gives | Licence |
|---|---|---|
| **Overture transportation** (pinned release, GeoParquet) | Segments and connectors forming a routable graph with stable ids; class and subclass; rules that apply along part of a segment (speed, access, surface, width, level, flags such as bridge and tunnel) | ODbL |
| **Raw OSM tags**, joined by OSM id | The long tail: `lanes`, `turn:lanes`, `sidewalk`, `cycleway`, `parking:*`, `lit`, `layer`, `surface` | ODbL |
| National datasets where open (for example road heights) | Deck heights for bridges | Per source, in the registry |
| Elevation | Ground height under roads | See `../data/README.md` |

Overture is read with DuckDB in the bake tools; the runtime never parses GeoParquet.

## 3. Pipeline

```
ingest → normalise graph → infer cross-sections → solve junctions → solve heights
       → emit (per node, per rung) → bake chunks
```

1. **Ingest** a region from the pinned Overture release; join OSM tags. Output: a compact
   regional graph (segments, connectors, rules), stored as a source-extract chunk.
2. **Normalise.** Split segments at rule changes so each piece has one cross-section. Assign
   every segment to the node that contains its anchor point (`world.md` §1.4).
3. **Infer cross-sections** from a rule table keyed by road class, known lane count,
   one-way, region and urban density: lane widths, sidewalks, cycle tracks, medians, parking,
   verges, tree lines. Real tags override the table. The solarpunk street (fewer car lanes,
   planted medians, tram tracks, permeable surfaces) is a second table applied on top.
4. **Solve junctions.** At each connector: ordered incident segments, corner radii, stop
   lines, crossings, lane connectivity (which lane can reach which).
5. **Solve heights.** Levels and bridge/tunnel flags give the stacking order; the solver
   fits deck heights to terrain with grade and clearance limits. Terrain is patched under
   at-grade roads so nothing floats.
6. **Emit**, per node:
   - R0: road surface patches, markings, curbs, prop anchors, collision, the **lane-level
     navigation graph**, and **parcels** (the polygons enclosed by roads).
   - R1/R2: a simplified ribbon in the node proxy.
   - R3: road albedo and emissive (street lighting) in terrain layers.
7. **Bake** to chunks stamped with the Overture release and rule-table version.

Parcels matter beyond roads: they orient building facades toward the street, define where
procedural fill may place buildings when footprints are missing, and are the unit a
`replace` or `suppress` override often targets.

## 4. Determinism and edits

- Every step is a pure function of (pinned data, rule tables, override layers); golden
  hashes cover the graph, cross-sections and heights separately.
- `road_patch` overrides (`world.md` §4.2) are applied after normalisation and before
  inference, so an authored road gets junctions and heights like any other.
- Segment ids are Overture GERS ids, so an override survives a data update when the segment
  still exists, and the bake reports overrides whose targets disappeared.

## 5. Implementation plan

| Phase | Work | Proof |
|---|---|---|
| 0 | Spike: one city from Overture through DuckDB into the roads v2 input format; count what tags are gained over vector tiles; licence posture for baked chunks recorded | Comparison note with numbers; registry entries |
| 3 | Ingest and normalise in the bake tools; rule tables; junction and height solvers ported to node space; nav graph and parcels; all rungs | Drive three reference routes with no gaps; nav graph routes a car across a city |
| 5 | Vehicle AI and traffic use the nav graph; surface types feed tyre grip | Traffic follows lanes and signals |

## 6. Open questions

1. How to infer lane counts where OSM has none and Overture gives only class (start with the
   rule table; consider width from imagery later).
2. Whether baked road chunks are a derivative database under ODbL and what that requires
   when shipped (legal review; tracked in `../data/README.md`).
