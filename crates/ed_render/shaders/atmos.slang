// Physically based atmosphere (Phase 2 W5, P2-D10), the shared part: medium, geometry, table
// lookups and the per-pixel terms resolve uses (sky from the sky-view table, sun light at a
// shaded point, aerial perspective from the froxel volume). The tables are built by
// atmos_tables.slang; the CPU mirror of every function here is src/atmosphere_model.rs.
//
// Model (Hillaire 2020, "A Scalable and Production Ready Sky and Atmosphere Rendering
// Technique"): Rayleigh + Mie (Cornette-Shanks phase) + ozone absorption, a transmittance table,
// a multiple-scattering table (isotropic second order summed as a geometric series), a sky-view
// table per view and an aerial-perspective froxel volume with exponential depth slices.
//
// Units: kilometres for lengths, 1/km for coefficients; radiance in the renderer's units (the
// sun is `f.atmos.sun.rgb` at the top of the atmosphere). All geometry is written in (h, mu)
// form: h is the altitude above the body's surface and mu a cosine to the local up, and every
// expression that would cancel near the ground (r^2 - rb^2 at r = rb + 1.7 m) is written with h
// so a ground camera keeps its precision in f32.
//
// Tables live in bindless float4 buffers (FrameData.atmos.bufs*), edge-inclusive: texel i of n
// sits at coordinate i / (n - 1), and lookups filter bilinearly by hand.
#pragma once

static const float ATM_PI = 3.14159265;
static const uint ATM_TRANS_W = 256;
static const uint ATM_TRANS_H = 64;
static const uint ATM_MS_N = 32;
static const uint ATM_SKY_W = 192;
static const uint ATM_SKY_H = 108;   // two halves: rows 0..53 above the horizon, 54..107 below
static const uint ATM_SURF_W = 96;
static const uint ATM_SURF_H = 54;
static const uint ATM_AP_N = 32;     // froxels per axis (x, y, depth slices)
static const uint ATM_SH_OUT = 288;  // ambient SH buffer: 32 x 9 partial sums, then the 9 coefficients

float atm_rb(FrameData* f) { return f.atmos.radii.x; }
float atm_rt(FrameData* f) { return f.atmos.radii.y; }

struct AtmMedium {
    float3 ray;     // Rayleigh scattering
    float3 mie;     // Mie scattering
    float3 scat;    // total scattering
    float3 ext;     // extinction
};

AtmMedium atm_medium(FrameData* f, float h) {
    h = max(h, 0.0);
    float rd = exp(-h / f.atmos.rayleigh.w);
    float md = exp(-h / f.atmos.mie_s.w);
    float od = max(0.0, 1.0 - abs(h - f.atmos.ozone.w) / f.atmos.ground.w);
    AtmMedium m;
    m.ray = f.atmos.rayleigh.rgb * rd;
    m.mie = f.atmos.mie_s.rgb * md;
    m.scat = m.ray + m.mie;
    m.ext = m.scat + f.atmos.mie_a.rgb * md + f.atmos.ozone.rgb * od;
    return m;
}

// Altitude at distance t along a ray leaving altitude h with zenith cosine mu:
// (r_t^2 - rb^2) / (r_t + rb), exact near the ground.
float atm_height_at(float rb, float h, float mu, float t) {
    float r = rb + h;
    float q = t * t + 2.0 * r * mu * t + h * (2.0 * rb + h);
    return q / (sqrt(max(rb * rb + q, 0.0)) + rb);
}

bool atm_hits_ground(float rb, float h, float mu) {
    float rm = (rb + h) * mu;
    return mu < 0.0 && rm * rm >= h * (2.0 * rb + h);
}

// Distance to the ground along a ray that hits it (conjugate form: no cancellation).
float atm_dist_ground(float rb, float h, float mu) {
    float a = -(rb + h) * mu;
    float c = h * (2.0 * rb + h);
    return c / max(a + sqrt(max(a * a - c, 0.0)), 1e-12);
}

// Distance to the top of the atmosphere from inside it (h <= rt - rb).
float atm_dist_top(float rb, float rt, float h, float mu) {
    float r = rb + h;
    float d = (r * mu) * (r * mu) + ((rt - rb) - h) * (rt + r);
    return max(-r * mu + sqrt(max(d, 0.0)), 0.0);
}

