// Material resolve: one full-screen pass shades every pixel once from the visibility buffer
// (sky where nothing was drawn). The triangle is refetched, the view ray intersected with it
// for perspective-correct barycentrics, and the material read from the bindless table.
// Materials v0: PBR (GGX / Smith / Schlick, metallic-roughness) and toon, same light inputs.
// M3: bindless textures (base colour, BC5 normal, ORM) sampled with ray-differential
// gradients (the neighbouring pixels' rays hit the same triangle plane), a tangent frame
// derived per triangle from position and UV deltas (no stored tangents), and the material's
// UV transform.
// M4d: the sun is shadowed by cascades (PCF), ambient light is the sky as SH9 (diffuse) and
// the sky model blended towards it by roughness (specular), both darkened by GTAO; toon reads
// the same terms (lighting.slang).
// M4e: the ambient SH is skylight plus ground bounce (light.rs `ambient_sh9`), AO is
// multi-bounce, and ambient diffuse gives up the specular lobe's share.
// M4f: large-scale sky visibility (ao.slang) swaps the sky a street canyon hides for bounce
// light off the buildings, in the diffuse and the specular ambient.
// Phase 2 W5: with the atmosphere on (atmos.slang), the sky, the specular environment, the sun
// reaching each point and the ambient SH come from its tables, and aerial perspective is laid
// over every opaque pixel.
#include "shader_abi.slang"
#include "common.slang"
#include "lighting.slang"
#include "terrain_common.slang"

struct ResolveC {
    uint frame;
    uint vis;
    uint instances;
    uint nodes;
    uint meshes;
    uint materials;
    uint verts;
    uint indices;
    uint uvs;       // bindless: float2 per vertex, parallel to verts
    uint smp;       // bindless: linear, mipmapped, repeat, anisotropic
    uint shadow;    // bindless depth texture: the cascade atlas (when shadows are on)
    uint ao;        // bindless buffer: half-res AO (when AO is on)
    uint t_draws;   // bindless buffer: the terrain provider's TerrainDraw table (terrain on)
    uint t_pool;    // bindless buffer: the terrain height page pool (terrain on)
    uint t_on;      // 1: a terrain provider drew this frame
    uint t_mat;     // terrain material record index in the draw table (0: the flat tint)
};
ED_CONSTANTS(ResolveC)

[shader("vertex")]
FsOut vs_resolve(uint vid: SV_VulkanVertexID) {
    return fullscreen_vertex(vid);
}

static const float PI = 3.14159265;

float d_ggx(float noh, float a) {
    float a2 = a * a;
    float d = noh * noh * (a2 - 1.0) + 1.0;
    return a2 / (PI * d * d);
}

float v_smith(float nov, float nol, float a) {
    float a2 = a * a;
    float gv = nol * sqrt(nov * nov * (1.0 - a2) + a2);
    float gl = nov * sqrt(nol * nol * (1.0 - a2) + a2);
    return 0.5 / max(gv + gl, 1e-5);
}

float3 f_schlick(float3 f0, float voh) {
    return f0 + (1.0 - f0) * pow(1.0 - voh, 5.0);
}

// Split-sum environment BRDF approximation (Karis 2014, mobile).
float2 env_brdf(float nov, float rough) {
    const float4 c0 = float4(-1.0, -0.0275, -0.572, 0.022);
    const float4 c1 = float4(1.0, 0.0425, 1.04, -0.04);
    float4 r = rough * c0 + c1;
    float a004 = min(r.x * r.x, exp2(-9.28 * nov)) * r.x + r.y;
    return float2(-1.04, 1.04) * a004 + r.zw;
}

// `spec_k` scales the specular lobe and the sky's specular share (grass sheen; 1: plain PBR).
float3 shade_pbr_k(FrameData* f, float3 sun, float3 albedo, float metal, float rough, float ao, float sv, float sun_vis, float3 n, float3 v, float spec_k) {
    metal = saturate(metal);
    rough = clamp(rough, 0.04, 1.0);
    float a = rough * rough;
    float3 l = f.sun_dir.xyz;
    float3 h = normalize(l + v);
    float nol = saturate(dot(n, l));
    float nov = max(dot(n, v), 1e-4);
    float noh = saturate(dot(n, h));
    float voh = saturate(dot(v, h));
    float3 f0 = lerp(float3(0.04), albedo, metal);
    float3 diff_col = albedo * (1.0 - metal);
    float3 F = f_schlick(f0, voh);
    float3 spec = d_ggx(noh, a) * v_smith(nov, nol, a) * F * spec_k;
    float3 direct = (diff_col * (1.0 - F) / PI + spec) * sun * (nol * sun_vis);
    float2 ab = env_brdf(nov, rough);
    float3 r = reflect(-v, n);
    float3 env = f.light.flags.z != 0 ? lerp(sky_env(f, r), sh_irradiance(f, r), rough) : hemi_ambient(f, r);
    env = occluded_ambient(f, env, sv);
    // Specular occlusion from AO (Lagarde 2014).
    float spec_occ = saturate(pow(nov + ao, exp2(-16.0 * a - 1.0)) - 1.0 + ao);
    // The specular lobe's share of the ambient light does not also reach the diffuse term.
    float3 f_env = f0 * ab.x + ab.y;
    float3 amb = diff_col * (1.0 - f_env) * occluded_ambient(f, ambient_light(f, n), sv) * ao_multibounce(f, ao, diff_col) + env * f_env * spec_occ * spec_k;
    return direct + amb;
}

float3 shade_pbr(FrameData* f, float3 sun, float3 albedo, float metal, float rough, float ao, float sv, float sun_vis, float3 n, float3 v) {
    return shade_pbr_k(f, sun, albedo, metal, rough, ao, sv, sun_vis, n, v, 1.0);
}

float3 shade_toon(FrameData* f, float3 sun, GpuMaterial m, float3 albedo, float3 n, float3 v, float ao, float sv, float sun_vis) {
    float3 l = f.sun_dir.xyz;
    float ndl = dot(n, l);
    float bands = max(m.params.z, 1.0);
    float lit = floor(saturate(ndl * 0.5 + 0.5) * bands + 0.5) / bands * sun_vis;
    float3 c = lerp(albedo * m.shade.rgb, albedo, lit);
    float3 light = sun * (lit / PI) + occluded_ambient(f, ambient_light(f, n), sv) * ao_multibounce(f, ao, albedo);
    float rim = pow(1.0 - saturate(dot(n, v)), 4.0) * m.shade.w * saturate(ndl + 0.3) * sun_vis;
    return c * light + rim * sun * 0.15;
}

// Unclamped barycentrics (b1, b2) of the ray `dir` (from the camera) on the triangle plane.
float2 ray_bary(float3 dir, float3 p0, float3 e1, float3 e2) {
    float3 pv = cross(dir, e2);
    float det = dot(e1, pv);
    if (abs(det) <= 1e-12) {
        return float2(1.0 / 3.0);
    }
    float inv = 1.0 / det;
    float3 tv = -p0;
    float3 qv = cross(tv, e1);
    return float2(dot(tv, pv) * inv, dot(dir, qv) * inv);
}

float2 uv_xform(GpuMaterial m, float2 uv) {
    float r = asfloat(m.tex.w);
    float s = sin(r);
    float c = cos(r);
    float2 q = uv * m.uv.zw;
    return float2(c * q.x + s * q.y, -s * q.x + c * q.y) + m.uv.xy;
}

// Terrain albedo (P-slice S1): a slope and altitude tint until real land cover arrives.
// Albedos are physical (linear, 0..1); heights metres above the ellipsoid; slopes as the cosine
// between the smooth normal and local up.
static const float3 T_ALB_FOREST = float3(0.055, 0.075, 0.040);  // forest canopy
static const float3 T_ALB_MEADOW = float3(0.125, 0.140, 0.070);  // grass, alpine meadow
static const float3 T_ALB_ROCK = float3(0.210, 0.200, 0.185);    // bare rock, scree
static const float3 T_ALB_SNOW = float3(0.800, 0.820, 0.860);    // old snow, firn
static const float2 T_TREELINE_M = float2(1950.0, 2350.0);       // forest -> meadow
static const float2 T_SNOWLINE_M = float2(2750.0, 3050.0);       // snow on gentle slopes above
static const float2 T_ROCK_COS = float2(0.82, 0.72);             // 35 -> 44 deg: plants -> rock
static const float2 T_SNOW_COS = float2(0.55, 0.70);             // snow holds below ~50 deg
static const float T_ROUGH = 0.92;
static const float T_SLOPE_M = 24.0;                             // material slope footprint (m)

float3 terrain_albedo(float h, float slope_cos) {
    float3 veg = lerp(T_ALB_FOREST, T_ALB_MEADOW, smoothstep(T_TREELINE_M.x, T_TREELINE_M.y, h));
    float rock = smoothstep(T_ROCK_COS.x, T_ROCK_COS.y, slope_cos);
    float3 a = lerp(veg, T_ALB_ROCK, rock);
    float snow = smoothstep(T_SNOWLINE_M.x, T_SNOWLINE_M.y, h) * smoothstep(T_SNOW_COS.x, T_SNOW_COS.y, slope_cos);
    return lerp(a, T_ALB_SNOW, snow);
}

// ---------------------------------------------------------------------------------------------
// Terrain materials (P-slice S7 c): scanned surface layers blended by the landform.
// ---------------------------------------------------------------------------------------------
// Layers (terrain.rs MAT_LAYERS). Weights come from the landform slope (~24 m), the fine slope,
// altitude, aspect (snow lies lower on north faces), curvature (scree and soil collect in
// hollows, ridges stay bare) and low-frequency noise; then a height blend with each layer's
// displacement map, so grass sits between stones and snow fills cracks first. Texture
// coordinates are world-stable face metres (`TerrainDraw.tex`, periodic in 4096 m); steep rock
// is projected triplanar on the (face u, height) and (face v, height) planes.
static const uint L_ROCK = 0;
static const uint L_SCREE = 1;
static const uint L_GRASS = 2;
static const uint L_FOREST = 3;
static const uint L_SNOW = 4;
static const uint L_SOIL = 5;
static const uint T_LAYERS = 6;

