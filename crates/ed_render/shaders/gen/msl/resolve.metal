#include <metal_stdlib>
#include <metal_math>
#include <metal_texture>
using namespace metal;
constant array<float3, int(5)> TINT_0 = { { float3(1.0f, 0.25f, 0.25f), float3(0.25f, 1.0f, 0.25f), float3(0.30000001192092896f, 0.44999998807907104f, 1.0f), float3(1.0f, 1.0f, 0.25f), float3(0.60000002384185791f, 0.60000002384185791f, 0.60000002384185791f) } };
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
    array<uint, int(4)> pad_0;
};

struct KernelContext_0
{
    ResolveC_0 constant* ed_c_0;
    ulong device* ed_buffers_0;
    ulong device* ed_textures_0;
    ulong device* ed_samplers_0;
};

packed_float2 device* ed_buf_0(uint i_0, KernelContext_0 thread* kernelContext_0)
{
    return (packed_float2 device*)kernelContext_0->ed_buffers_0[i_0];
}

struct GpuMaterial_natural_0
{
    packed_float4 base_0;
    packed_float4 params_0;
    packed_float4 shade_0;
    packed_uint4 tex_0;
    packed_float4 uv_0;
};

GpuMaterial_natural_0 device* ed_buf_1(uint i_1, KernelContext_0 thread* kernelContext_1)
{
    return (GpuMaterial_natural_0 device*)kernelContext_1->ed_buffers_0[i_1];
}

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

uint device* ed_buf_4(uint i_4, KernelContext_0 thread* kernelContext_4)
{
    return (uint device*)kernelContext_4->ed_buffers_0[i_4];
}

struct GpuMesh_natural_0
{
    packed_float4 sphere_0;
    uint lod_count_0;
    uint bin_base_0;
    uint capacity_0;
    uint pad_1;
    array<uint, int(4)> first_index_0;
    array<uint, int(4)> index_count_0;
    array<int, int(4)> base_vertex_0;
    array<float, int(4)> error_0;
};

GpuMesh_natural_0 device* ed_buf_5(uint i_5, KernelContext_0 thread* kernelContext_5)
{
    return (GpuMesh_natural_0 device*)kernelContext_5->ed_buffers_0[i_5];
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
    array<uint, int(4)> pad_2;
};

GpuInstance_natural_0 device* ed_buf_6(uint i_6, KernelContext_0 thread* kernelContext_6)
{
    return (GpuInstance_natural_0 device*)kernelContext_6->ed_buffers_0[i_6];
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
    uint pad_3;
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
    _Array_natural_matrixx3Cfloatx2C4x2C4x3E4_0 shadow_1;
    _Array_natural_vectorx3Cfloatx2C4x3E4_0 cascade_0;
    packed_float4 split_0;
    packed_float4 cfg_0;
    packed_float4 filter_0;
    packed_uint4 flags_0;
    packed_float4 ao_1;
    packed_float4 sky_0;
    _Array_natural_vectorx3Cfloatx2C4x3E9_0 sh_0;
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
};

