//! Procedural, tileable ground textures: one per [`Ground`] layer.
//!
//! Every layer is synthesized from periodic noise and little "painted"
//! elements (grass strokes, leaves, pebbles, slabs) into a height field and
//! an albedo. Normals, cavity AO and roughness are derived from the height.
//! Output is plain RGBA8 so the game can upload it as a texture array and
//! tests can dump it to disk. Each layer's mean albedo is matched to
//! [`Ground::mean_color`], so the far-field ground (mip tail) keeps the
//! palette that art direction picked.

use crate::noise::{fbm, smoothstep, value, worley};
use crate::surface::{Ground, LAYERS};

/// Real-world size (meters) that one texture repeat covers.
pub fn world_size_m(g: Ground) -> f32 {
    match g {
        Ground::Rock => 8.0,
        Ground::Farmland => 6.0,
        Ground::Snow => 8.0,
        Ground::Paving => 4.8,
        Ground::Sand => 4.0,
        Ground::Grass | Ground::DryGrass => 6.0,
        _ => 4.0,
    }
}

/// Normal-map strength per layer (shader scales detail normals by this too).
pub fn bumpiness(g: Ground) -> f32 {
    match g {
        Ground::Snow => 0.25,
        Ground::Sand => 0.9,
        Ground::Paving => 1.4,
        Ground::Rock => 2.2,
        Ground::Gravel => 2.0,
        Ground::Grass | Ground::DryGrass => 0.3,
        Ground::ForestFloor | Ground::Moss => 0.6,
        _ => 1.6,
    }
}

pub struct LayerTexture {
    pub size: u32,
    /// sRGB albedo in RGB, height (0 low .. 255 high) in A.
    pub albedo_height: Vec<u8>,
    /// Linear: tangent normal X, Y (0.5 = flat), perceptual roughness, AO.
    pub normal_rough_ao: Vec<u8>,
}

/// Linear working buffers for one layer.
struct Canvas {
    n: usize,
    h: Vec<f32>,
    c: Vec<[f32; 3]>,
    r: Vec<f32>,
}

impl Canvas {
    fn new(n: usize) -> Self {
        Self { n, h: vec![0.0; n * n], c: vec![[0.0; 3]; n * n], r: vec![0.9; n * n] }
    }

    /// Fill every pixel from a function of (s, t) in [0,1).
    fn fill(&mut self, f: impl Fn(f64, f64) -> (f32, [f32; 3], f32)) {
        let n = self.n;
        for y in 0..n {
            for x in 0..n {
                let (h, c, r) = f(x as f64 / n as f64, y as f64 / n as f64);
                let k = y * n + x;
                self.h[k] = h;
                self.c[k] = c;
                self.r[k] = r;
            }
        }
    }

    #[inline]
    fn idx(&self, x: i64, y: i64) -> usize {
        let n = self.n as i64;
        (y.rem_euclid(n) * n + x.rem_euclid(n)) as usize
    }

