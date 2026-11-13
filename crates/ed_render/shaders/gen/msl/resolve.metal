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
    uint t_mat_0;
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
    uint pad_0;
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
    array<uint, int(4)> pad_1;
};

GpuInstance_natural_0 device* ed_buf_4(uint i_4, KernelContext_0 thread* kernelContext_4)
{
    return (GpuInstance_natural_0 device*)kernelContext_4->ed_buffers_0[i_4];
}

constant array<float3, int(5)> TINT_1 = { { float3(1.0f, 0.25f, 0.25f), float3(0.25f, 1.0f, 0.25f), float3(0.30000001192092896f, 0.44999998807907104f, 1.0f), float3(1.0f, 1.0f, 0.25f), float3(0.60000002384185791f, 0.60000002384185791f, 0.60000002384185791f) } };
struct _Array_natural_vectorx3Cfloatx2C4x3E6_0
{
    array<packed_float4, int(6)> data_1;
};

struct _Array_natural_vectorx3Cfloatx2C4x3E2_0
{
    array<packed_float4, int(2)> data_2;
};

struct TerrainMat_natural_0
{
    uint albedo_0;
    uint normal_0;
    uint layers_0;
    uint size_0;
    array<float, int(8)> tile_0;
    _Array_natural_vectorx3Cfloatx2C4x3E6_0 gain_0;
    _Array_natural_vectorx3Cfloatx2C4x3E2_0 pad_2;
};

TerrainMat_natural_0 device* ed_buf_5(uint i_5, KernelContext_0 thread* kernelContext_5)
{
    return (TerrainMat_natural_0 device*)kernelContext_5->ed_buffers_0[i_5];
}

uint device* ed_buf_6(uint i_6, KernelContext_0 thread* kernelContext_6)
{
    return (uint device*)kernelContext_6->ed_buffers_0[i_6];
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
    packed_float4 tex_1;
};

TerrainDraw_natural_0 device* ed_buf_7(uint i_7, KernelContext_0 thread* kernelContext_7)
{
    return (TerrainDraw_natural_0 device*)kernelContext_7->ed_buffers_0[i_7];
}

packed_float4 device* ed_buf_8(uint i_8, KernelContext_0 thread* kernelContext_8)
{
    return (packed_float4 device*)kernelContext_8->ed_buffers_0[i_8];
}

struct _MatrixStorage_float4x4_ColMajornatural_0
{
    array<packed_float4, int(4)> data_3;
};

struct _Array_natural_vectorx3Cfloatx2C4x3E5_0
{
    array<packed_float4, int(5)> data_4;
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
    array<_MatrixStorage_float4x4_ColMajornatural_0, int(4)> data_5;
};

struct _Array_natural_vectorx3Cfloatx2C4x3E4_0
{
    array<packed_float4, int(4)> data_6;
};

