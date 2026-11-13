#include <metal_stdlib>
#include <metal_math>
#include <metal_texture>
using namespace metal;
struct PickC_0
{
    uint vis_0;
    uint instances_0;
    uint out_0;
    uint x_0;
    uint y_0;
    uint w_0;
    uint h_0;
    array<uint, int(9)> pad_0;
};

struct KernelContext_0
{
    PickC_0 constant* ed_c_0;
    ulong device* ed_buffers_0;
    ulong device* ed_textures_0;
};

float device* ed_buf_0(uint i_0, KernelContext_0 thread* kernelContext_0)
{
    return (float device*)kernelContext_0->ed_buffers_0[i_0];
}

struct _Array_natural_vectorx3Cfloatx2C4x3E3_0
{
    array<packed_float4, int(3)> data_0;
};

struct GpuInstance_natural_0
{
    _Array_natural_vectorx3Cfloatx2C4x3E3_0 m_0;
    _Array_natural_vectorx3Cfloatx2C4x3E3_0 prev_0;
    uint mesh_0;
    uint material_0;
    uint object_lo_0;
    uint node_flags_0;
    array<uint, int(4)> pad_1;
};

GpuInstance_natural_0 device* ed_buf_1(uint i_1, KernelContext_0 thread* kernelContext_1)
{
    return (GpuInstance_natural_0 device*)kernelContext_1->ed_buffers_0[i_1];
}

uint device* ed_buf_2(uint i_2, KernelContext_0 thread* kernelContext_2)
{
    return (uint device*)kernelContext_2->ed_buffers_0[i_2];
}

[[kernel]] void pick(uint3 tid_0 [[thread_position_in_grid]], PickC_0 constant* ed_c_1 [[buffer(0)]], ulong device* ed_buffers_1 [[buffer(2)]], ulong device* ed_textures_1 [[buffer(1)]])
{
    thread KernelContext_0 kernelContext_3;
    (&kernelContext_3)->ed_c_0 = ed_c_1;
    (&kernelContext_3)->ed_buffers_0 = ed_buffers_1;
    (&kernelContext_3)->ed_textures_0 = ed_textures_1;
    if((tid_0.x) != 0U)
    {
        return;
    }
    uint device* _S1 = ed_buf_2((&kernelContext_3)->ed_c_0->out_0, &kernelContext_3);
    thread uint2 v_0 = uint2(int2(int(0)) );
    bool _S2;
    if(((&kernelContext_3)->ed_c_0->x_0) < ((&kernelContext_3)->ed_c_0->w_0))
    {
        _S2 = ((&kernelContext_3)->ed_c_0->y_0) < ((&kernelContext_3)->ed_c_0->h_0);
    }
    else
    {
        _S2 = false;
    }
    if(_S2)
    {
        texture2d<uint, access::sample> _S3 = ((reinterpret_cast<device texture2d<uint, access::sample>*>(((&kernelContext_3)->ed_textures_0))[((&kernelContext_3)->ed_c_0->vis_0)]));
        int3 _S4 = int3(int((&kernelContext_3)->ed_c_0->x_0), int((&kernelContext_3)->ed_c_0->y_0), int(0));
        v_0 = ((_S3).read(vec<uint,2>(((_S4)).xy), uint(((_S4)).z))).xy;
    }
    if(((v_0.x) & 3221225472U) != 0U)
    {
        v_0.x = 0U;
    }
    *(_S1 + int(0)) = v_0.x;
    uint device* _S5 = _S1 + int(1);
    uint _S6;
    if((v_0.x) == 0U)
    {
        _S6 = 0U;
    }
    else
    {
        GpuInstance_natural_0 device* _S7 = ed_buf_1((&kernelContext_3)->ed_c_0->instances_0, &kernelContext_3);
        _S6 = (_S7 + (v_0.x - 1U))->object_lo_0;
    }
    *_S5 = _S6;
    *(_S1 + int(2)) = v_0.y;
    *(_S1 + int(3)) = 0U;
    return;
}

