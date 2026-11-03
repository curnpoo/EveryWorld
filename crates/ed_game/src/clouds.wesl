// Volumetric clouds: raymarched spherical shell around the planet.
// See clouds.rs for the big picture. Units are meters and lux; the result is
// multiplied by the view exposure like every other lit surface.
//
// Noise space: ECEF meters minus wind drift, modulo 262,144 m (computed on
// the CPU in f64). Every texture period below divides that, so the field is
// continuous across floating-origin rebases.

import bevy_pbr::render::mesh_view_bindings as vb;
import bevy_pbr::prepass::io as pio;

struct CloudUniform {
    noise_from_render: mat3x3<f32>,
    cam_noise: vec4<f32>,
    cam_pos: vec4<f32>,
    cam_up: vec4<f32>,
    planet: vec4<f32>,
    cu: vec4<f32>,
    st: vec4<f32>,
    misc: vec4<f32>,
    wind: vec4<f32>,
    shell: vec4<f32>,
    sun: vec4<f32>,
    sky_ambient: vec4<f32>,
    ground_ambient: vec4<f32>,
    haze: vec4<f32>,
    storm_pos: array<vec4<f32>, 8>,
    storm_up: array<vec4<f32>, 8>,
};

@group(3) @binding(0) var<uniform> cl: CloudUniform;
@group(3) @binding(1) var shape_tex: texture_3d<f32>;
@group(3) @binding(2) var shape_smp: sampler;
@group(3) @binding(3) var detail_tex: texture_3d<f32>;
@group(3) @binding(4) var detail_smp: sampler;

const PI: f32 = 3.14159265;

// Set by the shadow caster: stratus decks dim the sun globally instead of
// casting patchy shadows.
var<private> g_shadow_pass: bool = false;
const SHAPE_PERIOD: f32 = 8192.0;
const DETAIL_PERIOD: f32 = 1024.0;
const WEATHER_PERIOD: f32 = 65536.0;
// Extinction per meter at density 1. Real cumulus is ~0.05-0.1; a bit lower
// keeps the edges soft and translucent (storybook, not hard cotton).
const SIGMA: f32 = 0.036;
// Vertical optical depth of the clear atmosphere at sea level.
const TAU_RAYLEIGH: vec3<f32> = vec3<f32>(0.0464, 0.1080, 0.2650);
// Includes a moderate aerosol load (AOD ~0.06) so low sun goes deep orange.
const TAU_MIE: f32 = 0.06;
const TAU_OZONE: vec3<f32> = vec3<f32>(0.0140, 0.0420, 0.0020);

struct VOut {
    @builtin(position) pos: vec4<f32>,
};

@if(!PREPASS_PIPELINE)
@vertex
fn vertex(@location(0) position: vec3<f32>) -> VOut {
    var o: VOut;
    o.pos = vec4<f32>(position.xy, 0.5, 1.0);
    return o;
}

fn saturate(x: f32) -> f32 {
    return clamp(x, 0.0, 1.0);
}

fn remap(v: f32, lo: f32, hi: f32, nlo: f32, nhi: f32) -> f32 {
    return nlo + (v - lo) / max(hi - lo, 1e-5) * (nhi - nlo);
}

fn hg(cos_t: f32, g: f32) -> f32 {
    let g2 = g * g;
    return (1.0 - g2) / (4.0 * PI * pow(max(1.0 + g2 - 2.0 * g * cos_t, 1e-4), 1.5));
}

// Silver lining (strong forward lobe) plus a soft back lobe.
fn phase(cos_t: f32, k: f32) -> f32 {
    return mix(hg(cos_t, 0.78 * k), hg(cos_t, -0.25 * k), 0.3);
}

// Distances along a ray (from altitude h_c, cosine mu to the camera's up) to
// the sphere at altitude h. Numerically stable for planet-sized radii.
// Returns (near, far); far < near (= -1, -2) when there's no hit.
fn ray_shell(h_c: f32, mu: f32, h: f32) -> vec2<f32> {
    let R = cl.planet.x;
    let r_c = R + h_c;
    let dr2 = (h - h_c) * (2.0 * R + h + h_c);
    let b = r_c * mu;
    let disc = b * b + dr2;
    if disc < 0.0 {
        return vec2<f32>(-1.0, -2.0);
    }
    let s = sqrt(disc);
    if b >= 0.0 {
        return vec2<f32>(-(b + s), dr2 / max(b + s, 1e-3));
    }
    return vec2<f32>(-dr2 / max(s - b, 1e-3), s - b);
}

