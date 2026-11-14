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

struct ResolveC_0
{
    uint frame_0;
    uint vis_0;
    uint instances_0;
    uint nodes_0;
    uint meshes_0;
    uint materials_0;
    uint verts_0;
    uint indices_0;
    uint uvs_0;
    uint smp_0;
    uint shadow_0;
    uint ao_0;
    uint t_draws_0;
    uint t_pool_0;
    uint t_on_0;
    uint t_mat_0;
};

struct KernelContext_0
{
    ulong device* ed_buffers_0;
    ResolveC_0 constant* ed_c_0;
    ulong device* ed_samplers_0;
    ulong device* ed_textures_0;
};

uint device* ed_buf_0(uint i_0, KernelContext_0 thread* kernelContext_0)
{
    return (uint device*)kernelContext_0->ed_buffers_0[i_0];
}

float t_page_0(uint pool_0, uint slot_0, uint i_1, uint j_0, float scale_0, float offset_0, KernelContext_0 thread* kernelContext_1)
{
    uint s_0 = j_0 * 257U + i_1;
    uint device* _S1 = ed_buf_0(pool_0, kernelContext_1);
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

float t_own_h_0(uint pool_1, const TerrainDraw_natural_0 thread* d_0, uint i_2, uint j_1, KernelContext_0 thread* kernelContext_2)
{
    float4 _S2 = float4(d_0->hq_0) ;
    float _S3 = t_page_0(pool_1, (uint4(d_0->slots_0) ).x, i_2 * 4U, j_1 * 4U, _S2.x, _S2.y, kernelContext_2);
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
    uint pad_0;
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
    _Array_natural_matrixx3Cfloatx2C4x2C4x3E4_0 shadow_1;
    _Array_natural_vectorx3Cfloatx2C4x3E4_0 cascade_0;
    packed_float4 split_0;
    packed_float4 cfg_0;
    packed_float4 filter_0;
    packed_uint4 flags_0;
    packed_float4 ao_1;
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
    array<HzbLevel_0, int(16)> hzb_0;
    LightData_natural_0 light_0;
    AtmosData_natural_0 atmos_0;
    WindData_natural_0 wind_1;
    ImpData_natural_0 imp_0;
    SeasonData_natural_0 season_0;
    GrassData_natural_0 grass_0;
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

uint t_canopy_raw_0(uint pool_2, uint off_0, uint i_5, uint j_3, KernelContext_0 thread* kernelContext_5)
{
    uint device* _S19 = ed_buf_0(pool_2, kernelContext_5);
    return *(_S19 + (off_0 + j_3 * 257U + i_5));
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

float t_canopy_lift_0(uint pool_3, uint off_1, uint i_6, uint j_4, KernelContext_0 thread* kernelContext_6)
{
    if(off_1 == 0U)
    {
        return 0.0f;
    }
    uint _S21 = t_canopy_raw_0(pool_3, off_1, i_6, j_4, kernelContext_6);
    return t_canopy_h_0((_S21 >> 8U) & 255U) * smoothstep(0.059999998658895493f, 0.44999998807907104f, float(_S21 & 255U) / 255.0f);
}

float3 t_target_even_0(uint pool_4, const TerrainDraw_natural_0 thread* d_3, uint i_7, uint j_5, KernelContext_0 thread* kernelContext_7)
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
        float _S24 = t_own_h_0(pool_4, d_3, i_7, j_5, kernelContext_7);
        h_1 = _S24;
    }
    else
    {
        float4 _S25 = float4(d_3->hq_0) ;
        float _S26 = t_page_0(pool_4, _S22.y, qx_0 + 2U * i_7, qy_0 + 2U * j_5, _S25.z, _S25.w, kernelContext_7);
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
                float _S30 = t_canopy_lift_0(pool_4, (as_type<uint>((_S29.z))), i_7 * 4U, j_5 * 4U, kernelContext_7);
                h_1 = _S30;
            }
            else
            {
                float _S31 = t_canopy_lift_0(pool_4, poff_0, qx_0 + 2U * i_7, qy_0 + 2U * j_5, kernelContext_7);
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

float t_own_lift_0(uint pool_5, const TerrainDraw_natural_0 thread* d_4, uint i_8, uint j_6, float3 p_2, KernelContext_0 thread* kernelContext_8)
{
    if(((float4(d_4->polar_0) ).w) <= 0.0f)
    {
        return 0.0f;
    }
    float _S32 = t_lift_k_0(d_4, p_2, kernelContext_8);
    float _S33;
    if(_S32 > 0.0f)
    {
        float _S34 = t_canopy_lift_0(pool_5, (as_type<uint>(((float4(d_4->tex_0) ).z))), i_8 * 4U, j_6 * 4U, kernelContext_8);
        _S33 = _S32 * _S34;
    }
    else
    {
        _S33 = 0.0f;
    }
    return _S33;
}

float3 t_base_0(uint pool_6, const TerrainDraw_natural_0 thread* d_5, uint i_9, uint j_7, KernelContext_0 thread* kernelContext_9)
{
    float _S35 = t_own_h_0(pool_6, d_5, i_9, j_7, kernelContext_9);
    thread float3 dir_2;
    float3 _S36 = t_lattice_0(d_5, i_9, j_7, _S35, &dir_2);
    float3 p_3;
    if(((float4(d_5->polar_0) ).w) > 0.0f)
    {
        float _S37 = t_own_lift_0(pool_6, d_5, i_9, j_7, _S36, kernelContext_9);
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

float3 t_coarse_even_0(uint pool_7, const TerrainDraw_natural_0 thread* d_6, uint e_1, uint2 a_0, uint2 b_0, uint2 c_0, KernelContext_0 thread* kernelContext_10)
{
    float3 _S39 = t_base_0(pool_7, d_6, a_0.x, a_0.y, kernelContext_10);
    if((e_1 & 3U) == 0U)
    {
        return _S39;
    }
    float m_0 = t_morph_0((float4(d_6->origin_0) ).xyz + _S39, (float4(d_6->morph_0) ).zw);
    float3 _S40 = t_base_0(pool_7, d_6, b_0.x, b_0.y, kernelContext_10);
    float3 _S41 = t_base_0(pool_7, d_6, c_0.x, c_0.y, kernelContext_10);
    return mix(_S39, float3(0.5f)  * (_S40 + _S41), float3(m_0) );
}

float3 t_target_0(uint pool_8, const TerrainDraw_natural_0 thread* d_7, uint i_10, uint j_8, KernelContext_0 thread* kernelContext_11)
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
        float3 _S43 = t_target_even_0(pool_8, d_7, i_10, j_8, kernelContext_11);
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
        float3 _S44 = t_target_even_0(pool_8, d_7, i_10 - 1U, j_8, kernelContext_11);
        float3 _S45 = t_target_even_0(pool_8, d_7, i_10 + 1U, j_8, kernelContext_11);
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
        float3 _S46 = t_target_even_0(pool_8, d_7, i_10, j_8 - 1U, kernelContext_11);
        float3 _S47 = t_target_even_0(pool_8, d_7, i_10, j_8 + 1U, kernelContext_11);
        return float3(0.5f)  * (_S46 + _S47);
    }
    float3 _S48 = t_target_even_0(pool_8, d_7, i_10 - 1U, j_8 - 1U, kernelContext_11);
    float3 _S49 = t_target_even_0(pool_8, d_7, i_10 + 1U, j_8 + 1U, kernelContext_11);
    return float3(0.5f)  * (_S48 + _S49);
}

uint2 t_edge_at_0(uint edge_0, uint e_2)
{
    switch(edge_0)
    {
    case 0U:
        {
            return uint2(0U, e_2);
        }
    case 1U:
        {
            return uint2(64U, e_2);
        }
    case 2U:
        {
            return uint2(e_2, 0U);
        }
    default:
        {
            return uint2(e_2, 64U);
        }
    }
}

float3 t_coarse_0(uint pool_9, const TerrainDraw_natural_0 thread* d_8, uint edge_1, uint e_3, KernelContext_0 thread* kernelContext_12)
{
    if((e_3 & 1U) == 0U)
    {
        float3 _S50 = t_coarse_even_0(pool_9, d_8, e_3, t_edge_at_0(edge_1, e_3), t_edge_at_0(edge_1, e_3 - 2U), t_edge_at_0(edge_1, e_3 + 2U), kernelContext_12);
        return _S50;
    }
    uint _S51 = e_3 - 1U;
    uint2 _S52 = t_edge_at_0(edge_1, _S51);
    uint _S53 = e_3 + 1U;
    uint2 _S54 = t_edge_at_0(edge_1, _S53);
    float3 _S55 = t_coarse_even_0(pool_9, d_8, _S51, _S52, t_edge_at_0(edge_1, e_3 - 3U), _S54, kernelContext_12);
    float3 _S56 = t_coarse_even_0(pool_9, d_8, _S53, _S54, _S52, t_edge_at_0(edge_1, e_3 + 3U), kernelContext_12);
    return float3(0.5f)  * (_S55 + _S56);
}

float3 t_grid_vertex_0(uint pool_10, const TerrainDraw_natural_0 thread* d_9, uint i_11, uint j_9, KernelContext_0 thread* kernelContext_13)
{
    uint fl_0 = (uint4(d_9->slots_0) ).z;
    uint edge_2;
    uint e_4;
    if(i_11 == 0U)
    {
        edge_2 = 0U;
        e_4 = j_9;
    }
    else
    {
        if(i_11 == 64U)
        {
            edge_2 = 1U;
            e_4 = j_9;
        }
        else
        {
            if(j_9 == 0U)
            {
                edge_2 = 2U;
                e_4 = i_11;
            }
            else
            {
                if(j_9 == 64U)
                {
                    edge_2 = 3U;
                    e_4 = i_11;
                }
                else
                {
                    edge_2 = 4U;
                    e_4 = 0U;
                }
            }
        }
    }
    if(edge_2 < 4U)
    {
        uint E_0 = 1U << ((fl_0 >> (10U + 3U * edge_2)) & 7U);
        bool _S57;
        if((fl_0 & (1U << edge_2)) != 0U)
        {
            _S57 = E_0 <= 2U;
        }
        else
        {
            _S57 = false;
        }
        if(_S57)
        {
            float3 _S58 = t_coarse_0(pool_10, d_9, edge_2, e_4, kernelContext_13);
            return _S58;
        }
        if(E_0 > 1U)
        {
            uint lo_0 = e_4 & (~(E_0 - 1U));
            if(lo_0 == e_4)
            {
                float3 _S59 = t_base_0(pool_10, d_9, i_11, j_9, kernelContext_13);
                return _S59;
            }
            uint2 a_1 = t_edge_at_0(edge_2, lo_0);
            uint2 b_1 = t_edge_at_0(edge_2, lo_0 + E_0);
            float3 _S60 = t_base_0(pool_10, d_9, a_1.x, a_1.y, kernelContext_13);
            float3 _S61 = t_base_0(pool_10, d_9, b_1.x, b_1.y, kernelContext_13);
            return mix(_S60, _S61, float3((float(e_4 - lo_0) / float(E_0))) );
        }
    }
    float3 _S62 = t_base_0(pool_10, d_9, i_11, j_9, kernelContext_13);
    if((fl_0 & 4194304U) != 0U)
    {
        return _S62;
    }
    float m_1 = t_morph_0((float4(d_9->origin_0) ).xyz + _S62, (float4(d_9->morph_0) ).xy);
    if(m_1 <= 0.0f)
    {
        return _S62;
    }
    float3 _S63 = t_target_0(pool_10, d_9, i_11, j_9, kernelContext_13);
    return mix(_S62, _S63, float3(m_1) );
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

float3 t_vertex_0(uint pool_11, const TerrainDraw_natural_0 thread* d_10, uint v_0, KernelContext_0 thread* kernelContext_14)
{
    if(v_0 < 4225U)
    {
        float3 _S64 = (float4(d_10->origin_0) ).xyz;
        float3 _S65 = t_grid_vertex_0(pool_11, d_10, v_0 % 65U, v_0 / 65U, kernelContext_14);
        return _S64 + _S65;
    }
    uint2 ij_0 = t_ring_0(v_0 - 4225U);
    uint _S66 = ij_0.x;
    uint _S67 = ij_0.y;
    float3 _S68 = t_grid_vertex_0(pool_11, d_10, _S66, _S67, kernelContext_14);
    float depth_0;
    if((((uint4(d_10->slots_0) ).z) & 64U) != 0U)
    {
        depth_0 = (float4(d_10->origin_0) ).w;
    }
    else
    {
        depth_0 = 0.0f;
    }
    bool _S69;
    if(((float4(d_10->polar_0) ).w) > 0.0f)
    {
        _S69 = depth_0 > 0.0f;
    }
    else
    {
        _S69 = false;
    }
    if(_S69)
    {
        float _S70 = t_own_lift_0(pool_11, d_10, _S66, _S67, _S68, kernelContext_14);
        depth_0 = depth_0 + _S70;
    }
    float3 _S71 = (float4(d_10->origin_0) ).xyz;
    float3 _S72 = _S71 + _S68;
    float4 _S73 = float4(d_10->dir_c_0) ;
    return _S72 - normalize(_S72 - (_S71 - _S73.xyz * float3(_S73.w) )) * float3(depth_0) ;
}

constant array<float3, int(5)> TINT_0 = { { float3(1.0f, 0.25f, 0.25f), float3(0.25f, 1.0f, 0.25f), float3(0.30000001192092896f, 0.44999998807907104f, 1.0f), float3(1.0f, 1.0f, 0.25f), float3(0.60000002384185791f, 0.60000002384185791f, 0.60000002384185791f) } };
struct FsOut_0
{
    float4 pos_0;
};

TerrainDraw_natural_0 device* ed_buf_2(uint i_12, KernelContext_0 thread* kernelContext_15)
{
    return (TerrainDraw_natural_0 device*)kernelContext_15->ed_buffers_0[i_12];
}

uint3 t_triangle_0(uint prim_0)
{
    uint lod_1 = (prim_0 >> 24U) & 3U;
    uint _S74 = prim_0 & 16777215U;
    uint st_0 = 1U << lod_1;
    uint c_1 = 64U >> lod_1;
    uint _S75 = 2U * c_1 * c_1;
    uint3 _S76;
    if(_S74 < _S75)
    {
        uint q_1 = _S74 >> 1U;
        uint _S77 = q_1 % c_1;
        uint i_13 = _S77 * st_0;
        uint _S78 = q_1 / c_1;
        uint v00_0 = _S78 * st_0 * 65U + i_13;
        uint v10_0 = v00_0 + st_0;
        uint v01_0 = v00_0 + st_0 * 65U;
        uint v11_0 = v01_0 + st_0;
        if((_S74 & 1U) == 0U)
        {
            _S76 = uint3(v00_0, v10_0, v11_0);
        }
        else
        {
            _S76 = uint3(v00_0, v11_0, v01_0);
        }
        return _S76;
    }
    uint k_1 = _S74 - _S75;
    uint s_3 = k_1 >> 1U;
    uint s1_0 = (s_3 + 1U) % (4U * c_1);
    uint _S79 = s_3 * st_0;
    uint2 p_4 = t_ring_0(_S79);
    uint _S80 = s1_0 * st_0;
    uint2 q_2 = t_ring_0(_S80);
    uint tp_0 = p_4.y * 65U + p_4.x;
    uint tq_0 = q_2.y * 65U + q_2.x;
    uint a_2 = 4225U + _S79;
    uint b_2 = 4225U + _S80;
    if((k_1 & 1U) == 0U)
    {
        _S76 = uint3(a_2, b_2, tq_0);
    }
    else
    {
        _S76 = uint3(a_2, tq_0, tp_0);
    }
    return _S76;
}

float2 ray_bary_0(float3 dir_3, float3 p0_0, float3 e1_0, float3 e2_0)
{
    float3 pv_0 = cross(dir_3, e2_0);
    float det_0 = dot(e1_0, pv_0);
    if((abs(det_0)) <= 9.999999960041972e-13f)
    {
        return float2(0.3333333432674408f) ;
    }
    float inv_0 = 1.0f / det_0;
    float3 tv_1 = - p0_0;
    return float2(dot(tv_1, pv_0) * inv_0, dot(dir_3, cross(tv_1, e1_0)) * inv_0);
}

float2 t_vertex_grid_0(uint v_1)
{
    if(v_1 < 4225U)
    {
        return float2(float(v_1 % 65U), float(v_1 / 65U));
    }
    return float2(t_ring_0(v_1 - 4225U));
}

float3 t_lattice_f_0(const TerrainDraw_natural_0 thread* d_11, float2 g_0, float h_2, float3 thread* dir_4)
{
    float4 _S81 = float4(d_11->shape_0) ;
    float _S82 = _S81.x;
    float tu_1 = tan((g_0.x - 32.0f) * _S82);
    float tv_2 = tan((g_0.y - 32.0f) * _S82);
    float4 _S83 = float4(d_11->fu_0) ;
    float _S84 = _S83.w;
    float4 _S85 = float4(d_11->fv_0) ;
    float _S86 = _S85.w;
    float3 delta_1 = float3((tu_1 * (1.0f + _S84 * _S84) / (1.0f - _S84 * tu_1)))  * _S83.xyz + float3((tv_2 * (1.0f + _S86 * _S86) / (1.0f - _S86 * tv_2)))  * _S85.xyz;
    float4 _S87 = float4(d_11->cc_0) ;
    float lc_1 = _S87.w;
    float3 _S88 = _S87.xyz;
    float k_2 = 2.0f * dot(_S88, delta_1) + dot(delta_1, delta_1);
    float l_1 = sqrt(lc_1 * lc_1 + k_2);
    float3 dd_1 = delta_1 / float3(l_1)  - _S88 * float3((k_2 / ((lc_1 + l_1) * l_1 * lc_1))) ;
    float4 _S89 = float4(d_11->dir_c_0) ;
    float3 _S90 = _S89.xyz;
    float3 _S91 = _S90 + dd_1;
    *dir_4 = _S91;
    float3 _S92 = (float4(d_11->polar_0) ).xyz;
    float dzc_1 = dot(_S90, _S92);
    float ddz_1 = dot(dd_1, _S92);
    float dz_1 = dzc_1 + ddz_1;
    float e_6 = _S81.z;
    float s_4 = sqrt(1.0f - e_6 * dz_1 * dz_1);
    float sc_1 = _S81.w;
    float dr_1 = _S81.y * e_6 * ddz_1 * (dz_1 + dzc_1) / (s_4 * sc_1 * (s_4 + sc_1));
    return dd_1 * float3((_S89.w + dr_1))  + _S90 * float3(dr_1)  + _S91 * float3(h_2) ;
}

float t_sample_spacing_0(const TerrainDraw_natural_0 thread* d_12, float2 g_1)
{
    float2 _S93 = float2(0.5f, 0.0f);
    thread float3 dir_5;
    float3 _S94 = t_lattice_f_0(d_12, g_1 + _S93, 0.0f, &dir_5);
    float3 _S95 = t_lattice_f_0(d_12, g_1 - _S93, 0.0f, &dir_5);
    return length(_S94 - _S95) / 4.0f;
}

float t_page_at_0(uint pool_12, const TerrainDraw_natural_0 thread* d_13, int a_3, int b_3, KernelContext_0 thread* kernelContext_16)
{
    float4 _S96 = float4(d_13->hq_0) ;
    float _S97 = t_page_0(pool_12, (uint4(d_13->slots_0) ).x, uint(clamp(a_3, int(0), int(256))), uint(clamp(b_3, int(0), int(256))), _S96.x, _S96.y, kernelContext_16);
    return _S97;
}

void t_surface_0(uint pool_13, const TerrainDraw_natural_0 thread* d_14, float2 g_2, int r_1, float thread* h_3, float2 thread* dh_0, KernelContext_0 thread* kernelContext_17)
{
    float2 _S98 = float2(4.0f) ;
    float2 _S99 = float2(0.0f) ;
    float2 s_5 = clamp(g_2 * _S98, _S99, float2(256.0f) );
    int2 _S100 = int2(min(floor(s_5), float2(255.0f) ));
    float2 _S101 = s_5 - float2(_S100);
    *h_3 = 0.0f;
    *dh_0 = _S99;
    uint k_3 = 0U;
    for(;;)
    {
        if(k_3 < 4U)
        {
        }
        else
        {
            break;
        }
        uint _S102 = k_3 & 1U;
        uint _S103 = k_3 >> 1U;
        int2 q_3 = _S100 + int2(int(_S102), int(_S103));
        float _S104;
        if(_S102 != 0U)
        {
            _S104 = _S101.x;
        }
        else
        {
            _S104 = 1.0f - _S101.x;
        }
        float _S105;
        if(_S103 != 0U)
        {
            _S105 = _S101.y;
        }
        else
        {
            _S105 = 1.0f - _S101.y;
        }
        float w_1 = _S104 * _S105;
        int2 _S106 = int2(r_1) ;
        int2 _S107 = max(q_3 - _S106, int2(int(0)) );
        int2 _S108 = min(q_3 + _S106, int2(int(256)) );
        int _S109 = _S108.x;
        int _S110 = q_3.y;
        float _S111 = t_page_at_0(pool_13, d_14, _S109, _S110, kernelContext_17);
        int _S112 = _S107.x;
        float _S113 = t_page_at_0(pool_13, d_14, _S112, _S110, kernelContext_17);
        float gx_0 = (_S111 - _S113) / float(_S109 - _S112);
        int _S114 = q_3.x;
        int _S115 = _S108.y;
        float _S116 = t_page_at_0(pool_13, d_14, _S114, _S115, kernelContext_17);
        int _S117 = _S107.y;
        float _S118 = t_page_at_0(pool_13, d_14, _S114, _S117, kernelContext_17);
        float gy_0 = (_S116 - _S118) / float(_S115 - _S117);
        float _S119 = t_page_at_0(pool_13, d_14, _S114, _S110, kernelContext_17);
        *h_3 = *h_3 + w_1 * _S119;
        *dh_0 = *dh_0 + float2(w_1)  * float2(gx_0, gy_0);
        k_3 = k_3 + 1U;
    }
    *dh_0 = *dh_0 * _S98;
    return;
}

float3 t_lattice_f_1(const TerrainDraw_natural_0 thread* d_15, float2 g_3, float h_4, float3 thread* dir_6)
{
    float4 _S120 = float4(d_15->shape_0) ;
    float _S121 = _S120.x;
    float tu_2 = tan((g_3.x - 32.0f) * _S121);
    float tv_3 = tan((g_3.y - 32.0f) * _S121);
    float4 _S122 = float4(d_15->fu_0) ;
    float _S123 = _S122.w;
    float4 _S124 = float4(d_15->fv_0) ;
    float _S125 = _S124.w;
    float3 delta_2 = float3((tu_2 * (1.0f + _S123 * _S123) / (1.0f - _S123 * tu_2)))  * _S122.xyz + float3((tv_3 * (1.0f + _S125 * _S125) / (1.0f - _S125 * tv_3)))  * _S124.xyz;
    float4 _S126 = float4(d_15->cc_0) ;
    float lc_2 = _S126.w;
    float3 _S127 = _S126.xyz;
    float k_4 = 2.0f * dot(_S127, delta_2) + dot(delta_2, delta_2);
    float l_2 = sqrt(lc_2 * lc_2 + k_4);
    float3 dd_2 = delta_2 / float3(l_2)  - _S127 * float3((k_4 / ((lc_2 + l_2) * l_2 * lc_2))) ;
    float4 _S128 = float4(d_15->dir_c_0) ;
    float3 _S129 = _S128.xyz;
    float3 _S130 = _S129 + dd_2;
    *dir_6 = _S130;
    float3 _S131 = (float4(d_15->polar_0) ).xyz;
    float dzc_2 = dot(_S129, _S131);
    float ddz_2 = dot(dd_2, _S131);
    float dz_2 = dzc_2 + ddz_2;
    float e_7 = _S120.z;
    float s_6 = sqrt(1.0f - e_7 * dz_2 * dz_2);
    float sc_2 = _S120.w;
    float dr_2 = _S120.y * e_7 * ddz_2 * (dz_2 + dzc_2) / (s_6 * sc_2 * (s_6 + sc_2));
    return dd_2 * float3((_S128.w + dr_2))  + _S129 * float3(dr_2)  + _S130 * float3(h_4) ;
}

float3 t_smooth_normal_0(uint pool_14, const TerrainDraw_natural_0 thread* d_16, float2 g_4, int r_2, float3 thread* up_1, float thread* h_5, KernelContext_0 thread* kernelContext_18)
{
    thread float2 dh_1;
    t_surface_0(pool_14, d_16, g_4, r_2, h_5, &dh_1, kernelContext_18);
    float2 _S132 = float2(0.25f, 0.0f);
    thread float3 dir_7;
    float3 _S133 = t_lattice_f_0(d_16, g_4 + _S132, *h_5, &dir_7);
    float3 _S134 = t_lattice_f_0(d_16, g_4 - _S132, *h_5, &dir_7);
    float3 pu_0 = _S133 - _S134;
    float2 _S135 = float2(0.0f, 0.25f);
    float3 _S136 = t_lattice_f_0(d_16, g_4 + _S135, *h_5, &dir_7);
    float3 _S137 = t_lattice_f_0(d_16, g_4 - _S135, *h_5, &dir_7);
    float3 pv_1 = _S136 - _S137;
    float3 _S138 = t_lattice_f_1(d_16, g_4, *h_5, up_1);
    float3 _S139 = normalize(*up_1);
    *up_1 = _S139;
    float3 _S140 = float3(0.5f) ;
    float3 n_0 = normalize(cross(pu_0 / _S140 + _S139 * float3(dh_1.x) , pv_1 / _S140 + _S139 * float3(dh_1.y) ));
    float3 _S141;
    if((dot(n_0, _S139)) < 0.0f)
    {
        _S141 = - n_0;
    }
    else
    {
        _S141 = n_0;
    }
    return _S141;
}

float3 terrain_albedo_0(float h_6, float slope_cos_0)
{
    return mix(mix(mix(float3(0.054999999701976776f, 0.075000002980232239f, 0.039999999105930328f), float3(0.125f, 0.14000000059604645f, 0.070000000298023224f), float3(smoothstep(1950.0f, 2350.0f, h_6)) ), float3(0.20999999344348907f, 0.20000000298023224f, 0.18500000238418579f), float3(smoothstep(0.81999999284744263f, 0.72000002861022949f, slope_cos_0)) ), float3(0.80000001192092896f, 0.81999999284744263f, 0.86000001430511475f), float3((smoothstep(2750.0f, 3050.0f, h_6) * smoothstep(0.55000001192092896f, 0.69999998807907104f, slope_cos_0))) );
}

float4 t_canopy_bilin_0(uint pool_15, uint off_2, float2 s_7, KernelContext_0 thread* kernelContext_19)
{
    float2 _S142 = clamp(s_7, float2(0.0f) , float2(256.0f) );
    int2 _S143 = int2(min(floor(_S142), float2(255.0f) ));
    float2 _S144 = _S142 - float2(_S143);
    uint _S145 = uint(_S143.x);
    uint _S146 = uint(_S143.y);
    uint _S147 = t_canopy_raw_0(pool_15, off_2, _S145, _S146, kernelContext_19);
    float _S148 = _S144.x;
    float _S149 = 1.0f - _S148;
    float _S150 = _S144.y;
    float _S151 = 1.0f - _S150;
    float wk_0 = _S149 * _S151;
    float ww_0 = wk_0 * (float(_S147 & 255U) / 255.0f);
    float _S152 = ww_0 * t_canopy_h_0((_S147 >> 8U) & 255U);
    float _S153 = ww_0 * float((_S147 >> 16U) & 255U) / 255.0f;
    float _S154 = wk_0 * float(_S147 >> 30U) / 3.0f;
    uint _S155 = _S145 + 1U;
    uint _S156 = t_canopy_raw_0(pool_15, off_2, _S155, _S146, kernelContext_19);
    float wk_1 = _S148 * _S151;
    float ww_1 = wk_1 * (float(_S156 & 255U) / 255.0f);
    float cover_0 = ww_0 + ww_1;
    float hs_0 = _S152 + ww_1 * t_canopy_h_0((_S156 >> 8U) & 255U);
    float mx_0 = _S153 + ww_1 * float((_S156 >> 16U) & 255U) / 255.0f;
    float un_0 = _S154 + wk_1 * float(_S156 >> 30U) / 3.0f;
    uint _S157 = _S146 + 1U;
    uint _S158 = t_canopy_raw_0(pool_15, off_2, _S145, _S157, kernelContext_19);
    float wk_2 = _S149 * _S150;
    float ww_2 = wk_2 * (float(_S158 & 255U) / 255.0f);
    float cover_1 = cover_0 + ww_2;
    float hs_1 = hs_0 + ww_2 * t_canopy_h_0((_S158 >> 8U) & 255U);
    float mx_1 = mx_0 + ww_2 * float((_S158 >> 16U) & 255U) / 255.0f;
    float un_1 = un_0 + wk_2 * float(_S158 >> 30U) / 3.0f;
    uint _S159 = t_canopy_raw_0(pool_15, off_2, _S155, _S157, kernelContext_19);
    float wk_3 = _S148 * _S150;
    float ww_3 = wk_3 * (float(_S159 & 255U) / 255.0f);
    float cover_2 = cover_1 + ww_3;
    float hs_2 = hs_1 + ww_3 * t_canopy_h_0((_S159 >> 8U) & 255U);
    float mx_2 = mx_1 + ww_3 * float((_S159 >> 16U) & 255U) / 255.0f;
    float un_2 = un_1 + wk_3 * float(_S159 >> 30U) / 3.0f;
    float4 _S160;
    if(cover_2 > 0.0f)
    {
        _S160 = float4(cover_2, hs_2 / cover_2, mx_2 / cover_2, un_2);
    }
    else
    {
        _S160 = float4(cover_2, 0.0f, 0.0f, un_2);
    }
    return _S160;
}

struct _Array_natural_vectorx3Cfloatx2C4x3E6_0
{
    array<packed_float4, int(6)> data_6;
};

struct _Array_natural_vectorx3Cfloatx2C4x3E2_0
{
    array<packed_float4, int(2)> data_7;
};

struct TerrainMat_natural_0
{
    uint albedo_0;
    uint normal_0;
    uint layers_0;
    uint size_0;
    array<float, int(8)> tile_0;
    _Array_natural_vectorx3Cfloatx2C4x3E6_0 gain_0;
    _Array_natural_vectorx3Cfloatx2C4x3E2_0 pad_1;
};

TerrainMat_natural_0 device* ed_buf_3(uint i_14, KernelContext_0 thread* kernelContext_20)
{
    return (TerrainMat_natural_0 device*)kernelContext_20->ed_buffers_0[i_14];
}

float3 view_dir_0(FrameData_natural_0 device* f_0, float2 pixel_0)
{
    float2 uv_0 = pixel_0 / (float4(f_0->viewport_0) ).zw;
    float4 p_5 = (((float4(float2(uv_0.x * 2.0f - 1.0f, 1.0f - uv_0.y * 2.0f), 1.0f, 1.0f)) * (matrix<float,int(4),int(4)> (f_0->inv_view_proj_0.data_0[int(0)][int(0)], f_0->inv_view_proj_0.data_0[int(1)][int(0)], f_0->inv_view_proj_0.data_0[int(2)][int(0)], f_0->inv_view_proj_0.data_0[int(3)][int(0)], f_0->inv_view_proj_0.data_0[int(0)][int(1)], f_0->inv_view_proj_0.data_0[int(1)][int(1)], f_0->inv_view_proj_0.data_0[int(2)][int(1)], f_0->inv_view_proj_0.data_0[int(3)][int(1)], f_0->inv_view_proj_0.data_0[int(0)][int(2)], f_0->inv_view_proj_0.data_0[int(1)][int(2)], f_0->inv_view_proj_0.data_0[int(2)][int(2)], f_0->inv_view_proj_0.data_0[int(3)][int(2)], f_0->inv_view_proj_0.data_0[int(0)][int(3)], f_0->inv_view_proj_0.data_0[int(1)][int(3)], f_0->inv_view_proj_0.data_0[int(2)][int(3)], f_0->inv_view_proj_0.data_0[int(3)][int(3)]))));
    return normalize(p_5.xyz / float3(p_5.w) );
}

float t_page_bilin_0(uint pool_16, const TerrainDraw_natural_0 thread* d_17, float2 s_8, KernelContext_0 thread* kernelContext_21)
{
    float2 _S161 = clamp(s_8, float2(0.0f) , float2(256.0f) );
    int2 _S162 = int2(min(floor(_S161), float2(255.0f) ));
    float2 t_0 = _S161 - float2(_S162);
    int _S163 = _S162.x;
    int _S164 = _S162.y;
    float _S165 = t_page_at_0(pool_16, d_17, _S163, _S164, kernelContext_21);
    int _S166 = _S163 + int(1);
    float _S167 = t_page_at_0(pool_16, d_17, _S166, _S164, kernelContext_21);
    float _S168 = t_0.x;
    float a_4 = mix(_S165, _S167, _S168);
    int _S169 = _S164 + int(1);
    float _S170 = t_page_at_0(pool_16, d_17, _S163, _S169, kernelContext_21);
    float _S171 = t_page_at_0(pool_16, d_17, _S166, _S169, kernelContext_21);
    return mix(a_4, mix(_S170, _S171, _S168), t_0.y);
}

float length_squared_0(float3 v_2)
{
    return dot(v_2, v_2);
}

struct TSurf_0
{
    float3 albedo_1;
    float3 n_1;
    float rough_0;
    float ao_2;
};

float t_hash2_0(int2 c_2)
{
    uint h_7 = (uint(c_2.x) * 2376512323U) ^ (uint(c_2.y) * 3625334849U);
    uint h_8 = (h_7 ^ (h_7 >> 13U)) * 2246822507U;
    return float((h_8 ^ (h_8 >> 16U)) & 65535U) / 65535.0f;
}

float t_noise_0(float2 w_2, float cell_0)
{
    float2 q_4 = w_2 / float2(cell_0) ;
    float2 i_15 = floor(q_4);
    float2 f_1 = q_4 - i_15;
    float2 f_2 = f_1 * f_1 * (float2(3.0f)  - float2(2.0f)  * f_1);
    int per_0 = int(4096.0f / cell_0 + 0.5f);
    int2 _S172 = int2(i_15);
    int _S173 = _S172.x % per_0;
    int _S174 = (_S173 + per_0) % per_0;
    int _S175 = _S172.y % per_0;
    int _S176 = (_S175 + per_0) % per_0;
    int2 c0_0 = int2(_S174, _S176);
    int _S177 = (_S174 + int(1)) % per_0;
    int _S178 = (_S176 + int(1)) % per_0;
    float _S179 = f_2.x;
    return mix(mix(t_hash2_0(c0_0), t_hash2_0(int2(_S177, _S176)), _S179), mix(t_hash2_0(int2(_S174, _S178)), t_hash2_0(int2(_S177, _S178)), _S179), f_2.y);
}

struct TProj_0
{
    float2 w_3;
    float2 dx_0;
    float2 dy_0;
};

TProj_0 TProj_x24init_0(float2 w_4, float2 dx_1, float2 dy_1)
{
    thread TProj_0 _S180;
    (&_S180)->w_3 = w_4;
    (&_S180)->dx_0 = dx_1;
    (&_S180)->dy_0 = dy_1;
    return _S180;
}

float4 t_layer_albedo_0(const TerrainMat_natural_0 thread* m_2, sampler smp_1, uint layer_0, const TProj_0 thread* p_6, float near_0, KernelContext_0 thread* kernelContext_22)
{
    float k_5 = 1.0f / m_2->tile_0[layer_0];
    texture2d_array<float, access::sample> _S181 = ((reinterpret_cast<device texture2d_array<float, access::sample>*>((kernelContext_22->ed_textures_0))[(m_2->albedo_0)]));
    float _S182 = p_6->w_3.x;
    float _S183 = - p_6->w_3.y;
    float2 _S184 = float2(k_5) ;
    float _S185 = float(layer_0);
    float3 _S186 = float3(float2(_S182, _S183) * _S184, _S185);
    float _S187 = p_6->dx_0.x;
    float _S188 = - p_6->dx_0.y;
    float _S189 = p_6->dy_0.x;
    float _S190 = - p_6->dy_0.y;
    float4 a_5 = ((_S181).sample((smp_1), ((_S186)).xy, uint(((_S186)).z), gradient2d((float2(_S187, _S188) * _S184), (float2(_S189, _S190) * _S184))));
    float2 _S191 = float2((0.25f * k_5)) ;
    float3 _S192 = float3(float2(_S183, - _S182) * _S191, _S185);
    float4 b_4 = ((_S181).sample((smp_1), ((_S192)).xy, uint(((_S192)).z), gradient2d((float2(_S188, - _S187) * _S191), (float2(_S190, - _S189) * _S191))));
    float mixk_0 = mix(0.34999999403953552f, 0.75f, 1.0f - near_0);
    return float4(mix(a_5.xyz, b_4.xyz, float3(mixk_0) ), mix(a_5.w, b_4.w, 0.5f * mixk_0));
}

float4 t_layer_normal_0(const TerrainMat_natural_0 thread* m_3, sampler smp_2, uint layer_1, const TProj_0 thread* p_7, KernelContext_0 thread* kernelContext_23)
{
    float k_6 = 1.0f / m_3->tile_0[layer_1];
    texture2d_array<float, access::sample> _S193 = ((reinterpret_cast<device texture2d_array<float, access::sample>*>((kernelContext_23->ed_textures_0))[(m_3->normal_0)]));
    float2 _S194 = float2(k_6) ;
    float3 _S195 = float3(float2(p_7->w_3.x, - p_7->w_3.y) * _S194, float(layer_1));
    float4 b_5 = ((_S193).sample((smp_2), ((_S195)).xy, uint(((_S195)).z), gradient2d((float2(p_7->dx_0.x, - p_7->dx_0.y) * _S194), (float2(p_7->dy_0.x, - p_7->dy_0.y) * _S194))));
    return float4(b_5.xy * float2(2.0f)  - float2(1.0f) , b_5.z, b_5.w);
}

TSurf_0 terrain_material_0(FrameData_natural_0 device* f_3, const TerrainMat_natural_0 thread* m_4, const TerrainDraw_natural_0 thread* d_18, float3 n_2, float3 nm_0, float3 up_2, float h_9, float curv_0, float curv_f_0, float2 w_5, float2 dwx_0, float2 dwy_0, float dhx_0, float dhy_0, float3 tu_3, float3 tv_4, float dist_0, bool trees_0, float4 cv_0, float spacing_0, KernelContext_0 thread* kernelContext_24)
{
    float4 _S196;
    float best_0;
    bool _S197;
    float4 nr_0;
    float3 pert_0;
    float3 nw_0;
    sampler _S198 = ((reinterpret_cast<device sampler*>((kernelContext_24->ed_samplers_0))[(kernelContext_24->ed_c_0->smp_0)]));
    float cs_0 = dot(nm_0, up_2);
    float cf_0 = dot(n_2, up_2);
    float3 _S199 = (float4(d_18->polar_0) ).xyz;
    float3 north_0 = normalize(_S199 - up_2 * float3(dot(_S199, up_2)) );
    float3 nh_0 = nm_0 - up_2 * float3(cs_0) ;
    float northness_0;
    if((length_squared_0(nh_0)) > 9.99999997475242708e-07f)
    {
        northness_0 = dot(normalize(nh_0), north_0);
    }
    else
    {
        northness_0 = 0.0f;
    }
    float n1_0 = t_noise_0(w_5, 16.0f);
    float n2_0 = t_noise_0(w_5 + float2(37.0f) , 64.0f);
    float n3_0 = t_noise_0(w_5 + float2(911.0f) , 256.0f);
    float hollow_0 = saturate(curv_0 * 60.0f);
    float hollow_f_0 = saturate(curv_f_0 * 4.0f);
    float _S200 = n3_0 - 0.5f;
    float treeline_0 = 2150.0f + 120.0f * _S200 - 100.0f * northness_0;
    float _S201 = n2_0 - 0.5f;
    float snowline_0 = 3050.0f - 230.0f * northness_0 + 200.0f * _S201;
    float alpine_0 = smoothstep(treeline_0 - 150.0f, treeline_0 + 250.0f, h_9);
    float rock_cs_0 = mix(0.68000000715255737f, 0.76999998092651367f, alpine_0);
    float steep_0 = smoothstep(rock_cs_0, rock_cs_0 - 0.14000000059604645f, cs_0);
    float _S202 = n1_0 - 0.5f;
    float rock_0 = saturate(saturate(max(steep_0, smoothstep(rock_cs_0 - 0.059999998658895493f, rock_cs_0 - 0.20000000298023224f, cf_0) * 0.80000001192092896f) + saturate(- curv_0 * 60.0f) * 0.5f * alpine_0 + smoothstep(2700.0f, 3300.0f, h_9) * 0.44999998807907104f - 0.25f * hollow_0) + _S202 * 0.5f * smoothstep(0.92000001668930054f, 0.80000001192092896f, cs_0));
    float _S203 = _S201 * 0.80000001192092896f;
    float scree_0 = saturate(smoothstep(0.97000002861022949f, 0.87999999523162842f, cs_0) * smoothstep(0.6600000262260437f, 0.80000001192092896f, cs_0) * (0.10000000149011612f + hollow_0 * 1.2000000476837158f + _S203) * mix(0.10000000149011612f, 1.0f, alpine_0) + steep_0 * 0.30000001192092896f * alpine_0);
    float forest_0 = (1.0f - alpine_0) * smoothstep(0.99000000953674316f, 0.94999998807907104f, cs_0) * saturate(0.89999997615814209f + _S200 * 1.3999999761581421f + _S203);
    float forest_1;
    if(trees_0)
    {
        float _S204 = cv_0.x;
        forest_1 = mix(forest_0, mix(_S204, smoothstep(0.029999999329447746f, 0.5f, _S204), smoothstep(80.0f, 20.0f, spacing_0)), cv_0.w);
    }
    else
    {
        forest_1 = forest_0;
    }
    float grass_1 = (1.0f - forest_1) * smoothstep(3000.0f, 2600.0f, h_9 + _S202 * 200.0f);
    float soil_0 = saturate(hollow_f_0 * 0.80000001192092896f + (n1_0 - 0.64999997615814209f) * 1.5f) * (1.0f - alpine_0 * 0.5f);
    float snow_0 = smoothstep(snowline_0 - 120.0f, snowline_0 + 120.0f, h_9 + hollow_0 * 150.0f + _S202 * 120.0f) * smoothstep(0.62000000476837158f, 0.77999997138977051f, cf_0 + _S202 * 0.10000000149011612f);
    thread array<float, int(6)> wt_0;
    wt_0[0U] = rock_0;
    wt_0[1U] = scree_0 * (1.0f - rock_0 * 0.69999998807907104f);
    float _S205 = 1.0f - rock_0;
    float _S206 = 1.0f - scree_0 * 0.80000001192092896f;
    wt_0[2U] = grass_1 * _S205 * _S206;
    wt_0[3U] = forest_1 * _S205 * _S206;
    float _S207 = 1.0f - snow_0;
    wt_0[5U] = soil_0 * 0.5f * _S205 * _S207;
    wt_0[4U] = snow_0 * 1.6000000238418579f;
    wt_0[0U] = wt_0[0U] * _S207;
    wt_0[1U] = wt_0[1U] * _S207;
    wt_0[2U] = wt_0[2U] * _S207;
    wt_0[3U] = wt_0[3U] * _S207;
    wt_0[5U] = wt_0[5U] * _S207;
    float near_1 = saturate(1.0f - dist_0 / 400.0f);
    float3 _S208 = pow(abs(float3(dot(n_2, tu_3), dot(n_2, tv_4), cf_0)), float3(4.0f) );
    thread float3 tw_0 = _S208;
    float _S209 = _S208.x;
    float _S210 = _S208.y;
    float major_0 = max(_S209, _S210) - min(_S209, _S210);
    float2 _S211;
    if(_S209 > _S210)
    {
        _S211 = float2(major_0, 0.0f);
    }
    else
    {
        _S211 = float2(0.0f, major_0);
    }
    tw_0.xy = _S211;
    float3 _S212 = float3(0.0f) ;
    float3 _S213 = max(tw_0 - float3(0.029999999329447746f) , _S212);
    tw_0 = _S213;
    tw_0.z = max(_S213.z, 0.0099999997764825821f);
    float3 _S214 = tw_0 / float3(max(tw_0.x + tw_0.y + tw_0.z, 9.9999997473787516e-05f)) ;
    tw_0 = _S214;
    TProj_0 _S215 = TProj_x24init_0(w_5, dwx_0, dwy_0);
    bool side_u_0 = (_S214.y) >= (_S214.x);
    if(side_u_0)
    {
        northness_0 = tw_0.y;
    }
    else
    {
        northness_0 = tw_0.x;
    }
    thread TProj_0 side_0;
    if(side_u_0)
    {
        _S211 = float2(w_5.x, h_9);
    }
    else
    {
        _S211 = float2(w_5.y, h_9);
    }
    (&side_0)->w_3 = _S211;
    if(side_u_0)
    {
        _S211 = float2(dwx_0.x, dhx_0);
    }
    else
    {
        _S211 = float2(dwx_0.y, dhx_0);
    }
    (&side_0)->dx_0 = _S211;
    if(side_u_0)
    {
        _S211 = float2(dwy_0.x, dhy_0);
    }
    else
    {
        _S211 = float2(dwy_0.y, dhy_0);
    }
    (&side_0)->dy_0 = _S211;
    float3 _S216;
    if(side_u_0)
    {
        _S216 = tu_3;
    }
    else
    {
        _S216 = tv_4;
    }
    thread array<float4, int(6)> ah_0;
    for(;;)
    {
        for(;;)
        {
            for(;;)
            {
                float4 _S217 = float4(0.0f) ;
                _S196 = _S217;
                ah_0[0U] = _S217;
                if((wt_0[0U]) < 0.039999999105930328f)
                {
                    wt_0[0U] = 0.0f;
                    best_0 = 0.0f;
                    break;
                }
                bool _S218 = northness_0 > 0.0f;
                if((tw_0.z) > 0.0f)
                {
                    _S197 = true;
                }
                else
                {
                    _S197 = !_S218;
                }
                if(_S197)
                {
                    thread TProj_0 _S219 = _S215;
                    float4 _S220 = t_layer_albedo_0(m_4, _S198, 0U, &_S219, near_1, kernelContext_24);
                    nr_0 = _S220;
                }
                else
                {
                    nr_0 = _S217;
                }
                if(_S218)
                {
                    float4 _S221 = nr_0 * float4(tw_0.z) ;
                    thread TProj_0 _S222 = side_0;
                    float4 _S223 = t_layer_albedo_0(m_4, _S198, 0U, &_S222, near_1, kernelContext_24);
                    nr_0 = _S221 + _S223 * float4(northness_0) ;
                }
                ah_0[0U] = nr_0;
                float _S224 = wt_0[0U] + nr_0.w * mix(0.15000000596046448f, 0.60000002384185791f, near_1);
                wt_0[0U] = _S224;
                best_0 = max(0.0f, _S224);
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                ah_0[1U] = _S196;
                if((wt_0[1U]) < 0.039999999105930328f)
                {
                    wt_0[1U] = 0.0f;
                    break;
                }
                thread TProj_0 _S225 = _S215;
                float4 _S226 = t_layer_albedo_0(m_4, _S198, 1U, &_S225, near_1, kernelContext_24);
                ah_0[1U] = _S226;
                float _S227 = wt_0[1U] + _S226.w * mix(0.15000000596046448f, 0.60000002384185791f, near_1);
                wt_0[1U] = _S227;
                best_0 = max(best_0, _S227);
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                ah_0[2U] = _S196;
                if((wt_0[2U]) < 0.039999999105930328f)
                {
                    wt_0[2U] = 0.0f;
                    break;
                }
                thread TProj_0 _S228 = _S215;
                float4 _S229 = t_layer_albedo_0(m_4, _S198, 2U, &_S228, near_1, kernelContext_24);
                ah_0[2U] = _S229;
                float _S230 = wt_0[2U] + _S229.w * mix(0.15000000596046448f, 0.60000002384185791f, near_1);
                wt_0[2U] = _S230;
                best_0 = max(best_0, _S230);
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                ah_0[3U] = _S196;
                if((wt_0[3U]) < 0.039999999105930328f)
                {
                    wt_0[3U] = 0.0f;
                    break;
                }
                thread TProj_0 _S231 = _S215;
                float4 _S232 = t_layer_albedo_0(m_4, _S198, 3U, &_S231, near_1, kernelContext_24);
                ah_0[3U] = _S232;
                float _S233 = wt_0[3U] + _S232.w * mix(0.15000000596046448f, 0.60000002384185791f, near_1);
                wt_0[3U] = _S233;
                best_0 = max(best_0, _S233);
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                ah_0[4U] = _S196;
                if((wt_0[4U]) < 0.039999999105930328f)
                {
                    wt_0[4U] = 0.0f;
                    break;
                }
                bool _S234 = northness_0 > 0.0f;
                if((tw_0.z) > 0.0f)
                {
                    _S197 = true;
                }
                else
                {
                    _S197 = !_S234;
                }
                if(_S197)
                {
                    thread TProj_0 _S235 = _S215;
                    float4 _S236 = t_layer_albedo_0(m_4, _S198, 4U, &_S235, near_1, kernelContext_24);
                    nr_0 = _S236;
                }
                else
                {
                    nr_0 = _S196;
                }
                if(_S234)
                {
                    float4 _S237 = nr_0 * float4(tw_0.z) ;
                    thread TProj_0 _S238 = side_0;
                    float4 _S239 = t_layer_albedo_0(m_4, _S198, 4U, &_S238, near_1, kernelContext_24);
                    nr_0 = _S237 + _S239 * float4(northness_0) ;
                }
                ah_0[4U] = nr_0;
                float _S240 = wt_0[4U] + nr_0.w * mix(0.15000000596046448f, 0.60000002384185791f, near_1);
                wt_0[4U] = _S240;
                best_0 = max(best_0, _S240);
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                ah_0[5U] = _S196;
                if((wt_0[5U]) < 0.039999999105930328f)
                {
                    wt_0[5U] = 0.0f;
                    break;
                }
                thread TProj_0 _S241 = _S215;
                float4 _S242 = t_layer_albedo_0(m_4, _S198, 5U, &_S241, near_1, kernelContext_24);
                ah_0[5U] = _S242;
                float _S243 = wt_0[5U] + _S242.w * mix(0.15000000596046448f, 0.60000002384185791f, near_1);
                wt_0[5U] = _S243;
                best_0 = max(best_0, _S243);
                break;
            }
            break;
        }
        break;
    }
    if((wt_0[0U]) > 0.0f)
    {
        forest_1 = max(wt_0[0U] - (best_0 - 0.34999999403953552f), 0.0f);
    }
    else
    {
        forest_1 = 0.0f;
    }
    wt_0[0U] = forest_1;
    float s_9;
    if((wt_0[1U]) > 0.0f)
    {
        s_9 = max(wt_0[1U] - (best_0 - 0.34999999403953552f), 0.0f);
    }
    else
    {
        s_9 = 0.0f;
    }
    wt_0[1U] = s_9;
    float total_0 = forest_1 + s_9;
    if((wt_0[2U]) > 0.0f)
    {
        forest_1 = max(wt_0[2U] - (best_0 - 0.34999999403953552f), 0.0f);
    }
    else
    {
        forest_1 = 0.0f;
    }
    wt_0[2U] = forest_1;
    float total_1 = total_0 + forest_1;
    if((wt_0[3U]) > 0.0f)
    {
        forest_1 = max(wt_0[3U] - (best_0 - 0.34999999403953552f), 0.0f);
    }
    else
    {
        forest_1 = 0.0f;
    }
    wt_0[3U] = forest_1;
    float total_2 = total_1 + forest_1;
    if((wt_0[4U]) > 0.0f)
    {
        forest_1 = max(wt_0[4U] - (best_0 - 0.34999999403953552f), 0.0f);
    }
    else
    {
        forest_1 = 0.0f;
    }
    wt_0[4U] = forest_1;
    float total_3 = total_2 + forest_1;
    if((wt_0[5U]) > 0.0f)
    {
        forest_1 = max(wt_0[5U] - (best_0 - 0.34999999403953552f), 0.0f);
    }
    else
    {
        forest_1 = 0.0f;
    }
    wt_0[5U] = forest_1;
    float total_4 = total_3 + forest_1;
    thread TSurf_0 o_0;
    (&o_0)->albedo_1 = _S212;
    (&o_0)->n_1 = _S212;
    (&o_0)->rough_0 = 0.0f;
    (&o_0)->ao_2 = 0.0f;
    for(;;)
    {
        for(;;)
        {
            for(;;)
            {
                if((wt_0[0U]) <= 0.0f)
                {
                    pert_0 = _S212;
                    break;
                }
                float a_6 = wt_0[0U] / max(total_4, 9.9999997473787516e-06f);
                float3 _S244 = float3(a_6) ;
                (&o_0)->albedo_1 = (&o_0)->albedo_1 + _S244 * ah_0[0U].xyz * (float4((&m_4->gain_0)->data_6[0U]) ).xyz;
                float4 _S245 = float4(0.0f, 0.0f, 0.92000001668930054f, 1.0f);
                if(near_1 > 0.0f)
                {
                    bool _S246 = northness_0 > 0.0f;
                    if((tw_0.z) > 0.0f)
                    {
                        _S197 = true;
                    }
                    else
                    {
                        _S197 = !_S246;
                    }
                    if(_S197)
                    {
                        thread TProj_0 _S247 = _S215;
                        float4 _S248 = t_layer_normal_0(m_4, _S198, 0U, &_S247, kernelContext_24);
                        if(_S246)
                        {
                            s_9 = tw_0.z;
                        }
                        else
                        {
                            s_9 = 1.0f;
                        }
                        float3 _S249 = (tu_3 * float3(_S248.x)  + tv_4 * float3(_S248.y) ) * float3(s_9) ;
                        nr_0 = _S248 * float4(s_9) ;
                        pert_0 = _S249;
                    }
                    else
                    {
                        nr_0 = _S196;
                        pert_0 = _S212;
                    }
                    if(_S246)
                    {
                        thread TProj_0 _S250 = side_0;
                        float4 _S251 = t_layer_normal_0(m_4, _S198, 0U, &_S250, kernelContext_24);
                        float4 nr_1 = nr_0 + _S251 * float4(northness_0) ;
                        pert_0 = pert_0 + (_S216 * float3(_S251.x)  + up_2 * float3(_S251.y) ) * float3(northness_0) ;
                        nr_0 = nr_1;
                    }
                }
                else
                {
                    pert_0 = _S212;
                    nr_0 = _S245;
                }
                float3 _S252 = _S244 * pert_0;
                (&o_0)->rough_0 = (&o_0)->rough_0 + a_6 * nr_0.z;
                (&o_0)->ao_2 = (&o_0)->ao_2 + a_6 * nr_0.w;
                pert_0 = _S252;
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                if((wt_0[1U]) <= 0.0f)
                {
                    break;
                }
                float a_7 = wt_0[1U] / max(total_4, 9.9999997473787516e-06f);
                float3 _S253 = float3(a_7) ;
                (&o_0)->albedo_1 = (&o_0)->albedo_1 + _S253 * ah_0[1U].xyz * (float4((&m_4->gain_0)->data_6[1U]) ).xyz;
                float4 _S254 = float4(0.0f, 0.0f, 0.92000001668930054f, 1.0f);
                if(near_1 > 0.0f)
                {
                    thread TProj_0 _S255 = _S215;
                    float4 _S256 = t_layer_normal_0(m_4, _S198, 1U, &_S255, kernelContext_24);
                    nw_0 = tu_3 * float3(_S256.x)  + tv_4 * float3(_S256.y) ;
                    nr_0 = _S256;
                }
                else
                {
                    nw_0 = _S212;
                    nr_0 = _S254;
                }
                float3 pert_1 = pert_0 + _S253 * nw_0;
                (&o_0)->rough_0 = (&o_0)->rough_0 + a_7 * nr_0.z;
                (&o_0)->ao_2 = (&o_0)->ao_2 + a_7 * nr_0.w;
                pert_0 = pert_1;
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                if((wt_0[2U]) <= 0.0f)
                {
                    break;
                }
                float a_8 = wt_0[2U] / max(total_4, 9.9999997473787516e-06f);
                float3 _S257 = float3(a_8) ;
                (&o_0)->albedo_1 = (&o_0)->albedo_1 + _S257 * ah_0[2U].xyz * (float4((&m_4->gain_0)->data_6[2U]) ).xyz;
                float4 _S258 = float4(0.0f, 0.0f, 0.92000001668930054f, 1.0f);
                if(near_1 > 0.0f)
                {
                    thread TProj_0 _S259 = _S215;
                    float4 _S260 = t_layer_normal_0(m_4, _S198, 2U, &_S259, kernelContext_24);
                    nw_0 = tu_3 * float3(_S260.x)  + tv_4 * float3(_S260.y) ;
                    nr_0 = _S260;
                }
                else
                {
                    nw_0 = _S212;
                    nr_0 = _S258;
                }
                float3 pert_2 = pert_0 + _S257 * nw_0;
                (&o_0)->rough_0 = (&o_0)->rough_0 + a_8 * nr_0.z;
                (&o_0)->ao_2 = (&o_0)->ao_2 + a_8 * nr_0.w;
                pert_0 = pert_2;
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                if((wt_0[3U]) <= 0.0f)
                {
                    break;
                }
                float a_9 = wt_0[3U] / max(total_4, 9.9999997473787516e-06f);
                float3 _S261 = float3(a_9) ;
                (&o_0)->albedo_1 = (&o_0)->albedo_1 + _S261 * ah_0[3U].xyz * (float4((&m_4->gain_0)->data_6[3U]) ).xyz;
                float4 _S262 = float4(0.0f, 0.0f, 0.92000001668930054f, 1.0f);
                if(near_1 > 0.0f)
                {
                    thread TProj_0 _S263 = _S215;
                    float4 _S264 = t_layer_normal_0(m_4, _S198, 3U, &_S263, kernelContext_24);
                    nw_0 = tu_3 * float3(_S264.x)  + tv_4 * float3(_S264.y) ;
                    nr_0 = _S264;
                }
                else
                {
                    nw_0 = _S212;
                    nr_0 = _S262;
                }
                float3 pert_3 = pert_0 + _S261 * nw_0;
                (&o_0)->rough_0 = (&o_0)->rough_0 + a_9 * nr_0.z;
                (&o_0)->ao_2 = (&o_0)->ao_2 + a_9 * nr_0.w;
                pert_0 = pert_3;
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                if((wt_0[4U]) <= 0.0f)
                {
                    break;
                }
                float a_10 = wt_0[4U] / max(total_4, 9.9999997473787516e-06f);
                float3 _S265 = float3(a_10) ;
                (&o_0)->albedo_1 = (&o_0)->albedo_1 + _S265 * ah_0[4U].xyz * (float4((&m_4->gain_0)->data_6[4U]) ).xyz;
                float4 _S266 = float4(0.0f, 0.0f, 0.92000001668930054f, 1.0f);
                if(near_1 > 0.0f)
                {
                    bool _S267 = northness_0 > 0.0f;
                    if((tw_0.z) > 0.0f)
                    {
                        _S197 = true;
                    }
                    else
                    {
                        _S197 = !_S267;
                    }
                    if(_S197)
                    {
                        thread TProj_0 _S268 = _S215;
                        float4 _S269 = t_layer_normal_0(m_4, _S198, 4U, &_S268, kernelContext_24);
                        if(_S267)
                        {
                            s_9 = tw_0.z;
                        }
                        else
                        {
                            s_9 = 1.0f;
                        }
                        float3 _S270 = (tu_3 * float3(_S269.x)  + tv_4 * float3(_S269.y) ) * float3(s_9) ;
                        nr_0 = _S269 * float4(s_9) ;
                        nw_0 = _S270;
                    }
                    else
                    {
                        nr_0 = _S196;
                        nw_0 = _S212;
                    }
                    if(_S267)
                    {
                        thread TProj_0 _S271 = side_0;
                        float4 _S272 = t_layer_normal_0(m_4, _S198, 4U, &_S271, kernelContext_24);
                        float4 nr_2 = nr_0 + _S272 * float4(northness_0) ;
                        nw_0 = nw_0 + (_S216 * float3(_S272.x)  + up_2 * float3(_S272.y) ) * float3(northness_0) ;
                        nr_0 = nr_2;
                    }
                }
                else
                {
                    nw_0 = _S212;
                    nr_0 = _S266;
                }
                float3 pert_4 = pert_0 + _S265 * nw_0;
                (&o_0)->rough_0 = (&o_0)->rough_0 + a_10 * nr_0.z;
                (&o_0)->ao_2 = (&o_0)->ao_2 + a_10 * nr_0.w;
                pert_0 = pert_4;
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                if((wt_0[5U]) <= 0.0f)
                {
                    break;
                }
                float a_11 = wt_0[5U] / max(total_4, 9.9999997473787516e-06f);
                float3 _S273 = float3(a_11) ;
                (&o_0)->albedo_1 = (&o_0)->albedo_1 + _S273 * ah_0[5U].xyz * (float4((&m_4->gain_0)->data_6[5U]) ).xyz;
                float4 _S274 = float4(0.0f, 0.0f, 0.92000001668930054f, 1.0f);
                if(near_1 > 0.0f)
                {
                    thread TProj_0 _S275 = _S215;
                    float4 _S276 = t_layer_normal_0(m_4, _S198, 5U, &_S275, kernelContext_24);
                    nw_0 = tu_3 * float3(_S276.x)  + tv_4 * float3(_S276.y) ;
                    nr_0 = _S276;
                }
                else
                {
                    nw_0 = _S212;
                    nr_0 = _S274;
                }
                float3 pert_5 = pert_0 + _S273 * nw_0;
                (&o_0)->rough_0 = (&o_0)->rough_0 + a_11 * nr_0.z;
                (&o_0)->ao_2 = (&o_0)->ao_2 + a_11 * nr_0.w;
                pert_0 = pert_5;
                break;
            }
            break;
        }
        break;
    }
    if(total_4 <= 0.0f)
    {
        (&o_0)->albedo_1 = float3(0.125f, 0.14000000059604645f, 0.070000000298023224f);
        (&o_0)->rough_0 = 0.92000001668930054f;
        (&o_0)->ao_2 = 1.0f;
    }
    (&o_0)->albedo_1 = (&o_0)->albedo_1 * float3((mix(0.81999999284744263f, 1.1499999761581421f, n2_0) * mix(0.89999997615814209f, 1.0800000429153442f, n3_0) * mix(1.0f, 0.80000001192092896f, hollow_f_0 * _S207)))  * mix(float3(1.0f) , float3(1.0399999618530273f, 1.0f, 0.93999999761581421f), float3((n1_0 * _S207)) );
    float _S277 = wt_0[3U] / max(total_4, 9.9999997473787516e-06f) * smoothstep(60.0f, 600.0f, dist_0);
    if(trees_0)
    {
        northness_0 = 1.0f - cv_0.w;
    }
    else
    {
        northness_0 = 1.0f;
    }
    (&o_0)->albedo_1 = mix((&o_0)->albedo_1, float3(0.029999999329447746f, 0.043999999761581421f, 0.020999999716877937f) * float3(mix(0.85000002384185791f, 1.1499999761581421f, n1_0)) , float3((_S277 * northness_0)) );
    float ns_0 = mix(0.34999999403953552f, 1.0f, near_1);
    (&o_0)->n_1 = normalize(n_2 + pert_0 * float3(ns_0) );
    (&o_0)->ao_2 = mix(1.0f, (&o_0)->ao_2, ns_0);
    return o_0;
}

struct TForest_0
{
    float3 albedo_2;
    float3 n_3;
    float ao_3;
    float sun_1;
    float crown_0;
};

struct LeafSeason_0
{
    float leaves_0;
    float green_0;
    float colour_0;
    float evergreen_0;
    float3 target_0;
};

LeafSeason_0 season_off_0()
{
    thread LeafSeason_0 s_10;
    (&s_10)->leaves_0 = 1.0f;
    (&s_10)->green_0 = 1.0f;
    (&s_10)->colour_0 = 0.0f;
    (&s_10)->evergreen_0 = 0.0f;
    (&s_10)->target_0 = float3(0.0f, 0.0f, 0.0f);
    return s_10;
}

float season_ramp_0(float lo_1, float hi_0, float x_0)
{
    return smoothstep(lo_1, hi_0, x_0);
}

LeafSeason_0 season_state_0(FrameData_natural_0 device* f_4, uint row_0, float shift_0, bool south_0)
{
    if(((float4((&f_4->season_0)->cfg_4) ).x) <= 0.0f)
    {
        return season_off_0();
    }
    float d_19 = (float4((&f_4->season_0)->cfg_4) ).x;
    float d_20;
    if(south_0)
    {
        d_20 = fmod(d_19 + 182.625f, 365.25f);
    }
    else
    {
        d_20 = d_19;
    }
    float ds_0 = d_20 - shift_0;
    float da_0 = d_20 + (float4((&f_4->season_0)->cfg_4) ).y * shift_0;
    uint _S278 = (row_0 & 31U) * 2U;
    float4 _S279 = float4((&(&f_4->season_0)->rows_0)->data_5[_S278]) ;
    float4 _S280 = float4((&(&f_4->season_0)->rows_0)->data_5[_S278 + 1U]) ;
    thread LeafSeason_0 s_11;
    (&s_11)->target_0 = _S280.xyz;
    float _S281 = _S279.w;
    (&s_11)->evergreen_0 = _S281;
    if(_S281 > 0.5f)
    {
        float _S282 = _S279.y;
        float a_12 = season_ramp_0(_S282 - 10.0f, _S282 + 30.0f, da_0);
        float _S283 = _S279.x;
        float rel_1 = 1.0f - season_ramp_0(_S283 - 10.0f, _S283 + 20.0f, ds_0);
        (&s_11)->green_0 = 1.0f;
        (&s_11)->leaves_0 = 1.0f;
        (&s_11)->colour_0 = _S280.w * max(a_12, rel_1);
    }
    else
    {
        float _S284 = _S279.x;
        float _S285 = season_ramp_0(_S284 - 6.0f, _S284 + 8.0f, ds_0);
        (&s_11)->green_0 = _S285;
        float _S286 = _S279.y;
        (&s_11)->colour_0 = season_ramp_0(_S286 - 8.0f, _S286 + 14.0f, da_0);
        float _S287 = _S279.z;
        (&s_11)->leaves_0 = _S285 * (1.0f - season_ramp_0(_S287 - 10.0f, _S287 + 10.0f, da_0));
    }
    return s_11;
}

LeafSeason_0 season_far_0(FrameData_natural_0 device* f_5, bool conifer_0, float alt_m_0)
{
    float _S288;
    if(conifer_0)
    {
        _S288 = (float4((&f_5->season_0)->cfg_4) ).w;
    }
    else
    {
        _S288 = (float4((&f_5->season_0)->cfg_4) ).z;
    }
    LeafSeason_0 _S289 = season_state_0(f_5, uint(_S288), clamp(4.0f * (alt_m_0 - 500.0f) / 122.0f, -60.0f, 75.0f), false);
    return _S289;
}

float3 season_tint_0(const LeafSeason_0 thread* s_12, float3 alb_0)
{
    bool _S290;
    if((s_12->leaves_0) >= 1.0f)
    {
        _S290 = (s_12->colour_0) <= 0.0f;
    }
    else
    {
        _S290 = false;
    }
    if(_S290)
    {
        _S290 = (s_12->green_0) >= 1.0f;
    }
    else
    {
        _S290 = false;
    }
    if(_S290)
    {
        return alb_0;
    }
    float lum_0 = dot(alb_0, float3(0.2125999927520752f, 0.71520000696182251f, 0.072200000286102295f));
    float3 a_13;
    if((s_12->evergreen_0) < 0.5f)
    {
        a_13 = alb_0 * mix(float3(1.4500000476837158f, 1.2999999523162842f, 0.64999997615814209f), float3(1.0f, 1.0f, 1.0f), float3(s_12->green_0) );
    }
    else
    {
        a_13 = alb_0;
    }
    return mix(a_13, s_12->target_0 * float3(clamp(lum_0 * 5.0f, 0.44999998807907104f, 1.5f)) , float3(s_12->colour_0) );
}

float4 t_crown_0(float2 w_6, float cell_1, float pres_0, float rad_0, uint salt_0)
{
    int _S291;
    int2 _S292;
    float4 best_1;
    float2 x_1 = w_6 / float2(cell_1) ;
    float2 _S293 = floor(x_1);
    int _S294 = int(4096.0f / cell_1 + 0.5f);
    float4 _S295 = float4(0.0f, 0.0f, 9.0f, 0.0f);
    for(;;)
    {
        for(;;)
        {
            for(;;)
            {
                float2 c_3 = _S293 + float2(-1.0f, -1.0f);
                int2 _S296 = int2(c_3);
                int _S297 = _S296.x % _S294;
                int _S298 = (_S297 + _S294) % _S294;
                int _S299 = _S296.y % _S294;
                int _S300 = (_S299 + _S294) % _S294;
                int2 ci_0 = int2(_S298, _S300);
                int _S301 = int(salt_0);
                _S291 = _S301;
                int2 _S302 = int2(_S301, int(0));
                _S292 = _S302;
                if((t_hash2_0(ci_0 + _S302)) >= pres_0)
                {
                    best_1 = _S295;
                    break;
                }
                float h1_0 = t_hash2_0(ci_0 + int2(int(17), _S301 + int(5)));
                float2 _S303 = float2(0.5f) ;
                float2 q_5 = (x_1 - (c_3 + _S303 + (float2(h1_0, t_hash2_0(ci_0 + int2(_S301 + int(3), int(7)))) - _S303) * float2(0.80000001192092896f) )) / float2((rad_0 * mix(0.60000002384185791f, 1.3500000238418579f, t_hash2_0(ci_0 + int2(_S301 + int(9), int(31)))))) ;
                float dq_0 = length(q_5);
                if(dq_0 < 9.0f)
                {
                    best_1 = float4(q_5, dq_0, h1_0);
                }
                else
                {
                    best_1 = _S295;
                }
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                float2 c_4 = _S293 + float2(0.0f, -1.0f);
                int2 _S304 = int2(c_4);
                int _S305 = _S304.x % _S294;
                int _S306 = (_S305 + _S294) % _S294;
                int _S307 = _S304.y % _S294;
                int _S308 = (_S307 + _S294) % _S294;
                int2 ci_1 = int2(_S306, _S308);
                if((t_hash2_0(ci_1 + _S292)) >= pres_0)
                {
                    break;
                }
                float h1_1 = t_hash2_0(ci_1 + int2(int(17), _S291 + int(5)));
                float2 _S309 = float2(0.5f) ;
                float2 q_6 = (x_1 - (c_4 + _S309 + (float2(h1_1, t_hash2_0(ci_1 + int2(_S291 + int(3), int(7)))) - _S309) * float2(0.80000001192092896f) )) / float2((rad_0 * mix(0.60000002384185791f, 1.3500000238418579f, t_hash2_0(ci_1 + int2(_S291 + int(9), int(31)))))) ;
                float dq_1 = length(q_6);
                if(dq_1 < (best_1.z))
                {
                    best_1 = float4(q_6, dq_1, h1_1);
                }
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                float2 c_5 = _S293 + float2(1.0f, -1.0f);
                int2 _S310 = int2(c_5);
                int _S311 = _S310.x % _S294;
                int _S312 = (_S311 + _S294) % _S294;
                int _S313 = _S310.y % _S294;
                int _S314 = (_S313 + _S294) % _S294;
                int2 ci_2 = int2(_S312, _S314);
                if((t_hash2_0(ci_2 + _S292)) >= pres_0)
                {
                    break;
                }
                float h1_2 = t_hash2_0(ci_2 + int2(int(17), _S291 + int(5)));
                float2 _S315 = float2(0.5f) ;
                float2 q_7 = (x_1 - (c_5 + _S315 + (float2(h1_2, t_hash2_0(ci_2 + int2(_S291 + int(3), int(7)))) - _S315) * float2(0.80000001192092896f) )) / float2((rad_0 * mix(0.60000002384185791f, 1.3500000238418579f, t_hash2_0(ci_2 + int2(_S291 + int(9), int(31)))))) ;
                float dq_2 = length(q_7);
                if(dq_2 < (best_1.z))
                {
                    best_1 = float4(q_7, dq_2, h1_2);
                }
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                float2 c_6 = _S293 + float2(-1.0f, 0.0f);
                int2 _S316 = int2(c_6);
                int _S317 = _S316.x % _S294;
                int _S318 = (_S317 + _S294) % _S294;
                int _S319 = _S316.y % _S294;
                int _S320 = (_S319 + _S294) % _S294;
                int2 ci_3 = int2(_S318, _S320);
                if((t_hash2_0(ci_3 + _S292)) >= pres_0)
                {
                    break;
                }
                float h1_3 = t_hash2_0(ci_3 + int2(int(17), _S291 + int(5)));
                float2 _S321 = float2(0.5f) ;
                float2 q_8 = (x_1 - (c_6 + _S321 + (float2(h1_3, t_hash2_0(ci_3 + int2(_S291 + int(3), int(7)))) - _S321) * float2(0.80000001192092896f) )) / float2((rad_0 * mix(0.60000002384185791f, 1.3500000238418579f, t_hash2_0(ci_3 + int2(_S291 + int(9), int(31)))))) ;
                float dq_3 = length(q_8);
                if(dq_3 < (best_1.z))
                {
                    best_1 = float4(q_8, dq_3, h1_3);
                }
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                int2 _S322 = int2(_S293);
                int _S323 = _S322.x % _S294;
                int _S324 = (_S323 + _S294) % _S294;
                int _S325 = _S322.y % _S294;
                int _S326 = (_S325 + _S294) % _S294;
                int2 ci_4 = int2(_S324, _S326);
                if((t_hash2_0(ci_4 + _S292)) >= pres_0)
                {
                    break;
                }
                float h1_4 = t_hash2_0(ci_4 + int2(int(17), _S291 + int(5)));
                float2 _S327 = float2(0.5f) ;
                float2 q_9 = (x_1 - (_S293 + _S327 + (float2(h1_4, t_hash2_0(ci_4 + int2(_S291 + int(3), int(7)))) - _S327) * float2(0.80000001192092896f) )) / float2((rad_0 * mix(0.60000002384185791f, 1.3500000238418579f, t_hash2_0(ci_4 + int2(_S291 + int(9), int(31)))))) ;
                float dq_4 = length(q_9);
                if(dq_4 < (best_1.z))
                {
                    best_1 = float4(q_9, dq_4, h1_4);
                }
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                float2 c_7 = _S293 + float2(1.0f, 0.0f);
                int2 _S328 = int2(c_7);
                int _S329 = _S328.x % _S294;
                int _S330 = (_S329 + _S294) % _S294;
                int _S331 = _S328.y % _S294;
                int _S332 = (_S331 + _S294) % _S294;
                int2 ci_5 = int2(_S330, _S332);
                if((t_hash2_0(ci_5 + _S292)) >= pres_0)
                {
                    break;
                }
                float h1_5 = t_hash2_0(ci_5 + int2(int(17), _S291 + int(5)));
                float2 _S333 = float2(0.5f) ;
                float2 q_10 = (x_1 - (c_7 + _S333 + (float2(h1_5, t_hash2_0(ci_5 + int2(_S291 + int(3), int(7)))) - _S333) * float2(0.80000001192092896f) )) / float2((rad_0 * mix(0.60000002384185791f, 1.3500000238418579f, t_hash2_0(ci_5 + int2(_S291 + int(9), int(31)))))) ;
                float dq_5 = length(q_10);
                if(dq_5 < (best_1.z))
                {
                    best_1 = float4(q_10, dq_5, h1_5);
                }
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                float2 c_8 = _S293 + float2(-1.0f, 1.0f);
                int2 _S334 = int2(c_8);
                int _S335 = _S334.x % _S294;
                int _S336 = (_S335 + _S294) % _S294;
                int _S337 = _S334.y % _S294;
                int _S338 = (_S337 + _S294) % _S294;
                int2 ci_6 = int2(_S336, _S338);
                if((t_hash2_0(ci_6 + _S292)) >= pres_0)
                {
                    break;
                }
                float h1_6 = t_hash2_0(ci_6 + int2(int(17), _S291 + int(5)));
                float2 _S339 = float2(0.5f) ;
                float2 q_11 = (x_1 - (c_8 + _S339 + (float2(h1_6, t_hash2_0(ci_6 + int2(_S291 + int(3), int(7)))) - _S339) * float2(0.80000001192092896f) )) / float2((rad_0 * mix(0.60000002384185791f, 1.3500000238418579f, t_hash2_0(ci_6 + int2(_S291 + int(9), int(31)))))) ;
                float dq_6 = length(q_11);
                if(dq_6 < (best_1.z))
                {
                    best_1 = float4(q_11, dq_6, h1_6);
                }
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                float2 c_9 = _S293 + float2(0.0f, 1.0f);
                int2 _S340 = int2(c_9);
                int _S341 = _S340.x % _S294;
                int _S342 = (_S341 + _S294) % _S294;
                int _S343 = _S340.y % _S294;
                int _S344 = (_S343 + _S294) % _S294;
                int2 ci_7 = int2(_S342, _S344);
                if((t_hash2_0(ci_7 + _S292)) >= pres_0)
                {
                    break;
                }
                float h1_7 = t_hash2_0(ci_7 + int2(int(17), _S291 + int(5)));
                float2 _S345 = float2(0.5f) ;
                float2 q_12 = (x_1 - (c_9 + _S345 + (float2(h1_7, t_hash2_0(ci_7 + int2(_S291 + int(3), int(7)))) - _S345) * float2(0.80000001192092896f) )) / float2((rad_0 * mix(0.60000002384185791f, 1.3500000238418579f, t_hash2_0(ci_7 + int2(_S291 + int(9), int(31)))))) ;
                float dq_7 = length(q_12);
                if(dq_7 < (best_1.z))
                {
                    best_1 = float4(q_12, dq_7, h1_7);
                }
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                float2 c_10 = _S293 + float2(1.0f, 1.0f);
                int2 _S346 = int2(c_10);
                int _S347 = _S346.x % _S294;
                int _S348 = (_S347 + _S294) % _S294;
                int _S349 = _S346.y % _S294;
                int _S350 = (_S349 + _S294) % _S294;
                int2 ci_8 = int2(_S348, _S350);
                if((t_hash2_0(ci_8 + _S292)) >= pres_0)
                {
                    break;
                }
                float h1_8 = t_hash2_0(ci_8 + int2(int(17), _S291 + int(5)));
                float2 _S351 = float2(0.5f) ;
                float2 q_13 = (x_1 - (c_10 + _S351 + (float2(h1_8, t_hash2_0(ci_8 + int2(_S291 + int(3), int(7)))) - _S351) * float2(0.80000001192092896f) )) / float2((rad_0 * mix(0.60000002384185791f, 1.3500000238418579f, t_hash2_0(ci_8 + int2(_S291 + int(9), int(31)))))) ;
                float dq_8 = length(q_13);
                if(dq_8 < (best_1.z))
                {
                    best_1 = float4(q_13, dq_8, h1_8);
                }
                break;
            }
            break;
        }
        break;
    }
    return best_1;
}

TForest_0 t_forest_0(FrameData_natural_0 device* f_6, float alt_m_1, uint pool_17, const TerrainDraw_natural_0 thread* d_21, float4 cv_1, float2 g_5, float2 w_7, float3 tu_4, float3 tv_5, float3 up_3, float3 n_4, float3 floor_alb_0, float3 l_3, float px_m_0, float px_iso_0, float spacing_1, float near_px_0, KernelContext_0 thread* kernelContext_25)
{
    float _S352 = t_noise_0(w_7 + float2(911.0f) , 64.0f);
    float _S353 = _S352 * 0.64999997615814209f;
    float _S354 = t_noise_0(w_7 + float2(271.0f) , 16.0f);
    float _S355 = cv_1.x;
    float c_11 = saturate(_S355 + (_S353 + _S354 * 0.34999999403953552f - 0.5f) * 1.6000000238418579f * _S355 * (1.0f - _S355));
    float _S356 = max(cv_1.y, 2.0f);
    float mx_3 = cv_1.z;
    float3 crown_b_0;
    float3 crown_c_0;
    if(((float4((&f_6->season_0)->cfg_4) ).x) > 0.0f)
    {
        LeafSeason_0 _S357 = season_far_0(f_6, false, alt_m_1);
        LeafSeason_0 _S358 = season_far_0(f_6, true, alt_m_1);
        thread LeafSeason_0 _S359 = _S357;
        float3 _S360 = season_tint_0(&_S359, float3(0.05000000074505806f, 0.10999999940395355f, 0.024000000208616257f));
        float3 _S361 = mix(float3(0.034000001847743988f, 0.028000000864267349f, 0.02199999988079071f), _S360, float3(_S357.leaves_0) );
        thread LeafSeason_0 _S362 = _S358;
        float3 _S363 = season_tint_0(&_S362, float3(0.026000000536441803f, 0.064000003039836884f, 0.019999999552965164f));
        float3 _S364 = mix(float3(0.034000001847743988f, 0.028000000864267349f, 0.02199999988079071f), _S363, float3(_S358.leaves_0) );
        crown_b_0 = _S361;
        crown_c_0 = _S364;
    }
    else
    {
        crown_b_0 = float3(0.05000000074505806f, 0.10999999940395355f, 0.024000000208616257f);
        crown_c_0 = float3(0.026000000536441803f, 0.064000003039836884f, 0.019999999552965164f);
    }
    float3 leaf_0 = mix(crown_b_0, crown_c_0, float3(mx_3) );
    float n1_1 = t_noise_0(w_7 + float2(151.0f) , 32.0f);
    float n2_1 = t_noise_0(w_7 + float2(517.0f) , 128.0f);
    float stand_0 = mix(0.80000001192092896f, 1.2000000476837158f, n1_1) * mix(0.87999999523162842f, 1.1200000047683716f, n2_1);
    float2 sun2_0 = float2(dot(l_3, tu_4), dot(l_3, tv_5));
    float2 toward_0 = sun2_0 / float2(max(dot(l_3, up_3), 0.059999998658895493f)) ;
    float _S365 = max(_S356, 20.0f);
    float4 _S366 = t_canopy_bilin_0(pool_17, (as_type<uint>(((float4(d_21->tex_0) ).z))), g_5 * float2(4.0f)  + toward_0 * float2(_S365)  / float2(max(spacing_1, 0.0099999997764825821f)) , kernelContext_25);
    float page_sh_0 = _S366.x * smoothstep(0.40000000596046448f, 0.89999997615814209f, _S366.y / _S365);
    float3 fl_1 = mix(floor_alb_0, float3(0.05000000074505806f, 0.041999999433755875f, 0.029999999329447746f), float3((0.5f * c_11)) );
    float fl_ao_0 = mix(1.0f, 0.5f, c_11);
    float rad_1 = mix(0.41999998688697815f, 0.62000000476837158f, saturate(_S356 / 30.0f)) + 0.20000000298023224f * smoothstep(0.60000002384185791f, 1.0f, c_11);
    float pres_1 = saturate(c_11 / (3.1415901184082031f * rad_1 * rad_1));
    float pb_0 = pres_1 * (1.0f - mx_3);
    float pc_0 = pres_1 * mx_3;
    float _S367 = mix(8.0f, 5.119999885559082f, mx_3) * rad_1 * 2.0f;
    float crown_px_0 = _S367 / max(px_m_0, 0.0010000000474974513f);
    float det_1 = smoothstep(1.5f, 4.0f, crown_px_0);
    float _S368 = smoothstep(80.0f, 40.0f, _S367 / max(px_iso_0, 0.0010000000474974513f));
    bool near_on_0 = near_px_0 >= 0.0f;
    float paint_0;
    if(near_on_0)
    {
        FrameData_natural_0 device* _S369 = ed_buf_1(kernelContext_25->ed_c_0->frame_0, kernelContext_25);
        float4 _S370 = float4((&_S369->imp_0)->far_0) ;
        float _S371 = _S370.z;
        if(_S371 > 0.0f)
        {
            paint_0 = saturate((_S371 - near_px_0) / max(_S371 - _S370.w, 0.0010000000474974513f));
        }
        else
        {
            paint_0 = 1.0f - smoothstep(12.0f, 15.0f, near_px_0);
        }
    }
    else
    {
        paint_0 = _S368;
    }
    float3 _S372 = float3(stand_0) ;
    float floor_sun_far_0 = 1.0f - 0.92000001668930054f * max(c_11, page_sh_0);
    float ao_far_0 = mix(fl_ao_0, 0.85000002384185791f, c_11);
    float sun_far_0 = mix(floor_sun_far_0, 1.0f, c_11);
    thread TForest_0 o_1;
    (&o_1)->albedo_2 = mix(fl_1, leaf_0 * float3(0.81999999284744263f)  * _S372, float3(c_11) );
    (&o_1)->n_3 = n_4;
    (&o_1)->ao_3 = ao_far_0;
    (&o_1)->sun_1 = sun_far_0;
    (&o_1)->crown_0 = c_11;
    bool _S373;
    if(det_1 > 0.0f)
    {
        if(!near_on_0)
        {
            _S373 = true;
        }
        else
        {
            _S373 = paint_0 > 0.0f;
        }
    }
    else
    {
        _S373 = false;
    }
    if(_S373)
    {
        bool _S374 = pb_0 > 0.0f;
        float4 kb_0;
        if(_S374)
        {
            float4 _S375 = t_crown_0(w_7, 8.0f, pb_0, rad_1, 3U);
            kb_0 = _S375;
        }
        else
        {
            kb_0 = float4(0.0f, 0.0f, 9.0f, 0.0f);
        }
        bool _S376 = pc_0 > 0.0f;
        float4 kc_0;
        if(_S376)
        {
            float4 _S377 = t_crown_0(w_7 + float2(1000.0f) , 5.119999885559082f, pc_0, rad_1 * 0.89999997615814209f, 11U);
            kc_0 = _S377;
        }
        else
        {
            kc_0 = float4(0.0f, 0.0f, 9.0f, 0.0f);
        }
        bool conifer_1 = (kc_0.z) < (kb_0.z);
        float4 k_7;
        if(conifer_1)
        {
            k_7 = kc_0;
        }
        else
        {
            k_7 = kb_0;
        }
        float in_crown_0 = smoothstep(1.0f, 0.85000002384185791f, k_7.z);
        float3 t2_0 = tu_4 * float3(k_7.x)  + tv_5 * float3(k_7.y) ;
        float2 _S378 = k_7.xy;
        float _S379 = min(dot(_S378, _S378), 1.0f);
        float3 nc_0;
        if(conifer_1)
        {
            nc_0 = normalize(up_3 + t2_0 * float3(1.2999999523162842f) );
        }
        else
        {
            nc_0 = normalize(t2_0 + up_3 * float3(sqrt(max(1.0f - _S379, 0.039999999105930328f))) );
        }
        float3 nn_0 = normalize(n_4 + (nc_0 - up_3) * float3(0.89999997615814209f) );
        if(conifer_1)
        {
            crown_b_0 = crown_c_0;
        }
        float _S380 = k_7.w;
        float3 la_0 = crown_b_0 * float3(mix(0.75f, 1.25f, _S380))  * _S372 * mix(float3(1.1200000047683716f, 1.0f, 0.75f), float3(0.85000002384185791f, 0.97000002861022949f, 1.2000000476837158f), float3(fract(_S380 * 7.309999942779541f)) );
        float gap_sun_0;
        if(conifer_1)
        {
            gap_sun_0 = 1.6000000238418579f;
        }
        else
        {
            gap_sun_0 = 1.0f;
        }
        float _S381 = t_noise_0(w_7 * float2(gap_sun_0)  + float2(13.0f) , 1.0f);
        float3 la_1 = la_0 * float3(mix(1.0f, mix(0.69999998807907104f, 1.25f, _S381), smoothstep(6.0f, 16.0f, crown_px_0))) ;
        float crown_ao_0 = mix(0.69999998807907104f, 1.0f, 1.0f - _S379) * mix(1.0f, 0.69999998807907104f, (1.0f - saturate(0.5f + 0.80000001192092896f * dot(_S378, sun2_0 / float2(max(length(sun2_0), 0.0010000000474974513f)) ))) * 0.60000002384185791f);
        if(crown_px_0 > 3.0f)
        {
            float2 wt_1 = w_7 + toward_0 * float2(_S356)  * float2(0.55000001192092896f) ;
            float4 sb_0;
            if(_S374)
            {
                float4 _S382 = t_crown_0(wt_1, 8.0f, pb_0, rad_1, 3U);
                sb_0 = _S382;
            }
            else
            {
                sb_0 = float4(0.0f, 0.0f, 9.0f, 0.0f);
            }
            float4 sc_3;
            if(_S376)
            {
                float4 _S383 = t_crown_0(wt_1 + float2(1000.0f) , 5.119999885559082f, pc_0, rad_1 * 0.89999997615814209f, 11U);
                sc_3 = _S383;
            }
            else
            {
                sc_3 = float4(0.0f, 0.0f, 9.0f, 0.0f);
            }
            gap_sun_0 = mix(floor_sun_far_0, 1.0f - 0.92000001668930054f * smoothstep(1.1000000238418579f, 0.69999998807907104f, min(sb_0.z, sc_3.z)), smoothstep(3.0f, 6.0f, crown_px_0));
        }
        else
        {
            gap_sun_0 = floor_sun_far_0;
        }
        float _S384 = min(gap_sun_0, 1.0f - 0.89999997615814209f * page_sh_0);
        float closed_0 = smoothstep(0.5f, 0.94999998807907104f, c_11);
        float3 _S385 = float3(in_crown_0) ;
        float3 n_near_0 = normalize(mix(n_4, nn_0, _S385));
        float ao_near_0 = mix(mix(fl_ao_0, 0.75f, closed_0), crown_ao_0, in_crown_0);
        float sun_near_0 = mix(mix(_S384, max(_S384, 0.34999999403953552f), closed_0), 1.0f, in_crown_0);
        float3 _S386 = float3(det_1) ;
        (&o_1)->albedo_2 = mix((&o_1)->albedo_2, mix(mix(fl_1, leaf_0 * _S372 * float3(0.85000002384185791f) , float3(closed_0) ), la_1, _S385), _S386);
        (&o_1)->n_3 = normalize(mix((&o_1)->n_3, n_near_0, _S386));
        (&o_1)->ao_3 = mix((&o_1)->ao_3, ao_near_0, det_1);
        (&o_1)->sun_1 = mix((&o_1)->sun_1, sun_near_0, det_1);
        (&o_1)->crown_0 = mix((&o_1)->crown_0, in_crown_0, det_1);
    }
    if(paint_0 < 1.0f)
    {
        float _S387 = t_noise_0(w_7 + float2(77.0f) , 4.0f);
        float _S388 = _S387 * 0.60000002384185791f;
        float _S389 = t_noise_0(w_7 + float2(33.0f) , 1.0f);
        float _S390 = min(mix(1.0f, 0.40000000596046448f + 0.60000002384185791f * smoothstep(0.34999999403953552f, 0.75f, _S388 + _S389 * 0.40000000596046448f), c_11), 1.0f - 0.60000002384185791f * page_sh_0);
        float3 _S391 = float3(paint_0) ;
        (&o_1)->albedo_2 = mix(fl_1, (&o_1)->albedo_2, _S391);
        (&o_1)->n_3 = normalize(mix(n_4, (&o_1)->n_3, _S391));
        float ao_crown_0 = (&o_1)->ao_3;
        float sun_crown_0 = (&o_1)->sun_1;
        (&o_1)->ao_3 = mix(max(fl_ao_0, 0.75f), (&o_1)->ao_3, paint_0);
        (&o_1)->sun_1 = mix(_S390, (&o_1)->sun_1, paint_0);
        if(near_on_0)
        {
            (&o_1)->ao_3 = mix(max(fl_ao_0, 0.85000002384185791f), ao_crown_0, paint_0);
            (&o_1)->sun_1 = mix(1.0f - 0.5f * page_sh_0, sun_crown_0, paint_0);
        }
        (&o_1)->crown_0 = (&o_1)->crown_0 * paint_0;
    }
    return o_1;
}

float cascade_extent_0(FrameData_natural_0 device* f_7, uint c_12, float3 p_8)
{
    float4 s_13 = (((float4(p_8, 1.0f)) * (matrix<float,int(4),int(4)> ((&(&f_7->light_0)->shadow_1)->data_2[c_12].data_0[int(0)][int(0)], (&(&f_7->light_0)->shadow_1)->data_2[c_12].data_0[int(1)][int(0)], (&(&f_7->light_0)->shadow_1)->data_2[c_12].data_0[int(2)][int(0)], (&(&f_7->light_0)->shadow_1)->data_2[c_12].data_0[int(3)][int(0)], (&(&f_7->light_0)->shadow_1)->data_2[c_12].data_0[int(0)][int(1)], (&(&f_7->light_0)->shadow_1)->data_2[c_12].data_0[int(1)][int(1)], (&(&f_7->light_0)->shadow_1)->data_2[c_12].data_0[int(2)][int(1)], (&(&f_7->light_0)->shadow_1)->data_2[c_12].data_0[int(3)][int(1)], (&(&f_7->light_0)->shadow_1)->data_2[c_12].data_0[int(0)][int(2)], (&(&f_7->light_0)->shadow_1)->data_2[c_12].data_0[int(1)][int(2)], (&(&f_7->light_0)->shadow_1)->data_2[c_12].data_0[int(2)][int(2)], (&(&f_7->light_0)->shadow_1)->data_2[c_12].data_0[int(3)][int(2)], (&(&f_7->light_0)->shadow_1)->data_2[c_12].data_0[int(0)][int(3)], (&(&f_7->light_0)->shadow_1)->data_2[c_12].data_0[int(1)][int(3)], (&(&f_7->light_0)->shadow_1)->data_2[c_12].data_0[int(2)][int(3)], (&(&f_7->light_0)->shadow_1)->data_2[c_12].data_0[int(3)][int(3)]))));
    float _S392 = s_13.z;
    bool _S393;
    if(_S392 < 0.0f)
    {
        _S393 = true;
    }
    else
    {
        _S393 = _S392 > 1.0f;
    }
    float _S394;
    if(_S393)
    {
        _S394 = 1.0e+09f;
    }
    else
    {
        _S394 = max(abs(s_13.x), abs(s_13.y));
    }
    return _S394;
}

float pcf_radius_texels_0(FrameData_natural_0 device* f_8, float texel_0)
{
    return clamp((float4((&f_8->light_0)->sky_0) ).z / texel_0, 1.0f, max((float4((&f_8->light_0)->sky_0) ).w, 1.0f));
}

float ed_depth_read_0(uint i_16, uint2 p_9, KernelContext_0 thread* kernelContext_26)
{
    float _S395 = ((reinterpret_cast<device depth2d<float, access::sample>*>((kernelContext_26->ed_textures_0))[(i_16)].read((p_9))));
    return _S395;
}

float cascade_lookup_0(FrameData_natural_0 device* f_9, uint atlas_0, uint c_13, float3 p_10, float3 ng_0, float2 px_1, KernelContext_0 thread* kernelContext_27)
{
    float4 _S396 = float4((&(&f_9->light_0)->cascade_0)->data_3[c_13]) ;
    float tile_1 = (float4((&f_9->light_0)->cfg_0) ).y;
    float _S397 = _S396.x;
    float _S398 = pcf_radius_texels_0(f_9, _S397);
    float sp_0 = _S398 * 0.5f;
    float nol_0 = saturate(dot(ng_0, (float4(f_9->sun_dir_0) ).xyz));
    float4 s_14 = (((float4(p_10 + ng_0 * float3((_S397 * ((float4((&f_9->light_0)->filter_0) ).z + 2.0f * sqrt(1.0f - nol_0 * nol_0)))) , 1.0f)) * (matrix<float,int(4),int(4)> ((&(&f_9->light_0)->shadow_1)->data_2[c_13].data_0[int(0)][int(0)], (&(&f_9->light_0)->shadow_1)->data_2[c_13].data_0[int(1)][int(0)], (&(&f_9->light_0)->shadow_1)->data_2[c_13].data_0[int(2)][int(0)], (&(&f_9->light_0)->shadow_1)->data_2[c_13].data_0[int(3)][int(0)], (&(&f_9->light_0)->shadow_1)->data_2[c_13].data_0[int(0)][int(1)], (&(&f_9->light_0)->shadow_1)->data_2[c_13].data_0[int(1)][int(1)], (&(&f_9->light_0)->shadow_1)->data_2[c_13].data_0[int(2)][int(1)], (&(&f_9->light_0)->shadow_1)->data_2[c_13].data_0[int(3)][int(1)], (&(&f_9->light_0)->shadow_1)->data_2[c_13].data_0[int(0)][int(2)], (&(&f_9->light_0)->shadow_1)->data_2[c_13].data_0[int(1)][int(2)], (&(&f_9->light_0)->shadow_1)->data_2[c_13].data_0[int(2)][int(2)], (&(&f_9->light_0)->shadow_1)->data_2[c_13].data_0[int(3)][int(2)], (&(&f_9->light_0)->shadow_1)->data_2[c_13].data_0[int(0)][int(3)], (&(&f_9->light_0)->shadow_1)->data_2[c_13].data_0[int(1)][int(3)], (&(&f_9->light_0)->shadow_1)->data_2[c_13].data_0[int(2)][int(3)], (&(&f_9->light_0)->shadow_1)->data_2[c_13].data_0[int(3)][int(3)]))));
    float2 _S399 = float2(s_14.x * 0.5f + 0.5f, 0.5f - s_14.y * 0.5f) * float2(tile_1) ;
    float ign_0 = fract(52.982917785644531f * fract(dot(px_1, float2(0.067110560834407806f, 0.0058371499180793762f))));
    float2 _S400 = float2(0.5f) ;
    float2 _S401 = (float2(ign_0, fract(ign_0 * 7.309999942779541f + 0.37000000476837158f)) - _S400) * float2(0.0f) ;
    float3 ns_1 = (((float4(ng_0, 0.0f)) * (matrix<float,int(4),int(4)> ((&(&f_9->light_0)->shadow_1)->data_2[c_13].data_0[int(0)][int(0)], (&(&f_9->light_0)->shadow_1)->data_2[c_13].data_0[int(1)][int(0)], (&(&f_9->light_0)->shadow_1)->data_2[c_13].data_0[int(2)][int(0)], (&(&f_9->light_0)->shadow_1)->data_2[c_13].data_0[int(3)][int(0)], (&(&f_9->light_0)->shadow_1)->data_2[c_13].data_0[int(0)][int(1)], (&(&f_9->light_0)->shadow_1)->data_2[c_13].data_0[int(1)][int(1)], (&(&f_9->light_0)->shadow_1)->data_2[c_13].data_0[int(2)][int(1)], (&(&f_9->light_0)->shadow_1)->data_2[c_13].data_0[int(3)][int(1)], (&(&f_9->light_0)->shadow_1)->data_2[c_13].data_0[int(0)][int(2)], (&(&f_9->light_0)->shadow_1)->data_2[c_13].data_0[int(1)][int(2)], (&(&f_9->light_0)->shadow_1)->data_2[c_13].data_0[int(2)][int(2)], (&(&f_9->light_0)->shadow_1)->data_2[c_13].data_0[int(3)][int(2)], (&(&f_9->light_0)->shadow_1)->data_2[c_13].data_0[int(0)][int(3)], (&(&f_9->light_0)->shadow_1)->data_2[c_13].data_0[int(1)][int(3)], (&(&f_9->light_0)->shadow_1)->data_2[c_13].data_0[int(2)][int(3)], (&(&f_9->light_0)->shadow_1)->data_2[c_13].data_0[int(3)][int(3)])))).xyz;
    float rl_0 = _S397 * tile_1 * 0.5f;
    float _S402 = _S396.y;
    float _S403 = ns_1.z * _S402;
    float izn_0 = 1.0f / max(abs(_S403), 0.11999999731779099f);
    float _S404 = float((int(sign((_S403)))));
    float gx_1 = clamp(- (ns_1.x * rl_0) * izn_0 * _S404, -8.0f, 8.0f);
    float gy_1 = clamp(- (ns_1.y * rl_0) * izn_0 * _S404, -8.0f, 8.0f);
    float _S405 = s_14.z + ((float4((&f_9->light_0)->filter_0) ).w + 0.5f * (abs(gx_1) + abs(gy_1))) * _S397 / _S402;
    float _S406 = _S397 / _S402;
    int2 _S407 = int2(_S396.zw);
    int2 _S408 = _S407 + int2((int(tile_1) - int(1))) ;
    float _S409 = 1.0f / (_S398 + sp_0);
    int y_0 = int(-2);
    float lit_0 = 0.0f;
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
        float oy_0 = float(y_0) * sp_0;
        float _S410 = 1.0f - abs(oy_0) * _S409;
        int x_2 = int(-2);
        for(;;)
        {
            if(x_2 <= int(2))
            {
            }
            else
            {
                break;
            }
            float ox_0 = float(x_2) * sp_0;
            float w_8 = (1.0f - abs(ox_0) * _S409) * _S410;
            float2 tp_1 = floor(_S399 + float2(ox_0, oy_0) + _S401);
            float2 d_22 = tp_1 + _S400 - _S399;
            float _S411 = ed_depth_read_0(atlas_0, uint2(clamp(_S407 + int2(tp_1), _S407, _S408)), kernelContext_27);
            float _S412;
            if((_S405 + (gx_1 * d_22.x - gy_1 * d_22.y) * _S406) >= _S411)
            {
                _S412 = 1.0f;
            }
            else
            {
                _S412 = 0.0f;
            }
            float lit_1 = lit_0 + _S412 * w_8;
            float wsum_1 = wsum_0 + w_8;
            x_2 = x_2 + int(1);
            lit_0 = lit_1;
            wsum_0 = wsum_1;
        }
        y_0 = y_0 + int(1);
    }
    return lit_0 / wsum_0;
}

float sun_shadow_0(FrameData_natural_0 device* f_10, uint atlas_1, float3 p_11, float3 ng_1, float dist_1, float2 px_2, KernelContext_0 thread* kernelContext_28)
{
    uint n_5 = uint((float4((&f_10->light_0)->cfg_0) ).x);
    bool _S413;
    if(((uint4((&f_10->light_0)->flags_0) ).x) == 0U)
    {
        _S413 = true;
    }
    else
    {
        _S413 = n_5 == 0U;
    }
    if(_S413)
    {
        _S413 = true;
    }
    else
    {
        _S413 = dist_1 >= ((float4((&f_10->light_0)->filter_0) ).y);
    }
    if(_S413)
    {
        return 1.0f;
    }
    float _S414 = 1.0f - (float4((&f_10->light_0)->cfg_0) ).w * 2.0f / (float4((&f_10->light_0)->cfg_0) ).y;
    float _S415 = (float4((&f_10->light_0)->cfg_0) ).z;
    uint c_14 = 0U;
    for(;;)
    {
        if(c_14 < n_5)
        {
        }
        else
        {
            break;
        }
        float _S416 = cascade_extent_0(f_10, c_14, p_11);
        float e_8 = _S416 / _S414;
        if(e_8 > 1.0f)
        {
            c_14 = c_14 + 1U;
            continue;
        }
        float _S417 = cascade_lookup_0(f_10, atlas_1, c_14, p_11, ng_1, px_2, kernelContext_28);
        float blend_0 = smoothstep(1.0f - _S415, 1.0f, e_8);
        float v_3;
        if(blend_0 > 0.0f)
        {
            uint _S418 = c_14 + 1U;
            if(_S418 < n_5)
            {
                float _S419 = cascade_extent_0(f_10, _S418, p_11);
                _S413 = (_S419 / _S414) <= 1.0f;
            }
            else
            {
                _S413 = false;
            }
            if(_S413)
            {
                float _S420 = cascade_lookup_0(f_10, atlas_1, _S418, p_11, ng_1, px_2, kernelContext_28);
                v_3 = _S420;
            }
            else
            {
                v_3 = 1.0f;
            }
            v_3 = mix(_S417, v_3, blend_0);
        }
        else
        {
            v_3 = _S417;
        }
        return mix(v_3, 1.0f, smoothstep(0.0f, 1.0f, saturate((dist_1 - (float4((&f_10->light_0)->filter_0) ).x) / max((float4((&f_10->light_0)->filter_0) ).y - (float4((&f_10->light_0)->filter_0) ).x, 0.0010000000474974513f))));
    }
    return 1.0f;
}

packed_float4 device* ed_buf_4(uint i_17, KernelContext_0 thread* kernelContext_29)
{
    return (packed_float4 device*)kernelContext_29->ed_buffers_0[i_17];
}

float2 ao_upsample_0(FrameData_natural_0 device* f_11, uint ao_buf_0, float2 pixel_1, float zlin_0, KernelContext_0 thread* kernelContext_30)
{
    float sv_0;
    if(((uint4((&f_11->light_0)->flags_0) ).y) == 0U)
    {
        return float2(1.0f) ;
    }
    packed_float4 device* _S421 = ed_buf_4(ao_buf_0, kernelContext_30);
    uint _S422 = uint((float4((&f_11->light_0)->ao_1) ).y);
    uint _S423 = uint((float4((&f_11->light_0)->ao_1) ).z);
    float2 _S424 = float2(0.5f) ;
    float2 h_10 = (pixel_1 - _S424) * _S424;
    float2 b_6 = floor(h_10);
    float2 _S425 = h_10 - b_6;
    float2 _S426 = float2(0.0f) ;
    int y_1 = int(0);
    float2 sum_0 = _S426;
    float wsum_2 = 0.0f;
    for(;;)
    {
        if(y_1 < int(2))
        {
        }
        else
        {
            break;
        }
        int x_3 = int(0);
        for(;;)
        {
            if(x_3 < int(2))
            {
            }
            else
            {
                break;
            }
            int2 t_1 = clamp(int2(b_6) + int2(x_3, y_1), int2(int(0)) , int2(int(_S422) - int(1), int(_S423) - int(1)));
            float4 _S427 = float4(*(_S421 + (uint(t_1.y) * _S422 + uint(t_1.x)))) ;
            if(x_3 == int(0))
            {
                sv_0 = 1.0f - _S425.x;
            }
            else
            {
                sv_0 = _S425.x;
            }
            float _S428;
            if(y_1 == int(0))
            {
                _S428 = 1.0f - _S425.y;
            }
            else
            {
                _S428 = _S425.y;
            }
            float w_9 = max(sv_0 * _S428, 0.0010000000474974513f) * (1.0f / (0.0010000000474974513f + abs(_S427.z - zlin_0) / max(zlin_0, 0.0010000000474974513f) * 50.0f));
            float2 sum_1 = sum_0 + _S427.xy * float2(w_9) ;
            float wsum_3 = wsum_2 + w_9;
            x_3 = x_3 + int(1);
            sum_0 = sum_1;
            wsum_2 = wsum_3;
        }
        y_1 = y_1 + int(1);
    }
    float2 r_3;
    if(wsum_2 > 0.0f)
    {
        r_3 = sum_0 / float2(wsum_2) ;
    }
    else
    {
        r_3 = float2(1.0f) ;
    }
    if(((float4((&f_11->light_0)->skyvis_0) ).x) > 0.0f)
    {
        sv_0 = saturate(r_3.y);
    }
    else
    {
        sv_0 = 1.0f;
    }
    return float2(pow(saturate(r_3.x), (float4((&f_11->light_0)->ao_1) ).x), sv_0);
}

bool atmos_on_0(FrameData_natural_0 device* f_12)
{
    return ((uint4((&f_12->atmos_0)->cfg_1) ).x) != 0U;
}

float atm_rb_0(FrameData_natural_0 device* f_13)
{
    return (float4((&f_13->atmos_0)->radii_0) ).x;
}

float atm_height_at_0(float rb_0, float h_11, float mu_0, float t_2)
{
    float q_14 = t_2 * t_2 + 2.0f * (rb_0 + h_11) * mu_0 * t_2 + h_11 * (2.0f * rb_0 + h_11);
    return q_14 / (sqrt(max(rb_0 * rb_0 + q_14, 0.0f)) + rb_0);
}

float atm_rt_0(FrameData_natural_0 device* f_14)
{
    return (float4((&f_14->atmos_0)->radii_0) ).y;
}

float atm_dist_top_0(float rb_1, float rt_0, float h_12, float mu_1)
{
    float r_4 = rb_1 + h_12;
    float _S429 = r_4 * mu_1;
    return max(- r_4 * mu_1 + sqrt(max(_S429 * _S429 + (rt_0 - rb_1 - h_12) * (rt_0 + r_4), 0.0f)), 0.0f);
}

float2 atm_trans_uv_0(float rb_2, float rt_1, float h_13, float mu_2)
{
    float ht_0 = rt_1 - rb_2;
    float _S430 = clamp(h_13, 0.0f, ht_0);
    float big_h_0 = sqrt(ht_0 * (rt_1 + rb_2));
    float rho_0 = sqrt(max(_S430 * (2.0f * rb_2 + _S430), 0.0f));
    float dmin_0 = ht_0 - _S430;
    return float2((atm_dist_top_0(rb_2, rt_1, _S430, mu_2) - dmin_0) / max(rho_0 + big_h_0 - dmin_0, 9.99999997475242708e-07f), rho_0 / big_h_0);
}

float4 atm_bilinear_0(packed_float4 device* buf_0, uint w_10, uint row0_0, uint rows_1, float2 uv_1)
{
    float2 p_12 = saturate(uv_1) * float2(float(w_10 - 1U), float(rows_1 - 1U));
    uint2 _S431 = min(uint2(p_12), uint2(w_10 - 2U, rows_1 - 2U));
    float2 g_6 = p_12 - float2(_S431);
    uint b_7 = (row0_0 + _S431.y) * w_10 + _S431.x;
    float4 _S432 = float4(g_6.x) ;
    uint _S433 = b_7 + w_10;
    return mix(mix(float4(*(buf_0 + b_7)) , float4(*(buf_0 + (b_7 + 1U))) , _S432), mix(float4(*(buf_0 + _S433)) , float4(*(buf_0 + (_S433 + 1U))) , _S432), float4(g_6.y) );
}

float3 atm_transmittance_0(FrameData_natural_0 device* f_15, float h_14, float mu_3, KernelContext_0 thread* kernelContext_31)
{
    float _S434 = atm_rb_0(f_15);
    float _S435 = atm_rt_0(f_15);
    float2 uv_2 = atm_trans_uv_0(_S434, _S435, h_14, mu_3);
    packed_float4 device* _S436 = ed_buf_4((uint4((&f_15->atmos_0)->bufs_0) ).x, kernelContext_31);
    float4 _S437 = atm_bilinear_0(_S436, 256U, 0U, 64U, uv_2);
    return _S437.xyz;
}

float atm_rb_1(FrameData_natural_0 device* f_16)
{
    return (float4((&f_16->atmos_0)->radii_0) ).x;
}

float atm_cos_horizon_0(float rb_3, float h_15)
{
    return - sqrt(max(h_15 * (2.0f * rb_3 + h_15), 0.0f)) / (rb_3 + h_15);
}

float atm_earth_shadow_0(FrameData_natural_0 device* f_17, float h_16, float mu_s_0)
{
    float _S438 = atm_rb_1(f_17);
    float ch_0 = atm_cos_horizon_0(_S438, h_16);
    float s_15 = 0.0046999999321997166f * sqrt(max(1.0f - ch_0 * ch_0, 0.0f)) + 9.9999997473787516e-06f;
    return smoothstep(- s_15, s_15, mu_s_0 - ch_0);
}

float3 atm_sun_transmittance_0(FrameData_natural_0 device* f_18, float h_17, float mu_s_1, KernelContext_0 thread* kernelContext_32)
{
    float3 _S439 = atm_transmittance_0(f_18, h_17, mu_s_1, kernelContext_32);
    float _S440 = atm_earth_shadow_0(f_18, h_17, mu_s_1);
    return _S439 * float3(_S440) ;
}

float3 atmos_sun_at_0(FrameData_natural_0 device* f_19, float3 p_13, KernelContext_0 thread* kernelContext_33)
{
    float t_3 = length(p_13) * 0.0010000000474974513f;
    float3 d_23;
    if(t_3 > 0.0f)
    {
        d_23 = p_13 * float3((0.0010000000474974513f / t_3)) ;
    }
    else
    {
        d_23 = (float4((&f_19->atmos_0)->up_0) ).xyz;
    }
    float _S441 = atm_rb_0(f_19);
    float h0_0 = (float4((&f_19->atmos_0)->radii_0) ).z;
    float h_18 = atm_height_at_0(_S441, h0_0, dot(d_23, (float4((&f_19->atmos_0)->up_0) ).xyz), t_3);
    float3 _S442 = (float4((&f_19->atmos_0)->sun_0) ).xyz;
    float3 _S443 = atm_sun_transmittance_0(f_19, h_18, clamp(((_S441 + h0_0) * (float4((&f_19->atmos_0)->up_0) ).w + t_3 * dot(d_23, (float4(f_19->sun_dir_0) ).xyz)) / (_S441 + h_18), -1.0f, 1.0f), kernelContext_33);
    return _S442 * _S443;
}

float3 sun_light_at_0(FrameData_natural_0 device* f_20, float3 p_14, KernelContext_0 thread* kernelContext_34)
{
    bool _S444 = atmos_on_0(f_20);
    float3 _S445;
    if(_S444)
    {
        float3 _S446 = atmos_sun_at_0(f_20, p_14, kernelContext_34);
        _S445 = _S446;
    }
    else
    {
        _S445 = (float4(f_20->sun_color_0) ).xyz;
    }
    return _S445;
}

float3 f_schlick_0(float3 f0_0, float voh_0)
{
    return f0_0 + (float3(1.0f)  - f0_0) * float3(pow(1.0f - voh_0, 5.0f)) ;
}

float d_ggx_0(float noh_0, float a_14)
{
    float a2_0 = a_14 * a_14;
    float d_24 = noh_0 * noh_0 * (a2_0 - 1.0f) + 1.0f;
    return a2_0 / (3.1415927410125732f * d_24 * d_24);
}

float v_smith_0(float nov_0, float nol_1, float a_15)
{
    float a2_1 = a_15 * a_15;
    float _S447 = 1.0f - a2_1;
    return 0.5f / max(nol_1 * sqrt(nov_0 * nov_0 * _S447 + a2_1) + nov_0 * sqrt(nol_1 * nol_1 * _S447 + a2_1), 9.9999997473787516e-06f);
}

float2 env_brdf_0(float nov_1, float rough_1)
{
    float4 r_5 = float4(rough_1)  * float4(-1.0f, -0.027499999850988388f, -0.57200002670288086f, 0.02199999988079071f) + float4(1.0f, 0.042500000447034836f, 1.0399999618530273f, -0.039999999105930328f);
    float _S448 = r_5.x;
    return float2(-1.0399999618530273f, 1.0399999618530273f) * float2((min(_S448 * _S448, exp2(-9.2799997329711914f * nov_1)) * _S448 + r_5.y))  + r_5.zw;
}

bool atmos_on_1(FrameData_natural_0 device* f_21)
{
    return ((uint4((&f_21->atmos_0)->cfg_1) ).x) != 0U;
}

float3 atm_sky_view_0(FrameData_natural_0 device* f_22, uint buf_1, uint w_11, uint rows_2, float h_19, float3 d_25, KernelContext_0 thread* kernelContext_35)
{
    float mu_4 = clamp(dot(d_25, (float4((&f_22->atmos_0)->up_0) ).xyz), -1.0f, 1.0f);
    float mu_s_2 = (float4((&f_22->atmos_0)->up_0) ).w;
    float nu_0 = dot(d_25, (float4(f_22->sun_dir_0) ).xyz);
    float _S449 = sqrt(max(1.0f - mu_4 * mu_4, 0.0f)) * sqrt(max(1.0f - mu_s_2 * mu_s_2, 0.0f));
    float cphi_0;
    if(_S449 > 9.99999997475242708e-07f)
    {
        cphi_0 = clamp((nu_0 - mu_4 * mu_s_2) / _S449, -1.0f, 1.0f);
    }
    else
    {
        cphi_0 = 1.0f;
    }
    float u_0 = sqrt(acos(cphi_0) / 3.1415927410125732f);
    float _S450 = atm_rb_0(f_22);
    float zha_0 = acos(atm_cos_horizon_0(_S450, h_19));
    float theta_0 = acos(mu_4);
    uint half_rows_0 = rows_2 / 2U;
    float c_15;
    uint row0_1;
    if(theta_0 < zha_0)
    {
        float _S451 = 1.0f - sqrt(saturate(1.0f - theta_0 / zha_0));
        row0_1 = 0U;
        c_15 = _S451;
    }
    else
    {
        float _S452 = sqrt(saturate((theta_0 - zha_0) / max(3.1415927410125732f - zha_0, 9.99999997475242708e-07f)));
        row0_1 = half_rows_0;
        c_15 = _S452;
    }
    packed_float4 device* _S453 = ed_buf_4(buf_1, kernelContext_35);
    float4 _S454 = atm_bilinear_0(_S453, w_11, row0_1, half_rows_0, float2(u_0, c_15));
    return _S454.xyz;
}

float3 atmos_sky_0(FrameData_natural_0 device* f_23, float3 d_26, KernelContext_0 thread* kernelContext_36)
{
    float3 _S455 = atm_sky_view_0(f_23, (uint4((&f_23->atmos_0)->bufs_0) ).z, 192U, 108U, (float4((&f_23->atmos_0)->radii_0) ).z, d_26, kernelContext_36);
    return _S455;
}

float3 sky_rad_0(FrameData_natural_0 device* f_24, float3 d_27, float disc_k_0)
{
    float3 sun_2 = (float4(f_24->sun_dir_0) ).xyz;
    float h_20 = d_27.y;
    float t_4 = pow(saturate(1.0f - max(h_20, 0.0f)), 6.5f);
    float mu_5 = dot(d_27, sun_2);
    float warm_0 = saturate(1.0f - sun_2.y * 2.5f);
    float3 c_16 = mix((float4(f_24->sky_zenith_0) ).xyz, (float4(f_24->sky_horizon_0) ).xyz, float3(t_4) ) + float3(1.0f, 0.41999998688697815f, 0.14000000059604645f) * float3(pow(saturate(mu_5 * 0.5f + 0.5f), 5.0f))  * float3(warm_0)  * float3(0.85000002384185791f)  * float3((0.40000000596046448f + t_4)) ;
    float3 _S456 = float3((float4(f_24->sun_dir_0) ).w) ;
    return mix(mix(c_16, c_16 * float3(1.0f, 0.69999998807907104f, 0.55000001192092896f), float3((warm_0 * 0.40000000596046448f)) ) + float3(1.0f, 0.89999997615814209f, 0.69999998807907104f) * float3(pow(saturate(mu_5), 28.0f))  * float3(0.34999999403953552f) , (float4(f_24->ground_color_0) ).xyz, float3(saturate(- h_20 * 6.0f)) ) * _S456 + float3(1.0f, 0.94999998807907104f, 0.85000002384185791f) * float3((smoothstep(0.99965000152587891f, 0.99984997510910034f, mu_5) * step(0.0f, h_20)))  * float3(60.0f)  * _S456 * float3(disc_k_0) ;
}

float3 sky_env_0(FrameData_natural_0 device* f_25, float3 d_28, KernelContext_0 thread* kernelContext_37)
{
    bool _S457 = atmos_on_1(f_25);
    float3 _S458;
    if(_S457)
    {
        float3 _S459 = atmos_sky_0(f_25, d_28, kernelContext_37);
        _S458 = _S459;
    }
    else
    {
        float3 _S460 = sky_rad_0(f_25, d_28, 0.0f);
        _S458 = _S460;
    }
    return _S458;
}

float3 sh_irradiance_0(FrameData_natural_0 device* f_26, float3 n_6, KernelContext_0 thread* kernelContext_38)
{
    thread array<float4, int(9)> s_16;
    packed_float4 device* _S461 = ed_buf_4((uint4((&f_26->atmos_0)->bufs2_0) ).z, kernelContext_38);
    uint k_8 = 0U;
    for(;;)
    {
        if(k_8 < 9U)
        {
        }
        else
        {
            break;
        }
        bool _S462 = atmos_on_0(f_26);
        float4 _S463;
        if(_S462)
        {
            _S463 = float4(*(_S461 + (288U + k_8))) ;
        }
        else
        {
            _S463 = float4((&(&f_26->light_0)->sh_0)->data_4[k_8]) ;
        }
        s_16[k_8] = _S463;
        k_8 = k_8 + 1U;
    }
    float _S464 = n_6.y;
    float _S465 = n_6.z;
    float _S466 = n_6.x;
    return max(s_16[int(0)].xyz + (s_16[int(1)].xyz * float3(_S464)  + s_16[int(2)].xyz * float3(_S465)  + s_16[int(3)].xyz * float3(_S466) ) + (s_16[int(4)].xyz * float3((_S466 * _S464))  + s_16[int(5)].xyz * float3((_S464 * _S465))  + s_16[int(6)].xyz * float3((3.0f * _S465 * _S465 - 1.0f)) ) + (s_16[int(7)].xyz * float3((_S466 * _S465))  + s_16[int(8)].xyz * float3((_S466 * _S466 - _S464 * _S464)) ), float3(0.0f) );
}

float3 hemi_ambient_0(FrameData_natural_0 device* f_27, float3 n_7)
{
    return mix((float4(f_27->ground_color_0) ).xyz * float3((float4(f_27->sun_dir_0) ).w) , mix((float4(f_27->sky_horizon_0) ).xyz, (float4(f_27->sky_zenith_0) ).xyz, float3(0.5f) ) * float3((float4(f_27->sun_dir_0) ).w) , float3((n_7.y * 0.5f + 0.5f)) );
}

float3 ambient_light_0(FrameData_natural_0 device* f_28, float3 n_8, KernelContext_0 thread* kernelContext_39)
{
    float3 _S467;
    if(((uint4((&f_28->light_0)->flags_0) ).z) != 0U)
    {
        float3 _S468 = sh_irradiance_0(f_28, n_8, kernelContext_39);
        _S467 = _S468;
    }
    else
    {
        float3 _S469 = hemi_ambient_0(f_28, n_8);
        _S467 = _S469;
    }
    return _S467;
}

float3 occluded_ambient_0(FrameData_natural_0 device* f_29, float3 open_0, float sv_1, KernelContext_0 thread* kernelContext_40)
{
    float3 _S470 = ambient_light_0(f_29, float3(0.0f, -1.0f, 0.0f), kernelContext_40);
    return mix(_S470 * float3((float4((&f_29->light_0)->skyvis_0) ).y) , open_0, float3(sv_1) );
}

float3 ao_multibounce_0(FrameData_natural_0 device* f_30, float ao_4, float3 albedo_3)
{
    float3 _S471 = float3(ao_4) ;
    return mix(_S471, max(_S471, ((_S471 * (float3(2.0404000282287598f)  * albedo_3 - float3(0.33239999413490295f) ) + (float3(-4.795100212097168f)  * albedo_3 + float3(0.64170002937316895f) )) * _S471 + (float3(2.7551999092102051f)  * albedo_3 + float3(0.69029998779296875f) )) * _S471), float3((float4((&f_30->light_0)->sky_0) ).y) );
}

float3 shade_pbr_0(FrameData_natural_0 device* f_31, float3 sun_3, float3 albedo_4, float metal_0, float rough_2, float ao_5, float sv_2, float sun_vis_0, float3 n_9, float3 v_4, KernelContext_0 thread* kernelContext_41)
{
    float _S472 = saturate(metal_0);
    float _S473 = clamp(rough_2, 0.039999999105930328f, 1.0f);
    float a_16 = _S473 * _S473;
    float3 l_4 = (float4(f_31->sun_dir_0) ).xyz;
    float3 h_21 = normalize(l_4 + v_4);
    float nol_2 = saturate(dot(n_9, l_4));
    float _S474 = max(dot(n_9, v_4), 9.9999997473787516e-05f);
    float3 f0_1 = mix(float3(0.039999999105930328f) , albedo_4, float3(_S472) );
    float3 diff_col_0 = albedo_4 * float3((1.0f - _S472)) ;
    float3 F_0 = f_schlick_0(f0_1, saturate(dot(v_4, h_21)));
    float3 _S475 = float3(1.0f) ;
    float3 direct_0 = (diff_col_0 * (_S475 - F_0) / float3(3.1415927410125732f)  + float3((d_ggx_0(saturate(dot(n_9, h_21)), a_16) * v_smith_0(_S474, nol_2, a_16)))  * F_0) * sun_3 * float3((nol_2 * sun_vis_0)) ;
    float2 ab_0 = env_brdf_0(_S474, _S473);
    float3 r_6 = reflect(- v_4, n_9);
    float3 env_0;
    if(((uint4((&f_31->light_0)->flags_0) ).z) != 0U)
    {
        float3 _S476 = sky_env_0(f_31, r_6, kernelContext_41);
        float3 _S477 = sh_irradiance_0(f_31, r_6, kernelContext_41);
        env_0 = mix(_S476, _S477, float3(_S473) );
    }
    else
    {
        float3 _S478 = hemi_ambient_0(f_31, r_6);
        env_0 = _S478;
    }
    float3 _S479 = occluded_ambient_0(f_31, env_0, sv_2, kernelContext_41);
    float spec_occ_0 = saturate(pow(_S474 + ao_5, exp2(-16.0f * a_16 - 1.0f)) - 1.0f + ao_5);
    float3 f_env_0 = f0_1 * float3(ab_0.x)  + float3(ab_0.y) ;
    float3 _S480 = diff_col_0 * (_S475 - f_env_0);
    float3 _S481 = ambient_light_0(f_31, n_9, kernelContext_41);
    float3 _S482 = occluded_ambient_0(f_31, _S481, sv_2, kernelContext_41);
    float3 _S483 = _S480 * _S482;
    float3 _S484 = ao_multibounce_0(f_31, ao_5, diff_col_0);
    return direct_0 + (_S483 * _S484 + _S479 * f_env_0 * float3(spec_occ_0) );
}

float atm_enter_top_0(float rb_4, float rt_2, float h_22, float mu_6)
{
    float r_7 = rb_4 + h_22;
    float _S485 = r_7 * mu_6;
    float d_29 = _S485 * _S485 - (h_22 - (rt_2 - rb_4)) * (r_7 + rt_2);
    if(d_29 < 0.0f)
    {
        return -1.0f;
    }
    float t_5 = - r_7 * mu_6 - sqrt(d_29);
    float _S486;
    if(t_5 >= 0.0f)
    {
        _S486 = t_5;
    }
    else
    {
        _S486 = -1.0f;
    }
    return _S486;
}

float atm_ap_slice_0(FrameData_natural_0 device* f_32, float t_6)
{
    float n_10 = (float4((&f_32->atmos_0)->ap_0) ).x;
    return log2(1.0f + max(t_6, 0.0f) / n_10) / log2(1.0f + (float4((&f_32->atmos_0)->ap_0) ).y / n_10) * 32.0f;
}

void atm_froxel_xy_0(packed_float4 device* l_5, packed_float4 device* t_7, float2 g_7, uint2 i_18, uint k_9, float3 thread* lo_2, float3 thread* to_0)
{
    uint b_8 = (k_9 * 32U + i_18.y) * 32U + i_18.x;
    uint _S487 = b_8 + 1U;
    float4 _S488 = float4(g_7.x) ;
    uint _S489 = b_8 + 32U;
    uint _S490 = _S489 + 1U;
    float4 t0_0 = mix(float4(*(t_7 + b_8)) , float4(*(t_7 + _S487)) , _S488);
    float4 t1_0 = mix(float4(*(t_7 + _S489)) , float4(*(t_7 + _S490)) , _S488);
    float4 _S491 = float4(g_7.y) ;
    *lo_2 = mix(mix(float4(*(l_5 + b_8)) , float4(*(l_5 + _S487)) , _S488), mix(float4(*(l_5 + _S489)) , float4(*(l_5 + _S490)) , _S488), _S491).xyz;
    *to_0 = mix(t0_0, t1_0, _S491).xyz;
    return;
}

void atmos_aerial_0(FrameData_natural_0 device* f_33, float3 p_15, float2 px_3, float3 thread* l_6, float3 thread* tr_0, KernelContext_0 thread* kernelContext_42)
{
    *l_6 = float3(0.0f) ;
    float3 _S492 = float3(1.0f) ;
    *tr_0 = _S492;
    float t_8 = length(p_15) * 0.0010000000474974513f;
    if(t_8 <= 0.0f)
    {
        return;
    }
    float h_23 = (float4((&f_33->atmos_0)->radii_0) ).z;
    float _S493 = atm_rb_0(f_33);
    float _S494 = atm_rt_0(f_33);
    float te_0;
    if(h_23 > (_S494 - _S493))
    {
        float te_1 = atm_enter_top_0(_S493, _S494, h_23, dot(p_15 * float3((0.0010000000474974513f / t_8)) , (float4((&f_33->atmos_0)->up_0) ).xyz));
        bool _S495;
        if(te_1 < 0.0f)
        {
            _S495 = true;
        }
        else
        {
            _S495 = t_8 <= te_1;
        }
        if(_S495)
        {
            return;
        }
        te_0 = te_1;
    }
    else
    {
        te_0 = 0.0f;
    }
    float _S496 = atm_ap_slice_0(f_33, t_8 - te_0);
    float2 q_15 = clamp(px_3 / (float4(f_33->viewport_0) ).zw * float2(32.0f)  - float2(0.5f) , float2(0.0f) , float2(31.0f) );
    uint2 _S497 = min(uint2(q_15), uint2(30U) );
    float2 g_8 = q_15 - float2(_S497);
    packed_float4 device* _S498 = ed_buf_4((uint4((&f_33->atmos_0)->bufs2_0) ).x, kernelContext_42);
    packed_float4 device* _S499 = ed_buf_4((uint4((&f_33->atmos_0)->bufs2_0) ).y, kernelContext_42);
    thread float3 la_2;
    thread float3 ta_0;
    thread float3 lc_3;
    thread float3 tc_0;
    if(_S496 < 1.0f)
    {
        atm_froxel_xy_0(_S498, _S499, g_8, _S497, 0U, &lc_3, &tc_0);
        float3 _S500 = float3(_S496) ;
        *l_6 = lc_3 * _S500;
        *tr_0 = mix(_S492, tc_0, _S500);
        return;
    }
    float _S501 = min(_S496 - 1.0f, 31.0f);
    uint _S502 = min(uint(_S501), 30U);
    float gz_0 = _S501 - float(_S502);
    atm_froxel_xy_0(_S498, _S499, g_8, _S497, _S502, &la_2, &ta_0);
    atm_froxel_xy_0(_S498, _S499, g_8, _S497, _S502 + 1U, &lc_3, &tc_0);
    float3 _S503 = float3(gz_0) ;
    *l_6 = mix(la_2, lc_3, _S503);
    *tr_0 = mix(ta_0, tc_0, _S503);
    return;
}

float3 apply_aerial_0(FrameData_natural_0 device* f_34, float3 col_0, float3 p_16, float2 px_4, KernelContext_0 thread* kernelContext_43)
{
    bool _S504 = atmos_on_0(f_34);
    if(!_S504)
    {
        return col_0;
    }
    thread float3 l_7;
    thread float3 tr_1;
    atmos_aerial_0(f_34, p_16, px_4, &l_7, &tr_1, kernelContext_43);
    if(((uint4((&f_34->atmos_0)->cfg_1) ).y) == 1U)
    {
        return tr_1 * float3(0.5f) ;
    }
    if(((uint4((&f_34->atmos_0)->cfg_1) ).y) == 2U)
    {
        return l_7;
    }
    return col_0 * tr_1 + l_7;
}

uint shadow_cascade_0(FrameData_natural_0 device* f_35, float3 p_17, float dist_2)
{
    uint _S505 = uint((float4((&f_35->light_0)->cfg_0) ).x);
    bool _S506;
    if(((uint4((&f_35->light_0)->flags_0) ).x) == 0U)
    {
        _S506 = true;
    }
    else
    {
        _S506 = dist_2 >= ((float4((&f_35->light_0)->filter_0) ).y);
    }
    if(_S506)
    {
        return 4U;
    }
    float _S507 = 1.0f - (float4((&f_35->light_0)->cfg_0) ).w * 2.0f / (float4((&f_35->light_0)->cfg_0) ).y;
    uint c_17 = 0U;
    for(;;)
    {
        if(c_17 < _S505)
        {
        }
        else
        {
            break;
        }
        float _S508 = cascade_extent_0(f_35, c_17, p_17);
        if((_S508 / _S507) <= 1.0f)
        {
            return c_17;
        }
        c_17 = c_17 + 1U;
    }
    return 4U;
}

float4 terrain_pixel_0(FrameData_natural_0 device* f_36, const FsOut_0 thread* i_19, uint2 vis_1, float3 dir_8, KernelContext_0 thread* kernelContext_44)
{
    TerrainDraw_natural_0 device* _S509 = ed_buf_2(kernelContext_44->ed_c_0->t_draws_0, kernelContext_44);
    TerrainDraw_natural_0 device* _S510 = _S509 + ((vis_1.x) & 2147483647U);
    TerrainDraw_natural_0 d_30 = *_S510;
    thread TerrainDraw_natural_0 _S511 = *_S510;
    uint pool_18 = kernelContext_44->ed_c_0->t_pool_0;
    uint3 tri_0 = t_triangle_0((vis_1.y) & 268435455U);
    uint _S512 = tri_0.x;
    _S511 = d_30;
    float3 _S513 = t_vertex_0(pool_18, &_S511, _S512, kernelContext_44);
    uint _S514 = tri_0.y;
    float3 _S515 = t_vertex_0(pool_18, &_S511, _S514, kernelContext_44);
    float3 e1_1 = _S515 - _S513;
    uint _S516 = tri_0.z;
    float3 _S517 = t_vertex_0(pool_18, &_S511, _S516, kernelContext_44);
    float3 e2_1 = _S517 - _S513;
    float2 bb_0 = ray_bary_0(dir_8, _S513, e1_1, e2_1);
    float b1_0 = saturate(bb_0.x);
    float b2_0 = saturate(bb_0.y);
    float bs_0 = b1_0 + b2_0;
    float b1_1;
    float b2_1;
    if(bs_0 > 1.0f)
    {
        float b2_2 = b2_0 / bs_0;
        b1_1 = b1_0 / bs_0;
        b2_1 = b2_2;
    }
    else
    {
        b1_1 = b1_0;
        b2_1 = b2_0;
    }
    float3 p_18 = _S513 + e1_1 * float3(b1_1)  + e2_1 * float3(b2_1) ;
    float2 g_9 = t_vertex_grid_0(_S512) * float2((1.0f - b1_1 - b2_1))  + t_vertex_grid_0(_S514) * float2(b1_1)  + t_vertex_grid_0(_S516) * float2(b2_1) ;
    float _S518 = t_sample_spacing_0(&_S511, g_9);
    float _S519 = max(_S518, 0.0099999997764825821f);
    int rn_0 = clamp(int(1.2000000476837158f / _S519 + 0.5f), int(1), int(4));
    thread float3 up_4;
    thread float h_24;
    float3 _S520 = t_smooth_normal_0(pool_18, &_S511, g_9, rn_0, &up_4, &h_24, kernelContext_44);
    int r_8 = clamp(int(24.0f / _S519 + 0.5f), int(1), int(6));
    float3 nm_1;
    if(r_8 == rn_0)
    {
        nm_1 = _S520;
    }
    else
    {
        float3 _S521 = t_smooth_normal_0(pool_18, &_S511, g_9, r_8, &up_4, &h_24, kernelContext_44);
        nm_1 = _S521;
    }
    float3 _S522 = terrain_albedo_0(h_24, dot(nm_1, up_4));
    uint _S523 = (uint4((&_S511)->slots_0) ).z;
    bool _S524 = (_S523 & 8388608U) != 0U;
    float4 _S525 = float4(0.0f) ;
    bool _S526;
    if(_S524)
    {
        _S526 = (as_type<uint>(((float4((&_S511)->tex_0) ).z))) != 0U;
    }
    else
    {
        _S526 = false;
    }
    float4 cv_2;
    if(_S526)
    {
        float4 _S527 = t_canopy_bilin_0(pool_18, (as_type<uint>(((float4((&_S511)->tex_0) ).z))), g_9 * float2(4.0f) , kernelContext_44);
        cv_2 = _S527;
    }
    else
    {
        cv_2 = _S525;
    }
    float rough_3;
    float mat_ao_0;
    float3 n_11;
    float3 albedo_5;
    if((kernelContext_44->ed_c_0->t_mat_0) != 0U)
    {
        TerrainMat_natural_0 device* _S528 = ed_buf_3(kernelContext_44->ed_c_0->t_draws_0, kernelContext_44);
        thread TerrainMat_natural_0 _S529 = *(_S528 + kernelContext_44->ed_c_0->t_mat_0);
        float4 _S530 = float4((&_S511)->shape_0) ;
        float2 w_12 = (float4((&_S511)->tex_0) ).xy + (g_9 - float2(32.0f) ) * float2((_S530.x * _S530.y)) ;
        float3 _S531 = (float4((&_S511)->fu_0) ).xyz;
        float3 tu_5 = normalize(_S531 - up_4 * float3(dot(_S531, up_4)) );
        float3 _S532 = (float4((&_S511)->fv_0) ).xyz;
        float3 tv_6 = normalize(_S532 - up_4 * float3(dot(_S532, up_4)) );
        float2 _S533 = i_19->pos_0.xy;
        float3 _S534 = view_dir_0(f_36, _S533 + float2(1.0f, 0.0f));
        float2 bx_0 = ray_bary_0(_S534, _S513, e1_1, e2_1);
        float3 _S535 = view_dir_0(f_36, _S533 + float2(0.0f, 1.0f));
        float2 by_0 = ray_bary_0(_S535, _S513, e1_1, e2_1);
        float3 dpx_0 = _S513 + e1_1 * float3(bx_0.x)  + e2_1 * float3(bx_0.y)  - p_18;
        float3 dpy_0 = _S513 + e1_1 * float3(by_0.x)  + e2_1 * float3(by_0.y)  - p_18;
        float2 dwx_1 = float2(dot(dpx_0, tu_5), dot(dpx_0, tv_6));
        float2 dwy_1 = float2(dot(dpy_0, tu_5), dot(dpy_0, tv_6));
        float2 sp_1 = g_9 * float2(4.0f) ;
        int rf_0 = clamp(int(2.0f / _S519 + 0.5f), int(1), int(4));
        float _S536 = float(clamp(int(8.0f / _S519 + 0.5f), int(1), int(16)));
        float2 _S537 = float2(_S536, 0.0f);
        float _S538 = t_page_bilin_0(pool_18, &_S511, sp_1 + _S537, kernelContext_44);
        float _S539 = t_page_bilin_0(pool_18, &_S511, sp_1 - _S537, kernelContext_44);
        float _S540 = _S538 + _S539;
        float2 _S541 = float2(0.0f, _S536);
        float _S542 = t_page_bilin_0(pool_18, &_S511, sp_1 + _S541, kernelContext_44);
        float _S543 = _S540 + _S542;
        float _S544 = t_page_bilin_0(pool_18, &_S511, sp_1 - _S541, kernelContext_44);
        float lap_0 = _S543 + _S544 - 4.0f * h_24;
        if((length_squared_0(p_18)) < 1.6e+05f)
        {
            float _S545 = float(rf_0);
            float2 _S546 = float2(_S545, 0.0f);
            float _S547 = t_page_bilin_0(pool_18, &_S511, sp_1 + _S546, kernelContext_44);
            float _S548 = t_page_bilin_0(pool_18, &_S511, sp_1 - _S546, kernelContext_44);
            float _S549 = _S547 + _S548;
            float2 _S550 = float2(0.0f, _S545);
            float _S551 = t_page_bilin_0(pool_18, &_S511, sp_1 + _S550, kernelContext_44);
            float _S552 = _S549 + _S551;
            float _S553 = t_page_bilin_0(pool_18, &_S511, sp_1 - _S550, kernelContext_44);
            rough_3 = _S552 + _S553 - 4.0f * h_24;
        }
        else
        {
            rough_3 = 0.0f;
        }
        float dl_0 = _S536 * _S519;
        float df_0 = float(rf_0) * _S519;
        TSurf_0 _S554 = terrain_material_0(f_36, &_S529, &_S511, _S520, nm_1, up_4, h_24, lap_0 / (dl_0 * dl_0), rough_3 / (df_0 * df_0), w_12, dwx_1, dwy_1, dot(dpx_0, up_4), dot(dpy_0, up_4), tu_5, tv_6, length(p_18), _S524, cv_2, _S519, kernelContext_44);
        n_11 = _S554.n_1;
        albedo_5 = _S554.albedo_1;
        rough_3 = _S554.rough_0;
        mat_ao_0 = _S554.ao_2;
    }
    else
    {
        n_11 = _S520;
        albedo_5 = _S522;
        rough_3 = 0.92000001668930054f;
        mat_ao_0 = 1.0f;
    }
    if(_S524)
    {
        _S526 = (cv_2.x) > 0.0040000001899898052f;
    }
    else
    {
        _S526 = false;
    }
    float sun_k_0;
    if(_S526)
    {
        float4 _S555 = float4((&_S511)->shape_0) ;
        float2 w_13 = (float4((&_S511)->tex_0) ).xy + (g_9 - float2(32.0f) ) * float2((_S555.x * _S555.y)) ;
        float3 _S556 = (float4((&_S511)->fu_0) ).xyz;
        float3 tu_6 = normalize(_S556 - up_4 * float3(dot(_S556, up_4)) );
        float3 _S557 = (float4((&_S511)->fv_0) ).xyz;
        float3 tv_7 = normalize(_S557 - up_4 * float3(dot(_S557, up_4)) );
        float3 _S558 = view_dir_0(f_36, i_19->pos_0.xy + float2(1.0f, 0.0f));
        float _S559 = length(p_18);
        float px_iso_1 = _S559 * length(_S558 - dir_8);
        float px_m_1 = px_iso_1 / sqrt(max(abs(dot(dir_8, n_11)), 0.029999999329447746f));
        if(((float4((&f_36->wind_1)->cfg_2) ).z) > 0.5f)
        {
            _S526 = ((float4((&_S511)->polar_0) ).w) > 0.0f;
        }
        else
        {
            _S526 = false;
        }
        if(_S526)
        {
            sun_k_0 = (float4((&_S511)->polar_0) ).w / max(_S559, 1.0f);
        }
        else
        {
            sun_k_0 = -1.0f;
        }
        TForest_0 _S560 = t_forest_0(f_36, h_24, pool_18, &_S511, cv_2, g_9, w_13, tu_6, tv_7, up_4, n_11, albedo_5, (float4(f_36->sun_dir_0) ).xyz, px_m_1, px_iso_1, _S519, sun_k_0, kernelContext_44);
        float _S561 = mix(rough_3, 0.80000001192092896f, _S560.crown_0);
        float mat_ao_1 = mat_ao_0 * _S560.ao_3;
        n_11 = _S560.n_3;
        sun_k_0 = _S560.sun_1;
        albedo_5 = _S560.albedo_2;
        rough_3 = _S561;
        mat_ao_0 = mat_ao_1;
    }
    else
    {
        sun_k_0 = 1.0f;
    }
    float3 v_5 = - dir_8;
    float _S562 = dot(n_11, v_5);
    if(_S562 < 0.0f)
    {
        n_11 = normalize(n_11 - float3((1.0099999904632568f * _S562))  * v_5);
    }
    float zlin_1 = - (((float4(p_18, 1.0f)) * (matrix<float,int(4),int(4)> (f_36->view_0.data_0[int(0)][int(0)], f_36->view_0.data_0[int(1)][int(0)], f_36->view_0.data_0[int(2)][int(0)], f_36->view_0.data_0[int(3)][int(0)], f_36->view_0.data_0[int(0)][int(1)], f_36->view_0.data_0[int(1)][int(1)], f_36->view_0.data_0[int(2)][int(1)], f_36->view_0.data_0[int(3)][int(1)], f_36->view_0.data_0[int(0)][int(2)], f_36->view_0.data_0[int(1)][int(2)], f_36->view_0.data_0[int(2)][int(2)], f_36->view_0.data_0[int(3)][int(2)], f_36->view_0.data_0[int(0)][int(3)], f_36->view_0.data_0[int(1)][int(3)], f_36->view_0.data_0[int(2)][int(3)], f_36->view_0.data_0[int(3)][int(3)])))).z;
    float3 ng_2 = cross(e1_1, e2_1);
    float3 ng_3;
    if((length_squared_0(ng_2)) > 9.99999968265522539e-21f)
    {
        ng_3 = normalize(ng_2);
    }
    else
    {
        ng_3 = n_11;
    }
    if((dot(ng_3, v_5)) < 0.0f)
    {
        ng_3 = - ng_3;
    }
    float2 _S563 = i_19->pos_0.xy;
    float _S564 = sun_shadow_0(f_36, kernelContext_44->ed_c_0->shadow_0, p_18, ng_3, zlin_1, _S563, kernelContext_44);
    float sun_vis_1;
    if(_S524)
    {
        sun_vis_1 = min(_S564, sun_k_0);
    }
    else
    {
        sun_vis_1 = _S564;
    }
    float2 _S565 = ao_upsample_0(f_36, kernelContext_44->ed_c_0->ao_0, _S563, zlin_1, kernelContext_44);
    float ao_6 = _S565.x * mat_ao_0;
    float sv_3 = _S565.y;
    float3 _S566 = sun_light_at_0(f_36, p_18, kernelContext_44);
    float3 _S567 = shade_pbr_0(f_36, _S566, albedo_5, 0.0f, rough_3, ao_6, sv_3, sun_vis_1, n_11, v_5, kernelContext_44);
    float3 _S568 = apply_aerial_0(f_36, _S567, p_18, _S563, kernelContext_44);
    float3 col_1;
    if(((uint4((&f_36->light_0)->flags_0) ).w) == 1U)
    {
        uint _S569 = shadow_cascade_0(f_36, p_18, zlin_1);
        col_1 = TINT_0[_S569] * float3((0.039999999105930328f + 0.5f * sun_vis_1)) ;
    }
    else
    {
        if(((uint4((&f_36->light_0)->flags_0) ).w) == 2U)
        {
            col_1 = float3((0.5f * ao_6)) ;
        }
        else
        {
            if(((uint4((&f_36->light_0)->flags_0) ).w) == 4U)
            {
                col_1 = float3((0.5f * sv_3)) ;
            }
            else
            {
                col_1 = _S568;
            }
        }
    }
    if(_S524)
    {
        _S526 = (_S523 & 16777216U) != 0U;
    }
    else
    {
        _S526 = false;
    }
    if(_S526)
    {
        _S526 = (cv_2.x) > 0.0f;
    }
    else
    {
        _S526 = false;
    }
    if(_S526)
    {
        col_1 = mix(col_1, float3(saturate(cv_2.y / 40.0f), cv_2.x, cv_2.z) * float3(0.60000002384185791f) , float3(0.75f) );
    }
    return float4(col_1, 1.0f);
}

float3 t_target_even_1(uint pool_19, const TerrainDraw_natural_0 thread* d_31, uint i_20, uint j_10, KernelContext_0 thread* kernelContext_45)
{
    bool border_1;
    if(i_20 == 0U)
    {
        border_1 = true;
    }
    else
    {
        border_1 = j_10 == 0U;
    }
    if(border_1)
    {
        border_1 = true;
    }
    else
    {
        border_1 = i_20 == 64U;
    }
    if(border_1)
    {
        border_1 = true;
    }
    else
    {
        border_1 = j_10 == 64U;
    }
    bool own_1;
    if(border_1)
    {
        own_1 = true;
    }
    else
    {
        own_1 = ((uint4(d_31->slots_0) ).y) == 4294967295U;
    }
    uint4 _S570 = uint4(d_31->slots_0) ;
    uint _S571 = _S570.z;
    uint qx_1;
    if((_S571 & 16U) != 0U)
    {
        qx_1 = 128U;
    }
    else
    {
        qx_1 = 0U;
    }
    uint qy_1;
    if((_S571 & 32U) != 0U)
    {
        qy_1 = 128U;
    }
    else
    {
        qy_1 = 0U;
    }
    float h_25;
    if(own_1)
    {
        float _S572 = t_own_h_0(pool_19, d_31, i_20, j_10, kernelContext_45);
        h_25 = _S572;
    }
    else
    {
        float4 _S573 = float4(d_31->hq_0) ;
        float _S574 = t_page_0(pool_19, _S570.y, qx_1 + 2U * i_20, qy_1 + 2U * j_10, _S573.z, _S573.w, kernelContext_45);
        h_25 = _S574;
    }
    thread float3 dir_9;
    float3 _S575 = t_lattice_0(d_31, i_20, j_10, h_25, &dir_9);
    return _S575;
}

float3 t_base_1(uint pool_20, const TerrainDraw_natural_0 thread* d_32, uint i_21, uint j_11, KernelContext_0 thread* kernelContext_46)
{
    float _S576 = t_own_h_0(pool_20, d_32, i_21, j_11, kernelContext_46);
    thread float3 dir_10;
    float3 _S577 = t_lattice_0(d_32, i_21, j_11, _S576, &dir_10);
    return _S577;
}

float3 t_coarse_even_1(uint pool_21, const TerrainDraw_natural_0 thread* d_33, uint e_9, uint2 a_17, uint2 b_9, uint2 c_18, KernelContext_0 thread* kernelContext_47)
{
    float3 _S578 = t_base_1(pool_21, d_33, a_17.x, a_17.y, kernelContext_47);
    if((e_9 & 3U) == 0U)
    {
        return _S578;
    }
    float m_5 = t_morph_0((float4(d_33->origin_0) ).xyz + _S578, (float4(d_33->morph_0) ).zw);
    float3 _S579 = t_base_1(pool_21, d_33, b_9.x, b_9.y, kernelContext_47);
    float3 _S580 = t_base_1(pool_21, d_33, c_18.x, c_18.y, kernelContext_47);
    return mix(_S578, float3(0.5f)  * (_S579 + _S580), float3(m_5) );
}

float3 t_target_1(uint pool_22, const TerrainDraw_natural_0 thread* d_34, uint i_22, uint j_12, KernelContext_0 thread* kernelContext_48)
{
    bool ei_1 = (i_22 & 1U) == 0U;
    bool ej_1 = (j_12 & 1U) == 0U;
    bool _S581;
    if(ei_1)
    {
        _S581 = ej_1;
    }
    else
    {
        _S581 = false;
    }
    if(_S581)
    {
        float3 _S582 = t_target_even_1(pool_22, d_34, i_22, j_12, kernelContext_48);
        return _S582;
    }
    if(!ei_1)
    {
        _S581 = ej_1;
    }
    else
    {
        _S581 = false;
    }
    if(_S581)
    {
        float3 _S583 = t_target_even_1(pool_22, d_34, i_22 - 1U, j_12, kernelContext_48);
        float3 _S584 = t_target_even_1(pool_22, d_34, i_22 + 1U, j_12, kernelContext_48);
        return float3(0.5f)  * (_S583 + _S584);
    }
    if(ei_1)
    {
        _S581 = !ej_1;
    }
    else
    {
        _S581 = false;
    }
    if(_S581)
    {
        float3 _S585 = t_target_even_1(pool_22, d_34, i_22, j_12 - 1U, kernelContext_48);
        float3 _S586 = t_target_even_1(pool_22, d_34, i_22, j_12 + 1U, kernelContext_48);
        return float3(0.5f)  * (_S585 + _S586);
    }
    float3 _S587 = t_target_even_1(pool_22, d_34, i_22 - 1U, j_12 - 1U, kernelContext_48);
    float3 _S588 = t_target_even_1(pool_22, d_34, i_22 + 1U, j_12 + 1U, kernelContext_48);
    return float3(0.5f)  * (_S587 + _S588);
}

float3 t_coarse_1(uint pool_23, const TerrainDraw_natural_0 thread* d_35, uint edge_3, uint e_10, KernelContext_0 thread* kernelContext_49)
{
    if((e_10 & 1U) == 0U)
    {
        float3 _S589 = t_coarse_even_1(pool_23, d_35, e_10, t_edge_at_0(edge_3, e_10), t_edge_at_0(edge_3, e_10 - 2U), t_edge_at_0(edge_3, e_10 + 2U), kernelContext_49);
        return _S589;
    }
    uint _S590 = e_10 - 1U;
    uint2 _S591 = t_edge_at_0(edge_3, _S590);
    uint _S592 = e_10 + 1U;
    uint2 _S593 = t_edge_at_0(edge_3, _S592);
    float3 _S594 = t_coarse_even_1(pool_23, d_35, _S590, _S591, t_edge_at_0(edge_3, e_10 - 3U), _S593, kernelContext_49);
    float3 _S595 = t_coarse_even_1(pool_23, d_35, _S592, _S593, _S591, t_edge_at_0(edge_3, e_10 + 3U), kernelContext_49);
    return float3(0.5f)  * (_S594 + _S595);
}

float3 t_grid_vertex_1(uint pool_24, const TerrainDraw_natural_0 thread* d_36, uint i_23, uint j_13, KernelContext_0 thread* kernelContext_50)
{
    uint fl_2 = (uint4(d_36->slots_0) ).z;
    uint edge_4;
    uint e_11;
    if(i_23 == 0U)
    {
        edge_4 = 0U;
        e_11 = j_13;
    }
    else
    {
        if(i_23 == 64U)
        {
            edge_4 = 1U;
            e_11 = j_13;
        }
        else
        {
            if(j_13 == 0U)
            {
                edge_4 = 2U;
                e_11 = i_23;
            }
            else
            {
                if(j_13 == 64U)
                {
                    edge_4 = 3U;
                    e_11 = i_23;
                }
                else
                {
                    edge_4 = 4U;
                    e_11 = 0U;
                }
            }
        }
    }
    if(edge_4 < 4U)
    {
        uint E_1 = 1U << ((fl_2 >> (10U + 3U * edge_4)) & 7U);
        bool _S596;
        if((fl_2 & (1U << edge_4)) != 0U)
        {
            _S596 = E_1 <= 2U;
        }
        else
        {
            _S596 = false;
        }
        if(_S596)
        {
            float3 _S597 = t_coarse_1(pool_24, d_36, edge_4, e_11, kernelContext_50);
            return _S597;
        }
        if(E_1 > 1U)
        {
            uint lo_3 = e_11 & (~(E_1 - 1U));
            if(lo_3 == e_11)
            {
                float3 _S598 = t_base_1(pool_24, d_36, i_23, j_13, kernelContext_50);
                return _S598;
            }
            uint2 a_18 = t_edge_at_0(edge_4, lo_3);
            uint2 b_10 = t_edge_at_0(edge_4, lo_3 + E_1);
            float3 _S599 = t_base_1(pool_24, d_36, a_18.x, a_18.y, kernelContext_50);
            float3 _S600 = t_base_1(pool_24, d_36, b_10.x, b_10.y, kernelContext_50);
            return mix(_S599, _S600, float3((float(e_11 - lo_3) / float(E_1))) );
        }
    }
    float3 _S601 = t_base_1(pool_24, d_36, i_23, j_13, kernelContext_50);
    if((fl_2 & 4194304U) != 0U)
    {
        return _S601;
    }
    float m_6 = t_morph_0((float4(d_36->origin_0) ).xyz + _S601, (float4(d_36->morph_0) ).xy);
    if(m_6 <= 0.0f)
    {
        return _S601;
    }
    float3 _S602 = t_target_1(pool_24, d_36, i_23, j_13, kernelContext_50);
    return mix(_S601, _S602, float3(m_6) );
}

float3 t_vertex_1(uint pool_25, const TerrainDraw_natural_0 thread* d_37, uint v_6, KernelContext_0 thread* kernelContext_51)
{
    if(v_6 < 4225U)
    {
        float3 _S603 = (float4(d_37->origin_0) ).xyz;
        float3 _S604 = t_grid_vertex_1(pool_25, d_37, v_6 % 65U, v_6 / 65U, kernelContext_51);
        return _S603 + _S604;
    }
    uint2 ij_1 = t_ring_0(v_6 - 4225U);
    float3 _S605 = t_grid_vertex_1(pool_25, d_37, ij_1.x, ij_1.y, kernelContext_51);
    float depth_1;
    if((((uint4(d_37->slots_0) ).z) & 64U) != 0U)
    {
        depth_1 = (float4(d_37->origin_0) ).w;
    }
    else
    {
        depth_1 = 0.0f;
    }
    float3 _S606 = (float4(d_37->origin_0) ).xyz;
    float3 _S607 = _S606 + _S605;
    float4 _S608 = float4(d_37->dir_c_0) ;
    return _S607 - normalize(_S607 - (_S606 - _S608.xyz * float3(_S608.w) )) * float3(depth_1) ;
}

constant array<float3, int(5)> TINT_1 = { { float3(1.0f, 0.25f, 0.25f), float3(0.25f, 1.0f, 0.25f), float3(0.30000001192092896f, 0.44999998807907104f, 1.0f), float3(1.0f, 1.0f, 0.25f), float3(0.60000002384185791f, 0.60000002384185791f, 0.60000002384185791f) } };
float4 terrain_pixel_1(FrameData_natural_0 device* f_37, const FsOut_0 thread* i_24, uint2 vis_2, float3 dir_11, KernelContext_0 thread* kernelContext_52)
{
    TerrainDraw_natural_0 device* _S609 = ed_buf_2(kernelContext_52->ed_c_0->t_draws_0, kernelContext_52);
    TerrainDraw_natural_0 device* _S610 = _S609 + ((vis_2.x) & 2147483647U);
    TerrainDraw_natural_0 d_38 = *_S610;
    thread TerrainDraw_natural_0 _S611 = *_S610;
    uint pool_26 = kernelContext_52->ed_c_0->t_pool_0;
    uint3 tri_1 = t_triangle_0((vis_2.y) & 268435455U);
    uint _S612 = tri_1.x;
    _S611 = d_38;
    float3 _S613 = t_vertex_1(pool_26, &_S611, _S612, kernelContext_52);
    uint _S614 = tri_1.y;
    float3 _S615 = t_vertex_1(pool_26, &_S611, _S614, kernelContext_52);
    float3 e1_2 = _S615 - _S613;
    uint _S616 = tri_1.z;
    float3 _S617 = t_vertex_1(pool_26, &_S611, _S616, kernelContext_52);
    float3 e2_2 = _S617 - _S613;
    float2 bb_1 = ray_bary_0(dir_11, _S613, e1_2, e2_2);
    float b1_2 = saturate(bb_1.x);
    float b2_3 = saturate(bb_1.y);
    float bs_1 = b1_2 + b2_3;
    float b1_3;
    float b2_4;
    if(bs_1 > 1.0f)
    {
        float b2_5 = b2_3 / bs_1;
        b1_3 = b1_2 / bs_1;
        b2_4 = b2_5;
    }
    else
    {
        b1_3 = b1_2;
        b2_4 = b2_3;
    }
    float3 p_19 = _S613 + e1_2 * float3(b1_3)  + e2_2 * float3(b2_4) ;
    float2 g_10 = t_vertex_grid_0(_S612) * float2((1.0f - b1_3 - b2_4))  + t_vertex_grid_0(_S614) * float2(b1_3)  + t_vertex_grid_0(_S616) * float2(b2_4) ;
    float _S618 = t_sample_spacing_0(&_S611, g_10);
    float _S619 = max(_S618, 0.0099999997764825821f);
    int rn_1 = clamp(int(1.2000000476837158f / _S619 + 0.5f), int(1), int(4));
    thread float3 up_5;
    thread float h_26;
    float3 _S620 = t_smooth_normal_0(pool_26, &_S611, g_10, rn_1, &up_5, &h_26, kernelContext_52);
    int r_9 = clamp(int(24.0f / _S619 + 0.5f), int(1), int(6));
    float3 nm_2;
    if(r_9 == rn_1)
    {
        nm_2 = _S620;
    }
    else
    {
        float3 _S621 = t_smooth_normal_0(pool_26, &_S611, g_10, r_9, &up_5, &h_26, kernelContext_52);
        nm_2 = _S621;
    }
    float3 _S622 = terrain_albedo_0(h_26, dot(nm_2, up_5));
    float4 _S623 = float4(0.0f) ;
    float rough_4;
    float mat_ao_2;
    float3 n_12;
    float3 albedo_6;
    if((kernelContext_52->ed_c_0->t_mat_0) != 0U)
    {
        TerrainMat_natural_0 device* _S624 = ed_buf_3(kernelContext_52->ed_c_0->t_draws_0, kernelContext_52);
        thread TerrainMat_natural_0 _S625 = *(_S624 + kernelContext_52->ed_c_0->t_mat_0);
        float4 _S626 = float4((&_S611)->shape_0) ;
        float2 w_14 = (float4((&_S611)->tex_0) ).xy + (g_10 - float2(32.0f) ) * float2((_S626.x * _S626.y)) ;
        float3 _S627 = (float4((&_S611)->fu_0) ).xyz;
        float3 tu_7 = normalize(_S627 - up_5 * float3(dot(_S627, up_5)) );
        float3 _S628 = (float4((&_S611)->fv_0) ).xyz;
        float3 tv_8 = normalize(_S628 - up_5 * float3(dot(_S628, up_5)) );
        float2 _S629 = i_24->pos_0.xy;
        float3 _S630 = view_dir_0(f_37, _S629 + float2(1.0f, 0.0f));
        float2 bx_1 = ray_bary_0(_S630, _S613, e1_2, e2_2);
        float3 _S631 = view_dir_0(f_37, _S629 + float2(0.0f, 1.0f));
        float2 by_1 = ray_bary_0(_S631, _S613, e1_2, e2_2);
        float3 dpx_1 = _S613 + e1_2 * float3(bx_1.x)  + e2_2 * float3(bx_1.y)  - p_19;
        float3 dpy_1 = _S613 + e1_2 * float3(by_1.x)  + e2_2 * float3(by_1.y)  - p_19;
        float2 dwx_2 = float2(dot(dpx_1, tu_7), dot(dpx_1, tv_8));
        float2 dwy_2 = float2(dot(dpy_1, tu_7), dot(dpy_1, tv_8));
        float2 sp_2 = g_10 * float2(4.0f) ;
        int rf_1 = clamp(int(2.0f / _S619 + 0.5f), int(1), int(4));
        float _S632 = float(clamp(int(8.0f / _S619 + 0.5f), int(1), int(16)));
        float2 _S633 = float2(_S632, 0.0f);
        float _S634 = t_page_bilin_0(pool_26, &_S611, sp_2 + _S633, kernelContext_52);
        float _S635 = t_page_bilin_0(pool_26, &_S611, sp_2 - _S633, kernelContext_52);
        float _S636 = _S634 + _S635;
        float2 _S637 = float2(0.0f, _S632);
        float _S638 = t_page_bilin_0(pool_26, &_S611, sp_2 + _S637, kernelContext_52);
        float _S639 = _S636 + _S638;
        float _S640 = t_page_bilin_0(pool_26, &_S611, sp_2 - _S637, kernelContext_52);
        float lap_1 = _S639 + _S640 - 4.0f * h_26;
        if((length_squared_0(p_19)) < 1.6e+05f)
        {
            float _S641 = float(rf_1);
            float2 _S642 = float2(_S641, 0.0f);
            float _S643 = t_page_bilin_0(pool_26, &_S611, sp_2 + _S642, kernelContext_52);
            float _S644 = t_page_bilin_0(pool_26, &_S611, sp_2 - _S642, kernelContext_52);
            float _S645 = _S643 + _S644;
            float2 _S646 = float2(0.0f, _S641);
            float _S647 = t_page_bilin_0(pool_26, &_S611, sp_2 + _S646, kernelContext_52);
            float _S648 = _S645 + _S647;
            float _S649 = t_page_bilin_0(pool_26, &_S611, sp_2 - _S646, kernelContext_52);
            rough_4 = _S648 + _S649 - 4.0f * h_26;
        }
        else
        {
            rough_4 = 0.0f;
        }
        float dl_1 = _S632 * _S619;
        float df_1 = float(rf_1) * _S619;
        TSurf_0 _S650 = terrain_material_0(f_37, &_S625, &_S611, _S620, nm_2, up_5, h_26, lap_1 / (dl_1 * dl_1), rough_4 / (df_1 * df_1), w_14, dwx_2, dwy_2, dot(dpx_1, up_5), dot(dpy_1, up_5), tu_7, tv_8, length(p_19), false, _S623, _S619, kernelContext_52);
        n_12 = _S650.n_1;
        albedo_6 = _S650.albedo_1;
        rough_4 = _S650.rough_0;
        mat_ao_2 = _S650.ao_2;
    }
    else
    {
        n_12 = _S620;
        albedo_6 = _S622;
        rough_4 = 0.92000001668930054f;
        mat_ao_2 = 1.0f;
    }
    float3 v_7 = - dir_11;
    float _S651 = dot(n_12, v_7);
    if(_S651 < 0.0f)
    {
        n_12 = normalize(n_12 - float3((1.0099999904632568f * _S651))  * v_7);
    }
    float zlin_2 = - (((float4(p_19, 1.0f)) * (matrix<float,int(4),int(4)> (f_37->view_0.data_0[int(0)][int(0)], f_37->view_0.data_0[int(1)][int(0)], f_37->view_0.data_0[int(2)][int(0)], f_37->view_0.data_0[int(3)][int(0)], f_37->view_0.data_0[int(0)][int(1)], f_37->view_0.data_0[int(1)][int(1)], f_37->view_0.data_0[int(2)][int(1)], f_37->view_0.data_0[int(3)][int(1)], f_37->view_0.data_0[int(0)][int(2)], f_37->view_0.data_0[int(1)][int(2)], f_37->view_0.data_0[int(2)][int(2)], f_37->view_0.data_0[int(3)][int(2)], f_37->view_0.data_0[int(0)][int(3)], f_37->view_0.data_0[int(1)][int(3)], f_37->view_0.data_0[int(2)][int(3)], f_37->view_0.data_0[int(3)][int(3)])))).z;
    float3 ng_4 = cross(e1_2, e2_2);
    float3 ng_5;
    if((length_squared_0(ng_4)) > 9.99999968265522539e-21f)
    {
        ng_5 = normalize(ng_4);
    }
    else
    {
        ng_5 = n_12;
    }
    if((dot(ng_5, v_7)) < 0.0f)
    {
        ng_5 = - ng_5;
    }
    float2 _S652 = i_24->pos_0.xy;
    float _S653 = sun_shadow_0(f_37, kernelContext_52->ed_c_0->shadow_0, p_19, ng_5, zlin_2, _S652, kernelContext_52);
    float2 _S654 = ao_upsample_0(f_37, kernelContext_52->ed_c_0->ao_0, _S652, zlin_2, kernelContext_52);
    float ao_7 = _S654.x * mat_ao_2;
    float sv_4 = _S654.y;
    float3 _S655 = sun_light_at_0(f_37, p_19, kernelContext_52);
    float3 _S656 = shade_pbr_0(f_37, _S655, albedo_6, 0.0f, rough_4, ao_7, sv_4, _S653, n_12, v_7, kernelContext_52);
    float3 _S657 = apply_aerial_0(f_37, _S656, p_19, _S652, kernelContext_52);
    float3 col_2;
    if(((uint4((&f_37->light_0)->flags_0) ).w) == 1U)
    {
        uint _S658 = shadow_cascade_0(f_37, p_19, zlin_2);
        col_2 = TINT_1[_S658] * float3((0.039999999105930328f + 0.5f * _S653)) ;
    }
    else
    {
        if(((uint4((&f_37->light_0)->flags_0) ).w) == 2U)
        {
            col_2 = float3((0.5f * ao_7)) ;
        }
        else
        {
            if(((uint4((&f_37->light_0)->flags_0) ).w) == 4U)
            {
                col_2 = float3((0.5f * sv_4)) ;
            }
            else
            {
                col_2 = _S657;
            }
        }
    }
    return float4(col_2, 1.0f);
}

constant array<float3, int(5)> TINT_2 = { { float3(1.0f, 0.25f, 0.25f), float3(0.25f, 1.0f, 0.25f), float3(0.30000001192092896f, 0.44999998807907104f, 1.0f), float3(1.0f, 1.0f, 0.25f), float3(0.60000002384185791f, 0.60000002384185791f, 0.60000002384185791f) } };
float3 sky_rad_1(FrameData_natural_0 device* f_38, float3 d_39, float disc_k_1)
{
    float3 sun_4 = (float4(f_38->sun_dir_0) ).xyz;
    float h_27 = d_39.y;
    float t_9 = pow(saturate(1.0f - max(h_27, 0.0f)), 6.5f);
    float mu_7 = dot(d_39, sun_4);
    float warm_1 = saturate(1.0f - sun_4.y * 2.5f);
    float3 c_19 = mix((float4(f_38->sky_zenith_0) ).xyz, (float4(f_38->sky_horizon_0) ).xyz, float3(t_9) ) + float3(1.0f, 0.41999998688697815f, 0.14000000059604645f) * float3(pow(saturate(mu_7 * 0.5f + 0.5f), 5.0f))  * float3(warm_1)  * float3(0.85000002384185791f)  * float3((0.40000000596046448f + t_9)) ;
    float3 _S659 = float3((float4(f_38->sun_dir_0) ).w) ;
    return mix(mix(c_19, c_19 * float3(1.0f, 0.69999998807907104f, 0.55000001192092896f), float3((warm_1 * 0.40000000596046448f)) ) + float3(1.0f, 0.89999997615814209f, 0.69999998807907104f) * float3(pow(saturate(mu_7), 28.0f))  * float3(0.34999999403953552f) , (float4(f_38->ground_color_0) ).xyz, float3(saturate(- h_27 * 6.0f)) ) * _S659 + float3(1.0f, 0.94999998807907104f, 0.85000002384185791f) * float3((smoothstep(0.99965000152587891f, 0.99984997510910034f, mu_7) * step(0.0f, h_27)))  * float3(60.0f)  * _S659 * float3(disc_k_1) ;
}

float atm_rt_1(FrameData_natural_0 device* f_39)
{
    return (float4((&f_39->atmos_0)->radii_0) ).y;
}

bool atm_enter_0(FrameData_natural_0 device* f_40, float thread* h_28, float thread* mu_8, float thread* mu_s_3, float nu_1, float thread* te_2)
{
    *te_2 = 0.0f;
    float _S660 = atm_rb_1(f_40);
    float _S661 = atm_rt_1(f_40);
    float _S662 = _S661 - _S660;
    if((*h_28) <= _S662)
    {
        return true;
    }
    float _S663 = atm_enter_top_0(_S660, _S661, *h_28, *mu_8);
    *te_2 = _S663;
    if(_S663 < 0.0f)
    {
        return false;
    }
    float r_10 = _S660 + *h_28;
    *mu_8 = clamp((r_10 * *mu_8 + *te_2) / _S661, -1.0f, 1.0f);
    *mu_s_3 = clamp((r_10 * *mu_s_3 + *te_2 * nu_1) / _S661, -1.0f, 1.0f);
    *h_28 = _S662;
    return true;
}

bool atm_hits_ground_0(float rb_5, float h_29, float mu_9)
{
    float rm_0 = (rb_5 + h_29) * mu_9;
    bool _S664;
    if(mu_9 < 0.0f)
    {
        _S664 = (rm_0 * rm_0) >= (h_29 * (2.0f * rb_5 + h_29));
    }
    else
    {
        _S664 = false;
    }
    return _S664;
}

float3 atm_transmittance_1(FrameData_natural_0 device* f_41, float h_30, float mu_10, KernelContext_0 thread* kernelContext_53)
{
    float _S665 = atm_rb_0(f_41);
    float _S666 = atm_rt_0(f_41);
    float2 uv_3 = atm_trans_uv_0(_S665, _S666, h_30, mu_10);
    packed_float4 device* _S667 = ed_buf_4((uint4((&f_41->atmos_0)->bufs_0) ).x, kernelContext_53);
    float4 _S668 = atm_bilinear_0(_S667, 256U, 0U, 64U, uv_3);
    return _S668.xyz;
}

float3 atmos_sky_1(FrameData_natural_0 device* f_42, float3 d_40, KernelContext_0 thread* kernelContext_54)
{
    float3 _S669 = atm_sky_view_0(f_42, (uint4((&f_42->atmos_0)->bufs_0) ).z, 192U, 108U, (float4((&f_42->atmos_0)->radii_0) ).z, d_40, kernelContext_54);
    return _S669;
}

float3 atmos_sky_pixel_0(FrameData_natural_0 device* f_43, float3 d_41, float disc_k_2, KernelContext_0 thread* kernelContext_55)
{
    float3 _S670 = atmos_sky_1(f_43, d_41, kernelContext_55);
    float nu_2 = dot(d_41, (float4(f_43->sun_dir_0) ).xyz);
    float cr_0 = (float4((&f_43->atmos_0)->sun_0) ).w;
    float _S671 = cr_0 - 1.99999999495048542e-06f;
    float3 c_20;
    if(nu_2 > _S671)
    {
        thread float h_31 = (float4((&f_43->atmos_0)->radii_0) ).z;
        thread float mu_11 = dot(d_41, (float4((&f_43->atmos_0)->up_0) ).xyz);
        thread float mu_s_4 = (float4((&f_43->atmos_0)->up_0) ).w;
        float3 _S672 = float3(1.0f) ;
        thread float te_3;
        bool _S673 = atm_enter_0(f_43, &h_31, &mu_11, &mu_s_4, nu_2, &te_3);
        if(_S673)
        {
            float _S674 = atm_rb_0(f_43);
            if(atm_hits_ground_0(_S674, h_31, mu_11))
            {
                c_20 = float3(0.0f) ;
            }
            else
            {
                float3 _S675 = atm_transmittance_1(f_43, h_31, mu_11, kernelContext_55);
                c_20 = _S675;
            }
        }
        else
        {
            c_20 = _S672;
        }
        float sr2_0 = 1.0f - cr_0 * cr_0;
        c_20 = _S670 + min((float4((&f_43->atmos_0)->sun_0) ).xyz * c_20 / float3((3.1415927410125732f * sr2_0))  * float3((1.0f - 0.60000002384185791f * (1.0f - sqrt(1.0f - saturate((1.0f - nu_2 * nu_2) / sr2_0)))))  * float3(smoothstep(_S671, cr_0 + 1.99999999495048542e-06f, nu_2))  * float3(disc_k_2) , float3(30000.0f) );
    }
    else
    {
        c_20 = _S670;
    }
    return c_20;
}

float3 sky_pixel_0(FrameData_natural_0 device* f_44, float3 d_42, KernelContext_0 thread* kernelContext_56)
{
    bool _S676 = atmos_on_0(f_44);
    if(!_S676)
    {
        float3 _S677 = sky_rad_1(f_44, d_42, (float4((&f_44->light_0)->sky_0) ).x);
        return _S677;
    }
    if(((uint4((&f_44->atmos_0)->cfg_1) ).y) == 1U)
    {
        thread float h_32 = (float4((&f_44->atmos_0)->radii_0) ).z;
        thread float mu_12 = dot(d_42, (float4((&f_44->atmos_0)->up_0) ).xyz);
        thread float mu_s_5 = (float4((&f_44->atmos_0)->up_0) ).w;
        thread float te_4;
        bool _S678 = atm_enter_0(f_44, &h_32, &mu_12, &mu_s_5, dot(d_42, (float4(f_44->sun_dir_0) ).xyz), &te_4);
        if(!_S678)
        {
            return float3(0.5f) ;
        }
        float _S679 = atm_rb_0(f_44);
        float3 _S680;
        if(atm_hits_ground_0(_S679, h_32, mu_12))
        {
            _S680 = float3(0.0f) ;
        }
        else
        {
            float3 _S681 = atm_transmittance_1(f_44, h_32, mu_12, kernelContext_56);
            _S680 = _S681 * float3(0.5f) ;
        }
        return _S680;
    }
    float _S682;
    if(((uint4((&f_44->atmos_0)->cfg_1) ).y) == 2U)
    {
        _S682 = 0.0f;
    }
    else
    {
        _S682 = (float4((&f_44->light_0)->sky_0) ).x;
    }
    float3 _S683 = atmos_sky_pixel_0(f_44, d_42, _S682, kernelContext_56);
    return _S683;
}

struct GrassClump_0
{
    float px_5;
    float py_0;
    float pz_0;
    uint node_kind_0;
    uint up_6;
    uint facing_0;
    uint seed_0;
    uint hw_0;
};

GrassClump_0 device* ed_buf_5(uint i_25, KernelContext_0 thread* kernelContext_57)
{
    return (GrassClump_0 device*)kernelContext_57->ed_buffers_0[i_25];
}

struct GrassType_natural_0
{
    packed_float4 a_19;
    packed_float4 b_11;
    packed_float4 root_0;
    packed_float4 mid_0;
    packed_float4 tip_0;
    packed_float4 dry_0;
};

GrassType_natural_0 device* ed_buf_6(uint i_26, KernelContext_0 thread* kernelContext_58)
{
    return (GrassType_natural_0 device*)kernelContext_58->ed_buffers_0[i_26];
}

struct GrassBlade_0
{
    float3 p0_1;
    float3 p1_0;
    float3 p2_0;
    float3 side_1;
    float3 up_7;
    float w_15;
    float h_33;
    float head_0;
    float head_w_0;
    float shade_0;
    float t_ao_0;
};

float2 unpack_snorm2x16_0(uint p_20)
{
    return clamp(float2(int2(int(p_20 << 16U) >> 16U, int(p_20) >> 16U)) / float2(32767.0f) , float2(-1.0f) , float2(1.0f) );
}

float3 oct_decode_0(uint packed_0)
{
    float2 e_12 = unpack_snorm2x16_0(packed_0);
    float _S684 = e_12.x;
    float _S685 = e_12.y;
    float _S686 = 1.0f - abs(_S684) - abs(_S685);
    thread float3 n_13 = float3(_S684, _S685, _S686);
    float t_10 = saturate(- _S686);
    float _S687;
    if(_S684 >= 0.0f)
    {
        _S687 = - t_10;
    }
    else
    {
        _S687 = t_10;
    }
    n_13.x = n_13.x + _S687;
    if((n_13.y) >= 0.0f)
    {
        _S687 = - t_10;
    }
    else
    {
        _S687 = t_10;
    }
    n_13.y = n_13.y + _S687;
    return normalize(n_13);
}

float2 grass_lod_0(FrameData_natural_0 device* f_45, float d_43, float blades_0)
{
    float q_16 = (float4((&f_45->grass_0)->lod_0) ).x / max(d_43, 0.0010000000474974513f);
    float raw_0 = blades_0 * min(q_16 * q_16, 1.0f);
    float _S688 = min((float4((&f_45->grass_0)->lod_0) ).z, blades_0);
    return float2(max(raw_0, _S688), min(raw_0 / max(_S688, 0.0010000000474974513f), 1.0f));
}

uint grass_hash_0(uint x_4)
{
    uint s_17 = x_4 * 747796405U + 2891336453U;
    uint w_16 = ((s_17 >> ((s_17 >> 28U) + 4U)) ^ s_17) * 277803737U;
    return (w_16 >> 22U) ^ w_16;
}

float grass_rand_0(uint h_34)
{
    return float(h_34 >> 8U) * 5.9604644775390625e-08f;
}

GrassBlade_0 grass_blade_0(FrameData_natural_0 device* f_46, const GrassClump_0 thread* c_21, const GrassType_natural_0 thread* ty_0, float3 node_0, uint k_10, float lod_mul_0)
{
    float3 up_8 = oct_decode_0(c_21->up_6);
    float3 rt_3 = normalize(cross(up_8, oct_decode_0(c_21->facing_0)));
    float3 fa_0 = cross(rt_3, up_8);
    float3 base_0 = node_0 + float3(c_21->px_5, c_21->py_0, c_21->pz_0);
    float d_44 = length(base_0);
    float4 _S689 = float4(ty_0->a_19) ;
    float blades_1 = _S689.w;
    float2 _S690 = grass_lod_0(f_46, d_44, blades_1);
    float _S691 = max(_S690.x * lod_mul_0, min((float4((&f_46->grass_0)->lod_0) ).z, blades_1));
    float _S692 = _S690.y;
    float _S693 = min(pow(blades_1 / _S691, 0.80000001192092896f) * rsqrt(max(_S692, 0.20000000298023224f)), (float4((&f_46->grass_0)->lod_0) ).w);
    float keep_0 = saturate((_S692 - float((c_21->node_kind_0) >> 24U) / 255.0f) / (0.30000001192092896f * _S692 + 0.0010000000474974513f));
    float range_0 = (float4((&f_46->grass_0)->lod_0) ).y;
    float fade_0 = 1.0f - smoothstep(0.60000002384185791f * range_0, range_0, d_44);
    uint _S694 = c_21->seed_0;
    uint s0_0 = grass_hash_0((c_21->seed_0) ^ (k_10 * 2654435769U + 2135587861U));
    uint s1_1 = grass_hash_0(s0_0);
    uint s2_0 = grass_hash_0(s1_1);
    uint s3_0 = grass_hash_0(s2_0);
    uint s4_0 = grass_hash_0(s3_0);
    uint s5_0 = grass_hash_0(s4_0);
    float ang_0 = grass_rand_0(s0_0) * 6.2831854820251465f;
    float rad_2 = sqrt(grass_rand_0(s1_1));
    float4 _S695 = float4(ty_0->b_11) ;
    float3 off_3 = (fa_0 * float3(cos(ang_0))  + rt_3 * float3((sin(ang_0) * (float4(ty_0->root_0) ).w)) ) * float3((rad_2 * _S695.x)) ;
    thread GrassBlade_0 o_2;
    (&o_2)->up_7 = up_8;
    uint _S696 = (c_21->hw_0) & 65535U;
    float hc_0 = mix(_S689.x, _S689.y, grass_rand_0(grass_hash_0(c_21->seed_0 + 17U))) * (float(_S696) / 65535.0f * 4.0f);
    (&o_2)->p0_1 = base_0 + off_3 - up_8 * float3(min(0.029999999329447746f, 0.10000000149011612f * hc_0)) ;
    float h_35 = hc_0 * (1.0f - 0.34999999403953552f * rad_2) * mix(0.72000002861022949f, 1.1200000047683716f, grass_rand_0(s2_0));
    float grow_0 = saturate(_S691 - float(k_10)) * keep_0 * fade_0;
    bool _S697;
    if(k_10 < uint(blades_1))
    {
        _S697 = _S696 != 0U;
    }
    else
    {
        _S697 = false;
    }
    float _S698;
    if(_S697)
    {
        _S698 = h_35 * grow_0;
    }
    else
    {
        _S698 = 0.0f;
    }
    (&o_2)->h_33 = _S698;
    float ca_0 = grass_rand_0(grass_hash_0(_S694 + 29U)) * 6.2831854820251465f;
    float3 cdir_0 = fa_0 * float3(cos(ca_0))  + rt_3 * float3(sin(ca_0)) ;
    float3 radial_0;
    if(rad_2 > 0.0010000000474974513f)
    {
        radial_0 = normalize(off_3);
    }
    else
    {
        radial_0 = fa_0;
    }
    float ba_0 = grass_rand_0(s3_0) * 6.2831854820251465f;
    float3 ld_0 = cdir_0 * float3(0.89999997615814209f)  + radial_0 * float3(0.80000001192092896f)  + (fa_0 * float3(cos(ba_0))  + rt_3 * float3(sin(ba_0)) ) * float3(0.60000002384185791f) ;
    float3 ld_1;
    if((dot(ld_0, ld_0)) > 9.99999997475242708e-07f)
    {
        ld_1 = normalize(ld_0);
    }
    else
    {
        ld_1 = fa_0;
    }
    float lean_0 = _S695.y * mix(0.30000001192092896f, 1.0f, grass_rand_0(s4_0));
    float3 tipv_0 = up_8 * float3(cos(lean_0))  + ld_1 * float3(sin(lean_0)) ;
    WindData_natural_0 wn_0 = f_46->wind_1;
    if((((uint4((&f_46->grass_0)->bufs_2) ).w) & 4U) != 0U)
    {
        _S697 = ((float4(wn_0.cfg_2) ).x) > 0.5f;
    }
    else
    {
        _S697 = false;
    }
    float3 tipv_1;
    if(_S697)
    {
        float4 _S699 = float4(wn_0.wind_0) ;
        float2 _S700 = _S699.xz;
        float speed_0 = length(_S700);
        float2 wd_0;
        if(speed_0 > 0.0010000000474974513f)
        {
            wd_0 = _S700 / float2(speed_0) ;
        }
        else
        {
            wd_0 = float2(0.0f, 0.0f);
        }
        float4 _S701 = float4(wn_0.gust_0) ;
        float2 _S702 = base_0.xz;
        float _S703 = dot(_S702, wd_0);
        float g_11 = 0.5f + 0.5f * sin((_S701.x + _S703 / max(_S701.y, 1.0f)) * 6.2831854820251465f);
        float tw_1 = _S699.w;
        float _S704 = wd_0.y;
        float _S705 = wd_0.x;
        float _S706 = grass_rand_0(s5_0);
        float _S707 = 1.0f - _S695.z;
        float3 wv_0 = float3(_S705, 0.0f, _S704);
        tipv_1 = normalize(tipv_0 + (wv_0 - up_8 * float3(dot(wv_0, up_8)) ) * float3(clamp(speed_0 * 0.059999998658895493f * _S707 * (0.34999999403953552f + 0.89999997615814209f * (g_11 * g_11 * g_11 * _S701.z) + 0.69999998807907104f * (0.5f + 0.5f * sin(_S703 * 0.89999997615814209f - tw_1 * (1.2000000476837158f + 0.25f * speed_0) + 0.60000002384185791f * sin(dot(_S702, float2(- _S704, _S705)) * 0.34999999403953552f)))) + 0.039999999105930328f * _S707 * sin(tw_1 * (5.0f + 3.0f * _S706) + _S706 * 40.0f) * min(speed_0 * 0.20000000298023224f, 1.0f), -0.30000001192092896f, 1.3999999761581421f)) );
    }
    else
    {
        tipv_1 = tipv_0;
    }
    float3 _S708 = (&o_2)->p0_1 + tipv_1 * float3(((&o_2)->h_33 * (1.0f - 0.18000000715255737f * saturate(1.0f - dot(tipv_1, up_8))))) ;
    (&o_2)->p2_0 = _S708;
    float3 across_0 = _S708 - (&o_2)->p0_1 - up_8 * float3(dot(_S708 - (&o_2)->p0_1, up_8)) ;
    float curve_0 = _S695.w;
    (&o_2)->p1_0 = (&o_2)->p0_1 + up_8 * float3((max(dot(_S708 - (&o_2)->p0_1, up_8), 0.25f * (&o_2)->h_33) * mix(0.60000002384185791f, 1.0499999523162842f, curve_0)))  + across_0 * float3(mix(0.5f, 0.05000000074505806f, curve_0)) ;
    float3 lh_0;
    if((dot(across_0, across_0)) > 9.99999993922529029e-09f)
    {
        lh_0 = normalize(across_0);
    }
    else
    {
        lh_0 = ld_1;
    }
    float tw2_0 = (grass_rand_0(s5_0) - 0.5f) * 1.1000000238418579f;
    (&o_2)->side_1 = normalize(cross(up_8, lh_0) * float3(cos(tw2_0))  + lh_0 * float3(sin(tw2_0)) );
    (&o_2)->w_15 = _S689.z * _S693 * mix(0.80000001192092896f, 1.2000000476837158f, grass_rand_0(s1_1 ^ 85U));
    (&o_2)->head_0 = (float4(ty_0->mid_0) ).w;
    (&o_2)->head_w_0 = (float4(ty_0->tip_0) ).w;
    (&o_2)->shade_0 = mix(0.81999999284744263f, 1.1499999761581421f, grass_rand_0(s4_0 ^ 51U));
    (&o_2)->t_ao_0 = saturate(hc_0 / 0.80000001192092896f);
    return o_2;
}

float3 grass_point_0(const GrassBlade_0 thread* b_12, float t_11)
{
    float s_18 = 1.0f - t_11;
    return b_12->p0_1 * float3((s_18 * s_18))  + b_12->p1_0 * float3((2.0f * s_18 * t_11))  + b_12->p2_0 * float3((t_11 * t_11)) ;
}

float3 grass_tangent_0(const GrassBlade_0 thread* b_13, float t_12)
{
    return (b_13->p1_0 - b_13->p0_1) * float3((2.0f * (1.0f - t_12)))  + (b_13->p2_0 - b_13->p1_0) * float3((2.0f * t_12)) ;
}

float3 grass_view_side_0(const GrassBlade_0 thread* b_14, float t_13, float3 p_21)
{
    float3 _S709 = grass_tangent_0(b_14, t_13);
    float3 vs_0 = cross(normalize(_S709 + b_14->up_7 * float3(9.9999997473787516e-05f) ), normalize(p_21));
    if((dot(vs_0, vs_0)) < 9.99999993922529029e-09f)
    {
        return b_14->side_1;
    }
    float3 vs_1 = normalize(vs_0);
    float3 _S710 = b_14->side_1;
    float3 vs_2;
    if((dot(vs_1, b_14->side_1)) < 0.0f)
    {
        vs_2 = - vs_1;
    }
    else
    {
        vs_2 = vs_1;
    }
    return normalize(mix(_S710, vs_2, float3(0.55000001192092896f) ));
}

float3 shade_foliage_0(FrameData_natural_0 device* f_47, float3 sun_5, float3 albedo_7, float rough_5, float ao_8, float sv_5, float sun_vis_2, float3 n_14, float3 ng_6, float3 v_8, bool leaf_1, KernelContext_0 thread* kernelContext_59)
{
    float3 _S711 = shade_pbr_0(f_47, sun_5, albedo_7, 0.0f, rough_5, ao_8, sv_5, sun_vis_2, n_14, v_8, kernelContext_59);
    float3 col_3;
    if(leaf_1)
    {
        float3 l_8 = (float4(f_47->sun_dir_0) ).xyz;
        float3 trans_0 = albedo_7 * float3(1.1499999761581421f, 1.25f, 0.55000001192092896f);
        float3 col_4 = _S711 + sun_5 * float3(sun_vis_2)  * trans_0 * float3((saturate(- dot(n_14, l_8)) * (0.44999998807907104f + 1.2000000476837158f * pow(saturate(dot(- v_8, l_8)), 4.0f))))  * float3(0.31830987334251404f) ;
        float3 _S712 = hemi_ambient_0(f_47, - n_14);
        col_3 = col_4 + trans_0 * _S712 * float3(mix((float4((&f_47->light_0)->skyvis_0) ).y, 1.0f, sv_5))  * float3(ao_8)  * float3(0.30000001192092896f) ;
    }
    else
    {
        col_3 = _S711;
    }
    return col_3;
}

float4 grass_pixel_0(FrameData_natural_0 device* f_48, const FsOut_0 thread* i_27, uint2 vis_3, float3 dir_12, KernelContext_0 thread* kernelContext_60)
{
    GrassClump_0 device* _S713 = ed_buf_5((uint4((&f_48->grass_0)->bufs_2) ).x, kernelContext_60);
    GrassClump_0 c_22 = *(_S713 + ((vis_3.x) & 536870911U));
    GrassType_natural_0 device* _S714 = ed_buf_6((uint4((&f_48->grass_0)->bufs_2) ).y, kernelContext_60);
    GrassType_natural_0 device* _S715 = _S714 + (((c_22.node_kind_0) >> 12U) & 255U);
    GrassType_natural_0 ty_1 = *_S715;
    thread GrassType_natural_0 _S716 = *_S715;
    packed_float4 device* _S717 = ed_buf_4(kernelContext_60->ed_c_0->nodes_0, kernelContext_60);
    float3 node_1 = (float4(*(_S717 + ((c_22.node_kind_0) & 4095U))) ).xyz;
    uint _S718 = vis_3.y;
    uint k_11 = _S718 & 127U;
    float t_14 = float((_S718 >> 7U) & 4095U) / 4095.0f;
    float u_1 = float((_S718 >> 19U) & 255U) / 255.0f;
    thread GrassClump_0 _S719 = c_22;
    _S716 = ty_1;
    GrassBlade_0 _S720 = grass_blade_0(f_48, &_S719, &_S716, node_1, k_11, 1.0f);
    thread GrassBlade_0 _S721 = _S720;
    float3 _S722 = grass_point_0(&_S721, t_14);
    thread GrassBlade_0 _S723 = _S720;
    float3 _S724 = grass_view_side_0(&_S723, t_14, _S722);
    thread GrassBlade_0 _S725 = _S720;
    float3 _S726 = grass_tangent_0(&_S725, t_14);
    float3 v_9 = - dir_12;
    float3 n0_0 = normalize(cross(_S724, normalize(_S726 + _S720.up_7 * float3(9.9999997473787516e-05f) )));
    float3 n0_1;
    if((dot(n0_0, v_9)) < 0.0f)
    {
        n0_1 = - n0_0;
    }
    else
    {
        n0_1 = n0_0;
    }
    float dn_0 = dot(dir_12, n0_1);
    float lp_0 = length(_S722);
    float hit_0;
    if((abs(dn_0)) > 0.0010000000474974513f)
    {
        hit_0 = dot(_S722, n0_1) / dn_0;
    }
    else
    {
        hit_0 = lp_0;
    }
    float3 p_22 = dir_12 * float3(clamp(hit_0, lp_0 - 0.05000000074505806f, lp_0 + 0.05000000074505806f)) ;
    float far_1 = smoothstep((float4((&f_48->grass_0)->lod_0) ).x * 0.5f, (float4((&f_48->grass_0)->lod_0) ).y, length(p_22));
    float3 n_15 = normalize(mix(normalize(n0_1 + _S724 * float3(((u_1 - 0.5f) * 1.1000000238418579f)) ), _S720.up_7, float3(mix((float4((&f_48->grass_0)->look_0) ).x, 0.80000001192092896f, far_1)) ));
    float3 alb_1;
    if(t_14 < 0.5f)
    {
        alb_1 = mix((float4((&_S716)->root_0) ).xyz, (float4((&_S716)->mid_0) ).xyz, float3((t_14 * 2.0f)) );
    }
    else
    {
        alb_1 = mix((float4((&_S716)->mid_0) ).xyz, (float4((&_S716)->tip_0) ).xyz, float3(((t_14 - 0.5f) * 2.0f)) );
    }
    if((_S720.head_0) > 0.0f)
    {
        float _S727 = 1.0f - _S720.head_0;
        alb_1 = mix(alb_1, (float4((&_S716)->tip_0) ).xyz * float3(1.1000000238418579f) , float3(smoothstep(_S727 - 0.039999999105930328f, _S727, t_14)) );
    }
    float hue_0 = grass_rand_0(grass_hash_0(c_22.seed_0 + 53U)) - 0.5f;
    float3 alb_2 = alb_1 * (float3((mix(0.86000001430511475f, 1.1200000047683716f, grass_rand_0(grass_hash_0(c_22.seed_0 + 41U))) * _S720.shade_0))  * float3(1.0f + 0.10000000149011612f * hue_0, 1.0f, 1.0f - 0.11999999731779099f * hue_0));
    float dry_1 = float(((c_22.hw_0) >> 16U) & 255U) / 255.0f;
    float bd_0 = grass_rand_0(grass_hash_0((c_22.seed_0) ^ (k_11 * 2246822507U + 19088743U)));
    if(bd_0 < dry_1)
    {
        alb_1 = mix(alb_2, (float4((&_S716)->dry_0) ).xyz * float3(mix(0.85000002384185791f, 1.1000000238418579f, bd_0 / max(dry_1, 0.0010000000474974513f))) , float3(mix(0.55000001192092896f, 1.0f, t_14)) );
    }
    else
    {
        alb_1 = alb_2;
    }
    float3 _S728 = min(alb_1, float3(0.89999997615814209f, 0.89999997615814209f, 0.89999997615814209f));
    float ao_r_0 = mix(0.55000001192092896f, 0.2800000011920929f, _S720.t_ao_0);
    float ao_b_0 = mix(mix(ao_r_0, 1.0f, smoothstep(0.0f, 0.75f, t_14)), mix(ao_r_0, 1.0f, 0.69999998807907104f), far_1);
    float zlin_3 = - (((float4(p_22, 1.0f)) * (matrix<float,int(4),int(4)> (f_48->view_0.data_0[int(0)][int(0)], f_48->view_0.data_0[int(1)][int(0)], f_48->view_0.data_0[int(2)][int(0)], f_48->view_0.data_0[int(3)][int(0)], f_48->view_0.data_0[int(0)][int(1)], f_48->view_0.data_0[int(1)][int(1)], f_48->view_0.data_0[int(2)][int(1)], f_48->view_0.data_0[int(3)][int(1)], f_48->view_0.data_0[int(0)][int(2)], f_48->view_0.data_0[int(1)][int(2)], f_48->view_0.data_0[int(2)][int(2)], f_48->view_0.data_0[int(3)][int(2)], f_48->view_0.data_0[int(0)][int(3)], f_48->view_0.data_0[int(1)][int(3)], f_48->view_0.data_0[int(2)][int(3)], f_48->view_0.data_0[int(3)][int(3)])))).z;
    float2 _S729 = i_27->pos_0.xy;
    float _S730 = sun_shadow_0(f_48, kernelContext_60->ed_c_0->shadow_0, p_22, _S720.up_7, zlin_3, _S729, kernelContext_60);
    float2 _S731 = ao_upsample_0(f_48, kernelContext_60->ed_c_0->ao_0, _S729, zlin_3, kernelContext_60);
    float3 _S732 = sun_light_at_0(f_48, p_22, kernelContext_60);
    float _S733 = _S731.x;
    float _S734 = _S731.y;
    float3 _S735 = shade_foliage_0(f_48, _S732, _S728, 0.5f, _S733 * ao_b_0, _S734, _S730, n_15, n_15, v_9, true, kernelContext_60);
    float3 _S736 = apply_aerial_0(f_48, _S735, p_22, _S729, kernelContext_60);
    float3 col_5;
    if(((uint4((&f_48->light_0)->flags_0) ).w) == 2U)
    {
        col_5 = float3((0.5f * _S733 * ao_b_0)) ;
    }
    else
    {
        if(((uint4((&f_48->light_0)->flags_0) ).w) == 4U)
        {
            col_5 = float3((0.5f * _S734)) ;
        }
        else
        {
            if(((uint4((&f_48->light_0)->flags_0) ).w) == 1U)
            {
                col_5 = float3((0.039999999105930328f + 0.5f * _S730)) ;
            }
            else
            {
                col_5 = _S736;
            }
        }
    }
    return float4(col_5, 1.0f);
}

struct GpuTree_0
{
    float px_6;
    float py_1;
    float pz_1;
    uint node_kind_1;
    float scale_1;
    uint up_9;
    uint yaw_widen_0;
    uint user_0;
    uint season_1;
};

GpuTree_0 device* ed_buf_7(uint i_28, KernelContext_0 thread* kernelContext_61)
{
    return (GpuTree_0 device*)kernelContext_61->ed_buffers_0[i_28];
}

struct GpuImpKind_natural_0
{
    packed_uint4 tex_1;
    packed_float4 geo_0;
    packed_float4 leaf_2;
    packed_float4 bark_0;
};

GpuImpKind_natural_0 device* ed_buf_8(uint i_29, KernelContext_0 thread* kernelContext_62)
{
    return (GpuImpKind_natural_0 device*)kernelContext_62->ed_buffers_0[i_29];
}

struct ImpTree_0
{
    float3 c_23;
    float3 ex_0;
    float3 up_10;
    float3 ez_0;
    float s_19;
    float wd_1;
    float r_11;
    float3 cam_0;
    uint3 fi_0;
    float3 w_17;
    float3 d0_0;
    float3 d1_0;
    float3 d2_0;
};

float3 arc_from_y_0(float3 up_11, float3 v_10)
{
    float3 a_20 = float3(up_11.z, 0.0f, - up_11.x);
    float w_18 = 1.0f + up_11.y;
    float l_9 = sqrt(dot(a_20, a_20) + w_18 * w_18);
    if(l_9 < 9.99999997475242708e-07f)
    {
        return float3(v_10.x, - v_10.y, - v_10.z);
    }
    float3 a_21 = a_20 / float3(l_9) ;
    float3 t_15 = float3(2.0f)  * cross(a_21, v_10);
    return v_10 + float3((w_18 / l_9))  * t_15 + cross(a_21, t_15);
}

float3 imp_to_tree_0(const ImpTree_0 thread* t_16, float3 v_11)
{
    float _S737 = t_16->s_19 * t_16->wd_1;
    return float3(dot(v_11, t_16->ex_0) / _S737, dot(v_11, t_16->up_10) / t_16->s_19, dot(v_11, t_16->ez_0) / _S737);
}

float2 imp_oct_encode_0(float3 d_45)
{
    thread float3 _S738 = d_45;
    _S738.y = max(d_45.y, 0.0f);
    float3 _S739 = _S738 / float3(max(abs(_S738.x) + abs(_S738.y) + abs(_S738.z), 9.99999971718068537e-10f)) ;
    _S738 = _S739;
    float _S740 = _S739.x;
    float _S741 = _S739.z;
    float2 _S742 = float2(0.5f) ;
    return float2(_S740 + _S741, _S740 - _S741) * _S742 + _S742;
}

float3 imp_oct_decode_0(float2 uv_4)
{
    float2 g_12 = uv_4 * float2(2.0f)  - float2(1.0f) ;
    float _S743 = g_12.x;
    float _S744 = g_12.y;
    float x_5 = (_S743 + _S744) * 0.5f;
    float z_0 = (_S743 - _S744) * 0.5f;
    return normalize(float3(x_5, 1.0f - abs(x_5) - abs(z_0), z_0));
}

ImpTree_0 imp_tree_0(const GpuTree_0 thread* g_13, const GpuImpKind_natural_0 thread* k_12, float3 node_2, float3 sun_dir_1, bool sun_6)
{
    thread ImpTree_0 t_17;
    float3 _S745 = oct_decode_0(g_13->up_9);
    (&t_17)->up_10 = _S745;
    float yaw_0 = float((g_13->yaw_widen_0) & 65535U) / 65535.0f * 6.2831854820251465f;
    (&t_17)->wd_1 = 0.5f + float((g_13->yaw_widen_0) >> 16U) / 65535.0f;
    (&t_17)->s_19 = g_13->scale_1;
    float _S746 = cos(yaw_0);
    float _S747 = sin(yaw_0);
    (&t_17)->ex_0 = arc_from_y_0(_S745, float3(_S746, 0.0f, - _S747));
    (&t_17)->ez_0 = arc_from_y_0(_S745, float3(_S747, 0.0f, _S746));
    float4 _S748 = float4(k_12->geo_0) ;
    (&t_17)->r_11 = _S748.y;
    (&t_17)->c_23 = node_2 + float3(g_13->px_6, g_13->py_1, g_13->pz_1) + _S745 * float3((_S748.x * g_13->scale_1)) ;
    ImpTree_0 _S749 = t_17;
    float3 _S750;
    if(sun_6)
    {
        _S750 = sun_dir_1 * float3(1.0e+05f) ;
    }
    else
    {
        _S750 = - (&t_17)->c_23;
    }
    thread ImpTree_0 _S751 = _S749;
    float3 _S752 = imp_to_tree_0(&_S751, _S750);
    (&t_17)->cam_0 = _S752;
    uint _S753 = (uint4(k_12->tex_1) ).w;
    float n_16 = float(_S753);
    float2 _S754 = float2(n_16) ;
    float2 _S755 = float2(0.5f) ;
    float _S756 = n_16 - 1.0f;
    float2 gpos_0 = clamp(imp_oct_encode_0(normalize(_S752)) * _S754 - _S755, float2(0.0f, 0.0f), float2(_S756, _S756));
    float _S757 = n_16 - 2.0f;
    float2 _S758 = min(floor(gpos_0), float2(_S757, _S757));
    float2 fr_0 = gpos_0 - _S758;
    uint2 _S759 = uint2(_S758);
    uint2 c1_0 = _S759 + uint2(1U, 0U);
    uint2 c2_0 = _S759 + uint2(0U, 1U);
    float _S760 = fr_0.x;
    float _S761 = fr_0.y;
    float _S762 = _S760 + _S761;
    uint2 c0_1;
    if(_S762 < 1.0f)
    {
        (&t_17)->w_17 = float3(1.0f - _S760 - _S761, _S760, _S761);
        c0_1 = _S759;
    }
    else
    {
        uint2 _S763 = _S759 + uint2(1U, 1U);
        (&t_17)->w_17 = float3(_S762 - 1.0f, 1.0f - _S761, 1.0f - _S760);
        c0_1 = _S763;
    }
    (&t_17)->fi_0 = uint3(c0_1.y * _S753 + c0_1.x, c1_0.y * _S753 + c1_0.x, c2_0.y * _S753 + c2_0.x);
    (&t_17)->d0_0 = imp_oct_decode_0((float2(c0_1) + _S755) / _S754);
    (&t_17)->d1_0 = imp_oct_decode_0((float2(c1_0) + _S755) / _S754);
    (&t_17)->d2_0 = imp_oct_decode_0((float2(c2_0) + _S755) / _S754);
    return t_17;
}

void imp_view_basis_0(float3 v_12, float3 thread* right_0, float3 thread* up_12)
{
    float3 u0_0 = float3(0.0f, 1.0f, 0.0f) - v_12 * float3(v_12.y) ;
    float3 _S764;
    if((dot(u0_0, u0_0)) > 9.99999997475242708e-07f)
    {
        _S764 = normalize(u0_0);
    }
    else
    {
        _S764 = float3(0.0f, 0.0f, 1.0f);
    }
    *up_12 = _S764;
    *right_0 = normalize(cross(_S764, v_12));
    return;
}

float2 imp_frame_uv_0(const ImpTree_0 thread* t_18, float3 d_46, float3 q_17, float half_size_0)
{
    float3 _S765 = t_18->cam_0;
    float3 dir_13 = q_17 - t_18->cam_0;
    float den_0 = dot(dir_13, d_46);
    float k_13;
    if((abs(den_0)) > 9.99999997475242708e-07f)
    {
        k_13 = - dot(_S765, d_46) / den_0;
    }
    else
    {
        k_13 = 1.0f;
    }
    float3 h_36 = _S765 + dir_13 * float3(k_13) ;
    thread float3 r_12;
    thread float3 u_2;
    imp_view_basis_0(d_46, &r_12, &u_2);
    float _S766 = 2.0f * half_size_0;
    return float2(0.5f + dot(h_36, r_12) / _S766, 0.5f - dot(h_36, u_2) / _S766);
}

float2 imp_atlas_uv_0(uint fi_1, uint n_17, float2 uv_5, float frame_px_0)
{
    float m_7 = 0.5f / frame_px_0;
    float _S767 = 1.0f - m_7;
    float2 l_10 = clamp(uv_5, float2(m_7, m_7), float2(_S767, _S767));
    uint _S768 = fi_1 % n_17;
    float _S769 = float(_S768);
    uint _S770 = fi_1 / n_17;
    return (float2(_S769, float(_S770)) + l_10) / float2(float(n_17)) ;
}

bool imp_inside_0(float2 uv_6)
{
    bool _S771;
    if(all(uv_6 >= (float2(0.0f) )))
    {
        _S771 = all(uv_6 <= (float2(1.0f) ));
    }
    else
    {
        _S771 = false;
    }
    return _S771;
}

float3 imp_oct_sphere_0(float2 e_13)
{
    float2 f_49 = e_13 * float2(2.0f)  - float2(1.0f) ;
    float _S772 = f_49.x;
    float _S773 = f_49.y;
    float _S774 = 1.0f - abs(_S772) - abs(_S773);
    thread float3 n_18 = float3(_S772, _S774, _S773);
    float t_19 = saturate(- _S774);
    float _S775;
    if(_S772 >= 0.0f)
    {
        _S775 = - t_19;
    }
    else
    {
        _S775 = t_19;
    }
    n_18.x = n_18.x + _S775;
    if((n_18.z) >= 0.0f)
    {
        _S775 = - t_19;
    }
    else
    {
        _S775 = t_19;
    }
    n_18.z = n_18.z + _S775;
    return normalize(n_18);
}

float3 tree_hue_0(float h_37)
{
    float3 _S776;
    if(h_37 >= 0.0f)
    {
        _S776 = mix(float3(1.0f, 1.0f, 1.0f), float3(0.85000002384185791f, 0.97000002861022949f, 1.2000000476837158f), float3(h_37) );
    }
    else
    {
        _S776 = mix(float3(1.0f, 1.0f, 1.0f), float3(1.1200000047683716f, 1.0f, 0.75f), float3(- h_37) );
    }
    return _S776;
}

LeafSeason_0 season_state_1(FrameData_natural_0 device* f_50, uint row_1, float shift_1, bool south_1)
{
    if(((float4((&f_50->season_0)->cfg_4) ).x) <= 0.0f)
    {
        return season_off_0();
    }
    float d_47 = (float4((&f_50->season_0)->cfg_4) ).x;
    float d_48;
    if(south_1)
    {
        d_48 = fmod(d_47 + 182.625f, 365.25f);
    }
    else
    {
        d_48 = d_47;
    }
    float ds_1 = d_48 - shift_1;
    float da_1 = d_48 + (float4((&f_50->season_0)->cfg_4) ).y * shift_1;
    uint _S777 = (row_1 & 31U) * 2U;
    float4 _S778 = float4((&(&f_50->season_0)->rows_0)->data_5[_S777]) ;
    float4 _S779 = float4((&(&f_50->season_0)->rows_0)->data_5[_S777 + 1U]) ;
    thread LeafSeason_0 s_20;
    (&s_20)->target_0 = _S779.xyz;
    float _S780 = _S778.w;
    (&s_20)->evergreen_0 = _S780;
    if(_S780 > 0.5f)
    {
        float _S781 = _S778.y;
        float a_22 = season_ramp_0(_S781 - 10.0f, _S781 + 30.0f, da_1);
        float _S782 = _S778.x;
        float rel_2 = 1.0f - season_ramp_0(_S782 - 10.0f, _S782 + 20.0f, ds_1);
        (&s_20)->green_0 = 1.0f;
        (&s_20)->leaves_0 = 1.0f;
        (&s_20)->colour_0 = _S779.w * max(a_22, rel_2);
    }
    else
    {
        float _S783 = _S778.x;
        float _S784 = season_ramp_0(_S783 - 6.0f, _S783 + 8.0f, ds_1);
        (&s_20)->green_0 = _S784;
        float _S785 = _S778.y;
        (&s_20)->colour_0 = season_ramp_0(_S785 - 8.0f, _S785 + 14.0f, da_1);
        float _S786 = _S778.z;
        (&s_20)->leaves_0 = _S784 * (1.0f - season_ramp_0(_S786 - 10.0f, _S786 + 10.0f, da_1));
    }
    return s_20;
}

LeafSeason_0 season_of_word_0(FrameData_natural_0 device* f_51, uint word_0)
{
    LeafSeason_0 _S787 = season_state_1(f_51, word_0 & 255U, float(int(word_0) >> 16U) * 0.0625f, ((word_0 >> 8U) & 1U) != 0U);
    return _S787;
}

float4 impostor_pixel_0(FrameData_natural_0 device* f_52, const FsOut_0 thread* i_30, uint idx_0, float3 dir_14, KernelContext_0 thread* kernelContext_63)
{
    GpuTree_0 device* _S788 = ed_buf_7((uint4((&f_52->imp_0)->bufs_1) ).x, kernelContext_63);
    GpuTree_0 g_14 = *(_S788 + idx_0);
    GpuImpKind_natural_0 device* _S789 = ed_buf_8((uint4((&f_52->imp_0)->bufs_1) ).y, kernelContext_63);
    GpuImpKind_natural_0 device* _S790 = _S789 + (((g_14.node_kind_1) >> 12U) & 4095U);
    GpuImpKind_natural_0 k_14 = *_S790;
    thread GpuImpKind_natural_0 _S791 = *_S790;
    packed_float4 device* _S792 = ed_buf_4(kernelContext_63->ed_c_0->nodes_0, kernelContext_63);
    float3 node_3 = (float4(*(_S792 + ((g_14.node_kind_1) & 4095U))) ).xyz;
    float3 _S793 = (float4(f_52->sun_dir_0) ).xyz;
    thread GpuTree_0 _S794 = g_14;
    _S791 = k_14;
    ImpTree_0 _S795 = imp_tree_0(&_S794, &_S791, node_3, _S793, false);
    float3 nrm_0 = normalize(_S795.c_23);
    float hit_1 = dot(_S795.c_23, nrm_0) / max(dot(dir_14, nrm_0), 9.9999997473787516e-05f);
    float3 pq_0 = dir_14 * float3(hit_1) ;
    float3 _S796 = pq_0 - _S795.c_23;
    thread ImpTree_0 _S797 = _S795;
    float3 _S798 = imp_to_tree_0(&_S797, _S796);
    float half_t_0 = _S795.r_11 * (float4((&f_52->imp_0)->cfg_3) ).y;
    thread ImpTree_0 _S799 = _S795;
    float2 _S800 = imp_frame_uv_0(&_S799, _S795.d0_0, _S798, half_t_0);
    thread ImpTree_0 _S801 = _S795;
    float2 _S802 = imp_frame_uv_0(&_S801, _S795.d1_0, _S798, half_t_0);
    thread ImpTree_0 _S803 = _S795;
    float2 _S804 = imp_frame_uv_0(&_S803, _S795.d2_0, _S798, half_t_0);
    uint4 _S805 = uint4((&_S791)->tex_1) ;
    uint n_19 = _S805.w;
    float fpx_0 = (float4((&_S791)->geo_0) ).w;
    float4 _S806 = float4((&_S791)->leaf_2) ;
    float lod_2 = clamp(log2(max(hit_1 / ((float4(f_52->proj_0) ).w * 0.5f * (float4(f_52->proj_0) ).y), 9.99999997475242708e-07f) / (2.0f * half_t_0 * _S795.s_19 * max(_S795.wd_1, 1.0f) / fpx_0)), 0.0f, _S806.w - 1.0f);
    sampler _S807 = ((reinterpret_cast<device sampler*>((kernelContext_63->ed_samplers_0))[(kernelContext_63->ed_c_0->smp_0)]));
    texture2d<float, access::sample> _S808 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_63->ed_textures_0))[(_S805.x)]));
    texture2d<float, access::sample> _S809 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_63->ed_textures_0))[(_S805.y)]));
    texture2d<float, access::sample> _S810 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_63->ed_textures_0))[(_S805.z)]));
    float2 a0_0 = imp_atlas_uv_0(_S795.fi_0.x, n_19, _S800, fpx_0);
    float2 a1_0 = imp_atlas_uv_0(_S795.fi_0.y, n_19, _S802, fpx_0);
    float2 a2_2 = imp_atlas_uv_0(_S795.fi_0.z, n_19, _S804, fpx_0);
    float2 s0_1 = ((_S808).sample((_S807), (a0_0), level((lod_2)))).xy;
    float2 s1_2 = ((_S808).sample((_S807), (a1_0), level((lod_2)))).xy;
    float2 s2_1 = ((_S808).sample((_S807), (a2_2), level((lod_2)))).xy;
    float _S811 = s0_1.x;
    float _S812;
    if(imp_inside_0(_S800))
    {
        _S812 = 1.0f;
    }
    else
    {
        _S812 = 0.0f;
    }
    float _S813 = _S811 * _S812;
    float _S814 = s1_2.x;
    if(imp_inside_0(_S802))
    {
        _S812 = 1.0f;
    }
    else
    {
        _S812 = 0.0f;
    }
    float _S815 = _S814 * _S812;
    float _S816 = s2_1.x;
    if(imp_inside_0(_S804))
    {
        _S812 = 1.0f;
    }
    else
    {
        _S812 = 0.0f;
    }
    float3 wc_0 = _S795.w_17 * float3(_S813, _S815, _S816 * _S812);
    float ws_0 = wc_0.x + wc_0.y + wc_0.z;
    float3 wc_1;
    if(ws_0 > 9.9999997473787516e-05f)
    {
        wc_1 = wc_0 / float3(ws_0) ;
    }
    else
    {
        wc_1 = _S795.w_17;
    }
    float _S817 = wc_1.x;
    float _S818 = wc_1.y;
    float _S819 = wc_1.z;
    float3 nt_0 = float3(_S817)  * imp_oct_sphere_0(((_S809).sample((_S807), (a0_0), level((lod_2)))).xy) + float3(_S818)  * imp_oct_sphere_0(((_S809).sample((_S807), (a1_0), level((lod_2)))).xy) + float3(_S819)  * imp_oct_sphere_0(((_S809).sample((_S807), (a2_2), level((lod_2)))).xy);
    float2 vb_0 = float2(_S817)  * ((_S810).sample((_S807), (a0_0), level((lod_2)))).xy + float2(_S818)  * ((_S810).sample((_S807), (a1_0), level((lod_2)))).xy + float2(_S819)  * ((_S810).sample((_S807), (a2_2), level((lod_2)))).xy;
    float3 p_23 = pq_0 - dir_14 * float3(((_S817 * s0_1.y + _S818 * s1_2.y + _S819 * s2_1.y - 0.5f) * 2.0f * half_t_0 * _S795.s_19)) ;
    float3 nw_1 = _S795.ex_0 * float3((nt_0.x / _S795.wd_1))  + _S795.up_10 * float3(nt_0.y)  + _S795.ez_0 * float3((nt_0.z / _S795.wd_1)) ;
    float3 nw_2;
    if((dot(nw_1, nw_1)) > 9.99999993922529029e-09f)
    {
        nw_2 = normalize(nw_1);
    }
    else
    {
        nw_2 = - dir_14;
    }
    float3 v_13 = - dir_14;
    float _S820 = dot(nw_2, v_13);
    if(_S820 < 0.0f)
    {
        nw_2 = normalize(nw_2 - float3((1.0099999904632568f * _S820))  * v_13);
    }
    float bark_1 = saturate(vb_0.y);
    float iv_0 = float((g_14.user_0) & 255U) / 128.0f;
    float3 _S821 = float3((vb_0.x * (float4((&f_52->imp_0)->cfg_3) ).z)) ;
    float3 leaf_alb_0 = _S806.xyz * _S821 * float3(iv_0)  * tree_hue_0(clamp(float(((g_14.user_0) >> 8U) & 255U) / 127.5f - 1.0f, -1.0f, 1.0f));
    float3 leaf_alb_1;
    if(((float4((&f_52->season_0)->cfg_4) ).x) > 0.0f)
    {
        LeafSeason_0 _S822 = season_of_word_0(f_52, g_14.season_1);
        thread LeafSeason_0 _S823 = _S822;
        float3 _S824 = season_tint_0(&_S823, leaf_alb_0);
        leaf_alb_1 = _S824;
    }
    else
    {
        leaf_alb_1 = leaf_alb_0;
    }
    float3 _S825 = min(mix(leaf_alb_1, (float4((&_S791)->bark_0) ).xyz * _S821 * float3(mix(1.0f, iv_0, 0.5f)) , float3(bark_1) ), float3(0.89999997615814209f, 0.89999997615814209f, 0.89999997615814209f));
    float zlin_4 = - (((float4(p_23, 1.0f)) * (matrix<float,int(4),int(4)> (f_52->view_0.data_0[int(0)][int(0)], f_52->view_0.data_0[int(1)][int(0)], f_52->view_0.data_0[int(2)][int(0)], f_52->view_0.data_0[int(3)][int(0)], f_52->view_0.data_0[int(0)][int(1)], f_52->view_0.data_0[int(1)][int(1)], f_52->view_0.data_0[int(2)][int(1)], f_52->view_0.data_0[int(3)][int(1)], f_52->view_0.data_0[int(0)][int(2)], f_52->view_0.data_0[int(1)][int(2)], f_52->view_0.data_0[int(2)][int(2)], f_52->view_0.data_0[int(3)][int(2)], f_52->view_0.data_0[int(0)][int(3)], f_52->view_0.data_0[int(1)][int(3)], f_52->view_0.data_0[int(2)][int(3)], f_52->view_0.data_0[int(3)][int(3)])))).z;
    float2 _S826 = i_30->pos_0.xy;
    float _S827 = sun_shadow_0(f_52, kernelContext_63->ed_c_0->shadow_0, p_23, nw_2, zlin_4, _S826, kernelContext_63);
    float2 _S828 = ao_upsample_0(f_52, kernelContext_63->ed_c_0->ao_0, _S826, zlin_4, kernelContext_63);
    float3 _S829 = sun_light_at_0(f_52, p_23, kernelContext_63);
    float _S830 = _S828.x;
    float _S831 = _S828.y;
    float3 _S832 = shade_foliage_0(f_52, _S829, _S825, 0.81999999284744263f, _S830, _S831, _S827, nw_2, nw_2, v_13, bark_1 < 0.5f, kernelContext_63);
    float3 _S833 = apply_aerial_0(f_52, _S832, p_23, _S826, kernelContext_63);
    float3 col_6;
    if(((uint4((&f_52->light_0)->flags_0) ).w) == 2U)
    {
        col_6 = float3((0.5f * _S830)) ;
    }
    else
    {
        if(((uint4((&f_52->light_0)->flags_0) ).w) == 4U)
        {
            col_6 = float3((0.5f * _S831)) ;
        }
        else
        {
            if(((uint4((&f_52->light_0)->flags_0) ).w) == 1U)
            {
                col_6 = float3((0.039999999105930328f + 0.5f * _S827)) ;
            }
            else
            {
                col_6 = _S833;
            }
        }
    }
    return float4(col_6, 1.0f);
}

