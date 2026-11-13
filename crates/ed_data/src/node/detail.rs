//! Erosion detail (P-slice S7 b): relief finer than the elevation source, added to every page
//! sample as a pure function of where the sample is and what the smooth source field does there.
//!
//! The source (Mapterhorn, ~3-5 m pixels) knows the landform but not the gullies, rills, ledges
//! and lumps that make a slope read as eroded rock or a pasture from a few metres away. This
//! adds them, everywhere on Earth, with no place-specific rule:
//!
//! * **Gullies** (an analytic erosion filter after runevision's / Clay John's "eroded noise"):
//!   per octave, jittered points on a 3D lattice each carry a cosine stripe whose crests run
//!   **down** the slope (the stripe varies across the fall line); a smooth kernel blends the
//!   points. Each octave's own slope feeds the next octave's direction, so finer rills branch off
//!   coarser gullies. Strength grows with the source slope: flats get none, steep ground most.
//! * **Ledges**: on rock-steep ground the height is pulled toward soft terraces a few metres
//!   apart (strata), with the spacing and phase varied by noise.
//! * **Micro-relief**: low smooth value noise (fields, meadows), strongest on gentle ground.
//!
//! Amplitudes stay below the source's own resolution (at most ~1.5 m on a cliff, ~0.15 m on a
//! plain), so measured terrain does not move; octaves a page's lattice cannot carry (wavelength
//! under ~3 samples of the level's nominal spacing) fade out, so coarse levels stay smooth and a
//! node morphs into its children's detail.
//!
//! Determinism: f64 throughout, `libm` transcendentals, an integer hash of lattice cells; inputs
//! are the lattice point (bit-identical between neighbours), the source height and its gradient
//! (computed from the same source taps by every node that shares the point), and the level. So
//! shared borders stay bit-identical (G4).

use std::f64::consts::PI;

/// Gully octaves: wavelength (m) and depth as a fraction of the wavelength (at full strength).
const GULLY_OCTAVES: [(f64, f64); 4] = [(16.0, 0.040), (8.0, 0.040), (4.0, 0.036), (2.0, 0.030)];
/// Branching: how much of an octave's own slope steers the next octave's flow direction.
const BRANCH: f64 = 0.9;
/// Ledge (strata) spacing (m) and pull strength on rock-steep ground.
const LEDGE_M: f64 = 3.2;
const LEDGE_PULL: f64 = 0.22;
/// Micro-relief: wavelengths (m) and amplitudes (m).
const MICRO: [(f64, f64); 3] = [(14.0, 0.07), (5.0, 0.035), (2.2, 0.015)];
/// Nominal page sample spacing at level 0 (m): a quarter meridian over 256 cells. A pure
/// function of the level (never of the node's real, warped spacing), so neighbours agree.
const SPACING_L0_M: f64 = PI * 0.5 * 6_371_000.0 / 256.0;

/// Nominal page sample spacing (m) of node level `level`.
pub fn nominal_spacing(level: u8) -> f64 {
    SPACING_L0_M / (1u64 << level.min(40)) as f64
}