// From outside the atmosphere: distance to where the ray enters it, or -1 if it misses.
float atm_enter_top(float rb, float rt, float h, float mu) {
    float r = rb + h;
    float d = (r * mu) * (r * mu) - (h - (rt - rb)) * (r + rt);
    if (d < 0.0) {
        return -1.0;
    }
    float t = -r * mu - sqrt(d);
    return t >= 0.0 ? t : -1.0;
}

// Cosine of the horizon's zenith angle at altitude h (below -0 for h > 0).
float atm_cos_horizon(float rb, float h) {
    return -sqrt(max(h * (2.0 * rb + h), 0.0)) / (rb + h);
}

float atm_phase_rayleigh(float nu) {
    return 3.0 / (16.0 * ATM_PI) * (1.0 + nu * nu);
}

// Cornette-Shanks.
float atm_phase_mie(float g, float nu) {
    float k = 3.0 / (8.0 * ATM_PI) * (1.0 - g * g) / (2.0 + g * g);
    return k * (1.0 + nu * nu) / pow(max(1.0 + g * g - 2.0 * g * nu, 1e-4), 1.5);
}

float4 atm_bilinear(float4* buf, uint w, uint row0, uint rows, float2 uv) {
    float2 p = saturate(uv) * float2(float(w - 1u), float(rows - 1u));
    uint2 i = min(uint2(p), uint2(w - 2u, rows - 2u));
    float2 g = p - float2(i);
    uint b = (row0 + i.y) * w + i.x;
    float4 a = lerp(buf[b], buf[b + 1u], g.x);
    float4 c = lerp(buf[b + w], buf[b + w + 1u], g.x);
    return lerp(a, c, g.y);
}

// Transmittance table coordinates (Bruneton 2017): u = where the distance to the top lies
// between its minimum and its horizon value, v = rho / H.
float2 atm_trans_uv(float rb, float rt, float h, float mu) {
    float ht = rt - rb;
    h = clamp(h, 0.0, ht);
    float big_h = sqrt((rt - rb) * (rt + rb));
    float rho = sqrt(max(h * (2.0 * rb + h), 0.0));
    float d = atm_dist_top(rb, rt, h, mu);
    float dmin = ht - h;
    float dmax = rho + big_h;
    return float2((d - dmin) / max(dmax - dmin, 1e-6), rho / big_h);
}

// Transmittance from altitude h towards mu up to the top of the atmosphere.
float3 atm_transmittance(FrameData* f, float h, float mu) {
    float2 uv = atm_trans_uv(atm_rb(f), atm_rt(f), h, mu);
    return atm_bilinear(ed_buf<float4>(f.atmos.bufs.x), ATM_TRANS_W, 0u, ATM_TRANS_H, uv).rgb;
}

// Share of the sun's disc above the horizon at altitude h (the Earth's shadow).
float atm_earth_shadow(FrameData* f, float h, float mu_s) {
    float ch = atm_cos_horizon(atm_rb(f), h);
    float s = 0.0047 * sqrt(max(1.0 - ch * ch, 0.0)) + 1e-5;
    return smoothstep(-s, s, mu_s - ch);
}

float3 atm_sun_transmittance(FrameData* f, float h, float mu_s) {
    return atm_transmittance(f, h, mu_s) * atm_earth_shadow(f, h, mu_s);
}

// Multiple-scattering radiance per unit sun irradiance (Hillaire's Psi_ms).
float3 atm_multiscatter(FrameData* f, float h, float mu_s) {
    float2 uv = float2(mu_s * 0.5 + 0.5, h / (atm_rt(f) - atm_rb(f)));
    return atm_bilinear(ed_buf<float4>(f.atmos.bufs.y), ATM_MS_N, 0u, ATM_MS_N, uv).rgb;
}

struct AtmAcc {
    float3 l;   // in-scattered radiance
    float3 t;   // transmittance so far
};

// One ray segment of length dt with its midpoint at tm, from the origin (h0, mu, mu_s, nu).
// Analytic integration of the in-scattering over the segment (Hillaire 2015).
void atm_segment(FrameData* f, inout AtmAcc a, float h0, float mu, float mu_s, float nu, float tm, float dt, float pr, float pm) {
    float rb = atm_rb(f);
    float h = atm_height_at(rb, h0, mu, tm);
    float mus = clamp(((rb + h0) * mu_s + tm * nu) / (rb + h), -1.0, 1.0);
    AtmMedium m = atm_medium(f, h);
    float3 st = exp(-m.ext * dt);
    float3 s = f.atmos.sun.rgb * (atm_sun_transmittance(f, h, mus) * (m.ray * pr + m.mie * pm) + atm_multiscatter(f, h, mus) * m.scat);
    a.l += a.t * (s - s * st) / max(m.ext, float3(1e-9));
    a.t *= st;
}

