//! Canopy page: the 257 x 257 x 4-byte vegetation grid of one terrain node (trees design §0),
//! beside [`HeightPage`](crate::HeightPage) and on the same lattice (`node.grid_point(i, j, 256)`).
//! Pure data and decoding; producers (`ed_data::node::canopy`) fill it, consumers sample it.
//!
//! One `[u8; 4]` per sample:
//! 0. **cover**: crown cover fraction, `0..=255` is `0..=1`.
//! 1. **height**: canopy top in metres, log-coded (see [`encode_height`]).
//! 2. **mix**: conifer share, `0..=255` is `0..=1` (broadleaf is the rest).
//! 3. **under**: shrub plus ground-cover density, `0..=255` is `0..=1`.

use crate::height_page::{PAGE_CELLS, PAGE_N, PAGE_SAMPLES};

/// Canopy height at code 255 (m). Code 0 means "no canopy".
pub const HEIGHT_MAX_M: f32 = 90.0;
/// Canopy height at code 1 (m): the smallest representable crown top.
pub const HEIGHT_MIN_M: f32 = 0.5;

/// Log step between height codes: `(90 / 0.5)^(1/254)`, about 2.1 % per code (so a rounded
/// value is within ~1 % of the truth).
fn ratio_ln() -> f32 {
    libm::logf(HEIGHT_MAX_M / HEIGHT_MIN_M) / 254.0
}

/// Metres to the height code. `h < 0.25` m (and non-finite) is 0 (no canopy); heights above
/// 90 m clamp. Round trip error is under ~1.1 %.
pub fn encode_height(h: f32) -> u8 {
    if !(h >= HEIGHT_MIN_M * 0.5) {
        return 0;
    }
    let q = 1.0 + libm::logf(h.max(HEIGHT_MIN_M) / HEIGHT_MIN_M) / ratio_ln();
    libm::roundf(q).clamp(1.0, 255.0) as u8
}

/// Height code to metres (0 for code 0). Uses the software `exp` so every platform agrees.
pub fn decode_height(q: u8) -> f32 {
    if q == 0 {
        return 0.0;
    }
    HEIGHT_MIN_M * libm::expf((q - 1) as f32 * ratio_ln())
}

/// A canopy page. `samples[j * 257 + i] = [cover, height, mix, under]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanopyPage {
    pub samples: Vec<[u8; 4]>,
}

/// Byte length of [`CanopyPage::to_bytes`].
pub const CANOPY_BYTES: usize = PAGE_SAMPLES * 4;

impl CanopyPage {
    /// Builds a page from a function of the lattice indices `(i, j)`.
    pub fn from_fn(mut f: impl FnMut(usize, usize) -> [u8; 4]) -> CanopyPage {
        let mut samples = Vec::with_capacity(PAGE_SAMPLES);
        for j in 0..PAGE_N {
            for i in 0..PAGE_N {
                samples.push(f(i, j));
            }
        }
        CanopyPage { samples }
    }

    /// An empty page (no canopy anywhere).
    pub fn empty() -> CanopyPage {
        CanopyPage { samples: vec![[0; 4]; PAGE_SAMPLES] }
    }

    #[inline]
    pub fn get(&self, i: usize, j: usize) -> [u8; 4] {
        self.samples[j * PAGE_N + i]
    }

    /// Crown cover fraction `0..=1` at sample `(i, j)`.
    #[inline]
    pub fn cover(&self, i: usize, j: usize) -> f32 {
        self.get(i, j)[0] as f32 / 255.0
    }
    /// Canopy top in metres at sample `(i, j)`.
    #[inline]
    pub fn height_m(&self, i: usize, j: usize) -> f32 {
        decode_height(self.get(i, j)[1])
    }
    /// Conifer share `0..=1` at sample `(i, j)`.
    #[inline]
    pub fn mix(&self, i: usize, j: usize) -> f32 {
        self.get(i, j)[2] as f32 / 255.0
    }
    /// Understory density `0..=1` at sample `(i, j)`.
    #[inline]
    pub fn under(&self, i: usize, j: usize) -> f32 {
        self.get(i, j)[3] as f32 / 255.0
    }

