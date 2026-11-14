#include <metal_stdlib>
#include <metal_math>
#include <metal_texture>
using namespace metal;
constant array<uint, int(5)> GRASS_SEGS_0 = { { 6U, 5U, 4U, 3U, 2U } };
struct GrassType_natural_0
{
    packed_float4 a_0;
    packed_float4 b_0;
    packed_float4 root_0;
    packed_float4 mid_0;
    packed_float4 tip_0;
    packed_float4 dry_0;
    packed_float4 shell_0;
    packed_float4 carpet_0;
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

GrassType_natural_0 device* ed_buf_0(uint i_0, KernelContext_0 thread* kernelContext_0)
{
    return (GrassType_natural_0 device*)kernelContext_0->ed_buffers_0[i_0];
}

struct GrassClump_0
{
    float px_0;
    float py_0;
    float pz_0;
    uint node_kind_0;
    uint up_0;
    uint facing_0;
    uint seed_0;
    uint hw_0;
};

GrassClump_0 device* ed_buf_1(uint i_1, KernelContext_0 thread* kernelContext_1)
{
    return (GrassClump_0 device*)kernelContext_1->ed_buffers_0[i_1];
}

struct GpuImpKind_natural_0
{
    packed_uint4 tex_0;
    packed_float4 geo_0;
    packed_float4 leaf_0;
    packed_float4 bark_0;
};

GpuImpKind_natural_0 device* ed_buf_2(uint i_2, KernelContext_0 thread* kernelContext_2)
{
    return (GpuImpKind_natural_0 device*)kernelContext_2->ed_buffers_0[i_2];
}

struct GpuTree_0
{
    float px_1;
    float py_1;
    float pz_1;
    uint node_kind_1;
    float scale_0;
    uint up_1;
    uint yaw_widen_0;
    uint user_0;
    uint season_0;
};

GpuTree_0 device* ed_buf_3(uint i_3, KernelContext_0 thread* kernelContext_3)
{
    return (GpuTree_0 device*)kernelContext_3->ed_buffers_0[i_3];
}

struct GpuMaterial_natural_0
{
    packed_float4 base_0;
    packed_float4 params_0;
    packed_float4 shade_0;
    packed_uint4 tex_1;
    packed_float4 uv_0;
};

GpuMaterial_natural_0 device* ed_buf_4(uint i_4, KernelContext_0 thread* kernelContext_4)
{
    return (GpuMaterial_natural_0 device*)kernelContext_4->ed_buffers_0[i_4];
}

packed_uint2 device* ed_buf_5(uint i_5, KernelContext_0 thread* kernelContext_5)
{
    return (packed_uint2 device*)kernelContext_5->ed_buffers_0[i_5];
}

constant array<uint, int(64)> BAYER8_0 = { { 0U, 32U, 8U, 40U, 2U, 34U, 10U, 42U, 48U, 16U, 56U, 24U, 50U, 18U, 58U, 26U, 12U, 44U, 4U, 36U, 14U, 46U, 6U, 38U, 60U, 28U, 52U, 20U, 62U, 30U, 54U, 22U, 3U, 35U, 11U, 43U, 1U, 33U, 9U, 41U, 51U, 19U, 59U, 27U, 49U, 17U, 57U, 25U, 15U, 47U, 7U, 39U, 13U, 45U, 5U, 37U, 63U, 31U, 55U, 23U, 61U, 29U, 53U, 21U } };
packed_float4 device* ed_buf_6(uint i_6, KernelContext_0 thread* kernelContext_6)
{
    return (packed_float4 device*)kernelContext_6->ed_buffers_0[i_6];
}

struct Vtx_0
{
    float px_2;
    float py_2;
    float pz_2;
    uint n_0;
};

Vtx_0 device* ed_buf_7(uint i_7, KernelContext_0 thread* kernelContext_7)
{
    return (Vtx_0 device*)kernelContext_7->ed_buffers_0[i_7];
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

GpuInstance_natural_0 device* ed_buf_8(uint i_8, KernelContext_0 thread* kernelContext_8)
{
    return (GpuInstance_natural_0 device*)kernelContext_8->ed_buffers_0[i_8];
}

uint device* ed_buf_9(uint i_9, KernelContext_0 thread* kernelContext_9)
{
    return (uint device*)kernelContext_9->ed_buffers_0[i_9];
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
    packed_float4 up_2;
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
    array<packed_float4, int(64)> data_6;
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

struct ShellData_natural_0
{
    packed_uint4 cfg_5;
    packed_int4 off_0;
    packed_float4 c_0;
    packed_float4 fu_0;
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
    packed_float4 params_1;
    packed_float4 proj_0;
    packed_uint4 cull_0;
    array<HzbLevel_0, int(16)> hzb_0;
    LightData_natural_0 light_0;
    AtmosData_natural_0 atmos_0;
    WindData_natural_0 wind_1;
    ImpData_natural_0 imp_0;
    SeasonData_natural_0 season_1;
    GrassData_natural_0 grass_0;
    ShellData_natural_0 shell_1;
};

FrameData_natural_0 device* ed_buf_10(uint i_10, KernelContext_0 thread* kernelContext_10)
{
    return (FrameData_natural_0 device*)kernelContext_10->ed_buffers_0[i_10];
}

float3 xform_point_0(const GpuInstance_natural_0 thread* i_11, float3 p_0)
{
    float4 _S1 = float4((&i_11->m_0)->data_0[int(0)]) ;
    float4 _S2 = float4((&i_11->m_0)->data_0[int(1)]) ;
    float4 _S3 = float4((&i_11->m_0)->data_0[int(2)]) ;
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
    thread KernelContext_0 kernelContext_11;
    (&kernelContext_11)->ed_c_0 = ed_c_1;
    (&kernelContext_11)->ed_buffers_0 = ed_buffers_1;
    (&kernelContext_11)->ed_textures_0 = ed_textures_1;
    (&kernelContext_11)->ed_samplers_0 = ed_samplers_1;
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
    thread KernelContext_0 kernelContext_12;
    (&kernelContext_12)->ed_c_0 = ed_c_2;
    (&kernelContext_12)->ed_buffers_0 = ed_buffers_2;
    (&kernelContext_12)->ed_textures_0 = ed_textures_2;
    (&kernelContext_12)->ed_samplers_0 = ed_samplers_2;
    uint2 p_1 = uint2(pos_1.xy) & (uint2(7U) );
    if(((((BAYER8_0[p_1.y * 8U + p_1.x] + ((((_S6.entry_2) & 1048575U) * 2654435761U) >> 26U)) & 63U) * 4U + 2U) < (((_S6.entry_2) >> 22U) & 255U)) == ((((_S6.entry_2) >> 30U) & 1U) != 0U))
    {
        discard_fragment();
    }
    pixelOutput_1 _S7 = { vis_value_0(_S6.entry_2, prim_2) };
    return _S7;
}

float gc_keep_0(float d_0, float reach_0)
{
    float r0_0 = 0.19499999284744263f * reach_0;
    float k_0 = r0_0 / max(d_0, r0_0);
    return k_0 * k_0 * (1.0f - smoothstep(0.69999998807907104f * reach_0, reach_0, d_0));
}

float3 gc_scale_0(const GpuInstance_natural_0 thread* inst_0, float d_1)
{
    uint _S8 = inst_0->pad_1[int(2)];
    if((((inst_0->pad_1[int(2)]) >> 20U) & 1U) != 0U)
    {
        return float3(1.0f) ;
    }
    float k_1 = gc_keep_0(d_1, float((_S8 >> 12U) & 255U));
    float s_0 = saturate((k_1 - float(_S8 & 4095U) * 0.00024420025874860585f) / (0.30000001192092896f * k_1 + 0.0010000000474974513f));
    if(s_0 <= 0.0f)
    {
        return float3(0.0f) ;
    }
    float gx_0 = clamp(sqrt(1.0f / max(k_1, 9.9999997473787516e-05f)), 1.0f, 2.0f);
    return float3(gx_0, min(gx_0, 1.25f), gx_0) * float3(s_0) ;
}

float length_squared_0(float3 v_0)
{
    return dot(v_0, v_0);
}

float max_scale_0(const GpuInstance_natural_0 thread* i_12)
{
    float4 _S9 = float4((&i_12->m_0)->data_0[int(0)]) ;
    float4 _S10 = float4((&i_12->m_0)->data_0[int(1)]) ;
    float4 _S11 = float4((&i_12->m_0)->data_0[int(2)]) ;
    return sqrt(max(max(length_squared_0(float3(_S9.x, _S10.x, _S11.x)), length_squared_0(float3(_S9.y, _S10.y, _S11.y))), length_squared_0(float3(_S9.z, _S10.z, _S11.z))));
}

float2 unpack_snorm2x16_0(uint p_2)
{
    return clamp(float2(int2(int(p_2 << 16U) >> 16U, int(p_2) >> 16U)) / float2(32767.0f) , float2(-1.0f) , float2(1.0f) );
}

float3 oct_decode_0(uint packed_0)
{
    float2 e_0 = unpack_snorm2x16_0(packed_0);
    float _S12 = e_0.x;
    float _S13 = e_0.y;
    float _S14 = 1.0f - abs(_S12) - abs(_S13);
    thread float3 n_1 = float3(_S12, _S13, _S14);
    float t_0 = saturate(- _S14);
    float _S15;
    if(_S12 >= 0.0f)
    {
        _S15 = - t_0;
    }
    else
    {
        _S15 = t_0;
    }
    n_1.x = n_1.x + _S15;
    if((n_1.y) >= 0.0f)
    {
        _S15 = - t_0;
    }
    else
    {
        _S15 = t_0;
    }
    n_1.y = n_1.y + _S15;
    return normalize(n_1);
}

float3 xform_normal_0(const GpuInstance_natural_0 thread* i_13, float3 n_2)
{
    float4 _S16 = float4((&i_13->m_0)->data_0[int(0)]) ;
    float4 _S17 = float4((&i_13->m_0)->data_0[int(1)]) ;
    float4 _S18 = float4((&i_13->m_0)->data_0[int(2)]) ;
    float3 a0_0 = float3(_S16.x, _S17.x, _S18.x);
    float3 a1_0 = float3(_S16.y, _S17.y, _S18.y);
    float3 a2_0 = float3(_S16.z, _S17.z, _S18.z);
    return cross(a1_0, a2_0) * float3(n_2.x)  + cross(a2_0, a0_0) * float3(n_2.y)  + cross(a0_0, a1_0) * float3(n_2.z) ;
}

float3 tree_vertex_0(FrameData_natural_0 device* f_0, const GpuInstance_natural_0 thread* inst_1, float3 node_0, float3 pm_0, uint n_3, uint aux_0)
{
    uint _S19 = inst_1->pad_1[int(0)];
    float3 origin_0 = float3((float4((&inst_1->m_0)->data_0[int(0)]) ).w, (float4((&inst_1->m_0)->data_0[int(1)]) ).w, (float4((&inst_1->m_0)->data_0[int(2)]) ).w);
    bool _S20;
    if((aux_0 & 134217728U) != 0U)
    {
        _S20 = ((_S19 >> 16U) & (1U << (aux_0 >> 29U))) != 0U;
    }
    else
    {
        _S20 = false;
    }
    if(_S20)
    {
        return origin_0;
    }
    float3 _S21;
    if(((inst_1->node_flags_0) & 131072U) != 0U)
    {
        float3 _S22 = gc_scale_0(inst_1, length(node_0 + origin_0));
        if((_S22.x) <= 0.0f)
        {
            return origin_0;
        }
        _S21 = pm_0 * _S22;
    }
    else
    {
        _S21 = pm_0;
    }
    float3 _S23 = xform_point_0(inst_1, _S21);
    WindData_natural_0 w_0 = f_0->wind_1;
    float4 _S24 = float4(f_0->wind_1.cfg_2) ;
    if((_S24.x) < 0.5f)
    {
        return _S23;
    }
    float3 _S25 = node_0 + origin_0;
    float _S26 = _S24.y;
    float fade_0 = saturate((_S26 - length(_S25)) / max(0.25f * _S26, 1.0f));
    if(fade_0 <= 0.0f)
    {
        return _S23;
    }
    float q_0 = float((aux_0 >> 11U) & 127U) / 127.0f;
    float sway_0 = q_0 * q_0 * 4.0f;
    float flutter_0 = float((aux_0 >> 18U) & 7U) / 7.0f;
    float phase_0 = float((aux_0 >> 21U) & 63U) / 64.0f + float(_S19 >> 24U) / 255.0f;
    float4 _S27 = float4(w_0.wind_0) ;
    float2 _S28 = _S27.xz;
    float speed_0 = length(_S28);
    float2 wd_0;
    if(speed_0 > 0.0010000000474974513f)
    {
        wd_0 = _S28 / float2(speed_0) ;
    }
    else
    {
        wd_0 = float2(0.0f, 0.0f);
    }
    float4 _S29 = float4(w_0.gust_0) ;
    float s_1 = 0.5f + 0.5f * sin((_S29.x + dot(_S25.xz, wd_0) / max(_S29.y, 1.0f)) * 6.2831854820251465f);
    float gust_1 = s_1 * s_1 * s_1 * _S29.z;
    float t_1 = _S27.w;
    float _S30 = speed_0 * 0.10000000149011612f * fade_0;
    float _S31 = max_scale_0(inst_1);
    float k_2 = _S30 * _S31;
    float rock_0 = 0.55000001192092896f + gust_1 + 0.30000001192092896f * sin(t_1 * 1.7000000476837158f + phase_0 * 6.2831854820251465f) + 0.15000000596046448f * sin(t_1 * 0.62999999523162842f + phase_0 * 19.0f);
    thread float3 d_2 = float3(wd_0.x, 0.0f, wd_0.y) * float3((sway_0 * k_2 * rock_0)) ;
    d_2.y = d_2.y - 0.25f * sway_0 * k_2 * rock_0 * rock_0 * 0.30000001192092896f;
    float3 _S32 = xform_normal_0(inst_1, oct_decode_0(n_3));
    return _S23 + d_2 + normalize(_S32) * float3((flutter_0 * _S29.w * (0.60000002384185791f + gust_1) * 0.05000000074505806f * fade_0 * sin(t_1 * 11.0f + phase_0 * 40.0f + dot(_S23, float3(3.0999999046325684f, 1.7000000476837158f, 2.2999999523162842f)))))  * float3(_S31) ;
}

float2 tree_uv_0(uint w_1)
{
    return float2(float(w_1 & 65535U), float(w_1 >> 16U)) * float2(1.5259021893143654e-05f) ;
}

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
    thread LeafSeason_0 s_2;
    (&s_2)->leaves_0 = 1.0f;
    (&s_2)->green_0 = 1.0f;
    (&s_2)->colour_0 = 0.0f;
    (&s_2)->evergreen_0 = 0.0f;
    (&s_2)->target_0 = float3(0.0f, 0.0f, 0.0f);
    return s_2;
}

float season_ramp_0(float lo_0, float hi_0, float x_0)
{
    return smoothstep(lo_0, hi_0, x_0);
}

LeafSeason_0 season_state_0(FrameData_natural_0 device* f_1, uint row_0, float shift_0, bool south_0)
{
    if(((float4((&f_1->season_1)->cfg_4) ).x) <= 0.0f)
    {
        return season_off_0();
    }
    float d_3 = (float4((&f_1->season_1)->cfg_4) ).x;
    float d_4;
    if(south_0)
    {
        d_4 = fmod(d_3 + 182.625f, 365.25f);
    }
    else
    {
        d_4 = d_3;
    }
    float ds_0 = d_4 - shift_0;
    float da_0 = d_4 + (float4((&f_1->season_1)->cfg_4) ).y * shift_0;
    uint _S33 = (row_0 & 31U) * 2U;
    float4 _S34 = float4((&(&f_1->season_1)->rows_0)->data_6[_S33]) ;
    float4 _S35 = float4((&(&f_1->season_1)->rows_0)->data_6[_S33 + 1U]) ;
    thread LeafSeason_0 s_3;
    (&s_3)->target_0 = _S35.xyz;
    float _S36 = _S34.w;
    (&s_3)->evergreen_0 = _S36;
    if(_S36 > 0.5f)
    {
        float _S37 = _S34.y;
        float a_1 = season_ramp_0(_S37 - 10.0f, _S37 + 30.0f, da_0);
        float _S38 = _S34.x;
        float rel_0 = 1.0f - season_ramp_0(_S38 - 10.0f, _S38 + 20.0f, ds_0);
        (&s_3)->green_0 = 1.0f;
        (&s_3)->leaves_0 = 1.0f;
        (&s_3)->colour_0 = _S35.w * max(a_1, rel_0);
    }
    else
    {
        float _S39 = _S34.x;
        float _S40 = season_ramp_0(_S39 - 6.0f, _S39 + 8.0f, ds_0);
        (&s_3)->green_0 = _S40;
        float _S41 = _S34.y;
        (&s_3)->colour_0 = season_ramp_0(_S41 - 8.0f, _S41 + 14.0f, da_0);
        float _S42 = _S34.z;
        (&s_3)->leaves_0 = _S40 * (1.0f - season_ramp_0(_S42 - 10.0f, _S42 + 10.0f, da_0));
    }
    return s_3;
}

LeafSeason_0 season_of_word_0(FrameData_natural_0 device* f_2, uint word_0)
{
    LeafSeason_0 _S43 = season_state_0(f_2, word_0 & 255U, float(int(word_0) >> 16U) * 0.0625f, ((word_0 >> 8U) & 1U) != 0U);
    return _S43;
}

struct FoliageOut_0
{
    float4 pos_2;
    float2 uv_1;
    [[flat]] uint entry_3;
    [[flat]] uint tex_2;
    [[flat]] float2 sd_0;
};

FoliageOut_0 foliage_vertex_0(uint vid_0, uint iid_0, bool shadow_1, KernelContext_0 thread* kernelContext_13)
{
    FrameData_natural_0 device* _S44 = ed_buf_10(kernelContext_13->ed_c_0->frame_0, kernelContext_13);
    uint device* _S45 = ed_buf_9(kernelContext_13->ed_c_0->visible_0, kernelContext_13);
    uint entry_4 = *(_S45 + iid_0);
    GpuInstance_natural_0 device* _S46 = ed_buf_8(kernelContext_13->ed_c_0->instances_0, kernelContext_13);
    GpuInstance_natural_0 device* _S47 = _S46 + (entry_4 & 1048575U);
    GpuInstance_natural_0 inst_2 = *_S47;
    thread GpuInstance_natural_0 _S48 = *_S47;
    Vtx_0 device* _S49 = ed_buf_7(kernelContext_13->ed_c_0->verts_0, kernelContext_13);
    Vtx_0 v_1 = *(_S49 + vid_0);
    packed_uint2 device* _S50 = ed_buf_5(kernelContext_13->ed_c_0->uvs_0, kernelContext_13);
    uint2 _S51 = uint2(*(_S50 + vid_0)) ;
    packed_float4 device* _S52 = ed_buf_6(kernelContext_13->ed_c_0->nodes_0, kernelContext_13);
    float3 node_1 = (float4(*(_S52 + (((&_S48)->node_flags_0) & 65535U))) ).xyz;
    float3 _S53 = float3(v_1.px_2, v_1.py_2, v_1.pz_2);
    uint _S54 = _S51.y;
    _S48 = inst_2;
    float3 _S55 = tree_vertex_0(_S44, &_S48, node_1, _S53, v_1.n_0, _S54);
    float3 p_3 = node_1 + _S55;
    thread FoliageOut_0 o_0;
    matrix<float,int(4),int(4)>  _S56;
    if(shadow_1)
    {
        _S56 = matrix<float,int(4),int(4)> ((&(&_S44->light_0)->shadow_0)->data_3[kernelContext_13->ed_c_0->cascade_0].data_1[int(0)][int(0)], (&(&_S44->light_0)->shadow_0)->data_3[kernelContext_13->ed_c_0->cascade_0].data_1[int(1)][int(0)], (&(&_S44->light_0)->shadow_0)->data_3[kernelContext_13->ed_c_0->cascade_0].data_1[int(2)][int(0)], (&(&_S44->light_0)->shadow_0)->data_3[kernelContext_13->ed_c_0->cascade_0].data_1[int(3)][int(0)], (&(&_S44->light_0)->shadow_0)->data_3[kernelContext_13->ed_c_0->cascade_0].data_1[int(0)][int(1)], (&(&_S44->light_0)->shadow_0)->data_3[kernelContext_13->ed_c_0->cascade_0].data_1[int(1)][int(1)], (&(&_S44->light_0)->shadow_0)->data_3[kernelContext_13->ed_c_0->cascade_0].data_1[int(2)][int(1)], (&(&_S44->light_0)->shadow_0)->data_3[kernelContext_13->ed_c_0->cascade_0].data_1[int(3)][int(1)], (&(&_S44->light_0)->shadow_0)->data_3[kernelContext_13->ed_c_0->cascade_0].data_1[int(0)][int(2)], (&(&_S44->light_0)->shadow_0)->data_3[kernelContext_13->ed_c_0->cascade_0].data_1[int(1)][int(2)], (&(&_S44->light_0)->shadow_0)->data_3[kernelContext_13->ed_c_0->cascade_0].data_1[int(2)][int(2)], (&(&_S44->light_0)->shadow_0)->data_3[kernelContext_13->ed_c_0->cascade_0].data_1[int(3)][int(2)], (&(&_S44->light_0)->shadow_0)->data_3[kernelContext_13->ed_c_0->cascade_0].data_1[int(0)][int(3)], (&(&_S44->light_0)->shadow_0)->data_3[kernelContext_13->ed_c_0->cascade_0].data_1[int(1)][int(3)], (&(&_S44->light_0)->shadow_0)->data_3[kernelContext_13->ed_c_0->cascade_0].data_1[int(2)][int(3)], (&(&_S44->light_0)->shadow_0)->data_3[kernelContext_13->ed_c_0->cascade_0].data_1[int(3)][int(3)]);
    }
    else
    {
        _S56 = matrix<float,int(4),int(4)> (_S44->view_proj_0.data_1[int(0)][int(0)], _S44->view_proj_0.data_1[int(1)][int(0)], _S44->view_proj_0.data_1[int(2)][int(0)], _S44->view_proj_0.data_1[int(3)][int(0)], _S44->view_proj_0.data_1[int(0)][int(1)], _S44->view_proj_0.data_1[int(1)][int(1)], _S44->view_proj_0.data_1[int(2)][int(1)], _S44->view_proj_0.data_1[int(3)][int(1)], _S44->view_proj_0.data_1[int(0)][int(2)], _S44->view_proj_0.data_1[int(1)][int(2)], _S44->view_proj_0.data_1[int(2)][int(2)], _S44->view_proj_0.data_1[int(3)][int(2)], _S44->view_proj_0.data_1[int(0)][int(3)], _S44->view_proj_0.data_1[int(1)][int(3)], _S44->view_proj_0.data_1[int(2)][int(3)], _S44->view_proj_0.data_1[int(3)][int(3)]);
    }
    (&o_0)->pos_2 = (((float4(p_3, 1.0f)) * (_S56)));
    (&o_0)->uv_1 = tree_uv_0(_S51.x);
    (&o_0)->entry_3 = entry_4;
    bool _S57 = (_S54 & 134217728U) != 0U;
    uint _S58;
    if(_S57)
    {
        GpuMaterial_natural_0 device* _S59 = ed_buf_4(kernelContext_13->ed_c_0->materials_0, kernelContext_13);
        _S58 = (uint4((_S59 + (&_S48)->material_0)->tex_1) ).x;
    }
    else
    {
        _S58 = 4294967295U;
    }
    (&o_0)->tex_2 = _S58;
    (&o_0)->sd_0 = float2(0.0f, 1.0f);
    bool _S60;
    if(_S57)
    {
        _S60 = ((float4((&_S44->season_1)->cfg_4) ).x) > 0.0f;
    }
    else
    {
        _S60 = false;
    }
    if(_S60)
    {
        array<uint, int(4)> _S61 = (&_S48)->pad_1;
        LeafSeason_0 _S62 = season_of_word_0(_S44, (&_S48)->pad_1[int(1)]);
        (&o_0)->sd_0 = float2(fract(float((_S54 >> 21U) & 63U) * 0.015625f + float(_S54 >> 29U) * 0.13699999451637268f + float((_S61[int(0)]) >> 24U) * 0.0024235292803496122f + fract(float(vid_0 >> 2U) * 0.75487768650054932f)), _S62.leaves_0);
    }
    return o_0;
}

void foliage_clip_0(const FoliageOut_0 thread* i_14, KernelContext_0 thread* kernelContext_14)
{
    if((i_14->sd_0.x) >= (i_14->sd_0.y))
    {
        discard_fragment();
    }
    uint _S63 = i_14->tex_2;
    bool _S64;
    if((i_14->tex_2) != 4294967295U)
    {
        texture2d<float, access::sample> _S65 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_14->ed_textures_0))[(_S63)]));
        sampler _S66 = ((reinterpret_cast<device sampler*>((kernelContext_14->ed_samplers_0))[(kernelContext_14->ed_c_0->smp_0)]));
        _S64 = (((_S65).sample((_S66), (i_14->uv_1))).w) < 0.5f;
    }
    else
    {
        _S64 = false;
    }
    if(_S64)
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
    [[flat]] float2 sd_1 [[user(SEASON)]];
};

