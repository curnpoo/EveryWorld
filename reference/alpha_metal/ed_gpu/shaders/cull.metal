// GPU-driven frustum culling. Shared contract with crates/ed_gpu/src/cull.rs:
// identical struct layouts (Instance 32 B, CullParams 112 B, DrawIndexedIndirect 20 B) and the
// same sphere-vs-plane test. cull_cpu() in cull.rs is the reference.
//
// Output order is NOT deterministic on the GPU: visible draws are appended with an atomic
// counter, so threads race for slots. The set of draws is deterministic; the order is not.
// Consumers must not rely on order. Use cull_cpu() when a stable order is needed.
//
// Host contract:
//   * Zero `counter` before each dispatch.
//   * After the dispatch, the number of valid draws is min(*counter, max_draws). The counter
//     keeps counting past max_draws so the host can detect overflow.
//   * Dispatch with threadgroup size 64, grid rounded up to a multiple of 64.

#include <metal_stdlib>
using namespace metal;

// Layout must match ed_gpu::cull::Instance (32 bytes).
struct Instance {
    packed_float3 center;   // packed so the struct stays 32 bytes, as on the Rust side
    float radius;
    uint mesh;
    uint first_index;
    uint index_count;
    int base_vertex;
};

// Layout must match ed_gpu::cull::CullParams (112 bytes).
struct CullParams {
    float4 planes[6];       // inward-facing, normalised: dot(n, p) + d >= 0 is inside
    uint instance_count;
    uint max_draws;
    uint2 _pad;
};

// MTLDrawIndexedPrimitivesIndirectArguments field order. Matches ed_gpu::cull::DrawIndexedIndirect.
struct DrawIndexedIndirect {
    uint indexCount;
    uint instanceCount;
    uint indexStart;
    int baseVertex;
    uint baseInstance;      // first_instance on the Rust side
};

kernel void cull_instances(
    constant CullParams&        params   [[buffer(0)]],
    const device Instance*      instances [[buffer(1)]],
    device DrawIndexedIndirect* draws    [[buffer(2)]],
    device atomic_uint*         counter  [[buffer(3)]],
    uint gid [[thread_position_in_grid]])
[[max_total_threads_per_threadgroup(64)]]
{
    // Bounds check: the grid is rounded up to a multiple of 64.
    if (gid >= params.instance_count) {
        return;
    }

    const Instance inst = instances[gid];
    const float3 c = float3(inst.center);

    // Same test as sphere_visible() in cull.rs.
    for (uint i = 0; i < 6; ++i) {
        const float4 p = params.planes[i];
        const float dist = dot(p.xyz, c) + p.w;
        if (dist < -inst.radius) {
            return;
        }
    }

    // Append. The slot is only written when it is below max_draws, so the output buffer is
    // never written out of bounds.
    const uint slot = atomic_fetch_add_explicit(counter, 1u, memory_order_relaxed);
    if (slot < params.max_draws) {
        DrawIndexedIndirect d;
        d.indexCount = inst.index_count;
        d.instanceCount = 1u;
        d.indexStart = inst.first_index;
        d.baseVertex = inst.base_vertex;
        d.baseInstance = gid;
        draws[slot] = d;
    }
}
