# Trees and forests: design briefing (2026-11-12)

Status: **the owner has judged it** (2026-11-13). The decisions are locked in
`DECISIONS.md` as **O3-1 … O3-11** (trees) and **O3-W1 … O3-W6** (wind and weather); the
implementation order is §10 below. Nothing here is built or measured: every cost is an
estimate to be replaced by harness numbers.

Verified 2026-11-13 against the `foundation-refactor` worktree: every crate and file path
below resolves (`ed_data/src/node/`, `ew/src/terrain.rs`, `ed_render/src/{terrain,scene,
provider}.rs`, `ed_render/shaders/resolve.slang`, `ed_assets/src/bin/ew-bake.rs`,
`ed_world/src/trees/`, `terrain.rs::morph_band`, `resolve.slang` l.280/L_FOREST and l.397
canopy tint). Both canopy-height datasets are **CC BY 4.0**, so the licence gate in §7 is
closed; the registry entry is still required before any download.

## Summary for the owner (one page)

**The idea in one sentence.** A forest is one map, drawn four ways. Each terrain node gets
a small *canopy map* (how much of the ground is covered by crowns, how tall, which kind of
tree), made from real data. Close up we plant individual trees from that map; far away we
paint and lift the terrain with the same map. Because both come from one map, the forest
has the same outline and the same darkness at 1 m, 1 km and from orbit.

**Why the alpha looked sparse and glitchy.** Four causes, all in the code:

1. It capped trees per tile (16 k) and per zoom, so real forests were truncated.
2. Beyond 1 km it deleted trees on purpose (share kept = (1000 m / distance)²) and scaled
   the survivors up by at most 1.5x. At 4 km only 6% of trees remained: a thin forest.
3. Below 2.5 pixels trees faded out and nothing replaced them, so far slopes went bare.
4. Impostors were 32 px frames snapped to the nearest of 8x8 views, so trees flickered as
   the camera turned; card chunks stopped at 800 m and full trees at 230 m, each a visible
   line.

**What I recommend.**

- **Where trees grow:** real land cover (ESA WorldCover, already licensed) plus a canopy
  height map (needs your OK), then climate, altitude, slope, aspect and wetness for species
  and treeline, then seeded natural patterns below 10 m (gaps, clumps, ragged edges).
- **Unique trees:** a bank of about 8 shapes per species per age class, grown procedurally,
  each instance turned, scaled, leaned and tinted by its own seed. Hero trees can be
  authored in Blender later and sit in the same bank.
- **Distance ladder:** full tree → simpler tree → impostor (a small picture that keeps
  depth and lighting) → canopy as part of the terrain (the ground is lifted to crown
  height and coloured and lit as crowns). Hand-offs happen by size on screen, not by
  distance, and nothing is ever thinned.
- **Light:** forest floor darkens from the same map; forest edges cast long shadows; ready
  for the new sun shadows and later GI.

**What you will judge first.** Still images of Zermatt (your reference camera), a Blue
Ridge 300 m view and an orbit view, for three ways of placing trees and two ways of drawing
the far forest, with stand-in tree shapes. You pick by looking; then we build one rung at a
time.

**What it costs.** Target on High: about 3 ms of the 16.6 ms frame for trees and 1 ms for
ground cover at 1440p. Low about half; Ultra about 4.5 ms with denser near detail.

**What I need from you:** the five questions in §9, mainly the OK to download a canopy
height map and how tall the trees should stand in for real ones (true scale or slightly
exaggerated).

> **Answered 2026-11-13.** All five are decided (O3-7 … O3-11) and the owner added the wind
> and weather requirements (O3-W1 … O3-W6). The order of work follows from those answers:
> §10.4.

---

## 0. One truth: the canopy page

A `CanopyPage` per `NodeId`, beside `HeightPage` (`crates/ed_data/src/node/`), same lattice,
4 bytes per texel: **cover** (crown-cover fraction), **height** (canopy top, m, log-coded),
**mix** (conifer vs broadleaf share), **under** (shrub and ground-cover density). Built
from source data at any level directly (F2); a parent equals the area mean of its children
(tested, as `parent_heights`). It ships as a `layers` chunk (F4). Everything below reads
it: instance placement, impostors, the terrain canopy rung, forest-floor material, shadow
and GI terms, sim queries.

