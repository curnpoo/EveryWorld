//! Line-by-line port of `vendor/engine-sound-kit/special.js`: generators for
//! vehicles that aren't piston engines (fighter jet, steam locomotive, pod
//! racer, warp drive). Driven by the same targets as the engine voice, where
//! "rpm" means whatever turns: the turbine spool (jet, pod, warp) or chuffs
//! per minute (steam). Fixed-size state, no allocation.

use crate::cars::Car;
use crate::dsp::{clamp, coef, smoothstep, Biquad, Rng, TAU};
use crate::voice::{Event, Params, BLOCK};
use std::f64::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Jet,
    Pod,
    Warp,
    Steam,
}

/// Shared: smoothing of the drivetrain's values and the output stage.
#[derive(Clone, Copy)]
struct Base {
    sr: f64,
    rpm: f64,
    load: f64,
    overrun: f64,
    horn: f64,
    k_rpm: f64,
    k_load: f64,
    idle: f64,
    redline: f64,
    gain: f64,
}

impl Base {
    fn new(sr: f64) -> Self {
        Self { sr, rpm: 0.0, load: 0.0, overrun: 0.0, horn: 0.0, k_rpm: coef(sr, 0.03), k_load: coef(sr, 0.05), idle: 0.0, redline: 1.0, gain: 0.5 }
    }
    fn configure(&mut self, car: &Car) {
        self.idle = car.engine.idle;
        self.redline = car.engine.redline;
        self.gain = car.sound.gain;
    }
    #[inline(always)]
    fn follow(&mut self, t: &Params) {
        self.rpm += (t.rpm - self.rpm) * self.k_rpm;
        self.load += (t.load - self.load) * self.k_load;
        self.overrun += (t.overrun - self.overrun) * self.k_load;
        self.horn += (t.horn - self.horn) * self.k_load;
    }
    #[inline(always)]
    fn out(&self, x: f64, drive: f64) -> f64 {
        (x * drive).tanh() * self.gain / drive.tanh()
    }
}

fn lp(sr: f64, hz: f64, q: f64) -> Biquad {
    let mut b = Biquad::default();
    b.lowpass(sr, hz, q);
    b
}
fn bp(sr: f64, hz: f64, q: f64) -> Biquad {
    let mut b = Biquad::default();
    b.bandpass(sr, hz, q);
    b
}
#[inline(always)]
fn frac(x: f64) -> f64 {
    x - x.floor()
}

// ── Fighter jet ─────────────────────────────────────────────────────────
// Fan and compressor whine rising with the spool, a roar that opens up with
// thrust, a buzz-saw edge near full power, and at full throttle the
// afterburner lights: a whump, then a deep tearing rumble full of crackle.
pub struct Jet {
    b: Base,
    roar_l: Biquad,
    roar_r: Biquad,
    rum_l: Biquad,
    rum_r: Biquad,
    ab_l: Biquad,
    ab_r: Biquad,
    crack_bp: Biquad,
    ph1: f64,
    ph2: f64,
    ph3: f64,
    shaft: f64,
    ab: f64,
    ab_lit: bool,
    ign: f64,
    ign_t: f64,
    crack: f64,
}