    /// Paint a tapered stroke (grass blade, needle, twig) seen from above.
    /// Coordinates in pixels; wraps around the edges. Height adds on top.
    #[allow(clippy::too_many_arguments)]
    fn stroke(&mut self, x0: f32, y0: f32, dir: f32, len: f32, width: f32, height: f32, col: [f32; 3], rough: f32) {
        let (dx, dy) = (dir.cos(), dir.sin());
        let steps = (len * 1.5).ceil().max(2.0) as i32;
        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            let w = (width * (1.0 - t * 0.85)).max(0.5);
            let (cx, cy) = (x0 + dx * len * t, y0 + dy * len * t);
            let r = w.ceil() as i64;
            for oy in -r..=r {
                for ox in -r..=r {
                    let d = ((ox as f32).powi(2) + (oy as f32).powi(2)).sqrt();
                    if d > w {
                        continue;
                    }
                    let k = self.idx(cx as i64 + ox, cy as i64 + oy);
                    // Blades rise toward their tips; the top-most wins.
                    let hh = height * (0.35 + 0.65 * t) * (1.0 - 0.5 * d / w);
                    if hh > self.h[k] {
                        self.h[k] = hh;
                        let shade = 0.85 + 0.3 * t;
                        self.c[k] = [col[0] * shade, col[1] * shade, col[2] * shade];
                        self.r[k] = rough;
                    }
                }
            }
        }
    }

    /// Paint an ellipse blob (leaf, pebble) with a domed height profile.
    #[allow(clippy::too_many_arguments)]
    fn blob(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, rot: f32, height: f32, col: [f32; 3], rough: f32, dome: f32) {
        let r = rx.max(ry).ceil() as i64 + 1;
        let (c, s) = (rot.cos(), rot.sin());
        for oy in -r..=r {
            for ox in -r..=r {
                let (px, py) = (ox as f32 + cx.fract(), oy as f32 + cy.fract());
                let (lx, ly) = ((px * c + py * s) / rx, (-px * s + py * c) / ry);
                let d2 = lx * lx + ly * ly;
                if d2 > 1.0 {
                    continue;
                }
                let k = self.idx(cx as i64 + ox, cy as i64 + oy);
                let hh = height * (1.0 - dome + dome * (1.0 - d2).sqrt());
                if hh > self.h[k] {
                    self.h[k] = hh;
                    let edge = 1.0 - 0.25 * d2;
                    self.c[k] = [col[0] * edge, col[1] * edge, col[2] * edge];
                    self.r[k] = rough;
                }
            }
        }
    }
}

fn lin(hex: u32) -> [f32; 3] {
    let c = crate::mesh::rgb(hex);
    [c[0], c[1], c[2]]
}

fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn mul3(a: [f32; 3], k: f32) -> [f32; 3] {
    [a[0] * k, a[1] * k, a[2] * k]
}

/// Tiny deterministic RNG for painting.
struct Rng(u64);
impl Rng {
    fn f(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(1);
        crate::hash01(self.0, 0xC0FFEE)
    }
    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f()
    }
}

