// Provider 1 (discrete LOD) culling: one thread per instance slot.
//
// Two-phase occlusion (renderer.md §3, P1-D21):
//   cull_reset  - copy the bin template into the draw arguments (instance counts 0), zero stats
//   cull_early  - LOD selection + cross-fade state for every instance; instances that were
//                 visible last frame and pass the frustum are emitted into the phase-0 bins
//   (raster_early draws them, the depth pyramid is built from that depth)
//   cull_late   - frustum + occlusion against the new pyramid for every instance; the result
//                 is next frame's "visible last frame"; instances that pass and were not drawn
//                 early are emitted into the phase-1 bins (raster_late)
//
// Bins: per phase, [solid bins][dithered bins], each `bins_kind` long; a mesh owns
// `lod_count` consecutive bins starting at `bin_base` in each range. Every bin is a
// DrawIndexedArgs entry whose instances live at visible[first_instance ..].

struct CullC {
    uint frame;
    uint instances;
    uint meshes;
    uint state;
    uint args;
    uint visible;
    uint stats;
    uint hzb;
    uint count;       // instance slots to process
    uint bins_kind;   // bins per kind per phase
    uint templ;       // template arguments (both phases)
    uint nodes;
    uint pad[4];
};

// Stats words (mirror `CullStats` in src/provider.rs).
constant uint STAT_TOTAL = 0;
constant uint STAT_FRUSTUM = 1;
constant uint STAT_OCCLUSION = 2;
constant uint STAT_DRAWN_EARLY = 3;
constant uint STAT_DRAWN_LATE = 4;
constant uint STAT_ENTRIES = 5;
constant uint STAT_NONEMPTY = 6;
constant uint STAT_FADING = 7;
constant uint STAT_TRIANGLES = 8;
constant uint STAT_CHANGED = 9;
constant uint STAT_WORDS = 16;

inline void stat_add(device atomic_uint* stats, uint idx, uint v) {
    uint s = simd_sum(v);
    if (simd_is_first() && s != 0) {
        atomic_fetch_add_explicit(&stats[idx], s, memory_order_relaxed);
    }
}

kernel void cull_reset(uint i [[thread_position_in_grid]], ED_BINDLESS(CullC)) {
    uint n = ed_c.bins_kind * 4 * ARGS_WORDS;
    if (i < n) {
        ed_buf(uint, ed_c.args)[i] = ed_buf(uint, ed_c.templ)[i];
    }
    if (i < STAT_WORDS) {
        ed_buf(uint, ed_c.stats)[i] = 0;
    }
}

inline bool in_frustum(device const FrameData& f, float3 c, float r) {
    for (int k = 0; k < 5; k++) {
        if (dot(f.planes[k].xyz, c) + f.planes[k].w < -r) {
            return false;
        }
    }
    return true;
}

// Coarsest level whose projected error is under the threshold; going coarser needs the
// tighter `thr * hyst` (hysteresis). Mirrors `select_lod` in src/provider.rs.
inline uint select_lod(device const GpuMesh& m, float px_per_m, uint cur, bool init, float thr, float hyst) {
    uint n = clamp(m.lod_count, 1u, 4u);
    uint want = 0;
    for (uint l = 1; l < n; l++) {
        if (m.error[l] * px_per_m <= thr) {
            want = l;
        }
    }
    if (init && want > cur) {
        uint w = cur;
        for (uint l = cur + 1; l < n; l++) {
            if (m.error[l] * px_per_m <= thr * hyst) {
                w = l;
            }
        }
        return w;
    }
    return want;
}

struct Bounds {
    float3 c;    // camera-relative centre
    float r;
    float scale;
};

inline Bounds bounds(constant CullC& c, constant EdBuf* bufs, thread const GpuInstance& inst, device const GpuMesh& m) {
    device const float4* nodes = reinterpret_cast<device const float4*>(bufs[c.nodes].p);
    Bounds b;
    b.scale = max_scale(inst);
    b.c = nodes[inst.node_flags & 0xFFFFu].xyz + xform_point(inst, m.sphere.xyz);
    b.r = m.sphere.w * b.scale;
    return b;
}

struct Emitted {
    uint entries;
    uint nonempty;
    uint tris;
};

inline void push(device atomic_uint* args, device uint* args_plain, device uint* visible, uint bin, uint entry, uint cap,
                 uint tris, thread Emitted& e) {
    uint slot = atomic_fetch_add_explicit(&args[bin * ARGS_WORDS + 1], 1u, memory_order_relaxed);
    if (slot >= cap) {
        // Bin full (layout lags one frame behind instance growth): undo, drop the entry.
        atomic_fetch_sub_explicit(&args[bin * ARGS_WORDS + 1], 1u, memory_order_relaxed);
        return;
    }
    if (slot == 0) {
        e.nonempty += 1;
    }
    visible[args_plain[bin * ARGS_WORDS + 4] + slot] = entry;
    e.entries += 1;
    e.tris += tris;
}