struct PopC_0
{
    uint hdr_0;
    uint vis_1;
    uint state_0;
    uint out_1;
    uint w_1;
    uint h_1;
    uint tiles_x_0;
    uint tiles_y_0;
    float exposure_0;
    uint lut_0;
    array<uint, int(6)> pad_2;
};

struct AnyValue64
{
    uint field0_0;
    uint field1_0;
    uint field2_0;
    uint field3_0;
    uint field4_0;
    uint field5_0;
    uint field6_0;
    uint field7_0;
    uint field8_0;
    uint field9_0;
    uint field10_0;
    uint field11_0;
    uint field12_0;
    uint field13_0;
    uint field14_0;
    uint field15_0;
};

PopC_0 unpackAnyValue64_0(const AnyValue64 thread* _S8)
{
    thread PopC_0 _S9;
    (&_S9)->hdr_0 = _S8->field0_0;
    (&_S9)->vis_1 = _S8->field1_0;
    (&_S9)->state_0 = _S8->field2_0;
    (&_S9)->out_1 = _S8->field3_0;
    (&_S9)->w_1 = _S8->field4_0;
    (&_S9)->h_1 = _S8->field5_0;
    (&_S9)->tiles_x_0 = _S8->field6_0;
    (&_S9)->tiles_y_0 = _S8->field7_0;
    (&_S9)->exposure_0 = as_type<float>(_S8->field8_0);
    (&_S9)->lut_0 = _S8->field9_0;
    (&_S9)->pad_2[int(0)] = _S8->field10_0;
    (&_S9)->pad_2[int(1)] = _S8->field11_0;
    (&_S9)->pad_2[int(2)] = _S8->field12_0;
    (&_S9)->pad_2[int(3)] = _S8->field13_0;
    (&_S9)->pad_2[int(4)] = _S8->field14_0;
    (&_S9)->pad_2[int(5)] = _S8->field15_0;
    return _S9;
}

AnyValue64 packAnyValue64_0(KernelContext_0 thread* kernelContext_4)
{
    thread AnyValue64 _S10;
    (&_S10)->field0_0 = kernelContext_4->ed_c_0->vis_0;
    (&_S10)->field1_0 = kernelContext_4->ed_c_0->instances_0;
    (&_S10)->field2_0 = kernelContext_4->ed_c_0->out_0;
    (&_S10)->field3_0 = kernelContext_4->ed_c_0->x_0;
    (&_S10)->field4_0 = kernelContext_4->ed_c_0->y_0;
    (&_S10)->field5_0 = kernelContext_4->ed_c_0->w_0;
    (&_S10)->field6_0 = kernelContext_4->ed_c_0->h_0;
    (&_S10)->field7_0 = kernelContext_4->ed_c_0->pad_0[int(0)];
    (&_S10)->field8_0 = kernelContext_4->ed_c_0->pad_0[int(1)];
    (&_S10)->field9_0 = kernelContext_4->ed_c_0->pad_0[int(2)];
    (&_S10)->field10_0 = kernelContext_4->ed_c_0->pad_0[int(3)];
    (&_S10)->field11_0 = kernelContext_4->ed_c_0->pad_0[int(4)];
    (&_S10)->field12_0 = kernelContext_4->ed_c_0->pad_0[int(5)];
    (&_S10)->field13_0 = kernelContext_4->ed_c_0->pad_0[int(6)];
    (&_S10)->field14_0 = kernelContext_4->ed_c_0->pad_0[int(7)];
    (&_S10)->field15_0 = kernelContext_4->ed_c_0->pad_0[int(8)];
    return _S10;
}

PopC_0 pop_c_0(KernelContext_0 thread* kernelContext_5)
{
    AnyValue64 _S11 = packAnyValue64_0(kernelContext_5);
    thread AnyValue64 _S12 = _S11;
    PopC_0 _S13 = unpackAnyValue64_0(&_S12);
    return _S13;
}

