//! A small procedural lo-fi beat for the main menu. Engine-free, no samples:
//! swung drums, a detuned electric-piano chord loop, a plucked bass, sparse
//! pentatonic bells through a ping-pong delay, vinyl crackle, and a master
//! low-pass so everything sits back.
//!
//! 72 BPM, Cmaj9 - Am9 - Dm9 - G13, one bar each. `next_frame` returns one
//! stereo frame at `dsp::SR`; `set_gain` fades the whole thing in or out.

use crate::dsp::SR;
use std::f32::consts::TAU;

const BPM: f32 = 72.0;
/// How far off the grid the odd 16ths fall (fraction of a 16th).
const SWING: f32 = 0.17;

/// The four chords (MIDI), root-position voicings centred around C3-C4.
const CHORDS: [[u8; 5]; 4] = [[48, 52, 55, 59, 62], [45, 48, 52, 55, 59], [50, 53, 57, 60, 64], [43, 47, 53, 57, 64]];
const ROOTS: [u8; 4] = [36, 33, 38, 31];
/// C major pentatonic, C5 up to C6: safe over every chord above.
const PENTA: [u8; 6] = [72, 74, 76, 79, 81, 84];

fn mtof(m: u8) -> f32 {
    440.0 * 2f32.powf((m as f32 - 69.0) / 12.0)
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Tine,
    Bass,
    Kick,
    Snare,
    Hat,
    Bell,
}

struct Voice {
    kind: Kind,
    /// Seconds since the voice started; negative while it waits (strum offsets).
    t: f32,
    dur: f32,
    freq: f32,
    amp: f32,
    pan: f32,
    ph: f32,
    ph2: f32,
    /// Held noise value (sample-and-hold) and the previous one (for a high-pass).
    nv: f32,
    prev: f32,
}

impl Voice {
    fn new(kind: Kind, delay: f32, dur: f32, freq: f32, amp: f32, pan: f32) -> Self {
        Voice { kind, t: -delay, dur, freq, amp, pan, ph: 0.0, ph2: 0.0, nv: 0.0, prev: 0.0 }
    }
}

pub struct LoFi {
    voices: Vec<Voice>,
    rng: u32,
    time: f64,
    step: u32,
    gain: f32,
    target: f32,
    lp: [f32; 2],
    delay: [Vec<f32>; 2],
    di: usize,
    pop: f32,
    hiss: f32,
    wob: f32,
}

impl Default for LoFi {
    fn default() -> Self {
        Self::new()
    }
}

impl LoFi {
    pub fn new() -> Self {
        let dl = (SR * 60.0 / BPM * 0.75) as usize; // dotted eighth
        LoFi {
            voices: Vec::with_capacity(48),
            rng: 0x1234_5678,
            time: 0.0,
            step: 0,
            gain: 0.0,
            target: 0.0,
            lp: [0.0; 2],
            delay: [vec![0.0; dl], vec![0.0; dl]],
            di: 0,
            pop: 0.0,
            hiss: 0.0,
            wob: 0.0,
        }
    }

    /// Where the music fades to (0 = silent). Smoothed, so it never clicks.
    pub fn set_gain(&mut self, g: f32) {
        self.target = g.clamp(0.0, 1.0);
    }

