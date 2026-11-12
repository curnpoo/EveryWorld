// ed_gpu bindless ABI for Slang (P1-D1): the one shader source for Metal (MSL) and Vulkan
// (SPIR-V). `#include "shader_abi.slang"` first (slangc -I crates/ed_gpu/src), then declare
// the constants with `ED_CONSTANTS(MyConstants)` (at most 64 bytes).
//
// Compile with `-DED_METAL` for `-target metal` and `-DED_SPIRV` for `-target spirv`
// (tools/shaders/build.sh does both; see `ed_gpu::shader_abi` for the table layouts).
//
//   Metal                                  Vulkan
//   buffer(0)  64 bytes of constants       push constants (64 bytes, std430)
//   buffer(1)  texture table (MTLResourceID) set 0 binding 0 sampled images, binding 1 storage images
//   buffer(2)  buffer table (u64 GPU address) set 0 binding 4: u64 buffer device addresses
//   buffer(3)  sampler table               set 0 binding 3 samplers
//
// Buffers are pointers on both backends: `ed_buf<T>(i)` is a `T*` (MSL `device T*`, SPIR-V
// PhysicalStorageBuffer). Pointer data uses Slang's natural layout (scalar alignment, no
// padding to 16 bytes), so keep every struct read through a pointer a multiple of 16 bytes
// with vec4-aligned members, as `#[repr(C)]` on the Rust side expects.
//
// Textures: the table is untyped; the accessor names the view type and must match the
// texture behind index i (as with MSL_PRELUDE and GLSL_PRELUDE). Indices may be non-uniform.

#if !defined(ED_METAL) && !defined(ED_SPIRV)
#error "compile with -DED_METAL (target metal) or -DED_SPIRV (target spirv)"
#endif

#ifdef ED_METAL

#define ED_CONSTANTS(T) ConstantBuffer<T> ed_c : register(b0);
// All three tables are arrays of 8-byte entries (MTLResourceID, GPU address, MTLResourceID).
// MSL forbids texture or sampler pointee types on entry-point parameters, so the tables are
// declared as `ulong*` and each accessor reinterprets its entry as the view it names.
[[vk::binding(1, 0)]] StructuredBuffer<uint64_t> ed_textures : register(t1);
[[vk::binding(2, 0)]] StructuredBuffer<uint64_t> ed_buffers : register(t2);
[[vk::binding(3, 0)]] StructuredBuffer<uint64_t> ed_samplers : register(t3);

Texture2D ed_tex2d_(StructuredBuffer<uint64_t> t, uint i) {
    __target_switch { case metal: __intrinsic_asm "(reinterpret_cast<device texture2d<float, access::sample>*>($0)[$1])"; }
}
Texture2D<uint4> ed_tex2d_uint_(StructuredBuffer<uint64_t> t, uint i) {
    __target_switch { case metal: __intrinsic_asm "(reinterpret_cast<device texture2d<uint, access::sample>*>($0)[$1])"; }
}
Texture2DArray ed_tex2d_array_(StructuredBuffer<uint64_t> t, uint i) {
    __target_switch { case metal: __intrinsic_asm "(reinterpret_cast<device texture2d_array<float, access::sample>*>($0)[$1])"; }
}
TextureCube ed_texcube_(StructuredBuffer<uint64_t> t, uint i) {
    __target_switch { case metal: __intrinsic_asm "(reinterpret_cast<device texturecube<float, access::sample>*>($0)[$1])"; }
}
Texture3D ed_tex3d_(StructuredBuffer<uint64_t> t, uint i) {
    __target_switch { case metal: __intrinsic_asm "(reinterpret_cast<device texture3d<float, access::sample>*>($0)[$1])"; }
}
RWTexture2D<float4> ed_rwtex2d_(StructuredBuffer<uint64_t> t, uint i) {
    __target_switch { case metal: __intrinsic_asm "(reinterpret_cast<device texture2d<float, access::read_write>*>($0)[$1])"; }
}
float ed_depth_read_(StructuredBuffer<uint64_t> t, uint i, uint2 p) {
    __target_switch { case metal: __intrinsic_asm "(reinterpret_cast<device depth2d<float, access::sample>*>($0)[$1].read($2))"; }
}
SamplerState ed_smp_(StructuredBuffer<uint64_t> t, uint i) {
    __target_switch { case metal: __intrinsic_asm "(reinterpret_cast<device sampler*>($0)[$1])"; }
}

Texture2D ed_tex2d(uint i) { return ed_tex2d_(ed_textures, i); }
Texture2D<uint4> ed_tex2d_uint(uint i) { return ed_tex2d_uint_(ed_textures, i); }
Texture2DArray ed_tex2d_array(uint i) { return ed_tex2d_array_(ed_textures, i); }
TextureCube ed_texcube(uint i) { return ed_texcube_(ed_textures, i); }
Texture3D ed_tex3d(uint i) { return ed_tex3d_(ed_textures, i); }
RWTexture2D<float4> ed_rwtex2d(uint i) { return ed_rwtex2d_(ed_textures, i); }
/// Depth (texel `p`, mip 0) of a depth texture.
float ed_depth_read(uint i, uint2 p) { return ed_depth_read_(ed_textures, i, p); }
SamplerState ed_smp(uint i) { return ed_smp_(ed_samplers, i); }

#else // ED_SPIRV

#define ED_CONSTANTS(T) [[vk::push_constant]] ConstantBuffer<T> ed_c;
[[vk::binding(0, 0)]] Texture2D ed_textures_2d[];
[[vk::binding(0, 0)]] Texture2D<uint4> ed_textures_2d_uint[];
[[vk::binding(0, 0)]] Texture2D<float> ed_textures_depth[];
[[vk::binding(0, 0)]] Texture2DArray ed_textures_2d_array[];
[[vk::binding(0, 0)]] TextureCube ed_textures_cube[];
[[vk::binding(0, 0)]] Texture3D ed_textures_3d[];
[[vk::binding(1, 0)]] RWTexture2D<float4> ed_images_2d[];
[[vk::binding(3, 0)]] SamplerState ed_samplers[];
[[vk::binding(4, 0)]] StructuredBuffer<uint64_t> ed_buffers;

Texture2D ed_tex2d(uint i) { return ed_textures_2d[NonUniformResourceIndex(i)]; }
Texture2D<uint4> ed_tex2d_uint(uint i) { return ed_textures_2d_uint[NonUniformResourceIndex(i)]; }
Texture2DArray ed_tex2d_array(uint i) { return ed_textures_2d_array[NonUniformResourceIndex(i)]; }
TextureCube ed_texcube(uint i) { return ed_textures_cube[NonUniformResourceIndex(i)]; }
Texture3D ed_tex3d(uint i) { return ed_textures_3d[NonUniformResourceIndex(i)]; }
RWTexture2D<float4> ed_rwtex2d(uint i) { return ed_images_2d[NonUniformResourceIndex(i)]; }
/// Depth (texel `p`, mip 0) of a depth texture.
float ed_depth_read(uint i, uint2 p) { return ed_textures_depth[NonUniformResourceIndex(i)].Load(int3(int2(p), 0)); }
SamplerState ed_smp(uint i) { return ed_samplers[NonUniformResourceIndex(i)]; }

#endif

/// Buffer table entry `i` as a `T*` (read and write; may be non-uniform).
T* ed_buf<T>(uint i) { return (T*)ed_buffers[i]; }
