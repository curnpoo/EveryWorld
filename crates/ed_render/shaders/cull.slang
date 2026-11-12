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
#include "shader_abi.slang"
#include "common.slang"

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
ED_CONSTANTS(CullC)

// Stats words (mirror `CullStats` in src/provider.rs).
static const uint STAT_TOTAL = 0;
static const uint STAT_FRUSTUM = 1;
static const uint STAT_OCCLUSION = 2;
static const uint STAT_DRAWN_EARLY = 3;
static const uint STAT_DRAWN_LATE = 4;
static const uint STAT_ENTRIES = 5;
static const uint STAT_NONEMPTY = 6;
static const uint STAT_FADING = 7;
static const uint STAT_TRIANGLES = 8;
static const uint STAT_CHANGED = 9;
static const uint STAT_SHADOW = 10;          // 10..13: shadow casters per cascade
static const uint STAT_SHADOW_TRIANGLES = 14;
static const uint STAT_WORDS = 16;

// One atomic per wave: every thread of the group reaches the stat_add calls (no early return).
void stat_add(Atomic<uint>* stats, uint idx, uint v) {
    uint s = WaveActiveSum(v);
    if (WaveIsFirstLane() && s != 0) {
        stats[idx].add(s, MemoryOrder.Relaxed);
    }
}

// CULL_GROUP in src/provider.rs.
[shader("compute")]
[numthreads(64, 1, 1)]
void cull_reset(uint3 tid: SV_DispatchThreadID) {
    uint i = tid.x;
    uint n = ed_c.bins_kind * 4 * ARGS_WORDS;
    if (i < n) {
        ed_buf<uint>(ed_c.args)[i] = ed_buf<uint>(ed_c.templ)[i];
    }
    if (i < STAT_WORDS) {
        ed_buf<uint>(ed_c.stats)[i] = 0;
    }
}

bool in_frustum(FrameData* f, float3 c, float r) {
    for (int k = 0; k < 5; k++) {
        float4 pl = f.planes[k];
        if (dot(pl.xyz, c) + pl.w < -r) {
            return false;
        }
    }
    return true;
}

