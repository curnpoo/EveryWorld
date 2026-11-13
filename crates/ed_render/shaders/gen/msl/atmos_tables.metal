#include <metal_stdlib>
#include <metal_math>
#include <metal_texture>
using namespace metal;
constant array<float, int(9)> ATM_SH_A_0 = { { 1.0f, 0.66666668653488159f, 0.66666668653488159f, 0.66666668653488159f, 0.25f, 0.25f, 0.25f, 0.25f, 0.25f } };
constant array<float, int(9)> ATM_SH_K_0 = { { 0.28209498524665833f, 0.48860299587249756f, 0.48860299587249756f, 0.48860299587249756f, 1.0925480127334595f, 1.0925480127334595f, 0.31539198756217957f, 1.0925480127334595f, 0.54627400636672974f } };
struct AtmosC_0
{
    uint frame_0;
    uint mode_0;
    array<uint, int(14)> pad_0;
};

struct KernelContext_0
{
    AtmosC_0 constant* ed_c_0;
    ulong device* ed_buffers_0;
};

packed_float4 device* ed_buf_0(uint i_0, KernelContext_0 thread* kernelContext_0)
{
    return (packed_float4 device*)kernelContext_0->ed_buffers_0[i_0];
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
    AtmosData_natural_0 atmos_0;
    WindData_natural_0 wind_1;
};

FrameData_natural_0 device* ed_buf_1(uint i_1, KernelContext_0 thread* kernelContext_1)
{
    return (FrameData_natural_0 device*)kernelContext_1->ed_buffers_0[i_1];
}

float atm_rb_0(FrameData_natural_0 device* f_0)
{
    return (float4((&f_0->atmos_0)->radii_0) ).x;
}

float atm_rb_1(FrameData_natural_0 device* f_1)
{
    return (float4((&f_1->atmos_0)->radii_0) ).x;
}

float atm_rt_0(FrameData_natural_0 device* f_2)
{
    return (float4((&f_2->atmos_0)->radii_0) ).y;
}

float atm_rt_1(FrameData_natural_0 device* f_3)
{
    return (float4((&f_3->atmos_0)->radii_0) ).y;
}

float atm_height_at_0(float rb_0, float h_0, float mu_0, float t_0)
{
    float q_0 = t_0 * t_0 + 2.0f * (rb_0 + h_0) * mu_0 * t_0 + h_0 * (2.0f * rb_0 + h_0);
    return q_0 / (sqrt(max(rb_0 * rb_0 + q_0, 0.0f)) + rb_0);
}

struct AtmMedium_0
{
    float3 ray_0;
    float3 mie_0;
    float3 scat_0;
    float3 ext_0;
};

AtmMedium_0 atm_medium_0(FrameData_natural_0 device* f_4, float h_1)
{
    float _S1 = max(h_1, 0.0f);
    float _S2 = - _S1;
    float md_0 = exp(_S2 / (float4((&f_4->atmos_0)->mie_s_0) ).w);
    float _S3 = max(0.0f, 1.0f - abs(_S1 - (float4((&f_4->atmos_0)->ozone_0) ).w) / (float4((&f_4->atmos_0)->ground_0) ).w);
    thread AtmMedium_0 m_0;
    float3 _S4 = (float4((&f_4->atmos_0)->rayleigh_0) ).xyz * float3(exp(_S2 / (float4((&f_4->atmos_0)->rayleigh_0) ).w)) ;
    (&m_0)->ray_0 = _S4;
    float3 _S5 = float3(md_0) ;
    float3 _S6 = (float4((&f_4->atmos_0)->mie_s_0) ).xyz * _S5;
    (&m_0)->mie_0 = _S6;
    float3 _S7 = _S4 + _S6;
    (&m_0)->scat_0 = _S7;
    (&m_0)->ext_0 = _S7 + (float4((&f_4->atmos_0)->mie_a_0) ).xyz * _S5 + (float4((&f_4->atmos_0)->ozone_0) ).xyz * float3(_S3) ;
    return m_0;
}

