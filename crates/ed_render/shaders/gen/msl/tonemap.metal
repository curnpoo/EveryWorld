#include <metal_stdlib>
#include <metal_math>
#include <metal_texture>
using namespace metal;
struct ToneC_0
{
    uint hdr_0;
    uint encode_srgb_0;
    float exposure_0;
    uint lut_0;
    float4 viewport_0;
    uint ramp_0;
    array<uint, int(7)> pad_0;
};

struct KernelContext_0
{
    ToneC_0 constant* ed_c_0;
    ulong device* ed_textures_0;
    ulong device* ed_buffers_0;
};

uint device* ed_buf_0(uint i_0, KernelContext_0 thread* kernelContext_0)
{
    return (uint device*)kernelContext_0->ed_buffers_0[i_0];
}

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

float3 test_ramp_0(uint2 p_0, float2 size_0)
{
    array<float3, int(16)> BAND_0 = { { float3(1.0f, 1.0f, 1.0f), float3(1.0f, 0.0f, 0.0f), float3(0.0f, 1.0f, 0.0f), float3(0.0f, 0.0f, 1.0f), float3(1.0f, 1.0f, 0.0f), float3(0.0f, 1.0f, 1.0f), float3(1.0f, 0.0f, 1.0f), float3(1.0f, 0.5f, 0.0f), float3(0.5f, 1.0f, 0.0f), float3(0.0f, 1.0f, 0.5f), float3(0.0f, 0.5f, 1.0f), float3(0.5f, 0.0f, 1.0f), float3(1.0f, 0.0f, 0.5f), float3(1.0f, 0.20000000298023224f, 0.10000000149011612f), float3(0.10000000149011612f, 0.60000002384185791f, 0.20000000298023224f), float3(0.20000000298023224f, 0.30000001192092896f, 1.0f) } };
    uint _S1 = p_0.y * 16U / uint(size_0.y);
    return BAND_0[min(_S1, 15U)] * float3(exp2(-12.0f + 16.0f * (float(p_0.x) / max(size_0.x - 1.0f, 1.0f)))) ;
}

float3 lut_texel_0(uint device* lut_1, uint3 t_0)
{
    uint i_1 = ((t_0.z * 64U + t_0.y) * 64U + t_0.x) * 2U;
    uint device* _S2 = lut_1 + i_1;
    return float3((as_type<half>((ushort)(((*_S2) & 65535U)))), (as_type<half>((ushort)(((*_S2) >> 16U)))), (as_type<half>((ushort)(((*(lut_1 + (i_1 + 1U))) & 65535U)))));
}

float3 filmic_0(uint lut_buf_0, float3 x_0, KernelContext_0 thread* kernelContext_1)
{
    uint device* _S3 = ed_buf_0(lut_buf_0, kernelContext_1);
    float3 _S4 = max(x_0, float3(0.0f) );
    float3 t_1 = saturate((log2(select(_S4, _S4 + float3(1.5258779967552982e-05f) , _S4 < (float3(3.0517570849042386e-05f) )) / float3(0.18000000715255737f) ) + float3(11.0f) ) / float3(23.0f) ) * float3(63.0f) ;
    uint3 _S5 = min(uint3(t_1), uint3(62U) );
    float3 g_0 = t_1 - float3(_S5);
    float3 _S6 = lut_texel_0(_S3, _S5);
    float3 _S7 = lut_texel_0(_S3, _S5 + uint3(1U, 0U, 0U));
    float3 _S8 = float3(g_0.x) ;
    float3 c00_0 = mix(_S6, _S7, _S8);
    float3 _S9 = lut_texel_0(_S3, _S5 + uint3(0U, 1U, 0U));
    float3 _S10 = lut_texel_0(_S3, _S5 + uint3(1U, 1U, 0U));
    float3 c10_0 = mix(_S9, _S10, _S8);
    float3 _S11 = lut_texel_0(_S3, _S5 + uint3(0U, 0U, 1U));
    float3 _S12 = lut_texel_0(_S3, _S5 + uint3(1U, 0U, 1U));
    float3 c01_0 = mix(_S11, _S12, _S8);
    float3 _S13 = lut_texel_0(_S3, _S5 + uint3(0U, 1U, 1U));
    float3 _S14 = lut_texel_0(_S3, _S5 + uint3(1U, 1U, 1U));
    float3 _S15 = float3(g_0.y) ;
    return mix(mix(c00_0, c10_0, _S15), mix(c01_0, mix(_S13, _S14, _S8), _S15), float3(g_0.z) );
}