FrameData_natural_0 device* ed_buf_7(uint i_7, KernelContext_0 thread* kernelContext_7)
{
    return (FrameData_natural_0 device*)kernelContext_7->ed_buffers_0[i_7];
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

float3 view_dir_0(FrameData_natural_0 device* f_0, float2 pixel_0)
{
    float2 uv_1 = pixel_0 / (float4(f_0->viewport_0) ).zw;
    float4 p_0 = (((float4(float2(uv_1.x * 2.0f - 1.0f, 1.0f - uv_1.y * 2.0f), 1.0f, 1.0f)) * (matrix<float,int(4),int(4)> (f_0->inv_view_proj_0.data_1[int(0)][int(0)], f_0->inv_view_proj_0.data_1[int(1)][int(0)], f_0->inv_view_proj_0.data_1[int(2)][int(0)], f_0->inv_view_proj_0.data_1[int(3)][int(0)], f_0->inv_view_proj_0.data_1[int(0)][int(1)], f_0->inv_view_proj_0.data_1[int(1)][int(1)], f_0->inv_view_proj_0.data_1[int(2)][int(1)], f_0->inv_view_proj_0.data_1[int(3)][int(1)], f_0->inv_view_proj_0.data_1[int(0)][int(2)], f_0->inv_view_proj_0.data_1[int(1)][int(2)], f_0->inv_view_proj_0.data_1[int(2)][int(2)], f_0->inv_view_proj_0.data_1[int(3)][int(2)], f_0->inv_view_proj_0.data_1[int(0)][int(3)], f_0->inv_view_proj_0.data_1[int(1)][int(3)], f_0->inv_view_proj_0.data_1[int(2)][int(3)], f_0->inv_view_proj_0.data_1[int(3)][int(3)]))));
    return normalize(p_0.xyz / float3(p_0.w) );
}

float3 sky_rad_0(FrameData_natural_0 device* f_1, float3 d_0, float disc_k_0)
{
    float3 sun_0 = (float4(f_1->sun_dir_0) ).xyz;
    float h_0 = d_0.y;
    float t_0 = pow(saturate(1.0f - max(h_0, 0.0f)), 6.5f);
    float mu_0 = dot(d_0, sun_0);
    float warm_0 = saturate(1.0f - sun_0.y * 2.5f);
    float3 c_0 = mix((float4(f_1->sky_zenith_0) ).xyz, (float4(f_1->sky_horizon_0) ).xyz, float3(t_0) ) + float3(1.0f, 0.41999998688697815f, 0.14000000059604645f) * float3(pow(saturate(mu_0 * 0.5f + 0.5f), 5.0f))  * float3(warm_0)  * float3(0.85000002384185791f)  * float3((0.40000000596046448f + t_0)) ;
    float3 _S1 = float3((float4(f_1->sun_dir_0) ).w) ;
    return mix(mix(c_0, c_0 * float3(1.0f, 0.69999998807907104f, 0.55000001192092896f), float3((warm_0 * 0.40000000596046448f)) ) + float3(1.0f, 0.89999997615814209f, 0.69999998807907104f) * float3(pow(saturate(mu_0), 28.0f))  * float3(0.34999999403953552f) , (float4(f_1->ground_color_0) ).xyz, float3(saturate(- h_0 * 6.0f)) ) * _S1 + float3(1.0f, 0.94999998807907104f, 0.85000002384185791f) * float3((smoothstep(0.99965000152587891f, 0.99984997510910034f, mu_0) * step(0.0f, h_0)))  * float3(60.0f)  * _S1 * float3(disc_k_0) ;
}

float3 xform_point_0(const GpuInstance_natural_0 thread* i_8, float3 p_1)
{
    float4 _S2 = float4((&i_8->m_0)->data_0[int(0)]) ;
    float4 _S3 = float4((&i_8->m_0)->data_0[int(1)]) ;
    float4 _S4 = float4((&i_8->m_0)->data_0[int(2)]) ;
    return float3(dot(_S2.xyz, p_1) + _S2.w, dot(_S3.xyz, p_1) + _S3.w, dot(_S4.xyz, p_1) + _S4.w);
}

float2 ray_bary_0(float3 dir_0, float3 p0_0, float3 e1_0, float3 e2_0)
{
    float3 pv_0 = cross(dir_0, e2_0);
    float det_0 = dot(e1_0, pv_0);
    if((abs(det_0)) <= 9.999999960041972e-13f)
    {
        return float2(0.3333333432674408f) ;
    }
    float inv_0 = 1.0f / det_0;
    float3 tv_0 = - p0_0;
    return float2(dot(tv_0, pv_0) * inv_0, dot(dir_0, cross(tv_0, e1_0)) * inv_0);
}

float2 unpack_snorm2x16_0(uint p_2)
{
    return clamp(float2(int2(int(p_2 << 16U) >> 16U, int(p_2) >> 16U)) / float2(32767.0f) , float2(-1.0f) , float2(1.0f) );
}

float3 oct_decode_0(uint packed_0)
{
    float2 e_0 = unpack_snorm2x16_0(packed_0);
    float _S5 = e_0.x;
    float _S6 = e_0.y;
    float _S7 = 1.0f - abs(_S5) - abs(_S6);
    thread float3 n_1 = float3(_S5, _S6, _S7);
    float t_1 = saturate(- _S7);
    float _S8;
    if(_S5 >= 0.0f)
    {
        _S8 = - t_1;
    }
    else
    {
        _S8 = t_1;
    }
    n_1.x = n_1.x + _S8;
    if((n_1.y) >= 0.0f)
    {
        _S8 = - t_1;
    }
    else
    {
        _S8 = t_1;
    }
    n_1.y = n_1.y + _S8;
    return normalize(n_1);
}

float3 xform_normal_0(const GpuInstance_natural_0 thread* i_9, float3 n_2)
{
    float4 _S9 = float4((&i_9->m_0)->data_0[int(0)]) ;
    float4 _S10 = float4((&i_9->m_0)->data_0[int(1)]) ;
    float4 _S11 = float4((&i_9->m_0)->data_0[int(2)]) ;
    float3 a0_0 = float3(_S9.x, _S10.x, _S11.x);
    float3 a1_0 = float3(_S9.y, _S10.y, _S11.y);
    float3 a2_0 = float3(_S9.z, _S10.z, _S11.z);
    return cross(a1_0, a2_0) * float3(n_2.x)  + cross(a2_0, a0_0) * float3(n_2.y)  + cross(a0_0, a1_0) * float3(n_2.z) ;
}

float2 uv_xform_0(const GpuMaterial_natural_0 thread* m_1, float2 uv_2)
{
    float r_0 = (as_type<float>(((uint4(m_1->tex_0) ).w)));
    float s_0 = sin(r_0);
    float c_1 = cos(r_0);
    float4 _S12 = float4(m_1->uv_0) ;
    float2 q_0 = uv_2 * _S12.zw;
    float _S13 = q_0.x;
    float _S14 = q_0.y;
    return float2(c_1 * _S13 + s_0 * _S14, - s_0 * _S13 + c_1 * _S14) + _S12.xy;
}

float length_squared_0(float3 v_0)
{
    return dot(v_0, v_0);
}

float cascade_extent_0(FrameData_natural_0 device* f_2, uint c_2, float3 p_3)
{
    float4 s_1 = (((float4(p_3, 1.0f)) * (matrix<float,int(4),int(4)> ((&(&f_2->light_0)->shadow_1)->data_3[c_2].data_1[int(0)][int(0)], (&(&f_2->light_0)->shadow_1)->data_3[c_2].data_1[int(1)][int(0)], (&(&f_2->light_0)->shadow_1)->data_3[c_2].data_1[int(2)][int(0)], (&(&f_2->light_0)->shadow_1)->data_3[c_2].data_1[int(3)][int(0)], (&(&f_2->light_0)->shadow_1)->data_3[c_2].data_1[int(0)][int(1)], (&(&f_2->light_0)->shadow_1)->data_3[c_2].data_1[int(1)][int(1)], (&(&f_2->light_0)->shadow_1)->data_3[c_2].data_1[int(2)][int(1)], (&(&f_2->light_0)->shadow_1)->data_3[c_2].data_1[int(3)][int(1)], (&(&f_2->light_0)->shadow_1)->data_3[c_2].data_1[int(0)][int(2)], (&(&f_2->light_0)->shadow_1)->data_3[c_2].data_1[int(1)][int(2)], (&(&f_2->light_0)->shadow_1)->data_3[c_2].data_1[int(2)][int(2)], (&(&f_2->light_0)->shadow_1)->data_3[c_2].data_1[int(3)][int(2)], (&(&f_2->light_0)->shadow_1)->data_3[c_2].data_1[int(0)][int(3)], (&(&f_2->light_0)->shadow_1)->data_3[c_2].data_1[int(1)][int(3)], (&(&f_2->light_0)->shadow_1)->data_3[c_2].data_1[int(2)][int(3)], (&(&f_2->light_0)->shadow_1)->data_3[c_2].data_1[int(3)][int(3)]))));
    float _S15 = s_1.z;
    bool _S16;
    if(_S15 < 0.0f)
    {
        _S16 = true;
    }
    else
    {
        _S16 = _S15 > 1.0f;
    }
    float _S17;
    if(_S16)
    {
        _S17 = 1.0e+09f;
    }
    else
    {
        _S17 = max(abs(s_1.x), abs(s_1.y));
    }
    return _S17;
}

float pcf_radius_texels_0(FrameData_natural_0 device* f_3, float texel_0)
{
    return clamp((float4((&f_3->light_0)->sky_0) ).z / texel_0, 1.0f, max((float4((&f_3->light_0)->sky_0) ).w, 1.0f));
}

float ed_depth_read_0(uint i_10, uint2 p_4, KernelContext_0 thread* kernelContext_8)
{
    float _S18 = ((reinterpret_cast<device depth2d<float, access::sample>*>((kernelContext_8->ed_textures_0))[(i_10)].read((p_4))));
    return _S18;
}

float cascade_lookup_0(FrameData_natural_0 device* f_4, uint atlas_0, uint c_3, float3 p_5, float3 ng_0, float2 px_1, KernelContext_0 thread* kernelContext_9)
{
    float4 _S19 = float4((&(&f_4->light_0)->cascade_0)->data_4[c_3]) ;
    float tile_0 = (float4((&f_4->light_0)->cfg_0) ).y;
    float _S20 = _S19.x;
    float _S21 = pcf_radius_texels_0(f_4, _S20);
    float sp_0 = _S21 * 0.5f;
    float nol_0 = saturate(dot(ng_0, (float4(f_4->sun_dir_0) ).xyz));
    float4 s_2 = (((float4(p_5 + ng_0 * float3((_S20 * ((float4((&f_4->light_0)->filter_0) ).z + 2.0f * sqrt(1.0f - nol_0 * nol_0)))) , 1.0f)) * (matrix<float,int(4),int(4)> ((&(&f_4->light_0)->shadow_1)->data_3[c_3].data_1[int(0)][int(0)], (&(&f_4->light_0)->shadow_1)->data_3[c_3].data_1[int(1)][int(0)], (&(&f_4->light_0)->shadow_1)->data_3[c_3].data_1[int(2)][int(0)], (&(&f_4->light_0)->shadow_1)->data_3[c_3].data_1[int(3)][int(0)], (&(&f_4->light_0)->shadow_1)->data_3[c_3].data_1[int(0)][int(1)], (&(&f_4->light_0)->shadow_1)->data_3[c_3].data_1[int(1)][int(1)], (&(&f_4->light_0)->shadow_1)->data_3[c_3].data_1[int(2)][int(1)], (&(&f_4->light_0)->shadow_1)->data_3[c_3].data_1[int(3)][int(1)], (&(&f_4->light_0)->shadow_1)->data_3[c_3].data_1[int(0)][int(2)], (&(&f_4->light_0)->shadow_1)->data_3[c_3].data_1[int(1)][int(2)], (&(&f_4->light_0)->shadow_1)->data_3[c_3].data_1[int(2)][int(2)], (&(&f_4->light_0)->shadow_1)->data_3[c_3].data_1[int(3)][int(2)], (&(&f_4->light_0)->shadow_1)->data_3[c_3].data_1[int(0)][int(3)], (&(&f_4->light_0)->shadow_1)->data_3[c_3].data_1[int(1)][int(3)], (&(&f_4->light_0)->shadow_1)->data_3[c_3].data_1[int(2)][int(3)], (&(&f_4->light_0)->shadow_1)->data_3[c_3].data_1[int(3)][int(3)]))));
    float2 _S22 = float2(s_2.x * 0.5f + 0.5f, 0.5f - s_2.y * 0.5f) * float2(tile_0) ;
    float ign_0 = fract(52.982917785644531f * fract(dot(px_1, float2(0.067110560834407806f, 0.0058371499180793762f))));
    float2 _S23 = float2(0.5f) ;
    float2 _S24 = (float2(ign_0, fract(ign_0 * 7.309999942779541f + 0.37000000476837158f)) - _S23) * float2(0.0f) ;
    float3 ns_0 = (((float4(ng_0, 0.0f)) * (matrix<float,int(4),int(4)> ((&(&f_4->light_0)->shadow_1)->data_3[c_3].data_1[int(0)][int(0)], (&(&f_4->light_0)->shadow_1)->data_3[c_3].data_1[int(1)][int(0)], (&(&f_4->light_0)->shadow_1)->data_3[c_3].data_1[int(2)][int(0)], (&(&f_4->light_0)->shadow_1)->data_3[c_3].data_1[int(3)][int(0)], (&(&f_4->light_0)->shadow_1)->data_3[c_3].data_1[int(0)][int(1)], (&(&f_4->light_0)->shadow_1)->data_3[c_3].data_1[int(1)][int(1)], (&(&f_4->light_0)->shadow_1)->data_3[c_3].data_1[int(2)][int(1)], (&(&f_4->light_0)->shadow_1)->data_3[c_3].data_1[int(3)][int(1)], (&(&f_4->light_0)->shadow_1)->data_3[c_3].data_1[int(0)][int(2)], (&(&f_4->light_0)->shadow_1)->data_3[c_3].data_1[int(1)][int(2)], (&(&f_4->light_0)->shadow_1)->data_3[c_3].data_1[int(2)][int(2)], (&(&f_4->light_0)->shadow_1)->data_3[c_3].data_1[int(3)][int(2)], (&(&f_4->light_0)->shadow_1)->data_3[c_3].data_1[int(0)][int(3)], (&(&f_4->light_0)->shadow_1)->data_3[c_3].data_1[int(1)][int(3)], (&(&f_4->light_0)->shadow_1)->data_3[c_3].data_1[int(2)][int(3)], (&(&f_4->light_0)->shadow_1)->data_3[c_3].data_1[int(3)][int(3)])))).xyz;
    float rl_0 = _S20 * tile_0 * 0.5f;
    float _S25 = _S19.y;
    float _S26 = ns_0.z * _S25;
    float izn_0 = 1.0f / max(abs(_S26), 0.11999999731779099f);
    float _S27 = float((int(sign((_S26)))));
    float gx_0 = clamp(- (ns_0.x * rl_0) * izn_0 * _S27, -8.0f, 8.0f);
    float gy_0 = clamp(- (ns_0.y * rl_0) * izn_0 * _S27, -8.0f, 8.0f);
    float _S28 = s_2.z + ((float4((&f_4->light_0)->filter_0) ).w + 0.5f * (abs(gx_0) + abs(gy_0))) * _S20 / _S25;
    float _S29 = _S20 / _S25;
    int2 _S30 = int2(_S19.zw);
    int2 _S31 = _S30 + int2((int(tile_0) - int(1))) ;
    float _S32 = 1.0f / (_S21 + sp_0);
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
        float _S33 = 1.0f - abs(oy_0) * _S32;
        int x_0 = int(-2);
        for(;;)
        {
            if(x_0 <= int(2))
            {
            }
            else
            {
                break;
            }
            float ox_0 = float(x_0) * sp_0;
            float w_0 = (1.0f - abs(ox_0) * _S32) * _S33;
            float2 tp_0 = floor(_S22 + float2(ox_0, oy_0) + _S24);
            float2 d_1 = tp_0 + _S23 - _S22;
            float _S34 = ed_depth_read_0(atlas_0, uint2(clamp(_S30 + int2(tp_0), _S30, _S31)), kernelContext_9);
            float _S35;
            if((_S28 + (gx_0 * d_1.x - gy_0 * d_1.y) * _S29) >= _S34)
            {
                _S35 = 1.0f;
            }
            else
            {
                _S35 = 0.0f;
            }
            float lit_1 = lit_0 + _S35 * w_0;
            float wsum_1 = wsum_0 + w_0;
            x_0 = x_0 + int(1);
            lit_0 = lit_1;
            wsum_0 = wsum_1;
        }
        y_0 = y_0 + int(1);
    }
    return lit_0 / wsum_0;
}