[[kernel]] void atmos_transmittance(uint3 tid_0 [[thread_position_in_grid]], AtmosC_0 constant* ed_c_1 [[buffer(0)]], ulong device* ed_buffers_1 [[buffer(2)]])
{
    thread KernelContext_0 kernelContext_2;
    (&kernelContext_2)->ed_c_0 = ed_c_1;
    (&kernelContext_2)->ed_buffers_0 = ed_buffers_1;
    uint _S8 = tid_0.x;
    bool _S9;
    if(_S8 >= 256U)
    {
        _S9 = true;
    }
    else
    {
        _S9 = (tid_0.y) >= 64U;
    }
    if(_S9)
    {
        return;
    }
    FrameData_natural_0 device* _S10 = ed_buf_1((&kernelContext_2)->ed_c_0->frame_0, &kernelContext_2);
    float _S11 = atm_rb_0(_S10);
    float _S12 = atm_rt_0(_S10);
    float ht_0 = _S12 - _S11;
    uint _S13 = tid_0.y;
    float big_h_0 = sqrt(ht_0 * (_S12 + _S11));
    float rho_0 = float(_S13) / 63.0f * big_h_0;
    float _S14 = rho_0 * rho_0;
    float h_2 = _S14 / (sqrt(_S14 + _S11 * _S11) + _S11);
    float r_0 = _S11 + h_2;
    float dmin_0 = ht_0 - h_2;
    float d_0 = dmin_0 + float(_S8) / 255.0f * (rho_0 + big_h_0 - dmin_0);
    float _S15;
    if(d_0 > 9.99999997475242708e-07f)
    {
        _S15 = clamp((dmin_0 * (_S12 + r_0) - d_0 * d_0) / (2.0f * r_0 * d_0), -1.0f, 1.0f);
    }
    else
    {
        _S15 = 1.0f;
    }
    float3 _S16 = float3(0.0f) ;
    float prev_0 = 0.0f;
    uint i_2 = 0U;
    float3 od_0 = _S16;
    for(;;)
    {
        if(i_2 < 40U)
        {
        }
        else
        {
            break;
        }
        uint _S17 = i_2 + 1U;
        float x_0 = float(_S17) / 40.0f;
        float t1_0 = d_0 * x_0 * x_0;
        AtmMedium_0 _S18 = atm_medium_0(_S10, atm_height_at_0(_S11, h_2, _S15, 0.5f * (prev_0 + t1_0)));
        float3 od_1 = od_0 + _S18.ext_0 * float3((t1_0 - prev_0)) ;
        prev_0 = t1_0;
        i_2 = _S17;
        od_0 = od_1;
    }
    packed_float4 device* _S19 = ed_buf_0((uint4((&_S10->atmos_0)->bufs_0) ).x, &kernelContext_2);
    *(_S19 + (_S13 * 256U + _S8)) = packed_float4(float4(exp(- od_0), 1.0f)) ;
    return;
}

bool atm_hits_ground_0(float rb_1, float h_3, float mu_1)
{
    float rm_0 = (rb_1 + h_3) * mu_1;
    bool _S20;
    if(mu_1 < 0.0f)
    {
        _S20 = (rm_0 * rm_0) >= (h_3 * (2.0f * rb_1 + h_3));
    }
    else
    {
        _S20 = false;
    }
    return _S20;
}

float atm_dist_ground_0(float rb_2, float h_4, float mu_2)
{
    float a_0 = - (rb_2 + h_4) * mu_2;
    float c_0 = h_4 * (2.0f * rb_2 + h_4);
    return c_0 / max(a_0 + sqrt(max(a_0 * a_0 - c_0, 0.0f)), 9.999999960041972e-13f);
}

float atm_dist_top_0(float rb_3, float rt_0, float h_5, float mu_3)
{
    float r_1 = rb_3 + h_5;
    float _S21 = r_1 * mu_3;
    return max(- r_1 * mu_3 + sqrt(max(_S21 * _S21 + (rt_0 - rb_3 - h_5) * (rt_0 + r_1), 0.0f)), 0.0f);
}

float2 atm_trans_uv_0(float rb_4, float rt_1, float h_6, float mu_4)
{
    float ht_1 = rt_1 - rb_4;
    float _S22 = clamp(h_6, 0.0f, ht_1);
    float big_h_1 = sqrt(ht_1 * (rt_1 + rb_4));
    float rho_1 = sqrt(max(_S22 * (2.0f * rb_4 + _S22), 0.0f));
    float dmin_1 = ht_1 - _S22;
    return float2((atm_dist_top_0(rb_4, rt_1, _S22, mu_4) - dmin_1) / max(rho_1 + big_h_1 - dmin_1, 9.99999997475242708e-07f), rho_1 / big_h_1);
}

float4 atm_bilinear_0(packed_float4 device* buf_0, uint w_0, uint row0_0, uint rows_0, float2 uv_0)
{
    float2 p_0 = saturate(uv_0) * float2(float(w_0 - 1U), float(rows_0 - 1U));
    uint2 _S23 = min(uint2(p_0), uint2(w_0 - 2U, rows_0 - 2U));
    float2 g_0 = p_0 - float2(_S23);
    uint b_0 = (row0_0 + _S23.y) * w_0 + _S23.x;
    float4 _S24 = float4(g_0.x) ;
    uint _S25 = b_0 + w_0;
    return mix(mix(float4(*(buf_0 + b_0)) , float4(*(buf_0 + (b_0 + 1U))) , _S24), mix(float4(*(buf_0 + _S25)) , float4(*(buf_0 + (_S25 + 1U))) , _S24), float4(g_0.y) );
}

float3 atm_transmittance_0(FrameData_natural_0 device* f_5, float h_7, float mu_5, KernelContext_0 thread* kernelContext_3)
{
    float _S26 = atm_rb_0(f_5);
    float _S27 = atm_rt_0(f_5);
    float2 uv_1 = atm_trans_uv_0(_S26, _S27, h_7, mu_5);
    packed_float4 device* _S28 = ed_buf_0((uint4((&f_5->atmos_0)->bufs_0) ).x, kernelContext_3);
    float4 _S29 = atm_bilinear_0(_S28, 256U, 0U, 64U, uv_1);
    return _S29.xyz;
}

float atm_cos_horizon_0(float rb_5, float h_8)
{
    return - sqrt(max(h_8 * (2.0f * rb_5 + h_8), 0.0f)) / (rb_5 + h_8);
}

