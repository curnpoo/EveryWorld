//! Sun shadow cascades, re-planned from the camera's height above ground
//! (`docs/plans/shadows.md`, stage 1). The plan itself is engine-free
//! (`ed_runtime::shadows`); this is the Bevy glue.
//!
//! `EW_SHADOWS=legacy` restores the M0 config (2 cascades, 700 m, hard cut)
//! for A/B timing on the Mac. The far fade lives in the patched
//! `vendor/bevy_pbr/src/render/shadows.wesl`.

use avian3d::prelude::{SpatialQuery, SpatialQueryFilter};
use bevy::light::{CascadeShadowConfig, CascadeShadowConfigBuilder};
use bevy::prelude::*;
use ed_runtime::shadows::{CascadePlan, LEGACY};

use crate::origin::WorldOrigin;
use crate::sky::Sun;

pub struct ShadowsPlugin;

impl Plugin for ShadowsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, replan_cascades);
    }
}

fn legacy() -> bool {
    std::env::var("EW_SHADOWS").is_ok_and(|v| v == "legacy")
}

/// The cascade config for `agl_m` (or the legacy one).
pub fn cascade_config(agl_m: f32) -> CascadeShadowConfig {
    let p = if legacy() { LEGACY } else { CascadePlan::for_agl(agl_m) };
    builder(&p).build()
}

fn builder(p: &CascadePlan) -> CascadeShadowConfigBuilder {
    let (num_cascades, maximum_distance) = if cfg!(target_os = "windows") {
        (p.num.min(2), p.max.min(300.0))
    } else {
        (p.num, p.max)
    };
    CascadeShadowConfigBuilder {
        num_cascades,
        first_cascade_far_bound: p.first_far,
        maximum_distance,
        overlap_proportion: p.overlap,
        ..default()
    }
}

/// Height of the camera above whatever collider is under it (ground, roofs),
/// falling back to geodetic altitude when nothing is loaded below.
fn camera_agl(spatial: &SpatialQuery, origin: &WorldOrigin, pos: Vec3) -> f32 {
    const PROBE_UP: f32 = 2.0;
    let start = pos + Vec3::Y * PROBE_UP;
    let hit = spatial.cast_ray(start, Dir3::NEG_Y, 40_000.0, true, &SpatialQueryFilter::default());
    match hit {
        Some(h) => (h.distance - PROBE_UP).max(0.0),
        None => origin.to_geodetic(pos).alt_m.max(0.0) as f32,
    }
}

/// Re-plans when the quantised AGL bucket changes (a few times per climb,
/// never while walking), so cascades do not breathe frame to frame.
fn replan_cascades(
    spatial: SpatialQuery,
    origin: Res<WorldOrigin>,
    cams: Query<&GlobalTransform, (With<Camera3d>, Without<crate::globe::GlobeCamera>)>,
    mut sun: Query<&mut CascadeShadowConfig, With<Sun>>,
    mut last: Local<Option<CascadePlan>>,
    mut frame: Local<u32>,
) {
    *frame = frame.wrapping_add(1);
    if legacy() || *frame % 8 != 0 {
        return;
    }
    let (Some(cam), Ok(mut cfg)) = (cams.iter().next(), sun.single_mut()) else { return };
    let plan = CascadePlan::for_agl(camera_agl(&spatial, &origin, cam.translation()));
    if *last != Some(plan) {
        *cfg = builder(&plan).build();
        *last = Some(plan);
    }
}