float sun_shadow_0(FrameData_natural_0 device* f_5, uint atlas_1, float3 p_6, float3 ng_1, float dist_0, float2 px_2, KernelContext_0 thread* kernelContext_10)
{
    uint n_3 = uint((float4((&f_5->light_0)->cfg_0) ).x);
    bool _S36;
    if(((uint4((&f_5->light_0)->flags_0) ).x) == 0U)
    {
        _S36 = true;
    }
    else
    {
        _S36 = n_3 == 0U;
    }
    if(_S36)
    {
        _S36 = true;
    }
    else
    {
        _S36 = dist_0 >= ((float4((&f_5->light_0)->filter_0) ).y);
    }
    if(_S36)
    {
        return 1.0f;
    }
    float _S37 = 1.0f - (float4((&f_5->light_0)->cfg_0) ).w * 2.0f / (float4((&f_5->light_0)->cfg_0) ).y;
    float _S38 = (float4((&f_5->light_0)->cfg_0) ).z;
    uint c_4 = 0U;
    for(;;)
    {
        if(c_4 < n_3)
        {
        }
        else
        {
            break;
        }
        float _S39 = cascade_extent_0(f_5, c_4, p_6);
        float e_1 = _S39 / _S37;
        if(e_1 > 1.0f)
        {
            c_4 = c_4 + 1U;
            continue;
        }
        float _S40 = cascade_lookup_0(f_5, atlas_1, c_4, p_6, ng_1, px_2, kernelContext_10);
        float blend_0 = smoothstep(1.0f - _S38, 1.0f, e_1);
        float v_1;
        if(blend_0 > 0.0f)
        {
            uint _S41 = c_4 + 1U;
            if(_S41 < n_3)
            {
                float _S42 = cascade_extent_0(f_5, _S41, p_6);
                _S36 = (_S42 / _S37) <= 1.0f;
            }
            else
            {
                _S36 = false;
            }
            if(_S36)
            {
                float _S43 = cascade_lookup_0(f_5, atlas_1, _S41, p_6, ng_1, px_2, kernelContext_10);
                v_1 = _S43;
            }
            else
            {
                v_1 = 1.0f;
            }
            v_1 = mix(_S40, v_1, blend_0);
        }
        else
        {
            v_1 = _S40;
        }
        return mix(v_1, 1.0f, smoothstep(0.0f, 1.0f, saturate((dist_0 - (float4((&f_5->light_0)->filter_0) ).x) / max((float4((&f_5->light_0)->filter_0) ).y - (float4((&f_5->light_0)->filter_0) ).x, 0.0010000000474974513f))));
    }
    return 1.0f;
}

