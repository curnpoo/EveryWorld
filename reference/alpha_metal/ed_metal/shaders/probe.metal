#include <metal_stdlib>
using namespace metal;

struct VOut { float4 pos [[position]]; float3 color; };

vertex VOut probe_vs(uint vid [[vertex_id]], constant float &t [[buffer(0)]]) {
    float2 p[3] = { float2(0.0, 0.7), float2(-0.7, -0.6), float2(0.7, -0.6) };
    float3 c[3] = { float3(1,0.3,0.2), float3(0.2,1,0.4), float3(0.3,0.5,1) };
    float s = sin(t), co = cos(t);
    float2 q = float2(p[vid].x * co - p[vid].y * s, p[vid].x * s + p[vid].y * co);
    VOut o; o.pos = float4(q, 0, 1); o.color = c[vid]; return o;
}

fragment float4 probe_fs(VOut in [[stage_in]]) { return float4(in.color, 1); }
