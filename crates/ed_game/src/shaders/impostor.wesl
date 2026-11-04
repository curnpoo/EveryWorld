// Tree impostors: one camera-facing quad per tree, textured from a
// hemi-octahedral atlas (ed_world::trees::impostor; keep the mapping and
// basis below in sync with oct_encode / view_basis there).
//
// Thinning with distance: the share of real trees shown falls as
// (full_m / d)^2, and the tile's data may already hold only a share of the
// real trees; the trees shown grow by 1/sqrt(share) (capped), so canopy
// cover holds while the vertex count on screen stays bounded. A tree
// smaller than a few pixels shrinks to nothing, which hands the forest over
// to the satellite colour. Same vertex code in the main pass and prepass so
// depth, normals and motion vectors agree.

import bevy_pbr::render::{
    mesh_functions,
    mesh_view_bindings::view,
    view_transformations::position_world_to_clip,
};

@if(PREPASS_PIPELINE)
import bevy_pbr::prepass::{bindings as prepass_bindings, io::FragmentOutput};
@else
import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
};

struct ImpostorParams {
    // x: distance (m) up to which every tree shows, y: max growth,
    // z: projected size (px) below which a tree fades out, w: unused
    tune: vec4<f32>,
};

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> imp: ImpostorParams;
@group(constants::MATERIAL_BIND_GROUP) @binding(101) var albedo_tex: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(102) var imp_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(103) var normal_tex: texture_2d_array<f32>;

const FRAMES: f32 = 8.0;
// Frame side in bounding radii (sphere plus a 1 px margin at 32 px).
const PAD: f32 = 1.0666667;

struct ImpostorVertex {
    @builtin(instance_index) instance_index: u32,
    // Bounding-sphere centre.
    @location(0) position: vec3<f32>,
    // Tree up.
    @location(1) normal: vec3<f32>,
    // Quad corner, -1..1.
    @location(2) corner: vec2<f32>,
    // Satellite canopy colour, alpha = weight.
    @location(5) tint: vec4<f32>,
    // radius m, atlas layer, share of real trees kept, near cut-off m
    @location(8) params: vec4<f32>,
    // tree-local +X, w: thinning rank
    @location(9) axis: vec4<f32>,
};

struct ImpostorOut {
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) @interpolate(flat) layer: u32,
    @location(3) @interpolate(flat) ax: vec3<f32>,
    @location(4) @interpolate(flat) up: vec3<f32>,
    @location(5) previous_world_position: vec4<f32>,
    @location(6) @interpolate(flat) instance_index: u32,
    @location(7) unclipped_depth: f32,
    @location(8) @interpolate(flat) tint: vec4<f32>,
};

fn oct_encode(d: vec3<f32>) -> vec2<f32> {
    let dd = vec3<f32>(d.x, max(d.y, 0.0), d.z);
    let n = dd / max(abs(dd.x) + abs(dd.y) + abs(dd.z), 1e-6);
    return vec2<f32>(n.x + n.z, n.x - n.z) * 0.5 + 0.5;
}

