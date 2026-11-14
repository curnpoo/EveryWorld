#include <metal_stdlib>
#include <metal_math>
#include <metal_texture>
using namespace metal;
struct TerrainDraw_natural_0
{
    packed_float4 origin_0;
    packed_float4 dir_c_0;
    packed_float4 cc_0;
    packed_float4 fu_0;
    packed_float4 fv_0;
    packed_float4 shape_0;
    packed_float4 morph_0;
    packed_float4 polar_0;
    packed_uint4 slots_0;
    packed_float4 hq_0;
    packed_float4 tex_0;
    packed_uint4 gnd_0;
    packed_uint4 clim_0;
};

struct TerrainC_0
{
    uint frame_0;
    uint draws_0;
    uint pool_0;
    uint cascade_0;
    uint base_0;
    array<uint, int(11)> pad_0;
};

struct KernelContext_0
{
    ulong device* ed_buffers_0;
    TerrainC_0 constant* ed_c_0;
};

uint device* ed_buf_0(uint i_0, KernelContext_0 thread* kernelContext_0)
{
    return (uint device*)kernelContext_0->ed_buffers_0[i_0];
}

float t_page_0(uint pool_1, uint slot_0, uint i_1, uint j_0, float scale_0, float offset_0, KernelContext_0 thread* kernelContext_1)
{
    uint s_0 = j_0 * 257U + i_1;
    uint device* _S1 = ed_buf_0(pool_1, kernelContext_1);
    uint w_0 = *(_S1 + (slot_0 * 33025U + (s_0 >> 1U)));
    uint q_0;
    if((s_0 & 1U) != 0U)
    {
        q_0 = w_0 >> 16U;
    }
    else
    {
        q_0 = w_0 & 65535U;
    }
    return offset_0 + scale_0 * float(q_0);
}

float t_own_h_0(uint pool_2, const TerrainDraw_natural_0 thread* d_0, uint i_2, uint j_1, KernelContext_0 thread* kernelContext_2)
{
    float4 _S2 = float4(d_0->hq_0) ;
    float _S3 = t_page_0(pool_2, (uint4(d_0->slots_0) ).x, i_2 * 4U, j_1 * 4U, _S2.x, _S2.y, kernelContext_2);
    return _S3;
}

float3 t_lattice_0(const TerrainDraw_natural_0 thread* d_1, uint i_3, uint j_2, float h_0, float3 thread* dir_0)
{
    float4 _S4 = float4(d_1->shape_0) ;
    float _S5 = _S4.x;
    float tu_0 = tan((float(i_3) - 32.0f) * _S5);
    float tv_0 = tan((float(j_2) - 32.0f) * _S5);
    float4 _S6 = float4(d_1->fu_0) ;
    float _S7 = _S6.w;
    float4 _S8 = float4(d_1->fv_0) ;
    float _S9 = _S8.w;
    float3 delta_0 = float3((tu_0 * (1.0f + _S7 * _S7) / (1.0f - _S7 * tu_0)))  * _S6.xyz + float3((tv_0 * (1.0f + _S9 * _S9) / (1.0f - _S9 * tv_0)))  * _S8.xyz;
    float4 _S10 = float4(d_1->cc_0) ;
    float lc_0 = _S10.w;
    float3 _S11 = _S10.xyz;
    float k_0 = 2.0f * dot(_S11, delta_0) + dot(delta_0, delta_0);
    float l_0 = sqrt(lc_0 * lc_0 + k_0);
    float3 dd_0 = delta_0 / float3(l_0)  - _S11 * float3((k_0 / ((lc_0 + l_0) * l_0 * lc_0))) ;
    float4 _S12 = float4(d_1->dir_c_0) ;
    float3 _S13 = _S12.xyz;
    float3 _S14 = _S13 + dd_0;
    *dir_0 = _S14;
    float3 _S15 = (float4(d_1->polar_0) ).xyz;
    float dzc_0 = dot(_S13, _S15);
    float ddz_0 = dot(dd_0, _S15);
    float dz_0 = dzc_0 + ddz_0;
    float e_0 = _S4.z;
    float s_1 = sqrt(1.0f - e_0 * dz_0 * dz_0);
    float sc_0 = _S4.w;
    float dr_0 = _S4.y * e_0 * ddz_0 * (dz_0 + dzc_0) / (s_1 * sc_0 * (s_1 + sc_0));
    return dd_0 * float3((_S12.w + dr_0))  + _S13 * float3(dr_0)  + _S14 * float3(h_0) ;
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
    uint offset_1;
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
    _Array_natural_vectorx3Cfloatx2C4x3E4_0 cascade_1;
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
    packed_float4 far_1;
    packed_float4 under_0;
};

struct ShellData_natural_0
{
    packed_uint4 cfg_5;
    packed_int4 off_0;
    packed_float4 c_0;
    packed_float4 fu_1;
    packed_float4 cells_0;
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
    ImpData_natural_0 imp_0;
    SeasonData_natural_0 season_0;
    GrassData_natural_0 grass_0;
    ShellData_natural_0 shell_0;
};

FrameData_natural_0 device* ed_buf_1(uint i_4, KernelContext_0 thread* kernelContext_3)
{
    return (FrameData_natural_0 device*)kernelContext_3->ed_buffers_0[i_4];
}

float t_lift_k_0(const TerrainDraw_natural_0 thread* d_2, float3 p_0, KernelContext_0 thread* kernelContext_4)
{
    float _S16 = (float4(d_2->polar_0) ).w;
    if(_S16 <= 0.0f)
    {
        return 0.0f;
    }
    float px_0 = _S16 / max(length((float4(d_2->origin_0) ).xyz + p_0), 1.0f);
    float2 _S17 = float2(12.0f, 6.0f);
    FrameData_natural_0 device* _S18 = ed_buf_1(kernelContext_4->ed_c_0->frame_0, kernelContext_4);
    float2 r_0;
    if(((float4((&_S18->imp_0)->far_0) ).x) > 0.0f)
    {
        r_0 = (float4((&_S18->imp_0)->far_0) ).xy;
    }
    else
    {
        r_0 = _S17;
    }
    return smoothstep(r_0.x, r_0.y, px_0);
}

uint t_canopy_raw_0(uint pool_3, uint off_1, uint i_5, uint j_3, KernelContext_0 thread* kernelContext_5)
{
    uint device* _S19 = ed_buf_0(pool_3, kernelContext_5);
    return *(_S19 + (off_1 + j_3 * 257U + i_5));
}

float t_canopy_h_0(uint code_0)
{
    float _S20;
    if(code_0 == 0U)
    {
        _S20 = 0.0f;
    }
    else
    {
        _S20 = 0.5f * exp(float(code_0 - 1U) * 0.020444700494408607f);
    }
    return _S20;
}

float t_canopy_lift_0(uint pool_4, uint off_2, uint i_6, uint j_4, KernelContext_0 thread* kernelContext_6)
{
    if(off_2 == 0U)
    {
        return 0.0f;
    }
    uint _S21 = t_canopy_raw_0(pool_4, off_2, i_6, j_4, kernelContext_6);
    return t_canopy_h_0((_S21 >> 8U) & 255U) * smoothstep(0.059999998658895493f, 0.44999998807907104f, float(_S21 & 255U) / 255.0f);
}

float3 t_target_even_0(uint pool_5, const TerrainDraw_natural_0 thread* d_3, uint i_7, uint j_5, KernelContext_0 thread* kernelContext_7)
{
    bool border_0;
    if(i_7 == 0U)
    {
        border_0 = true;
    }
    else
    {
        border_0 = j_5 == 0U;
    }
    if(border_0)
    {
        border_0 = true;
    }
    else
    {
        border_0 = i_7 == 64U;
    }
    if(border_0)
    {
        border_0 = true;
    }
    else
    {
        border_0 = j_5 == 64U;
    }
    bool own_0;
    if(border_0)
    {
        own_0 = true;
    }
    else
    {
        own_0 = ((uint4(d_3->slots_0) ).y) == 4294967295U;
    }
    uint4 _S22 = uint4(d_3->slots_0) ;
    uint _S23 = _S22.z;
    uint qx_0;
    if((_S23 & 16U) != 0U)
    {
        qx_0 = 128U;
    }
    else
    {
        qx_0 = 0U;
    }
    uint qy_0;
    if((_S23 & 32U) != 0U)
    {
        qy_0 = 128U;
    }
    else
    {
        qy_0 = 0U;
    }
    float h_1;
    if(own_0)
    {
        float _S24 = t_own_h_0(pool_5, d_3, i_7, j_5, kernelContext_7);
        h_1 = _S24;
    }
    else
    {
        float4 _S25 = float4(d_3->hq_0) ;
        float _S26 = t_page_0(pool_5, _S22.y, qx_0 + 2U * i_7, qy_0 + 2U * j_5, _S25.z, _S25.w, kernelContext_7);
        h_1 = _S26;
    }
    thread float3 dir_1;
    float3 _S27 = t_lattice_0(d_3, i_7, j_5, h_1, &dir_1);
    float3 p_1;
    if(((float4(d_3->polar_0) ).w) > 0.0f)
    {
        float _S28 = t_lift_k_0(d_3, _S27, kernelContext_7);
        float4 _S29 = float4(d_3->tex_0) ;
        uint poff_0 = (as_type<uint>((_S29.w)));
        float lift_0;
        if(_S28 > 0.0f)
        {
            if(own_0)
            {
                border_0 = true;
            }
            else
            {
                border_0 = poff_0 == 0U;
            }
            if(border_0)
            {
                float _S30 = t_canopy_lift_0(pool_5, (as_type<uint>((_S29.z))), i_7 * 4U, j_5 * 4U, kernelContext_7);
                h_1 = _S30;
            }
            else
            {
                float _S31 = t_canopy_lift_0(pool_5, poff_0, qx_0 + 2U * i_7, qy_0 + 2U * j_5, kernelContext_7);
                h_1 = _S31;
            }
            lift_0 = h_1;
        }
        else
        {
            lift_0 = 0.0f;
        }
        p_1 = _S27 + dir_1 * float3((_S28 * lift_0)) ;
    }
    else
    {
        p_1 = _S27;
    }
    return p_1;
}

