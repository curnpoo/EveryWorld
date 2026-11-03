# Round 2: ground v2

Textured terrain plus near-camera ground cover, driven by the same land-cover and biome rules as before.

## What's here

| File | What it is |
|---|---|
| `ground_swatches.png` | Row 1: every ground layer on its own. Row 2: lush grass under each biome tint. Row 3: common mixes (grass/dirt, grass/moss, litter/moss, dry grass/dirt, sand/gravel, rock/snow, gravel/snow, paving/grass, farmland/dirt, wet mud) |
| `ground_swatches_closeup.png` | Low angle over the first swatches |
| `ground_<place>_stand.png` | Eye at 1.5 m |
| `ground_<place>_crouch.png` | Eye at 0.4 m, looking about 2.4 m ahead |
| `ground_<place>_aerial.png` | About 90 m up, textures only (scatter is gone at this height on purpose) |

Places: meadow, forest floor, desert sand, alpine rock + snow, Mediterranean dry scrub, urban park edge.

## Techniques

**Where things go (ed_world, engine-free, unit tested)**
- `surface.rs`: 12 ground layers (grass, dry grass, moss, forest floor, dirt, farmland, sand, gravel, rock, snow, paving, mud). `SurfaceMap::splat()` turns land cover + biome + slope + altitude into per-layer weights. Each cover class has a per-biome recipe (Mediterranean grass is dry grass, parks stay watered, desert rock is sandstone mixed with sand). Cover edges are soft and organic: the class grid is sampled at a few domain-warped taps. Uniform mixes are broken into patches with two scales of noise per layer. Then come the rules: scree, then rock on steep ground, beaches near sea level, and a patchy snowline that slides off cliffs.
- `SurfaceMap::tint()`: regional vegetation tint (golden Mediterranean, deep tropical green, blue-ish boreal) plus slow dryness/brightness patches and wetness. It goes in the vertex color and only living layers take it.
- All noise uses global tile coordinates, so patterns line up across tile seams and zoom levels.
- `terrain.rs`: z14 tiles get 3x subdivision (z13: 2x) so the splat follows the ~9 m land-cover grid. The extra vertices sit exactly on the height grid's triangles (same diagonal), so the render mesh still matches the collider and `TerrainGrid::sample`. Weights are packed into `MeshData::splat` (12 bytes per vertex).

**Textures (ed_world `ground_tex.rs`)**
- Every layer is synthesized at startup (about 1 s for 12 x 512 px on 4 threads): periodic noise, Worley cells and "painted" elements (grass strokes, leaves, needles, pebbles, slabs, plough rows, asymmetric ripples). This gives a height field, then normal, cavity AO and roughness are derived from it. The mean albedo is matched to `Ground::mean_color`, so the mip tail (what you see from altitude) keeps the palette.
- Upload: two RGBA8 2D array textures (albedo + height, normal + roughness + AO) with full CPU mip chains, plus one small macro-noise texture. No files, no downloads.

**Terrain shader (`shaders/terrain.wesl`, `ExtendedMaterial<StandardMaterial, GroundExt>`)**
- Per-pixel height blending of the active layers (weights under about 1 % are skipped): pebbles poke through snow, grass sits over dirt.
- Flat ground uses top-down projection, sampled twice (a rotated and shifted copy) and mixed by a macro mask, so the 3-8 m repeat is hard to spot. Directional layers (rows, ripples, slabs, drifts) only shift, they don't rotate. Steep ground (slope above about 0.1-0.22) cross-fades to triplanar with UDN normal blending.
- Three macro-noise scales (41 m, 263 m, 1.8 km) modulate brightness and dryness. Detail normals fade with distance. Wetness darkens and adds gloss.
- Texture space is world meters plus an offset that is updated on floating-origin rebase, so textures don't jump.
- Then it is standard Bevy PBR lighting (shadows, SSAO, atmosphere env light, Filmic).

