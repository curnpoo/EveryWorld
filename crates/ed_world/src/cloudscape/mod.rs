//! Cloudscape: the volumetric cloud field of every world (Phase 2 item 5, P2-D11 superseded by
//! the owner's 2026-11-12 decision: Nubis-style raymarched volumes in several layers,
//! procedural and weather-driven, lit from above and below). Design: `docs/plans/clouds.md`.
//!
//! Engine-free half. Everything here is a pure function of (seed, weather, time, position):
//! no state, no wall clock, no map iteration, so every client, the sim and the renderer see
//! the same clouds, and the CPU reference ([`light::march`]) draws exactly what the GPU pass
//! (`ed_render/shaders/clouds.slang`) draws.
//!
//! Pipeline, top to bottom:
//! - [`weather::CloudWeather`] is the interface the climate sim (`ed_climate`, O3-W4 layer 4)
//!   fills: humidity per level, instability, dew-point spread, fronts, winds aloft,
//!   precipitation. Presets stand in for the sim until it exists.
//! - [`weather::CloudWeather::layers`] turns it into three [`layer::CloudLayer`]s: low
//!   (cumulus to cumulonimbus, or a stratocumulus deck), mid (altocumulus / altostratus) and
//!   high (cirrus). Each has its own height band, coverage, cloud type, wind and shear.
//! - [`storm::storm_cells`] places thunderstorm cells deterministically: they drift with the
//!   steering wind, grow, mature with an anvil and rain, and decay.
//! - [`noise::NoiseBank`] holds the tileable noise volumes (shape, detail) and the 2D weather
//!   noise, baked once from a seed. The GPU gets the same bytes.
//! - [`field::CloudField`] is the density at any point: a weather map (where clouds are, how
//!   tall), a height profile per cloud type (flat bases, domed tops, anvils), shape noise
//!   carved by that envelope, then eroded by detail noise. Detail fades with distance so far
//!   clouds never shimmer.
//! - [`light`] is the lighting model (dual-lobe phase, multiple-scattering octaves, powder,
//!   sky and ground ambient) and the reference ray march.
//! - [`gpu`] packs one frame of all this into words for the renderer.
//!
//! Frame: the renderer's world frame, metres, x east, y up, z -north. Altitude is metres
//! above the world's sea-level datum; the curved-planet height of a point comes from the
//! marcher, not from y alone.

pub mod field;
pub mod gpu;
pub mod layer;
pub mod light;
pub mod noise;
pub mod storm;
pub mod weather;

pub use field::{CloudField, Lod};
pub use layer::{CloudLayer, LayerKind};
pub use noise::NoiseBank;
pub use storm::StormCell;
pub use weather::CloudWeather;

/// Planet radius used by the marcher for cloud-shell curvature (m). The atmosphere model's Earth radius, the
/// same value as `ed_render::atmosphere_model::Body::earth().radius_km`.
pub const EARTH_RADIUS_M: f64 = 6_360_000.0;

#[inline]
pub(crate) fn saturate(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

/// `v` from `[lo, hi]` to `[nlo, nhi]`, unclamped (Nubis `remap`).
#[inline]
pub(crate) fn remap(v: f32, lo: f32, hi: f32, nlo: f32, nhi: f32) -> f32 {
    nlo + (v - lo) / (hi - lo).max(1e-6) * (nhi - nlo)
}

#[inline]
pub(crate) fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = saturate((x - e0) / (e1 - e0));
    t * t * (3.0 - 2.0 * t)
}

#[inline]
pub(crate) fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
