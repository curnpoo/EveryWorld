// Physically based atmosphere tables (Phase 2 W5), one compute pass "atmos" before resolve
// (src/atmosphere.rs `record_atmosphere`). Dispatches, in order, with a barrier between each:
//   atmos_transmittance  256 x 64: transmittance to the top of the atmosphere per (h, mu)
//                        (only when the medium changed)
//   atmos_multiscatter   32 x 32: multiple-scattering radiance per unit sun irradiance per
//                        (mu_s, h), Hillaire's Psi_ms (only when the medium changed)
//   atmos_skyview        192 x 108 from the camera (mode 0) and 96 x 54 from the surface
//                        (mode 1, for the ambient): sky radiance per (azimuth from the sun,
//                        zenith angle), two halves split at the horizon
//   atmos_ap             32 x 32 froxel columns x 32 exponential depth slices: in-scattered
//                        radiance and transmittance from the camera to each slice's far end
//   atmos_sh_partial     32 equal-area bands of the surface sky -> SH9 partial sums
//   atmos_sh_final       9 coefficients in the shader's form (lighting.slang `sh_irradiance`)
// Every thread writes its own texel, and sums run in a fixed order: the output is the same bit
// for bit on every run (no atomics, no thread-order reductions).
#include "shader_abi.slang"
#include "common.slang"
#include "atmos.slang"

struct AtmosC {
    uint frame;     // bindless: FrameData
    uint mode;      // atmos_skyview: 0 camera table, 1 surface table
    uint pad[14];
};
ED_CONSTANTS(AtmosC)

static const uint ATM_MS_DIRS = 8;      // 8 x 8 directions per multiple-scattering texel
static const uint ATM_MS_STEPS = 20;
static const uint ATM_TRANS_STEPS = 40;
static const uint ATM_SKY_STEPS = 30;
static const uint ATM_SURF_STEPS = 20;
static const uint ATM_AP_SUB = 2;       // ray-march steps per froxel slice
static const uint ATM_SH_BANDS = 32;    // equal-area bands (one thread each) x ATM_SH_PHI
static const uint ATM_SH_PHI = 64;

[shader("compute")]
[numthreads(8, 8, 1)]
void atmos_transmittance(uint3 tid: SV_DispatchThreadID) {
    if (tid.x >= ATM_TRANS_W || tid.y >= ATM_TRANS_H) {
        return;
    }
    FrameData* f = ed_buf<FrameData>(ed_c.frame);
    float rb = atm_rb(f);
    float rt = atm_rt(f);
    float ht = rt - rb;
    float u = float(tid.x) / float(ATM_TRANS_W - 1u);
    float v = float(tid.y) / float(ATM_TRANS_H - 1u);
    // Inverse of atm_trans_uv.
    float big_h = sqrt(ht * (rt + rb));
    float rho = v * big_h;
    float h = rho * rho / (sqrt(rho * rho + rb * rb) + rb);
    float r = rb + h;
    float dmin = ht - h;
    float dmax = rho + big_h;
    float d = dmin + u * (dmax - dmin);
    float mu = d > 1e-6 ? clamp(((ht - h) * (rt + r) - d * d) / (2.0 * r * d), -1.0, 1.0) : 1.0;
    // Quadratic steps: dense near the start, where the aerosol layer (1.2 km scale height) is.
    float3 od = float3(0.0);
    float prev = 0.0;
    for (uint i = 0u; i < ATM_TRANS_STEPS; i++) {
        float x = float(i + 1u) / float(ATM_TRANS_STEPS);
        float t1 = d * x * x;
        od += atm_medium(f, atm_height_at(rb, h, mu, 0.5 * (prev + t1))).ext * (t1 - prev);
        prev = t1;
    }
    ed_buf<float4>(f.atmos.bufs.x)[tid.y * ATM_TRANS_W + tid.x] = float4(exp(-od), 1.0);
}

