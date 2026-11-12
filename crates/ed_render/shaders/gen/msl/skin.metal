#include <metal_stdlib>
#include <metal_math>
#include <metal_texture>
using namespace metal;
struct Vtx_0
{
    float px_0;
    float py_0;
    float pz_0;
    uint n_0;
};

struct SkinC_0
{
    uint src_0;
    uint dst_0;
    uint palette_0;
    uint count_0;
    uint src_base_0;
    uint dst_base_0;
    uint pal_base_0;
    array<uint, int(9)> pad_0;
};

struct KernelContext_0
{
    SkinC_0 constant* ed_c_0;
    ulong device* ed_buffers_0;
};

Vtx_0 device* ed_buf_0(uint i_0, KernelContext_0 thread* kernelContext_0)
{
    return (Vtx_0 device*)kernelContext_0->ed_buffers_0[i_0];
}

packed_float4 device* ed_buf_1(uint i_1, KernelContext_0 thread* kernelContext_1)
{
    return (packed_float4 device*)kernelContext_1->ed_buffers_0[i_1];
}

struct SkinVtx_0
{
    float px_1;
    float py_1;
    float pz_1;
    uint n_1;
    uint j01_0;
    uint j23_0;
    array<float, int(4)> w_0;
};

SkinVtx_0 device* ed_buf_2(uint i_2, KernelContext_0 thread* kernelContext_2)
{
    return (SkinVtx_0 device*)kernelContext_2->ed_buffers_0[i_2];
}

float2 unpack_snorm2x16_0(uint p_0)
{
    return clamp(float2(int2(int(p_0 << 16U) >> 16U, int(p_0) >> 16U)) / float2(32767.0f) , float2(-1.0f) , float2(1.0f) );
}

float3 oct_decode_0(uint packed_0)
{
    float2 e_0 = unpack_snorm2x16_0(packed_0);
    float _S1 = e_0.x;
    float _S2 = e_0.y;
    float _S3 = 1.0f - abs(_S1) - abs(_S2);
    thread float3 n_2 = float3(_S1, _S2, _S3);
    float t_0 = saturate(- _S3);
    float _S4;
    if(_S1 >= 0.0f)
    {
        _S4 = - t_0;
    }
    else
    {
        _S4 = t_0;
    }
    n_2.x = n_2.x + _S4;
    if((n_2.y) >= 0.0f)
    {
        _S4 = - t_0;
    }
    else
    {
        _S4 = t_0;
    }
    n_2.y = n_2.y + _S4;
    return normalize(n_2);
}

float length_squared_0(float3 v_0)
{
    return dot(v_0, v_0);
}

uint pack_snorm2x16_0(float2 v_1)
{
    float2 s_0 = clamp(v_1, float2(-1.0f) , float2(1.0f) ) * float2(32767.0f) ;
    int2 _S5 = int2(float2((vec<int,2>(sign((s_0))))) * floor(abs(s_0) + float2(0.5f) ));
    return (uint(_S5.x) & 65535U) | (uint(_S5.y) << 16U);
}

uint oct_encode_0(float3 n_3)
{
    float3 _S6 = n_3 / float3(max(abs(n_3.x) + abs(n_3.y) + abs(n_3.z), 9.99999968265522539e-21f)) ;
    float2 e_1 = _S6.xy;
    float2 e_2;
    if((_S6.z) < 0.0f)
    {
        e_2 = (float2(1.0f)  - abs(e_1.yx)) * select(float2(-1.0f) , float2(1.0f) , e_1 >= (float2(0.0f) ));
    }
    else
    {
        e_2 = e_1;
    }
    return pack_snorm2x16_0(e_2);
}

[[kernel]] void skin(uint3 tid_0 [[thread_position_in_grid]], SkinC_0 constant* ed_c_1 [[buffer(0)]], ulong device* ed_buffers_1 [[buffer(2)]])
{
    thread KernelContext_0 kernelContext_3;
    (&kernelContext_3)->ed_c_0 = ed_c_1;
    (&kernelContext_3)->ed_buffers_0 = ed_buffers_1;
    uint id_0 = tid_0.x;
    if(id_0 >= (ed_c_1->count_0))
    {
        return;
    }
    SkinVtx_0 device* _S7 = ed_buf_2((&kernelContext_3)->ed_c_0->src_0, &kernelContext_3);
    SkinVtx_0 s_1 = *(_S7 + ((&kernelContext_3)->ed_c_0->src_base_0 + id_0));
    packed_float4 device* _S8 = ed_buf_1((&kernelContext_3)->ed_c_0->palette_0, &kernelContext_3);
    packed_float4 device* _S9 = _S8 + (&kernelContext_3)->ed_c_0->pal_base_0 * 3U;
    array<uint, int(4)> _S10 = { { (s_1.j01_0) & 65535U, (s_1.j01_0) >> 16U, (s_1.j23_0) & 65535U, (s_1.j23_0) >> 16U } };
    float3 bp_0 = float3(s_1.px_1, s_1.py_1, s_1.pz_1);
    float3 bn_0 = oct_decode_0(s_1.n_1);
    float3 _S11 = float3(0.0f) ;
    uint k_0 = 0U;
    float3 p_1 = _S11;
    float3 n_4 = _S11;
    float wsum_0 = 0.0f;
    for(;;)
    {
        if(k_0 < 4U)
        {
        }
        else
        {
            break;
        }
        uint _S12 = k_0;
        if((s_1.w_0[k_0]) <= 0.0f)
        {
            k_0 = k_0 + 1U;
            continue;
        }
        uint _S13 = _S10[k_0] * 3U;
        float4 _S14 = float4(*(_S9 + _S13)) ;
        float4 _S15 = float4(*(_S9 + (_S13 + 1U))) ;
        float4 _S16 = float4(*(_S9 + (_S13 + 2U))) ;
        float3 _S17 = _S14.xyz;
        float3 _S18 = _S15.xyz;
        float3 _S19 = _S16.xyz;
        float3 _S20 = float3(s_1.w_0[_S12]) ;
        float3 n_5 = n_4 + _S20 * float3(dot(_S17, bn_0), dot(_S18, bn_0), dot(_S19, bn_0));
        float wsum_1 = wsum_0 + s_1.w_0[_S12];
        p_1 = p_1 + _S20 * float3(dot(_S17, bp_0) + _S14.w, dot(_S18, bp_0) + _S15.w, dot(_S19, bp_0) + _S16.w);
        n_4 = n_5;
        wsum_0 = wsum_1;
        k_0 = k_0 + 1U;
    }
    if(wsum_0 <= 0.0f)
    {
        p_1 = bp_0;
        n_4 = bn_0;
    }
    else
    {
        p_1 = p_1 / float3(wsum_0) ;
    }
    Vtx_0 device* _S21 = ed_buf_0((&kernelContext_3)->ed_c_0->dst_0, &kernelContext_3);
    Vtx_0 device* o_0 = _S21 + (&kernelContext_3)->ed_c_0->dst_base_0 + id_0;
    o_0->px_0 = p_1.x;
    o_0->py_0 = p_1.y;
    o_0->pz_0 = p_1.z;
    if((length_squared_0(n_4)) > 0.0f)
    {
        p_1 = normalize(n_4);
    }
    else
    {
        p_1 = bn_0;
    }
    o_0->n_0 = oct_encode_0(p_1);
    return;
}

