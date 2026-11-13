#include <metal_stdlib>
#include <metal_math>
#include <metal_texture>
using namespace metal;
struct GpuImpKind_natural_0
{
    packed_uint4 tex_0;
    packed_float4 geo_0;
    packed_float4 leaf_0;
    packed_float4 bark_0;
};

struct VisC_0
{
    uint frame_0;
    uint instances_0;
    uint nodes_0;
    uint visible_0;
    uint verts_0;
    uint cascade_0;
    uint materials_0;
    uint uvs_0;
    uint smp_0;
    array<uint, int(7)> pad_0;
};

struct KernelContext_0
{
    VisC_0 constant* ed_c_0;
    ulong device* ed_buffers_0;
    ulong device* ed_textures_0;
    ulong device* ed_samplers_0;
};

GpuImpKind_natural_0 device* ed_buf_0(uint i_0, KernelContext_0 thread* kernelContext_0)
{
    return (GpuImpKind_natural_0 device*)kernelContext_0->ed_buffers_0[i_0];
}

struct GpuTree_0
{
    float px_0;
    float py_0;
    float pz_0;
    uint node_kind_0;
    float scale_0;
    uint up_0;
    uint yaw_widen_0;
    uint user_0;
};

GpuTree_0 device* ed_buf_1(uint i_1, KernelContext_0 thread* kernelContext_1)
{
    return (GpuTree_0 device*)kernelContext_1->ed_buffers_0[i_1];
}

struct GpuMaterial_natural_0
{
    packed_float4 base_0;
    packed_float4 params_0;
    packed_float4 shade_0;
    packed_uint4 tex_1;
    packed_float4 uv_0;
};

GpuMaterial_natural_0 device* ed_buf_2(uint i_2, KernelContext_0 thread* kernelContext_2)
{
    return (GpuMaterial_natural_0 device*)kernelContext_2->ed_buffers_0[i_2];
}

packed_uint2 device* ed_buf_3(uint i_3, KernelContext_0 thread* kernelContext_3)
{
    return (packed_uint2 device*)kernelContext_3->ed_buffers_0[i_3];
}

constant array<uint, int(64)> BAYER8_0 = { { 0U, 32U, 8U, 40U, 2U, 34U, 10U, 42U, 48U, 16U, 56U, 24U, 50U, 18U, 58U, 26U, 12U, 44U, 4U, 36U, 14U, 46U, 6U, 38U, 60U, 28U, 52U, 20U, 62U, 30U, 54U, 22U, 3U, 35U, 11U, 43U, 1U, 33U, 9U, 41U, 51U, 19U, 59U, 27U, 49U, 17U, 57U, 25U, 15U, 47U, 7U, 39U, 13U, 45U, 5U, 37U, 63U, 31U, 55U, 23U, 61U, 29U, 53U, 21U } };
packed_float4 device* ed_buf_4(uint i_4, KernelContext_0 thread* kernelContext_4)
{
    return (packed_float4 device*)kernelContext_4->ed_buffers_0[i_4];
}

struct Vtx_0
{
    float px_1;
    float py_1;
    float pz_1;
    uint n_0;
};

Vtx_0 device* ed_buf_5(uint i_5, KernelContext_0 thread* kernelContext_5)
{
    return (Vtx_0 device*)kernelContext_5->ed_buffers_0[i_5];
}

struct _Array_natural_vectorx3Cfloatx2C4x3E3_0
{
    array<packed_float4, int(3)> data_0;
};

struct GpuInstance_natural_0
{
    _Array_natural_vectorx3Cfloatx2C4x3E3_0 m_0;
    _Array_natural_vectorx3Cfloatx2C4x3E3_0 prev_0;
    uint mesh_0;
    uint material_0;
    uint object_lo_0;
    uint node_flags_0;
    array<uint, int(4)> pad_1;
};

GpuInstance_natural_0 device* ed_buf_6(uint i_6, KernelContext_0 thread* kernelContext_6)
{
    return (GpuInstance_natural_0 device*)kernelContext_6->ed_buffers_0[i_6];
}

uint device* ed_buf_7(uint i_7, KernelContext_0 thread* kernelContext_7)
{
    return (uint device*)kernelContext_7->ed_buffers_0[i_7];
}

struct _MatrixStorage_float4x4_ColMajornatural_0
{
    array<packed_float4, int(4)> data_1;
};

struct _Array_natural_vectorx3Cfloatx2C4x3E5_0
{
    array<packed_float4, int(5)> data_2;
};

struct HzbLevel_0
{
    uint offset_0;
    uint width_0;
    uint height_0;
    uint pad_2;
};

struct _Array_natural_matrixx3Cfloatx2C4x2C4x3E4_0
{
    array<_MatrixStorage_float4x4_ColMajornatural_0, int(4)> data_3;
};

struct _Array_natural_vectorx3Cfloatx2C4x3E4_0
{
    array<packed_float4, int(4)> data_4;
};

struct _Array_natural_vectorx3Cfloatx2C4x3E9_0
{
    array<packed_float4, int(9)> data_5;
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
    packed_float4 up_1;
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
    packed_float4 params_1;
    packed_float4 proj_0;
    packed_uint4 cull_0;
    array<HzbLevel_0, int(16)> hzb_0;
    LightData_natural_0 light_0;
    AtmosData_natural_0 atmos_0;
    WindData_natural_0 wind_1;
    ImpData_natural_0 imp_0;
};

FrameData_natural_0 device* ed_buf_8(uint i_8, KernelContext_0 thread* kernelContext_8)
{
    return (FrameData_natural_0 device*)kernelContext_8->ed_buffers_0[i_8];
}

float3 xform_point_0(const GpuInstance_natural_0 thread* i_9, float3 p_0)
{
    float4 _S1 = float4((&i_9->m_0)->data_0[int(0)]) ;
    float4 _S2 = float4((&i_9->m_0)->data_0[int(1)]) ;
    float4 _S3 = float4((&i_9->m_0)->data_0[int(2)]) ;
    return float3(dot(_S1.xyz, p_0) + _S1.w, dot(_S2.xyz, p_0) + _S2.w, dot(_S3.xyz, p_0) + _S3.w);
}

uint4 vis_value_0(uint entry_0, uint prim_0)
{
    return uint4((entry_0 & 1048575U) + 1U, (((entry_0 >> 20U) & 3U) << 28U) | (prim_0 & 268435455U), 0U, 0U);
}

struct pixelOutput_0
{
    uint4 output_0 [[color(0)]];
};

struct pixelInput_0
{
    [[flat]] uint entry_1 [[user(ENTRY)]];
};

[[fragment]] pixelOutput_0 fs_vis(pixelInput_0 _S4 [[stage_in]], uint prim_1 [[primitive_id]], float4 pos_0 [[position]], VisC_0 constant* ed_c_1 [[buffer(0)]], ulong device* ed_buffers_1 [[buffer(2)]], ulong device* ed_textures_1 [[buffer(1)]], ulong device* ed_samplers_1 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_9;
    (&kernelContext_9)->ed_c_0 = ed_c_1;
    (&kernelContext_9)->ed_buffers_0 = ed_buffers_1;
    (&kernelContext_9)->ed_textures_0 = ed_textures_1;
    (&kernelContext_9)->ed_samplers_0 = ed_samplers_1;
    pixelOutput_0 _S5 = { vis_value_0(_S4.entry_1, prim_1) };
    return _S5;
}

struct pixelOutput_1
{
    uint4 output_1 [[color(0)]];
};

struct pixelInput_1
{
    [[flat]] uint entry_2 [[user(ENTRY)]];
};

[[fragment]] pixelOutput_1 fs_vis_dither(pixelInput_1 _S6 [[stage_in]], uint prim_2 [[primitive_id]], float4 pos_1 [[position]], VisC_0 constant* ed_c_2 [[buffer(0)]], ulong device* ed_buffers_2 [[buffer(2)]], ulong device* ed_textures_2 [[buffer(1)]], ulong device* ed_samplers_2 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_10;
    (&kernelContext_10)->ed_c_0 = ed_c_2;
    (&kernelContext_10)->ed_buffers_0 = ed_buffers_2;
    (&kernelContext_10)->ed_textures_0 = ed_textures_2;
    (&kernelContext_10)->ed_samplers_0 = ed_samplers_2;
    uint2 p_1 = uint2(pos_1.xy) & (uint2(7U) );
    if(((((BAYER8_0[p_1.y * 8U + p_1.x] + ((((_S6.entry_2) & 1048575U) * 2654435761U) >> 26U)) & 63U) * 4U + 2U) < (((_S6.entry_2) >> 22U) & 255U)) == ((((_S6.entry_2) >> 30U) & 1U) != 0U))
    {
        discard_fragment();
    }
    pixelOutput_1 _S7 = { vis_value_0(_S6.entry_2, prim_2) };
    return _S7;
}

float length_squared_0(float3 v_0)
{
    return dot(v_0, v_0);
}

float max_scale_0(const GpuInstance_natural_0 thread* i_10)
{
    float4 _S8 = float4((&i_10->m_0)->data_0[int(0)]) ;
    float4 _S9 = float4((&i_10->m_0)->data_0[int(1)]) ;
    float4 _S10 = float4((&i_10->m_0)->data_0[int(2)]) ;
    return sqrt(max(max(length_squared_0(float3(_S8.x, _S9.x, _S10.x)), length_squared_0(float3(_S8.y, _S9.y, _S10.y))), length_squared_0(float3(_S8.z, _S9.z, _S10.z))));
}

