# Far buildings (20 km) and Nanite-style options: research

Status: research only, 2026-11-05. Nothing here is implemented yet.

## How other engines show a city dozens of km away
1. **Hierarchical LOD tiles (3D Tiles / Cesium / photogrammetry).** A spatial tree; each tile has a
   screen-space-error (SSE) metric, the renderer draws a tile or descends to children. Raising the SSE
   budget pushes the detailed distance out at a draw-call cost. Cesium users fix far "pyramid" artefacts
   with more detail, which raises draw calls; the cheap fallback is instanced boxes for the skyline.
2. **Grid-cell tiling** (academic city renderers): buildings in a cell form one tile, loaded and drawn per cell (out-of-core).
3. **Proxy / merged meshes** (Unity ProxyMesh, Simplygon): many buildings retopologised into one mesh with a
   baked material; baked normals fake detail at mid range. Merging cuts draw calls.
4. **Octahedral impostors** (Epic Impostor Baker): fine for buildings too; the upper-hemisphere variant suits
   things sitting on terrain. We already have hemi-octahedral impostors for trees (draw mode 2).
5. **Mixed primitives** (Purdue): lines for far silhouettes, points mid, splats near.

Not found in search: MSFS internals, any 20 km shadow method. Far shadows are usually handled by
(a) a coarse last cascade, (b) baked/heightfield shadows for terrain (we have), (c) shadow proxies.

## Proposal for EveryWorld (native Metal)
- **Tier 0 (0-3 km):** current detailed buildings, 5 CSM cascades (building shadow range today 700 m).
- **Tier 1 (3-8 km):** merged per-tile extruded footprints, one draw per tile, no window geometry, albedo from a
  baked per-tile atlas colour; simple roof/wall shading.
- **Tier 2 (8-20 km):** per-tile single mesh of box-simplified massing (footprint extruded, collapsed by
  clustering) via `meshoptimizer` simplification, GPU-culled with indirect draws; optional impostor for landmarks.
- **Far shadows:** add a 6th very-coarse cascade for Tier 1, and for Tier 2 ray-march the heightfield plus a
  per-tile building-height grid (cheap, deterministic, multiplayer-safe). Terrain horizon shadows already exist.
- **Floor:** `ed_world/src/lod.rs` currently floors building LOD at 16 km; extend it with the tier table above.
- **Budget:** target <= 1500 draws total for buildings; Tier 1/2 via `drawIndexedPrimitives` indirect.

## Nanite-style virtual geometry
- No open-source Metal Nanite exists. Prior art: Nyx (D3D12), Vulcanite, jms55's Bevy virtual geometry,
  `meshoptimizer` `clusterlod.h` (cluster DAG + simplification), Metal 3 mesh shaders (WWDC22 10162).
- Mac path: cluster DAG built offline with `meshoptimizer`, per-cluster GPU culling in a compute pass, mesh
  shaders (Metal 3, Apple Silicon) for draw; software raster is not worth it on Apple GPUs.
- Windows path (later): same cluster data, D3D12 mesh shaders / Vulkan `VK_EXT_mesh_shader`.
- Recommendation: do not build full Nanite. Reuse cluster LOD only for buildings (Tier 1/2 above), where footprints
  simplify well; keep terrain on the existing clipmap/heightfield.

## Sources
- https://www.khronos.org/blog/liaison-between-khronos-and-open-geospatial-consortium-leads-to-3d-tiles-community-standard-built-on-gltf-for-streaming-massive-models
- https://community.cesium.com/t/lod-distance-rendering-problem-in-aerometrex-photogrammetry-layer/22247
- https://www.cjig.cn/en/article/doi/10.11834/jig.170501/
- https://docs.unity3d.com/Packages/com.unity.industry.toolkit@3.2/manual/lod-generator.html
- https://dev.epicgames.com/documentation/en-us/unreal-engine/impostor-baker-plugin-in-unreal-engine
- https://www.cs.purdue.edu/homes/bbenes/papers/Zhou14CAVW.pdf
