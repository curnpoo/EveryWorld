#include <metal_stdlib>
#include <metal_math>
#include <metal_texture>
using namespace metal;
struct EguiC_0
{
    float2 screen_0;
    uint buf_0;
    uint vtx_0;
    uint idx_0;
    uint tex_0;
    uint smp_0;
    uint linear_out_0;
    array<uint, int(8)> pad_0;
};

struct KernelContext_0
{
    EguiC_0 constant* ed_c_0;
    ulong device* ed_buffers_0;
    ulong device* ed_textures_0;
    ulong device* ed_samplers_0;
};

uint device* ed_buf_0(uint i_0, KernelContext_0 thread* kernelContext_0)
{
    return (uint device*)kernelContext_0->ed_buffers_0[i_0];
}

float3 egui_linear_from_gamma_0(float3 s_0)
{
    return select(pow((s_0 + float3(0.054999999701976776f) ) / float3(1.0549999475479126f) , float3(2.4000000953674316f) ), s_0 / float3(12.920000076293945f) , s_0 < (float3(0.040449999272823334f) ));
}

struct pixelOutput_0
{
    float4 output_0 [[color(0)]];
};

struct pixelInput_0
{
    float4 color_0 [[user(COLOR)]];
    float2 uv_0 [[user(TEXCOORD)]];
};

[[fragment]] pixelOutput_0 fs_egui(pixelInput_0 _S1 [[stage_in]], float4 pos_0 [[position]], EguiC_0 constant* ed_c_1 [[buffer(0)]], ulong device* ed_buffers_1 [[buffer(2)]], ulong device* ed_textures_1 [[buffer(1)]], ulong device* ed_samplers_1 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_1;
    (&kernelContext_1)->ed_c_0 = ed_c_1;
    (&kernelContext_1)->ed_buffers_0 = ed_buffers_1;
    (&kernelContext_1)->ed_textures_0 = ed_textures_1;
    (&kernelContext_1)->ed_samplers_0 = ed_samplers_1;
    texture2d<float, access::sample> _S2 = ((reinterpret_cast<device texture2d<float, access::sample>*>((ed_textures_1))[(ed_c_1->tex_0)]));
    sampler _S3 = ((reinterpret_cast<device sampler*>((ed_samplers_1))[(ed_c_1->smp_0)]));
    thread float4 c_0 = _S1.color_0 * ((_S2).sample((_S3), (_S1.uv_0)));
    if((ed_c_1->linear_out_0) != 0U)
    {
        c_0.xyz = egui_linear_from_gamma_0(c_0.xyz);
    }
    pixelOutput_0 _S4 = { c_0 };
    return _S4;
}

struct vs_egui_Result_0
{
    float4 pos_1 [[position]];
    float4 color_1 [[user(COLOR)]];
    float2 uv_1 [[user(TEXCOORD)]];
};

struct EguiOut_0
{
    float4 pos_2;
    float4 color_2;
    float2 uv_2;
};

[[vertex]] vs_egui_Result_0 vs_egui(uint vid_0 [[vertex_id]], EguiC_0 constant* ed_c_2 [[buffer(0)]], ulong device* ed_buffers_2 [[buffer(2)]], ulong device* ed_textures_2 [[buffer(1)]], ulong device* ed_samplers_2 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_2;
    (&kernelContext_2)->ed_c_0 = ed_c_2;
    (&kernelContext_2)->ed_buffers_0 = ed_buffers_2;
    (&kernelContext_2)->ed_textures_0 = ed_textures_2;
    (&kernelContext_2)->ed_samplers_0 = ed_samplers_2;
    uint device* _S5 = ed_buf_0(ed_c_2->buf_0, &kernelContext_2);
    uint device* v_0 = _S5 + (&kernelContext_2)->ed_c_0->vtx_0 / 4U + *(_S5 + ((&kernelContext_2)->ed_c_0->idx_0 / 4U + vid_0)) * 5U;
    uint color_3 = *(v_0 + int(4));
    thread EguiOut_0 o_0;
    (&o_0)->pos_2 = float4(2.0f * (as_type<float>((*(v_0 + int(0))))) / (&kernelContext_2)->ed_c_0->screen_0.x - 1.0f, 1.0f - 2.0f * (as_type<float>((*(v_0 + int(1))))) / (&kernelContext_2)->ed_c_0->screen_0.y, 0.0f, 1.0f);
    (&o_0)->color_2 = float4(float(color_3 & 255U), float((color_3 >> 8U) & 255U), float((color_3 >> 16U) & 255U), float(color_3 >> 24U)) / float4(255.0f) ;
    (&o_0)->uv_2 = float2((as_type<float>((*(v_0 + int(2))))), (as_type<float>((*(v_0 + int(3))))));
    thread vs_egui_Result_0 _S6;
    (&_S6)->pos_1 = o_0.pos_2;
    (&_S6)->color_1 = o_0.color_2;
    (&_S6)->uv_1 = o_0.uv_2;
    return _S6;
}