[shader("compute")]
[numthreads(8, 8, 1)]
void atmos_multiscatter(uint3 tid: SV_DispatchThreadID) {
    if (tid.x >= ATM_MS_N || tid.y >= ATM_MS_N) {
        return;
    }
    FrameData* f = ed_buf<FrameData>(ed_c.frame);
    float rb = atm_rb(f);
    float ht = atm_rt(f) - rb;
    float mu_s = float(tid.x) / float(ATM_MS_N - 1u) * 2.0 - 1.0;
    float h = max(float(tid.y) / float(ATM_MS_N - 1u) * ht, 1e-3);
    float ss = sqrt(max(1.0 - mu_s * mu_s, 0.0));
    float3 lsum = float3(0.0);
    float3 fsum = float3(0.0);
    for (uint i = 0u; i < ATM_MS_DIRS; i++) {
        float mu = 1.0 - 2.0 * (float(i) + 0.5) / float(ATM_MS_DIRS);
        float sv = sqrt(max(1.0 - mu * mu, 0.0));
        bool ground = atm_hits_ground(rb, h, mu);
        float tmax = ground ? atm_dist_ground(rb, h, mu) : atm_dist_top(rb, rb + ht, h, mu);
        for (uint j = 0u; j < ATM_MS_DIRS; j++) {
            float ph = 2.0 * ATM_PI * (float(j) + 0.5) / float(ATM_MS_DIRS);
            float nu = mu * mu_s + sv * ss * cos(ph);
            float3 thr = float3(1.0);
            float3 l = float3(0.0);
            float3 fms = float3(0.0);
            float prev = 0.0;
            for (uint k = 0u; k < ATM_MS_STEPS; k++) {
                float x = float(k + 1u) / float(ATM_MS_STEPS);
                float t1 = tmax * x * x;
                float dt = t1 - prev;
                float tm = 0.5 * (prev + t1);
                prev = t1;
                float hh = atm_height_at(rb, h, mu, tm);
                float mus = clamp(((rb + h) * mu_s + tm * nu) / (rb + hh), -1.0, 1.0);
                AtmMedium m = atm_medium(f, hh);
                float3 st = exp(-m.ext * dt);
                float3 e = max(m.ext, float3(1e-9));
                float3 s = atm_sun_transmittance(f, hh, mus) * m.scat / (4.0 * ATM_PI);
                l += thr * (s - s * st) / e;
                fms += thr * (m.scat - m.scat * st) / e;
                thr *= st;
            }
            if (ground) {
                float mus = clamp(((rb + h) * mu_s + tmax * nu) / rb, -1.0, 1.0);
                l += thr * atm_sun_transmittance(f, 0.0, mus) * (saturate(mus) * f.atmos.ap.z) * f.atmos.ground.rgb / ATM_PI;
            }
            lsum += l;
            fsum += fms;
        }
    }
    float n = float(ATM_MS_DIRS * ATM_MS_DIRS);
    float3 l2 = lsum / n;
    float3 fms = fsum / n;
    float3 psi = l2 / max(float3(1.0) - fms, float3(1e-3));
    ed_buf<float4>(f.atmos.bufs.y)[tid.y * ATM_MS_N + tid.x] = float4(psi, 1.0);
}

[shader("compute")]
[numthreads(8, 8, 1)]
void atmos_skyview(uint3 tid: SV_DispatchThreadID) {
    FrameData* f = ed_buf<FrameData>(ed_c.frame);
    bool surf = ed_c.mode != 0u;
    uint w = surf ? ATM_SURF_W : ATM_SKY_W;
    uint rows = surf ? ATM_SURF_H : ATM_SKY_H;
    if (tid.x >= w || tid.y >= rows) {
        return;
    }
    float h = surf ? f.atmos.radii.w : f.atmos.radii.z;
    uint half_rows = rows / 2u;
    bool lower = tid.y >= half_rows;
    float u = float(tid.x) / float(w - 1u);
    float c = float(tid.y - (lower ? half_rows : 0u)) / float(half_rows - 1u);
    float zha = acos(atm_cos_horizon(atm_rb(f), h));
    // Inverse of atm_sky_view's mapping, nudged off the horizon itself (the tangent ray belongs
    // to neither half).
    float theta = lower ? max(zha + (ATM_PI - zha) * c * c, zha + 2e-4) : min(zha * (1.0 - (1.0 - c) * (1.0 - c)), zha - 2e-4);
    float mu = cos(theta);
    float mu_s = f.atmos.up.w;
    float ss = sqrt(max(1.0 - mu_s * mu_s, 0.0));
    float nu = clamp(mu * mu_s + sin(theta) * ss * cos(ATM_PI * u * u), -1.0, 1.0);
    float3 l = atm_sky_ray(f, h, mu, mu_s, nu, surf ? ATM_SURF_STEPS : ATM_SKY_STEPS);
    ed_buf<float4>(surf ? f.atmos.bufs.w : f.atmos.bufs.z)[tid.y * w + tid.x] = float4(l, 1.0);
}