// Altitude along the ray, stable for small t.
fn altitude(h_c: f32, mu: f32, t: f32) -> f32 {
    let R = cl.planet.x;
    let r_c = R + h_c;
    let num = t * t + 2.0 * t * r_c * mu;
    let r = sqrt(max(r_c * r_c + num, 0.0));
    return h_c + num / (r + r_c);
}

// Chapman-ish relative air mass for a layer with scale height H, from
// altitude h toward direction cosine mu (vs. local up). Handles a sun below
// the local horizon (grazing path) and the Earth's shadow.
fn air_column(h: f32, mu: f32, H: f32) -> f32 {
    let R = cl.planet.x;
    let C = sqrt(PI * R / (2.0 * H));
    if mu >= 0.0 {
        return exp(-h / H) / (mu + 1.0 / C);
    }
    let ht = (R + h) * sqrt(max(1.0 - mu * mu, 0.0)) - R;
    return 2.0 * C * exp(-ht / H) - exp(-h / H) / (-mu + 1.0 / C);
}

fn sun_transmittance(h: f32, mu_s: f32) -> vec3<f32> {
    let R = cl.planet.x;
    // Earth's shadow: the ray toward the sun hits the ground.
    if mu_s < 0.0 {
        let ht = (R + h) * sqrt(max(1.0 - mu_s * mu_s, 0.0)) - R;
        if ht < 0.0 {
            return vec3<f32>(0.0);
        }
    }
    let am_r = air_column(max(h, 0.0), mu_s, 8000.0);
    let am_m = air_column(max(h, 0.0), mu_s, 1500.0);
    let am_o = air_column(max(h, 0.0), mu_s, 8000.0);
    return exp(-(TAU_RAYLEIGH * am_r + vec3<f32>(TAU_MIE * am_m) + TAU_OZONE * am_o));
}

struct Sample {
    density: f32,
    // 0 at the cloud base, 1 at the top (for ambient gradient).
    height_frac: f32,
};

// Large-scale coverage at a noise-space point projected to the camera's
// altitude plane (so it doesn't change with height).
fn weather_at(pw: vec3<f32>) -> f32 {
    let a = textureSampleLevel(shape_tex, shape_smp, pw / WEATHER_PERIOD, 0.0);
    let b = textureSampleLevel(shape_tex, shape_smp, pw / (WEATHER_PERIOD * 0.25) + vec3<f32>(0.37, 0.11, 0.73), 0.0);
    return a.g * 0.6 + b.r * 0.4;
}

// Storm cell influence at render-space point p, altitude h.
// Returns (influence, height fraction in the tower, anvil weight, signed
// distance to the edge in units of the core radius: < 0 inside).
fn storms_at(p: vec3<f32>, h: f32) -> vec4<f32> {
    var best = vec4<f32>(0.0, 0.0, 0.0, 9.0);
    let n = i32(cl.misc.y);
    let base = cl.cu.x;
    let wind = cl.wind.xyz;
    for (var i = 0; i < n; i++) {
        let c = cl.storm_pos[i];
        let u = cl.storm_up[i];
        let top = u.w;
        if h > top || h < 0.0 {
            continue;
        }
        let fh = saturate((h - base) / (top - base));
        var d = p - c.xyz;
        d = d - u.xyz * dot(d, u.xyz);
        // The tower leans downwind with height and narrows a little.
        let dt = d - wind * (fh * fh * c.w * 0.5);
        let r_tower = c.w * (1.0 - 0.25 * smoothstep(0.1, 0.7, fh));
        let edge_t = length(dt) / r_tower - 1.0;
        // The anvil: a wedge that flares out under the tropopause, blown far
        // downwind and only a little upwind. Union with the tower.
        let flare = smoothstep(0.8, 0.95, fh);
        let along = dot(d, wind) - flare * c.w * 2.4;
        let cross = d - wind * dot(d, wind);
        let r_along = c.w * mix(0.6, select(1.4, 4.2, along > 0.0), flare);
        let r_cross = c.w * mix(0.6, 1.7, flare);
        let e_a = sqrt((along * along) / (r_along * r_along) + dot(cross, cross) / (r_cross * r_cross));
        let edge_a = select(9.0, e_a - 1.0, fh > 0.8);
        let edge = min(edge_t, edge_a);
        if edge < best.w {
            let anvil = saturate(0.5 + (edge_t - edge_a) * 2.0) * flare;
            best = vec4<f32>(smoothstep(0.0, -0.45, edge), fh, anvil, edge);
        }
    }
    return best;
}

