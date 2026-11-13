//! Height page: the 257 x 257 u16 height grid of one terrain node (P2-D3), pure data and
//! decoding. Producers (`ed_data::node`, tests) fill it; consumers (`ed_planet`) sample it.

/// Cells per page side.
pub const PAGE_CELLS: usize = 256;
/// Samples per page side.
pub const PAGE_N: usize = PAGE_CELLS + 1;
pub const PAGE_SAMPLES: usize = PAGE_N * PAGE_N;
/// Quantisation step in metres of the canonical (data-layer) pages: 65535 steps cover 32.7 km,
/// more than sea floor to summit.
pub const HEIGHT_STEP_M: f32 = 0.5;
/// Step at [`FINE_FROM_LEVEL`]: 1/8 m; 65535 steps still cover 8.19 km, more relief than one
/// level-11 node (~5 km wide) holds.
pub const HEIGHT_STEP_FINE_M: f32 = 0.125;
/// First node level that uses [`HEIGHT_STEP_FINE_M`].
pub const FINE_FROM_LEVEL: u8 = 11;
/// From this level the step halves per level (the node halves, so the range 65535 steps cover
/// stays ~3.3x the node width: a 73 degree mean slope corner to corner).
pub const HALVING_FROM_LEVEL: u8 = 12;
/// Finest step (level 17 and up on Earth).
pub const HEIGHT_STEP_MIN_M: f32 = 1.0 / 256.0;

/// Quantisation step of the canonical pages of node level `level`: a fixed rule per level, so
/// same-level neighbours decode a shared edge to the same bits. Near the ground the step must
/// sit far below the source's own precision: the shading normal differentiates the page over a
/// metre or two, and a 1/8 m step there drew every quantum as a contour line (seen from 50 m on
/// the Zermatt valley floor). Powers of two, so decoding is exact.
pub fn height_step_for_level(level: u8) -> f32 {
    if level < FINE_FROM_LEVEL {
        HEIGHT_STEP_M
    } else {
        (HEIGHT_STEP_FINE_M / (1u32 << level.saturating_sub(HALVING_FROM_LEVEL).min(16)) as f32).max(HEIGHT_STEP_MIN_M)
    }
}

/// `height(i, j) = offset + scale * samples[j * 257 + i]`, decoded in f64. `i` runs along the
/// node's lattice x (`node.grid_point(i, j, 256)`), `j` along its lattice y.
#[derive(Clone, Debug, PartialEq)]
pub struct HeightPage {
    pub samples: Vec<u16>,
    pub scale: f32,
    pub offset: f32,
    /// Decoded minimum and maximum of the samples (m).
    pub min_m: f32,
    pub max_m: f32,
}

impl HeightPage {
    /// Builds a page from the metres at each sample, quantised over their own range.
    pub fn from_fn(mut f: impl FnMut(usize, usize) -> f64) -> HeightPage {
        let mut raw = Vec::with_capacity(PAGE_SAMPLES);
        for j in 0..PAGE_N {
            for i in 0..PAGE_N {
                raw.push(f(i, j));
            }
        }
        let lo = raw.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = raw.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let offset = lo as f32;
        let scale = (((hi - offset as f64) / 65_535.0) as f32).max(1e-6);
        let samples = raw.iter().map(|&h| ((h - offset as f64) / scale as f64).round().clamp(0.0, 65_535.0) as u16).collect();
        let mut page = HeightPage { samples, scale, offset, min_m: 0.0, max_m: 0.0 };
        let (mn, mx) = page.samples.iter().fold((u16::MAX, 0u16), |(a, b), &q| (a.min(q), b.max(q)));
        page.min_m = page.decode(mn) as f32;
        page.max_m = page.decode(mx) as f32;
        page
    }

    /// A page of one height.
    pub fn flat(h: f32) -> HeightPage {
        HeightPage { samples: vec![0; PAGE_SAMPLES], scale: 1.0, offset: h, min_m: h, max_m: h }
    }

    #[inline]
    pub fn decode(&self, q: u16) -> f64 {
        self.offset as f64 + self.scale as f64 * q as f64
    }

    /// Metres at sample `(i, j)`.
    #[inline]
    pub fn height(&self, i: usize, j: usize) -> f64 {
        self.decode(self.samples[j * PAGE_N + i])
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
        let (h00, h10, h01, h11) = (self.height(i, j), self.height(i + 1, j), self.height(i, j + 1), self.height(i + 1, j + 1));
        if tx >= ty { h00 + (h10 - h00) * tx + (h11 - h10) * ty } else { h00 + (h11 - h01) * tx + (h01 - h00) * ty }
    }

    /// Canonical byte form (little endian) for hashing and byte-identity tests.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(self.samples.len() * 2 + 16);
        for s in &self.samples {
            b.extend_from_slice(&s.to_le_bytes());
        }
        for f in [self.scale, self.offset, self.min_m, self.max_m] {
            b.extend_from_slice(&f.to_bits().to_le_bytes());
        }
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagonal_rule_and_quantisation() {
        let p = HeightPage::from_fn(|i, j| 100.0 + i as f64 * 0.5 - j as f64 * 0.25);
        assert!((p.height(10, 20) - (100.0 + 5.0 - 5.0)).abs() < 1e-3);
        let h = p.sample(0.3, 0.7);
        let want = 100.0 + 0.3 * 256.0 * 0.5 - 0.7 * 256.0 * 0.25;
        assert!((h - want).abs() < 1e-3, "{h} vs {want}");
        let mut q = HeightPage::flat(0.0);
        q.samples[1] = 10; // (1, 0): only the lower-right triangle (00, 10, 11) sees it
        assert!((q.sample(0.75 / 256.0, 0.25 / 256.0) - 5.0).abs() < 1e-9);
        assert!(q.sample(0.25 / 256.0, 0.75 / 256.0).abs() < 1e-9);
    }

    #[test]
    fn step_rule_halves_with_the_node() {
        let steps: Vec<f32> = (9..=19).map(height_step_for_level).collect();
        assert_eq!(steps, [0.5, 0.5, 0.125, 0.125, 0.0625, 0.03125, 1.0 / 64.0, 1.0 / 128.0, 1.0 / 256.0, 1.0 / 256.0, 1.0 / 256.0]);
        // 65535 steps cover > 3x the node width (Earth: face edge ~10 018 km / 2^level).
        for l in HALVING_FROM_LEVEL..=17 {
            let width = 10_018_000.0 / (1u64 << l) as f64;
            assert!(65_535.0 * height_step_for_level(l) as f64 > 3.0 * width, "level {l}");
        }
    }

    #[test]
    fn bytes_layout() {
        let p = HeightPage::flat(3.0);
        assert_eq!(p.to_bytes().len(), PAGE_SAMPLES * 2 + 16);
    }
}
