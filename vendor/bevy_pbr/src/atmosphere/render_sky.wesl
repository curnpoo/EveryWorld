import package::atmosphere::{
    bindings::{view, settings},
    bruneton_functions::sample_transmittance_lut_segment,
    functions::{
        sample_transmittance_lut,
        sample_sky_view_lut, direction_world_to_atmosphere,
        uv_to_ray_direction, uv_to_ndc, sample_aerial_view_lut,
        sample_sun_radiance, ndc_to_camera_dist, raymarch_atmosphere, 
        get_view_position, max_atmosphere_distance
    },
};

import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput;

@if(DUAL_SOURCE_BLENDING)
enable dual_source_blending;

@if(MULTISAMPLED)
@group(0) @binding(13) var depth_texture: texture_depth_multisampled_2d;
@else
@group(0) @binding(13) var depth_texture: texture_depth_2d;

struct RenderSkyOutput {
    @if(DUAL_SOURCE_BLENDING)
    @location(0) @blend_src(0) inscattering: vec4<f32>,
    @if(DUAL_SOURCE_BLENDING)
    @location(0) @blend_src(1) transmittance: vec4<f32>,
    @else
    @location(0) inscattering: vec4<f32>,
}

@fragment
fn main(in: FullscreenVertexOutput) -> RenderSkyOutput {
    let depth = textureLoad(depth_texture, vec2<i32>(in.position.xy), 0);

    let ray_dir_ws = uv_to_ray_direction(in.uv);
    let world_pos = get_view_position();
    let r = length(world_pos);
    let up = normalize(world_pos);
    let mu = dot(ray_dir_ws, up);
    let max_samples = settings.sky_max_samples;
    let should_raymarch = settings.rendering_method == 1u;

    var transmittance: vec3<f32>;
    var inscattering: vec3<f32>;

    let sun_radiance = sample_sun_radiance(ray_dir_ws);

    if depth == 0.0 {
        let ray_dir_as = direction_world_to_atmosphere(ray_dir_ws);
        transmittance = sample_transmittance_lut(r, mu);
        inscattering = sample_sky_view_lut(r, ray_dir_as);
        if should_raymarch {
            let t_max = max_atmosphere_distance(r, mu);
            let result = raymarch_atmosphere(world_pos, ray_dir_ws, t_max, max_samples, in.uv, true);
            inscattering = result.inscattering;
            transmittance = result.transmittance;
        }
        inscattering += sun_radiance * transmittance;
    } else {
        let t = ndc_to_camera_dist(vec3(uv_to_ndc(in.uv), depth));
        inscattering = sample_aerial_view_lut(in.uv, t);
        transmittance = sample_transmittance_lut_segment(r, mu, t);
        if should_raymarch {
            let result = raymarch_atmosphere(world_pos, ray_dir_ws, t, max_samples, in.uv, false);
            inscattering = result.inscattering;
            transmittance = result.transmittance;
        }
        // Art direction (EveryWorld): physically, a 100+ km slant path over dark
        // ocean/land is mostly path radiance, which reads as a grey veil. From
        // orbit we keep the limb/sky pass untouched but thin the veil laid over
        // the surface so the real-world colour stays legible.
        let alt = r - 6360000.0;
        let k = smoothstep(20000.0, 250000.0, alt);
        inscattering *= mix(1.0, 0.22, k);
        // Slant-path Rayleigh extinction turns transmittance orange (sunset physics),
        // which browns a navy sea. From orbit the blue in-scatter compensates in
        // reality; here we neutralise the tint instead of the veil.
        let tm = (transmittance.r + transmittance.g + transmittance.b) / 3.0;
        transmittance = mix(transmittance, vec3(tm), 0.9 * k);
    }

    // exposure compensation
    inscattering *= view.exposure;
    // Sanitize: a single NaN/inf here is smeared across the whole frame by bloom.
    // Bit test (exponent all ones) because fast-math may fold `x != x`.
    let ibits = bitcast<vec3<u32>>(inscattering);
    let bad = ((ibits >> vec3<u32>(23u)) & vec3<u32>(255u)) == vec3<u32>(255u);
    inscattering = select(clamp(inscattering, vec3(0.0), vec3(6.0e4)), vec3(0.0), bad);
    let tbits = bitcast<vec3<u32>>(transmittance);
    let tbad = ((tbits >> vec3<u32>(23u)) & vec3<u32>(255u)) == vec3<u32>(255u);
    transmittance = select(clamp(transmittance, vec3(0.0), vec3(1.0)), vec3(1.0), tbad);

    @if(DUAL_SOURCE_BLENDING)
    return RenderSkyOutput(vec4(inscattering, 0.0), vec4(transmittance, 1.0));
    @if(!DUAL_SOURCE_BLENDING)
    let mean_transmittance = (transmittance.r + transmittance.g + transmittance.b) / 3.0;
    @if(!DUAL_SOURCE_BLENDING)
    return RenderSkyOutput(vec4(inscattering, mean_transmittance));
    
}
