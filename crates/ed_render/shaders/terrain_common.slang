// Terrain provider (Phase 2 W3, P2-D1/D2/D7): one shared 65 x 65 node grid plus a skirt ring,
// displaced on the GPU from height pages. Included by terrain.slang and, for the terrain
// branch of the visibility resolve, by resolve.slang (after shader_abi.slang and
// common.slang). Layouts mirror crates/ed_render/src/terrain.rs (`DRAW_WORDS`,
// `PAGE_WORDS`); change both together.
//
// Positions follow ed_planet's surface rule exactly: a lattice direction `dir` on the
// ellipsoid, displaced radially, `dir * (radius(dir) + h)`. Everything is computed relative to
// the node origin (`dir_c * r_c`, f64 on the CPU) with difference formulas, so no f32 value
// ever holds a planet-sized coordinate; the CPU adds `origin - camera` (f64, then f32).
#pragma once

// Visibility word x of a terrain pixel: this bit plus the draw index (instance words are
// `instance + 1`, never this large). Word y is the primitive (no LOD bits).
static const uint TERRAIN_VIS_BIT = 0x80000000u;

static const uint T_CELLS = 64;                 // grid cells per node side
static const uint T_SIDE = T_CELLS + 1;         // grid vertices per side
static const uint T_GRID_VERTS = T_SIDE * T_SIDE;
static const uint T_RING = 4 * T_CELLS;          // skirt vertices (one per edge vertex)
static const uint T_GRID_TRIS = T_CELLS * T_CELLS * 2;
static const uint T_PAGE_N = 257;               // height page samples per side
static const uint T_PAGE_STRIDE = 4;            // page samples per grid cell
static const uint T_PAGE_WORDS = 33025;          // u32 words per pool slot (257 * 257 u16, padded)
static const uint T_NO_SLOT = 0xFFFFFFFFu;

// Flags word (TerrainDraw.slots.z).
static const uint T_COARSE_MASK = 0xFu;         // bit 0: i = 0 edge, 1: i = 64, 2: j = 0, 3: j = 64
static const uint T_QX = 1u << 4;               // child quadrant in the parent (x)
static const uint T_QY = 1u << 5;               // child quadrant in the parent (y)
static const uint T_SKIRT = 1u << 6;            // draw the skirt (else it collapses to the edge)
static const uint T_LOD_SHIFT = 8;              // bits 8-9: index LOD (stride 1 << lod)
static const uint T_EDGE_SHIFT = 10;            // bits 10-21: per edge, 3 bits log2 of the neighbour edge's spacing
static const uint T_NOMORPH = 1u << 22;         // no vertex of the node morphs
static const uint T_TREES = 1u << 23;           // far forest rung on (trees G3): forest from the canopy page
static const uint T_TREES_DEBUG = 1u << 24;     // EW_TREES_FAR=debug tint
static const uint T_GROUND = 1u << 25;          // ground page (own or parent, grass G4): carpet from the shares
static const uint T_PRIM_LOD_SHIFT = 24;        // the visibility primitive carries the LOD here

// One drawn node (208 bytes, `DRAW_WORDS` = 52). World axes, camera-relative.
struct TerrainDraw {
    float4 origin;  // xyz: node origin (ellipsoid point of the centre direction) - camera, w: skirt depth (m)
    float4 dir_c;   // xyz: centre direction, w: ellipsoid radius along it (m)
    float4 cc;      // xyz: cube point of the centre (F_n + T_u F_u + T_v F_v), w: its length
    float4 fu;      // xyz: face u axis, w: T_u = tan(centre angle u)
    float4 fv;      // xyz: face v axis, w: T_v
    float4 shape;   // x: angle per grid cell, y: equatorial radius, z: 1 - a^2/b^2, w: sqrt(1 - z dz_c^2)
    float4 morph;   // x, y: own morph start, end (m); z, w: the coarser level's (edge stitching)
    float4 polar;   // xyz: body polar axis (+Z body-fixed), w: canopy lift scale (TREE_REF_M x pixels per radian; 0: no lift)
    uint4 slots;    // x: page slot, y: parent page slot (T_NO_SLOT: none), z: flags, w: level
    float4 hq;      // x, y: page scale, offset; z, w: parent page scale, offset
    float4 tex;     // x, y: node centre face angles times the equatorial radius, mod 4096 m;
                    // z, w (bits): own and parent canopy page word offsets in the pool (0: none)
    uint4 gnd;      // x, y: own and parent ground page word offsets in the pool (0: none)
    uint4 clim;     // corner climate (NodeId::corner order): aridity low 16 bits, temperature high (terrain.rs clim_word)
};

