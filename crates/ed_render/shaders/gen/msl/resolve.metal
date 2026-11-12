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

uint device* ed_buf_3(uint i_3, KernelContext_0 thread* kernelContext_3)
{
    return (uint device*)kernelContext_3->ed_buffers_0[i_3];
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

GpuMesh_natural_0 device* ed_buf_4(uint i_4, KernelContext_0 thread* kernelContext_4)
{
    return (GpuMesh_natural_0 device*)kernelContext_4->ed_buffers_0[i_4];
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

GpuInstance_natural_0 device* ed_buf_5(uint i_5, KernelContext_0 thread* kernelContext_5)
{
    return (GpuInstance_natural_0 device*)kernelContext_5->ed_buffers_0[i_5];
}

packed_float4 device* ed_buf_6(uint i_6, KernelContext_0 thread* kernelContext_6)
{
    return (packed_float4 device*)kernelContext_6->ed_buffers_0[i_6];
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

float3 atm_transmittance_0(FrameData_natural_0 device* f_10, float h_7, float mu_7, KernelContext_0 thread* kernelContext_8)
{
    float _S15 = atm_rb_0(f_10);
    float _S16 = atm_rt_0(f_10);
    float2 uv_3 = atm_trans_uv_0(_S15, _S16, h_7, mu_7);
    packed_float4 device* _S17 = ed_buf_6((uint4((&f_10->atmos_0)->bufs_0) ).x, kernelContext_8);
    float4 _S18 = atm_bilinear_0(_S17, 256U, 0U, 64U, uv_3);
    return _S18.xyz;
}

float3 atm_transmittance_1(FrameData_natural_0 device* f_11, float h_8, float mu_8, KernelContext_0 thread* kernelContext_9)
{
    float _S19 = atm_rb_0(f_11);
    float _S20 = atm_rt_0(f_11);
    float2 uv_4 = atm_trans_uv_0(_S19, _S20, h_8, mu_8);
    packed_float4 device* _S21 = ed_buf_6((uint4((&f_11->atmos_0)->bufs_0) ).x, kernelContext_9);
    float4 _S22 = atm_bilinear_0(_S21, 256U, 0U, 64U, uv_4);
    return _S22.xyz;
}

float atm_cos_horizon_0(float rb_4, float h_9)
{
    return - sqrt(max(h_9 * (2.0f * rb_4 + h_9), 0.0f)) / (rb_4 + h_9);
}

float3 atm_sky_view_0(FrameData_natural_0 device* f_12, uint buf_1, uint w_1, uint rows_1, float h_10, float3 d_3, KernelContext_0 thread* kernelContext_10)
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
    packed_float4 device* _S27 = ed_buf_6(buf_1, kernelContext_10);
    float4 _S28 = atm_bilinear_0(_S27, w_1, row0_1, half_rows_0, float2(u_0, c_2));
    return _S28.xyz;
}

float3 atmos_sky_0(FrameData_natural_0 device* f_13, float3 d_4, KernelContext_0 thread* kernelContext_11)
{
    float3 _S29 = atm_sky_view_0(f_13, (uint4((&f_13->atmos_0)->bufs_0) ).z, 192U, 108U, (float4((&f_13->atmos_0)->radii_0) ).z, d_4, kernelContext_11);
    return _S29;
}

float3 atmos_sky_1(FrameData_natural_0 device* f_14, float3 d_5, KernelContext_0 thread* kernelContext_12)
{
    float3 _S30 = atm_sky_view_0(f_14, (uint4((&f_14->atmos_0)->bufs_0) ).z, 192U, 108U, (float4((&f_14->atmos_0)->radii_0) ).z, d_5, kernelContext_12);
    return _S30;
}

float3 atmos_sky_pixel_0(FrameData_natural_0 device* f_15, float3 d_6, float disc_k_2, KernelContext_0 thread* kernelContext_13)
{
    float3 _S31 = atmos_sky_0(f_15, d_6, kernelContext_13);
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
                float3 _S36 = atm_transmittance_0(f_15, h_11, mu_10, kernelContext_13);
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

float3 sky_pixel_0(FrameData_natural_0 device* f_16, float3 d_7, KernelContext_0 thread* kernelContext_14)
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
            float3 _S42 = atm_transmittance_0(f_16, h_12, mu_11, kernelContext_14);
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
    float3 _S44 = atmos_sky_pixel_0(f_16, d_7, _S43, kernelContext_14);
    return _S44;
}

float3 xform_point_0(const GpuInstance_natural_0 thread* i_8, float3 p_2)
{
    float4 _S45 = float4((&i_8->m_0)->data_0[int(0)]) ;
    float4 _S46 = float4((&i_8->m_0)->data_0[int(1)]) ;
    float4 _S47 = float4((&i_8->m_0)->data_0[int(2)]) ;
    return float3(dot(_S45.xyz, p_2) + _S45.w, dot(_S46.xyz, p_2) + _S46.w, dot(_S47.xyz, p_2) + _S47.w);
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

float2 unpack_snorm2x16_0(uint p_3)
{
    return clamp(float2(int2(int(p_3 << 16U) >> 16U, int(p_3) >> 16U)) / float2(32767.0f) , float2(-1.0f) , float2(1.0f) );
}

float3 oct_decode_0(uint packed_0)
{
    float2 e_0 = unpack_snorm2x16_0(packed_0);
    float _S48 = e_0.x;
    float _S49 = e_0.y;
    float _S50 = 1.0f - abs(_S48) - abs(_S49);
    thread float3 n_1 = float3(_S48, _S49, _S50);
    float t_3 = saturate(- _S50);
    float _S51;
    if(_S48 >= 0.0f)
    {
        _S51 = - t_3;
    }
    else
    {
        _S51 = t_3;
    }
    n_1.x = n_1.x + _S51;
    if((n_1.y) >= 0.0f)
    {
        _S51 = - t_3;
    }
    else
    {
        _S51 = t_3;
    }
    n_1.y = n_1.y + _S51;
    return normalize(n_1);
}

float3 xform_normal_0(const GpuInstance_natural_0 thread* i_9, float3 n_2)
{
    float4 _S52 = float4((&i_9->m_0)->data_0[int(0)]) ;
    float4 _S53 = float4((&i_9->m_0)->data_0[int(1)]) ;
    float4 _S54 = float4((&i_9->m_0)->data_0[int(2)]) ;
    float3 a0_0 = float3(_S52.x, _S53.x, _S54.x);
    float3 a1_0 = float3(_S52.y, _S53.y, _S54.y);
    float3 a2_0 = float3(_S52.z, _S53.z, _S54.z);
    return cross(a1_0, a2_0) * float3(n_2.x)  + cross(a2_0, a0_0) * float3(n_2.y)  + cross(a0_0, a1_0) * float3(n_2.z) ;
}

float2 uv_xform_0(const GpuMaterial_natural_0 thread* m_1, float2 uv_5)
{
    float r_3 = (as_type<float>(((uint4(m_1->tex_0) ).w)));
    float s_0 = sin(r_3);
    float c_4 = cos(r_3);
    float4 _S55 = float4(m_1->uv_0) ;
    float2 q_0 = uv_5 * _S55.zw;
    float _S56 = q_0.x;
    float _S57 = q_0.y;
    return float2(c_4 * _S56 + s_0 * _S57, - s_0 * _S56 + c_4 * _S57) + _S55.xy;
}

float length_squared_0(float3 v_0)
{
    return dot(v_0, v_0);
}

float cascade_extent_0(FrameData_natural_0 device* f_17, uint c_5, float3 p_4)
{
    float4 s_1 = (((float4(p_4, 1.0f)) * (matrix<float,int(4),int(4)> ((&(&f_17->light_0)->shadow_1)->data_3[c_5].data_1[int(0)][int(0)], (&(&f_17->light_0)->shadow_1)->data_3[c_5].data_1[int(1)][int(0)], (&(&f_17->light_0)->shadow_1)->data_3[c_5].data_1[int(2)][int(0)], (&(&f_17->light_0)->shadow_1)->data_3[c_5].data_1[int(3)][int(0)], (&(&f_17->light_0)->shadow_1)->data_3[c_5].data_1[int(0)][int(1)], (&(&f_17->light_0)->shadow_1)->data_3[c_5].data_1[int(1)][int(1)], (&(&f_17->light_0)->shadow_1)->data_3[c_5].data_1[int(2)][int(1)], (&(&f_17->light_0)->shadow_1)->data_3[c_5].data_1[int(3)][int(1)], (&(&f_17->light_0)->shadow_1)->data_3[c_5].data_1[int(0)][int(2)], (&(&f_17->light_0)->shadow_1)->data_3[c_5].data_1[int(1)][int(2)], (&(&f_17->light_0)->shadow_1)->data_3[c_5].data_1[int(2)][int(2)], (&(&f_17->light_0)->shadow_1)->data_3[c_5].data_1[int(3)][int(2)], (&(&f_17->light_0)->shadow_1)->data_3[c_5].data_1[int(0)][int(3)], (&(&f_17->light_0)->shadow_1)->data_3[c_5].data_1[int(1)][int(3)], (&(&f_17->light_0)->shadow_1)->data_3[c_5].data_1[int(2)][int(3)], (&(&f_17->light_0)->shadow_1)->data_3[c_5].data_1[int(3)][int(3)]))));
    float _S58 = s_1.z;
    bool _S59;
    if(_S58 < 0.0f)
    {
        _S59 = true;
    }
    else
    {
        _S59 = _S58 > 1.0f;
    }
    float _S60;
    if(_S59)
    {
        _S60 = 1.0e+09f;
    }
    else
    {
        _S60 = max(abs(s_1.x), abs(s_1.y));
    }
    return _S60;
}

float pcf_radius_texels_0(FrameData_natural_0 device* f_18, float texel_0)
{
    return clamp((float4((&f_18->light_0)->sky_0) ).z / texel_0, 1.0f, max((float4((&f_18->light_0)->sky_0) ).w, 1.0f));
}

float ed_depth_read_0(uint i_10, uint2 p_5, KernelContext_0 thread* kernelContext_15)
{
    float _S61 = ((reinterpret_cast<device depth2d<float, access::sample>*>((kernelContext_15->ed_textures_0))[(i_10)].read((p_5))));
    return _S61;
}

float cascade_lookup_0(FrameData_natural_0 device* f_19, uint atlas_0, uint c_6, float3 p_6, float3 ng_0, float2 px_1, KernelContext_0 thread* kernelContext_16)
{
    float4 _S62 = float4((&(&f_19->light_0)->cascade_0)->data_4[c_6]) ;
    float tile_0 = (float4((&f_19->light_0)->cfg_0) ).y;
    float _S63 = _S62.x;
    float _S64 = pcf_radius_texels_0(f_19, _S63);
    float sp_0 = _S64 * 0.5f;
    float nol_0 = saturate(dot(ng_0, (float4(f_19->sun_dir_0) ).xyz));
    float4 s_2 = (((float4(p_6 + ng_0 * float3((_S63 * ((float4((&f_19->light_0)->filter_0) ).z + 2.0f * sqrt(1.0f - nol_0 * nol_0)))) , 1.0f)) * (matrix<float,int(4),int(4)> ((&(&f_19->light_0)->shadow_1)->data_3[c_6].data_1[int(0)][int(0)], (&(&f_19->light_0)->shadow_1)->data_3[c_6].data_1[int(1)][int(0)], (&(&f_19->light_0)->shadow_1)->data_3[c_6].data_1[int(2)][int(0)], (&(&f_19->light_0)->shadow_1)->data_3[c_6].data_1[int(3)][int(0)], (&(&f_19->light_0)->shadow_1)->data_3[c_6].data_1[int(0)][int(1)], (&(&f_19->light_0)->shadow_1)->data_3[c_6].data_1[int(1)][int(1)], (&(&f_19->light_0)->shadow_1)->data_3[c_6].data_1[int(2)][int(1)], (&(&f_19->light_0)->shadow_1)->data_3[c_6].data_1[int(3)][int(1)], (&(&f_19->light_0)->shadow_1)->data_3[c_6].data_1[int(0)][int(2)], (&(&f_19->light_0)->shadow_1)->data_3[c_6].data_1[int(1)][int(2)], (&(&f_19->light_0)->shadow_1)->data_3[c_6].data_1[int(2)][int(2)], (&(&f_19->light_0)->shadow_1)->data_3[c_6].data_1[int(3)][int(2)], (&(&f_19->light_0)->shadow_1)->data_3[c_6].data_1[int(0)][int(3)], (&(&f_19->light_0)->shadow_1)->data_3[c_6].data_1[int(1)][int(3)], (&(&f_19->light_0)->shadow_1)->data_3[c_6].data_1[int(2)][int(3)], (&(&f_19->light_0)->shadow_1)->data_3[c_6].data_1[int(3)][int(3)]))));
    float2 _S65 = float2(s_2.x * 0.5f + 0.5f, 0.5f - s_2.y * 0.5f) * float2(tile_0) ;
    float ign_0 = fract(52.982917785644531f * fract(dot(px_1, float2(0.067110560834407806f, 0.0058371499180793762f))));
    float2 _S66 = float2(0.5f) ;
    float2 _S67 = (float2(ign_0, fract(ign_0 * 7.309999942779541f + 0.37000000476837158f)) - _S66) * float2(0.0f) ;
    float3 ns_0 = (((float4(ng_0, 0.0f)) * (matrix<float,int(4),int(4)> ((&(&f_19->light_0)->shadow_1)->data_3[c_6].data_1[int(0)][int(0)], (&(&f_19->light_0)->shadow_1)->data_3[c_6].data_1[int(1)][int(0)], (&(&f_19->light_0)->shadow_1)->data_3[c_6].data_1[int(2)][int(0)], (&(&f_19->light_0)->shadow_1)->data_3[c_6].data_1[int(3)][int(0)], (&(&f_19->light_0)->shadow_1)->data_3[c_6].data_1[int(0)][int(1)], (&(&f_19->light_0)->shadow_1)->data_3[c_6].data_1[int(1)][int(1)], (&(&f_19->light_0)->shadow_1)->data_3[c_6].data_1[int(2)][int(1)], (&(&f_19->light_0)->shadow_1)->data_3[c_6].data_1[int(3)][int(1)], (&(&f_19->light_0)->shadow_1)->data_3[c_6].data_1[int(0)][int(2)], (&(&f_19->light_0)->shadow_1)->data_3[c_6].data_1[int(1)][int(2)], (&(&f_19->light_0)->shadow_1)->data_3[c_6].data_1[int(2)][int(2)], (&(&f_19->light_0)->shadow_1)->data_3[c_6].data_1[int(3)][int(2)], (&(&f_19->light_0)->shadow_1)->data_3[c_6].data_1[int(0)][int(3)], (&(&f_19->light_0)->shadow_1)->data_3[c_6].data_1[int(1)][int(3)], (&(&f_19->light_0)->shadow_1)->data_3[c_6].data_1[int(2)][int(3)], (&(&f_19->light_0)->shadow_1)->data_3[c_6].data_1[int(3)][int(3)])))).xyz;
    float rl_0 = _S63 * tile_0 * 0.5f;
    float _S68 = _S62.y;
    float _S69 = ns_0.z * _S68;
    float izn_0 = 1.0f / max(abs(_S69), 0.11999999731779099f);
    float _S70 = float((int(sign((_S69)))));
    float gx_0 = clamp(- (ns_0.x * rl_0) * izn_0 * _S70, -8.0f, 8.0f);
    float gy_0 = clamp(- (ns_0.y * rl_0) * izn_0 * _S70, -8.0f, 8.0f);
    float _S71 = s_2.z + ((float4((&f_19->light_0)->filter_0) ).w + 0.5f * (abs(gx_0) + abs(gy_0))) * _S63 / _S68;
    float _S72 = _S63 / _S68;
    int2 _S73 = int2(_S62.zw);
    int2 _S74 = _S73 + int2((int(tile_0) - int(1))) ;
    float _S75 = 1.0f / (_S64 + sp_0);
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
        float _S76 = 1.0f - abs(oy_0) * _S75;
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
            float w_2 = (1.0f - abs(ox_0) * _S75) * _S76;
            float2 tp_0 = floor(_S65 + float2(ox_0, oy_0) + _S67);
            float2 d_8 = tp_0 + _S66 - _S65;
            float _S77 = ed_depth_read_0(atlas_0, uint2(clamp(_S73 + int2(tp_0), _S73, _S74)), kernelContext_16);
            float _S78;
            if((_S71 + (gx_0 * d_8.x - gy_0 * d_8.y) * _S72) >= _S77)
            {
                _S78 = 1.0f;
            }
            else
            {
                _S78 = 0.0f;
            }
            float lit_1 = lit_0 + _S78 * w_2;
            float wsum_1 = wsum_0 + w_2;
            x_0 = x_0 + int(1);
            lit_0 = lit_1;
            wsum_0 = wsum_1;
        }
        y_0 = y_0 + int(1);
    }
    return lit_0 / wsum_0;
}

float sun_shadow_0(FrameData_natural_0 device* f_20, uint atlas_1, float3 p_7, float3 ng_1, float dist_0, float2 px_2, KernelContext_0 thread* kernelContext_17)
{
    uint n_3 = uint((float4((&f_20->light_0)->cfg_0) ).x);
    bool _S79;
    if(((uint4((&f_20->light_0)->flags_0) ).x) == 0U)
    {
        _S79 = true;
    }
    else
    {
        _S79 = n_3 == 0U;
    }
    if(_S79)
    {
        _S79 = true;
    }
    else
    {
        _S79 = dist_0 >= ((float4((&f_20->light_0)->filter_0) ).y);
    }
    if(_S79)
    {
        return 1.0f;
    }
    float _S80 = 1.0f - (float4((&f_20->light_0)->cfg_0) ).w * 2.0f / (float4((&f_20->light_0)->cfg_0) ).y;
    float _S81 = (float4((&f_20->light_0)->cfg_0) ).z;
    uint c_7 = 0U;
    for(;;)
    {
        if(c_7 < n_3)
        {
        }
        else
        {
            break;
        }
        float _S82 = cascade_extent_0(f_20, c_7, p_7);
        float e_1 = _S82 / _S80;
        if(e_1 > 1.0f)
        {
            c_7 = c_7 + 1U;
            continue;
        }
        float _S83 = cascade_lookup_0(f_20, atlas_1, c_7, p_7, ng_1, px_2, kernelContext_17);
        float blend_0 = smoothstep(1.0f - _S81, 1.0f, e_1);
        float v_1;
        if(blend_0 > 0.0f)
        {
            uint _S84 = c_7 + 1U;
            if(_S84 < n_3)
            {
                float _S85 = cascade_extent_0(f_20, _S84, p_7);
                _S79 = (_S85 / _S80) <= 1.0f;
            }
            else
            {
                _S79 = false;
            }
            if(_S79)
            {
                float _S86 = cascade_lookup_0(f_20, atlas_1, _S84, p_7, ng_1, px_2, kernelContext_17);
                v_1 = _S86;
            }
            else
            {
                v_1 = 1.0f;
            }
            v_1 = mix(_S83, v_1, blend_0);
        }
        else
        {
            v_1 = _S83;
        }
        return mix(v_1, 1.0f, smoothstep(0.0f, 1.0f, saturate((dist_0 - (float4((&f_20->light_0)->filter_0) ).x) / max((float4((&f_20->light_0)->filter_0) ).y - (float4((&f_20->light_0)->filter_0) ).x, 0.0010000000474974513f))));
    }
    return 1.0f;
}

float2 ao_upsample_0(FrameData_natural_0 device* f_21, uint ao_buf_0, float2 pixel_1, float zlin_0, KernelContext_0 thread* kernelContext_18)
{
    float sv_0;
    if(((uint4((&f_21->light_0)->flags_0) ).y) == 0U)
    {
        return float2(1.0f) ;
    }
    packed_float4 device* _S87 = ed_buf_6(ao_buf_0, kernelContext_18);
    uint _S88 = uint((float4((&f_21->light_0)->ao_1) ).y);
    uint _S89 = uint((float4((&f_21->light_0)->ao_1) ).z);
    float2 _S90 = float2(0.5f) ;
    float2 h_13 = (pixel_1 - _S90) * _S90;
    float2 b_1 = floor(h_13);
    float2 _S91 = h_13 - b_1;
    float2 _S92 = float2(0.0f) ;
    int y_1 = int(0);
    float2 sum_0 = _S92;
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
            int2 t_4 = clamp(int2(b_1) + int2(x_1, y_1), int2(int(0)) , int2(int(_S88) - int(1), int(_S89) - int(1)));
            float4 _S93 = float4(*(_S87 + (uint(t_4.y) * _S88 + uint(t_4.x)))) ;
            if(x_1 == int(0))
            {
                sv_0 = 1.0f - _S91.x;
            }
            else
            {
                sv_0 = _S91.x;
            }
            float _S94;
            if(y_1 == int(0))
            {
                _S94 = 1.0f - _S91.y;
            }
            else
            {
                _S94 = _S91.y;
            }
            float w_3 = max(sv_0 * _S94, 0.0010000000474974513f) * (1.0f / (0.0010000000474974513f + abs(_S93.z - zlin_0) / max(zlin_0, 0.0010000000474974513f) * 50.0f));
            float2 sum_1 = sum_0 + _S93.xy * float2(w_3) ;
            float wsum_3 = wsum_2 + w_3;
            x_1 = x_1 + int(1);
            sum_0 = sum_1;
            wsum_2 = wsum_3;
        }
        y_1 = y_1 + int(1);
    }
    float2 r_4;
    if(wsum_2 > 0.0f)
    {
        r_4 = sum_0 / float2(wsum_2) ;
    }
    else
    {
        r_4 = float2(1.0f) ;
    }
    if(((float4((&f_21->light_0)->skyvis_0) ).x) > 0.0f)
    {
        sv_0 = saturate(r_4.y);
    }
    else
    {
        sv_0 = 1.0f;
    }
    return float2(pow(saturate(r_4.x), (float4((&f_21->light_0)->ao_1) ).x), sv_0);
}