// Radiance leaving the ground (Lambert) at the point where a ray from (h0, mu) lands after t:
// the sun through the atmosphere plus the skylight, approximated as pi times the
// multiple-scattering radiance there (Psi_ms is the mean radiance over the sphere).
float3 atm_ground_radiance(FrameData* f, float h0, float mu_s, float nu, float t) {
    float rb = atm_rb(f);
    float mus = clamp(((rb + h0) * mu_s + t * nu) / rb, -1.0, 1.0);
    float3 e = atm_sun_transmittance(f, 0.0, mus) * (saturate(mus) * f.atmos.ap.z) + atm_multiscatter(f, 0.0, mus) * ATM_PI;
    return f.atmos.sun.rgb * e * f.atmos.ground.rgb / ATM_PI;
}

// Moves a ray origin above the atmosphere to where the ray enters it. Returns false when the
// ray misses the atmosphere; `te` is the distance moved.
bool atm_enter(FrameData* f, inout float h, inout float mu, inout float mu_s, float nu, out float te) {
    te = 0.0;
    float rb = atm_rb(f);
    float rt = atm_rt(f);
    if (h <= rt - rb) {
        return true;
    }
    te = atm_enter_top(rb, rt, h, mu);
    if (te < 0.0) {
        return false;
    }
    float r = rb + h;
    mu = clamp((r * mu + te) / rt, -1.0, 1.0);
    mu_s = clamp((r * mu_s + te * nu) / rt, -1.0, 1.0);
    h = rt - rb;
    return true;
}

// Sky radiance along a ray from altitude h (sky-view table entries): `steps` segments,
// quadratic in distance, plus the lit ground where the ray lands.
float3 atm_sky_ray(FrameData* f, float h, float mu, float mu_s, float nu, uint steps) {
    float te;
    if (!atm_enter(f, h, mu, mu_s, nu, te)) {
        return float3(0.0);
    }
    float rb = atm_rb(f);
    bool ground = atm_hits_ground(rb, h, mu);
    float tmax = ground ? atm_dist_ground(rb, h, mu) : atm_dist_top(rb, atm_rt(f), h, mu);
    float pr = atm_phase_rayleigh(nu);
    float pm = atm_phase_mie(f.atmos.mie_a.w, nu);
    AtmAcc a;
    a.l = float3(0.0);
    a.t = float3(1.0);
    float prev = 0.0;
    for (uint i = 0u; i < steps; i++) {
        float x = float(i + 1u) / float(steps);
        float t1 = tmax * x * x;
        atm_segment(f, a, h, mu, mu_s, nu, 0.5 * (prev + t1), t1 - prev, pr, pm);
        prev = t1;
    }
    if (ground) {
        a.l += a.t * atm_ground_radiance(f, h, mu_s, nu, tmax);
    }
    return a.l;
}

// Sky-view table lookup for direction `d` (world axes) from altitude h: zenith angle mapped
// non-linearly around the horizon (each half of the table on its side of it, so the horizon
// line is never blurred across), azimuth from the sun as sqrt(phi / pi).
float3 atm_sky_view(FrameData* f, uint buf, uint w, uint rows, float h, float3 d) {
    float mu = clamp(dot(d, f.atmos.up.xyz), -1.0, 1.0);
    float mu_s = f.atmos.up.w;
    float nu = dot(d, f.sun_dir.xyz);
    float sv = sqrt(max(1.0 - mu * mu, 0.0));
    float ss = sqrt(max(1.0 - mu_s * mu_s, 0.0));
    float cphi = sv * ss > 1e-6 ? clamp((nu - mu * mu_s) / (sv * ss), -1.0, 1.0) : 1.0;
    float u = sqrt(acos(cphi) / ATM_PI);
    float zha = acos(atm_cos_horizon(atm_rb(f), h));
    float theta = acos(mu);
    uint half_rows = rows / 2u;
    float c;
    uint row0;
    if (theta < zha) {
        c = 1.0 - sqrt(saturate(1.0 - theta / zha));
        row0 = 0u;
    } else {
        c = sqrt(saturate((theta - zha) / max(ATM_PI - zha, 1e-6)));
        row0 = half_rows;
    }
    return atm_bilinear(ed_buf<float4>(buf), w, row0, half_rows, float2(u, c)).rgb;
}