// The terrain material record (P-slice S7, `MAT_WORDS` = 52 words, at index
// `ResolveC.t_mat` of the draw table): two RGBA8 texture arrays, a layer per material slot
// (terrain.rs `MAT_LAYERS`: rock, scree, grass, forest, snow, soil).
struct TerrainMat {
    uint albedo;    // bindless Texture2DArray: rgb base colour (sRGB), a displacement
    uint normal;    // bindless Texture2DArray: rg normal xy (OpenGL), b roughness, a AO
    uint layers;
    uint size;
    float tile[8];  // metres per texture repeat, per layer
    float4 gain[6]; // rgb: per-layer albedo calibration (target / texture mean, linear)
    float4 pad[4];
};

// Raw height of page sample (i, j) in pool slot `slot`.
float t_page(uint pool, uint slot, uint i, uint j, float scale, float offset) {
    uint s = j * T_PAGE_N + i;
    uint w = ed_buf<uint>(pool)[slot * T_PAGE_WORDS + (s >> 1)];
    uint q = (s & 1u) != 0 ? (w >> 16) : (w & 0xFFFFu);
    return offset + scale * float(q);
}

float t_own_h(uint pool, TerrainDraw d, uint i, uint j) {
    return t_page(pool, d.slots.x, i * T_PAGE_STRIDE, j * T_PAGE_STRIDE, d.hq.x, d.hq.y);
}

// ---------------------------------------------------------------------------------------------
// Canopy pages (trees G3): 257 x 257 packed [cover, height code, mix, under] (RGBA8 in a u32,
// ed_geo::CanopyPage) per pool slot, after every slot's height words; `off` is the page's first
// word (0: no page, no forest).
// ---------------------------------------------------------------------------------------------

// Lift ramp, mirrored in terrain.rs: none while the reference tree is over 12 px, full under 6.
static const float T_LIFT_NONE_PX = 12.0;
static const float T_LIFT_FULL_PX = 6.0;
static const float T_CANOPY_HMIN = 0.5;           // ed_geo canopy_page HEIGHT_MIN_M
static const float T_CANOPY_LNR = 0.0204447;      // ln(90 / 0.5) / 254

uint t_canopy_raw(uint pool, uint off, uint i, uint j) {
    return ed_buf<uint>(pool)[off + j * T_PAGE_N + i];
}

float t_canopy_h(uint code) {
    return code == 0u ? 0.0 : T_CANOPY_HMIN * exp(float(code - 1u) * T_CANOPY_LNR);
}

// Lift (m) of one canopy sample: the canopy top, cover-weighted with a steep ramp so a stand's
// edge is a soft wall rather than a ramp from zero.
float t_canopy_lift(uint pool, uint off, uint i, uint j) {
    if (off == 0u) {
        return 0.0;
    }
    uint c = t_canopy_raw(pool, off, i, j);
    float cover = float(c & 0xFFu) / 255.0;
    return t_canopy_h((c >> 8) & 0xFFu) * smoothstep(0.06, 0.45, cover);
}

// How much of the lift a node-relative point gets (by the reference tree's projected size).
float t_lift_k(TerrainDraw d, float3 p) {
    if (d.polar.w <= 0.0) {
        return 0.0;
    }
    float px = d.polar.w / max(length(d.origin.xyz + p), 1.0);
    // Tree impostors on (trees G5): the ramp from their one table (`FrameData.imp.far`), so
    // the lift rises only once the impostors have left.
    float2 r = float2(T_LIFT_NONE_PX, T_LIFT_FULL_PX);
    FrameData* f = ed_buf<FrameData>(ed_c.frame);
    if (f.imp.far.x > 0.0) {
        r = f.imp.far.xy;
    }
    return smoothstep(r.x, r.y, px);
}

// Own canopy lift at grid vertex (i, j), already scaled by the ramp at node-relative `p`.
float t_own_lift(uint pool, TerrainDraw d, uint i, uint j, float3 p) {
    if (d.polar.w <= 0.0) {
        return 0.0;
    }
    float k = t_lift_k(d, p);
    return k > 0.0 ? k * t_canopy_lift(pool, asuint(d.tex.z), i * T_PAGE_STRIDE, j * T_PAGE_STRIDE) : 0.0;
}

