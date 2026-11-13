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

float t_lift_k_0(const TerrainDraw_natural_0 thread* d_2, float3 p_0)
{
    float _S16 = (float4(d_2->polar_0) ).w;
    if(_S16 <= 0.0f)
    {
        return 0.0f;
    }
    return smoothstep(12.0f, 6.0f, _S16 / max(length((float4(d_2->origin_0) ).xyz + p_0), 1.0f));
}

uint t_canopy_raw_0(uint pool_3, uint off_0, uint i_4, uint j_3, KernelContext_0 thread* kernelContext_3)
{
    uint device* _S17 = ed_buf_0(pool_3, kernelContext_3);
    return *(_S17 + (off_0 + j_3 * 257U + i_4));
}

float t_canopy_h_0(uint code_0)
{
    float _S18;
    if(code_0 == 0U)
    {
        _S18 = 0.0f;
    }
    else
    {
        _S18 = 0.5f * exp(float(code_0 - 1U) * 0.020444700494408607f);
    }
    return _S18;
}

float t_canopy_lift_0(uint pool_4, uint off_1, uint i_5, uint j_4, KernelContext_0 thread* kernelContext_4)
{
    if(off_1 == 0U)
    {
        return 0.0f;
    }
    uint _S19 = t_canopy_raw_0(pool_4, off_1, i_5, j_4, kernelContext_4);
    return t_canopy_h_0((_S19 >> 8U) & 255U) * smoothstep(0.059999998658895493f, 0.44999998807907104f, float(_S19 & 255U) / 255.0f);
}

float3 t_target_even_0(uint pool_5, const TerrainDraw_natural_0 thread* d_3, uint i_6, uint j_5, KernelContext_0 thread* kernelContext_5)
{
    bool border_0;
    if(i_6 == 0U)
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
        border_0 = i_6 == 64U;
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
    uint4 _S20 = uint4(d_3->slots_0) ;
    uint _S21 = _S20.z;
    uint qx_0;
    if((_S21 & 16U) != 0U)
    {
        qx_0 = 128U;
    }
    else
    {
        qx_0 = 0U;
    }
    uint qy_0;
    if((_S21 & 32U) != 0U)
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
        float _S22 = t_own_h_0(pool_5, d_3, i_6, j_5, kernelContext_5);
        h_1 = _S22;
    }
    else
    {
        float4 _S23 = float4(d_3->hq_0) ;
        float _S24 = t_page_0(pool_5, _S20.y, qx_0 + 2U * i_6, qy_0 + 2U * j_5, _S23.z, _S23.w, kernelContext_5);
        h_1 = _S24;
    }
    thread float3 dir_1;
    float3 _S25 = t_lattice_0(d_3, i_6, j_5, h_1, &dir_1);
    float3 p_1;
    if(((float4(d_3->polar_0) ).w) > 0.0f)
    {
        float _S26 = t_lift_k_0(d_3, _S25);
        float4 _S27 = float4(d_3->tex_0) ;
        uint poff_0 = (as_type<uint>((_S27.w)));
        float lift_0;
        if(_S26 > 0.0f)
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
                float _S28 = t_canopy_lift_0(pool_5, (as_type<uint>((_S27.z))), i_6 * 4U, j_5 * 4U, kernelContext_5);
                h_1 = _S28;
            }
            else
            {
                float _S29 = t_canopy_lift_0(pool_5, poff_0, qx_0 + 2U * i_6, qy_0 + 2U * j_5, kernelContext_5);
                h_1 = _S29;
            }
            lift_0 = h_1;
        }
        else
        {
            lift_0 = 0.0f;
        }
        p_1 = _S25 + dir_1 * float3((_S26 * lift_0)) ;
    }
    else
    {
        p_1 = _S25;
    }
    return p_1;
}

float t_own_lift_0(uint pool_6, const TerrainDraw_natural_0 thread* d_4, uint i_7, uint j_6, float3 p_2, KernelContext_0 thread* kernelContext_6)
{
    if(((float4(d_4->polar_0) ).w) <= 0.0f)
    {
        return 0.0f;
    }
    float _S30 = t_lift_k_0(d_4, p_2);
    float _S31;
    if(_S30 > 0.0f)
    {
        float _S32 = t_canopy_lift_0(pool_6, (as_type<uint>(((float4(d_4->tex_0) ).z))), i_7 * 4U, j_6 * 4U, kernelContext_6);
        _S31 = _S30 * _S32;
    }
    else
    {
        _S31 = 0.0f;
    }
    return _S31;
}

float3 t_base_0(uint pool_7, const TerrainDraw_natural_0 thread* d_5, uint i_8, uint j_7, KernelContext_0 thread* kernelContext_7)
{
    float _S33 = t_own_h_0(pool_7, d_5, i_8, j_7, kernelContext_7);
    thread float3 dir_2;
    float3 _S34 = t_lattice_0(d_5, i_8, j_7, _S33, &dir_2);
    float3 p_3;
    if(((float4(d_5->polar_0) ).w) > 0.0f)
    {
        float _S35 = t_own_lift_0(pool_7, d_5, i_8, j_7, _S34, kernelContext_7);
        p_3 = _S34 + dir_2 * float3(_S35) ;
    }
    else
    {
        p_3 = _S34;
    }
    return p_3;
}

float t_morph_0(float3 rel_0, float2 band_0)
{
    float _S36 = band_0.x;
    return saturate((length(rel_0) - _S36) / max(band_0.y - _S36, 0.0010000000474974513f));
}

