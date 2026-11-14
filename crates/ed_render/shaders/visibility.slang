// Visibility-buffer raster (renderer.md §3): depth plus (instance + 1, LOD << 28 | primitive).
// Vertices are pulled from the shared vertex buffer; SV_VulkanVertexID already includes the
// draw's base vertex and SV_VulkanInstanceID its first instance (the bin's offset into the
// visible list), on both backends.
#include "shader_abi.slang"
#include "common.slang"

struct VisC {
    uint frame;
    uint instances;
    uint nodes;
    uint visible;
    uint verts;
    uint cascade;   // vs_shadow: which cascade matrix
    uint materials; // foliage: material table, per-vertex uv words, sampler
    uint uvs;
    uint smp;
    uint pad[7];
};
ED_CONSTANTS(VisC)

struct VisOut {
    float4 pos: SV_Position;
    nointerpolation uint entry: ENTRY;
};

[shader("vertex")]
VisOut vs_vis(uint vid: SV_VulkanVertexID, uint iid: SV_VulkanInstanceID) {
    FrameData* f = ed_buf<FrameData>(ed_c.frame);
    uint entry = ed_buf<uint>(ed_c.visible)[iid];
    GpuInstance inst = ed_buf<GpuInstance>(ed_c.instances)[entry & ENTRY_INSTANCE_MASK];
    Vtx v = ed_buf<Vtx>(ed_c.verts)[vid];
    float3 node = ed_buf<float4>(ed_c.nodes)[inst.node_flags & 0xFFFFu].xyz;
    float3 p = node + xform_point(inst, float3(v.px, v.py, v.pz));
    VisOut o;
    o.pos = mul(f.view_proj, float4(p, 1.0));
    o.entry = entry;
    return o;
}

uint4 vis_value(uint entry, uint prim) {
    uint lod = (entry >> 20) & 3u;
    return uint4((entry & ENTRY_INSTANCE_MASK) + 1u, (lod << 28) | (prim & 0x0FFFFFFFu), 0, 0);
}

[shader("fragment")]
uint4 fs_vis(VisOut i, uint prim: SV_PrimitiveID) : SV_Target0 {
    return vis_value(i.entry, prim);
}

// 8x8 ordered dither, values 0..63.
static const uint BAYER8[64] = {
     0, 32,  8, 40,  2, 34, 10, 42,
    48, 16, 56, 24, 50, 18, 58, 26,
    12, 44,  4, 36, 14, 46,  6, 38,
    60, 28, 52, 20, 62, 30, 54, 22,
     3, 35, 11, 43,  1, 33,  9, 41,
    51, 19, 59, 27, 49, 17, 57, 25,
    15, 47,  7, 39, 13, 45,  5, 37,
    63, 31, 55, 23, 61, 29, 53, 21,
};

// LOD cross-fade: the two levels of a fading instance draw complementary pixel sets. The
// current level keeps pixels whose threshold is under the coverage, the previous level the rest.
[shader("fragment")]
uint4 fs_vis_dither(VisOut i, uint prim: SV_PrimitiveID) : SV_Target0 {
    uint2 p = uint2(i.pos.xy) & 7u;
    uint inst = i.entry & ENTRY_INSTANCE_MASK;
    uint rot = (inst * 0x9E3779B1u) >> 26;  // per-instance offset so neighbours do not line up
    uint d = ((BAYER8[p.y * 8u + p.x] + rot) & 63u) * 4u + 2u;
    uint cov = (i.entry >> 22) & 0xFFu;
    bool side_prev = ((i.entry >> 30) & 1u) != 0;
    if ((d < cov) == side_prev) {
        discard;
    }
    return vis_value(i.entry, prim);
}

// Sun shadow caster (M4d): the same pulled vertices (after skinning, so skinned instances cast)
// into one cascade of the depth atlas, depth only. Light depth grows towards the sun (cleared to
// 0, GreaterEqual). Casters between the sun and the cascade volume are clamped onto its sun-side
// face ("pancaking") by the pipeline's depth clamp, per fragment: clamping vertex z instead
// bends the depth plane of large triangles (terrain, ground slabs) and makes them self-shadow.
struct ShadowOut {
    float4 pos: SV_Position;
};

