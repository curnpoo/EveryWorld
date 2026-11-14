//! Ground page (grass plan G2): per terrain node, `PAGE_N` x `PAGE_N` samples of land-cover
//! **shares** `[herbaceous, cropland, shrub/heath, wet]` (`0..=255` is `0..=1`), on the same
//! lattice as [`HeightPage`](crate::HeightPage) and [`CanopyPage`](crate::CanopyPage). Pure data;
//! `ed_data::node::ground` fills it, the grass scatter and the terrain carpet (G4) read it.

use crate::height_page::PAGE_N;

/// Four share bytes per lattice sample, row-major from `(0, 0)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroundPage {
    pub samples: Vec<[u8; 4]>,
}

impl GroundPage {
    pub fn from_fn(mut f: impl FnMut(usize, usize) -> [u8; 4]) -> GroundPage {
        let mut samples = Vec::with_capacity(PAGE_N * PAGE_N);
        for j in 0..PAGE_N {
            for i in 0..PAGE_N {
                samples.push(f(i, j));
            }
        }
        GroundPage { samples }
    }

    #[inline]
    pub fn get(&self, i: usize, j: usize) -> [u8; 4] {
        self.samples[j * PAGE_N + i]
    }

    /// Canonical bytes (hashing, a future chunk, the GPU pool: one little-endian u32 per sample
    /// with herb in the low byte).
    pub fn to_bytes(&self) -> Vec<u8> {
        self.samples.iter().flatten().copied().collect()
    }
}
