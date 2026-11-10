#include <metal_stdlib>
using namespace metal;

// Ocean FFT (stage 3 of docs/plans/ultra-water-port.md). 3 band-limited cascades, N x N each.
// Per cascade four complex fields are packed (two real fields per complex FFT):
//   0: slope x + i slope z      1: disp x + i disp z      2: dDx/dx + i dDz/dz      3: dDx/dz + i height
// ocean_spec evolves the spectrum to time t, ocean_fft runs row then column Stockham radix-2 FFTs in threadgroup
// memory, ocean_pack writes RGBA16F (slope x, slope z, Jacobian, height) per cascade into a texture array.

#define ON 128
struct OceanU { float4 len; float4 misc; }; // len = cascade lengths (m), misc.x = time, misc.y = choppiness, misc.z = loop period

float2 cmul(float2 a, float2 b) { return float2(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x); }
float2 cI(float2 a) { return float2(-a.y, a.x); } // i * a

kernel void ocean_spec(device const float4* h0 [[buffer(0)]], device float2* spec [[buffer(1)]], constant OceanU& U [[buffer(2)]], uint3 gid [[thread_position_in_grid]]) {
    uint x = gid.x, y = gid.y, c = gid.z;
    float L = U.len[c];
    float kx = 6.2831853 * (float(x) - ON / 2) / L, kz = 6.2831853 * (float(y) - ON / 2) / L;
    float k = sqrt(kx * kx + kz * kz);
    uint idx = y * ON + x;
    device float2* S = spec + c * 4 * ON * ON;
    float2 z0 = float2(0.0), z1 = z0, z2 = z0, z3 = z0;
    if (k > 1e-5) {
        float4 h = h0[c * ON * ON + idx];
        float w = sqrt(9.81 * k + 7.4e-5 * k * k * k);
        float w0 = 6.2831853 / U.misc.z;
        w = max(round(w / w0), 1.0) * w0;           // quantised so the field loops exactly every misc.z seconds
        float ph = w * U.misc.x;
        float2 e = float2(cos(ph), sin(ph));
        float2 H = cmul(h.xy, e) + cmul(h.zw, float2(e.x, -e.y));
        float ik = 1.0 / k;
        z0 = cI(H) * kx - H * kz;
        z1 = -cI(H) * (kx * ik) + H * (kz * ik);
        z2 = H * (kx * kx * ik) + cI(H) * (kz * kz * ik);
        z3 = H * (kx * kz * ik) + cI(H);
    }
    S[0 * ON * ON + idx] = z0; S[1 * ON * ON + idx] = z1; S[2 * ON * ON + idx] = z2; S[3 * ON * ON + idx] = z3;
}

// One threadgroup per (line, field, cascade); ON threads. pass.x: 0 = rows, 1 = columns.
kernel void ocean_fft(device float2* spec [[buffer(0)]], constant uint& pass [[buffer(1)]], uint3 tg [[threadgroup_position_in_grid]], uint tid [[thread_index_in_threadgroup]]) {
    threadgroup float2 buf[2][ON];
    uint line = tg.x, f = tg.y, c = tg.z;
    device float2* S = spec + (c * 4 + f) * ON * ON;
    uint gi = pass == 0 ? line * ON + tid : tid * ON + line;
    buf[0][tid] = S[gi];
    threadgroup_barrier(mem_flags::mem_threadgroup);
    uint src = 0;
    for (uint Ns = 1; Ns < ON; Ns *= 2) {
        if (tid < ON / 2) {
            uint j = tid;
            float ang = 6.2831853 * float(j % Ns) / float(Ns * 2);   // inverse transform: +angle
            float2 tw = float2(cos(ang), sin(ang));
            float2 v0 = buf[src][j];
            float2 v1 = cmul(buf[src][j + ON / 2], tw);
            uint d = (j / Ns) * Ns * 2 + (j % Ns);
            buf[1 - src][d] = v0 + v1;
            buf[1 - src][d + Ns] = v0 - v1;
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
        src = 1 - src;
    }
    S[gi] = buf[src][tid];
}

kernel void ocean_pack(device const float2* spec [[buffer(0)]], texture2d_array<float, access::write> outt [[texture(0)]], constant OceanU& U [[buffer(2)]], uint3 gid [[thread_position_in_grid]]) {
    uint x = gid.x, y = gid.y, c = gid.z;
    device const float2* S = spec + c * 4 * ON * ON + y * ON + x;
    float sgn = ((x + y) & 1) ? -1.0 : 1.0;           // spectrum was centred on k = 0
    float2 sl = S[0] * sgn;
    float2 d = S[1 * ON * ON] * sgn;
    float2 dd = S[2 * ON * ON] * sgn;
    float2 dz = S[3 * ON * ON] * sgn;
    float lam = U.misc.y;
    float J = (1.0 + lam * dd.x) * (1.0 + lam * dd.y) - lam * lam * dz.x * dz.x;
    outt.write(float4(sl.x, sl.y, J, dz.y), uint2(x, y), c);
}