// Periodic value noise (period 4096 m, `cell` must divide it), in [0, 1].
float t_hash2(int2 c) {
    uint h = uint(c.x) * 0x8da6b343u ^ uint(c.y) * 0xd8163841u;
    h ^= h >> 13;
    h *= 0x85ebca6bu;
    h ^= h >> 16;
    return float(h & 0xFFFFu) / 65535.0;
}
float t_noise(float2 w, float cell) {
    float2 q = w / cell;
    float2 i = floor(q);
    float2 f = q - i;
    f = f * f * (3.0 - 2.0 * f);
    int per = int(4096.0 / cell + 0.5);
    int2 c = int2(i);
    int2 c0 = int2(((c.x % per) + per) % per, ((c.y % per) + per) % per);
    int2 c1 = int2((c0.x + 1) % per, (c0.y + 1) % per);
    float a = t_hash2(c0);
    float b = t_hash2(int2(c1.x, c0.y));
    float cc = t_hash2(int2(c0.x, c1.y));
    float d = t_hash2(c1);
    return lerp(lerp(a, b, f.x), lerp(cc, d, f.x), f.y);
}

struct TSurf {
    float3 albedo;
    float3 n;
    float rough;
    float ao;
};

// One layer at one projection: albedo+height and normal+rough+AO, sampled with gradients.
// `t`, `b`: world directions of +u and +v of the projection (b: image up).
// One projection of a layer: `w` and its per-pixel steps in metres on the projection plane.
struct TProj {
    float2 w;
    float2 dx;
    float2 dy;
};

// Albedo (rgb) and displacement (a). Anti-tiling: a second, 4x larger copy with swapped axes
// (periodic-safe) carries most of the colour from mid distance out.
float4 t_layer_albedo(TerrainMat m, SamplerState smp, uint layer, TProj p, float near) {
    float k = 1.0 / m.tile[layer];
    Texture2DArray ta = ed_tex2d_array(m.albedo);
    float4 a = ta.SampleGrad(smp, float3(float2(p.w.x, -p.w.y) * k, float(layer)), float2(p.dx.x, -p.dx.y) * k, float2(p.dy.x, -p.dy.y) * k);
    float k2 = 0.25 * k;
    float4 b = ta.SampleGrad(smp, float3(float2(-p.w.y, -p.w.x) * k2, float(layer)), float2(-p.dx.y, -p.dx.x) * k2, float2(-p.dy.y, -p.dy.x) * k2);
    float mixk = lerp(0.35, 0.75, 1.0 - near);
    return float4(lerp(a.rgb, b.rgb, mixk), lerp(a.a, b.a, 0.5 * mixk));
}

// Normal xy (OpenGL, -1..1), roughness, AO.
float4 t_layer_normal(TerrainMat m, SamplerState smp, uint layer, TProj p) {
    float k = 1.0 / m.tile[layer];
    float4 b = ed_tex2d_array(m.normal).SampleGrad(smp, float3(float2(p.w.x, -p.w.y) * k, float(layer)), float2(p.dx.x, -p.dx.y) * k, float2(p.dy.x, -p.dy.y) * k);
    return float4(b.rg * 2.0 - 1.0, b.b, b.a);
}

// Bilinear height of the own page at page-sample coordinates `s` (clamped to the page).
float t_page_bilin(uint pool, TerrainDraw d, float2 s) {
    s = clamp(s, 0.0, float(T_PAGE_N - 1));
    int2 s0 = int2(min(floor(s), float2(float(T_PAGE_N - 2))));
    float2 t = s - float2(s0);
    float a = lerp(t_page_at(pool, d, s0.x, s0.y), t_page_at(pool, d, s0.x + 1, s0.y), t.x);
    float b = lerp(t_page_at(pool, d, s0.x, s0.y + 1), t_page_at(pool, d, s0.x + 1, s0.y + 1), t.x);
    return lerp(a, b, t.y);
}

// The blended surface. `n`: smooth shading normal; `nm`: landform normal (~24 m); `up`: radial
// up; `h`: height (m); `curv`: landform Laplacian (1/m, + hollow); `curv_f`: fine Laplacian;
// `w`, `dwx`, `dwy`: face metres and their per-pixel steps; `tu`, `tv`: world directions of
// +u, +v; `dist`: distance from the camera (m).
TSurf terrain_material(FrameData* f, TerrainMat m, TerrainDraw d, float3 n, float3 nm, float3 up, float h, float curv, float curv_f, float2 w, float2 dwx, float2 dwy, float dhx, float dhy, float3 tu, float3 tv, float dist, bool trees, float4 cv, float spacing) {
    SamplerState smp = ed_smp(ed_c.smp);
    float cs = dot(nm, up);          // landform slope cosine
    float cf = dot(n, up);           // fine slope cosine
    float3 north = normalize(d.polar.xyz - up * dot(d.polar.xyz, up));
    float3 nh = nm - up * cs;
    float northness = length_squared(nh) > 1e-6 ? dot(normalize(nh), north) : 0.0;
    float n1 = t_noise(w, 16.0);
    float n2 = t_noise(w + 37.0, 64.0);
    float n3 = t_noise(w + 911.0, 256.0);
    float nn = n1 * 0.35 + n2 * 0.4 + n3 * 0.25;  // 0..1
    float hollow = saturate(curv * 60.0);          // landform hollow (gully, cliff foot)
    float ridge = saturate(-curv * 60.0);
    float hollow_f = saturate(curv_f * 4.0);

    // Altitude bands, shifted by aspect and noise.
    float treeline = 2150.0 + 120.0 * (n3 - 0.5) - 100.0 * northness;
    float snowline = 3050.0 - 230.0 * northness + 200.0 * (n2 - 0.5);
    float alpine = smoothstep(treeline - 150.0, treeline + 250.0, h);  // forest -> open
    // Rock: steep landform (from ~40 deg above the treeline, ~48 below where forest holds the
    // slope), steep fine slope, ridges, and thin soil up high.
    float rock_cs = lerp(0.68, 0.77, alpine);
    float steep = smoothstep(rock_cs, rock_cs - 0.14, cs);
    float steep_f = smoothstep(rock_cs - 0.06, rock_cs - 0.2, cf);
    float rock = saturate(max(steep, steep_f * 0.8) + ridge * 0.5 * alpine + smoothstep(2700.0, 3300.0, h) * 0.45 - 0.25 * hollow);
    rock = saturate(rock + (n1 - 0.5) * 0.5 * smoothstep(0.92, 0.8, cs));
    // Scree: moderate slopes below rock, in hollows and at cliff feet, open ground.
    float moderate = smoothstep(0.97, 0.88, cs) * smoothstep(0.66, 0.80, cs);
    float scree = saturate(moderate * (0.1 + hollow * 1.2 + (n2 - 0.5) * 0.8) * lerp(0.1, 1.0, alpine) + steep * 0.3 * alpine);
    // Vegetation below: forest on slopes below the treeline, meadow on gentle or open ground.
    float veg = 1.0;
    // Forest: most of the slope below the treeline, with clearings; the flat valley floor is meadow.
    float forest = (1.0 - alpine) * smoothstep(0.99, 0.95, cs) * saturate(0.9 + (n3 - 0.5) * 1.4 + (n2 - 0.5) * 0.8);
    if (trees) {
        // Far forest rung (trees G3): the forest floor is where the canopy page has cover. A
        // steep edge on fine pages; linear in cover on coarse ones (an averaged edge sample
        // must not widen the stand: the density strip keeps one outline).
        // Where the page has no source data (cv.w, bilinear so the border is soft) the old
        // procedural rule above stays: no node-shaped rectangles of missing forest.
        float fd = lerp(cv.x, smoothstep(0.03, 0.5, cv.x), smoothstep(T_FOREST_LINEAR_M, T_FOREST_STEEP_M, spacing));
        forest = lerp(forest, fd, cv.w);
    }
    float grass = veg * (1.0 - forest) * smoothstep(3000.0, 2600.0, h + (n1 - 0.5) * 200.0);
    float soil = saturate(hollow_f * 0.8 + (n1 - 0.65) * 1.5) * (1.0 - alpine * 0.5);
    // Snow: above the snowline on what is not too steep to hold it; drifts in hollows.
    // Snow slides off past ~45-50 degrees: steep faces stay rock with snow on ledges.
    float snow = smoothstep(snowline - 120.0, snowline + 120.0, h + hollow * 150.0 + (n1 - 0.5) * 120.0) * smoothstep(0.62, 0.78, cf + (n1 - 0.5) * 0.1);

    float wt[T_LAYERS];
    wt[L_ROCK] = rock;
    wt[L_SCREE] = scree * (1.0 - rock * 0.7);
    wt[L_GRASS] = grass * (1.0 - rock) * (1.0 - scree * 0.8);
    wt[L_FOREST] = forest * (1.0 - rock) * (1.0 - scree * 0.8);
    wt[L_SOIL] = soil * 0.5 * (1.0 - rock) * (1.0 - snow);
    wt[L_SNOW] = snow * 1.6;
    [ForceUnroll]
    for (uint k = 0; k < T_LAYERS; k++) {
        if (k != L_SNOW) {
            wt[k] *= (1.0 - snow);
        }
    }

    // Projections: top (face u, v) for everything; rock and scree also on the two vertical
    // planes where the fine slope is steep.
    float near = saturate(1.0 - dist / 400.0);
    float3 ax = abs(float3(dot(n, tu), dot(n, tv), dot(n, up)));
    // Biplanar: the top plane and the dominant vertical one (two fetches instead of three).
    // The vertical weight is the major axis minus the minor one, so it fades to zero where
    // the two swap (no seam); the top plane fills that band.
    float3 tw = pow(ax, float3(4.0));
    float major = max(tw.x, tw.y) - min(tw.x, tw.y);
    tw.xy = tw.x > tw.y ? float2(major, 0.0) : float2(0.0, major);
    tw = max(tw - 0.03, 0.0);
    tw.z = max(tw.z, 0.01);
    tw /= max(tw.x + tw.y + tw.z, 1e-4);

    // Projections: top (face u, v); rock and snow add the dominant vertical plane ((u, height)
    // faces along v, (v, height) faces along u) where the fine slope is steep.
    TProj top = { w, dwx, dwy };
    bool side_u = tw.y >= tw.x;  // plane (u, height)
    float ws = side_u ? tw.y : tw.x;
    TProj side;
    side.w = side_u ? float2(w.x, h) : float2(w.y, h);
    side.dx = side_u ? float2(dwx.x, dhx) : float2(dwx.y, dhx);
    side.dy = side_u ? float2(dwy.x, dhy) : float2(dwy.y, dhy);
    float3 side_t = side_u ? tu : tv;

    // Pass 1: albedo and displacement of every active layer; height blend.
    float4 ah[T_LAYERS];
    float best = 0.0;
    [ForceUnroll]
    for (uint k = 0; k < T_LAYERS; k++) {
        ah[k] = float4(0.0);
        if (wt[k] < 0.04) {
            wt[k] = 0.0;
            continue;
        }
        bool tri = (k == L_ROCK || k == L_SNOW) && ws > 0.0;
        float4 a = tw.z > 0.0 || !tri ? t_layer_albedo(m, smp, k, top, near) : float4(0.0);
        if (tri) {
            a = a * tw.z + t_layer_albedo(m, smp, k, side, near) * ws;
        }
        ah[k] = a;
        // Height blend: displacement lifts a layer's own high points through its neighbours.
        wt[k] += a.a * lerp(0.15, 0.6, near);
        best = max(best, wt[k]);
    }
    float total = 0.0;
    [ForceUnroll]
    for (uint k = 0; k < T_LAYERS; k++) {
        wt[k] = wt[k] > 0.0 ? max(wt[k] - (best - 0.35), 0.0) : 0.0;
        total += wt[k];
    }
    // Pass 2: blend; normals only for the layers that survived, and only near.
    TSurf o;
    o.albedo = float3(0.0);
    o.n = float3(0.0);
    o.rough = 0.0;
    o.ao = 0.0;
    float3 pert = float3(0.0);
    [ForceUnroll]
    for (uint k = 0; k < T_LAYERS; k++) {
        if (wt[k] <= 0.0) {
            continue;
        }
        float a = wt[k] / max(total, 1e-5);
        o.albedo += a * ah[k].rgb * m.gain[k].rgb;
        float4 nr = float4(0.0, 0.0, T_ROUGH, 1.0);
        float3 nw = float3(0.0);
        if (near > 0.0) {
            bool tri = (k == L_ROCK || k == L_SNOW) && ws > 0.0;
            nr = float4(0.0);
            if (tw.z > 0.0 || !tri) {
                float4 b = t_layer_normal(m, smp, k, top);
                float s = tri ? tw.z : 1.0;
                nr = b * s;
                nw = (tu * b.x + tv * b.y) * s;
            }
            if (tri) {
                float4 b = t_layer_normal(m, smp, k, side);
                nr += b * ws;
                nw += (side_t * b.x + up * b.y) * ws;
            }
        }
        pert += a * nw;
        o.rough += a * nr.z;
        o.ao += a * nr.w;
    }
    if (total <= 0.0) {
        o.albedo = T_ALB_MEADOW;
        o.rough = T_ROUGH;
        o.ao = 1.0;
    }
    // Macro variation: brightness and warmth over tens to hundreds of metres (what the eye
    // reads on far slopes), damp darkening in hollows.
    float macro = lerp(0.82, 1.15, n2) * lerp(0.9, 1.08, n3);
    o.albedo *= macro * lerp(1.0, 0.8, hollow_f * (1.0 - snow));
    o.albedo *= lerp(float3(1.0), float3(1.04, 1.0, 0.94), n1 * (1.0 - snow));
    // The forest floor from afar reads as canopy (no trees drawn yet).
    float canopy = wt[L_FOREST] / max(total, 1e-5) * smoothstep(60.0, 600.0, dist) * (trees ? 1.0 - cv.w : 1.0);
    // Conifer canopy seen from afar, crown shadows included (darker than a sunlit leaf).
    o.albedo = lerp(o.albedo, float3(0.030, 0.044, 0.021) * lerp(0.85, 1.15, n1), canopy);
    // Normal map: fades with distance (the mips average it out anyway), whiteout-style add.
    float ns = lerp(0.35, 1.0, near);
    o.n = normalize(n + pert * ns);
    o.ao = lerp(1.0, o.ao, ns);
    return o;
}

