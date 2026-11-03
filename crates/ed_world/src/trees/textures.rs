//! Procedural foliage textures: a leaf-cluster atlas (alpha-tested cards),
//! a fissured bark tile and a ringed palm-trunk tile. Engine-free RGBA8
//! sRGB images with a full mip chain, so the game just uploads them.
//!
//! Atlas convention: 4x4 cells of `CELL` px. Inside a cell, v = 0 is the
//! *bottom* (where the twig attaches) and v = 1 the tip. Leaves are painted
//! in a neutral mid green; vertex colors tint them per species.

use super::Rng;
use glam::Vec2;
use std::f32::consts::{PI, TAU};

pub const ATLAS_COLS: u32 = 4;
pub const ATLAS_ROWS: u32 = 4;
pub const CELL: u32 = 512;

/// Atlas cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LeafCell {
    /// Lobed oak leaves on a twig.
    Oak = 0,
    /// Palmate maple / plane leaves.
    Palmate = 1,
    /// Small oval leaves (birch, boxwood, generic shrub).
    Oval = 2,
    /// Narrow lanceolate leaves (olive, oleander).
    Lance = 3,
    /// Flat spruce/fir needle spray.
    NeedleSpray = 4,
    /// Long pine needle tufts.
    PineTuft = 5,
    /// One palm frond (rachis + leaflets), mapped along a ribbon.
    PalmFrond = 6,
    /// One fern frond, mapped along a ribbon.
    FernFrond = 7,
    /// A few grass blades, mapped along a ribbon.
    Grass = 8,
    /// One banana leaf (midrib, parallel veins, tears).
    Banana = 9,
    /// Bipinnate acacia leaflets.
    Acacia = 10,
    /// Leaves with blossom clusters (pink, tinted by vertex color).
    Flowers = 11,
    /// Solid opaque bark color (far-LOD trunks in the leaf material).
    Solid = 12,
    /// Scale-like juniper / cypress sprays.
    Scale = 13,
    /// Dead, dry palm frond (date palm skirt).
    DeadFrond = 14,
    /// Silvery olive leaves (narrow, two-tone).
    Olive = 15,
}

/// Atlas UV for cell-local (u, v) in [0,1], v = 0 at the stem end.
pub fn atlas_uv(cell: LeafCell, u: f32, v: f32) -> [f32; 2] {
    let i = cell as u32;
    let (cx, cy) = ((i % ATLAS_COLS) as f32, (i / ATLAS_COLS) as f32);
    // Half-texel inset keeps bilinear taps inside the cell.
    let pad = 2.0 / CELL as f32;
    let u = pad + u.clamp(0.0, 1.0) * (1.0 - 2.0 * pad);
    let v = pad + v.clamp(0.0, 1.0) * (1.0 - 2.0 * pad);
    [(cx + u) / ATLAS_COLS as f32, (cy + 1.0 - v) / ATLAS_ROWS as f32]
}

/// RGBA8 sRGB image, row 0 at the top, with mip levels concatenated after
/// level 0 (each half the size of the previous, down to 1x1).
#[derive(Debug, Clone)]
pub struct Rgba8Image {
    pub width: u32,
    pub height: u32,
    pub mip_levels: u32,
    pub data: Vec<u8>,
}

/// Neutral leaf green the atlas is painted around (sRGB 0..1).
pub const LEAF_BASE: [f32; 3] = [0.42, 0.56, 0.25];

// ---------------------------------------------------------------- canvas --

struct Canvas {
    w: usize,
    h: usize,
    px: Vec<[f32; 4]>,
}

impl Canvas {
    fn new(w: usize, h: usize) -> Self {
        Self { w, h, px: vec![[0.0; 4]; w * h] }
    }

    /// Source-over blend of a (non-premultiplied) color with coverage `a`.
    fn blend(&mut self, x: i64, y: i64, c: [f32; 3], a: f32) {
        if x < 0 || y < 0 || x >= self.w as i64 || y >= self.h as i64 || a <= 0.0 {
            return;
        }
        let p = &mut self.px[y as usize * self.w + x as usize];
        let a = a.min(1.0);
        let out_a = a + p[3] * (1.0 - a);
        if out_a > 0.0 {
            for k in 0..3 {
                p[k] = (c[k] * a + p[k] * p[3] * (1.0 - a)) / out_a;
            }
        }
        p[3] = out_a;
    }
}

/// A rectangle (cell) on the canvas, in px.
#[derive(Clone, Copy)]
struct Cell {
    x0: f32,
    y0: f32,
    s: f32,
}

impl Cell {
    /// Cell-normalized (u right, v up from the bottom) -> canvas px.
    fn p(&self, u: f32, v: f32) -> Vec2 {
        Vec2::new(self.x0 + u * self.s, self.y0 + (1.0 - v) * self.s)
    }
}

fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn mul3(c: [f32; 3], k: f32) -> [f32; 3] {
    [c[0] * k, c[1] * k, c[2] * k]
}

fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// Tapered capsule stroke from `a` to `b` (twigs, needles, stems).
fn stroke(cv: &mut Canvas, a: Vec2, b: Vec2, w0: f32, w1: f32, c0: [f32; 3], c1: [f32; 3]) {
    let r = w0.max(w1) * 0.5 + 1.5;
    let (minx, maxx) = (a.x.min(b.x) - r, a.x.max(b.x) + r);
    let (miny, maxy) = (a.y.min(b.y) - r, a.y.max(b.y) + r);
    let ab = b - a;
    let len2 = ab.length_squared().max(1e-6);
    for y in miny.floor() as i64..=maxy.ceil() as i64 {
        for x in minx.floor() as i64..=maxx.ceil() as i64 {
            let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
            let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
            let d = (p - (a + ab * t)).length();
            let hw = (w0 + (w1 - w0) * t) * 0.5;
            let cov = (hw - d + 0.5).clamp(0.0, 1.0);
            if cov > 0.0 {
                // Round shading across the stroke.
                let across = (d / hw.max(0.5)).min(1.0);
                let c = mul3(mix3(c0, c1, t), 1.0 - 0.35 * across * across);
                cv.blend(x, y, c, cov);
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Shape {
    Oval,
    Lance,
    Oak,
    Palmate,
    Banana,
    Petal,
}

/// Half-width (0..1 of the leaf's width) at x in [0,1] along the leaf.
fn half_width(shape: Shape, x: f32) -> f32 {
    match shape {
        Shape::Oval => (PI * x.powf(0.85)).sin().max(0.0).powf(0.8),
        Shape::Lance => (PI * x.powf(1.1)).sin().max(0.0).powf(1.1),
        Shape::Oak => {
            let lobes = 0.62 + 0.38 * (x * PI * 4.5).sin().abs().powf(0.6);
            (PI * x.powf(0.9)).sin().max(0.0).powf(0.7) * lobes
        }
        Shape::Banana => (x * 10.0).min(1.0).min((1.0 - x) * 4.0).max(0.0).powf(0.5),
        Shape::Petal => (PI * x).sin().max(0.0).powf(0.6),
        Shape::Palmate => 0.0, // polar, handled separately
    }
}

/// Paint one leaf with its stalk at `base`, pointing along `angle` (radians,
/// 0 = up the cell). Shading: lighter toward the tip, darker rim, light
/// midrib and side veins, one half slightly darker (folded leaf).
#[allow(clippy::too_many_arguments)]
fn leaf(cv: &mut Canvas, base: Vec2, angle: f32, len: f32, width: f32, shape: Shape, color: [f32; 3], shade: f32) {
    let dir = Vec2::new(angle.sin(), -angle.cos()); // canvas y is down
    let side = Vec2::new(-dir.y, dir.x);
    let r = len.max(width) + 2.0;
    let (cx, cy) = (base.x + dir.x * len * 0.5, base.y + dir.y * len * 0.5);
    let vein = mul3(mix3(color, [0.85, 0.9, 0.55], 0.45), 1.1);
    for y in (cy - r).floor() as i64..=(cy + r).ceil() as i64 {
        for x in (cx - r).floor() as i64..=(cx + r).ceil() as i64 {
            let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5) - base;
            let (along, across) = (p.dot(dir), p.dot(side));
            let (cov, edge, xn, yn) = if let Shape::Palmate = shape {
                // Polar: five pointed lobes around the stalk end.
                let pr = p.length();
                let a = across.atan2(along);
                if a.abs() > PI * 0.82 {
                    continue;
                }
                let lobe = (a * 2.5).cos().abs();
                let rmax = len * (0.42 + 0.58 * lobe.powf(0.7)) * (1.0 - 0.25 * (a.abs() / PI));
                let cov = (rmax - pr + 0.7).clamp(0.0, 1.0);
                (cov, (pr / rmax).clamp(0.0, 1.0), (pr / len).clamp(0.0, 1.0), a / PI)
            } else {
                let xn = along / len;
                if !(0.0..=1.0).contains(&xn) {
                    continue;
                }
                let hw = half_width(shape, xn) * width * 0.5;
                if hw <= 0.0 {
                    continue;
                }
                let cov = (hw - across.abs() + 0.6).clamp(0.0, 1.0);
                (cov, (across.abs() / hw).clamp(0.0, 1.0), xn, across / hw)
            };
            if cov <= 0.0 {
                continue;
            }
            // Base shading.
            let mut c = mul3(color, (0.78 + 0.32 * xn) * shade);
            if yn < 0.0 {
                c = mul3(c, 0.9);
            }
            c = mul3(c, 1.0 - 0.3 * edge.powi(4));
            // Midrib and side veins.
            let midrib = match shape {
                Shape::Palmate => {
                    let a = yn * PI;
                    let d = ((a * 2.5 / PI).round() * PI / 2.5 - a).abs() * xn * len;
                    (1.0 - d / 1.2).clamp(0.0, 1.0)
                }
                _ => (1.0 - (yn.abs() * width * 0.5) / (0.9 + width * 0.02)).clamp(0.0, 1.0),
            };
            let side_v = match shape {
                Shape::Palmate | Shape::Petal => 0.0,
                Shape::Banana => smooth(0.88, 1.0, 1.0 - (xn * 40.0 - yn.abs() * 6.0).fract().min(1.0 - (xn * 40.0 - yn.abs() * 6.0).fract()) * 2.0) * 0.5,
                _ => {
                    let f = (xn * 7.0 - yn.abs() * 1.6).fract();
                    smooth(0.85, 1.0, 1.0 - f.min(1.0 - f) * 2.0) * 0.35 * (1.0 - edge)
                }
            };
            c = mix3(c, vein, (midrib * 0.7 + side_v).min(0.8));
            cv.blend(x, y, c, cov);
        }
    }
}

/// Slight per-leaf color variation around `base`.
fn vary(rng: &mut Rng, base: [f32; 3], amount: f32) -> [f32; 3] {
    let k = 1.0 + (rng.f() - 0.5) * amount;
    let warm = (rng.f() - 0.5) * amount * 0.6;
    [base[0] * k * (1.0 + warm), base[1] * k, base[2] * k * (1.0 - warm)]
}

const TWIG: [f32; 3] = [0.36, 0.28, 0.2];

/// Branching twig cluster filled with leaves: the classic "leaf card".
#[allow(clippy::too_many_arguments)]
fn twig_cluster(cv: &mut Canvas, cell: Cell, rng: &mut Rng, shape: Shape, n: usize, len: f32, width: f32, color: [f32; 3]) {
    let s = cell.s;
    // Main twig with a few side twigs, fanning upward.
    let mut twigs: Vec<(Vec2, Vec2)> = Vec::new();
    let base = cell.p(0.5, 0.02);
    let top = cell.p(0.5 + (rng.f() - 0.5) * 0.15, 0.78);
    twigs.push((base, top));
    for k in 0..4 {
        let t = 0.25 + 0.17 * k as f32;
        let a = base + (top - base) * t;
        let side = if k % 2 == 0 { -1.0 } else { 1.0 };
        let end = a + Vec2::new(side * s * (0.22 + 0.08 * rng.f()), -s * (0.18 + 0.12 * rng.f()));
        twigs.push((a, end));
    }
    // Leaves along all twigs, back layer first (darker) for depth.
    let mut leaves = Vec::new();
    for i in 0..n {
        let (a, b) = twigs[i % twigs.len()];
        let t = 0.2 + 0.8 * rng.f();
        let at = a + (b - a) * t;
        let tw = (b - a).normalize();
        let twig_angle = tw.x.atan2(-tw.y);
        let side = if rng.f() < 0.5 { -1.0 } else { 1.0 };
        let angle = twig_angle + side * (0.35 + 0.75 * rng.f());
        let l = len * s * (0.7 + 0.45 * rng.f()) * (1.0 - 0.3 * (1.0 - t));
        leaves.push((at, angle, l, rng.f()));
    }
    for (a, b) in &twigs {
        stroke(cv, *a, *b, s * 0.018, s * 0.006, TWIG, mul3(TWIG, 1.2));
    }
    leaves.sort_by(|a, b| a.3.total_cmp(&b.3));
    for (k, (at, angle, l, _)) in leaves.iter().enumerate() {
        let depth = k as f32 / leaves.len() as f32;
        let shade = 0.62 + 0.42 * depth;
        let c = vary(rng, color, 0.22);
        // Short petiole, then the blade.
        let dir = Vec2::new(angle.sin(), -angle.cos());
        let tip = *at + dir * l * 0.12;
        stroke(cv, *at, tip, s * 0.006, s * 0.005, mul3(TWIG, 1.4), mul3(TWIG, 1.4));
        leaf(cv, tip, *angle, *l, *l * width, shape, c, shade);
    }
}

/// Spruce/fir spray: a flat twig with dense short needles both sides.
fn needle_spray(cv: &mut Canvas, cell: Cell, rng: &mut Rng, color: [f32; 3], scale_like: bool) {
    let s = cell.s;
    let mut twigs = vec![(cell.p(0.5, 0.02), cell.p(0.5, 0.95))];
    for k in 0..6 {
        let t = 0.18 + 0.12 * k as f32;
        let a = twigs[0].0 + (twigs[0].1 - twigs[0].0) * t;
        let side = if k % 2 == 0 { -1.0 } else { 1.0 };
        let l = s * (0.38 - 0.04 * k as f32) * (0.85 + 0.3 * rng.f());
        twigs.push((a, a + Vec2::new(side * l * 0.8, -l * 0.6)));
    }
    for (a, b) in &twigs {
        stroke(cv, *a, *b, s * 0.014, s * 0.006, TWIG, mul3(TWIG, 1.3));
    }
    for (ti, (a, b)) in twigs.iter().enumerate() {
        let len = (*b - *a).length();
        let d = (*b - *a) / len;
        let side = Vec2::new(-d.y, d.x);
        let count = (len / (if scale_like { 3.0 } else { 2.2 })) as usize;
        for i in 0..count {
            let t = i as f32 / count as f32;
            let p = *a + d * len * t;
            let taper = 1.0 - 0.6 * t;
            for sgn in [-1.0f32, 1.0] {
                let shade = 0.65 + 0.4 * rng.f() + if ti == 0 { 0.05 } else { 0.0 };
                let c = mul3(vary(rng, color, 0.2), shade);
                if scale_like {
                    // Overlapping scaly lobes hugging the twig.
                    let q = p + side * sgn * s * 0.012 + d * s * 0.01;
                    leaf(cv, q, (d + side * sgn * 0.5).x.atan2(-(d + side * sgn * 0.5).y), s * 0.085 * taper + 5.0, 0.75, Shape::Oval, c, shade);
                } else {
                    let nd = (d * 0.55 + side * sgn).normalize();
                    let l = s * 0.07 * taper * (0.8 + 0.4 * rng.f());
                    stroke(cv, p, p + nd * l, 2.6, 1.2, c, mul3(c, 1.15));
                }
            }
        }
    }
}

/// Pine: tufts of long needles fanning from short shoots.
fn pine_tufts(cv: &mut Canvas, cell: Cell, rng: &mut Rng, color: [f32; 3]) {
    let s = cell.s;
    let base = cell.p(0.5, 0.02);
    let shoots: Vec<Vec2> = (0..5)
        .map(|k| {
            let a = (k as f32 - 2.0) * 0.38 + (rng.f() - 0.5) * 0.2;
            base + Vec2::new(a.sin(), -a.cos()) * s * (0.35 + 0.1 * rng.f())
        })
        .collect();
    for sh in &shoots {
        stroke(cv, base, *sh, s * 0.02, s * 0.012, TWIG, mul3(TWIG, 1.3));
    }
    for sh in &shoots {
        let dir = (*sh - base).normalize();
        for _ in 0..70 {
            let t = rng.f();
            let p = base + (*sh - base) * (0.5 + 0.5 * t);
            let a = dir.x.atan2(-dir.y) + (rng.f() - 0.5) * 2.2;
            let l = s * (0.18 + 0.14 * rng.f());
            let d = Vec2::new(a.sin(), -a.cos());
            let c = mul3(vary(rng, color, 0.25), 0.7 + 0.4 * rng.f());
            stroke(cv, p, p + d * l, 2.4, 1.0, c, mul3(c, 1.2));
        }
    }
}

/// A frond mapped along a ribbon: rachis down the middle (u = 0.5), leaflets
/// out to both edges. `leaflets` per side; `droop` bends leaflet tips toward
/// the frond tip; `fern` gives lobed, tapered pinnae.
fn frond(cv: &mut Canvas, cell: Cell, rng: &mut Rng, color: [f32; 3], leaflets: usize, fern: bool, dead: bool) {
    let s = cell.s;
    let rachis_c = if dead { [0.55, 0.42, 0.28] } else { mix3(color, [0.75, 0.7, 0.45], 0.4) };
    stroke(cv, cell.p(0.5, 0.0), cell.p(0.5, 1.0), s * 0.03, s * 0.008, rachis_c, rachis_c);
    for i in 0..leaflets {
        let t = 0.04 + 0.94 * (i as f32 + rng.f() * 0.4) / leaflets as f32;
        for sgn in [-1.0f32, 1.0] {
            if dead && rng.f() < 0.25 {
                continue; // torn
            }
            let root = cell.p(0.5 + sgn * 0.01, t);
            let reach = if fern { (PI * t.powf(0.8)).sin() * 0.46 } else { (0.35 + 0.65 * (PI * t.powf(0.6)).sin()) * 0.47 };
            let tip = cell.p(0.5 + sgn * reach, (t + 0.06 + 0.05 * rng.f()).min(1.0));
            let base_c = if dead { mix3([0.62, 0.48, 0.3], [0.5, 0.38, 0.24], rng.f()) } else { vary(rng, color, 0.2) };
            let shade = 0.72 + 0.35 * rng.f();
            if fern {
                // Pinna: a tapered strip with little lobes.
                let n = 7;
                let d = tip - root;
                for k in 0..n {
                    let kt = k as f32 / n as f32;
                    let p = root + d * kt;
                    let w = s * 0.022 * (1.0 - kt * 0.8) * (0.5 + 0.5 * (PI * t).sin());
                    let a = d.x.atan2(-d.y);
                    leaf(cv, p, a - sgn * 0.9, w * 2.2 + 2.0, 0.7, Shape::Oval, mul3(base_c, shade), 1.0);
                    leaf(cv, p, a + sgn * 0.9, w * 2.2 + 2.0, 0.7, Shape::Oval, mul3(base_c, shade * 0.9), 1.0);
                }
                stroke(cv, root, tip, 2.0, 1.0, mul3(base_c, 0.9), base_c);
            } else {
                let w0 = s * if dead { 0.022 } else { 0.03 };
                stroke(cv, root, tip, w0, 1.2, mul3(base_c, shade), mul3(base_c, shade * 1.15));
            }
        }
    }
}

fn grass(cv: &mut Canvas, cell: Cell, rng: &mut Rng, color: [f32; 3]) {
    // Blades from the bottom edge, gently curving, the whole width.
    for _ in 0..26 {
        let x0 = 0.1 + 0.8 * rng.f();
        let lean = (rng.f() - 0.5) * 0.35;
        let h = 0.6 + 0.4 * rng.f();
        let w = cell.s * (0.025 + 0.02 * rng.f());
        let c = mul3(vary(rng, color, 0.3), 0.75 + 0.35 * rng.f());
        let mut prev = cell.p(x0, 0.0);
        let segs = 8;
        for k in 1..=segs {
            let t = k as f32 / segs as f32;
            let p = cell.p(x0 + lean * t * t, h * t);
            let tip = mix3(c, [0.8, 0.78, 0.5], t * t * 0.5);
            stroke(cv, prev, p, w * (1.0 - (t - 0.12).max(0.0)), w * (1.0 - t * 0.95), mix3(c, tip, t * 0.5), tip);
            prev = p;
        }
    }
}

fn banana_leaf(cv: &mut Canvas, cell: Cell, rng: &mut Rng, color: [f32; 3]) {
    let s = cell.s;
    leaf(cv, cell.p(0.5, 0.0), 0.0, s * 0.99, s * 0.96, Shape::Banana, color, 1.0);
    // Wind tears: cut thin gaps along the vein direction.
    for _ in 0..9 {
        let sgn = if rng.f() < 0.5 { -1.0 } else { 1.0 };
        let t = 0.15 + 0.8 * rng.f();
        let a = cell.p(0.5 + sgn * (0.5 - 0.02), t + 0.08);
        let b = cell.p(0.5 + sgn * (0.12 + 0.25 * rng.f()), t);
        let ab = b - a;
        let len2 = ab.length_squared();
        let (minx, maxx) = (a.x.min(b.x) - 3.0, a.x.max(b.x) + 3.0);
        let (miny, maxy) = (a.y.min(b.y) - 3.0, a.y.max(b.y) + 3.0);
        for y in miny as i64..=maxy as i64 {
            for x in minx as i64..=maxx as i64 {
                let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                let tt = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
                if (p - (a + ab * tt)).length() < 1.6 * (1.0 - tt) + 0.4 && x >= 0 && y >= 0 && (x as usize) < cv.w && (y as usize) < cv.h {
                    cv.px[y as usize * cv.w + x as usize][3] = 0.0;
                }
            }
        }
    }
    // Sun-dried brown margin near the tip.
    for y in cell.y0 as usize..(cell.y0 + s * 0.25) as usize {
        for x in cell.x0 as usize..(cell.x0 + s) as usize {
            let p = &mut cv.px[y * cv.w + x];
            if p[3] > 0.0 {
                let u = (x as f32 - cell.x0) / s;
                let k = smooth(0.38, 0.5, (u - 0.5).abs()) * 0.6;
                let dry = [0.55, 0.45, 0.25];
                for c in 0..3 {
                    p[c] = p[c] + (dry[c] - p[c]) * k;
                }
            }
        }
    }
}

fn acacia(cv: &mut Canvas, cell: Cell, rng: &mut Rng, color: [f32; 3]) {
    // Dense sprays of bipinnate leaves: many pinnae packed with leaflets.
    let s = cell.s;
    let base = cell.p(0.5, 0.02);
    for k in 0..13 {
        let a = (k as f32 - 6.0) * 0.2 + (rng.f() - 0.5) * 0.12;
        let d = Vec2::new(a.sin(), -a.cos());
        let l = s * (0.62 + 0.3 * rng.f()) * (1.0 - 0.25 * (a.abs() / 1.2));
        let end = base + d * l;
        stroke(cv, base, end, 3.0, 1.5, TWIG, TWIG);
        for j in 0..12 {
            let t = 0.15 + 0.85 * j as f32 / 12.0;
            let p = base + (end - base) * t;
            for sgn in [-1.0f32, 1.0] {
                let pd = (d + Vec2::new(-d.y, d.x) * sgn * 1.1).normalize();
                let pl = s * 0.11 * (1.0 - 0.45 * t) * (0.8 + 0.4 * rng.f());
                let pe = p + pd * pl;
                let c = mul3(vary(rng, color, 0.25), 0.65 + 0.45 * rng.f());
                stroke(cv, p, pe, 1.6, 1.0, mul3(c, 0.7), mul3(c, 0.7));
                for m in 0..7 {
                    let q = p + (pe - p) * (m as f32 / 7.0);
                    let la = pd.x.atan2(-pd.y);
                    leaf(cv, q, la - 1.25, s * 0.032, 0.55, Shape::Oval, c, 1.0);
                    leaf(cv, q, la + 1.25, s * 0.032, 0.55, Shape::Oval, mul3(c, 0.88), 1.0);
                }
            }
        }
        // A few pale thorns.
        if rng.f() < 0.5 {
            let p = base + (end - base) * 0.25;
            stroke(cv, p, p + Vec2::new(-d.y, d.x) * s * 0.05, 2.0, 0.6, [0.85, 0.82, 0.75], [0.9, 0.88, 0.8]);
        }
    }
}

fn flowers(cv: &mut Canvas, cell: Cell, rng: &mut Rng, leaf_c: [f32; 3]) {
    twig_cluster(cv, cell, rng, Shape::Lance, 30, 0.26, 0.32, leaf_c);
    // Blossom clusters near the top: five-petal flowers.
    let petal = [0.95, 0.55, 0.72];
    for _ in 0..7 {
        let c = cell.p(0.2 + 0.6 * rng.f(), 0.55 + 0.4 * rng.f());
        for _ in 0..7 {
            let fc = c + Vec2::new((rng.f() - 0.5) * cell.s * 0.14, (rng.f() - 0.5) * cell.s * 0.14);
            let r = cell.s * (0.028 + 0.012 * rng.f());
            let col = mix3(petal, [1.0, 0.92, 0.95], rng.f() * 0.35);
            let rot = rng.f() * TAU;
            for k in 0..5 {
                let a = rot + k as f32 / 5.0 * TAU;
                leaf(cv, fc, a, r, 0.9, Shape::Petal, mul3(col, 0.85 + 0.2 * rng.f()), 1.0);
            }
            stroke(cv, fc, fc + Vec2::new(0.5, 0.5), 3.0, 3.0, [1.0, 0.95, 0.6], [1.0, 0.95, 0.6]);
        }
    }
}

fn solid(cv: &mut Canvas, cell: Cell, rng: &mut Rng) {
    // Neutral bark tone with soft noise, fully opaque.
    for y in cell.y0 as usize..(cell.y0 + cell.s) as usize {
        for x in cell.x0 as usize..(cell.x0 + cell.s) as usize {
            let n = 0.92 + 0.08 * rng.f();
            cv.px[y * cv.w + x] = [0.8 * n, 0.8 * n, 0.8 * n, 1.0];
        }
    }
}

/// Paint the full leaf atlas.
pub fn leaf_atlas() -> Rgba8Image {
    let (w, h) = ((ATLAS_COLS * CELL) as usize, (ATLAS_ROWS * CELL) as usize);
    let mut cv = Canvas::new(w, h);
    let cell = |c: LeafCell| {
        let i = c as u32;
        Cell { x0: ((i % ATLAS_COLS) * CELL) as f32, y0: ((i / ATLAS_COLS) * CELL) as f32, s: CELL as f32 }
    };
    let g = LEAF_BASE;
    let mut rng = Rng::new(0xA71A5);
    twig_cluster(&mut cv, cell(LeafCell::Oak), &mut rng, Shape::Oak, 34, 0.25, 0.55, g);
    twig_cluster(&mut cv, cell(LeafCell::Palmate), &mut rng, Shape::Palmate, 22, 0.17, 1.0, g);
    twig_cluster(&mut cv, cell(LeafCell::Oval), &mut rng, Shape::Oval, 60, 0.15, 0.62, g);
    twig_cluster(&mut cv, cell(LeafCell::Lance), &mut rng, Shape::Lance, 52, 0.22, 0.26, g);
    needle_spray(&mut cv, cell(LeafCell::NeedleSpray), &mut rng, g, false);
    pine_tufts(&mut cv, cell(LeafCell::PineTuft), &mut rng, g);
    frond(&mut cv, cell(LeafCell::PalmFrond), &mut rng, g, 38, false, false);
    frond(&mut cv, cell(LeafCell::FernFrond), &mut rng, g, 22, true, false);
    grass(&mut cv, cell(LeafCell::Grass), &mut rng, g);
    banana_leaf(&mut cv, cell(LeafCell::Banana), &mut rng, g);
    acacia(&mut cv, cell(LeafCell::Acacia), &mut rng, g);
    flowers(&mut cv, cell(LeafCell::Flowers), &mut rng, g);
    solid(&mut cv, cell(LeafCell::Solid), &mut rng);
    needle_spray(&mut cv, cell(LeafCell::Scale), &mut rng, g, true);
    frond(&mut cv, cell(LeafCell::DeadFrond), &mut rng, g, 34, false, true);
    // Olive: narrow leaves, silvery undersides showing.
    twig_cluster(&mut cv, cell(LeafCell::Olive), &mut rng, Shape::Lance, 64, 0.2, 0.2, g);
    finish(cv, true)
}

// ----------------------------------------------------------------- bark --

/// Periodic value noise on a `period`-cell lattice (tiles seamlessly).
fn pnoise(x: f32, y: f32, px: u32, py: u32, seed: u64) -> f32 {
    let (xi, yi) = (x.floor(), y.floor());
    let (fx, fy) = (x - xi, y - yi);
    let h = |i: i64, j: i64| {
        let (i, j) = (i.rem_euclid(px as i64) as u64, j.rem_euclid(py as i64) as u64);
        crate::hash01(seed, i * 7919 + j * 104_729)
    };
    let (ux, uy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let (i, j) = (xi as i64, yi as i64);
    let a = h(i, j) + (h(i + 1, j) - h(i, j)) * ux;
    let b = h(i, j + 1) + (h(i + 1, j + 1) - h(i, j + 1)) * ux;
    a + (b - a) * uy
}

fn fbm(u: f32, v: f32, fx: u32, fy: u32, oct: u32, seed: u64) -> f32 {
    let (mut s, mut amp, mut norm) = (0.0, 1.0, 0.0);
    for o in 0..oct {
        let k = 1 << o;
        s += amp * pnoise(u * (fx * k) as f32, v * (fy * k) as f32, fx * k, fy * k, seed + o as u64);
        norm += amp;
        amp *= 0.5;
    }
    s / norm
}

/// Fissured bark, 512x512, tiles both ways. Neutral grey-brown; vertex
/// colors set the species tone. One repeat ~ 1.2 m of trunk.
pub fn bark_texture() -> Rgba8Image {
    let n = 512usize;
    let mut cv = Canvas::new(n, n);
    for y in 0..n {
        for x in 0..n {
            let (u, v) = (x as f32 / n as f32, y as f32 / n as f32);
            // Vertical plates separated by dark fissures.
            let warp = fbm(u, v, 4, 2, 3, 11) * 0.35;
            let ridges = fbm(u + warp, v * 0.25, 9, 2, 3, 12);
            let fissure = smooth(0.32, 0.5, ridges);
            let grain = fbm(u, v, 32, 8, 3, 13);
            let mottle = fbm(u, v, 3, 3, 2, 14);
            let l = (0.38 + 0.55 * fissure) * (0.85 + 0.25 * grain) * (0.9 + 0.2 * mottle);
            cv.px[y * n + x] = [l * 1.03, l, l * 0.94, 1.0];
        }
    }
    finish(cv, false)
}

/// Palm trunk: horizontal leaf-scar rings (4 per tile) with fibrous texture
/// between. One repeat ~ 1.2 m of trunk.
pub fn palm_bark_texture() -> Rgba8Image {
    let n = 512usize;
    let mut cv = Canvas::new(n, n);
    for y in 0..n {
        for x in 0..n {
            let (u, v) = (x as f32 / n as f32, y as f32 / n as f32);
            let wob = (fbm(u, v, 6, 2, 2, 21) - 0.5) * 0.06;
            let r = ((v + wob) * 4.0).fract();
            // Scar: thin dark groove then a raised lip.
            let groove = 1.0 - 0.55 * (1.0 - smooth(0.0, 0.06, r)) * smooth(-0.02, 0.0, r);
            let lip = 1.0 + 0.18 * smooth(0.06, 0.12, r) * (1.0 - smooth(0.12, 0.3, r));
            let band = 0.9 + 0.1 * (r * PI).sin();
            let fibers = 0.85 + 0.25 * fbm(u, v, 48, 4, 2, 22);
            let mottle = 0.85 + 0.3 * fbm(u, v, 4, 4, 3, 23);
            let l = 0.62 * groove * lip * band * fibers * mottle;
            cv.px[y * n + x] = [l * 1.04, l, l * 0.92, 1.0];
        }
    }
    finish(cv, false)
}

// ----------------------------------------------------------------- mips --

/// Quantize to sRGB8 and append a mip chain. For alpha-tested atlases, mip
/// colors are alpha-weighted (no dark fringes) and alpha is rescaled per
/// cell so coverage at the 0.5 cutoff matches level 0 (leaves don't thin out
/// with distance).
fn finish(cv: Canvas, alpha_tested: bool) -> Rgba8Image {
    let (w, h) = (cv.w, cv.h);
    let mut levels: Vec<(usize, usize, Vec<[f32; 4]>)> = vec![(w, h, cv.px)];
    if alpha_tested {
        // Fill transparent texels with the cell's mean leaf color so bilinear
        // filtering at leaf edges stays green.
        let (cw, ch) = (w / ATLAS_COLS as usize, h / ATLAS_ROWS as usize);
        let px = &mut levels[0].2;
        for cy in 0..ATLAS_ROWS as usize {
            for cx in 0..ATLAS_COLS as usize {
                let (mut sum, mut wsum) = ([0.0f32; 3], 0.0f32);
                for y in cy * ch..(cy + 1) * ch {
                    for x in cx * cw..(cx + 1) * cw {
                        let p = px[y * w + x];
                        for k in 0..3 {
                            sum[k] += p[k] * p[3];
                        }
                        wsum += p[3];
                    }
                }
                let mean = if wsum > 0.0 { [sum[0] / wsum, sum[1] / wsum, sum[2] / wsum] } else { LEAF_BASE };
                for y in cy * ch..(cy + 1) * ch {
                    for x in cx * cw..(cx + 1) * cw {
                        let p = &mut px[y * w + x];
                        let a = p[3];
                        for k in 0..3 {
                            p[k] = p[k] * a + mean[k] * (1.0 - a);
                        }
                    }
                }
            }
        }
    }
    let coverage = |px: &[[f32; 4]], w: usize, x0: usize, y0: usize, cw: usize, ch: usize, scale: f32| {
        let mut c = 0usize;
        for y in y0..y0 + ch {
            for x in x0..x0 + cw {
                if px[y * w + x][3] * scale > 0.5 {
                    c += 1;
                }
            }
        }
        c as f32 / (cw * ch) as f32
    };
    let mut target = vec![0.0f32; (ATLAS_COLS * ATLAS_ROWS) as usize];
    if alpha_tested {
        let (cw, ch) = (w / ATLAS_COLS as usize, h / ATLAS_ROWS as usize);
        for (i, t) in target.iter_mut().enumerate() {
            let (cx, cy) = (i % ATLAS_COLS as usize, i / ATLAS_COLS as usize);
            *t = coverage(&levels[0].2, w, cx * cw, cy * ch, cw, ch, 1.0);
        }
    }
    while levels.last().unwrap().0 > 1 || levels.last().unwrap().1 > 1 {
        let (pw, ph, prev) = levels.last().unwrap();
        let (nw, nh) = ((*pw / 2).max(1), (*ph / 2).max(1));
        let mut next = vec![[0.0f32; 4]; nw * nh];
        for y in 0..nh {
            for x in 0..nw {
                let (mut s, mut a, mut wsum) = ([0.0f32; 3], 0.0f32, 0.0f32);
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let (sx, sy) = ((x * 2 + dx).min(pw - 1), (y * 2 + dy).min(ph - 1));
                    let p = prev[sy * pw + sx];
                    let wgt = if alpha_tested { p[3] + 1e-4 } else { 1.0 };
                    for k in 0..3 {
                        s[k] += p[k] * wgt;
                    }
                    wsum += wgt;
                    a += p[3];
                }
                next[y * nw + x] = [s[0] / wsum, s[1] / wsum, s[2] / wsum, a / 4.0];
            }
        }
        if alpha_tested && nw >= ATLAS_COLS as usize * 2 && nh >= ATLAS_ROWS as usize * 2 {
            let (cw, ch) = (nw / ATLAS_COLS as usize, nh / ATLAS_ROWS as usize);
            for (i, t) in target.iter().enumerate() {
                if *t <= 0.0 || *t >= 1.0 {
                    continue;
                }
                let (cx, cy) = (i % ATLAS_COLS as usize, i / ATLAS_COLS as usize);
                // Binary search the alpha scale that restores coverage.
                let (mut lo, mut hi) = (0.5f32, 4.0f32);
                for _ in 0..10 {
                    let mid = (lo + hi) * 0.5;
                    if coverage(&next, nw, cx * cw, cy * ch, cw, ch, mid) < *t {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                let k = (lo + hi) * 0.5;
                for y in cy * ch..(cy + 1) * ch {
                    for x in cx * cw..(cx + 1) * cw {
                        let p = &mut next[y * nw + x];
                        p[3] = (p[3] * k).min(1.0);
                    }
                }
            }
        }
        levels.push((nw, nh, next));
    }
    let mut data = Vec::new();
    for (_, _, px) in &levels {
        for p in px {
            for k in 0..3 {
                data.push((p[k].clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
            }
            data.push((p[3].clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
        }
    }
    Rgba8Image { width: w as u32, height: h as u32, mip_levels: levels.len() as u32, data }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atlas_cells_have_leaves_and_holes() {
        let img = leaf_atlas();
        assert_eq!(img.width, 2048);
        assert_eq!(img.mip_levels, 12);
        let w = img.width as usize;
        for i in 0..16u32 {
            let (cx, cy) = ((i % ATLAS_COLS) * CELL, (i / ATLAS_COLS) * CELL);
            let mut opaque = 0;
            for y in cy..cy + CELL {
                for x in cx..cx + CELL {
                    if img.data[(y as usize * w + x as usize) * 4 + 3] > 127 {
                        opaque += 1;
                    }
                }
            }
            let frac = opaque as f32 / (CELL * CELL) as f32;
            if i == LeafCell::Solid as u32 {
                assert!(frac > 0.99);
            } else {
                assert!(frac > 0.08 && frac < 0.9, "cell {i}: coverage {frac}");
            }
        }
    }

    #[test]
    fn uv_inside_cell() {
        let [u, v] = atlas_uv(LeafCell::Flowers, 0.0, 0.0);
        assert!((0.75..1.0).contains(&u) && (0.5..0.75).contains(&v));
    }
}