struct _Array_natural_vectorx3Cfloatx2C4x3E9_0
{
    array<packed_float4, int(9)> data_7;
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

FrameData_natural_0 device* ed_buf_9(uint i_9, KernelContext_0 thread* kernelContext_9)
{
    return (FrameData_natural_0 device*)kernelContext_9->ed_buffers_0[i_9];
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
    float4 p_0 = (((float4(float2(uv_1.x * 2.0f - 1.0f, 1.0f - uv_1.y * 2.0f), 1.0f, 1.0f)) * (matrix<float,int(4),int(4)> (f_0->inv_view_proj_0.data_3[int(0)][int(0)], f_0->inv_view_proj_0.data_3[int(1)][int(0)], f_0->inv_view_proj_0.data_3[int(2)][int(0)], f_0->inv_view_proj_0.data_3[int(3)][int(0)], f_0->inv_view_proj_0.data_3[int(0)][int(1)], f_0->inv_view_proj_0.data_3[int(1)][int(1)], f_0->inv_view_proj_0.data_3[int(2)][int(1)], f_0->inv_view_proj_0.data_3[int(3)][int(1)], f_0->inv_view_proj_0.data_3[int(0)][int(2)], f_0->inv_view_proj_0.data_3[int(1)][int(2)], f_0->inv_view_proj_0.data_3[int(2)][int(2)], f_0->inv_view_proj_0.data_3[int(3)][int(2)], f_0->inv_view_proj_0.data_3[int(0)][int(3)], f_0->inv_view_proj_0.data_3[int(1)][int(3)], f_0->inv_view_proj_0.data_3[int(2)][int(3)], f_0->inv_view_proj_0.data_3[int(3)][int(3)]))));
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

float3 atm_transmittance_0(FrameData_natural_0 device* f_10, float h_7, float mu_7, KernelContext_0 thread* kernelContext_10)
{
    float _S15 = atm_rb_0(f_10);
    float _S16 = atm_rt_0(f_10);
    float2 uv_3 = atm_trans_uv_0(_S15, _S16, h_7, mu_7);
    packed_float4 device* _S17 = ed_buf_8((uint4((&f_10->atmos_0)->bufs_0) ).x, kernelContext_10);
    float4 _S18 = atm_bilinear_0(_S17, 256U, 0U, 64U, uv_3);
    return _S18.xyz;
}

float3 atm_transmittance_1(FrameData_natural_0 device* f_11, float h_8, float mu_8, KernelContext_0 thread* kernelContext_11)
{
    float _S19 = atm_rb_0(f_11);
    float _S20 = atm_rt_0(f_11);
    float2 uv_4 = atm_trans_uv_0(_S19, _S20, h_8, mu_8);
    packed_float4 device* _S21 = ed_buf_8((uint4((&f_11->atmos_0)->bufs_0) ).x, kernelContext_11);
    float4 _S22 = atm_bilinear_0(_S21, 256U, 0U, 64U, uv_4);
    return _S22.xyz;
}

float atm_cos_horizon_0(float rb_4, float h_9)
{
    return - sqrt(max(h_9 * (2.0f * rb_4 + h_9), 0.0f)) / (rb_4 + h_9);
}

float3 atm_sky_view_0(FrameData_natural_0 device* f_12, uint buf_1, uint w_1, uint rows_1, float h_10, float3 d_3, KernelContext_0 thread* kernelContext_12)
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
    packed_float4 device* _S27 = ed_buf_8(buf_1, kernelContext_12);
    float4 _S28 = atm_bilinear_0(_S27, w_1, row0_1, half_rows_0, float2(u_0, c_2));
    return _S28.xyz;
}

float3 atmos_sky_0(FrameData_natural_0 device* f_13, float3 d_4, KernelContext_0 thread* kernelContext_13)
{
    float3 _S29 = atm_sky_view_0(f_13, (uint4((&f_13->atmos_0)->bufs_0) ).z, 192U, 108U, (float4((&f_13->atmos_0)->radii_0) ).z, d_4, kernelContext_13);
    return _S29;
}

float3 atmos_sky_1(FrameData_natural_0 device* f_14, float3 d_5, KernelContext_0 thread* kernelContext_14)
{
    float3 _S30 = atm_sky_view_0(f_14, (uint4((&f_14->atmos_0)->bufs_0) ).z, 192U, 108U, (float4((&f_14->atmos_0)->radii_0) ).z, d_5, kernelContext_14);
    return _S30;
}

float3 atmos_sky_pixel_0(FrameData_natural_0 device* f_15, float3 d_6, float disc_k_2, KernelContext_0 thread* kernelContext_15)
{
    float3 _S31 = atmos_sky_0(f_15, d_6, kernelContext_15);
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
                float3 _S36 = atm_transmittance_0(f_15, h_11, mu_10, kernelContext_15);
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

float3 sky_pixel_0(FrameData_natural_0 device* f_16, float3 d_7, KernelContext_0 thread* kernelContext_16)
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
            float3 _S42 = atm_transmittance_0(f_16, h_12, mu_11, kernelContext_16);
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
    float3 _S44 = atmos_sky_pixel_0(f_16, d_7, _S43, kernelContext_16);
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
    uint lod_0 = (prim_0 >> 24U) & 3U;
    uint _S45 = prim_0 & 16777215U;
    uint st_0 = 1U << lod_0;
    uint c_4 = 64U >> lod_0;
    uint _S46 = 2U * c_4 * c_4;
    uint3 _S47;
    if(_S45 < _S46)
    {
        uint q_0 = _S45 >> 1U;
        uint _S48 = q_0 % c_4;
        uint i_10 = _S48 * st_0;
        uint _S49 = q_0 / c_4;
        uint v00_0 = _S49 * st_0 * 65U + i_10;
        uint v10_0 = v00_0 + st_0;
        uint v01_0 = v00_0 + st_0 * 65U;
        uint v11_0 = v01_0 + st_0;
        if((_S45 & 1U) == 0U)
        {
            _S47 = uint3(v00_0, v10_0, v11_0);
        }
        else
        {
            _S47 = uint3(v00_0, v11_0, v01_0);
        }
        return _S47;
    }
    uint k_0 = _S45 - _S46;
    uint s_1 = k_0 >> 1U;
    uint s1_0 = (s_1 + 1U) % (4U * c_4);
    uint _S50 = s_1 * st_0;
    uint2 p_2 = t_ring_0(_S50);
    uint _S51 = s1_0 * st_0;
    uint2 q_1 = t_ring_0(_S51);
    uint tp_0 = p_2.y * 65U + p_2.x;
    uint tq_0 = q_1.y * 65U + q_1.x;
    uint a_0 = 4225U + _S50;
    uint b_1 = 4225U + _S51;
    if((k_0 & 1U) == 0U)
    {
        _S47 = uint3(a_0, b_1, tq_0);
    }
    else
    {
        _S47 = uint3(a_0, tq_0, tp_0);
    }
    return _S47;
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

float t_page_0(uint pool_0, uint slot_0, uint i_11, uint j_0, float scale_0, float offset_1, KernelContext_0 thread* kernelContext_17)
{
    uint s_2 = j_0 * 257U + i_11;
    uint device* _S52 = ed_buf_6(pool_0, kernelContext_17);
    uint w_2 = *(_S52 + (slot_0 * 33025U + (s_2 >> 1U)));
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

float t_own_h_0(uint pool_1, const TerrainDraw_natural_0 thread* d_8, uint i_12, uint j_1, KernelContext_0 thread* kernelContext_18)
{
    float4 _S53 = float4(d_8->hq_0) ;
    float _S54 = t_page_0(pool_1, (uint4(d_8->slots_0) ).x, i_12 * 4U, j_1 * 4U, _S53.x, _S53.y, kernelContext_18);
    return _S54;
}

float3 t_lattice_0(const TerrainDraw_natural_0 thread* d_9, uint i_13, uint j_2, float h_13, float3 thread* dir_0)
{
    float4 _S55 = float4(d_9->shape_0) ;
    float _S56 = _S55.x;
    float tu_0 = tan((float(i_13) - 32.0f) * _S56);
    float tv_0 = tan((float(j_2) - 32.0f) * _S56);
    float4 _S57 = float4(d_9->fu_0) ;
    float _S58 = _S57.w;
    float4 _S59 = float4(d_9->fv_0) ;
    float _S60 = _S59.w;
    float3 delta_0 = float3((tu_0 * (1.0f + _S58 * _S58) / (1.0f - _S58 * tu_0)))  * _S57.xyz + float3((tv_0 * (1.0f + _S60 * _S60) / (1.0f - _S60 * tv_0)))  * _S59.xyz;
    float4 _S61 = float4(d_9->cc_0) ;
    float lc_0 = _S61.w;
    float3 _S62 = _S61.xyz;
    float k_1 = 2.0f * dot(_S62, delta_0) + dot(delta_0, delta_0);
    float l_0 = sqrt(lc_0 * lc_0 + k_1);
    float3 dd_0 = delta_0 / float3(l_0)  - _S62 * float3((k_1 / ((lc_0 + l_0) * l_0 * lc_0))) ;
    float4 _S63 = float4(d_9->dir_c_0) ;
    float3 _S64 = _S63.xyz;
    float3 _S65 = _S64 + dd_0;
    *dir_0 = _S65;
    float3 _S66 = (float4(d_9->polar_0) ).xyz;
    float dzc_0 = dot(_S64, _S66);
    float ddz_0 = dot(dd_0, _S66);
    float dz_0 = dzc_0 + ddz_0;
    float e_2 = _S55.z;
    float s_3 = sqrt(1.0f - e_2 * dz_0 * dz_0);
    float sc_0 = _S55.w;
    float dr_0 = _S55.y * e_2 * ddz_0 * (dz_0 + dzc_0) / (s_3 * sc_0 * (s_3 + sc_0));
    return dd_0 * float3((_S63.w + dr_0))  + _S64 * float3(dr_0)  + _S65 * float3(h_13) ;
}

float3 t_base_0(uint pool_2, const TerrainDraw_natural_0 thread* d_10, uint i_14, uint j_3, KernelContext_0 thread* kernelContext_19)
{
    float _S67 = t_own_h_0(pool_2, d_10, i_14, j_3, kernelContext_19);
    thread float3 dir_1;
    float3 _S68 = t_lattice_0(d_10, i_14, j_3, _S67, &dir_1);
    return _S68;
}

float t_morph_0(float3 rel_0, float2 band_0)
{
    float _S69 = band_0.x;
    return saturate((length(rel_0) - _S69) / max(band_0.y - _S69, 0.0010000000474974513f));
}

float3 t_coarse_even_0(uint pool_3, const TerrainDraw_natural_0 thread* d_11, uint e_3, uint2 a_1, uint2 b_2, uint2 c_5, KernelContext_0 thread* kernelContext_20)
{
    float3 _S70 = t_base_0(pool_3, d_11, a_1.x, a_1.y, kernelContext_20);
    if((e_3 & 3U) == 0U)
    {
        return _S70;
    }
    float m_1 = t_morph_0((float4(d_11->origin_0) ).xyz + _S70, (float4(d_11->morph_0) ).zw);
    float3 _S71 = t_base_0(pool_3, d_11, b_2.x, b_2.y, kernelContext_20);
    float3 _S72 = t_base_0(pool_3, d_11, c_5.x, c_5.y, kernelContext_20);
    return mix(_S70, float3(0.5f)  * (_S71 + _S72), float3(m_1) );
}

float3 t_coarse_0(uint pool_4, const TerrainDraw_natural_0 thread* d_12, uint edge_1, uint e_4, KernelContext_0 thread* kernelContext_21)
{
    if((e_4 & 1U) == 0U)
    {
        float3 _S73 = t_coarse_even_0(pool_4, d_12, e_4, t_edge_at_0(edge_1, e_4), t_edge_at_0(edge_1, e_4 - 2U), t_edge_at_0(edge_1, e_4 + 2U), kernelContext_21);
        return _S73;
    }
    uint _S74 = e_4 - 1U;
    uint2 _S75 = t_edge_at_0(edge_1, _S74);
    uint _S76 = e_4 + 1U;
    uint2 _S77 = t_edge_at_0(edge_1, _S76);
    float3 _S78 = t_coarse_even_0(pool_4, d_12, _S74, _S75, t_edge_at_0(edge_1, e_4 - 3U), _S77, kernelContext_21);
    float3 _S79 = t_coarse_even_0(pool_4, d_12, _S76, _S77, _S75, t_edge_at_0(edge_1, e_4 + 3U), kernelContext_21);
    return float3(0.5f)  * (_S78 + _S79);
}

float3 t_target_even_0(uint pool_5, const TerrainDraw_natural_0 thread* d_13, uint i_15, uint j_4, KernelContext_0 thread* kernelContext_22)
{
    bool border_0;
    if(i_15 == 0U)
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
        border_0 = i_15 == 64U;
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
        border_0 = ((uint4(d_13->slots_0) ).y) == 4294967295U;
    }
    float h_14;
    if(border_0)
    {
        float _S80 = t_own_h_0(pool_5, d_13, i_15, j_4, kernelContext_22);
        h_14 = _S80;
    }
    else
    {
        uint4 _S81 = uint4(d_13->slots_0) ;
        uint _S82 = _S81.z;
        uint qx_0;
        if((_S82 & 16U) != 0U)
        {
            qx_0 = 128U;
        }
        else
        {
            qx_0 = 0U;
        }
        uint qy_0;
        if((_S82 & 32U) != 0U)
        {
            qy_0 = 128U;
        }
        else
        {
            qy_0 = 0U;
        }
        float4 _S83 = float4(d_13->hq_0) ;
        float _S84 = t_page_0(pool_5, _S81.y, qx_0 + 2U * i_15, qy_0 + 2U * j_4, _S83.z, _S83.w, kernelContext_22);
        h_14 = _S84;
    }
    thread float3 dir_2;
    float3 _S85 = t_lattice_0(d_13, i_15, j_4, h_14, &dir_2);
    return _S85;
}

float3 t_target_0(uint pool_6, const TerrainDraw_natural_0 thread* d_14, uint i_16, uint j_5, KernelContext_0 thread* kernelContext_23)
{
    bool ei_0 = (i_16 & 1U) == 0U;
    bool ej_0 = (j_5 & 1U) == 0U;
    bool _S86;
    if(ei_0)
    {
        _S86 = ej_0;
    }
    else
    {
        _S86 = false;
    }
    if(_S86)
    {
        float3 _S87 = t_target_even_0(pool_6, d_14, i_16, j_5, kernelContext_23);
        return _S87;
    }
    if(!ei_0)
    {
        _S86 = ej_0;
    }
    else
    {
        _S86 = false;
    }
    if(_S86)
    {
        float3 _S88 = t_target_even_0(pool_6, d_14, i_16 - 1U, j_5, kernelContext_23);
        float3 _S89 = t_target_even_0(pool_6, d_14, i_16 + 1U, j_5, kernelContext_23);
        return float3(0.5f)  * (_S88 + _S89);
    }
    if(ei_0)
    {
        _S86 = !ej_0;
    }
    else
    {
        _S86 = false;
    }
    if(_S86)
    {
        float3 _S90 = t_target_even_0(pool_6, d_14, i_16, j_5 - 1U, kernelContext_23);
        float3 _S91 = t_target_even_0(pool_6, d_14, i_16, j_5 + 1U, kernelContext_23);
        return float3(0.5f)  * (_S90 + _S91);
    }
    float3 _S92 = t_target_even_0(pool_6, d_14, i_16 - 1U, j_5 - 1U, kernelContext_23);
    float3 _S93 = t_target_even_0(pool_6, d_14, i_16 + 1U, j_5 + 1U, kernelContext_23);
    return float3(0.5f)  * (_S92 + _S93);
}

float3 t_grid_vertex_0(uint pool_7, const TerrainDraw_natural_0 thread* d_15, uint i_17, uint j_6, KernelContext_0 thread* kernelContext_24)
{
    uint fl_0 = (uint4(d_15->slots_0) ).z;
    uint edge_2;
    uint e_5;
    if(i_17 == 0U)
    {
        edge_2 = 0U;
        e_5 = j_6;
    }
    else
    {
        if(i_17 == 64U)
        {
            edge_2 = 1U;
            e_5 = j_6;
        }
        else
        {
            if(j_6 == 0U)
            {
                edge_2 = 2U;
                e_5 = i_17;
            }
            else
            {
                if(j_6 == 64U)
                {
                    edge_2 = 3U;
                    e_5 = i_17;
                }
                else
                {
                    edge_2 = 4U;
                    e_5 = 0U;
                }
            }
        }
    }
    if(edge_2 < 4U)
    {
        uint E_0 = 1U << ((fl_0 >> (10U + 3U * edge_2)) & 7U);
        bool _S94;
        if((fl_0 & (1U << edge_2)) != 0U)
        {
            _S94 = E_0 <= 2U;
        }
        else
        {
            _S94 = false;
        }
        if(_S94)
        {
            float3 _S95 = t_coarse_0(pool_7, d_15, edge_2, e_5, kernelContext_24);
            return _S95;
        }
        if(E_0 > 1U)
        {
            uint lo_0 = e_5 & (~(E_0 - 1U));
            if(lo_0 == e_5)
            {
                float3 _S96 = t_base_0(pool_7, d_15, i_17, j_6, kernelContext_24);
                return _S96;
            }
            uint2 a_2 = t_edge_at_0(edge_2, lo_0);
            uint2 b_3 = t_edge_at_0(edge_2, lo_0 + E_0);
            float3 _S97 = t_base_0(pool_7, d_15, a_2.x, a_2.y, kernelContext_24);
            float3 _S98 = t_base_0(pool_7, d_15, b_3.x, b_3.y, kernelContext_24);
            return mix(_S97, _S98, float3((float(e_5 - lo_0) / float(E_0))) );
        }
    }
    float3 _S99 = t_base_0(pool_7, d_15, i_17, j_6, kernelContext_24);
    if((fl_0 & 4194304U) != 0U)
    {
        return _S99;
    }
    float m_2 = t_morph_0((float4(d_15->origin_0) ).xyz + _S99, (float4(d_15->morph_0) ).xy);
    if(m_2 <= 0.0f)
    {
        return _S99;
    }
    float3 _S100 = t_target_0(pool_7, d_15, i_17, j_6, kernelContext_24);
    return mix(_S99, _S100, float3(m_2) );
}

float3 t_vertex_0(uint pool_8, const TerrainDraw_natural_0 thread* d_16, uint v_0, KernelContext_0 thread* kernelContext_25)
{
    if(v_0 < 4225U)
    {
        float3 _S101 = (float4(d_16->origin_0) ).xyz;
        float3 _S102 = t_grid_vertex_0(pool_8, d_16, v_0 % 65U, v_0 / 65U, kernelContext_25);
        return _S101 + _S102;
    }
    uint2 ij_0 = t_ring_0(v_0 - 4225U);
    float3 _S103 = t_grid_vertex_0(pool_8, d_16, ij_0.x, ij_0.y, kernelContext_25);
    float depth_0;
    if((((uint4(d_16->slots_0) ).z) & 64U) != 0U)
    {
        depth_0 = (float4(d_16->origin_0) ).w;
    }
    else
    {
        depth_0 = 0.0f;
    }
    float3 _S104 = (float4(d_16->origin_0) ).xyz;
    float3 _S105 = _S104 + _S103;
    float4 _S106 = float4(d_16->dir_c_0) ;
    return _S105 - normalize(_S105 - (_S104 - _S106.xyz * float3(_S106.w) )) * float3(depth_0) ;
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

float3 t_lattice_f_0(const TerrainDraw_natural_0 thread* d_17, float2 g_1, float h_15, float3 thread* dir_4)
{
    float4 _S107 = float4(d_17->shape_0) ;
    float _S108 = _S107.x;
    float tu_1 = tan((g_1.x - 32.0f) * _S108);
    float tv_2 = tan((g_1.y - 32.0f) * _S108);
    float4 _S109 = float4(d_17->fu_0) ;
    float _S110 = _S109.w;
    float4 _S111 = float4(d_17->fv_0) ;
    float _S112 = _S111.w;
    float3 delta_1 = float3((tu_1 * (1.0f + _S110 * _S110) / (1.0f - _S110 * tu_1)))  * _S109.xyz + float3((tv_2 * (1.0f + _S112 * _S112) / (1.0f - _S112 * tv_2)))  * _S111.xyz;
    float4 _S113 = float4(d_17->cc_0) ;
    float lc_1 = _S113.w;
    float3 _S114 = _S113.xyz;
    float k_2 = 2.0f * dot(_S114, delta_1) + dot(delta_1, delta_1);
    float l_1 = sqrt(lc_1 * lc_1 + k_2);
    float3 dd_1 = delta_1 / float3(l_1)  - _S114 * float3((k_2 / ((lc_1 + l_1) * l_1 * lc_1))) ;
    float4 _S115 = float4(d_17->dir_c_0) ;
    float3 _S116 = _S115.xyz;
    float3 _S117 = _S116 + dd_1;
    *dir_4 = _S117;
    float3 _S118 = (float4(d_17->polar_0) ).xyz;
    float dzc_1 = dot(_S116, _S118);
    float ddz_1 = dot(dd_1, _S118);
    float dz_1 = dzc_1 + ddz_1;
    float e_6 = _S107.z;
    float s_4 = sqrt(1.0f - e_6 * dz_1 * dz_1);
    float sc_1 = _S107.w;
    float dr_1 = _S107.y * e_6 * ddz_1 * (dz_1 + dzc_1) / (s_4 * sc_1 * (s_4 + sc_1));
    return dd_1 * float3((_S115.w + dr_1))  + _S116 * float3(dr_1)  + _S117 * float3(h_15) ;
}

float3 t_lattice_f_1(const TerrainDraw_natural_0 thread* d_18, float2 g_2, float h_16, float3 thread* dir_5)
{
    float4 _S119 = float4(d_18->shape_0) ;
    float _S120 = _S119.x;
    float tu_2 = tan((g_2.x - 32.0f) * _S120);
    float tv_3 = tan((g_2.y - 32.0f) * _S120);
    float4 _S121 = float4(d_18->fu_0) ;
    float _S122 = _S121.w;
    float4 _S123 = float4(d_18->fv_0) ;
    float _S124 = _S123.w;
    float3 delta_2 = float3((tu_2 * (1.0f + _S122 * _S122) / (1.0f - _S122 * tu_2)))  * _S121.xyz + float3((tv_3 * (1.0f + _S124 * _S124) / (1.0f - _S124 * tv_3)))  * _S123.xyz;
    float4 _S125 = float4(d_18->cc_0) ;
    float lc_2 = _S125.w;
    float3 _S126 = _S125.xyz;
    float k_3 = 2.0f * dot(_S126, delta_2) + dot(delta_2, delta_2);
    float l_2 = sqrt(lc_2 * lc_2 + k_3);
    float3 dd_2 = delta_2 / float3(l_2)  - _S126 * float3((k_3 / ((lc_2 + l_2) * l_2 * lc_2))) ;
    float4 _S127 = float4(d_18->dir_c_0) ;
    float3 _S128 = _S127.xyz;
    float3 _S129 = _S128 + dd_2;
    *dir_5 = _S129;
    float3 _S130 = (float4(d_18->polar_0) ).xyz;
    float dzc_2 = dot(_S128, _S130);
    float ddz_2 = dot(dd_2, _S130);
    float dz_2 = dzc_2 + ddz_2;
    float e_7 = _S119.z;
    float s_5 = sqrt(1.0f - e_7 * dz_2 * dz_2);
    float sc_2 = _S119.w;
    float dr_2 = _S119.y * e_7 * ddz_2 * (dz_2 + dzc_2) / (s_5 * sc_2 * (s_5 + sc_2));
    return dd_2 * float3((_S127.w + dr_2))  + _S128 * float3(dr_2)  + _S129 * float3(h_16) ;
}

float t_sample_spacing_0(const TerrainDraw_natural_0 thread* d_19, float2 g_3)
{
    float2 _S131 = float2(0.5f, 0.0f);
    thread float3 dir_6;
    float3 _S132 = t_lattice_f_0(d_19, g_3 + _S131, 0.0f, &dir_6);
    float3 _S133 = t_lattice_f_0(d_19, g_3 - _S131, 0.0f, &dir_6);
    return length(_S132 - _S133) / 4.0f;
}

float t_page_at_0(uint pool_9, const TerrainDraw_natural_0 thread* d_20, int a_3, int b_4, KernelContext_0 thread* kernelContext_26)
{
    float4 _S134 = float4(d_20->hq_0) ;
    float _S135 = t_page_0(pool_9, (uint4(d_20->slots_0) ).x, uint(clamp(a_3, int(0), int(256))), uint(clamp(b_4, int(0), int(256))), _S134.x, _S134.y, kernelContext_26);
    return _S135;
}

void t_surface_0(uint pool_10, const TerrainDraw_natural_0 thread* d_21, float2 g_4, int r_3, float thread* h_17, float2 thread* dh_0, KernelContext_0 thread* kernelContext_27)
{
    float2 _S136 = float2(4.0f) ;
    float2 _S137 = float2(0.0f) ;
    float2 s_6 = clamp(g_4 * _S136, _S137, float2(256.0f) );
    int2 _S138 = int2(min(floor(s_6), float2(255.0f) ));
    float2 _S139 = s_6 - float2(_S138);
    *h_17 = 0.0f;
    *dh_0 = _S137;
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
        uint _S140 = k_4 & 1U;
        uint _S141 = k_4 >> 1U;
        int2 q_3 = _S138 + int2(int(_S140), int(_S141));
        float _S142;
        if(_S140 != 0U)
        {
            _S142 = _S139.x;
        }
        else
        {
            _S142 = 1.0f - _S139.x;
        }
        float _S143;
        if(_S141 != 0U)
        {
            _S143 = _S139.y;
        }
        else
        {
            _S143 = 1.0f - _S139.y;
        }
        float w_3 = _S142 * _S143;
        int2 _S144 = int2(r_3) ;
        int2 _S145 = max(q_3 - _S144, int2(int(0)) );
        int2 _S146 = min(q_3 + _S144, int2(int(256)) );
        int _S147 = _S146.x;
        int _S148 = q_3.y;
        float _S149 = t_page_at_0(pool_10, d_21, _S147, _S148, kernelContext_27);
        int _S150 = _S145.x;
        float _S151 = t_page_at_0(pool_10, d_21, _S150, _S148, kernelContext_27);
        float gx_0 = (_S149 - _S151) / float(_S147 - _S150);
        int _S152 = q_3.x;
        int _S153 = _S146.y;
        float _S154 = t_page_at_0(pool_10, d_21, _S152, _S153, kernelContext_27);
        int _S155 = _S145.y;
        float _S156 = t_page_at_0(pool_10, d_21, _S152, _S155, kernelContext_27);
        float gy_0 = (_S154 - _S156) / float(_S153 - _S155);
        float _S157 = t_page_at_0(pool_10, d_21, _S152, _S148, kernelContext_27);
        *h_17 = *h_17 + w_3 * _S157;
        *dh_0 = *dh_0 + float2(w_3)  * float2(gx_0, gy_0);
        k_4 = k_4 + 1U;
    }
    *dh_0 = *dh_0 * _S136;
    return;
}

float3 t_smooth_normal_0(uint pool_11, const TerrainDraw_natural_0 thread* d_22, float2 g_5, int r_4, float3 thread* up_1, float thread* h_18, KernelContext_0 thread* kernelContext_28)
{
    thread float2 dh_1;
    t_surface_0(pool_11, d_22, g_5, r_4, h_18, &dh_1, kernelContext_28);
    float2 _S158 = float2(0.25f, 0.0f);
    thread float3 dir_7;
    float3 _S159 = t_lattice_f_0(d_22, g_5 + _S158, *h_18, &dir_7);
    float3 _S160 = t_lattice_f_0(d_22, g_5 - _S158, *h_18, &dir_7);
    float3 pu_0 = _S159 - _S160;
    float2 _S161 = float2(0.0f, 0.25f);
    float3 _S162 = t_lattice_f_0(d_22, g_5 + _S161, *h_18, &dir_7);
    float3 _S163 = t_lattice_f_0(d_22, g_5 - _S161, *h_18, &dir_7);
    float3 pv_1 = _S162 - _S163;
    float3 _S164 = t_lattice_f_1(d_22, g_5, *h_18, up_1);
    float3 _S165 = normalize(*up_1);
    *up_1 = _S165;
    float3 _S166 = float3(0.5f) ;
    float3 n_1 = normalize(cross(pu_0 / _S166 + _S165 * float3(dh_1.x) , pv_1 / _S166 + _S165 * float3(dh_1.y) ));
    float3 _S167;
    if((dot(n_1, _S165)) < 0.0f)
    {
        _S167 = - n_1;
    }
    else
    {
        _S167 = n_1;
    }
    return _S167;
}

float3 terrain_albedo_0(float h_19, float slope_cos_0)
{
    return mix(mix(mix(float3(0.054999999701976776f, 0.075000002980232239f, 0.039999999105930328f), float3(0.125f, 0.14000000059604645f, 0.070000000298023224f), float3(smoothstep(1950.0f, 2350.0f, h_19)) ), float3(0.20999999344348907f, 0.20000000298023224f, 0.18500000238418579f), float3(smoothstep(0.81999999284744263f, 0.72000002861022949f, slope_cos_0)) ), float3(0.80000001192092896f, 0.81999999284744263f, 0.86000001430511475f), float3((smoothstep(2750.0f, 3050.0f, h_19) * smoothstep(0.55000001192092896f, 0.69999998807907104f, slope_cos_0))) );
}

float t_page_bilin_0(uint pool_12, const TerrainDraw_natural_0 thread* d_23, float2 s_7, KernelContext_0 thread* kernelContext_29)
{
    float2 _S168 = clamp(s_7, float2(0.0f) , float2(256.0f) );
    int2 _S169 = int2(min(floor(_S168), float2(255.0f) ));
    float2 t_3 = _S168 - float2(_S169);
    int _S170 = _S169.x;
    int _S171 = _S169.y;
    float _S172 = t_page_at_0(pool_12, d_23, _S170, _S171, kernelContext_29);
    int _S173 = _S170 + int(1);
    float _S174 = t_page_at_0(pool_12, d_23, _S173, _S171, kernelContext_29);
    float _S175 = t_3.x;
    float a_4 = mix(_S172, _S174, _S175);
    int _S176 = _S171 + int(1);
    float _S177 = t_page_at_0(pool_12, d_23, _S170, _S176, kernelContext_29);
    float _S178 = t_page_at_0(pool_12, d_23, _S173, _S176, kernelContext_29);
    return mix(a_4, mix(_S177, _S178, _S175), t_3.y);
}

float length_squared_0(float3 v_2)
{
    return dot(v_2, v_2);
}

float t_hash2_0(int2 c_6)
{
    uint h_20 = (uint(c_6.x) * 2376512323U) ^ (uint(c_6.y) * 3625334849U);
    uint h_21 = (h_20 ^ (h_20 >> 13U)) * 2246822507U;
    return float((h_21 ^ (h_21 >> 16U)) & 65535U) / 65535.0f;
}

float t_noise_0(float2 w_4, float cell_0)
{
    float2 q_4 = w_4 / float2(cell_0) ;
    float2 i_18 = floor(q_4);
    float2 f_17 = q_4 - i_18;
    float2 f_18 = f_17 * f_17 * (float2(3.0f)  - float2(2.0f)  * f_17);
    int per_0 = int(4096.0f / cell_0 + 0.5f);
    int2 _S179 = int2(i_18);
    int _S180 = _S179.x % per_0;
    int _S181 = (_S180 + per_0) % per_0;
    int _S182 = _S179.y % per_0;
    int _S183 = (_S182 + per_0) % per_0;
    int2 c0_0 = int2(_S181, _S183);
    int _S184 = (_S181 + int(1)) % per_0;
    int _S185 = (_S183 + int(1)) % per_0;
    float _S186 = f_18.x;
    return mix(mix(t_hash2_0(c0_0), t_hash2_0(int2(_S184, _S183)), _S186), mix(t_hash2_0(int2(_S181, _S185)), t_hash2_0(int2(_S184, _S185)), _S186), f_18.y);
}

struct TProj_0
{
    float2 w_5;
    float2 dx_0;
    float2 dy_0;
};

TProj_0 TProj_x24init_0(float2 w_6, float2 dx_1, float2 dy_1)
{
    thread TProj_0 _S187;
    (&_S187)->w_5 = w_6;
    (&_S187)->dx_0 = dx_1;
    (&_S187)->dy_0 = dy_1;
    return _S187;
}

float4 t_layer_albedo_0(const TerrainMat_natural_0 thread* m_3, sampler smp_1, uint layer_0, const TProj_0 thread* p_3, float near_0, KernelContext_0 thread* kernelContext_30)
{
    float k_5 = 1.0f / m_3->tile_0[layer_0];
    texture2d_array<float, access::sample> _S188 = ((reinterpret_cast<device texture2d_array<float, access::sample>*>((kernelContext_30->ed_textures_0))[(m_3->albedo_0)]));
    float _S189 = p_3->w_5.x;
    float _S190 = - p_3->w_5.y;
    float2 _S191 = float2(k_5) ;
    float _S192 = float(layer_0);
    float3 _S193 = float3(float2(_S189, _S190) * _S191, _S192);
    float _S194 = p_3->dx_0.x;
    float _S195 = - p_3->dx_0.y;
    float _S196 = p_3->dy_0.x;
    float _S197 = - p_3->dy_0.y;
    float4 a_5 = ((_S188).sample((smp_1), ((_S193)).xy, uint(((_S193)).z), gradient2d((float2(_S194, _S195) * _S191), (float2(_S196, _S197) * _S191))));
    float2 _S198 = float2((0.25f * k_5)) ;
    float3 _S199 = float3(float2(_S190, - _S189) * _S198, _S192);
    float4 b_5 = ((_S188).sample((smp_1), ((_S199)).xy, uint(((_S199)).z), gradient2d((float2(_S195, - _S194) * _S198), (float2(_S197, - _S196) * _S198))));
    float mixk_0 = mix(0.34999999403953552f, 0.75f, 1.0f - near_0);
    return float4(mix(a_5.xyz, b_5.xyz, float3(mixk_0) ), mix(a_5.w, b_5.w, 0.5f * mixk_0));
}

float4 t_layer_normal_0(const TerrainMat_natural_0 thread* m_4, sampler smp_2, uint layer_1, const TProj_0 thread* p_4, KernelContext_0 thread* kernelContext_31)
{
    float k_6 = 1.0f / m_4->tile_0[layer_1];
    texture2d_array<float, access::sample> _S200 = ((reinterpret_cast<device texture2d_array<float, access::sample>*>((kernelContext_31->ed_textures_0))[(m_4->normal_0)]));
    float2 _S201 = float2(k_6) ;
    float3 _S202 = float3(float2(p_4->w_5.x, - p_4->w_5.y) * _S201, float(layer_1));
    float4 b_6 = ((_S200).sample((smp_2), ((_S202)).xy, uint(((_S202)).z), gradient2d((float2(p_4->dx_0.x, - p_4->dx_0.y) * _S201), (float2(p_4->dy_0.x, - p_4->dy_0.y) * _S201))));
    return float4(b_6.xy * float2(2.0f)  - float2(1.0f) , b_6.z, b_6.w);
}

struct TSurf_0
{
    float3 albedo_1;
    float3 n_2;
    float rough_0;
    float ao_2;
};

TSurf_0 terrain_material_0(FrameData_natural_0 device* f_19, const TerrainMat_natural_0 thread* m_5, const TerrainDraw_natural_0 thread* d_24, float3 n_3, float3 nm_0, float3 up_2, float h_22, float curv_0, float curv_f_0, float2 w_7, float2 dwx_0, float2 dwy_0, float dhx_0, float dhy_0, float3 tu_3, float3 tv_4, float dist_0, KernelContext_0 thread* kernelContext_32)
{
    float4 _S203;
    float best_0;
    bool _S204;
    float4 nr_0;
    float3 pert_0;
    float3 nw_0;
    sampler _S205 = ((reinterpret_cast<device sampler*>((kernelContext_32->ed_samplers_0))[(kernelContext_32->ed_c_0->smp_0)]));
    float cs_0 = dot(nm_0, up_2);
    float cf_0 = dot(n_3, up_2);
    float3 _S206 = (float4(d_24->polar_0) ).xyz;
    float3 north_0 = normalize(_S206 - up_2 * float3(dot(_S206, up_2)) );
    float3 nh_0 = nm_0 - up_2 * float3(cs_0) ;
    float northness_0;
    if((length_squared_0(nh_0)) > 9.99999997475242708e-07f)
    {
        northness_0 = dot(normalize(nh_0), north_0);
    }
    else
    {
        northness_0 = 0.0f;
    }
    float n1_0 = t_noise_0(w_7, 16.0f);
    float n2_0 = t_noise_0(w_7 + float2(37.0f) , 64.0f);
    float n3_0 = t_noise_0(w_7 + float2(911.0f) , 256.0f);
    float hollow_0 = saturate(curv_0 * 60.0f);
    float hollow_f_0 = saturate(curv_f_0 * 4.0f);
    float _S207 = n3_0 - 0.5f;
    float treeline_0 = 2150.0f + 120.0f * _S207 - 100.0f * northness_0;
    float _S208 = n2_0 - 0.5f;
    float snowline_0 = 3050.0f - 230.0f * northness_0 + 200.0f * _S208;
    float alpine_0 = smoothstep(treeline_0 - 150.0f, treeline_0 + 250.0f, h_22);
    float rock_cs_0 = mix(0.68000000715255737f, 0.76999998092651367f, alpine_0);
    float steep_0 = smoothstep(rock_cs_0, rock_cs_0 - 0.14000000059604645f, cs_0);
    float _S209 = n1_0 - 0.5f;
    float rock_0 = saturate(saturate(max(steep_0, smoothstep(rock_cs_0 - 0.059999998658895493f, rock_cs_0 - 0.20000000298023224f, cf_0) * 0.80000001192092896f) + saturate(- curv_0 * 60.0f) * 0.5f * alpine_0 + smoothstep(2700.0f, 3300.0f, h_22) * 0.44999998807907104f - 0.25f * hollow_0) + _S209 * 0.5f * smoothstep(0.92000001668930054f, 0.80000001192092896f, cs_0));
    float _S210 = _S208 * 0.80000001192092896f;
    float scree_0 = saturate(smoothstep(0.97000002861022949f, 0.87999999523162842f, cs_0) * smoothstep(0.6600000262260437f, 0.80000001192092896f, cs_0) * (0.10000000149011612f + hollow_0 * 1.2000000476837158f + _S210) * mix(0.10000000149011612f, 1.0f, alpine_0) + steep_0 * 0.30000001192092896f * alpine_0);
    float forest_0 = (1.0f - alpine_0) * smoothstep(0.99000000953674316f, 0.94999998807907104f, cs_0) * saturate(0.89999997615814209f + _S207 * 1.3999999761581421f + _S210);
    float grass_0 = (1.0f - forest_0) * smoothstep(3000.0f, 2600.0f, h_22 + _S209 * 200.0f);
    float soil_0 = saturate(hollow_f_0 * 0.80000001192092896f + (n1_0 - 0.64999997615814209f) * 1.5f) * (1.0f - alpine_0 * 0.5f);
    float snow_0 = smoothstep(snowline_0 - 120.0f, snowline_0 + 120.0f, h_22 + hollow_0 * 150.0f + _S209 * 120.0f) * smoothstep(0.62000000476837158f, 0.77999997138977051f, cf_0 + _S209 * 0.10000000149011612f);
    thread array<float, int(6)> wt_0;
    wt_0[0U] = rock_0;
    wt_0[1U] = scree_0 * (1.0f - rock_0 * 0.69999998807907104f);
    float _S211 = 1.0f - rock_0;
    float _S212 = 1.0f - scree_0 * 0.80000001192092896f;
    wt_0[2U] = grass_0 * _S211 * _S212;
    wt_0[3U] = forest_0 * _S211 * _S212;
    float _S213 = 1.0f - snow_0;
    wt_0[5U] = soil_0 * 0.5f * _S211 * _S213;
    wt_0[4U] = snow_0 * 1.6000000238418579f;
    wt_0[0U] = wt_0[0U] * _S213;
    wt_0[1U] = wt_0[1U] * _S213;
    wt_0[2U] = wt_0[2U] * _S213;
    wt_0[3U] = wt_0[3U] * _S213;
    wt_0[5U] = wt_0[5U] * _S213;
    float near_1 = saturate(1.0f - dist_0 / 400.0f);
    float3 _S214 = pow(abs(float3(dot(n_3, tu_3), dot(n_3, tv_4), cf_0)), float3(4.0f) );
    thread float3 tw_0 = _S214;
    float _S215 = _S214.x;
    float _S216 = _S214.y;
    float major_0 = max(_S215, _S216) - min(_S215, _S216);
    float2 _S217;
    if(_S215 > _S216)
    {
        _S217 = float2(major_0, 0.0f);
    }
    else
    {
        _S217 = float2(0.0f, major_0);
    }
    tw_0.xy = _S217;
    float3 _S218 = float3(0.0f) ;
    float3 _S219 = max(tw_0 - float3(0.029999999329447746f) , _S218);
    tw_0 = _S219;
    tw_0.z = max(_S219.z, 0.0099999997764825821f);
    float3 _S220 = tw_0 / float3(max(tw_0.x + tw_0.y + tw_0.z, 9.9999997473787516e-05f)) ;
    tw_0 = _S220;
    TProj_0 _S221 = TProj_x24init_0(w_7, dwx_0, dwy_0);
    bool side_u_0 = (_S220.y) >= (_S220.x);
    if(side_u_0)
    {
        northness_0 = tw_0.y;
    }
    else
    {
        northness_0 = tw_0.x;
    }
    thread TProj_0 side_0;
    if(side_u_0)
    {
        _S217 = float2(w_7.x, h_22);
    }
    else
    {
        _S217 = float2(w_7.y, h_22);
    }
    (&side_0)->w_5 = _S217;
    if(side_u_0)
    {
        _S217 = float2(dwx_0.x, dhx_0);
    }
    else
    {
        _S217 = float2(dwx_0.y, dhx_0);
    }
    (&side_0)->dx_0 = _S217;
    if(side_u_0)
    {
        _S217 = float2(dwy_0.x, dhy_0);
    }
    else
    {
        _S217 = float2(dwy_0.y, dhy_0);
    }
    (&side_0)->dy_0 = _S217;
    float3 _S222;
    if(side_u_0)
    {
        _S222 = tu_3;
    }
    else
    {
        _S222 = tv_4;
    }
    thread array<float4, int(6)> ah_0;
    for(;;)
    {
        for(;;)
        {
            for(;;)
            {
                float4 _S223 = float4(0.0f) ;
                _S203 = _S223;
                ah_0[0U] = _S223;
                if((wt_0[0U]) < 0.039999999105930328f)
                {
                    wt_0[0U] = 0.0f;
                    best_0 = 0.0f;
                    break;
                }
                bool _S224 = northness_0 > 0.0f;
                if((tw_0.z) > 0.0f)
                {
                    _S204 = true;
                }
                else
                {
                    _S204 = !_S224;
                }
                if(_S204)
                {
                    thread TProj_0 _S225 = _S221;
                    float4 _S226 = t_layer_albedo_0(m_5, _S205, 0U, &_S225, near_1, kernelContext_32);
                    nr_0 = _S226;
                }
                else
                {
                    nr_0 = _S223;
                }
                if(_S224)
                {
                    float4 _S227 = nr_0 * float4(tw_0.z) ;
                    thread TProj_0 _S228 = side_0;
                    float4 _S229 = t_layer_albedo_0(m_5, _S205, 0U, &_S228, near_1, kernelContext_32);
                    nr_0 = _S227 + _S229 * float4(northness_0) ;
                }
                ah_0[0U] = nr_0;
                float _S230 = wt_0[0U] + nr_0.w * mix(0.15000000596046448f, 0.60000002384185791f, near_1);
                wt_0[0U] = _S230;
                best_0 = max(0.0f, _S230);
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                ah_0[1U] = _S203;
                if((wt_0[1U]) < 0.039999999105930328f)
                {
                    wt_0[1U] = 0.0f;
                    break;
                }
                thread TProj_0 _S231 = _S221;
                float4 _S232 = t_layer_albedo_0(m_5, _S205, 1U, &_S231, near_1, kernelContext_32);
                ah_0[1U] = _S232;
                float _S233 = wt_0[1U] + _S232.w * mix(0.15000000596046448f, 0.60000002384185791f, near_1);
                wt_0[1U] = _S233;
                best_0 = max(best_0, _S233);
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                ah_0[2U] = _S203;
                if((wt_0[2U]) < 0.039999999105930328f)
                {
                    wt_0[2U] = 0.0f;
                    break;
                }
                thread TProj_0 _S234 = _S221;
                float4 _S235 = t_layer_albedo_0(m_5, _S205, 2U, &_S234, near_1, kernelContext_32);
                ah_0[2U] = _S235;
                float _S236 = wt_0[2U] + _S235.w * mix(0.15000000596046448f, 0.60000002384185791f, near_1);
                wt_0[2U] = _S236;
                best_0 = max(best_0, _S236);
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                ah_0[3U] = _S203;
                if((wt_0[3U]) < 0.039999999105930328f)
                {
                    wt_0[3U] = 0.0f;
                    break;
                }
                thread TProj_0 _S237 = _S221;
                float4 _S238 = t_layer_albedo_0(m_5, _S205, 3U, &_S237, near_1, kernelContext_32);
                ah_0[3U] = _S238;
                float _S239 = wt_0[3U] + _S238.w * mix(0.15000000596046448f, 0.60000002384185791f, near_1);
                wt_0[3U] = _S239;
                best_0 = max(best_0, _S239);
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                ah_0[4U] = _S203;
                if((wt_0[4U]) < 0.039999999105930328f)
                {
                    wt_0[4U] = 0.0f;
                    break;
                }
                bool _S240 = northness_0 > 0.0f;
                if((tw_0.z) > 0.0f)
                {
                    _S204 = true;
                }
                else
                {
                    _S204 = !_S240;
                }
                if(_S204)
                {
                    thread TProj_0 _S241 = _S221;
                    float4 _S242 = t_layer_albedo_0(m_5, _S205, 4U, &_S241, near_1, kernelContext_32);
                    nr_0 = _S242;
                }
                else
                {
                    nr_0 = _S203;
                }
                if(_S240)
                {
                    float4 _S243 = nr_0 * float4(tw_0.z) ;
                    thread TProj_0 _S244 = side_0;
                    float4 _S245 = t_layer_albedo_0(m_5, _S205, 4U, &_S244, near_1, kernelContext_32);
                    nr_0 = _S243 + _S245 * float4(northness_0) ;
                }
                ah_0[4U] = nr_0;
                float _S246 = wt_0[4U] + nr_0.w * mix(0.15000000596046448f, 0.60000002384185791f, near_1);
                wt_0[4U] = _S246;
                best_0 = max(best_0, _S246);
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                ah_0[5U] = _S203;
                if((wt_0[5U]) < 0.039999999105930328f)
                {
                    wt_0[5U] = 0.0f;
                    break;
                }
                thread TProj_0 _S247 = _S221;
                float4 _S248 = t_layer_albedo_0(m_5, _S205, 5U, &_S247, near_1, kernelContext_32);
                ah_0[5U] = _S248;
                float _S249 = wt_0[5U] + _S248.w * mix(0.15000000596046448f, 0.60000002384185791f, near_1);
                wt_0[5U] = _S249;
                best_0 = max(best_0, _S249);
                break;
            }
            break;
        }
        break;
    }
    float s_8;
    if((wt_0[0U]) > 0.0f)
    {
        s_8 = max(wt_0[0U] - (best_0 - 0.34999999403953552f), 0.0f);
    }
    else
    {
        s_8 = 0.0f;
    }
    wt_0[0U] = s_8;
    float _S250;
    if((wt_0[1U]) > 0.0f)
    {
        _S250 = max(wt_0[1U] - (best_0 - 0.34999999403953552f), 0.0f);
    }
    else
    {
        _S250 = 0.0f;
    }
    wt_0[1U] = _S250;
    float total_0 = s_8 + _S250;
    if((wt_0[2U]) > 0.0f)
    {
        s_8 = max(wt_0[2U] - (best_0 - 0.34999999403953552f), 0.0f);
    }
    else
    {
        s_8 = 0.0f;
    }
    wt_0[2U] = s_8;
    float total_1 = total_0 + s_8;
    if((wt_0[3U]) > 0.0f)
    {
        s_8 = max(wt_0[3U] - (best_0 - 0.34999999403953552f), 0.0f);
    }
    else
    {
        s_8 = 0.0f;
    }
    wt_0[3U] = s_8;
    float total_2 = total_1 + s_8;
    if((wt_0[4U]) > 0.0f)
    {
        s_8 = max(wt_0[4U] - (best_0 - 0.34999999403953552f), 0.0f);
    }
    else
    {
        s_8 = 0.0f;
    }
    wt_0[4U] = s_8;
    float total_3 = total_2 + s_8;
    if((wt_0[5U]) > 0.0f)
    {
        best_0 = max(wt_0[5U] - (best_0 - 0.34999999403953552f), 0.0f);
    }
    else
    {
        best_0 = 0.0f;
    }
    wt_0[5U] = best_0;
    float total_4 = total_3 + best_0;
    thread TSurf_0 o_1;
    (&o_1)->albedo_1 = _S218;
    (&o_1)->n_2 = _S218;
    (&o_1)->rough_0 = 0.0f;
    (&o_1)->ao_2 = 0.0f;
    for(;;)
    {
        for(;;)
        {
            for(;;)
            {
                if((wt_0[0U]) <= 0.0f)
                {
                    pert_0 = _S218;
                    break;
                }
                float a_6 = wt_0[0U] / max(total_4, 9.9999997473787516e-06f);
                float3 _S251 = float3(a_6) ;
                (&o_1)->albedo_1 = (&o_1)->albedo_1 + _S251 * ah_0[0U].xyz * (float4((&m_5->gain_0)->data_1[0U]) ).xyz;
                float4 _S252 = float4(0.0f, 0.0f, 0.92000001668930054f, 1.0f);
                if(near_1 > 0.0f)
                {
                    bool _S253 = northness_0 > 0.0f;
                    if((tw_0.z) > 0.0f)
                    {
                        _S204 = true;
                    }
                    else
                    {
                        _S204 = !_S253;
                    }
                    if(_S204)
                    {
                        thread TProj_0 _S254 = _S221;
                        float4 _S255 = t_layer_normal_0(m_5, _S205, 0U, &_S254, kernelContext_32);
                        if(_S253)
                        {
                            s_8 = tw_0.z;
                        }
                        else
                        {
                            s_8 = 1.0f;
                        }
                        float3 _S256 = (tu_3 * float3(_S255.x)  + tv_4 * float3(_S255.y) ) * float3(s_8) ;
                        nr_0 = _S255 * float4(s_8) ;
                        pert_0 = _S256;
                    }
                    else
                    {
                        nr_0 = _S203;
                        pert_0 = _S218;
                    }
                    if(_S253)
                    {
                        thread TProj_0 _S257 = side_0;
                        float4 _S258 = t_layer_normal_0(m_5, _S205, 0U, &_S257, kernelContext_32);
                        float4 nr_1 = nr_0 + _S258 * float4(northness_0) ;
                        pert_0 = pert_0 + (_S222 * float3(_S258.x)  + up_2 * float3(_S258.y) ) * float3(northness_0) ;
                        nr_0 = nr_1;
                    }
                }
                else
                {
                    pert_0 = _S218;
                    nr_0 = _S252;
                }
                float3 _S259 = _S251 * pert_0;
                (&o_1)->rough_0 = (&o_1)->rough_0 + a_6 * nr_0.z;
                (&o_1)->ao_2 = (&o_1)->ao_2 + a_6 * nr_0.w;
                pert_0 = _S259;
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                if((wt_0[1U]) <= 0.0f)
                {
                    break;
                }
                float a_7 = wt_0[1U] / max(total_4, 9.9999997473787516e-06f);
                float3 _S260 = float3(a_7) ;
                (&o_1)->albedo_1 = (&o_1)->albedo_1 + _S260 * ah_0[1U].xyz * (float4((&m_5->gain_0)->data_1[1U]) ).xyz;
                float4 _S261 = float4(0.0f, 0.0f, 0.92000001668930054f, 1.0f);
                if(near_1 > 0.0f)
                {
                    thread TProj_0 _S262 = _S221;
                    float4 _S263 = t_layer_normal_0(m_5, _S205, 1U, &_S262, kernelContext_32);
                    nw_0 = tu_3 * float3(_S263.x)  + tv_4 * float3(_S263.y) ;
                    nr_0 = _S263;
                }
                else
                {
                    nw_0 = _S218;
                    nr_0 = _S261;
                }
                float3 pert_1 = pert_0 + _S260 * nw_0;
                (&o_1)->rough_0 = (&o_1)->rough_0 + a_7 * nr_0.z;
                (&o_1)->ao_2 = (&o_1)->ao_2 + a_7 * nr_0.w;
                pert_0 = pert_1;
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                if((wt_0[2U]) <= 0.0f)
                {
                    break;
                }
                float a_8 = wt_0[2U] / max(total_4, 9.9999997473787516e-06f);
                float3 _S264 = float3(a_8) ;
                (&o_1)->albedo_1 = (&o_1)->albedo_1 + _S264 * ah_0[2U].xyz * (float4((&m_5->gain_0)->data_1[2U]) ).xyz;
                float4 _S265 = float4(0.0f, 0.0f, 0.92000001668930054f, 1.0f);
                if(near_1 > 0.0f)
                {
                    thread TProj_0 _S266 = _S221;
                    float4 _S267 = t_layer_normal_0(m_5, _S205, 2U, &_S266, kernelContext_32);
                    nw_0 = tu_3 * float3(_S267.x)  + tv_4 * float3(_S267.y) ;
                    nr_0 = _S267;
                }
                else
                {
                    nw_0 = _S218;
                    nr_0 = _S265;
                }
                float3 pert_2 = pert_0 + _S264 * nw_0;
                (&o_1)->rough_0 = (&o_1)->rough_0 + a_8 * nr_0.z;
                (&o_1)->ao_2 = (&o_1)->ao_2 + a_8 * nr_0.w;
                pert_0 = pert_2;
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                if((wt_0[3U]) <= 0.0f)
                {
                    break;
                }
                float a_9 = wt_0[3U] / max(total_4, 9.9999997473787516e-06f);
                float3 _S268 = float3(a_9) ;
                (&o_1)->albedo_1 = (&o_1)->albedo_1 + _S268 * ah_0[3U].xyz * (float4((&m_5->gain_0)->data_1[3U]) ).xyz;
                float4 _S269 = float4(0.0f, 0.0f, 0.92000001668930054f, 1.0f);
                if(near_1 > 0.0f)
                {
                    thread TProj_0 _S270 = _S221;
                    float4 _S271 = t_layer_normal_0(m_5, _S205, 3U, &_S270, kernelContext_32);
                    nw_0 = tu_3 * float3(_S271.x)  + tv_4 * float3(_S271.y) ;
                    nr_0 = _S271;
                }
                else
                {
                    nw_0 = _S218;
                    nr_0 = _S269;
                }
                float3 pert_3 = pert_0 + _S268 * nw_0;
                (&o_1)->rough_0 = (&o_1)->rough_0 + a_9 * nr_0.z;
                (&o_1)->ao_2 = (&o_1)->ao_2 + a_9 * nr_0.w;
                pert_0 = pert_3;
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                if((wt_0[4U]) <= 0.0f)
                {
                    break;
                }
                float a_10 = wt_0[4U] / max(total_4, 9.9999997473787516e-06f);
                float3 _S272 = float3(a_10) ;
                (&o_1)->albedo_1 = (&o_1)->albedo_1 + _S272 * ah_0[4U].xyz * (float4((&m_5->gain_0)->data_1[4U]) ).xyz;
                float4 _S273 = float4(0.0f, 0.0f, 0.92000001668930054f, 1.0f);
                if(near_1 > 0.0f)
                {
                    bool _S274 = northness_0 > 0.0f;
                    if((tw_0.z) > 0.0f)
                    {
                        _S204 = true;
                    }
                    else
                    {
                        _S204 = !_S274;
                    }
                    if(_S204)
                    {
                        thread TProj_0 _S275 = _S221;
                        float4 _S276 = t_layer_normal_0(m_5, _S205, 4U, &_S275, kernelContext_32);
                        if(_S274)
                        {
                            s_8 = tw_0.z;
                        }
                        else
                        {
                            s_8 = 1.0f;
                        }
                        float3 _S277 = (tu_3 * float3(_S276.x)  + tv_4 * float3(_S276.y) ) * float3(s_8) ;
                        nr_0 = _S276 * float4(s_8) ;
                        nw_0 = _S277;
                    }
                    else
                    {
                        nr_0 = _S203;
                        nw_0 = _S218;
                    }
                    if(_S274)
                    {
                        thread TProj_0 _S278 = side_0;
                        float4 _S279 = t_layer_normal_0(m_5, _S205, 4U, &_S278, kernelContext_32);
                        float4 nr_2 = nr_0 + _S279 * float4(northness_0) ;
                        nw_0 = nw_0 + (_S222 * float3(_S279.x)  + up_2 * float3(_S279.y) ) * float3(northness_0) ;
                        nr_0 = nr_2;
                    }
                }
                else
                {
                    nw_0 = _S218;
                    nr_0 = _S273;
                }
                float3 pert_4 = pert_0 + _S272 * nw_0;
                (&o_1)->rough_0 = (&o_1)->rough_0 + a_10 * nr_0.z;
                (&o_1)->ao_2 = (&o_1)->ao_2 + a_10 * nr_0.w;
                pert_0 = pert_4;
                break;
            }
            break;
        }
        for(;;)
        {
            for(;;)
            {
                if((wt_0[5U]) <= 0.0f)
                {
                    break;
                }
                float a_11 = wt_0[5U] / max(total_4, 9.9999997473787516e-06f);
                float3 _S280 = float3(a_11) ;
                (&o_1)->albedo_1 = (&o_1)->albedo_1 + _S280 * ah_0[5U].xyz * (float4((&m_5->gain_0)->data_1[5U]) ).xyz;
                float4 _S281 = float4(0.0f, 0.0f, 0.92000001668930054f, 1.0f);
                if(near_1 > 0.0f)
                {
                    thread TProj_0 _S282 = _S221;
                    float4 _S283 = t_layer_normal_0(m_5, _S205, 5U, &_S282, kernelContext_32);
                    nw_0 = tu_3 * float3(_S283.x)  + tv_4 * float3(_S283.y) ;
                    nr_0 = _S283;
                }
                else
                {
                    nw_0 = _S218;
                    nr_0 = _S281;
                }
                float3 pert_5 = pert_0 + _S280 * nw_0;
                (&o_1)->rough_0 = (&o_1)->rough_0 + a_11 * nr_0.z;
                (&o_1)->ao_2 = (&o_1)->ao_2 + a_11 * nr_0.w;
                pert_0 = pert_5;
                break;
            }
            break;
        }
        break;
    }
    if(total_4 <= 0.0f)
    {
        (&o_1)->albedo_1 = float3(0.125f, 0.14000000059604645f, 0.070000000298023224f);
        (&o_1)->rough_0 = 0.92000001668930054f;
        (&o_1)->ao_2 = 1.0f;
    }
    (&o_1)->albedo_1 = mix((&o_1)->albedo_1 * float3((mix(0.81999999284744263f, 1.1499999761581421f, n2_0) * mix(0.89999997615814209f, 1.0800000429153442f, n3_0) * mix(1.0f, 0.80000001192092896f, hollow_f_0 * _S213)))  * mix(float3(1.0f) , float3(1.0399999618530273f, 1.0f, 0.93999999761581421f), float3((n1_0 * _S213)) ), float3(0.029999999329447746f, 0.043999999761581421f, 0.020999999716877937f) * float3(mix(0.85000002384185791f, 1.1499999761581421f, n1_0)) , float3((wt_0[3U] / max(total_4, 9.9999997473787516e-06f) * smoothstep(60.0f, 600.0f, dist_0))) );
    float ns_0 = mix(0.34999999403953552f, 1.0f, near_1);
    (&o_1)->n_2 = normalize(n_3 + pert_0 * float3(ns_0) );
    (&o_1)->ao_2 = mix(1.0f, (&o_1)->ao_2, ns_0);
    return o_1;
}

float cascade_extent_0(FrameData_natural_0 device* f_20, uint c_7, float3 p_5)
{
    float4 s_9 = (((float4(p_5, 1.0f)) * (matrix<float,int(4),int(4)> ((&(&f_20->light_0)->shadow_1)->data_5[c_7].data_3[int(0)][int(0)], (&(&f_20->light_0)->shadow_1)->data_5[c_7].data_3[int(1)][int(0)], (&(&f_20->light_0)->shadow_1)->data_5[c_7].data_3[int(2)][int(0)], (&(&f_20->light_0)->shadow_1)->data_5[c_7].data_3[int(3)][int(0)], (&(&f_20->light_0)->shadow_1)->data_5[c_7].data_3[int(0)][int(1)], (&(&f_20->light_0)->shadow_1)->data_5[c_7].data_3[int(1)][int(1)], (&(&f_20->light_0)->shadow_1)->data_5[c_7].data_3[int(2)][int(1)], (&(&f_20->light_0)->shadow_1)->data_5[c_7].data_3[int(3)][int(1)], (&(&f_20->light_0)->shadow_1)->data_5[c_7].data_3[int(0)][int(2)], (&(&f_20->light_0)->shadow_1)->data_5[c_7].data_3[int(1)][int(2)], (&(&f_20->light_0)->shadow_1)->data_5[c_7].data_3[int(2)][int(2)], (&(&f_20->light_0)->shadow_1)->data_5[c_7].data_3[int(3)][int(2)], (&(&f_20->light_0)->shadow_1)->data_5[c_7].data_3[int(0)][int(3)], (&(&f_20->light_0)->shadow_1)->data_5[c_7].data_3[int(1)][int(3)], (&(&f_20->light_0)->shadow_1)->data_5[c_7].data_3[int(2)][int(3)], (&(&f_20->light_0)->shadow_1)->data_5[c_7].data_3[int(3)][int(3)]))));
    float _S284 = s_9.z;
    bool _S285;
    if(_S284 < 0.0f)
    {
        _S285 = true;
    }
    else
    {
        _S285 = _S284 > 1.0f;
    }
    float _S286;
    if(_S285)
    {
        _S286 = 1.0e+09f;
    }
    else
    {
        _S286 = max(abs(s_9.x), abs(s_9.y));
    }
    return _S286;
}

float pcf_radius_texels_0(FrameData_natural_0 device* f_21, float texel_0)
{
    return clamp((float4((&f_21->light_0)->sky_0) ).z / texel_0, 1.0f, max((float4((&f_21->light_0)->sky_0) ).w, 1.0f));
}

float ed_depth_read_0(uint i_19, uint2 p_6, KernelContext_0 thread* kernelContext_33)
{
    float _S287 = ((reinterpret_cast<device depth2d<float, access::sample>*>((kernelContext_33->ed_textures_0))[(i_19)].read((p_6))));
    return _S287;
}

float cascade_lookup_0(FrameData_natural_0 device* f_22, uint atlas_0, uint c_8, float3 p_7, float3 ng_0, float2 px_1, KernelContext_0 thread* kernelContext_34)
{
    float4 _S288 = float4((&(&f_22->light_0)->cascade_0)->data_6[c_8]) ;
    float tile_1 = (float4((&f_22->light_0)->cfg_0) ).y;
    float _S289 = _S288.x;
    float _S290 = pcf_radius_texels_0(f_22, _S289);
    float sp_0 = _S290 * 0.5f;
    float nol_0 = saturate(dot(ng_0, (float4(f_22->sun_dir_0) ).xyz));
    float4 s_10 = (((float4(p_7 + ng_0 * float3((_S289 * ((float4((&f_22->light_0)->filter_0) ).z + 2.0f * sqrt(1.0f - nol_0 * nol_0)))) , 1.0f)) * (matrix<float,int(4),int(4)> ((&(&f_22->light_0)->shadow_1)->data_5[c_8].data_3[int(0)][int(0)], (&(&f_22->light_0)->shadow_1)->data_5[c_8].data_3[int(1)][int(0)], (&(&f_22->light_0)->shadow_1)->data_5[c_8].data_3[int(2)][int(0)], (&(&f_22->light_0)->shadow_1)->data_5[c_8].data_3[int(3)][int(0)], (&(&f_22->light_0)->shadow_1)->data_5[c_8].data_3[int(0)][int(1)], (&(&f_22->light_0)->shadow_1)->data_5[c_8].data_3[int(1)][int(1)], (&(&f_22->light_0)->shadow_1)->data_5[c_8].data_3[int(2)][int(1)], (&(&f_22->light_0)->shadow_1)->data_5[c_8].data_3[int(3)][int(1)], (&(&f_22->light_0)->shadow_1)->data_5[c_8].data_3[int(0)][int(2)], (&(&f_22->light_0)->shadow_1)->data_5[c_8].data_3[int(1)][int(2)], (&(&f_22->light_0)->shadow_1)->data_5[c_8].data_3[int(2)][int(2)], (&(&f_22->light_0)->shadow_1)->data_5[c_8].data_3[int(3)][int(2)], (&(&f_22->light_0)->shadow_1)->data_5[c_8].data_3[int(0)][int(3)], (&(&f_22->light_0)->shadow_1)->data_5[c_8].data_3[int(1)][int(3)], (&(&f_22->light_0)->shadow_1)->data_5[c_8].data_3[int(2)][int(3)], (&(&f_22->light_0)->shadow_1)->data_5[c_8].data_3[int(3)][int(3)]))));
    float2 _S291 = float2(s_10.x * 0.5f + 0.5f, 0.5f - s_10.y * 0.5f) * float2(tile_1) ;
    float ign_0 = fract(52.982917785644531f * fract(dot(px_1, float2(0.067110560834407806f, 0.0058371499180793762f))));
    float2 _S292 = float2(0.5f) ;
    float2 _S293 = (float2(ign_0, fract(ign_0 * 7.309999942779541f + 0.37000000476837158f)) - _S292) * float2(0.0f) ;
    float3 ns_1 = (((float4(ng_0, 0.0f)) * (matrix<float,int(4),int(4)> ((&(&f_22->light_0)->shadow_1)->data_5[c_8].data_3[int(0)][int(0)], (&(&f_22->light_0)->shadow_1)->data_5[c_8].data_3[int(1)][int(0)], (&(&f_22->light_0)->shadow_1)->data_5[c_8].data_3[int(2)][int(0)], (&(&f_22->light_0)->shadow_1)->data_5[c_8].data_3[int(3)][int(0)], (&(&f_22->light_0)->shadow_1)->data_5[c_8].data_3[int(0)][int(1)], (&(&f_22->light_0)->shadow_1)->data_5[c_8].data_3[int(1)][int(1)], (&(&f_22->light_0)->shadow_1)->data_5[c_8].data_3[int(2)][int(1)], (&(&f_22->light_0)->shadow_1)->data_5[c_8].data_3[int(3)][int(1)], (&(&f_22->light_0)->shadow_1)->data_5[c_8].data_3[int(0)][int(2)], (&(&f_22->light_0)->shadow_1)->data_5[c_8].data_3[int(1)][int(2)], (&(&f_22->light_0)->shadow_1)->data_5[c_8].data_3[int(2)][int(2)], (&(&f_22->light_0)->shadow_1)->data_5[c_8].data_3[int(3)][int(2)], (&(&f_22->light_0)->shadow_1)->data_5[c_8].data_3[int(0)][int(3)], (&(&f_22->light_0)->shadow_1)->data_5[c_8].data_3[int(1)][int(3)], (&(&f_22->light_0)->shadow_1)->data_5[c_8].data_3[int(2)][int(3)], (&(&f_22->light_0)->shadow_1)->data_5[c_8].data_3[int(3)][int(3)])))).xyz;
    float rl_0 = _S289 * tile_1 * 0.5f;
    float _S294 = _S288.y;
    float _S295 = ns_1.z * _S294;
    float izn_0 = 1.0f / max(abs(_S295), 0.11999999731779099f);
    float _S296 = float((int(sign((_S295)))));
    float gx_1 = clamp(- (ns_1.x * rl_0) * izn_0 * _S296, -8.0f, 8.0f);
    float gy_1 = clamp(- (ns_1.y * rl_0) * izn_0 * _S296, -8.0f, 8.0f);
    float _S297 = s_10.z + ((float4((&f_22->light_0)->filter_0) ).w + 0.5f * (abs(gx_1) + abs(gy_1))) * _S289 / _S294;
    float _S298 = _S289 / _S294;
    int2 _S299 = int2(_S288.zw);
    int2 _S300 = _S299 + int2((int(tile_1) - int(1))) ;
    float _S301 = 1.0f / (_S290 + sp_0);
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
        float _S302 = 1.0f - abs(oy_0) * _S301;
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
            float w_8 = (1.0f - abs(ox_0) * _S301) * _S302;
            float2 tp_1 = floor(_S291 + float2(ox_0, oy_0) + _S293);
            float2 d_25 = tp_1 + _S292 - _S291;
            float _S303 = ed_depth_read_0(atlas_0, uint2(clamp(_S299 + int2(tp_1), _S299, _S300)), kernelContext_34);
            float _S304;
            if((_S297 + (gx_1 * d_25.x - gy_1 * d_25.y) * _S298) >= _S303)
            {
                _S304 = 1.0f;
            }
            else
            {
                _S304 = 0.0f;
            }
            float lit_1 = lit_0 + _S304 * w_8;
            float wsum_1 = wsum_0 + w_8;
            x_0 = x_0 + int(1);
            lit_0 = lit_1;
            wsum_0 = wsum_1;
        }
        y_0 = y_0 + int(1);
    }
    return lit_0 / wsum_0;
}