float ao_upsample_0(FrameData_natural_0 device* f_6, uint ao_buf_0, float2 pixel_1, float zlin_0, KernelContext_0 thread* kernelContext_11)
{
    float ao_2;
    if(((uint4((&f_6->light_0)->flags_0) ).y) == 0U)
    {
        return 1.0f;
    }
    packed_float2 device* _S44 = ed_buf_0(ao_buf_0, kernelContext_11);
    uint _S45 = uint((float4((&f_6->light_0)->ao_1) ).y);
    uint _S46 = uint((float4((&f_6->light_0)->ao_1) ).z);
    float2 _S47 = float2(0.5f) ;
    float2 h_1 = (pixel_1 - _S47) * _S47;
    float2 b_0 = floor(h_1);
    float2 _S48 = h_1 - b_0;
    int y_1 = int(0);
    float sum_0 = 0.0f;
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
        int x_1 = int(0);
        for(;;)
        {
            if(x_1 < int(2))
            {
            }
            else
            {
                break;
            }
            int2 t_2 = clamp(int2(b_0) + int2(x_1, y_1), int2(int(0)) , int2(int(_S45) - int(1), int(_S46) - int(1)));
            float2 _S49 = float2(*(_S44 + (uint(t_2.y) * _S45 + uint(t_2.x)))) ;
            if(x_1 == int(0))
            {
                ao_2 = 1.0f - _S48.x;
            }
            else
            {
                ao_2 = _S48.x;
            }
            float _S50;
            if(y_1 == int(0))
            {
                _S50 = 1.0f - _S48.y;
            }
            else
            {
                _S50 = _S48.y;
            }
            float w_1 = max(ao_2 * _S50, 0.0010000000474974513f) * (1.0f / (0.0010000000474974513f + abs(_S49.y - zlin_0) / max(zlin_0, 0.0010000000474974513f) * 50.0f));
            float sum_1 = sum_0 + _S49.x * w_1;
            float wsum_3 = wsum_2 + w_1;
            x_1 = x_1 + int(1);
            sum_0 = sum_1;
            wsum_2 = wsum_3;
        }
        y_1 = y_1 + int(1);
    }
    if(wsum_2 > 0.0f)
    {
        ao_2 = sum_0 / wsum_2;
    }
    else
    {
        ao_2 = 1.0f;
    }
    return pow(saturate(ao_2), (float4((&f_6->light_0)->ao_1) ).x);
}

float3 sh_irradiance_0(FrameData_natural_0 device* f_7, float3 n_4)
{
    float _S51 = n_4.y;
    float _S52 = n_4.z;
    float _S53 = n_4.x;
    return max((float4((&(&f_7->light_0)->sh_0)->data_5[int(0)]) ).xyz + ((float4((&(&f_7->light_0)->sh_0)->data_5[int(1)]) ).xyz * float3(_S51)  + (float4((&(&f_7->light_0)->sh_0)->data_5[int(2)]) ).xyz * float3(_S52)  + (float4((&(&f_7->light_0)->sh_0)->data_5[int(3)]) ).xyz * float3(_S53) ) + ((float4((&(&f_7->light_0)->sh_0)->data_5[int(4)]) ).xyz * float3((_S53 * _S51))  + (float4((&(&f_7->light_0)->sh_0)->data_5[int(5)]) ).xyz * float3((_S51 * _S52))  + (float4((&(&f_7->light_0)->sh_0)->data_5[int(6)]) ).xyz * float3((3.0f * _S52 * _S52 - 1.0f)) ) + ((float4((&(&f_7->light_0)->sh_0)->data_5[int(7)]) ).xyz * float3((_S53 * _S52))  + (float4((&(&f_7->light_0)->sh_0)->data_5[int(8)]) ).xyz * float3((_S53 * _S53 - _S51 * _S51)) ), float3(0.0f) );
}

