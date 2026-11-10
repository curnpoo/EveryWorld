//! W4d: compare cube-to-sphere mappings and test edge agreement. `cargo run --release`.
use std::collections::HashSet;
use std::f64::consts::FRAC_PI_4;

type V = [f64; 3];
fn norm(p: V) -> V {
    let l = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
    // `+ 0.0` turns -0.0 into 0.0: faces that negate an axis would otherwise disagree in the sign bit.
    [p[0] / l + 0.0, p[1] / l + 0.0, p[2] / l + 0.0]
}
fn dot(a: V, b: V) -> f64 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
fn cross(a: V, b: V) -> V { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }
fn arc(a: V, b: V) -> f64 { let c = cross(a, b); dot(c, c).sqrt().atan2(dot(a, b)) }
/// Solid angle of a spherical triangle (Van Oosterom and Strackee).
fn tri(a: V, b: V, c: V) -> f64 { 2.0 * dot(a, cross(b, c)).abs().atan2(1.0 + dot(a, b) + dot(b, c) + dot(c, a)) }

#[derive(Clone, Copy, PartialEq, Debug)]
enum Map { Gnomonic, Tangent, S2Quadratic }

/// Lattice coordinate i in 0..=n to the face coordinate in [-1, 1]. Exact at both ends and
/// exactly odd about the centre, so two faces that share an edge compute identical values.
fn warp(m: Map, i: u64, n: u64) -> f64 {
    if i == 0 { return -1.0 }
    if i == n { return 1.0 }
    if 2 * i == n { return 0.0 }
    if 2 * i > n { return -warp(m, n - i, n) }
    let s = (2 * i) as f64 / n as f64 - 1.0; // in (-1, 0)
    match m {
        Map::Gnomonic => s,
        Map::Tangent => (s * FRAC_PI_4).tan(),
        Map::S2Quadratic => { let t = 0.5 * (s + 1.0); (1.0 / 3.0) * (1.0 - 4.0 * (1.0 - t) * (1.0 - t)) }
    }
}

/// Faces: 0 +X, 1 -X, 2 +Y, 3 -Y, 4 +Z, 5 -Z. Right-handed (u, v) on each.
fn cube(face: u8, u: f64, v: f64) -> V {
    match face {
        0 => [1.0, u, v],
        1 => [-1.0, -u, v],
        2 => [-u, 1.0, v],
        3 => [u, -1.0, v],
        4 => [-v, u, 1.0],
        _ => [v, u, -1.0],
    }
}
fn point(m: Map, face: u8, i: u64, j: u64, n: u64) -> V { norm(cube(face, warp(m, i, n), warp(m, j, n))) }

fn main() {
    println!("mapping        level  area max/min  aspect max  edge max/min");
    for m in [Map::Gnomonic, Map::S2Quadratic, Map::Tangent] {
        for level in [6u32, 12] {
            let n = 1u64 << level;
            // One octant of one face is enough by symmetry; sample a 256-step lattice at high levels.
            let step = (n / 256).max(1);
            let (mut amin, mut amax, mut asp, mut emin, mut emax) = (f64::MAX, 0f64, 0f64, f64::MAX, 0f64);
            let mut i = n / 2;
            while i < n {
                let mut j = n / 2;
                while j < n {
                    let (a, b, c, d) = (point(m, 0, i, j, n), point(m, 0, i + 1, j, n), point(m, 0, i + 1, j + 1, n), point(m, 0, i, j + 1, n));
                    let area = tri(a, b, c) + tri(a, c, d);
                    let (eu, ev) = (arc(a, b), arc(a, d));
                    amin = amin.min(area); amax = amax.max(area);
                    asp = asp.max(eu.max(ev) / eu.min(ev));
                    emin = emin.min(eu.min(ev)); emax = emax.max(eu.max(ev));
                    j += step;
                }
                i += step;
            }
            println!("{:<14} L{:<5} {:>10.4}   {:>8.4}   {:>8.4}", format!("{m:?}"), level, amax / amin, asp, emax / emin);
        }
    }

    // Edge agreement: every lattice point on a face boundary must be bit-identical to a point
    // on the boundary of a neighbouring face.
    for m in [Map::Gnomonic, Map::S2Quadratic, Map::Tangent] {
        let n = 1u64 << 10;
        let bits = |p: V| [p[0].to_bits(), p[1].to_bits(), p[2].to_bits()];
        let mut sets: Vec<HashSet<[u64; 3]>> = Vec::new();
        for f in 0..6u8 {
            let mut s = HashSet::new();
            for k in 0..=n {
                for (i, j) in [(k, 0), (k, n), (0, k), (n, k)] { s.insert(bits(point(m, f, i, j, n))); }
            }
            sets.push(s);
        }
        let mut orphans = 0;
        for f in 0..6 {
            for p in &sets[f] {
                if !(0..6).any(|g| g != f && sets[g].contains(p)) { orphans += 1; }
            }
        }
        println!("edge agreement {m:?}: {} boundary points per face, {orphans} without a bit-identical twin", sets[0].len());
    }
    // A child's corner must equal its parent's lattice point at the finer level.
    let m = Map::Tangent;
    let mut bad = 0;
    for l in 0..15u32 {
        let (n, n2) = (1u64 << l, 1u64 << (l + 1));
        for i in 0..=n.min(64) { if warp(m, i, n).to_bits() != warp(m, 2 * i, n2).to_bits() { bad += 1; } }
    }
    println!("parent/child lattice agreement (Tangent, L0-L15): {bad} mismatches");
    let r = 6_371_008.8_f64;
    for l in [6u32, 12, 15] { println!("L{l}: mean node edge {:.1} km", (4.0 * std::f64::consts::PI * r * r / (6.0 * 4f64.powi(l as i32))).sqrt() / 1000.0); }
}