/// First level that receives any detail (the coarsest octave fits from here).
pub fn first_detail_level() -> u8 {
    (0..40u8).find(|&l| octave_band(GULLY_OCTAVES[0].0.max(MICRO[0].0), l) > 0.0).unwrap_or(40)
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

/// One gully octave at `q` (lattice units): stripes whose crests run along the unit tangent
/// `flow` (the fall line), so they vary along `across` = the unit horizontal direction across
/// the slope. Returns the value in about [-1, 1] and its derivative with respect to `q`.
fn gully(q: V3, across: V3, salt: u64) -> (f64, V3) {
    const JITTER: f64 = 0.25;
    const R2: f64 = 1.0;
    let c0 = [q[0].floor(), q[1].floor(), q[2].floor()];
    let (mut sv, mut sw) = (0.0, 0.0);
    let mut sd = [0.0; 3];
    for dz in -1..=1i64 {
        for dy in -1..=1i64 {
            for dx in -1..=1i64 {
                let c = [c0[0] + dx as f64, c0[1] + dy as f64, c0[2] + dz as f64];
                let j = unit3(hash3(c[0] as i64, c[1] as i64, c[2] as i64, salt));
                let pp = [q[0] - (c[0] + 0.5 + JITTER * j[0]), q[1] - (c[1] + 0.5 + JITTER * j[1]), q[2] - (c[2] + 0.5 + JITTER * j[2])];
                let d2 = dot(pp, pp);
                if d2 >= R2 {
                    continue;
                }
                let k = 1.0 - d2 / R2;
                let w = k * k * k;
                let (s, co) = libm::sincos(2.0 * PI * dot(pp, across));
                sv += w * co;
                sd = add_s(sd, across, -w * s * 2.0 * PI);
                sw += w;
            }
        }
    }
    let n = 1.0 / (sw + 0.02);
    (sv * n, [sd[0] * n, sd[1] * n, sd[2] * n])
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

/// Local frame of a geodetic point: east, north, up (ECEF unit vectors).
pub(crate) fn enu(lat_rad: f64, lon_rad: f64) -> (V3, V3, V3) {
    let (sl, cl) = libm::sincos(lat_rad);
    let (so, co) = libm::sincos(lon_rad);
    ([-so, co, 0.0], [-sl * co, -sl * so, cl], [cl * co, cl * so, sl])
}

/// Detail height (m) to add to a page sample.
///
/// * `p`: the lattice point on the ellipsoid (ECEF metres, `d * r(d)`).
/// * `lat_rad`, `lon_rad`: its geodetic latitude and longitude.
/// * `h`: the smooth source height there (m).
/// * `grad_en`: the source height's gradient (m per m) toward east and north.
/// * `level`: the node level (selects the octaves the page can carry).
pub fn erosion_detail(p: V3, lat_rad: f64, lon_rad: f64, h: f64, grad_en: [f64; 2], level: u8) -> f64 {
    let (e, n, up) = enu(lat_rad, lon_rad);
    let mut g = add_s([e[0] * grad_en[0], e[1] * grad_en[0], e[2] * grad_en[0]], n, grad_en[1]);
    let slope0 = dot(g, g).sqrt();
    let mut out = 0.0;
    // Gullies.
    for (k, &(lambda, depth)) in GULLY_OCTAVES.iter().enumerate() {
        let band = octave_band(lambda, level);
        if band <= 0.0 {
            continue;
        }
        let slope = dot(g, g).sqrt();
        let strength = smoothstep(0.06, 0.55, slope);
        if strength <= 0.0 {
            continue;
        }
        // Unit horizontal direction across the fall line (up x gradient).
        let a = cross(up, g);
        let al = dot(a, a).sqrt().max(1e-12);
        let across = [a[0] / al, a[1] / al, a[2] / al];
        let f = 1.0 / lambda;
        let off = 0.37 * k as f64;
        let q = [p[0] * f + off, p[1] * f + 2.0 * off, p[2] * f + 3.0 * off];
        let (v, dv) = gully(q, across, 0x6A11_E400 + k as u64);
        // Sharp gully floors, rounded spurs: bias the cosine toward its crests.
        let shaped = v - 0.25 * v * v + 0.25 * 0.5;
        let dshape = 1.0 - 0.5 * v;
        let amp = band * strength * depth * lambda;
        out += amp * shaped;
        // Steer the next octave by this one's slope (metres per metre), kept in the tangent plane.
        let gd = [dv[0] * f * amp * dshape * BRANCH, dv[1] * f * amp * dshape * BRANCH, dv[2] * f * amp * dshape * BRANCH];
        let gd = add_s(gd, up, -dot(gd, up));
        g = add_s(g, gd, 1.0);
    }
    // Ledges on rock-steep ground (from ~38 degrees).
    let rock = smoothstep(0.75, 1.3, slope0) * octave_band(LEDGE_M * 2.0, level);
    if rock > 0.0 {
        let wobble = value3([p[0] / 23.0, p[1] / 23.0, p[2] / 23.0], 0x1ED6E);
        let spacing = LEDGE_M * (1.0 + 0.3 * wobble);
        let hh = (h + out) / spacing + 0.5 * value3([p[0] / 41.0, p[1] / 41.0, p[2] / 41.0], 0x1ED6F);
        let fl = hh.floor();
        let t = hh - fl;
        // A riser over the top 40 % of each band, a near-flat bench below it.
        let terrace = fl + smoothstep(0.6, 1.0, t);
        out += rock * LEDGE_PULL * (terrace - hh) * spacing;
    }
    // Micro-relief, strongest on gentle ground.
    let gentle = 1.0 - 0.6 * smoothstep(0.1, 0.6, slope0);
    for (k, &(lambda, amp)) in MICRO.iter().enumerate() {
        let band = octave_band(lambda, level);
        if band <= 0.0 {
            continue;
        }
        let f = 1.0 / lambda;
        out += band * gentle * amp * value3([p[0] * f, p[1] * f, p[2] * f], 0xA1C0 + k as u64);
    }
    out
}

/// Largest |detail| the function can return (m): the bound on how far it moves measured ground.
pub fn max_detail_m() -> f64 {
    let g: f64 = GULLY_OCTAVES.iter().map(|&(l, d)| l * d * 1.125).sum();
    let m: f64 = MICRO.iter().map(|&(_, a)| a).sum();
    g + LEDGE_PULL * LEDGE_M * 1.3 + m
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
        assert_eq!(erosion_detail(p, la, lo, 2500.0, [0.8, -0.4], 8), 0.0);
        assert!(first_detail_level() >= 12, "{}", first_detail_level());
        let bound = max_detail_m();
        assert!(bound < 2.5, "bound {bound}");
        let mut worst = 0.0f64;
        for k in 0..4000 {
            let t = k as f64 * 0.37;
            let (p, la, lo) = point(45.98 + t * 1e-6, 7.70 + (t * 1.3).sin() * 1e-4);
            let g = [(t * 0.7).sin() * 1.5, (t * 0.3).cos() * 1.5];
            let d = erosion_detail(p, la, lo, 2000.0 + t, g, 16);
            assert!(d.is_finite());
            worst = worst.max(d.abs());
        }
        assert!(worst <= bound, "{worst} > {bound}");
        assert!(worst > 0.3, "steep ground got almost no detail ({worst})");
    }

    #[test]
    fn steep_ground_is_rougher_than_flat_ground() {
        let rough = |g: [f64; 2]| {
            let mut s = 0.0;
            for k in 0..2000 {
                let (p, la, lo) = point(46.0 + k as f64 * 3e-6, 7.7);
                s += erosion_detail(p, la, lo, 1600.0, g, 16).powi(2);
            }
            (s / 2000.0).sqrt()
        };
        let (flat, steep) = (rough([0.0, 0.0]), rough([0.9, 0.3]));
        assert!(flat > 0.005 && flat < 0.12, "flat rms {flat}");
        assert!(steep > 3.0 * flat, "steep rms {steep} vs flat {flat}");
    }

    #[test]
    fn same_inputs_same_bits() {
        let (p, la, lo) = point(-33.9, 18.4);
        let a = erosion_detail(p, la, lo, 800.0, [0.5, 0.2], 17);
        let b = erosion_detail(p, la, lo, 800.0, [0.5, 0.2], 17);
        assert_eq!(a.to_bits(), b.to_bits());
    }
}