float t_own_lift_0(uint pool_6, const TerrainDraw_natural_0 thread* d_4, uint i_8, uint j_6, float3 p_2, KernelContext_0 thread* kernelContext_8)
{
    if(((float4(d_4->polar_0) ).w) <= 0.0f)
    {
        return 0.0f;
    }
    float _S32 = t_lift_k_0(d_4, p_2, kernelContext_8);
    float _S33;
    if(_S32 > 0.0f)
    {
        float _S34 = t_canopy_lift_0(pool_6, (as_type<uint>(((float4(d_4->tex_0) ).z))), i_8 * 4U, j_6 * 4U, kernelContext_8);
        _S33 = _S32 * _S34;
    }
    else
    {
        _S33 = 0.0f;
    }
    return _S33;
}

float3 t_base_0(uint pool_7, const TerrainDraw_natural_0 thread* d_5, uint i_9, uint j_7, KernelContext_0 thread* kernelContext_9)
{
    float _S35 = t_own_h_0(pool_7, d_5, i_9, j_7, kernelContext_9);
    thread float3 dir_2;
    float3 _S36 = t_lattice_0(d_5, i_9, j_7, _S35, &dir_2);
    float3 p_3;
    if(((float4(d_5->polar_0) ).w) > 0.0f)
    {
        float _S37 = t_own_lift_0(pool_7, d_5, i_9, j_7, _S36, kernelContext_9);
        p_3 = _S36 + dir_2 * float3(_S37) ;
    }
    else
    {
        p_3 = _S36;
    }
    return p_3;
}

float t_morph_0(float3 rel_0, float2 band_1)
{
    float _S38 = band_1.x;
    return saturate((length(rel_0) - _S38) / max(band_1.y - _S38, 0.0010000000474974513f));
}

float3 t_coarse_even_0(uint pool_8, const TerrainDraw_natural_0 thread* d_6, uint e_1, uint2 a_0, uint2 b_0, uint2 c_1, KernelContext_0 thread* kernelContext_10)
{
    float3 _S39 = t_base_0(pool_8, d_6, a_0.x, a_0.y, kernelContext_10);
    if((e_1 & 3U) == 0U)
    {
        return _S39;
    }
    float m_0 = t_morph_0((float4(d_6->origin_0) ).xyz + _S39, (float4(d_6->morph_0) ).zw);
    float3 _S40 = t_base_0(pool_8, d_6, b_0.x, b_0.y, kernelContext_10);
    float3 _S41 = t_base_0(pool_8, d_6, c_1.x, c_1.y, kernelContext_10);
    return mix(_S39, float3(0.5f)  * (_S40 + _S41), float3(m_0) );
}

float3 t_target_0(uint pool_9, const TerrainDraw_natural_0 thread* d_7, uint i_10, uint j_8, KernelContext_0 thread* kernelContext_11)
{
    bool ei_0 = (i_10 & 1U) == 0U;
    bool ej_0 = (j_8 & 1U) == 0U;
    bool _S42;
    if(ei_0)
    {
        _S42 = ej_0;
    }
    else
    {
        _S42 = false;
    }
    if(_S42)
    {
        float3 _S43 = t_target_even_0(pool_9, d_7, i_10, j_8, kernelContext_11);
        return _S43;
    }
    if(!ei_0)
    {
        _S42 = ej_0;
    }
    else
    {
        _S42 = false;
    }
    if(_S42)
    {
        float3 _S44 = t_target_even_0(pool_9, d_7, i_10 - 1U, j_8, kernelContext_11);
        float3 _S45 = t_target_even_0(pool_9, d_7, i_10 + 1U, j_8, kernelContext_11);
        return float3(0.5f)  * (_S44 + _S45);
    }
    if(ei_0)
    {
        _S42 = !ej_0;
    }
    else
    {
        _S42 = false;
    }
    if(_S42)
    {
        float3 _S46 = t_target_even_0(pool_9, d_7, i_10, j_8 - 1U, kernelContext_11);
        float3 _S47 = t_target_even_0(pool_9, d_7, i_10, j_8 + 1U, kernelContext_11);
        return float3(0.5f)  * (_S46 + _S47);
    }
    float3 _S48 = t_target_even_0(pool_9, d_7, i_10 - 1U, j_8 - 1U, kernelContext_11);
    float3 _S49 = t_target_even_0(pool_9, d_7, i_10 + 1U, j_8 + 1U, kernelContext_11);
    return float3(0.5f)  * (_S48 + _S49);
}

float3 t_target_even_1(uint pool_10, const TerrainDraw_natural_0 thread* d_8, uint i_11, uint j_9, KernelContext_0 thread* kernelContext_12)
{
    bool border_1;
    if(i_11 == 0U)
    {
        border_1 = true;
    }
    else
    {
        border_1 = j_9 == 0U;
    }
    if(border_1)
    {
        border_1 = true;
    }
    else
    {
        border_1 = i_11 == 64U;
    }
    if(border_1)
    {
        border_1 = true;
    }
    else
    {
        border_1 = j_9 == 64U;
    }
    bool own_1;
    if(border_1)
    {
        own_1 = true;
    }
    else
    {
        own_1 = ((uint4(d_8->slots_0) ).y) == 4294967295U;
    }
    uint4 _S50 = uint4(d_8->slots_0) ;
    uint _S51 = _S50.z;
    uint qx_1;
    if((_S51 & 16U) != 0U)
    {
        qx_1 = 128U;
    }
    else
    {
        qx_1 = 0U;
    }
    uint qy_1;
    if((_S51 & 32U) != 0U)
    {
        qy_1 = 128U;
    }
    else
    {
        qy_1 = 0U;
    }
    float h_2;
    if(own_1)
    {
        float _S52 = t_own_h_0(pool_10, d_8, i_11, j_9, kernelContext_12);
        h_2 = _S52;
    }
    else
    {
        float4 _S53 = float4(d_8->hq_0) ;
        float _S54 = t_page_0(pool_10, _S50.y, qx_1 + 2U * i_11, qy_1 + 2U * j_9, _S53.z, _S53.w, kernelContext_12);
        h_2 = _S54;
    }
    thread float3 dir_3;
    float3 _S55 = t_lattice_0(d_8, i_11, j_9, h_2, &dir_3);
    return _S55;
}

float3 t_base_1(uint pool_11, const TerrainDraw_natural_0 thread* d_9, uint i_12, uint j_10, KernelContext_0 thread* kernelContext_13)
{
    float _S56 = t_own_h_0(pool_11, d_9, i_12, j_10, kernelContext_13);
    thread float3 dir_4;
    float3 _S57 = t_lattice_0(d_9, i_12, j_10, _S56, &dir_4);
    return _S57;
}

float3 t_coarse_even_1(uint pool_12, const TerrainDraw_natural_0 thread* d_10, uint e_2, uint2 a_1, uint2 b_1, uint2 c_2, KernelContext_0 thread* kernelContext_14)
{
    float3 _S58 = t_base_1(pool_12, d_10, a_1.x, a_1.y, kernelContext_14);
    if((e_2 & 3U) == 0U)
    {
        return _S58;
    }
    float m_1 = t_morph_0((float4(d_10->origin_0) ).xyz + _S58, (float4(d_10->morph_0) ).zw);
    float3 _S59 = t_base_1(pool_12, d_10, b_1.x, b_1.y, kernelContext_14);
    float3 _S60 = t_base_1(pool_12, d_10, c_2.x, c_2.y, kernelContext_14);
    return mix(_S58, float3(0.5f)  * (_S59 + _S60), float3(m_1) );
}

float3 t_coarse_even_2(uint pool_13, const TerrainDraw_natural_0 thread* d_11, uint e_3, uint2 a_2, uint2 b_2, uint2 c_3, KernelContext_0 thread* kernelContext_15)
{
    float3 _S61 = t_base_1(pool_13, d_11, a_2.x, a_2.y, kernelContext_15);
    if((e_3 & 3U) == 0U)
    {
        return _S61;
    }
    float m_2 = t_morph_0((float4(d_11->origin_0) ).xyz + _S61, (float4(d_11->morph_0) ).zw);
    float3 _S62 = t_base_1(pool_13, d_11, b_2.x, b_2.y, kernelContext_15);
    float3 _S63 = t_base_1(pool_13, d_11, c_3.x, c_3.y, kernelContext_15);
    return mix(_S61, float3(0.5f)  * (_S62 + _S63), float3(m_2) );
}

float3 t_target_1(uint pool_14, const TerrainDraw_natural_0 thread* d_12, uint i_13, uint j_11, KernelContext_0 thread* kernelContext_16)
{
    bool ei_1 = (i_13 & 1U) == 0U;
    bool ej_1 = (j_11 & 1U) == 0U;
    bool _S64;
    if(ei_1)
    {
        _S64 = ej_1;
    }
    else
    {
        _S64 = false;
    }
    if(_S64)
    {
        float3 _S65 = t_target_even_1(pool_14, d_12, i_13, j_11, kernelContext_16);
        return _S65;
    }
    if(!ei_1)
    {
        _S64 = ej_1;
    }
    else
    {
        _S64 = false;
    }
    if(_S64)
    {
        float3 _S66 = t_target_even_1(pool_14, d_12, i_13 - 1U, j_11, kernelContext_16);
        float3 _S67 = t_target_even_1(pool_14, d_12, i_13 + 1U, j_11, kernelContext_16);
        return float3(0.5f)  * (_S66 + _S67);
    }
    if(ei_1)
    {
        _S64 = !ej_1;
    }
    else
    {
        _S64 = false;
    }
    if(_S64)
    {
        float3 _S68 = t_target_even_1(pool_14, d_12, i_13, j_11 - 1U, kernelContext_16);
        float3 _S69 = t_target_even_1(pool_14, d_12, i_13, j_11 + 1U, kernelContext_16);
        return float3(0.5f)  * (_S68 + _S69);
    }
    float3 _S70 = t_target_even_1(pool_14, d_12, i_13 - 1U, j_11 - 1U, kernelContext_16);
    float3 _S71 = t_target_even_1(pool_14, d_12, i_13 + 1U, j_11 + 1U, kernelContext_16);
    return float3(0.5f)  * (_S70 + _S71);
}

