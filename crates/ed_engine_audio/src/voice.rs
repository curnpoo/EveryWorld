//! `EngineVoice`: a line-by-line port of `vendor/engine-sound-kit/synth.js`.
//!
//! Signal path, per sample:
//!   crank angle → cylinder firings (the car's firing order) → an exhaust
//!   pressure pulse plus combustion noise into that cylinder's bank, after its
//!   header delay → turbine smoothing → X/H-pipe crossover → tailpipe
//!   waveguides (delay line with a lossy, inverting reflection) panned to
//!   stereo → body resonances → muffler low-pass → plus intake roar,
//!   valvetrain, turbo whistle / blow-off, supercharger whine, gearbox whine
//!   and afterfire pops → DC block → saturation.
//!
//! The JS is the source of truth. Comments from it are kept where they explain
//! the sound; numeric expressions keep the JS evaluation order so the port
//! tracks the reference closely (state is f64 like JS numbers, f32 exactly
//! where the JS stores into a Float32Array).
//!
//! Real-time rules (README "Performance rules"): nothing here allocates after
//! [`EngineVoice::new`]. Slots (pulses, pops, bangs, flames), pipes and
//! per-cylinder tables are fixed-size, and `configure` only rewrites them, so
//! switching cars on the audio thread is safe. There is no JIT, so the JS
//! `warmUp()` has no Rust counterpart.

use crate::cars::{Bov, Car, Sound};
use crate::dsp::{clamp, coef, lerp, or, sign, smoothstep, Biquad, Pipe, Rng, TAU};
use crate::special::Special;
use std::f64::consts::{PI, SQRT_2};
use std::sync::OnceLock;

/// Controls are applied every BLOCK samples (the muffler, turbo and
/// smoothing of overrun/starter/shaft), as in the JS.
pub const BLOCK: usize = 32;
const MAX_PULSES: usize = 48;
const MAX_POPS: usize = 16;
/// Up to 6 ringing + a burst of up to 10.
const MAX_BANGS: usize = 16;
const MAX_FLAMES: usize = 4;
const SHAPE_N: usize = 2048;
/// Pulses are computed out to 8 pulse widths.
const SHAPE_END: f64 = 8.0;
/// Fixed capacities (the kit's largest: 24 cylinders, 2 pipes, 4 body
/// resonances; the JS banks array is 4 long).
pub const MAX_CYL: usize = 32;
pub const MAX_PIPES: usize = 4;
const MAX_BANKS: usize = 4;
/// Load-equivalent of engine braking with body on (a light cruise is ~0.15-0.2).
const BODY: f64 = 0.18;

struct Shapes {
    pulse: [f32; SHAPE_N + 1],
    burst: [f32; SHAPE_N + 1],
}

/// Pulse shape x·e^(1-x) (peaks at 1 when x = 1, long smooth tail) and the
/// envelope of the noise burst riding on it, tabulated over 0..SHAPE_END
/// (Float32Array in the JS).
fn shapes() -> &'static Shapes {
    static S: OnceLock<Shapes> = OnceLock::new();
    S.get_or_init(|| {
        let mut s = Shapes { pulse: [0.0; SHAPE_N + 1], burst: [0.0; SHAPE_N + 1] };
        for i in 0..=SHAPE_N {
            let x = i as f64 / SHAPE_N as f64 * SHAPE_END;
            s.pulse[i] = (x * (1.0 - x).exp()) as f32;
            s.burst[i] = ((-x / 1.4).exp() * (x * 4.0).min(1.0)) as f32;
        }
        s
    })
}

/// What the game sends every frame (README "Driving it from a game").
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Params {
    /// Crank rpm (idle .. limiter). For turbines: spool; steam: chuffs/min.
    pub rpm: f64,
    /// 0..1 combustion strength: ~0.12 + 0.88 × throttle on throttle, 0 off it.
    pub load: f64,
    /// 0..1: 1 = closed throttle above idle (engine braking).
    pub overrun: f64,
    /// 1 while the ignition is cut (rev limiter, upshift cut).
    pub cut: f64,
    /// 0..1 while cranking at start-up.
    pub starter: f64,
    /// Gearbox output rpm (gearbox whine); 0 when stopped.
    pub shaft: f64,
    /// Steam whistle (steam train only).
    pub horn: f64,
    /// 0..1 gentle pull-away from a stop (softens the low boom).
    pub pull_away: f64,
    /// 0..1 optional: keeps engine braking at a light-cruise level.
    pub body: f64,
}

/// Drivetrain events (`voice.event(type, strength)` in the JS).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// An upshift (strength = throttle).
    Up,
    /// A downshift.
    Down,
    /// Snapping off the throttle from high revs (blow-off, pop or bang).
    Lift,
    /// The start-up bark.
    Start,
    /// Came to a stop (air-brake hiss on the semi, blow-down on the steam train).
    Stop,
}

impl Event {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "up" => Event::Up,
            "down" => Event::Down,
            "lift" => Event::Lift,
            "start" => Event::Start,
            "stop" => Event::Stop,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Default)]
struct Pulse {
    on: bool,
    wait: f64,
    age: f64,
    step: f64,
    amp: f64,
    n_amp: f64,
    bank: usize,
}

#[derive(Clone, Copy, Default)]
struct Pop {
    on: bool,
    wait: f64,
    age: f64,
    amp: f64,
    pan: f64,
    hz: f64,
    body: f64,
    n1: f64,
    n2: f64,
}

#[derive(Clone, Copy, Default)]
struct Bang {
    wait: f64,
    t: f64,
    amp: f64,
    ph: f64,
    sub: f64,
    lp: f64,
    lp2: f64,
    single: bool,
    bp: f64,
    bp2: f64,
    crack_mul: f64,
}

#[derive(Clone, Copy, Default)]
struct Flame {
    wait: f64,
    t: f64,
    amp: f64,
    lp: f64,
    lp2: f64,
    ph: f64,
}

#[derive(Clone, Copy, Default)]
struct Firing {
    deg: f64,
    bank: usize,
    idx: usize,
}

#[derive(Clone, Copy, Default)]
struct PipeCfg {
    bank: usize,
    gl: f64,
    gr: f64,
}

#[derive(Clone, Copy, Default)]
struct DcBlock {
    x: f64,
    y: f64,
}

pub struct EngineVoice {
    sr: f64,
    pub rng: Rng,
    pub target: Params,
    rpm: f64,
    load: f64,
    overrun: f64,
    cut: f64,
    starter: f64,
    shaft: f64,
    body: f64,
    character: f64,
    crackle_user: f64,
    k_rpm: f64,
    k_load: f64,

    to_next: f64,
    next: usize,
    fire_lag: f64,
    pulses: [Pulse; MAX_PULSES],
    /// Pulses at this index and above are all off.
    pulse_top: usize,
    pops: [Pop; MAX_POPS],

    bank_in: [f32; MAX_BANKS],
    turb_lp: [f32; MAX_BANKS],
    pipes: [Pipe; MAX_PIPES],
    body_l: [Biquad; 6],
    body_r: [Biquad; 6],
    muf_l: Biquad,
    muf_r: Biquad,
    bass_l: Biquad,
    bass_r: Biquad,
    tone_l: Biquad,
    tone_r: Biquad,
    thump_bp: Biquad,
    thump_bp2: Biquad,
    intake_bp: Biquad,
    mech_bp: Biquad,
    hiss_bp: Biquad,
    rush_bp: Biquad,
    bov_bp: Biquad,
    starter_bp: Biquad,
    click_bp: Biquad,
    pop_lp: f64,

