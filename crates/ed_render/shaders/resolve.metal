// Material resolve: one full-screen pass shades every pixel once from the visibility buffer
// (sky where nothing was drawn). The triangle is refetched, the view ray intersected with it
// for perspective-correct barycentrics, and the material read from the bindless table.
// Materials v0: PBR (GGX / Smith / Schlick, metallic-roughness) and toon, same light inputs.

struct ResolveC {
    uint frame;
    uint vis;
    uint instances;
    uint nodes;
    uint meshes;
    uint materials;
    uint verts;
    uint indices;
    uint pad[8];
};

vertex FsOut vs_resolve(uint vid [[vertex_id]], ED_BINDLESS(ResolveC)) {
    return fullscreen_vertex(vid);
}

constant float PI = 3.14159265;

inline float d_ggx(float noh, float a) {
    float a2 = a * a;
    float d = noh * noh * (a2 - 1.0) + 1.0;
    return a2 / (PI * d * d);
}

inline float v_smith(float nov, float nol, float a) {
    float a2 = a * a;
    float gv = nol * sqrt(nov * nov * (1.0 - a2) + a2);
    float gl = nov * sqrt(nol * nol * (1.0 - a2) + a2);
    return 0.5 / max(gv + gl, 1e-5);
}

inline float3 f_schlick(float3 f0, float voh) {
    return f0 + (1.0 - f0) * pow(1.0 - voh, 5.0);
}

// Split-sum environment BRDF approximation (Karis 2014, mobile).
inline float2 env_brdf(float nov, float rough) {
    const float4 c0 = float4(-1.0, -0.0275, -0.572, 0.022);
    const float4 c1 = float4(1.0, 0.0425, 1.04, -0.04);
    float4 r = rough * c0 + c1;
    float a004 = min(r.x * r.x, exp2(-9.28 * nov)) * r.x + r.y;
    return float2(-1.04, 1.04) * a004 + r.zw;
}

inline float3 ambient(device const FrameData& f, float3 n) {
    // Hemisphere: sky above, ground bounce below.
    float t = n.y * 0.5 + 0.5;
    float3 sky = mix(f.sky_horizon.rgb, f.sky_zenith.rgb, 0.5);
    return mix(f.ground_color.rgb, sky, t);
}

inline float3 shade_pbr(device const FrameData& f, GpuMaterial m, float3 albedo, float3 n, float3 v) {
    float metal = saturate(m.params.x);
    float rough = clamp(m.params.y, 0.04, 1.0);
    float a = rough * rough;
    float3 l = f.sun_dir.xyz;
    float3 h = normalize(l + v);
    float nol = saturate(dot(n, l));
    float nov = max(dot(n, v), 1e-4);
    float noh = saturate(dot(n, h));
    float voh = saturate(dot(v, h));
    float3 f0 = mix(float3(0.04), albedo, metal);
    float3 diff_col = albedo * (1.0 - metal);
    float3 F = f_schlick(f0, voh);
    float3 spec = d_ggx(noh, a) * v_smith(nov, nol, a) * F;
    float3 sun = f.sun_color.rgb * nol;
    float3 direct = (diff_col * (1.0 - F) + spec * PI) * sun;
    float2 ab = env_brdf(nov, rough);
    float3 r = reflect(-v, n);
    float3 env = mix(sky_radiance(f, r), ambient(f, n), rough);
    float3 amb = diff_col * ambient(f, n) + env * (f0 * ab.x + ab.y);
    return direct + amb;
}

inline float3 shade_toon(device const FrameData& f, GpuMaterial m, float3 albedo, float3 n, float3 v) {
    float3 l = f.sun_dir.xyz;
    float ndl = dot(n, l);
    float bands = max(m.params.z, 1.0);
    float lit = floor(saturate(ndl * 0.5 + 0.5) * bands + 0.5) / bands;
    float3 shade = m.shade.rgb;
    float3 c = mix(albedo * shade, albedo, lit);
    float3 light = f.sun_color.rgb * 0.6 + ambient(f, n);
    float rim = pow(1.0 - saturate(dot(n, v)), 4.0) * m.shade.w * saturate(ndl + 0.3);
    return c * light * 0.5 + rim * f.sun_color.rgb * 0.3;
}

