#include <metal_stdlib>
using namespace metal;

struct MU { float4x4 inv_cur; float4x4 prev; float4 size; };

// Camera-only motion vectors (UV units, previous minus current) from reverse-Z depth. MetalFX is told the
// scale (-w, -h), so this becomes pixel motion. Moving objects ghost slightly (no per-object velocity yet).
kernel void motion_cs(depth2d<float, access::read> dep [[texture(0)]], texture2d<float, access::write> mv [[texture(1)]],
                      constant MU& U [[buffer(0)]], uint2 gid [[thread_position_in_grid]]) {
    if (gid.x >= mv.get_width() || gid.y >= mv.get_height()) return;
    float d = max(dep.read(gid), 1e-7);
    float2 uv = (float2(gid) + 0.5) * U.size.zw;
    float2 ndc = float2(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    float4 w = U.inv_cur * float4(ndc, d, 1.0);
    w /= w.w;
    float4 pc = U.prev * float4(w.xyz, 1.0);
    float2 pn = pc.xy / pc.w;
    float2 puv = float2(pn.x * 0.5 + 0.5, 0.5 - pn.y * 0.5);
    mv.write(float4(puv - uv, 0.0, 0.0), gid);
}
