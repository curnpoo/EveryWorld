// Foliage wind: prepass / shadow vertex shader. Same displacement as
// foliage.wesl so depth, normals, motion vectors and shadows line up.

import bevy_pbr::{
    prepass::io::{Vertex, VertexOutput, decompress_vertex},
    render::{mesh_functions, view_transformations::position_world_to_clip},
};

// ---- shared wind (identical in foliage.wesl and foliage_prepass.wesl so
// depth prepass, shadows and the main pass agree to the bit) ----

struct FoliageWind {
    // xy: direction the wind blows toward (world x, z), z: strength, w: time (s)
    wind: vec4<f32>,
    // x: previous frame's time (motion vectors), yzw: unused
    prev: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> foliage: FoliageWind;

fn hash13(p: vec3<f32>) -> f32 {
    let q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    let r = q + dot(q, q.yzx + 33.33);
    return fract((r.x + r.y) * r.z);
}

// `packed` = (floor(phase*64)*4 + sway, flutter), see ed_world::trees::pack_wind.
// `origin` is the mesh's world origin (instanced tree base or chunk center),
// `local` the vertex's object-space position (stable flutter seed).
fn wind_offset(world: vec3<f32>, origin: vec3<f32>, local: vec3<f32>, packed: vec2<f32>, scale: f32, t: f32) -> vec3<f32> {
    let pk = floor(packed.x * 0.25);
    let sway = (packed.x - pk * 4.0) * scale;
    let phase = pk / 64.0 * 6.2831853 + hash13(floor(origin * 0.5)) * 6.2831853;
    let dir = normalize(vec3<f32>(foliage.wind.x, 0.0, foliage.wind.y) + vec3<f32>(1e-4, 0.0, 0.0));
    let cross = vec3<f32>(-dir.z, 0.0, dir.x);
    let strength = foliage.wind.z;
    // Gust fronts roll across the landscape along the wind.
    let along = dot(world.xz, dir.xz);
    let gust = 0.6 + 0.4 * sin(along * 0.05 - t * 1.3) * sin(along * 0.017 - t * 0.43 + 1.7);
    let bend = sway * strength * gust;
    var off = dir * bend * (0.75 + 0.25 * sin(t * 1.7 + phase))
        + cross * sway * strength * 0.22 * sin(t * 1.13 + phase * 1.37);
    // Keep branch length roughly constant: bending lowers the tip a little.
    off.y = off.y - 0.25 * bend * bend / max(sway * 3.0 + 0.5, 0.5);
    // Leaf flutter: fast, small, per-vertex direction from a stable seed.
    let fl = packed.y * scale * strength * (0.4 + gust);
    let s = hash13(local * 7.31 + origin * 0.01);
    let fdir = normalize(vec3<f32>(s - 0.5, 0.6, fract(s * 13.7) - 0.5));
    off = off + fdir * fl * sin(t * (8.0 + 5.0 * s) + s * 40.0 + phase);
    return off;
}

@vertex
fn vertex(vertex_in: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(vertex_in, vertex_in.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(vertex_in.instance_index);
    var wp = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
@if(VERTEX_UVS_B) {
    let scale = length(world_from_local[0].xyz);
    wp = vec4<f32>(wp.xyz + wind_offset(wp.xyz, world_from_local[3].xyz, vertex.position, vertex.uv_b, scale, foliage.wind.w), 1.0);
}
    out.world_position = wp;
    out.position = position_world_to_clip(wp.xyz);
@if(UNCLIPPED_DEPTH_ORTHO_EMULATION) {
    out.unclipped_depth = out.position.z;
    out.position.z = min(out.position.z, 1.0);
}
@if(VERTEX_UVS_A)
    out.uv = vertex.uv;
@if(VERTEX_UVS_B)
    out.uv_b = vertex.uv_b;
@if(NORMAL_PREPASS_OR_DEFERRED_PREPASS) {
@if(VERTEX_NORMALS)
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex_in.instance_index);
@if(VERTEX_TANGENTS)
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(world_from_local, vertex.tangent, vertex_in.instance_index);
}
@if(VERTEX_COLORS)
    out.color = vertex.color;
@if(MOTION_VECTOR_PREPASS) {
    let prev_model = mesh_functions::get_previous_world_from_local(vertex_in.instance_index);
    var pwp = mesh_functions::mesh_position_local_to_world(prev_model, vec4<f32>(vertex.position, 1.0));
@if(VERTEX_UVS_B) {
    let pscale = length(prev_model[0].xyz);
    pwp = vec4<f32>(pwp.xyz + wind_offset(pwp.xyz, prev_model[3].xyz, vertex.position, vertex.uv_b, pscale, foliage.prev.x), 1.0);
}
    out.previous_world_position = pwp;
}
@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = vertex_in.instance_index;
@if(VISIBILITY_RANGE_DITHER)
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(vertex_in.instance_index, world_from_local[3]);
    return out;
}
