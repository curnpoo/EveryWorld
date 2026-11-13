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
    uint lod_0 = (prim_0 >> 24U) & 3U;
    uint _S74 = prim_0 & 16777215U;
    uint st_0 = 1U << lod_0;
    uint c_1 = 64U >> lod_0;
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
    array<packed_float4, int(6)> data_5;
};

struct _Array_natural_vectorx3Cfloatx2C4x3E2_0
{
    array<packed_float4, int(2)> data_6;
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
    float grass_0 = (1.0f - forest_1) * smoothstep(3000.0f, 2600.0f, h_9 + _S202 * 200.0f);
    float soil_0 = saturate(hollow_f_0 * 0.80000001192092896f + (n1_0 - 0.64999997615814209f) * 1.5f) * (1.0f - alpine_0 * 0.5f);
    float snow_0 = smoothstep(snowline_0 - 120.0f, snowline_0 + 120.0f, h_9 + hollow_0 * 150.0f + _S202 * 120.0f) * smoothstep(0.62000000476837158f, 0.77999997138977051f, cf_0 + _S202 * 0.10000000149011612f);
    thread array<float, int(6)> wt_0;
    wt_0[0U] = rock_0;
    wt_0[1U] = scree_0 * (1.0f - rock_0 * 0.69999998807907104f);
    float _S205 = 1.0f - rock_0;
    float _S206 = 1.0f - scree_0 * 0.80000001192092896f;
    wt_0[2U] = grass_0 * _S205 * _S206;
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
                (&o_0)->albedo_1 = (&o_0)->albedo_1 + _S244 * ah_0[0U].xyz * (float4((&m_4->gain_0)->data_5[0U]) ).xyz;
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
                (&o_0)->albedo_1 = (&o_0)->albedo_1 + _S253 * ah_0[1U].xyz * (float4((&m_4->gain_0)->data_5[1U]) ).xyz;
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
                (&o_0)->albedo_1 = (&o_0)->albedo_1 + _S257 * ah_0[2U].xyz * (float4((&m_4->gain_0)->data_5[2U]) ).xyz;
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
                (&o_0)->albedo_1 = (&o_0)->albedo_1 + _S261 * ah_0[3U].xyz * (float4((&m_4->gain_0)->data_5[3U]) ).xyz;
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
                (&o_0)->albedo_1 = (&o_0)->albedo_1 + _S265 * ah_0[4U].xyz * (float4((&m_4->gain_0)->data_5[4U]) ).xyz;
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
                (&o_0)->albedo_1 = (&o_0)->albedo_1 + _S273 * ah_0[5U].xyz * (float4((&m_4->gain_0)->data_5[5U]) ).xyz;
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

float4 t_crown_0(float2 w_6, float cell_1, float pres_0, float rad_0, uint salt_0)
{
    int _S278;
    int2 _S279;
    float4 best_1;
    float2 x_0 = w_6 / float2(cell_1) ;
    float2 _S280 = floor(x_0);
    int _S281 = int(4096.0f / cell_1 + 0.5f);
    float4 _S282 = float4(0.0f, 0.0f, 9.0f, 0.0f);
    for(;;)
    {
        for(;;)
        {
            for(;;)
            {
                float2 c_3 = _S280 + float2(-1.0f, -1.0f);
                int2 _S283 = int2(c_3);
                int _S284 = _S283.x % _S281;
                int _S285 = (_S284 + _S281) % _S281;
                int _S286 = _S283.y % _S281;
                int _S287 = (_S286 + _S281) % _S281;
                int2 ci_0 = int2(_S285, _S287);
                int _S288 = int(salt_0);
                _S278 = _S288;
                int2 _S289 = int2(_S288, int(0));
                _S279 = _S289;
                if((t_hash2_0(ci_0 + _S289)) >= pres_0)
                {
                    best_1 = _S282;
                    break;
                }
                float h1_0 = t_hash2_0(ci_0 + int2(int(17), _S288 + int(5)));
                float2 _S290 = float2(0.5f) ;
                float2 q_5 = (x_0 - (c_3 + _S290 + (float2(h1_0, t_hash2_0(ci_0 + int2(_S288 + int(3), int(7)))) - _S290) * float2(0.80000001192092896f) )) / float2((rad_0 * mix(0.60000002384185791f, 1.3500000238418579f, t_hash2_0(ci_0 + int2(_S288 + int(9), int(31)))))) ;
                float dq_0 = length(q_5);
                if(dq_0 < 9.0f)
                {
                    best_1 = float4(q_5, dq_0, h1_0);
                }
                else
                {
                    best_1 = _S282;
                }
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                float2 c_4 = _S280 + float2(0.0f, -1.0f);
                int2 _S291 = int2(c_4);
                int _S292 = _S291.x % _S281;
                int _S293 = (_S292 + _S281) % _S281;
                int _S294 = _S291.y % _S281;
                int _S295 = (_S294 + _S281) % _S281;
                int2 ci_1 = int2(_S293, _S295);
                if((t_hash2_0(ci_1 + _S279)) >= pres_0)
                {
                    break;
                }
                float h1_1 = t_hash2_0(ci_1 + int2(int(17), _S278 + int(5)));
                float2 _S296 = float2(0.5f) ;
                float2 q_6 = (x_0 - (c_4 + _S296 + (float2(h1_1, t_hash2_0(ci_1 + int2(_S278 + int(3), int(7)))) - _S296) * float2(0.80000001192092896f) )) / float2((rad_0 * mix(0.60000002384185791f, 1.3500000238418579f, t_hash2_0(ci_1 + int2(_S278 + int(9), int(31)))))) ;
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
                float2 c_5 = _S280 + float2(1.0f, -1.0f);
                int2 _S297 = int2(c_5);
                int _S298 = _S297.x % _S281;
                int _S299 = (_S298 + _S281) % _S281;
                int _S300 = _S297.y % _S281;
                int _S301 = (_S300 + _S281) % _S281;
                int2 ci_2 = int2(_S299, _S301);
                if((t_hash2_0(ci_2 + _S279)) >= pres_0)
                {
                    break;
                }
                float h1_2 = t_hash2_0(ci_2 + int2(int(17), _S278 + int(5)));
                float2 _S302 = float2(0.5f) ;
                float2 q_7 = (x_0 - (c_5 + _S302 + (float2(h1_2, t_hash2_0(ci_2 + int2(_S278 + int(3), int(7)))) - _S302) * float2(0.80000001192092896f) )) / float2((rad_0 * mix(0.60000002384185791f, 1.3500000238418579f, t_hash2_0(ci_2 + int2(_S278 + int(9), int(31)))))) ;
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
                float2 c_6 = _S280 + float2(-1.0f, 0.0f);
                int2 _S303 = int2(c_6);
                int _S304 = _S303.x % _S281;
                int _S305 = (_S304 + _S281) % _S281;
                int _S306 = _S303.y % _S281;
                int _S307 = (_S306 + _S281) % _S281;
                int2 ci_3 = int2(_S305, _S307);
                if((t_hash2_0(ci_3 + _S279)) >= pres_0)
                {
                    break;
                }
                float h1_3 = t_hash2_0(ci_3 + int2(int(17), _S278 + int(5)));
                float2 _S308 = float2(0.5f) ;
                float2 q_8 = (x_0 - (c_6 + _S308 + (float2(h1_3, t_hash2_0(ci_3 + int2(_S278 + int(3), int(7)))) - _S308) * float2(0.80000001192092896f) )) / float2((rad_0 * mix(0.60000002384185791f, 1.3500000238418579f, t_hash2_0(ci_3 + int2(_S278 + int(9), int(31)))))) ;
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
                int2 _S309 = int2(_S280);
                int _S310 = _S309.x % _S281;
                int _S311 = (_S310 + _S281) % _S281;
                int _S312 = _S309.y % _S281;
                int _S313 = (_S312 + _S281) % _S281;
                int2 ci_4 = int2(_S311, _S313);
                if((t_hash2_0(ci_4 + _S279)) >= pres_0)
                {
                    break;
                }
                float h1_4 = t_hash2_0(ci_4 + int2(int(17), _S278 + int(5)));
                float2 _S314 = float2(0.5f) ;
                float2 q_9 = (x_0 - (_S280 + _S314 + (float2(h1_4, t_hash2_0(ci_4 + int2(_S278 + int(3), int(7)))) - _S314) * float2(0.80000001192092896f) )) / float2((rad_0 * mix(0.60000002384185791f, 1.3500000238418579f, t_hash2_0(ci_4 + int2(_S278 + int(9), int(31)))))) ;
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
                float2 c_7 = _S280 + float2(1.0f, 0.0f);
                int2 _S315 = int2(c_7);
                int _S316 = _S315.x % _S281;
                int _S317 = (_S316 + _S281) % _S281;
                int _S318 = _S315.y % _S281;
                int _S319 = (_S318 + _S281) % _S281;
                int2 ci_5 = int2(_S317, _S319);
                if((t_hash2_0(ci_5 + _S279)) >= pres_0)
                {
                    break;
                }
                float h1_5 = t_hash2_0(ci_5 + int2(int(17), _S278 + int(5)));
                float2 _S320 = float2(0.5f) ;
                float2 q_10 = (x_0 - (c_7 + _S320 + (float2(h1_5, t_hash2_0(ci_5 + int2(_S278 + int(3), int(7)))) - _S320) * float2(0.80000001192092896f) )) / float2((rad_0 * mix(0.60000002384185791f, 1.3500000238418579f, t_hash2_0(ci_5 + int2(_S278 + int(9), int(31)))))) ;
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
                float2 c_8 = _S280 + float2(-1.0f, 1.0f);
                int2 _S321 = int2(c_8);
                int _S322 = _S321.x % _S281;
                int _S323 = (_S322 + _S281) % _S281;
                int _S324 = _S321.y % _S281;
                int _S325 = (_S324 + _S281) % _S281;
                int2 ci_6 = int2(_S323, _S325);
                if((t_hash2_0(ci_6 + _S279)) >= pres_0)
                {
                    break;
                }
                float h1_6 = t_hash2_0(ci_6 + int2(int(17), _S278 + int(5)));
                float2 _S326 = float2(0.5f) ;
                float2 q_11 = (x_0 - (c_8 + _S326 + (float2(h1_6, t_hash2_0(ci_6 + int2(_S278 + int(3), int(7)))) - _S326) * float2(0.80000001192092896f) )) / float2((rad_0 * mix(0.60000002384185791f, 1.3500000238418579f, t_hash2_0(ci_6 + int2(_S278 + int(9), int(31)))))) ;
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
                float2 c_9 = _S280 + float2(0.0f, 1.0f);
                int2 _S327 = int2(c_9);
                int _S328 = _S327.x % _S281;
                int _S329 = (_S328 + _S281) % _S281;
                int _S330 = _S327.y % _S281;
                int _S331 = (_S330 + _S281) % _S281;
                int2 ci_7 = int2(_S329, _S331);
                if((t_hash2_0(ci_7 + _S279)) >= pres_0)
                {
                    break;
                }
                float h1_7 = t_hash2_0(ci_7 + int2(int(17), _S278 + int(5)));
                float2 _S332 = float2(0.5f) ;
                float2 q_12 = (x_0 - (c_9 + _S332 + (float2(h1_7, t_hash2_0(ci_7 + int2(_S278 + int(3), int(7)))) - _S332) * float2(0.80000001192092896f) )) / float2((rad_0 * mix(0.60000002384185791f, 1.3500000238418579f, t_hash2_0(ci_7 + int2(_S278 + int(9), int(31)))))) ;
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
                float2 c_10 = _S280 + float2(1.0f, 1.0f);
                int2 _S333 = int2(c_10);
                int _S334 = _S333.x % _S281;
                int _S335 = (_S334 + _S281) % _S281;
                int _S336 = _S333.y % _S281;
                int _S337 = (_S336 + _S281) % _S281;
                int2 ci_8 = int2(_S335, _S337);
                if((t_hash2_0(ci_8 + _S279)) >= pres_0)
                {
                    break;
                }
                float h1_8 = t_hash2_0(ci_8 + int2(int(17), _S278 + int(5)));
                float2 _S338 = float2(0.5f) ;
                float2 q_13 = (x_0 - (c_10 + _S338 + (float2(h1_8, t_hash2_0(ci_8 + int2(_S278 + int(3), int(7)))) - _S338) * float2(0.80000001192092896f) )) / float2((rad_0 * mix(0.60000002384185791f, 1.3500000238418579f, t_hash2_0(ci_8 + int2(_S278 + int(9), int(31)))))) ;
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

TForest_0 t_forest_0(uint pool_17, const TerrainDraw_natural_0 thread* d_19, float4 cv_1, float2 g_5, float2 w_7, float3 tu_4, float3 tv_5, float3 up_3, float3 n_4, float3 floor_alb_0, float3 l_3, float px_m_0, float px_iso_0, float spacing_1, float near_px_0, KernelContext_0 thread* kernelContext_25)
{
    float _S339 = t_noise_0(w_7 + float2(911.0f) , 64.0f);
    float _S340 = _S339 * 0.64999997615814209f;
    float _S341 = t_noise_0(w_7 + float2(271.0f) , 16.0f);
    float _S342 = cv_1.x;
    float c_11 = saturate(_S342 + (_S340 + _S341 * 0.34999999403953552f - 0.5f) * 1.6000000238418579f * _S342 * (1.0f - _S342));
    float _S343 = max(cv_1.y, 2.0f);
    float mx_3 = cv_1.z;
    float3 leaf_0 = mix(float3(0.05000000074505806f, 0.10999999940395355f, 0.024000000208616257f), float3(0.026000000536441803f, 0.064000003039836884f, 0.019999999552965164f), float3(mx_3) );
    float n1_1 = t_noise_0(w_7 + float2(151.0f) , 32.0f);
    float n2_1 = t_noise_0(w_7 + float2(517.0f) , 128.0f);
    float stand_0 = mix(0.80000001192092896f, 1.2000000476837158f, n1_1) * mix(0.87999999523162842f, 1.1200000047683716f, n2_1);
    float2 sun2_0 = float2(dot(l_3, tu_4), dot(l_3, tv_5));
    float2 toward_0 = sun2_0 / float2(max(dot(l_3, up_3), 0.059999998658895493f)) ;
    float _S344 = max(_S343, 20.0f);
    float4 _S345 = t_canopy_bilin_0(pool_17, (as_type<uint>(((float4(d_19->tex_0) ).z))), g_5 * float2(4.0f)  + toward_0 * float2(_S344)  / float2(max(spacing_1, 0.0099999997764825821f)) , kernelContext_25);
    float page_sh_0 = _S345.x * smoothstep(0.40000000596046448f, 0.89999997615814209f, _S345.y / _S344);
    float3 fl_1 = mix(floor_alb_0, float3(0.05000000074505806f, 0.041999999433755875f, 0.029999999329447746f), float3((0.5f * c_11)) );
    float fl_ao_0 = mix(1.0f, 0.5f, c_11);
    float rad_1 = mix(0.41999998688697815f, 0.62000000476837158f, saturate(_S343 / 30.0f)) + 0.20000000298023224f * smoothstep(0.60000002384185791f, 1.0f, c_11);
    float pres_1 = saturate(c_11 / (3.1415901184082031f * rad_1 * rad_1));
    float pb_0 = pres_1 * (1.0f - mx_3);
    float pc_0 = pres_1 * mx_3;
    float _S346 = mix(8.0f, 5.119999885559082f, mx_3) * rad_1 * 2.0f;
    float crown_px_0 = _S346 / max(px_m_0, 0.0010000000474974513f);
    float det_1 = smoothstep(1.5f, 4.0f, crown_px_0);
    float _S347 = smoothstep(80.0f, 40.0f, _S346 / max(px_iso_0, 0.0010000000474974513f));
    bool near_on_0 = near_px_0 >= 0.0f;
    float paint_0;
    if(near_on_0)
    {
        FrameData_natural_0 device* _S348 = ed_buf_1(kernelContext_25->ed_c_0->frame_0, kernelContext_25);
        float4 _S349 = float4((&_S348->imp_0)->far_0) ;
        float _S350 = _S349.z;
        if(_S350 > 0.0f)
        {
            paint_0 = saturate((_S350 - near_px_0) / max(_S350 - _S349.w, 0.0010000000474974513f));
        }
        else
        {
            paint_0 = 1.0f - smoothstep(12.0f, 15.0f, near_px_0);
        }
    }
    else
    {
        paint_0 = _S347;
    }
    float3 _S351 = float3(stand_0) ;
    float floor_sun_far_0 = 1.0f - 0.92000001668930054f * max(c_11, page_sh_0);
    float ao_far_0 = mix(fl_ao_0, 0.85000002384185791f, c_11);
    float sun_far_0 = mix(floor_sun_far_0, 1.0f, c_11);
    thread TForest_0 o_1;
    (&o_1)->albedo_2 = mix(fl_1, leaf_0 * float3(0.81999999284744263f)  * _S351, float3(c_11) );
    (&o_1)->n_3 = n_4;
    (&o_1)->ao_3 = ao_far_0;
    (&o_1)->sun_1 = sun_far_0;
    (&o_1)->crown_0 = c_11;
    bool _S352;
    if(det_1 > 0.0f)
    {
        if(!near_on_0)
        {
            _S352 = true;
        }
        else
        {
            _S352 = paint_0 > 0.0f;
        }
    }
    else
    {
        _S352 = false;
    }
    if(_S352)
    {
        bool _S353 = pb_0 > 0.0f;
        float4 kb_0;
        if(_S353)
        {
            float4 _S354 = t_crown_0(w_7, 8.0f, pb_0, rad_1, 3U);
            kb_0 = _S354;
        }
        else
        {
            kb_0 = float4(0.0f, 0.0f, 9.0f, 0.0f);
        }
        bool _S355 = pc_0 > 0.0f;
        float4 kc_0;
        if(_S355)
        {
            float4 _S356 = t_crown_0(w_7 + float2(1000.0f) , 5.119999885559082f, pc_0, rad_1 * 0.89999997615814209f, 11U);
            kc_0 = _S356;
        }
        else
        {
            kc_0 = float4(0.0f, 0.0f, 9.0f, 0.0f);
        }
        bool conifer_0 = (kc_0.z) < (kb_0.z);
        float4 k_7;
        if(conifer_0)
        {
            k_7 = kc_0;
        }
        else
        {
            k_7 = kb_0;
        }
        float in_crown_0 = smoothstep(1.0f, 0.85000002384185791f, k_7.z);
        float3 t2_0 = tu_4 * float3(k_7.x)  + tv_5 * float3(k_7.y) ;
        float2 _S357 = k_7.xy;
        float _S358 = min(dot(_S357, _S357), 1.0f);
        float3 nc_0;
        if(conifer_0)
        {
            nc_0 = normalize(up_3 + t2_0 * float3(1.2999999523162842f) );
        }
        else
        {
            nc_0 = normalize(t2_0 + up_3 * float3(sqrt(max(1.0f - _S358, 0.039999999105930328f))) );
        }
        float3 nn_0 = normalize(n_4 + (nc_0 - up_3) * float3(0.89999997615814209f) );
        if(conifer_0)
        {
            nc_0 = float3(0.026000000536441803f, 0.064000003039836884f, 0.019999999552965164f);
        }
        else
        {
            nc_0 = float3(0.05000000074505806f, 0.10999999940395355f, 0.024000000208616257f);
        }
        float _S359 = k_7.w;
        float3 la_0 = nc_0 * float3(mix(0.75f, 1.25f, _S359))  * _S351 * mix(float3(1.1200000047683716f, 1.0f, 0.75f), float3(0.85000002384185791f, 0.97000002861022949f, 1.2000000476837158f), float3(fract(_S359 * 7.309999942779541f)) );
        float gap_sun_0;
        if(conifer_0)
        {
            gap_sun_0 = 1.6000000238418579f;
        }
        else
        {
            gap_sun_0 = 1.0f;
        }
        float _S360 = t_noise_0(w_7 * float2(gap_sun_0)  + float2(13.0f) , 1.0f);
        float3 la_1 = la_0 * float3(mix(1.0f, mix(0.69999998807907104f, 1.25f, _S360), smoothstep(6.0f, 16.0f, crown_px_0))) ;
        float crown_ao_0 = mix(0.69999998807907104f, 1.0f, 1.0f - _S358) * mix(1.0f, 0.69999998807907104f, (1.0f - saturate(0.5f + 0.80000001192092896f * dot(_S357, sun2_0 / float2(max(length(sun2_0), 0.0010000000474974513f)) ))) * 0.60000002384185791f);
        if(crown_px_0 > 3.0f)
        {
            float2 wt_1 = w_7 + toward_0 * float2(_S343)  * float2(0.55000001192092896f) ;
            float4 sb_0;
            if(_S353)
            {
                float4 _S361 = t_crown_0(wt_1, 8.0f, pb_0, rad_1, 3U);
                sb_0 = _S361;
            }
            else
            {
                sb_0 = float4(0.0f, 0.0f, 9.0f, 0.0f);
            }
            float4 sc_3;
            if(_S355)
            {
                float4 _S362 = t_crown_0(wt_1 + float2(1000.0f) , 5.119999885559082f, pc_0, rad_1 * 0.89999997615814209f, 11U);
                sc_3 = _S362;
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
        float _S363 = min(gap_sun_0, 1.0f - 0.89999997615814209f * page_sh_0);
        float closed_0 = smoothstep(0.5f, 0.94999998807907104f, c_11);
        float3 _S364 = float3(in_crown_0) ;
        float3 n_near_0 = normalize(mix(n_4, nn_0, _S364));
        float ao_near_0 = mix(mix(fl_ao_0, 0.75f, closed_0), crown_ao_0, in_crown_0);
        float sun_near_0 = mix(mix(_S363, max(_S363, 0.34999999403953552f), closed_0), 1.0f, in_crown_0);
        float3 _S365 = float3(det_1) ;
        (&o_1)->albedo_2 = mix((&o_1)->albedo_2, mix(mix(fl_1, leaf_0 * _S351 * float3(0.85000002384185791f) , float3(closed_0) ), la_1, _S364), _S365);
        (&o_1)->n_3 = normalize(mix((&o_1)->n_3, n_near_0, _S365));
        (&o_1)->ao_3 = mix((&o_1)->ao_3, ao_near_0, det_1);
        (&o_1)->sun_1 = mix((&o_1)->sun_1, sun_near_0, det_1);
        (&o_1)->crown_0 = mix((&o_1)->crown_0, in_crown_0, det_1);
    }
    if(paint_0 < 1.0f)
    {
        float _S366 = t_noise_0(w_7 + float2(77.0f) , 4.0f);
        float _S367 = _S366 * 0.60000002384185791f;
        float _S368 = t_noise_0(w_7 + float2(33.0f) , 1.0f);
        float _S369 = min(mix(1.0f, 0.40000000596046448f + 0.60000002384185791f * smoothstep(0.34999999403953552f, 0.75f, _S367 + _S368 * 0.40000000596046448f), c_11), 1.0f - 0.60000002384185791f * page_sh_0);
        float3 _S370 = float3(paint_0) ;
        (&o_1)->albedo_2 = mix(fl_1, (&o_1)->albedo_2, _S370);
        (&o_1)->n_3 = normalize(mix(n_4, (&o_1)->n_3, _S370));
        float ao_crown_0 = (&o_1)->ao_3;
        float sun_crown_0 = (&o_1)->sun_1;
        (&o_1)->ao_3 = mix(max(fl_ao_0, 0.75f), (&o_1)->ao_3, paint_0);
        (&o_1)->sun_1 = mix(_S369, (&o_1)->sun_1, paint_0);
        if(near_on_0)
        {
            (&o_1)->ao_3 = mix(fl_ao_0, ao_crown_0, paint_0);
            (&o_1)->sun_1 = mix(1.0f - 0.5f * page_sh_0, sun_crown_0, paint_0);
        }
        (&o_1)->crown_0 = (&o_1)->crown_0 * paint_0;
    }
    return o_1;
}

float cascade_extent_0(FrameData_natural_0 device* f_4, uint c_12, float3 p_8)
{
    float4 s_10 = (((float4(p_8, 1.0f)) * (matrix<float,int(4),int(4)> ((&(&f_4->light_0)->shadow_1)->data_2[c_12].data_0[int(0)][int(0)], (&(&f_4->light_0)->shadow_1)->data_2[c_12].data_0[int(1)][int(0)], (&(&f_4->light_0)->shadow_1)->data_2[c_12].data_0[int(2)][int(0)], (&(&f_4->light_0)->shadow_1)->data_2[c_12].data_0[int(3)][int(0)], (&(&f_4->light_0)->shadow_1)->data_2[c_12].data_0[int(0)][int(1)], (&(&f_4->light_0)->shadow_1)->data_2[c_12].data_0[int(1)][int(1)], (&(&f_4->light_0)->shadow_1)->data_2[c_12].data_0[int(2)][int(1)], (&(&f_4->light_0)->shadow_1)->data_2[c_12].data_0[int(3)][int(1)], (&(&f_4->light_0)->shadow_1)->data_2[c_12].data_0[int(0)][int(2)], (&(&f_4->light_0)->shadow_1)->data_2[c_12].data_0[int(1)][int(2)], (&(&f_4->light_0)->shadow_1)->data_2[c_12].data_0[int(2)][int(2)], (&(&f_4->light_0)->shadow_1)->data_2[c_12].data_0[int(3)][int(2)], (&(&f_4->light_0)->shadow_1)->data_2[c_12].data_0[int(0)][int(3)], (&(&f_4->light_0)->shadow_1)->data_2[c_12].data_0[int(1)][int(3)], (&(&f_4->light_0)->shadow_1)->data_2[c_12].data_0[int(2)][int(3)], (&(&f_4->light_0)->shadow_1)->data_2[c_12].data_0[int(3)][int(3)]))));
    float _S371 = s_10.z;
    bool _S372;
    if(_S371 < 0.0f)
    {
        _S372 = true;
    }
    else
    {
        _S372 = _S371 > 1.0f;
    }
    float _S373;
    if(_S372)
    {
        _S373 = 1.0e+09f;
    }
    else
    {
        _S373 = max(abs(s_10.x), abs(s_10.y));
    }
    return _S373;
}

float pcf_radius_texels_0(FrameData_natural_0 device* f_5, float texel_0)
{
    return clamp((float4((&f_5->light_0)->sky_0) ).z / texel_0, 1.0f, max((float4((&f_5->light_0)->sky_0) ).w, 1.0f));
}

float ed_depth_read_0(uint i_16, uint2 p_9, KernelContext_0 thread* kernelContext_26)
{
    float _S374 = ((reinterpret_cast<device depth2d<float, access::sample>*>((kernelContext_26->ed_textures_0))[(i_16)].read((p_9))));
    return _S374;
}

float cascade_lookup_0(FrameData_natural_0 device* f_6, uint atlas_0, uint c_13, float3 p_10, float3 ng_0, float2 px_1, KernelContext_0 thread* kernelContext_27)
{
    float4 _S375 = float4((&(&f_6->light_0)->cascade_0)->data_3[c_13]) ;
    float tile_1 = (float4((&f_6->light_0)->cfg_0) ).y;
    float _S376 = _S375.x;
    float _S377 = pcf_radius_texels_0(f_6, _S376);
    float sp_0 = _S377 * 0.5f;
    float nol_0 = saturate(dot(ng_0, (float4(f_6->sun_dir_0) ).xyz));
    float4 s_11 = (((float4(p_10 + ng_0 * float3((_S376 * ((float4((&f_6->light_0)->filter_0) ).z + 2.0f * sqrt(1.0f - nol_0 * nol_0)))) , 1.0f)) * (matrix<float,int(4),int(4)> ((&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(0)][int(0)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(1)][int(0)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(2)][int(0)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(3)][int(0)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(0)][int(1)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(1)][int(1)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(2)][int(1)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(3)][int(1)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(0)][int(2)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(1)][int(2)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(2)][int(2)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(3)][int(2)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(0)][int(3)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(1)][int(3)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(2)][int(3)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(3)][int(3)]))));
    float2 _S378 = float2(s_11.x * 0.5f + 0.5f, 0.5f - s_11.y * 0.5f) * float2(tile_1) ;
    float ign_0 = fract(52.982917785644531f * fract(dot(px_1, float2(0.067110560834407806f, 0.0058371499180793762f))));
    float2 _S379 = float2(0.5f) ;
    float2 _S380 = (float2(ign_0, fract(ign_0 * 7.309999942779541f + 0.37000000476837158f)) - _S379) * float2(0.0f) ;
    float3 ns_1 = (((float4(ng_0, 0.0f)) * (matrix<float,int(4),int(4)> ((&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(0)][int(0)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(1)][int(0)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(2)][int(0)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(3)][int(0)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(0)][int(1)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(1)][int(1)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(2)][int(1)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(3)][int(1)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(0)][int(2)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(1)][int(2)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(2)][int(2)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(3)][int(2)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(0)][int(3)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(1)][int(3)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(2)][int(3)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(3)][int(3)])))).xyz;
    float rl_0 = _S376 * tile_1 * 0.5f;
    float _S381 = _S375.y;
    float _S382 = ns_1.z * _S381;
    float izn_0 = 1.0f / max(abs(_S382), 0.11999999731779099f);
    float _S383 = float((int(sign((_S382)))));
    float gx_1 = clamp(- (ns_1.x * rl_0) * izn_0 * _S383, -8.0f, 8.0f);
    float gy_1 = clamp(- (ns_1.y * rl_0) * izn_0 * _S383, -8.0f, 8.0f);
    float _S384 = s_11.z + ((float4((&f_6->light_0)->filter_0) ).w + 0.5f * (abs(gx_1) + abs(gy_1))) * _S376 / _S381;
    float _S385 = _S376 / _S381;
    int2 _S386 = int2(_S375.zw);
    int2 _S387 = _S386 + int2((int(tile_1) - int(1))) ;
    float _S388 = 1.0f / (_S377 + sp_0);
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
        float _S389 = 1.0f - abs(oy_0) * _S388;
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
            float w_8 = (1.0f - abs(ox_0) * _S388) * _S389;
            float2 tp_1 = floor(_S378 + float2(ox_0, oy_0) + _S380);
            float2 d_20 = tp_1 + _S379 - _S378;
            float _S390 = ed_depth_read_0(atlas_0, uint2(clamp(_S386 + int2(tp_1), _S386, _S387)), kernelContext_27);
            float _S391;
            if((_S384 + (gx_1 * d_20.x - gy_1 * d_20.y) * _S385) >= _S390)
            {
                _S391 = 1.0f;
            }
            else
            {
                _S391 = 0.0f;
            }
            float lit_1 = lit_0 + _S391 * w_8;
            float wsum_1 = wsum_0 + w_8;
            x_1 = x_1 + int(1);
            lit_0 = lit_1;
            wsum_0 = wsum_1;
        }
        y_0 = y_0 + int(1);
    }
    return lit_0 / wsum_0;
}

float sun_shadow_0(FrameData_natural_0 device* f_7, uint atlas_1, float3 p_11, float3 ng_1, float dist_1, float2 px_2, KernelContext_0 thread* kernelContext_28)
{
    uint n_5 = uint((float4((&f_7->light_0)->cfg_0) ).x);
    bool _S392;
    if(((uint4((&f_7->light_0)->flags_0) ).x) == 0U)
    {
        _S392 = true;
    }
    else
    {
        _S392 = n_5 == 0U;
    }
    if(_S392)
    {
        _S392 = true;
    }
    else
    {
        _S392 = dist_1 >= ((float4((&f_7->light_0)->filter_0) ).y);
    }
    if(_S392)
    {
        return 1.0f;
    }
    float _S393 = 1.0f - (float4((&f_7->light_0)->cfg_0) ).w * 2.0f / (float4((&f_7->light_0)->cfg_0) ).y;
    float _S394 = (float4((&f_7->light_0)->cfg_0) ).z;
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
        float _S395 = cascade_extent_0(f_7, c_14, p_11);
        float e_8 = _S395 / _S393;
        if(e_8 > 1.0f)
        {
            c_14 = c_14 + 1U;
            continue;
        }
        float _S396 = cascade_lookup_0(f_7, atlas_1, c_14, p_11, ng_1, px_2, kernelContext_28);
        float blend_0 = smoothstep(1.0f - _S394, 1.0f, e_8);
        float v_3;
        if(blend_0 > 0.0f)
        {
            uint _S397 = c_14 + 1U;
            if(_S397 < n_5)
            {
                float _S398 = cascade_extent_0(f_7, _S397, p_11);
                _S392 = (_S398 / _S393) <= 1.0f;
            }
            else
            {
                _S392 = false;
            }
            if(_S392)
            {
                float _S399 = cascade_lookup_0(f_7, atlas_1, _S397, p_11, ng_1, px_2, kernelContext_28);
                v_3 = _S399;
            }
            else
            {
                v_3 = 1.0f;
            }
            v_3 = mix(_S396, v_3, blend_0);
        }
        else
        {
            v_3 = _S396;
        }
        return mix(v_3, 1.0f, smoothstep(0.0f, 1.0f, saturate((dist_1 - (float4((&f_7->light_0)->filter_0) ).x) / max((float4((&f_7->light_0)->filter_0) ).y - (float4((&f_7->light_0)->filter_0) ).x, 0.0010000000474974513f))));
    }
    return 1.0f;
}

packed_float4 device* ed_buf_4(uint i_17, KernelContext_0 thread* kernelContext_29)
{
    return (packed_float4 device*)kernelContext_29->ed_buffers_0[i_17];
}

float2 ao_upsample_0(FrameData_natural_0 device* f_8, uint ao_buf_0, float2 pixel_1, float zlin_0, KernelContext_0 thread* kernelContext_30)
{
    float sv_0;
    if(((uint4((&f_8->light_0)->flags_0) ).y) == 0U)
    {
        return float2(1.0f) ;
    }
    packed_float4 device* _S400 = ed_buf_4(ao_buf_0, kernelContext_30);
    uint _S401 = uint((float4((&f_8->light_0)->ao_1) ).y);
    uint _S402 = uint((float4((&f_8->light_0)->ao_1) ).z);
    float2 _S403 = float2(0.5f) ;
    float2 h_10 = (pixel_1 - _S403) * _S403;
    float2 b_6 = floor(h_10);
    float2 _S404 = h_10 - b_6;
    float2 _S405 = float2(0.0f) ;
    int y_1 = int(0);
    float2 sum_0 = _S405;
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
            int2 t_1 = clamp(int2(b_6) + int2(x_2, y_1), int2(int(0)) , int2(int(_S401) - int(1), int(_S402) - int(1)));
            float4 _S406 = float4(*(_S400 + (uint(t_1.y) * _S401 + uint(t_1.x)))) ;
            if(x_2 == int(0))
            {
                sv_0 = 1.0f - _S404.x;
            }
            else
            {
                sv_0 = _S404.x;
            }
            float _S407;
            if(y_1 == int(0))
            {
                _S407 = 1.0f - _S404.y;
            }
            else
            {
                _S407 = _S404.y;
            }
            float w_9 = max(sv_0 * _S407, 0.0010000000474974513f) * (1.0f / (0.0010000000474974513f + abs(_S406.z - zlin_0) / max(zlin_0, 0.0010000000474974513f) * 50.0f));
            float2 sum_1 = sum_0 + _S406.xy * float2(w_9) ;
            float wsum_3 = wsum_2 + w_9;
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

float atm_height_at_0(float rb_0, float h_11, float mu_0, float t_2)
{
    float q_14 = t_2 * t_2 + 2.0f * (rb_0 + h_11) * mu_0 * t_2 + h_11 * (2.0f * rb_0 + h_11);
    return q_14 / (sqrt(max(rb_0 * rb_0 + q_14, 0.0f)) + rb_0);
}

float atm_rt_0(FrameData_natural_0 device* f_11)
{
    return (float4((&f_11->atmos_0)->radii_0) ).y;
}

float atm_dist_top_0(float rb_1, float rt_0, float h_12, float mu_1)
{
    float r_4 = rb_1 + h_12;
    float _S408 = r_4 * mu_1;
    return max(- r_4 * mu_1 + sqrt(max(_S408 * _S408 + (rt_0 - rb_1 - h_12) * (rt_0 + r_4), 0.0f)), 0.0f);
}

float2 atm_trans_uv_0(float rb_2, float rt_1, float h_13, float mu_2)
{
    float ht_0 = rt_1 - rb_2;
    float _S409 = clamp(h_13, 0.0f, ht_0);
    float big_h_0 = sqrt(ht_0 * (rt_1 + rb_2));
    float rho_0 = sqrt(max(_S409 * (2.0f * rb_2 + _S409), 0.0f));
    float dmin_0 = ht_0 - _S409;
    return float2((atm_dist_top_0(rb_2, rt_1, _S409, mu_2) - dmin_0) / max(rho_0 + big_h_0 - dmin_0, 9.99999997475242708e-07f), rho_0 / big_h_0);
}

float4 atm_bilinear_0(packed_float4 device* buf_0, uint w_10, uint row0_0, uint rows_0, float2 uv_1)
{
    float2 p_12 = saturate(uv_1) * float2(float(w_10 - 1U), float(rows_0 - 1U));
    uint2 _S410 = min(uint2(p_12), uint2(w_10 - 2U, rows_0 - 2U));
    float2 g_6 = p_12 - float2(_S410);
    uint b_7 = (row0_0 + _S410.y) * w_10 + _S410.x;
    float4 _S411 = float4(g_6.x) ;
    uint _S412 = b_7 + w_10;
    return mix(mix(float4(*(buf_0 + b_7)) , float4(*(buf_0 + (b_7 + 1U))) , _S411), mix(float4(*(buf_0 + _S412)) , float4(*(buf_0 + (_S412 + 1U))) , _S411), float4(g_6.y) );
}

float3 atm_transmittance_0(FrameData_natural_0 device* f_12, float h_14, float mu_3, KernelContext_0 thread* kernelContext_31)
{
    float _S413 = atm_rb_0(f_12);
    float _S414 = atm_rt_0(f_12);
    float2 uv_2 = atm_trans_uv_0(_S413, _S414, h_14, mu_3);
    packed_float4 device* _S415 = ed_buf_4((uint4((&f_12->atmos_0)->bufs_0) ).x, kernelContext_31);
    float4 _S416 = atm_bilinear_0(_S415, 256U, 0U, 64U, uv_2);
    return _S416.xyz;
}

float atm_rb_1(FrameData_natural_0 device* f_13)
{
    return (float4((&f_13->atmos_0)->radii_0) ).x;
}

float atm_cos_horizon_0(float rb_3, float h_15)
{
    return - sqrt(max(h_15 * (2.0f * rb_3 + h_15), 0.0f)) / (rb_3 + h_15);
}

float atm_earth_shadow_0(FrameData_natural_0 device* f_14, float h_16, float mu_s_0)
{
    float _S417 = atm_rb_1(f_14);
    float ch_0 = atm_cos_horizon_0(_S417, h_16);
    float s_12 = 0.0046999999321997166f * sqrt(max(1.0f - ch_0 * ch_0, 0.0f)) + 9.9999997473787516e-06f;
    return smoothstep(- s_12, s_12, mu_s_0 - ch_0);
}

float3 atm_sun_transmittance_0(FrameData_natural_0 device* f_15, float h_17, float mu_s_1, KernelContext_0 thread* kernelContext_32)
{
    float3 _S418 = atm_transmittance_0(f_15, h_17, mu_s_1, kernelContext_32);
    float _S419 = atm_earth_shadow_0(f_15, h_17, mu_s_1);
    return _S418 * float3(_S419) ;
}

float3 atmos_sun_at_0(FrameData_natural_0 device* f_16, float3 p_13, KernelContext_0 thread* kernelContext_33)
{
    float t_3 = length(p_13) * 0.0010000000474974513f;
    float3 d_21;
    if(t_3 > 0.0f)
    {
        d_21 = p_13 * float3((0.0010000000474974513f / t_3)) ;
    }
    else
    {
        d_21 = (float4((&f_16->atmos_0)->up_0) ).xyz;
    }
    float _S420 = atm_rb_0(f_16);
    float h0_0 = (float4((&f_16->atmos_0)->radii_0) ).z;
    float h_18 = atm_height_at_0(_S420, h0_0, dot(d_21, (float4((&f_16->atmos_0)->up_0) ).xyz), t_3);
    float3 _S421 = (float4((&f_16->atmos_0)->sun_0) ).xyz;
    float3 _S422 = atm_sun_transmittance_0(f_16, h_18, clamp(((_S420 + h0_0) * (float4((&f_16->atmos_0)->up_0) ).w + t_3 * dot(d_21, (float4(f_16->sun_dir_0) ).xyz)) / (_S420 + h_18), -1.0f, 1.0f), kernelContext_33);
    return _S421 * _S422;
}

float3 sun_light_at_0(FrameData_natural_0 device* f_17, float3 p_14, KernelContext_0 thread* kernelContext_34)
{
    bool _S423 = atmos_on_0(f_17);
    float3 _S424;
    if(_S423)
    {
        float3 _S425 = atmos_sun_at_0(f_17, p_14, kernelContext_34);
        _S424 = _S425;
    }
    else
    {
        _S424 = (float4(f_17->sun_color_0) ).xyz;
    }
    return _S424;
}

float3 f_schlick_0(float3 f0_0, float voh_0)
{
    return f0_0 + (float3(1.0f)  - f0_0) * float3(pow(1.0f - voh_0, 5.0f)) ;
}

float d_ggx_0(float noh_0, float a_12)
{
    float a2_0 = a_12 * a_12;
    float d_22 = noh_0 * noh_0 * (a2_0 - 1.0f) + 1.0f;
    return a2_0 / (3.1415927410125732f * d_22 * d_22);
}

float v_smith_0(float nov_0, float nol_1, float a_13)
{
    float a2_1 = a_13 * a_13;
    float _S426 = 1.0f - a2_1;
    return 0.5f / max(nol_1 * sqrt(nov_0 * nov_0 * _S426 + a2_1) + nov_0 * sqrt(nol_1 * nol_1 * _S426 + a2_1), 9.9999997473787516e-06f);
}

float2 env_brdf_0(float nov_1, float rough_1)
{
    float4 r_5 = float4(rough_1)  * float4(-1.0f, -0.027499999850988388f, -0.57200002670288086f, 0.02199999988079071f) + float4(1.0f, 0.042500000447034836f, 1.0399999618530273f, -0.039999999105930328f);
    float _S427 = r_5.x;
    return float2(-1.0399999618530273f, 1.0399999618530273f) * float2((min(_S427 * _S427, exp2(-9.2799997329711914f * nov_1)) * _S427 + r_5.y))  + r_5.zw;
}

bool atmos_on_1(FrameData_natural_0 device* f_18)
{
    return ((uint4((&f_18->atmos_0)->cfg_1) ).x) != 0U;
}

float3 atm_sky_view_0(FrameData_natural_0 device* f_19, uint buf_1, uint w_11, uint rows_1, float h_19, float3 d_23, KernelContext_0 thread* kernelContext_35)
{
    float mu_4 = clamp(dot(d_23, (float4((&f_19->atmos_0)->up_0) ).xyz), -1.0f, 1.0f);
    float mu_s_2 = (float4((&f_19->atmos_0)->up_0) ).w;
    float nu_0 = dot(d_23, (float4(f_19->sun_dir_0) ).xyz);
    float _S428 = sqrt(max(1.0f - mu_4 * mu_4, 0.0f)) * sqrt(max(1.0f - mu_s_2 * mu_s_2, 0.0f));
    float cphi_0;
    if(_S428 > 9.99999997475242708e-07f)
    {
        cphi_0 = clamp((nu_0 - mu_4 * mu_s_2) / _S428, -1.0f, 1.0f);
    }
    else
    {
        cphi_0 = 1.0f;
    }
    float u_0 = sqrt(acos(cphi_0) / 3.1415927410125732f);
    float _S429 = atm_rb_0(f_19);
    float zha_0 = acos(atm_cos_horizon_0(_S429, h_19));
    float theta_0 = acos(mu_4);
    uint half_rows_0 = rows_1 / 2U;
    float c_15;
    uint row0_1;
    if(theta_0 < zha_0)
    {
        float _S430 = 1.0f - sqrt(saturate(1.0f - theta_0 / zha_0));
        row0_1 = 0U;
        c_15 = _S430;
    }
    else
    {
        float _S431 = sqrt(saturate((theta_0 - zha_0) / max(3.1415927410125732f - zha_0, 9.99999997475242708e-07f)));
        row0_1 = half_rows_0;
        c_15 = _S431;
    }
    packed_float4 device* _S432 = ed_buf_4(buf_1, kernelContext_35);
    float4 _S433 = atm_bilinear_0(_S432, w_11, row0_1, half_rows_0, float2(u_0, c_15));
    return _S433.xyz;
}

float3 atmos_sky_0(FrameData_natural_0 device* f_20, float3 d_24, KernelContext_0 thread* kernelContext_36)
{
    float3 _S434 = atm_sky_view_0(f_20, (uint4((&f_20->atmos_0)->bufs_0) ).z, 192U, 108U, (float4((&f_20->atmos_0)->radii_0) ).z, d_24, kernelContext_36);
    return _S434;
}

float3 sky_rad_0(FrameData_natural_0 device* f_21, float3 d_25, float disc_k_0)
{
    float3 sun_2 = (float4(f_21->sun_dir_0) ).xyz;
    float h_20 = d_25.y;
    float t_4 = pow(saturate(1.0f - max(h_20, 0.0f)), 6.5f);
    float mu_5 = dot(d_25, sun_2);
    float warm_0 = saturate(1.0f - sun_2.y * 2.5f);
    float3 c_16 = mix((float4(f_21->sky_zenith_0) ).xyz, (float4(f_21->sky_horizon_0) ).xyz, float3(t_4) ) + float3(1.0f, 0.41999998688697815f, 0.14000000059604645f) * float3(pow(saturate(mu_5 * 0.5f + 0.5f), 5.0f))  * float3(warm_0)  * float3(0.85000002384185791f)  * float3((0.40000000596046448f + t_4)) ;
    float3 _S435 = float3((float4(f_21->sun_dir_0) ).w) ;
    return mix(mix(c_16, c_16 * float3(1.0f, 0.69999998807907104f, 0.55000001192092896f), float3((warm_0 * 0.40000000596046448f)) ) + float3(1.0f, 0.89999997615814209f, 0.69999998807907104f) * float3(pow(saturate(mu_5), 28.0f))  * float3(0.34999999403953552f) , (float4(f_21->ground_color_0) ).xyz, float3(saturate(- h_20 * 6.0f)) ) * _S435 + float3(1.0f, 0.94999998807907104f, 0.85000002384185791f) * float3((smoothstep(0.99965000152587891f, 0.99984997510910034f, mu_5) * step(0.0f, h_20)))  * float3(60.0f)  * _S435 * float3(disc_k_0) ;
}

float3 sky_env_0(FrameData_natural_0 device* f_22, float3 d_26, KernelContext_0 thread* kernelContext_37)
{
    bool _S436 = atmos_on_1(f_22);
    float3 _S437;
    if(_S436)
    {
        float3 _S438 = atmos_sky_0(f_22, d_26, kernelContext_37);
        _S437 = _S438;
    }
    else
    {
        float3 _S439 = sky_rad_0(f_22, d_26, 0.0f);
        _S437 = _S439;
    }
    return _S437;
}

float3 sh_irradiance_0(FrameData_natural_0 device* f_23, float3 n_6, KernelContext_0 thread* kernelContext_38)
{
    thread array<float4, int(9)> s_13;
    packed_float4 device* _S440 = ed_buf_4((uint4((&f_23->atmos_0)->bufs2_0) ).z, kernelContext_38);
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
        bool _S441 = atmos_on_0(f_23);
        float4 _S442;
        if(_S441)
        {
            _S442 = float4(*(_S440 + (288U + k_8))) ;
        }
        else
        {
            _S442 = float4((&(&f_23->light_0)->sh_0)->data_4[k_8]) ;
        }
        s_13[k_8] = _S442;
        k_8 = k_8 + 1U;
    }
    float _S443 = n_6.y;
    float _S444 = n_6.z;
    float _S445 = n_6.x;
    return max(s_13[int(0)].xyz + (s_13[int(1)].xyz * float3(_S443)  + s_13[int(2)].xyz * float3(_S444)  + s_13[int(3)].xyz * float3(_S445) ) + (s_13[int(4)].xyz * float3((_S445 * _S443))  + s_13[int(5)].xyz * float3((_S443 * _S444))  + s_13[int(6)].xyz * float3((3.0f * _S444 * _S444 - 1.0f)) ) + (s_13[int(7)].xyz * float3((_S445 * _S444))  + s_13[int(8)].xyz * float3((_S445 * _S445 - _S443 * _S443)) ), float3(0.0f) );
}

float3 hemi_ambient_0(FrameData_natural_0 device* f_24, float3 n_7)
{
    return mix((float4(f_24->ground_color_0) ).xyz * float3((float4(f_24->sun_dir_0) ).w) , mix((float4(f_24->sky_horizon_0) ).xyz, (float4(f_24->sky_zenith_0) ).xyz, float3(0.5f) ) * float3((float4(f_24->sun_dir_0) ).w) , float3((n_7.y * 0.5f + 0.5f)) );
}

float3 ambient_light_0(FrameData_natural_0 device* f_25, float3 n_8, KernelContext_0 thread* kernelContext_39)
{
    float3 _S446;
    if(((uint4((&f_25->light_0)->flags_0) ).z) != 0U)
    {
        float3 _S447 = sh_irradiance_0(f_25, n_8, kernelContext_39);
        _S446 = _S447;
    }
    else
    {
        float3 _S448 = hemi_ambient_0(f_25, n_8);
        _S446 = _S448;
    }
    return _S446;
}

float3 occluded_ambient_0(FrameData_natural_0 device* f_26, float3 open_0, float sv_1, KernelContext_0 thread* kernelContext_40)
{
    float3 _S449 = ambient_light_0(f_26, float3(0.0f, -1.0f, 0.0f), kernelContext_40);
    return mix(_S449 * float3((float4((&f_26->light_0)->skyvis_0) ).y) , open_0, float3(sv_1) );
}

float3 ao_multibounce_0(FrameData_natural_0 device* f_27, float ao_4, float3 albedo_3)
{
    float3 _S450 = float3(ao_4) ;
    return mix(_S450, max(_S450, ((_S450 * (float3(2.0404000282287598f)  * albedo_3 - float3(0.33239999413490295f) ) + (float3(-4.795100212097168f)  * albedo_3 + float3(0.64170002937316895f) )) * _S450 + (float3(2.7551999092102051f)  * albedo_3 + float3(0.69029998779296875f) )) * _S450), float3((float4((&f_27->light_0)->sky_0) ).y) );
}

float3 shade_pbr_0(FrameData_natural_0 device* f_28, float3 sun_3, float3 albedo_4, float metal_0, float rough_2, float ao_5, float sv_2, float sun_vis_0, float3 n_9, float3 v_4, KernelContext_0 thread* kernelContext_41)
{
    float _S451 = saturate(metal_0);
    float _S452 = clamp(rough_2, 0.039999999105930328f, 1.0f);
    float a_14 = _S452 * _S452;
    float3 l_4 = (float4(f_28->sun_dir_0) ).xyz;
    float3 h_21 = normalize(l_4 + v_4);
    float nol_2 = saturate(dot(n_9, l_4));
    float _S453 = max(dot(n_9, v_4), 9.9999997473787516e-05f);
    float3 f0_1 = mix(float3(0.039999999105930328f) , albedo_4, float3(_S451) );
    float3 diff_col_0 = albedo_4 * float3((1.0f - _S451)) ;
    float3 F_0 = f_schlick_0(f0_1, saturate(dot(v_4, h_21)));
    float3 _S454 = float3(1.0f) ;
    float3 direct_0 = (diff_col_0 * (_S454 - F_0) / float3(3.1415927410125732f)  + float3((d_ggx_0(saturate(dot(n_9, h_21)), a_14) * v_smith_0(_S453, nol_2, a_14)))  * F_0) * sun_3 * float3((nol_2 * sun_vis_0)) ;
    float2 ab_0 = env_brdf_0(_S453, _S452);
    float3 r_6 = reflect(- v_4, n_9);
    float3 env_0;
    if(((uint4((&f_28->light_0)->flags_0) ).z) != 0U)
    {
        float3 _S455 = sky_env_0(f_28, r_6, kernelContext_41);
        float3 _S456 = sh_irradiance_0(f_28, r_6, kernelContext_41);
        env_0 = mix(_S455, _S456, float3(_S452) );
    }
    else
    {
        float3 _S457 = hemi_ambient_0(f_28, r_6);
        env_0 = _S457;
    }
    float3 _S458 = occluded_ambient_0(f_28, env_0, sv_2, kernelContext_41);
    float spec_occ_0 = saturate(pow(_S453 + ao_5, exp2(-16.0f * a_14 - 1.0f)) - 1.0f + ao_5);
    float3 f_env_0 = f0_1 * float3(ab_0.x)  + float3(ab_0.y) ;
    float3 _S459 = diff_col_0 * (_S454 - f_env_0);
    float3 _S460 = ambient_light_0(f_28, n_9, kernelContext_41);
    float3 _S461 = occluded_ambient_0(f_28, _S460, sv_2, kernelContext_41);
    float3 _S462 = _S459 * _S461;
    float3 _S463 = ao_multibounce_0(f_28, ao_5, diff_col_0);
    return direct_0 + (_S462 * _S463 + _S458 * f_env_0 * float3(spec_occ_0) );
}

float atm_enter_top_0(float rb_4, float rt_2, float h_22, float mu_6)
{
    float r_7 = rb_4 + h_22;
    float _S464 = r_7 * mu_6;
    float d_27 = _S464 * _S464 - (h_22 - (rt_2 - rb_4)) * (r_7 + rt_2);
    if(d_27 < 0.0f)
    {
        return -1.0f;
    }
    float t_5 = - r_7 * mu_6 - sqrt(d_27);
    float _S465;
    if(t_5 >= 0.0f)
    {
        _S465 = t_5;
    }
    else
    {
        _S465 = -1.0f;
    }
    return _S465;
}

float atm_ap_slice_0(FrameData_natural_0 device* f_29, float t_6)
{
    float n_10 = (float4((&f_29->atmos_0)->ap_0) ).x;
    return log2(1.0f + max(t_6, 0.0f) / n_10) / log2(1.0f + (float4((&f_29->atmos_0)->ap_0) ).y / n_10) * 32.0f;
}

void atm_froxel_xy_0(packed_float4 device* l_5, packed_float4 device* t_7, float2 g_7, uint2 i_18, uint k_9, float3 thread* lo_1, float3 thread* to_0)
{
    uint b_8 = (k_9 * 32U + i_18.y) * 32U + i_18.x;
    uint _S466 = b_8 + 1U;
    float4 _S467 = float4(g_7.x) ;
    uint _S468 = b_8 + 32U;
    uint _S469 = _S468 + 1U;
    float4 t0_0 = mix(float4(*(t_7 + b_8)) , float4(*(t_7 + _S466)) , _S467);
    float4 t1_0 = mix(float4(*(t_7 + _S468)) , float4(*(t_7 + _S469)) , _S467);
    float4 _S470 = float4(g_7.y) ;
    *lo_1 = mix(mix(float4(*(l_5 + b_8)) , float4(*(l_5 + _S466)) , _S467), mix(float4(*(l_5 + _S468)) , float4(*(l_5 + _S469)) , _S467), _S470).xyz;
    *to_0 = mix(t0_0, t1_0, _S470).xyz;
    return;
}

void atmos_aerial_0(FrameData_natural_0 device* f_30, float3 p_15, float2 px_3, float3 thread* l_6, float3 thread* tr_0, KernelContext_0 thread* kernelContext_42)
{
    *l_6 = float3(0.0f) ;
    float3 _S471 = float3(1.0f) ;
    *tr_0 = _S471;
    float t_8 = length(p_15) * 0.0010000000474974513f;
    if(t_8 <= 0.0f)
    {
        return;
    }
    float h_23 = (float4((&f_30->atmos_0)->radii_0) ).z;
    float _S472 = atm_rb_0(f_30);
    float _S473 = atm_rt_0(f_30);
    float te_0;
    if(h_23 > (_S473 - _S472))
    {
        float te_1 = atm_enter_top_0(_S472, _S473, h_23, dot(p_15 * float3((0.0010000000474974513f / t_8)) , (float4((&f_30->atmos_0)->up_0) ).xyz));
        bool _S474;
        if(te_1 < 0.0f)
        {
            _S474 = true;
        }
        else
        {
            _S474 = t_8 <= te_1;
        }
        if(_S474)
        {
            return;
        }
        te_0 = te_1;
    }
    else
    {
        te_0 = 0.0f;
    }
    float _S475 = atm_ap_slice_0(f_30, t_8 - te_0);
    float2 q_15 = clamp(px_3 / (float4(f_30->viewport_0) ).zw * float2(32.0f)  - float2(0.5f) , float2(0.0f) , float2(31.0f) );
    uint2 _S476 = min(uint2(q_15), uint2(30U) );
    float2 g_8 = q_15 - float2(_S476);
    packed_float4 device* _S477 = ed_buf_4((uint4((&f_30->atmos_0)->bufs2_0) ).x, kernelContext_42);
    packed_float4 device* _S478 = ed_buf_4((uint4((&f_30->atmos_0)->bufs2_0) ).y, kernelContext_42);
    thread float3 la_2;
    thread float3 ta_0;
    thread float3 lc_3;
    thread float3 tc_0;
    if(_S475 < 1.0f)
    {
        atm_froxel_xy_0(_S477, _S478, g_8, _S476, 0U, &lc_3, &tc_0);
        float3 _S479 = float3(_S475) ;
        *l_6 = lc_3 * _S479;
        *tr_0 = mix(_S471, tc_0, _S479);
        return;
    }
    float _S480 = min(_S475 - 1.0f, 31.0f);
    uint _S481 = min(uint(_S480), 30U);
    float gz_0 = _S480 - float(_S481);
    atm_froxel_xy_0(_S477, _S478, g_8, _S476, _S481, &la_2, &ta_0);
    atm_froxel_xy_0(_S477, _S478, g_8, _S476, _S481 + 1U, &lc_3, &tc_0);
    float3 _S482 = float3(gz_0) ;
    *l_6 = mix(la_2, lc_3, _S482);
    *tr_0 = mix(ta_0, tc_0, _S482);
    return;
}

float3 apply_aerial_0(FrameData_natural_0 device* f_31, float3 col_0, float3 p_16, float2 px_4, KernelContext_0 thread* kernelContext_43)
{
    bool _S483 = atmos_on_0(f_31);
    if(!_S483)
    {
        return col_0;
    }
    thread float3 l_7;
    thread float3 tr_1;
    atmos_aerial_0(f_31, p_16, px_4, &l_7, &tr_1, kernelContext_43);
    if(((uint4((&f_31->atmos_0)->cfg_1) ).y) == 1U)
    {
        return tr_1 * float3(0.5f) ;
    }
    if(((uint4((&f_31->atmos_0)->cfg_1) ).y) == 2U)
    {
        return l_7;
    }
    return col_0 * tr_1 + l_7;
}

uint shadow_cascade_0(FrameData_natural_0 device* f_32, float3 p_17, float dist_2)
{
    uint _S484 = uint((float4((&f_32->light_0)->cfg_0) ).x);
    bool _S485;
    if(((uint4((&f_32->light_0)->flags_0) ).x) == 0U)
    {
        _S485 = true;
    }
    else
    {
        _S485 = dist_2 >= ((float4((&f_32->light_0)->filter_0) ).y);
    }
    if(_S485)
    {
        return 4U;
    }
    float _S486 = 1.0f - (float4((&f_32->light_0)->cfg_0) ).w * 2.0f / (float4((&f_32->light_0)->cfg_0) ).y;
    uint c_17 = 0U;
    for(;;)
    {
        if(c_17 < _S484)
        {
        }
        else
        {
            break;
        }
        float _S487 = cascade_extent_0(f_32, c_17, p_17);
        if((_S487 / _S486) <= 1.0f)
        {
            return c_17;
        }
        c_17 = c_17 + 1U;
    }
    return 4U;
}

float4 terrain_pixel_0(FrameData_natural_0 device* f_33, const FsOut_0 thread* i_19, uint2 vis_1, float3 dir_8, KernelContext_0 thread* kernelContext_44)
{
    TerrainDraw_natural_0 device* _S488 = ed_buf_2(kernelContext_44->ed_c_0->t_draws_0, kernelContext_44);
    TerrainDraw_natural_0 device* _S489 = _S488 + ((vis_1.x) & 2147483647U);
    TerrainDraw_natural_0 d_28 = *_S489;
    thread TerrainDraw_natural_0 _S490 = *_S489;
    uint pool_18 = kernelContext_44->ed_c_0->t_pool_0;
    uint3 tri_0 = t_triangle_0((vis_1.y) & 268435455U);
    uint _S491 = tri_0.x;
    _S490 = d_28;
    float3 _S492 = t_vertex_0(pool_18, &_S490, _S491, kernelContext_44);
    uint _S493 = tri_0.y;
    float3 _S494 = t_vertex_0(pool_18, &_S490, _S493, kernelContext_44);
    float3 e1_1 = _S494 - _S492;
    uint _S495 = tri_0.z;
    float3 _S496 = t_vertex_0(pool_18, &_S490, _S495, kernelContext_44);
    float3 e2_1 = _S496 - _S492;
    float2 bb_0 = ray_bary_0(dir_8, _S492, e1_1, e2_1);
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
    float3 p_18 = _S492 + e1_1 * float3(b1_1)  + e2_1 * float3(b2_1) ;
    float2 g_9 = t_vertex_grid_0(_S491) * float2((1.0f - b1_1 - b2_1))  + t_vertex_grid_0(_S493) * float2(b1_1)  + t_vertex_grid_0(_S495) * float2(b2_1) ;
    float _S497 = t_sample_spacing_0(&_S490, g_9);
    float _S498 = max(_S497, 0.0099999997764825821f);
    int rn_0 = clamp(int(1.2000000476837158f / _S498 + 0.5f), int(1), int(4));
    thread float3 up_4;
    thread float h_24;
    float3 _S499 = t_smooth_normal_0(pool_18, &_S490, g_9, rn_0, &up_4, &h_24, kernelContext_44);
    int r_8 = clamp(int(24.0f / _S498 + 0.5f), int(1), int(6));
    float3 nm_1;
    if(r_8 == rn_0)
    {
        nm_1 = _S499;
    }
    else
    {
        float3 _S500 = t_smooth_normal_0(pool_18, &_S490, g_9, r_8, &up_4, &h_24, kernelContext_44);
        nm_1 = _S500;
    }
    float3 _S501 = terrain_albedo_0(h_24, dot(nm_1, up_4));
    uint _S502 = (uint4((&_S490)->slots_0) ).z;
    bool _S503 = (_S502 & 8388608U) != 0U;
    float4 _S504 = float4(0.0f) ;
    bool _S505;
    if(_S503)
    {
        _S505 = (as_type<uint>(((float4((&_S490)->tex_0) ).z))) != 0U;
    }
    else
    {
        _S505 = false;
    }
    float4 cv_2;
    if(_S505)
    {
        float4 _S506 = t_canopy_bilin_0(pool_18, (as_type<uint>(((float4((&_S490)->tex_0) ).z))), g_9 * float2(4.0f) , kernelContext_44);
        cv_2 = _S506;
    }
    else
    {
        cv_2 = _S504;
    }
    float rough_3;
    float mat_ao_0;
    float3 n_11;
    float3 albedo_5;
    if((kernelContext_44->ed_c_0->t_mat_0) != 0U)
    {
        TerrainMat_natural_0 device* _S507 = ed_buf_3(kernelContext_44->ed_c_0->t_draws_0, kernelContext_44);
        thread TerrainMat_natural_0 _S508 = *(_S507 + kernelContext_44->ed_c_0->t_mat_0);
        float4 _S509 = float4((&_S490)->shape_0) ;
        float2 w_12 = (float4((&_S490)->tex_0) ).xy + (g_9 - float2(32.0f) ) * float2((_S509.x * _S509.y)) ;
        float3 _S510 = (float4((&_S490)->fu_0) ).xyz;
        float3 tu_5 = normalize(_S510 - up_4 * float3(dot(_S510, up_4)) );
        float3 _S511 = (float4((&_S490)->fv_0) ).xyz;
        float3 tv_6 = normalize(_S511 - up_4 * float3(dot(_S511, up_4)) );
        float2 _S512 = i_19->pos_0.xy;
        float3 _S513 = view_dir_0(f_33, _S512 + float2(1.0f, 0.0f));
        float2 bx_0 = ray_bary_0(_S513, _S492, e1_1, e2_1);
        float3 _S514 = view_dir_0(f_33, _S512 + float2(0.0f, 1.0f));
        float2 by_0 = ray_bary_0(_S514, _S492, e1_1, e2_1);
        float3 dpx_0 = _S492 + e1_1 * float3(bx_0.x)  + e2_1 * float3(bx_0.y)  - p_18;
        float3 dpy_0 = _S492 + e1_1 * float3(by_0.x)  + e2_1 * float3(by_0.y)  - p_18;
        float2 dwx_1 = float2(dot(dpx_0, tu_5), dot(dpx_0, tv_6));
        float2 dwy_1 = float2(dot(dpy_0, tu_5), dot(dpy_0, tv_6));
        float2 sp_1 = g_9 * float2(4.0f) ;
        int rf_0 = clamp(int(2.0f / _S498 + 0.5f), int(1), int(4));
        float _S515 = float(clamp(int(8.0f / _S498 + 0.5f), int(1), int(16)));
        float2 _S516 = float2(_S515, 0.0f);
        float _S517 = t_page_bilin_0(pool_18, &_S490, sp_1 + _S516, kernelContext_44);
        float _S518 = t_page_bilin_0(pool_18, &_S490, sp_1 - _S516, kernelContext_44);
        float _S519 = _S517 + _S518;
        float2 _S520 = float2(0.0f, _S515);
        float _S521 = t_page_bilin_0(pool_18, &_S490, sp_1 + _S520, kernelContext_44);
        float _S522 = _S519 + _S521;
        float _S523 = t_page_bilin_0(pool_18, &_S490, sp_1 - _S520, kernelContext_44);
        float lap_0 = _S522 + _S523 - 4.0f * h_24;
        if((length_squared_0(p_18)) < 1.6e+05f)
        {
            float _S524 = float(rf_0);
            float2 _S525 = float2(_S524, 0.0f);
            float _S526 = t_page_bilin_0(pool_18, &_S490, sp_1 + _S525, kernelContext_44);
            float _S527 = t_page_bilin_0(pool_18, &_S490, sp_1 - _S525, kernelContext_44);
            float _S528 = _S526 + _S527;
            float2 _S529 = float2(0.0f, _S524);
            float _S530 = t_page_bilin_0(pool_18, &_S490, sp_1 + _S529, kernelContext_44);
            float _S531 = _S528 + _S530;
            float _S532 = t_page_bilin_0(pool_18, &_S490, sp_1 - _S529, kernelContext_44);
            rough_3 = _S531 + _S532 - 4.0f * h_24;
        }
        else
        {
            rough_3 = 0.0f;
        }
        float dl_0 = _S515 * _S498;
        float df_0 = float(rf_0) * _S498;
        TSurf_0 _S533 = terrain_material_0(f_33, &_S508, &_S490, _S499, nm_1, up_4, h_24, lap_0 / (dl_0 * dl_0), rough_3 / (df_0 * df_0), w_12, dwx_1, dwy_1, dot(dpx_0, up_4), dot(dpy_0, up_4), tu_5, tv_6, length(p_18), _S503, cv_2, _S498, kernelContext_44);
        n_11 = _S533.n_1;
        albedo_5 = _S533.albedo_1;
        rough_3 = _S533.rough_0;
        mat_ao_0 = _S533.ao_2;
    }
    else
    {
        n_11 = _S499;
        albedo_5 = _S501;
        rough_3 = 0.92000001668930054f;
        mat_ao_0 = 1.0f;
    }
    if(_S503)
    {
        _S505 = (cv_2.x) > 0.0040000001899898052f;
    }
    else
    {
        _S505 = false;
    }
    float sun_k_0;
    if(_S505)
    {
        float4 _S534 = float4((&_S490)->shape_0) ;
        float2 w_13 = (float4((&_S490)->tex_0) ).xy + (g_9 - float2(32.0f) ) * float2((_S534.x * _S534.y)) ;
        float3 _S535 = (float4((&_S490)->fu_0) ).xyz;
        float3 tu_6 = normalize(_S535 - up_4 * float3(dot(_S535, up_4)) );
        float3 _S536 = (float4((&_S490)->fv_0) ).xyz;
        float3 tv_7 = normalize(_S536 - up_4 * float3(dot(_S536, up_4)) );
        float3 _S537 = view_dir_0(f_33, i_19->pos_0.xy + float2(1.0f, 0.0f));
        float _S538 = length(p_18);
        float px_iso_1 = _S538 * length(_S537 - dir_8);
        float px_m_1 = px_iso_1 / sqrt(max(abs(dot(dir_8, n_11)), 0.029999999329447746f));
        if(((float4((&f_33->wind_1)->cfg_2) ).z) > 0.5f)
        {
            _S505 = ((float4((&_S490)->polar_0) ).w) > 0.0f;
        }
        else
        {
            _S505 = false;
        }
        if(_S505)
        {
            sun_k_0 = (float4((&_S490)->polar_0) ).w / max(_S538, 1.0f);
        }
        else
        {
            sun_k_0 = -1.0f;
        }
        TForest_0 _S539 = t_forest_0(pool_18, &_S490, cv_2, g_9, w_13, tu_6, tv_7, up_4, n_11, albedo_5, (float4(f_33->sun_dir_0) ).xyz, px_m_1, px_iso_1, _S498, sun_k_0, kernelContext_44);
        float _S540 = mix(rough_3, 0.80000001192092896f, _S539.crown_0);
        float mat_ao_1 = mat_ao_0 * _S539.ao_3;
        n_11 = _S539.n_3;
        sun_k_0 = _S539.sun_1;
        albedo_5 = _S539.albedo_2;
        rough_3 = _S540;
        mat_ao_0 = mat_ao_1;
    }
    else
    {
        sun_k_0 = 1.0f;
    }
    float3 v_5 = - dir_8;
    float _S541 = dot(n_11, v_5);
    if(_S541 < 0.0f)
    {
        n_11 = normalize(n_11 - float3((1.0099999904632568f * _S541))  * v_5);
    }
    float zlin_1 = - (((float4(p_18, 1.0f)) * (matrix<float,int(4),int(4)> (f_33->view_0.data_0[int(0)][int(0)], f_33->view_0.data_0[int(1)][int(0)], f_33->view_0.data_0[int(2)][int(0)], f_33->view_0.data_0[int(3)][int(0)], f_33->view_0.data_0[int(0)][int(1)], f_33->view_0.data_0[int(1)][int(1)], f_33->view_0.data_0[int(2)][int(1)], f_33->view_0.data_0[int(3)][int(1)], f_33->view_0.data_0[int(0)][int(2)], f_33->view_0.data_0[int(1)][int(2)], f_33->view_0.data_0[int(2)][int(2)], f_33->view_0.data_0[int(3)][int(2)], f_33->view_0.data_0[int(0)][int(3)], f_33->view_0.data_0[int(1)][int(3)], f_33->view_0.data_0[int(2)][int(3)], f_33->view_0.data_0[int(3)][int(3)])))).z;
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
    float2 _S542 = i_19->pos_0.xy;
    float _S543 = sun_shadow_0(f_33, kernelContext_44->ed_c_0->shadow_0, p_18, ng_3, zlin_1, _S542, kernelContext_44);
    float sun_vis_1;
    if(_S503)
    {
        sun_vis_1 = min(_S543, sun_k_0);
    }
    else
    {
        sun_vis_1 = _S543;
    }
    float2 _S544 = ao_upsample_0(f_33, kernelContext_44->ed_c_0->ao_0, _S542, zlin_1, kernelContext_44);
    float ao_6 = _S544.x * mat_ao_0;
    float sv_3 = _S544.y;
    float3 _S545 = sun_light_at_0(f_33, p_18, kernelContext_44);
    float3 _S546 = shade_pbr_0(f_33, _S545, albedo_5, 0.0f, rough_3, ao_6, sv_3, sun_vis_1, n_11, v_5, kernelContext_44);
    float3 _S547 = apply_aerial_0(f_33, _S546, p_18, _S542, kernelContext_44);
    float3 col_1;
    if(((uint4((&f_33->light_0)->flags_0) ).w) == 1U)
    {
        uint _S548 = shadow_cascade_0(f_33, p_18, zlin_1);
        col_1 = TINT_0[_S548] * float3((0.039999999105930328f + 0.5f * sun_vis_1)) ;
    }
    else
    {
        if(((uint4((&f_33->light_0)->flags_0) ).w) == 2U)
        {
            col_1 = float3((0.5f * ao_6)) ;
        }
        else
        {
            if(((uint4((&f_33->light_0)->flags_0) ).w) == 4U)
            {
                col_1 = float3((0.5f * sv_3)) ;
            }
            else
            {
                col_1 = _S547;
            }
        }
    }
    if(_S503)
    {
        _S505 = (_S502 & 16777216U) != 0U;
    }
    else
    {
        _S505 = false;
    }
    if(_S505)
    {
        _S505 = (cv_2.x) > 0.0f;
    }
    else
    {
        _S505 = false;
    }
    if(_S505)
    {
        col_1 = mix(col_1, float3(saturate(cv_2.y / 40.0f), cv_2.x, cv_2.z) * float3(0.60000002384185791f) , float3(0.75f) );
    }
    return float4(col_1, 1.0f);
}

float3 t_target_even_1(uint pool_19, const TerrainDraw_natural_0 thread* d_29, uint i_20, uint j_10, KernelContext_0 thread* kernelContext_45)
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
        own_1 = ((uint4(d_29->slots_0) ).y) == 4294967295U;
    }
    uint4 _S549 = uint4(d_29->slots_0) ;
    uint _S550 = _S549.z;
    uint qx_1;
    if((_S550 & 16U) != 0U)
    {
        qx_1 = 128U;
    }
    else
    {
        qx_1 = 0U;
    }
    uint qy_1;
    if((_S550 & 32U) != 0U)
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
        float _S551 = t_own_h_0(pool_19, d_29, i_20, j_10, kernelContext_45);
        h_25 = _S551;
    }
    else
    {
        float4 _S552 = float4(d_29->hq_0) ;
        float _S553 = t_page_0(pool_19, _S549.y, qx_1 + 2U * i_20, qy_1 + 2U * j_10, _S552.z, _S552.w, kernelContext_45);
        h_25 = _S553;
    }
    thread float3 dir_9;
    float3 _S554 = t_lattice_0(d_29, i_20, j_10, h_25, &dir_9);
    return _S554;
}

float3 t_base_1(uint pool_20, const TerrainDraw_natural_0 thread* d_30, uint i_21, uint j_11, KernelContext_0 thread* kernelContext_46)
{
    float _S555 = t_own_h_0(pool_20, d_30, i_21, j_11, kernelContext_46);
    thread float3 dir_10;
    float3 _S556 = t_lattice_0(d_30, i_21, j_11, _S555, &dir_10);
    return _S556;
}

float3 t_coarse_even_1(uint pool_21, const TerrainDraw_natural_0 thread* d_31, uint e_9, uint2 a_15, uint2 b_9, uint2 c_18, KernelContext_0 thread* kernelContext_47)
{
    float3 _S557 = t_base_1(pool_21, d_31, a_15.x, a_15.y, kernelContext_47);
    if((e_9 & 3U) == 0U)
    {
        return _S557;
    }
    float m_5 = t_morph_0((float4(d_31->origin_0) ).xyz + _S557, (float4(d_31->morph_0) ).zw);
    float3 _S558 = t_base_1(pool_21, d_31, b_9.x, b_9.y, kernelContext_47);
    float3 _S559 = t_base_1(pool_21, d_31, c_18.x, c_18.y, kernelContext_47);
    return mix(_S557, float3(0.5f)  * (_S558 + _S559), float3(m_5) );
}

float3 t_target_1(uint pool_22, const TerrainDraw_natural_0 thread* d_32, uint i_22, uint j_12, KernelContext_0 thread* kernelContext_48)
{
    bool ei_1 = (i_22 & 1U) == 0U;
    bool ej_1 = (j_12 & 1U) == 0U;
    bool _S560;
    if(ei_1)
    {
        _S560 = ej_1;
    }
    else
    {
        _S560 = false;
    }
    if(_S560)
    {
        float3 _S561 = t_target_even_1(pool_22, d_32, i_22, j_12, kernelContext_48);
        return _S561;
    }
    if(!ei_1)
    {
        _S560 = ej_1;
    }
    else
    {
        _S560 = false;
    }
    if(_S560)
    {
        float3 _S562 = t_target_even_1(pool_22, d_32, i_22 - 1U, j_12, kernelContext_48);
        float3 _S563 = t_target_even_1(pool_22, d_32, i_22 + 1U, j_12, kernelContext_48);
        return float3(0.5f)  * (_S562 + _S563);
    }
    if(ei_1)
    {
        _S560 = !ej_1;
    }
    else
    {
        _S560 = false;
    }
    if(_S560)
    {
        float3 _S564 = t_target_even_1(pool_22, d_32, i_22, j_12 - 1U, kernelContext_48);
        float3 _S565 = t_target_even_1(pool_22, d_32, i_22, j_12 + 1U, kernelContext_48);
        return float3(0.5f)  * (_S564 + _S565);
    }
    float3 _S566 = t_target_even_1(pool_22, d_32, i_22 - 1U, j_12 - 1U, kernelContext_48);
    float3 _S567 = t_target_even_1(pool_22, d_32, i_22 + 1U, j_12 + 1U, kernelContext_48);
    return float3(0.5f)  * (_S566 + _S567);
}

float3 t_coarse_1(uint pool_23, const TerrainDraw_natural_0 thread* d_33, uint edge_3, uint e_10, KernelContext_0 thread* kernelContext_49)
{
    if((e_10 & 1U) == 0U)
    {
        float3 _S568 = t_coarse_even_1(pool_23, d_33, e_10, t_edge_at_0(edge_3, e_10), t_edge_at_0(edge_3, e_10 - 2U), t_edge_at_0(edge_3, e_10 + 2U), kernelContext_49);
        return _S568;
    }
    uint _S569 = e_10 - 1U;
    uint2 _S570 = t_edge_at_0(edge_3, _S569);
    uint _S571 = e_10 + 1U;
    uint2 _S572 = t_edge_at_0(edge_3, _S571);
    float3 _S573 = t_coarse_even_1(pool_23, d_33, _S569, _S570, t_edge_at_0(edge_3, e_10 - 3U), _S572, kernelContext_49);
    float3 _S574 = t_coarse_even_1(pool_23, d_33, _S571, _S572, _S570, t_edge_at_0(edge_3, e_10 + 3U), kernelContext_49);
    return float3(0.5f)  * (_S573 + _S574);
}

float3 t_grid_vertex_1(uint pool_24, const TerrainDraw_natural_0 thread* d_34, uint i_23, uint j_13, KernelContext_0 thread* kernelContext_50)
{
    uint fl_2 = (uint4(d_34->slots_0) ).z;
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
        bool _S575;
        if((fl_2 & (1U << edge_4)) != 0U)
        {
            _S575 = E_1 <= 2U;
        }
        else
        {
            _S575 = false;
        }
        if(_S575)
        {
            float3 _S576 = t_coarse_1(pool_24, d_34, edge_4, e_11, kernelContext_50);
            return _S576;
        }
        if(E_1 > 1U)
        {
            uint lo_2 = e_11 & (~(E_1 - 1U));
            if(lo_2 == e_11)
            {
                float3 _S577 = t_base_1(pool_24, d_34, i_23, j_13, kernelContext_50);
                return _S577;
            }
            uint2 a_16 = t_edge_at_0(edge_4, lo_2);
            uint2 b_10 = t_edge_at_0(edge_4, lo_2 + E_1);
            float3 _S578 = t_base_1(pool_24, d_34, a_16.x, a_16.y, kernelContext_50);
            float3 _S579 = t_base_1(pool_24, d_34, b_10.x, b_10.y, kernelContext_50);
            return mix(_S578, _S579, float3((float(e_11 - lo_2) / float(E_1))) );
        }
    }
    float3 _S580 = t_base_1(pool_24, d_34, i_23, j_13, kernelContext_50);
    if((fl_2 & 4194304U) != 0U)
    {
        return _S580;
    }
    float m_6 = t_morph_0((float4(d_34->origin_0) ).xyz + _S580, (float4(d_34->morph_0) ).xy);
    if(m_6 <= 0.0f)
    {
        return _S580;
    }
    float3 _S581 = t_target_1(pool_24, d_34, i_23, j_13, kernelContext_50);
    return mix(_S580, _S581, float3(m_6) );
}