float atm_height_at_0(float rb_5, float h_14, float mu_12, float t_5)
{
    float q_1 = t_5 * t_5 + 2.0f * (rb_5 + h_14) * mu_12 * t_5 + h_14 * (2.0f * rb_5 + h_14);
    return q_1 / (sqrt(max(rb_5 * rb_5 + q_1, 0.0f)) + rb_5);
}

float atm_earth_shadow_0(FrameData_natural_0 device* f_22, float h_15, float mu_s_4)
{
    float _S95 = atm_rb_1(f_22);
    float ch_0 = atm_cos_horizon_0(_S95, h_15);
    float s_3 = 0.0046999999321997166f * sqrt(max(1.0f - ch_0 * ch_0, 0.0f)) + 9.9999997473787516e-06f;
    return smoothstep(- s_3, s_3, mu_s_4 - ch_0);
}

float3 atm_sun_transmittance_0(FrameData_natural_0 device* f_23, float h_16, float mu_s_5, KernelContext_0 thread* kernelContext_19)
{
    float3 _S96 = atm_transmittance_1(f_23, h_16, mu_s_5, kernelContext_19);
    float _S97 = atm_earth_shadow_0(f_23, h_16, mu_s_5);
    return _S96 * float3(_S97) ;
}

float3 atmos_sun_at_0(FrameData_natural_0 device* f_24, float3 p_8, KernelContext_0 thread* kernelContext_20)
{
    float t_6 = length(p_8) * 0.0010000000474974513f;
    float3 d_9;
    if(t_6 > 0.0f)
    {
        d_9 = p_8 * float3((0.0010000000474974513f / t_6)) ;
    }
    else
    {
        d_9 = (float4((&f_24->atmos_0)->up_0) ).xyz;
    }
    float _S98 = atm_rb_0(f_24);
    float h0_0 = (float4((&f_24->atmos_0)->radii_0) ).z;
    float h_17 = atm_height_at_0(_S98, h0_0, dot(d_9, (float4((&f_24->atmos_0)->up_0) ).xyz), t_6);
    float3 _S99 = (float4((&f_24->atmos_0)->sun_0) ).xyz;
    float3 _S100 = atm_sun_transmittance_0(f_24, h_17, clamp(((_S98 + h0_0) * (float4((&f_24->atmos_0)->up_0) ).w + t_6 * dot(d_9, (float4(f_24->sun_dir_0) ).xyz)) / (_S98 + h_17), -1.0f, 1.0f), kernelContext_20);
    return _S99 * _S100;
}

