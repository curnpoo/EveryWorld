#include <metal_stdlib>
#include <metal_math>
#include <metal_texture>
using namespace metal;
struct AoC_0
{
    uint frame_0;
    uint depth_0;
    uint raw_0;
    uint out_0;
    uint mode_0;
    uint hw_0;
    uint hh_0;
    float radius_0;
    array<uint, int(8)> pad_0;
};

struct KernelContext_0
{
    AoC_0 constant* ed_c_0;
    ulong device* ed_buffers_0;
    ulong device* ed_textures_0;
};

packed_float2 device* ed_buf_0(uint i_0, KernelContext_0 thread* kernelContext_0)
{
    return (packed_float2 device*)kernelContext_0->ed_buffers_0[i_0];
}

struct _MatrixStorage_float4x4_ColMajornatural_0
{
    array<packed_float4, int(4)> data_0;
};

struct _Array_natural_vectorx3Cfloatx2C4x3E5_0
{
    array<packed_float4, int(5)> data_1;
};

struct HzbLevel_0
{
    uint offset_0;
    uint width_0;
    uint height_0;
    uint pad_1;
};

struct _Array_natural_matrixx3Cfloatx2C4x2C4x3E4_0
{
    array<_MatrixStorage_float4x4_ColMajornatural_0, int(4)> data_2;
};

struct _Array_natural_vectorx3Cfloatx2C4x3E4_0
{
    array<packed_float4, int(4)> data_3;
};

struct _Array_natural_vectorx3Cfloatx2C4x3E9_0
{
    array<packed_float4, int(9)> data_4;
};

struct LightData_natural_0
{
    _Array_natural_matrixx3Cfloatx2C4x2C4x3E4_0 shadow_0;
    _Array_natural_vectorx3Cfloatx2C4x3E4_0 cascade_0;
    packed_float4 split_0;
    packed_float4 cfg_0;
    packed_float4 filter_0;
    packed_uint4 flags_0;
    packed_float4 ao_0;
    packed_float4 sky_0;
    _Array_natural_vectorx3Cfloatx2C4x3E9_0 sh_0;
};

struct FrameData_natural_0
{
    _MatrixStorage_float4x4_ColMajornatural_0 view_proj_0;
    _MatrixStorage_float4x4_ColMajornatural_0 inv_view_proj_0;
    _MatrixStorage_float4x4_ColMajornatural_0 view_0;
    _Array_natural_vectorx3Cfloatx2C4x3E5_0 planes_0;
    packed_float4 sun_dir_0;
    packed_float4 sun_color_0;
    packed_float4 sky_zenith_0;
    packed_float4 sky_horizon_0;
    packed_float4 ground_color_0;
    packed_float4 viewport_0;
    packed_float4 params_0;
    packed_float4 proj_0;
    packed_uint4 cull_0;
    array<HzbLevel_0, int(16)> hzb_0;
    LightData_natural_0 light_0;
};

FrameData_natural_0 device* ed_buf_1(uint i_1, KernelContext_0 thread* kernelContext_1)
{
    return (FrameData_natural_0 device*)kernelContext_1->ed_buffers_0[i_1];
}

float ed_depth_read_0(uint i_2, uint2 p_0, KernelContext_0 thread* kernelContext_2)
{
    float _S1 = ((reinterpret_cast<device depth2d<float, access::sample>*>((kernelContext_2->ed_textures_0))[(i_2)].read((p_0))));
    return _S1;
}

float linear_z_0(FrameData_natural_0 device* f_0, int2 q_0, KernelContext_0 thread* kernelContext_3)
{
    float _S2 = ed_depth_read_0(kernelContext_3->ed_c_0->depth_0, uint2(clamp(q_0, int2(int(0)) , int2((float4(f_0->proj_0) ).zw) - int2(int(1)) )), kernelContext_3);
    float _S3;
    if(_S2 > 0.0f)
    {
        _S3 = min((float4(f_0->params_0) ).z / _S2, 1.0e+06f);
    }
    else
    {
        _S3 = 1.0e+06f;
    }
    return _S3;
}