/// Synthesize one layer at `size` x `size` (power of two recommended).
pub fn layer(g: Ground, size: u32) -> LayerTexture {
    let n = size as usize;
    let mut cv = Canvas::new(n);
    let px_per_m = n as f32 / world_size_m(g);
    let seed = g as u64 * 1000 + 7;
    let mut rng = Rng(seed * 7919);
    // Periods for noise: `p` cells across the texture => feature ~ size/p.
    let per = |cells: i64| cells;
    match g {
        Ground::Grass | Ground::DryGrass => {
            let dry = g == Ground::DryGrass;
            let soil = if dry { lin(0xa08a5c) } else { lin(0x587030) };
            cv.fill(|s, t| {
                let clump = fbm(s * 6.0, t * 6.0, seed, 4, per(6));
                let h = 0.12 * clump;
                let c = mix3(mul3(soil, 0.85), soil, clump);
                (h, c, 0.95)
            });
            let palette: Vec<[f32; 3]> = if dry {
                [0xc9b06a, 0xb59a55, 0xd8c487, 0xa88d4c, 0x9c9a58, 0xe0cf98].iter().map(|&h| lin(h)).collect()
            } else {
                [0x6fa23b, 0x5d9132, 0x82b247, 0x4f7f2a, 0x93b850, 0x6a8f35].iter().map(|&h| lin(h)).collect()
            };
            let count = (n * n) as f32 * if dry { 0.2 } else { 0.28 };
            for _ in 0..count as usize {
                let (x, y) = (rng.f() * n as f32, rng.f() * n as f32);
                // Clumpy: skip some strokes where the clump noise is low.
                let cl = fbm(x as f64 / n as f64 * 6.0, y as f64 / n as f64 * 6.0, seed, 4, 6);
                if rng.f() > 0.85 + 0.3 * cl {
                    continue;
                }
                let len = rng.range(0.03, if dry { 0.10 } else { 0.07 }) * px_per_m;
                let col = palette[(rng.f() * palette.len() as f32) as usize % palette.len()];
                let col = mul3(col, 0.93 + 0.12 * cl);
                cv.stroke(x, y, rng.f() * std::f32::consts::TAU, len, rng.range(1.0, 2.2), rng.range(0.4, 1.0), col, 0.8);
            }
        }
        Ground::Moss => {
            // Soft cushions at two scales, no outlines, fuzzy with tiny fronds.
            cv.fill(|s, t| {
                let w = fbm(s * 5.0, t * 5.0, seed + 7, 3, 5) as f64 * 0.12;
                let (d1, _, id) = worley(s * 9.0 + w, t * 9.0 - w, seed, 9, 1.0);
                let (e1, _, _) = worley(s * 23.0 - w, t * 23.0 + w, seed + 1, 23, 1.0);
                let big = (1.0 - d1 * 1.1).clamp(0.0, 1.0).powf(0.7);
                let small = (1.0 - e1 * 1.2).clamp(0.0, 1.0);
                let fine = fbm(s * 90.0, t * 90.0, seed + 2, 3, 90);
                let h = 0.55 * big + 0.25 * small + 0.2 * fine;
                let _ = id;
                let hue = fbm(s * 7.0, t * 7.0, seed + 5, 3, 7);
                let base = mix3(lin(0x46692a), lin(0x8aa43c), 0.45 * hue + 0.35 * h + 0.2 * fine);
                let c = mix3(mul3(lin(0x3b4224), 0.9), base, smoothstep(0.1, 0.45, h));
                (h, c, 0.92)
            });
            for _ in 0..(n * n / 30) {
                let (x, y) = (rng.f() * n as f32, rng.f() * n as f32);
                let col = mul3(lin(if rng.f() < 0.5 { 0x7f9c3a } else { 0x5c7f2c }), rng.range(0.8, 1.2));
                let k = cv.idx(x as i64, y as i64);
                let base_h = cv.h[k];
                cv.stroke(x, y, rng.f() * 6.283, rng.range(0.006, 0.014) * px_per_m, 0.8, base_h + 0.08, col, 0.9);
            }
            for _ in 0..(n * n / 400) {
                // Sporophyte specks.
                let (x, y) = (rng.f() * n as f32, rng.f() * n as f32);
                cv.blob(x, y, 1.2, 1.2, 0.0, 1.0, lin(0xb5a24a), 0.8, 0.5);
            }
        }
        Ground::ForestFloor => {
            cv.fill(|s, t| {
                let f = fbm(s * 10.0, t * 10.0, seed, 4, 10);
                (0.1 * f, mix3(lin(0x2f2418), lin(0x4a3a26), f), 0.95)
            });
            let leaf_cols: Vec<[f32; 3]> = [0x8a5a2b, 0x6b4a2a, 0xa06a2c, 0x7a6a35, 0x5a4024, 0xb07a3a, 0x6c5a30].iter().map(|&h| lin(h)).collect();
            for i in 0..(n * n / 45) {
                let (x, y) = (rng.f() * n as f32, rng.f() * n as f32);
                let hgt = 0.2 + 0.8 * (i as f32 / (n * n / 45) as f32); // later = on top
                if rng.f() < 0.4 {
                    // Needles / twigs.
                    let len = rng.range(0.03, 0.09) * px_per_m;
                    cv.stroke(x, y, rng.f() * 6.283, len, 0.8, hgt, mul3(lin(0x7a5a34), rng.range(0.6, 1.1)), 0.8);
                } else {
                    let r = rng.range(0.012, 0.03) * px_per_m;
                    let col = mul3(mix3(leaf_cols[(rng.f() * 7.0) as usize % 7], lin(0x5a4a30), 0.35), rng.range(0.75, 1.1));
                    cv.blob(x, y, r, r * rng.range(0.45, 0.7), rng.f() * 6.283, hgt, col, 0.75, 0.25);
                }
            }
        }
        Ground::Dirt | Ground::Mud => {
            let mud = g == Ground::Mud;
            // No lattice features: a Worley grid of equal stones or an even
            // crack net reads as a repeating stud pattern once normal-mapped,
            // whatever the tiling. Undulation + patchy, warped cracks here;
            // pebbles are painted below with heavy-tailed sizes and clustering.
            cv.fill(|s, t| {
                let f = fbm(s * 8.0, t * 8.0, seed, 5, 8);
                let swell = fbm(s * 3.0, t * 3.0, seed + 6, 3, 3);
                let wx = (fbm(s * 5.0, t * 5.0, seed + 7, 3, 5) - 0.5) as f64 * 0.25;
                let wy = (fbm(s * 5.0, t * 5.0, seed + 8, 3, 5) - 0.5) as f64 * 0.25;
                let (c1, c2, _) = worley(s * 7.0 + wx, t * 7.0 + wy, seed + 4, 7, 1.0);
                // Cracks only where the ground dried out (patchy), width varies.
                let dried = smoothstep(0.5, 0.62, fbm(s * 4.0, t * 4.0, seed + 9, 3, 4));
                let cw = 0.02 + 0.05 * fbm(s * 20.0, t * 20.0, seed + 10, 2, 20);
                let crack = (1.0 - smoothstep(0.0, cw, c2 - c1)) * dried;
                let fine = fbm(s * 60.0, t * 60.0, seed + 3, 3, 60);
                let mut h = 0.35 * f + 0.35 * swell + 0.12 * fine - 0.22 * crack * (!mud) as u8 as f32;
                let (lo, hi) = if mud { (lin(0x2e261e), lin(0x5a4a3a)) } else { (lin(0x5e4630), lin(0x9a7a58)) };
                let mut c = mix3(lo, hi, 0.6 * f + 0.25 * swell + 0.15 * fine);
                c = mix3(c, mul3(lo, 0.75), crack * 0.6);
                let mut r = 0.92;
                if mud {
                    // Puddles: flat, dark, glossy in the low spots.
                    let p = 1.0 - smoothstep(0.26, 0.31, f);
                    h = h * (1.0 - p) + 0.16 * p;
                    c = mix3(c, mul3(lo, 0.6), p);
                    r = 0.85 - 0.75 * p;
                }
                (h, c, r)
            });
            // Pebbles: random positions, heavy-tailed sizes (mostly grit, a few
            // stones), clustered by noise, half sunk so they read as embedded.
            let cols: Vec<[f32; 3]> = [0x8f8678, 0x7a6e60, 0x9c9384, 0x6a5e50, 0xa89a86].iter().map(|&h| lin(h)).collect();
            let count = (n * n) / if mud { 900 } else { 260 };
            for _ in 0..count {
                let (x, y) = (rng.f() * n as f32, rng.f() * n as f32);
                let cl = fbm(x as f64 / n as f64 * 5.0, y as f64 / n as f64 * 5.0, seed + 12, 3, 5);
                if rng.f() > smoothstep(0.35, 0.7, cl) {
                    continue;
                }
                let r = (0.004 + 0.03 * rng.f().powi(3)) * px_per_m;
                let k = cv.idx(x as i64, y as i64);
                let base_h = cv.h[k];
                let col = mul3(cols[(rng.f() * 5.0) as usize % 5], rng.range(0.75, 1.1));
                cv.blob(x, y, r, r * rng.range(0.55, 0.95), rng.f() * 6.283, base_h + 0.08 + r / px_per_m * 4.0, col, rng.range(0.6, 0.85), 0.6);
            }
        }
        Ground::Farmland => {
            let rows = 9.0; // 6 m tile -> ~0.67 m spacing
            cv.fill(|s, t| {
                let warp = (fbm(s * 3.0, t * 3.0, seed, 3, 3) - 0.5) as f64 * 0.04;
                let ph = ((s + warp) * rows).fract() as f32;
                let ridge = (0.5 - 0.5 * (ph * std::f32::consts::TAU).cos()).powf(0.8);
                let clod = (1.0 - worley(s * 120.0, t * 120.0, seed + 2, 120, 1.0).0 * 1.8).clamp(0.0, 1.0);
                let f = fbm(s * 30.0, t * 30.0, seed + 5, 3, 30);
                let h = 0.7 * ridge + 0.2 * clod + 0.1 * f;
                let damp = 1.0 - ridge;
                let c = mix3(lin(0x8e7050), lin(0x4f3a28), damp * 0.8 + 0.2 * (1.0 - f));
                (h, c, 0.95 - 0.1 * damp)
            });
            // Young crop along the ridges.
            for _ in 0..(n * n / 90) {
                let row = (rng.f() * rows as f32).floor();
                let x = (row + 0.5 + rng.range(-0.06, 0.06)) / rows as f32 * n as f32;
                let y = rng.f() * n as f32;
                if fbm(x as f64 / n as f64 * 4.0, y as f64 / n as f64 * 4.0, seed + 9, 3, 4) < 0.45 {
                    continue;
                }
                let col = mul3(lin(0x6f9a3a), rng.range(0.7, 1.1));
                cv.stroke(x, y, rng.f() * 6.283, rng.range(0.02, 0.05) * px_per_m, 1.4, 1.1, col, 0.8);
            }
        }
        Ground::Sand => {
            let ripples = 34.0;
            cv.fill(|s, t| {
                let warp = (fbm(s * 4.0, t * 4.0, seed, 4, 4) - 0.5) as f64 * 0.09;
                let bend = (fbm(s * 2.0, t * 2.0, seed + 1, 2, 2) - 0.5) as f64 * 0.05;
                let ph = ((t + warp + bend * (s * 6.283).sin()) * ripples).fract() as f32;
                // Asymmetric ripple: gentle stoss side, steep lee side.
                let prof = if ph < 0.75 { ph / 0.75 } else { (1.0 - ph) / 0.25 };
                let prof = smoothstep(0.0, 1.0, prof);
                let grain = value(s * 400.0, t * 400.0, seed + 2, 400);
                let f = fbm(s * 12.0, t * 12.0, seed + 3, 3, 12);
                let h = 0.55 * prof + 0.12 * grain + 0.33 * f;
                // Heavy minerals collect in the troughs.
                let c = mix3(mix3(lin(0xb89c6c), lin(0xead6a6), prof * 0.7 + 0.3 * f), lin(0x9a8460), (1.0 - prof) * 0.25 * (grain > 0.7) as u8 as f32);
                (h, mul3(c, 0.95 + 0.1 * grain), 0.97)
            });
        }
        Ground::Gravel => {
            cv.fill(|s, t| {
                let f = fbm(s * 10.0, t * 10.0, seed, 3, 10);
                (0.1 * f, mix3(lin(0x5e5850), lin(0x7a746a), f), 0.95)
            });
            let cols: Vec<[f32; 3]> = [0x9a958b, 0x857f75, 0xaaa498, 0x77706a, 0x948a78, 0xb0a89a, 0x6e6a64].iter().map(|&h| lin(h)).collect();
            for i in 0..(n * n / 45) {
                let (x, y) = (rng.f() * n as f32, rng.f() * n as f32);
                let r = rng.range(0.008, 0.02) * px_per_m;
                let col = mul3(cols[(rng.f() * 7.0) as usize % 7], rng.range(0.8, 1.1));
                let hgt = 0.3 + 0.7 * (i as f32 / (n * n / 45) as f32);
                cv.blob(x, y, r, r * rng.range(0.6, 0.95), rng.f() * 6.283, hgt, col, rng.range(0.6, 0.85), 0.8);
            }
        }
        Ground::Rock => {
            cv.fill(|s, t| {
                let warp = fbm(s * 3.0, t * 3.0, seed + 9, 4, 3) as f64;
                let warp2 = fbm(s * 7.0, t * 7.0, seed + 10, 3, 7) as f64;
                let strata = ((t + 0.18 * warp) * 9.0 * 6.283).sin() as f32 * 0.5 + 0.5;
                let f = fbm(s * 5.0, t * 5.0, seed, 6, 5);
                let (d1, d2, id) = worley(s * 4.0 + 0.6 * warp2, t * 4.0 + 0.6 * warp, seed + 1, 4, 1.0);
                // Cracks only in places, so faces read as broken slabs, not tiles.
                let mask = smoothstep(0.45, 0.6, fbm(s * 6.0, t * 6.0, seed + 11, 3, 6));
                let crack = (1.0 - smoothstep(0.0, 0.03, d2 - d1)) * mask;
                let facet = crate::hash01(id, 5);
                let fine = fbm(s * 60.0, t * 60.0, seed + 2, 3, 60);
                let pits = smoothstep(0.62, 0.75, fbm(s * 30.0, t * 30.0, seed + 3, 2, 30));
                let h = 0.5 * f + 0.12 * strata + 0.1 * facet + 0.2 * fine - 0.4 * crack - 0.1 * pits;
                let mut c = mix3(lin(0x6c675f), lin(0xa8a196), 0.65 * f + 0.2 * facet + 0.15 * fine);
                c = mix3(c, lin(0x8d806e), strata * 0.22);
                c = mix3(c, mul3(lin(0x3a3632), 0.8), crack * 0.85 + pits * 0.3);
                // Lichen: pale green-grey and orange spots.
                let ln = fbm(s * 22.0, t * 22.0, seed + 4, 4, 22);
                let li = smoothstep(0.66, 0.74, ln);
                c = mix3(c, if crate::hash01(id, 9) > 0.75 { lin(0xb88a3a) } else { lin(0xa3a88a) }, li * 0.65);
                (h, c, 0.78 + 0.15 * li)
            });
        }
        Ground::Snow => {
            cv.fill(|s, t| {
                let f = fbm(s * 4.0, t * 4.0, seed, 5, 4);
                let wind = ((t + 0.08 * f as f64) * 18.0 * 6.283).sin() as f32 * 0.5 + 0.5;
                let fine = value(s * 300.0, t * 300.0, seed + 2, 300);
                let h = 0.85 * f + 0.12 * wind + 0.03 * fine;
                let c = mix3(lin(0xdfe8f2), lin(0xffffff), 0.5 + 0.5 * h);
                // Sparkle: a few very smooth crystals.
                let r = if fine > 0.93 { 0.25 } else { 0.62 };
                (h, c, r)
            });
        }
        Ground::Paving => {
            // Running-bond slabs, 60 x 40 cm, worn and stained.
            let (sw, sh) = (0.6 / 4.8, 0.4 / 4.8);
            cv.fill(|s, t| {
                let row = (t / sh).floor();
                let off = if row as i64 % 2 == 0 { 0.0 } else { 0.5 };
                let col = (s / sw + off).floor();
                let (fx, fy) = ((s / sw + off).fract() as f32, (t / sh).fract() as f32);
                let joint = 0.035f32;
                let edge = smoothstep(0.0, joint, fx.min(1.0 - fx) * (sw / sh) as f32).min(smoothstep(0.0, joint, fy.min(1.0 - fy)));
                let id = ((col as i64 as u64) << 20) ^ row as i64 as u64;
                let tone = crate::hash01(id, 1);
                let f = fbm(s * 40.0, t * 40.0, seed, 4, 40);
                let stain = smoothstep(0.55, 0.8, fbm(s * 5.0, t * 5.0, seed + 3, 3, 5));
                let h = edge * (0.75 + 0.1 * f + 0.1 * tone);
                let base = mix3(lin(0x9c988f), lin(0xbab5aa), tone);
                let mut c = mix3(mul3(lin(0x4a4740), 0.9), mul3(base, 0.9 + 0.15 * f), edge);
                c = mix3(c, lin(0x6e6a60), stain * 0.35);
                // Weeds in the joints.
                let weed = (1.0 - edge) * smoothstep(0.6, 0.75, fbm(s * 18.0, t * 18.0, seed + 5, 3, 18));
                c = mix3(c, lin(0x5d7f30), weed);
                (h + weed * 0.3, c, 0.75 + 0.15 * (1.0 - edge))
            });
        }
    }
    finish(g, cv)
}