float3 t_coarse_even_0(uint pool_8, const TerrainDraw_natural_0 thread* d_6, uint e_1, uint2 a_0, uint2 b_0, uint2 c_0, KernelContext_0 thread* kernelContext_8)
{
    float3 _S37 = t_base_0(pool_8, d_6, a_0.x, a_0.y, kernelContext_8);
    if((e_1 & 3U) == 0U)
    {
        return _S37;
    }
    float m_0 = t_morph_0((float4(d_6->origin_0) ).xyz + _S37, (float4(d_6->morph_0) ).zw);
    float3 _S38 = t_base_0(pool_8, d_6, b_0.x, b_0.y, kernelContext_8);
    float3 _S39 = t_base_0(pool_8, d_6, c_0.x, c_0.y, kernelContext_8);
    return mix(_S37, float3(0.5f)  * (_S38 + _S39), float3(m_0) );
}

float3 t_target_0(uint pool_9, const TerrainDraw_natural_0 thread* d_7, uint i_9, uint j_8, KernelContext_0 thread* kernelContext_9)
{
    bool ei_0 = (i_9 & 1U) == 0U;
    bool ej_0 = (j_8 & 1U) == 0U;
    bool _S40;
    if(ei_0)
    {
        _S40 = ej_0;
    }
    else
    {
        _S40 = false;
    }
    if(_S40)
    {
        float3 _S41 = t_target_even_0(pool_9, d_7, i_9, j_8, kernelContext_9);
        return _S41;
    }
    if(!ei_0)
    {
        _S40 = ej_0;
    }
    else
    {
        _S40 = false;
    }
    if(_S40)
    {
        float3 _S42 = t_target_even_0(pool_9, d_7, i_9 - 1U, j_8, kernelContext_9);
        float3 _S43 = t_target_even_0(pool_9, d_7, i_9 + 1U, j_8, kernelContext_9);
        return float3(0.5f)  * (_S42 + _S43);
    }
    if(ei_0)
    {
        _S40 = !ej_0;
    }
    else
    {
        _S40 = false;
    }
    if(_S40)
    {
        float3 _S44 = t_target_even_0(pool_9, d_7, i_9, j_8 - 1U, kernelContext_9);
        float3 _S45 = t_target_even_0(pool_9, d_7, i_9, j_8 + 1U, kernelContext_9);
        return float3(0.5f)  * (_S44 + _S45);
    }
    float3 _S46 = t_target_even_0(pool_9, d_7, i_9 - 1U, j_8 - 1U, kernelContext_9);
    float3 _S47 = t_target_even_0(pool_9, d_7, i_9 + 1U, j_8 + 1U, kernelContext_9);
    return float3(0.5f)  * (_S46 + _S47);
}

float3 t_target_even_1(uint pool_10, const TerrainDraw_natural_0 thread* d_8, uint i_10, uint j_9, KernelContext_0 thread* kernelContext_10)
{
    bool border_1;
    if(i_10 == 0U)
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
        border_1 = i_10 == 64U;
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
    uint4 _S48 = uint4(d_8->slots_0) ;
    uint _S49 = _S48.z;
    uint qx_1;
    if((_S49 & 16U) != 0U)
    {
        qx_1 = 128U;
    }
    else
    {
        qx_1 = 0U;
    }
    uint qy_1;
    if((_S49 & 32U) != 0U)
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
        float _S50 = t_own_h_0(pool_10, d_8, i_10, j_9, kernelContext_10);
        h_2 = _S50;
    }
    else
    {
        float4 _S51 = float4(d_8->hq_0) ;
        float _S52 = t_page_0(pool_10, _S48.y, qx_1 + 2U * i_10, qy_1 + 2U * j_9, _S51.z, _S51.w, kernelContext_10);
        h_2 = _S52;
    }
    thread float3 dir_3;
    float3 _S53 = t_lattice_0(d_8, i_10, j_9, h_2, &dir_3);
    return _S53;
}

float3 t_base_1(uint pool_11, const TerrainDraw_natural_0 thread* d_9, uint i_11, uint j_10, KernelContext_0 thread* kernelContext_11)
{
    float _S54 = t_own_h_0(pool_11, d_9, i_11, j_10, kernelContext_11);
    thread float3 dir_4;
    float3 _S55 = t_lattice_0(d_9, i_11, j_10, _S54, &dir_4);
    return _S55;
}

float3 t_coarse_even_1(uint pool_12, const TerrainDraw_natural_0 thread* d_10, uint e_2, uint2 a_1, uint2 b_1, uint2 c_1, KernelContext_0 thread* kernelContext_12)
{
    float3 _S56 = t_base_1(pool_12, d_10, a_1.x, a_1.y, kernelContext_12);
    if((e_2 & 3U) == 0U)
    {
        return _S56;
    }
    float m_1 = t_morph_0((float4(d_10->origin_0) ).xyz + _S56, (float4(d_10->morph_0) ).zw);
    float3 _S57 = t_base_1(pool_12, d_10, b_1.x, b_1.y, kernelContext_12);
    float3 _S58 = t_base_1(pool_12, d_10, c_1.x, c_1.y, kernelContext_12);
    return mix(_S56, float3(0.5f)  * (_S57 + _S58), float3(m_1) );
}