float sun_shadow_0(FrameData_natural_0 device* f_23, uint atlas_1, float3 p_8, float3 ng_1, float dist_1, float2 px_2, KernelContext_0 thread* kernelContext_35)
{
    uint n_4 = uint((float4((&f_23->light_0)->cfg_0) ).x);
    bool _S305;
    if(((uint4((&f_23->light_0)->flags_0) ).x) == 0U)
    {
        _S305 = true;
    }
    else
    {
        _S305 = n_4 == 0U;
    }
    if(_S305)
    {
        _S305 = true;
    }
    else
    {
        _S305 = dist_1 >= ((float4((&f_23->light_0)->filter_0) ).y);
    }
    if(_S305)
    {
        return 1.0f;
    }
    float _S306 = 1.0f - (float4((&f_23->light_0)->cfg_0) ).w * 2.0f / (float4((&f_23->light_0)->cfg_0) ).y;
    float _S307 = (float4((&f_23->light_0)->cfg_0) ).z;
    uint c_9 = 0U;
    for(;;)
    {
        if(c_9 < n_4)
        {
        }
        else
        {
            break;
        }
        float _S308 = cascade_extent_0(f_23, c_9, p_8);
        float e_8 = _S308 / _S306;
        if(e_8 > 1.0f)
        {
            c_9 = c_9 + 1U;
            continue;
        }
        float _S309 = cascade_lookup_0(f_23, atlas_1, c_9, p_8, ng_1, px_2, kernelContext_35);
        float blend_0 = smoothstep(1.0f - _S307, 1.0f, e_8);
        float v_3;
        if(blend_0 > 0.0f)
        {
            uint _S310 = c_9 + 1U;
            if(_S310 < n_4)
            {
                float _S311 = cascade_extent_0(f_23, _S310, p_8);
                _S305 = (_S311 / _S306) <= 1.0f;
            }
            else
            {
                _S305 = false;
            }
            if(_S305)
            {
                float _S312 = cascade_lookup_0(f_23, atlas_1, _S310, p_8, ng_1, px_2, kernelContext_35);
                v_3 = _S312;
            }
            else
            {
                v_3 = 1.0f;
            }
            v_3 = mix(_S309, v_3, blend_0);
        }
        else
        {
            v_3 = _S309;
        }
        return mix(v_3, 1.0f, smoothstep(0.0f, 1.0f, saturate((dist_1 - (float4((&f_23->light_0)->filter_0) ).x) / max((float4((&f_23->light_0)->filter_0) ).y - (float4((&f_23->light_0)->filter_0) ).x, 0.0010000000474974513f))));
    }
    return 1.0f;
}