float half_bits_to_float_0(uint h_2)
{
    uint e_0 = (h_2 >> 10U) & 31U;
    uint m_1 = h_2 & 1023U;
    uint _S14 = h_2 & 32768U;
    float s_0;
    if(_S14 != 0U)
    {
        s_0 = -1.0f;
    }
    else
    {
        s_0 = 1.0f;
    }
    if(e_0 == 0U)
    {
        return s_0 * float(m_1) * 5.9604644775390625e-08f;
    }
    if(e_0 == 31U)
    {
        return (as_type<float>((((_S14 << 16U) | 2139095040U) | (m_1 << 13U))));
    }
    return (as_type<float>((((_S14 << 16U) | ((e_0 + 112U) << 23U)) | (m_1 << 13U))));
}

float3 lut_texel_0(uint device* lut_1, uint3 t_0)
{
    uint i_3 = ((t_0.z * 64U + t_0.y) * 64U + t_0.x) * 2U;
    uint device* _S15 = lut_1 + i_3;
    return float3(half_bits_to_float_0((*_S15) & 65535U), half_bits_to_float_0((*_S15) >> 16U), half_bits_to_float_0((*(lut_1 + (i_3 + 1U))) & 65535U));
}

float3 filmic_0(uint lut_buf_0, float3 x_1, KernelContext_0 thread* kernelContext_6)
{
    uint device* _S16 = ed_buf_2(lut_buf_0, kernelContext_6);
    float3 _S17 = max(x_1, float3(0.0f) );
    float3 t_1 = saturate((log2(select(_S17, _S17 + float3(1.5258779967552982e-05f) , _S17 < (float3(3.0517570849042386e-05f) )) / float3(0.18000000715255737f) ) + float3(11.0f) ) / float3(23.0f) ) * float3(63.0f) ;
    uint3 _S18 = min(uint3(t_1), uint3(62U) );
    float3 g_0 = t_1 - float3(_S18);
    float3 _S19 = lut_texel_0(_S16, _S18);
    float3 _S20 = lut_texel_0(_S16, _S18 + uint3(1U, 0U, 0U));
    float3 _S21 = float3(g_0.x) ;
    float3 c00_0 = mix(_S19, _S20, _S21);
    float3 _S22 = lut_texel_0(_S16, _S18 + uint3(0U, 1U, 0U));
    float3 _S23 = lut_texel_0(_S16, _S18 + uint3(1U, 1U, 0U));
    float3 c10_0 = mix(_S22, _S23, _S21);
    float3 _S24 = lut_texel_0(_S16, _S18 + uint3(0U, 0U, 1U));
    float3 _S25 = lut_texel_0(_S16, _S18 + uint3(1U, 0U, 1U));
    float3 c01_0 = mix(_S24, _S25, _S21);
    float3 _S26 = lut_texel_0(_S16, _S18 + uint3(0U, 1U, 1U));
    float3 _S27 = lut_texel_0(_S16, _S18 + uint3(1U, 1U, 1U));
    float3 _S28 = float3(g_0.y) ;
    return mix(mix(c00_0, c10_0, _S28), mix(c01_0, mix(_S26, _S27, _S21), _S28), float3(g_0.z) );
}

float3 alpha_tonemap_0(uint lut_buf_1, float3 hdr_1, float exposure_1, KernelContext_0 thread* kernelContext_7)
{
    float3 _S29 = filmic_0(lut_buf_1, hdr_1 * float3(exposure_1) , kernelContext_7);
    return pow(saturate(_S29 * mix(float3(1.0f) , float3(1.0f, 1.0f, 1.0f), float3(smoothstep(0.34999999403953552f, 0.89999997615814209f, dot(_S29, float3(0.2125999927520752f, 0.71520000696182251f, 0.072200000286102295f)))) )), float3(0.45454543828964233f) );
}

