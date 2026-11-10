#include <metal_stdlib>
using namespace metal;

// Native volumetric clouds: a port of ed_game/src/clouds.wesl (the Bevy path) with altitude-stability fixes.
//
// Pipeline (see src/clouds.rs):
//   cloud_march   half-res raymarch of the cloud shell, clipped against the scene depth (checkerboard of the
//                 2x2 block's nearest / farthest sample so both sides of a silhouette exist nearby)
//   cloud_resolve temporal accumulation: reproject history by the cloud-weighted hit distance, depth-validate,
//                 variance-clip against the current 3x3
//   cloud_comp_fs depth-aware bilateral upsample, premultiplied-blended onto the HDR target
//
// Units: cloud lighting is computed in lux (like the Bevy shader) and scaled to native HDR by V.proj.z at the end.
// Altitude-stability changes vs the Bevy shader (each marked "STABLE:"):
//   * weather coverage is sampled on a fixed sea-level plane (was the camera's altitude plane: it slid through
//     the noise as you climbed, so coverage changed with height alone)
//   * the max distance grows with the horizon distance above the layer and fades out (was a hard ring)
//   * the step size grows to cover the whole ray span within the step budget (was truncated, hard edge)
//   * noise is sampled with a mip level from the step size (was aliasing/sparkle at distance)
//   * sun elevation is evaluated at the sample's own local vertical (was the camera's, wrong far away)

constant float PI = 3.14159265;
constant float SHAPE_PERIOD = 8192.0;
constant float DETAIL_PERIOD = 1024.0;
constant float WEATHER_PERIOD = 65536.0;
constant float SIGMA = 0.036;
constant float3 TAU_RAYLEIGH = float3(0.0464, 0.1080, 0.2650);
constant float TAU_MIE = 0.06;
constant float3 TAU_OZONE = float3(0.0140, 0.0420, 0.0020);
constant float PLANET_R = 6371000.0;

struct CloudU {
    float4 nfr0; float4 nfr1; float4 nfr2;   // noise_from_render columns
    float4 cam_noise;
    float4 cam_pos;
    float4 cam_up;       // xyz up, w camera altitude (m)
    float4 planet;       // x radius, y max distance, z sun white balance, w brightness gain
    float4 cu;           // cumulus base, top, coverage, density
    float4 st;           // stratus base, top, amount, belly darkness
    float4 misc;         // precipitation, storm count, steps, light steps
    float4 wind;         // xyz downwind, w speed
    float4 shell;        // bottom, top, haze distance, step scale
    float4 sun;          // xyz to sun, w lux
    float4 sky_ambient;
    float4 ground_ambient;
    float4 haze;
    float4 storm_pos[8];
    float4 storm_up[8];
};

struct ViewU {
    float4 right;        // camera right * tan(fov/2) * aspect
    float4 up;           // camera up * tan(fov/2)
    float4 fwd;          // camera forward (unit)
    float4 size;         // half-res w, h, 1/w, 1/h
    float4 proj;         // x near, y frame, z lux->HDR scale, w history reset (1 = drop history)
    float4 full;         // full-res w, h
    float4x4 prev_vp;    // previous frame's un-jittered reverse-Z view-projection
    float4 sunv;         // xyz to sun, w day factor
};

constexpr sampler ns(address::repeat, filter::linear, mip_filter::linear);
constexpr sampler hs(address::clamp_to_edge, filter::linear);

float3 to_noise(constant CloudU& cl, float3 v) { return cl.nfr0.xyz * v.x + cl.nfr1.xyz * v.y + cl.nfr2.xyz * v.z; }

float sat(float x) { return clamp(x, 0.0, 1.0); }
float remap(float v, float lo, float hi, float nlo, float nhi) { return nlo + (v - lo) / max(hi - lo, 1e-5) * (nhi - nlo); }
float hg(float c, float g) { float g2 = g * g; return (1.0 - g2) / (4.0 * PI * pow(max(1.0 + g2 - 2.0 * g * c, 1e-4), 1.5)); }
float phase(float c, float k) { return mix(hg(c, 0.78 * k), hg(c, -0.25 * k), 0.3); }