float2 unpack_snorm2x16_0(uint p_2)
{
    return clamp(float2(int2(int(p_2 << 16U) >> 16U, int(p_2) >> 16U)) / float2(32767.0f) , float2(-1.0f) , float2(1.0f) );
}

float3 oct_decode_0(uint packed_0)
{
    float2 e_0 = unpack_snorm2x16_0(packed_0);
    float _S11 = e_0.x;
    float _S12 = e_0.y;
    float _S13 = 1.0f - abs(_S11) - abs(_S12);
    thread float3 n_1 = float3(_S11, _S12, _S13);
    float t_0 = saturate(- _S13);
    float _S14;
    if(_S11 >= 0.0f)
    {
        _S14 = - t_0;
    }
    else
    {
        _S14 = t_0;
    }
    n_1.x = n_1.x + _S14;
    if((n_1.y) >= 0.0f)
    {
        _S14 = - t_0;
    }
    else
    {
        _S14 = t_0;
    }
    n_1.y = n_1.y + _S14;
    return normalize(n_1);
}

float3 xform_normal_0(const GpuInstance_natural_0 thread* i_11, float3 n_2)
{
    float4 _S15 = float4((&i_11->m_0)->data_0[int(0)]) ;
    float4 _S16 = float4((&i_11->m_0)->data_0[int(1)]) ;
    float4 _S17 = float4((&i_11->m_0)->data_0[int(2)]) ;
    float3 a0_0 = float3(_S15.x, _S16.x, _S17.x);
    float3 a1_0 = float3(_S15.y, _S16.y, _S17.y);
    float3 a2_0 = float3(_S15.z, _S16.z, _S17.z);
    return cross(a1_0, a2_0) * float3(n_2.x)  + cross(a2_0, a0_0) * float3(n_2.y)  + cross(a0_0, a1_0) * float3(n_2.z) ;
}

float3 tree_vertex_0(FrameData_natural_0 device* f_0, const GpuInstance_natural_0 thread* inst_0, float3 node_0, float3 pm_0, uint n_3, uint aux_0)
{
    uint _S18 = inst_0->pad_1[int(0)];
    float3 origin_0 = float3((float4((&inst_0->m_0)->data_0[int(0)]) ).w, (float4((&inst_0->m_0)->data_0[int(1)]) ).w, (float4((&inst_0->m_0)->data_0[int(2)]) ).w);
    bool _S19;
    if((aux_0 & 134217728U) != 0U)
    {
        _S19 = ((_S18 >> 16U) & (1U << (aux_0 >> 29U))) != 0U;
    }
    else
    {
        _S19 = false;
    }
    if(_S19)
    {
        return origin_0;
    }
    float3 _S20 = xform_point_0(inst_0, pm_0);
    WindData_natural_0 w_0 = f_0->wind_1;
    float4 _S21 = float4(f_0->wind_1.cfg_2) ;
    if((_S21.x) < 0.5f)
    {
        return _S20;
    }
    float3 _S22 = node_0 + origin_0;
    float _S23 = _S21.y;
    float fade_0 = saturate((_S23 - length(_S22)) / max(0.25f * _S23, 1.0f));
    if(fade_0 <= 0.0f)
    {
        return _S20;
    }
    float q_0 = float((aux_0 >> 11U) & 127U) / 127.0f;
    float sway_0 = q_0 * q_0 * 4.0f;
    float flutter_0 = float((aux_0 >> 18U) & 7U) / 7.0f;
    float phase_0 = float((aux_0 >> 21U) & 63U) / 64.0f + float(_S18 >> 24U) / 255.0f;
    float4 _S24 = float4(w_0.wind_0) ;
    float2 _S25 = _S24.xz;
    float speed_0 = length(_S25);
    float2 wd_0;
    if(speed_0 > 0.0010000000474974513f)
    {
        wd_0 = _S25 / float2(speed_0) ;
    }
    else
    {
        wd_0 = float2(0.0f, 0.0f);
    }
    float4 _S26 = float4(w_0.gust_0) ;
    float s_0 = 0.5f + 0.5f * sin((_S26.x + dot(_S22.xz, wd_0) / max(_S26.y, 1.0f)) * 6.2831854820251465f);
    float gust_1 = s_0 * s_0 * s_0 * _S26.z;
    float t_1 = _S24.w;
    float _S27 = speed_0 * 0.10000000149011612f * fade_0;
    float _S28 = max_scale_0(inst_0);
    float k_0 = _S27 * _S28;
    float rock_0 = 0.55000001192092896f + gust_1 + 0.30000001192092896f * sin(t_1 * 1.7000000476837158f + phase_0 * 6.2831854820251465f) + 0.15000000596046448f * sin(t_1 * 0.62999999523162842f + phase_0 * 19.0f);
    thread float3 d_0 = float3(wd_0.x, 0.0f, wd_0.y) * float3((sway_0 * k_0 * rock_0)) ;
    d_0.y = d_0.y - 0.25f * sway_0 * k_0 * rock_0 * rock_0 * 0.30000001192092896f;
    float3 _S29 = xform_normal_0(inst_0, oct_decode_0(n_3));
    return _S20 + d_0 + normalize(_S29) * float3((flutter_0 * _S26.w * (0.60000002384185791f + gust_1) * 0.05000000074505806f * fade_0 * sin(t_1 * 11.0f + phase_0 * 40.0f + dot(_S20, float3(3.0999999046325684f, 1.7000000476837158f, 2.2999999523162842f)))))  * float3(_S28) ;
}

float2 tree_uv_0(uint w_1)
{
    return float2(float(w_1 & 65535U), float(w_1 >> 16U)) * float2(1.5259021893143654e-05f) ;
}

struct FoliageOut_0
{
    float4 pos_2;
    float2 uv_1;
    [[flat]] uint entry_3;
    [[flat]] uint tex_2;
};

FoliageOut_0 foliage_vertex_0(uint vid_0, uint iid_0, bool shadow_1, KernelContext_0 thread* kernelContext_11)
{
    FrameData_natural_0 device* _S30 = ed_buf_8(kernelContext_11->ed_c_0->frame_0, kernelContext_11);
    uint device* _S31 = ed_buf_7(kernelContext_11->ed_c_0->visible_0, kernelContext_11);
    uint entry_4 = *(_S31 + iid_0);
    GpuInstance_natural_0 device* _S32 = ed_buf_6(kernelContext_11->ed_c_0->instances_0, kernelContext_11);
    GpuInstance_natural_0 device* _S33 = _S32 + (entry_4 & 1048575U);
    GpuInstance_natural_0 inst_1 = *_S33;
    thread GpuInstance_natural_0 _S34 = *_S33;
    Vtx_0 device* _S35 = ed_buf_5(kernelContext_11->ed_c_0->verts_0, kernelContext_11);
    Vtx_0 v_1 = *(_S35 + vid_0);
    packed_uint2 device* _S36 = ed_buf_3(kernelContext_11->ed_c_0->uvs_0, kernelContext_11);
    uint2 _S37 = uint2(*(_S36 + vid_0)) ;
    packed_float4 device* _S38 = ed_buf_4(kernelContext_11->ed_c_0->nodes_0, kernelContext_11);
    float3 node_1 = (float4(*(_S38 + (((&_S34)->node_flags_0) & 65535U))) ).xyz;
    float3 _S39 = float3(v_1.px_1, v_1.py_1, v_1.pz_1);
    uint _S40 = _S37.y;
    _S34 = inst_1;
    float3 _S41 = tree_vertex_0(_S30, &_S34, node_1, _S39, v_1.n_0, _S40);
    float3 p_3 = node_1 + _S41;
    thread FoliageOut_0 o_0;
    matrix<float,int(4),int(4)>  _S42;
    if(shadow_1)
    {
        _S42 = matrix<float,int(4),int(4)> ((&(&_S30->light_0)->shadow_0)->data_3[kernelContext_11->ed_c_0->cascade_0].data_1[int(0)][int(0)], (&(&_S30->light_0)->shadow_0)->data_3[kernelContext_11->ed_c_0->cascade_0].data_1[int(1)][int(0)], (&(&_S30->light_0)->shadow_0)->data_3[kernelContext_11->ed_c_0->cascade_0].data_1[int(2)][int(0)], (&(&_S30->light_0)->shadow_0)->data_3[kernelContext_11->ed_c_0->cascade_0].data_1[int(3)][int(0)], (&(&_S30->light_0)->shadow_0)->data_3[kernelContext_11->ed_c_0->cascade_0].data_1[int(0)][int(1)], (&(&_S30->light_0)->shadow_0)->data_3[kernelContext_11->ed_c_0->cascade_0].data_1[int(1)][int(1)], (&(&_S30->light_0)->shadow_0)->data_3[kernelContext_11->ed_c_0->cascade_0].data_1[int(2)][int(1)], (&(&_S30->light_0)->shadow_0)->data_3[kernelContext_11->ed_c_0->cascade_0].data_1[int(3)][int(1)], (&(&_S30->light_0)->shadow_0)->data_3[kernelContext_11->ed_c_0->cascade_0].data_1[int(0)][int(2)], (&(&_S30->light_0)->shadow_0)->data_3[kernelContext_11->ed_c_0->cascade_0].data_1[int(1)][int(2)], (&(&_S30->light_0)->shadow_0)->data_3[kernelContext_11->ed_c_0->cascade_0].data_1[int(2)][int(2)], (&(&_S30->light_0)->shadow_0)->data_3[kernelContext_11->ed_c_0->cascade_0].data_1[int(3)][int(2)], (&(&_S30->light_0)->shadow_0)->data_3[kernelContext_11->ed_c_0->cascade_0].data_1[int(0)][int(3)], (&(&_S30->light_0)->shadow_0)->data_3[kernelContext_11->ed_c_0->cascade_0].data_1[int(1)][int(3)], (&(&_S30->light_0)->shadow_0)->data_3[kernelContext_11->ed_c_0->cascade_0].data_1[int(2)][int(3)], (&(&_S30->light_0)->shadow_0)->data_3[kernelContext_11->ed_c_0->cascade_0].data_1[int(3)][int(3)]);
    }
    else
    {
        _S42 = matrix<float,int(4),int(4)> (_S30->view_proj_0.data_1[int(0)][int(0)], _S30->view_proj_0.data_1[int(1)][int(0)], _S30->view_proj_0.data_1[int(2)][int(0)], _S30->view_proj_0.data_1[int(3)][int(0)], _S30->view_proj_0.data_1[int(0)][int(1)], _S30->view_proj_0.data_1[int(1)][int(1)], _S30->view_proj_0.data_1[int(2)][int(1)], _S30->view_proj_0.data_1[int(3)][int(1)], _S30->view_proj_0.data_1[int(0)][int(2)], _S30->view_proj_0.data_1[int(1)][int(2)], _S30->view_proj_0.data_1[int(2)][int(2)], _S30->view_proj_0.data_1[int(3)][int(2)], _S30->view_proj_0.data_1[int(0)][int(3)], _S30->view_proj_0.data_1[int(1)][int(3)], _S30->view_proj_0.data_1[int(2)][int(3)], _S30->view_proj_0.data_1[int(3)][int(3)]);
    }
    (&o_0)->pos_2 = (((float4(p_3, 1.0f)) * (_S42)));
    (&o_0)->uv_1 = tree_uv_0(_S37.x);
    (&o_0)->entry_3 = entry_4;
    uint _S43;
    if((_S40 & 134217728U) != 0U)
    {
        GpuMaterial_natural_0 device* _S44 = ed_buf_2(kernelContext_11->ed_c_0->materials_0, kernelContext_11);
        _S43 = (uint4((_S44 + (&_S34)->material_0)->tex_1) ).x;
    }
    else
    {
        _S43 = 4294967295U;
    }
    (&o_0)->tex_2 = _S43;
    return o_0;
}