## 1. Distribution

Inputs: WorldCover class (tree, shrub, grass, crop, wetland, mangrove, bare, snow), canopy
height, CHELSA temperature and rain, altitude, landform slope and aspect
(`detail.rs::Landform`), the erosion hollow/spur term (wetness, riparian), lithology later.

| Option | How | Result | Fails when |
|---|---|---|---|
| A. Rules only | Extend today's `resolve.slang` rule (altitude, slope, noise) | No download; plausible Alps | Wrong everywhere the rule was not tuned; no real clearings or village trees |
| B. Data + stateless jittered lattice (**recommended**) | Cover field from data, refined below 10 m by seeded gap and clump noise. Trees live on a global lattice on the cube face (cells about 3 m, keyed by face and integer cell, never by node). A cell holds a tree when `hash(cell) < p(cover, crown size)`; `p` is solved so expected crown cover equals the field | Deterministic, seamless by construction, same trees whichever node or rung asks; the shader can recompute the same cell, so far crowns sit where near trees are | Grid regularity in sparse stands (fix: two lattices at different scales, big trees first, small fill gaps) |
| C. Data + simulated stand | Per node, run a seeded growth and competition pass (Poisson-disc with size hierarchy, gap dynamics) with a margin | Most natural spacing | Needs margins and ordering rules to be seamless; cannot be recomputed in a shader; costlier bake |

Recommendation: **B**, with C's size hierarchy folded in (old trees placed first, then
sub-canopy, then saplings in gaps). Patterns:

- **Jungle vs tundra:** cover 0.9–1.0 and height 30–45 m gives overlapping crowns, about
  300 canopy trees per hectare plus understory; tundra cover < 0.05 gives lone dwarf trees.
- **Treeline:** from warm-season temperature (CHELSA), shifted by aspect and wind
  exposure; cover falls and height shrinks over a 150–250 m band (krummholz), not a line.
- **Edges and clearings:** the 10 m data edge is warped by seeded noise at 2–8 m; an edge
  band gets shrubs and saplings; gap dynamics punch 10–40 m clearings at a climate-driven
  rate.
- **Riparian strips:** hollow term and water distance raise cover and switch species.
- **Village trees:** WorldCover built-up cells take a low stochastic cover until Phase 3
  supplies parcels.
