//! Erosion detail (P-slice S7 b, v2): relief finer than the elevation source, added to every
//! page sample as a pure function of where the sample is and what the source landform does
//! around it.
//!
//! The source knows the landform but not the gullies, rills, ledges and lumps that make a slope
//! read as eroded rock or a soil-mantled hill. This adds them everywhere on Earth with no
//! place-specific rule, as a fractal over a wide band of scales whose character follows the
//! landform:
//!
//! * **Gullies** (an analytic erosion filter after runevision's / Clay John's "eroded noise"):
//!   [`GULLY_N`] octaves from [`TOP_M`] (just under what the source resolves) down to ~3.5 m,
//!   at an irrational ratio, each on its own hashed rotation and offset, so no grid or period
//!   shows. Per octave, jittered lattice points carry cosine stripes whose crests run down the
//!   fall line. Amplitude follows a power law `(lambda / TOP_M)^H`; `H` is steeper on soft
//!   ground (soil creep rounds small relief) and shallower on rock. Only the big octaves
//!   (over ~25 m) are domain-warped; small ones stay straight.
//! * **Drainage**: every octave is steered by the source gradient smoothed over ~50-170 m
//!   (the large-scale fall line), never the local wiggly slope, so small rills run straight
//!   downhill instead of curling around bumps. The fine gradient and the branching from the
//!   coarser synthetic octaves only come in where the smoothed slope flattens out. Topographic
//!   position (the source against its smoothed self, a proxy for contributing area) deepens
//!   gullies in hollows and softens them on spurs.
//! * **Character** varies at km scale (value noise) and with the landform from the source:
//!   relief over ~300 m and the smoothed slope select rock (sharp V gullies, ledges) or soft
//!   hills (rounded, filled drainage); near-flat floors keep only gentle swales.
//! * **Ledges**: on rock-steep ground the height is pulled toward strata at two spacings that
//!   vary regionally, on folded (not level) bedding.
//! * **Lumps and micro-relief**: smooth value noise on gentle ground and sub-metre texture.
//!
//! Octaves a page's lattice cannot carry (wavelength under ~3 samples of the level's nominal
//! spacing) fade out, so coarse levels stay smooth and a node morphs into its children's detail.
//!
//! Determinism: f64 throughout, `libm` transcendentals, an integer hash of lattice cells; inputs
//! are the lattice point (bit-identical between neighbours) and the [`Landform`] (computed from
//! the same fixed source neighbourhood by every node that shares the point), and the level. So
//! shared borders stay bit-identical (G4).

use std::f64::consts::PI;
use std::sync::OnceLock;

/// Wavelength of the largest gully octave (m): around the source's own resolution limit.
const TOP_M: f64 = 100.0;
/// Ratio between octaves (irrational, so octave lattices never line up).
const RATIO: f64 = 2.312_741_9;
/// Gully octaves (100 m down to ~3.5 m) and micro octaves (1.6, 0.7 m) after them.
const GULLY_N: usize = 5;
const MICRO_N: usize = 2;
/// Depth (m) of the top gully octave at full strength on rock.
const TOP_DEPTH_M: f64 = 4.0;
/// Spectral exponent: amplitude ~ wavelength^H.
const H_BASE: f64 = 0.72;
/// Branching: how much of an octave's own slope steers the next octave's flow direction.
const BRANCH: f64 = 0.8;
/// Ledge (strata) spacing range (m) and pull strength on rock-steep ground.
const LEDGE_MIN_M: f64 = 1.8;
const LEDGE_SPAN_M: f64 = 5.0;
const LEDGE_PULL: f64 = 0.3;
/// Second, wider strata set: spacing multiple and pull.
const LEDGE2_MUL: f64 = 4.3;
const LEDGE2_PULL: f64 = 0.12;
/// Lumps on gentle ground: amplitude (m) at 10 m wavelength, on the ~8 m and ~3.5 m octaves.
const LUMP_10M: f64 = 0.09;
/// Nominal page sample spacing at level 0 (m): a quarter meridian over 256 cells. A pure
/// function of the level (never of the node's real, warped spacing), so neighbours agree.
const SPACING_L0_M: f64 = PI * 0.5 * 6_371_000.0 / 256.0;