float atm_earth_shadow_0(FrameData_natural_0 device* f_6, float h_9, float mu_s_0)
{
    float _S30 = atm_rb_1(f_6);
    float ch_0 = atm_cos_horizon_0(_S30, h_9);
    float s_0 = 0.0046999999321997166f * sqrt(max(1.0f - ch_0 * ch_0, 0.0f)) + 9.9999997473787516e-06f;
    return smoothstep(- s_0, s_0, mu_s_0 - ch_0);
}

float3 atm_sun_transmittance_0(FrameData_natural_0 device* f_7, float h_10, float mu_s_1, KernelContext_0 thread* kernelContext_4)
{
    float3 _S31 = atm_transmittance_0(f_7, h_10, mu_s_1, kernelContext_4);
    float _S32 = atm_earth_shadow_0(f_7, h_10, mu_s_1);
    return _S31 * float3(_S32) ;
}

float3 atm_sun_transmittance_1(FrameData_natural_0 device* f_8, float h_11, float mu_s_2, KernelContext_0 thread* kernelContext_5)
{
    float3 _S33 = atm_transmittance_0(f_8, h_11, mu_s_2, kernelContext_5);
    float _S34 = atm_earth_shadow_0(f_8, h_11, mu_s_2);
    return _S33 * float3(_S34) ;
}

[[kernel]] void atmos_multiscatter(uint3 tid_1 [[thread_position_in_grid]], AtmosC_0 constant* ed_c_2 [[buffer(0)]], ulong device* ed_buffers_2 [[buffer(2)]])
{
    thread KernelContext_0 kernelContext_6;
    (&kernelContext_6)->ed_c_0 = ed_c_2;
    (&kernelContext_6)->ed_buffers_0 = ed_buffers_2;
    uint _S35 = tid_1.x;
    bool _S36;
    if(_S35 >= 32U)
    {
        _S36 = true;
    }
    else
    {
        _S36 = (tid_1.y) >= 32U;
    }
    if(_S36)
    {
        return;
    }
    FrameData_natural_0 device* _S37 = ed_buf_1((&kernelContext_6)->ed_c_0->frame_0, &kernelContext_6);
    float _S38 = atm_rb_0(_S37);
    float _S39 = atm_rt_0(_S37);
    float ht_2 = _S39 - _S38;
    float mu_s_3 = float(_S35) / 31.0f * 2.0f - 1.0f;
    uint _S40 = tid_1.y;
    float _S41 = max(float(_S40) / 31.0f * ht_2, 0.0010000000474974513f);
    float _S42 = sqrt(max(1.0f - mu_s_3 * mu_s_3, 0.0f));
    float3 _S43 = float3(0.0f) ;
    uint i_3 = 0U;
    float3 lsum_0 = _S43;
    float3 fsum_0 = _S43;
    for(;;)
    {
        if(i_3 < 8U)
        {
        }
        else
        {
            break;
        }
        float mu_6 = 1.0f - 2.0f * (float(i_3) + 0.5f) / 8.0f;
        float _S44 = sqrt(max(1.0f - mu_6 * mu_6, 0.0f));
        bool ground_1 = atm_hits_ground_0(_S38, _S41, mu_6);
        float _S45;
        if(ground_1)
        {
            _S45 = atm_dist_ground_0(_S38, _S41, mu_6);
        }
        else
        {
            _S45 = atm_dist_top_0(_S38, _S38 + ht_2, _S41, mu_6);
        }
        uint j_0 = 0U;
        for(;;)
        {
            if(j_0 < 8U)
            {
            }
            else
            {
                break;
            }
            float nu_0 = mu_6 * mu_s_3 + _S44 * _S42 * cos(6.2831854820251465f * (float(j_0) + 0.5f) / 8.0f);
            float3 _S46 = float3(1.0f) ;
            float prev_1 = 0.0f;
            uint k_0 = 0U;
            float3 thr_0 = _S46;
            float3 l_0 = _S43;
            float3 fms_0 = _S43;
            for(;;)
            {
                if(k_0 < 20U)
                {
                }
                else
                {
                    break;
                }
                uint _S47 = k_0 + 1U;
                float x_1 = float(_S47) / 20.0f;
                float t1_1 = _S45 * x_1 * x_1;
                float dt_0 = t1_1 - prev_1;
                float tm_0 = 0.5f * (prev_1 + t1_1);
                float hh_0 = atm_height_at_0(_S38, _S41, mu_6, tm_0);
                float mus_0 = clamp(((_S38 + _S41) * mu_s_3 + tm_0 * nu_0) / (_S38 + hh_0), -1.0f, 1.0f);
                AtmMedium_0 _S48 = atm_medium_0(_S37, hh_0);
                float3 st_0 = exp(- _S48.ext_0 * float3(dt_0) );
                float3 _S49 = max(_S48.ext_0, float3(9.99999971718068537e-10f) );
                float3 _S50 = atm_sun_transmittance_0(_S37, hh_0, mus_0, &kernelContext_6);
                float3 s_1 = _S50 * _S48.scat_0 / float3(12.566370964050293f) ;
                float3 l_1 = l_0 + thr_0 * (s_1 - s_1 * st_0) / _S49;
                float3 fms_1 = fms_0 + thr_0 * (_S48.scat_0 - _S48.scat_0 * st_0) / _S49;
                float3 thr_1 = thr_0 * st_0;
                prev_1 = t1_1;
                k_0 = _S47;
                thr_0 = thr_1;
                l_0 = l_1;
                fms_0 = fms_1;
            }
            float3 l_2;
            if(ground_1)
            {
                float mus_1 = clamp(((_S38 + _S41) * mu_s_3 + _S45 * nu_0) / _S38, -1.0f, 1.0f);
                float3 _S51 = atm_sun_transmittance_0(_S37, 0.0f, mus_1, &kernelContext_6);
                l_2 = l_0 + thr_0 * _S51 * float3((saturate(mus_1) * (float4((&_S37->atmos_0)->ap_0) ).z))  * (float4((&_S37->atmos_0)->ground_0) ).xyz / float3(3.1415927410125732f) ;
            }
            else
            {
                l_2 = l_0;
            }
            float3 lsum_1 = lsum_0 + l_2;
            float3 fsum_1 = fsum_0 + fms_0;
            j_0 = j_0 + 1U;
            lsum_0 = lsum_1;
            fsum_0 = fsum_1;
        }
        i_3 = i_3 + 1U;
    }
    float3 _S52 = float3(64.0f) ;
    float3 psi_0 = lsum_0 / _S52 / max(float3(1.0f)  - fsum_0 / _S52, float3(0.0010000000474974513f) );
    packed_float4 device* _S53 = ed_buf_0((uint4((&_S37->atmos_0)->bufs_0) ).y, &kernelContext_6);
    *(_S53 + (_S40 * 32U + _S35)) = packed_float4(float4(psi_0, 1.0f)) ;
    return;
}

