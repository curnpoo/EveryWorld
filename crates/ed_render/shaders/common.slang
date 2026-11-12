// Shared declarations for the ed_render shaders (Slang, P1-D1). Included after
// shader_abi.slang by every module. Layouts mirror crates/ed_render/src/gpu.rs (`FrameData`,
// instance, mesh, material and vertex words); change both together. Every struct read
// through a buffer pointer is a multiple of 16 bytes with vec4 members on 16-byte offsets
// (pointer data uses Slang's natural layout, see shader_abi.slang), except the scalar-only
// vertex structs.
#pragma once

// Lighting (M4d), the tail of FrameData (src/gpu.rs `FD_LIGHT`, src/light.rs `LightData`).
struct LightData {
    float4x4 shadow[4];     // camera-relative world -> cascade clip: x, y in -1..1, z 0..1 (1 = towards the sun)
    float4 cascade[4];      // x: texel size (m), y: depth range (m), z, w: tile origin in the atlas (px)
    float4 split;           // view distance where each cascade ends (m)
    float4 cfg;             // x: cascade count (0 = shadows off), y: tile size (px), z: blend band (fraction), w: edge margin (texels)
    float4 filter;          // x: fade start (m), y: shadow distance (m), z: normal bias (texels), w: depth bias (texels)
    uint4 flags;            // x: shadows, y: ao, z: sky ambient (SH), w: debug view (1 shadow, 2 ao)
    float4 ao;              // x: intensity (exponent), y: half-res width, z: half-res height, w: unused
    float4 sky;             // x: sun disc scale, yzw: unused
    float4 sh[9];           // sky irradiance / pi as SH9 (rgb), basis constants folded in (src/light.rs)
};

// Per-view data, one cpu_write buffer read through the bindless buffer table.
struct HzbLevel { uint offset; uint width; uint height; uint pad; };
struct FrameData {
    float4x4 view_proj;     // camera-relative: rotation and projection only (P1-D7)
    float4x4 inv_view_proj;
    float4x4 view;          // camera-relative world -> view (rotation only), view looks down -Z
    float4 planes[5];       // left, right, bottom, top, near: xyz inward normal, w distance
    float4 sun_dir;         // xyz: unit vector towards the sun, w: daylight factor (0.03..1)
    float4 sun_color;       // rgb: sun irradiance (linear)
    float4 sky_zenith;      // rgb: sky model zenith colour
    float4 sky_horizon;     // rgb: sky model horizon colour
    float4 ground_color;    // rgb: below-horizon colour (sky model)
    float4 viewport;        // x, y, w, h in output pixels
    float4 params;          // x: exposure, y: LOD threshold (px), z: near (m), w: LOD hysteresis
    float4 proj;            // x: P00, y: P11, z: target width, w: target height
    uint4 cull;             // x: HZB levels (0 = no occlusion test), y: fade frames, z: frame, w: flags
    HzbLevel hzb[16];       // depth pyramid levels inside the HZB buffer (floats)
    LightData light;
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

// Sky radiance (the alpha's analytic sky, reference/alpha_metal world.metal `sky_rad`, ported
// with its constants in FrameData): zenith/horizon gradient, warm forward scatter at low sun,
// a sun glow, the ground below the horizon and the sun disc (`disc_k` 0 leaves it out: the
// direct sun term carries that light). Mirrored on the CPU by src/light.rs `sky_rad` (SH bake).
float3 sky_rad(FrameData* f, float3 d, float disc_k) {
    float3 sun = f.sun_dir.xyz;
    float day = f.sun_dir.w;
    float h = d.y;
    float t = pow(saturate(1.0 - max(h, 0.0)), 6.5);
    float3 c = lerp(f.sky_zenith.rgb, f.sky_horizon.rgb, t);
    float mu = dot(d, sun);
    float warm = saturate(1.0 - sun.y * 2.5);
    c += float3(1.0, 0.42, 0.14) * pow(saturate(mu * 0.5 + 0.5), 5.0) * warm * 0.85 * (0.4 + t);
    c = lerp(c, c * float3(1.0, 0.7, 0.55), warm * 0.4);
    c += float3(1.0, 0.9, 0.7) * pow(saturate(mu), 28.0) * 0.35;
    c = lerp(c, f.ground_color.rgb, saturate(-h * 6.0));
    float disk = smoothstep(0.99965, 0.99985, mu) * step(0.0, h);
    return c * day + float3(1.0, 0.95, 0.85) * disk * 60.0 * day * disc_k;
}

// The alpha's tone mapper (world.metal `resolve_fs`), exact maths (P1-D18). The film curve is
// a 64^3 RGBA16F LUT (Blender Filmic "-11..12" log encoding, assets/filmic_lut_rgba16f.bin)
// stored in a buffer, two words per texel (x fastest), filtered trilinearly here so CPU and
// GPU agree (src/tonemap.rs mirrors every line).
static const uint FILMIC_LUT_N = 64;

float3 lut_texel(uint* lut, uint3 t) {
    uint i = ((t.z * FILMIC_LUT_N + t.y) * FILMIC_LUT_N + t.x) * 2u;
    uint a = lut[i];
    uint b = lut[i + 1u];
    return float3(f16tof32(a & 0xFFFFu), f16tof32(a >> 16), f16tof32(b & 0xFFFFu));
}

float3 filmic(uint lut_buf, float3 x) {
    uint* lut = ed_buf<uint>(lut_buf);
    x = max(x, float3(0.0));
    x = select(x < float3(0.00003051757), x + float3(0.00001525878), x);
    float3 n = saturate((log2(x / 0.18) + 11.0) / 23.0);
    float3 t = n * float(FILMIC_LUT_N - 1u);
    uint3 i = min(uint3(t), uint3(FILMIC_LUT_N - 2u));
    float3 g = t - float3(i);
    float3 c00 = lerp(lut_texel(lut, i), lut_texel(lut, i + uint3(1, 0, 0)), g.x);
    float3 c10 = lerp(lut_texel(lut, i + uint3(0, 1, 0)), lut_texel(lut, i + uint3(1, 1, 0)), g.x);
    float3 c01 = lerp(lut_texel(lut, i + uint3(0, 0, 1)), lut_texel(lut, i + uint3(1, 0, 1)), g.x);
    float3 c11 = lerp(lut_texel(lut, i + uint3(0, 1, 1)), lut_texel(lut, i + uint3(1, 1, 1)), g.x);
    return lerp(lerp(c00, c10, g.y), lerp(c01, c11, g.y), g.z);
}

// HDR scene radiance -> display value (gamma 2.2 encoded, 0..1), as the alpha wrote it.
float3 alpha_tonemap(uint lut_buf, float3 hdr, float exposure) {
    float3 c = hdr * exposure;
    c *= float3(1.10, 1.0, 0.86);
    c = filmic(lut_buf, c);
    float l = dot(c, float3(0.2126, 0.7152, 0.0722));
    c *= lerp(float3(1.0), float3(1.03, 1.0, 0.95), smoothstep(0.35, 0.9, l));
    return pow(saturate(c), float3(1.0 / 2.2));
}
