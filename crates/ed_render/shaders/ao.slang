// Ground-truth ambient occlusion (GTAO, Jimenez et al. 2016, after XeGTAO) from the depth
// buffer at half resolution, no temporal filter (M4d). Two dispatches in the "ao" pass:
//   mode 0 (gtao): per half-res texel, view-space position and normal from depth, 2 slices x
//                  2 directions x 4 steps (4x4 interleaved noise for slice angle and step
//                  jitter), cosine-weighted visible arc -> raw[t] = (ao, sky visibility, linear
//                  depth, 0). The AO radius is contact scale (`radius`, under a metre, and at
//                  most AO_MAX_FRAC of the screen height up close), so it darkens creases and
//                  contacts only. The same slices are searched a second time at a large radius
//                  (M4f, `sv_on`): the share of the sky the street canyon leaves open, which
//                  resolve uses to swap occluded skylight for light bounced off the buildings.
//                  It runs on one slice per texel (checkerboard) with linear steps from the AO
//                  radius out to `sv_radius` or 0.75 of the screen height, reading the depth
//                  pyramid (farthest depth per block) at a level matched to the step length and
//                  never finer than SV_MIN_M of world per block, so only building-scale
//                  occluders survive: people, poles, benches and trunks erode away instead of
//                  acting as canyon walls (a horizon slice treats every occluder as infinitely
//                  wide). It also treats samples well in front of the receiver as thin (XeGTAO's
//                  thin-occluder compensation). The stored sky visibility is the share of the
//                  AO-visible arc that the far horizon leaves open (far arc / near arc), so the
//                  two terms multiply without counting a near occluder twice.
//   mode 1 (blur): 5x5 depth-aware tent (outer taps at half weight) over the 4x4 noise period,
//                  centred on the texel (a 4x4 box would shift the result by half a texel).
// Resolve upsamples `out` with depth weights (lighting.slang `ao_upsample`) and applies it to
// ambient light only.
#include "shader_abi.slang"
#include "common.slang"

struct AoC {
    uint frame;
    uint depth;     // bindless depth texture (full res)
    uint raw;       // bindless buffer: float4 per half-res texel
    uint out;       // bindless buffer: float4 per half-res texel
    uint mode;      // 0 gtao, 1 blur
    uint hw;        // half-res size
    uint hh;
    float radius;   // world-space radius (m)
    float sv_radius; // sky-visibility search radius (m)
    uint sv_on;     // 1: also search the sky visibility
    uint hzb;       // bindless buffer: the depth pyramid (provider.rs `hzb_levels`)
    uint hzb_on;    // 1: `hzb` is valid (levels in FrameData.hzb, count in cull.x)
    uint pad[4];
};
ED_CONSTANTS(AoC)

static const float PI_AO = 3.14159265;
static const float FAR_Z = 1.0e6;
static const uint AO_SLICES = 2;
static const uint AO_STEPS = 4;
static const uint SV_STEPS = 4;
// Contact AO: largest screen radius (fraction of the target height, full-res px), so a
// surface right in front of the camera does not search half the screen.
static const float AO_MAX_FRAC = 0.25;
// Sky visibility ignores occluders narrower than this (m): the depth pyramid level it reads
// has blocks at least this wide at the receiver's distance.
static const float SV_MIN_M = 2.0;
// Sky-visibility search: largest screen radius (fraction of the target height, full-res px)
// and thin-occluder compensation.
static const float SV_MAX_FRAC = 0.75;
static const float SV_THIN = 3.0;

// Linear view depth (m) of a full-res pixel; sky is FAR_Z.
float linear_z(FrameData* f, int2 q) {
    int2 fs = int2(f.proj.zw);
    q = clamp(q, int2(0), fs - 1);
    float d = ed_depth_read(ed_c.depth, uint2(q));
    return d > 0.0 ? min(f.params.z / d, FAR_Z) : FAR_Z;
}

