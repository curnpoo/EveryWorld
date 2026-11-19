// Volumetric clouds (Phase 2 item 5; design: docs/plans/clouds.md). The field and its lighting
// are ed_world::cloudscape (field.rs, light.rs, storm.rs); this file mirrors them line for line
// and the CPU reference (ed_render/examples/cloud_preview.rs) is the look target. Constants
// named CL_* are checked against the Rust values by ed_world `gpu::tests::shader_constants_match`.
//
// Passes (src/clouds.rs):
// - clouds_march (compute, half res, checkerboard): each frame marches one half-res pixel of
//   every 2x2 (a quarter of them, 1/16 of the full-res pixels) with a per-pixel jitter, adds
//   aerial perspective, and estimates the cloud shadow on the ground under opaque pixels.
// - clouds_temporal (compute, half res): reprojects last frame's result (previous
//   view-projection, camera delta, the cloud's own depth), clamps it to this frame's fresh
//   neighbours and fills the three pixels not marched this frame. Converges the jitter: no
//   grain, no banding.
// - fs_clouds_composite (fullscreen raster, full res, premultiplied blend over HDR): a
//   depth-aware upsample; writes (L, 1 - T * shadow), so the target becomes hdr * T * shadow + L.
//
// CloudData (ed_world cloudscape/gpu.rs): float4 rows in a cpu_write buffer.

#include "shader_abi.slang"
#include "common.slang"
#include "lighting.slang"

struct ClC {
    uint frame;     // bindless FrameData
    uint cloud;     // bindless CloudData rows
    uint depth;     // bindless depth texture (full res)
    uint smp;       // bindless sampler (linear, repeat)
    uint shape;     // bindless 3D texture
    uint detail;    // bindless 3D texture
    uint weather;   // bindless 2D texture
    uint cur_a;     // half-res float4: this frame's marched (L.rgb, T)
    uint cur_b;     // half-res float4: (cloud depth km, ground shadow, scene depth km, 1)
    uint prev_a;    // last frame's history (L, T)
    uint prev_b;    // last frame's history (depth, shadow, scene depth, 1)
    uint out_a;     // this frame's history
    uint out_b;
    uint hw;        // half-res size
    uint hh;
    uint pad;
};
ED_CONSTANTS(ClC)

// ---- Constants mirrored from Rust (checked by the Rust test).
static const float CL_SIGMA_MAX = 0.11;
static const float CL_SIGMA_RAIN = 0.0012;
static const float CL_RAIN_ALBEDO = 0.55;
static const float CL_SIGMA_INTERIOR = 0.4;
static const float CL_CARVE = 0.55;
static const float CL_EDGE_RAMP = 0.55;
static const float CL_SHAPE_PERIOD_M = 5000.0;
static const float CL_DETAIL_PERIOD_M = 900.0;
static const float CL_MS_A = 0.45;
static const float CL_MS_B = 0.6;
static const float CL_MS_C = 0.5;
static const float CL_G_FWD = 0.8;
static const float CL_G_BACK = -0.3;
static const float CL_BACK_W = 0.3;
static const uint CL_OCTAVES = 4u;
static const uint CL_LIGHT_N = 8u;
// (distance, length) of the light-march steps; the last two are coarse (light.rs LIGHT_STEPS).
static const float2 CL_LIGHT[8] = {
    float2(25.0, 50.0), float2(80.0, 60.0), float2(160.0, 100.0), float2(290.0, 160.0),
    float2(500.0, 260.0), float2(860.0, 460.0), float2(1900.0, 1600.0), float2(4200.0, 3000.0)
};
static const uint CL_MAX_STORMS = 16u;
static const uint CL_LAYER_ROW0 = 6u;
static const uint CL_LAYER_ROWS = 8u;
static const uint CL_STORM_ROW0 = 30u;
static const uint CL_REPROJ_ROW0 = 94u;
static const float CL_PI = 3.14159265;
static const float CL_FAR_KM = 1.0e6;

// Rust smoothstep (well defined for any edge order).
float cl_ss(float e0, float e1, float x) {
    float t = saturate((x - e0) / (e1 - e0));
    return t * t * (3.0 - 2.0 * t);
}

float cl_remap(float v, float lo, float hi, float nlo, float nhi) {
    return nlo + (v - lo) / max(hi - lo, 1e-6) * (nhi - nlo);
}

float4* cl_rows() {
    return ed_buf<float4>(ed_c.cloud);
}

// ---- One layer (cloudscape/gpu.rs `layer_rows`).
struct ClLayer {
    float base;
    float top;
    float th;
    float cu;
    float towering;
    float density;
    float wisp;
    float active;
    float2 ax0;
    float2 ax1;
    float4 wuv;     // weather uv at the camera: field A (xy), field B (zw)
    float4 suv;     // shape uv (xy), detail uv (zw) at the camera
    float4 fade;    // weight A, weight B, variance norm, 1 / weather tile (m)
    float4 shear;   // shear east/north, precipitation, shape y offset
    float idx;
    float coverage;
};

