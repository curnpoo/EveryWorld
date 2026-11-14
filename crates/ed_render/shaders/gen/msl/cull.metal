#include <metal_stdlib>
#include <metal_math>
#include <metal_texture>
using namespace metal;
struct GrassType_natural_0
{
    packed_float4 a_0;
    packed_float4 b_0;
    packed_float4 root_0;
    packed_float4 mid_0;
    packed_float4 tip_0;
    packed_float4 dry_0;
};

struct CullC_0
{
    uint frame_0;
    uint instances_0;
    uint meshes_0;
    uint state_0;
    uint args_0;
    uint visible_0;
    uint stats_0;
    uint hzb_0;
    uint count_0;
    uint bins_kind_0;
    uint templ_0;
    uint nodes_0;
    array<uint, int(4)> pad_0;
};

struct KernelContext_0
{
    CullC_0 constant* ed_c_0;
    ulong device* ed_buffers_0;
};

GrassType_natural_0 device* ed_buf_0(uint i_0, KernelContext_0 thread* kernelContext_0)
{
    return (GrassType_natural_0 device*)kernelContext_0->ed_buffers_0[i_0];
}

struct GrassClump_0
{
    float px_0;
    float py_0;
    float pz_0;
    uint node_kind_0;
    uint up_0;
    uint facing_0;
    uint seed_0;
    uint hw_0;
};

GrassClump_0 device* ed_buf_1(uint i_1, KernelContext_0 thread* kernelContext_1)
{
    return (GrassClump_0 device*)kernelContext_1->ed_buffers_0[i_1];
}

constant array<uint, int(5)> GRASS_FIRST_0 = { { 0U, 2304U, 3264U, 3648U, 3792U } };
constant array<uint, int(5)> GRASS_SEGS_0 = { { 6U, 5U, 4U, 3U, 2U } };
constant array<uint, int(5)> GRASS_SLOTS_0 = { { 64U, 32U, 16U, 8U, 4U } };
struct GpuImpKind_natural_0
{
    packed_uint4 tex_0;
    packed_float4 geo_0;
    packed_float4 leaf_0;
    packed_float4 bark_0;
};

GpuImpKind_natural_0 device* ed_buf_2(uint i_2, KernelContext_0 thread* kernelContext_2)
{
    return (GpuImpKind_natural_0 device*)kernelContext_2->ed_buffers_0[i_2];
}

struct GpuTree_0
{
    float px_1;
    float py_1;
    float pz_1;
    uint node_kind_1;
    float scale_0;
    uint up_1;
    uint yaw_widen_0;
    uint user_0;
    uint season_0;
};

GpuTree_0 device* ed_buf_3(uint i_3, KernelContext_0 thread* kernelContext_3)
{
    return (GpuTree_0 device*)kernelContext_3->ed_buffers_0[i_3];
}

constant array<float, int(4)> SHADOW_LOD_BIAS_0 = { { 8.0f, 64.0f, 128.0f, 128.0f } };
float device* ed_buf_4(uint i_4, KernelContext_0 thread* kernelContext_4)
{
    return (float device*)kernelContext_4->ed_buffers_0[i_4];
}

packed_float4 device* ed_buf_5(uint i_5, KernelContext_0 thread* kernelContext_5)
{
    return (packed_float4 device*)kernelContext_5->ed_buffers_0[i_5];
}

struct GpuMesh_natural_0
{
    packed_float4 sphere_0;
    uint lod_count_0;
    uint bin_base_0;
    uint capacity_0;
    uint flags_0;
    array<uint, int(4)> first_index_0;
    array<uint, int(4)> index_count_0;
    array<int, int(4)> base_vertex_0;
    array<float, int(4)> error_0;
};