// Lattice point (i, j) at height h, relative to the node origin. Returns the direction too.
float3 t_lattice(TerrainDraw d, uint i, uint j, float h, out float3 dir) {
    // Offsets from the centre along the face: u = tan(a_c + k D) = (T + t) / (1 - T t), so
    // u - T = t (1 + T^2) / (1 - T t) with t = tan(k D) small and exact near the centre.
    float tu = tan((float(i) - 32.0) * d.shape.x);
    float tv = tan((float(j) - 32.0) * d.shape.x);
    float du = tu * (1.0 + d.fu.w * d.fu.w) / (1.0 - d.fu.w * tu);
    float dv = tv * (1.0 + d.fv.w * d.fv.w) / (1.0 - d.fv.w * tv);
    float3 delta = du * d.fu.xyz + dv * d.fv.xyz;
    float lc = d.cc.w;
    float k = 2.0 * dot(d.cc.xyz, delta) + dot(delta, delta);   // |c|^2 - |c_c|^2
    float l = sqrt(lc * lc + k);
    // dir - dir_c = delta / l + c_c (1 / l - 1 / l_c), with l_c - l = -k / (l_c + l).
    float3 dd = delta / l - d.cc.xyz * (k / ((lc + l) * l * lc));
    dir = d.dir_c.xyz + dd;
    // Ellipsoid radius a / sqrt(1 - e dz^2) as a difference from the centre's.
    float dzc = dot(d.dir_c.xyz, d.polar.xyz);
    float ddz = dot(dd, d.polar.xyz);
    float dz = dzc + ddz;
    float e = d.shape.z;
    float s = sqrt(1.0 - e * dz * dz);
    float sc = d.shape.w;
    float dr = d.shape.y * e * ddz * (dz + dzc) / (s * sc * (s + sc));
    float r = d.dir_c.w + dr;
    return dd * r + d.dir_c.xyz * dr + dir * h;
}

// Unmorphed position of grid vertex (i, j) (own heights), node-relative.
float3 t_base<let L : int>(uint pool, TerrainDraw d, uint i, uint j) {
    float3 dir;
    float3 p = t_lattice(d, i, j, t_own_h(pool, d, i, j), dir);
    if (L != 0 && d.polar.w > 0.0) {
        p += dir * t_own_lift(pool, d, i, j, p);  // canopy lift (render only, trees G3)
    }
    return p;
}

// Morph target of an even grid vertex: the parent's height at the same lattice point. Border
// vertices keep their own height (it equals the neighbours' and the coarser level's: G4).
float3 t_target_even<let L : int>(uint pool, TerrainDraw d, uint i, uint j) {
    bool border = i == 0 || j == 0 || i == T_CELLS || j == T_CELLS;
    float h;
    bool own = border || d.slots.y == T_NO_SLOT;
    uint qx = (d.slots.z & T_QX) != 0 ? 128u : 0u;
    uint qy = (d.slots.z & T_QY) != 0 ? 128u : 0u;
    if (own) {
        h = t_own_h(pool, d, i, j);
    } else {
        h = t_page(pool, d.slots.y, qx + 2u * i, qy + 2u * j, d.hq.z, d.hq.w);
    }
    float3 dir;
    float3 p = t_lattice(d, i, j, h, dir);
    if (L != 0 && d.polar.w > 0.0) {
        // The parent's lift at the same lattice point (its own canopy page), so a full morph is
        // the parent's lifted mesh; without a parent canopy page, our own.
        float k = t_lift_k(d, p);
        uint poff = asuint(d.tex.w);
        float lift = 0.0;
        if (k > 0.0) {
            lift = own || poff == 0u ? t_canopy_lift(pool, asuint(d.tex.z), i * T_PAGE_STRIDE, j * T_PAGE_STRIDE) : t_canopy_lift(pool, poff, qx + 2u * i, qy + 2u * j);
        }
        p += dir * (k * lift);
    }
    return p;
}

// The parent's surface at grid vertex (i, j): even vertices on the parent's lattice, odd ones
// on the midpoint of the parent edge (or the 00-11 diagonal) they split.
float3 t_target<let L : int>(uint pool, TerrainDraw d, uint i, uint j) {
    bool ei = (i & 1u) == 0;
    bool ej = (j & 1u) == 0;
    if (ei && ej) {
        return t_target_even<L>(pool, d, i, j);
    }
    if (!ei && ej) {
        return 0.5 * (t_target_even<L>(pool, d, i - 1u, j) + t_target_even<L>(pool, d, i + 1u, j));
    }
    if (ei && !ej) {
        return 0.5 * (t_target_even<L>(pool, d, i, j - 1u) + t_target_even<L>(pool, d, i, j + 1u));
    }
    return 0.5 * (t_target_even<L>(pool, d, i - 1u, j - 1u) + t_target_even<L>(pool, d, i + 1u, j + 1u));
}

