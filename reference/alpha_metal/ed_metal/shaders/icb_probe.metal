#include <metal_stdlib>
using namespace metal;

struct Inst { packed_float3 center; float radius; uint mesh; uint first_index; uint index_count; int base_vertex; };
struct Frame { float4 planes[6]; float4x4 vp; uint count; uint max_draws; uint2 pad; };
struct Container { command_buffer cmd [[id(0)]]; };

// One thread per instance: frustum test, then encode an indexed draw into the ICB at a compacted slot.
kernel void cull_encode(uint gid [[thread_position_in_grid]],
                        constant Frame& F [[buffer(0)]],
                        device const Inst* insts [[buffer(1)]],
                        device atomic_uint* counter [[buffer(2)]],
                        device const ushort* indices [[buffer(3)]],
                        device const packed_float3* verts [[buffer(4)]],
                        constant Container& C [[buffer(5)]]) {
    if (gid >= F.count) return;
    Inst in = insts[gid];
    float3 c = in.center;
    for (int i = 0; i < 6; i++) {
        if (dot(F.planes[i].xyz, c) + F.planes[i].w < -in.radius) return;
    }
    uint slot = atomic_fetch_add_explicit(counter, 1, memory_order_relaxed);
    if (slot >= F.max_draws) return;
    render_command cmd(C.cmd, slot);
    cmd.set_vertex_buffer(verts, 0);
    cmd.set_vertex_buffer(insts, 1);
    cmd.set_vertex_buffer(&F, 2);
    cmd.draw_indexed_primitives(primitive_type::triangle, in.index_count, indices + in.first_index, 1, in.base_vertex, gid);
}

struct VOut { float4 pos [[position]]; float3 color; };

vertex VOut icb_vs(uint vid [[vertex_id]], uint iid [[instance_id]], uint base [[base_instance]],
                   device const packed_float3* verts [[buffer(0)]],
                   device const Inst* insts [[buffer(1)]],
                   constant Frame& F [[buffer(2)]]) {
    Inst in = insts[base + iid];
    float3 p = float3(verts[vid]) * in.radius * 0.7 + float3(in.center);
    VOut o;
    o.pos = F.vp * float4(p, 1);
    float h = clamp(in.center.y / 60.0, 0.0, 1.0);
    o.color = mix(float3(0.2, 0.5, 0.3), float3(0.9, 0.85, 0.8), h) * (0.55 + 0.45 * float(verts[vid].y > 0.0));
    return o;
}

fragment float4 icb_fs(VOut in [[stage_in]]) { return float4(in.color, 1); }