float3 sun_light_at_0(FrameData_natural_0 device* f_25, float3 p_9, KernelContext_0 thread* kernelContext_21)
{
    bool _S101 = atmos_on_0(f_25);
    float3 _S102;
    if(_S101)
    {
        float3 _S103 = atmos_sun_at_0(f_25, p_9, kernelContext_21);
        _S102 = _S103;
    }
    else
    {
        _S102 = (float4(f_25->sun_color_0) ).xyz;
    }
    return _S102;
}

float3 sh_irradiance_0(FrameData_natural_0 device* f_26, float3 n_4, KernelContext_0 thread* kernelContext_22)
{
    thread array<float4, int(9)> s_4;
    packed_float4 device* _S104 = ed_buf_6((uint4((&f_26->atmos_0)->bufs2_0) ).z, kernelContext_22);
    uint k_0 = 0U;
    for(;;)
    {
        if(k_0 < 9U)
        {
        }
        else
        {
            break;
        }
        bool _S105 = atmos_on_0(f_26);
        float4 _S106;
        if(_S105)
        {
            _S106 = float4(*(_S104 + (288U + k_0))) ;
        }
        else
        {
            _S106 = float4((&(&f_26->light_0)->sh_0)->data_5[k_0]) ;
        }
        s_4[k_0] = _S106;
        k_0 = k_0 + 1U;
    }
    float _S107 = n_4.y;
    float _S108 = n_4.z;
    float _S109 = n_4.x;
    return max(s_4[int(0)].xyz + (s_4[int(1)].xyz * float3(_S107)  + s_4[int(2)].xyz * float3(_S108)  + s_4[int(3)].xyz * float3(_S109) ) + (s_4[int(4)].xyz * float3((_S109 * _S107))  + s_4[int(5)].xyz * float3((_S107 * _S108))  + s_4[int(6)].xyz * float3((3.0f * _S108 * _S108 - 1.0f)) ) + (s_4[int(7)].xyz * float3((_S109 * _S108))  + s_4[int(8)].xyz * float3((_S109 * _S109 - _S107 * _S107)) ), float3(0.0f) );
}

