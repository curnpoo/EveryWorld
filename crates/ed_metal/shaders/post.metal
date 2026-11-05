// Screen-space post chain: GTAO, SSR, bloom, auto-exposure, tonemap, FXAA.
// Conventions: reverse-Z infinite far plane, depth = near / view_z (1 at near, 0 = sky).
// View space: right-handed, +X right, +Y up, camera looks down -Z. Normals in the G-buffer are WORLD space.
#include <metal_stdlib>
using namespace metal;

struct PostU {
    float4x4 inv_proj;
    float4x4 view;      // world -> view
    float4x4 view_inv;  // view -> world
    float4 size;        // w, h, 1/w, 1/h (full resolution)
    float4 proj;        // near, tan(fovy/2), aspect, 0
    float4 ao;          // radius_m, intensity, power, slices
    float4 ao2;         // steps, 0, 0, 0
    float4 ssr;         // max_dist_m, thickness_m, steps, intensity
    float4 ssr2;        // f0, roughness_max, 0, 0
    float4 bloom;       // threshold, knee, intensity, 0
    float4 fin;         // exposure_comp (stops), vignette, dither, tonemap id
    float4 fin2;        // auto_exposure (0/1), dt, exposure_min, exposure_max
    float4 fin3;        // ae_key, fxaa_subpix, fxaa_edge_thr, fxaa_edge_thr_min
    uint4 ints;         // frame, 0, 0, 0
};

constant float PI = 3.14159265359;
constexpr sampler smp(filter::linear, address::clamp_to_edge, mip_filter::linear);

inline float ign(float2 p, uint frame) {
    p += 5.588238 * float(frame & 63u);
    return fract(52.9829189 * fract(dot(p, float2(0.06711056, 0.00583715))));
}

