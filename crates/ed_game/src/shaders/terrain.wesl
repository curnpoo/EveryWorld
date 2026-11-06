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
    mesh_view_bindings::{view, lights},
};

struct GroundParams {
    size: array<vec4<f32>, 3>,
    bump: array<vec4<f32>, 3>,
    offset: vec4<f32>,
    // Far shadows: Earth centre in render space (w = 1 when enabled), pole axis.
    far_c: vec4<f32>,
    far_p: vec4<f32>,
};

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> ground: GroundParams;
@group(constants::MATERIAL_BIND_GROUP) @binding(101) var albedo_tex: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(102) var ground_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(103) var normal_tex: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(104) var macro_tex: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(105) var macro_sampler: sampler;
// Satellite colour pool, one array per tile resolution class. Each terrain
// mesh's MeshTag is (class + 1) << 24 | layer, 0 = no satellite colour.
@group(constants::MATERIAL_BIND_GROUP) @binding(106) var sat_512: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(107) var sat_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(108) var sat_256: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(109) var sat_128: texture_2d_array<f32>;
// Far-shadow pool (ed_world::farshadow): one layer per tile, layer + 1 in
// MeshTag bits 12-23. Horizon: 4 groups of 65 columns, channel = azimuth
// 4g+c (16 compass directions, sqrt-encoded elevation); row 65 = metadata.
// Surface: R top (ground+max(building, canopy)), G ground, B canopy share,
// A building height, metres above the tile's min height.
@group(constants::MATERIAL_BIND_GROUP) @binding(110) var far_horizon: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(111) var far_surface: texture_2d_array<f32>;
// Baked DEM detail (ed_world::detail_normal), same class/layer as the
// satellite colour; MeshTag bit 23 says it is present. R/G: east/south slope
// delta vs the mesh, B: curvature (+ gully), A: fine slope. 0.5 + atan(x)/pi.
@group(constants::MATERIAL_BIND_GROUP) @binding(112) var detail_512: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(113) var detail_256: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(114) var detail_128: texture_2d_array<f32>;

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
    @location(12) @interpolate(flat) sat_tag: u32,
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
    out.sat_tag = mesh_functions::get_tag(v.instance_index);
    return out;
}

// Satellite colour for this pixel from the shared pool. Texels sit on
// vertices, so uv is remapped to texel centres. Gradients are taken up front
// so the per-class branches stay legal in non-uniform control flow.
fn sample_sat(uv: vec2<f32>, tag: u32) -> vec4<f32> {
    let cls = tag >> 24u;
    let layer = i32(tag & 0xFFFu);
    var n = 256.0;
    if cls == 1u {
        n = 1024.0;
    } else if cls == 2u {
        n = 512.0;
    }
    let k = (n - 1.0) / n;
    let st = uv * k + 0.5 / n;
    let dx = dpdx(uv) * k;
    let dy = dpdy(uv) * k;
    if cls == 1u {
        return textureSampleGrad(sat_512, sat_sampler, st, layer, dx, dy);
    }
    if cls == 2u {
        return textureSampleGrad(sat_256, sat_sampler, st, layer, dx, dy);
    }
    if cls == 3u {
        return textureSampleGrad(sat_128, sat_sampler, st, layer, dx, dy);
    }
    return vec4<f32>(0.0);
}

// Baked detail for this pixel; see sample_sat for the texel remap.
fn sample_detail(uv: vec2<f32>, tag: u32) -> vec4<f32> {
    let cls = tag >> 24u;
    let layer = i32(tag & 0xFFFu);
    var n = 256.0;
    if cls == 1u {
        n = 1024.0;
    } else if cls == 2u {
        n = 512.0;
    }
    let k = (n - 1.0) / n;
    let st = uv * k + 0.5 / n;
    let dx = dpdx(uv) * k;
    let dy = dpdy(uv) * k;
    var d = vec4<f32>(0.5, 0.5, 0.5, 0.0);
    if (tag & 0x800000u) == 0u {
        return d;
    }
    if cls == 1u {
        d = textureSampleGrad(detail_512, sat_sampler, st, layer, dx, dy);
    } else if cls == 2u {
        d = textureSampleGrad(detail_256, sat_sampler, st, layer, dx, dy);
    } else if cls == 3u {
        d = textureSampleGrad(detail_128, sat_sampler, st, layer, dx, dy);
    }
    return d;
}

fn detail_dec(x: f32) -> f32 {
    return tan((clamp(x, 0.02, 0.98) - 0.5) * 3.14159265);
}

