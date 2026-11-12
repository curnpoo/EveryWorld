#include <metal_stdlib>
#include <metal_math>
#include <metal_texture>
using namespace metal;
struct C_0
{
    uint src_0;
    uint dst_0;
    uint counter_0;
    uint n_0;
    uint utex_0;
    uint depth_0;
    uint tex_0;
    uint smp_0;
    array<uint, int(8)> pad_0;
};

struct KernelContext_0
{
    C_0 constant* ed_c_0;
    ulong device* ed_buffers_0;
    ulong device* ed_textures_0;
    ulong device* ed_samplers_0;
};

packed_uint4 device* ed_buf_0(uint i_0, KernelContext_0 thread* kernelContext_0)
{
    return (packed_uint4 device*)kernelContext_0->ed_buffers_0[i_0];
}

uint device* ed_buf_1(uint i_1, KernelContext_0 thread* kernelContext_1)
{
    return (uint device*)kernelContext_1->ed_buffers_0[i_1];
}

struct Item_natural_0
{
    packed_float4 a_0;
    packed_uint4 b_0;
};

Item_natural_0 device* ed_buf_2(uint i_2, KernelContext_0 thread* kernelContext_2)
{
    return (Item_natural_0 device*)kernelContext_2->ed_buffers_0[i_2];
}

[[kernel]] void abi_compute(uint3 id_0 [[thread_position_in_grid]], C_0 constant* ed_c_1 [[buffer(0)]], ulong device* ed_buffers_1 [[buffer(2)]], ulong device* ed_textures_1 [[buffer(1)]], ulong device* ed_samplers_1 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_3;
    (&kernelContext_3)->ed_c_0 = ed_c_1;
    (&kernelContext_3)->ed_buffers_0 = ed_buffers_1;
    (&kernelContext_3)->ed_textures_0 = ed_textures_1;
    (&kernelContext_3)->ed_samplers_0 = ed_samplers_1;
    uint _S1 = id_0.x;
    uint one_0;
    if(_S1 < (ed_c_1->n_0))
    {
        Item_natural_0 device* _S2 = ed_buf_2((&kernelContext_3)->ed_c_0->src_0, &kernelContext_3);
        Item_natural_0 s_0 = *(_S2 + _S1);
        Item_natural_0 device* _S3 = ed_buf_2((&kernelContext_3)->ed_c_0->dst_0, &kernelContext_3);
        Item_natural_0 device* _S4 = _S3 + _S1;
        _S4->a_0 = packed_float4((float4(s_0.a_0)  * float4(2.0f) )) ;
        _S4->b_0 = packed_uint4((uint4(s_0.b_0)  + uint4(1U, 2U, 3U, 4U))) ;
        one_0 = 1U;
    }
    else
    {
        one_0 = 0U;
    }
    uint sum_0 = simd_sum(one_0);
    bool _S5 = simd_is_first();
    bool _S6;
    if(_S5)
    {
        _S6 = sum_0 != 0U;
    }
    else
    {
        _S6 = false;
    }
    if(_S6)
    {
        uint device* _S7 = ed_buf_1((&kernelContext_3)->ed_c_0->counter_0, &kernelContext_3);
        uint _S8 = atomic_fetch_add_explicit((atomic<uint> device*)((ulong)(_S7)) + int(0), sum_0, memory_order_relaxed);
    }
    return;
}

float ed_depth_read_0(uint i_3, uint2 p_0, KernelContext_0 thread* kernelContext_4)
{
    float _S9 = ((reinterpret_cast<device depth2d<float, access::sample>*>((kernelContext_4->ed_textures_0))[(i_3)].read((p_0))));
    return _S9;
}

