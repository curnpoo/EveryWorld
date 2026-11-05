//! Small f32 DSP kit for the soundscape: noise colours, one-pole and
//! state-variable filters, smoothing, delay line. Everything is per-sample
//! and allocation-free after construction.

pub const SR: f32 = 48_000.0;
pub const TAU: f32 = std::f32::consts::TAU;

/// xorshift32: deterministic, seeded, fast.
#[derive(Clone, Copy, Debug)]
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: u32) -> Self {
        Rng(seed.max(1).wrapping_mul(2_654_435_761) | 1)
    }
    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    /// [0, 1)
    #[inline]
    pub fn f(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 * (1.0 / 16_777_216.0)
    }
    /// [-1, 1)
    #[inline]
    pub fn bi(&mut self) -> f32 {
        self.f() * 2.0 - 1.0
    }
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f()
    }
    /// True with probability `rate_hz / SR` this sample (Poisson trigger).
    #[inline]
    pub fn chance(&mut self, rate_hz: f32) -> bool {
        rate_hz > 0.0 && self.f() < rate_hz / SR
    }
}

/// Pink noise (Paul Kellet's economy filter), roughly unit RMS-ish.
#[derive(Clone, Copy, Default, Debug)]
pub struct Pink {
    b: [f32; 3],
}

impl Pink {
    #[inline]
    pub fn tick(&mut self, white: f32) -> f32 {
        self.b[0] = 0.99765 * self.b[0] + white * 0.0990460;
        self.b[1] = 0.96300 * self.b[1] + white * 0.2965164;
        self.b[2] = 0.57000 * self.b[2] + white * 1.0526913;
        (self.b[0] + self.b[1] + self.b[2] + white * 0.1848) * 0.25
    }
}

/// Brown (red) noise: leaky integrated white.
#[derive(Clone, Copy, Default, Debug)]
pub struct Brown {
    y: f32,
}

impl Brown {
    #[inline]
    pub fn tick(&mut self, white: f32) -> f32 {
        self.y = (self.y + white * 0.02) * 0.998;
        self.y * 3.5
    }
}

/// One-pole low-pass. `a` is set per block from a cutoff.
#[derive(Clone, Copy, Default, Debug)]
pub struct OnePole {
    pub y: f32,
    a: f32,
}

impl OnePole {
    pub fn set(&mut self, fc: f32) {
        self.a = 1.0 - (-TAU * fc.clamp(1.0, SR * 0.45) / SR).exp();
    }
    #[inline]
    pub fn lp(&mut self, x: f32) -> f32 {
        self.y += self.a * (x - self.y);
        self.y
    }
    #[inline]
    pub fn hp(&mut self, x: f32) -> f32 {
        x - self.lp(x)
    }
}

/// Trapezoidal state-variable filter (Simper). Set coefficients per block.
#[derive(Clone, Copy, Default, Debug)]
pub struct Svf {
    ic1: f32,
    ic2: f32,
    a1: f32,
    a2: f32,
    a3: f32,
    k: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Lp,
    Bp,
    Hp,
}

impl Svf {
    pub fn new(fc: f32, q: f32) -> Self {
        let mut s = Svf::default();
        s.set(fc, q);
        s
    }
    pub fn set(&mut self, fc: f32, q: f32) {
        let g = (std::f32::consts::PI * fc.clamp(10.0, SR * 0.45) / SR).tan();
        self.k = 1.0 / q.max(0.05);
        self.a1 = 1.0 / (1.0 + g * (g + self.k));
        self.a2 = g * self.a1;
        self.a3 = g * self.a2;
    }
    /// Returns (low, band, high).
    #[inline]
    pub fn tick(&mut self, x: f32) -> (f32, f32, f32) {
        let v3 = x - self.ic2;
        let v1 = self.a1 * self.ic1 + self.a2 * v3;
        let v2 = self.ic2 + self.a2 * self.ic1 + self.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        (v2, v1, x - self.k * v1 - v2)
    }
    #[inline]
    pub fn run(&mut self, x: f32, m: Mode) -> f32 {
        let (l, b, h) = self.tick(x);
        match m {
            Mode::Lp => l,
            Mode::Bp => b,
            Mode::Hp => h,
        }
    }
}

/// Fractional delay line (linear interpolation).
pub struct Delay {
    buf: Vec<f32>,
    w: usize,
}

impl Delay {
    pub fn new(max_s: f32) -> Self {
        Delay { buf: vec![0.0; (max_s * SR) as usize + 2], w: 0 }
    }
    #[inline]
    pub fn write(&mut self, x: f32) {
        self.buf[self.w] = x;
        self.w = (self.w + 1) % self.buf.len();
    }
    /// Sample written `d` samples ago (call before `write`).
    #[inline]
    pub fn read(&self, d: f32) -> f32 {
        let n = self.buf.len();
        let d = d.clamp(1.0, (n - 2) as f32);
        let i = d as usize;
        let f = d - i as f32;
        let a = self.buf[(self.w + n - i) % n];
        let b = self.buf[(self.w + n - i - 1) % n];
        a + (b - a) * f
    }
}

#[inline]
pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Equal-power pan, `p` in -1..1 -> (left, right) gains.
#[inline]
pub fn pan(p: f32) -> (f32, f32) {
    let a = (p.clamp(-1.0, 1.0) + 1.0) * std::f32::consts::FRAC_PI_4;
    (a.cos(), a.sin())
}

pub fn db(d: f32) -> f32 {
    10f32.powf(d / 20.0)
}

/// Fast sine for oscillators: `ph` in turns.
#[inline]
pub fn sin_t(ph: f32) -> f32 {
    (ph * TAU).sin()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rng_is_deterministic_and_in_range() {
        let mut a = Rng::new(7);
        let mut b = Rng::new(7);
        for _ in 0..1000 {
            let x = a.f();
            assert_eq!(x, b.f());
            assert!((0.0..1.0).contains(&x));
        }
    }

    #[test]
    fn svf_lowpass_kills_highs() {
        let mut lo = Svf::new(200.0, 0.707);
        let mut e_in = 0.0;
        let mut e_out = 0.0;
        for i in 0..48_000 {
            let x = sin_t(i as f32 * 8_000.0 / SR);
            let y = lo.run(x, Mode::Lp);
            if i > 1000 {
                e_in += x * x;
                e_out += y * y;
            }
        }
        assert!(e_out < e_in * 0.01, "8 kHz through a 200 Hz LP: {e_out} vs {e_in}");
    }

    #[test]
    fn delay_returns_past() {
        let mut d = Delay::new(0.1);
        for i in 0..100 {
            d.write(i as f32);
        }
        assert!((d.read(10.0) - 90.0).abs() < 1e-4);
    }

    #[test]
    fn pan_is_equal_power() {
        for p in [-1.0, -0.3, 0.0, 0.5, 1.0] {
            let (l, r) = pan(p);
            assert!((l * l + r * r - 1.0).abs() < 1e-5);
        }
    }
}
