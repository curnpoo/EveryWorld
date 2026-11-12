// Shared declarations for the ed_render shaders (Slang, P1-D1). Included after
// shader_abi.slang by every module. Layouts mirror crates/ed_render/src/gpu.rs (`FrameData`,
// instance, mesh, material and vertex words); change both together. Every struct read
// through a buffer pointer is a multiple of 16 bytes with vec4 members on 16-byte offsets
// (pointer data uses Slang's natural layout, see shader_abi.slang), except the scalar-only
// vertex structs.
#pragma once

// Per-view data, one cpu_write buffer read through the bindless buffer table.
struct HzbLevel { uint offset; uint width; uint height; uint pad; };
struct FrameData {
    float4x4 view_proj;     // camera-relative: rotation and projection only (P1-D7)
    float4x4 inv_view_proj;
    float4x4 view;          // camera-relative world -> view (rotation only), view looks down -Z
    float4 planes[5];       // left, right, bottom, top, near: xyz inward normal, w distance
    float4 sun_dir;         // xyz: unit vector towards the sun
    float4 sun_color;       // rgb: sun radiance scale (linear)
    float4 sky_zenith;      // rgb
    float4 sky_horizon;     // rgb
    float4 ground_color;    // rgb: below-horizon colour and bounce light
    float4 viewport;        // x, y, w, h in output pixels
    float4 params;          // x: exposure, y: LOD threshold (px), z: near (m), w: LOD hysteresis
    float4 proj;            // x: P00, y: P11, z: target width, w: target height
    uint4 cull;             // x: HZB levels (0 = no occlusion test), y: fade frames, z: frame, w: flags
    HzbLevel hzb[16];       // depth pyramid levels inside the HZB buffer (floats)
};

// One instance (128 bytes). Transform rows are a 3x4 affine matrix relative to the node origin.
struct GpuInstance {
    float4 m[3];
    float4 prev[3];         // previous frame's transform (motion vectors; not read yet)
    uint mesh;
    uint material;
    uint object_lo;         // ObjectId low 32 bits (picking)
    uint node_flags;        // node index (bits 0-15), flags (bits 16-31)
    uint pad[4];
};
static const uint INSTANCE_ALIVE = 1u << 16;

// One mesh with its LOD chain (96 bytes).
struct GpuMesh {
    float4 sphere;          // xyz: centre (mesh space), w: radius
    uint lod_count;
    uint bin_base;          // first bin of this mesh (LOD 0) in each kind/phase range
    uint capacity;          // visible-list slots per bin
    uint pad;
    uint first_index[4];
    uint index_count[4];
    int base_vertex[4];
    float error[4];         // geometric error of each level, mesh units (metres)
};

// One material (80 bytes). Both models read the same lighting inputs.
struct GpuMaterial {
    float4 base;            // rgb: base colour (linear), w: model (0 PBR, 1 toon)
    float4 params;          // x: metallic, y: roughness, z: toon bands, w: pattern (1 = checker)
    float4 shade;           // rgb: toon shadow colour, w: toon rim strength
    uint4 tex;              // bindless textures: x base colour, y normal (BC5), z ORM; NO_TEXTURE = none;
                            // w: uv rotation (float bits)
    float4 uv;              // xy: uv offset, zw: uv scale (KHR_texture_transform order)
};
static const uint NO_TEXTURE = 0xFFFFFFFFu;

// Mesh vertex (16 bytes): position, octahedral normal (snorm16 x2).
struct Vtx { float px; float py; float pz; uint n; };

// Skinning source vertex (40 bytes): bind-pose position and normal, 4 joints (u16), 4 weights.
struct SkinVtx { float px; float py; float pz; uint n; uint j01; uint j23; float w[4]; };

// Draw argument entry (DrawIndexedArgs in ed_gpu), 5 words.
static const uint ARGS_WORDS = 5;

// Visible-list entry: instance (bits 0-19), LOD (20-21), dither coverage (22-29), side (30).
static const uint ENTRY_INSTANCE_MASK = 0xFFFFFu;

// Instance state (cull): cur LOD (0-1), prev LOD (2-3), fade frames left (4-9), flags.
static const uint ST_FADE_SHIFT = 4;
static const uint ST_VISIBLE = 1u << 10;
static const uint ST_INIT = 1u << 11;
static const uint ST_CHANGED = 1u << 12;

