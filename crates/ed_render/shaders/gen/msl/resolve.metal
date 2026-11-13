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
    uint t_draws_0;
    uint t_pool_0;
    uint t_on_0;
    array<uint, int(1)> pad_0;
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

struct Vtx_0
{
    float px_0;
    float py_0;
    float pz_0;
    uint n_0;
};

Vtx_0 device* ed_buf_2(uint i_2, KernelContext_0 thread* kernelContext_2)
{
    return (Vtx_0 device*)kernelContext_2->ed_buffers_0[i_2];
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

GpuMesh_natural_0 device* ed_buf_3(uint i_3, KernelContext_0 thread* kernelContext_3)
{
    return (GpuMesh_natural_0 device*)kernelContext_3->ed_buffers_0[i_3];
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

GpuInstance_natural_0 device* ed_buf_4(uint i_4, KernelContext_0 thread* kernelContext_4)
{
    return (GpuInstance_natural_0 device*)kernelContext_4->ed_buffers_0[i_4];
}

constant array<float3, int(5)> TINT_1 = { { float3(1.0f, 0.25f, 0.25f), float3(0.25f, 1.0f, 0.25f), float3(0.30000001192092896f, 0.44999998807907104f, 1.0f), float3(1.0f, 1.0f, 0.25f), float3(0.60000002384185791f, 0.60000002384185791f, 0.60000002384185791f) } };
uint device* ed_buf_5(uint i_5, KernelContext_0 thread* kernelContext_5)
{
    return (uint device*)kernelContext_5->ed_buffers_0[i_5];
}

struct TerrainDraw_natural_0
{
    packed_float4 origin_0;
    packed_float4 dir_c_0;
    packed_float4 cc_0;
    packed_float4 fu_0;
    packed_float4 fv_0;
    packed_float4 shape_0;
    packed_float4 morph_0;
    packed_float4 polar_0;
    packed_uint4 slots_0;
    packed_float4 hq_0;
};

TerrainDraw_natural_0 device* ed_buf_6(uint i_6, KernelContext_0 thread* kernelContext_6)
{
    return (TerrainDraw_natural_0 device*)kernelContext_6->ed_buffers_0[i_6];
}

packed_float4 device* ed_buf_7(uint i_7, KernelContext_0 thread* kernelContext_7)
{
    return (packed_float4 device*)kernelContext_7->ed_buffers_0[i_7];
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
    packed_float4 params_1;
    packed_float4 proj_0;
    packed_uint4 cull_0;
    array<HzbLevel_0, int(16)> hzb_0;
    LightData_natural_0 light_0;
    AtmosData_natural_0 atmos_0;
};

FrameData_natural_0 device* ed_buf_8(uint i_8, KernelContext_0 thread* kernelContext_8)
{
    return (FrameData_natural_0 device*)kernelContext_8->ed_buffers_0[i_8];
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

bool atmos_on_0(FrameData_natural_0 device* f_1)
{
    return ((uint4((&f_1->atmos_0)->cfg_1) ).x) != 0U;
}

bool atmos_on_1(FrameData_natural_0 device* f_2)
{
    return ((uint4((&f_2->atmos_0)->cfg_1) ).x) != 0U;
}

float3 sky_rad_0(FrameData_natural_0 device* f_3, float3 d_0, float disc_k_0)
{
    float3 sun_1 = (float4(f_3->sun_dir_0) ).xyz;
    float h_0 = d_0.y;
    float t_0 = pow(saturate(1.0f - max(h_0, 0.0f)), 6.5f);
    float mu_0 = dot(d_0, sun_1);
    float warm_0 = saturate(1.0f - sun_1.y * 2.5f);
    float3 c_0 = mix((float4(f_3->sky_zenith_0) ).xyz, (float4(f_3->sky_horizon_0) ).xyz, float3(t_0) ) + float3(1.0f, 0.41999998688697815f, 0.14000000059604645f) * float3(pow(saturate(mu_0 * 0.5f + 0.5f), 5.0f))  * float3(warm_0)  * float3(0.85000002384185791f)  * float3((0.40000000596046448f + t_0)) ;
    float3 _S1 = float3((float4(f_3->sun_dir_0) ).w) ;
    return mix(mix(c_0, c_0 * float3(1.0f, 0.69999998807907104f, 0.55000001192092896f), float3((warm_0 * 0.40000000596046448f)) ) + float3(1.0f, 0.89999997615814209f, 0.69999998807907104f) * float3(pow(saturate(mu_0), 28.0f))  * float3(0.34999999403953552f) , (float4(f_3->ground_color_0) ).xyz, float3(saturate(- h_0 * 6.0f)) ) * _S1 + float3(1.0f, 0.94999998807907104f, 0.85000002384185791f) * float3((smoothstep(0.99965000152587891f, 0.99984997510910034f, mu_0) * step(0.0f, h_0)))  * float3(60.0f)  * _S1 * float3(disc_k_0) ;
}

float3 sky_rad_1(FrameData_natural_0 device* f_4, float3 d_1, float disc_k_1)
{
    float3 sun_2 = (float4(f_4->sun_dir_0) ).xyz;
    float h_1 = d_1.y;
    float t_1 = pow(saturate(1.0f - max(h_1, 0.0f)), 6.5f);
    float mu_1 = dot(d_1, sun_2);
    float warm_1 = saturate(1.0f - sun_2.y * 2.5f);
    float3 c_1 = mix((float4(f_4->sky_zenith_0) ).xyz, (float4(f_4->sky_horizon_0) ).xyz, float3(t_1) ) + float3(1.0f, 0.41999998688697815f, 0.14000000059604645f) * float3(pow(saturate(mu_1 * 0.5f + 0.5f), 5.0f))  * float3(warm_1)  * float3(0.85000002384185791f)  * float3((0.40000000596046448f + t_1)) ;
    float3 _S2 = float3((float4(f_4->sun_dir_0) ).w) ;
    return mix(mix(c_1, c_1 * float3(1.0f, 0.69999998807907104f, 0.55000001192092896f), float3((warm_1 * 0.40000000596046448f)) ) + float3(1.0f, 0.89999997615814209f, 0.69999998807907104f) * float3(pow(saturate(mu_1), 28.0f))  * float3(0.34999999403953552f) , (float4(f_4->ground_color_0) ).xyz, float3(saturate(- h_1 * 6.0f)) ) * _S2 + float3(1.0f, 0.94999998807907104f, 0.85000002384185791f) * float3((smoothstep(0.99965000152587891f, 0.99984997510910034f, mu_1) * step(0.0f, h_1)))  * float3(60.0f)  * _S2 * float3(disc_k_1) ;
}

float atm_rb_0(FrameData_natural_0 device* f_5)
{
    return (float4((&f_5->atmos_0)->radii_0) ).x;
}

float atm_rb_1(FrameData_natural_0 device* f_6)
{
    return (float4((&f_6->atmos_0)->radii_0) ).x;
}

float atm_rt_0(FrameData_natural_0 device* f_7)
{
    return (float4((&f_7->atmos_0)->radii_0) ).y;
}

float atm_rt_1(FrameData_natural_0 device* f_8)
{
    return (float4((&f_8->atmos_0)->radii_0) ).y;
}

float atm_enter_top_0(float rb_0, float rt_0, float h_2, float mu_2)
{
    float r_0 = rb_0 + h_2;
    float _S3 = r_0 * mu_2;
    float d_2 = _S3 * _S3 - (h_2 - (rt_0 - rb_0)) * (r_0 + rt_0);
    if(d_2 < 0.0f)
    {
        return -1.0f;
    }
    float t_2 = - r_0 * mu_2 - sqrt(d_2);
    float _S4;
    if(t_2 >= 0.0f)
    {
        _S4 = t_2;
    }
    else
    {
        _S4 = -1.0f;
    }
    return _S4;
}

bool atm_enter_0(FrameData_natural_0 device* f_9, float thread* h_3, float thread* mu_3, float thread* mu_s_0, float nu_0, float thread* te_0)
{
    *te_0 = 0.0f;
    float _S5 = atm_rb_1(f_9);
    float _S6 = atm_rt_1(f_9);
    float _S7 = _S6 - _S5;
    if((*h_3) <= _S7)
    {
        return true;
    }
    float _S8 = atm_enter_top_0(_S5, _S6, *h_3, *mu_3);
    *te_0 = _S8;
    if(_S8 < 0.0f)
    {
        return false;
    }
    float r_1 = _S5 + *h_3;
    *mu_3 = clamp((r_1 * *mu_3 + *te_0) / _S6, -1.0f, 1.0f);
    *mu_s_0 = clamp((r_1 * *mu_s_0 + *te_0 * nu_0) / _S6, -1.0f, 1.0f);
    *h_3 = _S7;
    return true;
}

bool atm_hits_ground_0(float rb_1, float h_4, float mu_4)
{
    float rm_0 = (rb_1 + h_4) * mu_4;
    bool _S9;
    if(mu_4 < 0.0f)
    {
        _S9 = (rm_0 * rm_0) >= (h_4 * (2.0f * rb_1 + h_4));
    }
    else
    {
        _S9 = false;
    }
    return _S9;
}

float atm_dist_top_0(float rb_2, float rt_1, float h_5, float mu_5)
{
    float r_2 = rb_2 + h_5;
    float _S10 = r_2 * mu_5;
    return max(- r_2 * mu_5 + sqrt(max(_S10 * _S10 + (rt_1 - rb_2 - h_5) * (rt_1 + r_2), 0.0f)), 0.0f);
}

float2 atm_trans_uv_0(float rb_3, float rt_2, float h_6, float mu_6)
{
    float ht_0 = rt_2 - rb_3;
    float _S11 = clamp(h_6, 0.0f, ht_0);
    float big_h_0 = sqrt(ht_0 * (rt_2 + rb_3));
    float rho_0 = sqrt(max(_S11 * (2.0f * rb_3 + _S11), 0.0f));
    float dmin_0 = ht_0 - _S11;
    return float2((atm_dist_top_0(rb_3, rt_2, _S11, mu_6) - dmin_0) / max(rho_0 + big_h_0 - dmin_0, 9.99999997475242708e-07f), rho_0 / big_h_0);
}

float4 atm_bilinear_0(packed_float4 device* buf_0, uint w_0, uint row0_0, uint rows_0, float2 uv_2)
{
    float2 p_1 = saturate(uv_2) * float2(float(w_0 - 1U), float(rows_0 - 1U));
    uint2 _S12 = min(uint2(p_1), uint2(w_0 - 2U, rows_0 - 2U));
    float2 g_0 = p_1 - float2(_S12);
    uint b_0 = (row0_0 + _S12.y) * w_0 + _S12.x;
    float4 _S13 = float4(g_0.x) ;
    uint _S14 = b_0 + w_0;
    return mix(mix(float4(*(buf_0 + b_0)) , float4(*(buf_0 + (b_0 + 1U))) , _S13), mix(float4(*(buf_0 + _S14)) , float4(*(buf_0 + (_S14 + 1U))) , _S13), float4(g_0.y) );
}

float3 atm_transmittance_0(FrameData_natural_0 device* f_10, float h_7, float mu_7, KernelContext_0 thread* kernelContext_9)
{
    float _S15 = atm_rb_0(f_10);
    float _S16 = atm_rt_0(f_10);
    float2 uv_3 = atm_trans_uv_0(_S15, _S16, h_7, mu_7);
    packed_float4 device* _S17 = ed_buf_7((uint4((&f_10->atmos_0)->bufs_0) ).x, kernelContext_9);
    float4 _S18 = atm_bilinear_0(_S17, 256U, 0U, 64U, uv_3);
    return _S18.xyz;
}

float3 atm_transmittance_1(FrameData_natural_0 device* f_11, float h_8, float mu_8, KernelContext_0 thread* kernelContext_10)
{
    float _S19 = atm_rb_0(f_11);
    float _S20 = atm_rt_0(f_11);
    float2 uv_4 = atm_trans_uv_0(_S19, _S20, h_8, mu_8);
    packed_float4 device* _S21 = ed_buf_7((uint4((&f_11->atmos_0)->bufs_0) ).x, kernelContext_10);
    float4 _S22 = atm_bilinear_0(_S21, 256U, 0U, 64U, uv_4);
    return _S22.xyz;
}

float atm_cos_horizon_0(float rb_4, float h_9)
{
    return - sqrt(max(h_9 * (2.0f * rb_4 + h_9), 0.0f)) / (rb_4 + h_9);
}

float3 atm_sky_view_0(FrameData_natural_0 device* f_12, uint buf_1, uint w_1, uint rows_1, float h_10, float3 d_3, KernelContext_0 thread* kernelContext_11)
{
    float mu_9 = clamp(dot(d_3, (float4((&f_12->atmos_0)->up_0) ).xyz), -1.0f, 1.0f);
    float mu_s_1 = (float4((&f_12->atmos_0)->up_0) ).w;
    float nu_1 = dot(d_3, (float4(f_12->sun_dir_0) ).xyz);
    float _S23 = sqrt(max(1.0f - mu_9 * mu_9, 0.0f)) * sqrt(max(1.0f - mu_s_1 * mu_s_1, 0.0f));
    float cphi_0;
    if(_S23 > 9.99999997475242708e-07f)
    {
        cphi_0 = clamp((nu_1 - mu_9 * mu_s_1) / _S23, -1.0f, 1.0f);
    }
    else
    {
        cphi_0 = 1.0f;
    }
    float u_0 = sqrt(acos(cphi_0) / 3.1415927410125732f);
    float _S24 = atm_rb_0(f_12);
    float zha_0 = acos(atm_cos_horizon_0(_S24, h_10));
    float theta_0 = acos(mu_9);
    uint half_rows_0 = rows_1 / 2U;
    float c_2;
    uint row0_1;
    if(theta_0 < zha_0)
    {
        float _S25 = 1.0f - sqrt(saturate(1.0f - theta_0 / zha_0));
        row0_1 = 0U;
        c_2 = _S25;
    }
    else
    {
        float _S26 = sqrt(saturate((theta_0 - zha_0) / max(3.1415927410125732f - zha_0, 9.99999997475242708e-07f)));
        row0_1 = half_rows_0;
        c_2 = _S26;
    }
    packed_float4 device* _S27 = ed_buf_7(buf_1, kernelContext_11);
    float4 _S28 = atm_bilinear_0(_S27, w_1, row0_1, half_rows_0, float2(u_0, c_2));
    return _S28.xyz;
}

float3 atmos_sky_0(FrameData_natural_0 device* f_13, float3 d_4, KernelContext_0 thread* kernelContext_12)
{
    float3 _S29 = atm_sky_view_0(f_13, (uint4((&f_13->atmos_0)->bufs_0) ).z, 192U, 108U, (float4((&f_13->atmos_0)->radii_0) ).z, d_4, kernelContext_12);
    return _S29;
}

float3 atmos_sky_1(FrameData_natural_0 device* f_14, float3 d_5, KernelContext_0 thread* kernelContext_13)
{
    float3 _S30 = atm_sky_view_0(f_14, (uint4((&f_14->atmos_0)->bufs_0) ).z, 192U, 108U, (float4((&f_14->atmos_0)->radii_0) ).z, d_5, kernelContext_13);
    return _S30;
}

float3 atmos_sky_pixel_0(FrameData_natural_0 device* f_15, float3 d_6, float disc_k_2, KernelContext_0 thread* kernelContext_14)
{
    float3 _S31 = atmos_sky_0(f_15, d_6, kernelContext_14);
    float nu_2 = dot(d_6, (float4(f_15->sun_dir_0) ).xyz);
    float cr_0 = (float4((&f_15->atmos_0)->sun_0) ).w;
    float _S32 = cr_0 - 1.99999999495048542e-06f;
    float3 c_3;
    if(nu_2 > _S32)
    {
        thread float h_11 = (float4((&f_15->atmos_0)->radii_0) ).z;
        thread float mu_10 = dot(d_6, (float4((&f_15->atmos_0)->up_0) ).xyz);
        thread float mu_s_2 = (float4((&f_15->atmos_0)->up_0) ).w;
        float3 _S33 = float3(1.0f) ;
        thread float te_1;
        bool _S34 = atm_enter_0(f_15, &h_11, &mu_10, &mu_s_2, nu_2, &te_1);
        if(_S34)
        {
            float _S35 = atm_rb_0(f_15);
            if(atm_hits_ground_0(_S35, h_11, mu_10))
            {
                c_3 = float3(0.0f) ;
            }
            else
            {
                float3 _S36 = atm_transmittance_0(f_15, h_11, mu_10, kernelContext_14);
                c_3 = _S36;
            }
        }
        else
        {
            c_3 = _S33;
        }
        float sr2_0 = 1.0f - cr_0 * cr_0;
        c_3 = _S31 + min((float4((&f_15->atmos_0)->sun_0) ).xyz * c_3 / float3((3.1415927410125732f * sr2_0))  * float3((1.0f - 0.60000002384185791f * (1.0f - sqrt(1.0f - saturate((1.0f - nu_2 * nu_2) / sr2_0)))))  * float3(smoothstep(_S32, cr_0 + 1.99999999495048542e-06f, nu_2))  * float3(disc_k_2) , float3(30000.0f) );
    }
    else
    {
        c_3 = _S31;
    }
    return c_3;
}

float3 sky_pixel_0(FrameData_natural_0 device* f_16, float3 d_7, KernelContext_0 thread* kernelContext_15)
{
    bool _S37 = atmos_on_0(f_16);
    if(!_S37)
    {
        float3 _S38 = sky_rad_0(f_16, d_7, (float4((&f_16->light_0)->sky_0) ).x);
        return _S38;
    }
    if(((uint4((&f_16->atmos_0)->cfg_1) ).y) == 1U)
    {
        thread float h_12 = (float4((&f_16->atmos_0)->radii_0) ).z;
        thread float mu_11 = dot(d_7, (float4((&f_16->atmos_0)->up_0) ).xyz);
        thread float mu_s_3 = (float4((&f_16->atmos_0)->up_0) ).w;
        thread float te_2;
        bool _S39 = atm_enter_0(f_16, &h_12, &mu_11, &mu_s_3, dot(d_7, (float4(f_16->sun_dir_0) ).xyz), &te_2);
        if(!_S39)
        {
            return float3(0.5f) ;
        }
        float _S40 = atm_rb_0(f_16);
        float3 _S41;
        if(atm_hits_ground_0(_S40, h_12, mu_11))
        {
            _S41 = float3(0.0f) ;
        }
        else
        {
            float3 _S42 = atm_transmittance_0(f_16, h_12, mu_11, kernelContext_15);
            _S41 = _S42 * float3(0.5f) ;
        }
        return _S41;
    }
    float _S43;
    if(((uint4((&f_16->atmos_0)->cfg_1) ).y) == 2U)
    {
        _S43 = 0.0f;
    }
    else
    {
        _S43 = (float4((&f_16->light_0)->sky_0) ).x;
    }
    float3 _S44 = atmos_sky_pixel_0(f_16, d_7, _S43, kernelContext_15);
    return _S44;
}

uint2 t_ring_0(uint s_0)
{
    uint e_0 = s_0 % 64U;
    switch(s_0 / 64U)
    {
    case 0U:
        {
            return uint2(e_0, 0U);
        }
    case 1U:
        {
            return uint2(64U, e_0);
        }
    case 2U:
        {
            return uint2(64U - e_0, 64U);
        }
    default:
        {
            return uint2(0U, 64U - e_0);
        }
    }
}

uint3 t_triangle_0(uint prim_0)
{
    uint3 _S45;
    if(prim_0 < 8192U)
    {
        uint q_0 = prim_0 >> 1U;
        uint v00_0 = q_0 / 64U * 65U + q_0 % 64U;
        uint v10_0 = v00_0 + 1U;
        uint v01_0 = v00_0 + 65U;
        uint v11_0 = v01_0 + 1U;
        if((prim_0 & 1U) == 0U)
        {
            _S45 = uint3(v00_0, v10_0, v11_0);
        }
        else
        {
            _S45 = uint3(v00_0, v11_0, v01_0);
        }
        return _S45;
    }
    uint k_0 = prim_0 - 8192U;
    uint s_1 = k_0 >> 1U;
    uint s1_0 = (s_1 + 1U) % 256U;
    uint2 p_2 = t_ring_0(s_1);
    uint2 q_1 = t_ring_0(s1_0);
    uint tp_0 = p_2.y * 65U + p_2.x;
    uint tq_0 = q_1.y * 65U + q_1.x;
    uint a_0 = 4225U + s_1;
    uint b_1 = 4225U + s1_0;
    if((k_0 & 1U) == 0U)
    {
        _S45 = uint3(a_0, b_1, tq_0);
    }
    else
    {
        _S45 = uint3(a_0, tq_0, tp_0);
    }
    return _S45;
}

uint2 t_edge_at_0(uint edge_0, uint e_1)
{
    switch(edge_0)
    {
    case 0U:
        {
            return uint2(0U, e_1);
        }
    case 1U:
        {
            return uint2(64U, e_1);
        }
    case 2U:
        {
            return uint2(e_1, 0U);
        }
    default:
        {
            return uint2(e_1, 64U);
        }
    }
}

float t_page_0(uint pool_0, uint slot_0, uint i_9, uint j_0, float scale_0, float offset_1, KernelContext_0 thread* kernelContext_16)
{
    uint s_2 = j_0 * 257U + i_9;
    uint device* _S46 = ed_buf_5(pool_0, kernelContext_16);
    uint w_2 = *(_S46 + (slot_0 * 33025U + (s_2 >> 1U)));
    uint q_2;
    if((s_2 & 1U) != 0U)
    {
        q_2 = w_2 >> 16U;
    }
    else
    {
        q_2 = w_2 & 65535U;
    }
    return offset_1 + scale_0 * float(q_2);
}

float t_own_h_0(uint pool_1, const TerrainDraw_natural_0 thread* d_8, uint i_10, uint j_1, KernelContext_0 thread* kernelContext_17)
{
    float4 _S47 = float4(d_8->hq_0) ;
    float _S48 = t_page_0(pool_1, (uint4(d_8->slots_0) ).x, i_10 * 4U, j_1 * 4U, _S47.x, _S47.y, kernelContext_17);
    return _S48;
}

float3 t_lattice_0(const TerrainDraw_natural_0 thread* d_9, uint i_11, uint j_2, float h_13, float3 thread* dir_0)
{
    float4 _S49 = float4(d_9->shape_0) ;
    float _S50 = _S49.x;
    float tu_0 = tan((float(i_11) - 32.0f) * _S50);
    float tv_0 = tan((float(j_2) - 32.0f) * _S50);
    float4 _S51 = float4(d_9->fu_0) ;
    float _S52 = _S51.w;
    float4 _S53 = float4(d_9->fv_0) ;
    float _S54 = _S53.w;
    float3 delta_0 = float3((tu_0 * (1.0f + _S52 * _S52) / (1.0f - _S52 * tu_0)))  * _S51.xyz + float3((tv_0 * (1.0f + _S54 * _S54) / (1.0f - _S54 * tv_0)))  * _S53.xyz;
    float4 _S55 = float4(d_9->cc_0) ;
    float lc_0 = _S55.w;
    float3 _S56 = _S55.xyz;
    float k_1 = 2.0f * dot(_S56, delta_0) + dot(delta_0, delta_0);
    float l_0 = sqrt(lc_0 * lc_0 + k_1);
    float3 dd_0 = delta_0 / float3(l_0)  - _S56 * float3((k_1 / ((lc_0 + l_0) * l_0 * lc_0))) ;
    float4 _S57 = float4(d_9->dir_c_0) ;
    float3 _S58 = _S57.xyz;
    float3 _S59 = _S58 + dd_0;
    *dir_0 = _S59;
    float3 _S60 = (float4(d_9->polar_0) ).xyz;
    float dzc_0 = dot(_S58, _S60);
    float ddz_0 = dot(dd_0, _S60);
    float dz_0 = dzc_0 + ddz_0;
    float e_2 = _S49.z;
    float s_3 = sqrt(1.0f - e_2 * dz_0 * dz_0);
    float sc_0 = _S49.w;
    float dr_0 = _S49.y * e_2 * ddz_0 * (dz_0 + dzc_0) / (s_3 * sc_0 * (s_3 + sc_0));
    return dd_0 * float3((_S57.w + dr_0))  + _S58 * float3(dr_0)  + _S59 * float3(h_13) ;
}

float3 t_base_0(uint pool_2, const TerrainDraw_natural_0 thread* d_10, uint i_12, uint j_3, KernelContext_0 thread* kernelContext_18)
{
    float _S61 = t_own_h_0(pool_2, d_10, i_12, j_3, kernelContext_18);
    thread float3 dir_1;
    float3 _S62 = t_lattice_0(d_10, i_12, j_3, _S61, &dir_1);
    return _S62;
}

float t_morph_0(float3 rel_0, float2 band_0)
{
    float _S63 = band_0.x;
    return saturate((length(rel_0) - _S63) / max(band_0.y - _S63, 0.0010000000474974513f));
}

float3 t_coarse_even_0(uint pool_3, const TerrainDraw_natural_0 thread* d_11, uint e_3, uint2 a_1, uint2 b_2, uint2 c_4, KernelContext_0 thread* kernelContext_19)
{
    float3 _S64 = t_base_0(pool_3, d_11, a_1.x, a_1.y, kernelContext_19);
    if((e_3 & 3U) == 0U)
    {
        return _S64;
    }
    float m_1 = t_morph_0((float4(d_11->origin_0) ).xyz + _S64, (float4(d_11->morph_0) ).zw);
    float3 _S65 = t_base_0(pool_3, d_11, b_2.x, b_2.y, kernelContext_19);
    float3 _S66 = t_base_0(pool_3, d_11, c_4.x, c_4.y, kernelContext_19);
    return mix(_S64, float3(0.5f)  * (_S65 + _S66), float3(m_1) );
}

float3 t_coarse_even_1(uint pool_4, const TerrainDraw_natural_0 thread* d_12, uint e_4, uint2 a_2, uint2 b_3, uint2 c_5, KernelContext_0 thread* kernelContext_20)
{
    float3 _S67 = t_base_0(pool_4, d_12, a_2.x, a_2.y, kernelContext_20);
    if((e_4 & 3U) == 0U)
    {
        return _S67;
    }
    float m_2 = t_morph_0((float4(d_12->origin_0) ).xyz + _S67, (float4(d_12->morph_0) ).zw);
    float3 _S68 = t_base_0(pool_4, d_12, b_3.x, b_3.y, kernelContext_20);
    float3 _S69 = t_base_0(pool_4, d_12, c_5.x, c_5.y, kernelContext_20);
    return mix(_S67, float3(0.5f)  * (_S68 + _S69), float3(m_2) );
}

float3 t_coarse_0(uint pool_5, const TerrainDraw_natural_0 thread* d_13, uint edge_1, uint e_5, KernelContext_0 thread* kernelContext_21)
{
    if((e_5 & 1U) == 0U)
    {
        float3 _S70 = t_coarse_even_1(pool_5, d_13, e_5, t_edge_at_0(edge_1, e_5), t_edge_at_0(edge_1, e_5 - 2U), t_edge_at_0(edge_1, e_5 + 2U), kernelContext_21);
        return _S70;
    }
    uint _S71 = e_5 - 1U;
    uint2 _S72 = t_edge_at_0(edge_1, _S71);
    uint _S73 = e_5 + 1U;
    uint2 _S74 = t_edge_at_0(edge_1, _S73);
    float3 _S75 = t_coarse_even_1(pool_5, d_13, _S71, _S72, t_edge_at_0(edge_1, e_5 - 3U), _S74, kernelContext_21);
    float3 _S76 = t_coarse_even_1(pool_5, d_13, _S73, _S74, _S72, t_edge_at_0(edge_1, e_5 + 3U), kernelContext_21);
    return float3(0.5f)  * (_S75 + _S76);
}

float3 t_target_even_0(uint pool_6, const TerrainDraw_natural_0 thread* d_14, uint i_13, uint j_4, KernelContext_0 thread* kernelContext_22)
{
    bool border_0;
    if(i_13 == 0U)
    {
        border_0 = true;
    }
    else
    {
        border_0 = j_4 == 0U;
    }
    if(border_0)
    {
        border_0 = true;
    }
    else
    {
        border_0 = i_13 == 64U;
    }
    if(border_0)
    {
        border_0 = true;
    }
    else
    {
        border_0 = j_4 == 64U;
    }
    if(border_0)
    {
        border_0 = true;
    }
    else
    {
        border_0 = ((uint4(d_14->slots_0) ).y) == 4294967295U;
    }
    float h_14;
    if(border_0)
    {
        float _S77 = t_own_h_0(pool_6, d_14, i_13, j_4, kernelContext_22);
        h_14 = _S77;
    }
    else
    {
        uint4 _S78 = uint4(d_14->slots_0) ;
        uint _S79 = _S78.z;
        uint qx_0;
        if((_S79 & 16U) != 0U)
        {
            qx_0 = 128U;
        }
        else
        {
            qx_0 = 0U;
        }
        uint qy_0;
        if((_S79 & 32U) != 0U)
        {
            qy_0 = 128U;
        }
        else
        {
            qy_0 = 0U;
        }
        float4 _S80 = float4(d_14->hq_0) ;
        float _S81 = t_page_0(pool_6, _S78.y, qx_0 + 2U * i_13, qy_0 + 2U * j_4, _S80.z, _S80.w, kernelContext_22);
        h_14 = _S81;
    }
    thread float3 dir_2;
    float3 _S82 = t_lattice_0(d_14, i_13, j_4, h_14, &dir_2);
    return _S82;
}

float3 t_target_0(uint pool_7, const TerrainDraw_natural_0 thread* d_15, uint i_14, uint j_5, KernelContext_0 thread* kernelContext_23)
{
    bool ei_0 = (i_14 & 1U) == 0U;
    bool ej_0 = (j_5 & 1U) == 0U;
    bool _S83;
    if(ei_0)
    {
        _S83 = ej_0;
    }
    else
    {
        _S83 = false;
    }
    if(_S83)
    {
        float3 _S84 = t_target_even_0(pool_7, d_15, i_14, j_5, kernelContext_23);
        return _S84;
    }
    if(!ei_0)
    {
        _S83 = ej_0;
    }
    else
    {
        _S83 = false;
    }
    if(_S83)
    {
        float3 _S85 = t_target_even_0(pool_7, d_15, i_14 - 1U, j_5, kernelContext_23);
        float3 _S86 = t_target_even_0(pool_7, d_15, i_14 + 1U, j_5, kernelContext_23);
        return float3(0.5f)  * (_S85 + _S86);
    }
    if(ei_0)
    {
        _S83 = !ej_0;
    }
    else
    {
        _S83 = false;
    }
    if(_S83)
    {
        float3 _S87 = t_target_even_0(pool_7, d_15, i_14, j_5 - 1U, kernelContext_23);
        float3 _S88 = t_target_even_0(pool_7, d_15, i_14, j_5 + 1U, kernelContext_23);
        return float3(0.5f)  * (_S87 + _S88);
    }
    float3 _S89 = t_target_even_0(pool_7, d_15, i_14 - 1U, j_5 - 1U, kernelContext_23);
    float3 _S90 = t_target_even_0(pool_7, d_15, i_14 + 1U, j_5 + 1U, kernelContext_23);
    return float3(0.5f)  * (_S89 + _S90);
}

float3 t_grid_vertex_0(uint pool_8, const TerrainDraw_natural_0 thread* d_16, uint i_15, uint j_6, KernelContext_0 thread* kernelContext_24)
{
    uint coarse_0 = ((uint4(d_16->slots_0) ).z) & 15U;
    if(coarse_0 != 0U)
    {
        bool _S91;
        if((coarse_0 & 1U) != 0U)
        {
            _S91 = i_15 == 0U;
        }
        else
        {
            _S91 = false;
        }
        if(_S91)
        {
            float3 _S92 = t_coarse_0(pool_8, d_16, 0U, j_6, kernelContext_24);
            return _S92;
        }
        if((coarse_0 & 2U) != 0U)
        {
            _S91 = i_15 == 64U;
        }
        else
        {
            _S91 = false;
        }
        if(_S91)
        {
            float3 _S93 = t_coarse_0(pool_8, d_16, 1U, j_6, kernelContext_24);
            return _S93;
        }
        if((coarse_0 & 4U) != 0U)
        {
            _S91 = j_6 == 0U;
        }
        else
        {
            _S91 = false;
        }
        if(_S91)
        {
            float3 _S94 = t_coarse_0(pool_8, d_16, 2U, i_15, kernelContext_24);
            return _S94;
        }
        if((coarse_0 & 8U) != 0U)
        {
            _S91 = j_6 == 64U;
        }
        else
        {
            _S91 = false;
        }
        if(_S91)
        {
            float3 _S95 = t_coarse_0(pool_8, d_16, 3U, i_15, kernelContext_24);
            return _S95;
        }
    }
    float3 _S96 = t_base_0(pool_8, d_16, i_15, j_6, kernelContext_24);
    float m_3 = t_morph_0((float4(d_16->origin_0) ).xyz + _S96, (float4(d_16->morph_0) ).xy);
    if(m_3 <= 0.0f)
    {
        return _S96;
    }
    float3 _S97 = t_target_0(pool_8, d_16, i_15, j_6, kernelContext_24);
    return mix(_S96, _S97, float3(m_3) );
}

float3 t_vertex_0(uint pool_9, const TerrainDraw_natural_0 thread* d_17, uint v_0, KernelContext_0 thread* kernelContext_25)
{
    if(v_0 < 4225U)
    {
        float3 _S98 = (float4(d_17->origin_0) ).xyz;
        float3 _S99 = t_grid_vertex_0(pool_9, d_17, v_0 % 65U, v_0 / 65U, kernelContext_25);
        return _S98 + _S99;
    }
    uint2 ij_0 = t_ring_0(v_0 - 4225U);
    float3 _S100 = t_grid_vertex_0(pool_9, d_17, ij_0.x, ij_0.y, kernelContext_25);
    float depth_0;
    if((((uint4(d_17->slots_0) ).z) & 64U) != 0U)
    {
        depth_0 = (float4(d_17->origin_0) ).w;
    }
    else
    {
        depth_0 = 0.0f;
    }
    float3 _S101 = (float4(d_17->origin_0) ).xyz;
    float3 _S102 = _S101 + _S100;
    float4 _S103 = float4(d_17->dir_c_0) ;
    return _S102 - normalize(_S102 - (_S101 - _S103.xyz * float3(_S103.w) )) * float3(depth_0) ;
}

float2 ray_bary_0(float3 dir_3, float3 p0_0, float3 e1_0, float3 e2_0)
{
    float3 pv_0 = cross(dir_3, e2_0);
    float det_0 = dot(e1_0, pv_0);
    if((abs(det_0)) <= 9.999999960041972e-13f)
    {
        return float2(0.3333333432674408f) ;
    }
    float inv_0 = 1.0f / det_0;
    float3 tv_1 = - p0_0;
    return float2(dot(tv_1, pv_0) * inv_0, dot(dir_3, cross(tv_1, e1_0)) * inv_0);
}

float2 t_vertex_grid_0(uint v_1)
{
    if(v_1 < 4225U)
    {
        return float2(float(v_1 % 65U), float(v_1 / 65U));
    }
    return float2(t_ring_0(v_1 - 4225U));
}

float3 t_lattice_f_0(const TerrainDraw_natural_0 thread* d_18, float2 g_1, float h_15, float3 thread* dir_4)
{
    float4 _S104 = float4(d_18->shape_0) ;
    float _S105 = _S104.x;
    float tu_1 = tan((g_1.x - 32.0f) * _S105);
    float tv_2 = tan((g_1.y - 32.0f) * _S105);
    float4 _S106 = float4(d_18->fu_0) ;
    float _S107 = _S106.w;
    float4 _S108 = float4(d_18->fv_0) ;
    float _S109 = _S108.w;
    float3 delta_1 = float3((tu_1 * (1.0f + _S107 * _S107) / (1.0f - _S107 * tu_1)))  * _S106.xyz + float3((tv_2 * (1.0f + _S109 * _S109) / (1.0f - _S109 * tv_2)))  * _S108.xyz;
    float4 _S110 = float4(d_18->cc_0) ;
    float lc_1 = _S110.w;
    float3 _S111 = _S110.xyz;
    float k_2 = 2.0f * dot(_S111, delta_1) + dot(delta_1, delta_1);
    float l_1 = sqrt(lc_1 * lc_1 + k_2);
    float3 dd_1 = delta_1 / float3(l_1)  - _S111 * float3((k_2 / ((lc_1 + l_1) * l_1 * lc_1))) ;
    float4 _S112 = float4(d_18->dir_c_0) ;
    float3 _S113 = _S112.xyz;
    float3 _S114 = _S113 + dd_1;
    *dir_4 = _S114;
    float3 _S115 = (float4(d_18->polar_0) ).xyz;
    float dzc_1 = dot(_S113, _S115);
    float ddz_1 = dot(dd_1, _S115);
    float dz_1 = dzc_1 + ddz_1;
    float e_6 = _S104.z;
    float s_4 = sqrt(1.0f - e_6 * dz_1 * dz_1);
    float sc_1 = _S104.w;
    float dr_1 = _S104.y * e_6 * ddz_1 * (dz_1 + dzc_1) / (s_4 * sc_1 * (s_4 + sc_1));
    return dd_1 * float3((_S112.w + dr_1))  + _S113 * float3(dr_1)  + _S114 * float3(h_15) ;
}

float3 t_lattice_f_1(const TerrainDraw_natural_0 thread* d_19, float2 g_2, float h_16, float3 thread* dir_5)
{
    float4 _S116 = float4(d_19->shape_0) ;
    float _S117 = _S116.x;
    float tu_2 = tan((g_2.x - 32.0f) * _S117);
    float tv_3 = tan((g_2.y - 32.0f) * _S117);
    float4 _S118 = float4(d_19->fu_0) ;
    float _S119 = _S118.w;
    float4 _S120 = float4(d_19->fv_0) ;
    float _S121 = _S120.w;
    float3 delta_2 = float3((tu_2 * (1.0f + _S119 * _S119) / (1.0f - _S119 * tu_2)))  * _S118.xyz + float3((tv_3 * (1.0f + _S121 * _S121) / (1.0f - _S121 * tv_3)))  * _S120.xyz;
    float4 _S122 = float4(d_19->cc_0) ;
    float lc_2 = _S122.w;
    float3 _S123 = _S122.xyz;
    float k_3 = 2.0f * dot(_S123, delta_2) + dot(delta_2, delta_2);
    float l_2 = sqrt(lc_2 * lc_2 + k_3);
    float3 dd_2 = delta_2 / float3(l_2)  - _S123 * float3((k_3 / ((lc_2 + l_2) * l_2 * lc_2))) ;
    float4 _S124 = float4(d_19->dir_c_0) ;
    float3 _S125 = _S124.xyz;
    float3 _S126 = _S125 + dd_2;
    *dir_5 = _S126;
    float3 _S127 = (float4(d_19->polar_0) ).xyz;
    float dzc_2 = dot(_S125, _S127);
    float ddz_2 = dot(dd_2, _S127);
    float dz_2 = dzc_2 + ddz_2;
    float e_7 = _S116.z;
    float s_5 = sqrt(1.0f - e_7 * dz_2 * dz_2);
    float sc_2 = _S116.w;
    float dr_2 = _S116.y * e_7 * ddz_2 * (dz_2 + dzc_2) / (s_5 * sc_2 * (s_5 + sc_2));
    return dd_2 * float3((_S124.w + dr_2))  + _S125 * float3(dr_2)  + _S126 * float3(h_16) ;
}

float t_sample_spacing_0(const TerrainDraw_natural_0 thread* d_20, float2 g_3)
{
    float2 _S128 = float2(0.5f, 0.0f);
    thread float3 dir_6;
    float3 _S129 = t_lattice_f_0(d_20, g_3 + _S128, 0.0f, &dir_6);
    float3 _S130 = t_lattice_f_0(d_20, g_3 - _S128, 0.0f, &dir_6);
    return length(_S129 - _S130) / 4.0f;
}

float t_page_at_0(uint pool_10, const TerrainDraw_natural_0 thread* d_21, int a_3, int b_4, KernelContext_0 thread* kernelContext_26)
{
    float4 _S131 = float4(d_21->hq_0) ;
    float _S132 = t_page_0(pool_10, (uint4(d_21->slots_0) ).x, uint(clamp(a_3, int(0), int(256))), uint(clamp(b_4, int(0), int(256))), _S131.x, _S131.y, kernelContext_26);
    return _S132;
}

void t_surface_0(uint pool_11, const TerrainDraw_natural_0 thread* d_22, float2 g_4, int r_3, float thread* h_17, float2 thread* dh_0, KernelContext_0 thread* kernelContext_27)
{
    float2 _S133 = float2(4.0f) ;
    float2 _S134 = float2(0.0f) ;
    float2 s_6 = clamp(g_4 * _S133, _S134, float2(256.0f) );
    int2 _S135 = int2(min(floor(s_6), float2(255.0f) ));
    float2 _S136 = s_6 - float2(_S135);
    *h_17 = 0.0f;
    *dh_0 = _S134;
    uint k_4 = 0U;
    for(;;)
    {
        if(k_4 < 4U)
        {
        }
        else
        {
            break;
        }
        uint _S137 = k_4 & 1U;
        uint _S138 = k_4 >> 1U;
        int2 q_3 = _S135 + int2(int(_S137), int(_S138));
        float _S139;
        if(_S137 != 0U)
        {
            _S139 = _S136.x;
        }
        else
        {
            _S139 = 1.0f - _S136.x;
        }
        float _S140;
        if(_S138 != 0U)
        {
            _S140 = _S136.y;
        }
        else
        {
            _S140 = 1.0f - _S136.y;
        }
        float w_3 = _S139 * _S140;
        int2 _S141 = int2(r_3) ;
        int2 _S142 = max(q_3 - _S141, int2(int(0)) );
        int2 _S143 = min(q_3 + _S141, int2(int(256)) );
        int _S144 = _S143.x;
        int _S145 = q_3.y;
        float _S146 = t_page_at_0(pool_11, d_22, _S144, _S145, kernelContext_27);
        int _S147 = _S142.x;
        float _S148 = t_page_at_0(pool_11, d_22, _S147, _S145, kernelContext_27);
        float gx_0 = (_S146 - _S148) / float(_S144 - _S147);
        int _S149 = q_3.x;
        int _S150 = _S143.y;
        float _S151 = t_page_at_0(pool_11, d_22, _S149, _S150, kernelContext_27);
        int _S152 = _S142.y;
        float _S153 = t_page_at_0(pool_11, d_22, _S149, _S152, kernelContext_27);
        float gy_0 = (_S151 - _S153) / float(_S150 - _S152);
        float _S154 = t_page_at_0(pool_11, d_22, _S149, _S145, kernelContext_27);
        *h_17 = *h_17 + w_3 * _S154;
        *dh_0 = *dh_0 + float2(w_3)  * float2(gx_0, gy_0);
        k_4 = k_4 + 1U;
    }
    *dh_0 = *dh_0 * _S133;
    return;
}

float3 t_smooth_normal_0(uint pool_12, const TerrainDraw_natural_0 thread* d_23, float2 g_5, int r_4, float3 thread* up_1, float thread* h_18, KernelContext_0 thread* kernelContext_28)
{
    thread float2 dh_1;
    t_surface_0(pool_12, d_23, g_5, r_4, h_18, &dh_1, kernelContext_28);
    float2 _S155 = float2(0.25f, 0.0f);
    thread float3 dir_7;
    float3 _S156 = t_lattice_f_0(d_23, g_5 + _S155, *h_18, &dir_7);
    float3 _S157 = t_lattice_f_0(d_23, g_5 - _S155, *h_18, &dir_7);
    float3 pu_0 = _S156 - _S157;
    float2 _S158 = float2(0.0f, 0.25f);
    float3 _S159 = t_lattice_f_0(d_23, g_5 + _S158, *h_18, &dir_7);
    float3 _S160 = t_lattice_f_0(d_23, g_5 - _S158, *h_18, &dir_7);
    float3 pv_1 = _S159 - _S160;
    float3 _S161 = t_lattice_f_1(d_23, g_5, *h_18, up_1);
    float3 _S162 = normalize(*up_1);
    *up_1 = _S162;
    float3 _S163 = float3(0.5f) ;
    float3 n_1 = normalize(cross(pu_0 / _S163 + _S162 * float3(dh_1.x) , pv_1 / _S163 + _S162 * float3(dh_1.y) ));
    float3 _S164;
    if((dot(n_1, _S162)) < 0.0f)
    {
        _S164 = - n_1;
    }
    else
    {
        _S164 = n_1;
    }
    return _S164;
}

float3 terrain_albedo_0(float h_19, float slope_cos_0)
{
    return mix(mix(mix(float3(0.054999999701976776f, 0.075000002980232239f, 0.039999999105930328f), float3(0.125f, 0.14000000059604645f, 0.070000000298023224f), float3(smoothstep(1950.0f, 2350.0f, h_19)) ), float3(0.20999999344348907f, 0.20000000298023224f, 0.18500000238418579f), float3(smoothstep(0.81999999284744263f, 0.72000002861022949f, slope_cos_0)) ), float3(0.80000001192092896f, 0.81999999284744263f, 0.86000001430511475f), float3((smoothstep(2750.0f, 3050.0f, h_19) * smoothstep(0.55000001192092896f, 0.69999998807907104f, slope_cos_0))) );
}

float length_squared_0(float3 v_2)
{
    return dot(v_2, v_2);
}

float cascade_extent_0(FrameData_natural_0 device* f_17, uint c_6, float3 p_3)
{
    float4 s_7 = (((float4(p_3, 1.0f)) * (matrix<float,int(4),int(4)> ((&(&f_17->light_0)->shadow_1)->data_3[c_6].data_1[int(0)][int(0)], (&(&f_17->light_0)->shadow_1)->data_3[c_6].data_1[int(1)][int(0)], (&(&f_17->light_0)->shadow_1)->data_3[c_6].data_1[int(2)][int(0)], (&(&f_17->light_0)->shadow_1)->data_3[c_6].data_1[int(3)][int(0)], (&(&f_17->light_0)->shadow_1)->data_3[c_6].data_1[int(0)][int(1)], (&(&f_17->light_0)->shadow_1)->data_3[c_6].data_1[int(1)][int(1)], (&(&f_17->light_0)->shadow_1)->data_3[c_6].data_1[int(2)][int(1)], (&(&f_17->light_0)->shadow_1)->data_3[c_6].data_1[int(3)][int(1)], (&(&f_17->light_0)->shadow_1)->data_3[c_6].data_1[int(0)][int(2)], (&(&f_17->light_0)->shadow_1)->data_3[c_6].data_1[int(1)][int(2)], (&(&f_17->light_0)->shadow_1)->data_3[c_6].data_1[int(2)][int(2)], (&(&f_17->light_0)->shadow_1)->data_3[c_6].data_1[int(3)][int(2)], (&(&f_17->light_0)->shadow_1)->data_3[c_6].data_1[int(0)][int(3)], (&(&f_17->light_0)->shadow_1)->data_3[c_6].data_1[int(1)][int(3)], (&(&f_17->light_0)->shadow_1)->data_3[c_6].data_1[int(2)][int(3)], (&(&f_17->light_0)->shadow_1)->data_3[c_6].data_1[int(3)][int(3)]))));
    float _S165 = s_7.z;
    bool _S166;
    if(_S165 < 0.0f)
    {
        _S166 = true;
    }
    else
    {
        _S166 = _S165 > 1.0f;
    }
    float _S167;
    if(_S166)
    {
        _S167 = 1.0e+09f;
    }
    else
    {
        _S167 = max(abs(s_7.x), abs(s_7.y));
    }
    return _S167;
}

float pcf_radius_texels_0(FrameData_natural_0 device* f_18, float texel_0)
{
    return clamp((float4((&f_18->light_0)->sky_0) ).z / texel_0, 1.0f, max((float4((&f_18->light_0)->sky_0) ).w, 1.0f));
}

float ed_depth_read_0(uint i_16, uint2 p_4, KernelContext_0 thread* kernelContext_29)
{
    float _S168 = ((reinterpret_cast<device depth2d<float, access::sample>*>((kernelContext_29->ed_textures_0))[(i_16)].read((p_4))));
    return _S168;
}

float cascade_lookup_0(FrameData_natural_0 device* f_19, uint atlas_0, uint c_7, float3 p_5, float3 ng_0, float2 px_1, KernelContext_0 thread* kernelContext_30)
{
    float4 _S169 = float4((&(&f_19->light_0)->cascade_0)->data_4[c_7]) ;
    float tile_0 = (float4((&f_19->light_0)->cfg_0) ).y;
    float _S170 = _S169.x;
    float _S171 = pcf_radius_texels_0(f_19, _S170);
    float sp_0 = _S171 * 0.5f;
    float nol_0 = saturate(dot(ng_0, (float4(f_19->sun_dir_0) ).xyz));
    float4 s_8 = (((float4(p_5 + ng_0 * float3((_S170 * ((float4((&f_19->light_0)->filter_0) ).z + 2.0f * sqrt(1.0f - nol_0 * nol_0)))) , 1.0f)) * (matrix<float,int(4),int(4)> ((&(&f_19->light_0)->shadow_1)->data_3[c_7].data_1[int(0)][int(0)], (&(&f_19->light_0)->shadow_1)->data_3[c_7].data_1[int(1)][int(0)], (&(&f_19->light_0)->shadow_1)->data_3[c_7].data_1[int(2)][int(0)], (&(&f_19->light_0)->shadow_1)->data_3[c_7].data_1[int(3)][int(0)], (&(&f_19->light_0)->shadow_1)->data_3[c_7].data_1[int(0)][int(1)], (&(&f_19->light_0)->shadow_1)->data_3[c_7].data_1[int(1)][int(1)], (&(&f_19->light_0)->shadow_1)->data_3[c_7].data_1[int(2)][int(1)], (&(&f_19->light_0)->shadow_1)->data_3[c_7].data_1[int(3)][int(1)], (&(&f_19->light_0)->shadow_1)->data_3[c_7].data_1[int(0)][int(2)], (&(&f_19->light_0)->shadow_1)->data_3[c_7].data_1[int(1)][int(2)], (&(&f_19->light_0)->shadow_1)->data_3[c_7].data_1[int(2)][int(2)], (&(&f_19->light_0)->shadow_1)->data_3[c_7].data_1[int(3)][int(2)], (&(&f_19->light_0)->shadow_1)->data_3[c_7].data_1[int(0)][int(3)], (&(&f_19->light_0)->shadow_1)->data_3[c_7].data_1[int(1)][int(3)], (&(&f_19->light_0)->shadow_1)->data_3[c_7].data_1[int(2)][int(3)], (&(&f_19->light_0)->shadow_1)->data_3[c_7].data_1[int(3)][int(3)]))));
    float2 _S172 = float2(s_8.x * 0.5f + 0.5f, 0.5f - s_8.y * 0.5f) * float2(tile_0) ;
    float ign_0 = fract(52.982917785644531f * fract(dot(px_1, float2(0.067110560834407806f, 0.0058371499180793762f))));
    float2 _S173 = float2(0.5f) ;
    float2 _S174 = (float2(ign_0, fract(ign_0 * 7.309999942779541f + 0.37000000476837158f)) - _S173) * float2(0.0f) ;
    float3 ns_0 = (((float4(ng_0, 0.0f)) * (matrix<float,int(4),int(4)> ((&(&f_19->light_0)->shadow_1)->data_3[c_7].data_1[int(0)][int(0)], (&(&f_19->light_0)->shadow_1)->data_3[c_7].data_1[int(1)][int(0)], (&(&f_19->light_0)->shadow_1)->data_3[c_7].data_1[int(2)][int(0)], (&(&f_19->light_0)->shadow_1)->data_3[c_7].data_1[int(3)][int(0)], (&(&f_19->light_0)->shadow_1)->data_3[c_7].data_1[int(0)][int(1)], (&(&f_19->light_0)->shadow_1)->data_3[c_7].data_1[int(1)][int(1)], (&(&f_19->light_0)->shadow_1)->data_3[c_7].data_1[int(2)][int(1)], (&(&f_19->light_0)->shadow_1)->data_3[c_7].data_1[int(3)][int(1)], (&(&f_19->light_0)->shadow_1)->data_3[c_7].data_1[int(0)][int(2)], (&(&f_19->light_0)->shadow_1)->data_3[c_7].data_1[int(1)][int(2)], (&(&f_19->light_0)->shadow_1)->data_3[c_7].data_1[int(2)][int(2)], (&(&f_19->light_0)->shadow_1)->data_3[c_7].data_1[int(3)][int(2)], (&(&f_19->light_0)->shadow_1)->data_3[c_7].data_1[int(0)][int(3)], (&(&f_19->light_0)->shadow_1)->data_3[c_7].data_1[int(1)][int(3)], (&(&f_19->light_0)->shadow_1)->data_3[c_7].data_1[int(2)][int(3)], (&(&f_19->light_0)->shadow_1)->data_3[c_7].data_1[int(3)][int(3)])))).xyz;
    float rl_0 = _S170 * tile_0 * 0.5f;
    float _S175 = _S169.y;
    float _S176 = ns_0.z * _S175;
    float izn_0 = 1.0f / max(abs(_S176), 0.11999999731779099f);
    float _S177 = float((int(sign((_S176)))));
    float gx_1 = clamp(- (ns_0.x * rl_0) * izn_0 * _S177, -8.0f, 8.0f);
    float gy_1 = clamp(- (ns_0.y * rl_0) * izn_0 * _S177, -8.0f, 8.0f);
    float _S178 = s_8.z + ((float4((&f_19->light_0)->filter_0) ).w + 0.5f * (abs(gx_1) + abs(gy_1))) * _S170 / _S175;
    float _S179 = _S170 / _S175;
    int2 _S180 = int2(_S169.zw);
    int2 _S181 = _S180 + int2((int(tile_0) - int(1))) ;
    float _S182 = 1.0f / (_S171 + sp_0);
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
        float _S183 = 1.0f - abs(oy_0) * _S182;
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
            float w_4 = (1.0f - abs(ox_0) * _S182) * _S183;
            float2 tp_1 = floor(_S172 + float2(ox_0, oy_0) + _S174);
            float2 d_24 = tp_1 + _S173 - _S172;
            float _S184 = ed_depth_read_0(atlas_0, uint2(clamp(_S180 + int2(tp_1), _S180, _S181)), kernelContext_30);
            float _S185;
            if((_S178 + (gx_1 * d_24.x - gy_1 * d_24.y) * _S179) >= _S184)
            {
                _S185 = 1.0f;
            }
            else
            {
                _S185 = 0.0f;
            }
            float lit_1 = lit_0 + _S185 * w_4;
            float wsum_1 = wsum_0 + w_4;
            x_0 = x_0 + int(1);
            lit_0 = lit_1;
            wsum_0 = wsum_1;
        }
        y_0 = y_0 + int(1);
    }
    return lit_0 / wsum_0;
}

float sun_shadow_0(FrameData_natural_0 device* f_20, uint atlas_1, float3 p_6, float3 ng_1, float dist_0, float2 px_2, KernelContext_0 thread* kernelContext_31)
{
    uint n_2 = uint((float4((&f_20->light_0)->cfg_0) ).x);
    bool _S186;
    if(((uint4((&f_20->light_0)->flags_0) ).x) == 0U)
    {
        _S186 = true;
    }
    else
    {
        _S186 = n_2 == 0U;
    }
    if(_S186)
    {
        _S186 = true;
    }
    else
    {
        _S186 = dist_0 >= ((float4((&f_20->light_0)->filter_0) ).y);
    }
    if(_S186)
    {
        return 1.0f;
    }
    float _S187 = 1.0f - (float4((&f_20->light_0)->cfg_0) ).w * 2.0f / (float4((&f_20->light_0)->cfg_0) ).y;
    float _S188 = (float4((&f_20->light_0)->cfg_0) ).z;
    uint c_8 = 0U;
    for(;;)
    {
        if(c_8 < n_2)
        {
        }
        else
        {
            break;
        }
        float _S189 = cascade_extent_0(f_20, c_8, p_6);
        float e_8 = _S189 / _S187;
        if(e_8 > 1.0f)
        {
            c_8 = c_8 + 1U;
            continue;
        }
        float _S190 = cascade_lookup_0(f_20, atlas_1, c_8, p_6, ng_1, px_2, kernelContext_31);
        float blend_0 = smoothstep(1.0f - _S188, 1.0f, e_8);
        float v_3;
        if(blend_0 > 0.0f)
        {
            uint _S191 = c_8 + 1U;
            if(_S191 < n_2)
            {
                float _S192 = cascade_extent_0(f_20, _S191, p_6);
                _S186 = (_S192 / _S187) <= 1.0f;
            }
            else
            {
                _S186 = false;
            }
            if(_S186)
            {
                float _S193 = cascade_lookup_0(f_20, atlas_1, _S191, p_6, ng_1, px_2, kernelContext_31);
                v_3 = _S193;
            }
            else
            {
                v_3 = 1.0f;
            }
            v_3 = mix(_S190, v_3, blend_0);
        }
        else
        {
            v_3 = _S190;
        }
        return mix(v_3, 1.0f, smoothstep(0.0f, 1.0f, saturate((dist_0 - (float4((&f_20->light_0)->filter_0) ).x) / max((float4((&f_20->light_0)->filter_0) ).y - (float4((&f_20->light_0)->filter_0) ).x, 0.0010000000474974513f))));
    }
    return 1.0f;
}

float2 ao_upsample_0(FrameData_natural_0 device* f_21, uint ao_buf_0, float2 pixel_1, float zlin_0, KernelContext_0 thread* kernelContext_32)
{
    float sv_0;
    if(((uint4((&f_21->light_0)->flags_0) ).y) == 0U)
    {
        return float2(1.0f) ;
    }
    packed_float4 device* _S194 = ed_buf_7(ao_buf_0, kernelContext_32);
    uint _S195 = uint((float4((&f_21->light_0)->ao_1) ).y);
    uint _S196 = uint((float4((&f_21->light_0)->ao_1) ).z);
    float2 _S197 = float2(0.5f) ;
    float2 h_20 = (pixel_1 - _S197) * _S197;
    float2 b_5 = floor(h_20);
    float2 _S198 = h_20 - b_5;
    float2 _S199 = float2(0.0f) ;
    int y_1 = int(0);
    float2 sum_0 = _S199;
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
            int2 t_3 = clamp(int2(b_5) + int2(x_1, y_1), int2(int(0)) , int2(int(_S195) - int(1), int(_S196) - int(1)));
            float4 _S200 = float4(*(_S194 + (uint(t_3.y) * _S195 + uint(t_3.x)))) ;
            if(x_1 == int(0))
            {
                sv_0 = 1.0f - _S198.x;
            }
            else
            {
                sv_0 = _S198.x;
            }
            float _S201;
            if(y_1 == int(0))
            {
                _S201 = 1.0f - _S198.y;
            }
            else
            {
                _S201 = _S198.y;
            }
            float w_5 = max(sv_0 * _S201, 0.0010000000474974513f) * (1.0f / (0.0010000000474974513f + abs(_S200.z - zlin_0) / max(zlin_0, 0.0010000000474974513f) * 50.0f));
            float2 sum_1 = sum_0 + _S200.xy * float2(w_5) ;
            float wsum_3 = wsum_2 + w_5;
            x_1 = x_1 + int(1);
            sum_0 = sum_1;
            wsum_2 = wsum_3;
        }
        y_1 = y_1 + int(1);
    }
    float2 r_5;
    if(wsum_2 > 0.0f)
    {
        r_5 = sum_0 / float2(wsum_2) ;
    }
    else
    {
        r_5 = float2(1.0f) ;
    }
    if(((float4((&f_21->light_0)->skyvis_0) ).x) > 0.0f)
    {
        sv_0 = saturate(r_5.y);
    }
    else
    {
        sv_0 = 1.0f;
    }
    return float2(pow(saturate(r_5.x), (float4((&f_21->light_0)->ao_1) ).x), sv_0);
}