float3 hemi_ambient_0(FrameData_natural_0 device* f_27, float3 n_5)
{
    return mix((float4(f_27->ground_color_0) ).xyz * float3((float4(f_27->sun_dir_0) ).w) , mix((float4(f_27->sky_horizon_0) ).xyz, (float4(f_27->sky_zenith_0) ).xyz, float3(0.5f) ) * float3((float4(f_27->sun_dir_0) ).w) , float3((n_5.y * 0.5f + 0.5f)) );
}

float3 ambient_light_0(FrameData_natural_0 device* f_28, float3 n_6, KernelContext_0 thread* kernelContext_23)
{
    float3 _S110;
    if(((uint4((&f_28->light_0)->flags_0) ).z) != 0U)
    {
        float3 _S111 = sh_irradiance_0(f_28, n_6, kernelContext_23);
        _S110 = _S111;
    }
    else
    {
        float3 _S112 = hemi_ambient_0(f_28, n_6);
        _S110 = _S112;
    }
    return _S110;
}

float3 occluded_ambient_0(FrameData_natural_0 device* f_29, float3 open_0, float sv_1, KernelContext_0 thread* kernelContext_24)
{
    float3 _S113 = ambient_light_0(f_29, float3(0.0f, -1.0f, 0.0f), kernelContext_24);
    return mix(_S113 * float3((float4((&f_29->light_0)->skyvis_0) ).y) , open_0, float3(sv_1) );
}

float3 ao_multibounce_0(FrameData_natural_0 device* f_30, float ao_2, float3 albedo_0)
{
    float3 _S114 = float3(ao_2) ;
    return mix(_S114, max(_S114, ((_S114 * (float3(2.0404000282287598f)  * albedo_0 - float3(0.33239999413490295f) ) + (float3(-4.795100212097168f)  * albedo_0 + float3(0.64170002937316895f) )) * _S114 + (float3(2.7551999092102051f)  * albedo_0 + float3(0.69029998779296875f) )) * _S114), float3((float4((&f_30->light_0)->sky_0) ).y) );
}

float3 shade_toon_0(FrameData_natural_0 device* f_31, float3 sun_3, const GpuMaterial_natural_0 thread* m_2, float3 albedo_1, float3 n_7, float3 v_2, float ao_3, float sv_2, float sun_vis_0, KernelContext_0 thread* kernelContext_25)
{
    float ndl_0 = dot(n_7, (float4(f_31->sun_dir_0) ).xyz);
    float _S115 = max((float4(m_2->params_0) ).z, 1.0f);
    float lit_2 = floor(saturate(ndl_0 * 0.5f + 0.5f) * _S115 + 0.5f) / _S115 * sun_vis_0;
    float4 _S116 = float4(m_2->shade_0) ;
    float3 c_8 = mix(albedo_1 * _S116.xyz, albedo_1, float3(lit_2) );
    float3 _S117 = sun_3 * float3((lit_2 / 3.1415927410125732f)) ;
    float3 _S118 = ambient_light_0(f_31, n_7, kernelContext_25);
    float3 _S119 = occluded_ambient_0(f_31, _S118, sv_2, kernelContext_25);
    float3 _S120 = ao_multibounce_0(f_31, ao_3, albedo_1);
    return c_8 * (_S117 + _S119 * _S120) + float3((pow(1.0f - saturate(dot(n_7, v_2)), 4.0f) * _S116.w * saturate(ndl_0 + 0.30000001192092896f) * sun_vis_0))  * sun_3 * float3(0.15000000596046448f) ;
}

