//! Fixed-step loop (P1-D16, P1-D17): the simulation advances in fixed 60 Hz steps from an
//! accumulator fed with frame time; rendering interpolates between the last two states with
//! the accumulator fraction. Wall-clock time is read by the caller, outside the step; a step
//! sees only its fixed `dt` and its input snapshot.

/// Simulation rate.
pub const SIM_HZ: f64 = 60.0;
/// Most steps one frame may run. Time beyond this is dropped (the step clamp), so a long
/// hitch slows the simulation down instead of spiralling.
pub const MAX_STEPS_PER_FRAME: u32 = 8;

/// Something advanced in fixed steps. `ed_sim` plugs in here later.
pub trait Simulation {
    type Input;
    fn step(&mut self, input: &Self::Input, dt: f64);
}

/// The accumulator.
#[derive(Clone, Debug)]
pub struct FixedStep {
    dt: f64,
    max_steps: u32,
    acc: f64,
    steps: u64,
}

impl Default for FixedStep {
    fn default() -> Self {
        Self::new(1.0 / SIM_HZ, MAX_STEPS_PER_FRAME)
    }
}

impl FixedStep {
    pub fn new(dt: f64, max_steps: u32) -> Self {
        assert!(dt > 0.0 && max_steps > 0);
        Self { dt, max_steps, acc: 0.0, steps: 0 }
    }

    /// The fixed step, seconds.
    pub fn dt(&self) -> f64 {
        self.dt
    }

    /// Total steps run so far.
    pub fn steps(&self) -> u64 {
        self.steps
    }

    /// Adds `frame_dt` seconds and returns how many steps to run now. Negative or NaN frame
    /// times count as zero; time beyond `max_steps` steps is dropped.
    pub fn advance(&mut self, frame_dt: f64) -> u32 {
        let frame_dt = if frame_dt.is_finite() { frame_dt.max(0.0) } else { 0.0 };
        self.acc += frame_dt;
        // A tiny epsilon so that exact multiples of dt (headless virtual time) are not lost
        // to rounding.
        let mut n = ((self.acc + 1e-9) / self.dt).floor() as u64;
        if n > self.max_steps as u64 {
            n = self.max_steps as u64;
            self.acc = self.dt * n as f64; // clamp: drop the excess
        }
        self.acc = (self.acc - self.dt * n as f64).max(0.0);
        self.steps += n;
        n as u32
    }

    /// Interpolation fraction between the previous and the current state, in [0, 1).
    pub fn alpha(&self) -> f32 {
        ((self.acc / self.dt) as f32).clamp(0.0, 0.999_999)
    }

    /// Runs the steps for one frame on `sim`. `input(i)` gives the snapshot for step `i` of
    /// this frame (so one-shot input such as mouse deltas can go to the first step only).
    /// Returns the interpolation fraction.
    pub fn run<S: Simulation>(&mut self, frame_dt: f64, sim: &mut S, mut input: impl FnMut(u32) -> S::Input) -> f32 {
        let n = self.advance(frame_dt);
        for i in 0..n {
            let inp = input(i);
            sim.step(&inp, self.dt);
        }
        self.alpha()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Counter(u32);
    impl Simulation for Counter {
        type Input = ();
        fn step(&mut self, _: &(), dt: f64) {
            assert_eq!(dt, 1.0 / 60.0);
            self.0 += 1;
        }
    }

    #[test]
    fn steps_follow_accumulated_time() {
        let mut f = FixedStep::default();
        assert_eq!(f.advance(0.010), 0);
        assert!((f.alpha() - 0.6).abs() < 1e-4);
        assert_eq!(f.advance(0.010), 1); // 20 ms -> 1 step, 3.33 ms left
        assert!((f.alpha() - 0.2).abs() < 1e-3);
        assert_eq!(f.advance(1.0 / 30.0), 2);
        assert_eq!(f.steps(), 3);
    }

    #[test]
    fn exact_virtual_time_is_one_step_per_frame() {
        let mut f = FixedStep::default();
        let mut c = Counter(0);
        for _ in 0..600 {
            assert_eq!(f.run(1.0 / 60.0, &mut c, |_| ()), 0.0);
        }
        assert_eq!(c.0, 600);
    }

    #[test]
    fn step_clamp_drops_excess_time() {
        let mut f = FixedStep::default();
        assert_eq!(f.advance(5.0), MAX_STEPS_PER_FRAME);
        assert_eq!(f.alpha(), 0.0, "excess dropped, nothing carried over");
        assert_eq!(f.advance(1.0 / 60.0), 1);
    }

    #[test]
    fn bad_frame_times_are_ignored() {
        let mut f = FixedStep::default();
        assert_eq!(f.advance(-1.0), 0);
        assert_eq!(f.advance(f64::NAN), 0);
        assert_eq!(f.alpha(), 0.0);
    }
}