uint2 t_edge_at_0(uint edge_0, uint e_4)
{
    switch(edge_0)
    {
    case 0U:
        {
            return uint2(0U, e_4);
        }
    case 1U:
        {
            return uint2(64U, e_4);
        }
    case 2U:
        {
            return uint2(e_4, 0U);
        }
    default:
        {
            return uint2(e_4, 64U);
        }
    }
}

float3 t_coarse_0(uint pool_15, const TerrainDraw_natural_0 thread* d_13, uint edge_1, uint e_5, KernelContext_0 thread* kernelContext_17)
{
    if((e_5 & 1U) == 0U)
    {
        float3 _S72 = t_coarse_even_2(pool_15, d_13, e_5, t_edge_at_0(edge_1, e_5), t_edge_at_0(edge_1, e_5 - 2U), t_edge_at_0(edge_1, e_5 + 2U), kernelContext_17);
        return _S72;
    }
    uint _S73 = e_5 - 1U;
    uint2 _S74 = t_edge_at_0(edge_1, _S73);
    uint _S75 = e_5 + 1U;
    uint2 _S76 = t_edge_at_0(edge_1, _S75);
    float3 _S77 = t_coarse_even_2(pool_15, d_13, _S73, _S74, t_edge_at_0(edge_1, e_5 - 3U), _S76, kernelContext_17);
    float3 _S78 = t_coarse_even_2(pool_15, d_13, _S75, _S76, _S74, t_edge_at_0(edge_1, e_5 + 3U), kernelContext_17);
    return float3(0.5f)  * (_S77 + _S78);
}

float3 t_grid_vertex_0(uint pool_16, const TerrainDraw_natural_0 thread* d_14, uint i_14, uint j_12, KernelContext_0 thread* kernelContext_18)
{
    uint fl_0 = (uint4(d_14->slots_0) ).z;
    uint edge_2;
    uint e_6;
    if(i_14 == 0U)
    {
        edge_2 = 0U;
        e_6 = j_12;
    }
    else
    {
        if(i_14 == 64U)
        {
            edge_2 = 1U;
            e_6 = j_12;
        }
        else
        {
            if(j_12 == 0U)
            {
                edge_2 = 2U;
                e_6 = i_14;
            }
            else
            {
                if(j_12 == 64U)
                {
                    edge_2 = 3U;
                    e_6 = i_14;
                }
                else
                {
                    edge_2 = 4U;
                    e_6 = 0U;
                }
            }
        }
    }
    if(edge_2 < 4U)
    {
        uint E_0 = 1U << ((fl_0 >> (10U + 3U * edge_2)) & 7U);
        bool _S79;
        if((fl_0 & (1U << edge_2)) != 0U)
        {
            _S79 = E_0 <= 2U;
        }
        else
        {
            _S79 = false;
        }
        if(_S79)
        {
            float3 _S80 = t_coarse_0(pool_16, d_14, edge_2, e_6, kernelContext_18);
            return _S80;
        }
        if(E_0 > 1U)
        {
            uint lo_0 = e_6 & (~(E_0 - 1U));
            if(lo_0 == e_6)
            {
                float3 _S81 = t_base_1(pool_16, d_14, i_14, j_12, kernelContext_18);
                return _S81;
            }
            uint2 a_3 = t_edge_at_0(edge_2, lo_0);
            uint2 b_3 = t_edge_at_0(edge_2, lo_0 + E_0);
            float3 _S82 = t_base_1(pool_16, d_14, a_3.x, a_3.y, kernelContext_18);
            float3 _S83 = t_base_1(pool_16, d_14, b_3.x, b_3.y, kernelContext_18);
            return mix(_S82, _S83, float3((float(e_6 - lo_0) / float(E_0))) );
        }
    }
    float3 _S84 = t_base_1(pool_16, d_14, i_14, j_12, kernelContext_18);
    if((fl_0 & 4194304U) != 0U)
    {
        return _S84;
    }
    float m_3 = t_morph_0((float4(d_14->origin_0) ).xyz + _S84, (float4(d_14->morph_0) ).xy);
    if(m_3 <= 0.0f)
    {
        return _S84;
    }
    float3 _S85 = t_target_1(pool_16, d_14, i_14, j_12, kernelContext_18);
    return mix(_S84, _S85, float3(m_3) );
}

struct GrassType_natural_0
{
    packed_float4 a_4;
    packed_float4 b_4;
    packed_float4 root_0;
    packed_float4 mid_0;
    packed_float4 tip_0;
    packed_float4 dry_0;
    packed_float4 shell_1;
    packed_float4 carpet_0;
};

GrassType_natural_0 device* ed_buf_2(uint i_15, KernelContext_0 thread* kernelContext_19)
{
    return (GrassType_natural_0 device*)kernelContext_19->ed_buffers_0[i_15];
}

uint2 t_ring_0(uint s_2)
{
    uint e_7 = s_2 % 64U;
    switch(s_2 / 64U)
    {
    case 0U:
        {
            return uint2(e_7, 0U);
        }
    case 1U:
        {
            return uint2(64U, e_7);
        }
    case 2U:
        {
            return uint2(64U - e_7, 64U);
        }
    default:
        {
            return uint2(0U, 64U - e_7);
        }
    }
}

float3 t_vertex_0(uint pool_17, const TerrainDraw_natural_0 thread* d_15, uint v_0, KernelContext_0 thread* kernelContext_20)
{
    if(v_0 < 4225U)
    {
        float3 _S86 = (float4(d_15->origin_0) ).xyz;
        float3 _S87 = t_grid_vertex_0(pool_17, d_15, v_0 % 65U, v_0 / 65U, kernelContext_20);
        return _S86 + _S87;
    }
    uint2 ij_0 = t_ring_0(v_0 - 4225U);
    float3 _S88 = t_grid_vertex_0(pool_17, d_15, ij_0.x, ij_0.y, kernelContext_20);
    float depth_0;
    if((((uint4(d_15->slots_0) ).z) & 64U) != 0U)
    {
        depth_0 = (float4(d_15->origin_0) ).w;
    }
    else
    {
        depth_0 = 0.0f;
    }
    float3 _S89 = (float4(d_15->origin_0) ).xyz;
    float3 _S90 = _S89 + _S88;
    float4 _S91 = float4(d_15->dir_c_0) ;
    return _S90 - normalize(_S90 - (_S89 - _S91.xyz * float3(_S91.w) )) * float3(depth_0) ;
}

struct TerrainShadowOut_0
{
    float4 pos_0;
};

TerrainDraw_natural_0 device* ed_buf_3(uint i_16, KernelContext_0 thread* kernelContext_21)
{
    return (TerrainDraw_natural_0 device*)kernelContext_21->ed_buffers_0[i_16];
}

float t_own_h_1(uint _S92, TerrainDraw_natural_0 device* _S93, uint _S94, uint _S95, KernelContext_0 thread* kernelContext_22)
{
    float4 _S96 = float4(_S93->hq_0) ;
    float _S97 = t_page_0(_S92, (uint4(_S93->slots_0) ).x, _S94 * 4U, _S95 * 4U, _S96.x, _S96.y, kernelContext_22);
    return _S97;
}

float3 t_base_2(uint _S98, TerrainDraw_natural_0 device* _S99, uint _S100, uint _S101, KernelContext_0 thread* kernelContext_23)
{
    TerrainDraw_natural_0 _S102 = *_S99;
    float _S103 = t_own_h_1(_S98, _S99, _S100, _S101, kernelContext_23);
    thread TerrainDraw_natural_0 _S104 = _S102;
    thread float3 dir_5;
    float3 _S105 = t_lattice_0(&_S104, _S100, _S101, _S103, &dir_5);
    float3 p_4;
    if(((float4((&_S104)->polar_0) ).w) > 0.0f)
    {
        float _S106 = t_own_lift_0(_S98, &_S104, _S100, _S101, _S105, kernelContext_23);
        p_4 = _S105 + dir_5 * float3(_S106) ;
    }
    else
    {
        p_4 = _S105;
    }
    return p_4;
}

float3 t_coarse_even_3(uint _S107, TerrainDraw_natural_0 device* _S108, uint _S109, uint2 _S110, uint2 _S111, uint2 _S112, KernelContext_0 thread* kernelContext_24)
{
    thread TerrainDraw_natural_0 _S113 = *_S108;
    float3 _S114 = t_base_2(_S107, _S108, _S110.x, _S110.y, kernelContext_24);
    if((_S109 & 3U) == 0U)
    {
        return _S114;
    }
    float m_4 = t_morph_0((float4((&_S113)->origin_0) ).xyz + _S114, (float4((&_S113)->morph_0) ).zw);
    float3 _S115 = t_base_0(_S107, &_S113, _S111.x, _S111.y, kernelContext_24);
    float3 _S116 = t_base_0(_S107, &_S113, _S112.x, _S112.y, kernelContext_24);
    return mix(_S114, float3(0.5f)  * (_S115 + _S116), float3(m_4) );
}

float3 t_coarse_1(uint _S117, TerrainDraw_natural_0 device* _S118, uint _S119, uint _S120, KernelContext_0 thread* kernelContext_25)
{
    thread TerrainDraw_natural_0 _S121 = *_S118;
    if((_S120 & 1U) == 0U)
    {
        float3 _S122 = t_coarse_even_3(_S117, _S118, _S120, t_edge_at_0(_S119, _S120), t_edge_at_0(_S119, _S120 - 2U), t_edge_at_0(_S119, _S120 + 2U), kernelContext_25);
        return _S122;
    }
    uint _S123 = _S120 - 1U;
    uint2 _S124 = t_edge_at_0(_S119, _S123);
    uint _S125 = _S120 + 1U;
    uint2 _S126 = t_edge_at_0(_S119, _S125);
    float3 _S127 = t_coarse_even_3(_S117, _S118, _S123, _S124, t_edge_at_0(_S119, _S120 - 3U), _S126, kernelContext_25);
    float3 _S128 = t_coarse_even_0(_S117, &_S121, _S125, _S126, _S124, t_edge_at_0(_S119, _S120 + 3U), kernelContext_25);
    return float3(0.5f)  * (_S127 + _S128);
}