// snorm16 pack/unpack as MSL's unpack_snorm2x16_to_float / pack_float_to_snorm2x16
// (round half away from zero, which SPIR-V's Round does not promise).
float2 unpack_snorm2x16(uint p) {
    int2 i = int2(int(p << 16) >> 16, int(p) >> 16);
    return clamp(float2(i) / 32767.0, -1.0, 1.0);
}
uint pack_snorm2x16(float2 v) {
    float2 s = clamp(v, -1.0, 1.0) * 32767.0;
    int2 i = int2(sign(s) * floor(abs(s) + 0.5));
    return (uint(i.x) & 0xFFFFu) | (uint(i.y) << 16);
}

// MSL unpack_unorm4x8_to_float: byte 0 is x.
float4 unpack_unorm4x8(uint p) {
    return float4(p & 0xFFu, (p >> 8) & 0xFFu, (p >> 16) & 0xFFu, p >> 24) / 255.0;
}

float3 oct_decode(uint packed) {
    float2 e = unpack_snorm2x16(packed);
    float3 n = float3(e.x, e.y, 1.0 - abs(e.x) - abs(e.y));
    float t = saturate(-n.z);
    n.x += n.x >= 0.0 ? -t : t;
    n.y += n.y >= 0.0 ? -t : t;
    return normalize(n);
}

uint oct_encode(float3 n) {
    n /= max(abs(n.x) + abs(n.y) + abs(n.z), 1e-20);
    float2 e = n.xy;
    if (n.z < 0.0) {
        e = (1.0 - abs(e.yx)) * select(e >= 0.0, float2(1.0), float2(-1.0));
    }
    return pack_snorm2x16(e);
}

float length_squared(float3 v) { return dot(v, v); }

float3 xform_point(GpuInstance i, float3 p) {
    return float3(dot(i.m[0].xyz, p) + i.m[0].w, dot(i.m[1].xyz, p) + i.m[1].w, dot(i.m[2].xyz, p) + i.m[2].w);
}

// Normals transform with the cofactor matrix (inverse transpose times determinant).
float3 xform_normal(GpuInstance i, float3 n) {
    float3 a0 = float3(i.m[0].x, i.m[1].x, i.m[2].x);
    float3 a1 = float3(i.m[0].y, i.m[1].y, i.m[2].y);
    float3 a2 = float3(i.m[0].z, i.m[1].z, i.m[2].z);
    return cross(a1, a2) * n.x + cross(a2, a0) * n.y + cross(a0, a1) * n.z;
}

float max_scale(GpuInstance i) {
    float3 a0 = float3(i.m[0].x, i.m[1].x, i.m[2].x);
    float3 a1 = float3(i.m[0].y, i.m[1].y, i.m[2].y);
    float3 a2 = float3(i.m[0].z, i.m[1].z, i.m[2].z);
    return sqrt(max(max(length_squared(a0), length_squared(a1)), length_squared(a2)));
}

struct FsOut {
    float4 pos: SV_Position;
};

// One triangle covering the viewport.
FsOut fullscreen_vertex(uint vid) {
    float2 p = float2((vid << 1) & 2, vid & 2);
    FsOut o;
    o.pos = float4(p * 2.0 - 1.0, 0.0, 1.0);
    return o;
}

// World-space view direction (camera-relative) of a pixel of a viewport-sized target.
float3 view_dir(FrameData* f, float2 pixel) {
    float2 uv = pixel / f.viewport.zw;
    float2 ndc = float2(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    // Reverse-Z: depth 1 is the near plane, which keeps w finite with an infinite far plane.
    float4 p = mul(f.inv_view_proj, float4(ndc, 1.0, 1.0));
    return normalize(p.xyz / p.w);
}

// Analytic gradient sky (no sun disc). Placeholder until the atmosphere port (Phase 2).
float3 sky_radiance(FrameData* f, float3 d) {
    float t = pow(saturate(d.y), 0.45);
    float3 sky = lerp(f.sky_horizon.rgb, f.sky_zenith.rgb, t);
    sky = lerp(sky, f.ground_color.rgb, saturate(-d.y * 8.0));
    float mu = saturate(dot(d, f.sun_dir.xyz));
    sky += f.sun_color.rgb * 0.06 * pow(mu, 8.0);
    return sky;
}

// ACES fit (Narkowicz 2015), shared by tonemap and the pop detector. PLACEHOLDER (P1-D18).
float3 aces_fit(float3 x) {
    return saturate((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14));
}