float3 hemi_ambient_0(FrameData_natural_0 device* f_8, float3 n_5)
{
    return mix((float4(f_8->ground_color_0) ).xyz * float3((float4(f_8->sun_dir_0) ).w) , mix((float4(f_8->sky_horizon_0) ).xyz, (float4(f_8->sky_zenith_0) ).xyz, float3(0.5f) ) * float3((float4(f_8->sun_dir_0) ).w) , float3((n_5.y * 0.5f + 0.5f)) );
}

float3 ambient_light_0(FrameData_natural_0 device* f_9, float3 n_6)
{
    float3 _S54;
    if(((uint4((&f_9->light_0)->flags_0) ).z) != 0U)
    {
        float3 _S55 = sh_irradiance_0(f_9, n_6);
        _S54 = _S55;
    }
    else
    {
        float3 _S56 = hemi_ambient_0(f_9, n_6);
        _S54 = _S56;
    }
    return _S54;
}

float3 ao_multibounce_0(FrameData_natural_0 device* f_10, float ao_3, float3 albedo_0)
{
    float3 _S57 = float3(ao_3) ;
    return mix(_S57, max(_S57, ((_S57 * (float3(2.0404000282287598f)  * albedo_0 - float3(0.33239999413490295f) ) + (float3(-4.795100212097168f)  * albedo_0 + float3(0.64170002937316895f) )) * _S57 + (float3(2.7551999092102051f)  * albedo_0 + float3(0.69029998779296875f) )) * _S57), float3((float4((&f_10->light_0)->sky_0) ).y) );
}

float3 shade_toon_0(FrameData_natural_0 device* f_11, const GpuMaterial_natural_0 thread* m_2, float3 albedo_1, float3 n_7, float3 v_2, float ao_4, float sun_vis_0)
{
    float ndl_0 = dot(n_7, (float4(f_11->sun_dir_0) ).xyz);
    float _S58 = max((float4(m_2->params_0) ).z, 1.0f);
    float lit_2 = floor(saturate(ndl_0 * 0.5f + 0.5f) * _S58 + 0.5f) / _S58 * sun_vis_0;
    float4 _S59 = float4(m_2->shade_0) ;
    float3 c_5 = mix(albedo_1 * _S59.xyz, albedo_1, float3(lit_2) );
    float3 _S60 = (float4(f_11->sun_color_0) ).xyz * float3((lit_2 / 3.1415927410125732f)) ;
    float3 _S61 = ambient_light_0(f_11, n_7);
    float3 _S62 = ao_multibounce_0(f_11, ao_4, albedo_1);
    return c_5 * (_S60 + _S61 * _S62) + float3((pow(1.0f - saturate(dot(n_7, v_2)), 4.0f) * _S59.w * saturate(ndl_0 + 0.30000001192092896f) * sun_vis_0))  * (float4(f_11->sun_color_0) ).xyz * float3(0.15000000596046448f) ;
}

float3 f_schlick_0(float3 f0_0, float voh_0)
{
    return f0_0 + (float3(1.0f)  - f0_0) * float3(pow(1.0f - voh_0, 5.0f)) ;
}

float d_ggx_0(float noh_0, float a_0)
{
    float a2_1 = a_0 * a_0;
    float d_2 = noh_0 * noh_0 * (a2_1 - 1.0f) + 1.0f;
    return a2_1 / (3.1415927410125732f * d_2 * d_2);
}

float v_smith_0(float nov_0, float nol_1, float a_1)
{
    float a2_2 = a_1 * a_1;
    float _S63 = 1.0f - a2_2;
    return 0.5f / max(nol_1 * sqrt(nov_0 * nov_0 * _S63 + a2_2) + nov_0 * sqrt(nol_1 * nol_1 * _S63 + a2_2), 9.9999997473787516e-06f);
}

float2 env_brdf_0(float nov_1, float rough_0)
{
    float4 r_1 = float4(rough_0)  * float4(-1.0f, -0.027499999850988388f, -0.57200002670288086f, 0.02199999988079071f) + float4(1.0f, 0.042500000447034836f, 1.0399999618530273f, -0.039999999105930328f);
    float _S64 = r_1.x;
    return float2(-1.0399999618530273f, 1.0399999618530273f) * float2((min(_S64 * _S64, exp2(-9.2799997329711914f * nov_1)) * _S64 + r_1.y))  + r_1.zw;
}

float3 shade_pbr_0(FrameData_natural_0 device* f_12, float3 albedo_2, float metal_0, float rough_1, float ao_5, float sun_vis_1, float3 n_8, float3 v_3)
{
    float _S65 = saturate(metal_0);
    float _S66 = clamp(rough_1, 0.039999999105930328f, 1.0f);
    float a_2 = _S66 * _S66;
    float3 l_0 = (float4(f_12->sun_dir_0) ).xyz;
    float3 h_2 = normalize(l_0 + v_3);
    float nol_2 = saturate(dot(n_8, l_0));
    float _S67 = max(dot(n_8, v_3), 9.9999997473787516e-05f);
    float3 f0_1 = mix(float3(0.039999999105930328f) , albedo_2, float3(_S65) );
    float3 diff_col_0 = albedo_2 * float3((1.0f - _S65)) ;
    float3 F_0 = f_schlick_0(f0_1, saturate(dot(v_3, h_2)));
    float3 _S68 = float3(1.0f) ;
    float3 direct_0 = (diff_col_0 * (_S68 - F_0) / float3(3.1415927410125732f)  + float3((d_ggx_0(saturate(dot(n_8, h_2)), a_2) * v_smith_0(_S67, nol_2, a_2)))  * F_0) * (float4(f_12->sun_color_0) ).xyz * float3((nol_2 * sun_vis_1)) ;
    float2 ab_0 = env_brdf_0(_S67, _S66);
    float3 r_2 = reflect(- v_3, n_8);
    float3 env_0;
    if(((uint4((&f_12->light_0)->flags_0) ).z) != 0U)
    {
        float3 _S69 = sky_rad_0(f_12, r_2, 0.0f);
        float3 _S70 = sh_irradiance_0(f_12, r_2);
        env_0 = mix(_S69, _S70, float3(_S66) );
    }
    else
    {
        float3 _S71 = hemi_ambient_0(f_12, r_2);
        env_0 = _S71;
    }
    float spec_occ_0 = saturate(pow(_S67 + ao_5, exp2(-16.0f * a_2 - 1.0f)) - 1.0f + ao_5);
    float3 f_env_0 = f0_1 * float3(ab_0.x)  + float3(ab_0.y) ;
    float3 _S72 = diff_col_0 * (_S68 - f_env_0);
    float3 _S73 = ambient_light_0(f_12, n_8);
    float3 _S74 = _S72 * _S73;
    float3 _S75 = ao_multibounce_0(f_12, ao_5, diff_col_0);
    return direct_0 + (_S74 * _S75 + env_0 * f_env_0 * float3(spec_occ_0) );
}