float atm_height_at_0(float rb_5, float h_21, float mu_12, float t_4)
{
    float q_4 = t_4 * t_4 + 2.0f * (rb_5 + h_21) * mu_12 * t_4 + h_21 * (2.0f * rb_5 + h_21);
    return q_4 / (sqrt(max(rb_5 * rb_5 + q_4, 0.0f)) + rb_5);
}

float atm_earth_shadow_0(FrameData_natural_0 device* f_22, float h_22, float mu_s_4)
{
    float _S202 = atm_rb_1(f_22);
    float ch_0 = atm_cos_horizon_0(_S202, h_22);
    float s_9 = 0.0046999999321997166f * sqrt(max(1.0f - ch_0 * ch_0, 0.0f)) + 9.9999997473787516e-06f;
    return smoothstep(- s_9, s_9, mu_s_4 - ch_0);
}

float3 atm_sun_transmittance_0(FrameData_natural_0 device* f_23, float h_23, float mu_s_5, KernelContext_0 thread* kernelContext_33)
{
    float3 _S203 = atm_transmittance_1(f_23, h_23, mu_s_5, kernelContext_33);
    float _S204 = atm_earth_shadow_0(f_23, h_23, mu_s_5);
    return _S203 * float3(_S204) ;
}

float3 atmos_sun_at_0(FrameData_natural_0 device* f_24, float3 p_7, KernelContext_0 thread* kernelContext_34)
{
    float t_5 = length(p_7) * 0.0010000000474974513f;
    float3 d_25;
    if(t_5 > 0.0f)
    {
        d_25 = p_7 * float3((0.0010000000474974513f / t_5)) ;
    }
    else
    {
        d_25 = (float4((&f_24->atmos_0)->up_0) ).xyz;
    }
    float _S205 = atm_rb_0(f_24);
    float h0_0 = (float4((&f_24->atmos_0)->radii_0) ).z;
    float h_24 = atm_height_at_0(_S205, h0_0, dot(d_25, (float4((&f_24->atmos_0)->up_0) ).xyz), t_5);
    float3 _S206 = (float4((&f_24->atmos_0)->sun_0) ).xyz;
    float3 _S207 = atm_sun_transmittance_0(f_24, h_24, clamp(((_S205 + h0_0) * (float4((&f_24->atmos_0)->up_0) ).w + t_5 * dot(d_25, (float4(f_24->sun_dir_0) ).xyz)) / (_S205 + h_24), -1.0f, 1.0f), kernelContext_34);
    return _S206 * _S207;
}

