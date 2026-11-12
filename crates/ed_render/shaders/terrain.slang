// Terrain raster (Phase 2 W3): the shared node mesh, one instance per drawn node, into the
// visibility buffer (x: TERRAIN_VIS_BIT | draw, y: primitive) and into the sun shadow atlas.
// Vertices are pulled: SV_VulkanVertexID is the shared index buffer's value (0..4480), the
// instance is the draw index (first instance 0).
#include "shader_abi.slang"
#include "common.slang"
#include "terrain_common.slang"

struct TerrainC {
    uint frame;
    uint draws;     // TerrainDraw table
    uint pool;      // height page pool (u16 pairs)
    uint cascade;   // vs_terrain_shadow: which cascade matrix
    uint base;      // vs_terrain_shadow: the cascade's first draw (its culled list)
    uint pad[11];
};
ED_CONSTANTS(TerrainC)

struct TerrainOut {
    float4 pos: SV_Position;
    nointerpolation uint draw: DRAW;
};

[shader("vertex")]
TerrainOut vs_terrain(uint vid: SV_VulkanVertexID, uint iid: SV_VulkanInstanceID) {
    FrameData* f = ed_buf<FrameData>(ed_c.frame);
    TerrainDraw d = ed_buf<TerrainDraw>(ed_c.draws)[iid];
    float3 p = t_vertex(ed_c.pool, d, vid);
    TerrainOut o;
    o.pos = mul(f.view_proj, float4(p, 1.0));
    o.draw = iid;
    return o;
}

[shader("fragment")]
uint4 fs_terrain(TerrainOut i, uint prim: SV_PrimitiveID) : SV_Target0 {
    return uint4(TERRAIN_VIS_BIT | i.draw, prim & 0x0FFFFFFFu, 0, 0);
}

// Sun shadow caster: the same vertices into one cascade of the depth atlas (depth only, depth
// clamp pancakes casters in front of the cascade, as visibility.slang vs_shadow).
struct TerrainShadowOut {
    float4 pos: SV_Position;
};

[shader("vertex")]
TerrainShadowOut vs_terrain_shadow(uint vid: SV_VulkanVertexID, uint iid: SV_VulkanInstanceID) {
    FrameData* f = ed_buf<FrameData>(ed_c.frame);
    TerrainDraw d = ed_buf<TerrainDraw>(ed_c.draws)[ed_c.base + iid];
    float3 p = t_vertex(ed_c.pool, d, vid);
    TerrainShadowOut o;
    o.pos = mul(f.light.shadow[ed_c.cascade], float4(p, 1.0));
    return o;
}
