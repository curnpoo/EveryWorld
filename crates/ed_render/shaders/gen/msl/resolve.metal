#include <metal_stdlib>
#include <metal_math>
#include <metal_texture>
using namespace metal;
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
    array<uint, int(6)> pad_0;
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

float3 sky_radiance_0(FrameData_natural_0 device* f_1, float3 d_0)
{
    float _S1 = d_0.y;
    return mix(mix((float4(f_1->sky_horizon_0) ).xyz, (float4(f_1->sky_zenith_0) ).xyz, float3(pow(saturate(_S1), 0.44999998807907104f)) ), (float4(f_1->ground_color_0) ).xyz, float3(saturate(- _S1 * 8.0f)) ) + (float4(f_1->sun_color_0) ).xyz * float3(0.059999998658895493f)  * float3(pow(saturate(dot(d_0, (float4(f_1->sun_dir_0) ).xyz)), 8.0f)) ;
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
    float t_0 = saturate(- _S7);
    float _S8;
    if(_S5 >= 0.0f)
    {
        _S8 = - t_0;
    }
    else
    {
        _S8 = t_0;
    }
    n_1.x = n_1.x + _S8;
    if((n_1.y) >= 0.0f)
    {
        _S8 = - t_0;
    }
    else
    {
        _S8 = t_0;
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
    float c_0 = cos(r_0);
    float4 _S12 = float4(m_1->uv_0) ;
    float2 q_0 = uv_2 * _S12.zw;
    float _S13 = q_0.x;
    float _S14 = q_0.y;
    return float2(c_0 * _S13 + s_0 * _S14, - s_0 * _S13 + c_0 * _S14) + _S12.xy;
}

float length_squared_0(float3 v_0)
{
    return dot(v_0, v_0);
}

float3 ambient_0(FrameData_natural_0 device* f_2, float3 n_3)
{
    return mix((float4(f_2->ground_color_0) ).xyz, mix((float4(f_2->sky_horizon_0) ).xyz, (float4(f_2->sky_zenith_0) ).xyz, float3(0.5f) ), float3((n_3.y * 0.5f + 0.5f)) );
}

float3 shade_toon_0(FrameData_natural_0 device* f_3, const GpuMaterial_natural_0 thread* m_2, float3 albedo_0, float3 n_4, float3 v_1)
{
    float ndl_0 = dot(n_4, (float4(f_3->sun_dir_0) ).xyz);
    float _S15 = max((float4(m_2->params_0) ).z, 1.0f);
    float4 _S16 = float4(m_2->shade_0) ;
    float3 c_1 = mix(albedo_0 * _S16.xyz, albedo_0, float3((floor(saturate(ndl_0 * 0.5f + 0.5f) * _S15 + 0.5f) / _S15)) );
    float3 _S17 = (float4(f_3->sun_color_0) ).xyz * float3(0.60000002384185791f) ;
    float3 _S18 = ambient_0(f_3, n_4);
    return c_1 * (_S17 + _S18) * float3(0.5f)  + float3((pow(1.0f - saturate(dot(n_4, v_1)), 4.0f) * _S16.w * saturate(ndl_0 + 0.30000001192092896f)))  * (float4(f_3->sun_color_0) ).xyz * float3(0.30000001192092896f) ;
}

float3 f_schlick_0(float3 f0_0, float voh_0)
{
    return f0_0 + (float3(1.0f)  - f0_0) * float3(pow(1.0f - voh_0, 5.0f)) ;
}

float d_ggx_0(float noh_0, float a_0)
{
    float a2_1 = a_0 * a_0;
    float d_1 = noh_0 * noh_0 * (a2_1 - 1.0f) + 1.0f;
    return a2_1 / (3.1415927410125732f * d_1 * d_1);
}

float v_smith_0(float nov_0, float nol_0, float a_1)
{
    float a2_2 = a_1 * a_1;
    float _S19 = 1.0f - a2_2;
    return 0.5f / max(nol_0 * sqrt(nov_0 * nov_0 * _S19 + a2_2) + nov_0 * sqrt(nol_0 * nol_0 * _S19 + a2_2), 9.9999997473787516e-06f);
}

float2 env_brdf_0(float nov_1, float rough_0)
{
    float4 r_1 = float4(rough_0)  * float4(-1.0f, -0.027499999850988388f, -0.57200002670288086f, 0.02199999988079071f) + float4(1.0f, 0.042500000447034836f, 1.0399999618530273f, -0.039999999105930328f);
    float _S20 = r_1.x;
    return float2(-1.0399999618530273f, 1.0399999618530273f) * float2((min(_S20 * _S20, exp2(-9.2799997329711914f * nov_1)) * _S20 + r_1.y))  + r_1.zw;
}

float3 shade_pbr_0(FrameData_natural_0 device* f_4, float3 albedo_1, float metal_0, float rough_1, float ao_0, float3 n_5, float3 v_2)
{
    float _S21 = saturate(metal_0);
    float _S22 = clamp(rough_1, 0.039999999105930328f, 1.0f);
    float a_2 = _S22 * _S22;
    float3 l_0 = (float4(f_4->sun_dir_0) ).xyz;
    float3 h_0 = normalize(l_0 + v_2);
    float nol_1 = saturate(dot(n_5, l_0));
    float _S23 = max(dot(n_5, v_2), 9.9999997473787516e-05f);
    float3 f0_1 = mix(float3(0.039999999105930328f) , albedo_1, float3(_S21) );
    float3 diff_col_0 = albedo_1 * float3((1.0f - _S21)) ;
    float3 F_0 = f_schlick_0(f0_1, saturate(dot(v_2, h_0)));
    float3 direct_0 = (diff_col_0 * (float3(1.0f)  - F_0) + float3((d_ggx_0(saturate(dot(n_5, h_0)), a_2) * v_smith_0(_S23, nol_1, a_2)))  * F_0 * float3(3.1415927410125732f) ) * ((float4(f_4->sun_color_0) ).xyz * float3(nol_1) );
    float2 ab_0 = env_brdf_0(_S23, _S22);
    float3 _S24 = sky_radiance_0(f_4, reflect(- v_2, n_5));
    float3 _S25 = ambient_0(f_4, n_5);
    float3 env_0 = mix(_S24, _S25, float3(_S22) );
    float3 _S26 = ambient_0(f_4, n_5);
    return direct_0 + (diff_col_0 * _S26 + env_0 * (f0_1 * float3(ab_0.x)  + float3(ab_0.y) )) * float3(ao_0) ;
}

struct pixelOutput_0
{
    float4 output_0 [[color(0)]];
};

[[fragment]] pixelOutput_0 fs_resolve(float4 pos_1 [[position]], ResolveC_0 constant* ed_c_1 [[buffer(0)]], ulong device* ed_buffers_1 [[buffer(2)]], ulong device* ed_textures_1 [[buffer(1)]], ulong device* ed_samplers_1 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_8;
    (&kernelContext_8)->ed_c_0 = ed_c_1;
    (&kernelContext_8)->ed_buffers_0 = ed_buffers_1;
    (&kernelContext_8)->ed_textures_0 = ed_textures_1;
    (&kernelContext_8)->ed_samplers_0 = ed_samplers_1;
    FrameData_natural_0 device* _S27 = ed_buf_7(ed_c_1->frame_0, &kernelContext_8);
    float2 _S28 = pos_1.xy;
    uint2 _S29 = uint2(_S28);
    texture2d<uint, access::sample> _S30 = ((reinterpret_cast<device texture2d<uint, access::sample>*>(((&kernelContext_8)->ed_textures_0))[((&kernelContext_8)->ed_c_0->vis_0)]));
    int3 _S31 = int3(int2(_S29), int(0));
    uint2 vis_1 = ((_S30).read(vec<uint,2>(((_S31)).xy), uint(((_S31)).z))).xy;
    float3 _S32 = view_dir_0(_S27, _S28);
    uint _S33 = vis_1.x;
    if(_S33 == 0U)
    {
        float3 _S34 = sky_radiance_0(_S27, _S32);
        pixelOutput_0 _S35 = { float4(_S34, 1.0f) };
        return _S35;
    }
    GpuInstance_natural_0 device* _S36 = ed_buf_6((&kernelContext_8)->ed_c_0->instances_0, &kernelContext_8);
    thread GpuInstance_natural_0 _S37 = *(_S36 + (_S33 - 1U));
    GpuMesh_natural_0 device* _S38 = ed_buf_5((&kernelContext_8)->ed_c_0->meshes_0, &kernelContext_8);
    GpuMesh_natural_0 device* _S39 = _S38 + (&_S37)->mesh_0;
    uint _S40 = vis_1.y;
    uint lod_0 = _S40 >> 28U;
    uint prim_0 = (_S40 & 268435455U) % max(_S39->index_count_0[lod_0] / 3U, 1U);
    uint device* _S41 = ed_buf_4((&kernelContext_8)->ed_c_0->indices_0, &kernelContext_8);
    uint device* idx_0 = _S41 + _S39->first_index_0[lod_0] + prim_0 * 3U;
    Vtx_0 device* _S42 = ed_buf_3((&kernelContext_8)->ed_c_0->verts_0, &kernelContext_8);
    int i0_0 = int(*(idx_0 + int(0))) + _S39->base_vertex_0[lod_0];
    int i1_0 = int(*(idx_0 + int(1))) + _S39->base_vertex_0[lod_0];
    int i2_0 = int(*(idx_0 + int(2))) + _S39->base_vertex_0[lod_0];
    Vtx_0 v0_0 = *(_S42 + i0_0);
    Vtx_0 v1_0 = *(_S42 + i1_0);
    Vtx_0 v2_0 = *(_S42 + i2_0);
    packed_float4 device* _S43 = ed_buf_2((&kernelContext_8)->ed_c_0->nodes_0, &kernelContext_8);
    float3 node_0 = (float4(*(_S43 + (((&_S37)->node_flags_0) & 65535U))) ).xyz;
    float3 _S44 = xform_point_0(&_S37, float3(v0_0.px_0, v0_0.py_0, v0_0.pz_0));
    float3 _S45 = xform_point_0(&_S37, float3(v1_0.px_0, v1_0.py_0, v1_0.pz_0));
    float3 _S46 = xform_point_0(&_S37, float3(v2_0.px_0, v2_0.py_0, v2_0.pz_0));
    float3 p0_1 = node_0 + _S44;
    float3 e1_1 = _S45 - _S44;
    float3 e2_1 = _S46 - _S44;
    float2 bb_0 = ray_bary_0(_S32, p0_1, e1_1, e2_1);
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
    float3 _S47 = float3((1.0f - b1_1 - b2_1)) ;
    float3 local_0 = _S44 * _S47 + _S45 * float3(b1_1)  + _S46 * float3(b2_1) ;
    float3 _S48 = xform_normal_0(&_S37, oct_decode_0(v0_0.n_0) * _S47 + oct_decode_0(v1_0.n_0) * float3(b1_1)  + oct_decode_0(v2_0.n_0) * float3(b2_1) );
    float3 n_6 = normalize(_S48);
    float3 v_3 = - _S32;
    GpuMaterial_natural_0 device* _S49 = ed_buf_1((&kernelContext_8)->ed_c_0->materials_0, &kernelContext_8);
    thread GpuMaterial_natural_0 _S50 = *(_S49 + (&_S37)->material_0);
    float4 _S51 = float4((&_S50)->base_0) ;
    float3 albedo_2 = _S51.xyz;
    float4 _S52 = float4((&_S50)->params_0) ;
    float metal_1 = _S52.x;
    float rough_2 = _S52.y;
    uint4 _S53 = uint4((&_S50)->tex_0) ;
    uint _S54 = _S53.x;
    bool _S55 = _S54 != 4294967295U;
    bool textured_0;
    if(_S55)
    {
        textured_0 = true;
    }
    else
    {
        textured_0 = (_S53.y) != 4294967295U;
    }
    if(textured_0)
    {
        textured_0 = true;
    }
    else
    {
        textured_0 = (_S53.z) != 4294967295U;
    }
    float ao_1;
    float metal_2;
    float rough_3;
    float3 albedo_3;
    float3 n_7;
    if(textured_0)
    {
        packed_float2 device* _S56 = ed_buf_0((&kernelContext_8)->ed_c_0->uvs_0, &kernelContext_8);
        float2 _S57 = float2(*(_S56 + i0_0)) ;
        float2 d1_0 = float2(*(_S56 + i1_0))  - _S57;
        float2 d2_0 = float2(*(_S56 + i2_0))  - _S57;
        float2 _S58 = uv_xform_0(&_S50, _S57 + d1_0 * float2(b1_1)  + d2_0 * float2(b2_1) );
        float3 _S59 = view_dir_0(_S27, _S28 + float2(1.0f, 0.0f));
        float2 bx_0 = ray_bary_0(_S59, p0_1, e1_1, e2_1);
        float3 _S60 = view_dir_0(_S27, _S28 + float2(0.0f, 1.0f));
        float2 by_0 = ray_bary_0(_S60, p0_1, e1_1, e2_1);
        float2 _S61 = uv_xform_0(&_S50, _S57 + d1_0 * float2(bx_0.x)  + d2_0 * float2(bx_0.y) );
        float2 ux_0 = _S61 - _S58;
        float2 _S62 = uv_xform_0(&_S50, _S57 + d1_0 * float2(by_0.x)  + d2_0 * float2(by_0.y) );
        float2 uy_0 = _S62 - _S58;
        sampler _S63 = ((reinterpret_cast<device sampler*>(((&kernelContext_8)->ed_samplers_0))[((&kernelContext_8)->ed_c_0->smp_0)]));
        if(_S55)
        {
            texture2d<float, access::sample> _S64 = ((reinterpret_cast<device texture2d<float, access::sample>*>(((&kernelContext_8)->ed_textures_0))[(_S54)]));
            albedo_3 = albedo_2 * ((_S64).sample((_S63), (_S58), gradient2d((ux_0), (uy_0)))).xyz;
        }
        else
        {
            albedo_3 = albedo_2;
        }
        uint _S65 = _S53.z;
        if(_S65 != 4294967295U)
        {
            texture2d<float, access::sample> _S66 = ((reinterpret_cast<device texture2d<float, access::sample>*>(((&kernelContext_8)->ed_textures_0))[(_S65)]));
            float3 orm_0 = ((_S66).sample((_S63), (_S58), gradient2d((ux_0), (uy_0)))).xyz;
            float rough_4 = rough_2 * orm_0.y;
            float metal_3 = metal_1 * orm_0.z;
            ao_1 = orm_0.x;
            metal_2 = metal_3;
            rough_3 = rough_4;
        }
        else
        {
            ao_1 = 1.0f;
            metal_2 = metal_1;
            rough_3 = rough_2;
        }
        uint _S67 = _S53.y;
        if(_S67 != 4294967295U)
        {
            float _S68 = d1_0.x;
            float _S69 = d2_0.y;
            float _S70 = d1_0.y;
            float _S71 = d2_0.x;
            float det_1 = _S68 * _S69 - _S70 * _S71;
            if((abs(det_1)) > 9.999999960041972e-13f)
            {
                float3 _S72 = float3((1.0f / det_1)) ;
                float3 tg_0 = (e1_1 * float3(_S69)  - e2_1 * float3(_S70) ) * _S72;
                float3 bt_0 = (e2_1 * float3(_S68)  - e1_1 * float3(_S71) ) * _S72;
                float3 tg_1 = tg_0 - n_6 * float3(dot(n_6, tg_0)) ;
                if((length_squared_0(tg_1)) > 9.99999968265522539e-21f)
                {
                    float3 tg_2 = normalize(tg_1);
                    float3 _S73 = cross(n_6, tg_2);
                    float sgn_0;
                    if((dot(_S73, bt_0)) < 0.0f)
                    {
                        sgn_0 = -1.0f;
                    }
                    else
                    {
                        sgn_0 = 1.0f;
                    }
                    float3 bn_0 = _S73 * float3(- sgn_0) ;
                    texture2d<float, access::sample> _S74 = ((reinterpret_cast<device texture2d<float, access::sample>*>(((&kernelContext_8)->ed_textures_0))[(_S67)]));
                    float2 xy_0 = ((_S74).sample((_S63), (_S58), gradient2d((ux_0), (uy_0)))).xy * float2(2.0f)  - float2(1.0f) ;
                    n_7 = normalize(tg_2 * float3(xy_0.x)  + bn_0 * float3(xy_0.y)  + n_6 * float3(sqrt(saturate(1.0f - dot(xy_0, xy_0)))) );
                }
                else
                {
                    n_7 = n_6;
                }
            }
            else
            {
                n_7 = n_6;
            }
        }
        else
        {
            n_7 = n_6;
        }
    }
    else
    {
        n_7 = n_6;
        ao_1 = 1.0f;
        albedo_3 = albedo_2;
        metal_2 = metal_1;
        rough_3 = rough_2;
    }
    float _S75 = dot(n_7, v_3);
    if(_S75 < 0.0f)
    {
        n_7 = normalize(n_7 - float3((1.0099999904632568f * _S75))  * v_3);
    }
    if((_S52.w) > 0.5f)
    {
        float2 c_2 = floor(local_0.xz * float2(0.5f) );
        albedo_3 = albedo_3 * float3(mix(0.80000001192092896f, 1.1499999761581421f, fmod(abs(c_2.x + c_2.y), 2.0f))) ;
    }
    float3 col_0;
    if((_S51.w) > 0.5f)
    {
        float3 _S76 = shade_toon_0(_S27, &_S50, albedo_3, n_7, v_3);
        col_0 = _S76;
    }
    else
    {
        float3 _S77 = shade_pbr_0(_S27, albedo_3, metal_2, rough_3, ao_1, n_7, v_3);
        col_0 = _S77;
    }
    pixelOutput_0 _S78 = { float4(col_0, 1.0f) };
    return _S78;
}

struct vs_resolve_Result_0
{
    float4 pos_2 [[position]];
};

[[vertex]] vs_resolve_Result_0 vs_resolve(uint vid_1 [[vertex_id]], ResolveC_0 constant* ed_c_2 [[buffer(0)]], ulong device* ed_buffers_2 [[buffer(2)]], ulong device* ed_textures_2 [[buffer(1)]], ulong device* ed_samplers_2 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_9;
    (&kernelContext_9)->ed_c_0 = ed_c_2;
    (&kernelContext_9)->ed_buffers_0 = ed_buffers_2;
    (&kernelContext_9)->ed_textures_0 = ed_textures_2;
    (&kernelContext_9)->ed_samplers_0 = ed_samplers_2;
    thread vs_resolve_Result_0 _S79;
    (&_S79)->pos_2 = fullscreen_vertex_0(vid_1).pos_0;
    return _S79;
}