// Cloud density. `p` render space, `pn` noise space, `pw` noise space at
// the camera's altitude plane, `h` altitude, `t` distance from camera.
fn density(p: vec3<f32>, pn: vec3<f32>, pw: vec3<f32>, h: f32, t: f32, detail: bool) -> Sample {
    var s: Sample;
    s.density = 0.0;
    s.height_frac = 0.0;
    let cov = cl.cu.z;
    let wx = weather_at(pw);
    // Clumps of cells, holes between; overcast stays covered.
    let local_cov = saturate(cov + (wx - 0.5) * (0.7 - 0.4 * cov));
    let storm = storms_at(p, h);

    let sh = textureSampleLevel(shape_tex, shape_smp, pn / SHAPE_PERIOD, 0.0);
    let wfbm = sh.g * 0.625 + sh.b * 0.25 + sh.a * 0.125;
    let base_shape = saturate(remap(sh.r, wfbm - 1.0, 1.0, 0.0, 1.0));

    // --- Cumulus: flat base, domed top; bigger cells reach higher ---
    var d = 0.0;
    var hf = 0.0;
    let cu_base = cl.cu.x;
    let cu_top = cu_base + (cl.cu.y - cu_base) * (0.35 + 0.65 * saturate(local_cov * 1.6));
    let f = (h - cu_base) / max(cu_top - cu_base, 1.0);
    if f > 0.0 && f < 1.0 && local_cov > 0.0 {
        let grad = saturate(f / 0.07) * saturate((1.0 - f) / 0.55);
        // Coverage shrinks toward the top: towers are narrower than their base.
        let cov_h = local_cov * mix(1.0, 0.55, f * f);
        let v = saturate(remap(base_shape * grad, 1.0 - cov_h, 1.0, 0.0, 1.0)) * cov_h;
        if v > d {
            d = v;
            hf = f;
        }
    }

    // --- Stratus / stratocumulus deck ---
    let amount = cl.st.z;
    if amount > 0.0 && !g_shadow_pass {
        let fs = (h - cl.st.x) / max(cl.st.y - cl.st.x, 1.0);
        if fs > 0.0 && fs < 1.0 {
            // Lumpy top, flatter base, broad rolls from the low-frequency shape.
            let roll = textureSampleLevel(shape_tex, shape_smp, pn / (SHAPE_PERIOD * 2.0) + vec3<f32>(0.5, 0.2, 0.9), 0.0);
            let lump = roll.r * 0.6 + base_shape * 0.4;
            let prof = saturate(fs / 0.12) * saturate((1.0 - fs) / (0.25 + 0.5 * lump));
            let st_cov = saturate(amount * (0.6 + 0.6 * wx));
            let v = saturate(remap(lump * prof, 1.0 - st_cov, 1.0, 0.0, 1.0)) * (0.25 + 0.75 * lump) * (0.4 + 0.6 * st_cov);
            if v > d {
                d = v;
                hf = fs;
            }
        }
    }

    // --- Cumulonimbus ---
    if storm.w < 0.75 && h > cu_base - 150.0 {
        let fh = storm.y;
        // Cauliflower: kilometer-scale turrets push the edge in and out.
        let b1 = textureSampleLevel(shape_tex, shape_smp, pn / SHAPE_PERIOD + vec3<f32>(0.3, 0.6, 0.1), 0.0);
        let b2 = textureSampleLevel(shape_tex, shape_smp, pn / (SHAPE_PERIOD * 0.5), 0.0);
        let billow = b1.g * 0.55 + b2.r * 0.45;
        let tower_edge = storm.w + (0.5 - billow) * 1.3;
        // Anvil edges are ragged, its underside lumpy.
        let anvil_edge = storm.w + (0.5 - base_shape) * 0.8 + (0.5 - billow) * 0.5;
        let e = mix(tower_edge, anvil_edge, storm.z);
        let solid = smoothstep(0.05, -0.2, e);
        // Flat base, a sharp flat top at the tropopause.
        let cap = 1.0 - smoothstep(0.95, 1.0, fh);
        let foot = saturate((h - cu_base) / 120.0);
        // The anvil is a thinner, more translucent sheet than the tower.
        let v = solid * cap * foot * mix(1.4, 0.12 * smoothstep(0.1, -0.5, e) + 0.14, storm.z);
        if v > d {
            d = v;
            hf = fh;
        }
    }

    // Light march: average erosion instead of sampling detail.
    if !detail && d > 0.0 {
        d = saturate(remap(d, 0.14, 1.0, 0.0, 1.0));
    }
    // --- Detail erosion: wispy at the base, billowy at the top ---
    if detail && d > 0.0 {
        let fade = saturate(1.0 - t / 40000.0);
        let dn = textureSampleLevel(detail_tex, detail_smp, pn / DETAIL_PERIOD, 0.0);
        let dfbm = dn.r * 0.625 + dn.g * 0.25 + dn.b * 0.125;
        let dmod = mix(dfbm, 1.0 - dfbm, saturate(hf * 4.0));
        let strength = mix(0.42, 0.15, saturate(storm.x * 2.0) * (1.0 - storm.z));
        d = saturate(remap(d, dmod * strength * fade, 1.0, 0.0, 1.0));
    }
    s.density = d * cl.cu.w;
    s.height_frac = hf;

    return s;
}