float3 f_schlick_0(float3 f0_0, float voh_0)
{
    return f0_0 + (float3(1.0f)  - f0_0) * float3(pow(1.0f - voh_0, 5.0f)) ;
}

float d_ggx_0(float noh_0, float a_0)
{
    float a2_1 = a_0 * a_0;
    float d_10 = noh_0 * noh_0 * (a2_1 - 1.0f) + 1.0f;
    return a2_1 / (3.1415927410125732f * d_10 * d_10);
}

float v_smith_0(float nov_0, float nol_1, float a_1)
{
    float a2_2 = a_1 * a_1;
    float _S121 = 1.0f - a2_2;
    return 0.5f / max(nol_1 * sqrt(nov_0 * nov_0 * _S121 + a2_2) + nov_0 * sqrt(nol_1 * nol_1 * _S121 + a2_2), 9.9999997473787516e-06f);
}

float2 env_brdf_0(float nov_1, float rough_0)
{
    float4 r_5 = float4(rough_0)  * float4(-1.0f, -0.027499999850988388f, -0.57200002670288086f, 0.02199999988079071f) + float4(1.0f, 0.042500000447034836f, 1.0399999618530273f, -0.039999999105930328f);
    float _S122 = r_5.x;
    return float2(-1.0399999618530273f, 1.0399999618530273f) * float2((min(_S122 * _S122, exp2(-9.2799997329711914f * nov_1)) * _S122 + r_5.y))  + r_5.zw;
}

float3 sky_env_0(FrameData_natural_0 device* f_32, float3 d_11, KernelContext_0 thread* kernelContext_26)
{
    bool _S123 = atmos_on_1(f_32);
    float3 _S124;
    if(_S123)
    {
        float3 _S125 = atmos_sky_1(f_32, d_11, kernelContext_26);
        _S124 = _S125;
    }
    else
    {
        float3 _S126 = sky_rad_1(f_32, d_11, 0.0f);
        _S124 = _S126;
    }
    return _S124;
}

float3 shade_pbr_0(FrameData_natural_0 device* f_33, float3 sun_4, float3 albedo_2, float metal_0, float rough_1, float ao_4, float sv_3, float sun_vis_1, float3 n_8, float3 v_3, KernelContext_0 thread* kernelContext_27)
{
    float _S127 = saturate(metal_0);
    float _S128 = clamp(rough_1, 0.039999999105930328f, 1.0f);
    float a_2 = _S128 * _S128;
    float3 l_0 = (float4(f_33->sun_dir_0) ).xyz;
    float3 h_18 = normalize(l_0 + v_3);
    float nol_2 = saturate(dot(n_8, l_0));
    float _S129 = max(dot(n_8, v_3), 9.9999997473787516e-05f);
    float3 f0_1 = mix(float3(0.039999999105930328f) , albedo_2, float3(_S127) );
    float3 diff_col_0 = albedo_2 * float3((1.0f - _S127)) ;
    float3 F_0 = f_schlick_0(f0_1, saturate(dot(v_3, h_18)));
    float3 _S130 = float3(1.0f) ;
    float3 direct_0 = (diff_col_0 * (_S130 - F_0) / float3(3.1415927410125732f)  + float3((d_ggx_0(saturate(dot(n_8, h_18)), a_2) * v_smith_0(_S129, nol_2, a_2)))  * F_0) * sun_4 * float3((nol_2 * sun_vis_1)) ;
    float2 ab_0 = env_brdf_0(_S129, _S128);
    float3 r_6 = reflect(- v_3, n_8);
    float3 env_0;
    if(((uint4((&f_33->light_0)->flags_0) ).z) != 0U)
    {
        float3 _S131 = sky_env_0(f_33, r_6, kernelContext_27);
        float3 _S132 = sh_irradiance_0(f_33, r_6, kernelContext_27);
        env_0 = mix(_S131, _S132, float3(_S128) );
    }
    else
    {
        float3 _S133 = hemi_ambient_0(f_33, r_6);
        env_0 = _S133;
    }
    float3 _S134 = occluded_ambient_0(f_33, env_0, sv_3, kernelContext_27);
    float spec_occ_0 = saturate(pow(_S129 + ao_4, exp2(-16.0f * a_2 - 1.0f)) - 1.0f + ao_4);
    float3 f_env_0 = f0_1 * float3(ab_0.x)  + float3(ab_0.y) ;
    float3 _S135 = diff_col_0 * (_S130 - f_env_0);
    float3 _S136 = ambient_light_0(f_33, n_8, kernelContext_27);
    float3 _S137 = occluded_ambient_0(f_33, _S136, sv_3, kernelContext_27);
    float3 _S138 = _S135 * _S137;
    float3 _S139 = ao_multibounce_0(f_33, ao_4, diff_col_0);
    return direct_0 + (_S138 * _S139 + _S134 * f_env_0 * float3(spec_occ_0) );
}

float atm_ap_slice_0(FrameData_natural_0 device* f_34, float t_7)
{
    float n_9 = (float4((&f_34->atmos_0)->ap_0) ).x;
    return log2(1.0f + max(t_7, 0.0f) / n_9) / log2(1.0f + (float4((&f_34->atmos_0)->ap_0) ).y / n_9) * 32.0f;
}

void atm_froxel_xy_0(packed_float4 device* l_1, packed_float4 device* t_8, float2 g_1, uint2 i_11, uint k_1, float3 thread* lo_0, float3 thread* to_0)
{
    uint b_2 = (k_1 * 32U + i_11.y) * 32U + i_11.x;
    uint _S140 = b_2 + 1U;
    float4 _S141 = float4(g_1.x) ;
    uint _S142 = b_2 + 32U;
    uint _S143 = _S142 + 1U;
    float4 t0_0 = mix(float4(*(t_8 + b_2)) , float4(*(t_8 + _S140)) , _S141);
    float4 t1_0 = mix(float4(*(t_8 + _S142)) , float4(*(t_8 + _S143)) , _S141);
    float4 _S144 = float4(g_1.y) ;
    *lo_0 = mix(mix(float4(*(l_1 + b_2)) , float4(*(l_1 + _S140)) , _S141), mix(float4(*(l_1 + _S142)) , float4(*(l_1 + _S143)) , _S141), _S144).xyz;
    *to_0 = mix(t0_0, t1_0, _S144).xyz;
    return;
}