// Same analytic sky as world.metal (keep in sync): clouds fade into exactly the colour the fog fades terrain to.
float3 sky_rad(float3 d, float3 sun, float day) {
    float h = d.y;
    float3 zen = float3(0.025, 0.09, 0.40), hor = float3(0.36, 0.55, 0.95);
    float t = pow(saturate(1.0 - max(h, 0.0)), 6.5);
    float3 c = mix(zen, hor, t);
    float mu = dot(d, sun);
    float warm = saturate(1.0 - sun.y * 2.5);
    c += float3(1.0, 0.42, 0.14) * pow(saturate(mu * 0.5 + 0.5), 5.0) * warm * 0.85 * (0.4 + t);
    c = mix(c, c * float3(1.0, 0.7, 0.55), warm * 0.4);
    c += float3(1.0, 0.9, 0.7) * pow(saturate(mu), 28.0) * 0.35;
    return c * day;
}

// Distances along a ray (altitude h_c, cosine mu to up) to the sphere at altitude h. (near, far); far < near = miss.
float2 ray_shell(float h_c, float mu, float h) {
    float R = PLANET_R;
    float r_c = R + h_c;
    float dr2 = (h - h_c) * (2.0 * R + h + h_c);
    float b = r_c * mu;
    float disc = b * b + dr2;
    if (disc < 0.0) return float2(-1.0, -2.0);
    float s = sqrt(disc);
    if (b >= 0.0) return float2(-(b + s), dr2 / max(b + s, 1e-3));
    return float2(-dr2 / max(s - b, 1e-3), s - b);
}

float altitude_at(float h_c, float mu, float t) {
    float r_c = PLANET_R + h_c;
    float num = t * t + 2.0 * t * r_c * mu;
    float r = sqrt(max(r_c * r_c + num, 0.0));
    return h_c + num / (r + r_c);
}

float air_column(float h, float mu, float H) {
    float R = PLANET_R;
    float C = sqrt(PI * R / (2.0 * H));
    if (mu >= 0.0) return exp(-h / H) / (mu + 1.0 / C);
    float ht = (R + h) * sqrt(max(1.0 - mu * mu, 0.0)) - R;
    return 2.0 * C * exp(-ht / H) - exp(-h / H) / (-mu + 1.0 / C);
}

float3 sun_transmittance(float h, float mu_s) {
    float R = PLANET_R;
    if (mu_s < 0.0) {
        float ht = (R + h) * sqrt(max(1.0 - mu_s * mu_s, 0.0)) - R;
        if (ht < 0.0) return float3(0.0);
    }
    float hh = max(h, 0.0);
    float am_r = air_column(hh, mu_s, 8000.0);
    float am_m = air_column(hh, mu_s, 1500.0);
    float am_o = air_column(hh, mu_s, 8000.0);
    return exp(-(TAU_RAYLEIGH * am_r + float3(TAU_MIE * am_m) + TAU_OZONE * am_o));
}

struct Smp { float density; float height_frac; };

// STABLE: pw is the sample projected to SEA LEVEL along the camera's up, so the weather map doesn't move when
// the camera changes altitude.
float weather_at(texture3d<float> shape, float3 pw) {
    float4 a = shape.sample(ns, pw / WEATHER_PERIOD, level(0));
    float4 b = shape.sample(ns, pw / (WEATHER_PERIOD * 0.25) + float3(0.37, 0.11, 0.73), level(0));
    return a.g * 0.6 + b.r * 0.4;
}