float3 sun_light_at_0(FrameData_natural_0 device* f_25, float3 p_8, KernelContext_0 thread* kernelContext_35)
{
    bool _S208 = atmos_on_0(f_25);
    float3 _S209;
    if(_S208)
    {
        float3 _S210 = atmos_sun_at_0(f_25, p_8, kernelContext_35);
        _S209 = _S210;
    }
    else
    {
        _S209 = (float4(f_25->sun_color_0) ).xyz;
    }
    return _S209;
}

float3 f_schlick_0(float3 f0_0, float voh_0)
{
    return f0_0 + (float3(1.0f)  - f0_0) * float3(pow(1.0f - voh_0, 5.0f)) ;
}

float d_ggx_0(float noh_0, float a_4)
{
    float a2_0 = a_4 * a_4;
    float d_26 = noh_0 * noh_0 * (a2_0 - 1.0f) + 1.0f;
    return a2_0 / (3.1415927410125732f * d_26 * d_26);
}

float v_smith_0(float nov_0, float nol_1, float a_5)
{
    float a2_1 = a_5 * a_5;
    float _S211 = 1.0f - a2_1;
    return 0.5f / max(nol_1 * sqrt(nov_0 * nov_0 * _S211 + a2_1) + nov_0 * sqrt(nol_1 * nol_1 * _S211 + a2_1), 9.9999997473787516e-06f);
}

