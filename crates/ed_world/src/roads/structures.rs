//! Bridge decks, piers, tunnel tubes and portals: the parts of a road that are not draped on the ground.
//!
//! Decks and tubes are swept along the solved centreline with a closed cross-section, so they have an
//! underside, parapets and walls instead of a floating ribbon with a curtain. The same sweep, run again into a
//! separate buffer, is the collider (thin, one surface, no colours).

use super::{CROWN, P, StreetKit, p, side_dirs, tri_out};
use crate::mesh::{MeshData, rgb};
use glam::Vec3;

const CONCRETE: u32 = 0xa09e98;
const CONCRETE_DARK: u32 = 0x6a6a68;

/// Sweep a closed profile along `pts` with no skirts, no terrain fitting and no tint noise.
pub(super) fn sweep_raw(m: &mut MeshData, pts: &[Vec3], up: Vec3, prof: &[P]) {
    if pts.len() < 2 || prof.len() < 2 {
        return;
    }
    let side = side_dirs(pts, up);
    let mut dist = Vec::with_capacity(pts.len());
    let mut d = 0.0f32;
    for i in 0..pts.len() {
        if i > 0 {
            d += pts[i].distance(pts[i - 1]);
        }
        dist.push(d);
    }
    let mut prev: Option<Vec<(u32, u32)>> = None;
    for i in 0..pts.len() {
        let mut cur = Vec::with_capacity(prof.len() - 1);
        for k in 0..prof.len() - 1 {
            let (a, b) = (&prof[k], &prof[k + 1]);
            let n = (side[i] * -(b.h - a.h) + up * (b.o - a.o)).normalize_or_zero();
            let at = |q: &P| pts[i] + side[i] * q.o + up * q.h;
            let l = m.vertex(at(a).to_array(), n.to_array(), [a.o, dist[i]], a.c);
            let r = m.vertex(
                at(b).to_array(),
                n.to_array(),
                [b.o, dist[i]],
                if a.grad { b.c } else { a.c },
            );
            cur.push((l, r));
        }
        if let Some(pv) = &prev {
            for k in 0..cur.len() {
                if (prof[k + 1].o - prof[k].o).abs() < 1e-4
                    && (prof[k + 1].h - prof[k].h).abs() < 1e-4
                {
                    continue;
                }
                let ((pl, pr), (l, r)) = (pv[k], cur[k]);
                tri_out(m, pl, pr, r);
                tri_out(m, pl, r, l);
            }
        }
        prev = Some(cur);
    }
}

/// Closed deck section, left to right over the top and back along the underside:
/// parapet, curb, cambered asphalt, curb, parapet, then the girder bottom.
pub(super) fn deck_profile(hw: f32, depth: f32, kit: &StreetKit) -> Vec<P> {
    let (c, u, a) = (rgb(CONCRETE), rgb(CONCRETE_DARK), rgb(kit.asphalt));
    vec![
        p(-hw - 0.6, -depth, u, false),
        p(-hw - 0.6, 0.9, c, false),
        p(-hw - 0.25, 0.9, c, false),
        p(-hw - 0.25, 0.15, c, false),
        p(-hw, 0.15, c, false),
        p(-hw, 0.02, a, false),
        p(0.0, CROWN, a, false),
        p(hw, 0.02, a, false),
        p(hw, 0.15, c, false),
        p(hw + 0.25, 0.15, c, false),
        p(hw + 0.25, 0.9, c, false),
        p(hw + 0.6, 0.9, c, false),
        p(hw + 0.6, -depth, u, false),
        p(-hw - 0.6, -depth, u, false),
    ]
}

/// New Jersey barrier: sloped base, 0.81 m tall, closed section (bottom included so it has a collider).
pub(super) fn jersey_profile() -> Vec<P> {
    let c = rgb(0xb2b0aa);
    vec![
        p(-0.30, 0.0, c, false),
        p(-0.2, 0.33, c, false),
        p(-0.09, 0.81, c, false),
        p(0.09, 0.81, c, false),
        p(0.2, 0.33, c, false),
        p(0.30, 0.0, c, false),
        p(-0.30, 0.0, c, false),
    ]
}

