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
    uint4 flags;            // x: shadows, y: ao, z: sky ambient (SH), w: debug view (1 shadow, 2 ao, 3 depth, 4 sky visibility)
    float4 ao;              // x: intensity (exponent), y: half-res width, z: half-res height, w: radius (m)
    float4 sky;             // x: sun disc scale, y: AO multi-bounce (0..1), z: shadow filter radius (m), w: max filter radius (texels)
    float4 sh[9];           // ambient (skylight + ground bounce) irradiance / pi as SH9 (rgb), basis constants folded in (src/light.rs)
    float4 skyvis;          // x: sky visibility on (1) / off (0), y: bounce fill of the occluded sky, z: search radius (m), w: unused
};

// Per-view data, one cpu_write buffer read through the bindless buffer table.
// Physically based atmosphere (atmos.slang; src/atmosphere.rs `atmos_words`). Lengths in km,
// coefficients in 1/km.
struct AtmosData {
    float4 radii;           // x: body radius, y: top of the atmosphere, z: camera altitude, w: surface altitude (ambient)
    float4 rayleigh;        // rgb: Rayleigh scattering, w: scale height
    float4 mie_s;           // rgb: Mie scattering, w: scale height
    float4 mie_a;           // rgb: Mie absorption, w: phase asymmetry g
    float4 ozone;           // rgb: ozone absorption at the peak, w: peak altitude
    float4 ground;          // rgb: ground albedo, w: ozone half width
    float4 sun;             // rgb: sun irradiance at the top of the atmosphere, w: cos of the disc's angular radius
    float4 up;              // xyz: local up at the camera, w: cos of the sun's zenith angle there
    float4 ap;              // x, y: froxel depth mapping n, D (km), z: sunlit share of the ground, w: unused
    uint4 bufs;             // bindless: transmittance, multiple scattering, sky view (camera), sky view (surface)
    uint4 bufs2;            // bindless: froxel in-scatter, froxel transmittance, ambient SH, unused
    uint4 cfg;              // x: on, y: debug view (1 transmittance, 2 in-scatter), z, w: unused
};

// Wind (trees G4b; ed_world::wind::WindField::gpu_words, src/lib.rs `set_wind`).
struct WindData {
    float4 wind;            // xyz: mean wind (m/s, render frame), w: time (s, mod 1 h)
    float4 gust;            // x: gust band phase at the camera (cycles), y: band spacing (m), z: gust strength, w: turbulence
    float4 cfg;             // x: wind on, y: rest distance (m; trees beyond hold still), z: near trees on (far rung yields), w: unused
};

// Tree impostors (trees G5, src/impostor.rs `imp_words`; one Rust table sets every band).
struct ImpData {
    uint4 bufs;             // bindless: x tree buffer, y kind table; z tree slots, w flags (IMP_ON) in
                            // bits 0..8, ground pull (1/16 px) above
    float4 band;            // x, y: T1 -> T2 (impostor full under x px of the tree's height, none over y);
                            // z, w: T2 -> T3 (each tree leaves at a hashed reference-tree size in z..w px)
    float4 far;             // x, y: far rung lift none / full (reference px); z, w: canopy paint none / full
    float4 cfg;             // x: reference tree height (m), y: frame pad, z: value range, w: first cascade
                            // where impostors cast every tree's shadow (meshes cast only nearer)
};
static const uint IMP_ON = 1u;
static const uint IMP_VIS_BIT = 0x40000000u;   // vis.x: impostor tree index (TERRAIN_VIS_BIT clear)

// Seasons (trees G5; ed_world::season, src/lib.rs `set_season`). cfg: x phase day (year day after
// the rate scalar; 0 = seasons off), y autumn gain, z, w the far rung's broadleaf and conifer rows.
// Two float4 per species row: (leaf-out, colour, drop days at the reference; evergreen flag),
// (autumn or winter albedo rgb, linear; winter share).
static const uint SEASON_ROWS = 32;
struct SeasonData {
    float4 cfg;
    float4 rows[64];
};

// Grass style A (src/grass.rs `GrassStore::words`, ed_world::grass).
struct GrassData {
    uint4 bufs;             // bindless: x clump buffer, y type table; z clump slots, w flags (1 on, 2 shadows, 4 wind)
    float4 lod;             // x: full-density radius r0 (m), y: range (m), z: fewest blades, w: most widening
    float4 look;            // x: shading normal blend to the ground normal (near), yzw: unused
};