    firing_env: f64,
    mech_env: f64,
    /// Turbo boost 0..1 (for a gauge).
    pub boost: f64,
    turbo_spool: f64,
    peak_load: f64,
    peak_boost: f64,
    bov_env: f64,
    bov_t: f64,
    bov_phase: f64,
    bov_lp: f64,
    bov_armed: bool,
    starter_rpm: f64,
    starter_ph2: f64,
    cam_ph: f64,
    mix_e: f64,
    mix_b: f64,
    mix_p: f64,
    whistle_ph: f64,
    whistle_ph2: f64,
    whistle_u: f64,
    w_pf: f64,
    w_ps: f64,
    blow_ph: f64,
    blow_ph2: f64,
    gear_ph: f64,
    starter_ph: f64,
    click_env: f64,
    click_t: f64,
    thump_ph: f64,
    // Backfires and flames live in fixed slots, reused; bangs[0..n_bangs)
    // are the live ones, oldest first; likewise flames.
    bangs: [Bang; MAX_BANGS],
    n_bangs: usize,
    air_bp: Biquad,
    air_env: f64,
    air_t: f64,
    /// Anti-lag: time since the last big bang.
    al_t: f64,
    flames: [Flame; MAX_FLAMES],
    n_flames: usize,
    // Cartoon: kazoo phase and formants, the springy rev wobble, the honk.
    kaz_ph: f64,
    kaz_f1: Biquad,
    kaz_f2: Biquad,
    spring: f64,
    spring_v: f64,
    last_rpm: f64,
    honk_t: f64,
    honk_ph: f64,
    honk_bp: Biquad,
    dc_l: DcBlock,
    dc_r: DcBlock,

    // Set by configure().
    car: &'static Car,
    special: Option<Special>,
    idle: f64,
    redline: f64,
    firing: [Firing; MAX_CYL],
    gaps: [f64; MAX_CYL],
    trim: [f64; MAX_CYL],
    header: [f64; MAX_CYL],
    n_cyl: usize,
    n_banks: usize,
    pipe_cfg: [PipeCfg; MAX_PIPES],
    n_pipes: usize,
    pipe_norm: f64,
    n_body: usize,
    tone_on: bool,
    turb_k: f64,
    bov_lp_k: f64,
    w_pf_k: f64,
    w_ps_k: f64,
    turbo_disp: f64,
    turbo_flow_ref: f64,
}

impl EngineVoice {
    /// `sr`: sample rate; `car`: a resolved car (the catalog's are);
    /// `seed`: the noise seed (0 picks the JS default 12345).
    pub fn new(sr: f64, car: &'static Car, seed: u32) -> Self {
        Self::build(sr, car, seed, std::array::from_fn(|_| Pipe::new(sr)))
    }

    /// Start over as a brand-new voice for `car` (same as `new`, without
    /// allocating: the pipe buffers are kept and cleared). For reusing a
    /// mixer slot on the audio thread.
    pub fn reset(&mut self, car: &'static Car, seed: u32) {
        let mut pipes = std::mem::replace(&mut self.pipes, std::array::from_fn(|_| Pipe::empty()));
        for p in &mut pipes {
            p.clear();
        }
        *self = Self::build(self.sr, car, seed, pipes);
    }

    fn build(sr: f64, car: &'static Car, seed: u32, pipes: [Pipe; MAX_PIPES]) -> Self {
        let _ = shapes();
        let mut mech_bp = Biquad::default();
        mech_bp.bandpass(sr, 4200.0, 1.2);
        let mut hiss_bp = Biquad::default();
        hiss_bp.bandpass(sr, 5200.0, 0.7);
        let mut starter_bp = Biquad::default();
        starter_bp.bandpass(sr, 1800.0, 2.0);
        let mut click_bp = Biquad::default();
        click_bp.bandpass(sr, 2600.0, 1.5);
        let mut air_bp = Biquad::default();
        air_bp.bandpass(sr, 3800.0, 0.7);
        let mut honk_bp = Biquad::default();
        honk_bp.bandpass(sr, 1100.0, 1.2);
        let mut v = Self {
            sr,
            rng: Rng::new(if seed == 0 { 12345 } else { seed }),
            target: Params::default(),
            rpm: 0.0,
            load: 0.0,
            overrun: 0.0,
            cut: 0.0,
            starter: 0.0,
            shaft: 0.0,
            body: 0.0,
            character: 0.5,
            crackle_user: 0.5,
            k_rpm: coef(sr, 0.018),
            k_load: coef(sr, 0.03),
            to_next: 0.0,
            next: 0,
            fire_lag: 0.0,
            pulses: [Pulse::default(); MAX_PULSES],
            pulse_top: 0,
            pops: [Pop::default(); MAX_POPS],
            bank_in: [0.0; MAX_BANKS],
            turb_lp: [0.0; MAX_BANKS],
            pipes,
            body_l: [Biquad::default(); 6],
            body_r: [Biquad::default(); 6],
            muf_l: Biquad::default(),
            muf_r: Biquad::default(),
            bass_l: Biquad::default(),
            bass_r: Biquad::default(),
            tone_l: Biquad::default(),
            tone_r: Biquad::default(),
            thump_bp: Biquad::default(),
            thump_bp2: Biquad::default(),
            intake_bp: Biquad::default(),
            mech_bp,
            hiss_bp,
            rush_bp: Biquad::default(),
            bov_bp: Biquad::default(),
            starter_bp,
            click_bp,
            pop_lp: 0.0,
            firing_env: 0.0,
            mech_env: 0.0,
            boost: 0.0,
            turbo_spool: 0.0,
            peak_load: 0.0,
            peak_boost: 0.0,
            bov_env: 0.0,
            bov_t: 9.0,
            bov_phase: 0.0,
            bov_lp: 0.0,
            bov_armed: false,
            starter_rpm: 0.0,
            starter_ph2: 0.0,
            cam_ph: 0.0,
            mix_e: 1.0,
            mix_b: 1.0,
            mix_p: 1.0,
            whistle_ph: 0.0,
            whistle_ph2: 0.0,
            whistle_u: 0.0,
            w_pf: 0.0,
            w_ps: 0.0,
            blow_ph: 0.0,
            blow_ph2: 0.0,
            gear_ph: 0.0,
            starter_ph: 0.0,
            click_env: 0.0,
            click_t: 1.0,
            thump_ph: 0.0,
            bangs: [Bang::default(); MAX_BANGS],
            n_bangs: 0,
            air_bp,
            air_env: 0.0,
            air_t: 9.0,
            al_t: 9.0,
            flames: [Flame::default(); MAX_FLAMES],
            n_flames: 0,
            kaz_ph: 0.0,
            kaz_f1: Biquad::default(),
            kaz_f2: Biquad::default(),
            spring: 0.0,
            spring_v: 0.0,
            last_rpm: 0.0,
            honk_t: 9.0,
            honk_ph: 0.0,
            honk_bp,
            dc_l: DcBlock::default(),
            dc_r: DcBlock::default(),
            car,
            special: None,
            idle: 0.0,
            redline: 1.0,
            firing: [Firing::default(); MAX_CYL],
            gaps: [720.0; MAX_CYL],
            trim: [0.0; MAX_CYL],
            header: [0.0; MAX_CYL],
            n_cyl: 1,
            n_banks: 1,
            pipe_cfg: [PipeCfg::default(); MAX_PIPES],
            n_pipes: 0,
            pipe_norm: 1.0,
            n_body: 0,
            tone_on: false,
            turb_k: 0.0,
            bov_lp_k: 0.0,
            w_pf_k: 0.0,
            w_ps_k: 0.0,
            turbo_disp: 2.0,
            turbo_flow_ref: 3.0,
        };
        v.configure(car);
        v
    }