float linear_z_1(FrameData_natural_0 device* f_1, int2 q_1, KernelContext_0 thread* kernelContext_4)
{
    float _S4 = ed_depth_read_0(kernelContext_4->ed_c_0->depth_0, uint2(clamp(q_1, int2(int(0)) , int2((float4(f_1->proj_0) ).zw) - int2(int(1)) )), kernelContext_4);
    float _S5;
    if(_S4 > 0.0f)
    {
        _S5 = min((float4(f_1->params_0) ).z / _S4, 1.0e+06f);
    }
    else
    {
        _S5 = 1.0e+06f;
    }
    return _S5;
}

float3 view_pos_0(FrameData_natural_0 device* f_2, float2 q_2, float z_0)
{
    float2 uv_0 = (q_2 + float2(0.5f) ) / (float4(f_2->proj_0) ).zw;
    return float3((uv_0.x * 2.0f - 1.0f) / (float4(f_2->proj_0) ).x * z_0, (1.0f - uv_0.y * 2.0f) / (float4(f_2->proj_0) ).y * z_0, - z_0);
}

float3 view_pos_1(FrameData_natural_0 device* f_3, float2 q_3, float z_1)
{
    float2 uv_1 = (q_3 + float2(0.5f) ) / (float4(f_3->proj_0) ).zw;
    return float3((uv_1.x * 2.0f - 1.0f) / (float4(f_3->proj_0) ).x * z_1, (1.0f - uv_1.y * 2.0f) / (float4(f_3->proj_0) ).y * z_1, - z_1);
}

float3 view_at_0(FrameData_natural_0 device* f_4, int2 q_4, KernelContext_0 thread* kernelContext_5)
{
    float2 _S6 = float2(q_4);
    float _S7 = linear_z_1(f_4, q_4, kernelContext_5);
    float3 _S8 = view_pos_1(f_4, _S6, _S7);
    return _S8;
}

float fast_acos_0(float x_0)
{
    float a_0 = abs(x_0);
    float r_0 = (-0.15658299624919891f * a_0 + 1.5707963705062866f) * sqrt(1.0f - a_0);
    float _S9;
    if(x_0 >= 0.0f)
    {
        _S9 = r_0;
    }
    else
    {
        _S9 = 3.1415927410125732f - r_0;
    }
    return _S9;
}