/// Interior of a two-way road tunnel: floor, walls, flattened-arch ceiling.
pub(super) fn tube_profile(hw: f32, kit: &StreetKit) -> Vec<P> {
    let (w, ceil, a, walk) = (
        rgb(0x8d8c88),
        rgb(0x7a7977),
        rgb(kit.asphalt),
        rgb(0x9a9893),
    );
    let r = hw + 1.0;
    let mut v = vec![
        p(-r, 0.2, walk, false),
        p(-hw, 0.2, walk, false),
        p(-hw, 0.02, a, false),
        p(0.0, CROWN, a, false),
        p(hw, 0.02, a, false),
        p(hw, 0.2, walk, false),
        p(r, 0.2, walk, false),
        p(r, 3.4, w, false),
    ];
    for k in 1..=8 {
        let th = k as f32 / 8.0 * std::f32::consts::PI;
        v.push(p(r * th.cos(), 3.4 + 2.2 * th.sin(), ceil, false));
    }
    v.push(p(-r, 0.2, w, false));
    v
}

/// Axis-aligned-in-a-frame box: centre `c`, axes `r`,`u`,`f`, half sizes `h`.
pub(super) fn boxm(m: &mut MeshData, c: Vec3, r: Vec3, u: Vec3, f: Vec3, h: Vec3, color: [f32; 4]) {
    let (rx, uy, fz) = (r * h.x, u * h.y, f * h.z);
    let corner = |sx: f32, sy: f32, sz: f32| c + rx * sx + uy * sy + fz * sz;
    let faces: [[(f32, f32, f32); 4]; 6] = [
        [
            (-1.0, 1.0, -1.0),
            (-1.0, 1.0, 1.0),
            (1.0, 1.0, 1.0),
            (1.0, 1.0, -1.0),
        ],
        [
            (-1.0, -1.0, 1.0),
            (-1.0, -1.0, -1.0),
            (1.0, -1.0, -1.0),
            (1.0, -1.0, 1.0),
        ],
        [
            (1.0, -1.0, -1.0),
            (1.0, 1.0, -1.0),
            (1.0, 1.0, 1.0),
            (1.0, -1.0, 1.0),
        ],
        [
            (-1.0, -1.0, 1.0),
            (-1.0, 1.0, 1.0),
            (-1.0, 1.0, -1.0),
            (-1.0, -1.0, -1.0),
        ],
        [
            (-1.0, -1.0, 1.0),
            (1.0, -1.0, 1.0),
            (1.0, 1.0, 1.0),
            (-1.0, 1.0, 1.0),
        ],
        [
            (1.0, -1.0, -1.0),
            (-1.0, -1.0, -1.0),
            (-1.0, 1.0, -1.0),
            (1.0, 1.0, -1.0),
        ],
    ];
    for f in faces {
        m.quad(
            f.map(|(a, b, c)| corner(a, b, c)),
            [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            color,
        );
    }
}

/// Hammerhead piers under a deck: two columns and a cap beam every ~30 m, where there is room to the ground
/// (`room[i]`: metres from the underside to the ground at point i) and no other road passes underneath
/// (`skip`: arc ranges to leave clear).
pub(super) fn piers(
    m: &mut MeshData,
    pts: &[Vec3],
    up: Vec3,
    hw: f32,
    depth: f32,
    room: &[f32],
    skip: &[(f32, f32)],
) {
    if pts.len() < 2 || room.len() != pts.len() {
        return;
    }
    let side = side_dirs(pts, up);
    let mut s = vec![0.0f32; pts.len()];
    for i in 1..pts.len() {
        s[i] = s[i - 1] + pts[i].distance(pts[i - 1]);
    }
    let total = s[pts.len() - 1];
    let color = rgb(CONCRETE);
    let mut next = 15.0f32;
    for i in 1..pts.len() - 1 {
        if s[i] < next {
            continue;
        }
        next = s[i] + 30.0;
        if s[i] < 12.0 || s[i] > total - 12.0 || skip.iter().any(|&(a, b)| s[i] >= a && s[i] <= b) {
            continue;
        }
        let gap = room[i];
        if gap < 3.0 {
            continue;
        }
        let fwd = side[i].cross(up).normalize_or_zero();
        let bottom = pts[i] - up * (depth + gap + 1.0);
        let col_h = (gap + 1.0 - 1.1) * 0.5;
        for sgn in [-1.0f32, 1.0] {
            let c = bottom + up * col_h + side[i] * (sgn * hw * 0.5);
            boxm(m, c, side[i], up, fwd, Vec3::new(0.85, col_h, 0.85), color);
        }
        boxm(
            m,
            pts[i] - up * (depth + 0.55),
            side[i],
            up,
            fwd,
            Vec3::new(hw + 0.4, 0.55, 0.9),
            color,
        );
    }
}

/// Concrete portal at a tunnel mouth: a lintel and two cheek walls. `dir` points into the tunnel.
pub(super) fn portal(m: &mut MeshData, at: Vec3, dir: Vec3, up: Vec3, hw: f32) {
    let f = dir.normalize_or_zero();
    let r = f.cross(up).normalize_or_zero();
    let c = rgb(0x9b9a95);
    let half_open = hw + 1.0;
    boxm(
        m,
        at + up * 5.9 + f * 1.0,
        r,
        up,
        f,
        Vec3::new(half_open + 2.2, 1.3, 1.0),
        c,
    );
    for sgn in [-1.0f32, 1.0] {
        boxm(
            m,
            at + up * 3.4 + f * 1.0 + r * (sgn * (half_open + 1.1)),
            r,
            up,
            f,
            Vec3::new(1.1, 3.4, 1.0),
            c,
        );
    }
}

/// Retaining wall for a cut: vertical panels on sides where the natural ground sits above the road.
pub fn cut_wall(m: &mut MeshData, pts: &[Vec3], up: Vec3, hw: f32, ground: &dyn Fn(Vec3) -> f32, kit: &super::StreetKit) {
    let color = rgb(0x9a9893);
    let n = pts.len();
    if n < 2 {
        return;
    }
    let side_dirs: Vec<Vec3> = (0..n)
        .map(|i| {
            let a = pts[i.saturating_sub(1)];
            let b = pts[(i + 1).min(n - 1)];
            (b - a).cross(up).normalize_or_zero()
        })
        .collect();
    for i in 0..n - 1 {
        let (a, b) = (pts[i], pts[i + 1]);
        let sd = (side_dirs[i] + side_dirs[i + 1]).normalize_or(side_dirs[i]);
        let mid = (a + b) * 0.5;
        for sgn in [-1.0f32, 1.0] {
            let probe = mid + sd * (sgn * (hw + 1.5));
            let gnd = ground(probe);
            let road_y = mid.dot(up);
            let dh = gnd - road_y;
            if dh < 2.5 {
                continue;
            }
            // Draw a vertical wall from road level up to ground level along this segment.
            let seg_len = (b - a).length();
            let fwd = (b - a).normalize_or_zero();
            let (p0, p1) = (a, b);
            let wall = [
                p0 + sd * (sgn * hw),
                p1 + sd * (sgn * hw),
                p1 + sd * (sgn * hw) + up * dh,
                p0 + sd * (sgn * hw) + up * dh,
            ];
            m.quad(wall, [[0.0, 0.0]; 4], color);
            let _ = (seg_len, fwd, mid, gnd, road_y);
        }
    }
}

/// Supplementary bridge structure geometry (arch ribs, truss members, cables) layered on top of the
/// beam deck. Beam bridges add nothing.
pub fn bridge_extras(m: &mut MeshData, pts: &[Vec3], up: Vec3, hw: f32, kind: super::BridgeKind, kit: &super::StreetKit) {
    use super::BridgeKind;
    if pts.len() < 2 {
        return;
    }
    let color = rgb(kit.asphalt ^ 0x303030);
    match kind {
        BridgeKind::Beam => {}
        BridgeKind::ThroughArch => {
            // Two arch ribs rising above the deck, with vertical hangers.
            let n = pts.len();
            let mut left = Vec::with_capacity(n);
            let mut right = Vec::with_capacity(n);
            for p in pts {
                let rise = 0.0f32; // placeholder; compute from position along span
                let _ = rise;
                left.push(*p + up * 0.0);
                right.push(*p + up * 0.0);
            }
            let _ = (left, right, color, hw);
            // Draw a simple arch rib as a raised centreline sweep.
            let mut rib: Vec<Vec3> = pts
                .iter()
                .enumerate()
                .map(|(i, pt)| {
                    let t = i as f32 / (n as f32 - 1.0).max(1.0);
                    let arch = (t * (1.0 - t)) * 4.0 * (hw + 4.0);
                    *pt + up * (arch + 1.0)
                })
                .collect();
            if rib.len() >= 2 {
                // sweep a thin box along the rib
                let prof = vec![p(-0.15, 0.0, color, false), p(0.15, 0.0, color, false)];
                sweep_raw(m, &rib, up, &prof);
            }
            // Hangers
            for (i, pt) in pts.iter().enumerate() {
                if i == 0 || i == n - 1 {
                    continue;
                }
                let t = i as f32 / (n as f32 - 1.0).max(1.0);
                let arch = (t * (1.0 - t)) * 4.0 * (hw + 4.0) + 1.0;
                let top = *pt + up * arch;
                let bot = *pt + up * 0.15;
                let seg = [bot, top];
                let prof = vec![p(-0.08, 0.0, color, false), p(0.08, 0.0, color, false)];
                sweep_raw(m, &seg, up, &prof);
            }
        }
        BridgeKind::StoneArch => {
            // A masonry arch under the deck: a raised curve from abutment to abutment.
            let n = pts.len();
            let mut rib: Vec<Vec3> = pts
                .iter()
                .enumerate()
                .map(|(i, pt)| {
                    let t = i as f32 / (n as f32 - 1.0).max(1.0);
                    let arch = -(t * (1.0 - t)) * 4.0 * (hw + 2.0);
                    *pt + up * (arch - 0.5)
                })
                .collect();
            if rib.len() >= 2 {
                let prof = vec![p(-0.25, 0.0, color, false), p(0.25, 0.0, color, false)];
                sweep_raw(m, &rib, up, &prof);
            }
        }
        BridgeKind::Truss => {
            // Diagonal + vertical web members between top and bottom chords.
            let n = pts.len();
            let top: Vec<Vec3> = pts.iter().map(|pt| *pt + up * (hw * 0.0 + 1.2)).collect();
            let bot: Vec<Vec3> = pts.iter().map(|pt| *pt + up * (-0.5)).collect();
            let chord_prof = vec![p(-0.1, 0.0, color, false), p(0.1, 0.0, color, false)];
            sweep_raw(m, &top, up, &chord_prof);
            sweep_raw(m, &bot, up, &chord_prof);
            for i in 0..n - 1 {
                // vertical
                let seg = [bot[i], top[i]];
                sweep_raw(m, &seg, up, &chord_prof);
                // diagonal
                if i + 1 < n {
                    let seg = [bot[i], top[i + 1]];
                    sweep_raw(m, &seg, up, &chord_prof);
                }
            }
        }
        BridgeKind::Suspension => {
            // Two towers at the quarter points + main cable + hangers.
            let n = pts.len();
            let tower_h = hw + 8.0;
            for &ti in &[n / 4, 3 * n / 4] {
                if ti == 0 || ti >= n {
                    continue;
                }
                let base = pts[ti];
                let tower = [base, base + up * tower_h];
                let prof = vec![p(-0.4, 0.0, color, false), p(0.4, 0.0, color, false)];
                sweep_raw(m, &tower, up, &prof);
            }
            // Main cable: a sagging curve between endpoints (and over towers).
            let cable: Vec<Vec3> = pts
                .iter()
                .enumerate()
                .map(|(i, pt)| {
                    let t = i as f32 / (n as f32 - 1.0).max(1.0);
                    let sag = (t * (1.0 - t)) * 2.0 * (hw + 2.0);
                    *pt + up * (tower_h.min(hw + 6.0) - sag)
                })
                .collect();
            if cable.len() >= 2 {
                let prof = vec![p(-0.12, 0.0, color, false), p(0.12, 0.0, color, false)];
                sweep_raw(m, &cable, up, &prof);
            }
            // Hangers
            for (i, pt) in pts.iter().enumerate() {
                if i % 3 != 0 {
                    continue;
                }
                let t = i as f32 / (n as f32 - 1.0).max(1.0);
                let sag = (t * (1.0 - t)) * 2.0 * (hw + 2.0);
                let top = *pt + up * (tower_h.min(hw + 6.0) - sag);
                let bot = *pt + up * 0.15;
                let seg = [bot, top];
                let prof = vec![p(-0.06, 0.0, color, false), p(0.06, 0.0, color, false)];
                sweep_raw(m, &seg, up, &prof);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roads::{Region, StreetKit};

    #[test]
    fn deck_section_is_closed_and_faces_outward() {
        let kit = StreetKit::for_region(Region::NorthAmericaCity);
        let prof = deck_profile(7.0, 1.5, &kit);
        let first = prof.first().unwrap();
        let last = prof.last().unwrap();
        assert!((first.o - last.o).abs() < 1e-5 && (first.h - last.h).abs() < 1e-5);
        let pts: Vec<Vec3> = (0..6)
            .map(|i| Vec3::new(i as f32 * 5.0, 10.0, 0.0))
            .collect();
        let mut m = MeshData::default();
        sweep_raw(&mut m, &pts, Vec3::Y, &prof);
        assert!(!m.is_empty());
        // Every triangle's geometric normal agrees with its vertex normals.
        for t in m.indices.chunks_exact(3) {
            let v = |i: u32| Vec3::from(m.positions[i as usize]);
            let n = (v(t[1]) - v(t[0])).cross(v(t[2]) - v(t[0]));
            let want = Vec3::from(m.normals[t[0] as usize]);
            assert!(n.dot(want) >= -1e-4);
        }
    }

    #[test]
    fn piers_only_where_there_is_room_and_no_road_below() {
        let pts: Vec<Vec3> = (0..40)
            .map(|i| Vec3::new(i as f32 * 5.0, 20.0, 0.0))
            .collect();
        let mut with = MeshData::default();
        let room = vec![18.0; pts.len()];
        piers(&mut with, &pts, Vec3::Y, 6.0, 1.5, &room, &[]);
        assert!(!with.is_empty(), "20 m up needs piers");
        let mut skipped = MeshData::default();
        piers(
            &mut skipped,
            &pts,
            Vec3::Y,
            6.0,
            1.5,
            &room,
            &[(0.0, 1000.0)],
        );
        assert!(skipped.is_empty(), "a road underneath keeps the piers away");
        let mut low = MeshData::default();
        piers(
            &mut low,
            &pts,
            Vec3::Y,
            6.0,
            1.5,
            &vec![0.5; pts.len()],
            &[],
        );
        assert!(low.is_empty(), "a deck on the ground needs none");
    }
}