// ---------------------------------------------------------------------------------------------
// The far forest rung (trees G3, §3 T3 option b, §4 option A): the canopy page read as crowns.
// ---------------------------------------------------------------------------------------------

// Crown albedo per species family (linear sunlit leaf mass): terrain.rs `CROWN_ALBEDO`, the one
// table (a test checks these agree).
static const float3 T_CROWN_BROAD = float3(0.050, 0.110, 0.024);
static const float3 T_CROWN_CONIFER = float3(0.026, 0.064, 0.020);
static const float3 T_CROWN_BARE = float3(0.034, 0.028, 0.022);   // a leafless crown: branches and shade
static const float3 T_LITTER = float3(0.050, 0.042, 0.030);   // needle and leaf litter
static const float T_CROWN_ROUGH = 0.8;
// Page spacings (m) between which the forest-floor weight goes from a steep edge to linear.
static const float T_FOREST_STEEP_M = 20.0;
static const float T_FOREST_LINEAR_M = 80.0;
// Crown lattices (m; both divide the 4096 m texture period): broadleaf and conifer.
static const float T_CELL_BROAD = 8.0;
static const float T_CELL_CONIFER = 5.12;

// Cover-weighted bilinear canopy at page-sample coordinates `s`: cover, height (m), conifer
// share, data share (0 = no source data here; as ed_geo `CanopyPage::bilinear`, data bilinear).
float4 t_canopy_bilin(uint pool, uint off, float2 s) {
    s = clamp(s, 0.0, float(T_PAGE_N - 1));
    int2 s0 = int2(min(floor(s), float2(float(T_PAGE_N - 2))));
    float2 t = s - float2(s0);
    float cover = 0.0;
    float hs = 0.0;
    float hw = 0.0;
    float mx = 0.0;
    float un = 0.0;
    [ForceUnroll]
    for (uint k = 0; k < 4; k++) {
        uint c = t_canopy_raw(pool, off, uint(s0.x) + (k & 1u), uint(s0.y) + (k >> 1));
        float wk = ((k & 1u) != 0 ? t.x : 1.0 - t.x) * ((k >> 1) != 0 ? t.y : 1.0 - t.y);
        float cc = float(c & 0xFFu) / 255.0;
        float ww = wk * cc;
        cover += wk * cc;
        hs += ww * t_canopy_h((c >> 8) & 0xFFu);
        mx += ww * float((c >> 16) & 0xFFu) / 255.0;
        un += wk * float(c >> 30) / 3.0;
        hw += ww;
    }
    return hw > 0.0 ? float4(cover, hs / hw, mx / hw, un) : float4(cover, 0.0, 0.0, un);
}

// Nearest crown of a jittered lattice (`cell` m, period 4096 m) around face metres `w`: a cell
// holds a crown when its hash is under `pres`; radius `rad` (cells). Returns xy: the offset
// from the crown centre in radii, z: distance in radii (9: none), w: the crown's hash.
float4 t_crown(float2 w, float cell, float pres, float rad, uint salt) {
    float2 x = w / cell;
    float2 c0 = floor(x);
    int per = int(4096.0 / cell + 0.5);
    float4 best = float4(0.0, 0.0, 9.0, 0.0);
    [ForceUnroll]
    for (int k = 0; k < 9; k++) {
        float2 c = c0 + float2(float(k % 3 - 1), float(k / 3 - 1));
        int2 ci = int2(c);
        ci = int2(((ci.x % per) + per) % per, ((ci.y % per) + per) % per);
        float h0 = t_hash2(ci + int2(int(salt), 0));
        if (h0 >= pres) {
            continue;
        }
        float h1 = t_hash2(ci + int2(17, int(salt) + 5));
        float h2 = t_hash2(ci + int2(int(salt) + 9, 31));
        float r = rad * lerp(0.6, 1.35, h2);
        float2 ctr = c + 0.5 + (float2(h1, t_hash2(ci + int2(int(salt) + 3, 7))) - 0.5) * 0.8;
        float2 q = (x - ctr) / r;
        float dq = length(q);
        if (dq < best.z) {
            best = float4(q, dq, h1);
        }
    }
    return best;
}

struct TForest {
    float3 albedo;
    float3 n;
    float ao;
    float sun;    // sun visibility cap (crown and page shadows)
    float crown;  // crown share of the pixel (roughness)
};