float atm_enter_top_0(float rb_6, float rt_2, float h_12, float mu_7)
{
    float r_2 = rb_6 + h_12;
    float _S54 = r_2 * mu_7;
    float d_1 = _S54 * _S54 - (h_12 - (rt_2 - rb_6)) * (r_2 + rt_2);
    if(d_1 < 0.0f)
    {
        return -1.0f;
    }
    float t_1 = - r_2 * mu_7 - sqrt(d_1);
    float _S55;
    if(t_1 >= 0.0f)
    {
        _S55 = t_1;
    }
    else
    {
        _S55 = -1.0f;
    }
    return _S55;
}

bool atm_enter_0(FrameData_natural_0 device* f_9, float thread* h_13, float thread* mu_8, float thread* mu_s_4, float nu_1, float thread* te_0)
{
    *te_0 = 0.0f;
    float _S56 = atm_rb_1(f_9);
    float _S57 = atm_rt_1(f_9);
    float _S58 = _S57 - _S56;
    if((*h_13) <= _S58)
    {
        return true;
    }
    float _S59 = atm_enter_top_0(_S56, _S57, *h_13, *mu_8);
    *te_0 = _S59;
    if(_S59 < 0.0f)
    {
        return false;
    }
    float r_3 = _S56 + *h_13;
    *mu_8 = clamp((r_3 * *mu_8 + *te_0) / _S57, -1.0f, 1.0f);
    *mu_s_4 = clamp((r_3 * *mu_s_4 + *te_0 * nu_1) / _S57, -1.0f, 1.0f);
    *h_13 = _S58;
    return true;
}

float atm_phase_rayleigh_0(float nu_2)
{
    return 0.059683103114366531f * (1.0f + nu_2 * nu_2);
}

float atm_phase_mie_0(float g_1, float nu_3)
{
    float _S60 = g_1 * g_1;
    return 0.11936620622873306f * (1.0f - _S60) / (2.0f + _S60) * (1.0f + nu_3 * nu_3) / pow(max(1.0f + _S60 - 2.0f * g_1 * nu_3, 9.9999997473787516e-05f), 1.5f);
}

float3 atm_multiscatter_0(FrameData_natural_0 device* f_10, float h_14, float mu_s_5, KernelContext_0 thread* kernelContext_7)
{
    float _S61 = mu_s_5 * 0.5f + 0.5f;
    float _S62 = atm_rt_0(f_10);
    float _S63 = atm_rb_0(f_10);
    float2 uv_2 = float2(_S61, h_14 / (_S62 - _S63));
    packed_float4 device* _S64 = ed_buf_0((uint4((&f_10->atmos_0)->bufs_0) ).y, kernelContext_7);
    float4 _S65 = atm_bilinear_0(_S64, 32U, 0U, 32U, uv_2);
    return _S65.xyz;
}

struct AtmAcc_0
{
    float3 l_3;
    float3 t_2;
};

