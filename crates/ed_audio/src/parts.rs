//! One-shot voices. Every event (a footstep, thunder, a bird phrase) expands
//! into a handful of `Part`s: a filtered noise burst or a swept tone with an
//! attack/exp-decay envelope, optional AM, inharmonic partials and a pan
//! sweep. Recipes are data in `recipes.rs`.

use crate::dsp::{Brown, Mode, OnePole, Rng, SR, Svf, pan, sin_t};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Src {
    White,
    Brown,
    Sine,
    Square,
    Saw,
}

/// Where a part is mixed (see the mixing rules in `scape.rs`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bus {
    /// Carried through the robot's own body: never gated or ducked.
    Body,
    /// Carried by air near the player: fades with air density, not ducked.
    Air,
    /// World ambience: fades early with altitude, ducked by speed wind, muffled by snow.
    Amb,
    Ui,
}

#[derive(Clone, Copy, Debug)]
pub struct Part {
    pub src: Src,
    /// Frequency (tone) or filter centre (noise), sweeping f0 -> f1 with time constant `sweep_s`.
    pub f0: f32,
    pub f1: f32,
    pub sweep_s: f32,
    /// Noise filter; `None` = unfiltered.
    pub filt: Option<(Mode, f32)>,
    /// Fixed extra low-pass on the output (distance damping), Hz; 0 = off.
    pub lp: f32,
    pub delay: f32,
    pub attack: f32,
    pub decay: f32,
    pub len: f32,
    pub gain: f32,
    pub pan0: f32,
    pub pan1: f32,
    pub bus: Bus,
    pub am_hz: f32,
    pub am_depth: f32,
    /// Extra inharmonic partials as frequency ratios (bells, pings); 0 = unused.
    pub partials: [f32; 3],
    // state
    t: u32,
    ph: [f32; 4],
    am_ph: f32,
    svf: Svf,
    out_lp: OnePole,
    brown: Brown,
    rng: Rng,
    f_now: f32,
}

impl Part {
    fn base(src: Src, f: f32) -> Part {
        Part {
            src,
            f0: f,
            f1: f,
            sweep_s: 1.0,
            filt: None,
            lp: 0.0,
            delay: 0.0,
            attack: 0.002,
            decay: 0.1,
            len: 0.3,
            gain: 0.3,
            pan0: 0.0,
            pan1: 0.0,
            bus: Bus::Body,
            am_hz: 0.0,
            am_depth: 0.0,
            partials: [0.0; 3],
            t: 0,
            ph: [0.0; 4],
            am_ph: 0.0,
            svf: Svf::default(),
            out_lp: OnePole::default(),
            brown: Brown::default(),
            rng: Rng::new(1),
            f_now: f,
        }
    }
    /// Noise through a filter centred at `fc`.
    pub fn noise(mode: Mode, fc: f32, q: f32) -> Part {
        Part { filt: Some((mode, q)), ..Part::base(Src::White, fc) }
    }
    pub fn brown(lp_hz: f32) -> Part {
        Part { filt: Some((Mode::Lp, 0.7)), ..Part::base(Src::Brown, lp_hz) }
    }
    pub fn tone(src: Src, f: f32) -> Part {
        Part::base(src, f)
    }
    pub fn sweep(mut self, to: f32, s: f32) -> Part {
        self.f1 = to;
        self.sweep_s = s.max(1e-3);
        self
    }
    /// attack s, exp-decay time constant s, total length s.
    pub fn env(mut self, attack: f32, decay: f32, len: f32) -> Part {
        self.attack = attack.max(0.0005);
        self.decay = decay.max(0.001);
        self.len = len.max(self.attack + 0.005);
        self
    }
    pub fn at(mut self, delay: f32) -> Part {
        self.delay = delay.max(0.0);
        self
    }
    pub fn gain(mut self, g: f32) -> Part {
        self.gain = g;
        self
    }
    pub fn pan(mut self, p: f32) -> Part {
        self.pan0 = p;
        self.pan1 = p;
        self
    }
    pub fn pan_sweep(mut self, a: f32, b: f32) -> Part {
        self.pan0 = a;
        self.pan1 = b;
        self
    }
    pub fn bus(mut self, b: Bus) -> Part {
        self.bus = b;
        self
    }
    pub fn am(mut self, hz: f32, depth: f32) -> Part {
        self.am_hz = hz;
        self.am_depth = depth.clamp(0.0, 1.0);
        self
    }
    pub fn partials(mut self, p: [f32; 3]) -> Part {
        self.partials = p;
        self
    }
    pub fn lowpass(mut self, hz: f32) -> Part {
        self.lp = hz;
        self
    }
    pub(crate) fn seed(&mut self, s: u32) {
        self.rng = Rng::new(s);
        self.out_lp.set(if self.lp > 0.0 { self.lp } else { 20_000.0 });
        self.retune();
    }

