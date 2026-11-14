// Readback helpers: picking (ObjectId under a pixel) and the pop detector's per-tile stats.
#include "shader_abi.slang"
#include "common.slang"

struct PickC {
    uint vis;
    uint instances;
    uint out;
    uint x;
    uint y;
    uint w;
    uint h;
    uint pad[9];
};

struct PopC {
    uint hdr;
    uint vis;
    uint state;
    uint out;
    uint w;
    uint h;
    uint tiles_x;
    uint tiles_y;
    float exposure;
    uint lut;       // bindless buffer: the Filmic LUT (tonemap)
    uint pad[6];
};

// A module has one constants block; pop_stats reads the same 64 bytes as PopC.
ED_CONSTANTS(PickC)
PopC pop_c() { PickC raw = ed_c; return reinterpret<PopC>(raw); }

// out[0] = instance + 1 (0 = nothing), out[1] = ObjectId low 32 bits, out[2] = raw vis.y.
[shader("compute")]
[numthreads(1, 1, 1)]
void pick(uint3 tid: SV_DispatchThreadID) {
    if (tid.x != 0) {
        return;
    }
    uint* o = ed_buf<uint>(ed_c.out);
    uint2 v = uint2(0);
    if (ed_c.x < ed_c.w && ed_c.y < ed_c.h) {
        v = ed_tex2d_uint(ed_c.vis).Load(int3(int(ed_c.x), int(ed_c.y), 0)).xy;
    }
    if ((v.x & 0xE0000000u) != 0) {
        v.x = 0;  // terrain (TERRAIN_VIS_BIT): not an instance, nothing picked
    }
    o[0] = v.x;
    o[1] = v.x == 0 ? 0u : ed_buf<GpuInstance>(ed_c.instances)[v.x - 1u].object_lo;
    o[2] = v.y;
    o[3] = 0;
}

static const uint POP_TILE = 8;

// One thread per 8x8 tile: mean display luminance and the number of pixels that belong to an
// instance whose LOD is cross-fading or switched this frame.
[shader("compute")]
[numthreads(8, 8, 1)]
void pop_stats(uint3 tid: SV_DispatchThreadID) {
    uint2 t = tid.xy;
    PopC c = pop_c();
    if (t.x >= c.tiles_x || t.y >= c.tiles_y) {
        return;
    }
    Texture2D hdr = ed_tex2d(c.hdr);
    Texture2D<uint4> vis = ed_tex2d_uint(c.vis);
    uint* state = ed_buf<uint>(c.state);
    float sum = 0.0;
    uint trans = 0;
    uint n = 0;
    for (uint y = 0; y < POP_TILE; y++) {
        for (uint x = 0; x < POP_TILE; x++) {
            uint2 p = t * POP_TILE + uint2(x, y);
            if (p.x >= c.w || p.y >= c.h) {
                continue;
            }
            float3 col = alpha_tonemap(c.lut, hdr.Load(int3(int2(p), 0)).rgb, c.exposure);
            sum += dot(col, float3(0.2126, 0.7152, 0.0722));
            n += 1;
            uint id = vis.Load(int3(int2(p), 0)).x;
            if (id != 0 && (id & 0xE0000000u) == 0) {
                uint st = state[id - 1u];
                if (((st >> ST_FADE_SHIFT) & 63u) != 0 || (st & ST_CHANGED) != 0) {
                    trans += 1;
                }
            }
        }
    }
    float* o = ed_buf<float>(c.out);
    uint i = (t.y * c.tiles_x + t.x) * 2u;
    o[i] = n > 0 ? sum / float(n) : 0.0;
    o[i + 1] = float(trans);
}