struct _Array_natural_vectorx3Cfloatx2C4x3E3_0
{
    array<packed_float4, int(3)> data_8;
};

struct GpuInstance_natural_0
{
    _Array_natural_vectorx3Cfloatx2C4x3E3_0 m_8;
    _Array_natural_vectorx3Cfloatx2C4x3E3_0 prev_0;
    uint mesh_0;
    uint material_0;
    uint object_lo_0;
    uint node_flags_0;
    array<uint, int(4)> pad_2;
};

GpuInstance_natural_0 device* ed_buf_9(uint i_31, KernelContext_0 thread* kernelContext_64)
{
    return (GpuInstance_natural_0 device*)kernelContext_64->ed_buffers_0[i_31];
}

struct GpuMesh_natural_0
{
    packed_float4 sphere_0;
    uint lod_count_0;
    uint bin_base_0;
    uint capacity_0;
    uint flags_1;
    array<uint, int(4)> first_index_0;
    array<uint, int(4)> index_count_0;
    array<int, int(4)> base_vertex_0;
    array<float, int(4)> error_0;
};

GpuMesh_natural_0 device* ed_buf_10(uint i_32, KernelContext_0 thread* kernelContext_65)
{
    return (GpuMesh_natural_0 device*)kernelContext_65->ed_buffers_0[i_32];
}