float2 env_brdf_0(float nov_1, float rough_0)
{
    float4 r_6 = float4(rough_0)  * float4(-1.0f, -0.027499999850988388f, -0.57200002670288086f, 0.02199999988079071f) + float4(1.0f, 0.042500000447034836f, 1.0399999618530273f, -0.039999999105930328f);
    float _S212 = r_6.x;
    return float2(-1.0399999618530273f, 1.0399999618530273f) * float2((min(_S212 * _S212, exp2(-9.2799997329711914f * nov_1)) * _S212 + r_6.y))  + r_6.zw;
}

float3 sky_env_0(FrameData_natural_0 device* f_26, float3 d_27, KernelContext_0 thread* kernelContext_36)
{
    bool _S213 = atmos_on_1(f_26);
    float3 _S214;
    if(_S213)
    {
        float3 _S215 = atmos_sky_1(f_26, d_27, kernelContext_36);
        _S214 = _S215;
    }
    else
    {
        float3 _S216 = sky_rad_1(f_26, d_27, 0.0f);
        _S214 = _S216;
    }
    return _S214;
}

float3 sh_irradiance_0(FrameData_natural_0 device* f_27, float3 n_3, KernelContext_0 thread* kernelContext_37)
{
    thread array<float4, int(9)> s_10;
    packed_float4 device* _S217 = ed_buf_7((uint4((&f_27->atmos_0)->bufs2_0) ).z, kernelContext_37);
    uint k_5 = 0U;
    for(;;)
    {
        if(k_5 < 9U)
        {
        }
        else
        {
            break;
        }
        bool _S218 = atmos_on_0(f_27);
        float4 _S219;
        if(_S218)
        {
            _S219 = float4(*(_S217 + (288U + k_5))) ;
        }
        else
        {
            _S219 = float4((&(&f_27->light_0)->sh_0)->data_5[k_5]) ;
        }
        s_10[k_5] = _S219;
        k_5 = k_5 + 1U;
    }
    float _S220 = n_3.y;
    float _S221 = n_3.z;
    float _S222 = n_3.x;
    return max(s_10[int(0)].xyz + (s_10[int(1)].xyz * float3(_S220)  + s_10[int(2)].xyz * float3(_S221)  + s_10[int(3)].xyz * float3(_S222) ) + (s_10[int(4)].xyz * float3((_S222 * _S220))  + s_10[int(5)].xyz * float3((_S220 * _S221))  + s_10[int(6)].xyz * float3((3.0f * _S221 * _S221 - 1.0f)) ) + (s_10[int(7)].xyz * float3((_S222 * _S221))  + s_10[int(8)].xyz * float3((_S222 * _S222 - _S220 * _S220)) ), float3(0.0f) );
}