impl Jet {
    fn new(sr: f64) -> Self {
        Self {
            b: Base::new(sr),
            roar_l: Biquad::default(),
            roar_r: Biquad::default(),
            rum_l: lp(sr, 140.0, 0.7),
            rum_r: lp(sr, 140.0, 0.7),
            ab_l: lp(sr, 380.0, 0.7),
            ab_r: lp(sr, 380.0, 0.7),
            crack_bp: bp(sr, 2400.0, 0.9),
            ph1: 0.0,
            ph2: 0.0,
            ph3: 0.0,
            shaft: 0.0,
            ab: 0.0,
            ab_lit: false,
            ign: 0.0,
            ign_t: 0.0,
            crack: 0.0,
        }
    }
    fn render(&mut self, out_l: &mut [f32], out_r: &mut [f32], tg: &Params, rng: &mut Rng) {
        let sr = self.b.sr;
        let dt = BLOCK as f64 / sr;
        for i in 0..out_l.len() {
            self.b.follow(tg);
            let n = clamp(self.b.rpm / self.b.redline, 0.0, 1.05);
            let load = self.b.load;
            if i % BLOCK == 0 {
                let fc = 250.0 + 3800.0 * n * n;
                self.roar_l.lowpass(sr, fc, 0.6);
                self.roar_r.lowpass(sr, fc * 1.04, 0.6);
                // Afterburner at 60%+ throttle (load = 0.1 + 0.9·throttle), spooled.
                let want = load > 0.64 && n > 0.5;
                if want && !self.ab_lit && self.ab < 0.1 {
                    self.ign = 1.0;
                    self.ign_t = 0.0;
                }
                self.ab_lit = want;
                let (goal, tau) = if want { (1.0, 0.35) } else { (0.0, 0.6) };
                self.ab += (goal - self.ab) * (dt / tau).min(1.0);
            }
            let thrust = 0.05 + n.powf(2.2);
            let (nl, nr) = (rng.bi(), rng.bi());
            let mut l = self.roar_l.process(nl) * thrust * 1.3 + self.rum_l.process(nl) * thrust * 2.2;
            let mut r = self.roar_r.process(nr) * thrust * 1.3 + self.rum_r.process(nr) * thrust * 2.2;
            // Whine: fan blade-pass with a beating sideband, and the compressor above it.
            let (f1, f2) = (900.0 + 2600.0 * n, 2200.0 + 5200.0 * n);
            self.ph1 = frac(self.ph1 + f1 / sr);
            self.ph2 = frac(self.ph2 + f2 / sr);
            self.ph3 = frac(self.ph3 + f1 * 1.06 / sr);
            let whine = 0.1 * (0.4 + 0.6 * n) * ((self.ph1 * TAU).sin() + 0.3 * (self.ph3 * TAU).sin())
                + 0.06 * (0.2 + 0.8 * n) * (self.ph2 * TAU).sin();
            // Buzz-saw: the fan tips go supersonic near full power.
            self.shaft = frac(self.shaft + (60.0 + 160.0 * n) / sr);
            let buzz = (2.0 * self.shaft - 1.0) * 0.07 * smoothstep((n - 0.72) / 0.28) * (0.4 + 0.6 * load);
            l += whine + buzz;
            r += whine * 0.95 + buzz;
            // Afterburner: deep tearing rumble plus crackle, lit with a whump.
            if self.ab > 1e-3 {
                l += self.ab_l.process(rng.bi()) * self.ab * 5.0;
                r += self.ab_r.process(rng.bi()) * self.ab * 5.0;
                if rng.next() < self.ab * 700.0 / sr {
                    self.crack = 0.6 + 0.4 * rng.next();
                }
                if self.crack > 1e-3 {
                    let c = self.crack_bp.process(rng.bi()) * self.crack * 1.1;
                    l += c;
                    r += c * 0.8;
                    self.crack *= (-1.0 / (0.004 * sr)).exp();
                }
            }
            if self.ign > 1e-3 {
                self.ign_t += 1.0 / sr;
                let w = self.ign * ((TAU * 38.0 * self.ign_t).sin() * 2.4 + rng.bi() * 0.8 * (-self.ign_t / 0.05).exp());
                l += w;
                r += w;
                self.ign *= (-1.0 / (0.35 * sr)).exp();
            }
            out_l[i] = self.b.out(l, 1.3) as f32;
            out_r[i] = self.b.out(r, 1.3) as f32;
        }
    }
}

// ── Pod racer ───────────────────────────────────────────────────────────
// Two open turbines, slightly out of tune with each other: a nasal, buzzing
// scream (a sawtooth through a resonance, chopped by a low buzz), a ragged
// roar, and electrical crackle that grows with thrust.
pub struct Pod {
    b: Base,
    res: [Biquad; 2],
    roar: [Biquad; 2],
    zap: Biquad,
    ph: [f64; 2],
    bz: [f64; 2],
    lfo: f64,
    crack: f64,
    crack_pan: f64,
}

