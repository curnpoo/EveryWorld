#include <metal_stdlib>
using namespace metal;

// Native world shaders: 4-cascade shadowed PBR into linear HDR + a normal/roughness target, analytic sky,
// then a resolve pass (exposure + filmic tonemap) into the drawable. All positions are origin-relative f32.

struct Vtx { packed_float3 p; packed_float3 n; packed_float2 uv; packed_float4 c; };
struct FrameU {
    float4x4 vp;
    float4 sun;        // xyz toward sun, w = day factor 0..1
    float4 cam;
    float4 sky;
    float4 fog;        // x density
    float4x4 csm[5];
    float4 csm_info[5]; // x world texel size, y 1/zrange
    float4 csm_split;   // view-depth far of cascades 0-3
    float4 csm_far;     // x = view-depth far of the terrain-only cascade 4
    float4 cr; float4 cu; float4 cf;  // camera basis; cr/cu pre-scaled by tan(fov/2)
    float4 params;      // x exposure, y shadows on, z 1/shadow size
};
struct DrawU { float4x4 model; float4 tint; float4 emissive; float4 mat; }; // mat: rough, metal, cutoff, unlit

struct VOut { float4 pos [[position]]; float3 nrm; float2 uv; float4 col; float3 wpos; };

constexpr sampler ts(address::repeat, filter::linear, mip_filter::linear, max_anisotropy(16));
constexpr sampler tsc(address::clamp_to_edge, filter::linear, mip_filter::linear, max_anisotropy(16));
constexpr sampler shs(coord::normalized, filter::linear, address::clamp_to_edge, compare_func::less_equal);

vertex VOut world_vs(uint vid [[vertex_id]],
                     device const Vtx* verts [[buffer(0)]],
                     constant FrameU& F [[buffer(1)]],
                     constant DrawU& D [[buffer(2)]]) {
    Vtx v = verts[vid];
    float4 w = D.model * float4(float3(v.p), 1.0);
    VOut o;
    o.pos = F.vp * w;
    o.wpos = w.xyz;
    o.nrm = (D.model * float4(float3(v.n), 0.0)).xyz;
    o.uv = float2(v.uv);
    o.col = float4(v.c) * D.tint;
    return o;
}

// ---- Shadow pass ----
struct SOut { float4 pos [[position]]; float2 uv; };
vertex SOut shadow_vs(uint vid [[vertex_id]], device const Vtx* verts [[buffer(0)]],
                      constant float4x4& LVP [[buffer(1)]], constant DrawU& D [[buffer(2)]]) {
    Vtx v = verts[vid];
    SOut o;
    o.pos = LVP * (D.model * float4(float3(v.p), 1.0));
    o.uv = float2(v.uv);
    return o;
}
fragment void shadow_fs_mask(SOut in [[stage_in]], constant DrawU& D [[buffer(2)]], texture2d<float> tex [[texture(0)]]) {
    float a = tex.sample(ts, in.uv).a * D.tint.a;
    if (a < D.mat.z) discard_fragment();
}

// ---- Sky ----
float3 sky_rad(float3 d, float3 sun, float day, float disc_k = 1.0) {
    float h = d.y;
    float3 zen = float3(0.025, 0.09, 0.40), hor = float3(0.36, 0.55, 0.95);
    float t = pow(saturate(1.0 - max(h, 0.0)), 6.5);
    float3 c = mix(zen, hor, t);
    float mu = dot(d, sun);
    float warm = saturate(1.0 - sun.y * 2.5);
    c += float3(1.0, 0.42, 0.14) * pow(saturate(mu * 0.5 + 0.5), 5.0) * warm * 0.85 * (0.4 + t);
    c = mix(c, c * float3(1.0, 0.7, 0.55), warm * 0.4);
    c += float3(1.0, 0.9, 0.7) * pow(saturate(mu), 28.0) * 0.35;
    float3 ground = float3(0.30, 0.31, 0.32);
    c = mix(c, ground, saturate(-h * 6.0));
    float disk = smoothstep(0.99965, 0.99985, mu) * step(0.0, h);
    return c * day + float3(1.0, 0.95, 0.85) * disk * 60.0 * day * disc_k;
}

struct FsOut { float4 pos [[position]]; float2 ndc; };
vertex FsOut fs_vs(uint vid [[vertex_id]]) {
    float2 p = float2((vid << 1) & 2, vid & 2);
    FsOut o; o.pos = float4(p * 2.0 - 1.0, 0.0, 1.0); o.ndc = p * 2.0 - 1.0;
    return o;
}

struct SkyOut { float4 c [[color(0)]]; float4 n [[color(1)]]; };
fragment SkyOut sky_fs(FsOut in [[stage_in]], constant FrameU& F [[buffer(1)]]) {
    float3 d = normalize(F.cf.xyz + F.cr.xyz * in.ndc.x + F.cu.xyz * in.ndc.y);
    SkyOut o; o.c = float4(sky_rad(d, F.sun.xyz, F.sun.w), 1.0); o.n = float4(0.0);
    return o;
}

// ---- Lighting ----
float shadow_vis(float3 wpos, float3 N, float vd, constant FrameU& F, depth2d_array<float> sm) {
    if (F.params.y < 0.5 || vd > F.csm_far.x) return 1.0;
    int ci = 4;
    for (int i = 0; i < 4; i++) { if (vd < F.csm_split[i]) { ci = i; break; } }
    if (F.params.w > 5.5 && F.params.w < 9.5) { ci = int(F.params.w + 0.5) - 6; }
    float4 inf = F.csm_info[ci];
    // Cascades >=1 render decimated proxies (grid 1.5-4 m), so receivers need a bias that covers that error.
    float nb = ci == 0 ? 2.0 : (ci == 1 ? 4.0 : (ci == 4 ? 0.8 : 6.0));
    float3 p = wpos + N * inf.x * nb;
    float4 c = F.csm[ci] * float4(p, 1.0);
    float2 uv = float2(c.x * 0.5 + 0.5, 0.5 - c.y * 0.5);
    float z = c.z - inf.y * inf.x * nb * 1.5;
    if (F.params.w > 9.5) { float st = sm.read(uint2(uv * 2048.0), ci); return saturate(0.5 + (z - st) / inf.y / 20.0); }
    float vis = 0.0;
    float2 o = float2(F.params.z);
    for (int y = -1; y <= 1; y++)
        for (int x = -1; x <= 1; x++)
            vis += sm.sample_compare(shs, uv + float2(x, y) * o, ci, z);
    vis /= 9.0;
    float fade = saturate((F.csm_far.x - vd) / (F.csm_far.x * 0.2));
    return mix(1.0, vis, fade);
}

