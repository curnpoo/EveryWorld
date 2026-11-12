#include <metal_stdlib>
#include <metal_math>
#include <metal_texture>
using namespace metal;
struct OverlayV_0
{
    float px_0;
    float py_0;
    float pz_0;
    uint color_0;
};

struct OverlayC_0
{
    uint frame_0;
    uint verts_0;
    uint first_0;
    uint linear_out_0;
    float4 offset_0;
    array<uint, int(8)> pad_0;
};

struct KernelContext_0
{
    OverlayC_0 constant* ed_c_0;
    ulong device* ed_buffers_0;
};

OverlayV_0 device* ed_buf_0(uint i_0, KernelContext_0 thread* kernelContext_0)
{
    return (OverlayV_0 device*)kernelContext_0->ed_buffers_0[i_0];
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
    _Array_natural_vectorx3Cfloatx2C4x3E4_0 cascade_0;
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

FrameData_natural_0 device* ed_buf_1(uint i_1, KernelContext_0 thread* kernelContext_1)
{
    return (FrameData_natural_0 device*)kernelContext_1->ed_buffers_0[i_1];
}

float4 unpack_unorm4x8_0(uint p_0)
{
    return float4(float(p_0 & 255U), float((p_0 >> 8U) & 255U), float((p_0 >> 16U) & 255U), float(p_0 >> 24U)) / float4(255.0f) ;
}

float3 overlay_linear_from_gamma_0(float3 s_0)
{
    return select(pow((s_0 + float3(0.054999999701976776f) ) / float3(1.0549999475479126f) , float3(2.4000000953674316f) ), s_0 / float3(12.920000076293945f) , s_0 < (float3(0.040449999272823334f) ));
}

struct pixelOutput_0
{
    float4 output_0 [[color(0)]];
};

struct pixelInput_0
{
    float4 color_1 [[user(COLOR)]];
};

[[fragment]] pixelOutput_0 fs_overlay(pixelInput_0 _S1 [[stage_in]], float4 pos_0 [[position]], OverlayC_0 constant* ed_c_1 [[buffer(0)]], ulong device* ed_buffers_1 [[buffer(2)]])
{
    thread KernelContext_0 kernelContext_2;
    (&kernelContext_2)->ed_c_0 = ed_c_1;
    (&kernelContext_2)->ed_buffers_0 = ed_buffers_1;
    thread float4 c_0 = _S1.color_1;
    if((ed_c_1->linear_out_0) != 0U)
    {
        c_0.xyz = overlay_linear_from_gamma_0(c_0.xyz);
    }
    pixelOutput_0 _S2 = { c_0 };
    return _S2;
}

struct vs_overlay_Result_0
{
    float4 pos_1 [[position]];
    float4 color_2 [[user(COLOR)]];
};

struct OverlayOut_0
{
    float4 pos_2;
    float4 color_3;
};

[[vertex]] vs_overlay_Result_0 vs_overlay(uint vid_0 [[vertex_id]], OverlayC_0 constant* ed_c_2 [[buffer(0)]], ulong device* ed_buffers_2 [[buffer(2)]])
{
    thread KernelContext_0 kernelContext_3;
    (&kernelContext_3)->ed_c_0 = ed_c_2;
    (&kernelContext_3)->ed_buffers_0 = ed_buffers_2;
    FrameData_natural_0 device* _S3 = ed_buf_1(ed_c_2->frame_0, &kernelContext_3);
    OverlayV_0 device* _S4 = ed_buf_0((&kernelContext_3)->ed_c_0->verts_0, &kernelContext_3);
    OverlayV_0 device* _S5 = _S4 + ((&kernelContext_3)->ed_c_0->first_0 + vid_0);
    OverlayV_0 v_0 = *_S5;
    thread OverlayOut_0 o_0;
    (&o_0)->pos_2 = (((float4(float3((*_S5).px_0, (*_S5).py_0, (*_S5).pz_0) + (&kernelContext_3)->ed_c_0->offset_0.xyz, 1.0f)) * (matrix<float,int(4),int(4)> (_S3->view_proj_0.data_0[int(0)][int(0)], _S3->view_proj_0.data_0[int(1)][int(0)], _S3->view_proj_0.data_0[int(2)][int(0)], _S3->view_proj_0.data_0[int(3)][int(0)], _S3->view_proj_0.data_0[int(0)][int(1)], _S3->view_proj_0.data_0[int(1)][int(1)], _S3->view_proj_0.data_0[int(2)][int(1)], _S3->view_proj_0.data_0[int(3)][int(1)], _S3->view_proj_0.data_0[int(0)][int(2)], _S3->view_proj_0.data_0[int(1)][int(2)], _S3->view_proj_0.data_0[int(2)][int(2)], _S3->view_proj_0.data_0[int(3)][int(2)], _S3->view_proj_0.data_0[int(0)][int(3)], _S3->view_proj_0.data_0[int(1)][int(3)], _S3->view_proj_0.data_0[int(2)][int(3)], _S3->view_proj_0.data_0[int(3)][int(3)]))));
    (&o_0)->color_3 = unpack_unorm4x8_0(v_0.color_0);
    thread vs_overlay_Result_0 _S6;
    (&_S6)->pos_1 = o_0.pos_2;
    (&_S6)->color_2 = o_0.color_3;
    return _S6;
}