[shader("vertex")]
ShadowOut vs_shadow(uint vid: SV_VulkanVertexID, uint iid: SV_VulkanInstanceID) {
    FrameData* f = ed_buf<FrameData>(ed_c.frame);
    uint entry = ed_buf<uint>(ed_c.visible)[iid];
    GpuInstance inst = ed_buf<GpuInstance>(ed_c.instances)[entry & ENTRY_INSTANCE_MASK];
    Vtx v = ed_buf<Vtx>(ed_c.verts)[vid];
    float3 node = ed_buf<float4>(ed_c.nodes)[inst.node_flags & 0xFFFFu].xyz;
    float3 p = node + xform_point(inst, float3(v.px, v.py, v.pz));
    ShadowOut o;
    o.pos = mul(f.light.shadow[ed_c.cascade], float4(p, 1.0));
    return o;
}

// Trees (G4): bank meshes (MESH_FOLIAGE) draw with these. Wind moves the vertices
// (common.slang `tree_vertex`); leaf cards are alpha-tested against the leaf atlas.
struct FoliageOut {
    float4 pos: SV_Position;
    float2 uv: TEXCOORD0;
    nointerpolation uint entry: ENTRY;
    nointerpolation uint tex: TEXID;   // atlas (bindless), NO_TEXTURE: opaque (bark, shells)
    nointerpolation float2 sd: SEASON; // season: this card's hash and the share of leaves on the tree
};

FoliageOut foliage_vertex(uint vid, uint iid, bool shadow) {
    FrameData* f = ed_buf<FrameData>(ed_c.frame);
    uint entry = ed_buf<uint>(ed_c.visible)[iid];
    GpuInstance inst = ed_buf<GpuInstance>(ed_c.instances)[entry & ENTRY_INSTANCE_MASK];
    Vtx v = ed_buf<Vtx>(ed_c.verts)[vid];
    uint2 w = ed_buf<uint2>(ed_c.uvs)[vid];
    float3 node = ed_buf<float4>(ed_c.nodes)[inst.node_flags & 0xFFFFu].xyz;
    float3 p = node + tree_vertex(f, inst, node, float3(v.px, v.py, v.pz), v.n, w.y);
    FoliageOut o;
    o.pos = mul(shadow ? f.light.shadow[ed_c.cascade] : f.view_proj, float4(p, 1.0));
    o.uv = tree_uv(w.x);
    o.entry = entry;
    o.tex = (w.y & AUX_CARD) != 0u ? ed_buf<GpuMaterial>(ed_c.materials)[inst.material].tex.x : NO_TEXTURE;
    o.sd = float2(0.0, 1.0);
    if ((w.y & AUX_CARD) != 0u && f.season.cfg.x > 0.0) {
        LeafSeason ls = season_of_word(f, inst.pad[1]);
        // Per-card hash: the spray's phase and branch group, the tree's phase, the card (4 vertices).
        float h = float((w.y >> 21) & 63u) * (1.0 / 64.0) + float(w.y >> 29) * 0.137 + float(inst.pad[0] >> 24) * (0.618 / 255.0) + frac(float(vid >> 2) * 0.7548776662);
        o.sd = float2(frac(h), ls.leaves);
    }
    return o;
}

[shader("vertex")]
FoliageOut vs_foliage(uint vid: SV_VulkanVertexID, uint iid: SV_VulkanInstanceID) {
    return foliage_vertex(vid, iid, false);
}

[shader("vertex")]
FoliageOut vs_shadow_foliage(uint vid: SV_VulkanVertexID, uint iid: SV_VulkanInstanceID) {
    return foliage_vertex(vid, iid, true);
}

void foliage_clip(FoliageOut i) {
    if (i.sd.x >= i.sd.y) {
        discard;
    }
    if (i.tex != NO_TEXTURE && ed_tex2d(i.tex).Sample(ed_smp(ed_c.smp), i.uv).a < 0.5) {
        discard;
    }
}