// (influence, height fraction in the tower, anvil weight, signed edge distance in core radii)
float4 storms_at(constant CloudU& cl, float3 p, float h) {
    float4 best = float4(0.0, 0.0, 0.0, 9.0);
    int n = int(cl.misc.y);
    float base = cl.cu.x;
    float3 wind = cl.wind.xyz;
    for (int i = 0; i < n; i++) {
        float4 c = cl.storm_pos[i];
        float4 u = cl.storm_up[i];
        float top = u.w;
        if (h > top || h < 0.0) continue;
        float fh = sat((h - base) / (top - base));
        float3 d = p - c.xyz;
        d = d - u.xyz * dot(d, u.xyz);
        float3 dt = d - wind * (fh * fh * c.w * 0.5);
        float r_tower = c.w * (1.0 - 0.25 * smoothstep(0.1, 0.7, fh));
        float edge_t = length(dt) / r_tower - 1.0;
        float flare = smoothstep(0.8, 0.95, fh);
        float along = dot(d, wind) - flare * c.w * 2.4;
        float3 cross_v = d - wind * dot(d, wind);
        float r_along = c.w * mix(0.6, along > 0.0 ? 4.2 : 1.4, flare);
        float r_cross = c.w * mix(0.6, 1.7, flare);
        float e_a = sqrt((along * along) / (r_along * r_along) + dot(cross_v, cross_v) / (r_cross * r_cross));
        float edge_a = fh > 0.8 ? e_a - 1.0 : 9.0;
        float edge = min(edge_t, edge_a);
        if (edge < best.w) {
            float anvil = sat(0.5 + (edge_t - edge_a) * 2.0) * flare;
            best = float4(smoothstep(0.0, -0.45, edge), fh, anvil, edge);
        }
    }
    return best;
}

// `dt` is the step length this sample stands for: it picks the noise mip so a coarse step averages the field
// instead of aliasing it. STABLE: mips instead of point-sampling at distance.
Smp density(constant CloudU& cl, texture3d<float> shape, texture3d<float> detail, float3 p, float3 pn, float3 pw, float h, float t, bool want_detail, float dt) {
    Smp s; s.density = 0.0; s.height_frac = 0.0;
    float lod_s = clamp(log2(max(dt, 1.0) / 128.0), 0.0, 5.0);
    float lod_d = clamp(log2(max(dt, 1.0) / 48.0), 0.0, 4.0);
    float cov = cl.cu.z;
    float wx = weather_at(shape, pw);
    float local_cov = sat(cov + (wx - 0.5) * (0.7 - 0.4 * cov));
    float4 storm = storms_at(cl, p, h);

    float4 sh = shape.sample(ns, pn / SHAPE_PERIOD, level(lod_s));
    float wfbm = sh.g * 0.625 + sh.b * 0.25 + sh.a * 0.125;
    float base_shape = sat(remap(sh.r, wfbm - 1.0, 1.0, 0.0, 1.0));

    float d = 0.0, hf = 0.0;
    float cu_base = cl.cu.x;
    float cu_top = cu_base + (cl.cu.y - cu_base) * (0.35 + 0.65 * sat(local_cov * 1.6));
    float f = (h - cu_base) / max(cu_top - cu_base, 1.0);
    if (f > 0.0 && f < 1.0 && local_cov > 0.0) {
        float grad = sat(f / 0.07) * sat((1.0 - f) / 0.55);
        float cov_h = local_cov * mix(1.0, 0.55, f * f);
        float v = sat(remap(base_shape * grad, 1.0 - cov_h, 1.0, 0.0, 1.0)) * cov_h;
        if (v > d) { d = v; hf = f; }
    }

    float amount = cl.st.z;
    if (amount > 0.0) {
        float fs = (h - cl.st.x) / max(cl.st.y - cl.st.x, 1.0);
        if (fs > 0.0 && fs < 1.0) {
            float4 roll = shape.sample(ns, pn / (SHAPE_PERIOD * 2.0) + float3(0.5, 0.2, 0.9), level(lod_s));
            float lump = roll.r * 0.6 + base_shape * 0.4;
            float prof = sat(fs / 0.12) * sat((1.0 - fs) / (0.25 + 0.5 * lump));
            float st_cov = sat(amount * (0.6 + 0.6 * wx));
            float v = sat(remap(lump * prof, 1.0 - st_cov, 1.0, 0.0, 1.0)) * (0.25 + 0.75 * lump) * (0.4 + 0.6 * st_cov);
            if (v > d) { d = v; hf = fs; }
        }
    }

    if (storm.w < 0.75 && h > cu_base - 150.0) {
        float fh = storm.y;
        float4 b1 = shape.sample(ns, pn / SHAPE_PERIOD + float3(0.3, 0.6, 0.1), level(lod_s));
        float4 b2 = shape.sample(ns, pn / (SHAPE_PERIOD * 0.5), level(lod_s));
        float billow = b1.g * 0.55 + b2.r * 0.45;
        float tower_edge = storm.w + (0.5 - billow) * 1.3;
        float anvil_edge = storm.w + (0.5 - base_shape) * 0.8 + (0.5 - billow) * 0.5;
        float e = mix(tower_edge, anvil_edge, storm.z);
        float solid = smoothstep(0.05, -0.2, e);
        float cap = 1.0 - smoothstep(0.95, 1.0, fh);
        float foot = sat((h - cu_base) / 120.0);
        float v = solid * cap * foot * mix(1.4, 0.12 * smoothstep(0.1, -0.5, e) + 0.14, storm.z);
        if (v > d) { d = v; hf = fh; }
    }

    if (!want_detail && d > 0.0) d = sat(remap(d, 0.14, 1.0, 0.0, 1.0));
    if (want_detail && d > 0.0) {
        // Detail erodes edges; it fades with distance (and with the mip) rather than shimmering.
        float fade = sat(1.0 - t / 40000.0) * sat(1.0 - lod_d / 3.5);
        float4 dn = detail.sample(ns, pn / DETAIL_PERIOD, level(lod_d));
        float dfbm = dn.r * 0.625 + dn.g * 0.25 + dn.b * 0.125;
        float dmod = mix(dfbm, 1.0 - dfbm, sat(hf * 4.0));
        float strength = mix(0.42, 0.15, sat(storm.x * 2.0) * (1.0 - storm.z));
        d = sat(remap(d, dmod * strength * fade, 1.0, 0.0, 1.0));
    }
    s.density = d * cl.cu.w;
    s.height_frac = hf;
    return s;
}