[[fragment]] pixelOutput_2 fs_foliage(pixelInput_2 _S67 [[stage_in]], uint prim_3 [[primitive_id]], float4 pos_3 [[position]], VisC_0 constant* ed_c_3 [[buffer(0)]], ulong device* ed_buffers_3 [[buffer(2)]], ulong device* ed_textures_3 [[buffer(1)]], ulong device* ed_samplers_3 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_15;
    (&kernelContext_15)->ed_c_0 = ed_c_3;
    (&kernelContext_15)->ed_buffers_0 = ed_buffers_3;
    (&kernelContext_15)->ed_textures_0 = ed_textures_3;
    (&kernelContext_15)->ed_samplers_0 = ed_samplers_3;
    thread FoliageOut_0 _S68;
    (&_S68)->pos_2 = pos_3;
    (&_S68)->uv_1 = _S67.uv_2;
    (&_S68)->entry_3 = _S67.entry_5;
    (&_S68)->tex_2 = _S67.tex_3;
    (&_S68)->sd_0 = _S67.sd_1;
    foliage_clip_0(&_S68, &kernelContext_15);
    pixelOutput_2 _S69 = { vis_value_0(_S67.entry_5, prim_3) };
    return _S69;
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
    [[flat]] float2 sd_2 [[user(SEASON)]];
};