[shader("fragment")]
uint4 fs_foliage(FoliageOut i, uint prim: SV_PrimitiveID) : SV_Target0 {
    foliage_clip(i);
    return vis_value(i.entry, prim);
}

[shader("fragment")]
uint4 fs_foliage_dither(FoliageOut i, uint prim: SV_PrimitiveID) : SV_Target0 {
    foliage_clip(i);
    uint2 p = uint2(i.pos.xy) & 7u;
    uint inst = i.entry & ENTRY_INSTANCE_MASK;
    uint rot = (inst * 0x9E3779B1u) >> 26;
    uint d = ((BAYER8[p.y * 8u + p.x] + rot) & 63u) * 4u + 2u;
    uint cov = (i.entry >> 22) & 0xFFu;
    bool side_prev = ((i.entry >> 30) & 1u) != 0;
    if ((d < cov) == side_prev) {
        discard;
    }
    return vis_value(i.entry, prim);
}

[shader("fragment")]
void fs_shadow_foliage(FoliageOut i) {
    foliage_clip(i);
}

// ---- Tree impostors (trees G5, src/impostor.rs) ---------------------------------------------
// One camera-facing quad per visible tree (6 indices, the corner from the vertex id), its
// three nearest octahedral frames blended; coverage is dithered (the mean coverage of the
// real tree) times the entry's fade, and depth is offset by the baked depth. VisC words:
// instances = tree buffer, verts = kind table, visible = the impostor visible list.
static const uint IMP_ENTRY_MASK = 0xFFFFFFu;

struct ImpOut {
    float4 pos: SV_Position;
    float2 uv0: TEXCOORD0;
    float2 uv1: TEXCOORD1;
    float2 uv2: TEXCOORD2;
    float zv: TEXCOORD3;                    // view depth of the quad point (m); 0 for shadows
    nointerpolation uint entry: ENTRY;
    nointerpolation uint4 fi: FRAMES;       // xyz: the three frames, w: texture A (bindless)
    nointerpolation float4 w: WEIGHTS;      // xyz: their weights, w: depth scale (m per unit)
    nointerpolation float2 geo: GEO;        // frames per side, frame px
    nointerpolation float front: FRONT;     // how far the quad was moved towards the viewer (m)
    nointerpolation float lv: LEAVES;       // season: share of the coverage left (bare crowns keep their branches)
};