// Optical depth from a point toward the sun (cone of growing steps).
fn light_depth(p: vec3<f32>, pn: vec3<f32>, h: f32, t: f32) -> f32 {
    let steps = i32(cl.misc.w);
    let sun = cl.sun.xyz;
    let sun_n = cl.noise_from_render * sun;
    let up_n = cl.noise_from_render * cl.cam_up.xyz;
    let mu_s = dot(sun, cl.cam_up.xyz);
    var od = 0.0;
    var dist = 0.0;
    // Geometric steps covering ~3.5 km toward the sun.
    var step = 3500.0 * 1.2 / (pow(2.2, f32(steps)) - 1.0);
    for (var i = 0; i < steps; i++) {
        let mid = dist + step * 0.5;
        let hp = h + mu_s * mid;
        if hp > cl.shell.y {
            break;
        }
        let pp = p + sun * mid;
        let pnp = pn + sun_n * mid;
        let pwp = pnp - up_n * (hp - cl.cam_up.w);
        od += density(pp, pnp, pwp, hp, t + mid, false).density * step;
        dist += step;
        step *= 2.2;
    }
    return od * SIGMA;
}

fn ign(xy: vec2<f32>) -> f32 {
    return fract(52.9829189 * fract(dot(xy, vec2<f32>(0.06711056, 0.00583715))));
}