void foliage_clip_0(const FoliageOut_0 thread* i_12, KernelContext_0 thread* kernelContext_12)
{
    uint _S45 = i_12->tex_2;
    bool _S46;
    if((i_12->tex_2) != 4294967295U)
    {
        texture2d<float, access::sample> _S47 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_12->ed_textures_0))[(_S45)]));
        sampler _S48 = ((reinterpret_cast<device sampler*>((kernelContext_12->ed_samplers_0))[(kernelContext_12->ed_c_0->smp_0)]));
        _S46 = (((_S47).sample((_S48), (i_12->uv_1))).w) < 0.5f;
    }
    else
    {
        _S46 = false;
    }
    if(_S46)
    {
        discard_fragment();
    }
    return;
}

struct pixelOutput_2
{
    uint4 output_2 [[color(0)]];
};

struct pixelInput_2
{
    float2 uv_2 [[user(TEXCOORD)]];
    [[flat]] uint entry_5 [[user(ENTRY)]];
    [[flat]] uint tex_3 [[user(TEXID)]];
};

[[fragment]] pixelOutput_2 fs_foliage(pixelInput_2 _S49 [[stage_in]], uint prim_3 [[primitive_id]], float4 pos_3 [[position]], VisC_0 constant* ed_c_3 [[buffer(0)]], ulong device* ed_buffers_3 [[buffer(2)]], ulong device* ed_textures_3 [[buffer(1)]], ulong device* ed_samplers_3 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_13;
    (&kernelContext_13)->ed_c_0 = ed_c_3;
    (&kernelContext_13)->ed_buffers_0 = ed_buffers_3;
    (&kernelContext_13)->ed_textures_0 = ed_textures_3;
    (&kernelContext_13)->ed_samplers_0 = ed_samplers_3;
    thread FoliageOut_0 _S50;
    (&_S50)->pos_2 = pos_3;
    (&_S50)->uv_1 = _S49.uv_2;
    (&_S50)->entry_3 = _S49.entry_5;
    (&_S50)->tex_2 = _S49.tex_3;
    foliage_clip_0(&_S50, &kernelContext_13);
    pixelOutput_2 _S51 = { vis_value_0(_S49.entry_5, prim_3) };
    return _S51;
}

struct pixelOutput_3
{
    uint4 output_3 [[color(0)]];
};

struct pixelInput_3
{
    float2 uv_3 [[user(TEXCOORD)]];
    [[flat]] uint entry_6 [[user(ENTRY)]];
    [[flat]] uint tex_4 [[user(TEXID)]];
};

[[fragment]] pixelOutput_3 fs_foliage_dither(pixelInput_3 _S52 [[stage_in]], uint prim_4 [[primitive_id]], float4 pos_4 [[position]], VisC_0 constant* ed_c_4 [[buffer(0)]], ulong device* ed_buffers_4 [[buffer(2)]], ulong device* ed_textures_4 [[buffer(1)]], ulong device* ed_samplers_4 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_14;
    (&kernelContext_14)->ed_c_0 = ed_c_4;
    (&kernelContext_14)->ed_buffers_0 = ed_buffers_4;
    (&kernelContext_14)->ed_textures_0 = ed_textures_4;
    (&kernelContext_14)->ed_samplers_0 = ed_samplers_4;
    thread FoliageOut_0 _S53;
    (&_S53)->pos_2 = pos_4;
    (&_S53)->uv_1 = _S52.uv_3;
    (&_S53)->entry_3 = _S52.entry_6;
    (&_S53)->tex_2 = _S52.tex_4;
    foliage_clip_0(&_S53, &kernelContext_14);
    uint2 p_4 = uint2(pos_4.xy) & (uint2(7U) );
    if(((((BAYER8_0[p_4.y * 8U + p_4.x] + ((((_S52.entry_6) & 1048575U) * 2654435761U) >> 26U)) & 63U) * 4U + 2U) < (((_S52.entry_6) >> 22U) & 255U)) == ((((_S52.entry_6) >> 30U) & 1U) != 0U))
    {
        discard_fragment();
    }
    pixelOutput_3 _S54 = { vis_value_0(_S52.entry_6, prim_4) };
    return _S54;
}

struct pixelInput_4
{
    float2 uv_4 [[user(TEXCOORD)]];
    [[flat]] uint entry_7 [[user(ENTRY)]];
    [[flat]] uint tex_5 [[user(TEXID)]];
};

[[fragment]] void fs_shadow_foliage(pixelInput_4 _S55 [[stage_in]], float4 pos_5 [[position]], VisC_0 constant* ed_c_5 [[buffer(0)]], ulong device* ed_buffers_5 [[buffer(2)]], ulong device* ed_textures_5 [[buffer(1)]], ulong device* ed_samplers_5 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_15;
    (&kernelContext_15)->ed_c_0 = ed_c_5;
    (&kernelContext_15)->ed_buffers_0 = ed_buffers_5;
    (&kernelContext_15)->ed_textures_0 = ed_textures_5;
    (&kernelContext_15)->ed_samplers_0 = ed_samplers_5;
    thread FoliageOut_0 _S56;
    (&_S56)->pos_2 = pos_5;
    (&_S56)->uv_1 = _S55.uv_4;
    (&_S56)->entry_3 = _S55.entry_7;
    (&_S56)->tex_2 = _S55.tex_5;
    foliage_clip_0(&_S56, &kernelContext_15);
    return;
}

float3 arc_from_y_0(float3 up_2, float3 v_2)
{
    float3 a_0 = float3(up_2.z, 0.0f, - up_2.x);
    float w_2 = 1.0f + up_2.y;
    float l_0 = sqrt(dot(a_0, a_0) + w_2 * w_2);
    if(l_0 < 9.99999997475242708e-07f)
    {
        return float3(v_2.x, - v_2.y, - v_2.z);
    }
    float3 a_1 = a_0 / float3(l_0) ;
    float3 t_2 = float3(2.0f)  * cross(a_1, v_2);
    return v_2 + float3((w_2 / l_0))  * t_2 + cross(a_1, t_2);
}

struct ImpTree_0
{
    float3 c_0;
    float3 ex_0;
    float3 up_3;
    float3 ez_0;
    float s_1;
    float wd_1;
    float r_0;
    float3 cam_0;
    uint3 fi_0;
    float3 w_3;
    float3 d0_0;
    float3 d1_0;
    float3 d2_0;
};

float3 imp_to_tree_0(const ImpTree_0 thread* t_3, float3 v_3)
{
    float _S57 = t_3->s_1 * t_3->wd_1;
    return float3(dot(v_3, t_3->ex_0) / _S57, dot(v_3, t_3->up_3) / t_3->s_1, dot(v_3, t_3->ez_0) / _S57);
}

float2 imp_oct_encode_0(float3 d_1)
{
    thread float3 _S58 = d_1;
    _S58.y = max(d_1.y, 0.0f);
    float3 _S59 = _S58 / float3(max(abs(_S58.x) + abs(_S58.y) + abs(_S58.z), 9.99999971718068537e-10f)) ;
    _S58 = _S59;
    float _S60 = _S59.x;
    float _S61 = _S59.z;
    float2 _S62 = float2(0.5f) ;
    return float2(_S60 + _S61, _S60 - _S61) * _S62 + _S62;
}

float3 imp_oct_decode_0(float2 uv_5)
{
    float2 g_0 = uv_5 * float2(2.0f)  - float2(1.0f) ;
    float _S63 = g_0.x;
    float _S64 = g_0.y;
    float x_0 = (_S63 + _S64) * 0.5f;
    float z_0 = (_S63 - _S64) * 0.5f;
    return normalize(float3(x_0, 1.0f - abs(x_0) - abs(z_0), z_0));
}

