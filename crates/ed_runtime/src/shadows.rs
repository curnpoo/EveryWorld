//! Sun shadow cascade planning (`docs/plans/shadows.md`, stage 1).
//!
//! Engine-free: picks cascade count, split bounds and fade from the camera's
//! height above ground, and predicts the shadow texel size per cascade so the
//! numbers in the plan doc are reproducible. `ed_game::shadows` applies the plan
//! to Bevy's `CascadeShadowConfig`.
//!
//! Splits follow Bevy's builder exactly (exponential between `first_far` and
//! `max`; slice `i` starts at `(1 - overlap) * bound[i-1]`; each cascade's ortho
//! box is fit to the slice's bounding-sphere diameter, rounded up to whole
//! metres), so `texel_m` matches what the GPU actually gets.

/// What the sun's cascades should cover this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CascadePlan {
    pub num: usize,
    /// Far bound of cascade 0 (view-space depth, metres).
    pub first_far: f32,
    /// Far bound of the last cascade; shadows fade to lit before this.
    pub max: f32,
    /// Fraction of each cascade blended into the next one.
    pub overlap: f32,
}

/// Cascade 0 never ends closer than this: the player, cars, street furniture.
pub const NEAR_FIRST_M: f32 = 15.0;
/// Shadow reach on the ground (walking, driving). The far fade starts at
/// `FADE_START * max` (hard-coded in the patched `shadows.wesl`).
pub const GROUND_MAX_M: f32 = 2_500.0;
/// Upper cap when flying: past this the far layer (terrain horizon maps) and
/// cloud shadows carry it; more cascade reach only blurs the texels.
pub const AIR_MAX_M: f32 = 12_000.0;
/// Must match `FAR_FADE_START` in `vendor/bevy_pbr/src/render/shadows.wesl`.
pub const FADE_START: f32 = 0.8;
/// AGL quantisation: plans only change when AGL crosses a quarter-octave step,
/// so the cascades do not breathe (and shimmer) as the camera bobs.
const STEPS_PER_OCTAVE: f32 = 4.0;

/// The legacy M0 config (two cascades to 700 m, hard cut). `EW_SHADOWS=legacy`.
pub const LEGACY: CascadePlan = CascadePlan { num: 2, first_far: 40.0, max: 700.0, overlap: 0.15 };

impl CascadePlan {
    /// Plan for a camera `agl_m` metres above the ground under it.
    ///
    /// The nearest shaded ground is roughly `agl` away in view depth, so cascade
    /// 0 starts covering useful pixels around there; the far end grows with
    /// altitude because you see further. Ratios between neighbouring cascades
    /// stay near 5.5x, which is what makes the cascade seams hard to spot.
    pub fn for_agl(agl_m: f32) -> Self {
        let agl = quantize_agl(agl_m);
        let max = (8.0 * agl).clamp(GROUND_MAX_M, AIR_MAX_M);
        // At least 4x between first and last so four cascades still mean something.
        let first_far = (0.5 * agl).clamp(NEAR_FIRST_M, max / 4.0);
        CascadePlan { num: 4, first_far, max, overlap: 0.2 }
    }

    /// Far bound of each cascade (Bevy's exponential split).
    pub fn bounds(&self) -> Vec<f32> {
        if self.num <= 1 {
            return vec![self.max];
        }
        let base = (self.max / self.first_far).powf(1.0 / (self.num as f32 - 1.0));
        (0..self.num).map(|i| self.first_far * base.powi(i as i32)).collect()
    }

    /// Near edge of each cascade's frustum slice.
    pub fn slice_nears(&self, near_plane: f32) -> Vec<f32> {
        let b = self.bounds();
        (0..self.num).map(|i| if i == 0 { near_plane } else { (1.0 - self.overlap) * b[i - 1] }).collect()
    }

    /// World-space size of one shadow-map texel per cascade (metres).
    pub fn texel_m(&self, fov_y_rad: f32, aspect: f32, near_plane: f32, map_size: u32) -> Vec<f32> {
        let b = self.bounds();
        let n = self.slice_nears(near_plane);
        (0..self.num).map(|i| slice_diameter(n[i], b[i], fov_y_rad, aspect) / map_size as f32).collect()
    }

