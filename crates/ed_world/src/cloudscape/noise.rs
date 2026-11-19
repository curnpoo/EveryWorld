//! Tileable noise for the cloud field, baked once from the world seed. The CPU reference and
//! the GPU read the same bytes with the same filter (trilinear, texel centres at `(i+0.5)/n`,
//! wrapping), so the two draw the same clouds.
//!
//! - **shape** (`n`^3 RGBA8): R Perlin-Worley (connected billows), G/B/A Worley fBm starting
//!   at 4, 8 and 10 cells (3 octaves each). The low-frequency body of every cloud.
//! - **detail** (`n`^3 RGBA8): R/G/B Worley fBm starting at 2, 4, 6 cells, A Perlin fBm from
//!   3. Erodes edges: Worley for billowy cauliflower tops, Perlin for torn, wispy bases and
//!   cirrus fibres.
//! - **weather** (`n`^2 RGBA8): R and G two independent coverage fields (the field crossfades
//!   between them over time, so clouds form and dissolve instead of only sliding), B tower
//!   height (which cells grow tall), A regional variation (clusters and clear gaps).
//!
//! Storage is half float (`Rgba16Float` on the GPU, the same f16 bits here): 8-bit noise
//! terraces wherever its gradient is under one step per texel (bilinear interpolation of
//! equal bytes is flat), and a low sun turns those plateaus into contour ripples on smooth
//! cloud tops. f16 steps are 8x finer in 0.5..1.
//!
//! Why the textures look smooth on screen ("no weird noise"): every channel is an fBm of
//! smooth or F1-Worley octaves (no value noise, no hash grain), the mip chain ([`mips_3d`])
//! lets the GPU pick a pre-filtered level by distance, and the field fades detail out before
//! it would alias (see `field::Lod`).

use crate::clouds::{perlin, worley};
use glam::{Vec2, Vec3, Vec4};

/// Default sizes (texels per side). Shape 128^3 = 8 MiB, detail 64^3 = 1 MiB, weather 512^2
/// = 1 MiB, before mips.
///
/// Every octave stays at 2.7 texels per noise cell or more at these sizes (see the channel
/// lists): finer octaves alias inside the texture itself and show as moire ripples on flat
/// cloud tops, which no filtering at draw time can remove.
pub const SHAPE_N: usize = 128;
pub const DETAIL_N: usize = 64;
pub const WEATHER_N: usize = 512;

/// World size of one shape tile (m): one tile spans several cumulus.
pub const SHAPE_PERIOD_M: f64 = 5_000.0;
/// World size of one detail tile (m).
pub const DETAIL_PERIOD_M: f64 = 900.0;
/// World size of one weather tile at `cell_m` = 1 m: the field scales it by the layer's cell
/// size, so one tile holds 8 x 8 cloud cells (R and G have 8 cells per side).
pub const WEATHER_CELLS: f64 = 8.0;

/// A tiling 3D RGBA volume of f16 bits, x fastest, then y, then z.
#[derive(Clone, Debug, PartialEq)]
pub struct Volume {
    pub n: usize,
    pub data: Vec<u16>,
}

/// A tiling 2D RGBA map of f16 bits, x fastest.
#[derive(Clone, Debug, PartialEq)]
pub struct Map2 {
    pub n: usize,
    pub data: Vec<u16>,
}

/// IEEE half -> f32 (exact).
pub fn f16_to_f32(h: u16) -> f32 {
    let (s, e, m) = ((h >> 15) as u32, ((h >> 10) & 0x1f) as u32, (h & 0x3ff) as u32);
    let bits = match (e, m) {
        (0, 0) => s << 31,
        (0, _) => {
            // Subnormal: renormalise.
            let mut e = 127 - 15 + 1;
            let mut m = m;
            while m & 0x400 == 0 {
                m <<= 1;
                e -= 1;
            }
            (s << 31) | ((e as u32) << 23) | ((m & 0x3ff) << 13)
        }
        (31, _) => (s << 31) | 0x7f80_0000 | (m << 13),
        _ => (s << 31) | ((e + 127 - 15) << 23) | (m << 13),
    };
    f32::from_bits(bits)
}