ImpTree_0 imp_tree_0(const GpuTree_0 thread* g_1, const GpuImpKind_natural_0 thread* k_1, float3 node_2, float3 sun_dir_1, bool sun_1)
{
    thread ImpTree_0 t_4;
    float3 _S65 = oct_decode_0(g_1->up_0);
    (&t_4)->up_3 = _S65;
    float yaw_0 = float((g_1->yaw_widen_0) & 65535U) / 65535.0f * 6.2831854820251465f;
    (&t_4)->wd_1 = 0.5f + float((g_1->yaw_widen_0) >> 16U) / 65535.0f;
    (&t_4)->s_1 = g_1->scale_0;
    float _S66 = cos(yaw_0);
    float _S67 = sin(yaw_0);
    (&t_4)->ex_0 = arc_from_y_0(_S65, float3(_S66, 0.0f, - _S67));
    (&t_4)->ez_0 = arc_from_y_0(_S65, float3(_S67, 0.0f, _S66));
    float4 _S68 = float4(k_1->geo_0) ;
    (&t_4)->r_0 = _S68.y;
    (&t_4)->c_0 = node_2 + float3(g_1->px_0, g_1->py_0, g_1->pz_0) + _S65 * float3((_S68.x * g_1->scale_0)) ;
    ImpTree_0 _S69 = t_4;
    float3 _S70;
    if(sun_1)
    {
        _S70 = sun_dir_1 * float3(1.0e+05f) ;
    }
    else
    {
        _S70 = - (&t_4)->c_0;
    }
    thread ImpTree_0 _S71 = _S69;
    float3 _S72 = imp_to_tree_0(&_S71, _S70);
    (&t_4)->cam_0 = _S72;
    uint _S73 = (uint4(k_1->tex_0) ).w;
    float n_4 = float(_S73);
    float2 _S74 = float2(n_4) ;
    float2 _S75 = float2(0.5f) ;
    float _S76 = n_4 - 1.0f;
    float2 gpos_0 = clamp(imp_oct_encode_0(normalize(_S72)) * _S74 - _S75, float2(0.0f, 0.0f), float2(_S76, _S76));
    float _S77 = n_4 - 2.0f;
    float2 _S78 = min(floor(gpos_0), float2(_S77, _S77));
    float2 fr_0 = gpos_0 - _S78;
    uint2 _S79 = uint2(_S78);
    uint2 c1_0 = _S79 + uint2(1U, 0U);
    uint2 c2_0 = _S79 + uint2(0U, 1U);
    float _S80 = fr_0.x;
    float _S81 = fr_0.y;
    float _S82 = _S80 + _S81;
    uint2 c0_0;
    if(_S82 < 1.0f)
    {
        (&t_4)->w_3 = float3(1.0f - _S80 - _S81, _S80, _S81);
        c0_0 = _S79;
    }
    else
    {
        uint2 _S83 = _S79 + uint2(1U, 1U);
        (&t_4)->w_3 = float3(_S82 - 1.0f, 1.0f - _S81, 1.0f - _S80);
        c0_0 = _S83;
    }
    (&t_4)->fi_0 = uint3(c0_0.y * _S73 + c0_0.x, c1_0.y * _S73 + c1_0.x, c2_0.y * _S73 + c2_0.x);
    (&t_4)->d0_0 = imp_oct_decode_0((float2(c0_0) + _S75) / _S74);
    (&t_4)->d1_0 = imp_oct_decode_0((float2(c1_0) + _S75) / _S74);
    (&t_4)->d2_0 = imp_oct_decode_0((float2(c2_0) + _S75) / _S74);
    return t_4;
}

void imp_view_basis_0(float3 v_4, float3 thread* right_0, float3 thread* up_4)
{
    float3 u0_0 = float3(0.0f, 1.0f, 0.0f) - v_4 * float3(v_4.y) ;
    float3 _S84;
    if((dot(u0_0, u0_0)) > 9.99999997475242708e-07f)
    {
        _S84 = normalize(u0_0);
    }
    else
    {
        _S84 = float3(0.0f, 0.0f, 1.0f);
    }
    *up_4 = _S84;
    *right_0 = normalize(cross(_S84, v_4));
    return;
}

float2 imp_frame_uv_0(const ImpTree_0 thread* t_5, float3 d_2, float3 q_1, float half_size_0)
{
    float3 _S85 = t_5->cam_0;
    float3 dir_0 = q_1 - t_5->cam_0;
    float den_0 = dot(dir_0, d_2);
    float k_2;
    if((abs(den_0)) > 9.99999997475242708e-07f)
    {
        k_2 = - dot(_S85, d_2) / den_0;
    }
    else
    {
        k_2 = 1.0f;
    }
    float3 h_0 = _S85 + dir_0 * float3(k_2) ;
    thread float3 r_1;
    thread float3 u_0;
    imp_view_basis_0(d_2, &r_1, &u_0);
    float _S86 = 2.0f * half_size_0;
    return float2(0.5f + dot(h_0, r_1) / _S86, 0.5f - dot(h_0, u_0) / _S86);
}

struct ImpOut_0
{
    float4 pos_6;
    float2 uv0_0;
    float2 uv1_0;
    float2 uv2_0;
    float zv_0;
    [[flat]] uint entry_8;
    [[flat]] uint4 fi_1;
    [[flat]] float4 w_4;
    [[flat]] float2 geo_1;
    [[flat]] float front_0;
};

