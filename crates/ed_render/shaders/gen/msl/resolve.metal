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
    packed_uint4 anc_0;
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
    ulong device* ed_textures_0;
    ulong device* ed_samplers_0;
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

uint t_canopy_raw_0(uint pool_2, uint off_1, uint i_5, uint j_3, KernelContext_0 thread* kernelContext_5)
{
    uint device* _S19 = ed_buf_0(pool_2, kernelContext_5);
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

float t_canopy_lift_0(uint pool_3, uint off_2, uint i_6, uint j_4, KernelContext_0 thread* kernelContext_6)
{
    if(off_2 == 0U)
    {
        return 0.0f;
    }
    uint _S21 = t_canopy_raw_0(pool_3, off_2, i_6, j_4, kernelContext_6);
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

float3 t_coarse_even_0(uint pool_7, const TerrainDraw_natural_0 thread* d_6, uint e_1, uint2 a_0, uint2 b_0, uint2 c_1, KernelContext_0 thread* kernelContext_10)
{
    float3 _S39 = t_base_0(pool_7, d_6, a_0.x, a_0.y, kernelContext_10);
    if((e_1 & 3U) == 0U)
    {
        return _S39;
    }
    float m_0 = t_morph_0((float4(d_6->origin_0) ).xyz + _S39, (float4(d_6->morph_0) ).zw);
    float3 _S40 = t_base_0(pool_7, d_6, b_0.x, b_0.y, kernelContext_10);
    float3 _S41 = t_base_0(pool_7, d_6, c_1.x, c_1.y, kernelContext_10);
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
    uint c_2 = 64U >> lod_1;
    uint _S75 = 2U * c_2 * c_2;
    uint3 _S76;
    if(_S74 < _S75)
    {
        uint q_1 = _S74 >> 1U;
        uint _S77 = q_1 % c_2;
        uint i_13 = _S77 * st_0;
        uint _S78 = q_1 / c_2;
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
    uint s1_0 = (s_3 + 1U) % (4U * c_2);
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

float3 t_base_1(uint pool_12, const TerrainDraw_natural_0 thread* d_11, uint i_14, uint j_10, KernelContext_0 thread* kernelContext_16)
{
    float _S81 = t_own_h_0(pool_12, d_11, i_14, j_10, kernelContext_16);
    thread float3 dir_3;
    float3 _S82 = t_lattice_0(d_11, i_14, j_10, _S81, &dir_3);
    return _S82;
}

float3 t_coarse_even_1(uint pool_13, const TerrainDraw_natural_0 thread* d_12, uint e_6, uint2 a_3, uint2 b_3, uint2 c_3, KernelContext_0 thread* kernelContext_17)
{
    float3 _S83 = t_base_1(pool_13, d_12, a_3.x, a_3.y, kernelContext_17);
    if((e_6 & 3U) == 0U)
    {
        return _S83;
    }
    float m_2 = t_morph_0((float4(d_12->origin_0) ).xyz + _S83, (float4(d_12->morph_0) ).zw);
    float3 _S84 = t_base_1(pool_13, d_12, b_3.x, b_3.y, kernelContext_17);
    float3 _S85 = t_base_1(pool_13, d_12, c_3.x, c_3.y, kernelContext_17);
    return mix(_S83, float3(0.5f)  * (_S84 + _S85), float3(m_2) );
}

float3 t_coarse_1(uint pool_14, const TerrainDraw_natural_0 thread* d_13, uint edge_3, uint e_7, KernelContext_0 thread* kernelContext_18)
{
    if((e_7 & 1U) == 0U)
    {
        float3 _S86 = t_coarse_even_1(pool_14, d_13, e_7, t_edge_at_0(edge_3, e_7), t_edge_at_0(edge_3, e_7 - 2U), t_edge_at_0(edge_3, e_7 + 2U), kernelContext_18);
        return _S86;
    }
    uint _S87 = e_7 - 1U;
    uint2 _S88 = t_edge_at_0(edge_3, _S87);
    uint _S89 = e_7 + 1U;
    uint2 _S90 = t_edge_at_0(edge_3, _S89);
    float3 _S91 = t_coarse_even_1(pool_14, d_13, _S87, _S88, t_edge_at_0(edge_3, e_7 - 3U), _S90, kernelContext_18);
    float3 _S92 = t_coarse_even_1(pool_14, d_13, _S89, _S90, _S88, t_edge_at_0(edge_3, e_7 + 3U), kernelContext_18);
    return float3(0.5f)  * (_S91 + _S92);
}

float3 t_target_even_1(uint pool_15, const TerrainDraw_natural_0 thread* d_14, uint i_15, uint j_11, KernelContext_0 thread* kernelContext_19)
{
    bool border_1;
    if(i_15 == 0U)
    {
        border_1 = true;
    }
    else
    {
        border_1 = j_11 == 0U;
    }
    if(border_1)
    {
        border_1 = true;
    }
    else
    {
        border_1 = i_15 == 64U;
    }
    if(border_1)
    {
        border_1 = true;
    }
    else
    {
        border_1 = j_11 == 64U;
    }
    bool own_1;
    if(border_1)
    {
        own_1 = true;
    }
    else
    {
        own_1 = ((uint4(d_14->slots_0) ).y) == 4294967295U;
    }
    uint4 _S93 = uint4(d_14->slots_0) ;
    uint _S94 = _S93.z;
    uint qx_1;
    if((_S94 & 16U) != 0U)
    {
        qx_1 = 128U;
    }
    else
    {
        qx_1 = 0U;
    }
    uint qy_1;
    if((_S94 & 32U) != 0U)
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
        float _S95 = t_own_h_0(pool_15, d_14, i_15, j_11, kernelContext_19);
        h_2 = _S95;
    }
    else
    {
        float4 _S96 = float4(d_14->hq_0) ;
        float _S97 = t_page_0(pool_15, _S93.y, qx_1 + 2U * i_15, qy_1 + 2U * j_11, _S96.z, _S96.w, kernelContext_19);
        h_2 = _S97;
    }
    thread float3 dir_4;
    float3 _S98 = t_lattice_0(d_14, i_15, j_11, h_2, &dir_4);
    return _S98;
}

float3 t_target_1(uint pool_16, const TerrainDraw_natural_0 thread* d_15, uint i_16, uint j_12, KernelContext_0 thread* kernelContext_20)
{
    bool ei_1 = (i_16 & 1U) == 0U;
    bool ej_1 = (j_12 & 1U) == 0U;
    bool _S99;
    if(ei_1)
    {
        _S99 = ej_1;
    }
    else
    {
        _S99 = false;
    }
    if(_S99)
    {
        float3 _S100 = t_target_even_1(pool_16, d_15, i_16, j_12, kernelContext_20);
        return _S100;
    }
    if(!ei_1)
    {
        _S99 = ej_1;
    }
    else
    {
        _S99 = false;
    }
    if(_S99)
    {
        float3 _S101 = t_target_even_1(pool_16, d_15, i_16 - 1U, j_12, kernelContext_20);
        float3 _S102 = t_target_even_1(pool_16, d_15, i_16 + 1U, j_12, kernelContext_20);
        return float3(0.5f)  * (_S101 + _S102);
    }
    if(ei_1)
    {
        _S99 = !ej_1;
    }
    else
    {
        _S99 = false;
    }
    if(_S99)
    {
        float3 _S103 = t_target_even_1(pool_16, d_15, i_16, j_12 - 1U, kernelContext_20);
        float3 _S104 = t_target_even_1(pool_16, d_15, i_16, j_12 + 1U, kernelContext_20);
        return float3(0.5f)  * (_S103 + _S104);
    }
    float3 _S105 = t_target_even_1(pool_16, d_15, i_16 - 1U, j_12 - 1U, kernelContext_20);
    float3 _S106 = t_target_even_1(pool_16, d_15, i_16 + 1U, j_12 + 1U, kernelContext_20);
    return float3(0.5f)  * (_S105 + _S106);
}

float3 t_grid_vertex_1(uint pool_17, const TerrainDraw_natural_0 thread* d_16, uint i_17, uint j_13, KernelContext_0 thread* kernelContext_21)
{
    uint fl_1 = (uint4(d_16->slots_0) ).z;
    uint edge_4;
    uint e_8;
    if(i_17 == 0U)
    {
        edge_4 = 0U;
        e_8 = j_13;
    }
    else
    {
        if(i_17 == 64U)
        {
            edge_4 = 1U;
            e_8 = j_13;
        }
        else
        {
            if(j_13 == 0U)
            {
                edge_4 = 2U;
                e_8 = i_17;
            }
            else
            {
                if(j_13 == 64U)
                {
                    edge_4 = 3U;
                    e_8 = i_17;
                }
                else
                {
                    edge_4 = 4U;
                    e_8 = 0U;
                }
            }
        }
    }
    if(edge_4 < 4U)
    {
        uint E_1 = 1U << ((fl_1 >> (10U + 3U * edge_4)) & 7U);
        bool _S107;
        if((fl_1 & (1U << edge_4)) != 0U)
        {
            _S107 = E_1 <= 2U;
        }
        else
        {
            _S107 = false;
        }
        if(_S107)
        {
            float3 _S108 = t_coarse_1(pool_17, d_16, edge_4, e_8, kernelContext_21);
            return _S108;
        }
        if(E_1 > 1U)
        {
            uint lo_1 = e_8 & (~(E_1 - 1U));
            if(lo_1 == e_8)
            {
                float3 _S109 = t_base_1(pool_17, d_16, i_17, j_13, kernelContext_21);
                return _S109;
            }
            uint2 a_4 = t_edge_at_0(edge_4, lo_1);
            uint2 b_4 = t_edge_at_0(edge_4, lo_1 + E_1);
            float3 _S110 = t_base_1(pool_17, d_16, a_4.x, a_4.y, kernelContext_21);
            float3 _S111 = t_base_1(pool_17, d_16, b_4.x, b_4.y, kernelContext_21);
            return mix(_S110, _S111, float3((float(e_8 - lo_1) / float(E_1))) );
        }
    }
    float3 _S112 = t_base_1(pool_17, d_16, i_17, j_13, kernelContext_21);
    if((fl_1 & 4194304U) != 0U)
    {
        return _S112;
    }
    float m_3 = t_morph_0((float4(d_16->origin_0) ).xyz + _S112, (float4(d_16->morph_0) ).xy);
    if(m_3 <= 0.0f)
    {
        return _S112;
    }
    float3 _S113 = t_target_1(pool_17, d_16, i_17, j_13, kernelContext_21);
    return mix(_S112, _S113, float3(m_3) );
}

float3 t_vertex_1(uint pool_18, const TerrainDraw_natural_0 thread* d_17, uint v_1, KernelContext_0 thread* kernelContext_22)
{
    if(v_1 < 4225U)
    {
        float3 _S114 = (float4(d_17->origin_0) ).xyz;
        float3 _S115 = t_grid_vertex_1(pool_18, d_17, v_1 % 65U, v_1 / 65U, kernelContext_22);
        return _S114 + _S115;
    }
    uint2 ij_1 = t_ring_0(v_1 - 4225U);
    float3 _S116 = t_grid_vertex_1(pool_18, d_17, ij_1.x, ij_1.y, kernelContext_22);
    float depth_1;
    if((((uint4(d_17->slots_0) ).z) & 64U) != 0U)
    {
        depth_1 = (float4(d_17->origin_0) ).w;
    }
    else
    {
        depth_1 = 0.0f;
    }
    float3 _S117 = (float4(d_17->origin_0) ).xyz;
    float3 _S118 = _S117 + _S116;
    float4 _S119 = float4(d_17->dir_c_0) ;
    return _S118 - normalize(_S118 - (_S117 - _S119.xyz * float3(_S119.w) )) * float3(depth_1) ;
}

float2 ray_bary_0(float3 dir_5, float3 p0_0, float3 e1_0, float3 e2_0)
{
    float3 pv_0 = cross(dir_5, e2_0);
    float det_0 = dot(e1_0, pv_0);
    if((abs(det_0)) <= 9.999999960041972e-13f)
    {
        return float2(0.3333333432674408f) ;
    }
    float inv_0 = 1.0f / det_0;
    float3 tv_1 = - p0_0;
    return float2(dot(tv_1, pv_0) * inv_0, dot(dir_5, cross(tv_1, e1_0)) * inv_0);
}

float2 t_vertex_grid_0(uint v_2)
{
    if(v_2 < 4225U)
    {
        return float2(float(v_2 % 65U), float(v_2 / 65U));
    }
    return float2(t_ring_0(v_2 - 4225U));
}

float t_page_at_0(uint pool_19, const TerrainDraw_natural_0 thread* d_18, int a_5, int b_5, KernelContext_0 thread* kernelContext_23)
{
    float4 _S120 = float4(d_18->hq_0) ;
    float _S121 = t_page_0(pool_19, (uint4(d_18->slots_0) ).x, uint(clamp(a_5, int(0), int(256))), uint(clamp(b_5, int(0), int(256))), _S120.x, _S120.y, kernelContext_23);
    return _S121;
}

void t_surface_0(uint pool_20, const TerrainDraw_natural_0 thread* d_19, float2 g_0, int r_1, float thread* h_3, float2 thread* dh_0, KernelContext_0 thread* kernelContext_24)
{
    float2 _S122 = float2(4.0f) ;
    float2 _S123 = float2(0.0f) ;
    float2 s_4 = clamp(g_0 * _S122, _S123, float2(256.0f) );
    int2 _S124 = int2(min(floor(s_4), float2(255.0f) ));
    float2 _S125 = s_4 - float2(_S124);
    *h_3 = 0.0f;
    *dh_0 = _S123;
    uint k_2 = 0U;
    for(;;)
    {
        if(k_2 < 4U)
        {
        }
        else
        {
            break;
        }
        uint _S126 = k_2 & 1U;
        uint _S127 = k_2 >> 1U;
        int2 q_3 = _S124 + int2(int(_S126), int(_S127));
        float _S128;
        if(_S126 != 0U)
        {
            _S128 = _S125.x;
        }
        else
        {
            _S128 = 1.0f - _S125.x;
        }
        float _S129;
        if(_S127 != 0U)
        {
            _S129 = _S125.y;
        }
        else
        {
            _S129 = 1.0f - _S125.y;
        }
        float w_1 = _S128 * _S129;
        int2 _S130 = int2(r_1) ;
        int2 _S131 = max(q_3 - _S130, int2(int(0)) );
        int2 _S132 = min(q_3 + _S130, int2(int(256)) );
        int _S133 = _S132.x;
        int _S134 = q_3.y;
        float _S135 = t_page_at_0(pool_20, d_19, _S133, _S134, kernelContext_24);
        int _S136 = _S131.x;
        float _S137 = t_page_at_0(pool_20, d_19, _S136, _S134, kernelContext_24);
        float gx_0 = (_S135 - _S137) / float(_S133 - _S136);
        int _S138 = q_3.x;
        int _S139 = _S132.y;
        float _S140 = t_page_at_0(pool_20, d_19, _S138, _S139, kernelContext_24);
        int _S141 = _S131.y;
        float _S142 = t_page_at_0(pool_20, d_19, _S138, _S141, kernelContext_24);
        float gy_0 = (_S140 - _S142) / float(_S139 - _S141);
        float _S143 = t_page_at_0(pool_20, d_19, _S138, _S134, kernelContext_24);
        *h_3 = *h_3 + w_1 * _S143;
        *dh_0 = *dh_0 + float2(w_1)  * float2(gx_0, gy_0);
        k_2 = k_2 + 1U;
    }
    *dh_0 = *dh_0 * _S122;
    return;
}

float3 t_lattice_f_0(const TerrainDraw_natural_0 thread* d_20, float2 g_1, float h_4, float3 thread* dir_6)
{
    float4 _S144 = float4(d_20->shape_0) ;
    float _S145 = _S144.x;
    float tu_1 = tan((g_1.x - 32.0f) * _S145);
    float tv_2 = tan((g_1.y - 32.0f) * _S145);
    float4 _S146 = float4(d_20->fu_0) ;
    float _S147 = _S146.w;
    float4 _S148 = float4(d_20->fv_0) ;
    float _S149 = _S148.w;
    float3 delta_1 = float3((tu_1 * (1.0f + _S147 * _S147) / (1.0f - _S147 * tu_1)))  * _S146.xyz + float3((tv_2 * (1.0f + _S149 * _S149) / (1.0f - _S149 * tv_2)))  * _S148.xyz;
    float4 _S150 = float4(d_20->cc_0) ;
    float lc_1 = _S150.w;
    float3 _S151 = _S150.xyz;
    float k_3 = 2.0f * dot(_S151, delta_1) + dot(delta_1, delta_1);
    float l_1 = sqrt(lc_1 * lc_1 + k_3);
    float3 dd_1 = delta_1 / float3(l_1)  - _S151 * float3((k_3 / ((lc_1 + l_1) * l_1 * lc_1))) ;
    float4 _S152 = float4(d_20->dir_c_0) ;
    float3 _S153 = _S152.xyz;
    float3 _S154 = _S153 + dd_1;
    *dir_6 = _S154;
    float3 _S155 = (float4(d_20->polar_0) ).xyz;
    float dzc_1 = dot(_S153, _S155);
    float ddz_1 = dot(dd_1, _S155);
    float dz_1 = dzc_1 + ddz_1;
    float e_9 = _S144.z;
    float s_5 = sqrt(1.0f - e_9 * dz_1 * dz_1);
    float sc_1 = _S144.w;
    float dr_1 = _S144.y * e_9 * ddz_1 * (dz_1 + dzc_1) / (s_5 * sc_1 * (s_5 + sc_1));
    return dd_1 * float3((_S152.w + dr_1))  + _S153 * float3(dr_1)  + _S154 * float3(h_4) ;
}

float3 t_lattice_f_1(const TerrainDraw_natural_0 thread* d_21, float2 g_2, float h_5, float3 thread* dir_7)
{
    float4 _S156 = float4(d_21->shape_0) ;
    float _S157 = _S156.x;
    float tu_2 = tan((g_2.x - 32.0f) * _S157);
    float tv_3 = tan((g_2.y - 32.0f) * _S157);
    float4 _S158 = float4(d_21->fu_0) ;
    float _S159 = _S158.w;
    float4 _S160 = float4(d_21->fv_0) ;
    float _S161 = _S160.w;
    float3 delta_2 = float3((tu_2 * (1.0f + _S159 * _S159) / (1.0f - _S159 * tu_2)))  * _S158.xyz + float3((tv_3 * (1.0f + _S161 * _S161) / (1.0f - _S161 * tv_3)))  * _S160.xyz;
    float4 _S162 = float4(d_21->cc_0) ;
    float lc_2 = _S162.w;
    float3 _S163 = _S162.xyz;
    float k_4 = 2.0f * dot(_S163, delta_2) + dot(delta_2, delta_2);
    float l_2 = sqrt(lc_2 * lc_2 + k_4);
    float3 dd_2 = delta_2 / float3(l_2)  - _S163 * float3((k_4 / ((lc_2 + l_2) * l_2 * lc_2))) ;
    float4 _S164 = float4(d_21->dir_c_0) ;
    float3 _S165 = _S164.xyz;
    float3 _S166 = _S165 + dd_2;
    *dir_7 = _S166;
    float3 _S167 = (float4(d_21->polar_0) ).xyz;
    float dzc_2 = dot(_S165, _S167);
    float ddz_2 = dot(dd_2, _S167);
    float dz_2 = dzc_2 + ddz_2;
    float e_10 = _S156.z;
    float s_6 = sqrt(1.0f - e_10 * dz_2 * dz_2);
    float sc_2 = _S156.w;
    float dr_2 = _S156.y * e_10 * ddz_2 * (dz_2 + dzc_2) / (s_6 * sc_2 * (s_6 + sc_2));
    return dd_2 * float3((_S164.w + dr_2))  + _S165 * float3(dr_2)  + _S166 * float3(h_5) ;
}

float3 t_smooth_normal_0(uint pool_21, const TerrainDraw_natural_0 thread* d_22, float2 g_3, int r_2, float3 thread* up_1, float thread* h_6, KernelContext_0 thread* kernelContext_25)
{
    thread float2 dh_1;
    t_surface_0(pool_21, d_22, g_3, r_2, h_6, &dh_1, kernelContext_25);
    float2 _S168 = float2(0.25f, 0.0f);
    thread float3 dir_8;
    float3 _S169 = t_lattice_f_0(d_22, g_3 + _S168, *h_6, &dir_8);
    float3 _S170 = t_lattice_f_0(d_22, g_3 - _S168, *h_6, &dir_8);
    float3 pu_0 = _S169 - _S170;
    float2 _S171 = float2(0.0f, 0.25f);
    float3 _S172 = t_lattice_f_0(d_22, g_3 + _S171, *h_6, &dir_8);
    float3 _S173 = t_lattice_f_0(d_22, g_3 - _S171, *h_6, &dir_8);
    float3 pv_1 = _S172 - _S173;
    float3 _S174 = t_lattice_f_1(d_22, g_3, *h_6, up_1);
    float3 _S175 = normalize(*up_1);
    *up_1 = _S175;
    float3 _S176 = float3(0.5f) ;
    float3 n_0 = normalize(cross(pu_0 / _S176 + _S175 * float3(dh_1.x) , pv_1 / _S176 + _S175 * float3(dh_1.y) ));
    float3 _S177;
    if((dot(n_0, _S175)) < 0.0f)
    {
        _S177 = - n_0;
    }
    else
    {
        _S177 = n_0;
    }
    return _S177;
}

float2 shell_wrel_0(FrameData_natural_0 device* f_0, const TerrainDraw_natural_0 thread* d_23, float2 g_4)
{
    float2 c_4 = (float4(d_23->tex_0) ).xy - (float4((&f_0->shell_0)->c_0) ).xy;
    float2 _S178 = float2(4096.0f) ;
    float4 _S179 = float4(d_23->shape_0) ;
    return c_4 - _S178 * round(c_4 / _S178) + (g_4 - float2(32.0f) ) * float2((_S179.x * _S179.y)) ;
}

struct ShellHit_0
{
    bool hit_0;
    float tt_0;
    float rnd_0;
    uint kind_0;
    float dry_0;
    float2 side_0;
};

uint grass_hash_0(uint x_0)
{
    uint s_7 = x_0 * 747796405U + 2891336453U;
    uint w_2 = ((s_7 >> ((s_7 >> 28U) + 4U)) ^ s_7) * 277803737U;
    return (w_2 >> 22U) ^ w_2;
}

uint shell_hash2_0(int2 q_4, uint salt_0)
{
    return grass_hash_0((uint(q_4.x) * 2654435761U) ^ (grass_hash_0(uint(q_4.y) ^ salt_0)));
}

float grass_rand_0(uint h_7)
{
    return float(h_7 >> 8U) * 5.9604644775390625e-08f;
}

uint shell_map_0(FrameData_natural_0 device* f_1, uint buf_0, uint coarse_0, int2 cell_0, KernelContext_0 thread* kernelContext_26)
{
    int n_1 = int((float4((&f_1->shell_0)->cells_0) ).z);
    bool _S180;
    if(any(cell_0 < (int2((- n_1 / int(2))) )))
    {
        _S180 = true;
    }
    else
    {
        _S180 = any(cell_0 >= (int2((n_1 / int(2))) ));
    }
    if(_S180)
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
    uint2 _S181 = uint2((cell_0 + off_3) & (int2((n_1 - int(1))) ));
    uint base_0 = (uint4((&f_1->shell_0)->cfg_5) ).z + coarse_0 * uint(n_1 * n_1);
    uint device* _S182 = ed_buf_0(buf_0, kernelContext_26);
    return *(_S182 + (base_0 + _S181.y * uint(n_1) + _S181.x));
}

struct GrassType_0
{
    float4 a_6;
    float4 b_6;
    float4 root_0;
    float4 mid_0;
    float4 tip_0;
    float4 dry_1;
    float4 shell_1;
    float4 carpet_0;
};

struct GrassType_natural_0
{
    packed_float4 a_6;
    packed_float4 b_6;
    packed_float4 root_0;
    packed_float4 mid_0;
    packed_float4 tip_0;
    packed_float4 dry_1;
    packed_float4 shell_1;
    packed_float4 carpet_0;
};

GrassType_natural_0 device* ed_buf_3(uint i_18, KernelContext_0 thread* kernelContext_27)
{
    return (GrassType_natural_0 device*)kernelContext_27->ed_buffers_0[i_18];
}

GrassType_0 shell_type_0(FrameData_natural_0 device* f_2, uint buf_1, uint kind_1, KernelContext_0 thread* kernelContext_28)
{
    GrassType_natural_0 device* _S183 = ed_buf_3(buf_1, kernelContext_28);
    GrassType_natural_0 device* _S184 = _S183 + ((uint4((&f_2->shell_0)->cfg_5) ).y + kind_1);
    GrassType_0 _S185 = { float4((*_S184).a_6) , float4((*_S184).b_6) , float4((*_S184).root_0) , float4((*_S184).mid_0) , float4((*_S184).tip_0) , float4((*_S184).dry_1) , float4((*_S184).shell_1) , float4((*_S184).carpet_0)  };
    return _S185;
}

ShellHit_0 shell_strand_0(FrameData_natural_0 device* f_3, uint buf_2, float2 wrel_0, uint k_5, uint n_2, uint L_0, float dz_3, bool force_0, KernelContext_0 thread* kernelContext_29)
{
    thread ShellHit_0 o_0;
    (&o_0)->hit_0 = false;
    (&o_0)->tt_0 = 0.0f;
    (&o_0)->rnd_0 = 0.0f;
    (&o_0)->kind_0 = 0U;
    (&o_0)->dry_0 = 0.0f;
    (&o_0)->side_0 = float2(0.0f) ;
    uint jh_0 = shell_hash2_0(int2(floor(wrel_0 * float2(8.0f) )), 81U);
    float2 _S186 = float2(0.5f) ;
    float2 _S187 = float2((float4((&f_3->shell_0)->cells_0) ).x) ;
    uint _S188 = shell_map_0(f_3, buf_2, 0U, int2(floor((wrel_0 + (float2(grass_rand_0(jh_0), grass_rand_0(grass_hash_0(jh_0 + 7U))) - _S186) * _S187) / _S187)), kernelContext_29);
    uint _S189 = _S188 & 255U;
    if(_S189 == 0U)
    {
        return o_0;
    }
    uint kind_2 = _S189 - 1U;
    float dens_0 = float((_S188 >> 8U) & 255U) / 255.0f;
    float hsc_0 = float((_S188 >> 16U) & 255U) / 255.0f * 2.0f;
    float dry_2 = float((_S188 >> 24U) & 255U) / 255.0f;
    GrassType_0 _S190 = shell_type_0(f_3, buf_2, kind_2, kernelContext_29);
    float hk_0 = (float4((&f_3->shell_0)->fu_1) ).w * float(k_5 + 1U) / float(n_2);
    float H_0 = _S190.shell_1.x * hsc_0;
    (&o_0)->kind_0 = kind_2;
    float2 _S191 = float2((_S190.shell_1.y * float(1U << L_0))) ;
    float2 q_5 = wrel_0 / _S191;
    float2 lq_0 = fract(q_5);
    uint h_8 = shell_hash2_0(int2(floor(q_5)) + int2(round((float4((&f_3->shell_0)->c_0) ).xy / _S191)), 163U + L_0);
    float r0_0 = grass_rand_0(h_8);
    float r1_0 = grass_rand_0(grass_hash_0(h_8 + 1U));
    float r2_0 = grass_rand_0(grass_hash_0(h_8 + 2U));
    float r3_0 = grass_rand_0(grass_hash_0(h_8 + 3U));
    (&o_0)->rnd_0 = r1_0;
    float _S192;
    if(r3_0 < dry_2)
    {
        _S192 = 1.0f;
    }
    else
    {
        _S192 = 0.0f;
    }
    (&o_0)->dry_0 = _S192;
    float fade_0 = smoothstep((float4((&f_3->shell_0)->c_0) ).z * 0.72000002861022949f, (float4((&f_3->shell_0)->c_0) ).z, dz_3);
    bool _S193 = !force_0;
    bool _S194;
    if(_S193)
    {
        _S194 = (grass_rand_0(grass_hash_0(h_8 + 5U))) < fade_0;
    }
    else
    {
        _S194 = false;
    }
    if(_S194)
    {
        return o_0;
    }
    if(k_5 == 0U)
    {
        (&o_0)->tt_0 = 0.079999998211860657f;
        (&o_0)->side_0 = lq_0 - _S186;
        if(force_0)
        {
            _S194 = true;
        }
        else
        {
            _S194 = r2_0 < (_S190.shell_1.z * dens_0 * 1.1499999761581421f + 0.05000000074505806f);
        }
        (&o_0)->hit_0 = _S194;
        return o_0;
    }
    if(_S193)
    {
        _S194 = r0_0 >= (_S190.shell_1.z * dens_0);
    }
    else
    {
        _S194 = false;
    }
    if(_S194)
    {
        return o_0;
    }
    float tt_1 = hk_0 / max(H_0 * mix(0.5f, 1.0f, r1_0), 9.9999997473787516e-05f);
    if(_S193)
    {
        _S194 = tt_1 > 1.0f;
    }
    else
    {
        _S194 = false;
    }
    if(_S194)
    {
        return o_0;
    }
    float tt_2 = saturate(tt_1);
    float a_7 = r2_0 * 6.2831854820251465f;
    float rad_0 = 0.41999998688697815f * (1.0f - pow(tt_2, 1.6000000238418579f)) + 0.039999999105930328f;
    float2 dl_0 = lq_0 - (float2(0.5f)  + (float2(grass_rand_0(grass_hash_0(h_8 + 9U)), grass_rand_0(grass_hash_0(h_8 + 11U))) - _S186) * _S186 + float2(cos(a_7), sin(a_7)) * float2((0.20000000298023224f * tt_2 * tt_2)) );
    (&o_0)->side_0 = dl_0 / float2(rad_0) ;
    (&o_0)->tt_0 = tt_2;
    if(force_0)
    {
        _S194 = true;
    }
    else
    {
        _S194 = (dot(dl_0, dl_0)) < (rad_0 * rad_0);
    }
    (&o_0)->hit_0 = _S194;
    return o_0;
}

float cascade_extent_0(FrameData_natural_0 device* f_4, uint c_5, float3 p_5)
{
    float4 s_8 = (((float4(p_5, 1.0f)) * (matrix<float,int(4),int(4)> ((&(&f_4->light_0)->shadow_1)->data_2[c_5].data_0[int(0)][int(0)], (&(&f_4->light_0)->shadow_1)->data_2[c_5].data_0[int(1)][int(0)], (&(&f_4->light_0)->shadow_1)->data_2[c_5].data_0[int(2)][int(0)], (&(&f_4->light_0)->shadow_1)->data_2[c_5].data_0[int(3)][int(0)], (&(&f_4->light_0)->shadow_1)->data_2[c_5].data_0[int(0)][int(1)], (&(&f_4->light_0)->shadow_1)->data_2[c_5].data_0[int(1)][int(1)], (&(&f_4->light_0)->shadow_1)->data_2[c_5].data_0[int(2)][int(1)], (&(&f_4->light_0)->shadow_1)->data_2[c_5].data_0[int(3)][int(1)], (&(&f_4->light_0)->shadow_1)->data_2[c_5].data_0[int(0)][int(2)], (&(&f_4->light_0)->shadow_1)->data_2[c_5].data_0[int(1)][int(2)], (&(&f_4->light_0)->shadow_1)->data_2[c_5].data_0[int(2)][int(2)], (&(&f_4->light_0)->shadow_1)->data_2[c_5].data_0[int(3)][int(2)], (&(&f_4->light_0)->shadow_1)->data_2[c_5].data_0[int(0)][int(3)], (&(&f_4->light_0)->shadow_1)->data_2[c_5].data_0[int(1)][int(3)], (&(&f_4->light_0)->shadow_1)->data_2[c_5].data_0[int(2)][int(3)], (&(&f_4->light_0)->shadow_1)->data_2[c_5].data_0[int(3)][int(3)]))));
    float _S195 = s_8.z;
    bool _S196;
    if(_S195 < 0.0f)
    {
        _S196 = true;
    }
    else
    {
        _S196 = _S195 > 1.0f;
    }
    float _S197;
    if(_S196)
    {
        _S197 = 1.0e+09f;
    }
    else
    {
        _S197 = max(abs(s_8.x), abs(s_8.y));
    }
    return _S197;
}

float pcf_radius_texels_0(FrameData_natural_0 device* f_5, float texel_0)
{
    return clamp((float4((&f_5->light_0)->sky_0) ).z / texel_0, 1.0f, max((float4((&f_5->light_0)->sky_0) ).w, 1.0f));
}

float ed_depth_read_0(uint i_19, uint2 p_6, KernelContext_0 thread* kernelContext_30)
{
    float _S198 = ((reinterpret_cast<device depth2d<float, access::sample>*>((kernelContext_30->ed_textures_0))[(i_19)].read((p_6))));
    return _S198;
}

float cascade_lookup_0(FrameData_natural_0 device* f_6, uint atlas_0, uint c_6, float3 p_7, float3 ng_0, float2 px_1, KernelContext_0 thread* kernelContext_31)
{
    float4 _S199 = float4((&(&f_6->light_0)->cascade_0)->data_3[c_6]) ;
    float tile_0 = (float4((&f_6->light_0)->cfg_0) ).y;
    float _S200 = _S199.x;
    float _S201 = pcf_radius_texels_0(f_6, _S200);
    float sp_0 = _S201 * 0.5f;
    float nol_0 = saturate(dot(ng_0, (float4(f_6->sun_dir_0) ).xyz));
    float4 s_9 = (((float4(p_7 + ng_0 * float3((_S200 * ((float4((&f_6->light_0)->filter_0) ).z + 2.0f * sqrt(1.0f - nol_0 * nol_0)))) , 1.0f)) * (matrix<float,int(4),int(4)> ((&(&f_6->light_0)->shadow_1)->data_2[c_6].data_0[int(0)][int(0)], (&(&f_6->light_0)->shadow_1)->data_2[c_6].data_0[int(1)][int(0)], (&(&f_6->light_0)->shadow_1)->data_2[c_6].data_0[int(2)][int(0)], (&(&f_6->light_0)->shadow_1)->data_2[c_6].data_0[int(3)][int(0)], (&(&f_6->light_0)->shadow_1)->data_2[c_6].data_0[int(0)][int(1)], (&(&f_6->light_0)->shadow_1)->data_2[c_6].data_0[int(1)][int(1)], (&(&f_6->light_0)->shadow_1)->data_2[c_6].data_0[int(2)][int(1)], (&(&f_6->light_0)->shadow_1)->data_2[c_6].data_0[int(3)][int(1)], (&(&f_6->light_0)->shadow_1)->data_2[c_6].data_0[int(0)][int(2)], (&(&f_6->light_0)->shadow_1)->data_2[c_6].data_0[int(1)][int(2)], (&(&f_6->light_0)->shadow_1)->data_2[c_6].data_0[int(2)][int(2)], (&(&f_6->light_0)->shadow_1)->data_2[c_6].data_0[int(3)][int(2)], (&(&f_6->light_0)->shadow_1)->data_2[c_6].data_0[int(0)][int(3)], (&(&f_6->light_0)->shadow_1)->data_2[c_6].data_0[int(1)][int(3)], (&(&f_6->light_0)->shadow_1)->data_2[c_6].data_0[int(2)][int(3)], (&(&f_6->light_0)->shadow_1)->data_2[c_6].data_0[int(3)][int(3)]))));
    float2 _S202 = float2(s_9.x * 0.5f + 0.5f, 0.5f - s_9.y * 0.5f) * float2(tile_0) ;
    float ign_0 = fract(52.982917785644531f * fract(dot(px_1, float2(0.067110560834407806f, 0.0058371499180793762f))));
    float2 _S203 = float2(0.5f) ;
    float2 _S204 = (float2(ign_0, fract(ign_0 * 7.309999942779541f + 0.37000000476837158f)) - _S203) * float2(0.0f) ;
    float3 ns_0 = (((float4(ng_0, 0.0f)) * (matrix<float,int(4),int(4)> ((&(&f_6->light_0)->shadow_1)->data_2[c_6].data_0[int(0)][int(0)], (&(&f_6->light_0)->shadow_1)->data_2[c_6].data_0[int(1)][int(0)], (&(&f_6->light_0)->shadow_1)->data_2[c_6].data_0[int(2)][int(0)], (&(&f_6->light_0)->shadow_1)->data_2[c_6].data_0[int(3)][int(0)], (&(&f_6->light_0)->shadow_1)->data_2[c_6].data_0[int(0)][int(1)], (&(&f_6->light_0)->shadow_1)->data_2[c_6].data_0[int(1)][int(1)], (&(&f_6->light_0)->shadow_1)->data_2[c_6].data_0[int(2)][int(1)], (&(&f_6->light_0)->shadow_1)->data_2[c_6].data_0[int(3)][int(1)], (&(&f_6->light_0)->shadow_1)->data_2[c_6].data_0[int(0)][int(2)], (&(&f_6->light_0)->shadow_1)->data_2[c_6].data_0[int(1)][int(2)], (&(&f_6->light_0)->shadow_1)->data_2[c_6].data_0[int(2)][int(2)], (&(&f_6->light_0)->shadow_1)->data_2[c_6].data_0[int(3)][int(2)], (&(&f_6->light_0)->shadow_1)->data_2[c_6].data_0[int(0)][int(3)], (&(&f_6->light_0)->shadow_1)->data_2[c_6].data_0[int(1)][int(3)], (&(&f_6->light_0)->shadow_1)->data_2[c_6].data_0[int(2)][int(3)], (&(&f_6->light_0)->shadow_1)->data_2[c_6].data_0[int(3)][int(3)])))).xyz;
    float rl_0 = _S200 * tile_0 * 0.5f;
    float _S205 = _S199.y;
    float _S206 = ns_0.z * _S205;
    float izn_0 = 1.0f / max(abs(_S206), 0.11999999731779099f);
    float _S207 = float((int(sign((_S206)))));
    float gx_1 = clamp(- (ns_0.x * rl_0) * izn_0 * _S207, -8.0f, 8.0f);
    float gy_1 = clamp(- (ns_0.y * rl_0) * izn_0 * _S207, -8.0f, 8.0f);
    float _S208 = s_9.z + ((float4((&f_6->light_0)->filter_0) ).w + 0.5f * (abs(gx_1) + abs(gy_1))) * _S200 / _S205;
    float _S209 = _S200 / _S205;
    int2 _S210 = int2(_S199.zw);
    int2 _S211 = _S210 + int2((int(tile_0) - int(1))) ;
    float _S212 = 1.0f / (_S201 + sp_0);
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
        float _S213 = 1.0f - abs(oy_0) * _S212;
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
            float ox_0 = float(x_1) * sp_0;
            float w_3 = (1.0f - abs(ox_0) * _S212) * _S213;
            float2 tp_1 = floor(_S202 + float2(ox_0, oy_0) + _S204);
            float2 d_24 = tp_1 + _S203 - _S202;
            float _S214 = ed_depth_read_0(atlas_0, uint2(clamp(_S210 + int2(tp_1), _S210, _S211)), kernelContext_31);
            float _S215;
            if((_S208 + (gx_1 * d_24.x - gy_1 * d_24.y) * _S209) >= _S214)
            {
                _S215 = 1.0f;
            }
            else
            {
                _S215 = 0.0f;
            }
            float lit_1 = lit_0 + _S215 * w_3;
            float wsum_1 = wsum_0 + w_3;
            x_1 = x_1 + int(1);
            lit_0 = lit_1;
            wsum_0 = wsum_1;
        }
        y_0 = y_0 + int(1);
    }
    return lit_0 / wsum_0;
}

float sun_shadow_0(FrameData_natural_0 device* f_7, uint atlas_1, float3 p_8, float3 ng_1, float dist_0, float2 px_2, KernelContext_0 thread* kernelContext_32)
{
    uint n_3 = uint((float4((&f_7->light_0)->cfg_0) ).x);
    bool _S216;
    if(((uint4((&f_7->light_0)->flags_0) ).x) == 0U)
    {
        _S216 = true;
    }
    else
    {
        _S216 = n_3 == 0U;
    }
    if(_S216)
    {
        _S216 = true;
    }
    else
    {
        _S216 = dist_0 >= ((float4((&f_7->light_0)->filter_0) ).y);
    }
    if(_S216)
    {
        return 1.0f;
    }
    float _S217 = 1.0f - (float4((&f_7->light_0)->cfg_0) ).w * 2.0f / (float4((&f_7->light_0)->cfg_0) ).y;
    float _S218 = (float4((&f_7->light_0)->cfg_0) ).z;
    uint c_7 = 0U;
    for(;;)
    {
        if(c_7 < n_3)
        {
        }
        else
        {
            break;
        }
        float _S219 = cascade_extent_0(f_7, c_7, p_8);
        float e_11 = _S219 / _S217;
        if(e_11 > 1.0f)
        {
            c_7 = c_7 + 1U;
            continue;
        }
        float _S220 = cascade_lookup_0(f_7, atlas_1, c_7, p_8, ng_1, px_2, kernelContext_32);
        float blend_0 = smoothstep(1.0f - _S218, 1.0f, e_11);
        float v_3;
        if(blend_0 > 0.0f)
        {
            uint _S221 = c_7 + 1U;
            if(_S221 < n_3)
            {
                float _S222 = cascade_extent_0(f_7, _S221, p_8);
                _S216 = (_S222 / _S217) <= 1.0f;
            }
            else
            {
                _S216 = false;
            }
            if(_S216)
            {
                float _S223 = cascade_lookup_0(f_7, atlas_1, _S221, p_8, ng_1, px_2, kernelContext_32);
                v_3 = _S223;
            }
            else
            {
                v_3 = 1.0f;
            }
            v_3 = mix(_S220, v_3, blend_0);
        }
        else
        {
            v_3 = _S220;
        }
        return mix(v_3, 1.0f, smoothstep(0.0f, 1.0f, saturate((dist_0 - (float4((&f_7->light_0)->filter_0) ).x) / max((float4((&f_7->light_0)->filter_0) ).y - (float4((&f_7->light_0)->filter_0) ).x, 0.0010000000474974513f))));
    }
    return 1.0f;
}

packed_float4 device* ed_buf_4(uint i_20, KernelContext_0 thread* kernelContext_33)
{
    return (packed_float4 device*)kernelContext_33->ed_buffers_0[i_20];
}

float2 ao_upsample_0(FrameData_natural_0 device* f_8, uint ao_buf_0, float2 pixel_0, float zlin_0, KernelContext_0 thread* kernelContext_34)
{
    float sv_0;
    if(((uint4((&f_8->light_0)->flags_0) ).y) == 0U)
    {
        return float2(1.0f) ;
    }
    packed_float4 device* _S224 = ed_buf_4(ao_buf_0, kernelContext_34);
    uint _S225 = uint((float4((&f_8->light_0)->ao_1) ).y);
    uint _S226 = uint((float4((&f_8->light_0)->ao_1) ).z);
    float2 _S227 = float2(0.5f) ;
    float2 h_9 = (pixel_0 - _S227) * _S227;
    float2 b_7 = floor(h_9);
    float2 _S228 = h_9 - b_7;
    float2 _S229 = float2(0.0f) ;
    int y_1 = int(0);
    float2 sum_0 = _S229;
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
        int x_2 = int(0);
        for(;;)
        {
            if(x_2 < int(2))
            {
            }
            else
            {
                break;
            }
            int2 t_0 = clamp(int2(b_7) + int2(x_2, y_1), int2(int(0)) , int2(int(_S225) - int(1), int(_S226) - int(1)));
            float4 _S230 = float4(*(_S224 + (uint(t_0.y) * _S225 + uint(t_0.x)))) ;
            if(x_2 == int(0))
            {
                sv_0 = 1.0f - _S228.x;
            }
            else
            {
                sv_0 = _S228.x;
            }
            float _S231;
            if(y_1 == int(0))
            {
                _S231 = 1.0f - _S228.y;
            }
            else
            {
                _S231 = _S228.y;
            }
            float w_4 = max(sv_0 * _S231, 0.0010000000474974513f) * (1.0f / (0.0010000000474974513f + abs(_S230.z - zlin_0) / max(zlin_0, 0.0010000000474974513f) * 50.0f));
            float2 sum_1 = sum_0 + _S230.xy * float2(w_4) ;
            float wsum_3 = wsum_2 + w_4;
            x_2 = x_2 + int(1);
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
    if(((float4((&f_8->light_0)->skyvis_0) ).x) > 0.0f)
    {
        sv_0 = saturate(r_3.y);
    }
    else
    {
        sv_0 = 1.0f;
    }
    return float2(pow(saturate(r_3.x), (float4((&f_8->light_0)->ao_1) ).x), sv_0);
}

bool atmos_on_0(FrameData_natural_0 device* f_9)
{
    return ((uint4((&f_9->atmos_0)->cfg_1) ).x) != 0U;
}

float atm_rb_0(FrameData_natural_0 device* f_10)
{
    return (float4((&f_10->atmos_0)->radii_0) ).x;
}

float atm_height_at_0(float rb_0, float h_10, float mu_0, float t_1)
{
    float q_6 = t_1 * t_1 + 2.0f * (rb_0 + h_10) * mu_0 * t_1 + h_10 * (2.0f * rb_0 + h_10);
    return q_6 / (sqrt(max(rb_0 * rb_0 + q_6, 0.0f)) + rb_0);
}

float atm_rt_0(FrameData_natural_0 device* f_11)
{
    return (float4((&f_11->atmos_0)->radii_0) ).y;
}

float atm_dist_top_0(float rb_1, float rt_0, float h_11, float mu_1)
{
    float r_4 = rb_1 + h_11;
    float _S232 = r_4 * mu_1;
    return max(- r_4 * mu_1 + sqrt(max(_S232 * _S232 + (rt_0 - rb_1 - h_11) * (rt_0 + r_4), 0.0f)), 0.0f);
}

float2 atm_trans_uv_0(float rb_2, float rt_1, float h_12, float mu_2)
{
    float ht_0 = rt_1 - rb_2;
    float _S233 = clamp(h_12, 0.0f, ht_0);
    float big_h_0 = sqrt(ht_0 * (rt_1 + rb_2));
    float rho_0 = sqrt(max(_S233 * (2.0f * rb_2 + _S233), 0.0f));
    float dmin_0 = ht_0 - _S233;
    return float2((atm_dist_top_0(rb_2, rt_1, _S233, mu_2) - dmin_0) / max(rho_0 + big_h_0 - dmin_0, 9.99999997475242708e-07f), rho_0 / big_h_0);
}

float4 atm_bilinear_0(packed_float4 device* buf_3, uint w_5, uint row0_0, uint rows_1, float2 uv_0)
{
    float2 p_9 = saturate(uv_0) * float2(float(w_5 - 1U), float(rows_1 - 1U));
    uint2 _S234 = min(uint2(p_9), uint2(w_5 - 2U, rows_1 - 2U));
    float2 g_5 = p_9 - float2(_S234);
    uint b_8 = (row0_0 + _S234.y) * w_5 + _S234.x;
    float4 _S235 = float4(g_5.x) ;
    uint _S236 = b_8 + w_5;
    return mix(mix(float4(*(buf_3 + b_8)) , float4(*(buf_3 + (b_8 + 1U))) , _S235), mix(float4(*(buf_3 + _S236)) , float4(*(buf_3 + (_S236 + 1U))) , _S235), float4(g_5.y) );
}

float3 atm_transmittance_0(FrameData_natural_0 device* f_12, float h_13, float mu_3, KernelContext_0 thread* kernelContext_35)
{
    float _S237 = atm_rb_0(f_12);
    float _S238 = atm_rt_0(f_12);
    float2 uv_1 = atm_trans_uv_0(_S237, _S238, h_13, mu_3);
    packed_float4 device* _S239 = ed_buf_4((uint4((&f_12->atmos_0)->bufs_0) ).x, kernelContext_35);
    float4 _S240 = atm_bilinear_0(_S239, 256U, 0U, 64U, uv_1);
    return _S240.xyz;
}

float atm_rb_1(FrameData_natural_0 device* f_13)
{
    return (float4((&f_13->atmos_0)->radii_0) ).x;
}

float atm_cos_horizon_0(float rb_3, float h_14)
{
    return - sqrt(max(h_14 * (2.0f * rb_3 + h_14), 0.0f)) / (rb_3 + h_14);
}

float atm_earth_shadow_0(FrameData_natural_0 device* f_14, float h_15, float mu_s_0)
{
    float _S241 = atm_rb_1(f_14);
    float ch_0 = atm_cos_horizon_0(_S241, h_15);
    float s_10 = 0.0046999999321997166f * sqrt(max(1.0f - ch_0 * ch_0, 0.0f)) + 9.9999997473787516e-06f;
    return smoothstep(- s_10, s_10, mu_s_0 - ch_0);
}

float3 atm_sun_transmittance_0(FrameData_natural_0 device* f_15, float h_16, float mu_s_1, KernelContext_0 thread* kernelContext_36)
{
    float3 _S242 = atm_transmittance_0(f_15, h_16, mu_s_1, kernelContext_36);
    float _S243 = atm_earth_shadow_0(f_15, h_16, mu_s_1);
    return _S242 * float3(_S243) ;
}

float3 atmos_sun_at_0(FrameData_natural_0 device* f_16, float3 p_10, KernelContext_0 thread* kernelContext_37)
{
    float t_2 = length(p_10) * 0.0010000000474974513f;
    float3 d_25;
    if(t_2 > 0.0f)
    {
        d_25 = p_10 * float3((0.0010000000474974513f / t_2)) ;
    }
    else
    {
        d_25 = (float4((&f_16->atmos_0)->up_0) ).xyz;
    }
    float _S244 = atm_rb_0(f_16);
    float h0_0 = (float4((&f_16->atmos_0)->radii_0) ).z;
    float h_17 = atm_height_at_0(_S244, h0_0, dot(d_25, (float4((&f_16->atmos_0)->up_0) ).xyz), t_2);
    float3 _S245 = (float4((&f_16->atmos_0)->sun_0) ).xyz;
    float3 _S246 = atm_sun_transmittance_0(f_16, h_17, clamp(((_S244 + h0_0) * (float4((&f_16->atmos_0)->up_0) ).w + t_2 * dot(d_25, (float4(f_16->sun_dir_0) ).xyz)) / (_S244 + h_17), -1.0f, 1.0f), kernelContext_37);
    return _S245 * _S246;
}

float3 sun_light_at_0(FrameData_natural_0 device* f_17, float3 p_11, KernelContext_0 thread* kernelContext_38)
{
    bool _S247 = atmos_on_0(f_17);
    float3 _S248;
    if(_S247)
    {
        float3 _S249 = atmos_sun_at_0(f_17, p_11, kernelContext_38);
        _S248 = _S249;
    }
    else
    {
        _S248 = (float4(f_17->sun_color_0) ).xyz;
    }
    return _S248;
}

float3 f_schlick_0(float3 f0_0, float voh_0)
{
    return f0_0 + (float3(1.0f)  - f0_0) * float3(pow(1.0f - voh_0, 5.0f)) ;
}

float d_ggx_0(float noh_0, float a_8)
{
    float a2_0 = a_8 * a_8;
    float d_26 = noh_0 * noh_0 * (a2_0 - 1.0f) + 1.0f;
    return a2_0 / (3.1415927410125732f * d_26 * d_26);
}

float v_smith_0(float nov_0, float nol_1, float a_9)
{
    float a2_1 = a_9 * a_9;
    float _S250 = 1.0f - a2_1;
    return 0.5f / max(nol_1 * sqrt(nov_0 * nov_0 * _S250 + a2_1) + nov_0 * sqrt(nol_1 * nol_1 * _S250 + a2_1), 9.9999997473787516e-06f);
}

float2 env_brdf_0(float nov_1, float rough_0)
{
    float4 r_5 = float4(rough_0)  * float4(-1.0f, -0.027499999850988388f, -0.57200002670288086f, 0.02199999988079071f) + float4(1.0f, 0.042500000447034836f, 1.0399999618530273f, -0.039999999105930328f);
    float _S251 = r_5.x;
    return float2(-1.0399999618530273f, 1.0399999618530273f) * float2((min(_S251 * _S251, exp2(-9.2799997329711914f * nov_1)) * _S251 + r_5.y))  + r_5.zw;
}

bool atmos_on_1(FrameData_natural_0 device* f_18)
{
    return ((uint4((&f_18->atmos_0)->cfg_1) ).x) != 0U;
}

float3 atm_sky_view_0(FrameData_natural_0 device* f_19, uint buf_4, uint w_6, uint rows_2, float h_18, float3 d_27, KernelContext_0 thread* kernelContext_39)
{
    float mu_4 = clamp(dot(d_27, (float4((&f_19->atmos_0)->up_0) ).xyz), -1.0f, 1.0f);
    float mu_s_2 = (float4((&f_19->atmos_0)->up_0) ).w;
    float nu_0 = dot(d_27, (float4(f_19->sun_dir_0) ).xyz);
    float _S252 = sqrt(max(1.0f - mu_4 * mu_4, 0.0f)) * sqrt(max(1.0f - mu_s_2 * mu_s_2, 0.0f));
    float cphi_0;
    if(_S252 > 9.99999997475242708e-07f)
    {
        cphi_0 = clamp((nu_0 - mu_4 * mu_s_2) / _S252, -1.0f, 1.0f);
    }
    else
    {
        cphi_0 = 1.0f;
    }
    float u_0 = sqrt(acos(cphi_0) / 3.1415927410125732f);
    float _S253 = atm_rb_0(f_19);
    float zha_0 = acos(atm_cos_horizon_0(_S253, h_18));
    float theta_0 = acos(mu_4);
    uint half_rows_0 = rows_2 / 2U;
    float c_8;
    uint row0_1;
    if(theta_0 < zha_0)
    {
        float _S254 = 1.0f - sqrt(saturate(1.0f - theta_0 / zha_0));
        row0_1 = 0U;
        c_8 = _S254;
    }
    else
    {
        float _S255 = sqrt(saturate((theta_0 - zha_0) / max(3.1415927410125732f - zha_0, 9.99999997475242708e-07f)));
        row0_1 = half_rows_0;
        c_8 = _S255;
    }
    packed_float4 device* _S256 = ed_buf_4(buf_4, kernelContext_39);
    float4 _S257 = atm_bilinear_0(_S256, w_6, row0_1, half_rows_0, float2(u_0, c_8));
    return _S257.xyz;
}

float3 atmos_sky_0(FrameData_natural_0 device* f_20, float3 d_28, KernelContext_0 thread* kernelContext_40)
{
    float3 _S258 = atm_sky_view_0(f_20, (uint4((&f_20->atmos_0)->bufs_0) ).z, 192U, 108U, (float4((&f_20->atmos_0)->radii_0) ).z, d_28, kernelContext_40);
    return _S258;
}

float3 sky_rad_0(FrameData_natural_0 device* f_21, float3 d_29, float disc_k_0)
{
    float3 sun_1 = (float4(f_21->sun_dir_0) ).xyz;
    float h_19 = d_29.y;
    float t_3 = pow(saturate(1.0f - max(h_19, 0.0f)), 6.5f);
    float mu_5 = dot(d_29, sun_1);
    float warm_0 = saturate(1.0f - sun_1.y * 2.5f);
    float3 c_9 = mix((float4(f_21->sky_zenith_0) ).xyz, (float4(f_21->sky_horizon_0) ).xyz, float3(t_3) ) + float3(1.0f, 0.41999998688697815f, 0.14000000059604645f) * float3(pow(saturate(mu_5 * 0.5f + 0.5f), 5.0f))  * float3(warm_0)  * float3(0.85000002384185791f)  * float3((0.40000000596046448f + t_3)) ;
    float3 _S259 = float3((float4(f_21->sun_dir_0) ).w) ;
    return mix(mix(c_9, c_9 * float3(1.0f, 0.69999998807907104f, 0.55000001192092896f), float3((warm_0 * 0.40000000596046448f)) ) + float3(1.0f, 0.89999997615814209f, 0.69999998807907104f) * float3(pow(saturate(mu_5), 28.0f))  * float3(0.34999999403953552f) , (float4(f_21->ground_color_0) ).xyz, float3(saturate(- h_19 * 6.0f)) ) * _S259 + float3(1.0f, 0.94999998807907104f, 0.85000002384185791f) * float3((smoothstep(0.99965000152587891f, 0.99984997510910034f, mu_5) * step(0.0f, h_19)))  * float3(60.0f)  * _S259 * float3(disc_k_0) ;
}

float3 sky_env_0(FrameData_natural_0 device* f_22, float3 d_30, KernelContext_0 thread* kernelContext_41)
{
    bool _S260 = atmos_on_1(f_22);
    float3 _S261;
    if(_S260)
    {
        float3 _S262 = atmos_sky_0(f_22, d_30, kernelContext_41);
        _S261 = _S262;
    }
    else
    {
        float3 _S263 = sky_rad_0(f_22, d_30, 0.0f);
        _S261 = _S263;
    }
    return _S261;
}

float3 sh_irradiance_0(FrameData_natural_0 device* f_23, float3 n_4, KernelContext_0 thread* kernelContext_42)
{
    thread array<float4, int(9)> s_11;
    packed_float4 device* _S264 = ed_buf_4((uint4((&f_23->atmos_0)->bufs2_0) ).z, kernelContext_42);
    uint k_6 = 0U;
    for(;;)
    {
        if(k_6 < 9U)
        {
        }
        else
        {
            break;
        }
        bool _S265 = atmos_on_0(f_23);
        float4 _S266;
        if(_S265)
        {
            _S266 = float4(*(_S264 + (288U + k_6))) ;
        }
        else
        {
            _S266 = float4((&(&f_23->light_0)->sh_0)->data_4[k_6]) ;
        }
        s_11[k_6] = _S266;
        k_6 = k_6 + 1U;
    }
    float _S267 = n_4.y;
    float _S268 = n_4.z;
    float _S269 = n_4.x;
    return max(s_11[int(0)].xyz + (s_11[int(1)].xyz * float3(_S267)  + s_11[int(2)].xyz * float3(_S268)  + s_11[int(3)].xyz * float3(_S269) ) + (s_11[int(4)].xyz * float3((_S269 * _S267))  + s_11[int(5)].xyz * float3((_S267 * _S268))  + s_11[int(6)].xyz * float3((3.0f * _S268 * _S268 - 1.0f)) ) + (s_11[int(7)].xyz * float3((_S269 * _S268))  + s_11[int(8)].xyz * float3((_S269 * _S269 - _S267 * _S267)) ), float3(0.0f) );
}

float3 hemi_ambient_0(FrameData_natural_0 device* f_24, float3 n_5)
{
    return mix((float4(f_24->ground_color_0) ).xyz * float3((float4(f_24->sun_dir_0) ).w) , mix((float4(f_24->sky_horizon_0) ).xyz, (float4(f_24->sky_zenith_0) ).xyz, float3(0.5f) ) * float3((float4(f_24->sun_dir_0) ).w) , float3((n_5.y * 0.5f + 0.5f)) );
}

float3 ambient_light_0(FrameData_natural_0 device* f_25, float3 n_6, KernelContext_0 thread* kernelContext_43)
{
    float3 _S270;
    if(((uint4((&f_25->light_0)->flags_0) ).z) != 0U)
    {
        float3 _S271 = sh_irradiance_0(f_25, n_6, kernelContext_43);
        _S270 = _S271;
    }
    else
    {
        float3 _S272 = hemi_ambient_0(f_25, n_6);
        _S270 = _S272;
    }
    return _S270;
}

float3 occluded_ambient_0(FrameData_natural_0 device* f_26, float3 open_0, float sv_1, KernelContext_0 thread* kernelContext_44)
{
    float3 _S273 = ambient_light_0(f_26, float3(0.0f, -1.0f, 0.0f), kernelContext_44);
    return mix(_S273 * float3((float4((&f_26->light_0)->skyvis_0) ).y) , open_0, float3(sv_1) );
}

float3 ao_multibounce_0(FrameData_natural_0 device* f_27, float ao_2, float3 albedo_0)
{
    float3 _S274 = float3(ao_2) ;
    return mix(_S274, max(_S274, ((_S274 * (float3(2.0404000282287598f)  * albedo_0 - float3(0.33239999413490295f) ) + (float3(-4.795100212097168f)  * albedo_0 + float3(0.64170002937316895f) )) * _S274 + (float3(2.7551999092102051f)  * albedo_0 + float3(0.69029998779296875f) )) * _S274), float3((float4((&f_27->light_0)->sky_0) ).y) );
}

float3 shade_pbr_k_0(FrameData_natural_0 device* f_28, float3 sun_2, float3 albedo_1, float metal_0, float rough_1, float ao_3, float sv_2, float sun_vis_0, float3 n_7, float3 v_4, float spec_k_0, KernelContext_0 thread* kernelContext_45)
{
    float _S275 = saturate(metal_0);
    float _S276 = clamp(rough_1, 0.039999999105930328f, 1.0f);
    float a_10 = _S276 * _S276;
    float3 l_3 = (float4(f_28->sun_dir_0) ).xyz;
    float3 h_20 = normalize(l_3 + v_4);
    float nol_2 = saturate(dot(n_7, l_3));
    float _S277 = max(dot(n_7, v_4), 9.9999997473787516e-05f);
    float3 f0_1 = mix(float3(0.039999999105930328f) , albedo_1, float3(_S275) );
    float3 diff_col_0 = albedo_1 * float3((1.0f - _S275)) ;
    float3 F_0 = f_schlick_0(f0_1, saturate(dot(v_4, h_20)));
    float3 _S278 = float3(spec_k_0) ;
    float3 _S279 = float3(1.0f) ;
    float3 direct_0 = (diff_col_0 * (_S279 - F_0) / float3(3.1415927410125732f)  + float3((d_ggx_0(saturate(dot(n_7, h_20)), a_10) * v_smith_0(_S277, nol_2, a_10)))  * F_0 * _S278) * sun_2 * float3((nol_2 * sun_vis_0)) ;
    float2 ab_0 = env_brdf_0(_S277, _S276);
    float3 r_6 = reflect(- v_4, n_7);
    float3 env_0;
    if(((uint4((&f_28->light_0)->flags_0) ).z) != 0U)
    {
        float3 _S280 = sky_env_0(f_28, r_6, kernelContext_45);
        float3 _S281 = sh_irradiance_0(f_28, r_6, kernelContext_45);
        env_0 = mix(_S280, _S281, float3(_S276) );
    }
    else
    {
        float3 _S282 = hemi_ambient_0(f_28, r_6);
        env_0 = _S282;
    }
    float3 _S283 = occluded_ambient_0(f_28, env_0, sv_2, kernelContext_45);
    float spec_occ_0 = saturate(pow(_S277 + ao_3, exp2(-16.0f * a_10 - 1.0f)) - 1.0f + ao_3);
    float3 f_env_0 = f0_1 * float3(ab_0.x)  + float3(ab_0.y) ;
    float3 _S284 = diff_col_0 * (_S279 - f_env_0);
    float3 _S285 = ambient_light_0(f_28, n_7, kernelContext_45);
    float3 _S286 = occluded_ambient_0(f_28, _S285, sv_2, kernelContext_45);
    float3 _S287 = _S284 * _S286;
    float3 _S288 = ao_multibounce_0(f_28, ao_3, diff_col_0);
    return direct_0 + (_S287 * _S288 + _S283 * f_env_0 * float3(spec_occ_0)  * _S278);
}

float3 shade_foliage_k_0(FrameData_natural_0 device* f_29, float3 sun_3, float3 albedo_2, float rough_2, float ao_4, float sv_3, float sun_vis_1, float3 n_8, float3 ng_2, float3 v_5, bool leaf_0, float spec_k_1, KernelContext_0 thread* kernelContext_46)
{
    float3 _S289 = shade_pbr_k_0(f_29, sun_3, albedo_2, 0.0f, rough_2, ao_4, sv_3, sun_vis_1, n_8, v_5, spec_k_1, kernelContext_46);
    float3 col_0;
    if(leaf_0)
    {
        float3 l_4 = (float4(f_29->sun_dir_0) ).xyz;
        float3 trans_0 = albedo_2 * float3(1.1499999761581421f, 1.25f, 0.55000001192092896f);
        float3 col_1 = _S289 + sun_3 * float3(sun_vis_1)  * trans_0 * float3((saturate(- dot(n_8, l_4)) * (0.44999998807907104f + 1.2000000476837158f * pow(saturate(dot(- v_5, l_4)), 4.0f))))  * float3(0.31830987334251404f) ;
        float3 _S290 = hemi_ambient_0(f_29, - n_8);
        col_0 = col_1 + trans_0 * _S290 * float3(mix((float4((&f_29->light_0)->skyvis_0) ).y, 1.0f, sv_3))  * float3(ao_4)  * float3(0.30000001192092896f) ;
    }
    else
    {
        col_0 = _S289;
    }
    return col_0;
}

float atm_enter_top_0(float rb_4, float rt_2, float h_21, float mu_6)
{
    float r_7 = rb_4 + h_21;
    float _S291 = r_7 * mu_6;
    float d_31 = _S291 * _S291 - (h_21 - (rt_2 - rb_4)) * (r_7 + rt_2);
    if(d_31 < 0.0f)
    {
        return -1.0f;
    }
    float t_4 = - r_7 * mu_6 - sqrt(d_31);
    float _S292;
    if(t_4 >= 0.0f)
    {
        _S292 = t_4;
    }
    else
    {
        _S292 = -1.0f;
    }
    return _S292;
}

float atm_ap_slice_0(FrameData_natural_0 device* f_30, float t_5)
{
    float n_9 = (float4((&f_30->atmos_0)->ap_0) ).x;
    return log2(1.0f + max(t_5, 0.0f) / n_9) / log2(1.0f + (float4((&f_30->atmos_0)->ap_0) ).y / n_9) * 32.0f;
}

void atm_froxel_xy_0(packed_float4 device* l_5, packed_float4 device* t_6, float2 g_6, uint2 i_21, uint k_7, float3 thread* lo_2, float3 thread* to_0)
{
    uint b_9 = (k_7 * 32U + i_21.y) * 32U + i_21.x;
    uint _S293 = b_9 + 1U;
    float4 _S294 = float4(g_6.x) ;
    uint _S295 = b_9 + 32U;
    uint _S296 = _S295 + 1U;
    float4 t0_0 = mix(float4(*(t_6 + b_9)) , float4(*(t_6 + _S293)) , _S294);
    float4 t1_0 = mix(float4(*(t_6 + _S295)) , float4(*(t_6 + _S296)) , _S294);
    float4 _S297 = float4(g_6.y) ;
    *lo_2 = mix(mix(float4(*(l_5 + b_9)) , float4(*(l_5 + _S293)) , _S294), mix(float4(*(l_5 + _S295)) , float4(*(l_5 + _S296)) , _S294), _S297).xyz;
    *to_0 = mix(t0_0, t1_0, _S297).xyz;
    return;
}

void atmos_aerial_0(FrameData_natural_0 device* f_31, float3 p_12, float2 px_3, float3 thread* l_6, float3 thread* tr_0, KernelContext_0 thread* kernelContext_47)
{
    *l_6 = float3(0.0f) ;
    float3 _S298 = float3(1.0f) ;
    *tr_0 = _S298;
    float t_7 = length(p_12) * 0.0010000000474974513f;
    if(t_7 <= 0.0f)
    {
        return;
    }
    float h_22 = (float4((&f_31->atmos_0)->radii_0) ).z;
    float _S299 = atm_rb_0(f_31);
    float _S300 = atm_rt_0(f_31);
    float te_0;
    if(h_22 > (_S300 - _S299))
    {
        float te_1 = atm_enter_top_0(_S299, _S300, h_22, dot(p_12 * float3((0.0010000000474974513f / t_7)) , (float4((&f_31->atmos_0)->up_0) ).xyz));
        bool _S301;
        if(te_1 < 0.0f)
        {
            _S301 = true;
        }
        else
        {
            _S301 = t_7 <= te_1;
        }
        if(_S301)
        {
            return;
        }
        te_0 = te_1;
    }
    else
    {
        te_0 = 0.0f;
    }
    float _S302 = atm_ap_slice_0(f_31, t_7 - te_0);
    float2 q_7 = clamp(px_3 / (float4(f_31->viewport_0) ).zw * float2(32.0f)  - float2(0.5f) , float2(0.0f) , float2(31.0f) );
    uint2 _S303 = min(uint2(q_7), uint2(30U) );
    float2 g_7 = q_7 - float2(_S303);
    packed_float4 device* _S304 = ed_buf_4((uint4((&f_31->atmos_0)->bufs2_0) ).x, kernelContext_47);
    packed_float4 device* _S305 = ed_buf_4((uint4((&f_31->atmos_0)->bufs2_0) ).y, kernelContext_47);
    thread float3 la_0;
    thread float3 ta_0;
    thread float3 lc_3;
    thread float3 tc_0;
    if(_S302 < 1.0f)
    {
        atm_froxel_xy_0(_S304, _S305, g_7, _S303, 0U, &lc_3, &tc_0);
        float3 _S306 = float3(_S302) ;
        *l_6 = lc_3 * _S306;
        *tr_0 = mix(_S298, tc_0, _S306);
        return;
    }
    float _S307 = min(_S302 - 1.0f, 31.0f);
    uint _S308 = min(uint(_S307), 30U);
    float gz_0 = _S307 - float(_S308);
    atm_froxel_xy_0(_S304, _S305, g_7, _S303, _S308, &la_0, &ta_0);
    atm_froxel_xy_0(_S304, _S305, g_7, _S303, _S308 + 1U, &lc_3, &tc_0);
    float3 _S309 = float3(gz_0) ;
    *l_6 = mix(la_0, lc_3, _S309);
    *tr_0 = mix(ta_0, tc_0, _S309);
    return;
}

float3 apply_aerial_0(FrameData_natural_0 device* f_32, float3 col_2, float3 p_13, float2 px_4, KernelContext_0 thread* kernelContext_48)
{
    bool _S310 = atmos_on_0(f_32);
    if(!_S310)
    {
        return col_2;
    }
    thread float3 l_7;
    thread float3 tr_1;
    atmos_aerial_0(f_32, p_13, px_4, &l_7, &tr_1, kernelContext_48);
    if(((uint4((&f_32->atmos_0)->cfg_1) ).y) == 1U)
    {
        return tr_1 * float3(0.5f) ;
    }
    if(((uint4((&f_32->atmos_0)->cfg_1) ).y) == 2U)
    {
        return l_7;
    }
    return col_2 * tr_1 + l_7;
}

float4 shell_pixel_0(FrameData_natural_0 device* f_33, const FsOut_0 thread* i_22, uint2 vis_1, float3 dir_9, KernelContext_0 thread* kernelContext_49)
{
    TerrainDraw_natural_0 device* _S311 = ed_buf_2(kernelContext_49->ed_c_0->t_draws_0, kernelContext_49);
    uint _S312 = vis_1.x;
    TerrainDraw_natural_0 device* _S313 = _S311 + (_S312 & 16777215U);
    TerrainDraw_natural_0 d_32 = *_S313;
    thread TerrainDraw_natural_0 _S314 = *_S313;
    uint pool_22 = kernelContext_49->ed_c_0->t_pool_0;
    uint buf_5 = kernelContext_49->ed_c_0->t_draws_0;
    uint L_1 = (_S312 >> 24U) & 7U;
    uint _S315 = vis_1.y;
    uint k_8 = (_S315 >> 27U) & 15U;
    uint _S316 = max((uint4((&f_33->shell_0)->cfg_5) ).w, 1U);
    float hk_1 = (float4((&f_33->shell_0)->fu_1) ).w * float(k_8 + 1U) / float(_S316);
    uint3 tri_0 = t_triangle_0(_S315 & 67108863U);
    float4 _S317 = float4((&_S314)->dir_c_0) ;
    float3 bc_0 = (float4((&_S314)->origin_0) ).xyz - _S317.xyz * float3(_S317.w) ;
    uint _S318 = tri_0.x;
    _S314 = d_32;
    float3 _S319 = t_vertex_1(pool_22, &_S314, _S318, kernelContext_49);
    uint _S320 = tri_0.y;
    float3 _S321 = t_vertex_1(pool_22, &_S314, _S320, kernelContext_49);
    uint _S322 = tri_0.z;
    float3 _S323 = t_vertex_1(pool_22, &_S314, _S322, kernelContext_49);
    float3 _S324 = float3(hk_1) ;
    float3 a0_0 = _S319 + normalize(_S319 - bc_0) * _S324;
    float3 e1_1 = _S321 + normalize(_S321 - bc_0) * _S324 - a0_0;
    float3 e2_1 = _S323 + normalize(_S323 - bc_0) * _S324 - a0_0;
    float2 bb_0 = ray_bary_0(dir_9, a0_0, e1_1, e2_1);
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
    float3 p_14 = a0_0 + e1_1 * float3(b1_1)  + e2_1 * float3(b2_1) ;
    float2 g_8 = t_vertex_grid_0(_S318) * float2((1.0f - b1_1 - b2_1))  + t_vertex_grid_0(_S320) * float2(b1_1)  + t_vertex_grid_0(_S322) * float2(b2_1) ;
    thread float3 up_2;
    thread float h_23;
    float3 _S325 = t_smooth_normal_0(pool_22, &_S314, g_8, int(2), &up_2, &h_23, kernelContext_49);
    float2 _S326 = shell_wrel_0(f_33, &_S314, g_8);
    float dz_4 = - (((float4(p_14, 1.0f)) * (matrix<float,int(4),int(4)> (f_33->view_0.data_0[int(0)][int(0)], f_33->view_0.data_0[int(1)][int(0)], f_33->view_0.data_0[int(2)][int(0)], f_33->view_0.data_0[int(3)][int(0)], f_33->view_0.data_0[int(0)][int(1)], f_33->view_0.data_0[int(1)][int(1)], f_33->view_0.data_0[int(2)][int(1)], f_33->view_0.data_0[int(3)][int(1)], f_33->view_0.data_0[int(0)][int(2)], f_33->view_0.data_0[int(1)][int(2)], f_33->view_0.data_0[int(2)][int(2)], f_33->view_0.data_0[int(3)][int(2)], f_33->view_0.data_0[int(0)][int(3)], f_33->view_0.data_0[int(1)][int(3)], f_33->view_0.data_0[int(2)][int(3)], f_33->view_0.data_0[int(3)][int(3)])))).z;
    ShellHit_0 _S327 = shell_strand_0(f_33, buf_5, _S326, k_8, _S316, L_1, dz_4, true, kernelContext_49);
    GrassType_0 _S328 = shell_type_0(f_33, buf_5, _S327.kind_0, kernelContext_49);
    float3 _S329 = (float4((&_S314)->fu_0) ).xyz;
    float3 _S330 = (float4((&_S314)->fv_0) ).xyz;
    float far_2 = smoothstep(3.0f, (float4((&f_33->shell_0)->c_0) ).z * 0.60000002384185791f, dz_4);
    float3 nrm_0 = normalize(_S325 + (normalize(_S329 - up_2 * float3(dot(_S329, up_2)) ) * float3(_S327.side_0.x)  + normalize(_S330 - up_2 * float3(dot(_S330, up_2)) ) * float3(_S327.side_0.y) ) * float3((0.89999997615814209f * (1.0f - 0.5f * _S327.tt_0) * (1.0f - far_2))) );
    float3 v_6 = - dir_9;
    float _S331 = dot(nrm_0, v_6);
    float3 nrm_1;
    if(_S331 < 0.0f)
    {
        nrm_1 = normalize(nrm_0 - float3((1.0099999904632568f * _S331))  * v_6);
    }
    else
    {
        nrm_1 = nrm_0;
    }
    float3 alb_0;
    if((_S327.tt_0) < 0.5f)
    {
        alb_0 = mix(_S328.root_0.xyz, _S328.mid_0.xyz, float3((_S327.tt_0 * 2.0f)) );
    }
    else
    {
        alb_0 = mix(_S328.mid_0.xyz, _S328.tip_0.xyz, float3(((_S327.tt_0 - 0.5f) * 2.0f)) );
    }
    float hue_0 = grass_rand_0(grass_hash_0((as_type<uint>((_S327.rnd_0))) + 53U)) - 0.5f;
    float3 alb_1 = alb_0 * (float3(mix(0.8399999737739563f, 1.1200000047683716f, _S327.rnd_0))  * float3(1.0f + 0.10000000149011612f * hue_0, 1.0f, 1.0f - 0.11999999731779099f * hue_0));
    if((_S327.dry_0) > 0.5f)
    {
        alb_0 = mix(alb_1, _S328.dry_1.xyz * float3(mix(0.85000002384185791f, 1.1000000238418579f, _S327.rnd_0)) , float3(mix(0.5f, 1.0f, _S327.tt_0)) );
    }
    else
    {
        alb_0 = alb_1;
    }
    float3 _S332 = min(alb_0, float3(0.89999997615814209f, 0.89999997615814209f, 0.89999997615814209f));
    float ao_h_0 = mix(mix(0.40000000596046448f, 1.0f, pow(_S327.tt_0, 0.80000001192092896f)), 0.72000002861022949f, far_2 * 0.60000002384185791f);
    float2 _S333 = i_22->pos_0.xy;
    float _S334 = sun_shadow_0(f_33, kernelContext_49->ed_c_0->shadow_0, p_14, up_2, dz_4, _S333, kernelContext_49);
    float2 _S335 = ao_upsample_0(f_33, kernelContext_49->ed_c_0->ao_0, _S333, dz_4, kernelContext_49);
    float3 _S336 = sun_light_at_0(f_33, p_14, kernelContext_49);
    float _S337 = _S335.x;
    float _S338 = _S335.y;
    float3 _S339 = shade_foliage_k_0(f_33, _S336, _S332, 0.55000001192092896f, _S337 * ao_h_0, _S338, _S334 * mix(0.75f, 1.0f, _S327.tt_0), nrm_1, nrm_1, v_6, true, _S328.shell_1.w, kernelContext_49);
    float3 _S340 = apply_aerial_0(f_33, _S339, p_14, _S333, kernelContext_49);
    float3 col_3;
    if(((uint4((&f_33->light_0)->flags_0) ).w) == 2U)
    {
        col_3 = float3((0.5f * _S337 * ao_h_0)) ;
    }
    else
    {
        if(((uint4((&f_33->light_0)->flags_0) ).w) == 4U)
        {
            col_3 = float3((0.5f * _S338)) ;
        }
        else
        {
            if(((uint4((&f_33->light_0)->flags_0) ).w) == 1U)
            {
                col_3 = float3((0.039999999105930328f + 0.5f * _S334)) ;
            }
            else
            {
                col_3 = _S340;
            }
        }
    }
    return float4(col_3, 1.0f);
}

float t_sample_spacing_0(const TerrainDraw_natural_0 thread* d_33, float2 g_9)
{
    float2 _S341 = float2(0.5f, 0.0f);
    thread float3 dir_10;
    float3 _S342 = t_lattice_f_0(d_33, g_9 + _S341, 0.0f, &dir_10);
    float3 _S343 = t_lattice_f_0(d_33, g_9 - _S341, 0.0f, &dir_10);
    return length(_S342 - _S343) / 4.0f;
}

float3 terrain_albedo_0(float h_24, float slope_cos_0)
{
    return mix(mix(mix(float3(0.054999999701976776f, 0.075000002980232239f, 0.039999999105930328f), float3(0.125f, 0.14000000059604645f, 0.070000000298023224f), float3(smoothstep(1950.0f, 2350.0f, h_24)) ), float3(0.20999999344348907f, 0.20000000298023224f, 0.18500000238418579f), float3(smoothstep(0.81999999284744263f, 0.72000002861022949f, slope_cos_0)) ), float3(0.80000001192092896f, 0.81999999284744263f, 0.86000001430511475f), float3((smoothstep(2750.0f, 3050.0f, h_24) * smoothstep(0.55000001192092896f, 0.69999998807907104f, slope_cos_0))) );
}

float4 t_canopy_bilin_0(uint pool_23, uint off_4, float2 s_12, KernelContext_0 thread* kernelContext_50)
{
    float2 _S344 = clamp(s_12, float2(0.0f) , float2(256.0f) );
    int2 _S345 = int2(min(floor(_S344), float2(255.0f) ));
    float2 _S346 = _S344 - float2(_S345);
    uint _S347 = uint(_S345.x);
    uint _S348 = uint(_S345.y);
    uint _S349 = t_canopy_raw_0(pool_23, off_4, _S347, _S348, kernelContext_50);
    float _S350 = _S346.x;
    float _S351 = 1.0f - _S350;
    float _S352 = _S346.y;
    float _S353 = 1.0f - _S352;
    float wk_0 = _S351 * _S353;
    float ww_0 = wk_0 * (float(_S349 & 255U) / 255.0f);
    float _S354 = ww_0 * t_canopy_h_0((_S349 >> 8U) & 255U);
    float _S355 = ww_0 * float((_S349 >> 16U) & 255U) / 255.0f;
    float _S356 = wk_0 * float(_S349 >> 30U) / 3.0f;
    uint _S357 = _S347 + 1U;
    uint _S358 = t_canopy_raw_0(pool_23, off_4, _S357, _S348, kernelContext_50);
    float wk_1 = _S350 * _S353;
    float ww_1 = wk_1 * (float(_S358 & 255U) / 255.0f);
    float cover_0 = ww_0 + ww_1;
    float hs_0 = _S354 + ww_1 * t_canopy_h_0((_S358 >> 8U) & 255U);
    float mx_0 = _S355 + ww_1 * float((_S358 >> 16U) & 255U) / 255.0f;
    float un_0 = _S356 + wk_1 * float(_S358 >> 30U) / 3.0f;
    uint _S359 = _S348 + 1U;
    uint _S360 = t_canopy_raw_0(pool_23, off_4, _S347, _S359, kernelContext_50);
    float wk_2 = _S351 * _S352;
    float ww_2 = wk_2 * (float(_S360 & 255U) / 255.0f);
    float cover_1 = cover_0 + ww_2;
    float hs_1 = hs_0 + ww_2 * t_canopy_h_0((_S360 >> 8U) & 255U);
    float mx_1 = mx_0 + ww_2 * float((_S360 >> 16U) & 255U) / 255.0f;
    float un_1 = un_0 + wk_2 * float(_S360 >> 30U) / 3.0f;
    uint _S361 = t_canopy_raw_0(pool_23, off_4, _S357, _S359, kernelContext_50);
    float wk_3 = _S350 * _S352;
    float ww_3 = wk_3 * (float(_S361 & 255U) / 255.0f);
    float cover_2 = cover_1 + ww_3;
    float hs_2 = hs_1 + ww_3 * t_canopy_h_0((_S361 >> 8U) & 255U);
    float mx_2 = mx_1 + ww_3 * float((_S361 >> 16U) & 255U) / 255.0f;
    float un_2 = un_1 + wk_3 * float(_S361 >> 30U) / 3.0f;
    float4 _S362;
    if(cover_2 > 0.0f)
    {
        _S362 = float4(cover_2, hs_2 / cover_2, mx_2 / cover_2, un_2);
    }
    else
    {
        _S362 = float4(cover_2, 0.0f, 0.0f, un_2);
    }
    return _S362;
}

struct TCarpet_0
{
    bool on_0;
    float v_7;
    float dither_0;
    float edge_5;
    float mask_0;
    float2 cover_3;
    float tall_0;
    float3 grass_gain_0;
    float3 soil_gain_0;
};

float3 view_dir_0(FrameData_natural_0 device* f_34, float2 pixel_1)
{
    float2 uv_2 = pixel_1 / (float4(f_34->viewport_0) ).zw;
    float4 p_15 = (((float4(float2(uv_2.x * 2.0f - 1.0f, 1.0f - uv_2.y * 2.0f), 1.0f, 1.0f)) * (matrix<float,int(4),int(4)> (f_34->inv_view_proj_0.data_0[int(0)][int(0)], f_34->inv_view_proj_0.data_0[int(1)][int(0)], f_34->inv_view_proj_0.data_0[int(2)][int(0)], f_34->inv_view_proj_0.data_0[int(3)][int(0)], f_34->inv_view_proj_0.data_0[int(0)][int(1)], f_34->inv_view_proj_0.data_0[int(1)][int(1)], f_34->inv_view_proj_0.data_0[int(2)][int(1)], f_34->inv_view_proj_0.data_0[int(3)][int(1)], f_34->inv_view_proj_0.data_0[int(0)][int(2)], f_34->inv_view_proj_0.data_0[int(1)][int(2)], f_34->inv_view_proj_0.data_0[int(2)][int(2)], f_34->inv_view_proj_0.data_0[int(3)][int(2)], f_34->inv_view_proj_0.data_0[int(0)][int(3)], f_34->inv_view_proj_0.data_0[int(1)][int(3)], f_34->inv_view_proj_0.data_0[int(2)][int(3)], f_34->inv_view_proj_0.data_0[int(3)][int(3)]))));
    return normalize(p_15.xyz / float3(p_15.w) );
}

float2 gf_hash2_0(int2 c_10, uint salt_1)
{
    uint h_25 = ((uint(c_10.x) * 2376512323U) ^ (uint(c_10.y) * 3625334849U)) ^ (salt_1 * 2654435761U);
    uint h_26 = (h_25 ^ (h_25 >> 13U)) * 2246822507U;
    uint h_27 = h_26 ^ (h_26 >> 16U);
    return float2(float(h_27 & 65535U), float(h_27 >> 16U)) / float2(65535.0f) ;
}

float2 gf_noise2_0(float2 w_7, float cell_1, uint salt_2)
{
    float2 q_8 = w_7 / float2(cell_1) ;
    float2 i_23 = floor(q_8);
    float2 f_35 = q_8 - i_23;
    float2 f_36 = f_35 * f_35 * (float2(3.0f)  - float2(2.0f)  * f_35);
    int per_0 = int(4096.0f / cell_1 + 0.5f);
    int2 _S363 = int2(i_23);
    int _S364 = _S363.x % per_0;
    int _S365 = (_S364 + per_0) % per_0;
    int _S366 = _S363.y % per_0;
    int _S367 = (_S366 + per_0) % per_0;
    int2 c0_0 = int2(_S365, _S367);
    int _S368 = (_S365 + int(1)) % per_0;
    int _S369 = (_S367 + int(1)) % per_0;
    float2 _S370 = float2(f_36.x) ;
    return mix(mix(gf_hash2_0(c0_0, salt_2), gf_hash2_0(int2(_S368, _S367), salt_2), _S370), mix(gf_hash2_0(int2(_S365, _S369), salt_2), gf_hash2_0(int2(_S368, _S369), salt_2), _S370), float2(f_36.y) );
}

float2 gf_warp_f_0(float2 w_8, bool fine_0)
{
    float2 _S371 = gf_noise2_0(w_8, 64.0f, 11U);
    float2 _S372 = float2(0.5f) ;
    float2 o_1 = float2(18.0f)  * (_S371 - _S372);
    float2 o_2;
    if(fine_0)
    {
        float2 _S373 = gf_noise2_0(w_8, 16.0f, 12U);
        float2 _S374 = float2(7.0f)  * (_S373 - _S372);
        float2 _S375 = gf_noise2_0(w_8, 4.0f, 13U);
        o_2 = o_1 + (_S374 + float2(2.4000000953674316f)  * (_S375 - _S372));
    }
    else
    {
        o_2 = o_1;
    }
    return o_2;
}

float2 t_lc_sample_0(const TerrainDraw_natural_0 thread* d_34, float2 g_10)
{
    uint4 _S376 = uint4(d_34->anc_0) ;
    uint _S377 = _S376.z;
    return float2(float(_S377 & 65535U), float(_S377 >> 16U)) / float2(128.0f)  + g_10 * float2(4.0f)  * float2((as_type<float>((_S376.w)))) ;
}

float4 t_ground_raw_0(uint pool_24, uint off_5, uint i_24, uint j_14, KernelContext_0 thread* kernelContext_51)
{
    uint device* _S378 = ed_buf_0(pool_24, kernelContext_51);
    uint device* _S379 = _S378 + (off_5 + j_14 * 257U + i_24);
    return float4(float((*_S379) & 255U), float(((*_S379) >> 8U) & 255U), float(((*_S379) >> 16U) & 255U), float((*_S379) >> 24U)) / float4(255.0f) ;
}

float4 t_ground_bilin_0(uint pool_25, uint off_6, float2 s_13, KernelContext_0 thread* kernelContext_52)
{
    float2 _S380 = clamp(s_13, float2(0.0f) , float2(256.0f) );
    int2 _S381 = int2(min(floor(_S380), float2(255.0f) ));
    float2 t_8 = _S380 - float2(_S381);
    uint2 _S382 = uint2(_S381);
    uint _S383 = _S382.x;
    uint _S384 = _S382.y;
    float4 _S385 = t_ground_raw_0(pool_25, off_6, _S383, _S384, kernelContext_52);
    uint _S386 = _S383 + 1U;
    float4 _S387 = t_ground_raw_0(pool_25, off_6, _S386, _S384, kernelContext_52);
    float4 _S388 = float4(t_8.x) ;
    float4 a_11 = mix(_S385, _S387, _S388);
    uint _S389 = _S384 + 1U;
    float4 _S390 = t_ground_raw_0(pool_25, off_6, _S383, _S389, kernelContext_52);
    float4 _S391 = t_ground_raw_0(pool_25, off_6, _S386, _S389, kernelContext_52);
    return mix(a_11, mix(_S390, _S391, _S388), float4(t_8.y) );
}

float4 t_ground_shares_0(uint pool_26, const TerrainDraw_natural_0 thread* d_35, float2 g_11, KernelContext_0 thread* kernelContext_53)
{
    float2 s_14 = g_11 * float2(4.0f) ;
    uint4 _S392 = uint4(d_35->gnd_0) ;
    uint _S393 = _S392.x;
    if(_S393 != 0U)
    {
        float4 _S394 = t_ground_bilin_0(pool_26, _S393, s_14, kernelContext_53);
        return _S394;
    }
    uint _S395 = (uint4(d_35->slots_0) ).z;
    float _S396;
    if((_S395 & 16U) != 0U)
    {
        _S396 = 128.0f;
    }
    else
    {
        _S396 = 0.0f;
    }
    float _S397;
    if((_S395 & 32U) != 0U)
    {
        _S397 = 128.0f;
    }
    else
    {
        _S397 = 0.0f;
    }
    float4 _S398 = t_ground_bilin_0(pool_26, _S392.y, float2(_S396, _S397) + float2(0.5f)  * s_14, kernelContext_53);
    return _S398;
}

float4 t_ground_lc_0(uint pool_27, const TerrainDraw_natural_0 thread* d_36, float2 g_12, KernelContext_0 thread* kernelContext_54)
{
    uint _S399 = (uint4(d_36->anc_0) ).x;
    if(_S399 != 0U)
    {
        float2 _S400 = t_lc_sample_0(d_36, g_12);
        float4 _S401 = t_ground_bilin_0(pool_27, _S399, _S400, kernelContext_54);
        return _S401;
    }
    float4 _S402 = t_ground_shares_0(pool_27, d_36, g_12, kernelContext_54);
    return _S402;
}

float4 gf_understorey_0(float4 s_15, float cover_4)
{
    thread float4 _S403 = s_15;
    _S403.x = _S403.x + max(1.0f - (s_15.x + s_15.y + s_15.z + s_15.w) - cover_4, 0.0f) * smoothstep(0.0020000000949949026f, 0.019999999552965164f, cover_4);
    return _S403;
}

float gf_noise_0(float2 w_9, float cell_2, uint salt_3)
{
    float2 _S404 = gf_noise2_0(w_9, cell_2, salt_3);
    return _S404.x;
}

float gf_fringe_0(float2 w_10)
{
    float2 _S405 = gf_noise2_0(w_10, 4.0f, 13U);
    float _S406 = 0.60000002384185791f * _S405.x;
    float _S407 = gf_noise_0(w_10, 2.0f, 15U);
    return 0.15000000596046448f + 0.69999998807907104f * smoothstep(0.25f, 0.75f, _S406 + 0.40000000596046448f * _S407);
}

float gf_fringe_mask_0(float4 shares_0, float cover_5, float fringe_0)
{
    return max(smoothstep(fringe_0 - 0.20000000298023224f, fringe_0 + 0.20000000298023224f, smoothstep(0.0030000000260770321f, 0.10000000149011612f, cover_5)), smoothstep(0.05000000074505806f, 0.30000001192092896f, shares_0.x + shares_0.y + shares_0.z + shares_0.w));
}

float2 t_climate_0(const TerrainDraw_natural_0 thread* d_37, float2 g_13)
{
    thread array<float2, int(4)> c_11;
    c_11[0U] = float2(float((d_37->clim_0[0U]) & 65535U) / 65535.0f * 4.0f, -60.0f + float((d_37->clim_0[0U]) >> 16U) / 65535.0f * 100.0f);
    c_11[1U] = float2(float((d_37->clim_0[1U]) & 65535U) / 65535.0f * 4.0f, -60.0f + float((d_37->clim_0[1U]) >> 16U) / 65535.0f * 100.0f);
    c_11[2U] = float2(float((d_37->clim_0[2U]) & 65535U) / 65535.0f * 4.0f, -60.0f + float((d_37->clim_0[2U]) >> 16U) / 65535.0f * 100.0f);
    c_11[3U] = float2(float((d_37->clim_0[3U]) & 65535U) / 65535.0f * 4.0f, -60.0f + float((d_37->clim_0[3U]) >> 16U) / 65535.0f * 100.0f);
    float2 t_9 = saturate(g_13 / float2(64.0f) );
    float2 _S408 = float2(t_9.x) ;
    return mix(mix(c_11[int(0)], c_11[int(1)], _S408), mix(c_11[int(2)], c_11[int(3)], _S408), float2(t_9.y) );
}

float t_band_0(float x_3, float2 r_8, float s_16)
{
    float _S409 = r_8.x;
    float a_12;
    if(_S409 < -1.0e+08f)
    {
        a_12 = 1.0f;
    }
    else
    {
        a_12 = smoothstep(0.0f, 1.0f, (x_3 - _S409) / (2.0f * s_16) + 0.5f);
    }
    float _S410 = r_8.y;
    float b_10;
    if(_S410 > 1.0e+08f)
    {
        b_10 = 1.0f;
    }
    else
    {
        b_10 = 1.0f - smoothstep(0.0f, 1.0f, (x_3 - _S410) / (2.0f * s_16) + 0.5f);
    }
    return a_12 * b_10;
}

float3 gf_field_0(float2 w_11)
{
    float2 a8_0 = gf_noise2_0(w_11, 8.0f, 31U);
    float2 a32_0 = gf_noise2_0(w_11, 32.0f, 32U);
    float _S411 = a8_0.x;
    float nd_0 = 0.64999997615814209f * _S411 + 0.34999999403953552f * a32_0.x;
    float _S412 = gf_noise_0(w_11, 128.0f, 41U);
    float np_0 = 0.5f * _S412 + 0.33000001311302185f * a32_0.y + 0.17000000178813934f * a8_0.y;
    float _S413 = gf_noise_0(w_11, 64.0f, 51U);
    return float3(0.20000000298023224f + 0.60000002384185791f * smoothstep(0.20000000298023224f, 0.80000001192092896f, nd_0), smoothstep(0.23999999463558197f, 0.75999999046325684f, np_0), 0.5f + 0.5f * smoothstep(0.25f, 0.69999998807907104f, 0.60000002384185791f * _S413 + 0.40000000596046448f * _S411));
}

TCarpet_0 t_carpet_0(FrameData_natural_0 device* f_37, uint pool_28, const TerrainDraw_natural_0 thread* d_38, float2 g_14, float2 gw_0, float2 w_12, float far_3, float h_28, float2 cover_6, KernelContext_0 thread* kernelContext_55)
{
    uint k0_0;
    uint k1_0;
    thread TCarpet_0 o_3;
    (&o_3)->on_0 = false;
    (&o_3)->v_7 = 0.0f;
    (&o_3)->dither_0 = 0.5f;
    (&o_3)->edge_5 = 0.18000000715255737f;
    (&o_3)->mask_0 = 1.0f;
    (&o_3)->cover_3 = cover_6;
    (&o_3)->tall_0 = 0.0f;
    float3 _S414 = float3(1.0f) ;
    (&o_3)->grass_gain_0 = _S414;
    (&o_3)->soil_gain_0 = _S414;
    uint tw_0 = (as_type<uint>(((float4((&f_37->shell_0)->cells_0) ).w)));
    bool _S415;
    if(tw_0 == 0U)
    {
        _S415 = true;
    }
    else
    {
        _S415 = (((uint4(d_38->slots_0) ).z) & 33554432U) == 0U;
    }
    if(_S415)
    {
        return o_3;
    }
    (&o_3)->on_0 = true;
    packed_float4 device* _S416 = ed_buf_4(kernelContext_55->ed_c_0->t_draws_0, kernelContext_55);
    uint b_11 = tw_0 >> 2U;
    float4 _S417 = float4(*(_S416 + b_11)) ;
    float4 _S418 = float4(*(_S416 + (b_11 + 1U))) ;
    float4 _S419 = float4(*(_S416 + (b_11 + 2U))) ;
    float4 _S420 = float4(*(_S416 + (b_11 + 3U))) ;
    float4 _S421 = t_ground_lc_0(pool_28, d_38, gw_0, kernelContext_55);
    float _S422 = cover_6.x;
    float4 _S423 = gf_understorey_0(_S421, _S422);
    bool _S424 = far_3 < 1.0f;
    float v_8;
    if(_S424)
    {
        float _S425 = gf_fringe_0(w_12);
        v_8 = mix(_S425, 0.5f, far_3);
    }
    else
    {
        v_8 = 0.5f;
    }
    (&o_3)->mask_0 = gf_fringe_mask_0(_S421, _S422, v_8);
    float2 _S426 = t_climate_0(d_38, g_14);
    float _S427 = _S426.y - 0.0065000001341104507f * max(h_28, 0.0f);
    float3 _S428 = float3(0.0f) ;
    uint _S429 = min(uint(_S417.w), 15U);
    uint k0_1 = 16U;
    uint k1_1 = 0U;
    uint k_9 = 1U;
    float3 cc_1 = _S428;
    v_8 = 0.0f;
    float tl_0 = 0.0f;
    for(;;)
    {
        if(k_9 <= _S429)
        {
        }
        else
        {
            break;
        }
        uint r_9 = b_11 + 4U * k_9;
        float4 _S430 = float4(*(_S416 + r_9)) ;
        uint _S431 = min(uint(_S430.x), 3U);
        if((_S423[_S431]) <= 0.0f)
        {
            k_9 = k_9 + 1U;
            continue;
        }
        float4 _S432 = float4(*(_S416 + (r_9 + 1U))) ;
        float wr_0 = _S423[_S431] * _S430.y * (t_band_0(_S426.x, _S432.xy, 0.05000000074505806f) * t_band_0(h_28, _S432.zw, 150.0f) * t_band_0(_S427, (float4(*(_S416 + (r_9 + 2U))) ).xy, 1.5f));
        if(wr_0 > 0.0f)
        {
            k0_0 = min(k0_1, k_9);
            k1_0 = k_9;
        }
        else
        {
            k0_0 = k0_1;
            k1_0 = k1_1;
        }
        float3 cc_2 = cc_1 + float3(wr_0)  * (float4(*(_S416 + (r_9 + 3U))) ).xyz;
        float v_9 = v_8 + wr_0;
        float tl_1 = tl_0 + wr_0 * _S430.z;
        k0_1 = k0_0;
        k1_1 = k1_0;
        cc_1 = cc_2;
        v_8 = v_9;
        tl_0 = tl_1;
        k_9 = k_9 + 1U;
    }
    float3 _S433 = float3(0.0010000000474974513f) ;
    float3 _S434 = max(_S417.xyz, _S433);
    bool _S435 = v_8 > 9.9999997473787516e-05f;
    float3 carpet_1;
    if(_S435)
    {
        carpet_1 = cc_1 / float3(v_8) ;
    }
    else
    {
        carpet_1 = _S434;
    }
    float3 gfld_0;
    if(_S424)
    {
        float3 _S436 = gf_field_0(w_12);
        gfld_0 = _S436;
    }
    else
    {
        gfld_0 = float3(0.5f, 0.5f, 0.75f);
    }
    if(_S435)
    {
        _S415 = _S424;
    }
    else
    {
        _S415 = false;
    }
    if(_S415)
    {
        _S415 = k1_1 > k0_1;
    }
    else
    {
        _S415 = false;
    }
    if(_S415)
    {
        float _S437 = clamp(gfld_0.y, 0.0f, 0.99989998340606689f) * v_8;
        k0_0 = 0U;
        k_9 = k0_1;
        float at_0 = _S437;
        for(;;)
        {
            if(k_9 <= k1_1)
            {
            }
            else
            {
                break;
            }
            uint r_10 = b_11 + 4U * k_9;
            float4 _S438 = float4(*(_S416 + r_10)) ;
            uint _S439 = min(uint(_S438.x), 3U);
            if((_S423[_S439]) <= 0.0f)
            {
                k_9 = k_9 + 1U;
                continue;
            }
            float4 _S440 = float4(*(_S416 + (r_10 + 1U))) ;
            float wr_1 = _S423[_S439] * _S438.y * t_band_0(_S426.x, _S440.xy, 0.05000000074505806f) * t_band_0(h_28, _S440.zw, 150.0f) * t_band_0(_S427, (float4(*(_S416 + (r_10 + 2U))) ).xy, 1.5f);
            float at_1;
            if(wr_1 > 0.0f)
            {
                if(at_0 < wr_1)
                {
                    k0_0 = k_9;
                    break;
                }
                float at_2 = at_0 - wr_1;
                k1_0 = k_9;
                at_1 = at_2;
            }
            else
            {
                k1_0 = k0_0;
                at_1 = at_0;
            }
            k0_0 = k1_0;
            at_0 = at_1;
            k_9 = k_9 + 1U;
        }
        if(k0_0 != 0U)
        {
            carpet_1 = mix(carpet_1, (float4(*(_S416 + (b_11 + 4U * k0_0 + 3U))) ).xyz, float3((0.60000002384185791f * (1.0f - far_3))) );
        }
    }
    float3 carpet_2 = carpet_1 * float3((1.1200000047683716f - 0.30000001192092896f * mix(gfld_0.z, 0.75f, far_3))) ;
    (&o_3)->v_7 = v_8;
    (&o_3)->dither_0 = mix(gfld_0.x, 0.5f, far_3);
    (&o_3)->edge_5 = mix(0.18000000715255737f, 0.44999998807907104f, far_3);
    if(_S435)
    {
        v_8 = tl_0 / v_8;
    }
    else
    {
        v_8 = 0.0f;
    }
    (&o_3)->tall_0 = v_8;
    (&o_3)->grass_gain_0 = clamp(carpet_2 / _S434, float3(0.20000000298023224f) , float3(5.0f) );
    (&o_3)->soil_gain_0 = mix(_S414, _S419.xyz / max(_S418.xyz, _S433), float3((1.0f - smoothstep(_S419.w, _S420.w, _S426.x))) );
    return o_3;
}

struct _Array_natural_vectorx3Cfloatx2C4x3E6_0
{
    array<packed_float4, int(6)> data_6;
};

struct TerrainMat_natural_0
{
    uint albedo_3;
    uint normal_0;
    uint layers_0;
    uint size_0;
    array<float, int(8)> tile_1;
    _Array_natural_vectorx3Cfloatx2C4x3E6_0 gain_0;
    _Array_natural_vectorx3Cfloatx2C4x3E5_0 pad_1;
};

TerrainMat_natural_0 device* ed_buf_5(uint i_25, KernelContext_0 thread* kernelContext_56)
{
    return (TerrainMat_natural_0 device*)kernelContext_56->ed_buffers_0[i_25];
}

float t_page_bilin_0(uint pool_29, const TerrainDraw_natural_0 thread* d_39, float2 s_17, KernelContext_0 thread* kernelContext_57)
{
    float2 _S441 = clamp(s_17, float2(0.0f) , float2(256.0f) );
    int2 _S442 = int2(min(floor(_S441), float2(255.0f) ));
    float2 t_10 = _S441 - float2(_S442);
    int _S443 = _S442.x;
    int _S444 = _S442.y;
    float _S445 = t_page_at_0(pool_29, d_39, _S443, _S444, kernelContext_57);
    int _S446 = _S443 + int(1);
    float _S447 = t_page_at_0(pool_29, d_39, _S446, _S444, kernelContext_57);
    float _S448 = t_10.x;
    float a_13 = mix(_S445, _S447, _S448);
    int _S449 = _S444 + int(1);
    float _S450 = t_page_at_0(pool_29, d_39, _S443, _S449, kernelContext_57);
    float _S451 = t_page_at_0(pool_29, d_39, _S446, _S449, kernelContext_57);
    return mix(a_13, mix(_S450, _S451, _S448), t_10.y);
}

float length_squared_0(float3 v_10)
{
    return dot(v_10, v_10);
}

struct TSurf_0
{
    float3 albedo_4;
    float3 n_10;
    float rough_3;
    float ao_5;
    float grass_1;
};

float t_hash2_0(int2 c_12)
{
    uint h_29 = (uint(c_12.x) * 2376512323U) ^ (uint(c_12.y) * 3625334849U);
    uint h_30 = (h_29 ^ (h_29 >> 13U)) * 2246822507U;
    return float((h_30 ^ (h_30 >> 16U)) & 65535U) / 65535.0f;
}

float t_noise_0(float2 w_13, float cell_3)
{
    float2 q_9 = w_13 / float2(cell_3) ;
    float2 i_26 = floor(q_9);
    float2 f_38 = q_9 - i_26;
    float2 f_39 = f_38 * f_38 * (float2(3.0f)  - float2(2.0f)  * f_38);
    int per_1 = int(4096.0f / cell_3 + 0.5f);
    int2 _S452 = int2(i_26);
    int _S453 = _S452.x % per_1;
    int _S454 = (_S453 + per_1) % per_1;
    int _S455 = _S452.y % per_1;
    int _S456 = (_S455 + per_1) % per_1;
    int2 c0_1 = int2(_S454, _S456);
    int _S457 = (_S454 + int(1)) % per_1;
    int _S458 = (_S456 + int(1)) % per_1;
    float _S459 = f_39.x;
    return mix(mix(t_hash2_0(c0_1), t_hash2_0(int2(_S457, _S456)), _S459), mix(t_hash2_0(int2(_S454, _S458)), t_hash2_0(int2(_S457, _S458)), _S459), f_39.y);
}

float gf_grass_e_0(float v_11, float cover_7, float dither_1, float e_12)
{
    return smoothstep(dither_1 - e_12, dither_1 + e_12, smoothstep(0.079999998211860657f, 0.5f, saturate(v_11 / max(1.0f - cover_7, 0.20000000298023224f))) * (1.0f - smoothstep(0.30000001192092896f, 0.85000002384185791f, cover_7)));
}

float gf_litter_0(float cover_8)
{
    return smoothstep(0.05000000074505806f, 0.34999999403953552f, cover_8);
}

struct TProj_0
{
    float2 w_14;
    float2 dx_0;
    float2 dy_0;
};

TProj_0 TProj_x24init_0(float2 w_15, float2 dx_1, float2 dy_1)
{
    thread TProj_0 _S460;
    (&_S460)->w_14 = w_15;
    (&_S460)->dx_0 = dx_1;
    (&_S460)->dy_0 = dy_1;
    return _S460;
}

float4 t_layer_albedo_0(const TerrainMat_natural_0 thread* m_4, sampler smp_1, uint layer_0, const TProj_0 thread* p_16, float near_0, float dc_0, KernelContext_0 thread* kernelContext_58)
{
    float k_10 = 1.0f / m_4->tile_1[layer_0];
    texture2d_array<float, access::sample> _S461 = ((reinterpret_cast<device texture2d_array<float, access::sample>*>((kernelContext_58->ed_textures_0))[(m_4->albedo_3)]));
    float _S462 = p_16->w_14.x;
    float _S463 = - p_16->w_14.y;
    float2 _S464 = float2(k_10) ;
    float _S465 = float(layer_0);
    float3 _S466 = float3(float2(_S462, _S463) * _S464, _S465);
    float _S467 = p_16->dx_0.x;
    float _S468 = - p_16->dx_0.y;
    float _S469 = p_16->dy_0.x;
    float _S470 = - p_16->dy_0.y;
    float4 a_14 = ((_S461).sample((smp_1), ((_S466)).xy, uint(((_S466)).z), gradient2d((float2(_S467, _S468) * _S464), (float2(_S469, _S470) * _S464))));
    float2 _S471 = float2((0.25f * k_10)) ;
    float3 _S472 = float3(float2(_S463, - _S462) * _S471, _S465);
    float4 b_12 = ((_S461).sample((smp_1), ((_S472)).xy, uint(((_S472)).z), gradient2d((float2(_S468, - _S467) * _S471), (float2(_S470, - _S469) * _S471))));
    float mixk_0 = mix(0.34999999403953552f, 0.75f, 1.0f - near_0);
    float4 c_13 = float4(mix(a_14.xyz, b_12.xyz, float3(mixk_0) ), mix(a_14.w, b_12.w, 0.5f * mixk_0));
    float4 c_14;
    if(dc_0 < 1.0f)
    {
        float3 _S473 = float3(0.5f, 0.5f, _S465);
        c_14 = mix(((_S461).sample((smp_1), ((_S473)).xy, uint(((_S473)).z), level((16.0f)))), c_13, float4(dc_0) );
    }
    else
    {
        c_14 = c_13;
    }
    return c_14;
}

float4 t_layer_normal_0(const TerrainMat_natural_0 thread* m_5, sampler smp_2, uint layer_1, const TProj_0 thread* p_17, KernelContext_0 thread* kernelContext_59)
{
    float k_11 = 1.0f / m_5->tile_1[layer_1];
    texture2d_array<float, access::sample> _S474 = ((reinterpret_cast<device texture2d_array<float, access::sample>*>((kernelContext_59->ed_textures_0))[(m_5->normal_0)]));
    float2 _S475 = float2(k_11) ;
    float3 _S476 = float3(float2(p_17->w_14.x, - p_17->w_14.y) * _S475, float(layer_1));
    float4 b_13 = ((_S474).sample((smp_2), ((_S476)).xy, uint(((_S476)).z), gradient2d((float2(p_17->dx_0.x, - p_17->dx_0.y) * _S475), (float2(p_17->dy_0.x, - p_17->dy_0.y) * _S475))));
    return float4(b_13.xy * float2(2.0f)  - float2(1.0f) , b_13.z, b_13.w);
}

TSurf_0 terrain_material_0(FrameData_natural_0 device* f_40, const TerrainMat_natural_0 thread* m_6, const TerrainDraw_natural_0 thread* d_40, float3 n_11, float3 nm_0, float3 up_3, float h_31, float curv_0, float curv_f_0, float2 w_16, float2 dwx_0, float2 dwy_0, float dhx_0, float dhy_0, float3 tu_3, float3 tv_4, float dist_1, bool trees_0, float4 cv_0, float spacing_0, const TCarpet_0 thread* gc_0, KernelContext_0 thread* kernelContext_60)
{
    float4 _S477;
    float best_0;
    bool _S478;
    float4 nr_0;
    float3 pert_0;
    float3 nw_0;
    sampler _S479 = ((reinterpret_cast<device sampler*>((kernelContext_60->ed_samplers_0))[(kernelContext_60->ed_c_0->smp_0)]));
    float cs_0 = dot(nm_0, up_3);
    float cf_0 = dot(n_11, up_3);
    float3 _S480 = (float4(d_40->polar_0) ).xyz;
    float3 north_0 = normalize(_S480 - up_3 * float3(dot(_S480, up_3)) );
    float3 nh_0 = nm_0 - up_3 * float3(cs_0) ;
    float northness_0;
    if((length_squared_0(nh_0)) > 9.99999997475242708e-07f)
    {
        northness_0 = dot(normalize(nh_0), north_0);
    }
    else
    {
        northness_0 = 0.0f;
    }
    float n1_0 = t_noise_0(w_16, 16.0f);
    float n2_0 = t_noise_0(w_16 + float2(37.0f) , 64.0f);
    float n3_0 = t_noise_0(w_16 + float2(911.0f) , 256.0f);
    float hollow_0 = saturate(curv_0 * 60.0f);
    float hollow_f_0 = saturate(curv_f_0 * 4.0f);
    float _S481 = n3_0 - 0.5f;
    float treeline_0 = 2150.0f + 120.0f * _S481 - 100.0f * northness_0;
    float _S482 = n2_0 - 0.5f;
    float snowline_0 = 3050.0f - 230.0f * northness_0 + 200.0f * _S482;
    float alpine_0 = smoothstep(treeline_0 - 150.0f, treeline_0 + 250.0f, h_31);
    float rock_cs_0 = mix(0.68000000715255737f, 0.76999998092651367f, alpine_0);
    float steep_0 = smoothstep(rock_cs_0, rock_cs_0 - 0.14000000059604645f, cs_0);
    float _S483 = n1_0 - 0.5f;
    float rock_0 = saturate(saturate(max(steep_0, smoothstep(rock_cs_0 - 0.059999998658895493f, rock_cs_0 - 0.20000000298023224f, cf_0) * 0.80000001192092896f) + saturate(- curv_0 * 60.0f) * 0.5f * alpine_0 + smoothstep(2700.0f, 3300.0f, h_31) * 0.44999998807907104f - 0.25f * hollow_0) + _S483 * 0.5f * smoothstep(0.92000001668930054f, 0.80000001192092896f, cs_0));
    float _S484 = _S482 * 0.80000001192092896f;
    float scree_0 = saturate(smoothstep(0.97000002861022949f, 0.87999999523162842f, cs_0) * smoothstep(0.6600000262260437f, 0.80000001192092896f, cs_0) * (0.10000000149011612f + hollow_0 * 1.2000000476837158f + _S484) * mix(0.10000000149011612f, 1.0f, alpine_0) + steep_0 * 0.30000001192092896f * alpine_0);
    float forest_0 = (1.0f - alpine_0) * smoothstep(0.99000000953674316f, 0.94999998807907104f, cs_0) * saturate(0.89999997615814209f + _S481 * 1.3999999761581421f + _S484);
    float forest_1;
    if(trees_0)
    {
        float _S485 = cv_0.x;
        forest_1 = mix(forest_0, mix(_S485, smoothstep(0.029999999329447746f, 0.5f, _S485), smoothstep(80.0f, 20.0f, spacing_0)), cv_0.w);
    }
    else
    {
        forest_1 = forest_0;
    }
    float grass_2 = (1.0f - forest_1) * smoothstep(3000.0f, 2600.0f, h_31 + _S483 * 200.0f);
    float soil_0 = saturate(hollow_f_0 * 0.80000001192092896f + (n1_0 - 0.64999997615814209f) * 1.5f) * (1.0f - alpine_0 * 0.5f);
    float snow_0 = smoothstep(snowline_0 - 120.0f, snowline_0 + 120.0f, h_31 + hollow_0 * 150.0f + _S483 * 120.0f) * smoothstep(0.62000000476837158f, 0.77999997138977051f, cf_0 + _S483 * 0.10000000149011612f);
    thread array<float, int(6)> wt_0;
    wt_0[0U] = rock_0;
    wt_0[1U] = scree_0 * (1.0f - rock_0 * 0.69999998807907104f);
    float _S486 = 1.0f - rock_0;
    float _S487 = 1.0f - scree_0 * 0.80000001192092896f;
    wt_0[2U] = grass_2 * _S486 * _S487;
    wt_0[3U] = forest_1 * _S486 * _S487;
    float _S488 = 1.0f - snow_0;
    wt_0[5U] = soil_0 * 0.5f * _S486 * _S488;
    wt_0[4U] = snow_0 * 1.6000000238418579f;
    bool _S489 = gc_0->on_0;
    if(gc_0->on_0)
    {
        float cov_0 = mix(forest_0, gc_0->cover_3.x, gc_0->cover_3.y);
        float gg_0 = gf_grass_e_0(gc_0->v_7, cov_0, gc_0->dither_0, gc_0->edge_5) * gc_0->mask_0;
        float lit_2 = gf_litter_0(cov_0);
        float vs_0 = wt_0[2U] + wt_0[3U];
        wt_0[2U] = vs_0 * gg_0;
        float _S490 = vs_0 * (1.0f - gg_0);
        wt_0[3U] = _S490 * lit_2;
        wt_0[5U] = wt_0[5U] + _S490 * (1.0f - lit_2);
    }
    wt_0[0U] = wt_0[0U] * _S488;
    wt_0[1U] = wt_0[1U] * _S488;
    wt_0[2U] = wt_0[2U] * _S488;
    wt_0[3U] = wt_0[3U] * _S488;
    wt_0[5U] = wt_0[5U] * _S488;
    float fp_0 = sqrt(max(length(dwx_0) * length(dwy_0), 9.99999993922529029e-09f));
    float near_1 = 1.0f - smoothstep(0.30000001192092896f, 2.5f, fp_0);
    float _S491 = mix(1.0f, 0.69999998807907104f, smoothstep(0.039999999105930328f, 0.40000000596046448f, fp_0));
    float3 _S492 = pow(abs(float3(dot(n_11, tu_3), dot(n_11, tv_4), cf_0)), float3(4.0f) );
    thread float3 tw_1 = _S492;
    float _S493 = _S492.x;
    float _S494 = _S492.y;
    float major_0 = max(_S493, _S494) - min(_S493, _S494);
    float2 _S495;
    if(_S493 > _S494)
    {
        _S495 = float2(major_0, 0.0f);
    }
    else
    {
        _S495 = float2(0.0f, major_0);
    }
    tw_1.xy = _S495;
    float3 _S496 = float3(0.0f) ;
    float3 _S497 = max(tw_1 - float3(0.029999999329447746f) , _S496);
    tw_1 = _S497;
    tw_1.z = max(_S497.z, 0.0099999997764825821f);
    float3 _S498 = tw_1 / float3(max(tw_1.x + tw_1.y + tw_1.z, 9.9999997473787516e-05f)) ;
    tw_1 = _S498;
    TProj_0 _S499 = TProj_x24init_0(w_16, dwx_0, dwy_0);
    bool side_u_0 = (_S498.y) >= (_S498.x);
    if(side_u_0)
    {
        northness_0 = tw_1.y;
    }
    else
    {
        northness_0 = tw_1.x;
    }
    thread TProj_0 side_1;
    if(side_u_0)
    {
        _S495 = float2(w_16.x, h_31);
    }
    else
    {
        _S495 = float2(w_16.y, h_31);
    }
    (&side_1)->w_14 = _S495;
    if(side_u_0)
    {
        _S495 = float2(dwx_0.x, dhx_0);
    }
    else
    {
        _S495 = float2(dwx_0.y, dhx_0);
    }
    (&side_1)->dx_0 = _S495;
    if(side_u_0)
    {
        _S495 = float2(dwy_0.x, dhy_0);
    }
    else
    {
        _S495 = float2(dwy_0.y, dhy_0);
    }
    (&side_1)->dy_0 = _S495;
    float3 _S500;
    if(side_u_0)
    {
        _S500 = tu_3;
    }
    else
    {
        _S500 = tv_4;
    }
    thread array<float4, int(6)> ah_0;
    for(;;)
    {
        for(;;)
        {
            for(;;)
            {
                float4 _S501 = float4(0.0f) ;
                _S477 = _S501;
                ah_0[0U] = _S501;
                if((wt_0[0U]) < 0.039999999105930328f)
                {
                    wt_0[0U] = 0.0f;
                    best_0 = 0.0f;
                    break;
                }
                bool _S502 = northness_0 > 0.0f;
                if((tw_1.z) > 0.0f)
                {
                    _S478 = true;
                }
                else
                {
                    _S478 = !_S502;
                }
                if(_S478)
                {
                    thread TProj_0 _S503 = _S499;
                    float4 _S504 = t_layer_albedo_0(m_6, _S479, 0U, &_S503, near_1, _S491, kernelContext_60);
                    nr_0 = _S504;
                }
                else
                {
                    nr_0 = _S501;
                }
                if(_S502)
                {
                    float4 _S505 = nr_0 * float4(tw_1.z) ;
                    thread TProj_0 _S506 = side_1;
                    float4 _S507 = t_layer_albedo_0(m_6, _S479, 0U, &_S506, near_1, _S491, kernelContext_60);
                    nr_0 = _S505 + _S507 * float4(northness_0) ;
                }
                ah_0[0U] = nr_0;
                float _S508 = wt_0[0U] + nr_0.w * mix(0.15000000596046448f, 0.60000002384185791f, near_1);
                wt_0[0U] = _S508;
                best_0 = max(0.0f, _S508);
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                ah_0[1U] = _S477;
                if((wt_0[1U]) < 0.039999999105930328f)
                {
                    wt_0[1U] = 0.0f;
                    break;
                }
                thread TProj_0 _S509 = _S499;
                float4 _S510 = t_layer_albedo_0(m_6, _S479, 1U, &_S509, near_1, _S491, kernelContext_60);
                ah_0[1U] = _S510;
                float _S511 = wt_0[1U] + _S510.w * mix(0.15000000596046448f, 0.60000002384185791f, near_1);
                wt_0[1U] = _S511;
                best_0 = max(best_0, _S511);
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                ah_0[2U] = _S477;
                if((wt_0[2U]) < 0.039999999105930328f)
                {
                    wt_0[2U] = 0.0f;
                    break;
                }
                thread TProj_0 _S512 = _S499;
                float4 _S513 = t_layer_albedo_0(m_6, _S479, 2U, &_S512, near_1, _S491, kernelContext_60);
                ah_0[2U] = _S513;
                float _S514 = wt_0[2U] + _S513.w * mix(0.15000000596046448f, 0.60000002384185791f, near_1);
                wt_0[2U] = _S514;
                best_0 = max(best_0, _S514);
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                ah_0[3U] = _S477;
                if((wt_0[3U]) < 0.039999999105930328f)
                {
                    wt_0[3U] = 0.0f;
                    break;
                }
                thread TProj_0 _S515 = _S499;
                float4 _S516 = t_layer_albedo_0(m_6, _S479, 3U, &_S515, near_1, _S491, kernelContext_60);
                ah_0[3U] = _S516;
                float _S517 = wt_0[3U] + _S516.w * mix(0.15000000596046448f, 0.60000002384185791f, near_1);
                wt_0[3U] = _S517;
                best_0 = max(best_0, _S517);
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                ah_0[4U] = _S477;
                if((wt_0[4U]) < 0.039999999105930328f)
                {
                    wt_0[4U] = 0.0f;
                    break;
                }
                bool _S518 = northness_0 > 0.0f;
                if((tw_1.z) > 0.0f)
                {
                    _S478 = true;
                }
                else
                {
                    _S478 = !_S518;
                }
                if(_S478)
                {
                    thread TProj_0 _S519 = _S499;
                    float4 _S520 = t_layer_albedo_0(m_6, _S479, 4U, &_S519, near_1, _S491, kernelContext_60);
                    nr_0 = _S520;
                }
                else
                {
                    nr_0 = _S477;
                }
                if(_S518)
                {
                    float4 _S521 = nr_0 * float4(tw_1.z) ;
                    thread TProj_0 _S522 = side_1;
                    float4 _S523 = t_layer_albedo_0(m_6, _S479, 4U, &_S522, near_1, _S491, kernelContext_60);
                    nr_0 = _S521 + _S523 * float4(northness_0) ;
                }
                ah_0[4U] = nr_0;
                float _S524 = wt_0[4U] + nr_0.w * mix(0.15000000596046448f, 0.60000002384185791f, near_1);
                wt_0[4U] = _S524;
                best_0 = max(best_0, _S524);
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                ah_0[5U] = _S477;
                if((wt_0[5U]) < 0.039999999105930328f)
                {
                    wt_0[5U] = 0.0f;
                    break;
                }
                thread TProj_0 _S525 = _S499;
                float4 _S526 = t_layer_albedo_0(m_6, _S479, 5U, &_S525, near_1, _S491, kernelContext_60);
                ah_0[5U] = _S526;
                float _S527 = wt_0[5U] + _S526.w * mix(0.15000000596046448f, 0.60000002384185791f, near_1);
                wt_0[5U] = _S527;
                best_0 = max(best_0, _S527);
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
    float s_18;
    if((wt_0[1U]) > 0.0f)
    {
        s_18 = max(wt_0[1U] - (best_0 - 0.34999999403953552f), 0.0f);
    }
    else
    {
        s_18 = 0.0f;
    }
    wt_0[1U] = s_18;
    float total_0 = forest_1 + s_18;
    if((wt_0[2U]) > 0.0f)
    {
        float _S528 = wt_0[2U];
        if(_S489)
        {
            forest_1 = 0.60000002384185791f;
        }
        else
        {
            forest_1 = 0.34999999403953552f;
        }
        forest_1 = max(_S528 - (best_0 - forest_1), 0.0f);
    }
    else
    {
        forest_1 = 0.0f;
    }
    wt_0[2U] = forest_1;
    float total_1 = total_0 + forest_1;
    if((wt_0[3U]) > 0.0f)
    {
        float _S529 = wt_0[3U];
        if(_S489)
        {
            forest_1 = 0.60000002384185791f;
        }
        else
        {
            forest_1 = 0.34999999403953552f;
        }
        forest_1 = max(_S529 - (best_0 - forest_1), 0.0f);
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
        float _S530 = wt_0[5U];
        if(_S489)
        {
            forest_1 = 0.60000002384185791f;
        }
        else
        {
            forest_1 = 0.34999999403953552f;
        }
        forest_1 = max(_S530 - (best_0 - forest_1), 0.0f);
    }
    else
    {
        forest_1 = 0.0f;
    }
    wt_0[5U] = forest_1;
    float total_4 = total_3 + forest_1;
    thread TSurf_0 o_4;
    (&o_4)->albedo_4 = _S496;
    (&o_4)->n_10 = _S496;
    (&o_4)->rough_3 = 0.0f;
    (&o_4)->ao_5 = 0.0f;
    (&o_4)->grass_1 = 0.0f;
    for(;;)
    {
        for(;;)
        {
            for(;;)
            {
                if((wt_0[0U]) <= 0.0f)
                {
                    pert_0 = _S496;
                    break;
                }
                float a_15 = wt_0[0U] / max(total_4, 9.9999997473787516e-06f);
                float3 _S531 = float3(a_15) ;
                (&o_4)->albedo_4 = (&o_4)->albedo_4 + _S531 * ah_0[0U].xyz * (float4((&m_6->gain_0)->data_6[0U]) ).xyz;
                float4 _S532 = float4(0.0f, 0.0f, 0.92000001668930054f, 1.0f);
                if(near_1 > 0.0f)
                {
                    bool _S533 = northness_0 > 0.0f;
                    if((tw_1.z) > 0.0f)
                    {
                        _S478 = true;
                    }
                    else
                    {
                        _S478 = !_S533;
                    }
                    if(_S478)
                    {
                        thread TProj_0 _S534 = _S499;
                        float4 _S535 = t_layer_normal_0(m_6, _S479, 0U, &_S534, kernelContext_60);
                        if(_S533)
                        {
                            s_18 = tw_1.z;
                        }
                        else
                        {
                            s_18 = 1.0f;
                        }
                        float3 _S536 = (tu_3 * float3(_S535.x)  + tv_4 * float3(_S535.y) ) * float3(s_18) ;
                        nr_0 = _S535 * float4(s_18) ;
                        pert_0 = _S536;
                    }
                    else
                    {
                        nr_0 = _S477;
                        pert_0 = _S496;
                    }
                    if(_S533)
                    {
                        thread TProj_0 _S537 = side_1;
                        float4 _S538 = t_layer_normal_0(m_6, _S479, 0U, &_S537, kernelContext_60);
                        float4 nr_1 = nr_0 + _S538 * float4(northness_0) ;
                        pert_0 = pert_0 + (_S500 * float3(_S538.x)  + up_3 * float3(_S538.y) ) * float3(northness_0) ;
                        nr_0 = nr_1;
                    }
                }
                else
                {
                    pert_0 = _S496;
                    nr_0 = _S532;
                }
                float3 _S539 = _S531 * pert_0;
                (&o_4)->rough_3 = (&o_4)->rough_3 + a_15 * nr_0.z;
                (&o_4)->ao_5 = (&o_4)->ao_5 + a_15 * nr_0.w;
                pert_0 = _S539;
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
                float a_16 = wt_0[1U] / max(total_4, 9.9999997473787516e-06f);
                float3 _S540 = float3(a_16) ;
                (&o_4)->albedo_4 = (&o_4)->albedo_4 + _S540 * ah_0[1U].xyz * (float4((&m_6->gain_0)->data_6[1U]) ).xyz;
                float4 _S541 = float4(0.0f, 0.0f, 0.92000001668930054f, 1.0f);
                if(near_1 > 0.0f)
                {
                    thread TProj_0 _S542 = _S499;
                    float4 _S543 = t_layer_normal_0(m_6, _S479, 1U, &_S542, kernelContext_60);
                    nw_0 = tu_3 * float3(_S543.x)  + tv_4 * float3(_S543.y) ;
                    nr_0 = _S543;
                }
                else
                {
                    nw_0 = _S496;
                    nr_0 = _S541;
                }
                float3 pert_1 = pert_0 + _S540 * nw_0;
                (&o_4)->rough_3 = (&o_4)->rough_3 + a_16 * nr_0.z;
                (&o_4)->ao_5 = (&o_4)->ao_5 + a_16 * nr_0.w;
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
                float a_17 = wt_0[2U] / max(total_4, 9.9999997473787516e-06f);
                float3 _S544 = float3(a_17) ;
                (&o_4)->albedo_4 = (&o_4)->albedo_4 + _S544 * ah_0[2U].xyz * (float4((&m_6->gain_0)->data_6[2U]) ).xyz * gc_0->grass_gain_0;
                (&o_4)->grass_1 = a_17;
                float4 _S545 = float4(0.0f, 0.0f, 0.92000001668930054f, 1.0f);
                if(near_1 > 0.0f)
                {
                    thread TProj_0 _S546 = _S499;
                    float4 _S547 = t_layer_normal_0(m_6, _S479, 2U, &_S546, kernelContext_60);
                    nw_0 = tu_3 * float3(_S547.x)  + tv_4 * float3(_S547.y) ;
                    nr_0 = _S547;
                }
                else
                {
                    nw_0 = _S496;
                    nr_0 = _S545;
                }
                float3 pert_2 = pert_0 + _S544 * nw_0;
                (&o_4)->rough_3 = (&o_4)->rough_3 + a_17 * nr_0.z;
                (&o_4)->ao_5 = (&o_4)->ao_5 + a_17 * nr_0.w;
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
                float a_18 = wt_0[3U] / max(total_4, 9.9999997473787516e-06f);
                float3 _S548 = float3(a_18) ;
                (&o_4)->albedo_4 = (&o_4)->albedo_4 + _S548 * ah_0[3U].xyz * (float4((&m_6->gain_0)->data_6[3U]) ).xyz;
                float4 _S549 = float4(0.0f, 0.0f, 0.92000001668930054f, 1.0f);
                if(near_1 > 0.0f)
                {
                    thread TProj_0 _S550 = _S499;
                    float4 _S551 = t_layer_normal_0(m_6, _S479, 3U, &_S550, kernelContext_60);
                    nw_0 = tu_3 * float3(_S551.x)  + tv_4 * float3(_S551.y) ;
                    nr_0 = _S551;
                }
                else
                {
                    nw_0 = _S496;
                    nr_0 = _S549;
                }
                float3 pert_3 = pert_0 + _S548 * nw_0;
                (&o_4)->rough_3 = (&o_4)->rough_3 + a_18 * nr_0.z;
                (&o_4)->ao_5 = (&o_4)->ao_5 + a_18 * nr_0.w;
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
                float a_19 = wt_0[4U] / max(total_4, 9.9999997473787516e-06f);
                float3 _S552 = float3(a_19) ;
                (&o_4)->albedo_4 = (&o_4)->albedo_4 + _S552 * ah_0[4U].xyz * (float4((&m_6->gain_0)->data_6[4U]) ).xyz;
                float4 _S553 = float4(0.0f, 0.0f, 0.92000001668930054f, 1.0f);
                if(near_1 > 0.0f)
                {
                    bool _S554 = northness_0 > 0.0f;
                    if((tw_1.z) > 0.0f)
                    {
                        _S478 = true;
                    }
                    else
                    {
                        _S478 = !_S554;
                    }
                    if(_S478)
                    {
                        thread TProj_0 _S555 = _S499;
                        float4 _S556 = t_layer_normal_0(m_6, _S479, 4U, &_S555, kernelContext_60);
                        if(_S554)
                        {
                            s_18 = tw_1.z;
                        }
                        else
                        {
                            s_18 = 1.0f;
                        }
                        float3 _S557 = (tu_3 * float3(_S556.x)  + tv_4 * float3(_S556.y) ) * float3(s_18) ;
                        nr_0 = _S556 * float4(s_18) ;
                        nw_0 = _S557;
                    }
                    else
                    {
                        nr_0 = _S477;
                        nw_0 = _S496;
                    }
                    if(_S554)
                    {
                        thread TProj_0 _S558 = side_1;
                        float4 _S559 = t_layer_normal_0(m_6, _S479, 4U, &_S558, kernelContext_60);
                        float4 nr_2 = nr_0 + _S559 * float4(northness_0) ;
                        nw_0 = nw_0 + (_S500 * float3(_S559.x)  + up_3 * float3(_S559.y) ) * float3(northness_0) ;
                        nr_0 = nr_2;
                    }
                }
                else
                {
                    nw_0 = _S496;
                    nr_0 = _S553;
                }
                float3 pert_4 = pert_0 + _S552 * nw_0;
                (&o_4)->rough_3 = (&o_4)->rough_3 + a_19 * nr_0.z;
                (&o_4)->ao_5 = (&o_4)->ao_5 + a_19 * nr_0.w;
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
                float a_20 = wt_0[5U] / max(total_4, 9.9999997473787516e-06f);
                float3 _S560 = float3(a_20) ;
                (&o_4)->albedo_4 = (&o_4)->albedo_4 + _S560 * ah_0[5U].xyz * (float4((&m_6->gain_0)->data_6[5U]) ).xyz * gc_0->soil_gain_0;
                float4 _S561 = float4(0.0f, 0.0f, 0.92000001668930054f, 1.0f);
                if(near_1 > 0.0f)
                {
                    thread TProj_0 _S562 = _S499;
                    float4 _S563 = t_layer_normal_0(m_6, _S479, 5U, &_S562, kernelContext_60);
                    nw_0 = tu_3 * float3(_S563.x)  + tv_4 * float3(_S563.y) ;
                    nr_0 = _S563;
                }
                else
                {
                    nw_0 = _S496;
                    nr_0 = _S561;
                }
                float3 pert_5 = pert_0 + _S560 * nw_0;
                (&o_4)->rough_3 = (&o_4)->rough_3 + a_20 * nr_0.z;
                (&o_4)->ao_5 = (&o_4)->ao_5 + a_20 * nr_0.w;
                pert_0 = pert_5;
                break;
            }
            break;
        }
        break;
    }
    if(total_4 <= 0.0f)
    {
        (&o_4)->albedo_4 = float3(0.125f, 0.14000000059604645f, 0.070000000298023224f);
        (&o_4)->rough_3 = 0.92000001668930054f;
        (&o_4)->ao_5 = 1.0f;
    }
    float2 wq_0 = w_16 + float2(48.0f)  * float2(_S482, _S481);
    float _S564 = t_noise_0(wq_0 + float2(503.0f) , 128.0f);
    float _S565 = _S564 * 0.60000002384185791f;
    float _S566 = t_noise_0(wq_0 * float2(1.0f, 0.60000002384185791f) + float2(71.0f) , 32.0f);
    float3 hue_1 = mix(float3(0.93000000715255737f, 1.0299999713897705f, 0.97000002861022949f), float3(1.0800000429153442f, 1.0f, 0.81999999284744263f), float3(smoothstep(0.25f, 0.75f, _S565 + _S566 * 0.40000000596046448f)) );
    float3 _S567 = float3(1.0f) ;
    (&o_4)->albedo_4 = (&o_4)->albedo_4 * (mix(_S567, hue_1 / float3(dot(hue_1, float3(0.2125999927520752f, 0.71520000696182251f, 0.072200000286102295f))) , float3(_S488) ) * float3(mix(0.95999997854232788f, 1.0399999618530273f, n2_0)) ) * float3(mix(1.0f, 0.80000001192092896f, hollow_f_0 * _S488))  * mix(_S567, float3(1.0399999618530273f, 1.0f, 0.93999999761581421f), float3((n1_0 * _S488)) );
    float _S568 = wt_0[3U] / max(total_4, 9.9999997473787516e-06f) * smoothstep(60.0f, 600.0f, dist_1);
    if(trees_0)
    {
        northness_0 = 1.0f - cv_0.w;
    }
    else
    {
        northness_0 = 1.0f;
    }
    (&o_4)->albedo_4 = mix((&o_4)->albedo_4, float3(0.029999999329447746f, 0.043999999761581421f, 0.020999999716877937f) * float3(mix(0.85000002384185791f, 1.1499999761581421f, n1_0)) , float3((_S568 * northness_0)) );
    float ns_1 = mix(0.34999999403953552f, 1.0f, near_1);
    (&o_4)->n_10 = normalize(n_11 + pert_0 * float3(ns_1) );
    (&o_4)->ao_5 = mix(1.0f, (&o_4)->ao_5, ns_1);
    return o_4;
}

struct TForest_0
{
    float3 albedo_5;
    float3 n_12;
    float ao_6;
    float sun_4;
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
    thread LeafSeason_0 s_19;
    (&s_19)->leaves_0 = 1.0f;
    (&s_19)->green_0 = 1.0f;
    (&s_19)->colour_0 = 0.0f;
    (&s_19)->evergreen_0 = 0.0f;
    (&s_19)->target_0 = float3(0.0f, 0.0f, 0.0f);
    return s_19;
}

float season_ramp_0(float lo_3, float hi_0, float x_4)
{
    return smoothstep(lo_3, hi_0, x_4);
}

LeafSeason_0 season_state_0(FrameData_natural_0 device* f_41, uint row_0, float shift_0, bool south_0)
{
    if(((float4((&f_41->season_0)->cfg_4) ).x) <= 0.0f)
    {
        return season_off_0();
    }
    float d_41 = (float4((&f_41->season_0)->cfg_4) ).x;
    float d_42;
    if(south_0)
    {
        d_42 = fmod(d_41 + 182.625f, 365.25f);
    }
    else
    {
        d_42 = d_41;
    }
    float ds_0 = d_42 - shift_0;
    float da_0 = d_42 + (float4((&f_41->season_0)->cfg_4) ).y * shift_0;
    uint _S569 = (row_0 & 31U) * 2U;
    float4 _S570 = float4((&(&f_41->season_0)->rows_0)->data_5[_S569]) ;
    float4 _S571 = float4((&(&f_41->season_0)->rows_0)->data_5[_S569 + 1U]) ;
    thread LeafSeason_0 s_20;
    (&s_20)->target_0 = _S571.xyz;
    float _S572 = _S570.w;
    (&s_20)->evergreen_0 = _S572;
    if(_S572 > 0.5f)
    {
        float _S573 = _S570.y;
        float a_21 = season_ramp_0(_S573 - 10.0f, _S573 + 30.0f, da_0);
        float _S574 = _S570.x;
        float rel_1 = 1.0f - season_ramp_0(_S574 - 10.0f, _S574 + 20.0f, ds_0);
        (&s_20)->green_0 = 1.0f;
        (&s_20)->leaves_0 = 1.0f;
        (&s_20)->colour_0 = _S571.w * max(a_21, rel_1);
    }
    else
    {
        float _S575 = _S570.x;
        float _S576 = season_ramp_0(_S575 - 6.0f, _S575 + 8.0f, ds_0);
        (&s_20)->green_0 = _S576;
        float _S577 = _S570.y;
        (&s_20)->colour_0 = season_ramp_0(_S577 - 8.0f, _S577 + 14.0f, da_0);
        float _S578 = _S570.z;
        (&s_20)->leaves_0 = _S576 * (1.0f - season_ramp_0(_S578 - 10.0f, _S578 + 10.0f, da_0));
    }
    return s_20;
}

LeafSeason_0 season_far_0(FrameData_natural_0 device* f_42, bool conifer_0, float alt_m_0)
{
    float _S579;
    if(conifer_0)
    {
        _S579 = (float4((&f_42->season_0)->cfg_4) ).w;
    }
    else
    {
        _S579 = (float4((&f_42->season_0)->cfg_4) ).z;
    }
    LeafSeason_0 _S580 = season_state_0(f_42, uint(_S579), clamp(4.0f * (alt_m_0 - 500.0f) / 122.0f, -60.0f, 75.0f), false);
    return _S580;
}

float3 season_tint_0(const LeafSeason_0 thread* s_21, float3 alb_2)
{
    bool _S581;
    if((s_21->leaves_0) >= 1.0f)
    {
        _S581 = (s_21->colour_0) <= 0.0f;
    }
    else
    {
        _S581 = false;
    }
    if(_S581)
    {
        _S581 = (s_21->green_0) >= 1.0f;
    }
    else
    {
        _S581 = false;
    }
    if(_S581)
    {
        return alb_2;
    }
    float lum_0 = dot(alb_2, float3(0.2125999927520752f, 0.71520000696182251f, 0.072200000286102295f));
    float3 a_22;
    if((s_21->evergreen_0) < 0.5f)
    {
        a_22 = alb_2 * mix(float3(1.4500000476837158f, 1.2999999523162842f, 0.64999997615814209f), float3(1.0f, 1.0f, 1.0f), float3(s_21->green_0) );
    }
    else
    {
        a_22 = alb_2;
    }
    return mix(a_22, s_21->target_0 * float3(clamp(lum_0 * 5.0f, 0.44999998807907104f, 1.5f)) , float3(s_21->colour_0) );
}

float4 t_crown_0(float2 w_17, float cell_4, float pres_0, float rad_1, uint salt_4)
{
    int _S582;
    int2 _S583;
    float4 best_1;
    float2 x_5 = w_17 / float2(cell_4) ;
    float2 _S584 = floor(x_5);
    int _S585 = int(4096.0f / cell_4 + 0.5f);
    float4 _S586 = float4(0.0f, 0.0f, 9.0f, 0.0f);
    for(;;)
    {
        for(;;)
        {
            for(;;)
            {
                float2 c_15 = _S584 + float2(-1.0f, -1.0f);
                int2 _S587 = int2(c_15);
                int _S588 = _S587.x % _S585;
                int _S589 = (_S588 + _S585) % _S585;
                int _S590 = _S587.y % _S585;
                int _S591 = (_S590 + _S585) % _S585;
                int2 ci_0 = int2(_S589, _S591);
                int _S592 = int(salt_4);
                _S582 = _S592;
                int2 _S593 = int2(_S592, int(0));
                _S583 = _S593;
                if((t_hash2_0(ci_0 + _S593)) >= pres_0)
                {
                    best_1 = _S586;
                    break;
                }
                float h1_0 = t_hash2_0(ci_0 + int2(int(17), _S592 + int(5)));
                float2 _S594 = float2(0.5f) ;
                float2 q_10 = (x_5 - (c_15 + _S594 + (float2(h1_0, t_hash2_0(ci_0 + int2(_S592 + int(3), int(7)))) - _S594) * float2(0.80000001192092896f) )) / float2((rad_1 * mix(0.60000002384185791f, 1.3500000238418579f, t_hash2_0(ci_0 + int2(_S592 + int(9), int(31)))))) ;
                float dq_0 = length(q_10);
                if(dq_0 < 9.0f)
                {
                    best_1 = float4(q_10, dq_0, h1_0);
                }
                else
                {
                    best_1 = _S586;
                }
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                float2 c_16 = _S584 + float2(0.0f, -1.0f);
                int2 _S595 = int2(c_16);
                int _S596 = _S595.x % _S585;
                int _S597 = (_S596 + _S585) % _S585;
                int _S598 = _S595.y % _S585;
                int _S599 = (_S598 + _S585) % _S585;
                int2 ci_1 = int2(_S597, _S599);
                if((t_hash2_0(ci_1 + _S583)) >= pres_0)
                {
                    break;
                }
                float h1_1 = t_hash2_0(ci_1 + int2(int(17), _S582 + int(5)));
                float2 _S600 = float2(0.5f) ;
                float2 q_11 = (x_5 - (c_16 + _S600 + (float2(h1_1, t_hash2_0(ci_1 + int2(_S582 + int(3), int(7)))) - _S600) * float2(0.80000001192092896f) )) / float2((rad_1 * mix(0.60000002384185791f, 1.3500000238418579f, t_hash2_0(ci_1 + int2(_S582 + int(9), int(31)))))) ;
                float dq_1 = length(q_11);
                if(dq_1 < (best_1.z))
                {
                    best_1 = float4(q_11, dq_1, h1_1);
                }
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                float2 c_17 = _S584 + float2(1.0f, -1.0f);
                int2 _S601 = int2(c_17);
                int _S602 = _S601.x % _S585;
                int _S603 = (_S602 + _S585) % _S585;
                int _S604 = _S601.y % _S585;
                int _S605 = (_S604 + _S585) % _S585;
                int2 ci_2 = int2(_S603, _S605);
                if((t_hash2_0(ci_2 + _S583)) >= pres_0)
                {
                    break;
                }
                float h1_2 = t_hash2_0(ci_2 + int2(int(17), _S582 + int(5)));
                float2 _S606 = float2(0.5f) ;
                float2 q_12 = (x_5 - (c_17 + _S606 + (float2(h1_2, t_hash2_0(ci_2 + int2(_S582 + int(3), int(7)))) - _S606) * float2(0.80000001192092896f) )) / float2((rad_1 * mix(0.60000002384185791f, 1.3500000238418579f, t_hash2_0(ci_2 + int2(_S582 + int(9), int(31)))))) ;
                float dq_2 = length(q_12);
                if(dq_2 < (best_1.z))
                {
                    best_1 = float4(q_12, dq_2, h1_2);
                }
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                float2 c_18 = _S584 + float2(-1.0f, 0.0f);
                int2 _S607 = int2(c_18);
                int _S608 = _S607.x % _S585;
                int _S609 = (_S608 + _S585) % _S585;
                int _S610 = _S607.y % _S585;
                int _S611 = (_S610 + _S585) % _S585;
                int2 ci_3 = int2(_S609, _S611);
                if((t_hash2_0(ci_3 + _S583)) >= pres_0)
                {
                    break;
                }
                float h1_3 = t_hash2_0(ci_3 + int2(int(17), _S582 + int(5)));
                float2 _S612 = float2(0.5f) ;
                float2 q_13 = (x_5 - (c_18 + _S612 + (float2(h1_3, t_hash2_0(ci_3 + int2(_S582 + int(3), int(7)))) - _S612) * float2(0.80000001192092896f) )) / float2((rad_1 * mix(0.60000002384185791f, 1.3500000238418579f, t_hash2_0(ci_3 + int2(_S582 + int(9), int(31)))))) ;
                float dq_3 = length(q_13);
                if(dq_3 < (best_1.z))
                {
                    best_1 = float4(q_13, dq_3, h1_3);
                }
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                int2 _S613 = int2(_S584);
                int _S614 = _S613.x % _S585;
                int _S615 = (_S614 + _S585) % _S585;
                int _S616 = _S613.y % _S585;
                int _S617 = (_S616 + _S585) % _S585;
                int2 ci_4 = int2(_S615, _S617);
                if((t_hash2_0(ci_4 + _S583)) >= pres_0)
                {
                    break;
                }
                float h1_4 = t_hash2_0(ci_4 + int2(int(17), _S582 + int(5)));
                float2 _S618 = float2(0.5f) ;
                float2 q_14 = (x_5 - (_S584 + _S618 + (float2(h1_4, t_hash2_0(ci_4 + int2(_S582 + int(3), int(7)))) - _S618) * float2(0.80000001192092896f) )) / float2((rad_1 * mix(0.60000002384185791f, 1.3500000238418579f, t_hash2_0(ci_4 + int2(_S582 + int(9), int(31)))))) ;
                float dq_4 = length(q_14);
                if(dq_4 < (best_1.z))
                {
                    best_1 = float4(q_14, dq_4, h1_4);
                }
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                float2 c_19 = _S584 + float2(1.0f, 0.0f);
                int2 _S619 = int2(c_19);
                int _S620 = _S619.x % _S585;
                int _S621 = (_S620 + _S585) % _S585;
                int _S622 = _S619.y % _S585;
                int _S623 = (_S622 + _S585) % _S585;
                int2 ci_5 = int2(_S621, _S623);
                if((t_hash2_0(ci_5 + _S583)) >= pres_0)
                {
                    break;
                }
                float h1_5 = t_hash2_0(ci_5 + int2(int(17), _S582 + int(5)));
                float2 _S624 = float2(0.5f) ;
                float2 q_15 = (x_5 - (c_19 + _S624 + (float2(h1_5, t_hash2_0(ci_5 + int2(_S582 + int(3), int(7)))) - _S624) * float2(0.80000001192092896f) )) / float2((rad_1 * mix(0.60000002384185791f, 1.3500000238418579f, t_hash2_0(ci_5 + int2(_S582 + int(9), int(31)))))) ;
                float dq_5 = length(q_15);
                if(dq_5 < (best_1.z))
                {
                    best_1 = float4(q_15, dq_5, h1_5);
                }
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                float2 c_20 = _S584 + float2(-1.0f, 1.0f);
                int2 _S625 = int2(c_20);
                int _S626 = _S625.x % _S585;
                int _S627 = (_S626 + _S585) % _S585;
                int _S628 = _S625.y % _S585;
                int _S629 = (_S628 + _S585) % _S585;
                int2 ci_6 = int2(_S627, _S629);
                if((t_hash2_0(ci_6 + _S583)) >= pres_0)
                {
                    break;
                }
                float h1_6 = t_hash2_0(ci_6 + int2(int(17), _S582 + int(5)));
                float2 _S630 = float2(0.5f) ;
                float2 q_16 = (x_5 - (c_20 + _S630 + (float2(h1_6, t_hash2_0(ci_6 + int2(_S582 + int(3), int(7)))) - _S630) * float2(0.80000001192092896f) )) / float2((rad_1 * mix(0.60000002384185791f, 1.3500000238418579f, t_hash2_0(ci_6 + int2(_S582 + int(9), int(31)))))) ;
                float dq_6 = length(q_16);
                if(dq_6 < (best_1.z))
                {
                    best_1 = float4(q_16, dq_6, h1_6);
                }
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                float2 c_21 = _S584 + float2(0.0f, 1.0f);
                int2 _S631 = int2(c_21);
                int _S632 = _S631.x % _S585;
                int _S633 = (_S632 + _S585) % _S585;
                int _S634 = _S631.y % _S585;
                int _S635 = (_S634 + _S585) % _S585;
                int2 ci_7 = int2(_S633, _S635);
                if((t_hash2_0(ci_7 + _S583)) >= pres_0)
                {
                    break;
                }
                float h1_7 = t_hash2_0(ci_7 + int2(int(17), _S582 + int(5)));
                float2 _S636 = float2(0.5f) ;
                float2 q_17 = (x_5 - (c_21 + _S636 + (float2(h1_7, t_hash2_0(ci_7 + int2(_S582 + int(3), int(7)))) - _S636) * float2(0.80000001192092896f) )) / float2((rad_1 * mix(0.60000002384185791f, 1.3500000238418579f, t_hash2_0(ci_7 + int2(_S582 + int(9), int(31)))))) ;
                float dq_7 = length(q_17);
                if(dq_7 < (best_1.z))
                {
                    best_1 = float4(q_17, dq_7, h1_7);
                }
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                float2 c_22 = _S584 + float2(1.0f, 1.0f);
                int2 _S637 = int2(c_22);
                int _S638 = _S637.x % _S585;
                int _S639 = (_S638 + _S585) % _S585;
                int _S640 = _S637.y % _S585;
                int _S641 = (_S640 + _S585) % _S585;
                int2 ci_8 = int2(_S639, _S641);
                if((t_hash2_0(ci_8 + _S583)) >= pres_0)
                {
                    break;
                }
                float h1_8 = t_hash2_0(ci_8 + int2(int(17), _S582 + int(5)));
                float2 _S642 = float2(0.5f) ;
                float2 q_18 = (x_5 - (c_22 + _S642 + (float2(h1_8, t_hash2_0(ci_8 + int2(_S582 + int(3), int(7)))) - _S642) * float2(0.80000001192092896f) )) / float2((rad_1 * mix(0.60000002384185791f, 1.3500000238418579f, t_hash2_0(ci_8 + int2(_S582 + int(9), int(31)))))) ;
                float dq_8 = length(q_18);
                if(dq_8 < (best_1.z))
                {
                    best_1 = float4(q_18, dq_8, h1_8);
                }
                break;
            }
            break;
        }
        break;
    }
    return best_1;
}

TForest_0 t_forest_0(FrameData_natural_0 device* f_43, float alt_m_1, uint pool_30, const TerrainDraw_natural_0 thread* d_43, float4 cv_1, float2 g_15, float2 w_18, float3 tu_4, float3 tv_5, float3 up_4, float3 n_13, float3 floor_alb_0, float3 l_8, float px_m_0, float px_iso_0, float spacing_1, float near_px_0, KernelContext_0 thread* kernelContext_61)
{
    float _S643 = t_noise_0(w_18 + float2(911.0f) , 64.0f);
    float _S644 = _S643 * 0.64999997615814209f;
    float _S645 = t_noise_0(w_18 + float2(271.0f) , 16.0f);
    float _S646 = cv_1.x;
    float c_23 = saturate(_S646 + (_S644 + _S645 * 0.34999999403953552f - 0.5f) * 1.6000000238418579f * _S646 * (1.0f - _S646));
    float _S647 = max(cv_1.y, 2.0f);
    float mx_3 = cv_1.z;
    float3 crown_b_0;
    float3 crown_c_0;
    if(((float4((&f_43->season_0)->cfg_4) ).x) > 0.0f)
    {
        LeafSeason_0 _S648 = season_far_0(f_43, false, alt_m_1);
        LeafSeason_0 _S649 = season_far_0(f_43, true, alt_m_1);
        thread LeafSeason_0 _S650 = _S648;
        float3 _S651 = season_tint_0(&_S650, float3(0.05000000074505806f, 0.10999999940395355f, 0.024000000208616257f));
        float3 _S652 = mix(float3(0.034000001847743988f, 0.028000000864267349f, 0.02199999988079071f), _S651, float3(_S648.leaves_0) );
        thread LeafSeason_0 _S653 = _S649;
        float3 _S654 = season_tint_0(&_S653, float3(0.026000000536441803f, 0.064000003039836884f, 0.019999999552965164f));
        float3 _S655 = mix(float3(0.034000001847743988f, 0.028000000864267349f, 0.02199999988079071f), _S654, float3(_S649.leaves_0) );
        crown_b_0 = _S652;
        crown_c_0 = _S655;
    }
    else
    {
        crown_b_0 = float3(0.05000000074505806f, 0.10999999940395355f, 0.024000000208616257f);
        crown_c_0 = float3(0.026000000536441803f, 0.064000003039836884f, 0.019999999552965164f);
    }
    float3 leaf_1 = mix(crown_b_0, crown_c_0, float3(mx_3) );
    float n1_1 = t_noise_0(w_18 + float2(151.0f) , 32.0f);
    float n2_1 = t_noise_0(w_18 + float2(517.0f) , 128.0f);
    float stand_0 = mix(0.80000001192092896f, 1.2000000476837158f, n1_1) * mix(0.87999999523162842f, 1.1200000047683716f, n2_1);
    float2 sun2_0 = float2(dot(l_8, tu_4), dot(l_8, tv_5));
    float2 toward_0 = sun2_0 / float2(max(dot(l_8, up_4), 0.059999998658895493f)) ;
    float _S656 = max(_S647, 20.0f);
    float4 _S657 = t_canopy_bilin_0(pool_30, (as_type<uint>(((float4(d_43->tex_0) ).z))), g_15 * float2(4.0f)  + toward_0 * float2(_S656)  / float2(max(spacing_1, 0.0099999997764825821f)) , kernelContext_61);
    float page_sh_0 = _S657.x * smoothstep(0.40000000596046448f, 0.89999997615814209f, _S657.y / _S656);
    float3 fl_2 = mix(floor_alb_0, float3(0.05000000074505806f, 0.041999999433755875f, 0.029999999329447746f), float3((0.5f * c_23)) );
    float fl_ao_0 = mix(1.0f, 0.5f, c_23);
    float rad_2 = mix(0.41999998688697815f, 0.62000000476837158f, saturate(_S647 / 30.0f)) + 0.20000000298023224f * smoothstep(0.60000002384185791f, 1.0f, c_23);
    float pres_1 = saturate(c_23 / (3.1415901184082031f * rad_2 * rad_2));
    float pb_0 = pres_1 * (1.0f - mx_3);
    float pc_0 = pres_1 * mx_3;
    float _S658 = mix(8.0f, 5.119999885559082f, mx_3) * rad_2 * 2.0f;
    float crown_px_0 = _S658 / max(px_m_0, 0.0010000000474974513f);
    float det_1 = smoothstep(1.5f, 4.0f, crown_px_0);
    float _S659 = smoothstep(80.0f, 40.0f, _S658 / max(px_iso_0, 0.0010000000474974513f));
    bool near_on_0 = near_px_0 >= 0.0f;
    float paint_0;
    if(near_on_0)
    {
        FrameData_natural_0 device* _S660 = ed_buf_1(kernelContext_61->ed_c_0->frame_0, kernelContext_61);
        float4 _S661 = float4((&_S660->imp_0)->far_0) ;
        float _S662 = _S661.z;
        if(_S662 > 0.0f)
        {
            paint_0 = saturate((_S662 - near_px_0) / max(_S662 - _S661.w, 0.0010000000474974513f));
        }
        else
        {
            paint_0 = 1.0f - smoothstep(12.0f, 15.0f, near_px_0);
        }
    }
    else
    {
        paint_0 = _S659;
    }
    float3 _S663 = float3(stand_0) ;
    float floor_sun_far_0 = 1.0f - 0.92000001668930054f * max(c_23, page_sh_0);
    float ao_far_0 = mix(fl_ao_0, 0.85000002384185791f, c_23);
    float sun_far_0 = mix(floor_sun_far_0, 1.0f, c_23);
    thread TForest_0 o_5;
    (&o_5)->albedo_5 = mix(fl_2, leaf_1 * float3(0.81999999284744263f)  * _S663, float3(c_23) );
    (&o_5)->n_12 = n_13;
    (&o_5)->ao_6 = ao_far_0;
    (&o_5)->sun_4 = sun_far_0;
    (&o_5)->crown_0 = c_23;
    bool _S664;
    if(det_1 > 0.0f)
    {
        if(!near_on_0)
        {
            _S664 = true;
        }
        else
        {
            _S664 = paint_0 > 0.0f;
        }
    }
    else
    {
        _S664 = false;
    }
    if(_S664)
    {
        bool _S665 = pb_0 > 0.0f;
        float4 kb_0;
        if(_S665)
        {
            float4 _S666 = t_crown_0(w_18, 8.0f, pb_0, rad_2, 3U);
            kb_0 = _S666;
        }
        else
        {
            kb_0 = float4(0.0f, 0.0f, 9.0f, 0.0f);
        }
        bool _S667 = pc_0 > 0.0f;
        float4 kc_0;
        if(_S667)
        {
            float4 _S668 = t_crown_0(w_18 + float2(1000.0f) , 5.119999885559082f, pc_0, rad_2 * 0.89999997615814209f, 11U);
            kc_0 = _S668;
        }
        else
        {
            kc_0 = float4(0.0f, 0.0f, 9.0f, 0.0f);
        }
        bool conifer_1 = (kc_0.z) < (kb_0.z);
        float4 k_12;
        if(conifer_1)
        {
            k_12 = kc_0;
        }
        else
        {
            k_12 = kb_0;
        }
        float in_crown_0 = smoothstep(1.0f, 0.85000002384185791f, k_12.z);
        float3 t2_0 = tu_4 * float3(k_12.x)  + tv_5 * float3(k_12.y) ;
        float2 _S669 = k_12.xy;
        float _S670 = min(dot(_S669, _S669), 1.0f);
        float3 nc_0;
        if(conifer_1)
        {
            nc_0 = normalize(up_4 + t2_0 * float3(1.2999999523162842f) );
        }
        else
        {
            nc_0 = normalize(t2_0 + up_4 * float3(sqrt(max(1.0f - _S670, 0.039999999105930328f))) );
        }
        float3 nn_0 = normalize(n_13 + (nc_0 - up_4) * float3(0.89999997615814209f) );
        if(conifer_1)
        {
            crown_b_0 = crown_c_0;
        }
        float _S671 = k_12.w;
        float3 la_1 = crown_b_0 * float3(mix(0.75f, 1.25f, _S671))  * _S663 * mix(float3(1.1200000047683716f, 1.0f, 0.75f), float3(0.85000002384185791f, 0.97000002861022949f, 1.2000000476837158f), float3(fract(_S671 * 7.309999942779541f)) );
        float gap_sun_0;
        if(conifer_1)
        {
            gap_sun_0 = 1.6000000238418579f;
        }
        else
        {
            gap_sun_0 = 1.0f;
        }
        float _S672 = t_noise_0(w_18 * float2(gap_sun_0)  + float2(13.0f) , 1.0f);
        float3 la_2 = la_1 * float3(mix(1.0f, mix(0.69999998807907104f, 1.25f, _S672), smoothstep(6.0f, 16.0f, crown_px_0))) ;
        float crown_ao_0 = mix(0.69999998807907104f, 1.0f, 1.0f - _S670) * mix(1.0f, 0.69999998807907104f, (1.0f - saturate(0.5f + 0.80000001192092896f * dot(_S669, sun2_0 / float2(max(length(sun2_0), 0.0010000000474974513f)) ))) * 0.60000002384185791f);
        if(crown_px_0 > 3.0f)
        {
            float2 wt_1 = w_18 + toward_0 * float2(_S647)  * float2(0.55000001192092896f) ;
            float4 sb_0;
            if(_S665)
            {
                float4 _S673 = t_crown_0(wt_1, 8.0f, pb_0, rad_2, 3U);
                sb_0 = _S673;
            }
            else
            {
                sb_0 = float4(0.0f, 0.0f, 9.0f, 0.0f);
            }
            float4 sc_3;
            if(_S667)
            {
                float4 _S674 = t_crown_0(wt_1 + float2(1000.0f) , 5.119999885559082f, pc_0, rad_2 * 0.89999997615814209f, 11U);
                sc_3 = _S674;
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
        float _S675 = min(gap_sun_0, 1.0f - 0.89999997615814209f * page_sh_0);
        float closed_0 = smoothstep(0.5f, 0.94999998807907104f, c_23);
        float3 _S676 = float3(in_crown_0) ;
        float3 n_near_0 = normalize(mix(n_13, nn_0, _S676));
        float ao_near_0 = mix(mix(fl_ao_0, 0.75f, closed_0), crown_ao_0, in_crown_0);
        float sun_near_0 = mix(mix(_S675, max(_S675, 0.34999999403953552f), closed_0), 1.0f, in_crown_0);
        float3 _S677 = float3(det_1) ;
        (&o_5)->albedo_5 = mix((&o_5)->albedo_5, mix(mix(fl_2, leaf_1 * _S663 * float3(0.85000002384185791f) , float3(closed_0) ), la_2, _S676), _S677);
        (&o_5)->n_12 = normalize(mix((&o_5)->n_12, n_near_0, _S677));
        (&o_5)->ao_6 = mix((&o_5)->ao_6, ao_near_0, det_1);
        (&o_5)->sun_4 = mix((&o_5)->sun_4, sun_near_0, det_1);
        (&o_5)->crown_0 = mix((&o_5)->crown_0, in_crown_0, det_1);
    }
    if(paint_0 < 1.0f)
    {
        float _S678 = t_noise_0(w_18 + float2(77.0f) , 4.0f);
        float _S679 = _S678 * 0.60000002384185791f;
        float _S680 = t_noise_0(w_18 + float2(33.0f) , 1.0f);
        float _S681 = min(mix(1.0f, 0.40000000596046448f + 0.60000002384185791f * smoothstep(0.34999999403953552f, 0.75f, _S679 + _S680 * 0.40000000596046448f), c_23), 1.0f - 0.60000002384185791f * page_sh_0);
        float3 _S682 = float3(paint_0) ;
        (&o_5)->albedo_5 = mix(fl_2, (&o_5)->albedo_5, _S682);
        (&o_5)->n_12 = normalize(mix(n_13, (&o_5)->n_12, _S682));
        float ao_crown_0 = (&o_5)->ao_6;
        float sun_crown_0 = (&o_5)->sun_4;
        (&o_5)->ao_6 = mix(max(fl_ao_0, 0.75f), (&o_5)->ao_6, paint_0);
        (&o_5)->sun_4 = mix(_S681, (&o_5)->sun_4, paint_0);
        if(near_on_0)
        {
            (&o_5)->ao_6 = mix(max(fl_ao_0, 0.85000002384185791f), ao_crown_0, paint_0);
            (&o_5)->sun_4 = mix(1.0f - 0.5f * page_sh_0, sun_crown_0, paint_0);
        }
        (&o_5)->crown_0 = (&o_5)->crown_0 * paint_0;
    }
    return o_5;
}

float3 ground_shade_0(FrameData_natural_0 device* f_44, float3 albedo_6, float k_13, float tall_1, float2 w_19, float dist_2, float px_m_1)
{
    float m_7;
    if(((float4((&f_44->grass_0)->under_0) ).y) > 0.0f)
    {
        float _S683 = 3.0f * px_m_1;
        float a1_0 = saturate(1.0f / _S683 - 0.33000001311302185f);
        float a4_0 = saturate(4.0f / _S683 - 0.33000001311302185f);
        float a16_0 = saturate(16.0f / _S683 - 0.33000001311302185f);
        float _S684 = t_noise_0(w_19 + float2(3.0f) , 1.0f);
        float _S685 = (_S684 - 0.5f) * a1_0 * 0.69999998807907104f;
        float _S686 = t_noise_0(w_19 + float2(17.0f) , 4.0f);
        float _S687 = _S685 + (_S686 - 0.5f) * a4_0 * 0.80000001192092896f;
        float _S688 = t_noise_0(w_19 + float2(101.0f) , 16.0f);
        m_7 = 1.0f + (float4((&f_44->grass_0)->under_0) ).y * (_S687 + (_S688 - 0.5f) * a16_0 * 0.60000002384185791f);
    }
    else
    {
        m_7 = 1.0f;
    }
    bool _S689;
    if(((float4((&f_44->grass_0)->under_0) ).x) > 0.0f)
    {
        _S689 = (((uint4((&f_44->grass_0)->bufs_2) ).w) & 1U) != 0U;
    }
    else
    {
        _S689 = false;
    }
    if(_S689)
    {
        float R_0 = (float4((&f_44->grass_0)->lod_0) ).y;
        m_7 = m_7 * (1.0f - (float4((&f_44->grass_0)->under_0) ).x * (1.0f - smoothstep(0.10000000149011612f * R_0, 0.60000002384185791f * R_0, dist_2)) * mix(0.25f, 1.0f, tall_1));
    }
    return albedo_6 * float3(mix(1.0f, m_7, k_13)) ;
}

bool shell_face_0(FrameData_natural_0 device* f_45, const TerrainDraw_natural_0 thread* d_44)
{
    bool _S690;
    if((((uint4((&f_45->shell_0)->cfg_5) ).x) & 4U) != 0U)
    {
        _S690 = (dot((float4(d_44->fu_0) ).xyz, (float4((&f_45->shell_0)->fu_1) ).xyz)) > 0.99900001287460327f;
    }
    else
    {
        _S690 = false;
    }
    return _S690;
}

float tint_noise_0(float2 wr_2, float s_22, int2 base_1, uint salt_5)
{
    float2 q_19 = wr_2 / float2(s_22) ;
    float2 fq_0 = floor(q_19);
    float2 t_11 = q_19 - fq_0;
    float2 t_12 = t_11 * t_11 * (float2(3.0f)  - float2(2.0f)  * t_11);
    int2 c_24 = int2(fq_0) + base_1;
    int _S691 = c_24.x;
    uint _S692 = uint(_S691) * 2376512323U;
    int _S693 = c_24.y;
    uint _S694 = uint(_S693) * 3625334849U;
    uint _S695 = uint(_S691 + int(1)) * 2376512323U;
    uint _S696 = uint(_S693 + int(1)) * 3625334849U;
    float _S697 = t_12.x;
    return mix(mix(grass_rand_0(grass_hash_0((_S692 ^ _S694) ^ salt_5)), grass_rand_0(grass_hash_0((_S695 ^ _S694) ^ salt_5)), _S697), mix(grass_rand_0(grass_hash_0((_S692 ^ _S696) ^ salt_5)), grass_rand_0(grass_hash_0((_S695 ^ _S696) ^ salt_5)), _S697), t_12.y);
}

float3 shell_tint_0(FrameData_natural_0 device* f_46, const TerrainDraw_natural_0 thread* d_45, float2 g_16, float3 albedo_7, float dist_3, float px_m_2, KernelContext_0 thread* kernelContext_62)
{
    float3 carpet_3;
    float2 _S698 = shell_wrel_0(f_46, d_45, g_16);
    float cm_0 = (float4((&f_46->shell_0)->cells_0) ).y;
    float2 q_20 = _S698 / float2(cm_0)  - float2(0.5f) ;
    float2 fq_1 = floor(q_20);
    float2 _S699 = q_20 - fq_1;
    int2 _S700 = int2(fq_1);
    float3 _S701 = float3(0.0f) ;
    packed_float4 device* _S702 = ed_buf_4(kernelContext_62->ed_c_0->t_draws_0, kernelContext_62);
    thread array<uint, int(4)> ws_0;
    uint c_25 = 0U;
    for(;;)
    {
        if(c_25 < 4U)
        {
        }
        else
        {
            break;
        }
        uint _S703 = shell_map_0(f_46, kernelContext_62->ed_c_0->t_draws_0, 1U, _S700 + int2(int(c_25 & 1U), int(c_25 >> 1U)), kernelContext_62);
        ws_0[c_25] = _S703;
        c_25 = c_25 + 1U;
    }
    uint k0_2 = (ws_0[int(0)]) & 255U;
    float3 carpet0_0;
    float tall0_0;
    if(k0_2 != 0U)
    {
        uint r_11 = ((uint4((&f_46->shell_0)->cfg_5) ).y + k0_2 - 1U) * 8U;
        float _S704 = smoothstep(0.11999999731779099f, 0.5f, (float4(*(_S702 + r_11)) ).y);
        carpet0_0 = (float4(*(_S702 + (r_11 + 7U))) ).xyz;
        tall0_0 = _S704;
    }
    else
    {
        carpet0_0 = _S701;
        tall0_0 = 0.0f;
    }
    c_25 = 0U;
    float3 cc_3 = _S701;
    float cov_1 = 0.0f;
    float tall_2 = 0.0f;
    for(;;)
    {
        if(c_25 < 4U)
        {
        }
        else
        {
            break;
        }
        int _S705 = int(c_25 >> 1U);
        float _S706;
        if(int(c_25 & 1U) != int(0))
        {
            _S706 = _S699.x;
        }
        else
        {
            _S706 = 1.0f - _S699.x;
        }
        float _S707;
        if(_S705 != int(0))
        {
            _S707 = _S699.y;
        }
        else
        {
            _S707 = 1.0f - _S699.y;
        }
        float wb_0 = _S706 * _S707;
        uint w_20 = ws_0[c_25];
        uint kk_0 = (ws_0[c_25]) & 255U;
        if(kk_0 != 0U)
        {
            float dens_1 = float((w_20 >> 8U) & 255U) / 255.0f;
            float tl_2;
            if(kk_0 != k0_2)
            {
                uint r_12 = ((uint4((&f_46->shell_0)->cfg_5) ).y + kk_0 - 1U) * 8U;
                float _S708 = smoothstep(0.11999999731779099f, 0.5f, (float4(*(_S702 + r_12)) ).y);
                carpet_3 = (float4(*(_S702 + (r_12 + 7U))) ).xyz;
                tl_2 = _S708;
            }
            else
            {
                carpet_3 = carpet0_0;
                tl_2 = tall0_0;
            }
            float _S709 = wb_0 * dens_1;
            float cov_2 = cov_1 + _S709;
            float tall_3 = tall_2 + tl_2 * _S709;
            cc_3 = cc_3 + carpet_3 * float3(_S709) ;
            cov_1 = cov_2;
            tall_2 = tall_3;
        }
        c_25 = c_25 + 1U;
    }
    if(cov_1 <= 0.0f)
    {
        return albedo_7;
    }
    float3 tgt_0 = cc_3 / float3(cov_1) ;
    float edge_6 = 1.0f - smoothstep(0.80000001192092896f, 0.97000002861022949f, max(abs(_S698.x), abs(_S698.y)) / (cm_0 * (float4((&f_46->shell_0)->cells_0) ).z * 0.5f));
    float3 LUM_0 = float3(0.2125999927520752f, 0.71520000696182251f, 0.072200000286102295f);
    bool modulate_0 = ((float4((&f_46->grass_0)->under_0) ).z) > 0.5f;
    if(!modulate_0)
    {
        carpet_3 = tgt_0 * float3(clamp(dot(albedo_7, LUM_0) / max(dot(tgt_0, LUM_0), 9.9999997473787516e-05f), 0.75f, 1.25f)) ;
    }
    else
    {
        carpet_3 = tgt_0;
    }
    if(((float4((&f_46->grass_0)->under_0) ).y) > 0.0f)
    {
        float _S710 = 3.0f * px_m_2;
        carpet_3 = carpet_3 * float3((1.0f + (float4((&f_46->grass_0)->under_0) ).y * ((tint_noise_0(_S698, 1.0f, int2(round((float4((&f_46->shell_0)->c_0) ).xy)), 81U) - 0.5f) * saturate(1.0f / _S710 - 0.33000001311302185f) * 0.69999998807907104f + (tint_noise_0(_S698, 4.0f, int2(round((float4((&f_46->shell_0)->c_0) ).xy / float2(4.0f) )), 147U) - 0.5f) * saturate(4.0f / _S710 - 0.33000001311302185f) * 0.80000001192092896f + (tint_noise_0(_S698, 16.0f, int2(round((float4((&f_46->shell_0)->c_0) ).xy / float2(16.0f) )), 45U) - 0.5f) * saturate(16.0f / _S710 - 0.33000001311302185f) * 0.60000002384185791f))) ;
    }
    bool _S711;
    if(((float4((&f_46->grass_0)->under_0) ).x) > 0.0f)
    {
        _S711 = (((uint4((&f_46->grass_0)->bufs_2) ).w) & 1U) != 0U;
    }
    else
    {
        _S711 = false;
    }
    if(_S711)
    {
        float R_1 = (float4((&f_46->grass_0)->lod_0) ).y;
        carpet_3 = carpet_3 * float3((1.0f - (float4((&f_46->grass_0)->under_0) ).x * (1.0f - smoothstep(0.10000000149011612f * R_1, 0.60000002384185791f * R_1, dist_3)) * mix(0.25f, 1.0f, tall_2 / cov_1))) ;
    }
    if(modulate_0)
    {
        return albedo_7 * mix(float3(1.0f, 1.0f, 1.0f), clamp(carpet_3 / float3(0.10999999940395355f, 0.1550000011920929f, 0.05000000074505806f), float3(0.30000001192092896f, 0.30000001192092896f, 0.30000001192092896f), float3(4.0f, 4.0f, 4.0f)), float3((saturate((float4((&f_46->shell_0)->c_0) ).w * cov_1 * edge_6) * mix(0.60000002384185791f, 1.0f, smoothstep(5.0f, 60.0f, dist_3)))) );
    }
    return mix(albedo_7, carpet_3, float3(saturate((float4((&f_46->shell_0)->c_0) ).w * cov_1 * edge_6)) );
}

float3 shade_pbr_k_1(FrameData_natural_0 device* f_47, float3 sun_5, float3 albedo_8, float metal_1, float rough_4, float ao_7, float sv_4, float sun_vis_2, float3 n_14, float3 v_12, float spec_k_2, KernelContext_0 thread* kernelContext_63)
{
    float _S712 = saturate(metal_1);
    float _S713 = clamp(rough_4, 0.039999999105930328f, 1.0f);
    float a_23 = _S713 * _S713;
    float3 l_9 = (float4(f_47->sun_dir_0) ).xyz;
    float3 h_32 = normalize(l_9 + v_12);
    float nol_3 = saturate(dot(n_14, l_9));
    float _S714 = max(dot(n_14, v_12), 9.9999997473787516e-05f);
    float3 f0_2 = mix(float3(0.039999999105930328f) , albedo_8, float3(_S712) );
    float3 diff_col_1 = albedo_8 * float3((1.0f - _S712)) ;
    float3 F_1 = f_schlick_0(f0_2, saturate(dot(v_12, h_32)));
    float3 _S715 = float3(spec_k_2) ;
    float3 _S716 = float3(1.0f) ;
    float3 direct_1 = (diff_col_1 * (_S716 - F_1) / float3(3.1415927410125732f)  + float3((d_ggx_0(saturate(dot(n_14, h_32)), a_23) * v_smith_0(_S714, nol_3, a_23)))  * F_1 * _S715) * sun_5 * float3((nol_3 * sun_vis_2)) ;
    float2 ab_1 = env_brdf_0(_S714, _S713);
    float3 r_13 = reflect(- v_12, n_14);
    float3 env_1;
    if(((uint4((&f_47->light_0)->flags_0) ).z) != 0U)
    {
        float3 _S717 = sky_env_0(f_47, r_13, kernelContext_63);
        float3 _S718 = sh_irradiance_0(f_47, r_13, kernelContext_63);
        env_1 = mix(_S717, _S718, float3(_S713) );
    }
    else
    {
        float3 _S719 = hemi_ambient_0(f_47, r_13);
        env_1 = _S719;
    }
    float3 _S720 = occluded_ambient_0(f_47, env_1, sv_4, kernelContext_63);
    float spec_occ_1 = saturate(pow(_S714 + ao_7, exp2(-16.0f * a_23 - 1.0f)) - 1.0f + ao_7);
    float3 f_env_1 = f0_2 * float3(ab_1.x)  + float3(ab_1.y) ;
    float3 _S721 = diff_col_1 * (_S716 - f_env_1);
    float3 _S722 = ambient_light_0(f_47, n_14, kernelContext_63);
    float3 _S723 = occluded_ambient_0(f_47, _S722, sv_4, kernelContext_63);
    float3 _S724 = _S721 * _S723;
    float3 _S725 = ao_multibounce_0(f_47, ao_7, diff_col_1);
    return direct_1 + (_S724 * _S725 + _S720 * f_env_1 * float3(spec_occ_1)  * _S715);
}

float3 shade_pbr_0(FrameData_natural_0 device* f_48, float3 sun_6, float3 albedo_9, float metal_2, float rough_5, float ao_8, float sv_5, float sun_vis_3, float3 n_15, float3 v_13, KernelContext_0 thread* kernelContext_64)
{
    float3 _S726 = shade_pbr_k_1(f_48, sun_6, albedo_9, metal_2, rough_5, ao_8, sv_5, sun_vis_3, n_15, v_13, 1.0f, kernelContext_64);
    return _S726;
}

uint shadow_cascade_0(FrameData_natural_0 device* f_49, float3 p_18, float dist_4)
{
    uint _S727 = uint((float4((&f_49->light_0)->cfg_0) ).x);
    bool _S728;
    if(((uint4((&f_49->light_0)->flags_0) ).x) == 0U)
    {
        _S728 = true;
    }
    else
    {
        _S728 = dist_4 >= ((float4((&f_49->light_0)->filter_0) ).y);
    }
    if(_S728)
    {
        return 4U;
    }
    float _S729 = 1.0f - (float4((&f_49->light_0)->cfg_0) ).w * 2.0f / (float4((&f_49->light_0)->cfg_0) ).y;
    uint c_26 = 0U;
    for(;;)
    {
        if(c_26 < _S727)
        {
        }
        else
        {
            break;
        }
        float _S730 = cascade_extent_0(f_49, c_26, p_18);
        if((_S730 / _S729) <= 1.0f)
        {
            return c_26;
        }
        c_26 = c_26 + 1U;
    }
    return 4U;
}

float4 terrain_pixel_0(FrameData_natural_0 device* f_50, const FsOut_0 thread* i_27, uint2 vis_2, float3 dir_11, KernelContext_0 thread* kernelContext_65)
{
    uint _S731 = vis_2.y;
    if((_S731 & 2147483648U) != 0U)
    {
        float4 _S732 = shell_pixel_0(f_50, i_27, vis_2, dir_11, kernelContext_65);
        return _S732;
    }
    TerrainDraw_natural_0 device* _S733 = ed_buf_2(kernelContext_65->ed_c_0->t_draws_0, kernelContext_65);
    TerrainDraw_natural_0 device* _S734 = _S733 + ((vis_2.x) & 2147483647U);
    TerrainDraw_natural_0 d_46 = *_S734;
    thread TerrainDraw_natural_0 _S735 = *_S734;
    uint pool_31 = kernelContext_65->ed_c_0->t_pool_0;
    uint3 tri_1 = t_triangle_0(_S731 & 268435455U);
    uint _S736 = tri_1.x;
    _S735 = d_46;
    float3 _S737 = t_vertex_0(pool_31, &_S735, _S736, kernelContext_65);
    uint _S738 = tri_1.y;
    float3 _S739 = t_vertex_0(pool_31, &_S735, _S738, kernelContext_65);
    float3 e1_2 = _S739 - _S737;
    uint _S740 = tri_1.z;
    float3 _S741 = t_vertex_0(pool_31, &_S735, _S740, kernelContext_65);
    float3 e2_2 = _S741 - _S737;
    float2 bb_1 = ray_bary_0(dir_11, _S737, e1_2, e2_2);
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
    float3 p_19 = _S737 + e1_2 * float3(b1_3)  + e2_2 * float3(b2_4) ;
    float2 g_17 = t_vertex_grid_0(_S736) * float2((1.0f - b1_3 - b2_4))  + t_vertex_grid_0(_S738) * float2(b1_3)  + t_vertex_grid_0(_S740) * float2(b2_4) ;
    float _S742 = t_sample_spacing_0(&_S735, g_17);
    float _S743 = max(_S742, 0.0099999997764825821f);
    int rn_0 = clamp(int(1.2000000476837158f / _S743 + 0.5f), int(1), int(4));
    thread float3 up_5;
    thread float h_33;
    float3 _S744 = t_smooth_normal_0(pool_31, &_S735, g_17, rn_0, &up_5, &h_33, kernelContext_65);
    int r_14 = clamp(int(24.0f / _S743 + 0.5f), int(1), int(6));
    float3 nm_1;
    if(r_14 == rn_0)
    {
        nm_1 = _S744;
    }
    else
    {
        float3 _S745 = t_smooth_normal_0(pool_31, &_S735, g_17, r_14, &up_5, &h_33, kernelContext_65);
        nm_1 = _S745;
    }
    float3 _S746 = terrain_albedo_0(h_33, dot(nm_1, up_5));
    uint _S747 = (uint4((&_S735)->slots_0) ).z;
    bool _S748 = (_S747 & 8388608U) != 0U;
    float4 _S749 = float4(0.0f) ;
    bool _S750;
    if(_S748)
    {
        _S750 = (as_type<uint>(((float4((&_S735)->tex_0) ).z))) != 0U;
    }
    else
    {
        _S750 = false;
    }
    float4 cv_2;
    if(_S750)
    {
        float4 _S751 = t_canopy_bilin_0(pool_31, (as_type<uint>(((float4((&_S735)->tex_0) ).z))), g_17 * float2(4.0f) , kernelContext_65);
        cv_2 = _S751;
    }
    else
    {
        cv_2 = _S749;
    }
    float4 _S752 = float4((&_S735)->shape_0) ;
    float4 _S753 = float4((&_S735)->tex_0) ;
    float2 _S754 = float2((_S752.x * _S752.y)) ;
    float2 w0_0 = _S753.xy + (g_17 - float2(32.0f) ) * _S754;
    thread TCarpet_0 gc_1;
    (&gc_1)->on_0 = false;
    (&gc_1)->v_7 = 0.0f;
    (&gc_1)->dither_0 = 0.5f;
    (&gc_1)->edge_5 = 0.18000000715255737f;
    (&gc_1)->mask_0 = 1.0f;
    float2 _S755 = float2(0.0f) ;
    (&gc_1)->cover_3 = _S755;
    (&gc_1)->tall_0 = 0.0f;
    float3 _S756 = float3(1.0f) ;
    (&gc_1)->grass_gain_0 = _S756;
    (&gc_1)->soil_gain_0 = _S756;
    if((_S747 & 33554432U) != 0U)
    {
        float2 _S757 = i_27->pos_0.xy;
        float3 _S758 = view_dir_0(f_50, _S757);
        float3 _S759 = view_dir_0(f_50, _S757 + float2(1.0f, 0.0f));
        float far_4 = smoothstep(0.75f, 3.0f, length(p_19) * length(_S759 - _S758) / sqrt(max(abs(dot(_S758, up_5)), 0.05000000074505806f)));
        bool _S760 = far_4 < 1.0f;
        float2 _S761 = gf_warp_f_0(w0_0, _S760);
        float2 gw_1 = g_17 + _S761 / _S754;
        thread float2 cov_3 = _S755;
        uint _S762 = (uint4((&_S735)->anc_0) ).y;
        if(_S762 != 0U)
        {
            float2 _S763 = t_lc_sample_0(&_S735, gw_1);
            float4 _S764 = t_canopy_bilin_0(pool_31, _S762, _S763, kernelContext_65);
            cov_3 = _S764.xw;
            if(_S760)
            {
                float2 _S765 = t_lc_sample_0(&_S735, gw_1 + float2(4.0f, 2.0f) / _S754);
                float4 _S766 = t_canopy_bilin_0(pool_31, _S762, _S765, kernelContext_65);
                float _S767 = cov_3.x + _S766.x;
                float2 _S768 = t_lc_sample_0(&_S735, gw_1 + float2(-2.0f, 4.0f) / _S754);
                float4 _S769 = t_canopy_bilin_0(pool_31, _S762, _S768, kernelContext_65);
                float _S770 = _S767 + _S769.x;
                float2 _S771 = t_lc_sample_0(&_S735, gw_1 + float2(-4.0f, -2.0f) / _S754);
                float4 _S772 = t_canopy_bilin_0(pool_31, _S762, _S771, kernelContext_65);
                float _S773 = _S770 + _S772.x;
                float2 _S774 = t_lc_sample_0(&_S735, gw_1 + float2(2.0f, -4.0f) / _S754);
                float4 _S775 = t_canopy_bilin_0(pool_31, _S762, _S774, kernelContext_65);
                cov_3.x = (_S773 + _S775.x) * 0.20000000298023224f;
            }
        }
        else
        {
            if(_S748)
            {
                _S750 = (as_type<uint>((_S753.z))) != 0U;
            }
            else
            {
                _S750 = false;
            }
            if(_S750)
            {
                uint _S776 = (as_type<uint>((_S753.z)));
                float2 _S777 = float2(4.0f) ;
                float4 _S778 = t_canopy_bilin_0(pool_31, _S776, gw_1 * _S777, kernelContext_65);
                cov_3 = _S778.xw;
                if(_S760)
                {
                    float4 _S779 = t_canopy_bilin_0(pool_31, _S776, (gw_1 + float2(4.0f, 2.0f) / _S754) * _S777, kernelContext_65);
                    float _S780 = cov_3.x + _S779.x;
                    float4 _S781 = t_canopy_bilin_0(pool_31, _S776, (gw_1 + float2(-2.0f, 4.0f) / _S754) * _S777, kernelContext_65);
                    float _S782 = _S780 + _S781.x;
                    float4 _S783 = t_canopy_bilin_0(pool_31, _S776, (gw_1 + float2(-4.0f, -2.0f) / _S754) * _S777, kernelContext_65);
                    float _S784 = _S782 + _S783.x;
                    float4 _S785 = t_canopy_bilin_0(pool_31, _S776, (gw_1 + float2(2.0f, -4.0f) / _S754) * _S777, kernelContext_65);
                    cov_3.x = (_S784 + _S785.x) * 0.20000000298023224f;
                }
            }
        }
        TCarpet_0 _S786 = t_carpet_0(f_50, pool_31, &_S735, g_17, gw_1, w0_0, far_4, h_33, cov_3, kernelContext_65);
        gc_1 = _S786;
    }
    float rough_6;
    float mat_ao_0;
    float grass_k_0;
    float3 n_16;
    float3 albedo_10;
    if((kernelContext_65->ed_c_0->t_mat_0) != 0U)
    {
        TerrainMat_natural_0 device* _S787 = ed_buf_5(kernelContext_65->ed_c_0->t_draws_0, kernelContext_65);
        thread TerrainMat_natural_0 _S788 = *(_S787 + kernelContext_65->ed_c_0->t_mat_0);
        float3 _S789 = (float4((&_S735)->fu_0) ).xyz;
        float3 tu_5 = normalize(_S789 - up_5 * float3(dot(_S789, up_5)) );
        float3 _S790 = (float4((&_S735)->fv_0) ).xyz;
        float3 tv_6 = normalize(_S790 - up_5 * float3(dot(_S790, up_5)) );
        float2 _S791 = i_27->pos_0.xy;
        float3 _S792 = view_dir_0(f_50, _S791 + float2(1.0f, 0.0f));
        float2 bx_0 = ray_bary_0(_S792, _S737, e1_2, e2_2);
        float3 _S793 = view_dir_0(f_50, _S791 + float2(0.0f, 1.0f));
        float2 by_0 = ray_bary_0(_S793, _S737, e1_2, e2_2);
        float3 dpx_0 = _S737 + e1_2 * float3(bx_0.x)  + e2_2 * float3(bx_0.y)  - p_19;
        float3 dpy_0 = _S737 + e1_2 * float3(by_0.x)  + e2_2 * float3(by_0.y)  - p_19;
        float2 dwx_1 = float2(dot(dpx_0, tu_5), dot(dpx_0, tv_6));
        float2 dwy_1 = float2(dot(dpy_0, tu_5), dot(dpy_0, tv_6));
        float2 sp_1 = g_17 * float2(4.0f) ;
        int rf_0 = clamp(int(2.0f / _S743 + 0.5f), int(1), int(4));
        float _S794 = float(clamp(int(8.0f / _S743 + 0.5f), int(1), int(16)));
        float2 _S795 = float2(_S794, 0.0f);
        float _S796 = t_page_bilin_0(pool_31, &_S735, sp_1 + _S795, kernelContext_65);
        float _S797 = t_page_bilin_0(pool_31, &_S735, sp_1 - _S795, kernelContext_65);
        float _S798 = _S796 + _S797;
        float2 _S799 = float2(0.0f, _S794);
        float _S800 = t_page_bilin_0(pool_31, &_S735, sp_1 + _S799, kernelContext_65);
        float _S801 = _S798 + _S800;
        float _S802 = t_page_bilin_0(pool_31, &_S735, sp_1 - _S799, kernelContext_65);
        float lap_0 = _S801 + _S802 - 4.0f * h_33;
        if((length_squared_0(p_19)) < 1.6e+05f)
        {
            float _S803 = float(rf_0);
            float2 _S804 = float2(_S803, 0.0f);
            float _S805 = t_page_bilin_0(pool_31, &_S735, sp_1 + _S804, kernelContext_65);
            float _S806 = t_page_bilin_0(pool_31, &_S735, sp_1 - _S804, kernelContext_65);
            float _S807 = _S805 + _S806;
            float2 _S808 = float2(0.0f, _S803);
            float _S809 = t_page_bilin_0(pool_31, &_S735, sp_1 + _S808, kernelContext_65);
            float _S810 = _S807 + _S809;
            float _S811 = t_page_bilin_0(pool_31, &_S735, sp_1 - _S808, kernelContext_65);
            rough_6 = _S810 + _S811 - 4.0f * h_33;
        }
        else
        {
            rough_6 = 0.0f;
        }
        float dl_1 = _S794 * _S743;
        float df_0 = float(rf_0) * _S743;
        float _S812 = lap_0 / (dl_1 * dl_1);
        float _S813 = rough_6 / (df_0 * df_0);
        float _S814 = dot(dpx_0, up_5);
        float _S815 = dot(dpy_0, up_5);
        float _S816 = length(p_19);
        thread TCarpet_0 _S817 = gc_1;
        TSurf_0 _S818 = terrain_material_0(f_50, &_S788, &_S735, _S744, nm_1, up_5, h_33, _S812, _S813, w0_0, dwx_1, dwy_1, _S814, _S815, tu_5, tv_6, _S816, _S748, cv_2, _S743, &_S817, kernelContext_65);
        n_16 = _S818.n_10;
        albedo_10 = _S818.albedo_4;
        rough_6 = _S818.rough_3;
        mat_ao_0 = _S818.ao_5;
        grass_k_0 = _S818.grass_1;
    }
    else
    {
        n_16 = _S744;
        albedo_10 = _S746;
        rough_6 = 0.92000001668930054f;
        mat_ao_0 = 1.0f;
        grass_k_0 = 0.0f;
    }
    if(_S748)
    {
        _S750 = (cv_2.x) > 0.0040000001899898052f;
    }
    else
    {
        _S750 = false;
    }
    float sun_k_0;
    if(_S750)
    {
        float3 _S819 = (float4((&_S735)->fu_0) ).xyz;
        float3 tu_6 = normalize(_S819 - up_5 * float3(dot(_S819, up_5)) );
        float3 _S820 = (float4((&_S735)->fv_0) ).xyz;
        float3 tv_7 = normalize(_S820 - up_5 * float3(dot(_S820, up_5)) );
        float3 _S821 = view_dir_0(f_50, i_27->pos_0.xy + float2(1.0f, 0.0f));
        float _S822 = length(p_19);
        float px_iso_1 = _S822 * length(_S821 - dir_11);
        float px_m_3 = px_iso_1 / sqrt(max(abs(dot(dir_11, n_16)), 0.029999999329447746f));
        if(((float4((&f_50->wind_1)->cfg_2) ).z) > 0.5f)
        {
            _S750 = ((float4((&_S735)->polar_0) ).w) > 0.0f;
        }
        else
        {
            _S750 = false;
        }
        if(_S750)
        {
            sun_k_0 = (float4((&_S735)->polar_0) ).w / max(_S822, 1.0f);
        }
        else
        {
            sun_k_0 = -1.0f;
        }
        TForest_0 _S823 = t_forest_0(f_50, h_33, pool_31, &_S735, cv_2, g_17, w0_0, tu_6, tv_7, up_5, n_16, albedo_10, (float4(f_50->sun_dir_0) ).xyz, px_m_3, px_iso_1, _S743, sun_k_0, kernelContext_65);
        float _S824 = mix(rough_6, 0.80000001192092896f, _S823.crown_0);
        float mat_ao_1 = mat_ao_0 * _S823.ao_6;
        n_16 = _S823.n_12;
        albedo_10 = _S823.albedo_5;
        sun_k_0 = _S823.sun_4;
        mat_ao_0 = mat_ao_1;
        rough_6 = _S824;
    }
    else
    {
        sun_k_0 = 1.0f;
    }
    if((&gc_1)->on_0)
    {
        if(grass_k_0 > 0.0f)
        {
            float3 _S825 = view_dir_0(f_50, i_27->pos_0.xy + float2(1.0f, 0.0f));
            float _S826 = length(p_19);
            float3 _S827 = ground_shade_0(f_50, albedo_10, grass_k_0, (&gc_1)->tall_0, w0_0, _S826, _S826 * length(_S825 - dir_11) / sqrt(max(abs(dot(dir_11, n_16)), 0.029999999329447746f)));
            albedo_10 = _S827;
        }
    }
    else
    {
        if((((uint4((&f_50->shell_0)->cfg_5) ).x) & 2U) != 0U)
        {
            bool _S828 = shell_face_0(f_50, &_S735);
            _S750 = _S828;
        }
        else
        {
            _S750 = false;
        }
        if(_S750)
        {
            float3 _S829 = view_dir_0(f_50, i_27->pos_0.xy + float2(1.0f, 0.0f));
            float _S830 = length(p_19);
            float3 _S831 = shell_tint_0(f_50, &_S735, g_17, albedo_10, _S830, _S830 * length(_S829 - dir_11) / sqrt(max(abs(dot(dir_11, n_16)), 0.029999999329447746f)), kernelContext_65);
            albedo_10 = _S831;
        }
    }
    float3 v_14 = - dir_11;
    float _S832 = dot(n_16, v_14);
    if(_S832 < 0.0f)
    {
        n_16 = normalize(n_16 - float3((1.0099999904632568f * _S832))  * v_14);
    }
    float zlin_1 = - (((float4(p_19, 1.0f)) * (matrix<float,int(4),int(4)> (f_50->view_0.data_0[int(0)][int(0)], f_50->view_0.data_0[int(1)][int(0)], f_50->view_0.data_0[int(2)][int(0)], f_50->view_0.data_0[int(3)][int(0)], f_50->view_0.data_0[int(0)][int(1)], f_50->view_0.data_0[int(1)][int(1)], f_50->view_0.data_0[int(2)][int(1)], f_50->view_0.data_0[int(3)][int(1)], f_50->view_0.data_0[int(0)][int(2)], f_50->view_0.data_0[int(1)][int(2)], f_50->view_0.data_0[int(2)][int(2)], f_50->view_0.data_0[int(3)][int(2)], f_50->view_0.data_0[int(0)][int(3)], f_50->view_0.data_0[int(1)][int(3)], f_50->view_0.data_0[int(2)][int(3)], f_50->view_0.data_0[int(3)][int(3)])))).z;
    float3 ng_3 = cross(e1_2, e2_2);
    float3 ng_4;
    if((length_squared_0(ng_3)) > 9.99999968265522539e-21f)
    {
        ng_4 = normalize(ng_3);
    }
    else
    {
        ng_4 = n_16;
    }
    if((dot(ng_4, v_14)) < 0.0f)
    {
        ng_4 = - ng_4;
    }
    float2 _S833 = i_27->pos_0.xy;
    float _S834 = sun_shadow_0(f_50, kernelContext_65->ed_c_0->shadow_0, p_19, ng_4, zlin_1, _S833, kernelContext_65);
    float sun_vis_4;
    if(_S748)
    {
        sun_vis_4 = min(_S834, sun_k_0);
    }
    else
    {
        sun_vis_4 = _S834;
    }
    float2 _S835 = ao_upsample_0(f_50, kernelContext_65->ed_c_0->ao_0, _S833, zlin_1, kernelContext_65);
    float ao_9 = _S835.x * mat_ao_0;
    float sv_6 = _S835.y;
    float3 _S836 = sun_light_at_0(f_50, p_19, kernelContext_65);
    float3 _S837 = shade_pbr_0(f_50, _S836, albedo_10, 0.0f, rough_6, ao_9, sv_6, sun_vis_4, n_16, v_14, kernelContext_65);
    float3 _S838 = apply_aerial_0(f_50, _S837, p_19, _S833, kernelContext_65);
    float3 col_4;
    if(((uint4((&f_50->light_0)->flags_0) ).w) == 1U)
    {
        uint _S839 = shadow_cascade_0(f_50, p_19, zlin_1);
        col_4 = TINT_0[_S839] * float3((0.039999999105930328f + 0.5f * sun_vis_4)) ;
    }
    else
    {
        if(((uint4((&f_50->light_0)->flags_0) ).w) == 2U)
        {
            col_4 = float3((0.5f * ao_9)) ;
        }
        else
        {
            if(((uint4((&f_50->light_0)->flags_0) ).w) == 4U)
            {
                col_4 = float3((0.5f * sv_6)) ;
            }
            else
            {
                col_4 = _S838;
            }
        }
    }
    if(_S748)
    {
        _S750 = (_S747 & 16777216U) != 0U;
    }
    else
    {
        _S750 = false;
    }
    if(_S750)
    {
        _S750 = (cv_2.x) > 0.0f;
    }
    else
    {
        _S750 = false;
    }
    if(_S750)
    {
        col_4 = mix(col_4, float3(saturate(cv_2.y / 40.0f), cv_2.x, cv_2.z) * float3(0.60000002384185791f) , float3(0.75f) );
    }
    return float4(col_4, 1.0f);
}

constant array<float3, int(5)> TINT_1 = { { float3(1.0f, 0.25f, 0.25f), float3(0.25f, 1.0f, 0.25f), float3(0.30000001192092896f, 0.44999998807907104f, 1.0f), float3(1.0f, 1.0f, 0.25f), float3(0.60000002384185791f, 0.60000002384185791f, 0.60000002384185791f) } };
float4 terrain_pixel_1(FrameData_natural_0 device* f_51, const FsOut_0 thread* i_28, uint2 vis_3, float3 dir_12, KernelContext_0 thread* kernelContext_66)
{
    uint _S840 = vis_3.y;
    if((_S840 & 2147483648U) != 0U)
    {
        float4 _S841 = shell_pixel_0(f_51, i_28, vis_3, dir_12, kernelContext_66);
        return _S841;
    }
    TerrainDraw_natural_0 device* _S842 = ed_buf_2(kernelContext_66->ed_c_0->t_draws_0, kernelContext_66);
    TerrainDraw_natural_0 device* _S843 = _S842 + ((vis_3.x) & 2147483647U);
    TerrainDraw_natural_0 d_47 = *_S843;
    thread TerrainDraw_natural_0 _S844 = *_S843;
    uint pool_32 = kernelContext_66->ed_c_0->t_pool_0;
    uint3 tri_2 = t_triangle_0(_S840 & 268435455U);
    uint _S845 = tri_2.x;
    _S844 = d_47;
    float3 _S846 = t_vertex_1(pool_32, &_S844, _S845, kernelContext_66);
    uint _S847 = tri_2.y;
    float3 _S848 = t_vertex_1(pool_32, &_S844, _S847, kernelContext_66);
    float3 e1_3 = _S848 - _S846;
    uint _S849 = tri_2.z;
    float3 _S850 = t_vertex_1(pool_32, &_S844, _S849, kernelContext_66);
    float3 e2_3 = _S850 - _S846;
    float2 bb_2 = ray_bary_0(dir_12, _S846, e1_3, e2_3);
    float b1_4 = saturate(bb_2.x);
    float b2_6 = saturate(bb_2.y);
    float bs_2 = b1_4 + b2_6;
    float b1_5;
    float b2_7;
    if(bs_2 > 1.0f)
    {
        float b2_8 = b2_6 / bs_2;
        b1_5 = b1_4 / bs_2;
        b2_7 = b2_8;
    }
    else
    {
        b1_5 = b1_4;
        b2_7 = b2_6;
    }
    float3 p_20 = _S846 + e1_3 * float3(b1_5)  + e2_3 * float3(b2_7) ;
    float2 g_18 = t_vertex_grid_0(_S845) * float2((1.0f - b1_5 - b2_7))  + t_vertex_grid_0(_S847) * float2(b1_5)  + t_vertex_grid_0(_S849) * float2(b2_7) ;
    float _S851 = t_sample_spacing_0(&_S844, g_18);
    float _S852 = max(_S851, 0.0099999997764825821f);
    int rn_1 = clamp(int(1.2000000476837158f / _S852 + 0.5f), int(1), int(4));
    thread float3 up_6;
    thread float h_34;
    float3 _S853 = t_smooth_normal_0(pool_32, &_S844, g_18, rn_1, &up_6, &h_34, kernelContext_66);
    int r_15 = clamp(int(24.0f / _S852 + 0.5f), int(1), int(6));
    float3 nm_2;
    if(r_15 == rn_1)
    {
        nm_2 = _S853;
    }
    else
    {
        float3 _S854 = t_smooth_normal_0(pool_32, &_S844, g_18, r_15, &up_6, &h_34, kernelContext_66);
        nm_2 = _S854;
    }
    float3 _S855 = terrain_albedo_0(h_34, dot(nm_2, up_6));
    float4 _S856 = float4(0.0f) ;
    float4 _S857 = float4((&_S844)->shape_0) ;
    float2 _S858 = float2((_S857.x * _S857.y)) ;
    float2 w0_1 = (float4((&_S844)->tex_0) ).xy + (g_18 - float2(32.0f) ) * _S858;
    thread TCarpet_0 gc_2;
    (&gc_2)->on_0 = false;
    (&gc_2)->v_7 = 0.0f;
    (&gc_2)->dither_0 = 0.5f;
    (&gc_2)->edge_5 = 0.18000000715255737f;
    (&gc_2)->mask_0 = 1.0f;
    float2 _S859 = float2(0.0f) ;
    (&gc_2)->cover_3 = _S859;
    (&gc_2)->tall_0 = 0.0f;
    float3 _S860 = float3(1.0f) ;
    (&gc_2)->grass_gain_0 = _S860;
    (&gc_2)->soil_gain_0 = _S860;
    if((((uint4((&_S844)->slots_0) ).z) & 33554432U) != 0U)
    {
        float2 _S861 = i_28->pos_0.xy;
        float3 _S862 = view_dir_0(f_51, _S861);
        float3 _S863 = view_dir_0(f_51, _S861 + float2(1.0f, 0.0f));
        float far_5 = smoothstep(0.75f, 3.0f, length(p_20) * length(_S863 - _S862) / sqrt(max(abs(dot(_S862, up_6)), 0.05000000074505806f)));
        bool _S864 = far_5 < 1.0f;
        float2 _S865 = gf_warp_f_0(w0_1, _S864);
        float2 gw_2 = g_18 + _S865 / _S858;
        thread float2 cov_4 = _S859;
        uint _S866 = (uint4((&_S844)->anc_0) ).y;
        if(_S866 != 0U)
        {
            float2 _S867 = t_lc_sample_0(&_S844, gw_2);
            float4 _S868 = t_canopy_bilin_0(pool_32, _S866, _S867, kernelContext_66);
            cov_4 = _S868.xw;
            if(_S864)
            {
                float2 _S869 = t_lc_sample_0(&_S844, gw_2 + float2(4.0f, 2.0f) / _S858);
                float4 _S870 = t_canopy_bilin_0(pool_32, _S866, _S869, kernelContext_66);
                float _S871 = cov_4.x + _S870.x;
                float2 _S872 = t_lc_sample_0(&_S844, gw_2 + float2(-2.0f, 4.0f) / _S858);
                float4 _S873 = t_canopy_bilin_0(pool_32, _S866, _S872, kernelContext_66);
                float _S874 = _S871 + _S873.x;
                float2 _S875 = t_lc_sample_0(&_S844, gw_2 + float2(-4.0f, -2.0f) / _S858);
                float4 _S876 = t_canopy_bilin_0(pool_32, _S866, _S875, kernelContext_66);
                float _S877 = _S874 + _S876.x;
                float2 _S878 = t_lc_sample_0(&_S844, gw_2 + float2(2.0f, -4.0f) / _S858);
                float4 _S879 = t_canopy_bilin_0(pool_32, _S866, _S878, kernelContext_66);
                cov_4.x = (_S877 + _S879.x) * 0.20000000298023224f;
            }
        }
        TCarpet_0 _S880 = t_carpet_0(f_51, pool_32, &_S844, g_18, gw_2, w0_1, far_5, h_34, cov_4, kernelContext_66);
        gc_2 = _S880;
    }
    float grass_k_1;
    float mat_ao_2;
    float rough_7;
    float3 n_17;
    float3 albedo_11;
    if((kernelContext_66->ed_c_0->t_mat_0) != 0U)
    {
        TerrainMat_natural_0 device* _S881 = ed_buf_5(kernelContext_66->ed_c_0->t_draws_0, kernelContext_66);
        thread TerrainMat_natural_0 _S882 = *(_S881 + kernelContext_66->ed_c_0->t_mat_0);
        float3 _S883 = (float4((&_S844)->fu_0) ).xyz;
        float3 tu_7 = normalize(_S883 - up_6 * float3(dot(_S883, up_6)) );
        float3 _S884 = (float4((&_S844)->fv_0) ).xyz;
        float3 tv_8 = normalize(_S884 - up_6 * float3(dot(_S884, up_6)) );
        float2 _S885 = i_28->pos_0.xy;
        float3 _S886 = view_dir_0(f_51, _S885 + float2(1.0f, 0.0f));
        float2 bx_1 = ray_bary_0(_S886, _S846, e1_3, e2_3);
        float3 _S887 = view_dir_0(f_51, _S885 + float2(0.0f, 1.0f));
        float2 by_1 = ray_bary_0(_S887, _S846, e1_3, e2_3);
        float3 dpx_1 = _S846 + e1_3 * float3(bx_1.x)  + e2_3 * float3(bx_1.y)  - p_20;
        float3 dpy_1 = _S846 + e1_3 * float3(by_1.x)  + e2_3 * float3(by_1.y)  - p_20;
        float2 dwx_2 = float2(dot(dpx_1, tu_7), dot(dpx_1, tv_8));
        float2 dwy_2 = float2(dot(dpy_1, tu_7), dot(dpy_1, tv_8));
        float2 sp_2 = g_18 * float2(4.0f) ;
        int rf_1 = clamp(int(2.0f / _S852 + 0.5f), int(1), int(4));
        float _S888 = float(clamp(int(8.0f / _S852 + 0.5f), int(1), int(16)));
        float2 _S889 = float2(_S888, 0.0f);
        float _S890 = t_page_bilin_0(pool_32, &_S844, sp_2 + _S889, kernelContext_66);
        float _S891 = t_page_bilin_0(pool_32, &_S844, sp_2 - _S889, kernelContext_66);
        float _S892 = _S890 + _S891;
        float2 _S893 = float2(0.0f, _S888);
        float _S894 = t_page_bilin_0(pool_32, &_S844, sp_2 + _S893, kernelContext_66);
        float _S895 = _S892 + _S894;
        float _S896 = t_page_bilin_0(pool_32, &_S844, sp_2 - _S893, kernelContext_66);
        float lap_1 = _S895 + _S896 - 4.0f * h_34;
        if((length_squared_0(p_20)) < 1.6e+05f)
        {
            float _S897 = float(rf_1);
            float2 _S898 = float2(_S897, 0.0f);
            float _S899 = t_page_bilin_0(pool_32, &_S844, sp_2 + _S898, kernelContext_66);
            float _S900 = t_page_bilin_0(pool_32, &_S844, sp_2 - _S898, kernelContext_66);
            float _S901 = _S899 + _S900;
            float2 _S902 = float2(0.0f, _S897);
            float _S903 = t_page_bilin_0(pool_32, &_S844, sp_2 + _S902, kernelContext_66);
            float _S904 = _S901 + _S903;
            float _S905 = t_page_bilin_0(pool_32, &_S844, sp_2 - _S902, kernelContext_66);
            grass_k_1 = _S904 + _S905 - 4.0f * h_34;
        }
        else
        {
            grass_k_1 = 0.0f;
        }
        float dl_2 = _S888 * _S852;
        float df_1 = float(rf_1) * _S852;
        float _S906 = lap_1 / (dl_2 * dl_2);
        float _S907 = grass_k_1 / (df_1 * df_1);
        float _S908 = dot(dpx_1, up_6);
        float _S909 = dot(dpy_1, up_6);
        float _S910 = length(p_20);
        thread TCarpet_0 _S911 = gc_2;
        TSurf_0 _S912 = terrain_material_0(f_51, &_S882, &_S844, _S853, nm_2, up_6, h_34, _S906, _S907, w0_1, dwx_2, dwy_2, _S908, _S909, tu_7, tv_8, _S910, false, _S856, _S852, &_S911, kernelContext_66);
        grass_k_1 = _S912.grass_1;
        n_17 = _S912.n_10;
        albedo_11 = _S912.albedo_4;
        mat_ao_2 = _S912.ao_5;
        rough_7 = _S912.rough_3;
    }
    else
    {
        grass_k_1 = 0.0f;
        n_17 = _S853;
        albedo_11 = _S855;
        mat_ao_2 = 1.0f;
        rough_7 = 0.92000001668930054f;
    }
    if((&gc_2)->on_0)
    {
        if(grass_k_1 > 0.0f)
        {
            float3 _S913 = view_dir_0(f_51, i_28->pos_0.xy + float2(1.0f, 0.0f));
            float _S914 = length(p_20);
            float3 _S915 = ground_shade_0(f_51, albedo_11, grass_k_1, (&gc_2)->tall_0, w0_1, _S914, _S914 * length(_S913 - dir_12) / sqrt(max(abs(dot(dir_12, n_17)), 0.029999999329447746f)));
            albedo_11 = _S915;
        }
    }
    else
    {
        bool _S916;
        if((((uint4((&f_51->shell_0)->cfg_5) ).x) & 2U) != 0U)
        {
            bool _S917 = shell_face_0(f_51, &_S844);
            _S916 = _S917;
        }
        else
        {
            _S916 = false;
        }
        if(_S916)
        {
            float3 _S918 = view_dir_0(f_51, i_28->pos_0.xy + float2(1.0f, 0.0f));
            float _S919 = length(p_20);
            float3 _S920 = shell_tint_0(f_51, &_S844, g_18, albedo_11, _S919, _S919 * length(_S918 - dir_12) / sqrt(max(abs(dot(dir_12, n_17)), 0.029999999329447746f)), kernelContext_66);
            albedo_11 = _S920;
        }
    }
    float3 v_15 = - dir_12;
    float _S921 = dot(n_17, v_15);
    if(_S921 < 0.0f)
    {
        n_17 = normalize(n_17 - float3((1.0099999904632568f * _S921))  * v_15);
    }
    float zlin_2 = - (((float4(p_20, 1.0f)) * (matrix<float,int(4),int(4)> (f_51->view_0.data_0[int(0)][int(0)], f_51->view_0.data_0[int(1)][int(0)], f_51->view_0.data_0[int(2)][int(0)], f_51->view_0.data_0[int(3)][int(0)], f_51->view_0.data_0[int(0)][int(1)], f_51->view_0.data_0[int(1)][int(1)], f_51->view_0.data_0[int(2)][int(1)], f_51->view_0.data_0[int(3)][int(1)], f_51->view_0.data_0[int(0)][int(2)], f_51->view_0.data_0[int(1)][int(2)], f_51->view_0.data_0[int(2)][int(2)], f_51->view_0.data_0[int(3)][int(2)], f_51->view_0.data_0[int(0)][int(3)], f_51->view_0.data_0[int(1)][int(3)], f_51->view_0.data_0[int(2)][int(3)], f_51->view_0.data_0[int(3)][int(3)])))).z;
    float3 ng_5 = cross(e1_3, e2_3);
    float3 ng_6;
    if((length_squared_0(ng_5)) > 9.99999968265522539e-21f)
    {
        ng_6 = normalize(ng_5);
    }
    else
    {
        ng_6 = n_17;
    }
    if((dot(ng_6, v_15)) < 0.0f)
    {
        ng_6 = - ng_6;
    }
    float2 _S922 = i_28->pos_0.xy;
    float _S923 = sun_shadow_0(f_51, kernelContext_66->ed_c_0->shadow_0, p_20, ng_6, zlin_2, _S922, kernelContext_66);
    float2 _S924 = ao_upsample_0(f_51, kernelContext_66->ed_c_0->ao_0, _S922, zlin_2, kernelContext_66);
    float ao_10 = _S924.x * mat_ao_2;
    float sv_7 = _S924.y;
    float3 _S925 = sun_light_at_0(f_51, p_20, kernelContext_66);
    float3 _S926 = shade_pbr_0(f_51, _S925, albedo_11, 0.0f, rough_7, ao_10, sv_7, _S923, n_17, v_15, kernelContext_66);
    float3 _S927 = apply_aerial_0(f_51, _S926, p_20, _S922, kernelContext_66);
    float3 col_5;
    if(((uint4((&f_51->light_0)->flags_0) ).w) == 1U)
    {
        uint _S928 = shadow_cascade_0(f_51, p_20, zlin_2);
        col_5 = TINT_1[_S928] * float3((0.039999999105930328f + 0.5f * _S923)) ;
    }
    else
    {
        if(((uint4((&f_51->light_0)->flags_0) ).w) == 2U)
        {
            col_5 = float3((0.5f * ao_10)) ;
        }
        else
        {
            if(((uint4((&f_51->light_0)->flags_0) ).w) == 4U)
            {
                col_5 = float3((0.5f * sv_7)) ;
            }
            else
            {
                col_5 = _S927;
            }
        }
    }
    return float4(col_5, 1.0f);
}

constant array<float3, int(5)> TINT_2 = { { float3(1.0f, 0.25f, 0.25f), float3(0.25f, 1.0f, 0.25f), float3(0.30000001192092896f, 0.44999998807907104f, 1.0f), float3(1.0f, 1.0f, 0.25f), float3(0.60000002384185791f, 0.60000002384185791f, 0.60000002384185791f) } };
float3 sky_rad_1(FrameData_natural_0 device* f_52, float3 d_48, float disc_k_1)
{
    float3 sun_7 = (float4(f_52->sun_dir_0) ).xyz;
    float h_35 = d_48.y;
    float t_13 = pow(saturate(1.0f - max(h_35, 0.0f)), 6.5f);
    float mu_7 = dot(d_48, sun_7);
    float warm_1 = saturate(1.0f - sun_7.y * 2.5f);
    float3 c_27 = mix((float4(f_52->sky_zenith_0) ).xyz, (float4(f_52->sky_horizon_0) ).xyz, float3(t_13) ) + float3(1.0f, 0.41999998688697815f, 0.14000000059604645f) * float3(pow(saturate(mu_7 * 0.5f + 0.5f), 5.0f))  * float3(warm_1)  * float3(0.85000002384185791f)  * float3((0.40000000596046448f + t_13)) ;
    float3 _S929 = float3((float4(f_52->sun_dir_0) ).w) ;
    return mix(mix(c_27, c_27 * float3(1.0f, 0.69999998807907104f, 0.55000001192092896f), float3((warm_1 * 0.40000000596046448f)) ) + float3(1.0f, 0.89999997615814209f, 0.69999998807907104f) * float3(pow(saturate(mu_7), 28.0f))  * float3(0.34999999403953552f) , (float4(f_52->ground_color_0) ).xyz, float3(saturate(- h_35 * 6.0f)) ) * _S929 + float3(1.0f, 0.94999998807907104f, 0.85000002384185791f) * float3((smoothstep(0.99965000152587891f, 0.99984997510910034f, mu_7) * step(0.0f, h_35)))  * float3(60.0f)  * _S929 * float3(disc_k_1) ;
}

float atm_rt_1(FrameData_natural_0 device* f_53)
{
    return (float4((&f_53->atmos_0)->radii_0) ).y;
}

bool atm_enter_0(FrameData_natural_0 device* f_54, float thread* h_36, float thread* mu_8, float thread* mu_s_3, float nu_1, float thread* te_2)
{
    *te_2 = 0.0f;
    float _S930 = atm_rb_1(f_54);
    float _S931 = atm_rt_1(f_54);
    float _S932 = _S931 - _S930;
    if((*h_36) <= _S932)
    {
        return true;
    }
    float _S933 = atm_enter_top_0(_S930, _S931, *h_36, *mu_8);
    *te_2 = _S933;
    if(_S933 < 0.0f)
    {
        return false;
    }
    float r_16 = _S930 + *h_36;
    *mu_8 = clamp((r_16 * *mu_8 + *te_2) / _S931, -1.0f, 1.0f);
    *mu_s_3 = clamp((r_16 * *mu_s_3 + *te_2 * nu_1) / _S931, -1.0f, 1.0f);
    *h_36 = _S932;
    return true;
}

bool atm_hits_ground_0(float rb_5, float h_37, float mu_9)
{
    float rm_0 = (rb_5 + h_37) * mu_9;
    bool _S934;
    if(mu_9 < 0.0f)
    {
        _S934 = (rm_0 * rm_0) >= (h_37 * (2.0f * rb_5 + h_37));
    }
    else
    {
        _S934 = false;
    }
    return _S934;
}

float3 atm_transmittance_1(FrameData_natural_0 device* f_55, float h_38, float mu_10, KernelContext_0 thread* kernelContext_67)
{
    float _S935 = atm_rb_0(f_55);
    float _S936 = atm_rt_0(f_55);
    float2 uv_3 = atm_trans_uv_0(_S935, _S936, h_38, mu_10);
    packed_float4 device* _S937 = ed_buf_4((uint4((&f_55->atmos_0)->bufs_0) ).x, kernelContext_67);
    float4 _S938 = atm_bilinear_0(_S937, 256U, 0U, 64U, uv_3);
    return _S938.xyz;
}

float3 atmos_sky_1(FrameData_natural_0 device* f_56, float3 d_49, KernelContext_0 thread* kernelContext_68)
{
    float3 _S939 = atm_sky_view_0(f_56, (uint4((&f_56->atmos_0)->bufs_0) ).z, 192U, 108U, (float4((&f_56->atmos_0)->radii_0) ).z, d_49, kernelContext_68);
    return _S939;
}

float3 atmos_sky_pixel_0(FrameData_natural_0 device* f_57, float3 d_50, float disc_k_2, KernelContext_0 thread* kernelContext_69)
{
    float3 _S940 = atmos_sky_1(f_57, d_50, kernelContext_69);
    float nu_2 = dot(d_50, (float4(f_57->sun_dir_0) ).xyz);
    float cr_0 = (float4((&f_57->atmos_0)->sun_0) ).w;
    float _S941 = cr_0 - 1.99999999495048542e-06f;
    float3 c_28;
    if(nu_2 > _S941)
    {
        thread float h_39 = (float4((&f_57->atmos_0)->radii_0) ).z;
        thread float mu_11 = dot(d_50, (float4((&f_57->atmos_0)->up_0) ).xyz);
        thread float mu_s_4 = (float4((&f_57->atmos_0)->up_0) ).w;
        float3 _S942 = float3(1.0f) ;
        thread float te_3;
        bool _S943 = atm_enter_0(f_57, &h_39, &mu_11, &mu_s_4, nu_2, &te_3);
        if(_S943)
        {
            float _S944 = atm_rb_0(f_57);
            if(atm_hits_ground_0(_S944, h_39, mu_11))
            {
                c_28 = float3(0.0f) ;
            }
            else
            {
                float3 _S945 = atm_transmittance_1(f_57, h_39, mu_11, kernelContext_69);
                c_28 = _S945;
            }
        }
        else
        {
            c_28 = _S942;
        }
        float sr2_0 = 1.0f - cr_0 * cr_0;
        c_28 = _S940 + min((float4((&f_57->atmos_0)->sun_0) ).xyz * c_28 / float3((3.1415927410125732f * sr2_0))  * float3((1.0f - 0.60000002384185791f * (1.0f - sqrt(1.0f - saturate((1.0f - nu_2 * nu_2) / sr2_0)))))  * float3(smoothstep(_S941, cr_0 + 1.99999999495048542e-06f, nu_2))  * float3(disc_k_2) , float3(30000.0f) );
    }
    else
    {
        c_28 = _S940;
    }
    return c_28;
}

float3 sky_pixel_0(FrameData_natural_0 device* f_58, float3 d_51, KernelContext_0 thread* kernelContext_70)
{
    bool _S946 = atmos_on_0(f_58);
    if(!_S946)
    {
        float3 _S947 = sky_rad_1(f_58, d_51, (float4((&f_58->light_0)->sky_0) ).x);
        return _S947;
    }
    if(((uint4((&f_58->atmos_0)->cfg_1) ).y) == 1U)
    {
        thread float h_40 = (float4((&f_58->atmos_0)->radii_0) ).z;
        thread float mu_12 = dot(d_51, (float4((&f_58->atmos_0)->up_0) ).xyz);
        thread float mu_s_5 = (float4((&f_58->atmos_0)->up_0) ).w;
        thread float te_4;
        bool _S948 = atm_enter_0(f_58, &h_40, &mu_12, &mu_s_5, dot(d_51, (float4(f_58->sun_dir_0) ).xyz), &te_4);
        if(!_S948)
        {
            return float3(0.5f) ;
        }
        float _S949 = atm_rb_0(f_58);
        float3 _S950;
        if(atm_hits_ground_0(_S949, h_40, mu_12))
        {
            _S950 = float3(0.0f) ;
        }
        else
        {
            float3 _S951 = atm_transmittance_1(f_58, h_40, mu_12, kernelContext_70);
            _S950 = _S951 * float3(0.5f) ;
        }
        return _S950;
    }
    float _S952;
    if(((uint4((&f_58->atmos_0)->cfg_1) ).y) == 2U)
    {
        _S952 = 0.0f;
    }
    else
    {
        _S952 = (float4((&f_58->light_0)->sky_0) ).x;
    }
    float3 _S953 = atmos_sky_pixel_0(f_58, d_51, _S952, kernelContext_70);
    return _S953;
}

struct GrassClump_0
{
    float px_5;
    float py_0;
    float pz_0;
    uint node_kind_0;
    uint up_7;
    uint facing_0;
    uint seed_0;
    uint hw_0;
};

GrassClump_0 device* ed_buf_6(uint i_29, KernelContext_0 thread* kernelContext_71)
{
    return (GrassClump_0 device*)kernelContext_71->ed_buffers_0[i_29];
}

struct GrassBlade_0
{
    float3 p0_1;
    float3 p1_0;
    float3 p2_0;
    float3 side_2;
    float3 up_8;
    float w_21;
    float h_41;
    float head_0;
    float head_w_0;
    float shade_0;
    float t_ao_0;
};

float2 unpack_snorm2x16_0(uint p_21)
{
    return clamp(float2(int2(int(p_21 << 16U) >> 16U, int(p_21) >> 16U)) / float2(32767.0f) , float2(-1.0f) , float2(1.0f) );
}

float3 oct_decode_0(uint packed_0)
{
    float2 e_13 = unpack_snorm2x16_0(packed_0);
    float _S954 = e_13.x;
    float _S955 = e_13.y;
    float _S956 = 1.0f - abs(_S954) - abs(_S955);
    thread float3 n_18 = float3(_S954, _S955, _S956);
    float t_14 = saturate(- _S956);
    float _S957;
    if(_S954 >= 0.0f)
    {
        _S957 = - t_14;
    }
    else
    {
        _S957 = t_14;
    }
    n_18.x = n_18.x + _S957;
    if((n_18.y) >= 0.0f)
    {
        _S957 = - t_14;
    }
    else
    {
        _S957 = t_14;
    }
    n_18.y = n_18.y + _S957;
    return normalize(n_18);
}

float grass_floor_0(FrameData_natural_0 device* f_59, float d_52, float blades_0)
{
    float bm_0 = (float4((&f_59->grass_0)->lod_0) ).z;
    float bm_1;
    if(((float4((&f_59->grass_0)->look_0) ).z) > 0.0f)
    {
        bm_1 = clamp(bm_0 * (float4((&f_59->grass_0)->look_0) ).z / max(d_52, 0.0010000000474974513f), min((float4((&f_59->grass_0)->look_0) ).y, bm_0), bm_0);
    }
    else
    {
        bm_1 = bm_0;
    }
    return max(min(bm_1, blades_0), 0.0010000000474974513f);
}

float2 grass_lod_0(FrameData_natural_0 device* f_60, float d_53, float blades_1)
{
    float q_21 = (float4((&f_60->grass_0)->lod_0) ).x / max(d_53, 0.0010000000474974513f);
    float raw_0 = blades_1 * min(q_21 * q_21, 1.0f);
    float _S958 = grass_floor_0(f_60, d_53, blades_1);
    return float2(max(raw_0, _S958), min(raw_0 / _S958, 1.0f));
}

float grass_rank_0(uint node_kind_1)
{
    uint q_22 = node_kind_1 >> 24U;
    float _S959;
    if(q_22 == 0U)
    {
        _S959 = 0.0f;
    }
    else
    {
        _S959 = exp2((float(q_22) / 255.0f - 1.0f) * 14.0f);
    }
    return _S959;
}

float grass_radius_0(FrameData_natural_0 device* f_61, const GrassType_natural_0 thread* ty_0, float keep_0)
{
    return max((float4(ty_0->b_6) ).x, (float4((&f_61->grass_0)->far_1) ).x * 0.34000000357627869f * rsqrt(max(keep_0, 9.9999997473787516e-05f)));
}

float grass_grow_0(FrameData_natural_0 device* f_62, float d_54)
{
    return 1.0f + (float4((&f_62->grass_0)->far_1) ).y * smoothstep(2.0f * (float4((&f_62->grass_0)->lod_0) ).x, 0.5f * (float4((&f_62->grass_0)->lod_0) ).y, d_54);
}

GrassBlade_0 grass_blade_0(FrameData_natural_0 device* f_63, const GrassClump_0 thread* c_29, const GrassType_natural_0 thread* ty_1, float3 node_0, uint k_14, float lod_mul_0)
{
    float3 up_9 = oct_decode_0(c_29->up_7);
    float3 rt_3 = normalize(cross(up_9, oct_decode_0(c_29->facing_0)));
    float3 fa_0 = cross(rt_3, up_9);
    float3 base_2 = node_0 + float3(c_29->px_5, c_29->py_0, c_29->pz_0);
    float d_55 = length(base_2);
    float4 _S960 = float4(ty_1->a_6) ;
    float blades_2 = _S960.w;
    float2 _S961 = grass_lod_0(f_63, d_55, blades_2);
    float _S962 = grass_floor_0(f_63, d_55, blades_2);
    float _S963 = max(_S961.x * lod_mul_0, _S962);
    float _S964 = _S961.y;
    float _S965 = min(pow(blades_2 / _S963, 0.80000001192092896f) * rsqrt(max(_S964, 0.20000000298023224f)), (float4((&f_63->grass_0)->lod_0) ).w);
    float keep_1 = saturate((_S964 - grass_rank_0(c_29->node_kind_0)) / (0.30000001192092896f * _S964 + 0.0010000000474974513f));
    float range_0 = (float4((&f_63->grass_0)->lod_0) ).y;
    float fade_1 = 1.0f - smoothstep(0.60000002384185791f * range_0, range_0, d_55);
    uint _S966 = c_29->seed_0;
    uint s0_0 = grass_hash_0((c_29->seed_0) ^ (k_14 * 2654435769U + 2135587861U));
    uint s1_1 = grass_hash_0(s0_0);
    uint s2_0 = grass_hash_0(s1_1);
    uint s3_0 = grass_hash_0(s2_0);
    uint s4_0 = grass_hash_0(s3_0);
    uint s5_0 = grass_hash_0(s4_0);
    float ang_0 = grass_rand_0(s0_0) * 6.2831854820251465f;
    float rad_3 = sqrt(grass_rand_0(s1_1));
    float _S967 = grass_radius_0(f_63, ty_1, _S964);
    float3 off_7 = (fa_0 * float3(cos(ang_0))  + rt_3 * float3((sin(ang_0) * (float4(ty_1->root_0) ).w)) ) * float3((rad_3 * _S967)) ;
    thread GrassBlade_0 o_6;
    (&o_6)->up_8 = up_9;
    uint _S968 = (c_29->hw_0) & 65535U;
    float hc_0 = mix(_S960.x, _S960.y, grass_rand_0(grass_hash_0(_S966 + 17U))) * (float(_S968) / 65535.0f * 4.0f);
    (&o_6)->p0_1 = base_2 + off_7 - up_9 * float3(min(0.029999999329447746f, 0.10000000149011612f * hc_0)) ;
    float _S969 = hc_0 * (1.0f - 0.34999999403953552f * rad_3) * mix(0.72000002861022949f, 1.1200000047683716f, grass_rand_0(s2_0));
    float _S970 = grass_grow_0(f_63, d_55);
    float h_42 = _S969 * _S970;
    float h_43;
    if(((float4((&f_63->grass_0)->under_0) ).w) > 0.0f)
    {
        h_43 = max(h_42, min(1.5f * (d_55 * 2.0f / max((float4(f_63->proj_0) ).y * (float4(f_63->proj_0) ).w, 1.0f)), 3.0f * h_42) * (1.0f - smoothstep(0.15000000596046448f, 0.44999998807907104f, abs(dot(base_2 / float3(max(d_55, 0.0010000000474974513f)) , up_9)))) * smoothstep(2.0f * (float4((&f_63->grass_0)->lod_0) ).x, 4.0f * (float4((&f_63->grass_0)->lod_0) ).x, d_55));
    }
    else
    {
        h_43 = h_42;
    }
    float grow_0 = saturate(_S963 - float(k_14)) * keep_1 * fade_1;
    bool _S971;
    if(k_14 < uint(blades_2))
    {
        _S971 = _S968 != 0U;
    }
    else
    {
        _S971 = false;
    }
    if(_S971)
    {
        h_43 = h_43 * grow_0;
    }
    else
    {
        h_43 = 0.0f;
    }
    (&o_6)->h_41 = h_43;
    float ca_0 = grass_rand_0(grass_hash_0(_S966 + 29U)) * 6.2831854820251465f;
    float3 cdir_0 = fa_0 * float3(cos(ca_0))  + rt_3 * float3(sin(ca_0)) ;
    float3 radial_0;
    if(rad_3 > 0.0010000000474974513f)
    {
        radial_0 = normalize(off_7);
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
    float4 _S972 = float4(ty_1->b_6) ;
    float lean_0 = _S972.y * mix(0.30000001192092896f, 1.0f, grass_rand_0(s4_0));
    float3 tipv_0 = up_9 * float3(cos(lean_0))  + ld_1 * float3(sin(lean_0)) ;
    WindData_natural_0 wn_0 = f_63->wind_1;
    if((((uint4((&f_63->grass_0)->bufs_2) ).w) & 4U) != 0U)
    {
        _S971 = ((float4(wn_0.cfg_2) ).x) > 0.5f;
    }
    else
    {
        _S971 = false;
    }
    float3 tipv_1;
    if(_S971)
    {
        float4 _S973 = float4(wn_0.wind_0) ;
        float2 _S974 = _S973.xz;
        float speed_0 = length(_S974);
        float2 wd_0;
        if(speed_0 > 0.0010000000474974513f)
        {
            wd_0 = _S974 / float2(speed_0) ;
        }
        else
        {
            wd_0 = float2(0.0f, 0.0f);
        }
        float4 _S975 = float4(wn_0.gust_0) ;
        float2 _S976 = base_2.xz;
        float _S977 = dot(_S976, wd_0);
        float g_19 = 0.5f + 0.5f * sin((_S975.x + _S977 / max(_S975.y, 1.0f)) * 6.2831854820251465f);
        float tw_2 = _S973.w;
        float _S978 = wd_0.y;
        float _S979 = wd_0.x;
        float _S980 = grass_rand_0(s5_0);
        float _S981 = 1.0f - _S972.z;
        float3 wv_0 = float3(_S979, 0.0f, _S978);
        tipv_1 = normalize(tipv_0 + (wv_0 - up_9 * float3(dot(wv_0, up_9)) ) * float3(clamp(speed_0 * 0.059999998658895493f * _S981 * (0.34999999403953552f + 0.89999997615814209f * (g_19 * g_19 * g_19 * _S975.z) + 0.69999998807907104f * (0.5f + 0.5f * sin(_S977 * 0.89999997615814209f - tw_2 * (1.2000000476837158f + 0.25f * speed_0) + 0.60000002384185791f * sin(dot(_S976, float2(- _S978, _S979)) * 0.34999999403953552f)))) + 0.039999999105930328f * _S981 * sin(tw_2 * (5.0f + 3.0f * _S980) + _S980 * 40.0f) * min(speed_0 * 0.20000000298023224f, 1.0f), -0.30000001192092896f, 1.3999999761581421f)) );
    }
    else
    {
        tipv_1 = tipv_0;
    }
    float3 _S982 = (&o_6)->p0_1 + tipv_1 * float3(((&o_6)->h_41 * (1.0f - 0.18000000715255737f * saturate(1.0f - dot(tipv_1, up_9))))) ;
    (&o_6)->p2_0 = _S982;
    float3 across_0 = _S982 - (&o_6)->p0_1 - up_9 * float3(dot(_S982 - (&o_6)->p0_1, up_9)) ;
    float curve_0 = _S972.w;
    (&o_6)->p1_0 = (&o_6)->p0_1 + up_9 * float3((max(dot(_S982 - (&o_6)->p0_1, up_9), 0.25f * (&o_6)->h_41) * mix(0.60000002384185791f, 1.0499999523162842f, curve_0)))  + across_0 * float3(mix(0.5f, 0.05000000074505806f, curve_0)) ;
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
    (&o_6)->side_2 = normalize(cross(up_9, lh_0) * float3(cos(tw2_0))  + lh_0 * float3(sin(tw2_0)) );
    float _S983 = mix(0.80000001192092896f, 1.2000000476837158f, grass_rand_0(s1_1 ^ 85U));
    (&o_6)->w_21 = max(max(_S960.z * _S965 * _S983, (float4((&f_63->grass_0)->under_0) ).w * (1.0f - smoothstep(0.15000000596046448f, 0.44999998807907104f, abs(dot(base_2 / float3(max(d_55, 0.0010000000474974513f)) , up_9)))) * (1.0f - _S964) * (0.60000002384185791f * rsqrt(max(_S964, 9.9999997473787516e-05f))) / max(_S963, 1.0f) * _S983), (float4((&f_63->grass_0)->look_0) ).w * d_55 * 2.0f / max((float4(f_63->proj_0) ).y * (float4(f_63->proj_0) ).w, 1.0f));
    (&o_6)->head_0 = (float4(ty_1->mid_0) ).w;
    (&o_6)->head_w_0 = (float4(ty_1->tip_0) ).w;
    (&o_6)->shade_0 = mix(0.81999999284744263f, 1.1499999761581421f, grass_rand_0(s4_0 ^ 51U));
    (&o_6)->t_ao_0 = saturate(hc_0 / 0.80000001192092896f);
    return o_6;
}

float3 grass_point_0(const GrassBlade_0 thread* b_14, float t_15)
{
    float s_23 = 1.0f - t_15;
    return b_14->p0_1 * float3((s_23 * s_23))  + b_14->p1_0 * float3((2.0f * s_23 * t_15))  + b_14->p2_0 * float3((t_15 * t_15)) ;
}

float3 grass_tangent_0(const GrassBlade_0 thread* b_15, float t_16)
{
    return (b_15->p1_0 - b_15->p0_1) * float3((2.0f * (1.0f - t_16)))  + (b_15->p2_0 - b_15->p1_0) * float3((2.0f * t_16)) ;
}

float3 grass_view_side_0(const GrassBlade_0 thread* b_16, float t_17, float3 p_22)
{
    float3 _S984 = grass_tangent_0(b_16, t_17);
    float3 vs_1 = cross(normalize(_S984 + b_16->up_8 * float3(9.9999997473787516e-05f) ), normalize(p_22));
    if((dot(vs_1, vs_1)) < 9.99999993922529029e-09f)
    {
        return b_16->side_2;
    }
    float3 vs_2 = normalize(vs_1);
    float3 _S985 = b_16->side_2;
    float3 vs_3;
    if((dot(vs_2, b_16->side_2)) < 0.0f)
    {
        vs_3 = - vs_2;
    }
    else
    {
        vs_3 = vs_2;
    }
    return normalize(mix(_S985, vs_3, float3(0.55000001192092896f) ));
}

float4 grass_pixel_0(FrameData_natural_0 device* f_64, const FsOut_0 thread* i_30, uint2 vis_4, float3 dir_13, KernelContext_0 thread* kernelContext_72)
{
    GrassClump_0 device* _S986 = ed_buf_6((uint4((&f_64->grass_0)->bufs_2) ).x, kernelContext_72);
    GrassClump_0 c_30 = *(_S986 + ((vis_4.x) & 536870911U));
    GrassType_natural_0 device* _S987 = ed_buf_3((uint4((&f_64->grass_0)->bufs_2) ).y, kernelContext_72);
    GrassType_natural_0 device* _S988 = _S987 + (((c_30.node_kind_0) >> 12U) & 255U);
    GrassType_natural_0 ty_2 = *_S988;
    thread GrassType_natural_0 _S989 = *_S988;
    packed_float4 device* _S990 = ed_buf_4(kernelContext_72->ed_c_0->nodes_0, kernelContext_72);
    float3 node_1 = (float4(*(_S990 + ((c_30.node_kind_0) & 4095U))) ).xyz;
    uint _S991 = vis_4.y;
    uint k_15 = _S991 & 127U;
    float t_18 = float((_S991 >> 7U) & 4095U) / 4095.0f;
    float u_1 = float((_S991 >> 19U) & 255U) / 255.0f;
    thread GrassClump_0 _S992 = c_30;
    _S989 = ty_2;
    GrassBlade_0 _S993 = grass_blade_0(f_64, &_S992, &_S989, node_1, k_15, 1.0f);
    thread GrassBlade_0 _S994 = _S993;
    float3 _S995 = grass_point_0(&_S994, t_18);
    thread GrassBlade_0 _S996 = _S993;
    float3 _S997 = grass_view_side_0(&_S996, t_18, _S995);
    thread GrassBlade_0 _S998 = _S993;
    float3 _S999 = grass_tangent_0(&_S998, t_18);
    float3 v_16 = - dir_13;
    float3 n0_0 = normalize(cross(_S997, normalize(_S999 + _S993.up_8 * float3(9.9999997473787516e-05f) )));
    float3 n0_1;
    if((dot(n0_0, v_16)) < 0.0f)
    {
        n0_1 = - n0_0;
    }
    else
    {
        n0_1 = n0_0;
    }
    float dn_0 = dot(dir_13, n0_1);
    float lp_0 = length(_S995);
    float hit_1;
    if((abs(dn_0)) > 0.0010000000474974513f)
    {
        hit_1 = dot(_S995, n0_1) / dn_0;
    }
    else
    {
        hit_1 = lp_0;
    }
    float3 p_23 = dir_13 * float3(clamp(hit_1, lp_0 - 0.05000000074505806f, lp_0 + 0.05000000074505806f)) ;
    float d_56 = length(p_23);
    float far_6 = smoothstep((float4((&f_64->grass_0)->lod_0) ).x * 0.5f, min((float4((&f_64->grass_0)->lod_0) ).y, 150.0f), d_56);
    float avg_0 = (float4((&f_64->grass_0)->far_1) ).z * smoothstep(2.0f * (float4((&f_64->grass_0)->lod_0) ).x, 0.40000000596046448f * (float4((&f_64->grass_0)->lod_0) ).y, d_56);
    float3 n_19 = normalize(mix(normalize(n0_1 + _S997 * float3(((u_1 - 0.5f) * 1.1000000238418579f)) ), _S993.up_8, float3(mix(mix((float4((&f_64->grass_0)->look_0) ).x, 0.80000001192092896f, far_6), 0.85000002384185791f, avg_0)) ));
    float3 alb_3;
    if(t_18 < 0.5f)
    {
        alb_3 = mix((float4((&_S989)->root_0) ).xyz, (float4((&_S989)->mid_0) ).xyz, float3((t_18 * 2.0f)) );
    }
    else
    {
        alb_3 = mix((float4((&_S989)->mid_0) ).xyz, (float4((&_S989)->tip_0) ).xyz, float3(((t_18 - 0.5f) * 2.0f)) );
    }
    if((_S993.head_0) > 0.0f)
    {
        float _S1000 = 1.0f - _S993.head_0;
        alb_3 = mix(alb_3, (float4((&_S989)->tip_0) ).xyz * float3(1.1000000238418579f) , float3(smoothstep(_S1000 - 0.039999999105930328f, _S1000, t_18)) );
    }
    float hue_2 = grass_rand_0(grass_hash_0(c_30.seed_0 + 53U)) - 0.5f;
    float3 alb_4 = mix(alb_3, (float4((&_S989)->root_0) ).xyz * float3(0.2199999988079071f)  + (float4((&_S989)->mid_0) ).xyz * float3(0.46000000834465027f)  + (float4((&_S989)->tip_0) ).xyz * float3(0.31999999284744263f) , float3((0.60000002384185791f * avg_0)) ) * (float3((mix(0.86000001430511475f, 1.1200000047683716f, grass_rand_0(grass_hash_0(c_30.seed_0 + 41U))) * mix(_S993.shade_0, 1.0f, avg_0)))  * float3(1.0f + 0.10000000149011612f * hue_2, 1.0f, 1.0f - 0.11999999731779099f * hue_2));
    float dry_3 = float(((c_30.hw_0) >> 16U) & 255U) / 255.0f;
    float bd_0 = grass_rand_0(grass_hash_0((c_30.seed_0) ^ (k_15 * 2246822507U + 19088743U)));
    if(bd_0 < dry_3)
    {
        alb_3 = mix(alb_4, (float4((&_S989)->dry_1) ).xyz * float3(mix(0.85000002384185791f, 1.1000000238418579f, bd_0 / max(dry_3, 0.0010000000474974513f))) , float3(mix(mix(0.55000001192092896f, 1.0f, t_18), 0.80000001192092896f, avg_0)) );
    }
    else
    {
        alb_3 = alb_4;
    }
    float3 _S1001 = min(alb_3, float3(0.89999997615814209f, 0.89999997615814209f, 0.89999997615814209f));
    float ao_r_0 = mix(0.55000001192092896f, 0.2800000011920929f, _S993.t_ao_0);
    float ao_b_0 = mix(mix(ao_r_0, 1.0f, smoothstep(0.0f, 0.75f, t_18)), mix(ao_r_0, 1.0f, 0.69999998807907104f), max(far_6, avg_0));
    float zlin_3 = - (((float4(p_23, 1.0f)) * (matrix<float,int(4),int(4)> (f_64->view_0.data_0[int(0)][int(0)], f_64->view_0.data_0[int(1)][int(0)], f_64->view_0.data_0[int(2)][int(0)], f_64->view_0.data_0[int(3)][int(0)], f_64->view_0.data_0[int(0)][int(1)], f_64->view_0.data_0[int(1)][int(1)], f_64->view_0.data_0[int(2)][int(1)], f_64->view_0.data_0[int(3)][int(1)], f_64->view_0.data_0[int(0)][int(2)], f_64->view_0.data_0[int(1)][int(2)], f_64->view_0.data_0[int(2)][int(2)], f_64->view_0.data_0[int(3)][int(2)], f_64->view_0.data_0[int(0)][int(3)], f_64->view_0.data_0[int(1)][int(3)], f_64->view_0.data_0[int(2)][int(3)], f_64->view_0.data_0[int(3)][int(3)])))).z;
    float2 _S1002 = i_30->pos_0.xy;
    float _S1003 = sun_shadow_0(f_64, kernelContext_72->ed_c_0->shadow_0, p_23, _S993.up_8, zlin_3, _S1002, kernelContext_72);
    float2 _S1004 = ao_upsample_0(f_64, kernelContext_72->ed_c_0->ao_0, _S1002, zlin_3, kernelContext_72);
    float3 _S1005 = sun_light_at_0(f_64, p_23, kernelContext_72);
    float _S1006 = _S1004.x;
    float _S1007 = _S1004.y;
    float3 _S1008 = shade_foliage_k_0(f_64, _S1005, _S1001, 0.5f, _S1006 * ao_b_0, _S1007, _S1003, n_19, n_19, v_16, true, (float4((&_S989)->shell_1) ).w, kernelContext_72);
    float3 _S1009 = apply_aerial_0(f_64, _S1008, p_23, _S1002, kernelContext_72);
    float3 col_6;
    if(((uint4((&f_64->light_0)->flags_0) ).w) == 2U)
    {
        col_6 = float3((0.5f * _S1006 * ao_b_0)) ;
    }
    else
    {
        if(((uint4((&f_64->light_0)->flags_0) ).w) == 4U)
        {
            col_6 = float3((0.5f * _S1007)) ;
        }
        else
        {
            if(((uint4((&f_64->light_0)->flags_0) ).w) == 1U)
            {
                col_6 = float3((0.039999999105930328f + 0.5f * _S1003)) ;
            }
            else
            {
                col_6 = _S1009;
            }
        }
    }
    return float4(col_6, 1.0f);
}

struct GpuTree_0
{
    float px_6;
    float py_1;
    float pz_1;
    uint node_kind_2;
    float scale_1;
    uint up_10;
    uint yaw_widen_0;
    uint user_0;
    uint season_1;
};

GpuTree_0 device* ed_buf_7(uint i_31, KernelContext_0 thread* kernelContext_73)
{
    return (GpuTree_0 device*)kernelContext_73->ed_buffers_0[i_31];
}

struct GpuImpKind_natural_0
{
    packed_uint4 tex_1;
    packed_float4 geo_0;
    packed_float4 leaf_2;
    packed_float4 bark_0;
};

GpuImpKind_natural_0 device* ed_buf_8(uint i_32, KernelContext_0 thread* kernelContext_74)
{
    return (GpuImpKind_natural_0 device*)kernelContext_74->ed_buffers_0[i_32];
}

struct ImpTree_0
{
    float3 c_31;
    float3 ex_0;
    float3 up_11;
    float3 ez_0;
    float s_24;
    float wd_1;
    float r_17;
    float3 cam_0;
    uint3 fi_0;
    float3 w_22;
    float3 d0_0;
    float3 d1_0;
    float3 d2_0;
};

float3 arc_from_y_0(float3 up_12, float3 v_17)
{
    float3 a_24 = float3(up_12.z, 0.0f, - up_12.x);
    float w_23 = 1.0f + up_12.y;
    float l_10 = sqrt(dot(a_24, a_24) + w_23 * w_23);
    if(l_10 < 9.99999997475242708e-07f)
    {
        return float3(v_17.x, - v_17.y, - v_17.z);
    }
    float3 a_25 = a_24 / float3(l_10) ;
    float3 t_19 = float3(2.0f)  * cross(a_25, v_17);
    return v_17 + float3((w_23 / l_10))  * t_19 + cross(a_25, t_19);
}

float3 imp_to_tree_0(const ImpTree_0 thread* t_20, float3 v_18)
{
    float _S1010 = t_20->s_24 * t_20->wd_1;
    return float3(dot(v_18, t_20->ex_0) / _S1010, dot(v_18, t_20->up_11) / t_20->s_24, dot(v_18, t_20->ez_0) / _S1010);
}

float2 imp_oct_encode_0(float3 d_57)
{
    thread float3 _S1011 = d_57;
    _S1011.y = max(d_57.y, 0.0f);
    float3 _S1012 = _S1011 / float3(max(abs(_S1011.x) + abs(_S1011.y) + abs(_S1011.z), 9.99999971718068537e-10f)) ;
    _S1011 = _S1012;
    float _S1013 = _S1012.x;
    float _S1014 = _S1012.z;
    float2 _S1015 = float2(0.5f) ;
    return float2(_S1013 + _S1014, _S1013 - _S1014) * _S1015 + _S1015;
}

float3 imp_oct_decode_0(float2 uv_4)
{
    float2 g_20 = uv_4 * float2(2.0f)  - float2(1.0f) ;
    float _S1016 = g_20.x;
    float _S1017 = g_20.y;
    float x_6 = (_S1016 + _S1017) * 0.5f;
    float z_0 = (_S1016 - _S1017) * 0.5f;
    return normalize(float3(x_6, 1.0f - abs(x_6) - abs(z_0), z_0));
}

ImpTree_0 imp_tree_0(const GpuTree_0 thread* g_21, const GpuImpKind_natural_0 thread* k_16, float3 node_2, float3 sun_dir_1, bool sun_8)
{
    thread ImpTree_0 t_21;
    float3 _S1018 = oct_decode_0(g_21->up_10);
    (&t_21)->up_11 = _S1018;
    float yaw_0 = float((g_21->yaw_widen_0) & 65535U) / 65535.0f * 6.2831854820251465f;
    (&t_21)->wd_1 = 0.5f + float((g_21->yaw_widen_0) >> 16U) / 65535.0f;
    (&t_21)->s_24 = g_21->scale_1;
    float _S1019 = cos(yaw_0);
    float _S1020 = sin(yaw_0);
    (&t_21)->ex_0 = arc_from_y_0(_S1018, float3(_S1019, 0.0f, - _S1020));
    (&t_21)->ez_0 = arc_from_y_0(_S1018, float3(_S1020, 0.0f, _S1019));
    float4 _S1021 = float4(k_16->geo_0) ;
    (&t_21)->r_17 = _S1021.y;
    (&t_21)->c_31 = node_2 + float3(g_21->px_6, g_21->py_1, g_21->pz_1) + _S1018 * float3((_S1021.x * g_21->scale_1)) ;
    ImpTree_0 _S1022 = t_21;
    float3 _S1023;
    if(sun_8)
    {
        _S1023 = sun_dir_1 * float3(1.0e+05f) ;
    }
    else
    {
        _S1023 = - (&t_21)->c_31;
    }
    thread ImpTree_0 _S1024 = _S1022;
    float3 _S1025 = imp_to_tree_0(&_S1024, _S1023);
    (&t_21)->cam_0 = _S1025;
    uint _S1026 = (uint4(k_16->tex_1) ).w;
    float n_20 = float(_S1026);
    float2 _S1027 = float2(n_20) ;
    float2 _S1028 = float2(0.5f) ;
    float _S1029 = n_20 - 1.0f;
    float2 gpos_0 = clamp(imp_oct_encode_0(normalize(_S1025)) * _S1027 - _S1028, float2(0.0f, 0.0f), float2(_S1029, _S1029));
    float _S1030 = n_20 - 2.0f;
    float2 _S1031 = min(floor(gpos_0), float2(_S1030, _S1030));
    float2 fr_0 = gpos_0 - _S1031;
    uint2 _S1032 = uint2(_S1031);
    uint2 c1_0 = _S1032 + uint2(1U, 0U);
    uint2 c2_0 = _S1032 + uint2(0U, 1U);
    float _S1033 = fr_0.x;
    float _S1034 = fr_0.y;
    float _S1035 = _S1033 + _S1034;
    uint2 c0_2;
    if(_S1035 < 1.0f)
    {
        (&t_21)->w_22 = float3(1.0f - _S1033 - _S1034, _S1033, _S1034);
        c0_2 = _S1032;
    }
    else
    {
        uint2 _S1036 = _S1032 + uint2(1U, 1U);
        (&t_21)->w_22 = float3(_S1035 - 1.0f, 1.0f - _S1034, 1.0f - _S1033);
        c0_2 = _S1036;
    }
    (&t_21)->fi_0 = uint3(c0_2.y * _S1026 + c0_2.x, c1_0.y * _S1026 + c1_0.x, c2_0.y * _S1026 + c2_0.x);
    (&t_21)->d0_0 = imp_oct_decode_0((float2(c0_2) + _S1028) / _S1027);
    (&t_21)->d1_0 = imp_oct_decode_0((float2(c1_0) + _S1028) / _S1027);
    (&t_21)->d2_0 = imp_oct_decode_0((float2(c2_0) + _S1028) / _S1027);
    return t_21;
}

void imp_view_basis_0(float3 v_19, float3 thread* right_0, float3 thread* up_13)
{
    float3 u0_0 = float3(0.0f, 1.0f, 0.0f) - v_19 * float3(v_19.y) ;
    float3 _S1037;
    if((dot(u0_0, u0_0)) > 9.99999997475242708e-07f)
    {
        _S1037 = normalize(u0_0);
    }
    else
    {
        _S1037 = float3(0.0f, 0.0f, 1.0f);
    }
    *up_13 = _S1037;
    *right_0 = normalize(cross(_S1037, v_19));
    return;
}

float2 imp_frame_uv_0(const ImpTree_0 thread* t_22, float3 d_58, float3 q_23, float half_size_0)
{
    float3 _S1038 = t_22->cam_0;
    float3 dir_14 = q_23 - t_22->cam_0;
    float den_0 = dot(dir_14, d_58);
    float k_17;
    if((abs(den_0)) > 9.99999997475242708e-07f)
    {
        k_17 = - dot(_S1038, d_58) / den_0;
    }
    else
    {
        k_17 = 1.0f;
    }
    float3 h_44 = _S1038 + dir_14 * float3(k_17) ;
    thread float3 r_18;
    thread float3 u_2;
    imp_view_basis_0(d_58, &r_18, &u_2);
    float _S1039 = 2.0f * half_size_0;
    return float2(0.5f + dot(h_44, r_18) / _S1039, 0.5f - dot(h_44, u_2) / _S1039);
}

float2 imp_atlas_uv_0(uint fi_1, uint n_21, float2 uv_5, float frame_px_0)
{
    float m_8 = 0.5f / frame_px_0;
    float _S1040 = 1.0f - m_8;
    float2 l_11 = clamp(uv_5, float2(m_8, m_8), float2(_S1040, _S1040));
    uint _S1041 = fi_1 % n_21;
    float _S1042 = float(_S1041);
    uint _S1043 = fi_1 / n_21;
    return (float2(_S1042, float(_S1043)) + l_11) / float2(float(n_21)) ;
}

bool imp_inside_0(float2 uv_6)
{
    bool _S1044;
    if(all(uv_6 >= (float2(0.0f) )))
    {
        _S1044 = all(uv_6 <= (float2(1.0f) ));
    }
    else
    {
        _S1044 = false;
    }
    return _S1044;
}

float3 imp_oct_sphere_0(float2 e_14)
{
    float2 f_65 = e_14 * float2(2.0f)  - float2(1.0f) ;
    float _S1045 = f_65.x;
    float _S1046 = f_65.y;
    float _S1047 = 1.0f - abs(_S1045) - abs(_S1046);
    thread float3 n_22 = float3(_S1045, _S1047, _S1046);
    float t_23 = saturate(- _S1047);
    float _S1048;
    if(_S1045 >= 0.0f)
    {
        _S1048 = - t_23;
    }
    else
    {
        _S1048 = t_23;
    }
    n_22.x = n_22.x + _S1048;
    if((n_22.z) >= 0.0f)
    {
        _S1048 = - t_23;
    }
    else
    {
        _S1048 = t_23;
    }
    n_22.z = n_22.z + _S1048;
    return normalize(n_22);
}

float3 tree_hue_0(float h_45)
{
    float3 _S1049;
    if(h_45 >= 0.0f)
    {
        _S1049 = mix(float3(1.0f, 1.0f, 1.0f), float3(0.85000002384185791f, 0.97000002861022949f, 1.2000000476837158f), float3(h_45) );
    }
    else
    {
        _S1049 = mix(float3(1.0f, 1.0f, 1.0f), float3(1.1200000047683716f, 1.0f, 0.75f), float3(- h_45) );
    }
    return _S1049;
}

LeafSeason_0 season_state_1(FrameData_natural_0 device* f_66, uint row_1, float shift_1, bool south_1)
{
    if(((float4((&f_66->season_0)->cfg_4) ).x) <= 0.0f)
    {
        return season_off_0();
    }
    float d_59 = (float4((&f_66->season_0)->cfg_4) ).x;
    float d_60;
    if(south_1)
    {
        d_60 = fmod(d_59 + 182.625f, 365.25f);
    }
    else
    {
        d_60 = d_59;
    }
    float ds_1 = d_60 - shift_1;
    float da_1 = d_60 + (float4((&f_66->season_0)->cfg_4) ).y * shift_1;
    uint _S1050 = (row_1 & 31U) * 2U;
    float4 _S1051 = float4((&(&f_66->season_0)->rows_0)->data_5[_S1050]) ;
    float4 _S1052 = float4((&(&f_66->season_0)->rows_0)->data_5[_S1050 + 1U]) ;
    thread LeafSeason_0 s_25;
    (&s_25)->target_0 = _S1052.xyz;
    float _S1053 = _S1051.w;
    (&s_25)->evergreen_0 = _S1053;
    if(_S1053 > 0.5f)
    {
        float _S1054 = _S1051.y;
        float a_26 = season_ramp_0(_S1054 - 10.0f, _S1054 + 30.0f, da_1);
        float _S1055 = _S1051.x;
        float rel_2 = 1.0f - season_ramp_0(_S1055 - 10.0f, _S1055 + 20.0f, ds_1);
        (&s_25)->green_0 = 1.0f;
        (&s_25)->leaves_0 = 1.0f;
        (&s_25)->colour_0 = _S1052.w * max(a_26, rel_2);
    }
    else
    {
        float _S1056 = _S1051.x;
        float _S1057 = season_ramp_0(_S1056 - 6.0f, _S1056 + 8.0f, ds_1);
        (&s_25)->green_0 = _S1057;
        float _S1058 = _S1051.y;
        (&s_25)->colour_0 = season_ramp_0(_S1058 - 8.0f, _S1058 + 14.0f, da_1);
        float _S1059 = _S1051.z;
        (&s_25)->leaves_0 = _S1057 * (1.0f - season_ramp_0(_S1059 - 10.0f, _S1059 + 10.0f, da_1));
    }
    return s_25;
}

LeafSeason_0 season_of_word_0(FrameData_natural_0 device* f_67, uint word_0)
{
    LeafSeason_0 _S1060 = season_state_1(f_67, word_0 & 255U, float(int(word_0) >> 16U) * 0.0625f, ((word_0 >> 8U) & 1U) != 0U);
    return _S1060;
}

float3 shade_foliage_k_1(FrameData_natural_0 device* f_68, float3 sun_9, float3 albedo_12, float rough_8, float ao_11, float sv_8, float sun_vis_5, float3 n_23, float3 ng_7, float3 v_20, bool leaf_3, float spec_k_3, KernelContext_0 thread* kernelContext_75)
{
    float3 _S1061 = shade_pbr_k_0(f_68, sun_9, albedo_12, 0.0f, rough_8, ao_11, sv_8, sun_vis_5, n_23, v_20, spec_k_3, kernelContext_75);
    float3 col_7;
    if(leaf_3)
    {
        float3 l_12 = (float4(f_68->sun_dir_0) ).xyz;
        float3 trans_1 = albedo_12 * float3(1.1499999761581421f, 1.25f, 0.55000001192092896f);
        float3 col_8 = _S1061 + sun_9 * float3(sun_vis_5)  * trans_1 * float3((saturate(- dot(n_23, l_12)) * (0.44999998807907104f + 1.2000000476837158f * pow(saturate(dot(- v_20, l_12)), 4.0f))))  * float3(0.31830987334251404f) ;
        float3 _S1062 = hemi_ambient_0(f_68, - n_23);
        col_7 = col_8 + trans_1 * _S1062 * float3(mix((float4((&f_68->light_0)->skyvis_0) ).y, 1.0f, sv_8))  * float3(ao_11)  * float3(0.30000001192092896f) ;
    }
    else
    {
        col_7 = _S1061;
    }
    return col_7;
}

float3 shade_foliage_0(FrameData_natural_0 device* f_69, float3 sun_10, float3 albedo_13, float rough_9, float ao_12, float sv_9, float sun_vis_6, float3 n_24, float3 ng_8, float3 v_21, bool leaf_4, KernelContext_0 thread* kernelContext_76)
{
    float3 _S1063 = shade_foliage_k_1(f_69, sun_10, albedo_13, rough_9, ao_12, sv_9, sun_vis_6, n_24, ng_8, v_21, leaf_4, 1.0f, kernelContext_76);
    return _S1063;
}

float4 impostor_pixel_0(FrameData_natural_0 device* f_70, const FsOut_0 thread* i_33, uint idx_0, float3 dir_15, KernelContext_0 thread* kernelContext_77)
{
    GpuTree_0 device* _S1064 = ed_buf_7((uint4((&f_70->imp_0)->bufs_1) ).x, kernelContext_77);
    GpuTree_0 g_22 = *(_S1064 + idx_0);
    GpuImpKind_natural_0 device* _S1065 = ed_buf_8((uint4((&f_70->imp_0)->bufs_1) ).y, kernelContext_77);
    GpuImpKind_natural_0 device* _S1066 = _S1065 + (((g_22.node_kind_2) >> 12U) & 4095U);
    GpuImpKind_natural_0 k_18 = *_S1066;
    thread GpuImpKind_natural_0 _S1067 = *_S1066;
    packed_float4 device* _S1068 = ed_buf_4(kernelContext_77->ed_c_0->nodes_0, kernelContext_77);
    float3 node_3 = (float4(*(_S1068 + ((g_22.node_kind_2) & 4095U))) ).xyz;
    float3 _S1069 = (float4(f_70->sun_dir_0) ).xyz;
    thread GpuTree_0 _S1070 = g_22;
    _S1067 = k_18;
    ImpTree_0 _S1071 = imp_tree_0(&_S1070, &_S1067, node_3, _S1069, false);
    float3 nrm_2 = normalize(_S1071.c_31);
    float hit_2 = dot(_S1071.c_31, nrm_2) / max(dot(dir_15, nrm_2), 9.9999997473787516e-05f);
    float3 pq_0 = dir_15 * float3(hit_2) ;
    float3 _S1072 = pq_0 - _S1071.c_31;
    thread ImpTree_0 _S1073 = _S1071;
    float3 _S1074 = imp_to_tree_0(&_S1073, _S1072);
    float half_t_0 = _S1071.r_17 * (float4((&f_70->imp_0)->cfg_3) ).y;
    thread ImpTree_0 _S1075 = _S1071;
    float2 _S1076 = imp_frame_uv_0(&_S1075, _S1071.d0_0, _S1074, half_t_0);
    thread ImpTree_0 _S1077 = _S1071;
    float2 _S1078 = imp_frame_uv_0(&_S1077, _S1071.d1_0, _S1074, half_t_0);
    thread ImpTree_0 _S1079 = _S1071;
    float2 _S1080 = imp_frame_uv_0(&_S1079, _S1071.d2_0, _S1074, half_t_0);
    uint4 _S1081 = uint4((&_S1067)->tex_1) ;
    uint n_25 = _S1081.w;
    float fpx_0 = (float4((&_S1067)->geo_0) ).w;
    float4 _S1082 = float4((&_S1067)->leaf_2) ;
    float lod_2 = clamp(log2(max(hit_2 / ((float4(f_70->proj_0) ).w * 0.5f * (float4(f_70->proj_0) ).y), 9.99999997475242708e-07f) / (2.0f * half_t_0 * _S1071.s_24 * max(_S1071.wd_1, 1.0f) / fpx_0)), 0.0f, _S1082.w - 1.0f);
    sampler _S1083 = ((reinterpret_cast<device sampler*>((kernelContext_77->ed_samplers_0))[(kernelContext_77->ed_c_0->smp_0)]));
    texture2d<float, access::sample> _S1084 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_77->ed_textures_0))[(_S1081.x)]));
    texture2d<float, access::sample> _S1085 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_77->ed_textures_0))[(_S1081.y)]));
    texture2d<float, access::sample> _S1086 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_77->ed_textures_0))[(_S1081.z)]));
    float2 a0_1 = imp_atlas_uv_0(_S1071.fi_0.x, n_25, _S1076, fpx_0);
    float2 a1_1 = imp_atlas_uv_0(_S1071.fi_0.y, n_25, _S1078, fpx_0);
    float2 a2_2 = imp_atlas_uv_0(_S1071.fi_0.z, n_25, _S1080, fpx_0);
    float2 s0_1 = ((_S1084).sample((_S1083), (a0_1), level((lod_2)))).xy;
    float2 s1_2 = ((_S1084).sample((_S1083), (a1_1), level((lod_2)))).xy;
    float2 s2_1 = ((_S1084).sample((_S1083), (a2_2), level((lod_2)))).xy;
    float _S1087 = s0_1.x;
    float _S1088;
    if(imp_inside_0(_S1076))
    {
        _S1088 = 1.0f;
    }
    else
    {
        _S1088 = 0.0f;
    }
    float _S1089 = _S1087 * _S1088;
    float _S1090 = s1_2.x;
    if(imp_inside_0(_S1078))
    {
        _S1088 = 1.0f;
    }
    else
    {
        _S1088 = 0.0f;
    }
    float _S1091 = _S1090 * _S1088;
    float _S1092 = s2_1.x;
    if(imp_inside_0(_S1080))
    {
        _S1088 = 1.0f;
    }
    else
    {
        _S1088 = 0.0f;
    }
    float3 wc_0 = _S1071.w_22 * float3(_S1089, _S1091, _S1092 * _S1088);
    float ws_1 = wc_0.x + wc_0.y + wc_0.z;
    float3 wc_1;
    if(ws_1 > 9.9999997473787516e-05f)
    {
        wc_1 = wc_0 / float3(ws_1) ;
    }
    else
    {
        wc_1 = _S1071.w_22;
    }
    float _S1093 = wc_1.x;
    float _S1094 = wc_1.y;
    float _S1095 = wc_1.z;
    float3 nt_0 = float3(_S1093)  * imp_oct_sphere_0(((_S1085).sample((_S1083), (a0_1), level((lod_2)))).xy) + float3(_S1094)  * imp_oct_sphere_0(((_S1085).sample((_S1083), (a1_1), level((lod_2)))).xy) + float3(_S1095)  * imp_oct_sphere_0(((_S1085).sample((_S1083), (a2_2), level((lod_2)))).xy);
    float2 vb_0 = float2(_S1093)  * ((_S1086).sample((_S1083), (a0_1), level((lod_2)))).xy + float2(_S1094)  * ((_S1086).sample((_S1083), (a1_1), level((lod_2)))).xy + float2(_S1095)  * ((_S1086).sample((_S1083), (a2_2), level((lod_2)))).xy;
    float3 p_24 = pq_0 - dir_15 * float3(((_S1093 * s0_1.y + _S1094 * s1_2.y + _S1095 * s2_1.y - 0.5f) * 2.0f * half_t_0 * _S1071.s_24)) ;
    float3 nw_1 = _S1071.ex_0 * float3((nt_0.x / _S1071.wd_1))  + _S1071.up_11 * float3(nt_0.y)  + _S1071.ez_0 * float3((nt_0.z / _S1071.wd_1)) ;
    float3 nw_2;
    if((dot(nw_1, nw_1)) > 9.99999993922529029e-09f)
    {
        nw_2 = normalize(nw_1);
    }
    else
    {
        nw_2 = - dir_15;
    }
    float3 v_22 = - dir_15;
    float _S1096 = dot(nw_2, v_22);
    if(_S1096 < 0.0f)
    {
        nw_2 = normalize(nw_2 - float3((1.0099999904632568f * _S1096))  * v_22);
    }
    float bark_1 = saturate(vb_0.y);
    float iv_0 = float((g_22.user_0) & 255U) / 128.0f;
    float3 _S1097 = float3((vb_0.x * (float4((&f_70->imp_0)->cfg_3) ).z)) ;
    float3 leaf_alb_0 = _S1082.xyz * _S1097 * float3(iv_0)  * tree_hue_0(clamp(float(((g_22.user_0) >> 8U) & 255U) / 127.5f - 1.0f, -1.0f, 1.0f));
    float3 leaf_alb_1;
    if(((float4((&f_70->season_0)->cfg_4) ).x) > 0.0f)
    {
        LeafSeason_0 _S1098 = season_of_word_0(f_70, g_22.season_1);
        thread LeafSeason_0 _S1099 = _S1098;
        float3 _S1100 = season_tint_0(&_S1099, leaf_alb_0);
        leaf_alb_1 = _S1100;
    }
    else
    {
        leaf_alb_1 = leaf_alb_0;
    }
    float3 _S1101 = min(mix(leaf_alb_1, (float4((&_S1067)->bark_0) ).xyz * _S1097 * float3(mix(1.0f, iv_0, 0.5f)) , float3(bark_1) ), float3(0.89999997615814209f, 0.89999997615814209f, 0.89999997615814209f));
    float zlin_4 = - (((float4(p_24, 1.0f)) * (matrix<float,int(4),int(4)> (f_70->view_0.data_0[int(0)][int(0)], f_70->view_0.data_0[int(1)][int(0)], f_70->view_0.data_0[int(2)][int(0)], f_70->view_0.data_0[int(3)][int(0)], f_70->view_0.data_0[int(0)][int(1)], f_70->view_0.data_0[int(1)][int(1)], f_70->view_0.data_0[int(2)][int(1)], f_70->view_0.data_0[int(3)][int(1)], f_70->view_0.data_0[int(0)][int(2)], f_70->view_0.data_0[int(1)][int(2)], f_70->view_0.data_0[int(2)][int(2)], f_70->view_0.data_0[int(3)][int(2)], f_70->view_0.data_0[int(0)][int(3)], f_70->view_0.data_0[int(1)][int(3)], f_70->view_0.data_0[int(2)][int(3)], f_70->view_0.data_0[int(3)][int(3)])))).z;
    float2 _S1102 = i_33->pos_0.xy;
    float _S1103 = sun_shadow_0(f_70, kernelContext_77->ed_c_0->shadow_0, p_24, nw_2, zlin_4, _S1102, kernelContext_77);
    float2 _S1104 = ao_upsample_0(f_70, kernelContext_77->ed_c_0->ao_0, _S1102, zlin_4, kernelContext_77);
    float3 _S1105 = sun_light_at_0(f_70, p_24, kernelContext_77);
    float _S1106 = _S1104.x;
    float _S1107 = _S1104.y;
    float3 _S1108 = shade_foliage_0(f_70, _S1105, _S1101, 0.81999999284744263f, _S1106, _S1107, _S1103, nw_2, nw_2, v_22, bark_1 < 0.5f, kernelContext_77);
    float3 _S1109 = apply_aerial_0(f_70, _S1108, p_24, _S1102, kernelContext_77);
    float3 col_9;
    if(((uint4((&f_70->light_0)->flags_0) ).w) == 2U)
    {
        col_9 = float3((0.5f * _S1106)) ;
    }
    else
    {
        if(((uint4((&f_70->light_0)->flags_0) ).w) == 4U)
        {
            col_9 = float3((0.5f * _S1107)) ;
        }
        else
        {
            if(((uint4((&f_70->light_0)->flags_0) ).w) == 1U)
            {
                col_9 = float3((0.039999999105930328f + 0.5f * _S1103)) ;
            }
            else
            {
                col_9 = _S1109;
            }
        }
    }
    return float4(col_9, 1.0f);
}

struct _Array_natural_vectorx3Cfloatx2C4x3E3_0
{
    array<packed_float4, int(3)> data_7;
};

struct GpuInstance_natural_0
{
    _Array_natural_vectorx3Cfloatx2C4x3E3_0 m_9;
    _Array_natural_vectorx3Cfloatx2C4x3E3_0 prev_0;
    uint mesh_0;
    uint material_0;
    uint object_lo_0;
    uint node_flags_0;
    array<uint, int(4)> pad_2;
};

GpuInstance_natural_0 device* ed_buf_9(uint i_34, KernelContext_0 thread* kernelContext_78)
{
    return (GpuInstance_natural_0 device*)kernelContext_78->ed_buffers_0[i_34];
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

GpuMesh_natural_0 device* ed_buf_10(uint i_35, KernelContext_0 thread* kernelContext_79)
{
    return (GpuMesh_natural_0 device*)kernelContext_79->ed_buffers_0[i_35];
}

struct Vtx_0
{
    float px_7;
    float py_2;
    float pz_2;
    uint n_26;
};

Vtx_0 device* ed_buf_11(uint i_36, KernelContext_0 thread* kernelContext_80)
{
    return (Vtx_0 device*)kernelContext_80->ed_buffers_0[i_36];
}

packed_uint2 device* ed_buf_12(uint i_37, KernelContext_0 thread* kernelContext_81)
{
    return (packed_uint2 device*)kernelContext_81->ed_buffers_0[i_37];
}

float gc_keep_0(float d_61, float reach_0)
{
    float r0_1 = 0.19499999284744263f * reach_0;
    float k_19 = r0_1 / max(d_61, r0_1);
    return k_19 * k_19 * (1.0f - smoothstep(0.69999998807907104f * reach_0, reach_0, d_61));
}

float3 gc_scale_0(const GpuInstance_natural_0 thread* inst_0, float d_62)
{
    uint _S1110 = inst_0->pad_2[int(2)];
    if((((inst_0->pad_2[int(2)]) >> 20U) & 1U) != 0U)
    {
        return float3(1.0f) ;
    }
    float k_20 = gc_keep_0(d_62, float((_S1110 >> 12U) & 255U));
    float s_26 = saturate((k_20 - float(_S1110 & 4095U) * 0.00024420025874860585f) / (0.30000001192092896f * k_20 + 0.0010000000474974513f));
    if(s_26 <= 0.0f)
    {
        return float3(0.0f) ;
    }
    float gx_2 = clamp(sqrt(1.0f / max(k_20, 9.9999997473787516e-05f)), 1.0f, 2.0f);
    return float3(gx_2, min(gx_2, 1.25f), gx_2) * float3(s_26) ;
}

float3 xform_point_0(const GpuInstance_natural_0 thread* i_38, float3 p_25)
{
    float4 _S1111 = float4((&i_38->m_9)->data_7[int(0)]) ;
    float4 _S1112 = float4((&i_38->m_9)->data_7[int(1)]) ;
    float4 _S1113 = float4((&i_38->m_9)->data_7[int(2)]) ;
    return float3(dot(_S1111.xyz, p_25) + _S1111.w, dot(_S1112.xyz, p_25) + _S1112.w, dot(_S1113.xyz, p_25) + _S1113.w);
}

float max_scale_0(const GpuInstance_natural_0 thread* i_39)
{
    float4 _S1114 = float4((&i_39->m_9)->data_7[int(0)]) ;
    float4 _S1115 = float4((&i_39->m_9)->data_7[int(1)]) ;
    float4 _S1116 = float4((&i_39->m_9)->data_7[int(2)]) ;
    return sqrt(max(max(length_squared_0(float3(_S1114.x, _S1115.x, _S1116.x)), length_squared_0(float3(_S1114.y, _S1115.y, _S1116.y))), length_squared_0(float3(_S1114.z, _S1115.z, _S1116.z))));
}

float3 xform_normal_0(const GpuInstance_natural_0 thread* i_40, float3 n_27)
{
    float4 _S1117 = float4((&i_40->m_9)->data_7[int(0)]) ;
    float4 _S1118 = float4((&i_40->m_9)->data_7[int(1)]) ;
    float4 _S1119 = float4((&i_40->m_9)->data_7[int(2)]) ;
    float3 a0_2 = float3(_S1117.x, _S1118.x, _S1119.x);
    float3 a1_2 = float3(_S1117.y, _S1118.y, _S1119.y);
    float3 a2_3 = float3(_S1117.z, _S1118.z, _S1119.z);
    return cross(a1_2, a2_3) * float3(n_27.x)  + cross(a2_3, a0_2) * float3(n_27.y)  + cross(a0_2, a1_2) * float3(n_27.z) ;
}

float3 tree_vertex_0(FrameData_natural_0 device* f_71, const GpuInstance_natural_0 thread* inst_1, float3 node_4, float3 pm_0, uint n_28, uint aux_0)
{
    uint _S1120 = inst_1->pad_2[int(0)];
    float3 origin_1 = float3((float4((&inst_1->m_9)->data_7[int(0)]) ).w, (float4((&inst_1->m_9)->data_7[int(1)]) ).w, (float4((&inst_1->m_9)->data_7[int(2)]) ).w);
    bool _S1121;
    if((aux_0 & 134217728U) != 0U)
    {
        _S1121 = ((_S1120 >> 16U) & (1U << (aux_0 >> 29U))) != 0U;
    }
    else
    {
        _S1121 = false;
    }
    if(_S1121)
    {
        return origin_1;
    }
    float3 _S1122;
    if(((inst_1->node_flags_0) & 131072U) != 0U)
    {
        float3 _S1123 = gc_scale_0(inst_1, length(node_4 + origin_1));
        if((_S1123.x) <= 0.0f)
        {
            return origin_1;
        }
        _S1122 = pm_0 * _S1123;
    }
    else
    {
        _S1122 = pm_0;
    }
    float3 _S1124 = xform_point_0(inst_1, _S1122);
    WindData_natural_0 w_24 = f_71->wind_1;
    float4 _S1125 = float4(f_71->wind_1.cfg_2) ;
    if((_S1125.x) < 0.5f)
    {
        return _S1124;
    }
    float3 _S1126 = node_4 + origin_1;
    float _S1127 = _S1125.y;
    float fade_2 = saturate((_S1127 - length(_S1126)) / max(0.25f * _S1127, 1.0f));
    if(fade_2 <= 0.0f)
    {
        return _S1124;
    }
    float q_24 = float((aux_0 >> 11U) & 127U) / 127.0f;
    float sway_0 = q_24 * q_24 * 4.0f;
    float flutter_0 = float((aux_0 >> 18U) & 7U) / 7.0f;
    float phase_0 = float((aux_0 >> 21U) & 63U) / 64.0f + float(_S1120 >> 24U) / 255.0f;
    float4 _S1128 = float4(w_24.wind_0) ;
    float2 _S1129 = _S1128.xz;
    float speed_1 = length(_S1129);
    float2 wd_2;
    if(speed_1 > 0.0010000000474974513f)
    {
        wd_2 = _S1129 / float2(speed_1) ;
    }
    else
    {
        wd_2 = float2(0.0f, 0.0f);
    }
    float4 _S1130 = float4(w_24.gust_0) ;
    float s_27 = 0.5f + 0.5f * sin((_S1130.x + dot(_S1126.xz, wd_2) / max(_S1130.y, 1.0f)) * 6.2831854820251465f);
    float gust_1 = s_27 * s_27 * s_27 * _S1130.z;
    float t_24 = _S1128.w;
    float _S1131 = speed_1 * 0.10000000149011612f * fade_2;
    float _S1132 = max_scale_0(inst_1);
    float k_21 = _S1131 * _S1132;
    float rock_1 = 0.55000001192092896f + gust_1 + 0.30000001192092896f * sin(t_24 * 1.7000000476837158f + phase_0 * 6.2831854820251465f) + 0.15000000596046448f * sin(t_24 * 0.62999999523162842f + phase_0 * 19.0f);
    thread float3 d_63 = float3(wd_2.x, 0.0f, wd_2.y) * float3((sway_0 * k_21 * rock_1)) ;
    d_63.y = d_63.y - 0.25f * sway_0 * k_21 * rock_1 * rock_1 * 0.30000001192092896f;
    float3 _S1133 = xform_normal_0(inst_1, oct_decode_0(n_28));
    return _S1124 + d_63 + normalize(_S1133) * float3((flutter_0 * _S1130.w * (0.60000002384185791f + gust_1) * 0.05000000074505806f * fade_2 * sin(t_24 * 11.0f + phase_0 * 40.0f + dot(_S1124, float3(3.0999999046325684f, 1.7000000476837158f, 2.2999999523162842f)))))  * float3(_S1132) ;
}

struct GpuMaterial_natural_0
{
    packed_float4 base_3;
    packed_float4 params_1;
    packed_float4 shade_1;
    packed_uint4 tex_2;
    packed_float4 uv_7;
};

GpuMaterial_natural_0 device* ed_buf_13(uint i_41, KernelContext_0 thread* kernelContext_82)
{
    return (GpuMaterial_natural_0 device*)kernelContext_82->ed_buffers_0[i_41];
}

struct TreeSurf_0
{
    float3 albedo_14;
    bool leaf_5;
    bool shell_2;
};

float2 tree_uv_0(uint w_25)
{
    return float2(float(w_25 & 65535U), float(w_25 >> 16U)) * float2(1.5259021893143654e-05f) ;
}

TreeSurf_0 tree_surface_0(FrameData_natural_0 device* f_72, const GpuInstance_natural_0 thread* inst_2, const GpuMaterial_natural_0 thread* m_10, uint2 w0_2, uint2 w1_0, uint2 w2_0, float b0_0, float b1_6, float b2_9, float2 pix_0, float3 p0_2, float3 e1_4, float3 e2_4, KernelContext_0 thread* kernelContext_83)
{
    uint a_27 = w0_2.y;
    uint _S1134 = w1_0.y;
    uint _S1135 = w2_0.y;
    float val_0 = (float(a_27 & 127U) * b0_0 + float(_S1134 & 127U) * b1_6 + float(_S1135 & 127U) * b2_9) * 0.015748031437397003f;
    float hue_3 = (float((a_27 >> 7U) & 15U) * b0_0 + float((_S1134 >> 7U) & 15U) * b1_6 + float((_S1135 >> 7U) & 15U) * b2_9) * 0.13333334028720856f - 1.0f;
    float iv_1 = float((inst_2->pad_2[int(0)]) & 255U) / 128.0f;
    float ih_0 = float(((inst_2->pad_2[int(0)]) >> 8U) & 255U) / 127.5f - 1.0f;
    thread TreeSurf_0 o_7;
    (&o_7)->leaf_5 = (a_27 & 134217728U) != 0U;
    (&o_7)->shell_2 = false;
    if((a_27 & 268435456U) != 0U)
    {
        (&o_7)->albedo_14 = (float4(m_10->shade_1) ).xyz * float3(val_0)  * float3(mix(1.0f, iv_1, 0.5f)) ;
        return o_7;
    }
    float3 alb_5 = (float4(m_10->base_3) ).xyz * float3(val_0)  * float3(iv_1)  * tree_hue_0(clamp(hue_3 + 0.5f * ih_0, -1.0f, 1.0f));
    bool _S1136;
    if((&o_7)->leaf_5)
    {
        _S1136 = ((uint4(m_10->tex_2) ).x) != 4294967295U;
    }
    else
    {
        _S1136 = false;
    }
    float3 alb_6;
    if(_S1136)
    {
        float2 t0_1 = tree_uv_0(w0_2.x);
        float2 d1_1 = tree_uv_0(w1_0.x) - t0_1;
        float2 d2_1 = tree_uv_0(w2_0.x) - t0_1;
        float2 uv_8 = t0_1 + d1_1 * float2(b1_6)  + d2_1 * float2(b2_9) ;
        float3 _S1137 = view_dir_0(f_72, pix_0 + float2(1.0f, 0.0f));
        float2 bx_2 = ray_bary_0(_S1137, p0_2, e1_4, e2_4);
        float3 _S1138 = view_dir_0(f_72, pix_0 + float2(0.0f, 1.0f));
        float2 by_2 = ray_bary_0(_S1138, p0_2, e1_4, e2_4);
        float2 ux_0 = t0_1 + d1_1 * float2(bx_2.x)  + d2_1 * float2(bx_2.y)  - uv_8;
        float2 uy_0 = t0_1 + d1_1 * float2(by_2.x)  + d2_1 * float2(by_2.y)  - uv_8;
        texture2d<float, access::sample> _S1139 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_83->ed_textures_0))[((uint4(m_10->tex_2) ).x)]));
        sampler _S1140 = ((reinterpret_cast<device sampler*>((kernelContext_83->ed_samplers_0))[(kernelContext_83->ed_c_0->smp_0)]));
        alb_6 = alb_5 * clamp(((_S1139).sample((_S1140), (uv_8), gradient2d((ux_0), (uy_0)))).xyz / float3(0.14699999988079071f, 0.27399998903274536f, 0.050999999046325684f), float3(0.20000000298023224f, 0.20000000298023224f, 0.20000000298023224f), float3(2.5f, 2.5f, 2.5f));
    }
    else
    {
        alb_6 = alb_5;
    }
    if(((float4((&f_72->season_0)->cfg_4) ).x) > 0.0f)
    {
        LeafSeason_0 _S1141 = season_of_word_0(f_72, inst_2->pad_2[int(1)]);
        thread LeafSeason_0 _S1142 = _S1141;
        float3 _S1143 = season_tint_0(&_S1142, alb_6);
        alb_6 = _S1143;
    }
    (&o_7)->shell_2 = !(&o_7)->leaf_5;
    (&o_7)->albedo_14 = min(alb_6, float3(0.89999997615814209f, 0.89999997615814209f, 0.89999997615814209f));
    return o_7;
}

float bark_height_0(float2 q_25)
{
    float _S1144 = q_25.x;
    float _S1145 = q_25.y;
    float plates_0 = t_noise_0(float2(_S1144, _S1145 * 0.30000001192092896f), 0.070000000298023224f);
    float _S1146 = t_noise_0(q_25 * float2(1.0f, 0.25f) + float2(31.0f) , 0.18000000715255737f);
    float _S1147 = t_noise_0(float2(_S1144 + (_S1146 - 0.5f) * 0.079999998211860657f, _S1145 * 0.11999999731779099f), 0.035000000149011612f);
    return saturate(smoothstep(0.2199999988079071f, 0.51999998092651367f, _S1147) * (0.55000001192092896f + 0.44999998807907104f * plates_0));
}

packed_float2 device* ed_buf_14(uint i_42, KernelContext_0 thread* kernelContext_84)
{
    return (packed_float2 device*)kernelContext_84->ed_buffers_0[i_42];
}

float2 uv_xform_0(const GpuMaterial_natural_0 thread* m_11, float2 uv_9)
{
    float r_19 = (as_type<float>(((uint4(m_11->tex_2) ).w)));
    float s_28 = sin(r_19);
    float c_32 = cos(r_19);
    float4 _S1148 = float4(m_11->uv_7) ;
    float2 q_26 = uv_9 * _S1148.zw;
    float _S1149 = q_26.x;
    float _S1150 = q_26.y;
    return float2(c_32 * _S1149 + s_28 * _S1150, - s_28 * _S1149 + c_32 * _S1150) + _S1148.xy;
}

float3 shade_toon_0(FrameData_natural_0 device* f_73, float3 sun_11, const GpuMaterial_natural_0 thread* m_12, float3 albedo_15, float3 n_29, float3 v_23, float ao_13, float sv_10, float sun_vis_7, KernelContext_0 thread* kernelContext_85)
{
    float ndl_0 = dot(n_29, (float4(f_73->sun_dir_0) ).xyz);
    float _S1151 = max((float4(m_12->params_1) ).z, 1.0f);
    float lit_3 = floor(saturate(ndl_0 * 0.5f + 0.5f) * _S1151 + 0.5f) / _S1151 * sun_vis_7;
    float4 _S1152 = float4(m_12->shade_1) ;
    float3 c_33 = mix(albedo_15 * _S1152.xyz, albedo_15, float3(lit_3) );
    float3 _S1153 = sun_11 * float3((lit_3 / 3.1415927410125732f)) ;
    float3 _S1154 = ambient_light_0(f_73, n_29, kernelContext_85);
    float3 _S1155 = occluded_ambient_0(f_73, _S1154, sv_10, kernelContext_85);
    float3 _S1156 = ao_multibounce_0(f_73, ao_13, albedo_15);
    return c_33 * (_S1153 + _S1155 * _S1156) + float3((pow(1.0f - saturate(dot(n_29, v_23)), 4.0f) * _S1152.w * saturate(ndl_0 + 0.30000001192092896f) * sun_vis_7))  * sun_11 * float3(0.15000000596046448f) ;
}

float cascade_depth_delta_0(FrameData_natural_0 device* f_74, uint atlas_2, uint c_34, float3 p_26, KernelContext_0 thread* kernelContext_86)
{
    float4 _S1157 = float4((&(&f_74->light_0)->cascade_0)->data_3[c_34]) ;
    float tile_2 = (float4((&f_74->light_0)->cfg_0) ).y;
    float4 s_29 = (((float4(p_26, 1.0f)) * (matrix<float,int(4),int(4)> ((&(&f_74->light_0)->shadow_1)->data_2[c_34].data_0[int(0)][int(0)], (&(&f_74->light_0)->shadow_1)->data_2[c_34].data_0[int(1)][int(0)], (&(&f_74->light_0)->shadow_1)->data_2[c_34].data_0[int(2)][int(0)], (&(&f_74->light_0)->shadow_1)->data_2[c_34].data_0[int(3)][int(0)], (&(&f_74->light_0)->shadow_1)->data_2[c_34].data_0[int(0)][int(1)], (&(&f_74->light_0)->shadow_1)->data_2[c_34].data_0[int(1)][int(1)], (&(&f_74->light_0)->shadow_1)->data_2[c_34].data_0[int(2)][int(1)], (&(&f_74->light_0)->shadow_1)->data_2[c_34].data_0[int(3)][int(1)], (&(&f_74->light_0)->shadow_1)->data_2[c_34].data_0[int(0)][int(2)], (&(&f_74->light_0)->shadow_1)->data_2[c_34].data_0[int(1)][int(2)], (&(&f_74->light_0)->shadow_1)->data_2[c_34].data_0[int(2)][int(2)], (&(&f_74->light_0)->shadow_1)->data_2[c_34].data_0[int(3)][int(2)], (&(&f_74->light_0)->shadow_1)->data_2[c_34].data_0[int(0)][int(3)], (&(&f_74->light_0)->shadow_1)->data_2[c_34].data_0[int(1)][int(3)], (&(&f_74->light_0)->shadow_1)->data_2[c_34].data_0[int(2)][int(3)], (&(&f_74->light_0)->shadow_1)->data_2[c_34].data_0[int(3)][int(3)]))));
    int2 _S1158 = int2(_S1157.zw);
    float _S1159 = s_29.z;
    float _S1160 = ed_depth_read_0(atlas_2, uint2(clamp(_S1158 + int2(floor(float2(s_29.x * 0.5f + 0.5f, 0.5f - s_29.y * 0.5f) * float2(tile_2) )), _S1158, _S1158 + int2((int(tile_2) - int(1))) )), kernelContext_86);
    return (_S1159 - _S1160) * _S1157.y;
}

float4 resolve_px_0(const FsOut_0 thread* i_43, bool terrain_0, KernelContext_0 thread* kernelContext_87)
{
    FrameData_natural_0 device* _S1161 = ed_buf_1(kernelContext_87->ed_c_0->frame_0, kernelContext_87);
    float2 _S1162 = i_43->pos_0.xy;
    uint2 _S1163 = uint2(_S1162);
    texture2d<uint, access::sample> _S1164 = ((reinterpret_cast<device texture2d<uint, access::sample>*>((kernelContext_87->ed_textures_0))[(kernelContext_87->ed_c_0->vis_0)]));
    int3 _S1165 = int3(int2(_S1163), int(0));
    uint2 vis_5 = ((_S1164).read(vec<uint,2>(((_S1165)).xy), uint(((_S1165)).z))).xy;
    float3 _S1166 = view_dir_0(_S1161, _S1162);
    uint _S1167 = vis_5.x;
    if(_S1167 == 0U)
    {
        float3 _S1168 = sky_pixel_0(_S1161, _S1166, kernelContext_87);
        return float4(_S1168, 1.0f);
    }
    bool textured_0;
    if(terrain_0)
    {
        textured_0 = (_S1167 & 2147483648U) != 0U;
    }
    else
    {
        textured_0 = false;
    }
    if(textured_0)
    {
        float4 _S1169 = terrain_pixel_0(_S1161, i_43, vis_5, _S1166, kernelContext_87);
        return _S1169;
    }
    if((_S1167 & 3758096384U) == 536870912U)
    {
        float4 _S1170 = grass_pixel_0(_S1161, i_43, vis_5, _S1166, kernelContext_87);
        return _S1170;
    }
    if((_S1167 & 3221225472U) == 1073741824U)
    {
        float4 _S1171 = impostor_pixel_0(_S1161, i_43, _S1167 & 3221225471U, _S1166, kernelContext_87);
        return _S1171;
    }
    GpuInstance_natural_0 device* _S1172 = ed_buf_9(kernelContext_87->ed_c_0->instances_0, kernelContext_87);
    thread GpuInstance_natural_0 _S1173 = *(_S1172 + (_S1167 - 1U));
    GpuMesh_natural_0 device* _S1174 = ed_buf_10(kernelContext_87->ed_c_0->meshes_0, kernelContext_87);
    GpuMesh_natural_0 device* _S1175 = _S1174 + (&_S1173)->mesh_0;
    uint _S1176 = vis_5.y;
    uint lod_3 = _S1176 >> 28U;
    uint prim_1 = (_S1176 & 268435455U) % max(_S1175->index_count_0[lod_3] / 3U, 1U);
    uint device* _S1177 = ed_buf_0(kernelContext_87->ed_c_0->indices_0, kernelContext_87);
    uint device* idx_1 = _S1177 + _S1175->first_index_0[lod_3] + prim_1 * 3U;
    Vtx_0 device* _S1178 = ed_buf_11(kernelContext_87->ed_c_0->verts_0, kernelContext_87);
    int i0_0 = int(*(idx_1 + int(0))) + _S1175->base_vertex_0[lod_3];
    int i1_0 = int(*(idx_1 + int(1))) + _S1175->base_vertex_0[lod_3];
    int i2_0 = int(*(idx_1 + int(2))) + _S1175->base_vertex_0[lod_3];
    Vtx_0 v0_0 = *(_S1178 + i0_0);
    Vtx_0 v1_0 = *(_S1178 + i1_0);
    Vtx_0 v2_0 = *(_S1178 + i2_0);
    packed_float4 device* _S1179 = ed_buf_4(kernelContext_87->ed_c_0->nodes_0, kernelContext_87);
    uint _S1180 = (&_S1173)->node_flags_0;
    float3 node_5 = (float4(*(_S1179 + (((&_S1173)->node_flags_0) & 65535U))) ).xyz;
    bool foliage_0 = ((_S1175->flags_1) & 1U) != 0U;
    uint2 _S1181 = uint2(0U, 0U);
    float3 l0_0;
    float3 l1_0;
    float3 l2_0;
    uint2 tw0_0;
    uint2 tw1_0;
    uint2 tw2_1;
    if(foliage_0)
    {
        packed_uint2 device* _S1182 = ed_buf_12(kernelContext_87->ed_c_0->uvs_0, kernelContext_87);
        uint2 _S1183 = uint2(*(_S1182 + i0_0)) ;
        uint2 _S1184 = uint2(*(_S1182 + i1_0)) ;
        uint2 _S1185 = uint2(*(_S1182 + i2_0)) ;
        float3 _S1186 = tree_vertex_0(_S1161, &_S1173, node_5, float3(v0_0.px_7, v0_0.py_2, v0_0.pz_2), v0_0.n_26, _S1183.y);
        float3 _S1187 = tree_vertex_0(_S1161, &_S1173, node_5, float3(v1_0.px_7, v1_0.py_2, v1_0.pz_2), v1_0.n_26, _S1184.y);
        float3 _S1188 = tree_vertex_0(_S1161, &_S1173, node_5, float3(v2_0.px_7, v2_0.py_2, v2_0.pz_2), v2_0.n_26, _S1185.y);
        l0_0 = _S1186;
        l1_0 = _S1187;
        l2_0 = _S1188;
        tw0_0 = _S1183;
        tw1_0 = _S1184;
        tw2_1 = _S1185;
    }
    else
    {
        float3 _S1189 = xform_point_0(&_S1173, float3(v0_0.px_7, v0_0.py_2, v0_0.pz_2));
        float3 _S1190 = xform_point_0(&_S1173, float3(v1_0.px_7, v1_0.py_2, v1_0.pz_2));
        float3 _S1191 = xform_point_0(&_S1173, float3(v2_0.px_7, v2_0.py_2, v2_0.pz_2));
        l0_0 = _S1189;
        l1_0 = _S1190;
        l2_0 = _S1191;
        tw0_0 = _S1181;
        tw1_0 = _S1181;
        tw2_1 = _S1181;
    }
    float3 p0_3 = node_5 + l0_0;
    float3 e1_5 = l1_0 - l0_0;
    float3 e2_5 = l2_0 - l0_0;
    float2 bb_3 = ray_bary_0(_S1166, p0_3, e1_5, e2_5);
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
    float b0_1 = 1.0f - b1_8 - b2_11;
    float3 _S1192 = float3(b0_1) ;
    float3 local_0 = l0_0 * _S1192 + l1_0 * float3(b1_8)  + l2_0 * float3(b2_11) ;
    float3 _S1193 = xform_normal_0(&_S1173, oct_decode_0(v0_0.n_26) * _S1192 + oct_decode_0(v1_0.n_26) * float3(b1_8)  + oct_decode_0(v2_0.n_26) * float3(b2_11) );
    float3 n_30 = normalize(_S1193);
    float3 v_24 = - _S1166;
    GpuMaterial_natural_0 device* _S1194 = ed_buf_13(kernelContext_87->ed_c_0->materials_0, kernelContext_87);
    thread GpuMaterial_natural_0 _S1195 = *(_S1194 + (&_S1173)->material_0);
    float4 _S1196 = float4((&_S1195)->base_3) ;
    float3 albedo_16 = _S1196.xyz;
    float4 _S1197 = float4((&_S1195)->params_1) ;
    float metal_3 = _S1197.x;
    float rough_10 = _S1197.y;
    uint4 _S1198 = uint4((&_S1195)->tex_2) ;
    uint _S1199 = _S1198.x;
    bool _S1200 = _S1199 != 4294967295U;
    if(_S1200)
    {
        textured_0 = true;
    }
    else
    {
        textured_0 = (_S1198.y) != 4294967295U;
    }
    if(textured_0)
    {
        textured_0 = true;
    }
    else
    {
        textured_0 = (_S1198.z) != 4294967295U;
    }
    bool leaf_6;
    float3 albedo_17;
    float3 n_31;
    float ao_14;
    float rough_11;
    float metal_4;
    float d_64;
    if(foliage_0)
    {
        TreeSurf_0 _S1201 = tree_surface_0(_S1161, &_S1173, &_S1195, tw0_0, tw1_0, tw2_1, b0_1, b1_8, b2_11, _S1162, p0_3, e1_5, e2_5, kernelContext_87);
        if(_S1201.shell_2)
        {
            float3 _S1202 = xform_normal_0(&_S1173, float3(0.0f, 1.0f, 0.0f));
            float3 up_14 = normalize(_S1202);
            float clump_0 = t_noise_0(local_0.xz * float2(0.89999997615814209f)  + float2((local_0.y * 0.60000002384185791f)) , 1.0f);
            float3 albedo_18 = _S1201.albedo_14 * float3((mix(0.60000002384185791f, 1.1499999761581421f, clump_0) * 0.85000002384185791f)) ;
            float _S1203 = mix(0.64999997615814209f, 1.0f, clump_0);
            n_31 = normalize(mix(n_30, up_14, float3(0.40000000596046448f) ));
            albedo_17 = albedo_18;
            ao_14 = _S1203;
            rough_11 = 1.0f;
        }
        else
        {
            n_31 = n_30;
            albedo_17 = _S1201.albedo_14;
            ao_14 = 1.0f;
            rough_11 = 0.81999999284744263f;
        }
        if((_S1180 & 131072U) != 0U)
        {
            float mot_0 = t_noise_0(local_0.xz * float2(1.7000000476837158f)  + float2((local_0.y * 3.0f)) , 1.0f);
            float3 albedo_19 = albedo_17 * float3(mix(0.81999999284744263f, 1.1200000047683716f, mot_0)) ;
            if(_S1201.leaf_5)
            {
                b1_8 = 0.75f;
            }
            else
            {
                b1_8 = 0.89999997615814209f;
            }
            albedo_17 = albedo_19;
            rough_11 = b1_8;
        }
        else
        {
            if(!_S1201.leaf_5)
            {
                textured_0 = !_S1201.shell_2;
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
                float3 pm_1 = float3(v0_0.px_7, v0_0.py_2, v0_0.pz_2) * _S1192 + float3(v1_0.px_7, v1_0.py_2, v1_0.pz_2) * float3(b1_8)  + float3(v2_0.px_7, v2_0.py_2, v2_0.pz_2) * float3(b2_11) ;
                float _S1204 = pm_1.y;
                float2 q_27 = float2(atan2(pm_1.z, pm_1.x) * max(length(pm_1.xz), 0.019999999552965164f), _S1204);
                float hb_0 = bark_height_0(q_27);
                float _S1205 = t_noise_0(q_27 * float2(1.0f, 0.5f) + float2(7.0f) , 0.44999998807907104f);
                float3 _S1206 = mix(albedo_17, float3(dot(albedo_17, float3(0.30000001192092896f, 0.5f, 0.20000000298023224f)))  * float3(1.0499999523162842f, 1.0499999523162842f, 1.0f) * float3(1.3500000238418579f) , float3((smoothstep(0.55000001192092896f, 0.85000002384185791f, _S1205) * 0.44999998807907104f)) ) * float3(mix(0.5f, 1.1200000047683716f, hb_0))  * float3(mix(0.72000002861022949f, 1.0f, smoothstep(0.0f, 1.2000000476837158f, _S1204))) ;
                float3 _S1207 = xform_normal_0(&_S1173, float3(0.0f, 1.0f, 0.0f));
                float3 tu_8 = cross(normalize(_S1207), n_31);
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
                    float hu_0 = bark_height_0(q_27 + float2(0.012000000104308128f, 0.0f));
                    float hv_0 = bark_height_0(q_27 + float2(0.0f, 0.012000000104308128f));
                    float3 tu_9 = normalize(tu_8);
                    n_31 = normalize(n_31 - float3(0.89999997615814209f)  * (float3(((hu_0 - hb_0) / 0.012000000104308128f * 0.029999999329447746f))  * tu_9 + float3(((hv_0 - hb_0) / 0.012000000104308128f * 0.029999999329447746f))  * normalize(cross(n_31, tu_9))));
                }
                float ao_15 = ao_14 * mix(0.64999997615814209f, 1.0f, hb_0);
                albedo_17 = _S1206;
                ao_14 = ao_15;
            }
        }
        if(_S1201.leaf_5)
        {
            float3 ngc_0 = cross(e1_5, e2_5);
            if((dot(ngc_0, v_24) * dot(ngc_0, n_31)) < 0.0f)
            {
                n_31 = - n_31;
            }
        }
        leaf_6 = _S1201.leaf_5;
        metal_4 = 0.0f;
    }
    else
    {
        if(textured_0)
        {
            packed_float2 device* _S1208 = ed_buf_14(kernelContext_87->ed_c_0->uvs_0, kernelContext_87);
            float2 _S1209 = float2(*(_S1208 + i0_0)) ;
            float2 d1_2 = float2(*(_S1208 + i1_0))  - _S1209;
            float2 d2_2 = float2(*(_S1208 + i2_0))  - _S1209;
            float2 _S1210 = uv_xform_0(&_S1195, _S1209 + d1_2 * float2(b1_8)  + d2_2 * float2(b2_11) );
            float3 _S1211 = view_dir_0(_S1161, _S1162 + float2(1.0f, 0.0f));
            float2 bx_3 = ray_bary_0(_S1211, p0_3, e1_5, e2_5);
            float3 _S1212 = view_dir_0(_S1161, _S1162 + float2(0.0f, 1.0f));
            float2 by_3 = ray_bary_0(_S1212, p0_3, e1_5, e2_5);
            float2 _S1213 = uv_xform_0(&_S1195, _S1209 + d1_2 * float2(bx_3.x)  + d2_2 * float2(bx_3.y) );
            float2 ux_1 = _S1213 - _S1210;
            float2 _S1214 = uv_xform_0(&_S1195, _S1209 + d1_2 * float2(by_3.x)  + d2_2 * float2(by_3.y) );
            float2 uy_1 = _S1214 - _S1210;
            sampler _S1215 = ((reinterpret_cast<device sampler*>((kernelContext_87->ed_samplers_0))[(kernelContext_87->ed_c_0->smp_0)]));
            if(_S1200)
            {
                texture2d<float, access::sample> _S1216 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_87->ed_textures_0))[(_S1199)]));
                albedo_17 = albedo_16 * ((_S1216).sample((_S1215), (_S1210), gradient2d((ux_1), (uy_1)))).xyz;
            }
            else
            {
                albedo_17 = albedo_16;
            }
            uint _S1217 = _S1198.z;
            if(_S1217 != 4294967295U)
            {
                texture2d<float, access::sample> _S1218 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_87->ed_textures_0))[(_S1217)]));
                float3 orm_0 = ((_S1218).sample((_S1215), (_S1210), gradient2d((ux_1), (uy_1)))).xyz;
                float rough_12 = rough_10 * orm_0.y;
                float metal_5 = metal_3 * orm_0.z;
                ao_14 = orm_0.x;
                rough_11 = rough_12;
                metal_4 = metal_5;
            }
            else
            {
                ao_14 = 1.0f;
                rough_11 = rough_10;
                metal_4 = metal_3;
            }
            uint _S1219 = _S1198.y;
            if(_S1219 != 4294967295U)
            {
                float _S1220 = d1_2.x;
                float _S1221 = d2_2.y;
                float _S1222 = d1_2.y;
                float _S1223 = d2_2.x;
                float det_2 = _S1220 * _S1221 - _S1222 * _S1223;
                if((abs(det_2)) > 9.999999960041972e-13f)
                {
                    float3 _S1224 = float3((1.0f / det_2)) ;
                    float3 tg_0 = (e1_5 * float3(_S1221)  - e2_5 * float3(_S1222) ) * _S1224;
                    float3 bt_0 = (e2_5 * float3(_S1220)  - e1_5 * float3(_S1223) ) * _S1224;
                    float3 tg_1 = tg_0 - n_30 * float3(dot(n_30, tg_0)) ;
                    if((length_squared_0(tg_1)) > 9.99999968265522539e-21f)
                    {
                        float3 tg_2 = normalize(tg_1);
                        float3 _S1225 = cross(n_30, tg_2);
                        if((dot(_S1225, bt_0)) < 0.0f)
                        {
                            d_64 = -1.0f;
                        }
                        else
                        {
                            d_64 = 1.0f;
                        }
                        float3 bn_0 = _S1225 * float3(- d_64) ;
                        texture2d<float, access::sample> _S1226 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_87->ed_textures_0))[(_S1219)]));
                        float2 xy_0 = ((_S1226).sample((_S1215), (_S1210), gradient2d((ux_1), (uy_1)))).xy * float2(2.0f)  - float2(1.0f) ;
                        n_31 = normalize(tg_2 * float3(xy_0.x)  + bn_0 * float3(xy_0.y)  + n_30 * float3(sqrt(saturate(1.0f - dot(xy_0, xy_0)))) );
                    }
                    else
                    {
                        n_31 = n_30;
                    }
                }
                else
                {
                    n_31 = n_30;
                }
            }
            else
            {
                n_31 = n_30;
            }
        }
        else
        {
            n_31 = n_30;
            albedo_17 = albedo_16;
            ao_14 = 1.0f;
            rough_11 = rough_10;
            metal_4 = metal_3;
        }
        leaf_6 = false;
    }
    float _S1227 = dot(n_31, v_24);
    if(_S1227 < 0.0f)
    {
        n_31 = normalize(n_31 - float3((1.0099999904632568f * _S1227))  * v_24);
    }
    if((_S1197.w) > 0.5f)
    {
        float2 c_35 = floor(local_0.xz * float2(0.5f) );
        albedo_17 = albedo_17 * float3(mix(0.80000001192092896f, 1.1499999761581421f, fmod(abs(c_35.x + c_35.y), 2.0f))) ;
    }
    float3 p_27 = node_5 + local_0;
    float zlin_5 = - (((float4(p_27, 1.0f)) * (matrix<float,int(4),int(4)> (_S1161->view_0.data_0[int(0)][int(0)], _S1161->view_0.data_0[int(1)][int(0)], _S1161->view_0.data_0[int(2)][int(0)], _S1161->view_0.data_0[int(3)][int(0)], _S1161->view_0.data_0[int(0)][int(1)], _S1161->view_0.data_0[int(1)][int(1)], _S1161->view_0.data_0[int(2)][int(1)], _S1161->view_0.data_0[int(3)][int(1)], _S1161->view_0.data_0[int(0)][int(2)], _S1161->view_0.data_0[int(1)][int(2)], _S1161->view_0.data_0[int(2)][int(2)], _S1161->view_0.data_0[int(3)][int(2)], _S1161->view_0.data_0[int(0)][int(3)], _S1161->view_0.data_0[int(1)][int(3)], _S1161->view_0.data_0[int(2)][int(3)], _S1161->view_0.data_0[int(3)][int(3)])))).z;
    float3 ng_9 = cross(e1_5, e2_5);
    float3 ng_10;
    if((length_squared_0(ng_9)) > 9.99999968265522539e-21f)
    {
        ng_10 = normalize(ng_9);
    }
    else
    {
        ng_10 = n_31;
    }
    if((dot(ng_10, v_24)) < 0.0f)
    {
        ng_10 = - ng_10;
    }
    float _S1228 = sun_shadow_0(_S1161, kernelContext_87->ed_c_0->shadow_0, p_27, ng_10, zlin_5, _S1162, kernelContext_87);
    float2 _S1229 = ao_upsample_0(_S1161, kernelContext_87->ed_c_0->ao_0, _S1162, zlin_5, kernelContext_87);
    float ao_16 = ao_14 * _S1229.x;
    float sv_11 = _S1229.y;
    float3 _S1230 = sun_light_at_0(_S1161, p_27, kernelContext_87);
    float3 col_10;
    if(foliage_0)
    {
        float3 _S1231 = shade_foliage_0(_S1161, _S1230, albedo_17, rough_11, ao_16, sv_11, _S1228, n_31, ng_10, v_24, leaf_6, kernelContext_87);
        col_10 = _S1231;
    }
    else
    {
        if((_S1196.w) > 0.5f)
        {
            float3 _S1232 = shade_toon_0(_S1161, _S1230, &_S1195, albedo_17, n_31, v_24, ao_16, sv_11, _S1228, kernelContext_87);
            l0_0 = _S1232;
        }
        else
        {
            float3 _S1233 = shade_pbr_0(_S1161, _S1230, albedo_17, metal_4, rough_11, ao_16, sv_11, _S1228, n_31, v_24, kernelContext_87);
            l0_0 = _S1233;
        }
        col_10 = l0_0;
    }
    float3 _S1234 = apply_aerial_0(_S1161, col_10, p_27, _S1162, kernelContext_87);
    if(((uint4((&_S1161->light_0)->flags_0) ).w) == 1U)
    {
        uint _S1235 = shadow_cascade_0(_S1161, p_27, zlin_5);
        col_10 = TINT_2[_S1235] * float3((0.039999999105930328f + 0.5f * _S1228)) ;
    }
    else
    {
        if(((uint4((&_S1161->light_0)->flags_0) ).w) == 3U)
        {
            uint _S1236 = shadow_cascade_0(_S1161, p_27, zlin_5);
            if(_S1236 < 4U)
            {
                float _S1237 = cascade_depth_delta_0(_S1161, kernelContext_87->ed_c_0->shadow_0, _S1236, p_27, kernelContext_87);
                d_64 = _S1237;
            }
            else
            {
                d_64 = 0.0f;
            }
            col_10 = float3(max(d_64, 0.0f), max(- d_64, 0.0f), 0.0f) * float3(0.10000000149011612f) ;
        }
        else
        {
            if(((uint4((&_S1161->light_0)->flags_0) ).w) == 2U)
            {
                col_10 = float3((0.5f * ao_16)) ;
            }
            else
            {
                if(((uint4((&_S1161->light_0)->flags_0) ).w) == 4U)
                {
                    col_10 = float3((0.5f * sv_11)) ;
                }
                else
                {
                    col_10 = _S1234;
                }
            }
        }
    }
    return float4(col_10, 1.0f);
}

constant array<float3, int(5)> TINT_3 = { { float3(1.0f, 0.25f, 0.25f), float3(0.25f, 1.0f, 0.25f), float3(0.30000001192092896f, 0.44999998807907104f, 1.0f), float3(1.0f, 1.0f, 0.25f), float3(0.60000002384185791f, 0.60000002384185791f, 0.60000002384185791f) } };
float4 resolve_px_1(const FsOut_0 thread* i_44, bool terrain_1, KernelContext_0 thread* kernelContext_88)
{
    FrameData_natural_0 device* _S1238 = ed_buf_1(kernelContext_88->ed_c_0->frame_0, kernelContext_88);
    float2 _S1239 = i_44->pos_0.xy;
    uint2 _S1240 = uint2(_S1239);
    texture2d<uint, access::sample> _S1241 = ((reinterpret_cast<device texture2d<uint, access::sample>*>((kernelContext_88->ed_textures_0))[(kernelContext_88->ed_c_0->vis_0)]));
    int3 _S1242 = int3(int2(_S1240), int(0));
    uint2 vis_6 = ((_S1241).read(vec<uint,2>(((_S1242)).xy), uint(((_S1242)).z))).xy;
    float3 _S1243 = view_dir_0(_S1238, _S1239);
    uint _S1244 = vis_6.x;
    if(_S1244 == 0U)
    {
        float3 _S1245 = sky_pixel_0(_S1238, _S1243, kernelContext_88);
        return float4(_S1245, 1.0f);
    }
    bool textured_1;
    if(terrain_1)
    {
        textured_1 = (_S1244 & 2147483648U) != 0U;
    }
    else
    {
        textured_1 = false;
    }
    if(textured_1)
    {
        float4 _S1246 = terrain_pixel_1(_S1238, i_44, vis_6, _S1243, kernelContext_88);
        return _S1246;
    }
    if((_S1244 & 3758096384U) == 536870912U)
    {
        float4 _S1247 = grass_pixel_0(_S1238, i_44, vis_6, _S1243, kernelContext_88);
        return _S1247;
    }
    if((_S1244 & 3221225472U) == 1073741824U)
    {
        float4 _S1248 = impostor_pixel_0(_S1238, i_44, _S1244 & 3221225471U, _S1243, kernelContext_88);
        return _S1248;
    }
    GpuInstance_natural_0 device* _S1249 = ed_buf_9(kernelContext_88->ed_c_0->instances_0, kernelContext_88);
    thread GpuInstance_natural_0 _S1250 = *(_S1249 + (_S1244 - 1U));
    GpuMesh_natural_0 device* _S1251 = ed_buf_10(kernelContext_88->ed_c_0->meshes_0, kernelContext_88);
    GpuMesh_natural_0 device* _S1252 = _S1251 + (&_S1250)->mesh_0;
    uint _S1253 = vis_6.y;
    uint lod_4 = _S1253 >> 28U;
    uint prim_2 = (_S1253 & 268435455U) % max(_S1252->index_count_0[lod_4] / 3U, 1U);
    uint device* _S1254 = ed_buf_0(kernelContext_88->ed_c_0->indices_0, kernelContext_88);
    uint device* idx_2 = _S1254 + _S1252->first_index_0[lod_4] + prim_2 * 3U;
    Vtx_0 device* _S1255 = ed_buf_11(kernelContext_88->ed_c_0->verts_0, kernelContext_88);
    int i0_1 = int(*(idx_2 + int(0))) + _S1252->base_vertex_0[lod_4];
    int i1_1 = int(*(idx_2 + int(1))) + _S1252->base_vertex_0[lod_4];
    int i2_1 = int(*(idx_2 + int(2))) + _S1252->base_vertex_0[lod_4];
    Vtx_0 v0_1 = *(_S1255 + i0_1);
    Vtx_0 v1_1 = *(_S1255 + i1_1);
    Vtx_0 v2_1 = *(_S1255 + i2_1);
    packed_float4 device* _S1256 = ed_buf_4(kernelContext_88->ed_c_0->nodes_0, kernelContext_88);
    uint _S1257 = (&_S1250)->node_flags_0;
    float3 node_6 = (float4(*(_S1256 + (((&_S1250)->node_flags_0) & 65535U))) ).xyz;
    bool foliage_1 = ((_S1252->flags_1) & 1U) != 0U;
    uint2 _S1258 = uint2(0U, 0U);
    float3 l0_1;
    float3 l1_1;
    float3 l2_1;
    uint2 tw0_1;
    uint2 tw1_1;
    uint2 tw2_2;
    if(foliage_1)
    {
        packed_uint2 device* _S1259 = ed_buf_12(kernelContext_88->ed_c_0->uvs_0, kernelContext_88);
        uint2 _S1260 = uint2(*(_S1259 + i0_1)) ;
        uint2 _S1261 = uint2(*(_S1259 + i1_1)) ;
        uint2 _S1262 = uint2(*(_S1259 + i2_1)) ;
        float3 _S1263 = tree_vertex_0(_S1238, &_S1250, node_6, float3(v0_1.px_7, v0_1.py_2, v0_1.pz_2), v0_1.n_26, _S1260.y);
        float3 _S1264 = tree_vertex_0(_S1238, &_S1250, node_6, float3(v1_1.px_7, v1_1.py_2, v1_1.pz_2), v1_1.n_26, _S1261.y);
        float3 _S1265 = tree_vertex_0(_S1238, &_S1250, node_6, float3(v2_1.px_7, v2_1.py_2, v2_1.pz_2), v2_1.n_26, _S1262.y);
        l0_1 = _S1263;
        l1_1 = _S1264;
        l2_1 = _S1265;
        tw0_1 = _S1260;
        tw1_1 = _S1261;
        tw2_2 = _S1262;
    }
    else
    {
        float3 _S1266 = xform_point_0(&_S1250, float3(v0_1.px_7, v0_1.py_2, v0_1.pz_2));
        float3 _S1267 = xform_point_0(&_S1250, float3(v1_1.px_7, v1_1.py_2, v1_1.pz_2));
        float3 _S1268 = xform_point_0(&_S1250, float3(v2_1.px_7, v2_1.py_2, v2_1.pz_2));
        l0_1 = _S1266;
        l1_1 = _S1267;
        l2_1 = _S1268;
        tw0_1 = _S1258;
        tw1_1 = _S1258;
        tw2_2 = _S1258;
    }
    float3 p0_4 = node_6 + l0_1;
    float3 e1_6 = l1_1 - l0_1;
    float3 e2_6 = l2_1 - l0_1;
    float2 bb_4 = ray_bary_0(_S1243, p0_4, e1_6, e2_6);
    float b1_9 = saturate(bb_4.x);
    float b2_13 = saturate(bb_4.y);
    float bs_4 = b1_9 + b2_13;
    float b1_10;
    float b2_14;
    if(bs_4 > 1.0f)
    {
        float b2_15 = b2_13 / bs_4;
        b1_10 = b1_9 / bs_4;
        b2_14 = b2_15;
    }
    else
    {
        b1_10 = b1_9;
        b2_14 = b2_13;
    }
    float b0_2 = 1.0f - b1_10 - b2_14;
    float3 _S1269 = float3(b0_2) ;
    float3 local_1 = l0_1 * _S1269 + l1_1 * float3(b1_10)  + l2_1 * float3(b2_14) ;
    float3 _S1270 = xform_normal_0(&_S1250, oct_decode_0(v0_1.n_26) * _S1269 + oct_decode_0(v1_1.n_26) * float3(b1_10)  + oct_decode_0(v2_1.n_26) * float3(b2_14) );
    float3 n_32 = normalize(_S1270);
    float3 v_25 = - _S1243;
    GpuMaterial_natural_0 device* _S1271 = ed_buf_13(kernelContext_88->ed_c_0->materials_0, kernelContext_88);
    thread GpuMaterial_natural_0 _S1272 = *(_S1271 + (&_S1250)->material_0);
    float4 _S1273 = float4((&_S1272)->base_3) ;
    float3 albedo_20 = _S1273.xyz;
    float4 _S1274 = float4((&_S1272)->params_1) ;
    float metal_6 = _S1274.x;
    float rough_13 = _S1274.y;
    uint4 _S1275 = uint4((&_S1272)->tex_2) ;
    uint _S1276 = _S1275.x;
    bool _S1277 = _S1276 != 4294967295U;
    if(_S1277)
    {
        textured_1 = true;
    }
    else
    {
        textured_1 = (_S1275.y) != 4294967295U;
    }
    if(textured_1)
    {
        textured_1 = true;
    }
    else
    {
        textured_1 = (_S1275.z) != 4294967295U;
    }
    bool leaf_7;
    float3 albedo_21;
    float3 n_33;
    float ao_17;
    float rough_14;
    float metal_7;
    float d_65;
    if(foliage_1)
    {
        TreeSurf_0 _S1278 = tree_surface_0(_S1238, &_S1250, &_S1272, tw0_1, tw1_1, tw2_2, b0_2, b1_10, b2_14, _S1239, p0_4, e1_6, e2_6, kernelContext_88);
        if(_S1278.shell_2)
        {
            float3 _S1279 = xform_normal_0(&_S1250, float3(0.0f, 1.0f, 0.0f));
            float3 up_15 = normalize(_S1279);
            float clump_1 = t_noise_0(local_1.xz * float2(0.89999997615814209f)  + float2((local_1.y * 0.60000002384185791f)) , 1.0f);
            float3 albedo_22 = _S1278.albedo_14 * float3((mix(0.60000002384185791f, 1.1499999761581421f, clump_1) * 0.85000002384185791f)) ;
            float _S1280 = mix(0.64999997615814209f, 1.0f, clump_1);
            n_33 = normalize(mix(n_32, up_15, float3(0.40000000596046448f) ));
            albedo_21 = albedo_22;
            ao_17 = _S1280;
            rough_14 = 1.0f;
        }
        else
        {
            n_33 = n_32;
            albedo_21 = _S1278.albedo_14;
            ao_17 = 1.0f;
            rough_14 = 0.81999999284744263f;
        }
        if((_S1257 & 131072U) != 0U)
        {
            float mot_1 = t_noise_0(local_1.xz * float2(1.7000000476837158f)  + float2((local_1.y * 3.0f)) , 1.0f);
            float3 albedo_23 = albedo_21 * float3(mix(0.81999999284744263f, 1.1200000047683716f, mot_1)) ;
            if(_S1278.leaf_5)
            {
                b1_10 = 0.75f;
            }
            else
            {
                b1_10 = 0.89999997615814209f;
            }
            albedo_21 = albedo_23;
            rough_14 = b1_10;
        }
        else
        {
            if(!_S1278.leaf_5)
            {
                textured_1 = !_S1278.shell_2;
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
                float3 pm_2 = float3(v0_1.px_7, v0_1.py_2, v0_1.pz_2) * _S1269 + float3(v1_1.px_7, v1_1.py_2, v1_1.pz_2) * float3(b1_10)  + float3(v2_1.px_7, v2_1.py_2, v2_1.pz_2) * float3(b2_14) ;
                float _S1281 = pm_2.y;
                float2 q_28 = float2(atan2(pm_2.z, pm_2.x) * max(length(pm_2.xz), 0.019999999552965164f), _S1281);
                float hb_1 = bark_height_0(q_28);
                float _S1282 = t_noise_0(q_28 * float2(1.0f, 0.5f) + float2(7.0f) , 0.44999998807907104f);
                float3 _S1283 = mix(albedo_21, float3(dot(albedo_21, float3(0.30000001192092896f, 0.5f, 0.20000000298023224f)))  * float3(1.0499999523162842f, 1.0499999523162842f, 1.0f) * float3(1.3500000238418579f) , float3((smoothstep(0.55000001192092896f, 0.85000002384185791f, _S1282) * 0.44999998807907104f)) ) * float3(mix(0.5f, 1.1200000047683716f, hb_1))  * float3(mix(0.72000002861022949f, 1.0f, smoothstep(0.0f, 1.2000000476837158f, _S1281))) ;
                float3 _S1284 = xform_normal_0(&_S1250, float3(0.0f, 1.0f, 0.0f));
                float3 tu_10 = cross(normalize(_S1284), n_33);
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
                    float hu_1 = bark_height_0(q_28 + float2(0.012000000104308128f, 0.0f));
                    float hv_1 = bark_height_0(q_28 + float2(0.0f, 0.012000000104308128f));
                    float3 tu_11 = normalize(tu_10);
                    n_33 = normalize(n_33 - float3(0.89999997615814209f)  * (float3(((hu_1 - hb_1) / 0.012000000104308128f * 0.029999999329447746f))  * tu_11 + float3(((hv_1 - hb_1) / 0.012000000104308128f * 0.029999999329447746f))  * normalize(cross(n_33, tu_11))));
                }
                float ao_18 = ao_17 * mix(0.64999997615814209f, 1.0f, hb_1);
                albedo_21 = _S1283;
                ao_17 = ao_18;
            }
        }
        if(_S1278.leaf_5)
        {
            float3 ngc_1 = cross(e1_6, e2_6);
            if((dot(ngc_1, v_25) * dot(ngc_1, n_33)) < 0.0f)
            {
                n_33 = - n_33;
            }
        }
        leaf_7 = _S1278.leaf_5;
        metal_7 = 0.0f;
    }
    else
    {
        if(textured_1)
        {
            packed_float2 device* _S1285 = ed_buf_14(kernelContext_88->ed_c_0->uvs_0, kernelContext_88);
            float2 _S1286 = float2(*(_S1285 + i0_1)) ;
            float2 d1_3 = float2(*(_S1285 + i1_1))  - _S1286;
            float2 d2_3 = float2(*(_S1285 + i2_1))  - _S1286;
            float2 _S1287 = uv_xform_0(&_S1272, _S1286 + d1_3 * float2(b1_10)  + d2_3 * float2(b2_14) );
            float3 _S1288 = view_dir_0(_S1238, _S1239 + float2(1.0f, 0.0f));
            float2 bx_4 = ray_bary_0(_S1288, p0_4, e1_6, e2_6);
            float3 _S1289 = view_dir_0(_S1238, _S1239 + float2(0.0f, 1.0f));
            float2 by_4 = ray_bary_0(_S1289, p0_4, e1_6, e2_6);
            float2 _S1290 = uv_xform_0(&_S1272, _S1286 + d1_3 * float2(bx_4.x)  + d2_3 * float2(bx_4.y) );
            float2 ux_2 = _S1290 - _S1287;
            float2 _S1291 = uv_xform_0(&_S1272, _S1286 + d1_3 * float2(by_4.x)  + d2_3 * float2(by_4.y) );
            float2 uy_2 = _S1291 - _S1287;
            sampler _S1292 = ((reinterpret_cast<device sampler*>((kernelContext_88->ed_samplers_0))[(kernelContext_88->ed_c_0->smp_0)]));
            if(_S1277)
            {
                texture2d<float, access::sample> _S1293 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_88->ed_textures_0))[(_S1276)]));
                albedo_21 = albedo_20 * ((_S1293).sample((_S1292), (_S1287), gradient2d((ux_2), (uy_2)))).xyz;
            }
            else
            {
                albedo_21 = albedo_20;
            }
            uint _S1294 = _S1275.z;
            if(_S1294 != 4294967295U)
            {
                texture2d<float, access::sample> _S1295 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_88->ed_textures_0))[(_S1294)]));
                float3 orm_1 = ((_S1295).sample((_S1292), (_S1287), gradient2d((ux_2), (uy_2)))).xyz;
                float rough_15 = rough_13 * orm_1.y;
                float metal_8 = metal_6 * orm_1.z;
                ao_17 = orm_1.x;
                rough_14 = rough_15;
                metal_7 = metal_8;
            }
            else
            {
                ao_17 = 1.0f;
                rough_14 = rough_13;
                metal_7 = metal_6;
            }
            uint _S1296 = _S1275.y;
            if(_S1296 != 4294967295U)
            {
                float _S1297 = d1_3.x;
                float _S1298 = d2_3.y;
                float _S1299 = d1_3.y;
                float _S1300 = d2_3.x;
                float det_3 = _S1297 * _S1298 - _S1299 * _S1300;
                if((abs(det_3)) > 9.999999960041972e-13f)
                {
                    float3 _S1301 = float3((1.0f / det_3)) ;
                    float3 tg_3 = (e1_6 * float3(_S1298)  - e2_6 * float3(_S1299) ) * _S1301;
                    float3 bt_1 = (e2_6 * float3(_S1297)  - e1_6 * float3(_S1300) ) * _S1301;
                    float3 tg_4 = tg_3 - n_32 * float3(dot(n_32, tg_3)) ;
                    if((length_squared_0(tg_4)) > 9.99999968265522539e-21f)
                    {
                        float3 tg_5 = normalize(tg_4);
                        float3 _S1302 = cross(n_32, tg_5);
                        if((dot(_S1302, bt_1)) < 0.0f)
                        {
                            d_65 = -1.0f;
                        }
                        else
                        {
                            d_65 = 1.0f;
                        }
                        float3 bn_1 = _S1302 * float3(- d_65) ;
                        texture2d<float, access::sample> _S1303 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_88->ed_textures_0))[(_S1296)]));
                        float2 xy_1 = ((_S1303).sample((_S1292), (_S1287), gradient2d((ux_2), (uy_2)))).xy * float2(2.0f)  - float2(1.0f) ;
                        n_33 = normalize(tg_5 * float3(xy_1.x)  + bn_1 * float3(xy_1.y)  + n_32 * float3(sqrt(saturate(1.0f - dot(xy_1, xy_1)))) );
                    }
                    else
                    {
                        n_33 = n_32;
                    }
                }
                else
                {
                    n_33 = n_32;
                }
            }
            else
            {
                n_33 = n_32;
            }
        }
        else
        {
            n_33 = n_32;
            albedo_21 = albedo_20;
            ao_17 = 1.0f;
            rough_14 = rough_13;
            metal_7 = metal_6;
        }
        leaf_7 = false;
    }
    float _S1304 = dot(n_33, v_25);
    if(_S1304 < 0.0f)
    {
        n_33 = normalize(n_33 - float3((1.0099999904632568f * _S1304))  * v_25);
    }
    if((_S1274.w) > 0.5f)
    {
        float2 c_36 = floor(local_1.xz * float2(0.5f) );
        albedo_21 = albedo_21 * float3(mix(0.80000001192092896f, 1.1499999761581421f, fmod(abs(c_36.x + c_36.y), 2.0f))) ;
    }
    float3 p_28 = node_6 + local_1;
    float zlin_6 = - (((float4(p_28, 1.0f)) * (matrix<float,int(4),int(4)> (_S1238->view_0.data_0[int(0)][int(0)], _S1238->view_0.data_0[int(1)][int(0)], _S1238->view_0.data_0[int(2)][int(0)], _S1238->view_0.data_0[int(3)][int(0)], _S1238->view_0.data_0[int(0)][int(1)], _S1238->view_0.data_0[int(1)][int(1)], _S1238->view_0.data_0[int(2)][int(1)], _S1238->view_0.data_0[int(3)][int(1)], _S1238->view_0.data_0[int(0)][int(2)], _S1238->view_0.data_0[int(1)][int(2)], _S1238->view_0.data_0[int(2)][int(2)], _S1238->view_0.data_0[int(3)][int(2)], _S1238->view_0.data_0[int(0)][int(3)], _S1238->view_0.data_0[int(1)][int(3)], _S1238->view_0.data_0[int(2)][int(3)], _S1238->view_0.data_0[int(3)][int(3)])))).z;
    float3 ng_11 = cross(e1_6, e2_6);
    float3 ng_12;
    if((length_squared_0(ng_11)) > 9.99999968265522539e-21f)
    {
        ng_12 = normalize(ng_11);
    }
    else
    {
        ng_12 = n_33;
    }
    if((dot(ng_12, v_25)) < 0.0f)
    {
        ng_12 = - ng_12;
    }
    float _S1305 = sun_shadow_0(_S1238, kernelContext_88->ed_c_0->shadow_0, p_28, ng_12, zlin_6, _S1239, kernelContext_88);
    float2 _S1306 = ao_upsample_0(_S1238, kernelContext_88->ed_c_0->ao_0, _S1239, zlin_6, kernelContext_88);
    float ao_19 = ao_17 * _S1306.x;
    float sv_12 = _S1306.y;
    float3 _S1307 = sun_light_at_0(_S1238, p_28, kernelContext_88);
    float3 col_11;
    if(foliage_1)
    {
        float3 _S1308 = shade_foliage_0(_S1238, _S1307, albedo_21, rough_14, ao_19, sv_12, _S1305, n_33, ng_12, v_25, leaf_7, kernelContext_88);
        col_11 = _S1308;
    }
    else
    {
        if((_S1273.w) > 0.5f)
        {
            float3 _S1309 = shade_toon_0(_S1238, _S1307, &_S1272, albedo_21, n_33, v_25, ao_19, sv_12, _S1305, kernelContext_88);
            l0_1 = _S1309;
        }
        else
        {
            float3 _S1310 = shade_pbr_0(_S1238, _S1307, albedo_21, metal_7, rough_14, ao_19, sv_12, _S1305, n_33, v_25, kernelContext_88);
            l0_1 = _S1310;
        }
        col_11 = l0_1;
    }
    float3 _S1311 = apply_aerial_0(_S1238, col_11, p_28, _S1239, kernelContext_88);
    if(((uint4((&_S1238->light_0)->flags_0) ).w) == 1U)
    {
        uint _S1312 = shadow_cascade_0(_S1238, p_28, zlin_6);
        col_11 = TINT_3[_S1312] * float3((0.039999999105930328f + 0.5f * _S1305)) ;
    }
    else
    {
        if(((uint4((&_S1238->light_0)->flags_0) ).w) == 3U)
        {
            uint _S1313 = shadow_cascade_0(_S1238, p_28, zlin_6);
            if(_S1313 < 4U)
            {
                float _S1314 = cascade_depth_delta_0(_S1238, kernelContext_88->ed_c_0->shadow_0, _S1313, p_28, kernelContext_88);
                d_65 = _S1314;
            }
            else
            {
                d_65 = 0.0f;
            }
            col_11 = float3(max(d_65, 0.0f), max(- d_65, 0.0f), 0.0f) * float3(0.10000000149011612f) ;
        }
        else
        {
            if(((uint4((&_S1238->light_0)->flags_0) ).w) == 2U)
            {
                col_11 = float3((0.5f * ao_19)) ;
            }
            else
            {
                if(((uint4((&_S1238->light_0)->flags_0) ).w) == 4U)
                {
                    col_11 = float3((0.5f * sv_12)) ;
                }
                else
                {
                    col_11 = _S1311;
                }
            }
        }
    }
    return float4(col_11, 1.0f);
}

FsOut_0 fullscreen_vertex_0(uint vid_0)
{
    thread FsOut_0 o_8;
    (&o_8)->pos_0 = float4(float2(float((vid_0 << 1U) & 2U), float(vid_0 & 2U)) * float2(2.0f)  - float2(1.0f) , 0.0f, 1.0f);
    return o_8;
}

struct pixelOutput_0
{
    float4 output_0 [[color(0)]];
};

[[fragment]] pixelOutput_0 fs_resolve(float4 pos_1 [[position]], ulong device* ed_buffers_1 [[buffer(2)]], ResolveC_0 constant* ed_c_1 [[buffer(0)]], ulong device* ed_textures_1 [[buffer(1)]], ulong device* ed_samplers_1 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_89;
    (&kernelContext_89)->ed_buffers_0 = ed_buffers_1;
    (&kernelContext_89)->ed_c_0 = ed_c_1;
    (&kernelContext_89)->ed_textures_0 = ed_textures_1;
    (&kernelContext_89)->ed_samplers_0 = ed_samplers_1;
    thread FsOut_0 _S1315;
    (&_S1315)->pos_0 = pos_1;
    float4 _S1316 = resolve_px_1(&_S1315, false, &kernelContext_89);
    pixelOutput_0 _S1317 = { _S1316 };
    return _S1317;
}

struct pixelOutput_1
{
    float4 output_1 [[color(0)]];
};

[[fragment]] pixelOutput_1 fs_resolve_terrain(float4 pos_2 [[position]], ulong device* ed_buffers_2 [[buffer(2)]], ResolveC_0 constant* ed_c_2 [[buffer(0)]], ulong device* ed_textures_2 [[buffer(1)]], ulong device* ed_samplers_2 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_90;
    (&kernelContext_90)->ed_buffers_0 = ed_buffers_2;
    (&kernelContext_90)->ed_c_0 = ed_c_2;
    (&kernelContext_90)->ed_textures_0 = ed_textures_2;
    (&kernelContext_90)->ed_samplers_0 = ed_samplers_2;
    thread FsOut_0 _S1318;
    (&_S1318)->pos_0 = pos_2;
    float4 _S1319 = resolve_px_1(&_S1318, true, &kernelContext_90);
    pixelOutput_1 _S1320 = { _S1319 };
    return _S1320;
}

struct pixelOutput_2
{
    float4 output_2 [[color(0)]];
};

[[fragment]] pixelOutput_2 fs_resolve_trees(float4 pos_3 [[position]], ulong device* ed_buffers_3 [[buffer(2)]], ResolveC_0 constant* ed_c_3 [[buffer(0)]], ulong device* ed_textures_3 [[buffer(1)]], ulong device* ed_samplers_3 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_91;
    (&kernelContext_91)->ed_buffers_0 = ed_buffers_3;
    (&kernelContext_91)->ed_c_0 = ed_c_3;
    (&kernelContext_91)->ed_textures_0 = ed_textures_3;
    (&kernelContext_91)->ed_samplers_0 = ed_samplers_3;
    thread FsOut_0 _S1321;
    (&_S1321)->pos_0 = pos_3;
    float4 _S1322 = resolve_px_0(&_S1321, true, &kernelContext_91);
    pixelOutput_2 _S1323 = { _S1322 };
    return _S1323;
}

struct vs_resolve_Result_0
{
    float4 pos_4 [[position]];
};

[[vertex]] vs_resolve_Result_0 vs_resolve(uint vid_1 [[vertex_id]], ulong device* ed_buffers_4 [[buffer(2)]], ResolveC_0 constant* ed_c_4 [[buffer(0)]], ulong device* ed_textures_4 [[buffer(1)]], ulong device* ed_samplers_4 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_92;
    (&kernelContext_92)->ed_buffers_0 = ed_buffers_4;
    (&kernelContext_92)->ed_c_0 = ed_c_4;
    (&kernelContext_92)->ed_textures_0 = ed_textures_4;
    (&kernelContext_92)->ed_samplers_0 = ed_samplers_4;
    thread vs_resolve_Result_0 _S1324;
    (&_S1324)->pos_4 = fullscreen_vertex_0(vid_1).pos_0;
    return _S1324;
}