impl Pod {
    fn new(sr: f64) -> Self {
        Self {
            b: Base::new(sr),
            res: [Biquad::default(); 2],
            roar: [Biquad::default(); 2],
            zap: bp(sr, 3200.0, 1.2),
            ph: [0.0; 2],
            bz: [0.0; 2],
            lfo: 0.0,
            crack: 0.0,
            crack_pan: 0.0,
        }
    }
    fn render(&mut self, out_l: &mut [f32], out_r: &mut [f32], tg: &Params, rng: &mut Rng) {
        let sr = self.b.sr;
        for i in 0..out_l.len() {
            self.b.follow(tg);
            let n = clamp(self.b.rpm / self.b.redline, 0.0, 1.05);
            let load = self.b.load;
            let thrust = n.powf(1.8);
            self.lfo = frac(self.lfo + 0.7 / sr);
            let det = 0.012 * (self.lfo * TAU).sin();
            let mut out = [0.0f64; 2];
            for s in 0..2 {
                let ns = n * (1.0 + if s == 1 { -det } else { det });
                if i % BLOCK == 0 {
                    self.res[s].bandpass(sr, 1800.0 + 1600.0 * ns, 2.2);
                    self.roar[s].lowpass(sr, 400.0 + 3000.0 * ns, 0.7);
                }
                self.ph[s] = frac(self.ph[s] + (140.0 + 720.0 * ns) / sr);
                self.bz[s] = frac(self.bz[s] + (45.0 + 160.0 * ns) / sr);
                let saw = 2.0 * self.ph[s] - 1.0;
                let chop = if self.bz[s] < 0.5 { 1.0 } else { 0.35 };
                let a = self.res[s].process(saw) * (0.5 + 0.8 * thrust) * chop * 1.4;
                let nz = rng.bi();
                out[s] = a + self.roar[s].process(nz) * thrust * chop * 1.6 + saw * 0.05 * (0.3 + load);
            }
            // Electrical crackle.
            if rng.next() < (15.0 + 90.0 * thrust) / sr {
                self.crack = 0.5 + 0.5 * rng.next();
                self.crack_pan = rng.bi();
            }
            if self.crack > 1e-3 {
                let c = self.zap.process(rng.bi()) * self.crack * 1.4;
                out[0] += c * (1.0 - self.crack_pan) * 0.6;
                out[1] += c * (1.0 + self.crack_pan) * 0.6;
                self.crack *= (-1.0 / (0.004 * sr)).exp();
            }
            out_l[i] = self.b.out(out[0], 2.4) as f32;
            out_r[i] = self.b.out(out[1], 2.4) as f32;
        }
    }
}

// ── Warp drive ──────────────────────────────────────────────────────────
// A deep, chorused core hum that climbs with speed, a faint shimmer on top,
// and on hard acceleration a jump: a rising whine and whoosh that ends in a
// deep boom.
pub struct Warp {
    b: Base,
    ph: [f64; 10],
    lfo: f64,
    shim: Biquad,
    whoosh: Biquad,
    pub jump_t: f64,
    cool: f64,
    prev_load: f64,
    j_ph: f64,
    boom_t: f64,
}