float3 t_grid_vertex_1(uint _S129, TerrainDraw_natural_0 device* _S130, uint _S131, uint _S132, KernelContext_0 thread* kernelContext_26)
{
    thread TerrainDraw_natural_0 _S133 = *_S130;
    uint fl_1 = (uint4((&_S133)->slots_0) ).z;
    uint edge_3;
    uint e_8;
    if(_S131 == 0U)
    {
        edge_3 = 0U;
        e_8 = _S132;
    }
    else
    {
        if(_S131 == 64U)
        {
            edge_3 = 1U;
            e_8 = _S132;
        }
        else
        {
            if(_S132 == 0U)
            {
                edge_3 = 2U;
                e_8 = _S131;
            }
            else
            {
                if(_S132 == 64U)
                {
                    edge_3 = 3U;
                    e_8 = _S131;
                }
                else
                {
                    edge_3 = 4U;
                    e_8 = 0U;
                }
            }
        }
    }
    if(edge_3 < 4U)
    {
        uint E_1 = 1U << ((fl_1 >> (10U + 3U * edge_3)) & 7U);
        bool _S134;
        if((fl_1 & (1U << edge_3)) != 0U)
        {
            _S134 = E_1 <= 2U;
        }
        else
        {
            _S134 = false;
        }
        if(_S134)
        {
            float3 _S135 = t_coarse_1(_S129, _S130, edge_3, e_8, kernelContext_26);
            return _S135;
        }
        if(E_1 > 1U)
        {
            uint lo_1 = e_8 & (~(E_1 - 1U));
            if(lo_1 == e_8)
            {
                float3 _S136 = t_base_2(_S129, _S130, _S131, _S132, kernelContext_26);
                return _S136;
            }
            uint2 a_5 = t_edge_at_0(edge_3, lo_1);
            uint2 b_5 = t_edge_at_0(edge_3, lo_1 + E_1);
            float3 _S137 = t_base_2(_S129, _S130, a_5.x, a_5.y, kernelContext_26);
            float3 _S138 = t_base_0(_S129, &_S133, b_5.x, b_5.y, kernelContext_26);
            return mix(_S137, _S138, float3((float(e_8 - lo_1) / float(E_1))) );
        }
    }
    float3 _S139 = t_base_2(_S129, _S130, _S131, _S132, kernelContext_26);
    if((fl_1 & 4194304U) != 0U)
    {
        return _S139;
    }
    float m_5 = t_morph_0((float4((&_S133)->origin_0) ).xyz + _S139, (float4((&_S133)->morph_0) ).xy);
    if(m_5 <= 0.0f)
    {
        return _S139;
    }
    float3 _S140 = t_target_0(_S129, &_S133, _S131, _S132, kernelContext_26);
    return mix(_S139, _S140, float3(m_5) );
}

float3 t_vertex_1(uint _S141, TerrainDraw_natural_0 device* _S142, uint _S143, KernelContext_0 thread* kernelContext_27)
{
    thread TerrainDraw_natural_0 _S144 = *_S142;
    if(_S143 < 4225U)
    {
        float3 _S145 = (float4((&_S144)->origin_0) ).xyz;
        float3 _S146 = t_grid_vertex_1(_S141, _S142, _S143 % 65U, _S143 / 65U, kernelContext_27);
        return _S145 + _S146;
    }
    uint2 ij_1 = t_ring_0(_S143 - 4225U);
    uint _S147 = ij_1.x;
    uint _S148 = ij_1.y;
    float3 _S149 = t_grid_vertex_1(_S141, _S142, _S147, _S148, kernelContext_27);
    float depth_1;
    if((((uint4((&_S144)->slots_0) ).z) & 64U) != 0U)
    {
        depth_1 = (float4((&_S144)->origin_0) ).w;
    }
    else
    {
        depth_1 = 0.0f;
    }
    bool _S150;
    if(((float4((&_S144)->polar_0) ).w) > 0.0f)
    {
        _S150 = depth_1 > 0.0f;
    }
    else
    {
        _S150 = false;
    }
    if(_S150)
    {
        float _S151 = t_own_lift_0(_S141, &_S144, _S147, _S148, _S149, kernelContext_27);
        depth_1 = depth_1 + _S151;
    }
    float3 _S152 = (float4((&_S144)->origin_0) ).xyz;
    float3 _S153 = _S152 + _S149;
    float4 _S154 = float4((&_S144)->dir_c_0) ;
    return _S153 - normalize(_S153 - (_S152 - _S154.xyz * float3(_S154.w) )) * float3(depth_1) ;
}

TerrainShadowOut_0 terrain_shadow_vs_0(uint vid_0, uint iid_0, KernelContext_0 thread* kernelContext_28)
{
    FrameData_natural_0 device* _S155 = ed_buf_1(kernelContext_28->ed_c_0->frame_0, kernelContext_28);
    TerrainDraw_natural_0 device* _S156 = ed_buf_3(kernelContext_28->ed_c_0->draws_0, kernelContext_28);
    float3 _S157 = t_vertex_1(kernelContext_28->ed_c_0->pool_0, _S156 + (kernelContext_28->ed_c_0->base_0 + iid_0), vid_0, kernelContext_28);
    thread TerrainShadowOut_0 o_0;
    (&o_0)->pos_0 = (((float4(_S157, 1.0f)) * (matrix<float,int(4),int(4)> ((&(&_S155->light_0)->shadow_0)->data_2[kernelContext_28->ed_c_0->cascade_0].data_0[int(0)][int(0)], (&(&_S155->light_0)->shadow_0)->data_2[kernelContext_28->ed_c_0->cascade_0].data_0[int(1)][int(0)], (&(&_S155->light_0)->shadow_0)->data_2[kernelContext_28->ed_c_0->cascade_0].data_0[int(2)][int(0)], (&(&_S155->light_0)->shadow_0)->data_2[kernelContext_28->ed_c_0->cascade_0].data_0[int(3)][int(0)], (&(&_S155->light_0)->shadow_0)->data_2[kernelContext_28->ed_c_0->cascade_0].data_0[int(0)][int(1)], (&(&_S155->light_0)->shadow_0)->data_2[kernelContext_28->ed_c_0->cascade_0].data_0[int(1)][int(1)], (&(&_S155->light_0)->shadow_0)->data_2[kernelContext_28->ed_c_0->cascade_0].data_0[int(2)][int(1)], (&(&_S155->light_0)->shadow_0)->data_2[kernelContext_28->ed_c_0->cascade_0].data_0[int(3)][int(1)], (&(&_S155->light_0)->shadow_0)->data_2[kernelContext_28->ed_c_0->cascade_0].data_0[int(0)][int(2)], (&(&_S155->light_0)->shadow_0)->data_2[kernelContext_28->ed_c_0->cascade_0].data_0[int(1)][int(2)], (&(&_S155->light_0)->shadow_0)->data_2[kernelContext_28->ed_c_0->cascade_0].data_0[int(2)][int(2)], (&(&_S155->light_0)->shadow_0)->data_2[kernelContext_28->ed_c_0->cascade_0].data_0[int(3)][int(2)], (&(&_S155->light_0)->shadow_0)->data_2[kernelContext_28->ed_c_0->cascade_0].data_0[int(0)][int(3)], (&(&_S155->light_0)->shadow_0)->data_2[kernelContext_28->ed_c_0->cascade_0].data_0[int(1)][int(3)], (&(&_S155->light_0)->shadow_0)->data_2[kernelContext_28->ed_c_0->cascade_0].data_0[int(2)][int(3)], (&(&_S155->light_0)->shadow_0)->data_2[kernelContext_28->ed_c_0->cascade_0].data_0[int(3)][int(3)]))));
    return o_0;
}

float3 t_base_3(uint _S158, TerrainDraw_natural_0 device* _S159, uint _S160, uint _S161, KernelContext_0 thread* kernelContext_29)
{
    TerrainDraw_natural_0 _S162 = *_S159;
    float _S163 = t_own_h_1(_S158, _S159, _S160, _S161, kernelContext_29);
    thread TerrainDraw_natural_0 _S164 = _S162;
    thread float3 dir_6;
    float3 _S165 = t_lattice_0(&_S164, _S160, _S161, _S163, &dir_6);
    return _S165;
}

float3 t_coarse_even_4(uint _S166, TerrainDraw_natural_0 device* _S167, uint _S168, uint2 _S169, uint2 _S170, uint2 _S171, KernelContext_0 thread* kernelContext_30)
{
    thread TerrainDraw_natural_0 _S172 = *_S167;
    float3 _S173 = t_base_3(_S166, _S167, _S169.x, _S169.y, kernelContext_30);
    if((_S168 & 3U) == 0U)
    {
        return _S173;
    }
    float m_6 = t_morph_0((float4((&_S172)->origin_0) ).xyz + _S173, (float4((&_S172)->morph_0) ).zw);
    float3 _S174 = t_base_1(_S166, &_S172, _S170.x, _S170.y, kernelContext_30);
    float3 _S175 = t_base_1(_S166, &_S172, _S171.x, _S171.y, kernelContext_30);
    return mix(_S173, float3(0.5f)  * (_S174 + _S175), float3(m_6) );
}

float3 t_coarse_2(uint _S176, TerrainDraw_natural_0 device* _S177, uint _S178, uint _S179, KernelContext_0 thread* kernelContext_31)
{
    thread TerrainDraw_natural_0 _S180 = *_S177;
    if((_S179 & 1U) == 0U)
    {
        float3 _S181 = t_coarse_even_4(_S176, _S177, _S179, t_edge_at_0(_S178, _S179), t_edge_at_0(_S178, _S179 - 2U), t_edge_at_0(_S178, _S179 + 2U), kernelContext_31);
        return _S181;
    }
    uint _S182 = _S179 - 1U;
    uint2 _S183 = t_edge_at_0(_S178, _S182);
    uint _S184 = _S179 + 1U;
    uint2 _S185 = t_edge_at_0(_S178, _S184);
    float3 _S186 = t_coarse_even_4(_S176, _S177, _S182, _S183, t_edge_at_0(_S178, _S179 - 3U), _S185, kernelContext_31);
    float3 _S187 = t_coarse_even_1(_S176, &_S180, _S184, _S185, _S183, t_edge_at_0(_S178, _S179 + 3U), kernelContext_31);
    return float3(0.5f)  * (_S186 + _S187);
}

