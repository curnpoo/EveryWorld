// overlay pass: editor gizmo/selection lines and triangles and physics debug draw, drawn on
// the output target after tonemap. No depth test, alpha blended.
//
// Vertices are 16 bytes: float3 position relative to the overlay origin, then RGBA8 colour
// (sRGB, straight alpha). `offset` = overlay origin - camera position (f32 of an f64
// difference, P1-D7), so the same batch draws correctly in every view.
#include "shader_abi.slang"
#include "common.slang"

struct OverlayC {
    uint frame;        // bindless index of FrameData (this view's)
    uint verts;        // bindless index of the overlay vertex buffer
    uint first;        // first vertex of this draw
    uint linear_out;   // 1 when the target stores linear values (*Srgb and float formats)
    float4 offset;     // xyz: overlay origin relative to the camera
    uint pad[8];
};
ED_CONSTANTS(OverlayC)

struct OverlayV { float px; float py; float pz; uint color; };
struct OverlayOut { float4 pos: SV_Position; float4 color: COLOR; };

float3 overlay_linear_from_gamma(float3 s) {
    float3 lo = s / 12.92;
    float3 hi = pow((s + 0.055) / 1.055, float3(2.4));
    return select(s < 0.04045, lo, hi);
}

[shader("vertex")]
OverlayOut vs_overlay(uint vid: SV_VulkanVertexID) {
    FrameData* fd = ed_buf<FrameData>(ed_c.frame);
    OverlayV v = ed_buf<OverlayV>(ed_c.verts)[ed_c.first + vid];
    OverlayOut o;
    float3 p = float3(v.px, v.py, v.pz) + ed_c.offset.xyz;
    o.pos = mul(fd.view_proj, float4(p, 1.0));
    o.color = unpack_unorm4x8(v.color);
    return o;
}

[shader("fragment")]
float4 fs_overlay(OverlayOut i) : SV_Target0 {
    float4 c = i.color;
    if (ed_c.linear_out != 0) {
        c.rgb = overlay_linear_from_gamma(c.rgb);
    }
    return c;
}