float3 t_vertex_1(uint pool_25, const TerrainDraw_natural_0 thread* d_35, uint v_6, KernelContext_0 thread* kernelContext_51)
{
    if(v_6 < 4225U)
    {
        float3 _S582 = (float4(d_35->origin_0) ).xyz;
        float3 _S583 = t_grid_vertex_1(pool_25, d_35, v_6 % 65U, v_6 / 65U, kernelContext_51);
        return _S582 + _S583;
    }
    uint2 ij_1 = t_ring_0(v_6 - 4225U);
    float3 _S584 = t_grid_vertex_1(pool_25, d_35, ij_1.x, ij_1.y, kernelContext_51);
    float depth_1;
    if((((uint4(d_35->slots_0) ).z) & 64U) != 0U)
    {
        depth_1 = (float4(d_35->origin_0) ).w;
    }
    else
    {
        depth_1 = 0.0f;
    }
    float3 _S585 = (float4(d_35->origin_0) ).xyz;
    float3 _S586 = _S585 + _S584;
    float4 _S587 = float4(d_35->dir_c_0) ;
    return _S586 - normalize(_S586 - (_S585 - _S587.xyz * float3(_S587.w) )) * float3(depth_1) ;
}

constant array<float3, int(5)> TINT_1 = { { float3(1.0f, 0.25f, 0.25f), float3(0.25f, 1.0f, 0.25f), float3(0.30000001192092896f, 0.44999998807907104f, 1.0f), float3(1.0f, 1.0f, 0.25f), float3(0.60000002384185791f, 0.60000002384185791f, 0.60000002384185791f) } };
float4 terrain_pixel_1(FrameData_natural_0 device* f_34, const FsOut_0 thread* i_24, uint2 vis_2, float3 dir_11, KernelContext_0 thread* kernelContext_52)
{
    TerrainDraw_natural_0 device* _S588 = ed_buf_2(kernelContext_52->ed_c_0->t_draws_0, kernelContext_52);
    TerrainDraw_natural_0 device* _S589 = _S588 + ((vis_2.x) & 2147483647U);
    TerrainDraw_natural_0 d_36 = *_S589;
    thread TerrainDraw_natural_0 _S590 = *_S589;
    uint pool_26 = kernelContext_52->ed_c_0->t_pool_0;
    uint3 tri_1 = t_triangle_0((vis_2.y) & 268435455U);
    uint _S591 = tri_1.x;
    _S590 = d_36;
    float3 _S592 = t_vertex_1(pool_26, &_S590, _S591, kernelContext_52);
    uint _S593 = tri_1.y;
    float3 _S594 = t_vertex_1(pool_26, &_S590, _S593, kernelContext_52);
    float3 e1_2 = _S594 - _S592;
    uint _S595 = tri_1.z;
    float3 _S596 = t_vertex_1(pool_26, &_S590, _S595, kernelContext_52);
    float3 e2_2 = _S596 - _S592;
    float2 bb_1 = ray_bary_0(dir_11, _S592, e1_2, e2_2);
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
    float3 p_19 = _S592 + e1_2 * float3(b1_3)  + e2_2 * float3(b2_4) ;
    float2 g_10 = t_vertex_grid_0(_S591) * float2((1.0f - b1_3 - b2_4))  + t_vertex_grid_0(_S593) * float2(b1_3)  + t_vertex_grid_0(_S595) * float2(b2_4) ;
    float _S597 = t_sample_spacing_0(&_S590, g_10);
    float _S598 = max(_S597, 0.0099999997764825821f);
    int rn_1 = clamp(int(1.2000000476837158f / _S598 + 0.5f), int(1), int(4));
    thread float3 up_5;
    thread float h_26;
    float3 _S599 = t_smooth_normal_0(pool_26, &_S590, g_10, rn_1, &up_5, &h_26, kernelContext_52);
    int r_9 = clamp(int(24.0f / _S598 + 0.5f), int(1), int(6));
    float3 nm_2;
    if(r_9 == rn_1)
    {
        nm_2 = _S599;
    }
    else
    {
        float3 _S600 = t_smooth_normal_0(pool_26, &_S590, g_10, r_9, &up_5, &h_26, kernelContext_52);
        nm_2 = _S600;
    }
    float3 _S601 = terrain_albedo_0(h_26, dot(nm_2, up_5));
    float4 _S602 = float4(0.0f) ;
    float rough_4;
    float mat_ao_2;
    float3 n_12;
    float3 albedo_6;
    if((kernelContext_52->ed_c_0->t_mat_0) != 0U)
    {
        TerrainMat_natural_0 device* _S603 = ed_buf_3(kernelContext_52->ed_c_0->t_draws_0, kernelContext_52);
        thread TerrainMat_natural_0 _S604 = *(_S603 + kernelContext_52->ed_c_0->t_mat_0);
        float4 _S605 = float4((&_S590)->shape_0) ;
        float2 w_14 = (float4((&_S590)->tex_0) ).xy + (g_10 - float2(32.0f) ) * float2((_S605.x * _S605.y)) ;
        float3 _S606 = (float4((&_S590)->fu_0) ).xyz;
        float3 tu_7 = normalize(_S606 - up_5 * float3(dot(_S606, up_5)) );
        float3 _S607 = (float4((&_S590)->fv_0) ).xyz;
        float3 tv_8 = normalize(_S607 - up_5 * float3(dot(_S607, up_5)) );
        float2 _S608 = i_24->pos_0.xy;
        float3 _S609 = view_dir_0(f_34, _S608 + float2(1.0f, 0.0f));
        float2 bx_1 = ray_bary_0(_S609, _S592, e1_2, e2_2);
        float3 _S610 = view_dir_0(f_34, _S608 + float2(0.0f, 1.0f));
        float2 by_1 = ray_bary_0(_S610, _S592, e1_2, e2_2);
        float3 dpx_1 = _S592 + e1_2 * float3(bx_1.x)  + e2_2 * float3(bx_1.y)  - p_19;
        float3 dpy_1 = _S592 + e1_2 * float3(by_1.x)  + e2_2 * float3(by_1.y)  - p_19;
        float2 dwx_2 = float2(dot(dpx_1, tu_7), dot(dpx_1, tv_8));
        float2 dwy_2 = float2(dot(dpy_1, tu_7), dot(dpy_1, tv_8));
        float2 sp_2 = g_10 * float2(4.0f) ;
        int rf_1 = clamp(int(2.0f / _S598 + 0.5f), int(1), int(4));
        float _S611 = float(clamp(int(8.0f / _S598 + 0.5f), int(1), int(16)));
        float2 _S612 = float2(_S611, 0.0f);
        float _S613 = t_page_bilin_0(pool_26, &_S590, sp_2 + _S612, kernelContext_52);
        float _S614 = t_page_bilin_0(pool_26, &_S590, sp_2 - _S612, kernelContext_52);
        float _S615 = _S613 + _S614;
        float2 _S616 = float2(0.0f, _S611);
        float _S617 = t_page_bilin_0(pool_26, &_S590, sp_2 + _S616, kernelContext_52);
        float _S618 = _S615 + _S617;
        float _S619 = t_page_bilin_0(pool_26, &_S590, sp_2 - _S616, kernelContext_52);
        float lap_1 = _S618 + _S619 - 4.0f * h_26;
        if((length_squared_0(p_19)) < 1.6e+05f)
        {
            float _S620 = float(rf_1);
            float2 _S621 = float2(_S620, 0.0f);
            float _S622 = t_page_bilin_0(pool_26, &_S590, sp_2 + _S621, kernelContext_52);
            float _S623 = t_page_bilin_0(pool_26, &_S590, sp_2 - _S621, kernelContext_52);
            float _S624 = _S622 + _S623;
            float2 _S625 = float2(0.0f, _S620);
            float _S626 = t_page_bilin_0(pool_26, &_S590, sp_2 + _S625, kernelContext_52);
            float _S627 = _S624 + _S626;
            float _S628 = t_page_bilin_0(pool_26, &_S590, sp_2 - _S625, kernelContext_52);
            rough_4 = _S627 + _S628 - 4.0f * h_26;
        }
        else
        {
            rough_4 = 0.0f;
        }
        float dl_1 = _S611 * _S598;
        float df_1 = float(rf_1) * _S598;
        TSurf_0 _S629 = terrain_material_0(f_34, &_S604, &_S590, _S599, nm_2, up_5, h_26, lap_1 / (dl_1 * dl_1), rough_4 / (df_1 * df_1), w_14, dwx_2, dwy_2, dot(dpx_1, up_5), dot(dpy_1, up_5), tu_7, tv_8, length(p_19), false, _S602, _S598, kernelContext_52);
        n_12 = _S629.n_1;
        albedo_6 = _S629.albedo_1;
        rough_4 = _S629.rough_0;
        mat_ao_2 = _S629.ao_2;
    }
    else
    {
        n_12 = _S599;
        albedo_6 = _S601;
        rough_4 = 0.92000001668930054f;
        mat_ao_2 = 1.0f;
    }
    float3 v_7 = - dir_11;
    float _S630 = dot(n_12, v_7);
    if(_S630 < 0.0f)
    {
        n_12 = normalize(n_12 - float3((1.0099999904632568f * _S630))  * v_7);
    }
    float zlin_2 = - (((float4(p_19, 1.0f)) * (matrix<float,int(4),int(4)> (f_34->view_0.data_0[int(0)][int(0)], f_34->view_0.data_0[int(1)][int(0)], f_34->view_0.data_0[int(2)][int(0)], f_34->view_0.data_0[int(3)][int(0)], f_34->view_0.data_0[int(0)][int(1)], f_34->view_0.data_0[int(1)][int(1)], f_34->view_0.data_0[int(2)][int(1)], f_34->view_0.data_0[int(3)][int(1)], f_34->view_0.data_0[int(0)][int(2)], f_34->view_0.data_0[int(1)][int(2)], f_34->view_0.data_0[int(2)][int(2)], f_34->view_0.data_0[int(3)][int(2)], f_34->view_0.data_0[int(0)][int(3)], f_34->view_0.data_0[int(1)][int(3)], f_34->view_0.data_0[int(2)][int(3)], f_34->view_0.data_0[int(3)][int(3)])))).z;
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
    float2 _S631 = i_24->pos_0.xy;
    float _S632 = sun_shadow_0(f_34, kernelContext_52->ed_c_0->shadow_0, p_19, ng_5, zlin_2, _S631, kernelContext_52);
    float2 _S633 = ao_upsample_0(f_34, kernelContext_52->ed_c_0->ao_0, _S631, zlin_2, kernelContext_52);
    float ao_7 = _S633.x * mat_ao_2;
    float sv_4 = _S633.y;
    float3 _S634 = sun_light_at_0(f_34, p_19, kernelContext_52);
    float3 _S635 = shade_pbr_0(f_34, _S634, albedo_6, 0.0f, rough_4, ao_7, sv_4, _S632, n_12, v_7, kernelContext_52);
    float3 _S636 = apply_aerial_0(f_34, _S635, p_19, _S631, kernelContext_52);
    float3 col_2;
    if(((uint4((&f_34->light_0)->flags_0) ).w) == 1U)
    {
        uint _S637 = shadow_cascade_0(f_34, p_19, zlin_2);
        col_2 = TINT_1[_S637] * float3((0.039999999105930328f + 0.5f * _S632)) ;
    }
    else
    {
        if(((uint4((&f_34->light_0)->flags_0) ).w) == 2U)
        {
            col_2 = float3((0.5f * ao_7)) ;
        }
        else
        {
            if(((uint4((&f_34->light_0)->flags_0) ).w) == 4U)
            {
                col_2 = float3((0.5f * sv_4)) ;
            }
            else
            {
                col_2 = _S636;
            }
        }
    }
    return float4(col_2, 1.0f);
}

