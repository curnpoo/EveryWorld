// Map-globe atmosphere: an additive ray-marched shell around the unit-radius
// Earth, drawn as one screen-covering triangle. It reads the camera from the
// view uniforms, so it works from low orbit out to the Moon's distance.
// Output is raw display-referred radiance (the globe camera's exposure is
// compensated by `gain`), added on top of the lit planet and the stars.
// Alpha MUST be 0: Bevy's AlphaMode::Add uses the premultiplied pipeline
// (src + (1 - src.a) * dst), so alpha 1 would paint over the whole planet.

import bevy_pbr::render::mesh_view_bindings as vb;

struct AtmoUniform {
    // xyz: direction to the sun (globe space), w: overall gain.
    sun: vec4<f32>,
    // x: atmosphere top radius, y: scale height, z: cloud/haze whiteness, w: unused.
    shape: vec4<f32>,
};

@group(3) @binding(0) var<uniform> atmo: AtmoUniform;

struct VOut {
    @builtin(position) pos: vec4<f32>,
};

@vertex
fn vertex(@location(0) position: vec3<f32>) -> VOut {
    var o: VOut;
    o.pos = vec4<f32>(position.xy, 0.5, 1.0);
    return o;
}

// Ray vs sphere at the origin: (t_near, t_far), or (-1, -1) on a miss.
fn hit(ro: vec3<f32>, rd: vec3<f32>, r: f32) -> vec2<f32> {
    let b = dot(ro, rd);
    let c = dot(ro, ro) - r * r;
    let d = b * b - c;
    if d < 0.0 {
        return vec2<f32>(-1.0, -1.0);
    }
    let s = sqrt(d);
    return vec2<f32>(-b - s, -b + s);
}

const RAYLEIGH: vec3<f32> = vec3<f32>(0.18, 0.42, 1.0);
const SUNSET: vec3<f32> = vec3<f32>(1.0, 0.38, 0.12);
const STEPS: i32 = 28;

@fragment
fn fragment(in: VOut) -> @location(0) vec4<f32> {
    let view = vb::view;
    let uv = (in.pos.xy - view.viewport.xy) / view.viewport.zw;
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let far_w = view.world_from_clip * vec4<f32>(ndc, 1.0, 1.0);
    let ro = view.world_position;
    let rd = normalize(far_w.xyz / far_w.w - ro);
    let sun = normalize(atmo.sun.xyz);

    let r_top = atmo.shape.x;
    let h_scale = atmo.shape.y;
    let shell = hit(ro, rd, r_top);
    if shell.y < 0.0 {
        return vec4<f32>(0.0);
    }
    var t0 = max(shell.x, 0.0);
    var t1 = shell.y;
    let ground = hit(ro, rd, 1.0);
    if ground.x > 0.0 {
        t1 = min(t1, ground.x);
    }
    let len = t1 - t0;
    if len <= 0.0 {
        return vec4<f32>(0.0);
    }

    let dt = len / f32(STEPS);
    var acc = vec3<f32>(0.0);
    var optical = 0.0;
    for (var i = 0; i < STEPS; i = i + 1) {
        let p = ro + rd * (t0 + (f32(i) + 0.5) * dt);
        let r = length(p);
        let dens = exp(-max(r - 1.0, 0.0) / h_scale);
        let n = p / r;
        let mu = dot(n, sun);
        // Sunlight reaching this point: dims through the terminator, goes
        // red where it grazes a long path through the air.
        let lit = smoothstep(-0.18, 0.22, mu);
        let grazing = 1.0 - smoothstep(0.0, 0.45, mu);
        let tint = mix(RAYLEIGH, SUNSET, grazing * grazing * lit);
        optical = optical + dens * dt;
        let view_ext = exp(-optical * 9.0);
        acc = acc + tint * (lit * dens * dt * view_ext);
    }
    // Rayleigh phase (symmetric) plus a little forward glow toward the sun.
    let cos_t = dot(rd, sun);
    let phase = 0.55 + 0.45 * cos_t * cos_t + 0.35 * pow(max(cos_t, 0.0), 8.0);
    let col = acc * phase * atmo.sun.w;
    return vec4<f32>(col, 0.0);
}
