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
    mesh_view_bindings::lights,
};

struct ImpostorParams {
    // x: distance (m) up to which every tree shows, y: max growth,
    // z: projected size (px) below which a tree fades out, w: unused
    tune: vec4<f32>,
    // Far shadows (as GroundParams in terrain.wesl): Earth centre in render
    // space (w = on), pole axis (w = debug tint).
    far_c: vec4<f32>,
    far_p: vec4<f32>,
};

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> imp: ImpostorParams;
@group(constants::MATERIAL_BIND_GROUP) @binding(101) var albedo_tex: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(102) var imp_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(103) var normal_tex: texture_2d_array<f32>;
// Far-shadow horizon pool (far_shadows.rs); layer + 1 in MeshTag bits 12-23.
@group(constants::MATERIAL_BIND_GROUP) @binding(110) var far_horizon: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(111) var far_sampler: sampler;

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
    // Tile (u, v) of the tree's base.
    @location(10) tile_uv: vec2<f32>,
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
    // Live billboard axes in the tree's frame (see oct blend in `sample_impostor`).
    @location(9) @interpolate(flat) br: vec3<f32>,
    @location(10) @interpolate(flat) bu: vec3<f32>,
    // Live view direction in the tree's frame.
    @location(11) @interpolate(flat) vl: vec3<f32>,
    // Far shadows: tree centre (world), tile uv, MeshTag.
    @location(12) @interpolate(flat) centre: vec3<f32>,
    @location(13) @interpolate(flat) tile_uv: vec2<f32>,
    @location(14) @interpolate(flat) tag: u32,
};

fn oct_encode(d: vec3<f32>) -> vec2<f32> {
    let dd = vec3<f32>(d.x, max(d.y, 0.0), d.z);
    let n = dd / max(abs(dd.x) + abs(dd.y) + abs(dd.z), 1e-6);
    return vec2<f32>(n.x + n.z, n.x - n.z) * 0.5 + 0.5;
}

fn oct_decode(uv: vec2<f32>) -> vec3<f32> {
    let g = uv * 2.0 - 1.0;
    let x = (g.x + g.y) * 0.5;
    let z = (g.x - g.y) * 0.5;
    return normalize(vec3<f32>(x, 1.0 - abs(x) - abs(z), z));
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
    let no_thin = v.params.z < 0.0;
    let kept = max(abs(v.params.z), 1e-6);
    let near = v.params.w;
    let f = min(1.0, pow(imp.tune.x / d, 2.0));
    let keep_here = f / kept;
    // Wide ramp so a tree fades in over a ~40% change in share, not a pop.
    var s = 1.0;
    if !no_thin {
        s = clamp((keep_here - v.axis.w) / max(0.4 * keep_here, 0.004), 0.0, 1.0);
    }
    // Three quarters of the thinning compensation (pow 0.75): far crowns still
    // grow to cover thinned neighbours, but shrink with distance instead of
    // staying the same size out to the cap.
    var grow = 1.0;
    // Fine tiles meet the leaf cards at `near`: match their size there, grow
    // to full canopy cover further out.

    if near > 0.0 {
        s = s * smoothstep(near - 20.0, near + 20.0, d);
    }
    let r = radius * grow;
    let px = 2.0 * r * view.clip_from_view[1][1] * view.viewport.w * 0.5 / d;
    // A quad under ~2 px flickers (rasteriser drops it); ramp it out quickly
    // before that. The far tint already matches the satellite colour.
    s = s * smoothstep(imp.tune.z * 0.8, imp.tune.z * 1.3, px);
    // Pull the quad toward the camera a little so slopes don't cut it.
    // Per-tree depth offset (0.2..0.6 r toward camera), fixed for each tree:
    // overlapping crowns get distinct depths, so the same one wins every frame.
    let tie = fract(sin(dot(c.xz * 0.01, vec2<f32>(12.9898, 78.233))) * 43758.5453);
    let off = (right_w * v.corner.x + up_w * v.corner.y) * (r * PAD * s) + vw * (r * (0.2 + 0.4 * tie) * s);
    let wp = c + off;

    // Corner uv in 0..1 (y down); the fragment picks and blends atlas frames.
    out.uv = vec2<f32>(v.corner.x * 0.5 + 0.5, 0.5 - v.corner.y * 0.5);
    out.br = br;
    out.bu = bu;
    out.vl = vl;
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
    out.centre = c;
    out.tile_uv = v.tile_uv;
    out.tag = mesh_functions::get_tag(v.instance_index);
    return out;
}