float3 hemi_ambient_0(FrameData_natural_0 device* f_28, float3 n_4)
{
    return mix((float4(f_28->ground_color_0) ).xyz * float3((float4(f_28->sun_dir_0) ).w) , mix((float4(f_28->sky_horizon_0) ).xyz, (float4(f_28->sky_zenith_0) ).xyz, float3(0.5f) ) * float3((float4(f_28->sun_dir_0) ).w) , float3((n_4.y * 0.5f + 0.5f)) );
}

float3 ambient_light_0(FrameData_natural_0 device* f_29, float3 n_5, KernelContext_0 thread* kernelContext_38)
{
    float3 _S223;
    if(((uint4((&f_29->light_0)->flags_0) ).z) != 0U)
    {
        float3 _S224 = sh_irradiance_0(f_29, n_5, kernelContext_38);
        _S223 = _S224;
    }
    else
    {
        float3 _S225 = hemi_ambient_0(f_29, n_5);
        _S223 = _S225;
    }
    return _S223;
}

float3 occluded_ambient_0(FrameData_natural_0 device* f_30, float3 open_0, float sv_1, KernelContext_0 thread* kernelContext_39)
{
    float3 _S226 = ambient_light_0(f_30, float3(0.0f, -1.0f, 0.0f), kernelContext_39);
    return mix(_S226 * float3((float4((&f_30->light_0)->skyvis_0) ).y) , open_0, float3(sv_1) );
}

float3 ao_multibounce_0(FrameData_natural_0 device* f_31, float ao_2, float3 albedo_0)
{
    float3 _S227 = float3(ao_2) ;
    return mix(_S227, max(_S227, ((_S227 * (float3(2.0404000282287598f)  * albedo_0 - float3(0.33239999413490295f) ) + (float3(-4.795100212097168f)  * albedo_0 + float3(0.64170002937316895f) )) * _S227 + (float3(2.7551999092102051f)  * albedo_0 + float3(0.69029998779296875f) )) * _S227), float3((float4((&f_31->light_0)->sky_0) ).y) );
}

float3 shade_pbr_0(FrameData_natural_0 device* f_32, float3 sun_3, float3 albedo_1, float metal_0, float rough_1, float ao_3, float sv_2, float sun_vis_0, float3 n_6, float3 v_4, KernelContext_0 thread* kernelContext_40)
{
    float _S228 = saturate(metal_0);
    float _S229 = clamp(rough_1, 0.039999999105930328f, 1.0f);
    float a_6 = _S229 * _S229;
    float3 l_3 = (float4(f_32->sun_dir_0) ).xyz;
    float3 h_25 = normalize(l_3 + v_4);
    float nol_2 = saturate(dot(n_6, l_3));
    float _S230 = max(dot(n_6, v_4), 9.9999997473787516e-05f);
    float3 f0_1 = mix(float3(0.039999999105930328f) , albedo_1, float3(_S228) );
    float3 diff_col_0 = albedo_1 * float3((1.0f - _S228)) ;
    float3 F_0 = f_schlick_0(f0_1, saturate(dot(v_4, h_25)));
    float3 _S231 = float3(1.0f) ;
    float3 direct_0 = (diff_col_0 * (_S231 - F_0) / float3(3.1415927410125732f)  + float3((d_ggx_0(saturate(dot(n_6, h_25)), a_6) * v_smith_0(_S230, nol_2, a_6)))  * F_0) * sun_3 * float3((nol_2 * sun_vis_0)) ;
    float2 ab_0 = env_brdf_0(_S230, _S229);
    float3 r_7 = reflect(- v_4, n_6);
    float3 env_0;
    if(((uint4((&f_32->light_0)->flags_0) ).z) != 0U)
    {
        float3 _S232 = sky_env_0(f_32, r_7, kernelContext_40);
        float3 _S233 = sh_irradiance_0(f_32, r_7, kernelContext_40);
        env_0 = mix(_S232, _S233, float3(_S229) );
    }
    else
    {
        float3 _S234 = hemi_ambient_0(f_32, r_7);
        env_0 = _S234;
    }
    float3 _S235 = occluded_ambient_0(f_32, env_0, sv_2, kernelContext_40);
    float spec_occ_0 = saturate(pow(_S230 + ao_3, exp2(-16.0f * a_6 - 1.0f)) - 1.0f + ao_3);
    float3 f_env_0 = f0_1 * float3(ab_0.x)  + float3(ab_0.y) ;
    float3 _S236 = diff_col_0 * (_S231 - f_env_0);
    float3 _S237 = ambient_light_0(f_32, n_6, kernelContext_40);
    float3 _S238 = occluded_ambient_0(f_32, _S237, sv_2, kernelContext_40);
    float3 _S239 = _S236 * _S238;
    float3 _S240 = ao_multibounce_0(f_32, ao_3, diff_col_0);
    return direct_0 + (_S239 * _S240 + _S235 * f_env_0 * float3(spec_occ_0) );
}

