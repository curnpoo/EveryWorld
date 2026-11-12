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
#include "shader_abi.slang"
#include "common.slang"
#include "lighting.slang"

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
    uint pad[4];
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

float3 shade_pbr(FrameData* f, float3 albedo, float metal, float rough, float ao, float sv, float sun_vis, float3 n, float3 v) {
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
    float3 spec = d_ggx(noh, a) * v_smith(nov, nol, a) * F;
    float3 direct = (diff_col * (1.0 - F) / PI + spec) * f.sun_color.rgb * (nol * sun_vis);
    float2 ab = env_brdf(nov, rough);
    float3 r = reflect(-v, n);
    float3 env = f.light.flags.z != 0 ? lerp(sky_rad(f, r, 0.0), sh_irradiance(f, r), rough) : hemi_ambient(f, r);
    env = occluded_ambient(f, env, sv);
    // Specular occlusion from AO (Lagarde 2014).
    float spec_occ = saturate(pow(nov + ao, exp2(-16.0 * a - 1.0)) - 1.0 + ao);
    // The specular lobe's share of the ambient light does not also reach the diffuse term.
    float3 f_env = f0 * ab.x + ab.y;
    float3 amb = diff_col * (1.0 - f_env) * occluded_ambient(f, ambient_light(f, n), sv) * ao_multibounce(f, ao, diff_col) + env * f_env * spec_occ;
    return direct + amb;
}

float3 shade_toon(FrameData* f, GpuMaterial m, float3 albedo, float3 n, float3 v, float ao, float sv, float sun_vis) {
    float3 l = f.sun_dir.xyz;
    float ndl = dot(n, l);
    float bands = max(m.params.z, 1.0);
    float lit = floor(saturate(ndl * 0.5 + 0.5) * bands + 0.5) / bands * sun_vis;
    float3 c = lerp(albedo * m.shade.rgb, albedo, lit);
    float3 light = f.sun_color.rgb * (lit / PI) + occluded_ambient(f, ambient_light(f, n), sv) * ao_multibounce(f, ao, albedo);
    float rim = pow(1.0 - saturate(dot(n, v)), 4.0) * m.shade.w * saturate(ndl + 0.3) * sun_vis;
    return c * light + rim * f.sun_color.rgb * 0.15;
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

[shader("fragment")]
float4 fs_resolve(FsOut i) : SV_Target0 {
    FrameData* f = ed_buf<FrameData>(ed_c.frame);
    uint2 px = uint2(i.pos.xy);
    uint2 vis = ed_tex2d_uint(ed_c.vis).Load(int3(int2(px), 0)).xy;
    float3 dir = view_dir(f, i.pos.xy);
    if (vis.x == 0) {
        return float4(sky_rad(f, dir, f.light.sky.x), 1.0);
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
    float3 l0 = xform_point(inst, float3(v0.px, v0.py, v0.pz));
    float3 l1 = xform_point(inst, float3(v1.px, v1.py, v1.pz));
    float3 l2 = xform_point(inst, float3(v2.px, v2.py, v2.pz));
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
    if (textured) {
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
    float3 col = m.base.w > 0.5 ? shade_toon(f, m, albedo, n, v, ao, sv, sun_vis) : shade_pbr(f, albedo, metal, rough, ao, sv, sun_vis, n, v);
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
