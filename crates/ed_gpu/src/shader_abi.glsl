// ed_gpu bindless ABI for SPIR-V (Vulkan backend), hand-written GLSL side.
// Include after `#version 460`. Mirrors MSL_PRELUDE: see `ed_gpu::shader_abi` for the table.
//
//   push constants          64 bytes of draw or compute constants (ED_CONSTANTS)
//   set 0, binding 0        texture table, sampled images (one index space with binding 1)
//   set 0, binding 1        texture table, storage images (only for shader_write textures)
//   set 0, binding 2        buffer table, storage buffers (one copy of the set per frame slot)
//   set 0, binding 3        sampler table
//
// Accessors wrap every index in nonuniformEXT, so indices read from buffers are legal.
#extension GL_EXT_nonuniform_qualifier : require
#extension GL_EXT_samplerless_texture_functions : require
// Storage images without a format qualifier (shaderStorageImageRead/WriteWithoutFormat).
#extension GL_EXT_shader_image_load_formatted : require

layout(set = 0, binding = 0) uniform texture2D ed_textures_2d[];
layout(set = 0, binding = 0) uniform texture2DArray ed_textures_2d_array[];
layout(set = 0, binding = 0) uniform textureCube ed_textures_cube[];
layout(set = 0, binding = 0) uniform texture3D ed_textures_3d[];
layout(set = 0, binding = 1) uniform image2D ed_images_2d[];
layout(set = 0, binding = 3) uniform sampler ed_samplers[];

// `ED_CONSTANTS { uint a; float b; } ed_c;` declares the 64-byte constant block.
#define ED_CONSTANTS layout(push_constant, std430) uniform EdConstants
// `ED_BUFFER(T)` once per element type T, then `ed_buf(T, i)[j]` reads or writes element j.
#define ED_BUFFER(T) layout(set = 0, binding = 2, std430) buffer EdBuf_##T { T data[]; } ed_buf_##T[];
#define ed_buf(T, i) ed_buf_##T[nonuniformEXT(i)].data

#define ed_tex2d(i) ed_textures_2d[nonuniformEXT(i)]
#define ed_tex2d_array(i) ed_textures_2d_array[nonuniformEXT(i)]
#define ed_texcube(i) ed_textures_cube[nonuniformEXT(i)]
#define ed_tex3d(i) ed_textures_3d[nonuniformEXT(i)]
#define ed_rwtex2d(i) ed_images_2d[nonuniformEXT(i)]
#define ed_smp(i) ed_samplers[nonuniformEXT(i)]
