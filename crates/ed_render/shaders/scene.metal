// scene pass: instanced boxes (test scene), vertex pulling from bindless buffers, one sun.
// Layouts mirror `Vertex`, `Instance` and `SceneConstants` in crates/ed_render/src/lib.rs.

struct Vtx { float4 pos; float4 nrm; };
// Positions are relative to the scene's node origin (f32); the view adds the camera-relative
// node origin from the constants, so nothing large reaches the GPU (P1-D7).
struct Instance {
    float4 pos_yaw;   // xyz: position, w: yaw (radians, about +Y)
    float4 scale;     // xyz
    float4 color;     // rgb: albedo (linear), w: 1 = checker pattern
};
struct SceneC {
    uint frame;
    uint instances;
    uint vertices;
    uint pad0;
    float4 origin_rel; // node origin minus camera position, computed in f64
    uint pad[8];
};

struct VOut {
    float4 pos [[position]];
    float3 nrm;
    float3 albedo;
    float3 local;     // node-relative position (pattern)
    float pattern [[flat]];
};

inline float3 rot_y(float3 v, float a) {
    float s = sin(a), c = cos(a);
    return float3(c * v.x + s * v.z, v.y, -s * v.x + c * v.z);
}

vertex VOut vs_scene(uint vid [[vertex_id]], uint iid [[instance_id]], ED_BINDLESS(SceneC)) {
    device const FrameData& f = *ed_buf(FrameData, ed_c.frame);
    Vtx v = ed_buf(Vtx, ed_c.vertices)[vid];
    Instance inst = ed_buf(Instance, ed_c.instances)[iid];
    float3 local = inst.pos_yaw.xyz + rot_y(v.pos.xyz * inst.scale.xyz, inst.pos_yaw.w);
    float3 rel = ed_c.origin_rel.xyz + local;
    VOut o;
    o.pos = f.view_proj * float4(rel, 1.0);
    o.nrm = rot_y(v.nrm.xyz / inst.scale.xyz, inst.pos_yaw.w);
    o.albedo = inst.color.rgb;
    o.local = local;
    o.pattern = inst.color.w;
    return o;
}

fragment float4 fs_scene(VOut in [[stage_in]], ED_BINDLESS(SceneC)) {
    device const FrameData& f = *ed_buf(FrameData, ed_c.frame);
    float3 n = normalize(in.nrm);
    float3 albedo = in.albedo;
    if (in.pattern > 0.5) {
        float2 cell = floor(in.local.xz / 2.0);
        albedo *= fmod(abs(cell.x + cell.y), 2.0) < 1.0 ? 1.0 : 0.82;
    }
    float ndl = saturate(dot(n, f.sun_dir.xyz));
    float3 ambient = mix(f.ground_color.rgb * 0.6, f.sky_zenith.rgb * 0.8, n.y * 0.5 + 0.5);
    float3 c = albedo * (f.sun_color.rgb * ndl + ambient);
    return float4(c, 1.0);
}
