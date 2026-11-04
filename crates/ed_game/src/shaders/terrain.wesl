// Ground v2: terrain splat material.
//
// 12 tileable ground layers live in two texture arrays:
//   albedo_tex: sRGB albedo + height (a)
//   normal_tex: tangent normal xy, perceptual roughness, AO
// Per-vertex splat weights (3 x unorm8x4) say how much of each layer is
// here; per-pixel height blending makes the transitions crisp and natural
// (pebbles poke through snow, grass over dirt). Flat ground is projected
// top-down; steep ground switches to triplanar so cliffs don't smear.
// Several macro-noise scales modulate brightness and dryness to kill tiling.

import bevy_pbr::render::{
    mesh_functions,
    view_transformations::position_world_to_clip,
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    mesh_view_bindings::view,
};

struct GroundParams {
    size: array<vec4<f32>, 3>,
    bump: array<vec4<f32>, 3>,
    offset: vec4<f32>,
};

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> ground: GroundParams;
@group(constants::MATERIAL_BIND_GROUP) @binding(101) var albedo_tex: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(102) var ground_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(103) var normal_tex: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(104) var macro_tex: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(105) var macro_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(106) var sat_tex: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(107) var sat_sampler: sampler;

struct TerrainVertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(5) color: vec4<f32>,
    @location(8) splat0: vec4<f32>,
    @location(9) splat1: vec4<f32>,
    @location(10) splat2: vec4<f32>,
    @location(11) sat: vec4<f32>,
};

struct TerrainVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(5) color: vec4<f32>,
    @location(6) @interpolate(flat) instance_index: u32,
    @location(8) splat0: vec4<f32>,
    @location(9) splat1: vec4<f32>,
    @location(10) splat2: vec4<f32>,
    @location(11) sat: vec4<f32>,
};

@vertex
fn vertex(v: TerrainVertex) -> TerrainVertexOutput {
    var out: TerrainVertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(v.instance_index);
    out.world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(v.position, 1.0));
    out.position = position_world_to_clip(out.world_position.xyz);
    out.world_normal = mesh_functions::mesh_normal_local_to_world(v.normal, v.instance_index);
    out.uv = v.uv;
    out.color = v.color;
    out.instance_index = v.instance_index;
    out.splat0 = v.splat0;
    out.splat1 = v.splat1;
    out.splat2 = v.splat2;
    out.sat = v.sat;
    return out;
}

struct Layer {
    albedo: vec3<f32>,
    height: f32,
    normal: vec3<f32>, // world space, detail applied
    rough: f32,
    ao: f32,
};

fn layer_size(i: i32) -> f32 {
    return ground.size[i / 4][i % 4];
}

fn layer_bump(i: i32) -> f32 {
    return ground.bump[i / 4][i % 4];
}

fn rot2(p: vec2<f32>, a: f32) -> vec2<f32> {
    let c = cos(a);
    let s = sin(a);
    return vec2<f32>(c * p.x - s * p.y, s * p.x + c * p.y);
}

// One projection of one layer: albedo/height + (tangent normal xy, rough, ao).
struct Tap {
    ah: vec4<f32>,
    nr: vec4<f32>,
};

fn tap(i: i32, uv: vec2<f32>, dx: vec2<f32>, dy: vec2<f32>) -> Tap {
    var t: Tap;
    t.ah = textureSampleGrad(albedo_tex, ground_sampler, uv, i, dx, dy);
    t.nr = textureSampleGrad(normal_tex, ground_sampler, uv, i, dx, dy);
    return t;
}

// Top-down projection, sampled twice (rotated + shifted copy) and mixed by
// a macro mask, height-aware, so the repeat is hard to spot.
fn sample_planar(i: i32, p: vec2<f32>, dp_dx: vec2<f32>, dp_dy: vec2<f32>, mask: f32, n: vec3<f32>, strength: f32) -> Layer {
    let inv = 1.0 / layer_size(i);
    let a = tap(i, p * inv, dp_dx * inv, dp_dy * inv);
    // Directional patterns (snow drifts, plough rows, sand ripples, slabs)
    // only shift; the rest also rotate.
    var ang = 2.17 + f32(i) * 0.61;
    if i == 5 || i == 6 || i == 9 || i == 10 {
        ang = 0.0;
    }
    let b = tap(i, rot2(p * inv, ang) + vec2<f32>(0.37, 0.71), rot2(dp_dx * inv, ang), rot2(dp_dy * inv, ang));
    let ka = a.ah.a + (1.0 - mask) * 1.2;
    let kb = b.ah.a + mask * 1.2;
    let t = clamp((kb - ka) * 4.0 + 0.5, 0.0, 1.0);
    var l: Layer;
    l.albedo = mix(a.ah.rgb, b.ah.rgb, t);
    l.height = mix(a.ah.a, b.ah.a, t);
    let nra = vec2<f32>(a.nr.x, a.nr.y) * 2.0 - 1.0;
    let nrb = rot2(vec2<f32>(b.nr.x, b.nr.y) * 2.0 - 1.0, ang);
    let tn = mix(nra, nrb, t) * strength * layer_bump(i);
    l.normal = normalize(n + vec3<f32>(tn.x, 0.0, tn.y));
    l.rough = mix(a.nr.z, b.nr.z, t);
    l.ao = mix(a.nr.w, b.nr.w, t);
    return l;
}

