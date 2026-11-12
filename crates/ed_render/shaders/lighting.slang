// Baseline lighting terms read by resolve (M4d): sky ambient as SH9, cascaded sun shadows with
// a tent PCF and cascade blending, and the AO upsample. The CPU side (cascade fit, SH bake,
// switches) is src/light.rs; the data is FrameData.light (common.slang `LightData`).
#pragma once
#include "atmos.slang"

// Diffuse ambient light (skylight above, ground bounce below: light.rs `ambient_sh9`, or with
// the atmosphere on the surface sky-view table, atmos_tables.slang `atmos_sh_final`) reaching
// a surface with normal `n`, divided by pi (multiply by albedo).
// The SH9 coefficients already include the basis constants and the cosine lobe.
float3 sh_irradiance(FrameData* f, float3 n) {
    float4 s[9];
    float4* ab = ed_buf<float4>(f.atmos.bufs2.z);
    for (uint k = 0u; k < 9u; k++) {
        s[k] = atmos_on(f) ? ab[ATM_SH_OUT + k] : f.light.sh[k];
    }
    float3 c = s[0].rgb;
    c += s[1].rgb * n.y + s[2].rgb * n.z + s[3].rgb * n.x;
    c += s[4].rgb * (n.x * n.y) + s[5].rgb * (n.y * n.z) + s[6].rgb * (3.0 * n.z * n.z - 1.0);
    c += s[7].rgb * (n.x * n.z) + s[8].rgb * (n.x * n.x - n.y * n.y);
    return max(c, float3(0.0));
}

// Fallback ambient when the sky SH is off (--no-sky-ambient): the sky model's zenith/horizon
// average above, the ground colour below.
float3 hemi_ambient(FrameData* f, float3 n) {
    float t = n.y * 0.5 + 0.5;
    float3 sky = lerp(f.sky_horizon.rgb, f.sky_zenith.rgb, 0.5) * f.sun_dir.w;
    return lerp(f.ground_color.rgb * f.sun_dir.w, sky, t);
}

float3 ambient_light(FrameData* f, float3 n) {
    return f.light.flags.z != 0 ? sh_irradiance(f, n) : hemi_ambient(f, n);
}

// Filter radius in texels of cascade `c`: the world radius f.light.sky.z (m) divided by the
// cascade's texel size, clamped to [KMIN, f.light.sky.w]. Mirrors light.rs `filter_texels`, so
// penumbra width is the same in metres in every cascade until the texel size (far cascades)
// or the tap budget (near ones) clamps it.
static const float PCF_KMIN = 1.0;
float pcf_radius_texels(FrameData* f, float texel) {
    return clamp(f.light.sky.z / texel, PCF_KMIN, max(f.light.sky.w, PCF_KMIN));
}