ClLayer cl_layer(float4* cd, uint i) {
    uint r = CL_LAYER_ROW0 + i * CL_LAYER_ROWS;
    ClLayer l;
    l.base = cd[r].x;
    l.top = cd[r].y;
    l.th = cd[r].z;
    l.cu = cd[r].w;
    l.towering = cd[r + 1u].x;
    l.density = cd[r + 1u].y;
    l.wisp = cd[r + 1u].z;
    l.active = cd[r + 1u].w;
    l.ax0 = cd[r + 2u].xy;
    l.ax1 = cd[r + 2u].zw;
    l.wuv = cd[r + 3u];
    l.suv = cd[r + 4u];
    l.fade = cd[r + 5u];
    l.shear = cd[r + 6u];
    l.idx = cd[r + 7u].x;
    l.coverage = cd[r + 7u].z;
    return l;
}

float cl_thickness(ClLayer l) {
    return max(l.top - l.base, 1.0);
}

float cl_floor(ClLayer l, float ground) {
    return l.shear.z > 0.0 ? ground : l.base;
}

// ---- Storms (storm.rs `StormCell::envelope`, `rain_at`), camera-relative.
struct ClStorm {
    float2 p;       // render x, z relative to the camera
    float radius;
    float base;
    float top;
    float core;
    float anvil;
    float rain;
    float2 dir;
    float reach;
    float2 lean;
};

ClStorm cl_storm(float4* cd, uint i) {
    uint r = CL_STORM_ROW0 + i * 4u;
    ClStorm s;
    s.p = cd[r].xy;
    s.radius = cd[r].z;
    s.base = cd[r].w;
    s.top = cd[r + 1u].x;
    s.core = cd[r + 1u].y;
    s.anvil = cd[r + 1u].z;
    s.rain = cd[r + 1u].w;
    s.dir = cd[r + 2u].xy;
    s.reach = cd[r + 2u].z;
    s.lean = cd[r + 3u].xy;
    return s;
}

uint cl_storm_count(float4* cd) {
    return min(uint(cd[5].x), CL_MAX_STORMS);
}

// (envelope, height fraction) of storm s at camera-relative render (x, z) and altitude h.
float2 cl_storm_env(ClStorm s, float2 xz, float h) {
    float dz = h - s.base;
    float height = max(s.top - s.base, 1.0);
    if (dz < 0.0 || dz > height) {
        return float2(0.0);
    }
    float hf = dz / height;
    float2 rel = xz - s.p - s.lean * dz;
    float r = s.radius * (0.75 + 0.25 * cl_ss(0.0, 0.3, hf)) * (1.0 - 0.35 * cl_ss(0.85, 1.0, hf));
    float core = (1.0 - cl_ss(r * 0.45, r, length(rel))) * s.core * (1.0 - cl_ss(0.93, 1.0, hf));
    float band = cl_ss(0.7, 0.84, hf) * (1.0 - cl_ss(0.94, 1.0, hf));
    float along = dot(rel, s.dir);
    float across = rel.x * s.dir.y - rel.y * s.dir.x;   // glam perp_dot(rel, dir)
    float reach = s.reach * s.anvil;
    float2 a = float2((along - reach * 0.4) / (reach * 0.6 + s.radius), across / (s.radius * (1.1 + 1.6 * s.anvil)));
    float anvil = band * (1.0 - cl_ss(0.55, 1.0, length(a))) * s.anvil;
    return float2(max(core, anvil), hf);
}

float cl_storm_rain(ClStorm s, float2 xz) {
    if (s.rain <= 0.0) {
        return 0.0;
    }
    float d = length(xz - s.p);
    return s.rain * (1.0 - cl_ss(s.radius * 0.3, s.radius * 0.8, d));
}

// ---- Columns and profiles (field.rs `column`, `profile`).
struct ClCol {
    float presence;
    float htop;
    float rain;
    float cov;
};

ClCol cl_column(ClLayer l, float2 rel_en, float lod) {
    SamplerState smp = ed_smp(ed_c.smp);
    Texture2D wt = ed_tex2d(ed_c.weather);
    float2 q = float2(dot(l.ax0, rel_en), dot(l.ax1, rel_en)) * l.fade.w;
    float4 wa = wt.SampleLevel(smp, l.wuv.xy + q, lod);
    float4 wb = wt.SampleLevel(smp, l.wuv.zw + q, lod);
    float cov = 0.5 + ((wa.x - 0.5) * l.fade.x + (wb.y - 0.5) * l.fade.y) * l.fade.z;
    float region = (wa.w - 0.5) * 0.18 * (1.0 - l.coverage);
    float th = l.th - region;
    float soft = clamp(1.0 - th, 0.12, 0.5) * lerp(0.6, 0.85, l.cu);
    ClCol c;
    c.cov = cov;
    c.presence = saturate(cl_remap(cov, th, th + soft, 0.0, 1.0));
    float tw = wa.z * l.fade.x + wb.z * l.fade.y;
    float tall = cl_ss(1.0 - l.towering - 0.15, 1.0 - l.towering + 0.35, tw);
    float htop = lerp(lerp(0.3, 0.55, l.towering), 1.0, tall);
    float sheet = lerp(0.4, 1.0, cl_ss(0.25, 0.8, cov));
    c.htop = lerp(sheet, htop, l.cu);
    c.rain = l.shear.z * cl_ss(0.35, 0.8, c.presence) * lerp(1.0, tall, l.cu);
    return c;
}