float3 t_grid_vertex_2(uint _S188, TerrainDraw_natural_0 device* _S189, uint _S190, uint _S191, KernelContext_0 thread* kernelContext_32)
{
    thread TerrainDraw_natural_0 _S192 = *_S189;
    uint fl_2 = (uint4((&_S192)->slots_0) ).z;
    uint edge_4;
    uint e_9;
    if(_S190 == 0U)
    {
        edge_4 = 0U;
        e_9 = _S191;
    }
    else
    {
        if(_S190 == 64U)
        {
            edge_4 = 1U;
            e_9 = _S191;
        }
        else
        {
            if(_S191 == 0U)
            {
                edge_4 = 2U;
                e_9 = _S190;
            }
            else
            {
                if(_S191 == 64U)
                {
                    edge_4 = 3U;
                    e_9 = _S190;
                }
                else
                {
                    edge_4 = 4U;
                    e_9 = 0U;
                }
            }
        }
    }
    if(edge_4 < 4U)
    {
        uint E_2 = 1U << ((fl_2 >> (10U + 3U * edge_4)) & 7U);
        bool _S193;
        if((fl_2 & (1U << edge_4)) != 0U)
        {
            _S193 = E_2 <= 2U;
        }
        else
        {
            _S193 = false;
        }
        if(_S193)
        {
            float3 _S194 = t_coarse_2(_S188, _S189, edge_4, e_9, kernelContext_32);
            return _S194;
        }
        if(E_2 > 1U)
        {
            uint lo_2 = e_9 & (~(E_2 - 1U));
            if(lo_2 == e_9)
            {
                float3 _S195 = t_base_3(_S188, _S189, _S190, _S191, kernelContext_32);
                return _S195;
            }
            uint2 a_6 = t_edge_at_0(edge_4, lo_2);
            uint2 b_6 = t_edge_at_0(edge_4, lo_2 + E_2);
            float3 _S196 = t_base_3(_S188, _S189, a_6.x, a_6.y, kernelContext_32);
            float3 _S197 = t_base_1(_S188, &_S192, b_6.x, b_6.y, kernelContext_32);
            return mix(_S196, _S197, float3((float(e_9 - lo_2) / float(E_2))) );
        }
    }
    float3 _S198 = t_base_3(_S188, _S189, _S190, _S191, kernelContext_32);
    if((fl_2 & 4194304U) != 0U)
    {
        return _S198;
    }
    float m_7 = t_morph_0((float4((&_S192)->origin_0) ).xyz + _S198, (float4((&_S192)->morph_0) ).xy);
    if(m_7 <= 0.0f)
    {
        return _S198;
    }
    float3 _S199 = t_target_1(_S188, &_S192, _S190, _S191, kernelContext_32);
    return mix(_S198, _S199, float3(m_7) );
}

float3 t_vertex_2(uint _S200, TerrainDraw_natural_0 device* _S201, uint _S202, KernelContext_0 thread* kernelContext_33)
{
    TerrainDraw_natural_0 _S203 = *_S201;
    if(_S202 < 4225U)
    {
        float3 _S204 = (float4(_S203.origin_0) ).xyz;
        float3 _S205 = t_grid_vertex_2(_S200, _S201, _S202 % 65U, _S202 / 65U, kernelContext_33);
        return _S204 + _S205;
    }
    uint2 ij_2 = t_ring_0(_S202 - 4225U);
    float3 _S206 = t_grid_vertex_2(_S200, _S201, ij_2.x, ij_2.y, kernelContext_33);
    float depth_2;
    if((((uint4(_S203.slots_0) ).z) & 64U) != 0U)
    {
        depth_2 = (float4(_S203.origin_0) ).w;
    }
    else
    {
        depth_2 = 0.0f;
    }
    float3 _S207 = (float4(_S203.origin_0) ).xyz;
    float3 _S208 = _S207 + _S206;
    float4 _S209 = float4(_S203.dir_c_0) ;
    return _S208 - normalize(_S208 - (_S207 - _S209.xyz * float3(_S209.w) )) * float3(depth_2) ;
}

TerrainShadowOut_0 terrain_shadow_vs_1(uint vid_1, uint iid_1, KernelContext_0 thread* kernelContext_34)
{
    FrameData_natural_0 device* _S210 = ed_buf_1(kernelContext_34->ed_c_0->frame_0, kernelContext_34);
    TerrainDraw_natural_0 device* _S211 = ed_buf_3(kernelContext_34->ed_c_0->draws_0, kernelContext_34);
    float3 _S212 = t_vertex_2(kernelContext_34->ed_c_0->pool_0, _S211 + (kernelContext_34->ed_c_0->base_0 + iid_1), vid_1, kernelContext_34);
    thread TerrainShadowOut_0 o_1;
    (&o_1)->pos_0 = (((float4(_S212, 1.0f)) * (matrix<float,int(4),int(4)> ((&(&_S210->light_0)->shadow_0)->data_2[kernelContext_34->ed_c_0->cascade_0].data_0[int(0)][int(0)], (&(&_S210->light_0)->shadow_0)->data_2[kernelContext_34->ed_c_0->cascade_0].data_0[int(1)][int(0)], (&(&_S210->light_0)->shadow_0)->data_2[kernelContext_34->ed_c_0->cascade_0].data_0[int(2)][int(0)], (&(&_S210->light_0)->shadow_0)->data_2[kernelContext_34->ed_c_0->cascade_0].data_0[int(3)][int(0)], (&(&_S210->light_0)->shadow_0)->data_2[kernelContext_34->ed_c_0->cascade_0].data_0[int(0)][int(1)], (&(&_S210->light_0)->shadow_0)->data_2[kernelContext_34->ed_c_0->cascade_0].data_0[int(1)][int(1)], (&(&_S210->light_0)->shadow_0)->data_2[kernelContext_34->ed_c_0->cascade_0].data_0[int(2)][int(1)], (&(&_S210->light_0)->shadow_0)->data_2[kernelContext_34->ed_c_0->cascade_0].data_0[int(3)][int(1)], (&(&_S210->light_0)->shadow_0)->data_2[kernelContext_34->ed_c_0->cascade_0].data_0[int(0)][int(2)], (&(&_S210->light_0)->shadow_0)->data_2[kernelContext_34->ed_c_0->cascade_0].data_0[int(1)][int(2)], (&(&_S210->light_0)->shadow_0)->data_2[kernelContext_34->ed_c_0->cascade_0].data_0[int(2)][int(2)], (&(&_S210->light_0)->shadow_0)->data_2[kernelContext_34->ed_c_0->cascade_0].data_0[int(3)][int(2)], (&(&_S210->light_0)->shadow_0)->data_2[kernelContext_34->ed_c_0->cascade_0].data_0[int(0)][int(3)], (&(&_S210->light_0)->shadow_0)->data_2[kernelContext_34->ed_c_0->cascade_0].data_0[int(1)][int(3)], (&(&_S210->light_0)->shadow_0)->data_2[kernelContext_34->ed_c_0->cascade_0].data_0[int(2)][int(3)], (&(&_S210->light_0)->shadow_0)->data_2[kernelContext_34->ed_c_0->cascade_0].data_0[int(3)][int(3)]))));
    return o_1;
}

struct TerrainOut_0
{
    float4 pos_1;
    [[flat]] uint draw_0;
    [[flat]] uint lod_1;
};

TerrainOut_0 terrain_vs_0(uint vid_2, uint iid_2, KernelContext_0 thread* kernelContext_35)
{
    FrameData_natural_0 device* _S213 = ed_buf_1(kernelContext_35->ed_c_0->frame_0, kernelContext_35);
    TerrainDraw_natural_0 device* _S214 = ed_buf_3(kernelContext_35->ed_c_0->draws_0, kernelContext_35);
    TerrainDraw_natural_0 device* _S215 = _S214 + (kernelContext_35->ed_c_0->base_0 + iid_2);
    TerrainDraw_natural_0 d_16 = *_S215;
    float3 _S216 = t_vertex_1(kernelContext_35->ed_c_0->pool_0, _S215, vid_2, kernelContext_35);
    thread TerrainOut_0 o_2;
    (&o_2)->pos_1 = (((float4(_S216, 1.0f)) * (matrix<float,int(4),int(4)> (_S213->view_proj_0.data_0[int(0)][int(0)], _S213->view_proj_0.data_0[int(1)][int(0)], _S213->view_proj_0.data_0[int(2)][int(0)], _S213->view_proj_0.data_0[int(3)][int(0)], _S213->view_proj_0.data_0[int(0)][int(1)], _S213->view_proj_0.data_0[int(1)][int(1)], _S213->view_proj_0.data_0[int(2)][int(1)], _S213->view_proj_0.data_0[int(3)][int(1)], _S213->view_proj_0.data_0[int(0)][int(2)], _S213->view_proj_0.data_0[int(1)][int(2)], _S213->view_proj_0.data_0[int(2)][int(2)], _S213->view_proj_0.data_0[int(3)][int(2)], _S213->view_proj_0.data_0[int(0)][int(3)], _S213->view_proj_0.data_0[int(1)][int(3)], _S213->view_proj_0.data_0[int(2)][int(3)], _S213->view_proj_0.data_0[int(3)][int(3)]))));
    (&o_2)->draw_0 = kernelContext_35->ed_c_0->base_0 + iid_2;
    (&o_2)->lod_1 = (((uint4(d_16.slots_0) ).z) >> 8U) & 3U;
    return o_2;
}

