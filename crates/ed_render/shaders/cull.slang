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
static const uint STAT_GRASS_BLADES = 15;     // grass blades drawn (clumps x their blade count)
static const uint STAT_IMPOSTORS = 16;        // impostors drawn (trees G5)
static const uint STAT_IMP_CASTERS = 17;      // impostor shadow casters, all cascades
static const uint STAT_GRASS = 18;            // grass clumps drawn (style A)
static const uint STAT_GRASS_CASTERS = 19;    // grass clumps cast into the nearest cascade
static const uint STAT_WORDS = 20;

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
// under SHADOW_MIN_TEXELS texels, picks a stateless LOD from the cascade's texel size and appends to that
// cascade's bin. No per-instance CPU work.
static const float SHADOW_MIN_TEXELS = 12.0;  // casters smaller than this (texels) are dropped
static const float SHADOW_LOD_BIAS[4] = { 8.0, 64.0, 128.0, 128.0 };     // per-cascade shadow LOD error budget, x the view threshold (px of shadow texel)

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
            // Trees (MESH_FOLIAGE): a tree the view has handed to the far rung (its view LOD is
            // the empty last level) casts no shadow either; nearer, the shadow LOD stops at the
            // last drawn level (the shell), so a tree never loses its shadow while it is drawn.
            uint lod_max = 3u;
            uint lod_min = 0u;
            if ((m.flags & MESH_FOLIAGE) != 0u) {
                float4 proj = f.proj;
                float dist = max(length(b.c) - b.r, f.params.z);
                uint vl = select_lod(m, b.scale * proj.w * 0.5 * proj.y / dist, 0u, false, f.params.y, 1.0);
                if (m.index_count[vl] == 0u || (m.lod_count >= 4u && vl + 2u >= m.lod_count)) {
                    nc = 0u;
                }
                lod_max = max(m.lod_count, 2u) - 2u;
                // Impostor shadows on (trees G5): far cascades take every tree's impostor.
                if ((f.imp.bufs.w & 2u) != 0u) {
                    nc = min(nc, uint(f.imp.cfg.w));
                }
                // Leaf shadows come from the simplified level (its cards keep the crown's holes;
                // the full tree's thousands of cards add little a shadow map can resolve).
                // Impostor chains (trees G5: full, simplified, empty) have no shell: the
                // simplified level casts until the tree leaves to its impostor.
                lod_min = min(1u, lod_max);
            }
            // Ground cover (trees G7, `ground_word`): bit 21 casts no shadow, bit 22 casts in
            // the first cascade only (small plants and stones; logs and boulders cast in all).
            if ((inst.node_flags & INSTANCE_GROUND) != 0u) {
                uint g = inst.pad[2];
                nc = ((g >> 21) & 1u) != 0u ? 0u : (((g >> 22) & 1u) != 0u ? min(nc, 1u) : nc);
            }
            for (uint c = 0; c < nc; c++) {
                float4 cs = f.light.cascade[c];
                float4 s = mul(f.light.shadow[c], float4(b.c, 1.0));
                float rc = b.r / (cs.x * tile * 0.5);
                float rz = b.r / cs.y;
                if (abs(s.x) > 1.0 + rc || abs(s.y) > 1.0 + rc || s.z < -rz || 2.0 * b.r < SHADOW_MIN_TEXELS * cs.x) {
                    continue;
                }
                uint lod = clamp(select_lod(m, b.scale / cs.x, 0u, false, f.params.y * SHADOW_LOD_BIAS[c], 1.0), lod_min, lod_max);
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

// ---- Tree impostors (trees G5, src/impostor.rs) ---------------------------------------------
// The CullC words mean: instances = tree buffer, meshes = kind table, args = the impostor draw
// arguments (one DrawIndexedArgs for the view, then one per cascade), visible = the impostor
// visible list (view entries from 0, cascade c's from (c + 1) * bins_kind), count = tree slots,
// bins_kind = visible-list slots per list.
// Entry: tree index (bits 0-23) | coverage (24-31, the fades: 255 = whole tree).
static const uint IMP_INDEX_MASK = 0xFFFFFFu;

[shader("compute")]
[numthreads(64, 1, 1)]
void imp_reset(uint3 tid: SV_DispatchThreadID) {
    uint i = tid.x;
    if (i < 5u * ARGS_WORDS) {
        uint w = i % ARGS_WORDS;
        uint list = i / ARGS_WORDS;
        // index count 6 (one quad), instances 0, first index 0, base vertex 0, first instance.
        ed_buf<uint>(ed_c.args)[i] = w == 0u ? 6u : (w == 4u ? list * ed_c.bins_kind : 0u);
    }
}

float imp_hash01(uint h) {
    return (float(h) + 0.5) / 256.0;
}

[shader("compute")]
[numthreads(64, 1, 1)]
void imp_cull(uint3 tid: SV_DispatchThreadID) {
    uint i = tid.x;
    FrameData* f = ed_buf<FrameData>(ed_c.frame);
    Atomic<uint>* stats = (Atomic<uint>*)ed_buf<uint>(ed_c.stats);
    uint drawn = 0;
    uint casters = 0;
    if (i < ed_c.count) {
        GpuTree g = ed_buf<GpuTree>(ed_c.instances)[i];
        if (g.scale > 0.0) {
            GpuImpKind k = ed_buf<GpuImpKind>(ed_c.meshes)[(g.node_kind >> 12) & 0xFFFu];
            float3 node = ed_buf<float4>(ed_c.nodes)[g.node_kind & 0xFFFu].xyz;
            float3 up = oct_decode(g.up);
            float wd = 0.5 + float(g.yaw_widen >> 16) / 65535.0;
            float3 c = node + float3(g.px, g.py, g.pz) + up * (k.geo.x * g.scale);
            float r = k.geo.y * g.scale * max(wd, 1.0);
            float dist = max(length(c), f.params.z);
            float ppr = f.proj.w * 0.5 * f.proj.y;
            float hpx = k.geo.z * g.scale * ppr / dist;
            float refpx = f.imp.cfg.x * ppr / dist;
            float4 band = f.imp.band;
            // T1 -> T2: the bank mesh leaves at T1 (its empty level); the impostor is whole under
            // band.x and gone over band.y (the mesh still draws there).
            float a_in = saturate((band.y - hpx) / max(band.y - band.x, 1e-3));
            // T2 -> T3: this tree leaves at its own hashed size in band.z..w (the canopy paint
            // under it rises linearly over the same band, so covered area stays constant), with
            // a short dither so it does not blink.
            float thr = band.z + (band.w - band.z) * imp_hash01(g.node_kind >> 24);
            float a_out = saturate((refpx - thr) / max((band.w - band.z) * 0.15, 1e-3));
            float a = a_in * a_out;
            uint cov = uint(a * 255.0 + 0.5);
            // From cascade `cfg.w` on, every tree's sun shadow is its impostor (the meshes cast
            // only in the nearer cascades): coverage there is the T3 exit alone.
            uint cov_far = uint(a_out * 255.0 + 0.5);
            uint far_c = uint(f.imp.cfg.w);
            if (cov_far > 0u) {
                bool vis = cov > 0u && in_frustum(f, c, r);
                if (vis && f.cull.x > 0) {
                    vis = !occluded(f, ed_buf<float>(ed_c.hzb), c, r);
                }
                if (vis) {
                    Atomic<uint>* args = (Atomic<uint>*)ed_buf<uint>(ed_c.args);
                    uint slot = args[1].add(1u, MemoryOrder.Relaxed);
                    if (slot < ed_c.bins_kind) {
                        ed_buf<uint>(ed_c.visible)[slot] = i | (cov << 24);
                        drawn = 1;
                    }
                }
                // Sun shadow casters (IMP_SHADOWS): sun-facing billboards, per cascade.
                if ((f.imp.bufs.w & 2u) != 0u && f.light.cfg.x > 0.0) {
                    uint nc = min(uint(f.light.cfg.x), 4u);
                    float tile = f.light.cfg.y;
                    Atomic<uint>* args = (Atomic<uint>*)ed_buf<uint>(ed_c.args);
                    for (uint cc = 0; cc < nc; cc++) {
                        uint cv = cc >= far_c ? cov_far : cov;
                        if (cv == 0u) {
                            continue;
                        }
                        float4 cs = f.light.cascade[cc];
                        float4 sp = mul(f.light.shadow[cc], float4(c, 1.0));
                        float rc = r / (cs.x * tile * 0.5);
                        float rz = r / cs.y;
                        if (abs(sp.x) > 1.0 + rc || abs(sp.y) > 1.0 + rc || sp.z < -rz || 2.0 * r < 2.0 * cs.x) {
                            continue;
                        }
                        uint slot = args[(cc + 1u) * ARGS_WORDS + 1u].add(1u, MemoryOrder.Relaxed);
                        if (slot < ed_c.bins_kind) {
                            ed_buf<uint>(ed_c.visible)[(cc + 1u) * ed_c.bins_kind + slot] = i | (cv << 24);
                            casters += 1u;
                        }
                    }
                }
            }
        }
    }
    stat_add(stats, STAT_IMPOSTORS, drawn);
    stat_add(stats, STAT_IMP_CASTERS, casters);
}

// ---- Grass style A (src/grass.rs) -----------------------------------------------------------
// CullC words: instances = clump buffer, meshes = type table, args = 10 DrawIndexedArgs (the
// view's five buckets, then the nearest cascade's), visible = the lists (list l from
// l * bins_kind), count = clump slots, bins_kind = slots per list.

[shader("compute")]
[numthreads(64, 1, 1)]
void grass_reset(uint3 tid: SV_DispatchThreadID) {
    uint i = tid.x;
    if (i < 2u * GRASS_BUCKETS * ARGS_WORDS) {
        uint w = i % ARGS_WORDS;
        uint list = i / ARGS_WORDS;
        uint b = list % GRASS_BUCKETS;
        uint v = 0u;
        if (w == 0u) {
            v = GRASS_SLOTS[b] * GRASS_SEGS[b] * 6u;
        } else if (w == 2u) {
            v = GRASS_FIRST[b];
        } else if (w == 4u) {
            v = list * ed_c.bins_kind;
        }
        ed_buf<uint>(ed_c.args)[i] = v;
    }
}

void grass_emit(uint list, uint i) {
    Atomic<uint>* args = (Atomic<uint>*)ed_buf<uint>(ed_c.args);
    uint slot = args[list * ARGS_WORDS + 1u].add(1u, MemoryOrder.Relaxed);
    if (slot < ed_c.bins_kind) {
        ed_buf<uint>(ed_c.visible)[list * ed_c.bins_kind + slot] = i;
    }
}

[shader("compute")]
[numthreads(64, 1, 1)]
void grass_cull(uint3 tid: SV_DispatchThreadID) {
    uint i = tid.x;
    FrameData* f = ed_buf<FrameData>(ed_c.frame);
    Atomic<uint>* stats = (Atomic<uint>*)ed_buf<uint>(ed_c.stats);
    uint drawn = 0;
    uint casters = 0;
    uint blades = 0;
    if (i < ed_c.count) {
        GrassClump g = ed_buf<GrassClump>(ed_c.instances)[i];
        if ((g.hw & 0xFFFFu) != 0u) {
            GrassType ty = ed_buf<GrassType>(ed_c.meshes)[(g.node_kind >> 12) & 0xFFu];
            float3 node = ed_buf<float4>(ed_c.nodes)[g.node_kind & 0xFFFu].xyz;
            float3 up = oct_decode(g.up);
            float h = ty.a.y * float(g.hw & 0xFFFFu) / 65535.0 * 4.0;
            float3 base = node + float3(g.px, g.py, g.pz);
            float d = length(base);
            float2 lod = grass_lod(f, d, ty.a.w);
            h *= grass_grow(f, d);
            float3 c = base + up * (0.5 * h);
            float r = grass_radius(f, ty, lod.y) * 1.15 + 0.6 * h + 0.05;
            float rank = grass_rank(g.node_kind);
            if (d < f.grass.lod.y && rank < lod.y) {
                uint b = grass_bucket(lod.x);
                bool vis = in_frustum(f, c, r);
                if (vis && f.cull.x > 0) {
                    vis = !occluded(f, ed_buf<float>(ed_c.hzb), c, r);
                }
                if (vis) {
                    grass_emit(b, i);
                    drawn = 1;
                    blades = uint(ceil(lod.x));
                }
                // Nearest cascade only: one bucket coarser (half the blades, wider).
                if ((f.grass.bufs.w & 2u) != 0u && f.light.cfg.x > 0.0 && d < (f.grass.far.w > 0.0 ? min(f.light.split.x, f.grass.far.w) : f.light.split.x)) {
                    float tile = f.light.cfg.y;
                    float4 cs = f.light.cascade[0];
                    float4 sp = mul(f.light.shadow[0], float4(c, 1.0));
                    float rc = r / (cs.x * tile * 0.5);
                    float rz = r / cs.y;
                    if (!(abs(sp.x) > 1.0 + rc || abs(sp.y) > 1.0 + rc || sp.z < -rz)) {
                        grass_emit(GRASS_BUCKETS + min(b + 1u, GRASS_BUCKETS - 1u), i);
                        casters = 1;
                    }
                }
            }
        }
    }
    stat_add(stats, STAT_GRASS, drawn);
    stat_add(stats, STAT_GRASS_CASTERS, casters);
    stat_add(stats, STAT_GRASS_BLADES, blades);
}