float light_depth(constant CloudU& cl, texture3d<float> shape, texture3d<float> detail, float3 p, float3 pn, float h, float t, float mu_s_local) {
    int steps = int(cl.misc.w);
    float3 sun = cl.sun.xyz;
    float3 sun_n = to_noise(cl, sun);
    float3 up_n = to_noise(cl, cl.cam_up.xyz);
    float od = 0.0, dist = 0.0;
    float step_len = 3500.0 * 1.2 / (pow(2.2, float(steps)) - 1.0);
    for (int i = 0; i < steps; i++) {
        float mid = dist + step_len * 0.5;
        float hp = h + mu_s_local * mid;
        if (hp > cl.shell.y) break;
        float3 pp = p + sun * mid;
        float3 pnp = pn + sun_n * mid;
        float3 pwp = pnp - up_n * hp;   // STABLE: sea-level plane
        od += density(cl, shape, detail, pp, pnp, pwp, hp, t + mid, false, step_len).density * step_len;
        dist += step_len;
        step_len *= 2.2;
    }
    return od * SIGMA;
}

float ign(float2 xy) { return fract(52.9829189 * fract(dot(xy, float2(0.06711056, 0.00583715)))); }

// Rain shafts under the storm cells. Returns (rgb radiance in lux/sr-ish, transmittance).
float4 rain_shafts(constant CloudU& cl, texture3d<float> shape, texture3d<float> detail, float3 ro, float3 rd, float h_c, float mu, float t_a, float t_b, float jitter) {
    float precip = cl.misc.x;
    int n = int(cl.misc.y);
    float3 col = float3(0.0);
    float T = 1.0;
    if (precip <= 0.0 || n == 0) return float4(col, T);
    float base = cl.cu.x;
    float3 wind = cl.wind.xyz;
    float3 wind_n = to_noise(cl, wind);
    float3 up_n = to_noise(cl, cl.cam_up.xyz);
    float3 lum = (cl.sky_ambient.rgb * 0.55 + cl.ground_ambient.rgb * 0.45) / PI;
    for (int i = 0; i < n; i++) {
        float4 c = cl.storm_pos[i];
        float3 u = cl.storm_up[i].xyz;
        float rr = c.w * 0.85;
        float3 o = ro - (c.xyz + wind * c.w * 0.35);
        float3 op = o - u * dot(o, u);
        float3 dp = rd - u * dot(rd, u);
        float A = dot(dp, dp);
        float B = 2.0 * dot(op, dp);
        float C = dot(op, op) - rr * rr;
        float disc = B * B - 4.0 * A * C;
        if (disc <= 0.0 || A < 1e-6) continue;
        float sq = sqrt(disc);
        float ta = max((-B - sq) / (2.0 * A), t_a);
        float tb = min((-B + sq) / (2.0 * A), t_b);
        if (tb <= ta) continue;
        float len = tb - ta;
        float od = 0.0;
        for (int k = 0; k < 6; k++) {
            float ts = ta + len * (float(k) + jitter) / 6.0;
            float h = altitude_at(h_c, mu, ts);
            if (h > base || h < -50.0) continue;
            float3 p = ro + rd * ts;
            float3 q = p - (c.xyz + wind * c.w * 0.35);
            float3 qp = q - u * dot(q, u);
            float radial = smoothstep(1.0, 0.25, length(qp) / rr);
            float3 pn = cl.cam_noise.xyz + to_noise(cl, rd * ts);
            float3 pw = pn - up_n * h + wind_n * (base - h) * 0.35;
            float4 sn = detail.sample(ns, pw / (DETAIL_PERIOD * 2.0), level(1));
            float curtain = shape.sample(ns, pw / (SHAPE_PERIOD * 0.5), level(1)).r;
            float streak = sat(sn.g * 1.5 - 0.35) * sat(curtain * 1.8 - 0.2);
            float vert = smoothstep(base, base * 0.7, h) * (0.55 + 0.45 * sat(h / 300.0));
            od += radial * streak * vert * (len / 6.0) * 0.0009 * precip;
        }
        float Ts = exp(-od);
        col += T * lum * (1.0 - Ts);
        T *= Ts;
    }
    return float4(col, T);
}