struct Vtx_0
{
    float px_7;
    float py_2;
    float pz_2;
    uint n_20;
};

Vtx_0 device* ed_buf_11(uint i_33, KernelContext_0 thread* kernelContext_66)
{
    return (Vtx_0 device*)kernelContext_66->ed_buffers_0[i_33];
}

packed_uint2 device* ed_buf_12(uint i_34, KernelContext_0 thread* kernelContext_67)
{
    return (packed_uint2 device*)kernelContext_67->ed_buffers_0[i_34];
}

float gc_keep_0(float d_49, float reach_0)
{
    float r0_0 = 0.19499999284744263f * reach_0;
    float k_15 = r0_0 / max(d_49, r0_0);
    return k_15 * k_15 * (1.0f - smoothstep(0.69999998807907104f * reach_0, reach_0, d_49));
}

float3 gc_scale_0(const GpuInstance_natural_0 thread* inst_0, float d_50)
{
    uint _S834 = inst_0->pad_2[int(2)];
    if((((inst_0->pad_2[int(2)]) >> 20U) & 1U) != 0U)
    {
        return float3(1.0f) ;
    }
    float k_16 = gc_keep_0(d_50, float((_S834 >> 12U) & 255U));
    float s_21 = saturate((k_16 - float(_S834 & 4095U) * 0.00024420025874860585f) / (0.30000001192092896f * k_16 + 0.0010000000474974513f));
    if(s_21 <= 0.0f)
    {
        return float3(0.0f) ;
    }
    float gx_2 = clamp(sqrt(1.0f / max(k_16, 9.9999997473787516e-05f)), 1.0f, 2.0f);
    return float3(gx_2, min(gx_2, 1.25f), gx_2) * float3(s_21) ;
}