// One cascade lookup: a 5x5 grid of point taps over a tent of radius k texels (k from
// pcf_radius_texels, so constant in metres), the grid jittered per pixel by up to one tap
// spacing so a wide kernel turns banding into fine noise. Compared in light depth. `p` is the
// camera-relative position, `ng` the geometric normal facing the viewer, `px` the pixel for the
// jitter. The normal offset grows with the receiver's slope to the sun: a plane tilted by theta
// drops tan(theta) texels of depth per texel, so a kernel of radius k needs about k sin(theta)
// texels of offset along the normal on top of the constant bias.
float cascade_lookup(FrameData* f, uint atlas, uint c, float3 p, float3 ng, float2 px) {
    float4 cs = f.light.cascade[c];
    float tile = f.light.cfg.y;
    float k = pcf_radius_texels(f, cs.x);
    float sp = k * 0.5;
    float nol = saturate(dot(ng, f.sun_dir.xyz));
    float slope = sqrt(1.0 - nol * nol);
    float3 q = p + ng * (cs.x * (f.light.filter.z + 2.0 * slope));
    float4 s = mul(f.light.shadow[c], float4(q, 1.0));
    float2 uv = float2(s.x * 0.5 + 0.5, 0.5 - s.y * 0.5) * tile;
    float ign = frac(52.9829189 * frac(dot(px, float2(0.06711056, 0.00583715))));
    float ign2 = frac(ign * 7.31 + 0.37);
    float2 jit = (float2(ign, ign2) - 0.5) * 0.0;
    // Receiver plane in light space (metres of light depth per metre along lx and ly): a tap
    // reads the stored depth at its own texel, so the receiver depth is moved to that texel
    // along the surface (no acne at a wide kernel), plus half a texel of slack for the cell.
    float3 ns = mul(f.light.shadow[c], float4(ng, 0.0)).xyz;
    float rl = cs.x * tile * 0.5;
    float3 nl = float3(ns.x * rl, ns.y * rl, ns.z * cs.y);
    float izn = 1.0 / max(abs(nl.z), 0.12);
    float gx = clamp(-nl.x * izn * sign(nl.z), -8.0, 8.0);
    float gy = clamp(-nl.y * izn * sign(nl.z), -8.0, 8.0);
    float zr = s.z + (f.light.filter.w + 0.5 * (abs(gx) + abs(gy))) * cs.x / cs.y;
    float zk = cs.x / cs.y;
    float lit = 0.0;
    float wsum = 0.0;
    int2 lo = int2(cs.zw);
    int2 hi = lo + int2(int(tile) - 1);
    float inv = 1.0 / (k + sp);
    for (int y = -2; y <= 2; y++) {
        float oy = float(y) * sp;
        float wy = 1.0 - abs(oy) * inv;
        for (int x = -2; x <= 2; x++) {
            float ox = float(x) * sp;
            float w = (1.0 - abs(ox) * inv) * wy;
            float2 tp = floor(uv + float2(ox, oy) + jit);
            float2 d = tp + 0.5 - uv;
            int2 t = clamp(lo + int2(tp), lo, hi);
            float zs = ed_depth_read(atlas, uint2(t));
            float zt = zr + (gx * d.x - gy * d.y) * zk;
            lit += (zt >= zs ? 1.0 : 0.0) * w;
            wsum += w;
        }
    }
    return lit / wsum;
}

// Debug: (receiver - stored) light depth in metres at the centre texel of cascade `c`.
float cascade_depth_delta(FrameData* f, uint atlas, uint c, float3 p) {
    float4 cs = f.light.cascade[c];
    float tile = f.light.cfg.y;
    float4 s = mul(f.light.shadow[c], float4(p, 1.0));
    float2 uv = float2(s.x * 0.5 + 0.5, 0.5 - s.y * 0.5) * tile;
    int2 lo = int2(cs.zw);
    int2 t = clamp(lo + int2(floor(uv)), lo, lo + int2(int(tile) - 1));
    return (s.z - ed_depth_read(atlas, uint2(t))) * cs.y;
}

// Light-space edge distance of `p` in cascade `c`: max(|x|, |y|) (1 = the tile edge). A point
// outside the cascade's depth range counts as outside: the tile's square covers far more than
// the slice's bounding sphere, and a receiver behind the volume (z < 0) would compare against
// the cleared depth and read as shadowed.
float cascade_extent(FrameData* f, uint c, float3 p) {
    float4 s = mul(f.light.shadow[c], float4(p, 1.0));
    return (s.z < 0.0 || s.z > 1.0) ? 1e9 : max(abs(s.x), abs(s.y));
}

// Sun visibility 0..1 at camera-relative `p` (view distance `dist`). The first cascade whose
// tile holds the point (minus the filter margin) is used; inside the blend band near its edge
// the next cascade is blended in; the last cascade fades to lit between the fade start and the
// shadow distance; past every cascade the sun is unshadowed.
float sun_shadow(FrameData* f, uint atlas, float3 p, float3 ng, float dist, float2 px) {
    uint n = uint(f.light.cfg.x);
    if (f.light.flags.x == 0 || n == 0 || dist >= f.light.filter.y) {
        return 1.0;
    }
    float margin = 1.0 - f.light.cfg.w * 2.0 / f.light.cfg.y;
    float band = f.light.cfg.z;
    for (uint c = 0; c < n; c++) {
        float e = cascade_extent(f, c, p) / margin;
        if (e > 1.0) {
            continue;
        }
        float v = cascade_lookup(f, atlas, c, p, ng, px);
        float blend = smoothstep(1.0 - band, 1.0, e);
        if (blend > 0.0) {
            float next = 1.0;
            if (c + 1 < n && cascade_extent(f, c + 1, p) / margin <= 1.0) {
                next = cascade_lookup(f, atlas, c + 1, p, ng, px);
            }
            v = lerp(v, next, blend);
        }
        float fade = smoothstep(0.0, 1.0, saturate((dist - f.light.filter.x) / max(f.light.filter.y - f.light.filter.x, 1e-3)));
        return lerp(v, 1.0, fade);
    }
    return 1.0;
}

