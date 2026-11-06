//! Parking garages you can drive into: open storeys on a column grid, a belt beam round each floor, and a ramp
//! between every pair of floors, with a collider for the slabs, ramps and parapets.
//!
//! The footprint comes from Overture (`class = parking`, a few hundred square metres or more). A rectangular
//! footprint gets ramps (each storey's slab has a hole where its arriving ramp rises through); an irregular one
//! gets open decks without ramps. Engine-free and deterministic: everything is derived from the footprint.

use crate::mesh::{MeshData, rgb};
use crate::meshkit;
use glam::Vec3;

pub const FLOOR_H: f32 = 3.0;
pub const MIN_AREA_M2: f32 = 400.0;
const RAMP_GRADE: f32 = 0.12;
const RAMP_W: f32 = 5.5;
const COLUMN_SPACING: f32 = 7.5;
const BELT_DOWN: f32 = 0.7;
const BELT_UP: f32 = 0.95;

pub struct GarageMesh {
    pub visible: MeshData,
    pub collider: MeshData,
    pub floors: u32,
    pub ramps: u32,
}

pub fn floors_for(height: Option<f32>) -> u32 {
    height
        .map(|h| ((h / FLOOR_H).round() as i32).clamp(2, 8) as u32)
        .unwrap_or(4)
}

fn point_in_poly(p: [f32; 2], poly: &[[f32; 2]]) -> bool {
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[j]);
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// A vertical wall quad between `a` and `b` from height `h0` to `h1`, facing `outward`.
fn wall(
    m: &mut MeshData,
    a: Vec3,
    b: Vec3,
    up: Vec3,
    h0: f32,
    h1: f32,
    outward: Vec3,
    c: [f32; 4],
) {
    let p = [a + up * h0, b + up * h0, b + up * h1, a + up * h1];
    let n = (p[1] - p[0]).cross(p[2] - p[0]);
    let q = if n.dot(outward) >= 0.0 {
        p
    } else {
        [p[1], p[0], p[3], p[2]]
    };
    m.quad(q, [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]], c);
}

