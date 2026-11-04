# Plan: performance (virtual geometry + instancing)

Status: PLAN / findings only. Direction set by user: Nanite-like virtual geometry as the core, full detail everywhere, NO impostors, NO classic discrete LOD meshes. Update Nov 4: the user approved impostors for trees (drawn as far as they make sense); everything else keeps this direction.

## Measured (perf.csv, perf_lod.csv, perf_lowres_nopost.csv, rows with tiles loaded)
- 15-21 fps average, p99 frame 90-114 ms.
- 59-136 million triangles submitted, 7-9k entities. Fewer triangles -> higher fps. Disabling post-processing barely helped, so the cost is geometry.

## How things draw today
- Near trees (`foliage.rs`): entity per tree + child entity per part, shared mesh handles. Far trees: merged mesh per chunk, swapped with near entities by distance.
- Ground cover (`ground_scatter.rs`): every blade/flower BAKED into one merged mesh per chunk (`to_scatter_mesh`), rebuilt on band change. Not instanced.
- Buildings: LOD massing + per-chunk detail meshes. Terrain: quadtree tiles (`lod.rs`).
- Nothing is culled below chunk granularity; no occlusion culling.

## What "meshlets" are (so we share vocabulary)
A meshlet (cluster) is a small group of ~64-128 triangles of one mesh. Virtual geometry builds a hierarchy of clusters offline: groups of clusters get merged and simplified into coarser clusters, up to a root. At runtime the GPU walks this hierarchy per frame and picks, per region of the mesh, the coarsest cluster whose simplification error is under ~1 pixel on screen. Result: triangle size is about a pixel at every distance, so there is no popping and no hand-made LOD meshes, and a cluster is dropped entirely if it is off-screen or hidden. Clusters smaller than a few pixels are rasterized in compute ("software raster") because the hardware raster is slow on tiny triangles.

## Design
1. **Cluster pipeline for static, repeated assets first**: tree species variants, rocks, props, window/door/building parts. These are few meshes drawn many times, so offline/at-startup cluster building pays off and instancing multiplies the gain (one cluster hierarchy, many instance transforms). Wind sway moves into the vertex stage (no skinning).
2. **Procedural per-chunk geometry (buildings, bushes baked meshes)**: build the cluster hierarchy on the worker thread when the chunk is generated (like tile generation today), and upload clusters instead of a flat mesh. Cost to measure: meshopt clustering+simplification time per chunk.
3. **Terrain**: do not use meshlets. Terrain is a heightfield, so use CDLOD/geometry clipmaps with GPU-sampled heightmap: triangle size tracks pixels from camera out to the horizon and from orbit. Same "one triangle per pixel" idea, no hierarchy build.
4. **Textures follow the same rule**: virtual texturing / mip streaming for terrain and ground (full-res near, coarse far), driven by screen footprint. Distant terrain color can come from the satellite imagery.
5. **Shadows**: near and mid distance cast real shadows (cascades; clusters are also culled for shadow views). Far objects get a cheap fake contact/blob shadow baked into the ground tint (a darkening decal or terrain-shadow-map term), not "no shadow". This is not an impostor of the object, only its shadow.
6. **Ground cover (grass, flowers)**: one tuft mesh per kind, one instance buffer per chunk, GPU culled and density-faded by distance (fewer instances, never swapped meshes). Removes the baked-mesh rebuilds.

## Open risks (to verify FIRST with a tiny prototype)
- Bevy 0.20's experimental meshlet feature on Metal: it needs GPU features (64-bit atomics for the visibility buffer) and I have not confirmed Apple GPU support. If it is not supported, the fallback is our own cluster culling with compute (large effort) or GPU-driven indirect draws with per-cluster frustum/occlusion culling and no software raster.
- Bevy meshlets have restrictions (static meshes, no skinning/morphs, limits on alpha-tested materials). Leaves/grass that rely on alpha cutouts need real geometry leaves (as Nanite does) or an exception path.
- Runtime cluster building cost for procedural chunks.

## Steps
1. **Attribute cost**: per-category triangle and draw counts in `perf.rs` plus GPU time; fixed camera shots (Zermatt, Central Park, high altitude). Gives the baseline for every later change.
2. **Feasibility prototype**: Bevy meshlet on this Mac with one tree species instanced ~100k times; measure fps and check visuals. Decision point: Bevy's pipeline vs own.
3. **Instancing + clusters for foliage/props/ground cover** (steps 1 and 6 above) since that is the largest known waste.
4. **Terrain CDLOD + texture streaming.**
5. **Buildings into clusters** (worker-built).
6. **Shadow strategy** (real near, fake far) once geometry is settled.

## Probe results (done: `crates/ed_game/src/bin/meshlet_probe.rs`, `--features meshlet`)
Run: `cargo run --release -p ed_game --features meshlet --bin meshlet_probe -- [--standard] [--grid N]`. M3 Pro, Metal, Bevy 0.20.1.
- Bevy's meshlet pipeline WORKS on this GPU (64-bit atomics etc. present). Conversion of a 33k-tri mesh: 0.12 s.
- 13M tris: both paths hit the 120 fps display cap. 121M tris: standard ~52 fps, meshlet ~120 (capped). 756M tris: standard 16.7 fps, meshlet ~120 (capped). So the technique scales as hoped.

## Hard limits of Bevy's meshlets (from its docs/source) that clash with our world
1. Vertex attributes: position, normal, UV only. No vertex colors (trees, buildings, props currently use them).
2. No custom vertex shader: GPU wind sway on foliage is impossible on this path.
3. Opaque only: no alpha-mask leaves or grass cards (foliage.rs uses `AlphaMode::Mask`).
4. Conversion is "slow, ahead of time, not for dynamically generated geometry". Fine for a species/prop library; heavy for per-chunk buildings.
5. Only forward OR deferred for opaque materials; MSAA off (already off).

## Revised track
- **Track A (now, no restrictions)**: true GPU instancing with our own shader for ground cover, bushes, trees (wind, vertex color, alpha stay). One mesh per kind + per-chunk instance buffer, distance thinning of density instead of baked-mesh swaps.
- **Track B (next)**: Bevy meshlets for opaque, static, library assets: rocks, props, building parts, tree TRUNKS/branches. Vertex color is replaced by a palette texture sampled via UV. Wind for trunks is dropped or done with a rigid per-instance sway.
- **Track C (engine direction, later)**: our own cluster renderer (compute culling + LOD, vertex color, custom vertex stage, alpha-tested leaves via real geometry), reusing what the probe teaches. Needed for procedural per-chunk buildings.

## Measured wins so far (Zermatt, 1280x720, M3 Pro; per-tile tris via EW_TILE_STATS=1)
- Baseline: 12 fps (84 ms), 30M visible tris. Breakdown: rocks 15M (417k rocks baked per tile, never culled), trees 12.6M, terrain 2.6M. Shadows cost ~17 ms, post ~0, and cost scales with resolution (tiny triangles -> 2x2 quad overdraw).
- Rocks chunked 16x16 per tile + `VisibilityRange` 400 m, none on coarse tiles: 31 fps.
- TEMPORARY scaffolding, to be replaced by Track C (user wants no discrete LODs): far trees in 3 density bands (full <800 m, 1-in-4 x1.6 to 2.5 km, 1-in-16 x2.6 beyond) via `VisibilityRange` in `foliage.rs`: 60 fps (vsync cap).
