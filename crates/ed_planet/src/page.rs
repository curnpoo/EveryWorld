//! Height pages: the decoded `terrain` chunk grid (P2-D3) and its sampling rule.

use ed_geo::BodyId;
use glam::DVec3;
use std::f64::consts::FRAC_PI_4;

/// Samples per side of a height page (256 cells).
pub const PAGE_SIZE: usize = 257;
/// Cells per side of a height page.
pub const PAGE_CELLS: usize = PAGE_SIZE - 1;

/// One node's height grid: `PAGE_SIZE²` quantised samples, row-major with `i` along the
/// node's lattice x and `j` along its lattice y (`samples[j * PAGE_SIZE + i]`). Sample
/// `(i, j)` sits on lattice point `node.grid_point(i, j, 256)`. Metres above the ellipsoid
/// are `offset + scale * q`, decoded in f64.
#[derive(Clone, Debug, PartialEq)]
pub struct HeightPage {
    pub samples: Vec<u16>,
    pub scale: f32,
    pub offset: f32,
    /// Decoded minimum and maximum of the samples (m).
    pub min: f32,
    pub max: f32,
}

impl HeightPage {
    /// Builds a page from the metres at each sample, quantised over their own range.
    pub fn from_fn(mut f: impl FnMut(usize, usize) -> f64) -> HeightPage {
        let mut raw = Vec::with_capacity(PAGE_SIZE * PAGE_SIZE);
        for j in 0..PAGE_SIZE {
            for i in 0..PAGE_SIZE {
                raw.push(f(i, j));
            }
        }
        let lo = raw.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = raw.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let offset = lo as f32;
        let scale = (((hi - offset as f64) / 65_535.0) as f32).max(1e-6);
        let samples = raw.iter().map(|&h| ((h - offset as f64) / scale as f64).round().clamp(0.0, 65_535.0) as u16).collect();
        let mut page = HeightPage { samples, scale, offset, min: 0.0, max: 0.0 };
        let (mn, mx) = page.samples.iter().fold((u16::MAX, 0u16), |(a, b), &q| (a.min(q), b.max(q)));
        page.min = page.decode(mn) as f32;
        page.max = page.decode(mx) as f32;
        page
    }

    /// A page of one height.
    pub fn flat(h: f32) -> HeightPage {
        HeightPage { samples: vec![0; PAGE_SIZE * PAGE_SIZE], scale: 1.0, offset: h, min: h, max: h }
    }

    pub fn decode(&self, q: u16) -> f64 {
        self.offset as f64 + self.scale as f64 * q as f64
    }

    /// Metres at sample `(i, j)`.
    pub fn get(&self, i: usize, j: usize) -> f64 {
        self.decode(self.samples[j * PAGE_SIZE + i])
    }

    /// Metres at node-local coordinates `(u, v)` in `[0, 1]` (lattice x, lattice y).
    ///
    /// Each cell is two triangles split on the 00-11 diagonal, as the alpha's `TerrainGrid`,
    /// so the collider and the picture agree.
    pub fn sample(&self, u: f64, v: f64) -> f64 {
        let s = PAGE_CELLS as f64;
        let fx = (u.clamp(0.0, 1.0) * s).min(s - 1e-9);
        let fy = (v.clamp(0.0, 1.0) * s).min(s - 1e-9);
        let (i, j) = (fx.floor() as usize, fy.floor() as usize);
        let (tx, ty) = (fx - i as f64, fy - j as f64);
        let (h00, h10, h01, h11) = (self.get(i, j), self.get(i + 1, j), self.get(i, j + 1), self.get(i + 1, j + 1));
        if tx >= ty { h00 + (h10 - h00) * tx + (h11 - h10) * ty } else { h00 + (h11 - h01) * tx + (h01 - h00) * ty }
    }
}

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
    fn diagonal_rule_and_quantisation() {
        let p = HeightPage::from_fn(|i, j| 100.0 + i as f64 * 0.5 - j as f64 * 0.25);
        assert!((p.get(10, 20) - (100.0 + 5.0 - 5.0)).abs() < 1e-3);
        // A plane is reproduced exactly by either triangle.
        let h = p.sample(0.3, 0.7);
        let want = 100.0 + 0.3 * 256.0 * 0.5 - 0.7 * 256.0 * 0.25;
        assert!((h - want).abs() < 1e-3, "{h} vs {want}");
        // Off-plane cell: the 00-11 diagonal decides.
        let mut q = HeightPage::flat(0.0);
        q.samples[1] = 10; // (1, 0): only the lower-right triangle (00, 10, 11) sees it
        assert!((q.sample(0.75 / 256.0, 0.25 / 256.0) - 5.0).abs() < 1e-9);
        assert!(q.sample(0.25 / 256.0, 0.75 / 256.0).abs() < 1e-9);
    }

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