float3 alpha_tonemap_0(uint lut_buf_1, float3 hdr_1, float exposure_1, KernelContext_0 thread* kernelContext_2)
{
    float3 _S16 = filmic_0(lut_buf_1, hdr_1 * float3(exposure_1)  * float3(1.1000000238418579f, 1.0f, 0.86000001430511475f), kernelContext_2);
    return pow(saturate(_S16 * mix(float3(1.0f) , float3(1.0299999713897705f, 1.0f, 0.94999998807907104f), float3(smoothstep(0.34999999403953552f, 0.89999997615814209f, dot(_S16, float3(0.2125999927520752f, 0.71520000696182251f, 0.072200000286102295f)))) )), float3(0.45454543828964233f) );
}

float3 srgb_decode_0(float3 c_0)
{
    return select(pow((c_0 + float3(0.054999999701976776f) ) / float3(1.0549999475479126f) , float3(2.4000000953674316f) ), c_0 / float3(12.920000076293945f) , c_0 <= (float3(0.040449999272823334f) ));
}

struct pixelOutput_0
{
    float4 output_0 [[color(0)]];
};

[[fragment]] pixelOutput_0 fs_tonemap(float4 pos_1 [[position]], ToneC_0 constant* ed_c_1 [[buffer(0)]], ulong device* ed_textures_1 [[buffer(1)]], ulong device* ed_buffers_1 [[buffer(2)]])
{
    thread KernelContext_0 kernelContext_3;
    (&kernelContext_3)->ed_c_0 = ed_c_1;
    (&kernelContext_3)->ed_textures_0 = ed_textures_1;
    (&kernelContext_3)->ed_buffers_0 = ed_buffers_1;
    uint2 _S17 = uint2(max(pos_1.xy - ed_c_1->viewport_0.xy, float2(0.0f) ));
    float3 hdr_2;
    if((ed_c_1->ramp_0) != 0U)
    {
        float3 _S18 = test_ramp_0(_S17, ed_c_1->viewport_0.zw);
        hdr_2 = _S18;
    }
    else
    {
        texture2d<float, access::sample> _S19 = ((reinterpret_cast<device texture2d<float, access::sample>*>(((&kernelContext_3)->ed_textures_0))[((&kernelContext_3)->ed_c_0->hdr_0)]));
        int3 _S20 = int3(int2(_S17), int(0));
        hdr_2 = ((_S19).read(vec<uint,2>(((_S20)).xy), uint(((_S20)).z))).xyz;
    }
    float3 _S21 = alpha_tonemap_0((&kernelContext_3)->ed_c_0->lut_0, hdr_2, (&kernelContext_3)->ed_c_0->exposure_0, &kernelContext_3);
    float3 c_1;
    if(((&kernelContext_3)->ed_c_0->encode_srgb_0) == 0U)
    {
        c_1 = srgb_decode_0(_S21);
    }
    else
    {
        c_1 = _S21;
    }
    pixelOutput_0 _S22 = { float4(c_1, 1.0f) };
    return _S22;
}

struct vs_fullscreen_Result_0
{
    float4 pos_2 [[position]];
};

[[vertex]] vs_fullscreen_Result_0 vs_fullscreen(uint vid_1 [[vertex_id]], ToneC_0 constant* ed_c_2 [[buffer(0)]], ulong device* ed_textures_2 [[buffer(1)]], ulong device* ed_buffers_2 [[buffer(2)]])
{
    thread KernelContext_0 kernelContext_4;
    (&kernelContext_4)->ed_c_0 = ed_c_2;
    (&kernelContext_4)->ed_textures_0 = ed_textures_2;
    (&kernelContext_4)->ed_buffers_0 = ed_buffers_2;
    thread vs_fullscreen_Result_0 _S23;
    (&_S23)->pos_2 = fullscreen_vertex_0(vid_1).pos_0;
    return _S23;
}