constant array<float3, int(5)> TINT_2 = { { float3(1.0f, 0.25f, 0.25f), float3(0.25f, 1.0f, 0.25f), float3(0.30000001192092896f, 0.44999998807907104f, 1.0f), float3(1.0f, 1.0f, 0.25f), float3(0.60000002384185791f, 0.60000002384185791f, 0.60000002384185791f) } };
float3 sky_rad_1(FrameData_natural_0 device* f_35, float3 d_37, float disc_k_1)
{
    float3 sun_4 = (float4(f_35->sun_dir_0) ).xyz;
    float h_27 = d_37.y;
    float t_9 = pow(saturate(1.0f - max(h_27, 0.0f)), 6.5f);
    float mu_7 = dot(d_37, sun_4);
    float warm_1 = saturate(1.0f - sun_4.y * 2.5f);
    float3 c_19 = mix((float4(f_35->sky_zenith_0) ).xyz, (float4(f_35->sky_horizon_0) ).xyz, float3(t_9) ) + float3(1.0f, 0.41999998688697815f, 0.14000000059604645f) * float3(pow(saturate(mu_7 * 0.5f + 0.5f), 5.0f))  * float3(warm_1)  * float3(0.85000002384185791f)  * float3((0.40000000596046448f + t_9)) ;
    float3 _S638 = float3((float4(f_35->sun_dir_0) ).w) ;
    return mix(mix(c_19, c_19 * float3(1.0f, 0.69999998807907104f, 0.55000001192092896f), float3((warm_1 * 0.40000000596046448f)) ) + float3(1.0f, 0.89999997615814209f, 0.69999998807907104f) * float3(pow(saturate(mu_7), 28.0f))  * float3(0.34999999403953552f) , (float4(f_35->ground_color_0) ).xyz, float3(saturate(- h_27 * 6.0f)) ) * _S638 + float3(1.0f, 0.94999998807907104f, 0.85000002384185791f) * float3((smoothstep(0.99965000152587891f, 0.99984997510910034f, mu_7) * step(0.0f, h_27)))  * float3(60.0f)  * _S638 * float3(disc_k_1) ;
}