ImpOut imp_vertex(uint vid, uint iid, bool shadow) {
    FrameData* f = ed_buf<FrameData>(ed_c.frame);
    uint entry = ed_buf<uint>(ed_c.visible)[iid];
    GpuTree g = ed_buf<GpuTree>(ed_c.instances)[entry & IMP_ENTRY_MASK];
    GpuImpKind k = ed_buf<GpuImpKind>(ed_c.verts)[(g.node_kind >> 12) & 0xFFFu];
    float3 node = ed_buf<float4>(ed_c.nodes)[g.node_kind & 0xFFFu].xyz;
    float3 l = f.sun_dir.xyz;
    ImpTree t = imp_tree(g, k, node, l, shadow);
    float3 vw = shadow ? l : normalize(-t.c);
    float3 u0 = t.up - vw * dot(vw, t.up);
    float3 bu = dot(u0, u0) > 1e-6 ? normalize(u0) : t.ex;
    float3 br = normalize(cross(bu, vw));
    float pad = f.imp.cfg.y;
    float half_t = t.r * pad;
    float half_w = half_t * t.s * max(t.wd, 1.0);
    // Across, the quad spans the widest leaf (a 2 % pad), not the bounding sphere: a narrow
    // conifer's quad loses over half its (discarded) pixels.
    float crown_max = float(asuint(k.bark.w) >> 21) / 2047.0 * 1.5 * k.geo.z;
    float half_x = crown_max > 0.0 ? min(half_w, crown_max * 1.02 * pad * t.s * t.wd) : half_w;
    uint corner = vid & 3u;
    float2 cxy = float2((corner & 1u) != 0u ? 1.0 : -1.0, (corner & 2u) != 0u ? 1.0 : -1.0);
    float3 p = t.c + br * (cxy.x * half_x) + bu * (cxy.y * half_w);
    float3 q = imp_to_tree(t, p - t.c);
    // View: the quad stands at the front of the bounding sphere, so the per-pixel depth only
    // ever moves away from the viewer (conservative depth: early depth rejection stays on).
    float front = 0.0;
    if (!shadow) {
        front = min(half_w, max(length(t.c) - f.params.z * 2.0, 0.0));
        p -= vw * front;
        // Ground pull: along the corner's own view ray (the picture does not move), so the
        // coarser terrain drawn at distance does not bury trees standing on the finest ground.
        float ppr = f.proj.w * 0.5 * f.proj.y;
        float pull = float(f.imp.bufs.w >> 8) / 16.0 * length(t.c) / max(ppr, 1.0);
        float lp = length(p);
        p *= max(lp - pull, min(lp, f.params.z * 2.0)) / max(lp, 1e-6);
    }
    ImpOut o;
    o.pos = mul(shadow ? f.light.shadow[ed_c.cascade] : f.view_proj, float4(p, 1.0));
    o.uv0 = imp_frame_uv(t, t.d0, q, half_t);
    o.uv1 = imp_frame_uv(t, t.d1, q, half_t);
    o.uv2 = imp_frame_uv(t, t.d2, q, half_t);
    o.zv = shadow ? 0.0 : -mul(f.view, float4(p, 1.0)).z;
    o.entry = entry;
    o.fi = uint4(t.fi, k.tex.x);
    o.w = float4(t.w, 2.0 * half_t * t.s);
    o.front = front;
    o.geo = float2(float(k.tex.w), k.geo.w);
    o.lv = 1.0;
    if (f.season.cfg.x > 0.0) {
        o.lv = lerp(0.22, 1.0, season_of_word(f, g.season).leaves);
    }
    return o;
}

[shader("vertex")]
ImpOut vs_imp(uint vid: SV_VulkanVertexID, uint iid: SV_VulkanInstanceID) {
    return imp_vertex(vid, iid, false);
}

[shader("vertex")]
ImpOut vs_imp_shadow(uint vid: SV_VulkanVertexID, uint iid: SV_VulkanInstanceID) {
    return imp_vertex(vid, iid, true);
}

// Far-cascade shadow caster (trees G5): an opaque diamond spanning the crown as the sun sees
// it (crown base to tip along the projected up, crown radius across), in the plane through
// the trunk facing the sun. Depth only, no fragment stage. Fading trees shrink with coverage.
[shader("vertex")]
float4 vs_imp_shadow_proxy(uint vid: SV_VulkanVertexID, uint iid: SV_VulkanInstanceID) : SV_Position {
    FrameData* f = ed_buf<FrameData>(ed_c.frame);
    uint entry = ed_buf<uint>(ed_c.visible)[iid];
    GpuTree g = ed_buf<GpuTree>(ed_c.instances)[entry & IMP_ENTRY_MASK];
    GpuImpKind k = ed_buf<GpuImpKind>(ed_c.verts)[(g.node_kind >> 12) & 0xFFFu];
    float3 node = ed_buf<float4>(ed_c.nodes)[g.node_kind & 0xFFFu].xyz;
    float3 l = f.sun_dir.xyz;
    float3 up = oct_decode(g.up);
    float s = g.scale;
    float wd = 0.5 + float(g.yaw_widen >> 16) / 65535.0;
    float h = k.geo.z;
    uint cw = asuint(k.bark.w);
    float cb = float(cw & 0x3FFu) / 1023.0 * h * s;
    float cr = float((cw >> 10) & 0x7FFu) / 2047.0 * 1.5 * h * s * wd;
    float sin_el = abs(dot(l, up));
    float3 u0 = up - l * dot(l, up);
    float cos_el = length(u0);
    float3 bu = cos_el > 1e-3 ? u0 / cos_el : float3(1.0, 0.0, 0.0);
    float3 br = normalize(cross(bu, l));
    float yt = h * s * cos_el + 0.5 * cr * sin_el;
    float yb = cb * cos_el - cr * sin_el;
    float ym = 0.5 * (yt + yb);
    // Diamond area matched to a round crown's (x1.12); coverage fades shrink it.
    float k_s = 1.12 * sqrt(float(entry >> 24) / 255.0);
    float yw = lerp(yb, yt, 0.4);
    uint corner = vid & 3u;
    float2 q = corner == 0u ? float2(-cr, yw) : corner == 1u ? float2(0.0, yb) : corner == 2u ? float2(0.0, yt) : float2(cr, yw);
    q = float2(q.x * k_s, ym + (q.y - ym) * k_s);
    float3 p = node + float3(g.px, g.py, g.pz) + bu * q.y + br * q.x;
    return mul(f.light.shadow[ed_c.cascade], float4(p, 1.0));
}