/// What the source says about the ground around a sample (from `node_heights`).
#[derive(Clone, Copy, Debug)]
pub struct Landform {
    /// Fine source height (m) and its gradient (m per m, toward east and north).
    pub h: f64,
    pub grad: [f64; 2],
    /// Source gradient smoothed over ~50 m and ~170 m.
    pub grad_m: [f64; 2],
    pub grad_l: [f64; 2],
    /// Topographic position: fine minus ~50 m smoothed height, and ~50 m minus ~170 m smoothed
    /// (m; negative in hollows and valleys, positive on spurs and ridges).
    pub tpi_m: f64,
    pub tpi_l: f64,
    /// Relief: standard deviation of the source over ~300 m (m).
    pub relief: f64,
}

impl Landform {
    /// A uniform slope (tests, callers without the smoothed fields): every scale has `grad`,
    /// no hollows or spurs, relief from the slope over ~300 m.
    pub fn uniform(h: f64, grad: [f64; 2]) -> Landform {
        let s = (grad[0] * grad[0] + grad[1] * grad[1]).sqrt();
        Landform { h, grad, grad_m: grad, grad_l: grad, tpi_m: 0.0, tpi_l: 0.0, relief: 60.0 * s }
    }
}

/// Nominal page sample spacing (m) of node level `level`.
pub fn nominal_spacing(level: u8) -> f64 {
    SPACING_L0_M / (1u64 << level.min(40)) as f64
}

/// First level that receives any detail (the coarsest octave fits from here).
pub fn first_detail_level() -> u8 {
    (0..40u8).find(|&l| octave_band(TOP_M, l) > 0.0).unwrap_or(40).max(FIRST_LEVEL)
}

/// Detail starts here (2.4 km pages, z14 source) at half strength, full from the next level:
/// coarser pages sample a source too coarse for the landform fields to mean much.
const FIRST_LEVEL: u8 = 12;
#[inline]
fn level_fade(level: u8) -> f64 {
    if level < FIRST_LEVEL {
        0.0
    } else if level == FIRST_LEVEL {
        0.5
    } else {
        1.0
    }
}