float atm_rt_1(FrameData_natural_0 device* f_36)
{
    return (float4((&f_36->atmos_0)->radii_0) ).y;
}

bool atm_enter_0(FrameData_natural_0 device* f_37, float thread* h_28, float thread* mu_8, float thread* mu_s_3, float nu_1, float thread* te_2)
{
    *te_2 = 0.0f;
    float _S639 = atm_rb_1(f_37);
    float _S640 = atm_rt_1(f_37);
    float _S641 = _S640 - _S639;
    if((*h_28) <= _S641)
    {
        return true;
    }
    float _S642 = atm_enter_top_0(_S639, _S640, *h_28, *mu_8);
    *te_2 = _S642;
    if(_S642 < 0.0f)
    {
        return false;
    }
    float r_10 = _S639 + *h_28;
    *mu_8 = clamp((r_10 * *mu_8 + *te_2) / _S640, -1.0f, 1.0f);
    *mu_s_3 = clamp((r_10 * *mu_s_3 + *te_2 * nu_1) / _S640, -1.0f, 1.0f);
    *h_28 = _S641;
    return true;
}

bool atm_hits_ground_0(float rb_5, float h_29, float mu_9)
{
    float rm_0 = (rb_5 + h_29) * mu_9;
    bool _S643;
    if(mu_9 < 0.0f)
    {
        _S643 = (rm_0 * rm_0) >= (h_29 * (2.0f * rb_5 + h_29));
    }
    else
    {
        _S643 = false;
    }
    return _S643;
}