TerrainOut_0 terrain_vs_1(uint vid_3, uint iid_3, KernelContext_0 thread* kernelContext_36)
{
    FrameData_natural_0 device* _S217 = ed_buf_1(kernelContext_36->ed_c_0->frame_0, kernelContext_36);
    TerrainDraw_natural_0 device* _S218 = ed_buf_3(kernelContext_36->ed_c_0->draws_0, kernelContext_36);
    TerrainDraw_natural_0 device* _S219 = _S218 + (kernelContext_36->ed_c_0->base_0 + iid_3);
    TerrainDraw_natural_0 d_17 = *_S219;
    float3 _S220 = t_vertex_2(kernelContext_36->ed_c_0->pool_0, _S219, vid_3, kernelContext_36);
    thread TerrainOut_0 o_3;
    (&o_3)->pos_1 = (((float4(_S220, 1.0f)) * (matrix<float,int(4),int(4)> (_S217->view_proj_0.data_0[int(0)][int(0)], _S217->view_proj_0.data_0[int(1)][int(0)], _S217->view_proj_0.data_0[int(2)][int(0)], _S217->view_proj_0.data_0[int(3)][int(0)], _S217->view_proj_0.data_0[int(0)][int(1)], _S217->view_proj_0.data_0[int(1)][int(1)], _S217->view_proj_0.data_0[int(2)][int(1)], _S217->view_proj_0.data_0[int(3)][int(1)], _S217->view_proj_0.data_0[int(0)][int(2)], _S217->view_proj_0.data_0[int(1)][int(2)], _S217->view_proj_0.data_0[int(2)][int(2)], _S217->view_proj_0.data_0[int(3)][int(2)], _S217->view_proj_0.data_0[int(0)][int(3)], _S217->view_proj_0.data_0[int(1)][int(3)], _S217->view_proj_0.data_0[int(2)][int(3)], _S217->view_proj_0.data_0[int(3)][int(3)]))));
    (&o_3)->draw_0 = kernelContext_36->ed_c_0->base_0 + iid_3;
    (&o_3)->lod_1 = (((uint4(d_17.slots_0) ).z) >> 8U) & 3U;
    return o_3;
}

float t_own_h_2(uint pool_18, const TerrainDraw_natural_0 thread* d_18, uint i_17, uint j_13, KernelContext_0 thread* kernelContext_37)
{
    float4 _S221 = float4(d_18->hq_0) ;
    float _S222 = t_page_0(pool_18, (uint4(d_18->slots_0) ).x, i_17 * 4U, j_13 * 4U, _S221.x, _S221.y, kernelContext_37);
    return _S222;
}

struct pixelOutput_0
{
    uint4 output_0 [[color(0)]];
};

struct pixelInput_0
{
    [[flat]] uint draw_1 [[user(DRAW)]];
    [[flat]] uint lod_2 [[user(LOD)]];
};

[[fragment]] pixelOutput_0 fs_terrain(pixelInput_0 _S223 [[stage_in]], uint prim_0 [[primitive_id]], float4 pos_2 [[position]], ulong device* ed_buffers_1 [[buffer(2)]], TerrainC_0 constant* ed_c_1 [[buffer(0)]])
{
    thread KernelContext_0 kernelContext_38;
    (&kernelContext_38)->ed_buffers_0 = ed_buffers_1;
    (&kernelContext_38)->ed_c_0 = ed_c_1;
    pixelOutput_0 _S224 = { uint4(2147483648U | (_S223.draw_1), (prim_0 | ((_S223.lod_2) << 24U)) & 268435455U, 0U, 0U) };
    return _S224;
}

uint3 t_triangle_0(uint prim_1)
{
    uint lod_3 = (prim_1 >> 24U) & 3U;
    uint _S225 = prim_1 & 16777215U;
    uint st_0 = 1U << lod_3;
    uint c_4 = 64U >> lod_3;
    uint _S226 = 2U * c_4 * c_4;
    uint3 _S227;
    if(_S225 < _S226)
    {
        uint q_1 = _S225 >> 1U;
        uint _S228 = q_1 % c_4;
        uint i_18 = _S228 * st_0;
        uint _S229 = q_1 / c_4;
        uint v00_0 = _S229 * st_0 * 65U + i_18;
        uint v10_0 = v00_0 + st_0;
        uint v01_0 = v00_0 + st_0 * 65U;
        uint v11_0 = v01_0 + st_0;
        if((_S225 & 1U) == 0U)
        {
            _S227 = uint3(v00_0, v10_0, v11_0);
        }
        else
        {
            _S227 = uint3(v00_0, v11_0, v01_0);
        }
        return _S227;
    }
    uint k_1 = _S225 - _S226;
    uint s_3 = k_1 >> 1U;
    uint s1_0 = (s_3 + 1U) % (4U * c_4);
    uint _S230 = s_3 * st_0;
    uint2 p_5 = t_ring_0(_S230);
    uint _S231 = s1_0 * st_0;
    uint2 q_2 = t_ring_0(_S231);
    uint tp_0 = p_5.y * 65U + p_5.x;
    uint tq_0 = q_2.y * 65U + q_2.x;
    uint a_7 = 4225U + _S230;
    uint b_7 = 4225U + _S231;
    if((k_1 & 1U) == 0U)
    {
        _S227 = uint3(a_7, b_7, tq_0);
    }
    else
    {
        _S227 = uint3(a_7, tq_0, tp_0);
    }
    return _S227;
}

float2 t_vertex_grid_0(uint v_1)
{
    if(v_1 < 4225U)
    {
        return float2(float(v_1 % 65U), float(v_1 / 65U));
    }
    return float2(t_ring_0(v_1 - 4225U));
}

float2 shell_wrel_0(FrameData_natural_0 device* f_0, const TerrainDraw_natural_0 thread* d_19, float2 g_0)
{
    float2 c_5 = (float4(d_19->tex_0) ).xy - (float4((&f_0->shell_0)->c_0) ).xy;
    float2 _S232 = float2(4096.0f) ;
    float4 _S233 = float4(d_19->shape_0) ;
    return c_5 - _S232 * round(c_5 / _S232) + (g_0 - float2(32.0f) ) * float2((_S233.x * _S233.y)) ;
}

uint grass_hash_0(uint x_0)
{
    uint s_4 = x_0 * 747796405U + 2891336453U;
    uint w_1 = ((s_4 >> ((s_4 >> 28U) + 4U)) ^ s_4) * 277803737U;
    return (w_1 >> 22U) ^ w_1;
}

float grass_rand_0(uint h_3)
{
    return float(h_3 >> 8U) * 5.9604644775390625e-08f;
}

uint shell_map_0(FrameData_natural_0 device* f_1, uint buf_0, uint coarse_0, int2 cell_0, KernelContext_0 thread* kernelContext_39)
{
    int n_0 = int((float4((&f_1->shell_0)->cells_0) ).z);
    bool _S234;
    if(any(cell_0 < (int2((- n_0 / int(2))) )))
    {
        _S234 = true;
    }
    else
    {
        _S234 = any(cell_0 >= (int2((n_0 / int(2))) ));
    }
    if(_S234)
    {
        return 0U;
    }
    int2 off_3;
    if(coarse_0 != 0U)
    {
        off_3 = (int4((&f_1->shell_0)->off_0) ).zw;
    }
    else
    {
        off_3 = (int4((&f_1->shell_0)->off_0) ).xy;
    }
    uint2 _S235 = uint2((cell_0 + off_3) & (int2((n_0 - int(1))) ));
    uint base_1 = (uint4((&f_1->shell_0)->cfg_5) ).z + coarse_0 * uint(n_0 * n_0);
    uint device* _S236 = ed_buf_0(buf_0, kernelContext_39);
    return *(_S236 + (base_1 + _S235.y * uint(n_0) + _S235.x));
}

uint shell_map_1(FrameData_natural_0 device* f_2, uint buf_1, uint coarse_1, int2 cell_1, KernelContext_0 thread* kernelContext_40)
{
    int n_1 = int((float4((&f_2->shell_0)->cells_0) ).z);
    bool _S237;
    if(any(cell_1 < (int2((- n_1 / int(2))) )))
    {
        _S237 = true;
    }
    else
    {
        _S237 = any(cell_1 >= (int2((n_1 / int(2))) ));
    }
    if(_S237)
    {
        return 0U;
    }
    int2 off_4;
    if(coarse_1 != 0U)
    {
        off_4 = (int4((&f_2->shell_0)->off_0) ).zw;
    }
    else
    {
        off_4 = (int4((&f_2->shell_0)->off_0) ).xy;
    }
    uint2 _S238 = uint2((cell_1 + off_4) & (int2((n_1 - int(1))) ));
    uint base_2 = (uint4((&f_2->shell_0)->cfg_5) ).z + coarse_1 * uint(n_1 * n_1);
    uint device* _S239 = ed_buf_0(buf_1, kernelContext_40);
    return *(_S239 + (base_2 + _S238.y * uint(n_1) + _S238.x));
}

struct GrassType_0
{
    float4 a_4;
    float4 b_4;
    float4 root_0;
    float4 mid_0;
    float4 tip_0;
    float4 dry_0;
    float4 shell_1;
    float4 carpet_0;
};

GrassType_0 shell_type_0(FrameData_natural_0 device* f_3, uint buf_2, uint kind_0, KernelContext_0 thread* kernelContext_41)
{
    GrassType_natural_0 device* _S240 = ed_buf_2(buf_2, kernelContext_41);
    GrassType_natural_0 device* _S241 = _S240 + ((uint4((&f_3->shell_0)->cfg_5) ).y + kind_0);
    GrassType_0 _S242 = { float4((*_S241).a_4) , float4((*_S241).b_4) , float4((*_S241).root_0) , float4((*_S241).mid_0) , float4((*_S241).tip_0) , float4((*_S241).dry_0) , float4((*_S241).shell_1) , float4((*_S241).carpet_0)  };
    return _S242;
}