float t_morph(float3 rel, float2 band) {
    return saturate((length(rel) - band.x) / max(band.y - band.x, 1e-3));
}

// Grid vertex (i, j) along an edge of a coarser neighbour, `e` the index along that edge and
// (i, j) = at(e): the coarser node's own edge, with its own morph.
float3 t_coarse_even<let L : int>(uint pool, TerrainDraw d, uint e, uint2 a, uint2 b, uint2 c) {
    float3 p = t_base<L>(pool, d, a.x, a.y);
    if ((e & 3u) == 0) {
        return p;
    }
    // A coarse odd vertex: morphs toward the midpoint of its coarse neighbours.
    float m = t_morph(d.origin.xyz + p, d.morph.zw);
    float3 mid = 0.5 * (t_base<L>(pool, d, b.x, b.y) + t_base<L>(pool, d, c.x, c.y));
    return lerp(p, mid, m);
}

uint2 t_edge_at(uint edge, uint e) {
    switch (edge) {
    case 0: return uint2(0, e);
    case 1: return uint2(T_CELLS, e);
    case 2: return uint2(e, 0);
    default: return uint2(e, T_CELLS);
    }
}

float3 t_coarse<let L : int>(uint pool, TerrainDraw d, uint edge, uint e) {
    if ((e & 1u) == 0) {
        return t_coarse_even<L>(pool, d, e, t_edge_at(edge, e), t_edge_at(edge, e - 2u), t_edge_at(edge, e + 2u));
    }
    float3 a = t_coarse_even<L>(pool, d, e - 1u, t_edge_at(edge, e - 1u), t_edge_at(edge, e - 3u), t_edge_at(edge, e + 1u));
    float3 b = t_coarse_even<L>(pool, d, e + 1u, t_edge_at(edge, e + 1u), t_edge_at(edge, e - 1u), t_edge_at(edge, e + 3u));
    return 0.5 * (a + b);
}

// Final position of grid vertex (i, j), node-relative: geomorphed toward the parent. An edge
// along a coarser neighbour is pinned to that neighbour's (morphing) edge; along a neighbour that
// draws fewer vertices (index LOD) the vertices between its lattice points slide onto the segment.
float3 t_grid_vertex<let L : int>(uint pool, TerrainDraw d, uint i, uint j) {
    uint fl = d.slots.z;
    uint edge = 4;
    uint e = 0;
    if (i == 0) { edge = 0; e = j; }
    else if (i == T_CELLS) { edge = 1; e = j; }
    else if (j == 0) { edge = 2; e = i; }
    else if (j == T_CELLS) { edge = 3; e = i; }
    if (edge < 4) {
        bool cb = (fl & (1u << edge)) != 0;
        uint E = 1u << ((fl >> (T_EDGE_SHIFT + 3u * edge)) & 7u);
        if (cb && E <= 2u) {
            return t_coarse<L>(pool, d, edge, e);
        }
        if (E > 1u) {
            uint lo = e & ~(E - 1u);
            if (lo == e) {
                return t_base<L>(pool, d, i, j);
            }
            uint2 a = t_edge_at(edge, lo);
            uint2 b = t_edge_at(edge, lo + E);
            return lerp(t_base<L>(pool, d, a.x, a.y), t_base<L>(pool, d, b.x, b.y), float(e - lo) / float(E));
        }
    }
    float3 p = t_base<L>(pool, d, i, j);
    if ((fl & T_NOMORPH) != 0) {
        return p;
    }
    float m = t_morph(d.origin.xyz + p, d.morph.xy);
    if (m <= 0.0) {
        return p;
    }
    return lerp(p, t_target<L>(pool, d, i, j), m);
}

// Skirt ring vertex s (0..255), counter-clockwise seen from above: j = 0 (i rising), i = 64
// (j rising), j = 64 (i falling), i = 0 (j falling).
uint2 t_ring(uint s) {
    uint e = s % T_CELLS;
    switch (s / T_CELLS) {
    case 0: return uint2(e, 0);
    case 1: return uint2(T_CELLS, e);
    case 2: return uint2(T_CELLS - e, T_CELLS);
    default: return uint2(0, T_CELLS - e);
    }
}

