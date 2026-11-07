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
    float4x4 csm[6];
    float4 csm_info[6]; // x world texel size, y 1/zrange
    float4 csm_split;   // view-depth far of cascades 0-3
    float4 csm_far;     // x = far of terrain-only cascade 5, y = far of mid cascade 4
    float4 cr; float4 cu; float4 cf;  // camera basis; cr/cu pre-scaled by tan(fov/2)
    float4 params;      // x exposure, y shadows on, z 1/shadow size
    float4 pb0;         // probe volume: xyz world min corner, w bounce intensity (0 = no volume)
    float4 pb1;         // xyz 1/size
};
struct DrawU { float4x4 model; float4 tint; float4 emissive; float4 mat; float4 flags; }; // mat: rough, metal, cutoff, unlit; flags.x: global ocean

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
    int ci = vd < F.csm_far.y ? 4 : 5;
    for (int i = 0; i < 4; i++) { if (vd < F.csm_split[i]) { ci = i; break; } }
    if (F.params.w > 5.5 && F.params.w < 9.5) { ci = int(F.params.w + 0.5) - 6; }
    if (F.params.w > 15.5 && F.params.w < 17.5) { ci = F.params.w < 16.5 ? 4 : 5; }
    if (F.params.w >= 9.5 && F.params.w < 16.0) { int k = int(F.params.w + 0.5) - 10; if (k >= 0 && k <= 5) { ci = k; } }
    float4 inf = F.csm_info[ci];
    // Cascades >=1 render decimated proxies (grid 1.5-4 m), so receivers need a bias that covers that error.
    float nb = ci == 0 ? 2.0 : (ci == 1 ? 4.0 : (ci == 4 ? 2.0 : (ci == 5 ? 0.8 : 6.0)));
    float3 p = wpos + N * inf.x * nb;
    float4 c = F.csm[ci] * float4(p, 1.0);
    float2 uv = float2(c.x * 0.5 + 0.5, 0.5 - c.y * 0.5);
    float z = c.z - inf.y * inf.x * nb * 1.5;
    if (F.params.w > 9.5 && F.params.w < 15.5) { float st = sm.read(uint2(uv * 2048.0), ci); return saturate(0.5 + (z - st) / inf.y / 20.0); }
    float vis = 0.0;
    float2 o = float2(F.params.z);
    for (int y = -1; y <= 1; y++)
        for (int x = -1; x <= 1; x++)
            vis += sm.sample_compare(shs, uv + float2(x, y) * o, ci, z);
    vis /= 9.0;
    float fade = saturate((F.csm_far.x - vd) / (F.csm_far.x * 0.2));
    return mix(1.0, vis, fade);
}


constexpr sampler pbs(coord::normalized, filter::linear, address::clamp_to_edge);
// Probe GI (port of irradiance_volume.wesl): returns (sky visibility, bounce rgb) weights via out params.
float probe_gi(float3 wp, float3 N, constant FrameU& F, texture3d<float> pv, thread float& vis, thread float3& bounce) {
    vis = 1.0; bounce = float3(0.0);
    if (F.pb0.w <= 0.0) return 0.0;
    float3 u = (wp - F.pb0.xyz) * F.pb1.xyz - 0.5;
    float3 e = (0.5 - abs(u)) / float3(0.06, 0.02, 0.06);
    float w = saturate(min(e.x, min(e.y, e.z)));
    if (w <= 0.0) return 0.0;
    float3 dims = float3(pv.get_width(), pv.get_height(), pv.get_depth());
    float3 res = dims / float3(1.0, 2.0, 4.0);
    // Inverse of ed_world::probes::warp_axis: dense near the camera, sparse out to 1 km.
    float2 v = u.xz * 2.0;
    float2 gw = sign(v) * pow(abs(v), float2(2.0 / 3.0));
    float s = clamp((gw.x * 0.5 + 0.5) * res.x, 0.5, res.x - 0.5);
    float p = clamp((gw.y * 0.5 + 0.5) * res.z, 0.5, res.z - 0.5);
    float4 g = pv.sample(pbs, float3(s / dims.x, 0.5 / dims.y, (p + 3.0 * res.z) / dims.z), level(0));
    float t = clamp((u.y - g.r) / max(g.g, 1e-6), 0.0, 1.0);
    float layer = sqrt(t) * (res.y - 1.0) + 0.5;
    float uu = s / dims.x, vv = layer / dims.y;
    float3 neg = select(float3(0.0), float3(0.5), N < float3(0.0));
    float4 fx = pv.sample(pbs, float3(uu, vv + neg.x, p / dims.z), level(0));
    float4 fy = pv.sample(pbs, float3(uu, vv + neg.y, (p + res.z) / dims.z), level(0));
    float4 fz = pv.sample(pbs, float3(uu, vv + neg.z, (p + 2.0 * res.z) / dims.z), level(0));
    float3 NN = N * N;
    float4 f = fx * NN.x + fy * NN.y + fz * NN.z;
    vis = f.a; bounce = f.rgb * F.pb0.w;
    return w;
}

