// A/B probe for cluster rendering (crates/ed_metal/src/bin/cluster_ab.rs).
// Path A (baseline): one drawIndexed per instance over the whole building, CPU-culled.
// Path B (clusters): compute cull over (instance, cluster) pairs with frustum + backface cone,
// then one mesh-shader threadgroup per visible pair. Shared fragment shader.
// Layouts must match the Rust structs in cluster_ab.rs and ed_gpu::cluster::Cluster (48 bytes).

#include <metal_stdlib>
#include <metal_mesh>
using namespace metal;

struct Frame {
    float4 planes[6];
    float4x4 vp;
    float4 cam;            // xyz camera position, w = 1 when the backface cone test is on
    uint instances;
    uint clusters;         // clusters per instance
    uint max_pairs;
    uint pad;
    float threshold_px;
    float screen_height;
    float fov_y;
    float slope;
    uint pad2;
};

struct Cluster {
    packed_float3 center;
    float radius;
    packed_float3 axis;
    float cutoff;
    uint first_index;      // u16 units into the cluster index array
    uint tri_count;
    int base_vertex;
    uint pad;
    float error;
    float parent_error;
};

struct Inst {
    packed_float3 offset;
    float pad;
};

struct VOut {
    float4 pos [[position]];
    float3 nrm;
    float4 col;
};

kernel void cluster_cull(constant Frame& F [[buffer(0)]],
                         device const Cluster* cl [[buffer(1)]],
                         device const Inst* insts [[buffer(2)]],
                         device uint2* pairs [[buffer(3)]],
                         device atomic_uint* counter [[buffer(4)]],
                         device atomic_uint* tris [[buffer(5)]],
                         uint gid [[thread_position_in_grid]]) {
    if (gid >= F.instances * F.clusters) return;
    uint ii = gid / F.clusters;
    uint ci = gid % F.clusters;
    Cluster c = cl[ci];
    float3 center = float3(c.center) + float3(insts[ii].offset);
    for (int i = 0; i < 6; i++) {
        if (dot(F.planes[i].xyz, center) + F.planes[i].w < -c.radius) return;
    }
    if (F.cam.w > 0.5 && c.cutoff < 1.0) {
        float3 v = center - F.cam.xyz;
        float len2 = dot(v, v);
        if (len2 > c.radius * c.radius && dot(v, float3(c.axis)) / sqrt(len2) >= c.cutoff) return;
    }
    // LOD selection: draw this cluster iff its own projected error <= threshold
    // AND its parent's projected error > threshold.
    {
        float dx = center.x - F.cam.x;
        float dy = center.y - F.cam.y;
        float dz = center.z - F.cam.z;
        float dist = sqrt(dx*dx + dy*dy + dz*dz);
        // LOD selection using a precomputed slope so GPU and CPU agree (avoids tan precision).
        if (dist <= 0.0) return;
        if (!(c.error <= F.slope * dist && c.parent_error > F.slope * dist)) return;
    }
    uint slot = atomic_fetch_add_explicit(counter, 1u, memory_order_relaxed);
    if (slot < F.max_pairs) pairs[slot] = uint2(ii, ci);
    atomic_fetch_add_explicit(tris, c.tri_count, memory_order_relaxed);
}

// Threadgroups per grid dimension are capped at 65535, so visible pairs wrap into rows.
kernel void cluster_finish(constant Frame& F [[buffer(0)]],
                           device atomic_uint* counter [[buffer(1)]],
                           device uint* indirect [[buffer(2)]]) {
    uint n = min(atomic_load_explicit(counter, memory_order_relaxed), F.max_pairs);
    indirect[0] = min(n, 65535u);
    indirect[1] = (n + 65534u) / 65535u;
    indirect[2] = 1;
}

using MeshOut = metal::mesh<VOut, void, 128, 128, metal::topology::triangle>;

[[mesh]] void cluster_mesh(MeshOut m,
                           constant Frame& F [[buffer(0)]],
                           device const uint2* pairs [[buffer(1)]],
                           device const Inst* insts [[buffer(2)]],
                           device const Cluster* cl [[buffer(3)]],
                           device const packed_float3* vpos [[buffer(4)]],
                           device const packed_float3* vnrm [[buffer(5)]],
                           device const float4* vcol [[buffer(6)]],
                           device const ushort* lidx [[buffer(7)]],
                           device atomic_uint* counter [[buffer(8)]],
                           uint3 tgp [[threadgroup_position_in_grid]],
                           uint lid [[thread_index_in_threadgroup]]) {
    uint tg = tgp.y * 65535u + tgp.x;
    uint n = min(atomic_load_explicit(counter, memory_order_relaxed), F.max_pairs);
    if (tg >= n) {
        m.set_primitive_count(0);
        return;
    }
    threadgroup atomic_uint nvt;
    uint2 p = pairs[tg];
    Cluster c = cl[p.y];
    float3 off = float3(insts[p.x].offset);
    if (lid == 0) atomic_store_explicit(&nvt, 0u, memory_order_relaxed);
    threadgroup_barrier(mem_flags::mem_threadgroup);
    for (uint t = lid; t < c.tri_count; t += 128) {
        uint b = c.first_index + 3 * t;
        uint mx = max(uint(lidx[b]), max(uint(lidx[b + 1]), uint(lidx[b + 2])));
        atomic_fetch_max_explicit(&nvt, mx + 1u, memory_order_relaxed);
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    uint nv = atomic_load_explicit(&nvt, memory_order_relaxed);
    m.set_primitive_count(c.tri_count);
    for (uint v = lid; v < nv; v += 128) {
        uint g = uint(c.base_vertex) + v;
        VOut o;
        o.pos = F.vp * float4(float3(vpos[g]) + off, 1.0);
        o.nrm = float3(vnrm[g]);
        o.col = vcol[g];
        m.set_vertex(v, o);
    }
    for (uint t = lid; t < c.tri_count; t += 128) {
        uint b = c.first_index + 3 * t;
        m.set_index(3 * t, uint(lidx[b]));
        m.set_index(3 * t + 1, uint(lidx[b + 1]));
        m.set_index(3 * t + 2, uint(lidx[b + 2]));
    }
}

vertex VOut base_vs(uint vid [[vertex_id]],
                    device const packed_float3* pos [[buffer(0)]],
                    device const packed_float3* nrm [[buffer(1)]],
                    device const float4* col [[buffer(2)]],
                    constant float4& off [[buffer(3)]],
                    constant Frame& F [[buffer(4)]]) {
    VOut o;
    o.pos = F.vp * float4(float3(pos[vid]) + off.xyz, 1.0);
    o.nrm = float3(nrm[vid]);
    o.col = col[vid];
    return o;
}

fragment float4 shade_fs(VOut in [[stage_in]]) {
    float ndl = clamp(dot(normalize(in.nrm), normalize(float3(0.4, 0.8, 0.3))), 0.0, 1.0);
    return float4(in.col.rgb * (0.35 + 0.65 * ndl), 1.0);
}