// Grass shells and the terrain's grass tint (styles C and D; src/terrain.rs `ShellGrass`,
// ed_world::grass map). Maps and type rows live in the terrain draw table (`ResolveC.t_draws`).
struct ShellData {
    uint4 cfg;              // x: flags (1 shells, 2 tint, 4 map), y: type rows (GrassType index), z: fine map word (coarse at + n*n), w: shells
    int4 off;               // xy: fine map slot offset, zw: coarse
    float4 c;               // xy: map centre face metres (mod 4096, a multiple of 32), z: shell range (m), w: tint strength
    float4 fu;              // xyz: the map face's u axis (render frame), w: top shell height (m)
    float4 cells;           // x: fine cell (m), y: coarse cell (m), z: cells per side, w: unused
};

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
    AtmosData atmos;
    WindData wind;
    ImpData imp;
    SeasonData season;
    GrassData grass;
    ShellData shell;
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
    uint flags;             // MESH_FOLIAGE
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

// GpuMesh.flags: a tree bank mesh (ed_world::trees::bank). Its `uvs` words are
// (atlas uv unorm16x2, aux bits) instead of float2, it draws with the foliage pipelines
// (alpha-tested cards, wind) and resolves with the foliage shading.
static const uint MESH_FOLIAGE = 1u;
// Tree vertex aux word (bank.rs `pack_aux`).
static const uint AUX_CARD = 1u << 27;
static const uint AUX_BARK = 1u << 28;

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

// IEEE half -> float by integer math: exact for every half, and needs no Float16/Int16 shader
// capability (Slang's f16tof32 declares both on SPIR-V, which Vulkan devices may lack).
float half_bits_to_float(uint h) {
    uint e = (h >> 10) & 0x1Fu;
    uint m = h & 0x3FFu;
    float s = (h & 0x8000u) != 0u ? -1.0 : 1.0;
    if (e == 0u) {
        return s * float(m) * 5.9604644775390625e-8;  // 2^-24
    }
    if (e == 31u) {
        return asfloat(((h & 0x8000u) << 16) | 0x7F800000u | (m << 13));
    }
    return asfloat(((h & 0x8000u) << 16) | ((e + 112u) << 23) | (m << 13));
}