// The march for one texel. `d` = raw reverse-Z scene depth (0 = sky). Returns premultiplied HDR colour + alpha,
// and the alpha-weighted mean hit distance through `t_mean` (used for temporal reprojection).
float4 march(constant CloudU& cl, constant ViewU& V, texture3d<float> shape, texture3d<float> detail, uint2 pix, float3 rd, float d, thread float& t_mean) {
    float3 ro = cl.cam_pos.xyz;
    float3 fwd = V.fwd.xyz;
    t_mean = 100000.0;

    float t_scene = 1e30;
    if (d > 0.0) {
        float view_z = V.proj.x / d;
        t_scene = view_z / max(dot(rd, fwd), 1e-4);
    }

    float3 up = cl.cam_up.xyz;
    float h_c = cl.cam_up.w;
    float mu = dot(rd, up);
    float h_lo = cl.shell.x;
    float h_hi = cl.shell.y;
    float2 lo = ray_shell(h_c, mu, h_lo);
    float2 hi = ray_shell(h_c, mu, h_hi);
    float2 ground = ray_shell(h_c, mu, 0.0);
    bool hits_ground = mu < 0.0 && ground.y >= ground.x && ground.x > 0.0;

    // STABLE: reach grows with the horizon distance above the layer (a camera 15 km above sees cloud tops at the
    // horizon ~450 km away; a fixed 300 km clip drew a hard ring).
    float horizon = sqrt(2.0 * PLANET_R * max(h_c - h_hi, 0.0));
    float t_limit = cl.planet.y + horizon;
    float t_max = min(t_scene, t_limit);
    float jitter = fract(ign(float2(pix)) + fract(V.proj.y * 0.618034) );

    float t0 = 0.0, t1 = 0.0;
    if (h_c < h_lo) {
        if (!hits_ground) { t0 = lo.y; t1 = hi.y; }
    } else if (h_c <= h_hi) {
        t0 = 0.0;
        if (lo.y >= lo.x && lo.x > 0.0) t1 = lo.x; else t1 = hi.y;
    } else if (hi.y >= hi.x && hi.y > 0.0) {
        t0 = max(hi.x, 0.0);
        if (lo.y >= lo.x && lo.x > 0.0) t1 = lo.x; else t1 = hi.y;
    }
    t1 = min(t1, t_max);

    float rain_end = (h_c < h_lo && !hits_ground) ? min(t0, t_max) : min(t_max, 60000.0);
    float4 rain = float4(0.0, 0.0, 0.0, 1.0);
    if (h_c < cl.cu.x + 500.0 || mu < 0.0) rain = rain_shafts(cl, shape, detail, ro, rd, h_c, mu, 0.0, rain_end, jitter);

    int steps = int(cl.misc.z);
    float step_scale = cl.shell.w;
    float3 sun = cl.sun.xyz;
    float cos_v = dot(rd, sun);
    float3 rd_n = to_noise(cl, rd);
    float3 up_n = to_noise(cl, up);
    float darkness = cl.st.w;
    float ph0 = phase(cos_v, 1.0);
    float ph1 = phase(cos_v, 0.5);
    float ph2 = phase(cos_v, 0.25);
    float ph3 = 1.0 / (4.0 * PI);
    float3 earth_c = up * -(PLANET_R + h_c);   // planet centre relative to the camera

    // STABLE: guarantee the whole span is covered inside the step budget; coarsen instead of truncating.
    float span = max(t1 - t0, 0.0);
    float min_dt = span / (float(steps) * 1.4);

    float T = 1.0;
    float3 col = float3(0.0);
    float t_weight = 0.0;
    float t = t0;
    int fine = 0;
    int i = 0;
    while (true) {
        if (i >= steps * 2 || t >= t1 || T < 0.015) break;
        i++;
        float base_dt = max((25.0 + t * 0.011) * step_scale, min_dt);
        float dt = fine == 0 ? base_dt * 3.0 : base_dt;
        float ts = t + dt * jitter;
        float h = altitude_at(h_c, mu, ts);
        float3 p = ro + rd * ts;
        float3 pn = cl.cam_noise.xyz + rd_n * ts;
        float3 pw = pn - up_n * h;   // STABLE: sea-level plane
        Smp s = density(cl, shape, detail, p, pn, pw, h, ts, true, dt);
        if (s.density <= 0.001) {
            if (fine > 0) fine--;
            t += dt;
            continue;
        }
        if (fine == 0) {
            fine = 5;
            t = max(t0, t - base_dt);
            continue;
        }
        fine = 5;
        float sigma_e = s.density * SIGMA;

        // STABLE: sun elevation at this sample's own vertical.
        float3 lup = normalize(rd * ts - earth_c);
        float mu_s = dot(sun, lup);
        float3 sun_t = sun_transmittance(h, mu_s);
        float3 sun_col = cl.sun.w * mix(sun_t, float3(dot(sun_t, float3(0.2126, 0.7152, 0.0722))), cl.planet.z);
        float od = light_depth(cl, shape, detail, p, pn, h, ts, mu_s);
        float ms = ph0 * exp(-od) + 0.8 * ph1 * exp(-od * 0.35) + 0.6 * ph2 * exp(-od * 0.15) + 0.5 * ph3 * exp(-od * 0.08);
        float powder = mix(1.0, 1.0 - exp(-sigma_e * 120.0 - od * 0.8), 0.5 * sat(0.6 - cos_v * 0.6));
        float3 direct = sun_col * ms * powder * 2.0;

        float hfrac = s.height_frac;
        float sky_vis = 0.25 + 0.75 * exp(-od * 0.1);
        float amb_mix = sat(hfrac * 1.1 + 0.35);
        float3 amb = mix(cl.ground_ambient.rgb, cl.sky_ambient.rgb * sky_vis, amb_mix) / PI;
        float belly = 1.0 - darkness * 0.8 * (1.0 - sat(hfrac * 1.4));
        amb *= belly;
        direct *= mix(1.0, belly, 0.5);
        float3 S = (direct + amb) * sigma_e * cl.planet.w;
        float Tstep = exp(-sigma_e * dt);
        col += T * (S - S * Tstep) / max(sigma_e, 1e-7);
        t_weight += ts * T * (1.0 - Tstep);
        T *= Tstep;
        t += dt;
    }

    float alpha = 1.0 - T;
    float k = V.proj.z;     // lux -> native HDR
    if (alpha > 0.0) {
        float tm = t_weight / max(alpha, 1e-5);
        t_mean = tm;
        float haze_dist = cl.shell.z * exp(h_c / 8000.0);
        float air = exp(-tm / haze_dist);
        // Haze is the native sky colour toward the horizon in this view direction, so cloud and terrain fog agree.
        float3 hd = rd; hd.y = max(hd.y, 0.10);
        float3 hz = sky_rad(normalize(hd), sun, V.sunv.w) * mix(1.0, 0.65, sat(mu * 3.0));
        col = col * k * air + hz * (1.0 - air) * alpha;
        // STABLE: soft fade at the reach limit instead of a hard cut.
        float edge = 1.0 - smoothstep(0.7, 1.0, tm / t_limit);
        col *= edge;
        alpha *= edge;
    } else {
        col = float3(0.0);
    }
    float3 rain_c = rain.rgb * k;
    if (h_c < h_lo && !hits_ground) col = rain_c + rain.a * col;
    else col = col + T * rain_c;
    alpha = 1.0 - (1.0 - alpha) * rain.a;
    if (alpha <= 0.0) return float4(0.0);
    return float4(col, alpha);
}

