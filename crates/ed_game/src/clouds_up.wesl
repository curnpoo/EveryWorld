// Composites the half-resolution cloud march (`cloud_half.rs`) onto the HDR
// view target with a depth-aware upsample: each full-res pixel blends the 4
// nearest half-res texels, bilinear weights scaled down where a texel marched
// against a different depth (mountain edge vs sky), so clouds don't bleed
// across silhouettes. Output is premultiplied (colour, 1 - transmittance).

import bevy_render::view::View;

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var depth: texture_depth_2d;
@group(0) @binding(2) var half_col: texture_2d<f32>;
@group(0) @binding(3) var half_depth: texture_2d<f32>;

// View-space distance from reverse-Z depth; the sky is "very far".
fn lin(d: f32) -> f32 {
    return select(1e8, view.clip_from_view[3][2] / max(d, 1e-12), d > 0.0);
}

@fragment
fn fragment(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let zf = lin(textureLoad(depth, vec2<i32>(pos.xy), 0));
    let lim = vec2<i32>(textureDimensions(half_col)) - 1;
    let hp = pos.xy * 0.5 - 0.5;
    let b = vec2<i32>(floor(hp));
    let f = hp - floor(hp);
    var acc = vec4<f32>(0.0);
    var wsum = 0.0;
    for (var j = 0; j < 4; j++) {
        let o = vec2<i32>(j & 1, j >> 1);
        let q = clamp(b + o, vec2<i32>(0), lim);
        let wx = select(1.0 - f.x, f.x, o.x == 1);
        let wy = select(1.0 - f.y, f.y, o.y == 1);
        let zi = lin(textureLoad(half_depth, q, 0).r);
        let rel = abs(zf - zi) / max(min(zf, zi), 1.0);
        let w = (wx * wy + 1e-3) / (0.02 + rel);
        acc += textureLoad(half_col, q, 0) * w;
        wsum += w;
    }
    return acc / max(wsum, 1e-6);
}
