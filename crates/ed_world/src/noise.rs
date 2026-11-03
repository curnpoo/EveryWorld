//! Small deterministic noise kit: value noise, fBm and Worley (cellular)
//! noise, in plain and periodic (tileable) flavours. Used for ground splat
//! breakup, procedural ground textures and sculpted scatter meshes.

#[inline]
fn hash3(x: i64, y: i64, seed: u64) -> f32 {
    crate::hash01(((x as u64) << 32) ^ (y as u64 & 0xffff_ffff), seed.wrapping_mul(0x2545_F491_4F6C_DD1D) ^ 0x5bd1_e995)
}

#[inline]
fn smooth(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[inline]
fn wrap(i: i64, period: i64) -> i64 {
    if period <= 0 { i } else { i.rem_euclid(period) }
}

/// Value noise in [0,1]. `period` > 0 makes it tile every `period` units.
pub fn value(x: f64, y: f64, seed: u64, period: i64) -> f32 {
    let (xf, yf) = (x.floor(), y.floor());
    let (xi, yi) = (xf as i64, yf as i64);
    let (tx, ty) = (smooth((x - xf) as f32), smooth((y - yf) as f32));
    let h = |i: i64, j: i64| hash3(wrap(xi + i, period), wrap(yi + j, period), seed);
    let a = h(0, 0) + (h(1, 0) - h(0, 0)) * tx;
    let b = h(0, 1) + (h(1, 1) - h(0, 1)) * tx;
    a + (b - a) * ty
}

/// Fractal value noise in [0,1]. With `period` > 0 every octave tiles (the
/// period doubles with the frequency).
pub fn fbm(x: f64, y: f64, seed: u64, octaves: u32, period: i64) -> f32 {
    let (mut sum, mut amp, mut norm, mut f, mut p) = (0.0, 0.5, 0.0, 1.0, period);
    for o in 0..octaves {
        sum += amp * value(x * f, y * f, seed.wrapping_add(o as u64 * 101), p);
        norm += amp;
        amp *= 0.5;
        f *= 2.0;
        if p > 0 {
            p *= 2;
        }
    }
    sum / norm
}

/// Worley noise: (distance to nearest feature point, to second nearest, id of nearest cell).
/// Distances are in cell units. `period` > 0 tiles.
pub fn worley(x: f64, y: f64, seed: u64, period: i64, jitter: f32) -> (f32, f32, u64) {
    let (xi, yi) = (x.floor() as i64, y.floor() as i64);
    let (mut d1, mut d2, mut id) = (9.0f32, 9.0f32, 0u64);
    for j in -1..=1 {
        for i in -1..=1 {
            let (cx, cy) = (wrap(xi + i, period), wrap(yi + j, period));
            let fx = (xi + i) as f64 + 0.5 + (hash3(cx, cy, seed) as f64 - 0.5) * jitter as f64;
            let fy = (yi + j) as f64 + 0.5 + (hash3(cx, cy, seed ^ 0x9e37) as f64 - 0.5) * jitter as f64;
            let d = (((fx - x).powi(2) + (fy - y).powi(2)) as f32).sqrt();
            if d < d1 {
                d2 = d1;
                d1 = d;
                id = ((cx as u64) << 32) ^ cy as u64;
            } else if d < d2 {
                d2 = d;
            }
        }
    }
    (d1, d2, id)
}

/// 3D value noise in [0,1] (for sculpting meshes; not periodic).
pub fn value3(x: f32, y: f32, z: f32, seed: u64) -> f32 {
    let (xf, yf, zf) = (x.floor(), y.floor(), z.floor());
    let (xi, yi, zi) = (xf as i64, yf as i64, zf as i64);
    let (tx, ty, tz) = (smooth(x - xf), smooth(y - yf), smooth(z - zf));
    let h = |i: i64, j: i64, k: i64| hash3(xi + i, (yi + j) * 7919 + (zi + k) * 104_729, seed);
    let l = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let x00 = l(h(0, 0, 0), h(1, 0, 0), tx);
    let x10 = l(h(0, 1, 0), h(1, 1, 0), tx);
    let x01 = l(h(0, 0, 1), h(1, 0, 1), tx);
    let x11 = l(h(0, 1, 1), h(1, 1, 1), tx);
    l(l(x00, x10, ty), l(x01, x11, ty), tz)
}

pub fn fbm3(x: f32, y: f32, z: f32, seed: u64, octaves: u32) -> f32 {
    let (mut sum, mut amp, mut norm, mut f) = (0.0, 0.5, 0.0, 1.0);
    for o in 0..octaves {
        sum += amp * value3(x * f, y * f, z * f, seed.wrapping_add(o as u64 * 31));
        norm += amp;
        amp *= 0.5;
        f *= 2.03;
    }
    sum / norm
}

#[inline]
pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn periodic_noise_tiles() {
        for k in 0..20 {
            let (x, y) = (k as f64 * 0.37, k as f64 * 0.91);
            assert!((value(x, y, 3, 8) - value(x + 8.0, y - 16.0, 3, 8)).abs() < 1e-5);
            assert!((fbm(x, y, 3, 4, 8) - fbm(x + 8.0, y + 8.0, 3, 4, 8)).abs() < 1e-5);
            let a = worley(x, y, 5, 6, 1.0);
            let b = worley(x + 6.0, y, 5, 6, 1.0);
            assert!((a.0 - b.0).abs() < 1e-4);
        }
    }

    #[test]
    fn ranges() {
        for k in 0..500 {
            let v = fbm(k as f64 * 0.173, k as f64 * 0.311, 9, 5, 0);
            assert!((0.0..=1.0).contains(&v));
            let w = value3(k as f32 * 0.21, 1.3, k as f32 * 0.05, 2);
            assert!((0.0..=1.0).contains(&w));
        }
    }
}