float3 pixel_ray(constant ViewU& V, float2 full_px) {
    float2 uv = full_px / V.full.xy;
    float2 ndc = float2(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    return normalize(V.fwd.xyz + V.right.xyz * ndc.x + V.up.xyz * ndc.y);
}

kernel void cloud_march(texture2d<float, access::write> out [[texture(0)]],
                        texture2d<float, access::write> aux [[texture(1)]],
                        depth2d<float> depth [[texture(2)]],
                        texture3d<float> shape [[texture(3)]],
                        texture3d<float> detail [[texture(4)]],
                        constant CloudU& cl [[buffer(0)]],
                        constant ViewU& V [[buffer(1)]],
                        uint2 gid [[thread_position_in_grid]]) {
    if (gid.x >= uint(V.size.x) || gid.y >= uint(V.size.y)) return;
    int2 full = int2(gid) * 2;
    int2 lim = int2(V.full.xy) - 1;
    float d00 = depth.read(uint2(min(full, lim)));
    float d10 = depth.read(uint2(min(full + int2(1, 0), lim)));
    float d01 = depth.read(uint2(min(full + int2(0, 1), lim)));
    float d11 = depth.read(uint2(min(full + int2(1, 1), lim)));
    float nearest = max(max(d00, d10), max(d01, d11));     // reverse-Z: larger = nearer
    float farthest = min(min(d00, d10), min(d01, d11));
    float d = (((gid.x + gid.y) & 1u) == 0u) ? nearest : farthest;
    float3 rd = pixel_ray(V, float2(full) + 1.0);
    float t_mean;
    float4 c = march(cl, V, shape, detail, gid, rd, d, t_mean);
    out.write(c, gid);
    aux.write(float4(d, t_mean, 0.0, 0.0), gid);
}

float lin_depth(float near_p, float d) { return d > 0.0 ? near_p / d : 1e9; }

kernel void cloud_resolve(texture2d<float> cur [[texture(0)]],
                          texture2d<float> cur_aux [[texture(1)]],
                          texture2d<float> hist [[texture(2)]],
                          texture2d<float> hist_aux [[texture(3)]],
                          texture2d<float, access::write> out [[texture(4)]],
                          texture2d<float, access::write> out_aux [[texture(5)]],
                          constant CloudU& cl [[buffer(0)]],
                          constant ViewU& V [[buffer(1)]],
                          uint2 gid [[thread_position_in_grid]]) {
    if (gid.x >= uint(V.size.x) || gid.y >= uint(V.size.y)) return;
    int2 hi = int2(V.size.xy) - 1;
    float4 c = cur.read(gid);
    float4 ca = cur_aux.read(gid);
    out_aux.write(ca, gid);

    if (V.proj.w > 0.5) { out.write(c, gid); return; }

    // 3x3 neighbourhood of the current frame: variance clip box for the history.
    float4 m1 = float4(0.0), m2 = float4(0.0);
    float4 mn = float4(1e9), mx = float4(-1e9);
    for (int y = -1; y <= 1; y++) {
        for (int x = -1; x <= 1; x++) {
            float4 s = cur.read(uint2(clamp(int2(gid) + int2(x, y), int2(0), hi)));
            m1 += s; m2 += s * s;
            mn = min(mn, s); mx = max(mx, s);
        }
    }
    m1 /= 9.0; m2 /= 9.0;
    float4 sd = sqrt(max(m2 - m1 * m1, 0.0));
    float4 lo_b = max(mn, m1 - 1.5 * sd), hi_b = min(mx, m1 + 1.5 * sd);

    // Reproject by the cloud-weighted hit distance (100 km for empty sky = rotation-only reprojection).
    float3 rd = pixel_ray(V, float2(gid) * 2.0 + 1.0);
    float3 wp = cl.cam_pos.xyz + rd * ca.y;
    float4 pc = V.prev_vp * float4(wp, 1.0);
    float4 res = c;
    if (pc.w > 1e-4) {
        float2 ndc = pc.xy / pc.w;
        float2 uvp = float2(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
        if (all(uvp > 0.0) && all(uvp < 1.0)) {
            int2 tp = clamp(int2(uvp * V.size.xy), int2(0), hi);
            float4 ha = hist_aux.read(uint2(tp));
            float a = lin_depth(V.proj.x, ca.x), b = lin_depth(V.proj.x, ha.x);
            float rel = abs(a - b) / max(min(a, b), 1.0);
            if (rel < 0.15) {
                float4 hv = hist.sample(hs, uvp);
                float4 hc = clamp(hv, lo_b, hi_b);
                float clipped = length(hv - hc) / (length(hv) + 1e-3);
                float w = mix(0.1, 0.35, sat(clipped * 2.0));
                res = mix(hc, c, w);
            }
        }
    }
    out.write(res, gid);
}

struct CompOut { float4 pos [[position]]; };
vertex CompOut cloud_comp_vs(uint vid [[vertex_id]]) {
    float2 p = float2((vid << 1) & 2, vid & 2);
    CompOut o; o.pos = float4(p * 2.0 - 1.0, 0.0, 1.0);
    return o;
}

fragment float4 cloud_comp_fs(CompOut in [[stage_in]],
                              texture2d<float> col [[texture(0)]],
                              texture2d<float> aux [[texture(1)]],
                              depth2d<float> depth [[texture(2)]],
                              constant ViewU& V [[buffer(1)]]) {
    int2 hi = int2(V.size.xy) - 1;
    uint2 px = uint2(in.pos.xy);
    float lf = lin_depth(V.proj.x, depth.read(px));
    float2 hp = in.pos.xy * 0.5 - 0.5;
    int2 i0 = int2(floor(hp));
    float2 f = hp - floor(hp);
    float4 acc = float4(0.0);
    float wsum = 0.0;
    float best_r = 1e9;
    float4 best = float4(0.0);
    for (int k = 0; k < 4; k++) {
        int2 o = int2(k & 1, k >> 1);
        int2 tp = clamp(i0 + o, int2(0), hi);
        float bw = (o.x == 1 ? f.x : 1.0 - f.x) * (o.y == 1 ? f.y : 1.0 - f.y);
        float4 a = aux.read(uint2(tp));
        float lm = lin_depth(V.proj.x, a.x);
        float r = abs(lm - lf) / max(min(lm, lf), 1.0);
        float4 c = col.read(uint2(tp));
        float w = bw * exp(-r * r * 300.0);
        acc += c * w; wsum += w;
        if (r < best_r) { best_r = r; best = c; }
    }
    return wsum > 1e-4 ? acc / wsum : best;
}