/// f32 -> IEEE half, round to nearest even (finite inputs in the noise range).
pub fn f32_to_f16(v: f32) -> u16 {
    let b = v.to_bits();
    let s = ((b >> 16) & 0x8000) as u16;
    let e = ((b >> 23) & 0xff) as i32 - 127 + 15;
    let m = b & 0x7f_ffff;
    if e >= 31 {
        return s | 0x7c00;
    }
    if e <= 0 {
        if e < -10 {
            return s;
        }
        let m = m | 0x80_0000;
        let shift = (14 - e) as u32;
        let half = 1u32 << (shift - 1);
        let r = m + half - 1 + ((m >> shift) & 1);
        return s | (r >> shift) as u16;
    }
    let r = m + 0xfff + ((m >> 13) & 1);
    let (e, r) = if r & 0x80_0000 != 0 { (e + 1, 0) } else { (e, r) };
    if e >= 31 {
        return s | 0x7c00;
    }
    s | ((e as u16) << 10) | (r >> 13) as u16
}

fn f16_table() -> &'static [f32] {
    static T: std::sync::OnceLock<Vec<f32>> = std::sync::OnceLock::new();
    T.get_or_init(|| (0..=u16::MAX).map(f16_to_f32).collect())
}

#[inline]
fn texel(data: &[u16], i: usize) -> Vec4 {
    let t = f16_table();
    let p = &data[i * 4..i * 4 + 4];
    Vec4::new(t[p[0] as usize], t[p[1] as usize], t[p[2] as usize], t[p[3] as usize])
}

/// Little-endian bytes of f16 texels, for an `Rgba16Float` upload.
pub fn f16_bytes(data: &[u16]) -> Vec<u8> {
    data.iter().flat_map(|h| h.to_le_bytes()).collect()
}

/// Wrapped texel coordinate pair and weight for one axis (GPU rule: centres at `(i+0.5)/n`).
#[inline]
fn axis(u: f32, n: usize) -> (usize, usize, f32) {
    let x = u * n as f32 - 0.5;
    let f = x.floor();
    let i = (f as i64).rem_euclid(n as i64) as usize;
    (i, (i + 1) % n, x - f)
}

impl Volume {
    /// Trilinear sample at `p` in tile units (wraps).
    pub fn sample(&self, p: Vec3) -> Vec4 {
        let n = self.n;
        let (x0, x1, fx) = axis(p.x, n);
        let (y0, y1, fy) = axis(p.y, n);
        let (z0, z1, fz) = axis(p.z, n);
        let at = |x: usize, y: usize, z: usize| texel(&self.data, x + n * (y + n * z));
        let a = at(x0, y0, z0).lerp(at(x1, y0, z0), fx);
        let b = at(x0, y1, z0).lerp(at(x1, y1, z0), fx);
        let c = at(x0, y0, z1).lerp(at(x1, y0, z1), fx);
        let d = at(x0, y1, z1).lerp(at(x1, y1, z1), fx);
        a.lerp(b, fy).lerp(c.lerp(d, fy), fz)
    }
}

impl Map2 {
    /// Bilinear sample at `uv` in tile units (wraps).
    pub fn sample(&self, uv: Vec2) -> Vec4 {
        let n = self.n;
        let (x0, x1, fx) = axis(uv.x, n);
        let (y0, y1, fy) = axis(uv.y, n);
        let at = |x: usize, y: usize| texel(&self.data, x + n * y);
        at(x0, y0).lerp(at(x1, y0), fx).lerp(at(x0, y1).lerp(at(x1, y1), fx), fy)
    }
}