// Blended coverage of the three frames (dithered against `th`) and the depth offset (m,
// towards the viewer). Discards uncovered pixels.
float imp_coverage(ImpOut i) {
    Texture2D a = ed_tex2d(i.fi.w);
    SamplerState smp = ed_smp(ed_c.smp);
    uint n = uint(i.geo.x);
    float2 t0 = imp_atlas_uv(i.fi.x, n, i.uv0, i.geo.y);
    float2 t1 = imp_atlas_uv(i.fi.y, n, i.uv1, i.geo.y);
    float2 t2 = imp_atlas_uv(i.fi.z, n, i.uv2, i.geo.y);
    float2 s0 = a.Sample(smp, t0).rg * (imp_inside(i.uv0) ? 1.0 : 0.0);
    float2 s1 = a.Sample(smp, t1).rg * (imp_inside(i.uv1) ? 1.0 : 0.0);
    float2 s2 = a.Sample(smp, t2).rg * (imp_inside(i.uv2) ? 1.0 : 0.0);
    float cov = i.w.x * s0.x + i.w.y * s1.x + i.w.z * s2.x;
    // Soft alpha test: dithered between 0.25 and 0.75 coverage, solid above.
    float c = saturate((cov - 0.5) * 2.0 + 0.5) * float(i.entry >> 24) / 255.0 * i.lv;
    uint2 p = uint2(i.pos.xy) & 7u;
    uint rot = ((i.entry & IMP_ENTRY_MASK) * 0x9E3779B1u) >> 26;
    float th = (float((BAYER8[p.y * 8u + p.x] + rot) & 63u) + 0.5) / 64.0;
    if (c <= th) {
        discard;
    }
    float wd = i.w.x * s0.x + i.w.y * s1.x + i.w.z * s2.x;
    float d = i.w.x * s0.x * s0.y + i.w.y * s1.x * s1.y + i.w.z * s2.x * s2.y;
    return wd > 1e-4 ? (d / wd - 0.5) * i.w.w : 0.0;
}

struct ImpDepthOut {
    uint4 vis: SV_Target0;
    float depth: SV_DepthLessEqual;   // reverse Z: never nearer than the quad
};

[shader("fragment")]
ImpDepthOut fs_imp(ImpOut i) {
    FrameData* f = ed_buf<FrameData>(ed_c.frame);
    float dz = imp_coverage(i);
    float near = f.params.z;
    ImpDepthOut o;
    o.vis = uint4(IMP_VIS_BIT | (i.entry & IMP_ENTRY_MASK), 0u, 0u, 0u);
    o.depth = min(near / max(i.zv + max(i.front - dz, 0.0), near), i.pos.z);
    return o;
}