float3 t_target_1(uint pool_13, const TerrainDraw_natural_0 thread* d_11, uint i_12, uint j_11, KernelContext_0 thread* kernelContext_13)
{
    bool ei_1 = (i_12 & 1U) == 0U;
    bool ej_1 = (j_11 & 1U) == 0U;
    bool _S59;
    if(ei_1)
    {
        _S59 = ej_1;
    }
    else
    {
        _S59 = false;
    }
    if(_S59)
    {
        float3 _S60 = t_target_even_1(pool_13, d_11, i_12, j_11, kernelContext_13);
        return _S60;
    }
    if(!ei_1)
    {
        _S59 = ej_1;
    }
    else
    {
        _S59 = false;
    }
    if(_S59)
    {
        float3 _S61 = t_target_even_1(pool_13, d_11, i_12 - 1U, j_11, kernelContext_13);
        float3 _S62 = t_target_even_1(pool_13, d_11, i_12 + 1U, j_11, kernelContext_13);
        return float3(0.5f)  * (_S61 + _S62);
    }
    if(ei_1)
    {
        _S59 = !ej_1;
    }
    else
    {
        _S59 = false;
    }
    if(_S59)
    {
        float3 _S63 = t_target_even_1(pool_13, d_11, i_12, j_11 - 1U, kernelContext_13);
        float3 _S64 = t_target_even_1(pool_13, d_11, i_12, j_11 + 1U, kernelContext_13);
        return float3(0.5f)  * (_S63 + _S64);
    }
    float3 _S65 = t_target_even_1(pool_13, d_11, i_12 - 1U, j_11 - 1U, kernelContext_13);
    float3 _S66 = t_target_even_1(pool_13, d_11, i_12 + 1U, j_11 + 1U, kernelContext_13);
    return float3(0.5f)  * (_S65 + _S66);
}

struct TerrainShadowOut_0
{
    float4 pos_0;
};

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

FrameData_natural_0 device* ed_buf_1(uint i_13, KernelContext_0 thread* kernelContext_14)
{
    return (FrameData_natural_0 device*)kernelContext_14->ed_buffers_0[i_13];
}

TerrainDraw_natural_0 device* ed_buf_2(uint i_14, KernelContext_0 thread* kernelContext_15)
{
    return (TerrainDraw_natural_0 device*)kernelContext_15->ed_buffers_0[i_14];
}

uint2 t_edge_at_0(uint edge_0, uint e_3)
{
    switch(edge_0)
    {
    case 0U:
        {
            return uint2(0U, e_3);
        }
    case 1U:
        {
            return uint2(64U, e_3);
        }
    case 2U:
        {
            return uint2(e_3, 0U);
        }
    default:
        {
            return uint2(e_3, 64U);
        }
    }
}

float t_own_h_1(uint _S67, TerrainDraw_natural_0 device* _S68, uint _S69, uint _S70, KernelContext_0 thread* kernelContext_16)
{
    float4 _S71 = float4(_S68->hq_0) ;
    float _S72 = t_page_0(_S67, (uint4(_S68->slots_0) ).x, _S69 * 4U, _S70 * 4U, _S71.x, _S71.y, kernelContext_16);
    return _S72;
}

float3 t_base_2(uint _S73, TerrainDraw_natural_0 device* _S74, uint _S75, uint _S76, KernelContext_0 thread* kernelContext_17)
{
    TerrainDraw_natural_0 _S77 = *_S74;
    float _S78 = t_own_h_1(_S73, _S74, _S75, _S76, kernelContext_17);
    thread TerrainDraw_natural_0 _S79 = _S77;
    thread float3 dir_5;
    float3 _S80 = t_lattice_0(&_S79, _S75, _S76, _S78, &dir_5);
    float3 p_4;
    if(((float4((&_S79)->polar_0) ).w) > 0.0f)
    {
        float _S81 = t_own_lift_0(_S73, &_S79, _S75, _S76, _S80, kernelContext_17);
        p_4 = _S80 + dir_5 * float3(_S81) ;
    }
    else
    {
        p_4 = _S80;
    }
    return p_4;
}

float3 t_coarse_even_2(uint _S82, TerrainDraw_natural_0 device* _S83, uint _S84, uint2 _S85, uint2 _S86, uint2 _S87, KernelContext_0 thread* kernelContext_18)
{
    thread TerrainDraw_natural_0 _S88 = *_S83;
    float3 _S89 = t_base_2(_S82, _S83, _S85.x, _S85.y, kernelContext_18);
    if((_S84 & 3U) == 0U)
    {
        return _S89;
    }
    float m_2 = t_morph_0((float4((&_S88)->origin_0) ).xyz + _S89, (float4((&_S88)->morph_0) ).zw);
    float3 _S90 = t_base_0(_S82, &_S88, _S86.x, _S86.y, kernelContext_18);
    float3 _S91 = t_base_0(_S82, &_S88, _S87.x, _S87.y, kernelContext_18);
    return mix(_S89, float3(0.5f)  * (_S90 + _S91), float3(m_2) );
}

float3 t_coarse_0(uint _S92, TerrainDraw_natural_0 device* _S93, uint _S94, uint _S95, KernelContext_0 thread* kernelContext_19)
{
    thread TerrainDraw_natural_0 _S96 = *_S93;
    if((_S95 & 1U) == 0U)
    {
        float3 _S97 = t_coarse_even_2(_S92, _S93, _S95, t_edge_at_0(_S94, _S95), t_edge_at_0(_S94, _S95 - 2U), t_edge_at_0(_S94, _S95 + 2U), kernelContext_19);
        return _S97;
    }
    uint _S98 = _S95 - 1U;
    uint2 _S99 = t_edge_at_0(_S94, _S98);
    uint _S100 = _S95 + 1U;
    uint2 _S101 = t_edge_at_0(_S94, _S100);
    float3 _S102 = t_coarse_even_2(_S92, _S93, _S98, _S99, t_edge_at_0(_S94, _S95 - 3U), _S101, kernelContext_19);
    float3 _S103 = t_coarse_even_0(_S92, &_S96, _S100, _S101, _S99, t_edge_at_0(_S94, _S95 + 3U), kernelContext_19);
    return float3(0.5f)  * (_S102 + _S103);
}

