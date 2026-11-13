#include <metal_stdlib>
#include <metal_math>
#include <metal_texture>
using namespace metal;
struct GpuMaterial_natural_0
{
    packed_float4 base_0;
    packed_float4 params_0;
    packed_float4 shade_0;
    packed_uint4 tex_0;
    packed_float4 uv_0;
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

GpuMaterial_natural_0 device* ed_buf_0(uint i_0, KernelContext_0 thread* kernelContext_0)
{
    return (GpuMaterial_natural_0 device*)kernelContext_0->ed_buffers_0[i_0];
}

packed_uint2 device* ed_buf_1(uint i_1, KernelContext_0 thread* kernelContext_1)
{
    return (packed_uint2 device*)kernelContext_1->ed_buffers_0[i_1];
}

constant array<uint, int(64)> BAYER8_0 = { { 0U, 32U, 8U, 40U, 2U, 34U, 10U, 42U, 48U, 16U, 56U, 24U, 50U, 18U, 58U, 26U, 12U, 44U, 4U, 36U, 14U, 46U, 6U, 38U, 60U, 28U, 52U, 20U, 62U, 30U, 54U, 22U, 3U, 35U, 11U, 43U, 1U, 33U, 9U, 41U, 51U, 19U, 59U, 27U, 49U, 17U, 57U, 25U, 15U, 47U, 7U, 39U, 13U, 45U, 5U, 37U, 63U, 31U, 55U, 23U, 61U, 29U, 53U, 21U } };
packed_float4 device* ed_buf_2(uint i_2, KernelContext_0 thread* kernelContext_2)
{
    return (packed_float4 device*)kernelContext_2->ed_buffers_0[i_2];
}

struct Vtx_0
{
    float px_0;
    float py_0;
    float pz_0;
    uint n_0;
};

Vtx_0 device* ed_buf_3(uint i_3, KernelContext_0 thread* kernelContext_3)
{
    return (Vtx_0 device*)kernelContext_3->ed_buffers_0[i_3];
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

GpuInstance_natural_0 device* ed_buf_4(uint i_4, KernelContext_0 thread* kernelContext_4)
{
    return (GpuInstance_natural_0 device*)kernelContext_4->ed_buffers_0[i_4];
}

uint device* ed_buf_5(uint i_5, KernelContext_0 thread* kernelContext_5)
{
    return (uint device*)kernelContext_5->ed_buffers_0[i_5];
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
    packed_float4 params_1;
    packed_float4 proj_0;
    packed_uint4 cull_0;
    array<HzbLevel_0, int(16)> hzb_0;
    LightData_natural_0 light_0;
    AtmosData_natural_0 atmos_0;
    WindData_natural_0 wind_1;
};

FrameData_natural_0 device* ed_buf_6(uint i_6, KernelContext_0 thread* kernelContext_6)
{
    return (FrameData_natural_0 device*)kernelContext_6->ed_buffers_0[i_6];
}

float3 xform_point_0(const GpuInstance_natural_0 thread* i_7, float3 p_0)
{
    float4 _S1 = float4((&i_7->m_0)->data_0[int(0)]) ;
    float4 _S2 = float4((&i_7->m_0)->data_0[int(1)]) ;
    float4 _S3 = float4((&i_7->m_0)->data_0[int(2)]) ;
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
    thread KernelContext_0 kernelContext_7;
    (&kernelContext_7)->ed_c_0 = ed_c_1;
    (&kernelContext_7)->ed_buffers_0 = ed_buffers_1;
    (&kernelContext_7)->ed_textures_0 = ed_textures_1;
    (&kernelContext_7)->ed_samplers_0 = ed_samplers_1;
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
    thread KernelContext_0 kernelContext_8;
    (&kernelContext_8)->ed_c_0 = ed_c_2;
    (&kernelContext_8)->ed_buffers_0 = ed_buffers_2;
    (&kernelContext_8)->ed_textures_0 = ed_textures_2;
    (&kernelContext_8)->ed_samplers_0 = ed_samplers_2;
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

float max_scale_0(const GpuInstance_natural_0 thread* i_8)
{
    float4 _S8 = float4((&i_8->m_0)->data_0[int(0)]) ;
    float4 _S9 = float4((&i_8->m_0)->data_0[int(1)]) ;
    float4 _S10 = float4((&i_8->m_0)->data_0[int(2)]) ;
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

float3 xform_normal_0(const GpuInstance_natural_0 thread* i_9, float3 n_2)
{
    float4 _S15 = float4((&i_9->m_0)->data_0[int(0)]) ;
    float4 _S16 = float4((&i_9->m_0)->data_0[int(1)]) ;
    float4 _S17 = float4((&i_9->m_0)->data_0[int(2)]) ;
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
    [[flat]] uint tex_1;
};

FoliageOut_0 foliage_vertex_0(uint vid_0, uint iid_0, bool shadow_1, KernelContext_0 thread* kernelContext_9)
{
    FrameData_natural_0 device* _S30 = ed_buf_6(kernelContext_9->ed_c_0->frame_0, kernelContext_9);
    uint device* _S31 = ed_buf_5(kernelContext_9->ed_c_0->visible_0, kernelContext_9);
    uint entry_4 = *(_S31 + iid_0);
    GpuInstance_natural_0 device* _S32 = ed_buf_4(kernelContext_9->ed_c_0->instances_0, kernelContext_9);
    GpuInstance_natural_0 device* _S33 = _S32 + (entry_4 & 1048575U);
    GpuInstance_natural_0 inst_1 = *_S33;
    thread GpuInstance_natural_0 _S34 = *_S33;
    Vtx_0 device* _S35 = ed_buf_3(kernelContext_9->ed_c_0->verts_0, kernelContext_9);
    Vtx_0 v_1 = *(_S35 + vid_0);
    packed_uint2 device* _S36 = ed_buf_1(kernelContext_9->ed_c_0->uvs_0, kernelContext_9);
    uint2 _S37 = uint2(*(_S36 + vid_0)) ;
    packed_float4 device* _S38 = ed_buf_2(kernelContext_9->ed_c_0->nodes_0, kernelContext_9);
    float3 node_1 = (float4(*(_S38 + (((&_S34)->node_flags_0) & 65535U))) ).xyz;
    float3 _S39 = float3(v_1.px_0, v_1.py_0, v_1.pz_0);
    uint _S40 = _S37.y;
    _S34 = inst_1;
    float3 _S41 = tree_vertex_0(_S30, &_S34, node_1, _S39, v_1.n_0, _S40);
    float3 p_3 = node_1 + _S41;
    thread FoliageOut_0 o_0;
    matrix<float,int(4),int(4)>  _S42;
    if(shadow_1)
    {
        _S42 = matrix<float,int(4),int(4)> ((&(&_S30->light_0)->shadow_0)->data_3[kernelContext_9->ed_c_0->cascade_0].data_1[int(0)][int(0)], (&(&_S30->light_0)->shadow_0)->data_3[kernelContext_9->ed_c_0->cascade_0].data_1[int(1)][int(0)], (&(&_S30->light_0)->shadow_0)->data_3[kernelContext_9->ed_c_0->cascade_0].data_1[int(2)][int(0)], (&(&_S30->light_0)->shadow_0)->data_3[kernelContext_9->ed_c_0->cascade_0].data_1[int(3)][int(0)], (&(&_S30->light_0)->shadow_0)->data_3[kernelContext_9->ed_c_0->cascade_0].data_1[int(0)][int(1)], (&(&_S30->light_0)->shadow_0)->data_3[kernelContext_9->ed_c_0->cascade_0].data_1[int(1)][int(1)], (&(&_S30->light_0)->shadow_0)->data_3[kernelContext_9->ed_c_0->cascade_0].data_1[int(2)][int(1)], (&(&_S30->light_0)->shadow_0)->data_3[kernelContext_9->ed_c_0->cascade_0].data_1[int(3)][int(1)], (&(&_S30->light_0)->shadow_0)->data_3[kernelContext_9->ed_c_0->cascade_0].data_1[int(0)][int(2)], (&(&_S30->light_0)->shadow_0)->data_3[kernelContext_9->ed_c_0->cascade_0].data_1[int(1)][int(2)], (&(&_S30->light_0)->shadow_0)->data_3[kernelContext_9->ed_c_0->cascade_0].data_1[int(2)][int(2)], (&(&_S30->light_0)->shadow_0)->data_3[kernelContext_9->ed_c_0->cascade_0].data_1[int(3)][int(2)], (&(&_S30->light_0)->shadow_0)->data_3[kernelContext_9->ed_c_0->cascade_0].data_1[int(0)][int(3)], (&(&_S30->light_0)->shadow_0)->data_3[kernelContext_9->ed_c_0->cascade_0].data_1[int(1)][int(3)], (&(&_S30->light_0)->shadow_0)->data_3[kernelContext_9->ed_c_0->cascade_0].data_1[int(2)][int(3)], (&(&_S30->light_0)->shadow_0)->data_3[kernelContext_9->ed_c_0->cascade_0].data_1[int(3)][int(3)]);
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
        GpuMaterial_natural_0 device* _S44 = ed_buf_0(kernelContext_9->ed_c_0->materials_0, kernelContext_9);
        _S43 = (uint4((_S44 + (&_S34)->material_0)->tex_0) ).x;
    }
    else
    {
        _S43 = 4294967295U;
    }
    (&o_0)->tex_1 = _S43;
    return o_0;
}

void foliage_clip_0(const FoliageOut_0 thread* i_10, KernelContext_0 thread* kernelContext_10)
{
    uint _S45 = i_10->tex_1;
    bool _S46;
    if((i_10->tex_1) != 4294967295U)
    {
        texture2d<float, access::sample> _S47 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_10->ed_textures_0))[(_S45)]));
        sampler _S48 = ((reinterpret_cast<device sampler*>((kernelContext_10->ed_samplers_0))[(kernelContext_10->ed_c_0->smp_0)]));
        _S46 = (((_S47).sample((_S48), (i_10->uv_1))).w) < 0.5f;
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
    [[flat]] uint tex_2 [[user(TEXID)]];
};

[[fragment]] pixelOutput_2 fs_foliage(pixelInput_2 _S49 [[stage_in]], uint prim_3 [[primitive_id]], float4 pos_3 [[position]], VisC_0 constant* ed_c_3 [[buffer(0)]], ulong device* ed_buffers_3 [[buffer(2)]], ulong device* ed_textures_3 [[buffer(1)]], ulong device* ed_samplers_3 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_11;
    (&kernelContext_11)->ed_c_0 = ed_c_3;
    (&kernelContext_11)->ed_buffers_0 = ed_buffers_3;
    (&kernelContext_11)->ed_textures_0 = ed_textures_3;
    (&kernelContext_11)->ed_samplers_0 = ed_samplers_3;
    thread FoliageOut_0 _S50;
    (&_S50)->pos_2 = pos_3;
    (&_S50)->uv_1 = _S49.uv_2;
    (&_S50)->entry_3 = _S49.entry_5;
    (&_S50)->tex_1 = _S49.tex_2;
    foliage_clip_0(&_S50, &kernelContext_11);
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
    [[flat]] uint tex_3 [[user(TEXID)]];
};

[[fragment]] pixelOutput_3 fs_foliage_dither(pixelInput_3 _S52 [[stage_in]], uint prim_4 [[primitive_id]], float4 pos_4 [[position]], VisC_0 constant* ed_c_4 [[buffer(0)]], ulong device* ed_buffers_4 [[buffer(2)]], ulong device* ed_textures_4 [[buffer(1)]], ulong device* ed_samplers_4 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_12;
    (&kernelContext_12)->ed_c_0 = ed_c_4;
    (&kernelContext_12)->ed_buffers_0 = ed_buffers_4;
    (&kernelContext_12)->ed_textures_0 = ed_textures_4;
    (&kernelContext_12)->ed_samplers_0 = ed_samplers_4;
    thread FoliageOut_0 _S53;
    (&_S53)->pos_2 = pos_4;
    (&_S53)->uv_1 = _S52.uv_3;
    (&_S53)->entry_3 = _S52.entry_6;
    (&_S53)->tex_1 = _S52.tex_3;
    foliage_clip_0(&_S53, &kernelContext_12);
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
    [[flat]] uint tex_4 [[user(TEXID)]];
};

[[fragment]] void fs_shadow_foliage(pixelInput_4 _S55 [[stage_in]], float4 pos_5 [[position]], VisC_0 constant* ed_c_5 [[buffer(0)]], ulong device* ed_buffers_5 [[buffer(2)]], ulong device* ed_textures_5 [[buffer(1)]], ulong device* ed_samplers_5 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_13;
    (&kernelContext_13)->ed_c_0 = ed_c_5;
    (&kernelContext_13)->ed_buffers_0 = ed_buffers_5;
    (&kernelContext_13)->ed_textures_0 = ed_textures_5;
    (&kernelContext_13)->ed_samplers_0 = ed_samplers_5;
    thread FoliageOut_0 _S56;
    (&_S56)->pos_2 = pos_5;
    (&_S56)->uv_1 = _S55.uv_4;
    (&_S56)->entry_3 = _S55.entry_7;
    (&_S56)->tex_1 = _S55.tex_4;
    foliage_clip_0(&_S56, &kernelContext_13);
    return;
}

struct vs_vis_Result_0
{
    float4 pos_6 [[position]];
    uint entry_8 [[user(ENTRY)]];
};

struct VisOut_0
{
    float4 pos_7;
    [[flat]] uint entry_9;
};

[[vertex]] vs_vis_Result_0 vs_vis(uint vid_1 [[vertex_id]], uint iid_1 [[instance_id]], VisC_0 constant* ed_c_6 [[buffer(0)]], ulong device* ed_buffers_6 [[buffer(2)]], ulong device* ed_textures_6 [[buffer(1)]], ulong device* ed_samplers_6 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_14;
    (&kernelContext_14)->ed_c_0 = ed_c_6;
    (&kernelContext_14)->ed_buffers_0 = ed_buffers_6;
    (&kernelContext_14)->ed_textures_0 = ed_textures_6;
    (&kernelContext_14)->ed_samplers_0 = ed_samplers_6;
    FrameData_natural_0 device* _S57 = ed_buf_6(ed_c_6->frame_0, &kernelContext_14);
    uint device* _S58 = ed_buf_5((&kernelContext_14)->ed_c_0->visible_0, &kernelContext_14);
    uint entry_10 = *(_S58 + iid_1);
    GpuInstance_natural_0 device* _S59 = ed_buf_4((&kernelContext_14)->ed_c_0->instances_0, &kernelContext_14);
    GpuInstance_natural_0 device* _S60 = _S59 + (entry_10 & 1048575U);
    GpuInstance_natural_0 inst_2 = *_S60;
    thread GpuInstance_natural_0 _S61 = *_S60;
    Vtx_0 device* _S62 = ed_buf_3((&kernelContext_14)->ed_c_0->verts_0, &kernelContext_14);
    Vtx_0 v_2 = *(_S62 + vid_1);
    packed_float4 device* _S63 = ed_buf_2((&kernelContext_14)->ed_c_0->nodes_0, &kernelContext_14);
    float3 node_2 = (float4(*(_S63 + (((&_S61)->node_flags_0) & 65535U))) ).xyz;
    float3 _S64 = float3(v_2.px_0, v_2.py_0, v_2.pz_0);
    _S61 = inst_2;
    float3 _S65 = xform_point_0(&_S61, _S64);
    thread VisOut_0 o_1;
    (&o_1)->pos_7 = (((float4(node_2 + _S65, 1.0f)) * (matrix<float,int(4),int(4)> (_S57->view_proj_0.data_1[int(0)][int(0)], _S57->view_proj_0.data_1[int(1)][int(0)], _S57->view_proj_0.data_1[int(2)][int(0)], _S57->view_proj_0.data_1[int(3)][int(0)], _S57->view_proj_0.data_1[int(0)][int(1)], _S57->view_proj_0.data_1[int(1)][int(1)], _S57->view_proj_0.data_1[int(2)][int(1)], _S57->view_proj_0.data_1[int(3)][int(1)], _S57->view_proj_0.data_1[int(0)][int(2)], _S57->view_proj_0.data_1[int(1)][int(2)], _S57->view_proj_0.data_1[int(2)][int(2)], _S57->view_proj_0.data_1[int(3)][int(2)], _S57->view_proj_0.data_1[int(0)][int(3)], _S57->view_proj_0.data_1[int(1)][int(3)], _S57->view_proj_0.data_1[int(2)][int(3)], _S57->view_proj_0.data_1[int(3)][int(3)]))));
    (&o_1)->entry_9 = entry_10;
    thread vs_vis_Result_0 _S66;
    (&_S66)->pos_6 = o_1.pos_7;
    (&_S66)->entry_8 = o_1.entry_9;
    return _S66;
}

struct vs_shadow_Result_0
{
    float4 pos_8 [[position]];
};

struct ShadowOut_0
{
    float4 pos_9;
};

[[vertex]] vs_shadow_Result_0 vs_shadow(uint vid_2 [[vertex_id]], uint iid_2 [[instance_id]], VisC_0 constant* ed_c_7 [[buffer(0)]], ulong device* ed_buffers_7 [[buffer(2)]], ulong device* ed_textures_7 [[buffer(1)]], ulong device* ed_samplers_7 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_15;
    (&kernelContext_15)->ed_c_0 = ed_c_7;
    (&kernelContext_15)->ed_buffers_0 = ed_buffers_7;
    (&kernelContext_15)->ed_textures_0 = ed_textures_7;
    (&kernelContext_15)->ed_samplers_0 = ed_samplers_7;
    FrameData_natural_0 device* _S67 = ed_buf_6(ed_c_7->frame_0, &kernelContext_15);
    uint device* _S68 = ed_buf_5((&kernelContext_15)->ed_c_0->visible_0, &kernelContext_15);
    uint entry_11 = *(_S68 + iid_2);
    GpuInstance_natural_0 device* _S69 = ed_buf_4((&kernelContext_15)->ed_c_0->instances_0, &kernelContext_15);
    GpuInstance_natural_0 device* _S70 = _S69 + (entry_11 & 1048575U);
    GpuInstance_natural_0 inst_3 = *_S70;
    thread GpuInstance_natural_0 _S71 = *_S70;
    Vtx_0 device* _S72 = ed_buf_3((&kernelContext_15)->ed_c_0->verts_0, &kernelContext_15);
    Vtx_0 v_3 = *(_S72 + vid_2);
    packed_float4 device* _S73 = ed_buf_2((&kernelContext_15)->ed_c_0->nodes_0, &kernelContext_15);
    float3 node_3 = (float4(*(_S73 + (((&_S71)->node_flags_0) & 65535U))) ).xyz;
    float3 _S74 = float3(v_3.px_0, v_3.py_0, v_3.pz_0);
    _S71 = inst_3;
    float3 _S75 = xform_point_0(&_S71, _S74);
    thread ShadowOut_0 o_2;
    (&o_2)->pos_9 = (((float4(node_3 + _S75, 1.0f)) * (matrix<float,int(4),int(4)> ((&(&_S67->light_0)->shadow_0)->data_3[(&kernelContext_15)->ed_c_0->cascade_0].data_1[int(0)][int(0)], (&(&_S67->light_0)->shadow_0)->data_3[(&kernelContext_15)->ed_c_0->cascade_0].data_1[int(1)][int(0)], (&(&_S67->light_0)->shadow_0)->data_3[(&kernelContext_15)->ed_c_0->cascade_0].data_1[int(2)][int(0)], (&(&_S67->light_0)->shadow_0)->data_3[(&kernelContext_15)->ed_c_0->cascade_0].data_1[int(3)][int(0)], (&(&_S67->light_0)->shadow_0)->data_3[(&kernelContext_15)->ed_c_0->cascade_0].data_1[int(0)][int(1)], (&(&_S67->light_0)->shadow_0)->data_3[(&kernelContext_15)->ed_c_0->cascade_0].data_1[int(1)][int(1)], (&(&_S67->light_0)->shadow_0)->data_3[(&kernelContext_15)->ed_c_0->cascade_0].data_1[int(2)][int(1)], (&(&_S67->light_0)->shadow_0)->data_3[(&kernelContext_15)->ed_c_0->cascade_0].data_1[int(3)][int(1)], (&(&_S67->light_0)->shadow_0)->data_3[(&kernelContext_15)->ed_c_0->cascade_0].data_1[int(0)][int(2)], (&(&_S67->light_0)->shadow_0)->data_3[(&kernelContext_15)->ed_c_0->cascade_0].data_1[int(1)][int(2)], (&(&_S67->light_0)->shadow_0)->data_3[(&kernelContext_15)->ed_c_0->cascade_0].data_1[int(2)][int(2)], (&(&_S67->light_0)->shadow_0)->data_3[(&kernelContext_15)->ed_c_0->cascade_0].data_1[int(3)][int(2)], (&(&_S67->light_0)->shadow_0)->data_3[(&kernelContext_15)->ed_c_0->cascade_0].data_1[int(0)][int(3)], (&(&_S67->light_0)->shadow_0)->data_3[(&kernelContext_15)->ed_c_0->cascade_0].data_1[int(1)][int(3)], (&(&_S67->light_0)->shadow_0)->data_3[(&kernelContext_15)->ed_c_0->cascade_0].data_1[int(2)][int(3)], (&(&_S67->light_0)->shadow_0)->data_3[(&kernelContext_15)->ed_c_0->cascade_0].data_1[int(3)][int(3)]))));
    thread vs_shadow_Result_0 _S76;
    (&_S76)->pos_8 = o_2.pos_9;
    return _S76;
}

struct vs_foliage_Result_0
{
    float4 pos_10 [[position]];
    float2 uv_5 [[user(TEXCOORD)]];
    uint entry_12 [[user(ENTRY)]];
    uint tex_5 [[user(TEXID)]];
};

[[vertex]] vs_foliage_Result_0 vs_foliage(uint vid_3 [[vertex_id]], uint iid_3 [[instance_id]], VisC_0 constant* ed_c_8 [[buffer(0)]], ulong device* ed_buffers_8 [[buffer(2)]], ulong device* ed_textures_8 [[buffer(1)]], ulong device* ed_samplers_8 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_16;
    (&kernelContext_16)->ed_c_0 = ed_c_8;
    (&kernelContext_16)->ed_buffers_0 = ed_buffers_8;
    (&kernelContext_16)->ed_textures_0 = ed_textures_8;
    (&kernelContext_16)->ed_samplers_0 = ed_samplers_8;
    FoliageOut_0 _S77 = foliage_vertex_0(vid_3, iid_3, false, &kernelContext_16);
    thread vs_foliage_Result_0 _S78;
    (&_S78)->pos_10 = _S77.pos_2;
    (&_S78)->uv_5 = _S77.uv_1;
    (&_S78)->entry_12 = _S77.entry_3;
    (&_S78)->tex_5 = _S77.tex_1;
    return _S78;
}

struct vs_shadow_foliage_Result_0
{
    float4 pos_11 [[position]];
    float2 uv_6 [[user(TEXCOORD)]];
    uint entry_13 [[user(ENTRY)]];
    uint tex_6 [[user(TEXID)]];
};

[[vertex]] vs_shadow_foliage_Result_0 vs_shadow_foliage(uint vid_4 [[vertex_id]], uint iid_4 [[instance_id]], VisC_0 constant* ed_c_9 [[buffer(0)]], ulong device* ed_buffers_9 [[buffer(2)]], ulong device* ed_textures_9 [[buffer(1)]], ulong device* ed_samplers_9 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_17;
    (&kernelContext_17)->ed_c_0 = ed_c_9;
    (&kernelContext_17)->ed_buffers_0 = ed_buffers_9;
    (&kernelContext_17)->ed_textures_0 = ed_textures_9;
    (&kernelContext_17)->ed_samplers_0 = ed_samplers_9;
    FoliageOut_0 _S79 = foliage_vertex_0(vid_4, iid_4, true, &kernelContext_17);
    thread vs_shadow_foliage_Result_0 _S80;
    (&_S80)->pos_11 = _S79.pos_2;
    (&_S80)->uv_6 = _S79.uv_1;
    (&_S80)->entry_13 = _S79.entry_3;
    (&_S80)->tex_6 = _S79.tex_1;
    return _S80;
}

