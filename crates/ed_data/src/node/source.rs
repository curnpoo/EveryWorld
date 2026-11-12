//! Raster sources behind a trait, so tests can serve synthetic tiles.

use crate::DiskCache;
use ed_geo::tile::TileId;
use std::sync::Arc;

/// A square raster of heights in metres, row 0 = north edge, column 0 = west edge, pixel
/// centres on a regular grid that exactly tiles the world (`size` pixels per tile).
#[derive(Debug, Clone)]
pub struct Raster {
    pub size: usize,
    pub heights: Vec<f32>,
}

/// Where node heights come from. A tile is addressed by Web-Mercator `(z, x, y)` as plain
/// integers; `None` means "no data" and is sampled as 0 m (open ocean surface).
pub trait SourceView {
    fn elevation_tile(&self, z: u8, x: u32, y: u32) -> Option<Arc<Raster>>;
}

/// The real source: `elevation::fetch` through the disk cache (network on a miss).
pub struct ElevationSource<'a> {
    pub cache: &'a DiskCache,
}

impl SourceView for ElevationSource<'_> {
    fn elevation_tile(&self, z: u8, x: u32, y: u32) -> Option<Arc<Raster>> {
        let t = crate::elevation::fetch(self.cache, TileId::new(z, x, y)).ok()?;
        Some(Arc::new(Raster { size: t.size, heights: t.heights }))
    }
}
