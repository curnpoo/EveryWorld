#version 460
#extension GL_GOOGLE_include_directive : require
#include "shader_abi.glsl"
// Quad per instance: rect and colour from bindless buffer ed_c.inst.
struct Inst { vec4 rect; vec4 col; };
ED_CONSTANTS { uint inst; } ed_c;
ED_BUFFER(Inst)
layout(location = 0) flat out vec4 o_col;
layout(location = 1) out vec2 o_uv;
const vec2 Q[6] = vec2[](vec2(0, 0), vec2(1, 0), vec2(0, 1), vec2(0, 1), vec2(1, 0), vec2(1, 1));
void main() {
    Inst d = ed_buf(Inst, ed_c.inst)[gl_InstanceIndex];
    vec2 q = Q[gl_VertexIndex];
    gl_Position = vec4(mix(d.rect.xy, d.rect.zw, q), 0.5, 1.0);
    o_col = d.col;
    o_uv = q;
}