// Triplanar (UDN normal blend) for cliffs and steep banks.
fn sample_triplanar(i: i32, p: vec3<f32>, dp_dx: vec3<f32>, dp_dy: vec3<f32>, n: vec3<f32>, strength: f32) -> Layer {
    let inv = 1.0 / layer_size(i);
    var bw = pow(abs(n), vec3<f32>(4.0));
    bw = bw / (bw.x + bw.y + bw.z);
    let tx = tap(i, p.zy * inv, dp_dx.zy * inv, dp_dy.zy * inv);
    let ty = tap(i, p.xz * inv, dp_dx.xz * inv, dp_dy.xz * inv);
    let tz = tap(i, p.xy * inv, dp_dx.xy * inv, dp_dy.xy * inv);
    let k = strength * layer_bump(i);
    let nx = (vec2<f32>(tx.nr.x, tx.nr.y) * 2.0 - 1.0) * k;
    let ny = (vec2<f32>(ty.nr.x, ty.nr.y) * 2.0 - 1.0) * k;
    let nz = (vec2<f32>(tz.nr.x, tz.nr.y) * 2.0 - 1.0) * k;
    let wx = vec3<f32>(n.x, nx.y + n.y, nx.x + n.z);
    let wy = vec3<f32>(ny.x + n.x, n.y, ny.y + n.z);
    let wz = vec3<f32>(nz.x + n.x, nz.y + n.y, n.z);
    var l: Layer;
    l.albedo = tx.ah.rgb * bw.x + ty.ah.rgb * bw.y + tz.ah.rgb * bw.z;
    l.height = tx.ah.a * bw.x + ty.ah.a * bw.y + tz.ah.a * bw.z;
    l.normal = normalize(wx * bw.x + wy * bw.y + wz * bw.z);
    l.rough = tx.nr.z * bw.x + ty.nr.z * bw.y + tz.nr.z * bw.z;
    l.ao = tx.nr.w * bw.x + ty.nr.w * bw.y + tz.nr.w * bw.z;
    return l;
}

fn is_living(i: i32) -> bool {
    // Grass, DryGrass, Moss, Farmland (see ed_world::surface::Ground).
    return i == 0 || i == 1 || i == 2 || i == 5;
}

