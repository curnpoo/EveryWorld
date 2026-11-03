//! Procedural engine sound. No samples: every cylinder fires at its real
//! crank angle, each combustion pulse excites an exhaust-pipe resonator
//! (a feedback delay line tuned to the pipe length), and intake roar plus
//! mechanical noise ride on top. Uneven firing (cross-plane V8, V-twin)
//! produces the burble for the same reason it does in a real engine.
//!
//! The synth is plain `f32` DSP so it runs in any audio callback.

use std::f32::consts::TAU;

#[derive(Debug, Clone)]
pub struct EngineProfile {
    pub name: &'static str,
    /// Firing angles in crank degrees within a 720° four-stroke cycle.
    pub firing_deg: Vec<f32>,
    /// Exhaust pipe length (m): sets the resonant pitch.
    pub pipe_m: f32,
    /// 0..1, how much each pulse rings (muffler vs straight pipe).
    pub resonance: f32,
    /// Low-pass cutoff (Hz) on the exhaust: muffled vs raspy.
    pub brightness_hz: f32,
    pub idle_rpm: f32,
    pub redline_rpm: f32,
    /// Two banks with separate pipes (V engines): stereo-ish thickness.
    pub dual_exhaust: bool,
}

impl EngineProfile {
    fn even(n: usize) -> Vec<f32> {
        (0..n).map(|i| i as f32 * 720.0 / n as f32).collect()
    }

    pub fn inline4() -> Self {
        Self { name: "Inline-4", firing_deg: Self::even(4), pipe_m: 1.6, resonance: 0.55, brightness_hz: 2_400.0, idle_rpm: 850.0, redline_rpm: 7_200.0, dual_exhaust: false }
    }

    /// Cross-plane V8: firing order 1-8-4-3-6-5-7-2, each bank fires unevenly
    /// (90/180/270 gaps), which is the classic muscle-car burble.
    pub fn v8_crossplane() -> Self {
        Self {
            name: "V8 cross-plane",
            firing_deg: vec![0.0, 90.0, 180.0, 270.0, 360.0, 450.0, 540.0, 630.0],
            pipe_m: 2.4,
            resonance: 0.72,
            brightness_hz: 1_500.0,
            idle_rpm: 700.0,
            redline_rpm: 6_800.0,
            dual_exhaust: true,
        }
    }

    pub fn flat6() -> Self {
        Self { name: "Flat-6", firing_deg: Self::even(6), pipe_m: 1.3, resonance: 0.62, brightness_hz: 3_400.0, idle_rpm: 900.0, redline_rpm: 9_000.0, dual_exhaust: true }
    }

    /// 45° V-twin: 315°/405° uneven gap ("potato potato").
    pub fn vtwin() -> Self {
        Self { name: "V-twin", firing_deg: vec![0.0, 315.0], pipe_m: 1.1, resonance: 0.5, brightness_hz: 1_800.0, idle_rpm: 950.0, redline_rpm: 6_000.0, dual_exhaust: false }
    }

    /// Which cylinders are on the left bank (for dual exhaust). For the
    /// cross-plane V8 the per-bank spacing is deliberately uneven.
    fn bank(&self, i: usize) -> usize {
        if !self.dual_exhaust {
            return 0;
        }
        if self.firing_deg.len() == 8 {
            // Firing order 1-8-4-3-6-5-7-2 mapped to banks (1-4 left, 5-8 right).
            [0, 1, 0, 0, 1, 1, 1, 0][i]
        } else {
            i % 2
        }
    }
}

/// One-pole low-pass.
#[derive(Default, Clone, Copy)]
struct Lp {
    z: f32,
}
impl Lp {
    fn run(&mut self, x: f32, cutoff: f32, sr: f32) -> f32 {
        let a = 1.0 - (-TAU * cutoff / sr).exp();
        self.z += a * (x - self.z);
        self.z
    }
}

/// Feedback comb (pipe resonator).
struct Pipe {
    buf: Vec<f32>,
    pos: usize,
    lp: Lp,
}
impl Pipe {
    fn new(len: usize) -> Self {
        Self { buf: vec![0.0; len.max(2)], pos: 0, lp: Lp::default() }
    }
    fn run(&mut self, x: f32, fb: f32, damp_hz: f32, sr: f32) -> f32 {
        let y = self.buf[self.pos];
        let damped = self.lp.run(y, damp_hz, sr);
        self.buf[self.pos] = x + damped * fb;
        self.pos = (self.pos + 1) % self.buf.len();
        y
    }
}

pub struct EngineSynth {
    pub profile: EngineProfile,
    sr: f32,
    crank_deg: f32,
    pipes: [Pipe; 2],
    pulse_env: Vec<f32>,
    out_lp: Lp,
    rng: u32,
    /// Smoothed controls.
    rpm: f32,
    throttle: f32,
    /// Targets (set from the game thread).
    pub target_rpm: f32,
    pub target_throttle: f32,
}

impl EngineSynth {
    pub fn new(profile: EngineProfile, sample_rate: f32) -> Self {
        // Quarter-wave pipe resonance: f = c / 4L, delay = 1/f.
        let c = 340.0;
        let delay = |l: f32| (sample_rate * 4.0 * l / c) as usize;
        let n = profile.firing_deg.len();
        let idle = profile.idle_rpm;
        Self {
            pipes: [Pipe::new(delay(profile.pipe_m)), Pipe::new(delay(profile.pipe_m * 1.07))],
            profile,
            sr: sample_rate,
            crank_deg: 0.0,
            pulse_env: vec![0.0; n],
            out_lp: Lp::default(),
            rng: 0x1234_5678,
            rpm: idle,
            throttle: 0.0,
            target_rpm: idle,
            target_throttle: 0.0,
        }
    }