// Shadow casters: the heaviest frame alone, coverage only, the quad's own depth (no depth
// export, so early depth stays on: trees G5: view (a) shadow raster fell with it).
[shader("fragment")]
void fs_imp_shadow(ImpOut i) {
    Texture2D a = ed_tex2d(i.fi.w);
    SamplerState smp = ed_smp(ed_c.smp);
    uint n = uint(i.geo.x);
    uint fr = i.fi.x;
    float2 uv = i.uv0;
    if (i.w.y > i.w.x && i.w.y >= i.w.z) {
        fr = i.fi.y;
        uv = i.uv1;
    } else if (i.w.z > i.w.x && i.w.z > i.w.y) {
        fr = i.fi.z;
        uv = i.uv2;
    }
    float cov = imp_inside(uv) ? a.Sample(smp, imp_atlas_uv(fr, n, uv, i.geo.y)).r : 0.0;
    float c = saturate((cov - 0.5) * 2.0 + 0.5) * float(i.entry >> 24) / 255.0 * i.lv;
    uint2 p = uint2(i.pos.xy) & 7u;
    uint rot = ((i.entry & IMP_ENTRY_MASK) * 0x9E3779B1u) >> 26;
    float th = (float((BAYER8[p.y * 8u + p.x] + rot) & 63u) + 0.5) / 64.0;
    if (c <= th) {
        discard;
    }
}

// ---- Grass style A (src/grass.rs) -----------------------------------------------------------
// No vertex buffer: the bucket's index buffer strips blade slots; vertex id -> (blade, segment,
// side). VisC words: instances = clump buffer, verts = type table, visible = the list,
// cascade = the bucket. vis.x = GRASS_VIS_BIT | clump; vis.y = blade | t (unorm12) << 7 |
// u (unorm8) << 19.
struct GrassOut {
    float4 pos: SV_Position;
    float2 tu: TEXCOORD0;                   // along the blade, across it
    nointerpolation uint clump: ENTRY;
    nointerpolation uint blade: BLADE;
};

// Vertex `vid` of the bucket's blade strip. `shadow`: the nearest cascade, half the blades.
GrassOut grass_vertex(uint vid, uint iid, bool shadow) {
    FrameData* f = ed_buf<FrameData>(ed_c.frame);
    uint ci = ed_buf<uint>(ed_c.visible)[iid];
    GrassClump c = ed_buf<GrassClump>(ed_c.instances)[ci];
    GrassType ty = ed_buf<GrassType>(ed_c.verts)[(c.node_kind >> 12) & 0xFFu];
    float3 node = ed_buf<float4>(ed_c.nodes)[c.node_kind & 0xFFFu].xyz;
    uint bkt = min(ed_c.cascade, GRASS_BUCKETS - 1u);
    uint segs = GRASS_SEGS[bkt];
    uint vpb = 2u * (segs + 1u);
    uint k = vid / vpb;
    uint r = vid - k * vpb;
    uint seg = r >> 1;
    float side = float(r & 1u);
    GrassBlade b = grass_blade(f, c, ty, node, k, shadow ? 0.5 : 1.0);
    float t = float(seg) / float(segs);
    float3 p = grass_point(b, t);
    float3 sd = shadow ? b.side : grass_view_side(b, t, p);
    float w = grass_width(b, t);
    p += sd * ((side - 0.5) * w);
    GrassOut o;
    o.pos = b.h > 0.0 ? mul(shadow ? f.light.shadow[0] : f.view_proj, float4(p, 1.0)) : float4(0.0, 0.0, -1.0, 1.0);
    o.tu = float2(t, side);
    o.clump = ci;
    o.blade = k;
    return o;
}

[shader("vertex")]
GrassOut vs_grass(uint vid: SV_VulkanVertexID, uint iid: SV_VulkanInstanceID) {
    return grass_vertex(vid, iid, false);
}

[shader("vertex")]
float4 vs_grass_shadow(uint vid: SV_VulkanVertexID, uint iid: SV_VulkanInstanceID) : SV_Position {
    return grass_vertex(vid, iid, true).pos;
}

[shader("fragment")]
uint4 fs_grass(GrassOut i) : SV_Target0 {
    uint t = uint(saturate(i.tu.x) * 4095.0 + 0.5);
    uint u = uint(saturate(i.tu.y) * 255.0 + 0.5);
    return uint4(GRASS_VIS_BIT | (i.clump & GRASS_INDEX_MASK), (i.blade & 127u) | (t << 7) | (u << 19), 0u, 0u);
}