#[inline]
fn smoothstep(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How much of an octave of wavelength `lambda` a level carries (0 below ~2.5 samples per
/// wavelength, 1 from 4).
#[inline]
fn octave_band(lambda: f64, level: u8) -> f64 {
    smoothstep(2.5, 4.0, lambda / nominal_spacing(level))
}

type V3 = [f64; 3];
#[inline]
fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
#[inline]
fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
#[inline]
fn add_s(a: V3, b: V3, s: f64) -> V3 {
    [a[0] + b[0] * s, a[1] + b[1] * s, a[2] + b[2] * s]
}
#[inline]
fn scale(a: V3, s: f64) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
#[inline]
fn horiz(e: V3, n: V3, g: [f64; 2]) -> V3 {
    add_s(scale(e, g[0]), n, g[1])
}

/// 64-bit integer hash of a lattice cell and a salt (splitmix finaliser).
#[inline]
fn hash3(x: i64, y: i64, z: i64, salt: u64) -> u64 {
    let mut h = (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ (z as u64).wrapping_mul(0x1656_67B1_9E37_79F9) ^ salt;
    h = (h ^ (h >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^ (h >> 31)
}

/// Three values in [-1, 1) from a hash.
#[inline]
fn unit3(h: u64) -> V3 {
    let f = |s: u32| ((h >> s) & 0x1F_FFFF) as f64 / (1u64 << 20) as f64 - 1.0;
    [f(0), f(21), f(42)]
}

/// Row-major 3x3 rotation.
type M3 = [f64; 9];
#[inline]
fn rot(m: &M3, v: V3) -> V3 {
    [m[0] * v[0] + m[1] * v[1] + m[2] * v[2], m[3] * v[0] + m[4] * v[1] + m[5] * v[2], m[6] * v[0] + m[7] * v[1] + m[8] * v[2]]
}
#[inline]
fn rot_t(m: &M3, v: V3) -> V3 {
    [m[0] * v[0] + m[3] * v[1] + m[6] * v[2], m[1] * v[0] + m[4] * v[1] + m[7] * v[2], m[2] * v[0] + m[5] * v[1] + m[8] * v[2]]
}

/// Per-octave rotations (hashed axis and angle) and lattice offsets, so octave lattices never
/// share axes or origins. Built once, deterministically (libm).
#[derive(Clone, Copy)]
struct Oct {
    m: M3,
    off: V3,
    lambda: f64,
    /// ln(lambda / TOP_M) and the lump amplitude at this wavelength.
    ln: f64,
    lump: f64,
}

fn octave_frames() -> &'static [Oct; GULLY_N + MICRO_N] {
    static F: OnceLock<[Oct; GULLY_N + MICRO_N]> = OnceLock::new();
    F.get_or_init(|| {
        let mut out = [Oct { m: [0.0; 9], off: [0.0; 3], lambda: 0.0, ln: 0.0, lump: 0.0 }; GULLY_N + MICRO_N];
        for (k, o) in out.iter_mut().enumerate() {
            o.lambda = TOP_M / libm::pow(RATIO, k as f64);
            o.ln = libm::log(o.lambda / TOP_M);
            o.lump = LUMP_10M * libm::pow(o.lambda / 10.0, 0.85);
            let a = unit3(hash3(k as i64, 7, 11, 0x0C7A_F0A5));
            let l = dot(a, a).sqrt().max(1e-6);
            let (x, y, z) = (a[0] / l, a[1] / l, a[2] / l);
            let ang = 0.7 + 2.399_963 * k as f64; // golden angle steps
            let (s, c) = libm::sincos(ang);
            let t = 1.0 - c;
            o.m = [t * x * x + c, t * x * y - s * z, t * x * z + s * y, t * x * y + s * z, t * y * y + c, t * y * z - s * x, t * x * z - s * y, t * y * z + s * x, t * z * z + c];
            let off = unit3(hash3(k as i64, 3, 5, 0x0FF5_E7));
            o.off = scale(off, 50.0);
        }
        out
    })
}

/// `(sin, cos)` of `2 pi x`: range-reduced Taylor polynomials (|error| < 1e-4), plain
/// arithmetic only, so bit-identical everywhere and cheaper than libm in the inner loop.
#[inline]
fn sincos_turns(x: f64) -> (f64, f64) {
    let t = x - x.round(); // [-0.5, 0.5]
    // Fold to [-0.25, 0.25]: cos flips sign, sin is symmetric about +-0.25.
    let (t, sgn) = if t > 0.25 { (0.5 - t, -1.0) } else if t < -0.25 { (-0.5 - t, -1.0) } else { (t, 1.0) };
    let a = 2.0 * PI * t;
    let a2 = a * a;
    let s = a * (1.0 - a2 / 6.0 * (1.0 - a2 / 20.0 * (1.0 - a2 / 42.0 * (1.0 - a2 / 72.0))));
    let c = 1.0 - a2 / 2.0 * (1.0 - a2 / 12.0 * (1.0 - a2 / 30.0 * (1.0 - a2 / 56.0 * (1.0 - a2 / 90.0))));
    (s, c * sgn)
}

/// One gully octave at `q` (lattice units): stripes whose crests run along the fall line, so
/// they vary along `across` (unit, lattice space). Returns the value in (-1, 1) and its
/// derivative with respect to `q`.
fn gully(q: V3, across: V3, salt: u64) -> (f64, V3) {
    const JITTER: f64 = 0.25;
    let c0 = [q[0].floor(), q[1].floor(), q[2].floor()];
    // Only cells whose jittered point can lie within distance 1 (centre +- 1.25 per axis).
    let range = |i: usize| {
        let f = q[i] - c0[i];
        (if f < 0.75 { -1i64 } else { 0 }, if f > 0.25 { 1i64 } else { 0 })
    };
    let (rx, ry, rz) = (range(0), range(1), range(2));
    let (mut sv, mut sw) = (0.0, 0.0);
    let mut sd = [0.0; 3];
    for dz in rz.0..=rz.1 {
        for dy in ry.0..=ry.1 {
            for dx in rx.0..=rx.1 {
                let c = [c0[0] + dx as f64, c0[1] + dy as f64, c0[2] + dz as f64];
                let j = unit3(hash3(c[0] as i64, c[1] as i64, c[2] as i64, salt));
                let pp = [q[0] - (c[0] + 0.5 + JITTER * j[0]), q[1] - (c[1] + 0.5 + JITTER * j[1]), q[2] - (c[2] + 0.5 + JITTER * j[2])];
                let d2 = dot(pp, pp);
                if d2 >= 1.0 {
                    continue;
                }
                let k = 1.0 - d2;
                let w = k * k * k;
                let (s, co) = sincos_turns(dot(pp, across));
                sv += w * co;
                sd = add_s(sd, across, -w * s * 2.0 * PI);
                sw += w;
            }
        }
    }
    let n = 1.0 / (sw + 0.02);
    (sv * n, scale(sd, n))
}

/// Smooth 3D value noise in [-1, 1] (quintic-faded trilinear of hashed lattice values).
fn value3(q: V3, salt: u64) -> f64 {
    let c = [q[0].floor(), q[1].floor(), q[2].floor()];
    let f = [q[0] - c[0], q[1] - c[1], q[2] - c[2]];
    let fade = |t: f64| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
    let u = [fade(f[0]), fade(f[1]), fade(f[2])];
    let v = |dx: i64, dy: i64, dz: i64| (hash3(c[0] as i64 + dx, c[1] as i64 + dy, c[2] as i64 + dz, salt) >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0;
    let lerp = |a: f64, b: f64, t: f64| a + (b - a) * t;
    let x00 = lerp(v(0, 0, 0), v(1, 0, 0), u[0]);
    let x10 = lerp(v(0, 1, 0), v(1, 1, 0), u[0]);
    let x01 = lerp(v(0, 0, 1), v(1, 0, 1), u[0]);
    let x11 = lerp(v(0, 1, 1), v(1, 1, 1), u[0]);
    lerp(lerp(x00, x10, u[1]), lerp(x01, x11, u[1]), u[2])
}

/// Four decorrelated smooth value-noise fields in [-1, 1] at wavelength `lambda` (m) from one
/// set of lattice hashes (16 bits each), for the cheap low-frequency fields.
fn vn4(p: V3, lambda: f64, salt: u64) -> [f64; 4] {
    let f = 1.0 / lambda;
    let q = [p[0] * f + 0.37, p[1] * f + 0.71, p[2] * f + 0.13];
    let c = [q[0].floor(), q[1].floor(), q[2].floor()];
    let fade = |t: f64| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
    let u = [fade(q[0] - c[0]), fade(q[1] - c[1]), fade(q[2] - c[2])];
    let mut out = [0.0; 4];
    for dz in 0..2i64 {
        for dy in 0..2i64 {
            for dx in 0..2i64 {
                let h = hash3(c[0] as i64 + dx, c[1] as i64 + dy, c[2] as i64 + dz, salt);
                let w = (if dx == 1 { u[0] } else { 1.0 - u[0] }) * (if dy == 1 { u[1] } else { 1.0 - u[1] }) * (if dz == 1 { u[2] } else { 1.0 - u[2] });
                for (i, o) in out.iter_mut().enumerate() {
                    *o += w * (((h >> (16 * i)) & 0xFFFF) as f64 / 32767.5 - 1.0);
                }
            }
        }
    }
    out
}

/// Value noise of `p` (m) at wavelength `lambda` (m), offset per salt so fields decorrelate.
#[inline]
fn vn(p: V3, lambda: f64, salt: u64) -> f64 {
    let f = 1.0 / lambda;
    let o = (salt & 0xFF) as f64 * 17.31;
    value3([p[0] * f + o, p[1] * f - o, p[2] * f + 0.5 * o], salt)
}

/// Local frame of a geodetic point: east, north, up (ECEF unit vectors).
pub(crate) fn enu(lat_rad: f64, lon_rad: f64) -> (V3, V3, V3) {
    let (sl, cl) = libm::sincos(lat_rad);
    let (so, co) = libm::sincos(lon_rad);
    ([-so, co, 0.0], [-sl * co, -sl * so, cl], [cl * co, cl * so, sl])
}

/// Wavelength (m) of gully/micro octave `k`.
#[inline]
fn lambda_of(k: usize) -> f64 {
    octave_frames()[k].lambda
}

/// Spectral exponent range (over the km-scale field and the landform).
const H_MIN: f64 = H_BASE - 0.1 - 0.1;
const H_MAX: f64 = H_BASE + 0.1 + 0.12;

/// Detail height (m) to add to a page sample.
///
/// * `p`: the lattice point on the ellipsoid (ECEF metres, `d * r(d)`).
/// * `lat_rad`, `lon_rad`: its geodetic latitude and longitude.
/// * `land`: the source landform there ([`Landform`]).
/// * `level`: the node level (selects the octaves the page can carry).
pub fn erosion_detail(p: V3, lat_rad: f64, lon_rad: f64, land: &Landform, level: u8) -> f64 {
    let fade = level_fade(level);
    if fade <= 0.0 {
        return 0.0;
    }
    let (e, n, up) = enu(lat_rad, lon_rad);
    let len2 = |g: [f64; 2]| (g[0] * g[0] + g[1] * g[1]).sqrt();
    let (s0, sm) = (len2(land.grad), len2(land.grad_m));

    // Regional character (km scale): spectral slope, overall gain, sharpness, strata.
    let [c1, c2, c3, c4] = vn4(p, 1900.0, 0xC1);
    // Landform: rock = steep at the ~50 m scale with high relief, or locally cliff-steep.
    let rocky = (smoothstep(0.5, 1.05, sm) * smoothstep(20.0, 70.0, land.relief)).max(smoothstep(0.95, 1.5, s0));
    let soft = 1.0 - rocky;
    let flat = 1.0 - smoothstep(0.03, 0.16, sm.max(s0 * 0.7));
    let h_exp = H_BASE + 0.1 * c1 + 0.12 * soft - 0.1 * rocky;
    let gain = (0.55 + 0.45 * rocky) * (1.0 + 0.3 * c2);
    let sharp = (rocky * 0.85 + 0.25 * c3).clamp(0.0, 1.0);
    // Topographic position, scaled by the local relief: hollows (negative) collect the water.
    let pos_m = land.tpi_m / (1.5 + 0.08 * land.relief);
    let pos_l = land.tpi_l / (3.0 + 0.2 * land.relief);
    let hollow = (0.6 * smoothstep(0.0, 1.0, -pos_m) + 0.4 * smoothstep(0.0, 1.0, -pos_l)).min(1.0);
    let spur = (0.6 * smoothstep(0.0, 1.0, pos_m) + 0.4 * smoothstep(0.0, 1.0, pos_l)).min(1.0);
    // Regional drift so flats still have a swale direction.
    let da = PI * (c1 * 0.7 + c4 * 0.7 + 1.0);
    let (dsn, dcs) = libm::sincos(da);
    let drift = horiz(e, n, [dcs * 0.02, dsn * 0.02]);

    // Domain warp, broad and weak, for the big octaves only: small gullies stay straight.
    let [w0, w1, w2, _] = vn4(p, 170.0, 0xE1);
    let wb = [w0, w1, w2];
    let frames = octave_frames();

    let (gf, gm, gl) = (horiz(e, n, land.grad), horiz(e, n, land.grad_m), horiz(e, n, land.grad_l));
    let mut dslope = [0.0; 3]; // accumulated synthetic slope (m per m, world)
    let mut out = 0.0;
    for (k, o) in frames.iter().enumerate().take(GULLY_N) {
        let lambda = o.lambda;
        let band = octave_band(lambda, level);
        if band <= 0.0 {
            continue;
        }
        // Flow direction: the smoothed fall line (~50 m for small octaves, ~170 m for large),
        // so small gullies run straight and parallel down uniform slopes. The fine slope only
        // steers where the ground flattens, where real channels meander.
        let b = smoothstep(35.0, 90.0, lambda);
        let gsm = add_s(scale(gm, 1.0 - b), gl, b);
        let s_sm = dot(gsm, gsm).sqrt();
        let loose = 1.0 - smoothstep(0.08, 0.3, s_sm);
        let fine_w = if lambda < 20.0 { 0.25 + 0.5 * loose } else { 0.0 };
        let gsrc = add_s(scale(gsm, 1.0 - fine_w), gf, fine_w);
        let s_src = dot(gsrc, gsrc).sqrt();
        let strength = smoothstep(0.03, 0.45, s_src).max(flat * if lambda > 9.0 { 0.12 } else { 0.04 });
        // Water collects: small and mid octaves concentrate in hollows and soften on spurs.
        let conv = if lambda < 50.0 { 1.0 + 0.9 * hollow - 0.45 * spur } else { 1.0 };
        let amp = band * strength * conv * gain * TOP_DEPTH_M * libm::exp(h_exp * o.ln);
        if amp < 1e-4 {
            continue;
        }
        // Branching off coarser octaves: weak on steep ground (straight rills), more on flats.
        let g = add_s(add_s(gsrc, dslope, BRANCH * (0.2 + 0.8 * loose)), drift, 1.0);
        let a3 = cross(up, g);
        let al = dot(a3, a3).sqrt().max(1e-12);
        let (m, off) = (&o.m, &o.off);
        let across = rot(m, scale(a3, 1.0 / al));
        let warp = if lambda > 25.0 { scale(wb, 0.1 * lambda) } else { [0.0; 3] };
        let f = 1.0 / lambda;
        let q = add_s(scale(rot(m, add_s(p, warp, 1.0)), f), *off, 1.0);
        let (v, dv) = gully(q, across, 0x6A11_E400 + k as u64);
        // V-shaped floors and rounded spurs on rock; plain rounded swales on soft ground.
        let shaped = v + sharp * (0.25 - 0.5 * v * v);
        let dshape = 1.0 - sharp * v;
        out += amp * shaped;
        let gd = scale(rot_t(m, dv), f * amp * dshape);
        dslope = add_s(dslope, add_s(gd, up, -dot(gd, up)), 1.0);
        // Lumps on gentle ground (meadows, forest floor): rounded value noise.
        if k == 3 || k == 4 {
            let lump = 1.3 * o.lump * (1.0 - 0.7 * smoothstep(0.15, 0.6, s0)) * (1.0 + 0.4 * c2);
            out += band * lump * value3(add_s(q, *off, 3.0), 0xA1C0 + k as u64);
        }
    }
    // Ledges on rock-steep ground (from ~38 degrees), on folded bedding.
    let rock = smoothstep(0.85, 1.4, sm.max(s0 * 0.8)) * smoothstep(25.0, 60.0, land.relief);
    let band1 = octave_band(LEDGE_MIN_M * 2.0, level);
    if rock > 0.0 && band1 > 0.0 {
        let fold = 25.0 * vn(p, 380.0, 0xF1) + 6.0 * vn(p, 90.0, 0xF2);
        let spacing = (LEDGE_MIN_M + LEDGE_SPAN_M * (0.5 + 0.5 * c4)) * (1.0 + 0.25 * vn(p, 23.0, 0x1ED6E));
        let riser = 0.6 + 0.15 * c3;
        let pull = LEDGE_PULL * (0.55 + 0.45 * c2);
        let base = land.h + out + fold;
        for (mul, pk, bd) in [(1.0, pull, band1), (LEDGE2_MUL, LEDGE2_PULL, 1.0)] {
            let sp = spacing * mul;
            let hh = base / sp + 0.5 * vn(p, 41.0 * mul, 0x1ED6F);
            let fl = hh.floor();
            let terrace = fl + smoothstep(riser, 1.0, hh - fl);
            out += rock * bd * pk * (terrace - hh) * sp;
        }
    }
    // Sub-metre micro-relief (rougher on rock).
    for (k, o) in frames.iter().enumerate().skip(GULLY_N) {
        let lambda = o.lambda;
        let band = octave_band(lambda, level);
        if band <= 0.0 {
            continue;
        }
        let (m, off) = (&o.m, &o.off);
        let q = add_s(scale(rot(m, p), 1.0 / lambda), *off, 1.0);
        let v = value3(q, 0xA1D0 + k as u64);
        let v = v + sharp * (0.5 - v.abs() - 0.25);
        let amp = band * 0.35 * (0.4 + 0.6 * gain) * TOP_DEPTH_M * libm::exp(h_exp.min(H_BASE) * o.ln);
        out += amp * v;
    }
    out * fade
}

/// Largest |detail| the function can return (m): the bound on how far it moves measured ground.
pub fn max_detail_m() -> f64 {
    let amp = |lambda: f64| {
        let r = lambda / TOP_M;
        libm::pow(r, H_MIN).max(libm::pow(r, H_MAX)) * TOP_DEPTH_M * 1.3
    };
    let mut s = 0.0;
    for k in 0..GULLY_N {
        let l = lambda_of(k);
        s += amp(l) * 1.9 * 1.25;
        if k == 3 || k == 4 {
            s += LUMP_10M * libm::pow(l / 10.0, 0.85) * 1.3 * 1.4;
        }
    }
    for k in GULLY_N..GULLY_N + MICRO_N {
        s += amp(lambda_of(k)) * 0.35 * 1.25;
    }
    let ledge = LEDGE_PULL * (LEDGE_MIN_M + LEDGE_SPAN_M) * 1.25 + LEDGE2_PULL * (LEDGE_MIN_M + LEDGE_SPAN_M) * 1.25 * LEDGE2_MUL;
    s + ledge
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(lat: f64, lon: f64) -> (V3, f64, f64) {
        let (la, lo) = (lat.to_radians(), lon.to_radians());
        let (_, _, u) = enu(la, lo);
        (u.map(|c| c * 6_371_000.0), la, lo)
    }

    #[test]
    fn coarse_levels_get_nothing_and_fine_levels_stay_bounded() {
        let (p, la, lo) = point(45.98, 7.70);
        assert_eq!(erosion_detail(p, la, lo, &Landform::uniform(2500.0, [0.8, -0.4]), 8), 0.0);
        assert!(first_detail_level() >= 12, "{}", first_detail_level());
        let bound = max_detail_m();
        assert!(bound < 50.0, "bound {bound}");
        let mut worst = 0.0f64;
        for k in 0..4000 {
            let t = k as f64 * 0.37;
            let (p, la, lo) = point(45.98 + t * 1e-6, 7.70 + (t * 1.3).sin() * 1e-4);
            let g = [(t * 0.7).sin() * 1.5, (t * 0.3).cos() * 1.5];
            let mut land = Landform::uniform(2000.0 + t, g);
            land.tpi_m = (t * 0.11).sin() * 20.0;
            land.tpi_l = (t * 0.05).cos() * 40.0;
            let d = erosion_detail(p, la, lo, &land, 16);
            assert!(d.is_finite());
            worst = worst.max(d.abs());
        }
        assert!(worst <= bound, "{worst} > {bound}");
        assert!(worst > 0.5, "steep ground got almost no detail ({worst})");
    }

    #[test]
    fn steep_ground_is_rougher_than_flat_ground() {
        let rough = |g: [f64; 2]| {
            let mut s = 0.0;
            for k in 0..2000 {
                let (p, la, lo) = point(46.0 + k as f64 * 3e-6, 7.7);
                s += erosion_detail(p, la, lo, &Landform::uniform(1600.0, g), 16).powi(2);
            }
            (s / 2000.0).sqrt()
        };
        let (flat, steep) = (rough([0.0, 0.0]), rough([0.9, 0.3]));
        assert!(flat > 0.005 && flat < 0.3, "flat rms {flat}");
        assert!(steep > 3.0 * flat, "steep rms {steep} vs flat {flat}");
    }

    #[test]
    fn hollows_are_cut_deeper_than_spurs() {
        // Gully relief (rms of second differences along the contour) in a hollow vs on a spur.
        let rough = |tpi: f64| {
            let mut s = 0.0;
            let mut prev = [0.0; 2];
            for k in 0..3000 {
                let (p, la, lo) = point(46.0, 7.7 + k as f64 * 4e-6);
                let mut land = Landform::uniform(1600.0, [0.0, 0.5]);
                land.relief = 30.0;
                land.tpi_m = tpi;
                land.tpi_l = tpi;
                let d = erosion_detail(p, la, lo, &land, 17);
                if k >= 2 {
                    s += (d - 2.0 * prev[1] + prev[0]).powi(2);
                }
                prev = [prev[1], d];
            }
            s.sqrt()
        };
        let (hollow, spur) = (rough(-8.0), rough(8.0));
        assert!(hollow > 1.5 * spur, "hollow {hollow} spur {spur}");
    }

    #[test]
    fn fast_sincos_is_close() {
        for k in -2000..2000 {
            let x = k as f64 * 0.00137;
            let (s, c) = sincos_turns(x);
            let (ls, lc) = libm::sincos(2.0 * PI * x);
            assert!((s - ls).abs() < 1e-4 && (c - lc).abs() < 1e-4, "{x}: {s} {c} vs {ls} {lc}");
        }
    }

    #[test]
    fn same_inputs_same_bits() {
        let (p, la, lo) = point(-33.9, 18.4);
        let land = Landform::uniform(800.0, [0.5, 0.2]);
        let a = erosion_detail(p, la, lo, &land, 17);
        let b = erosion_detail(p, la, lo, &land, 17);
        assert_eq!(a.to_bits(), b.to_bits());
    }
}
