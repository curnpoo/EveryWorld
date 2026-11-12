#include <metal_stdlib>
#include <metal_math>
#include <metal_texture>
using namespace metal;
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
    TerrainC_0 constant* ed_c_0;
    ulong device* ed_buffers_0;
};

uint device* ed_buf_0(uint i_0, KernelContext_0 thread* kernelContext_0)
{
    return (uint device*)kernelContext_0->ed_buffers_0[i_0];
}

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
};

TerrainDraw_natural_0 device* ed_buf_1(uint i_1, KernelContext_0 thread* kernelContext_1)
{
    return (TerrainDraw_natural_0 device*)kernelContext_1->ed_buffers_0[i_1];
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
};

FrameData_natural_0 device* ed_buf_2(uint i_2, KernelContext_0 thread* kernelContext_2)
{
    return (FrameData_natural_0 device*)kernelContext_2->ed_buffers_0[i_2];
}

uint2 t_edge_at_0(uint edge_0, uint e_0)
{
    switch(edge_0)
    {
    case 0U:
        {
            return uint2(0U, e_0);
        }
    case 1U:
        {
            return uint2(64U, e_0);
        }
    case 2U:
        {
            return uint2(e_0, 0U);
        }
    default:
        {
            return uint2(e_0, 64U);
        }
    }
}

float t_page_0(uint pool_1, uint slot_0, uint i_3, uint j_0, float scale_0, float offset_1, KernelContext_0 thread* kernelContext_3)
{
    uint s_0 = j_0 * 257U + i_3;
    uint device* _S1 = ed_buf_0(pool_1, kernelContext_3);
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
    return offset_1 + scale_0 * float(q_0);
}

float t_own_h_0(uint pool_2, const TerrainDraw_natural_0 thread* d_0, uint i_4, uint j_1, KernelContext_0 thread* kernelContext_4)
{
    float4 _S2 = float4(d_0->hq_0) ;
    float _S3 = t_page_0(pool_2, (uint4(d_0->slots_0) ).x, i_4 * 4U, j_1 * 4U, _S2.x, _S2.y, kernelContext_4);
    return _S3;
}

float3 t_lattice_0(const TerrainDraw_natural_0 thread* d_1, uint i_5, uint j_2, float h_0, float3 thread* dir_0)
{
    float4 _S4 = float4(d_1->shape_0) ;
    float _S5 = _S4.x;
    float tu_0 = tan((float(i_5) - 32.0f) * _S5);
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
    float e_1 = _S4.z;
    float s_1 = sqrt(1.0f - e_1 * dz_0 * dz_0);
    float sc_0 = _S4.w;
    float dr_0 = _S4.y * e_1 * ddz_0 * (dz_0 + dzc_0) / (s_1 * sc_0 * (s_1 + sc_0));
    return dd_0 * float3((_S12.w + dr_0))  + _S13 * float3(dr_0)  + _S14 * float3(h_0) ;
}

float3 t_base_0(uint pool_3, const TerrainDraw_natural_0 thread* d_2, uint i_6, uint j_3, KernelContext_0 thread* kernelContext_5)
{
    float _S16 = t_own_h_0(pool_3, d_2, i_6, j_3, kernelContext_5);
    thread float3 dir_1;
    float3 _S17 = t_lattice_0(d_2, i_6, j_3, _S16, &dir_1);
    return _S17;
}

float t_morph_0(float3 rel_0, float2 band_0)
{
    float _S18 = band_0.x;
    return saturate((length(rel_0) - _S18) / max(band_0.y - _S18, 0.0010000000474974513f));
}

float3 t_coarse_even_0(uint pool_4, const TerrainDraw_natural_0 thread* d_3, uint e_2, uint2 a_0, uint2 b_0, uint2 c_0, KernelContext_0 thread* kernelContext_6)
{
    float3 _S19 = t_base_0(pool_4, d_3, a_0.x, a_0.y, kernelContext_6);
    if((e_2 & 3U) == 0U)
    {
        return _S19;
    }
    float m_0 = t_morph_0((float4(d_3->origin_0) ).xyz + _S19, (float4(d_3->morph_0) ).zw);
    float3 _S20 = t_base_0(pool_4, d_3, b_0.x, b_0.y, kernelContext_6);
    float3 _S21 = t_base_0(pool_4, d_3, c_0.x, c_0.y, kernelContext_6);
    return mix(_S19, float3(0.5f)  * (_S20 + _S21), float3(m_0) );
}

