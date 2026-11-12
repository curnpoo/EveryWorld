#include <metal_stdlib>
#include <metal_math>
#include <metal_texture>
using namespace metal;
struct HzbC_0
{
    uint hzb_0;
    uint depth_0;
    uint src_off_0;
    uint src_w_0;
    uint src_h_0;
    uint dst_off_0;
    uint dst_w_0;
    uint dst_h_0;
    array<uint, int(8)> pad_0;
};

struct KernelContext_0
{
    HzbC_0 constant* ed_c_0;
    ulong device* ed_textures_0;
    ulong device* ed_buffers_0;
};

float device* ed_buf_0(uint i_0, KernelContext_0 thread* kernelContext_0)
{
    return (float device*)kernelContext_0->ed_buffers_0[i_0];
}

uint2 src_range_0(uint d_0, uint dst_n_0, uint src_n_0)
{
    uint _S1 = d_0 * 2U;
    uint _S2 = src_n_0 - 1U;
    uint _S3 = min(_S1, _S2);
    uint b_0;
    if((d_0 + 1U) == dst_n_0)
    {
        b_0 = _S2;
    }
    else
    {
        b_0 = min(_S1 + 1U, _S2);
    }
    return uint2(_S3, b_0);
}

float ed_depth_read_0(uint i_1, uint2 p_0, KernelContext_0 thread* kernelContext_1)
{
    float _S4 = ((reinterpret_cast<device depth2d<float, access::sample>*>((kernelContext_1->ed_textures_0))[(i_1)].read((p_0))));
    return _S4;
}

[[kernel]] void hzb_first(uint3 tid_0 [[thread_position_in_grid]], HzbC_0 constant* ed_c_1 [[buffer(0)]], ulong device* ed_textures_1 [[buffer(1)]], ulong device* ed_buffers_1 [[buffer(2)]])
{
    thread KernelContext_0 kernelContext_2;
    (&kernelContext_2)->ed_c_0 = ed_c_1;
    (&kernelContext_2)->ed_textures_0 = ed_textures_1;
    (&kernelContext_2)->ed_buffers_0 = ed_buffers_1;
    uint2 t_0 = tid_0.xy;
    uint _S5 = t_0.x;
    bool _S6;
    if(_S5 >= (ed_c_1->dst_w_0))
    {
        _S6 = true;
    }
    else
    {
        _S6 = (t_0.y) >= ((&kernelContext_2)->ed_c_0->dst_h_0);
    }
    if(_S6)
    {
        return;
    }
    uint2 _S7 = src_range_0(_S5, ed_c_1->dst_w_0, (&kernelContext_2)->ed_c_0->src_w_0);
    uint _S8 = t_0.y;
    uint2 ry_0 = src_range_0(_S8, (&kernelContext_2)->ed_c_0->dst_h_0, (&kernelContext_2)->ed_c_0->src_h_0);
    uint _S9 = ry_0.x;
    float m_0 = 1.0f;
    uint y_0 = _S9;
    for(;;)
    {
        if(y_0 <= (ry_0.y))
        {
        }
        else
        {
            break;
        }
        uint x_0 = _S7.x;
        for(;;)
        {
            if(x_0 <= (_S7.y))
            {
            }
            else
            {
                break;
            }
            float _S10 = ed_depth_read_0((&kernelContext_2)->ed_c_0->depth_0, uint2(x_0, y_0), &kernelContext_2);
            float _S11 = min(m_0, _S10);
            uint _S12 = x_0 + 1U;
            m_0 = _S11;
            x_0 = _S12;
        }
        y_0 = y_0 + 1U;
    }
    float device* _S13 = ed_buf_0((&kernelContext_2)->ed_c_0->hzb_0, &kernelContext_2);
    *(_S13 + ((&kernelContext_2)->ed_c_0->dst_off_0 + _S8 * ed_c_1->dst_w_0 + _S5)) = m_0;
    return;
}

[[kernel]] void hzb_down(uint3 tid_1 [[thread_position_in_grid]], HzbC_0 constant* ed_c_2 [[buffer(0)]], ulong device* ed_textures_2 [[buffer(1)]], ulong device* ed_buffers_2 [[buffer(2)]])
{
    thread KernelContext_0 kernelContext_3;
    (&kernelContext_3)->ed_c_0 = ed_c_2;
    (&kernelContext_3)->ed_textures_0 = ed_textures_2;
    (&kernelContext_3)->ed_buffers_0 = ed_buffers_2;
    uint2 t_1 = tid_1.xy;
    uint _S14 = t_1.x;
    bool _S15;
    if(_S14 >= (ed_c_2->dst_w_0))
    {
        _S15 = true;
    }
    else
    {
        _S15 = (t_1.y) >= ((&kernelContext_3)->ed_c_0->dst_h_0);
    }
    if(_S15)
    {
        return;
    }
    float device* _S16 = ed_buf_0((&kernelContext_3)->ed_c_0->hzb_0, &kernelContext_3);
    uint2 _S17 = src_range_0(_S14, ed_c_2->dst_w_0, (&kernelContext_3)->ed_c_0->src_w_0);
    uint _S18 = t_1.y;
    uint2 ry_1 = src_range_0(_S18, (&kernelContext_3)->ed_c_0->dst_h_0, (&kernelContext_3)->ed_c_0->src_h_0);
    uint _S19 = ry_1.x;
    float m_1 = 1.0f;
    uint y_1 = _S19;
    for(;;)
    {
        if(y_1 <= (ry_1.y))
        {
        }
        else
        {
            break;
        }
        uint x_1 = _S17.x;
        for(;;)
        {
            if(x_1 <= (_S17.y))
            {
            }
            else
            {
                break;
            }
            float _S20 = min(m_1, *(_S16 + ((&kernelContext_3)->ed_c_0->src_off_0 + y_1 * (&kernelContext_3)->ed_c_0->src_w_0 + x_1)));
            uint _S21 = x_1 + 1U;
            m_1 = _S20;
            x_1 = _S21;
        }
        y_1 = y_1 + 1U;
    }
    *(_S16 + ((&kernelContext_3)->ed_c_0->dst_off_0 + _S18 * ed_c_2->dst_w_0 + _S14)) = m_1;
    return;
}

