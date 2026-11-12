// egui painter (crates/ed_editor/src/painter.rs, P1-D14). Vertices (20 bytes: float2 pos,
// float2 uv, RGBA8 colour) and u32 indices live in one ring buffer at byte offsets `vtx` and
// `idx` (multiples of 4), pulled through the bindless buffer table. Colours are egui's
// premultiplied gamma-space values; converted to linear only for linear targets.
#include "shader_abi.slang"

struct EguiC {
    float2 screen;
    uint buf;
    uint vtx;          // byte offset of this mesh's vertices
    uint idx;          // byte offset of this draw's indices
    uint tex;
    uint smp;
    uint linear_out;   // 1 when the target stores linear values (*Srgb and float formats)
    uint pad[8];
};
ED_CONSTANTS(EguiC)

struct EguiOut { float4 pos: SV_Position; float4 color: COLOR; float2 uv: TEXCOORD0; };

float3 egui_linear_from_gamma(float3 s) {
    float3 lo = s / 12.92;
    float3 hi = pow((s + 0.055) / 1.055, float3(2.4));
    return select(s < 0.04045, lo, hi);
}

[shader("vertex")]
EguiOut vs_egui(uint vid: SV_VulkanVertexID) {
    uint* base = ed_buf<uint>(ed_c.buf);
    uint i = base[ed_c.idx / 4 + vid];
    // Vertex words: pos.x, pos.y, uv.x, uv.y, colour. Read as words: a pointer to a 20-byte
    // struct reads zeros on MoltenVK (SPIR-V is valid; the MSL translation gets the stride wrong).
    uint* v = base + ed_c.vtx / 4 + i * 5;
    float2 pos = float2(asfloat(v[0]), asfloat(v[1]));
    uint color = v[4];
    EguiOut o;
    o.pos = float4(2.0 * pos.x / ed_c.screen.x - 1.0, 1.0 - 2.0 * pos.y / ed_c.screen.y, 0.0, 1.0);
    o.color = float4(color & 0xFFu, (color >> 8) & 0xFFu, (color >> 16) & 0xFFu, color >> 24) / 255.0; // premultiplied, gamma
    o.uv = float2(asfloat(v[2]), asfloat(v[3]));
    return o;
}

[shader("fragment")]
float4 fs_egui(EguiOut i) : SV_Target0 {
    // Textures hold egui's gamma-space premultiplied colours in a Unorm format, so sampling
    // returns gamma values and filtering happens in gamma space, as egui expects.
    float4 c = i.color * ed_tex2d(ed_c.tex).Sample(ed_smp(ed_c.smp), i.uv);
    if (ed_c.linear_out != 0) {
        c.rgb = egui_linear_from_gamma(c.rgb);
    }
    return c;
}