// The camera's sky (no sun disc) towards `d`: the sky itself, and the specular environment.
float3 atmos_sky(FrameData* f, float3 d) {
    return atm_sky_view(f, f.atmos.bufs.z, ATM_SKY_W, ATM_SKY_H, f.atmos.radii.z, d);
}

// Sky pixel: the sky-view table plus the sun's disc seen through the atmosphere, its radiance
// the irradiance over the disc's solid angle with limb darkening (capped below the RGBA16F
// range), times `disc_k`.
float3 atmos_sky_pixel(FrameData* f, float3 d, float disc_k) {
    float3 c = atmos_sky(f, d);
    float nu = dot(d, f.sun_dir.xyz);
    float cr = f.atmos.sun.w;
    if (nu > cr - 2e-6) {
        float h = f.atmos.radii.z;
        float mu = dot(d, f.atmos.up.xyz);
        float mu_s = f.atmos.up.w;
        float te;
        // A ray that misses the atmosphere misses the body too: the sun unattenuated.
        float3 tr = float3(1.0);
        if (atm_enter(f, h, mu, mu_s, nu, te)) {
            tr = atm_hits_ground(atm_rb(f), h, mu) ? float3(0.0) : atm_transmittance(f, h, mu);
        }
        float sr2 = 1.0 - cr * cr;  // sin^2 of the angular radius
        float x = saturate((1.0 - nu * nu) / sr2);  // (angle / radius)^2
        float limb = 1.0 - 0.6 * (1.0 - sqrt(1.0 - x));
        float edge = smoothstep(cr - 2e-6, cr + 2e-6, nu);
        float3 disc = f.atmos.sun.rgb * tr / (ATM_PI * sr2) * limb * edge * disc_k;
        c += min(disc, float3(30000.0));
    }
    return c;
}

// Sun irradiance reaching camera-relative point p (m): through the atmosphere to p's altitude,
// with the Earth's shadow.
float3 atmos_sun_at(FrameData* f, float3 p) {
    float t = length(p) * 0.001;
    float3 d = t > 0.0 ? p * (0.001 / t) : f.atmos.up.xyz;
    float rb = atm_rb(f);
    float h0 = f.atmos.radii.z;
    float mu = dot(d, f.atmos.up.xyz);
    float h = atm_height_at(rb, h0, mu, t);
    float mu_s = clamp(((rb + h0) * f.atmos.up.w + t * dot(d, f.sun_dir.xyz)) / (rb + h), -1.0, 1.0);
    return f.atmos.sun.rgb * atm_sun_transmittance(f, h, mu_s);
}

// Ambient blend weight for point p: 0 at the surface (surface sky table), 1 at the camera's
// altitude and above (camera table).
float atmos_ambient_weight(FrameData* f, float3 p) {
    float t = length(p) * 0.001;
    float h0 = f.atmos.radii.z;
    float mu = t > 0.0 ? dot(p * (0.001 / t), f.atmos.up.xyz) : 1.0;
    float h = atm_height_at(atm_rb(f), h0, mu, t);
    float hs = f.atmos.radii.w;
    return saturate((h - hs) / max(h0 - hs, 1e-3));
}

// Froxel depth slice coordinate (0..N) of distance t (km) past the entry offset.
float atm_ap_slice(FrameData* f, float t) {
    float n = f.atmos.ap.x;
    return log2(1.0 + max(t, 0.0) / n) / log2(1.0 + f.atmos.ap.y / n) * float(ATM_AP_N);
}

// Depth (km past the entry offset) where slice k ends.
float atm_ap_depth(FrameData* f, uint k) {
    float n = f.atmos.ap.x;
    return n * (exp2(float(k + 1u) / float(ATM_AP_N) * log2(1.0 + f.atmos.ap.y / n)) - 1.0);
}

void atm_froxel_xy(float4* l, float4* t, float2 g, uint2 i, uint k, out float3 lo, out float3 to) {
    uint b = (k * ATM_AP_N + i.y) * ATM_AP_N + i.x;
    uint n = ATM_AP_N;
    float4 l0 = lerp(l[b], l[b + 1u], g.x);
    float4 l1 = lerp(l[b + n], l[b + n + 1u], g.x);
    float4 t0 = lerp(t[b], t[b + 1u], g.x);
    float4 t1 = lerp(t[b + n], t[b + n + 1u], g.x);
    lo = lerp(l0, l1, g.y).rgb;
    to = lerp(t0, t1, g.y).rgb;
}