/// Derive normals/AO, match the mean color, pack to RGBA8.
fn finish(g: Ground, cv: Canvas) -> LayerTexture {
    let n = cv.n;
    // Normalize height to [0,1].
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for &h in &cv.h {
        lo = lo.min(h);
        hi = hi.max(h);
    }
    let span = (hi - lo).max(1e-5);
    let h: Vec<f32> = cv.h.iter().map(|x| (x - lo) / span).collect();
    // Height relief in pixels: how many px of depth the full range spans.
    let relief = bumpiness(g) * n as f32 / 256.0 * 3.0;
    // Blurred height for cavity AO.
    let blur = box_blur(&h, n, (n / 64).max(2));
    // Mean color match.
    let mut mean = [0.0f64; 3];
    for c in &cv.c {
        for k in 0..3 {
            mean[k] += c[k] as f64;
        }
    }
    let target = g.mean_color();
    let gain: Vec<f32> = (0..3).map(|k| (target[k] as f64 / (mean[k] / (n * n) as f64).max(1e-5)) as f32).collect();
    let mut ah = Vec::with_capacity(n * n * 4);
    let mut nr = Vec::with_capacity(n * n * 4);
    let at = |x: i64, y: i64| h[(y.rem_euclid(n as i64) as usize) * n + x.rem_euclid(n as i64) as usize];
    for y in 0..n as i64 {
        for x in 0..n as i64 {
            let k = y as usize * n + x as usize;
            let dx = (at(x + 1, y) - at(x - 1, y)) * 0.5 * relief;
            let dy = (at(x, y + 1) - at(x, y - 1)) * 0.5 * relief;
            let len = (dx * dx + dy * dy + 1.0).sqrt();
            let (nx, ny) = (-dx / len, -dy / len);
            let ao = (1.0 - (blur[k] - h[k]).max(0.0) * 2.2).clamp(0.35, 1.0);
            let c = cv.c[k];
            for ch in 0..3 {
                ah.push(to_srgb8(c[ch] * gain[ch]));
            }
            ah.push((h[k] * 255.0 + 0.5) as u8);
            nr.push(((nx * 0.5 + 0.5) * 255.0 + 0.5) as u8);
            nr.push(((ny * 0.5 + 0.5) * 255.0 + 0.5) as u8);
            nr.push((cv.r[k].clamp(0.02, 1.0) * 255.0 + 0.5) as u8);
            nr.push((ao * 255.0 + 0.5) as u8);
        }
    }
    LayerTexture { size: n as u32, albedo_height: ah, normal_rough_ao: nr }
}