float3 atm_transmittance_1(FrameData_natural_0 device* f_38, float h_30, float mu_10, KernelContext_0 thread* kernelContext_53)
{
    float _S644 = atm_rb_0(f_38);
    float _S645 = atm_rt_0(f_38);
    float2 uv_3 = atm_trans_uv_0(_S644, _S645, h_30, mu_10);
    packed_float4 device* _S646 = ed_buf_4((uint4((&f_38->atmos_0)->bufs_0) ).x, kernelContext_53);
    float4 _S647 = atm_bilinear_0(_S646, 256U, 0U, 64U, uv_3);
    return _S647.xyz;
}

float3 atmos_sky_1(FrameData_natural_0 device* f_39, float3 d_38, KernelContext_0 thread* kernelContext_54)
{
    float3 _S648 = atm_sky_view_0(f_39, (uint4((&f_39->atmos_0)->bufs_0) ).z, 192U, 108U, (float4((&f_39->atmos_0)->radii_0) ).z, d_38, kernelContext_54);
    return _S648;
}

float3 atmos_sky_pixel_0(FrameData_natural_0 device* f_40, float3 d_39, float disc_k_2, KernelContext_0 thread* kernelContext_55)
{
    float3 _S649 = atmos_sky_1(f_40, d_39, kernelContext_55);
    float nu_2 = dot(d_39, (float4(f_40->sun_dir_0) ).xyz);
    float cr_0 = (float4((&f_40->atmos_0)->sun_0) ).w;
    float _S650 = cr_0 - 1.99999999495048542e-06f;
    float3 c_20;
    if(nu_2 > _S650)
    {
        thread float h_31 = (float4((&f_40->atmos_0)->radii_0) ).z;
        thread float mu_11 = dot(d_39, (float4((&f_40->atmos_0)->up_0) ).xyz);
        thread float mu_s_4 = (float4((&f_40->atmos_0)->up_0) ).w;
        float3 _S651 = float3(1.0f) ;
        thread float te_3;
        bool _S652 = atm_enter_0(f_40, &h_31, &mu_11, &mu_s_4, nu_2, &te_3);
        if(_S652)
        {
            float _S653 = atm_rb_0(f_40);
            if(atm_hits_ground_0(_S653, h_31, mu_11))
            {
                c_20 = float3(0.0f) ;
            }
            else
            {
                float3 _S654 = atm_transmittance_1(f_40, h_31, mu_11, kernelContext_55);
                c_20 = _S654;
            }
        }
        else
        {
            c_20 = _S651;
        }
        float sr2_0 = 1.0f - cr_0 * cr_0;
        c_20 = _S649 + min((float4((&f_40->atmos_0)->sun_0) ).xyz * c_20 / float3((3.1415927410125732f * sr2_0))  * float3((1.0f - 0.60000002384185791f * (1.0f - sqrt(1.0f - saturate((1.0f - nu_2 * nu_2) / sr2_0)))))  * float3(smoothstep(_S650, cr_0 + 1.99999999495048542e-06f, nu_2))  * float3(disc_k_2) , float3(30000.0f) );
    }
    else
    {
        c_20 = _S649;
    }
    return c_20;
}

float3 sky_pixel_0(FrameData_natural_0 device* f_41, float3 d_40, KernelContext_0 thread* kernelContext_56)
{
    bool _S655 = atmos_on_0(f_41);
    if(!_S655)
    {
        float3 _S656 = sky_rad_1(f_41, d_40, (float4((&f_41->light_0)->sky_0) ).x);
        return _S656;
    }
    if(((uint4((&f_41->atmos_0)->cfg_1) ).y) == 1U)
    {
        thread float h_32 = (float4((&f_41->atmos_0)->radii_0) ).z;
        thread float mu_12 = dot(d_40, (float4((&f_41->atmos_0)->up_0) ).xyz);
        thread float mu_s_5 = (float4((&f_41->atmos_0)->up_0) ).w;
        thread float te_4;
        bool _S657 = atm_enter_0(f_41, &h_32, &mu_12, &mu_s_5, dot(d_40, (float4(f_41->sun_dir_0) ).xyz), &te_4);
        if(!_S657)
        {
            return float3(0.5f) ;
        }
        float _S658 = atm_rb_0(f_41);
        float3 _S659;
        if(atm_hits_ground_0(_S658, h_32, mu_12))
        {
            _S659 = float3(0.0f) ;
        }
        else
        {
            float3 _S660 = atm_transmittance_1(f_41, h_32, mu_12, kernelContext_56);
            _S659 = _S660 * float3(0.5f) ;
        }
        return _S659;
    }
    float _S661;
    if(((uint4((&f_41->atmos_0)->cfg_1) ).y) == 2U)
    {
        _S661 = 0.0f;
    }
    else
    {
        _S661 = (float4((&f_41->light_0)->sky_0) ).x;
    }
    float3 _S662 = atmos_sky_pixel_0(f_41, d_40, _S661, kernelContext_56);
    return _S662;
}

struct GpuTree_0
{
    float px_5;
    float py_0;
    float pz_0;
    uint node_kind_0;
    float scale_1;
    uint up_6;
    uint yaw_widen_0;
    uint user_0;
};

GpuTree_0 device* ed_buf_5(uint i_25, KernelContext_0 thread* kernelContext_57)
{
    return (GpuTree_0 device*)kernelContext_57->ed_buffers_0[i_25];
}

struct GpuImpKind_natural_0
{
    packed_uint4 tex_1;
    packed_float4 geo_0;
    packed_float4 leaf_1;
    packed_float4 bark_0;
};

GpuImpKind_natural_0 device* ed_buf_6(uint i_26, KernelContext_0 thread* kernelContext_58)
{
    return (GpuImpKind_natural_0 device*)kernelContext_58->ed_buffers_0[i_26];
}

struct ImpTree_0
{
    float3 c_21;
    float3 ex_0;
    float3 up_7;
    float3 ez_0;
    float s_14;
    float wd_0;
    float r_11;
    float3 cam_0;
    uint3 fi_0;
    float3 w_15;
    float3 d0_0;
    float3 d1_0;
    float3 d2_0;
};

float2 unpack_snorm2x16_0(uint p_20)
{
    return clamp(float2(int2(int(p_20 << 16U) >> 16U, int(p_20) >> 16U)) / float2(32767.0f) , float2(-1.0f) , float2(1.0f) );
}

float3 oct_decode_0(uint packed_0)
{
    float2 e_12 = unpack_snorm2x16_0(packed_0);
    float _S663 = e_12.x;
    float _S664 = e_12.y;
    float _S665 = 1.0f - abs(_S663) - abs(_S664);
    thread float3 n_13 = float3(_S663, _S664, _S665);
    float t_10 = saturate(- _S665);
    float _S666;
    if(_S663 >= 0.0f)
    {
        _S666 = - t_10;
    }
    else
    {
        _S666 = t_10;
    }
    n_13.x = n_13.x + _S666;
    if((n_13.y) >= 0.0f)
    {
        _S666 = - t_10;
    }
    else
    {
        _S666 = t_10;
    }
    n_13.y = n_13.y + _S666;
    return normalize(n_13);
}

float3 arc_from_y_0(float3 up_8, float3 v_8)
{
    float3 a_17 = float3(up_8.z, 0.0f, - up_8.x);
    float w_16 = 1.0f + up_8.y;
    float l_8 = sqrt(dot(a_17, a_17) + w_16 * w_16);
    if(l_8 < 9.99999997475242708e-07f)
    {
        return float3(v_8.x, - v_8.y, - v_8.z);
    }
    float3 a_18 = a_17 / float3(l_8) ;
    float3 t_11 = float3(2.0f)  * cross(a_18, v_8);
    return v_8 + float3((w_16 / l_8))  * t_11 + cross(a_18, t_11);
}

float3 imp_to_tree_0(const ImpTree_0 thread* t_12, float3 v_9)
{
    float _S667 = t_12->s_14 * t_12->wd_0;
    return float3(dot(v_9, t_12->ex_0) / _S667, dot(v_9, t_12->up_7) / t_12->s_14, dot(v_9, t_12->ez_0) / _S667);
}

float2 imp_oct_encode_0(float3 d_41)
{
    thread float3 _S668 = d_41;
    _S668.y = max(d_41.y, 0.0f);
    float3 _S669 = _S668 / float3(max(abs(_S668.x) + abs(_S668.y) + abs(_S668.z), 9.99999971718068537e-10f)) ;
    _S668 = _S669;
    float _S670 = _S669.x;
    float _S671 = _S669.z;
    float2 _S672 = float2(0.5f) ;
    return float2(_S670 + _S671, _S670 - _S671) * _S672 + _S672;
}

float3 imp_oct_decode_0(float2 uv_4)
{
    float2 g_11 = uv_4 * float2(2.0f)  - float2(1.0f) ;
    float _S673 = g_11.x;
    float _S674 = g_11.y;
    float x_3 = (_S673 + _S674) * 0.5f;
    float z_0 = (_S673 - _S674) * 0.5f;
    return normalize(float3(x_3, 1.0f - abs(x_3) - abs(z_0), z_0));
}

ImpTree_0 imp_tree_0(const GpuTree_0 thread* g_12, const GpuImpKind_natural_0 thread* k_10, float3 node_0, float3 sun_dir_1, bool sun_5)
{
    thread ImpTree_0 t_13;
    float3 _S675 = oct_decode_0(g_12->up_6);
    (&t_13)->up_7 = _S675;
    float yaw_0 = float((g_12->yaw_widen_0) & 65535U) / 65535.0f * 6.2831854820251465f;
    (&t_13)->wd_0 = 0.5f + float((g_12->yaw_widen_0) >> 16U) / 65535.0f;
    (&t_13)->s_14 = g_12->scale_1;
    float _S676 = cos(yaw_0);
    float _S677 = sin(yaw_0);
    (&t_13)->ex_0 = arc_from_y_0(_S675, float3(_S676, 0.0f, - _S677));
    (&t_13)->ez_0 = arc_from_y_0(_S675, float3(_S677, 0.0f, _S676));
    float4 _S678 = float4(k_10->geo_0) ;
    (&t_13)->r_11 = _S678.y;
    (&t_13)->c_21 = node_0 + float3(g_12->px_5, g_12->py_0, g_12->pz_0) + _S675 * float3((_S678.x * g_12->scale_1)) ;
    ImpTree_0 _S679 = t_13;
    float3 _S680;
    if(sun_5)
    {
        _S680 = sun_dir_1 * float3(1.0e+05f) ;
    }
    else
    {
        _S680 = - (&t_13)->c_21;
    }
    thread ImpTree_0 _S681 = _S679;
    float3 _S682 = imp_to_tree_0(&_S681, _S680);
    (&t_13)->cam_0 = _S682;
    uint _S683 = (uint4(k_10->tex_1) ).w;
    float n_14 = float(_S683);
    float2 _S684 = float2(n_14) ;
    float2 _S685 = float2(0.5f) ;
    float _S686 = n_14 - 1.0f;
    float2 gpos_0 = clamp(imp_oct_encode_0(normalize(_S682)) * _S684 - _S685, float2(0.0f, 0.0f), float2(_S686, _S686));
    float _S687 = n_14 - 2.0f;
    float2 _S688 = min(floor(gpos_0), float2(_S687, _S687));
    float2 fr_0 = gpos_0 - _S688;
    uint2 _S689 = uint2(_S688);
    uint2 c1_0 = _S689 + uint2(1U, 0U);
    uint2 c2_0 = _S689 + uint2(0U, 1U);
    float _S690 = fr_0.x;
    float _S691 = fr_0.y;
    float _S692 = _S690 + _S691;
    uint2 c0_1;
    if(_S692 < 1.0f)
    {
        (&t_13)->w_15 = float3(1.0f - _S690 - _S691, _S690, _S691);
        c0_1 = _S689;
    }
    else
    {
        uint2 _S693 = _S689 + uint2(1U, 1U);
        (&t_13)->w_15 = float3(_S692 - 1.0f, 1.0f - _S691, 1.0f - _S690);
        c0_1 = _S693;
    }
    (&t_13)->fi_0 = uint3(c0_1.y * _S683 + c0_1.x, c1_0.y * _S683 + c1_0.x, c2_0.y * _S683 + c2_0.x);
    (&t_13)->d0_0 = imp_oct_decode_0((float2(c0_1) + _S685) / _S684);
    (&t_13)->d1_0 = imp_oct_decode_0((float2(c1_0) + _S685) / _S684);
    (&t_13)->d2_0 = imp_oct_decode_0((float2(c2_0) + _S685) / _S684);
    return t_13;
}

void imp_view_basis_0(float3 v_10, float3 thread* right_0, float3 thread* up_9)
{
    float3 u0_0 = float3(0.0f, 1.0f, 0.0f) - v_10 * float3(v_10.y) ;
    float3 _S694;
    if((dot(u0_0, u0_0)) > 9.99999997475242708e-07f)
    {
        _S694 = normalize(u0_0);
    }
    else
    {
        _S694 = float3(0.0f, 0.0f, 1.0f);
    }
    *up_9 = _S694;
    *right_0 = normalize(cross(_S694, v_10));
    return;
}

float2 imp_frame_uv_0(const ImpTree_0 thread* t_14, float3 d_42, float3 q_16, float half_size_0)
{
    float3 _S695 = t_14->cam_0;
    float3 dir_12 = q_16 - t_14->cam_0;
    float den_0 = dot(dir_12, d_42);
    float k_11;
    if((abs(den_0)) > 9.99999997475242708e-07f)
    {
        k_11 = - dot(_S695, d_42) / den_0;
    }
    else
    {
        k_11 = 1.0f;
    }
    float3 h_33 = _S695 + dir_12 * float3(k_11) ;
    thread float3 r_12;
    thread float3 u_1;
    imp_view_basis_0(d_42, &r_12, &u_1);
    float _S696 = 2.0f * half_size_0;
    return float2(0.5f + dot(h_33, r_12) / _S696, 0.5f - dot(h_33, u_1) / _S696);
}

float2 imp_atlas_uv_0(uint fi_1, uint n_15, float2 uv_5, float frame_px_0)
{
    float m_7 = 0.5f / frame_px_0;
    float _S697 = 1.0f - m_7;
    float2 l_9 = clamp(uv_5, float2(m_7, m_7), float2(_S697, _S697));
    uint _S698 = fi_1 % n_15;
    float _S699 = float(_S698);
    uint _S700 = fi_1 / n_15;
    return (float2(_S699, float(_S700)) + l_9) / float2(float(n_15)) ;
}

bool imp_inside_0(float2 uv_6)
{
    bool _S701;
    if(all(uv_6 >= (float2(0.0f) )))
    {
        _S701 = all(uv_6 <= (float2(1.0f) ));
    }
    else
    {
        _S701 = false;
    }
    return _S701;
}

float3 imp_oct_sphere_0(float2 e_13)
{
    float2 f_42 = e_13 * float2(2.0f)  - float2(1.0f) ;
    float _S702 = f_42.x;
    float _S703 = f_42.y;
    float _S704 = 1.0f - abs(_S702) - abs(_S703);
    thread float3 n_16 = float3(_S702, _S704, _S703);
    float t_15 = saturate(- _S704);
    float _S705;
    if(_S702 >= 0.0f)
    {
        _S705 = - t_15;
    }
    else
    {
        _S705 = t_15;
    }
    n_16.x = n_16.x + _S705;
    if((n_16.z) >= 0.0f)
    {
        _S705 = - t_15;
    }
    else
    {
        _S705 = t_15;
    }
    n_16.z = n_16.z + _S705;
    return normalize(n_16);
}

float3 tree_hue_0(float h_34)
{
    float3 _S706;
    if(h_34 >= 0.0f)
    {
        _S706 = mix(float3(1.0f, 1.0f, 1.0f), float3(0.85000002384185791f, 0.97000002861022949f, 1.2000000476837158f), float3(h_34) );
    }
    else
    {
        _S706 = mix(float3(1.0f, 1.0f, 1.0f), float3(1.1200000047683716f, 1.0f, 0.75f), float3(- h_34) );
    }
    return _S706;
}