// Rain shafts: an analytic column under each storm cell, from the ground to
// the cloud base, with streaks that slant downwind. Returns (rgb radiance
// pre-exposure, transmittance) for the part of the ray in [t_a, t_b].
fn rain_shafts(ro: vec3<f32>, rd: vec3<f32>, h_c: f32, mu: f32, t_a: f32, t_b: f32, jitter: f32) -> vec4<f32> {
    let precip = cl.misc.x;
    let n = i32(cl.misc.y);
    var col = vec3<f32>(0.0);
    var T = 1.0;
    if precip <= 0.0 || n == 0 {
        return vec4<f32>(col, T);
    }
    let base = cl.cu.x;
    let wind = cl.wind.xyz;
    let wind_n = cl.noise_from_render * wind;
    let up_n = cl.noise_from_render * cl.cam_up.xyz;
    // Rain is lit by the grey light under the storm.
    let lum = (cl.sky_ambient.rgb * 0.55 + cl.ground_ambient.rgb * 0.45) / PI;
    for (var i = 0; i < n; i++) {
        let c = cl.storm_pos[i];
        let u = cl.storm_up[i].xyz;
        let rr = c.w * 0.85;
        // Ray vs. vertical cylinder (in the plane perpendicular to up).
        let o = ro - (c.xyz + wind * c.w * 0.35);
        let op = o - u * dot(o, u);
        let dp = rd - u * dot(rd, u);
        let A = dot(dp, dp);
        let B = 2.0 * dot(op, dp);
        let C = dot(op, op) - rr * rr;
        let disc = B * B - 4.0 * A * C;
        if disc <= 0.0 || A < 1e-6 {
            continue;
        }
        let sq = sqrt(disc);
        var ta = max((-B - sq) / (2.0 * A), t_a);
        var tb = min((-B + sq) / (2.0 * A), t_b);
        if tb <= ta {
            continue;
        }
        let len = tb - ta;
        var od = 0.0;
        for (var k = 0; k < 6; k++) {
            let ts = ta + len * (f32(k) + jitter) / 6.0;
            let h = altitude(h_c, mu, ts);
            if h > base || h < -50.0 {
                continue;
            }
            let p = ro + rd * ts;
            let q = p - (c.xyz + wind * c.w * 0.35);
            let qp = q - u * dot(q, u);
            let radial = smoothstep(1.0, 0.25, length(qp) / rr);
            // Streak noise in the horizontal plane, slanted with the wind.
            let pn = cl.cam_noise.xyz + cl.noise_from_render * (rd * ts);
            let pw = pn - up_n * (h - h_c) + wind_n * (base - h) * 0.35;
            let sn = textureSampleLevel(detail_tex, detail_smp, pw / (DETAIL_PERIOD * 2.0), 0.0);
            let curtain = textureSampleLevel(shape_tex, shape_smp, pw / (SHAPE_PERIOD * 0.5), 0.0).r;
            let streak = saturate(sn.g * 1.5 - 0.35) * saturate(curtain * 1.8 - 0.2);
            // Fade in under the base, thin out just above the ground.
            let vert = smoothstep(base, base * 0.7, h) * (0.55 + 0.45 * saturate(h / 300.0));
            od += radial * streak * vert * (len / 6.0) * 0.0009 * precip;
        }
        let Ts = exp(-od);
        col += T * lum * (1.0 - Ts);
        T *= Ts;
    }
    return vec4<f32>(col, T);
}

