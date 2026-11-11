// tonemap pass: HDR scene target -> output target.
//
// PLACEHOLDER: the curve is the Narkowicz (2015) ACES fit. It is replaced by the alpha's
// tone mapper, ported and compared against the reference capture (P1-D18, W3).

struct ToneC {
    uint hdr;          // bindless texture index of the HDR target
    uint encode_srgb;  // 1 when the output format is not an *Srgb format
    float exposure;
    uint pad0;
    float4 viewport;   // x, y, w, h of the view in the output target
    uint pad[8];
};

vertex FsOut vs_fullscreen(uint vid [[vertex_id]], ED_BINDLESS(ToneC)) {
    return fullscreen_vertex(vid);
}

inline float3 aces_fit(float3 x) {
    return saturate((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14));
}

inline float3 srgb_encode(float3 c) {
    return select(1.055 * pow(c, 1.0 / 2.4) - 0.055, c * 12.92, c <= 0.0031308);
}

fragment float4 fs_tonemap(FsOut in [[stage_in]], ED_BINDLESS(ToneC)) {
    uint2 p = uint2(max(in.pos.xy - ed_c.viewport.xy, 0.0));
    float3 hdr = ed_tex2d(ed_c.hdr).read(p).rgb;
    float3 c = aces_fit(hdr * ed_c.exposure);
    if (ed_c.encode_srgb != 0) {
        c = srgb_encode(c);
    }
    return float4(c, 1.0);
}
