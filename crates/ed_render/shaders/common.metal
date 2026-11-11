// Shared declarations for the ed_render v0 shaders (MSL-first fallback, P1-D1).
// Prepended after ed_gpu::shader_abi::MSL_PRELUDE. Layouts mirror `FrameData` in
// crates/ed_render/src/lib.rs; change both together.

// Per-view data, one cpu_write buffer read through the bindless buffer table.
struct FrameData {
    float4x4 view_proj;     // camera-relative: rotation and projection only (P1-D7)
    float4x4 inv_view_proj;
    float4 sun_dir;         // xyz: unit vector towards the sun
    float4 sun_color;       // rgb: sun radiance scale (linear)
    float4 sky_zenith;      // rgb
    float4 sky_horizon;     // rgb
    float4 ground_color;    // rgb: below-horizon colour and bounce light
    float4 viewport;        // x, y, w, h in output pixels
    float4 params;          // x: exposure
};

struct FsOut {
    float4 pos [[position]];
};

// One triangle covering the viewport.
inline FsOut fullscreen_vertex(uint vid) {
    float2 p = float2((vid << 1) & 2, vid & 2);
    FsOut o;
    o.pos = float4(p * 2.0 - 1.0, 0.0, 1.0);
    return o;
}

// World-space view direction (camera-relative) of a pixel of a viewport-sized target.
inline float3 view_dir(device const FrameData& f, float2 pixel) {
    float2 uv = pixel / f.viewport.zw;
    float2 ndc = float2(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    // Reverse-Z: depth 1 is the near plane, which keeps w finite with an infinite far plane.
    float4 p = f.inv_view_proj * float4(ndc, 1.0, 1.0);
    return normalize(p.xyz / p.w);
}

// Analytic gradient sky (no sun disc). Placeholder until the atmosphere port (Phase 2).
inline float3 sky_radiance(device const FrameData& f, float3 d) {
    float t = pow(saturate(d.y), 0.45);
    float3 sky = mix(f.sky_horizon.rgb, f.sky_zenith.rgb, t);
    sky = mix(sky, f.ground_color.rgb, saturate(-d.y * 8.0));
    float mu = saturate(dot(d, f.sun_dir.xyz));
    sky += f.sun_color.rgb * 0.06 * pow(mu, 8.0);
    return sky;
}
