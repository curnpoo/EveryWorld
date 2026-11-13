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

float t_lift_k_0(const TerrainDraw_natural_0 thread* d_2, float3 p_0)
{
    float _S16 = (float4(d_2->polar_0) ).w;
    if(_S16 <= 0.0f)
    {
        return 0.0f;
    }
    return smoothstep(12.0f, 6.0f, _S16 / max(length((float4(d_2->origin_0) ).xyz + p_0), 1.0f));
}

uint t_canopy_raw_0(uint pool_2, uint off_0, uint i_4, uint j_3, KernelContext_0 thread* kernelContext_3)
{
    uint device* _S17 = ed_buf_0(pool_2, kernelContext_3);
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

float t_canopy_lift_0(uint pool_3, uint off_1, uint i_5, uint j_4, KernelContext_0 thread* kernelContext_4)
{
    if(off_1 == 0U)
    {
        return 0.0f;
    }
    uint _S19 = t_canopy_raw_0(pool_3, off_1, i_5, j_4, kernelContext_4);
    return t_canopy_h_0((_S19 >> 8U) & 255U) * smoothstep(0.059999998658895493f, 0.44999998807907104f, float(_S19 & 255U) / 255.0f);
}

float3 t_target_even_0(uint pool_4, const TerrainDraw_natural_0 thread* d_3, uint i_6, uint j_5, KernelContext_0 thread* kernelContext_5)
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
        float _S22 = t_own_h_0(pool_4, d_3, i_6, j_5, kernelContext_5);
        h_1 = _S22;
    }
    else
    {
        float4 _S23 = float4(d_3->hq_0) ;
        float _S24 = t_page_0(pool_4, _S20.y, qx_0 + 2U * i_6, qy_0 + 2U * j_5, _S23.z, _S23.w, kernelContext_5);
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
                float _S28 = t_canopy_lift_0(pool_4, (as_type<uint>((_S27.z))), i_6 * 4U, j_5 * 4U, kernelContext_5);
                h_1 = _S28;
            }
            else
            {
                float _S29 = t_canopy_lift_0(pool_4, poff_0, qx_0 + 2U * i_6, qy_0 + 2U * j_5, kernelContext_5);
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

float t_own_lift_0(uint pool_5, const TerrainDraw_natural_0 thread* d_4, uint i_7, uint j_6, float3 p_2, KernelContext_0 thread* kernelContext_6)
{
    if(((float4(d_4->polar_0) ).w) <= 0.0f)
    {
        return 0.0f;
    }
    float _S30 = t_lift_k_0(d_4, p_2);
    float _S31;
    if(_S30 > 0.0f)
    {
        float _S32 = t_canopy_lift_0(pool_5, (as_type<uint>(((float4(d_4->tex_0) ).z))), i_7 * 4U, j_6 * 4U, kernelContext_6);
        _S31 = _S30 * _S32;
    }
    else
    {
        _S31 = 0.0f;
    }
    return _S31;
}

float3 t_base_0(uint pool_6, const TerrainDraw_natural_0 thread* d_5, uint i_8, uint j_7, KernelContext_0 thread* kernelContext_7)
{
    float _S33 = t_own_h_0(pool_6, d_5, i_8, j_7, kernelContext_7);
    thread float3 dir_2;
    float3 _S34 = t_lattice_0(d_5, i_8, j_7, _S33, &dir_2);
    float3 p_3;
    if(((float4(d_5->polar_0) ).w) > 0.0f)
    {
        float _S35 = t_own_lift_0(pool_6, d_5, i_8, j_7, _S34, kernelContext_7);
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

float3 t_coarse_even_0(uint pool_7, const TerrainDraw_natural_0 thread* d_6, uint e_1, uint2 a_0, uint2 b_0, uint2 c_0, KernelContext_0 thread* kernelContext_8)
{
    float3 _S37 = t_base_0(pool_7, d_6, a_0.x, a_0.y, kernelContext_8);
    if((e_1 & 3U) == 0U)
    {
        return _S37;
    }
    float m_0 = t_morph_0((float4(d_6->origin_0) ).xyz + _S37, (float4(d_6->morph_0) ).zw);
    float3 _S38 = t_base_0(pool_7, d_6, b_0.x, b_0.y, kernelContext_8);
    float3 _S39 = t_base_0(pool_7, d_6, c_0.x, c_0.y, kernelContext_8);
    return mix(_S37, float3(0.5f)  * (_S38 + _S39), float3(m_0) );
}

float3 t_target_0(uint pool_8, const TerrainDraw_natural_0 thread* d_7, uint i_9, uint j_8, KernelContext_0 thread* kernelContext_9)
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
        float3 _S41 = t_target_even_0(pool_8, d_7, i_9, j_8, kernelContext_9);
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
        float3 _S42 = t_target_even_0(pool_8, d_7, i_9 - 1U, j_8, kernelContext_9);
        float3 _S43 = t_target_even_0(pool_8, d_7, i_9 + 1U, j_8, kernelContext_9);
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
        float3 _S44 = t_target_even_0(pool_8, d_7, i_9, j_8 - 1U, kernelContext_9);
        float3 _S45 = t_target_even_0(pool_8, d_7, i_9, j_8 + 1U, kernelContext_9);
        return float3(0.5f)  * (_S44 + _S45);
    }
    float3 _S46 = t_target_even_0(pool_8, d_7, i_9 - 1U, j_8 - 1U, kernelContext_9);
    float3 _S47 = t_target_even_0(pool_8, d_7, i_9 + 1U, j_8 + 1U, kernelContext_9);
    return float3(0.5f)  * (_S46 + _S47);
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

float3 t_coarse_0(uint pool_9, const TerrainDraw_natural_0 thread* d_8, uint edge_1, uint e_3, KernelContext_0 thread* kernelContext_10)
{
    if((e_3 & 1U) == 0U)
    {
        float3 _S48 = t_coarse_even_0(pool_9, d_8, e_3, t_edge_at_0(edge_1, e_3), t_edge_at_0(edge_1, e_3 - 2U), t_edge_at_0(edge_1, e_3 + 2U), kernelContext_10);
        return _S48;
    }
    uint _S49 = e_3 - 1U;
    uint2 _S50 = t_edge_at_0(edge_1, _S49);
    uint _S51 = e_3 + 1U;
    uint2 _S52 = t_edge_at_0(edge_1, _S51);
    float3 _S53 = t_coarse_even_0(pool_9, d_8, _S49, _S50, t_edge_at_0(edge_1, e_3 - 3U), _S52, kernelContext_10);
    float3 _S54 = t_coarse_even_0(pool_9, d_8, _S51, _S52, _S50, t_edge_at_0(edge_1, e_3 + 3U), kernelContext_10);
    return float3(0.5f)  * (_S53 + _S54);
}

float3 t_grid_vertex_0(uint pool_10, const TerrainDraw_natural_0 thread* d_9, uint i_10, uint j_9, KernelContext_0 thread* kernelContext_11)
{
    uint fl_0 = (uint4(d_9->slots_0) ).z;
    uint edge_2;
    uint e_4;
    if(i_10 == 0U)
    {
        edge_2 = 0U;
        e_4 = j_9;
    }
    else
    {
        if(i_10 == 64U)
        {
            edge_2 = 1U;
            e_4 = j_9;
        }
        else
        {
            if(j_9 == 0U)
            {
                edge_2 = 2U;
                e_4 = i_10;
            }
            else
            {
                if(j_9 == 64U)
                {
                    edge_2 = 3U;
                    e_4 = i_10;
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
        bool _S55;
        if((fl_0 & (1U << edge_2)) != 0U)
        {
            _S55 = E_0 <= 2U;
        }
        else
        {
            _S55 = false;
        }
        if(_S55)
        {
            float3 _S56 = t_coarse_0(pool_10, d_9, edge_2, e_4, kernelContext_11);
            return _S56;
        }
        if(E_0 > 1U)
        {
            uint lo_0 = e_4 & (~(E_0 - 1U));
            if(lo_0 == e_4)
            {
                float3 _S57 = t_base_0(pool_10, d_9, i_10, j_9, kernelContext_11);
                return _S57;
            }
            uint2 a_1 = t_edge_at_0(edge_2, lo_0);
            uint2 b_1 = t_edge_at_0(edge_2, lo_0 + E_0);
            float3 _S58 = t_base_0(pool_10, d_9, a_1.x, a_1.y, kernelContext_11);
            float3 _S59 = t_base_0(pool_10, d_9, b_1.x, b_1.y, kernelContext_11);
            return mix(_S58, _S59, float3((float(e_4 - lo_0) / float(E_0))) );
        }
    }
    float3 _S60 = t_base_0(pool_10, d_9, i_10, j_9, kernelContext_11);
    if((fl_0 & 4194304U) != 0U)
    {
        return _S60;
    }
    float m_1 = t_morph_0((float4(d_9->origin_0) ).xyz + _S60, (float4(d_9->morph_0) ).xy);
    if(m_1 <= 0.0f)
    {
        return _S60;
    }
    float3 _S61 = t_target_0(pool_10, d_9, i_10, j_9, kernelContext_11);
    return mix(_S60, _S61, float3(m_1) );
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

float3 t_vertex_0(uint pool_11, const TerrainDraw_natural_0 thread* d_10, uint v_0, KernelContext_0 thread* kernelContext_12)
{
    if(v_0 < 4225U)
    {
        float3 _S62 = (float4(d_10->origin_0) ).xyz;
        float3 _S63 = t_grid_vertex_0(pool_11, d_10, v_0 % 65U, v_0 / 65U, kernelContext_12);
        return _S62 + _S63;
    }
    uint2 ij_0 = t_ring_0(v_0 - 4225U);
    uint _S64 = ij_0.x;
    uint _S65 = ij_0.y;
    float3 _S66 = t_grid_vertex_0(pool_11, d_10, _S64, _S65, kernelContext_12);
    float depth_0;
    if((((uint4(d_10->slots_0) ).z) & 64U) != 0U)
    {
        depth_0 = (float4(d_10->origin_0) ).w;
    }
    else
    {
        depth_0 = 0.0f;
    }
    bool _S67;
    if(((float4(d_10->polar_0) ).w) > 0.0f)
    {
        _S67 = depth_0 > 0.0f;
    }
    else
    {
        _S67 = false;
    }
    if(_S67)
    {
        float _S68 = t_own_lift_0(pool_11, d_10, _S64, _S65, _S66, kernelContext_12);
        depth_0 = depth_0 + _S68;
    }
    float3 _S69 = (float4(d_10->origin_0) ).xyz;
    float3 _S70 = _S69 + _S66;
    float4 _S71 = float4(d_10->dir_c_0) ;
    return _S70 - normalize(_S70 - (_S69 - _S71.xyz * float3(_S71.w) )) * float3(depth_0) ;
}

constant array<float3, int(5)> TINT_0 = { { float3(1.0f, 0.25f, 0.25f), float3(0.25f, 1.0f, 0.25f), float3(0.30000001192092896f, 0.44999998807907104f, 1.0f), float3(1.0f, 1.0f, 0.25f), float3(0.60000002384185791f, 0.60000002384185791f, 0.60000002384185791f) } };
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

struct FsOut_0
{
    float4 pos_0;
};

TerrainDraw_natural_0 device* ed_buf_1(uint i_11, KernelContext_0 thread* kernelContext_13)
{
    return (TerrainDraw_natural_0 device*)kernelContext_13->ed_buffers_0[i_11];
}

uint3 t_triangle_0(uint prim_0)
{
    uint lod_0 = (prim_0 >> 24U) & 3U;
    uint _S72 = prim_0 & 16777215U;
    uint st_0 = 1U << lod_0;
    uint c_1 = 64U >> lod_0;
    uint _S73 = 2U * c_1 * c_1;
    uint3 _S74;
    if(_S72 < _S73)
    {
        uint q_1 = _S72 >> 1U;
        uint _S75 = q_1 % c_1;
        uint i_12 = _S75 * st_0;
        uint _S76 = q_1 / c_1;
        uint v00_0 = _S76 * st_0 * 65U + i_12;
        uint v10_0 = v00_0 + st_0;
        uint v01_0 = v00_0 + st_0 * 65U;
        uint v11_0 = v01_0 + st_0;
        if((_S72 & 1U) == 0U)
        {
            _S74 = uint3(v00_0, v10_0, v11_0);
        }
        else
        {
            _S74 = uint3(v00_0, v11_0, v01_0);
        }
        return _S74;
    }
    uint k_1 = _S72 - _S73;
    uint s_3 = k_1 >> 1U;
    uint s1_0 = (s_3 + 1U) % (4U * c_1);
    uint _S77 = s_3 * st_0;
    uint2 p_4 = t_ring_0(_S77);
    uint _S78 = s1_0 * st_0;
    uint2 q_2 = t_ring_0(_S78);
    uint tp_0 = p_4.y * 65U + p_4.x;
    uint tq_0 = q_2.y * 65U + q_2.x;
    uint a_2 = 4225U + _S77;
    uint b_2 = 4225U + _S78;
    if((k_1 & 1U) == 0U)
    {
        _S74 = uint3(a_2, b_2, tq_0);
    }
    else
    {
        _S74 = uint3(a_2, tq_0, tp_0);
    }
    return _S74;
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
    float4 _S79 = float4(d_11->shape_0) ;
    float _S80 = _S79.x;
    float tu_1 = tan((g_0.x - 32.0f) * _S80);
    float tv_2 = tan((g_0.y - 32.0f) * _S80);
    float4 _S81 = float4(d_11->fu_0) ;
    float _S82 = _S81.w;
    float4 _S83 = float4(d_11->fv_0) ;
    float _S84 = _S83.w;
    float3 delta_1 = float3((tu_1 * (1.0f + _S82 * _S82) / (1.0f - _S82 * tu_1)))  * _S81.xyz + float3((tv_2 * (1.0f + _S84 * _S84) / (1.0f - _S84 * tv_2)))  * _S83.xyz;
    float4 _S85 = float4(d_11->cc_0) ;
    float lc_1 = _S85.w;
    float3 _S86 = _S85.xyz;
    float k_2 = 2.0f * dot(_S86, delta_1) + dot(delta_1, delta_1);
    float l_1 = sqrt(lc_1 * lc_1 + k_2);
    float3 dd_1 = delta_1 / float3(l_1)  - _S86 * float3((k_2 / ((lc_1 + l_1) * l_1 * lc_1))) ;
    float4 _S87 = float4(d_11->dir_c_0) ;
    float3 _S88 = _S87.xyz;
    float3 _S89 = _S88 + dd_1;
    *dir_4 = _S89;
    float3 _S90 = (float4(d_11->polar_0) ).xyz;
    float dzc_1 = dot(_S88, _S90);
    float ddz_1 = dot(dd_1, _S90);
    float dz_1 = dzc_1 + ddz_1;
    float e_6 = _S79.z;
    float s_4 = sqrt(1.0f - e_6 * dz_1 * dz_1);
    float sc_1 = _S79.w;
    float dr_1 = _S79.y * e_6 * ddz_1 * (dz_1 + dzc_1) / (s_4 * sc_1 * (s_4 + sc_1));
    return dd_1 * float3((_S87.w + dr_1))  + _S88 * float3(dr_1)  + _S89 * float3(h_2) ;
}

float t_sample_spacing_0(const TerrainDraw_natural_0 thread* d_12, float2 g_1)
{
    float2 _S91 = float2(0.5f, 0.0f);
    thread float3 dir_5;
    float3 _S92 = t_lattice_f_0(d_12, g_1 + _S91, 0.0f, &dir_5);
    float3 _S93 = t_lattice_f_0(d_12, g_1 - _S91, 0.0f, &dir_5);
    return length(_S92 - _S93) / 4.0f;
}

float t_page_at_0(uint pool_12, const TerrainDraw_natural_0 thread* d_13, int a_3, int b_3, KernelContext_0 thread* kernelContext_14)
{
    float4 _S94 = float4(d_13->hq_0) ;
    float _S95 = t_page_0(pool_12, (uint4(d_13->slots_0) ).x, uint(clamp(a_3, int(0), int(256))), uint(clamp(b_3, int(0), int(256))), _S94.x, _S94.y, kernelContext_14);
    return _S95;
}

void t_surface_0(uint pool_13, const TerrainDraw_natural_0 thread* d_14, float2 g_2, int r_0, float thread* h_3, float2 thread* dh_0, KernelContext_0 thread* kernelContext_15)
{
    float2 _S96 = float2(4.0f) ;
    float2 _S97 = float2(0.0f) ;
    float2 s_5 = clamp(g_2 * _S96, _S97, float2(256.0f) );
    int2 _S98 = int2(min(floor(s_5), float2(255.0f) ));
    float2 _S99 = s_5 - float2(_S98);
    *h_3 = 0.0f;
    *dh_0 = _S97;
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
        uint _S100 = k_3 & 1U;
        uint _S101 = k_3 >> 1U;
        int2 q_3 = _S98 + int2(int(_S100), int(_S101));
        float _S102;
        if(_S100 != 0U)
        {
            _S102 = _S99.x;
        }
        else
        {
            _S102 = 1.0f - _S99.x;
        }
        float _S103;
        if(_S101 != 0U)
        {
            _S103 = _S99.y;
        }
        else
        {
            _S103 = 1.0f - _S99.y;
        }
        float w_1 = _S102 * _S103;
        int2 _S104 = int2(r_0) ;
        int2 _S105 = max(q_3 - _S104, int2(int(0)) );
        int2 _S106 = min(q_3 + _S104, int2(int(256)) );
        int _S107 = _S106.x;
        int _S108 = q_3.y;
        float _S109 = t_page_at_0(pool_13, d_14, _S107, _S108, kernelContext_15);
        int _S110 = _S105.x;
        float _S111 = t_page_at_0(pool_13, d_14, _S110, _S108, kernelContext_15);
        float gx_0 = (_S109 - _S111) / float(_S107 - _S110);
        int _S112 = q_3.x;
        int _S113 = _S106.y;
        float _S114 = t_page_at_0(pool_13, d_14, _S112, _S113, kernelContext_15);
        int _S115 = _S105.y;
        float _S116 = t_page_at_0(pool_13, d_14, _S112, _S115, kernelContext_15);
        float gy_0 = (_S114 - _S116) / float(_S113 - _S115);
        float _S117 = t_page_at_0(pool_13, d_14, _S112, _S108, kernelContext_15);
        *h_3 = *h_3 + w_1 * _S117;
        *dh_0 = *dh_0 + float2(w_1)  * float2(gx_0, gy_0);
        k_3 = k_3 + 1U;
    }
    *dh_0 = *dh_0 * _S96;
    return;
}

float3 t_lattice_f_1(const TerrainDraw_natural_0 thread* d_15, float2 g_3, float h_4, float3 thread* dir_6)
{
    float4 _S118 = float4(d_15->shape_0) ;
    float _S119 = _S118.x;
    float tu_2 = tan((g_3.x - 32.0f) * _S119);
    float tv_3 = tan((g_3.y - 32.0f) * _S119);
    float4 _S120 = float4(d_15->fu_0) ;
    float _S121 = _S120.w;
    float4 _S122 = float4(d_15->fv_0) ;
    float _S123 = _S122.w;
    float3 delta_2 = float3((tu_2 * (1.0f + _S121 * _S121) / (1.0f - _S121 * tu_2)))  * _S120.xyz + float3((tv_3 * (1.0f + _S123 * _S123) / (1.0f - _S123 * tv_3)))  * _S122.xyz;
    float4 _S124 = float4(d_15->cc_0) ;
    float lc_2 = _S124.w;
    float3 _S125 = _S124.xyz;
    float k_4 = 2.0f * dot(_S125, delta_2) + dot(delta_2, delta_2);
    float l_2 = sqrt(lc_2 * lc_2 + k_4);
    float3 dd_2 = delta_2 / float3(l_2)  - _S125 * float3((k_4 / ((lc_2 + l_2) * l_2 * lc_2))) ;
    float4 _S126 = float4(d_15->dir_c_0) ;
    float3 _S127 = _S126.xyz;
    float3 _S128 = _S127 + dd_2;
    *dir_6 = _S128;
    float3 _S129 = (float4(d_15->polar_0) ).xyz;
    float dzc_2 = dot(_S127, _S129);
    float ddz_2 = dot(dd_2, _S129);
    float dz_2 = dzc_2 + ddz_2;
    float e_7 = _S118.z;
    float s_6 = sqrt(1.0f - e_7 * dz_2 * dz_2);
    float sc_2 = _S118.w;
    float dr_2 = _S118.y * e_7 * ddz_2 * (dz_2 + dzc_2) / (s_6 * sc_2 * (s_6 + sc_2));
    return dd_2 * float3((_S126.w + dr_2))  + _S127 * float3(dr_2)  + _S128 * float3(h_4) ;
}

float3 t_smooth_normal_0(uint pool_14, const TerrainDraw_natural_0 thread* d_16, float2 g_4, int r_1, float3 thread* up_1, float thread* h_5, KernelContext_0 thread* kernelContext_16)
{
    thread float2 dh_1;
    t_surface_0(pool_14, d_16, g_4, r_1, h_5, &dh_1, kernelContext_16);
    float2 _S130 = float2(0.25f, 0.0f);
    thread float3 dir_7;
    float3 _S131 = t_lattice_f_0(d_16, g_4 + _S130, *h_5, &dir_7);
    float3 _S132 = t_lattice_f_0(d_16, g_4 - _S130, *h_5, &dir_7);
    float3 pu_0 = _S131 - _S132;
    float2 _S133 = float2(0.0f, 0.25f);
    float3 _S134 = t_lattice_f_0(d_16, g_4 + _S133, *h_5, &dir_7);
    float3 _S135 = t_lattice_f_0(d_16, g_4 - _S133, *h_5, &dir_7);
    float3 pv_1 = _S134 - _S135;
    float3 _S136 = t_lattice_f_1(d_16, g_4, *h_5, up_1);
    float3 _S137 = normalize(*up_1);
    *up_1 = _S137;
    float3 _S138 = float3(0.5f) ;
    float3 n_0 = normalize(cross(pu_0 / _S138 + _S137 * float3(dh_1.x) , pv_1 / _S138 + _S137 * float3(dh_1.y) ));
    float3 _S139;
    if((dot(n_0, _S137)) < 0.0f)
    {
        _S139 = - n_0;
    }
    else
    {
        _S139 = n_0;
    }
    return _S139;
}

float3 terrain_albedo_0(float h_6, float slope_cos_0)
{
    return mix(mix(mix(float3(0.054999999701976776f, 0.075000002980232239f, 0.039999999105930328f), float3(0.125f, 0.14000000059604645f, 0.070000000298023224f), float3(smoothstep(1950.0f, 2350.0f, h_6)) ), float3(0.20999999344348907f, 0.20000000298023224f, 0.18500000238418579f), float3(smoothstep(0.81999999284744263f, 0.72000002861022949f, slope_cos_0)) ), float3(0.80000001192092896f, 0.81999999284744263f, 0.86000001430511475f), float3((smoothstep(2750.0f, 3050.0f, h_6) * smoothstep(0.55000001192092896f, 0.69999998807907104f, slope_cos_0))) );
}

float4 t_canopy_bilin_0(uint pool_15, uint off_2, float2 s_7, KernelContext_0 thread* kernelContext_17)
{
    float2 _S140 = clamp(s_7, float2(0.0f) , float2(256.0f) );
    int2 _S141 = int2(min(floor(_S140), float2(255.0f) ));
    float2 _S142 = _S140 - float2(_S141);
    uint _S143 = uint(_S141.x);
    uint _S144 = uint(_S141.y);
    uint _S145 = t_canopy_raw_0(pool_15, off_2, _S143, _S144, kernelContext_17);
    float _S146 = _S142.x;
    float _S147 = 1.0f - _S146;
    float _S148 = _S142.y;
    float _S149 = 1.0f - _S148;
    float wk_0 = _S147 * _S149;
    float ww_0 = wk_0 * (float(_S145 & 255U) / 255.0f);
    float _S150 = ww_0 * t_canopy_h_0((_S145 >> 8U) & 255U);
    float _S151 = ww_0 * float((_S145 >> 16U) & 255U) / 255.0f;
    float _S152 = wk_0 * float(_S145 >> 30U) / 3.0f;
    uint _S153 = _S143 + 1U;
    uint _S154 = t_canopy_raw_0(pool_15, off_2, _S153, _S144, kernelContext_17);
    float wk_1 = _S146 * _S149;
    float ww_1 = wk_1 * (float(_S154 & 255U) / 255.0f);
    float cover_0 = ww_0 + ww_1;
    float hs_0 = _S150 + ww_1 * t_canopy_h_0((_S154 >> 8U) & 255U);
    float mx_0 = _S151 + ww_1 * float((_S154 >> 16U) & 255U) / 255.0f;
    float un_0 = _S152 + wk_1 * float(_S154 >> 30U) / 3.0f;
    uint _S155 = _S144 + 1U;
    uint _S156 = t_canopy_raw_0(pool_15, off_2, _S143, _S155, kernelContext_17);
    float wk_2 = _S147 * _S148;
    float ww_2 = wk_2 * (float(_S156 & 255U) / 255.0f);
    float cover_1 = cover_0 + ww_2;
    float hs_1 = hs_0 + ww_2 * t_canopy_h_0((_S156 >> 8U) & 255U);
    float mx_1 = mx_0 + ww_2 * float((_S156 >> 16U) & 255U) / 255.0f;
    float un_1 = un_0 + wk_2 * float(_S156 >> 30U) / 3.0f;
    uint _S157 = t_canopy_raw_0(pool_15, off_2, _S153, _S155, kernelContext_17);
    float wk_3 = _S146 * _S148;
    float ww_3 = wk_3 * (float(_S157 & 255U) / 255.0f);
    float cover_2 = cover_1 + ww_3;
    float hs_2 = hs_1 + ww_3 * t_canopy_h_0((_S157 >> 8U) & 255U);
    float mx_2 = mx_1 + ww_3 * float((_S157 >> 16U) & 255U) / 255.0f;
    float un_2 = un_1 + wk_3 * float(_S157 >> 30U) / 3.0f;
    float4 _S158;
    if(cover_2 > 0.0f)
    {
        _S158 = float4(cover_2, hs_2 / cover_2, mx_2 / cover_2, un_2);
    }
    else
    {
        _S158 = float4(cover_2, 0.0f, 0.0f, un_2);
    }
    return _S158;
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

TerrainMat_natural_0 device* ed_buf_2(uint i_13, KernelContext_0 thread* kernelContext_18)
{
    return (TerrainMat_natural_0 device*)kernelContext_18->ed_buffers_0[i_13];
}

float3 view_dir_0(FrameData_natural_0 device* f_0, float2 pixel_0)
{
    float2 uv_0 = pixel_0 / (float4(f_0->viewport_0) ).zw;
    float4 p_5 = (((float4(float2(uv_0.x * 2.0f - 1.0f, 1.0f - uv_0.y * 2.0f), 1.0f, 1.0f)) * (matrix<float,int(4),int(4)> (f_0->inv_view_proj_0.data_0[int(0)][int(0)], f_0->inv_view_proj_0.data_0[int(1)][int(0)], f_0->inv_view_proj_0.data_0[int(2)][int(0)], f_0->inv_view_proj_0.data_0[int(3)][int(0)], f_0->inv_view_proj_0.data_0[int(0)][int(1)], f_0->inv_view_proj_0.data_0[int(1)][int(1)], f_0->inv_view_proj_0.data_0[int(2)][int(1)], f_0->inv_view_proj_0.data_0[int(3)][int(1)], f_0->inv_view_proj_0.data_0[int(0)][int(2)], f_0->inv_view_proj_0.data_0[int(1)][int(2)], f_0->inv_view_proj_0.data_0[int(2)][int(2)], f_0->inv_view_proj_0.data_0[int(3)][int(2)], f_0->inv_view_proj_0.data_0[int(0)][int(3)], f_0->inv_view_proj_0.data_0[int(1)][int(3)], f_0->inv_view_proj_0.data_0[int(2)][int(3)], f_0->inv_view_proj_0.data_0[int(3)][int(3)]))));
    return normalize(p_5.xyz / float3(p_5.w) );
}

float t_page_bilin_0(uint pool_16, const TerrainDraw_natural_0 thread* d_17, float2 s_8, KernelContext_0 thread* kernelContext_19)
{
    float2 _S159 = clamp(s_8, float2(0.0f) , float2(256.0f) );
    int2 _S160 = int2(min(floor(_S159), float2(255.0f) ));
    float2 t_0 = _S159 - float2(_S160);
    int _S161 = _S160.x;
    int _S162 = _S160.y;
    float _S163 = t_page_at_0(pool_16, d_17, _S161, _S162, kernelContext_19);
    int _S164 = _S161 + int(1);
    float _S165 = t_page_at_0(pool_16, d_17, _S164, _S162, kernelContext_19);
    float _S166 = t_0.x;
    float a_4 = mix(_S163, _S165, _S166);
    int _S167 = _S162 + int(1);
    float _S168 = t_page_at_0(pool_16, d_17, _S161, _S167, kernelContext_19);
    float _S169 = t_page_at_0(pool_16, d_17, _S164, _S167, kernelContext_19);
    return mix(a_4, mix(_S168, _S169, _S166), t_0.y);
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
    float2 i_14 = floor(q_4);
    float2 f_1 = q_4 - i_14;
    float2 f_2 = f_1 * f_1 * (float2(3.0f)  - float2(2.0f)  * f_1);
    int per_0 = int(4096.0f / cell_0 + 0.5f);
    int2 _S170 = int2(i_14);
    int _S171 = _S170.x % per_0;
    int _S172 = (_S171 + per_0) % per_0;
    int _S173 = _S170.y % per_0;
    int _S174 = (_S173 + per_0) % per_0;
    int2 c0_0 = int2(_S172, _S174);
    int _S175 = (_S172 + int(1)) % per_0;
    int _S176 = (_S174 + int(1)) % per_0;
    float _S177 = f_2.x;
    return mix(mix(t_hash2_0(c0_0), t_hash2_0(int2(_S175, _S174)), _S177), mix(t_hash2_0(int2(_S172, _S176)), t_hash2_0(int2(_S175, _S176)), _S177), f_2.y);
}

struct TProj_0
{
    float2 w_3;
    float2 dx_0;
    float2 dy_0;
};

TProj_0 TProj_x24init_0(float2 w_4, float2 dx_1, float2 dy_1)
{
    thread TProj_0 _S178;
    (&_S178)->w_3 = w_4;
    (&_S178)->dx_0 = dx_1;
    (&_S178)->dy_0 = dy_1;
    return _S178;
}

float4 t_layer_albedo_0(const TerrainMat_natural_0 thread* m_2, sampler smp_1, uint layer_0, const TProj_0 thread* p_6, float near_0, KernelContext_0 thread* kernelContext_20)
{
    float k_5 = 1.0f / m_2->tile_0[layer_0];
    texture2d_array<float, access::sample> _S179 = ((reinterpret_cast<device texture2d_array<float, access::sample>*>((kernelContext_20->ed_textures_0))[(m_2->albedo_0)]));
    float _S180 = p_6->w_3.x;
    float _S181 = - p_6->w_3.y;
    float2 _S182 = float2(k_5) ;
    float _S183 = float(layer_0);
    float3 _S184 = float3(float2(_S180, _S181) * _S182, _S183);
    float _S185 = p_6->dx_0.x;
    float _S186 = - p_6->dx_0.y;
    float _S187 = p_6->dy_0.x;
    float _S188 = - p_6->dy_0.y;
    float4 a_5 = ((_S179).sample((smp_1), ((_S184)).xy, uint(((_S184)).z), gradient2d((float2(_S185, _S186) * _S182), (float2(_S187, _S188) * _S182))));
    float2 _S189 = float2((0.25f * k_5)) ;
    float3 _S190 = float3(float2(_S181, - _S180) * _S189, _S183);
    float4 b_4 = ((_S179).sample((smp_1), ((_S190)).xy, uint(((_S190)).z), gradient2d((float2(_S186, - _S185) * _S189), (float2(_S188, - _S187) * _S189))));
    float mixk_0 = mix(0.34999999403953552f, 0.75f, 1.0f - near_0);
    return float4(mix(a_5.xyz, b_4.xyz, float3(mixk_0) ), mix(a_5.w, b_4.w, 0.5f * mixk_0));
}

float4 t_layer_normal_0(const TerrainMat_natural_0 thread* m_3, sampler smp_2, uint layer_1, const TProj_0 thread* p_7, KernelContext_0 thread* kernelContext_21)
{
    float k_6 = 1.0f / m_3->tile_0[layer_1];
    texture2d_array<float, access::sample> _S191 = ((reinterpret_cast<device texture2d_array<float, access::sample>*>((kernelContext_21->ed_textures_0))[(m_3->normal_0)]));
    float2 _S192 = float2(k_6) ;
    float3 _S193 = float3(float2(p_7->w_3.x, - p_7->w_3.y) * _S192, float(layer_1));
    float4 b_5 = ((_S191).sample((smp_2), ((_S193)).xy, uint(((_S193)).z), gradient2d((float2(p_7->dx_0.x, - p_7->dx_0.y) * _S192), (float2(p_7->dy_0.x, - p_7->dy_0.y) * _S192))));
    return float4(b_5.xy * float2(2.0f)  - float2(1.0f) , b_5.z, b_5.w);
}

TSurf_0 terrain_material_0(FrameData_natural_0 device* f_3, const TerrainMat_natural_0 thread* m_4, const TerrainDraw_natural_0 thread* d_18, float3 n_2, float3 nm_0, float3 up_2, float h_9, float curv_0, float curv_f_0, float2 w_5, float2 dwx_0, float2 dwy_0, float dhx_0, float dhy_0, float3 tu_3, float3 tv_4, float dist_0, bool trees_0, float4 cv_0, float spacing_0, KernelContext_0 thread* kernelContext_22)
{
    float4 _S194;
    float best_0;
    bool _S195;
    float4 nr_0;
    float3 pert_0;
    float3 nw_0;
    sampler _S196 = ((reinterpret_cast<device sampler*>((kernelContext_22->ed_samplers_0))[(kernelContext_22->ed_c_0->smp_0)]));
    float cs_0 = dot(nm_0, up_2);
    float cf_0 = dot(n_2, up_2);
    float3 _S197 = (float4(d_18->polar_0) ).xyz;
    float3 north_0 = normalize(_S197 - up_2 * float3(dot(_S197, up_2)) );
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
    float _S198 = n3_0 - 0.5f;
    float treeline_0 = 2150.0f + 120.0f * _S198 - 100.0f * northness_0;
    float _S199 = n2_0 - 0.5f;
    float snowline_0 = 3050.0f - 230.0f * northness_0 + 200.0f * _S199;
    float alpine_0 = smoothstep(treeline_0 - 150.0f, treeline_0 + 250.0f, h_9);
    float rock_cs_0 = mix(0.68000000715255737f, 0.76999998092651367f, alpine_0);
    float steep_0 = smoothstep(rock_cs_0, rock_cs_0 - 0.14000000059604645f, cs_0);
    float _S200 = n1_0 - 0.5f;
    float rock_0 = saturate(saturate(max(steep_0, smoothstep(rock_cs_0 - 0.059999998658895493f, rock_cs_0 - 0.20000000298023224f, cf_0) * 0.80000001192092896f) + saturate(- curv_0 * 60.0f) * 0.5f * alpine_0 + smoothstep(2700.0f, 3300.0f, h_9) * 0.44999998807907104f - 0.25f * hollow_0) + _S200 * 0.5f * smoothstep(0.92000001668930054f, 0.80000001192092896f, cs_0));
    float _S201 = _S199 * 0.80000001192092896f;
    float scree_0 = saturate(smoothstep(0.97000002861022949f, 0.87999999523162842f, cs_0) * smoothstep(0.6600000262260437f, 0.80000001192092896f, cs_0) * (0.10000000149011612f + hollow_0 * 1.2000000476837158f + _S201) * mix(0.10000000149011612f, 1.0f, alpine_0) + steep_0 * 0.30000001192092896f * alpine_0);
    float forest_0 = (1.0f - alpine_0) * smoothstep(0.99000000953674316f, 0.94999998807907104f, cs_0) * saturate(0.89999997615814209f + _S198 * 1.3999999761581421f + _S201);
    float forest_1;
    if(trees_0)
    {
        float _S202 = cv_0.x;
        forest_1 = mix(forest_0, mix(_S202, smoothstep(0.029999999329447746f, 0.5f, _S202), smoothstep(80.0f, 20.0f, spacing_0)), cv_0.w);
    }
    else
    {
        forest_1 = forest_0;
    }
    float grass_0 = (1.0f - forest_1) * smoothstep(3000.0f, 2600.0f, h_9 + _S200 * 200.0f);
    float soil_0 = saturate(hollow_f_0 * 0.80000001192092896f + (n1_0 - 0.64999997615814209f) * 1.5f) * (1.0f - alpine_0 * 0.5f);
    float snow_0 = smoothstep(snowline_0 - 120.0f, snowline_0 + 120.0f, h_9 + hollow_0 * 150.0f + _S200 * 120.0f) * smoothstep(0.62000000476837158f, 0.77999997138977051f, cf_0 + _S200 * 0.10000000149011612f);
    thread array<float, int(6)> wt_0;
    wt_0[0U] = rock_0;
    wt_0[1U] = scree_0 * (1.0f - rock_0 * 0.69999998807907104f);
    float _S203 = 1.0f - rock_0;
    float _S204 = 1.0f - scree_0 * 0.80000001192092896f;
    wt_0[2U] = grass_0 * _S203 * _S204;
    wt_0[3U] = forest_1 * _S203 * _S204;
    float _S205 = 1.0f - snow_0;
    wt_0[5U] = soil_0 * 0.5f * _S203 * _S205;
    wt_0[4U] = snow_0 * 1.6000000238418579f;
    wt_0[0U] = wt_0[0U] * _S205;
    wt_0[1U] = wt_0[1U] * _S205;
    wt_0[2U] = wt_0[2U] * _S205;
    wt_0[3U] = wt_0[3U] * _S205;
    wt_0[5U] = wt_0[5U] * _S205;
    float near_1 = saturate(1.0f - dist_0 / 400.0f);
    float3 _S206 = pow(abs(float3(dot(n_2, tu_3), dot(n_2, tv_4), cf_0)), float3(4.0f) );
    thread float3 tw_0 = _S206;
    float _S207 = _S206.x;
    float _S208 = _S206.y;
    float major_0 = max(_S207, _S208) - min(_S207, _S208);
    float2 _S209;
    if(_S207 > _S208)
    {
        _S209 = float2(major_0, 0.0f);
    }
    else
    {
        _S209 = float2(0.0f, major_0);
    }
    tw_0.xy = _S209;
    float3 _S210 = float3(0.0f) ;
    float3 _S211 = max(tw_0 - float3(0.029999999329447746f) , _S210);
    tw_0 = _S211;
    tw_0.z = max(_S211.z, 0.0099999997764825821f);
    float3 _S212 = tw_0 / float3(max(tw_0.x + tw_0.y + tw_0.z, 9.9999997473787516e-05f)) ;
    tw_0 = _S212;
    TProj_0 _S213 = TProj_x24init_0(w_5, dwx_0, dwy_0);
    bool side_u_0 = (_S212.y) >= (_S212.x);
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
        _S209 = float2(w_5.x, h_9);
    }
    else
    {
        _S209 = float2(w_5.y, h_9);
    }
    (&side_0)->w_3 = _S209;
    if(side_u_0)
    {
        _S209 = float2(dwx_0.x, dhx_0);
    }
    else
    {
        _S209 = float2(dwx_0.y, dhx_0);
    }
    (&side_0)->dx_0 = _S209;
    if(side_u_0)
    {
        _S209 = float2(dwy_0.x, dhy_0);
    }
    else
    {
        _S209 = float2(dwy_0.y, dhy_0);
    }
    (&side_0)->dy_0 = _S209;
    float3 _S214;
    if(side_u_0)
    {
        _S214 = tu_3;
    }
    else
    {
        _S214 = tv_4;
    }
    thread array<float4, int(6)> ah_0;
    for(;;)
    {
        for(;;)
        {
            for(;;)
            {
                float4 _S215 = float4(0.0f) ;
                _S194 = _S215;
                ah_0[0U] = _S215;
                if((wt_0[0U]) < 0.039999999105930328f)
                {
                    wt_0[0U] = 0.0f;
                    best_0 = 0.0f;
                    break;
                }
                bool _S216 = northness_0 > 0.0f;
                if((tw_0.z) > 0.0f)
                {
                    _S195 = true;
                }
                else
                {
                    _S195 = !_S216;
                }
                if(_S195)
                {
                    thread TProj_0 _S217 = _S213;
                    float4 _S218 = t_layer_albedo_0(m_4, _S196, 0U, &_S217, near_1, kernelContext_22);
                    nr_0 = _S218;
                }
                else
                {
                    nr_0 = _S215;
                }
                if(_S216)
                {
                    float4 _S219 = nr_0 * float4(tw_0.z) ;
                    thread TProj_0 _S220 = side_0;
                    float4 _S221 = t_layer_albedo_0(m_4, _S196, 0U, &_S220, near_1, kernelContext_22);
                    nr_0 = _S219 + _S221 * float4(northness_0) ;
                }
                ah_0[0U] = nr_0;
                float _S222 = wt_0[0U] + nr_0.w * mix(0.15000000596046448f, 0.60000002384185791f, near_1);
                wt_0[0U] = _S222;
                best_0 = max(0.0f, _S222);
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                ah_0[1U] = _S194;
                if((wt_0[1U]) < 0.039999999105930328f)
                {
                    wt_0[1U] = 0.0f;
                    break;
                }
                thread TProj_0 _S223 = _S213;
                float4 _S224 = t_layer_albedo_0(m_4, _S196, 1U, &_S223, near_1, kernelContext_22);
                ah_0[1U] = _S224;
                float _S225 = wt_0[1U] + _S224.w * mix(0.15000000596046448f, 0.60000002384185791f, near_1);
                wt_0[1U] = _S225;
                best_0 = max(best_0, _S225);
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                ah_0[2U] = _S194;
                if((wt_0[2U]) < 0.039999999105930328f)
                {
                    wt_0[2U] = 0.0f;
                    break;
                }
                thread TProj_0 _S226 = _S213;
                float4 _S227 = t_layer_albedo_0(m_4, _S196, 2U, &_S226, near_1, kernelContext_22);
                ah_0[2U] = _S227;
                float _S228 = wt_0[2U] + _S227.w * mix(0.15000000596046448f, 0.60000002384185791f, near_1);
                wt_0[2U] = _S228;
                best_0 = max(best_0, _S228);
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                ah_0[3U] = _S194;
                if((wt_0[3U]) < 0.039999999105930328f)
                {
                    wt_0[3U] = 0.0f;
                    break;
                }
                thread TProj_0 _S229 = _S213;
                float4 _S230 = t_layer_albedo_0(m_4, _S196, 3U, &_S229, near_1, kernelContext_22);
                ah_0[3U] = _S230;
                float _S231 = wt_0[3U] + _S230.w * mix(0.15000000596046448f, 0.60000002384185791f, near_1);
                wt_0[3U] = _S231;
                best_0 = max(best_0, _S231);
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                ah_0[4U] = _S194;
                if((wt_0[4U]) < 0.039999999105930328f)
                {
                    wt_0[4U] = 0.0f;
                    break;
                }
                bool _S232 = northness_0 > 0.0f;
                if((tw_0.z) > 0.0f)
                {
                    _S195 = true;
                }
                else
                {
                    _S195 = !_S232;
                }
                if(_S195)
                {
                    thread TProj_0 _S233 = _S213;
                    float4 _S234 = t_layer_albedo_0(m_4, _S196, 4U, &_S233, near_1, kernelContext_22);
                    nr_0 = _S234;
                }
                else
                {
                    nr_0 = _S194;
                }
                if(_S232)
                {
                    float4 _S235 = nr_0 * float4(tw_0.z) ;
                    thread TProj_0 _S236 = side_0;
                    float4 _S237 = t_layer_albedo_0(m_4, _S196, 4U, &_S236, near_1, kernelContext_22);
                    nr_0 = _S235 + _S237 * float4(northness_0) ;
                }
                ah_0[4U] = nr_0;
                float _S238 = wt_0[4U] + nr_0.w * mix(0.15000000596046448f, 0.60000002384185791f, near_1);
                wt_0[4U] = _S238;
                best_0 = max(best_0, _S238);
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                ah_0[5U] = _S194;
                if((wt_0[5U]) < 0.039999999105930328f)
                {
                    wt_0[5U] = 0.0f;
                    break;
                }
                thread TProj_0 _S239 = _S213;
                float4 _S240 = t_layer_albedo_0(m_4, _S196, 5U, &_S239, near_1, kernelContext_22);
                ah_0[5U] = _S240;
                float _S241 = wt_0[5U] + _S240.w * mix(0.15000000596046448f, 0.60000002384185791f, near_1);
                wt_0[5U] = _S241;
                best_0 = max(best_0, _S241);
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
    (&o_0)->albedo_1 = _S210;
    (&o_0)->n_1 = _S210;
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
                    pert_0 = _S210;
                    break;
                }
                float a_6 = wt_0[0U] / max(total_4, 9.9999997473787516e-06f);
                float3 _S242 = float3(a_6) ;
                (&o_0)->albedo_1 = (&o_0)->albedo_1 + _S242 * ah_0[0U].xyz * (float4((&m_4->gain_0)->data_5[0U]) ).xyz;
                float4 _S243 = float4(0.0f, 0.0f, 0.92000001668930054f, 1.0f);
                if(near_1 > 0.0f)
                {
                    bool _S244 = northness_0 > 0.0f;
                    if((tw_0.z) > 0.0f)
                    {
                        _S195 = true;
                    }
                    else
                    {
                        _S195 = !_S244;
                    }
                    if(_S195)
                    {
                        thread TProj_0 _S245 = _S213;
                        float4 _S246 = t_layer_normal_0(m_4, _S196, 0U, &_S245, kernelContext_22);
                        if(_S244)
                        {
                            s_9 = tw_0.z;
                        }
                        else
                        {
                            s_9 = 1.0f;
                        }
                        float3 _S247 = (tu_3 * float3(_S246.x)  + tv_4 * float3(_S246.y) ) * float3(s_9) ;
                        nr_0 = _S246 * float4(s_9) ;
                        pert_0 = _S247;
                    }
                    else
                    {
                        nr_0 = _S194;
                        pert_0 = _S210;
                    }
                    if(_S244)
                    {
                        thread TProj_0 _S248 = side_0;
                        float4 _S249 = t_layer_normal_0(m_4, _S196, 0U, &_S248, kernelContext_22);
                        float4 nr_1 = nr_0 + _S249 * float4(northness_0) ;
                        pert_0 = pert_0 + (_S214 * float3(_S249.x)  + up_2 * float3(_S249.y) ) * float3(northness_0) ;
                        nr_0 = nr_1;
                    }
                }
                else
                {
                    pert_0 = _S210;
                    nr_0 = _S243;
                }
                float3 _S250 = _S242 * pert_0;
                (&o_0)->rough_0 = (&o_0)->rough_0 + a_6 * nr_0.z;
                (&o_0)->ao_2 = (&o_0)->ao_2 + a_6 * nr_0.w;
                pert_0 = _S250;
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
                float3 _S251 = float3(a_7) ;
                (&o_0)->albedo_1 = (&o_0)->albedo_1 + _S251 * ah_0[1U].xyz * (float4((&m_4->gain_0)->data_5[1U]) ).xyz;
                float4 _S252 = float4(0.0f, 0.0f, 0.92000001668930054f, 1.0f);
                if(near_1 > 0.0f)
                {
                    thread TProj_0 _S253 = _S213;
                    float4 _S254 = t_layer_normal_0(m_4, _S196, 1U, &_S253, kernelContext_22);
                    nw_0 = tu_3 * float3(_S254.x)  + tv_4 * float3(_S254.y) ;
                    nr_0 = _S254;
                }
                else
                {
                    nw_0 = _S210;
                    nr_0 = _S252;
                }
                float3 pert_1 = pert_0 + _S251 * nw_0;
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
                float3 _S255 = float3(a_8) ;
                (&o_0)->albedo_1 = (&o_0)->albedo_1 + _S255 * ah_0[2U].xyz * (float4((&m_4->gain_0)->data_5[2U]) ).xyz;
                float4 _S256 = float4(0.0f, 0.0f, 0.92000001668930054f, 1.0f);
                if(near_1 > 0.0f)
                {
                    thread TProj_0 _S257 = _S213;
                    float4 _S258 = t_layer_normal_0(m_4, _S196, 2U, &_S257, kernelContext_22);
                    nw_0 = tu_3 * float3(_S258.x)  + tv_4 * float3(_S258.y) ;
                    nr_0 = _S258;
                }
                else
                {
                    nw_0 = _S210;
                    nr_0 = _S256;
                }
                float3 pert_2 = pert_0 + _S255 * nw_0;
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
                float3 _S259 = float3(a_9) ;
                (&o_0)->albedo_1 = (&o_0)->albedo_1 + _S259 * ah_0[3U].xyz * (float4((&m_4->gain_0)->data_5[3U]) ).xyz;
                float4 _S260 = float4(0.0f, 0.0f, 0.92000001668930054f, 1.0f);
                if(near_1 > 0.0f)
                {
                    thread TProj_0 _S261 = _S213;
                    float4 _S262 = t_layer_normal_0(m_4, _S196, 3U, &_S261, kernelContext_22);
                    nw_0 = tu_3 * float3(_S262.x)  + tv_4 * float3(_S262.y) ;
                    nr_0 = _S262;
                }
                else
                {
                    nw_0 = _S210;
                    nr_0 = _S260;
                }
                float3 pert_3 = pert_0 + _S259 * nw_0;
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
                float3 _S263 = float3(a_10) ;
                (&o_0)->albedo_1 = (&o_0)->albedo_1 + _S263 * ah_0[4U].xyz * (float4((&m_4->gain_0)->data_5[4U]) ).xyz;
                float4 _S264 = float4(0.0f, 0.0f, 0.92000001668930054f, 1.0f);
                if(near_1 > 0.0f)
                {
                    bool _S265 = northness_0 > 0.0f;
                    if((tw_0.z) > 0.0f)
                    {
                        _S195 = true;
                    }
                    else
                    {
                        _S195 = !_S265;
                    }
                    if(_S195)
                    {
                        thread TProj_0 _S266 = _S213;
                        float4 _S267 = t_layer_normal_0(m_4, _S196, 4U, &_S266, kernelContext_22);
                        if(_S265)
                        {
                            s_9 = tw_0.z;
                        }
                        else
                        {
                            s_9 = 1.0f;
                        }
                        float3 _S268 = (tu_3 * float3(_S267.x)  + tv_4 * float3(_S267.y) ) * float3(s_9) ;
                        nr_0 = _S267 * float4(s_9) ;
                        nw_0 = _S268;
                    }
                    else
                    {
                        nr_0 = _S194;
                        nw_0 = _S210;
                    }
                    if(_S265)
                    {
                        thread TProj_0 _S269 = side_0;
                        float4 _S270 = t_layer_normal_0(m_4, _S196, 4U, &_S269, kernelContext_22);
                        float4 nr_2 = nr_0 + _S270 * float4(northness_0) ;
                        nw_0 = nw_0 + (_S214 * float3(_S270.x)  + up_2 * float3(_S270.y) ) * float3(northness_0) ;
                        nr_0 = nr_2;
                    }
                }
                else
                {
                    nw_0 = _S210;
                    nr_0 = _S264;
                }
                float3 pert_4 = pert_0 + _S263 * nw_0;
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
                float3 _S271 = float3(a_11) ;
                (&o_0)->albedo_1 = (&o_0)->albedo_1 + _S271 * ah_0[5U].xyz * (float4((&m_4->gain_0)->data_5[5U]) ).xyz;
                float4 _S272 = float4(0.0f, 0.0f, 0.92000001668930054f, 1.0f);
                if(near_1 > 0.0f)
                {
                    thread TProj_0 _S273 = _S213;
                    float4 _S274 = t_layer_normal_0(m_4, _S196, 5U, &_S273, kernelContext_22);
                    nw_0 = tu_3 * float3(_S274.x)  + tv_4 * float3(_S274.y) ;
                    nr_0 = _S274;
                }
                else
                {
                    nw_0 = _S210;
                    nr_0 = _S272;
                }
                float3 pert_5 = pert_0 + _S271 * nw_0;
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
    (&o_0)->albedo_1 = (&o_0)->albedo_1 * float3((mix(0.81999999284744263f, 1.1499999761581421f, n2_0) * mix(0.89999997615814209f, 1.0800000429153442f, n3_0) * mix(1.0f, 0.80000001192092896f, hollow_f_0 * _S205)))  * mix(float3(1.0f) , float3(1.0399999618530273f, 1.0f, 0.93999999761581421f), float3((n1_0 * _S205)) );
    float _S275 = wt_0[3U] / max(total_4, 9.9999997473787516e-06f) * smoothstep(60.0f, 600.0f, dist_0);
    if(trees_0)
    {
        northness_0 = 1.0f - cv_0.w;
    }
    else
    {
        northness_0 = 1.0f;
    }
    (&o_0)->albedo_1 = mix((&o_0)->albedo_1, float3(0.029999999329447746f, 0.043999999761581421f, 0.020999999716877937f) * float3(mix(0.85000002384185791f, 1.1499999761581421f, n1_0)) , float3((_S275 * northness_0)) );
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
    int _S276;
    int2 _S277;
    float4 best_1;
    float2 x_0 = w_6 / float2(cell_1) ;
    float2 _S278 = floor(x_0);
    int _S279 = int(4096.0f / cell_1 + 0.5f);
    float4 _S280 = float4(0.0f, 0.0f, 9.0f, 0.0f);
    for(;;)
    {
        for(;;)
        {
            for(;;)
            {
                float2 c_3 = _S278 + float2(-1.0f, -1.0f);
                int2 _S281 = int2(c_3);
                int _S282 = _S281.x % _S279;
                int _S283 = (_S282 + _S279) % _S279;
                int _S284 = _S281.y % _S279;
                int _S285 = (_S284 + _S279) % _S279;
                int2 ci_0 = int2(_S283, _S285);
                int _S286 = int(salt_0);
                _S276 = _S286;
                int2 _S287 = int2(_S286, int(0));
                _S277 = _S287;
                if((t_hash2_0(ci_0 + _S287)) >= pres_0)
                {
                    best_1 = _S280;
                    break;
                }
                float h1_0 = t_hash2_0(ci_0 + int2(int(17), _S286 + int(5)));
                float2 _S288 = float2(0.5f) ;
                float2 q_5 = (x_0 - (c_3 + _S288 + (float2(h1_0, t_hash2_0(ci_0 + int2(_S286 + int(3), int(7)))) - _S288) * float2(0.80000001192092896f) )) / float2((rad_0 * mix(0.60000002384185791f, 1.3500000238418579f, t_hash2_0(ci_0 + int2(_S286 + int(9), int(31)))))) ;
                float dq_0 = length(q_5);
                if(dq_0 < 9.0f)
                {
                    best_1 = float4(q_5, dq_0, h1_0);
                }
                else
                {
                    best_1 = _S280;
                }
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                float2 c_4 = _S278 + float2(0.0f, -1.0f);
                int2 _S289 = int2(c_4);
                int _S290 = _S289.x % _S279;
                int _S291 = (_S290 + _S279) % _S279;
                int _S292 = _S289.y % _S279;
                int _S293 = (_S292 + _S279) % _S279;
                int2 ci_1 = int2(_S291, _S293);
                if((t_hash2_0(ci_1 + _S277)) >= pres_0)
                {
                    break;
                }
                float h1_1 = t_hash2_0(ci_1 + int2(int(17), _S276 + int(5)));
                float2 _S294 = float2(0.5f) ;
                float2 q_6 = (x_0 - (c_4 + _S294 + (float2(h1_1, t_hash2_0(ci_1 + int2(_S276 + int(3), int(7)))) - _S294) * float2(0.80000001192092896f) )) / float2((rad_0 * mix(0.60000002384185791f, 1.3500000238418579f, t_hash2_0(ci_1 + int2(_S276 + int(9), int(31)))))) ;
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
                float2 c_5 = _S278 + float2(1.0f, -1.0f);
                int2 _S295 = int2(c_5);
                int _S296 = _S295.x % _S279;
                int _S297 = (_S296 + _S279) % _S279;
                int _S298 = _S295.y % _S279;
                int _S299 = (_S298 + _S279) % _S279;
                int2 ci_2 = int2(_S297, _S299);
                if((t_hash2_0(ci_2 + _S277)) >= pres_0)
                {
                    break;
                }
                float h1_2 = t_hash2_0(ci_2 + int2(int(17), _S276 + int(5)));
                float2 _S300 = float2(0.5f) ;
                float2 q_7 = (x_0 - (c_5 + _S300 + (float2(h1_2, t_hash2_0(ci_2 + int2(_S276 + int(3), int(7)))) - _S300) * float2(0.80000001192092896f) )) / float2((rad_0 * mix(0.60000002384185791f, 1.3500000238418579f, t_hash2_0(ci_2 + int2(_S276 + int(9), int(31)))))) ;
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
                float2 c_6 = _S278 + float2(-1.0f, 0.0f);
                int2 _S301 = int2(c_6);
                int _S302 = _S301.x % _S279;
                int _S303 = (_S302 + _S279) % _S279;
                int _S304 = _S301.y % _S279;
                int _S305 = (_S304 + _S279) % _S279;
                int2 ci_3 = int2(_S303, _S305);
                if((t_hash2_0(ci_3 + _S277)) >= pres_0)
                {
                    break;
                }
                float h1_3 = t_hash2_0(ci_3 + int2(int(17), _S276 + int(5)));
                float2 _S306 = float2(0.5f) ;
                float2 q_8 = (x_0 - (c_6 + _S306 + (float2(h1_3, t_hash2_0(ci_3 + int2(_S276 + int(3), int(7)))) - _S306) * float2(0.80000001192092896f) )) / float2((rad_0 * mix(0.60000002384185791f, 1.3500000238418579f, t_hash2_0(ci_3 + int2(_S276 + int(9), int(31)))))) ;
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
                int2 _S307 = int2(_S278);
                int _S308 = _S307.x % _S279;
                int _S309 = (_S308 + _S279) % _S279;
                int _S310 = _S307.y % _S279;
                int _S311 = (_S310 + _S279) % _S279;
                int2 ci_4 = int2(_S309, _S311);
                if((t_hash2_0(ci_4 + _S277)) >= pres_0)
                {
                    break;
                }
                float h1_4 = t_hash2_0(ci_4 + int2(int(17), _S276 + int(5)));
                float2 _S312 = float2(0.5f) ;
                float2 q_9 = (x_0 - (_S278 + _S312 + (float2(h1_4, t_hash2_0(ci_4 + int2(_S276 + int(3), int(7)))) - _S312) * float2(0.80000001192092896f) )) / float2((rad_0 * mix(0.60000002384185791f, 1.3500000238418579f, t_hash2_0(ci_4 + int2(_S276 + int(9), int(31)))))) ;
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
                float2 c_7 = _S278 + float2(1.0f, 0.0f);
                int2 _S313 = int2(c_7);
                int _S314 = _S313.x % _S279;
                int _S315 = (_S314 + _S279) % _S279;
                int _S316 = _S313.y % _S279;
                int _S317 = (_S316 + _S279) % _S279;
                int2 ci_5 = int2(_S315, _S317);
                if((t_hash2_0(ci_5 + _S277)) >= pres_0)
                {
                    break;
                }
                float h1_5 = t_hash2_0(ci_5 + int2(int(17), _S276 + int(5)));
                float2 _S318 = float2(0.5f) ;
                float2 q_10 = (x_0 - (c_7 + _S318 + (float2(h1_5, t_hash2_0(ci_5 + int2(_S276 + int(3), int(7)))) - _S318) * float2(0.80000001192092896f) )) / float2((rad_0 * mix(0.60000002384185791f, 1.3500000238418579f, t_hash2_0(ci_5 + int2(_S276 + int(9), int(31)))))) ;
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
                float2 c_8 = _S278 + float2(-1.0f, 1.0f);
                int2 _S319 = int2(c_8);
                int _S320 = _S319.x % _S279;
                int _S321 = (_S320 + _S279) % _S279;
                int _S322 = _S319.y % _S279;
                int _S323 = (_S322 + _S279) % _S279;
                int2 ci_6 = int2(_S321, _S323);
                if((t_hash2_0(ci_6 + _S277)) >= pres_0)
                {
                    break;
                }
                float h1_6 = t_hash2_0(ci_6 + int2(int(17), _S276 + int(5)));
                float2 _S324 = float2(0.5f) ;
                float2 q_11 = (x_0 - (c_8 + _S324 + (float2(h1_6, t_hash2_0(ci_6 + int2(_S276 + int(3), int(7)))) - _S324) * float2(0.80000001192092896f) )) / float2((rad_0 * mix(0.60000002384185791f, 1.3500000238418579f, t_hash2_0(ci_6 + int2(_S276 + int(9), int(31)))))) ;
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
                float2 c_9 = _S278 + float2(0.0f, 1.0f);
                int2 _S325 = int2(c_9);
                int _S326 = _S325.x % _S279;
                int _S327 = (_S326 + _S279) % _S279;
                int _S328 = _S325.y % _S279;
                int _S329 = (_S328 + _S279) % _S279;
                int2 ci_7 = int2(_S327, _S329);
                if((t_hash2_0(ci_7 + _S277)) >= pres_0)
                {
                    break;
                }
                float h1_7 = t_hash2_0(ci_7 + int2(int(17), _S276 + int(5)));
                float2 _S330 = float2(0.5f) ;
                float2 q_12 = (x_0 - (c_9 + _S330 + (float2(h1_7, t_hash2_0(ci_7 + int2(_S276 + int(3), int(7)))) - _S330) * float2(0.80000001192092896f) )) / float2((rad_0 * mix(0.60000002384185791f, 1.3500000238418579f, t_hash2_0(ci_7 + int2(_S276 + int(9), int(31)))))) ;
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
                float2 c_10 = _S278 + float2(1.0f, 1.0f);
                int2 _S331 = int2(c_10);
                int _S332 = _S331.x % _S279;
                int _S333 = (_S332 + _S279) % _S279;
                int _S334 = _S331.y % _S279;
                int _S335 = (_S334 + _S279) % _S279;
                int2 ci_8 = int2(_S333, _S335);
                if((t_hash2_0(ci_8 + _S277)) >= pres_0)
                {
                    break;
                }
                float h1_8 = t_hash2_0(ci_8 + int2(int(17), _S276 + int(5)));
                float2 _S336 = float2(0.5f) ;
                float2 q_13 = (x_0 - (c_10 + _S336 + (float2(h1_8, t_hash2_0(ci_8 + int2(_S276 + int(3), int(7)))) - _S336) * float2(0.80000001192092896f) )) / float2((rad_0 * mix(0.60000002384185791f, 1.3500000238418579f, t_hash2_0(ci_8 + int2(_S276 + int(9), int(31)))))) ;
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

TForest_0 t_forest_0(uint pool_17, const TerrainDraw_natural_0 thread* d_19, float4 cv_1, float2 g_5, float2 w_7, float3 tu_4, float3 tv_5, float3 up_3, float3 n_4, float3 floor_alb_0, float3 l_3, float px_m_0, float px_iso_0, float spacing_1, float near_px_0, KernelContext_0 thread* kernelContext_23)
{
    float _S337 = t_noise_0(w_7 + float2(911.0f) , 64.0f);
    float _S338 = _S337 * 0.64999997615814209f;
    float _S339 = t_noise_0(w_7 + float2(271.0f) , 16.0f);
    float _S340 = cv_1.x;
    float c_11 = saturate(_S340 + (_S338 + _S339 * 0.34999999403953552f - 0.5f) * 1.6000000238418579f * _S340 * (1.0f - _S340));
    float _S341 = max(cv_1.y, 2.0f);
    float mx_3 = cv_1.z;
    float3 leaf_0 = mix(float3(0.05000000074505806f, 0.10999999940395355f, 0.024000000208616257f), float3(0.026000000536441803f, 0.064000003039836884f, 0.019999999552965164f), float3(mx_3) );
    float n1_1 = t_noise_0(w_7 + float2(151.0f) , 32.0f);
    float n2_1 = t_noise_0(w_7 + float2(517.0f) , 128.0f);
    float stand_0 = mix(0.80000001192092896f, 1.2000000476837158f, n1_1) * mix(0.87999999523162842f, 1.1200000047683716f, n2_1);
    float2 sun2_0 = float2(dot(l_3, tu_4), dot(l_3, tv_5));
    float2 toward_0 = sun2_0 / float2(max(dot(l_3, up_3), 0.059999998658895493f)) ;
    float _S342 = max(_S341, 20.0f);
    float4 _S343 = t_canopy_bilin_0(pool_17, (as_type<uint>(((float4(d_19->tex_0) ).z))), g_5 * float2(4.0f)  + toward_0 * float2(_S342)  / float2(max(spacing_1, 0.0099999997764825821f)) , kernelContext_23);
    float page_sh_0 = _S343.x * smoothstep(0.40000000596046448f, 0.89999997615814209f, _S343.y / _S342);
    float3 fl_1 = mix(floor_alb_0, float3(0.05000000074505806f, 0.041999999433755875f, 0.029999999329447746f), float3((0.5f * c_11)) );
    float fl_ao_0 = mix(1.0f, 0.5f, c_11);
    float rad_1 = mix(0.41999998688697815f, 0.62000000476837158f, saturate(_S341 / 30.0f)) + 0.20000000298023224f * smoothstep(0.60000002384185791f, 1.0f, c_11);
    float pres_1 = saturate(c_11 / (3.1415901184082031f * rad_1 * rad_1));
    float pb_0 = pres_1 * (1.0f - mx_3);
    float pc_0 = pres_1 * mx_3;
    float _S344 = mix(8.0f, 5.119999885559082f, mx_3) * rad_1 * 2.0f;
    float crown_px_0 = _S344 / max(px_m_0, 0.0010000000474974513f);
    float det_1 = smoothstep(1.5f, 4.0f, crown_px_0);
    float _S345 = smoothstep(80.0f, 40.0f, _S344 / max(px_iso_0, 0.0010000000474974513f));
    bool near_on_0 = near_px_0 >= 0.0f;
    float paint_0;
    if(near_on_0)
    {
        paint_0 = 1.0f - smoothstep(12.0f, 15.0f, near_px_0);
    }
    else
    {
        paint_0 = _S345;
    }
    float3 _S346 = float3(stand_0) ;
    float floor_sun_far_0 = 1.0f - 0.92000001668930054f * max(c_11, page_sh_0);
    float ao_far_0 = mix(fl_ao_0, 0.85000002384185791f, c_11);
    float sun_far_0 = mix(floor_sun_far_0, 1.0f, c_11);
    thread TForest_0 o_1;
    (&o_1)->albedo_2 = mix(fl_1, leaf_0 * float3(0.81999999284744263f)  * _S346, float3(c_11) );
    (&o_1)->n_3 = n_4;
    (&o_1)->ao_3 = ao_far_0;
    (&o_1)->sun_1 = sun_far_0;
    (&o_1)->crown_0 = c_11;
    bool _S347;
    if(det_1 > 0.0f)
    {
        if(!near_on_0)
        {
            _S347 = true;
        }
        else
        {
            _S347 = paint_0 > 0.0f;
        }
    }
    else
    {
        _S347 = false;
    }
    if(_S347)
    {
        bool _S348 = pb_0 > 0.0f;
        float4 kb_0;
        if(_S348)
        {
            float4 _S349 = t_crown_0(w_7, 8.0f, pb_0, rad_1, 3U);
            kb_0 = _S349;
        }
        else
        {
            kb_0 = float4(0.0f, 0.0f, 9.0f, 0.0f);
        }
        bool _S350 = pc_0 > 0.0f;
        float4 kc_0;
        if(_S350)
        {
            float4 _S351 = t_crown_0(w_7 + float2(1000.0f) , 5.119999885559082f, pc_0, rad_1 * 0.89999997615814209f, 11U);
            kc_0 = _S351;
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
        float2 _S352 = k_7.xy;
        float _S353 = min(dot(_S352, _S352), 1.0f);
        float3 nc_0;
        if(conifer_0)
        {
            nc_0 = normalize(up_3 + t2_0 * float3(1.2999999523162842f) );
        }
        else
        {
            nc_0 = normalize(t2_0 + up_3 * float3(sqrt(max(1.0f - _S353, 0.039999999105930328f))) );
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
        float _S354 = k_7.w;
        float3 la_0 = nc_0 * float3(mix(0.75f, 1.25f, _S354))  * _S346 * mix(float3(1.1200000047683716f, 1.0f, 0.75f), float3(0.85000002384185791f, 0.97000002861022949f, 1.2000000476837158f), float3(fract(_S354 * 7.309999942779541f)) );
        float gap_sun_0;
        if(conifer_0)
        {
            gap_sun_0 = 1.6000000238418579f;
        }
        else
        {
            gap_sun_0 = 1.0f;
        }
        float _S355 = t_noise_0(w_7 * float2(gap_sun_0)  + float2(13.0f) , 1.0f);
        float3 la_1 = la_0 * float3(mix(1.0f, mix(0.69999998807907104f, 1.25f, _S355), smoothstep(6.0f, 16.0f, crown_px_0))) ;
        float crown_ao_0 = mix(0.69999998807907104f, 1.0f, 1.0f - _S353) * mix(1.0f, 0.69999998807907104f, (1.0f - saturate(0.5f + 0.80000001192092896f * dot(_S352, sun2_0 / float2(max(length(sun2_0), 0.0010000000474974513f)) ))) * 0.60000002384185791f);
        if(crown_px_0 > 3.0f)
        {
            float2 wt_1 = w_7 + toward_0 * float2(_S341)  * float2(0.55000001192092896f) ;
            float4 sb_0;
            if(_S348)
            {
                float4 _S356 = t_crown_0(wt_1, 8.0f, pb_0, rad_1, 3U);
                sb_0 = _S356;
            }
            else
            {
                sb_0 = float4(0.0f, 0.0f, 9.0f, 0.0f);
            }
            float4 sc_3;
            if(_S350)
            {
                float4 _S357 = t_crown_0(wt_1 + float2(1000.0f) , 5.119999885559082f, pc_0, rad_1 * 0.89999997615814209f, 11U);
                sc_3 = _S357;
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
        float _S358 = min(gap_sun_0, 1.0f - 0.89999997615814209f * page_sh_0);
        float closed_0 = smoothstep(0.5f, 0.94999998807907104f, c_11);
        float3 _S359 = float3(in_crown_0) ;
        float3 n_near_0 = normalize(mix(n_4, nn_0, _S359));
        float ao_near_0 = mix(mix(fl_ao_0, 0.75f, closed_0), crown_ao_0, in_crown_0);
        float sun_near_0 = mix(mix(_S358, max(_S358, 0.34999999403953552f), closed_0), 1.0f, in_crown_0);
        float3 _S360 = float3(det_1) ;
        (&o_1)->albedo_2 = mix((&o_1)->albedo_2, mix(mix(fl_1, leaf_0 * _S346 * float3(0.85000002384185791f) , float3(closed_0) ), la_1, _S359), _S360);
        (&o_1)->n_3 = normalize(mix((&o_1)->n_3, n_near_0, _S360));
        (&o_1)->ao_3 = mix((&o_1)->ao_3, ao_near_0, det_1);
        (&o_1)->sun_1 = mix((&o_1)->sun_1, sun_near_0, det_1);
        (&o_1)->crown_0 = mix((&o_1)->crown_0, in_crown_0, det_1);
    }
    if(paint_0 < 1.0f)
    {
        float _S361 = t_noise_0(w_7 + float2(77.0f) , 4.0f);
        float _S362 = _S361 * 0.60000002384185791f;
        float _S363 = t_noise_0(w_7 + float2(33.0f) , 1.0f);
        float _S364 = min(mix(1.0f, 0.40000000596046448f + 0.60000002384185791f * smoothstep(0.34999999403953552f, 0.75f, _S362 + _S363 * 0.40000000596046448f), c_11), 1.0f - 0.60000002384185791f * page_sh_0);
        float3 _S365 = float3(paint_0) ;
        (&o_1)->albedo_2 = mix(fl_1, (&o_1)->albedo_2, _S365);
        (&o_1)->n_3 = normalize(mix(n_4, (&o_1)->n_3, _S365));
        float ao_crown_0 = (&o_1)->ao_3;
        float sun_crown_0 = (&o_1)->sun_1;
        (&o_1)->ao_3 = mix(max(fl_ao_0, 0.75f), (&o_1)->ao_3, paint_0);
        (&o_1)->sun_1 = mix(_S364, (&o_1)->sun_1, paint_0);
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
    float _S366 = s_10.z;
    bool _S367;
    if(_S366 < 0.0f)
    {
        _S367 = true;
    }
    else
    {
        _S367 = _S366 > 1.0f;
    }
    float _S368;
    if(_S367)
    {
        _S368 = 1.0e+09f;
    }
    else
    {
        _S368 = max(abs(s_10.x), abs(s_10.y));
    }
    return _S368;
}

float pcf_radius_texels_0(FrameData_natural_0 device* f_5, float texel_0)
{
    return clamp((float4((&f_5->light_0)->sky_0) ).z / texel_0, 1.0f, max((float4((&f_5->light_0)->sky_0) ).w, 1.0f));
}

float ed_depth_read_0(uint i_15, uint2 p_9, KernelContext_0 thread* kernelContext_24)
{
    float _S369 = ((reinterpret_cast<device depth2d<float, access::sample>*>((kernelContext_24->ed_textures_0))[(i_15)].read((p_9))));
    return _S369;
}

float cascade_lookup_0(FrameData_natural_0 device* f_6, uint atlas_0, uint c_13, float3 p_10, float3 ng_0, float2 px_0, KernelContext_0 thread* kernelContext_25)
{
    float4 _S370 = float4((&(&f_6->light_0)->cascade_0)->data_3[c_13]) ;
    float tile_1 = (float4((&f_6->light_0)->cfg_0) ).y;
    float _S371 = _S370.x;
    float _S372 = pcf_radius_texels_0(f_6, _S371);
    float sp_0 = _S372 * 0.5f;
    float nol_0 = saturate(dot(ng_0, (float4(f_6->sun_dir_0) ).xyz));
    float4 s_11 = (((float4(p_10 + ng_0 * float3((_S371 * ((float4((&f_6->light_0)->filter_0) ).z + 2.0f * sqrt(1.0f - nol_0 * nol_0)))) , 1.0f)) * (matrix<float,int(4),int(4)> ((&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(0)][int(0)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(1)][int(0)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(2)][int(0)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(3)][int(0)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(0)][int(1)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(1)][int(1)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(2)][int(1)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(3)][int(1)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(0)][int(2)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(1)][int(2)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(2)][int(2)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(3)][int(2)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(0)][int(3)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(1)][int(3)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(2)][int(3)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(3)][int(3)]))));
    float2 _S373 = float2(s_11.x * 0.5f + 0.5f, 0.5f - s_11.y * 0.5f) * float2(tile_1) ;
    float ign_0 = fract(52.982917785644531f * fract(dot(px_0, float2(0.067110560834407806f, 0.0058371499180793762f))));
    float2 _S374 = float2(0.5f) ;
    float2 _S375 = (float2(ign_0, fract(ign_0 * 7.309999942779541f + 0.37000000476837158f)) - _S374) * float2(0.0f) ;
    float3 ns_1 = (((float4(ng_0, 0.0f)) * (matrix<float,int(4),int(4)> ((&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(0)][int(0)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(1)][int(0)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(2)][int(0)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(3)][int(0)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(0)][int(1)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(1)][int(1)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(2)][int(1)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(3)][int(1)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(0)][int(2)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(1)][int(2)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(2)][int(2)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(3)][int(2)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(0)][int(3)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(1)][int(3)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(2)][int(3)], (&(&f_6->light_0)->shadow_1)->data_2[c_13].data_0[int(3)][int(3)])))).xyz;
    float rl_0 = _S371 * tile_1 * 0.5f;
    float _S376 = _S370.y;
    float _S377 = ns_1.z * _S376;
    float izn_0 = 1.0f / max(abs(_S377), 0.11999999731779099f);
    float _S378 = float((int(sign((_S377)))));
    float gx_1 = clamp(- (ns_1.x * rl_0) * izn_0 * _S378, -8.0f, 8.0f);
    float gy_1 = clamp(- (ns_1.y * rl_0) * izn_0 * _S378, -8.0f, 8.0f);
    float _S379 = s_11.z + ((float4((&f_6->light_0)->filter_0) ).w + 0.5f * (abs(gx_1) + abs(gy_1))) * _S371 / _S376;
    float _S380 = _S371 / _S376;
    int2 _S381 = int2(_S370.zw);
    int2 _S382 = _S381 + int2((int(tile_1) - int(1))) ;
    float _S383 = 1.0f / (_S372 + sp_0);
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
        float _S384 = 1.0f - abs(oy_0) * _S383;
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
            float w_8 = (1.0f - abs(ox_0) * _S383) * _S384;
            float2 tp_1 = floor(_S373 + float2(ox_0, oy_0) + _S375);
            float2 d_20 = tp_1 + _S374 - _S373;
            float _S385 = ed_depth_read_0(atlas_0, uint2(clamp(_S381 + int2(tp_1), _S381, _S382)), kernelContext_25);
            float _S386;
            if((_S379 + (gx_1 * d_20.x - gy_1 * d_20.y) * _S380) >= _S385)
            {
                _S386 = 1.0f;
            }
            else
            {
                _S386 = 0.0f;
            }
            float lit_1 = lit_0 + _S386 * w_8;
            float wsum_1 = wsum_0 + w_8;
            x_1 = x_1 + int(1);
            lit_0 = lit_1;
            wsum_0 = wsum_1;
        }
        y_0 = y_0 + int(1);
    }
    return lit_0 / wsum_0;
}

float sun_shadow_0(FrameData_natural_0 device* f_7, uint atlas_1, float3 p_11, float3 ng_1, float dist_1, float2 px_1, KernelContext_0 thread* kernelContext_26)
{
    uint n_5 = uint((float4((&f_7->light_0)->cfg_0) ).x);
    bool _S387;
    if(((uint4((&f_7->light_0)->flags_0) ).x) == 0U)
    {
        _S387 = true;
    }
    else
    {
        _S387 = n_5 == 0U;
    }
    if(_S387)
    {
        _S387 = true;
    }
    else
    {
        _S387 = dist_1 >= ((float4((&f_7->light_0)->filter_0) ).y);
    }
    if(_S387)
    {
        return 1.0f;
    }
    float _S388 = 1.0f - (float4((&f_7->light_0)->cfg_0) ).w * 2.0f / (float4((&f_7->light_0)->cfg_0) ).y;
    float _S389 = (float4((&f_7->light_0)->cfg_0) ).z;
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
        float _S390 = cascade_extent_0(f_7, c_14, p_11);
        float e_8 = _S390 / _S388;
        if(e_8 > 1.0f)
        {
            c_14 = c_14 + 1U;
            continue;
        }
        float _S391 = cascade_lookup_0(f_7, atlas_1, c_14, p_11, ng_1, px_1, kernelContext_26);
        float blend_0 = smoothstep(1.0f - _S389, 1.0f, e_8);
        float v_3;
        if(blend_0 > 0.0f)
        {
            uint _S392 = c_14 + 1U;
            if(_S392 < n_5)
            {
                float _S393 = cascade_extent_0(f_7, _S392, p_11);
                _S387 = (_S393 / _S388) <= 1.0f;
            }
            else
            {
                _S387 = false;
            }
            if(_S387)
            {
                float _S394 = cascade_lookup_0(f_7, atlas_1, _S392, p_11, ng_1, px_1, kernelContext_26);
                v_3 = _S394;
            }
            else
            {
                v_3 = 1.0f;
            }
            v_3 = mix(_S391, v_3, blend_0);
        }
        else
        {
            v_3 = _S391;
        }
        return mix(v_3, 1.0f, smoothstep(0.0f, 1.0f, saturate((dist_1 - (float4((&f_7->light_0)->filter_0) ).x) / max((float4((&f_7->light_0)->filter_0) ).y - (float4((&f_7->light_0)->filter_0) ).x, 0.0010000000474974513f))));
    }
    return 1.0f;
}

packed_float4 device* ed_buf_3(uint i_16, KernelContext_0 thread* kernelContext_27)
{
    return (packed_float4 device*)kernelContext_27->ed_buffers_0[i_16];
}

float2 ao_upsample_0(FrameData_natural_0 device* f_8, uint ao_buf_0, float2 pixel_1, float zlin_0, KernelContext_0 thread* kernelContext_28)
{
    float sv_0;
    if(((uint4((&f_8->light_0)->flags_0) ).y) == 0U)
    {
        return float2(1.0f) ;
    }
    packed_float4 device* _S395 = ed_buf_3(ao_buf_0, kernelContext_28);
    uint _S396 = uint((float4((&f_8->light_0)->ao_1) ).y);
    uint _S397 = uint((float4((&f_8->light_0)->ao_1) ).z);
    float2 _S398 = float2(0.5f) ;
    float2 h_10 = (pixel_1 - _S398) * _S398;
    float2 b_6 = floor(h_10);
    float2 _S399 = h_10 - b_6;
    float2 _S400 = float2(0.0f) ;
    int y_1 = int(0);
    float2 sum_0 = _S400;
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
            int2 t_1 = clamp(int2(b_6) + int2(x_2, y_1), int2(int(0)) , int2(int(_S396) - int(1), int(_S397) - int(1)));
            float4 _S401 = float4(*(_S395 + (uint(t_1.y) * _S396 + uint(t_1.x)))) ;
            if(x_2 == int(0))
            {
                sv_0 = 1.0f - _S399.x;
            }
            else
            {
                sv_0 = _S399.x;
            }
            float _S402;
            if(y_1 == int(0))
            {
                _S402 = 1.0f - _S399.y;
            }
            else
            {
                _S402 = _S399.y;
            }
            float w_9 = max(sv_0 * _S402, 0.0010000000474974513f) * (1.0f / (0.0010000000474974513f + abs(_S401.z - zlin_0) / max(zlin_0, 0.0010000000474974513f) * 50.0f));
            float2 sum_1 = sum_0 + _S401.xy * float2(w_9) ;
            float wsum_3 = wsum_2 + w_9;
            x_2 = x_2 + int(1);
            sum_0 = sum_1;
            wsum_2 = wsum_3;
        }
        y_1 = y_1 + int(1);
    }
    float2 r_2;
    if(wsum_2 > 0.0f)
    {
        r_2 = sum_0 / float2(wsum_2) ;
    }
    else
    {
        r_2 = float2(1.0f) ;
    }
    if(((float4((&f_8->light_0)->skyvis_0) ).x) > 0.0f)
    {
        sv_0 = saturate(r_2.y);
    }
    else
    {
        sv_0 = 1.0f;
    }
    return float2(pow(saturate(r_2.x), (float4((&f_8->light_0)->ao_1) ).x), sv_0);
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
    float r_3 = rb_1 + h_12;
    float _S403 = r_3 * mu_1;
    return max(- r_3 * mu_1 + sqrt(max(_S403 * _S403 + (rt_0 - rb_1 - h_12) * (rt_0 + r_3), 0.0f)), 0.0f);
}

float2 atm_trans_uv_0(float rb_2, float rt_1, float h_13, float mu_2)
{
    float ht_0 = rt_1 - rb_2;
    float _S404 = clamp(h_13, 0.0f, ht_0);
    float big_h_0 = sqrt(ht_0 * (rt_1 + rb_2));
    float rho_0 = sqrt(max(_S404 * (2.0f * rb_2 + _S404), 0.0f));
    float dmin_0 = ht_0 - _S404;
    return float2((atm_dist_top_0(rb_2, rt_1, _S404, mu_2) - dmin_0) / max(rho_0 + big_h_0 - dmin_0, 9.99999997475242708e-07f), rho_0 / big_h_0);
}

float4 atm_bilinear_0(packed_float4 device* buf_0, uint w_10, uint row0_0, uint rows_0, float2 uv_1)
{
    float2 p_12 = saturate(uv_1) * float2(float(w_10 - 1U), float(rows_0 - 1U));
    uint2 _S405 = min(uint2(p_12), uint2(w_10 - 2U, rows_0 - 2U));
    float2 g_6 = p_12 - float2(_S405);
    uint b_7 = (row0_0 + _S405.y) * w_10 + _S405.x;
    float4 _S406 = float4(g_6.x) ;
    uint _S407 = b_7 + w_10;
    return mix(mix(float4(*(buf_0 + b_7)) , float4(*(buf_0 + (b_7 + 1U))) , _S406), mix(float4(*(buf_0 + _S407)) , float4(*(buf_0 + (_S407 + 1U))) , _S406), float4(g_6.y) );
}

float3 atm_transmittance_0(FrameData_natural_0 device* f_12, float h_14, float mu_3, KernelContext_0 thread* kernelContext_29)
{
    float _S408 = atm_rb_0(f_12);
    float _S409 = atm_rt_0(f_12);
    float2 uv_2 = atm_trans_uv_0(_S408, _S409, h_14, mu_3);
    packed_float4 device* _S410 = ed_buf_3((uint4((&f_12->atmos_0)->bufs_0) ).x, kernelContext_29);
    float4 _S411 = atm_bilinear_0(_S410, 256U, 0U, 64U, uv_2);
    return _S411.xyz;
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
    float _S412 = atm_rb_1(f_14);
    float ch_0 = atm_cos_horizon_0(_S412, h_16);
    float s_12 = 0.0046999999321997166f * sqrt(max(1.0f - ch_0 * ch_0, 0.0f)) + 9.9999997473787516e-06f;
    return smoothstep(- s_12, s_12, mu_s_0 - ch_0);
}

float3 atm_sun_transmittance_0(FrameData_natural_0 device* f_15, float h_17, float mu_s_1, KernelContext_0 thread* kernelContext_30)
{
    float3 _S413 = atm_transmittance_0(f_15, h_17, mu_s_1, kernelContext_30);
    float _S414 = atm_earth_shadow_0(f_15, h_17, mu_s_1);
    return _S413 * float3(_S414) ;
}

float3 atmos_sun_at_0(FrameData_natural_0 device* f_16, float3 p_13, KernelContext_0 thread* kernelContext_31)
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
    float _S415 = atm_rb_0(f_16);
    float h0_0 = (float4((&f_16->atmos_0)->radii_0) ).z;
    float h_18 = atm_height_at_0(_S415, h0_0, dot(d_21, (float4((&f_16->atmos_0)->up_0) ).xyz), t_3);
    float3 _S416 = (float4((&f_16->atmos_0)->sun_0) ).xyz;
    float3 _S417 = atm_sun_transmittance_0(f_16, h_18, clamp(((_S415 + h0_0) * (float4((&f_16->atmos_0)->up_0) ).w + t_3 * dot(d_21, (float4(f_16->sun_dir_0) ).xyz)) / (_S415 + h_18), -1.0f, 1.0f), kernelContext_31);
    return _S416 * _S417;
}

float3 sun_light_at_0(FrameData_natural_0 device* f_17, float3 p_14, KernelContext_0 thread* kernelContext_32)
{
    bool _S418 = atmos_on_0(f_17);
    float3 _S419;
    if(_S418)
    {
        float3 _S420 = atmos_sun_at_0(f_17, p_14, kernelContext_32);
        _S419 = _S420;
    }
    else
    {
        _S419 = (float4(f_17->sun_color_0) ).xyz;
    }
    return _S419;
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
    float _S421 = 1.0f - a2_1;
    return 0.5f / max(nol_1 * sqrt(nov_0 * nov_0 * _S421 + a2_1) + nov_0 * sqrt(nol_1 * nol_1 * _S421 + a2_1), 9.9999997473787516e-06f);
}

float2 env_brdf_0(float nov_1, float rough_1)
{
    float4 r_4 = float4(rough_1)  * float4(-1.0f, -0.027499999850988388f, -0.57200002670288086f, 0.02199999988079071f) + float4(1.0f, 0.042500000447034836f, 1.0399999618530273f, -0.039999999105930328f);
    float _S422 = r_4.x;
    return float2(-1.0399999618530273f, 1.0399999618530273f) * float2((min(_S422 * _S422, exp2(-9.2799997329711914f * nov_1)) * _S422 + r_4.y))  + r_4.zw;
}

bool atmos_on_1(FrameData_natural_0 device* f_18)
{
    return ((uint4((&f_18->atmos_0)->cfg_1) ).x) != 0U;
}

float3 atm_sky_view_0(FrameData_natural_0 device* f_19, uint buf_1, uint w_11, uint rows_1, float h_19, float3 d_23, KernelContext_0 thread* kernelContext_33)
{
    float mu_4 = clamp(dot(d_23, (float4((&f_19->atmos_0)->up_0) ).xyz), -1.0f, 1.0f);
    float mu_s_2 = (float4((&f_19->atmos_0)->up_0) ).w;
    float nu_0 = dot(d_23, (float4(f_19->sun_dir_0) ).xyz);
    float _S423 = sqrt(max(1.0f - mu_4 * mu_4, 0.0f)) * sqrt(max(1.0f - mu_s_2 * mu_s_2, 0.0f));
    float cphi_0;
    if(_S423 > 9.99999997475242708e-07f)
    {
        cphi_0 = clamp((nu_0 - mu_4 * mu_s_2) / _S423, -1.0f, 1.0f);
    }
    else
    {
        cphi_0 = 1.0f;
    }
    float u_0 = sqrt(acos(cphi_0) / 3.1415927410125732f);
    float _S424 = atm_rb_0(f_19);
    float zha_0 = acos(atm_cos_horizon_0(_S424, h_19));
    float theta_0 = acos(mu_4);
    uint half_rows_0 = rows_1 / 2U;
    float c_15;
    uint row0_1;
    if(theta_0 < zha_0)
    {
        float _S425 = 1.0f - sqrt(saturate(1.0f - theta_0 / zha_0));
        row0_1 = 0U;
        c_15 = _S425;
    }
    else
    {
        float _S426 = sqrt(saturate((theta_0 - zha_0) / max(3.1415927410125732f - zha_0, 9.99999997475242708e-07f)));
        row0_1 = half_rows_0;
        c_15 = _S426;
    }
    packed_float4 device* _S427 = ed_buf_3(buf_1, kernelContext_33);
    float4 _S428 = atm_bilinear_0(_S427, w_11, row0_1, half_rows_0, float2(u_0, c_15));
    return _S428.xyz;
}

float3 atmos_sky_0(FrameData_natural_0 device* f_20, float3 d_24, KernelContext_0 thread* kernelContext_34)
{
    float3 _S429 = atm_sky_view_0(f_20, (uint4((&f_20->atmos_0)->bufs_0) ).z, 192U, 108U, (float4((&f_20->atmos_0)->radii_0) ).z, d_24, kernelContext_34);
    return _S429;
}

float3 sky_rad_0(FrameData_natural_0 device* f_21, float3 d_25, float disc_k_0)
{
    float3 sun_2 = (float4(f_21->sun_dir_0) ).xyz;
    float h_20 = d_25.y;
    float t_4 = pow(saturate(1.0f - max(h_20, 0.0f)), 6.5f);
    float mu_5 = dot(d_25, sun_2);
    float warm_0 = saturate(1.0f - sun_2.y * 2.5f);
    float3 c_16 = mix((float4(f_21->sky_zenith_0) ).xyz, (float4(f_21->sky_horizon_0) ).xyz, float3(t_4) ) + float3(1.0f, 0.41999998688697815f, 0.14000000059604645f) * float3(pow(saturate(mu_5 * 0.5f + 0.5f), 5.0f))  * float3(warm_0)  * float3(0.85000002384185791f)  * float3((0.40000000596046448f + t_4)) ;
    float3 _S430 = float3((float4(f_21->sun_dir_0) ).w) ;
    return mix(mix(c_16, c_16 * float3(1.0f, 0.69999998807907104f, 0.55000001192092896f), float3((warm_0 * 0.40000000596046448f)) ) + float3(1.0f, 0.89999997615814209f, 0.69999998807907104f) * float3(pow(saturate(mu_5), 28.0f))  * float3(0.34999999403953552f) , (float4(f_21->ground_color_0) ).xyz, float3(saturate(- h_20 * 6.0f)) ) * _S430 + float3(1.0f, 0.94999998807907104f, 0.85000002384185791f) * float3((smoothstep(0.99965000152587891f, 0.99984997510910034f, mu_5) * step(0.0f, h_20)))  * float3(60.0f)  * _S430 * float3(disc_k_0) ;
}

float3 sky_env_0(FrameData_natural_0 device* f_22, float3 d_26, KernelContext_0 thread* kernelContext_35)
{
    bool _S431 = atmos_on_1(f_22);
    float3 _S432;
    if(_S431)
    {
        float3 _S433 = atmos_sky_0(f_22, d_26, kernelContext_35);
        _S432 = _S433;
    }
    else
    {
        float3 _S434 = sky_rad_0(f_22, d_26, 0.0f);
        _S432 = _S434;
    }
    return _S432;
}

float3 sh_irradiance_0(FrameData_natural_0 device* f_23, float3 n_6, KernelContext_0 thread* kernelContext_36)
{
    thread array<float4, int(9)> s_13;
    packed_float4 device* _S435 = ed_buf_3((uint4((&f_23->atmos_0)->bufs2_0) ).z, kernelContext_36);
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
        bool _S436 = atmos_on_0(f_23);
        float4 _S437;
        if(_S436)
        {
            _S437 = float4(*(_S435 + (288U + k_8))) ;
        }
        else
        {
            _S437 = float4((&(&f_23->light_0)->sh_0)->data_4[k_8]) ;
        }
        s_13[k_8] = _S437;
        k_8 = k_8 + 1U;
    }
    float _S438 = n_6.y;
    float _S439 = n_6.z;
    float _S440 = n_6.x;
    return max(s_13[int(0)].xyz + (s_13[int(1)].xyz * float3(_S438)  + s_13[int(2)].xyz * float3(_S439)  + s_13[int(3)].xyz * float3(_S440) ) + (s_13[int(4)].xyz * float3((_S440 * _S438))  + s_13[int(5)].xyz * float3((_S438 * _S439))  + s_13[int(6)].xyz * float3((3.0f * _S439 * _S439 - 1.0f)) ) + (s_13[int(7)].xyz * float3((_S440 * _S439))  + s_13[int(8)].xyz * float3((_S440 * _S440 - _S438 * _S438)) ), float3(0.0f) );
}

float3 hemi_ambient_0(FrameData_natural_0 device* f_24, float3 n_7)
{
    return mix((float4(f_24->ground_color_0) ).xyz * float3((float4(f_24->sun_dir_0) ).w) , mix((float4(f_24->sky_horizon_0) ).xyz, (float4(f_24->sky_zenith_0) ).xyz, float3(0.5f) ) * float3((float4(f_24->sun_dir_0) ).w) , float3((n_7.y * 0.5f + 0.5f)) );
}

float3 ambient_light_0(FrameData_natural_0 device* f_25, float3 n_8, KernelContext_0 thread* kernelContext_37)
{
    float3 _S441;
    if(((uint4((&f_25->light_0)->flags_0) ).z) != 0U)
    {
        float3 _S442 = sh_irradiance_0(f_25, n_8, kernelContext_37);
        _S441 = _S442;
    }
    else
    {
        float3 _S443 = hemi_ambient_0(f_25, n_8);
        _S441 = _S443;
    }
    return _S441;
}

float3 occluded_ambient_0(FrameData_natural_0 device* f_26, float3 open_0, float sv_1, KernelContext_0 thread* kernelContext_38)
{
    float3 _S444 = ambient_light_0(f_26, float3(0.0f, -1.0f, 0.0f), kernelContext_38);
    return mix(_S444 * float3((float4((&f_26->light_0)->skyvis_0) ).y) , open_0, float3(sv_1) );
}

float3 ao_multibounce_0(FrameData_natural_0 device* f_27, float ao_4, float3 albedo_3)
{
    float3 _S445 = float3(ao_4) ;
    return mix(_S445, max(_S445, ((_S445 * (float3(2.0404000282287598f)  * albedo_3 - float3(0.33239999413490295f) ) + (float3(-4.795100212097168f)  * albedo_3 + float3(0.64170002937316895f) )) * _S445 + (float3(2.7551999092102051f)  * albedo_3 + float3(0.69029998779296875f) )) * _S445), float3((float4((&f_27->light_0)->sky_0) ).y) );
}

float3 shade_pbr_0(FrameData_natural_0 device* f_28, float3 sun_3, float3 albedo_4, float metal_0, float rough_2, float ao_5, float sv_2, float sun_vis_0, float3 n_9, float3 v_4, KernelContext_0 thread* kernelContext_39)
{
    float _S446 = saturate(metal_0);
    float _S447 = clamp(rough_2, 0.039999999105930328f, 1.0f);
    float a_14 = _S447 * _S447;
    float3 l_4 = (float4(f_28->sun_dir_0) ).xyz;
    float3 h_21 = normalize(l_4 + v_4);
    float nol_2 = saturate(dot(n_9, l_4));
    float _S448 = max(dot(n_9, v_4), 9.9999997473787516e-05f);
    float3 f0_1 = mix(float3(0.039999999105930328f) , albedo_4, float3(_S446) );
    float3 diff_col_0 = albedo_4 * float3((1.0f - _S446)) ;
    float3 F_0 = f_schlick_0(f0_1, saturate(dot(v_4, h_21)));
    float3 _S449 = float3(1.0f) ;
    float3 direct_0 = (diff_col_0 * (_S449 - F_0) / float3(3.1415927410125732f)  + float3((d_ggx_0(saturate(dot(n_9, h_21)), a_14) * v_smith_0(_S448, nol_2, a_14)))  * F_0) * sun_3 * float3((nol_2 * sun_vis_0)) ;
    float2 ab_0 = env_brdf_0(_S448, _S447);
    float3 r_5 = reflect(- v_4, n_9);
    float3 env_0;
    if(((uint4((&f_28->light_0)->flags_0) ).z) != 0U)
    {
        float3 _S450 = sky_env_0(f_28, r_5, kernelContext_39);
        float3 _S451 = sh_irradiance_0(f_28, r_5, kernelContext_39);
        env_0 = mix(_S450, _S451, float3(_S447) );
    }
    else
    {
        float3 _S452 = hemi_ambient_0(f_28, r_5);
        env_0 = _S452;
    }
    float3 _S453 = occluded_ambient_0(f_28, env_0, sv_2, kernelContext_39);
    float spec_occ_0 = saturate(pow(_S448 + ao_5, exp2(-16.0f * a_14 - 1.0f)) - 1.0f + ao_5);
    float3 f_env_0 = f0_1 * float3(ab_0.x)  + float3(ab_0.y) ;
    float3 _S454 = diff_col_0 * (_S449 - f_env_0);
    float3 _S455 = ambient_light_0(f_28, n_9, kernelContext_39);
    float3 _S456 = occluded_ambient_0(f_28, _S455, sv_2, kernelContext_39);
    float3 _S457 = _S454 * _S456;
    float3 _S458 = ao_multibounce_0(f_28, ao_5, diff_col_0);
    return direct_0 + (_S457 * _S458 + _S453 * f_env_0 * float3(spec_occ_0) );
}

float atm_enter_top_0(float rb_4, float rt_2, float h_22, float mu_6)
{
    float r_6 = rb_4 + h_22;
    float _S459 = r_6 * mu_6;
    float d_27 = _S459 * _S459 - (h_22 - (rt_2 - rb_4)) * (r_6 + rt_2);
    if(d_27 < 0.0f)
    {
        return -1.0f;
    }
    float t_5 = - r_6 * mu_6 - sqrt(d_27);
    float _S460;
    if(t_5 >= 0.0f)
    {
        _S460 = t_5;
    }
    else
    {
        _S460 = -1.0f;
    }
    return _S460;
}

float atm_ap_slice_0(FrameData_natural_0 device* f_29, float t_6)
{
    float n_10 = (float4((&f_29->atmos_0)->ap_0) ).x;
    return log2(1.0f + max(t_6, 0.0f) / n_10) / log2(1.0f + (float4((&f_29->atmos_0)->ap_0) ).y / n_10) * 32.0f;
}

void atm_froxel_xy_0(packed_float4 device* l_5, packed_float4 device* t_7, float2 g_7, uint2 i_17, uint k_9, float3 thread* lo_1, float3 thread* to_0)
{
    uint b_8 = (k_9 * 32U + i_17.y) * 32U + i_17.x;
    uint _S461 = b_8 + 1U;
    float4 _S462 = float4(g_7.x) ;
    uint _S463 = b_8 + 32U;
    uint _S464 = _S463 + 1U;
    float4 t0_0 = mix(float4(*(t_7 + b_8)) , float4(*(t_7 + _S461)) , _S462);
    float4 t1_0 = mix(float4(*(t_7 + _S463)) , float4(*(t_7 + _S464)) , _S462);
    float4 _S465 = float4(g_7.y) ;
    *lo_1 = mix(mix(float4(*(l_5 + b_8)) , float4(*(l_5 + _S461)) , _S462), mix(float4(*(l_5 + _S463)) , float4(*(l_5 + _S464)) , _S462), _S465).xyz;
    *to_0 = mix(t0_0, t1_0, _S465).xyz;
    return;
}

void atmos_aerial_0(FrameData_natural_0 device* f_30, float3 p_15, float2 px_2, float3 thread* l_6, float3 thread* tr_0, KernelContext_0 thread* kernelContext_40)
{
    *l_6 = float3(0.0f) ;
    float3 _S466 = float3(1.0f) ;
    *tr_0 = _S466;
    float t_8 = length(p_15) * 0.0010000000474974513f;
    if(t_8 <= 0.0f)
    {
        return;
    }
    float h_23 = (float4((&f_30->atmos_0)->radii_0) ).z;
    float _S467 = atm_rb_0(f_30);
    float _S468 = atm_rt_0(f_30);
    float te_0;
    if(h_23 > (_S468 - _S467))
    {
        float te_1 = atm_enter_top_0(_S467, _S468, h_23, dot(p_15 * float3((0.0010000000474974513f / t_8)) , (float4((&f_30->atmos_0)->up_0) ).xyz));
        bool _S469;
        if(te_1 < 0.0f)
        {
            _S469 = true;
        }
        else
        {
            _S469 = t_8 <= te_1;
        }
        if(_S469)
        {
            return;
        }
        te_0 = te_1;
    }
    else
    {
        te_0 = 0.0f;
    }
    float _S470 = atm_ap_slice_0(f_30, t_8 - te_0);
    float2 q_15 = clamp(px_2 / (float4(f_30->viewport_0) ).zw * float2(32.0f)  - float2(0.5f) , float2(0.0f) , float2(31.0f) );
    uint2 _S471 = min(uint2(q_15), uint2(30U) );
    float2 g_8 = q_15 - float2(_S471);
    packed_float4 device* _S472 = ed_buf_3((uint4((&f_30->atmos_0)->bufs2_0) ).x, kernelContext_40);
    packed_float4 device* _S473 = ed_buf_3((uint4((&f_30->atmos_0)->bufs2_0) ).y, kernelContext_40);
    thread float3 la_2;
    thread float3 ta_0;
    thread float3 lc_3;
    thread float3 tc_0;
    if(_S470 < 1.0f)
    {
        atm_froxel_xy_0(_S472, _S473, g_8, _S471, 0U, &lc_3, &tc_0);
        float3 _S474 = float3(_S470) ;
        *l_6 = lc_3 * _S474;
        *tr_0 = mix(_S466, tc_0, _S474);
        return;
    }
    float _S475 = min(_S470 - 1.0f, 31.0f);
    uint _S476 = min(uint(_S475), 30U);
    float gz_0 = _S475 - float(_S476);
    atm_froxel_xy_0(_S472, _S473, g_8, _S471, _S476, &la_2, &ta_0);
    atm_froxel_xy_0(_S472, _S473, g_8, _S471, _S476 + 1U, &lc_3, &tc_0);
    float3 _S477 = float3(gz_0) ;
    *l_6 = mix(la_2, lc_3, _S477);
    *tr_0 = mix(ta_0, tc_0, _S477);
    return;
}

float3 apply_aerial_0(FrameData_natural_0 device* f_31, float3 col_0, float3 p_16, float2 px_3, KernelContext_0 thread* kernelContext_41)
{
    bool _S478 = atmos_on_0(f_31);
    if(!_S478)
    {
        return col_0;
    }
    thread float3 l_7;
    thread float3 tr_1;
    atmos_aerial_0(f_31, p_16, px_3, &l_7, &tr_1, kernelContext_41);
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
    uint _S479 = uint((float4((&f_32->light_0)->cfg_0) ).x);
    bool _S480;
    if(((uint4((&f_32->light_0)->flags_0) ).x) == 0U)
    {
        _S480 = true;
    }
    else
    {
        _S480 = dist_2 >= ((float4((&f_32->light_0)->filter_0) ).y);
    }
    if(_S480)
    {
        return 4U;
    }
    float _S481 = 1.0f - (float4((&f_32->light_0)->cfg_0) ).w * 2.0f / (float4((&f_32->light_0)->cfg_0) ).y;
    uint c_17 = 0U;
    for(;;)
    {
        if(c_17 < _S479)
        {
        }
        else
        {
            break;
        }
        float _S482 = cascade_extent_0(f_32, c_17, p_17);
        if((_S482 / _S481) <= 1.0f)
        {
            return c_17;
        }
        c_17 = c_17 + 1U;
    }
    return 4U;
}

float4 terrain_pixel_0(FrameData_natural_0 device* f_33, const FsOut_0 thread* i_18, uint2 vis_1, float3 dir_8, KernelContext_0 thread* kernelContext_42)
{
    TerrainDraw_natural_0 device* _S483 = ed_buf_1(kernelContext_42->ed_c_0->t_draws_0, kernelContext_42);
    TerrainDraw_natural_0 device* _S484 = _S483 + ((vis_1.x) & 2147483647U);
    TerrainDraw_natural_0 d_28 = *_S484;
    thread TerrainDraw_natural_0 _S485 = *_S484;
    uint pool_18 = kernelContext_42->ed_c_0->t_pool_0;
    uint3 tri_0 = t_triangle_0((vis_1.y) & 268435455U);
    uint _S486 = tri_0.x;
    _S485 = d_28;
    float3 _S487 = t_vertex_0(pool_18, &_S485, _S486, kernelContext_42);
    uint _S488 = tri_0.y;
    float3 _S489 = t_vertex_0(pool_18, &_S485, _S488, kernelContext_42);
    float3 e1_1 = _S489 - _S487;
    uint _S490 = tri_0.z;
    float3 _S491 = t_vertex_0(pool_18, &_S485, _S490, kernelContext_42);
    float3 e2_1 = _S491 - _S487;
    float2 bb_0 = ray_bary_0(dir_8, _S487, e1_1, e2_1);
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
    float3 p_18 = _S487 + e1_1 * float3(b1_1)  + e2_1 * float3(b2_1) ;
    float2 g_9 = t_vertex_grid_0(_S486) * float2((1.0f - b1_1 - b2_1))  + t_vertex_grid_0(_S488) * float2(b1_1)  + t_vertex_grid_0(_S490) * float2(b2_1) ;
    float _S492 = t_sample_spacing_0(&_S485, g_9);
    float _S493 = max(_S492, 0.0099999997764825821f);
    int rn_0 = clamp(int(1.2000000476837158f / _S493 + 0.5f), int(1), int(4));
    thread float3 up_4;
    thread float h_24;
    float3 _S494 = t_smooth_normal_0(pool_18, &_S485, g_9, rn_0, &up_4, &h_24, kernelContext_42);
    int r_7 = clamp(int(24.0f / _S493 + 0.5f), int(1), int(6));
    float3 nm_1;
    if(r_7 == rn_0)
    {
        nm_1 = _S494;
    }
    else
    {
        float3 _S495 = t_smooth_normal_0(pool_18, &_S485, g_9, r_7, &up_4, &h_24, kernelContext_42);
        nm_1 = _S495;
    }
    float3 _S496 = terrain_albedo_0(h_24, dot(nm_1, up_4));
    uint _S497 = (uint4((&_S485)->slots_0) ).z;
    bool _S498 = (_S497 & 8388608U) != 0U;
    float4 _S499 = float4(0.0f) ;
    bool _S500;
    if(_S498)
    {
        _S500 = (as_type<uint>(((float4((&_S485)->tex_0) ).z))) != 0U;
    }
    else
    {
        _S500 = false;
    }
    float4 cv_2;
    if(_S500)
    {
        float4 _S501 = t_canopy_bilin_0(pool_18, (as_type<uint>(((float4((&_S485)->tex_0) ).z))), g_9 * float2(4.0f) , kernelContext_42);
        cv_2 = _S501;
    }
    else
    {
        cv_2 = _S499;
    }
    float rough_3;
    float mat_ao_0;
    float3 n_11;
    float3 albedo_5;
    if((kernelContext_42->ed_c_0->t_mat_0) != 0U)
    {
        TerrainMat_natural_0 device* _S502 = ed_buf_2(kernelContext_42->ed_c_0->t_draws_0, kernelContext_42);
        thread TerrainMat_natural_0 _S503 = *(_S502 + kernelContext_42->ed_c_0->t_mat_0);
        float4 _S504 = float4((&_S485)->shape_0) ;
        float2 w_12 = (float4((&_S485)->tex_0) ).xy + (g_9 - float2(32.0f) ) * float2((_S504.x * _S504.y)) ;
        float3 _S505 = (float4((&_S485)->fu_0) ).xyz;
        float3 tu_5 = normalize(_S505 - up_4 * float3(dot(_S505, up_4)) );
        float3 _S506 = (float4((&_S485)->fv_0) ).xyz;
        float3 tv_6 = normalize(_S506 - up_4 * float3(dot(_S506, up_4)) );
        float2 _S507 = i_18->pos_0.xy;
        float3 _S508 = view_dir_0(f_33, _S507 + float2(1.0f, 0.0f));
        float2 bx_0 = ray_bary_0(_S508, _S487, e1_1, e2_1);
        float3 _S509 = view_dir_0(f_33, _S507 + float2(0.0f, 1.0f));
        float2 by_0 = ray_bary_0(_S509, _S487, e1_1, e2_1);
        float3 dpx_0 = _S487 + e1_1 * float3(bx_0.x)  + e2_1 * float3(bx_0.y)  - p_18;
        float3 dpy_0 = _S487 + e1_1 * float3(by_0.x)  + e2_1 * float3(by_0.y)  - p_18;
        float2 dwx_1 = float2(dot(dpx_0, tu_5), dot(dpx_0, tv_6));
        float2 dwy_1 = float2(dot(dpy_0, tu_5), dot(dpy_0, tv_6));
        float2 sp_1 = g_9 * float2(4.0f) ;
        int rf_0 = clamp(int(2.0f / _S493 + 0.5f), int(1), int(4));
        float _S510 = float(clamp(int(8.0f / _S493 + 0.5f), int(1), int(16)));
        float2 _S511 = float2(_S510, 0.0f);
        float _S512 = t_page_bilin_0(pool_18, &_S485, sp_1 + _S511, kernelContext_42);
        float _S513 = t_page_bilin_0(pool_18, &_S485, sp_1 - _S511, kernelContext_42);
        float _S514 = _S512 + _S513;
        float2 _S515 = float2(0.0f, _S510);
        float _S516 = t_page_bilin_0(pool_18, &_S485, sp_1 + _S515, kernelContext_42);
        float _S517 = _S514 + _S516;
        float _S518 = t_page_bilin_0(pool_18, &_S485, sp_1 - _S515, kernelContext_42);
        float lap_0 = _S517 + _S518 - 4.0f * h_24;
        if((length_squared_0(p_18)) < 1.6e+05f)
        {
            float _S519 = float(rf_0);
            float2 _S520 = float2(_S519, 0.0f);
            float _S521 = t_page_bilin_0(pool_18, &_S485, sp_1 + _S520, kernelContext_42);
            float _S522 = t_page_bilin_0(pool_18, &_S485, sp_1 - _S520, kernelContext_42);
            float _S523 = _S521 + _S522;
            float2 _S524 = float2(0.0f, _S519);
            float _S525 = t_page_bilin_0(pool_18, &_S485, sp_1 + _S524, kernelContext_42);
            float _S526 = _S523 + _S525;
            float _S527 = t_page_bilin_0(pool_18, &_S485, sp_1 - _S524, kernelContext_42);
            rough_3 = _S526 + _S527 - 4.0f * h_24;
        }
        else
        {
            rough_3 = 0.0f;
        }
        float dl_0 = _S510 * _S493;
        float df_0 = float(rf_0) * _S493;
        TSurf_0 _S528 = terrain_material_0(f_33, &_S503, &_S485, _S494, nm_1, up_4, h_24, lap_0 / (dl_0 * dl_0), rough_3 / (df_0 * df_0), w_12, dwx_1, dwy_1, dot(dpx_0, up_4), dot(dpy_0, up_4), tu_5, tv_6, length(p_18), _S498, cv_2, _S493, kernelContext_42);
        n_11 = _S528.n_1;
        albedo_5 = _S528.albedo_1;
        rough_3 = _S528.rough_0;
        mat_ao_0 = _S528.ao_2;
    }
    else
    {
        n_11 = _S494;
        albedo_5 = _S496;
        rough_3 = 0.92000001668930054f;
        mat_ao_0 = 1.0f;
    }
    if(_S498)
    {
        _S500 = (cv_2.x) > 0.0040000001899898052f;
    }
    else
    {
        _S500 = false;
    }
    float sun_k_0;
    if(_S500)
    {
        float4 _S529 = float4((&_S485)->shape_0) ;
        float2 w_13 = (float4((&_S485)->tex_0) ).xy + (g_9 - float2(32.0f) ) * float2((_S529.x * _S529.y)) ;
        float3 _S530 = (float4((&_S485)->fu_0) ).xyz;
        float3 tu_6 = normalize(_S530 - up_4 * float3(dot(_S530, up_4)) );
        float3 _S531 = (float4((&_S485)->fv_0) ).xyz;
        float3 tv_7 = normalize(_S531 - up_4 * float3(dot(_S531, up_4)) );
        float3 _S532 = view_dir_0(f_33, i_18->pos_0.xy + float2(1.0f, 0.0f));
        float _S533 = length(p_18);
        float px_iso_1 = _S533 * length(_S532 - dir_8);
        float px_m_1 = px_iso_1 / sqrt(max(abs(dot(dir_8, n_11)), 0.029999999329447746f));
        if(((float4((&f_33->wind_1)->cfg_2) ).z) > 0.5f)
        {
            _S500 = ((float4((&_S485)->polar_0) ).w) > 0.0f;
        }
        else
        {
            _S500 = false;
        }
        if(_S500)
        {
            sun_k_0 = (float4((&_S485)->polar_0) ).w / max(_S533, 1.0f);
        }
        else
        {
            sun_k_0 = -1.0f;
        }
        TForest_0 _S534 = t_forest_0(pool_18, &_S485, cv_2, g_9, w_13, tu_6, tv_7, up_4, n_11, albedo_5, (float4(f_33->sun_dir_0) ).xyz, px_m_1, px_iso_1, _S493, sun_k_0, kernelContext_42);
        float _S535 = mix(rough_3, 0.80000001192092896f, _S534.crown_0);
        float mat_ao_1 = mat_ao_0 * _S534.ao_3;
        n_11 = _S534.n_3;
        sun_k_0 = _S534.sun_1;
        albedo_5 = _S534.albedo_2;
        rough_3 = _S535;
        mat_ao_0 = mat_ao_1;
    }
    else
    {
        sun_k_0 = 1.0f;
    }
    float3 v_5 = - dir_8;
    float _S536 = dot(n_11, v_5);
    if(_S536 < 0.0f)
    {
        n_11 = normalize(n_11 - float3((1.0099999904632568f * _S536))  * v_5);
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
    float2 _S537 = i_18->pos_0.xy;
    float _S538 = sun_shadow_0(f_33, kernelContext_42->ed_c_0->shadow_0, p_18, ng_3, zlin_1, _S537, kernelContext_42);
    float sun_vis_1;
    if(_S498)
    {
        sun_vis_1 = min(_S538, sun_k_0);
    }
    else
    {
        sun_vis_1 = _S538;
    }
    float2 _S539 = ao_upsample_0(f_33, kernelContext_42->ed_c_0->ao_0, _S537, zlin_1, kernelContext_42);
    float ao_6 = _S539.x * mat_ao_0;
    float sv_3 = _S539.y;
    float3 _S540 = sun_light_at_0(f_33, p_18, kernelContext_42);
    float3 _S541 = shade_pbr_0(f_33, _S540, albedo_5, 0.0f, rough_3, ao_6, sv_3, sun_vis_1, n_11, v_5, kernelContext_42);
    float3 _S542 = apply_aerial_0(f_33, _S541, p_18, _S537, kernelContext_42);
    float3 col_1;
    if(((uint4((&f_33->light_0)->flags_0) ).w) == 1U)
    {
        uint _S543 = shadow_cascade_0(f_33, p_18, zlin_1);
        col_1 = TINT_0[_S543] * float3((0.039999999105930328f + 0.5f * sun_vis_1)) ;
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
                col_1 = _S542;
            }
        }
    }
    if(_S498)
    {
        _S500 = (_S497 & 16777216U) != 0U;
    }
    else
    {
        _S500 = false;
    }
    if(_S500)
    {
        _S500 = (cv_2.x) > 0.0f;
    }
    else
    {
        _S500 = false;
    }
    if(_S500)
    {
        col_1 = mix(col_1, float3(saturate(cv_2.y / 40.0f), cv_2.x, cv_2.z) * float3(0.60000002384185791f) , float3(0.75f) );
    }
    return float4(col_1, 1.0f);
}

float3 t_target_even_1(uint pool_19, const TerrainDraw_natural_0 thread* d_29, uint i_19, uint j_10, KernelContext_0 thread* kernelContext_43)
{
    bool border_1;
    if(i_19 == 0U)
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
        border_1 = i_19 == 64U;
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
    uint4 _S544 = uint4(d_29->slots_0) ;
    uint _S545 = _S544.z;
    uint qx_1;
    if((_S545 & 16U) != 0U)
    {
        qx_1 = 128U;
    }
    else
    {
        qx_1 = 0U;
    }
    uint qy_1;
    if((_S545 & 32U) != 0U)
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
        float _S546 = t_own_h_0(pool_19, d_29, i_19, j_10, kernelContext_43);
        h_25 = _S546;
    }
    else
    {
        float4 _S547 = float4(d_29->hq_0) ;
        float _S548 = t_page_0(pool_19, _S544.y, qx_1 + 2U * i_19, qy_1 + 2U * j_10, _S547.z, _S547.w, kernelContext_43);
        h_25 = _S548;
    }
    thread float3 dir_9;
    float3 _S549 = t_lattice_0(d_29, i_19, j_10, h_25, &dir_9);
    return _S549;
}

float3 t_base_1(uint pool_20, const TerrainDraw_natural_0 thread* d_30, uint i_20, uint j_11, KernelContext_0 thread* kernelContext_44)
{
    float _S550 = t_own_h_0(pool_20, d_30, i_20, j_11, kernelContext_44);
    thread float3 dir_10;
    float3 _S551 = t_lattice_0(d_30, i_20, j_11, _S550, &dir_10);
    return _S551;
}

float3 t_coarse_even_1(uint pool_21, const TerrainDraw_natural_0 thread* d_31, uint e_9, uint2 a_15, uint2 b_9, uint2 c_18, KernelContext_0 thread* kernelContext_45)
{
    float3 _S552 = t_base_1(pool_21, d_31, a_15.x, a_15.y, kernelContext_45);
    if((e_9 & 3U) == 0U)
    {
        return _S552;
    }
    float m_5 = t_morph_0((float4(d_31->origin_0) ).xyz + _S552, (float4(d_31->morph_0) ).zw);
    float3 _S553 = t_base_1(pool_21, d_31, b_9.x, b_9.y, kernelContext_45);
    float3 _S554 = t_base_1(pool_21, d_31, c_18.x, c_18.y, kernelContext_45);
    return mix(_S552, float3(0.5f)  * (_S553 + _S554), float3(m_5) );
}

float3 t_target_1(uint pool_22, const TerrainDraw_natural_0 thread* d_32, uint i_21, uint j_12, KernelContext_0 thread* kernelContext_46)
{
    bool ei_1 = (i_21 & 1U) == 0U;
    bool ej_1 = (j_12 & 1U) == 0U;
    bool _S555;
    if(ei_1)
    {
        _S555 = ej_1;
    }
    else
    {
        _S555 = false;
    }
    if(_S555)
    {
        float3 _S556 = t_target_even_1(pool_22, d_32, i_21, j_12, kernelContext_46);
        return _S556;
    }
    if(!ei_1)
    {
        _S555 = ej_1;
    }
    else
    {
        _S555 = false;
    }
    if(_S555)
    {
        float3 _S557 = t_target_even_1(pool_22, d_32, i_21 - 1U, j_12, kernelContext_46);
        float3 _S558 = t_target_even_1(pool_22, d_32, i_21 + 1U, j_12, kernelContext_46);
        return float3(0.5f)  * (_S557 + _S558);
    }
    if(ei_1)
    {
        _S555 = !ej_1;
    }
    else
    {
        _S555 = false;
    }
    if(_S555)
    {
        float3 _S559 = t_target_even_1(pool_22, d_32, i_21, j_12 - 1U, kernelContext_46);
        float3 _S560 = t_target_even_1(pool_22, d_32, i_21, j_12 + 1U, kernelContext_46);
        return float3(0.5f)  * (_S559 + _S560);
    }
    float3 _S561 = t_target_even_1(pool_22, d_32, i_21 - 1U, j_12 - 1U, kernelContext_46);
    float3 _S562 = t_target_even_1(pool_22, d_32, i_21 + 1U, j_12 + 1U, kernelContext_46);
    return float3(0.5f)  * (_S561 + _S562);
}

float3 t_coarse_1(uint pool_23, const TerrainDraw_natural_0 thread* d_33, uint edge_3, uint e_10, KernelContext_0 thread* kernelContext_47)
{
    if((e_10 & 1U) == 0U)
    {
        float3 _S563 = t_coarse_even_1(pool_23, d_33, e_10, t_edge_at_0(edge_3, e_10), t_edge_at_0(edge_3, e_10 - 2U), t_edge_at_0(edge_3, e_10 + 2U), kernelContext_47);
        return _S563;
    }
    uint _S564 = e_10 - 1U;
    uint2 _S565 = t_edge_at_0(edge_3, _S564);
    uint _S566 = e_10 + 1U;
    uint2 _S567 = t_edge_at_0(edge_3, _S566);
    float3 _S568 = t_coarse_even_1(pool_23, d_33, _S564, _S565, t_edge_at_0(edge_3, e_10 - 3U), _S567, kernelContext_47);
    float3 _S569 = t_coarse_even_1(pool_23, d_33, _S566, _S567, _S565, t_edge_at_0(edge_3, e_10 + 3U), kernelContext_47);
    return float3(0.5f)  * (_S568 + _S569);
}

float3 t_grid_vertex_1(uint pool_24, const TerrainDraw_natural_0 thread* d_34, uint i_22, uint j_13, KernelContext_0 thread* kernelContext_48)
{
    uint fl_2 = (uint4(d_34->slots_0) ).z;
    uint edge_4;
    uint e_11;
    if(i_22 == 0U)
    {
        edge_4 = 0U;
        e_11 = j_13;
    }
    else
    {
        if(i_22 == 64U)
        {
            edge_4 = 1U;
            e_11 = j_13;
        }
        else
        {
            if(j_13 == 0U)
            {
                edge_4 = 2U;
                e_11 = i_22;
            }
            else
            {
                if(j_13 == 64U)
                {
                    edge_4 = 3U;
                    e_11 = i_22;
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
        bool _S570;
        if((fl_2 & (1U << edge_4)) != 0U)
        {
            _S570 = E_1 <= 2U;
        }
        else
        {
            _S570 = false;
        }
        if(_S570)
        {
            float3 _S571 = t_coarse_1(pool_24, d_34, edge_4, e_11, kernelContext_48);
            return _S571;
        }
        if(E_1 > 1U)
        {
            uint lo_2 = e_11 & (~(E_1 - 1U));
            if(lo_2 == e_11)
            {
                float3 _S572 = t_base_1(pool_24, d_34, i_22, j_13, kernelContext_48);
                return _S572;
            }
            uint2 a_16 = t_edge_at_0(edge_4, lo_2);
            uint2 b_10 = t_edge_at_0(edge_4, lo_2 + E_1);
            float3 _S573 = t_base_1(pool_24, d_34, a_16.x, a_16.y, kernelContext_48);
            float3 _S574 = t_base_1(pool_24, d_34, b_10.x, b_10.y, kernelContext_48);
            return mix(_S573, _S574, float3((float(e_11 - lo_2) / float(E_1))) );
        }
    }
    float3 _S575 = t_base_1(pool_24, d_34, i_22, j_13, kernelContext_48);
    if((fl_2 & 4194304U) != 0U)
    {
        return _S575;
    }
    float m_6 = t_morph_0((float4(d_34->origin_0) ).xyz + _S575, (float4(d_34->morph_0) ).xy);
    if(m_6 <= 0.0f)
    {
        return _S575;
    }
    float3 _S576 = t_target_1(pool_24, d_34, i_22, j_13, kernelContext_48);
    return mix(_S575, _S576, float3(m_6) );
}

float3 t_vertex_1(uint pool_25, const TerrainDraw_natural_0 thread* d_35, uint v_6, KernelContext_0 thread* kernelContext_49)
{
    if(v_6 < 4225U)
    {
        float3 _S577 = (float4(d_35->origin_0) ).xyz;
        float3 _S578 = t_grid_vertex_1(pool_25, d_35, v_6 % 65U, v_6 / 65U, kernelContext_49);
        return _S577 + _S578;
    }
    uint2 ij_1 = t_ring_0(v_6 - 4225U);
    float3 _S579 = t_grid_vertex_1(pool_25, d_35, ij_1.x, ij_1.y, kernelContext_49);
    float depth_1;
    if((((uint4(d_35->slots_0) ).z) & 64U) != 0U)
    {
        depth_1 = (float4(d_35->origin_0) ).w;
    }
    else
    {
        depth_1 = 0.0f;
    }
    float3 _S580 = (float4(d_35->origin_0) ).xyz;
    float3 _S581 = _S580 + _S579;
    float4 _S582 = float4(d_35->dir_c_0) ;
    return _S581 - normalize(_S581 - (_S580 - _S582.xyz * float3(_S582.w) )) * float3(depth_1) ;
}

constant array<float3, int(5)> TINT_1 = { { float3(1.0f, 0.25f, 0.25f), float3(0.25f, 1.0f, 0.25f), float3(0.30000001192092896f, 0.44999998807907104f, 1.0f), float3(1.0f, 1.0f, 0.25f), float3(0.60000002384185791f, 0.60000002384185791f, 0.60000002384185791f) } };
float4 terrain_pixel_1(FrameData_natural_0 device* f_34, const FsOut_0 thread* i_23, uint2 vis_2, float3 dir_11, KernelContext_0 thread* kernelContext_50)
{
    TerrainDraw_natural_0 device* _S583 = ed_buf_1(kernelContext_50->ed_c_0->t_draws_0, kernelContext_50);
    TerrainDraw_natural_0 device* _S584 = _S583 + ((vis_2.x) & 2147483647U);
    TerrainDraw_natural_0 d_36 = *_S584;
    thread TerrainDraw_natural_0 _S585 = *_S584;
    uint pool_26 = kernelContext_50->ed_c_0->t_pool_0;
    uint3 tri_1 = t_triangle_0((vis_2.y) & 268435455U);
    uint _S586 = tri_1.x;
    _S585 = d_36;
    float3 _S587 = t_vertex_1(pool_26, &_S585, _S586, kernelContext_50);
    uint _S588 = tri_1.y;
    float3 _S589 = t_vertex_1(pool_26, &_S585, _S588, kernelContext_50);
    float3 e1_2 = _S589 - _S587;
    uint _S590 = tri_1.z;
    float3 _S591 = t_vertex_1(pool_26, &_S585, _S590, kernelContext_50);
    float3 e2_2 = _S591 - _S587;
    float2 bb_1 = ray_bary_0(dir_11, _S587, e1_2, e2_2);
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
    float3 p_19 = _S587 + e1_2 * float3(b1_3)  + e2_2 * float3(b2_4) ;
    float2 g_10 = t_vertex_grid_0(_S586) * float2((1.0f - b1_3 - b2_4))  + t_vertex_grid_0(_S588) * float2(b1_3)  + t_vertex_grid_0(_S590) * float2(b2_4) ;
    float _S592 = t_sample_spacing_0(&_S585, g_10);
    float _S593 = max(_S592, 0.0099999997764825821f);
    int rn_1 = clamp(int(1.2000000476837158f / _S593 + 0.5f), int(1), int(4));
    thread float3 up_5;
    thread float h_26;
    float3 _S594 = t_smooth_normal_0(pool_26, &_S585, g_10, rn_1, &up_5, &h_26, kernelContext_50);
    int r_8 = clamp(int(24.0f / _S593 + 0.5f), int(1), int(6));
    float3 nm_2;
    if(r_8 == rn_1)
    {
        nm_2 = _S594;
    }
    else
    {
        float3 _S595 = t_smooth_normal_0(pool_26, &_S585, g_10, r_8, &up_5, &h_26, kernelContext_50);
        nm_2 = _S595;
    }
    float3 _S596 = terrain_albedo_0(h_26, dot(nm_2, up_5));
    float4 _S597 = float4(0.0f) ;
    float rough_4;
    float mat_ao_2;
    float3 n_12;
    float3 albedo_6;
    if((kernelContext_50->ed_c_0->t_mat_0) != 0U)
    {
        TerrainMat_natural_0 device* _S598 = ed_buf_2(kernelContext_50->ed_c_0->t_draws_0, kernelContext_50);
        thread TerrainMat_natural_0 _S599 = *(_S598 + kernelContext_50->ed_c_0->t_mat_0);
        float4 _S600 = float4((&_S585)->shape_0) ;
        float2 w_14 = (float4((&_S585)->tex_0) ).xy + (g_10 - float2(32.0f) ) * float2((_S600.x * _S600.y)) ;
        float3 _S601 = (float4((&_S585)->fu_0) ).xyz;
        float3 tu_7 = normalize(_S601 - up_5 * float3(dot(_S601, up_5)) );
        float3 _S602 = (float4((&_S585)->fv_0) ).xyz;
        float3 tv_8 = normalize(_S602 - up_5 * float3(dot(_S602, up_5)) );
        float2 _S603 = i_23->pos_0.xy;
        float3 _S604 = view_dir_0(f_34, _S603 + float2(1.0f, 0.0f));
        float2 bx_1 = ray_bary_0(_S604, _S587, e1_2, e2_2);
        float3 _S605 = view_dir_0(f_34, _S603 + float2(0.0f, 1.0f));
        float2 by_1 = ray_bary_0(_S605, _S587, e1_2, e2_2);
        float3 dpx_1 = _S587 + e1_2 * float3(bx_1.x)  + e2_2 * float3(bx_1.y)  - p_19;
        float3 dpy_1 = _S587 + e1_2 * float3(by_1.x)  + e2_2 * float3(by_1.y)  - p_19;
        float2 dwx_2 = float2(dot(dpx_1, tu_7), dot(dpx_1, tv_8));
        float2 dwy_2 = float2(dot(dpy_1, tu_7), dot(dpy_1, tv_8));
        float2 sp_2 = g_10 * float2(4.0f) ;
        int rf_1 = clamp(int(2.0f / _S593 + 0.5f), int(1), int(4));
        float _S606 = float(clamp(int(8.0f / _S593 + 0.5f), int(1), int(16)));
        float2 _S607 = float2(_S606, 0.0f);
        float _S608 = t_page_bilin_0(pool_26, &_S585, sp_2 + _S607, kernelContext_50);
        float _S609 = t_page_bilin_0(pool_26, &_S585, sp_2 - _S607, kernelContext_50);
        float _S610 = _S608 + _S609;
        float2 _S611 = float2(0.0f, _S606);
        float _S612 = t_page_bilin_0(pool_26, &_S585, sp_2 + _S611, kernelContext_50);
        float _S613 = _S610 + _S612;
        float _S614 = t_page_bilin_0(pool_26, &_S585, sp_2 - _S611, kernelContext_50);
        float lap_1 = _S613 + _S614 - 4.0f * h_26;
        if((length_squared_0(p_19)) < 1.6e+05f)
        {
            float _S615 = float(rf_1);
            float2 _S616 = float2(_S615, 0.0f);
            float _S617 = t_page_bilin_0(pool_26, &_S585, sp_2 + _S616, kernelContext_50);
            float _S618 = t_page_bilin_0(pool_26, &_S585, sp_2 - _S616, kernelContext_50);
            float _S619 = _S617 + _S618;
            float2 _S620 = float2(0.0f, _S615);
            float _S621 = t_page_bilin_0(pool_26, &_S585, sp_2 + _S620, kernelContext_50);
            float _S622 = _S619 + _S621;
            float _S623 = t_page_bilin_0(pool_26, &_S585, sp_2 - _S620, kernelContext_50);
            rough_4 = _S622 + _S623 - 4.0f * h_26;
        }
        else
        {
            rough_4 = 0.0f;
        }
        float dl_1 = _S606 * _S593;
        float df_1 = float(rf_1) * _S593;
        TSurf_0 _S624 = terrain_material_0(f_34, &_S599, &_S585, _S594, nm_2, up_5, h_26, lap_1 / (dl_1 * dl_1), rough_4 / (df_1 * df_1), w_14, dwx_2, dwy_2, dot(dpx_1, up_5), dot(dpy_1, up_5), tu_7, tv_8, length(p_19), false, _S597, _S593, kernelContext_50);
        n_12 = _S624.n_1;
        albedo_6 = _S624.albedo_1;
        rough_4 = _S624.rough_0;
        mat_ao_2 = _S624.ao_2;
    }
    else
    {
        n_12 = _S594;
        albedo_6 = _S596;
        rough_4 = 0.92000001668930054f;
        mat_ao_2 = 1.0f;
    }
    float3 v_7 = - dir_11;
    float _S625 = dot(n_12, v_7);
    if(_S625 < 0.0f)
    {
        n_12 = normalize(n_12 - float3((1.0099999904632568f * _S625))  * v_7);
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
    float2 _S626 = i_23->pos_0.xy;
    float _S627 = sun_shadow_0(f_34, kernelContext_50->ed_c_0->shadow_0, p_19, ng_5, zlin_2, _S626, kernelContext_50);
    float2 _S628 = ao_upsample_0(f_34, kernelContext_50->ed_c_0->ao_0, _S626, zlin_2, kernelContext_50);
    float ao_7 = _S628.x * mat_ao_2;
    float sv_4 = _S628.y;
    float3 _S629 = sun_light_at_0(f_34, p_19, kernelContext_50);
    float3 _S630 = shade_pbr_0(f_34, _S629, albedo_6, 0.0f, rough_4, ao_7, sv_4, _S627, n_12, v_7, kernelContext_50);
    float3 _S631 = apply_aerial_0(f_34, _S630, p_19, _S626, kernelContext_50);
    float3 col_2;
    if(((uint4((&f_34->light_0)->flags_0) ).w) == 1U)
    {
        uint _S632 = shadow_cascade_0(f_34, p_19, zlin_2);
        col_2 = TINT_1[_S632] * float3((0.039999999105930328f + 0.5f * _S627)) ;
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
                col_2 = _S631;
            }
        }
    }
    return float4(col_2, 1.0f);
}

constant array<float3, int(5)> TINT_2 = { { float3(1.0f, 0.25f, 0.25f), float3(0.25f, 1.0f, 0.25f), float3(0.30000001192092896f, 0.44999998807907104f, 1.0f), float3(1.0f, 1.0f, 0.25f), float3(0.60000002384185791f, 0.60000002384185791f, 0.60000002384185791f) } };
FrameData_natural_0 device* ed_buf_4(uint i_24, KernelContext_0 thread* kernelContext_51)
{
    return (FrameData_natural_0 device*)kernelContext_51->ed_buffers_0[i_24];
}

float3 sky_rad_1(FrameData_natural_0 device* f_35, float3 d_37, float disc_k_1)
{
    float3 sun_4 = (float4(f_35->sun_dir_0) ).xyz;
    float h_27 = d_37.y;
    float t_9 = pow(saturate(1.0f - max(h_27, 0.0f)), 6.5f);
    float mu_7 = dot(d_37, sun_4);
    float warm_1 = saturate(1.0f - sun_4.y * 2.5f);
    float3 c_19 = mix((float4(f_35->sky_zenith_0) ).xyz, (float4(f_35->sky_horizon_0) ).xyz, float3(t_9) ) + float3(1.0f, 0.41999998688697815f, 0.14000000059604645f) * float3(pow(saturate(mu_7 * 0.5f + 0.5f), 5.0f))  * float3(warm_1)  * float3(0.85000002384185791f)  * float3((0.40000000596046448f + t_9)) ;
    float3 _S633 = float3((float4(f_35->sun_dir_0) ).w) ;
    return mix(mix(c_19, c_19 * float3(1.0f, 0.69999998807907104f, 0.55000001192092896f), float3((warm_1 * 0.40000000596046448f)) ) + float3(1.0f, 0.89999997615814209f, 0.69999998807907104f) * float3(pow(saturate(mu_7), 28.0f))  * float3(0.34999999403953552f) , (float4(f_35->ground_color_0) ).xyz, float3(saturate(- h_27 * 6.0f)) ) * _S633 + float3(1.0f, 0.94999998807907104f, 0.85000002384185791f) * float3((smoothstep(0.99965000152587891f, 0.99984997510910034f, mu_7) * step(0.0f, h_27)))  * float3(60.0f)  * _S633 * float3(disc_k_1) ;
}

float atm_rt_1(FrameData_natural_0 device* f_36)
{
    return (float4((&f_36->atmos_0)->radii_0) ).y;
}

bool atm_enter_0(FrameData_natural_0 device* f_37, float thread* h_28, float thread* mu_8, float thread* mu_s_3, float nu_1, float thread* te_2)
{
    *te_2 = 0.0f;
    float _S634 = atm_rb_1(f_37);
    float _S635 = atm_rt_1(f_37);
    float _S636 = _S635 - _S634;
    if((*h_28) <= _S636)
    {
        return true;
    }
    float _S637 = atm_enter_top_0(_S634, _S635, *h_28, *mu_8);
    *te_2 = _S637;
    if(_S637 < 0.0f)
    {
        return false;
    }
    float r_9 = _S634 + *h_28;
    *mu_8 = clamp((r_9 * *mu_8 + *te_2) / _S635, -1.0f, 1.0f);
    *mu_s_3 = clamp((r_9 * *mu_s_3 + *te_2 * nu_1) / _S635, -1.0f, 1.0f);
    *h_28 = _S636;
    return true;
}

bool atm_hits_ground_0(float rb_5, float h_29, float mu_9)
{
    float rm_0 = (rb_5 + h_29) * mu_9;
    bool _S638;
    if(mu_9 < 0.0f)
    {
        _S638 = (rm_0 * rm_0) >= (h_29 * (2.0f * rb_5 + h_29));
    }
    else
    {
        _S638 = false;
    }
    return _S638;
}

float3 atm_transmittance_1(FrameData_natural_0 device* f_38, float h_30, float mu_10, KernelContext_0 thread* kernelContext_52)
{
    float _S639 = atm_rb_0(f_38);
    float _S640 = atm_rt_0(f_38);
    float2 uv_3 = atm_trans_uv_0(_S639, _S640, h_30, mu_10);
    packed_float4 device* _S641 = ed_buf_3((uint4((&f_38->atmos_0)->bufs_0) ).x, kernelContext_52);
    float4 _S642 = atm_bilinear_0(_S641, 256U, 0U, 64U, uv_3);
    return _S642.xyz;
}

float3 atmos_sky_1(FrameData_natural_0 device* f_39, float3 d_38, KernelContext_0 thread* kernelContext_53)
{
    float3 _S643 = atm_sky_view_0(f_39, (uint4((&f_39->atmos_0)->bufs_0) ).z, 192U, 108U, (float4((&f_39->atmos_0)->radii_0) ).z, d_38, kernelContext_53);
    return _S643;
}

float3 atmos_sky_pixel_0(FrameData_natural_0 device* f_40, float3 d_39, float disc_k_2, KernelContext_0 thread* kernelContext_54)
{
    float3 _S644 = atmos_sky_1(f_40, d_39, kernelContext_54);
    float nu_2 = dot(d_39, (float4(f_40->sun_dir_0) ).xyz);
    float cr_0 = (float4((&f_40->atmos_0)->sun_0) ).w;
    float _S645 = cr_0 - 1.99999999495048542e-06f;
    float3 c_20;
    if(nu_2 > _S645)
    {
        thread float h_31 = (float4((&f_40->atmos_0)->radii_0) ).z;
        thread float mu_11 = dot(d_39, (float4((&f_40->atmos_0)->up_0) ).xyz);
        thread float mu_s_4 = (float4((&f_40->atmos_0)->up_0) ).w;
        float3 _S646 = float3(1.0f) ;
        thread float te_3;
        bool _S647 = atm_enter_0(f_40, &h_31, &mu_11, &mu_s_4, nu_2, &te_3);
        if(_S647)
        {
            float _S648 = atm_rb_0(f_40);
            if(atm_hits_ground_0(_S648, h_31, mu_11))
            {
                c_20 = float3(0.0f) ;
            }
            else
            {
                float3 _S649 = atm_transmittance_1(f_40, h_31, mu_11, kernelContext_54);
                c_20 = _S649;
            }
        }
        else
        {
            c_20 = _S646;
        }
        float sr2_0 = 1.0f - cr_0 * cr_0;
        c_20 = _S644 + min((float4((&f_40->atmos_0)->sun_0) ).xyz * c_20 / float3((3.1415927410125732f * sr2_0))  * float3((1.0f - 0.60000002384185791f * (1.0f - sqrt(1.0f - saturate((1.0f - nu_2 * nu_2) / sr2_0)))))  * float3(smoothstep(_S645, cr_0 + 1.99999999495048542e-06f, nu_2))  * float3(disc_k_2) , float3(30000.0f) );
    }
    else
    {
        c_20 = _S644;
    }
    return c_20;
}

float3 sky_pixel_0(FrameData_natural_0 device* f_41, float3 d_40, KernelContext_0 thread* kernelContext_55)
{
    bool _S650 = atmos_on_0(f_41);
    if(!_S650)
    {
        float3 _S651 = sky_rad_1(f_41, d_40, (float4((&f_41->light_0)->sky_0) ).x);
        return _S651;
    }
    if(((uint4((&f_41->atmos_0)->cfg_1) ).y) == 1U)
    {
        thread float h_32 = (float4((&f_41->atmos_0)->radii_0) ).z;
        thread float mu_12 = dot(d_40, (float4((&f_41->atmos_0)->up_0) ).xyz);
        thread float mu_s_5 = (float4((&f_41->atmos_0)->up_0) ).w;
        thread float te_4;
        bool _S652 = atm_enter_0(f_41, &h_32, &mu_12, &mu_s_5, dot(d_40, (float4(f_41->sun_dir_0) ).xyz), &te_4);
        if(!_S652)
        {
            return float3(0.5f) ;
        }
        float _S653 = atm_rb_0(f_41);
        float3 _S654;
        if(atm_hits_ground_0(_S653, h_32, mu_12))
        {
            _S654 = float3(0.0f) ;
        }
        else
        {
            float3 _S655 = atm_transmittance_1(f_41, h_32, mu_12, kernelContext_55);
            _S654 = _S655 * float3(0.5f) ;
        }
        return _S654;
    }
    float _S656;
    if(((uint4((&f_41->atmos_0)->cfg_1) ).y) == 2U)
    {
        _S656 = 0.0f;
    }
    else
    {
        _S656 = (float4((&f_41->light_0)->sky_0) ).x;
    }
    float3 _S657 = atmos_sky_pixel_0(f_41, d_40, _S656, kernelContext_55);
    return _S657;
}

struct _Array_natural_vectorx3Cfloatx2C4x3E3_0
{
    array<packed_float4, int(3)> data_7;
};

struct GpuInstance_natural_0
{
    _Array_natural_vectorx3Cfloatx2C4x3E3_0 m_7;
    _Array_natural_vectorx3Cfloatx2C4x3E3_0 prev_0;
    uint mesh_0;
    uint material_0;
    uint object_lo_0;
    uint node_flags_0;
    array<uint, int(4)> pad_2;
};

GpuInstance_natural_0 device* ed_buf_5(uint i_25, KernelContext_0 thread* kernelContext_56)
{
    return (GpuInstance_natural_0 device*)kernelContext_56->ed_buffers_0[i_25];
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

GpuMesh_natural_0 device* ed_buf_6(uint i_26, KernelContext_0 thread* kernelContext_57)
{
    return (GpuMesh_natural_0 device*)kernelContext_57->ed_buffers_0[i_26];
}

struct Vtx_0
{
    float px_4;
    float py_0;
    float pz_0;
    uint n_13;
};

Vtx_0 device* ed_buf_7(uint i_27, KernelContext_0 thread* kernelContext_58)
{
    return (Vtx_0 device*)kernelContext_58->ed_buffers_0[i_27];
}

packed_uint2 device* ed_buf_8(uint i_28, KernelContext_0 thread* kernelContext_59)
{
    return (packed_uint2 device*)kernelContext_59->ed_buffers_0[i_28];
}

float3 xform_point_0(const GpuInstance_natural_0 thread* i_29, float3 p_20)
{
    float4 _S658 = float4((&i_29->m_7)->data_7[int(0)]) ;
    float4 _S659 = float4((&i_29->m_7)->data_7[int(1)]) ;
    float4 _S660 = float4((&i_29->m_7)->data_7[int(2)]) ;
    return float3(dot(_S658.xyz, p_20) + _S658.w, dot(_S659.xyz, p_20) + _S659.w, dot(_S660.xyz, p_20) + _S660.w);
}

float max_scale_0(const GpuInstance_natural_0 thread* i_30)
{
    float4 _S661 = float4((&i_30->m_7)->data_7[int(0)]) ;
    float4 _S662 = float4((&i_30->m_7)->data_7[int(1)]) ;
    float4 _S663 = float4((&i_30->m_7)->data_7[int(2)]) ;
    return sqrt(max(max(length_squared_0(float3(_S661.x, _S662.x, _S663.x)), length_squared_0(float3(_S661.y, _S662.y, _S663.y))), length_squared_0(float3(_S661.z, _S662.z, _S663.z))));
}

float2 unpack_snorm2x16_0(uint p_21)
{
    return clamp(float2(int2(int(p_21 << 16U) >> 16U, int(p_21) >> 16U)) / float2(32767.0f) , float2(-1.0f) , float2(1.0f) );
}

float3 oct_decode_0(uint packed_0)
{
    float2 e_12 = unpack_snorm2x16_0(packed_0);
    float _S664 = e_12.x;
    float _S665 = e_12.y;
    float _S666 = 1.0f - abs(_S664) - abs(_S665);
    thread float3 n_14 = float3(_S664, _S665, _S666);
    float t_10 = saturate(- _S666);
    float _S667;
    if(_S664 >= 0.0f)
    {
        _S667 = - t_10;
    }
    else
    {
        _S667 = t_10;
    }
    n_14.x = n_14.x + _S667;
    if((n_14.y) >= 0.0f)
    {
        _S667 = - t_10;
    }
    else
    {
        _S667 = t_10;
    }
    n_14.y = n_14.y + _S667;
    return normalize(n_14);
}

float3 xform_normal_0(const GpuInstance_natural_0 thread* i_31, float3 n_15)
{
    float4 _S668 = float4((&i_31->m_7)->data_7[int(0)]) ;
    float4 _S669 = float4((&i_31->m_7)->data_7[int(1)]) ;
    float4 _S670 = float4((&i_31->m_7)->data_7[int(2)]) ;
    float3 a0_0 = float3(_S668.x, _S669.x, _S670.x);
    float3 a1_0 = float3(_S668.y, _S669.y, _S670.y);
    float3 a2_2 = float3(_S668.z, _S669.z, _S670.z);
    return cross(a1_0, a2_2) * float3(n_15.x)  + cross(a2_2, a0_0) * float3(n_15.y)  + cross(a0_0, a1_0) * float3(n_15.z) ;
}

float3 tree_vertex_0(FrameData_natural_0 device* f_42, const GpuInstance_natural_0 thread* inst_0, float3 node_0, float3 pm_0, uint n_16, uint aux_0)
{
    uint _S671 = inst_0->pad_2[int(0)];
    float3 origin_1 = float3((float4((&inst_0->m_7)->data_7[int(0)]) ).w, (float4((&inst_0->m_7)->data_7[int(1)]) ).w, (float4((&inst_0->m_7)->data_7[int(2)]) ).w);
    bool _S672;
    if((aux_0 & 134217728U) != 0U)
    {
        _S672 = ((_S671 >> 16U) & (1U << (aux_0 >> 29U))) != 0U;
    }
    else
    {
        _S672 = false;
    }
    if(_S672)
    {
        return origin_1;
    }
    float3 _S673 = xform_point_0(inst_0, pm_0);
    WindData_natural_0 w_15 = f_42->wind_1;
    float4 _S674 = float4(f_42->wind_1.cfg_2) ;
    if((_S674.x) < 0.5f)
    {
        return _S673;
    }
    float3 _S675 = node_0 + origin_1;
    float _S676 = _S674.y;
    float fade_0 = saturate((_S676 - length(_S675)) / max(0.25f * _S676, 1.0f));
    if(fade_0 <= 0.0f)
    {
        return _S673;
    }
    float q_16 = float((aux_0 >> 11U) & 127U) / 127.0f;
    float sway_0 = q_16 * q_16 * 4.0f;
    float flutter_0 = float((aux_0 >> 18U) & 7U) / 7.0f;
    float phase_0 = float((aux_0 >> 21U) & 63U) / 64.0f + float(_S671 >> 24U) / 255.0f;
    float4 _S677 = float4(w_15.wind_0) ;
    float2 _S678 = _S677.xz;
    float speed_0 = length(_S678);
    float2 wd_0;
    if(speed_0 > 0.0010000000474974513f)
    {
        wd_0 = _S678 / float2(speed_0) ;
    }
    else
    {
        wd_0 = float2(0.0f, 0.0f);
    }
    float4 _S679 = float4(w_15.gust_0) ;
    float s_14 = 0.5f + 0.5f * sin((_S679.x + dot(_S675.xz, wd_0) / max(_S679.y, 1.0f)) * 6.2831854820251465f);
    float gust_1 = s_14 * s_14 * s_14 * _S679.z;
    float t_11 = _S677.w;
    float _S680 = speed_0 * 0.10000000149011612f * fade_0;
    float _S681 = max_scale_0(inst_0);
    float k_10 = _S680 * _S681;
    float rock_1 = 0.55000001192092896f + gust_1 + 0.30000001192092896f * sin(t_11 * 1.7000000476837158f + phase_0 * 6.2831854820251465f) + 0.15000000596046448f * sin(t_11 * 0.62999999523162842f + phase_0 * 19.0f);
    thread float3 d_41 = float3(wd_0.x, 0.0f, wd_0.y) * float3((sway_0 * k_10 * rock_1)) ;
    d_41.y = d_41.y - 0.25f * sway_0 * k_10 * rock_1 * rock_1 * 0.30000001192092896f;
    float3 _S682 = xform_normal_0(inst_0, oct_decode_0(n_16));
    return _S673 + d_41 + normalize(_S682) * float3((flutter_0 * _S679.w * (0.60000002384185791f + gust_1) * 0.05000000074505806f * fade_0 * sin(t_11 * 11.0f + phase_0 * 40.0f + dot(_S673, float3(3.0999999046325684f, 1.7000000476837158f, 2.2999999523162842f)))))  * float3(_S681) ;
}

struct GpuMaterial_natural_0
{
    packed_float4 base_0;
    packed_float4 params_1;
    packed_float4 shade_0;
    packed_uint4 tex_1;
    packed_float4 uv_4;
};

GpuMaterial_natural_0 device* ed_buf_9(uint i_32, KernelContext_0 thread* kernelContext_60)
{
    return (GpuMaterial_natural_0 device*)kernelContext_60->ed_buffers_0[i_32];
}

struct TreeSurf_0
{
    float3 albedo_7;
    bool leaf_1;
    bool shell_0;
};

float3 tree_hue_0(float h_33)
{
    float3 _S683;
    if(h_33 >= 0.0f)
    {
        _S683 = mix(float3(1.0f, 1.0f, 1.0f), float3(0.85000002384185791f, 0.97000002861022949f, 1.2000000476837158f), float3(h_33) );
    }
    else
    {
        _S683 = mix(float3(1.0f, 1.0f, 1.0f), float3(1.1200000047683716f, 1.0f, 0.75f), float3(- h_33) );
    }
    return _S683;
}

float2 tree_uv_0(uint w_16)
{
    return float2(float(w_16 & 65535U), float(w_16 >> 16U)) * float2(1.5259021893143654e-05f) ;
}

TreeSurf_0 tree_surface_0(FrameData_natural_0 device* f_43, const GpuInstance_natural_0 thread* inst_1, const GpuMaterial_natural_0 thread* m_8, uint2 w0_0, uint2 w1_0, uint2 w2_0, float b0_0, float b1_4, float b2_6, float2 pix_0, float3 p0_1, float3 e1_3, float3 e2_3, KernelContext_0 thread* kernelContext_61)
{
    uint a_17 = w0_0.y;
    uint _S684 = w1_0.y;
    uint _S685 = w2_0.y;
    float val_0 = (float(a_17 & 127U) * b0_0 + float(_S684 & 127U) * b1_4 + float(_S685 & 127U) * b2_6) * 0.015748031437397003f;
    float hue_0 = (float((a_17 >> 7U) & 15U) * b0_0 + float((_S684 >> 7U) & 15U) * b1_4 + float((_S685 >> 7U) & 15U) * b2_6) * 0.13333334028720856f - 1.0f;
    float iv_0 = float((inst_1->pad_2[int(0)]) & 255U) / 128.0f;
    float ih_0 = float(((inst_1->pad_2[int(0)]) >> 8U) & 255U) / 127.5f - 1.0f;
    thread TreeSurf_0 o_2;
    (&o_2)->leaf_1 = (a_17 & 134217728U) != 0U;
    (&o_2)->shell_0 = false;
    if((a_17 & 268435456U) != 0U)
    {
        (&o_2)->albedo_7 = (float4(m_8->shade_0) ).xyz * float3(val_0)  * float3(mix(1.0f, iv_0, 0.5f)) ;
        return o_2;
    }
    float3 alb_0 = (float4(m_8->base_0) ).xyz * float3(val_0)  * float3(iv_0)  * tree_hue_0(clamp(hue_0 + 0.5f * ih_0, -1.0f, 1.0f));
    bool _S686;
    if((&o_2)->leaf_1)
    {
        _S686 = ((uint4(m_8->tex_1) ).x) != 4294967295U;
    }
    else
    {
        _S686 = false;
    }
    float3 alb_1;
    if(_S686)
    {
        float2 t0_1 = tree_uv_0(w0_0.x);
        float2 d1_0 = tree_uv_0(w1_0.x) - t0_1;
        float2 d2_0 = tree_uv_0(w2_0.x) - t0_1;
        float2 uv_5 = t0_1 + d1_0 * float2(b1_4)  + d2_0 * float2(b2_6) ;
        float3 _S687 = view_dir_0(f_43, pix_0 + float2(1.0f, 0.0f));
        float2 bx_2 = ray_bary_0(_S687, p0_1, e1_3, e2_3);
        float3 _S688 = view_dir_0(f_43, pix_0 + float2(0.0f, 1.0f));
        float2 by_2 = ray_bary_0(_S688, p0_1, e1_3, e2_3);
        float2 ux_0 = t0_1 + d1_0 * float2(bx_2.x)  + d2_0 * float2(bx_2.y)  - uv_5;
        float2 uy_0 = t0_1 + d1_0 * float2(by_2.x)  + d2_0 * float2(by_2.y)  - uv_5;
        texture2d<float, access::sample> _S689 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_61->ed_textures_0))[((uint4(m_8->tex_1) ).x)]));
        sampler _S690 = ((reinterpret_cast<device sampler*>((kernelContext_61->ed_samplers_0))[(kernelContext_61->ed_c_0->smp_0)]));
        alb_1 = alb_0 * clamp(((_S689).sample((_S690), (uv_5), gradient2d((ux_0), (uy_0)))).xyz / float3(0.14699999988079071f, 0.27399998903274536f, 0.050999999046325684f), float3(0.20000000298023224f, 0.20000000298023224f, 0.20000000298023224f), float3(2.5f, 2.5f, 2.5f));
    }
    else
    {
        alb_1 = alb_0;
    }
    (&o_2)->shell_0 = !(&o_2)->leaf_1;
    (&o_2)->albedo_7 = min(alb_1, float3(0.89999997615814209f, 0.89999997615814209f, 0.89999997615814209f));
    return o_2;
}

packed_float2 device* ed_buf_10(uint i_33, KernelContext_0 thread* kernelContext_62)
{
    return (packed_float2 device*)kernelContext_62->ed_buffers_0[i_33];
}

float2 uv_xform_0(const GpuMaterial_natural_0 thread* m_9, float2 uv_6)
{
    float r_10 = (as_type<float>(((uint4(m_9->tex_1) ).w)));
    float s_15 = sin(r_10);
    float c_21 = cos(r_10);
    float4 _S691 = float4(m_9->uv_4) ;
    float2 q_17 = uv_6 * _S691.zw;
    float _S692 = q_17.x;
    float _S693 = q_17.y;
    return float2(c_21 * _S692 + s_15 * _S693, - s_15 * _S692 + c_21 * _S693) + _S691.xy;
}

float3 shade_foliage_0(FrameData_natural_0 device* f_44, float3 sun_5, float3 albedo_8, float rough_5, float ao_8, float sv_5, float sun_vis_2, float3 n_17, float3 ng_6, float3 v_8, bool leaf_2, KernelContext_0 thread* kernelContext_63)
{
    float3 _S694 = shade_pbr_0(f_44, sun_5, albedo_8, 0.0f, rough_5, ao_8, sv_5, sun_vis_2, n_17, v_8, kernelContext_63);
    float3 col_3;
    if(leaf_2)
    {
        float3 l_8 = (float4(f_44->sun_dir_0) ).xyz;
        col_3 = _S694 + sun_5 * float3(sun_vis_2)  * (albedo_8 * float3(1.1499999761581421f, 1.25f, 0.55000001192092896f)) * float3((saturate(- dot(n_17, l_8)) * (0.44999998807907104f + 1.2000000476837158f * pow(saturate(dot(- v_8, l_8)), 4.0f))))  * float3(0.31830987334251404f) ;
    }
    else
    {
        col_3 = _S694;
    }
    return col_3;
}

float3 shade_toon_0(FrameData_natural_0 device* f_45, float3 sun_6, const GpuMaterial_natural_0 thread* m_10, float3 albedo_9, float3 n_18, float3 v_9, float ao_9, float sv_6, float sun_vis_3, KernelContext_0 thread* kernelContext_64)
{
    float ndl_0 = dot(n_18, (float4(f_45->sun_dir_0) ).xyz);
    float _S695 = max((float4(m_10->params_1) ).z, 1.0f);
    float lit_2 = floor(saturate(ndl_0 * 0.5f + 0.5f) * _S695 + 0.5f) / _S695 * sun_vis_3;
    float4 _S696 = float4(m_10->shade_0) ;
    float3 c_22 = mix(albedo_9 * _S696.xyz, albedo_9, float3(lit_2) );
    float3 _S697 = sun_6 * float3((lit_2 / 3.1415927410125732f)) ;
    float3 _S698 = ambient_light_0(f_45, n_18, kernelContext_64);
    float3 _S699 = occluded_ambient_0(f_45, _S698, sv_6, kernelContext_64);
    float3 _S700 = ao_multibounce_0(f_45, ao_9, albedo_9);
    return c_22 * (_S697 + _S699 * _S700) + float3((pow(1.0f - saturate(dot(n_18, v_9)), 4.0f) * _S696.w * saturate(ndl_0 + 0.30000001192092896f) * sun_vis_3))  * sun_6 * float3(0.15000000596046448f) ;
}

float cascade_depth_delta_0(FrameData_natural_0 device* f_46, uint atlas_2, uint c_23, float3 p_22, KernelContext_0 thread* kernelContext_65)
{
    float4 _S701 = float4((&(&f_46->light_0)->cascade_0)->data_3[c_23]) ;
    float tile_2 = (float4((&f_46->light_0)->cfg_0) ).y;
    float4 s_16 = (((float4(p_22, 1.0f)) * (matrix<float,int(4),int(4)> ((&(&f_46->light_0)->shadow_1)->data_2[c_23].data_0[int(0)][int(0)], (&(&f_46->light_0)->shadow_1)->data_2[c_23].data_0[int(1)][int(0)], (&(&f_46->light_0)->shadow_1)->data_2[c_23].data_0[int(2)][int(0)], (&(&f_46->light_0)->shadow_1)->data_2[c_23].data_0[int(3)][int(0)], (&(&f_46->light_0)->shadow_1)->data_2[c_23].data_0[int(0)][int(1)], (&(&f_46->light_0)->shadow_1)->data_2[c_23].data_0[int(1)][int(1)], (&(&f_46->light_0)->shadow_1)->data_2[c_23].data_0[int(2)][int(1)], (&(&f_46->light_0)->shadow_1)->data_2[c_23].data_0[int(3)][int(1)], (&(&f_46->light_0)->shadow_1)->data_2[c_23].data_0[int(0)][int(2)], (&(&f_46->light_0)->shadow_1)->data_2[c_23].data_0[int(1)][int(2)], (&(&f_46->light_0)->shadow_1)->data_2[c_23].data_0[int(2)][int(2)], (&(&f_46->light_0)->shadow_1)->data_2[c_23].data_0[int(3)][int(2)], (&(&f_46->light_0)->shadow_1)->data_2[c_23].data_0[int(0)][int(3)], (&(&f_46->light_0)->shadow_1)->data_2[c_23].data_0[int(1)][int(3)], (&(&f_46->light_0)->shadow_1)->data_2[c_23].data_0[int(2)][int(3)], (&(&f_46->light_0)->shadow_1)->data_2[c_23].data_0[int(3)][int(3)]))));
    int2 _S702 = int2(_S701.zw);
    float _S703 = s_16.z;
    float _S704 = ed_depth_read_0(atlas_2, uint2(clamp(_S702 + int2(floor(float2(s_16.x * 0.5f + 0.5f, 0.5f - s_16.y * 0.5f) * float2(tile_2) )), _S702, _S702 + int2((int(tile_2) - int(1))) )), kernelContext_65);
    return (_S703 - _S704) * _S701.y;
}

float4 resolve_px_0(const FsOut_0 thread* i_34, bool terrain_0, KernelContext_0 thread* kernelContext_66)
{
    FrameData_natural_0 device* _S705 = ed_buf_4(kernelContext_66->ed_c_0->frame_0, kernelContext_66);
    float2 _S706 = i_34->pos_0.xy;
    uint2 _S707 = uint2(_S706);
    texture2d<uint, access::sample> _S708 = ((reinterpret_cast<device texture2d<uint, access::sample>*>((kernelContext_66->ed_textures_0))[(kernelContext_66->ed_c_0->vis_0)]));
    int3 _S709 = int3(int2(_S707), int(0));
    uint2 vis_3 = ((_S708).read(vec<uint,2>(((_S709)).xy), uint(((_S709)).z))).xy;
    float3 _S710 = view_dir_0(_S705, _S706);
    uint _S711 = vis_3.x;
    if(_S711 == 0U)
    {
        float3 _S712 = sky_pixel_0(_S705, _S710, kernelContext_66);
        return float4(_S712, 1.0f);
    }
    bool textured_0;
    if(terrain_0)
    {
        textured_0 = (_S711 & 2147483648U) != 0U;
    }
    else
    {
        textured_0 = false;
    }
    if(textured_0)
    {
        float4 _S713 = terrain_pixel_0(_S705, i_34, vis_3, _S710, kernelContext_66);
        return _S713;
    }
    GpuInstance_natural_0 device* _S714 = ed_buf_5(kernelContext_66->ed_c_0->instances_0, kernelContext_66);
    thread GpuInstance_natural_0 _S715 = *(_S714 + (_S711 - 1U));
    GpuMesh_natural_0 device* _S716 = ed_buf_6(kernelContext_66->ed_c_0->meshes_0, kernelContext_66);
    GpuMesh_natural_0 device* _S717 = _S716 + (&_S715)->mesh_0;
    uint _S718 = vis_3.y;
    uint lod_1 = _S718 >> 28U;
    uint prim_1 = (_S718 & 268435455U) % max(_S717->index_count_0[lod_1] / 3U, 1U);
    uint device* _S719 = ed_buf_0(kernelContext_66->ed_c_0->indices_0, kernelContext_66);
    uint device* idx_0 = _S719 + _S717->first_index_0[lod_1] + prim_1 * 3U;
    Vtx_0 device* _S720 = ed_buf_7(kernelContext_66->ed_c_0->verts_0, kernelContext_66);
    int i0_0 = int(*(idx_0 + int(0))) + _S717->base_vertex_0[lod_1];
    int i1_0 = int(*(idx_0 + int(1))) + _S717->base_vertex_0[lod_1];
    int i2_0 = int(*(idx_0 + int(2))) + _S717->base_vertex_0[lod_1];
    Vtx_0 v0_0 = *(_S720 + i0_0);
    Vtx_0 v1_0 = *(_S720 + i1_0);
    Vtx_0 v2_0 = *(_S720 + i2_0);
    packed_float4 device* _S721 = ed_buf_3(kernelContext_66->ed_c_0->nodes_0, kernelContext_66);
    float3 node_1 = (float4(*(_S721 + (((&_S715)->node_flags_0) & 65535U))) ).xyz;
    bool foliage_0 = ((_S717->flags_1) & 1U) != 0U;
    uint2 _S722 = uint2(0U, 0U);
    float3 l0_0;
    float3 l1_0;
    float3 l2_0;
    uint2 tw0_0;
    uint2 tw1_0;
    uint2 tw2_0;
    if(foliage_0)
    {
        packed_uint2 device* _S723 = ed_buf_8(kernelContext_66->ed_c_0->uvs_0, kernelContext_66);
        uint2 _S724 = uint2(*(_S723 + i0_0)) ;
        uint2 _S725 = uint2(*(_S723 + i1_0)) ;
        uint2 _S726 = uint2(*(_S723 + i2_0)) ;
        float3 _S727 = tree_vertex_0(_S705, &_S715, node_1, float3(v0_0.px_4, v0_0.py_0, v0_0.pz_0), v0_0.n_13, _S724.y);
        float3 _S728 = tree_vertex_0(_S705, &_S715, node_1, float3(v1_0.px_4, v1_0.py_0, v1_0.pz_0), v1_0.n_13, _S725.y);
        float3 _S729 = tree_vertex_0(_S705, &_S715, node_1, float3(v2_0.px_4, v2_0.py_0, v2_0.pz_0), v2_0.n_13, _S726.y);
        l0_0 = _S727;
        l1_0 = _S728;
        l2_0 = _S729;
        tw0_0 = _S724;
        tw1_0 = _S725;
        tw2_0 = _S726;
    }
    else
    {
        float3 _S730 = xform_point_0(&_S715, float3(v0_0.px_4, v0_0.py_0, v0_0.pz_0));
        float3 _S731 = xform_point_0(&_S715, float3(v1_0.px_4, v1_0.py_0, v1_0.pz_0));
        float3 _S732 = xform_point_0(&_S715, float3(v2_0.px_4, v2_0.py_0, v2_0.pz_0));
        l0_0 = _S730;
        l1_0 = _S731;
        l2_0 = _S732;
        tw0_0 = _S722;
        tw1_0 = _S722;
        tw2_0 = _S722;
    }
    float3 p0_2 = node_1 + l0_0;
    float3 e1_4 = l1_0 - l0_0;
    float3 e2_4 = l2_0 - l0_0;
    float2 bb_2 = ray_bary_0(_S710, p0_2, e1_4, e2_4);
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
    float3 _S733 = float3(b0_1) ;
    float3 local_0 = l0_0 * _S733 + l1_0 * float3(b1_6)  + l2_0 * float3(b2_8) ;
    float3 _S734 = xform_normal_0(&_S715, oct_decode_0(v0_0.n_13) * _S733 + oct_decode_0(v1_0.n_13) * float3(b1_6)  + oct_decode_0(v2_0.n_13) * float3(b2_8) );
    float3 n_19 = normalize(_S734);
    float3 v_10 = - _S710;
    GpuMaterial_natural_0 device* _S735 = ed_buf_9(kernelContext_66->ed_c_0->materials_0, kernelContext_66);
    thread GpuMaterial_natural_0 _S736 = *(_S735 + (&_S715)->material_0);
    float4 _S737 = float4((&_S736)->base_0) ;
    float3 albedo_10 = _S737.xyz;
    float4 _S738 = float4((&_S736)->params_1) ;
    float metal_1 = _S738.x;
    float rough_6 = _S738.y;
    uint4 _S739 = uint4((&_S736)->tex_1) ;
    uint _S740 = _S739.x;
    bool _S741 = _S740 != 4294967295U;
    if(_S741)
    {
        textured_0 = true;
    }
    else
    {
        textured_0 = (_S739.y) != 4294967295U;
    }
    if(textured_0)
    {
        textured_0 = true;
    }
    else
    {
        textured_0 = (_S739.z) != 4294967295U;
    }
    bool leaf_3;
    float3 albedo_11;
    float3 n_20;
    float ao_10;
    float rough_7;
    float metal_2;
    float d_42;
    if(foliage_0)
    {
        TreeSurf_0 _S742 = tree_surface_0(_S705, &_S715, &_S736, tw0_0, tw1_0, tw2_0, b0_1, b1_6, b2_8, _S706, p0_2, e1_4, e2_4, kernelContext_66);
        if(_S742.shell_0)
        {
            float3 _S743 = xform_normal_0(&_S715, float3(0.0f, 1.0f, 0.0f));
            float3 up_6 = normalize(_S743);
            float clump_0 = t_noise_0(local_0.xz * float2(0.89999997615814209f)  + float2((local_0.y * 0.60000002384185791f)) , 1.0f);
            float3 albedo_12 = _S742.albedo_7 * float3((mix(0.60000002384185791f, 1.1499999761581421f, clump_0) * 0.85000002384185791f)) ;
            float _S744 = mix(0.64999997615814209f, 1.0f, clump_0);
            n_20 = normalize(mix(n_19, up_6, float3(0.40000000596046448f) ));
            albedo_11 = albedo_12;
            ao_10 = _S744;
            rough_7 = 1.0f;
        }
        else
        {
            n_20 = n_19;
            albedo_11 = _S742.albedo_7;
            ao_10 = 1.0f;
            rough_7 = 0.81999999284744263f;
        }
        if(_S742.leaf_1)
        {
            float3 ngc_0 = cross(e1_4, e2_4);
            if((dot(ngc_0, v_10) * dot(ngc_0, n_20)) < 0.0f)
            {
                n_20 = - n_20;
            }
        }
        leaf_3 = _S742.leaf_1;
        metal_2 = 0.0f;
    }
    else
    {
        if(textured_0)
        {
            packed_float2 device* _S745 = ed_buf_10(kernelContext_66->ed_c_0->uvs_0, kernelContext_66);
            float2 _S746 = float2(*(_S745 + i0_0)) ;
            float2 d1_1 = float2(*(_S745 + i1_0))  - _S746;
            float2 d2_1 = float2(*(_S745 + i2_0))  - _S746;
            float2 _S747 = uv_xform_0(&_S736, _S746 + d1_1 * float2(b1_6)  + d2_1 * float2(b2_8) );
            float3 _S748 = view_dir_0(_S705, _S706 + float2(1.0f, 0.0f));
            float2 bx_3 = ray_bary_0(_S748, p0_2, e1_4, e2_4);
            float3 _S749 = view_dir_0(_S705, _S706 + float2(0.0f, 1.0f));
            float2 by_3 = ray_bary_0(_S749, p0_2, e1_4, e2_4);
            float2 _S750 = uv_xform_0(&_S736, _S746 + d1_1 * float2(bx_3.x)  + d2_1 * float2(bx_3.y) );
            float2 ux_1 = _S750 - _S747;
            float2 _S751 = uv_xform_0(&_S736, _S746 + d1_1 * float2(by_3.x)  + d2_1 * float2(by_3.y) );
            float2 uy_1 = _S751 - _S747;
            sampler _S752 = ((reinterpret_cast<device sampler*>((kernelContext_66->ed_samplers_0))[(kernelContext_66->ed_c_0->smp_0)]));
            if(_S741)
            {
                texture2d<float, access::sample> _S753 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_66->ed_textures_0))[(_S740)]));
                albedo_11 = albedo_10 * ((_S753).sample((_S752), (_S747), gradient2d((ux_1), (uy_1)))).xyz;
            }
            else
            {
                albedo_11 = albedo_10;
            }
            uint _S754 = _S739.z;
            if(_S754 != 4294967295U)
            {
                texture2d<float, access::sample> _S755 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_66->ed_textures_0))[(_S754)]));
                float3 orm_0 = ((_S755).sample((_S752), (_S747), gradient2d((ux_1), (uy_1)))).xyz;
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
            uint _S756 = _S739.y;
            if(_S756 != 4294967295U)
            {
                float _S757 = d1_1.x;
                float _S758 = d2_1.y;
                float _S759 = d1_1.y;
                float _S760 = d2_1.x;
                float det_2 = _S757 * _S758 - _S759 * _S760;
                if((abs(det_2)) > 9.999999960041972e-13f)
                {
                    float3 _S761 = float3((1.0f / det_2)) ;
                    float3 tg_0 = (e1_4 * float3(_S758)  - e2_4 * float3(_S759) ) * _S761;
                    float3 bt_0 = (e2_4 * float3(_S757)  - e1_4 * float3(_S760) ) * _S761;
                    float3 tg_1 = tg_0 - n_19 * float3(dot(n_19, tg_0)) ;
                    if((length_squared_0(tg_1)) > 9.99999968265522539e-21f)
                    {
                        float3 tg_2 = normalize(tg_1);
                        float3 _S762 = cross(n_19, tg_2);
                        if((dot(_S762, bt_0)) < 0.0f)
                        {
                            d_42 = -1.0f;
                        }
                        else
                        {
                            d_42 = 1.0f;
                        }
                        float3 bn_0 = _S762 * float3(- d_42) ;
                        texture2d<float, access::sample> _S763 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_66->ed_textures_0))[(_S756)]));
                        float2 xy_0 = ((_S763).sample((_S752), (_S747), gradient2d((ux_1), (uy_1)))).xy * float2(2.0f)  - float2(1.0f) ;
                        n_20 = normalize(tg_2 * float3(xy_0.x)  + bn_0 * float3(xy_0.y)  + n_19 * float3(sqrt(saturate(1.0f - dot(xy_0, xy_0)))) );
                    }
                    else
                    {
                        n_20 = n_19;
                    }
                }
                else
                {
                    n_20 = n_19;
                }
            }
            else
            {
                n_20 = n_19;
            }
        }
        else
        {
            n_20 = n_19;
            albedo_11 = albedo_10;
            ao_10 = 1.0f;
            rough_7 = rough_6;
            metal_2 = metal_1;
        }
        leaf_3 = false;
    }
    float _S764 = dot(n_20, v_10);
    if(_S764 < 0.0f)
    {
        n_20 = normalize(n_20 - float3((1.0099999904632568f * _S764))  * v_10);
    }
    if((_S738.w) > 0.5f)
    {
        float2 c_24 = floor(local_0.xz * float2(0.5f) );
        albedo_11 = albedo_11 * float3(mix(0.80000001192092896f, 1.1499999761581421f, fmod(abs(c_24.x + c_24.y), 2.0f))) ;
    }
    float3 p_23 = node_1 + local_0;
    float zlin_3 = - (((float4(p_23, 1.0f)) * (matrix<float,int(4),int(4)> (_S705->view_0.data_0[int(0)][int(0)], _S705->view_0.data_0[int(1)][int(0)], _S705->view_0.data_0[int(2)][int(0)], _S705->view_0.data_0[int(3)][int(0)], _S705->view_0.data_0[int(0)][int(1)], _S705->view_0.data_0[int(1)][int(1)], _S705->view_0.data_0[int(2)][int(1)], _S705->view_0.data_0[int(3)][int(1)], _S705->view_0.data_0[int(0)][int(2)], _S705->view_0.data_0[int(1)][int(2)], _S705->view_0.data_0[int(2)][int(2)], _S705->view_0.data_0[int(3)][int(2)], _S705->view_0.data_0[int(0)][int(3)], _S705->view_0.data_0[int(1)][int(3)], _S705->view_0.data_0[int(2)][int(3)], _S705->view_0.data_0[int(3)][int(3)])))).z;
    float3 ng_7 = cross(e1_4, e2_4);
    float3 ng_8;
    if((length_squared_0(ng_7)) > 9.99999968265522539e-21f)
    {
        ng_8 = normalize(ng_7);
    }
    else
    {
        ng_8 = n_20;
    }
    if((dot(ng_8, v_10)) < 0.0f)
    {
        ng_8 = - ng_8;
    }
    float _S765 = sun_shadow_0(_S705, kernelContext_66->ed_c_0->shadow_0, p_23, ng_8, zlin_3, _S706, kernelContext_66);
    float2 _S766 = ao_upsample_0(_S705, kernelContext_66->ed_c_0->ao_0, _S706, zlin_3, kernelContext_66);
    float ao_11 = ao_10 * _S766.x;
    float sv_7 = _S766.y;
    float3 _S767 = sun_light_at_0(_S705, p_23, kernelContext_66);
    float3 col_4;
    if(foliage_0)
    {
        float3 _S768 = shade_foliage_0(_S705, _S767, albedo_11, rough_7, ao_11, sv_7, _S765, n_20, ng_8, v_10, leaf_3, kernelContext_66);
        col_4 = _S768;
    }
    else
    {
        if((_S737.w) > 0.5f)
        {
            float3 _S769 = shade_toon_0(_S705, _S767, &_S736, albedo_11, n_20, v_10, ao_11, sv_7, _S765, kernelContext_66);
            l0_0 = _S769;
        }
        else
        {
            float3 _S770 = shade_pbr_0(_S705, _S767, albedo_11, metal_2, rough_7, ao_11, sv_7, _S765, n_20, v_10, kernelContext_66);
            l0_0 = _S770;
        }
        col_4 = l0_0;
    }
    float3 _S771 = apply_aerial_0(_S705, col_4, p_23, _S706, kernelContext_66);
    if(((uint4((&_S705->light_0)->flags_0) ).w) == 1U)
    {
        uint _S772 = shadow_cascade_0(_S705, p_23, zlin_3);
        col_4 = TINT_2[_S772] * float3((0.039999999105930328f + 0.5f * _S765)) ;
    }
    else
    {
        if(((uint4((&_S705->light_0)->flags_0) ).w) == 3U)
        {
            uint _S773 = shadow_cascade_0(_S705, p_23, zlin_3);
            if(_S773 < 4U)
            {
                float _S774 = cascade_depth_delta_0(_S705, kernelContext_66->ed_c_0->shadow_0, _S773, p_23, kernelContext_66);
                d_42 = _S774;
            }
            else
            {
                d_42 = 0.0f;
            }
            col_4 = float3(max(d_42, 0.0f), max(- d_42, 0.0f), 0.0f) * float3(0.10000000149011612f) ;
        }
        else
        {
            if(((uint4((&_S705->light_0)->flags_0) ).w) == 2U)
            {
                col_4 = float3((0.5f * ao_11)) ;
            }
            else
            {
                if(((uint4((&_S705->light_0)->flags_0) ).w) == 4U)
                {
                    col_4 = float3((0.5f * sv_7)) ;
                }
                else
                {
                    col_4 = _S771;
                }
            }
        }
    }
    return float4(col_4, 1.0f);
}