- **Species mix:** rows in a species table (as the alpha's `SpeciesDef`) with niche curves
  (temperature, rain, altitude, wetness, light); the cell hash picks by niche weight.

Cost: bake only. Target under 5 ms per node page on one core, 0.5 MB per resident page;
identical on every tier (tiers change drawing, never placement).

## 2. Uniqueness

| Option | How | Cost | Fails when |
|---|---|---|---|
| A. Procedural bank (**recommended start**) | Port the alpha generator and `Genome` (`alpha-early:crates/ed_world/src/trees/`), bake N variants per species per age class to meshes with a LOD chain and an impostor; `ew-bake`, deterministic | No art time | Generator quality limits close-up realism |
| B. Authored glTF bases x variation | Blender trees (F6), 3–4 per species, varied by instance parameters | Best close-up | 3–4 shapes read as clones in a stand; art cost per species |
| C. Per-instance generation | Every tree grown at load | Truly unique | Memory and load time; no batching; cannot have impostors |

Recommendation: **A now, B's authored trees enter the same bank later.** Per instance, from
`hash(cell)`: yaw, height and girth (±25%, conditioned on the canopy page's height), lean
toward light and away from slope, hue and value tint, season phase, and two branch-drop
masks that hide whole limbs in the vertex shader. Bank size: **8 variants x 3 age classes**
per species (conifers can run 6, specimen broadleaves 12). With continuous yaw and scale,
two neighbours share a shape with probability 1/8 and never share pose; I expect no visible
clones at this size, and the prototype board in step 3 tests it. First species: Norway
spruce, larch, Arolla pine, birch/alder (Zermatt); oak, tulip poplar, red maple, hemlock,
white pine, rhododendron (Blue Ridge).

Memory estimate (to measure): meshes about 1 MB per variant with LODs; impostor about
0.7 MB per variant at 8x8 frames of 64 px with albedo, normal and depth. High: about
240 variants, roughly 400 MB. Low: 4 variants per species and 32 px frames, about 80 MB.
Ultra: 12 variants, 96 px frames.

## 3. The distance ladder

Hand-offs are by projected tree height in pixels (1440p, 60° vertical: 1 m at 1 km is
1.25 px). Distances are for a 15 m tree, as illustration.

| Tier | Rung | Drawn as | From → to (px; m) |
|---|---|---|---|
| T0 | R0 | Full mesh, wind, trunk collision | > 250 px; < 75 m |
| T1 | R0 | LOD chain: fewer, larger leaf cards, simplified limbs | 250 → 40 px; 75 → 470 m |
| T2 | R1 | Octahedral impostor with depth and normals, blended between the three nearest frames, per-instance tint and scale | 40 → 6 px; 470 m → 3 km |
| T3 | R2/R3 | Canopy in the terrain: vertices lifted by canopy height, crown albedo, crown-bump normal and self-shadow from the lattice hash, roughness | < 8 px; > 2.3 km |
| T4 | R3/R4 | Canopy albedo in terrain layers, later tied to Sentinel-2 hue | Orbit |

Rules that remove the alpha's faults:

- **No caps, no thinning.** Instance count is whatever the canopy page yields. The budget
  acts on hand-off thresholds (Low hands to impostors at 80 px and to canopy at 14 px).
- **Density is conserved.** T0–T2 are the same lattice cells. T2 → T3 is a dithered
  cross-fade over 8 → 6 px: each impostor dithers out by its hash while the canopy shading
  under it fades in from the same cover value, so covered screen area is constant. Vertex
  lift ramps in only after impostors are gone, when a crown is under 6 px, using the
  existing geomorph band (`terrain.rs::morph_band`).
- **Colour is conserved.** One species row gives the leaf colour used by mesh, impostor
  bake and canopy albedo (world.md "Consistency").
- **No frame snapping.** Impostor frames are blended and depth-offset, 64 px on High.

Fit to the engine: T0–T2 are instances in provider 1's pools (`ed_render/src/scene.rs`,
`provider.rs`), generated per node from its canopy page into an `instances` chunk; never
merged meshes. T3–T4 live in the terrain provider and `fs_resolve_terrain`, replacing the
hard-coded canopy tint at `resolve.slang` ~l.397. The lift is render-only: collision and
height queries keep the bare-ground truth (P2-D3).

T3 options: (a) colour only, as today: free, but ridgelines look shaved and edges cast no
shadow; (b) **lift + crown shading (recommended)**: real silhouette and shadows because
terrain is already a shadow caster, costs one page sample in the vertex stage; fails at
coarse vertex spacing where edges become ramps (keep T2 longer at forest edges); (c)
ray-marched canopy slab: best parallax, about 1 ms more, Ultra candidate only.

**Track G:** later. Leaf cards are alpha-tested aggregates that cluster simplification
handles badly; foliage stays on provider 1. Trunks and limbs can be measured in Track G
step 5.

Cost estimate (High, forest view): T0 about 60 trees, T1 about 6 k, T2 about 250 k quads
in the frustum; 2.0 ms visibility + 1.0 ms shading. Low 1.5 ms, Ultra 4.5 ms. The risk is
impostor overdraw and culling 250 k instances; the fallback is moving the T2 → T3
threshold nearer, which the cross-fade makes safe.

## 4. Lighting and shadows

- **Self-shadow:** each bank mesh carries baked crown depth and AO per vertex, used for
  sun transmission and ambient darkening; impostors bake the same.
- **Under the canopy:** the terrain resolve dims sky light by `cover` and adds dapple.
  Forest floor reads dark at every distance.
- **Forest shadows on terrain:** near, T0/T1 cast into the sun shadow maps with shadow
  LODs. Mid, impostors do not cast; the resolve samples the canopy page offset along the
  sun by `height / tan(elevation)` (one tap): long edge shadows for nearly nothing. Far,
  the lifted terrain casts by itself and feeds the far height term.
- **VSM readiness:** trees are static casters with stable ids; wind must not invalidate
  cached pages, so shadows use the rest pose beyond about 30 m (look decision, Opus).
- **Voxel GI (Phase 4):** the canopy page is injected as a slab of opacity and albedo; no
  per-tree voxelisation.
- Options: (A) page-based terms only (Low); (B) A + near casters (**High**); (C) B +
  impostor shadow cards or the slab march (Ultra).

Fails when: low sun on mid-distance forest shows the one-tap shadow as too smooth.

## 5. Ground cover and the forest layer

`under` and `cover` drive the terrain material weights (replacing the noise rule for
`L_FOREST` at `resolve.slang` ~l.280): closed conifer gives needle litter and moss, open
broadleaf gives leaf litter and ferns, edges give grass and shrubs. Scatter rides the same
lattice at finer cells: ferns (bracken_fern), shrubs (bay_laurel), logs and stumps
(mossy_stump, old_wooden_log) from the dev-only Megascans library, grass as GPU-instanced
blades within about 60 m fading into the grass material. Options: (A) material only
(Low); (B) material + scatter to 60 m (**High**); (C) to 150 m with understory impostors
(Ultra). Cost target 1.0 ms High. Megascans meshes are YELLOW: dev only until `ew-bake`
and the licence gate cover them.

## 6. Collision, destruction, sim

- The lattice and canopy page are engine-free (`ed_data`, `ed_world`); `ed_sim` and
  `ed_server` can compute any tree's position, species and size without render crates.
- Collision: trunk capsules for trees within about 60 m of a player, built from the same
  cells. Leaves, impostors and canopy never collide.
- Identity: `ObjectId = (NodeId, cell, slot)`, stable across runs. A knocked tree is a
  server-owned override record (F5) keyed by that id: the generator skips the cell, the
  server spawns a dynamic body. One felled tree does not change the canopy page; clear-cuts
  later write an override layer for it.
- Options: (A) static trunks only (**now**); (B) + override records (destruction phase);
  (C) fully simulated stands (not planned).

## 7. Data and licensing

| Dataset | Use | Status | Size for Zermatt + Blue Ridge (estimate) |
|---|---|---|---|
| ESA WorldCover 2021, 10 m | Cover classes | GREEN; reader exists (`ed_data/src/worldcover.rs`, range reads) | A few MB per 30 km window |
| CHELSA v2.1 | Treeline, species niche | GREEN; needs a baked extract replacing WorldClim (RED) | Under 1 MB per region |
| Canopy height, 10 m (ETH Global Canopy Height 2020; I believe CC BY 4.0) | Height, old vs young stands | YELLOW: verify licence, add registry entry, **owner OK to download** | Windowed reads if hosted as COG, about 5–10 MB per region; whole 3° tiles are hundreds of MB |
| Alternative: Meta/WRI 1 m canopy height | Individual crowns, clearings | YELLOW, same checks | Much larger; windowed only |
| Macrostrat lithology | Soil preference | YELLOW, later | Small |

Without canopy height the system still runs: height comes from climate and species
allometry (option 1A/B fallback), but old-growth vs regrowth is lost.

## 8. Build plan (gates, no dates; one worker at a time)

> **Superseded by §10.4.** This was the plan before the owner decided. The dependency-correct
> order is G0–G11 in §10.4; keep this section for the prototype detail it carries (the
> `EW_TREES_PROTO` switches and the board list), not for the sequence.

**Step 1 — option prototypes (Sonnet, from this brief).** Throwaway, behind
`EW_TREES_PROTO`, no docs, no git.
- New `crates/ed_data/src/node/canopy.rs`: `CanopyPage`, `node_canopy(src, id)`,
  `parent_canopy`, and `tree_cells(id, page) -> Vec<TreeCell>` for distributions **A, B,
  C** of §1 (`EW_TREES_PROTO=a|b|c`). WorldCover through the existing reader into the
  cache; no canopy height yet. Tests: two runs identical; trees on a shared edge identical
  from both neighbours; parent equals mean of children.
- `crates/ew/src/terrain.rs` (page loading beside heights), `crates/ed_render/src/
  terrain.rs` and `shaders/resolve.slang`: bind the canopy page; far forest drawn as
  **F1 colour only** or **F2 lift + crown shading** (`EW_TREES_FAR=1|2`).
- Stand-in trees: three procedural shapes (spruce cone, broadleaf ellipsoid, larch) with a
  3-level LOD chain as provider-1 instances from `tree_cells`, per-instance yaw, scale and
  tint, out to the T2 → T3 threshold. No impostors yet; record instance count and frame
  time.
- Boards, 2560x1440, hour 15, each for a/b/c x F1/F2 (12 images per view, plus one
  contact sheet per view):
  1. Zermatt: `--ground 46.0200,7.7560,236.2,-1.5,20` with `EW_FOV=45`.
  2. Blue Ridge at 300 m above ground, pitch −15.
  3. Orbit, 400 km over each place.
  4. A density strip: one camera at 50 m, 500 m, 5 km, 50 km from the same slope.
- **Gate:** the owner picks a distribution and a far rung; forest outline in board 4 is
  the same at all four distances (cover-pixel fraction within 10% between neighbours).

**Step 2 — canopy page for real (Sonnet):** chunk kind, residency, CHELSA extract, canopy
height if approved. Gate: determinism and seam tests; bake time and bytes recorded.
**Step 3 — tree bank and look (Opus):** port generator and genome, species rows for both
regions, wind, leaf shading. Gate: owner sees a 100-tree stand board with no clones.
**Step 4 — T0/T1 instancing at full density (Sonnet, Opus checks):** Gate: 3 ms at High in
the harness, no caps.
**Step 5 — impostors and the T2 → T3 cross-fade (Opus core, Sonnet bake plumbing):** Gate:
scripted ground-to-orbit flight with no pop on the frame-difference metric.
**Step 6 — lighting hooks (Opus):** floor darkening, offset shadow, casters. Gate: owner
judges against the Zermatt photo at three times of day.
**Step 7 — ground cover and forest floor (Opus look, Sonnet scatter plumbing).**
**Step 8 — trunk collision and `ObjectId`s (Sonnet).** Gate: car stops at a trunk; ids
stable across runs.
**Step 9 — measurement battery and tier switches (Sonnet), then one Fable review.**

## 9. Questions for the owner

> **Answered 2026-11-13.** The owner's answers are locked in `DECISIONS.md` as O3-7 (Q1),
> O3-8 (Q2), O3-9 (Q3), O3-10 (Q4) and O3-11 (Q5). The questions are kept for the record.

1. OK to download a canopy height map (ETH 10 m first) once the licence is verified?
2. True-scale trees and real density, or slightly exaggerated crowns so forests read
   better from the air?
3. Seasons: do leaves change with date and latitude in Phase 2, or fixed summer?
4. Is 3 ms + 1 ms on High the right share of the frame for forests, given clouds and VSM
   still to come?
5. Authored Blender hero trees: wanted in Phase 2, or procedural only until Phase 4?

---

# 10. Owner decisions and implementation order (2026-11-13)

The owner has answered the questions in §9 and added requirements the brief did not cover.
The decisions are locked in `DECISIONS.md` as **O3-1 … O3-11** (trees) and **O3-W1 …
O3-W6** (wind and weather). This section turns them into an order of work, and records the
places where they do not fit together or do not fit the Phase 2 budget.

## 10.1 Where the pieces live

| Piece | File | State |
|---|---|---|
| `HeightPage`, `PAGE_CELLS`, `PAGE_N` | `ed_geo`, re-exported by `ed_data/src/node/heights.rs` | landed; the pattern `CanopyPage` copies |
| `node_heights`, `parent_heights` | `ed_data/src/node/heights.rs` | landed; "a parent equals the mean of its children" is already tested |
| Tree generator, `Genome`, `WindMesh` | `ed_world/src/trees/mod.rs` | landed |
| `SpeciesDef` rows, palettes, trunk colliders | `ed_world/src/trees/species.rs` | landed |
| Impostor bank | `ed_world/src/trees/impostor.rs` | landed; `FRAMES = 8`, `FRAME_PX = 32`, `CARD_RANGE_M = 800` — the exact faults §Summary names |
| Tree placement | `ed_world/src/foliage.rs::scatter` | 8 m cells, hash jitter, `MAX_PER_CELL = 3.0`, `max_trees` thinning. Already closer to §1-B than to §1-A |
| Ground scatter, grass flex channel | `ed_world/src/ground_scatter.rs` | landed |
| Terrain provider, scene, geometry slot | `ed_render/src/{terrain,scene,provider}.rs` | landed (F3, provider 1) |
| Terrain resolve shader | `ed_render/shaders/resolve.slang` | landed; the canopy binding lands here (`L_FOREST` l.280, canopy tint l.397) |
| Cloud climate, storm cells, noise volumes | `ed_world/src/clouds.rs` | landed, but driven by live Open-Meteo |
| Live weather (wind speed, direction, gusts) | `ed_data/src/weather.rs` | landed (alpha source) |
| Bake tool | `ed_assets/src/bin/ew-bake.rs` | landed |
| Cluster geometry (Track G) | `phases/track-g.md`, `plans/renderer.md` §3 | parallel, never blocks a gate (D17) |

Two consequences worth stating plainly:

- **§1-A ("rules only") does not describe the code.** The code is already a jittered lattice
  driven by real density (`surface.tree_density`, `cover_at`, biome niche). Moving from
  today's code to §1-B is a small delta; moving to §1-C is real work.
- **The impostor bank is the blocker for "truly unique".** Eight 32 px frames per species
  is the 2026-11-04 limit, and `alpha/plans/procedural-flora.md` step 3 already planned to
  bucket by genome instead of variant index. O3-3 makes that change mandatory.

## 10.2 Contradictions to resolve

**10.2.1 The simulated stand cannot be recomputed in a shader, and the slab marches a field.**
O3-1 puts placement in a per-node growth pass; O3-2 wants a ray-marched canopy at distance.
A march needs a field, not a tree list. *Resolution:* the growth pass writes its measured
cover, canopy height and mix back into the canopy page, and the slab marches that page. Near
trees and far forest then agree on density and height, not on individual trees — which is
what the T2→T3 cross-fade needs. The remaining risk is that the cross-fade matches a
silhouette, not just a density: a simulated stand's outline is not the same as the field it
was measured from. That needs a test, and it is the riskiest item in the plan.

**10.2.2 The most expensive far rung, with the smallest tree budget.** O3-2 is the brief's
"+1 ms" option; O3-10 sets trees at 2 ms + 0.5 ms. They cannot both be true.
*Resolution:* build the lift + crown shading rung (§3-B) first, because it is needed
regardless — it is the cheap fallback and the baseline for the T3 hand-off. Add the slab
afterwards, gated on a measurement, and only where it pays.

**10.2.3 Wind-swayed trees casting shadows breaks the static-caster rule.** O3-4's note
wants dynamic shadows from wind-swayed trees against buildings; §4 says trees are static
casters precisely so wind cannot invalidate cached shadow pages. *Resolution:* gate by
**shadow-cascade range**, not distance from the player. Near trees inside the cascade are
dynamic casters; everything outside it uses the rest pose. The VSM raster budget then has
to be re-measured on real content.

**10.2.4 Q1 wants both datasets.** O3-7's note says A and B. §7 treats B as a second
approval. *Resolution:* one interface, two sources — the canopy page carries a resolution
field (10 m global, 1 m local). ETH first, behind the windowed-read pattern in
`ed_data/src/worldcover.rs`. Both licences are CC BY 4.0 (verified 2026-11-12); registry entries go in before the first read, then only the
engineering remains.

**10.2.5 "Authored bases × variation" is not what O3-3's note describes.** §2-B is 3–4
hand-made trees per species; the note describes authored *parts* (branches, leaves, ferns)
assembled by a generator — the `BranchSpec` work from `alpha/plans/procedural-flora.md`
step 6. *Resolution:* build the assembly system, not a bank. Parameters for each species'
branch geometry; authored meshes feed the generator; the impostor bake runs on the
assembled result, keyed by genome bucket.

## 10.3 The budget

Phase 2 §5 budgets the bare planet at **8.0 ms** of 16.6 ms (terrain 2.0, atmosphere 1.5,
cloud shell 1.0, ocean 1.0, sun VSM 2.5, tone map + upscale 0.6), leaving 8.6 ms for
everything else. The new work adds to that:

| Item | Cost | Note |
|---|---|---|
| Trees (O3-10) | 2.5 ms | the owner's leaner choice |
| Ground cover | 1.0 ms High | O3-5 is the Ultra option; it is not costed |
| Clouds | 2.5–3.0 ms, unmeasured | O3-W5 wants raymarched volumes; the 1.0 ms line is for the cloud *shell* being replaced |
| Canopy slab (O3-2) | +1.0 ms over the lift rung | the brief's estimate |
| Dynamic near-tree casters (O3-4) | +? on the VSM raster | pushes past the 1.2 ms amortised figure |

Total ≈ **15.3 ms**, leaving about 1.3 ms for Phase 3 geometry, characters and UI — before
the dynamic shadow cost, which is unknown.

This is not a reason to abandon anything; it is a reason to sequence. The Phase 2 rule is to
measure and report, not to adjust (§5). So: build the lift rung and the lean tree budget
first, measure on the Zermatt and Blue Ridge boards, then admit the slab, the 150 m scatter
and the dynamic shadows one at a time, each with a number.

## 10.4 Order of work — gates, not dates

Ordered by dependency, not preference. Each gate is checkable.

**G0. The canopy page.** `CanopyPage` in `ed_geo` beside `HeightPage`: 4 bytes per texel
(cover, height, mix, under), `node_canopy`, `parent_canopy` (mean of children, tested the
way `parent_heights` is), chunk kind and residency. Everything below depends on this and
nothing else does.

**G1. Cover from real data.** WorldCover through the existing reader into the cache, CHELSA
extract, ETH 10 m behind a windowed read. *Gate:* two runs identical; trees on a shared edge
identical from both neighbours; parent equals the mean of children.

**G2. O3-1 — the simulated stand.** Replace `foliage::scatter` with a seeded
growth/competition pass (Poisson-disc, size hierarchy, gap dynamics), with margins.
*Gate:* the pass writes its measured cover/height/mix back into the canopy page, and the
page agrees with the placed trees within 5 %.

**G3. O3-2, part one — the lift rung (§3-B).** Bind the canopy page in `resolve.slang`; lift
terrain vertices by canopy height; crown albedo, crown bump and self-shadow from the lattice
hash. *Gate:* the density-strip board (50 m / 500 m / 5 km / 50 km) holds the same forest
outline within 10 %.

**G4. O3-10 — the lean budget and the measurement battery.** Hand-off thresholds, tier
switches, no caps, no thinning. *Gate:* 2 ms + 0.5 ms on the harness at High, 1440p,
measured — not estimated.

**G4b. The wind interface.** `WeatherSample` — wind vector, gust, turbulence — as a function
of (position, time), plus the shader term that applies the per-vertex sway, flutter and
phase the generator already bakes. The field may return a static default for now; the
**shape of the interface cannot change later** without a rebake and a shader rewrite. See
§10.5.

**G5. O3-8, O3-9, O3-7 — per-species crown bias, season phase, canopy height data.** Crown
bias is a `SpeciesDef` column applied at bake into the canopy page. Season phase is date +
latitude + altitude, with a global rate scalar so it can be gradual or fast. *Gate:* the
Zermatt board at three times of day and three altitudes.

**G6. O3-3 and O3-11 — authored parts and assembly.** Expose the branch parameters as genome
ranges; authored meshes feed the generator; impostor bake on the assembled result, bucketed
by genome. *Gate:* a 100-tree stand board with no visible clones, and a far view that still
reads.

**G7. O3-5 — ground cover.** Hierarchy-based scatter (logs, then moss), dense grass,
niche-driven by biome and climate. *Gate:* the scatter agrees with the canopy above it, and
the 150 m reach is measured, not assumed.

**G8. O3-4 — near casters.** Trees inside the shadow cascade are dynamic casters; outside it,
rest pose. *Gate:* a night board with a street light and high wind — a tree sways against a
building — and the VSM raster cost reported.

**G9. O3-2, part two — the ray-marched slab.** Only after G4 and G3 have real numbers.
*Gate:* it costs what it promised, and it is only enabled where it pays.

**G10. O3-6 — override records and felling.** `ObjectId = (NodeId, cell, slot)`;
server-owned override records; the generator skips the cell; the server spawns a dynamic
body. *Gate:* a car stops at a trunk, a tree falls with enough force, ids stable across runs.

**G11. Branch breaking.** Coupled to G6: the tree mesh has to be splittable, which is an
asset requirement, not a renderer one.

## 10.5 Wind and weather

The design of record for the simulation is the older plan
`alpha/plans/world-sim.md` ("Plan: procedural climate, weather and storms"), which is still
valid as a design reference (`AGENTS.md`: older plan docs are). Weather and climate are
procedural, not live Earth weather: **real Earth running its own version, with different
parameters.**

**What the owner wants to see (O3-W1, O3-W5, O3-W6):** leaves rustling in the wind, trees
swaying softly with a breeze, hard wind making them lean, and bands of wind flowing through
grass fields. Genuine fronts. Clouds that rain, with the rain coming from the clouds rather
than from the air. Faster clouds with faster wind. Terrain affecting the weather. The wind
affects every tree and every blade — the whole forest and the whole field, not a subset.

**What already exists, so nobody rebuilds it:**

| Piece | Where | State |
|---|---|---|
| Per-vertex wind data: sway amplitude, leaf flutter, phase, packed into UV1 | `ed_world/src/trees/mod.rs` `WindMesh`, `pack_wind`, `unpack_wind` | landed |
| Grass flex channel (`uv.x = flex`, 0 rigid … 1 sways fully) | `ed_world/src/ground_scatter.rs` | landed |
| Cloud climate from a weather report (`CloudParams`), storm cells, tileable noise volumes | `ed_world/src/clouds.rs` | landed, driven by live Open-Meteo |
| Live weather with wind speed, direction and gusts | `ed_data/src/weather.rs` | landed (alpha source) |

**What is missing:** the wind *field* — `WeatherSample` from `world-sim.md` layer 4 — and
the shader term that applies it. `ed_render/shaders/*.slang` has no wind term today.

**The scope decision (O3-W3):** the climate and weather simulation is a later phase, with
the gameplay systems that read it. For now, trees. But three things must be settled in the
tree work because the later simulation plugs into them, and changing them afterwards costs
a rebake and a shader rewrite:

1. The wind path in the tree and grass shaders, and the `WeatherSample` shape (G4b).
2. The impostor bake at **rest pose** — an impostor has no wind, so it must be baked
   unswayed and the wind term applied consistently in the shader, or the cross-fade to full
   trees will pop.
3. The shadow cache rule (§10.2.3).

## 10.6 What needs the owner or an artist

Not a limitation of effort — these are the parts where a person has to look or decide:

- **Art direction.** "Super dense lush grass", "trees need to feel real", the night
  street-light look, "bands of wind through the grass". Agents can build the system; the look
  has to be judged on boards.
- **Asset licences.** Authored tree parts, leaf textures, species palettes, fern meshes.
  Blender is available to the implementation agents (O3 note, 2026-11-13), so making the
  assets is not blocked on a human — but the owner mentioned free tree assets, and licences
  still need a human pass before anything ships.
- **The contradictions in §10.2.** Each needs an owner answer, and three of them change what
  gets built.
- **The budget.** If G4 and G8 come in over, something moves to a later phase. That is a
  scope decision, not an engineering one.
