//! The building blocks of `synth.js` / `special.js`, ported with the same
//! arithmetic (f64 state like JS numbers; f32 where the JS uses a
//! Float32Array).

pub const TAU: f64 = std::f64::consts::PI * 2.0;

#[inline(always)]
pub fn clamp(x: f64, lo: f64, hi: f64) -> f64 {
    if x < lo { lo } else if x > hi { hi } else { x }
}
#[inline(always)]
pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}
#[inline(always)]
pub fn smoothstep(x: f64) -> f64 {
    let x = clamp(x, 0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}
#[inline(always)]
pub fn coef(sr: f64, seconds: f64) -> f64 {
    1.0 - (-1.0 / (sr * seconds.max(1e-5))).exp()
}
/// JS `a || b` for numbers: 0 (or NaN) falls back.
#[inline(always)]
pub fn or(a: f64, b: f64) -> f64 {
    if a != 0.0 && !a.is_nan() { a } else { b }
}
/// JS `Math.max` semantics are the same as f64::max for non-NaN inputs.
#[inline(always)]
pub fn sign(x: f64) -> f64 {
    if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { x }
}

/// The seeded xorshift32 noise generator (`Rng` in synth.js). Integer
/// arithmetic, so it is bit-identical to the JS.
#[derive(Clone, Copy, Debug)]
pub struct Rng {
    pub s: u32,
}

impl Rng {
    pub fn new(seed: u32) -> Self {
        Self { s: if seed == 0 { 0x2545f491 } else { seed } }
    }
    #[inline(always)]
    pub fn next(&mut self) -> f64 {
        let mut s = self.s;
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        self.s = s;
        s as f64 / 4294967296.0
    }
    /// -1..1 (`next() * 2 - 1`; identical to the inlined `s / 2^31 - 1`).
    #[inline(always)]
    pub fn bi(&mut self) -> f64 {
        let mut s = self.s;
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        self.s = s;
        s as f64 / 2147483648.0 - 1.0
    }
}

/// RBJ cookbook biquad, transposed direct form II.
#[derive(Clone, Copy, Debug)]
pub struct Biquad {
    pub b0: f64,
    pub b1: f64,
    pub b2: f64,
    pub a1: f64,
    pub a2: f64,
    pub z1: f64,
    pub z2: f64,
}

impl Default for Biquad {
    fn default() -> Self {
        Self { b0: 1.0, b1: 0.0, b2: 0.0, a1: 0.0, a2: 0.0, z1: 0.0, z2: 0.0 }
    }
}

impl Biquad {
    #[inline(always)]
    fn set(&mut self, b0: f64, b1: f64, b2: f64, a0: f64, a1: f64, a2: f64) {
        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = a1 / a0;
        self.a2 = a2 / a0;
    }
    pub fn lowpass(&mut self, sr: f64, hz: f64, q: f64) -> &mut Self {
        let w = TAU * clamp(hz, 10.0, sr * 0.45) / sr;
        let (c, al) = (w.cos(), w.sin() / (2.0 * q));
        self.set((1.0 - c) / 2.0, 1.0 - c, (1.0 - c) / 2.0, 1.0 + al, -2.0 * c, 1.0 - al);
        self
    }
    pub fn bandpass(&mut self, sr: f64, hz: f64, q: f64) -> &mut Self {
        let w = TAU * clamp(hz, 10.0, sr * 0.45) / sr;
        let (c, al) = (w.cos(), w.sin() / (2.0 * q));
        self.set(al, 0.0, -al, 1.0 + al, -2.0 * c, 1.0 - al);
        self
    }
    pub fn lowshelf(&mut self, sr: f64, hz: f64, db: f64) -> &mut Self {
        let a = 10f64.powf(db / 40.0);
        let w = TAU * clamp(hz, 10.0, sr * 0.45) / sr;
        let c = w.cos();
        let al = w.sin() / 2.0 * std::f64::consts::SQRT_2;
        let r = 2.0 * a.sqrt() * al;
        self.set(
            a * ((a + 1.0) - (a - 1.0) * c + r),
            2.0 * a * ((a - 1.0) - (a + 1.0) * c),
            a * ((a + 1.0) - (a - 1.0) * c - r),
            (a + 1.0) + (a - 1.0) * c + r,
            -2.0 * ((a - 1.0) + (a + 1.0) * c),
            (a + 1.0) + (a - 1.0) * c - r,
        );
        self
    }
    pub fn peak(&mut self, sr: f64, hz: f64, q: f64, db: f64) -> &mut Self {
        let a = 10f64.powf(db / 40.0);
        let w = TAU * clamp(hz, 10.0, sr * 0.45) / sr;
        let (c, al) = (w.cos(), w.sin() / (2.0 * q));
        self.set(1.0 + al * a, -2.0 * c, 1.0 - al * a, 1.0 + al / a, -2.0 * c, 1.0 - al / a);
        self
    }
    #[inline(always)]
    pub fn process(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }
}

/// Tailpipe: a delay line fed back through a one-pole low-pass (wall losses)
/// and an inverting reflection. The buffer is fixed at construction (50 ms)
/// and never reallocated.
#[derive(Clone, Debug)]
pub struct Pipe {
    pub buf: Box<[f32]>,
    pub w: usize,
    pub lp: f64,
    pub d: f64,
    pub r: f64,
    pub k: f64,
}

impl Pipe {
    pub fn new(sr: f64) -> Self {
        let len = (sr * 0.05).ceil() as usize + 4;
        Self { buf: vec![0.0; len].into_boxed_slice(), w: 0, lp: 0.0, d: 100.0, r: -0.5, k: 0.7 }
    }
    /// A placeholder with no buffer (a zero-length box does not allocate).
    pub fn empty() -> Self {
        Self { buf: Box::new([]), w: 0, lp: 0.0, d: 100.0, r: -0.5, k: 0.7 }
    }
    /// Back to the state `new` gives (silence), keeping the buffer.
    pub fn clear(&mut self) {
        self.buf.fill(0.0);
        self.w = 0;
        self.lp = 0.0;
        self.d = 100.0;
        self.r = -0.5;
        self.k = 0.7;
    }
    pub fn set(&mut self, sr: f64, ms: f64, reflect: f64, damp: f64) {
        self.d = clamp(ms * sr / 1000.0, 2.0, self.buf.len() as f64 - 3.0);
        self.r = clamp(reflect, -0.97, 0.97);
        self.k = 1.0 - clamp(damp, 0.0, 0.95);
    }
    #[inline(always)]
    pub fn process(&mut self, x: f64) -> f64 {
        let len = self.buf.len();
        let mut rp = self.w as f64 - self.d;
        if rp < 0.0 {
            rp += len as f64;
        }
        let i = rp as usize;
        let f = rp - i as f64;
        let j = if i + 1 == len { 0 } else { i + 1 };
        let (bi, bj) = (self.buf[i] as f64, self.buf[j] as f64);
        self.lp += self.k * (bi + (bj - bi) * f - self.lp);
        let y = x + self.r * self.lp;
        self.buf[self.w] = y as f32;
        self.w = if self.w + 1 == len { 0 } else { self.w + 1 };
        y
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rng_matches_js_sequence() {
        // node -e "let s=12345;for(let i=0;i<3;i++){s^=s<<13;s^=s>>>17;s^=s<<5;s>>>=0;console.log(s)}"
        let mut r = Rng::new(12345);
        let a: Vec<u32> = (0..3).map(|_| { r.next(); r.s }).collect();
        assert_eq!(a, vec![3336926330, 1697253807, 2816511904]);
    }
}