[[fragment]] pixelOutput_3 fs_foliage_dither(pixelInput_3 _S70 [[stage_in]], uint prim_4 [[primitive_id]], float4 pos_4 [[position]], VisC_0 constant* ed_c_4 [[buffer(0)]], ulong device* ed_buffers_4 [[buffer(2)]], ulong device* ed_textures_4 [[buffer(1)]], ulong device* ed_samplers_4 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_16;
    (&kernelContext_16)->ed_c_0 = ed_c_4;
    (&kernelContext_16)->ed_buffers_0 = ed_buffers_4;
    (&kernelContext_16)->ed_textures_0 = ed_textures_4;
    (&kernelContext_16)->ed_samplers_0 = ed_samplers_4;
    thread FoliageOut_0 _S71;
    (&_S71)->pos_2 = pos_4;
    (&_S71)->uv_1 = _S70.uv_3;
    (&_S71)->entry_3 = _S70.entry_6;
    (&_S71)->tex_2 = _S70.tex_4;
    (&_S71)->sd_0 = _S70.sd_2;
    foliage_clip_0(&_S71, &kernelContext_16);
    uint2 p_4 = uint2(pos_4.xy) & (uint2(7U) );
    if(((((BAYER8_0[p_4.y * 8U + p_4.x] + ((((_S70.entry_6) & 1048575U) * 2654435761U) >> 26U)) & 63U) * 4U + 2U) < (((_S70.entry_6) >> 22U) & 255U)) == ((((_S70.entry_6) >> 30U) & 1U) != 0U))
    {
        discard_fragment();
    }
    pixelOutput_3 _S72 = { vis_value_0(_S70.entry_6, prim_4) };
    return _S72;
}

struct pixelInput_4
{
    float2 uv_4 [[user(TEXCOORD)]];
    [[flat]] uint entry_7 [[user(ENTRY)]];
    [[flat]] uint tex_5 [[user(TEXID)]];
    [[flat]] float2 sd_3 [[user(SEASON)]];
};

[[fragment]] void fs_shadow_foliage(pixelInput_4 _S73 [[stage_in]], float4 pos_5 [[position]], VisC_0 constant* ed_c_5 [[buffer(0)]], ulong device* ed_buffers_5 [[buffer(2)]], ulong device* ed_textures_5 [[buffer(1)]], ulong device* ed_samplers_5 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_17;
    (&kernelContext_17)->ed_c_0 = ed_c_5;
    (&kernelContext_17)->ed_buffers_0 = ed_buffers_5;
    (&kernelContext_17)->ed_textures_0 = ed_textures_5;
    (&kernelContext_17)->ed_samplers_0 = ed_samplers_5;
    thread FoliageOut_0 _S74;
    (&_S74)->pos_2 = pos_5;
    (&_S74)->uv_1 = _S73.uv_4;
    (&_S74)->entry_3 = _S73.entry_7;
    (&_S74)->tex_2 = _S73.tex_5;
    (&_S74)->sd_0 = _S73.sd_3;
    foliage_clip_0(&_S74, &kernelContext_17);
    return;
}

float3 arc_from_y_0(float3 up_3, float3 v_2)
{
    float3 a_2 = float3(up_3.z, 0.0f, - up_3.x);
    float w_2 = 1.0f + up_3.y;
    float l_0 = sqrt(dot(a_2, a_2) + w_2 * w_2);
    if(l_0 < 9.99999997475242708e-07f)
    {
        return float3(v_2.x, - v_2.y, - v_2.z);
    }
    float3 a_3 = a_2 / float3(l_0) ;
    float3 t_2 = float3(2.0f)  * cross(a_3, v_2);
    return v_2 + float3((w_2 / l_0))  * t_2 + cross(a_3, t_2);
}

struct ImpTree_0
{
    float3 c_1;
    float3 ex_0;
    float3 up_4;
    float3 ez_0;
    float s_4;
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
    float _S75 = t_3->s_4 * t_3->wd_1;
    return float3(dot(v_3, t_3->ex_0) / _S75, dot(v_3, t_3->up_4) / t_3->s_4, dot(v_3, t_3->ez_0) / _S75);
}

float2 imp_oct_encode_0(float3 d_5)
{
    thread float3 _S76 = d_5;
    _S76.y = max(d_5.y, 0.0f);
    float3 _S77 = _S76 / float3(max(abs(_S76.x) + abs(_S76.y) + abs(_S76.z), 9.99999971718068537e-10f)) ;
    _S76 = _S77;
    float _S78 = _S77.x;
    float _S79 = _S77.z;
    float2 _S80 = float2(0.5f) ;
    return float2(_S78 + _S79, _S78 - _S79) * _S80 + _S80;
}

float3 imp_oct_decode_0(float2 uv_5)
{
    float2 g_0 = uv_5 * float2(2.0f)  - float2(1.0f) ;
    float _S81 = g_0.x;
    float _S82 = g_0.y;
    float x_1 = (_S81 + _S82) * 0.5f;
    float z_0 = (_S81 - _S82) * 0.5f;
    return normalize(float3(x_1, 1.0f - abs(x_1) - abs(z_0), z_0));
}

ImpTree_0 imp_tree_0(const GpuTree_0 thread* g_1, const GpuImpKind_natural_0 thread* k_3, float3 node_2, float3 sun_dir_1, bool sun_1)
{
    thread ImpTree_0 t_4;
    float3 _S83 = oct_decode_0(g_1->up_1);
    (&t_4)->up_4 = _S83;
    float yaw_0 = float((g_1->yaw_widen_0) & 65535U) / 65535.0f * 6.2831854820251465f;
    (&t_4)->wd_1 = 0.5f + float((g_1->yaw_widen_0) >> 16U) / 65535.0f;
    (&t_4)->s_4 = g_1->scale_0;
    float _S84 = cos(yaw_0);
    float _S85 = sin(yaw_0);
    (&t_4)->ex_0 = arc_from_y_0(_S83, float3(_S84, 0.0f, - _S85));
    (&t_4)->ez_0 = arc_from_y_0(_S83, float3(_S85, 0.0f, _S84));
    float4 _S86 = float4(k_3->geo_0) ;
    (&t_4)->r_0 = _S86.y;
    (&t_4)->c_1 = node_2 + float3(g_1->px_1, g_1->py_1, g_1->pz_1) + _S83 * float3((_S86.x * g_1->scale_0)) ;
    ImpTree_0 _S87 = t_4;
    float3 _S88;
    if(sun_1)
    {
        _S88 = sun_dir_1 * float3(1.0e+05f) ;
    }
    else
    {
        _S88 = - (&t_4)->c_1;
    }
    thread ImpTree_0 _S89 = _S87;
    float3 _S90 = imp_to_tree_0(&_S89, _S88);
    (&t_4)->cam_0 = _S90;
    uint _S91 = (uint4(k_3->tex_0) ).w;
    float n_4 = float(_S91);
    float2 _S92 = float2(n_4) ;
    float2 _S93 = float2(0.5f) ;
    float _S94 = n_4 - 1.0f;
    float2 gpos_0 = clamp(imp_oct_encode_0(normalize(_S90)) * _S92 - _S93, float2(0.0f, 0.0f), float2(_S94, _S94));
    float _S95 = n_4 - 2.0f;
    float2 _S96 = min(floor(gpos_0), float2(_S95, _S95));
    float2 fr_0 = gpos_0 - _S96;
    uint2 _S97 = uint2(_S96);
    uint2 c1_0 = _S97 + uint2(1U, 0U);
    uint2 c2_0 = _S97 + uint2(0U, 1U);
    float _S98 = fr_0.x;
    float _S99 = fr_0.y;
    float _S100 = _S98 + _S99;
    uint2 c0_0;
    if(_S100 < 1.0f)
    {
        (&t_4)->w_3 = float3(1.0f - _S98 - _S99, _S98, _S99);
        c0_0 = _S97;
    }
    else
    {
        uint2 _S101 = _S97 + uint2(1U, 1U);
        (&t_4)->w_3 = float3(_S100 - 1.0f, 1.0f - _S99, 1.0f - _S98);
        c0_0 = _S101;
    }
    (&t_4)->fi_0 = uint3(c0_0.y * _S91 + c0_0.x, c1_0.y * _S91 + c1_0.x, c2_0.y * _S91 + c2_0.x);
    (&t_4)->d0_0 = imp_oct_decode_0((float2(c0_0) + _S93) / _S92);
    (&t_4)->d1_0 = imp_oct_decode_0((float2(c1_0) + _S93) / _S92);
    (&t_4)->d2_0 = imp_oct_decode_0((float2(c2_0) + _S93) / _S92);
    return t_4;
}

void imp_view_basis_0(float3 v_4, float3 thread* right_0, float3 thread* up_5)
{
    float3 u0_0 = float3(0.0f, 1.0f, 0.0f) - v_4 * float3(v_4.y) ;
    float3 _S102;
    if((dot(u0_0, u0_0)) > 9.99999997475242708e-07f)
    {
        _S102 = normalize(u0_0);
    }
    else
    {
        _S102 = float3(0.0f, 0.0f, 1.0f);
    }
    *up_5 = _S102;
    *right_0 = normalize(cross(_S102, v_4));
    return;
}

float2 imp_frame_uv_0(const ImpTree_0 thread* t_5, float3 d_6, float3 q_1, float half_size_0)
{
    float3 _S103 = t_5->cam_0;
    float3 dir_0 = q_1 - t_5->cam_0;
    float den_0 = dot(dir_0, d_6);
    float k_4;
    if((abs(den_0)) > 9.99999997475242708e-07f)
    {
        k_4 = - dot(_S103, d_6) / den_0;
    }
    else
    {
        k_4 = 1.0f;
    }
    float3 h_0 = _S103 + dir_0 * float3(k_4) ;
    thread float3 r_1;
    thread float3 u_0;
    imp_view_basis_0(d_6, &r_1, &u_0);
    float _S104 = 2.0f * half_size_0;
    return float2(0.5f + dot(h_0, r_1) / _S104, 0.5f - dot(h_0, u_0) / _S104);
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
    [[flat]] float lv_0;
};