void atmos_aerial_0(FrameData_natural_0 device* f_35, float3 p_10, float2 px_3, float3 thread* l_2, float3 thread* tr_0, KernelContext_0 thread* kernelContext_28)
{
    *l_2 = float3(0.0f) ;
    float3 _S145 = float3(1.0f) ;
    *tr_0 = _S145;
    float t_9 = length(p_10) * 0.0010000000474974513f;
    if(t_9 <= 0.0f)
    {
        return;
    }
    float h_19 = (float4((&f_35->atmos_0)->radii_0) ).z;
    float _S146 = atm_rb_0(f_35);
    float _S147 = atm_rt_0(f_35);
    float te_3;
    if(h_19 > (_S147 - _S146))
    {
        float te_4 = atm_enter_top_0(_S146, _S147, h_19, dot(p_10 * float3((0.0010000000474974513f / t_9)) , (float4((&f_35->atmos_0)->up_0) ).xyz));
        bool _S148;
        if(te_4 < 0.0f)
        {
            _S148 = true;
        }
        else
        {
            _S148 = t_9 <= te_4;
        }
        if(_S148)
        {
            return;
        }
        te_3 = te_4;
    }
    else
    {
        te_3 = 0.0f;
    }
    float _S149 = atm_ap_slice_0(f_35, t_9 - te_3);
    float2 q_2 = clamp(px_3 / (float4(f_35->viewport_0) ).zw * float2(32.0f)  - float2(0.5f) , float2(0.0f) , float2(31.0f) );
    uint2 _S150 = min(uint2(q_2), uint2(30U) );
    float2 g_2 = q_2 - float2(_S150);
    packed_float4 device* _S151 = ed_buf_6((uint4((&f_35->atmos_0)->bufs2_0) ).x, kernelContext_28);
    packed_float4 device* _S152 = ed_buf_6((uint4((&f_35->atmos_0)->bufs2_0) ).y, kernelContext_28);
    thread float3 la_0;
    thread float3 ta_0;
    thread float3 lc_0;
    thread float3 tc_0;
    if(_S149 < 1.0f)
    {
        atm_froxel_xy_0(_S151, _S152, g_2, _S150, 0U, &lc_0, &tc_0);
        float3 _S153 = float3(_S149) ;
        *l_2 = lc_0 * _S153;
        *tr_0 = mix(_S145, tc_0, _S153);
        return;
    }
    float _S154 = min(_S149 - 1.0f, 31.0f);
    uint _S155 = min(uint(_S154), 30U);
    float gz_0 = _S154 - float(_S155);
    atm_froxel_xy_0(_S151, _S152, g_2, _S150, _S155, &la_0, &ta_0);
    atm_froxel_xy_0(_S151, _S152, g_2, _S150, _S155 + 1U, &lc_0, &tc_0);
    float3 _S156 = float3(gz_0) ;
    *l_2 = mix(la_0, lc_0, _S156);
    *tr_0 = mix(ta_0, tc_0, _S156);
    return;
}

float3 apply_aerial_0(FrameData_natural_0 device* f_36, float3 col_0, float3 p_11, float2 px_4, KernelContext_0 thread* kernelContext_29)
{
    bool _S157 = atmos_on_0(f_36);
    if(!_S157)
    {
        return col_0;
    }
    thread float3 l_3;
    thread float3 tr_1;
    atmos_aerial_0(f_36, p_11, px_4, &l_3, &tr_1, kernelContext_29);
    if(((uint4((&f_36->atmos_0)->cfg_1) ).y) == 1U)
    {
        return tr_1 * float3(0.5f) ;
    }
    if(((uint4((&f_36->atmos_0)->cfg_1) ).y) == 2U)
    {
        return l_3;
    }
    return col_0 * tr_1 + l_3;
}

uint shadow_cascade_0(FrameData_natural_0 device* f_37, float3 p_12, float dist_1)
{
    uint _S158 = uint((float4((&f_37->light_0)->cfg_0) ).x);
    bool _S159;
    if(((uint4((&f_37->light_0)->flags_0) ).x) == 0U)
    {
        _S159 = true;
    }
    else
    {
        _S159 = dist_1 >= ((float4((&f_37->light_0)->filter_0) ).y);
    }
    if(_S159)
    {
        return 4U;
    }
    float _S160 = 1.0f - (float4((&f_37->light_0)->cfg_0) ).w * 2.0f / (float4((&f_37->light_0)->cfg_0) ).y;
    uint c_9 = 0U;
    for(;;)
    {
        if(c_9 < _S158)
        {
        }
        else
        {
            break;
        }
        float _S161 = cascade_extent_0(f_37, c_9, p_12);
        if((_S161 / _S160) <= 1.0f)
        {
            return c_9;
        }
        c_9 = c_9 + 1U;
    }
    return 4U;
}

float cascade_depth_delta_0(FrameData_natural_0 device* f_38, uint atlas_2, uint c_10, float3 p_13, KernelContext_0 thread* kernelContext_30)
{
    float4 _S162 = float4((&(&f_38->light_0)->cascade_0)->data_4[c_10]) ;
    float tile_1 = (float4((&f_38->light_0)->cfg_0) ).y;
    float4 s_5 = (((float4(p_13, 1.0f)) * (matrix<float,int(4),int(4)> ((&(&f_38->light_0)->shadow_1)->data_3[c_10].data_1[int(0)][int(0)], (&(&f_38->light_0)->shadow_1)->data_3[c_10].data_1[int(1)][int(0)], (&(&f_38->light_0)->shadow_1)->data_3[c_10].data_1[int(2)][int(0)], (&(&f_38->light_0)->shadow_1)->data_3[c_10].data_1[int(3)][int(0)], (&(&f_38->light_0)->shadow_1)->data_3[c_10].data_1[int(0)][int(1)], (&(&f_38->light_0)->shadow_1)->data_3[c_10].data_1[int(1)][int(1)], (&(&f_38->light_0)->shadow_1)->data_3[c_10].data_1[int(2)][int(1)], (&(&f_38->light_0)->shadow_1)->data_3[c_10].data_1[int(3)][int(1)], (&(&f_38->light_0)->shadow_1)->data_3[c_10].data_1[int(0)][int(2)], (&(&f_38->light_0)->shadow_1)->data_3[c_10].data_1[int(1)][int(2)], (&(&f_38->light_0)->shadow_1)->data_3[c_10].data_1[int(2)][int(2)], (&(&f_38->light_0)->shadow_1)->data_3[c_10].data_1[int(3)][int(2)], (&(&f_38->light_0)->shadow_1)->data_3[c_10].data_1[int(0)][int(3)], (&(&f_38->light_0)->shadow_1)->data_3[c_10].data_1[int(1)][int(3)], (&(&f_38->light_0)->shadow_1)->data_3[c_10].data_1[int(2)][int(3)], (&(&f_38->light_0)->shadow_1)->data_3[c_10].data_1[int(3)][int(3)]))));
    int2 _S163 = int2(_S162.zw);
    float _S164 = s_5.z;
    float _S165 = ed_depth_read_0(atlas_2, uint2(clamp(_S163 + int2(floor(float2(s_5.x * 0.5f + 0.5f, 0.5f - s_5.y * 0.5f) * float2(tile_1) )), _S163, _S163 + int2((int(tile_1) - int(1))) )), kernelContext_30);
    return (_S164 - _S165) * _S162.y;
}

struct pixelOutput_0
{
    float4 output_0 [[color(0)]];
};