float3 shade_ex(VOut in, float4 base, bool front, constant FrameU& F, constant DrawU& D, depth2d_array<float> sm, thread float3& Nout, float rough_in, float ao_in) {
    float3 N = normalize(in.nrm);
    if (!front) N = -N;
    Nout = N;
    float3 toCam = F.cam.xyz - in.wpos;
    float dist = length(toCam);
    float3 V = toCam / max(dist, 1e-4);
    float day = F.sun.w;
    float3 albedo = base.rgb;
    float metal = D.mat.y;
    float rough = clamp(rough_in, mix(0.6, 0.045, metal), 1.0);
    float3 L = F.sun.xyz;
    float3 col;
    if (D.mat.w > 0.5) {
        col = albedo;
    } else {
        float ndl = saturate(dot(N, L));
        float vd = dot(in.wpos - F.cam.xyz, F.cf.xyz);
        float vis = ndl > 0.0 ? shadow_vis(in.wpos, N, vd, F, sm) : 0.0;
        float3 sunc = mix(float3(1.0, 0.62, 0.34), float3(1.0, 0.96, 0.88), saturate(L.y * 2.5)) * 6.4 * day;
        float3 H = normalize(L + V);
        float ndh = saturate(dot(N, H)), ndv = max(dot(N, V), 1e-3), vdh = saturate(dot(V, H));
        float a = rough * rough, a2 = a * a;
        float dd = ndh * ndh * (a2 - 1.0) + 1.0;
        float Dg = a2 / (M_PI_F * dd * dd);
        float k = (rough + 1.0) * (rough + 1.0) / 8.0;
        float G = (ndl / (ndl * (1.0 - k) + k)) * (ndv / (ndv * (1.0 - k) + k));
        float3 F0 = mix(float3(0.04), albedo, metal);
        float3 Fr = F0 + (1.0 - F0) * pow(1.0 - vdh, 5.0);
        float3 spec = Dg * G * Fr / max(4.0 * ndl * ndv, 1e-3);
        float3 diffc = albedo * (1.0 - metal);
        float3 direct = (diffc / M_PI_F * (1.0 - Fr) + spec) * sunc * ndl * vis;
        // Ambient: sky dome above, bounce below, plus a roughness-blurred sky reflection.
        // Ambient: desaturated sky dome (real skylight is far whiter than the zenith colour), warm ground bounce.
        float3 upr = sky_rad(float3(0, 1, 0), L, day) * 0.5 + sky_rad(normalize(float3(N.x, 0.25, N.z)), L, day) * 0.5;
        float ul = dot(upr, float3(0.3, 0.59, 0.11));
        float3 up = mix(float3(ul), upr, 0.45) * 1.0;
        float3 ground = float3(0.34, 0.31, 0.27) * day * (0.5 + 0.5 * L.y);
        float3 amb = mix(ground, up, N.y * 0.5 + 0.5);
        float3 R = reflect(-V, N);
        float gloss = (1.0 - rough) * (1.0 - rough) * mix(0.35, 1.0, metal);
        float3 env = mix(amb, sky_rad(R, L, day), gloss);
        float3 Fenv = (F0 + (max(float3(1.0 - rough), F0) - F0) * pow(1.0 - ndv, 5.0)) * mix(0.3, 1.0, metal);
        float specocc = mix(1.0, saturate(R.y * 4.0 + 1.0), 0.8);
        float3 ambient = (diffc * amb * (1.0 - Fenv * 0.5) + env * Fenv * specocc * gloss) * ao_in;
        col = direct + ambient;
        if (F.params.w > 0.5) { int m = int(F.params.w + 0.5); return m == 1 ? N * 0.5 + 0.5 : ((m == 2 || m >= 6) ? float3(vis) : (m == 3 ? ambient : (m == 4 ? float3(vd < F.csm_split[0] ? 1 : 0, vd >= F.csm_split[0] && vd < F.csm_split[1] ? 1 : 0, vd >= F.csm_split[1] && vd < F.csm_split[2] ? 1 : 0) : float3(ndl)))); }
    }
    col += D.emissive.rgb;
    // Aerial-perspective fog toward the sky colour in the view direction.
    float3 fd = -V; fd.y = max(fd.y, 0.10);
    float f = 1.0 - exp(-dist * F.fog.x);
    col = mix(col, sky_rad(normalize(fd), L, day, 0.0), saturate(f));
    return col;
}


float3 shade(VOut in, float4 base, bool front, constant FrameU& F, constant DrawU& D, depth2d_array<float> sm, thread float3& Nout) {
    return shade_ex(in, base, front, F, D, sm, Nout, D.mat.x, 1.0);
}

// ---- Terrain surface: baked DEM detail, per-pixel rock/snow, strata, micro relief ----
float h21(float2 p) { p = fract(p * float2(123.34, 456.21)); p += dot(p, p + 45.32); return fract(p.x * p.y); }
float vnoise(float2 p) {
    float2 i = floor(p), f = fract(p); f = f * f * (3.0 - 2.0 * f);
    return mix(mix(h21(i), h21(i + float2(1, 0)), f.x), mix(h21(i + float2(0, 1)), h21(i + float2(1, 1)), f.x), f.y);
}
float fbm3(float2 p) { float a = 0.5, s = 0.0; for (int i = 0; i < 3; i++) { s += a * vnoise(p); p = p * 2.03 + 17.1; a *= 0.5; } return s; }
float detail_dec(float x) { return tan((clamp(x, 0.02, 0.98) - 0.5) * 3.14159265); }
// Gradient of fbm at p (finite difference).
float2 fbm_grad(float2 p) { const float e = 0.06; float h = fbm3(p); return float2(fbm3(p + float2(e, 0)) - h, fbm3(p + float2(0, e)) - h) / e; }