float2 ao_upsample_0(FrameData_natural_0 device* f_24, uint ao_buf_0, float2 pixel_1, float zlin_0, KernelContext_0 thread* kernelContext_36)
{
    float sv_0;
    if(((uint4((&f_24->light_0)->flags_0) ).y) == 0U)
    {
        return float2(1.0f) ;
    }
    packed_float4 device* _S313 = ed_buf_8(ao_buf_0, kernelContext_36);
    uint _S314 = uint((float4((&f_24->light_0)->ao_1) ).y);
    uint _S315 = uint((float4((&f_24->light_0)->ao_1) ).z);
    float2 _S316 = float2(0.5f) ;
    float2 h_23 = (pixel_1 - _S316) * _S316;
    float2 b_7 = floor(h_23);
    float2 _S317 = h_23 - b_7;
    float2 _S318 = float2(0.0f) ;
    int y_1 = int(0);
    float2 sum_0 = _S318;
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
            int2 t_4 = clamp(int2(b_7) + int2(x_1, y_1), int2(int(0)) , int2(int(_S314) - int(1), int(_S315) - int(1)));
            float4 _S319 = float4(*(_S313 + (uint(t_4.y) * _S314 + uint(t_4.x)))) ;
            if(x_1 == int(0))
            {
                sv_0 = 1.0f - _S317.x;
            }
            else
            {
                sv_0 = _S317.x;
            }
            float _S320;
            if(y_1 == int(0))
            {
                _S320 = 1.0f - _S317.y;
            }
            else
            {
                _S320 = _S317.y;
            }
            float w_9 = max(sv_0 * _S320, 0.0010000000474974513f) * (1.0f / (0.0010000000474974513f + abs(_S319.z - zlin_0) / max(zlin_0, 0.0010000000474974513f) * 50.0f));
            float2 sum_1 = sum_0 + _S319.xy * float2(w_9) ;
            float wsum_3 = wsum_2 + w_9;
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
    if(((float4((&f_24->light_0)->skyvis_0) ).x) > 0.0f)
    {
        sv_0 = saturate(r_5.y);
    }
    else
    {
        sv_0 = 1.0f;
    }
    return float2(pow(saturate(r_5.x), (float4((&f_24->light_0)->ao_1) ).x), sv_0);
}