float3 t_grid_vertex_0(uint _S104, TerrainDraw_natural_0 device* _S105, uint _S106, uint _S107, KernelContext_0 thread* kernelContext_20)
{
    thread TerrainDraw_natural_0 _S108 = *_S105;
    uint fl_0 = (uint4((&_S108)->slots_0) ).z;
    uint edge_1;
    uint e_4;
    if(_S106 == 0U)
    {
        edge_1 = 0U;
        e_4 = _S107;
    }
    else
    {
        if(_S106 == 64U)
        {
            edge_1 = 1U;
            e_4 = _S107;
        }
        else
        {
            if(_S107 == 0U)
            {
                edge_1 = 2U;
                e_4 = _S106;
            }
            else
            {
                if(_S107 == 64U)
                {
                    edge_1 = 3U;
                    e_4 = _S106;
                }
                else
                {
                    edge_1 = 4U;
                    e_4 = 0U;
                }
            }
        }
    }
    if(edge_1 < 4U)
    {
        uint E_0 = 1U << ((fl_0 >> (10U + 3U * edge_1)) & 7U);
        bool _S109;
        if((fl_0 & (1U << edge_1)) != 0U)
        {
            _S109 = E_0 <= 2U;
        }
        else
        {
            _S109 = false;
        }
        if(_S109)
        {
            float3 _S110 = t_coarse_0(_S104, _S105, edge_1, e_4, kernelContext_20);
            return _S110;
        }
        if(E_0 > 1U)
        {
            uint lo_0 = e_4 & (~(E_0 - 1U));
            if(lo_0 == e_4)
            {
                float3 _S111 = t_base_2(_S104, _S105, _S106, _S107, kernelContext_20);
                return _S111;
            }
            uint2 a_2 = t_edge_at_0(edge_1, lo_0);
            uint2 b_2 = t_edge_at_0(edge_1, lo_0 + E_0);
            float3 _S112 = t_base_2(_S104, _S105, a_2.x, a_2.y, kernelContext_20);
            float3 _S113 = t_base_0(_S104, &_S108, b_2.x, b_2.y, kernelContext_20);
            return mix(_S112, _S113, float3((float(e_4 - lo_0) / float(E_0))) );
        }
    }
    float3 _S114 = t_base_2(_S104, _S105, _S106, _S107, kernelContext_20);
    if((fl_0 & 4194304U) != 0U)
    {
        return _S114;
    }
    float m_3 = t_morph_0((float4((&_S108)->origin_0) ).xyz + _S114, (float4((&_S108)->morph_0) ).xy);
    if(m_3 <= 0.0f)
    {
        return _S114;
    }
    float3 _S115 = t_target_0(_S104, &_S108, _S106, _S107, kernelContext_20);
    return mix(_S114, _S115, float3(m_3) );
}

uint2 t_ring_0(uint s_2)
{
    uint e_5 = s_2 % 64U;
    switch(s_2 / 64U)
    {
    case 0U:
        {
            return uint2(e_5, 0U);
        }
    case 1U:
        {
            return uint2(64U, e_5);
        }
    case 2U:
        {
            return uint2(64U - e_5, 64U);
        }
    default:
        {
            return uint2(0U, 64U - e_5);
        }
    }
}

float3 t_vertex_0(uint _S116, TerrainDraw_natural_0 device* _S117, uint _S118, KernelContext_0 thread* kernelContext_21)
{
    thread TerrainDraw_natural_0 _S119 = *_S117;
    if(_S118 < 4225U)
    {
        float3 _S120 = (float4((&_S119)->origin_0) ).xyz;
        float3 _S121 = t_grid_vertex_0(_S116, _S117, _S118 % 65U, _S118 / 65U, kernelContext_21);
        return _S120 + _S121;
    }
    uint2 ij_0 = t_ring_0(_S118 - 4225U);
    uint _S122 = ij_0.x;
    uint _S123 = ij_0.y;
    float3 _S124 = t_grid_vertex_0(_S116, _S117, _S122, _S123, kernelContext_21);
    float depth_0;
    if((((uint4((&_S119)->slots_0) ).z) & 64U) != 0U)
    {
        depth_0 = (float4((&_S119)->origin_0) ).w;
    }
    else
    {
        depth_0 = 0.0f;
    }
    bool _S125;
    if(((float4((&_S119)->polar_0) ).w) > 0.0f)
    {
        _S125 = depth_0 > 0.0f;
    }
    else
    {
        _S125 = false;
    }
    if(_S125)
    {
        float _S126 = t_own_lift_0(_S116, &_S119, _S122, _S123, _S124, kernelContext_21);
        depth_0 = depth_0 + _S126;
    }
    float3 _S127 = (float4((&_S119)->origin_0) ).xyz;
    float3 _S128 = _S127 + _S124;
    float4 _S129 = float4((&_S119)->dir_c_0) ;
    return _S128 - normalize(_S128 - (_S127 - _S129.xyz * float3(_S129.w) )) * float3(depth_0) ;
}