float3 xform_point_0(const GpuInstance_natural_0 thread* i_35, float3 p_24)
{
    float4 _S835 = float4((&i_35->m_8)->data_8[int(0)]) ;
    float4 _S836 = float4((&i_35->m_8)->data_8[int(1)]) ;
    float4 _S837 = float4((&i_35->m_8)->data_8[int(2)]) ;
    return float3(dot(_S835.xyz, p_24) + _S835.w, dot(_S836.xyz, p_24) + _S836.w, dot(_S837.xyz, p_24) + _S837.w);
}

float max_scale_0(const GpuInstance_natural_0 thread* i_36)
{
    float4 _S838 = float4((&i_36->m_8)->data_8[int(0)]) ;
    float4 _S839 = float4((&i_36->m_8)->data_8[int(1)]) ;
    float4 _S840 = float4((&i_36->m_8)->data_8[int(2)]) ;
    return sqrt(max(max(length_squared_0(float3(_S838.x, _S839.x, _S840.x)), length_squared_0(float3(_S838.y, _S839.y, _S840.y))), length_squared_0(float3(_S838.z, _S839.z, _S840.z))));
}

float3 xform_normal_0(const GpuInstance_natural_0 thread* i_37, float3 n_21)
{
    float4 _S841 = float4((&i_37->m_8)->data_8[int(0)]) ;
    float4 _S842 = float4((&i_37->m_8)->data_8[int(1)]) ;
    float4 _S843 = float4((&i_37->m_8)->data_8[int(2)]) ;
    float3 a0_1 = float3(_S841.x, _S842.x, _S843.x);
    float3 a1_1 = float3(_S841.y, _S842.y, _S843.y);
    float3 a2_3 = float3(_S841.z, _S842.z, _S843.z);
    return cross(a1_1, a2_3) * float3(n_21.x)  + cross(a2_3, a0_1) * float3(n_21.y)  + cross(a0_1, a1_1) * float3(n_21.z) ;
}

