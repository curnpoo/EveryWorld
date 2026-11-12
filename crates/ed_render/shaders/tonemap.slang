// tonemap pass: HDR scene target -> output target.
//
// PLACEHOLDER: the curve is the Narkowicz (2015) ACES fit. It is replaced by the alpha's
// tone mapper, ported and compared against the reference capture (P1-D18, W3).
#include "shader_abi.slang"
#include "common.slang"

struct ToneC {
    uint hdr;          // bindless texture index of the HDR target
    uint encode_srgb;  // 1 when the output format is not an *Srgb format
    float exposure;
    uint pad0;
    float4 viewport;   // x, y, w, h of the view in the output target
    uint pad[8];
};
ED_CONSTANTS(ToneC)

[shader("vertex")]
FsOut vs_fullscreen(uint vid: SV_VulkanVertexID) {
    return fullscreen_vertex(vid);
}

float3 srgb_encode(float3 c) {
    return select(c <= 0.0031308, c * 12.92, 1.055 * pow(c, 1.0 / 2.4) - 0.055);
}

[shader("fragment")]
float4 fs_tonemap(FsOut i) : SV_Target0 {
    uint2 p = uint2(max(i.pos.xy - ed_c.viewport.xy, 0.0));
    float3 hdr = ed_tex2d(ed_c.hdr).Load(int3(int2(p), 0)).rgb;
    float3 c = aces_fit(hdr * ed_c.exposure);
    if (ed_c.encode_srgb != 0) {
        c = srgb_encode(c);
    }
    return float4(c, 1.0);
}
