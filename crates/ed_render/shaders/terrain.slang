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
    uint base;      // the list's first draw (the view's, or the cascade's culled list)
    uint pad[11];
};
ED_CONSTANTS(TerrainC)

struct TerrainOut {
    float4 pos: SV_Position;
    nointerpolation uint draw: DRAW;
    nointerpolation uint lod: LOD;
};

TerrainOut terrain_vs<let L : int>(uint vid, uint iid) {
    FrameData* f = ed_buf<FrameData>(ed_c.frame);
    TerrainDraw d = ed_buf<TerrainDraw>(ed_c.draws)[ed_c.base + iid];
    float3 p = t_vertex<L>(ed_c.pool, d, vid);
    TerrainOut o;
    o.pos = mul(f.view_proj, float4(p, 1.0));
    o.draw = ed_c.base + iid;
    o.lod = (d.slots.z >> T_LOD_SHIFT) & 3u;
    return o;
}

[shader("vertex")]
TerrainOut vs_terrain(uint vid: SV_VulkanVertexID, uint iid: SV_VulkanInstanceID) {
    return terrain_vs<0>(vid, iid);
}

// With the far forest rung's canopy lift (trees G3): a separate entry so `vs_terrain` keeps its
// exact code when the rung is off.
[shader("vertex")]
TerrainOut vs_terrain_trees(uint vid: SV_VulkanVertexID, uint iid: SV_VulkanInstanceID) {
    return terrain_vs<1>(vid, iid);
}

[shader("fragment")]
uint4 fs_terrain(TerrainOut i, uint prim: SV_PrimitiveID) : SV_Target0 {
    return uint4(TERRAIN_VIS_BIT | i.draw, (prim | (i.lod << T_PRIM_LOD_SHIFT)) & 0x0FFFFFFFu, 0, 0);
}

// Sun shadow caster: the same vertices into one cascade of the depth atlas (depth only, depth
// clamp pancakes casters in front of the cascade, as visibility.slang vs_shadow).
struct TerrainShadowOut {
    float4 pos: SV_Position;
};

TerrainShadowOut terrain_shadow_vs<let L : int>(uint vid, uint iid) {
    FrameData* f = ed_buf<FrameData>(ed_c.frame);
    TerrainDraw d = ed_buf<TerrainDraw>(ed_c.draws)[ed_c.base + iid];
    float3 p = t_vertex<L>(ed_c.pool, d, vid);
    TerrainShadowOut o;
    o.pos = mul(f.light.shadow[ed_c.cascade], float4(p, 1.0));
    return o;
}

[shader("vertex")]
TerrainShadowOut vs_terrain_shadow(uint vid: SV_VulkanVertexID, uint iid: SV_VulkanInstanceID) {
    return terrain_shadow_vs<0>(vid, iid);
}

[shader("vertex")]
TerrainShadowOut vs_terrain_shadow_trees(uint vid: SV_VulkanVertexID, uint iid: SV_VulkanInstanceID) {
    return terrain_shadow_vs<1>(vid, iid);
}

// ---- Grass shells (styles C and D) ---------------------------------------------------------
// One draw per near node (ed_c.base: its record, ed_c.cascade: its index LOD), the shells as
// instances (top first, so the floor shades least), no index buffer: vertex `vid` is corner
// vid % 3 of grid primitive vid / 3, so a triangle out of range or a shell this distance skips
// culls whole (every corner makes the same decision from the first vertex).
struct ShellOut {
    float4 pos: SV_Position;
    float2 wrel: WREL;
    float dz: DZ;
    nointerpolation uint draw: DRAW;
    nointerpolation uint lk: LK;   // lod | k << 8 | n << 16
};

[shader("vertex")]
ShellOut vs_shell(uint vid: SV_VulkanVertexID, uint iid: SV_VulkanInstanceID) {
    FrameData* f = ed_buf<FrameData>(ed_c.frame);
    TerrainDraw d = ed_buf<TerrainDraw>(ed_c.draws)[ed_c.base];
    uint n = max(f.shell.cfg.w, 1u);
    uint k = n - 1u - iid;
    uint lod = ed_c.cascade;
    uint prim = vid / 3u;
    uint3 tri = t_triangle(prim | (lod << T_PRIM_LOD_SHIFT));
    ShellOut o;
    o.pos = float4(0.0, 0.0, -1.0, 1.0);
    o.wrel = float2(0.0);
    o.dz = 0.0;
    o.draw = ed_c.base;
    o.lk = 0u;
    // Triangle distance from its first corner (unmorphed), less a cell and a half.
    float2 g0 = t_vertex_grid(tri.x);
    float3 dir;
    float3 p0 = d.origin.xyz + t_lattice(d, uint(g0.x), uint(g0.y), t_own_h(ed_c.pool, d, uint(g0.x), uint(g0.y)), dir);
    float R = f.shell.c.z;
    float dt = length(p0) - 1.5 * float(1u << lod) * d.shape.x * d.shape.y;
    // Fewer shells with distance: every shell near, every 2nd, then every 4th.
    uint step = dt < 0.3 * R ? 1u : (dt < 0.6 * R ? 2u : 4u);
    if (dt > R || (k != 0u && ((k + 1u) % step) != 0u)) {
        return o;
    }
    uint v = vid % 3u == 0u ? tri.x : (vid % 3u == 1u ? tri.y : tri.z);
    float3 p = t_vertex<0>(ed_c.pool, d, v);
    float3 up = normalize(p - (d.origin.xyz - d.dir_c.xyz * d.dir_c.w));
    float hk = f.shell.fu.w * float(k + 1u) / float(n);
    p += up * hk;
    o.pos = mul(f.view_proj, float4(p, 1.0));
    o.wrel = shell_wrel(f, d, t_vertex_grid(v));
    o.dz = -mul(f.view, float4(p, 1.0)).z;
    o.lk = lod | (k << 8) | (n << 16);
    return o;
}

[shader("fragment")]
uint4 fs_shell(ShellOut i, uint prim: SV_PrimitiveID) : SV_Target0 {
    FrameData* f = ed_buf<FrameData>(ed_c.frame);
    uint k = (i.lk >> 8) & 0xFFu;
    uint n = i.lk >> 16;
    uint lod = i.lk & 3u;
    // Pixel footprint (m on the ground) for the strand LOD, dithered per pixel.
    float px = sqrt(length(ddx(i.wrel)) * length(ddy(i.wrel)));
    uint2 pp = uint2(i.pos.xy);
    float dith = grass_rand(grass_hash(pp.x * 1973u + pp.y * 9277u));
    // The finest strand cell of the type decides the LOD; read it from the fine map.
    uint w = shell_map(f, ed_c.draws, 0u, int2(floor(i.wrel / f.shell.cells.x)));
    if ((w & 0xFFu) == 0u) {
        discard;
    }
    float sm = shell_type(f, ed_c.draws, (w & 0xFFu) - 1u).shell.y;
    uint L = shell_lod(f, sm, px, dith);
    ShellHit s = shell_strand(f, ed_c.draws, i.wrel, k, n, L, i.dz, false);
    if (!s.hit) {
        discard;
    }
    return uint4(TERRAIN_VIS_BIT | (L << SHELL_L_SHIFT) | i.draw, (prim | (lod << T_PRIM_LOD_SHIFT)) | (k << SHELL_K_SHIFT) | SHELL_BIT, 0, 0);
}