// Coarsest level whose projected error is under the threshold; going coarser needs the
// tighter `thr * hyst` (hysteresis). Mirrors `select_lod` in src/provider.rs.
uint select_lod(GpuMesh* m, float px_per_m, uint cur, bool init, float thr, float hyst) {
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

Bounds bounds(GpuInstance inst, GpuMesh* m) {
    float4* nodes = ed_buf<float4>(ed_c.nodes);
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

void push(Atomic<uint>* args, uint* args_plain, uint* visible, uint bin, uint entry, uint cap, uint tris, inout Emitted e) {
    uint slot = args[bin * ARGS_WORDS + 1].add(1u, MemoryOrder.Relaxed);
    if (slot >= cap) {
        // Bin full (layout lags one frame behind instance growth): undo, drop the entry.
        args[bin * ARGS_WORDS + 1].sub(1u, MemoryOrder.Relaxed);
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
void emit(GpuMesh* m, uint inst, uint st, uint phase, uint fade_frames, inout Emitted e) {
    uint* args_plain = ed_buf<uint>(ed_c.args);
    Atomic<uint>* args = (Atomic<uint>*)args_plain;
    uint* visible = ed_buf<uint>(ed_c.visible);
    uint cur = st & 3u;
    uint prev = (st >> 2) & 3u;
    uint fade = (st >> ST_FADE_SHIFT) & 63u;
    uint phase_base = phase * 2u * ed_c.bins_kind + m.bin_base;
    uint cap = m.capacity;
    if (fade == 0) {
        push(args, args_plain, visible, phase_base + cur, inst | (cur << 20), cap, m.index_count[cur] / 3u, e);
    } else {
        // `cur` covers (N - fade + 1) / (N + 1) of the pixels, `prev` the rest.
        uint cov = clamp(((fade_frames - fade + 1u) * 256u) / (fade_frames + 1u), 1u, 255u);
        uint dith = phase_base + ed_c.bins_kind;
        push(args, args_plain, visible, dith + cur, inst | (cur << 20) | (cov << 22), cap, m.index_count[cur] / 3u, e);
        push(args, args_plain, visible, dith + prev, inst | (prev << 20) | (cov << 22) | (1u << 30), cap, m.index_count[prev] / 3u, e);
    }
}

[shader("compute")]
[numthreads(64, 1, 1)]
void cull_early(uint3 tid: SV_DispatchThreadID) {
    uint i = tid.x;
    FrameData* f = ed_buf<FrameData>(ed_c.frame);
    Atomic<uint>* stats = (Atomic<uint>*)ed_buf<uint>(ed_c.stats);
    Emitted e = { 0, 0, 0 };
    uint drawn = 0;
    if (i < ed_c.count) {
        uint* state = ed_buf<uint>(ed_c.state);
        GpuInstance inst = ed_buf<GpuInstance>(ed_c.instances)[i];
        if ((inst.node_flags & INSTANCE_ALIVE) == 0) {
            state[i] = 0;
        } else {
            GpuMesh* m = ed_buf<GpuMesh>(ed_c.meshes) + inst.mesh;
            Bounds b = bounds(inst, m);
            bool inf = in_frustum(f, b.c, b.r);
            uint st = state[i];
            bool init = (st & ST_INIT) != 0;
            uint cur = st & 3u;
            uint prev = (st >> 2) & 3u;
            uint fade = (st >> ST_FADE_SHIFT) & 63u;
            uint n = f.cull.y;
            float4 params = f.params;
            float4 proj = f.proj;
            float dist = max(length(b.c) - b.r, params.z);
            float px_per_m = b.scale * proj.w * 0.5 * proj.y / dist;
            uint want = select_lod(m, px_per_m, cur, init, params.y, params.w);
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
                emit(m, i, st, 0, n, e);
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
bool project_sphere(float3 c, float r, float znear, float p00, float p11, out float4 aabb) {
    aabb = float4(0.0);
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
bool occluded(FrameData* f, float* hzb, float3 c_rel, float r) {
    float3 v = mul(f.view, float4(c_rel, 1.0)).xyz;
    float3 cv = float3(v.x, v.y, -v.z);
    float4 params = f.params;
    float4 proj = f.proj;
    float znear = params.z;
    float4 aabb;
    if (!project_sphere(cv, r, znear, proj.x, proj.y, aabb)) {
        return false;
    }
    float w = proj.z;
    float h = proj.w;
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
    float* p = hzb + lv.offset;
    float d = min(min(p[ty0 * lv.width + tx0], p[ty0 * lv.width + tx1]), min(p[ty1 * lv.width + tx0], p[ty1 * lv.width + tx1]));
    float nearest = znear / (cv.z - r);
    return nearest < d;
}

[shader("compute")]
[numthreads(64, 1, 1)]
void cull_late(uint3 tid: SV_DispatchThreadID) {
    uint i = tid.x;
    FrameData* f = ed_buf<FrameData>(ed_c.frame);
    Atomic<uint>* stats = (Atomic<uint>*)ed_buf<uint>(ed_c.stats);
    Emitted e = { 0, 0, 0 };
    uint total = 0, fr = 0, occ = 0, drawn = 0, fading = 0, changed = 0;
    if (i < ed_c.count) {
        GpuInstance inst = ed_buf<GpuInstance>(ed_c.instances)[i];
        if ((inst.node_flags & INSTANCE_ALIVE) != 0) {
            uint* state = ed_buf<uint>(ed_c.state);
            GpuMesh* m = ed_buf<GpuMesh>(ed_c.meshes) + inst.mesh;
            Bounds b = bounds(inst, m);
            uint st = state[i];
            total = 1;
            bool inf = in_frustum(f, b.c, b.r);
            bool vis = inf;
            if (inf && f.cull.x > 0) {
                vis = !occluded(f, ed_buf<float>(ed_c.hzb), b.c, b.r);
            }
            fr = inf ? 1 : 0;
            occ = vis ? 1 : 0;
            bool early = inf && (st & ST_VISIBLE) != 0;
            if (vis && !early) {
                emit(m, i, st, 1, f.cull.y, e);
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

// Sun shadow casters (M4d, P1-D20): the same bins and template as the main view, one range per
// cascade (range c = cascade c). shadow_reset copies the template into the shadow draw
// arguments; shadow_cull tests every live instance's bounding sphere against each cascade's
// light-space box (casters towards the sun are kept: the raster clamps them), drops casters
// under 0.75 texel, picks a stateless LOD from the cascade's texel size and appends to that
// cascade's bin. No per-instance CPU work.
[shader("compute")]
[numthreads(64, 1, 1)]
void shadow_reset(uint3 tid: SV_DispatchThreadID) {
    uint i = tid.x;
    if (i < ed_c.bins_kind * 4 * ARGS_WORDS) {
        ed_buf<uint>(ed_c.args)[i] = ed_buf<uint>(ed_c.templ)[i];
    }
}

[shader("compute")]
[numthreads(64, 1, 1)]
void shadow_cull(uint3 tid: SV_DispatchThreadID) {
    uint i = tid.x;
    FrameData* f = ed_buf<FrameData>(ed_c.frame);
    Atomic<uint>* stats = (Atomic<uint>*)ed_buf<uint>(ed_c.stats);
    Emitted e = { 0, 0, 0 };
    uint cnt0 = 0, cnt1 = 0, cnt2 = 0, cnt3 = 0;
    if (i < ed_c.count) {
        GpuInstance inst = ed_buf<GpuInstance>(ed_c.instances)[i];
        if ((inst.node_flags & INSTANCE_ALIVE) != 0) {
            uint* args_plain = ed_buf<uint>(ed_c.args);
            Atomic<uint>* args = (Atomic<uint>*)args_plain;
            uint* visible = ed_buf<uint>(ed_c.visible);
            GpuMesh* m = ed_buf<GpuMesh>(ed_c.meshes) + inst.mesh;
            Bounds b = bounds(inst, m);
            uint nc = min(uint(f.light.cfg.x), 4u);
            float tile = f.light.cfg.y;
            for (uint c = 0; c < nc; c++) {
                float4 cs = f.light.cascade[c];
                float4 s = mul(f.light.shadow[c], float4(b.c, 1.0));
                float rc = b.r / (cs.x * tile * 0.5);
                float rz = b.r / cs.y;
                if (abs(s.x) > 1.0 + rc || abs(s.y) > 1.0 + rc || s.z < -rz || 2.0 * b.r < 0.75 * cs.x) {
                    continue;
                }
                uint lod = select_lod(m, b.scale / cs.x, 0u, false, f.params.y, 1.0);
                push(args, args_plain, visible, c * ed_c.bins_kind + m.bin_base + lod, i | (lod << 20), m.capacity, m.index_count[lod] / 3u, e);
                cnt0 += c == 0 ? 1u : 0u;
                cnt1 += c == 1 ? 1u : 0u;
                cnt2 += c == 2 ? 1u : 0u;
                cnt3 += c == 3 ? 1u : 0u;
            }
        }
    }
    stat_add(stats, STAT_SHADOW, cnt0);
    stat_add(stats, STAT_SHADOW + 1, cnt1);
    stat_add(stats, STAT_SHADOW + 2, cnt2);
    stat_add(stats, STAT_SHADOW + 3, cnt3);
    stat_add(stats, STAT_SHADOW_TRIANGLES, e.tris);
}