float atm_ap_slice_0(FrameData_natural_0 device* f_33, float t_6)
{
    float n_7 = (float4((&f_33->atmos_0)->ap_0) ).x;
    return log2(1.0f + max(t_6, 0.0f) / n_7) / log2(1.0f + (float4((&f_33->atmos_0)->ap_0) ).y / n_7) * 32.0f;
}

void atm_froxel_xy_0(packed_float4 device* l_4, packed_float4 device* t_7, float2 g_6, uint2 i_17, uint k_6, float3 thread* lo_0, float3 thread* to_0)
{
    uint b_6 = (k_6 * 32U + i_17.y) * 32U + i_17.x;
    uint _S241 = b_6 + 1U;
    float4 _S242 = float4(g_6.x) ;
    uint _S243 = b_6 + 32U;
    uint _S244 = _S243 + 1U;
    float4 t0_0 = mix(float4(*(t_7 + b_6)) , float4(*(t_7 + _S241)) , _S242);
    float4 t1_0 = mix(float4(*(t_7 + _S243)) , float4(*(t_7 + _S244)) , _S242);
    float4 _S245 = float4(g_6.y) ;
    *lo_0 = mix(mix(float4(*(l_4 + b_6)) , float4(*(l_4 + _S241)) , _S242), mix(float4(*(l_4 + _S243)) , float4(*(l_4 + _S244)) , _S242), _S245).xyz;
    *to_0 = mix(t0_0, t1_0, _S245).xyz;
    return;
}

void atmos_aerial_0(FrameData_natural_0 device* f_34, float3 p_9, float2 px_3, float3 thread* l_5, float3 thread* tr_0, KernelContext_0 thread* kernelContext_41)
{
    *l_5 = float3(0.0f) ;
    float3 _S246 = float3(1.0f) ;
    *tr_0 = _S246;
    float t_8 = length(p_9) * 0.0010000000474974513f;
    if(t_8 <= 0.0f)
    {
        return;
    }
    float h_26 = (float4((&f_34->atmos_0)->radii_0) ).z;
    float _S247 = atm_rb_0(f_34);
    float _S248 = atm_rt_0(f_34);
    float te_3;
    if(h_26 > (_S248 - _S247))
    {
        float te_4 = atm_enter_top_0(_S247, _S248, h_26, dot(p_9 * float3((0.0010000000474974513f / t_8)) , (float4((&f_34->atmos_0)->up_0) ).xyz));
        bool _S249;
        if(te_4 < 0.0f)
        {
            _S249 = true;
        }
        else
        {
            _S249 = t_8 <= te_4;
        }
        if(_S249)
        {
            return;
        }
        te_3 = te_4;
    }
    else
    {
        te_3 = 0.0f;
    }
    float _S250 = atm_ap_slice_0(f_34, t_8 - te_3);
    float2 q_5 = clamp(px_3 / (float4(f_34->viewport_0) ).zw * float2(32.0f)  - float2(0.5f) , float2(0.0f) , float2(31.0f) );
    uint2 _S251 = min(uint2(q_5), uint2(30U) );
    float2 g_7 = q_5 - float2(_S251);
    packed_float4 device* _S252 = ed_buf_7((uint4((&f_34->atmos_0)->bufs2_0) ).x, kernelContext_41);
    packed_float4 device* _S253 = ed_buf_7((uint4((&f_34->atmos_0)->bufs2_0) ).y, kernelContext_41);
    thread float3 la_0;
    thread float3 ta_0;
    thread float3 lc_3;
    thread float3 tc_0;
    if(_S250 < 1.0f)
    {
        atm_froxel_xy_0(_S252, _S253, g_7, _S251, 0U, &lc_3, &tc_0);
        float3 _S254 = float3(_S250) ;
        *l_5 = lc_3 * _S254;
        *tr_0 = mix(_S246, tc_0, _S254);
        return;
    }
    float _S255 = min(_S250 - 1.0f, 31.0f);
    uint _S256 = min(uint(_S255), 30U);
    float gz_0 = _S255 - float(_S256);
    atm_froxel_xy_0(_S252, _S253, g_7, _S251, _S256, &la_0, &ta_0);
    atm_froxel_xy_0(_S252, _S253, g_7, _S251, _S256 + 1U, &lc_3, &tc_0);
    float3 _S257 = float3(gz_0) ;
    *l_5 = mix(la_0, lc_3, _S257);
    *tr_0 = mix(ta_0, tc_0, _S257);
    return;
}

float3 apply_aerial_0(FrameData_natural_0 device* f_35, float3 col_0, float3 p_10, float2 px_4, KernelContext_0 thread* kernelContext_42)
{
    bool _S258 = atmos_on_0(f_35);
    if(!_S258)
    {
        return col_0;
    }
    thread float3 l_6;
    thread float3 tr_1;
    atmos_aerial_0(f_35, p_10, px_4, &l_6, &tr_1, kernelContext_42);
    if(((uint4((&f_35->atmos_0)->cfg_1) ).y) == 1U)
    {
        return tr_1 * float3(0.5f) ;
    }
    if(((uint4((&f_35->atmos_0)->cfg_1) ).y) == 2U)
    {
        return l_6;
    }
    return col_0 * tr_1 + l_6;
}

uint shadow_cascade_0(FrameData_natural_0 device* f_36, float3 p_11, float dist_1)
{
    uint _S259 = uint((float4((&f_36->light_0)->cfg_0) ).x);
    bool _S260;
    if(((uint4((&f_36->light_0)->flags_0) ).x) == 0U)
    {
        _S260 = true;
    }
    else
    {
        _S260 = dist_1 >= ((float4((&f_36->light_0)->filter_0) ).y);
    }
    if(_S260)
    {
        return 4U;
    }
    float _S261 = 1.0f - (float4((&f_36->light_0)->cfg_0) ).w * 2.0f / (float4((&f_36->light_0)->cfg_0) ).y;
    uint c_9 = 0U;
    for(;;)
    {
        if(c_9 < _S259)
        {
        }
        else
        {
            break;
        }
        float _S262 = cascade_extent_0(f_36, c_9, p_11);
        if((_S262 / _S261) <= 1.0f)
        {
            return c_9;
        }
        c_9 = c_9 + 1U;
    }
    return 4U;
}

float t_own_h_1(uint _S263, TerrainDraw_natural_0 device* _S264, uint _S265, uint _S266, KernelContext_0 thread* kernelContext_43)
{
    float4 _S267 = float4(_S264->hq_0) ;
    float _S268 = t_page_0(_S263, (uint4(_S264->slots_0) ).x, _S265 * 4U, _S266 * 4U, _S267.x, _S267.y, kernelContext_43);
    return _S268;
}

float3 t_base_1(uint _S269, TerrainDraw_natural_0 device* _S270, uint _S271, uint _S272, KernelContext_0 thread* kernelContext_44)
{
    TerrainDraw_natural_0 _S273 = *_S270;
    float _S274 = t_own_h_1(_S269, _S270, _S271, _S272, kernelContext_44);
    thread TerrainDraw_natural_0 _S275 = _S273;
    thread float3 dir_8;
    float3 _S276 = t_lattice_0(&_S275, _S271, _S272, _S274, &dir_8);
    return _S276;
}

float3 t_coarse_even_2(uint _S277, TerrainDraw_natural_0 device* _S278, uint _S279, uint2 _S280, uint2 _S281, uint2 _S282, KernelContext_0 thread* kernelContext_45)
{
    thread TerrainDraw_natural_0 _S283 = *_S278;
    float3 _S284 = t_base_1(_S277, _S278, _S280.x, _S280.y, kernelContext_45);
    if((_S279 & 3U) == 0U)
    {
        return _S284;
    }
    float m_4 = t_morph_0((float4((&_S283)->origin_0) ).xyz + _S284, (float4((&_S283)->morph_0) ).zw);
    float3 _S285 = t_base_0(_S277, &_S283, _S281.x, _S281.y, kernelContext_45);
    float3 _S286 = t_base_0(_S277, &_S283, _S282.x, _S282.y, kernelContext_45);
    return mix(_S284, float3(0.5f)  * (_S285 + _S286), float3(m_4) );
}

float3 t_coarse_1(uint _S287, TerrainDraw_natural_0 device* _S288, uint _S289, uint _S290, KernelContext_0 thread* kernelContext_46)
{
    thread TerrainDraw_natural_0 _S291 = *_S288;
    if((_S290 & 1U) == 0U)
    {
        float3 _S292 = t_coarse_even_2(_S287, _S288, _S290, t_edge_at_0(_S289, _S290), t_edge_at_0(_S289, _S290 - 2U), t_edge_at_0(_S289, _S290 + 2U), kernelContext_46);
        return _S292;
    }
    uint _S293 = _S290 - 1U;
    uint2 _S294 = t_edge_at_0(_S289, _S293);
    uint _S295 = _S290 + 1U;
    uint2 _S296 = t_edge_at_0(_S289, _S295);
    float3 _S297 = t_coarse_even_2(_S287, _S288, _S293, _S294, t_edge_at_0(_S289, _S290 - 3U), _S296, kernelContext_46);
    float3 _S298 = t_coarse_even_0(_S287, &_S291, _S295, _S296, _S294, t_edge_at_0(_S289, _S290 + 3U), kernelContext_46);
    return float3(0.5f)  * (_S297 + _S298);
}

float3 t_grid_vertex_1(uint _S299, TerrainDraw_natural_0 device* _S300, uint _S301, uint _S302, KernelContext_0 thread* kernelContext_47)
{
    thread TerrainDraw_natural_0 _S303 = *_S300;
    uint coarse_1 = ((uint4((&_S303)->slots_0) ).z) & 15U;
    if(coarse_1 != 0U)
    {
        bool _S304;
        if((coarse_1 & 1U) != 0U)
        {
            _S304 = _S301 == 0U;
        }
        else
        {
            _S304 = false;
        }
        if(_S304)
        {
            float3 _S305 = t_coarse_1(_S299, _S300, 0U, _S302, kernelContext_47);
            return _S305;
        }
        if((coarse_1 & 2U) != 0U)
        {
            _S304 = _S301 == 64U;
        }
        else
        {
            _S304 = false;
        }
        if(_S304)
        {
            float3 _S306 = t_coarse_1(_S299, _S300, 1U, _S302, kernelContext_47);
            return _S306;
        }
        if((coarse_1 & 4U) != 0U)
        {
            _S304 = _S302 == 0U;
        }
        else
        {
            _S304 = false;
        }
        if(_S304)
        {
            float3 _S307 = t_coarse_1(_S299, _S300, 2U, _S301, kernelContext_47);
            return _S307;
        }
        if((coarse_1 & 8U) != 0U)
        {
            _S304 = _S302 == 64U;
        }
        else
        {
            _S304 = false;
        }
        if(_S304)
        {
            float3 _S308 = t_coarse_1(_S299, _S300, 3U, _S301, kernelContext_47);
            return _S308;
        }
    }
    float3 _S309 = t_base_1(_S299, _S300, _S301, _S302, kernelContext_47);
    float m_5 = t_morph_0((float4((&_S303)->origin_0) ).xyz + _S309, (float4((&_S303)->morph_0) ).xy);
    if(m_5 <= 0.0f)
    {
        return _S309;
    }
    float3 _S310 = t_target_0(_S299, &_S303, _S301, _S302, kernelContext_47);
    return mix(_S309, _S310, float3(m_5) );
}

float3 t_vertex_1(uint _S311, TerrainDraw_natural_0 device* _S312, uint _S313, KernelContext_0 thread* kernelContext_48)
{
    TerrainDraw_natural_0 _S314 = *_S312;
    if(_S313 < 4225U)
    {
        float3 _S315 = (float4(_S314.origin_0) ).xyz;
        float3 _S316 = t_grid_vertex_1(_S311, _S312, _S313 % 65U, _S313 / 65U, kernelContext_48);
        return _S315 + _S316;
    }
    uint2 ij_1 = t_ring_0(_S313 - 4225U);
    float3 _S317 = t_grid_vertex_1(_S311, _S312, ij_1.x, ij_1.y, kernelContext_48);
    float depth_1;
    if((((uint4(_S314.slots_0) ).z) & 64U) != 0U)
    {
        depth_1 = (float4(_S314.origin_0) ).w;
    }
    else
    {
        depth_1 = 0.0f;
    }
    float3 _S318 = (float4(_S314.origin_0) ).xyz;
    float3 _S319 = _S318 + _S317;
    float4 _S320 = float4(_S314.dir_c_0) ;
    return _S319 - normalize(_S319 - (_S318 - _S320.xyz * float3(_S320.w) )) * float3(depth_1) ;
}