// (envelope, height fraction in the column).
float2 cl_profile(ClLayer l, float h, float htop, float presence) {
    float thick = cl_thickness(l) * htop;
    float dh = h - l.base;
    if (dh < 0.0 || dh > thick || presence <= 0.0) {
        return float2(0.0);
    }
    float x = dh / thick;
    float cu = l.cu;
    float bottom = cl_ss(0.0, lerp(0.3 * thick, min(90.0, thick * 0.25), cu), dh);
    float dome = lerp(0.0, 0.92, pow(cl_ss(0.2, 1.0, x), 1.3));
    float waist = 0.12 * (1.0 - cl_ss(0.0, 0.2, x));
    float need = lerp(0.05 + 0.5 * cl_ss(0.6, 1.0, x), dome + waist, cu);
    float e = saturate((presence - need) / max(1.0 - need, 0.08) * lerp(1.5, 2.5, cu));
    return float2(bottom * e, x);
}

// Cheap upper bound of the density (field.rs `envelope`): 0 means certainly empty.
float cl_envelope(float4* cd, float3 r, float h, float lod_w) {
    float2 rel_en = float2(r.x, -r.z);
    float ground = cd[1].z;
    float e = 0.0;
    for (uint li = 0u; li < 3u; li++) {
        ClLayer l = cl_layer(cd, li);
        if (l.active == 0.0 || h > l.top || h < cl_floor(l, ground)) {
            continue;
        }
        ClCol c = cl_column(l, rel_en, lod_w);
        if (h < l.base) {
            e = max(e, c.rain);
            continue;
        }
        e = max(e, cl_profile(l, h, c.htop, c.presence).x);
    }
    uint ns = cl_storm_count(cd);
    for (uint i = 0u; i < ns; i++) {
        ClStorm s = cl_storm(cd, i);
        e = max(e, cl_storm_env(s, r.xz, h).x);
        if (h < s.base) {
            e = max(e, cl_storm_rain(s, r.xz));
        }
    }
    return e;
}

// Shape carved by the envelope, eroded by detail (field.rs `shape`).
float cl_shape(float4* cd, ClLayer l, float2 rel_en, float h, float env, float hf, float detail_w, float lod_s, float lod_d) {
    SamplerState smp = ed_smp(ed_c.smp);
    float dh = h - l.base;
    float2 rq = rel_en - l.shear.xy * dh;
    float2 qo = float2(dot(l.ax0, rq), dot(l.ax1, rq));
    float3 ps = float3(l.suv.x + qo.x / CL_SHAPE_PERIOD_M, h / CL_SHAPE_PERIOD_M + l.shear.w, l.suv.y + qo.y / CL_SHAPE_PERIOD_M);
    float4 s = ed_tex3d(ed_c.shape).SampleLevel(smp, ps, lod_s);
    float wfbm = s.y * 0.625 + s.z * 0.25 + s.w * 0.125;
    float base = cl_remap(s.x, wfbm - 1.0, 1.0, 0.0, 1.0);
    float th = 1.0 - env * CL_CARVE;
    float c = saturate(cl_remap(base, th, lerp(th, 1.0, CL_EDGE_RAMP), 0.0, 1.0));
    if (c <= 0.0) {
        return 0.0;
    }
    if (detail_w > 0.0) {
        float3 warp = (s.yzw - 0.5) * 0.35;
        float rise = cd[2].z;
        float3 pd = float3(l.suv.z + qo.x / CL_DETAIL_PERIOD_M, (h - rise) / CL_DETAIL_PERIOD_M + l.idx * 0.29, l.suv.w + qo.y / CL_DETAIL_PERIOD_M) + warp;
        float4 d = ed_tex3d(ed_c.detail).SampleLevel(smp, pd, lod_d);
        float billow = d.x * 0.6 + d.y * 0.32 + d.z * 0.08;
        float wisp = lerp(billow, d.w, l.wisp);
        float e = lerp(wisp, 1.0 - billow, saturate(hf * 3.0) * (1.0 - l.wisp));
        float amount = lerp(0.3, 0.55, l.wisp) * detail_w;
        c = saturate(cl_remap(c, e * amount, 1.0, 0.0, 1.0));
        float fib = 1.0 - abs(2.0 * d.w - 1.0);
        float fw = saturate((l.wisp - 0.75) * 5.0) * max(detail_w, 0.5);
        c *= lerp(1.0, cl_ss(0.65, 0.97, fib), fw);
    }
    return c * lerp(1.0, lerp(0.55, 1.0, cl_ss(0.0, 0.35, hf)), l.cu);
}