ImpOut_0 imp_vertex_0(uint vid_1, uint iid_1, bool shadow_2, KernelContext_0 thread* kernelContext_16)
{
    FrameData_natural_0 device* _S87 = ed_buf_8(kernelContext_16->ed_c_0->frame_0, kernelContext_16);
    uint device* _S88 = ed_buf_7(kernelContext_16->ed_c_0->visible_0, kernelContext_16);
    uint entry_9 = *(_S88 + iid_1);
    GpuTree_0 device* _S89 = ed_buf_1(kernelContext_16->ed_c_0->instances_0, kernelContext_16);
    GpuTree_0 g_2 = *(_S89 + (entry_9 & 16777215U));
    GpuImpKind_natural_0 device* _S90 = ed_buf_0(kernelContext_16->ed_c_0->verts_0, kernelContext_16);
    GpuImpKind_natural_0 device* _S91 = _S90 + (((g_2.node_kind_0) >> 12U) & 4095U);
    GpuImpKind_natural_0 k_3 = *_S91;
    thread GpuImpKind_natural_0 _S92 = *_S91;
    packed_float4 device* _S93 = ed_buf_4(kernelContext_16->ed_c_0->nodes_0, kernelContext_16);
    float3 node_3 = (float4(*(_S93 + ((g_2.node_kind_0) & 4095U))) ).xyz;
    float3 l_1 = (float4(_S87->sun_dir_0) ).xyz;
    thread GpuTree_0 _S94 = g_2;
    _S92 = k_3;
    ImpTree_0 _S95 = imp_tree_0(&_S94, &_S92, node_3, l_1, shadow_2);
    float3 vw_0;
    if(shadow_2)
    {
        vw_0 = l_1;
    }
    else
    {
        vw_0 = normalize(- _S95.c_0);
    }
    float3 u0_1 = _S95.up_3 - vw_0 * float3(dot(vw_0, _S95.up_3)) ;
    float3 bu_0;
    if((dot(u0_1, u0_1)) > 9.99999997475242708e-07f)
    {
        bu_0 = normalize(u0_1);
    }
    else
    {
        bu_0 = _S95.ex_0;
    }
    float3 br_0 = normalize(cross(bu_0, vw_0));
    float pad_3 = (float4((&_S87->imp_0)->cfg_3) ).y;
    float half_t_0 = _S95.r_0 * pad_3;
    float half_w_0 = half_t_0 * _S95.s_1 * max(_S95.wd_1, 1.0f);
    float4 _S96 = float4((&_S92)->geo_0) ;
    float crown_max_0 = float((as_type<uint>(((float4((&_S92)->bark_0) ).w))) >> 21U) / 2047.0f * 1.5f * _S96.z;
    float half_x_0;
    if(crown_max_0 > 0.0f)
    {
        half_x_0 = min(half_w_0, crown_max_0 * 1.0199999809265137f * pad_3 * _S95.s_1 * _S95.wd_1);
    }
    else
    {
        half_x_0 = half_w_0;
    }
    uint corner_0 = vid_1 & 3U;
    float front_1;
    if((corner_0 & 1U) != 0U)
    {
        front_1 = 1.0f;
    }
    else
    {
        front_1 = -1.0f;
    }
    float _S97;
    if((corner_0 & 2U) != 0U)
    {
        _S97 = 1.0f;
    }
    else
    {
        _S97 = -1.0f;
    }
    float3 p_5 = _S95.c_0 + br_0 * float3((front_1 * half_x_0))  + bu_0 * float3((_S97 * half_w_0)) ;
    float3 _S98 = p_5 - _S95.c_0;
    thread ImpTree_0 _S99 = _S95;
    float3 _S100 = imp_to_tree_0(&_S99, _S98);
    float3 p_6;
    if(!shadow_2)
    {
        float _S101 = length(_S95.c_0);
        float _S102 = min(half_w_0, max(_S101 - (float4(_S87->params_1) ).z * 2.0f, 0.0f));
        float3 p_7 = p_5 - vw_0 * float3(_S102) ;
        float lp_0 = length(p_7);
        float3 p_8 = p_7 * float3((max(lp_0 - float(((uint4((&_S87->imp_0)->bufs_1) ).w) >> 8U) / 16.0f * _S101 / max((float4(_S87->proj_0) ).w * 0.5f * (float4(_S87->proj_0) ).y, 1.0f), min(lp_0, (float4(_S87->params_1) ).z * 2.0f)) / max(lp_0, 9.99999997475242708e-07f))) ;
        front_1 = _S102;
        p_6 = p_8;
    }
    else
    {
        front_1 = 0.0f;
        p_6 = p_5;
    }
    thread ImpOut_0 o_1;
    matrix<float,int(4),int(4)>  _S103;
    if(shadow_2)
    {
        _S103 = matrix<float,int(4),int(4)> ((&(&_S87->light_0)->shadow_0)->data_3[kernelContext_16->ed_c_0->cascade_0].data_1[int(0)][int(0)], (&(&_S87->light_0)->shadow_0)->data_3[kernelContext_16->ed_c_0->cascade_0].data_1[int(1)][int(0)], (&(&_S87->light_0)->shadow_0)->data_3[kernelContext_16->ed_c_0->cascade_0].data_1[int(2)][int(0)], (&(&_S87->light_0)->shadow_0)->data_3[kernelContext_16->ed_c_0->cascade_0].data_1[int(3)][int(0)], (&(&_S87->light_0)->shadow_0)->data_3[kernelContext_16->ed_c_0->cascade_0].data_1[int(0)][int(1)], (&(&_S87->light_0)->shadow_0)->data_3[kernelContext_16->ed_c_0->cascade_0].data_1[int(1)][int(1)], (&(&_S87->light_0)->shadow_0)->data_3[kernelContext_16->ed_c_0->cascade_0].data_1[int(2)][int(1)], (&(&_S87->light_0)->shadow_0)->data_3[kernelContext_16->ed_c_0->cascade_0].data_1[int(3)][int(1)], (&(&_S87->light_0)->shadow_0)->data_3[kernelContext_16->ed_c_0->cascade_0].data_1[int(0)][int(2)], (&(&_S87->light_0)->shadow_0)->data_3[kernelContext_16->ed_c_0->cascade_0].data_1[int(1)][int(2)], (&(&_S87->light_0)->shadow_0)->data_3[kernelContext_16->ed_c_0->cascade_0].data_1[int(2)][int(2)], (&(&_S87->light_0)->shadow_0)->data_3[kernelContext_16->ed_c_0->cascade_0].data_1[int(3)][int(2)], (&(&_S87->light_0)->shadow_0)->data_3[kernelContext_16->ed_c_0->cascade_0].data_1[int(0)][int(3)], (&(&_S87->light_0)->shadow_0)->data_3[kernelContext_16->ed_c_0->cascade_0].data_1[int(1)][int(3)], (&(&_S87->light_0)->shadow_0)->data_3[kernelContext_16->ed_c_0->cascade_0].data_1[int(2)][int(3)], (&(&_S87->light_0)->shadow_0)->data_3[kernelContext_16->ed_c_0->cascade_0].data_1[int(3)][int(3)]);
    }
    else
    {
        _S103 = matrix<float,int(4),int(4)> (_S87->view_proj_0.data_1[int(0)][int(0)], _S87->view_proj_0.data_1[int(1)][int(0)], _S87->view_proj_0.data_1[int(2)][int(0)], _S87->view_proj_0.data_1[int(3)][int(0)], _S87->view_proj_0.data_1[int(0)][int(1)], _S87->view_proj_0.data_1[int(1)][int(1)], _S87->view_proj_0.data_1[int(2)][int(1)], _S87->view_proj_0.data_1[int(3)][int(1)], _S87->view_proj_0.data_1[int(0)][int(2)], _S87->view_proj_0.data_1[int(1)][int(2)], _S87->view_proj_0.data_1[int(2)][int(2)], _S87->view_proj_0.data_1[int(3)][int(2)], _S87->view_proj_0.data_1[int(0)][int(3)], _S87->view_proj_0.data_1[int(1)][int(3)], _S87->view_proj_0.data_1[int(2)][int(3)], _S87->view_proj_0.data_1[int(3)][int(3)]);
    }
    float4 _S104 = float4(p_6, 1.0f);
    (&o_1)->pos_6 = (((_S104) * (_S103)));
    thread ImpTree_0 _S105 = _S95;
    float2 _S106 = imp_frame_uv_0(&_S105, _S95.d0_0, _S100, half_t_0);
    (&o_1)->uv0_0 = _S106;
    thread ImpTree_0 _S107 = _S95;
    float2 _S108 = imp_frame_uv_0(&_S107, _S95.d1_0, _S100, half_t_0);
    (&o_1)->uv1_0 = _S108;
    thread ImpTree_0 _S109 = _S95;
    float2 _S110 = imp_frame_uv_0(&_S109, _S95.d2_0, _S100, half_t_0);
    (&o_1)->uv2_0 = _S110;
    if(shadow_2)
    {
        half_x_0 = 0.0f;
    }
    else
    {
        half_x_0 = - (((_S104) * (matrix<float,int(4),int(4)> (_S87->view_0.data_1[int(0)][int(0)], _S87->view_0.data_1[int(1)][int(0)], _S87->view_0.data_1[int(2)][int(0)], _S87->view_0.data_1[int(3)][int(0)], _S87->view_0.data_1[int(0)][int(1)], _S87->view_0.data_1[int(1)][int(1)], _S87->view_0.data_1[int(2)][int(1)], _S87->view_0.data_1[int(3)][int(1)], _S87->view_0.data_1[int(0)][int(2)], _S87->view_0.data_1[int(1)][int(2)], _S87->view_0.data_1[int(2)][int(2)], _S87->view_0.data_1[int(3)][int(2)], _S87->view_0.data_1[int(0)][int(3)], _S87->view_0.data_1[int(1)][int(3)], _S87->view_0.data_1[int(2)][int(3)], _S87->view_0.data_1[int(3)][int(3)])))).z;
    }
    (&o_1)->zv_0 = half_x_0;
    (&o_1)->entry_8 = entry_9;
    uint4 _S111 = uint4((&_S92)->tex_0) ;
    (&o_1)->fi_1 = uint4(_S95.fi_0, _S111.x);
    (&o_1)->w_4 = float4(_S95.w_3, 2.0f * half_t_0 * _S95.s_1);
    (&o_1)->front_0 = front_1;
    (&o_1)->geo_1 = float2(float(_S111.w), _S96.w);
    return o_1;
}

struct vertexOutput_0
{
    float4 output_4 [[position]];
};