ImpOut_0 imp_vertex_0(uint vid_1, uint iid_1, bool shadow_2, KernelContext_0 thread* kernelContext_18)
{
    FrameData_natural_0 device* _S105 = ed_buf_10(kernelContext_18->ed_c_0->frame_0, kernelContext_18);
    uint device* _S106 = ed_buf_9(kernelContext_18->ed_c_0->visible_0, kernelContext_18);
    uint entry_9 = *(_S106 + iid_1);
    GpuTree_0 device* _S107 = ed_buf_3(kernelContext_18->ed_c_0->instances_0, kernelContext_18);
    GpuTree_0 g_2 = *(_S107 + (entry_9 & 16777215U));
    GpuImpKind_natural_0 device* _S108 = ed_buf_2(kernelContext_18->ed_c_0->verts_0, kernelContext_18);
    GpuImpKind_natural_0 device* _S109 = _S108 + (((g_2.node_kind_1) >> 12U) & 4095U);
    GpuImpKind_natural_0 k_5 = *_S109;
    thread GpuImpKind_natural_0 _S110 = *_S109;
    packed_float4 device* _S111 = ed_buf_6(kernelContext_18->ed_c_0->nodes_0, kernelContext_18);
    float3 node_3 = (float4(*(_S111 + ((g_2.node_kind_1) & 4095U))) ).xyz;
    float3 l_1 = (float4(_S105->sun_dir_0) ).xyz;
    thread GpuTree_0 _S112 = g_2;
    _S110 = k_5;
    ImpTree_0 _S113 = imp_tree_0(&_S112, &_S110, node_3, l_1, shadow_2);
    float3 vw_0;
    if(shadow_2)
    {
        vw_0 = l_1;
    }
    else
    {
        vw_0 = normalize(- _S113.c_1);
    }
    float3 u0_1 = _S113.up_4 - vw_0 * float3(dot(vw_0, _S113.up_4)) ;
    float3 bu_0;
    if((dot(u0_1, u0_1)) > 9.99999997475242708e-07f)
    {
        bu_0 = normalize(u0_1);
    }
    else
    {
        bu_0 = _S113.ex_0;
    }
    float3 br_0 = normalize(cross(bu_0, vw_0));
    float pad_3 = (float4((&_S105->imp_0)->cfg_3) ).y;
    float half_t_0 = _S113.r_0 * pad_3;
    float half_w_0 = half_t_0 * _S113.s_4 * max(_S113.wd_1, 1.0f);
    float4 _S114 = float4((&_S110)->geo_0) ;
    float crown_max_0 = float((as_type<uint>(((float4((&_S110)->bark_0) ).w))) >> 21U) / 2047.0f * 1.5f * _S114.z;
    float half_x_0;
    if(crown_max_0 > 0.0f)
    {
        half_x_0 = min(half_w_0, crown_max_0 * 1.0199999809265137f * pad_3 * _S113.s_4 * _S113.wd_1);
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
    float _S115;
    if((corner_0 & 2U) != 0U)
    {
        _S115 = 1.0f;
    }
    else
    {
        _S115 = -1.0f;
    }
    float3 p_5 = _S113.c_1 + br_0 * float3((front_1 * half_x_0))  + bu_0 * float3((_S115 * half_w_0)) ;
    float3 _S116 = p_5 - _S113.c_1;
    thread ImpTree_0 _S117 = _S113;
    float3 _S118 = imp_to_tree_0(&_S117, _S116);
    float3 p_6;
    if(!shadow_2)
    {
        float _S119 = length(_S113.c_1);
        float _S120 = min(half_w_0, max(_S119 - (float4(_S105->params_1) ).z * 2.0f, 0.0f));
        float3 p_7 = p_5 - vw_0 * float3(_S120) ;
        float lp_0 = length(p_7);
        float3 p_8 = p_7 * float3((max(lp_0 - float(((uint4((&_S105->imp_0)->bufs_1) ).w) >> 8U) / 16.0f * _S119 / max((float4(_S105->proj_0) ).w * 0.5f * (float4(_S105->proj_0) ).y, 1.0f), min(lp_0, (float4(_S105->params_1) ).z * 2.0f)) / max(lp_0, 9.99999997475242708e-07f))) ;
        front_1 = _S120;
        p_6 = p_8;
    }
    else
    {
        front_1 = 0.0f;
        p_6 = p_5;
    }
    thread ImpOut_0 o_1;
    matrix<float,int(4),int(4)>  _S121;
    if(shadow_2)
    {
        _S121 = matrix<float,int(4),int(4)> ((&(&_S105->light_0)->shadow_0)->data_3[kernelContext_18->ed_c_0->cascade_0].data_1[int(0)][int(0)], (&(&_S105->light_0)->shadow_0)->data_3[kernelContext_18->ed_c_0->cascade_0].data_1[int(1)][int(0)], (&(&_S105->light_0)->shadow_0)->data_3[kernelContext_18->ed_c_0->cascade_0].data_1[int(2)][int(0)], (&(&_S105->light_0)->shadow_0)->data_3[kernelContext_18->ed_c_0->cascade_0].data_1[int(3)][int(0)], (&(&_S105->light_0)->shadow_0)->data_3[kernelContext_18->ed_c_0->cascade_0].data_1[int(0)][int(1)], (&(&_S105->light_0)->shadow_0)->data_3[kernelContext_18->ed_c_0->cascade_0].data_1[int(1)][int(1)], (&(&_S105->light_0)->shadow_0)->data_3[kernelContext_18->ed_c_0->cascade_0].data_1[int(2)][int(1)], (&(&_S105->light_0)->shadow_0)->data_3[kernelContext_18->ed_c_0->cascade_0].data_1[int(3)][int(1)], (&(&_S105->light_0)->shadow_0)->data_3[kernelContext_18->ed_c_0->cascade_0].data_1[int(0)][int(2)], (&(&_S105->light_0)->shadow_0)->data_3[kernelContext_18->ed_c_0->cascade_0].data_1[int(1)][int(2)], (&(&_S105->light_0)->shadow_0)->data_3[kernelContext_18->ed_c_0->cascade_0].data_1[int(2)][int(2)], (&(&_S105->light_0)->shadow_0)->data_3[kernelContext_18->ed_c_0->cascade_0].data_1[int(3)][int(2)], (&(&_S105->light_0)->shadow_0)->data_3[kernelContext_18->ed_c_0->cascade_0].data_1[int(0)][int(3)], (&(&_S105->light_0)->shadow_0)->data_3[kernelContext_18->ed_c_0->cascade_0].data_1[int(1)][int(3)], (&(&_S105->light_0)->shadow_0)->data_3[kernelContext_18->ed_c_0->cascade_0].data_1[int(2)][int(3)], (&(&_S105->light_0)->shadow_0)->data_3[kernelContext_18->ed_c_0->cascade_0].data_1[int(3)][int(3)]);
    }
    else
    {
        _S121 = matrix<float,int(4),int(4)> (_S105->view_proj_0.data_1[int(0)][int(0)], _S105->view_proj_0.data_1[int(1)][int(0)], _S105->view_proj_0.data_1[int(2)][int(0)], _S105->view_proj_0.data_1[int(3)][int(0)], _S105->view_proj_0.data_1[int(0)][int(1)], _S105->view_proj_0.data_1[int(1)][int(1)], _S105->view_proj_0.data_1[int(2)][int(1)], _S105->view_proj_0.data_1[int(3)][int(1)], _S105->view_proj_0.data_1[int(0)][int(2)], _S105->view_proj_0.data_1[int(1)][int(2)], _S105->view_proj_0.data_1[int(2)][int(2)], _S105->view_proj_0.data_1[int(3)][int(2)], _S105->view_proj_0.data_1[int(0)][int(3)], _S105->view_proj_0.data_1[int(1)][int(3)], _S105->view_proj_0.data_1[int(2)][int(3)], _S105->view_proj_0.data_1[int(3)][int(3)]);
    }
    float4 _S122 = float4(p_6, 1.0f);
    (&o_1)->pos_6 = (((_S122) * (_S121)));
    thread ImpTree_0 _S123 = _S113;
    float2 _S124 = imp_frame_uv_0(&_S123, _S113.d0_0, _S118, half_t_0);
    (&o_1)->uv0_0 = _S124;
    thread ImpTree_0 _S125 = _S113;
    float2 _S126 = imp_frame_uv_0(&_S125, _S113.d1_0, _S118, half_t_0);
    (&o_1)->uv1_0 = _S126;
    thread ImpTree_0 _S127 = _S113;
    float2 _S128 = imp_frame_uv_0(&_S127, _S113.d2_0, _S118, half_t_0);
    (&o_1)->uv2_0 = _S128;
    if(shadow_2)
    {
        half_x_0 = 0.0f;
    }
    else
    {
        half_x_0 = - (((_S122) * (matrix<float,int(4),int(4)> (_S105->view_0.data_1[int(0)][int(0)], _S105->view_0.data_1[int(1)][int(0)], _S105->view_0.data_1[int(2)][int(0)], _S105->view_0.data_1[int(3)][int(0)], _S105->view_0.data_1[int(0)][int(1)], _S105->view_0.data_1[int(1)][int(1)], _S105->view_0.data_1[int(2)][int(1)], _S105->view_0.data_1[int(3)][int(1)], _S105->view_0.data_1[int(0)][int(2)], _S105->view_0.data_1[int(1)][int(2)], _S105->view_0.data_1[int(2)][int(2)], _S105->view_0.data_1[int(3)][int(2)], _S105->view_0.data_1[int(0)][int(3)], _S105->view_0.data_1[int(1)][int(3)], _S105->view_0.data_1[int(2)][int(3)], _S105->view_0.data_1[int(3)][int(3)])))).z;
    }
    (&o_1)->zv_0 = half_x_0;
    (&o_1)->entry_8 = entry_9;
    uint4 _S129 = uint4((&_S110)->tex_0) ;
    (&o_1)->fi_1 = uint4(_S113.fi_0, _S129.x);
    (&o_1)->w_4 = float4(_S113.w_3, 2.0f * half_t_0 * _S113.s_4);
    (&o_1)->front_0 = front_1;
    (&o_1)->geo_1 = float2(float(_S129.w), _S114.w);
    (&o_1)->lv_0 = 1.0f;
    if(((float4((&_S105->season_1)->cfg_4) ).x) > 0.0f)
    {
        LeafSeason_0 _S130 = season_of_word_0(_S105, g_2.season_0);
        (&o_1)->lv_0 = mix(0.2199999988079071f, 1.0f, _S130.leaves_0);
    }
    return o_1;
}

struct vertexOutput_0
{
    float4 output_4 [[position]];
};

[[vertex]] vertexOutput_0 vs_imp_shadow_proxy(uint vid_2 [[vertex_id]], uint iid_2 [[instance_id]], VisC_0 constant* ed_c_6 [[buffer(0)]], ulong device* ed_buffers_6 [[buffer(2)]], ulong device* ed_textures_6 [[buffer(1)]], ulong device* ed_samplers_6 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_19;
    (&kernelContext_19)->ed_c_0 = ed_c_6;
    (&kernelContext_19)->ed_buffers_0 = ed_buffers_6;
    (&kernelContext_19)->ed_textures_0 = ed_textures_6;
    (&kernelContext_19)->ed_samplers_0 = ed_samplers_6;
    FrameData_natural_0 device* _S131 = ed_buf_10(ed_c_6->frame_0, &kernelContext_19);
    uint device* _S132 = ed_buf_9((&kernelContext_19)->ed_c_0->visible_0, &kernelContext_19);
    uint entry_10 = *(_S132 + iid_2);
    GpuTree_0 device* _S133 = ed_buf_3((&kernelContext_19)->ed_c_0->instances_0, &kernelContext_19);
    GpuTree_0 g_3 = *(_S133 + (entry_10 & 16777215U));
    GpuImpKind_natural_0 device* _S134 = ed_buf_2((&kernelContext_19)->ed_c_0->verts_0, &kernelContext_19);
    GpuImpKind_natural_0 k_6 = *(_S134 + (((g_3.node_kind_1) >> 12U) & 4095U));
    packed_float4 device* _S135 = ed_buf_6((&kernelContext_19)->ed_c_0->nodes_0, &kernelContext_19);
    float3 node_4 = (float4(*(_S135 + ((g_3.node_kind_1) & 4095U))) ).xyz;
    float3 l_2 = (float4(_S131->sun_dir_0) ).xyz;
    float3 up_6 = oct_decode_0(g_3.up_1);
    float h_1 = (float4(k_6.geo_0) ).z;
    uint cw_0 = (as_type<uint>(((float4(k_6.bark_0) ).w)));
    float cb_0 = float(cw_0 & 1023U) / 1023.0f * h_1 * g_3.scale_0;
    float cr_0 = float((cw_0 >> 10U) & 2047U) / 2047.0f * 1.5f * h_1 * g_3.scale_0 * (0.5f + float((g_3.yaw_widen_0) >> 16U) / 65535.0f);
    float _S136 = dot(l_2, up_6);
    float sin_el_0 = abs(_S136);
    float3 u0_2 = up_6 - l_2 * float3(_S136) ;
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
    vertexOutput_0 _S137 = { (((float4(node_4 + float3(g_3.px_1, g_3.py_1, g_3.pz_1) + bu_1 * float3((ym_0 + (q_2.y - ym_0) * k_s_0))  + br_1 * float3((q_2.x * k_s_0)) , 1.0f)) * (matrix<float,int(4),int(4)> ((&(&_S131->light_0)->shadow_0)->data_3[(&kernelContext_19)->ed_c_0->cascade_0].data_1[int(0)][int(0)], (&(&_S131->light_0)->shadow_0)->data_3[(&kernelContext_19)->ed_c_0->cascade_0].data_1[int(1)][int(0)], (&(&_S131->light_0)->shadow_0)->data_3[(&kernelContext_19)->ed_c_0->cascade_0].data_1[int(2)][int(0)], (&(&_S131->light_0)->shadow_0)->data_3[(&kernelContext_19)->ed_c_0->cascade_0].data_1[int(3)][int(0)], (&(&_S131->light_0)->shadow_0)->data_3[(&kernelContext_19)->ed_c_0->cascade_0].data_1[int(0)][int(1)], (&(&_S131->light_0)->shadow_0)->data_3[(&kernelContext_19)->ed_c_0->cascade_0].data_1[int(1)][int(1)], (&(&_S131->light_0)->shadow_0)->data_3[(&kernelContext_19)->ed_c_0->cascade_0].data_1[int(2)][int(1)], (&(&_S131->light_0)->shadow_0)->data_3[(&kernelContext_19)->ed_c_0->cascade_0].data_1[int(3)][int(1)], (&(&_S131->light_0)->shadow_0)->data_3[(&kernelContext_19)->ed_c_0->cascade_0].data_1[int(0)][int(2)], (&(&_S131->light_0)->shadow_0)->data_3[(&kernelContext_19)->ed_c_0->cascade_0].data_1[int(1)][int(2)], (&(&_S131->light_0)->shadow_0)->data_3[(&kernelContext_19)->ed_c_0->cascade_0].data_1[int(2)][int(2)], (&(&_S131->light_0)->shadow_0)->data_3[(&kernelContext_19)->ed_c_0->cascade_0].data_1[int(3)][int(2)], (&(&_S131->light_0)->shadow_0)->data_3[(&kernelContext_19)->ed_c_0->cascade_0].data_1[int(0)][int(3)], (&(&_S131->light_0)->shadow_0)->data_3[(&kernelContext_19)->ed_c_0->cascade_0].data_1[int(1)][int(3)], (&(&_S131->light_0)->shadow_0)->data_3[(&kernelContext_19)->ed_c_0->cascade_0].data_1[int(2)][int(3)], (&(&_S131->light_0)->shadow_0)->data_3[(&kernelContext_19)->ed_c_0->cascade_0].data_1[int(3)][int(3)])))) };
    return _S137;
}

float2 imp_atlas_uv_0(uint fi_2, uint n_5, float2 uv_6, float frame_px_0)
{
    float m_1 = 0.5f / frame_px_0;
    float _S138 = 1.0f - m_1;
    float2 l_3 = clamp(uv_6, float2(m_1, m_1), float2(_S138, _S138));
    uint _S139 = fi_2 % n_5;
    float _S140 = float(_S139);
    uint _S141 = fi_2 / n_5;
    return (float2(_S140, float(_S141)) + l_3) / float2(float(n_5)) ;
}

bool imp_inside_0(float2 uv_7)
{
    bool _S142;
    if(all(uv_7 >= (float2(0.0f) )))
    {
        _S142 = all(uv_7 <= (float2(1.0f) ));
    }
    else
    {
        _S142 = false;
    }
    return _S142;
}

float imp_coverage_0(const ImpOut_0 thread* i_15, KernelContext_0 thread* kernelContext_20)
{
    uint4 _S143 = i_15->fi_1;
    texture2d<float, access::sample> _S144 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_20->ed_textures_0))[(i_15->fi_1.w)]));
    sampler _S145 = ((reinterpret_cast<device sampler*>((kernelContext_20->ed_samplers_0))[(kernelContext_20->ed_c_0->smp_0)]));
    uint n_6 = uint(i_15->geo_1.x);
    float _S146 = i_15->geo_1.y;
    float2 t0_0 = imp_atlas_uv_0(_S143.x, n_6, i_15->uv0_0, _S146);
    float2 _S147 = i_15->uv1_0;
    float2 t1_0 = imp_atlas_uv_0(_S143.y, n_6, i_15->uv1_0, _S146);
    float2 _S148 = i_15->uv2_0;
    float2 t2_0 = imp_atlas_uv_0(_S143.z, n_6, i_15->uv2_0, _S146);
    float2 _S149 = ((_S144).sample((_S145), (t0_0))).xy;
    float _S150;
    if(imp_inside_0(i_15->uv0_0))
    {
        _S150 = 1.0f;
    }
    else
    {
        _S150 = 0.0f;
    }
    float2 s0_0 = _S149 * float2(_S150) ;
    float2 _S151 = ((_S144).sample((_S145), (t1_0))).xy;
    if(imp_inside_0(_S147))
    {
        _S150 = 1.0f;
    }
    else
    {
        _S150 = 0.0f;
    }
    float2 s1_0 = _S151 * float2(_S150) ;
    float2 _S152 = ((_S144).sample((_S145), (t2_0))).xy;
    if(imp_inside_0(_S148))
    {
        _S150 = 1.0f;
    }
    else
    {
        _S150 = 0.0f;
    }
    float2 s2_0 = _S152 * float2(_S150) ;
    float4 _S153 = i_15->w_4;
    float _S154 = i_15->w_4.x * s0_0.x;
    float _S155 = i_15->w_4.y * s1_0.x;
    float _S156 = i_15->w_4.z * s2_0.x;
    float cov_0 = _S154 + _S155 + _S156;
    uint2 p_9 = uint2(i_15->pos_6.xy) & (uint2(7U) );
    if((saturate((cov_0 - 0.5f) * 2.0f + 0.5f) * float((i_15->entry_8) >> 24U) / 255.0f * i_15->lv_0) <= ((float((BAYER8_0[p_9.y * 8U + p_9.x] + ((((i_15->entry_8) & 16777215U) * 2654435761U) >> 26U)) & 63U) + 0.5f) / 64.0f))
    {
        discard_fragment();
    }
    float d_7 = _S154 * s0_0.y + _S155 * s1_0.y + _S156 * s2_0.y;
    if(cov_0 > 9.9999997473787516e-05f)
    {
        _S150 = (d_7 / cov_0 - 0.5f) * _S153.w;
    }
    else
    {
        _S150 = 0.0f;
    }
    return _S150;
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
    [[flat]] float lv_1 [[user(LEAVES)]];
};