@vertex
fn vertex(v: ImpostorVertex) -> ImpostorOut {
    var out: ImpostorOut;
    let wfl = mesh_functions::get_world_from_local(v.instance_index);
    let c = (wfl * vec4<f32>(v.position, 1.0)).xyz;
    let up = normalize((wfl * vec4<f32>(v.normal, 0.0)).xyz);
    var ax = (wfl * vec4<f32>(v.axis.xyz, 0.0)).xyz;
    ax = normalize(ax - up * dot(ax, up));
    let az = cross(ax, up);

    let to_cam = view.world_position.xyz - c;
    let d = max(length(to_cam), 1e-3);
    let vw = to_cam / d;
    // View direction in the tree's frame, clamped to the upper hemisphere.
    var vl = vec3<f32>(dot(vw, ax), max(dot(vw, up), 0.0), dot(vw, az));
    vl = normalize(vl + vec3<f32>(0.0, 1e-4, 0.0));
    let u0 = vec3<f32>(0.0, 1.0, 0.0) - vl * vl.y;
    var bu = vec3<f32>(0.0, 0.0, 1.0);
    if dot(u0, u0) > 1e-6 {
        bu = normalize(u0);
    }
    let br = normalize(cross(bu, vl));
    let right_w = ax * br.x + up * br.y + az * br.z;
    let up_w = ax * bu.x + up * bu.y + az * bu.z;

    let radius = v.params.x;
    let kept = max(v.params.z, 1e-6);
    let near = v.params.w;
    let f = min(1.0, pow(imp.tune.x / d, 2.0));
    let keep_here = f / kept;
    var s = clamp((keep_here - v.axis.w) / max(0.2 * keep_here, 0.002), 0.0, 1.0);
    var grow = min(inverseSqrt(max(min(f, kept), 1e-6)), imp.tune.y);
    // Fine tiles meet the leaf cards at `near`: match their size there, grow
    // to full canopy cover further out.
    if near > 0.0 {
        grow = mix(1.0, grow, smoothstep(near + 300.0, near + 1500.0, d));
    }
    if near > 0.0 {
        s = s * smoothstep(near - 20.0, near + 20.0, d);
    }
    let r = radius * grow;
    let px = 2.0 * r * view.clip_from_view[1][1] * view.viewport.w * 0.5 / d;
    s = s * smoothstep(imp.tune.z * 0.5, imp.tune.z, px);
    // Pull the quad toward the camera a little so slopes don't cut it.
    let off = (right_w * v.corner.x + up_w * v.corner.y) * (r * PAD * s) + vw * (r * 0.4 * s);
    let wp = c + off;

    let frame = clamp(floor(oct_encode(vl) * FRAMES), vec2<f32>(0.0), vec2<f32>(FRAMES - 1.0));
    out.uv = (frame + vec2<f32>(v.corner.x * 0.5 + 0.5, 0.5 - v.corner.y * 0.5)) / FRAMES;
    out.layer = u32(v.params.y + 0.5);
    out.ax = ax;
    out.up = up;
    out.world_position = vec4<f32>(wp, 1.0);
    out.position = position_world_to_clip(wp);
    out.unclipped_depth = out.position.z;
@if(PREPASS_PIPELINE && UNCLIPPED_DEPTH_ORTHO_EMULATION) {
    out.position.z = min(out.position.z, 1.0);
}
    let prev = mesh_functions::get_previous_world_from_local(v.instance_index);
    out.previous_world_position = vec4<f32>((prev * vec4<f32>(v.position, 1.0)).xyz + off, 1.0);
    out.instance_index = v.instance_index;
    out.tint = v.tint;
    return out;
}

struct Sampled {
    albedo: vec4<f32>,
    normal: vec3<f32>,
};

fn sample_impostor(in: ImpostorOut) -> Sampled {
    var s: Sampled;
    s.albedo = textureSample(albedo_tex, imp_sampler, in.uv, in.layer);
    let nt = textureSample(normal_tex, imp_sampler, in.uv, in.layer).xyz * 2.0 - 1.0;
    let az = cross(in.ax, in.up);
    s.normal = normalize(in.ax * nt.x + in.up * nt.y + az * nt.z + vec3<f32>(0.0, 1e-5, 0.0));
    return s;
}

@if(PREPASS_PIPELINE)
@fragment
fn fragment(in: ImpostorOut) -> FragmentOutput {
    let s = sample_impostor(in);
    if s.albedo.a < 0.5 {
        discard;
    }
    var out: FragmentOutput;
@if(NORMAL_PREPASS)
    out.normal = vec4<f32>(s.normal * 0.5 + vec3<f32>(0.5), 1.0);
@if(UNCLIPPED_DEPTH_ORTHO_EMULATION)
    out.frag_depth = in.unclipped_depth;
@if(MOTION_VECTOR_PREPASS) {
    let clip_t = view.unjittered_clip_from_world * in.world_position;
    let prev_t = prepass_bindings::previous_view_uniforms.unjittered_clip_from_world * in.previous_world_position;
    out.motion_vector = (clip_t.xy / clip_t.w - prev_t.xy / prev_t.w) * vec2<f32>(0.5, -0.5);
}
    return out;
}

@if(!PREPASS_PIPELINE)
@fragment
fn fragment(in: ImpostorOut, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    let s = sample_impostor(in);
    if s.albedo.a < 0.5 {
        discard;
    }
    var vo: VertexOutput;
    vo.position = in.position;
    vo.world_position = in.world_position;
    vo.world_normal = s.normal;
@if(VERTEX_UVS_A)
    vo.uv = in.uv;
@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    vo.instance_index = in.instance_index;
    var pbr_input = pbr_input_from_standard_material(vo, true);
    // Far away, take the canopy colour from the satellite photo (keeping the
    // impostor's own shading), so distant forest matches the ground under it
    // instead of speckling it with brighter crowns.
    let dist = length(view.world_position.xyz - in.world_position.xyz);
    let shade = clamp(dot(s.albedo.rgb, vec3<f32>(0.3, 0.6, 0.1)) / 0.08, 0.6, 1.4);
    let w = in.tint.a * smoothstep(600.0, 3000.0, dist) * 0.9;
    let albedo = mix(s.albedo.rgb, in.tint.rgb * shade, w);
    pbr_input.material.base_color = vec4<f32>(albedo, 1.0);
    pbr_input.N = s.normal;
    pbr_input.world_normal = s.normal;
    pbr_input.specular_occlusion = pbr_input.specular_occlusion * 0.25;
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