struct ClSample {
    float sigma;
    float albedo;
    float tau_up;
    float tau_down;
};

// Density at camera-relative render offset r, altitude h (field.rs `sample`).
ClSample cl_sample(float4* cd, float3 r, float h, float detail_w, float lod_s, float lod_d, float lod_w) {
    float2 rel_en = float2(r.x, -r.z);
    float ground = cd[1].z;
    ClSample o;
    o.sigma = 0.0;
    o.albedo = 1.0;
    o.tau_up = 0.0;
    o.tau_down = 0.0;
    float rain = 0.0;
    float best = 0.0;
    uint ns = cl_storm_count(cd);
    for (uint li = 0u; li < 3u; li++) {
        ClLayer l = cl_layer(cd, li);
        if (l.active == 0.0 || h > l.top || h < cl_floor(l, ground)) {
            continue;
        }
        ClCol c = cl_column(l, rel_en, lod_w);
        if (h < l.base) {
            rain = max(rain, c.rain);
            continue;
        }
        float2 pr = cl_profile(l, h, c.htop, c.presence);
        float env = pr.x;
        float hf = pr.y;
        float density = l.density;
        float col_m = cl_thickness(l) * c.htop;
        float above = (1.0 - pr.y) * col_m;
        float below = pr.y * col_m;
        if (li == 0u) {
            for (uint i = 0u; i < ns; i++) {
                ClStorm s = cl_storm(cd, i);
                float2 se = cl_storm_env(s, r.xz, h);
                if (se.x > env) {
                    env = se.x;
                    hf = se.y;
                    density = max(density, 1.5);
                    float sh = s.top - s.base;
                    above = (1.0 - se.y) * sh;
                    below = se.y * sh;
                }
            }
        }
        if (env <= 0.0) {
            continue;
        }
        float sigma = cl_shape(cd, l, rel_en, h, env, hf, detail_w, lod_s, lod_d) * density * CL_SIGMA_MAX;
        if (sigma > o.sigma) {
            best = sigma;
            float si = density * CL_SIGMA_MAX * CL_SIGMA_INTERIOR;
            o.tau_up = si * above;
            o.tau_down = si * below;
        }
        o.sigma += sigma;
    }
    ClLayer l0 = cl_layer(cd, 0u);
    if (l0.active != 0.0 && h > l0.top) {
        for (uint i = 0u; i < ns; i++) {
            ClStorm s = cl_storm(cd, i);
            float2 se = cl_storm_env(s, r.xz, h);
            if (se.x > 0.0) {
                float sigma = cl_shape(cd, l0, rel_en, h, se.x, se.y, detail_w, lod_s, lod_d) * 1.5 * CL_SIGMA_MAX;
                o.sigma += sigma;
                if (sigma > best) {
                    best = sigma;
                    float si = 1.5 * CL_SIGMA_MAX * CL_SIGMA_INTERIOR;
                    float sh = s.top - s.base;
                    o.tau_up = si * (1.0 - se.y) * sh;
                    o.tau_down = si * se.y * sh;
                }
            }
        }
    }
    for (uint i = 0u; i < ns; i++) {
        ClStorm s = cl_storm(cd, i);
        if (h < s.base) {
            rain = max(rain, cl_storm_rain(s, r.xz));
        }
    }
    if (rain > 0.0 && o.sigma < 1e-4) {
        float3 p = float3(cd[4].z + rel_en.x / 900.0, frac(h / 4000.0), cd[4].w + rel_en.y / 900.0);
        float n = ed_tex3d(ed_c.detail).SampleLevel(ed_smp(ed_c.smp), p, lod_d).w;
        o.sigma = saturate(cl_remap(n, 0.35, 0.75, 0.0, 1.0)) * rain * CL_SIGMA_RAIN;
        o.albedo = CL_RAIN_ALBEDO;
        o.tau_up = 40.0 * sqrt(rain);
        o.tau_down = 0.0;
    }
    return o;
}

// ---- Planet (light.rs `Planet`): curved altitude and shell intervals about the camera.
float3 cl_up(FrameData* f) {
    return atmos_on(f) ? normalize(f.atmos.up.xyz) : float3(0.0, 1.0, 0.0);
}

float cl_altitude(float4* cd, float3 up, float3 r) {
    float rr = cd[2].y;
    float yr = dot(r, up);
    float y = cd[2].x + yr;
    float h2 = max(dot(r, r) - yr * yr, 0.0);
    float num = h2 + y * y + 2.0 * y * rr;
    float len = sqrt(h2 + (rr + y) * (rr + y));
    return num / (len + rr);
}