struct Layer {
    albedo: vec3<f32>,
    height: f32,
    normal: vec3<f32>, // world space, detail applied
    rough: f32,
    ao: f32,
};

const FAR_CANOPY_H: f32 = 14.0;   // = farshadow::CANOPY_H
const FAR_STEPS: i32 = 12;
const FAR_MARCH_M: f32 = 400.0;

// Horizon elevation (radians) from one horizon group at texel coords px.
fn far_hz(px: vec2<f32>, k: u32, layer: i32) -> f32 {
    let g = f32(k / 4u);
    let t = textureSampleLevel(far_horizon, sat_sampler, (px + vec2<f32>(g * 65.0, 0.0)) / vec2<f32>(260.0, 66.0), layer, 0.0);
    let s = t[k % 4u];
    return s * s * 1.5707963;
}

// Sun visibility past the cascades: terrain horizon map (unlimited reach,
// works from orbit), canopy self-darkening, and a short height-field march
// over buildings + canopy. 1 = lit.
fn far_shadow(wp: vec3<f32>, uv: vec2<f32>, tag: u32, L: vec3<f32>) -> f32 {
    let f = (tag >> 12u) & 0x7FFu; // bit 23 = detail flag
    if f == 0u {
        return 1.0;
    }
    let layer = i32(f - 1u);
    let up = normalize(wp - ground.far_c.xyz);
    let east = normalize(cross(ground.far_p.xyz, up));
    let north = cross(up, east);
    let le = dot(L, east);
    let ln = dot(L, north);
    let sin_e = dot(L, up);
    if sin_e < -0.02 {
        return 1.0; // night: the sun light is off anyway
    }
    let elev = asin(clamp(sin_e, -1.0, 1.0));
    var az = atan2(le, ln);
    if az < 0.0 {
        az += 6.2831853;
    }

    // Horizon map.
    let meta0 = textureLoad(far_horizon, vec2<i32>(0, 65), layer, 0);
    let meta1 = textureLoad(far_horizon, vec2<i32>(1, 65), layer, 0);
    let extent = bitcast<f32>(pack4x8unorm(meta0));
    let n = f32(pack4x8unorm(meta1));
    let px = vec2<f32>(0.5) + clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)) * (n - 1.0);
    let a = az / 6.2831853 * 16.0;
    let k0 = u32(floor(a)) % 16u;
    let k1 = (k0 + 1u) % 16u;
    let hz = mix(far_hz(px, k0, layer), far_hz(px, k1, layer), fract(a));
    let horizon = smoothstep(-0.009, 0.009, elev - hz);

    // Surface: canopy self-shadow (leaves shade the floor below them) and a
    // march towards the sun over buildings and trees.
    let s0 = textureSampleLevel(far_surface, sat_sampler, uv, layer, 0.0);
    let tan_e = max(tan(max(elev, 0.0)), 0.05);
    let canopy = 1.0 - 0.6 * s0.b * (1.0 - exp(-0.8 * s0.b / tan_e));
    var occ = 0.0;
    if extent / 128.0 <= 100.0 && horizon > 0.0 {
        let lh = max(length(vec2<f32>(le, ln)), 1e-4);
        let dir = vec2<f32>(le, -ln) / lh / extent;
        let h0 = s0.g + FAR_CANOPY_H * s0.b + 1.5;
        for (var i = 1; i <= FAR_STEPS; i++) {
            let t = FAR_MARCH_M * pow(f32(i) / f32(FAR_STEPS), 1.5);
            let suv = uv + dir * t;
            if any(suv < vec2<f32>(0.0)) || any(suv > vec2<f32>(1.0)) {
                break;
            }
            let top = textureSampleLevel(far_surface, sat_sampler, suv, layer, 0.0).r;
            occ = max(occ, smoothstep(0.0, 2.0, top - (h0 + t * tan_e)));
        }
    }
    return horizon * canopy * (1.0 - occ);
}

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

