#version 460
#extension GL_GOOGLE_include_directive : require
#include "shader_abi.glsl"
layout(location = 0) flat in vec4 i_col;
layout(location = 0) out vec4 o;
void main() { o = i_col; }