@if(!PREPASS_PIPELINE)
@fragment
fn fragment(in: VOut) -> @location(0) vec4<f32> {
    let frag = in.pos;
    let view = vb::view;
    let uv = (frag.xy - view.viewport.xy) / view.viewport.zw;
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let near_w = view.world_from_clip * vec4<f32>(ndc, 1.0, 1.0);
    let ro = view.world_position;
    let rd = normalize(near_w.xyz / near_w.w - ro);

    // Opaque scene distance along this ray (terrain, buildings, ocean).
    var t_scene = 1e30;
    @if(DEPTH_PREPASS && !MULTISAMPLED) {
        let d = textureLoad(vb::depth_prepass_texture, vec2<i32>(frag.xy), 0);
        if d > 0.0 {
            let view_z = view.clip_from_view[3][2] / d;
            let fwd = normalize(-view.world_from_view[2].xyz);
            t_scene = view_z / max(dot(rd, fwd), 1e-4);
        }
    }

    let up = cl.cam_up.xyz;
    let h_c = cl.cam_up.w;
    let mu = dot(rd, up);
    let h_lo = cl.shell.x;
    let h_hi = cl.shell.y;
    let lo = ray_shell(h_c, mu, h_lo);
    let hi = ray_shell(h_c, mu, h_hi);
    let ground = ray_shell(h_c, mu, 0.0);
    let hits_ground = mu < 0.0 && ground.y >= ground.x && ground.x > 0.0;
    let t_max = min(t_scene, cl.planet.y);
    let jitter = fract(ign(frag.xy) + f32(view.frame_count % 64u) * 0.618034);

    var t0 = 0.0;
    var t1 = 0.0;
    if h_c < h_lo {
        // Below the clouds: leave the lower sphere, then the upper one.
        if !hits_ground {
            t0 = lo.y;
            t1 = hi.y;
        }
    } else if h_c <= h_hi {
        // Inside the layer.
        t0 = 0.0;
        if lo.y >= lo.x && lo.x > 0.0 {
            t1 = lo.x;
        } else {
            t1 = hi.y;
        }
    } else if hi.y >= hi.x && hi.y > 0.0 {
        // Above: enter the top, stop at the bottom (or leave the top).
        t0 = max(hi.x, 0.0);
        if lo.y >= lo.x && lo.x > 0.0 {
            t1 = lo.x;
        } else {
            t1 = hi.y;
        }
    }
    t1 = min(t1, t_max);

    // Rain under the storms: in front of the clouds when we're below them.
    let rain_end = select(min(t_max, 60000.0), min(t0, t_max), h_c < h_lo && !hits_ground);
    var rain = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    if h_c < cl.cu.x + 500.0 || mu < 0.0 {
        rain = rain_shafts(ro, rd, h_c, mu, 0.0, rain_end, jitter);
    }

    let steps = i32(cl.misc.z);
    let step_scale = cl.shell.w;
    let sun = cl.sun.xyz;
    let cos_v = dot(rd, sun);
    let rd_n = cl.noise_from_render * rd;
    let up_n = cl.noise_from_render * up;
    let darkness = cl.st.w;
    // Silver-lining phase for each multiple-scattering octave.
    let ph0 = phase(cos_v, 1.0);
    let ph1 = phase(cos_v, 0.5);
    let ph2 = phase(cos_v, 0.25);
    let ph3 = 1.0 / (4.0 * PI);

    var T = 1.0;
    var col = vec3<f32>(0.0);
    var t_weight = 0.0;
    var t = t0;
    var fine = 0;
    var i = 0;
    loop {
        if i >= steps * 2 || t >= t1 || T < 0.015 {
            break;
        }
        i++;
        // Steps grow with distance; empty air is crossed 3x faster.
        let base_dt = (25.0 + t * 0.011) * step_scale;
        var dt = base_dt;
        if fine == 0 {
            dt = base_dt * 3.0;
        }
        let ts = t + dt * jitter;
        let h = altitude(h_c, mu, ts);
        let p = ro + rd * ts;
        let pn = cl.cam_noise.xyz + rd_n * ts;
        let pw = pn - up_n * (h - h_c);
        let s = density(p, pn, pw, h, ts, true);
        if s.density <= 0.001 {
            if fine > 0 {
                fine--;
            }
            t += dt;
            continue;
        }
        if fine == 0 {
            // Entered a cloud on a coarse step: back up and refine.
            fine = 5;
            t = max(t0, t - base_dt);
            continue;
        }
        fine = 5;
        let sigma_e = s.density * SIGMA;

        // Sun through the atmosphere (warm and dim when low, gone in the
        // Earth's shadow), then multiple-scattering octaves through the cloud.
        let mu_s = dot(sun, up);
        // Keep a sunset warm, but white-balance the midday sun toward white.
        let sun_t = sun_transmittance(h, mu_s);
        let sun_col = cl.sun.w * mix(sun_t, vec3<f32>(dot(sun_t, vec3<f32>(0.2126, 0.7152, 0.0722))), cl.planet.z);
        let od = light_depth(p, pn, h, ts);
        let ms = ph0 * exp(-od) + 0.8 * ph1 * exp(-od * 0.35) + 0.6 * ph2 * exp(-od * 0.15) + 0.5 * ph3 * exp(-od * 0.08);
        // Powder: thin edges facing away from the sun are darker.
        let powder = mix(1.0, 1.0 - exp(-sigma_e * 120.0 - od * 0.8), 0.5 * saturate(0.6 - cos_v * 0.6));
        var direct = sun_col * ms * powder * 2.0;

        // Ambient: sky from above (dimmer through thick cloud), ground bounce
        // from below; rain-laden bellies go darker.
        let hfrac = s.height_frac;
        let sky_vis = 0.25 + 0.75 * exp(-od * 0.1);
        let amb_mix = saturate(hfrac * 1.1 + 0.35);
        var amb = mix(cl.ground_ambient.rgb, cl.sky_ambient.rgb * sky_vis, amb_mix) / PI;
        let belly = 1.0 - darkness * 0.8 * (1.0 - saturate(hfrac * 1.4));
        amb *= belly;
        direct *= mix(1.0, belly, 0.5);
        let S = (direct + amb) * sigma_e * cl.planet.w;
        let Tstep = exp(-sigma_e * dt);
        col += T * (S - S * Tstep) / max(sigma_e, 1e-7);
        t_weight += ts * T * (1.0 - Tstep);
        T *= Tstep;
        t += dt;
    }

    var alpha = 1.0 - T;
    if alpha > 0.0 {
        // Aerial perspective: distant clouds melt into the horizon haze.
        let t_mean = t_weight / max(alpha, 1e-5);
        let haze_dist = cl.shell.z * exp(h_c / 8000.0);
        let air = exp(-t_mean / haze_dist);
        let glow = 1.0 + cl.haze.w * pow(max(cos_v, 0.0), 6.0);
        let haze = cl.haze.rgb * glow * mix(1.0, 0.65, saturate(mu * 3.0));
        col = col * air + haze * (1.0 - air) * alpha;
    }
    // Composite rain (front or behind) with the clouds.
    if h_c < h_lo && !hits_ground {
        col = rain.rgb + rain.a * col;
    } else {
        col = col + T * rain.rgb;
    }
    alpha = 1.0 - T * rain.a;
    if alpha <= 0.0 {
        return vec4<f32>(0.0);
    }
    return vec4<f32>(col * view.exposure, alpha);
}