[[kernel]] void pop_stats(uint3 tid_1 [[thread_position_in_grid]], PickC_0 constant* ed_c_2 [[buffer(0)]], ulong device* ed_buffers_2 [[buffer(2)]], ulong device* ed_textures_2 [[buffer(1)]])
{
    thread KernelContext_0 kernelContext_8;
    (&kernelContext_8)->ed_c_0 = ed_c_2;
    (&kernelContext_8)->ed_buffers_0 = ed_buffers_2;
    (&kernelContext_8)->ed_textures_0 = ed_textures_2;
    uint2 t_2 = tid_1.xy;
    PopC_0 _S30 = pop_c_0(&kernelContext_8);
    uint _S31 = t_2.x;
    bool _S32;
    if(_S31 >= (_S30.tiles_x_0))
    {
        _S32 = true;
    }
    else
    {
        _S32 = (t_2.y) >= (_S30.tiles_y_0);
    }
    if(_S32)
    {
        return;
    }
    texture2d<float, access::sample> _S33 = ((reinterpret_cast<device texture2d<float, access::sample>*>(((&kernelContext_8)->ed_textures_0))[(_S30.hdr_0)]));
    texture2d<uint, access::sample> _S34 = ((reinterpret_cast<device texture2d<uint, access::sample>*>(((&kernelContext_8)->ed_textures_0))[(_S30.vis_1)]));
    uint device* _S35 = ed_buf_2(_S30.state_0, &kernelContext_8);
    uint y_1 = 0U;
    float sum_0 = 0.0f;
    uint n_0 = 0U;
    uint trans_0 = 0U;
    for(;;)
    {
        if(y_1 < 8U)
        {
        }
        else
        {
            break;
        }
        uint x_2 = 0U;
        float sum_1 = sum_0;
        uint n_1 = n_0;
        uint trans_1 = trans_0;
        for(;;)
        {
            if(x_2 < 8U)
            {
            }
            else
            {
                break;
            }
            uint2 p_0 = t_2 * uint2(8U)  + uint2(x_2, y_1);
            if((p_0.x) >= (_S30.w_1))
            {
                _S32 = true;
            }
            else
            {
                _S32 = (p_0.y) >= (_S30.h_1);
            }
            uint trans_2;
            if(_S32)
            {
                trans_2 = trans_1;
                x_2 = x_2 + 1U;
                trans_1 = trans_2;
                continue;
            }
            int3 _S36 = int3(int2(p_0), int(0));
            float3 _S37 = alpha_tonemap_0(_S30.lut_0, ((_S33).read(vec<uint,2>(((_S36)).xy), uint(((_S36)).z))).xyz, _S30.exposure_0, &kernelContext_8);
            float sum_2 = sum_1 + dot(_S37, float3(0.2125999927520752f, 0.71520000696182251f, 0.072200000286102295f));
            uint n_2 = n_1 + 1U;
            uint id_0 = ((_S34).read(vec<uint,2>(((_S36)).xy), uint(((_S36)).z))).x;
            bool _S38;
            if(id_0 != 0U)
            {
                _S38 = (id_0 & 3221225472U) == 0U;
            }
            else
            {
                _S38 = false;
            }
            if(_S38)
            {
                uint device* _S39 = _S35 + (id_0 - 1U);
                uint st_0 = *_S39;
                bool _S40;
                if((((*_S39) >> 4U) & 63U) != 0U)
                {
                    _S40 = true;
                }
                else
                {
                    _S40 = (st_0 & 4096U) != 0U;
                }
                if(_S40)
                {
                    trans_2 = trans_1 + 1U;
                }
                else
                {
                    trans_2 = trans_1;
                }
            }
            else
            {
                trans_2 = trans_1;
            }
            sum_1 = sum_2;
            n_1 = n_2;
            x_2 = x_2 + 1U;
            trans_1 = trans_2;
        }
        y_1 = y_1 + 1U;
        sum_0 = sum_1;
        n_0 = n_1;
        trans_0 = trans_1;
    }
    float device* _S41 = ed_buf_0(_S30.out_1, &kernelContext_8);
    uint i_4 = (t_2.y * _S30.tiles_x_0 + _S31) * 2U;
    float device* _S42 = _S41 + i_4;
    if(n_0 > 0U)
    {
        sum_0 = sum_0 / float(n_0);
    }
    else
    {
        sum_0 = 0.0f;
    }
    *_S42 = sum_0;
    *(_S41 + (i_4 + 1U)) = float(trans_0);
    return;
}