float atm_height_at_0(float rb_5, float h_24, float mu_12, float t_5)
{
    float q_5 = t_5 * t_5 + 2.0f * (rb_5 + h_24) * mu_12 * t_5 + h_24 * (2.0f * rb_5 + h_24);
    return q_5 / (sqrt(max(rb_5 * rb_5 + q_5, 0.0f)) + rb_5);
}

float atm_earth_shadow_0(FrameData_natural_0 device* f_25, float h_25, float mu_s_4)
{
    float _S321 = atm_rb_1(f_25);
    float ch_0 = atm_cos_horizon_0(_S321, h_25);
    float s_11 = 0.0046999999321997166f * sqrt(max(1.0f - ch_0 * ch_0, 0.0f)) + 9.9999997473787516e-06f;
    return smoothstep(- s_11, s_11, mu_s_4 - ch_0);
}

float3 atm_sun_transmittance_0(FrameData_natural_0 device* f_26, float h_26, float mu_s_5, KernelContext_0 thread* kernelContext_37)
{
    float3 _S322 = atm_transmittance_1(f_26, h_26, mu_s_5, kernelContext_37);
    float _S323 = atm_earth_shadow_0(f_26, h_26, mu_s_5);
    return _S322 * float3(_S323) ;
}

float3 atmos_sun_at_0(FrameData_natural_0 device* f_27, float3 p_9, KernelContext_0 thread* kernelContext_38)
{
    float t_6 = length(p_9) * 0.0010000000474974513f;
    float3 d_26;
    if(t_6 > 0.0f)
    {
        d_26 = p_9 * float3((0.0010000000474974513f / t_6)) ;
    }
    else
    {
        d_26 = (float4((&f_27->atmos_0)->up_0) ).xyz;
    }
    float _S324 = atm_rb_0(f_27);
    float h0_0 = (float4((&f_27->atmos_0)->radii_0) ).z;
    float h_27 = atm_height_at_0(_S324, h0_0, dot(d_26, (float4((&f_27->atmos_0)->up_0) ).xyz), t_6);
    float3 _S325 = (float4((&f_27->atmos_0)->sun_0) ).xyz;
    float3 _S326 = atm_sun_transmittance_0(f_27, h_27, clamp(((_S324 + h0_0) * (float4((&f_27->atmos_0)->up_0) ).w + t_6 * dot(d_26, (float4(f_27->sun_dir_0) ).xyz)) / (_S324 + h_27), -1.0f, 1.0f), kernelContext_38);
    return _S325 * _S326;
}