// Vertex `v` of the shared node mesh (grid row-major, then the skirt ring), camera-relative.
// `L` != 0 adds the canopy lift (trees G3); a compile-time constant per entry point so the
// unlifted shaders keep their exact code.
float3 t_vertex<let L : int>(uint pool, TerrainDraw d, uint v) {
    if (v < T_GRID_VERTS) {
        return d.origin.xyz + t_grid_vertex<L>(pool, d, v % T_SIDE, v / T_SIDE);
    }
    uint2 ij = t_ring(v - T_GRID_VERTS);
    float3 p = t_grid_vertex<L>(pool, d, ij.x, ij.y);
    float depth = (d.slots.z & T_SKIRT) != 0 ? d.origin.w : 0.0;
    if (L != 0 && d.polar.w > 0.0 && depth > 0.0) {
        // Down past the canopy lift: neighbours lift from their own pages (cracks at forest
        // edges between levels close against the bare ground).
        depth += t_own_lift(pool, d, ij.x, ij.y, p);
    }
    // Radially down (the node's up is close enough to the vertex's for a skirt).
    float3 up = normalize(d.origin.xyz + p - (d.origin.xyz - d.dir_c.xyz * d.dir_c.w));
    return d.origin.xyz + p - up * depth;
}

// The three vertex numbers of primitive `prim` of the shared mesh (the index buffer's order:
// quads row-major split on the 00-11 diagonal, then two triangles per skirt segment; the LOD
// at T_PRIM_LOD_SHIFT picks the stride, 64 >> lod cells per side, the ring following it).
uint3 t_triangle(uint prim) {
    uint lod = (prim >> T_PRIM_LOD_SHIFT) & 3u;
    prim &= (1u << T_PRIM_LOD_SHIFT) - 1u;
    uint st = 1u << lod;
    uint c = T_CELLS >> lod;
    if (prim < 2u * c * c) {
        uint q = prim >> 1;
        uint i = (q % c) * st;
        uint j = (q / c) * st;
        uint v00 = j * T_SIDE + i;
        uint v10 = v00 + st;
        uint v01 = v00 + st * T_SIDE;
        uint v11 = v01 + st;
        return (prim & 1u) == 0 ? uint3(v00, v10, v11) : uint3(v00, v11, v01);
    }
    uint k = prim - 2u * c * c;
    uint s = k >> 1;
    uint s1 = (s + 1u) % (4u * c);
    uint2 p = t_ring(s * st);
    uint2 q = t_ring(s1 * st);
    uint tp = p.y * T_SIDE + p.x;
    uint tq = q.y * T_SIDE + q.x;
    uint a = T_GRID_VERTS + s * st;
    uint b = T_GRID_VERTS + s1 * st;
    return (k & 1u) == 0 ? uint3(a, b, tq) : uint3(a, tq, tp);
}

// ---------------------------------------------------------------------------------------------
// Resolve helpers (P-slice S1): the smooth surface under a terrain pixel.
// ---------------------------------------------------------------------------------------------

// `t_lattice` at fractional grid coordinates (same formulas).
float3 t_lattice_f(TerrainDraw d, float2 g, float h, out float3 dir) {
    float tu = tan((g.x - 32.0) * d.shape.x);
    float tv = tan((g.y - 32.0) * d.shape.x);
    float du = tu * (1.0 + d.fu.w * d.fu.w) / (1.0 - d.fu.w * tu);
    float dv = tv * (1.0 + d.fv.w * d.fv.w) / (1.0 - d.fv.w * tv);
    float3 delta = du * d.fu.xyz + dv * d.fv.xyz;
    float lc = d.cc.w;
    float k = 2.0 * dot(d.cc.xyz, delta) + dot(delta, delta);
    float l = sqrt(lc * lc + k);
    float3 dd = delta / l - d.cc.xyz * (k / ((lc + l) * l * lc));
    dir = d.dir_c.xyz + dd;
    float dzc = dot(d.dir_c.xyz, d.polar.xyz);
    float ddz = dot(dd, d.polar.xyz);
    float dz = dzc + ddz;
    float e = d.shape.z;
    float s = sqrt(1.0 - e * dz * dz);
    float sc = d.shape.w;
    float dr = d.shape.y * e * ddz * (dz + dzc) / (s * sc * (s + sc));
    float r = d.dir_c.w + dr;
    return dd * r + d.dir_c.xyz * dr + dir * h;
}

// Grid coordinates (i, j) of vertex `v` of the shared mesh (a skirt vertex: its edge vertex).
float2 t_vertex_grid(uint v) {
    if (v < T_GRID_VERTS) {
        return float2(float(v % T_SIDE), float(v / T_SIDE));
    }
    return float2(t_ring(v - T_GRID_VERTS));
}