GpuMesh_natural_0 device* ed_buf_6(uint i_6, KernelContext_0 thread* kernelContext_6)
{
    return (GpuMesh_natural_0 device*)kernelContext_6->ed_buffers_0[i_6];
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

GpuInstance_natural_0 device* ed_buf_7(uint i_7, KernelContext_0 thread* kernelContext_7)
{
    return (GpuInstance_natural_0 device*)kernelContext_7->ed_buffers_0[i_7];
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
    _Array_natural_vectorx3Cfloatx2C4x3E4_0 cascade_0;
    packed_float4 split_0;
    packed_float4 cfg_0;
    packed_float4 filter_0;
    packed_uint4 flags_1;
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
    packed_float4 up_2;
    packed_float4 ap_0;
    packed_uint4 bufs_0;
    packed_uint4 bufs2_0;
    packed_uint4 cfg_1;
};

struct WindData_natural_0
{
    packed_float4 wind_0;
    packed_float4 gust_0;
    packed_float4 cfg_2;
};

struct ImpData_natural_0
{
    packed_uint4 bufs_1;
    packed_float4 band_0;
    packed_float4 far_0;
    packed_float4 cfg_3;
};

struct _Array_natural_vectorx3Cfloatx2C4x3E64_0
{
    array<packed_float4, int(64)> data_6;
};

struct SeasonData_natural_0
{
    packed_float4 cfg_4;
    _Array_natural_vectorx3Cfloatx2C4x3E64_0 rows_0;
};

struct GrassData_natural_0
{
    packed_uint4 bufs_2;
    packed_float4 lod_0;
    packed_float4 look_0;
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
    array<HzbLevel_0, int(16)> hzb_1;
    LightData_natural_0 light_0;
    AtmosData_natural_0 atmos_0;
    WindData_natural_0 wind_1;
    ImpData_natural_0 imp_0;
    SeasonData_natural_0 season_1;
    GrassData_natural_0 grass_0;
};

FrameData_natural_0 device* ed_buf_8(uint i_8, KernelContext_0 thread* kernelContext_8)
{
    return (FrameData_natural_0 device*)kernelContext_8->ed_buffers_0[i_8];
}

uint device* ed_buf_9(uint i_9, KernelContext_0 thread* kernelContext_9)
{
    return (uint device*)kernelContext_9->ed_buffers_0[i_9];
}

[[kernel]] void cull_reset(uint3 tid_0 [[thread_position_in_grid]], CullC_0 constant* ed_c_1 [[buffer(0)]], ulong device* ed_buffers_1 [[buffer(2)]])
{
    thread KernelContext_0 kernelContext_10;
    (&kernelContext_10)->ed_c_0 = ed_c_1;
    (&kernelContext_10)->ed_buffers_0 = ed_buffers_1;
    uint i_10 = tid_0.x;
    if(i_10 < (ed_c_1->bins_kind_0 * 4U * 5U))
    {
        uint device* _S1 = ed_buf_9((&kernelContext_10)->ed_c_0->args_0, &kernelContext_10);
        uint device* _S2 = _S1 + i_10;
        uint device* _S3 = ed_buf_9((&kernelContext_10)->ed_c_0->templ_0, &kernelContext_10);
        *_S2 = *(_S3 + i_10);
    }
    if(i_10 < 20U)
    {
        uint device* _S4 = ed_buf_9((&kernelContext_10)->ed_c_0->stats_0, &kernelContext_10);
        *(_S4 + i_10) = 0U;
    }
    return;
}

struct Emitted_0
{
    uint entries_0;
    uint nonempty_0;
    uint tris_0;
};

Emitted_0 Emitted_x24init_0(uint entries_1, uint nonempty_1, uint tris_1)
{
    thread Emitted_0 _S5;
    (&_S5)->entries_0 = entries_1;
    (&_S5)->nonempty_0 = nonempty_1;
    (&_S5)->tris_0 = tris_1;
    return _S5;
}

float length_squared_0(float3 v_0)
{
    return dot(v_0, v_0);
}

float max_scale_0(const GpuInstance_natural_0 thread* i_11)
{
    float4 _S6 = float4((&i_11->m_0)->data_0[int(0)]) ;
    float4 _S7 = float4((&i_11->m_0)->data_0[int(1)]) ;
    float4 _S8 = float4((&i_11->m_0)->data_0[int(2)]) ;
    return sqrt(max(max(length_squared_0(float3(_S6.x, _S7.x, _S8.x)), length_squared_0(float3(_S6.y, _S7.y, _S8.y))), length_squared_0(float3(_S6.z, _S7.z, _S8.z))));
}

float3 xform_point_0(const GpuInstance_natural_0 thread* i_12, float3 p_0)
{
    float4 _S9 = float4((&i_12->m_0)->data_0[int(0)]) ;
    float4 _S10 = float4((&i_12->m_0)->data_0[int(1)]) ;
    float4 _S11 = float4((&i_12->m_0)->data_0[int(2)]) ;
    return float3(dot(_S9.xyz, p_0) + _S9.w, dot(_S10.xyz, p_0) + _S10.w, dot(_S11.xyz, p_0) + _S11.w);
}

struct Bounds_0
{
    float3 c_0;
    float r_0;
    float scale_1;
};

Bounds_0 bounds_0(const GpuInstance_natural_0 thread* inst_0, GpuMesh_natural_0 device* m_1, KernelContext_0 thread* kernelContext_11)
{
    packed_float4 device* _S12 = ed_buf_5(kernelContext_11->ed_c_0->nodes_0, kernelContext_11);
    thread Bounds_0 b_1;
    float _S13 = max_scale_0(inst_0);
    (&b_1)->scale_1 = _S13;
    float3 _S14 = (float4(*(_S12 + ((inst_0->node_flags_0) & 65535U))) ).xyz;
    float3 _S15 = xform_point_0(inst_0, (float4(m_1->sphere_0) ).xyz);
    (&b_1)->c_0 = _S14 + _S15;
    (&b_1)->r_0 = (float4(m_1->sphere_0) ).w * _S13;
    return b_1;
}

bool in_frustum_0(FrameData_natural_0 device* f_0, float3 c_1, float r_1)
{
    int k_0 = int(0);
    for(;;)
    {
        if(k_0 < int(5))
        {
        }
        else
        {
            break;
        }
        float4 _S16 = float4((&f_0->planes_0)->data_2[k_0]) ;
        if((dot(_S16.xyz, c_1) + _S16.w) < (- r_1))
        {
            return false;
        }
        k_0 = k_0 + int(1);
    }
    return true;
}

uint select_lod_0(GpuMesh_natural_0 device* m_2, float px_per_m_0, uint cur_0, bool init_0, float thr_0, float hyst_0)
{
    uint _S17 = clamp(m_2->lod_count_0, 1U, 4U);
    uint want_0 = 0U;
    uint l_0 = 1U;
    for(;;)
    {
        if(l_0 < _S17)
        {
        }
        else
        {
            break;
        }
        if((m_2->error_0[l_0] * px_per_m_0) <= thr_0)
        {
            want_0 = l_0;
        }
        l_0 = l_0 + 1U;
    }
    bool _S18;
    if(init_0)
    {
        _S18 = want_0 > cur_0;
    }
    else
    {
        _S18 = false;
    }
    if(_S18)
    {
        uint _S19 = cur_0 + 1U;
        uint w_0 = cur_0;
        l_0 = _S19;
        for(;;)
        {
            if(l_0 < _S17)
            {
            }
            else
            {
                break;
            }
            if((m_2->error_0[l_0] * px_per_m_0) <= (thr_0 * hyst_0))
            {
                w_0 = l_0;
            }
            l_0 = l_0 + 1U;
        }
        return w_0;
    }
    return want_0;
}

void push_0(atomic<uint> device* args_1, uint device* args_plain_0, uint device* visible_1, uint bin_0, uint entry_0, uint cap_0, uint tris_2, Emitted_0 thread* e_0)
{
    uint _S20 = bin_0 * 5U;
    atomic<uint> device* _S21 = args_1 + (_S20 + 1U);
    uint slot_0 = atomic_fetch_add_explicit(_S21, 1U, memory_order_relaxed);
    if(slot_0 >= cap_0)
    {
        uint _S22 = atomic_fetch_sub_explicit(_S21, 1U, memory_order_relaxed);
        return;
    }
    if(slot_0 == 0U)
    {
        e_0->nonempty_0 = e_0->nonempty_0 + 1U;
    }
    *(visible_1 + (*(args_plain_0 + (_S20 + 4U)) + slot_0)) = entry_0;
    e_0->entries_0 = e_0->entries_0 + 1U;
    e_0->tris_0 = e_0->tris_0 + tris_2;
    return;
}

void emit_0(GpuMesh_natural_0 device* m_3, uint inst_1, uint st_0, uint phase_0, uint fade_frames_0, Emitted_0 thread* e_1, KernelContext_0 thread* kernelContext_12)
{
    uint device* _S23 = ed_buf_9(kernelContext_12->ed_c_0->args_0, kernelContext_12);
    atomic<uint> device* _S24 = (atomic<uint> device*)((ulong)(_S23));
    uint device* _S25 = ed_buf_9(kernelContext_12->ed_c_0->visible_0, kernelContext_12);
    uint cur_1 = st_0 & 3U;
    uint prev_1 = (st_0 >> 2U) & 3U;
    uint fade_0 = (st_0 >> 4U) & 63U;
    uint phase_base_0 = phase_0 * 2U * kernelContext_12->ed_c_0->bins_kind_0 + m_3->bin_base_0;
    uint cap_1 = m_3->capacity_0;
    if(fade_0 == 0U)
    {
        push_0(_S24, _S23, _S25, phase_base_0 + cur_1, inst_1 | (cur_1 << 20U), cap_1, m_3->index_count_0[cur_1] / 3U, e_1);
    }
    else
    {
        uint _S26 = (fade_frames_0 - fade_0 + 1U) * 256U / (fade_frames_0 + 1U);
        uint dith_0 = phase_base_0 + kernelContext_12->ed_c_0->bins_kind_0;
        uint _S27 = (clamp(_S26, 1U, 255U)) << 22U;
        push_0(_S24, _S23, _S25, dith_0 + cur_1, (inst_1 | (cur_1 << 20U)) | _S27, cap_1, m_3->index_count_0[cur_1] / 3U, e_1);
        push_0(_S24, _S23, _S25, dith_0 + prev_1, ((inst_1 | (prev_1 << 20U)) | _S27) | 1073741824U, cap_1, m_3->index_count_0[prev_1] / 3U, e_1);
    }
    return;
}

void stat_add_0(atomic<uint> device* stats_1, uint idx_0, uint v_1)
{
    uint s_0 = simd_sum(v_1);
    bool _S28 = simd_is_first();
    bool _S29;
    if(_S28)
    {
        _S29 = s_0 != 0U;
    }
    else
    {
        _S29 = false;
    }
    if(_S29)
    {
        uint _S30 = atomic_fetch_add_explicit(stats_1 + idx_0, s_0, memory_order_relaxed);
    }
    return;
}

[[kernel]] void cull_early(uint3 tid_1 [[thread_position_in_grid]], CullC_0 constant* ed_c_2 [[buffer(0)]], ulong device* ed_buffers_2 [[buffer(2)]])
{
    thread KernelContext_0 kernelContext_13;
    (&kernelContext_13)->ed_c_0 = ed_c_2;
    (&kernelContext_13)->ed_buffers_0 = ed_buffers_2;
    uint i_13 = tid_1.x;
    FrameData_natural_0 device* _S31 = ed_buf_8(ed_c_2->frame_0, &kernelContext_13);
    uint device* _S32 = ed_buf_9((&kernelContext_13)->ed_c_0->stats_0, &kernelContext_13);
    atomic<uint> device* _S33 = (atomic<uint> device*)((ulong)(_S32));
    thread Emitted_0 e_2 = Emitted_x24init_0(0U, 0U, 0U);
    uint drawn_0;
    if(i_13 < ((&kernelContext_13)->ed_c_0->count_0))
    {
        uint device* _S34 = ed_buf_9((&kernelContext_13)->ed_c_0->state_0, &kernelContext_13);
        GpuInstance_natural_0 device* _S35 = ed_buf_7((&kernelContext_13)->ed_c_0->instances_0, &kernelContext_13);
        thread GpuInstance_natural_0 _S36 = *(_S35 + i_13);
        if((((&_S36)->node_flags_0) & 65536U) == 0U)
        {
            *(_S34 + i_13) = 0U;
            drawn_0 = 0U;
        }
        else
        {
            GpuMesh_natural_0 device* _S37 = ed_buf_6((&kernelContext_13)->ed_c_0->meshes_0, &kernelContext_13);
            GpuMesh_natural_0 device* _S38 = _S37 + (&_S36)->mesh_0;
            Bounds_0 _S39 = bounds_0(&_S36, _S38, &kernelContext_13);
            bool _S40 = in_frustum_0(_S31, _S39.c_0, _S39.r_0);
            uint device* _S41 = _S34 + i_13;
            uint st_1 = *_S41;
            bool init_1 = ((*_S41) & 2048U) != 0U;
            uint cur_2 = (*_S41) & 3U;
            uint _S42 = ((*_S41) >> 2U) & 3U;
            uint fade_1 = ((*_S41) >> 4U) & 63U;
            uint n_0 = (uint4(_S31->cull_0) ).y;
            float4 _S43 = float4(_S31->params_0) ;
            float4 _S44 = float4(_S31->proj_0) ;
            uint _S45 = select_lod_0(_S38, _S39.scale_1 * _S44.w * 0.5f * _S44.y / max(length(_S39.c_0) - _S39.r_0, _S43.z), cur_2, init_1, _S43.y, _S43.w);
            uint fade_2;
            if(fade_1 > 0U)
            {
                fade_2 = fade_1 - 1U;
            }
            else
            {
                fade_2 = fade_1;
            }
            bool changed_0;
            if(!init_1)
            {
                changed_0 = true;
            }
            else
            {
                changed_0 = !_S40;
            }
            if(changed_0)
            {
                changed_0 = true;
            }
            else
            {
                changed_0 = n_0 == 0U;
            }
            uint cur_3;
            uint prev_2;
            if(changed_0)
            {
                if(init_1)
                {
                    changed_0 = _S40;
                }
                else
                {
                    changed_0 = false;
                }
                if(changed_0)
                {
                    changed_0 = _S45 != cur_2;
                }
                else
                {
                    changed_0 = false;
                }
                cur_3 = _S45;
                prev_2 = _S42;
                fade_2 = 0U;
            }
            else
            {
                if(fade_2 == 0U)
                {
                    changed_0 = _S45 != cur_2;
                }
                else
                {
                    changed_0 = false;
                }
                if(changed_0)
                {
                    cur_3 = _S45;
                    prev_2 = cur_2;
                    fade_2 = n_0;
                }
                else
                {
                    cur_3 = cur_2;
                    prev_2 = _S42;
                }
            }
            if(fade_2 == 0U)
            {
                prev_2 = cur_3;
            }
            uint _S46 = (((cur_3 | (prev_2 << 2U)) | (fade_2 << 4U)) | (st_1 & 1024U)) | 2048U;
            if(changed_0)
            {
                fade_2 = 4096U;
            }
            else
            {
                fade_2 = 0U;
            }
            uint st_2 = _S46 | fade_2;
            *_S41 = st_2;
            if(_S40)
            {
                changed_0 = (st_2 & 1024U) != 0U;
            }
            else
            {
                changed_0 = false;
            }
            if(changed_0)
            {
                emit_0(_S38, i_13, st_2, 0U, n_0, &e_2, &kernelContext_13);
                drawn_0 = 1U;
            }
            else
            {
                drawn_0 = 0U;
            }
        }
    }
    else
    {
        drawn_0 = 0U;
    }
    stat_add_0(_S33, 3U, drawn_0);
    stat_add_0(_S33, 5U, (&e_2)->entries_0);
    stat_add_0(_S33, 6U, (&e_2)->nonempty_0);
    stat_add_0(_S33, 8U, (&e_2)->tris_0);
    return;
}

bool project_sphere_0(float3 c_2, float r_2, float znear_0, float p00_0, float p11_0, float4 thread* aabb_0)
{
    *aabb_0 = float4(0.0f) ;
    float _S47 = c_2.z;
    if(_S47 < (r_2 + znear_0))
    {
        return false;
    }
    float3 cr_0 = c_2 * float3(r_2) ;
    float czr2_0 = _S47 * _S47 - r_2 * r_2;
    float _S48 = c_2.x;
    float vx_0 = sqrt(_S48 * _S48 + czr2_0);
    float _S49 = vx_0 * _S48;
    float _S50 = cr_0.z;
    float _S51 = vx_0 * _S47;
    float _S52 = cr_0.x;
    float _S53 = c_2.y;
    float vy_0 = sqrt(_S53 * _S53 + czr2_0);
    float _S54 = vy_0 * _S53;
    float _S55 = vy_0 * _S47;
    float _S56 = cr_0.y;
    *aabb_0 = float4((_S49 - _S50) / (_S51 + _S52) * p00_0, (_S54 + _S50) / (_S55 - _S56) * p11_0, (_S49 + _S50) / (_S51 - _S52) * p00_0, (_S54 - _S50) / (_S55 + _S56) * p11_0) * float4(0.5f, -0.5f, 0.5f, -0.5f) + float4(0.5f) ;
    return true;
}

bool occluded_0(FrameData_natural_0 device* f_1, float device* hzb_2, float3 c_rel_0, float r_3)
{
    float3 v_2 = (((float4(c_rel_0, 1.0f)) * (matrix<float,int(4),int(4)> (f_1->view_0.data_1[int(0)][int(0)], f_1->view_0.data_1[int(1)][int(0)], f_1->view_0.data_1[int(2)][int(0)], f_1->view_0.data_1[int(3)][int(0)], f_1->view_0.data_1[int(0)][int(1)], f_1->view_0.data_1[int(1)][int(1)], f_1->view_0.data_1[int(2)][int(1)], f_1->view_0.data_1[int(3)][int(1)], f_1->view_0.data_1[int(0)][int(2)], f_1->view_0.data_1[int(1)][int(2)], f_1->view_0.data_1[int(2)][int(2)], f_1->view_0.data_1[int(3)][int(2)], f_1->view_0.data_1[int(0)][int(3)], f_1->view_0.data_1[int(1)][int(3)], f_1->view_0.data_1[int(2)][int(3)], f_1->view_0.data_1[int(3)][int(3)])))).xyz;
    float _S57 = - v_2.z;
    float4 _S58 = float4(f_1->proj_0) ;
    float znear_1 = (float4(f_1->params_0) ).z;
    thread float4 aabb_1;
    bool _S59 = project_sphere_0(float3(v_2.x, v_2.y, _S57), r_3, znear_1, _S58.x, _S58.y, &aabb_1);
    if(!_S59)
    {
        return false;
    }
    float w_1 = _S58.z;
    float h_0 = _S58.w;
    float x0_0 = clamp(aabb_1.x, 0.0f, 1.0f) * w_1;
    float y0_0 = clamp(aabb_1.y, 0.0f, 1.0f) * h_0;
    float x1_0 = clamp(aabb_1.z, 0.0f, 1.0f) * w_1;
    float y1_0 = clamp(aabb_1.w, 0.0f, 1.0f) * h_0;
    float _S60 = max(x1_0 - x0_0, y1_0 - y0_0);
    int levels_0 = int((uint4(f_1->cull_0) ).x);
    int l_1;
    if(_S60 <= 1.0f)
    {
        l_1 = int(0);
    }
    else
    {
        l_1 = int(ceil(log2(_S60))) - int(1);
    }
    if(l_1 > (levels_0 - int(1)))
    {
        return false;
    }
    int _S61 = max(l_1, int(0));
    float ts_0 = float(2U << uint(_S61));
    uint _S62 = f_1->hzb_1[_S61].width_0 - 1U;
    uint _S63 = min(uint(x0_0 / ts_0), _S62);
    uint _S64 = f_1->hzb_1[_S61].height_0 - 1U;
    uint _S65 = min(uint(x1_0 / ts_0), _S62);
    float device* p_1 = hzb_2 + f_1->hzb_1[_S61].offset_0;
    uint _S66 = min(uint(y0_0 / ts_0), _S64) * f_1->hzb_1[_S61].width_0;
    uint _S67 = min(uint(y1_0 / ts_0), _S64) * f_1->hzb_1[_S61].width_0;
    return (znear_1 / (_S57 - r_3)) < (min(min(*(p_1 + (_S66 + _S63)), *(p_1 + (_S66 + _S65))), min(*(p_1 + (_S67 + _S63)), *(p_1 + (_S67 + _S65)))));
}

[[kernel]] void cull_late(uint3 tid_2 [[thread_position_in_grid]], CullC_0 constant* ed_c_3 [[buffer(0)]], ulong device* ed_buffers_3 [[buffer(2)]])
{
    thread KernelContext_0 kernelContext_14;
    (&kernelContext_14)->ed_c_0 = ed_c_3;
    (&kernelContext_14)->ed_buffers_0 = ed_buffers_3;
    uint i_14 = tid_2.x;
    FrameData_natural_0 device* _S68 = ed_buf_8(ed_c_3->frame_0, &kernelContext_14);
    uint device* _S69 = ed_buf_9((&kernelContext_14)->ed_c_0->stats_0, &kernelContext_14);
    atomic<uint> device* _S70 = (atomic<uint> device*)((ulong)(_S69));
    thread Emitted_0 e_3 = Emitted_x24init_0(0U, 0U, 0U);
    uint total_0;
    uint fr_0;
    uint occ_0;
    uint drawn_1;
    uint fading_0;
    uint changed_1;
    if(i_14 < ((&kernelContext_14)->ed_c_0->count_0))
    {
        GpuInstance_natural_0 device* _S71 = ed_buf_7((&kernelContext_14)->ed_c_0->instances_0, &kernelContext_14);
        thread GpuInstance_natural_0 _S72 = *(_S71 + i_14);
        if((((&_S72)->node_flags_0) & 65536U) != 0U)
        {
            uint device* _S73 = ed_buf_9((&kernelContext_14)->ed_c_0->state_0, &kernelContext_14);
            GpuMesh_natural_0 device* _S74 = ed_buf_6((&kernelContext_14)->ed_c_0->meshes_0, &kernelContext_14);
            GpuMesh_natural_0 device* _S75 = _S74 + (&_S72)->mesh_0;
            Bounds_0 _S76 = bounds_0(&_S72, _S75, &kernelContext_14);
            uint device* _S77 = _S73 + i_14;
            uint st_3 = *_S77;
            bool _S78 = in_frustum_0(_S68, _S76.c_0, _S76.r_0);
            bool vis_0;
            if(_S78)
            {
                vis_0 = ((uint4(_S68->cull_0) ).x) > 0U;
            }
            else
            {
                vis_0 = false;
            }
            if(vis_0)
            {
                float device* _S79 = ed_buf_4((&kernelContext_14)->ed_c_0->hzb_0, &kernelContext_14);
                bool _S80 = occluded_0(_S68, _S79, _S76.c_0, _S76.r_0);
                vis_0 = !_S80;
            }
            else
            {
                vis_0 = _S78;
            }
            int _S81;
            if(_S78)
            {
                _S81 = int(1);
            }
            else
            {
                _S81 = int(0);
            }
            uint _S82 = uint(_S81);
            if(vis_0)
            {
                _S81 = int(1);
            }
            else
            {
                _S81 = int(0);
            }
            uint _S83 = uint(_S81);
            bool early_0;
            if(_S78)
            {
                early_0 = (st_3 & 1024U) != 0U;
            }
            else
            {
                early_0 = false;
            }
            bool _S84;
            if(vis_0)
            {
                _S84 = !early_0;
            }
            else
            {
                _S84 = false;
            }
            if(_S84)
            {
                emit_0(_S75, i_14, st_3, 1U, (uint4(_S68->cull_0) ).y, &e_3, &kernelContext_14);
                total_0 = 1U;
            }
            else
            {
                total_0 = 0U;
            }
            if(early_0)
            {
                early_0 = true;
            }
            else
            {
                early_0 = total_0 != 0U;
            }
            if(early_0)
            {
                if(((st_3 >> 4U) & 63U) != 0U)
                {
                    _S81 = int(1);
                }
                else
                {
                    _S81 = int(0);
                }
                uint _S85 = uint(_S81);
                if((st_3 & 4096U) != 0U)
                {
                    _S81 = int(1);
                }
                else
                {
                    _S81 = int(0);
                }
                uint _S86 = uint(_S81);
                fr_0 = _S85;
                occ_0 = _S86;
            }
            else
            {
                fr_0 = 0U;
                occ_0 = 0U;
            }
            uint _S87 = st_3 & 4294966271U;
            if(vis_0)
            {
                drawn_1 = 1024U;
            }
            else
            {
                drawn_1 = 0U;
            }
            *_S77 = _S87 | drawn_1;
            uint _S88 = total_0;
            uint _S89 = fr_0;
            uint _S90 = occ_0;
            total_0 = 1U;
            fr_0 = _S82;
            occ_0 = _S83;
            drawn_1 = _S88;
            fading_0 = _S89;
            changed_1 = _S90;
        }
        else
        {
            total_0 = 0U;
            fr_0 = 0U;
            occ_0 = 0U;
            drawn_1 = 0U;
            fading_0 = 0U;
            changed_1 = 0U;
        }
    }
    else
    {
        total_0 = 0U;
        fr_0 = 0U;
        occ_0 = 0U;
        drawn_1 = 0U;
        fading_0 = 0U;
        changed_1 = 0U;
    }
    stat_add_0(_S70, 0U, total_0);
    stat_add_0(_S70, 1U, fr_0);
    stat_add_0(_S70, 2U, occ_0);
    stat_add_0(_S70, 4U, drawn_1);
    stat_add_0(_S70, 5U, (&e_3)->entries_0);
    stat_add_0(_S70, 6U, (&e_3)->nonempty_0);
    stat_add_0(_S70, 8U, (&e_3)->tris_0);
    stat_add_0(_S70, 7U, fading_0);
    stat_add_0(_S70, 9U, changed_1);
    return;
}

[[kernel]] void shadow_reset(uint3 tid_3 [[thread_position_in_grid]], CullC_0 constant* ed_c_4 [[buffer(0)]], ulong device* ed_buffers_4 [[buffer(2)]])
{
    thread KernelContext_0 kernelContext_15;
    (&kernelContext_15)->ed_c_0 = ed_c_4;
    (&kernelContext_15)->ed_buffers_0 = ed_buffers_4;
    uint i_15 = tid_3.x;
    if(i_15 < (ed_c_4->bins_kind_0 * 4U * 5U))
    {
        uint device* _S91 = ed_buf_9((&kernelContext_15)->ed_c_0->args_0, &kernelContext_15);
        uint device* _S92 = _S91 + i_15;
        uint device* _S93 = ed_buf_9((&kernelContext_15)->ed_c_0->templ_0, &kernelContext_15);
        *_S92 = *(_S93 + i_15);
    }
    return;
}

[[kernel]] void shadow_cull(uint3 tid_4 [[thread_position_in_grid]], CullC_0 constant* ed_c_5 [[buffer(0)]], ulong device* ed_buffers_5 [[buffer(2)]])
{
    thread KernelContext_0 kernelContext_16;
    (&kernelContext_16)->ed_c_0 = ed_c_5;
    (&kernelContext_16)->ed_buffers_0 = ed_buffers_5;
    uint i_16 = tid_4.x;
    FrameData_natural_0 device* _S94 = ed_buf_8(ed_c_5->frame_0, &kernelContext_16);
    uint device* _S95 = ed_buf_9((&kernelContext_16)->ed_c_0->stats_0, &kernelContext_16);
    atomic<uint> device* _S96 = (atomic<uint> device*)((ulong)(_S95));
    thread Emitted_0 e_4 = Emitted_x24init_0(0U, 0U, 0U);
    uint cnt0_0;
    uint cnt1_0;
    uint cnt2_0;
    uint cnt3_0;
    if(i_16 < ((&kernelContext_16)->ed_c_0->count_0))
    {
        GpuInstance_natural_0 device* _S97 = ed_buf_7((&kernelContext_16)->ed_c_0->instances_0, &kernelContext_16);
        thread GpuInstance_natural_0 _S98 = *(_S97 + i_16);
        uint _S99 = (&_S98)->node_flags_0;
        if((((&_S98)->node_flags_0) & 65536U) != 0U)
        {
            uint device* _S100 = ed_buf_9((&kernelContext_16)->ed_c_0->args_0, &kernelContext_16);
            atomic<uint> device* _S101 = (atomic<uint> device*)((ulong)(_S100));
            uint device* _S102 = ed_buf_9((&kernelContext_16)->ed_c_0->visible_0, &kernelContext_16);
            GpuMesh_natural_0 device* _S103 = ed_buf_6((&kernelContext_16)->ed_c_0->meshes_0, &kernelContext_16);
            GpuMesh_natural_0 device* _S104 = _S103 + (&_S98)->mesh_0;
            Bounds_0 _S105 = bounds_0(&_S98, _S104, &kernelContext_16);
            uint _S106 = min(uint((float4((&_S94->light_0)->cfg_0) ).x), 4U);
            float _S107 = (float4((&_S94->light_0)->cfg_0) ).y;
            bool _S108;
            uint nc_0;
            uint lod_min_0;
            uint lod_max_0;
            if(((_S104->flags_0) & 1U) != 0U)
            {
                float4 _S109 = float4(_S94->proj_0) ;
                uint _S110 = select_lod_0(_S104, _S105.scale_1 * _S109.w * 0.5f * _S109.y / max(length(_S105.c_0) - _S105.r_0, (float4(_S94->params_0) ).z), 0U, false, (float4(_S94->params_0) ).y, 1.0f);
                if((_S104->index_count_0[_S110]) == 0U)
                {
                    _S108 = true;
                }
                else
                {
                    if((_S104->lod_count_0) >= 4U)
                    {
                        _S108 = (_S110 + 2U) >= (_S104->lod_count_0);
                    }
                    else
                    {
                        _S108 = false;
                    }
                }
                if(_S108)
                {
                    nc_0 = 0U;
                }
                else
                {
                    nc_0 = _S106;
                }
                uint lod_max_1 = max(_S104->lod_count_0, 2U) - 2U;
                if((((uint4((&_S94->imp_0)->bufs_1) ).w) & 2U) != 0U)
                {
                    nc_0 = min(nc_0, uint((float4((&_S94->imp_0)->cfg_3) ).w));
                }
                lod_min_0 = min(1U, lod_max_1);
                lod_max_0 = lod_max_1;
            }
            else
            {
                nc_0 = _S106;
                lod_min_0 = 0U;
                lod_max_0 = 3U;
            }
            if((_S99 & 131072U) != 0U)
            {
                array<uint, int(4)> _S111 = (&_S98)->pad_1;
                if(((((&_S98)->pad_1[int(2)]) >> 21U) & 1U) != 0U)
                {
                    nc_0 = 0U;
                }
                else
                {
                    if((((_S111[int(2)]) >> 22U) & 1U) != 0U)
                    {
                        nc_0 = min(nc_0, 1U);
                    }
                }
            }
            uint c_3 = 0U;
            cnt0_0 = 0U;
            cnt1_0 = 0U;
            cnt2_0 = 0U;
            cnt3_0 = 0U;
            for(;;)
            {
                if(c_3 < nc_0)
                {
                }
                else
                {
                    break;
                }
                float4 _S112 = float4((&(&_S94->light_0)->cascade_0)->data_4[c_3]) ;
                float4 s_1 = (((float4(_S105.c_0, 1.0f)) * (matrix<float,int(4),int(4)> ((&(&_S94->light_0)->shadow_0)->data_3[c_3].data_1[int(0)][int(0)], (&(&_S94->light_0)->shadow_0)->data_3[c_3].data_1[int(1)][int(0)], (&(&_S94->light_0)->shadow_0)->data_3[c_3].data_1[int(2)][int(0)], (&(&_S94->light_0)->shadow_0)->data_3[c_3].data_1[int(3)][int(0)], (&(&_S94->light_0)->shadow_0)->data_3[c_3].data_1[int(0)][int(1)], (&(&_S94->light_0)->shadow_0)->data_3[c_3].data_1[int(1)][int(1)], (&(&_S94->light_0)->shadow_0)->data_3[c_3].data_1[int(2)][int(1)], (&(&_S94->light_0)->shadow_0)->data_3[c_3].data_1[int(3)][int(1)], (&(&_S94->light_0)->shadow_0)->data_3[c_3].data_1[int(0)][int(2)], (&(&_S94->light_0)->shadow_0)->data_3[c_3].data_1[int(1)][int(2)], (&(&_S94->light_0)->shadow_0)->data_3[c_3].data_1[int(2)][int(2)], (&(&_S94->light_0)->shadow_0)->data_3[c_3].data_1[int(3)][int(2)], (&(&_S94->light_0)->shadow_0)->data_3[c_3].data_1[int(0)][int(3)], (&(&_S94->light_0)->shadow_0)->data_3[c_3].data_1[int(1)][int(3)], (&(&_S94->light_0)->shadow_0)->data_3[c_3].data_1[int(2)][int(3)], (&(&_S94->light_0)->shadow_0)->data_3[c_3].data_1[int(3)][int(3)]))));
                float _S113 = _S112.x;
                float rz_0 = _S105.r_0 / _S112.y;
                float _S114 = 1.0f + _S105.r_0 / (_S113 * _S107 * 0.5f);
                if((abs(s_1.x)) > _S114)
                {
                    _S108 = true;
                }
                else
                {
                    _S108 = (abs(s_1.y)) > _S114;
                }
                bool _S115;
                if(_S108)
                {
                    _S115 = true;
                }
                else
                {
                    _S115 = (s_1.z) < (- rz_0);
                }
                bool _S116;
                if(_S115)
                {
                    _S116 = true;
                }
                else
                {
                    _S116 = (2.0f * _S105.r_0) < (12.0f * _S113);
                }
                if(_S116)
                {
                    c_3 = c_3 + 1U;
                    continue;
                }
                uint _S117 = select_lod_0(_S104, _S105.scale_1 / _S113, 0U, false, (float4(_S94->params_0) ).y * SHADOW_LOD_BIAS_0[c_3], 1.0f);
                uint lod_1 = clamp(_S117, lod_min_0, lod_max_0);
                push_0(_S101, _S100, _S102, c_3 * (&kernelContext_16)->ed_c_0->bins_kind_0 + _S104->bin_base_0 + lod_1, i_16 | (lod_1 << 20U), _S104->capacity_0, _S104->index_count_0[lod_1] / 3U, &e_4);
                uint _S118;
                if(c_3 == 0U)
                {
                    _S118 = 1U;
                }
                else
                {
                    _S118 = 0U;
                }
                uint cnt0_1 = cnt0_0 + _S118;
                uint _S119;
                if(c_3 == 1U)
                {
                    _S119 = 1U;
                }
                else
                {
                    _S119 = 0U;
                }
                uint cnt1_1 = cnt1_0 + _S119;
                uint _S120;
                if(c_3 == 2U)
                {
                    _S120 = 1U;
                }
                else
                {
                    _S120 = 0U;
                }
                uint cnt2_1 = cnt2_0 + _S120;
                uint _S121;
                if(c_3 == 3U)
                {
                    _S121 = 1U;
                }
                else
                {
                    _S121 = 0U;
                }
                uint cnt3_1 = cnt3_0 + _S121;
                cnt0_0 = cnt0_1;
                cnt1_0 = cnt1_1;
                cnt2_0 = cnt2_1;
                cnt3_0 = cnt3_1;
                c_3 = c_3 + 1U;
            }
        }
        else
        {
            cnt0_0 = 0U;
            cnt1_0 = 0U;
            cnt2_0 = 0U;
            cnt3_0 = 0U;
        }
    }
    else
    {
        cnt0_0 = 0U;
        cnt1_0 = 0U;
        cnt2_0 = 0U;
        cnt3_0 = 0U;
    }
    stat_add_0(_S96, 10U, cnt0_0);
    stat_add_0(_S96, 11U, cnt1_0);
    stat_add_0(_S96, 12U, cnt2_0);
    stat_add_0(_S96, 13U, cnt3_0);
    stat_add_0(_S96, 14U, (&e_4)->tris_0);
    return;
}

[[kernel]] void imp_reset(uint3 tid_5 [[thread_position_in_grid]], CullC_0 constant* ed_c_6 [[buffer(0)]], ulong device* ed_buffers_6 [[buffer(2)]])
{
    thread KernelContext_0 kernelContext_17;
    (&kernelContext_17)->ed_c_0 = ed_c_6;
    (&kernelContext_17)->ed_buffers_0 = ed_buffers_6;
    uint i_17 = tid_5.x;
    if(i_17 < 25U)
    {
        uint w_2 = i_17 % 5U;
        uint list_0 = i_17 / 5U;
        uint device* _S122 = ed_buf_9((&kernelContext_17)->ed_c_0->args_0, &kernelContext_17);
        uint device* _S123 = _S122 + i_17;
        uint _S124;
        if(w_2 == 0U)
        {
            _S124 = 6U;
        }
        else
        {
            if(w_2 == 4U)
            {
                _S124 = list_0 * (&kernelContext_17)->ed_c_0->bins_kind_0;
            }
            else
            {
                _S124 = 0U;
            }
        }
        *_S123 = _S124;
    }
    return;
}

float2 unpack_snorm2x16_0(uint p_2)
{
    return clamp(float2(int2(int(p_2 << 16U) >> 16U, int(p_2) >> 16U)) / float2(32767.0f) , float2(-1.0f) , float2(1.0f) );
}

float3 oct_decode_0(uint packed_0)
{
    float2 e_5 = unpack_snorm2x16_0(packed_0);
    float _S125 = e_5.x;
    float _S126 = e_5.y;
    float _S127 = 1.0f - abs(_S125) - abs(_S126);
    thread float3 n_1 = float3(_S125, _S126, _S127);
    float t_0 = saturate(- _S127);
    float _S128;
    if(_S125 >= 0.0f)
    {
        _S128 = - t_0;
    }
    else
    {
        _S128 = t_0;
    }
    n_1.x = n_1.x + _S128;
    if((n_1.y) >= 0.0f)
    {
        _S128 = - t_0;
    }
    else
    {
        _S128 = t_0;
    }
    n_1.y = n_1.y + _S128;
    return normalize(n_1);
}

float imp_hash01_0(uint h_1)
{
    return (float(h_1) + 0.5f) / 256.0f;
}

[[kernel]] void imp_cull(uint3 tid_6 [[thread_position_in_grid]], CullC_0 constant* ed_c_7 [[buffer(0)]], ulong device* ed_buffers_7 [[buffer(2)]])
{
    thread KernelContext_0 kernelContext_18;
    (&kernelContext_18)->ed_c_0 = ed_c_7;
    (&kernelContext_18)->ed_buffers_0 = ed_buffers_7;
    uint i_18 = tid_6.x;
    FrameData_natural_0 device* _S129 = ed_buf_8(ed_c_7->frame_0, &kernelContext_18);
    uint device* _S130 = ed_buf_9((&kernelContext_18)->ed_c_0->stats_0, &kernelContext_18);
    atomic<uint> device* _S131 = (atomic<uint> device*)((ulong)(_S130));
    uint drawn_2;
    uint casters_0;
    if(i_18 < ((&kernelContext_18)->ed_c_0->count_0))
    {
        GpuTree_0 device* _S132 = ed_buf_3((&kernelContext_18)->ed_c_0->instances_0, &kernelContext_18);
        GpuTree_0 device* _S133 = _S132 + i_18;
        GpuTree_0 g_0 = *_S133;
        if(((*_S133).scale_0) > 0.0f)
        {
            GpuImpKind_natural_0 device* _S134 = ed_buf_2((&kernelContext_18)->ed_c_0->meshes_0, &kernelContext_18);
            GpuImpKind_natural_0 k_1 = *(_S134 + (((g_0.node_kind_1) >> 12U) & 4095U));
            packed_float4 device* _S135 = ed_buf_5((&kernelContext_18)->ed_c_0->nodes_0, &kernelContext_18);
            float4 _S136 = float4(k_1.geo_0) ;
            float3 c_4 = (float4(*(_S135 + ((g_0.node_kind_1) & 4095U))) ).xyz + float3(g_0.px_1, g_0.py_1, g_0.pz_1) + oct_decode_0(g_0.up_1) * float3((_S136.x * g_0.scale_0)) ;
            float r_4 = _S136.y * g_0.scale_0 * max(0.5f + float((g_0.yaw_widen_0) >> 16U) / 65535.0f, 1.0f);
            float _S137 = max(length(c_4), (float4(_S129->params_0) ).z);
            float ppr_0 = (float4(_S129->proj_0) ).w * 0.5f * (float4(_S129->proj_0) ).y;
            float4 _S138 = float4((&_S129->imp_0)->band_0) ;
            float _S139 = _S138.y;
            float _S140 = _S138.z;
            float _S141 = _S138.w - _S140;
            float a_out_0 = saturate(((float4((&_S129->imp_0)->cfg_3) ).x * ppr_0 / _S137 - (_S140 + _S141 * imp_hash01_0((g_0.node_kind_1) >> 24U))) / max(_S141 * 0.15000000596046448f, 0.0010000000474974513f));
            uint cov_0 = uint(saturate((_S139 - _S136.z * g_0.scale_0 * ppr_0 / _S137) / max(_S139 - _S138.x, 0.0010000000474974513f)) * a_out_0 * 255.0f + 0.5f);
            uint cov_far_0 = uint(a_out_0 * 255.0f + 0.5f);
            uint _S142 = uint((float4((&_S129->imp_0)->cfg_3) ).w);
            if(cov_far_0 > 0U)
            {
                bool vis_1;
                if(cov_0 > 0U)
                {
                    bool _S143 = in_frustum_0(_S129, c_4, r_4);
                    vis_1 = _S143;
                }
                else
                {
                    vis_1 = false;
                }
                bool _S144;
                if(vis_1)
                {
                    _S144 = ((uint4(_S129->cull_0) ).x) > 0U;
                }
                else
                {
                    _S144 = false;
                }
                if(_S144)
                {
                    float device* _S145 = ed_buf_4((&kernelContext_18)->ed_c_0->hzb_0, &kernelContext_18);
                    bool _S146 = occluded_0(_S129, _S145, c_4, r_4);
                    vis_1 = !_S146;
                }
                if(vis_1)
                {
                    uint device* _S147 = ed_buf_9((&kernelContext_18)->ed_c_0->args_0, &kernelContext_18);
                    uint slot_1 = atomic_fetch_add_explicit((atomic<uint> device*)((ulong)(_S147)) + int(1), 1U, memory_order_relaxed);
                    if(slot_1 < ((&kernelContext_18)->ed_c_0->bins_kind_0))
                    {
                        uint device* _S148 = ed_buf_9((&kernelContext_18)->ed_c_0->visible_0, &kernelContext_18);
                        *(_S148 + slot_1) = i_18 | (cov_0 << 24U);
                        drawn_2 = 1U;
                    }
                    else
                    {
                        drawn_2 = 0U;
                    }
                }
                else
                {
                    drawn_2 = 0U;
                }
                if((((uint4((&_S129->imp_0)->bufs_1) ).w) & 2U) != 0U)
                {
                    vis_1 = ((float4((&_S129->light_0)->cfg_0) ).x) > 0.0f;
                }
                else
                {
                    vis_1 = false;
                }
                if(vis_1)
                {
                    uint _S149 = min(uint((float4((&_S129->light_0)->cfg_0) ).x), 4U);
                    float _S150 = (float4((&_S129->light_0)->cfg_0) ).y;
                    uint device* _S151 = ed_buf_9((&kernelContext_18)->ed_c_0->args_0, &kernelContext_18);
                    atomic<uint> device* _S152 = (atomic<uint> device*)((ulong)(_S151));
                    uint cc_0 = 0U;
                    casters_0 = 0U;
                    for(;;)
                    {
                        if(cc_0 < _S149)
                        {
                        }
                        else
                        {
                            break;
                        }
                        uint cv_0;
                        if(cc_0 >= _S142)
                        {
                            cv_0 = cov_far_0;
                        }
                        else
                        {
                            cv_0 = cov_0;
                        }
                        if(cv_0 == 0U)
                        {
                            cc_0 = cc_0 + 1U;
                            continue;
                        }
                        float4 _S153 = float4((&(&_S129->light_0)->cascade_0)->data_4[cc_0]) ;
                        float4 sp_0 = (((float4(c_4, 1.0f)) * (matrix<float,int(4),int(4)> ((&(&_S129->light_0)->shadow_0)->data_3[cc_0].data_1[int(0)][int(0)], (&(&_S129->light_0)->shadow_0)->data_3[cc_0].data_1[int(1)][int(0)], (&(&_S129->light_0)->shadow_0)->data_3[cc_0].data_1[int(2)][int(0)], (&(&_S129->light_0)->shadow_0)->data_3[cc_0].data_1[int(3)][int(0)], (&(&_S129->light_0)->shadow_0)->data_3[cc_0].data_1[int(0)][int(1)], (&(&_S129->light_0)->shadow_0)->data_3[cc_0].data_1[int(1)][int(1)], (&(&_S129->light_0)->shadow_0)->data_3[cc_0].data_1[int(2)][int(1)], (&(&_S129->light_0)->shadow_0)->data_3[cc_0].data_1[int(3)][int(1)], (&(&_S129->light_0)->shadow_0)->data_3[cc_0].data_1[int(0)][int(2)], (&(&_S129->light_0)->shadow_0)->data_3[cc_0].data_1[int(1)][int(2)], (&(&_S129->light_0)->shadow_0)->data_3[cc_0].data_1[int(2)][int(2)], (&(&_S129->light_0)->shadow_0)->data_3[cc_0].data_1[int(3)][int(2)], (&(&_S129->light_0)->shadow_0)->data_3[cc_0].data_1[int(0)][int(3)], (&(&_S129->light_0)->shadow_0)->data_3[cc_0].data_1[int(1)][int(3)], (&(&_S129->light_0)->shadow_0)->data_3[cc_0].data_1[int(2)][int(3)], (&(&_S129->light_0)->shadow_0)->data_3[cc_0].data_1[int(3)][int(3)]))));
                        float _S154 = _S153.x;
                        float rz_1 = r_4 / _S153.y;
                        float _S155 = 1.0f + r_4 / (_S154 * _S150 * 0.5f);
                        if((abs(sp_0.x)) > _S155)
                        {
                            vis_1 = true;
                        }
                        else
                        {
                            vis_1 = (abs(sp_0.y)) > _S155;
                        }
                        if(vis_1)
                        {
                            _S144 = true;
                        }
                        else
                        {
                            _S144 = (sp_0.z) < (- rz_1);
                        }
                        bool _S156;
                        if(_S144)
                        {
                            _S156 = true;
                        }
                        else
                        {
                            _S156 = (2.0f * r_4) < (2.0f * _S154);
                        }
                        if(_S156)
                        {
                            cc_0 = cc_0 + 1U;
                            continue;
                        }
                        uint _S157 = cc_0 + 1U;
                        uint slot_2 = atomic_fetch_add_explicit(_S152 + (_S157 * 5U + 1U), 1U, memory_order_relaxed);
                        uint casters_1;
                        if(slot_2 < ((&kernelContext_18)->ed_c_0->bins_kind_0))
                        {
                            uint device* _S158 = ed_buf_9((&kernelContext_18)->ed_c_0->visible_0, &kernelContext_18);
                            *(_S158 + (_S157 * (&kernelContext_18)->ed_c_0->bins_kind_0 + slot_2)) = i_18 | (cv_0 << 24U);
                            casters_1 = casters_0 + 1U;
                        }
                        else
                        {
                            casters_1 = casters_0;
                        }
                        casters_0 = casters_1;
                        cc_0 = cc_0 + 1U;
                    }
                }
                else
                {
                    casters_0 = 0U;
                }
            }
            else
            {
                drawn_2 = 0U;
                casters_0 = 0U;
            }
        }
        else
        {
            drawn_2 = 0U;
            casters_0 = 0U;
        }
    }
    else
    {
        drawn_2 = 0U;
        casters_0 = 0U;
    }
    stat_add_0(_S131, 16U, drawn_2);
    stat_add_0(_S131, 17U, casters_0);
    return;
}

[[kernel]] void grass_reset(uint3 tid_7 [[thread_position_in_grid]], CullC_0 constant* ed_c_8 [[buffer(0)]], ulong device* ed_buffers_8 [[buffer(2)]])
{
    thread KernelContext_0 kernelContext_19;
    (&kernelContext_19)->ed_c_0 = ed_c_8;
    (&kernelContext_19)->ed_buffers_0 = ed_buffers_8;
    uint i_19 = tid_7.x;
    if(i_19 < 50U)
    {
        uint w_3 = i_19 % 5U;
        uint list_1 = i_19 / 5U;
        uint b_2 = list_1 % 5U;
        uint v_3;
        if(w_3 == 0U)
        {
            v_3 = GRASS_SLOTS_0[b_2] * GRASS_SEGS_0[b_2] * 6U;
        }
        else
        {
            if(w_3 == 2U)
            {
                v_3 = GRASS_FIRST_0[b_2];
            }
            else
            {
                if(w_3 == 4U)
                {
                    v_3 = list_1 * (&kernelContext_19)->ed_c_0->bins_kind_0;
                }
                else
                {
                    v_3 = 0U;
                }
            }
        }
        uint device* _S159 = ed_buf_9((&kernelContext_19)->ed_c_0->args_0, &kernelContext_19);
        *(_S159 + i_19) = v_3;
    }
    return;
}

float2 grass_lod_0(FrameData_natural_0 device* f_2, float d_0, float blades_0)
{
    float q_0 = (float4((&f_2->grass_0)->lod_0) ).x / max(d_0, 0.0010000000474974513f);
    float raw_0 = blades_0 * min(q_0 * q_0, 1.0f);
    float _S160 = min((float4((&f_2->grass_0)->lod_0) ).z, blades_0);
    return float2(max(raw_0, _S160), min(raw_0 / max(_S160, 0.0010000000474974513f), 1.0f));
}

uint grass_bucket_0(float cnt_0)
{
    uint _S161 = uint(ceil(cnt_0));
    int b_3 = int(4);
    for(;;)
    {
        if(b_3 > int(0))
        {
        }
        else
        {
            break;
        }
        if((GRASS_SLOTS_0[b_3]) >= _S161)
        {
            return uint(b_3);
        }
        b_3 = b_3 - int(1);
    }
    return 0U;
}

void grass_emit_0(uint list_2, uint i_20, KernelContext_0 thread* kernelContext_20)
{
    uint device* _S162 = ed_buf_9(kernelContext_20->ed_c_0->args_0, kernelContext_20);
    uint slot_3 = atomic_fetch_add_explicit((atomic<uint> device*)((ulong)(_S162)) + (list_2 * 5U + 1U), 1U, memory_order_relaxed);
    if(slot_3 < (kernelContext_20->ed_c_0->bins_kind_0))
    {
        uint device* _S163 = ed_buf_9(kernelContext_20->ed_c_0->visible_0, kernelContext_20);
        *(_S163 + (list_2 * kernelContext_20->ed_c_0->bins_kind_0 + slot_3)) = i_20;
    }
    return;
}

[[kernel]] void grass_cull(uint3 tid_8 [[thread_position_in_grid]], CullC_0 constant* ed_c_9 [[buffer(0)]], ulong device* ed_buffers_9 [[buffer(2)]])
{
    thread KernelContext_0 kernelContext_21;
    (&kernelContext_21)->ed_c_0 = ed_c_9;
    (&kernelContext_21)->ed_buffers_0 = ed_buffers_9;
    uint i_21 = tid_8.x;
    FrameData_natural_0 device* _S164 = ed_buf_8(ed_c_9->frame_0, &kernelContext_21);
    uint device* _S165 = ed_buf_9((&kernelContext_21)->ed_c_0->stats_0, &kernelContext_21);
    atomic<uint> device* _S166 = (atomic<uint> device*)((ulong)(_S165));
    uint drawn_3;
    uint casters_2;
    if(i_21 < ((&kernelContext_21)->ed_c_0->count_0))
    {
        GrassClump_0 device* _S167 = ed_buf_1((&kernelContext_21)->ed_c_0->instances_0, &kernelContext_21);
        GrassClump_0 device* _S168 = _S167 + i_21;
        GrassClump_0 g_1 = *_S168;
        uint _S169 = ((*_S168).hw_0) & 65535U;
        if(_S169 != 0U)
        {
            GrassType_natural_0 device* _S170 = ed_buf_0((&kernelContext_21)->ed_c_0->meshes_0, &kernelContext_21);
            GrassType_natural_0 ty_0 = *(_S170 + (((g_1.node_kind_0) >> 12U) & 255U));
            packed_float4 device* _S171 = ed_buf_5((&kernelContext_21)->ed_c_0->nodes_0, &kernelContext_21);
            float4 _S172 = float4(ty_0.a_0) ;
            float h_2 = _S172.y * float(_S169) / 65535.0f * 4.0f;
            float3 base_0 = (float4(*(_S171 + ((g_1.node_kind_0) & 4095U))) ).xyz + float3(g_1.px_0, g_1.py_0, g_1.pz_0);
            float3 c_5 = base_0 + oct_decode_0(g_1.up_0) * float3((0.5f * h_2)) ;
            float r_5 = (float4(ty_0.b_0) ).x * 1.1499999761581421f + 0.60000002384185791f * h_2 + 0.05000000074505806f;
            float d_1 = length(base_0);
            float2 _S173 = grass_lod_0(_S164, d_1, _S172.w);
            float rank_0 = float((g_1.node_kind_0) >> 24U) / 255.0f;
            bool vis_2;
            if(d_1 < ((float4((&_S164->grass_0)->lod_0) ).y))
            {
                vis_2 = rank_0 < (_S173.y);
            }
            else
            {
                vis_2 = false;
            }
            if(vis_2)
            {
                uint b_4 = grass_bucket_0(_S173.x);
                bool _S174 = in_frustum_0(_S164, c_5, r_5);
                if(_S174)
                {
                    vis_2 = ((uint4(_S164->cull_0) ).x) > 0U;
                }
                else
                {
                    vis_2 = false;
                }
                if(vis_2)
                {
                    float device* _S175 = ed_buf_4((&kernelContext_21)->ed_c_0->hzb_0, &kernelContext_21);
                    bool _S176 = occluded_0(_S164, _S175, c_5, r_5);
                    vis_2 = !_S176;
                }
                else
                {
                    vis_2 = _S174;
                }
                if(vis_2)
                {
                    grass_emit_0(b_4, i_21, &kernelContext_21);
                    drawn_3 = 1U;
                }
                else
                {
                    drawn_3 = 0U;
                }
                if((((uint4((&_S164->grass_0)->bufs_2) ).w) & 2U) != 0U)
                {
                    vis_2 = ((float4((&_S164->light_0)->cfg_0) ).x) > 0.0f;
                }
                else
                {
                    vis_2 = false;
                }
                if(vis_2)
                {
                    vis_2 = d_1 < ((float4((&_S164->light_0)->split_0) ).x);
                }
                else
                {
                    vis_2 = false;
                }
                if(vis_2)
                {
                    float4 _S177 = float4((&(&_S164->light_0)->cascade_0)->data_4[int(0)]) ;
                    float4 sp_1 = (((float4(c_5, 1.0f)) * (matrix<float,int(4),int(4)> ((&(&_S164->light_0)->shadow_0)->data_3[int(0)].data_1[int(0)][int(0)], (&(&_S164->light_0)->shadow_0)->data_3[int(0)].data_1[int(1)][int(0)], (&(&_S164->light_0)->shadow_0)->data_3[int(0)].data_1[int(2)][int(0)], (&(&_S164->light_0)->shadow_0)->data_3[int(0)].data_1[int(3)][int(0)], (&(&_S164->light_0)->shadow_0)->data_3[int(0)].data_1[int(0)][int(1)], (&(&_S164->light_0)->shadow_0)->data_3[int(0)].data_1[int(1)][int(1)], (&(&_S164->light_0)->shadow_0)->data_3[int(0)].data_1[int(2)][int(1)], (&(&_S164->light_0)->shadow_0)->data_3[int(0)].data_1[int(3)][int(1)], (&(&_S164->light_0)->shadow_0)->data_3[int(0)].data_1[int(0)][int(2)], (&(&_S164->light_0)->shadow_0)->data_3[int(0)].data_1[int(1)][int(2)], (&(&_S164->light_0)->shadow_0)->data_3[int(0)].data_1[int(2)][int(2)], (&(&_S164->light_0)->shadow_0)->data_3[int(0)].data_1[int(3)][int(2)], (&(&_S164->light_0)->shadow_0)->data_3[int(0)].data_1[int(0)][int(3)], (&(&_S164->light_0)->shadow_0)->data_3[int(0)].data_1[int(1)][int(3)], (&(&_S164->light_0)->shadow_0)->data_3[int(0)].data_1[int(2)][int(3)], (&(&_S164->light_0)->shadow_0)->data_3[int(0)].data_1[int(3)][int(3)]))));
                    float rz_2 = r_5 / _S177.y;
                    float _S178 = 1.0f + r_5 / (_S177.x * (float4((&_S164->light_0)->cfg_0) ).y * 0.5f);
                    if((abs(sp_1.x)) > _S178)
                    {
                        vis_2 = true;
                    }
                    else
                    {
                        vis_2 = (abs(sp_1.y)) > _S178;
                    }
                    if(vis_2)
                    {
                        vis_2 = true;
                    }
                    else
                    {
                        vis_2 = (sp_1.z) < (- rz_2);
                    }
                    if(!vis_2)
                    {
                        grass_emit_0(5U + min(b_4 + 1U, 4U), i_21, &kernelContext_21);
                        casters_2 = 1U;
                    }
                    else
                    {
                        casters_2 = 0U;
                    }
                }
                else
                {
                    casters_2 = 0U;
                }
            }
            else
            {
                drawn_3 = 0U;
                casters_2 = 0U;
            }
        }
        else
        {
            drawn_3 = 0U;
            casters_2 = 0U;
        }
    }
    else
    {
        drawn_3 = 0U;
        casters_2 = 0U;
    }
    stat_add_0(_S166, 18U, drawn_3);
    stat_add_0(_S166, 19U, casters_2);
    return;
}

