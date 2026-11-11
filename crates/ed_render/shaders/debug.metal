// Readback helpers: picking (ObjectId under a pixel) and the pop detector's per-tile stats.

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

// out[0] = instance + 1 (0 = nothing), out[1] = ObjectId low 32 bits, out[2] = raw vis.y.
kernel void pick(uint t [[thread_position_in_grid]], ED_BINDLESS(PickC)) {
    if (t != 0) {
        return;
    }
    device uint* out = ed_buf(uint, ed_c.out);
    uint2 v = uint2(0);
    if (ed_c.x < ed_c.w && ed_c.y < ed_c.h) {
        v = ed_tex2d_uint(ed_c.vis).read(uint2(ed_c.x, ed_c.y)).xy;
    }
    out[0] = v.x;
    out[1] = v.x == 0 ? 0u : ed_buf(GpuInstance, ed_c.instances)[v.x - 1u].object_lo;
    out[2] = v.y;
    out[3] = 0;
}

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
    uint pad[7];
};

constant uint POP_TILE = 8;

// One thread per 8x8 tile: mean display luminance and the number of pixels that belong to an
// instance whose LOD is cross-fading or switched this frame.
kernel void pop_stats(uint2 t [[thread_position_in_grid]], ED_BINDLESS(PopC)) {
    if (t.x >= ed_c.tiles_x || t.y >= ed_c.tiles_y) {
        return;
    }
    texture2d<float> hdr = ed_tex2d(ed_c.hdr);
    texture2d<uint> vis = ed_tex2d_uint(ed_c.vis);
    device const uint* state = ed_buf(uint, ed_c.state);
    float sum = 0.0;
    uint trans = 0;
    uint n = 0;
    for (uint y = 0; y < POP_TILE; y++) {
        for (uint x = 0; x < POP_TILE; x++) {
            uint2 p = t * POP_TILE + uint2(x, y);
            if (p.x >= ed_c.w || p.y >= ed_c.h) {
                continue;
            }
            float3 c = aces_fit(hdr.read(p).rgb * ed_c.exposure);
            sum += dot(c, float3(0.2126, 0.7152, 0.0722));
            n += 1;
            uint id = vis.read(p).x;
            if (id != 0) {
                uint st = state[id - 1u];
                if (((st >> ST_FADE_SHIFT) & 63u) != 0 || (st & ST_CHANGED) != 0) {
                    trans += 1;
                }
            }
        }
    }
    device float* out = ed_buf(float, ed_c.out);
    uint i = (t.y * ed_c.tiles_x + t.x) * 2u;
    out[i] = n > 0 ? sum / float(n) : 0.0;
    out[i + 1] = float(trans);
}