float3 shade_foliage_0(FrameData_natural_0 device* f_43, float3 sun_6, float3 albedo_7, float rough_5, float ao_8, float sv_5, float sun_vis_2, float3 n_17, float3 ng_6, float3 v_11, bool leaf_2, KernelContext_0 thread* kernelContext_59)
{
    float3 _S707 = shade_pbr_0(f_43, sun_6, albedo_7, 0.0f, rough_5, ao_8, sv_5, sun_vis_2, n_17, v_11, kernelContext_59);
    float3 col_3;
    if(leaf_2)
    {
        float3 l_10 = (float4(f_43->sun_dir_0) ).xyz;
        col_3 = _S707 + sun_6 * float3(sun_vis_2)  * (albedo_7 * float3(1.1499999761581421f, 1.25f, 0.55000001192092896f)) * float3((saturate(- dot(n_17, l_10)) * (0.44999998807907104f + 1.2000000476837158f * pow(saturate(dot(- v_11, l_10)), 4.0f))))  * float3(0.31830987334251404f) ;
    }
    else
    {
        col_3 = _S707;
    }
    return col_3;
}

float4 impostor_pixel_0(FrameData_natural_0 device* f_44, const FsOut_0 thread* i_27, uint idx_0, float3 dir_13, KernelContext_0 thread* kernelContext_60)
{
    GpuTree_0 device* _S708 = ed_buf_5((uint4((&f_44->imp_0)->bufs_1) ).x, kernelContext_60);
    GpuTree_0 g_13 = *(_S708 + idx_0);
    GpuImpKind_natural_0 device* _S709 = ed_buf_6((uint4((&f_44->imp_0)->bufs_1) ).y, kernelContext_60);
    GpuImpKind_natural_0 device* _S710 = _S709 + (((g_13.node_kind_0) >> 12U) & 4095U);
    GpuImpKind_natural_0 k_12 = *_S710;
    thread GpuImpKind_natural_0 _S711 = *_S710;
    packed_float4 device* _S712 = ed_buf_4(kernelContext_60->ed_c_0->nodes_0, kernelContext_60);
    float3 node_1 = (float4(*(_S712 + ((g_13.node_kind_0) & 4095U))) ).xyz;
    float3 _S713 = (float4(f_44->sun_dir_0) ).xyz;
    thread GpuTree_0 _S714 = g_13;
    _S711 = k_12;
    ImpTree_0 _S715 = imp_tree_0(&_S714, &_S711, node_1, _S713, false);
    float3 nrm_0 = normalize(_S715.c_21);
    float hit_0 = dot(_S715.c_21, nrm_0) / max(dot(dir_13, nrm_0), 9.9999997473787516e-05f);
    float3 pq_0 = dir_13 * float3(hit_0) ;
    float3 _S716 = pq_0 - _S715.c_21;
    thread ImpTree_0 _S717 = _S715;
    float3 _S718 = imp_to_tree_0(&_S717, _S716);
    float half_t_0 = _S715.r_11 * (float4((&f_44->imp_0)->cfg_3) ).y;
    thread ImpTree_0 _S719 = _S715;
    float2 _S720 = imp_frame_uv_0(&_S719, _S715.d0_0, _S718, half_t_0);
    thread ImpTree_0 _S721 = _S715;
    float2 _S722 = imp_frame_uv_0(&_S721, _S715.d1_0, _S718, half_t_0);
    thread ImpTree_0 _S723 = _S715;
    float2 _S724 = imp_frame_uv_0(&_S723, _S715.d2_0, _S718, half_t_0);
    uint4 _S725 = uint4((&_S711)->tex_1) ;
    uint n_18 = _S725.w;
    float fpx_0 = (float4((&_S711)->geo_0) ).w;
    float4 _S726 = float4((&_S711)->leaf_1) ;
    float lod_1 = clamp(log2(max(hit_0 / ((float4(f_44->proj_0) ).w * 0.5f * (float4(f_44->proj_0) ).y), 9.99999997475242708e-07f) / (2.0f * half_t_0 * _S715.s_14 * max(_S715.wd_0, 1.0f) / fpx_0)), 0.0f, _S726.w - 1.0f);
    sampler _S727 = ((reinterpret_cast<device sampler*>((kernelContext_60->ed_samplers_0))[(kernelContext_60->ed_c_0->smp_0)]));
    texture2d<float, access::sample> _S728 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_60->ed_textures_0))[(_S725.x)]));
    texture2d<float, access::sample> _S729 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_60->ed_textures_0))[(_S725.y)]));
    texture2d<float, access::sample> _S730 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_60->ed_textures_0))[(_S725.z)]));
    float2 a0_0 = imp_atlas_uv_0(_S715.fi_0.x, n_18, _S720, fpx_0);
    float2 a1_0 = imp_atlas_uv_0(_S715.fi_0.y, n_18, _S722, fpx_0);
    float2 a2_2 = imp_atlas_uv_0(_S715.fi_0.z, n_18, _S724, fpx_0);
    float2 s0_0 = ((_S728).sample((_S727), (a0_0), level((lod_1)))).xy;
    float2 s1_1 = ((_S728).sample((_S727), (a1_0), level((lod_1)))).xy;
    float2 s2_0 = ((_S728).sample((_S727), (a2_2), level((lod_1)))).xy;
    float _S731 = s0_0.x;
    float _S732;
    if(imp_inside_0(_S720))
    {
        _S732 = 1.0f;
    }
    else
    {
        _S732 = 0.0f;
    }
    float _S733 = _S731 * _S732;
    float _S734 = s1_1.x;
    if(imp_inside_0(_S722))
    {
        _S732 = 1.0f;
    }
    else
    {
        _S732 = 0.0f;
    }
    float _S735 = _S734 * _S732;
    float _S736 = s2_0.x;
    if(imp_inside_0(_S724))
    {
        _S732 = 1.0f;
    }
    else
    {
        _S732 = 0.0f;
    }
    float3 wc_0 = _S715.w_15 * float3(_S733, _S735, _S736 * _S732);
    float ws_0 = wc_0.x + wc_0.y + wc_0.z;
    float3 wc_1;
    if(ws_0 > 9.9999997473787516e-05f)
    {
        wc_1 = wc_0 / float3(ws_0) ;
    }
    else
    {
        wc_1 = _S715.w_15;
    }
    float _S737 = wc_1.x;
    float _S738 = wc_1.y;
    float _S739 = wc_1.z;
    float3 nt_0 = float3(_S737)  * imp_oct_sphere_0(((_S729).sample((_S727), (a0_0), level((lod_1)))).xy) + float3(_S738)  * imp_oct_sphere_0(((_S729).sample((_S727), (a1_0), level((lod_1)))).xy) + float3(_S739)  * imp_oct_sphere_0(((_S729).sample((_S727), (a2_2), level((lod_1)))).xy);
    float2 vb_0 = float2(_S737)  * ((_S730).sample((_S727), (a0_0), level((lod_1)))).xy + float2(_S738)  * ((_S730).sample((_S727), (a1_0), level((lod_1)))).xy + float2(_S739)  * ((_S730).sample((_S727), (a2_2), level((lod_1)))).xy;
    float3 p_21 = pq_0 - dir_13 * float3(((_S737 * s0_0.y + _S738 * s1_1.y + _S739 * s2_0.y - 0.5f) * 2.0f * half_t_0 * _S715.s_14)) ;
    float3 nw_1 = _S715.ex_0 * float3((nt_0.x / _S715.wd_0))  + _S715.up_7 * float3(nt_0.y)  + _S715.ez_0 * float3((nt_0.z / _S715.wd_0)) ;
    float3 nw_2;
    if((dot(nw_1, nw_1)) > 9.99999993922529029e-09f)
    {
        nw_2 = normalize(nw_1);
    }
    else
    {
        nw_2 = - dir_13;
    }
    float3 v_12 = - dir_13;
    float _S740 = dot(nw_2, v_12);
    if(_S740 < 0.0f)
    {
        nw_2 = normalize(nw_2 - float3((1.0099999904632568f * _S740))  * v_12);
    }
    float bark_1 = saturate(vb_0.y);
    float iv_0 = float((g_13.user_0) & 255U) / 128.0f;
    float3 _S741 = float3((vb_0.x * (float4((&f_44->imp_0)->cfg_3) ).z)) ;
    float3 _S742 = min(mix(_S726.xyz * _S741 * float3(iv_0)  * tree_hue_0(clamp(float(((g_13.user_0) >> 8U) & 255U) / 127.5f - 1.0f, -1.0f, 1.0f)), (float4((&_S711)->bark_0) ).xyz * _S741 * float3(mix(1.0f, iv_0, 0.5f)) , float3(bark_1) ), float3(0.89999997615814209f, 0.89999997615814209f, 0.89999997615814209f));
    float zlin_3 = - (((float4(p_21, 1.0f)) * (matrix<float,int(4),int(4)> (f_44->view_0.data_0[int(0)][int(0)], f_44->view_0.data_0[int(1)][int(0)], f_44->view_0.data_0[int(2)][int(0)], f_44->view_0.data_0[int(3)][int(0)], f_44->view_0.data_0[int(0)][int(1)], f_44->view_0.data_0[int(1)][int(1)], f_44->view_0.data_0[int(2)][int(1)], f_44->view_0.data_0[int(3)][int(1)], f_44->view_0.data_0[int(0)][int(2)], f_44->view_0.data_0[int(1)][int(2)], f_44->view_0.data_0[int(2)][int(2)], f_44->view_0.data_0[int(3)][int(2)], f_44->view_0.data_0[int(0)][int(3)], f_44->view_0.data_0[int(1)][int(3)], f_44->view_0.data_0[int(2)][int(3)], f_44->view_0.data_0[int(3)][int(3)])))).z;
    float2 _S743 = i_27->pos_0.xy;
    float _S744 = sun_shadow_0(f_44, kernelContext_60->ed_c_0->shadow_0, p_21, nw_2, zlin_3, _S743, kernelContext_60);
    float2 _S745 = ao_upsample_0(f_44, kernelContext_60->ed_c_0->ao_0, _S743, zlin_3, kernelContext_60);
    float3 _S746 = sun_light_at_0(f_44, p_21, kernelContext_60);
    float _S747 = _S745.x;
    float _S748 = _S745.y;
    float3 _S749 = shade_foliage_0(f_44, _S746, _S742, 0.81999999284744263f, _S747, _S748, _S744, nw_2, nw_2, v_12, bark_1 < 0.5f, kernelContext_60);
    float3 _S750 = apply_aerial_0(f_44, _S749, p_21, _S743, kernelContext_60);
    float3 col_4;
    if(((uint4((&f_44->light_0)->flags_0) ).w) == 2U)
    {
        col_4 = float3((0.5f * _S747)) ;
    }
    else
    {
        if(((uint4((&f_44->light_0)->flags_0) ).w) == 4U)
        {
            col_4 = float3((0.5f * _S748)) ;
        }
        else
        {
            if(((uint4((&f_44->light_0)->flags_0) ).w) == 1U)
            {
                col_4 = float3((0.039999999105930328f + 0.5f * _S744)) ;
            }
            else
            {
                col_4 = _S750;
            }
        }
    }
    return float4(col_4, 1.0f);
}

struct _Array_natural_vectorx3Cfloatx2C4x3E3_0
{
    array<packed_float4, int(3)> data_7;
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

GpuInstance_natural_0 device* ed_buf_7(uint i_28, KernelContext_0 thread* kernelContext_61)
{
    return (GpuInstance_natural_0 device*)kernelContext_61->ed_buffers_0[i_28];
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

GpuMesh_natural_0 device* ed_buf_8(uint i_29, KernelContext_0 thread* kernelContext_62)
{
    return (GpuMesh_natural_0 device*)kernelContext_62->ed_buffers_0[i_29];
}

struct Vtx_0
{
    float px_6;
    float py_1;
    float pz_1;
    uint n_19;
};

Vtx_0 device* ed_buf_9(uint i_30, KernelContext_0 thread* kernelContext_63)
{
    return (Vtx_0 device*)kernelContext_63->ed_buffers_0[i_30];
}

packed_uint2 device* ed_buf_10(uint i_31, KernelContext_0 thread* kernelContext_64)
{
    return (packed_uint2 device*)kernelContext_64->ed_buffers_0[i_31];
}

float3 xform_point_0(const GpuInstance_natural_0 thread* i_32, float3 p_22)
{
    float4 _S751 = float4((&i_32->m_8)->data_7[int(0)]) ;
    float4 _S752 = float4((&i_32->m_8)->data_7[int(1)]) ;
    float4 _S753 = float4((&i_32->m_8)->data_7[int(2)]) ;
    return float3(dot(_S751.xyz, p_22) + _S751.w, dot(_S752.xyz, p_22) + _S752.w, dot(_S753.xyz, p_22) + _S753.w);
}

float max_scale_0(const GpuInstance_natural_0 thread* i_33)
{
    float4 _S754 = float4((&i_33->m_8)->data_7[int(0)]) ;
    float4 _S755 = float4((&i_33->m_8)->data_7[int(1)]) ;
    float4 _S756 = float4((&i_33->m_8)->data_7[int(2)]) ;
    return sqrt(max(max(length_squared_0(float3(_S754.x, _S755.x, _S756.x)), length_squared_0(float3(_S754.y, _S755.y, _S756.y))), length_squared_0(float3(_S754.z, _S755.z, _S756.z))));
}

float3 xform_normal_0(const GpuInstance_natural_0 thread* i_34, float3 n_20)
{
    float4 _S757 = float4((&i_34->m_8)->data_7[int(0)]) ;
    float4 _S758 = float4((&i_34->m_8)->data_7[int(1)]) ;
    float4 _S759 = float4((&i_34->m_8)->data_7[int(2)]) ;
    float3 a0_1 = float3(_S757.x, _S758.x, _S759.x);
    float3 a1_1 = float3(_S757.y, _S758.y, _S759.y);
    float3 a2_3 = float3(_S757.z, _S758.z, _S759.z);
    return cross(a1_1, a2_3) * float3(n_20.x)  + cross(a2_3, a0_1) * float3(n_20.y)  + cross(a0_1, a1_1) * float3(n_20.z) ;
}

float3 tree_vertex_0(FrameData_natural_0 device* f_45, const GpuInstance_natural_0 thread* inst_0, float3 node_2, float3 pm_0, uint n_21, uint aux_0)
{
    uint _S760 = inst_0->pad_2[int(0)];
    float3 origin_1 = float3((float4((&inst_0->m_8)->data_7[int(0)]) ).w, (float4((&inst_0->m_8)->data_7[int(1)]) ).w, (float4((&inst_0->m_8)->data_7[int(2)]) ).w);
    bool _S761;
    if((aux_0 & 134217728U) != 0U)
    {
        _S761 = ((_S760 >> 16U) & (1U << (aux_0 >> 29U))) != 0U;
    }
    else
    {
        _S761 = false;
    }
    if(_S761)
    {
        return origin_1;
    }
    float3 _S762 = xform_point_0(inst_0, pm_0);
    WindData_natural_0 w_17 = f_45->wind_1;
    float4 _S763 = float4(f_45->wind_1.cfg_2) ;
    if((_S763.x) < 0.5f)
    {
        return _S762;
    }
    float3 _S764 = node_2 + origin_1;
    float _S765 = _S763.y;
    float fade_0 = saturate((_S765 - length(_S764)) / max(0.25f * _S765, 1.0f));
    if(fade_0 <= 0.0f)
    {
        return _S762;
    }
    float q_17 = float((aux_0 >> 11U) & 127U) / 127.0f;
    float sway_0 = q_17 * q_17 * 4.0f;
    float flutter_0 = float((aux_0 >> 18U) & 7U) / 7.0f;
    float phase_0 = float((aux_0 >> 21U) & 63U) / 64.0f + float(_S760 >> 24U) / 255.0f;
    float4 _S766 = float4(w_17.wind_0) ;
    float2 _S767 = _S766.xz;
    float speed_0 = length(_S767);
    float2 wd_1;
    if(speed_0 > 0.0010000000474974513f)
    {
        wd_1 = _S767 / float2(speed_0) ;
    }
    else
    {
        wd_1 = float2(0.0f, 0.0f);
    }
    float4 _S768 = float4(w_17.gust_0) ;
    float s_15 = 0.5f + 0.5f * sin((_S768.x + dot(_S764.xz, wd_1) / max(_S768.y, 1.0f)) * 6.2831854820251465f);
    float gust_1 = s_15 * s_15 * s_15 * _S768.z;
    float t_16 = _S766.w;
    float _S769 = speed_0 * 0.10000000149011612f * fade_0;
    float _S770 = max_scale_0(inst_0);
    float k_13 = _S769 * _S770;
    float rock_1 = 0.55000001192092896f + gust_1 + 0.30000001192092896f * sin(t_16 * 1.7000000476837158f + phase_0 * 6.2831854820251465f) + 0.15000000596046448f * sin(t_16 * 0.62999999523162842f + phase_0 * 19.0f);
    thread float3 d_43 = float3(wd_1.x, 0.0f, wd_1.y) * float3((sway_0 * k_13 * rock_1)) ;
    d_43.y = d_43.y - 0.25f * sway_0 * k_13 * rock_1 * rock_1 * 0.30000001192092896f;
    float3 _S771 = xform_normal_0(inst_0, oct_decode_0(n_21));
    return _S762 + d_43 + normalize(_S771) * float3((flutter_0 * _S768.w * (0.60000002384185791f + gust_1) * 0.05000000074505806f * fade_0 * sin(t_16 * 11.0f + phase_0 * 40.0f + dot(_S762, float3(3.0999999046325684f, 1.7000000476837158f, 2.2999999523162842f)))))  * float3(_S770) ;
}

struct GpuMaterial_natural_0
{
    packed_float4 base_0;
    packed_float4 params_1;
    packed_float4 shade_0;
    packed_uint4 tex_2;
    packed_float4 uv_7;
};

GpuMaterial_natural_0 device* ed_buf_11(uint i_35, KernelContext_0 thread* kernelContext_65)
{
    return (GpuMaterial_natural_0 device*)kernelContext_65->ed_buffers_0[i_35];
}

struct TreeSurf_0
{
    float3 albedo_8;
    bool leaf_3;
    bool shell_0;
};

float2 tree_uv_0(uint w_18)
{
    return float2(float(w_18 & 65535U), float(w_18 >> 16U)) * float2(1.5259021893143654e-05f) ;
}

TreeSurf_0 tree_surface_0(FrameData_natural_0 device* f_46, const GpuInstance_natural_0 thread* inst_1, const GpuMaterial_natural_0 thread* m_9, uint2 w0_0, uint2 w1_0, uint2 w2_0, float b0_0, float b1_4, float b2_6, float2 pix_0, float3 p0_1, float3 e1_3, float3 e2_3, KernelContext_0 thread* kernelContext_66)
{
    uint a_19 = w0_0.y;
    uint _S772 = w1_0.y;
    uint _S773 = w2_0.y;
    float val_0 = (float(a_19 & 127U) * b0_0 + float(_S772 & 127U) * b1_4 + float(_S773 & 127U) * b2_6) * 0.015748031437397003f;
    float hue_0 = (float((a_19 >> 7U) & 15U) * b0_0 + float((_S772 >> 7U) & 15U) * b1_4 + float((_S773 >> 7U) & 15U) * b2_6) * 0.13333334028720856f - 1.0f;
    float iv_1 = float((inst_1->pad_2[int(0)]) & 255U) / 128.0f;
    float ih_0 = float(((inst_1->pad_2[int(0)]) >> 8U) & 255U) / 127.5f - 1.0f;
    thread TreeSurf_0 o_2;
    (&o_2)->leaf_3 = (a_19 & 134217728U) != 0U;
    (&o_2)->shell_0 = false;
    if((a_19 & 268435456U) != 0U)
    {
        (&o_2)->albedo_8 = (float4(m_9->shade_0) ).xyz * float3(val_0)  * float3(mix(1.0f, iv_1, 0.5f)) ;
        return o_2;
    }
    float3 alb_0 = (float4(m_9->base_0) ).xyz * float3(val_0)  * float3(iv_1)  * tree_hue_0(clamp(hue_0 + 0.5f * ih_0, -1.0f, 1.0f));
    bool _S774;
    if((&o_2)->leaf_3)
    {
        _S774 = ((uint4(m_9->tex_2) ).x) != 4294967295U;
    }
    else
    {
        _S774 = false;
    }
    float3 alb_1;
    if(_S774)
    {
        float2 t0_1 = tree_uv_0(w0_0.x);
        float2 d1_1 = tree_uv_0(w1_0.x) - t0_1;
        float2 d2_1 = tree_uv_0(w2_0.x) - t0_1;
        float2 uv_8 = t0_1 + d1_1 * float2(b1_4)  + d2_1 * float2(b2_6) ;
        float3 _S775 = view_dir_0(f_46, pix_0 + float2(1.0f, 0.0f));
        float2 bx_2 = ray_bary_0(_S775, p0_1, e1_3, e2_3);
        float3 _S776 = view_dir_0(f_46, pix_0 + float2(0.0f, 1.0f));
        float2 by_2 = ray_bary_0(_S776, p0_1, e1_3, e2_3);
        float2 ux_0 = t0_1 + d1_1 * float2(bx_2.x)  + d2_1 * float2(bx_2.y)  - uv_8;
        float2 uy_0 = t0_1 + d1_1 * float2(by_2.x)  + d2_1 * float2(by_2.y)  - uv_8;
        texture2d<float, access::sample> _S777 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_66->ed_textures_0))[((uint4(m_9->tex_2) ).x)]));
        sampler _S778 = ((reinterpret_cast<device sampler*>((kernelContext_66->ed_samplers_0))[(kernelContext_66->ed_c_0->smp_0)]));
        alb_1 = alb_0 * clamp(((_S777).sample((_S778), (uv_8), gradient2d((ux_0), (uy_0)))).xyz / float3(0.14699999988079071f, 0.27399998903274536f, 0.050999999046325684f), float3(0.20000000298023224f, 0.20000000298023224f, 0.20000000298023224f), float3(2.5f, 2.5f, 2.5f));
    }
    else
    {
        alb_1 = alb_0;
    }
    (&o_2)->shell_0 = !(&o_2)->leaf_3;
    (&o_2)->albedo_8 = min(alb_1, float3(0.89999997615814209f, 0.89999997615814209f, 0.89999997615814209f));
    return o_2;
}