uint shadow_cascade_0(FrameData_natural_0 device* f_13, float3 p_7, float dist_1)
{
    uint _S76 = uint((float4((&f_13->light_0)->cfg_0) ).x);
    bool _S77;
    if(((uint4((&f_13->light_0)->flags_0) ).x) == 0U)
    {
        _S77 = true;
    }
    else
    {
        _S77 = dist_1 >= ((float4((&f_13->light_0)->filter_0) ).y);
    }
    if(_S77)
    {
        return 4U;
    }
    float _S78 = 1.0f - (float4((&f_13->light_0)->cfg_0) ).w * 2.0f / (float4((&f_13->light_0)->cfg_0) ).y;
    uint c_6 = 0U;
    for(;;)
    {
        if(c_6 < _S76)
        {
        }
        else
        {
            break;
        }
        float _S79 = cascade_extent_0(f_13, c_6, p_7);
        if((_S79 / _S78) <= 1.0f)
        {
            return c_6;
        }
        c_6 = c_6 + 1U;
    }
    return 4U;
}

float cascade_depth_delta_0(FrameData_natural_0 device* f_14, uint atlas_2, uint c_7, float3 p_8, KernelContext_0 thread* kernelContext_12)
{
    float4 _S80 = float4((&(&f_14->light_0)->cascade_0)->data_4[c_7]) ;
    float tile_1 = (float4((&f_14->light_0)->cfg_0) ).y;
    float4 s_3 = (((float4(p_8, 1.0f)) * (matrix<float,int(4),int(4)> ((&(&f_14->light_0)->shadow_1)->data_3[c_7].data_1[int(0)][int(0)], (&(&f_14->light_0)->shadow_1)->data_3[c_7].data_1[int(1)][int(0)], (&(&f_14->light_0)->shadow_1)->data_3[c_7].data_1[int(2)][int(0)], (&(&f_14->light_0)->shadow_1)->data_3[c_7].data_1[int(3)][int(0)], (&(&f_14->light_0)->shadow_1)->data_3[c_7].data_1[int(0)][int(1)], (&(&f_14->light_0)->shadow_1)->data_3[c_7].data_1[int(1)][int(1)], (&(&f_14->light_0)->shadow_1)->data_3[c_7].data_1[int(2)][int(1)], (&(&f_14->light_0)->shadow_1)->data_3[c_7].data_1[int(3)][int(1)], (&(&f_14->light_0)->shadow_1)->data_3[c_7].data_1[int(0)][int(2)], (&(&f_14->light_0)->shadow_1)->data_3[c_7].data_1[int(1)][int(2)], (&(&f_14->light_0)->shadow_1)->data_3[c_7].data_1[int(2)][int(2)], (&(&f_14->light_0)->shadow_1)->data_3[c_7].data_1[int(3)][int(2)], (&(&f_14->light_0)->shadow_1)->data_3[c_7].data_1[int(0)][int(3)], (&(&f_14->light_0)->shadow_1)->data_3[c_7].data_1[int(1)][int(3)], (&(&f_14->light_0)->shadow_1)->data_3[c_7].data_1[int(2)][int(3)], (&(&f_14->light_0)->shadow_1)->data_3[c_7].data_1[int(3)][int(3)]))));
    int2 _S81 = int2(_S80.zw);
    float _S82 = s_3.z;
    float _S83 = ed_depth_read_0(atlas_2, uint2(clamp(_S81 + int2(floor(float2(s_3.x * 0.5f + 0.5f, 0.5f - s_3.y * 0.5f) * float2(tile_1) )), _S81, _S81 + int2((int(tile_1) - int(1))) )), kernelContext_12);
    return (_S82 - _S83) * _S80.y;
}

struct pixelOutput_0
{
    float4 output_0 [[color(0)]];
};