float3 t_target_even_0(uint pool_5, const TerrainDraw_natural_0 thread* d_4, uint i_7, uint j_4, KernelContext_0 thread* kernelContext_7)
{
    bool border_0;
    if(i_7 == 0U)
    {
        border_0 = true;
    }
    else
    {
        border_0 = j_4 == 0U;
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
        border_0 = j_4 == 64U;
    }
    if(border_0)
    {
        border_0 = true;
    }
    else
    {
        border_0 = ((uint4(d_4->slots_0) ).y) == 4294967295U;
    }
    float h_1;
    if(border_0)
    {
        float _S22 = t_own_h_0(pool_5, d_4, i_7, j_4, kernelContext_7);
        h_1 = _S22;
    }
    else
    {
        uint4 _S23 = uint4(d_4->slots_0) ;
        uint _S24 = _S23.z;
        uint qx_0;
        if((_S24 & 16U) != 0U)
        {
            qx_0 = 128U;
        }
        else
        {
            qx_0 = 0U;
        }
        uint qy_0;
        if((_S24 & 32U) != 0U)
        {
            qy_0 = 128U;
        }
        else
        {
            qy_0 = 0U;
        }
        float4 _S25 = float4(d_4->hq_0) ;
        float _S26 = t_page_0(pool_5, _S23.y, qx_0 + 2U * i_7, qy_0 + 2U * j_4, _S25.z, _S25.w, kernelContext_7);
        h_1 = _S26;
    }
    thread float3 dir_2;
    float3 _S27 = t_lattice_0(d_4, i_7, j_4, h_1, &dir_2);
    return _S27;
}

float3 t_target_0(uint pool_6, const TerrainDraw_natural_0 thread* d_5, uint i_8, uint j_5, KernelContext_0 thread* kernelContext_8)
{
    bool ei_0 = (i_8 & 1U) == 0U;
    bool ej_0 = (j_5 & 1U) == 0U;
    bool _S28;
    if(ei_0)
    {
        _S28 = ej_0;
    }
    else
    {
        _S28 = false;
    }
    if(_S28)
    {
        float3 _S29 = t_target_even_0(pool_6, d_5, i_8, j_5, kernelContext_8);
        return _S29;
    }
    if(!ei_0)
    {
        _S28 = ej_0;
    }
    else
    {
        _S28 = false;
    }
    if(_S28)
    {
        float3 _S30 = t_target_even_0(pool_6, d_5, i_8 - 1U, j_5, kernelContext_8);
        float3 _S31 = t_target_even_0(pool_6, d_5, i_8 + 1U, j_5, kernelContext_8);
        return float3(0.5f)  * (_S30 + _S31);
    }
    if(ei_0)
    {
        _S28 = !ej_0;
    }
    else
    {
        _S28 = false;
    }
    if(_S28)
    {
        float3 _S32 = t_target_even_0(pool_6, d_5, i_8, j_5 - 1U, kernelContext_8);
        float3 _S33 = t_target_even_0(pool_6, d_5, i_8, j_5 + 1U, kernelContext_8);
        return float3(0.5f)  * (_S32 + _S33);
    }
    float3 _S34 = t_target_even_0(pool_6, d_5, i_8 - 1U, j_5 - 1U, kernelContext_8);
    float3 _S35 = t_target_even_0(pool_6, d_5, i_8 + 1U, j_5 + 1U, kernelContext_8);
    return float3(0.5f)  * (_S34 + _S35);
}

uint2 t_ring_0(uint s_2)
{
    uint e_3 = s_2 % 64U;
    switch(s_2 / 64U)
    {
    case 0U:
        {
            return uint2(e_3, 0U);
        }
    case 1U:
        {
            return uint2(64U, e_3);
        }
    case 2U:
        {
            return uint2(64U - e_3, 64U);
        }
    default:
        {
            return uint2(0U, 64U - e_3);
        }
    }
}

struct pixelOutput_0
{
    uint4 output_0 [[color(0)]];
};

struct pixelInput_0
{
    [[flat]] uint draw_0 [[user(DRAW)]];
};

[[fragment]] pixelOutput_0 fs_terrain(pixelInput_0 _S36 [[stage_in]], uint prim_0 [[primitive_id]], float4 pos_0 [[position]], TerrainC_0 constant* ed_c_1 [[buffer(0)]], ulong device* ed_buffers_1 [[buffer(2)]])
{
    thread KernelContext_0 kernelContext_9;
    (&kernelContext_9)->ed_c_0 = ed_c_1;
    (&kernelContext_9)->ed_buffers_0 = ed_buffers_1;
    pixelOutput_0 _S37 = { uint4(2147483648U | (_S36.draw_0), prim_0 & 268435455U, 0U, 0U) };
    return _S37;
}