@fragment
fn fragment(in: TerrainVertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    // Hand Bevy's PBR the parts it understands.
    var vo: VertexOutput;
    vo.position = in.position;
    vo.world_position = in.world_position;
    vo.world_normal = in.world_normal;
@if(VERTEX_UVS_A)
    vo.uv = in.uv;
@if(VERTEX_COLORS)
    vo.color = vec4<f32>(1.0);
@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    vo.instance_index = in.instance_index;
    var pbr_input = pbr_input_from_standard_material(vo, is_front);

    let wp = in.world_position.xyz;
    let n = normalize(in.world_normal);
    // Texture space: world meters plus the rebase-stable offset.
    let p3 = wp + vec3<f32>(ground.offset.x, 0.0, ground.offset.y);
    let p2 = p3.xz;
    let dp3_dx = dpdx(p3);
    let dp3_dy = dpdy(p3);
    let dist = length(view.world_position.xyz - wp);
    let strength = ground.offset.z * mix(1.0, 0.35, smoothstep(60.0, 600.0, dist));

    // Macro variation at three unrelated scales.
    let m_a = textureSample(macro_tex, macro_sampler, p2 / 41.0);
    let m_b = textureSample(macro_tex, macro_sampler, p2 / 263.0);
    let m_c = textureSample(macro_tex, macro_sampler, p2 / 1777.0);
    let mask = smoothstep(0.35, 0.65, m_a.g);

    var w: array<f32, 12>;
    w[0] = in.splat0.x; w[1] = in.splat0.y; w[2] = in.splat0.z; w[3] = in.splat0.w;
    w[4] = in.splat1.x; w[5] = in.splat1.y; w[6] = in.splat1.z; w[7] = in.splat1.w;
    w[8] = in.splat2.x; w[9] = in.splat2.y; w[10] = in.splat2.z; w[11] = in.splat2.w;

    let slope = 1.0 - clamp(n.y, 0.0, 1.0);
    let tri_t = smoothstep(0.10, 0.22, slope);

    // Pass 1: sample active layers.
    var layers: array<Layer, 12>;
    var hmax = -10.0;
    for (var i = 0; i < 12; i = i + 1) {
        if w[i] < 0.012 {
            continue;
        }
        var l: Layer;
        if tri_t < 0.01 {
            l = sample_planar(i, p2, dp3_dx.xz, dp3_dy.xz, mask, n, strength);
        } else if tri_t > 0.99 {
            l = sample_triplanar(i, p3, dp3_dx, dp3_dy, n, strength);
        } else {
            let a = sample_planar(i, p2, dp3_dx.xz, dp3_dy.xz, mask, n, strength);
            let b = sample_triplanar(i, p3, dp3_dx, dp3_dy, n, strength);
            l.albedo = mix(a.albedo, b.albedo, tri_t);
            l.height = mix(a.height, b.height, tri_t);
            l.normal = normalize(mix(a.normal, b.normal, tri_t));
            l.rough = mix(a.rough, b.rough, tri_t);
            l.ao = mix(a.ao, b.ao, tri_t);
        }
        layers[i] = l;
        hmax = max(hmax, l.height * 0.6 + w[i]);
    }

    // Pass 2: height blend.
    let depth = 0.18;
    var total = 0.0;
    var albedo = vec3<f32>(0.0);
    var nsum = vec3<f32>(0.0);
    var rough = 0.0;
    var ao = 0.0;
    let tint = in.color.rgb;
    let dry = clamp((m_b.r - 0.45) * 0.9 + (m_c.g - 0.5) * 0.8, -0.25, 0.45);
    for (var i = 0; i < 12; i = i + 1) {
        if w[i] < 0.012 {
            continue;
        }
        let l = layers[i];
        let b = max(l.height * 0.6 + w[i] - (hmax - depth), 0.0);
        if b <= 0.0 {
            continue;
        }
        var c = l.albedo;
        if is_living(i) {
            c = c * tint;
            c = mix(c, c * vec3<f32>(1.22, 1.06, 0.72), max(dry, 0.0));
            c = c * (1.0 + min(dry, 0.0) * vec3<f32>(0.4, -0.15, 0.3));
        }
        albedo += c * b;
        nsum += l.normal * b;
        rough += l.rough * b;
        ao += l.ao * b;
        total += b;
    }
    if total <= 0.0 {
        total = 1.0;
        albedo = vec3<f32>(0.5);
        nsum = n;
        rough = 0.9;
        ao = 1.0;
    }
    albedo = albedo / total;
    rough = rough / total;
    ao = ao / total;
    var nd = normalize(nsum);

    // Broad brightness variation so big areas never look like one swatch.
    let bright = mix(0.86, 1.14, m_a.r) * mix(0.84, 1.16, m_b.b) * mix(0.88, 1.12, m_c.a);
    albedo = albedo * bright;

    // Wetness (wetlands, lake beds): darker, glossier.
    let wet = in.color.a;
    albedo = albedo * (1.0 - 0.45 * wet);
    rough = mix(rough, 0.22, wet * 0.8);

    // Satellite albedo (gamma-encoded per vertex). alpha: 0 none, ~0.5 water, 1 land.
    // Per-pixel satellite colour: texels sit on vertices, so remap uv to texel centres.
    let sd = vec2<f32>(textureDimensions(sat_tex));
    let sat_s = textureSample(sat_tex, sat_sampler, in.uv * (sd - 1.0) / sd + 0.5 / sd);
    let has_sat = smoothstep(0.2, 0.4, sat_s.a);
    let sat_lin = pow(max(sat_s.rgb, vec3<f32>(0.0)), vec3<f32>(2.2));
    // Water: satellite colour, very smooth, no ground texture or wet darkening.
    let water_f = has_sat * clamp((1.0 - sat_s.a) * 2.0, 0.0, 1.0);
    // Far field: real-world colour takes over with distance (texture detail
    // is kept as a luminance modulation so it doesn't go flat).
    let far_f = has_sat * smoothstep(400.0, 4000.0, dist) * 0.9;
    let detail = clamp(dot(albedo, vec3<f32>(0.2126, 0.7152, 0.0722)) / max(dot(albedo, vec3<f32>(0.3333)), 0.02), 0.6, 1.4);
    albedo = mix(albedo, sat_lin * mix(1.0, detail, 0.35), far_f);
    albedo = mix(albedo, sat_lin, water_f);
    // Far away, a mirror-smooth sea just reflects the hazy sky as a flat grey sheet
    // (and differs per tile); keep it glossy only up close.
    rough = mix(rough, mix(0.07, 0.35, smoothstep(2000.0, 60000.0, dist)), water_f);
    ao = mix(ao, 1.0, water_f);
    // From altitude, fresnel at grazing angles would reflect the (poorly probed) hazy
    // sky and wash the sea grey; let the satellite/navy albedo carry the colour.
    pbr_input.material.reflectance = pbr_input.material.reflectance * (1.0 - 0.85 * water_f * smoothstep(5000.0, 80000.0, dist));
    nd = normalize(mix(nd, n, water_f));

    pbr_input.material.base_color = vec4<f32>(albedo, 1.0);
    pbr_input.material.perceptual_roughness = clamp(rough, 0.04, 1.0);
    pbr_input.N = nd;
    pbr_input.diffuse_occlusion = pbr_input.diffuse_occlusion * ao;
    pbr_input.specular_occlusion = pbr_input.specular_occlusion * ao;

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
