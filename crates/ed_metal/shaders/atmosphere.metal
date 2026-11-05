// Atmosphere LUT compute kernels. Prefixed at build time with atmosphere_funcs.metal.
struct AtmoU {
    float4 sun_h;   // xyz = unit dir toward sun (world, up=+Y), w = camera height above ground in km
    float4 illum;   // rgb = sun illuminance at top of atmosphere
    float4x4 inv_vp; // reverse-Z inverse view-projection
};

// Source term S (in-scattered radiance per unit length at P toward -dir) for unit sun illuminance.
inline float3 at_S(float3 P, float3 dir, float3 sun, texture2d<float> trans, texture2d<float> ms,
                   thread float3& ext) {
    float r = length(P);
    float3 rs, e; float mie;
    at_medium(max(r - AT_BOT, 0.0), rs, mie, e);
    ext = e;
    float c = dot(dir, sun);
    float3 phase_scat = rs * at_rayleigh_phase(c) + mie * at_mie_phase(c);
    float shadow = at_ray_sphere(P, sun, AT_BOT) > 0.0 ? 0.0 : 1.0;
    float3 psi = ms.sample(AT_SMP, at_ms_uv(r, dot(P, sun) / r), level(0)).rgb;
    return shadow * at_sun_T(trans, P, sun) * phase_scat + psi * (rs + mie);
}

kernel void at_transmittance(texture2d<float, access::write> out [[texture(0)]], uint2 gid [[thread_position_in_grid]]) {
    if (gid.x >= AT_TRANS_RES.x || gid.y >= AT_TRANS_RES.y) return;
    float r, mu;
    at_trans_params((float2(gid) + 0.5) / float2(AT_TRANS_RES), r, mu);
    float3 P0 = float3(0.0, r, 0.0), dir = float3(sqrt(max(1.0 - mu * mu, 0.0)), mu, 0.0);
    float tmax = at_ray_sphere(P0, dir, AT_TOP);
    const int N = 64;
    float dt = tmax / N;
    float3 tau = 0.0;
    for (int i = 0; i < N; i++) {
        float3 P = P0 + dir * ((i + 0.5) * dt);
        float3 rs, e; float m;
        at_medium(max(length(P) - AT_BOT, 0.0), rs, m, e);
        tau += e * dt;
    }
    out.write(float4(exp(-tau), 1.0), gid);
}

kernel void at_multiscatter(texture2d<float, access::write> out [[texture(0)]], texture2d<float> trans [[texture(1)]],
                            uint2 gid [[thread_position_in_grid]]) {
    if (gid.x >= AT_MS_RES.x || gid.y >= AT_MS_RES.y) return;
    float2 uv = at_sub_to_unit((float2(gid) + 0.5) / float2(AT_MS_RES), float2(AT_MS_RES));
    float sc = uv.x * 2.0 - 1.0;
    float r = max(AT_BOT + saturate(uv.y) * (AT_TOP - AT_BOT), AT_BOT + 0.001);
    float3 P0 = float3(0.0, r, 0.0), sun = float3(sqrt(max(1.0 - sc * sc, 0.0)), sc, 0.0);
    const int ND = 64, NS = 20;
    float3 Lsum = 0.0, Fsum = 0.0;
    for (int i = 0; i < ND; i++) {
        float z = 1.0 - 2.0 * (i + 0.5) / ND, phi = i * 2.39996323, rxy = sqrt(max(1.0 - z * z, 0.0));
        float3 dir = float3(rxy * cos(phi), z, rxy * sin(phi));
        float tg = at_ray_sphere(P0, dir, AT_BOT);
        bool hit = tg > 0.0;
        float tmax = hit ? tg : at_ray_sphere(P0, dir, AT_TOP);
        float dt = tmax / NS;
        float3 L = 0.0, F = 0.0, thr = 1.0;
        for (int s = 0; s < NS; s++) {
            float3 P = P0 + dir * ((s + 0.5) * dt);
            float3 rs, e; float m;
            at_medium(max(length(P) - AT_BOT, 0.0), rs, m, e);
            float3 sc3 = rs + m, sT = exp(-e * dt);
            float shadow = at_ray_sphere(P, sun, AT_BOT) > 0.0 ? 0.0 : 1.0;
            float3 S = shadow * at_sun_T(trans, P, sun) * sc3 * (1.0 / (4.0 * AT_PI));
            F += thr * (sc3 - sc3 * sT) / e;
            L += thr * (S - S * sT) / e;
            thr *= sT;
        }
        if (hit) {
            float3 Pg = P0 + dir * tmax, N = normalize(Pg);
            L += thr * at_sun_T(trans, Pg, sun) * saturate(dot(N, sun)) * AT_GROUND_ALBEDO / AT_PI;
        }
        Lsum += L; Fsum += F;
    }
    float3 L2 = Lsum / ND, f = Fsum / ND;
    out.write(float4(L2 / (1.0 - f), 1.0), gid);
}

