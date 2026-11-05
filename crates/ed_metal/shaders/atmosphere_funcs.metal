// Atmosphere shared MSL (Hillaire 2020). Concatenate in front of any shader that wants to sample the sky LUTs.
// Units inside this file: kilometres, planet-centred, camera "up" = +Y. Luminance scale = sun illuminance given to the LUT pass.
#include <metal_stdlib>
using namespace metal;

constant float AT_BOT = 6360.0;
constant float AT_TOP = 6460.0;
constant float AT_PI = 3.14159265359;
constant float3 AT_RAY_S = float3(0.005802, 0.013558, 0.033100); // /km, density exp(-h/8)
constant float AT_MIE_S = 0.003996;                              // /km, density exp(-h/1.2)
constant float AT_MIE_E = 0.004440;
constant float3 AT_OZO_A = float3(0.000650, 0.001881, 0.000085); // tent density around 25 km
constant float AT_MIE_G = 0.8;
constant float3 AT_GROUND_ALBEDO = float3(0.3);
constant float AT_SUN_RADIUS = 0.004675; // rad (0.268 deg)

constant uint2 AT_TRANS_RES = uint2(256, 64);
constant uint2 AT_MS_RES = uint2(32, 32);
constant uint2 AT_SKY_RES = uint2(192, 108);
constant uint3 AT_AP_RES = uint3(32, 32, 32);
constant float AT_AP_MAX_M = 64000.0;

constexpr sampler AT_SMP(filter::linear, address::clamp_to_edge);

inline float2 at_unit_to_sub(float2 u, float2 res) { return (u + 0.5 / res) * (res / (res + 1.0)); }
inline float2 at_sub_to_unit(float2 u, float2 res) { return (u - 0.5 / res) * (res / (res - 1.0)); }

inline float at_ray_sphere(float3 o, float3 d, float r) {
    float b = dot(o, d), c = dot(o, o) - r * r, disc = b * b - c;
    if (disc < 0.0) return -1.0;
    float s = sqrt(disc), t0 = -b - s, t1 = -b + s;
    return t0 > 0.0 ? t0 : (t1 > 0.0 ? t1 : -1.0);
}

inline float at_rayleigh_phase(float c) { return 3.0 / (16.0 * AT_PI) * (1.0 + c * c); }
inline float at_mie_phase(float c) {
    float g = AT_MIE_G, k = 3.0 / (8.0 * AT_PI) * (1.0 - g * g) / (2.0 + g * g);
    return k * (1.0 + c * c) / pow(max(1.0 + g * g - 2.0 * g * c, 1e-4), 1.5);
}

// Medium at altitude h (km): Rayleigh scattering, Mie scattering, total extinction.
inline void at_medium(float h, thread float3& rs, thread float& ms, thread float3& ext) {
    float dr = exp(-h / 8.0), dm = exp(-h / 1.2), dz = max(0.0, 1.0 - abs(h - 25.0) / 15.0);
    rs = AT_RAY_S * dr;
    ms = AT_MIE_S * dm;
    ext = rs + AT_MIE_E * dm + AT_OZO_A * dz;
}

// ---- Transmittance LUT (256x64): u = view-angle distance, v = height.
inline float2 at_trans_uv(float r, float mu) {
    r = clamp(r, AT_BOT, AT_TOP);
    float H = sqrt(AT_TOP * AT_TOP - AT_BOT * AT_BOT);
    float rho = sqrt(max(0.0, r * r - AT_BOT * AT_BOT));
    float disc = r * r * (mu * mu - 1.0) + AT_TOP * AT_TOP;
    float d = max(0.0, -r * mu + sqrt(max(0.0, disc)));
    float dmin = AT_TOP - r, dmax = rho + H;
    return at_unit_to_sub(float2((d - dmin) / (dmax - dmin), rho / H), float2(AT_TRANS_RES));
}
inline void at_trans_params(float2 uv, thread float& r, thread float& mu) {
    uv = at_sub_to_unit(uv, float2(AT_TRANS_RES));
    float H = sqrt(AT_TOP * AT_TOP - AT_BOT * AT_BOT);
    float rho = H * uv.y;
    r = sqrt(rho * rho + AT_BOT * AT_BOT);
    float dmin = AT_TOP - r, dmax = rho + H;
    float d = dmin + uv.x * (dmax - dmin);
    mu = d == 0.0 ? 1.0 : clamp((H * H - rho * rho - d * d) / (2.0 * r * d), -1.0, 1.0);
}
// Transmittance from planet-centred point P along direction with cosine mu = dot(up, dir).
inline float3 at_sun_T(texture2d<float> trans, float3 P, float3 sun) {
    float r = length(P);
    return trans.sample(AT_SMP, at_trans_uv(r, dot(P, sun) / r), level(0)).rgb;
}

// ---- Multiple-scattering LUT (32x32): u = sun cos, v = height.
inline float2 at_ms_uv(float r, float sun_cos) {
    return at_unit_to_sub(float2(sun_cos * 0.5 + 0.5, saturate((r - AT_BOT) / (AT_TOP - AT_BOT))), float2(AT_MS_RES));
}

