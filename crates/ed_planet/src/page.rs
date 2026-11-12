//! Height pages: the decoded `terrain` chunk grid (P2-D3) and its sampling rule.

use ed_geo::BodyId;
pub use ed_geo::{HeightPage, PAGE_CELLS, PAGE_N};
use glam::DVec3;
use std::f64::consts::FRAC_PI_4;

/// A direction expressed in lattice terms: the cube face and the fractional position on the
/// face in `[0, 1]²` (linear in lattice index, the inverse of `ed_geo::node::warp`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FacePos {
    pub body: BodyId,
    pub face: u8,
    pub s: f64,
    pub t: f64,
}

impl FacePos {
    /// Same face choice as `NodeId::from_unit`.
    pub fn from_dir(body: BodyId, p: DVec3) -> FacePos {
        let a = p.abs();
        let axis = if a.x >= a.y && a.x >= a.z { 0 } else if a.y >= a.z { 1 } else { 2 };
        let m = [p.x, p.y, p.z][axis];
        let face = (axis * 2) as u8 + (m < 0.0) as u8;
        let k = m.abs();
        let c = [p.x / k, p.y / k, p.z / k];
        let (u, v) = match face {
            0 => (c[1], c[2]),
            1 => (-c[1], c[2]),
            2 => (-c[0], c[2]),
            3 => (c[0], c[2]),
            4 => (c[1], -c[0]),
            _ => (c[1], c[0]),
        };
        let lin = |w: f64| (libm::atan(w.clamp(-1.0, 1.0)) / FRAC_PI_4 + 1.0) * 0.5;
        FacePos { body, face, s: lin(u), t: lin(v) }
    }

    /// The node at `level` holding this position and the node-local `(u, v)` in `[0, 1]`.
    pub fn node(&self, level: u8) -> (ed_geo::NodeId, f64, f64) {
        let n = (1u64 << level) as f64;
        let (fx, fy) = (self.s * n, self.t * n);
        let x = (fx.floor().max(0.0) as u32).min((1u32 << level) - 1);
        let y = (fy.floor().max(0.0) as u32).min((1u32 << level) - 1);
        let id = ed_geo::NodeId::new(self.body, self.face, level, x, y).expect("valid node");
        (id, (fx - x as f64).clamp(0.0, 1.0), (fy - y as f64).clamp(0.0, 1.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed_geo::NodeId;

    #[test]
    fn face_pos_matches_from_unit() {
        let b = BodyId(3);
        let mut seed = 7u64;
        for _ in 0..2000 {
            let mut r = || {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                (seed >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
            };
            let p = DVec3::new(r(), r(), r());
            let fp = FacePos::from_dir(b, p);
            for level in [0u8, 3, 9, 15] {
                let (id, u, v) = fp.node(level);
                let want = NodeId::from_unit(b, p, level);
                if id != want {
                    // Only allowed on a cell boundary, where both answers are adjacent.
                    assert!(u < 1e-9 || u > 1.0 - 1e-9 || v < 1e-9 || v > 1.0 - 1e-9, "{id} vs {want}");
                }
            }
        }
    }
}