[[vertex]] vertexOutput_0 vs_imp_shadow_proxy(uint vid_2 [[vertex_id]], uint iid_2 [[instance_id]], VisC_0 constant* ed_c_6 [[buffer(0)]], ulong device* ed_buffers_6 [[buffer(2)]], ulong device* ed_textures_6 [[buffer(1)]], ulong device* ed_samplers_6 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_17;
    (&kernelContext_17)->ed_c_0 = ed_c_6;
    (&kernelContext_17)->ed_buffers_0 = ed_buffers_6;
    (&kernelContext_17)->ed_textures_0 = ed_textures_6;
    (&kernelContext_17)->ed_samplers_0 = ed_samplers_6;
    FrameData_natural_0 device* _S112 = ed_buf_8(ed_c_6->frame_0, &kernelContext_17);
    uint device* _S113 = ed_buf_7((&kernelContext_17)->ed_c_0->visible_0, &kernelContext_17);
    uint entry_10 = *(_S113 + iid_2);
    GpuTree_0 device* _S114 = ed_buf_1((&kernelContext_17)->ed_c_0->instances_0, &kernelContext_17);
    GpuTree_0 g_3 = *(_S114 + (entry_10 & 16777215U));
    GpuImpKind_natural_0 device* _S115 = ed_buf_0((&kernelContext_17)->ed_c_0->verts_0, &kernelContext_17);
    GpuImpKind_natural_0 k_4 = *(_S115 + (((g_3.node_kind_0) >> 12U) & 4095U));
    packed_float4 device* _S116 = ed_buf_4((&kernelContext_17)->ed_c_0->nodes_0, &kernelContext_17);
    float3 node_4 = (float4(*(_S116 + ((g_3.node_kind_0) & 4095U))) ).xyz;
    float3 l_2 = (float4(_S112->sun_dir_0) ).xyz;
    float3 up_5 = oct_decode_0(g_3.up_0);
    float h_1 = (float4(k_4.geo_0) ).z;
    uint cw_0 = (as_type<uint>(((float4(k_4.bark_0) ).w)));
    float cb_0 = float(cw_0 & 1023U) / 1023.0f * h_1 * g_3.scale_0;
    float cr_0 = float((cw_0 >> 10U) & 2047U) / 2047.0f * 1.5f * h_1 * g_3.scale_0 * (0.5f + float((g_3.yaw_widen_0) >> 16U) / 65535.0f);
    float _S117 = dot(l_2, up_5);
    float sin_el_0 = abs(_S117);
    float3 u0_2 = up_5 - l_2 * float3(_S117) ;
    float cos_el_0 = length(u0_2);
    float3 bu_1;
    if(cos_el_0 > 0.0010000000474974513f)
    {
        bu_1 = u0_2 / float3(cos_el_0) ;
    }
    else
    {
        bu_1 = float3(1.0f, 0.0f, 0.0f);
    }
    float3 br_1 = normalize(cross(bu_1, l_2));
    float yt_0 = h_1 * g_3.scale_0 * cos_el_0 + 0.5f * cr_0 * sin_el_0;
    float yb_0 = cb_0 * cos_el_0 - cr_0 * sin_el_0;
    float ym_0 = 0.5f * (yt_0 + yb_0);
    float k_s_0 = 1.1200000047683716f * sqrt(float(entry_10 >> 24U) / 255.0f);
    float yw_0 = mix(yb_0, yt_0, 0.40000000596046448f);
    uint corner_1 = vid_2 & 3U;
    float2 q_2;
    if(corner_1 == 0U)
    {
        q_2 = float2(- cr_0, yw_0);
    }
    else
    {
        if(corner_1 == 1U)
        {
            q_2 = float2(0.0f, yb_0);
        }
        else
        {
            if(corner_1 == 2U)
            {
                q_2 = float2(0.0f, yt_0);
            }
            else
            {
                q_2 = float2(cr_0, yw_0);
            }
        }
    }
    vertexOutput_0 _S118 = { (((float4(node_4 + float3(g_3.px_0, g_3.py_0, g_3.pz_0) + bu_1 * float3((ym_0 + (q_2.y - ym_0) * k_s_0))  + br_1 * float3((q_2.x * k_s_0)) , 1.0f)) * (matrix<float,int(4),int(4)> ((&(&_S112->light_0)->shadow_0)->data_3[(&kernelContext_17)->ed_c_0->cascade_0].data_1[int(0)][int(0)], (&(&_S112->light_0)->shadow_0)->data_3[(&kernelContext_17)->ed_c_0->cascade_0].data_1[int(1)][int(0)], (&(&_S112->light_0)->shadow_0)->data_3[(&kernelContext_17)->ed_c_0->cascade_0].data_1[int(2)][int(0)], (&(&_S112->light_0)->shadow_0)->data_3[(&kernelContext_17)->ed_c_0->cascade_0].data_1[int(3)][int(0)], (&(&_S112->light_0)->shadow_0)->data_3[(&kernelContext_17)->ed_c_0->cascade_0].data_1[int(0)][int(1)], (&(&_S112->light_0)->shadow_0)->data_3[(&kernelContext_17)->ed_c_0->cascade_0].data_1[int(1)][int(1)], (&(&_S112->light_0)->shadow_0)->data_3[(&kernelContext_17)->ed_c_0->cascade_0].data_1[int(2)][int(1)], (&(&_S112->light_0)->shadow_0)->data_3[(&kernelContext_17)->ed_c_0->cascade_0].data_1[int(3)][int(1)], (&(&_S112->light_0)->shadow_0)->data_3[(&kernelContext_17)->ed_c_0->cascade_0].data_1[int(0)][int(2)], (&(&_S112->light_0)->shadow_0)->data_3[(&kernelContext_17)->ed_c_0->cascade_0].data_1[int(1)][int(2)], (&(&_S112->light_0)->shadow_0)->data_3[(&kernelContext_17)->ed_c_0->cascade_0].data_1[int(2)][int(2)], (&(&_S112->light_0)->shadow_0)->data_3[(&kernelContext_17)->ed_c_0->cascade_0].data_1[int(3)][int(2)], (&(&_S112->light_0)->shadow_0)->data_3[(&kernelContext_17)->ed_c_0->cascade_0].data_1[int(0)][int(3)], (&(&_S112->light_0)->shadow_0)->data_3[(&kernelContext_17)->ed_c_0->cascade_0].data_1[int(1)][int(3)], (&(&_S112->light_0)->shadow_0)->data_3[(&kernelContext_17)->ed_c_0->cascade_0].data_1[int(2)][int(3)], (&(&_S112->light_0)->shadow_0)->data_3[(&kernelContext_17)->ed_c_0->cascade_0].data_1[int(3)][int(3)])))) };
    return _S118;
}

float2 imp_atlas_uv_0(uint fi_2, uint n_5, float2 uv_6, float frame_px_0)
{
    float m_1 = 0.5f / frame_px_0;
    float _S119 = 1.0f - m_1;
    float2 l_3 = clamp(uv_6, float2(m_1, m_1), float2(_S119, _S119));
    uint _S120 = fi_2 % n_5;
    float _S121 = float(_S120);
    uint _S122 = fi_2 / n_5;
    return (float2(_S121, float(_S122)) + l_3) / float2(float(n_5)) ;
}

bool imp_inside_0(float2 uv_7)
{
    bool _S123;
    if(all(uv_7 >= (float2(0.0f) )))
    {
        _S123 = all(uv_7 <= (float2(1.0f) ));
    }
    else
    {
        _S123 = false;
    }
    return _S123;
}

float imp_coverage_0(const ImpOut_0 thread* i_13, KernelContext_0 thread* kernelContext_18)
{
    uint4 _S124 = i_13->fi_1;
    texture2d<float, access::sample> _S125 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_18->ed_textures_0))[(i_13->fi_1.w)]));
    sampler _S126 = ((reinterpret_cast<device sampler*>((kernelContext_18->ed_samplers_0))[(kernelContext_18->ed_c_0->smp_0)]));
    uint n_6 = uint(i_13->geo_1.x);
    float _S127 = i_13->geo_1.y;
    float2 t0_0 = imp_atlas_uv_0(_S124.x, n_6, i_13->uv0_0, _S127);
    float2 _S128 = i_13->uv1_0;
    float2 t1_0 = imp_atlas_uv_0(_S124.y, n_6, i_13->uv1_0, _S127);
    float2 _S129 = i_13->uv2_0;
    float2 t2_0 = imp_atlas_uv_0(_S124.z, n_6, i_13->uv2_0, _S127);
    float2 _S130 = ((_S125).sample((_S126), (t0_0))).xy;
    float _S131;
    if(imp_inside_0(i_13->uv0_0))
    {
        _S131 = 1.0f;
    }
    else
    {
        _S131 = 0.0f;
    }
    float2 s0_0 = _S130 * float2(_S131) ;
    float2 _S132 = ((_S125).sample((_S126), (t1_0))).xy;
    if(imp_inside_0(_S128))
    {
        _S131 = 1.0f;
    }
    else
    {
        _S131 = 0.0f;
    }
    float2 s1_0 = _S132 * float2(_S131) ;
    float2 _S133 = ((_S125).sample((_S126), (t2_0))).xy;
    if(imp_inside_0(_S129))
    {
        _S131 = 1.0f;
    }
    else
    {
        _S131 = 0.0f;
    }
    float2 s2_0 = _S133 * float2(_S131) ;
    float4 _S134 = i_13->w_4;
    float _S135 = i_13->w_4.x * s0_0.x;
    float _S136 = i_13->w_4.y * s1_0.x;
    float _S137 = i_13->w_4.z * s2_0.x;
    float cov_0 = _S135 + _S136 + _S137;
    uint2 p_9 = uint2(i_13->pos_6.xy) & (uint2(7U) );
    if((saturate((cov_0 - 0.5f) * 2.0f + 0.5f) * float((i_13->entry_8) >> 24U) / 255.0f) <= ((float((BAYER8_0[p_9.y * 8U + p_9.x] + ((((i_13->entry_8) & 16777215U) * 2654435761U) >> 26U)) & 63U) + 0.5f) / 64.0f))
    {
        discard_fragment();
    }
    float d_3 = _S135 * s0_0.y + _S136 * s1_0.y + _S137 * s2_0.y;
    if(cov_0 > 9.9999997473787516e-05f)
    {
        _S131 = (d_3 / cov_0 - 0.5f) * _S134.w;
    }
    else
    {
        _S131 = 0.0f;
    }
    return _S131;
}

struct ImpDepthOut_0
{
    uint4 vis_0 [[color(0)]];
    float depth_0 [[depth(less)]];
};

struct pixelInput_5
{
    float2 uv0_1 [[user(TEXCOORD)]];
    float2 uv1_1 [[user(TEXCOORD_1)]];
    float2 uv2_1 [[user(TEXCOORD_2)]];
    float zv_1 [[user(TEXCOORD_3)]];
    [[flat]] uint entry_11 [[user(ENTRY)]];
    [[flat]] uint4 fi_3 [[user(FRAMES)]];
    [[flat]] float4 w_5 [[user(WEIGHTS)]];
    [[flat]] float2 geo_2 [[user(GEO)]];
    [[flat]] float front_2 [[user(FRONT)]];
};

[[fragment]] ImpDepthOut_0 fs_imp(pixelInput_5 _S138 [[stage_in]], float4 pos_7 [[position]], VisC_0 constant* ed_c_7 [[buffer(0)]], ulong device* ed_buffers_7 [[buffer(2)]], ulong device* ed_textures_7 [[buffer(1)]], ulong device* ed_samplers_7 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_19;
    (&kernelContext_19)->ed_c_0 = ed_c_7;
    (&kernelContext_19)->ed_buffers_0 = ed_buffers_7;
    (&kernelContext_19)->ed_textures_0 = ed_textures_7;
    (&kernelContext_19)->ed_samplers_0 = ed_samplers_7;
    FrameData_natural_0 device* _S139 = ed_buf_8(ed_c_7->frame_0, &kernelContext_19);
    thread ImpOut_0 _S140;
    (&_S140)->pos_6 = pos_7;
    (&_S140)->uv0_0 = _S138.uv0_1;
    (&_S140)->uv1_0 = _S138.uv1_1;
    (&_S140)->uv2_0 = _S138.uv2_1;
    (&_S140)->zv_0 = _S138.zv_1;
    (&_S140)->entry_8 = _S138.entry_11;
    (&_S140)->fi_1 = _S138.fi_3;
    (&_S140)->w_4 = _S138.w_5;
    (&_S140)->geo_1 = _S138.geo_2;
    (&_S140)->front_0 = _S138.front_2;
    float _S141 = imp_coverage_0(&_S140, &kernelContext_19);
    float near_0 = (float4(_S139->params_1) ).z;
    thread ImpDepthOut_0 o_2;
    (&o_2)->vis_0 = uint4(1073741824U | ((_S138.entry_11) & 16777215U), 0U, 0U, 0U);
    (&o_2)->depth_0 = min(near_0 / max(_S138.zv_1 + max(_S138.front_2 - _S141, 0.0f), near_0), pos_7.z);
    return o_2;
}

