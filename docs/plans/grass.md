# Grass and ground colour, near to orbit

Status: plan (2026-11-14). Owner direction, steps and gates. Builds on grass style E
(`EW_GRASS_STYLE=e`, `CHANGES.md` 2026-11-13) and trees (`trees.md`). Data rules:
`../data/README.md`. Decisions it follows: P2-D9 (ground material near, imagery far, blend
by pixel footprint), F2 (any rung straight from source), phase-2 §7.1 (imagery colours the
far field and calibrates near ground colour).

## 0. Owner direction

- Ghost-of-Tsushima grass: dense, lush, all grass types (2026-11-12).
- "The tall blades are winners for sure. But it needs to have much further distance." The
  flat shell carpet loses (2026-11-13).
- "I can see on Zermatt where the grass ends ... far away grass looks grid like and thinned
  out. I thought it would go much further and it widens and looks thicker farther away."
- "Can we not make grass colour basically to orbit? Grass colour is seen from space."
- "Real land cover would also help with grass colour, ground colour, and this cut-off at
  2 km, because the sat data would feed the world data."

## 1. What is wrong today, and why

| Symptom | Cause |
|---|---|
| Far blades look like a thinning grid | Clumps thin on a stratified rank lattice (`ed_world/src/grass.rs` `clump_rank`/`rank_step`); far survivors keep 1-3 blades whose widening is capped (`EW_GRASS_WIDTH` 5, `EW_GRASS_GROW` 0.25), so coverage per square metre falls with distance and the lattice shows |
| A visible end to the blades | Range is a fixed 500 m with a height fade, not tied to pixel size; the ground under it is not the colour of the grass it replaces |
| Colour cut-off at 2 km | The grass type and carpet colour come from a camera-centred map (`MAP_COARSE_M` 16 m x 256 = +-2 km). Any camera-centred map has an edge |
| Zermatt meadow reads as short lawn | Real land cover never reaches the grass: the type comes from the lab's test map only, so every real place falls back to lawn |

The last two have one fix: the grass type and its colour must live **in the world data, per
terrain node, at every level**, like heights and canopy pages already do. Then every rung,
from a blade at your feet to a pixel seen from orbit, reads the same answer, and there is no
edge anywhere.

## 2. The design: one ladder from source data to every rung

```
WorldCover (10 m classes)  ─┐
climate (WorldClim)         ├─>  ground page per node  ─>  grass type weights  ─┬─> blades (near)
altitude, slope, aspect    ─┘    (shares, all levels)      (rule table, data)   ├─> carpet colour (mid)
                                                                                └─> far albedo
Sentinel-2 composite  ───────────>  colour page per node (hue, brightness)  ───────┘   (calibrates all three)
```

### 2.1 Ground page (new, `ed_data::node::ground`)

Same 257 x 257 lattice as the height and canopy pages (`ed_geo` `PAGE_N`), built the same
way as `node_canopy` (`ed_data/src/node/canopy.rs`): from WorldCover through the existing
COG range reads and disk cache, on the global lat/lon cell grid, so neighbours agree on shared
samples and any level can be built straight from source (F2); parents are area means. WorldCover
is GREEN (CC BY 4.0) and already read for canopy, so this needs no new download.

Four bytes per sample, all **shares** (so parents average correctly; a class id would not):
herbaceous (grass), cropland, shrub/heath, wet (wetland plus moss/lichen). Bare is what is left
after trees, these four, snow and water. Climate is low-frequency and stays a per-node lookup
(`ed_data` `climate`), not a page channel.

### 2.2 Grass type weights (rule table, `ed_world::grass`)

A data table, not code branches: each `GrassType` row gets a niche (land-cover share it
needs, temperature and precipitation range, altitude band, slope limit). For a sample the
generator computes weights over rows, deterministic per sample. First rows: lowland meadow,
alpine meadow, pasture (short), cropland cereal, other crops, wetland sedge and rush, dry
steppe tufts, sparse desert tufts, tundra moss and lichen. The lab keeps its test map behind
`grasslab` only.

Blade scatter (`scatter_node`) reads the weights from the node's ground page. The terrain shader
reads the same pages for the carpet colour at every distance; the camera-centred tint map is
removed.

### 2.3 Colour from imagery (Sentinel-2 composite)

Phase-2 §7.1 step 6, partly pulled forward. A per-node colour page (low resolution: hue and
brightness, not a photo) baked from the composite.
- **Far** (from aircraft height up, data rule 5): it is the ground albedo, blended in by pixel
  footprint (P2-D9).
- **Near:** it calibrates each grass type's colour toward the real place, so blades, carpet and
  orbit agree: dry yellow steppe, green Swiss pasture, red clay.

Imagery never becomes the ground you stand on.

### 2.4 Far blades: thicker, not thinner

Goal: covered area per square metre stays constant with distance. As clumps thin, survivors
widen and densify into tufts sized to the pixel. Survivors are chosen by a jittered, blue-noise
rank, so no lattice shows. The blade range follows pixel size (blades end where a tuft would be
under about 1 px), not a fixed 500 m. Under the blades, the ground takes the field's mean blade
colour, so thinning never shows bare soil.

## 3. Steps and gates

| Step | Work | Who | Gate |
|---|---|---|---|
| G1 | Far blades thicker: coverage kept, blue-noise survivors, pixel-size range, field-colour floor | Opus (look) | Lab and Zermatt eye views: no grid, no visible blade end; gpu_frame no worse than style E today |
| G2 | Ground pages from WorldCover, every level, parent means, streamed beside canopy pages | Sonnet (data, plumbing) | Border test: neighbour and parent edges bit-identical over random nodes (as the height-page border test); Zermatt pages show meadow where photos do |
| G3 | Grass type rule table plus scatter from ground pages; the new rows tried in the lab | Sonnet (table, plumbing), Opus (rows, colours) | Zermatt valley meadow shows tall meadow blades; the owner judges each new type in the lab |
| G4 | Carpet colour from ground pages at every rung; camera tint map removed | Opus | A 30 m -> 1 km -> 10 km -> orbit sweep shows no colour edge |
| G5 | Imagery: registry entry, then the owner's OK on source and download size; bake colour pages; far blend; near calibration | Sonnet (bake), Opus (blend) | 10 km and orbit match the composite's colour; ground level keeps its character |
| G6 | Erosion gullies softened under grass on gentle ground (owner's call) | Opus | Lab and Zermatt from 500 m: no dark streaks on grassy slopes; rock unchanged |

Every step keeps an off switch and records before and after gpu_frame in `CHANGES.md`.
G1 and G2 touch different crates and can run back to back. G3 needs G2. G4 needs G3. G5 needs
the download approval only.

## 4. Open for the owner

- G5 imagery source. Options are a Copernicus global mosaic, or our own composite from
  Level-2A scenes; both YELLOW. EOX cloudless is RED and stays out. Also the download size,
  approved before anything runs.
- Seasons: the composite fixes one season. A later sim step could modulate greenness by
  date; weather modulates, it never decides what grows.
- G6: soften gullies under grass or leave them.