// ---------------------------------------------------------------------------
// Cloud shadows. A horizontal caster plane follows the camera on a render
// layer only the sun sees; this alpha-mask prepass shader keeps the texels
// whose column of cloud toward the sun is optically thick, so cumulus throw
// patchy shadows into the cascaded shadow map.
// ---------------------------------------------------------------------------

fn shadow_discard(p: vec3<f32>) -> bool {
    g_shadow_pass = true;
    // Under a solid deck the sun is dimmed globally instead (see sun_fraction).
    if cl.st.z > 0.5 {
        return true;
    }
    let up = cl.cam_up.xyz;
    let sun = cl.sun.xyz;
    let mu_s = dot(sun, up);
    if mu_s < 0.05 {
        return true;
    }
    let rel = p - cl.cam_pos.xyz;
    let h_plane = cl.cam_up.w + dot(rel, up);
    let lo = cl.shell.x + 150.0;
    let hi = min(cl.shell.y, lo + 4000.0);
    let up_n = cl.noise_from_render * up;
    let n = 10;
    let dh = (hi - lo) / f32(n);
    var od = 0.0;
    for (var k = 0; k < n; k++) {
        let hk = lo + dh * (f32(k) + 0.5);
        let q = p + sun * ((hk - h_plane) / mu_s);
        let qn = cl.cam_noise.xyz + cl.noise_from_render * (q - cl.cam_pos.xyz);
        let qw = qn - up_n * (hk - cl.cam_up.w);
        od += density(q, qn, qw, hk, 0.0, true).density * dh / mu_s;
    }
    return od * SIGMA < 2.0;
}

@if(PREPASS_PIPELINE && PREPASS_FRAGMENT)
@fragment
fn fragment(in: pio::VertexOutput) -> pio::FragmentOutput {
    if shadow_discard(in.world_position.xyz) {
        discard;
    }
    var out: pio::FragmentOutput;
    @if(NORMAL_PREPASS)
    out.normal = vec4<f32>(0.5, 1.0, 0.5, 1.0);
    @if(MOTION_VECTOR_PREPASS)
    out.motion_vector = vec2<f32>(0.0);
    @if(UNCLIPPED_DEPTH_ORTHO_EMULATION)
    out.frag_depth = in.unclipped_depth;
    return out;
}

@if(PREPASS_PIPELINE && !PREPASS_FRAGMENT)
@fragment
fn fragment(in: pio::VertexOutput) {
    if shadow_discard(in.world_position.xyz) {
        discard;
    }
}