// The canopy over a terrain pixel. `floor_alb`: the ground's albedo (the forest floor where
// cover is); `n`: the ground's shading normal; `px_m`: pixel footprint (m); `spacing`: page
// sample spacing (m).
TForest t_forest(FrameData* f, float alt_m, uint pool, TerrainDraw d, float4 cv, float2 g, float2 w, float3 tu, float3 tv, float3 up, float3 n, float3 floor_alb, float3 l, float px_m, float px_iso, float spacing, float near_px) {
    // Stand edges: the page's cover warped by noise (fixed scales, so no seam where the level
    // changes), so a forest outline is ragged rather than a staircase; only where cover is partial.
    float ew = t_noise(w + 911.0, 64.0) * 0.65 + t_noise(w + 271.0, 16.0) * 0.35;
    float c = saturate(cv.x + (ew - 0.5) * 1.6 * cv.x * (1.0 - cv.x));
    float hc = max(cv.y, 2.0);
    float mx = cv.z;
    // Seasons: the far rung follows the same table (the generic broadleaf and spruce rows), the
    // place's shift from the ground height only. Bare crowns fall back to branch and shadow.
    float3 crown_b = T_CROWN_BROAD;
    float3 crown_c = T_CROWN_CONIFER;
    if (f.season.cfg.x > 0.0) {
        LeafSeason sb = season_far(f, false, alt_m);
        LeafSeason sc = season_far(f, true, alt_m);
        crown_b = lerp(T_CROWN_BARE, season_tint(sb, crown_b), sb.leaves);
        crown_c = lerp(T_CROWN_BARE, season_tint(sc, crown_c), sc.leaves);
    }
    float3 leaf = lerp(crown_b, crown_c, mx);
    // Stand-scale variation (clumps, age, species patches): what reads at kilometres.
    float n1 = t_noise(w + 151.0, 32.0);
    float n2 = t_noise(w + 517.0, 128.0);
    float stand = lerp(0.8, 1.2, n1) * lerp(0.88, 1.12, n2);

    // Sun in face metres: horizontal direction over its elevation.
    float sin_el = max(dot(l, up), 0.06);
    float2 sun2 = float2(dot(l, tu), dot(l, tv));
    float2 toward = sun2 / sin_el;  // metres along the ground per metre of height

    // Page shadow (one tap, §4): the canopy toward the sun at the distance a crown top casts.
    float hs = max(hc, 20.0);
    float4 tap = t_canopy_bilin(pool, asuint(d.tex.z), g * float(T_PAGE_STRIDE) + toward * hs / max(spacing, 0.01));
    float page_sh = tap.x * smoothstep(0.4, 0.9, tap.y / hs);

    // Forest floor under the canopy: litter, sky dimmed by cover, sun through gaps.
    float3 fl = lerp(floor_alb, T_LITTER, 0.5 * c);
    float fl_ao = lerp(1.0, 0.5, c);

    // Crown lattices: broadleaf and conifer, each holding its share of the cover. A crown's
    // radius grows with canopy height; presence is set so the crowns cover `c` of the ground.
    // Closed canopies: the crowns grow into each other (gaps only where cover is).
    float rad = lerp(0.42, 0.62, saturate(hc / 30.0)) + 0.2 * smoothstep(0.6, 1.0, c);
    float area = 3.14159 * rad * rad;
    float pres = saturate(c / area);
    float pb = pres * (1.0 - mx);
    float pc = pres * mx;
    float cell = lerp(T_CELL_BROAD, T_CELL_CONIFER, mx);
    float crown_px = cell * rad * 2.0 / max(px_m, 1e-3);
    float det = smoothstep(1.5, 4.0, crown_px);       // individual crowns resolved
    float paint = smoothstep(80.0, 40.0, cell * rad * 2.0 / max(px_iso, 1e-3));  // crowns drawn on the ground at all
    // Near trees on (G4, `near_px` >= 0: the reference tree's projected height): real trees
    // stand here down to the hand-off (bank.rs HANDOFF_PX = T_LIFT_NONE_PX), so the ground is
    // forest floor; the crowns come back as the near trees dither out.
    bool near_on = near_px >= 0.0;
    if (near_on) {
        float4 far = ed_buf<FrameData>(ed_c.frame).imp.far;
        if (far.z > 0.0) {
            // Impostors (trees G5): each tree leaves at its hashed size in the same band, so
            // the painted crowns rise linearly as the share of trees still standing falls.
            paint = saturate((far.z - near_px) / max(far.z - far.w, 1e-3));
        } else {
            paint = 1.0 - smoothstep(T_LIFT_NONE_PX, T_LIFT_NONE_PX * 1.25, near_px);
        }
    }

    TForest o;
    // Far: crowns unresolved, an area mix of crowns (with their own shading folded in) and
    // floor in their shade.
    float3 crown_far = leaf * 0.82 * stand;
    float floor_sun_far = 1.0 - 0.92 * max(c, page_sh);
    float3 a_far = lerp(fl, crown_far, c);
    float ao_far = lerp(fl_ao, 0.85, c);
    float sun_far = lerp(floor_sun_far, 1.0, c);
    o.albedo = a_far;
    o.n = n;
    o.ao = ao_far;
    o.sun = sun_far;
    o.crown = c;
    if (det > 0.0 && (!near_on || paint > 0.0)) {
        float4 kb = pb > 0.0 ? t_crown(w, T_CELL_BROAD, pb, rad, 3u) : float4(0.0, 0.0, 9.0, 0.0);
        float4 kc = pc > 0.0 ? t_crown(w + 1000.0, T_CELL_CONIFER, pc, rad * 0.9, 11u) : float4(0.0, 0.0, 9.0, 0.0);
        bool conifer = kc.z < kb.z;
        float4 k = conifer ? kc : kb;
        float in_crown = smoothstep(1.0, 0.85, k.z);
        float3 a_near;
        float3 n_near;
        float ao_near;
        float sun_near;
        float3 t2 = tu * k.x + tv * k.y;  // outward from the crown axis, in radii
        float r2 = min(dot(k.xy, k.xy), 1.0);
        // Crown shape: a cone (conifer, steep sides) or a dome (broadleaf).
        float3 nc = conifer ? normalize(up + t2 * 1.3) : normalize(t2 + up * sqrt(max(1.0 - r2, 0.04)));
        float3 nn = normalize(n + (nc - up) * 0.9);
        float3 la = (conifer ? crown_c : crown_b) * lerp(0.75, 1.25, k.w) * stand;
        // Per-crown hue: yellow-green to blue-green (species and vigour within the family).
        la *= lerp(float3(1.12, 1.0, 0.75), float3(0.85, 0.97, 1.2), frac(k.w * 7.31));
        // Leaf clumps inside the crown (fades out as the crown shrinks on screen).
        la *= lerp(1.0, lerp(0.7, 1.25, t_noise(w * (conifer ? 1.6 : 1.0) + 13.0, 1.0)), smoothstep(6.0, 16.0, crown_px));
        // Self-shadow inside the crown: the far side from the sun and the crown's skirt.
        float self_sh = saturate(0.5 + 0.8 * dot(k.xy, sun2 / max(length(sun2), 1e-3)));
        float crown_ao = lerp(0.7, 1.0, 1.0 - r2) * lerp(1.0, 0.7, (1.0 - self_sh) * 0.6);
        // Gaps: crown shadows cast by the crowns toward the sun (mid-crown height).
        // Unresolved below ~3 px a crown: the area mix's floor shade instead (saves 18 hashes).
        float gap_sun = floor_sun_far;
        if (crown_px > 3.0) {
            float2 wt = w + toward * hc * 0.55;
            float4 sb = pb > 0.0 ? t_crown(wt, T_CELL_BROAD, pb, rad, 3u) : float4(0.0, 0.0, 9.0, 0.0);
            float4 sc = pc > 0.0 ? t_crown(wt + 1000.0, T_CELL_CONIFER, pc, rad * 0.9, 11u) : float4(0.0, 0.0, 9.0, 0.0);
            gap_sun = lerp(gap_sun, 1.0 - 0.92 * smoothstep(1.1, 0.7, min(sb.z, sc.z)), smoothstep(3.0, 6.0, crown_px));
        }
        gap_sun = min(gap_sun, 1.0 - 0.9 * page_sh);
        // Between crowns of a closed stand the eye meets lower crowns and crown flanks in
        // shade, not the floor (oblique views hide the floor where cover is high).
        float closed = smoothstep(0.5, 0.95, c);
        float3 fl_gap = lerp(fl, leaf * stand * 0.85, closed);
        gap_sun = lerp(gap_sun, max(gap_sun, 0.35), closed);
        a_near = lerp(fl_gap, la, in_crown);
        n_near = normalize(lerp(n, nn, in_crown));
        ao_near = lerp(lerp(fl_ao, 0.75, closed), crown_ao, in_crown);
        sun_near = lerp(gap_sun, 1.0, in_crown);
        o.albedo = lerp(o.albedo, a_near, det);
        o.n = normalize(lerp(o.n, n_near, det));
        o.ao = lerp(o.ao, ao_near, det);
        o.sun = lerp(o.sun, sun_near, det);
        o.crown = lerp(o.crown, in_crown, det);
    }
    if (paint < 1.0) {
        // Near: no trees are drawn yet (G4/G5 bring them); the ground reads as forest floor,
        // dappled where the canopy lets light through.
        float dap = smoothstep(0.35, 0.75, t_noise(w + 77.0, 4.0) * 0.6 + t_noise(w + 33.0, 1.0) * 0.4);
        float sun_floor = min(lerp(1.0, 0.4 + 0.6 * dap, c), 1.0 - 0.6 * page_sh);
        o.albedo = lerp(fl, o.albedo, paint);
        o.n = normalize(lerp(n, o.n, paint));
        // Until G4/G5 draw the near trees the floor is seen as if lit from the stand's edge:
        // dimmed, never black.
        float ao_crown = o.ao;
        float sun_crown = o.sun;
        o.ao = lerp(max(fl_ao, 0.75), o.ao, paint);
        o.sun = lerp(sun_floor, o.sun, paint);
        if (near_on) {
            // The near trees' own shadows light the floor; the sky they hide is left to the
            // screen-space AO. The page's cover says nothing about the stand's own gaps (stream
            // beds, rock, `GAP_STRENGTH`), and a cover-darkened floor there showed as black
            // streaks once impostors replaced the overhanging meshes.
            o.ao = lerp(max(fl_ao, 0.85), ao_crown, paint);
            o.sun = lerp(1.0 - 0.5 * page_sh, sun_crown, paint);
        }
        o.crown = o.crown * paint;
    }
    return o;
}

// ---- Grass shells (styles C and D) --------------------------------------------------------
// A shell pixel: the shell's triangle (the drawn facet lifted radially by the shell height)
// refetched as terrain does, the same strand test for its attributes, then shaded like a blade:
// root -> mid -> tip up the strand, darker low in the turf, the type's sheen.
float4 shell_pixel(FrameData* f, FsOut i, uint2 vis, float3 dir) {
    TerrainDraw d = ed_buf<TerrainDraw>(ed_c.t_draws)[vis.x & SHELL_DRAW_MASK];
    uint pool = ed_c.t_pool;
    uint buf = ed_c.t_draws;
    uint L = (vis.x >> SHELL_L_SHIFT) & 7u;
    uint k = (vis.y >> SHELL_K_SHIFT) & 15u;
    uint n = max(f.shell.cfg.w, 1u);
    float hk = f.shell.fu.w * float(k + 1u) / float(n);
    uint3 tri = t_triangle(vis.y & SHELL_PRIM_MASK);
    float3 bc = d.origin.xyz - d.dir_c.xyz * d.dir_c.w;
    float3 a0 = t_vertex<0>(pool, d, tri.x);
    float3 a1 = t_vertex<0>(pool, d, tri.y);
    float3 a2 = t_vertex<0>(pool, d, tri.z);
    a0 += normalize(a0 - bc) * hk;
    a1 += normalize(a1 - bc) * hk;
    a2 += normalize(a2 - bc) * hk;
    float3 e1 = a1 - a0;
    float3 e2 = a2 - a0;
    float2 bb = ray_bary(dir, a0, e1, e2);
    float b1 = saturate(bb.x);
    float b2 = saturate(bb.y);
    float bs = b1 + b2;
    if (bs > 1.0) {
        b1 /= bs;
        b2 /= bs;
    }
    float b0 = 1.0 - b1 - b2;
    float3 p = a0 + e1 * b1 + e2 * b2;
    float2 g = t_vertex_grid(tri.x) * b0 + t_vertex_grid(tri.y) * b1 + t_vertex_grid(tri.z) * b2;
    float3 up;
    float h;
    float3 nt = t_smooth_normal(pool, d, g, 2, up, h);
    float2 wrel = shell_wrel(f, d, g);
    float dz = -mul(f.view, float4(p, 1.0)).z;
    ShellHit s = shell_strand(f, buf, wrel, k, n, L, dz, true);
    GrassType ty = shell_type(f, buf, s.kind);
    // Rounded strands: the ground normal bent out from the strand axis (less far away, where a
    // pixel spans many strands and the turf reads as a surface).
    float3 tu = normalize(d.fu.xyz - up * dot(d.fu.xyz, up));
    float3 tv = normalize(d.fv.xyz - up * dot(d.fv.xyz, up));
    float far = smoothstep(3.0, f.shell.c.z * 0.6, dz);
    float3 sn = tu * s.side.x + tv * s.side.y;
    float3 nrm = normalize(nt + sn * (0.9 * (1.0 - 0.5 * s.tt) * (1.0 - far)));
    float3 v = -dir;
    if (dot(nrm, v) < 0.0) {
        nrm = normalize(nrm - 1.01 * dot(nrm, v) * v);
    }
    float tt = s.tt;
    float3 alb = tt < 0.5 ? lerp(ty.root.rgb, ty.mid.rgb, tt * 2.0) : lerp(ty.mid.rgb, ty.tip.rgb, (tt - 0.5) * 2.0);
    float hue = grass_rand(grass_hash(asuint(s.rnd) + 53u)) - 0.5;
    alb *= lerp(0.84, 1.12, s.rnd) * float3(1.0 + 0.10 * hue, 1.0, 1.0 - 0.12 * hue);
    if (s.dry > 0.5) {
        alb = lerp(alb, ty.dry.rgb * lerp(0.85, 1.1, s.rnd), lerp(0.5, 1.0, tt));
    }
    alb = min(alb, float3(0.9, 0.9, 0.9));
    // Turf self-shadowing: dark at the roots, open at the tips; far turf averages it.
    float ao_h = lerp(0.4, 1.0, pow(tt, 0.8));
    ao_h = lerp(ao_h, 0.72, far * 0.6);
    float sun_vis = sun_shadow(f, ed_c.shadow, p, up, dz, i.pos.xy);
    float2 occ = ao_upsample(f, ed_c.ao, i.pos.xy, dz);
    float3 sun = sun_light_at(f, p);
    float3 col = shade_foliage_k(f, sun, alb, 0.55, occ.x * ao_h, occ.y, sun_vis * lerp(0.75, 1.0, tt), nrm, nrm, v, true, ty.shell.w);
    col = apply_aerial(f, col, p, i.pos.xy);
    if (f.light.flags.w == 2u) {
        col = float3(0.5 * occ.x * ao_h);
    } else if (f.light.flags.w == 4u) {
        col = float3(0.5 * occ.y);
    } else if (f.light.flags.w == 1u) {
        col = float3(0.04 + 0.5 * sun_vis);
    }
    return float4(col, 1.0);
}

