// Skinning pre-pass (M3): one thread per vertex of one skinned mesh. Reads the bind-pose
// source (SkinVtx) and this frame's joint palette (3x4 rows per joint, mesh space), writes
// the skinned vertex into the mesh's own range of the shared vertex buffer, which provider 1
// then draws like any mesh. Linear blend skinning; normals use the same matrices (uniform
// scale joints; non-uniform joint scale would need the cofactor, not needed yet).

struct SkinC {
    uint src;       // bindless: SkinVtx buffer
    uint dst;       // bindless: Vtx buffer (shared vertices)
    uint palette;   // bindless: float4 rows, 3 per joint
    uint count;     // vertices
    uint src_base;  // first SkinVtx
    uint dst_base;  // first Vtx
    uint pal_base;  // first joint in the palette
    uint pad[9];
};

kernel void skin(uint id [[thread_position_in_grid]], ED_BINDLESS(SkinC)) {
    if (id >= ed_c.count) {
        return;
    }
    SkinVtx s = ed_buf(SkinVtx, ed_c.src)[ed_c.src_base + id];
    device const float4* pal = ed_buf(float4, ed_c.palette) + ed_c.pal_base * 3u;
    uint j[4] = { s.j01 & 0xFFFFu, s.j01 >> 16, s.j23 & 0xFFFFu, s.j23 >> 16 };
    float3 bp = float3(s.px, s.py, s.pz);
    float3 bn = oct_decode(s.n);
    float3 p = float3(0.0);
    float3 n = float3(0.0);
    float wsum = 0.0;
    for (uint k = 0; k < 4; k++) {
        float w = s.w[k];
        if (w <= 0.0) {
            continue;
        }
        float4 r0 = pal[j[k] * 3u];
        float4 r1 = pal[j[k] * 3u + 1u];
        float4 r2 = pal[j[k] * 3u + 2u];
        p += w * float3(dot(r0.xyz, bp) + r0.w, dot(r1.xyz, bp) + r1.w, dot(r2.xyz, bp) + r2.w);
        n += w * float3(dot(r0.xyz, bn), dot(r1.xyz, bn), dot(r2.xyz, bn));
        wsum += w;
    }
    if (wsum <= 0.0) {
        p = bp;
        n = bn;
    } else {
        p /= wsum;
    }
    device Vtx* o = ed_buf(Vtx, ed_c.dst) + ed_c.dst_base + id;
    o->px = p.x;
    o->py = p.y;
    o->pz = p.z;
    o->n = oct_encode(length_squared(n) > 0.0 ? normalize(n) : bn);
}