float3 shade_ex(VOut in, float4 base, bool front, constant FrameU& F, constant DrawU& D, depth2d_array<float> sm, texture3d<float> pv, thread float3& Nout, float rough_in, float ao_in) {
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
        float pvis; float3 pbn;
        float pw = probe_gi(in.wpos, N, F, pv, pvis, pbn);
        amb = mix(amb, amb * pvis + up * pbn, pw);
        float3 R = reflect(-V, N);
        float gloss = (1.0 - rough) * (1.0 - rough) * mix(0.35, 1.0, metal);
        float3 env = mix(amb, sky_rad(R, L, day), gloss);
        float3 Fenv = (F0 + (max(float3(1.0 - rough), F0) - F0) * pow(1.0 - ndv, 5.0)) * mix(0.3, 1.0, metal);
        float specocc = mix(1.0, saturate(R.y * 4.0 + 1.0), 0.8);
        float3 diffuse_indirect = diffc * amb * (1.0 - Fenv * 0.5) * ao_in;
        float3 ambient = diffuse_indirect + env * Fenv * specocc * gloss * ao_in;
        col = direct + ambient;
        if (F.params.w > 0.5) {
            int m = int(F.params.w + 0.5);
            if (m == 18) return float3(dot(diffuse_indirect, float3(0.2126, 0.7152, 0.0722)));
            if (m == 19) return float3(pw);
            if (m == 20) return float3(pvis);
            if (m == 21) return float3(dot(pbn, float3(0.2126, 0.7152, 0.0722)));
            if (m == 22 || m == 23) {
                float3 du = (in.wpos - F.pb0.xyz) * F.pb1.xyz - 0.5;
                float3 de = (0.5 - abs(du)) / float3(0.06, 0.02, 0.06);
                return m == 22 ? float3(saturate(min(de.x, de.z))) : float3(saturate(de.y));
            }
            if (m == 1) return N * 0.5 + 0.5;
            if (m == 2 || m >= 6) return float3(vis);
            if (m == 3) return ambient;
            if (m == 4) return float3(vd < F.csm_split[0] ? 1 : 0, vd >= F.csm_split[0] && vd < F.csm_split[1] ? 1 : 0, vd >= F.csm_split[1] && vd < F.csm_split[2] ? 1 : 0);
            return float3(ndl);
        }
    }
    col += D.emissive.rgb;
    // Aerial-perspective fog toward the sky colour in the view direction.
    float3 fd = -V; fd.y = max(fd.y, 0.10);
    float f = 1.0 - exp(-dist * F.fog.x);
    col = mix(col, sky_rad(normalize(fd), L, day, 0.0), saturate(f));
    return col;
}