// Terrain under and past the grass (styles C and D): the coarse map's carpet colour (bilinear,
// density weighted), keeping the ground texture's brightness variation within +-25 %.
float3 shell_tint(FrameData* f, TerrainDraw d, float2 g, float3 albedo) {
    float2 wr = shell_wrel(f, d, g);
    float cm = f.shell.cells.y;
    float2 q = wr / cm - 0.5;
    float2 fq = floor(q);
    float2 t = q - fq;
    int2 q0 = int2(fq);
    float3 cc = float3(0.0);
    float cov = 0.0;
    for (uint c = 0; c < 4u; c++) {
        int2 o = int2(int(c & 1u), int(c >> 1));
        float wb = (o.x != 0 ? t.x : 1.0 - t.x) * (o.y != 0 ? t.y : 1.0 - t.y);
        uint w = shell_map(f, ed_c.t_draws, 1u, q0 + o);
        if ((w & 0xFFu) != 0u) {
            float dens = float((w >> 8) & 0xFFu) / 255.0;
            GrassType ty = shell_type(f, ed_c.t_draws, (w & 0xFFu) - 1u);
            cc += ty.carpet.rgb * (wb * dens);
            cov += wb * dens;
        }
    }
    if (cov <= 0.0) {
        return albedo;
    }
    float3 tgt = cc / cov;
    float half_m = cm * f.shell.cells.z * 0.5;
    float edge = 1.0 - smoothstep(0.8, 0.97, max(abs(wr.x), abs(wr.y)) / half_m);
    const float3 LUM = float3(0.2126, 0.7152, 0.0722);
    tgt *= clamp(dot(albedo, LUM) / max(dot(tgt, LUM), 1e-4), 0.75, 1.25);
    return lerp(albedo, tgt, saturate(f.shell.c.w * cov * edge));
}

// A terrain pixel: the rasterised triangle refetched from the draw record and page pool (exact
// position on the drawn facet), a smooth normal from the height page, the slope/altitude tint,
// then the same light as every opaque pixel.
float4 terrain_pixel<let TR : int>(FrameData* f, FsOut i, uint2 vis, float3 dir) {
    if ((vis.y & SHELL_BIT) != 0u) {
        return shell_pixel(f, i, vis, dir);
    }
    bool trees_on = TR != 0;
    TerrainDraw d = ed_buf<TerrainDraw>(ed_c.t_draws)[vis.x & ~TERRAIN_VIS_BIT];
    uint pool = ed_c.t_pool;
    uint3 tri = t_triangle(vis.y & 0x0FFFFFFFu);
    float3 p0 = t_vertex<TR>(pool, d, tri.x);
    float3 e1 = t_vertex<TR>(pool, d, tri.y) - p0;
    float3 e2 = t_vertex<TR>(pool, d, tri.z) - p0;
    float2 bb = ray_bary(dir, p0, e1, e2);
    float b1 = saturate(bb.x);
    float b2 = saturate(bb.y);
    float bs = b1 + b2;
    if (bs > 1.0) {
        b1 /= bs;
        b2 /= bs;
    }
    float b0 = 1.0 - b1 - b2;
    float3 p = p0 + e1 * b1 + e2 * b2;  // camera-relative
    float2 g = t_vertex_grid(tri.x) * b0 + t_vertex_grid(tri.y) * b1 + t_vertex_grid(tri.z) * b2;
    float3 up;
    float h;
    // Shading normal over a ~2.4 m baseline (+-1.2 m): +-1 sample of a 0.6 m page turns the
    // 1/8 m height step into ~6 degrees of normal banding; the source is 6.6 m anyway.
    float spacing = max(t_sample_spacing(d, g), 0.01);
    int rn = clamp(int(1.2 / spacing + 0.5), 1, 4);
    float3 n = t_smooth_normal(pool, d, g, rn, up, h);
    // Material slope over ~T_SLOPE_M: the landform, not source noise (lossy DEM tiles).
    int r = clamp(int(T_SLOPE_M / spacing + 0.5), 1, 6);
    float3 nm = r == rn ? n : t_smooth_normal(pool, d, g, r, up, h);
    float3 albedo = terrain_albedo(h, dot(nm, up));
    // Far forest rung (trees G3): the canopy page under the pixel.
    bool trees = trees_on && (d.slots.z & T_TREES) != 0u;
    float4 cv = float4(0.0);
    if (trees && asuint(d.tex.z) != 0u) {
        cv = t_canopy_bilin(pool, asuint(d.tex.z), g * float(T_PAGE_STRIDE));
    }
    float rough = T_ROUGH;
    float mat_ao = 1.0;
    if (ed_c.t_mat != 0u) {
        TerrainMat m = ed_buf<TerrainMat>(ed_c.t_draws)[ed_c.t_mat];
        // World-stable face metres and their per-pixel steps (ray differentials on the facet).
        float cell_m = d.shape.x * d.shape.y;
        float2 w = d.tex.xy + (g - 32.0) * cell_m;
        float3 tu = normalize(d.fu.xyz - up * dot(d.fu.xyz, up));
        float3 tv = normalize(d.fv.xyz - up * dot(d.fv.xyz, up));
        float2 bx = ray_bary(view_dir(f, i.pos.xy + float2(1.0, 0.0)), p0, e1, e2);
        float2 by = ray_bary(view_dir(f, i.pos.xy + float2(0.0, 1.0)), p0, e1, e2);
        float3 dpx = p0 + e1 * bx.x + e2 * bx.y - p;
        float3 dpy = p0 + e1 * by.x + e2 * by.y - p;
        float2 dwx = float2(dot(dpx, tu), dot(dpx, tv));
        float2 dwy = float2(dot(dpy, tu), dot(dpy, tv));
        // Curvature: Laplacian of the bilinear page over ~8 m (landform) and ~2 m (fine, near
        // only). Bilinear taps keep it smooth (a nearest-sample stencil steps per sample).
        float2 sp = g * float(T_PAGE_STRIDE);
        int rl = clamp(int(8.0 / spacing + 0.5), 1, 16);
        int rf = clamp(int(2.0 / spacing + 0.5), 1, 4);
        float lap = t_page_bilin(pool, d, sp + float2(rl, 0)) + t_page_bilin(pool, d, sp - float2(rl, 0)) + t_page_bilin(pool, d, sp + float2(0, rl)) + t_page_bilin(pool, d, sp - float2(0, rl)) - 4.0 * h;
        float lapf = 0.0;
        if (length_squared(p) < 400.0 * 400.0) {
            lapf = t_page_bilin(pool, d, sp + float2(rf, 0)) + t_page_bilin(pool, d, sp - float2(rf, 0)) + t_page_bilin(pool, d, sp + float2(0, rf)) + t_page_bilin(pool, d, sp - float2(0, rf)) - 4.0 * h;
        }
        float dl = float(rl) * spacing;
        float df = float(rf) * spacing;
        TSurf s = terrain_material(f, m, d, n, nm, up, h, lap / (dl * dl), lapf / (df * df), w, dwx, dwy, dot(dpx, up), dot(dpy, up), tu, tv, length(p), trees, cv, spacing);
        albedo = s.albedo;
        n = s.n;
        rough = s.rough;
        mat_ao = s.ao;
    }
    float sun_k = 1.0;
    if (trees && cv.x > 0.004) {
        float cell_m = d.shape.x * d.shape.y;
        float2 w = d.tex.xy + (g - 32.0) * cell_m;
        float3 tu = normalize(d.fu.xyz - up * dot(d.fu.xyz, up));
        float3 tv = normalize(d.fv.xyz - up * dot(d.fv.xyz, up));
        // Pixel footprint on the ground (m): the pixel's angle times distance, stretched by the
        // grazing angle (geometric mean of the two footprint axes).
        float pa = length(view_dir(f, i.pos.xy + float2(1.0, 0.0)) - dir);
        float px_iso = length(p) * pa;
        float px_m = px_iso / sqrt(max(abs(dot(dir, n)), 0.03));
        float near_px = f.wind.cfg.z > 0.5 && d.polar.w > 0.0 ? d.polar.w / max(length(p), 1.0) : -1.0;
        TForest fo = t_forest(f, h, pool, d, cv, g, w, tu, tv, up, n, albedo, f.sun_dir.xyz, px_m, px_iso, spacing, near_px);
        albedo = fo.albedo;
        n = fo.n;
        rough = lerp(rough, T_CROWN_ROUGH, fo.crown);
        mat_ao *= fo.ao;
        sun_k = fo.sun;
    }
    if ((f.shell.cfg.x & 2u) != 0u && shell_face(f, d)) {
        albedo = shell_tint(f, d, g, albedo);
    }
    float3 v = -dir;
    if (dot(n, v) < 0.0) {
        n = normalize(n - 1.01 * dot(n, v) * v);
    }
    float zlin = -mul(f.view, float4(p, 1.0)).z;
    float3 ng = cross(e1, e2);
    ng = length_squared(ng) > 1e-20 ? normalize(ng) : n;
    if (dot(ng, v) < 0.0) {
        ng = -ng;
    }
    float sun_vis = sun_shadow(f, ed_c.shadow, p, ng, zlin, i.pos.xy);
    if (trees) {
        sun_vis = min(sun_vis, sun_k);
    }
    float2 occ = ao_upsample(f, ed_c.ao, i.pos.xy, zlin);
    float ao = occ.x * mat_ao;
    float sv = occ.y;
    float3 sun = sun_light_at(f, p);
    float3 col = shade_pbr(f, sun, albedo, 0.0, rough, ao, sv, sun_vis, n, v);
    col = apply_aerial(f, col, p, i.pos.xy);
    if (f.light.flags.w == 1u) {
        static const float3 TINT[5] = { float3(1.0, 0.25, 0.25), float3(0.25, 1.0, 0.25), float3(0.3, 0.45, 1.0), float3(1.0, 1.0, 0.25), float3(0.6, 0.6, 0.6) };
        col = TINT[shadow_cascade(f, p, zlin)] * (0.04 + 0.5 * sun_vis);
    } else if (f.light.flags.w == 2u) {
        col = float3(0.5 * ao);
    } else if (f.light.flags.w == 4u) {
        col = float3(0.5 * sv);
    }
    if (trees && (d.slots.z & T_TREES_DEBUG) != 0u && cv.x > 0.0) {
        // EW_TREES_FAR=debug: red = canopy height (40 m full), green = cover, blue = conifer share.
        col = lerp(col, float3(saturate(cv.y / 40.0), cv.x, cv.z) * 0.6, 0.75);
    }
    return float4(col, 1.0);
}