constant array<float3, int(5)> TINT_3 = { { float3(1.0f, 0.25f, 0.25f), float3(0.25f, 1.0f, 0.25f), float3(0.30000001192092896f, 0.44999998807907104f, 1.0f), float3(1.0f, 1.0f, 0.25f), float3(0.60000002384185791f, 0.60000002384185791f, 0.60000002384185791f) } };
float4 resolve_px_1(const FsOut_0 thread* i_35, bool terrain_1, KernelContext_0 thread* kernelContext_67)
{
    FrameData_natural_0 device* _S775 = ed_buf_4(kernelContext_67->ed_c_0->frame_0, kernelContext_67);
    float2 _S776 = i_35->pos_0.xy;
    uint2 _S777 = uint2(_S776);
    texture2d<uint, access::sample> _S778 = ((reinterpret_cast<device texture2d<uint, access::sample>*>((kernelContext_67->ed_textures_0))[(kernelContext_67->ed_c_0->vis_0)]));
    int3 _S779 = int3(int2(_S777), int(0));
    uint2 vis_4 = ((_S778).read(vec<uint,2>(((_S779)).xy), uint(((_S779)).z))).xy;
    float3 _S780 = view_dir_0(_S775, _S776);
    uint _S781 = vis_4.x;
    if(_S781 == 0U)
    {
        float3 _S782 = sky_pixel_0(_S775, _S780, kernelContext_67);
        return float4(_S782, 1.0f);
    }
    bool textured_1;
    if(terrain_1)
    {
        textured_1 = (_S781 & 2147483648U) != 0U;
    }
    else
    {
        textured_1 = false;
    }
    if(textured_1)
    {
        float4 _S783 = terrain_pixel_1(_S775, i_35, vis_4, _S780, kernelContext_67);
        return _S783;
    }
    GpuInstance_natural_0 device* _S784 = ed_buf_5(kernelContext_67->ed_c_0->instances_0, kernelContext_67);
    thread GpuInstance_natural_0 _S785 = *(_S784 + (_S781 - 1U));
    GpuMesh_natural_0 device* _S786 = ed_buf_6(kernelContext_67->ed_c_0->meshes_0, kernelContext_67);
    GpuMesh_natural_0 device* _S787 = _S786 + (&_S785)->mesh_0;
    uint _S788 = vis_4.y;
    uint lod_2 = _S788 >> 28U;
    uint prim_2 = (_S788 & 268435455U) % max(_S787->index_count_0[lod_2] / 3U, 1U);
    uint device* _S789 = ed_buf_0(kernelContext_67->ed_c_0->indices_0, kernelContext_67);
    uint device* idx_1 = _S789 + _S787->first_index_0[lod_2] + prim_2 * 3U;
    Vtx_0 device* _S790 = ed_buf_7(kernelContext_67->ed_c_0->verts_0, kernelContext_67);
    int i0_1 = int(*(idx_1 + int(0))) + _S787->base_vertex_0[lod_2];
    int i1_1 = int(*(idx_1 + int(1))) + _S787->base_vertex_0[lod_2];
    int i2_1 = int(*(idx_1 + int(2))) + _S787->base_vertex_0[lod_2];
    Vtx_0 v0_1 = *(_S790 + i0_1);
    Vtx_0 v1_1 = *(_S790 + i1_1);
    Vtx_0 v2_1 = *(_S790 + i2_1);
    packed_float4 device* _S791 = ed_buf_3(kernelContext_67->ed_c_0->nodes_0, kernelContext_67);
    float3 node_2 = (float4(*(_S791 + (((&_S785)->node_flags_0) & 65535U))) ).xyz;
    bool foliage_1 = ((_S787->flags_1) & 1U) != 0U;
    uint2 _S792 = uint2(0U, 0U);
    float3 l0_1;
    float3 l1_1;
    float3 l2_1;
    uint2 tw0_1;
    uint2 tw1_1;
    uint2 tw2_1;
    if(foliage_1)
    {
        packed_uint2 device* _S793 = ed_buf_8(kernelContext_67->ed_c_0->uvs_0, kernelContext_67);
        uint2 _S794 = uint2(*(_S793 + i0_1)) ;
        uint2 _S795 = uint2(*(_S793 + i1_1)) ;
        uint2 _S796 = uint2(*(_S793 + i2_1)) ;
        float3 _S797 = tree_vertex_0(_S775, &_S785, node_2, float3(v0_1.px_4, v0_1.py_0, v0_1.pz_0), v0_1.n_13, _S794.y);
        float3 _S798 = tree_vertex_0(_S775, &_S785, node_2, float3(v1_1.px_4, v1_1.py_0, v1_1.pz_0), v1_1.n_13, _S795.y);
        float3 _S799 = tree_vertex_0(_S775, &_S785, node_2, float3(v2_1.px_4, v2_1.py_0, v2_1.pz_0), v2_1.n_13, _S796.y);
        l0_1 = _S797;
        l1_1 = _S798;
        l2_1 = _S799;
        tw0_1 = _S794;
        tw1_1 = _S795;
        tw2_1 = _S796;
    }
    else
    {
        float3 _S800 = xform_point_0(&_S785, float3(v0_1.px_4, v0_1.py_0, v0_1.pz_0));
        float3 _S801 = xform_point_0(&_S785, float3(v1_1.px_4, v1_1.py_0, v1_1.pz_0));
        float3 _S802 = xform_point_0(&_S785, float3(v2_1.px_4, v2_1.py_0, v2_1.pz_0));
        l0_1 = _S800;
        l1_1 = _S801;
        l2_1 = _S802;
        tw0_1 = _S792;
        tw1_1 = _S792;
        tw2_1 = _S792;
    }
    float3 p0_3 = node_2 + l0_1;
    float3 e1_5 = l1_1 - l0_1;
    float3 e2_5 = l2_1 - l0_1;
    float2 bb_3 = ray_bary_0(_S780, p0_3, e1_5, e2_5);
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
    float3 _S803 = float3(b0_2) ;
    float3 local_1 = l0_1 * _S803 + l1_1 * float3(b1_8)  + l2_1 * float3(b2_11) ;
    float3 _S804 = xform_normal_0(&_S785, oct_decode_0(v0_1.n_13) * _S803 + oct_decode_0(v1_1.n_13) * float3(b1_8)  + oct_decode_0(v2_1.n_13) * float3(b2_11) );
    float3 n_21 = normalize(_S804);
    float3 v_11 = - _S780;
    GpuMaterial_natural_0 device* _S805 = ed_buf_9(kernelContext_67->ed_c_0->materials_0, kernelContext_67);
    thread GpuMaterial_natural_0 _S806 = *(_S805 + (&_S785)->material_0);
    float4 _S807 = float4((&_S806)->base_0) ;
    float3 albedo_13 = _S807.xyz;
    float4 _S808 = float4((&_S806)->params_1) ;
    float metal_4 = _S808.x;
    float rough_9 = _S808.y;
    uint4 _S809 = uint4((&_S806)->tex_1) ;
    uint _S810 = _S809.x;
    bool _S811 = _S810 != 4294967295U;
    if(_S811)
    {
        textured_1 = true;
    }
    else
    {
        textured_1 = (_S809.y) != 4294967295U;
    }
    if(textured_1)
    {
        textured_1 = true;
    }
    else
    {
        textured_1 = (_S809.z) != 4294967295U;
    }
    bool leaf_4;
    float3 albedo_14;
    float3 n_22;
    float ao_12;
    float rough_10;
    float metal_5;
    float d_43;
    if(foliage_1)
    {
        TreeSurf_0 _S812 = tree_surface_0(_S775, &_S785, &_S806, tw0_1, tw1_1, tw2_1, b0_2, b1_8, b2_11, _S776, p0_3, e1_5, e2_5, kernelContext_67);
        if(_S812.shell_0)
        {
            float3 _S813 = xform_normal_0(&_S785, float3(0.0f, 1.0f, 0.0f));
            float3 up_7 = normalize(_S813);
            float clump_1 = t_noise_0(local_1.xz * float2(0.89999997615814209f)  + float2((local_1.y * 0.60000002384185791f)) , 1.0f);
            float3 albedo_15 = _S812.albedo_7 * float3((mix(0.60000002384185791f, 1.1499999761581421f, clump_1) * 0.85000002384185791f)) ;
            float _S814 = mix(0.64999997615814209f, 1.0f, clump_1);
            n_22 = normalize(mix(n_21, up_7, float3(0.40000000596046448f) ));
            albedo_14 = albedo_15;
            ao_12 = _S814;
            rough_10 = 1.0f;
        }
        else
        {
            n_22 = n_21;
            albedo_14 = _S812.albedo_7;
            ao_12 = 1.0f;
            rough_10 = 0.81999999284744263f;
        }
        if(_S812.leaf_1)
        {
            float3 ngc_1 = cross(e1_5, e2_5);
            if((dot(ngc_1, v_11) * dot(ngc_1, n_22)) < 0.0f)
            {
                n_22 = - n_22;
            }
        }
        leaf_4 = _S812.leaf_1;
        metal_5 = 0.0f;
    }
    else
    {
        if(textured_1)
        {
            packed_float2 device* _S815 = ed_buf_10(kernelContext_67->ed_c_0->uvs_0, kernelContext_67);
            float2 _S816 = float2(*(_S815 + i0_1)) ;
            float2 d1_2 = float2(*(_S815 + i1_1))  - _S816;
            float2 d2_2 = float2(*(_S815 + i2_1))  - _S816;
            float2 _S817 = uv_xform_0(&_S806, _S816 + d1_2 * float2(b1_8)  + d2_2 * float2(b2_11) );
            float3 _S818 = view_dir_0(_S775, _S776 + float2(1.0f, 0.0f));
            float2 bx_4 = ray_bary_0(_S818, p0_3, e1_5, e2_5);
            float3 _S819 = view_dir_0(_S775, _S776 + float2(0.0f, 1.0f));
            float2 by_4 = ray_bary_0(_S819, p0_3, e1_5, e2_5);
            float2 _S820 = uv_xform_0(&_S806, _S816 + d1_2 * float2(bx_4.x)  + d2_2 * float2(bx_4.y) );
            float2 ux_2 = _S820 - _S817;
            float2 _S821 = uv_xform_0(&_S806, _S816 + d1_2 * float2(by_4.x)  + d2_2 * float2(by_4.y) );
            float2 uy_2 = _S821 - _S817;
            sampler _S822 = ((reinterpret_cast<device sampler*>((kernelContext_67->ed_samplers_0))[(kernelContext_67->ed_c_0->smp_0)]));
            if(_S811)
            {
                texture2d<float, access::sample> _S823 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_67->ed_textures_0))[(_S810)]));
                albedo_14 = albedo_13 * ((_S823).sample((_S822), (_S817), gradient2d((ux_2), (uy_2)))).xyz;
            }
            else
            {
                albedo_14 = albedo_13;
            }
            uint _S824 = _S809.z;
            if(_S824 != 4294967295U)
            {
                texture2d<float, access::sample> _S825 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_67->ed_textures_0))[(_S824)]));
                float3 orm_1 = ((_S825).sample((_S822), (_S817), gradient2d((ux_2), (uy_2)))).xyz;
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
            uint _S826 = _S809.y;
            if(_S826 != 4294967295U)
            {
                float _S827 = d1_2.x;
                float _S828 = d2_2.y;
                float _S829 = d1_2.y;
                float _S830 = d2_2.x;
                float det_3 = _S827 * _S828 - _S829 * _S830;
                if((abs(det_3)) > 9.999999960041972e-13f)
                {
                    float3 _S831 = float3((1.0f / det_3)) ;
                    float3 tg_3 = (e1_5 * float3(_S828)  - e2_5 * float3(_S829) ) * _S831;
                    float3 bt_1 = (e2_5 * float3(_S827)  - e1_5 * float3(_S830) ) * _S831;
                    float3 tg_4 = tg_3 - n_21 * float3(dot(n_21, tg_3)) ;
                    if((length_squared_0(tg_4)) > 9.99999968265522539e-21f)
                    {
                        float3 tg_5 = normalize(tg_4);
                        float3 _S832 = cross(n_21, tg_5);
                        if((dot(_S832, bt_1)) < 0.0f)
                        {
                            d_43 = -1.0f;
                        }
                        else
                        {
                            d_43 = 1.0f;
                        }
                        float3 bn_1 = _S832 * float3(- d_43) ;
                        texture2d<float, access::sample> _S833 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_67->ed_textures_0))[(_S826)]));
                        float2 xy_1 = ((_S833).sample((_S822), (_S817), gradient2d((ux_2), (uy_2)))).xy * float2(2.0f)  - float2(1.0f) ;
                        n_22 = normalize(tg_5 * float3(xy_1.x)  + bn_1 * float3(xy_1.y)  + n_21 * float3(sqrt(saturate(1.0f - dot(xy_1, xy_1)))) );
                    }
                    else
                    {
                        n_22 = n_21;
                    }
                }
                else
                {
                    n_22 = n_21;
                }
            }
            else
            {
                n_22 = n_21;
            }
        }
        else
        {
            n_22 = n_21;
            albedo_14 = albedo_13;
            ao_12 = 1.0f;
            rough_10 = rough_9;
            metal_5 = metal_4;
        }
        leaf_4 = false;
    }
    float _S834 = dot(n_22, v_11);
    if(_S834 < 0.0f)
    {
        n_22 = normalize(n_22 - float3((1.0099999904632568f * _S834))  * v_11);
    }
    if((_S808.w) > 0.5f)
    {
        float2 c_25 = floor(local_1.xz * float2(0.5f) );
        albedo_14 = albedo_14 * float3(mix(0.80000001192092896f, 1.1499999761581421f, fmod(abs(c_25.x + c_25.y), 2.0f))) ;
    }
    float3 p_24 = node_2 + local_1;
    float zlin_4 = - (((float4(p_24, 1.0f)) * (matrix<float,int(4),int(4)> (_S775->view_0.data_0[int(0)][int(0)], _S775->view_0.data_0[int(1)][int(0)], _S775->view_0.data_0[int(2)][int(0)], _S775->view_0.data_0[int(3)][int(0)], _S775->view_0.data_0[int(0)][int(1)], _S775->view_0.data_0[int(1)][int(1)], _S775->view_0.data_0[int(2)][int(1)], _S775->view_0.data_0[int(3)][int(1)], _S775->view_0.data_0[int(0)][int(2)], _S775->view_0.data_0[int(1)][int(2)], _S775->view_0.data_0[int(2)][int(2)], _S775->view_0.data_0[int(3)][int(2)], _S775->view_0.data_0[int(0)][int(3)], _S775->view_0.data_0[int(1)][int(3)], _S775->view_0.data_0[int(2)][int(3)], _S775->view_0.data_0[int(3)][int(3)])))).z;
    float3 ng_9 = cross(e1_5, e2_5);
    float3 ng_10;
    if((length_squared_0(ng_9)) > 9.99999968265522539e-21f)
    {
        ng_10 = normalize(ng_9);
    }
    else
    {
        ng_10 = n_22;
    }
    if((dot(ng_10, v_11)) < 0.0f)
    {
        ng_10 = - ng_10;
    }
    float _S835 = sun_shadow_0(_S775, kernelContext_67->ed_c_0->shadow_0, p_24, ng_10, zlin_4, _S776, kernelContext_67);
    float2 _S836 = ao_upsample_0(_S775, kernelContext_67->ed_c_0->ao_0, _S776, zlin_4, kernelContext_67);
    float ao_13 = ao_12 * _S836.x;
    float sv_8 = _S836.y;
    float3 _S837 = sun_light_at_0(_S775, p_24, kernelContext_67);
    float3 col_5;
    if(foliage_1)
    {
        float3 _S838 = shade_foliage_0(_S775, _S837, albedo_14, rough_10, ao_13, sv_8, _S835, n_22, ng_10, v_11, leaf_4, kernelContext_67);
        col_5 = _S838;
    }
    else
    {
        if((_S807.w) > 0.5f)
        {
            float3 _S839 = shade_toon_0(_S775, _S837, &_S806, albedo_14, n_22, v_11, ao_13, sv_8, _S835, kernelContext_67);
            l0_1 = _S839;
        }
        else
        {
            float3 _S840 = shade_pbr_0(_S775, _S837, albedo_14, metal_5, rough_10, ao_13, sv_8, _S835, n_22, v_11, kernelContext_67);
            l0_1 = _S840;
        }
        col_5 = l0_1;
    }
    float3 _S841 = apply_aerial_0(_S775, col_5, p_24, _S776, kernelContext_67);
    if(((uint4((&_S775->light_0)->flags_0) ).w) == 1U)
    {
        uint _S842 = shadow_cascade_0(_S775, p_24, zlin_4);
        col_5 = TINT_3[_S842] * float3((0.039999999105930328f + 0.5f * _S835)) ;
    }
    else
    {
        if(((uint4((&_S775->light_0)->flags_0) ).w) == 3U)
        {
            uint _S843 = shadow_cascade_0(_S775, p_24, zlin_4);
            if(_S843 < 4U)
            {
                float _S844 = cascade_depth_delta_0(_S775, kernelContext_67->ed_c_0->shadow_0, _S843, p_24, kernelContext_67);
                d_43 = _S844;
            }
            else
            {
                d_43 = 0.0f;
            }
            col_5 = float3(max(d_43, 0.0f), max(- d_43, 0.0f), 0.0f) * float3(0.10000000149011612f) ;
        }
        else
        {
            if(((uint4((&_S775->light_0)->flags_0) ).w) == 2U)
            {
                col_5 = float3((0.5f * ao_13)) ;
            }
            else
            {
                if(((uint4((&_S775->light_0)->flags_0) ).w) == 4U)
                {
                    col_5 = float3((0.5f * sv_8)) ;
                }
                else
                {
                    col_5 = _S841;
                }
            }
        }
    }
    return float4(col_5, 1.0f);
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
    thread KernelContext_0 kernelContext_68;
    (&kernelContext_68)->ed_buffers_0 = ed_buffers_1;
    (&kernelContext_68)->ed_c_0 = ed_c_1;
    (&kernelContext_68)->ed_samplers_0 = ed_samplers_1;
    (&kernelContext_68)->ed_textures_0 = ed_textures_1;
    thread FsOut_0 _S845;
    (&_S845)->pos_0 = pos_1;
    float4 _S846 = resolve_px_1(&_S845, false, &kernelContext_68);
    pixelOutput_0 _S847 = { _S846 };
    return _S847;
}

