//! Ground collision surface of one terrain node (P-slice S3): the same 65 x 65 grid the renderer
//! draws (`ed_render::terrain`), expressed in the sim frame, so a collider and the picture agree.
//!
//! Rule: grid vertex `(i, j)` is at body point `body.point(id.grid_point(4*step*i, 4*step*j, 256), h)`
//! with `h = page.height(4*step*i, 4*step*j)`. Quads split on the 00-11 diagonal. Positions are
//! formed in f64 and made relative to the node's origin before the f32 narrowing.

use crate::anchor::Anchor;
use crate::page::HeightPage;
use ed_geo::NodeId;
use glam::{DVec3, Vec3};

/// Render grid cells per node side; with `step` s the surface has `CELLS / s` cells.
pub const CELLS: u32 = 64;
/// Page samples between render grid vertices.
pub const STRIDE: u32 = 4;

/// Surface of `id` at `step` (1 = every render grid vertex, 2 = every other one, ...; a divisor of 64).
/// Returns the node origin in the sim frame, vertices relative to it, and triangles wound
/// counter-clockwise seen from above (local up), as `ed_sim` meshes want.
pub fn node_surface(anchor: &Anchor, id: NodeId, page: &HeightPage, step: u32) -> (DVec3, Vec<Vec3>, Vec<[u32; 3]>) {
    assert!(step >= 1 && CELLS.is_multiple_of(step), "step must divide {CELLS}");
    let n = CELLS / step;
    let side = n + 1;
    let mid = 0.5 * (page.min_m as f64 + page.max_m as f64);
    let origin = anchor.body_to_sim(anchor.body.point(id.centre(), mid));
    let stride = (STRIDE * step) as usize;
    let mut verts = Vec::with_capacity((side * side) as usize);
    for j in 0..side {
        for i in 0..side {
            let (pi, pj) = (i as usize * stride, j as usize * stride);
            let h = page.height(pi, pj);
            let b = anchor.body.point(id.grid_point(pi as u32, pj as u32, 256), h);
            verts.push((anchor.body_to_sim(b) - origin).as_vec3());
        }
    }
    let mut tris = Vec::with_capacity((n * n * 2) as usize);
    for j in 0..n {
        for i in 0..n {
            let v00 = j * side + i;
            let (v10, v01) = (v00 + 1, v00 + side);
            let v11 = v01 + 1;
            tris.push([v00, v10, v11]);
            tris.push([v00, v11, v01]);
        }
    }
    // Wind to face up: judged once on the whole grid (lattice handedness differs per cube face).
    let up = anchor.up_sim(origin).as_vec3();
    let mut s = 0.0;
    for t in &tris {
        let (a, b, c) = (verts[t[0] as usize], verts[t[1] as usize], verts[t[2] as usize]);
        s += (b - a).cross(c - a).dot(up);
    }
    if s < 0.0 {
        for t in &mut tris {
            t.swap(1, 2);
        }
    }
    (origin, verts, tris)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::Body;

    fn setup() -> (Anchor, NodeId) {
        let a = Anchor::from_ecef(Body::earth(), ed_geo::Geodetic::new(46.0207, 7.7491, 0.0).to_ecef());
        let id = NodeId::from_unit(a.body.id, a.dir, 16);
        (a, id)
    }

    #[test]
    fn vertices_match_render_rule() {
        let (a, id) = setup();
        let page = HeightPage::from_fn(|i, j| 1500.0 + 0.3 * i as f64 + 0.1 * j as f64 + ((i * j) % 7) as f64);
        let (o, v, t) = node_surface(&a, id, &page, 1);
        assert_eq!(v.len(), 65 * 65);
        assert_eq!(t.len(), 64 * 64 * 2);
        for (i, j) in [(0u32, 0u32), (64, 0), (0, 64), (64, 64), (13, 40), (32, 32)] {
            let (pi, pj) = (4 * i, 4 * j);
            let want = a.body_to_sim(a.body.point(id.grid_point(pi, pj, 256), page.height(pi as usize, pj as usize)));
            let got = o + v[(j * 65 + i) as usize].as_dvec3();
            assert!((want - got).length() < 1e-3, "({i},{j}): {want} vs {got}");
        }
        let (_, v2, t2) = node_surface(&a, id, &page, 4);
        assert_eq!((v2.len(), t2.len()), (17 * 17, 16 * 16 * 2));
    }

    #[test]
    fn flat_page_faces_up_and_is_deterministic() {
        let (a, id) = setup();
        let page = HeightPage::flat(2000.0);
        let (o, v, t) = node_surface(&a, id, &page, 1);
        for tr in &t {
            let (p, q, r) = (v[tr[0] as usize], v[tr[1] as usize], v[tr[2] as usize]);
            let n = (q - p).cross(r - p).normalize();
            let up = a.up_sim(o + p.as_dvec3()).as_vec3();
            assert!(n.dot(up) > 0.99, "{n} vs {up}");
        }
        assert_eq!(node_surface(&a, id, &page, 1), (o, v, t));
    }
}