// Updates `n` (world normal) and `base.rgb` (satellite albedo) for a terrain pixel.
void terrain_surf(thread float3& n, thread float4& base, float3 wpos, float dist, float4 det) {
    float ny = max(n.y, 0.08);
    float wall = 1.0 - smoothstep(0.55, 0.8, 1.0 - n.y);
    float2 dg = float2(detail_dec(det.r), detail_dec(det.g)) * wall;
    n = normalize(n / ny + float3(-dg.x, 0.0, -dg.y));
    float slope = 1.0 - clamp(n.y, 0.0, 1.0);
    float curv = clamp(detail_dec(det.b), -3.0, 3.0); // + gully, - ridge
    float3 a = base.rgb;
    // Sentinel-2 composites carry magenta casts on dry scrub/bare soil: lift green toward the R/B mean.
    a.g = mix(a.g, max(a.g, 0.5 * (a.r + a.b) * 1.05), saturate((min(a.r, a.b) - a.g) * 20.0) * 0.9);
    a.b = mix(a.b, min(a.b, a.r * 0.8), saturate((a.b - a.g) * 10.0) * 0.5);
    const float3 lw = float3(0.2126, 0.7152, 0.0722);
    float lum = dot(a, lw);
    float mx = max(a.r, max(a.g, a.b)), mn = min(a.r, min(a.g, a.b));
    float sat = (mx - mn) / max(mx, 1e-3);
    float nz = fbm3(wpos.xz * 0.045);
    float snow_s = smoothstep(0.30, 0.55, lum) * (1.0 - smoothstep(0.12, 0.32, sat));
    float snow_p = snow_s * (1.0 - smoothstep(0.30, 0.50, slope - curv * 0.06 + (nz - 0.5) * 0.10));
    float rock_f = smoothstep(0.30, 0.55, slope) * (1.0 - snow_p);
    float3 rock = float3(1.0, 0.97, 0.92) * mix(0.17, clamp(lum, 0.05, 0.30), 0.35) * (1.0 + (nz - 0.5) * 0.6);
    a = mix(a, rock, rock_f * mix(0.8, 1.0, smoothstep(0.55, 0.8, slope)));
    // Bedrock strata: warped horizontal bands.
    float band = 0.5 + 0.5 * sin(wpos.y * 0.55 + nz * 9.0 + fbm3(wpos.xz * 0.012) * 6.0);
    a *= 1.0 + (band - 0.5) * 0.45 * rock_f;
    a *= 1.0 - 0.22 * max(curv, 0.0) * (1.0 - snow_p);
    // Snow: lift toward near-white, cool-neutral.
    float lum_now = max(dot(a, lw), 0.02);
    a *= mix(1.0, max(1.0, 0.8 / lum_now), snow_p * 0.9);
    a = mix(a, float3(dot(a, lw)) * float3(0.97, 0.99, 1.03), snow_p * 0.35);
    // Micro relief: planar on flat ground, projected on faces; rock is rougher than snow.
    float amp = (0.15 + rock_f * 1.0 + (1.0 - snow_p) * 0.2) * (1.0 - smoothstep(40.0, 500.0, dist));
    float amp2 = (0.2 + rock_f * 0.8) * (1.0 - smoothstep(300.0, 3000.0, dist));
    float wf = 1.0 - smoothstep(0.22, 0.5, slope);
    float2 q = float2(wpos.x + wpos.z, wpos.y);
    float2 g1f = fbm_grad(wpos.xz * 0.9), g1w = fbm_grad(q * 0.9);
    float2 g2f = fbm_grad(wpos.xz * 0.07), g2w = fbm_grad(q * 0.07);
    float3 hn = normalize(float3(n.x, 0.0, n.z) + float3(1e-4));
    float3 Pf = float3(-g1f.x, 0.0, -g1f.y) * amp * 0.5 + float3(-g2f.x, 0.0, -g2f.y) * amp2 * 0.6;
    float3 Pw = (hn * -g1w.x + float3(0.0, -g1w.y, 0.0)) * amp * 0.5 + (hn * -g2w.x + float3(0.0, -g2w.y, 0.0)) * amp2 * 0.6;
    n = normalize(n + mix(Pw, Pf, wf));
    // Distance detail: satellite imagery is blurry far away, so add world-space albedo variation
    // (tilled-field / scrub / rock mottling) whose strength grows as real texel detail runs out.
    float dfar = smoothstep(120.0, 1500.0, dist);
    float v = fbm3(wpos.xz / 7.0) * 0.4 + fbm3(wpos.xz / 45.0) * 0.35 + fbm3(wpos.xz / 300.0) * 0.25;
    a *= 1.0 + (v - 0.5) * 1.0 * (0.25 + 0.75 * dfar) * (1.0 - snow_p * 0.7);
    a *= mix(float3(1.06, 1.0, 0.93), float3(0.95, 1.0, 1.05), fbm3(wpos.xz / 170.0)) * 0.5 + 0.5;
    base.rgb = a;
}