[[fragment]] pixelOutput_0 fs_resolve(float4 pos_1 [[position]], ResolveC_0 constant* ed_c_1 [[buffer(0)]], ulong device* ed_buffers_1 [[buffer(2)]], ulong device* ed_textures_1 [[buffer(1)]], ulong device* ed_samplers_1 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_13;
    (&kernelContext_13)->ed_c_0 = ed_c_1;
    (&kernelContext_13)->ed_buffers_0 = ed_buffers_1;
    (&kernelContext_13)->ed_textures_0 = ed_textures_1;
    (&kernelContext_13)->ed_samplers_0 = ed_samplers_1;
    FrameData_natural_0 device* _S84 = ed_buf_7(ed_c_1->frame_0, &kernelContext_13);
    float2 _S85 = pos_1.xy;
    uint2 _S86 = uint2(_S85);
    texture2d<uint, access::sample> _S87 = ((reinterpret_cast<device texture2d<uint, access::sample>*>(((&kernelContext_13)->ed_textures_0))[((&kernelContext_13)->ed_c_0->vis_0)]));
    int3 _S88 = int3(int2(_S86), int(0));
    uint2 vis_1 = ((_S87).read(vec<uint,2>(((_S88)).xy), uint(((_S88)).z))).xy;
    float3 _S89 = view_dir_0(_S84, _S85);
    uint _S90 = vis_1.x;
    if(_S90 == 0U)
    {
        float3 _S91 = sky_rad_0(_S84, _S89, (float4((&_S84->light_0)->sky_0) ).x);
        pixelOutput_0 _S92 = { float4(_S91, 1.0f) };
        return _S92;
    }
    GpuInstance_natural_0 device* _S93 = ed_buf_6((&kernelContext_13)->ed_c_0->instances_0, &kernelContext_13);
    thread GpuInstance_natural_0 _S94 = *(_S93 + (_S90 - 1U));
    GpuMesh_natural_0 device* _S95 = ed_buf_5((&kernelContext_13)->ed_c_0->meshes_0, &kernelContext_13);
    GpuMesh_natural_0 device* _S96 = _S95 + (&_S94)->mesh_0;
    uint _S97 = vis_1.y;
    uint lod_0 = _S97 >> 28U;
    uint prim_0 = (_S97 & 268435455U) % max(_S96->index_count_0[lod_0] / 3U, 1U);
    uint device* _S98 = ed_buf_4((&kernelContext_13)->ed_c_0->indices_0, &kernelContext_13);
    uint device* idx_0 = _S98 + _S96->first_index_0[lod_0] + prim_0 * 3U;
    Vtx_0 device* _S99 = ed_buf_3((&kernelContext_13)->ed_c_0->verts_0, &kernelContext_13);
    int i0_0 = int(*(idx_0 + int(0))) + _S96->base_vertex_0[lod_0];
    int i1_0 = int(*(idx_0 + int(1))) + _S96->base_vertex_0[lod_0];
    int i2_0 = int(*(idx_0 + int(2))) + _S96->base_vertex_0[lod_0];
    Vtx_0 v0_0 = *(_S99 + i0_0);
    Vtx_0 v1_0 = *(_S99 + i1_0);
    Vtx_0 v2_0 = *(_S99 + i2_0);
    packed_float4 device* _S100 = ed_buf_2((&kernelContext_13)->ed_c_0->nodes_0, &kernelContext_13);
    float3 node_0 = (float4(*(_S100 + (((&_S94)->node_flags_0) & 65535U))) ).xyz;
    float3 _S101 = xform_point_0(&_S94, float3(v0_0.px_0, v0_0.py_0, v0_0.pz_0));
    float3 _S102 = xform_point_0(&_S94, float3(v1_0.px_0, v1_0.py_0, v1_0.pz_0));
    float3 _S103 = xform_point_0(&_S94, float3(v2_0.px_0, v2_0.py_0, v2_0.pz_0));
    float3 p0_1 = node_0 + _S101;
    float3 e1_1 = _S102 - _S101;
    float3 e2_1 = _S103 - _S101;
    float2 bb_0 = ray_bary_0(_S89, p0_1, e1_1, e2_1);
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
    float3 _S104 = float3((1.0f - b1_1 - b2_1)) ;
    float3 local_0 = _S101 * _S104 + _S102 * float3(b1_1)  + _S103 * float3(b2_1) ;
    float3 _S105 = xform_normal_0(&_S94, oct_decode_0(v0_0.n_0) * _S104 + oct_decode_0(v1_0.n_0) * float3(b1_1)  + oct_decode_0(v2_0.n_0) * float3(b2_1) );
    float3 n_9 = normalize(_S105);
    float3 v_4 = - _S89;
    GpuMaterial_natural_0 device* _S106 = ed_buf_1((&kernelContext_13)->ed_c_0->materials_0, &kernelContext_13);
    thread GpuMaterial_natural_0 _S107 = *(_S106 + (&_S94)->material_0);
    float4 _S108 = float4((&_S107)->base_0) ;
    float3 albedo_3 = _S108.xyz;
    float4 _S109 = float4((&_S107)->params_0) ;
    float metal_1 = _S109.x;
    float rough_2 = _S109.y;
    uint4 _S110 = uint4((&_S107)->tex_0) ;
    uint _S111 = _S110.x;
    bool _S112 = _S111 != 4294967295U;
    bool textured_0;
    if(_S112)
    {
        textured_0 = true;
    }
    else
    {
        textured_0 = (_S110.y) != 4294967295U;
    }
    if(textured_0)
    {
        textured_0 = true;
    }
    else
    {
        textured_0 = (_S110.z) != 4294967295U;
    }
    float ao_6;
    float metal_2;
    float rough_3;
    float d_3;
    float3 albedo_4;
    float3 n_10;
    if(textured_0)
    {
        packed_float2 device* _S113 = ed_buf_0((&kernelContext_13)->ed_c_0->uvs_0, &kernelContext_13);
        float2 _S114 = float2(*(_S113 + i0_0)) ;
        float2 d1_0 = float2(*(_S113 + i1_0))  - _S114;
        float2 d2_0 = float2(*(_S113 + i2_0))  - _S114;
        float2 _S115 = uv_xform_0(&_S107, _S114 + d1_0 * float2(b1_1)  + d2_0 * float2(b2_1) );
        float3 _S116 = view_dir_0(_S84, _S85 + float2(1.0f, 0.0f));
        float2 bx_0 = ray_bary_0(_S116, p0_1, e1_1, e2_1);
        float3 _S117 = view_dir_0(_S84, _S85 + float2(0.0f, 1.0f));
        float2 by_0 = ray_bary_0(_S117, p0_1, e1_1, e2_1);
        float2 _S118 = uv_xform_0(&_S107, _S114 + d1_0 * float2(bx_0.x)  + d2_0 * float2(bx_0.y) );
        float2 ux_0 = _S118 - _S115;
        float2 _S119 = uv_xform_0(&_S107, _S114 + d1_0 * float2(by_0.x)  + d2_0 * float2(by_0.y) );
        float2 uy_0 = _S119 - _S115;
        sampler _S120 = ((reinterpret_cast<device sampler*>(((&kernelContext_13)->ed_samplers_0))[((&kernelContext_13)->ed_c_0->smp_0)]));
        if(_S112)
        {
            texture2d<float, access::sample> _S121 = ((reinterpret_cast<device texture2d<float, access::sample>*>(((&kernelContext_13)->ed_textures_0))[(_S111)]));
            albedo_4 = albedo_3 * ((_S121).sample((_S120), (_S115), gradient2d((ux_0), (uy_0)))).xyz;
        }
        else
        {
            albedo_4 = albedo_3;
        }
        uint _S122 = _S110.z;
        if(_S122 != 4294967295U)
        {
            texture2d<float, access::sample> _S123 = ((reinterpret_cast<device texture2d<float, access::sample>*>(((&kernelContext_13)->ed_textures_0))[(_S122)]));
            float3 orm_0 = ((_S123).sample((_S120), (_S115), gradient2d((ux_0), (uy_0)))).xyz;
            float rough_4 = rough_2 * orm_0.y;
            float metal_3 = metal_1 * orm_0.z;
            ao_6 = orm_0.x;
            metal_2 = metal_3;
            rough_3 = rough_4;
        }
        else
        {
            ao_6 = 1.0f;
            metal_2 = metal_1;
            rough_3 = rough_2;
        }
        uint _S124 = _S110.y;
        if(_S124 != 4294967295U)
        {
            float _S125 = d1_0.x;
            float _S126 = d2_0.y;
            float _S127 = d1_0.y;
            float _S128 = d2_0.x;
            float det_1 = _S125 * _S126 - _S127 * _S128;
            if((abs(det_1)) > 9.999999960041972e-13f)
            {
                float3 _S129 = float3((1.0f / det_1)) ;
                float3 tg_0 = (e1_1 * float3(_S126)  - e2_1 * float3(_S127) ) * _S129;
                float3 bt_0 = (e2_1 * float3(_S125)  - e1_1 * float3(_S128) ) * _S129;
                float3 tg_1 = tg_0 - n_9 * float3(dot(n_9, tg_0)) ;
                if((length_squared_0(tg_1)) > 9.99999968265522539e-21f)
                {
                    float3 tg_2 = normalize(tg_1);
                    float3 _S130 = cross(n_9, tg_2);
                    if((dot(_S130, bt_0)) < 0.0f)
                    {
                        d_3 = -1.0f;
                    }
                    else
                    {
                        d_3 = 1.0f;
                    }
                    float3 bn_0 = _S130 * float3(- d_3) ;
                    texture2d<float, access::sample> _S131 = ((reinterpret_cast<device texture2d<float, access::sample>*>(((&kernelContext_13)->ed_textures_0))[(_S124)]));
                    float2 xy_0 = ((_S131).sample((_S120), (_S115), gradient2d((ux_0), (uy_0)))).xy * float2(2.0f)  - float2(1.0f) ;
                    n_10 = normalize(tg_2 * float3(xy_0.x)  + bn_0 * float3(xy_0.y)  + n_9 * float3(sqrt(saturate(1.0f - dot(xy_0, xy_0)))) );
                }
                else
                {
                    n_10 = n_9;
                }
            }
            else
            {
                n_10 = n_9;
            }
        }
        else
        {
            n_10 = n_9;
        }
    }
    else
    {
        n_10 = n_9;
        albedo_4 = albedo_3;
        ao_6 = 1.0f;
        metal_2 = metal_1;
        rough_3 = rough_2;
    }
    float _S132 = dot(n_10, v_4);
    if(_S132 < 0.0f)
    {
        n_10 = normalize(n_10 - float3((1.0099999904632568f * _S132))  * v_4);
    }
    if((_S109.w) > 0.5f)
    {
        float2 c_8 = floor(local_0.xz * float2(0.5f) );
        albedo_4 = albedo_4 * float3(mix(0.80000001192092896f, 1.1499999761581421f, fmod(abs(c_8.x + c_8.y), 2.0f))) ;
    }
    float3 p_9 = node_0 + local_0;
    float zlin_1 = - (((float4(p_9, 1.0f)) * (matrix<float,int(4),int(4)> (_S84->view_0.data_1[int(0)][int(0)], _S84->view_0.data_1[int(1)][int(0)], _S84->view_0.data_1[int(2)][int(0)], _S84->view_0.data_1[int(3)][int(0)], _S84->view_0.data_1[int(0)][int(1)], _S84->view_0.data_1[int(1)][int(1)], _S84->view_0.data_1[int(2)][int(1)], _S84->view_0.data_1[int(3)][int(1)], _S84->view_0.data_1[int(0)][int(2)], _S84->view_0.data_1[int(1)][int(2)], _S84->view_0.data_1[int(2)][int(2)], _S84->view_0.data_1[int(3)][int(2)], _S84->view_0.data_1[int(0)][int(3)], _S84->view_0.data_1[int(1)][int(3)], _S84->view_0.data_1[int(2)][int(3)], _S84->view_0.data_1[int(3)][int(3)])))).z;
    float3 ng_2 = cross(e1_1, e2_1);
    float3 ng_3;
    if((length_squared_0(ng_2)) > 9.99999968265522539e-21f)
    {
        ng_3 = normalize(ng_2);
    }
    else
    {
        ng_3 = n_10;
    }
    if((dot(ng_3, v_4)) < 0.0f)
    {
        ng_3 = - ng_3;
    }
    float _S133 = sun_shadow_0(_S84, (&kernelContext_13)->ed_c_0->shadow_0, p_9, ng_3, zlin_1, _S85, &kernelContext_13);
    float _S134 = ao_upsample_0(_S84, (&kernelContext_13)->ed_c_0->ao_0, _S85, zlin_1, &kernelContext_13);
    float ao_7 = ao_6 * _S134;
    if((_S108.w) > 0.5f)
    {
        float3 _S135 = shade_toon_0(_S84, &_S107, albedo_4, n_10, v_4, ao_7, _S133);
        albedo_4 = _S135;
    }
    else
    {
        float3 _S136 = shade_pbr_0(_S84, albedo_4, metal_2, rough_3, ao_7, _S133, n_10, v_4);
        albedo_4 = _S136;
    }
    float3 col_0;
    if(((uint4((&_S84->light_0)->flags_0) ).w) == 1U)
    {
        uint _S137 = shadow_cascade_0(_S84, p_9, zlin_1);
        col_0 = TINT_0[_S137] * float3((0.039999999105930328f + 0.5f * _S133)) ;
    }
    else
    {
        if(((uint4((&_S84->light_0)->flags_0) ).w) == 3U)
        {
            uint _S138 = shadow_cascade_0(_S84, p_9, zlin_1);
            if(_S138 < 4U)
            {
                float _S139 = cascade_depth_delta_0(_S84, (&kernelContext_13)->ed_c_0->shadow_0, _S138, p_9, &kernelContext_13);
                d_3 = _S139;
            }
            else
            {
                d_3 = 0.0f;
            }
            col_0 = float3(max(d_3, 0.0f), max(- d_3, 0.0f), 0.0f) * float3(0.10000000149011612f) ;
        }
        else
        {
            if(((uint4((&_S84->light_0)->flags_0) ).w) == 2U)
            {
                col_0 = float3((0.5f * ao_7)) ;
            }
            else
            {
                col_0 = albedo_4;
            }
        }
    }
    pixelOutput_0 _S140 = { float4(col_0, 1.0f) };
    return _S140;
}

struct vs_resolve_Result_0
{
    float4 pos_2 [[position]];
};

[[vertex]] vs_resolve_Result_0 vs_resolve(uint vid_1 [[vertex_id]], ResolveC_0 constant* ed_c_2 [[buffer(0)]], ulong device* ed_buffers_2 [[buffer(2)]], ulong device* ed_textures_2 [[buffer(1)]], ulong device* ed_samplers_2 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_14;
    (&kernelContext_14)->ed_c_0 = ed_c_2;
    (&kernelContext_14)->ed_buffers_0 = ed_buffers_2;
    (&kernelContext_14)->ed_textures_0 = ed_textures_2;
    (&kernelContext_14)->ed_samplers_0 = ed_samplers_2;
    thread vs_resolve_Result_0 _S141;
    (&_S141)->pos_2 = fullscreen_vertex_0(vid_1).pos_0;
    return _S141;
}