GrassType_0 shell_type_1(FrameData_natural_0 device* f_4, uint buf_3, uint kind_1, KernelContext_0 thread* kernelContext_42)
{
    GrassType_natural_0 device* _S243 = ed_buf_2(buf_3, kernelContext_42);
    GrassType_natural_0 device* _S244 = _S243 + ((uint4((&f_4->shell_0)->cfg_5) ).y + kind_1);
    GrassType_0 _S245 = { float4((*_S244).a_4) , float4((*_S244).b_4) , float4((*_S244).root_0) , float4((*_S244).mid_0) , float4((*_S244).tip_0) , float4((*_S244).dry_0) , float4((*_S244).shell_1) , float4((*_S244).carpet_0)  };
    return _S245;
}

uint shell_lod_0(FrameData_natural_0 device* f_5, float sm_0, float px_m_0, float dith_0)
{
    return uint(clamp(floor(log2(max(px_m_0 * 2.0f / sm_0, 1.0f)) + dith_0 - 0.5f + 0.5f), 0.0f, 7.0f));
}

uint shell_hash2_0(int2 q_3, uint salt_0)
{
    return grass_hash_0((uint(q_3.x) * 2654435761U) ^ (grass_hash_0(uint(q_3.y) ^ salt_0)));
}

struct ShellHit_0
{
    bool hit_0;
    float tt_0;
    float rnd_0;
    uint kind_2;
    float dry_1;
    float2 side_0;
};

ShellHit_0 shell_strand_0(FrameData_natural_0 device* f_6, uint buf_4, float2 wrel_0, uint k_2, uint n_2, uint L_0, float dz_1, bool force_0, KernelContext_0 thread* kernelContext_43)
{
    thread ShellHit_0 o_4;
    (&o_4)->hit_0 = false;
    (&o_4)->tt_0 = 0.0f;
    (&o_4)->rnd_0 = 0.0f;
    (&o_4)->kind_2 = 0U;
    (&o_4)->dry_1 = 0.0f;
    (&o_4)->side_0 = float2(0.0f) ;
    uint jh_0 = shell_hash2_0(int2(floor(wrel_0 * float2(8.0f) )), 81U);
    float2 _S246 = float2(0.5f) ;
    float2 _S247 = float2((float4((&f_6->shell_0)->cells_0) ).x) ;
    uint _S248 = shell_map_1(f_6, buf_4, 0U, int2(floor((wrel_0 + (float2(grass_rand_0(jh_0), grass_rand_0(grass_hash_0(jh_0 + 7U))) - _S246) * _S247) / _S247)), kernelContext_43);
    uint _S249 = _S248 & 255U;
    if(_S249 == 0U)
    {
        return o_4;
    }
    uint kind_3 = _S249 - 1U;
    float dens_0 = float((_S248 >> 8U) & 255U) / 255.0f;
    float hsc_0 = float((_S248 >> 16U) & 255U) / 255.0f * 2.0f;
    float dry_2 = float((_S248 >> 24U) & 255U) / 255.0f;
    GrassType_0 _S250 = shell_type_1(f_6, buf_4, kind_3, kernelContext_43);
    float hk_0 = (float4((&f_6->shell_0)->fu_1) ).w * float(k_2 + 1U) / float(n_2);
    float H_0 = _S250.shell_1.x * hsc_0;
    (&o_4)->kind_2 = kind_3;
    float2 _S251 = float2((_S250.shell_1.y * float(1U << L_0))) ;
    float2 q_4 = wrel_0 / _S251;
    float2 lq_0 = fract(q_4);
    uint h_4 = shell_hash2_0(int2(floor(q_4)) + int2(round((float4((&f_6->shell_0)->c_0) ).xy / _S251)), 163U + L_0);
    float r0_0 = grass_rand_0(h_4);
    float r1_0 = grass_rand_0(grass_hash_0(h_4 + 1U));
    float r2_0 = grass_rand_0(grass_hash_0(h_4 + 2U));
    float r3_0 = grass_rand_0(grass_hash_0(h_4 + 3U));
    (&o_4)->rnd_0 = r1_0;
    float _S252;
    if(r3_0 < dry_2)
    {
        _S252 = 1.0f;
    }
    else
    {
        _S252 = 0.0f;
    }
    (&o_4)->dry_1 = _S252;
    float fade_0 = smoothstep((float4((&f_6->shell_0)->c_0) ).z * 0.72000002861022949f, (float4((&f_6->shell_0)->c_0) ).z, dz_1);
    bool _S253 = !force_0;
    bool _S254;
    if(_S253)
    {
        _S254 = (grass_rand_0(grass_hash_0(h_4 + 5U))) < fade_0;
    }
    else
    {
        _S254 = false;
    }
    if(_S254)
    {
        return o_4;
    }
    if(k_2 == 0U)
    {
        (&o_4)->tt_0 = 0.079999998211860657f;
        (&o_4)->side_0 = lq_0 - _S246;
        if(force_0)
        {
            _S254 = true;
        }
        else
        {
            _S254 = r2_0 < (_S250.shell_1.z * dens_0 * 1.1499999761581421f + 0.05000000074505806f);
        }
        (&o_4)->hit_0 = _S254;
        return o_4;
    }
    if(_S253)
    {
        _S254 = r0_0 >= (_S250.shell_1.z * dens_0);
    }
    else
    {
        _S254 = false;
    }
    if(_S254)
    {
        return o_4;
    }
    float tt_1 = hk_0 / max(H_0 * mix(0.5f, 1.0f, r1_0), 9.9999997473787516e-05f);
    if(_S253)
    {
        _S254 = tt_1 > 1.0f;
    }
    else
    {
        _S254 = false;
    }
    if(_S254)
    {
        return o_4;
    }
    float tt_2 = saturate(tt_1);
    float a_8 = r2_0 * 6.2831854820251465f;
    float rad_0 = 0.41999998688697815f * (1.0f - pow(tt_2, 1.6000000238418579f)) + 0.039999999105930328f;
    float2 dl_0 = lq_0 - (float2(0.5f)  + (float2(grass_rand_0(grass_hash_0(h_4 + 9U)), grass_rand_0(grass_hash_0(h_4 + 11U))) - _S246) * _S246 + float2(cos(a_8), sin(a_8)) * float2((0.20000000298023224f * tt_2 * tt_2)) );
    (&o_4)->side_0 = dl_0 / float2(rad_0) ;
    (&o_4)->tt_0 = tt_2;
    if(force_0)
    {
        _S254 = true;
    }
    else
    {
        _S254 = (dot(dl_0, dl_0)) < (rad_0 * rad_0);
    }
    (&o_4)->hit_0 = _S254;
    return o_4;
}

struct pixelOutput_1
{
    uint4 output_1 [[color(0)]];
};

struct pixelInput_1
{
    float2 wrel_1 [[user(WREL)]];
    float dz_2 [[user(DZ)]];
    [[flat]] uint draw_2 [[user(DRAW)]];
    [[flat]] uint lk_0 [[user(LK)]];
};

[[fragment]] pixelOutput_1 fs_shell(pixelInput_1 _S255 [[stage_in]], uint prim_2 [[primitive_id]], float4 pos_3 [[position]], ulong device* ed_buffers_2 [[buffer(2)]], TerrainC_0 constant* ed_c_2 [[buffer(0)]])
{
    thread KernelContext_0 kernelContext_44;
    (&kernelContext_44)->ed_buffers_0 = ed_buffers_2;
    (&kernelContext_44)->ed_c_0 = ed_c_2;
    FrameData_natural_0 device* _S256 = ed_buf_1(ed_c_2->frame_0, &kernelContext_44);
    uint k_3 = ((_S255.lk_0) >> 8U) & 255U;
    uint n_3 = (_S255.lk_0) >> 16U;
    uint lod_4 = (_S255.lk_0) & 3U;
    float px_1 = sqrt(length(dfdx(_S255.wrel_1)) * length(dfdy(_S255.wrel_1)));
    uint2 _S257 = uint2(pos_3.xy);
    float dith_1 = grass_rand_0(grass_hash_0(_S257.x * 1973U + _S257.y * 9277U));
    uint _S258 = shell_map_0(_S256, (&kernelContext_44)->ed_c_0->draws_0, 0U, int2(floor(_S255.wrel_1 / float2((float4((&_S256->shell_0)->cells_0) ).x) )), &kernelContext_44);
    uint _S259 = _S258 & 255U;
    if(_S259 == 0U)
    {
        discard_fragment();
    }
    GrassType_0 _S260 = shell_type_0(_S256, (&kernelContext_44)->ed_c_0->draws_0, _S259 - 1U, &kernelContext_44);
    uint _S261 = shell_lod_0(_S256, _S260.shell_1.y, px_1, dith_1);
    ShellHit_0 _S262 = shell_strand_0(_S256, (&kernelContext_44)->ed_c_0->draws_0, _S255.wrel_1, k_3, n_3, _S261, _S255.dz_2, false, &kernelContext_44);
    if(!_S262.hit_0)
    {
        discard_fragment();
    }
    pixelOutput_1 _S263 = { uint4((2147483648U | (_S261 << 24U)) | (_S255.draw_2), ((prim_2 | (lod_4 << 24U)) | (k_3 << 27U)) | 2147483648U, 0U, 0U) };
    return _S263;
}

struct vs_terrain_Result_0
{
    float4 pos_4 [[position]];
    uint draw_3 [[user(DRAW)]];
    uint lod_5 [[user(LOD)]];
};

[[vertex]] vs_terrain_Result_0 vs_terrain(uint vid_4 [[vertex_id]], uint iid_4 [[instance_id]], ulong device* ed_buffers_3 [[buffer(2)]], TerrainC_0 constant* ed_c_3 [[buffer(0)]])
{
    thread KernelContext_0 kernelContext_45;
    (&kernelContext_45)->ed_buffers_0 = ed_buffers_3;
    (&kernelContext_45)->ed_c_0 = ed_c_3;
    TerrainOut_0 _S264 = terrain_vs_1(vid_4, iid_4, &kernelContext_45);
    thread vs_terrain_Result_0 _S265;
    (&_S265)->pos_4 = _S264.pos_1;
    (&_S265)->draw_3 = _S264.draw_0;
    (&_S265)->lod_5 = _S264.lod_1;
    return _S265;
}

struct vs_terrain_trees_Result_0
{
    float4 pos_5 [[position]];
    uint draw_4 [[user(DRAW)]];
    uint lod_6 [[user(LOD)]];
};