    fn retune(&mut self) {
        let local = (self.t as f32 / SR - self.delay).max(0.0);
        self.f_now = self.f1 + (self.f0 - self.f1) * (-local / self.sweep_s).exp();
        if let Some((_, q)) = self.filt {
            self.svf.set(self.f_now, q);
        }
    }

    pub fn done(&self) -> bool {
        self.t as f32 >= (self.delay + self.len) * SR
    }

    /// Next stereo sample. Cheap while waiting out its delay.
    #[inline]
    pub fn tick(&mut self) -> (f32, f32) {
        let t = self.t;
        self.t += 1;
        let start = self.delay * SR;
        if (t as f32) < start {
            return (0.0, 0.0);
        }
        if t % 32 == 0 {
            self.retune();
        }
        let local = t as f32 / SR - self.delay;
        let env = if local < self.attack {
            local / self.attack
        } else {
            (-(local - self.attack) / self.decay).exp()
        };
        let tail = ((self.len - local) / 0.005).clamp(0.0, 1.0);
        let dt = self.f_now / SR;
        let x = match self.src {
            Src::White => self.rng.bi(),
            Src::Brown => {
                let w = self.rng.bi();
                self.brown.tick(w)
            }
            Src::Sine => {
                let mut s = sin_t(self.ph[0]);
                self.ph[0] = (self.ph[0] + dt).fract();
                for (i, r) in self.partials.iter().enumerate() {
                    if *r > 0.0 {
                        s += sin_t(self.ph[i + 1]) * (0.6 / (i as f32 + 1.0));
                        self.ph[i + 1] = (self.ph[i + 1] + dt * r).fract();
                    }
                }
                s
            }
            Src::Square => {
                let s = if self.ph[0] < 0.5 { 0.7 } else { -0.7 };
                self.ph[0] = (self.ph[0] + dt).fract();
                s
            }
            Src::Saw => {
                let s = self.ph[0] * 2.0 - 1.0;
                self.ph[0] = (self.ph[0] + dt).fract();
                s * 0.7
            }
        };
        let mut y = match self.filt {
            Some((m, _)) => self.svf.run(x, m),
            None => x,
        };
        if self.lp > 0.0 {
            y = self.out_lp.lp(y);
        }
        if self.am_hz > 0.0 {
            let m = 0.5 + 0.5 * sin_t(self.am_ph);
            self.am_ph = (self.am_ph + self.am_hz / SR).fract();
            y *= 1.0 - self.am_depth + self.am_depth * m;
        }
        let g = y * env * tail * self.gain;
        let k = (local / self.len).clamp(0.0, 1.0);
        let (l, r) = pan(self.pan0 + (self.pan1 - self.pan0) * k);
        (g * l, g * r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn part_waits_then_ends() {
        let mut p = Part::noise(Mode::Bp, 1_000.0, 1.0).env(0.001, 0.02, 0.05).at(0.01);
        p.seed(3);
        let mut first = None;
        let mut n = 0;
        while !p.done() {
            let (l, r) = p.tick();
            if first.is_none() && (l != 0.0 || r != 0.0) {
                first = Some(n);
            }
            n += 1;
        }
        let f = first.unwrap();
        assert!((f as f32 - 0.01 * SR).abs() < 2.0, "starts after its delay: {f}");
        assert!((n as f32 - 0.06 * SR).abs() < 2.0);
    }

    #[test]
    fn sweep_reaches_target() {
        let mut p = Part::tone(Src::Sine, 1_000.0).sweep(200.0, 0.01).env(0.001, 1.0, 0.2);
        p.seed(1);
        for _ in 0..(0.15 * SR) as usize {
            p.tick();
        }
        assert!((p.f_now - 200.0).abs() < 1.0);
    }
}
