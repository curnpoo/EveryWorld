//! Render-loop helper: turns variable real frame time into whole fixed ticks plus an
//! interpolation factor. Lives on the caller's side of the step; the sim itself never sees `real_dt`.

use crate::DT;

#[derive(Clone, Copy, Debug)]
pub struct Accumulator {
    acc: f64,
    max_steps: u32,
}

impl Accumulator {
    /// `max_steps` caps the ticks run per frame (spiral-of-death guard); time beyond it is dropped.
    pub fn new(max_steps: u32) -> Self {
        Accumulator { acc: 0.0, max_steps: max_steps.max(1) }
    }

    /// Adds `real_dt` seconds and returns `(ticks to run now, alpha)`, where `alpha` in `[0, 1)`
    /// is how far the render time is between the last two ticks. Negative or non-finite input
    /// counts as zero. When the clamp hits, whole ticks beyond it are dropped and the
    /// fractional part is kept.
    pub fn advance(&mut self, real_dt: f64) -> (u32, f64) {
        if real_dt.is_finite() && real_dt > 0.0 {
            self.acc += real_dt;
        }
        let whole = (self.acc / DT).floor();
        let steps = if whole >= self.max_steps as f64 {
            self.acc -= whole * DT;
            self.max_steps
        } else {
            let n = whole as u32;
            self.acc -= n as f64 * DT;
            n
        };
        // Guard against rounding leaving acc a hair below 0 or at DT.
        if self.acc < 0.0 {
            self.acc = 0.0;
        }
        let alpha = (self.acc / DT).clamp(0.0, 1.0 - f64::EPSILON);
        (steps, alpha)
    }

    pub fn alpha(&self) -> f64 {
        (self.acc / DT).clamp(0.0, 1.0 - f64::EPSILON)
    }

    pub fn reset(&mut self) {
        self.acc = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accumulator_counts_alpha_and_clamp() {
        let mut a = Accumulator::new(5);
        // Exactly one tick.
        assert_eq!(a.advance(DT).0, 1);
        assert!(a.alpha() < 1e-9);
        // Half a tick: no step, alpha 0.5.
        let (n, alpha) = a.advance(DT * 0.5);
        assert_eq!(n, 0);
        assert!((alpha - 0.5).abs() < 1e-9);
        // Another 1.75 ticks: total 2.25 -> 2 steps, alpha 0.25.
        let (n, alpha) = a.advance(DT * 1.75);
        assert_eq!(n, 2);
        assert!((alpha - 0.25).abs() < 1e-9);
        // A 1 s hitch: clamped to 5 steps, fraction kept.
        let (n, alpha) = a.advance(1.0);
        assert_eq!(n, 5);
        assert!((0.0..1.0).contains(&alpha));
        // The dropped time does not come back.
        assert_eq!(a.advance(0.0).0, 0);
        // Bad input is ignored.
        assert_eq!(a.advance(f64::NAN).0, 0);
        assert_eq!(a.advance(-1.0).0, 0);
        // 60 frames at 60 Hz give 60 ticks over the run.
        let mut b = Accumulator::new(4);
        let total: u32 = (0..60).map(|_| b.advance(1.0 / 60.0).0).sum();
        assert!((59..=60).contains(&total));
        // 144 Hz render: 1 s gives 60 ticks within one.
        let mut c = Accumulator::new(4);
        let total: u32 = (0..144).map(|_| c.advance(1.0 / 144.0).0).sum();
        assert!((59..=60).contains(&total));
    }
}