struct GOut { float4 c [[color(0)]]; float4 n [[color(1)]]; };
fragment GOut world_fs(VOut in [[stage_in]], bool front [[front_facing]],
                       constant FrameU& F [[buffer(1)]], constant DrawU& D [[buffer(2)]],
                       texture2d<float> tex [[texture(0)]], depth2d_array<float> sm [[texture(1)]],
                       texture2d<float> tex2 [[texture(2)]]) {
    float4 base;
    VOut iv = in;
    if (D.emissive.w > 0.5) {
        // Terrain tile: clamp + half-texel inset so bilinear never wraps across tile edges.
        float n = float(tex.get_width());
        float2 st = in.uv * ((n - 1.0) / n) + 0.5 / n;
        base = tex.sample(tsc, st) * in.col;
        float4 det = tex2.sample(tsc, st);
        float3 nn = normalize(in.nrm);
        float dist = length(F.cam.xyz - in.wpos);
        terrain_surf(nn, base, in.wpos, dist, det);
        iv.nrm = nn;
    } else {
        base = tex.sample(ts, in.uv) * in.col;
        // Tame over-saturated orange (roof palettes) toward terracotta/brick.
        float mx = max(base.r, max(base.g, base.b)), mn = min(base.r, min(base.g, base.b));
        float osat = (mx - mn) / max(mx, 1e-3);
        float orange = step(1.3 * base.g, base.r) * smoothstep(0.45, 0.8, osat);
        base.rgb = mix(base.rgb, float3(dot(base.rgb, float3(0.2126, 0.7152, 0.0722))) * float3(1.0, 0.92, 0.85), orange * 0.45);
    }
    if (D.mat.z > 0.0 && base.a < D.mat.z) discard_fragment();
    float3 N;
    float3 col = shade(iv, base, front, F, D, sm, N);
    GOut o; o.c = float4(col, 1.0); o.n = float4(N * 0.5 + 0.5, D.mat.x);
    return o;
}
struct BOut { float4 c [[color(0)]]; float4 n [[color(1)]]; };
fragment BOut world_fs_blend(VOut in [[stage_in]], bool front [[front_facing]],
                             constant FrameU& F [[buffer(1)]], constant DrawU& D [[buffer(2)]],
                             texture2d<float> tex [[texture(0)]], depth2d_array<float> sm [[texture(1)]]) {
    float4 base = tex.sample(ts, in.uv) * in.col;
    float3 N;
    float3 col = shade(in, base, front, F, D, sm, N);
    BOut o; o.c = float4(col, base.a); o.n = float4(0.0);
    return o;
}

// ---- Ground layers (port of ed_game shaders/terrain.wesl): 12 splatted PBR layers ----
struct GroundU { float4 size[3]; float4 bump[3]; float4 misc; }; // misc.x strength, misc.y on
struct GT { texture2d_array<float> al; texture2d_array<float> nr; texture2d<float> mt; };
struct TOut { float4 pos [[position]]; float3 nrm; float2 uv; float4 col; float3 wpos; float4 s0; float4 s1; float4 s2; };
struct SplatV { uchar4 a; uchar4 b; uchar4 c; };
vertex TOut terrain_vs(uint vid [[vertex_id]], device const Vtx* verts [[buffer(0)]], constant FrameU& F [[buffer(1)]],
                       constant DrawU& D [[buffer(2)]], device const SplatV* spl [[buffer(3)]]) {
    Vtx v = verts[vid];
    float4 w = D.model * float4(float3(v.p), 1.0);
    TOut o;
    o.pos = F.vp * w; o.wpos = w.xyz;
    o.nrm = (D.model * float4(float3(v.n), 0.0)).xyz;
    o.uv = float2(v.uv); o.col = float4(v.c) * D.tint;
    SplatV s = spl[vid];
    o.s0 = float4(s.a) / 255.0; o.s1 = float4(s.b) / 255.0; o.s2 = float4(s.c) / 255.0;
    return o;
}
float2 rot2(float2 p, float a) { float c = cos(a), s = sin(a); return float2(c * p.x - s * p.y, s * p.x + c * p.y); }
float4 mac(texture2d<float> mt, float2 p, float s, float a, float2 off) {
    float4 x = mt.sample(ts, rot2(p / s, a) + off);
    float4 y = mt.sample(ts, rot2(p / (s * 1.618034), a + 2.39996) + off.yx + float2(0.37, 0.11));
    return clamp((x + y - 1.0) * 0.72 + 0.5, 0.0, 1.0);
}
struct Layer { float3 albedo; float height; float3 normal; float rough; float ao; };
struct Tap { float4 ah; float4 nr; };
Tap gtap(thread const GT& g, int i, float2 uv, float2 dx, float2 dy) {
    Tap t; t.ah = g.al.sample(ts, uv, uint(i), gradient2d(dx, dy)); t.nr = g.nr.sample(ts, uv, uint(i), gradient2d(dx, dy)); return t;
}
Tap gtap_var(thread const GT& g, int i, float2 uv, float2 dx, float2 dy, float vm) {
    float w = smoothstep(0.42, 0.58, vm);
    float ang = 2.1 + float(i) * 0.37, k = 0.73;
    Tap a = {float4(0), float4(0)}, b = {float4(0), float4(0)};
    if (w < 0.999) a = gtap(g, i, uv, dx, dy);
    if (w > 0.001) {
        b = gtap(g, i, rot2(uv * k, ang) + float2(0.37, 0.61), rot2(dx * k, ang), rot2(dy * k, ang));
        float2 bn = rot2(b.nr.xy * 2.0 - 1.0, -ang) * 0.5 + 0.5;
        b.nr = float4(bn, b.nr.z, b.nr.w);
    }
    if (w <= 0.001) return a;
    if (w >= 0.999) return b;
    Tap t; t.ah = mix(a.ah, b.ah, w); t.nr = mix(a.nr, b.nr, w); return t;
}
float2 hash22(float2 p) {
    float2 r = float2(dot(p, float2(127.1, 311.7)), dot(p, float2(269.5, 183.3)));
    return fract(sin(r) * 43758.5453);
}
float lsize(constant GroundU& G, int i) { return G.size[i / 4][i % 4]; }
float lbump(constant GroundU& G, int i) { return G.bump[i / 4][i % 4]; }