float t_own_h_1(uint _S38, TerrainDraw_natural_0 device* _S39, uint _S40, uint _S41, KernelContext_0 thread* kernelContext_10)
{
    float4 _S42 = float4(_S39->hq_0) ;
    float _S43 = t_page_0(_S38, (uint4(_S39->slots_0) ).x, _S40 * 4U, _S41 * 4U, _S42.x, _S42.y, kernelContext_10);
    return _S43;
}

float3 t_base_1(uint _S44, TerrainDraw_natural_0 device* _S45, uint _S46, uint _S47, KernelContext_0 thread* kernelContext_11)
{
    TerrainDraw_natural_0 _S48 = *_S45;
    float _S49 = t_own_h_1(_S44, _S45, _S46, _S47, kernelContext_11);
    thread TerrainDraw_natural_0 _S50 = _S48;
    thread float3 dir_3;
    float3 _S51 = t_lattice_0(&_S50, _S46, _S47, _S49, &dir_3);
    return _S51;
}

float3 t_coarse_even_1(uint _S52, TerrainDraw_natural_0 device* _S53, uint _S54, uint2 _S55, uint2 _S56, uint2 _S57, KernelContext_0 thread* kernelContext_12)
{
    thread TerrainDraw_natural_0 _S58 = *_S53;
    float3 _S59 = t_base_1(_S52, _S53, _S55.x, _S55.y, kernelContext_12);
    if((_S54 & 3U) == 0U)
    {
        return _S59;
    }
    float m_1 = t_morph_0((float4((&_S58)->origin_0) ).xyz + _S59, (float4((&_S58)->morph_0) ).zw);
    float3 _S60 = t_base_0(_S52, &_S58, _S56.x, _S56.y, kernelContext_12);
    float3 _S61 = t_base_0(_S52, &_S58, _S57.x, _S57.y, kernelContext_12);
    return mix(_S59, float3(0.5f)  * (_S60 + _S61), float3(m_1) );
}

float3 t_coarse_0(uint _S62, TerrainDraw_natural_0 device* _S63, uint _S64, uint _S65, KernelContext_0 thread* kernelContext_13)
{
    thread TerrainDraw_natural_0 _S66 = *_S63;
    if((_S65 & 1U) == 0U)
    {
        float3 _S67 = t_coarse_even_1(_S62, _S63, _S65, t_edge_at_0(_S64, _S65), t_edge_at_0(_S64, _S65 - 2U), t_edge_at_0(_S64, _S65 + 2U), kernelContext_13);
        return _S67;
    }
    uint _S68 = _S65 - 1U;
    uint2 _S69 = t_edge_at_0(_S64, _S68);
    uint _S70 = _S65 + 1U;
    uint2 _S71 = t_edge_at_0(_S64, _S70);
    float3 _S72 = t_coarse_even_1(_S62, _S63, _S68, _S69, t_edge_at_0(_S64, _S65 - 3U), _S71, kernelContext_13);
    float3 _S73 = t_coarse_even_0(_S62, &_S66, _S70, _S71, _S69, t_edge_at_0(_S64, _S65 + 3U), kernelContext_13);
    return float3(0.5f)  * (_S72 + _S73);
}