impl Warp {
    fn new(sr: f64) -> Self {
        Self {
            b: Base::new(sr),
            ph: [0.0; 10],
            lfo: 0.0,
            shim: bp(sr, 5500.0, 3.0),
            whoosh: Biquad::default(),
            jump_t: 9.0,
            cool: 9.0,
            prev_load: 0.0,
            j_ph: 0.0,
            boom_t: 9.0,
        }
    }
    fn render(&mut self, out_l: &mut [f32], out_r: &mut [f32], tg: &Params, rng: &mut Rng) {
        const PARTS: [f64; 5] = [1.0, 2.0, 3.0, 4.02, 6.01];
        const AMPS: [f64; 5] = [1.0, 0.6, 0.35, 0.22, 0.12];
        let sr = self.b.sr;
        let dt = BLOCK as f64 / sr;
        for i in 0..out_l.len() {
            self.b.follow(tg);
            let n = clamp(self.b.rpm / self.b.redline, 0.0, 1.05);
            let load = self.b.load;
            if i % BLOCK == 0 {
                self.cool += dt;
                self.jump_t += dt;
                self.boom_t += dt;
                if load > 0.75 && self.prev_load <= 0.75 && self.cool > 5.0 {
                    self.jump_t = 0.0;
                    self.cool = 0.0;
                    self.j_ph = 0.0;
                }
                self.prev_load = load;
                if self.jump_t < 1.25 {
                    self.whoosh.bandpass(sr, 200.0 * 30f64.powf(self.jump_t / 1.2), 1.5);
                }
                if self.jump_t >= 1.2 && self.jump_t < 1.2 + dt * 1.5 {
                    self.boom_t = 0.0;
                }
            }
            self.lfo = frac(self.lfo + 0.13 / sr);
            let f0 = 30.0 + 55.0 * n;
            let level = 0.35 + 0.4 * load + 0.25 * n;
            let (mut l, mut r) = (0.0f64, 0.0f64);
            for k in 0..5 {
                let d = 0.003 * ((self.lfo + k as f64 * 0.2) * TAU).sin();
                self.ph[k] = frac(self.ph[k] + f0 * PARTS[k] * (1.0 + d) / sr);
                self.ph[k + 5] = frac(self.ph[k + 5] + f0 * PARTS[k] * (1.0 - d) / sr);
                l += (self.ph[k] * TAU).sin() * AMPS[k];
                r += (self.ph[k + 5] * TAU).sin() * AMPS[k];
            }
            l *= level * 0.5;
            r *= level * 0.5;
            let sh = self.shim.process(rng.bi()) * (0.03 + 0.05 * n) * (0.6 + 0.4 * (self.lfo * TAU * 3.0).sin());
            l += sh;
            r -= sh;
            if self.jump_t < 1.25 {
                let x = self.jump_t / 1.2;
                let env = (PI * x.min(1.0)).sin() * 0.8 + 0.2 * x;
                self.j_ph = frac(self.j_ph + f0 * 2.0 * 16f64.powf(x) / sr);
                let j = (self.j_ph * TAU).sin() * 0.25 * env + self.whoosh.process(rng.bi()) * 1.2 * env;
                l += j;
                r += j;
            }
            if self.boom_t < 1.5 {
                let b = (TAU * 34.0 * self.boom_t).sin() * 1.8 * (-self.boom_t / 0.45).exp() + rng.bi() * 0.3 * (-self.boom_t / 0.03).exp();
                l += b;
                r += b;
            }
            out_l[i] = self.b.out(l, 1.2) as f32;
            out_r[i] = self.b.out(r, 1.2) as f32;
        }
    }
}

// ── Steam locomotive ────────────────────────────────────────────────────
// Four chuffs per turn of the driving wheels, louder with the regulator
// open; rod clank and rail joints that follow the speed; the air pump
// thumping at a standstill; a three-chime whistle on `horn`; and a
// blow-down of steam on stopping.
#[derive(Clone, Copy)]
struct Chime {
    f: f64,
    ph: f64,
    bp: Biquad,
}

pub struct Steam {
    b: Base,
    puff: Biquad,
    body: Biquad,
    hiss: Biquad,
    clank: Biquad,
    rail: Biquad,
    blow: Biquad,
    chime: [Chime; 3],
    ch_ph: f64,
    ch_n: usize,
    ch_amp: f64,
    ch_t: f64,
    ch_tau: f64,
    rod_ph: f64,
    clank_env: f64,
    // Pending rail-joint clicks (seconds until each), oldest first.
    pub rail_x: f64,
    clicks: [f64; 16],
    n_clicks: usize,
    pub pump_t: f64,
    pump_env: f64,
    pump_ph: f64,
    blow_env: f64,
    horn_t: f64,
    tire: f64,
}

