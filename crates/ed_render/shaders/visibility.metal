// Visibility-buffer raster (renderer.md §3): depth plus (instance + 1, LOD << 28 | primitive).
// Vertices are pulled from the shared vertex buffer; [[vertex_id]] already includes the draw's
// base vertex and [[instance_id]] its first instance (the bin's offset into the visible list).

struct VisC {
    uint frame;
    uint instances;
    uint nodes;
    uint visible;
    uint verts;
    uint pad[11];
};

struct VisOut {
    float4 pos [[position]];
    uint entry [[flat]];
};

struct VisTarget {
    uint4 v [[color(0)]];
};

vertex VisOut vs_vis(uint vid [[vertex_id]], uint iid [[instance_id]], ED_BINDLESS(VisC)) {
    device const FrameData& f = *ed_buf(FrameData, ed_c.frame);
    uint entry = ed_buf(uint, ed_c.visible)[iid];
    GpuInstance inst = ed_buf(GpuInstance, ed_c.instances)[entry & ENTRY_INSTANCE_MASK];
    Vtx v = ed_buf(Vtx, ed_c.verts)[vid];
    float3 node = ed_buf(float4, ed_c.nodes)[inst.node_flags & 0xFFFFu].xyz;
    float3 p = node + xform_point(inst, float3(v.px, v.py, v.pz));
    VisOut o;
    o.pos = f.view_proj * float4(p, 1.0);
    o.entry = entry;
    return o;
}

inline VisTarget vis_value(uint entry, uint prim) {
    VisTarget t;
    uint lod = (entry >> 20) & 3u;
    t.v = uint4((entry & ENTRY_INSTANCE_MASK) + 1u, (lod << 28) | (prim & 0x0FFFFFFFu), 0, 0);
    return t;
}

fragment VisTarget fs_vis(VisOut in [[stage_in]], uint prim [[primitive_id]], ED_BINDLESS(VisC)) {
    return vis_value(in.entry, prim);
}

// 8x8 ordered dither, values 0..63.
constant uchar BAYER8[64] = {
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
fragment VisTarget fs_vis_dither(VisOut in [[stage_in]], uint prim [[primitive_id]], ED_BINDLESS(VisC)) {
    uint2 p = uint2(in.pos.xy) & 7u;
    uint inst = in.entry & ENTRY_INSTANCE_MASK;
    uint rot = (inst * 0x9E3779B1u) >> 26;  // per-instance offset so neighbours do not line up
    uint d = ((uint(BAYER8[p.y * 8u + p.x]) + rot) & 63u) * 4u + 2u;
    uint cov = (in.entry >> 22) & 0xFFu;
    bool side_prev = (in.entry >> 30) & 1u;
    if ((d < cov) == side_prev) {
        discard_fragment();
    }
    return vis_value(in.entry, prim);
}