// Own page height at integer page sample (a, b), clamped to the page.
float t_page_at(uint pool, TerrainDraw d, int a, int b) {
    uint i = uint(clamp(a, 0, int(T_PAGE_N) - 1));
    uint j = uint(clamp(b, 0, int(T_PAGE_N) - 1));
    return t_page(pool, d.slots.x, i, j, d.hq.x, d.hq.y);
}

// Height (m) and gradient (m per grid cell, along i and j) of the own page at grid
// coordinates `g`: bilinear heights, central-difference gradients (over +-`r` page samples)
// at the four page samples around the point blended bilinearly (a smooth normal, not the
// facet's; a wider `r` gives the landform slope, without source noise).
void t_surface(uint pool, TerrainDraw d, float2 g, int r, out float h, out float2 dh) {
    float2 s = clamp(g * float(T_PAGE_STRIDE), 0.0, float(T_PAGE_N - 1));
    int2 s0 = int2(min(floor(s), float2(float(T_PAGE_N - 2))));
    float2 t = s - float2(s0);
    h = 0.0;
    dh = float2(0.0);
    for (uint k = 0; k < 4; k++) {
        int2 q = s0 + int2(int(k & 1u), int(k >> 1));
        float w = ((k & 1u) != 0 ? t.x : 1.0 - t.x) * ((k >> 1) != 0 ? t.y : 1.0 - t.y);
        int2 lo = max(q - r, int2(0));
        int2 hi = min(q + r, int2(int(T_PAGE_N) - 1));
        float gx = (t_page_at(pool, d, hi.x, q.y) - t_page_at(pool, d, lo.x, q.y)) / float(hi.x - lo.x);
        float gy = (t_page_at(pool, d, q.x, hi.y) - t_page_at(pool, d, q.x, lo.y)) / float(hi.y - lo.y);
        h += w * t_page_at(pool, d, q.x, q.y);
        dh += w * float2(gx, gy);
    }
    dh *= float(T_PAGE_STRIDE);
}

// Smooth outward normal and radial up at grid coordinates `g` (world axes); `h` the height;
// gradients over +-`r` page samples (see `t_surface`).
float3 t_smooth_normal(uint pool, TerrainDraw d, float2 g, int r, out float3 up, out float h) {
    float2 dh;
    t_surface(pool, d, g, r, h, dh);
    const float e = 0.25;
    float3 dir;
    float3 pu = t_lattice_f(d, g + float2(e, 0.0), h, dir) - t_lattice_f(d, g - float2(e, 0.0), h, dir);
    float3 pv = t_lattice_f(d, g + float2(0.0, e), h, dir) - t_lattice_f(d, g - float2(0.0, e), h, dir);
    t_lattice_f(d, g, h, up);
    up = normalize(up);
    float3 tu = pu / (2.0 * e) + up * dh.x;
    float3 tv = pv / (2.0 * e) + up * dh.y;
    float3 n = normalize(cross(tu, tv));
    return dot(n, up) < 0.0 ? -n : n;
}

// Page sample spacing (m) at grid coordinates `g` (along i).
float t_sample_spacing(TerrainDraw d, float2 g) {
    float3 dir;
    return length(t_lattice_f(d, g + float2(0.5, 0.0), 0.0, dir) - t_lattice_f(d, g - float2(0.5, 0.0), 0.0, dir)) / float(T_PAGE_STRIDE);
}

// ---------------------------------------------------------------------------------------------
// Grass shells (styles C and D, ew grass.rs): stacked offsets of the drawn ground with an
// alpha-tested strand lattice, the type from a toroidal map of face-metre cells around the
// camera (ed_world grass.rs `build_map`; layout in terrain.rs `shell_layout`).
// Visibility words of a shell pixel: x = TERRAIN_VIS_BIT | L << 24 | draw, y = prim | lod << 24 |
// k << 27 | SHELL_BIT (plain terrain pixels keep y under 2^28).
// ---------------------------------------------------------------------------------------------

// ---------------------------------------------------------------------------------------------
// Ground pages (grass G4): 257 x 257 packed [herb, crop, shrub, wet] land-cover shares (RGBA8 in
// a u32, ed_geo::GroundPage) per pool slot, after every slot's canopy words.
// ---------------------------------------------------------------------------------------------