void atm_segment_0(FrameData_natural_0 device* f_11, AtmAcc_0 thread* a_1, float h0_0, float mu_9, float mu_s_6, float nu_4, float tm_1, float dt_1, float pr_0, float pm_0, KernelContext_0 thread* kernelContext_8)
{
    float _S66 = atm_rb_0(f_11);
    float h_15 = atm_height_at_0(_S66, h0_0, mu_9, tm_1);
    float mus_2 = clamp(((_S66 + h0_0) * mu_s_6 + tm_1 * nu_4) / (_S66 + h_15), -1.0f, 1.0f);
    AtmMedium_0 _S67 = atm_medium_0(f_11, h_15);
    float3 st_1 = exp(- _S67.ext_0 * float3(dt_1) );
    float3 _S68 = (float4((&f_11->atmos_0)->sun_0) ).xyz;
    float3 _S69 = atm_sun_transmittance_1(f_11, h_15, mus_2, kernelContext_8);
    float3 _S70 = _S69 * (_S67.ray_0 * float3(pr_0)  + _S67.mie_0 * float3(pm_0) );
    float3 _S71 = atm_multiscatter_0(f_11, h_15, mus_2, kernelContext_8);
    float3 s_2 = _S68 * (_S70 + _S71 * _S67.scat_0);
    a_1->l_3 = a_1->l_3 + a_1->t_2 * (s_2 - s_2 * st_1) / max(_S67.ext_0, float3(9.99999971718068537e-10f) );
    a_1->t_2 = a_1->t_2 * st_1;
    return;
}

void atm_segment_1(FrameData_natural_0 device* f_12, AtmAcc_0 thread* a_2, float h0_1, float mu_10, float mu_s_7, float nu_5, float tm_2, float dt_2, float pr_1, float pm_1, KernelContext_0 thread* kernelContext_9)
{
    float _S72 = atm_rb_0(f_12);
    float h_16 = atm_height_at_0(_S72, h0_1, mu_10, tm_2);
    float mus_3 = clamp(((_S72 + h0_1) * mu_s_7 + tm_2 * nu_5) / (_S72 + h_16), -1.0f, 1.0f);
    AtmMedium_0 _S73 = atm_medium_0(f_12, h_16);
    float3 st_2 = exp(- _S73.ext_0 * float3(dt_2) );
    float3 _S74 = (float4((&f_12->atmos_0)->sun_0) ).xyz;
    float3 _S75 = atm_sun_transmittance_1(f_12, h_16, mus_3, kernelContext_9);
    float3 _S76 = _S75 * (_S73.ray_0 * float3(pr_1)  + _S73.mie_0 * float3(pm_1) );
    float3 _S77 = atm_multiscatter_0(f_12, h_16, mus_3, kernelContext_9);
    float3 s_3 = _S74 * (_S76 + _S77 * _S73.scat_0);
    a_2->l_3 = a_2->l_3 + a_2->t_2 * (s_3 - s_3 * st_2) / max(_S73.ext_0, float3(9.99999971718068537e-10f) );
    a_2->t_2 = a_2->t_2 * st_2;
    return;
}

float3 atm_ground_radiance_0(FrameData_natural_0 device* f_13, float h0_2, float mu_s_8, float nu_6, float t_3, KernelContext_0 thread* kernelContext_10)
{
    float _S78 = atm_rb_0(f_13);
    float mus_4 = clamp(((_S78 + h0_2) * mu_s_8 + t_3 * nu_6) / _S78, -1.0f, 1.0f);
    float3 _S79 = atm_sun_transmittance_1(f_13, 0.0f, mus_4, kernelContext_10);
    float3 _S80 = _S79 * float3((saturate(mus_4) * (float4((&f_13->atmos_0)->ap_0) ).z)) ;
    float3 _S81 = atm_multiscatter_0(f_13, 0.0f, mus_4, kernelContext_10);
    float3 _S82 = float3(3.1415927410125732f) ;
    return (float4((&f_13->atmos_0)->sun_0) ).xyz * (_S80 + _S81 * _S82) * (float4((&f_13->atmos_0)->ground_0) ).xyz / _S82;
}

float3 atm_sky_ray_0(FrameData_natural_0 device* f_14, float h_17, float mu_11, float mu_s_9, float nu_7, uint steps_0, KernelContext_0 thread* kernelContext_11)
{
    thread float _S83 = h_17;
    thread float _S84 = mu_11;
    thread float _S85 = mu_s_9;
    thread float te_1;
    bool _S86 = atm_enter_0(f_14, &_S83, &_S84, &_S85, nu_7, &te_1);
    if(!_S86)
    {
        return float3(0.0f) ;
    }
    float _S87 = atm_rb_0(f_14);
    bool ground_2 = atm_hits_ground_0(_S87, _S83, _S84);
    float tmax_0;
    if(ground_2)
    {
        tmax_0 = atm_dist_ground_0(_S87, _S83, _S84);
    }
    else
    {
        float _S88 = atm_rt_0(f_14);
        tmax_0 = atm_dist_top_0(_S87, _S88, _S83, _S84);
    }
    float _S89 = atm_phase_rayleigh_0(nu_7);
    float _S90 = atm_phase_mie_0((float4((&f_14->atmos_0)->mie_a_0) ).w, nu_7);
    thread AtmAcc_0 a_3;
    (&a_3)->l_3 = float3(0.0f) ;
    (&a_3)->t_2 = float3(1.0f) ;
    float prev_2 = 0.0f;
    uint i_4 = 0U;
    for(;;)
    {
        if(i_4 < steps_0)
        {
        }
        else
        {
            break;
        }
        uint _S91 = i_4 + 1U;
        float x_2 = float(_S91) / float(steps_0);
        float t1_2 = tmax_0 * x_2 * x_2;
        atm_segment_1(f_14, &a_3, _S83, _S84, _S85, nu_7, 0.5f * (prev_2 + t1_2), t1_2 - prev_2, _S89, _S90, kernelContext_11);
        prev_2 = t1_2;
        i_4 = _S91;
    }
    if(ground_2)
    {
        float3 _S92 = atm_ground_radiance_0(f_14, _S83, _S85, nu_7, tmax_0, kernelContext_11);
        (&a_3)->l_3 = (&a_3)->l_3 + (&a_3)->t_2 * _S92;
    }
    return (&a_3)->l_3;
}