float3 tree_vertex_0(FrameData_natural_0 device* f_53, const GpuInstance_natural_0 thread* inst_1, float3 node_4, float3 pm_0, uint n_22, uint aux_0)
{
    uint _S844 = inst_1->pad_2[int(0)];
    float3 origin_1 = float3((float4((&inst_1->m_8)->data_8[int(0)]) ).w, (float4((&inst_1->m_8)->data_8[int(1)]) ).w, (float4((&inst_1->m_8)->data_8[int(2)]) ).w);
    bool _S845;
    if((aux_0 & 134217728U) != 0U)
    {
        _S845 = ((_S844 >> 16U) & (1U << (aux_0 >> 29U))) != 0U;
    }
    else
    {
        _S845 = false;
    }
    if(_S845)
    {
        return origin_1;
    }
    float3 _S846;
    if(((inst_1->node_flags_0) & 131072U) != 0U)
    {
        float3 _S847 = gc_scale_0(inst_1, length(node_4 + origin_1));
        if((_S847.x) <= 0.0f)
        {
            return origin_1;
        }
        _S846 = pm_0 * _S847;
    }
    else
    {
        _S846 = pm_0;
    }
    float3 _S848 = xform_point_0(inst_1, _S846);
    WindData_natural_0 w_19 = f_53->wind_1;
    float4 _S849 = float4(f_53->wind_1.cfg_2) ;
    if((_S849.x) < 0.5f)
    {
        return _S848;
    }
    float3 _S850 = node_4 + origin_1;
    float _S851 = _S849.y;
    float fade_1 = saturate((_S851 - length(_S850)) / max(0.25f * _S851, 1.0f));
    if(fade_1 <= 0.0f)
    {
        return _S848;
    }
    float q_18 = float((aux_0 >> 11U) & 127U) / 127.0f;
    float sway_0 = q_18 * q_18 * 4.0f;
    float flutter_0 = float((aux_0 >> 18U) & 7U) / 7.0f;
    float phase_0 = float((aux_0 >> 21U) & 63U) / 64.0f + float(_S844 >> 24U) / 255.0f;
    float4 _S852 = float4(w_19.wind_0) ;
    float2 _S853 = _S852.xz;
    float speed_1 = length(_S853);
    float2 wd_2;
    if(speed_1 > 0.0010000000474974513f)
    {
        wd_2 = _S853 / float2(speed_1) ;
    }
    else
    {
        wd_2 = float2(0.0f, 0.0f);
    }
    float4 _S854 = float4(w_19.gust_0) ;
    float s_22 = 0.5f + 0.5f * sin((_S854.x + dot(_S850.xz, wd_2) / max(_S854.y, 1.0f)) * 6.2831854820251465f);
    float gust_1 = s_22 * s_22 * s_22 * _S854.z;
    float t_20 = _S852.w;
    float _S855 = speed_1 * 0.10000000149011612f * fade_1;
    float _S856 = max_scale_0(inst_1);
    float k_17 = _S855 * _S856;
    float rock_1 = 0.55000001192092896f + gust_1 + 0.30000001192092896f * sin(t_20 * 1.7000000476837158f + phase_0 * 6.2831854820251465f) + 0.15000000596046448f * sin(t_20 * 0.62999999523162842f + phase_0 * 19.0f);
    thread float3 d_51 = float3(wd_2.x, 0.0f, wd_2.y) * float3((sway_0 * k_17 * rock_1)) ;
    d_51.y = d_51.y - 0.25f * sway_0 * k_17 * rock_1 * rock_1 * 0.30000001192092896f;
    float3 _S857 = xform_normal_0(inst_1, oct_decode_0(n_22));
    return _S848 + d_51 + normalize(_S857) * float3((flutter_0 * _S854.w * (0.60000002384185791f + gust_1) * 0.05000000074505806f * fade_1 * sin(t_20 * 11.0f + phase_0 * 40.0f + dot(_S848, float3(3.0999999046325684f, 1.7000000476837158f, 2.2999999523162842f)))))  * float3(_S856) ;
}