struct pixelInput_6
{
    float2 uv0_2 [[user(TEXCOORD)]];
    float2 uv1_2 [[user(TEXCOORD_1)]];
    float2 uv2_2 [[user(TEXCOORD_2)]];
    float zv_2 [[user(TEXCOORD_3)]];
    [[flat]] uint entry_12 [[user(ENTRY)]];
    [[flat]] uint4 fi_4 [[user(FRAMES)]];
    [[flat]] float4 w_6 [[user(WEIGHTS)]];
    [[flat]] float2 geo_3 [[user(GEO)]];
    [[flat]] float front_3 [[user(FRONT)]];
};

[[fragment]] void fs_imp_shadow(pixelInput_6 _S142 [[stage_in]], float4 pos_8 [[position]], VisC_0 constant* ed_c_8 [[buffer(0)]], ulong device* ed_buffers_8 [[buffer(2)]], ulong device* ed_textures_8 [[buffer(1)]], ulong device* ed_samplers_8 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_20;
    (&kernelContext_20)->ed_c_0 = ed_c_8;
    (&kernelContext_20)->ed_buffers_0 = ed_buffers_8;
    (&kernelContext_20)->ed_textures_0 = ed_textures_8;
    (&kernelContext_20)->ed_samplers_0 = ed_samplers_8;
    texture2d<float, access::sample> _S143 = ((reinterpret_cast<device texture2d<float, access::sample>*>((ed_textures_8))[(_S142.fi_4.w)]));
    sampler _S144 = ((reinterpret_cast<device sampler*>((ed_samplers_8))[(ed_c_8->smp_0)]));
    uint n_7 = uint(_S142.geo_3.x);
    uint _S145 = _S142.fi_4.x;
    float _S146 = _S142.w_6.y;
    float _S147 = _S142.w_6.x;
    bool _S148;
    if(_S146 > _S147)
    {
        _S148 = _S146 >= (_S142.w_6.z);
    }
    else
    {
        _S148 = false;
    }
    float2 uv_8;
    uint fr_1;
    if(_S148)
    {
        uint _S149 = _S142.fi_4.y;
        uv_8 = _S142.uv1_2;
        fr_1 = _S149;
    }
    else
    {
        float _S150 = _S142.w_6.z;
        if(_S150 > _S147)
        {
            _S148 = _S150 > _S146;
        }
        else
        {
            _S148 = false;
        }
        if(_S148)
        {
            uint _S151 = _S142.fi_4.z;
            uv_8 = _S142.uv2_2;
            fr_1 = _S151;
        }
        else
        {
            uv_8 = _S142.uv0_2;
            fr_1 = _S145;
        }
    }
    float cov_1;
    if(imp_inside_0(uv_8))
    {
        float2 _S152 = imp_atlas_uv_0(fr_1, n_7, uv_8, _S142.geo_3.y);
        cov_1 = ((_S143).sample((_S144), (_S152))).x;
    }
    else
    {
        cov_1 = 0.0f;
    }
    uint2 p_10 = uint2(pos_8.xy) & (uint2(7U) );
    if((saturate((cov_1 - 0.5f) * 2.0f + 0.5f) * float((_S142.entry_12) >> 24U) / 255.0f) <= ((float((BAYER8_0[p_10.y * 8U + p_10.x] + ((((_S142.entry_12) & 16777215U) * 2654435761U) >> 26U)) & 63U) + 0.5f) / 64.0f))
    {
        discard_fragment();
    }
    return;
}

struct vs_vis_Result_0
{
    float4 pos_9 [[position]];
    uint entry_13 [[user(ENTRY)]];
};

struct VisOut_0
{
    float4 pos_10;
    [[flat]] uint entry_14;
};

[[vertex]] vs_vis_Result_0 vs_vis(uint vid_3 [[vertex_id]], uint iid_3 [[instance_id]], VisC_0 constant* ed_c_9 [[buffer(0)]], ulong device* ed_buffers_9 [[buffer(2)]], ulong device* ed_textures_9 [[buffer(1)]], ulong device* ed_samplers_9 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_21;
    (&kernelContext_21)->ed_c_0 = ed_c_9;
    (&kernelContext_21)->ed_buffers_0 = ed_buffers_9;
    (&kernelContext_21)->ed_textures_0 = ed_textures_9;
    (&kernelContext_21)->ed_samplers_0 = ed_samplers_9;
    FrameData_natural_0 device* _S153 = ed_buf_8(ed_c_9->frame_0, &kernelContext_21);
    uint device* _S154 = ed_buf_7((&kernelContext_21)->ed_c_0->visible_0, &kernelContext_21);
    uint entry_15 = *(_S154 + iid_3);
    GpuInstance_natural_0 device* _S155 = ed_buf_6((&kernelContext_21)->ed_c_0->instances_0, &kernelContext_21);
    GpuInstance_natural_0 device* _S156 = _S155 + (entry_15 & 1048575U);
    GpuInstance_natural_0 inst_2 = *_S156;
    thread GpuInstance_natural_0 _S157 = *_S156;
    Vtx_0 device* _S158 = ed_buf_5((&kernelContext_21)->ed_c_0->verts_0, &kernelContext_21);
    Vtx_0 v_5 = *(_S158 + vid_3);
    packed_float4 device* _S159 = ed_buf_4((&kernelContext_21)->ed_c_0->nodes_0, &kernelContext_21);
    float3 node_5 = (float4(*(_S159 + (((&_S157)->node_flags_0) & 65535U))) ).xyz;
    float3 _S160 = float3(v_5.px_1, v_5.py_1, v_5.pz_1);
    _S157 = inst_2;
    float3 _S161 = xform_point_0(&_S157, _S160);
    thread VisOut_0 o_3;
    (&o_3)->pos_10 = (((float4(node_5 + _S161, 1.0f)) * (matrix<float,int(4),int(4)> (_S153->view_proj_0.data_1[int(0)][int(0)], _S153->view_proj_0.data_1[int(1)][int(0)], _S153->view_proj_0.data_1[int(2)][int(0)], _S153->view_proj_0.data_1[int(3)][int(0)], _S153->view_proj_0.data_1[int(0)][int(1)], _S153->view_proj_0.data_1[int(1)][int(1)], _S153->view_proj_0.data_1[int(2)][int(1)], _S153->view_proj_0.data_1[int(3)][int(1)], _S153->view_proj_0.data_1[int(0)][int(2)], _S153->view_proj_0.data_1[int(1)][int(2)], _S153->view_proj_0.data_1[int(2)][int(2)], _S153->view_proj_0.data_1[int(3)][int(2)], _S153->view_proj_0.data_1[int(0)][int(3)], _S153->view_proj_0.data_1[int(1)][int(3)], _S153->view_proj_0.data_1[int(2)][int(3)], _S153->view_proj_0.data_1[int(3)][int(3)]))));
    (&o_3)->entry_14 = entry_15;
    thread vs_vis_Result_0 _S162;
    (&_S162)->pos_9 = o_3.pos_10;
    (&_S162)->entry_13 = o_3.entry_14;
    return _S162;
}

struct vs_shadow_Result_0
{
    float4 pos_11 [[position]];
};

struct ShadowOut_0
{
    float4 pos_12;
};

