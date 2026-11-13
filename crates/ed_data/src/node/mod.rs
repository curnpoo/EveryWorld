//! Node-space source data (Phase 2 W2). Resamples public rasters into the cube-sphere node
//! lattice of `ed_geo::node`. No slippy-map type appears in this module's public API.

mod detail;
mod heights;
mod source;

pub use heights::{
    HeightPage, HEIGHT_STEP_FINE_M, HEIGHT_STEP_M, cubic_weights, height_step_for_level, PAGE_CELLS, PAGE_SAMPLES, node_heights, parent_heights, source_step, source_zoom, DEQUANT_FROM_LEVEL, DEQUANT_PASSES,
};
pub use detail::{erosion_detail, max_detail_m, nominal_spacing};
pub use heights::{cubic_dweights, detail_enabled};
pub use source::{ElevationSource, Raster, SourceView};
