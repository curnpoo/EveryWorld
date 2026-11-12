//! Shader-side binding ABI: where a shader finds the draw constants and the bindless tables.
//!
//! Every pipeline sees the same four bindings, set by the device; there are no other binding
//! points (contract section 1). On Metal they are buffer slots shared by the vertex,
//! fragment and kernel stages:
//!
//! | Slot | Content |
//! |---|---|
//! | `buffer(0)` | 64 bytes of draw or compute constants |
//! | `buffer(1)` | texture table: one texture handle per [`crate::BindlessTexture`] index |
//! | `buffer(2)` | buffer table: one device pointer per [`crate::BindlessBuffer`] index |
//! | `buffer(3)` | sampler table: one sampler per [`crate::BindlessSampler`] index |
//!
//! Texture table entries are untyped: the shader decides whether index `i` is a
//! `texture2d`, `texture2d_array`, `texturecube` or `texture3d`, and with which access, and
//! must match the texture it was given. Unbound or stale indices are undefined behaviour on
//! the GPU, as on every bindless API.
//!
//! Hand-written MSL starts with [`MSL_PRELUDE`] and declares its entry points with
//! `ED_BINDLESS(ConstantsType)` as the trailing parameters.
//!
//! On Vulkan (SPIR-V) the same four tables are one descriptor set (set 0) shared by every
//! pipeline, plus a 64-byte push-constant range for all stages:
//!
//! | Binding | Content |
//! |---|---|
//! | push constants | 64 bytes of draw or compute constants |
//! | set 0, binding 0 | texture table, sampled-image descriptors |
//! | set 0, binding 1 | texture table, storage-image descriptors (same index; written only for `shader_write` textures, mip 0) |
//! | set 0, binding 2 | buffer table, storage-buffer descriptors (one set per frame slot, so `cpu_write` buffers resolve to this frame's copy) |
//! | set 0, binding 3 | sampler table |
//!
//! Each texture gets one view of its natural type (2D, 2D array, cube, 3D); the shader must
//! read index `i` through the matching alias. A texture with `shader_write` usage is sampled
//! in `GENERAL` layout, a depth texture in `DEPTH_STENCIL_READ_ONLY_OPTIMAL`. Compute shaders
//! may take their workgroup size from specialization constants 0, 1, 2
//! (`layout(local_size_x_id = 0, ...)`, emitted as `LocalSizeId`, which needs the
//! `maintenance4` feature the device enables); the device sets them from
//! [`crate::ComputePipelineDesc::workgroup_size`], and checks a fixed `local_size` against it.
//! The buffer table is read-write in every stage, as on Metal (the device enables
//! `vertexPipelineStoresAndAtomics` and `fragmentStoresAndAtomics`).
//! Hand-written GLSL includes [`GLSL_PRELUDE`] after `#version 460`
//! (`#extension GL_GOOGLE_include_directive : require` + `#include "shader_abi.glsl"` with
//! `glslangValidator -I crates/ed_gpu/src`; see `tests/shaders/build.sh`).

/// Prepend to hand-written MSL. Defines `ED_BINDLESS(T)` and typed accessors:
/// `ed_tex2d(i)`, `ed_tex2d_array(i)`, `ed_texcube(i)`, `ed_tex3d(i)`, `ed_rwtex2d(i)`,
/// `ed_buf(T, i)` (a `device T*`), `ed_smp(i)`, and `ed_c` for the constants.
pub const MSL_PRELUDE: &str = r#"
#include <metal_stdlib>
using namespace metal;

struct EdTex2D { texture2d<float> t; };
struct EdTex2DArray { texture2d_array<float> t; };
struct EdTexCube { texturecube<float> t; };
struct EdTex3D { texture3d<float> t; };
struct EdRwTex2D { texture2d<float, access::read_write> t; };
struct EdBuf { device uchar* p; };
struct EdSmp { sampler s; };

#define ED_BINDLESS(ConstT) \
    constant ConstT& ed_c [[buffer(0)]], \
    constant EdTex2D* ed_textures [[buffer(1)]], \
    constant EdBuf* ed_buffers [[buffer(2)]], \
    constant EdSmp* ed_samplers [[buffer(3)]]

#define ed_tex2d(i) (ed_textures[(i)].t)
#define ed_tex2d_array(i) (reinterpret_cast<constant EdTex2DArray*>(ed_textures)[(i)].t)
#define ed_texcube(i) (reinterpret_cast<constant EdTexCube*>(ed_textures)[(i)].t)
#define ed_tex3d(i) (reinterpret_cast<constant EdTex3D*>(ed_textures)[(i)].t)
#define ed_rwtex2d(i) (reinterpret_cast<constant EdRwTex2D*>(ed_textures)[(i)].t)
#define ed_buf(T, i) (reinterpret_cast<device T*>(ed_buffers[(i)].p))
#define ed_smp(i) (ed_samplers[(i)].s)
"#;

/// GLSL side of the Vulkan binding ABI (`src/shader_abi.glsl`). Include after `#version 460`
/// (glslang `#include` with `-I crates/ed_gpu/src`, or paste). Defines `ED_CONSTANTS`,
/// `ED_BUFFER(T)` and the accessors `ed_tex2d(i)`, `ed_tex2d_array(i)`, `ed_texcube(i)`,
/// `ed_tex3d(i)`, `ed_rwtex2d(i)`, `ed_buf(T, i)`, `ed_smp(i)`.
pub const GLSL_PRELUDE: &str = include_str!("shader_abi.glsl");