impl Steam {
    fn new(sr: f64) -> Self {
        let chime = [349.0, 440.0, 523.0].map(|f| Chime { f, ph: 0.0, bp: bp(sr, f, 30.0) });
        Self {
            b: Base::new(sr),
            puff: bp(sr, 1100.0, 0.8),
            body: lp(sr, 260.0, 0.7),
            hiss: bp(sr, 6000.0, 0.5),
            clank: bp(sr, 2800.0, 3.0),
            rail: bp(sr, 1800.0, 2.0),
            blow: bp(sr, 2500.0, 0.5),
            chime,
            ch_ph: 0.0,
            ch_n: 0,
            ch_amp: 0.0,
            ch_t: 9.0,
            ch_tau: 0.1,
            rod_ph: 0.0,
            clank_env: 0.0,
            rail_x: 0.0,
            clicks: [0.0; 16],
            n_clicks: 0,
            pump_t: 0.0,
            pump_env: 0.0,
            pump_ph: 0.0,
            blow_env: 0.0,
            horn_t: 0.0,
            tire: 1.0,
        }
    }
    fn render(&mut self, out_l: &mut [f32], out_r: &mut [f32], tg: &Params, rng: &mut Rng) {
        const ACCENT: [f64; 4] = [1.0, 0.75, 0.9, 0.7];
        let sr = self.b.sr;
        for i in 0..out_l.len() {
            self.b.follow(tg);
            let rate = self.b.rpm.max(0.0) / 60.0; // chuffs per second
            let n = clamp(self.b.rpm / self.b.redline, 0.0, 1.05);
            let load = self.b.load;
            let (mut l, mut r) = (0.0f64, 0.0f64);
            // Chuffs.
            if rate > 0.2 {
                self.ch_ph += rate / sr;
                if self.ch_ph >= 1.0 {
                    self.ch_ph -= 1.0;
                    self.ch_n = (self.ch_n + 1) % 4;
                    self.ch_t = 0.0;
                    self.ch_amp = (0.15 + 0.85 * load) * ACCENT[self.ch_n] * (0.9 + 0.2 * rng.next());
                    self.ch_tau = 0.16f64.min(0.5 / rate);
                }
            }
            if self.ch_t < 1.0 {
                self.ch_t += 1.0 / sr;
                let e = self.ch_amp * (self.ch_t / 0.004).min(1.0) * (-self.ch_t / self.ch_tau).exp();
                let nz = rng.bi();
                let c = (self.puff.process(nz) * 0.9 + self.body.process(nz) * 1.6) * e;
                l += c;
                r += c * 0.9;
            }
            // Rods: two clanks per turn of the wheels.
            if rate > 0.2 {
                self.rod_ph += rate / 2.0 / sr;
                if self.rod_ph >= 1.0 {
                    self.rod_ph -= 1.0;
                    self.clank_env = 0.15 * (0.3 + n);
                }
            }
            if self.clank_env > 1e-4 {
                let c = self.clank.process(rng.bi()) * self.clank_env;
                l += c * 0.7;
                r += c;
                self.clank_env *= (-1.0 / (0.008 * sr)).exp();
            }
            // Rail joints every 18 m, front and rear truck: clickety-clack.
            let v = rate / 4.0 * self.tire; // m/s
            if v > 1.0 {
                self.rail_x += v / sr;
                if self.rail_x >= 18.0 && self.n_clicks < 15 {
                    self.rail_x -= 18.0;
                    self.clicks[self.n_clicks] = 0.0;
                    self.clicks[self.n_clicks + 1] = 2.5 / v;
                    self.n_clicks += 2;
                }
            }
            let mut k = 0;
            while k < self.n_clicks {
                self.clicks[k] -= 1.0 / sr;
                let ck = self.clicks[k];
                if ck <= 0.0 && ck > -0.015 {
                    let tt = -ck;
                    let c = (self.rail.process(rng.bi()) * 0.5 + (TAU * 120.0 * tt).sin() * 0.6) * (-tt / 0.006).exp() * 0.25;
                    l += c;
                    r += c;
                    k += 1;
                } else if ck <= -0.015 {
                    // Done: close the gap.
                    self.clicks[k..self.n_clicks].rotate_left(1);
                    self.n_clicks -= 1;
                } else {
                    k += 1;
                }
            }
            // Standing: the air pump thumps.
            if rate <= 0.2 {
                self.pump_t += 1.0 / sr;
                if self.pump_t > 1.1 {
                    self.pump_t = 0.0;
                    self.pump_env = 1.0;
                    self.pump_ph = 0.0;
                }
            }
            if self.pump_env > 1e-3 {
                self.pump_ph += 70.0 / sr;
                let p = self.pump_env * ((self.pump_ph * TAU).sin() * 0.35 + self.puff.process(rng.bi()) * 0.08);
                l += p;
                r += p;
                self.pump_env *= (-1.0 / (0.08 * sr)).exp();
            }
            // Boiler hiss, louder working hard; and the blow-down on stopping.
            let hs = self.hiss.process(rng.bi()) * (0.015 + if rate > 0.2 { 0.02 + 0.03 * load } else { 0.0 });
            l += hs;
            r += hs * 0.8;
            if self.blow_env > 1e-3 {
                let b = self.blow.process(rng.bi()) * self.blow_env * 0.9;
                l += b;
                r += b;
                self.blow_env *= (-1.0 / (1.1 * sr)).exp();
            }
            // Whistle: three chimes with breath, sliding up into pitch.
            if self.b.horn > 1e-3 {
                self.horn_t += 1.0 / sr;
                let bend = 1.0 - 0.04 * (-self.horn_t / 0.12).exp();
                let mut w = 0.0;
                for c in self.chime.iter_mut() {
                    c.ph = frac(c.ph + c.f * bend / sr);
                    w += (c.ph * TAU).sin() * 0.18 + c.bp.process(rng.bi()) * 0.6;
                }
                w *= self.b.horn;
                l += w;
                r += w;
            } else {
                self.horn_t = 0.0;
            }
            out_l[i] = self.b.out(l * 4.0, 1.4) as f32;
            out_r[i] = self.b.out(r * 4.0, 1.4) as f32;
        }
    }
}