[shader("compute")]
[numthreads(8, 8, 1)]
void atmos_ap(uint3 tid: SV_DispatchThreadID) {
    if (tid.x >= ATM_AP_N || tid.y >= ATM_AP_N) {
        return;
    }
    FrameData* f = ed_buf<FrameData>(ed_c.frame);
    float2 px = (float2(tid.xy) + 0.5) / float(ATM_AP_N) * f.viewport.zw;
    float3 d = view_dir(f, px);
    float h = f.atmos.radii.z;
    float mu = clamp(dot(d, f.atmos.up.xyz), -1.0, 1.0);
    float mu_s = f.atmos.up.w;
    float nu = dot(d, f.sun_dir.xyz);
    float te;
    bool inside = atm_enter(f, h, mu, mu_s, nu, te);
    float pr = atm_phase_rayleigh(nu);
    float pm = atm_phase_mie(f.atmos.mie_a.w, nu);
    AtmAcc a;
    a.l = float3(0.0);
    a.t = float3(1.0);
    float prev = 0.0;
    float4* lb = ed_buf<float4>(f.atmos.bufs2.x);
    float4* tb = ed_buf<float4>(f.atmos.bufs2.y);
    for (uint k = 0u; k < ATM_AP_N; k++) {
        if (inside) {
            float t1 = atm_ap_depth(f, k);
            for (uint s = 0u; s < ATM_AP_SUB; s++) {
                float ta = lerp(prev, t1, float(s) / float(ATM_AP_SUB));
                float tb1 = lerp(prev, t1, float(s + 1u) / float(ATM_AP_SUB));
                atm_segment(f, a, h, mu, mu_s, nu, 0.5 * (ta + tb1), tb1 - ta, pr, pm);
            }
            prev = t1;
        }
        uint b = (k * ATM_AP_N + tid.y) * ATM_AP_N + tid.x;
        lb[b] = float4(a.l, 1.0);
        tb[b] = float4(a.t, 1.0);
    }
}

static const float ATM_SH_K[9] = { 0.282095, 0.488603, 0.488603, 0.488603, 1.092548, 1.092548, 0.315392, 1.092548, 0.546274 };
static const float ATM_SH_A[9] = { 1.0, 2.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0, 0.25, 0.25, 0.25, 0.25, 0.25 };

[shader("compute")]
[numthreads(32, 1, 1)]
void atmos_sh_partial(uint3 tid: SV_DispatchThreadID) {
    if (tid.x >= ATM_SH_BANDS) {
        return;
    }
    FrameData* f = ed_buf<FrameData>(ed_c.frame);
    // Same equal-area grid as light.rs `sh9_of` (polar axis z).
    float ct = 1.0 - 2.0 * (float(tid.x) + 0.5) / float(ATM_SH_BANDS);
    float st = sqrt(max(1.0 - ct * ct, 0.0));
    float3 acc[9];
    for (uint k = 0u; k < 9u; k++) {
        acc[k] = float3(0.0);
    }
    for (uint j = 0u; j < ATM_SH_PHI; j++) {
        float ph = 2.0 * ATM_PI * (float(j) + 0.5) / float(ATM_SH_PHI);
        float3 d = float3(st * cos(ph), st * sin(ph), ct);
        float3 l = atm_sky_view(f, f.atmos.bufs.w, ATM_SURF_W, ATM_SURF_H, f.atmos.radii.w, d);
        float y[9] = { 1.0, d.y, d.z, d.x, d.x * d.y, d.y * d.z, 3.0 * d.z * d.z - 1.0, d.x * d.z, d.x * d.x - d.y * d.y };
        for (uint k = 0u; k < 9u; k++) {
            acc[k] += l * (y[k] * ATM_SH_K[k]);
        }
    }
    float4* out = ed_buf<float4>(f.atmos.bufs2.z);
    for (uint k = 0u; k < 9u; k++) {
        out[tid.x * 9u + k] = float4(acc[k], 0.0);
    }
}

[shader("compute")]
[numthreads(32, 1, 1)]
void atmos_sh_final(uint3 tid: SV_DispatchThreadID) {
    if (tid.x >= 9u) {
        return;
    }
    FrameData* f = ed_buf<FrameData>(ed_c.frame);
    float4* buf = ed_buf<float4>(f.atmos.bufs2.z);
    float3 s = float3(0.0);
    for (uint i = 0u; i < ATM_SH_BANDS; i++) {
        s += buf[i * 9u + tid.x].rgb;
    }
    float d_omega = 4.0 * ATM_PI / float(ATM_SH_BANDS * ATM_SH_PHI);
    buf[ATM_SH_OUT + tid.x] = float4(s * (d_omega * ATM_SH_A[tid.x] * ATM_SH_K[tid.x]), 0.0);
}