    pub fn sample_rate(&self) -> f64 {
        self.sr
    }

    pub fn car(&self) -> &'static Car {
        self.car
    }

    /// Safe to call while running: the crank keeps turning and nothing
    /// clicks. Does not allocate.
    pub fn configure(&mut self, car: &'static Car) {
        let sr = self.sr;
        let s = &car.sound;
        self.car = car;
        // Not an engine at all (jet, steam, ...): its own generator does the work.
        match Special::kind_of(&car.kind) {
            Some(kind) => {
                let same = self.special.as_ref().is_some_and(|sp| sp.kind() == kind);
                if !same {
                    self.special = Some(Special::new(kind, sr));
                }
                if let Some(sp) = self.special.as_mut() {
                    sp.configure(car);
                }
            }
            None => self.special = None,
        }
        self.idle = car.engine.idle;
        self.redline = car.engine.redline;
        let n = car.firing.len().clamp(1, MAX_CYL);
        for i in 0..n {
            let f = car.firing.get(i).copied().unwrap_or([0.0, 0.0]);
            self.firing[i] = Firing { deg: f[0], bank: f[1] as usize, idx: i };
        }
        // Stable sort by angle (JS Array.prototype.sort is stable), in place.
        self.firing[..n].sort_by(|a, b| a.deg.total_cmp(&b.deg));
        self.n_cyl = n;
        let cyc = if car.cycle > 0.0 { car.cycle } else { 720.0 }; // 360 for a two-stroke
        for i in 0..n {
            let g = (self.firing[(i + 1) % n].deg - self.firing[i].deg + cyc) % cyc;
            self.gaps[i] = if g != 0.0 { g } else { cyc };
        }
        if self.next >= n {
            self.next = 0;
        }

        // Fixed cylinder-to-cylinder differences, the same every time for a car.
        let mut tr = Rng::new((n * 7919 + 17) as u32);
        for i in 0..n {
            self.trim[i] = tr.bi() * s.trim;
        }
        for i in 0..n {
            let h = s.header.as_ref().and_then(|h| h.get(i).copied()).unwrap_or(0.0);
            self.header[i] = if h != 0.0 && !h.is_nan() { h * sr / 1000.0 } else { 0.0 };
        }

        let max_bank = car.firing.iter().map(|f| f[1] as usize + 1).max().unwrap_or(1);
        self.n_banks = max_bank.clamp(1, MAX_BANKS);
        // Pipes sharing a bank split its flow, so each gets an equal share.
        let np = s.pipes.len().min(MAX_PIPES);
        let mut per_bank = [0usize; MAX_BANKS];
        for p in &s.pipes[..np] {
            per_bank[p.bank.min(self.n_banks - 1)] += 1;
        }
        for (i, p) in s.pipes[..np].iter().enumerate() {
            let bank = p.bank.min(self.n_banks - 1);
            let a = (clamp(p.pan, -1.0, 1.0) + 1.0) * PI / 4.0;
            let w = SQRT_2 / per_bank[bank] as f64;
            self.pipe_cfg[i] = PipeCfg { bank, gl: a.cos() * w, gr: a.sin() * w };
            self.pipes[i].set(sr, p.ms, s.reflect, s.damp);
        }
        self.n_pipes = np;
        self.pipe_norm = 1.0 / (self.n_banks as f64).sqrt();

        for i in 0..6 {
            match s.body.get(i) {
                Some(b) => {
                    self.body_l[i].peak(sr, b.hz, b.q, b.db);
                    self.body_r[i].peak(sr, b.hz, b.q, b.db);
                }
                None => {
                    self.body_l[i].peak(sr, 1000.0, 1.0, 0.0);
                    self.body_r[i].peak(sr, 1000.0, 1.0, 0.0);
                }
            }
        }
        self.n_body = s.body.len().min(6);

        // Thump: every firing rings a low resonance, the chest-felt boom of combustion.
        self.thump_bp.bandpass(sr, s.thump.hz, s.thump.q);
        self.thump_bp2.bandpass(sr, s.thump.hz, s.thump.q);
        self.bass_l.lowshelf(sr, s.bass.hz, s.bass.db);
        self.bass_r.lowshelf(sr, s.bass.hz, s.bass.db);
        // Optional final tone: rolls the top off the engine (not the turbo or
        // bangs laid on top).
        self.tone_on = s.tone_hz > 0.0;
        if self.tone_on {
            self.tone_l.lowpass(sr, s.tone_hz, 0.7);
            self.tone_r.lowpass(sr, s.tone_hz, 0.7);
        }

        let t = &s.turbo;
        self.turb_k = 1.0 - (-TAU * lerp(14000.0, 1400.0, clamp(t.damp * t.amt, 0.0, 1.0)) / sr).exp();
        self.hiss_bp.bandpass(sr, or(t.hiss_hz, 5200.0), or(t.hiss_q, 0.7));
        self.rush_bp.bandpass(sr, 5200.0, 0.7);
        self.bov_lp_k = 1.0 - (-TAU * 2500.0 / sr).exp();
        self.w_pf_k = 1.0 - (-TAU * 150.0 / sr).exp();
        self.w_ps_k = 1.0 - (-TAU * 3.0 / sr).exp();

        // Turbo model: displacement drives exhaust flow; spool is shaft speed,
        // boost is compressor pressure.
        self.turbo_disp = or(car.engine.disp.unwrap_or(0.0), 2.0); // litres
        self.turbo_flow_ref = 3.0; // 3 L = reference flow (1.0)
    }

    pub fn set_params(&mut self, p: &Params) {
        self.target = *p;
    }

    /// Mix: engine, turbo/supercharger and pops/bangs levels, 1 = as voiced.
    pub fn set_mix(&mut self, engine: f64, boost: f64, pops: f64) {
        self.mix_e = clamp(engine, 0.0, 2.0);
        self.mix_b = clamp(boost, 0.0, 2.0);
        self.mix_p = clamp(pops, 0.0, 2.0);
    }

    /// character 0..1 is stock to open pipes (0.75 the default in the app);
    /// crackle 0..1.
    pub fn set_user(&mut self, character: f64, crackle: f64) {
        self.character = clamp(character, 0.0, 1.0);
        self.crackle_user = clamp(crackle, 0.0, 1.0);
    }

    /// A manual gearbox's clutch/gear click.
    pub fn click(&mut self, amt: f64) {
        self.click_env = self.click_env.max(amt);
        self.click_t = 0.0;
    }

    /// Drivetrain events: up / down / lift / start fire backfires; stop lets
    /// the air brakes hiss.
    pub fn event(&mut self, ev: Event, amt: f64) {
        if let Some(sp) = self.special.as_mut() {
            sp.event(ev, amt);
            return;
        }
        let s = &self.car.sound;
        if ev == Event::Stop {
            if s.air_brake > 0.0 {
                self.air_env = s.air_brake;
                self.air_t = 0.0;
            }
            return;
        }
        // Letting off or shifting up with boost on: the turbo vents.
        // (A quick DCT upshift barely closes the throttle: bovShift 0 keeps it quiet.)
        if ev == Event::Lift {
            self.release(amt);
        } else if ev == Event::Up && s.turbo.bov_shift != Some(0.0) {
            self.release(amt * s.turbo.bov_shift.unwrap_or(1.0));
        }
        let g = s.gunshot;
        if g.amt > 0.0 && ev == Event::Lift {
            self.gunshot(amt);
            return;
        }
        if g.flame > 0.0 && ev == Event::Up {
            self.flame(0.5 * g.flame * amt, 25.0);
        }
        self.bang(amt);
    }

    // Turbo pressure let go: a blow-off valve's "pssh" (or the flutter's
    // "stu-stu-stu"), then the whistle spools down quickly. Volume follows the
    // trapped boost pressure.
    fn release(&mut self, strength: f64) {
        let t = &self.car.sound.turbo;
        // One vent per boost build.
        if t.amt <= 0.0 || t.bov == Bov::None || self.boost < 0.08 || self.bov_t < 0.25 || !self.bov_armed {
            return;
        }
        self.bov_armed = false;
        self.bov_env = (self.boost * 1.1 + 0.08).min(1.3) * t.bov_amt * (0.6 + 0.4 * clamp(strength, 0.0, 1.0));
        self.bov_t = 0.0;
        self.bov_phase = 0.0;
        self.boost *= 0.3;
        self.turbo_spool *= 0.4;
        // One lift, one vent: forget the peaks.
        self.peak_load = 0.0;
        self.peak_boost = 0.0;
    }

    /// Lift-off gunshot: one huge, dry crack-and-boom, then (if the car spits
    /// fire) a fireball that rolls out of the pipes a beat later.
    pub fn gunshot(&mut self, strength: f64) {
        let g = self.car.sound.gunshot;
        if self.n_bangs > 6 {
            return;
        }
        let amt = g.amt * self.crackle_user * 2.0 * (0.6 + 0.4 * strength);
        if amt < 0.02 {
            return;
        }
        self.add_bang(8.0 * self.sr / 1000.0, amt, true, 1.9);
        if g.flame > 0.0 {
            self.flame(g.flame * strength * self.crackle_user * 2.0, 14.0);
        }
    }

    /// Flame: a soft low "whoomp" of burning fuel, then a few hard crackles.
    pub fn flame(&mut self, amp: f64, delay_ms: f64) {
        if amp < 0.02 || self.n_flames > 3 {
            return;
        }
        let f = &mut self.flames[self.n_flames];
        self.n_flames += 1;
        *f = Flame { wait: delay_ms * self.sr / 1000.0, t: 0.0, amp, lp: 0.0, lp2: 0.0, ph: 0.0 };
    }

    /// A single loud backfire: a crack, then a deep boom whose pitch drops,
    /// then a low rumble. Some cars fire a burst ("ra-ta-ta").
    pub fn bang(&mut self, strength: f64) {
        let s: &'static Sound = &self.car.sound;
        if s.toon > 0.0 {
            // a-wooga
            if self.honk_t > 0.6 {
                self.honk_t = 0.0;
                self.honk_ph = 0.0;
            }
            return;
        }
        let amt = s.bang * self.crackle_user * 2.0 * strength;
        if amt < 0.02 || self.n_bangs > 6 {
            return;
        }
        let [lo, hi] = s.bang_burst;
        let count = (lo + self.rng.next() * (hi - lo) * (strength + 0.2).min(1.0)).round().max(1.0) as usize;
        let mut wait = (15.0 + self.rng.next() * 30.0) * self.sr / 1000.0;
        for i in 0..count {
            let a = amt * if i == 0 { 1.0 } else { 0.55 + 0.45 * self.rng.next() };
            self.add_bang(wait, a, count == 1, 1.0);
            wait += s.bang_gap_ms * (0.7 + 0.6 * self.rng.next()) * self.sr / 1000.0;
        }
    }

    fn add_bang(&mut self, wait: f64, amp: f64, single: bool, crack_mul: f64) {
        if self.n_bangs >= MAX_BANGS {
            return; // (the JS caps the count before it gets here)
        }
        self.bangs[self.n_bangs] =
            Bang { wait, t: 0.0, amp, ph: 0.0, sub: 0.0, lp: 0.0, lp2: 0.0, single, bp: 0.0, bp2: 0.0, crack_mul };
        self.n_bangs += 1;
    }

    fn fire(&mut self, fi: usize) {
        let f = self.firing[fi];
        let s: &'static Sound = &self.car.sound;
        let rpm = self.rpm;
        let rpm_n = clamp((rpm - self.idle) / (self.redline - self.idle), 0.0, 1.0);
        let ch = self.character;
        let (load, cut) = (self.load, self.cut);

        // Pumping pulse (the exhaust valve opens even without combustion) plus
        // combustion, which grows with load and vanishes while the spark is cut.
        let ov = self.overrun * (1.0 - load);
        let lb = load.max(BODY * self.body * self.overrun);
        let mut amp = 0.13 * (1.0 - 0.3 * ov) + (1.0 - cut) * (0.12 + 0.88 * lb);
        // Idle roll: how much of the cylinder-to-cylinder unevenness survives
        // near idle; fades back to 1 by +1,500 rpm.
        let idle_n = 1.0 - smoothstep((rpm - self.idle) / 1500.0);
        let roll = 1.0 + (s.idle_roll - 1.0) * idle_n;
        amp *= (0.55 + 0.45 * rpm_n) * (1.0 + self.trim[f.idx] * roll) * (1.0 + s.rough * roll * 2.0 * self.rng.bi());
        // Lumpy cam: uneven combustion near idle.
        let lope = s.lope * roll * (0.6 + 0.8 * ch) * idle_n;
        if lope > 0.0 {
            amp *= 1.0 + lope * 1.6 * self.rng.bi();
            if self.rng.next() < lope * 0.25 * (0.3 + self.crackle_user) {
                amp *= 0.3;
            }
        }
        // Overrun: a steady, softer drone. overrunRough adds a little unevenness.
        if ov > 0.05 && s.overrun_rough > 0.0 {
            amp *= 1.0 + ov * s.overrun_rough * 1.4 * self.rng.bi();
            if self.rng.next() < ov * s.overrun_rough * 0.3 {
                amp *= 0.25;
            }
        }
        amp = amp.max(0.02);
        let mut n_amp = s.noise * (0.6 + 0.8 * ch) * (0.2 * (1.0 - 0.6 * ov) + 0.8 * lb * (1.0 - cut)) * amp;
        let mut w_ms = lerp(s.pulse.idle_ms, s.pulse.top_ms, rpm_n.powf(0.7)) * (1.0 + s.overrun_dark * 0.8 * ov);

        // Jake brake: on overrun the exhaust valves open at the top of each
        // compression stroke: a hard bark per cylinder.
        let jk = if s.jake.amt > 0.0 && rpm > self.idle * 1.3 { s.jake.amt * smoothstep((ov - 0.2) / 0.5) } else { 0.0 };
        if jk > 0.0 {
            amp = amp.max(jk * (0.55 + 0.45 * rpm_n));
            n_amp = n_amp.max(jk * 1.3 * amp);
            w_ms *= 1.0 - 0.45 * jk / s.jake.amt;
        }

        // Anti-lag: off throttle, fuel and spark kept going late: pops on most
        // firings and regular big bangs.
        if s.antilag > 0.0 && ov > 0.3 && rpm > self.idle * 1.4 {
            let al = s.antilag * self.crackle_user * 2.0;
            if self.rng.next() < al * 0.22 {
                let a = s.pop_amt * (0.7 + 0.6 * self.rng.next());
                self.pop(a);
            }
            if self.al_t > 0.18 + self.rng.next() * 0.5 && self.rng.next() < al * 0.05 {
                let a = 0.35 + 0.4 * self.rng.next();
                self.bang(a);
                self.al_t = 0.0;
            }
        }
        let w_samp = (w_ms * self.sr / 1000.0).max(1.5);
        let step = SHAPE_N as f64 / SHAPE_END / w_samp; // table index advance per sample

        for i in 0..MAX_PULSES {
            let p = &mut self.pulses[i];
            if p.on {
                continue;
            }
            *p = Pulse {
                on: true,
                bank: f.bank.min(MAX_BANKS - 1),
                amp,
                n_amp,
                step,
                wait: self.header[f.idx],
                age: self.fire_lag * step,
            };
            if i >= self.pulse_top {
                self.pulse_top = i + 1;
            }
            break;
        }
        self.mech_env = 1.0;

        // Afterfire: unburnt fuel lighting off in the hot exhaust on overrun or
        // during an ignition cut.
        let cr = s.crackle * self.crackle_user * 2.0;
        if cr > 0.0 && rpm > self.idle * 1.35 {
            let chance = cr * (0.5 + ch) * if cut > 0.5 { 0.045 } else { 0.012 * self.overrun };
            if self.rng.next() < chance {
                let a = s.pop_amt * (0.5 + 0.9 * self.rng.next() * self.rng.next());
                self.pop(a);
            }
        }
    }

    fn pop(&mut self, amp: f64) {
        let sr = self.sr;
        let has_pipes = self.n_pipes > 0;
        for i in 0..MAX_POPS {
            if self.pops[i].on {
                continue;
            }
            let wait = (3.0 + self.rng.next() * 30.0) * sr / 1000.0;
            let hz = 50.0 + self.rng.next() * 35.0;
            let body = 250.0 + self.rng.next() * 350.0;
            let pan = if has_pipes { if self.rng.next() < 0.5 { -0.5 } else { 0.5 } } else { 0.0 };
            self.pops[i] = Pop { on: true, age: 0.0, wait, amp, hz, body, n1: 0.0, n2: 0.0, pan };
            break;
        }
    }

    fn block(&mut self) {
        let s: &'static Sound = &self.car.sound;
        let sr = self.sr;
        let dt = BLOCK as f64 / sr;
        let (rpm, load, ch) = (self.rpm, self.load, self.character);
        let rpm_n = clamp((rpm - self.idle) / (self.redline - self.idle), 0.0, 1.0);

        // Muffler opens with load, revs and Character.
        let ov = self.overrun * (1.0 - load);
        let lb = load.max(BODY * self.body * self.overrun);
        let open = clamp(
            0.1 + 0.6 * lb + 0.15 * rpm_n + 0.35 * (ch - 0.5) - 0.35 * s.overrun_dark * ov * (1.0 - 0.5 * self.body),
            0.0,
            1.0,
        );
        let fc = s.muffler.closed_hz * (s.muffler.open_hz / s.muffler.closed_hz).powf(open);
        self.muf_l.lowpass(sr, fc, 0.6);
        self.muf_r.lowpass(sr, fc, 0.6);
        self.intake_bp.bandpass(sr, s.intake.hz * (0.75 + 0.5 * rpm_n), s.intake.q);

        // Turbo: a shaft with a turbine and a compressor on it.
        self.al_t += dt;
        self.air_t += dt;
        self.honk_t += dt;
        let t = &s.turbo;
        if t.amt > 0.0 {
            // Displacement sets how fast the turbo spools, not how much boost.
            let flow_ref = self.turbo_disp / self.turbo_flow_ref; // 1.0 at 3 L
            let mut energy = load * smoothstep(rpm / (t.spool_rpm * 0.6)) * (1.0 - self.cut * 0.5);
            if s.antilag > 0.0 && rpm > self.idle * 1.4 {
                energy = energy.max(s.antilag * 0.8 * self.overrun);
            }
            let flow = (energy * (0.7 + 0.6 * flow_ref)).min(1.0);

            // The shaft's inertia chases exhaust energy.
            let spool_tau = (t.spool_s * clamp(1.6 - 0.6 * flow_ref, 0.4, 1.6)).max(0.05);
            self.turbo_spool += (flow - self.turbo_spool) * clamp(dt / spool_tau, 0.0, 1.0);

            // The compressor map: no boost below a minimum shaft speed.
            let map_floor = 0.18;
            let boost_tgt = smoothstep((self.turbo_spool - map_floor) / (1.0 - map_floor));
            self.boost += (boost_tgt - self.boost) * clamp(dt / 0.22, 0.0, 1.0);

            // Lift-off: the throttle snaps shut with boost trapped ahead of it.
            self.peak_load = load.max(self.peak_load - dt * 0.3);
            self.peak_boost = self.boost.max(self.peak_boost - dt * 0.3);
            self.bov_t += dt;
            if load > 0.4 && self.boost > 0.3 {
                self.bov_armed = true;
            }
            if t.bov != Bov::None
                && self.peak_boost > 0.1
                && self.peak_load > 0.2
                && self.peak_load - load > 0.15
                && load < 0.12f64.max(self.peak_load * 0.75)
                && self.bov_t > 1.2
            {
                self.release(0.7); // strength is neutral; volume comes from trapped boost
                self.peak_load = load;
            }
            if self.bov_env > 1e-4 {
                let e = self.bov_env / t.bov_amt.max(1e-3);
                if t.bov == Bov::Flutter {
                    self.bov_bp.bandpass(sr, 650.0 + 700.0 * e, 1.6);
                } else if let Some(h) = t.bov_hz {
                    self.bov_bp.bandpass(sr, h[0] + (h[1] - h[0]) * e.min(1.0), or(t.bov_q, 1.1));
                } else {
                    self.bov_bp.bandpass(sr, 1700.0 + 2300.0 * e, 1.1);
                }
                self.bov_env *= (-dt / if t.bov == Bov::Flutter { 0.55 } else { or(t.bov_decay, 0.4) }).exp();
            }
        } else {
            self.boost = 0.0;
            self.turbo_spool = 0.0;
            self.bov_env = 0.0;
        }
    }

    /// Render `out_l.len()` frames (both slices must be the same length).
    /// Controls are re-read every [`BLOCK`] samples counted from the start of
    /// each call, as in the JS, so call it with a multiple of 32 frames
    /// (the game uses 128) for even timing.
    pub fn render(&mut self, out_l: &mut [f32], out_r: &mut [f32]) {
        let n = out_l.len().min(out_r.len());
        if let Some(sp) = self.special.as_mut() {
            sp.render(&mut out_l[..n], &mut out_r[..n], &self.target, &mut self.rng);
            return;
        }
        let pulse_tab = &shapes().pulse;
        let burst_tab = &shapes().burst;
        let s: &'static Sound = &self.car.sound;
        let sr = self.sr;
        let (n_pipes, n_banks) = (self.n_pipes, self.n_banks);
        let tg = self.target;
        let cross = clamp(s.cross, 0.0, 1.0);
        let (t, bl, gw) = (&s.turbo, &s.blower, &s.gear_whine);
        let drive = s.drive * (0.85 + 0.3 * self.character);
        let sat_norm = s.gain / drive.tanh();
        let (mix_e, mix_b, mix_p) = (self.mix_e, self.mix_b, self.mix_p);
        let k_pop = 1.0 - (-TAU * 5500.0 / sr).exp();
        let mech_k = (-1.0 / (0.0012 * sr)).exp();
        let click_k = (-1.0 / (0.004 * sr)).exp();
        let flutter = t.bov == Bov::Flutter;

        for i in 0..n {
            if i % BLOCK == 0 {
                self.overrun += (tg.overrun - self.overrun) * 0.08;
                self.body += (tg.body - self.body) * 0.08;
                self.starter += (tg.starter - self.starter) * 0.05;
                self.shaft += (tg.shaft - self.shaft) * 0.05;
                self.cut = tg.cut;
                self.block();
            }
            self.rpm += (tg.rpm - self.rpm) * self.k_rpm;
            self.load += (tg.load - self.load) * self.k_load;
            let (rpm, load) = (self.rpm, self.load);
            let rpm_n = clamp((rpm - self.idle) / (self.redline - self.idle), 0.0, 1.0);
            // With body on, engine braking keeps a light-cruise level.
            let lb = load.max(BODY * self.body * self.overrun);
            let ovq = self.overrun * (1.0 - load) * (1.0 - 0.7 * self.body);
            // Coasting: the bright layers (intake, valvetrain) back off with the exhaust.
            let quiet = 1.0 - 0.8 * s.overrun_dark * ovq;

            // Crank: fire every cylinder whose angle we passed this sample.
            let d_deg = rpm * 6.0 / sr;
            if d_deg > 0.0 {
                self.to_next -= d_deg;
                let mut guard = 0;
                while self.to_next <= 0.0 && guard < 8 {
                    guard += 1;
                    self.fire_lag = -self.to_next / d_deg;
                    self.fire(self.next);
                    self.to_next += self.gaps[self.next];
                    self.next = (self.next + 1) % self.n_cyl;
                }
            }

            // Exhaust pulses into their banks. Only slots below pulse_top can be on.
            for b in 0..n_banks {
                self.bank_in[b] = 0.0;
            }
            let (mut env, mut thump_in, mut hi_on) = (0.0f64, 0.0f64, 0usize);
            for k in 0..self.pulse_top {
                let p = &mut self.pulses[k];
                if !p.on {
                    continue;
                }
                hi_on = k + 1;
                if p.wait > 0.0 {
                    p.wait -= 1.0;
                    continue;
                }
                let idx = p.age as usize;
                if idx >= SHAPE_N {
                    p.on = false;
                    continue;
                }
                let sh = pulse_tab[idx] as f64;
                let nz = self.rng.bi();
                let bi = &mut self.bank_in[p.bank];
                *bi = (*bi as f64 + (p.amp * sh + p.n_amp * burst_tab[idx] as f64 * nz)) as f32;
                env += sh;
                thump_in += p.amp * sh;
                p.age += p.step;
            }
            self.pulse_top = hi_on;
            self.firing_env = env;

            // Turbine smoothing, then the crossover pipe.
            let mut mean = 0.0f64;
            for b in 0..n_banks {
                let tl = self.turb_lp[b] as f64;
                let v = (tl + self.turb_k * (self.bank_in[b] as f64 - tl)) as f32;
                self.turb_lp[b] = v;
                mean += v as f64;
            }
            mean /= n_banks as f64;

            let (mut l, mut r) = (0.0f64, 0.0f64);
            for k in 0..n_pipes {
                let c = self.pipe_cfg[k];
                let x = (1.0 - cross) * self.turb_lp[c.bank] as f64 + cross * mean;
                let y = self.pipes[k].process(x);
                l += y * c.gl;
                r += y * c.gr;
            }
            l *= self.pipe_norm;
            r *= self.pipe_norm;

            for k in 0..self.n_body {
                l = self.body_l[k].process(l);
                r = self.body_r[k].process(r);
            }
            // Muffler, then the bass shelf.
            l = self.muf_l.process(l);
            r = self.muf_r.process(r);
            l = self.bass_l.process(l);
            r = self.bass_r.process(r);
            if s.thump.amt > 0.0 {
                let mut th = thump_in;
                th = self.thump_bp.process(th);
                th = self.thump_bp2.process(th);
                // Gentle pull-away: the boom follows idle-level load, a bit quieter.
                let pa = tg.pull_away;
                let tl = lb - (lb - 0.15).max(0.0) * pa;
                let k = s.thump.amt * 6.0 * (0.45 + 0.55 * tl) * (1.0 - 0.4 * ovq) * (1.0 - 0.3 * pa);
                l += th * k;
                r += th * k;
                // Sub: cylinder-to-cylinder differences put energy at half and
                // full crank speed, pulsing with the firings.
                self.cam_ph += rpm / 120.0 / sr;
                if self.cam_ph > 1.0 {
                    self.cam_ph -= 1.0;
                }
                let a = self.cam_ph * TAU;
                // Near idle these slow beats are the "roll"; idleRoll tames them.
                let roll = 1.0 + (s.idle_roll - 1.0) * (1.0 - smoothstep((rpm - self.idle) / 1500.0));
                let sub = (0.7 * roll * a.sin() + (1.25 - 0.25 * roll) * (2.0 * a + 0.6).sin()) * (0.55 + 0.45 * env.min(1.5));
                let ks = s.thump.amt * s.thump.sub * 0.45 * (0.35 + 0.65 * tl) * (1.0 - 0.5 * ovq) * (1.0 - 0.3 * pa)
                    * (1.0 - 0.5 * smoothstep((rpm - 5000.0) / 5000.0));
                l += sub * ks;
                r += sub * ks;
            }

            // Everything below bypasses the muffler. C goes through the
            // saturator with the engine; T is laid on top of it, clean.
            let (mut c, mut tt_top) = (0.0f64, 0.0f64);

            if s.intake.amt > 0.0 {
                let x = self.rng.bi();
                let nz = self.intake_bp.process(x);
                c += nz * s.intake.amt * quiet * (0.12 + 0.88 * lb) * (0.3 + 0.7 * rpm_n) * (0.4 + env.min(2.0));
            }
            if s.mech > 0.0 {
                self.mech_env *= mech_k;
                let x = self.rng.bi();
                let y = self.mech_bp.process(x);
                c += y * s.mech * quiet * self.mech_env * (0.5 + 0.5 * rpm_n);
            }
            if t.amt > 0.0 {
                let b = self.boost;
                if b > 1e-3 || t.whistle_idle > 0.0 {
                    let (w_hz, mut w_amp);
                    if t.whistle_fade != 0.0 {
                        // Compressor blade-pass: pitch follows the shaft,
                        // sweeping up as it spools; loudest low in the sweep.
                        let sp = self.turbo_spool;
                        let u = (sp * sp * sp * sp).max(self.whistle_u - 0.3 / sr);
                        self.whistle_u = u;
                        w_hz = t.whistle_hz[0] * (t.whistle_hz[1] / t.whistle_hz[0]).powf(u);
                        w_amp = smoothstep(sp / 0.5) * smoothstep((b - 0.45) / 0.45) * (1.0 - t.whistle_fade * u);
                        // A faint whine even at idle (and as the shaft spins down).
                        if t.whistle_idle > 0.0 {
                            w_amp = w_amp.max(t.whistle_idle * (1.0 - t.whistle_fade * u));
                        }
                        // Each exhaust pulse kicks the turbine: near idle the
                        // whine pulses with the firing.
                        if t.whistle_pulse > 0.0 {
                            self.w_pf += self.w_pf_k * (env - self.w_pf);
                            self.w_ps += self.w_ps_k * (self.w_pf - self.w_ps);
                            let d = t.whistle_pulse * (1.0 - smoothstep((rpm - self.idle) / 2500.0));
                            w_amp *= 1.0 - d + d * 2.5f64.min(self.w_pf / (self.w_ps + 1e-6));
                        }
                    } else {
                        w_hz = lerp(t.whistle_hz[0], t.whistle_hz[1], b);
                        w_amp = b * b;
                    }
                    self.whistle_ph += w_hz / sr;
                    if self.whistle_ph > 1.0 {
                        self.whistle_ph -= 1.0;
                    }
                    let ph = self.whistle_ph * TAU;
                    let mut wv = ph.sin() + 0.25 * (2.0 * ph).sin();
                    // Twin turbos never spin quite together.
                    if t.whistle_twin > 0.0 {
                        self.whistle_ph2 += w_hz * t.whistle_twin / sr;
                        if self.whistle_ph2 > 1.0 {
                            self.whistle_ph2 -= 1.0;
                        }
                        wv += 0.6 * (self.whistle_ph2 * TAU).sin();
                    }
                    tt_top += 0.6 * t.whistle * t.amt * w_amp * wv * mix_b;
                    let x = self.rng.bi();
                    let y = self.hiss_bp.process(x);
                    // With a tone filter the turbo's rush sits under it with the engine.
                    if self.tone_on {
                        c += 0.7 * y * t.hiss * t.amt * b * mix_b;
                    } else {
                        tt_top += 0.7 * y * t.hiss * t.amt * b * mix_b;
                    }
                    // Rush: on hard, high-boost pulls, bright air on top of everything.
                    if t.rush > 0.0 {
                        let k = smoothstep((b - 0.5) / 0.4) * smoothstep((load - 0.4) / 0.4);
                        if k > 0.0 {
                            let xr = self.rng.bi();
                            let yr = self.rush_bp.process(xr);
                            tt_top += yr * t.rush * t.amt * k * mix_b;
                        }
                    }
                }
                if self.bov_env > 1e-4 {
                    let mut e = self.bov_env * (self.bov_t / or(t.bov_attack, 1.0 / 120.0)).min(1.0);
                    if flutter {
                        let r = self.bov_env / t.bov_amt.max(1e-3);
                        self.bov_phase += (11.0 + 17.0 * r) / sr;
                        let m = 0.5 + 0.5 * (self.bov_phase * TAU).sin();
                        e *= m * m * m * 1.8;
                    }
                    let mut x = self.rng.bi();
                    // A tuned valve (bovHz) sounds fuller: darker air, more body.
                    if t.bov_hz.is_some() {
                        self.bov_lp += self.bov_lp_k * (x - self.bov_lp);
                        x = self.bov_lp * 2.0;
                    }
                    let y = self.bov_bp.process(x);
                    let lev = if flutter { 3.2 } else { 1.8 };
                    tt_top += y * e * lev * mix_b;
                }
            }
            if bl.amt > 0.0 {
                let f = rpm / 60.0 * bl.ratio;
                self.blow_ph += f / sr;
                if self.blow_ph > 1.0 {
                    self.blow_ph -= 1.0;
                }
                self.blow_ph2 += f * 1.0031 / sr;
                if self.blow_ph2 > 1.0 {
                    self.blow_ph2 -= 1.0;
                }
                let (a, a2) = (self.blow_ph * TAU, self.blow_ph2 * TAU);
                let tone = a.sin() + 0.85 * (2.0 * a).sin() + 0.4 * (3.0 * a).sin() + 0.25 * (4.0 * a).sin()
                    + 0.6 * (a2.sin() + 0.7 * (2.0 * a2).sin());
                // Belt-driven: always spinning, louder under boost, quieter on overrun.
                tt_top += tone * bl.amt * (0.25 * quiet + 0.75 * smoothstep(load * 1.5)) * (0.25 + 0.75 * rpm_n) * 0.5 * mix_b;
            }
            if gw.amt > 0.0 && self.shaft > 50.0 {
                self.gear_ph += self.shaft / 60.0 * gw.ratio / sr;
                if self.gear_ph > 1.0 {
                    self.gear_ph -= 1.0;
                }
                tt_top += (self.gear_ph * TAU).sin() * gw.amt * clamp(self.shaft / 2500.0, 0.0, 1.0) * (0.5 + 0.5 * load);
            }
            if self.starter > 1e-3 || self.starter_rpm > 5.0 {
                // The motor follows the crank while engaged, then spins down.
                if self.starter > 0.5 {
                    self.starter_rpm += (rpm - self.starter_rpm) * 0.01;
                } else {
                    self.starter_rpm *= 0.9996;
                }
                let st = s.starter;
                let motor = self.starter_rpm * 13.0 / 60.0 * st.pitch; // armature rev/s
                // A real starter is a rasp: pinion gear mesh (sawtooth) and the
                // commutator/brush whine (square-ish).
                self.starter_ph += motor * 9.0 / sr;
                if self.starter_ph > 2.0 {
                    self.starter_ph -= 2.0;
                }
                self.starter_ph2 += motor * 24.0 / sr;
                if self.starter_ph2 > 1.0 {
                    self.starter_ph2 -= 1.0;
                }
                let saw = self.starter_ph - 1.0; // -1..1 mesh grind
                let a2 = self.starter_ph2 * TAU;
                let lev = st.amt * self.starter.max(self.starter_rpm / 250.0 * 0.5);
                let comm = sign(a2.sin()) * 0.6 + 0.4 * (a2 * 2.0).sin();
                let x = self.rng.bi();
                let y = self.starter_bp.process(x);
                let grit = y * (0.4 + 0.6 * (saw * 0.5 + 0.5));
                c += lev * (saw * 0.04 + comm * 0.02 + grit * 0.03);
            }
            if self.click_env > 1e-4 {
                self.click_t += 1.0 / sr;
                self.thump_ph += 140.0 / sr;
                let x = self.rng.bi();
                let y = self.click_bp.process(x);
                c += self.click_env * (y * 0.5 + (self.thump_ph * TAU).sin() * 0.3 * (-self.click_t / 0.02).exp());
                self.click_env *= click_k;
            }

            // Air brakes: a sharp "pssht" with a long hiss tail.
            if self.air_env > 1e-4 {
                let e = self.air_env * ((self.air_t * 60.0).min(1.0) * (0.35 + 0.65 * (-self.air_t / 0.12).exp()));
                let x = self.rng.bi();
                tt_top += self.air_bp.process(x) * e * 0.5;
                self.air_env *= (-1.0 / (0.9 * sr)).exp();
            }
            // Cartoon: a kazoo riding on the engine, its pitch on a spring that
            // overshoots every rev change; and the a-wooga horn.
            if s.toon > 0.0 {
                let dr = (rpm - self.last_rpm) * sr; // rpm per second
                self.last_rpm = rpm;
                let w = TAU * 5.5;
                self.spring_v += (-w * w * self.spring - 2.0 * 0.12 * w * self.spring_v + dr * 0.004) / sr;
                self.spring += self.spring_v / sr;
                let f = (70.0 + rpm * 0.045) * (0.35 * clamp(self.spring, -2.0, 2.0)).exp();
                self.kaz_ph += f / sr;
                if self.kaz_ph > 1.0 {
                    self.kaz_ph -= 1.0;
                }
                if (i & 31) == 0 {
                    self.kaz_f1.bandpass(sr, lerp(450.0, 750.0, load), 4.0);
                    self.kaz_f2.bandpass(sr, lerp(1000.0, 1400.0, load), 5.0);
                }
                let saw = 2.0 * self.kaz_ph - 1.0 + 0.15 * self.rng.bi();
                let kz = self.kaz_f1.process(saw) * 1.2 + self.kaz_f2.process(saw) * 0.8;
                c += kz * s.toon * 0.18 * (0.35 + 0.65 * load) * (0.5 + 0.5 * env.min(1.5));
                if self.honk_t < 0.55 {
                    let ht = self.honk_t;
                    let hf = if ht < 0.25 { lerp(260.0, 420.0, ht / 0.25) } else { lerp(420.0, 300.0, (ht - 0.25) / 0.3) };
                    self.honk_ph += hf / sr;
                    if self.honk_ph > 1.0 {
                        self.honk_ph -= 1.0;
                    }
                    let sq = if self.honk_ph < 0.5 { 1.0 } else { -1.0 };
                    tt_top += self.honk_bp.process(sq) * s.toon * s.bang * 0.5 * (ht * 40.0).min(1.0) * ((0.55 - ht) * 12.0).min(1.0);
                }
            }

            // Afterfire pops: a sharp crack plus a short low thump.
            let (mut pop_l, mut pop_r) = (0.0f64, 0.0f64);
            for k in 0..MAX_POPS {
                if !self.pops[k].on {
                    continue;
                }
                if self.pops[k].wait > 0.0 {
                    self.pops[k].wait -= 1.0;
                    continue;
                }
                let tt = self.pops[k].age / sr;
                if tt > 0.09 {
                    self.pops[k].on = false;
                    continue;
                }
                let nz = self.rng.bi();
                let q = &mut self.pops[k];
                q.n1 += 0.2 * (nz - q.n1);
                q.n2 += 0.2 * (q.n1 - q.n2);
                let v = q.amp
                    * (q.n2 * 4.0 * (-tt / 0.005).exp()
                        + 0.8 * (TAU * q.body * tt).sin() * (-tt / 0.01).exp()
                        + 1.3 * (TAU * q.hz * tt).sin() * (-tt / 0.03).exp());
                pop_l += v * (0.5 - q.pan * 0.5);
                pop_r += v * (0.5 + q.pan * 0.5);
                q.age += 1.0;
            }
            self.pop_lp += k_pop * ((pop_l + pop_r) - self.pop_lp);
            let pl = (self.pop_lp * 0.5 + (pop_l - pop_r) * 0.2) * mix_p;
            let pr = (self.pop_lp * 0.5 - (pop_l - pop_r) * 0.2) * mix_p;

            l += c + pl;
            r += c + pr;

            // DC block, then saturate.
            let y = l - self.dc_l.x + 0.9995 * self.dc_l.y;
            self.dc_l.x = l;
            self.dc_l.y = y;
            l = y;
            let y = r - self.dc_r.x + 0.9995 * self.dc_r.y;
            self.dc_r.x = r;
            self.dc_r.y = y;
            r = y;
            let mut bsum = 0.0f64;
            let mut k = 0;
            while k < self.n_bangs {
                if self.bangs[k].wait > 0.0 {
                    self.bangs[k].wait -= 1.0;
                    k += 1;
                    continue;
                }
                let n1 = self.rng.bi();
                let n2 = self.rng.bi();
                let g = &mut self.bangs[k];
                let tt = g.t / sr;
                // Boom: starts near 70 Hz and sweeps down to about 22 Hz, with
                // a 30 Hz sub underneath and a low rumble; a small crack on top.
                // A lone bang rings longer; bangs in a burst are short and punchy.
                let len = if g.single { 1.0 } else { 0.4 };
                g.ph += (22.0 + 48.0 * (-tt / 0.04).exp()) / sr;
                g.sub += 30.0 / sr;
                g.lp += 0.004 * (n1 - g.lp);
                g.lp2 += 0.004 * (g.lp - g.lp2);
                // Pop, not click: a rounded 2 ms pressure push, a dark puff of
                // air and a short hollow tone around 320 Hz.
                g.bp += 0.18 * (n2 - g.bp);
                g.bp2 += 0.18 * (g.bp - g.bp2);
                let push = if tt < 0.002 { (PI * tt / 0.002).sin() } else { 0.0 };
                let cm = or(g.crack_mul, 1.0);
                let crack = (g.bp2 * 6.0 * (-tt / 0.003).exp() + push * 1.3 + (TAU * 320.0 * tt).sin() * (-tt / 0.008).exp() * 0.9) * cm;
                let att = (tt / 0.002).min(1.0);
                bsum += g.amp
                    * (crack * s.bang_crack * 1.6
                        + att * (g.ph * TAU).sin() * (-tt / (0.16 * len)).exp() * 2.2
                        + att * (g.sub * TAU).sin() * (-tt / (0.22 * len)).exp() * 1.4
                        + g.lp2 * (-tt / (0.25 * len)).exp() * 40.0);
                g.t += 1.0;
                if tt > 0.8 {
                    // Done: close the gap, park the slot at the end.
                    self.bangs[k..self.n_bangs].rotate_left(1);
                    self.n_bangs -= 1;
                } else {
                    k += 1;
                }
            }
            // Flames: dark roaring noise swelling in about 25 ms and burning
            // out over a third of a second, a 55 Hz whoomp, sparse crackles.
            let mut k = 0;
            while k < self.n_flames {
                if self.flames[k].wait > 0.0 {
                    self.flames[k].wait -= 1.0;
                    k += 1;
                    continue;
                }
                let tt = self.flames[k].t / sr;
                let nz = self.rng.bi();
                let mut cr = 0.0;
                if tt > 0.04 && tt < 0.35 && self.rng.next() < 0.004 {
                    cr = self.rng.bi() * 1.5;
                }
                let f = &mut self.flames[k];
                f.lp += 0.05 * (nz - f.lp);
                f.lp2 += 0.08 * (f.lp - f.lp2);
                f.ph += 55.0 / sr;
                let fenv = (tt / 0.025).min(1.0) * (-tt / 0.14).exp();
                bsum += f.amp * (fenv * (f.lp2 * 26.0 + (f.ph * TAU).sin() * 0.9) + cr * (-tt / 0.2).exp());
                f.t += 1.0;
                if tt > 0.6 {
                    self.flames[k..self.n_flames].rotate_left(1);
                    self.n_flames -= 1;
                } else {
                    k += 1;
                }
            }
            let top = tt_top * s.gain + (bsum * 1.4).tanh() * 0.6 * mix_p;
            if self.tone_on {
                l = (drive * l * 0.35).tanh() * sat_norm * mix_e;
                r = (drive * r * 0.35).tanh() * sat_norm * mix_e;
                out_l[i] = (self.tone_l.process(l) + top) as f32;
                out_r[i] = (self.tone_r.process(r) + top) as f32;
            } else {
                out_l[i] = ((drive * l * 0.35).tanh() * sat_norm * mix_e + top) as f32;
                out_r[i] = ((drive * r * 0.35).tanh() * sat_norm * mix_e + top) as f32;
            }
        }
    }
}