// Debug: the cascade `sun_shadow` picks for `p` (n = past every cascade or shadows off).
uint shadow_cascade(FrameData* f, float3 p, float dist) {
    uint n = uint(f.light.cfg.x);
    if (f.light.flags.x == 0 || dist >= f.light.filter.y) {
        return 4u;
    }
    float margin = 1.0 - f.light.cfg.w * 2.0 / f.light.cfg.y;
    for (uint c = 0; c < n; c++) {
        if (cascade_extent(f, c, p) / margin <= 1.0) {
            return c;
        }
    }
    return 4u;
}

// Ambient occlusion and sky visibility at a full-res pixel from the half-res GTAO result
// (ao.slang): the four nearest half-res texels, weighted bilinearly and by how close their
// linear depth is to this pixel's (`zlin`), so occlusion does not bleed across depth edges.
// Half-res texel t is the full-res pixel 2t (its centre at 2t + 0.5), and the blur keeps it
// there. Returns (ao ^ intensity, sky visibility).
float2 ao_upsample(FrameData* f, uint ao_buf, float2 pixel, float zlin) {
    if (f.light.flags.y == 0) {
        return float2(1.0);
    }
    float4* buf = ed_buf<float4>(ao_buf);
    uint hw = uint(f.light.ao.y);
    uint hh = uint(f.light.ao.z);
    float2 h = (pixel - 0.5) * 0.5;
    float2 b = floor(h);
    float2 g = h - b;
    float2 sum = float2(0.0);
    float wsum = 0.0;
    for (int y = 0; y < 2; y++) {
        for (int x = 0; x < 2; x++) {
            int2 t = clamp(int2(b) + int2(x, y), int2(0), int2(int(hw) - 1, int(hh) - 1));
            float4 s = buf[uint(t.y) * hw + uint(t.x)];
            float wb = (x == 0 ? 1.0 - g.x : g.x) * (y == 0 ? 1.0 - g.y : g.y);
            float wz = 1.0 / (1e-3 + abs(s.z - zlin) / max(zlin, 1e-3) * 50.0);
            float w = max(wb, 1e-3) * wz;
            sum += s.xy * w;
            wsum += w;
        }
    }
    float2 r = wsum > 0.0 ? sum / wsum : float2(1.0);
    float sv = f.light.skyvis.x > 0.0 ? saturate(r.y) : 1.0;
    return float2(pow(saturate(r.x), f.light.ao.x), sv);
}

// Ambient light with large-scale sky occlusion (M4f): the share of the hemisphere a street
// canyon hides (1 - `sv`) sees building and ground surfaces instead of sky, lit about like
// the ground the bounce term models, so it gets the lower hemisphere's bounce radiance scaled
// by `f.light.skyvis.y` instead of skylight. sv = 1 is the open-ground ambient unchanged.
float3 occluded_ambient(FrameData* f, float3 open, float sv) {
    float3 bounce = ambient_light(f, float3(0.0, -1.0, 0.0)) * f.light.skyvis.y;
    return lerp(bounce, open, sv);
}

// Multi-bounce ambient occlusion (Jimenez et al. 2016): light bouncing inside a crease fills
// it back in, more on bright albedo, so occlusion is per channel and never darker than `ao`.
// `f.light.sky.y` blends it in (0 = plain AO).
float3 ao_multibounce(FrameData* f, float ao, float3 albedo) {
    float3 a = 2.0404 * albedo - 0.3324;
    float3 b = -4.7951 * albedo + 0.6417;
    float3 c = 2.7552 * albedo + 0.6903;
    float3 mb = max(float3(ao), ((ao * a + b) * ao + c) * ao);
    return lerp(float3(ao), mb, f.light.sky.y);
}
