#include <metal_stdlib>
#include <metal_math>
#include <metal_texture>
using namespace metal;
constant array<uint, int(64)> BAYER8_0 = { { 0U, 32U, 8U, 40U, 2U, 34U, 10U, 42U, 48U, 16U, 56U, 24U, 50U, 18U, 58U, 26U, 12U, 44U, 4U, 36U, 14U, 46U, 6U, 38U, 60U, 28U, 52U, 20U, 62U, 30U, 54U, 22U, 3U, 35U, 11U, 43U, 1U, 33U, 9U, 41U, 51U, 19U, 59U, 27U, 49U, 17U, 57U, 25U, 15U, 47U, 7U, 39U, 13U, 45U, 5U, 37U, 63U, 31U, 55U, 23U, 61U, 29U, 53U, 21U } };
struct VisC_0
{
    uint frame_0;
    uint instances_0;
    uint nodes_0;
    uint visible_0;
    uint verts_0;
    uint cascade_0;
    array<uint, int(10)> pad_0;
};

struct KernelContext_0
{
    VisC_0 constant* ed_c_0;
    ulong device* ed_buffers_0;
};

packed_float4 device* ed_buf_0(uint i_0, KernelContext_0 thread* kernelContext_0)
{
    return (packed_float4 device*)kernelContext_0->ed_buffers_0[i_0];
}

struct Vtx_0
{
    float px_0;
    float py_0;
    float pz_0;
    uint n_0;
};