// Roots (near, far) of the sphere at altitude h along a ray whose cosine with up is mu, from
// altitude a; false when it misses.
bool cl_sphere(float rr, float a, float mu, float h, out float2 t) {
    float ry = rr + a;
    float b = mu * ry;
    float c = (a - h) * (2.0 * rr + a + h);
    float disc = b * b - c;
    t = float2(0.0);
    if (disc < 0.0) {
        return false;
    }
    float s = sqrt(disc);
    float q = -b - (b >= 0.0 ? s : -s);
    float t0 = abs(q) < 1e-12 ? -s : q;
    float t1 = abs(q) < 1e-12 ? s : c / q;
    t = float2(min(t0, t1), max(t0, t1));
    return true;
}

bool cl_shell(float rr, float a, float mu, float lo, float hi, float tmax, out float2 seg) {
    seg = float2(0.0);
    float2 s_lo;
    float2 s_hi;
    float t0;
    float t1;
    if (a < lo) {
        if (!cl_sphere(rr, a, mu, lo, s_lo) || !cl_sphere(rr, a, mu, hi, s_hi)) {
            return false;
        }
        t0 = s_lo.y;
        t1 = s_hi.y;
    } else if (a <= hi) {
        if (!cl_sphere(rr, a, mu, hi, s_hi)) {
            return false;
        }
        t0 = 0.0;
        t1 = s_hi.y;
        if (cl_sphere(rr, a, mu, lo, s_lo) && s_lo.x > 0.0) {
            t1 = min(t1, s_lo.x);
        }
    } else {
        if (!cl_sphere(rr, a, mu, hi, s_hi) || s_hi.y <= 0.0) {
            return false;
        }
        t0 = max(s_hi.x, 0.0);
        t1 = s_hi.y;
        if (cl_sphere(rr, a, mu, lo, s_lo) && s_lo.x > 0.0) {
            t1 = min(t1, s_lo.x);
        }
    }
    t0 = max(t0, 0.0);
    t1 = min(t1, tmax);
    seg = float2(t0, t1);
    return t1 > t0;
}

// ---- Lighting (light.rs).
float cl_hg(float c, float g) {
    float g2 = g * g;
    return (1.0 - g2) / (4.0 * CL_PI * pow(max(1.0 + g2 - 2.0 * g * c, 1e-4), 1.5));
}

float cl_phase(float c, float k) {
    return lerp(cl_hg(c, CL_G_FWD * k), cl_hg(c, CL_G_BACK * k), CL_BACK_W);
}

float cl_sun_scatter(float tau, float c, float sigma) {
    float a = 1.0;
    float b = 1.0;
    float k = 1.0;
    float sum = 0.0;
    for (uint i = 0u; i < CL_OCTAVES; i++) {
        sum += b * cl_phase(c, k) * exp(-tau * a);
        a *= CL_MS_A;
        b *= CL_MS_B;
        k *= CL_MS_C;
    }
    float powder = 1.0 - exp(-sigma * 120.0);
    float away = saturate(0.5 - 0.5 * c);
    return sum * lerp(1.0, lerp(0.55, 1.0, powder), away);
}

float cl_diffuse_t(float tau) {
    return 1.0 / (1.0 + 0.11 * max(tau, 0.0));
}

float cl_light_depth(float4* cd, float3 up, float3 r, float3 sun, bool near, float jitter, float lod_s, float lod_d, float lod_w) {
    float tau = 0.0;
    float k = 0.7 + 0.6 * jitter;
    for (uint i = 0u; i < CL_LIGHT_N; i++) {
        float2 st = CL_LIGHT[i] * k;
        float3 q = r + sun * st.x;
        float h = cl_altitude(cd, up, q);
        float dw = (i < 4u && near) ? 1.0 : 0.0;
        ClSample s = cl_sample(cd, q, h, dw, lod_s + float(i) * 0.5, lod_d, lod_w);
        tau += s.sigma * st.y;
    }
    return tau;
}

float cl_ign(float2 p, uint frame) {
    float2 q = p + 5.588238 * float(frame % 64u);
    return frac(52.982918 * frac(0.06711056 * q.x + 0.00583715 * q.y));
}

// Ray distance (m) of the scene at a full-res pixel; CL_FAR_KM * 1000 for sky.
float cl_scene_t(FrameData* f, uint2 q, float3 dir) {
    float d = ed_depth_read(ed_c.depth, q);
    if (d <= 0.0) {
        return CL_FAR_KM * 1000.0;
    }
    float z = f.params.z / d;
    float3 dv = mul(f.view, float4(dir, 0.0)).xyz;
    return z / max(-dv.z, 1e-4);
}

struct ClMarch {
    float3 l;
    float tr;
    float depth;
};

