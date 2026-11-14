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
    float sv_radius_0;
    uint sv_on_0;
    uint hzb_0;
    uint hzb_on_0;
    array<uint, int(4)> pad_0;
};

struct KernelContext_0
{
    AoC_0 constant* ed_c_0;
    ulong device* ed_buffers_0;
    ulong device* ed_textures_0;
};

float device* ed_buf_0(uint i_0, KernelContext_0 thread* kernelContext_0)
{
    return (float device*)kernelContext_0->ed_buffers_0[i_0];
}

packed_float4 device* ed_buf_1(uint i_1, KernelContext_0 thread* kernelContext_1)
{
    return (packed_float4 device*)kernelContext_1->ed_buffers_0[i_1];
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
    packed_float4 skyvis_0;
};

struct AtmosData_natural_0
{
    packed_float4 radii_0;
    packed_float4 rayleigh_0;
    packed_float4 mie_s_0;
    packed_float4 mie_a_0;
    packed_float4 ozone_0;
    packed_float4 ground_0;
    packed_float4 sun_0;
    packed_float4 up_0;
    packed_float4 ap_0;
    packed_uint4 bufs_0;
    packed_uint4 bufs2_0;
    packed_uint4 cfg_1;
};

struct WindData_natural_0
{
    packed_float4 wind_0;
    packed_float4 gust_0;
    packed_float4 cfg_2;
};

struct ImpData_natural_0
{
    packed_uint4 bufs_1;
    packed_float4 band_0;
    packed_float4 far_0;
    packed_float4 cfg_3;
};

struct _Array_natural_vectorx3Cfloatx2C4x3E64_0
{
    array<packed_float4, int(64)> data_5;
};

struct SeasonData_natural_0
{
    packed_float4 cfg_4;
    _Array_natural_vectorx3Cfloatx2C4x3E64_0 rows_0;
};

struct GrassData_natural_0
{
    packed_uint4 bufs_2;
    packed_float4 lod_0;
    packed_float4 look_0;
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
    array<HzbLevel_0, int(16)> hzb_1;
    LightData_natural_0 light_0;
    AtmosData_natural_0 atmos_0;
    WindData_natural_0 wind_1;
    ImpData_natural_0 imp_0;
    SeasonData_natural_0 season_0;
    GrassData_natural_0 grass_0;
};

FrameData_natural_0 device* ed_buf_2(uint i_2, KernelContext_0 thread* kernelContext_2)
{
    return (FrameData_natural_0 device*)kernelContext_2->ed_buffers_0[i_2];
}

float ed_depth_read_0(uint i_3, uint2 p_0, KernelContext_0 thread* kernelContext_3)
{
    float _S1 = ((reinterpret_cast<device depth2d<float, access::sample>*>((kernelContext_3->ed_textures_0))[(i_3)].read((p_0))));
    return _S1;
}

