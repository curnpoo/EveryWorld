// Ground-truth ambient occlusion (GTAO, Jimenez et al. 2016, after XeGTAO) from the depth
// buffer at half resolution, no temporal filter (M4d). Two dispatches in the "ao" pass:
//   mode 0 (gtao): per half-res texel, view-space position and normal from depth, 2 slices x
//                  2 directions x 4 steps (4x4 interleaved noise for slice angle and step
//                  jitter), cosine-weighted visible arc -> raw[t] = (visibility, linear depth)
//   mode 1 (blur): 4x4 depth-aware box over the noise period -> out[t]
// Resolve upsamples `out` with depth weights (lighting.slang `ao_upsample`) and applies it to
// ambient light only.
#include "shader_abi.slang"
#include "common.slang"

struct AoC {
    uint frame;
    uint depth;     // bindless depth texture (full res)
    uint raw;       // bindless buffer: float2 per half-res texel
    uint out;       // bindless buffer: float2 per half-res texel
    uint mode;      // 0 gtao, 1 blur
    uint hw;        // half-res size
    uint hh;
    float radius;   // world-space radius (m)
    uint pad[8];
};
ED_CONSTANTS(AoC)

static const float PI_AO = 3.14159265;
static const float FAR_Z = 1.0e6;
static const uint AO_SLICES = 2;
static const uint AO_STEPS = 4;

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

float fast_acos(float x) {
    float a = abs(x);
    float r = (-0.156583 * a + PI_AO * 0.5) * sqrt(1.0 - a);
    return x >= 0.0 ? r : PI_AO - r;
}

void gtao(FrameData* f, uint2 t) {
    float2* raw = ed_buf<float2>(ed_c.raw);
    int2 q = int2(t * 2u);
    float z = linear_z(f, q);
    uint o = t.y * ed_c.hw + t.x;
    if (z >= FAR_Z) {
        raw[o] = float2(1.0, FAR_Z);
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
    // Screen radius in full-res pixels.
    float r_px = ed_c.radius * f.proj.y * f.proj.w * 0.5 / z;
    if (r_px < 2.0) {
        raw[o] = float2(1.0, z);
        return;
    }
    r_px = min(r_px, 256.0);
    // 4x4 noise: a Bayer order for the slice angle, interleaved gradient noise for the steps.
    const uint BAYER4[16] = { 0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5 };
    float n0 = (float(BAYER4[(t.y & 3u) * 4u + (t.x & 3u)]) + 0.5) / 16.0;
    float n1 = frac(52.9829189 * frac(0.06711056 * float(t.x & 3u) + 0.00583715 * float(t.y & 3u)));
    float falloff_range = 0.6 * ed_c.radius;
    float falloff_mul = -1.0 / falloff_range;
    float falloff_add = (ed_c.radius - falloff_range) / falloff_range + 1.0;
    float vis = 0.0;
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
        float low0 = cos(ang_n + PI_AO * 0.5);
        float low1 = cos(ang_n - PI_AO * 0.5);
        float h0 = low0;
        float h1 = low1;
        for (uint k = 0; k < AO_STEPS; k++) {
            float st = (float(k) + n1) / float(AO_STEPS);
            float off = max(st * st * r_px, 1.0 + float(k));
            float2 d = omega * off;
            float3 s0 = view_at(f, q + int2(round(d)));
            float3 s1 = view_at(f, q - int2(round(d)));
            float3 d0 = s0 - p;
            float3 d1 = s1 - p;
            float l0 = length(d0);
            float l1 = length(d1);
            float c0 = dot(d0 / max(l0, 1e-6), v);
            float c1 = dot(d1 / max(l1, 1e-6), v);
            float w0 = saturate(l0 * falloff_mul + falloff_add);
            float w1 = saturate(l1 * falloff_mul + falloff_add);
            h0 = max(h0, lerp(low0, c0, w0));
            h1 = max(h1, lerp(low1, c1, w1));
        }
        float a0 = -fast_acos(h1);
        float a1 = fast_acos(h0);
        a0 = ang_n + max(a0 - ang_n, -PI_AO * 0.5);
        a1 = ang_n + min(a1 - ang_n, PI_AO * 0.5);
        float sin_n = sin(ang_n);
        float arc0 = (cos_n + 2.0 * a0 * sin_n - cos(2.0 * a0 - ang_n)) * 0.25;
        float arc1 = (cos_n + 2.0 * a1 * sin_n - cos(2.0 * a1 - ang_n)) * 0.25;
        vis += pn_len * (arc0 + arc1);
    }
    vis /= float(AO_SLICES);
    // A plane reads ~0.98-1.0 from depth quantisation: map that to exactly unoccluded.
    raw[o] = float2(saturate(vis / 0.97), z);
}

void blur(uint2 t) {
    float2* raw = ed_buf<float2>(ed_c.raw);
    float2* outb = ed_buf<float2>(ed_c.out);
    uint o = t.y * ed_c.hw + t.x;
    float2 c = raw[o];
    if (c.y >= FAR_Z) {
        outb[o] = c;
        return;
    }
    float sum = 0.0;
    float wsum = 0.0;
    for (int y = -1; y <= 2; y++) {
        for (int x = -1; x <= 2; x++) {
            int2 s = clamp(int2(t) + int2(x, y), int2(0), int2(int(ed_c.hw) - 1, int(ed_c.hh) - 1));
            float2 v = raw[uint(s.y) * ed_c.hw + uint(s.x)];
            float w = saturate(1.0 - abs(v.y - c.y) / (0.05 * c.y));
            sum += v.x * w;
            wsum += w;
        }
    }
    outb[o] = float2(wsum > 0.0 ? sum / wsum : c.x, c.y);
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