/// Every noise texture of the cloud field.
#[derive(Clone, Debug, PartialEq)]
pub struct NoiseBank {
    pub shape: Volume,
    pub detail: Volume,
    pub weather: Map2,
    pub seed: u64,
    /// Coverage quantiles of the weather map's R channel: `cov_q[k]` is the value a fraction
    /// `1 - k / COV_Q` of the texels reach. [`NoiseBank::cov_threshold`] turns a coverage
    /// into the noise threshold that covers exactly that share of the sky.
    pub cov_q: [f32; COV_Q + 1],
}

/// Steps of the coverage quantile table.
pub const COV_Q: usize = 32;

fn worley_fbm(p: [f32; 3], freq: i32, seed: u32) -> f32 {
    worley(p, freq, seed) * 0.625 + worley(p, freq * 2, seed + 1) * 0.25 + worley(p, freq * 4, seed + 2) * 0.125
}

fn perlin_fbm(p: [f32; 3], freq: i32, seed: u32, octaves: u32) -> f32 {
    let (mut sum, mut amp, mut norm, mut f) = (0.0, 0.5, 0.0, freq);
    for o in 0..octaves {
        sum += perlin(p, f, seed + o) * amp;
        norm += amp;
        amp *= 0.5;
        f *= 2;
    }
    (sum / norm * 0.5 * 1.6 + 0.5).clamp(0.0, 1.0)
}

fn remap(v: f32, lo: f32, hi: f32, nlo: f32, nhi: f32) -> f32 {
    nlo + (v - lo) / (hi - lo) * (nhi - nlo)
}

/// Perlin-Worley: Perlin dilated by Worley, keeping Perlin's connected structure with
/// Worley's round billows (Schneider 2015).
fn perlin_worley(p: [f32; 3], freq: i32, seed: u32) -> f32 {
    let pf = perlin_fbm(p, freq, seed, 4);
    let w = worley_fbm(p, freq, seed + 10);
    remap(pf, w - 1.0, 1.0, 0.0, 1.0).clamp(0.0, 1.0)
}

fn quant(v: f32) -> u16 {
    f32_to_f16(v.clamp(0.0, 1.0))
}

/// Fills `count` RGBA texels (f16 bits) in parallel; `f(i)` gives texel `i`.
fn fill(count: usize, f: impl Fn(usize) -> [f32; 4] + Sync) -> Vec<u16> {
    let mut data = vec![0u16; count * 4];
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(1, 32);
    let per = count.div_ceil(threads);
    std::thread::scope(|s| {
        for (k, chunk) in data.chunks_mut(per * 4).enumerate() {
            let f = &f;
            s.spawn(move || {
                for (j, px) in chunk.chunks_exact_mut(4).enumerate() {
                    let v = f(k * per + j);
                    for c in 0..4 {
                        px[c] = quant(v[c]);
                    }
                }
            });
        }
    });
    data
}

fn volume(n: usize, f: impl Fn([f32; 3]) -> [f32; 4] + Sync) -> Volume {
    let data = fill(n * n * n, |i| {
        let (x, y, z) = (i % n, (i / n) % n, i / (n * n));
        f([(x as f32 + 0.5) / n as f32, (y as f32 + 0.5) / n as f32, (z as f32 + 0.5) / n as f32])
    });
    Volume { n, data }
}

impl NoiseBank {
    /// The full-size bank for `seed` (a few seconds on 4 cores in release; bake it once, off
    /// the frame, and share it).
    pub fn bake(seed: u64) -> Self {
        Self::bake_sized(seed, SHAPE_N, DETAIL_N, WEATHER_N)
    }

