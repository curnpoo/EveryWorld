// Foliage wind: main-pass vertex shader for ExtendedMaterial<StandardMaterial, WindExt>.
// Sways trunks and branches and flutters leaves using per-vertex wind data
// in UV1 and the live wind uniform; the PBR fragment shader is unchanged.

import bevy_pbr::render::{
    mesh_functions,
    forward_io::{Vertex, VertexOutput, decompress_vertex},
    view_transformations::position_world_to_clip,
};

import bevy_pbr::render::mesh_view_bindings::view;

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


// Leaf cards hand over to impostors at ~590 m. Per-vertex hashed band so the
// edge dissolves instead of drawing a line; impostors fade in over the same
// band (impostor.wesl, near = CARD_RANGE_M - 210). Skipped in orthographic
// (shadow) views, where `view` is the light, not the camera.
const CARD_HANDOFF_M: f32 = 590.0;
const CARD_CELL_M: f32 = 4.0;
// Decide per CARD_CELL_M cell (not per vertex) so a leaf quad is kept or
// dropped whole; `cell_c` is the cell centre in local space. Returns false in
// orthographic views.
fn card_culled(world_from_local: mat4x4<f32>, local: vec3<f32>) -> bool {
    if view.clip_from_view[3][3] > 0.5 { return false; }
    let cell = floor(local / CARD_CELL_M);
    let cell_c = (cell + 0.5) * CARD_CELL_M;
    let wc = (world_from_local * vec4<f32>(cell_c, 1.0)).xyz;
    let d = distance(wc, view.world_position);
    return d > CARD_HANDOFF_M - 20.0 + 40.0 * hash13(cell);
}
fn card_cell_center(world_from_local: mat4x4<f32>, local: vec3<f32>) -> vec4<f32> {
    let c = (floor(local / CARD_CELL_M) + 0.5) * CARD_CELL_M;
    return vec4<f32>((world_from_local * vec4<f32>(c, 1.0)).xyz, 1.0);
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
    if card_culled(world_from_local, vertex.position) { wp = card_cell_center(world_from_local, vertex.position); }
}
@if(VERTEX_NORMALS)
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex_in.instance_index);
    out.world_position = wp;
    out.position = position_world_to_clip(wp.xyz);
@if(VERTEX_UVS_A)
    out.uv = vertex.uv;
@if(VERTEX_UVS_B)
    out.uv_b = vertex.uv_b;
@if(VERTEX_TANGENTS)
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(world_from_local, vertex.tangent, vertex_in.instance_index);
@if(VERTEX_COLORS)
    out.color = vertex.color;
@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = vertex_in.instance_index;
@if(VISIBILITY_RANGE_DITHER)
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(vertex_in.instance_index, world_from_local[3]);
    return out;
}