void gtao_0(FrameData_natural_0 device* f_5, uint2 t_0, KernelContext_0 thread* kernelContext_6)
{
    packed_float2 device* _S10 = ed_buf_0(kernelContext_6->ed_c_0->raw_0, kernelContext_6);
    int2 _S11 = int2(t_0 * uint2(2U) );
    float _S12 = linear_z_0(f_5, _S11, kernelContext_6);
    uint _S13 = t_0.y;
    uint _S14 = t_0.x;
    uint o_0 = _S13 * kernelContext_6->ed_c_0->hw_0 + _S14;
    if(_S12 >= 1.0e+06f)
    {
        *(_S10 + o_0) = packed_float2(float2(1.0f, 1.0e+06f)) ;
        return;
    }
    float3 _S15 = view_pos_0(f_5, float2(_S11), _S12);
    int2 _S16 = int2(int(2), int(0));
    float3 _S17 = view_at_0(f_5, _S11 + _S16, kernelContext_6);
    float3 _S18 = view_at_0(f_5, _S11 - _S16, kernelContext_6);
    int2 _S19 = int2(int(0), int(2));
    float3 _S20 = view_at_0(f_5, _S11 + _S19, kernelContext_6);
    float3 _S21 = view_at_0(f_5, _S11 - _S19, kernelContext_6);
    float _S22 = _S15.z;
    float3 dx_0;
    if((abs(_S17.z - _S22)) < (abs(_S22 - _S18.z)))
    {
        dx_0 = _S17 - _S15;
    }
    else
    {
        dx_0 = _S15 - _S18;
    }
    float3 dy_0;
    if((abs(_S20.z - _S22)) < (abs(_S22 - _S21.z)))
    {
        dy_0 = _S20 - _S15;
    }
    else
    {
        dy_0 = _S15 - _S21;
    }
    float3 n_0 = normalize(cross(dy_0, dx_0));
    float3 v_0 = normalize(- _S15);
    float3 n_1;
    if((dot(n_0, v_0)) < 0.0f)
    {
        n_1 = - n_0;
    }
    else
    {
        n_1 = n_0;
    }
    float r_px_0 = kernelContext_6->ed_c_0->radius_0 * (float4(f_5->proj_0) ).y * (float4(f_5->proj_0) ).w * 0.5f / _S12;
    if(r_px_0 < 2.0f)
    {
        *(_S10 + o_0) = packed_float2(float2(1.0f, _S12)) ;
        return;
    }
    float _S23 = min(r_px_0, 256.0f);
    array<uint, int(16)> BAYER4_0 = { { 0U, 8U, 2U, 10U, 12U, 4U, 14U, 6U, 3U, 11U, 1U, 9U, 15U, 7U, 13U, 5U } };
    uint _S24 = _S13 & 3U;
    uint _S25 = _S14 & 3U;
    float _S26 = (float(BAYER4_0[_S24 * 4U + _S25]) + 0.5f) / 16.0f;
    float _S27 = fract(52.982917785644531f * fract(0.067110560834407806f * float(_S25) + 0.0058371499180793762f * float(_S24)));
    float falloff_range_0 = 0.60000002384185791f * kernelContext_6->ed_c_0->radius_0;
    float _S28 = -1.0f / falloff_range_0;
    float _S29 = (kernelContext_6->ed_c_0->radius_0 - falloff_range_0) / falloff_range_0 + 1.0f;
    uint s_0 = 0U;
    float vis_0 = 0.0f;
    for(;;)
    {
        if(s_0 < 2U)
        {
        }
        else
        {
            break;
        }
        float phi_0 = (float(s_0) + _S26) / 2.0f * 3.1415927410125732f;
        float _S30 = cos(phi_0);
        float _S31 = sin(phi_0);
        float3 dir_0 = float3(_S30, _S31, 0.0f);
        float2 _S32 = float2(_S30, - _S31);
        float3 ortho_0 = dir_0 - float3(dot(dir_0, v_0))  * v_0;
        float3 axis_0 = normalize(cross(ortho_0, v_0));
        float3 pn_0 = n_1 - axis_0 * float3(dot(n_1, axis_0)) ;
        float pn_len_0 = length(pn_0);
        float sgn_0;
        if((dot(ortho_0, pn_0)) >= 0.0f)
        {
            sgn_0 = 1.0f;
        }
        else
        {
            sgn_0 = -1.0f;
        }
        float cos_n_0 = saturate(dot(pn_0, v_0) / max(pn_len_0, 9.99999997475242708e-07f));
        float ang_n_0 = sgn_0 * fast_acos_0(cos_n_0);
        float low0_0 = cos(ang_n_0 + 1.5707963705062866f);
        float low1_0 = cos(ang_n_0 - 1.5707963705062866f);
        float h0_0 = low0_0;
        float h1_0 = low1_0;
        uint k_0 = 0U;
        for(;;)
        {
            if(k_0 < 4U)
            {
            }
            else
            {
                break;
            }
            float _S33 = float(k_0);
            float st_0 = (_S33 + _S27) / 4.0f;
            int2 _S34 = int2(round(_S32 * float2(max(st_0 * st_0 * _S23, 1.0f + _S33)) ));
            float3 _S35 = view_at_0(f_5, _S11 + _S34, kernelContext_6);
            float3 _S36 = view_at_0(f_5, _S11 - _S34, kernelContext_6);
            float3 d0_0 = _S35 - _S15;
            float3 d1_0 = _S36 - _S15;
            float l0_0 = length(d0_0);
            float l1_0 = length(d1_0);
            float _S37 = max(h0_0, mix(low0_0, dot(d0_0 / float3(max(l0_0, 9.99999997475242708e-07f)) , v_0), saturate(l0_0 * _S28 + _S29)));
            float _S38 = max(h1_0, mix(low1_0, dot(d1_0 / float3(max(l1_0, 9.99999997475242708e-07f)) , v_0), saturate(l1_0 * _S28 + _S29)));
            uint _S39 = k_0 + 1U;
            h0_0 = _S37;
            h1_0 = _S38;
            k_0 = _S39;
        }
        float sin_n_0 = sin(ang_n_0);
        float _S40 = 2.0f * (ang_n_0 + max(- fast_acos_0(h1_0) - ang_n_0, -1.5707963705062866f));
        float _S41 = 2.0f * (ang_n_0 + min(fast_acos_0(h0_0) - ang_n_0, 1.5707963705062866f));
        float vis_1 = vis_0 + pn_len_0 * ((cos_n_0 + _S40 * sin_n_0 - cos(_S40 - ang_n_0)) * 0.25f + (cos_n_0 + _S41 * sin_n_0 - cos(_S41 - ang_n_0)) * 0.25f);
        s_0 = s_0 + 1U;
        vis_0 = vis_1;
    }
    *(_S10 + o_0) = packed_float2(float2(saturate(vis_0 / 2.0f / 0.97000002861022949f), _S12)) ;
    return;
}