fn box_blur(src: &[f32], n: usize, r: usize) -> Vec<f32> {
    let pass = |src: &[f32], horiz: bool| -> Vec<f32> {
        let mut out = vec![0.0; n * n];
        let r = r as i64;
        for y in 0..n {
            for x in 0..n {
                let mut s = 0.0;
                for o in -r..=r {
                    let (xx, yy) = if horiz { ((x as i64 + o).rem_euclid(n as i64) as usize, y) } else { (x, (y as i64 + o).rem_euclid(n as i64) as usize) };
                    s += src[yy * n + xx];
                }
                out[y * n + x] = s / (2 * r + 1) as f32;
            }
        }
        out
    };
    let a = pass(src, true);
    pass(&a, false)
}

fn to_srgb8(v: f32) -> u8 {
    let v = v.clamp(0.0, 1.0);
    let s = if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 };
    (s * 255.0 + 0.5) as u8
}

fn from_srgb8(v: u8) -> f32 {
    let v = v as f32 / 255.0;
    if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}

/// Full mip chain (level 0 first) of a square RGBA8 image, box filtered.
/// `srgb_rgb`: filter RGB in linear space (albedo textures).
pub fn mip_chain(rgba: &[u8], size: u32, srgb_rgb: bool) -> (Vec<u8>, u32) {
    let mut out = rgba.to_vec();
    let mut cur = rgba.to_vec();
    let mut s = size as usize;
    let mut levels = 1;
    while s > 1 {
        let h = s / 2;
        let mut next = vec![0u8; h * h * 4];
        for y in 0..h {
            for x in 0..h {
                for ch in 0..4 {
                    let px = |xx: usize, yy: usize| cur[(yy * s + xx) * 4 + ch];
                    let q = [px(2 * x, 2 * y), px(2 * x + 1, 2 * y), px(2 * x, 2 * y + 1), px(2 * x + 1, 2 * y + 1)];
                    next[(y * h + x) * 4 + ch] = if srgb_rgb && ch < 3 {
                        to_srgb8(q.iter().map(|&v| from_srgb8(v)).sum::<f32>() / 4.0)
                    } else {
                        ((q.iter().map(|&v| v as u32).sum::<u32>() + 2) / 4) as u8
                    };
                }
            }
        }
        out.extend_from_slice(&next);
        cur = next;
        s = h;
        levels += 1;
    }
    (out, levels)
}