static const float T_CLIM_ARID_MAX = 4.0;     // terrain.rs CLIM_ARID_MAX
static const float T_CLIM_T0 = -60.0;         // CLIM_T0
static const float T_CLIM_T_RANGE = 100.0;    // CLIM_T_RANGE

float4 t_ground_raw(uint pool, uint off, uint i, uint j) {
    uint c = ed_buf<uint>(pool)[off + j * T_PAGE_N + i];
    return float4(float(c & 0xFFu), float((c >> 8) & 0xFFu), float((c >> 16) & 0xFFu), float(c >> 24)) / 255.0;
}

// Bilinear shares at page-sample coordinates `s` of the page at `off`.
float4 t_ground_bilin(uint pool, uint off, float2 s) {
    s = clamp(s, 0.0, float(T_PAGE_N - 1));
    int2 s0 = int2(min(floor(s), float2(float(T_PAGE_N - 2))));
    float2 t = s - float2(s0);
    uint2 u = uint2(s0);
    float4 a = lerp(t_ground_raw(pool, off, u.x, u.y), t_ground_raw(pool, off, u.x + 1u, u.y), t.x);
    float4 b = lerp(t_ground_raw(pool, off, u.x, u.y + 1u), t_ground_raw(pool, off, u.x + 1u, u.y + 1u), t.x);
    return lerp(a, b, t.y);
}

// Shares at grid point `g` of a draw: its own page, else its parent's quadrant (a draw whose own
// page is still building).
float4 t_ground_shares(uint pool, TerrainDraw d, float2 g) {
    float2 s = g * float(T_PAGE_STRIDE);
    if (d.gnd.x != 0u) {
        return t_ground_bilin(pool, d.gnd.x, s);
    }
    float2 q = float2((d.slots.z & T_QX) != 0u ? 128.0 : 0.0, (d.slots.z & T_QY) != 0u ? 128.0 : 0.0);
    return t_ground_bilin(pool, d.gnd.y, q + 0.5 * s);
}

// Climate (aridity, sea-level mean temperature C) at grid point `g`: bilinear over the corners.
float2 t_climate(TerrainDraw d, float2 g) {
    float2 c[4];
    [ForceUnroll]
    for (uint k = 0; k < 4u; k++) {
        uint w = d.clim[k];
        c[k] = float2(float(w & 0xFFFFu) / 65535.0 * T_CLIM_ARID_MAX, T_CLIM_T0 + float(w >> 16) / 65535.0 * T_CLIM_T_RANGE);
    }
    float2 t = saturate(g / float(T_CELLS));
    return lerp(lerp(c[0], c[1], t.x), lerp(c[2], c[3], t.x), t.y);
}

static const uint SHELL_BIT = 0x80000000u;
static const uint SHELL_K_SHIFT = 27;
static const uint SHELL_PRIM_MASK = 0x03FFFFFFu;
static const uint SHELL_L_SHIFT = 24;
static const uint SHELL_DRAW_MASK = 0x00FFFFFFu;

// Face metres of grid point `g` relative to the map centre (wrapped to +-2048 m).
float2 shell_wrel(FrameData* f, TerrainDraw d, float2 g) {
    float2 c = d.tex.xy - f.shell.c.xy;
    c -= 4096.0 * round(c / 4096.0);
    return c + (g - 32.0) * (d.shape.x * d.shape.y);
}

// The draw is on the map's face (the maps are keyed in that face's metres).
bool shell_face(FrameData* f, TerrainDraw d) {
    return (f.shell.cfg.x & 4u) != 0u && dot(d.fu.xyz, f.shell.fu.xyz) > 0.999;
}

// Map word of cell `cell` (relative to the centre) of the fine (0) or coarse (1) map; 0 outside.
uint shell_map(FrameData* f, uint buf, uint coarse, int2 cell) {
    int n = int(f.shell.cells.z);
    if (any(cell < -n / 2) || any(cell >= n / 2)) {
        return 0u;
    }
    int2 off = coarse != 0u ? f.shell.off.zw : f.shell.off.xy;
    uint2 s = uint2((cell + off) & (n - 1));
    uint base = f.shell.cfg.z + coarse * uint(n * n);
    return ed_buf<uint>(buf)[base + s.y * uint(n) + s.x];
}

GrassType shell_type(FrameData* f, uint buf, uint kind) {
    return ed_buf<GrassType>(buf)[f.shell.cfg.y + kind];
}

struct ShellHit {
    bool hit;
    float tt;       // height up the strand (0 root, 1 tip)
    float rnd;      // per-strand random
    uint kind;
    float dry;      // this strand is dry (0/1)
    float2 side;    // pixel offset from the strand axis (cell units / radius), for the normal
};

