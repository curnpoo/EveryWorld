// tonemap pass: HDR scene target -> output target, with the alpha's tone mapper ported exactly
// (common.slang `alpha_tonemap`, P1-D18): exposure, white balance (neutral since M4f), the
// Filmic LUT, a highlight tint (neutral since M4f) and gamma 2.2. The alpha wrote that gamma-encoded value to a UNORM drawable;
// here a UNORM target gets it as is, and an *Srgb or float target (encoded by the display path)
// gets its sRGB-decoded value, so the bytes on screen are the alpha's either way.
#include "shader_abi.slang"
#include "common.slang"

struct ToneC {
    uint hdr;          // bindless texture index of the HDR target
    uint encode_srgb;  // 1 when the output format is not an *Srgb or float format (UNORM)
    float exposure;
    uint lut;          // bindless buffer: the Filmic LUT
    float4 viewport;   // x, y, w, h of the view in the output target
    uint ramp;         // 1: map the test ramp (tonemap.rs `test_ramp`) instead of `hdr`
    uint pad[7];
};
ED_CONSTANTS(ToneC)

[shader("vertex")]
FsOut vs_fullscreen(uint vid: SV_VulkanVertexID) {
    return fullscreen_vertex(vid);
}

float3 srgb_decode(float3 c) {
    return select(c <= 0.04045, c / 12.92, pow((c + 0.055) / 1.055, float3(2.4)));
}

// The tolerance-test input (mirrored by tonemap.rs `test_ramp`): 16 row bands (grey, R, G, B,
// then 12 saturated hues), each a 16-stop exposure ramp 2^-12 .. 2^4 left to right.
float3 test_ramp(uint2 p, float2 size) {
    const float3 BAND[16] = {
        float3(1, 1, 1), float3(1, 0, 0), float3(0, 1, 0), float3(0, 0, 1),
        float3(1, 1, 0), float3(0, 1, 1), float3(1, 0, 1), float3(1, 0.5, 0),
        float3(0.5, 1, 0), float3(0, 1, 0.5), float3(0, 0.5, 1), float3(0.5, 0, 1),
        float3(1, 0, 0.5), float3(1, 0.2, 0.1), float3(0.1, 0.6, 0.2), float3(0.2, 0.3, 1),
    };
    uint band = min(p.y * 16u / uint(size.y), 15u);
    float x = float(p.x) / max(size.x - 1.0, 1.0);
    return BAND[band] * exp2(-12.0 + 16.0 * x);
}

[shader("fragment")]
float4 fs_tonemap(FsOut i) : SV_Target0 {
    uint2 p = uint2(max(i.pos.xy - ed_c.viewport.xy, 0.0));
    float3 hdr = ed_c.ramp != 0 ? test_ramp(p, ed_c.viewport.zw) : ed_tex2d(ed_c.hdr).Load(int3(int2(p), 0)).rgb;
    float3 c = alpha_tonemap(ed_c.lut, hdr, ed_c.exposure);
    if (ed_c.encode_srgb == 0) {
        c = srgb_decode(c);
    }
    return float4(c, 1.0);
}