// Aperiodic macro noise. One tap of the tiled macro texture repeats every
// `s` metres, which reads as a grid of identical blotches from the air. The
// sum of two taps at an irrational scale ratio (golden) and unrelated
// rotations never repeats. (No domain warp: it smears the noise into
// swirls.) Contrast is restored after averaging.
fn mac(p: vec2<f32>, s: f32, a: f32, off: vec2<f32>) -> vec4<f32> {
    let x = textureSample(macro_tex, macro_sampler, rot2(p / s, a) + off);
    let y = textureSample(macro_tex, macro_sampler, rot2(p / (s * 1.618034), a + 2.39996) + off.yx + vec2<f32>(0.37, 0.11));
    return clamp((x + y - 1.0) * 0.72 + 0.5, vec4<f32>(0.0), vec4<f32>(1.0));
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

// Anti-tiling for single taps: a second copy of the layer, rotated, scaled by
// an irrational-ish factor and offset, swapped in by world-space noise `vm`
// (0..1). Transitions are narrow so both copies stay sharp (no ghosted
// double image); normals of the second copy are rotated back.
fn tap_var(i: i32, uv: vec2<f32>, dx: vec2<f32>, dy: vec2<f32>, vm: f32) -> Tap {
    let w = smoothstep(0.42, 0.58, vm);
    let ang = 2.1 + f32(i) * 0.37;
    let k = 0.73;
    var a: Tap;
    var b: Tap;
    if w < 0.999 {
        a = tap(i, uv, dx, dy);
    }
    if w > 0.001 {
        b = tap(i, rot2(uv * k, ang) + vec2<f32>(0.37, 0.61), rot2(dx * k, ang), rot2(dy * k, ang));
        let bn = rot2(vec2<f32>(b.nr.x, b.nr.y) * 2.0 - 1.0, -ang) * 0.5 + 0.5;
        b.nr = vec4<f32>(bn, b.nr.z, b.nr.w);
    }
    if w <= 0.001 {
        return a;
    }
    if w >= 0.999 {
        return b;
    }
    var t: Tap;
    t.ah = mix(a.ah, b.ah, w);
    t.nr = mix(a.nr, b.nr, w);
    return t;
}

fn hash22(p: vec2<f32>) -> vec2<f32> {
    let r = vec2<f32>(dot(p, vec2<f32>(127.1, 311.7)), dot(p, vec2<f32>(269.5, 183.3)));
    return fract(sin(r) * 43758.5453);
}

// Top-down projection with hex-grid stochastic tiling (Mikkelsen): the plane
// is cut into a triangle grid, each vertex gets its own random offset and
// rotation of the texture, and the three overlapping copies are blended by
// barycentric weight (sharpened), so no repeat survives. `hexw` < 0.5 falls
// back to one plain tap (far away, where mips already average the pattern).
fn sample_planar(i: i32, p: vec2<f32>, dp_dx: vec2<f32>, dp_dy: vec2<f32>, hexw: f32, n: vec3<f32>, strength: f32, vm: f32) -> Layer {
    let inv = 1.0 / layer_size(i);
    let st = p * inv;
    let dx = dp_dx * inv;
    let dy = dp_dy * inv;
    var ah = vec4<f32>(0.0);
    var nr = vec4<f32>(0.0);
    var nxy = vec2<f32>(0.0);
    if hexw < 0.5 {
        let t = tap_var(i, st, dx, dy, vm);
        ah = t.ah;
        nr = t.nr;
        nxy = vec2<f32>(t.nr.x, t.nr.y) * 2.0 - 1.0;
    } else {
        // Directional patterns (snow drifts, plough rows, sand ripples, slabs)
        // only shift; the rest also rotate.
        let directional = i == 5 || i == 6 || i == 9 || i == 10;
        let sk = st * 3.4641016;
        let skewed = vec2<f32>(sk.x, -0.57735027 * sk.x + 1.1547005 * sk.y);
        let base = floor(skewed);
        let f = fract(skewed);
        let z = 1.0 - f.x - f.y;
        var wv: vec3<f32>;
        var v: array<vec2<f32>, 3>;
        if z > 0.0 {
            wv = vec3<f32>(z, f.y, f.x);
            v[0] = base;
            v[1] = base + vec2<f32>(0.0, 1.0);
            v[2] = base + vec2<f32>(1.0, 0.0);
        } else {
            wv = vec3<f32>(-z, 1.0 - f.y, 1.0 - f.x);
            v[0] = base + vec2<f32>(1.0, 1.0);
            v[1] = base + vec2<f32>(1.0, 0.0);
            v[2] = base + vec2<f32>(0.0, 1.0);
        }
        wv = wv * wv * wv * wv;
        wv = wv / (wv.x + wv.y + wv.z);
        for (var k = 0; k < 3; k = k + 1) {
            if wv[k] < 0.01 {
                continue;
            }
            let h = hash22(v[k] + vec2<f32>(f32(i) * 17.0, f32(i) * 5.0));
            var ang = h.x * 6.2831853;
            if directional {
                ang = 0.0;
            }
            let off = h * 9.0;
            let t = tap(i, rot2(st, ang) + off, rot2(dx, ang), rot2(dy, ang));
            ah += t.ah * wv[k];
            nr += t.nr * wv[k];
            // Tangent normal back into world orientation.
            nxy += rot2(vec2<f32>(t.nr.x, t.nr.y) * 2.0 - 1.0, -ang) * wv[k];
        }
    }
    var l: Layer;
    l.albedo = ah.rgb;
    l.height = ah.a;
    let tn = nxy * strength * layer_bump(i);
    l.normal = normalize(n + vec3<f32>(tn.x, 0.0, tn.y));
    l.rough = nr.z;
    l.ao = nr.w;
    return l;
}

// Triplanar (UDN normal blend) for cliffs and steep banks.
fn sample_triplanar(i: i32, p: vec3<f32>, dp_dx: vec3<f32>, dp_dy: vec3<f32>, n: vec3<f32>, strength: f32, vm: f32) -> Layer {
    let inv = 1.0 / layer_size(i);
    var bw = pow(abs(n), vec3<f32>(4.0));
    bw = bw / (bw.x + bw.y + bw.z);
    // Each projection gets its own noise phase so the cliff, ledge and side
    // walls don't swap copies in lockstep.
    let tx = tap_var(i, p.zy * inv, dp_dx.zy * inv, dp_dy.zy * inv, fract(vm + 0.31));
    let ty = tap_var(i, p.xz * inv, dp_dx.xz * inv, dp_dy.xz * inv, vm);
    let tz = tap_var(i, p.xy * inv, dp_dx.xy * inv, dp_dy.xy * inv, fract(vm + 0.67));
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

// Fade-in weight for a noise octave of the given feature size (m): invisible
// while it is smaller than ~1.5 pixels of footprint, so no shimmer.
fn band(feature: f32, fp: f32) -> f32 {
    return smoothstep(fp * 1.5, fp * 6.0, feature);
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
    // DEM detail finer than the mesh: add the baked slope delta to the vertex
    // normal in heightfield form (x east, z south), so far ridges, gullies and
    // couloirs light correctly even where a quad spans hundreds of metres.
    let det = sample_detail(in.uv, in.sat_tag);
    var n = normalize(in.world_normal);
    {
        let ny = max(n.y, 0.08);
        // Top-down baked data smears down near-vertical walls: fade it there.
        let wall = 1.0 - smoothstep(0.55, 0.8, 1.0 - n.y);
        let dg = vec2<f32>(detail_dec(det.r), detail_dec(det.g)) * wall;
        n = normalize(n / ny + vec3<f32>(-dg.x, 0.0, -dg.y));
    }
    // Texture space: world meters plus the rebase-stable offset.
    let p3 = wp + vec3<f32>(ground.offset.x, 0.0, ground.offset.y);
    let p2 = p3.xz;
    let dist = length(view.world_position.xyz - wp);
    // Negative mip bias with distance: layer grain survives past ~100 m
    // instead of averaging to a flat swatch (anisotropic filter + TAA absorb
    // the extra shimmer).
    let gsc = mix(1.0, 0.6, smoothstep(30.0, 250.0, dist));
    let dp3_dx = dpdx(p3) * gsc;
    let dp3_dy = dpdy(p3) * gsc;
    let strength_base = ground.offset.z * mix(1.0, 0.6, smoothstep(60.0, 900.0, dist));

    // Macro variation at three unrelated scales.
    // Macro variation; scales rotated so their axes don't line up.
    let m_c = textureSample(macro_tex, macro_sampler, rot2(p2 / 1777.0, 0.83));
    let m_b = textureSample(macro_tex, macro_sampler, rot2(p2 / 263.0, 1.91));
    let m_a = mac(p2, 41.0, 0.31, vec2<f32>(0.0));
    // Mid-frequency detail: the 1-20 m band the scales above skip. Analytic
    // noise, so it doesn't average away with distance like layer mips do.
    let m_d = mac(p2, 23.0, 1.27, vec2<f32>(0.31, 0.17));
    let m_e = mac(p2, 67.0, 2.71, vec2<f32>(0.53, 0.81));
    let m_p = mac(p2, 29.0, 4.03, vec2<f32>(0.70, 0.20));
    // roughly -0.4..0.4
    let fb = (m_d.b - 0.5) * 0.6 + (m_d.g - 0.5) * 0.9 + (m_e.b - 0.5) * 0.7;
    let fb2 = (m_e.g - 0.5) * 1.2 + (m_d.r - 0.5) * 0.6;
    let cell = m_p.a - 0.5; // per-patch random: field / scrub patch identity
    // Anti-tiling selector (see tap_var): two unrelated scales so the swap
    // pattern itself has no period a viewer can pick out.
    let m_v = mac(p2, 157.0, 5.17, vec2<f32>(0.21, 0.67));
    let vm = clamp(m_v.g * 0.65 + m_e.a * 0.35 + (m_a.b - 0.5) * 0.25, 0.0, 1.0);
    let mid_fade = 1.0 - smoothstep(2500.0, 6000.0, dist);
    // Detail-normal strength drifts 0.55-1.35x across the ground.
    let strength = strength_base * mix(0.55, 1.35, clamp(m_b.a * 0.6 + m_v.b * 0.4, 0.0, 1.0));

    var w: array<f32, 12>;
    w[0] = in.splat0.x; w[1] = in.splat0.y; w[2] = in.splat0.z; w[3] = in.splat0.w;
    w[4] = in.splat1.x; w[5] = in.splat1.y; w[6] = in.splat1.z; w[7] = in.splat1.w;
    w[8] = in.splat2.x; w[9] = in.splat2.y; w[10] = in.splat2.z; w[11] = in.splat2.w;

    let slope = 1.0 - clamp(n.y, 0.0, 1.0);
    let tri_t = smoothstep(0.10, 0.22, slope);

    // Per-pixel alpine material. Splat weights come per vertex from the
    // vertex slope (surface.rs), which far away is a km-wide average. Redo the
    // steep-ground and snow rules here from the detailed normal and baked
    // curvature: rock on every buttress, scree on ledges between them, snow
    // that slides off steep faces, fills gullies, thins on sunny (south)
    // aspects and blows off convex ridges.
    let curv = clamp(detail_dec(det.b), -3.0, 3.0); // + gully, - ridge
    {
        let vn = normalize(in.world_normal);
        let vslope = 1.0 - clamp(vn.y, 0.0, 1.0);
        let jit = fb * 0.10;
        // Rock: add where the pixel is steeper than its vertex; where it is
        // flatter (a ledge in a face) the vertex's rock turns to scree.
        let cliff_v = smoothstep(0.18, 0.36, vslope);
        let cliff_p = smoothstep(0.18, 0.36, slope + jit);
        let add = max(cliff_p - cliff_v, 0.0) / max(1.0 - cliff_v, 1e-3);
        let drop = min(max(cliff_v - cliff_p, 0.0), w[8]);
        for (var i = 0; i < 12; i = i + 1) {
            w[i] = w[i] * (1.0 - add);
        }
        w[8] = w[8] + add - drop * (1.0 - add);
        w[7] = w[7] + drop * (1.0 - add);
        // Scree aprons in concave ground below faces.
        let apron = smoothstep(0.10, 0.22, slope + jit) * (1.0 - cliff_p) * smoothstep(0.0, 0.6, curv) * 0.6;
        for (var i = 0; i < 12; i = i + 1) {
            w[i] = w[i] * (1.0 - apron);
        }
        w[7] = w[7] + apron;

        // Snow: undo the vertex's steepness cut to recover "how far above the
        // snowline", then re-apply it per pixel with gully/ridge/aspect bias.
        let w9 = w[9];
        let keep_v = 1.0 - smoothstep(0.28, 0.45, vslope);
        let alt = clamp(w9 / max(keep_v, 0.08), 0.0, 1.0) * step(0.004, w9);
        let hold = clamp(alt * 1.25 + curv * 0.18 - n.z * 0.22 + fb * 0.35 - 0.1, 0.0, 1.0);
        let steep_p = slope - curv * 0.06 + n.z * 0.04 + jit;
        var snow_p = hold * (1.0 - smoothstep(0.30, 0.50, steep_p));
        // Ice caps / glaciers flagged by the vertex stay put.
        snow_p = max(snow_p, select(0.0, w9, w9 > 0.85 && slope < 0.5));
        let rest = 1.0 - w9;
        if rest > 0.05 {
            let k = (1.0 - snow_p) / rest;
            for (var i = 0; i < 12; i = i + 1) {
                w[i] = w[i] * k;
            }
        } else {
            for (var i = 0; i < 12; i = i + 1) {
                w[i] = 0.0;
            }
            w[8] = 1.0 - snow_p;
        }
        w[9] = snow_p;
    }

    // Ragged layer edges: push each layer's weight along its own noise so the
    // winner flips in patches instead of fading linearly between vertices.
    let edge_amt = 0.32 * (0.5 + 0.5 * smoothstep(10.0, 150.0, dist));
    var wn: array<f32, 12>;
    for (var i = 0; i < 12; i = i + 1) {
        let k = f32(i % 3) - 1.0;
        let k2 = f32((i / 3) % 2) * 2.0 - 1.0;
        wn[i] = max(w[i] + (fb * k + fb2 * k2 * 0.6) * edge_amt * 2.0, 0.0);
    }

    let hexw = 1.0 - step(1200.0, dist);

    // Pass 1: sample active layers.
    var layers: array<Layer, 12>;
    var hmax = -10.0;
    for (var i = 0; i < 12; i = i + 1) {
        if w[i] < 0.012 {
            continue;
        }
        var l: Layer;
        if tri_t < 0.01 {
            l = sample_planar(i, p2, dp3_dx.xz, dp3_dy.xz, hexw, n, strength, vm);
        } else if tri_t > 0.99 {
            l = sample_triplanar(i, p3, dp3_dx, dp3_dy, n, strength, vm);
        } else {
            let a = sample_planar(i, p2, dp3_dx.xz, dp3_dy.xz, hexw, n, strength, vm);
            let b = sample_triplanar(i, p3, dp3_dx, dp3_dy, n, strength, vm);
            l.albedo = mix(a.albedo, b.albedo, tri_t);
            l.height = mix(a.height, b.height, tri_t);
            l.normal = normalize(mix(a.normal, b.normal, tri_t));
            l.rough = mix(a.rough, b.rough, tri_t);
            l.ao = mix(a.ao, b.ao, tri_t);
        }
        layers[i] = l;
        hmax = max(hmax, l.height * 0.6 + wn[i]);
    }

    // Pass 2: height blend.
    let depth = mix(0.18, 0.09, smoothstep(60.0, 400.0, dist));
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
        let b = max(l.height * 0.6 + wn[i] - (hmax - depth), 0.0);
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
    // Mid-scale luminance and hue: warm/cool drift on every layer, plus
    // yellow-green <-> blue-green and per-patch tone on living ground.
    var living = 0.0;
    for (var i = 0; i < 12; i = i + 1) {
        if is_living(i) {
            living += w[i];
        }
    }
    living = clamp(living, 0.0, 1.0);
    // Snow stays near white: no darkening or blue/warm drift on it.
    let snow_w = w[9];
    let keep = 1.0 - snow_w * 0.9;
    albedo = albedo * (1.0 + fb * 0.55 * mid_fade * keep);
    albedo = albedo * (1.0 + fb2 * vec3<f32>(0.16, 0.03, -0.14) * mid_fade * keep);
    albedo = albedo * mix(vec3<f32>(1.0), 1.0 + cell * vec3<f32>(0.30, 0.16, 0.34), living * mid_fade);
    albedo = max(albedo, vec3<f32>(0.0));

    // Mid-scale relief: finite-difference normal from the same noise, so
    // slopes and hummocks catch the sun at 300 m where layer bump has faded.
    let gx = mac(p2 + vec2<f32>(1.5, 0.0), 23.0, 1.27, vec2<f32>(0.31, 0.17)).g - m_d.g;
    let gz = mac(p2 + vec2<f32>(0.0, 1.5), 23.0, 1.27, vec2<f32>(0.31, 0.17)).g - m_d.g;
    nd = normalize(nd + vec3<f32>(-gx, 0.0, -gz) * 1.6 * mid_fade * (1.0 - 0.7 * snow_w) * (1.0 - smoothstep(0.7, 0.95, slope)));

    // Bedrock strata and cavities. Strata: warped horizontal bands on rock,
    // light/dark by world height, so faces read as layered limestone/gneiss
    // rather than a smooth gradient. Cavity: gullies darken, crests catch
    // light (the AO a coarse mesh can't produce on its own).
    {
        let rock_w = clamp(w[8] + w[7] * 0.5, 0.0, 1.0) * (1.0 - snow_w);
        let hy = p3.y + (m_b.r - 0.5) * 30.0 + (m_a.g - 0.5) * 8.0;
        // Sharpened bands: thin dark seams between thicker pale beds.
        let b0 = sin(hy * 0.42) * 0.5 + sin(hy * 0.113 + 1.7) * 0.5;
        let band1 = sign(b0) * pow(abs(b0), 0.6);
        let strata = 1.0 + band1 * 0.30 * rock_w;
        albedo = albedo * strata * mix(vec3<f32>(1.0), vec3<f32>(1.04, 1.0, 0.95), clamp(band1, 0.0, 1.0) * rock_w);
        let cav = clamp(curv * 0.35, -0.4, 1.0);
        ao = ao * (1.0 - 0.5 * max(cav, 0.0)) * (1.0 + 0.15 * max(-cav, 0.0));
        albedo = albedo * (1.0 - 0.25 * max(cav, 0.0) * (1.0 - snow_w));
    }

    // Wetness (wetlands, lake beds): darker, glossier.
    let wet = in.color.a;
    albedo = albedo * (1.0 - 0.45 * wet);
    rough = mix(rough, 0.22, wet * 0.8);

    // Satellite albedo (gamma-encoded per vertex). alpha: 0 none, ~0.5 water, 1 land.
    // Per-pixel satellite colour: texels sit on vertices, so remap uv to texel centres.
    let sat_s = sample_sat(in.uv, in.sat_tag);
    let has_sat = smoothstep(0.2, 0.4, sat_s.a);
    let sat_lin = pow(max(sat_s.rgb, vec3<f32>(0.0)), vec3<f32>(2.2));
    // Water: satellite colour, very smooth, no ground texture or wet darkening.
    let water_f = has_sat * clamp((1.0 - sat_s.a) * 2.0, 0.0, 1.0);
    // Far field: real-world colour takes over with distance (texture detail
    // is kept as a luminance modulation so it doesn't go flat).
    // From ~120 m the satellite steers chroma and half the luminance (real
    // fields, forest edges, bare patches) while procedural grain stays.
    let lum_w = vec3<f32>(0.2126, 0.7152, 0.0722);
    let lp = dot(albedo, lum_w);
    let ls = max(dot(sat_lin, lum_w), 0.02);
    let chroma = min(sat_lin / ls, vec3<f32>(3.0));
    let steered = chroma * mix(lp, ls, 0.5 * (1.0 - snow_w));
    let steer_f = has_sat * smoothstep(120.0, 1500.0, dist) * 0.7 * (1.0 - snow_w * 0.8) * (1.0 - smoothstep(0.6, 0.85, slope));
    albedo = mix(albedo, steered, steer_f * (1.0 - water_f));
    // Snow decided per pixel above wins over the satellite's (seasonal,
    // blurry) snow where they disagree.
    let far_f = has_sat * smoothstep(600.0, 4500.0, dist) * 0.9 * (1.0 - 0.6 * snow_w);
    let detail = clamp(dot(albedo, lum_w) / max(dot(albedo, vec3<f32>(0.3333)), 0.02), 0.6, 1.4);
    // Satellite texels are 20-150 m, so far terrain is blurry. Re-add octaves
    // from 8 m up to km scale, each fading in only once it is >1.5 px wide.
    let fp = dist * 0.0016;
    var sat_col = sat_lin;
    {
        let m_f = mac(p2, 131.0, 3.37, vec2<f32>(0.13, 0.57));
        let m_g = mac(p2, 487.0, 0.57, vec2<f32>(0.29, 0.43));
        let m_h = textureSample(macro_tex, macro_sampler, p2 / 4100.0 + vec2<f32>(0.61, 0.07));
        let fn_ = (m_f.b - 0.5) * band(8.0, fp) + (m_f.g - 0.5) * band(16.0, fp)
            + (m_g.b - 0.5) * band(30.0, fp) + (m_g.g - 0.5) * band(60.0, fp)
            + (m_c.b - 0.5) * band(111.0, fp)
            + (m_h.b - 0.5) * band(256.0, fp) + (m_h.g - 0.5) * band(512.0, fp);
        let hue = (m_g.r - 0.5) * band(120.0, fp) + (m_h.r - 0.5) * band(900.0, fp);
        let amp = clamp(fn_ * 1.1, -0.45, 0.45) * (1.0 - snow_w * 0.85);
        let hv = hue * (1.0 - snow_w * 0.85);
        // Satellite saturation/contrast is flat; stretch it a little.
        sat_col = mix(vec3<f32>(dot(sat_col, lum_w)), sat_col, 1.2);
        sat_col = sat_col * (1.0 + amp) * (1.0 + hv * vec3<f32>(0.35, 0.08, -0.30));
        // Procedural-only far ground (no satellite tile) gets the same octaves.
        albedo = albedo * (1.0 + amp * (1.0 - has_sat) * smoothstep(200.0, 1500.0, dist));
    }
    albedo = mix(albedo, max(sat_col, vec3<f32>(0.0)) * mix(1.0, detail, 0.5), far_f);
    // Steep faces: satellite texels (20-150 m) smear shadowed and lit sides
    // into mud. Keep only their brightness as a hint and let bare rock read as
    // rock: neutral grey-brown, lit by the (detail) normal instead.
    {
        let rock_f = smoothstep(0.30, 0.55, slope) * (1.0 - snow_w) * (1.0 - water_f) * max(steer_f, far_f);
        let rl = clamp(dot(albedo, lum_w), 0.05, 0.30);
        let rock = vec3<f32>(1.0, 0.95, 0.88) * mix(0.17, rl, 0.35) * (1.0 + (m_b.g - 0.5) * 0.4);
        // Near-vertical walls: satellite texels are stretched into streaks; drop them entirely.
        albedo = mix(albedo, rock, rock_f * mix(0.85, 1.0, smoothstep(0.55, 0.8, slope)));
    }
    // Snow: lift toward near white and cool-neutral (satellite shadows and
    // layer darkening otherwise leave it a dull grey-blue).
    let snow_t = clamp(snow_w * 1.4, 0.0, 1.0);
    let lum_now = max(dot(albedo, lum_w), 0.02);
    albedo = albedo * mix(1.0, max(1.0, 0.78 / lum_now), snow_t * 0.9);
    albedo = mix(albedo, vec3<f32>(dot(albedo, lum_w)) * vec3<f32>(0.97, 0.99, 1.03), snow_t * 0.35);
    albedo = mix(albedo, sat_lin, water_f);
    // Far away, a mirror-smooth sea just reflects the hazy sky as a flat grey sheet
    // (and differs per tile); keep it glossy only up close.
    rough = mix(rough, mix(0.07, 0.35, smoothstep(2000.0, 60000.0, dist)), water_f);
    ao = mix(ao, 1.0, water_f);
    // From altitude, fresnel at grazing angles would reflect the (poorly probed) hazy
    // sky and wash the sea grey; let the satellite/navy albedo carry the colour.
    pbr_input.material.reflectance = pbr_input.material.reflectance * (1.0 - 0.85 * water_f * smoothstep(5000.0, 80000.0, dist));
    nd = normalize(mix(nd, n, water_f));

    // Dry ground is a rough, matte surface. Mip-averaged normals and the
    // distance-flattened albedo otherwise leave a sheen at grazing sun angles
    // (worst from far away). Floor the roughness, push it to ~1 with distance,
    // and fade specular reflectance out too. Water keeps its own gloss above.
    let land_f = 1.0 - water_f;
    let far_rough = smoothstep(25.0, 500.0, dist);
    rough = mix(rough, max(rough, 0.97), land_f * far_rough);
    rough = mix(rough, max(rough, 0.8), land_f * (1.0 - wet));
    pbr_input.material.reflectance = pbr_input.material.reflectance * (1.0 - 0.7 * land_f * smoothstep(60.0, 1200.0, dist));
    pbr_input.specular_occlusion = pbr_input.specular_occlusion * mix(1.0, 0.3, land_f * far_rough);

    pbr_input.material.base_color = vec4<f32>(albedo, 1.0);
    pbr_input.material.perceptual_roughness = clamp(rough, 0.04, 1.0);
    pbr_input.N = nd;
    pbr_input.diffuse_occlusion = pbr_input.diffuse_occlusion * ao;
    pbr_input.specular_occlusion = pbr_input.specular_occlusion * ao;

    // Far shadows take over where the last cascade fades out (see far_shadows.rs).
    var far_dbg = 1.0;
    if ground.far_c.w > 0.5 && lights.n_directional_lights > 0u {
        let sun = &lights.directional_lights[0];
        let cmax = (*sun).cascades[max((*sun).num_cascades, 1u) - 1u].far_bound;
        let w = smoothstep(0.75 * cmax, cmax, dist);
        if w > 0.0 {
            let fs = far_shadow(wp, in.uv, in.sat_tag, (*sun).direction_to_light);
            far_dbg = mix(1.0, fs, w);
            pbr_input.directional_shadow_factor *= far_dbg;
        }
    }

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    if ground.far_p.w > 0.5 {
        out.color = vec4<f32>(mix(out.color.rgb, vec3<f32>(1.0, 0.0, 1.0) * max(dot(out.color.rgb, vec3<f32>(0.3, 0.59, 0.11)), 0.05), 1.0 - far_dbg), out.color.a);
    }
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