uint shell_hash2(int2 q, uint salt) {
    return grass_hash(uint(q.x) * 0x9E3779B1u ^ grass_hash(uint(q.y) ^ salt));
}

// Strand LOD: strand cells grow by 2^L so a strand stays at least ~1.5 pixels across
// (`px_m`: the pixel footprint on the ground, `dith`: 0..1 to dither the step).
uint shell_lod(FrameData* f, float sm, float px_m, float dith) {
    float l = log2(max(px_m * 2.0 / sm, 1.0)) + dith - 0.5;
    return uint(clamp(floor(l + 0.5), 0.0, 7.0));
}

// The strand test of shell `k` (of `n`) at `wrel` (face metres from the map centre), view depth
// `dz`, strand LOD `L`. Deterministic in (world position, k, n, L): the raster and the resolve
// agree. `force`: the resolve, which only wants the attributes.
ShellHit shell_strand(FrameData* f, uint buf, float2 wrel, uint k, uint n, uint L, float dz, bool force) {
    ShellHit o;
    o.hit = false;
    o.tt = 0.0;
    o.rnd = 0.0;
    o.kind = 0u;
    o.dry = 0.0;
    o.side = float2(0.0);
    // The type cell, jittered by half a cell per 1/8 m so type edges interlock.
    float fine = f.shell.cells.x;
    int2 jq = int2(floor(wrel * 8.0));
    uint jh = shell_hash2(jq, 0x51u);
    float2 jit = (float2(grass_rand(jh), grass_rand(grass_hash(jh + 7u))) - 0.5) * fine;
    uint w = shell_map(f, buf, 0u, int2(floor((wrel + jit) / fine)));
    if ((w & 0xFFu) == 0u) {
        return o;
    }
    uint kind = (w & 0xFFu) - 1u;
    float dens = float((w >> 8) & 0xFFu) / 255.0;
    float hsc = float((w >> 16) & 0xFFu) / 255.0 * 2.0;
    float dry = float((w >> 24) & 0xFFu) / 255.0;
    GrassType ty = shell_type(f, buf, kind);
    float hk = f.shell.fu.w * float(k + 1u) / float(n);
    float H = ty.shell.x * hsc;
    o.kind = kind;
    // Strand lattice: 2^-m metres, offset by the (32 m snapped) centre so it is world-stable.
    float sc = ty.shell.y * float(1u << L);
    float2 q = wrel / sc;
    int2 qi = int2(floor(q)) + int2(round(f.shell.c.xy / sc));
    float2 lq = frac(q);
    uint h = shell_hash2(qi, 0xA3u + L);
    float r0 = grass_rand(h);
    float r1 = grass_rand(grass_hash(h + 1u));
    float r2 = grass_rand(grass_hash(h + 2u));
    float r3 = grass_rand(grass_hash(h + 3u));
    o.rnd = r1;
    o.dry = r3 < dry ? 1.0 : 0.0;
    // Range fade: strands thin out (speckle) over the last quarter of the range.
    float fade = smoothstep(f.shell.c.z * 0.72, f.shell.c.z, dz);
    if (!force && grass_rand(grass_hash(h + 5u)) < fade) {
        return o;
    }
    if (k == 0u) {
        // The floor: a near-solid carpet under the strands.
        o.tt = 0.08;
        o.side = lq - 0.5;
        o.hit = force || r2 < ty.shell.z * dens * 1.15 + 0.05;
        return o;
    }
    if (!force && r0 >= ty.shell.z * dens) {
        return o;
    }
    float hs = H * lerp(0.5, 1.0, r1);
    float tt = hk / max(hs, 1e-4);
    if (!force && tt > 1.0) {
        return o;
    }
    tt = saturate(tt);
    // Tapered strand, leaning with height (direction per strand).
    float a = r2 * 6.2831853;
    // Strand positions jittered across most of the cell (a regular lattice beats into moire).
    float2 c = float2(0.5) + (float2(grass_rand(grass_hash(h + 9u)), grass_rand(grass_hash(h + 11u))) - 0.5) * 0.5 + float2(cos(a), sin(a)) * (0.2 * tt * tt);
    float rad = 0.42 * (1.0 - pow(tt, 1.6)) + 0.04;
    float2 dl = lq - c;
    o.side = dl / rad;
    o.tt = tt;
    o.hit = force || dot(dl, dl) < rad * rad;
    return o;
}