**Ground cover (`ed_world::ground_scatter`, `ed_game::ground_scatter`, `shaders/ground_scatter.wesl`)**
- Kinds: grass tufts (meadow / lawn / alpine variants), dry tufts with seed heads, wildflowers (regional palettes: poppies in the Mediterranean, blue/purple in the Alps, daisies on lawns), weed rosettes, pebbles, rocks, moss cushions, dry scrub (woody or leafy garrigue), ferns, twigs and snow lumps.
- No primitives. Blades are curved and tapered. Rocks are icospheres chipped by random planes (flat facets with soft, bevel-like edges), fBm-displaced, squashed, flattened underneath, with mottled AO and lichen on top. Cushions are lumpy displaced domes. Twigs are bent prisms with side branches.
- Placement: one jittered candidate per cell per kind. The probability is `sum(splat weight x kind affinity)`, using the same splat as the terrain shader, so a tuft only grows where the ground is drawn as grass. Roads and buildings are masked out with the foliage occupancy grid. Cliffs reject tall plants.
- Streaming: z14 tiles are cut into 128 x 128 chunks (13-19 m). Chunks within 100 m of the camera are built on worker threads (sync in lookdev) and spawned as one merged mesh each under their tile. Each chunk belongs to a distance band and is rebuilt only when it needs more instances, or could shed a lot.
- Smooth fade: every vertex carries its instance pivot + `reach.floor() + rank`. The vertex shader shrinks an instance to its pivot once `rank > fade(distance)`, where fade falls as 1/d^2 (constant on-screen density) and reaches zero at the kind's reach (24 m pebbles up to 100 m rocks). Grass gets a bit bolder with distance. Blades within about 1 m of the camera are faded out so they never fill the lens.
- Wind: gust + flutter bend, quadratic toward the tips. The same vertex shader runs in the main pass, prepass, motion vectors and shadows, so TAA, SSAO and shadows agree with the swaying geometry.

## Perf notes

- Texture synthesis: about 1.1-1.3 s at startup (debug build, 4 threads). The arrays are about 33 MB with mips.
- Terrain: about 4x more triangles on z14 tiles than before (193^2 grid instead of 65^2), 2.25x on z13. Coarser tiles are unchanged.
- Fragment cost: typically 2-4 active layers x 4 texture fetches (2 planar copies x 2 arrays), up to 3 projections on cliffs. All fetches use `textureSampleGrad`, so skipping layers is legal in non-uniform control flow.
- Scatter: near chunks in a lush meadow run about 20 tufts/m^2. Expect roughly 0.5-1.5 M scatter vertices around the camera in dense grass, far less elsewhere. Bands 4+ don't cast shadows. Chunk builds are a few ms each on a worker.
- All renders here come from the CPU renderer (lavapipe), at about 2 s per frame. They say nothing about Metal frame times. Profile on a Mac before tuning density.

## Known gaps

- Crouch views (0.4 m) can show faint thin streaks in the sky under the CPU renderer. Not chased down. The renders in this folder are from the board before the last tuning pass (grass texture contrast, forest litter, moss and snow lump sizes, alpine/urban camera spots), which was built and unit tested but not re-rendered. The live Zermatt shot was not completed (CPU renderer too slow).

- Lavapipe streaks: in lookdev, terrain tiles with skirts produced long sky-spanning streaks under the CPU renderer. The CPU-side geometry is verified clean (unit tests check edge lengths and vertex/pivot distances), and the streaks happen with the stock StandardMaterial too. So lookdev place tiles are built without skirts, and their shared edge heights match exactly. Watch for it on Metal with streamed tiles; if it shows up there, it is a skirt/clipping issue rather than the ground shader.
- Scatter is CPU-built merged meshes, not GPU-instanced. That is simple and robust; GPU instancing or compute placement would cut memory if density goes up.
- Splat weights are per-vertex (about 9 m spacing on z14). Height blending hides most of it, but very thin features (a 3 m path) still come from roads/OSM, not the ground splat.
- Farmland rows run in one world direction. Real fields should follow their own polygon's orientation.
- Rock and dirt color doesn't vary with real geology yet. The only regional variation is per biome (desert sandstone, Mediterranean limestone). Macrostrat is the planned upgrade.
- Snow lumps and moss cushions are small and subtle on purpose. Deep snow drifts would need terrain displacement.
