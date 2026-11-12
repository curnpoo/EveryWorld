#version 460
#extension GL_GOOGLE_include_directive : require
#include "shader_abi.glsl"
// Full-screen triangle, uv in 0..1 with v down.
layout(location = 0) flat out vec4 o_col;
layout(location = 1) out vec2 o_uv;
void main() {
    uint vid = uint(gl_VertexIndex);
    vec2 p = vec2((vid << 1) & 2u, vid & 2u) * 2.0 - 1.0; // (-1,-1) (3,-1) (-1,3): CCW
    gl_Position = vec4(p, 0.5, 1.0);
    o_col = vec4(0);
    o_uv = vec2(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
}