TerrainShadowOut_0 terrain_shadow_vs_0(uint vid_0, uint iid_0, KernelContext_0 thread* kernelContext_22)
{
    FrameData_natural_0 device* _S130 = ed_buf_1(kernelContext_22->ed_c_0->frame_0, kernelContext_22);
    TerrainDraw_natural_0 device* _S131 = ed_buf_2(kernelContext_22->ed_c_0->draws_0, kernelContext_22);
    float3 _S132 = t_vertex_0(kernelContext_22->ed_c_0->pool_0, _S131 + (kernelContext_22->ed_c_0->base_0 + iid_0), vid_0, kernelContext_22);
    thread TerrainShadowOut_0 o_0;
    (&o_0)->pos_0 = (((float4(_S132, 1.0f)) * (matrix<float,int(4),int(4)> ((&(&_S130->light_0)->shadow_0)->data_2[kernelContext_22->ed_c_0->cascade_0].data_0[int(0)][int(0)], (&(&_S130->light_0)->shadow_0)->data_2[kernelContext_22->ed_c_0->cascade_0].data_0[int(1)][int(0)], (&(&_S130->light_0)->shadow_0)->data_2[kernelContext_22->ed_c_0->cascade_0].data_0[int(2)][int(0)], (&(&_S130->light_0)->shadow_0)->data_2[kernelContext_22->ed_c_0->cascade_0].data_0[int(3)][int(0)], (&(&_S130->light_0)->shadow_0)->data_2[kernelContext_22->ed_c_0->cascade_0].data_0[int(0)][int(1)], (&(&_S130->light_0)->shadow_0)->data_2[kernelContext_22->ed_c_0->cascade_0].data_0[int(1)][int(1)], (&(&_S130->light_0)->shadow_0)->data_2[kernelContext_22->ed_c_0->cascade_0].data_0[int(2)][int(1)], (&(&_S130->light_0)->shadow_0)->data_2[kernelContext_22->ed_c_0->cascade_0].data_0[int(3)][int(1)], (&(&_S130->light_0)->shadow_0)->data_2[kernelContext_22->ed_c_0->cascade_0].data_0[int(0)][int(2)], (&(&_S130->light_0)->shadow_0)->data_2[kernelContext_22->ed_c_0->cascade_0].data_0[int(1)][int(2)], (&(&_S130->light_0)->shadow_0)->data_2[kernelContext_22->ed_c_0->cascade_0].data_0[int(2)][int(2)], (&(&_S130->light_0)->shadow_0)->data_2[kernelContext_22->ed_c_0->cascade_0].data_0[int(3)][int(2)], (&(&_S130->light_0)->shadow_0)->data_2[kernelContext_22->ed_c_0->cascade_0].data_0[int(0)][int(3)], (&(&_S130->light_0)->shadow_0)->data_2[kernelContext_22->ed_c_0->cascade_0].data_0[int(1)][int(3)], (&(&_S130->light_0)->shadow_0)->data_2[kernelContext_22->ed_c_0->cascade_0].data_0[int(2)][int(3)], (&(&_S130->light_0)->shadow_0)->data_2[kernelContext_22->ed_c_0->cascade_0].data_0[int(3)][int(3)]))));
    return o_0;
}

float3 t_base_3(uint _S133, TerrainDraw_natural_0 device* _S134, uint _S135, uint _S136, KernelContext_0 thread* kernelContext_23)
{
    TerrainDraw_natural_0 _S137 = *_S134;
    float _S138 = t_own_h_1(_S133, _S134, _S135, _S136, kernelContext_23);
    thread TerrainDraw_natural_0 _S139 = _S137;
    thread float3 dir_6;
    float3 _S140 = t_lattice_0(&_S139, _S135, _S136, _S138, &dir_6);
    return _S140;
}

float3 t_coarse_even_3(uint _S141, TerrainDraw_natural_0 device* _S142, uint _S143, uint2 _S144, uint2 _S145, uint2 _S146, KernelContext_0 thread* kernelContext_24)
{
    thread TerrainDraw_natural_0 _S147 = *_S142;
    float3 _S148 = t_base_3(_S141, _S142, _S144.x, _S144.y, kernelContext_24);
    if((_S143 & 3U) == 0U)
    {
        return _S148;
    }
    float m_4 = t_morph_0((float4((&_S147)->origin_0) ).xyz + _S148, (float4((&_S147)->morph_0) ).zw);
    float3 _S149 = t_base_1(_S141, &_S147, _S145.x, _S145.y, kernelContext_24);
    float3 _S150 = t_base_1(_S141, &_S147, _S146.x, _S146.y, kernelContext_24);
    return mix(_S148, float3(0.5f)  * (_S149 + _S150), float3(m_4) );
}

float3 t_coarse_1(uint _S151, TerrainDraw_natural_0 device* _S152, uint _S153, uint _S154, KernelContext_0 thread* kernelContext_25)
{
    thread TerrainDraw_natural_0 _S155 = *_S152;
    if((_S154 & 1U) == 0U)
    {
        float3 _S156 = t_coarse_even_3(_S151, _S152, _S154, t_edge_at_0(_S153, _S154), t_edge_at_0(_S153, _S154 - 2U), t_edge_at_0(_S153, _S154 + 2U), kernelContext_25);
        return _S156;
    }
    uint _S157 = _S154 - 1U;
    uint2 _S158 = t_edge_at_0(_S153, _S157);
    uint _S159 = _S154 + 1U;
    uint2 _S160 = t_edge_at_0(_S153, _S159);
    float3 _S161 = t_coarse_even_3(_S151, _S152, _S157, _S158, t_edge_at_0(_S153, _S154 - 3U), _S160, kernelContext_25);
    float3 _S162 = t_coarse_even_1(_S151, &_S155, _S159, _S160, _S158, t_edge_at_0(_S153, _S154 + 3U), kernelContext_25);
    return float3(0.5f)  * (_S161 + _S162);
}

