// clear/sky pass: analytic gradient sky and sun disc into the HDR target.

struct SkyC { uint frame; uint pad[15]; };

vertex FsOut vs_fullscreen(uint vid [[vertex_id]], ED_BINDLESS(SkyC)) {
    return fullscreen_vertex(vid);
}

fragment float4 fs_sky(FsOut in [[stage_in]], ED_BINDLESS(SkyC)) {
    device const FrameData& f = *ed_buf(FrameData, ed_c.frame);
    float3 d = view_dir(f, in.pos.xy);
    float3 c = sky_radiance(f, d);
    // Sun disc, about 0.55 degrees across, soft edge.
    float mu = dot(d, f.sun_dir.xyz);
    c += f.sun_color.rgb * 40.0 * smoothstep(0.99996, 0.99999, mu);
    return float4(c, 1.0);
}