kernel void at_skyview(texture2d<float, access::write> out [[texture(0)]], texture2d<float> trans [[texture(1)]],
                       texture2d<float> ms [[texture(2)]], constant AtmoU& U [[buffer(0)]],
                       uint2 gid [[thread_position_in_grid]]) {
    if (gid.x >= AT_SKY_RES.x || gid.y >= AT_SKY_RES.y) return;
    float r = AT_BOT + max(U.sun_h.w, 0.001);
    float vz, lvc;
    at_sky_params(r, (float2(gid) + 0.5) / float2(AT_SKY_RES), vz, lvc);
    float sxy = sqrt(max(1.0 - vz * vz, 0.0));
    float3 dir = float3(sxy * lvc, vz, sxy * sqrt(max(1.0 - lvc * lvc, 0.0)));
    float sc = clamp(U.sun_h.y, -1.0, 1.0);
    float3 sun = float3(sqrt(max(1.0 - sc * sc, 0.0)), sc, 0.0);
    float3 P0 = float3(0.0, r, 0.0);
    float tg = at_ray_sphere(P0, dir, AT_BOT);
    bool hit = tg > 0.0;
    float tmax = hit ? tg : at_ray_sphere(P0, dir, AT_TOP);
    const int N = 48;
    float3 L = 0.0, thr = 1.0;
    for (int i = 0; i < N; i++) {
        float ta = tmax * float(i) * float(i) / (N * N), tb = tmax * float(i + 1) * float(i + 1) / (N * N);
        float dt = tb - ta;
        float3 P = P0 + dir * (0.5 * (ta + tb));
        float3 e;
        float3 S = at_S(P, dir, sun, trans, ms, e);
        float3 sT = exp(-e * dt);
        L += thr * (S - S * sT) / e;
        thr *= sT;
    }
    if (hit) {
        float3 Pg = P0 + dir * tmax, Ng = normalize(Pg);
        L += thr * at_sun_T(trans, Pg, sun) * saturate(dot(Ng, sun)) * AT_GROUND_ALBEDO / AT_PI;
    }
    out.write(float4(L * U.illum.rgb, (thr.r + thr.g + thr.b) / 3.0), gid);
}

kernel void at_aerial(texture3d<float, access::write> out [[texture(0)]], texture2d<float> trans [[texture(1)]],
                      texture2d<float> ms [[texture(2)]], constant AtmoU& U [[buffer(0)]],
                      uint2 gid [[thread_position_in_grid]]) {
    if (gid.x >= AT_AP_RES.x || gid.y >= AT_AP_RES.y) return;
    float2 uv = (float2(gid) + 0.5) / float2(AT_AP_RES.xy);
    float2 ndc = float2(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    float4 a = U.inv_vp * float4(ndc, 1.0, 1.0), b = U.inv_vp * float4(ndc, 0.5, 1.0);
    float3 dir = normalize(b.xyz / b.w - a.xyz / a.w);
    float r = AT_BOT + max(U.sun_h.w, 0.001);
    float3 P0 = float3(0.0, r, 0.0);
    float3 sun = U.sun_h.xyz;
    sun = float3(sqrt(max(1.0 - sun.y * sun.y, 0.0)), clamp(sun.y, -1.0, 1.0), 0.0); // frame where sun azimuth is +X
    // The LUTs are azimuth-symmetric about up, so rotate the sun into a plane and keep the true view dir/sun angle:
    float2 sh = U.sun_h.xz; float sl = length(sh);
    sh = sl > 1e-5 ? sh / sl : float2(1.0, 0.0);
    // rotate world dir so that world sun azimuth maps to +X
    float3 d = float3(dir.x * sh.x + dir.z * sh.y, dir.y, -dir.x * sh.y + dir.z * sh.x);
    float tTop = at_ray_sphere(P0, d, AT_TOP);
    float3 L = 0.0, thr = 1.0;
    float tprev = 0.0;
    for (uint k = 0; k < AT_AP_RES.z; k++) {
        float tend = min(at_ap_slice_dist_m(float(k)) * 0.001, tTop);
        const int NS = 3;
        float dt = max(tend - tprev, 0.0) / NS;
        for (int s = 0; s < NS; s++) {
            float3 P = P0 + d * (tprev + (s + 0.5) * dt);
            float3 e;
            float3 S = at_S(P, d, sun, trans, ms, e);
            float3 sT = exp(-e * dt);
            L += thr * (S - S * sT) / e;
            thr *= sT;
        }
        tprev = max(tend, tprev);
        out.write(float4(L * U.illum.rgb, (thr.r + thr.g + thr.b) / 3.0), uint3(gid, k));
    }
}