float3 sun_light_at_0(FrameData_natural_0 device* f_28, float3 p_10, KernelContext_0 thread* kernelContext_39)
{
    bool _S327 = atmos_on_0(f_28);
    float3 _S328;
    if(_S327)
    {
        float3 _S329 = atmos_sun_at_0(f_28, p_10, kernelContext_39);
        _S328 = _S329;
    }
    else
    {
        _S328 = (float4(f_28->sun_color_0) ).xyz;
    }
    return _S328;
}

float3 f_schlick_0(float3 f0_0, float voh_0)
{
    return f0_0 + (float3(1.0f)  - f0_0) * float3(pow(1.0f - voh_0, 5.0f)) ;
}

float d_ggx_0(float noh_0, float a_12)
{
    float a2_0 = a_12 * a_12;
    float d_27 = noh_0 * noh_0 * (a2_0 - 1.0f) + 1.0f;
    return a2_0 / (3.1415927410125732f * d_27 * d_27);
}

float v_smith_0(float nov_0, float nol_1, float a_13)
{
    float a2_1 = a_13 * a_13;
    float _S330 = 1.0f - a2_1;
    return 0.5f / max(nol_1 * sqrt(nov_0 * nov_0 * _S330 + a2_1) + nov_0 * sqrt(nol_1 * nol_1 * _S330 + a2_1), 9.9999997473787516e-06f);
}

float2 env_brdf_0(float nov_1, float rough_1)
{
    float4 r_6 = float4(rough_1)  * float4(-1.0f, -0.027499999850988388f, -0.57200002670288086f, 0.02199999988079071f) + float4(1.0f, 0.042500000447034836f, 1.0399999618530273f, -0.039999999105930328f);
    float _S331 = r_6.x;
    return float2(-1.0399999618530273f, 1.0399999618530273f) * float2((min(_S331 * _S331, exp2(-9.2799997329711914f * nov_1)) * _S331 + r_6.y))  + r_6.zw;
}

float3 sky_env_0(FrameData_natural_0 device* f_29, float3 d_28, KernelContext_0 thread* kernelContext_40)
{
    bool _S332 = atmos_on_1(f_29);
    float3 _S333;
    if(_S332)
    {
        float3 _S334 = atmos_sky_1(f_29, d_28, kernelContext_40);
        _S333 = _S334;
    }
    else
    {
        float3 _S335 = sky_rad_1(f_29, d_28, 0.0f);
        _S333 = _S335;
    }
    return _S333;
}

float3 sh_irradiance_0(FrameData_natural_0 device* f_30, float3 n_5, KernelContext_0 thread* kernelContext_41)
{
    thread array<float4, int(9)> s_12;
    packed_float4 device* _S336 = ed_buf_8((uint4((&f_30->atmos_0)->bufs2_0) ).z, kernelContext_41);
    uint k_7 = 0U;
    for(;;)
    {
        if(k_7 < 9U)
        {
        }
        else
        {
            break;
        }
        bool _S337 = atmos_on_0(f_30);
        float4 _S338;
        if(_S337)
        {
            _S338 = float4(*(_S336 + (288U + k_7))) ;
        }
        else
        {
            _S338 = float4((&(&f_30->light_0)->sh_0)->data_7[k_7]) ;
        }
        s_12[k_7] = _S338;
        k_7 = k_7 + 1U;
    }
    float _S339 = n_5.y;
    float _S340 = n_5.z;
    float _S341 = n_5.x;
    return max(s_12[int(0)].xyz + (s_12[int(1)].xyz * float3(_S339)  + s_12[int(2)].xyz * float3(_S340)  + s_12[int(3)].xyz * float3(_S341) ) + (s_12[int(4)].xyz * float3((_S341 * _S339))  + s_12[int(5)].xyz * float3((_S339 * _S340))  + s_12[int(6)].xyz * float3((3.0f * _S340 * _S340 - 1.0f)) ) + (s_12[int(7)].xyz * float3((_S341 * _S340))  + s_12[int(8)].xyz * float3((_S341 * _S341 - _S339 * _S339)) ), float3(0.0f) );
}

float3 hemi_ambient_0(FrameData_natural_0 device* f_31, float3 n_6)
{
    return mix((float4(f_31->ground_color_0) ).xyz * float3((float4(f_31->sun_dir_0) ).w) , mix((float4(f_31->sky_horizon_0) ).xyz, (float4(f_31->sky_zenith_0) ).xyz, float3(0.5f) ) * float3((float4(f_31->sun_dir_0) ).w) , float3((n_6.y * 0.5f + 0.5f)) );
}

float3 ambient_light_0(FrameData_natural_0 device* f_32, float3 n_7, KernelContext_0 thread* kernelContext_42)
{
    float3 _S342;
    if(((uint4((&f_32->light_0)->flags_0) ).z) != 0U)
    {
        float3 _S343 = sh_irradiance_0(f_32, n_7, kernelContext_42);
        _S342 = _S343;
    }
    else
    {
        float3 _S344 = hemi_ambient_0(f_32, n_7);
        _S342 = _S344;
    }
    return _S342;
}

float3 occluded_ambient_0(FrameData_natural_0 device* f_33, float3 open_0, float sv_1, KernelContext_0 thread* kernelContext_43)
{
    float3 _S345 = ambient_light_0(f_33, float3(0.0f, -1.0f, 0.0f), kernelContext_43);
    return mix(_S345 * float3((float4((&f_33->light_0)->skyvis_0) ).y) , open_0, float3(sv_1) );
}

float3 ao_multibounce_0(FrameData_natural_0 device* f_34, float ao_3, float3 albedo_2)
{
    float3 _S346 = float3(ao_3) ;
    return mix(_S346, max(_S346, ((_S346 * (float3(2.0404000282287598f)  * albedo_2 - float3(0.33239999413490295f) ) + (float3(-4.795100212097168f)  * albedo_2 + float3(0.64170002937316895f) )) * _S346 + (float3(2.7551999092102051f)  * albedo_2 + float3(0.69029998779296875f) )) * _S346), float3((float4((&f_34->light_0)->sky_0) ).y) );
}

float3 shade_pbr_0(FrameData_natural_0 device* f_35, float3 sun_3, float3 albedo_3, float metal_0, float rough_2, float ao_4, float sv_2, float sun_vis_0, float3 n_8, float3 v_4, KernelContext_0 thread* kernelContext_44)
{
    float _S347 = saturate(metal_0);
    float _S348 = clamp(rough_2, 0.039999999105930328f, 1.0f);
    float a_14 = _S348 * _S348;
    float3 l_3 = (float4(f_35->sun_dir_0) ).xyz;
    float3 h_28 = normalize(l_3 + v_4);
    float nol_2 = saturate(dot(n_8, l_3));
    float _S349 = max(dot(n_8, v_4), 9.9999997473787516e-05f);
    float3 f0_1 = mix(float3(0.039999999105930328f) , albedo_3, float3(_S347) );
    float3 diff_col_0 = albedo_3 * float3((1.0f - _S347)) ;
    float3 F_0 = f_schlick_0(f0_1, saturate(dot(v_4, h_28)));
    float3 _S350 = float3(1.0f) ;
    float3 direct_0 = (diff_col_0 * (_S350 - F_0) / float3(3.1415927410125732f)  + float3((d_ggx_0(saturate(dot(n_8, h_28)), a_14) * v_smith_0(_S349, nol_2, a_14)))  * F_0) * sun_3 * float3((nol_2 * sun_vis_0)) ;
    float2 ab_0 = env_brdf_0(_S349, _S348);
    float3 r_7 = reflect(- v_4, n_8);
    float3 env_0;
    if(((uint4((&f_35->light_0)->flags_0) ).z) != 0U)
    {
        float3 _S351 = sky_env_0(f_35, r_7, kernelContext_44);
        float3 _S352 = sh_irradiance_0(f_35, r_7, kernelContext_44);
        env_0 = mix(_S351, _S352, float3(_S348) );
    }
    else
    {
        float3 _S353 = hemi_ambient_0(f_35, r_7);
        env_0 = _S353;
    }
    float3 _S354 = occluded_ambient_0(f_35, env_0, sv_2, kernelContext_44);
    float spec_occ_0 = saturate(pow(_S349 + ao_4, exp2(-16.0f * a_14 - 1.0f)) - 1.0f + ao_4);
    float3 f_env_0 = f0_1 * float3(ab_0.x)  + float3(ab_0.y) ;
    float3 _S355 = diff_col_0 * (_S350 - f_env_0);
    float3 _S356 = ambient_light_0(f_35, n_8, kernelContext_44);
    float3 _S357 = occluded_ambient_0(f_35, _S356, sv_2, kernelContext_44);
    float3 _S358 = _S355 * _S357;
    float3 _S359 = ao_multibounce_0(f_35, ao_4, diff_col_0);
    return direct_0 + (_S358 * _S359 + _S354 * f_env_0 * float3(spec_occ_0) );
}

float atm_ap_slice_0(FrameData_natural_0 device* f_36, float t_7)
{
    float n_9 = (float4((&f_36->atmos_0)->ap_0) ).x;
    return log2(1.0f + max(t_7, 0.0f) / n_9) / log2(1.0f + (float4((&f_36->atmos_0)->ap_0) ).y / n_9) * 32.0f;
}

void atm_froxel_xy_0(packed_float4 device* l_4, packed_float4 device* t_8, float2 g_6, uint2 i_20, uint k_8, float3 thread* lo_1, float3 thread* to_0)
{
    uint b_8 = (k_8 * 32U + i_20.y) * 32U + i_20.x;
    uint _S360 = b_8 + 1U;
    float4 _S361 = float4(g_6.x) ;
    uint _S362 = b_8 + 32U;
    uint _S363 = _S362 + 1U;
    float4 t0_0 = mix(float4(*(t_8 + b_8)) , float4(*(t_8 + _S360)) , _S361);
    float4 t1_0 = mix(float4(*(t_8 + _S362)) , float4(*(t_8 + _S363)) , _S361);
    float4 _S364 = float4(g_6.y) ;
    *lo_1 = mix(mix(float4(*(l_4 + b_8)) , float4(*(l_4 + _S360)) , _S361), mix(float4(*(l_4 + _S362)) , float4(*(l_4 + _S363)) , _S361), _S364).xyz;
    *to_0 = mix(t0_0, t1_0, _S364).xyz;
    return;
}

void atmos_aerial_0(FrameData_natural_0 device* f_37, float3 p_11, float2 px_3, float3 thread* l_5, float3 thread* tr_0, KernelContext_0 thread* kernelContext_45)
{
    *l_5 = float3(0.0f) ;
    float3 _S365 = float3(1.0f) ;
    *tr_0 = _S365;
    float t_9 = length(p_11) * 0.0010000000474974513f;
    if(t_9 <= 0.0f)
    {
        return;
    }
    float h_29 = (float4((&f_37->atmos_0)->radii_0) ).z;
    float _S366 = atm_rb_0(f_37);
    float _S367 = atm_rt_0(f_37);
    float te_3;
    if(h_29 > (_S367 - _S366))
    {
        float te_4 = atm_enter_top_0(_S366, _S367, h_29, dot(p_11 * float3((0.0010000000474974513f / t_9)) , (float4((&f_37->atmos_0)->up_0) ).xyz));
        bool _S368;
        if(te_4 < 0.0f)
        {
            _S368 = true;
        }
        else
        {
            _S368 = t_9 <= te_4;
        }
        if(_S368)
        {
            return;
        }
        te_3 = te_4;
    }
    else
    {
        te_3 = 0.0f;
    }
    float _S369 = atm_ap_slice_0(f_37, t_9 - te_3);
    float2 q_6 = clamp(px_3 / (float4(f_37->viewport_0) ).zw * float2(32.0f)  - float2(0.5f) , float2(0.0f) , float2(31.0f) );
    uint2 _S370 = min(uint2(q_6), uint2(30U) );
    float2 g_7 = q_6 - float2(_S370);
    packed_float4 device* _S371 = ed_buf_8((uint4((&f_37->atmos_0)->bufs2_0) ).x, kernelContext_45);
    packed_float4 device* _S372 = ed_buf_8((uint4((&f_37->atmos_0)->bufs2_0) ).y, kernelContext_45);
    thread float3 la_0;
    thread float3 ta_0;
    thread float3 lc_3;
    thread float3 tc_0;
    if(_S369 < 1.0f)
    {
        atm_froxel_xy_0(_S371, _S372, g_7, _S370, 0U, &lc_3, &tc_0);
        float3 _S373 = float3(_S369) ;
        *l_5 = lc_3 * _S373;
        *tr_0 = mix(_S365, tc_0, _S373);
        return;
    }
    float _S374 = min(_S369 - 1.0f, 31.0f);
    uint _S375 = min(uint(_S374), 30U);
    float gz_0 = _S374 - float(_S375);
    atm_froxel_xy_0(_S371, _S372, g_7, _S370, _S375, &la_0, &ta_0);
    atm_froxel_xy_0(_S371, _S372, g_7, _S370, _S375 + 1U, &lc_3, &tc_0);
    float3 _S376 = float3(gz_0) ;
    *l_5 = mix(la_0, lc_3, _S376);
    *tr_0 = mix(ta_0, tc_0, _S376);
    return;
}

float3 apply_aerial_0(FrameData_natural_0 device* f_38, float3 col_0, float3 p_12, float2 px_4, KernelContext_0 thread* kernelContext_46)
{
    bool _S377 = atmos_on_0(f_38);
    if(!_S377)
    {
        return col_0;
    }
    thread float3 l_6;
    thread float3 tr_1;
    atmos_aerial_0(f_38, p_12, px_4, &l_6, &tr_1, kernelContext_46);
    if(((uint4((&f_38->atmos_0)->cfg_1) ).y) == 1U)
    {
        return tr_1 * float3(0.5f) ;
    }
    if(((uint4((&f_38->atmos_0)->cfg_1) ).y) == 2U)
    {
        return l_6;
    }
    return col_0 * tr_1 + l_6;
}

uint shadow_cascade_0(FrameData_natural_0 device* f_39, float3 p_13, float dist_2)
{
    uint _S378 = uint((float4((&f_39->light_0)->cfg_0) ).x);
    bool _S379;
    if(((uint4((&f_39->light_0)->flags_0) ).x) == 0U)
    {
        _S379 = true;
    }
    else
    {
        _S379 = dist_2 >= ((float4((&f_39->light_0)->filter_0) ).y);
    }
    if(_S379)
    {
        return 4U;
    }
    float _S380 = 1.0f - (float4((&f_39->light_0)->cfg_0) ).w * 2.0f / (float4((&f_39->light_0)->cfg_0) ).y;
    uint c_10 = 0U;
    for(;;)
    {
        if(c_10 < _S378)
        {
        }
        else
        {
            break;
        }
        float _S381 = cascade_extent_0(f_39, c_10, p_13);
        if((_S381 / _S380) <= 1.0f)
        {
            return c_10;
        }
        c_10 = c_10 + 1U;
    }
    return 4U;
}