[[fragment]] ImpDepthOut_0 fs_imp(pixelInput_5 _S157 [[stage_in]], float4 pos_7 [[position]], VisC_0 constant* ed_c_7 [[buffer(0)]], ulong device* ed_buffers_7 [[buffer(2)]], ulong device* ed_textures_7 [[buffer(1)]], ulong device* ed_samplers_7 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_21;
    (&kernelContext_21)->ed_c_0 = ed_c_7;
    (&kernelContext_21)->ed_buffers_0 = ed_buffers_7;
    (&kernelContext_21)->ed_textures_0 = ed_textures_7;
    (&kernelContext_21)->ed_samplers_0 = ed_samplers_7;
    FrameData_natural_0 device* _S158 = ed_buf_10(ed_c_7->frame_0, &kernelContext_21);
    thread ImpOut_0 _S159;
    (&_S159)->pos_6 = pos_7;
    (&_S159)->uv0_0 = _S157.uv0_1;
    (&_S159)->uv1_0 = _S157.uv1_1;
    (&_S159)->uv2_0 = _S157.uv2_1;
    (&_S159)->zv_0 = _S157.zv_1;
    (&_S159)->entry_8 = _S157.entry_11;
    (&_S159)->fi_1 = _S157.fi_3;
    (&_S159)->w_4 = _S157.w_5;
    (&_S159)->geo_1 = _S157.geo_2;
    (&_S159)->front_0 = _S157.front_2;
    (&_S159)->lv_0 = _S157.lv_1;
    float _S160 = imp_coverage_0(&_S159, &kernelContext_21);
    float near_0 = (float4(_S158->params_1) ).z;
    thread ImpDepthOut_0 o_2;
    (&o_2)->vis_0 = uint4(1073741824U | ((_S157.entry_11) & 16777215U), 0U, 0U, 0U);
    (&o_2)->depth_0 = min(near_0 / max(_S157.zv_1 + max(_S157.front_2 - _S160, 0.0f), near_0), pos_7.z);
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
    [[flat]] float lv_2 [[user(LEAVES)]];
};

[[fragment]] void fs_imp_shadow(pixelInput_6 _S161 [[stage_in]], float4 pos_8 [[position]], VisC_0 constant* ed_c_8 [[buffer(0)]], ulong device* ed_buffers_8 [[buffer(2)]], ulong device* ed_textures_8 [[buffer(1)]], ulong device* ed_samplers_8 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_22;
    (&kernelContext_22)->ed_c_0 = ed_c_8;
    (&kernelContext_22)->ed_buffers_0 = ed_buffers_8;
    (&kernelContext_22)->ed_textures_0 = ed_textures_8;
    (&kernelContext_22)->ed_samplers_0 = ed_samplers_8;
    texture2d<float, access::sample> _S162 = ((reinterpret_cast<device texture2d<float, access::sample>*>((ed_textures_8))[(_S161.fi_4.w)]));
    sampler _S163 = ((reinterpret_cast<device sampler*>((ed_samplers_8))[(ed_c_8->smp_0)]));
    uint n_7 = uint(_S161.geo_3.x);
    uint _S164 = _S161.fi_4.x;
    float _S165 = _S161.w_6.y;
    float _S166 = _S161.w_6.x;
    bool _S167;
    if(_S165 > _S166)
    {
        _S167 = _S165 >= (_S161.w_6.z);
    }
    else
    {
        _S167 = false;
    }
    float2 uv_8;
    uint fr_1;
    if(_S167)
    {
        uint _S168 = _S161.fi_4.y;
        uv_8 = _S161.uv1_2;
        fr_1 = _S168;
    }
    else
    {
        float _S169 = _S161.w_6.z;
        if(_S169 > _S166)
        {
            _S167 = _S169 > _S165;
        }
        else
        {
            _S167 = false;
        }
        if(_S167)
        {
            uint _S170 = _S161.fi_4.z;
            uv_8 = _S161.uv2_2;
            fr_1 = _S170;
        }
        else
        {
            uv_8 = _S161.uv0_2;
            fr_1 = _S164;
        }
    }
    float cov_1;
    if(imp_inside_0(uv_8))
    {
        float2 _S171 = imp_atlas_uv_0(fr_1, n_7, uv_8, _S161.geo_3.y);
        cov_1 = ((_S162).sample((_S163), (_S171))).x;
    }
    else
    {
        cov_1 = 0.0f;
    }
    uint2 p_10 = uint2(pos_8.xy) & (uint2(7U) );
    if((saturate((cov_1 - 0.5f) * 2.0f + 0.5f) * float((_S161.entry_12) >> 24U) / 255.0f * _S161.lv_2) <= ((float((BAYER8_0[p_10.y * 8U + p_10.x] + ((((_S161.entry_12) & 16777215U) * 2654435761U) >> 26U)) & 63U) + 0.5f) / 64.0f))
    {
        discard_fragment();
    }
    return;
}

float2 grass_lod_0(FrameData_natural_0 device* f_3, float d_8, float blades_0)
{
    float q_3 = (float4((&f_3->grass_0)->lod_0) ).x / max(d_8, 0.0010000000474974513f);
    float raw_0 = blades_0 * min(q_3 * q_3, 1.0f);
    float _S172 = min((float4((&f_3->grass_0)->lod_0) ).z, blades_0);
    return float2(max(raw_0, _S172), min(raw_0 / max(_S172, 0.0010000000474974513f), 1.0f));
}