fragment float4 fs_resolve(FsOut in [[stage_in]], ED_BINDLESS(ResolveC)) {
    device const FrameData& f = *ed_buf(FrameData, ed_c.frame);
    uint2 px = uint2(in.pos.xy);
    uint2 vis = ed_tex2d_uint(ed_c.vis).read(px).xy;
    float3 dir = view_dir(f, in.pos.xy);
    if (vis.x == 0) {
        return float4(sky_radiance(f, dir), 1.0);
    }
    GpuInstance inst = ed_buf(GpuInstance, ed_c.instances)[vis.x - 1u];
    device const GpuMesh& mesh = ed_buf(GpuMesh, ed_c.meshes)[inst.mesh];
    uint lod = vis.y >> 28;
    uint tris = max(mesh.index_count[lod] / 3u, 1u);
    uint prim = (vis.y & 0x0FFFFFFFu) % tris;
    device const uint* idx = ed_buf(uint, ed_c.indices) + mesh.first_index[lod] + prim * 3u;
    device const Vtx* verts = ed_buf(Vtx, ed_c.verts);
    int bv = mesh.base_vertex[lod];
    Vtx v0 = verts[int(idx[0]) + bv];
    Vtx v1 = verts[int(idx[1]) + bv];
    Vtx v2 = verts[int(idx[2]) + bv];
    float3 node = ed_buf(float4, ed_c.nodes)[inst.node_flags & 0xFFFFu].xyz;
    float3 l0 = xform_point(inst, float3(v0.px, v0.py, v0.pz));
    float3 l1 = xform_point(inst, float3(v1.px, v1.py, v1.pz));
    float3 l2 = xform_point(inst, float3(v2.px, v2.py, v2.pz));
    // Ray from the camera (origin, camera-relative space) through the pixel.
    float3 p0 = node + l0;
    float3 e1 = l1 - l0;
    float3 e2 = l2 - l0;
    float3 pv = cross(dir, e2);
    float det = dot(e1, pv);
    float b1 = 1.0 / 3.0;
    float b2 = 1.0 / 3.0;
    if (abs(det) > 1e-12) {
        float inv = 1.0 / det;
        float3 tv = -p0;
        float3 qv = cross(tv, e1);
        b1 = saturate(dot(tv, pv) * inv);
        b2 = saturate(dot(dir, qv) * inv);
        float s = b1 + b2;
        if (s > 1.0) {
            b1 /= s;
            b2 /= s;
        }
    }
    float b0 = 1.0 - b1 - b2;
    float3 local = l0 * b0 + l1 * b1 + l2 * b2;  // node-local position
    float3 nm = oct_decode(v0.n) * b0 + oct_decode(v1.n) * b1 + oct_decode(v2.n) * b2;
    float3 n = normalize(xform_normal(inst, nm));
    float3 v = -dir;
    if (dot(n, v) < 0.0) {
        n = normalize(n - 1.01 * dot(n, v) * v);  // grazing interpolated normals: keep them facing
    }
    GpuMaterial m = ed_buf(GpuMaterial, ed_c.materials)[inst.material];
    float3 albedo = m.base.rgb;
    if (m.params.w > 0.5) {
        float2 c = floor(local.xz * 0.5);
        float k = fmod(abs(c.x + c.y), 2.0);
        albedo *= mix(0.8, 1.15, k);
    }
    float3 col = m.base.w > 0.5 ? shade_toon(f, m, albedo, n, v) : shade_pbr(f, m, albedo, n, v);
    return float4(col, 1.0);
}
