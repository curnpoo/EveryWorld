// Depth pyramid (HZB) in one float buffer. Level 0 is floor(W/2) x floor(H/2) of the depth
// target, each next level halves again (at least 1). A texel stores the farthest depth (the
// minimum, reverse-Z) of the texels it covers; the last row and column also fold in the odd
// leftover source texel so the pyramid stays conservative for every size.

struct HzbC {
    uint hzb;       // bindless buffer
    uint depth;     // bindless depth texture (hzb_first only)
    uint src_off;   // hzb_down: source level offset (floats)
    uint src_w;
    uint src_h;
    uint dst_off;
    uint dst_w;
    uint dst_h;
    uint pad[8];
};

inline uint2 src_range(uint d, uint dst_n, uint src_n) {
    uint a = min(d * 2u, src_n - 1u);
    uint b = d + 1u == dst_n ? src_n - 1u : min(d * 2u + 1u, src_n - 1u);
    return uint2(a, b);
}

kernel void hzb_first(uint2 t [[thread_position_in_grid]], ED_BINDLESS(HzbC)) {
    if (t.x >= ed_c.dst_w || t.y >= ed_c.dst_h) {
        return;
    }
    depth2d<float> d = ed_depth2d(ed_c.depth);
    uint2 rx = src_range(t.x, ed_c.dst_w, ed_c.src_w);
    uint2 ry = src_range(t.y, ed_c.dst_h, ed_c.src_h);
    float m = 1.0;
    for (uint y = ry.x; y <= ry.y; y++) {
        for (uint x = rx.x; x <= rx.y; x++) {
            m = min(m, d.read(uint2(x, y)));
        }
    }
    ed_buf(float, ed_c.hzb)[ed_c.dst_off + t.y * ed_c.dst_w + t.x] = m;
}

kernel void hzb_down(uint2 t [[thread_position_in_grid]], ED_BINDLESS(HzbC)) {
    if (t.x >= ed_c.dst_w || t.y >= ed_c.dst_h) {
        return;
    }
    device float* h = ed_buf(float, ed_c.hzb);
    uint2 rx = src_range(t.x, ed_c.dst_w, ed_c.src_w);
    uint2 ry = src_range(t.y, ed_c.dst_h, ed_c.src_h);
    float m = 1.0;
    for (uint y = ry.x; y <= ry.y; y++) {
        for (uint x = rx.x; x <= rx.y; x++) {
            m = min(m, h[ed_c.src_off + y * ed_c.src_w + x]);
        }
    }
    h[ed_c.dst_off + t.y * ed_c.dst_w + t.x] = m;
}