[[kernel]] void abi_textures(uint3 id_1 [[thread_position_in_grid]], C_0 constant* ed_c_2 [[buffer(0)]], ulong device* ed_buffers_2 [[buffer(2)]], ulong device* ed_textures_2 [[buffer(1)]], ulong device* ed_samplers_2 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_5;
    (&kernelContext_5)->ed_c_0 = ed_c_2;
    (&kernelContext_5)->ed_buffers_0 = ed_buffers_2;
    (&kernelContext_5)->ed_textures_0 = ed_textures_2;
    (&kernelContext_5)->ed_samplers_0 = ed_samplers_2;
    texture2d<uint, access::sample> _S10 = ((reinterpret_cast<device texture2d<uint, access::sample>*>((ed_textures_2))[(ed_c_2->utex_0)]));
    uint _S11 = id_1.x;
    uint _S12 = id_1.y;
    int3 _S13 = int3(int(_S11 * 8U + 4U), int(63U - _S12), int(0));
    uint4 u_0 = ((_S10).read(vec<uint,2>(((_S13)).xy), uint(((_S13)).z)));
    uint2 _S14 = id_1.xy;
    float _S15 = ed_depth_read_0(ed_c_2->depth_0, _S14, &kernelContext_5);
    texture2d<float, access::sample> _S16 = ((reinterpret_cast<device texture2d<float, access::sample>*>(((&kernelContext_5)->ed_textures_0))[((&kernelContext_5)->ed_c_0->tex_0)]));
    int3 _S17 = int3(int2(_S14), int(0));
    float4 f_0 = ((_S16).read(vec<uint,2>(((_S17)).xy), uint(((_S17)).z)));
    texture2d<float, access::sample> _S18 = ((reinterpret_cast<device texture2d<float, access::sample>*>(((&kernelContext_5)->ed_textures_0))[((&kernelContext_5)->ed_c_0->tex_0)]));
    sampler _S19 = ((reinterpret_cast<device sampler*>(((&kernelContext_5)->ed_samplers_0))[((&kernelContext_5)->ed_c_0->smp_0)]));
    float4 s_1 = ((_S18).sample((_S19), ((float2(_S14) + float2(0.5f) ) / float2(8.0f) ), level((0.0f))));
    packed_uint4 device* _S20 = ed_buf_0((&kernelContext_5)->ed_c_0->dst_0, &kernelContext_5);
    packed_uint4 device* _S21 = _S20 + (_S12 * 8U + _S11) * 2U;
    *(_S21 + int(0)) = packed_uint4(uint4(u_0.x, u_0.y, (as_type<uint>((_S15))), 0U)) ;
    packed_uint4 device* _S22 = _S21 + int(1);
    *_S22 = packed_uint4(uint4(f_0 * float4(255.0f)  + float4(0.5f) )) ;
    (*_S22)[int(3)] = uint(s_1.x * 255.0f + 0.5f);
    return;
}

struct pixelOutput_0
{
    uint4 output_0 [[color(0)]];
};

struct pixelInput_0
{
    [[flat]] uint inst_0 [[user(INST)]];
};

[[fragment]] pixelOutput_0 abi_fs(pixelInput_0 _S23 [[stage_in]], uint prim_0 [[primitive_id]], float4 pos_0 [[position]], C_0 constant* ed_c_3 [[buffer(0)]], ulong device* ed_buffers_3 [[buffer(2)]], ulong device* ed_textures_3 [[buffer(1)]], ulong device* ed_samplers_3 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_6;
    (&kernelContext_6)->ed_c_0 = ed_c_3;
    (&kernelContext_6)->ed_buffers_0 = ed_buffers_3;
    (&kernelContext_6)->ed_textures_0 = ed_textures_3;
    (&kernelContext_6)->ed_samplers_0 = ed_samplers_3;
    pixelOutput_0 _S24 = { uint4(_S23.inst_0 + 1U, prim_0, 0U, 0U) };
    return _S24;
}

struct abi_vs_Result_0
{
    float4 pos_1 [[position]];
    uint inst_1 [[user(INST)]];
};

struct VsOut_0
{
    float4 pos_2;
    [[flat]] uint inst_2;
};

[[vertex]] abi_vs_Result_0 abi_vs(uint vid_0 [[vertex_id]], uint iid_0 [[instance_id]], C_0 constant* ed_c_4 [[buffer(0)]], ulong device* ed_buffers_4 [[buffer(2)]], ulong device* ed_textures_4 [[buffer(1)]], ulong device* ed_samplers_4 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_7;
    (&kernelContext_7)->ed_c_0 = ed_c_4;
    (&kernelContext_7)->ed_buffers_0 = ed_buffers_4;
    (&kernelContext_7)->ed_textures_0 = ed_textures_4;
    (&kernelContext_7)->ed_samplers_0 = ed_samplers_4;
    uint k_0 = vid_0 % 3U;
    float w_0 = 2.0f / float(ed_c_4->n_0);
    float x0_0 = -1.0f + w_0 * float(vid_0 / 3U);
    float2 p_1;
    if(k_0 == 0U)
    {
        p_1 = float2(x0_0, -1.0f);
    }
    else
    {
        if(k_0 == 1U)
        {
            p_1 = float2(x0_0 + w_0, -1.0f);
        }
        else
        {
            p_1 = float2(x0_0 + w_0 * 0.5f, 1.0f);
        }
    }
    thread VsOut_0 o_0;
    (&o_0)->pos_2 = float4(p_1, 0.5f, 1.0f);
    (&o_0)->inst_2 = iid_0;
    VsOut_0 _S25 = o_0;
    thread abi_vs_Result_0 _S26;
    (&_S26)->pos_1 = _S25.pos_2;
    (&_S26)->inst_1 = _S25.inst_2;
    return _S26;
}