struct pixelOutput_1
{
    float4 output_1 [[color(0)]];
};

[[fragment]] pixelOutput_1 fs_resolve_terrain(float4 pos_2 [[position]], ulong device* ed_buffers_2 [[buffer(2)]], ResolveC_0 constant* ed_c_2 [[buffer(0)]], ulong device* ed_samplers_2 [[buffer(3)]], ulong device* ed_textures_2 [[buffer(1)]])
{
    thread KernelContext_0 kernelContext_69;
    (&kernelContext_69)->ed_buffers_0 = ed_buffers_2;
    (&kernelContext_69)->ed_c_0 = ed_c_2;
    (&kernelContext_69)->ed_samplers_0 = ed_samplers_2;
    (&kernelContext_69)->ed_textures_0 = ed_textures_2;
    thread FsOut_0 _S848;
    (&_S848)->pos_0 = pos_2;
    float4 _S849 = resolve_px_1(&_S848, true, &kernelContext_69);
    pixelOutput_1 _S850 = { _S849 };
    return _S850;
}

struct pixelOutput_2
{
    float4 output_2 [[color(0)]];
};

[[fragment]] pixelOutput_2 fs_resolve_trees(float4 pos_3 [[position]], ulong device* ed_buffers_3 [[buffer(2)]], ResolveC_0 constant* ed_c_3 [[buffer(0)]], ulong device* ed_samplers_3 [[buffer(3)]], ulong device* ed_textures_3 [[buffer(1)]])
{
    thread KernelContext_0 kernelContext_70;
    (&kernelContext_70)->ed_buffers_0 = ed_buffers_3;
    (&kernelContext_70)->ed_c_0 = ed_c_3;
    (&kernelContext_70)->ed_samplers_0 = ed_samplers_3;
    (&kernelContext_70)->ed_textures_0 = ed_textures_3;
    thread FsOut_0 _S851;
    (&_S851)->pos_0 = pos_3;
    float4 _S852 = resolve_px_0(&_S851, true, &kernelContext_70);
    pixelOutput_2 _S853 = { _S852 };
    return _S853;
}

struct vs_resolve_Result_0
{
    float4 pos_4 [[position]];
};

[[vertex]] vs_resolve_Result_0 vs_resolve(uint vid_1 [[vertex_id]], ulong device* ed_buffers_4 [[buffer(2)]], ResolveC_0 constant* ed_c_4 [[buffer(0)]], ulong device* ed_samplers_4 [[buffer(3)]], ulong device* ed_textures_4 [[buffer(1)]])
{
    thread KernelContext_0 kernelContext_71;
    (&kernelContext_71)->ed_buffers_0 = ed_buffers_4;
    (&kernelContext_71)->ed_c_0 = ed_c_4;
    (&kernelContext_71)->ed_samplers_0 = ed_samplers_4;
    (&kernelContext_71)->ed_textures_0 = ed_textures_4;
    thread vs_resolve_Result_0 _S854;
    (&_S854)->pos_4 = fullscreen_vertex_0(vid_1).pos_0;
    return _S854;
}