// The resolve; `terrain` is a compile-time constant per entry point, so `fs_resolve` (no
// terrain provider) compiles to exactly the code it had before terrain existed.
// ---- Trees (G4) -----------------------------------------------------------------------------

// Leaf atlas mean colour (ed_world trees `LEAF_BASE`, linear): the atlas carries leaf shape and
// detail; the hue comes from the species table (bank.rs `leaf_albedo`).
static const float3 LEAF_BASE_LIN = float3(0.147, 0.274, 0.051);

struct TreeSurf {
    float3 albedo;
    bool leaf;
    bool shell;  // the crown shell (LOD 2): neither a card nor bark
};

float3 tree_hue(float h) {
    return h >= 0.0 ? lerp(float3(1.0, 1.0, 1.0), float3(0.85, 0.97, 1.2), h) : lerp(float3(1.0, 1.0, 1.0), float3(1.12, 1.0, 0.75), -h);
}

// Bark relief (look pass): conifer and broadleaf bark is plates split by darker fissures,
// mottled by lichen and darker and wetter at the butt. Procedural on the mesh-space position
// (cylindrical around the trunk axis, metres): (arc length, height). Returns the plate
// height 0..1; the caller tints the albedo and tilts the normal by its gradient.
float bark_height(float2 q) {
    // Plates: cells ~7 cm wide, ~25 cm tall; fissures where the stretched noise is low.
    float plates = t_noise(float2(q.x, q.y * 0.3), 0.07);
    float w = t_noise(q * float2(1.0, 0.25) + 31.0, 0.18) - 0.5;
    float fiss = smoothstep(0.22, 0.52, t_noise(float2(q.x + w * 0.08, q.y * 0.12), 0.035));
    return saturate(fiss * (0.55 + 0.45 * plates));
}

TreeSurf tree_surface(FrameData* f, GpuInstance inst, GpuMaterial m, uint2 w0, uint2 w1, uint2 w2, float b0, float b1, float b2, float2 pix, float3 p0, float3 e1, float3 e2) {
    TreeSurf o;
    uint a = w0.y;
    float val = (float(w0.y & 127u) * b0 + float(w1.y & 127u) * b1 + float(w2.y & 127u) * b2) * (2.0 / 127.0);
    float hue = (float((w0.y >> 7) & 15u) * b0 + float((w1.y >> 7) & 15u) * b1 + float((w2.y >> 7) & 15u) * b2) * (2.0 / 15.0) - 1.0;
    uint user = inst.pad[0];
    float iv = float(user & 255u) / 128.0;
    float ih = float((user >> 8) & 255u) / 127.5 - 1.0;
    o.leaf = (a & AUX_CARD) != 0u;
    o.shell = false;
    if ((a & AUX_BARK) != 0u) {
        o.albedo = m.shade.rgb * val * lerp(1.0, iv, 0.5);
        return o;
    }
    float3 alb = m.base.rgb * val * iv * tree_hue(clamp(hue + 0.5 * ih, -1.0, 1.0));
    if (o.leaf && m.tex.x != NO_TEXTURE) {
        float2 t0 = tree_uv(w0.x);
        float2 d1 = tree_uv(w1.x) - t0;
        float2 d2 = tree_uv(w2.x) - t0;
        float2 uv = t0 + d1 * b1 + d2 * b2;
        float2 bx = ray_bary(view_dir(f, pix + float2(1.0, 0.0)), p0, e1, e2);
        float2 by = ray_bary(view_dir(f, pix + float2(0.0, 1.0)), p0, e1, e2);
        float2 ux = t0 + d1 * bx.x + d2 * bx.y - uv;
        float2 uy = t0 + d1 * by.x + d2 * by.y - uv;
        float3 tex = ed_tex2d(m.tex.x).SampleGrad(ed_smp(ed_c.smp), uv, ux, uy).rgb;
        alb *= clamp(tex / LEAF_BASE_LIN, float3(0.2, 0.2, 0.2), float3(2.5, 2.5, 2.5));
    }
    if (f.season.cfg.x > 0.0) {
        alb = season_tint(season_of_word(f, inst.pad[1]), alb);
    }
    o.shell = !o.leaf;
    o.albedo = min(alb, float3(0.9, 0.9, 0.9));
    return o;
}

// Foliage: diffuse-dominant GGX plus light through the leaves (thin translucent cards: the
// sun from behind a leaf comes through yellow-green, strongest looking towards the sun).
float3 shade_foliage(FrameData* f, float3 sun, float3 albedo, float rough, float ao, float sv, float sun_vis, float3 n, float3 ng, float3 v, bool leaf) {
    return shade_foliage_k(f, sun, albedo, rough, ao, sv, sun_vis, n, ng, v, leaf, 1.0);
}

float3 shade_foliage_k(FrameData* f, float3 sun, float3 albedo, float rough, float ao, float sv, float sun_vis, float3 n, float3 ng, float3 v, bool leaf, float spec_k) {
    float3 col = shade_pbr_k(f, sun, albedo, 0.0, rough, ao, sv, sun_vis, n, v, spec_k);
    if (leaf) {
        float3 l = f.sun_dir.xyz;
        float back = saturate(-dot(n, l));
        float fwd = pow(saturate(dot(-v, l)), 4.0);
        float3 trans = albedo * float3(1.15, 1.25, 0.55);
        col += sun * sun_vis * trans * (back * (0.45 + 1.2 * fwd)) * (1.0 / 3.14159265);
        // Skylight through the leaf: thin needles and leaves pass ~10-15 % of what lands on
        // their far side, so a shaded card is lit from behind by the sky, not only in front.
        // The cheap two-colour hemisphere (no SH loads): it is a fill term, 0.3 weighted.
        col += trans * hemi_ambient(f, -n) * lerp(f.light.skyvis.y, 1.0, sv) * ao * 0.3;
    }
    return col;
}