packed_float2 device* ed_buf_12(uint i_36, KernelContext_0 thread* kernelContext_67)
{
    return (packed_float2 device*)kernelContext_67->ed_buffers_0[i_36];
}

float2 uv_xform_0(const GpuMaterial_natural_0 thread* m_10, float2 uv_9)
{
    float r_13 = (as_type<float>(((uint4(m_10->tex_2) ).w)));
    float s_16 = sin(r_13);
    float c_22 = cos(r_13);
    float4 _S779 = float4(m_10->uv_7) ;
    float2 q_18 = uv_9 * _S779.zw;
    float _S780 = q_18.x;
    float _S781 = q_18.y;
    return float2(c_22 * _S780 + s_16 * _S781, - s_16 * _S780 + c_22 * _S781) + _S779.xy;
}

float3 shade_toon_0(FrameData_natural_0 device* f_47, float3 sun_7, const GpuMaterial_natural_0 thread* m_11, float3 albedo_9, float3 n_22, float3 v_13, float ao_9, float sv_6, float sun_vis_3, KernelContext_0 thread* kernelContext_68)
{
    float ndl_0 = dot(n_22, (float4(f_47->sun_dir_0) ).xyz);
    float _S782 = max((float4(m_11->params_1) ).z, 1.0f);
    float lit_2 = floor(saturate(ndl_0 * 0.5f + 0.5f) * _S782 + 0.5f) / _S782 * sun_vis_3;
    float4 _S783 = float4(m_11->shade_0) ;
    float3 c_23 = mix(albedo_9 * _S783.xyz, albedo_9, float3(lit_2) );
    float3 _S784 = sun_7 * float3((lit_2 / 3.1415927410125732f)) ;
    float3 _S785 = ambient_light_0(f_47, n_22, kernelContext_68);
    float3 _S786 = occluded_ambient_0(f_47, _S785, sv_6, kernelContext_68);
    float3 _S787 = ao_multibounce_0(f_47, ao_9, albedo_9);
    return c_23 * (_S784 + _S786 * _S787) + float3((pow(1.0f - saturate(dot(n_22, v_13)), 4.0f) * _S783.w * saturate(ndl_0 + 0.30000001192092896f) * sun_vis_3))  * sun_7 * float3(0.15000000596046448f) ;
}

float cascade_depth_delta_0(FrameData_natural_0 device* f_48, uint atlas_2, uint c_24, float3 p_23, KernelContext_0 thread* kernelContext_69)
{
    float4 _S788 = float4((&(&f_48->light_0)->cascade_0)->data_3[c_24]) ;
    float tile_2 = (float4((&f_48->light_0)->cfg_0) ).y;
    float4 s_17 = (((float4(p_23, 1.0f)) * (matrix<float,int(4),int(4)> ((&(&f_48->light_0)->shadow_1)->data_2[c_24].data_0[int(0)][int(0)], (&(&f_48->light_0)->shadow_1)->data_2[c_24].data_0[int(1)][int(0)], (&(&f_48->light_0)->shadow_1)->data_2[c_24].data_0[int(2)][int(0)], (&(&f_48->light_0)->shadow_1)->data_2[c_24].data_0[int(3)][int(0)], (&(&f_48->light_0)->shadow_1)->data_2[c_24].data_0[int(0)][int(1)], (&(&f_48->light_0)->shadow_1)->data_2[c_24].data_0[int(1)][int(1)], (&(&f_48->light_0)->shadow_1)->data_2[c_24].data_0[int(2)][int(1)], (&(&f_48->light_0)->shadow_1)->data_2[c_24].data_0[int(3)][int(1)], (&(&f_48->light_0)->shadow_1)->data_2[c_24].data_0[int(0)][int(2)], (&(&f_48->light_0)->shadow_1)->data_2[c_24].data_0[int(1)][int(2)], (&(&f_48->light_0)->shadow_1)->data_2[c_24].data_0[int(2)][int(2)], (&(&f_48->light_0)->shadow_1)->data_2[c_24].data_0[int(3)][int(2)], (&(&f_48->light_0)->shadow_1)->data_2[c_24].data_0[int(0)][int(3)], (&(&f_48->light_0)->shadow_1)->data_2[c_24].data_0[int(1)][int(3)], (&(&f_48->light_0)->shadow_1)->data_2[c_24].data_0[int(2)][int(3)], (&(&f_48->light_0)->shadow_1)->data_2[c_24].data_0[int(3)][int(3)]))));
    int2 _S789 = int2(_S788.zw);
    float _S790 = s_17.z;
    float _S791 = ed_depth_read_0(atlas_2, uint2(clamp(_S789 + int2(floor(float2(s_17.x * 0.5f + 0.5f, 0.5f - s_17.y * 0.5f) * float2(tile_2) )), _S789, _S789 + int2((int(tile_2) - int(1))) )), kernelContext_69);
    return (_S790 - _S791) * _S788.y;
}

float4 resolve_px_0(const FsOut_0 thread* i_37, bool terrain_0, KernelContext_0 thread* kernelContext_70)
{
    FrameData_natural_0 device* _S792 = ed_buf_1(kernelContext_70->ed_c_0->frame_0, kernelContext_70);
    float2 _S793 = i_37->pos_0.xy;
    uint2 _S794 = uint2(_S793);
    texture2d<uint, access::sample> _S795 = ((reinterpret_cast<device texture2d<uint, access::sample>*>((kernelContext_70->ed_textures_0))[(kernelContext_70->ed_c_0->vis_0)]));
    int3 _S796 = int3(int2(_S794), int(0));
    uint2 vis_3 = ((_S795).read(vec<uint,2>(((_S796)).xy), uint(((_S796)).z))).xy;
    float3 _S797 = view_dir_0(_S792, _S793);
    uint _S798 = vis_3.x;
    if(_S798 == 0U)
    {
        float3 _S799 = sky_pixel_0(_S792, _S797, kernelContext_70);
        return float4(_S799, 1.0f);
    }
    bool textured_0;
    if(terrain_0)
    {
        textured_0 = (_S798 & 2147483648U) != 0U;
    }
    else
    {
        textured_0 = false;
    }
    if(textured_0)
    {
        float4 _S800 = terrain_pixel_0(_S792, i_37, vis_3, _S797, kernelContext_70);
        return _S800;
    }
    if((_S798 & 3221225472U) == 1073741824U)
    {
        float4 _S801 = impostor_pixel_0(_S792, i_37, _S798 & 3221225471U, _S797, kernelContext_70);
        return _S801;
    }
    GpuInstance_natural_0 device* _S802 = ed_buf_7(kernelContext_70->ed_c_0->instances_0, kernelContext_70);
    thread GpuInstance_natural_0 _S803 = *(_S802 + (_S798 - 1U));
    GpuMesh_natural_0 device* _S804 = ed_buf_8(kernelContext_70->ed_c_0->meshes_0, kernelContext_70);
    GpuMesh_natural_0 device* _S805 = _S804 + (&_S803)->mesh_0;
    uint _S806 = vis_3.y;
    uint lod_2 = _S806 >> 28U;
    uint prim_1 = (_S806 & 268435455U) % max(_S805->index_count_0[lod_2] / 3U, 1U);
    uint device* _S807 = ed_buf_0(kernelContext_70->ed_c_0->indices_0, kernelContext_70);
    uint device* idx_1 = _S807 + _S805->first_index_0[lod_2] + prim_1 * 3U;
    Vtx_0 device* _S808 = ed_buf_9(kernelContext_70->ed_c_0->verts_0, kernelContext_70);
    int i0_0 = int(*(idx_1 + int(0))) + _S805->base_vertex_0[lod_2];
    int i1_0 = int(*(idx_1 + int(1))) + _S805->base_vertex_0[lod_2];
    int i2_0 = int(*(idx_1 + int(2))) + _S805->base_vertex_0[lod_2];
    Vtx_0 v0_0 = *(_S808 + i0_0);
    Vtx_0 v1_0 = *(_S808 + i1_0);
    Vtx_0 v2_0 = *(_S808 + i2_0);
    packed_float4 device* _S809 = ed_buf_4(kernelContext_70->ed_c_0->nodes_0, kernelContext_70);
    float3 node_3 = (float4(*(_S809 + (((&_S803)->node_flags_0) & 65535U))) ).xyz;
    bool foliage_0 = ((_S805->flags_1) & 1U) != 0U;
    uint2 _S810 = uint2(0U, 0U);
    float3 l0_0;
    float3 l1_0;
    float3 l2_0;
    uint2 tw0_0;
    uint2 tw1_0;
    uint2 tw2_0;
    if(foliage_0)
    {
        packed_uint2 device* _S811 = ed_buf_10(kernelContext_70->ed_c_0->uvs_0, kernelContext_70);
        uint2 _S812 = uint2(*(_S811 + i0_0)) ;
        uint2 _S813 = uint2(*(_S811 + i1_0)) ;
        uint2 _S814 = uint2(*(_S811 + i2_0)) ;
        float3 _S815 = tree_vertex_0(_S792, &_S803, node_3, float3(v0_0.px_6, v0_0.py_1, v0_0.pz_1), v0_0.n_19, _S812.y);
        float3 _S816 = tree_vertex_0(_S792, &_S803, node_3, float3(v1_0.px_6, v1_0.py_1, v1_0.pz_1), v1_0.n_19, _S813.y);
        float3 _S817 = tree_vertex_0(_S792, &_S803, node_3, float3(v2_0.px_6, v2_0.py_1, v2_0.pz_1), v2_0.n_19, _S814.y);
        l0_0 = _S815;
        l1_0 = _S816;
        l2_0 = _S817;
        tw0_0 = _S812;
        tw1_0 = _S813;
        tw2_0 = _S814;
    }
    else
    {
        float3 _S818 = xform_point_0(&_S803, float3(v0_0.px_6, v0_0.py_1, v0_0.pz_1));
        float3 _S819 = xform_point_0(&_S803, float3(v1_0.px_6, v1_0.py_1, v1_0.pz_1));
        float3 _S820 = xform_point_0(&_S803, float3(v2_0.px_6, v2_0.py_1, v2_0.pz_1));
        l0_0 = _S818;
        l1_0 = _S819;
        l2_0 = _S820;
        tw0_0 = _S810;
        tw1_0 = _S810;
        tw2_0 = _S810;
    }
    float3 p0_2 = node_3 + l0_0;
    float3 e1_4 = l1_0 - l0_0;
    float3 e2_4 = l2_0 - l0_0;
    float2 bb_2 = ray_bary_0(_S797, p0_2, e1_4, e2_4);
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
    float3 _S821 = float3(b0_1) ;
    float3 local_0 = l0_0 * _S821 + l1_0 * float3(b1_6)  + l2_0 * float3(b2_8) ;
    float3 _S822 = xform_normal_0(&_S803, oct_decode_0(v0_0.n_19) * _S821 + oct_decode_0(v1_0.n_19) * float3(b1_6)  + oct_decode_0(v2_0.n_19) * float3(b2_8) );
    float3 n_23 = normalize(_S822);
    float3 v_14 = - _S797;
    GpuMaterial_natural_0 device* _S823 = ed_buf_11(kernelContext_70->ed_c_0->materials_0, kernelContext_70);
    thread GpuMaterial_natural_0 _S824 = *(_S823 + (&_S803)->material_0);
    float4 _S825 = float4((&_S824)->base_0) ;
    float3 albedo_10 = _S825.xyz;
    float4 _S826 = float4((&_S824)->params_1) ;
    float metal_1 = _S826.x;
    float rough_6 = _S826.y;
    uint4 _S827 = uint4((&_S824)->tex_2) ;
    uint _S828 = _S827.x;
    bool _S829 = _S828 != 4294967295U;
    if(_S829)
    {
        textured_0 = true;
    }
    else
    {
        textured_0 = (_S827.y) != 4294967295U;
    }
    if(textured_0)
    {
        textured_0 = true;
    }
    else
    {
        textured_0 = (_S827.z) != 4294967295U;
    }
    bool leaf_4;
    float3 albedo_11;
    float3 n_24;
    float ao_10;
    float rough_7;
    float metal_2;
    float d_44;
    if(foliage_0)
    {
        TreeSurf_0 _S830 = tree_surface_0(_S792, &_S803, &_S824, tw0_0, tw1_0, tw2_0, b0_1, b1_6, b2_8, _S793, p0_2, e1_4, e2_4, kernelContext_70);
        if(_S830.shell_0)
        {
            float3 _S831 = xform_normal_0(&_S803, float3(0.0f, 1.0f, 0.0f));
            float3 up_10 = normalize(_S831);
            float clump_0 = t_noise_0(local_0.xz * float2(0.89999997615814209f)  + float2((local_0.y * 0.60000002384185791f)) , 1.0f);
            float3 albedo_12 = _S830.albedo_8 * float3((mix(0.60000002384185791f, 1.1499999761581421f, clump_0) * 0.85000002384185791f)) ;
            float _S832 = mix(0.64999997615814209f, 1.0f, clump_0);
            n_24 = normalize(mix(n_23, up_10, float3(0.40000000596046448f) ));
            albedo_11 = albedo_12;
            ao_10 = _S832;
            rough_7 = 1.0f;
        }
        else
        {
            n_24 = n_23;
            albedo_11 = _S830.albedo_8;
            ao_10 = 1.0f;
            rough_7 = 0.81999999284744263f;
        }
        if(_S830.leaf_3)
        {
            float3 ngc_0 = cross(e1_4, e2_4);
            if((dot(ngc_0, v_14) * dot(ngc_0, n_24)) < 0.0f)
            {
                n_24 = - n_24;
            }
        }
        leaf_4 = _S830.leaf_3;
        metal_2 = 0.0f;
    }
    else
    {
        if(textured_0)
        {
            packed_float2 device* _S833 = ed_buf_12(kernelContext_70->ed_c_0->uvs_0, kernelContext_70);
            float2 _S834 = float2(*(_S833 + i0_0)) ;
            float2 d1_2 = float2(*(_S833 + i1_0))  - _S834;
            float2 d2_2 = float2(*(_S833 + i2_0))  - _S834;
            float2 _S835 = uv_xform_0(&_S824, _S834 + d1_2 * float2(b1_6)  + d2_2 * float2(b2_8) );
            float3 _S836 = view_dir_0(_S792, _S793 + float2(1.0f, 0.0f));
            float2 bx_3 = ray_bary_0(_S836, p0_2, e1_4, e2_4);
            float3 _S837 = view_dir_0(_S792, _S793 + float2(0.0f, 1.0f));
            float2 by_3 = ray_bary_0(_S837, p0_2, e1_4, e2_4);
            float2 _S838 = uv_xform_0(&_S824, _S834 + d1_2 * float2(bx_3.x)  + d2_2 * float2(bx_3.y) );
            float2 ux_1 = _S838 - _S835;
            float2 _S839 = uv_xform_0(&_S824, _S834 + d1_2 * float2(by_3.x)  + d2_2 * float2(by_3.y) );
            float2 uy_1 = _S839 - _S835;
            sampler _S840 = ((reinterpret_cast<device sampler*>((kernelContext_70->ed_samplers_0))[(kernelContext_70->ed_c_0->smp_0)]));
            if(_S829)
            {
                texture2d<float, access::sample> _S841 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_70->ed_textures_0))[(_S828)]));
                albedo_11 = albedo_10 * ((_S841).sample((_S840), (_S835), gradient2d((ux_1), (uy_1)))).xyz;
            }
            else
            {
                albedo_11 = albedo_10;
            }
            uint _S842 = _S827.z;
            if(_S842 != 4294967295U)
            {
                texture2d<float, access::sample> _S843 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_70->ed_textures_0))[(_S842)]));
                float3 orm_0 = ((_S843).sample((_S840), (_S835), gradient2d((ux_1), (uy_1)))).xyz;
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
            uint _S844 = _S827.y;
            if(_S844 != 4294967295U)
            {
                float _S845 = d1_2.x;
                float _S846 = d2_2.y;
                float _S847 = d1_2.y;
                float _S848 = d2_2.x;
                float det_2 = _S845 * _S846 - _S847 * _S848;
                if((abs(det_2)) > 9.999999960041972e-13f)
                {
                    float3 _S849 = float3((1.0f / det_2)) ;
                    float3 tg_0 = (e1_4 * float3(_S846)  - e2_4 * float3(_S847) ) * _S849;
                    float3 bt_0 = (e2_4 * float3(_S845)  - e1_4 * float3(_S848) ) * _S849;
                    float3 tg_1 = tg_0 - n_23 * float3(dot(n_23, tg_0)) ;
                    if((length_squared_0(tg_1)) > 9.99999968265522539e-21f)
                    {
                        float3 tg_2 = normalize(tg_1);
                        float3 _S850 = cross(n_23, tg_2);
                        if((dot(_S850, bt_0)) < 0.0f)
                        {
                            d_44 = -1.0f;
                        }
                        else
                        {
                            d_44 = 1.0f;
                        }
                        float3 bn_0 = _S850 * float3(- d_44) ;
                        texture2d<float, access::sample> _S851 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_70->ed_textures_0))[(_S844)]));
                        float2 xy_0 = ((_S851).sample((_S840), (_S835), gradient2d((ux_1), (uy_1)))).xy * float2(2.0f)  - float2(1.0f) ;
                        n_24 = normalize(tg_2 * float3(xy_0.x)  + bn_0 * float3(xy_0.y)  + n_23 * float3(sqrt(saturate(1.0f - dot(xy_0, xy_0)))) );
                    }
                    else
                    {
                        n_24 = n_23;
                    }
                }
                else
                {
                    n_24 = n_23;
                }
            }
            else
            {
                n_24 = n_23;
            }
        }
        else
        {
            n_24 = n_23;
            albedo_11 = albedo_10;
            ao_10 = 1.0f;
            rough_7 = rough_6;
            metal_2 = metal_1;
        }
        leaf_4 = false;
    }
    float _S852 = dot(n_24, v_14);
    if(_S852 < 0.0f)
    {
        n_24 = normalize(n_24 - float3((1.0099999904632568f * _S852))  * v_14);
    }
    if((_S826.w) > 0.5f)
    {
        float2 c_25 = floor(local_0.xz * float2(0.5f) );
        albedo_11 = albedo_11 * float3(mix(0.80000001192092896f, 1.1499999761581421f, fmod(abs(c_25.x + c_25.y), 2.0f))) ;
    }
    float3 p_24 = node_3 + local_0;
    float zlin_4 = - (((float4(p_24, 1.0f)) * (matrix<float,int(4),int(4)> (_S792->view_0.data_0[int(0)][int(0)], _S792->view_0.data_0[int(1)][int(0)], _S792->view_0.data_0[int(2)][int(0)], _S792->view_0.data_0[int(3)][int(0)], _S792->view_0.data_0[int(0)][int(1)], _S792->view_0.data_0[int(1)][int(1)], _S792->view_0.data_0[int(2)][int(1)], _S792->view_0.data_0[int(3)][int(1)], _S792->view_0.data_0[int(0)][int(2)], _S792->view_0.data_0[int(1)][int(2)], _S792->view_0.data_0[int(2)][int(2)], _S792->view_0.data_0[int(3)][int(2)], _S792->view_0.data_0[int(0)][int(3)], _S792->view_0.data_0[int(1)][int(3)], _S792->view_0.data_0[int(2)][int(3)], _S792->view_0.data_0[int(3)][int(3)])))).z;
    float3 ng_7 = cross(e1_4, e2_4);
    float3 ng_8;
    if((length_squared_0(ng_7)) > 9.99999968265522539e-21f)
    {
        ng_8 = normalize(ng_7);
    }
    else
    {
        ng_8 = n_24;
    }
    if((dot(ng_8, v_14)) < 0.0f)
    {
        ng_8 = - ng_8;
    }
    float _S853 = sun_shadow_0(_S792, kernelContext_70->ed_c_0->shadow_0, p_24, ng_8, zlin_4, _S793, kernelContext_70);
    float2 _S854 = ao_upsample_0(_S792, kernelContext_70->ed_c_0->ao_0, _S793, zlin_4, kernelContext_70);
    float ao_11 = ao_10 * _S854.x;
    float sv_7 = _S854.y;
    float3 _S855 = sun_light_at_0(_S792, p_24, kernelContext_70);
    float3 col_5;
    if(foliage_0)
    {
        float3 _S856 = shade_foliage_0(_S792, _S855, albedo_11, rough_7, ao_11, sv_7, _S853, n_24, ng_8, v_14, leaf_4, kernelContext_70);
        col_5 = _S856;
    }
    else
    {
        if((_S825.w) > 0.5f)
        {
            float3 _S857 = shade_toon_0(_S792, _S855, &_S824, albedo_11, n_24, v_14, ao_11, sv_7, _S853, kernelContext_70);
            l0_0 = _S857;
        }
        else
        {
            float3 _S858 = shade_pbr_0(_S792, _S855, albedo_11, metal_2, rough_7, ao_11, sv_7, _S853, n_24, v_14, kernelContext_70);
            l0_0 = _S858;
        }
        col_5 = l0_0;
    }
    float3 _S859 = apply_aerial_0(_S792, col_5, p_24, _S793, kernelContext_70);
    if(((uint4((&_S792->light_0)->flags_0) ).w) == 1U)
    {
        uint _S860 = shadow_cascade_0(_S792, p_24, zlin_4);
        col_5 = TINT_2[_S860] * float3((0.039999999105930328f + 0.5f * _S853)) ;
    }
    else
    {
        if(((uint4((&_S792->light_0)->flags_0) ).w) == 3U)
        {
            uint _S861 = shadow_cascade_0(_S792, p_24, zlin_4);
            if(_S861 < 4U)
            {
                float _S862 = cascade_depth_delta_0(_S792, kernelContext_70->ed_c_0->shadow_0, _S861, p_24, kernelContext_70);
                d_44 = _S862;
            }
            else
            {
                d_44 = 0.0f;
            }
            col_5 = float3(max(d_44, 0.0f), max(- d_44, 0.0f), 0.0f) * float3(0.10000000149011612f) ;
        }
        else
        {
            if(((uint4((&_S792->light_0)->flags_0) ).w) == 2U)
            {
                col_5 = float3((0.5f * ao_11)) ;
            }
            else
            {
                if(((uint4((&_S792->light_0)->flags_0) ).w) == 4U)
                {
                    col_5 = float3((0.5f * sv_7)) ;
                }
                else
                {
                    col_5 = _S859;
                }
            }
        }
    }
    return float4(col_5, 1.0f);
}

