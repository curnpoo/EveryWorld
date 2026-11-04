// FSR 1 EASU (Edge Adaptive Spatial Upsampling), a port of AMD's ffx_fsr1.h
// FsrEasuF. It runs as a full-screen UI material at native resolution and
// reads the low-res 3D render with 12 texel loads around the sample point:
//
//        b c
//      e f g h
//      i j k l
//        n o
//
// Edge direction and length come from luma gradients of the 2x2 bilinear
// quad (f g j k); the taps are weighted by a Lanczos-2-like kernel stretched
// along the edge, then clamped to the quad's min/max to stop ringing.
// Sharpening (RCAS's job in real FSR) is done by CAS on the 3D camera.

import bevy_ui_render::ui_vertex_output::UiVertexOutput;

@group(1) @binding(0) var src: texture_2d<f32>;

fn tex(p: vec2<i32>, dims: vec2<i32>) -> vec3<f32> {
    // The source is sRGB, loads come back linear. EASU is tuned for
    // perceptual space, so filter in (approx) gamma 2.
    return sqrt(max(textureLoad(src, clamp(p, vec2<i32>(0), dims - 1), 0).rgb, vec3<f32>(0.0)));
}

fn luma(c: vec3<f32>) -> f32 {
    return c.b * 0.5 + c.r * 0.5 + c.g;
}

struct DirLen {
    dir: vec2<f32>,
    len: f32,
}

fn easu_set(acc: DirLen, w: f32, la: f32, lb: f32, lc: f32, ld: f32, le: f32) -> DirLen {
    var o = acc;
    let dc = ld - lc;
    let cb = lc - lb;
    var len_x = max(abs(dc), abs(cb));
    len_x = 1.0 / max(len_x, 1e-5);
    let dir_x = ld - lb;
    o.dir.x += dir_x * w;
    len_x = clamp(abs(dir_x) * len_x, 0.0, 1.0);
    len_x *= len_x;
    o.len += len_x * w;
    let ec = le - lc;
    let ca = lc - la;
    var len_y = max(abs(ec), abs(ca));
    len_y = 1.0 / max(len_y, 1e-5);
    let dir_y = le - la;
    o.dir.y += dir_y * w;
    len_y = clamp(abs(dir_y) * len_y, 0.0, 1.0);
    len_y *= len_y;
    o.len += len_y * w;
    return o;
}

fn easu_tap(off: vec2<f32>, dir: vec2<f32>, len2: vec2<f32>, lob: f32, clp: f32) -> f32 {
    var v = vec2<f32>(off.x * dir.x + off.y * dir.y, off.x * -dir.y + off.y * dir.x);
    v *= len2;
    let d2 = min(v.x * v.x + v.y * v.y, clp);
    var wb = 2.0 / 5.0 * d2 - 1.0;
    var wa = lob * d2 - 1.0;
    wb *= wb;
    wa *= wa;
    wb = 25.0 / 16.0 * wb - (25.0 / 16.0 - 1.0);
    return wb * wa;
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let dims = vec2<i32>(textureDimensions(src));
    let pin = in.uv * vec2<f32>(dims) - 0.5;
    let fp = floor(pin);
    let pp = pin - fp;
    let f0 = vec2<i32>(fp);

    let b = tex(f0 + vec2(0, -1), dims);
    let c = tex(f0 + vec2(1, -1), dims);
    let e = tex(f0 + vec2(-1, 0), dims);
    let f = tex(f0 + vec2(0, 0), dims);
    let g = tex(f0 + vec2(1, 0), dims);
    let h = tex(f0 + vec2(2, 0), dims);
    let i = tex(f0 + vec2(-1, 1), dims);
    let j = tex(f0 + vec2(0, 1), dims);
    let k = tex(f0 + vec2(1, 1), dims);
    let l = tex(f0 + vec2(2, 1), dims);
    let n = tex(f0 + vec2(0, 2), dims);
    let o = tex(f0 + vec2(1, 2), dims);

    let bl = luma(b); let cl = luma(c); let el = luma(e); let fl = luma(f);
    let gl = luma(g); let hl = luma(h); let il = luma(i); let jl = luma(j);
    let kl = luma(k); let ll = luma(l); let nl = luma(n); let ol = luma(o);

    var dl = DirLen(vec2<f32>(0.0), 0.0);
    dl = easu_set(dl, (1.0 - pp.x) * (1.0 - pp.y), bl, el, fl, gl, jl);
    dl = easu_set(dl, pp.x * (1.0 - pp.y), cl, fl, gl, hl, kl);
    dl = easu_set(dl, (1.0 - pp.x) * pp.y, fl, il, jl, kl, nl);
    dl = easu_set(dl, pp.x * pp.y, gl, jl, kl, ll, ol);

    var dir = dl.dir;
    let dir2 = dir * dir;
    var dir_r = dir2.x + dir2.y;
    let zro = dir_r < 1.0 / 32768.0;
    dir_r = select(inverseSqrt(dir_r), 1.0, zro);
    dir.x = select(dir.x, 1.0, zro);
    dir *= dir_r;
    var len = dl.len * 0.5;
    len *= len;
    let stretch = (dir.x * dir.x + dir.y * dir.y) / max(max(abs(dir.x), abs(dir.y)), 1e-5);
    let len2 = vec2<f32>(1.0 + (stretch - 1.0) * len, 1.0 - 0.5 * len);
    let lob = 0.5 + ((1.0 / 4.0 - 0.04) - 0.5) * len;
    let clp = 1.0 / lob;

    var ac = vec3<f32>(0.0);
    var aw = 0.0;
    var w: f32;
    w = easu_tap(vec2(0.0, -1.0) - pp, dir, len2, lob, clp); ac += b * w; aw += w;
    w = easu_tap(vec2(1.0, -1.0) - pp, dir, len2, lob, clp); ac += c * w; aw += w;
    w = easu_tap(vec2(-1.0, 1.0) - pp, dir, len2, lob, clp); ac += i * w; aw += w;
    w = easu_tap(vec2(0.0, 1.0) - pp, dir, len2, lob, clp); ac += j * w; aw += w;
    w = easu_tap(vec2(0.0, 0.0) - pp, dir, len2, lob, clp); ac += f * w; aw += w;
    w = easu_tap(vec2(-1.0, 0.0) - pp, dir, len2, lob, clp); ac += e * w; aw += w;
    w = easu_tap(vec2(1.0, 1.0) - pp, dir, len2, lob, clp); ac += k * w; aw += w;
    w = easu_tap(vec2(2.0, 1.0) - pp, dir, len2, lob, clp); ac += l * w; aw += w;
    w = easu_tap(vec2(2.0, 0.0) - pp, dir, len2, lob, clp); ac += h * w; aw += w;
    w = easu_tap(vec2(1.0, 0.0) - pp, dir, len2, lob, clp); ac += g * w; aw += w;
    w = easu_tap(vec2(1.0, 2.0) - pp, dir, len2, lob, clp); ac += o * w; aw += w;
    w = easu_tap(vec2(0.0, 2.0) - pp, dir, len2, lob, clp); ac += n * w; aw += w;

    let mn = min(min(f, g), min(j, k));
    let mx = max(max(f, g), max(j, k));
    let pix = clamp(ac / max(aw, 1e-5), mn, mx);
    return vec4<f32>(pix * pix, 1.0);
}
