//! Junction geometry: rounded corners and crosswalks.
//!
//! Where three or more roads meet, each pair of neighbouring branches leaves a wedge between their edges. A
//! corner fill rounds the wedge's tip with an arc of radius R tangent to both edges (a curb return in a town, a
//! paved flare on a country road). Urban junctions of ordinary streets also get zebra crosswalks across each
//! branch just outside the corners. Everything is plain 2D geometry in the tile's metre frame, lifted onto the
//! graded ground by the caller.

use super::RoadClass;

#[derive(Debug, Clone)]
pub struct Branch {
    /// Unit direction away from the node, in the tile's metre frame.
    pub dir: [f64; 2],
    pub hw: f64,
    pub class: RoadClass,
}

#[derive(Debug, Clone)]
pub struct JunctionSpec {
    pub pos: [f64; 2],
    pub urban: bool,
    pub branches: Vec<Branch>,
}

/// Curb return radius by the larger road of the pair (cities are tight, big roads sweep wider).
pub fn radius_for(a: RoadClass, b: RoadClass, urban: bool) -> f64 {
    let rank = a.rank().max(b.rank());
    if urban {
        match rank {
            5 => 8.0,
            3 | 4 => 6.0,
            2 => 4.5,
            _ => 3.0,
        }
    } else if rank >= 3 {
        12.0
    } else {
        6.0
    }
}

fn norm(v: [f64; 2]) -> [f64; 2] {
    let l = v[0].hypot(v[1]).max(1e-12);
    [v[0] / l, v[1] / l]
}

/// The asphalt added at the wedge between branches `b1` and `b2`: corner point, the two tangent points and
/// the arc between them. `None` for wedges that are nearly straight or nearly closed.
pub fn corner_fill(p0: [f64; 2], b1: &Branch, b2: &Branch, radius: f64) -> Option<Vec<[f64; 2]>> {
    let (d1, d2) = (norm(b1.dir), norm(b2.dir));
    let cosv = (d1[0] * d2[0] + d1[1] * d2[1]).clamp(-1.0, 1.0);
    let theta = cosv.acos();
    if !(20f64.to_radians()..=160f64.to_radians()).contains(&theta) {
        return None;
    }
    // Edge of each branch on the side facing the other.
    let perp = |d: [f64; 2]| [-d[1], d[0]];
    let side = |d: [f64; 2], other: [f64; 2]| {
        let n = perp(d);
        if n[0] * other[0] + n[1] * other[1] >= 0.0 {
            n
        } else {
            [-n[0], -n[1]]
        }
    };
    let (n1, n2) = (side(d1, d2), side(d2, d1));
    let (a, b) = (
        [p0[0] + n1[0] * b1.hw, p0[1] + n1[1] * b1.hw],
        [p0[0] + n2[0] * b2.hw, p0[1] + n2[1] * b2.hw],
    );
    // a + d1 s = b + d2 u
    let den = d1[0] * d2[1] - d1[1] * d2[0];
    if den.abs() < 1e-9 {
        return None;
    }
    let s = ((b[0] - a[0]) * d2[1] - (b[1] - a[1]) * d2[0]) / den;
    let c = [a[0] + d1[0] * s, a[1] + d1[1] * s];
    let t = (radius / (theta * 0.5).tan()).min(18.0);
    let r = t * (theta * 0.5).tan();
    let (t1, t2) = (
        [c[0] + d1[0] * t, c[1] + d1[1] * t],
        [c[0] + d2[0] * t, c[1] + d2[1] * t],
    );
    let bis = norm([d1[0] + d2[0], d1[1] + d2[1]]);
    let o = [
        c[0] + bis[0] * r / (theta * 0.5).sin(),
        c[1] + bis[1] * r / (theta * 0.5).sin(),
    ];
    let (a1, a2) = (
        (t1[1] - o[1]).atan2(t1[0] - o[0]),
        (t2[1] - o[1]).atan2(t2[0] - o[0]),
    );
    let mut delta = a2 - a1;
    while delta > std::f64::consts::PI {
        delta -= 2.0 * std::f64::consts::PI;
    }
    while delta < -std::f64::consts::PI {
        delta += 2.0 * std::f64::consts::PI;
    }
    let mut poly = vec![c, t1];
    for k in 1..6 {
        let ang = a1 + delta * k as f64 / 6.0;
        poly.push([o[0] + r * ang.cos(), o[1] + r * ang.sin()]);
    }
    poly.push(t2);
    Some(poly)
}

/// Zebra bars across a branch at distance `at` from the node: (centre, along-road direction, lateral direction).
pub fn crosswalk_bars(p0: [f64; 2], b: &Branch, at: f64) -> Vec<([f64; 2], [f64; 2], [f64; 2])> {
    let d = norm(b.dir);
    let lat = [-d[1], d[0]];
    let c = [p0[0] + d[0] * at, p0[1] + d[1] * at];
    let n = ((2.0 * b.hw - 0.8) / 1.0).floor().max(0.0) as i32;
    (0..n)
        .map(|k| {
            let o = -b.hw + 0.4 + 0.5 + k as f64;
            ([c[0] + lat[0] * o, c[1] + lat[1] * o], d, lat)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn br(dir: [f64; 2], hw: f64) -> Branch {
        Branch {
            dir,
            hw,
            class: RoadClass::Minor,
        }
    }

    #[test]
    fn right_angle_corner_is_a_quarter_arc_tangent_to_both_edges() {
        // Roads along +x and +y, half width 3: the corner point is (3, 3).
        let poly =
            corner_fill([0.0, 0.0], &br([1.0, 0.0], 3.0), &br([0.0, 1.0], 3.0), 4.0).unwrap();
        let c = poly[0];
        assert!(
            (c[0] - 3.0).abs() < 1e-9 && (c[1] - 3.0).abs() < 1e-9,
            "{c:?}"
        );
        let (t1, t2) = (poly[1], *poly.last().unwrap());
        assert!(
            (t1[0] - 7.0).abs() < 1e-9 && (t1[1] - 3.0).abs() < 1e-9,
            "tangent distance R/tan(45deg) = R: {t1:?}"
        );
        assert!((t2[0] - 3.0).abs() < 1e-9 && (t2[1] - 7.0).abs() < 1e-9);
        // Arc points are all R from the centre (7, 7).
        for p in &poly[1..] {
            let r = (p[0] - 7.0).hypot(p[1] - 7.0);
            assert!((r - 4.0).abs() < 1e-9, "{r}");
        }
        // The arc bulges toward the corner: its midpoint is nearer c than the chord's midpoint.
        let mid = poly[poly.len() / 2];
        let chord = [(t1[0] + t2[0]) / 2.0, (t1[1] + t2[1]) / 2.0];
        assert!(mid[0].hypot(mid[1]) < chord[0].hypot(chord[1]));
    }

    #[test]
    fn straight_and_hairpin_wedges_get_no_fill() {
        assert!(corner_fill([0.0; 2], &br([1.0, 0.0], 3.0), &br([-1.0, 0.0], 3.0), 4.0).is_none());
        assert!(corner_fill([0.0; 2], &br([1.0, 0.0], 3.0), &br([1.0, 0.05], 3.0), 4.0).is_none());
    }

    #[test]
    fn crosswalk_bars_span_the_carriageway() {
        let bars = crosswalk_bars([0.0, 0.0], &br([1.0, 0.0], 3.5), 10.0);
        assert!(bars.len() >= 5);
        assert!(
            bars.iter()
                .all(|(c, _, _)| (c[0] - 10.0).abs() < 1e-9 && c[1].abs() <= 3.5)
        );
    }
}
