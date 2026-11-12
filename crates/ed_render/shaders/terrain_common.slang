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

// One drawn node (160 bytes, `DRAW_WORDS` = 40). World axes, camera-relative.
struct TerrainDraw {
    float4 origin;  // xyz: node origin (ellipsoid point of the centre direction) - camera, w: skirt depth (m)
    float4 dir_c;   // xyz: centre direction, w: ellipsoid radius along it (m)
    float4 cc;      // xyz: cube point of the centre (F_n + T_u F_u + T_v F_v), w: its length
    float4 fu;      // xyz: face u axis, w: T_u = tan(centre angle u)
    float4 fv;      // xyz: face v axis, w: T_v
    float4 shape;   // x: angle per grid cell, y: equatorial radius, z: 1 - a^2/b^2, w: sqrt(1 - z dz_c^2)
    float4 morph;   // x, y: own morph start, end (m); z, w: the coarser level's (edge stitching)
    float4 polar;   // xyz: body polar axis (+Z body-fixed)
    uint4 slots;    // x: page slot, y: parent page slot (T_NO_SLOT: none), z: flags, w: level
    float4 hq;      // x, y: page scale, offset; z, w: parent page scale, offset
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
float3 t_base(uint pool, TerrainDraw d, uint i, uint j) {
    float3 dir;
    return t_lattice(d, i, j, t_own_h(pool, d, i, j), dir);
}

// Morph target of an even grid vertex: the parent's height at the same lattice point. Border
// vertices keep their own height (it equals the neighbours' and the coarser level's: G4).
float3 t_target_even(uint pool, TerrainDraw d, uint i, uint j) {
    bool border = i == 0 || j == 0 || i == T_CELLS || j == T_CELLS;
    float h;
    if (border || d.slots.y == T_NO_SLOT) {
        h = t_own_h(pool, d, i, j);
    } else {
        uint qx = (d.slots.z & T_QX) != 0 ? 128u : 0u;
        uint qy = (d.slots.z & T_QY) != 0 ? 128u : 0u;
        h = t_page(pool, d.slots.y, qx + 2u * i, qy + 2u * j, d.hq.z, d.hq.w);
    }
    float3 dir;
    return t_lattice(d, i, j, h, dir);
}

// The parent's surface at grid vertex (i, j): even vertices on the parent's lattice, odd ones
// on the midpoint of the parent edge (or the 00-11 diagonal) they split.
float3 t_target(uint pool, TerrainDraw d, uint i, uint j) {
    bool ei = (i & 1u) == 0;
    bool ej = (j & 1u) == 0;
    if (ei && ej) {
        return t_target_even(pool, d, i, j);
    }
    if (!ei && ej) {
        return 0.5 * (t_target_even(pool, d, i - 1u, j) + t_target_even(pool, d, i + 1u, j));
    }
    if (ei && !ej) {
        return 0.5 * (t_target_even(pool, d, i, j - 1u) + t_target_even(pool, d, i, j + 1u));
    }
    return 0.5 * (t_target_even(pool, d, i - 1u, j - 1u) + t_target_even(pool, d, i + 1u, j + 1u));
}

float t_morph(float3 rel, float2 band) {
    return saturate((length(rel) - band.x) / max(band.y - band.x, 1e-3));
}

// Grid vertex (i, j) along an edge of a coarser neighbour, `e` the index along that edge and
// (i, j) = at(e): the coarser node's own edge, with its own morph.
float3 t_coarse_even(uint pool, TerrainDraw d, uint e, uint2 a, uint2 b, uint2 c) {
    float3 p = t_base(pool, d, a.x, a.y);
    if ((e & 3u) == 0) {
        return p;
    }
    // A coarse odd vertex: morphs toward the midpoint of its coarse neighbours.
    float m = t_morph(d.origin.xyz + p, d.morph.zw);
    float3 mid = 0.5 * (t_base(pool, d, b.x, b.y) + t_base(pool, d, c.x, c.y));
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

float3 t_coarse(uint pool, TerrainDraw d, uint edge, uint e) {
    if ((e & 1u) == 0) {
        return t_coarse_even(pool, d, e, t_edge_at(edge, e), t_edge_at(edge, e - 2u), t_edge_at(edge, e + 2u));
    }
    float3 a = t_coarse_even(pool, d, e - 1u, t_edge_at(edge, e - 1u), t_edge_at(edge, e - 3u), t_edge_at(edge, e + 1u));
    float3 b = t_coarse_even(pool, d, e + 1u, t_edge_at(edge, e + 1u), t_edge_at(edge, e - 1u), t_edge_at(edge, e + 3u));
    return 0.5 * (a + b);
}

// Final position of grid vertex (i, j), node-relative: geomorphed toward the parent, edges
// along a coarser neighbour pinned to that neighbour's edge.
float3 t_grid_vertex(uint pool, TerrainDraw d, uint i, uint j) {
    uint coarse = d.slots.z & T_COARSE_MASK;
    if (coarse != 0) {
        if ((coarse & 1u) != 0 && i == 0) return t_coarse(pool, d, 0, j);
        if ((coarse & 2u) != 0 && i == T_CELLS) return t_coarse(pool, d, 1, j);
        if ((coarse & 4u) != 0 && j == 0) return t_coarse(pool, d, 2, i);
        if ((coarse & 8u) != 0 && j == T_CELLS) return t_coarse(pool, d, 3, i);
    }
    float3 p = t_base(pool, d, i, j);
    float m = t_morph(d.origin.xyz + p, d.morph.xy);
    if (m <= 0.0) {
        return p;
    }
    return lerp(p, t_target(pool, d, i, j), m);
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
float3 t_vertex(uint pool, TerrainDraw d, uint v) {
    if (v < T_GRID_VERTS) {
        return d.origin.xyz + t_grid_vertex(pool, d, v % T_SIDE, v / T_SIDE);
    }
    uint2 ij = t_ring(v - T_GRID_VERTS);
    float3 p = t_grid_vertex(pool, d, ij.x, ij.y);
    float depth = (d.slots.z & T_SKIRT) != 0 ? d.origin.w : 0.0;
    // Radially down (the node's up is close enough to the vertex's for a skirt).
    float3 up = normalize(d.origin.xyz + p - (d.origin.xyz - d.dir_c.xyz * d.dir_c.w));
    return d.origin.xyz + p - up * depth;
}

// The three vertex numbers of primitive `prim` of the shared mesh (the index buffer's order:
// quads row-major split on the 00-11 diagonal, then two triangles per skirt segment).
uint3 t_triangle(uint prim) {
    if (prim < T_GRID_TRIS) {
        uint q = prim >> 1;
        uint i = q % T_CELLS;
        uint j = q / T_CELLS;
        uint v00 = j * T_SIDE + i;
        uint v10 = v00 + 1u;
        uint v01 = v00 + T_SIDE;
        uint v11 = v01 + 1u;
        return (prim & 1u) == 0 ? uint3(v00, v10, v11) : uint3(v00, v11, v01);
    }
    uint k = prim - T_GRID_TRIS;
    uint s = k >> 1;
    uint s1 = (s + 1u) % T_RING;
    uint2 p = t_ring(s);
    uint2 q = t_ring(s1);
    uint tp = p.y * T_SIDE + p.x;
    uint tq = q.y * T_SIDE + q.x;
    uint a = T_GRID_VERTS + s;
    uint b = T_GRID_VERTS + s1;
    return (k & 1u) == 0 ? uint3(a, b, tq) : uint3(a, tq, tp);
}