float4 terrain_pixel_0(FrameData_natural_0 device* f_40, const FsOut_0 thread* i_21, uint2 vis_1, float3 dir_8, KernelContext_0 thread* kernelContext_47)
{
    TerrainDraw_natural_0 device* _S382 = ed_buf_7(kernelContext_47->ed_c_0->t_draws_0, kernelContext_47);
    TerrainDraw_natural_0 device* _S383 = _S382 + ((vis_1.x) & 2147483647U);
    TerrainDraw_natural_0 d_29 = *_S383;
    thread TerrainDraw_natural_0 _S384 = *_S383;
    uint pool_13 = kernelContext_47->ed_c_0->t_pool_0;
    uint3 tri_0 = t_triangle_0((vis_1.y) & 268435455U);
    uint _S385 = tri_0.x;
    _S384 = d_29;
    float3 _S386 = t_vertex_0(pool_13, &_S384, _S385, kernelContext_47);
    uint _S387 = tri_0.y;
    float3 _S388 = t_vertex_0(pool_13, &_S384, _S387, kernelContext_47);
    float3 e1_1 = _S388 - _S386;
    uint _S389 = tri_0.z;
    float3 _S390 = t_vertex_0(pool_13, &_S384, _S389, kernelContext_47);
    float3 e2_1 = _S390 - _S386;
    float2 bb_0 = ray_bary_0(dir_8, _S386, e1_1, e2_1);
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
    float3 p_14 = _S386 + e1_1 * float3(b1_1)  + e2_1 * float3(b2_1) ;
    float2 g_8 = t_vertex_grid_0(_S385) * float2((1.0f - b1_1 - b2_1))  + t_vertex_grid_0(_S387) * float2(b1_1)  + t_vertex_grid_0(_S389) * float2(b2_1) ;
    float _S391 = t_sample_spacing_0(&_S384, g_8);
    float _S392 = max(_S391, 0.0099999997764825821f);
    int rn_0 = clamp(int(1.2000000476837158f / _S392 + 0.5f), int(1), int(4));
    thread float3 up_3;
    thread float h_30;
    float3 _S393 = t_smooth_normal_0(pool_13, &_S384, g_8, rn_0, &up_3, &h_30, kernelContext_47);
    int r_8 = clamp(int(24.0f / _S392 + 0.5f), int(1), int(6));
    float3 nm_1;
    if(r_8 == rn_0)
    {
        nm_1 = _S393;
    }
    else
    {
        float3 _S394 = t_smooth_normal_0(pool_13, &_S384, g_8, r_8, &up_3, &h_30, kernelContext_47);
        nm_1 = _S394;
    }
    float3 _S395 = terrain_albedo_0(h_30, dot(nm_1, up_3));
    float mat_ao_0;
    float rough_3;
    float3 n_10;
    float3 albedo_4;
    if((kernelContext_47->ed_c_0->t_mat_0) != 0U)
    {
        TerrainMat_natural_0 device* _S396 = ed_buf_5(kernelContext_47->ed_c_0->t_draws_0, kernelContext_47);
        thread TerrainMat_natural_0 _S397 = *(_S396 + kernelContext_47->ed_c_0->t_mat_0);
        float4 _S398 = float4((&_S384)->shape_0) ;
        float2 w_10 = (float4((&_S384)->tex_1) ).xy + (g_8 - float2(32.0f) ) * float2((_S398.x * _S398.y)) ;
        float3 _S399 = (float4((&_S384)->fu_0) ).xyz;
        float3 tu_4 = normalize(_S399 - up_3 * float3(dot(_S399, up_3)) );
        float3 _S400 = (float4((&_S384)->fv_0) ).xyz;
        float3 tv_5 = normalize(_S400 - up_3 * float3(dot(_S400, up_3)) );
        float2 _S401 = i_21->pos_0.xy;
        float3 _S402 = view_dir_0(f_40, _S401 + float2(1.0f, 0.0f));
        float2 bx_0 = ray_bary_0(_S402, _S386, e1_1, e2_1);
        float3 _S403 = view_dir_0(f_40, _S401 + float2(0.0f, 1.0f));
        float2 by_0 = ray_bary_0(_S403, _S386, e1_1, e2_1);
        float3 dpx_0 = _S386 + e1_1 * float3(bx_0.x)  + e2_1 * float3(bx_0.y)  - p_14;
        float3 dpy_0 = _S386 + e1_1 * float3(by_0.x)  + e2_1 * float3(by_0.y)  - p_14;
        float2 dwx_1 = float2(dot(dpx_0, tu_4), dot(dpx_0, tv_5));
        float2 dwy_1 = float2(dot(dpy_0, tu_4), dot(dpy_0, tv_5));
        float2 sp_1 = g_8 * float2(4.0f) ;
        int rf_0 = clamp(int(2.0f / _S392 + 0.5f), int(1), int(4));
        float _S404 = float(clamp(int(8.0f / _S392 + 0.5f), int(1), int(16)));
        float2 _S405 = float2(_S404, 0.0f);
        float _S406 = t_page_bilin_0(pool_13, &_S384, sp_1 + _S405, kernelContext_47);
        float _S407 = t_page_bilin_0(pool_13, &_S384, sp_1 - _S405, kernelContext_47);
        float _S408 = _S406 + _S407;
        float2 _S409 = float2(0.0f, _S404);
        float _S410 = t_page_bilin_0(pool_13, &_S384, sp_1 + _S409, kernelContext_47);
        float _S411 = _S408 + _S410;
        float _S412 = t_page_bilin_0(pool_13, &_S384, sp_1 - _S409, kernelContext_47);
        float lap_0 = _S411 + _S412 - 4.0f * h_30;
        if((length_squared_0(p_14)) < 1.6e+05f)
        {
            float _S413 = float(rf_0);
            float2 _S414 = float2(_S413, 0.0f);
            float _S415 = t_page_bilin_0(pool_13, &_S384, sp_1 + _S414, kernelContext_47);
            float _S416 = t_page_bilin_0(pool_13, &_S384, sp_1 - _S414, kernelContext_47);
            float _S417 = _S415 + _S416;
            float2 _S418 = float2(0.0f, _S413);
            float _S419 = t_page_bilin_0(pool_13, &_S384, sp_1 + _S418, kernelContext_47);
            float _S420 = _S417 + _S419;
            float _S421 = t_page_bilin_0(pool_13, &_S384, sp_1 - _S418, kernelContext_47);
            mat_ao_0 = _S420 + _S421 - 4.0f * h_30;
        }
        else
        {
            mat_ao_0 = 0.0f;
        }
        float dl_0 = _S404 * _S392;
        float df_0 = float(rf_0) * _S392;
        TSurf_0 _S422 = terrain_material_0(f_40, &_S397, &_S384, _S393, nm_1, up_3, h_30, lap_0 / (dl_0 * dl_0), mat_ao_0 / (df_0 * df_0), w_10, dwx_1, dwy_1, dot(dpx_0, up_3), dot(dpy_0, up_3), tu_4, tv_5, length(p_14), kernelContext_47);
        n_10 = _S422.n_2;
        mat_ao_0 = _S422.ao_2;
        albedo_4 = _S422.albedo_1;
        rough_3 = _S422.rough_0;
    }
    else
    {
        n_10 = _S393;
        mat_ao_0 = 1.0f;
        albedo_4 = _S395;
        rough_3 = 0.92000001668930054f;
    }
    float3 v_5 = - dir_8;
    float _S423 = dot(n_10, v_5);
    if(_S423 < 0.0f)
    {
        n_10 = normalize(n_10 - float3((1.0099999904632568f * _S423))  * v_5);
    }
    float zlin_1 = - (((float4(p_14, 1.0f)) * (matrix<float,int(4),int(4)> (f_40->view_0.data_3[int(0)][int(0)], f_40->view_0.data_3[int(1)][int(0)], f_40->view_0.data_3[int(2)][int(0)], f_40->view_0.data_3[int(3)][int(0)], f_40->view_0.data_3[int(0)][int(1)], f_40->view_0.data_3[int(1)][int(1)], f_40->view_0.data_3[int(2)][int(1)], f_40->view_0.data_3[int(3)][int(1)], f_40->view_0.data_3[int(0)][int(2)], f_40->view_0.data_3[int(1)][int(2)], f_40->view_0.data_3[int(2)][int(2)], f_40->view_0.data_3[int(3)][int(2)], f_40->view_0.data_3[int(0)][int(3)], f_40->view_0.data_3[int(1)][int(3)], f_40->view_0.data_3[int(2)][int(3)], f_40->view_0.data_3[int(3)][int(3)])))).z;
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
    if((dot(ng_3, v_5)) < 0.0f)
    {
        ng_3 = - ng_3;
    }
    float2 _S424 = i_21->pos_0.xy;
    float _S425 = sun_shadow_0(f_40, kernelContext_47->ed_c_0->shadow_0, p_14, ng_3, zlin_1, _S424, kernelContext_47);
    float2 _S426 = ao_upsample_0(f_40, kernelContext_47->ed_c_0->ao_0, _S424, zlin_1, kernelContext_47);
    float ao_5 = _S426.x * mat_ao_0;
    float sv_3 = _S426.y;
    float3 _S427 = sun_light_at_0(f_40, p_14, kernelContext_47);
    float3 _S428 = shade_pbr_0(f_40, _S427, albedo_4, 0.0f, rough_3, ao_5, sv_3, _S425, n_10, v_5, kernelContext_47);
    float3 _S429 = apply_aerial_0(f_40, _S428, p_14, _S424, kernelContext_47);
    float3 col_1;
    if(((uint4((&f_40->light_0)->flags_0) ).w) == 1U)
    {
        uint _S430 = shadow_cascade_0(f_40, p_14, zlin_1);
        col_1 = TINT_1[_S430] * float3((0.039999999105930328f + 0.5f * _S425)) ;
    }
    else
    {
        if(((uint4((&f_40->light_0)->flags_0) ).w) == 2U)
        {
            col_1 = float3((0.5f * ao_5)) ;
        }
        else
        {
            if(((uint4((&f_40->light_0)->flags_0) ).w) == 4U)
            {
                col_1 = float3((0.5f * sv_3)) ;
            }
            else
            {
                col_1 = _S429;
            }
        }
    }
    return float4(col_1, 1.0f);
}

float3 xform_point_0(const GpuInstance_natural_0 thread* i_22, float3 p_15)
{
    float4 _S431 = float4((&i_22->m_0)->data_0[int(0)]) ;
    float4 _S432 = float4((&i_22->m_0)->data_0[int(1)]) ;
    float4 _S433 = float4((&i_22->m_0)->data_0[int(2)]) ;
    return float3(dot(_S431.xyz, p_15) + _S431.w, dot(_S432.xyz, p_15) + _S432.w, dot(_S433.xyz, p_15) + _S433.w);
}

float2 unpack_snorm2x16_0(uint p_16)
{
    return clamp(float2(int2(int(p_16 << 16U) >> 16U, int(p_16) >> 16U)) / float2(32767.0f) , float2(-1.0f) , float2(1.0f) );
}

float3 oct_decode_0(uint packed_0)
{
    float2 e_9 = unpack_snorm2x16_0(packed_0);
    float _S434 = e_9.x;
    float _S435 = e_9.y;
    float _S436 = 1.0f - abs(_S434) - abs(_S435);
    thread float3 n_11 = float3(_S434, _S435, _S436);
    float t_10 = saturate(- _S436);
    float _S437;
    if(_S434 >= 0.0f)
    {
        _S437 = - t_10;
    }
    else
    {
        _S437 = t_10;
    }
    n_11.x = n_11.x + _S437;
    if((n_11.y) >= 0.0f)
    {
        _S437 = - t_10;
    }
    else
    {
        _S437 = t_10;
    }
    n_11.y = n_11.y + _S437;
    return normalize(n_11);
}

float3 xform_normal_0(const GpuInstance_natural_0 thread* i_23, float3 n_12)
{
    float4 _S438 = float4((&i_23->m_0)->data_0[int(0)]) ;
    float4 _S439 = float4((&i_23->m_0)->data_0[int(1)]) ;
    float4 _S440 = float4((&i_23->m_0)->data_0[int(2)]) ;
    float3 a0_0 = float3(_S438.x, _S439.x, _S440.x);
    float3 a1_0 = float3(_S438.y, _S439.y, _S440.y);
    float3 a2_2 = float3(_S438.z, _S439.z, _S440.z);
    return cross(a1_0, a2_2) * float3(n_12.x)  + cross(a2_2, a0_0) * float3(n_12.y)  + cross(a0_0, a1_0) * float3(n_12.z) ;
}

float2 uv_xform_0(const GpuMaterial_natural_0 thread* m_6, float2 uv_5)
{
    float r_9 = (as_type<float>(((uint4(m_6->tex_0) ).w)));
    float s_13 = sin(r_9);
    float c_11 = cos(r_9);
    float4 _S441 = float4(m_6->uv_0) ;
    float2 q_7 = uv_5 * _S441.zw;
    float _S442 = q_7.x;
    float _S443 = q_7.y;
    return float2(c_11 * _S442 + s_13 * _S443, - s_13 * _S442 + c_11 * _S443) + _S441.xy;
}

float3 shade_toon_0(FrameData_natural_0 device* f_41, float3 sun_4, const GpuMaterial_natural_0 thread* m_7, float3 albedo_5, float3 n_13, float3 v_6, float ao_6, float sv_4, float sun_vis_1, KernelContext_0 thread* kernelContext_48)
{
    float ndl_0 = dot(n_13, (float4(f_41->sun_dir_0) ).xyz);
    float _S444 = max((float4(m_7->params_0) ).z, 1.0f);
    float lit_2 = floor(saturate(ndl_0 * 0.5f + 0.5f) * _S444 + 0.5f) / _S444 * sun_vis_1;
    float4 _S445 = float4(m_7->shade_0) ;
    float3 c_12 = mix(albedo_5 * _S445.xyz, albedo_5, float3(lit_2) );
    float3 _S446 = sun_4 * float3((lit_2 / 3.1415927410125732f)) ;
    float3 _S447 = ambient_light_0(f_41, n_13, kernelContext_48);
    float3 _S448 = occluded_ambient_0(f_41, _S447, sv_4, kernelContext_48);
    float3 _S449 = ao_multibounce_0(f_41, ao_6, albedo_5);
    return c_12 * (_S446 + _S448 * _S449) + float3((pow(1.0f - saturate(dot(n_13, v_6)), 4.0f) * _S445.w * saturate(ndl_0 + 0.30000001192092896f) * sun_vis_1))  * sun_4 * float3(0.15000000596046448f) ;
}

float cascade_depth_delta_0(FrameData_natural_0 device* f_42, uint atlas_2, uint c_13, float3 p_17, KernelContext_0 thread* kernelContext_49)
{
    float4 _S450 = float4((&(&f_42->light_0)->cascade_0)->data_6[c_13]) ;
    float tile_2 = (float4((&f_42->light_0)->cfg_0) ).y;
    float4 s_14 = (((float4(p_17, 1.0f)) * (matrix<float,int(4),int(4)> ((&(&f_42->light_0)->shadow_1)->data_5[c_13].data_3[int(0)][int(0)], (&(&f_42->light_0)->shadow_1)->data_5[c_13].data_3[int(1)][int(0)], (&(&f_42->light_0)->shadow_1)->data_5[c_13].data_3[int(2)][int(0)], (&(&f_42->light_0)->shadow_1)->data_5[c_13].data_3[int(3)][int(0)], (&(&f_42->light_0)->shadow_1)->data_5[c_13].data_3[int(0)][int(1)], (&(&f_42->light_0)->shadow_1)->data_5[c_13].data_3[int(1)][int(1)], (&(&f_42->light_0)->shadow_1)->data_5[c_13].data_3[int(2)][int(1)], (&(&f_42->light_0)->shadow_1)->data_5[c_13].data_3[int(3)][int(1)], (&(&f_42->light_0)->shadow_1)->data_5[c_13].data_3[int(0)][int(2)], (&(&f_42->light_0)->shadow_1)->data_5[c_13].data_3[int(1)][int(2)], (&(&f_42->light_0)->shadow_1)->data_5[c_13].data_3[int(2)][int(2)], (&(&f_42->light_0)->shadow_1)->data_5[c_13].data_3[int(3)][int(2)], (&(&f_42->light_0)->shadow_1)->data_5[c_13].data_3[int(0)][int(3)], (&(&f_42->light_0)->shadow_1)->data_5[c_13].data_3[int(1)][int(3)], (&(&f_42->light_0)->shadow_1)->data_5[c_13].data_3[int(2)][int(3)], (&(&f_42->light_0)->shadow_1)->data_5[c_13].data_3[int(3)][int(3)]))));
    int2 _S451 = int2(_S450.zw);
    float _S452 = s_14.z;
    float _S453 = ed_depth_read_0(atlas_2, uint2(clamp(_S451 + int2(floor(float2(s_14.x * 0.5f + 0.5f, 0.5f - s_14.y * 0.5f) * float2(tile_2) )), _S451, _S451 + int2((int(tile_2) - int(1))) )), kernelContext_49);
    return (_S452 - _S453) * _S450.y;
}