uint grass_hash_0(uint x_2)
{
    uint s_5 = x_2 * 747796405U + 2891336453U;
    uint w_7 = ((s_5 >> ((s_5 >> 28U) + 4U)) ^ s_5) * 277803737U;
    return (w_7 >> 22U) ^ w_7;
}

float grass_rand_0(uint h_2)
{
    return float(h_2 >> 8U) * 5.9604644775390625e-08f;
}

struct GrassBlade_0
{
    float3 p0_0;
    float3 p1_0;
    float3 p2_0;
    float3 side_0;
    float3 up_7;
    float w_8;
    float h_3;
    float head_0;
    float head_w_0;
    float shade_1;
    float t_ao_0;
};

GrassBlade_0 grass_blade_0(FrameData_natural_0 device* f_4, const GrassClump_0 thread* c_2, const GrassType_natural_0 thread* ty_0, float3 node_5, uint k_7, float lod_mul_0)
{
    float3 up_8 = oct_decode_0(c_2->up_0);
    float3 rt_0 = normalize(cross(up_8, oct_decode_0(c_2->facing_0)));
    float3 fa_0 = cross(rt_0, up_8);
    float3 base_1 = node_5 + float3(c_2->px_0, c_2->py_0, c_2->pz_0);
    float d_9 = length(base_1);
    float4 _S173 = float4(ty_0->a_0) ;
    float blades_1 = _S173.w;
    float2 _S174 = grass_lod_0(f_4, d_9, blades_1);
    float _S175 = max(_S174.x * lod_mul_0, min((float4((&f_4->grass_0)->lod_0) ).z, blades_1));
    float _S176 = _S174.y;
    float _S177 = min(pow(blades_1 / _S175, 0.80000001192092896f) * rsqrt(max(_S176, 0.20000000298023224f)), (float4((&f_4->grass_0)->lod_0) ).w);
    float keep_0 = saturate((_S176 - float((c_2->node_kind_0) >> 24U) / 255.0f) / (0.30000001192092896f * _S176 + 0.0010000000474974513f));
    float range_0 = (float4((&f_4->grass_0)->lod_0) ).y;
    float fade_1 = 1.0f - smoothstep(0.60000002384185791f * range_0, range_0, d_9);
    uint _S178 = c_2->seed_0;
    uint s0_1 = grass_hash_0((c_2->seed_0) ^ (k_7 * 2654435769U + 2135587861U));
    uint s1_1 = grass_hash_0(s0_1);
    uint s2_1 = grass_hash_0(s1_1);
    uint s3_0 = grass_hash_0(s2_1);
    uint s4_0 = grass_hash_0(s3_0);
    uint s5_0 = grass_hash_0(s4_0);
    float ang_0 = grass_rand_0(s0_1) * 6.2831854820251465f;
    float rad_0 = sqrt(grass_rand_0(s1_1));
    float4 _S179 = float4(ty_0->b_0) ;
    float3 off_1 = (fa_0 * float3(cos(ang_0))  + rt_0 * float3((sin(ang_0) * (float4(ty_0->root_0) ).w)) ) * float3((rad_0 * _S179.x)) ;
    thread GrassBlade_0 o_3;
    (&o_3)->up_7 = up_8;
    uint _S180 = (c_2->hw_0) & 65535U;
    float hc_0 = mix(_S173.x, _S173.y, grass_rand_0(grass_hash_0(c_2->seed_0 + 17U))) * (float(_S180) / 65535.0f * 4.0f);
    (&o_3)->p0_0 = base_1 + off_1 - up_8 * float3(min(0.029999999329447746f, 0.10000000149011612f * hc_0)) ;
    float h_4 = hc_0 * (1.0f - 0.34999999403953552f * rad_0) * mix(0.72000002861022949f, 1.1200000047683716f, grass_rand_0(s2_1));
    float grow_0 = saturate(_S175 - float(k_7)) * keep_0 * fade_1;
    bool _S181;
    if(k_7 < uint(blades_1))
    {
        _S181 = _S180 != 0U;
    }
    else
    {
        _S181 = false;
    }
    float _S182;
    if(_S181)
    {
        _S182 = h_4 * grow_0;
    }
    else
    {
        _S182 = 0.0f;
    }
    (&o_3)->h_3 = _S182;
    float ca_0 = grass_rand_0(grass_hash_0(_S178 + 29U)) * 6.2831854820251465f;
    float3 cdir_0 = fa_0 * float3(cos(ca_0))  + rt_0 * float3(sin(ca_0)) ;
    float3 radial_0;
    if(rad_0 > 0.0010000000474974513f)
    {
        radial_0 = normalize(off_1);
    }
    else
    {
        radial_0 = fa_0;
    }
    float ba_0 = grass_rand_0(s3_0) * 6.2831854820251465f;
    float3 ld_0 = cdir_0 * float3(0.89999997615814209f)  + radial_0 * float3(0.80000001192092896f)  + (fa_0 * float3(cos(ba_0))  + rt_0 * float3(sin(ba_0)) ) * float3(0.60000002384185791f) ;
    float3 ld_1;
    if((dot(ld_0, ld_0)) > 9.99999997475242708e-07f)
    {
        ld_1 = normalize(ld_0);
    }
    else
    {
        ld_1 = fa_0;
    }
    float lean_0 = _S179.y * mix(0.30000001192092896f, 1.0f, grass_rand_0(s4_0));
    float3 tipv_0 = up_8 * float3(cos(lean_0))  + ld_1 * float3(sin(lean_0)) ;
    WindData_natural_0 wn_0 = f_4->wind_1;
    if((((uint4((&f_4->grass_0)->bufs_2) ).w) & 4U) != 0U)
    {
        _S181 = ((float4(wn_0.cfg_2) ).x) > 0.5f;
    }
    else
    {
        _S181 = false;
    }
    float3 tipv_1;
    if(_S181)
    {
        float4 _S183 = float4(wn_0.wind_0) ;
        float2 _S184 = _S183.xz;
        float speed_1 = length(_S184);
        float2 wd_2;
        if(speed_1 > 0.0010000000474974513f)
        {
            wd_2 = _S184 / float2(speed_1) ;
        }
        else
        {
            wd_2 = float2(0.0f, 0.0f);
        }
        float4 _S185 = float4(wn_0.gust_0) ;
        float2 _S186 = base_1.xz;
        float _S187 = dot(_S186, wd_2);
        float g_4 = 0.5f + 0.5f * sin((_S185.x + _S187 / max(_S185.y, 1.0f)) * 6.2831854820251465f);
        float tw_0 = _S183.w;
        float _S188 = wd_2.y;
        float _S189 = wd_2.x;
        float _S190 = grass_rand_0(s5_0);
        float _S191 = 1.0f - _S179.z;
        float3 wv_0 = float3(_S189, 0.0f, _S188);
        tipv_1 = normalize(tipv_0 + (wv_0 - up_8 * float3(dot(wv_0, up_8)) ) * float3(clamp(speed_1 * 0.059999998658895493f * _S191 * (0.34999999403953552f + 0.89999997615814209f * (g_4 * g_4 * g_4 * _S185.z) + 0.69999998807907104f * (0.5f + 0.5f * sin(_S187 * 0.89999997615814209f - tw_0 * (1.2000000476837158f + 0.25f * speed_1) + 0.60000002384185791f * sin(dot(_S186, float2(- _S188, _S189)) * 0.34999999403953552f)))) + 0.039999999105930328f * _S191 * sin(tw_0 * (5.0f + 3.0f * _S190) + _S190 * 40.0f) * min(speed_1 * 0.20000000298023224f, 1.0f), -0.30000001192092896f, 1.3999999761581421f)) );
    }
    else
    {
        tipv_1 = tipv_0;
    }
    float3 _S192 = (&o_3)->p0_0 + tipv_1 * float3(((&o_3)->h_3 * (1.0f - 0.18000000715255737f * saturate(1.0f - dot(tipv_1, up_8))))) ;
    (&o_3)->p2_0 = _S192;
    float3 across_0 = _S192 - (&o_3)->p0_0 - up_8 * float3(dot(_S192 - (&o_3)->p0_0, up_8)) ;
    float curve_0 = _S179.w;
    (&o_3)->p1_0 = (&o_3)->p0_0 + up_8 * float3((max(dot(_S192 - (&o_3)->p0_0, up_8), 0.25f * (&o_3)->h_3) * mix(0.60000002384185791f, 1.0499999523162842f, curve_0)))  + across_0 * float3(mix(0.5f, 0.05000000074505806f, curve_0)) ;
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
    (&o_3)->side_0 = normalize(cross(up_8, lh_0) * float3(cos(tw2_0))  + lh_0 * float3(sin(tw2_0)) );
    (&o_3)->w_8 = _S173.z * _S177 * mix(0.80000001192092896f, 1.2000000476837158f, grass_rand_0(s1_1 ^ 85U));
    (&o_3)->head_0 = (float4(ty_0->mid_0) ).w;
    (&o_3)->head_w_0 = (float4(ty_0->tip_0) ).w;
    (&o_3)->shade_1 = mix(0.81999999284744263f, 1.1499999761581421f, grass_rand_0(s4_0 ^ 51U));
    (&o_3)->t_ao_0 = saturate(hc_0 / 0.80000001192092896f);
    return o_3;
}

float3 grass_point_0(const GrassBlade_0 thread* b_1, float t_6)
{
    float s_6 = 1.0f - t_6;
    return b_1->p0_0 * float3((s_6 * s_6))  + b_1->p1_0 * float3((2.0f * s_6 * t_6))  + b_1->p2_0 * float3((t_6 * t_6)) ;
}

float3 grass_tangent_0(const GrassBlade_0 thread* b_2, float t_7)
{
    return (b_2->p1_0 - b_2->p0_0) * float3((2.0f * (1.0f - t_7)))  + (b_2->p2_0 - b_2->p1_0) * float3((2.0f * t_7)) ;
}

float3 grass_view_side_0(const GrassBlade_0 thread* b_3, float t_8, float3 p_11)
{
    float3 _S193 = grass_tangent_0(b_3, t_8);
    float3 vs_0 = cross(normalize(_S193 + b_3->up_7 * float3(9.9999997473787516e-05f) ), normalize(p_11));
    if((dot(vs_0, vs_0)) < 9.99999993922529029e-09f)
    {
        return b_3->side_0;
    }
    float3 vs_1 = normalize(vs_0);
    float3 _S194 = b_3->side_0;
    float3 vs_2;
    if((dot(vs_1, b_3->side_0)) < 0.0f)
    {
        vs_2 = - vs_1;
    }
    else
    {
        vs_2 = vs_1;
    }
    return normalize(mix(_S194, vs_2, float3(0.55000001192092896f) ));
}

float grass_width_0(const GrassBlade_0 thread* b_4, float t_9)
{
    float _S195 = b_4->w_8;
    float w_9 = b_4->w_8 * (1.0f - t_9 * t_9 * t_9) * (1.0f - 0.20000000298023224f * t_9);
    float _S196 = b_4->head_0;
    float w_10;
    if((b_4->head_0) > 0.0f)
    {
        float _S197 = 1.0f - _S196;
        w_10 = mix(w_9, _S195 * b_4->head_w_0 * (1.0f - 0.25f * t_9), smoothstep(_S197 - 0.039999999105930328f, _S197, t_9) * (1.0f - smoothstep(0.93000000715255737f, 1.0f, t_9)));
    }
    else
    {
        w_10 = w_9;
    }
    return w_10;
}

struct GrassOut_0
{
    float4 pos_9;
    float2 tu_0;
    [[flat]] uint clump_0;
    [[flat]] uint blade_0;
};