[[fragment]] pixelOutput_0 fs_resolve(float4 pos_1 [[position]], ResolveC_0 constant* ed_c_1 [[buffer(0)]], ulong device* ed_buffers_1 [[buffer(2)]], ulong device* ed_textures_1 [[buffer(1)]], ulong device* ed_samplers_1 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_31;
    (&kernelContext_31)->ed_c_0 = ed_c_1;
    (&kernelContext_31)->ed_buffers_0 = ed_buffers_1;
    (&kernelContext_31)->ed_textures_0 = ed_textures_1;
    (&kernelContext_31)->ed_samplers_0 = ed_samplers_1;
    FrameData_natural_0 device* _S166 = ed_buf_7(ed_c_1->frame_0, &kernelContext_31);
    float2 _S167 = pos_1.xy;
    uint2 _S168 = uint2(_S167);
    texture2d<uint, access::sample> _S169 = ((reinterpret_cast<device texture2d<uint, access::sample>*>(((&kernelContext_31)->ed_textures_0))[((&kernelContext_31)->ed_c_0->vis_0)]));
    int3 _S170 = int3(int2(_S168), int(0));
    uint2 vis_1 = ((_S169).read(vec<uint,2>(((_S170)).xy), uint(((_S170)).z))).xy;
    float3 _S171 = view_dir_0(_S166, _S167);
    uint _S172 = vis_1.x;
    if(_S172 == 0U)
    {
        float3 _S173 = sky_pixel_0(_S166, _S171, &kernelContext_31);
        pixelOutput_0 _S174 = { float4(_S173, 1.0f) };
        return _S174;
    }
    GpuInstance_natural_0 device* _S175 = ed_buf_5((&kernelContext_31)->ed_c_0->instances_0, &kernelContext_31);
    thread GpuInstance_natural_0 _S176 = *(_S175 + (_S172 - 1U));
    GpuMesh_natural_0 device* _S177 = ed_buf_4((&kernelContext_31)->ed_c_0->meshes_0, &kernelContext_31);
    GpuMesh_natural_0 device* _S178 = _S177 + (&_S176)->mesh_0;
    uint _S179 = vis_1.y;
    uint lod_0 = _S179 >> 28U;
    uint prim_0 = (_S179 & 268435455U) % max(_S178->index_count_0[lod_0] / 3U, 1U);
    uint device* _S180 = ed_buf_3((&kernelContext_31)->ed_c_0->indices_0, &kernelContext_31);
    uint device* idx_0 = _S180 + _S178->first_index_0[lod_0] + prim_0 * 3U;
    Vtx_0 device* _S181 = ed_buf_2((&kernelContext_31)->ed_c_0->verts_0, &kernelContext_31);
    int i0_0 = int(*(idx_0 + int(0))) + _S178->base_vertex_0[lod_0];
    int i1_0 = int(*(idx_0 + int(1))) + _S178->base_vertex_0[lod_0];
    int i2_0 = int(*(idx_0 + int(2))) + _S178->base_vertex_0[lod_0];
    Vtx_0 v0_0 = *(_S181 + i0_0);
    Vtx_0 v1_0 = *(_S181 + i1_0);
    Vtx_0 v2_0 = *(_S181 + i2_0);
    packed_float4 device* _S182 = ed_buf_6((&kernelContext_31)->ed_c_0->nodes_0, &kernelContext_31);
    float3 node_0 = (float4(*(_S182 + (((&_S176)->node_flags_0) & 65535U))) ).xyz;
    float3 _S183 = xform_point_0(&_S176, float3(v0_0.px_0, v0_0.py_0, v0_0.pz_0));
    float3 _S184 = xform_point_0(&_S176, float3(v1_0.px_0, v1_0.py_0, v1_0.pz_0));
    float3 _S185 = xform_point_0(&_S176, float3(v2_0.px_0, v2_0.py_0, v2_0.pz_0));
    float3 p0_1 = node_0 + _S183;
    float3 e1_1 = _S184 - _S183;
    float3 e2_1 = _S185 - _S183;
    float2 bb_0 = ray_bary_0(_S171, p0_1, e1_1, e2_1);
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
    float3 _S186 = float3((1.0f - b1_1 - b2_1)) ;
    float3 local_0 = _S183 * _S186 + _S184 * float3(b1_1)  + _S185 * float3(b2_1) ;
    float3 _S187 = xform_normal_0(&_S176, oct_decode_0(v0_0.n_0) * _S186 + oct_decode_0(v1_0.n_0) * float3(b1_1)  + oct_decode_0(v2_0.n_0) * float3(b2_1) );
    float3 n_10 = normalize(_S187);
    float3 v_4 = - _S171;
    GpuMaterial_natural_0 device* _S188 = ed_buf_1((&kernelContext_31)->ed_c_0->materials_0, &kernelContext_31);
    thread GpuMaterial_natural_0 _S189 = *(_S188 + (&_S176)->material_0);
    float4 _S190 = float4((&_S189)->base_0) ;
    float3 albedo_3 = _S190.xyz;
    float4 _S191 = float4((&_S189)->params_0) ;
    float metal_1 = _S191.x;
    float rough_2 = _S191.y;
    uint4 _S192 = uint4((&_S189)->tex_0) ;
    uint _S193 = _S192.x;
    bool _S194 = _S193 != 4294967295U;
    bool textured_0;
    if(_S194)
    {
        textured_0 = true;
    }
    else
    {
        textured_0 = (_S192.y) != 4294967295U;
    }
    if(textured_0)
    {
        textured_0 = true;
    }
    else
    {
        textured_0 = (_S192.z) != 4294967295U;
    }
    float ao_5;
    float metal_2;
    float rough_3;
    float d_12;
    float3 albedo_4;
    float3 n_11;
    if(textured_0)
    {
        packed_float2 device* _S195 = ed_buf_0((&kernelContext_31)->ed_c_0->uvs_0, &kernelContext_31);
        float2 _S196 = float2(*(_S195 + i0_0)) ;
        float2 d1_0 = float2(*(_S195 + i1_0))  - _S196;
        float2 d2_0 = float2(*(_S195 + i2_0))  - _S196;
        float2 _S197 = uv_xform_0(&_S189, _S196 + d1_0 * float2(b1_1)  + d2_0 * float2(b2_1) );
        float3 _S198 = view_dir_0(_S166, _S167 + float2(1.0f, 0.0f));
        float2 bx_0 = ray_bary_0(_S198, p0_1, e1_1, e2_1);
        float3 _S199 = view_dir_0(_S166, _S167 + float2(0.0f, 1.0f));
        float2 by_0 = ray_bary_0(_S199, p0_1, e1_1, e2_1);
        float2 _S200 = uv_xform_0(&_S189, _S196 + d1_0 * float2(bx_0.x)  + d2_0 * float2(bx_0.y) );
        float2 ux_0 = _S200 - _S197;
        float2 _S201 = uv_xform_0(&_S189, _S196 + d1_0 * float2(by_0.x)  + d2_0 * float2(by_0.y) );
        float2 uy_0 = _S201 - _S197;
        sampler _S202 = ((reinterpret_cast<device sampler*>(((&kernelContext_31)->ed_samplers_0))[((&kernelContext_31)->ed_c_0->smp_0)]));
        if(_S194)
        {
            texture2d<float, access::sample> _S203 = ((reinterpret_cast<device texture2d<float, access::sample>*>(((&kernelContext_31)->ed_textures_0))[(_S193)]));
            albedo_4 = albedo_3 * ((_S203).sample((_S202), (_S197), gradient2d((ux_0), (uy_0)))).xyz;
        }
        else
        {
            albedo_4 = albedo_3;
        }
        uint _S204 = _S192.z;
        if(_S204 != 4294967295U)
        {
            texture2d<float, access::sample> _S205 = ((reinterpret_cast<device texture2d<float, access::sample>*>(((&kernelContext_31)->ed_textures_0))[(_S204)]));
            float3 orm_0 = ((_S205).sample((_S202), (_S197), gradient2d((ux_0), (uy_0)))).xyz;
            float rough_4 = rough_2 * orm_0.y;
            float metal_3 = metal_1 * orm_0.z;
            ao_5 = orm_0.x;
            metal_2 = metal_3;
            rough_3 = rough_4;
        }
        else
        {
            ao_5 = 1.0f;
            metal_2 = metal_1;
            rough_3 = rough_2;
        }
        uint _S206 = _S192.y;
        if(_S206 != 4294967295U)
        {
            float _S207 = d1_0.x;
            float _S208 = d2_0.y;
            float _S209 = d1_0.y;
            float _S210 = d2_0.x;
            float det_1 = _S207 * _S208 - _S209 * _S210;
            if((abs(det_1)) > 9.999999960041972e-13f)
            {
                float3 _S211 = float3((1.0f / det_1)) ;
                float3 tg_0 = (e1_1 * float3(_S208)  - e2_1 * float3(_S209) ) * _S211;
                float3 bt_0 = (e2_1 * float3(_S207)  - e1_1 * float3(_S210) ) * _S211;
                float3 tg_1 = tg_0 - n_10 * float3(dot(n_10, tg_0)) ;
                if((length_squared_0(tg_1)) > 9.99999968265522539e-21f)
                {
                    float3 tg_2 = normalize(tg_1);
                    float3 _S212 = cross(n_10, tg_2);
                    if((dot(_S212, bt_0)) < 0.0f)
                    {
                        d_12 = -1.0f;
                    }
                    else
                    {
                        d_12 = 1.0f;
                    }
                    float3 bn_0 = _S212 * float3(- d_12) ;
                    texture2d<float, access::sample> _S213 = ((reinterpret_cast<device texture2d<float, access::sample>*>(((&kernelContext_31)->ed_textures_0))[(_S206)]));
                    float2 xy_0 = ((_S213).sample((_S202), (_S197), gradient2d((ux_0), (uy_0)))).xy * float2(2.0f)  - float2(1.0f) ;
                    n_11 = normalize(tg_2 * float3(xy_0.x)  + bn_0 * float3(xy_0.y)  + n_10 * float3(sqrt(saturate(1.0f - dot(xy_0, xy_0)))) );
                }
                else
                {
                    n_11 = n_10;
                }
            }
            else
            {
                n_11 = n_10;
            }
        }
        else
        {
            n_11 = n_10;
        }
    }
    else
    {
        n_11 = n_10;
        albedo_4 = albedo_3;
        ao_5 = 1.0f;
        metal_2 = metal_1;
        rough_3 = rough_2;
    }
    float _S214 = dot(n_11, v_4);
    if(_S214 < 0.0f)
    {
        n_11 = normalize(n_11 - float3((1.0099999904632568f * _S214))  * v_4);
    }
    if((_S191.w) > 0.5f)
    {
        float2 c_11 = floor(local_0.xz * float2(0.5f) );
        albedo_4 = albedo_4 * float3(mix(0.80000001192092896f, 1.1499999761581421f, fmod(abs(c_11.x + c_11.y), 2.0f))) ;
    }
    float3 p_14 = node_0 + local_0;
    float zlin_1 = - (((float4(p_14, 1.0f)) * (matrix<float,int(4),int(4)> (_S166->view_0.data_1[int(0)][int(0)], _S166->view_0.data_1[int(1)][int(0)], _S166->view_0.data_1[int(2)][int(0)], _S166->view_0.data_1[int(3)][int(0)], _S166->view_0.data_1[int(0)][int(1)], _S166->view_0.data_1[int(1)][int(1)], _S166->view_0.data_1[int(2)][int(1)], _S166->view_0.data_1[int(3)][int(1)], _S166->view_0.data_1[int(0)][int(2)], _S166->view_0.data_1[int(1)][int(2)], _S166->view_0.data_1[int(2)][int(2)], _S166->view_0.data_1[int(3)][int(2)], _S166->view_0.data_1[int(0)][int(3)], _S166->view_0.data_1[int(1)][int(3)], _S166->view_0.data_1[int(2)][int(3)], _S166->view_0.data_1[int(3)][int(3)])))).z;
    float3 ng_2 = cross(e1_1, e2_1);
    float3 ng_3;
    if((length_squared_0(ng_2)) > 9.99999968265522539e-21f)
    {
        ng_3 = normalize(ng_2);
    }
    else
    {
        ng_3 = n_11;
    }
    if((dot(ng_3, v_4)) < 0.0f)
    {
        ng_3 = - ng_3;
    }
    float _S215 = sun_shadow_0(_S166, (&kernelContext_31)->ed_c_0->shadow_0, p_14, ng_3, zlin_1, _S167, &kernelContext_31);
    float2 _S216 = ao_upsample_0(_S166, (&kernelContext_31)->ed_c_0->ao_0, _S167, zlin_1, &kernelContext_31);
    float ao_6 = ao_5 * _S216.x;
    float sv_4 = _S216.y;
    float3 _S217 = sun_light_at_0(_S166, p_14, &kernelContext_31);
    float3 col_1;
    if((_S190.w) > 0.5f)
    {
        float3 _S218 = shade_toon_0(_S166, _S217, &_S189, albedo_4, n_11, v_4, ao_6, sv_4, _S215, &kernelContext_31);
        col_1 = _S218;
    }
    else
    {
        float3 _S219 = shade_pbr_0(_S166, _S217, albedo_4, metal_2, rough_3, ao_6, sv_4, _S215, n_11, v_4, &kernelContext_31);
        col_1 = _S219;
    }
    float3 _S220 = apply_aerial_0(_S166, col_1, p_14, _S167, &kernelContext_31);
    if(((uint4((&_S166->light_0)->flags_0) ).w) == 1U)
    {
        uint _S221 = shadow_cascade_0(_S166, p_14, zlin_1);
        col_1 = TINT_0[_S221] * float3((0.039999999105930328f + 0.5f * _S215)) ;
    }
    else
    {
        if(((uint4((&_S166->light_0)->flags_0) ).w) == 3U)
        {
            uint _S222 = shadow_cascade_0(_S166, p_14, zlin_1);
            if(_S222 < 4U)
            {
                float _S223 = cascade_depth_delta_0(_S166, (&kernelContext_31)->ed_c_0->shadow_0, _S222, p_14, &kernelContext_31);
                d_12 = _S223;
            }
            else
            {
                d_12 = 0.0f;
            }
            col_1 = float3(max(d_12, 0.0f), max(- d_12, 0.0f), 0.0f) * float3(0.10000000149011612f) ;
        }
        else
        {
            if(((uint4((&_S166->light_0)->flags_0) ).w) == 2U)
            {
                col_1 = float3((0.5f * ao_6)) ;
            }
            else
            {
                if(((uint4((&_S166->light_0)->flags_0) ).w) == 4U)
                {
                    col_1 = float3((0.5f * sv_4)) ;
                }
                else
                {
                    col_1 = _S220;
                }
            }
        }
    }
    pixelOutput_0 _S224 = { float4(col_1, 1.0f) };
    return _S224;
}

struct vs_resolve_Result_0
{
    float4 pos_2 [[position]];
};

[[vertex]] vs_resolve_Result_0 vs_resolve(uint vid_1 [[vertex_id]], ResolveC_0 constant* ed_c_2 [[buffer(0)]], ulong device* ed_buffers_2 [[buffer(2)]], ulong device* ed_textures_2 [[buffer(1)]], ulong device* ed_samplers_2 [[buffer(3)]])
{
    thread KernelContext_0 kernelContext_32;
    (&kernelContext_32)->ed_c_0 = ed_c_2;
    (&kernelContext_32)->ed_buffers_0 = ed_buffers_2;
    (&kernelContext_32)->ed_textures_0 = ed_textures_2;
    (&kernelContext_32)->ed_samplers_0 = ed_samplers_2;
    thread vs_resolve_Result_0 _S225;
    (&_S225)->pos_2 = fullscreen_vertex_0(vid_1).pos_0;
    return _S225;
}