[[kernel]] void atmos_skyview(uint3 tid_2 [[thread_position_in_grid]], AtmosC_0 constant* ed_c_3 [[buffer(0)]], ulong device* ed_buffers_3 [[buffer(2)]])
{
    thread KernelContext_0 kernelContext_12;
    (&kernelContext_12)->ed_c_0 = ed_c_3;
    (&kernelContext_12)->ed_buffers_0 = ed_buffers_3;
    FrameData_natural_0 device* _S93 = ed_buf_1(ed_c_3->frame_0, &kernelContext_12);
    bool surf_0 = ((&kernelContext_12)->ed_c_0->mode_0) != 0U;
    uint w_1;
    if(surf_0)
    {
        w_1 = 96U;
    }
    else
    {
        w_1 = 192U;
    }
    uint rows_1;
    if(surf_0)
    {
        rows_1 = 54U;
    }
    else
    {
        rows_1 = 108U;
    }
    uint _S94 = tid_2.x;
    bool _S95;
    if(_S94 >= w_1)
    {
        _S95 = true;
    }
    else
    {
        _S95 = (tid_2.y) >= rows_1;
    }
    if(_S95)
    {
        return;
    }
    float h_18;
    if(surf_0)
    {
        h_18 = (float4((&_S93->atmos_0)->radii_0) ).w;
    }
    else
    {
        h_18 = (float4((&_S93->atmos_0)->radii_0) ).z;
    }
    uint half_rows_0 = rows_1 / 2U;
    uint _S96 = tid_2.y;
    bool lower_0 = _S96 >= half_rows_0;
    float u_0 = float(_S94) / float(w_1 - 1U);
    if(lower_0)
    {
        rows_1 = half_rows_0;
    }
    else
    {
        rows_1 = 0U;
    }
    float c_1 = float(_S96 - rows_1) / float(half_rows_0 - 1U);
    float _S97 = atm_rb_0(_S93);
    float zha_0 = acos(atm_cos_horizon_0(_S97, h_18));
    float theta_0;
    if(lower_0)
    {
        theta_0 = max(zha_0 + (3.1415927410125732f - zha_0) * c_1 * c_1, zha_0 + 0.00019999999494757503f);
    }
    else
    {
        float _S98 = 1.0f - c_1;
        theta_0 = min(zha_0 * (1.0f - _S98 * _S98), zha_0 - 0.00019999999494757503f);
    }
    float mu_12 = cos(theta_0);
    float mu_s_10 = (float4((&_S93->atmos_0)->up_0) ).w;
    float nu_8 = clamp(mu_12 * mu_s_10 + sin(theta_0) * sqrt(max(1.0f - mu_s_10 * mu_s_10, 0.0f)) * cos(3.1415927410125732f * u_0 * u_0), -1.0f, 1.0f);
    if(surf_0)
    {
        rows_1 = 20U;
    }
    else
    {
        rows_1 = 30U;
    }
    float3 _S99 = atm_sky_ray_0(_S93, h_18, mu_12, mu_s_10, nu_8, rows_1, &kernelContext_12);
    if(surf_0)
    {
        rows_1 = (uint4((&_S93->atmos_0)->bufs_0) ).w;
    }
    else
    {
        rows_1 = (uint4((&_S93->atmos_0)->bufs_0) ).z;
    }
    packed_float4 device* _S100 = ed_buf_0(rows_1, &kernelContext_12);
    *(_S100 + (_S96 * w_1 + _S94)) = packed_float4(float4(_S99, 1.0f)) ;
    return;
}

float3 view_dir_0(FrameData_natural_0 device* f_15, float2 pixel_0)
{
    float2 uv_3 = pixel_0 / (float4(f_15->viewport_0) ).zw;
    float4 p_1 = (((float4(float2(uv_3.x * 2.0f - 1.0f, 1.0f - uv_3.y * 2.0f), 1.0f, 1.0f)) * (matrix<float,int(4),int(4)> (f_15->inv_view_proj_0.data_0[int(0)][int(0)], f_15->inv_view_proj_0.data_0[int(1)][int(0)], f_15->inv_view_proj_0.data_0[int(2)][int(0)], f_15->inv_view_proj_0.data_0[int(3)][int(0)], f_15->inv_view_proj_0.data_0[int(0)][int(1)], f_15->inv_view_proj_0.data_0[int(1)][int(1)], f_15->inv_view_proj_0.data_0[int(2)][int(1)], f_15->inv_view_proj_0.data_0[int(3)][int(1)], f_15->inv_view_proj_0.data_0[int(0)][int(2)], f_15->inv_view_proj_0.data_0[int(1)][int(2)], f_15->inv_view_proj_0.data_0[int(2)][int(2)], f_15->inv_view_proj_0.data_0[int(3)][int(2)], f_15->inv_view_proj_0.data_0[int(0)][int(3)], f_15->inv_view_proj_0.data_0[int(1)][int(3)], f_15->inv_view_proj_0.data_0[int(2)][int(3)], f_15->inv_view_proj_0.data_0[int(3)][int(3)]))));
    return normalize(p_1.xyz / float3(p_1.w) );
}