float4 terrain_pixel_0(FrameData_natural_0 device* f_37, const FsOut_0 thread* i_18, uint2 vis_1, float3 dir_9, KernelContext_0 thread* kernelContext_49)
{
    TerrainDraw_natural_0 device* _S321 = ed_buf_6(kernelContext_49->ed_c_0->t_draws_0, kernelContext_49);
    TerrainDraw_natural_0 device* _S322 = _S321 + ((vis_1.x) & 2147483647U);
    thread TerrainDraw_natural_0 _S323 = *_S322;
    uint pool_13 = kernelContext_49->ed_c_0->t_pool_0;
    uint3 tri_0 = t_triangle_0((vis_1.y) & 268435455U);
    uint _S324 = tri_0.x;
    float3 _S325 = t_vertex_1(kernelContext_49->ed_c_0->t_pool_0, _S322, _S324, kernelContext_49);
    uint _S326 = tri_0.y;
    float3 _S327 = t_vertex_0(pool_13, &_S323, _S326, kernelContext_49);
    float3 e1_1 = _S327 - _S325;
    uint _S328 = tri_0.z;
    float3 _S329 = t_vertex_0(pool_13, &_S323, _S328, kernelContext_49);
    float3 e2_1 = _S329 - _S325;
    float2 bb_0 = ray_bary_0(dir_9, _S325, e1_1, e2_1);
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
    float3 p_12 = _S325 + e1_1 * float3(b1_1)  + e2_1 * float3(b2_1) ;
    float2 g_8 = t_vertex_grid_0(_S324) * float2((1.0f - b1_1 - b2_1))  + t_vertex_grid_0(_S326) * float2(b1_1)  + t_vertex_grid_0(_S328) * float2(b2_1) ;
    float _S330 = t_sample_spacing_0(&_S323, g_8);
    float _S331 = max(_S330, 0.0099999997764825821f);
    int rn_0 = clamp(int(1.2000000476837158f / _S331 + 0.5f), int(1), int(4));
    thread float3 up_2;
    thread float h_27;
    float3 _S332 = t_smooth_normal_0(pool_13, &_S323, g_8, rn_0, &up_2, &h_27, kernelContext_49);
    int r_8 = clamp(int(24.0f / _S331 + 0.5f), int(1), int(6));
    float3 nm_0;
    if(r_8 == rn_0)
    {
        nm_0 = _S332;
    }
    else
    {
        float3 _S333 = t_smooth_normal_0(pool_13, &_S323, g_8, r_8, &up_2, &h_27, kernelContext_49);
        nm_0 = _S333;
    }
    float3 albedo_2 = terrain_albedo_0(h_27, dot(nm_0, up_2));
    float3 v_5 = - dir_9;
    float _S334 = dot(_S332, v_5);
    float3 n_8;
    if(_S334 < 0.0f)
    {
        n_8 = normalize(_S332 - float3((1.0099999904632568f * _S334))  * v_5);
    }
    else
    {
        n_8 = _S332;
    }
    float zlin_1 = - (((float4(p_12, 1.0f)) * (matrix<float,int(4),int(4)> (f_37->view_0.data_1[int(0)][int(0)], f_37->view_0.data_1[int(1)][int(0)], f_37->view_0.data_1[int(2)][int(0)], f_37->view_0.data_1[int(3)][int(0)], f_37->view_0.data_1[int(0)][int(1)], f_37->view_0.data_1[int(1)][int(1)], f_37->view_0.data_1[int(2)][int(1)], f_37->view_0.data_1[int(3)][int(1)], f_37->view_0.data_1[int(0)][int(2)], f_37->view_0.data_1[int(1)][int(2)], f_37->view_0.data_1[int(2)][int(2)], f_37->view_0.data_1[int(3)][int(2)], f_37->view_0.data_1[int(0)][int(3)], f_37->view_0.data_1[int(1)][int(3)], f_37->view_0.data_1[int(2)][int(3)], f_37->view_0.data_1[int(3)][int(3)])))).z;
    float3 ng_2 = cross(e1_1, e2_1);
    float3 ng_3;
    if((length_squared_0(ng_2)) > 9.99999968265522539e-21f)
    {
        ng_3 = normalize(ng_2);
    }
    else
    {
        ng_3 = n_8;
    }
    if((dot(ng_3, v_5)) < 0.0f)
    {
        ng_3 = - ng_3;
    }
    float2 _S335 = i_18->pos_0.xy;
    float _S336 = sun_shadow_0(f_37, kernelContext_49->ed_c_0->shadow_0, p_12, ng_3, zlin_1, _S335, kernelContext_49);
    float2 _S337 = ao_upsample_0(f_37, kernelContext_49->ed_c_0->ao_0, _S335, zlin_1, kernelContext_49);
    float ao_4 = _S337.x;
    float sv_3 = _S337.y;
    float3 _S338 = sun_light_at_0(f_37, p_12, kernelContext_49);
    float3 _S339 = shade_pbr_0(f_37, _S338, albedo_2, 0.0f, 0.92000001668930054f, ao_4, sv_3, _S336, n_8, v_5, kernelContext_49);
    float3 _S340 = apply_aerial_0(f_37, _S339, p_12, _S335, kernelContext_49);
    float3 col_1;
    if(((uint4((&f_37->light_0)->flags_0) ).w) == 1U)
    {
        uint _S341 = shadow_cascade_0(f_37, p_12, zlin_1);
        col_1 = TINT_1[_S341] * float3((0.039999999105930328f + 0.5f * _S336)) ;
    }
    else
    {
        if(((uint4((&f_37->light_0)->flags_0) ).w) == 2U)
        {
            col_1 = float3((0.5f * ao_4)) ;
        }
        else
        {
            if(((uint4((&f_37->light_0)->flags_0) ).w) == 4U)
            {
                col_1 = float3((0.5f * sv_3)) ;
            }
            else
            {
                col_1 = _S340;
            }
        }
    }
    return float4(col_1, 1.0f);
}

float3 xform_point_0(const GpuInstance_natural_0 thread* i_19, float3 p_13)
{
    float4 _S342 = float4((&i_19->m_0)->data_0[int(0)]) ;
    float4 _S343 = float4((&i_19->m_0)->data_0[int(1)]) ;
    float4 _S344 = float4((&i_19->m_0)->data_0[int(2)]) ;
    return float3(dot(_S342.xyz, p_13) + _S342.w, dot(_S343.xyz, p_13) + _S343.w, dot(_S344.xyz, p_13) + _S344.w);
}

float2 unpack_snorm2x16_0(uint p_14)
{
    return clamp(float2(int2(int(p_14 << 16U) >> 16U, int(p_14) >> 16U)) / float2(32767.0f) , float2(-1.0f) , float2(1.0f) );
}

float3 oct_decode_0(uint packed_0)
{
    float2 e_9 = unpack_snorm2x16_0(packed_0);
    float _S345 = e_9.x;
    float _S346 = e_9.y;
    float _S347 = 1.0f - abs(_S345) - abs(_S346);
    thread float3 n_9 = float3(_S345, _S346, _S347);
    float t_9 = saturate(- _S347);
    float _S348;
    if(_S345 >= 0.0f)
    {
        _S348 = - t_9;
    }
    else
    {
        _S348 = t_9;
    }
    n_9.x = n_9.x + _S348;
    if((n_9.y) >= 0.0f)
    {
        _S348 = - t_9;
    }
    else
    {
        _S348 = t_9;
    }
    n_9.y = n_9.y + _S348;
    return normalize(n_9);
}

float3 xform_normal_0(const GpuInstance_natural_0 thread* i_20, float3 n_10)
{
    float4 _S349 = float4((&i_20->m_0)->data_0[int(0)]) ;
    float4 _S350 = float4((&i_20->m_0)->data_0[int(1)]) ;
    float4 _S351 = float4((&i_20->m_0)->data_0[int(2)]) ;
    float3 a0_0 = float3(_S349.x, _S350.x, _S351.x);
    float3 a1_0 = float3(_S349.y, _S350.y, _S351.y);
    float3 a2_2 = float3(_S349.z, _S350.z, _S351.z);
    return cross(a1_0, a2_2) * float3(n_10.x)  + cross(a2_2, a0_0) * float3(n_10.y)  + cross(a0_0, a1_0) * float3(n_10.z) ;
}

float2 uv_xform_0(const GpuMaterial_natural_0 thread* m_6, float2 uv_5)
{
    float r_9 = (as_type<float>(((uint4(m_6->tex_0) ).w)));
    float s_11 = sin(r_9);
    float c_10 = cos(r_9);
    float4 _S352 = float4(m_6->uv_0) ;
    float2 q_6 = uv_5 * _S352.zw;
    float _S353 = q_6.x;
    float _S354 = q_6.y;
    return float2(c_10 * _S353 + s_11 * _S354, - s_11 * _S353 + c_10 * _S354) + _S352.xy;
}

float3 shade_toon_0(FrameData_natural_0 device* f_38, float3 sun_4, const GpuMaterial_natural_0 thread* m_7, float3 albedo_3, float3 n_11, float3 v_6, float ao_5, float sv_4, float sun_vis_1, KernelContext_0 thread* kernelContext_50)
{
    float ndl_0 = dot(n_11, (float4(f_38->sun_dir_0) ).xyz);
    float _S355 = max((float4(m_7->params_0) ).z, 1.0f);
    float lit_2 = floor(saturate(ndl_0 * 0.5f + 0.5f) * _S355 + 0.5f) / _S355 * sun_vis_1;
    float4 _S356 = float4(m_7->shade_0) ;
    float3 c_11 = mix(albedo_3 * _S356.xyz, albedo_3, float3(lit_2) );
    float3 _S357 = sun_4 * float3((lit_2 / 3.1415927410125732f)) ;
    float3 _S358 = ambient_light_0(f_38, n_11, kernelContext_50);
    float3 _S359 = occluded_ambient_0(f_38, _S358, sv_4, kernelContext_50);
    float3 _S360 = ao_multibounce_0(f_38, ao_5, albedo_3);
    return c_11 * (_S357 + _S359 * _S360) + float3((pow(1.0f - saturate(dot(n_11, v_6)), 4.0f) * _S356.w * saturate(ndl_0 + 0.30000001192092896f) * sun_vis_1))  * sun_4 * float3(0.15000000596046448f) ;
}

float cascade_depth_delta_0(FrameData_natural_0 device* f_39, uint atlas_2, uint c_12, float3 p_15, KernelContext_0 thread* kernelContext_51)
{
    float4 _S361 = float4((&(&f_39->light_0)->cascade_0)->data_4[c_12]) ;
    float tile_1 = (float4((&f_39->light_0)->cfg_0) ).y;
    float4 s_12 = (((float4(p_15, 1.0f)) * (matrix<float,int(4),int(4)> ((&(&f_39->light_0)->shadow_1)->data_3[c_12].data_1[int(0)][int(0)], (&(&f_39->light_0)->shadow_1)->data_3[c_12].data_1[int(1)][int(0)], (&(&f_39->light_0)->shadow_1)->data_3[c_12].data_1[int(2)][int(0)], (&(&f_39->light_0)->shadow_1)->data_3[c_12].data_1[int(3)][int(0)], (&(&f_39->light_0)->shadow_1)->data_3[c_12].data_1[int(0)][int(1)], (&(&f_39->light_0)->shadow_1)->data_3[c_12].data_1[int(1)][int(1)], (&(&f_39->light_0)->shadow_1)->data_3[c_12].data_1[int(2)][int(1)], (&(&f_39->light_0)->shadow_1)->data_3[c_12].data_1[int(3)][int(1)], (&(&f_39->light_0)->shadow_1)->data_3[c_12].data_1[int(0)][int(2)], (&(&f_39->light_0)->shadow_1)->data_3[c_12].data_1[int(1)][int(2)], (&(&f_39->light_0)->shadow_1)->data_3[c_12].data_1[int(2)][int(2)], (&(&f_39->light_0)->shadow_1)->data_3[c_12].data_1[int(3)][int(2)], (&(&f_39->light_0)->shadow_1)->data_3[c_12].data_1[int(0)][int(3)], (&(&f_39->light_0)->shadow_1)->data_3[c_12].data_1[int(1)][int(3)], (&(&f_39->light_0)->shadow_1)->data_3[c_12].data_1[int(2)][int(3)], (&(&f_39->light_0)->shadow_1)->data_3[c_12].data_1[int(3)][int(3)]))));
    int2 _S362 = int2(_S361.zw);
    float _S363 = s_12.z;
    float _S364 = ed_depth_read_0(atlas_2, uint2(clamp(_S362 + int2(floor(float2(s_12.x * 0.5f + 0.5f, 0.5f - s_12.y * 0.5f) * float2(tile_1) )), _S362, _S362 + int2((int(tile_1) - int(1))) )), kernelContext_51);
    return (_S363 - _S364) * _S361.y;
}