Layer sample_planar(thread const GT& g, constant GroundU& G, int i, float2 p, float2 dp_dx, float2 dp_dy, float hexw, float3 n, float strength, float vm) {
    float inv = 1.0 / lsize(G, i);
    float2 st = p * inv, dx = dp_dx * inv, dy = dp_dy * inv;
    float4 ah = float4(0), nr = float4(0);
    float2 nxy = float2(0);
    if (hexw < 0.5) {
        Tap t = gtap_var(g, i, st, dx, dy, vm);
        ah = t.ah; nr = t.nr; nxy = t.nr.xy * 2.0 - 1.0;
    } else {
        bool directional = i == 5 || i == 6 || i == 9 || i == 10;
        float2 sk = st * 3.4641016;
        float2 skewed = float2(sk.x, -0.57735027 * sk.x + 1.1547005 * sk.y);
        float2 base = floor(skewed), f = fract(skewed);
        float z = 1.0 - f.x - f.y;
        float3 wv; float2 v[3];
        if (z > 0.0) { wv = float3(z, f.y, f.x); v[0] = base; v[1] = base + float2(0, 1); v[2] = base + float2(1, 0); }
        else { wv = float3(-z, 1.0 - f.y, 1.0 - f.x); v[0] = base + float2(1, 1); v[1] = base + float2(1, 0); v[2] = base + float2(0, 1); }
        wv = wv * wv * wv * wv; wv = wv / (wv.x + wv.y + wv.z);
        for (int k = 0; k < 3; k++) {
            if (wv[k] < 0.01) continue;
            float2 h = hash22(v[k] + float2(float(i) * 17.0, float(i) * 5.0));
            float ang = directional ? 0.0 : h.x * 6.2831853;
            float2 off = h * 9.0;
            Tap t = gtap(g, i, rot2(st, ang) + off, rot2(dx, ang), rot2(dy, ang));
            ah += t.ah * wv[k]; nr += t.nr * wv[k];
            nxy += rot2(t.nr.xy * 2.0 - 1.0, -ang) * wv[k];
        }
    }
    Layer l; l.albedo = ah.rgb; l.height = ah.a;
    float2 tn = nxy * strength * lbump(G, i);
    l.normal = normalize(n + float3(tn.x, 0.0, tn.y));
    l.rough = nr.z; l.ao = nr.w;
    return l;
}
Layer sample_triplanar(thread const GT& g, constant GroundU& G, int i, float3 p, float3 dp_dx, float3 dp_dy, float3 n, float strength, float vm) {
    float inv = 1.0 / lsize(G, i);
    float3 bw = pow(abs(n), float3(4.0)); bw = bw / (bw.x + bw.y + bw.z);
    Tap tx = gtap_var(g, i, p.zy * inv, dp_dx.zy * inv, dp_dy.zy * inv, fract(vm + 0.31));
    Tap ty = gtap_var(g, i, p.xz * inv, dp_dx.xz * inv, dp_dy.xz * inv, vm);
    Tap tz = gtap_var(g, i, p.xy * inv, dp_dx.xy * inv, dp_dy.xy * inv, fract(vm + 0.67));
    float k = strength * lbump(G, i);
    float2 nx = (tx.nr.xy * 2.0 - 1.0) * k, ny = (ty.nr.xy * 2.0 - 1.0) * k, nz = (tz.nr.xy * 2.0 - 1.0) * k;
    float3 wx = float3(n.x, nx.y + n.y, nx.x + n.z);
    float3 wy = float3(ny.x + n.x, n.y, ny.y + n.z);
    float3 wz = float3(nz.x + n.x, nz.y + n.y, n.z);
    Layer l;
    l.albedo = tx.ah.rgb * bw.x + ty.ah.rgb * bw.y + tz.ah.rgb * bw.z;
    l.height = tx.ah.a * bw.x + ty.ah.a * bw.y + tz.ah.a * bw.z;
    l.normal = normalize(wx * bw.x + wy * bw.y + wz * bw.z);
    l.rough = tx.nr.z * bw.x + ty.nr.z * bw.y + tz.nr.z * bw.z;
    l.ao = tx.nr.w * bw.x + ty.nr.w * bw.y + tz.nr.w * bw.z;
    return l;
}
float gband(float feature, float fp) { return smoothstep(fp * 1.5, fp * 6.0, feature); }
bool is_living(int i) { return i == 0 || i == 1 || i == 2 || i == 5; }

