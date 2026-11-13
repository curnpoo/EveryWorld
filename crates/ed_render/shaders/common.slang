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