float atm_ap_depth_0(FrameData_natural_0 device* f_16, uint k_1)
{
    float n_0 = (float4((&f_16->atmos_0)->ap_0) ).x;
    return n_0 * (exp2(float(k_1 + 1U) / 32.0f * log2(1.0f + (float4((&f_16->atmos_0)->ap_0) ).y / n_0)) - 1.0f);
}

[[kernel]] void atmos_ap(uint3 tid_3 [[thread_position_in_grid]], AtmosC_0 constant* ed_c_4 [[buffer(0)]], ulong device* ed_buffers_4 [[buffer(2)]])
{
    thread KernelContext_0 kernelContext_13;
    (&kernelContext_13)->ed_c_0 = ed_c_4;
    (&kernelContext_13)->ed_buffers_0 = ed_buffers_4;
    uint _S101 = tid_3.x;
    bool _S102;
    if(_S101 >= 32U)
    {
        _S102 = true;
    }
    else
    {
        _S102 = (tid_3.y) >= 32U;
    }
    if(_S102)
    {
        return;
    }
    FrameData_natural_0 device* _S103 = ed_buf_1((&kernelContext_13)->ed_c_0->frame_0, &kernelContext_13);
    float3 _S104 = view_dir_0(_S103, (float2(tid_3.xy) + float2(0.5f) ) / float2(32.0f)  * (float4(_S103->viewport_0) ).zw);
    thread float h_19 = (float4((&_S103->atmos_0)->radii_0) ).z;
    thread float mu_13 = clamp(dot(_S104, (float4((&_S103->atmos_0)->up_0) ).xyz), -1.0f, 1.0f);
    thread float mu_s_11 = (float4((&_S103->atmos_0)->up_0) ).w;
    float nu_9 = dot(_S104, (float4(_S103->sun_dir_0) ).xyz);
    thread float te_2;
    bool _S105 = atm_enter_0(_S103, &h_19, &mu_13, &mu_s_11, nu_9, &te_2);
    float _S106 = atm_phase_rayleigh_0(nu_9);
    float _S107 = atm_phase_mie_0((float4((&_S103->atmos_0)->mie_a_0) ).w, nu_9);
    thread AtmAcc_0 a_4;
    (&a_4)->l_3 = float3(0.0f) ;
    (&a_4)->t_2 = float3(1.0f) ;
    packed_float4 device* _S108 = ed_buf_0((uint4((&_S103->atmos_0)->bufs2_0) ).x, &kernelContext_13);
    packed_float4 device* _S109 = ed_buf_0((uint4((&_S103->atmos_0)->bufs2_0) ).y, &kernelContext_13);
    float prev_3 = 0.0f;
    uint k_2 = 0U;
    for(;;)
    {
        if(k_2 < 32U)
        {
        }
        else
        {
            break;
        }
        if(_S105)
        {
            float _S110 = atm_ap_depth_0(_S103, k_2);
            uint s_4 = 0U;
            for(;;)
            {
                if(s_4 < 2U)
                {
                }
                else
                {
                    break;
                }
                float ta_0 = mix(prev_3, _S110, float(s_4) / 2.0f);
                uint _S111 = s_4 + 1U;
                float tb1_0 = mix(prev_3, _S110, float(_S111) / 2.0f);
                atm_segment_0(_S103, &a_4, h_19, mu_13, mu_s_11, nu_9, 0.5f * (ta_0 + tb1_0), tb1_0 - ta_0, _S106, _S107, &kernelContext_13);
                s_4 = _S111;
            }
            prev_3 = _S110;
        }
        uint b_1 = (k_2 * 32U + tid_3.y) * 32U + _S101;
        *(_S108 + b_1) = packed_float4(float4((&a_4)->l_3, 1.0f)) ;
        *(_S109 + b_1) = packed_float4(float4((&a_4)->t_2, 1.0f)) ;
        k_2 = k_2 + 1U;
    }
    return;
}

float3 atm_sky_view_0(FrameData_natural_0 device* f_17, uint buf_1, uint w_2, uint rows_2, float h_20, float3 d_2, KernelContext_0 thread* kernelContext_14)
{
    float mu_14 = clamp(dot(d_2, (float4((&f_17->atmos_0)->up_0) ).xyz), -1.0f, 1.0f);
    float mu_s_12 = (float4((&f_17->atmos_0)->up_0) ).w;
    float nu_10 = dot(d_2, (float4(f_17->sun_dir_0) ).xyz);
    float _S112 = sqrt(max(1.0f - mu_14 * mu_14, 0.0f)) * sqrt(max(1.0f - mu_s_12 * mu_s_12, 0.0f));
    float cphi_0;
    if(_S112 > 9.99999997475242708e-07f)
    {
        cphi_0 = clamp((nu_10 - mu_14 * mu_s_12) / _S112, -1.0f, 1.0f);
    }
    else
    {
        cphi_0 = 1.0f;
    }
    float u_1 = sqrt(acos(cphi_0) / 3.1415927410125732f);
    float _S113 = atm_rb_0(f_17);
    float zha_1 = acos(atm_cos_horizon_0(_S113, h_20));
    float theta_1 = acos(mu_14);
    uint half_rows_1 = rows_2 / 2U;
    float c_2;
    uint row0_1;
    if(theta_1 < zha_1)
    {
        float _S114 = 1.0f - sqrt(saturate(1.0f - theta_1 / zha_1));
        row0_1 = 0U;
        c_2 = _S114;
    }
    else
    {
        float _S115 = sqrt(saturate((theta_1 - zha_1) / max(3.1415927410125732f - zha_1, 9.99999997475242708e-07f)));
        row0_1 = half_rows_1;
        c_2 = _S115;
    }
    packed_float4 device* _S116 = ed_buf_0(buf_1, kernelContext_14);
    float4 _S117 = atm_bilinear_0(_S116, w_2, row0_1, half_rows_1, float2(u_1, c_2));
    return _S117.xyz;
}