fragment GOut terrain_fs(TOut in [[stage_in]], bool front [[front_facing]],
                         constant FrameU& F [[buffer(1)]], constant DrawU& D [[buffer(2)]], constant GroundU& G [[buffer(3)]],
                         texture2d<float> tex [[texture(0)]], depth2d_array<float> sm [[texture(1)]], texture2d<float> tex2 [[texture(2)]],
                         texture2d_array<float> gal [[texture(3)]], texture2d_array<float> gnr [[texture(4)]], texture2d<float> mtex [[texture(5)]]) {
    float nt = float(tex.get_width());
    float2 stc = in.uv * ((nt - 1.0) / nt) + 0.5 / nt;
    float4 sat_s = tex.sample(tsc, stc);
    float4 det = tex2.sample(tsc, stc);
    float3 n = normalize(in.nrm);
    VOut iv; iv.pos = in.pos; iv.nrm = n; iv.uv = in.uv; iv.col = in.col; iv.wpos = in.wpos;
    float3 wp = in.wpos;
    float dist = length(F.cam.xyz - wp);
    float3 N; float3 col;
    if (G.misc.y < 0.5) {
        float4 base = sat_s * in.col;
        terrain_surf(n, base, wp, dist, det);
        iv.nrm = n;
        col = shade(iv, base, front, F, D, sm, N);
        GOut o; o.c = float4(col, 1.0); o.n = float4(N * 0.5 + 0.5, D.mat.x); return o;
    }
    {
        float ny = max(n.y, 0.08);
        float wall = 1.0 - smoothstep(0.55, 0.8, 1.0 - n.y);
        float2 dg = float2(detail_dec(det.r), detail_dec(det.g)) * wall;
        n = normalize(n / ny + float3(-dg.x, 0.0, -dg.y));
    }
    GT g = {gal, gnr, mtex};
    float3 p3 = wp; float2 p2 = p3.xz;
    float gsc = mix(1.0, 0.6, smoothstep(30.0, 250.0, dist));
    float3 dp3_dx = dfdx(p3) * gsc, dp3_dy = dfdy(p3) * gsc;
    float strength_base = G.misc.x * mix(1.0, 0.6, smoothstep(60.0, 900.0, dist));
    float4 m_c = mtex.sample(ts, rot2(p2 / 1777.0, 0.83));
    float4 m_b = mtex.sample(ts, rot2(p2 / 263.0, 1.91));
    float4 m_a = mac(mtex, p2, 41.0, 0.31, float2(0));
    float4 m_d = mac(mtex, p2, 23.0, 1.27, float2(0.31, 0.17));
    float4 m_e = mac(mtex, p2, 67.0, 2.71, float2(0.53, 0.81));
    float4 m_p = mac(mtex, p2, 29.0, 4.03, float2(0.70, 0.20));
    float fb = (m_d.b - 0.5) * 0.6 + (m_d.g - 0.5) * 0.9 + (m_e.b - 0.5) * 0.7;
    float fb2 = (m_e.g - 0.5) * 1.2 + (m_d.r - 0.5) * 0.6;
    float cell = m_p.a - 0.5;
    float4 m_v = mac(mtex, p2, 157.0, 5.17, float2(0.21, 0.67));
    float vm = clamp(m_v.g * 0.65 + m_e.a * 0.35 + (m_a.b - 0.5) * 0.25, 0.0, 1.0);
    float mid_fade = 1.0 - smoothstep(2500.0, 6000.0, dist);
    float strength = strength_base * mix(0.55, 1.35, clamp(m_b.a * 0.6 + m_v.b * 0.4, 0.0, 1.0));

    float w[12] = {in.s0.x, in.s0.y, in.s0.z, in.s0.w, in.s1.x, in.s1.y, in.s1.z, in.s1.w, in.s2.x, in.s2.y, in.s2.z, in.s2.w};
    float slope = 1.0 - clamp(n.y, 0.0, 1.0);
    float tri_t = smoothstep(0.10, 0.22, slope);
    float curv = clamp(detail_dec(det.b), -3.0, 3.0);
    {
        float3 vn = normalize(in.nrm);
        float vslope = 1.0 - clamp(vn.y, 0.0, 1.0);
        float jit = fb * 0.10;
        float cliff_v = smoothstep(0.18, 0.36, vslope);
        float cliff_p = smoothstep(0.18, 0.36, slope + jit);
        float add = max(cliff_p - cliff_v, 0.0) / max(1.0 - cliff_v, 1e-3);
        float drop = min(max(cliff_v - cliff_p, 0.0), w[8]);
        for (int i = 0; i < 12; i++) w[i] *= (1.0 - add);
        w[8] = w[8] + add - drop * (1.0 - add);
        w[7] = w[7] + drop * (1.0 - add);
        float apron = smoothstep(0.10, 0.22, slope + jit) * (1.0 - cliff_p) * smoothstep(0.0, 0.6, curv) * 0.6;
        for (int i = 0; i < 12; i++) w[i] *= (1.0 - apron);
        w[7] += apron;
        float w9 = w[9];
        float keep_v = 1.0 - smoothstep(0.28, 0.45, vslope);
        float alt = clamp(w9 / max(keep_v, 0.08), 0.0, 1.0) * step(0.004, w9);
        float hold = clamp(alt * 1.25 + curv * 0.18 - n.z * 0.22 + fb * 0.35 - 0.1, 0.0, 1.0);
        float steep_p = slope - curv * 0.06 + n.z * 0.04 + jit;
        float snow_p = hold * (1.0 - smoothstep(0.30, 0.50, steep_p));
        snow_p = max(snow_p, (w9 > 0.85 && slope < 0.5) ? w9 : 0.0);
        float rest = 1.0 - w9;
        if (rest > 0.05) { float k = (1.0 - snow_p) / rest; for (int i = 0; i < 12; i++) w[i] *= k; }
        else { for (int i = 0; i < 12; i++) w[i] = 0.0; w[8] = 1.0 - snow_p; }
        w[9] = snow_p;
    }
    float edge_amt = 0.32 * (0.5 + 0.5 * smoothstep(10.0, 150.0, dist));
    float wn[12];
    for (int i = 0; i < 12; i++) {
        float k = float(i % 3) - 1.0, k2 = float((i / 3) % 2) * 2.0 - 1.0;
        wn[i] = max(w[i] + (fb * k + fb2 * k2 * 0.6) * edge_amt * 2.0, 0.0);
    }
    float hexw = 1.0 - step(1200.0, dist);
    Layer layers[12];
    float hmax = -10.0;
    for (int i = 0; i < 12; i++) {
        if (w[i] < 0.012 || dist > 5000.0) continue;
        Layer l;
        if (tri_t < 0.01) l = sample_planar(g, G, i, p2, dp3_dx.xz, dp3_dy.xz, hexw, n, strength, vm);
        else if (tri_t > 0.99) l = sample_triplanar(g, G, i, p3, dp3_dx, dp3_dy, n, strength, vm);
        else {
            Layer a = sample_planar(g, G, i, p2, dp3_dx.xz, dp3_dy.xz, hexw, n, strength, vm);
            Layer b = sample_triplanar(g, G, i, p3, dp3_dx, dp3_dy, n, strength, vm);
            l.albedo = mix(a.albedo, b.albedo, tri_t); l.height = mix(a.height, b.height, tri_t);
            l.normal = normalize(mix(a.normal, b.normal, tri_t)); l.rough = mix(a.rough, b.rough, tri_t); l.ao = mix(a.ao, b.ao, tri_t);
        }
        layers[i] = l;
        hmax = max(hmax, l.height * 0.6 + wn[i]);
    }
    float depth = mix(0.18, 0.09, smoothstep(60.0, 400.0, dist));
    float total = 0.0, rough = 0.0, ao = 0.0;
    float3 albedo = float3(0), nsum = float3(0);
    float3 tint = in.col.rgb;
    float dry = clamp((m_b.r - 0.45) * 0.9 + (m_c.g - 0.5) * 0.8, -0.25, 0.45);
    for (int i = 0; i < 12; i++) {
        if (w[i] < 0.012 || dist > 5000.0) continue;
        Layer l = layers[i];
        float b = max(l.height * 0.6 + wn[i] - (hmax - depth), 0.0);
        if (b <= 0.0) continue;
        float3 c = l.albedo;
        if (is_living(i)) {
            c = c * tint;
            c = mix(c, c * float3(1.22, 1.06, 0.72), max(dry, 0.0));
            c = c * (1.0 + min(dry, 0.0) * float3(0.4, -0.15, 0.3));
        }
        albedo += c * b; nsum += l.normal * b; rough += l.rough * b; ao += l.ao * b; total += b;
    }
    if (total <= 0.0) { total = 1.0; albedo = float3(0.5); nsum = n; rough = 0.9; ao = 1.0; }
    albedo /= total; rough /= total; ao /= total;
    float3 nd = normalize(nsum);
    float bright = mix(0.86, 1.14, m_a.r) * mix(0.84, 1.16, m_b.b) * mix(0.88, 1.12, m_c.a);
    albedo *= bright;
    float living = 0.0;
    for (int i = 0; i < 12; i++) if (is_living(i)) living += w[i];
    living = clamp(living, 0.0, 1.0);
    float snow_w = w[9];
    float keepk = 1.0 - snow_w * 0.9;
    albedo *= (1.0 + fb * 0.55 * mid_fade * keepk);
    albedo *= (1.0 + fb2 * float3(0.16, 0.03, -0.14) * mid_fade * keepk);
    albedo *= mix(float3(1.0), 1.0 + cell * float3(0.30, 0.16, 0.34), living * mid_fade);
    albedo = max(albedo, float3(0.0));
    float gx = mac(mtex, p2 + float2(1.5, 0.0), 23.0, 1.27, float2(0.31, 0.17)).g - m_d.g;
    float gz = mac(mtex, p2 + float2(0.0, 1.5), 23.0, 1.27, float2(0.31, 0.17)).g - m_d.g;
    nd = normalize(nd + float3(-gx, 0.0, -gz) * 1.6 * mid_fade * (1.0 - 0.7 * snow_w) * (1.0 - smoothstep(0.7, 0.95, slope)));
    {
        float rock_w = clamp(w[8] + w[7] * 0.5, 0.0, 1.0) * (1.0 - snow_w);
        float hy = p3.y + (m_b.r - 0.5) * 30.0 + (m_a.g - 0.5) * 8.0;
        float b0 = sin(hy * 0.42) * 0.5 + sin(hy * 0.113 + 1.7) * 0.5;
        float band1 = sign(b0) * pow(abs(b0), 0.6);
        float strata = 1.0 + band1 * 0.30 * rock_w;
        albedo = albedo * strata * mix(float3(1.0), float3(1.04, 1.0, 0.95), clamp(band1, 0.0, 1.0) * rock_w);
        float cav = clamp(curv * 0.35, -0.4, 1.0);
        ao = ao * (1.0 - 0.5 * max(cav, 0.0)) * (1.0 + 0.15 * max(-cav, 0.0));
        albedo = albedo * (1.0 - 0.25 * max(cav, 0.0) * (1.0 - snow_w));
    }
    float wet = in.col.a;
    albedo *= (1.0 - 0.45 * wet);
    rough = mix(rough, 0.22, wet * 0.8);
    float has_sat = smoothstep(0.2, 0.4, sat_s.a);
    float3 sat_lin = max(sat_s.rgb, 0.0);
    float water_f = has_sat * clamp((1.0 - sat_s.a) * 2.0, 0.0, 1.0);
    const float3 lum_w = float3(0.2126, 0.7152, 0.0722);
    float lp = dot(albedo, lum_w), ls = max(dot(sat_lin, lum_w), 0.02);
    float3 chroma = min(sat_lin / ls, float3(3.0));
    float3 steered = chroma * mix(lp, ls, 0.5 * (1.0 - snow_w));
    float steer_f = has_sat * smoothstep(120.0, 1500.0, dist) * 0.7 * (1.0 - snow_w * 0.8) * (1.0 - smoothstep(0.6, 0.85, slope));
    albedo = mix(albedo, steered, steer_f * (1.0 - water_f));
    float far_f = has_sat * smoothstep(600.0, 4500.0, dist) * 0.9 * (1.0 - 0.6 * snow_w);
    float detail = clamp(dot(albedo, lum_w) / max(dot(albedo, float3(0.3333)), 0.02), 0.6, 1.4);
    float fp = dist * 0.0016;
    float3 sat_col = sat_lin;
    {
        float4 m_f = mac(mtex, p2, 131.0, 3.37, float2(0.13, 0.57));
        float4 m_g = mac(mtex, p2, 487.0, 0.57, float2(0.29, 0.43));
        float4 m_h = mtex.sample(ts, p2 / 4100.0 + float2(0.61, 0.07));
        float fn_ = (m_f.b - 0.5) * gband(8.0, fp) + (m_f.g - 0.5) * gband(16.0, fp) + (m_g.b - 0.5) * gband(30.0, fp) + (m_g.g - 0.5) * gband(60.0, fp)
                  + (m_c.b - 0.5) * gband(111.0, fp) + (m_h.b - 0.5) * gband(256.0, fp) + (m_h.g - 0.5) * gband(512.0, fp);
        float hue = (m_g.r - 0.5) * gband(120.0, fp) + (m_h.r - 0.5) * gband(900.0, fp);
        float amp = clamp(fn_ * 1.1, -0.45, 0.45) * (1.0 - snow_w * 0.85);
        float hv = hue * (1.0 - snow_w * 0.85);
        sat_col = mix(float3(dot(sat_col, lum_w)), sat_col, 1.2);
        sat_col = sat_col * (1.0 + amp) * (1.0 + hv * float3(0.35, 0.08, -0.30));
        albedo = albedo * (1.0 + amp * (1.0 - has_sat) * smoothstep(200.0, 1500.0, dist));
    }
    albedo = mix(albedo, max(sat_col, 0.0) * mix(1.0, detail, 0.5), far_f);
    {
        float rock_f = smoothstep(0.30, 0.55, slope) * (1.0 - snow_w) * (1.0 - water_f) * max(steer_f, far_f);
        float rl = clamp(dot(albedo, lum_w), 0.05, 0.30);
        float3 rock = float3(1.0, 0.95, 0.88) * mix(0.17, rl, 0.35) * (1.0 + (m_b.g - 0.5) * 0.4);
        albedo = mix(albedo, rock, rock_f * mix(0.85, 1.0, smoothstep(0.55, 0.8, slope)));
    }
    float snow_t = clamp(snow_w * 1.4, 0.0, 1.0);
    float lum_now = max(dot(albedo, lum_w), 0.02);
    albedo *= mix(1.0, max(1.0, 0.78 / lum_now), snow_t * 0.9);
    albedo = mix(albedo, float3(dot(albedo, lum_w)) * float3(0.97, 0.99, 1.03), snow_t * 0.35);
    albedo = mix(albedo, sat_lin, water_f);
    rough = mix(rough, mix(0.07, 0.35, smoothstep(2000.0, 60000.0, dist)), water_f);
    ao = mix(ao, 1.0, water_f);
    nd = normalize(mix(nd, n, water_f));
    float land_f = 1.0 - water_f;
    rough = mix(rough, max(rough, 0.97), land_f * smoothstep(25.0, 500.0, dist));
    rough = mix(rough, max(rough, 0.8), land_f * (1.0 - wet));
    iv.nrm = nd;
    col = shade_ex(iv, float4(albedo, 1.0), front, F, D, sm, N, clamp(rough, 0.04, 1.0), ao);
    GOut o; o.c = float4(col, 1.0); o.n = float4(N * 0.5 + 0.5, rough); return o;
}

