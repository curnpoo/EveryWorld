#version 460
#extension GL_GOOGLE_include_directive : require
#include "shader_abi.glsl"
// Left half samples the texture named by table entry 0 of buffer ed_c.sel, right half entry 1.
ED_CONSTANTS { uint sel; uint smp; } ed_c;
ED_BUFFER(uint)
layout(location = 1) in vec2 i_uv;
layout(location = 0) out vec4 o;
void main() {
    uint t = i_uv.x < 0.5 ? ed_buf(uint, ed_c.sel)[0] : ed_buf(uint, ed_c.sel)[1];
    o = texture(sampler2D(ed_tex2d(t), ed_smp(ed_c.smp)), i_uv);
}