    fn rand(&mut self) -> f32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }

    fn trigger(&mut self, s: u32) {
        let bar = ((s / 16) % 4) as usize;
        let st = s % 16;
        let beat = 60.0 / BPM;
        // Chords: a strummed hit on the one and a soft re-hit after the "and" of 2.
        if st == 0 || st == 10 {
            let soft = st == 10;
            let notes: &[u8] = if soft { &CHORDS[bar][1..] } else { &CHORDS[bar] };
            for (i, n) in notes.iter().enumerate() {
                let strum = i as f32 * (0.012 + 0.012 * self.rand());
                let amp = if soft { 0.030 } else { 0.052 };
                let pan = (i as f32 / 4.0 - 0.5) * 0.5;
                let dur = if soft { beat * 1.6 } else { beat * 2.4 };
                self.voices.push(Voice::new(Kind::Tine, strum, dur, mtof(*n), amp, pan));
            }
        }
        if st == 0 || st == 10 {
            self.voices.push(Voice::new(Kind::Bass, 0.0, 1.1, mtof(ROOTS[bar]), 0.26, 0.0));
        }
        if st == 0 || st == 10 || (st == 7 && bar % 2 == 1) {
            let a = if st == 7 { 0.35 } else { 0.5 };
            self.voices.push(Voice::new(Kind::Kick, 0.0, 0.35, 0.0, a, 0.0));
        }
        if st == 4 || st == 12 {
            self.voices.push(Voice::new(Kind::Snare, 0.0, 0.28, 0.0, 0.22, 0.05));
        }
        if st % 2 == 0 {
            let r = self.rand();
            let a = if st % 4 == 0 { 0.055 } else { 0.03 } * (0.7 + 0.6 * r);
            self.voices.push(Voice::new(Kind::Hat, 0.0, 0.07, 0.0, a, 0.25 * (r - 0.5)));
        }
        // Sparse bells.
        if st % 2 == 0 && !(st == 0 && bar == 0) && self.rand() < 0.2 {
            let n = PENTA[(self.rand() * PENTA.len() as f32) as usize % PENTA.len()];
            let pan = self.rand() * 1.2 - 0.6;
            self.voices.push(Voice::new(Kind::Bell, 0.0, 3.0, mtof(n), 0.07, pan));
        }
    }

    /// One stereo frame, nominally within -1..1.
    pub fn next_frame(&mut self) -> [f32; 2] {
        self.gain += (self.target - self.gain) * 0.00002;
        if self.target == 0.0 && self.gain < 0.0004 {
            self.gain = 0.0;
            return [0.0, 0.0];
        }
        let dt = 1.0 / SR;
        let sl = 60.0 / BPM as f64 / 4.0;
        loop {
            let at = self.step as f64 * sl + if self.step % 2 == 1 { SWING as f64 * sl } else { 0.0 };
            if self.time < at {
                break;
            }
            let s = self.step;
            self.trigger(s);
            self.step += 1;
        }
        self.time += dt as f64;

        self.wob += TAU * 0.35 * dt;
        let vib = 1.0 + 0.0045 * self.wob.sin() + 0.0015 * (self.wob * 13.0).sin();
        let trem = 1.0 + 0.06 * (self.wob * 12.9).sin();

        let (mut dry_l, mut dry_r) = (0.0f32, 0.0f32);
        let (mut mel_l, mut mel_r) = (0.0f32, 0.0f32);
        let mut i = 0;
        while i < self.voices.len() {
            // Noise for the voices that want it, drawn before borrowing the voice.
            let nz = self.rand() * 2.0 - 1.0;
            let v = &mut self.voices[i];
            v.t += dt;
            if v.t < 0.0 {
                i += 1;
                continue;
            }
            if v.t > v.dur {
                self.voices.swap_remove(i);
                continue;
            }
            let t = v.t;
            let fade = ((v.dur - t) / 0.12).clamp(0.0, 1.0);
            let (x, melodic) = match v.kind {
                Kind::Tine => {
                    v.ph = (v.ph + v.freq * vib / SR).fract();
                    v.ph2 = (v.ph2 + v.freq * 1.0035 * vib / SR).fract();
                    let a = (1.0 - (-t / 0.004).exp()) * (-t / 1.5).exp();
                    let tone = (TAU * v.ph).sin() + (TAU * v.ph2).sin() + 0.45 * (TAU * 2.0 * v.ph).sin() * (-t * 4.0).exp() + 0.14 * (TAU * 7.1 * v.ph).sin() * (-t * 28.0).exp();
                    (tone * 0.5 * a * trem, true)
                }
                Kind::Bass => {
                    v.ph = (v.ph + v.freq / SR).fract();
                    let a = (1.0 - (-t / 0.008).exp()) * (-t * 2.4).exp();
                    (((TAU * v.ph).sin() + 0.25 * (TAU * 2.0 * v.ph).sin()) * a, false)
                }
                Kind::Kick => {
                    let f = 44.0 + 95.0 * (-t * 30.0).exp();
                    v.ph = (v.ph + f / SR).fract();
                    ((TAU * v.ph).sin() * (-t * 9.0).exp(), false)
                }
                Kind::Snare => {
                    // Sample-and-hold noise (about 1/3 rate) for a crunchy, crushed feel.
                    if (t * SR) as u32 % 3 == 0 {
                        v.nv = nz;
                    }
                    v.ph = (v.ph + 185.0 / SR).fract();
                    ((v.nv * 0.8 * (-t * 17.0).exp() + (TAU * v.ph).sin() * 0.5 * (-t * 24.0).exp()), false)
                }
                Kind::Hat => {
                    let hp = nz - v.prev;
                    v.prev = nz;
                    (hp * 0.5 * (-t * 70.0).exp(), false)
                }
                Kind::Bell => {
                    v.ph = (v.ph + v.freq * vib / SR).fract();
                    let a = (1.0 - (-t / 0.003).exp()) * (-t * 1.7).exp();
                    (((TAU * v.ph).sin() + 0.3 * (TAU * 3.0 * v.ph).sin() * (-t * 6.0).exp()) * a, true)
                }
            };
            let x = x * v.amp * fade;
            // Equal-power-ish pan.
            let l = x * (1.0 - v.pan.max(0.0) * 0.6);
            let r = x * (1.0 + v.pan.min(0.0) * 0.6);
            if melodic {
                mel_l += l;
                mel_r += r;
            } else {
                dry_l += l;
                dry_r += r;
            }
            i += 1;
        }

        // Ping-pong delay on the melodic bus.
        let n = self.delay[0].len();
        let (dl, dr) = (self.delay[0][self.di], self.delay[1][self.di]);
        self.delay[0][self.di] = mel_l + dr * 0.38;
        self.delay[1][self.di] = mel_r + dl * 0.38;
        self.di = (self.di + 1) % n;

        // Vinyl: a faint hiss and the odd pop.
        let r = self.rand();
        if r < 22.0 / SR {
            self.pop = 0.02 + 0.05 * self.rand();
        }
        self.pop *= 0.9925;
        let n2 = self.rand() * 2.0 - 1.0;
        self.hiss += (n2 - self.hiss) * 0.08;
        let vinyl = n2 * self.pop + self.hiss * 0.012;

        let mut out = [0.0f32; 2];
        let mix = [dry_l + mel_l + dl * 0.7 + vinyl, dry_r + mel_r + dr * 0.7 + vinyl];
        let a = 1.0 - (-TAU * 3200.0 / SR).exp();
        for c in 0..2 {
            self.lp[c] += (mix[c] - self.lp[c]) * a;
            out[c] = (self.lp[c] * 1.5).tanh() * self.gain;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silent_until_gain_then_audible_and_bounded() {
        let mut m = LoFi::new();
        for _ in 0..1000 {
            assert_eq!(m.next_frame(), [0.0, 0.0]);
        }
        m.set_gain(1.0);
        let (mut peak, mut energy) = (0.0f32, 0.0f64);
        // 20 s covers more than a full 4-bar loop (13.3 s).
        for _ in 0..(SR as usize * 20) {
            let f = m.next_frame();
            assert!(f[0].is_finite() && f[1].is_finite());
            peak = peak.max(f[0].abs()).max(f[1].abs());
            energy += (f[0] * f[0]) as f64;
        }
        assert!(peak < 1.0, "peak {peak}");
        assert!(energy > 1.0, "too quiet: {energy}");
    }

    #[test]
    fn fades_out_to_exact_silence() {
        let mut m = LoFi::new();
        m.set_gain(1.0);
        for _ in 0..SR as usize {
            m.next_frame();
        }
        m.set_gain(0.0);
        for _ in 0..(SR as usize * 20) {
            m.next_frame();
        }
        assert_eq!(m.next_frame(), [0.0, 0.0]);
    }
}