float3 shade(VOut in, float4 base, bool front, constant FrameU& F, constant DrawU& D, depth2d_array<float> sm, texture3d<float> pv, thread float3& Nout) {
    return shade_ex(in, base, front, F, D, sm, pv, Nout, D.mat.x, 1.0);
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
    n = normalize(n); // baked detail normal delta disabled
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
    float amp = 0.0; // analytic fBm micro-relief disabled
    float amp2 = 0.0;
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
                       texture2d<float> tex2 [[texture(2)]], texture3d<float> pv [[texture(8)]]) {
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
    float3 col = shade(iv, base, front, F, D, sm, pv, N);
    GOut o; o.c = float4(col, 1.0); o.n = float4(N * 0.5 + 0.5, D.mat.x);
    return o;
}
struct BOut { float4 c [[color(0)]]; float4 n [[color(1)]]; };
fragment BOut world_fs_blend(VOut in [[stage_in]], bool front [[front_facing]],
                             constant FrameU& F [[buffer(1)]], constant DrawU& D [[buffer(2)]],
                             texture2d<float> tex [[texture(0)]], depth2d_array<float> sm [[texture(1)]], texture3d<float> pv [[texture(8)]]) {
    float4 base = tex.sample(ts, in.uv) * in.col;
    float3 N;
    float3 col = shade(in, base, front, F, D, sm, pv, N);
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
                         texture2d_array<float> gal [[texture(3)]], texture2d_array<float> gnr [[texture(4)]], texture2d<float> mtex [[texture(5)]], texture3d<float> pv [[texture(8)]]) {
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
        col = shade(iv, base, front, F, D, sm, pv, N);
        GOut o; o.c = float4(col, 1.0); o.n = float4(N * 0.5 + 0.5, D.mat.x); return o;
    }
    {
        n = normalize(n); // baked detail normal delta disabled
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
    nd = normalize(nd); // mid-scale normal perturbation disabled
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
    float3 wnrm = n;
    if (water_f > 0.01) {
        float tt = F.cam.w;
        float fadew = 1.0 - smoothstep(3000.0, 30000.0, dist);
        float2 w1 = mtex.sample(ts, p2 / 23.0 + tt * float2(0.031, 0.017)).rg - 0.5;
        float2 w2 = mtex.sample(ts, p2 / 7.3 - tt * float2(0.047, -0.029)).rg - 0.5;
        float2 w3 = mtex.sample(ts, p2 / 2.1 + tt * float2(-0.06, 0.05)).rg - 0.5;
        float2 g = (w1 * 0.9 + w2 * 0.6 + w3 * 0.35) * 0.8 * fadew;
        wnrm = normalize(n + float3(g.x, 0.0, g.y));
        nd = normalize(mix(nd, wnrm, water_f));
    }
    float land_f = 1.0 - water_f;
    rough = mix(rough, max(rough, 0.97), land_f * smoothstep(25.0, 500.0, dist));
    rough = mix(rough, max(rough, 0.8), land_f * (1.0 - wet));
    iv.nrm = nd;
    col = shade_ex(iv, float4(albedo, 1.0), front, F, D, sm, pv, N, clamp(rough, 0.04, 1.0), ao);
    if (water_f > 0.01) {
        float3 Vw = normalize(F.cam.xyz - in.wpos);
        float3 Rw = reflect(-Vw, wnrm);
        Rw.y = abs(Rw.y);
        float nv = saturate(dot(wnrm, Vw));
        float fr = 0.02 + 0.98 * pow(1.0 - nv, 5.0);
        float3 sk = sky_rad(Rw, F.sun.xyz, F.sun.w);
        col += sk * fr * water_f * 0.85;
        N = normalize(mix(N, wnrm, water_f));
    }
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
    if (F.fog.y < 0.5) {
        // FXAA-lite on compressed HDR luma: blend along the edge tangent only where contrast is high.
        float2 px = 1.0 / float2(hdr.get_width(), hdr.get_height());
        float3 cN = min(hdr.sample(s, uv + float2(0, -px.y), level(0)).rgb, float3(8.0));
        float3 cS = min(hdr.sample(s, uv + float2(0, px.y), level(0)).rgb, float3(8.0));
        float3 cW = min(hdr.sample(s, uv + float2(-px.x, 0), level(0)).rgb, float3(8.0));
        float3 cE = min(hdr.sample(s, uv + float2(px.x, 0), level(0)).rgb, float3(8.0));
        float3 cM = min(h, float3(8.0));
        const float3 lw = float3(0.299, 0.587, 0.114);
        float lM = dot(cM, lw), lN = dot(cN, lw), lS = dot(cS, lw), lW = dot(cW, lw), lE = dot(cE, lw);
        lM = lM / (1.0 + lM); lN = lN / (1.0 + lN); lS = lS / (1.0 + lS); lW = lW / (1.0 + lW); lE = lE / (1.0 + lE);
        float lmax = max(max(lN, lS), max(max(lW, lE), lM)), lmin = min(min(lN, lS), min(min(lW, lE), lM));
        float rng = lmax - lmin;
        if (rng > max(0.03, lmax * 0.1)) {
            float2 g = float2(lE - lW, lS - lN);                 // gradient (across the edge)
            float2 t = normalize(float2(-g.y, g.x) + 1e-5);      // tangent (along the edge)
            float3 a = hdr.sample(s, uv + t * px * 0.5, level(0)).rgb;
            float3 b = hdr.sample(s, uv - t * px * 0.5, level(0)).rgb;
            float3 a2 = hdr.sample(s, uv + t * px * 1.5, level(0)).rgb;
            float3 b2 = hdr.sample(s, uv - t * px * 1.5, level(0)).rgb;
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
    if (F.params.w > 17.5 && F.params.w < 18.5) c = float3(dot(c, float3(0.2126, 0.7152, 0.0722)));
    return float4(pow(saturate(c), float3(1.0 / 2.2)), 1.0);
}

// ---- Tree impostors: camera-facing quads textured from a hemi-octahedral atlas ----
// (ed_world::trees::impostor; port of ed_game/src/shaders/impostor.wesl). Far shadows are not ported yet.
struct ImpX { packed_float4 params; packed_float4 axis; packed_float2 tile; };
struct IOut {
    float4 pos [[position]];
    float2 uv;
    float3 wpos;
    float4 tint [[flat]];
    uint layer [[flat]];
    float3 ax [[flat]];
    float3 up [[flat]];
    float3 br [[flat]];
    float3 bu [[flat]];
    float3 vl [[flat]];
};
constant float IMP_FRAMES = 8.0;
constant float IMP_PAD = 1.0666667;
constant float IMP_FULL_M = 1000.0;
constant float IMP_GROW_MAX = 6.0;
constant float IMP_MIN_PX = 2.5;

float2 imp_oct_encode(float3 d) {
    float3 dd = float3(d.x, max(d.y, 0.0), d.z);
    float3 n = dd / max(abs(dd.x) + abs(dd.y) + abs(dd.z), 1e-6);
    return float2(n.x + n.z, n.x - n.z) * 0.5 + 0.5;
}
float3 imp_oct_decode(float2 uv) {
    float2 g = uv * 2.0 - 1.0;
    float x = (g.x + g.y) * 0.5, z = (g.x - g.y) * 0.5;
    return normalize(float3(x, 1.0 - abs(x) - abs(z), z));
}
float3 imp_bu(float3 v) {
    float3 u0 = float3(0.0, 1.0, 0.0) - v * v.y;
    return dot(u0, u0) > 1e-6 ? normalize(u0) : float3(0.0, 0.0, 1.0);
}

vertex IOut impostor_vs(uint vid [[vertex_id]], device const Vtx* verts [[buffer(0)]], constant FrameU& F [[buffer(1)]],
                        constant DrawU& D [[buffer(2)]], device const ImpX* ex [[buffer(3)]]) {
    Vtx v = verts[vid];
    ImpX x = ex[vid];
    float3 c = (D.model * float4(float3(v.p), 1.0)).xyz;
    float3 up = normalize((D.model * float4(float3(v.n), 0.0)).xyz);
    float3 ax = (D.model * float4(float3(x.axis.xyz), 0.0)).xyz;
    ax = normalize(ax - up * dot(ax, up));
    float3 az = cross(ax, up);
    float3 to_cam = F.cam.xyz - c;
    float d = max(length(to_cam), 1e-3);
    float3 vw = to_cam / d;
    float3 vl = float3(dot(vw, ax), max(dot(vw, up), 0.0), dot(vw, az));
    vl = normalize(vl + float3(0.0, 1e-4, 0.0));
    float3 bu = imp_bu(vl);
    float3 br = normalize(cross(bu, vl));
    float3 right_w = ax * br.x + up * br.y + az * br.z;
    float3 up_w = ax * bu.x + up * bu.y + az * bu.z;

    float radius = x.params.x, kept = max(x.params[2], 1e-6), nearc = x.params[3];
    float f = min(1.0, pow(IMP_FULL_M / d, 2.0));
    float keep_here = f / kept;
    float s = clamp((keep_here - x.axis[3]) / max(0.4 * keep_here, 0.004), 0.0, 1.0);
    float grow = pow(min(rsqrt(max(min(f, kept), 1e-6)), IMP_GROW_MAX), 0.75);
    if (nearc > 0.0) {
        grow = mix(1.0, grow, smoothstep(nearc + 300.0, nearc + 1500.0, d));
        s *= smoothstep(nearc - 20.0, nearc + 20.0, d);
    }
    float r = radius * grow;
    float px = r * F.fog.z / d; // fog.z = viewport height / tan(fov/2)
    s *= smoothstep(IMP_MIN_PX * 0.8, IMP_MIN_PX * 1.3, px);
    float tie = fract(sin(dot(c.xz * 0.01, float2(12.9898, 78.233))) * 43758.5453);
    float2 corner = float2(v.uv);
    float3 off = (right_w * corner.x + up_w * corner.y) * (r * IMP_PAD * s) + vw * (r * (0.2 + 0.4 * tie) * s);
    float3 wp = c + off;
    IOut o;
    o.pos = F.vp * float4(wp, 1.0);
    o.wpos = wp;
    o.uv = float2(corner.x * 0.5 + 0.5, 0.5 - corner.y * 0.5);
    o.tint = float4(v.c);
    o.layer = uint(x.params[1] + 0.5);
    o.ax = ax; o.up = up; o.br = br; o.bu = bu; o.vl = vl;
    return o;
}

fragment GOut impostor_fs(IOut in [[stage_in]], bool front [[front_facing]],
                          constant FrameU& F [[buffer(1)]], constant DrawU& D [[buffer(2)]],
                          depth2d_array<float> sm [[texture(1)]],
                          texture2d_array<float> albedo_t [[texture(6)]], texture2d_array<float> normal_t [[texture(7)]], texture3d<float> pv [[texture(8)]]) {
    float2 c = float2(in.uv.x * 2.0 - 1.0, 1.0 - in.uv.y * 2.0);
    float3 p = in.br * c.x + in.bu * c.y;
    float2 g = imp_oct_encode(in.vl) * IMP_FRAMES - 0.5;
    float2 g0 = floor(g);
    float2 t = g - g0;
    float3 acc = 0.0, nacc = 0.0;
    float asum = 0.0, wsum = 0.0;
    for (uint k = 0; k < 4; k++) {
        float2 o = float2(float(k & 1u), float(k >> 1));
        float w = mix(1.0 - t.x, t.x, o.x) * mix(1.0 - t.y, t.y, o.y);
        float2 fr = clamp(g0 + o, 0.0, IMP_FRAMES - 1.0);
        float3 dk = imp_oct_decode((fr + 0.5) / IMP_FRAMES);
        float3 buk = imp_bu(dk);
        float3 brk = normalize(cross(buk, dk));
        float2 q = float2(dot(p, brk), dot(p, buk));
        float inside = step(abs(q.x), 1.0) * step(abs(q.y), 1.0);
        float2 uvk = clamp(float2(q.x * 0.5 + 0.5, 0.5 - q.y * 0.5), 0.015, 0.985);
        float2 auv = (fr + uvk) / IMP_FRAMES;
        float4 a = albedo_t.sample(tsc, auv, in.layer);
        float3 n = normal_t.sample(tsc, auv, in.layer).xyz * 2.0 - 1.0;
        float wa = w * a.a * inside;
        acc += a.rgb * wa; nacc += n * wa; asum += wa; wsum += w;
    }
    float cov = asum / max(wsum, 1e-5);
    if (cov < 0.5) discard_fragment();
    float3 alb = acc / max(asum, 1e-5);
    float3 nt = nacc / max(asum, 1e-5);
    float3 az = cross(in.ax, in.up);
    float3 nrm = normalize(in.ax * nt.x + in.up * nt.y + az * nt.z + float3(0.0, 1e-5, 0.0));
    float dist = length(F.cam.xyz - in.wpos);
    // Far crowns take the satellite canopy colour (keeping the impostor's shading) and a smoother normal.
    float shade = clamp(dot(alb, float3(0.3, 0.6, 0.1)) / 0.08, 0.85, 1.15);
    float wt = in.tint.a * smoothstep(400.0, 2500.0, dist);
    alb = mix(alb, in.tint.rgb * shade, wt);
    nrm = normalize(mix(nrm, in.up, smoothstep(500.0, 2500.0, dist) * 0.85));
    VOut iv; iv.pos = in.pos; iv.nrm = nrm; iv.uv = in.uv; iv.col = float4(1.0); iv.wpos = in.wpos;
    float3 N;
    float3 col = shade_ex(iv, float4(alb, 1.0), true, F, D, sm, pv, N, 0.95, 1.0);
    GOut out; out.c = float4(col, 1.0); out.n = float4(N * 0.5 + 0.5, 0.95);
    return out;
}

// ---- Water surface (stage 2 of docs/plans/ultra-water-port.md) ----
// Analytic multi-wave normals with slope-variance roughness (waves finer than a pixel turn into roughness,
// not aliasing), Lagarde Fresnel, GGX sun, Beer-Lambert absorption through the real column of water
// (scene depth behind the surface), SSS, glitter and a soft shoreline. Sampled copies of scene depth/colour
// come from a blit made between the opaque and water passes.
constant float2 WDIR[8] = { float2(0.80, 0.60), float2(0.62, 0.78), float2(0.95, 0.31), float2(0.40, 0.92), float2(0.99, -0.10), float2(0.70, 0.71), float2(0.88, 0.47), float2(0.30, -0.95) };
constant float WLEN[8] = { 62.0, 33.0, 17.5, 9.6, 5.3, 2.8, 1.45, 0.78 };
constant float WSTEEP[8] = { 0.050, 0.055, 0.060, 0.060, 0.055, 0.050, 0.045, 0.040 };

float3 wave_normal(float2 p, float t, float pix, thread float& svar) {
    float2 g = float2(0.0);
    svar = 0.0003;
    for (int i = 0; i < 8; i++) {
        float k = 2.0 * M_PI_F / WLEN[i];
        float w = sqrt(9.81 * k);
        float fade = 1.0 - smoothstep(0.35, 1.2, pix * k * 0.5);   // wave narrower than ~2 px -> variance
        float s = WSTEEP[i];
        float ph = dot(WDIR[i], p) * k - w * t + float(i) * 1.7;
        g += WDIR[i] * (cos(ph) * s * fade);
        svar += 0.5 * s * s * (1.0 - fade);
    }
    return normalize(float3(-g.x, 1.0, -g.y));
}

// FFT ocean (shaders/ocean.metal): 3 cascades of slope(x,z), Jacobian, height in a tiled texture array.
// OV = per-cascade slope variance (xyz) + FFT-on flag (w). Mip-filtered slopes lose energy as the footprint
// outgrows a cascade, so that energy is re-added as roughness variance.
constant float OLEN[3] = { 600.0, 85.0, 12.0 };
// The wave detail is only worth reading close to the camera; past that the sea is a smooth sheet.
// The slopes are faded out over a wide band, so the water drifts to a mirror instead of ending
// on a visible line where the waves stop.
constant float WAVE_NEAR_M = 100.0;
constant float WAVE_FAR_M = 400.0;

float3 fft_normal(texture2d_array<float> oc, float2 p, float pix, float4 OV, thread float& svar, thread float& jac) {
    constexpr sampler os(filter::linear, mip_filter::linear, address::repeat, max_anisotropy(4));
    float2 g = float2(0.0);
    svar = 0.0003;
    jac = 1.0;
    for (int c = 0; c < 3; c++) {
        float4 v = oc.sample(os, p / OLEN[c], uint(c));
        g += v.xy;
        jac += v.z - 1.0;
        svar += OV[c] * smoothstep(0.4, 2.0, pix / (OLEN[c] / 128.0));
    }
    return normalize(float3(-g.x, 1.0, -g.y));
}

struct WOut { float4 c [[color(0)]]; float4 n [[color(1)]]; };
fragment WOut water_fs(VOut in [[stage_in]], constant FrameU& F [[buffer(1)]], constant DrawU& D [[buffer(2)]],
                       depth2d_array<float> sm [[texture(1)]], texture3d<float> pv [[texture(8)]],
                       depth2d<float> dcopy [[texture(9)]], texture2d<float> hcopy [[texture(10)]],
                       texture2d_array<float> oc [[texture(11)]], constant float4& OV [[buffer(3)]]) {
    float3 toCam = F.cam.xyz - in.wpos;
    float dist = max(length(toCam), 1e-3);
    float3 V = toCam / dist;
    float3 L = F.sun.xyz;
    float day = F.sun.w;
    float t = F.cam.w;
    float pix = 2.0 * dist / max(F.fog.z, 1.0);   // metres per pixel (fog.z = viewport height / tan(fov/2))
    float svar;
    float jac = 1.0;
    // Waves only matter within a couple of hundred metres; past that the sea is a smooth sheet.
    // The band is wide, so the water drifts to a mirror rather than ending on a visible line.
    float nf = 1.0 - smoothstep(WAVE_NEAR_M, WAVE_FAR_M, dist);
    float3 N = OV.w > 0.5 ? fft_normal(oc, in.wpos.xz, pix, OV, svar, jac) : wave_normal(in.wpos.xz, t, pix, svar);
    N = normalize(mix(N, float3(0, 1, 0), nf));
    N = normalize(mix(N, float3(0, 1, 0), smoothstep(8000.0, 40000.0, dist)));
    float ndv = max(dot(N, V), 0.02);
    // Water column behind this fragment.
    float near = F.csm_far.z;
    uint2 px = uint2(in.pos.xy);
    float dbg = dcopy.read(px);
    float zw = near / max(in.pos.z, 1e-9);
    float cosf = max(dot(-V, F.cf.xyz), 0.05);
    bool has_bg = dbg > 1e-7;
    float thick = has_bg ? clamp((near / dbg - zw) / cosf, 0.0, 400.0) : 400.0;
    float hdep = thick * abs(V.y);
    // Water within a few cm of the ground is coplanar with it: depth precision makes the two z-fight, and foam
    // keyed on that noisy depth speckles inland. Drop those fragments (the beach shows through) and fade
    // everything shore-related from a distance-scaled threshold instead of from zero.
    float thr = 0.02 + dist * 0.0005;
    if (has_bg && hdep < thr) discard_fragment();
    float shore = smoothstep(thr, thr + 0.25, hdep);
    // Refraction: shift the lookup by the wave normal, scaled by column thickness, and only accept it when the shifted
    // texel is really behind the surface (otherwise a foreground object would smear into the water).
    float3 bg;
    if (has_bg) {
        float2 off = N.xz * clamp(thick * 0.6, 0.0, 14.0) * 1.0 / max(1.0, dist * 0.02);
        uint2 px2 = uint2(clamp(float2(px) + off, float2(0.0), float2(dcopy.get_width() - 1, dcopy.get_height() - 1)));
        bool ok = dcopy.read(px2) < in.pos.z * 0.999 && dcopy.read(px2) > 1e-7;
        bg = hcopy.read(ok ? px2 : px).rgb;
    } else {
        bg = sky_rad(refract(-V, N, 0.75), L, day, 0.0) * 0.3;
    }
    // Caustics on the bed: two crossed ridged-noise layers, lit by the sun after it has crossed the water column.
    if (has_bg && hdep < 10.0 && day > 0.01) {
        float2 cp = (in.wpos.xz - V.xz / max(V.y, 0.1) * thick * abs(V.y)) * 0.8;
        float c1 = 1.0 - abs(vnoise(cp + t * 0.35) * 2.0 - 1.0);
        float c2 = 1.0 - abs(vnoise(cp * 1.7 - t * 0.28 + 9.0) * 2.0 - 1.0);
        float cau = pow(c1 * c2, 6.0) * 5.0 * (1.0 - smoothstep(2.0, 10.0, hdep)) * smoothstep(0.0, 0.5, hdep);
        bg *= 1.0 + cau * saturate(L.y * 2.0) * day;
    }
    // Beer-Lambert: red goes first, shallow sand reads turquoise, deep water goes blue-green.
    float3 absorb = float3(0.40, 0.062, 0.032);
    // The open sea is deep even where the elevation data has no bathymetry: the sea floor there
    // is a flat plane a metre or two down and is shaded like a lake bed, so without this the
    // floor shows through the water as pale patches. The shore logic still uses the real depth.
    float dcol = D.flags.x > 0.5 ? max(thick, 35.0) : thick;
    float3 T = exp(-absorb * dcol);
    float lightk = day * (0.30 + 0.70 * saturate(L.y));
    float3 body = float3(0.030, 0.150, 0.210) * lightk * (1.0 - exp(-dcol * 0.30));
    float3 under = bg * T + body;
    // Reflection (analytic sky + sun); Lagarde roughness-aware Fresnel with F0 = 0.02.
    float a = clamp(sqrt(svar), 0.02, 0.7);
    float3 R = reflect(-V, N); R.y = abs(R.y);
    float fr = 0.02 + (max(1.0 - a, 0.02) - 0.02) * pow(1.0 - ndv, 5.0);
    float3 refl = sky_rad(R, L, day, 0.0);
    float3 H = normalize(L + V);
    float ndl = saturate(dot(N, L)), ndh = saturate(dot(N, H));
    float vd = dot(in.wpos - F.cam.xyz, F.cf.xyz);
    float vis = ndl > 0.0 ? shadow_vis(in.wpos, float3(0, 1, 0), vd, F, sm) : 0.0;
    float a2 = a * a, dd = ndh * ndh * (a2 - 1.0) + 1.0;
    float Dg = a2 / (M_PI_F * dd * dd);
    float3 sunc = mix(float3(1.0, 0.62, 0.34), float3(1.0, 0.96, 0.88), saturate(L.y * 2.5)) * 6.4 * day;
    // The sun on a wave field is a sparkle, not a sheet: break the highlight up with a noise
    // whose features stay a few pixels wide. Close up the sparkle is what keeps the highlight
    // from turning into flat pale patches; far away the features fall under a pixel, so the
    // sparkle has to go rather than alias.
    float glit = mix(1.0, 0.75 + 0.5 * vnoise(in.wpos.xz * 3.1 + t * 0.7), 1.0 - smoothstep(0.12, 0.4, pix));
    float3 spec = sunc * min(Dg * fr / max(4.0 * ndv, 0.05), 45.0) * ndl * vis * glit;
    // Sub-surface glow through thin crests, strongest looking toward the sun.
    float sss = pow(saturate(dot(-V, L) * 0.5 + 0.5), 4.0) * pow(1.0 - ndv, 2.0) * lightk;
    float3 glow = float3(0.04, 0.22, 0.16) * sss * 0.6 * saturate(1.0 - exp(-thick));
    // Soft shoreline: the surface dissolves into the bed instead of cutting a hard line; lace foam on the wet edge.
    float edge = smoothstep(thr, thr + 0.35, hdep);
    float foam = 0.0;
    // Breaker bands: shoreward-travelling bands keyed on the real depth below the surface; they steepen and break as
    // the water shoals (hdep is the carved terrain depth, standing in for the Ultra-Water shore map).
    if (has_bg && hdep < 3.5) {
        float shoal = 1.0 - smoothstep(0.4, 3.5, hdep);
        float bph = hdep * 2.4 + t * 0.9 + vnoise(in.wpos.xz * 0.35) * 2.5;
        float band = smoothstep(0.55, 0.95, 0.5 + 0.5 * sin(bph * 2.0));
        float swash = smoothstep(0.0, 0.5, hdep) * (1.0 - smoothstep(0.5, 1.6, hdep));
        foam = max(foam, band * shoal * (0.35 + swash * 0.65) * smoothstep(0.05, 0.4, hdep));
    }
    if (hdep < 1.2 && has_bg) {
        float lace = fbm3(in.wpos.xz * 1.7 + float2(t * 0.15, -t * 0.1)) * 0.6 + fbm3(in.wpos.xz * 5.0 - t * 0.2) * 0.4;
        foam = max(foam, (1.0 - smoothstep(0.0, 1.1, hdep)) * smoothstep(0.30, 0.65, lace + (1.0 - smoothstep(0.0, 0.5, hdep)) * 0.45));
    }
    // Whitecaps: the Jacobian marks the sharp crests. Only the crests where the surface actually
    // folds get foam, otherwise whole patches of the sea turn pale. The foam lives inside the
    // wave range, so it and the waves end together and the sea has one soft transition.
    foam = max(foam, smoothstep(0.02, -0.25, jac) * nf * 0.5);
    foam *= shore;
    float3 col = under * (1.0 - fr * edge) + (refl * fr + spec) * edge + glow;
    col = mix(col, float3(0.9, 0.93, 0.95) * lightk * 0.9 + sunc * 0.05 * vis, saturate(foam) * 0.85);
    float3 fd = -V; fd.y = max(fd.y, 0.10);
    float f = 1.0 - exp(-dist * F.fog.x);
    col = mix(col, sky_rad(normalize(fd), L, day, 0.0), saturate(f));
    WOut o; o.c = float4(col, 1.0); o.n = float4(N * 0.5 + 0.5, 0.04);
    return o;
}