[[vertex]] vs_terrain_trees_Result_0 vs_terrain_trees(uint vid_5 [[vertex_id]], uint iid_5 [[instance_id]], ulong device* ed_buffers_4 [[buffer(2)]], TerrainC_0 constant* ed_c_4 [[buffer(0)]])
{
    thread KernelContext_0 kernelContext_46;
    (&kernelContext_46)->ed_buffers_0 = ed_buffers_4;
    (&kernelContext_46)->ed_c_0 = ed_c_4;
    TerrainOut_0 _S266 = terrain_vs_0(vid_5, iid_5, &kernelContext_46);
    thread vs_terrain_trees_Result_0 _S267;
    (&_S267)->pos_5 = _S266.pos_1;
    (&_S267)->draw_4 = _S266.draw_0;
    (&_S267)->lod_6 = _S266.lod_1;
    return _S267;
}

struct vs_terrain_shadow_Result_0
{
    float4 pos_6 [[position]];
};

[[vertex]] vs_terrain_shadow_Result_0 vs_terrain_shadow(uint vid_6 [[vertex_id]], uint iid_6 [[instance_id]], ulong device* ed_buffers_5 [[buffer(2)]], TerrainC_0 constant* ed_c_5 [[buffer(0)]])
{
    thread KernelContext_0 kernelContext_47;
    (&kernelContext_47)->ed_buffers_0 = ed_buffers_5;
    (&kernelContext_47)->ed_c_0 = ed_c_5;
    TerrainShadowOut_0 _S268 = terrain_shadow_vs_1(vid_6, iid_6, &kernelContext_47);
    thread vs_terrain_shadow_Result_0 _S269;
    (&_S269)->pos_6 = _S268.pos_0;
    return _S269;
}

struct vs_terrain_shadow_trees_Result_0
{
    float4 pos_7 [[position]];
};

[[vertex]] vs_terrain_shadow_trees_Result_0 vs_terrain_shadow_trees(uint vid_7 [[vertex_id]], uint iid_7 [[instance_id]], ulong device* ed_buffers_6 [[buffer(2)]], TerrainC_0 constant* ed_c_6 [[buffer(0)]])
{
    thread KernelContext_0 kernelContext_48;
    (&kernelContext_48)->ed_buffers_0 = ed_buffers_6;
    (&kernelContext_48)->ed_c_0 = ed_c_6;
    TerrainShadowOut_0 _S270 = terrain_shadow_vs_0(vid_7, iid_7, &kernelContext_48);
    thread vs_terrain_shadow_trees_Result_0 _S271;
    (&_S271)->pos_7 = _S270.pos_0;
    return _S271;
}

struct vs_shell_Result_0
{
    float4 pos_8 [[position]];
    float2 wrel_2 [[user(WREL)]];
    float dz_3 [[user(DZ)]];
    uint draw_5 [[user(DRAW)]];
    uint lk_1 [[user(LK)]];
};

struct ShellOut_0
{
    float4 pos_9;
    float2 wrel_3;
    float dz_4;
    [[flat]] uint draw_6;
    [[flat]] uint lk_2;
};

[[vertex]] vs_shell_Result_0 vs_shell(uint vid_8 [[vertex_id]], uint iid_8 [[instance_id]], ulong device* ed_buffers_7 [[buffer(2)]], TerrainC_0 constant* ed_c_7 [[buffer(0)]])
{
    ShellOut_0 _S272;
    thread KernelContext_0 kernelContext_49;
    (&kernelContext_49)->ed_buffers_0 = ed_buffers_7;
    (&kernelContext_49)->ed_c_0 = ed_c_7;
    for(;;)
    {
        FrameData_natural_0 device* _S273 = ed_buf_1((&kernelContext_49)->ed_c_0->frame_0, &kernelContext_49);
        TerrainDraw_natural_0 device* _S274 = ed_buf_3((&kernelContext_49)->ed_c_0->draws_0, &kernelContext_49);
        TerrainDraw_natural_0 device* _S275 = _S274 + (&kernelContext_49)->ed_c_0->base_0;
        TerrainDraw_natural_0 d_20 = *_S275;
        thread TerrainDraw_natural_0 _S276 = *_S275;
        uint _S277 = max((uint4((&_S273->shell_0)->cfg_5) ).w, 1U);
        uint k_4 = _S277 - 1U - iid_8;
        uint lod_7 = (&kernelContext_49)->ed_c_0->cascade_0;
        uint3 tri_0 = t_triangle_0((vid_8 / 3U) | (((&kernelContext_49)->ed_c_0->cascade_0) << 24U));
        thread ShellOut_0 o_5;
        (&o_5)->pos_9 = float4(0.0f, 0.0f, -1.0f, 1.0f);
        (&o_5)->wrel_3 = float2(0.0f) ;
        (&o_5)->dz_4 = 0.0f;
        (&o_5)->draw_6 = (&kernelContext_49)->ed_c_0->base_0;
        (&o_5)->lk_2 = 0U;
        uint _S278 = tri_0.x;
        float2 g0_0 = t_vertex_grid_0(_S278);
        float3 _S279 = (float4((&_S276)->origin_0) ).xyz;
        uint _S280 = uint(g0_0.x);
        uint _S281 = uint(g0_0.y);
        uint _S282 = (&kernelContext_49)->ed_c_0->pool_0;
        _S276 = d_20;
        float _S283 = t_own_h_2(_S282, &_S276, _S280, _S281, &kernelContext_49);
        thread float3 dir_7;
        float3 _S284 = t_lattice_0(&_S276, _S280, _S281, _S283, &dir_7);
        float R_0 = (float4((&_S273->shell_0)->c_0) ).z;
        float4 _S285 = float4((&_S276)->shape_0) ;
        float dt_0 = length(_S279 + _S284) - 1.5f * float(1U << lod_7) * _S285.x * _S285.y;
        uint step_0;
        if(dt_0 < (0.30000001192092896f * R_0))
        {
            step_0 = 1U;
        }
        else
        {
            if(dt_0 < (0.60000002384185791f * R_0))
            {
                step_0 = 2U;
            }
            else
            {
                step_0 = 4U;
            }
        }
        bool _S286;
        if(dt_0 > R_0)
        {
            _S286 = true;
        }
        else
        {
            if(k_4 != 0U)
            {
                uint _S287 = (k_4 + 1U) % step_0;
                _S286 = _S287 != 0U;
            }
            else
            {
                _S286 = false;
            }
        }
        if(_S286)
        {
            _S272 = o_5;
            break;
        }
        uint _S288 = vid_8 % 3U;
        uint v_2;
        if(_S288 == 0U)
        {
            v_2 = _S278;
        }
        else
        {
            if(_S288 == 1U)
            {
                step_0 = tri_0.y;
            }
            else
            {
                step_0 = tri_0.z;
            }
            v_2 = step_0;
        }
        float3 _S289 = t_vertex_0((&kernelContext_49)->ed_c_0->pool_0, &_S276, v_2, &kernelContext_49);
        float4 _S290 = float4((&_S276)->dir_c_0) ;
        float4 _S291 = float4(_S289 + normalize(_S289 - (_S279 - _S290.xyz * float3(_S290.w) )) * float3(((float4((&_S273->shell_0)->fu_1) ).w * float(k_4 + 1U) / float(_S277))) , 1.0f);
        (&o_5)->pos_9 = (((_S291) * (matrix<float,int(4),int(4)> (_S273->view_proj_0.data_0[int(0)][int(0)], _S273->view_proj_0.data_0[int(1)][int(0)], _S273->view_proj_0.data_0[int(2)][int(0)], _S273->view_proj_0.data_0[int(3)][int(0)], _S273->view_proj_0.data_0[int(0)][int(1)], _S273->view_proj_0.data_0[int(1)][int(1)], _S273->view_proj_0.data_0[int(2)][int(1)], _S273->view_proj_0.data_0[int(3)][int(1)], _S273->view_proj_0.data_0[int(0)][int(2)], _S273->view_proj_0.data_0[int(1)][int(2)], _S273->view_proj_0.data_0[int(2)][int(2)], _S273->view_proj_0.data_0[int(3)][int(2)], _S273->view_proj_0.data_0[int(0)][int(3)], _S273->view_proj_0.data_0[int(1)][int(3)], _S273->view_proj_0.data_0[int(2)][int(3)], _S273->view_proj_0.data_0[int(3)][int(3)]))));
        float2 _S292 = shell_wrel_0(_S273, &_S276, t_vertex_grid_0(v_2));
        (&o_5)->wrel_3 = _S292;
        (&o_5)->dz_4 = - (((_S291) * (matrix<float,int(4),int(4)> (_S273->view_0.data_0[int(0)][int(0)], _S273->view_0.data_0[int(1)][int(0)], _S273->view_0.data_0[int(2)][int(0)], _S273->view_0.data_0[int(3)][int(0)], _S273->view_0.data_0[int(0)][int(1)], _S273->view_0.data_0[int(1)][int(1)], _S273->view_0.data_0[int(2)][int(1)], _S273->view_0.data_0[int(3)][int(1)], _S273->view_0.data_0[int(0)][int(2)], _S273->view_0.data_0[int(1)][int(2)], _S273->view_0.data_0[int(2)][int(2)], _S273->view_0.data_0[int(3)][int(2)], _S273->view_0.data_0[int(0)][int(3)], _S273->view_0.data_0[int(1)][int(3)], _S273->view_0.data_0[int(2)][int(3)], _S273->view_0.data_0[int(3)][int(3)])))).z;
        (&o_5)->lk_2 = (lod_7 | (k_4 << 8U)) | (_S277 << 16U);
        _S272 = o_5;
        break;
    }
    thread vs_shell_Result_0 _S293;
    (&_S293)->pos_8 = _S272.pos_9;
    (&_S293)->wrel_2 = _S272.wrel_3;
    (&_S293)->dz_3 = _S272.dz_4;
    (&_S293)->draw_5 = _S272.draw_6;
    (&_S293)->lk_1 = _S272.lk_2;
    return _S293;
}

