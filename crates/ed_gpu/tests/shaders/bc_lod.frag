#version 460
#extension GL_GOOGLE_include_directive : require
#include "shader_abi.glsl"
ED_CONSTANTS { uint tex; uint smp; float lod; } ed_c;
layout(location = 1) in vec2 i_uv;
layout(location = 0) out vec4 o;
void main() {
    vec4 c = textureLod(sampler2D(ed_tex2d(ed_c.tex), ed_smp(ed_c.smp)), i_uv, ed_c.lod);
    o = vec4(c.rgb, 1.0);
}
