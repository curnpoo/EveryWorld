//! One cloud layer: a height band with its own coverage, cloud type, wind and look. The three
//! bands of a sky come from [`crate::cloudscape::weather::CloudWeather::layers`].

use glam::Vec2;

/// Which band a layer is (fixed order: the GPU block stores them in this order).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerKind {
    /// Cumulus humilis to congestus, or a stratocumulus / stratus deck (base at the lifting
    /// condensation level). Thunderstorm cells live in this band and grow through the others.
    Low,
    /// Altocumulus patches and altostratus sheets (3 to 7 km).
    Mid,
    /// Cirrus streaks and cirrostratus veils (7 to 12 km).
    High,
}

impl LayerKind {
    pub const ALL: [LayerKind; 3] = [LayerKind::Low, LayerKind::Mid, LayerKind::High];
}

/// A cloud layer. Heights are metres above the sea-level datum; winds are m/s in the render
/// frame's horizontal axes as `(east, north)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CloudLayer {
    pub kind: LayerKind,
    /// Cloud base (m). Cumulus bases are flat at this height.
    pub base_m: f32,
    /// Highest top any cloud in the layer reaches (m). Individual clouds stop lower; the
    /// weather map's tower channel decides how tall each one is.
    pub top_m: f32,
    /// Fraction of the sky the layer covers, 0..1.
    pub coverage: f32,
    /// 0: a sheet (stratus, altostratus, cirrostratus), 1: heaps (cumulus, altocumulus).
    pub cumuliform: f32,
    /// How tall the clouds are as a fraction of the band, 0..1: the share of columns that
    /// grow to the top. Low for fair-weather cumulus, high for congestus.
    pub towering: f32,
    /// Optical density multiplier (1: a typical cumulus, ~0.06 /m extinction at the core).
    pub density: f32,
    /// Edge erosion by detail noise, 0..1. 0: hard, billowy cauliflower edges; 1: torn and
    /// wispy (fractus, cirrus fibres).
    pub wispiness: f32,
    /// Horizontal size of the cloud cells (m): spacing of individual clouds.
    pub cell_m: f32,
    /// Stretch along the wind (1: round cells; cirrus streaks are 4 to 8).
    pub stretch: f32,
    /// Wind at the layer (m/s, east, north). Clouds drift with it.
    pub wind: Vec2,
    /// Horizontal offset of the tops per metre of height (m/m, east, north): towers lean
    /// downwind when the wind grows with height.
    pub shear: Vec2,
    /// Rain from this layer, 0..1 (shafts under the thickest columns, reaching the ground).
    pub precipitation: f32,
}

impl CloudLayer {
    /// An empty layer of `kind` (zero coverage).
    pub fn empty(kind: LayerKind) -> Self {
        let (base_m, top_m) = match kind {
            LayerKind::Low => (1500.0, 3000.0),
            LayerKind::Mid => (4000.0, 5500.0),
            LayerKind::High => (8500.0, 10000.0),
        };
        Self {
            kind,
            base_m,
            top_m,
            coverage: 0.0,
            cumuliform: 1.0,
            towering: 0.3,
            density: 1.0,
            wispiness: 0.3,
            cell_m: 3000.0,
            stretch: 1.0,
            wind: Vec2::ZERO,
            shear: Vec2::ZERO,
            precipitation: 0.0,
        }
    }

    pub fn thickness(&self) -> f32 {
        (self.top_m - self.base_m).max(1.0)
    }

    /// True when the layer can contain cloud anywhere.
    pub fn active(&self) -> bool {
        self.coverage > 0.003 && self.density > 0.0
    }

    /// Lowest altitude anything of this layer reaches, rain shafts included.
    pub fn floor_m(&self, ground_m: f32) -> f32 {
        if self.precipitation > 0.0 { ground_m } else { self.base_m }
    }
}