float3 t_grid_vertex_0(uint _S74, TerrainDraw_natural_0 device* _S75, uint _S76, uint _S77, KernelContext_0 thread* kernelContext_14)
{
    thread TerrainDraw_natural_0 _S78 = *_S75;
    uint coarse_0 = ((uint4((&_S78)->slots_0) ).z) & 15U;
    if(coarse_0 != 0U)
    {
        bool _S79;
        if((coarse_0 & 1U) != 0U)
        {
            _S79 = _S76 == 0U;
        }
        else
        {
            _S79 = false;
        }
        if(_S79)
        {
            float3 _S80 = t_coarse_0(_S74, _S75, 0U, _S77, kernelContext_14);
            return _S80;
        }
        if((coarse_0 & 2U) != 0U)
        {
            _S79 = _S76 == 64U;
        }
        else
        {
            _S79 = false;
        }
        if(_S79)
        {
            float3 _S81 = t_coarse_0(_S74, _S75, 1U, _S77, kernelContext_14);
            return _S81;
        }
        if((coarse_0 & 4U) != 0U)
        {
            _S79 = _S77 == 0U;
        }
        else
        {
            _S79 = false;
        }
        if(_S79)
        {
            float3 _S82 = t_coarse_0(_S74, _S75, 2U, _S76, kernelContext_14);
            return _S82;
        }
        if((coarse_0 & 8U) != 0U)
        {
            _S79 = _S77 == 64U;
        }
        else
        {
            _S79 = false;
        }
        if(_S79)
        {
            float3 _S83 = t_coarse_0(_S74, _S75, 3U, _S76, kernelContext_14);
            return _S83;
        }
    }
    float3 _S84 = t_base_1(_S74, _S75, _S76, _S77, kernelContext_14);
    float m_2 = t_morph_0((float4((&_S78)->origin_0) ).xyz + _S84, (float4((&_S78)->morph_0) ).xy);
    if(m_2 <= 0.0f)
    {
        return _S84;
    }
    float3 _S85 = t_target_0(_S74, &_S78, _S76, _S77, kernelContext_14);
    return mix(_S84, _S85, float3(m_2) );
}

float3 t_vertex_0(uint _S86, TerrainDraw_natural_0 device* _S87, uint _S88, KernelContext_0 thread* kernelContext_15)
{
    TerrainDraw_natural_0 _S89 = *_S87;
    if(_S88 < 4225U)
    {
        float3 _S90 = (float4(_S89.origin_0) ).xyz;
        float3 _S91 = t_grid_vertex_0(_S86, _S87, _S88 % 65U, _S88 / 65U, kernelContext_15);
        return _S90 + _S91;
    }
    uint2 ij_0 = t_ring_0(_S88 - 4225U);
    float3 _S92 = t_grid_vertex_0(_S86, _S87, ij_0.x, ij_0.y, kernelContext_15);
    float depth_0;
    if((((uint4(_S89.slots_0) ).z) & 64U) != 0U)
    {
        depth_0 = (float4(_S89.origin_0) ).w;
    }
    else
    {
        depth_0 = 0.0f;
    }
    float3 _S93 = (float4(_S89.origin_0) ).xyz;
    float3 _S94 = _S93 + _S92;
    float4 _S95 = float4(_S89.dir_c_0) ;
    return _S94 - normalize(_S94 - (_S93 - _S95.xyz * float3(_S95.w) )) * float3(depth_0) ;
}

struct vs_terrain_Result_0
{
    float4 pos_1 [[position]];
    uint draw_1 [[user(DRAW)]];
};

struct TerrainOut_0
{
    float4 pos_2;
    [[flat]] uint draw_2;
};

[[vertex]] vs_terrain_Result_0 vs_terrain(uint vid_0 [[vertex_id]], uint iid_0 [[instance_id]], TerrainC_0 constant* ed_c_2 [[buffer(0)]], ulong device* ed_buffers_2 [[buffer(2)]])
{
    thread KernelContext_0 kernelContext_16;
    (&kernelContext_16)->ed_c_0 = ed_c_2;
    (&kernelContext_16)->ed_buffers_0 = ed_buffers_2;
    FrameData_natural_0 device* _S96 = ed_buf_2(ed_c_2->frame_0, &kernelContext_16);
    TerrainDraw_natural_0 device* _S97 = ed_buf_1((&kernelContext_16)->ed_c_0->draws_0, &kernelContext_16);
    float3 _S98 = t_vertex_0((&kernelContext_16)->ed_c_0->pool_0, _S97 + iid_0, vid_0, &kernelContext_16);
    thread TerrainOut_0 o_0;
    (&o_0)->pos_2 = (((float4(_S98, 1.0f)) * (matrix<float,int(4),int(4)> (_S96->view_proj_0.data_0[int(0)][int(0)], _S96->view_proj_0.data_0[int(1)][int(0)], _S96->view_proj_0.data_0[int(2)][int(0)], _S96->view_proj_0.data_0[int(3)][int(0)], _S96->view_proj_0.data_0[int(0)][int(1)], _S96->view_proj_0.data_0[int(1)][int(1)], _S96->view_proj_0.data_0[int(2)][int(1)], _S96->view_proj_0.data_0[int(3)][int(1)], _S96->view_proj_0.data_0[int(0)][int(2)], _S96->view_proj_0.data_0[int(1)][int(2)], _S96->view_proj_0.data_0[int(2)][int(2)], _S96->view_proj_0.data_0[int(3)][int(2)], _S96->view_proj_0.data_0[int(0)][int(3)], _S96->view_proj_0.data_0[int(1)][int(3)], _S96->view_proj_0.data_0[int(2)][int(3)], _S96->view_proj_0.data_0[int(3)][int(3)]))));
    (&o_0)->draw_2 = iid_0;
    thread vs_terrain_Result_0 _S99;
    (&_S99)->pos_1 = o_0.pos_2;
    (&_S99)->draw_1 = o_0.draw_2;
    return _S99;
}