    /// A bank with custom sizes (tests use small ones).
    pub fn bake_sized(seed: u64, shape_n: usize, detail_n: usize, weather_n: usize) -> Self {
        let s = (seed ^ (seed >> 32)) as u32;
        let shape = volume(shape_n, |p| {
            [perlin_worley(p, 4, s.wrapping_add(11)), worley_fbm(p, 4, s.wrapping_add(21)), worley_fbm(p, 8, s.wrapping_add(31)), worley_fbm(p, 10, s.wrapping_add(41))]
        });
        let detail = volume(detail_n, |p| {
            [worley_fbm(p, 2, s.wrapping_add(51)), worley_fbm(p, 4, s.wrapping_add(61)), worley_fbm(p, 6, s.wrapping_add(71)), perlin_fbm(p, 3, s.wrapping_add(81), 4)]
        });
        let n = weather_n;
        let wdata = fill(n * n, |i| {
            let p = [(i % n) as f32 / n as f32 + 0.5 / n as f32, (i / n) as f32 / n as f32 + 0.5 / n as f32, 0.37];
            let cells = WEATHER_CELLS as i32;
            // Coverage: Perlin-Worley blobs (one blob per cell) with a little finer Perlin so
            // the outlines are not round.
            let cov = |seed: u32| {
                let pw = perlin_worley(p, cells, seed);
                (pw * 0.8 + perlin_fbm(p, cells * 2, seed + 5, 3) * 0.2).clamp(0.0, 1.0)
            };
            let tower = worley_fbm(p, cells, s.wrapping_add(131));
            let region = perlin_fbm(p, 2, s.wrapping_add(151), 3);
            [cov(s.wrapping_add(101)), cov(s.wrapping_add(117)), tower, region]
        });
        let mut r: Vec<f32> = wdata.iter().step_by(4).map(|&h| f16_to_f32(h)).collect();
        r.sort_unstable_by(f32::total_cmp);
        let mut cov_q = [0.0; COV_Q + 1];
        for (k, q) in cov_q.iter_mut().enumerate() {
            // k = 0: everything reaches it (coverage 1); k = COV_Q: nothing does.
            let i = (k * (r.len() - 1)) / COV_Q;
            *q = r[i];
        }
        cov_q[COV_Q] = 1.0;
        Self { shape, detail, weather: Map2 { n, data: wdata }, seed, cov_q }
    }

    /// The coverage-noise value above which a fraction `coverage` of the sky is cloud.
    pub fn cov_threshold(&self, coverage: f32) -> f32 {
        let x = (1.0 - coverage.clamp(0.0, 1.0)) * COV_Q as f32;
        let i = (x.floor() as usize).min(COV_Q - 1);
        let f = x - i as f32;
        self.cov_q[i] + (self.cov_q[i + 1] - self.cov_q[i]) * f
    }
}

/// Box-filtered mip chain of a volume (level 0 first), for the GPU upload: far clouds read a
/// pre-filtered level instead of aliasing the base one.
pub fn mips_3d(v: &Volume) -> Vec<Vec<u16>> {
    let t = f16_table();
    let mut out = vec![v.data.clone()];
    let mut n = v.n;
    while n > 1 {
        let (src, m) = (out.last().expect("level 0"), n / 2);
        let mut dst = vec![0u16; m * m * m * 4];
        for z in 0..m {
            for y in 0..m {
                for x in 0..m {
                    for c in 0..4 {
                        let mut sum = 0.0f32;
                        for k in 0..8 {
                            let (dx, dy, dz) = (k & 1, (k >> 1) & 1, k >> 2);
                            sum += t[src[((2 * x + dx) + n * ((2 * y + dy) + n * (2 * z + dz))) * 4 + c] as usize];
                        }
                        dst[(x + m * (y + m * z)) * 4 + c] = f32_to_f16(sum / 8.0);
                    }
                }
            }
        }
        out.push(dst);
        n = m;
    }
    out
}