    fn noise(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    /// Render one mono sample.
    pub fn sample(&mut self) -> f32 {
        let sr = self.sr;
        // Smooth control changes (no zipper noise).
        self.rpm += (self.target_rpm.clamp(self.profile.idle_rpm * 0.8, self.profile.redline_rpm) - self.rpm) * (8.0 / sr);
        self.throttle += (self.target_throttle.clamp(0.0, 1.0) - self.throttle) * (20.0 / sr);
        let prev = self.crank_deg;
        self.crank_deg = (self.crank_deg + self.rpm / 60.0 * 360.0 / sr) % 720.0;
        // Fire any cylinder whose angle we just crossed.
        let cur = self.crank_deg;
        let crossed = |a: f32| if prev <= cur { a > prev && a <= cur } else { a > prev || a <= cur };
        let load = 0.25 + 0.75 * self.throttle;
        let mut ex = [0.0f32; 2];
        for i in 0..self.profile.firing_deg.len() {
            if crossed(self.profile.firing_deg[i]) {
                // Tiny per-firing variation keeps it alive.
                self.pulse_env[i] = load * (0.9 + 0.2 * self.noise().abs());
            }
            let e = self.pulse_env[i];
            if e > 1e-4 {
                // Combustion pulse: sharp attack, ~3 ms decay, with grit.
                let v = e * (0.75 + 0.25 * self.noise());
                ex[self.profile.bank(i)] += v;
                self.pulse_env[i] *= (-1.0 / (0.003 * sr)).exp();
            }
        }
        let fb = 0.35 + 0.55 * self.profile.resonance;
        let damp = self.profile.brightness_hz * (0.6 + 0.6 * self.throttle);
        let a = self.pipes[0].run(ex[0], fb, damp, sr);
        let b = self.pipes[1].run(ex[1], fb, damp, sr);
        let exhaust = a + b + (ex[0] + ex[1]) * 0.4;
        // Intake hiss grows with throttle and rpm.
        let intake = self.noise() * self.throttle * (self.rpm / self.profile.redline_rpm) * 0.08;
        let raw = exhaust + intake;
        let y = self.out_lp.run(raw, self.profile.brightness_hz * 1.5, sr);
        (y * 0.5).tanh()
    }

    pub fn fill(&mut self, out: &mut [f32]) {
        for s in out {
            *s = self.sample();
        }
    }

    /// Fundamental firing frequency (Hz) at the current rpm.
    pub fn firing_hz(&self) -> f32 {
        self.rpm / 60.0 / 2.0 * self.profile.firing_deg.len() as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dft_mag(x: &[f32], sr: f32, f: f32) -> f32 {
        let (mut re, mut im) = (0.0f32, 0.0f32);
        for (n, v) in x.iter().enumerate() {
            let ph = TAU * f * n as f32 / sr;
            re += v * ph.cos();
            im -= v * ph.sin();
        }
        (re * re + im * im).sqrt() / x.len() as f32
    }

    fn render(p: EngineProfile, rpm: f32, thr: f32) -> (Vec<f32>, f32) {
        let sr = 22_050.0;
        let mut e = EngineSynth::new(p, sr);
        e.target_rpm = rpm;
        e.target_throttle = thr;
        let mut warm = vec![0.0; sr as usize];
        e.fill(&mut warm);
        let mut buf = vec![0.0; sr as usize / 2];
        e.fill(&mut buf);
        (buf, e.firing_hz())
    }

    #[test]
    fn output_is_sane() {
        for p in [EngineProfile::inline4(), EngineProfile::v8_crossplane(), EngineProfile::flat6(), EngineProfile::vtwin()] {
            let (buf, _) = render(p.clone(), 3_000.0, 0.7);
            assert!(buf.iter().all(|v| v.is_finite() && v.abs() <= 1.0), "{}", p.name);
            let rms = (buf.iter().map(|v| v * v).sum::<f32>() / buf.len() as f32).sqrt();
            assert!(rms > 0.01, "{} too quiet: {rms}", p.name);
        }
    }

    #[test]
    fn inline4_sings_at_firing_frequency() {
        // I4 at 3000 rpm fires at 100 Hz; that band should beat its neighbors.
        let (buf, f0) = render(EngineProfile::inline4(), 3_000.0, 0.8);
        assert!((f0 - 100.0).abs() < 1.0);
        let at = dft_mag(&buf, 22_050.0, f0);
        let off = dft_mag(&buf, 22_050.0, f0 * 1.37);
        assert!(at > off * 2.0, "firing {at} vs off {off}");
    }

    #[test]
    fn v8_burbles_at_half_order() {
        // Uneven per-bank firing puts energy at half the firing frequency
        // (the "lope"); an even-fire engine has almost none there.
        let (v8, f0) = render(EngineProfile::v8_crossplane(), 1_200.0, 0.3);
        let (i4, f0_i4) = render(EngineProfile::inline4(), 1_200.0, 0.3);
        let v8_sub = dft_mag(&v8, 22_050.0, f0 / 2.0) / dft_mag(&v8, 22_050.0, f0).max(1e-6);
        let i4_sub = dft_mag(&i4, 22_050.0, f0_i4 / 2.0) / dft_mag(&i4, 22_050.0, f0_i4).max(1e-6);
        assert!(v8_sub > i4_sub, "v8 sub-order {v8_sub} vs i4 {i4_sub}");
    }
}