// Emit one instance into the bins of `phase`: one solid entry, or two dithered entries with
// complementary masks while a LOD cross-fade runs.
inline void emit(constant CullC& c, constant EdBuf* bufs, device const GpuMesh& m, uint inst, uint st, uint phase, uint fade_frames,
                 thread Emitted& e) {
    device atomic_uint* args = reinterpret_cast<device atomic_uint*>(bufs[c.args].p);
    device uint* args_plain = reinterpret_cast<device uint*>(bufs[c.args].p);
    device uint* visible = reinterpret_cast<device uint*>(bufs[c.visible].p);
    uint cur = st & 3u;
    uint prev = (st >> 2) & 3u;
    uint fade = (st >> ST_FADE_SHIFT) & 63u;
    uint phase_base = phase * 2u * c.bins_kind + m.bin_base;
    if (fade == 0) {
        push(args, args_plain, visible, phase_base + cur, inst | (cur << 20), m.capacity, m.index_count[cur] / 3u, e);
    } else {
        // `cur` covers (N - fade + 1) / (N + 1) of the pixels, `prev` the rest.
        uint cov = clamp(((fade_frames - fade + 1u) * 256u) / (fade_frames + 1u), 1u, 255u);
        uint dith = phase_base + c.bins_kind;
        push(args, args_plain, visible, dith + cur, inst | (cur << 20) | (cov << 22), m.capacity, m.index_count[cur] / 3u, e);
        push(args, args_plain, visible, dith + prev, inst | (prev << 20) | (cov << 22) | (1u << 30), m.capacity, m.index_count[prev] / 3u, e);
    }
}

kernel void cull_early(uint i [[thread_position_in_grid]], ED_BINDLESS(CullC)) {
    device const FrameData& f = *ed_buf(FrameData, ed_c.frame);
    device atomic_uint* stats = ed_buf(atomic_uint, ed_c.stats);
    Emitted e = { 0, 0, 0 };
    uint drawn = 0;
    if (i < ed_c.count) {
        device uint* state = ed_buf(uint, ed_c.state);
        GpuInstance inst = ed_buf(GpuInstance, ed_c.instances)[i];
        if ((inst.node_flags & INSTANCE_ALIVE) == 0) {
            state[i] = 0;
        } else {
            device const GpuMesh& m = ed_buf(GpuMesh, ed_c.meshes)[inst.mesh];
            Bounds b = bounds(ed_c, ed_buffers, inst, m);
            bool inf = in_frustum(f, b.c, b.r);
            uint st = state[i];
            bool init = (st & ST_INIT) != 0;
            uint cur = st & 3u;
            uint prev = (st >> 2) & 3u;
            uint fade = (st >> ST_FADE_SHIFT) & 63u;
            uint n = f.cull.y;
            float dist = max(length(b.c) - b.r, f.params.z);
            float px_per_m = b.scale * f.proj.w * 0.5 * f.proj.y / dist;
            uint want = select_lod(m, px_per_m, cur, init, f.params.y, f.params.w);
            bool changed = false;
            if (fade > 0) {
                fade -= 1;
            }
            if (!init || !inf || n == 0) {
                // First sight, off screen, or fades off: switch at once.
                changed = init && inf && want != cur;
                cur = want;
                fade = 0;
            } else if (fade == 0 && want != cur) {
                prev = cur;
                cur = want;
                fade = n;
                changed = true;
            }
            if (fade == 0) {
                prev = cur;
            }
            st = cur | (prev << 2) | (fade << ST_FADE_SHIFT) | (st & ST_VISIBLE) | ST_INIT | (changed ? ST_CHANGED : 0u);
            state[i] = st;
            if (inf && (st & ST_VISIBLE) != 0) {
                emit(ed_c, ed_buffers, m, i, st, 0, n, e);
                drawn = 1;
            }
        }
    }
    stat_add(stats, STAT_DRAWN_EARLY, drawn);
    stat_add(stats, STAT_ENTRIES, e.entries);
    stat_add(stats, STAT_NONEMPTY, e.nonempty);
    stat_add(stats, STAT_TRIANGLES, e.tris);
}

// Screen rectangle (uv, y down) of a view-space sphere (x right, y up, z forward), after
// Mara and McGuire 2013 (as in zeux/niagara). False if it crosses the near plane.
inline bool project_sphere(float3 c, float r, float znear, float p00, float p11, thread float4& aabb) {
    if (c.z < r + znear) {
        return false;
    }
    float3 cr = c * r;
    float czr2 = c.z * c.z - r * r;
    float vx = sqrt(c.x * c.x + czr2);
    float minx = (vx * c.x - cr.z) / (vx * c.z + cr.x);
    float maxx = (vx * c.x + cr.z) / (vx * c.z - cr.x);
    float vy = sqrt(c.y * c.y + czr2);
    float miny = (vy * c.y - cr.z) / (vy * c.z + cr.y);
    float maxy = (vy * c.y + cr.z) / (vy * c.z - cr.y);
    aabb = float4(minx * p00, miny * p11, maxx * p00, maxy * p11);
    aabb = aabb.xwzy * float4(0.5, -0.5, 0.5, -0.5) + float4(0.5);
    return true;
}