void blur_0(uint2 t_1, KernelContext_0 thread* kernelContext_7)
{
    packed_float2 device* _S42 = ed_buf_0(kernelContext_7->ed_c_0->raw_0, kernelContext_7);
    packed_float2 device* _S43 = ed_buf_0(kernelContext_7->ed_c_0->out_0, kernelContext_7);
    uint o_1 = t_1.y * kernelContext_7->ed_c_0->hw_0 + t_1.x;
    float2 _S44 = float2(*(_S42 + o_1)) ;
    float _S45 = _S44.y;
    if(_S45 >= 1.0e+06f)
    {
        *(_S43 + o_1) = packed_float2(_S44) ;
        return;
    }
    int y_0 = int(-1);
    float sum_0 = 0.0f;
    float wsum_0 = 0.0f;
    for(;;)
    {
        if(y_0 <= int(2))
        {
        }
        else
        {
            break;
        }
        int x_1 = int(-1);
        for(;;)
        {
            if(x_1 <= int(2))
            {
            }
            else
            {
                break;
            }
            int2 s_1 = clamp(int2(t_1) + int2(x_1, y_0), int2(int(0)) , int2(int(kernelContext_7->ed_c_0->hw_0) - int(1), int(kernelContext_7->ed_c_0->hh_0) - int(1)));
            float2 _S46 = float2(*(_S42 + (uint(s_1.y) * kernelContext_7->ed_c_0->hw_0 + uint(s_1.x)))) ;
            float w_0 = saturate(1.0f - abs(_S46.y - _S45) / (0.05000000074505806f * _S45));
            float sum_1 = sum_0 + _S46.x * w_0;
            float wsum_1 = wsum_0 + w_0;
            x_1 = x_1 + int(1);
            sum_0 = sum_1;
            wsum_0 = wsum_1;
        }
        y_0 = y_0 + int(1);
    }
    packed_float2 device* _S47 = _S43 + o_1;
    if(wsum_0 > 0.0f)
    {
        sum_0 = sum_0 / wsum_0;
    }
    else
    {
        sum_0 = _S44.x;
    }
    *_S47 = packed_float2(float2(sum_0, _S45)) ;
    return;
}

[[kernel]] void ao_main(uint3 tid_0 [[thread_position_in_grid]], AoC_0 constant* ed_c_1 [[buffer(0)]], ulong device* ed_buffers_1 [[buffer(2)]], ulong device* ed_textures_1 [[buffer(1)]])
{
    thread KernelContext_0 kernelContext_8;
    (&kernelContext_8)->ed_c_0 = ed_c_1;
    (&kernelContext_8)->ed_buffers_0 = ed_buffers_1;
    (&kernelContext_8)->ed_textures_0 = ed_textures_1;
    uint2 t_2 = tid_0.xy;
    bool _S48;
    if((t_2.x) >= (ed_c_1->hw_0))
    {
        _S48 = true;
    }
    else
    {
        _S48 = (t_2.y) >= ((&kernelContext_8)->ed_c_0->hh_0);
    }
    if(_S48)
    {
        return;
    }
    FrameData_natural_0 device* _S49 = ed_buf_1((&kernelContext_8)->ed_c_0->frame_0, &kernelContext_8);
    if(((&kernelContext_8)->ed_c_0->mode_0) == 0U)
    {
        gtao_0(_S49, t_2, &kernelContext_8);
    }
    else
    {
        blur_0(t_2, &kernelContext_8);
    }
    return;
}