/// All layers, generated in parallel. Index = `Ground as usize`.
pub fn all_layers(size: u32) -> Vec<LayerTexture> {
    std::thread::scope(|sc| {
        let handles: Vec<_> = Ground::ALL.iter().map(|&g| sc.spawn(move || layer(g, size))).collect();
        handles.into_iter().map(|h| h.join().expect("texture thread")).collect()
    })
}

/// Tileable macro-variation texture (linear RGBA8): R = broad fBm,
/// G = medium fBm, B = fine fBm, A = cellular patches. Sampled at several
/// world scales in the shader to break up tiling.
pub fn macro_texture(size: u32) -> Vec<u8> {
    let n = size as usize;
    let mut out = Vec::with_capacity(n * n * 4);
    for y in 0..n {
        for x in 0..n {
            let (s, t) = (x as f64 / n as f64, y as f64 / n as f64);
            let r = fbm(s * 4.0, t * 4.0, 1, 5, 4);
            let g = fbm(s * 8.0, t * 8.0, 2, 4, 8);
            let b = fbm(s * 16.0, t * 16.0, 3, 4, 16);
            let (d1, _, id) = worley(s * 6.0, t * 6.0, 4, 6, 1.0);
            let a = crate::hash01(id, 2) * (1.0 - 0.3 * d1);
            for v in [r, g, b, a] {
                out.push((v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
            }
        }
    }
    out
}

/// Per-layer constants the shader needs, packed as 3 x vec4 (one value per layer).
pub fn layer_params() -> ([f32; LAYERS], [f32; LAYERS]) {
    let mut size = [0.0; LAYERS];
    let mut bump = [0.0; LAYERS];
    for g in Ground::ALL {
        size[g as usize] = world_size_m(g);
        bump[g as usize] = bumpiness(g);
    }
    (size, bump)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layers_tile_and_match_palette() {
        for g in [Ground::Grass, Ground::Sand, Ground::Paving, Ground::Rock] {
            let t = layer(g, 64);
            assert_eq!(t.albedo_height.len(), 64 * 64 * 4);
            assert_eq!(t.normal_rough_ao.len(), 64 * 64 * 4);
            // Mean albedo within a few % of the palette color.
            let mut m = [0.0f32; 3];
            for p in t.albedo_height.chunks_exact(4) {
                for k in 0..3 {
                    m[k] += from_srgb8(p[k]);
                }
            }
            let want = g.mean_color();
            for k in 0..3 {
                let got = m[k] / (64.0 * 64.0);
                assert!((got - want[k]).abs() < 0.03 + 0.1 * want[k], "{g:?} ch{k}: {got} vs {}", want[k]);
            }
        }
    }

    #[test]
    fn mips_go_to_one_pixel() {
        let img = vec![128u8; 16 * 16 * 4];
        let (all, levels) = mip_chain(&img, 16, true);
        assert_eq!(levels, 5);
        assert_eq!(all.len(), (256 + 64 + 16 + 4 + 1) * 4);
    }

    /// `cargo test -p ed_world dump_ground_textures -- --ignored` writes PPMs for eyeballing.
    #[test]
    #[ignore]
    fn dump_ground_textures() {
        let dir = std::env::temp_dir().join("ground_tex");
        std::fs::create_dir_all(&dir).unwrap();
        for (i, t) in all_layers(512).iter().enumerate() {
            for (tag, data, ch) in [("albedo", &t.albedo_height, 0usize), ("normal", &t.normal_rough_ao, 0)] {
                let mut ppm = format!("P6 {} {} 255\n", t.size, t.size).into_bytes();
                for p in data.chunks_exact(4) {
                    ppm.extend_from_slice(&p[ch..ch + 3]);
                }
                std::fs::write(dir.join(format!("{i:02}_{:?}_{tag}.ppm", Ground::ALL[i])), ppm).unwrap();
            }
        }
        eprintln!("wrote {}", dir.display());
    }
}
