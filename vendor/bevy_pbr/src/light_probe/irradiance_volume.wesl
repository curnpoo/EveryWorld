import package::light_probe::{light_probe_iterator_new, light_probe_iterator_next};
import package::render::mesh_view_bindings::{
    irradiance_volumes,
    irradiance_volume,
    irradiance_volume_sampler,
    light_probes,
};
import package::render::clustered_forward::ClusterableObjectIndexRanges;

@if(LIGHTMAP) import package::render::mesh_view_types::LIGHT_PROBE_FLAG_AFFECTS_LIGHTMAPPED_MESH_DIFFUSE;

@if(IRRADIANCE_VOLUMES_ARE_USABLE)

/// See:
/// https://advances.realtimerendering.com/s2006/Mitchell-ShadingInValvesSourceEngine.pdf
/// Slide 28, "Ambient Cube Basis"
fn irradiance_volume_light(
    world_position: vec3<f32>,
    N: vec3<f32>,
    clusterable_object_index_ranges: ptr<function, ClusterableObjectIndexRanges>,
) -> vec3<f32> {
    // Find all irradiance volumes that contain the fragment. We're going to
    // accumulate all the irradiance from them in a weighted sum.
    var iterator = light_probe_iterator_new(
        world_position,
        /*is_irradiance_volume=*/ true,
        clusterable_object_index_ranges,
    );

    var total_weight = 0.0;
    var total_irradiance = vec3(0.0);

    while (true) {
        var query_result = light_probe_iterator_next(&iterator);

        // If there was no irradiance volume found, we're done.
        if (query_result.texture_index < 0) {
            break;
        }

        // If we're lightmapped, and the irradiance volume contributes no diffuse
        // light, then bail out.
        @if(LIGHTMAP)
        if ((query_result.flags & LIGHT_PROBE_FLAG_AFFECTS_LIGHTMAPPED_MESH_DIFFUSE) == 0u) {
            continue;
        }

        @if(MULTIPLE_LIGHT_PROBES_IN_ARRAY)
        let irradiance_volume_texture = irradiance_volumes[query_result.texture_index];
        @else
        let irradiance_volume_texture = irradiance_volume;

        let atlas_resolution = vec3<f32>(textureDimensions(irradiance_volume_texture));
        let resolution = vec3<f32>(textureDimensions(irradiance_volume_texture) / vec3(1u, 2u, 3u));

        // Make sure to clamp to the edges to avoid texture bleed.
        var unit_pos = (query_result.light_from_world * vec4(world_position, 1.0f)).xyz;
        let stp = clamp((unit_pos + 0.5) * resolution, vec3(0.5f), resolution - vec3(0.5f));
        let uvw = stp / atlas_resolution;

        // The bottom half of each cube slice is the negative part, so choose it if applicable on each
        // slice.
        let neg_offset = select(vec3(0.0f), vec3(0.5f), N < vec3(0.0f));

        let uvw_x = uvw + vec3(0.0f, neg_offset.x, 0.0f);
        let uvw_y = uvw + vec3(0.0f, neg_offset.y, 1.0f / 3.0f);
        let uvw_z = uvw + vec3(0.0f, neg_offset.z, 2.0f / 3.0f);

        let rgb_x = textureSampleLevel(irradiance_volume_texture, irradiance_volume_sampler, uvw_x, 0.0).rgb;
        let rgb_y = textureSampleLevel(irradiance_volume_texture, irradiance_volume_sampler, uvw_y, 0.0).rgb;
        let rgb_z = textureSampleLevel(irradiance_volume_texture, irradiance_volume_sampler, uvw_z, 0.0).rgb;

        // Use Valve's formula to sample.
        let NN = N * N;
        total_irradiance += (rgb_x * NN.x + rgb_y * NN.y + rgb_z * NN.z) * query_result.intensity *
            query_result.weight;

        total_weight += query_result.weight;
    }

    if (total_weight != 0.0) {
        total_irradiance /= total_weight;
    }

    return total_irradiance;
}