GrassOut_0 grass_vertex_0(uint vid_3, uint iid_3, bool shadow_3, KernelContext_0 thread* kernelContext_23)
{
    FrameData_natural_0 device* _S198 = ed_buf_10(kernelContext_23->ed_c_0->frame_0, kernelContext_23);
    uint device* _S199 = ed_buf_9(kernelContext_23->ed_c_0->visible_0, kernelContext_23);
    uint ci_0 = *(_S199 + iid_3);
    GrassClump_0 device* _S200 = ed_buf_1(kernelContext_23->ed_c_0->instances_0, kernelContext_23);
    GrassClump_0 c_3 = *(_S200 + ci_0);
    GrassType_natural_0 device* _S201 = ed_buf_0(kernelContext_23->ed_c_0->verts_0, kernelContext_23);
    thread GrassType_natural_0 _S202 = *(_S201 + (((c_3.node_kind_0) >> 12U) & 255U));
    packed_float4 device* _S203 = ed_buf_6(kernelContext_23->ed_c_0->nodes_0, kernelContext_23);
    float3 node_6 = (float4(*(_S203 + ((c_3.node_kind_0) & 4095U))) ).xyz;
    uint _S204 = min(kernelContext_23->ed_c_0->cascade_0, 4U);
    uint vpb_0 = 2U * (GRASS_SEGS_0[_S204] + 1U);
    uint k_8 = vid_3 / vpb_0;
    uint r_2 = vid_3 - k_8 * vpb_0;
    uint seg_0 = r_2 >> 1U;
    float side_1 = float(r_2 & 1U);
    float _S205;
    if(shadow_3)
    {
        _S205 = 0.5f;
    }
    else
    {
        _S205 = 1.0f;
    }
    thread GrassClump_0 _S206 = c_3;
    GrassBlade_0 _S207 = grass_blade_0(_S198, &_S206, &_S202, node_6, k_8, _S205);
    float t_10 = float(seg_0) / float(GRASS_SEGS_0[_S204]);
    thread GrassBlade_0 _S208 = _S207;
    float3 _S209 = grass_point_0(&_S208, t_10);
    float3 sd_4;
    if(shadow_3)
    {
        sd_4 = _S207.side_0;
    }
    else
    {
        thread GrassBlade_0 _S210 = _S207;
        float3 _S211 = grass_view_side_0(&_S210, t_10, _S209);
        sd_4 = _S211;
    }
    thread GrassBlade_0 _S212 = _S207;
    float _S213 = grass_width_0(&_S212, t_10);
    float3 p_12 = _S209 + sd_4 * float3(((side_1 - 0.5f) * _S213)) ;
    thread GrassOut_0 o_4;
    float4 _S214;
    if((_S207.h_3) > 0.0f)
    {
        matrix<float,int(4),int(4)>  _S215;
        if(shadow_3)
        {
            _S215 = matrix<float,int(4),int(4)> ((&(&_S198->light_0)->shadow_0)->data_3[int(0)].data_1[int(0)][int(0)], (&(&_S198->light_0)->shadow_0)->data_3[int(0)].data_1[int(1)][int(0)], (&(&_S198->light_0)->shadow_0)->data_3[int(0)].data_1[int(2)][int(0)], (&(&_S198->light_0)->shadow_0)->data_3[int(0)].data_1[int(3)][int(0)], (&(&_S198->light_0)->shadow_0)->data_3[int(0)].data_1[int(0)][int(1)], (&(&_S198->light_0)->shadow_0)->data_3[int(0)].data_1[int(1)][int(1)], (&(&_S198->light_0)->shadow_0)->data_3[int(0)].data_1[int(2)][int(1)], (&(&_S198->light_0)->shadow_0)->data_3[int(0)].data_1[int(3)][int(1)], (&(&_S198->light_0)->shadow_0)->data_3[int(0)].data_1[int(0)][int(2)], (&(&_S198->light_0)->shadow_0)->data_3[int(0)].data_1[int(1)][int(2)], (&(&_S198->light_0)->shadow_0)->data_3[int(0)].data_1[int(2)][int(2)], (&(&_S198->light_0)->shadow_0)->data_3[int(0)].data_1[int(3)][int(2)], (&(&_S198->light_0)->shadow_0)->data_3[int(0)].data_1[int(0)][int(3)], (&(&_S198->light_0)->shadow_0)->data_3[int(0)].data_1[int(1)][int(3)], (&(&_S198->light_0)->shadow_0)->data_3[int(0)].data_1[int(2)][int(3)], (&(&_S198->light_0)->shadow_0)->data_3[int(0)].data_1[int(3)][int(3)]);
        }
        else
        {
            _S215 = matrix<float,int(4),int(4)> (_S198->view_proj_0.data_1[int(0)][int(0)], _S198->view_proj_0.data_1[int(1)][int(0)], _S198->view_proj_0.data_1[int(2)][int(0)], _S198->view_proj_0.data_1[int(3)][int(0)], _S198->view_proj_0.data_1[int(0)][int(1)], _S198->view_proj_0.data_1[int(1)][int(1)], _S198->view_proj_0.data_1[int(2)][int(1)], _S198->view_proj_0.data_1[int(3)][int(1)], _S198->view_proj_0.data_1[int(0)][int(2)], _S198->view_proj_0.data_1[int(1)][int(2)], _S198->view_proj_0.data_1[int(2)][int(2)], _S198->view_proj_0.data_1[int(3)][int(2)], _S198->view_proj_0.data_1[int(0)][int(3)], _S198->view_proj_0.data_1[int(1)][int(3)], _S198->view_proj_0.data_1[int(2)][int(3)], _S198->view_proj_0.data_1[int(3)][int(3)]);
        }
        _S214 = (((float4(p_12, 1.0f)) * (_S215)));
    }
    else
    {
        _S214 = float4(0.0f, 0.0f, -1.0f, 1.0f);
    }
    (&o_4)->pos_9 = _S214;
    (&o_4)->tu_0 = float2(t_10, side_1);
    (&o_4)->clump_0 = ci_0;
    (&o_4)->blade_0 = k_8;
    return o_4;
}

struct vertexOutput_1
{
    float4 output_5 [[position]];
};

[[vertex]] vertexOutput_1 vs_grass_shadow(uint vid_4 [[vertex_id]], uint iid_4 [[instance_id]], VisC_0 constant* ed_c_9 [[buffer(0)]], ulong device* ed_buffers_9 [[buffer(2)]], ulong device* ed_textures_9 [[buffer(1)]], ulong device* ed_samplers_9 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_24;
    (&kernelContext_24)->ed_c_0 = ed_c_9;
    (&kernelContext_24)->ed_buffers_0 = ed_buffers_9;
    (&kernelContext_24)->ed_textures_0 = ed_textures_9;
    (&kernelContext_24)->ed_samplers_0 = ed_samplers_9;
    GrassOut_0 _S216 = grass_vertex_0(vid_4, iid_4, true, &kernelContext_24);
    vertexOutput_1 _S217 = { _S216.pos_9 };
    return _S217;
}

struct pixelOutput_4
{
    uint4 output_6 [[color(0)]];
};

struct pixelInput_7
{
    float2 tu_1 [[user(TEXCOORD)]];
    [[flat]] uint clump_1 [[user(ENTRY)]];
    [[flat]] uint blade_1 [[user(BLADE)]];
};

[[fragment]] pixelOutput_4 fs_grass(pixelInput_7 _S218 [[stage_in]], float4 pos_10 [[position]], VisC_0 constant* ed_c_10 [[buffer(0)]], ulong device* ed_buffers_10 [[buffer(2)]], ulong device* ed_textures_10 [[buffer(1)]], ulong device* ed_samplers_10 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_25;
    (&kernelContext_25)->ed_c_0 = ed_c_10;
    (&kernelContext_25)->ed_buffers_0 = ed_buffers_10;
    (&kernelContext_25)->ed_textures_0 = ed_textures_10;
    (&kernelContext_25)->ed_samplers_0 = ed_samplers_10;
    pixelOutput_4 _S219 = { uint4(536870912U | ((_S218.clump_1) & 536870911U), (((_S218.blade_1) & 127U) | (uint(saturate(_S218.tu_1.x) * 4095.0f + 0.5f) << 7U)) | (uint(saturate(_S218.tu_1.y) * 255.0f + 0.5f) << 19U), 0U, 0U) };
    return _S219;
}

struct vs_vis_Result_0
{
    float4 pos_11 [[position]];
    uint entry_13 [[user(ENTRY)]];
};

struct VisOut_0
{
    float4 pos_12;
    [[flat]] uint entry_14;
};

[[vertex]] vs_vis_Result_0 vs_vis(uint vid_5 [[vertex_id]], uint iid_5 [[instance_id]], VisC_0 constant* ed_c_11 [[buffer(0)]], ulong device* ed_buffers_11 [[buffer(2)]], ulong device* ed_textures_11 [[buffer(1)]], ulong device* ed_samplers_11 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_26;
    (&kernelContext_26)->ed_c_0 = ed_c_11;
    (&kernelContext_26)->ed_buffers_0 = ed_buffers_11;
    (&kernelContext_26)->ed_textures_0 = ed_textures_11;
    (&kernelContext_26)->ed_samplers_0 = ed_samplers_11;
    FrameData_natural_0 device* _S220 = ed_buf_10(ed_c_11->frame_0, &kernelContext_26);
    uint device* _S221 = ed_buf_9((&kernelContext_26)->ed_c_0->visible_0, &kernelContext_26);
    uint entry_15 = *(_S221 + iid_5);
    GpuInstance_natural_0 device* _S222 = ed_buf_8((&kernelContext_26)->ed_c_0->instances_0, &kernelContext_26);
    GpuInstance_natural_0 device* _S223 = _S222 + (entry_15 & 1048575U);
    GpuInstance_natural_0 inst_3 = *_S223;
    thread GpuInstance_natural_0 _S224 = *_S223;
    Vtx_0 device* _S225 = ed_buf_7((&kernelContext_26)->ed_c_0->verts_0, &kernelContext_26);
    Vtx_0 v_5 = *(_S225 + vid_5);
    packed_float4 device* _S226 = ed_buf_6((&kernelContext_26)->ed_c_0->nodes_0, &kernelContext_26);
    float3 node_7 = (float4(*(_S226 + (((&_S224)->node_flags_0) & 65535U))) ).xyz;
    float3 _S227 = float3(v_5.px_2, v_5.py_2, v_5.pz_2);
    _S224 = inst_3;
    float3 _S228 = xform_point_0(&_S224, _S227);
    thread VisOut_0 o_5;
    (&o_5)->pos_12 = (((float4(node_7 + _S228, 1.0f)) * (matrix<float,int(4),int(4)> (_S220->view_proj_0.data_1[int(0)][int(0)], _S220->view_proj_0.data_1[int(1)][int(0)], _S220->view_proj_0.data_1[int(2)][int(0)], _S220->view_proj_0.data_1[int(3)][int(0)], _S220->view_proj_0.data_1[int(0)][int(1)], _S220->view_proj_0.data_1[int(1)][int(1)], _S220->view_proj_0.data_1[int(2)][int(1)], _S220->view_proj_0.data_1[int(3)][int(1)], _S220->view_proj_0.data_1[int(0)][int(2)], _S220->view_proj_0.data_1[int(1)][int(2)], _S220->view_proj_0.data_1[int(2)][int(2)], _S220->view_proj_0.data_1[int(3)][int(2)], _S220->view_proj_0.data_1[int(0)][int(3)], _S220->view_proj_0.data_1[int(1)][int(3)], _S220->view_proj_0.data_1[int(2)][int(3)], _S220->view_proj_0.data_1[int(3)][int(3)]))));
    (&o_5)->entry_14 = entry_15;
    thread vs_vis_Result_0 _S229;
    (&_S229)->pos_11 = o_5.pos_12;
    (&_S229)->entry_13 = o_5.entry_14;
    return _S229;
}

struct vs_shadow_Result_0
{
    float4 pos_13 [[position]];
};

struct ShadowOut_0
{
    float4 pos_14;
};

