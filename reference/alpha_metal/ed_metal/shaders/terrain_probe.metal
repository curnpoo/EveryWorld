#include <metal_stdlib>
using namespace metal;

struct Block { float2 offset; float scale; float level; };
struct U { float4x4 vp; float2 centre; float2 pad; float3 sun; float pad2; };

float hash(float2 p) { return fract(sin(dot(p, float2(127.1, 311.7))) * 43758.5453); }
float vnoise(float2 p) {
    float2 i = floor(p), f = fract(p); f = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + float2(1, 0)), f.x), mix(hash(i + float2(0, 1)), hash(i + float2(1, 1)), f.x), f.y);
}
float height(float2 p) {
    float h = 0.0, a = 900.0, fr = 1.0 / 6000.0;
    for (int o = 0; o < 6; o++) { h += (vnoise(p * fr) - 0.5) * 2.0 * a; a *= 0.5; fr *= 2.0; }
    return max(h, -40.0);
}

struct VOut { float4 pos [[position]]; float3 color; };

vertex VOut terrain_vs(uint vid [[vertex_id]], uint iid [[instance_id]],
                       device const float2* verts [[buffer(0)]],
                       device const Block* blocks [[buffer(1)]],
                       constant U& u [[buffer(2)]]) {
    Block b = blocks[iid];
    float2 xz = u.centre + b.offset + verts[vid] * b.scale;
    float h = height(xz);
    float e = max(b.scale, 1.0);
    float3 n = normalize(float3(height(xz - float2(e, 0)) - height(xz + float2(e, 0)), 2.0 * e, height(xz - float2(0, e)) - height(xz + float2(0, e))));
    float d = clamp(dot(n, u.sun), 0.0, 1.0);
    float3 base = mix(float3(0.28, 0.42, 0.22), float3(0.62, 0.58, 0.54), smoothstep(150.0, 700.0, h));
    base = mix(base, float3(0.95), smoothstep(900.0, 1300.0, h));
    VOut o; o.pos = u.vp * float4(xz.x, h, xz.y, 1);
    o.color = base * (0.25 + 0.85 * d) * (0.85 + 0.15 * fract(b.level * 0.37));
    return o;
}
fragment float4 terrain_fs(VOut in [[stage_in]]) { return float4(in.color, 1); }