struct Sampled {
    albedo: vec4<f32>,
    normal: vec3<f32>,
};

fn sample_impostor(in: ImpostorOut) -> Sampled {
    var s: Sampled;
    // The pixel's offset from the tree centre, in the live billboard basis
    // (units of the frame half-width), then in the tree's own frame.
    let c = vec2<f32>(in.uv.x * 2.0 - 1.0, 1.0 - in.uv.y * 2.0);
    let p = in.br * c.x + in.bu * c.y;
    // Blend the four atlas views around the live direction (bilinear in the
    // octahedral grid), each reprojected with its own basis, so turning the
    // camera never snaps between views.
    let g = oct_encode(in.vl) * FRAMES - vec2<f32>(0.5);
    let g0 = floor(g);
    let t = g - g0;
    var acc = vec3<f32>(0.0);
    var nacc = vec3<f32>(0.0);
    var asum = 0.0;
    var wsum = 0.0;
    for (var k = 0u; k < 4u; k = k + 1u) {
        let o = vec2<f32>(f32(k & 1u), f32(k >> 1u));
        let w = mix(1.0 - t.x, t.x, o.x) * mix(1.0 - t.y, t.y, o.y);
        let fr = clamp(g0 + o, vec2<f32>(0.0), vec2<f32>(FRAMES - 1.0));
        let dk = oct_decode((fr + vec2<f32>(0.5)) / FRAMES);
        let u0 = vec3<f32>(0.0, 1.0, 0.0) - dk * dk.y;
        var buk = vec3<f32>(0.0, 0.0, 1.0);
        if dot(u0, u0) > 1e-6 {
            buk = normalize(u0);
        }
        let brk = normalize(cross(buk, dk));
        let q = vec2<f32>(dot(p, brk), dot(p, buk));
        let inside = step(abs(q.x), 1.0) * step(abs(q.y), 1.0);
        let uvk = clamp(vec2<f32>(q.x * 0.5 + 0.5, 0.5 - q.y * 0.5), vec2<f32>(0.015), vec2<f32>(0.985));
        let auv = (fr + uvk) / FRAMES;
        let a = textureSample(albedo_tex, imp_sampler, auv, in.layer);
        let n = textureSample(normal_tex, imp_sampler, auv, in.layer).xyz * 2.0 - 1.0;
        let wa = w * a.a * inside;
        acc = acc + a.rgb * wa;
        nacc = nacc + n * wa;
        asum = asum + wa;
        wsum = wsum + w;
    }
    s.albedo = vec4<f32>(acc / max(asum, 1e-5), asum / max(wsum, 1e-5));
    let nt = nacc / max(asum, 1e-5);
    let az = cross(in.ax, in.up);
    s.normal = normalize(in.ax * nt.x + in.up * nt.y + az * nt.z + vec3<f32>(0.0, 1e-5, 0.0));
    return s;
}