// ---- Tree impostors (trees G5) -----------------------------------------------------------
// The impostor raster wrote the tree index; recompute the same three frames, the ray's uvs on
// each, and blend the baked surface (coverage-weighted): depth, normal, value, bark. Shaded with
// the meshes' foliage model and their colour table (leaf/bark albedo, per-tree value and hue).
float4 impostor_pixel(FrameData* f, FsOut i, uint idx, float3 dir) {
    GpuTree g = ed_buf<GpuTree>(f.imp.bufs.x)[idx];
    GpuImpKind k = ed_buf<GpuImpKind>(f.imp.bufs.y)[(g.node_kind >> 12) & 0xFFFu];
    float3 node = ed_buf<float4>(ed_c.nodes)[g.node_kind & 0xFFFu].xyz;
    ImpTree t = imp_tree(g, k, node, f.sun_dir.xyz, false);
    float3 nrm = normalize(t.c);
    float hit = dot(t.c, nrm) / max(dot(dir, nrm), 1e-4);
    float3 pq = dir * hit;
    float3 q = imp_to_tree(t, pq - t.c);
    float half_t = t.r * f.imp.cfg.y;
    float2 u0 = imp_frame_uv(t, t.d0, q, half_t);
    float2 u1 = imp_frame_uv(t, t.d1, q, half_t);
    float2 u2 = imp_frame_uv(t, t.d2, q, half_t);
    uint n = k.tex.w;
    float fpx = k.geo.w;
    float ppr = f.proj.w * 0.5 * f.proj.y;
    float texel_m = 2.0 * half_t * t.s * max(t.wd, 1.0) / fpx;
    float lod = clamp(log2(max(hit / ppr, 1e-6) / texel_m), 0.0, k.leaf.w - 1.0);
    SamplerState smp = ed_smp(ed_c.smp);
    Texture2D ta = ed_tex2d(k.tex.x);
    Texture2D tb = ed_tex2d(k.tex.y);
    Texture2D tc = ed_tex2d(k.tex.z);
    float2 a0 = imp_atlas_uv(t.fi.x, n, u0, fpx);
    float2 a1 = imp_atlas_uv(t.fi.y, n, u1, fpx);
    float2 a2 = imp_atlas_uv(t.fi.z, n, u2, fpx);
    float2 s0 = ta.SampleLevel(smp, a0, lod).rg;
    float2 s1 = ta.SampleLevel(smp, a1, lod).rg;
    float2 s2 = ta.SampleLevel(smp, a2, lod).rg;
    float3 wc = t.w * float3(s0.x * (imp_inside(u0) ? 1.0 : 0.0), s1.x * (imp_inside(u1) ? 1.0 : 0.0), s2.x * (imp_inside(u2) ? 1.0 : 0.0));
    float ws = wc.x + wc.y + wc.z;
    wc = ws > 1e-4 ? wc / ws : t.w;
    float d01 = wc.x * s0.y + wc.y * s1.y + wc.z * s2.y;
    float3 nt = wc.x * imp_oct_sphere(tb.SampleLevel(smp, a0, lod).rg) + wc.y * imp_oct_sphere(tb.SampleLevel(smp, a1, lod).rg) + wc.z * imp_oct_sphere(tb.SampleLevel(smp, a2, lod).rg);
    float2 vb = wc.x * tc.SampleLevel(smp, a0, lod).rg + wc.y * tc.SampleLevel(smp, a1, lod).rg + wc.z * tc.SampleLevel(smp, a2, lod).rg;
    float dz = (d01 - 0.5) * 2.0 * half_t * t.s;
    float3 p = pq - dir * dz;
    float3 nw = t.ex * (nt.x / t.wd) + t.up * nt.y + t.ez * (nt.z / t.wd);
    nw = dot(nw, nw) > 1e-8 ? normalize(nw) : -dir;
    float3 v = -dir;
    if (dot(nw, v) < 0.0) {
        nw = normalize(nw - 1.01 * dot(nw, v) * v);
    }
    float value = vb.x * f.imp.cfg.z;
    float bark = saturate(vb.y);
    float iv = float(g.user & 255u) / 128.0;
    float ih = float((g.user >> 8) & 255u) / 127.5 - 1.0;
    float3 leaf_alb = k.leaf.rgb * value * iv * tree_hue(clamp(ih, -1.0, 1.0));
    if (f.season.cfg.x > 0.0) {
        leaf_alb = season_tint(season_of_word(f, g.season), leaf_alb);
    }
    float3 bark_alb = k.bark.rgb * value * lerp(1.0, iv, 0.5);
    float3 albedo = min(lerp(leaf_alb, bark_alb, bark), float3(0.9, 0.9, 0.9));
    float zlin = -mul(f.view, float4(p, 1.0)).z;
    float sun_vis = sun_shadow(f, ed_c.shadow, p, nw, zlin, i.pos.xy);
    float2 occ = ao_upsample(f, ed_c.ao, i.pos.xy, zlin);
    float3 sun = sun_light_at(f, p);
    float3 col = shade_foliage(f, sun, albedo, 0.82, occ.x, occ.y, sun_vis, nw, nw, v, bark < 0.5);
    col = apply_aerial(f, col, p, i.pos.xy);
    if (f.light.flags.w == 2u) {
        col = float3(0.5 * occ.x);
    } else if (f.light.flags.w == 4u) {
        col = float3(0.5 * occ.y);
    } else if (f.light.flags.w == 1u) {
        col = float3(0.04 + 0.5 * sun_vis);
    }
    return float4(col, 1.0);
}

// ---- Grass style A (src/grass.rs) ---------------------------------------------------------
// The raster wrote the clump, the blade and (t, u) on it: rebuild the same blade, put the pixel
// on the blade's face along the view ray, and shade it as a thin translucent leaf with a rounded
// normal bent towards the ground (more with distance, so far grass reads as a soft carpet).
float4 grass_pixel(FrameData* f, FsOut i, uint2 vis, float3 dir) {
    GrassClump c = ed_buf<GrassClump>(f.grass.bufs.x)[vis.x & GRASS_INDEX_MASK];
    GrassType ty = ed_buf<GrassType>(f.grass.bufs.y)[(c.node_kind >> 12) & 0xFFu];
    float3 node = ed_buf<float4>(ed_c.nodes)[c.node_kind & 0xFFFu].xyz;
    uint k = vis.y & 127u;
    float t = float((vis.y >> 7) & 4095u) / 4095.0;
    float u = float((vis.y >> 19) & 255u) / 255.0;
    GrassBlade b = grass_blade(f, c, ty, node, k, 1.0);
    float3 pt = grass_point(b, t);
    float3 sd = grass_view_side(b, t, pt);
    float3 tg = normalize(grass_tangent(b, t) + b.up * 1e-4);
    float3 v = -dir;
    float3 n0 = normalize(cross(sd, tg));
    n0 = dot(n0, v) < 0.0 ? -n0 : n0;
    float dn = dot(dir, n0);
    float lp = length(pt);
    float hit = abs(dn) > 1e-3 ? dot(pt, n0) / dn : lp;
    float3 p = dir * clamp(hit, lp - 0.05, lp + 0.05);
    float d = length(p);
    // Rounded across the blade, bent towards the ground normal (far: mostly the ground).
    float far = smoothstep(f.grass.lod.x * 0.5, f.grass.lod.y, d);
    float3 nr = normalize(n0 + sd * ((u - 0.5) * 1.1));
    float3 n = normalize(lerp(nr, b.up, lerp(f.grass.look.x, 0.8, far)));
    // Colour: root -> mid -> tip, seed heads in the tip colour, clump and blade variation, dry blades.
    float3 alb = t < 0.5 ? lerp(ty.root.rgb, ty.mid.rgb, t * 2.0) : lerp(ty.mid.rgb, ty.tip.rgb, (t - 0.5) * 2.0);
    if (b.head > 0.0) {
        alb = lerp(alb, ty.tip.rgb * 1.1, smoothstep(1.0 - b.head - 0.04, 1.0 - b.head, t));
    }
    uint cs = c.seed;
    float cv = lerp(0.86, 1.12, grass_rand(grass_hash(cs + 41u)));
    float hue = grass_rand(grass_hash(cs + 53u)) - 0.5;
    alb *= cv * b.shade * float3(1.0 + 0.10 * hue, 1.0, 1.0 - 0.12 * hue);
    float dry = float((c.hw >> 16) & 255u) / 255.0;
    float bd = grass_rand(grass_hash(cs ^ (k * 0x85EBCA6Bu + 0x1234567u)));
    if (bd < dry) {
        alb = lerp(alb, ty.dry.rgb * lerp(0.85, 1.1, bd / max(dry, 1e-3)), lerp(0.55, 1.0, t));
    }
    alb = min(alb, float3(0.9, 0.9, 0.9));
    // Inside the clump the roots are in shade (deeper for tall grass); far grass averages it.
    float ao_r = lerp(0.55, 0.28, b.t_ao);
    float ao_b = lerp(ao_r, 1.0, smoothstep(0.0, 0.75, t));
    ao_b = lerp(ao_b, lerp(ao_r, 1.0, 0.7), far);
    float zlin = -mul(f.view, float4(p, 1.0)).z;
    float sun_vis = sun_shadow(f, ed_c.shadow, p, b.up, zlin, i.pos.xy);
    float2 occ = ao_upsample(f, ed_c.ao, i.pos.xy, zlin);
    float3 sun = sun_light_at(f, p);
    float3 col = shade_foliage_k(f, sun, alb, 0.5, occ.x * ao_b, occ.y, sun_vis, n, n, v, true, ty.shell.w);
    col = apply_aerial(f, col, p, i.pos.xy);
    if (f.light.flags.w == 2u) {
        col = float3(0.5 * occ.x * ao_b);
    } else if (f.light.flags.w == 4u) {
        col = float3(0.5 * occ.y);
    } else if (f.light.flags.w == 1u) {
        col = float3(0.04 + 0.5 * sun_vis);
    }
    return float4(col, 1.0);
}