float4 resolve_px_0(const FsOut_0 thread* i_21, bool terrain_0, KernelContext_0 thread* kernelContext_52)
{
    FrameData_natural_0 device* _S365 = ed_buf_8(kernelContext_52->ed_c_0->frame_0, kernelContext_52);
    float2 _S366 = i_21->pos_0.xy;
    uint2 _S367 = uint2(_S366);
    texture2d<uint, access::sample> _S368 = ((reinterpret_cast<device texture2d<uint, access::sample>*>((kernelContext_52->ed_textures_0))[(kernelContext_52->ed_c_0->vis_0)]));
    int3 _S369 = int3(int2(_S367), int(0));
    uint2 vis_2 = ((_S368).read(vec<uint,2>(((_S369)).xy), uint(((_S369)).z))).xy;
    float3 _S370 = view_dir_0(_S365, _S366);
    uint _S371 = vis_2.x;
    if(_S371 == 0U)
    {
        float3 _S372 = sky_pixel_0(_S365, _S370, kernelContext_52);
        return float4(_S372, 1.0f);
    }
    bool textured_0;
    if(terrain_0)
    {
        textured_0 = (_S371 & 2147483648U) != 0U;
    }
    else
    {
        textured_0 = false;
    }
    if(textured_0)
    {
        float4 _S373 = terrain_pixel_0(_S365, i_21, vis_2, _S370, kernelContext_52);
        return _S373;
    }
    GpuInstance_natural_0 device* _S374 = ed_buf_4(kernelContext_52->ed_c_0->instances_0, kernelContext_52);
    thread GpuInstance_natural_0 _S375 = *(_S374 + (_S371 - 1U));
    GpuMesh_natural_0 device* _S376 = ed_buf_3(kernelContext_52->ed_c_0->meshes_0, kernelContext_52);
    GpuMesh_natural_0 device* _S377 = _S376 + (&_S375)->mesh_0;
    uint _S378 = vis_2.y;
    uint lod_0 = _S378 >> 28U;
    uint prim_1 = (_S378 & 268435455U) % max(_S377->index_count_0[lod_0] / 3U, 1U);
    uint device* _S379 = ed_buf_5(kernelContext_52->ed_c_0->indices_0, kernelContext_52);
    uint device* idx_0 = _S379 + _S377->first_index_0[lod_0] + prim_1 * 3U;
    Vtx_0 device* _S380 = ed_buf_2(kernelContext_52->ed_c_0->verts_0, kernelContext_52);
    int i0_0 = int(*(idx_0 + int(0))) + _S377->base_vertex_0[lod_0];
    int i1_0 = int(*(idx_0 + int(1))) + _S377->base_vertex_0[lod_0];
    int i2_0 = int(*(idx_0 + int(2))) + _S377->base_vertex_0[lod_0];
    Vtx_0 v0_0 = *(_S380 + i0_0);
    Vtx_0 v1_0 = *(_S380 + i1_0);
    Vtx_0 v2_0 = *(_S380 + i2_0);
    packed_float4 device* _S381 = ed_buf_7(kernelContext_52->ed_c_0->nodes_0, kernelContext_52);
    float3 node_0 = (float4(*(_S381 + (((&_S375)->node_flags_0) & 65535U))) ).xyz;
    float3 _S382 = xform_point_0(&_S375, float3(v0_0.px_0, v0_0.py_0, v0_0.pz_0));
    float3 _S383 = xform_point_0(&_S375, float3(v1_0.px_0, v1_0.py_0, v1_0.pz_0));
    float3 _S384 = xform_point_0(&_S375, float3(v2_0.px_0, v2_0.py_0, v2_0.pz_0));
    float3 p0_1 = node_0 + _S382;
    float3 e1_2 = _S383 - _S382;
    float3 e2_2 = _S384 - _S382;
    float2 bb_1 = ray_bary_0(_S370, p0_1, e1_2, e2_2);
    float b1_2 = saturate(bb_1.x);
    float b2_3 = saturate(bb_1.y);
    float bs_1 = b1_2 + b2_3;
    float b1_3;
    float b2_4;
    if(bs_1 > 1.0f)
    {
        float b2_5 = b2_3 / bs_1;
        b1_3 = b1_2 / bs_1;
        b2_4 = b2_5;
    }
    else
    {
        b1_3 = b1_2;
        b2_4 = b2_3;
    }
    float3 _S385 = float3((1.0f - b1_3 - b2_4)) ;
    float3 local_0 = _S382 * _S385 + _S383 * float3(b1_3)  + _S384 * float3(b2_4) ;
    float3 _S386 = xform_normal_0(&_S375, oct_decode_0(v0_0.n_0) * _S385 + oct_decode_0(v1_0.n_0) * float3(b1_3)  + oct_decode_0(v2_0.n_0) * float3(b2_4) );
    float3 n_12 = normalize(_S386);
    float3 v_7 = - _S370;
    GpuMaterial_natural_0 device* _S387 = ed_buf_1(kernelContext_52->ed_c_0->materials_0, kernelContext_52);
    thread GpuMaterial_natural_0 _S388 = *(_S387 + (&_S375)->material_0);
    float4 _S389 = float4((&_S388)->base_0) ;
    float3 albedo_4 = _S389.xyz;
    float4 _S390 = float4((&_S388)->params_0) ;
    float metal_1 = _S390.x;
    float rough_2 = _S390.y;
    uint4 _S391 = uint4((&_S388)->tex_0) ;
    uint _S392 = _S391.x;
    bool _S393 = _S392 != 4294967295U;
    if(_S393)
    {
        textured_0 = true;
    }
    else
    {
        textured_0 = (_S391.y) != 4294967295U;
    }
    if(textured_0)
    {
        textured_0 = true;
    }
    else
    {
        textured_0 = (_S391.z) != 4294967295U;
    }
    float ao_6;
    float metal_2;
    float rough_3;
    float d_28;
    float3 albedo_5;
    float3 n_13;
    if(textured_0)
    {
        packed_float2 device* _S394 = ed_buf_0(kernelContext_52->ed_c_0->uvs_0, kernelContext_52);
        float2 _S395 = float2(*(_S394 + i0_0)) ;
        float2 d1_0 = float2(*(_S394 + i1_0))  - _S395;
        float2 d2_0 = float2(*(_S394 + i2_0))  - _S395;
        float2 _S396 = uv_xform_0(&_S388, _S395 + d1_0 * float2(b1_3)  + d2_0 * float2(b2_4) );
        float3 _S397 = view_dir_0(_S365, _S366 + float2(1.0f, 0.0f));
        float2 bx_0 = ray_bary_0(_S397, p0_1, e1_2, e2_2);
        float3 _S398 = view_dir_0(_S365, _S366 + float2(0.0f, 1.0f));
        float2 by_0 = ray_bary_0(_S398, p0_1, e1_2, e2_2);
        float2 _S399 = uv_xform_0(&_S388, _S395 + d1_0 * float2(bx_0.x)  + d2_0 * float2(bx_0.y) );
        float2 ux_0 = _S399 - _S396;
        float2 _S400 = uv_xform_0(&_S388, _S395 + d1_0 * float2(by_0.x)  + d2_0 * float2(by_0.y) );
        float2 uy_0 = _S400 - _S396;
        sampler _S401 = ((reinterpret_cast<device sampler*>((kernelContext_52->ed_samplers_0))[(kernelContext_52->ed_c_0->smp_0)]));
        if(_S393)
        {
            texture2d<float, access::sample> _S402 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_52->ed_textures_0))[(_S392)]));
            albedo_5 = albedo_4 * ((_S402).sample((_S401), (_S396), gradient2d((ux_0), (uy_0)))).xyz;
        }
        else
        {
            albedo_5 = albedo_4;
        }
        uint _S403 = _S391.z;
        if(_S403 != 4294967295U)
        {
            texture2d<float, access::sample> _S404 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_52->ed_textures_0))[(_S403)]));
            float3 orm_0 = ((_S404).sample((_S401), (_S396), gradient2d((ux_0), (uy_0)))).xyz;
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
        uint _S405 = _S391.y;
        if(_S405 != 4294967295U)
        {
            float _S406 = d1_0.x;
            float _S407 = d2_0.y;
            float _S408 = d1_0.y;
            float _S409 = d2_0.x;
            float det_1 = _S406 * _S407 - _S408 * _S409;
            if((abs(det_1)) > 9.999999960041972e-13f)
            {
                float3 _S410 = float3((1.0f / det_1)) ;
                float3 tg_0 = (e1_2 * float3(_S407)  - e2_2 * float3(_S408) ) * _S410;
                float3 bt_0 = (e2_2 * float3(_S406)  - e1_2 * float3(_S409) ) * _S410;
                float3 tg_1 = tg_0 - n_12 * float3(dot(n_12, tg_0)) ;
                if((length_squared_0(tg_1)) > 9.99999968265522539e-21f)
                {
                    float3 tg_2 = normalize(tg_1);
                    float3 _S411 = cross(n_12, tg_2);
                    if((dot(_S411, bt_0)) < 0.0f)
                    {
                        d_28 = -1.0f;
                    }
                    else
                    {
                        d_28 = 1.0f;
                    }
                    float3 bn_0 = _S411 * float3(- d_28) ;
                    texture2d<float, access::sample> _S412 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_52->ed_textures_0))[(_S405)]));
                    float2 xy_0 = ((_S412).sample((_S401), (_S396), gradient2d((ux_0), (uy_0)))).xy * float2(2.0f)  - float2(1.0f) ;
                    n_13 = normalize(tg_2 * float3(xy_0.x)  + bn_0 * float3(xy_0.y)  + n_12 * float3(sqrt(saturate(1.0f - dot(xy_0, xy_0)))) );
                }
                else
                {
                    n_13 = n_12;
                }
            }
            else
            {
                n_13 = n_12;
            }
        }
        else
        {
            n_13 = n_12;
        }
    }
    else
    {
        n_13 = n_12;
        albedo_5 = albedo_4;
        ao_6 = 1.0f;
        metal_2 = metal_1;
        rough_3 = rough_2;
    }
    float _S413 = dot(n_13, v_7);
    if(_S413 < 0.0f)
    {
        n_13 = normalize(n_13 - float3((1.0099999904632568f * _S413))  * v_7);
    }
    if((_S390.w) > 0.5f)
    {
        float2 c_13 = floor(local_0.xz * float2(0.5f) );
        albedo_5 = albedo_5 * float3(mix(0.80000001192092896f, 1.1499999761581421f, fmod(abs(c_13.x + c_13.y), 2.0f))) ;
    }
    float3 p_16 = node_0 + local_0;
    float zlin_2 = - (((float4(p_16, 1.0f)) * (matrix<float,int(4),int(4)> (_S365->view_0.data_1[int(0)][int(0)], _S365->view_0.data_1[int(1)][int(0)], _S365->view_0.data_1[int(2)][int(0)], _S365->view_0.data_1[int(3)][int(0)], _S365->view_0.data_1[int(0)][int(1)], _S365->view_0.data_1[int(1)][int(1)], _S365->view_0.data_1[int(2)][int(1)], _S365->view_0.data_1[int(3)][int(1)], _S365->view_0.data_1[int(0)][int(2)], _S365->view_0.data_1[int(1)][int(2)], _S365->view_0.data_1[int(2)][int(2)], _S365->view_0.data_1[int(3)][int(2)], _S365->view_0.data_1[int(0)][int(3)], _S365->view_0.data_1[int(1)][int(3)], _S365->view_0.data_1[int(2)][int(3)], _S365->view_0.data_1[int(3)][int(3)])))).z;
    float3 ng_4 = cross(e1_2, e2_2);
    float3 ng_5;
    if((length_squared_0(ng_4)) > 9.99999968265522539e-21f)
    {
        ng_5 = normalize(ng_4);
    }
    else
    {
        ng_5 = n_13;
    }
    if((dot(ng_5, v_7)) < 0.0f)
    {
        ng_5 = - ng_5;
    }
    float _S414 = sun_shadow_0(_S365, kernelContext_52->ed_c_0->shadow_0, p_16, ng_5, zlin_2, _S366, kernelContext_52);
    float2 _S415 = ao_upsample_0(_S365, kernelContext_52->ed_c_0->ao_0, _S366, zlin_2, kernelContext_52);
    float ao_7 = ao_6 * _S415.x;
    float sv_5 = _S415.y;
    float3 _S416 = sun_light_at_0(_S365, p_16, kernelContext_52);
    float3 col_2;
    if((_S389.w) > 0.5f)
    {
        float3 _S417 = shade_toon_0(_S365, _S416, &_S388, albedo_5, n_13, v_7, ao_7, sv_5, _S414, kernelContext_52);
        col_2 = _S417;
    }
    else
    {
        float3 _S418 = shade_pbr_0(_S365, _S416, albedo_5, metal_2, rough_3, ao_7, sv_5, _S414, n_13, v_7, kernelContext_52);
        col_2 = _S418;
    }
    float3 _S419 = apply_aerial_0(_S365, col_2, p_16, _S366, kernelContext_52);
    if(((uint4((&_S365->light_0)->flags_0) ).w) == 1U)
    {
        uint _S420 = shadow_cascade_0(_S365, p_16, zlin_2);
        col_2 = TINT_0[_S420] * float3((0.039999999105930328f + 0.5f * _S414)) ;
    }
    else
    {
        if(((uint4((&_S365->light_0)->flags_0) ).w) == 3U)
        {
            uint _S421 = shadow_cascade_0(_S365, p_16, zlin_2);
            if(_S421 < 4U)
            {
                float _S422 = cascade_depth_delta_0(_S365, kernelContext_52->ed_c_0->shadow_0, _S421, p_16, kernelContext_52);
                d_28 = _S422;
            }
            else
            {
                d_28 = 0.0f;
            }
            col_2 = float3(max(d_28, 0.0f), max(- d_28, 0.0f), 0.0f) * float3(0.10000000149011612f) ;
        }
        else
        {
            if(((uint4((&_S365->light_0)->flags_0) ).w) == 2U)
            {
                col_2 = float3((0.5f * ao_7)) ;
            }
            else
            {
                if(((uint4((&_S365->light_0)->flags_0) ).w) == 4U)
                {
                    col_2 = float3((0.5f * sv_5)) ;
                }
                else
                {
                    col_2 = _S419;
                }
            }
        }
    }
    return float4(col_2, 1.0f);
}

struct pixelOutput_0
{
    float4 output_0 [[color(0)]];
};

[[fragment]] pixelOutput_0 fs_resolve(float4 pos_1 [[position]], ResolveC_0 constant* ed_c_1 [[buffer(0)]], ulong device* ed_buffers_1 [[buffer(2)]], ulong device* ed_textures_1 [[buffer(1)]], ulong device* ed_samplers_1 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_53;
    (&kernelContext_53)->ed_c_0 = ed_c_1;
    (&kernelContext_53)->ed_buffers_0 = ed_buffers_1;
    (&kernelContext_53)->ed_textures_0 = ed_textures_1;
    (&kernelContext_53)->ed_samplers_0 = ed_samplers_1;
    thread FsOut_0 _S423;
    (&_S423)->pos_0 = pos_1;
    float4 _S424 = resolve_px_0(&_S423, false, &kernelContext_53);
    pixelOutput_0 _S425 = { _S424 };
    return _S425;
}

struct pixelOutput_1
{
    float4 output_1 [[color(0)]];
};

[[fragment]] pixelOutput_1 fs_resolve_terrain(float4 pos_2 [[position]], ResolveC_0 constant* ed_c_2 [[buffer(0)]], ulong device* ed_buffers_2 [[buffer(2)]], ulong device* ed_textures_2 [[buffer(1)]], ulong device* ed_samplers_2 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_54;
    (&kernelContext_54)->ed_c_0 = ed_c_2;
    (&kernelContext_54)->ed_buffers_0 = ed_buffers_2;
    (&kernelContext_54)->ed_textures_0 = ed_textures_2;
    (&kernelContext_54)->ed_samplers_0 = ed_samplers_2;
    thread FsOut_0 _S426;
    (&_S426)->pos_0 = pos_2;
    float4 _S427 = resolve_px_0(&_S426, true, &kernelContext_54);
    pixelOutput_1 _S428 = { _S427 };
    return _S428;
}

struct vs_resolve_Result_0
{
    float4 pos_3 [[position]];
};

[[vertex]] vs_resolve_Result_0 vs_resolve(uint vid_1 [[vertex_id]], ResolveC_0 constant* ed_c_3 [[buffer(0)]], ulong device* ed_buffers_3 [[buffer(2)]], ulong device* ed_textures_3 [[buffer(1)]], ulong device* ed_samplers_3 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_55;
    (&kernelContext_55)->ed_c_0 = ed_c_3;
    (&kernelContext_55)->ed_buffers_0 = ed_buffers_3;
    (&kernelContext_55)->ed_textures_0 = ed_textures_3;
    (&kernelContext_55)->ed_samplers_0 = ed_samplers_3;
    thread vs_resolve_Result_0 _S429;
    (&_S429)->pos_3 = fullscreen_vertex_0(vid_1).pos_0;
    return _S429;
}

