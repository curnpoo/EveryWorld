# Round 2: trees v2

Renders in this folder come from `earthdrop --lookdev OUT --only trees --width 1280 --height 720`
(software rendered, Blender Filmic).

## Species (each with 3 seeded variations)

Trees: oak (`Broadleaf`), maple, plane (street tree, camouflage bark), birch (white bark with lenticels,
drooping twigs), spruce (`Conifer`, whorls with hanging sprays), fir, Scots pine (orange upper bark,
umbrella crown), Italian cypress, olive (gnarled, split trunk, silver green), umbrella acacia (flat
table), baobab (bottle trunk), coconut palm (`Palm`, curved trunk), date palm (dead frond skirt),
banana, saguaro (`Cactus`, ribbed, arms with elbows), prickly pear (lens pads, red fruit).

Shrubs and ground plants: generic shrub, boxwood, juniper, oleander (with blossoms), flowering bush
(three blossom colors), fern, grass tussock.

Biome and land-cover mixes live in `ed_world::foliage::species()` (woods get an understory of ferns
and shrubs, towns get street trees and garden shrubs). Tile scattering now thins evenly when a tile
hits its tree cap instead of filling from the north edge.

## Techniques

- **No primitives.** Trunks and branches are tapered tubes swept along curved, noise-perturbed
  paths with parallel-transport frames (smooth bends, no stepping), root flare lobes at the base,
  rounded domes on cactus tips, elliptical cross sections for prickly-pear pads, angular rib
  modulation for saguaros. Palm trunks are cubic Bezier curves with 40 rings plus a ringed scar
  texture.
- **Leaves.** Alpha-tested, two-sided leaf-cluster cards (cupped, 3x2 grid) and ribbons (palm and
  fern fronds, banana leaves, grass) textured from a procedurally painted 2048x2048 atlas
  (`ed_world::trees::textures`): lobed oak, palmate maple/plane, oval, lanceolate, olive, needle
  sprays, pine tufts, scale foliage, acacia leaflets, blossoms, palm/dead/fern fronds, grass,
  banana. Each leaf has its own shading (lighter tip, darker rim, midrib, side veins, folded half),
  and back layers of a cluster are painted darker for depth. Mips are generated in-crate with
  alpha-weighted color and per-cell coverage preservation so crowns do not thin out at distance.
- **Crown shading.** Leaf normals are bent toward the crown's outward direction so the canopy shades
  like one soft volume. Vertex colors bake ambient occlusion toward the crown interior and the
  ground plus a warm, lighter rim on the crown surface. The leaf material uses diffuse
  transmission so backlit leaves glow.
- **Wind.** `ExtendedMaterial<StandardMaterial, WindExt>` with WESL vertex shaders for the main pass
  and the prepass/shadow pass (same displacement, so depth, shadows and TAA motion vectors line
  up). UV1 packs a per-branch phase and sway amplitude (grows with height and along branches) plus a
  leaf flutter amplitude. Wind direction and strength come from the live weather
  (`sky::CurrentWeather`), with gust fronts rolling across the landscape.
- **LOD.** Finest tiles split their trees into a 12x12 chunk grid. Each chunk has one merged far
  mesh (about 30 to 40 large cards plus a simple trunk per tree, grown from the same seed as the
  full model so the silhouette matches). Within about 230 m the chunk swaps to full-detail trees,
  spawned as entities that share one mesh per species variant so Bevy instances them. Coarser
  tiles only carry the far mesh.

## Known gaps

- Near/far swap is a hard switch per chunk (with hysteresis), not a dithered crossfade.
- Per-instance tint is not applied to instanced near trees; color variety comes from the 3 variants.
- No bark normal maps; bark relief is texture and vertex color only.
- Far cards are scaled-up cluster cards, not baked impostors, so very distant crowns read slightly
  coarse.
- Ferns and grass are placed as scattered plants, not as a dense ground layer.
- The streamed-world check ran in a cloud container without a GPU; lookdev shots are the reference.