Vtx_0 device* ed_buf_1(uint i_1, KernelContext_0 thread* kernelContext_1)
{
    return (Vtx_0 device*)kernelContext_1->ed_buffers_0[i_1];
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

GpuInstance_natural_0 device* ed_buf_2(uint i_2, KernelContext_0 thread* kernelContext_2)
{
    return (GpuInstance_natural_0 device*)kernelContext_2->ed_buffers_0[i_2];
}

uint device* ed_buf_3(uint i_3, KernelContext_0 thread* kernelContext_3)
{
    return (uint device*)kernelContext_3->ed_buffers_0[i_3];
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
};

FrameData_natural_0 device* ed_buf_4(uint i_4, KernelContext_0 thread* kernelContext_4)
{
    return (FrameData_natural_0 device*)kernelContext_4->ed_buffers_0[i_4];
}

float3 xform_point_0(const GpuInstance_natural_0 thread* i_5, float3 p_0)
{
    float4 _S1 = float4((&i_5->m_0)->data_0[int(0)]) ;
    float4 _S2 = float4((&i_5->m_0)->data_0[int(1)]) ;
    float4 _S3 = float4((&i_5->m_0)->data_0[int(2)]) ;
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

[[fragment]] pixelOutput_0 fs_vis(pixelInput_0 _S4 [[stage_in]], uint prim_1 [[primitive_id]], float4 pos_0 [[position]], VisC_0 constant* ed_c_1 [[buffer(0)]], ulong device* ed_buffers_1 [[buffer(2)]])
{
    thread KernelContext_0 kernelContext_5;
    (&kernelContext_5)->ed_c_0 = ed_c_1;
    (&kernelContext_5)->ed_buffers_0 = ed_buffers_1;
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

[[fragment]] pixelOutput_1 fs_vis_dither(pixelInput_1 _S6 [[stage_in]], uint prim_2 [[primitive_id]], float4 pos_1 [[position]], VisC_0 constant* ed_c_2 [[buffer(0)]], ulong device* ed_buffers_2 [[buffer(2)]])
{
    thread KernelContext_0 kernelContext_6;
    (&kernelContext_6)->ed_c_0 = ed_c_2;
    (&kernelContext_6)->ed_buffers_0 = ed_buffers_2;
    uint2 p_1 = uint2(pos_1.xy) & (uint2(7U) );
    if(((((BAYER8_0[p_1.y * 8U + p_1.x] + ((((_S6.entry_2) & 1048575U) * 2654435761U) >> 26U)) & 63U) * 4U + 2U) < (((_S6.entry_2) >> 22U) & 255U)) == ((((_S6.entry_2) >> 30U) & 1U) != 0U))
    {
        discard_fragment();
    }
    pixelOutput_1 _S7 = { vis_value_0(_S6.entry_2, prim_2) };
    return _S7;
}

struct vs_vis_Result_0
{
    float4 pos_2 [[position]];
    uint entry_3 [[user(ENTRY)]];
};

struct VisOut_0
{
    float4 pos_3;
    [[flat]] uint entry_4;
};

[[vertex]] vs_vis_Result_0 vs_vis(uint vid_0 [[vertex_id]], uint iid_0 [[instance_id]], VisC_0 constant* ed_c_3 [[buffer(0)]], ulong device* ed_buffers_3 [[buffer(2)]])
{
    thread KernelContext_0 kernelContext_7;
    (&kernelContext_7)->ed_c_0 = ed_c_3;
    (&kernelContext_7)->ed_buffers_0 = ed_buffers_3;
    FrameData_natural_0 device* _S8 = ed_buf_4(ed_c_3->frame_0, &kernelContext_7);
    uint device* _S9 = ed_buf_3((&kernelContext_7)->ed_c_0->visible_0, &kernelContext_7);
    uint entry_5 = *(_S9 + iid_0);
    GpuInstance_natural_0 device* _S10 = ed_buf_2((&kernelContext_7)->ed_c_0->instances_0, &kernelContext_7);
    GpuInstance_natural_0 device* _S11 = _S10 + (entry_5 & 1048575U);
    GpuInstance_natural_0 inst_0 = *_S11;
    thread GpuInstance_natural_0 _S12 = *_S11;
    Vtx_0 device* _S13 = ed_buf_1((&kernelContext_7)->ed_c_0->verts_0, &kernelContext_7);
    Vtx_0 v_0 = *(_S13 + vid_0);
    packed_float4 device* _S14 = ed_buf_0((&kernelContext_7)->ed_c_0->nodes_0, &kernelContext_7);
    float3 node_0 = (float4(*(_S14 + (((&_S12)->node_flags_0) & 65535U))) ).xyz;
    float3 _S15 = float3(v_0.px_0, v_0.py_0, v_0.pz_0);
    _S12 = inst_0;
    float3 _S16 = xform_point_0(&_S12, _S15);
    thread VisOut_0 o_0;
    (&o_0)->pos_3 = (((float4(node_0 + _S16, 1.0f)) * (matrix<float,int(4),int(4)> (_S8->view_proj_0.data_1[int(0)][int(0)], _S8->view_proj_0.data_1[int(1)][int(0)], _S8->view_proj_0.data_1[int(2)][int(0)], _S8->view_proj_0.data_1[int(3)][int(0)], _S8->view_proj_0.data_1[int(0)][int(1)], _S8->view_proj_0.data_1[int(1)][int(1)], _S8->view_proj_0.data_1[int(2)][int(1)], _S8->view_proj_0.data_1[int(3)][int(1)], _S8->view_proj_0.data_1[int(0)][int(2)], _S8->view_proj_0.data_1[int(1)][int(2)], _S8->view_proj_0.data_1[int(2)][int(2)], _S8->view_proj_0.data_1[int(3)][int(2)], _S8->view_proj_0.data_1[int(0)][int(3)], _S8->view_proj_0.data_1[int(1)][int(3)], _S8->view_proj_0.data_1[int(2)][int(3)], _S8->view_proj_0.data_1[int(3)][int(3)]))));
    (&o_0)->entry_4 = entry_5;
    thread vs_vis_Result_0 _S17;
    (&_S17)->pos_2 = o_0.pos_3;
    (&_S17)->entry_3 = o_0.entry_4;
    return _S17;
}

struct vs_shadow_Result_0
{
    float4 pos_4 [[position]];
};

struct ShadowOut_0
{
    float4 pos_5;
};

[[vertex]] vs_shadow_Result_0 vs_shadow(uint vid_1 [[vertex_id]], uint iid_1 [[instance_id]], VisC_0 constant* ed_c_4 [[buffer(0)]], ulong device* ed_buffers_4 [[buffer(2)]])
{
    thread KernelContext_0 kernelContext_8;
    (&kernelContext_8)->ed_c_0 = ed_c_4;
    (&kernelContext_8)->ed_buffers_0 = ed_buffers_4;
    FrameData_natural_0 device* _S18 = ed_buf_4(ed_c_4->frame_0, &kernelContext_8);
    uint device* _S19 = ed_buf_3((&kernelContext_8)->ed_c_0->visible_0, &kernelContext_8);
    uint entry_6 = *(_S19 + iid_1);
    GpuInstance_natural_0 device* _S20 = ed_buf_2((&kernelContext_8)->ed_c_0->instances_0, &kernelContext_8);
    GpuInstance_natural_0 device* _S21 = _S20 + (entry_6 & 1048575U);
    GpuInstance_natural_0 inst_1 = *_S21;
    thread GpuInstance_natural_0 _S22 = *_S21;
    Vtx_0 device* _S23 = ed_buf_1((&kernelContext_8)->ed_c_0->verts_0, &kernelContext_8);
    Vtx_0 v_1 = *(_S23 + vid_1);
    packed_float4 device* _S24 = ed_buf_0((&kernelContext_8)->ed_c_0->nodes_0, &kernelContext_8);
    float3 node_1 = (float4(*(_S24 + (((&_S22)->node_flags_0) & 65535U))) ).xyz;
    float3 _S25 = float3(v_1.px_0, v_1.py_0, v_1.pz_0);
    _S22 = inst_1;
    float3 _S26 = xform_point_0(&_S22, _S25);
    thread ShadowOut_0 o_1;
    (&o_1)->pos_5 = (((float4(node_1 + _S26, 1.0f)) * (matrix<float,int(4),int(4)> ((&(&_S18->light_0)->shadow_0)->data_3[(&kernelContext_8)->ed_c_0->cascade_0].data_1[int(0)][int(0)], (&(&_S18->light_0)->shadow_0)->data_3[(&kernelContext_8)->ed_c_0->cascade_0].data_1[int(1)][int(0)], (&(&_S18->light_0)->shadow_0)->data_3[(&kernelContext_8)->ed_c_0->cascade_0].data_1[int(2)][int(0)], (&(&_S18->light_0)->shadow_0)->data_3[(&kernelContext_8)->ed_c_0->cascade_0].data_1[int(3)][int(0)], (&(&_S18->light_0)->shadow_0)->data_3[(&kernelContext_8)->ed_c_0->cascade_0].data_1[int(0)][int(1)], (&(&_S18->light_0)->shadow_0)->data_3[(&kernelContext_8)->ed_c_0->cascade_0].data_1[int(1)][int(1)], (&(&_S18->light_0)->shadow_0)->data_3[(&kernelContext_8)->ed_c_0->cascade_0].data_1[int(2)][int(1)], (&(&_S18->light_0)->shadow_0)->data_3[(&kernelContext_8)->ed_c_0->cascade_0].data_1[int(3)][int(1)], (&(&_S18->light_0)->shadow_0)->data_3[(&kernelContext_8)->ed_c_0->cascade_0].data_1[int(0)][int(2)], (&(&_S18->light_0)->shadow_0)->data_3[(&kernelContext_8)->ed_c_0->cascade_0].data_1[int(1)][int(2)], (&(&_S18->light_0)->shadow_0)->data_3[(&kernelContext_8)->ed_c_0->cascade_0].data_1[int(2)][int(2)], (&(&_S18->light_0)->shadow_0)->data_3[(&kernelContext_8)->ed_c_0->cascade_0].data_1[int(3)][int(2)], (&(&_S18->light_0)->shadow_0)->data_3[(&kernelContext_8)->ed_c_0->cascade_0].data_1[int(0)][int(3)], (&(&_S18->light_0)->shadow_0)->data_3[(&kernelContext_8)->ed_c_0->cascade_0].data_1[int(1)][int(3)], (&(&_S18->light_0)->shadow_0)->data_3[(&kernelContext_8)->ed_c_0->cascade_0].data_1[int(2)][int(3)], (&(&_S18->light_0)->shadow_0)->data_3[(&kernelContext_8)->ed_c_0->cascade_0].data_1[int(3)][int(3)]))));
    thread vs_shadow_Result_0 _S27;
    (&_S27)->pos_4 = o_1.pos_5;
    return _S27;
}