/// One of the special generators, held inline in the voice (no heap).
pub enum Special {
    Jet(Jet),
    Pod(Pod),
    Warp(Warp),
    Steam(Steam),
}

impl Special {
    pub fn kind_of(kind: &str) -> Option<Kind> {
        Some(match kind {
            "jet" => Kind::Jet,
            "pod" => Kind::Pod,
            "warp" => Kind::Warp,
            "steam" => Kind::Steam,
            _ => return None,
        })
    }
    pub fn new(kind: Kind, sr: f64) -> Self {
        match kind {
            Kind::Jet => Special::Jet(Jet::new(sr)),
            Kind::Pod => Special::Pod(Pod::new(sr)),
            Kind::Warp => Special::Warp(Warp::new(sr)),
            Kind::Steam => Special::Steam(Steam::new(sr)),
        }
    }
    pub fn kind(&self) -> Kind {
        match self {
            Special::Jet(_) => Kind::Jet,
            Special::Pod(_) => Kind::Pod,
            Special::Warp(_) => Kind::Warp,
            Special::Steam(_) => Kind::Steam,
        }
    }
    pub fn configure(&mut self, car: &Car) {
        match self {
            Special::Jet(g) => g.b.configure(car),
            Special::Pod(g) => g.b.configure(car),
            Special::Warp(g) => g.b.configure(car),
            Special::Steam(g) => {
                g.b.configure(car);
                g.tire = car.engine.tire;
            }
        }
    }
    pub fn event(&mut self, ev: Event, _amt: f64) {
        if let Special::Steam(g) = self
            && ev == Event::Stop
        {
            g.blow_env = 1.0;
        }
    }
    pub fn render(&mut self, out_l: &mut [f32], out_r: &mut [f32], tg: &Params, rng: &mut Rng) {
        match self {
            Special::Jet(g) => g.render(out_l, out_r, tg, rng),
            Special::Pod(g) => g.render(out_l, out_r, tg, rng),
            Special::Warp(g) => g.render(out_l, out_r, tg, rng),
            Special::Steam(g) => g.render(out_l, out_r, tg, rng),
        }
    }
}