inline float3 vpos(constant PostU& u, float2 uv, float d) {
    float2 ndc = float2(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    return float3(ndc.x * u.proj.y * u.proj.z * d, ndc.y * u.proj.y * d, -d);
}
inline float2 proj_uv(constant PostU& u, float3 p) {
    float d = -p.z;
    float2 ndc = float2(p.x / (d * u.proj.y * u.proj.z), p.y / (d * u.proj.y));
    return float2(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
}
inline float3 world_to_view_n(constant PostU& u, float3 nw) {
    return normalize((u.view * float4(nw, 0.0)).xyz);
}

// ---------------------------------------------------------------- linear depth (full res, R32Float)
kernel void post_lin_depth(depth2d<float, access::read> dep [[texture(0)]],
                           texture2d<float, access::write> dst [[texture(1)]],
                           constant PostU& u [[buffer(0)]], uint2 gid [[thread_position_in_grid]]) {
    if (gid.x >= dst.get_width() || gid.y >= dst.get_height()) return;
    float z = dep.read(gid);
    dst.write(float4(z > 1e-9 ? u.proj.x / z : 1e5, 0, 0, 0), gid);
}

// ---------------------------------------------------------------- GTAO (half res, R8Unorm)
kernel void post_gtao(texture2d<float, access::read> lin [[texture(0)]],
                      texture2d<float, access::read> nr [[texture(1)]],
                      texture2d<float, access::write> dst [[texture(2)]],
                      constant PostU& u [[buffer(0)]], uint2 gid [[thread_position_in_grid]]) {
    if (gid.x >= dst.get_width() || gid.y >= dst.get_height()) return;
    float2 fsz = u.size.xy;
    uint2 fi = min(gid * 2u, uint2(fsz) - 1u);
    float d = lin.read(fi).x;
    if (d >= 1e4) { dst.write(float4(1, 0, 0, 1), gid); return; }
    float2 fp = float2(fi);
    float2 uv = (fp + 0.5) / fsz;
    float3 P = vpos(u, uv, d);
    float3 N = world_to_view_n(u, nr.read(fi).xyz * 2.0 - 1.0);
    float3 V = normalize(-P);
    float R = u.ao.x * clamp(d / 40.0, 1.0, 10.0);  // grow the radius with distance so AO stays visible from the air
    float radius_px = min(R * (fsz.y * 0.5) / (u.proj.y * d), 0.3 * fsz.y);
    if (radius_px < 1.5) { dst.write(float4(1, 0, 0, 1), gid); return; }

    int slices = max(int(u.ao.w), 1);
    int steps = max(int(u.ao2.x), 2);
    uint fr = u.ints.x;
    float n1 = ign(fp, fr);
    float n2 = ign(fp + float2(47.0, 17.0), fr);
    float fall = 0.6 * R;
    float vis = 0.0;
    for (int i = 0; i < slices; i++) {
        float phi = (float(i) + n1) / float(slices) * PI;
        float cp = cos(phi), sp = sin(phi);
        float2 om = float2(cp, -sp);
        float3 dirV = float3(cp, sp, 0.0);
        float3 axis = normalize(cross(dirV, V));
        float3 orthoDir = dirV - dot(dirV, V) * V;
        float3 projN = N - axis * dot(N, axis);
        float projLen = length(projN);
        float signN = sign(dot(orthoDir, projN));
        float cosN = saturate(dot(projN, V) / max(projLen, 1e-5));
        float nn = signN * acos(cosN);
        float low0 = cos(nn + PI * 0.5), low1 = cos(nn - PI * 0.5);
        float H0 = low0, H1 = low1;
        for (int s = 0; s < steps; s++) {
            float st = (float(s) + n2) / float(steps);
            st *= st;
            float2 so = om * max(st * radius_px, float(s) + 1.0);
            float2 o = round(so);
            float2 c0 = fp + o, c1 = fp - o;
            if (all(c0 >= 0.0) && all(c0 < fsz)) {
                float d0 = lin.read(uint2(c0)).x;
                float3 dl = vpos(u, (c0 + 0.5) / fsz, d0) - P;
                float dist = max(length(dl), 1e-4);
                float shc = dot(dl / dist, V);
                shc = mix(low0, shc, saturate((R - dist) / fall));
                H0 = max(H0, shc);
            }
            if (all(c1 >= 0.0) && all(c1 < fsz)) {
                float d1 = lin.read(uint2(c1)).x;
                float3 dl = vpos(u, (c1 + 0.5) / fsz, d1) - P;
                float dist = max(length(dl), 1e-4);
                float shc = dot(dl / dist, V);
                shc = mix(low1, shc, saturate((R - dist) / fall));
                H1 = max(H1, shc);
            }
        }
        float h0 = -acos(clamp(H1, -1.0, 1.0));
        float h1 = acos(clamp(H0, -1.0, 1.0));
        h0 = nn + clamp(h0 - nn, -PI * 0.5, PI * 0.5);
        h1 = nn + clamp(h1 - nn, -PI * 0.5, PI * 0.5);
        float sn = sin(nn);
        float ia0 = (cosN + 2.0 * h0 * sn - cos(2.0 * h0 - nn)) * 0.25;
        float ia1 = (cosN + 2.0 * h1 * sn - cos(2.0 * h1 - nn)) * 0.25;
        vis += projLen * (ia0 + ia1);
    }
    vis = saturate(vis / float(slices));
    vis = pow(vis, u.ao.z);
    vis = mix(1.0, vis, u.ao.y);
    dst.write(float4(vis, 0, 0, 1), gid);
}

// Depth-aware separable blur at half res. dir = (1,0) or (0,1).
kernel void post_ao_blur(texture2d<float, access::read> src [[texture(0)]],
                         texture2d<float, access::read> lin [[texture(1)]],
                         texture2d<float, access::write> dst [[texture(2)]],
                         constant PostU& u [[buffer(0)]], constant int2& dir [[buffer(1)]],
                         uint2 gid [[thread_position_in_grid]]) {
    int2 sz = int2(dst.get_width(), dst.get_height());
    if (int(gid.x) >= sz.x || int(gid.y) >= sz.y) return;
    int2 fmax = int2(u.size.xy) - 1;
    float dc = lin.read(uint2(min(int2(gid) * 2, fmax))).x;
    float acc = 0.0, wsum = 0.0;
    for (int k = -3; k <= 3; k++) {
        int2 p = clamp(int2(gid) + dir * k, int2(0), sz - 1);
        float dn = lin.read(uint2(min(p * 2, fmax))).x;
        float wd = saturate(1.0 - abs(dn - dc) / (0.04 * dc + 0.02));
        float w = exp(-0.5 * float(k * k) / 2.5) * wd;
        acc += src.read(uint2(p)).x * w;
        wsum += w;
    }
    dst.write(float4(acc / max(wsum, 1e-5), 0, 0, 1), gid);
}

// Depth-aware bilateral upsample half -> full res (R8Unorm).
kernel void post_ao_upsample(texture2d<float, access::read> src [[texture(0)]],
                             texture2d<float, access::read> lin [[texture(1)]],
                             texture2d<float, access::write> dst [[texture(2)]],
                             constant PostU& u [[buffer(0)]], uint2 gid [[thread_position_in_grid]]) {
    if (gid.x >= dst.get_width() || gid.y >= dst.get_height()) return;
    int2 hs = int2(src.get_width(), src.get_height());
    int2 fmax = int2(u.size.xy) - 1;
    float dc = lin.read(gid).x;
    float2 h = (float2(gid) + 0.5) * 0.5 - 0.5;
    int2 b = int2(floor(h));
    float2 f = h - float2(b);
    float acc = 0.0, wsum = 0.0, nacc = 0.0;
    for (int j = 0; j < 2; j++) for (int i = 0; i < 2; i++) {
        int2 p = clamp(b + int2(i, j), int2(0), hs - 1);
        float wb = (i == 0 ? 1.0 - f.x : f.x) * (j == 0 ? 1.0 - f.y : f.y);
        float dn = lin.read(uint2(min(p * 2, fmax))).x;
        float wd = saturate(1.0 - abs(dn - dc) / (0.03 * dc + 0.01));
        float v = src.read(uint2(p)).x;
        acc += v * wb * wd; wsum += wb * wd;
        nacc += v * wb;
    }
    float r = wsum > 1e-3 ? acc / wsum : nacc;
    dst.write(float4(r, 0, 0, 1), gid);
}

// ---------------------------------------------------------------- SSR (half res, RGBA16F; rgb radiance, a confidence)
kernel void post_ssr_trace(texture2d<float> hdr [[texture(0)]],
                           depth2d<float, access::read> dep [[texture(1)]],
                           texture2d<float, access::read> nr [[texture(2)]],
                           texture2d<float, access::write> dst [[texture(3)]],
                           constant PostU& u [[buffer(0)]], uint2 gid [[thread_position_in_grid]]) {
    if (gid.x >= dst.get_width() || gid.y >= dst.get_height()) return;
    float2 fsz = u.size.xy;
    uint2 ip = min(gid * 2u, uint2(fsz) - 1u);
    float4 nrv = nr.read(ip);
    float rough = nrv.w;
    float z0 = dep.read(ip);
    float4 res = float4(0);
    if (z0 > 1e-9 && rough < u.ssr2.y) {
        float near = u.proj.x;
        float d = near / z0;
        float2 uv0 = (float2(ip) + 0.5) / fsz;
        float3 P = vpos(u, uv0, d);
        float3 N = world_to_view_n(u, nrv.xyz * 2.0 - 1.0);
        float3 I = normalize(P);
        float3 Rv = reflect(I, N);
        float camfade = saturate((0.25 - Rv.z) / 0.5);   // rays heading back to the camera fade out
        float tmax = u.ssr.x;
        if (Rv.z > 1e-4) tmax = min(tmax, (d - 0.1) / Rv.z);
        float3 P1 = P + Rv * tmax;
        float2 uv1 = proj_uv(u, P1);
        float zr1 = near / max(-P1.z, 1e-3);
        float2 duv = uv1 - uv0;
        float len = max(abs(duv.x), abs(duv.y)) * max(fsz.x, fsz.y);
        float len2 = max(dot(duv, duv), 1e-12);
        if (len >= 2.0 && camfade > 0.0 && tmax > 0.01) {
            int n = clamp(int(len / 2.0), 1, int(u.ssr.z));
            float jit = ign(float2(ip), u.ints.x);
            float thick = u.ssr.y;
            float prevf = 0.0;
            bool hit = false;
            float hitf = 0.0;
            for (int i = 0; i < n; i++) {
                float f = (float(i) + 0.25 + 0.75 * jit) / float(n);
                float2 uvf = uv0 + duv * f;
                if (any(uvf < 0.0) || any(uvf > 1.0)) break;
                uint2 pp = uint2(min(uvf * fsz, fsz - 1.0));
                float zs = dep.read(pp);
                if (zs > 1e-9) {
                    float2 uvc = (float2(pp) + 0.5) / fsz;
                    float fc = dot(uvc - uv0, duv) / len2;      // snap ray to pixel centre (exact on planes)
                    float zr = mix(z0, zr1, fc);
                    float dr = near / max(zr, 1e-9), ds = near / zs;
                    float diff = dr - ds;
                    if (diff > 0.01 + 0.003 * d && diff < thick) {
                        // refine
                        float lo = prevf, hi = f;
                        for (int k = 0; k < 6; k++) {
                            float m = 0.5 * (lo + hi);
                            float2 um = uv0 + duv * m;
                            uint2 pm = uint2(clamp(um * fsz, 0.0, fsz - 1.0));
                            float zsm = dep.read(pm);
                            float2 ucm = (float2(pm) + 0.5) / fsz;
                            float zrm = mix(z0, zr1, dot(ucm - uv0, duv) / len2);
                            float dm = near / max(zrm, 1e-9) - (zsm > 1e-9 ? near / zsm : 1e5);
                            if (dm > 0.01 + 0.003 * d) hi = m; else lo = m;
                        }
                        hit = true; hitf = hi;
                        break;
                    }
                }
                prevf = f;
            }
            if (hit) {
                float2 huv = uv0 + duv * hitf;
                uint2 hp = uint2(clamp(huv * fsz, 0.0, fsz - 1.0));
                float3 Nh = world_to_view_n(u, nr.read(hp).xyz * 2.0 - 1.0);
                float bf = saturate(1.0 - max(dot(Nh, Rv), 0.0) * 4.0);
                float2 e2 = min(huv, 1.0 - huv);
                float edge = saturate(min(e2.x, e2.y) / 0.08);
                float dfade = 1.0 - smoothstep(0.7, 1.0, hitf);
                float rfade = 1.0 - smoothstep(0.3, u.ssr2.y, rough);
                float maxlod = min(float(hdr.get_num_mip_levels() - 1), 6.0);
                float lod = clamp(log2(1.0 + rough * len * hitf * 0.7) + rough * 1.5, 0.0, maxlod);
                float3 col = hdr.sample(smp, huv, level(lod)).rgb;
                col = min(col, float3(64.0));
                res = float4(col, bf * edge * dfade * rfade * camfade);
            }
        }
    }
    dst.write(res, gid);
}

// Adds fresnel-weighted reflections into the HDR target (read_write). Skips sky and rough pixels.
kernel void post_ssr_composite(texture2d<float, access::read_write> hdr [[texture(0)]],
                               texture2d<float, access::read> nr [[texture(1)]],
                               depth2d<float, access::read> dep [[texture(2)]],
                               texture2d<float, access::read> ssr [[texture(3)]],
                               constant PostU& u [[buffer(0)]], uint2 gid [[thread_position_in_grid]]) {
    if (gid.x >= hdr.get_width() || gid.y >= hdr.get_height()) return;
    float4 nrv = nr.read(gid);
    float rough = nrv.w;
    if (rough >= u.ssr2.y || dep.read(gid) <= 1e-9) return;
    int2 hs = int2(ssr.get_width(), ssr.get_height());
    float2 h = (float2(gid) + 0.5) * 0.5 - 0.5;
    int2 b = int2(floor(h));
    float2 f = h - float2(b);
    float3 acc = 0; float wa = 0, wsum = 0;
    for (int j = 0; j < 2; j++) for (int i = 0; i < 2; i++) {
        int2 p = clamp(b + int2(i, j), int2(0), hs - 1);
        float wb = (i == 0 ? 1.0 - f.x : f.x) * (j == 0 ? 1.0 - f.y : f.y);
        float4 s = ssr.read(uint2(p));
        acc += s.rgb * s.a * wb; wa += s.a * wb; wsum += wb;
    }
    if (wa < 1e-4) return;
    float3 col = acc / wa;
    float conf = wa / max(wsum, 1e-5);
    float2 uv = (float2(gid) + 0.5) * u.size.zw;
    float3 I = normalize(vpos(u, uv, 1.0));
    float3 N = world_to_view_n(u, nrv.xyz * 2.0 - 1.0);
    float f0 = u.ssr2.x;
    float nv = saturate(dot(N, -I));
    float fres = f0 + (max(1.0 - rough, f0) - f0) * pow(1.0 - nv, 5.0);
    float4 c = hdr.read(gid);
    c.rgb += col * conf * fres * u.ssr.w;
    hdr.write(c, gid);
}

// ---------------------------------------------------------------- Bloom
inline float3 bloom_thresh(constant PostU& u, float3 c) {
    float br = max(c.r, max(c.g, c.b));
    float knee = max(u.bloom.y, 1e-4);
    float soft = clamp(br - u.bloom.x + knee, 0.0, 2.0 * knee);
    soft = soft * soft / (4.0 * knee + 1e-5);
    float contrib = max(soft, br - u.bloom.x) / max(br, 1e-5);
    return c * max(contrib, 0.0);
}
inline float kluma(float3 c) { return dot(c, float3(0.2126, 0.7152, 0.0722)); }

// First downsample: threshold + Karis average over the 13-tap dual-filter pattern. hdr (full) -> d0 (half).
kernel void post_bloom_down0(texture2d<float> src [[texture(0)]], texture2d<float, access::write> dst [[texture(1)]],
                             constant PostU& u [[buffer(0)]], uint2 gid [[thread_position_in_grid]]) {
    if (gid.x >= dst.get_width() || gid.y >= dst.get_height()) return;
    float2 uv = (float2(gid) + 0.5) / float2(dst.get_width(), dst.get_height());
    float2 t = 1.0 / float2(src.get_width(), src.get_height());
    #define TAP(dx, dy) bloom_thresh(u, min(src.sample(smp, uv + float2(dx, dy) * t, level(0)).rgb, float3(1e4)))
    float3 a = TAP(-2, 2), b = TAP(0, 2), c = TAP(2, 2);
    float3 d = TAP(-2, 0), e = TAP(0, 0), f = TAP(2, 0);
    float3 g = TAP(-2, -2), h = TAP(0, -2), i = TAP(2, -2);
    float3 j = TAP(-1, 1), k = TAP(1, 1), l = TAP(-1, -1), m = TAP(1, -1);
    #undef TAP
    float3 g0 = (j + k + l + m) * 0.25, g1 = (a + b + d + e) * 0.25, g2 = (b + c + e + f) * 0.25;
    float3 g3 = (d + e + g + h) * 0.25, g4 = (e + f + h + i) * 0.25;
    float w0 = 0.5 / (1.0 + kluma(g0)), w1 = 0.125 / (1.0 + kluma(g1)), w2 = 0.125 / (1.0 + kluma(g2));
    float w3 = 0.125 / (1.0 + kluma(g3)), w4 = 0.125 / (1.0 + kluma(g4));
    float3 r = (g0 * w0 + g1 * w1 + g2 * w2 + g3 * w3 + g4 * w4) / (w0 + w1 + w2 + w3 + w4);
    dst.write(float4(r, 1), gid);
}

kernel void post_bloom_down(texture2d<float> src [[texture(0)]], texture2d<float, access::write> dst [[texture(1)]],
                            uint2 gid [[thread_position_in_grid]]) {
    if (gid.x >= dst.get_width() || gid.y >= dst.get_height()) return;
    float2 uv = (float2(gid) + 0.5) / float2(dst.get_width(), dst.get_height());
    float2 t = 1.0 / float2(src.get_width(), src.get_height());
    #define TAP(dx, dy) src.sample(smp, uv + float2(dx, dy) * t, level(0)).rgb
    float3 r = TAP(0, 0) * 0.125;
    r += (TAP(-2, 2) + TAP(2, 2) + TAP(-2, -2) + TAP(2, -2)) * 0.03125;
    r += (TAP(0, 2) + TAP(0, -2) + TAP(-2, 0) + TAP(2, 0)) * 0.0625;
    r += (TAP(-1, 1) + TAP(1, 1) + TAP(-1, -1) + TAP(1, -1)) * 0.125;
    #undef TAP
    dst.write(float4(r, 1), gid);
}

// up_i = mix(down_i, tent(up_{i+1}), 0.6)
kernel void post_bloom_up(texture2d<float> base [[texture(0)]], texture2d<float> low [[texture(1)]],
                          texture2d<float, access::write> dst [[texture(2)]], uint2 gid [[thread_position_in_grid]]) {
    if (gid.x >= dst.get_width() || gid.y >= dst.get_height()) return;
    float2 uv = (float2(gid) + 0.5) / float2(dst.get_width(), dst.get_height());
    float2 t = 1.0 / float2(low.get_width(), low.get_height());
    #define TAP(dx, dy) low.sample(smp, uv + float2(dx, dy) * t, level(0)).rgb
    float3 r = TAP(0, 0) * 4.0 + (TAP(-1, 0) + TAP(1, 0) + TAP(0, -1) + TAP(0, 1)) * 2.0
             + (TAP(-1, -1) + TAP(1, -1) + TAP(-1, 1) + TAP(1, 1));
    #undef TAP
    r *= 1.0 / 16.0;
    float3 b = base.read(gid).rgb;
    dst.write(float4(mix(b, r, 0.6), 1), gid);
}

// ---------------------------------------------------------------- Auto exposure (log-average, temporal)
kernel void post_exposure(texture2d<float> hdr [[texture(0)]], device float4* st [[buffer(2)]],
                          constant PostU& u [[buffer(0)]], uint tid [[thread_index_in_threadgroup]]) {
    threadgroup float sh[256];
    uint tx = tid & 15u, ty = tid >> 4;
    float lod = clamp(log2(max(u.size.x, u.size.y) / 128.0), 0.0, float(hdr.get_num_mip_levels() - 1));
    float acc = 0.0;
    for (uint iy = 0; iy < 8; iy++) for (uint ix = 0; ix < 8; ix++) {
        float2 uv = (float2(tx * 8 + ix, ty * 8 + iy) + 0.5) / 128.0;
        float3 c = hdr.sample(smp, uv, level(lod)).rgb;
        acc += log2(clamp(kluma(c), 1e-4, 1e5));
    }
    sh[tid] = acc;
    threadgroup_barrier(mem_flags::mem_threadgroup);
    for (uint s = 128; s > 0; s >>= 1) {
        if (tid < s) sh[tid] += sh[tid + s];
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }
    if (tid == 0) {
        float avg = exp2(sh[0] / (256.0 * 64.0));
        float target = clamp(u.fin3.x / max(avg, 1e-4), u.fin2.z, u.fin2.w);
        float4 s = st[0];
        float e = target;
        if (s.z > 0.5 && u.ints.x != 0u) {
            float speed = target < s.x ? 3.0 : 1.0;   // adapt faster to bright than to dark
            float a = 1.0 - exp(-u.fin2.y * speed);
            e = exp2(mix(log2(s.x), log2(target), a));
        }
        st[0] = float4(e, avg, 1.0, 0.0);
    }
}

// ---------------------------------------------------------------- Final: tonemap + vignette + dither
inline float3 aces_hill(float3 v) {
    const float3x3 im = float3x3(float3(0.59719, 0.07600, 0.02840), float3(0.35458, 0.90834, 0.13383), float3(0.04823, 0.01566, 0.83777));
    const float3x3 om = float3x3(float3(1.60475, -0.10208, -0.00327), float3(-0.53108, 1.10813, -0.07276), float3(-0.07367, -0.00605, 1.07602));
    v = im * v;
    float3 a = v * (v + 0.0245786) - 0.000090537;
    float3 b = v * (0.983729 * v + 0.4329510) + 0.238081;
    return saturate(om * (a / b));
}
inline float3 pbr_neutral(float3 c) {
    const float sc = 0.8 - 0.04, desat = 0.15;
    float x = min(c.r, min(c.g, c.b));
    float off = x < 0.08 ? x - 6.25 * x * x : 0.04;
    c -= off;
    float peak = max(c.r, max(c.g, c.b));
    if (peak < sc) return c;
    float d = 1.0 - sc;
    float np = 1.0 - d * d / (peak + d - sc);
    c *= np / peak;
    float g = 1.0 - 1.0 / (desat * (peak - np) + 1.0);
    return mix(c, float3(np), g);
}
inline float3 srgb_oetf(float3 c) {
    c = saturate(c);
    return select(1.055 * pow(c, 1.0 / 2.4) - 0.055, c * 12.92, c <= 0.0031308);
}

inline float3 post_tonemap(texture2d<float> hdr, texture2d<float> bloom, constant PostU& u, device const float4* ex,
                           float2 uv, float2 pix) {
    float3 c = hdr.sample(smp, uv, level(0)).rgb;
    if (u.bloom.z > 0.0) c += bloom.sample(smp, uv, level(0)).rgb * u.bloom.z;
    float e = exp2(u.fin.x) * (u.fin2.x > 0.5 ? ex[0].x : 1.0);
    c *= e;
    if (u.fin.y > 0.0) {
        float2 q = (uv - 0.5) * 2.0;
        c *= 1.0 - u.fin.y * saturate(dot(q, q) * 0.5);
    }
    c = (u.fin.w < 0.5) ? aces_hill(c) : pbr_neutral(c);
    c = srgb_oetf(c);
    if (u.fin.z > 0.0) {
        float n = ign(pix, u.ints.x) + ign(pix + float2(19.0, 7.0), u.ints.x) - 1.0;   // triangular
        c += n * u.fin.z / 255.0;
    }
    return saturate(c);
}

kernel void post_final_cs(texture2d<float> hdr [[texture(0)]], texture2d<float> bloom [[texture(1)]],
                          texture2d<float, access::write> dst [[texture(2)]],
                          constant PostU& u [[buffer(0)]], device const float4* ex [[buffer(2)]],
                          uint2 gid [[thread_position_in_grid]]) {
    if (gid.x >= dst.get_width() || gid.y >= dst.get_height()) return;
    float2 pix = float2(gid);
    float3 c = post_tonemap(hdr, bloom, u, ex, (pix + 0.5) * u.size.zw, pix);
    dst.write(float4(c, 1), gid);
}

struct VOut { float4 pos [[position]]; float2 uv; };
vertex VOut post_fs_vs(uint vid [[vertex_id]]) {
    float2 p = float2(float((vid << 1) & 2u), float(vid & 2u));
    VOut o; o.pos = float4(p * 2.0 - 1.0, 0.0, 1.0); o.uv = float2(p.x, 1.0 - p.y); return o;
}
fragment float4 post_tonemap_fs(VOut in [[stage_in]], texture2d<float> hdr [[texture(0)]], texture2d<float> bloom [[texture(1)]],
                                constant PostU& u [[buffer(0)]], device const float4* ex [[buffer(1)]]) {
    return float4(post_tonemap(hdr, bloom, u, ex, in.uv, in.pos.xy), 1.0);
}
// Variant that stores perceptual luma in alpha for the FXAA pass.
fragment float4 post_tonemap_luma_fs(VOut in [[stage_in]], texture2d<float> hdr [[texture(0)]], texture2d<float> bloom [[texture(1)]],
                                     constant PostU& u [[buffer(0)]], device const float4* ex [[buffer(1)]]) {
    float3 c = post_tonemap(hdr, bloom, u, ex, in.uv, in.pos.xy);
    return float4(c, dot(c, float3(0.299, 0.587, 0.114)));
}

// ---------------------------------------------------------------- FXAA 3.11 (quality, luma in alpha)
fragment float4 post_fxaa_fs(VOut in [[stage_in]], texture2d<float> tex [[texture(0)]], constant PostU& u [[buffer(0)]]) {
    float2 uv = in.uv;
    float2 tx = u.size.zw;
    float subpix = u.fin3.y, ethr = u.fin3.z, ethrmin = u.fin3.w;
    #define L(p) tex.sample(smp, (p), level(0)).a
    float4 rgbM = tex.sample(smp, uv, level(0));
    float lM = rgbM.a;
    float lS = L(uv + float2(0, tx.y)), lN = L(uv + float2(0, -tx.y));
    float lE = L(uv + float2(tx.x, 0)), lW = L(uv + float2(-tx.x, 0));
    float maxSM = max(lS, lM), minSM = min(lS, lM);
    float maxESW = max(lE, maxSM), minESW = min(lE, minSM);
    float maxWN = max(lN, lW), minWN = min(lN, lW);
    float rMax = max(maxWN, maxESW), rMin = min(minWN, minESW);
    float range = rMax - rMin;
    if (range < max(ethrmin, rMax * ethr)) return float4(rgbM.rgb, 1.0);
    float lNW = L(uv + float2(-tx.x, -tx.y)), lNE = L(uv + float2(tx.x, -tx.y));
    float lSW = L(uv + float2(-tx.x, tx.y)), lSE = L(uv + float2(tx.x, tx.y));
    float lNS = lN + lS, lWE = lW + lE;
    float subNSWE = lNS + lWE;
    float eH1 = -2.0 * lM + lNS, eV1 = -2.0 * lM + lWE;
    float lNESE = lNE + lSE, lNWNE = lNW + lNE;
    float eH2 = -2.0 * lE + lNESE, eV2 = -2.0 * lN + lNWNE;
    float lNWSW = lNW + lSW, lSWSE = lSW + lSE;
    float eH4 = abs(eH1) * 2.0 + abs(eH2), eV4 = abs(eV1) * 2.0 + abs(eV2);
    float eH3 = -2.0 * lW + lNWSW, eV3 = -2.0 * lS + lSWSE;
    float eH = abs(eH3) + eH4, eV = abs(eV3) + eV4;
    float subNWSWNESE = lNWSW + lNESE;
    float lengthSign = tx.x;
    bool horz = eH >= eV;
    float subA = subNSWE * 2.0 + subNWSWNESE;
    if (!horz) { lN = lW; lS = lE; }
    if (horz) lengthSign = tx.y;
    float subB = subA * (1.0 / 12.0) - lM;
    float gN = lN - lM, gS = lS - lM;
    float lNN = lN + lM, lSS = lS + lM;
    bool pairN = abs(gN) >= abs(gS);
    float grad = max(abs(gN), abs(gS));
    if (pairN) lengthSign = -lengthSign;
    float subC = saturate(abs(subB) / range);
    float2 posB = uv;
    float2 offNP = horz ? float2(tx.x, 0.0) : float2(0.0, tx.y);
    if (!horz) posB.x += lengthSign * 0.5;
    if (horz) posB.y += lengthSign * 0.5;
    float2 posN = posB - offNP, posP = posB + offNP;
    float subD = -2.0 * subC + 3.0;
    float lEndN = L(posN);
    float subE = subC * subC;
    float lEndP = L(posP);
    if (!pairN) lNN = lSS;
    float gScaled = grad * 0.25;
    float lMM = lM - lNN * 0.5;
    float subF = subD * subE;
    bool lMLT = lMM < 0.0;
    lEndN -= lNN * 0.5; lEndP -= lNN * 0.5;
    bool doneN = abs(lEndN) >= gScaled, doneP = abs(lEndP) >= gScaled;
    if (!doneN) posN -= offNP;
    if (!doneP) posP += offNP;
    constexpr float stepv[10] = {1.0, 1.0, 1.0, 1.0, 1.5, 2.0, 2.0, 2.0, 4.0, 8.0};
    for (int i = 0; i < 10 && !(doneN && doneP); i++) {
        if (!doneN) lEndN = L(posN) - lNN * 0.5;
        if (!doneP) lEndP = L(posP) - lNN * 0.5;
        doneN = abs(lEndN) >= gScaled; doneP = abs(lEndP) >= gScaled;
        if (!doneN) posN -= offNP * stepv[i];
        if (!doneP) posP += offNP * stepv[i];
    }
    float dstN = horz ? uv.x - posN.x : uv.y - posN.y;
    float dstP = horz ? posP.x - uv.x : posP.y - uv.y;
    bool dirN = dstN < dstP;
    float lEndG = dirN ? lEndN : lEndP;
    bool good = ((lEndG < 0.0) != lMLT);
    float span = dstP + dstN;
    float dstM = dirN ? dstN : dstP;
    float pOff = dstM * (-1.0 / span) + 0.5;
    float pOffGood = good ? pOff : 0.0;
    float pOffSub = max(pOffGood, subF * subF * subpix);
    float2 posM = uv;
    if (!horz) posM.x += pOffSub * lengthSign;
    if (horz) posM.y += pOffSub * lengthSign;
    #undef L
    return float4(tex.sample(smp, posM, level(0)).rgb, 1.0);
}