// ---- Sky-view LUT (192x108): u = azimuth to sun (sqrt), v = non-linear latitude, horizon at v = 0.5.
inline float2 at_sky_uv(float r, float vz, float lvc) {
    float vh = sqrt(max(r * r - AT_BOT * AT_BOT, 0.0));
    float cosb = vh / r, beta = acos(cosb), zh = AT_PI - beta;
    float2 uv;
    if (vz >= -cosb) {
        float c = acos(clamp(vz, -1.0, 1.0)) / zh;
        uv.y = (1.0 - sqrt(max(1.0 - c, 0.0))) * 0.5;
    } else {
        float c = (acos(clamp(vz, -1.0, 1.0)) - zh) / beta;
        uv.y = sqrt(max(c, 0.0)) * 0.5 + 0.5;
    }
    uv.x = sqrt(saturate(-lvc * 0.5 + 0.5));
    return at_unit_to_sub(uv, float2(AT_SKY_RES));
}
inline void at_sky_params(float r, float2 uv, thread float& vz, thread float& lvc) {
    uv = at_sub_to_unit(uv, float2(AT_SKY_RES));
    float vh = sqrt(max(r * r - AT_BOT * AT_BOT, 0.0));
    float cosb = vh / r, beta = acos(cosb), zh = AT_PI - beta;
    if (uv.y < 0.5) {
        float c = 1.0 - 2.0 * uv.y; c = 1.0 - c * c;
        vz = cos(zh * c);
    } else {
        float c = uv.y * 2.0 - 1.0; c *= c;
        vz = cos(zh + beta * c);
    }
    lvc = -(uv.x * uv.x * 2.0 - 1.0);
}

// ---- Aerial-perspective froxels (32^3): xy = screen uv, z = sqrt(dist / 64 km), voxel k holds distance 64 km * ((k+1)/32)^2.
inline float at_ap_slice_dist_m(float k) { float w = (k + 1.0) / float(AT_AP_RES.z); return AT_AP_MAX_M * w * w; }

// ======================= Public helpers =======================

// Sky radiance (already scaled by the sun illuminance) looking along world dir. cam_h_m = camera height above ground (m).
inline float3 sky_radiance(float3 dir, float cam_h_m, float3 sun_dir, texture2d<float> sky_view) {
    float r = AT_BOT + max(cam_h_m * 0.001, 0.001);
    float2 h = dir.xz, s = sun_dir.xz;
    float hl = length(h), sl = length(s);
    h = hl > 1e-5 ? h / hl : float2(1.0, 0.0);
    s = sl > 1e-5 ? s / sl : float2(1.0, 0.0);
    return sky_view.sample(AT_SMP, at_sky_uv(r, clamp(dir.y, -1.0, 1.0), dot(h, s)), level(0)).rgb;
}

// Sun disc radiance with limb darkening, attenuated by the atmosphere; zero below the horizon.
inline float3 sun_disk(float3 dir, float3 sun_dir, float3 sun_illum, float cam_h_m, texture2d<float> trans) {
    float c = dot(dir, sun_dir);
    if (c <= 0.0) return float3(0.0);
    float ang = length(cross(dir, sun_dir)); // ~ angle for small angles
    float mask = saturate((AT_SUN_RADIUS - ang) / (0.06 * AT_SUN_RADIUS));
    if (mask <= 0.0) return float3(0.0);
    float r = AT_BOT + max(cam_h_m * 0.001, 0.001);
    float3 P = float3(0.0, r, 0.0);
    if (at_ray_sphere(P, dir, AT_BOT) > 0.0) return float3(0.0);
    float3 T = trans.sample(AT_SMP, at_trans_uv(r, dir.y), level(0)).rgb;
    float x = min(ang / AT_SUN_RADIUS, 1.0);
    float mu = sqrt(max(1.0 - x * x, 0.0));
    float3 u = float3(0.45, 0.60, 0.75); // per-channel limb-darkening strength, normalised to unit disc average
    float3 limb = (1.0 - u * (1.0 - mu)) / (1.0 - u / 3.0);
    float solid = AT_PI * AT_SUN_RADIUS * AT_SUN_RADIUS;
    return sun_illum / solid * T * limb * mask;
}

// Aerial perspective for screen uv (0..1, y down) and view-ray distance dist_m (metres from camera).
// Returns (in-scatter luminance rgb, transmittance a). Apply as: color = color * ap.a + ap.rgb.
inline float4 aerial_perspective(float2 uv, float dist_m, texture3d<float> ap) {
    float w = sqrt(saturate(dist_m / AT_AP_MAX_M));
    float z = w - 0.5 / float(AT_AP_RES.z);
    float4 v = ap.sample(AT_SMP, float3(uv, z), level(0));
    float d0 = at_ap_slice_dist_m(0.0);
    float f = saturate(dist_m / d0); // fade in over the first slice (voxel 0 holds d0)
    return float4(v.rgb * (dist_m < d0 ? f : 1.0), mix(1.0, v.a, dist_m < d0 ? f : 1.0));
}
