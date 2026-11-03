// Ground v2: near-camera ground cover (grass, flowers, pebbles, rocks...).
//
// Vertex-only material: the fragment side is Bevy's standard PBR.
// Every vertex carries its instance pivot and a packed (reach, rank). Each
// instance shrinks toward its pivot once its rank exceeds the visible
// fraction at its distance, so density falls off smoothly with no popping.
// Plants (uv.x = flex) bend in the wind, more toward their tips (uv.y).
// The same shader runs for the main pass, depth/normal/motion prepass and
// shadows, so all of them agree on where every blade is.

import bevy_pbr::render::{
    mesh_functions,
    view_transformations::position_world_to_clip,
};

@if(PREPASS_PIPELINE)
import bevy_pbr::prepass::io::VertexOutput;
@else
import bevy_pbr::render::forward_io::VertexOutput;

struct ScatterParams {
    camera: vec4<f32>,
    wind: vec4<f32>,
    time: vec4<f32>,
};

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> scatter: ScatterParams;

struct ScatterVertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(5) color: vec4<f32>,
    @location(8) pivot: vec4<f32>,
};

// Twin of ed_world::ground_scatter::fade. Keep in sync.
fn fade(d: f32, reach: f32) -> f32 {
    let x = d * 90.0 / max(reach, 1.0);
    let f = pow(16.0 / max(x, 16.0), 2.0);
    return f * (1.0 - smoothstep(67.5, 90.0, x));
}

fn displaced(local: vec3<f32>, uv: vec2<f32>, pivot: vec4<f32>, world_from_local: mat4x4<f32>, t: f32) -> vec3<f32> {
    let pivot_w = (world_from_local * vec4<f32>(pivot.xyz, 1.0)).xyz;
    let p_w = (world_from_local * vec4<f32>(local, 1.0)).xyz;
    let reach = floor(pivot.w);
    let rank = fract(pivot.w);
    var s = 1.0;
    if scatter.camera.w > 0.5 {
        let d = distance(pivot_w, scatter.camera.xyz);
        let f = fade(d, reach);
        s = clamp((f - rank) / max(0.15 * f, 0.015), 0.0, 1.0);
        // Keep blades out of the lens when crouching or lying in the grass.
        s = s * smoothstep(0.6, 1.3, d);
        // Grass gets a little bolder far away so thinning reads as texture.
        if uv.x > 0.9 {
            s = s * (1.0 + 0.6 * smoothstep(12.0, 60.0, d));
        }
    }
    var off = (p_w - pivot_w) * s;
    let flex = uv.x;
    if flex > 0.0 {
        let hgt = max(off.y, 0.0);
        let ph = dot(pivot_w.xz, vec2<f32>(0.13, 0.09));
        let gust = 0.55 + 0.3 * sin(t * 1.1 + ph * 0.31) + 0.15 * sin(t * 3.7 + ph * 2.3);
        let flutter = 0.1 * sin(t * 8.3 + ph * 7.0 + uv.y * 3.0);
        let bend = flex * uv.y * uv.y * scatter.wind.w * (gust * 0.4 + flutter);
        off += vec3<f32>(scatter.wind.x, 0.0, scatter.wind.z) * bend * hgt;
        off.y -= abs(bend) * hgt * 0.3;
    }
    return pivot_w + off;
}

@vertex
fn vertex(v: ScatterVertex) -> VertexOutput {
    var out: VertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(v.instance_index);
    let wp = displaced(v.position, v.uv, v.pivot, world_from_local, scatter.time.x);
    out.world_position = vec4<f32>(wp, 1.0);
    out.position = position_world_to_clip(wp);
@if(PREPASS_PIPELINE && UNCLIPPED_DEPTH_ORTHO_EMULATION) {
    out.unclipped_depth = out.position.z;
    out.position.z = min(out.position.z, 1.0);
}
@if(!PREPASS_PIPELINE || NORMAL_PREPASS_OR_DEFERRED_PREPASS)
    out.world_normal = mesh_functions::mesh_normal_local_to_world(v.normal, v.instance_index);
@if(VERTEX_UVS_A)
    out.uv = v.uv;
@if(VERTEX_COLORS)
    out.color = v.color;
@if(PREPASS_PIPELINE && MOTION_VECTOR_PREPASS) {
    let prev = mesh_functions::get_previous_world_from_local(v.instance_index);
    out.previous_world_position = vec4<f32>(displaced(v.position, v.uv, v.pivot, prev, scatter.time.y), 1.0);
}
@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = v.instance_index;
@if(VISIBILITY_RANGE_DITHER)
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(v.instance_index, world_from_local[3]);
    return out;
}
