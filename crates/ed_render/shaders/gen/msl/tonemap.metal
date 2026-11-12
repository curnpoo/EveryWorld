#include <metal_stdlib>
#include <metal_math>
#include <metal_texture>
using namespace metal;
struct FsOut_0
{
    float4 pos_0;
};

FsOut_0 fullscreen_vertex_0(uint vid_0)
{
    thread FsOut_0 o_0;
    (&o_0)->pos_0 = float4(float2(float((vid_0 << 1U) & 2U), float(vid_0 & 2U)) * float2(2.0f)  - float2(1.0f) , 0.0f, 1.0f);
    return o_0;
}

float3 aces_fit_0(float3 x_0)
{
    return saturate(x_0 * (float3(2.5099999904632568f)  * x_0 + float3(0.029999999329447746f) ) / (x_0 * (float3(2.4300000667572021f)  * x_0 + float3(0.5899999737739563f) ) + float3(0.14000000059604645f) ));
}

float3 srgb_encode_0(float3 c_0)
{
    return select(float3(1.0549999475479126f)  * pow(c_0, float3(0.4166666567325592f) ) - float3(0.054999999701976776f) , c_0 * float3(12.920000076293945f) , c_0 <= (float3(0.0031308000907301903f) ));
}

struct pixelOutput_0
{
    float4 output_0 [[color(0)]];
};

struct ToneC_0
{
    uint hdr_0;
    uint encode_srgb_0;
    float exposure_0;
    uint pad0_0;
    float4 viewport_0;
    array<uint, int(8)> pad_0;
};

struct KernelContext_0
{
    ToneC_0 constant* ed_c_0;
    ulong device* ed_textures_0;
};

[[fragment]] pixelOutput_0 fs_tonemap(float4 pos_1 [[position]], ToneC_0 constant* ed_c_1 [[buffer(0)]], ulong device* ed_textures_1 [[buffer(1)]])
{
    thread KernelContext_0 kernelContext_0;
    (&kernelContext_0)->ed_c_0 = ed_c_1;
    (&kernelContext_0)->ed_textures_0 = ed_textures_1;
    uint2 _S1 = uint2(max(pos_1.xy - ed_c_1->viewport_0.xy, float2(0.0f) ));
    texture2d<float, access::sample> _S2 = ((reinterpret_cast<device texture2d<float, access::sample>*>((ed_textures_1))[(ed_c_1->hdr_0)]));
    int3 _S3 = int3(int2(_S1), int(0));
    float3 c_1 = aces_fit_0(((_S2).read(vec<uint,2>(((_S3)).xy), uint(((_S3)).z))).xyz * float3(ed_c_1->exposure_0) );
    float3 c_2;
    if((ed_c_1->encode_srgb_0) != 0U)
    {
        c_2 = srgb_encode_0(c_1);
    }
    else
    {
        c_2 = c_1;
    }
    pixelOutput_0 _S4 = { float4(c_2, 1.0f) };
    return _S4;
}

struct vs_fullscreen_Result_0
{
    float4 pos_2 [[position]];
};

[[vertex]] vs_fullscreen_Result_0 vs_fullscreen(uint vid_1 [[vertex_id]], ToneC_0 constant* ed_c_2 [[buffer(0)]], ulong device* ed_textures_2 [[buffer(1)]])
{
    thread KernelContext_0 kernelContext_1;
    (&kernelContext_1)->ed_c_0 = ed_c_2;
    (&kernelContext_1)->ed_textures_0 = ed_textures_2;
    thread vs_fullscreen_Result_0 _S5;
    (&_S5)->pos_2 = fullscreen_vertex_0(vid_1).pos_0;
    return _S5;
}