/// Box-filtered mip chain of a 2D map (level 0 first).
pub fn mips_2d(v: &Map2) -> Vec<Vec<u16>> {
    let t = f16_table();
    let mut out = vec![v.data.clone()];
    let mut n = v.n;
    while n > 1 {
        let (src, m) = (out.last().expect("level 0"), n / 2);
        let mut dst = vec![0u16; m * m * 4];
        for y in 0..m {
            for x in 0..m {
                for c in 0..4 {
                    let s = |dx: usize, dy: usize| t[src[((2 * x + dx) + n * (2 * y + dy)) * 4 + c] as usize];
                    dst[(x + m * y) * 4 + c] = f32_to_f16((s(0, 0) + s(1, 0) + s(0, 1) + s(1, 1)) / 4.0);
                }
            }
        }
        out.push(dst);
        n = m;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bank() -> NoiseBank {
        NoiseBank::bake_sized(7, 16, 8, 32)
    }

    #[test]
    fn deterministic() {
        assert_eq!(bank(), bank());
        assert_ne!(bank().shape, NoiseBank::bake_sized(8, 16, 8, 32).shape);
    }

    #[test]
    fn samples_wrap_seamlessly() {
        let b = bank();
        for p in [Vec3::new(0.01, 0.3, 0.7), Vec3::new(0.5, 0.999, 0.2)] {
            let a = b.shape.sample(p);
            let c = b.shape.sample(p + Vec3::new(1.0, -3.0, 2.0));
            assert!((a - c).abs().max_element() < 1e-5);
        }
        let uv = Vec2::new(0.2, 0.9);
        assert!((b.weather.sample(uv) - b.weather.sample(uv + Vec2::new(5.0, -1.0))).abs().max_element() < 1e-5);
    }

    #[test]
    fn sampling_hits_texels_at_centres_and_interpolates() {
        let b = bank();
        let n = b.shape.n;
        let c = Vec3::splat(0.5) / n as f32 + Vec3::new(3.0, 5.0, 7.0) / n as f32;
        let i = 3 + n * (5 + n * 7);
        assert!((b.shape.sample(c) - texel(&b.shape.data, i)).abs().max_element() < 1e-5);
        // Continuous: tiny steps make tiny changes (no grain).
        let mut prev = b.shape.sample(Vec3::new(0.0, 0.2, 0.4));
        for k in 1..200 {
            let v = b.shape.sample(Vec3::new(k as f32 * 0.001, 0.2, 0.4));
            assert!((v - prev).abs().max_element() < 0.08, "step {k}");
            prev = v;
        }
    }

    #[test]
    fn channels_have_contrast() {
        let b = bank();
        for c in 0..4 {
            let vals: Vec<f32> = b.shape.data.iter().skip(c).step_by(4).map(|&h| f16_to_f32(h)).collect();
            let lo = vals.iter().copied().fold(1.0f32, f32::min);
            let hi = vals.iter().copied().fold(0.0f32, f32::max);
            assert!(hi - lo > 0.3, "shape channel {c}: {lo}..{hi}");
        }
    }

    #[test]
    fn half_floats_round_trip() {
        for v in [0.0f32, 1.0, 0.5, 0.1234, 0.999, 1e-5, 6.1e-5, 0.33333] {
            let h = f32_to_f16(v);
            let back = f16_to_f32(h);
            assert!((back - v).abs() <= v.abs() * 1e-3 + 1e-7, "{v} -> {back}");
        }
        for h in [0u16, 0x3c00, 0x3800, 0x0001, 0x03ff, 0x7bff] {
            assert_eq!(f32_to_f16(f16_to_f32(h)), h);
        }
        // Finer than 8 bits near 1 (where the carve thresholds sit).
        assert!(f16_to_f32(f32_to_f16(0.9)) != f16_to_f32(f32_to_f16(0.9 + 1.0 / 1024.0)));
    }

    #[test]
    fn mip_chains_reach_one_texel() {
        let b = bank();
        let m = mips_3d(&b.shape);
        assert_eq!(m.len(), 5);
        assert_eq!(m[4].len(), 4);
        let w = mips_2d(&b.weather);
        assert_eq!(w.len(), 6);
        // The 1-texel level is the mean.
        let mean = b.weather.data.iter().step_by(4).map(|&v| f16_to_f32(v)).sum::<f32>() / (32.0 * 32.0);
        assert!((f16_to_f32(w[5][0]) - mean).abs() < 2e-3);
    }
}