// ---- Resolve: exposure + filmic tonemap -> display ----
float3 aces(float3 x) {
    const float a = 2.51, b = 0.03, c = 2.43, d = 0.59, e = 0.14;
    return saturate((x * (a * x + b)) / (x * (c * x + d) + e));
}
// Blender Filmic: the same log2 encode (-11..12 EV around 0.18) + 64^3 LUT Bevy uses. LUT output is linear display.
float3 filmic(texture3d<float> lut, float3 x) {
    constexpr sampler ls(filter::linear, address::clamp_to_edge);
    x = max(x, 0.0);
    x = select(x, x + 0.00001525878, x < 0.00003051757);
    float3 n = saturate((log2(x / 0.18) + 11.0) / 23.0);
    return lut.sample(ls, n * (63.0 / 64.0) + 0.5 / 64.0).rgb;
}
fragment float4 resolve_fs(FsOut in [[stage_in]], constant FrameU& F [[buffer(1)]], texture2d<float> hdr [[texture(0)]], texture3d<float> lut [[texture(1)]], texture2d<float> aot [[texture(2)]], texture2d<float> bloomt [[texture(3)]], texture2d<float> bouncet [[texture(4)]], constant float4& pf [[buffer(2)]]) {
    constexpr sampler s(filter::linear, address::clamp_to_edge);
    float2 uv = float2(in.ndc.x * 0.5 + 0.5, 0.5 - in.ndc.y * 0.5);
    float3 h = hdr.sample(s, uv).rgb;
    {
        // FXAA-lite on compressed HDR luma: blend along the edge tangent only where contrast is high.
        float2 px = 1.0 / float2(hdr.get_width(), hdr.get_height());
        float3 cN = min(hdr.sample(s, uv + float2(0, -px.y)).rgb, float3(8.0));
        float3 cS = min(hdr.sample(s, uv + float2(0, px.y)).rgb, float3(8.0));
        float3 cW = min(hdr.sample(s, uv + float2(-px.x, 0)).rgb, float3(8.0));
        float3 cE = min(hdr.sample(s, uv + float2(px.x, 0)).rgb, float3(8.0));
        float3 cM = min(h, float3(8.0));
        const float3 lw = float3(0.299, 0.587, 0.114);
        float lM = dot(cM, lw), lN = dot(cN, lw), lS = dot(cS, lw), lW = dot(cW, lw), lE = dot(cE, lw);
        lM = lM / (1.0 + lM); lN = lN / (1.0 + lN); lS = lS / (1.0 + lS); lW = lW / (1.0 + lW); lE = lE / (1.0 + lE);
        float lmax = max(max(lN, lS), max(max(lW, lE), lM)), lmin = min(min(lN, lS), min(min(lW, lE), lM));
        float rng = lmax - lmin;
        if (rng > max(0.03, lmax * 0.1)) {
            float2 g = float2(lE - lW, lS - lN);                 // gradient (across the edge)
            float2 t = normalize(float2(-g.y, g.x) + 1e-5);      // tangent (along the edge)
            float3 a = hdr.sample(s, uv + t * px * 0.5).rgb;
            float3 b = hdr.sample(s, uv - t * px * 0.5).rgb;
            float3 a2 = hdr.sample(s, uv + t * px * 1.5).rgb;
            float3 b2 = hdr.sample(s, uv - t * px * 1.5).rgb;
            float3 blur = (min(a, float3(8.0)) + min(b, float3(8.0))) * 0.3 + (min(a2, float3(8.0)) + min(b2, float3(8.0))) * 0.1;
            h = mix(h, blur, saturate(rng * 6.0));
        }
    }
    if (pf.x > 0.5) {
        float ao = aot.sample(s, uv).r;
        // Screen-space bounce: occluded pixels receive the blurred colour of what surrounds them.
        if (pf.w > 0.5) { h += min(bouncet.sample(s, uv).rgb, float3(3.0)) * (1.0 - ao) * 0.5; }
        h *= mix(1.0, ao, 0.85);
    }
    if (pf.y > 0.5) { h += bloomt.sample(s, uv).rgb * 0.06; }
    if (pf.z > 0.5) { return float4(float3(aot.sample(s, uv).r), 1.0); }
    float3 c = h * F.params.x;
    // warm white balance before the curve (sunlit, slightly golden look)
    c *= float3(1.10, 1.0, 0.86);
    c = filmic(lut, c);
    // gentle warm tint in the highlights
    float l = dot(c, float3(0.2126, 0.7152, 0.0722));
    c *= mix(float3(1.0), float3(1.03, 1.0, 0.95), smoothstep(0.35, 0.9, l));
    return float4(pow(saturate(c), float3(1.0 / 2.2)), 1.0);
}