float4 resolve_px_0(const FsOut_0 thread* i_24, bool terrain_0, KernelContext_0 thread* kernelContext_50)
{
    FrameData_natural_0 device* _S454 = ed_buf_9(kernelContext_50->ed_c_0->frame_0, kernelContext_50);
    float2 _S455 = i_24->pos_0.xy;
    uint2 _S456 = uint2(_S455);
    texture2d<uint, access::sample> _S457 = ((reinterpret_cast<device texture2d<uint, access::sample>*>((kernelContext_50->ed_textures_0))[(kernelContext_50->ed_c_0->vis_0)]));
    int3 _S458 = int3(int2(_S456), int(0));
    uint2 vis_2 = ((_S457).read(vec<uint,2>(((_S458)).xy), uint(((_S458)).z))).xy;
    float3 _S459 = view_dir_0(_S454, _S455);
    uint _S460 = vis_2.x;
    if(_S460 == 0U)
    {
        float3 _S461 = sky_pixel_0(_S454, _S459, kernelContext_50);
        return float4(_S461, 1.0f);
    }
    bool textured_0;
    if(terrain_0)
    {
        textured_0 = (_S460 & 2147483648U) != 0U;
    }
    else
    {
        textured_0 = false;
    }
    if(textured_0)
    {
        float4 _S462 = terrain_pixel_0(_S454, i_24, vis_2, _S459, kernelContext_50);
        return _S462;
    }
    GpuInstance_natural_0 device* _S463 = ed_buf_4(kernelContext_50->ed_c_0->instances_0, kernelContext_50);
    thread GpuInstance_natural_0 _S464 = *(_S463 + (_S460 - 1U));
    GpuMesh_natural_0 device* _S465 = ed_buf_3(kernelContext_50->ed_c_0->meshes_0, kernelContext_50);
    GpuMesh_natural_0 device* _S466 = _S465 + (&_S464)->mesh_0;
    uint _S467 = vis_2.y;
    uint lod_1 = _S467 >> 28U;
    uint prim_1 = (_S467 & 268435455U) % max(_S466->index_count_0[lod_1] / 3U, 1U);
    uint device* _S468 = ed_buf_6(kernelContext_50->ed_c_0->indices_0, kernelContext_50);
    uint device* idx_0 = _S468 + _S466->first_index_0[lod_1] + prim_1 * 3U;
    Vtx_0 device* _S469 = ed_buf_2(kernelContext_50->ed_c_0->verts_0, kernelContext_50);
    int i0_0 = int(*(idx_0 + int(0))) + _S466->base_vertex_0[lod_1];
    int i1_0 = int(*(idx_0 + int(1))) + _S466->base_vertex_0[lod_1];
    int i2_0 = int(*(idx_0 + int(2))) + _S466->base_vertex_0[lod_1];
    Vtx_0 v0_0 = *(_S469 + i0_0);
    Vtx_0 v1_0 = *(_S469 + i1_0);
    Vtx_0 v2_0 = *(_S469 + i2_0);
    packed_float4 device* _S470 = ed_buf_8(kernelContext_50->ed_c_0->nodes_0, kernelContext_50);
    float3 node_0 = (float4(*(_S470 + (((&_S464)->node_flags_0) & 65535U))) ).xyz;
    float3 _S471 = xform_point_0(&_S464, float3(v0_0.px_0, v0_0.py_0, v0_0.pz_0));
    float3 _S472 = xform_point_0(&_S464, float3(v1_0.px_0, v1_0.py_0, v1_0.pz_0));
    float3 _S473 = xform_point_0(&_S464, float3(v2_0.px_0, v2_0.py_0, v2_0.pz_0));
    float3 p0_1 = node_0 + _S471;
    float3 e1_2 = _S472 - _S471;
    float3 e2_2 = _S473 - _S471;
    float2 bb_1 = ray_bary_0(_S459, p0_1, e1_2, e2_2);
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
    float3 _S474 = float3((1.0f - b1_3 - b2_4)) ;
    float3 local_0 = _S471 * _S474 + _S472 * float3(b1_3)  + _S473 * float3(b2_4) ;
    float3 _S475 = xform_normal_0(&_S464, oct_decode_0(v0_0.n_0) * _S474 + oct_decode_0(v1_0.n_0) * float3(b1_3)  + oct_decode_0(v2_0.n_0) * float3(b2_4) );
    float3 n_14 = normalize(_S475);
    float3 v_7 = - _S459;
    GpuMaterial_natural_0 device* _S476 = ed_buf_1(kernelContext_50->ed_c_0->materials_0, kernelContext_50);
    thread GpuMaterial_natural_0 _S477 = *(_S476 + (&_S464)->material_0);
    float4 _S478 = float4((&_S477)->base_0) ;
    float3 albedo_6 = _S478.xyz;
    float4 _S479 = float4((&_S477)->params_0) ;
    float metal_1 = _S479.x;
    float rough_4 = _S479.y;
    uint4 _S480 = uint4((&_S477)->tex_0) ;
    uint _S481 = _S480.x;
    bool _S482 = _S481 != 4294967295U;
    if(_S482)
    {
        textured_0 = true;
    }
    else
    {
        textured_0 = (_S480.y) != 4294967295U;
    }
    if(textured_0)
    {
        textured_0 = true;
    }
    else
    {
        textured_0 = (_S480.z) != 4294967295U;
    }
    float ao_7;
    float metal_2;
    float rough_5;
    float d_30;
    float3 albedo_7;
    float3 n_15;
    if(textured_0)
    {
        packed_float2 device* _S483 = ed_buf_0(kernelContext_50->ed_c_0->uvs_0, kernelContext_50);
        float2 _S484 = float2(*(_S483 + i0_0)) ;
        float2 d1_0 = float2(*(_S483 + i1_0))  - _S484;
        float2 d2_0 = float2(*(_S483 + i2_0))  - _S484;
        float2 _S485 = uv_xform_0(&_S477, _S484 + d1_0 * float2(b1_3)  + d2_0 * float2(b2_4) );
        float3 _S486 = view_dir_0(_S454, _S455 + float2(1.0f, 0.0f));
        float2 bx_1 = ray_bary_0(_S486, p0_1, e1_2, e2_2);
        float3 _S487 = view_dir_0(_S454, _S455 + float2(0.0f, 1.0f));
        float2 by_1 = ray_bary_0(_S487, p0_1, e1_2, e2_2);
        float2 _S488 = uv_xform_0(&_S477, _S484 + d1_0 * float2(bx_1.x)  + d2_0 * float2(bx_1.y) );
        float2 ux_0 = _S488 - _S485;
        float2 _S489 = uv_xform_0(&_S477, _S484 + d1_0 * float2(by_1.x)  + d2_0 * float2(by_1.y) );
        float2 uy_0 = _S489 - _S485;
        sampler _S490 = ((reinterpret_cast<device sampler*>((kernelContext_50->ed_samplers_0))[(kernelContext_50->ed_c_0->smp_0)]));
        if(_S482)
        {
            texture2d<float, access::sample> _S491 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_50->ed_textures_0))[(_S481)]));
            albedo_7 = albedo_6 * ((_S491).sample((_S490), (_S485), gradient2d((ux_0), (uy_0)))).xyz;
        }
        else
        {
            albedo_7 = albedo_6;
        }
        uint _S492 = _S480.z;
        if(_S492 != 4294967295U)
        {
            texture2d<float, access::sample> _S493 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_50->ed_textures_0))[(_S492)]));
            float3 orm_0 = ((_S493).sample((_S490), (_S485), gradient2d((ux_0), (uy_0)))).xyz;
            float rough_6 = rough_4 * orm_0.y;
            float metal_3 = metal_1 * orm_0.z;
            ao_7 = orm_0.x;
            metal_2 = metal_3;
            rough_5 = rough_6;
        }
        else
        {
            ao_7 = 1.0f;
            metal_2 = metal_1;
            rough_5 = rough_4;
        }
        uint _S494 = _S480.y;
        if(_S494 != 4294967295U)
        {
            float _S495 = d1_0.x;
            float _S496 = d2_0.y;
            float _S497 = d1_0.y;
            float _S498 = d2_0.x;
            float det_1 = _S495 * _S496 - _S497 * _S498;
            if((abs(det_1)) > 9.999999960041972e-13f)
            {
                float3 _S499 = float3((1.0f / det_1)) ;
                float3 tg_0 = (e1_2 * float3(_S496)  - e2_2 * float3(_S497) ) * _S499;
                float3 bt_0 = (e2_2 * float3(_S495)  - e1_2 * float3(_S498) ) * _S499;
                float3 tg_1 = tg_0 - n_14 * float3(dot(n_14, tg_0)) ;
                if((length_squared_0(tg_1)) > 9.99999968265522539e-21f)
                {
                    float3 tg_2 = normalize(tg_1);
                    float3 _S500 = cross(n_14, tg_2);
                    if((dot(_S500, bt_0)) < 0.0f)
                    {
                        d_30 = -1.0f;
                    }
                    else
                    {
                        d_30 = 1.0f;
                    }
                    float3 bn_0 = _S500 * float3(- d_30) ;
                    texture2d<float, access::sample> _S501 = ((reinterpret_cast<device texture2d<float, access::sample>*>((kernelContext_50->ed_textures_0))[(_S494)]));
                    float2 xy_0 = ((_S501).sample((_S490), (_S485), gradient2d((ux_0), (uy_0)))).xy * float2(2.0f)  - float2(1.0f) ;
                    n_15 = normalize(tg_2 * float3(xy_0.x)  + bn_0 * float3(xy_0.y)  + n_14 * float3(sqrt(saturate(1.0f - dot(xy_0, xy_0)))) );
                }
                else
                {
                    n_15 = n_14;
                }
            }
            else
            {
                n_15 = n_14;
            }
        }
        else
        {
            n_15 = n_14;
        }
    }
    else
    {
        n_15 = n_14;
        albedo_7 = albedo_6;
        ao_7 = 1.0f;
        metal_2 = metal_1;
        rough_5 = rough_4;
    }
    float _S502 = dot(n_15, v_7);
    if(_S502 < 0.0f)
    {
        n_15 = normalize(n_15 - float3((1.0099999904632568f * _S502))  * v_7);
    }
    if((_S479.w) > 0.5f)
    {
        float2 c_14 = floor(local_0.xz * float2(0.5f) );
        albedo_7 = albedo_7 * float3(mix(0.80000001192092896f, 1.1499999761581421f, fmod(abs(c_14.x + c_14.y), 2.0f))) ;
    }
    float3 p_18 = node_0 + local_0;
    float zlin_2 = - (((float4(p_18, 1.0f)) * (matrix<float,int(4),int(4)> (_S454->view_0.data_3[int(0)][int(0)], _S454->view_0.data_3[int(1)][int(0)], _S454->view_0.data_3[int(2)][int(0)], _S454->view_0.data_3[int(3)][int(0)], _S454->view_0.data_3[int(0)][int(1)], _S454->view_0.data_3[int(1)][int(1)], _S454->view_0.data_3[int(2)][int(1)], _S454->view_0.data_3[int(3)][int(1)], _S454->view_0.data_3[int(0)][int(2)], _S454->view_0.data_3[int(1)][int(2)], _S454->view_0.data_3[int(2)][int(2)], _S454->view_0.data_3[int(3)][int(2)], _S454->view_0.data_3[int(0)][int(3)], _S454->view_0.data_3[int(1)][int(3)], _S454->view_0.data_3[int(2)][int(3)], _S454->view_0.data_3[int(3)][int(3)])))).z;
    float3 ng_4 = cross(e1_2, e2_2);
    float3 ng_5;
    if((length_squared_0(ng_4)) > 9.99999968265522539e-21f)
    {
        ng_5 = normalize(ng_4);
    }
    else
    {
        ng_5 = n_15;
    }
    if((dot(ng_5, v_7)) < 0.0f)
    {
        ng_5 = - ng_5;
    }
    float _S503 = sun_shadow_0(_S454, kernelContext_50->ed_c_0->shadow_0, p_18, ng_5, zlin_2, _S455, kernelContext_50);
    float2 _S504 = ao_upsample_0(_S454, kernelContext_50->ed_c_0->ao_0, _S455, zlin_2, kernelContext_50);
    float ao_8 = ao_7 * _S504.x;
    float sv_5 = _S504.y;
    float3 _S505 = sun_light_at_0(_S454, p_18, kernelContext_50);
    float3 col_2;
    if((_S478.w) > 0.5f)
    {
        float3 _S506 = shade_toon_0(_S454, _S505, &_S477, albedo_7, n_15, v_7, ao_8, sv_5, _S503, kernelContext_50);
        col_2 = _S506;
    }
    else
    {
        float3 _S507 = shade_pbr_0(_S454, _S505, albedo_7, metal_2, rough_5, ao_8, sv_5, _S503, n_15, v_7, kernelContext_50);
        col_2 = _S507;
    }
    float3 _S508 = apply_aerial_0(_S454, col_2, p_18, _S455, kernelContext_50);
    if(((uint4((&_S454->light_0)->flags_0) ).w) == 1U)
    {
        uint _S509 = shadow_cascade_0(_S454, p_18, zlin_2);
        col_2 = TINT_0[_S509] * float3((0.039999999105930328f + 0.5f * _S503)) ;
    }
    else
    {
        if(((uint4((&_S454->light_0)->flags_0) ).w) == 3U)
        {
            uint _S510 = shadow_cascade_0(_S454, p_18, zlin_2);
            if(_S510 < 4U)
            {
                float _S511 = cascade_depth_delta_0(_S454, kernelContext_50->ed_c_0->shadow_0, _S510, p_18, kernelContext_50);
                d_30 = _S511;
            }
            else
            {
                d_30 = 0.0f;
            }
            col_2 = float3(max(d_30, 0.0f), max(- d_30, 0.0f), 0.0f) * float3(0.10000000149011612f) ;
        }
        else
        {
            if(((uint4((&_S454->light_0)->flags_0) ).w) == 2U)
            {
                col_2 = float3((0.5f * ao_8)) ;
            }
            else
            {
                if(((uint4((&_S454->light_0)->flags_0) ).w) == 4U)
                {
                    col_2 = float3((0.5f * sv_5)) ;
                }
                else
                {
                    col_2 = _S508;
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
    thread KernelContext_0 kernelContext_51;
    (&kernelContext_51)->ed_c_0 = ed_c_1;
    (&kernelContext_51)->ed_buffers_0 = ed_buffers_1;
    (&kernelContext_51)->ed_textures_0 = ed_textures_1;
    (&kernelContext_51)->ed_samplers_0 = ed_samplers_1;
    thread FsOut_0 _S512;
    (&_S512)->pos_0 = pos_1;
    float4 _S513 = resolve_px_0(&_S512, false, &kernelContext_51);
    pixelOutput_0 _S514 = { _S513 };
    return _S514;
}

struct pixelOutput_1
{
    float4 output_1 [[color(0)]];
};

[[fragment]] pixelOutput_1 fs_resolve_terrain(float4 pos_2 [[position]], ResolveC_0 constant* ed_c_2 [[buffer(0)]], ulong device* ed_buffers_2 [[buffer(2)]], ulong device* ed_textures_2 [[buffer(1)]], ulong device* ed_samplers_2 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_52;
    (&kernelContext_52)->ed_c_0 = ed_c_2;
    (&kernelContext_52)->ed_buffers_0 = ed_buffers_2;
    (&kernelContext_52)->ed_textures_0 = ed_textures_2;
    (&kernelContext_52)->ed_samplers_0 = ed_samplers_2;
    thread FsOut_0 _S515;
    (&_S515)->pos_0 = pos_2;
    float4 _S516 = resolve_px_0(&_S515, true, &kernelContext_52);
    pixelOutput_1 _S517 = { _S516 };
    return _S517;
}

struct vs_resolve_Result_0
{
    float4 pos_3 [[position]];
};

[[vertex]] vs_resolve_Result_0 vs_resolve(uint vid_1 [[vertex_id]], ResolveC_0 constant* ed_c_3 [[buffer(0)]], ulong device* ed_buffers_3 [[buffer(2)]], ulong device* ed_textures_3 [[buffer(1)]], ulong device* ed_samplers_3 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_53;
    (&kernelContext_53)->ed_c_0 = ed_c_3;
    (&kernelContext_53)->ed_buffers_0 = ed_buffers_3;
    (&kernelContext_53)->ed_textures_0 = ed_textures_3;
    (&kernelContext_53)->ed_samplers_0 = ed_samplers_3;
    thread vs_resolve_Result_0 _S518;
    (&_S518)->pos_3 = fullscreen_vertex_0(vid_1).pos_0;
    return _S518;
}