struct vs_terrain_shadow_Result_0
{
    float4 pos_3 [[position]];
};

struct TerrainShadowOut_0
{
    float4 pos_4;
};

[[vertex]] vs_terrain_shadow_Result_0 vs_terrain_shadow(uint vid_1 [[vertex_id]], uint iid_1 [[instance_id]], TerrainC_0 constant* ed_c_3 [[buffer(0)]], ulong device* ed_buffers_3 [[buffer(2)]])
{
    thread KernelContext_0 kernelContext_17;
    (&kernelContext_17)->ed_c_0 = ed_c_3;
    (&kernelContext_17)->ed_buffers_0 = ed_buffers_3;
    FrameData_natural_0 device* _S100 = ed_buf_2(ed_c_3->frame_0, &kernelContext_17);
    TerrainDraw_natural_0 device* _S101 = ed_buf_1((&kernelContext_17)->ed_c_0->draws_0, &kernelContext_17);
    float3 _S102 = t_vertex_0((&kernelContext_17)->ed_c_0->pool_0, _S101 + ((&kernelContext_17)->ed_c_0->base_0 + iid_1), vid_1, &kernelContext_17);
    thread TerrainShadowOut_0 o_1;
    (&o_1)->pos_4 = (((float4(_S102, 1.0f)) * (matrix<float,int(4),int(4)> ((&(&_S100->light_0)->shadow_0)->data_2[(&kernelContext_17)->ed_c_0->cascade_0].data_0[int(0)][int(0)], (&(&_S100->light_0)->shadow_0)->data_2[(&kernelContext_17)->ed_c_0->cascade_0].data_0[int(1)][int(0)], (&(&_S100->light_0)->shadow_0)->data_2[(&kernelContext_17)->ed_c_0->cascade_0].data_0[int(2)][int(0)], (&(&_S100->light_0)->shadow_0)->data_2[(&kernelContext_17)->ed_c_0->cascade_0].data_0[int(3)][int(0)], (&(&_S100->light_0)->shadow_0)->data_2[(&kernelContext_17)->ed_c_0->cascade_0].data_0[int(0)][int(1)], (&(&_S100->light_0)->shadow_0)->data_2[(&kernelContext_17)->ed_c_0->cascade_0].data_0[int(1)][int(1)], (&(&_S100->light_0)->shadow_0)->data_2[(&kernelContext_17)->ed_c_0->cascade_0].data_0[int(2)][int(1)], (&(&_S100->light_0)->shadow_0)->data_2[(&kernelContext_17)->ed_c_0->cascade_0].data_0[int(3)][int(1)], (&(&_S100->light_0)->shadow_0)->data_2[(&kernelContext_17)->ed_c_0->cascade_0].data_0[int(0)][int(2)], (&(&_S100->light_0)->shadow_0)->data_2[(&kernelContext_17)->ed_c_0->cascade_0].data_0[int(1)][int(2)], (&(&_S100->light_0)->shadow_0)->data_2[(&kernelContext_17)->ed_c_0->cascade_0].data_0[int(2)][int(2)], (&(&_S100->light_0)->shadow_0)->data_2[(&kernelContext_17)->ed_c_0->cascade_0].data_0[int(3)][int(2)], (&(&_S100->light_0)->shadow_0)->data_2[(&kernelContext_17)->ed_c_0->cascade_0].data_0[int(0)][int(3)], (&(&_S100->light_0)->shadow_0)->data_2[(&kernelContext_17)->ed_c_0->cascade_0].data_0[int(1)][int(3)], (&(&_S100->light_0)->shadow_0)->data_2[(&kernelContext_17)->ed_c_0->cascade_0].data_0[int(2)][int(3)], (&(&_S100->light_0)->shadow_0)->data_2[(&kernelContext_17)->ed_c_0->cascade_0].data_0[int(3)][int(3)]))));
    thread vs_terrain_shadow_Result_0 _S103;
    (&_S103)->pos_3 = o_1.pos_4;
    return _S103;
}

