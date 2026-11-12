// ed_gpu Slang ABI test shaders (tests/slang_abi.rs): pointers into the buffer table, atomics
// and wave ops through pointers, typed views of the texture table (float, uint, depth), the
// sampler table, and SV_PrimitiveID in a fragment shader.
#include "shader_abi.slang"

struct C {
    uint src;
    uint dst;
    uint counter;
    uint n;
    uint utex;
    uint depth;
    uint tex;
    uint smp;
    uint pad[8];
};
ED_CONSTANTS(C)

// 32 bytes: a float4 then a uint4, as a #[repr(C)] Rust struct.
struct Item {
    float4 a;
    uint4 b;
};

[shader("compute")]
[numthreads(64, 1, 1)]
void abi_compute(uint3 id: SV_DispatchThreadID) {
    uint one = 0;
    if (id.x < ed_c.n) {
        Item s = ed_buf<Item>(ed_c.src)[id.x];
        Item* d = ed_buf<Item>(ed_c.dst) + id.x;
        d.a = s.a * 2.0;
        d.b = s.b + uint4(1, 2, 3, 4);
        one = 1;
    }
    uint sum = WaveActiveSum(one);
    if (WaveIsFirstLane() && sum != 0) {
        Atomic<uint>* c = (Atomic<uint>*)ed_buf<uint>(ed_c.counter);
        c[0].add(sum);
    }
}

// Reads texel (8x + 4, 63 - y) of a 64x64 uint texture, and texel (x, y) of a depth texture and a
// float texture (Load and Sample).
[shader("compute")]
[numthreads(8, 8, 1)]
void abi_textures(uint3 id: SV_DispatchThreadID) {
    uint4 u = ed_tex2d_uint(ed_c.utex).Load(int3(id.x * 8 + 4, 63 - id.y, 0));
    float d = ed_depth_read(ed_c.depth, id.xy);
    float4 f = ed_tex2d(ed_c.tex).Load(int3(id.xy, 0));
    float4 s = ed_tex2d(ed_c.tex).SampleLevel(ed_smp(ed_c.smp), (float2(id.xy) + 0.5) / 8.0, 0.0);
    uint4* o = ed_buf<uint4>(ed_c.dst) + (id.y * 8 + id.x) * 2;
    o[0] = uint4(u.x, u.y, asuint(d), 0);
    o[1] = uint4(f * 255.0 + 0.5);
    o[1].w = uint(s.x * 255.0 + 0.5);
}

struct VsOut {
    float4 pos: SV_Position;
    nointerpolation uint inst: INST;
};

// Draw `n` triangles in a row, each 1/n of the width, full height.
[shader("vertex")]
VsOut abi_vs(uint vid: SV_VulkanVertexID, uint iid: SV_VulkanInstanceID) {
    uint tri = vid / 3;
    uint k = vid % 3;
    float w = 2.0 / float(ed_c.n);
    float x0 = -1.0 + w * float(tri);
    float2 p = k == 0 ? float2(x0, -1.0) : (k == 1 ? float2(x0 + w, -1.0) : float2(x0 + w * 0.5, 1.0));
    VsOut o;
    o.pos = float4(p, 0.5, 1.0);
    o.inst = iid;
    return o;
}

[shader("fragment")]
uint4 abi_fs(VsOut i, uint prim: SV_PrimitiveID) : SV_Target0 {
    return uint4(i.inst + 1, prim, 0, 0);
}