struct GpuMaterial_natural_0
{
    packed_float4 base_1;
    packed_float4 params_1;
    packed_float4 shade_1;
    packed_uint4 tex_2;
    packed_float4 uv_7;
};

GpuMaterial_natural_0 device* ed_buf_13(uint i_38, KernelContext_0 thread* kernelContext_68)
{
    return (GpuMaterial_natural_0 device*)kernelContext_68->ed_buffers_0[i_38];
}

struct TreeSurf_0
{
    float3 albedo_8;
    bool leaf_3;
    bool shell_0;
};

float2 tree_uv_0(uint w_20)
{
    return float2(float(w_20 & 65535U), float(w_20 >> 16U)) * float2(1.5259021893143654e-05f) ;
}

TreeSurf_0 tree_surface_0(FrameData_natural_0 device* f_54, const GpuInstance_natural_0 thread* inst_2, const GpuMaterial_natural_0 thread* m_9, uint2 w0_0, uint2 w1_0, uint2 w2_0, float b0_0, float b1_4, float b2_6, float2 pix_0, float3 p0_2, float3 e1_3, float3 e2_3, KernelContext_0 thread* kernelContext_69)
{
    uint a_23 = w0_0.y;
    uint _S858 = w1_0.y;
    uint _S859 = w2_0.y;
    float val_0 = (float(a_23 & 127U) * b0_0 + float(_S858 & 127U) * b1_4 + float(_S859 & 127U) * b2_6) * 0.015748031437397003f;
    float hue_1 = (float((a_23 >> 7U) & 15U) * b0_0 + float((_S858 >> 7U) & 15U) * b1_4 + float((_S859 >> 7U) & 15U) * b2_6) * 0.13333334028720856f - 1.0f;
    float iv_1 = float((inst_2->pad_2[int(0)]) & 255U) / 128.0f;
    float ih_0 = float(((inst_2->pad_2[int(0)]) >> 8U) & 255U) / 127.5f - 1.0f;
    thread TreeSurf_0 o_3;
    (&o_3)->leaf_3 = (a_23 & 134217728U) != 0U;
    (&o_3)->shell_0 = false;
    if((a_23 & 268435456U) != 0U)
    {
        (&o_3)->albedo_8 = (float4(m_9->shade_1) ).xyz * float3(val_0)  * float3(mix(1.0f, iv_1, 0.5f)) ;
        return o_3;
    }
    float3 alb_3 = (float4(m_9->base_1) ).xyz * float3(val_0)  * float3(iv_1)  * tree_hue_0(clamp(hue_1 + 0.5f * ih_0, -1.0f, 1.0f));
    bool _S860;
    if((&o_3)->leaf_3)
    {
        _S860 = ((uint4(m_9->tex_2) ).x) != 4294967295U;
    }
    else
    {
        _S860 = false;
    }
    float3 alb_4;
    if(_S860)
    {
        float2 t0_1 = tree_uv_0(w0_0.x);
        float2 d1_1 = tree_uv_0(w1_0.x) - t0_1;
        float2 d2_1 = tree_uv_0(w2_0.x) - t0_1;
        float2 uv_8 = t0_1 + d1_1 * float2(b1_4)  + d2_1 * float2(b2_6) ;
        float3 _S861 = view_dir_0(f_54, pix_0 + float2(1.0f, 0.0f));
        float2 bx_2 = ray_bary_0(_S861, p0_2, e1_3, e2_3);
        float3 _S862 = view_dir_0(f_54, pix_0 + float2(0.0f, 1.0f));
        float2 by_2 = ray_bary_0(_S862, p0_2, e1_3, e2_3);
        float2 ux_0 = t0_1 + d1_1 * float2(bx_2.x)  + d2_1 * float2(bx_2.y)  - uv_8;
        float2 uy_0 = t0_1 + d1_1 * float2(by_2.x)  + d2_1 * float2(by_2.y)  - uv_8;
        texture2d<float, access::sample> _S863 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_69->ed_textures_0))[((uint4(m_9->tex_2) ).x)]));
        sampler _S864 = ((reinterpret_cast<device sampler*>((kernelContext_69->ed_samplers_0))[(kernelContext_69->ed_c_0->smp_0)]));
        alb_4 = alb_3 * clamp(((_S863).sample((_S864), (uv_8), gradient2d((ux_0), (uy_0)))).xyz / float3(0.14699999988079071f, 0.27399998903274536f, 0.050999999046325684f), float3(0.20000000298023224f, 0.20000000298023224f, 0.20000000298023224f), float3(2.5f, 2.5f, 2.5f));
    }
    else
    {
        alb_4 = alb_3;
    }
    if(((float4((&f_54->season_0)->cfg_4) ).x) > 0.0f)
    {
        LeafSeason_0 _S865 = season_of_word_0(f_54, inst_2->pad_2[int(1)]);
        thread LeafSeason_0 _S866 = _S865;
        float3 _S867 = season_tint_0(&_S866, alb_4);
        alb_4 = _S867;
    }
    (&o_3)->shell_0 = !(&o_3)->leaf_3;
    (&o_3)->albedo_8 = min(alb_4, float3(0.89999997615814209f, 0.89999997615814209f, 0.89999997615814209f));
    return o_3;
}

