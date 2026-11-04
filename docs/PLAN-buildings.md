# Plan: building roofs and foundations

Status: IMPLEMENTED in `buildings.rs` (+ `meshkit::inset` clamp 0.3 -> 0.5), tests pass. NOT yet eyeballed in-game: run `tools/shoot_places.sh` (Zermatt, Aix, Central Park) to check.

Done: dense edge ground sampling + base_h = lowest - 0.5; per-wall `Wall.ground` profile + `foundation()` concrete skin; unsupported `upper` parts grounded in `prepare()`; `roof_plan()` (shared by detail + LOD): flat if holes/ring>24/floors>4 (pitched)/area>350 house, 500 other/short span>14; mansard gated; `roof_rects()` raster rectangle cover for L/T houses, flat if uncovered >5%; `pitched_rect` rise cap 4.5, hip ridge >= 0.3*half-span.
Not done: lookdev sloped test scene + gap debug colour (tests cover slopes instead); thresholds are not yet per-region in archkit; open LOD question below.

Original plan follows. Code lives in `crates/ed_world/src/buildings.rs` (+ `meshkit.rs`, `archkit.rs`).

## Problems
1. **Spiky roofs.** `pitched()` non-rect fallback insets the eave ring ~95% of the half-span with `meshkit::inset` (naive miter, clamp 1/0.3): spikes on sharp/concave corners, self-intersection on L/thin shapes. Square OBB -> single-apex pyramid; steep pitch (mansard 1.0) makes it tall. `lod_one` repeats the hip-by-inset.
2. **Roof piece sticking out beside a house.** `pitched()` roofs the footprint's bounding rectangle when `rectangularity > 0.82`; an L-shaped house missing up to 18% of its box gets a full-rectangle roof, so the missing corner is a roof hanging past the walls.
3. **Big buildings with triangle roofs.** Flat is only forced for floors > 8, ring > 24 vertices or holes. Wide low buildings (warehouses, offices) get a giant gable.
4. **Floating.** `prepare_one`: `base_h = g_min - 0.4` from ring-vertex samples only. No edge/interior sampling, no skirt. Terrain may render at a coarser LOD than the grid used here. `upper` parts (`min_height > 0.5`) have no support.

## Decisions (user-approved direction)
- Roof choice: single gable/hip only when footprint is nearly its bounding rectangle (~0.95+). Otherwise split into rectangles with joined roofs (L-shaped house), else flat. No straight-skeleton crate dependency.
- Size rule -> flat roof (parapet, rooftop gear) if footprint area above per-region threshold (~400-600 m2, in `archkit`), OR span > ~14 m, OR > 4 floors, OR non-house variant. Big buildings are offices/commercial/industrial.
- Safety net: any roof failing sanity checks (rise > limit, degenerate/inverted faces) falls back to flat.
- Same decision in `lod_one` so the roof shape does not change with distance.
- Foundations: concrete ring from the footprint, extending below the lowest ground sample (sampled along every edge at ~1 m and at interior points) + margin ~0.3 m. Visible stepped plinth on slopes, buried/short on flat ground. Nobody can see under a house. Automatic for every building; material/height per region. Collider matches.
- `upper` parts: extend walls down to a supporting building, or drop if none.

## Order of work
1. Test scene: sloped noisy `TerrainGrid` + awkward footprints (L, U, thin, wide, triangle); debug color for "gap under building". (`lookdev.rs` currently uses a flat grid, which hides terrain bugs.)
2. Foundations.
3. Roof rules: strict shape test, size rule, flat fallback, clamp `meshkit::inset`.
4. Rectangle-split roofs for L-shaped houses.
5. Property tests (roof height <= span*pitch, no NaN, no inverted normals, foundation bottom <= lowest ground) + snapshots at Zermatt, Aix, Central Park via `tools/shoot_places.sh`.

## Open question
Does the terrain render at a coarser LOD than the `TerrainGrid` used for buildings? If so, foundation depth must cover the difference (or sample the rendered LOD).