ClMarch cl_march(FrameData* f, float4* cd, float3 dir, float tmax, float jitter, float px_angle) {
    ClMarch o;
    o.l = float3(0.0);
    o.tr = 1.0;
    o.depth = 0.0;
    float lo = cd[1].x;
    float hi = cd[1].y;
    float ground = cd[1].z;
    float rr = cd[2].y;
    float alt0 = cd[2].x;
    float3 up = cl_up(f);
    float mu = dot(dir, up);
    float2 gs;
    if (cl_sphere(rr, alt0, mu, ground, gs) && gs.x > 0.0) {
        tmax = min(tmax, gs.x);
    }
    float2 seg;
    if (!cl_shell(rr, alt0, mu, lo, hi, min(tmax, cd[1].w), seg)) {
        return o;
    }
    float3 sun = f.sun_dir.xyz;
    float c = dot(dir, sun);
    float3 sky = ambient_light(f, up);
    float3 gnd = ambient_light(f, -up);
    uint max_steps = uint(cd[0].y);
    float step_frac = cd[2].w;
    float min_step = cd[3].x;
    float max_step = cd[3].y;
    float min_tr = cd[3].z;
    float fine = cd[3].w;
    float stride = cd[5].y;
    uint refine = uint(cd[5].z);
    float seg_min = (seg.y - seg.x) / float(max_steps);
    float t = seg.x;
    t += clamp(t * step_frac, min_step, max_step) * stride * jitter;
    // Back up only right after a stride, to the sample before it (light.rs `march`).
    float t_clear = t;
    uint fine_left = 0u;
    bool strided = false;
    float depth_acc = 0.0;
    float w_acc = 0.0;
    // Texture LODs from the pixel footprint (shape texel 5000/128 m, detail 900/64, weather
    // ~cell*8/512); the CPU reference supersamples instead.
    for (uint n = 0u; n < max_steps && t < seg.y; n++) {
        float dt = max(clamp(t * step_frac, min_step, max_step), seg_min);
        float3 r = dir * t;
        float h = cl_altitude(cd, up, r);
        float foot = t * px_angle;
        float lod_s = log2(max(foot / 39.0, 1.0));
        float lod_d = log2(max(foot / 14.0, 1.0));
        float lod_w = log2(max(foot / 40.0, 1.0));
        if (cl_envelope(cd, r, h, lod_w) <= 0.0) {
            t_clear = t;
            if (fine_left > 0u) {
                fine_left -= 1u;
                t += dt * fine;
                strided = false;
            } else {
                t += dt * stride;
                strided = true;
            }
            continue;
        }
        if (strided) {
            strided = false;
            fine_left = refine;
            t = t_clear + dt * fine;
            continue;
        }
        fine_left = refine;
        dt *= fine;
        float detail_w = 1.0 - cl_ss(cd[4].x, cd[4].y, t);
        ClSample s = cl_sample(cd, r, h, detail_w, lod_s, lod_d, lod_w);
        if (s.sigma > 0.0) {
            float lj = frac(jitter + float(n) * 0.618034);
            float tau = cl_light_depth(cd, up, r, sun, t < 20000.0, lj, lod_s, lod_d, lod_w);
            float3 sun_rad = sun_light_at(f, r);
            float3 amb = sky * cl_diffuse_t(s.tau_up) + gnd * (0.6 * cl_diffuse_t(s.tau_down));
            float3 scat = sun_rad * cl_sun_scatter(tau, c, s.sigma) + amb;
            float3 src = scat * (s.sigma * s.albedo);
            float ts = exp(-s.sigma * dt);
            o.l += o.tr * (src - src * ts) / s.sigma;
            float a = o.tr * (1.0 - ts);
            depth_acc += a * t;
            w_acc += a;
            o.tr *= ts;
            if (o.tr < min_tr) {
                break;
            }
        }
        t += dt;
    }
    o.depth = w_acc > 0.0 ? depth_acc / w_acc : 0.0;
    return o;
}

// Cloud transmittance towards the sun from camera-relative ground point g (8 coarse steps).
float cl_ground_shadow(FrameData* f, float4* cd, float3 g) {
    float3 up = cl_up(f);
    float rr = cd[2].y;
    float a = cl_altitude(cd, up, g);
    float3 sun = f.sun_dir.xyz;
    float2 seg;
    if (!cl_shell(rr, a, dot(sun, up), cd[1].x, cd[1].y, 60000.0, seg)) {
        return 1.0;
    }
    float dt = (seg.y - seg.x) / 8.0;
    float tau = 0.0;
    for (uint i = 0u; i < 8u; i++) {
        float3 q = g + sun * (seg.x + (float(i) + 0.5) * dt);
        ClSample s = cl_sample(cd, q, cl_altitude(cd, up, q), 0.0, 2.0, 0.0, 1.0);
        if (s.albedo >= 1.0) {
            tau += s.sigma * dt;
        }
    }
    return exp(-tau);
}

static const uint2 CL_BAYER[4] = { uint2(0u, 0u), uint2(1u, 1u), uint2(1u, 0u), uint2(0u, 1u) };