float bark_height_0(float2 q_19)
{
    float _S868 = q_19.x;
    float _S869 = q_19.y;
    float plates_0 = t_noise_0(float2(_S868, _S869 * 0.30000001192092896f), 0.070000000298023224f);
    float _S870 = t_noise_0(q_19 * float2(1.0f, 0.25f) + float2(31.0f) , 0.18000000715255737f);
    float _S871 = t_noise_0(float2(_S868 + (_S870 - 0.5f) * 0.079999998211860657f, _S869 * 0.11999999731779099f), 0.035000000149011612f);
    return saturate(smoothstep(0.2199999988079071f, 0.51999998092651367f, _S871) * (0.55000001192092896f + 0.44999998807907104f * plates_0));
}

packed_float2 device* ed_buf_14(uint i_39, KernelContext_0 thread* kernelContext_70)
{
    return (packed_float2 device*)kernelContext_70->ed_buffers_0[i_39];
}

float2 uv_xform_0(const GpuMaterial_natural_0 thread* m_10, float2 uv_9)
{
    float r_13 = (as_type<float>(((uint4(m_10->tex_2) ).w)));
    float s_23 = sin(r_13);
    float c_24 = cos(r_13);
    float4 _S872 = float4(m_10->uv_7) ;
    float2 q_20 = uv_9 * _S872.zw;
    float _S873 = q_20.x;
    float _S874 = q_20.y;
    return float2(c_24 * _S873 + s_23 * _S874, - s_23 * _S873 + c_24 * _S874) + _S872.xy;
}

float3 shade_toon_0(FrameData_natural_0 device* f_55, float3 sun_7, const GpuMaterial_natural_0 thread* m_11, float3 albedo_9, float3 n_23, float3 v_14, float ao_9, float sv_6, float sun_vis_3, KernelContext_0 thread* kernelContext_71)
{
    float ndl_0 = dot(n_23, (float4(f_55->sun_dir_0) ).xyz);
    float _S875 = max((float4(m_11->params_1) ).z, 1.0f);
    float lit_2 = floor(saturate(ndl_0 * 0.5f + 0.5f) * _S875 + 0.5f) / _S875 * sun_vis_3;
    float4 _S876 = float4(m_11->shade_1) ;
    float3 c_25 = mix(albedo_9 * _S876.xyz, albedo_9, float3(lit_2) );
    float3 _S877 = sun_7 * float3((lit_2 / 3.1415927410125732f)) ;
    float3 _S878 = ambient_light_0(f_55, n_23, kernelContext_71);
    float3 _S879 = occluded_ambient_0(f_55, _S878, sv_6, kernelContext_71);
    float3 _S880 = ao_multibounce_0(f_55, ao_9, albedo_9);
    return c_25 * (_S877 + _S879 * _S880) + float3((pow(1.0f - saturate(dot(n_23, v_14)), 4.0f) * _S876.w * saturate(ndl_0 + 0.30000001192092896f) * sun_vis_3))  * sun_7 * float3(0.15000000596046448f) ;
}

float cascade_depth_delta_0(FrameData_natural_0 device* f_56, uint atlas_2, uint c_26, float3 p_25, KernelContext_0 thread* kernelContext_72)
{
    float4 _S881 = float4((&(&f_56->light_0)->cascade_0)->data_3[c_26]) ;
    float tile_2 = (float4((&f_56->light_0)->cfg_0) ).y;
    float4 s_24 = (((float4(p_25, 1.0f)) * (matrix<float,int(4),int(4)> ((&(&f_56->light_0)->shadow_1)->data_2[c_26].data_0[int(0)][int(0)], (&(&f_56->light_0)->shadow_1)->data_2[c_26].data_0[int(1)][int(0)], (&(&f_56->light_0)->shadow_1)->data_2[c_26].data_0[int(2)][int(0)], (&(&f_56->light_0)->shadow_1)->data_2[c_26].data_0[int(3)][int(0)], (&(&f_56->light_0)->shadow_1)->data_2[c_26].data_0[int(0)][int(1)], (&(&f_56->light_0)->shadow_1)->data_2[c_26].data_0[int(1)][int(1)], (&(&f_56->light_0)->shadow_1)->data_2[c_26].data_0[int(2)][int(1)], (&(&f_56->light_0)->shadow_1)->data_2[c_26].data_0[int(3)][int(1)], (&(&f_56->light_0)->shadow_1)->data_2[c_26].data_0[int(0)][int(2)], (&(&f_56->light_0)->shadow_1)->data_2[c_26].data_0[int(1)][int(2)], (&(&f_56->light_0)->shadow_1)->data_2[c_26].data_0[int(2)][int(2)], (&(&f_56->light_0)->shadow_1)->data_2[c_26].data_0[int(3)][int(2)], (&(&f_56->light_0)->shadow_1)->data_2[c_26].data_0[int(0)][int(3)], (&(&f_56->light_0)->shadow_1)->data_2[c_26].data_0[int(1)][int(3)], (&(&f_56->light_0)->shadow_1)->data_2[c_26].data_0[int(2)][int(3)], (&(&f_56->light_0)->shadow_1)->data_2[c_26].data_0[int(3)][int(3)]))));
    int2 _S882 = int2(_S881.zw);
    float _S883 = s_24.z;
    float _S884 = ed_depth_read_0(atlas_2, uint2(clamp(_S882 + int2(floor(float2(s_24.x * 0.5f + 0.5f, 0.5f - s_24.y * 0.5f) * float2(tile_2) )), _S882, _S882 + int2((int(tile_2) - int(1))) )), kernelContext_72);
    return (_S883 - _S884) * _S881.y;
}

float4 resolve_px_0(const FsOut_0 thread* i_40, bool terrain_0, KernelContext_0 thread* kernelContext_73)
{
    FrameData_natural_0 device* _S885 = ed_buf_1(kernelContext_73->ed_c_0->frame_0, kernelContext_73);
    float2 _S886 = i_40->pos_0.xy;
    uint2 _S887 = uint2(_S886);
    texture2d<uint, access::sample> _S888 = ((reinterpret_cast<device texture2d<uint, access::sample>*>((kernelContext_73->ed_textures_0))[(kernelContext_73->ed_c_0->vis_0)]));
    int3 _S889 = int3(int2(_S887), int(0));
    uint2 vis_4 = ((_S888).read(vec<uint,2>(((_S889)).xy), uint(((_S889)).z))).xy;
    float3 _S890 = view_dir_0(_S885, _S886);
    uint _S891 = vis_4.x;
    if(_S891 == 0U)
    {
        float3 _S892 = sky_pixel_0(_S885, _S890, kernelContext_73);
        return float4(_S892, 1.0f);
    }
    bool textured_0;
    if(terrain_0)
    {
        textured_0 = (_S891 & 2147483648U) != 0U;
    }
    else
    {
        textured_0 = false;
    }
    if(textured_0)
    {
        float4 _S893 = terrain_pixel_0(_S885, i_40, vis_4, _S890, kernelContext_73);
        return _S893;
    }
    if((_S891 & 3758096384U) == 536870912U)
    {
        float4 _S894 = grass_pixel_0(_S885, i_40, vis_4, _S890, kernelContext_73);
        return _S894;
    }
    if((_S891 & 3221225472U) == 1073741824U)
    {
        float4 _S895 = impostor_pixel_0(_S885, i_40, _S891 & 3221225471U, _S890, kernelContext_73);
        return _S895;
    }
    GpuInstance_natural_0 device* _S896 = ed_buf_9(kernelContext_73->ed_c_0->instances_0, kernelContext_73);
    thread GpuInstance_natural_0 _S897 = *(_S896 + (_S891 - 1U));
    GpuMesh_natural_0 device* _S898 = ed_buf_10(kernelContext_73->ed_c_0->meshes_0, kernelContext_73);
    GpuMesh_natural_0 device* _S899 = _S898 + (&_S897)->mesh_0;
    uint _S900 = vis_4.y;
    uint lod_3 = _S900 >> 28U;
    uint prim_1 = (_S900 & 268435455U) % max(_S899->index_count_0[lod_3] / 3U, 1U);
    uint device* _S901 = ed_buf_0(kernelContext_73->ed_c_0->indices_0, kernelContext_73);
    uint device* idx_1 = _S901 + _S899->first_index_0[lod_3] + prim_1 * 3U;
    Vtx_0 device* _S902 = ed_buf_11(kernelContext_73->ed_c_0->verts_0, kernelContext_73);
    int i0_0 = int(*(idx_1 + int(0))) + _S899->base_vertex_0[lod_3];
    int i1_0 = int(*(idx_1 + int(1))) + _S899->base_vertex_0[lod_3];
    int i2_0 = int(*(idx_1 + int(2))) + _S899->base_vertex_0[lod_3];
    Vtx_0 v0_0 = *(_S902 + i0_0);
    Vtx_0 v1_0 = *(_S902 + i1_0);
    Vtx_0 v2_0 = *(_S902 + i2_0);
    packed_float4 device* _S903 = ed_buf_4(kernelContext_73->ed_c_0->nodes_0, kernelContext_73);
    uint _S904 = (&_S897)->node_flags_0;
    float3 node_5 = (float4(*(_S903 + (((&_S897)->node_flags_0) & 65535U))) ).xyz;
    bool foliage_0 = ((_S899->flags_1) & 1U) != 0U;
    uint2 _S905 = uint2(0U, 0U);
    float3 l0_0;
    float3 l1_0;
    float3 l2_0;
    uint2 tw0_0;
    uint2 tw1_0;
    uint2 tw2_1;
    if(foliage_0)
    {
        packed_uint2 device* _S906 = ed_buf_12(kernelContext_73->ed_c_0->uvs_0, kernelContext_73);
        uint2 _S907 = uint2(*(_S906 + i0_0)) ;
        uint2 _S908 = uint2(*(_S906 + i1_0)) ;
        uint2 _S909 = uint2(*(_S906 + i2_0)) ;
        float3 _S910 = tree_vertex_0(_S885, &_S897, node_5, float3(v0_0.px_7, v0_0.py_2, v0_0.pz_2), v0_0.n_20, _S907.y);
        float3 _S911 = tree_vertex_0(_S885, &_S897, node_5, float3(v1_0.px_7, v1_0.py_2, v1_0.pz_2), v1_0.n_20, _S908.y);
        float3 _S912 = tree_vertex_0(_S885, &_S897, node_5, float3(v2_0.px_7, v2_0.py_2, v2_0.pz_2), v2_0.n_20, _S909.y);
        l0_0 = _S910;
        l1_0 = _S911;
        l2_0 = _S912;
        tw0_0 = _S907;
        tw1_0 = _S908;
        tw2_1 = _S909;
    }
    else
    {
        float3 _S913 = xform_point_0(&_S897, float3(v0_0.px_7, v0_0.py_2, v0_0.pz_2));
        float3 _S914 = xform_point_0(&_S897, float3(v1_0.px_7, v1_0.py_2, v1_0.pz_2));
        float3 _S915 = xform_point_0(&_S897, float3(v2_0.px_7, v2_0.py_2, v2_0.pz_2));
        l0_0 = _S913;
        l1_0 = _S914;
        l2_0 = _S915;
        tw0_0 = _S905;
        tw1_0 = _S905;
        tw2_1 = _S905;
    }
    float3 p0_3 = node_5 + l0_0;
    float3 e1_4 = l1_0 - l0_0;
    float3 e2_4 = l2_0 - l0_0;
    float2 bb_2 = ray_bary_0(_S890, p0_3, e1_4, e2_4);
    float b1_5 = saturate(bb_2.x);
    float b2_7 = saturate(bb_2.y);
    float bs_2 = b1_5 + b2_7;
    float b1_6;
    float b2_8;
    if(bs_2 > 1.0f)
    {
        float b2_9 = b2_7 / bs_2;
        b1_6 = b1_5 / bs_2;
        b2_8 = b2_9;
    }
    else
    {
        b1_6 = b1_5;
        b2_8 = b2_7;
    }
    float b0_1 = 1.0f - b1_6 - b2_8;
    float3 _S916 = float3(b0_1) ;
    float3 local_0 = l0_0 * _S916 + l1_0 * float3(b1_6)  + l2_0 * float3(b2_8) ;
    float3 _S917 = xform_normal_0(&_S897, oct_decode_0(v0_0.n_20) * _S916 + oct_decode_0(v1_0.n_20) * float3(b1_6)  + oct_decode_0(v2_0.n_20) * float3(b2_8) );
    float3 n_24 = normalize(_S917);
    float3 v_15 = - _S890;
    GpuMaterial_natural_0 device* _S918 = ed_buf_13(kernelContext_73->ed_c_0->materials_0, kernelContext_73);
    thread GpuMaterial_natural_0 _S919 = *(_S918 + (&_S897)->material_0);
    float4 _S920 = float4((&_S919)->base_1) ;
    float3 albedo_10 = _S920.xyz;
    float4 _S921 = float4((&_S919)->params_1) ;
    float metal_1 = _S921.x;
    float rough_6 = _S921.y;
    uint4 _S922 = uint4((&_S919)->tex_2) ;
    uint _S923 = _S922.x;
    bool _S924 = _S923 != 4294967295U;
    if(_S924)
    {
        textured_0 = true;
    }
    else
    {
        textured_0 = (_S922.y) != 4294967295U;
    }
    if(textured_0)
    {
        textured_0 = true;
    }
    else
    {
        textured_0 = (_S922.z) != 4294967295U;
    }
    bool leaf_4;
    float3 albedo_11;
    float3 n_25;
    float ao_10;
    float rough_7;
    float metal_2;
    float d_52;
    if(foliage_0)
    {
        TreeSurf_0 _S925 = tree_surface_0(_S885, &_S897, &_S919, tw0_0, tw1_0, tw2_1, b0_1, b1_6, b2_8, _S886, p0_3, e1_4, e2_4, kernelContext_73);
        if(_S925.shell_0)
        {
            float3 _S926 = xform_normal_0(&_S897, float3(0.0f, 1.0f, 0.0f));
            float3 up_13 = normalize(_S926);
            float clump_0 = t_noise_0(local_0.xz * float2(0.89999997615814209f)  + float2((local_0.y * 0.60000002384185791f)) , 1.0f);
            float3 albedo_12 = _S925.albedo_8 * float3((mix(0.60000002384185791f, 1.1499999761581421f, clump_0) * 0.85000002384185791f)) ;
            float _S927 = mix(0.64999997615814209f, 1.0f, clump_0);
            n_25 = normalize(mix(n_24, up_13, float3(0.40000000596046448f) ));
            albedo_11 = albedo_12;
            ao_10 = _S927;
            rough_7 = 1.0f;
        }
        else
        {
            n_25 = n_24;
            albedo_11 = _S925.albedo_8;
            ao_10 = 1.0f;
            rough_7 = 0.81999999284744263f;
        }
        if((_S904 & 131072U) != 0U)
        {
            float mot_0 = t_noise_0(local_0.xz * float2(1.7000000476837158f)  + float2((local_0.y * 3.0f)) , 1.0f);
            float3 albedo_13 = albedo_11 * float3(mix(0.81999999284744263f, 1.1200000047683716f, mot_0)) ;
            if(_S925.leaf_3)
            {
                b1_6 = 0.75f;
            }
            else
            {
                b1_6 = 0.89999997615814209f;
            }
            albedo_11 = albedo_13;
            rough_7 = b1_6;
        }
        else
        {
            if(!_S925.leaf_3)
            {
                textured_0 = !_S925.shell_0;
            }
            else
            {
                textured_0 = false;
            }
            if(textured_0)
            {
                textured_0 = (length_squared_0(p0_3)) < 3600.0f;
            }
            else
            {
                textured_0 = false;
            }
            if(textured_0)
            {
                float3 pm_1 = float3(v0_0.px_7, v0_0.py_2, v0_0.pz_2) * _S916 + float3(v1_0.px_7, v1_0.py_2, v1_0.pz_2) * float3(b1_6)  + float3(v2_0.px_7, v2_0.py_2, v2_0.pz_2) * float3(b2_8) ;
                float _S928 = pm_1.y;
                float2 q_21 = float2(atan2(pm_1.z, pm_1.x) * max(length(pm_1.xz), 0.019999999552965164f), _S928);
                float hb_0 = bark_height_0(q_21);
                float _S929 = t_noise_0(q_21 * float2(1.0f, 0.5f) + float2(7.0f) , 0.44999998807907104f);
                float3 _S930 = mix(albedo_11, float3(dot(albedo_11, float3(0.30000001192092896f, 0.5f, 0.20000000298023224f)))  * float3(1.0499999523162842f, 1.0499999523162842f, 1.0f) * float3(1.3500000238418579f) , float3((smoothstep(0.55000001192092896f, 0.85000002384185791f, _S929) * 0.44999998807907104f)) ) * float3(mix(0.5f, 1.1200000047683716f, hb_0))  * float3(mix(0.72000002861022949f, 1.0f, smoothstep(0.0f, 1.2000000476837158f, _S928))) ;
                float3 _S931 = xform_normal_0(&_S897, float3(0.0f, 1.0f, 0.0f));
                float3 tu_8 = cross(normalize(_S931), n_25);
                if((length_squared_0(p0_3)) < 400.0f)
                {
                    textured_0 = (length_squared_0(tu_8)) > 9.99999997475242708e-07f;
                }
                else
                {
                    textured_0 = false;
                }
                if(textured_0)
                {
                    float hu_0 = bark_height_0(q_21 + float2(0.012000000104308128f, 0.0f));
                    float hv_0 = bark_height_0(q_21 + float2(0.0f, 0.012000000104308128f));
                    float3 tu_9 = normalize(tu_8);
                    n_25 = normalize(n_25 - float3(0.89999997615814209f)  * (float3(((hu_0 - hb_0) / 0.012000000104308128f * 0.029999999329447746f))  * tu_9 + float3(((hv_0 - hb_0) / 0.012000000104308128f * 0.029999999329447746f))  * normalize(cross(n_25, tu_9))));
                }
                float ao_11 = ao_10 * mix(0.64999997615814209f, 1.0f, hb_0);
                albedo_11 = _S930;
                ao_10 = ao_11;
            }
        }
        if(_S925.leaf_3)
        {
            float3 ngc_0 = cross(e1_4, e2_4);
            if((dot(ngc_0, v_15) * dot(ngc_0, n_25)) < 0.0f)
            {
                n_25 = - n_25;
            }
        }
        leaf_4 = _S925.leaf_3;
        metal_2 = 0.0f;
    }
    else
    {
        if(textured_0)
        {
            packed_float2 device* _S932 = ed_buf_14(kernelContext_73->ed_c_0->uvs_0, kernelContext_73);
            float2 _S933 = float2(*(_S932 + i0_0)) ;
            float2 d1_2 = float2(*(_S932 + i1_0))  - _S933;
            float2 d2_2 = float2(*(_S932 + i2_0))  - _S933;
            float2 _S934 = uv_xform_0(&_S919, _S933 + d1_2 * float2(b1_6)  + d2_2 * float2(b2_8) );
            float3 _S935 = view_dir_0(_S885, _S886 + float2(1.0f, 0.0f));
            float2 bx_3 = ray_bary_0(_S935, p0_3, e1_4, e2_4);
            float3 _S936 = view_dir_0(_S885, _S886 + float2(0.0f, 1.0f));
            float2 by_3 = ray_bary_0(_S936, p0_3, e1_4, e2_4);
            float2 _S937 = uv_xform_0(&_S919, _S933 + d1_2 * float2(bx_3.x)  + d2_2 * float2(bx_3.y) );
            float2 ux_1 = _S937 - _S934;
            float2 _S938 = uv_xform_0(&_S919, _S933 + d1_2 * float2(by_3.x)  + d2_2 * float2(by_3.y) );
            float2 uy_1 = _S938 - _S934;
            sampler _S939 = ((reinterpret_cast<device sampler*>((kernelContext_73->ed_samplers_0))[(kernelContext_73->ed_c_0->smp_0)]));
            if(_S924)
            {
                texture2d<float, access::sample> _S940 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_73->ed_textures_0))[(_S923)]));
                albedo_11 = albedo_10 * ((_S940).sample((_S939), (_S934), gradient2d((ux_1), (uy_1)))).xyz;
            }
            else
            {
                albedo_11 = albedo_10;
            }
            uint _S941 = _S922.z;
            if(_S941 != 4294967295U)
            {
                texture2d<float, access::sample> _S942 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_73->ed_textures_0))[(_S941)]));
                float3 orm_0 = ((_S942).sample((_S939), (_S934), gradient2d((ux_1), (uy_1)))).xyz;
                float rough_8 = rough_6 * orm_0.y;
                float metal_3 = metal_1 * orm_0.z;
                ao_10 = orm_0.x;
                rough_7 = rough_8;
                metal_2 = metal_3;
            }
            else
            {
                ao_10 = 1.0f;
                rough_7 = rough_6;
                metal_2 = metal_1;
            }
            uint _S943 = _S922.y;
            if(_S943 != 4294967295U)
            {
                float _S944 = d1_2.x;
                float _S945 = d2_2.y;
                float _S946 = d1_2.y;
                float _S947 = d2_2.x;
                float det_2 = _S944 * _S945 - _S946 * _S947;
                if((abs(det_2)) > 9.999999960041972e-13f)
                {
                    float3 _S948 = float3((1.0f / det_2)) ;
                    float3 tg_0 = (e1_4 * float3(_S945)  - e2_4 * float3(_S946) ) * _S948;
                    float3 bt_0 = (e2_4 * float3(_S944)  - e1_4 * float3(_S947) ) * _S948;
                    float3 tg_1 = tg_0 - n_24 * float3(dot(n_24, tg_0)) ;
                    if((length_squared_0(tg_1)) > 9.99999968265522539e-21f)
                    {
                        float3 tg_2 = normalize(tg_1);
                        float3 _S949 = cross(n_24, tg_2);
                        if((dot(_S949, bt_0)) < 0.0f)
                        {
                            d_52 = -1.0f;
                        }
                        else
                        {
                            d_52 = 1.0f;
                        }
                        float3 bn_0 = _S949 * float3(- d_52) ;
                        texture2d<float, access::sample> _S950 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_73->ed_textures_0))[(_S943)]));
                        float2 xy_0 = ((_S950).sample((_S939), (_S934), gradient2d((ux_1), (uy_1)))).xy * float2(2.0f)  - float2(1.0f) ;
                        n_25 = normalize(tg_2 * float3(xy_0.x)  + bn_0 * float3(xy_0.y)  + n_24 * float3(sqrt(saturate(1.0f - dot(xy_0, xy_0)))) );
                    }
                    else
                    {
                        n_25 = n_24;
                    }
                }
                else
                {
                    n_25 = n_24;
                }
            }
            else
            {
                n_25 = n_24;
            }
        }
        else
        {
            n_25 = n_24;
            albedo_11 = albedo_10;
            ao_10 = 1.0f;
            rough_7 = rough_6;
            metal_2 = metal_1;
        }
        leaf_4 = false;
    }
    float _S951 = dot(n_25, v_15);
    if(_S951 < 0.0f)
    {
        n_25 = normalize(n_25 - float3((1.0099999904632568f * _S951))  * v_15);
    }
    if((_S921.w) > 0.5f)
    {
        float2 c_27 = floor(local_0.xz * float2(0.5f) );
        albedo_11 = albedo_11 * float3(mix(0.80000001192092896f, 1.1499999761581421f, fmod(abs(c_27.x + c_27.y), 2.0f))) ;
    }
    float3 p_26 = node_5 + local_0;
    float zlin_5 = - (((float4(p_26, 1.0f)) * (matrix<float,int(4),int(4)> (_S885->view_0.data_0[int(0)][int(0)], _S885->view_0.data_0[int(1)][int(0)], _S885->view_0.data_0[int(2)][int(0)], _S885->view_0.data_0[int(3)][int(0)], _S885->view_0.data_0[int(0)][int(1)], _S885->view_0.data_0[int(1)][int(1)], _S885->view_0.data_0[int(2)][int(1)], _S885->view_0.data_0[int(3)][int(1)], _S885->view_0.data_0[int(0)][int(2)], _S885->view_0.data_0[int(1)][int(2)], _S885->view_0.data_0[int(2)][int(2)], _S885->view_0.data_0[int(3)][int(2)], _S885->view_0.data_0[int(0)][int(3)], _S885->view_0.data_0[int(1)][int(3)], _S885->view_0.data_0[int(2)][int(3)], _S885->view_0.data_0[int(3)][int(3)])))).z;
    float3 ng_7 = cross(e1_4, e2_4);
    float3 ng_8;
    if((length_squared_0(ng_7)) > 9.99999968265522539e-21f)
    {
        ng_8 = normalize(ng_7);
    }
    else
    {
        ng_8 = n_25;
    }
    if((dot(ng_8, v_15)) < 0.0f)
    {
        ng_8 = - ng_8;
    }
    float _S952 = sun_shadow_0(_S885, kernelContext_73->ed_c_0->shadow_0, p_26, ng_8, zlin_5, _S886, kernelContext_73);
    float2 _S953 = ao_upsample_0(_S885, kernelContext_73->ed_c_0->ao_0, _S886, zlin_5, kernelContext_73);
    float ao_12 = ao_10 * _S953.x;
    float sv_7 = _S953.y;
    float3 _S954 = sun_light_at_0(_S885, p_26, kernelContext_73);
    float3 col_7;
    if(foliage_0)
    {
        float3 _S955 = shade_foliage_0(_S885, _S954, albedo_11, rough_7, ao_12, sv_7, _S952, n_25, ng_8, v_15, leaf_4, kernelContext_73);
        col_7 = _S955;
    }
    else
    {
        if((_S920.w) > 0.5f)
        {
            float3 _S956 = shade_toon_0(_S885, _S954, &_S919, albedo_11, n_25, v_15, ao_12, sv_7, _S952, kernelContext_73);
            l0_0 = _S956;
        }
        else
        {
            float3 _S957 = shade_pbr_0(_S885, _S954, albedo_11, metal_2, rough_7, ao_12, sv_7, _S952, n_25, v_15, kernelContext_73);
            l0_0 = _S957;
        }
        col_7 = l0_0;
    }
    float3 _S958 = apply_aerial_0(_S885, col_7, p_26, _S886, kernelContext_73);
    if(((uint4((&_S885->light_0)->flags_0) ).w) == 1U)
    {
        uint _S959 = shadow_cascade_0(_S885, p_26, zlin_5);
        col_7 = TINT_2[_S959] * float3((0.039999999105930328f + 0.5f * _S952)) ;
    }
    else
    {
        if(((uint4((&_S885->light_0)->flags_0) ).w) == 3U)
        {
            uint _S960 = shadow_cascade_0(_S885, p_26, zlin_5);
            if(_S960 < 4U)
            {
                float _S961 = cascade_depth_delta_0(_S885, kernelContext_73->ed_c_0->shadow_0, _S960, p_26, kernelContext_73);
                d_52 = _S961;
            }
            else
            {
                d_52 = 0.0f;
            }
            col_7 = float3(max(d_52, 0.0f), max(- d_52, 0.0f), 0.0f) * float3(0.10000000149011612f) ;
        }
        else
        {
            if(((uint4((&_S885->light_0)->flags_0) ).w) == 2U)
            {
                col_7 = float3((0.5f * ao_12)) ;
            }
            else
            {
                if(((uint4((&_S885->light_0)->flags_0) ).w) == 4U)
                {
                    col_7 = float3((0.5f * sv_7)) ;
                }
                else
                {
                    col_7 = _S958;
                }
            }
        }
    }
    return float4(col_7, 1.0f);
}