// Aerial perspective at camera-relative point p (m) seen through pixel `px`: (in-scattered
// radiance, transmittance) from the froxel volume, trilinear.
void atmos_aerial(FrameData* f, float3 p, float2 px, out float3 l, out float3 tr) {
    l = float3(0.0);
    tr = float3(1.0);
    float t = length(p) * 0.001;
    if (t <= 0.0) {
        return;
    }
    float te = 0.0;
    float h = f.atmos.radii.z;
    float rb = atm_rb(f);
    float rt = atm_rt(f);
    if (h > rt - rb) {
        te = atm_enter_top(rb, rt, h, dot(p * (0.001 / t), f.atmos.up.xyz));
        if (te < 0.0 || t <= te) {
            return;
        }
    }
    float s = atm_ap_slice(f, t - te);
    float2 q = clamp(px / f.viewport.zw * float(ATM_AP_N) - 0.5, float2(0.0), float2(float(ATM_AP_N - 1u)));
    uint2 i = min(uint2(q), uint2(ATM_AP_N - 2u));
    float2 g = q - float2(i);
    float4* lb = ed_buf<float4>(f.atmos.bufs2.x);
    float4* tb = ed_buf<float4>(f.atmos.bufs2.y);
    float3 la, ta, lc, tc;
    if (s < 1.0) {
        // Between the camera (nothing) and the end of the first slice.
        atm_froxel_xy(lb, tb, g, i, 0u, lc, tc);
        l = lc * s;
        tr = lerp(float3(1.0), tc, s);
        return;
    }
    float sk = min(s - 1.0, float(ATM_AP_N - 1u));
    uint k = min(uint(sk), ATM_AP_N - 2u);
    float gz = sk - float(k);
    atm_froxel_xy(lb, tb, g, i, k, la, ta);
    atm_froxel_xy(lb, tb, g, i, k + 1u, lc, tc);
    l = lerp(la, lc, gz);
    tr = lerp(ta, tc, gz);
}

// ---- Resolve-facing switches: the atmosphere when FrameData.atmos.cfg.x is set, the analytic
// sky model (common.slang `sky_rad`, `sun_color`) otherwise (EW_NO_ATMOS=1).

bool atmos_on(FrameData* f) {
    return f.atmos.cfg.x != 0u;
}

// Sky pixel (nothing drawn there).
float3 sky_pixel(FrameData* f, float3 d) {
    if (!atmos_on(f)) {
        return sky_rad(f, d, f.light.sky.x);
    }
    if (f.atmos.cfg.y == 1u) {
        // Debug: transmittance of the view ray to the top of the atmosphere (0 into the ground).
        float h = f.atmos.radii.z;
        float mu = dot(d, f.atmos.up.xyz);
        float mu_s = f.atmos.up.w;
        float te;
        if (!atm_enter(f, h, mu, mu_s, dot(d, f.sun_dir.xyz), te)) {
            return float3(0.5);
        }
        return atm_hits_ground(atm_rb(f), h, mu) ? float3(0.0) : atm_transmittance(f, h, mu) * 0.5;
    }
    return atmos_sky_pixel(f, d, f.atmos.cfg.y == 2u ? 0.0 : f.light.sky.x);
}

// Sky radiance towards `d` without the sun disc (specular environment).
float3 sky_env(FrameData* f, float3 d) {
    return atmos_on(f) ? atmos_sky(f, d) : sky_rad(f, d, 0.0);
}

// Sun irradiance at camera-relative point p.
float3 sun_light_at(FrameData* f, float3 p) {
    return atmos_on(f) ? atmos_sun_at(f, p) : f.sun_color.rgb;
}

// Aerial perspective over a shaded opaque pixel (and the debug views).
float3 apply_aerial(FrameData* f, float3 col, float3 p, float2 px) {
    if (!atmos_on(f)) {
        return col;
    }
    float3 l;
    float3 tr;
    atmos_aerial(f, p, px, l, tr);
    if (f.atmos.cfg.y == 1u) {
        return tr * 0.5;
    }
    if (f.atmos.cfg.y == 2u) {
        return l;
    }
    return col * tr + l;
}