[shader("compute")]
[numthreads(8, 8, 1)]
void clouds_march(uint3 tid: SV_DispatchThreadID) {
    FrameData* f = ed_buf<FrameData>(ed_c.frame);
    float4* cd = cl_rows();
    uint frame = asuint(cd[0].z);
    uint2 p = tid.xy * 2u + CL_BAYER[frame & 3u];
    if (p.x >= ed_c.hw || p.y >= ed_c.hh) {
        return;
    }
    uint o = p.y * ed_c.hw + p.x;
    // The 2x2 full-res block: march to its farthest surface (the composite fades clouds that
    // lie behind a nearer pixel of the block).
    uint2 fs = uint2(f.proj.zw);
    float2 pc = float2(p) * 2.0 + 1.0;
    float3 dir = view_dir(f, pc);
    float tmax = 0.0;
    float tnear = CL_FAR_KM * 1000.0;
    for (uint k = 0u; k < 4u; k++) {
        uint2 q = min(p * 2u + uint2(k & 1u, k >> 1u), fs - 1u);
        float tq = cl_scene_t(f, q, dir);
        tmax = max(tmax, tq);
        tnear = min(tnear, tq);
    }
    float4* ca = ed_buf<float4>(ed_c.cur_a);
    float4* cb = ed_buf<float4>(ed_c.cur_b);
    if (cd[0].x == 0.0) {
        ca[o] = float4(0.0, 0.0, 0.0, 1.0);
        cb[o] = float4(0.0, 1.0, tmax * 0.001, 1.0);
        return;
    }
    // Pixel footprint (rad) at half res.
    float px_angle = 2.0 * 2.0 / (f.proj.y * f.proj.w);
    float j = cl_ign(pc, frame);
    ClMarch m = cl_march(f, cd, dir, tmax, j, px_angle);
    float3 l = m.l;
    if (m.tr < 1.0 && atmos_on(f)) {
        float3 la;
        float3 ta;
        atmos_aerial(f, dir * m.depth, pc, la, ta);
        l = ta * m.l + la * (1.0 - m.tr);
    }
    float shadow = 1.0;
    if (tnear < CL_FAR_KM * 999.0) {
        shadow = cl_ground_shadow(f, cd, dir * tnear);
    }
    ca[o] = float4(l, m.tr);
    cb[o] = float4(m.depth * 0.001, shadow, tmax * 0.001, 1.0);
}

float4 cl_fetch(float4* b, int2 p) {
    p = clamp(p, int2(0), int2(int(ed_c.hw) - 1, int(ed_c.hh) - 1));
    return b[uint(p.y) * ed_c.hw + uint(p.x)];
}

float4 cl_bilinear(float4* b, float2 q) {
    float2 g = q - 0.5;
    int2 i = int2(floor(g));
    float2 w = g - float2(i);
    float4 a = lerp(cl_fetch(b, i), cl_fetch(b, i + int2(1, 0)), w.x);
    float4 c = lerp(cl_fetch(b, i + int2(0, 1)), cl_fetch(b, i + int2(1, 1)), w.x);
    return lerp(a, c, w.y);
}

