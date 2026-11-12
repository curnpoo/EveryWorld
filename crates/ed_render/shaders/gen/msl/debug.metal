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
    uint2 _S2 = uint2(int2(int(0)) );
    bool _S3;
    if(((&kernelContext_3)->ed_c_0->x_0) < ((&kernelContext_3)->ed_c_0->w_0))
    {
        _S3 = ((&kernelContext_3)->ed_c_0->y_0) < ((&kernelContext_3)->ed_c_0->h_0);
    }
    else
    {
        _S3 = false;
    }
    uint2 v_0;
    if(_S3)
    {
        texture2d<uint, access::sample> _S4 = ((reinterpret_cast<device texture2d<uint, access::sample>*>(((&kernelContext_3)->ed_textures_0))[((&kernelContext_3)->ed_c_0->vis_0)]));
        int3 _S5 = int3(int((&kernelContext_3)->ed_c_0->x_0), int((&kernelContext_3)->ed_c_0->y_0), int(0));
        v_0 = ((_S4).read(vec<uint,2>(((_S5)).xy), uint(((_S5)).z))).xy;
    }
    else
    {
        v_0 = _S2;
    }
    uint _S6 = v_0.x;
    *(_S1 + int(0)) = _S6;
    uint device* _S7 = _S1 + int(1);
    uint _S8;
    if(_S6 == 0U)
    {
        _S8 = 0U;
    }
    else
    {
        GpuInstance_natural_0 device* _S9 = ed_buf_1((&kernelContext_3)->ed_c_0->instances_0, &kernelContext_3);
        _S8 = (_S9 + (_S6 - 1U))->object_lo_0;
    }
    *_S7 = _S8;
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
    array<uint, int(7)> pad_2;
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

PopC_0 unpackAnyValue64_0(const AnyValue64 thread* _S10)
{
    thread PopC_0 _S11;
    (&_S11)->hdr_0 = _S10->field0_0;
    (&_S11)->vis_1 = _S10->field1_0;
    (&_S11)->state_0 = _S10->field2_0;
    (&_S11)->out_1 = _S10->field3_0;
    (&_S11)->w_1 = _S10->field4_0;
    (&_S11)->h_1 = _S10->field5_0;
    (&_S11)->tiles_x_0 = _S10->field6_0;
    (&_S11)->tiles_y_0 = _S10->field7_0;
    (&_S11)->exposure_0 = as_type<float>(_S10->field8_0);
    (&_S11)->pad_2[int(0)] = _S10->field9_0;
    (&_S11)->pad_2[int(1)] = _S10->field10_0;
    (&_S11)->pad_2[int(2)] = _S10->field11_0;
    (&_S11)->pad_2[int(3)] = _S10->field12_0;
    (&_S11)->pad_2[int(4)] = _S10->field13_0;
    (&_S11)->pad_2[int(5)] = _S10->field14_0;
    (&_S11)->pad_2[int(6)] = _S10->field15_0;
    return _S11;
}

AnyValue64 packAnyValue64_0(KernelContext_0 thread* kernelContext_4)
{
    thread AnyValue64 _S12;
    (&_S12)->field0_0 = kernelContext_4->ed_c_0->vis_0;
    (&_S12)->field1_0 = kernelContext_4->ed_c_0->instances_0;
    (&_S12)->field2_0 = kernelContext_4->ed_c_0->out_0;
    (&_S12)->field3_0 = kernelContext_4->ed_c_0->x_0;
    (&_S12)->field4_0 = kernelContext_4->ed_c_0->y_0;
    (&_S12)->field5_0 = kernelContext_4->ed_c_0->w_0;
    (&_S12)->field6_0 = kernelContext_4->ed_c_0->h_0;
    (&_S12)->field7_0 = kernelContext_4->ed_c_0->pad_0[int(0)];
    (&_S12)->field8_0 = kernelContext_4->ed_c_0->pad_0[int(1)];
    (&_S12)->field9_0 = kernelContext_4->ed_c_0->pad_0[int(2)];
    (&_S12)->field10_0 = kernelContext_4->ed_c_0->pad_0[int(3)];
    (&_S12)->field11_0 = kernelContext_4->ed_c_0->pad_0[int(4)];
    (&_S12)->field12_0 = kernelContext_4->ed_c_0->pad_0[int(5)];
    (&_S12)->field13_0 = kernelContext_4->ed_c_0->pad_0[int(6)];
    (&_S12)->field14_0 = kernelContext_4->ed_c_0->pad_0[int(7)];
    (&_S12)->field15_0 = kernelContext_4->ed_c_0->pad_0[int(8)];
    return _S12;
}