@if(PREPASS_PIPELINE)
@fragment
fn fragment(in: ImpostorOut) -> FragmentOutput {
    if length(view.world_position.xyz - in.centre) < 230.0 {
        discard;
    }
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

// Horizon elevation (radians) from one horizon group at texel coords px
// (layout: ed_world::farshadow, same decode as terrain.wesl `far_hz`).
fn far_hz(px: vec2<f32>, k: u32, layer: i32) -> f32 {
    let g = f32(k / 4u);
    let t = textureSampleLevel(far_horizon, far_sampler, (px + vec2<f32>(g * 65.0, 0.0)) / vec2<f32>(260.0, 66.0), layer, 0.0);
    let s = t[k % 4u];
    return s * s * 1.5707963;
}

// Terrain horizon visibility at the tree (1 = lit): a forest on a slope in
// a ridge's shadow darkens with the ground under it. The terrain's canopy
// and height-field terms are for the floor below the leaves, not the crowns.
fn far_tree_shadow(wp: vec3<f32>, uv: vec2<f32>, tag: u32, L: vec3<f32>) -> f32 {
    let f = (tag >> 12u) & 0x7FFu; // bit 23 = detail flag
    if f == 0u {
        return 1.0;
    }
    let layer = i32(f - 1u);
    let up = normalize(wp - imp.far_c.xyz);
    let east = normalize(cross(imp.far_p.xyz, up));
    let north = cross(up, east);
    let sin_e = dot(L, up);
    if sin_e < -0.02 {
        return 1.0;
    }
    let elev = asin(clamp(sin_e, -1.0, 1.0));
    var az = atan2(dot(L, east), dot(L, north));
    if az < 0.0 {
        az += 6.2831853;
    }
    let meta1 = textureLoad(far_horizon, vec2<i32>(1, 65), layer, 0);
    let n = f32(pack4x8unorm(meta1));
    let px = vec2<f32>(0.5) + clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)) * (n - 1.0);
    let a = az / 6.2831853 * 16.0;
    let k0 = u32(floor(a)) % 16u;
    let k1 = (k0 + 1u) % 16u;
    let hz = mix(far_hz(px, k0, layer), far_hz(px, k1, layer), fract(a));
    return smoothstep(-0.009, 0.009, elev - hz);
}

@if(!PREPASS_PIPELINE)
@fragment
fn fragment(in: ImpostorOut, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    if length(view.world_position.xyz - in.centre) < 230.0 {
        discard;
    }
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
    // Narrow shade range: the photo already carries canopy darkness, so a
    // wide range double-darkens distant forest below the ground it sits on.
    let shade = clamp(dot(s.albedo.rgb, vec3<f32>(0.3, 0.6, 0.1)) / 0.08, 0.85, 1.15);
    let w = in.tint.a * smoothstep(400.0, 2500.0, dist) * 1.0;
    let albedo = mix(s.albedo.rgb, in.tint.rgb * shade, w);
    pbr_input.material.base_color = vec4<f32>(albedo, 1.0);
    // Far crowns merge into a smooth canopy: lean the per-tree normal toward
    // the tree's up so lighting stops flipping with every view change.
    let nfar = smoothstep(500.0, 2500.0, dist) * 0.85;
    let n_lit = normalize(mix(s.normal, in.up, nfar));
    pbr_input.N = n_lit;
    pbr_input.world_normal = n_lit;
    pbr_input.specular_occlusion = pbr_input.specular_occlusion * 0.25;
    // Far shadows take over where the last cascade fades out (as terrain.wesl).
    var far_dbg = 1.0;
    if imp.far_c.w > 0.5 && lights.n_directional_lights > 0u {
        let sun = &lights.directional_lights[0];
        let cmax = (*sun).cascades[max((*sun).num_cascades, 1u) - 1u].far_bound;
        let fw = smoothstep(0.75 * cmax, cmax, dist);
        if fw > 0.0 {
            far_dbg = mix(1.0, far_tree_shadow(in.centre, in.tile_uv, in.tag, (*sun).direction_to_light), fw);
            pbr_input.directional_shadow_factor *= far_dbg;
        }
    }
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    if imp.far_p.w > 0.5 {
        out.color = vec4<f32>(mix(out.color.rgb, vec3<f32>(0.0, 1.0, 1.0) * max(dot(out.color.rgb, vec3<f32>(0.3, 0.59, 0.11)), 0.05), 1.0 - far_dbg), out.color.a);
    }
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