    /// Where shadows start fading to lit (view depth).
    pub fn fade_start(&self) -> f32 {
        FADE_START * self.max
    }
}

/// Bounding diameter of a perspective frustum slice, rounded up like Bevy.
pub fn slice_diameter(near: f32, far: f32, fov_y_rad: f32, aspect: f32) -> f32 {
    let ty = (fov_y_rad * 0.5).tan();
    let tx = ty * aspect;
    let body = ((tx * far + tx * near).powi(2) + (ty * far + ty * near).powi(2) + (far - near).powi(2)).sqrt();
    let far_plane = 2.0 * (tx * far).hypot(ty * far);
    body.max(far_plane).ceil()
}

/// Snap AGL to quarter-octave steps (>= 1 m).
pub fn quantize_agl(agl_m: f32) -> f32 {
    let a = agl_m.max(1.0);
    let k = (a.log2() * STEPS_PER_OCTAVE).floor();
    (k / STEPS_PER_OCTAVE).exp2()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FOV: f32 = std::f32::consts::FRAC_PI_3;
    const ASPECT: f32 = 16.0 / 9.0;

    #[test]
    fn legacy_matches_measured_b3_numbers() {
        // 2 cascades to 700 m: 4.6 cm then 80.5 cm texels, a 17x jump at 34 m.
        let t = LEGACY.texel_m(FOV, ASPECT, 0.1, 2048);
        assert!((t[0] - 0.046).abs() < 0.002, "{t:?}");
        assert!((t[1] - 0.805).abs() < 0.01, "{t:?}");
        assert!(t[1] / t[0] > 15.0);
    }

    #[test]
    fn ground_plan_is_even_and_reaches_far() {
        let p = CascadePlan::for_agl(2.0);
        assert_eq!(p.num, 4);
        assert_eq!(p.first_far, NEAR_FIRST_M);
        assert_eq!(p.max, GROUND_MAX_M);
        let t = p.texel_m(FOV, ASPECT, 0.1, 2048);
        // First cascade is sharper than the old one; neighbours differ ~5.5x, not 17x.
        assert!(t[0] < 0.02, "{t:?}");
        for w in t[1..].windows(2) {
            let r = w[1] / w[0];
            assert!((4.5..6.5).contains(&r), "ratio {r} in {t:?}");
        }
        // Last cascade still resolves a house (< 3 m texels) at 2.5 km.
        assert!(t[3] < 3.0, "{t:?}");
    }

    #[test]
    fn bounds_are_monotonic_and_end_at_max() {
        for agl in [0.0, 3.0, 40.0, 300.0, 2_000.0, 50_000.0] {
            let p = CascadePlan::for_agl(agl);
            let b = p.bounds();
            assert_eq!(b.len(), p.num);
            assert!((b[0] - p.first_far).abs() < 1e-3);
            assert!((b[p.num - 1] - p.max).abs() / p.max < 1e-4);
            assert!(b.windows(2).all(|w| w[1] > w[0]), "{b:?}");
            assert!(p.max <= AIR_MAX_M && p.fade_start() < p.max);
        }
    }

    #[test]
    fn flying_pushes_cascades_out() {
        let ground = CascadePlan::for_agl(2.0);
        let air = CascadePlan::for_agl(1_500.0);
        assert!(air.first_far > ground.first_far * 10.0);
        assert!(air.max > ground.max * 3.0);
        assert_eq!(CascadePlan::for_agl(1e6).max, AIR_MAX_M);
    }

    #[test]
    fn agl_quantisation_is_stable_under_small_moves() {
        // A 2% bob does not change the plan most of the time; it never changes by more than one step.
        let mut changes = 0;
        let mut prev = CascadePlan::for_agl(100.0);
        for i in 0..200 {
            let agl = 100.0 * (1.0 + 0.02 * (i as f32 * 0.37).sin());
            let p = CascadePlan::for_agl(agl);
            if p != prev {
                changes += 1;
                assert!((p.first_far / prev.first_far - 1.0).abs() < 0.2);
            }
            prev = p;
        }
        assert!(changes < 200 / 4, "{changes}");
        assert_eq!(quantize_agl(0.0), 1.0);
        assert!(quantize_agl(1000.0) <= 1000.0 && quantize_agl(1000.0) > 1000.0 / 1.19);
    }
}