PopC_0 pop_c_0(KernelContext_0 thread* kernelContext_5)
{
    AnyValue64 _S13 = packAnyValue64_0(kernelContext_5);
    thread AnyValue64 _S14 = _S13;
    PopC_0 _S15 = unpackAnyValue64_0(&_S14);
    return _S15;
}

float3 aces_fit_0(float3 x_1)
{
    return saturate(x_1 * (float3(2.5099999904632568f)  * x_1 + float3(0.029999999329447746f) ) / (x_1 * (float3(2.4300000667572021f)  * x_1 + float3(0.5899999737739563f) ) + float3(0.14000000059604645f) ));
}

[[kernel]] void pop_stats(uint3 tid_1 [[thread_position_in_grid]], PickC_0 constant* ed_c_2 [[buffer(0)]], ulong device* ed_buffers_2 [[buffer(2)]], ulong device* ed_textures_2 [[buffer(1)]])
{
    thread KernelContext_0 kernelContext_6;
    (&kernelContext_6)->ed_c_0 = ed_c_2;
    (&kernelContext_6)->ed_buffers_0 = ed_buffers_2;
    (&kernelContext_6)->ed_textures_0 = ed_textures_2;
    uint2 t_0 = tid_1.xy;
    PopC_0 _S16 = pop_c_0(&kernelContext_6);
    uint _S17 = t_0.x;
    bool _S18;
    if(_S17 >= (_S16.tiles_x_0))
    {
        _S18 = true;
    }
    else
    {
        _S18 = (t_0.y) >= (_S16.tiles_y_0);
    }
    if(_S18)
    {
        return;
    }
    texture2d<float, access::sample> _S19 = ((reinterpret_cast<device texture2d<float, access::sample>*>(((&kernelContext_6)->ed_textures_0))[(_S16.hdr_0)]));
    texture2d<uint, access::sample> _S20 = ((reinterpret_cast<device texture2d<uint, access::sample>*>(((&kernelContext_6)->ed_textures_0))[(_S16.vis_1)]));
    uint device* _S21 = ed_buf_2(_S16.state_0, &kernelContext_6);
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
            uint2 p_0 = t_0 * uint2(8U)  + uint2(x_2, y_1);
            if((p_0.x) >= (_S16.w_1))
            {
                _S18 = true;
            }
            else
            {
                _S18 = (p_0.y) >= (_S16.h_1);
            }
            uint trans_2;
            if(_S18)
            {
                trans_2 = trans_1;
                x_2 = x_2 + 1U;
                trans_1 = trans_2;
                continue;
            }
            int3 _S22 = int3(int2(p_0), int(0));
            float sum_2 = sum_1 + dot(aces_fit_0(((_S19).read(vec<uint,2>(((_S22)).xy), uint(((_S22)).z))).xyz * float3(_S16.exposure_0) ), float3(0.2125999927520752f, 0.71520000696182251f, 0.072200000286102295f));
            uint n_2 = n_1 + 1U;
            uint id_0 = ((_S20).read(vec<uint,2>(((_S22)).xy), uint(((_S22)).z))).x;
            if(id_0 != 0U)
            {
                uint device* _S23 = _S21 + (id_0 - 1U);
                uint st_0 = *_S23;
                bool _S24;
                if((((*_S23) >> 4U) & 63U) != 0U)
                {
                    _S24 = true;
                }
                else
                {
                    _S24 = (st_0 & 4096U) != 0U;
                }
                if(_S24)
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
    float device* _S25 = ed_buf_0(_S16.out_1, &kernelContext_6);
    uint i_3 = (t_0.y * _S16.tiles_x_0 + _S17) * 2U;
    float device* _S26 = _S25 + i_3;
    if(n_0 > 0U)
    {
        sum_0 = sum_0 / float(n_0);
    }
    else
    {
        sum_0 = 0.0f;
    }
    *_S26 = sum_0;
    *(_S25 + (i_3 + 1U)) = float(trans_0);
    return;
}