// Depth pyramid test: occluded when the sphere's nearest depth is farther (smaller, reverse-Z)
// than the farthest depth stored over its screen rectangle.
inline bool occluded(device const FrameData& f, device const float* hzb, float3 c_rel, float r) {
    float3 v = (f.view * float4(c_rel, 1.0)).xyz;
    float3 cv = float3(v.x, v.y, -v.z);
    float znear = f.params.z;
    float4 aabb;
    if (!project_sphere(cv, r, znear, f.proj.x, f.proj.y, aabb)) {
        return false;
    }
    float w = f.proj.z;
    float h = f.proj.w;
    float x0 = clamp(aabb.x, 0.0, 1.0) * w;
    float y0 = clamp(aabb.y, 0.0, 1.0) * h;
    float x1 = clamp(aabb.z, 0.0, 1.0) * w;
    float y1 = clamp(aabb.w, 0.0, 1.0) * h;
    float size = max(x1 - x0, y1 - y0);
    int levels = int(f.cull.x);
    int l = size <= 1.0 ? 0 : int(ceil(log2(size))) - 1;
    if (l > levels - 1) {
        // Wider than the coarsest level's 2x2 footprint: four samples are not conservative.
        return false;
    }
    l = max(l, 0);
    HzbLevel lv = f.hzb[l];
    float ts = float(2u << uint(l));
    uint tx0 = min(uint(x0 / ts), lv.width - 1);
    uint ty0 = min(uint(y0 / ts), lv.height - 1);
    uint tx1 = min(uint(x1 / ts), lv.width - 1);
    uint ty1 = min(uint(y1 / ts), lv.height - 1);
    device const float* p = hzb + lv.offset;
    float d = min(min(p[ty0 * lv.width + tx0], p[ty0 * lv.width + tx1]), min(p[ty1 * lv.width + tx0], p[ty1 * lv.width + tx1]));
    float nearest = znear / (cv.z - r);
    return nearest < d;
}

kernel void cull_late(uint i [[thread_position_in_grid]], ED_BINDLESS(CullC)) {
    device const FrameData& f = *ed_buf(FrameData, ed_c.frame);
    device atomic_uint* stats = ed_buf(atomic_uint, ed_c.stats);
    Emitted e = { 0, 0, 0 };
    uint total = 0, fr = 0, occ = 0, drawn = 0, fading = 0, changed = 0;
    if (i < ed_c.count) {
        GpuInstance inst = ed_buf(GpuInstance, ed_c.instances)[i];
        if ((inst.node_flags & INSTANCE_ALIVE) != 0) {
            device uint* state = ed_buf(uint, ed_c.state);
            device const GpuMesh& m = ed_buf(GpuMesh, ed_c.meshes)[inst.mesh];
            Bounds b = bounds(ed_c, ed_buffers, inst, m);
            uint st = state[i];
            total = 1;
            bool inf = in_frustum(f, b.c, b.r);
            bool vis = inf;
            if (inf && f.cull.x > 0) {
                vis = !occluded(f, ed_buf(float, ed_c.hzb), b.c, b.r);
            }
            fr = inf ? 1 : 0;
            occ = vis ? 1 : 0;
            bool early = inf && (st & ST_VISIBLE) != 0;
            if (vis && !early) {
                emit(ed_c, ed_buffers, m, i, st, 1, f.cull.y, e);
                drawn = 1;
            }
            if (early || drawn != 0) {
                fading = ((st >> ST_FADE_SHIFT) & 63u) != 0 ? 1 : 0;
                changed = (st & ST_CHANGED) != 0 ? 1 : 0;
            }
            state[i] = (st & ~ST_VISIBLE) | (vis ? ST_VISIBLE : 0u);
        }
    }
    stat_add(stats, STAT_TOTAL, total);
    stat_add(stats, STAT_FRUSTUM, fr);
    stat_add(stats, STAT_OCCLUSION, occ);
    stat_add(stats, STAT_DRAWN_LATE, drawn);
    stat_add(stats, STAT_ENTRIES, e.entries);
    stat_add(stats, STAT_NONEMPTY, e.nonempty);
    stat_add(stats, STAT_TRIANGLES, e.tris);
    stat_add(stats, STAT_FADING, fading);
    stat_add(stats, STAT_CHANGED, changed);
}