[[vertex]] vs_shadow_Result_0 vs_shadow(uint vid_4 [[vertex_id]], uint iid_4 [[instance_id]], VisC_0 constant* ed_c_10 [[buffer(0)]], ulong device* ed_buffers_10 [[buffer(2)]], ulong device* ed_textures_10 [[buffer(1)]], ulong device* ed_samplers_10 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_22;
    (&kernelContext_22)->ed_c_0 = ed_c_10;
    (&kernelContext_22)->ed_buffers_0 = ed_buffers_10;
    (&kernelContext_22)->ed_textures_0 = ed_textures_10;
    (&kernelContext_22)->ed_samplers_0 = ed_samplers_10;
    FrameData_natural_0 device* _S163 = ed_buf_8(ed_c_10->frame_0, &kernelContext_22);
    uint device* _S164 = ed_buf_7((&kernelContext_22)->ed_c_0->visible_0, &kernelContext_22);
    uint entry_16 = *(_S164 + iid_4);
    GpuInstance_natural_0 device* _S165 = ed_buf_6((&kernelContext_22)->ed_c_0->instances_0, &kernelContext_22);
    GpuInstance_natural_0 device* _S166 = _S165 + (entry_16 & 1048575U);
    GpuInstance_natural_0 inst_3 = *_S166;
    thread GpuInstance_natural_0 _S167 = *_S166;
    Vtx_0 device* _S168 = ed_buf_5((&kernelContext_22)->ed_c_0->verts_0, &kernelContext_22);
    Vtx_0 v_6 = *(_S168 + vid_4);
    packed_float4 device* _S169 = ed_buf_4((&kernelContext_22)->ed_c_0->nodes_0, &kernelContext_22);
    float3 node_6 = (float4(*(_S169 + (((&_S167)->node_flags_0) & 65535U))) ).xyz;
    float3 _S170 = float3(v_6.px_1, v_6.py_1, v_6.pz_1);
    _S167 = inst_3;
    float3 _S171 = xform_point_0(&_S167, _S170);
    thread ShadowOut_0 o_4;
    (&o_4)->pos_12 = (((float4(node_6 + _S171, 1.0f)) * (matrix<float,int(4),int(4)> ((&(&_S163->light_0)->shadow_0)->data_3[(&kernelContext_22)->ed_c_0->cascade_0].data_1[int(0)][int(0)], (&(&_S163->light_0)->shadow_0)->data_3[(&kernelContext_22)->ed_c_0->cascade_0].data_1[int(1)][int(0)], (&(&_S163->light_0)->shadow_0)->data_3[(&kernelContext_22)->ed_c_0->cascade_0].data_1[int(2)][int(0)], (&(&_S163->light_0)->shadow_0)->data_3[(&kernelContext_22)->ed_c_0->cascade_0].data_1[int(3)][int(0)], (&(&_S163->light_0)->shadow_0)->data_3[(&kernelContext_22)->ed_c_0->cascade_0].data_1[int(0)][int(1)], (&(&_S163->light_0)->shadow_0)->data_3[(&kernelContext_22)->ed_c_0->cascade_0].data_1[int(1)][int(1)], (&(&_S163->light_0)->shadow_0)->data_3[(&kernelContext_22)->ed_c_0->cascade_0].data_1[int(2)][int(1)], (&(&_S163->light_0)->shadow_0)->data_3[(&kernelContext_22)->ed_c_0->cascade_0].data_1[int(3)][int(1)], (&(&_S163->light_0)->shadow_0)->data_3[(&kernelContext_22)->ed_c_0->cascade_0].data_1[int(0)][int(2)], (&(&_S163->light_0)->shadow_0)->data_3[(&kernelContext_22)->ed_c_0->cascade_0].data_1[int(1)][int(2)], (&(&_S163->light_0)->shadow_0)->data_3[(&kernelContext_22)->ed_c_0->cascade_0].data_1[int(2)][int(2)], (&(&_S163->light_0)->shadow_0)->data_3[(&kernelContext_22)->ed_c_0->cascade_0].data_1[int(3)][int(2)], (&(&_S163->light_0)->shadow_0)->data_3[(&kernelContext_22)->ed_c_0->cascade_0].data_1[int(0)][int(3)], (&(&_S163->light_0)->shadow_0)->data_3[(&kernelContext_22)->ed_c_0->cascade_0].data_1[int(1)][int(3)], (&(&_S163->light_0)->shadow_0)->data_3[(&kernelContext_22)->ed_c_0->cascade_0].data_1[int(2)][int(3)], (&(&_S163->light_0)->shadow_0)->data_3[(&kernelContext_22)->ed_c_0->cascade_0].data_1[int(3)][int(3)]))));
    thread vs_shadow_Result_0 _S172;
    (&_S172)->pos_11 = o_4.pos_12;
    return _S172;
}

struct vs_foliage_Result_0
{
    float4 pos_13 [[position]];
    float2 uv_9 [[user(TEXCOORD)]];
    uint entry_17 [[user(ENTRY)]];
    uint tex_6 [[user(TEXID)]];
};

[[vertex]] vs_foliage_Result_0 vs_foliage(uint vid_5 [[vertex_id]], uint iid_5 [[instance_id]], VisC_0 constant* ed_c_11 [[buffer(0)]], ulong device* ed_buffers_11 [[buffer(2)]], ulong device* ed_textures_11 [[buffer(1)]], ulong device* ed_samplers_11 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_23;
    (&kernelContext_23)->ed_c_0 = ed_c_11;
    (&kernelContext_23)->ed_buffers_0 = ed_buffers_11;
    (&kernelContext_23)->ed_textures_0 = ed_textures_11;
    (&kernelContext_23)->ed_samplers_0 = ed_samplers_11;
    FoliageOut_0 _S173 = foliage_vertex_0(vid_5, iid_5, false, &kernelContext_23);
    thread vs_foliage_Result_0 _S174;
    (&_S174)->pos_13 = _S173.pos_2;
    (&_S174)->uv_9 = _S173.uv_1;
    (&_S174)->entry_17 = _S173.entry_3;
    (&_S174)->tex_6 = _S173.tex_2;
    return _S174;
}

struct vs_shadow_foliage_Result_0
{
    float4 pos_14 [[position]];
    float2 uv_10 [[user(TEXCOORD)]];
    uint entry_18 [[user(ENTRY)]];
    uint tex_7 [[user(TEXID)]];
};

[[vertex]] vs_shadow_foliage_Result_0 vs_shadow_foliage(uint vid_6 [[vertex_id]], uint iid_6 [[instance_id]], VisC_0 constant* ed_c_12 [[buffer(0)]], ulong device* ed_buffers_12 [[buffer(2)]], ulong device* ed_textures_12 [[buffer(1)]], ulong device* ed_samplers_12 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_24;
    (&kernelContext_24)->ed_c_0 = ed_c_12;
    (&kernelContext_24)->ed_buffers_0 = ed_buffers_12;
    (&kernelContext_24)->ed_textures_0 = ed_textures_12;
    (&kernelContext_24)->ed_samplers_0 = ed_samplers_12;
    FoliageOut_0 _S175 = foliage_vertex_0(vid_6, iid_6, true, &kernelContext_24);
    thread vs_shadow_foliage_Result_0 _S176;
    (&_S176)->pos_14 = _S175.pos_2;
    (&_S176)->uv_10 = _S175.uv_1;
    (&_S176)->entry_18 = _S175.entry_3;
    (&_S176)->tex_7 = _S175.tex_2;
    return _S176;
}

struct vs_imp_Result_0
{
    float4 pos_15 [[position]];
    float2 uv0_3 [[user(TEXCOORD)]];
    float2 uv1_3 [[user(TEXCOORD_1)]];
    float2 uv2_3 [[user(TEXCOORD_2)]];
    float zv_3 [[user(TEXCOORD_3)]];
    uint entry_19 [[user(ENTRY)]];
    uint4 fi_5 [[user(FRAMES)]];
    float4 w_7 [[user(WEIGHTS)]];
    float2 geo_4 [[user(GEO)]];
    float front_4 [[user(FRONT)]];
};

[[vertex]] vs_imp_Result_0 vs_imp(uint vid_7 [[vertex_id]], uint iid_7 [[instance_id]], VisC_0 constant* ed_c_13 [[buffer(0)]], ulong device* ed_buffers_13 [[buffer(2)]], ulong device* ed_textures_13 [[buffer(1)]], ulong device* ed_samplers_13 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_25;
    (&kernelContext_25)->ed_c_0 = ed_c_13;
    (&kernelContext_25)->ed_buffers_0 = ed_buffers_13;
    (&kernelContext_25)->ed_textures_0 = ed_textures_13;
    (&kernelContext_25)->ed_samplers_0 = ed_samplers_13;
    ImpOut_0 _S177 = imp_vertex_0(vid_7, iid_7, false, &kernelContext_25);
    thread vs_imp_Result_0 _S178;
    (&_S178)->pos_15 = _S177.pos_6;
    (&_S178)->uv0_3 = _S177.uv0_0;
    (&_S178)->uv1_3 = _S177.uv1_0;
    (&_S178)->uv2_3 = _S177.uv2_0;
    (&_S178)->zv_3 = _S177.zv_0;
    (&_S178)->entry_19 = _S177.entry_8;
    (&_S178)->fi_5 = _S177.fi_1;
    (&_S178)->w_7 = _S177.w_4;
    (&_S178)->geo_4 = _S177.geo_1;
    (&_S178)->front_4 = _S177.front_0;
    return _S178;
}

struct vs_imp_shadow_Result_0
{
    float4 pos_16 [[position]];
    float2 uv0_4 [[user(TEXCOORD)]];
    float2 uv1_4 [[user(TEXCOORD_1)]];
    float2 uv2_4 [[user(TEXCOORD_2)]];
    float zv_4 [[user(TEXCOORD_3)]];
    uint entry_20 [[user(ENTRY)]];
    uint4 fi_6 [[user(FRAMES)]];
    float4 w_8 [[user(WEIGHTS)]];
    float2 geo_5 [[user(GEO)]];
    float front_5 [[user(FRONT)]];
};

[[vertex]] vs_imp_shadow_Result_0 vs_imp_shadow(uint vid_8 [[vertex_id]], uint iid_8 [[instance_id]], VisC_0 constant* ed_c_14 [[buffer(0)]], ulong device* ed_buffers_14 [[buffer(2)]], ulong device* ed_textures_14 [[buffer(1)]], ulong device* ed_samplers_14 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_26;
    (&kernelContext_26)->ed_c_0 = ed_c_14;
    (&kernelContext_26)->ed_buffers_0 = ed_buffers_14;
    (&kernelContext_26)->ed_textures_0 = ed_textures_14;
    (&kernelContext_26)->ed_samplers_0 = ed_samplers_14;
    ImpOut_0 _S179 = imp_vertex_0(vid_8, iid_8, true, &kernelContext_26);
    thread vs_imp_shadow_Result_0 _S180;
    (&_S180)->pos_16 = _S179.pos_6;
    (&_S180)->uv0_4 = _S179.uv0_0;
    (&_S180)->uv1_4 = _S179.uv1_0;
    (&_S180)->uv2_4 = _S179.uv2_0;
    (&_S180)->zv_4 = _S179.zv_0;
    (&_S180)->entry_20 = _S179.entry_8;
    (&_S180)->fi_6 = _S179.fi_1;
    (&_S180)->w_8 = _S179.w_4;
    (&_S180)->geo_5 = _S179.geo_1;
    (&_S180)->front_5 = _S179.front_0;
    return _S180;
}

