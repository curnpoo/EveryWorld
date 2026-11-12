#version 460
#extension GL_GOOGLE_include_directive : require
#include "shader_abi.glsl"
// Triangle ed_c.tri (or the instance index when ed_c.tri is ~0) at depth ed_c.z.
ED_CONSTANTS { vec4 col; float z; uint tri; } ed_c;
layout(location = 0) flat out vec4 o_col;
layout(location = 1) out vec2 o_uv;
const vec2 TRIS[12] = vec2[](
    vec2(-1, -1), vec2(1, -1), vec2(-1, 1),   // 0: bottom-left half, CCW
    vec2(1, 1), vec2(-1, 1), vec2(1, -1),     // 1: top-right half, CCW
    vec2(1, 1), vec2(1, -1), vec2(-1, 1),     // 2: top-right half, CW
    vec2(-3, -1), vec2(3, -1), vec2(0, 3));   // 3: full screen, CCW
void main() {
    uint iid = uint(gl_InstanceIndex);
    uint t = ed_c.tri == 0xffffffffu ? iid : ed_c.tri;
    gl_Position = vec4(TRIS[t * 3u + uint(gl_VertexIndex) % 3u], ed_c.z, 1.0);
    o_col = ed_c.tri == 0xffffffffu ? (iid == 0u ? vec4(1, 0, 0, 1) : vec4(0, 1, 0, 1)) : ed_c.col;
    o_uv = vec2(0);
}