[[vertex]] vs_shadow_Result_0 vs_shadow(uint vid_6 [[vertex_id]], uint iid_6 [[instance_id]], VisC_0 constant* ed_c_12 [[buffer(0)]], ulong device* ed_buffers_12 [[buffer(2)]], ulong device* ed_textures_12 [[buffer(1)]], ulong device* ed_samplers_12 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_27;
    (&kernelContext_27)->ed_c_0 = ed_c_12;
    (&kernelContext_27)->ed_buffers_0 = ed_buffers_12;
    (&kernelContext_27)->ed_textures_0 = ed_textures_12;
    (&kernelContext_27)->ed_samplers_0 = ed_samplers_12;
    FrameData_natural_0 device* _S230 = ed_buf_10(ed_c_12->frame_0, &kernelContext_27);
    uint device* _S231 = ed_buf_9((&kernelContext_27)->ed_c_0->visible_0, &kernelContext_27);
    uint entry_16 = *(_S231 + iid_6);
    GpuInstance_natural_0 device* _S232 = ed_buf_8((&kernelContext_27)->ed_c_0->instances_0, &kernelContext_27);
    GpuInstance_natural_0 device* _S233 = _S232 + (entry_16 & 1048575U);
    GpuInstance_natural_0 inst_4 = *_S233;
    thread GpuInstance_natural_0 _S234 = *_S233;
    Vtx_0 device* _S235 = ed_buf_7((&kernelContext_27)->ed_c_0->verts_0, &kernelContext_27);
    Vtx_0 v_6 = *(_S235 + vid_6);
    packed_float4 device* _S236 = ed_buf_6((&kernelContext_27)->ed_c_0->nodes_0, &kernelContext_27);
    float3 node_8 = (float4(*(_S236 + (((&_S234)->node_flags_0) & 65535U))) ).xyz;
    float3 _S237 = float3(v_6.px_2, v_6.py_2, v_6.pz_2);
    _S234 = inst_4;
    float3 _S238 = xform_point_0(&_S234, _S237);
    thread ShadowOut_0 o_6;
    (&o_6)->pos_14 = (((float4(node_8 + _S238, 1.0f)) * (matrix<float,int(4),int(4)> ((&(&_S230->light_0)->shadow_0)->data_3[(&kernelContext_27)->ed_c_0->cascade_0].data_1[int(0)][int(0)], (&(&_S230->light_0)->shadow_0)->data_3[(&kernelContext_27)->ed_c_0->cascade_0].data_1[int(1)][int(0)], (&(&_S230->light_0)->shadow_0)->data_3[(&kernelContext_27)->ed_c_0->cascade_0].data_1[int(2)][int(0)], (&(&_S230->light_0)->shadow_0)->data_3[(&kernelContext_27)->ed_c_0->cascade_0].data_1[int(3)][int(0)], (&(&_S230->light_0)->shadow_0)->data_3[(&kernelContext_27)->ed_c_0->cascade_0].data_1[int(0)][int(1)], (&(&_S230->light_0)->shadow_0)->data_3[(&kernelContext_27)->ed_c_0->cascade_0].data_1[int(1)][int(1)], (&(&_S230->light_0)->shadow_0)->data_3[(&kernelContext_27)->ed_c_0->cascade_0].data_1[int(2)][int(1)], (&(&_S230->light_0)->shadow_0)->data_3[(&kernelContext_27)->ed_c_0->cascade_0].data_1[int(3)][int(1)], (&(&_S230->light_0)->shadow_0)->data_3[(&kernelContext_27)->ed_c_0->cascade_0].data_1[int(0)][int(2)], (&(&_S230->light_0)->shadow_0)->data_3[(&kernelContext_27)->ed_c_0->cascade_0].data_1[int(1)][int(2)], (&(&_S230->light_0)->shadow_0)->data_3[(&kernelContext_27)->ed_c_0->cascade_0].data_1[int(2)][int(2)], (&(&_S230->light_0)->shadow_0)->data_3[(&kernelContext_27)->ed_c_0->cascade_0].data_1[int(3)][int(2)], (&(&_S230->light_0)->shadow_0)->data_3[(&kernelContext_27)->ed_c_0->cascade_0].data_1[int(0)][int(3)], (&(&_S230->light_0)->shadow_0)->data_3[(&kernelContext_27)->ed_c_0->cascade_0].data_1[int(1)][int(3)], (&(&_S230->light_0)->shadow_0)->data_3[(&kernelContext_27)->ed_c_0->cascade_0].data_1[int(2)][int(3)], (&(&_S230->light_0)->shadow_0)->data_3[(&kernelContext_27)->ed_c_0->cascade_0].data_1[int(3)][int(3)]))));
    thread vs_shadow_Result_0 _S239;
    (&_S239)->pos_13 = o_6.pos_14;
    return _S239;
}

struct vs_foliage_Result_0
{
    float4 pos_15 [[position]];
    float2 uv_9 [[user(TEXCOORD)]];
    uint entry_17 [[user(ENTRY)]];
    uint tex_6 [[user(TEXID)]];
    float2 sd_5 [[user(SEASON)]];
};

[[vertex]] vs_foliage_Result_0 vs_foliage(uint vid_7 [[vertex_id]], uint iid_7 [[instance_id]], VisC_0 constant* ed_c_13 [[buffer(0)]], ulong device* ed_buffers_13 [[buffer(2)]], ulong device* ed_textures_13 [[buffer(1)]], ulong device* ed_samplers_13 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_28;
    (&kernelContext_28)->ed_c_0 = ed_c_13;
    (&kernelContext_28)->ed_buffers_0 = ed_buffers_13;
    (&kernelContext_28)->ed_textures_0 = ed_textures_13;
    (&kernelContext_28)->ed_samplers_0 = ed_samplers_13;
    FoliageOut_0 _S240 = foliage_vertex_0(vid_7, iid_7, false, &kernelContext_28);
    thread vs_foliage_Result_0 _S241;
    (&_S241)->pos_15 = _S240.pos_2;
    (&_S241)->uv_9 = _S240.uv_1;
    (&_S241)->entry_17 = _S240.entry_3;
    (&_S241)->tex_6 = _S240.tex_2;
    (&_S241)->sd_5 = _S240.sd_0;
    return _S241;
}

struct vs_shadow_foliage_Result_0
{
    float4 pos_16 [[position]];
    float2 uv_10 [[user(TEXCOORD)]];
    uint entry_18 [[user(ENTRY)]];
    uint tex_7 [[user(TEXID)]];
    float2 sd_6 [[user(SEASON)]];
};

[[vertex]] vs_shadow_foliage_Result_0 vs_shadow_foliage(uint vid_8 [[vertex_id]], uint iid_8 [[instance_id]], VisC_0 constant* ed_c_14 [[buffer(0)]], ulong device* ed_buffers_14 [[buffer(2)]], ulong device* ed_textures_14 [[buffer(1)]], ulong device* ed_samplers_14 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_29;
    (&kernelContext_29)->ed_c_0 = ed_c_14;
    (&kernelContext_29)->ed_buffers_0 = ed_buffers_14;
    (&kernelContext_29)->ed_textures_0 = ed_textures_14;
    (&kernelContext_29)->ed_samplers_0 = ed_samplers_14;
    FoliageOut_0 _S242 = foliage_vertex_0(vid_8, iid_8, true, &kernelContext_29);
    thread vs_shadow_foliage_Result_0 _S243;
    (&_S243)->pos_16 = _S242.pos_2;
    (&_S243)->uv_10 = _S242.uv_1;
    (&_S243)->entry_18 = _S242.entry_3;
    (&_S243)->tex_7 = _S242.tex_2;
    (&_S243)->sd_6 = _S242.sd_0;
    return _S243;
}

struct vs_imp_Result_0
{
    float4 pos_17 [[position]];
    float2 uv0_3 [[user(TEXCOORD)]];
    float2 uv1_3 [[user(TEXCOORD_1)]];
    float2 uv2_3 [[user(TEXCOORD_2)]];
    float zv_3 [[user(TEXCOORD_3)]];
    uint entry_19 [[user(ENTRY)]];
    uint4 fi_5 [[user(FRAMES)]];
    float4 w_11 [[user(WEIGHTS)]];
    float2 geo_4 [[user(GEO)]];
    float front_4 [[user(FRONT)]];
    float lv_3 [[user(LEAVES)]];
};

[[vertex]] vs_imp_Result_0 vs_imp(uint vid_9 [[vertex_id]], uint iid_9 [[instance_id]], VisC_0 constant* ed_c_15 [[buffer(0)]], ulong device* ed_buffers_15 [[buffer(2)]], ulong device* ed_textures_15 [[buffer(1)]], ulong device* ed_samplers_15 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_30;
    (&kernelContext_30)->ed_c_0 = ed_c_15;
    (&kernelContext_30)->ed_buffers_0 = ed_buffers_15;
    (&kernelContext_30)->ed_textures_0 = ed_textures_15;
    (&kernelContext_30)->ed_samplers_0 = ed_samplers_15;
    ImpOut_0 _S244 = imp_vertex_0(vid_9, iid_9, false, &kernelContext_30);
    thread vs_imp_Result_0 _S245;
    (&_S245)->pos_17 = _S244.pos_6;
    (&_S245)->uv0_3 = _S244.uv0_0;
    (&_S245)->uv1_3 = _S244.uv1_0;
    (&_S245)->uv2_3 = _S244.uv2_0;
    (&_S245)->zv_3 = _S244.zv_0;
    (&_S245)->entry_19 = _S244.entry_8;
    (&_S245)->fi_5 = _S244.fi_1;
    (&_S245)->w_11 = _S244.w_4;
    (&_S245)->geo_4 = _S244.geo_1;
    (&_S245)->front_4 = _S244.front_0;
    (&_S245)->lv_3 = _S244.lv_0;
    return _S245;
}

struct vs_imp_shadow_Result_0
{
    float4 pos_18 [[position]];
    float2 uv0_4 [[user(TEXCOORD)]];
    float2 uv1_4 [[user(TEXCOORD_1)]];
    float2 uv2_4 [[user(TEXCOORD_2)]];
    float zv_4 [[user(TEXCOORD_3)]];
    uint entry_20 [[user(ENTRY)]];
    uint4 fi_6 [[user(FRAMES)]];
    float4 w_12 [[user(WEIGHTS)]];
    float2 geo_5 [[user(GEO)]];
    float front_5 [[user(FRONT)]];
    float lv_4 [[user(LEAVES)]];
};

[[vertex]] vs_imp_shadow_Result_0 vs_imp_shadow(uint vid_10 [[vertex_id]], uint iid_10 [[instance_id]], VisC_0 constant* ed_c_16 [[buffer(0)]], ulong device* ed_buffers_16 [[buffer(2)]], ulong device* ed_textures_16 [[buffer(1)]], ulong device* ed_samplers_16 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_31;
    (&kernelContext_31)->ed_c_0 = ed_c_16;
    (&kernelContext_31)->ed_buffers_0 = ed_buffers_16;
    (&kernelContext_31)->ed_textures_0 = ed_textures_16;
    (&kernelContext_31)->ed_samplers_0 = ed_samplers_16;
    ImpOut_0 _S246 = imp_vertex_0(vid_10, iid_10, true, &kernelContext_31);
    thread vs_imp_shadow_Result_0 _S247;
    (&_S247)->pos_18 = _S246.pos_6;
    (&_S247)->uv0_4 = _S246.uv0_0;
    (&_S247)->uv1_4 = _S246.uv1_0;
    (&_S247)->uv2_4 = _S246.uv2_0;
    (&_S247)->zv_4 = _S246.zv_0;
    (&_S247)->entry_20 = _S246.entry_8;
    (&_S247)->fi_6 = _S246.fi_1;
    (&_S247)->w_12 = _S246.w_4;
    (&_S247)->geo_5 = _S246.geo_1;
    (&_S247)->front_5 = _S246.front_0;
    (&_S247)->lv_4 = _S246.lv_0;
    return _S247;
}

struct vs_grass_Result_0
{
    float4 pos_19 [[position]];
    float2 tu_2 [[user(TEXCOORD)]];
    uint clump_2 [[user(ENTRY)]];
    uint blade_2 [[user(BLADE)]];
};

[[vertex]] vs_grass_Result_0 vs_grass(uint vid_11 [[vertex_id]], uint iid_11 [[instance_id]], VisC_0 constant* ed_c_17 [[buffer(0)]], ulong device* ed_buffers_17 [[buffer(2)]], ulong device* ed_textures_17 [[buffer(1)]], ulong device* ed_samplers_17 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_32;
    (&kernelContext_32)->ed_c_0 = ed_c_17;
    (&kernelContext_32)->ed_buffers_0 = ed_buffers_17;
    (&kernelContext_32)->ed_textures_0 = ed_textures_17;
    (&kernelContext_32)->ed_samplers_0 = ed_samplers_17;
    GrassOut_0 _S248 = grass_vertex_0(vid_11, iid_11, false, &kernelContext_32);
    thread vs_grass_Result_0 _S249;
    (&_S249)->pos_19 = _S248.pos_9;
    (&_S249)->tu_2 = _S248.tu_0;
    (&_S249)->clump_2 = _S248.clump_0;
    (&_S249)->blade_2 = _S248.blade_0;
    return _S249;
}