float3 lut_texel(uint* lut, uint3 t) {
    uint i = ((t.z * FILMIC_LUT_N + t.y) * FILMIC_LUT_N + t.x) * 2u;
    uint a = lut[i];
    uint b = lut[i + 1u];
    return float3(half_bits_to_float(a & 0xFFFFu), half_bits_to_float(a >> 16), half_bits_to_float(b & 0xFFFFu));
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

// HDR scene radiance -> display value (gamma 2.2 encoded, 0..1): the alpha's tone mapper
// (exposure, Filmic LUT, gamma 2.2) with its warm grade neutralised (M4f). The alpha multiplied
// by a white balance of (1.10, 1, 0.86) and tinted highlights by (1.03, 1, 0.95), which
// compensated for its blue ambient; with the calibrated light of M4e (warm sun, skylight
// R : B 0.5) it turned sunlit whites yellow. A camera on daylight white balance renders
// sunlit white neutral, so both are 1 now (tonemap.rs `WHITE_BALANCE`, `HIGHLIGHT_TINT`).
static const float3 TM_WHITE_BALANCE = float3(1.0, 1.0, 1.0);
static const float3 TM_HIGHLIGHT_TINT = float3(1.0, 1.0, 1.0);
float3 alpha_tonemap(uint lut_buf, float3 hdr, float exposure) {
    float3 c = hdr * exposure;
    c *= TM_WHITE_BALANCE;
    c = filmic(lut_buf, c);
    float l = dot(c, float3(0.2126, 0.7152, 0.0722));
    c *= lerp(float3(1.0), TM_HIGHLIGHT_TINT, smoothstep(0.35, 0.9, l));
    return pow(saturate(c), float3(1.0 / 2.2));
}

// ---- Trees (G4, G4b) ----------------------------------------------------------------------

float2 tree_uv(uint w) {
    return float2(float(w & 0xFFFFu), float(w >> 16)) * (1.0 / 65535.0);
}

// ---- Ground cover (trees G7, ed_world::ground_cover; keep/grow mirror the Rust) -------------
// Instance flag (scene.rs `GROUND_COVER_FLAG`, flags bit 0 -> node_flags bit 17); word pad[2]
// (`ground_word`): rank bits 0-11 (/4095), reach m bits 12-19, solid bit 20, no shadow bit 21,
// first shadow cascade only bit 22 (cull.slang `shadow_cull`).
static const uint INSTANCE_GROUND = 1u << 17;
static const float GC_NEAR_FRAC = 0.195;
static const float GC_FADE_FRAC = 0.7;
static const float GC_GROW_MAX = 2.0;
static const float GC_GROW_Y_MAX = 1.25;

float gc_keep(float d, float reach) {
    float r0 = GC_NEAR_FRAC * reach;
    float k = r0 / max(d, r0);
    return k * k * (1.0 - smoothstep(GC_FADE_FRAC * reach, reach, d));
}

// Model-space scale of a ground-cover instance at camera distance `d`: 0 when thinned out;
// survivors shrink smoothly as the threshold passes their rank and grow as density thins.
float3 gc_scale(GpuInstance inst, float d) {
    uint g = inst.pad[2];
    if (((g >> 20) & 1u) != 0u) {
        return float3(1.0);
    }
    float rank = float(g & 4095u) * (1.0 / 4095.0);
    float k = gc_keep(d, float((g >> 12) & 255u));
    float s = saturate((k - rank) / (0.3 * k + 1e-3));
    if (s <= 0.0) {
        return float3(0.0);
    }
    float gx = clamp(sqrt(1.0 / max(k, 1e-4)), 1.0, GC_GROW_MAX);
    float gy = min(gx, GC_GROW_Y_MAX);
    return float3(gx, gy, gx) * s;
}

// Tree vertex, node-local (instance transform applied): wind sway along the wind, gust band,
// leaf flutter along the normal, dropped branch groups collapsed (bank.rs `instance_params`
// user word: drop mask bits 16-23, phase bits 24-31). The same function moves the view, the
// shadow casters and the resolve's triangle, so they agree to the bit.
float3 tree_vertex(FrameData* f, GpuInstance inst, float3 node, float3 pm, uint n, uint aux) {
    uint user = inst.pad[0];
    float3 origin = float3(inst.m[0].w, inst.m[1].w, inst.m[2].w);
    if ((aux & AUX_CARD) != 0u && ((user >> 16) & (1u << (aux >> 29))) != 0u) {
        return origin;
    }
    if ((inst.node_flags & INSTANCE_GROUND) != 0u) {
        float3 g = gc_scale(inst, length(node + origin));
        if (g.x <= 0.0) {
            return origin;
        }
        pm *= g;
    }
    float3 l = xform_point(inst, pm);
    WindData w = f.wind;
    if (w.cfg.x < 0.5) {
        return l;
    }
    float dist = length(node + origin);
    float fade = saturate((w.cfg.y - dist) / max(0.25 * w.cfg.y, 1.0));
    if (fade <= 0.0) {
        return l;
    }
    float q = float((aux >> 11) & 127u) / 127.0;
    float sway = q * q * 4.0;
    float flutter = float((aux >> 18) & 7u) / 7.0;
    float phase = float((aux >> 21) & 63u) / 64.0 + float(user >> 24) / 255.0;
    float speed = length(w.wind.xz);
    float2 wd = speed > 1e-3 ? w.wind.xz / speed : float2(0.0, 0.0);
    float band = w.gust.x + dot((node + origin).xz, wd) / max(w.gust.y, 1.0);
    float s = 0.5 + 0.5 * sin(band * 6.2831853);
    float gust = s * s * s * w.gust.z;
    float t = w.wind.w;
    float k = speed * 0.1 * fade * max_scale(inst);
    // Steady lean into the wind, the gust on top, and a slow per-tree rock.
    float rock = 0.55 + gust + 0.3 * sin(t * 1.7 + phase * 6.2831853) + 0.15 * sin(t * 0.63 + phase * 19.0);
    float3 d = float3(wd.x, 0.0, wd.y) * (sway * k * rock);
    d.y -= 0.25 * sway * k * rock * rock * 0.3;
    // Leaf flutter: fast, small, along the normal.
    float3 nw = normalize(xform_normal(inst, oct_decode(n)));
    float fl = flutter * w.gust.w * (0.6 + gust) * 0.05 * fade * sin(t * 11.0 + phase * 40.0 + dot(l, float3(3.1, 1.7, 2.3)));
    return l + d + nw * fl * max_scale(inst);
}

// ---- Tree seasons (trees G5, ed_world::season; ramps mirror season.rs) -------------------------
struct LeafSeason {
    float leaves;   // share of the summer leaf mass on the tree: green * (1 - drop); evergreens 1
    float green;    // leaf-out 0..1
    float colour;   // how far toward `target` (autumn colour; winter colour for evergreens)
    float evergreen;
    float3 target;  // linear albedo
};

float season_ramp(float lo, float hi, float x) {
    return smoothstep(lo, hi, x);
}

LeafSeason season_off() {
    LeafSeason s;
    s.leaves = 1.0;
    s.green = 1.0;
    s.colour = 0.0;
    s.evergreen = 0.0;
    s.target = float3(0.0, 0.0, 0.0);
    return s;
}

// One species row at a place: `shift` (spring shift, days) and hemisphere.
LeafSeason season_state(FrameData* f, uint row, float shift, bool south) {
    if (f.season.cfg.x <= 0.0) {
        return season_off();
    }
    float d = f.season.cfg.x;
    if (south) {
        d = fmod(d + 182.625, 365.25);
    }
    float ds = d - shift;
    float da = d + f.season.cfg.y * shift;
    float4 r0 = f.season.rows[(row & 31u) * 2u];
    float4 r1 = f.season.rows[(row & 31u) * 2u + 1u];
    LeafSeason s;
    s.target = r1.rgb;
    s.evergreen = r0.w;
    if (r0.w > 0.5) {
        float a = season_ramp(r0.y - 10.0, r0.y + 30.0, da);
        float rel = 1.0 - season_ramp(r0.x - 10.0, r0.x + 20.0, ds);
        s.green = 1.0;
        s.leaves = 1.0;
        s.colour = r1.w * max(a, rel);
    } else {
        s.green = season_ramp(r0.x - 6.0, r0.x + 8.0, ds);
        s.colour = season_ramp(r0.y - 8.0, r0.y + 14.0, da);
        s.leaves = s.green * (1.0 - season_ramp(r0.z - 10.0, r0.z + 10.0, da));
    }
    return s;
}

// The per-tree word (season.rs `season_word`).
LeafSeason season_of_word(FrameData* f, uint word) {
    return season_state(f, word & 255u, float(int(word) >> 16) * (1.0 / 16.0), ((word >> 8) & 1u) != 0u);
}

// The far rung's trees: no per-tree place, so the shift comes from the ground height alone.
LeafSeason season_far(FrameData* f, bool conifer, float alt_m) {
    uint row = uint(conifer ? f.season.cfg.w : f.season.cfg.z);
    return season_state(f, row, clamp(4.0 * (alt_m - 500.0) / 122.0, -60.0, 75.0), false);
}

// Leaf albedo through the season: young leaves yellower, then toward the target colour keeping
// the texture's own light and dark.
float3 season_tint(LeafSeason s, float3 alb) {
    if (s.leaves >= 1.0 && s.colour <= 0.0 && s.green >= 1.0) {
        return alb;
    }
    float lum = dot(alb, float3(0.2126, 0.7152, 0.0722));
    float3 a = alb;
    if (s.evergreen < 0.5) {
        a *= lerp(float3(1.45, 1.3, 0.65), float3(1.0, 1.0, 1.0), s.green);
    }
    return lerp(a, s.target * clamp(lum * (1.0 / 0.2), 0.45, 1.5), s.colour);
}

// ---- Tree impostors (trees G5, src/impostor.rs) ---------------------------------------------

// One impostor tree (32 bytes): base position relative to its node, node (12 bits) | kind
// (12 bits) | hash (8 bits), scale (0: dead slot), up (oct_encode), yaw unorm16 | widen
// (unorm16, 0.5 + x), user word (value, hue, drop, phase; as the mesh instance's).
struct GpuTree { float px; float py; float pz; uint node_kind; float scale; uint up; uint yaw_widen; uint user; uint season; };
// One impostor kind (64 bytes): BC5 textures (A coverage/depth, B normal, C value/bark) and
// frames per side; centre height, radius, tree height, frame px; leaf albedo + mips; bark albedo + crown word (10:11:11 bits: base / height, radius and widest leaf / 1.5 height).
struct GpuImpKind { uint4 tex; float4 geo; float4 leaf; float4 bark; };

// Rotates `v` by the shortest arc from +Y to `up` (glam `Quat::from_rotation_arc(Y, up)`).
float3 arc_from_y(float3 up, float3 v) {
    float3 a = float3(up.z, 0.0, -up.x);    // cross(Y, up)
    float w = 1.0 + up.y;
    float l = sqrt(dot(a, a) + w * w);
    if (l < 1e-6) {
        return float3(v.x, -v.y, -v.z);     // up = -Y: half turn about X
    }
    a /= l;
    w /= l;
    float3 t = 2.0 * cross(a, v);
    return v + w * t + cross(a, t);
}

// Hemi-octahedral frame grid (ed_world trees/impostor.rs `oct_encode`, `oct_decode`, `view_basis`).
float2 imp_oct_encode(float3 d) {
    d.y = max(d.y, 0.0);
    d /= max(abs(d.x) + abs(d.y) + abs(d.z), 1e-9);
    return float2(d.x + d.z, d.x - d.z) * 0.5 + 0.5;
}
float3 imp_oct_decode(float2 uv) {
    float2 g = uv * 2.0 - 1.0;
    float x = (g.x + g.y) * 0.5;
    float z = (g.x - g.y) * 0.5;
    return normalize(float3(x, 1.0 - abs(x) - abs(z), z));
}
void imp_view_basis(float3 v, out float3 right, out float3 up) {
    float3 u0 = float3(0.0, 1.0, 0.0) - v * v.y;
    up = dot(u0, u0) > 1e-6 ? normalize(u0) : float3(0.0, 0.0, 1.0);
    right = normalize(cross(up, v));
}
// Full-sphere octahedral decode of a baked normal (y pole; bank_impostor.rs `oct_sphere`).
float3 imp_oct_sphere(float2 e) {
    float2 f = e * 2.0 - 1.0;
    float3 n = float3(f.x, 1.0 - abs(f.x) - abs(f.y), f.y);
    float t = saturate(-n.y);
    n.x += n.x >= 0.0 ? -t : t;
    n.z += n.z >= 0.0 ? -t : t;
    return normalize(n);
}

// A tree's frame of reference and its three blended frames for the view from the camera.
struct ImpTree {
    float3 c;       // camera-relative sphere centre
    float3 ex;      // tree axes (world, unit)
    float3 up;
    float3 ez;
    float s;        // scale
    float wd;       // crown widen (x, z)
    float r;        // sphere radius (tree space)
    float3 cam;     // camera in tree space (unscaled, relative to the centre)
    uint3 fi;       // frame index (j * N + i) of the three frames
    float3 w;       // their weights
    float3 d0; float3 d1; float3 d2;   // their view directions (tree space)
};

float3 imp_to_tree(ImpTree t, float3 v) {
    return float3(dot(v, t.ex) / (t.s * t.wd), dot(v, t.up) / t.s, dot(v, t.ez) / (t.s * t.wd));
}

// `sun`: seen from the sun along `sun_dir` (shadow casters), else from the camera.
ImpTree imp_tree(GpuTree g, GpuImpKind k, float3 node, float3 sun_dir, bool sun) {
    ImpTree t;
    t.up = oct_decode(g.up);
    float yaw = float(g.yaw_widen & 0xFFFFu) / 65535.0 * 6.2831853;
    t.wd = 0.5 + float(g.yaw_widen >> 16) / 65535.0;
    t.s = g.scale;
    t.ex = arc_from_y(t.up, float3(cos(yaw), 0.0, -sin(yaw)));
    t.ez = arc_from_y(t.up, float3(sin(yaw), 0.0, cos(yaw)));
    t.r = k.geo.y;
    t.c = node + float3(g.px, g.py, g.pz) + t.up * (k.geo.x * t.s);
    t.cam = imp_to_tree(t, sun ? sun_dir * 1.0e5 : -t.c);
    float n = float(k.tex.w);
    float3 v = normalize(t.cam);
    float2 gpos = clamp(imp_oct_encode(v) * n - 0.5, float2(0.0, 0.0), float2(n - 1.0, n - 1.0));
    float2 b = min(floor(gpos), float2(n - 2.0, n - 2.0));
    float2 fr = gpos - b;
    uint2 b0 = uint2(b);
    uint N = k.tex.w;
    uint2 c0 = b0;
    uint2 c1 = b0 + uint2(1u, 0u);
    uint2 c2 = b0 + uint2(0u, 1u);
    if (fr.x + fr.y < 1.0) {
        t.w = float3(1.0 - fr.x - fr.y, fr.x, fr.y);
    } else {
        c0 = b0 + uint2(1u, 1u);
        t.w = float3(fr.x + fr.y - 1.0, 1.0 - fr.y, 1.0 - fr.x);
    }
    t.fi = uint3(c0.y * N + c0.x, c1.y * N + c1.x, c2.y * N + c2.x);
    t.d0 = imp_oct_decode((float2(c0) + 0.5) / n);
    t.d1 = imp_oct_decode((float2(c1) + 0.5) / n);
    t.d2 = imp_oct_decode((float2(c2) + 0.5) / n);
    return t;
}

// Frame-local uv (0..1 inside the frame) where the ray from the camera through tree-space point
// `q` (relative to the centre) meets frame `d`'s plane.
float2 imp_frame_uv(ImpTree t, float3 d, float3 q, float half_size) {
    float3 dir = q - t.cam;
    float den = dot(dir, d);
    float k = abs(den) > 1e-6 ? -dot(t.cam, d) / den : 1.0;
    float3 h = t.cam + dir * k;
    float3 r;
    float3 u;
    imp_view_basis(d, r, u);
    return float2(0.5 + dot(h, r) / (2.0 * half_size), 0.5 - dot(h, u) / (2.0 * half_size));
}

// Atlas uv of frame `fi` at frame-local `uv` (clamped half a texel inside the frame).
float2 imp_atlas_uv(uint fi, uint n, float2 uv, float frame_px) {
    float m = 0.5 / frame_px;
    float2 l = clamp(uv, float2(m, m), float2(1.0 - m, 1.0 - m));
    return (float2(float(fi % n), float(fi / n)) + l) / float(n);
}
bool imp_inside(float2 uv) {
    return all(uv >= 0.0) && all(uv <= 1.0);
}

// ---- Grass style A (src/grass.rs, ed_world::grass) ----------------------------------------
// A clump record (32 bytes): centre relative to its node, node (12 bits) | type (8) | rank (8),
// ground normal and facing (oct_encode), seed, height scale (unorm16 x 4; 0 = dead slot) | dry
// (unorm8). Blades are built from the seed and the type row, the same in the vertex shader,
// the shadow pass and the resolve.
static const uint GRASS_VIS_BIT = 0x20000000u;   // vis.x: clump index (TERRAIN and IMP bits clear)
static const uint GRASS_INDEX_MASK = 0x1FFFFFFFu;
// Buckets: blade slots, segments and first index (src/grass.rs `BUCKETS`, `bucket_indices`).
static const uint GRASS_BUCKETS = 5u;
static const uint GRASS_SLOTS[5] = { 64u, 32u, 16u, 8u, 4u };
static const uint GRASS_SEGS[5] = { 6u, 5u, 4u, 3u, 2u };
static const uint GRASS_FIRST[5] = { 0u, 2304u, 3264u, 3648u, 3792u };

struct GrassClump { float px; float py; float pz; uint node_kind; uint up; uint facing; uint seed; uint hw; };
// Type row (ed_world::grass::gpu_row): a (h min, h max, width, blades), b (clump r, lean,
// stiffness, curve), root (rgb, aniso), mid (rgb, head share), tip (rgb, head width), dry (rgb, density),
// shell (shell height, strand cell, shell cover, sheen), carpet (rgb, shells only).
struct GrassType { float4 a; float4 b; float4 root; float4 mid; float4 tip; float4 dry; float4 shell; float4 carpet; };

uint grass_hash(uint x) {
    uint s = x * 747796405u + 2891336453u;
    uint w = ((s >> ((s >> 28u) + 4u)) ^ s) * 277803737u;
    return (w >> 22u) ^ w;
}
float grass_rand(uint h) {
    return float(h >> 8) * (1.0 / 16777216.0);
}

// Continuous blades per clump and the clump keep at distance `d` (ed_world::grass::lod).
float2 grass_lod(FrameData* f, float d, float blades) {
    float q = f.grass.lod.x / max(d, 1e-3);
    float raw = blades * min(q * q, 1.0);
    float bm = min(f.grass.lod.z, blades);
    return float2(max(raw, bm), min(raw / max(bm, 1e-3), 1.0));
}

// The smallest bucket holding `cnt` blades.
uint grass_bucket(float cnt) {
    uint need = uint(ceil(cnt));
    for (int b = 4; b > 0; b--) {
        if (GRASS_SLOTS[b] >= need) {
            return uint(b);
        }
    }
    return 0u;
}

struct GrassBlade {
    float3 p0;      // camera-relative quadratic Bezier: root, control, tip
    float3 p1;
    float3 p2;
    float3 side;    // width axis (unit)
    float3 up;      // ground normal
    float w;        // root width (m)
    float h;        // height (m); 0: not drawn
    float head;     // seed head share and width (over the root width)
    float head_w;
    float shade;    // per-blade value
    float t_ao;     // root darkness 0..1 (taller grass is darker inside)
};

float3 grass_point(GrassBlade b, float t) {
    float s = 1.0 - t;
    return b.p0 * (s * s) + b.p1 * (2.0 * s * t) + b.p2 * (t * t);
}
float3 grass_tangent(GrassBlade b, float t) {
    return (b.p1 - b.p0) * (2.0 * (1.0 - t)) + (b.p2 - b.p1) * (2.0 * t);
}
float grass_width(GrassBlade b, float t) {
    float w = b.w * (1.0 - t * t * t) * (1.0 - 0.2 * t);
    if (b.head > 0.0) {
        float hs = smoothstep(1.0 - b.head - 0.04, 1.0 - b.head, t) * (1.0 - smoothstep(0.93, 1.0, t));
        w = lerp(w, b.w * b.head_w * (1.0 - 0.25 * t), hs);
    }
    return w;
}

// Blade `k` of clump `c` (node origin `node`, camera-relative). `lod_mul` scales the blade count
// (1 view and resolve, 0.5 shadow casters: fewer, wider blades).
GrassBlade grass_blade(FrameData* f, GrassClump c, GrassType ty, float3 node, uint k, float lod_mul) {
    GrassBlade o;
    float3 up = oct_decode(c.up);
    float3 fa = oct_decode(c.facing);
    float3 rt = normalize(cross(up, fa));
    fa = cross(rt, up);
    float3 base = node + float3(c.px, c.py, c.pz);
    float d = length(base);
    float blades = ty.a.w;
    float2 lod = grass_lod(f, d, blades);
    float bm = min(f.grass.lod.z, blades);
    float cnt = max(lod.x * lod_mul, bm);
    // Widen as blades thin (coverage holds), and survivors of clump thinning a little more.
    float wscale = min(pow(blades / cnt, 0.8) * rsqrt(max(lod.y, 0.2)), f.grass.lod.w);
    float rank = float(c.node_kind >> 24) / 255.0;
    float keep = saturate((lod.y - rank) / (0.3 * lod.y + 1e-3));
    float range = f.grass.lod.y;
    float fade = 1.0 - smoothstep(0.6 * range, range, d);
    uint cs = c.seed;
    uint s0 = grass_hash(cs ^ (k * 0x9E3779B9u + 0x7F4A7C15u));
    uint s1 = grass_hash(s0);
    uint s2 = grass_hash(s1);
    uint s3 = grass_hash(s2);
    uint s4 = grass_hash(s3);
    uint s5 = grass_hash(s4);
    // Root: uniform in the clump's (anisotropic) disc, a little under the ground.
    float ang = grass_rand(s0) * 6.2831853;
    float rad = sqrt(grass_rand(s1));
    float cr = ty.b.x;
    float3 off = (fa * cos(ang) + rt * (sin(ang) * ty.root.w)) * (rad * cr);
    o.up = up;
    // Height: the clump's, lower towards its edge, jittered; blades past the count shrink away.
    float hs = float(c.hw & 0xFFFFu) / 65535.0 * 4.0;
    float hc = lerp(ty.a.x, ty.a.y, grass_rand(grass_hash(cs + 17u))) * hs;
    // Roots a little under the ground (short grass less, or it never shows).
    o.p0 = base + off - up * min(0.03, 0.1 * hc);
    float h = hc * (1.0 - 0.35 * rad) * lerp(0.72, 1.12, grass_rand(s2));
    float grow = saturate(cnt - float(k)) * keep * fade;
    o.h = (k < uint(blades) && (c.hw & 0xFFFFu) != 0u) ? h * grow : 0.0;
    // Lean: the clump's shared direction, outwards from its centre, and a little of its own.
    float ca = grass_rand(grass_hash(cs + 29u)) * 6.2831853;
    float3 cdir = fa * cos(ca) + rt * sin(ca);
    float3 radial = rad > 1e-3 ? normalize(off) : fa;
    float ba = grass_rand(s3) * 6.2831853;
    float3 ld = cdir * 0.9 + radial * 0.8 + (fa * cos(ba) + rt * sin(ba)) * 0.6;
    ld = dot(ld, ld) > 1e-6 ? normalize(ld) : fa;
    float lean = ty.b.y * lerp(0.3, 1.0, grass_rand(s4));
    float3 tipv = up * cos(lean) + ld * sin(lean);
    // Wind: the trees' mean wind and gust band, plus rolling waves along the wind.
    WindData wn = f.wind;
    if ((f.grass.bufs.w & 4u) != 0u && wn.cfg.x > 0.5) {
        float speed = length(wn.wind.xz);
        float2 wd = speed > 1e-3 ? wn.wind.xz / speed : float2(0.0, 0.0);
        float band = wn.gust.x + dot(base.xz, wd) / max(wn.gust.y, 1.0);
        float g = 0.5 + 0.5 * sin(band * 6.2831853);
        float gust = g * g * g * wn.gust.z;
        float tw = wn.wind.w;
        float wave = 0.5 + 0.5 * sin(dot(base.xz, wd) * 0.9 - tw * (1.2 + 0.25 * speed) + 0.6 * sin(dot(base.xz, float2(-wd.y, wd.x)) * 0.35));
        float flick = sin(tw * (5.0 + 3.0 * grass_rand(s5)) + grass_rand(s5) * 40.0);
        float amt = speed * 0.06 * (1.0 - ty.b.z) * (0.35 + 0.9 * gust + 0.7 * wave) + 0.04 * (1.0 - ty.b.z) * flick * min(speed * 0.2, 1.0);
        float3 wv = float3(wd.x, 0.0, wd.y);
        wv -= up * dot(wv, up);
        tipv = normalize(tipv + wv * clamp(amt, -0.3, 1.4));
    }
    float bend = saturate(1.0 - dot(tipv, up));
    o.p2 = o.p0 + tipv * (o.h * (1.0 - 0.18 * bend));
    float rise = max(dot(o.p2 - o.p0, up), 0.25 * o.h);
    float3 across = (o.p2 - o.p0) - up * dot(o.p2 - o.p0, up);
    float curve = ty.b.w;
    o.p1 = o.p0 + up * (rise * lerp(0.6, 1.05, curve)) + across * lerp(0.5, 0.05, curve);
    // Width across the lean (blades bend over their flat face), twisted a little.
    float3 lh = across;
    lh = dot(lh, lh) > 1e-8 ? normalize(lh) : ld;
    float tw2 = (grass_rand(s5) - 0.5) * 1.1;
    float3 sd = cross(up, lh);
    o.side = normalize(sd * cos(tw2) + lh * sin(tw2));
    o.w = ty.a.z * wscale * lerp(0.8, 1.2, grass_rand(s1 ^ 0x55u));
    o.head = ty.mid.w;
    o.head_w = ty.tip.w;
    o.shade = lerp(0.82, 1.15, grass_rand(s4 ^ 0x33u));
    o.t_ao = saturate(hc / 0.8);
    return o;
}

// The blade's width axis turned part way towards facing the camera (thin blades seen edge-on
// would vanish): the raster and the resolve use the same axis.
float3 grass_view_side(GrassBlade b, float t, float3 p) {
    float3 tg = normalize(grass_tangent(b, t) + b.up * 1e-4);
    float3 v = normalize(p);
    float3 vs = cross(tg, v);
    if (dot(vs, vs) < 1e-8) {
        return b.side;
    }
    vs = normalize(vs);
    vs = dot(vs, b.side) < 0.0 ? -vs : vs;
    return normalize(lerp(b.side, vs, 0.55));
}