// View-space position (x right, y up, looking down -z) of a pixel centre at linear depth z.
float3 view_pos(FrameData* f, float2 q, float z) {
    float2 uv = (q + 0.5) / f.proj.zw;
    float2 ndc = float2(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    return float3(ndc.x / f.proj.x * z, ndc.y / f.proj.y * z, -z);
}

float3 view_at(FrameData* f, int2 q) {
    return view_pos(f, float2(q), linear_z(f, q));
}

// View-space position from the depth pyramid near full-res pixel `q`: the level whose texel is
// about `off` / 16 pixels wide (sparse far taps then share cache lines across the wave), read
// at that texel's centre. The pyramid holds the farthest depth of each block, so thin
// occluders erode away there, which suits the canyon-scale search. Full-res depth without it.
// `min_px`: the smallest block (full-res px) to read, which erodes occluders narrower than it.
float3 view_at_far(FrameData* f, int2 q, float off, float min_px) {
    int levels = int(f.cull.x);
    if (ed_c.hzb_on == 0 || levels == 0) {
        return view_at(f, q);
    }
    int l = clamp(max(int(log2(max(off, 1.0))) - 4, int(ceil(log2(max(min_px, 2.0)))) - 1), 0, levels - 1);
    HzbLevel lv = f.hzb[l];
    float ts = float(2u << uint(l));
    int2 fs = int2(f.proj.zw);
    q = clamp(q, int2(0), fs - 1);
    uint2 tx = min(uint2(float2(q) / ts), uint2(lv.width - 1, lv.height - 1));
    float d = ed_buf<float>(ed_c.hzb)[lv.offset + tx.y * lv.width + tx.x];
    float z = d > 0.0 ? min(f.params.z / d, FAR_Z) : FAR_Z;
    return view_pos(f, (float2(tx) + 0.5) * ts - 0.5, z);
}

float fast_acos(float x) {
    float a = abs(x);
    float r = (-0.156583 * a + PI_AO * 0.5) * sqrt(1.0 - a);
    return x >= 0.0 ? r : PI_AO - r;
}

// Horizon cosines (towards +omega, towards -omega) of one slice: `steps` samples per side out to
// `r_px` full-res pixels, with the distance falloff over the outer 60% of `radius` (m). `thin`
// stretches a sample's distance along the view ray (0 = plain GTAO), so a sample far in front
// of the receiver counts as a thin occluder and fades out.
// `pw` spaces the steps (2: quadratic, dense near the centre; 1: linear). The steps run from
// `r0_px` to `r_px`; `min_px` is the far search's smallest pyramid block (`view_at_far`).
float2 slice_horizons(FrameData* f, int2 q, float3 p, float2 omega, float2 low, float r0_px, float r_px, float radius, float thin, uint steps, float jitter, float pw, bool far, float min_px) {
    float range = 0.6 * radius;
    float fmul = -1.0 / range;
    float fadd = (radius - range) / range + 1.0;
    float3 v = normalize(-p);
    float2 h = low;
    for (uint k = 0; k < steps; k++) {
        float st = (float(k) + jitter) / float(steps);
        float off = max(r0_px + (pw > 1.5 ? st * st : st) * (r_px - r0_px), 1.0 + float(k));
        float2 d = omega * off;
        float3 d0 = (far ? view_at_far(f, q + int2(round(d)), off, min_px) : view_at(f, q + int2(round(d)))) - p;
        float3 d1 = (far ? view_at_far(f, q - int2(round(d)), off, min_px) : view_at(f, q - int2(round(d)))) - p;
        float l0 = length(d0);
        float l1 = length(d1);
        float c0 = dot(d0 / max(l0, 1e-6), v);
        float c1 = dot(d1 / max(l1, 1e-6), v);
        float t0 = thin * max(dot(d0, v), 0.0);
        float t1 = thin * max(dot(d1, v), 0.0);
        float w0 = saturate((l0 + t0) * fmul + fadd);
        float w1 = saturate((l1 + t1) * fmul + fadd);
        h.x = max(h.x, lerp(low.x, c0, w0));
        h.y = max(h.y, lerp(low.y, c1, w1));
    }
    return h;
}

// Cosine-weighted visible arc of a slice between the horizons `h` (cosines), around the
// projected normal angle `ang_n`.
float slice_arc(float2 h, float ang_n, float cos_n) {
    float a0 = -fast_acos(h.y);
    float a1 = fast_acos(h.x);
    a0 = ang_n + max(a0 - ang_n, -PI_AO * 0.5);
    a1 = ang_n + min(a1 - ang_n, PI_AO * 0.5);
    float sin_n = sin(ang_n);
    float arc0 = (cos_n + 2.0 * a0 * sin_n - cos(2.0 * a0 - ang_n)) * 0.25;
    float arc1 = (cos_n + 2.0 * a1 * sin_n - cos(2.0 * a1 - ang_n)) * 0.25;
    return arc0 + arc1;
}

void gtao(FrameData* f, uint2 t) {
    float4* raw = ed_buf<float4>(ed_c.raw);
    int2 q = int2(t * 2u);
    float z = linear_z(f, q);
    uint o = t.y * ed_c.hw + t.x;
    if (z >= FAR_Z) {
        raw[o] = float4(1.0, 1.0, FAR_Z, 0.0);
        return;
    }
    float3 p = view_pos(f, float2(q), z);
    // Normal from the neighbour on the side with the smaller depth step (no edge smearing).
    float3 pr = view_at(f, q + int2(2, 0));
    float3 pl = view_at(f, q - int2(2, 0));
    float3 pd = view_at(f, q + int2(0, 2));
    float3 pu = view_at(f, q - int2(0, 2));
    float3 dx = abs(pr.z - p.z) < abs(p.z - pl.z) ? pr - p : p - pl;
    float3 dy = abs(pd.z - p.z) < abs(p.z - pu.z) ? pd - p : p - pu;
    float3 n = normalize(cross(dy, dx));
    float3 v = normalize(-p);
    if (dot(n, v) < 0.0) {
        n = -n;
    }
    // Screen radius in full-res pixels (per metre of world radius).
    float px_per_m = f.proj.y * f.proj.w * 0.5 / z;
    float r_px = min(ed_c.radius * px_per_m, AO_MAX_FRAC * f.proj.w);
    float sv_px = min(ed_c.sv_radius * px_per_m, SV_MAX_FRAC * f.proj.w);
    bool small = r_px >= 2.0;
    bool sv = ed_c.sv_on != 0 && sv_px >= 2.0;
    if (!small && !sv) {
        raw[o] = float4(1.0, 1.0, z, 0.0);
        return;
    }
    // 4x4 noise: a Bayer order for the slice angle, interleaved gradient noise for the steps.
    const uint BAYER4[16] = { 0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5 };
    float n0 = (float(BAYER4[(t.y & 3u) * 4u + (t.x & 3u)]) + 0.5) / 16.0;
    float n1 = frac(52.9829189 * frac(0.06711056 * float(t.x & 3u) + 0.00583715 * float(t.y & 3u)));
    float vis = 0.0;
    float vsky = 1.0;
    for (uint s = 0; s < AO_SLICES; s++) {
        float phi = (float(s) + n0) / float(AO_SLICES) * PI_AO;
        float3 dir = float3(cos(phi), sin(phi), 0.0);
        float2 omega = float2(dir.x, -dir.y);
        float3 ortho = dir - dot(dir, v) * v;
        float3 axis = normalize(cross(ortho, v));
        float3 pn = n - axis * dot(n, axis);
        float pn_len = length(pn);
        float sgn = dot(ortho, pn) >= 0.0 ? 1.0 : -1.0;
        float cos_n = saturate(dot(pn, v) / max(pn_len, 1e-6));
        float ang_n = sgn * fast_acos(cos_n);
        float2 low = float2(cos(ang_n + PI_AO * 0.5), cos(ang_n - PI_AO * 0.5));
        float2 h = small ? slice_horizons(f, q, p, omega, low, 0.0, r_px, ed_c.radius, 0.0, AO_STEPS, n1, 2.0, false, 0.0) : low;
        float arc = slice_arc(h, ang_n, cos_n);
        vis += pn_len * arc;
        // Sky visibility is low frequency: one of the two slices per texel (checkerboard), the
        // blur gathers the other half of the directions from the neighbours. Searched beyond
        // the AO radius and kept as the ratio of the far arc to the near one, so whatever the
        // AO already darkens is not darkened again.
        if (sv && s == ((t.x + t.y) & 1u)) {
            float r0 = min(max(r_px, 0.0), sv_px * 0.5);
            float2 hf = max(h, slice_horizons(f, q, p, omega, low, r0, sv_px, ed_c.sv_radius, SV_THIN, SV_STEPS, frac(n1 + 0.5), 1.0, true, SV_MIN_M * px_per_m));
            vsky = slice_arc(hf, ang_n, cos_n) / max(arc, 1e-4);
        }
    }
    vis /= float(AO_SLICES);
    // A plane reads ~0.98-1.0 from depth quantisation: map that to exactly unoccluded.
    raw[o] = float4(saturate(vis / 0.97), saturate(vsky / 0.98), z, 0.0);
}

float4 raw_at(int2 s) {
    s = clamp(s, int2(0), int2(int(ed_c.hw) - 1, int(ed_c.hh) - 1));
    return ed_buf<float4>(ed_c.raw)[uint(s.y) * ed_c.hw + uint(s.x)];
}

// The smaller of the two one-sided depth steps (no slope across a depth edge).
float depth_slope(float zm, float z, float zp) {
    float a = z - zm;
    float b = zp - z;
    return abs(a) < abs(b) ? a : b;
}

void blur(uint2 t) {
    float4* outb = ed_buf<float4>(ed_c.out);
    uint o = t.y * ed_c.hw + t.x;
    int2 ti = int2(t);
    float4 c = raw_at(ti);
    if (c.z >= FAR_Z) {
        outb[o] = c;
        return;
    }
    // Depth weights against the local plane (a ground plane seen at a grazing angle changes
    // depth by more than the tolerance from one texel to the next).
    float gx = depth_slope(raw_at(ti - int2(1, 0)).z, c.z, raw_at(ti + int2(1, 0)).z);
    float gy = depth_slope(raw_at(ti - int2(0, 1)).z, c.z, raw_at(ti + int2(0, 1)).z);
    float tol = 1.0 / (0.05 * c.z);
    float2 sum = float2(0.0);
    float wsum = 0.0;
    for (int y = -2; y <= 2; y++) {
        float wy = abs(y) == 2 ? 0.5 : 1.0;
        for (int x = -2; x <= 2; x++) {
            float wx = abs(x) == 2 ? 0.5 : 1.0;
            float4 v = raw_at(ti + int2(x, y));
            float pz = c.z + gx * float(x) + gy * float(y);
            float w = wx * wy * saturate(1.0 - abs(v.z - pz) * tol);
            sum += v.xy * w;
            wsum += w;
        }
    }
    outb[o] = float4(wsum > 0.0 ? sum / wsum : c.xy, c.z, 0.0);
}

[shader("compute")]
[numthreads(8, 8, 1)]
void ao_main(uint3 tid: SV_DispatchThreadID) {
    uint2 t = tid.xy;
    if (t.x >= ed_c.hw || t.y >= ed_c.hh) {
        return;
    }
    FrameData* f = ed_buf<FrameData>(ed_c.frame);
    if (ed_c.mode == 0) {
        gtao(f, t);
    } else {
        blur(t);
    }
}