pub fn build(ring: &[Vec3], up: Vec3, floors: u32) -> Option<GarageMesh> {
    if ring.len() < 3 {
        return None;
    }
    let area = meshkit::signed_area(ring, up).abs();
    if area < MIN_AREA_M2 {
        return None;
    }
    let n = ring.len();
    // Longest edge gives the plan axes.
    let mut e1 = Vec3::X;
    let mut best = 0.0f32;
    for i in 0..n {
        let d = (ring[(i + 1) % n] - ring[i]).reject_from_normalized(up);
        if d.length() > best {
            best = d.length();
            e1 = d.normalize_or_zero();
        }
    }
    let e2 = up.cross(e1).normalize_or_zero();
    let o = ring[0];
    let ab: Vec<[f32; 2]> = ring
        .iter()
        .map(|p| [(*p - o).dot(e1), (*p - o).dot(e2)])
        .collect();
    let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
    for p in &ab {
        for k in 0..2 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    let (la, lb) = (hi[0] - lo[0], hi[1] - lo[1]);
    let rect = area / (la * lb).max(1.0) > 0.75;
    let ramp_len = FLOOR_H / RAMP_GRADE;
    let with_ramps = rect && la >= ramp_len + 10.0 && lb >= 2.0 * RAMP_W + 3.0 && floors >= 2;
    let base = ring.iter().map(|p| p.dot(up)).fold(f32::MAX, f32::min);
    let at = |a: f32, b: f32, y: f32| o + e1 * a + e2 * b + up * (y - o.dot(up));
    let flat = |p: &Vec3, y: f32| *p + up * (y - p.dot(up));
    let (concrete, under, belt, col_c, ramp_c) = (
        rgb(0xa9a7a1),
        rgb(0x7f7d79),
        rgb(0x9b9994),
        rgb(0x8f8d88),
        rgb(0x6d6d70),
    );
    // Ramp footprints: storey k (1..=floors) arrives through its own rectangle; sides alternate.
    let ramp_rect = |k: u32| -> ([f32; 2], [f32; 2]) {
        let a0 = lo[0] + 2.0;
        let (b0, b1) = if k % 2 == 1 {
            (lo[1] + 1.0, lo[1] + 1.0 + RAMP_W)
        } else {
            (hi[1] - 1.0 - RAMP_W, hi[1] - 1.0)
        };
        ([a0, a0 + ramp_len], [b0, b1])
    };
    let mut vis = MeshData::default();
    let mut col = MeshData::default();
    let centre = ring.iter().fold(Vec3::ZERO, |s, p| s + *p) / n as f32;
    for k in 0..=floors {
        let y = base + k as f32 * FLOOR_H;
        let outer: Vec<Vec3> = ring.iter().map(|p| flat(p, y)).collect();
        let holes: Vec<Vec<Vec3>> = if with_ramps && k >= 1 {
            let (ra, rb) = ramp_rect(k);
            vec![vec![
                at(ra[0], rb[0], y),
                at(ra[1], rb[0], y),
                at(ra[1], rb[1], y),
                at(ra[0], rb[1], y),
            ]]
        } else {
            Vec::new()
        };
        meshkit::polygon(&mut vis, &outer, &holes, up, 6.0, concrete);
        meshkit::polygon(&mut col, &outer, &holes, up, 6.0, concrete);
        if k >= 1 {
            let under_outer: Vec<Vec3> = ring.iter().map(|p| flat(p, y - 0.35)).collect();
            let under_holes: Vec<Vec<Vec3>> = holes
                .iter()
                .map(|h| h.iter().map(|p| *p - up * 0.35).collect())
                .collect();
            meshkit::polygon(&mut vis, &under_outer, &under_holes, -up, 6.0, under);
        }
        // Belt beam round the perimeter, both faces.
        for i in 0..n {
            let (a, b) = (flat(&ring[i], y), flat(&ring[(i + 1) % n], y));
            let out = ((a + b) * 0.5 - flat(&centre, y)).reject_from_normalized(up);
            let out = if out.length() < 1e-4 {
                (b - a).cross(up)
            } else {
                out
            };
            wall(&mut vis, a, b, up, -BELT_DOWN, BELT_UP, out, belt);
            wall(&mut vis, a, b, up, -BELT_DOWN, BELT_UP, -out, belt);
            wall(&mut col, a, b, up, 0.0, BELT_UP, out, belt);
        }
    }
    // Columns between storeys on a grid, clear of ramps.
    let mut a = lo[0] + COLUMN_SPACING * 0.5;
    while a < hi[0] {
        let mut b = lo[1] + COLUMN_SPACING * 0.5;
        while b < hi[1] {
            let inside = point_in_poly([a, b], &ab);
            let in_ramp = with_ramps
                && (1..=floors).any(|k| {
                    let (ra, rb) = ramp_rect(k);
                    a > ra[0] - 1.0 && a < ra[1] + 1.0 && b > rb[0] - 1.0 && b < rb[1] + 1.0
                });
            if inside && !in_ramp {
                let h = 0.25;
                let corners = [
                    at(a - h, b - h, base),
                    at(a + h, b - h, base),
                    at(a + h, b + h, base),
                    at(a - h, b + h, base),
                ];
                let top = floors as f32 * FLOOR_H;
                for i in 0..4 {
                    let (p, q) = (corners[i], corners[(i + 1) % 4]);
                    let out = ((p + q) * 0.5 - at(a, b, base)).reject_from_normalized(up);
                    wall(&mut vis, p, q, up, 0.0, top, out, col_c);
                }
            }
            b += COLUMN_SPACING;
        }
        a += COLUMN_SPACING;
    }
    // Ramps.
    let mut ramps = 0;
    if with_ramps {
        for k in 1..=floors {
            let (ra, rb) = ramp_rect(k);
            let (y0, y1) = (base + (k - 1) as f32 * FLOOR_H, base + k as f32 * FLOOR_H);
            // Odd storeys rise toward +a, even toward -a.
            let (ya, yb) = if k % 2 == 1 { (y0, y1) } else { (y1, y0) };
            let p = [
                at(ra[0], rb[0], ya),
                at(ra[1], rb[0], yb),
                at(ra[1], rb[1], yb),
                at(ra[0], rb[1], ya),
            ];
            for m in [&mut vis, &mut col] {
                let n = (p[1] - p[0]).cross(p[2] - p[0]);
                let q = if n.dot(up) >= 0.0 {
                    p
                } else {
                    [p[1], p[0], p[3], p[2]]
                };
                m.quad(q, [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]], ramp_c);
            }
            ramps += 1;
        }
    }
    Some(GarageMesh {
        visible: vis,
        collider: col,
        floors,
        ramps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(w: f32, d: f32) -> Vec<Vec3> {
        vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(w, 0.0, 0.0),
            Vec3::new(w, 0.0, d),
            Vec3::new(0.0, 0.0, d),
        ]
    }

    #[test]
    fn rectangular_garage_has_a_ramp_per_storey_and_a_collider() {
        let g = build(&rect(60.0, 30.0), Vec3::Y, 4).unwrap();
        assert_eq!(g.ramps, 4);
        assert!(g.visible.triangle_count() > 200 && g.collider.triangle_count() > 40);
        assert!(g.visible.positions.iter().flatten().all(|v| v.is_finite()));
        // Highest slab is 4 storeys up.
        let top = g
            .visible
            .positions
            .iter()
            .map(|p| p[1])
            .fold(f32::MIN, f32::max);
        assert!((top - (4.0 * FLOOR_H + BELT_UP)).abs() < 0.05, "{top}");
    }

    #[test]
    fn small_or_odd_footprints() {
        assert!(
            build(&rect(15.0, 15.0), Vec3::Y, 3).is_none(),
            "under 400 m2 is a carport"
        );
        let l_shape = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(40.0, 0.0, 0.0),
            Vec3::new(40.0, 0.0, 15.0),
            Vec3::new(15.0, 0.0, 15.0),
            Vec3::new(15.0, 0.0, 40.0),
            Vec3::new(0.0, 0.0, 40.0),
        ];
        let g = build(&l_shape, Vec3::Y, 3).unwrap();
        assert_eq!(g.ramps, 0, "irregular footprints get decks without ramps");
        assert!(g.collider.triangle_count() > 10);
    }

    #[test]
    fn floors_follow_height() {
        assert_eq!(floors_for(Some(15.0)), 5);
        assert_eq!(floors_for(Some(3.0)), 2);
        assert_eq!(floors_for(None), 4);
    }
}