[[kernel]] void atmos_sh_partial(uint3 tid_4 [[thread_position_in_grid]], AtmosC_0 constant* ed_c_5 [[buffer(0)]], ulong device* ed_buffers_5 [[buffer(2)]])
{
    thread KernelContext_0 kernelContext_15;
    (&kernelContext_15)->ed_c_0 = ed_c_5;
    (&kernelContext_15)->ed_buffers_0 = ed_buffers_5;
    uint _S118 = tid_4.x;
    if(_S118 >= 32U)
    {
        return;
    }
    FrameData_natural_0 device* _S119 = ed_buf_1((&kernelContext_15)->ed_c_0->frame_0, &kernelContext_15);
    float ct_0 = 1.0f - 2.0f * (float(_S118) + 0.5f) / 32.0f;
    float _S120 = sqrt(max(1.0f - ct_0 * ct_0, 0.0f));
    thread array<float3, int(9)> acc_0;
    uint k_3 = 0U;
    for(;;)
    {
        if(k_3 < 9U)
        {
        }
        else
        {
            break;
        }
        acc_0[k_3] = float3(0.0f) ;
        k_3 = k_3 + 1U;
    }
    uint j_1 = 0U;
    for(;;)
    {
        if(j_1 < 64U)
        {
        }
        else
        {
            break;
        }
        float ph_0 = 6.2831854820251465f * (float(j_1) + 0.5f) / 64.0f;
        float _S121 = _S120 * cos(ph_0);
        float _S122 = _S120 * sin(ph_0);
        float3 _S123 = atm_sky_view_0(_S119, (uint4((&_S119->atmos_0)->bufs_0) ).w, 96U, 54U, (float4((&_S119->atmos_0)->radii_0) ).w, float3(_S121, _S122, ct_0), &kernelContext_15);
        array<float, int(9)> _S124 = { { 1.0f, _S122, ct_0, _S121, _S121 * _S122, _S122 * ct_0, 3.0f * ct_0 * ct_0 - 1.0f, _S121 * ct_0, _S121 * _S121 - _S122 * _S122 } };
        k_3 = 0U;
        for(;;)
        {
            if(k_3 < 9U)
            {
            }
            else
            {
                break;
            }
            acc_0[k_3] = acc_0[k_3] + _S123 * float3((_S124[k_3] * ATM_SH_K_0[k_3])) ;
            k_3 = k_3 + 1U;
        }
        j_1 = j_1 + 1U;
    }
    packed_float4 device* _S125 = ed_buf_0((uint4((&_S119->atmos_0)->bufs2_0) ).z, &kernelContext_15);
    k_3 = 0U;
    for(;;)
    {
        if(k_3 < 9U)
        {
        }
        else
        {
            break;
        }
        *(_S125 + (_S118 * 9U + k_3)) = packed_float4(float4(acc_0[k_3], 0.0f)) ;
        k_3 = k_3 + 1U;
    }
    return;
}

[[kernel]] void atmos_sh_final(uint3 tid_5 [[thread_position_in_grid]], AtmosC_0 constant* ed_c_6 [[buffer(0)]], ulong device* ed_buffers_6 [[buffer(2)]])
{
    thread KernelContext_0 kernelContext_16;
    (&kernelContext_16)->ed_c_0 = ed_c_6;
    (&kernelContext_16)->ed_buffers_0 = ed_buffers_6;
    uint _S126 = tid_5.x;
    if(_S126 >= 9U)
    {
        return;
    }
    FrameData_natural_0 device* _S127 = ed_buf_1((&kernelContext_16)->ed_c_0->frame_0, &kernelContext_16);
    packed_float4 device* _S128 = ed_buf_0((uint4((&_S127->atmos_0)->bufs2_0) ).z, &kernelContext_16);
    float3 _S129 = float3(0.0f) ;
    uint i_5 = 0U;
    float3 s_5 = _S129;
    for(;;)
    {
        if(i_5 < 32U)
        {
        }
        else
        {
            break;
        }
        float3 s_6 = s_5 + (float4(*(_S128 + (i_5 * 9U + _S126))) ).xyz;
        i_5 = i_5 + 1U;
        s_5 = s_6;
    }
    *(_S128 + (288U + _S126)) = packed_float4(float4(s_5 * float3((0.0061359233222901821f * ATM_SH_A_0[_S126] * ATM_SH_K_0[_S126])) , 0.0f)) ;
    return;
}