float linear_z_0(FrameData_natural_0 device* f_0, int2 q_0, KernelContext_0 thread* kernelContext_4)
{
    float _S2 = ed_depth_read_0(kernelContext_4->ed_c_0->depth_0, uint2(clamp(q_0, int2(int(0)) , int2((float4(f_0->proj_0) ).zw) - int2(int(1)) )), kernelContext_4);
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

float linear_z_1(FrameData_natural_0 device* f_1, int2 q_1, KernelContext_0 thread* kernelContext_5)
{
    float _S4 = ed_depth_read_0(kernelContext_5->ed_c_0->depth_0, uint2(clamp(q_1, int2(int(0)) , int2((float4(f_1->proj_0) ).zw) - int2(int(1)) )), kernelContext_5);
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

float3 view_at_0(FrameData_natural_0 device* f_4, int2 q_4, KernelContext_0 thread* kernelContext_6)
{
    float2 _S6 = float2(q_4);
    float _S7 = linear_z_1(f_4, q_4, kernelContext_6);
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

float3 view_at_far_0(FrameData_natural_0 device* f_5, int2 q_5, float off_0, float min_px_0, KernelContext_0 thread* kernelContext_7)
{
    int levels_0 = int((uint4(f_5->cull_0) ).x);
    bool _S10;
    if((kernelContext_7->ed_c_0->hzb_on_0) == 0U)
    {
        _S10 = true;
    }
    else
    {
        _S10 = levels_0 == int(0);
    }
    if(_S10)
    {
        float3 _S11 = view_at_0(f_5, q_5, kernelContext_7);
        return _S11;
    }
    int l_0 = clamp(max(int(log2(max(off_0, 1.0f))) - int(4), int(ceil(log2(max(min_px_0, 2.0f)))) - int(1)), int(0), levels_0 - int(1));
    HzbLevel_0 lv_0 = f_5->hzb_1[l_0];
    float2 _S12 = float2(float(2U << uint(l_0))) ;
    uint2 _S13 = min(uint2(float2(clamp(q_5, int2(int(0)) , int2((float4(f_5->proj_0) ).zw) - int2(int(1)) )) / _S12), uint2(f_5->hzb_1[l_0].width_0 - 1U, f_5->hzb_1[l_0].height_0 - 1U));
    float device* _S14 = ed_buf_0(kernelContext_7->ed_c_0->hzb_0, kernelContext_7);
    float device* _S15 = _S14 + (lv_0.offset_0 + _S13.y * lv_0.width_0 + _S13.x);
    float d_0 = *_S15;
    float z_2;
    if((*_S15) > 0.0f)
    {
        z_2 = min((float4(f_5->params_0) ).z / d_0, 1.0e+06f);
    }
    else
    {
        z_2 = 1.0e+06f;
    }
    float2 _S16 = float2(0.5f) ;
    float3 _S17 = view_pos_0(f_5, (float2(_S13) + _S16) * _S12 - _S16, z_2);
    return _S17;
}

float2 slice_horizons_0(FrameData_natural_0 device* f_6, int2 q_6, float3 p_1, float2 omega_0, float2 low_0, float r0_px_0, float r_px_0, float radius_1, float thin_0, uint steps_0, float jitter_0, float pw_0, bool far_1, float min_px_1, KernelContext_0 thread* kernelContext_8)
{
    float range_0 = 0.60000002384185791f * radius_1;
    float _S18 = -1.0f / range_0;
    float _S19 = (radius_1 - range_0) / range_0 + 1.0f;
    float3 _S20 = normalize(- p_1);
    thread float2 h_0 = low_0;
    uint k_0 = 0U;
    for(;;)
    {
        if(k_0 < steps_0)
        {
        }
        else
        {
            break;
        }
        float _S21 = float(k_0);
        float st_0 = (_S21 + jitter_0) / float(steps_0);
        float _S22;
        if(pw_0 > 1.5f)
        {
            _S22 = st_0 * st_0;
        }
        else
        {
            _S22 = st_0;
        }
        float _S23 = max(r0_px_0 + _S22 * (r_px_0 - r0_px_0), 1.0f + _S21);
        float2 d_1 = omega_0 * float2(_S23) ;
        float3 _S24;
        if(far_1)
        {
            float3 _S25 = view_at_far_0(f_6, q_6 + int2(round(d_1)), _S23, min_px_1, kernelContext_8);
            _S24 = _S25;
        }
        else
        {
            float3 _S26 = view_at_0(f_6, q_6 + int2(round(d_1)), kernelContext_8);
            _S24 = _S26;
        }
        float3 d0_0 = _S24 - p_1;
        float3 _S27;
        if(far_1)
        {
            float3 _S28 = view_at_far_0(f_6, q_6 - int2(round(d_1)), _S23, min_px_1, kernelContext_8);
            _S27 = _S28;
        }
        else
        {
            float3 _S29 = view_at_0(f_6, q_6 - int2(round(d_1)), kernelContext_8);
            _S27 = _S29;
        }
        float3 d1_0 = _S27 - p_1;
        float l0_0 = length(d0_0);
        float l1_0 = length(d1_0);
        float c1_0 = dot(d1_0 / float3(max(l1_0, 9.99999997475242708e-07f)) , _S20);
        float w1_0 = saturate((l1_0 + thin_0 * max(dot(d1_0, _S20), 0.0f)) * _S18 + _S19);
        h_0.x = max(h_0.x, mix(low_0.x, dot(d0_0 / float3(max(l0_0, 9.99999997475242708e-07f)) , _S20), saturate((l0_0 + thin_0 * max(dot(d0_0, _S20), 0.0f)) * _S18 + _S19)));
        h_0.y = max(h_0.y, mix(low_0.y, c1_0, w1_0));
        k_0 = k_0 + 1U;
    }
    return h_0;
}

float slice_arc_0(float2 h_1, float ang_n_0, float cos_n_0)
{
    float sin_n_0 = sin(ang_n_0);
    float _S30 = 2.0f * (ang_n_0 + max(- fast_acos_0(h_1.y) - ang_n_0, -1.5707963705062866f));
    float _S31 = 2.0f * (ang_n_0 + min(fast_acos_0(h_1.x) - ang_n_0, 1.5707963705062866f));
    return (cos_n_0 + _S30 * sin_n_0 - cos(_S30 - ang_n_0)) * 0.25f + (cos_n_0 + _S31 * sin_n_0 - cos(_S31 - ang_n_0)) * 0.25f;
}

void gtao_0(FrameData_natural_0 device* f_7, uint2 t_0, KernelContext_0 thread* kernelContext_9)
{
    packed_float4 device* _S32 = ed_buf_1(kernelContext_9->ed_c_0->raw_0, kernelContext_9);
    int2 _S33 = int2(t_0 * uint2(2U) );
    float _S34 = linear_z_0(f_7, _S33, kernelContext_9);
    uint _S35 = t_0.y;
    uint _S36 = t_0.x;
    uint o_0 = _S35 * kernelContext_9->ed_c_0->hw_0 + _S36;
    if(_S34 >= 1.0e+06f)
    {
        *(_S32 + o_0) = packed_float4(float4(1.0f, 1.0f, 1.0e+06f, 0.0f)) ;
        return;
    }
    float3 _S37 = view_pos_0(f_7, float2(_S33), _S34);
    int2 _S38 = int2(int(2), int(0));
    float3 _S39 = view_at_0(f_7, _S33 + _S38, kernelContext_9);
    float3 _S40 = view_at_0(f_7, _S33 - _S38, kernelContext_9);
    int2 _S41 = int2(int(0), int(2));
    float3 _S42 = view_at_0(f_7, _S33 + _S41, kernelContext_9);
    float3 _S43 = view_at_0(f_7, _S33 - _S41, kernelContext_9);
    float _S44 = _S37.z;
    float3 dx_0;
    if((abs(_S39.z - _S44)) < (abs(_S44 - _S40.z)))
    {
        dx_0 = _S39 - _S37;
    }
    else
    {
        dx_0 = _S37 - _S40;
    }
    float3 dy_0;
    if((abs(_S42.z - _S44)) < (abs(_S44 - _S43.z)))
    {
        dy_0 = _S42 - _S37;
    }
    else
    {
        dy_0 = _S37 - _S43;
    }
    float3 n_0 = normalize(cross(dy_0, dx_0));
    float3 v_0 = normalize(- _S37);
    float3 n_1;
    if((dot(n_0, v_0)) < 0.0f)
    {
        n_1 = - n_0;
    }
    else
    {
        n_1 = n_0;
    }
    float px_per_m_0 = (float4(f_7->proj_0) ).y * (float4(f_7->proj_0) ).w * 0.5f / _S34;
    float _S45 = min(kernelContext_9->ed_c_0->radius_0 * px_per_m_0, 0.25f * (float4(f_7->proj_0) ).w);
    float _S46 = min(kernelContext_9->ed_c_0->sv_radius_0 * px_per_m_0, 0.75f * (float4(f_7->proj_0) ).w);
    bool small_0 = _S45 >= 2.0f;
    bool sv_0;
    if((kernelContext_9->ed_c_0->sv_on_0) != 0U)
    {
        sv_0 = _S46 >= 2.0f;
    }
    else
    {
        sv_0 = false;
    }
    bool _S47;
    if(!small_0)
    {
        _S47 = !sv_0;
    }
    else
    {
        _S47 = false;
    }
    if(_S47)
    {
        *(_S32 + o_0) = packed_float4(float4(1.0f, 1.0f, _S34, 0.0f)) ;
        return;
    }
    array<uint, int(16)> BAYER4_0 = { { 0U, 8U, 2U, 10U, 12U, 4U, 14U, 6U, 3U, 11U, 1U, 9U, 15U, 7U, 13U, 5U } };
    uint _S48 = _S35 & 3U;
    uint _S49 = _S36 & 3U;
    float _S50 = (float(BAYER4_0[_S48 * 4U + _S49]) + 0.5f) / 16.0f;
    float _S51 = fract(52.982917785644531f * fract(0.067110560834407806f * float(_S49) + 0.0058371499180793762f * float(_S48)));
    float vsky_0 = 1.0f;
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
        float phi_0 = (float(s_0) + _S50) / 2.0f * 3.1415927410125732f;
        float _S52 = cos(phi_0);
        float _S53 = sin(phi_0);
        float3 dir_0 = float3(_S52, _S53, 0.0f);
        float2 omega_1 = float2(_S52, - _S53);
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
        float cos_n_1 = saturate(dot(pn_0, v_0) / max(pn_len_0, 9.99999997475242708e-07f));
        float ang_n_1 = sgn_0 * fast_acos_0(cos_n_1);
        float2 low_1 = float2(cos(ang_n_1 + 1.5707963705062866f), cos(ang_n_1 - 1.5707963705062866f));
        float2 h_2;
        if(small_0)
        {
            float2 _S54 = slice_horizons_0(f_7, _S33, _S37, omega_1, low_1, 0.0f, _S45, kernelContext_9->ed_c_0->radius_0, 0.0f, 4U, _S51, 2.0f, false, 0.0f, kernelContext_9);
            h_2 = _S54;
        }
        else
        {
            h_2 = low_1;
        }
        float arc_0 = slice_arc_0(h_2, ang_n_1, cos_n_1);
        float vis_1 = vis_0 + pn_len_0 * arc_0;
        if(sv_0)
        {
            _S47 = s_0 == ((_S36 + _S35) & 1U);
        }
        else
        {
            _S47 = false;
        }
        if(_S47)
        {
            float2 _S55 = slice_horizons_0(f_7, _S33, _S37, omega_1, low_1, min(max(_S45, 0.0f), _S46 * 0.5f), _S46, kernelContext_9->ed_c_0->sv_radius_0, 3.0f, 4U, fract(_S51 + 0.5f), 1.0f, true, 2.0f * px_per_m_0, kernelContext_9);
            vsky_0 = slice_arc_0(max(h_2, _S55), ang_n_1, cos_n_1) / max(arc_0, 9.9999997473787516e-05f);
        }
        s_0 = s_0 + 1U;
        vis_0 = vis_1;
    }
    *(_S32 + o_0) = packed_float4(float4(saturate(vis_0 / 2.0f / 0.97000002861022949f), saturate(vsky_0 / 0.98000001907348633f), _S34, 0.0f)) ;
    return;
}

float4 raw_at_0(int2 s_1, KernelContext_0 thread* kernelContext_10)
{
    int2 _S56 = clamp(s_1, int2(int(0)) , int2(int(kernelContext_10->ed_c_0->hw_0) - int(1), int(kernelContext_10->ed_c_0->hh_0) - int(1)));
    packed_float4 device* _S57 = ed_buf_1(kernelContext_10->ed_c_0->raw_0, kernelContext_10);
    return float4(*(_S57 + (uint(_S56.y) * kernelContext_10->ed_c_0->hw_0 + uint(_S56.x)))) ;
}

float depth_slope_0(float zm_0, float z_3, float zp_0)
{
    float a_1 = z_3 - zm_0;
    float b_0 = zp_0 - z_3;
    float _S58;
    if((abs(a_1)) < (abs(b_0)))
    {
        _S58 = a_1;
    }
    else
    {
        _S58 = b_0;
    }
    return _S58;
}

void blur_0(uint2 t_1, KernelContext_0 thread* kernelContext_11)
{
    packed_float4 device* _S59 = ed_buf_1(kernelContext_11->ed_c_0->out_0, kernelContext_11);
    uint o_1 = t_1.y * kernelContext_11->ed_c_0->hw_0 + t_1.x;
    int2 _S60 = int2(t_1);
    float4 _S61 = raw_at_0(_S60, kernelContext_11);
    float _S62 = _S61.z;
    if(_S62 >= 1.0e+06f)
    {
        *(_S59 + o_1) = packed_float4(_S61) ;
        return;
    }
    int2 _S63 = int2(int(1), int(0));
    float4 _S64 = raw_at_0(_S60 - _S63, kernelContext_11);
    float _S65 = _S64.z;
    float4 _S66 = raw_at_0(_S60 + _S63, kernelContext_11);
    float _S67 = depth_slope_0(_S65, _S62, _S66.z);
    int2 _S68 = int2(int(0), int(1));
    float4 _S69 = raw_at_0(_S60 - _S68, kernelContext_11);
    float _S70 = _S69.z;
    float4 _S71 = raw_at_0(_S60 + _S68, kernelContext_11);
    float _S72 = depth_slope_0(_S70, _S62, _S71.z);
    float _S73 = 1.0f / (0.05000000074505806f * _S62);
    float2 _S74 = float2(0.0f) ;
    int y_0 = int(-2);
    float2 sum_0 = _S74;
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
        float _S75;
        if((abs(y_0)) == int(2))
        {
            _S75 = 0.5f;
        }
        else
        {
            _S75 = 1.0f;
        }
        int x_1 = int(-2);
        for(;;)
        {
            if(x_1 <= int(2))
            {
            }
            else
            {
                break;
            }
            float wx_0;
            if((abs(x_1)) == int(2))
            {
                wx_0 = 0.5f;
            }
            else
            {
                wx_0 = 1.0f;
            }
            float4 _S76 = raw_at_0(_S60 + int2(x_1, y_0), kernelContext_11);
            float w_0 = wx_0 * _S75 * saturate(1.0f - abs(_S76.z - (_S62 + _S67 * float(x_1) + _S72 * float(y_0))) * _S73);
            float2 sum_1 = sum_0 + _S76.xy * float2(w_0) ;
            float wsum_1 = wsum_0 + w_0;
            x_1 = x_1 + int(1);
            sum_0 = sum_1;
            wsum_0 = wsum_1;
        }
        y_0 = y_0 + int(1);
    }
    packed_float4 device* _S77 = _S59 + o_1;
    if(wsum_0 > 0.0f)
    {
        sum_0 = sum_0 / float2(wsum_0) ;
    }
    else
    {
        sum_0 = _S61.xy;
    }
    *_S77 = packed_float4(float4(sum_0, _S62, 0.0f)) ;
    return;
}

[[kernel]] void ao_main(uint3 tid_0 [[thread_position_in_grid]], AoC_0 constant* ed_c_1 [[buffer(0)]], ulong device* ed_buffers_1 [[buffer(2)]], ulong device* ed_textures_1 [[buffer(1)]])
{
    thread KernelContext_0 kernelContext_12;
    (&kernelContext_12)->ed_c_0 = ed_c_1;
    (&kernelContext_12)->ed_buffers_0 = ed_buffers_1;
    (&kernelContext_12)->ed_textures_0 = ed_textures_1;
    uint2 t_2 = tid_0.xy;
    bool _S78;
    if((t_2.x) >= (ed_c_1->hw_0))
    {
        _S78 = true;
    }
    else
    {
        _S78 = (t_2.y) >= ((&kernelContext_12)->ed_c_0->hh_0);
    }
    if(_S78)
    {
        return;
    }
    FrameData_natural_0 device* _S79 = ed_buf_2((&kernelContext_12)->ed_c_0->frame_0, &kernelContext_12);
    if(((&kernelContext_12)->ed_c_0->mode_0) == 0U)
    {
        gtao_0(_S79, t_2, &kernelContext_12);
    }
    else
    {
        blur_0(t_2, &kernelContext_12);
    }
    return;
}