[shader("compute")]
[numthreads(8, 8, 1)]
void clouds_temporal(uint3 tid: SV_DispatchThreadID) {
    uint2 p = tid.xy;
    if (p.x >= ed_c.hw || p.y >= ed_c.hh) {
        return;
    }
    FrameData* f = ed_buf<FrameData>(ed_c.frame);
    float4* cd = cl_rows();
    uint frame = asuint(cd[0].z);
    uint2 off = CL_BAYER[frame & 3u];
    bool fresh = all((p & uint2(1u, 1u)) == off);
    float4* ca = ed_buf<float4>(ed_c.cur_a);
    float4* cb = ed_buf<float4>(ed_c.cur_b);
    float4* pa = ed_buf<float4>(ed_c.prev_a);
    float4* pb = ed_buf<float4>(ed_c.prev_b);
    uint o = p.y * ed_c.hw + p.x;
    // Neighbourhood of this frame's fresh pixels (stride 2 around the nearest one).
    int2 base = int2(p & uint2(~1u, ~1u)) + int2(off);
    float4 mn = float4(1e9);
    float4 mx = float4(-1e9);
    float4 nearest_a = cl_fetch(ca, base);
    float4 nearest_b = cl_fetch(cb, base);
    for (int y = -1; y <= 1; y++) {
        for (int x = -1; x <= 1; x++) {
            float4 v = cl_fetch(ca, base + int2(x, y) * 2);
            mn = min(mn, v);
            mx = max(mx, v);
        }
    }
    // Widen the box a little: the march is jittered, its neighbours are noisy.
    float4 ext = (mx - mn) * 0.15 + float4(float3(0.02), 0.02);
    mn -= ext;
    mx += ext;
    float4 cur_a = fresh ? ca[o] : nearest_a;
    float4 cur_b = fresh ? cb[o] : nearest_b;
    // Reproject: the cloud's own depth (or far sky when there is none) into last frame.
    float3 dir = view_dir(f, float2(p) * 2.0 + 1.0);
    float4 last_b = pb[o];
    float dkm = cur_a.w < 0.999 ? cur_b.x : (last_b.w > 0.0 && last_b.x > 0.0 ? last_b.x : 30.0);
    float3 pr = dir * dkm * 1000.0 + cd[CL_REPROJ_ROW0 + 4u].xyz;
    float4 c0 = cd[CL_REPROJ_ROW0];
    float4 c1 = cd[CL_REPROJ_ROW0 + 1u];
    float4 c2 = cd[CL_REPROJ_ROW0 + 2u];
    float4 c3 = cd[CL_REPROJ_ROW0 + 3u];
    float4 clip = c0 * pr.x + c1 * pr.y + c2 * pr.z + c3;
    bool valid = cd[CL_REPROJ_ROW0 + 4u].w > 0.0 && clip.w > 0.0;
    float2 hq = float2(0.0);
    if (valid) {
        float2 ndc = clip.xy / clip.w;
        float2 uv = float2(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
        hq = uv * float2(float(ed_c.hw), float(ed_c.hh));
        valid = all(uv >= 0.0) && all(uv <= 1.0);
    }
    float4 out_a;
    float4 out_b;
    if (valid) {
        float4 ha = clamp(cl_bilinear(pa, hq), mn, mx);
        float4 hb = cl_bilinear(pb, hq);
        if (fresh) {
            out_a = lerp(ha, cur_a, 0.3);
            out_b = float4(lerp(hb.x, cur_b.x, 0.3), lerp(hb.y, cur_b.y, 0.5), cur_b.z, 1.0);
        } else {
            out_a = ha;
            out_b = float4(hb.x, hb.y, cur_b.z, 1.0);
        }
    } else {
        out_a = cur_a;
        out_b = float4(cur_b.xyz, 1.0);
    }
    ed_buf<float4>(ed_c.out_a)[o] = out_a;
    ed_buf<float4>(ed_c.out_b)[o] = out_b;
}

[shader("vertex")]
FsOut vs_clouds(uint vid: SV_VulkanVertexID) {
    return fullscreen_vertex(vid);
}

[shader("fragment")]
float4 fs_clouds_composite(FsOut i): SV_Target {
    FrameData* f = ed_buf<FrameData>(ed_c.frame);
    float4* cd = cl_rows();
    uint2 px = uint2(i.pos.xy);
    float3 dir = view_dir(f, i.pos.xy);
    float tpx = cl_scene_t(f, px, dir) * 0.001;
    float4* ha = ed_buf<float4>(ed_c.out_a);
    float4* hb = ed_buf<float4>(ed_c.out_b);
    // Depth-aware bilinear upsample from half res: taps whose scene depth differs from this
    // pixel's are down-weighted, so clouds stop at mountain and building silhouettes.
    float2 q = i.pos.xy * 0.5 - 0.5;
    int2 b = int2(floor(q));
    float2 w = q - float2(b);
    float4 acc = float4(0.0);
    float sh = 0.0;
    float cdep = 0.0;
    float wsum = 0.0;
    for (uint k = 0u; k < 4u; k++) {
        int2 o = int2(int(k & 1u), int(k >> 1u));
        float bw = (o.x == 0 ? 1.0 - w.x : w.x) * (o.y == 0 ? 1.0 - w.y : w.y);
        float4 a = cl_fetch(ha, b + o);
        float4 bb = cl_fetch(hb, b + o);
        float dz = abs(log(max(tpx, 1e-3) / max(bb.z, 1e-3)));
        float ww = bw / (1e-3 + dz) + 1e-6;
        acc += a * ww;
        sh += bb.y * ww;
        cdep += bb.x * ww;
        wsum += ww;
    }
    acc /= wsum;
    sh /= wsum;
    cdep /= wsum;
    // Clouds behind this pixel's surface (marched for a farther pixel of the block).
    float vis = acc.w < 0.999 ? saturate((tpx - cdep * 0.9) / max(cdep * 0.1, 1e-3)) : 1.0;
    float3 l = acc.rgb * vis;
    float tr = lerp(1.0, acc.w, vis);
    // Ground shadow on opaque pixels: ~60% of lit ground is direct sun.
    float s = tpx < CL_FAR_KM * 0.999 ? lerp(1.0, sh, 0.6) : 1.0;
    uint dbg = uint(cd[0].w);
    if (dbg == 1u) {
        return float4(float3(tr), 1.0);
    }
    if (dbg == 2u) {
        return float4(float3(saturate(cdep / 50.0)), 1.0);
    }
    if (dbg == 3u) {
        return float4(float3(s), 1.0);
    }
    return float4(l, 1.0 - tr * s);
}
