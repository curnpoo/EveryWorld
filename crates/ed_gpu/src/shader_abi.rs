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