float4 resolve_px<let TR : int>(FsOut i, bool terrain) {
    FrameData* f = ed_buf<FrameData>(ed_c.frame);
    uint2 px = uint2(i.pos.xy);
    uint2 vis = ed_tex2d_uint(ed_c.vis).Load(int3(int2(px), 0)).xy;
    float3 dir = view_dir(f, i.pos.xy);
    if (vis.x == 0) {
        return float4(sky_pixel(f, dir), 1.0);
    }
    if (terrain && (vis.x & TERRAIN_VIS_BIT) != 0) {
        return terrain_pixel<TR>(f, i, vis, dir);
    }
    if ((vis.x & (GRASS_VIS_BIT | IMP_VIS_BIT | TERRAIN_VIS_BIT)) == GRASS_VIS_BIT) {
        return grass_pixel(f, i, vis, dir);
    }
    if ((vis.x & (IMP_VIS_BIT | TERRAIN_VIS_BIT)) == IMP_VIS_BIT) {
        return impostor_pixel(f, i, vis.x & ~IMP_VIS_BIT, dir);
    }
    GpuInstance inst = ed_buf<GpuInstance>(ed_c.instances)[vis.x - 1u];
    GpuMesh* mesh = ed_buf<GpuMesh>(ed_c.meshes) + inst.mesh;
    uint lod = vis.y >> 28;
    uint tris = max(mesh.index_count[lod] / 3u, 1u);
    uint prim = (vis.y & 0x0FFFFFFFu) % tris;
    uint* idx = ed_buf<uint>(ed_c.indices) + mesh.first_index[lod] + prim * 3u;
    Vtx* verts = ed_buf<Vtx>(ed_c.verts);
    int bv = mesh.base_vertex[lod];
    int i0 = int(idx[0]) + bv;
    int i1 = int(idx[1]) + bv;
    int i2 = int(idx[2]) + bv;
    Vtx v0 = verts[i0];
    Vtx v1 = verts[i1];
    Vtx v2 = verts[i2];
    float3 node = ed_buf<float4>(ed_c.nodes)[inst.node_flags & 0xFFFFu].xyz;
    bool foliage = (mesh.flags & MESH_FOLIAGE) != 0u;
    uint2 tw0 = uint2(0u, 0u);
    uint2 tw1 = uint2(0u, 0u);
    uint2 tw2 = uint2(0u, 0u);
    float3 l0, l1, l2;
    if (foliage) {
        // Trees: the same moved vertices the raster drew (common.slang `tree_vertex`).
        uint2* tw = ed_buf<uint2>(ed_c.uvs);
        tw0 = tw[i0];
        tw1 = tw[i1];
        tw2 = tw[i2];
        l0 = tree_vertex(f, inst, node, float3(v0.px, v0.py, v0.pz), v0.n, tw0.y);
        l1 = tree_vertex(f, inst, node, float3(v1.px, v1.py, v1.pz), v1.n, tw1.y);
        l2 = tree_vertex(f, inst, node, float3(v2.px, v2.py, v2.pz), v2.n, tw2.y);
    } else {
        l0 = xform_point(inst, float3(v0.px, v0.py, v0.pz));
        l1 = xform_point(inst, float3(v1.px, v1.py, v1.pz));
        l2 = xform_point(inst, float3(v2.px, v2.py, v2.pz));
    }
    // Ray from the camera (origin, camera-relative space) through the pixel.
    float3 p0 = node + l0;
    float3 e1 = l1 - l0;
    float3 e2 = l2 - l0;
    float2 bb = ray_bary(dir, p0, e1, e2);
    float b1 = saturate(bb.x);
    float b2 = saturate(bb.y);
    float bs = b1 + b2;
    if (bs > 1.0) {
        b1 /= bs;
        b2 /= bs;
    }
    float b0 = 1.0 - b1 - b2;
    float3 local = l0 * b0 + l1 * b1 + l2 * b2;  // node-local position
    float3 nm = oct_decode(v0.n) * b0 + oct_decode(v1.n) * b1 + oct_decode(v2.n) * b2;
    float3 n = normalize(xform_normal(inst, nm));
    float3 v = -dir;
    GpuMaterial m = ed_buf<GpuMaterial>(ed_c.materials)[inst.material];
    float3 albedo = m.base.rgb;
    float metal = m.params.x;
    float rough = m.params.y;
    float ao = 1.0;
    bool textured = m.tex.x != NO_TEXTURE || m.tex.y != NO_TEXTURE || m.tex.z != NO_TEXTURE;
    bool leaf = false;
    if (foliage) {
        TreeSurf ts = tree_surface(f, inst, m, tw0, tw1, tw2, b0, b1, b2, i.pos.xy, p0, e1, e2);
        albedo = ts.albedo;
        leaf = ts.leaf;
        // Leaves: matte with a faint sheen (a smoother lobe greys sunlit crowns from above).
        rough = 0.82;
        metal = 0.0;
        if (ts.shell) {
            // The crown shell is a leaf mass, not a smooth hull: leaf clumps break its
            // shading (node-local position, stable), the normal leans to the crown's up
            // (sky-lit tops, the far rung's crown shading), and it is matte.
            float3 up = normalize(xform_normal(inst, float3(0.0, 1.0, 0.0)));
            float clump = t_noise(local.xz * 0.9 + local.y * 0.6, 1.0);
            albedo *= lerp(0.6, 1.15, clump) * 0.85;
            ao *= lerp(0.65, 1.0, clump);
            n = normalize(lerp(n, up, 0.4));
            rough = 1.0;
        }
        bool ground = (inst.node_flags & INSTANCE_GROUND) != 0u;
        if (ground) {
            // Ground cover (G7): small things, no bark relief; a soft mottle (sun-bleached and
            // damp patches) on node-local position so neighbouring tufts differ.
            float mot = t_noise(local.xz * 1.7 + local.y * 3.0, 1.0);
            albedo *= lerp(0.82, 1.12, mot);
            rough = leaf ? 0.75 : 0.9;
        } else if (!leaf && !ts.shell && length_squared(p0) < 60.0 * 60.0) {
            // Bark (near only: the relief is sub-pixel past ~60 m; the normal tilt past ~20 m).
            float3 pm = float3(v0.px, v0.py, v0.pz) * b0 + float3(v1.px, v1.py, v1.pz) * b1 + float3(v2.px, v2.py, v2.pz) * b2;
            float rr = max(length(pm.xz), 0.02);
            float ang = atan2(pm.z, pm.x);
            float2 q = float2(ang * rr, pm.y);
            float hb = bark_height(q);
            // Lichen / weathering: greyer, lighter patches; the butt darker (soil splash, wet).
            float lich = smoothstep(0.55, 0.85, t_noise(q * float2(1.0, 0.5) + 7.0, 0.45));
            float butt = lerp(0.72, 1.0, smoothstep(0.0, 1.2, pm.y));
            float3 grey = dot(albedo, float3(0.3, 0.5, 0.2)) * float3(1.05, 1.05, 1.0);
            albedo = lerp(albedo, grey * 1.35, lich * 0.45) * lerp(0.5, 1.12, hb) * butt;
            float3 up = normalize(xform_normal(inst, float3(0.0, 1.0, 0.0)));
            float3 tu = cross(up, n);
            if (length_squared(p0) < 20.0 * 20.0 && length_squared(tu) > 1e-6) {
                const float E = 0.012;
                float hu = bark_height(q + float2(E, 0.0));
                float hv = bark_height(q + float2(0.0, E));
                tu = normalize(tu);
                float3 tv = normalize(cross(n, tu));
                n = normalize(n - 0.9 * ((hu - hb) / E * 0.03 * tu + (hv - hb) / E * 0.03 * tv));
            }
            ao *= lerp(0.65, 1.0, hb);
        }
        if (leaf) {
            // Two-sided cards: the face towards the viewer.
            float3 ngc = cross(e1, e2);
            if (dot(ngc, v) * dot(ngc, n) < 0.0) {
                n = -n;
            }
        }
    } else if (textured) {
        float2* uvs = ed_buf<float2>(ed_c.uvs);
        float2 t0 = uvs[i0];
        float2 t1 = uvs[i1];
        float2 t2 = uvs[i2];
        float2 d1 = t1 - t0;
        float2 d2 = t2 - t0;
        float2 uv = uv_xform(m, t0 + d1 * b1 + d2 * b2);
        // Ray differentials: where the next pixel's rays meet the same plane.
        float2 bx = ray_bary(view_dir(f, i.pos.xy + float2(1.0, 0.0)), p0, e1, e2);
        float2 by = ray_bary(view_dir(f, i.pos.xy + float2(0.0, 1.0)), p0, e1, e2);
        float2 ux = uv_xform(m, t0 + d1 * bx.x + d2 * bx.y) - uv;
        float2 uy = uv_xform(m, t0 + d1 * by.x + d2 * by.y) - uv;
        SamplerState smp = ed_smp(ed_c.smp);
        if (m.tex.x != NO_TEXTURE) {
            albedo *= ed_tex2d(m.tex.x).SampleGrad(smp, uv, ux, uy).rgb;
        }
        if (m.tex.z != NO_TEXTURE) {
            float3 orm = ed_tex2d(m.tex.z).SampleGrad(smp, uv, ux, uy).rgb;
            ao = orm.r;
            rough *= orm.g;
            metal *= orm.b;
        }
        if (m.tex.y != NO_TEXTURE) {
            // Tangent frame from the triangle's position and UV deltas (glTF: +V is down the
            // image, normal-map green is up, so the bitangent is -dP/dV).
            float det = d1.x * d2.y - d1.y * d2.x;
            if (abs(det) > 1e-12) {
                float r = 1.0 / det;
                float3 tg = (e1 * d2.y - e2 * d1.y) * r;
                float3 bt = (e2 * d1.x - e1 * d2.x) * r;
                tg = tg - n * dot(n, tg);
                if (length_squared(tg) > 1e-20) {
                    tg = normalize(tg);
                    float sgn = dot(cross(n, tg), bt) < 0.0 ? -1.0 : 1.0;
                    float3 bn = cross(n, tg) * -sgn;
                    float2 xy = ed_tex2d(m.tex.y).SampleGrad(smp, uv, ux, uy).rg * 2.0 - 1.0;
                    float z = sqrt(saturate(1.0 - dot(xy, xy)));
                    n = normalize(tg * xy.x + bn * xy.y + n * z);
                }
            }
        }
    }
    if (dot(n, v) < 0.0) {
        n = normalize(n - 1.01 * dot(n, v) * v);  // grazing interpolated normals: keep them facing
    }
    if (m.params.w > 0.5) {
        float2 c = floor(local.xz * 0.5);
        float k = fmod(abs(c.x + c.y), 2.0);
        albedo *= lerp(0.8, 1.15, k);
    }
    float3 p = node + local;  // camera-relative position
    float zlin = -mul(f.view, float4(p, 1.0)).z;
    float3 ng = cross(e1, e2);
    ng = length_squared(ng) > 1e-20 ? normalize(ng) : n;
    if (dot(ng, v) < 0.0) {
        ng = -ng;
    }
    float sun_vis = sun_shadow(f, ed_c.shadow, p, ng, zlin, i.pos.xy);
    float2 occ = ao_upsample(f, ed_c.ao, i.pos.xy, zlin);
    ao *= occ.x;
    float sv = occ.y;
    float3 sun = sun_light_at(f, p);
    float3 col = foliage ? shade_foliage(f, sun, albedo, rough, ao, sv, sun_vis, n, ng, v, leaf) : m.base.w > 0.5 ? shade_toon(f, sun, m, albedo, n, v, ao, sv, sun_vis) : shade_pbr(f, sun, albedo, metal, rough, ao, sv, sun_vis, n, v);
    col = apply_aerial(f, col, p, i.pos.xy);
    if (f.light.flags.w == 1u) {
        // Sun visibility (bright = lit) tinted by the cascade that served it.
        static const float3 TINT[5] = { float3(1.0, 0.25, 0.25), float3(0.25, 1.0, 0.25), float3(0.3, 0.45, 1.0), float3(1.0, 1.0, 0.25), float3(0.6, 0.6, 0.6) };
        col = TINT[shadow_cascade(f, p, zlin)] * (0.04 + 0.5 * sun_vis);
    } else if (f.light.flags.w == 3u) {
        // Receiver minus stored light depth at the centre texel (red: nearer the sun).
        uint c = shadow_cascade(f, p, zlin);
        float d = c < 4u ? cascade_depth_delta(f, ed_c.shadow, c, p) : 0.0;
        col = float3(max(d, 0.0), max(-d, 0.0), 0.0) * 0.1;  // 1 m of depth = 0.1
    } else if (f.light.flags.w == 2u) {
        col = float3(0.5 * ao);
    } else if (f.light.flags.w == 4u) {
        col = float3(0.5 * sv);
    }
    return float4(col, 1.0);
}

[shader("fragment")]
float4 fs_resolve(FsOut i) : SV_Target0 {
    return resolve_px<0>(i, false);
}

// With the terrain provider: terrain vis words (`TERRAIN_VIS_BIT`) shade as terrain.
[shader("fragment")]
float4 fs_resolve_terrain(FsOut i) : SV_Target0 {
    return resolve_px<0>(i, true);
}

// With the terrain provider and its far forest rung on (trees G3, `EW_TREES_FAR` not 0): a
// separate entry so `fs_resolve_terrain` keeps its exact code when the rung is off.
[shader("fragment")]
float4 fs_resolve_trees(FsOut i) : SV_Target0 {
    return resolve_px<1>(i, true);
}