constant array<float3, int(5)> TINT_3 = { { float3(1.0f, 0.25f, 0.25f), float3(0.25f, 1.0f, 0.25f), float3(0.30000001192092896f, 0.44999998807907104f, 1.0f), float3(1.0f, 1.0f, 0.25f), float3(0.60000002384185791f, 0.60000002384185791f, 0.60000002384185791f) } };
float4 resolve_px_1(const FsOut_0 thread* i_38, bool terrain_1, KernelContext_0 thread* kernelContext_71)
{
    FrameData_natural_0 device* _S863 = ed_buf_1(kernelContext_71->ed_c_0->frame_0, kernelContext_71);
    float2 _S864 = i_38->pos_0.xy;
    uint2 _S865 = uint2(_S864);
    texture2d<uint, access::sample> _S866 = ((reinterpret_cast<device texture2d<uint, access::sample>*>((kernelContext_71->ed_textures_0))[(kernelContext_71->ed_c_0->vis_0)]));
    int3 _S867 = int3(int2(_S865), int(0));
    uint2 vis_4 = ((_S866).read(vec<uint,2>(((_S867)).xy), uint(((_S867)).z))).xy;
    float3 _S868 = view_dir_0(_S863, _S864);
    uint _S869 = vis_4.x;
    if(_S869 == 0U)
    {
        float3 _S870 = sky_pixel_0(_S863, _S868, kernelContext_71);
        return float4(_S870, 1.0f);
    }
    bool textured_1;
    if(terrain_1)
    {
        textured_1 = (_S869 & 2147483648U) != 0U;
    }
    else
    {
        textured_1 = false;
    }
    if(textured_1)
    {
        float4 _S871 = terrain_pixel_1(_S863, i_38, vis_4, _S868, kernelContext_71);
        return _S871;
    }
    if((_S869 & 3221225472U) == 1073741824U)
    {
        float4 _S872 = impostor_pixel_0(_S863, i_38, _S869 & 3221225471U, _S868, kernelContext_71);
        return _S872;
    }
    GpuInstance_natural_0 device* _S873 = ed_buf_7(kernelContext_71->ed_c_0->instances_0, kernelContext_71);
    thread GpuInstance_natural_0 _S874 = *(_S873 + (_S869 - 1U));
    GpuMesh_natural_0 device* _S875 = ed_buf_8(kernelContext_71->ed_c_0->meshes_0, kernelContext_71);
    GpuMesh_natural_0 device* _S876 = _S875 + (&_S874)->mesh_0;
    uint _S877 = vis_4.y;
    uint lod_3 = _S877 >> 28U;
    uint prim_2 = (_S877 & 268435455U) % max(_S876->index_count_0[lod_3] / 3U, 1U);
    uint device* _S878 = ed_buf_0(kernelContext_71->ed_c_0->indices_0, kernelContext_71);
    uint device* idx_2 = _S878 + _S876->first_index_0[lod_3] + prim_2 * 3U;
    Vtx_0 device* _S879 = ed_buf_9(kernelContext_71->ed_c_0->verts_0, kernelContext_71);
    int i0_1 = int(*(idx_2 + int(0))) + _S876->base_vertex_0[lod_3];
    int i1_1 = int(*(idx_2 + int(1))) + _S876->base_vertex_0[lod_3];
    int i2_1 = int(*(idx_2 + int(2))) + _S876->base_vertex_0[lod_3];
    Vtx_0 v0_1 = *(_S879 + i0_1);
    Vtx_0 v1_1 = *(_S879 + i1_1);
    Vtx_0 v2_1 = *(_S879 + i2_1);
    packed_float4 device* _S880 = ed_buf_4(kernelContext_71->ed_c_0->nodes_0, kernelContext_71);
    float3 node_4 = (float4(*(_S880 + (((&_S874)->node_flags_0) & 65535U))) ).xyz;
    bool foliage_1 = ((_S876->flags_1) & 1U) != 0U;
    uint2 _S881 = uint2(0U, 0U);
    float3 l0_1;
    float3 l1_1;
    float3 l2_1;
    uint2 tw0_1;
    uint2 tw1_1;
    uint2 tw2_1;
    if(foliage_1)
    {
        packed_uint2 device* _S882 = ed_buf_10(kernelContext_71->ed_c_0->uvs_0, kernelContext_71);
        uint2 _S883 = uint2(*(_S882 + i0_1)) ;
        uint2 _S884 = uint2(*(_S882 + i1_1)) ;
        uint2 _S885 = uint2(*(_S882 + i2_1)) ;
        float3 _S886 = tree_vertex_0(_S863, &_S874, node_4, float3(v0_1.px_6, v0_1.py_1, v0_1.pz_1), v0_1.n_19, _S883.y);
        float3 _S887 = tree_vertex_0(_S863, &_S874, node_4, float3(v1_1.px_6, v1_1.py_1, v1_1.pz_1), v1_1.n_19, _S884.y);
        float3 _S888 = tree_vertex_0(_S863, &_S874, node_4, float3(v2_1.px_6, v2_1.py_1, v2_1.pz_1), v2_1.n_19, _S885.y);
        l0_1 = _S886;
        l1_1 = _S887;
        l2_1 = _S888;
        tw0_1 = _S883;
        tw1_1 = _S884;
        tw2_1 = _S885;
    }
    else
    {
        float3 _S889 = xform_point_0(&_S874, float3(v0_1.px_6, v0_1.py_1, v0_1.pz_1));
        float3 _S890 = xform_point_0(&_S874, float3(v1_1.px_6, v1_1.py_1, v1_1.pz_1));
        float3 _S891 = xform_point_0(&_S874, float3(v2_1.px_6, v2_1.py_1, v2_1.pz_1));
        l0_1 = _S889;
        l1_1 = _S890;
        l2_1 = _S891;
        tw0_1 = _S881;
        tw1_1 = _S881;
        tw2_1 = _S881;
    }
    float3 p0_3 = node_4 + l0_1;
    float3 e1_5 = l1_1 - l0_1;
    float3 e2_5 = l2_1 - l0_1;
    float2 bb_3 = ray_bary_0(_S868, p0_3, e1_5, e2_5);
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
    float3 _S892 = float3(b0_2) ;
    float3 local_1 = l0_1 * _S892 + l1_1 * float3(b1_8)  + l2_1 * float3(b2_11) ;
    float3 _S893 = xform_normal_0(&_S874, oct_decode_0(v0_1.n_19) * _S892 + oct_decode_0(v1_1.n_19) * float3(b1_8)  + oct_decode_0(v2_1.n_19) * float3(b2_11) );
    float3 n_25 = normalize(_S893);
    float3 v_15 = - _S868;
    GpuMaterial_natural_0 device* _S894 = ed_buf_11(kernelContext_71->ed_c_0->materials_0, kernelContext_71);
    thread GpuMaterial_natural_0 _S895 = *(_S894 + (&_S874)->material_0);
    float4 _S896 = float4((&_S895)->base_0) ;
    float3 albedo_13 = _S896.xyz;
    float4 _S897 = float4((&_S895)->params_1) ;
    float metal_4 = _S897.x;
    float rough_9 = _S897.y;
    uint4 _S898 = uint4((&_S895)->tex_2) ;
    uint _S899 = _S898.x;
    bool _S900 = _S899 != 4294967295U;
    if(_S900)
    {
        textured_1 = true;
    }
    else
    {
        textured_1 = (_S898.y) != 4294967295U;
    }
    if(textured_1)
    {
        textured_1 = true;
    }
    else
    {
        textured_1 = (_S898.z) != 4294967295U;
    }
    bool leaf_5;
    float3 albedo_14;
    float3 n_26;
    float ao_12;
    float rough_10;
    float metal_5;
    float d_45;
    if(foliage_1)
    {
        TreeSurf_0 _S901 = tree_surface_0(_S863, &_S874, &_S895, tw0_1, tw1_1, tw2_1, b0_2, b1_8, b2_11, _S864, p0_3, e1_5, e2_5, kernelContext_71);
        if(_S901.shell_0)
        {
            float3 _S902 = xform_normal_0(&_S874, float3(0.0f, 1.0f, 0.0f));
            float3 up_11 = normalize(_S902);
            float clump_1 = t_noise_0(local_1.xz * float2(0.89999997615814209f)  + float2((local_1.y * 0.60000002384185791f)) , 1.0f);
            float3 albedo_15 = _S901.albedo_8 * float3((mix(0.60000002384185791f, 1.1499999761581421f, clump_1) * 0.85000002384185791f)) ;
            float _S903 = mix(0.64999997615814209f, 1.0f, clump_1);
            n_26 = normalize(mix(n_25, up_11, float3(0.40000000596046448f) ));
            albedo_14 = albedo_15;
            ao_12 = _S903;
            rough_10 = 1.0f;
        }
        else
        {
            n_26 = n_25;
            albedo_14 = _S901.albedo_8;
            ao_12 = 1.0f;
            rough_10 = 0.81999999284744263f;
        }
        if(_S901.leaf_3)
        {
            float3 ngc_1 = cross(e1_5, e2_5);
            if((dot(ngc_1, v_15) * dot(ngc_1, n_26)) < 0.0f)
            {
                n_26 = - n_26;
            }
        }
        leaf_5 = _S901.leaf_3;
        metal_5 = 0.0f;
    }
    else
    {
        if(textured_1)
        {
            packed_float2 device* _S904 = ed_buf_12(kernelContext_71->ed_c_0->uvs_0, kernelContext_71);
            float2 _S905 = float2(*(_S904 + i0_1)) ;
            float2 d1_3 = float2(*(_S904 + i1_1))  - _S905;
            float2 d2_3 = float2(*(_S904 + i2_1))  - _S905;
            float2 _S906 = uv_xform_0(&_S895, _S905 + d1_3 * float2(b1_8)  + d2_3 * float2(b2_11) );
            float3 _S907 = view_dir_0(_S863, _S864 + float2(1.0f, 0.0f));
            float2 bx_4 = ray_bary_0(_S907, p0_3, e1_5, e2_5);
            float3 _S908 = view_dir_0(_S863, _S864 + float2(0.0f, 1.0f));
            float2 by_4 = ray_bary_0(_S908, p0_3, e1_5, e2_5);
            float2 _S909 = uv_xform_0(&_S895, _S905 + d1_3 * float2(bx_4.x)  + d2_3 * float2(bx_4.y) );
            float2 ux_2 = _S909 - _S906;
            float2 _S910 = uv_xform_0(&_S895, _S905 + d1_3 * float2(by_4.x)  + d2_3 * float2(by_4.y) );
            float2 uy_2 = _S910 - _S906;
            sampler _S911 = ((reinterpret_cast<device sampler*>((kernelContext_71->ed_samplers_0))[(kernelContext_71->ed_c_0->smp_0)]));
            if(_S900)
            {
                texture2d<float, access::sample> _S912 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_71->ed_textures_0))[(_S899)]));
                albedo_14 = albedo_13 * ((_S912).sample((_S911), (_S906), gradient2d((ux_2), (uy_2)))).xyz;
            }
            else
            {
                albedo_14 = albedo_13;
            }
            uint _S913 = _S898.z;
            if(_S913 != 4294967295U)
            {
                texture2d<float, access::sample> _S914 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_71->ed_textures_0))[(_S913)]));
                float3 orm_1 = ((_S914).sample((_S911), (_S906), gradient2d((ux_2), (uy_2)))).xyz;
                float rough_11 = rough_9 * orm_1.y;
                float metal_6 = metal_4 * orm_1.z;
                ao_12 = orm_1.x;
                rough_10 = rough_11;
                metal_5 = metal_6;
            }
            else
            {
                ao_12 = 1.0f;
                rough_10 = rough_9;
                metal_5 = metal_4;
            }
            uint _S915 = _S898.y;
            if(_S915 != 4294967295U)
            {
                float _S916 = d1_3.x;
                float _S917 = d2_3.y;
                float _S918 = d1_3.y;
                float _S919 = d2_3.x;
                float det_3 = _S916 * _S917 - _S918 * _S919;
                if((abs(det_3)) > 9.999999960041972e-13f)
                {
                    float3 _S920 = float3((1.0f / det_3)) ;
                    float3 tg_3 = (e1_5 * float3(_S917)  - e2_5 * float3(_S918) ) * _S920;
                    float3 bt_1 = (e2_5 * float3(_S916)  - e1_5 * float3(_S919) ) * _S920;
                    float3 tg_4 = tg_3 - n_25 * float3(dot(n_25, tg_3)) ;
                    if((length_squared_0(tg_4)) > 9.99999968265522539e-21f)
                    {
                        float3 tg_5 = normalize(tg_4);
                        float3 _S921 = cross(n_25, tg_5);
                        if((dot(_S921, bt_1)) < 0.0f)
                        {
                            d_45 = -1.0f;
                        }
                        else
                        {
                            d_45 = 1.0f;
                        }
                        float3 bn_1 = _S921 * float3(- d_45) ;
                        texture2d<float, access::sample> _S922 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_71->ed_textures_0))[(_S915)]));
                        float2 xy_1 = ((_S922).sample((_S911), (_S906), gradient2d((ux_2), (uy_2)))).xy * float2(2.0f)  - float2(1.0f) ;
                        n_26 = normalize(tg_5 * float3(xy_1.x)  + bn_1 * float3(xy_1.y)  + n_25 * float3(sqrt(saturate(1.0f - dot(xy_1, xy_1)))) );
                    }
                    else
                    {
                        n_26 = n_25;
                    }
                }
                else
                {
                    n_26 = n_25;
                }
            }
            else
            {
                n_26 = n_25;
            }
        }
        else
        {
            n_26 = n_25;
            albedo_14 = albedo_13;
            ao_12 = 1.0f;
            rough_10 = rough_9;
            metal_5 = metal_4;
        }
        leaf_5 = false;
    }
    float _S923 = dot(n_26, v_15);
    if(_S923 < 0.0f)
    {
        n_26 = normalize(n_26 - float3((1.0099999904632568f * _S923))  * v_15);
    }
    if((_S897.w) > 0.5f)
    {
        float2 c_26 = floor(local_1.xz * float2(0.5f) );
        albedo_14 = albedo_14 * float3(mix(0.80000001192092896f, 1.1499999761581421f, fmod(abs(c_26.x + c_26.y), 2.0f))) ;
    }
    float3 p_25 = node_4 + local_1;
    float zlin_5 = - (((float4(p_25, 1.0f)) * (matrix<float,int(4),int(4)> (_S863->view_0.data_0[int(0)][int(0)], _S863->view_0.data_0[int(1)][int(0)], _S863->view_0.data_0[int(2)][int(0)], _S863->view_0.data_0[int(3)][int(0)], _S863->view_0.data_0[int(0)][int(1)], _S863->view_0.data_0[int(1)][int(1)], _S863->view_0.data_0[int(2)][int(1)], _S863->view_0.data_0[int(3)][int(1)], _S863->view_0.data_0[int(0)][int(2)], _S863->view_0.data_0[int(1)][int(2)], _S863->view_0.data_0[int(2)][int(2)], _S863->view_0.data_0[int(3)][int(2)], _S863->view_0.data_0[int(0)][int(3)], _S863->view_0.data_0[int(1)][int(3)], _S863->view_0.data_0[int(2)][int(3)], _S863->view_0.data_0[int(3)][int(3)])))).z;
    float3 ng_9 = cross(e1_5, e2_5);
    float3 ng_10;
    if((length_squared_0(ng_9)) > 9.99999968265522539e-21f)
    {
        ng_10 = normalize(ng_9);
    }
    else
    {
        ng_10 = n_26;
    }
    if((dot(ng_10, v_15)) < 0.0f)
    {
        ng_10 = - ng_10;
    }
    float _S924 = sun_shadow_0(_S863, kernelContext_71->ed_c_0->shadow_0, p_25, ng_10, zlin_5, _S864, kernelContext_71);
    float2 _S925 = ao_upsample_0(_S863, kernelContext_71->ed_c_0->ao_0, _S864, zlin_5, kernelContext_71);
    float ao_13 = ao_12 * _S925.x;
    float sv_8 = _S925.y;
    float3 _S926 = sun_light_at_0(_S863, p_25, kernelContext_71);
    float3 col_6;
    if(foliage_1)
    {
        float3 _S927 = shade_foliage_0(_S863, _S926, albedo_14, rough_10, ao_13, sv_8, _S924, n_26, ng_10, v_15, leaf_5, kernelContext_71);
        col_6 = _S927;
    }
    else
    {
        if((_S896.w) > 0.5f)
        {
            float3 _S928 = shade_toon_0(_S863, _S926, &_S895, albedo_14, n_26, v_15, ao_13, sv_8, _S924, kernelContext_71);
            l0_1 = _S928;
        }
        else
        {
            float3 _S929 = shade_pbr_0(_S863, _S926, albedo_14, metal_5, rough_10, ao_13, sv_8, _S924, n_26, v_15, kernelContext_71);
            l0_1 = _S929;
        }
        col_6 = l0_1;
    }
    float3 _S930 = apply_aerial_0(_S863, col_6, p_25, _S864, kernelContext_71);
    if(((uint4((&_S863->light_0)->flags_0) ).w) == 1U)
    {
        uint _S931 = shadow_cascade_0(_S863, p_25, zlin_5);
        col_6 = TINT_3[_S931] * float3((0.039999999105930328f + 0.5f * _S924)) ;
    }
    else
    {
        if(((uint4((&_S863->light_0)->flags_0) ).w) == 3U)
        {
            uint _S932 = shadow_cascade_0(_S863, p_25, zlin_5);
            if(_S932 < 4U)
            {
                float _S933 = cascade_depth_delta_0(_S863, kernelContext_71->ed_c_0->shadow_0, _S932, p_25, kernelContext_71);
                d_45 = _S933;
            }
            else
            {
                d_45 = 0.0f;
            }
            col_6 = float3(max(d_45, 0.0f), max(- d_45, 0.0f), 0.0f) * float3(0.10000000149011612f) ;
        }
        else
        {
            if(((uint4((&_S863->light_0)->flags_0) ).w) == 2U)
            {
                col_6 = float3((0.5f * ao_13)) ;
            }
            else
            {
                if(((uint4((&_S863->light_0)->flags_0) ).w) == 4U)
                {
                    col_6 = float3((0.5f * sv_8)) ;
                }
                else
                {
                    col_6 = _S930;
                }
            }
        }
    }
    return float4(col_6, 1.0f);
}

FsOut_0 fullscreen_vertex_0(uint vid_0)
{
    thread FsOut_0 o_3;
    (&o_3)->pos_0 = float4(float2(float((vid_0 << 1U) & 2U), float(vid_0 & 2U)) * float2(2.0f)  - float2(1.0f) , 0.0f, 1.0f);
    return o_3;
}

struct pixelOutput_0
{
    float4 output_0 [[color(0)]];
};

[[fragment]] pixelOutput_0 fs_resolve(float4 pos_1 [[position]], ulong device* ed_buffers_1 [[buffer(2)]], ResolveC_0 constant* ed_c_1 [[buffer(0)]], ulong device* ed_samplers_1 [[buffer(3)]], ulong device* ed_textures_1 [[buffer(1)]])
{
    thread KernelContext_0 kernelContext_72;
    (&kernelContext_72)->ed_buffers_0 = ed_buffers_1;
    (&kernelContext_72)->ed_c_0 = ed_c_1;
    (&kernelContext_72)->ed_samplers_0 = ed_samplers_1;
    (&kernelContext_72)->ed_textures_0 = ed_textures_1;
    thread FsOut_0 _S934;
    (&_S934)->pos_0 = pos_1;
    float4 _S935 = resolve_px_1(&_S934, false, &kernelContext_72);
    pixelOutput_0 _S936 = { _S935 };
    return _S936;
}

struct pixelOutput_1
{
    float4 output_1 [[color(0)]];
};

[[fragment]] pixelOutput_1 fs_resolve_terrain(float4 pos_2 [[position]], ulong device* ed_buffers_2 [[buffer(2)]], ResolveC_0 constant* ed_c_2 [[buffer(0)]], ulong device* ed_samplers_2 [[buffer(3)]], ulong device* ed_textures_2 [[buffer(1)]])
{
    thread KernelContext_0 kernelContext_73;
    (&kernelContext_73)->ed_buffers_0 = ed_buffers_2;
    (&kernelContext_73)->ed_c_0 = ed_c_2;
    (&kernelContext_73)->ed_samplers_0 = ed_samplers_2;
    (&kernelContext_73)->ed_textures_0 = ed_textures_2;
    thread FsOut_0 _S937;
    (&_S937)->pos_0 = pos_2;
    float4 _S938 = resolve_px_1(&_S937, true, &kernelContext_73);
    pixelOutput_1 _S939 = { _S938 };
    return _S939;
}

struct pixelOutput_2
{
    float4 output_2 [[color(0)]];
};

[[fragment]] pixelOutput_2 fs_resolve_trees(float4 pos_3 [[position]], ulong device* ed_buffers_3 [[buffer(2)]], ResolveC_0 constant* ed_c_3 [[buffer(0)]], ulong device* ed_samplers_3 [[buffer(3)]], ulong device* ed_textures_3 [[buffer(1)]])
{
    thread KernelContext_0 kernelContext_74;
    (&kernelContext_74)->ed_buffers_0 = ed_buffers_3;
    (&kernelContext_74)->ed_c_0 = ed_c_3;
    (&kernelContext_74)->ed_samplers_0 = ed_samplers_3;
    (&kernelContext_74)->ed_textures_0 = ed_textures_3;
    thread FsOut_0 _S940;
    (&_S940)->pos_0 = pos_3;
    float4 _S941 = resolve_px_0(&_S940, true, &kernelContext_74);
    pixelOutput_2 _S942 = { _S941 };
    return _S942;
}

struct vs_resolve_Result_0
{
    float4 pos_4 [[position]];
};

[[vertex]] vs_resolve_Result_0 vs_resolve(uint vid_1 [[vertex_id]], ulong device* ed_buffers_4 [[buffer(2)]], ResolveC_0 constant* ed_c_4 [[buffer(0)]], ulong device* ed_samplers_4 [[buffer(3)]], ulong device* ed_textures_4 [[buffer(1)]])
{
    thread KernelContext_0 kernelContext_75;
    (&kernelContext_75)->ed_buffers_0 = ed_buffers_4;
    (&kernelContext_75)->ed_c_0 = ed_c_4;
    (&kernelContext_75)->ed_samplers_0 = ed_samplers_4;
    (&kernelContext_75)->ed_textures_0 = ed_textures_4;
    thread vs_resolve_Result_0 _S943;
    (&_S943)->pos_4 = fullscreen_vertex_0(vid_1).pos_0;
    return _S943;
}