constant array<float3, int(5)> TINT_3 = { { float3(1.0f, 0.25f, 0.25f), float3(0.25f, 1.0f, 0.25f), float3(0.30000001192092896f, 0.44999998807907104f, 1.0f), float3(1.0f, 1.0f, 0.25f), float3(0.60000002384185791f, 0.60000002384185791f, 0.60000002384185791f) } };
float4 resolve_px_1(const FsOut_0 thread* i_41, bool terrain_1, KernelContext_0 thread* kernelContext_74)
{
    FrameData_natural_0 device* _S962 = ed_buf_1(kernelContext_74->ed_c_0->frame_0, kernelContext_74);
    float2 _S963 = i_41->pos_0.xy;
    uint2 _S964 = uint2(_S963);
    texture2d<uint, access::sample> _S965 = ((reinterpret_cast<device texture2d<uint, access::sample>*>((kernelContext_74->ed_textures_0))[(kernelContext_74->ed_c_0->vis_0)]));
    int3 _S966 = int3(int2(_S964), int(0));
    uint2 vis_5 = ((_S965).read(vec<uint,2>(((_S966)).xy), uint(((_S966)).z))).xy;
    float3 _S967 = view_dir_0(_S962, _S963);
    uint _S968 = vis_5.x;
    if(_S968 == 0U)
    {
        float3 _S969 = sky_pixel_0(_S962, _S967, kernelContext_74);
        return float4(_S969, 1.0f);
    }
    bool textured_1;
    if(terrain_1)
    {
        textured_1 = (_S968 & 2147483648U) != 0U;
    }
    else
    {
        textured_1 = false;
    }
    if(textured_1)
    {
        float4 _S970 = terrain_pixel_1(_S962, i_41, vis_5, _S967, kernelContext_74);
        return _S970;
    }
    if((_S968 & 3758096384U) == 536870912U)
    {
        float4 _S971 = grass_pixel_0(_S962, i_41, vis_5, _S967, kernelContext_74);
        return _S971;
    }
    if((_S968 & 3221225472U) == 1073741824U)
    {
        float4 _S972 = impostor_pixel_0(_S962, i_41, _S968 & 3221225471U, _S967, kernelContext_74);
        return _S972;
    }
    GpuInstance_natural_0 device* _S973 = ed_buf_9(kernelContext_74->ed_c_0->instances_0, kernelContext_74);
    thread GpuInstance_natural_0 _S974 = *(_S973 + (_S968 - 1U));
    GpuMesh_natural_0 device* _S975 = ed_buf_10(kernelContext_74->ed_c_0->meshes_0, kernelContext_74);
    GpuMesh_natural_0 device* _S976 = _S975 + (&_S974)->mesh_0;
    uint _S977 = vis_5.y;
    uint lod_4 = _S977 >> 28U;
    uint prim_2 = (_S977 & 268435455U) % max(_S976->index_count_0[lod_4] / 3U, 1U);
    uint device* _S978 = ed_buf_0(kernelContext_74->ed_c_0->indices_0, kernelContext_74);
    uint device* idx_2 = _S978 + _S976->first_index_0[lod_4] + prim_2 * 3U;
    Vtx_0 device* _S979 = ed_buf_11(kernelContext_74->ed_c_0->verts_0, kernelContext_74);
    int i0_1 = int(*(idx_2 + int(0))) + _S976->base_vertex_0[lod_4];
    int i1_1 = int(*(idx_2 + int(1))) + _S976->base_vertex_0[lod_4];
    int i2_1 = int(*(idx_2 + int(2))) + _S976->base_vertex_0[lod_4];
    Vtx_0 v0_1 = *(_S979 + i0_1);
    Vtx_0 v1_1 = *(_S979 + i1_1);
    Vtx_0 v2_1 = *(_S979 + i2_1);
    packed_float4 device* _S980 = ed_buf_4(kernelContext_74->ed_c_0->nodes_0, kernelContext_74);
    uint _S981 = (&_S974)->node_flags_0;
    float3 node_6 = (float4(*(_S980 + (((&_S974)->node_flags_0) & 65535U))) ).xyz;
    bool foliage_1 = ((_S976->flags_1) & 1U) != 0U;
    uint2 _S982 = uint2(0U, 0U);
    float3 l0_1;
    float3 l1_1;
    float3 l2_1;
    uint2 tw0_1;
    uint2 tw1_1;
    uint2 tw2_2;
    if(foliage_1)
    {
        packed_uint2 device* _S983 = ed_buf_12(kernelContext_74->ed_c_0->uvs_0, kernelContext_74);
        uint2 _S984 = uint2(*(_S983 + i0_1)) ;
        uint2 _S985 = uint2(*(_S983 + i1_1)) ;
        uint2 _S986 = uint2(*(_S983 + i2_1)) ;
        float3 _S987 = tree_vertex_0(_S962, &_S974, node_6, float3(v0_1.px_7, v0_1.py_2, v0_1.pz_2), v0_1.n_20, _S984.y);
        float3 _S988 = tree_vertex_0(_S962, &_S974, node_6, float3(v1_1.px_7, v1_1.py_2, v1_1.pz_2), v1_1.n_20, _S985.y);
        float3 _S989 = tree_vertex_0(_S962, &_S974, node_6, float3(v2_1.px_7, v2_1.py_2, v2_1.pz_2), v2_1.n_20, _S986.y);
        l0_1 = _S987;
        l1_1 = _S988;
        l2_1 = _S989;
        tw0_1 = _S984;
        tw1_1 = _S985;
        tw2_2 = _S986;
    }
    else
    {
        float3 _S990 = xform_point_0(&_S974, float3(v0_1.px_7, v0_1.py_2, v0_1.pz_2));
        float3 _S991 = xform_point_0(&_S974, float3(v1_1.px_7, v1_1.py_2, v1_1.pz_2));
        float3 _S992 = xform_point_0(&_S974, float3(v2_1.px_7, v2_1.py_2, v2_1.pz_2));
        l0_1 = _S990;
        l1_1 = _S991;
        l2_1 = _S992;
        tw0_1 = _S982;
        tw1_1 = _S982;
        tw2_2 = _S982;
    }
    float3 p0_4 = node_6 + l0_1;
    float3 e1_5 = l1_1 - l0_1;
    float3 e2_5 = l2_1 - l0_1;
    float2 bb_3 = ray_bary_0(_S967, p0_4, e1_5, e2_5);
    float b1_7 = saturate(bb_3.x);
    float b2_10 = saturate(bb_3.y);
    float bs_3 = b1_7 + b2_10;
    float b1_8;
    float b2_11;
    if(bs_3 > 1.0f)
    {
        float b2_12 = b2_10 / bs_3;
        b1_8 = b1_7 / bs_3;
        b2_11 = b2_12;
    }
    else
    {
        b1_8 = b1_7;
        b2_11 = b2_10;
    }
    float b0_2 = 1.0f - b1_8 - b2_11;
    float3 _S993 = float3(b0_2) ;
    float3 local_1 = l0_1 * _S993 + l1_1 * float3(b1_8)  + l2_1 * float3(b2_11) ;
    float3 _S994 = xform_normal_0(&_S974, oct_decode_0(v0_1.n_20) * _S993 + oct_decode_0(v1_1.n_20) * float3(b1_8)  + oct_decode_0(v2_1.n_20) * float3(b2_11) );
    float3 n_26 = normalize(_S994);
    float3 v_16 = - _S967;
    GpuMaterial_natural_0 device* _S995 = ed_buf_13(kernelContext_74->ed_c_0->materials_0, kernelContext_74);
    thread GpuMaterial_natural_0 _S996 = *(_S995 + (&_S974)->material_0);
    float4 _S997 = float4((&_S996)->base_1) ;
    float3 albedo_14 = _S997.xyz;
    float4 _S998 = float4((&_S996)->params_1) ;
    float metal_4 = _S998.x;
    float rough_9 = _S998.y;
    uint4 _S999 = uint4((&_S996)->tex_2) ;
    uint _S1000 = _S999.x;
    bool _S1001 = _S1000 != 4294967295U;
    if(_S1001)
    {
        textured_1 = true;
    }
    else
    {
        textured_1 = (_S999.y) != 4294967295U;
    }
    if(textured_1)
    {
        textured_1 = true;
    }
    else
    {
        textured_1 = (_S999.z) != 4294967295U;
    }
    bool leaf_5;
    float3 albedo_15;
    float3 n_27;
    float ao_13;
    float rough_10;
    float metal_5;
    float d_53;
    if(foliage_1)
    {
        TreeSurf_0 _S1002 = tree_surface_0(_S962, &_S974, &_S996, tw0_1, tw1_1, tw2_2, b0_2, b1_8, b2_11, _S963, p0_4, e1_5, e2_5, kernelContext_74);
        if(_S1002.shell_0)
        {
            float3 _S1003 = xform_normal_0(&_S974, float3(0.0f, 1.0f, 0.0f));
            float3 up_14 = normalize(_S1003);
            float clump_1 = t_noise_0(local_1.xz * float2(0.89999997615814209f)  + float2((local_1.y * 0.60000002384185791f)) , 1.0f);
            float3 albedo_16 = _S1002.albedo_8 * float3((mix(0.60000002384185791f, 1.1499999761581421f, clump_1) * 0.85000002384185791f)) ;
            float _S1004 = mix(0.64999997615814209f, 1.0f, clump_1);
            n_27 = normalize(mix(n_26, up_14, float3(0.40000000596046448f) ));
            albedo_15 = albedo_16;
            ao_13 = _S1004;
            rough_10 = 1.0f;
        }
        else
        {
            n_27 = n_26;
            albedo_15 = _S1002.albedo_8;
            ao_13 = 1.0f;
            rough_10 = 0.81999999284744263f;
        }
        if((_S981 & 131072U) != 0U)
        {
            float mot_1 = t_noise_0(local_1.xz * float2(1.7000000476837158f)  + float2((local_1.y * 3.0f)) , 1.0f);
            float3 albedo_17 = albedo_15 * float3(mix(0.81999999284744263f, 1.1200000047683716f, mot_1)) ;
            if(_S1002.leaf_3)
            {
                b1_8 = 0.75f;
            }
            else
            {
                b1_8 = 0.89999997615814209f;
            }
            albedo_15 = albedo_17;
            rough_10 = b1_8;
        }
        else
        {
            if(!_S1002.leaf_3)
            {
                textured_1 = !_S1002.shell_0;
            }
            else
            {
                textured_1 = false;
            }
            if(textured_1)
            {
                textured_1 = (length_squared_0(p0_4)) < 3600.0f;
            }
            else
            {
                textured_1 = false;
            }
            if(textured_1)
            {
                float3 pm_2 = float3(v0_1.px_7, v0_1.py_2, v0_1.pz_2) * _S993 + float3(v1_1.px_7, v1_1.py_2, v1_1.pz_2) * float3(b1_8)  + float3(v2_1.px_7, v2_1.py_2, v2_1.pz_2) * float3(b2_11) ;
                float _S1005 = pm_2.y;
                float2 q_22 = float2(atan2(pm_2.z, pm_2.x) * max(length(pm_2.xz), 0.019999999552965164f), _S1005);
                float hb_1 = bark_height_0(q_22);
                float _S1006 = t_noise_0(q_22 * float2(1.0f, 0.5f) + float2(7.0f) , 0.44999998807907104f);
                float3 _S1007 = mix(albedo_15, float3(dot(albedo_15, float3(0.30000001192092896f, 0.5f, 0.20000000298023224f)))  * float3(1.0499999523162842f, 1.0499999523162842f, 1.0f) * float3(1.3500000238418579f) , float3((smoothstep(0.55000001192092896f, 0.85000002384185791f, _S1006) * 0.44999998807907104f)) ) * float3(mix(0.5f, 1.1200000047683716f, hb_1))  * float3(mix(0.72000002861022949f, 1.0f, smoothstep(0.0f, 1.2000000476837158f, _S1005))) ;
                float3 _S1008 = xform_normal_0(&_S974, float3(0.0f, 1.0f, 0.0f));
                float3 tu_10 = cross(normalize(_S1008), n_27);
                if((length_squared_0(p0_4)) < 400.0f)
                {
                    textured_1 = (length_squared_0(tu_10)) > 9.99999997475242708e-07f;
                }
                else
                {
                    textured_1 = false;
                }
                if(textured_1)
                {
                    float hu_1 = bark_height_0(q_22 + float2(0.012000000104308128f, 0.0f));
                    float hv_1 = bark_height_0(q_22 + float2(0.0f, 0.012000000104308128f));
                    float3 tu_11 = normalize(tu_10);
                    n_27 = normalize(n_27 - float3(0.89999997615814209f)  * (float3(((hu_1 - hb_1) / 0.012000000104308128f * 0.029999999329447746f))  * tu_11 + float3(((hv_1 - hb_1) / 0.012000000104308128f * 0.029999999329447746f))  * normalize(cross(n_27, tu_11))));
                }
                float ao_14 = ao_13 * mix(0.64999997615814209f, 1.0f, hb_1);
                albedo_15 = _S1007;
                ao_13 = ao_14;
            }
        }
        if(_S1002.leaf_3)
        {
            float3 ngc_1 = cross(e1_5, e2_5);
            if((dot(ngc_1, v_16) * dot(ngc_1, n_27)) < 0.0f)
            {
                n_27 = - n_27;
            }
        }
        leaf_5 = _S1002.leaf_3;
        metal_5 = 0.0f;
    }
    else
    {
        if(textured_1)
        {
            packed_float2 device* _S1009 = ed_buf_14(kernelContext_74->ed_c_0->uvs_0, kernelContext_74);
            float2 _S1010 = float2(*(_S1009 + i0_1)) ;
            float2 d1_3 = float2(*(_S1009 + i1_1))  - _S1010;
            float2 d2_3 = float2(*(_S1009 + i2_1))  - _S1010;
            float2 _S1011 = uv_xform_0(&_S996, _S1010 + d1_3 * float2(b1_8)  + d2_3 * float2(b2_11) );
            float3 _S1012 = view_dir_0(_S962, _S963 + float2(1.0f, 0.0f));
            float2 bx_4 = ray_bary_0(_S1012, p0_4, e1_5, e2_5);
            float3 _S1013 = view_dir_0(_S962, _S963 + float2(0.0f, 1.0f));
            float2 by_4 = ray_bary_0(_S1013, p0_4, e1_5, e2_5);
            float2 _S1014 = uv_xform_0(&_S996, _S1010 + d1_3 * float2(bx_4.x)  + d2_3 * float2(bx_4.y) );
            float2 ux_2 = _S1014 - _S1011;
            float2 _S1015 = uv_xform_0(&_S996, _S1010 + d1_3 * float2(by_4.x)  + d2_3 * float2(by_4.y) );
            float2 uy_2 = _S1015 - _S1011;
            sampler _S1016 = ((reinterpret_cast<device sampler*>((kernelContext_74->ed_samplers_0))[(kernelContext_74->ed_c_0->smp_0)]));
            if(_S1001)
            {
                texture2d<float, access::sample> _S1017 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_74->ed_textures_0))[(_S1000)]));
                albedo_15 = albedo_14 * ((_S1017).sample((_S1016), (_S1011), gradient2d((ux_2), (uy_2)))).xyz;
            }
            else
            {
                albedo_15 = albedo_14;
            }
            uint _S1018 = _S999.z;
            if(_S1018 != 4294967295U)
            {
                texture2d<float, access::sample> _S1019 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_74->ed_textures_0))[(_S1018)]));
                float3 orm_1 = ((_S1019).sample((_S1016), (_S1011), gradient2d((ux_2), (uy_2)))).xyz;
                float rough_11 = rough_9 * orm_1.y;
                float metal_6 = metal_4 * orm_1.z;
                ao_13 = orm_1.x;
                rough_10 = rough_11;
                metal_5 = metal_6;
            }
            else
            {
                ao_13 = 1.0f;
                rough_10 = rough_9;
                metal_5 = metal_4;
            }
            uint _S1020 = _S999.y;
            if(_S1020 != 4294967295U)
            {
                float _S1021 = d1_3.x;
                float _S1022 = d2_3.y;
                float _S1023 = d1_3.y;
                float _S1024 = d2_3.x;
                float det_3 = _S1021 * _S1022 - _S1023 * _S1024;
                if((abs(det_3)) > 9.999999960041972e-13f)
                {
                    float3 _S1025 = float3((1.0f / det_3)) ;
                    float3 tg_3 = (e1_5 * float3(_S1022)  - e2_5 * float3(_S1023) ) * _S1025;
                    float3 bt_1 = (e2_5 * float3(_S1021)  - e1_5 * float3(_S1024) ) * _S1025;
                    float3 tg_4 = tg_3 - n_26 * float3(dot(n_26, tg_3)) ;
                    if((length_squared_0(tg_4)) > 9.99999968265522539e-21f)
                    {
                        float3 tg_5 = normalize(tg_4);
                        float3 _S1026 = cross(n_26, tg_5);
                        if((dot(_S1026, bt_1)) < 0.0f)
                        {
                            d_53 = -1.0f;
                        }
                        else
                        {
                            d_53 = 1.0f;
                        }
                        float3 bn_1 = _S1026 * float3(- d_53) ;
                        texture2d<float, access::sample> _S1027 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_74->ed_textures_0))[(_S1020)]));
                        float2 xy_1 = ((_S1027).sample((_S1016), (_S1011), gradient2d((ux_2), (uy_2)))).xy * float2(2.0f)  - float2(1.0f) ;
                        n_27 = normalize(tg_5 * float3(xy_1.x)  + bn_1 * float3(xy_1.y)  + n_26 * float3(sqrt(saturate(1.0f - dot(xy_1, xy_1)))) );
                    }
                    else
                    {
                        n_27 = n_26;
                    }
                }
                else
                {
                    n_27 = n_26;
                }
            }
            else
            {
                n_27 = n_26;
            }
        }
        else
        {
            n_27 = n_26;
            albedo_15 = albedo_14;
            ao_13 = 1.0f;
            rough_10 = rough_9;
            metal_5 = metal_4;
        }
        leaf_5 = false;
    }
    float _S1028 = dot(n_27, v_16);
    if(_S1028 < 0.0f)
    {
        n_27 = normalize(n_27 - float3((1.0099999904632568f * _S1028))  * v_16);
    }
    if((_S998.w) > 0.5f)
    {
        float2 c_28 = floor(local_1.xz * float2(0.5f) );
        albedo_15 = albedo_15 * float3(mix(0.80000001192092896f, 1.1499999761581421f, fmod(abs(c_28.x + c_28.y), 2.0f))) ;
    }
    float3 p_27 = node_6 + local_1;
    float zlin_6 = - (((float4(p_27, 1.0f)) * (matrix<float,int(4),int(4)> (_S962->view_0.data_0[int(0)][int(0)], _S962->view_0.data_0[int(1)][int(0)], _S962->view_0.data_0[int(2)][int(0)], _S962->view_0.data_0[int(3)][int(0)], _S962->view_0.data_0[int(0)][int(1)], _S962->view_0.data_0[int(1)][int(1)], _S962->view_0.data_0[int(2)][int(1)], _S962->view_0.data_0[int(3)][int(1)], _S962->view_0.data_0[int(0)][int(2)], _S962->view_0.data_0[int(1)][int(2)], _S962->view_0.data_0[int(2)][int(2)], _S962->view_0.data_0[int(3)][int(2)], _S962->view_0.data_0[int(0)][int(3)], _S962->view_0.data_0[int(1)][int(3)], _S962->view_0.data_0[int(2)][int(3)], _S962->view_0.data_0[int(3)][int(3)])))).z;
    float3 ng_9 = cross(e1_5, e2_5);
    float3 ng_10;
    if((length_squared_0(ng_9)) > 9.99999968265522539e-21f)
    {
        ng_10 = normalize(ng_9);
    }
    else
    {
        ng_10 = n_27;
    }
    if((dot(ng_10, v_16)) < 0.0f)
    {
        ng_10 = - ng_10;
    }
    float _S1029 = sun_shadow_0(_S962, kernelContext_74->ed_c_0->shadow_0, p_27, ng_10, zlin_6, _S963, kernelContext_74);
    float2 _S1030 = ao_upsample_0(_S962, kernelContext_74->ed_c_0->ao_0, _S963, zlin_6, kernelContext_74);
    float ao_15 = ao_13 * _S1030.x;
    float sv_8 = _S1030.y;
    float3 _S1031 = sun_light_at_0(_S962, p_27, kernelContext_74);
    float3 col_8;
    if(foliage_1)
    {
        float3 _S1032 = shade_foliage_0(_S962, _S1031, albedo_15, rough_10, ao_15, sv_8, _S1029, n_27, ng_10, v_16, leaf_5, kernelContext_74);
        col_8 = _S1032;
    }
    else
    {
        if((_S997.w) > 0.5f)
        {
            float3 _S1033 = shade_toon_0(_S962, _S1031, &_S996, albedo_15, n_27, v_16, ao_15, sv_8, _S1029, kernelContext_74);
            l0_1 = _S1033;
        }
        else
        {
            float3 _S1034 = shade_pbr_0(_S962, _S1031, albedo_15, metal_5, rough_10, ao_15, sv_8, _S1029, n_27, v_16, kernelContext_74);
            l0_1 = _S1034;
        }
        col_8 = l0_1;
    }
    float3 _S1035 = apply_aerial_0(_S962, col_8, p_27, _S963, kernelContext_74);
    if(((uint4((&_S962->light_0)->flags_0) ).w) == 1U)
    {
        uint _S1036 = shadow_cascade_0(_S962, p_27, zlin_6);
        col_8 = TINT_3[_S1036] * float3((0.039999999105930328f + 0.5f * _S1029)) ;
    }
    else
    {
        if(((uint4((&_S962->light_0)->flags_0) ).w) == 3U)
        {
            uint _S1037 = shadow_cascade_0(_S962, p_27, zlin_6);
            if(_S1037 < 4U)
            {
                float _S1038 = cascade_depth_delta_0(_S962, kernelContext_74->ed_c_0->shadow_0, _S1037, p_27, kernelContext_74);
                d_53 = _S1038;
            }
            else
            {
                d_53 = 0.0f;
            }
            col_8 = float3(max(d_53, 0.0f), max(- d_53, 0.0f), 0.0f) * float3(0.10000000149011612f) ;
        }
        else
        {
            if(((uint4((&_S962->light_0)->flags_0) ).w) == 2U)
            {
                col_8 = float3((0.5f * ao_15)) ;
            }
            else
            {
                if(((uint4((&_S962->light_0)->flags_0) ).w) == 4U)
                {
                    col_8 = float3((0.5f * sv_8)) ;
                }
                else
                {
                    col_8 = _S1035;
                }
            }
        }
    }
    return float4(col_8, 1.0f);
}

FsOut_0 fullscreen_vertex_0(uint vid_0)
{
    thread FsOut_0 o_4;
    (&o_4)->pos_0 = float4(float2(float((vid_0 << 1U) & 2U), float(vid_0 & 2U)) * float2(2.0f)  - float2(1.0f) , 0.0f, 1.0f);
    return o_4;
}

struct pixelOutput_0
{
    float4 output_0 [[color(0)]];
};

[[fragment]] pixelOutput_0 fs_resolve(float4 pos_1 [[position]], ulong device* ed_buffers_1 [[buffer(2)]], ResolveC_0 constant* ed_c_1 [[buffer(0)]], ulong device* ed_samplers_1 [[buffer(3)]], ulong device* ed_textures_1 [[buffer(1)]])
{
    thread KernelContext_0 kernelContext_75;
    (&kernelContext_75)->ed_buffers_0 = ed_buffers_1;
    (&kernelContext_75)->ed_c_0 = ed_c_1;
    (&kernelContext_75)->ed_samplers_0 = ed_samplers_1;
    (&kernelContext_75)->ed_textures_0 = ed_textures_1;
    thread FsOut_0 _S1039;
    (&_S1039)->pos_0 = pos_1;
    float4 _S1040 = resolve_px_1(&_S1039, false, &kernelContext_75);
    pixelOutput_0 _S1041 = { _S1040 };
    return _S1041;
}

struct pixelOutput_1
{
    float4 output_1 [[color(0)]];
};

[[fragment]] pixelOutput_1 fs_resolve_terrain(float4 pos_2 [[position]], ulong device* ed_buffers_2 [[buffer(2)]], ResolveC_0 constant* ed_c_2 [[buffer(0)]], ulong device* ed_samplers_2 [[buffer(3)]], ulong device* ed_textures_2 [[buffer(1)]])
{
    thread KernelContext_0 kernelContext_76;
    (&kernelContext_76)->ed_buffers_0 = ed_buffers_2;
    (&kernelContext_76)->ed_c_0 = ed_c_2;
    (&kernelContext_76)->ed_samplers_0 = ed_samplers_2;
    (&kernelContext_76)->ed_textures_0 = ed_textures_2;
    thread FsOut_0 _S1042;
    (&_S1042)->pos_0 = pos_2;
    float4 _S1043 = resolve_px_1(&_S1042, true, &kernelContext_76);
    pixelOutput_1 _S1044 = { _S1043 };
    return _S1044;
}

struct pixelOutput_2
{
    float4 output_2 [[color(0)]];
};

[[fragment]] pixelOutput_2 fs_resolve_trees(float4 pos_3 [[position]], ulong device* ed_buffers_3 [[buffer(2)]], ResolveC_0 constant* ed_c_3 [[buffer(0)]], ulong device* ed_samplers_3 [[buffer(3)]], ulong device* ed_textures_3 [[buffer(1)]])
{
    thread KernelContext_0 kernelContext_77;
    (&kernelContext_77)->ed_buffers_0 = ed_buffers_3;
    (&kernelContext_77)->ed_c_0 = ed_c_3;
    (&kernelContext_77)->ed_samplers_0 = ed_samplers_3;
    (&kernelContext_77)->ed_textures_0 = ed_textures_3;
    thread FsOut_0 _S1045;
    (&_S1045)->pos_0 = pos_3;
    float4 _S1046 = resolve_px_0(&_S1045, true, &kernelContext_77);
    pixelOutput_2 _S1047 = { _S1046 };
    return _S1047;
}

struct vs_resolve_Result_0
{
    float4 pos_4 [[position]];
};

[[vertex]] vs_resolve_Result_0 vs_resolve(uint vid_1 [[vertex_id]], ulong device* ed_buffers_4 [[buffer(2)]], ResolveC_0 constant* ed_c_4 [[buffer(0)]], ulong device* ed_samplers_4 [[buffer(3)]], ulong device* ed_textures_4 [[buffer(1)]])
{
    thread KernelContext_0 kernelContext_78;
    (&kernelContext_78)->ed_buffers_0 = ed_buffers_4;
    (&kernelContext_78)->ed_c_0 = ed_c_4;
    (&kernelContext_78)->ed_samplers_0 = ed_samplers_4;
    (&kernelContext_78)->ed_textures_0 = ed_textures_4;
    thread vs_resolve_Result_0 _S1048;
    (&_S1048)->pos_4 = fullscreen_vertex_0(vid_1).pos_0;
    return _S1048;
}