    /// Decoded `[cover, height m, mix, under]` at node-local `(u, v)` in `[0, 1]`, bilinear.
    /// Height is interpolated in metres but weighted by cover, so a forest edge does not drag
    /// the canopy top toward the open ground beside it.
    pub fn bilinear(&self, u: f64, v: f64) -> [f32; 4] {
        let s = PAGE_CELLS as f64;
        let fx = (u.clamp(0.0, 1.0) * s).min(s - 1e-9);
        let fy = (v.clamp(0.0, 1.0) * s).min(s - 1e-9);
        let (i, j) = (fx.floor() as usize, fy.floor() as usize);
        let (tx, ty) = ((fx - i as f64) as f32, (fy - j as f64) as f32);
        let w = [(1.0 - tx) * (1.0 - ty), tx * (1.0 - ty), (1.0 - tx) * ty, tx * ty];
        let px = [self.get(i, j), self.get(i + 1, j), self.get(i, j + 1), self.get(i + 1, j + 1)];
        let mut cover = 0.0;
        let (mut hsum, mut hw) = (0.0, 0.0);
        let (mut mix, mut under) = (0.0, 0.0);
        for k in 0..4 {
            let c = px[k][0] as f32 / 255.0;
            cover += w[k] * c;
            let ww = w[k] * c;
            hsum += ww * decode_height(px[k][1]);
            hw += ww;
            mix += ww * px[k][2] as f32 / 255.0;
            under += w[k] * px[k][3] as f32 / 255.0;
        }
        let (height, mix) = if hw > 0.0 { (hsum / hw, mix / hw) } else { (0.0, 0.0) };
        [cover, height, mix, under]
    }

    /// Canonical bytes: the samples in row order, 4 bytes each (for hashing and a future chunk).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(CANOPY_BYTES);
        for s in &self.samples {
            b.extend_from_slice(s);
        }
        b
    }

    /// Inverse of [`to_bytes`](Self::to_bytes); `None` when the length is wrong.
    pub fn from_bytes(b: &[u8]) -> Option<CanopyPage> {
        if b.len() != CANOPY_BYTES {
            return None;
        }
        Some(CanopyPage { samples: b.chunks_exact(4).map(|c| [c[0], c[1], c[2], c[3]]).collect() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn height_code_round_trip() {
        assert_eq!(encode_height(0.0), 0);
        assert_eq!(decode_height(0), 0.0);
        assert_eq!(encode_height(1000.0), 255);
        assert!((decode_height(255) - 90.0).abs() < 0.2);
        let mut h = 0.6f32;
        while h < 89.0 {
            let d = decode_height(encode_height(h));
            assert!((d - h).abs() / h < 0.012, "{h} -> {d}");
            h *= 1.07;
        }
        // Monotone codes.
        for q in 1..255u8 {
            assert!(decode_height(q + 1) > decode_height(q));
        }
    }

    #[test]
    fn bytes_round_trip_and_sampling() {
        let p = CanopyPage::from_fn(|i, j| [(i % 256) as u8, encode_height(20.0), 100, (j % 256) as u8]);
        let b = p.to_bytes();
        assert_eq!(b.len(), CANOPY_BYTES);
        assert_eq!(CanopyPage::from_bytes(&b).unwrap(), p);
        assert!(CanopyPage::from_bytes(&b[1..]).is_none());
        // Sample at lattice point (10, 20) is exact.
        let v = p.bilinear(10.0 / 256.0, 20.0 / 256.0);
        assert!((v[0] - 10.0 / 255.0).abs() < 1e-5);
        assert!((v[3] - 20.0 / 255.0).abs() < 1e-5);
        assert!((v[1] - decode_height(encode_height(20.0))).abs() < 1e-3);
        // Halfway between two cover values.
        let v = p.bilinear(10.5 / 256.0, 0.0);
        assert!((v[0] - 10.5 / 255.0).abs() < 1e-5);
    }

    #[test]
    fn edge_does_not_drag_height() {
        let p = CanopyPage::from_fn(|i, _| if i < 128 { [255, encode_height(30.0), 255, 0] } else { [0, 0, 0, 0] });
        let v = p.bilinear(127.5 / 256.0, 0.5);
        assert!((v[1] - 30.0).abs() < 0.7, "{v:?}");
        assert!((v[0] - 0.5).abs() < 1e-5);
    }
}