float3 t_grid_vertex_1(uint _S163, TerrainDraw_natural_0 device* _S164, uint _S165, uint _S166, KernelContext_0 thread* kernelContext_26)
{
    thread TerrainDraw_natural_0 _S167 = *_S164;
    uint fl_1 = (uint4((&_S167)->slots_0) ).z;
    uint edge_2;
    uint e_6;
    if(_S165 == 0U)
    {
        edge_2 = 0U;
        e_6 = _S166;
    }
    else
    {
        if(_S165 == 64U)
        {
            edge_2 = 1U;
            e_6 = _S166;
        }
        else
        {
            if(_S166 == 0U)
            {
                edge_2 = 2U;
                e_6 = _S165;
            }
            else
            {
                if(_S166 == 64U)
                {
                    edge_2 = 3U;
                    e_6 = _S165;
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
        uint E_1 = 1U << ((fl_1 >> (10U + 3U * edge_2)) & 7U);
        bool _S168;
        if((fl_1 & (1U << edge_2)) != 0U)
        {
            _S168 = E_1 <= 2U;
        }
        else
        {
            _S168 = false;
        }
        if(_S168)
        {
            float3 _S169 = t_coarse_1(_S163, _S164, edge_2, e_6, kernelContext_26);
            return _S169;
        }
        if(E_1 > 1U)
        {
            uint lo_1 = e_6 & (~(E_1 - 1U));
            if(lo_1 == e_6)
            {
                float3 _S170 = t_base_3(_S163, _S164, _S165, _S166, kernelContext_26);
                return _S170;
            }
            uint2 a_3 = t_edge_at_0(edge_2, lo_1);
            uint2 b_3 = t_edge_at_0(edge_2, lo_1 + E_1);
            float3 _S171 = t_base_3(_S163, _S164, a_3.x, a_3.y, kernelContext_26);
            float3 _S172 = t_base_1(_S163, &_S167, b_3.x, b_3.y, kernelContext_26);
            return mix(_S171, _S172, float3((float(e_6 - lo_1) / float(E_1))) );
        }
    }
    float3 _S173 = t_base_3(_S163, _S164, _S165, _S166, kernelContext_26);
    if((fl_1 & 4194304U) != 0U)
    {
        return _S173;
    }
    float m_5 = t_morph_0((float4((&_S167)->origin_0) ).xyz + _S173, (float4((&_S167)->morph_0) ).xy);
    if(m_5 <= 0.0f)
    {
        return _S173;
    }
    float3 _S174 = t_target_1(_S163, &_S167, _S165, _S166, kernelContext_26);
    return mix(_S173, _S174, float3(m_5) );
}

float3 t_vertex_1(uint _S175, TerrainDraw_natural_0 device* _S176, uint _S177, KernelContext_0 thread* kernelContext_27)
{
    TerrainDraw_natural_0 _S178 = *_S176;
    if(_S177 < 4225U)
    {
        float3 _S179 = (float4(_S178.origin_0) ).xyz;
        float3 _S180 = t_grid_vertex_1(_S175, _S176, _S177 % 65U, _S177 / 65U, kernelContext_27);
        return _S179 + _S180;
    }
    uint2 ij_1 = t_ring_0(_S177 - 4225U);
    float3 _S181 = t_grid_vertex_1(_S175, _S176, ij_1.x, ij_1.y, kernelContext_27);
    float depth_1;
    if((((uint4(_S178.slots_0) ).z) & 64U) != 0U)
    {
        depth_1 = (float4(_S178.origin_0) ).w;
    }
    else
    {
        depth_1 = 0.0f;
    }
    float3 _S182 = (float4(_S178.origin_0) ).xyz;
    float3 _S183 = _S182 + _S181;
    float4 _S184 = float4(_S178.dir_c_0) ;
    return _S183 - normalize(_S183 - (_S182 - _S184.xyz * float3(_S184.w) )) * float3(depth_1) ;
}

TerrainShadowOut_0 terrain_shadow_vs_1(uint vid_1, uint iid_1, KernelContext_0 thread* kernelContext_28)
{
    FrameData_natural_0 device* _S185 = ed_buf_1(kernelContext_28->ed_c_0->frame_0, kernelContext_28);
    TerrainDraw_natural_0 device* _S186 = ed_buf_2(kernelContext_28->ed_c_0->draws_0, kernelContext_28);
    float3 _S187 = t_vertex_1(kernelContext_28->ed_c_0->pool_0, _S186 + (kernelContext_28->ed_c_0->base_0 + iid_1), vid_1, kernelContext_28);
    thread TerrainShadowOut_0 o_1;
    (&o_1)->pos_0 = (((float4(_S187, 1.0f)) * (matrix<float,int(4),int(4)> ((&(&_S185->light_0)->shadow_0)->data_2[kernelContext_28->ed_c_0->cascade_0].data_0[int(0)][int(0)], (&(&_S185->light_0)->shadow_0)->data_2[kernelContext_28->ed_c_0->cascade_0].data_0[int(1)][int(0)], (&(&_S185->light_0)->shadow_0)->data_2[kernelContext_28->ed_c_0->cascade_0].data_0[int(2)][int(0)], (&(&_S185->light_0)->shadow_0)->data_2[kernelContext_28->ed_c_0->cascade_0].data_0[int(3)][int(0)], (&(&_S185->light_0)->shadow_0)->data_2[kernelContext_28->ed_c_0->cascade_0].data_0[int(0)][int(1)], (&(&_S185->light_0)->shadow_0)->data_2[kernelContext_28->ed_c_0->cascade_0].data_0[int(1)][int(1)], (&(&_S185->light_0)->shadow_0)->data_2[kernelContext_28->ed_c_0->cascade_0].data_0[int(2)][int(1)], (&(&_S185->light_0)->shadow_0)->data_2[kernelContext_28->ed_c_0->cascade_0].data_0[int(3)][int(1)], (&(&_S185->light_0)->shadow_0)->data_2[kernelContext_28->ed_c_0->cascade_0].data_0[int(0)][int(2)], (&(&_S185->light_0)->shadow_0)->data_2[kernelContext_28->ed_c_0->cascade_0].data_0[int(1)][int(2)], (&(&_S185->light_0)->shadow_0)->data_2[kernelContext_28->ed_c_0->cascade_0].data_0[int(2)][int(2)], (&(&_S185->light_0)->shadow_0)->data_2[kernelContext_28->ed_c_0->cascade_0].data_0[int(3)][int(2)], (&(&_S185->light_0)->shadow_0)->data_2[kernelContext_28->ed_c_0->cascade_0].data_0[int(0)][int(3)], (&(&_S185->light_0)->shadow_0)->data_2[kernelContext_28->ed_c_0->cascade_0].data_0[int(1)][int(3)], (&(&_S185->light_0)->shadow_0)->data_2[kernelContext_28->ed_c_0->cascade_0].data_0[int(2)][int(3)], (&(&_S185->light_0)->shadow_0)->data_2[kernelContext_28->ed_c_0->cascade_0].data_0[int(3)][int(3)]))));
    return o_1;
}

struct TerrainOut_0
{
    float4 pos_1;
    [[flat]] uint draw_0;
    [[flat]] uint lod_0;
};

TerrainOut_0 terrain_vs_0(uint vid_2, uint iid_2, KernelContext_0 thread* kernelContext_29)
{
    FrameData_natural_0 device* _S188 = ed_buf_1(kernelContext_29->ed_c_0->frame_0, kernelContext_29);
    TerrainDraw_natural_0 device* _S189 = ed_buf_2(kernelContext_29->ed_c_0->draws_0, kernelContext_29);
    TerrainDraw_natural_0 device* _S190 = _S189 + (kernelContext_29->ed_c_0->base_0 + iid_2);
    TerrainDraw_natural_0 d_12 = *_S190;
    float3 _S191 = t_vertex_0(kernelContext_29->ed_c_0->pool_0, _S190, vid_2, kernelContext_29);
    thread TerrainOut_0 o_2;
    (&o_2)->pos_1 = (((float4(_S191, 1.0f)) * (matrix<float,int(4),int(4)> (_S188->view_proj_0.data_0[int(0)][int(0)], _S188->view_proj_0.data_0[int(1)][int(0)], _S188->view_proj_0.data_0[int(2)][int(0)], _S188->view_proj_0.data_0[int(3)][int(0)], _S188->view_proj_0.data_0[int(0)][int(1)], _S188->view_proj_0.data_0[int(1)][int(1)], _S188->view_proj_0.data_0[int(2)][int(1)], _S188->view_proj_0.data_0[int(3)][int(1)], _S188->view_proj_0.data_0[int(0)][int(2)], _S188->view_proj_0.data_0[int(1)][int(2)], _S188->view_proj_0.data_0[int(2)][int(2)], _S188->view_proj_0.data_0[int(3)][int(2)], _S188->view_proj_0.data_0[int(0)][int(3)], _S188->view_proj_0.data_0[int(1)][int(3)], _S188->view_proj_0.data_0[int(2)][int(3)], _S188->view_proj_0.data_0[int(3)][int(3)]))));
    (&o_2)->draw_0 = kernelContext_29->ed_c_0->base_0 + iid_2;
    (&o_2)->lod_0 = (((uint4(d_12.slots_0) ).z) >> 8U) & 3U;
    return o_2;
}

TerrainOut_0 terrain_vs_1(uint vid_3, uint iid_3, KernelContext_0 thread* kernelContext_30)
{
    FrameData_natural_0 device* _S192 = ed_buf_1(kernelContext_30->ed_c_0->frame_0, kernelContext_30);
    TerrainDraw_natural_0 device* _S193 = ed_buf_2(kernelContext_30->ed_c_0->draws_0, kernelContext_30);
    TerrainDraw_natural_0 device* _S194 = _S193 + (kernelContext_30->ed_c_0->base_0 + iid_3);
    TerrainDraw_natural_0 d_13 = *_S194;
    float3 _S195 = t_vertex_1(kernelContext_30->ed_c_0->pool_0, _S194, vid_3, kernelContext_30);
    thread TerrainOut_0 o_3;
    (&o_3)->pos_1 = (((float4(_S195, 1.0f)) * (matrix<float,int(4),int(4)> (_S192->view_proj_0.data_0[int(0)][int(0)], _S192->view_proj_0.data_0[int(1)][int(0)], _S192->view_proj_0.data_0[int(2)][int(0)], _S192->view_proj_0.data_0[int(3)][int(0)], _S192->view_proj_0.data_0[int(0)][int(1)], _S192->view_proj_0.data_0[int(1)][int(1)], _S192->view_proj_0.data_0[int(2)][int(1)], _S192->view_proj_0.data_0[int(3)][int(1)], _S192->view_proj_0.data_0[int(0)][int(2)], _S192->view_proj_0.data_0[int(1)][int(2)], _S192->view_proj_0.data_0[int(2)][int(2)], _S192->view_proj_0.data_0[int(3)][int(2)], _S192->view_proj_0.data_0[int(0)][int(3)], _S192->view_proj_0.data_0[int(1)][int(3)], _S192->view_proj_0.data_0[int(2)][int(3)], _S192->view_proj_0.data_0[int(3)][int(3)]))));
    (&o_3)->draw_0 = kernelContext_30->ed_c_0->base_0 + iid_3;
    (&o_3)->lod_0 = (((uint4(d_13.slots_0) ).z) >> 8U) & 3U;
    return o_3;
}

struct pixelOutput_0
{
    uint4 output_0 [[color(0)]];
};

struct pixelInput_0
{
    [[flat]] uint draw_1 [[user(DRAW)]];
    [[flat]] uint lod_1 [[user(LOD)]];
};

[[fragment]] pixelOutput_0 fs_terrain(pixelInput_0 _S196 [[stage_in]], uint prim_0 [[primitive_id]], float4 pos_2 [[position]], ulong device* ed_buffers_1 [[buffer(2)]], TerrainC_0 constant* ed_c_1 [[buffer(0)]])
{
    thread KernelContext_0 kernelContext_31;
    (&kernelContext_31)->ed_buffers_0 = ed_buffers_1;
    (&kernelContext_31)->ed_c_0 = ed_c_1;
    pixelOutput_0 _S197 = { uint4(2147483648U | (_S196.draw_1), (prim_0 | ((_S196.lod_1) << 24U)) & 268435455U, 0U, 0U) };
    return _S197;
}

struct vs_terrain_Result_0
{
    float4 pos_3 [[position]];
    uint draw_2 [[user(DRAW)]];
    uint lod_2 [[user(LOD)]];
};

[[vertex]] vs_terrain_Result_0 vs_terrain(uint vid_4 [[vertex_id]], uint iid_4 [[instance_id]], ulong device* ed_buffers_2 [[buffer(2)]], TerrainC_0 constant* ed_c_2 [[buffer(0)]])
{
    thread KernelContext_0 kernelContext_32;
    (&kernelContext_32)->ed_buffers_0 = ed_buffers_2;
    (&kernelContext_32)->ed_c_0 = ed_c_2;
    TerrainOut_0 _S198 = terrain_vs_1(vid_4, iid_4, &kernelContext_32);
    thread vs_terrain_Result_0 _S199;
    (&_S199)->pos_3 = _S198.pos_1;
    (&_S199)->draw_2 = _S198.draw_0;
    (&_S199)->lod_2 = _S198.lod_0;
    return _S199;
}

struct vs_terrain_trees_Result_0
{
    float4 pos_4 [[position]];
    uint draw_3 [[user(DRAW)]];
    uint lod_3 [[user(LOD)]];
};

[[vertex]] vs_terrain_trees_Result_0 vs_terrain_trees(uint vid_5 [[vertex_id]], uint iid_5 [[instance_id]], ulong device* ed_buffers_3 [[buffer(2)]], TerrainC_0 constant* ed_c_3 [[buffer(0)]])
{
    thread KernelContext_0 kernelContext_33;
    (&kernelContext_33)->ed_buffers_0 = ed_buffers_3;
    (&kernelContext_33)->ed_c_0 = ed_c_3;
    TerrainOut_0 _S200 = terrain_vs_0(vid_5, iid_5, &kernelContext_33);
    thread vs_terrain_trees_Result_0 _S201;
    (&_S201)->pos_4 = _S200.pos_1;
    (&_S201)->draw_3 = _S200.draw_0;
    (&_S201)->lod_3 = _S200.lod_0;
    return _S201;
}

struct vs_terrain_shadow_Result_0
{
    float4 pos_5 [[position]];
};

[[vertex]] vs_terrain_shadow_Result_0 vs_terrain_shadow(uint vid_6 [[vertex_id]], uint iid_6 [[instance_id]], ulong device* ed_buffers_4 [[buffer(2)]], TerrainC_0 constant* ed_c_4 [[buffer(0)]])
{
    thread KernelContext_0 kernelContext_34;
    (&kernelContext_34)->ed_buffers_0 = ed_buffers_4;
    (&kernelContext_34)->ed_c_0 = ed_c_4;
    TerrainShadowOut_0 _S202 = terrain_shadow_vs_1(vid_6, iid_6, &kernelContext_34);
    thread vs_terrain_shadow_Result_0 _S203;
    (&_S203)->pos_5 = _S202.pos_0;
    return _S203;
}

struct vs_terrain_shadow_trees_Result_0
{
    float4 pos_6 [[position]];
};

[[vertex]] vs_terrain_shadow_trees_Result_0 vs_terrain_shadow_trees(uint vid_7 [[vertex_id]], uint iid_7 [[instance_id]], ulong device* ed_buffers_5 [[buffer(2)]], TerrainC_0 constant* ed_c_5 [[buffer(0)]])
{
    thread KernelContext_0 kernelContext_35;
    (&kernelContext_35)->ed_buffers_0 = ed_buffers_5;
    (&kernelContext_35)->ed_c_0 = ed_c_5;
    TerrainShadowOut_0 _S204 = terrain_shadow_vs_0(vid_7, iid_7, &kernelContext_35);
    thread vs_terrain_shadow_trees_Result_0 _S205;
    (&_S205)->pos_6 = _S204.pos_0;
    return _S205;
}