// ---------------------------------------------------------------------------
// EveryWorld patch: terrain-following probe GI (see ed_world::probes).
//
// The volume texture is (nx, 2*ny, 4*nz): depth slabs X faces | Y faces |
// Z faces | ground; rows [0,ny) positive faces, [ny,2ny) negative. Each face
// texel is rgba = (bounce albedo, sky visibility relative to open flat
// ground). The ground slab (row 0) holds .r = unit-space y of the lowest probe
// layer, .g = unit-space span of the layers. Layers are spaced by t^2 so they
// hug the ground: layer = sqrt(t) * (ny - 1).
//
// Used multiplicatively by environment_map.wesl:
//   irradiance = mix(env(N), env(N) * vis(N) + env(up) * bounce(N) * intensity, weight)
// so outside the volume nothing changes.

struct ProbeGi {
    vis: f32,
    bounce: vec3<f32>,
    weight: f32,
}

@if(IRRADIANCE_VOLUMES_ARE_USABLE)
fn probe_gi(
    world_position: vec3<f32>,
    N: vec3<f32>,
    clusterable_object_index_ranges: ptr<function, ClusterableObjectIndexRanges>,
) -> ProbeGi {
    var out: ProbeGi;
    out.vis = 0.0;
    out.bounce = vec3(0.0);
    out.weight = 0.0;
    var iterator = light_probe_iterator_new(
        world_position,
        /*is_irradiance_volume=*/ true,
        clusterable_object_index_ranges,
    );
    var total = vec4(0.0);
    while (true) {
        var q = light_probe_iterator_next(&iterator);
        if (q.texture_index < 0) {
            break;
        }

        @if(MULTIPLE_LIGHT_PROBES_IN_ARRAY)
        let tex = irradiance_volumes[q.texture_index];
        @else
        let tex = irradiance_volume;

        let dims = vec3<f32>(textureDimensions(tex));
        let res = dims / vec3(1.0, 2.0, 4.0);
        let unit_pos = (q.light_from_world * vec4(world_position, 1.0)).xyz;

        // Inverse of ed_world::probes::warp_axis: dense near the camera,
        // sparse out to 1 km. Texel coords are clamped to outer probe centers.
        let radial = unit_pos.xz * 2.0;
        let grid = sign(radial) * pow(abs(radial), vec2<f32>(2.0 / 3.0));
        var s = clamp((grid.x * 0.5 + 0.5) * res.x, 0.5, res.x - 0.5);
        var p = clamp((grid.y * 0.5 + 0.5) * res.z, 0.5, res.z - 0.5);

        // Terrain-following vertical layer.
        let ground = textureSampleLevel(tex, irradiance_volume_sampler, vec3(s / dims.x, 0.5 / dims.y, (p + 3.0 * res.z) / dims.z), 0.0);
        let t = clamp((unit_pos.y - ground.r) / max(ground.g, 1e-6), 0.0, 1.0);
        let layer = sqrt(t) * (res.y - 1.0) + 0.5;

        let u = s / dims.x;
        let v = layer / dims.y;
        let neg = select(vec3(0.0), vec3(0.5), N < vec3(0.0));
        let fx = textureSampleLevel(tex, irradiance_volume_sampler, vec3(u, v + neg.x, (p + 0.0 * res.z) / dims.z), 0.0);
        let fy = textureSampleLevel(tex, irradiance_volume_sampler, vec3(u, v + neg.y, (p + 1.0 * res.z) / dims.z), 0.0);
        let fz = textureSampleLevel(tex, irradiance_volume_sampler, vec3(u, v + neg.z, (p + 2.0 * res.z) / dims.z), 0.0);
        let NN = N * N;
        let f = fx * NN.x + fy * NN.y + fz * NN.z;
        total += vec4(f.rgb * q.intensity, f.a) * q.weight;
        out.weight += q.weight;
    }
    if (out.weight > 0.0) {
        total /= out.weight;
        out.weight = min(out.weight, 1.0);
    }
    out.vis = total.a;
    out.bounce = total.rgb;
    return out;
}
