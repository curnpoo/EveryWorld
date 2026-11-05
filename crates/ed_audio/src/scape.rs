//! The audio-thread soundscape: continuous beds driven by smoothed `Params`,
//! a pool of one-shot `Part`s spawned by `Event`s and by the beds' own
//! schedulers (birds, bubbles, trains...), and the mixing rules from
//! `docs/plans/sound.md`:
//!
//! - **Body** bus (footsteps, servos, rain pinging the shell, re-entry
//!   rumble): carried by the robot itself, never gated. In space it is all
//!   you hear.
//! - **Air** bus (ear wind, boots, tyres, nearby one-shots): fades with air density.
//! - **Amb** bus (world beds): fades early with altitude, ducked up to -12 dB by
//!   speed wind, muffled by snow and fog. It also gets the street-canyon slap-back.
//! - **Ui** bus: altimeter, ground-proximity warning, mode chirps.
//!
//! Params are smoothed once per `BLOCK` and filter coefficients are set
//! per block, so the per-sample cost is a few dozen SVF ticks.

use crossbeam_channel::{Receiver, Sender, bounded};

use crate::dsp::{Brown, Delay, Mode, OnePole, Rng, SR, Svf, TAU, pan, sin_t, smoothstep as ss};
use crate::params::{Event, Params, blast, dolbear_cpm, fauna, rain_on, tyre, veg};
use crate::parts::{Bus, Part};
use crate::recipes;

pub const BLOCK: usize = 64;
const MAX_PARTS: usize = 256;
const BLOCK_S: f32 = BLOCK as f32 / SR;

pub enum Msg {
    Params(Params),
    Event(Event),
}

/// Game-side handle. Cheap to clone; never blocks (drops messages if the
/// audio thread is not draining).
#[derive(Clone)]
pub struct ScapeHandle {
    tx: Sender<Msg>,
}

impl ScapeHandle {
    pub fn set(&self, p: Params) {
        let _ = self.tx.try_send(Msg::Params(p));
    }
    pub fn event(&self, e: Event) {
        let _ = self.tx.try_send(Msg::Event(e));
    }
}

pub fn channel() -> (ScapeHandle, Scape) {
    let (tx, rx) = bounded(1024);
    (ScapeHandle { tx }, Scape::new(rx))
}

// ---------------------------------------------------------------- building blocks

/// Filtered noise (white or brown) with its own generator.
#[derive(Clone, Copy)]
struct Band {
    svf: Svf,
    rng: Rng,
    mode: Mode,
    brown: Option<Brown>,
}

impl Band {
    #[inline]
    fn tick(&mut self) -> f32 {
        let mut x = self.rng.bi();
        if let Some(b) = &mut self.brown {
            x = b.tick(x);
        }
        self.svf.run(x, self.mode)
    }
    #[inline]
    fn run(&mut self, x: f32) -> f32 {
        self.svf.run(x, self.mode)
    }
    fn set(&mut self, fc: f32, q: f32) {
        self.svf.set(fc, q);
    }
}

/// A slow random wobble with roughly unit RMS (low-passed white).
#[derive(Clone, Copy)]
struct Wander {
    y: f32,
    a: f32,
    norm: f32,
    rng: Rng,
}

impl Wander {
    #[inline]
    fn tick(&mut self) -> f32 {
        self.y += self.a * (self.rng.bi() - self.y);
        self.y * self.norm
    }
}

/// Seed sequence so every generator is decorrelated but deterministic.
struct Seq(u32);

impl Seq {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_add(0x9e37_79b9);
        self.0
    }
    fn band(&mut self, mode: Mode, fc: f32, q: f32) -> Band {
        Band { svf: Svf::new(fc, q), rng: Rng::new(self.next()), mode, brown: None }
    }
    fn brown(&mut self, lp: f32) -> Band {
        Band { brown: Some(Brown::default()), ..self.band(Mode::Lp, lp, 0.7) }
    }
    fn pair(&mut self, mode: Mode, fc: f32, q: f32) -> [Band; 2] {
        [self.band(mode, fc, q), self.band(mode, fc, q)]
    }
    /// `rate` = how often `tick` is called (SR, or 1/BLOCK_S for block-rate wanders).
    fn wander(&mut self, fc: f32, rate: f32) -> Wander {
        let a = (1.0 - (-TAU * fc / rate).exp()).max(1e-6);
        Wander { y: 0.0, a, norm: (3.0 * (2.0 - a) / a).sqrt(), rng: Rng::new(self.next()) }
    }
}

#[inline]
fn osc(ph: &mut f32, f: f32) -> f32 {
    let s = sin_t(*ph);
    *ph = (*ph + f / SR).fract();
    s
}

#[inline]
fn saw(ph: &mut f32, f: f32) -> f32 {
    let s = *ph * 2.0 - 1.0;
    *ph = (*ph + f / SR).fract();
    s
}

/// Soft clip that is transparent below about -12 dBFS.
#[inline]
fn soft(x: f32) -> f32 {
    let x = x.clamp(-3.0, 3.0);
    (x * (27.0 + x * x) / (27.0 + 9.0 * x * x)).clamp(-1.0, 1.0)
}

// ---------------------------------------------------------------- vegetation table

/// Wind through one class of plant (`docs/plans/sound.md` §5).
#[derive(Clone, Copy)]
struct VegRecipe {
    fc: f32,
    q: f32,
    /// Leaf flicker rate (Hz) and depth: birch shimmer is fast and deep, pine is smooth.
    flicker_hz: f32,
    depth: f32,
    /// Wind (m/s) where it starts to sound.
    wind_lo: f32,
    level: f32,
    /// Knocks per second at full veg weight and strong wind (palm fronds, banana slaps, reed stems).
    clack_rate: f32,
    clack_fc: f32,
    clack_q: f32,
}

const fn v(fc: f32, q: f32, flicker_hz: f32, depth: f32, wind_lo: f32, level: f32, clack_rate: f32, clack_fc: f32, clack_q: f32) -> VegRecipe {
    VegRecipe { fc, q, flicker_hz, depth, wind_lo, level, clack_rate, clack_fc, clack_q }
}

const VEG: [VegRecipe; veg::N] = [
    v(4_200.0, 0.6, 0.5, 0.3, 2.0, 0.10, 0.0, 0.0, 0.0),  // conifer: smooth high whisper
    v(1_800.0, 0.7, 1.5, 0.4, 3.0, 0.07, 0.0, 0.0, 0.0),  // cypress: dense, darker
    v(2_400.0, 0.8, 14.0, 0.7, 1.5, 0.09, 0.0, 0.0, 0.0), // broadleaf: leafy rustle
    v(4_800.0, 1.0, 28.0, 0.8, 1.0, 0.07, 0.0, 0.0, 0.0), // birch/aspen: fast shimmer
    v(5_500.0, 0.9, 6.0, 0.5, 2.0, 0.06, 0.0, 0.0, 0.0),  // olive: dry silvery hiss
    v(1_300.0, 0.8, 3.0, 0.6, 3.0, 0.08, 3.0, 1_100.0, 4.0), // palm: fronds clatter
    v(900.0, 0.7, 2.0, 0.7, 2.0, 0.06, 2.0, 600.0, 1.5),  // banana: big leaf slaps
    v(700.0, 0.6, 1.0, 0.4, 4.0, 0.04, 0.0, 0.0, 0.0),    // sparse/dead: low moan
    v(2_400.0, 20.0, 0.3, 0.5, 10.0, 0.05, 0.0, 0.0, 0.0), // cactus: whistles on gusts
    v(2_000.0, 0.8, 8.0, 0.6, 2.0, 0.05, 0.0, 0.0, 0.0),  // shrub
    v(3_200.0, 0.6, 0.15, 0.6, 2.0, 0.06, 0.0, 0.0, 0.0), // grass: slow waves
    v(800.0, 3.0, 4.0, 0.6, 2.0, 0.06, 4.0, 2_200.0, 3.0), // reeds: hollow, stems rattle
];

const BIRD_RATE: [f32; fauna::N] = [0.0, 0.25, 0.0, 0.0, 0.7, 0.05, 0.1, 0.2, 0.05, 0.02];

// ---------------------------------------------------------------- beds

/// Per-block gains for the beds (so the sample loop only multiplies).
#[derive(Default, Clone, Copy)]
struct Lv {
    ear: f32,
    ear_pan: (f32, f32),
    rough: f32,
    howl: f32,
    whistle: f32,
    flutter: f32,
    flutter_hz: f32,
    suit: f32,
    suit_f: f32,
    groan: f32,
    canopy: f32,
    boot: [f32; 2],
    boot_f: [f32; 2],
    crack_rate: [f32; 2],
    ab: f32,
    thrum: [f32; 2],
    blast: [f32; blast::N],
    blast_rumble: f32,
    charge: f32,
    charge_f: f32,
    reentry: f32,
    servo: f32,
    servo_f: f32,
    space: f32,
    wind: f32,
    wind_whistle: f32,
    veg: [f32; veg::N],
    surf: f32,
    lake: f32,
    river: f32,
    rapids: f32,
    fall: f32,
    rain_hiss: f32,
    leaf_rate: f32,
    leaf: f32,
    pave_rate: f32,
    pave: f32,
    roof_rate: f32,
    roof: f32,
    drizzle: f32,
    hail_rate: f32,
    drone: f32,
    cicada: f32,
    traffic_far: f32,
    traffic_near: f32,
    crowd: f32,
    industry: f32,
    hum: f32,
    hum_f: f32,
    sodium: f32,
    tyre: f32,
    whine: f32,
    whine_f: f32,
    gravel_rate: f32,
    gravel: f32,
    dirt: f32,
    skid: f32,
    stall: f32,
    cabin: f32,
    alt_tone: f32,
    gpw: f32,
    gpw_rate: f32,
}

#[derive(Default, Clone, Copy)]
struct Out {
    body: (f32, f32),
    air: (f32, f32),
    amb: (f32, f32),
    ui: (f32, f32),
}

impl Out {
    #[inline]
    fn add(&mut self, bus: Bus, (l, r): (f32, f32)) {
        let b = match bus {
            Bus::Body => &mut self.body,
            Bus::Air => &mut self.air,
            Bus::Amb => &mut self.amb,
            Bus::Ui => &mut self.ui,
        };
        b.0 += l;
        b.1 += r;
    }
}

struct Beds {
    rng: Rng,
    // air & body
    ear: [Band; 2],
    howl: Band,
    whistle: Band,
    buffet: Wander,
    roll: f32,
    flutter: Band,
    flutter_ph: f32,
    suit_ph: f32,
    suit_lp: OnePole,
    suit_buzz: Band,
    groan_ph: f32,
    canopy: Band,
    hiss: [Band; 2],
    rumble: [Band; 2],
    crack: [Band; 2],
    crack_env: [f32; 2],
    ab: Band,
    thrum_ph: [f32; 2],
    blast: [Band; blast::N],
    blast_rumble: Band,
    blast_w: Wander,
    charge_ph: f32,
    reentry: Band,
    reentry_hi: Band,
    reentry_w: Wander,
    servo_ph: f32,
    servo_lp: OnePole,
    space_ph: f32,
    // world
    gust: Wander,
    gust_now: f32,
    wind: [Band; 2],
    wind_whistle: Band,
    veg: [[Band; 2]; veg::N],
    veg_w: [Wander; veg::N],
    grass_ph: f32,
    surf_ph: f32,
    surf_period: f32,
    surf_env: f32,
    surf: [Band; 2],
    lake: Band,
    lake_w: Wander,
    river: [Band; 2],
    rapids: Band,
    fall: [Band; 2],
    fall_lo: Band,
    rain: [Band; 2],
    leaf: [Svf; 2],
    roof: Svf,
    pave: Band,
    drizzle: Band,
    hail: Band,
    drone: [Band; 3],
    drone_w: Wander,
    cicada: Band,
    cicada_ph: f32,
    cicada_swell: f32,
    crickets: [f32; 4],
    traffic: [Band; 2],
    near: Band,
    near_w: Wander,
    crowd: [Band; 4],
    crowd_w: [Wander; 4],
    ind_ph: f32,
    ind: Band,
    hum_ph: f32,
    // vehicles & UI
    tyre: Band,
    whine_ph: f32,
    gravel: Band,
    dirt: Band,
    dirt_w: Wander,
    skid_ph: f32,
    skid_w: Wander,
    skid_lp: OnePole,
    stall_ph: f32,
    stall_lp: OnePole,
    cabin: Band,
    alt_t: f32,
    alt_ph: f32,
    gpw_ph: f32,
    gpw_gate: f32,
}

impl Beds {
    fn new() -> Beds {
        let mut s = Seq(0x5eed_a0d1);
        let bl = 1.0 / BLOCK_S;
        Beds {
            rng: Rng::new(s.next()),
            ear: s.pair(Mode::Lp, 800.0, 0.6),
            howl: s.band(Mode::Bp, 420.0, 4.0),
            whistle: s.band(Mode::Bp, 3_200.0, 8.0),
            buffet: s.wander(18.0, SR),
            roll: 0.0,
            flutter: s.band(Mode::Bp, 250.0, 1.2),
            flutter_ph: 0.0,
            suit_ph: 0.0,
            suit_lp: OnePole::default(),
            suit_buzz: s.band(Mode::Bp, 600.0, 2.5),
            groan_ph: 0.0,
            canopy: s.band(Mode::Bp, 600.0, 0.8),
            hiss: s.pair(Mode::Bp, 1_400.0, 1.0),
            rumble: [s.brown(120.0), s.brown(120.0)],
            crack: s.pair(Mode::Hp, 2_500.0, 0.7),
            crack_env: [0.0; 2],
            ab: s.brown(55.0),
            thrum_ph: [0.0; 2],
            blast: [
                s.band(Mode::Lp, 600.0, 0.7),
                s.band(Mode::Bp, 3_500.0, 0.8),
                s.band(Mode::Hp, 3_000.0, 0.7),
                s.band(Mode::Bp, 5_500.0, 0.5),
                s.band(Mode::Bp, 2_200.0, 0.9),
            ],
            blast_rumble: s.brown(250.0),
            blast_w: s.wander(8.0, SR),
            charge_ph: 0.0,
            reentry: s.brown(180.0),
            reentry_hi: s.band(Mode::Bp, 900.0, 0.7),
            reentry_w: s.wander(12.0, SR),
            servo_ph: 0.0,
            servo_lp: OnePole::default(),
            space_ph: 0.0,
            gust: s.wander(0.08, bl),
            gust_now: 0.0,
            wind: s.pair(Mode::Bp, 400.0, 0.5),
            wind_whistle: s.band(Mode::Bp, 1_800.0, 12.0),
            veg: std::array::from_fn(|i| s.pair(Mode::Bp, VEG[i].fc, VEG[i].q)),
            veg_w: std::array::from_fn(|i| s.wander(VEG[i].flicker_hz, SR)),
            grass_ph: 0.0,
            surf_ph: 0.3,
            surf_period: 10.0,
            surf_env: 0.3,
            surf: [s.band(Mode::Lp, 900.0, 0.6), s.band(Mode::Bp, 2_500.0, 0.5)],
            lake: s.band(Mode::Bp, 500.0, 1.5),
            lake_w: s.wander(0.8, SR),
            river: [s.band(Mode::Lp, 700.0, 0.6), s.band(Mode::Bp, 1_800.0, 0.6)],
            rapids: s.band(Mode::Hp, 2_500.0, 0.7),
            fall: s.pair(Mode::Bp, 1_200.0, 0.5),
            fall_lo: s.brown(300.0),
            rain: s.pair(Mode::Bp, 5_500.0, 0.5),
            leaf: [Svf::new(3_000.0, 2.0), Svf::new(3_300.0, 2.0)],
            roof: Svf::new(450.0, 6.0),
            pave: s.band(Mode::Hp, 5_000.0, 0.7),
            drizzle: s.band(Mode::Bp, 7_000.0, 0.5),
            hail: s.band(Mode::Hp, 3_000.0, 0.7),
            drone: [s.band(Mode::Bp, 3_200.0, 10.0), s.band(Mode::Bp, 4_600.0, 10.0), s.band(Mode::Bp, 6_100.0, 10.0)],
            drone_w: s.wander(0.3, SR),
            cicada: s.band(Mode::Bp, 5_800.0, 4.0),
            cicada_ph: 0.0,
            cicada_swell: 0.0,
            crickets: [0.0, 0.27, 0.55, 0.8],
            traffic: [s.brown(500.0), s.band(Mode::Bp, 900.0, 0.5)],
            near: s.band(Mode::Bp, 700.0, 0.6),
            near_w: s.wander(0.25, SR),
            crowd: [s.band(Mode::Bp, 300.0, 3.0), s.band(Mode::Bp, 550.0, 3.0), s.band(Mode::Bp, 900.0, 3.0), s.band(Mode::Bp, 1_500.0, 3.0)],
            crowd_w: std::array::from_fn(|_| s.wander(4.0, SR)),
            ind_ph: 0.0,
            ind: s.band(Mode::Bp, 2_500.0, 0.7),
            hum_ph: 0.0,
            tyre: s.band(Mode::Bp, 600.0, 0.8),
            whine_ph: 0.0,
            gravel: s.band(Mode::Bp, 2_500.0, 1.0),
            dirt: s.brown(200.0),
            dirt_w: s.wander(6.0, SR),
            skid_ph: 0.0,
            skid_w: s.wander(7.0, SR),
            skid_lp: OnePole::default(),
            stall_ph: 0.0,
            stall_lp: OnePole::default(),
            cabin: s.band(Mode::Lp, 700.0, 0.6),
            alt_t: 0.95,
            alt_ph: 0.0,
            gpw_ph: 0.0,
            gpw_gate: 0.0,
        }
    }

    /// Per block: set filter coefficients, compute gains, schedule one-shots.
    fn prepare(&mut self, p: &Params, rng: &mut Rng, spawn: &mut Vec<Part>) -> Lv {
        let mut l = Lv::default();
        let air = p.air_gate();
        let v = p.airspeed();
        let hit = |rng: &mut Rng, rate: f32| rate > 0.0 && rng.f() < rate * BLOCK_S;

        // ---- ear wind: follows q (½ρv²), not speed, so thin air at altitude is quiet.
        let q = p.q.max(0.0);
        let supersonic = 1.0 - 0.8 * ss(1.0, 1.25, p.mach);
        // Log curve: audible from ~10 m/s, about -10 dB vs the old sqrt at freefall
        // speed (q ~1.5 kPa) and it stops growing past ~3x that.
        l.ear = (0.045 * (1.0 + q / 250.0).ln()).min(0.11) * supersonic;
        if l.ear > 1e-5 {
            let fc = (180.0 + 2_600.0 * ss(40.0, 900.0, q) + p.tuck * 1_500.0 - p.spread * 900.0).max(120.0);
            for b in &mut self.ear {
                b.set(fc, 0.6);
            }
            self.howl.set(420.0 * (1.0 + 0.3 * ss(1_200.0, 4_000.0, q)), 4.0);
            self.whistle.set(2_800.0 + 1_200.0 * ss(500.0, 3_000.0, q), 8.0);
            let transonic = ss(0.85, 0.98, p.mach) * (1.0 - ss(1.0, 1.1, p.mach));
            l.rough = (p.spread * 0.6 + p.flow_side.abs() * 0.5 + 0.8 * transonic).min(1.0);
            l.howl = 0.15 * ss(1_200.0, 2_600.0, q) * l.ear;
            l.whistle = 0.08 * p.tuck * ss(200.0, 1_500.0, q) * l.ear;
            self.roll = (self.roll + p.roll_rate * BLOCK_S) % TAU;
            let roll_amt = (p.roll_rate.abs() / 2.0).min(1.0);
            l.ear_pan = pan((p.flow_side * 0.6 + 0.5 * self.roll.sin() * roll_amt).clamp(-0.9, 0.9));
        }
        // freefall suit flutter, wingsuit buzz, canopy
        l.flutter = p.freefall * 0.12 * ss(10.0, 50.0, v) * (0.4 + 0.6 * p.spread);
        l.flutter_hz = 5.0 + 0.35 * v;
        l.suit = p.wingsuit * 0.12 * ss(15.0, 45.0, v);
        l.suit_f = 55.0 + 1.4 * v + 30.0 * p.wing_load;
        if l.suit > 1e-5 {
            self.suit_lp.set(1_200.0);
            self.suit_buzz.set(l.suit_f * 4.0, 2.5);
        }
        l.groan = 0.08 * p.wingsuit * p.wing_load;
        if hit(rng, p.wingsuit * (18.0 - v).max(0.0) * 0.5) {
            recipes::flap(spawn, rng, 0.1 * p.wingsuit);
        }
        l.canopy = 0.06 * p.canopy;
        if hit(rng, p.canopy * 1.2) {
            recipes::flap(spawn, rng, 0.08 * p.canopy);
        }

        // ---- rocket boots: two flames at slightly different pitches so they beat.
        let ab = p.afterburner;
        for i in 0..2 {
            let thr = p.boots[i].max(0.0);
            let pitch = [1.0, 1.065][i] * (1.0 + 0.7 * ab) * (1.0 + if i == 0 { 0.04 } else { -0.04 } * p.leg_steer);
            l.boot[i] = 0.12 * thr.min(1.0).sqrt() * (1.0 + 0.6 * ab);
            l.boot_f[i] = 34.0 * pitch;
            l.crack_rate[i] = if thr > 0.01 { 30.0 + 120.0 * thr } else { 0.0 };
            l.thrum[i] = thr.min(1.0) * (0.04 + 0.2 * (1.0 - air));
            if l.boot[i] > 1e-5 {
                self.hiss[i].set(1_400.0 * pitch, 1.0);
                self.rumble[i].set(120.0 * pitch * (1.0 + ab), 0.7);
            }
        }
        l.ab = 0.6 * ab * (p.boots[0] + p.boots[1]).min(1.0);
        for k in 0..blast::N {
            l.blast[k] = 0.3 * p.blast * p.blast_kind[k];
        }
        l.blast_rumble = 0.25 * p.blast;

        // ---- body: jump charge, re-entry, servos, space
        let c = p.jump_charge.clamp(0.0, 1.0);
        l.charge = 0.06 * ss(0.01, 0.1, c);
        l.charge_f = 160.0 + 520.0 * c.powf(1.5);
        l.reentry = p.reentry.clamp(0.0, 1.5);
        l.servo = 0.012 * p.servo;
        l.servo_f = 380.0 + 220.0 * p.servo;
        self.servo_lp.set(1_400.0);
        l.space = 0.006 * (1.0 - air);
        if hit(rng, 0.25 * (1.0 - air)) {
            recipes::blip(spawn, rng);
        }

        // ---- world wind + vegetation
        let gw = self.gust.tick();
        self.gust_now = (p.wind + p.gust * gw.max(-0.5)).max(0.0);
        let w = self.gust_now;
        l.wind = 0.05 * ss(1.0, 18.0, w) * (0.3 + 0.7 * p.exposed);
        if l.wind > 1e-5 {
            for b in &mut self.wind {
                b.set(250.0 + 30.0 * w, 0.5);
            }
            self.wind_whistle.set(1_500.0 + 60.0 * w, 12.0);
            l.wind_whistle = 0.01 * ss(10.0, 20.0, w) * p.exposed;
        }
        for i in 0..veg::N {
            let r = &VEG[i];
            let mut g = p.veg[i] * r.level * ss(r.wind_lo, r.wind_lo + 10.0, w);
            if i == veg::CACTUS {
                g *= ss(10.0, 16.0, w);
            }
            l.veg[i] = g;
            if g > 1e-5 {
                let fc = r.fc * (1.0 + 0.004 * w);
                self.veg[i][0].set(fc, r.q);
                self.veg[i][1].set(fc * 1.04, r.q);
            }
            if r.clack_rate > 0.0 && hit(rng, p.veg[i] * r.clack_rate * ss(r.wind_lo + 2.0, r.wind_lo + 12.0, w)) {
                recipes::clack(spawn, rng, r.clack_fc, r.clack_q, 0.12 * p.veg[i]);
            }
        }

        // ---- water
        if p.surf > 1e-4 {
            let size = 0.7 + 0.5 * ss(3.0, 15.0, p.wind);
            let before = self.surf_ph;
            self.surf_ph += BLOCK_S / self.surf_period;
            if before < 0.55 && self.surf_ph >= 0.55 {
                // the wave breaks
                let g = p.surf * size;
                if p.surf_rocky > 0.05 {
                    spawn.push(Part::brown(150.0).env(0.01, 0.6, 2.5).gain(0.7 * g * p.surf_rocky).pan(rng.bi() * 0.4).bus(Bus::Amb));
                    spawn.push(Part::noise(Mode::Hp, 4_000.0, 0.7).env(0.05, 0.4, 1.5).at(0.05).gain(0.25 * g * p.surf_rocky).bus(Bus::Amb));
                }
                spawn.push(Part::noise(Mode::Bp, 1_200.0, 0.6).env(0.02, 0.5, 2.0).gain(0.4 * g).pan(rng.bi() * 0.5).bus(Bus::Amb));
                spawn.push(Part::noise(Mode::Lp, 3_500.0, 0.7).env(0.4, 1.5, 4.0).at(0.3).gain(0.3 * g * (1.0 - p.surf_rocky)).pan_sweep(rng.bi() * 0.5, rng.bi() * 0.5).bus(Bus::Amb));
            }
            if self.surf_ph >= 1.0 {
                self.surf_ph -= 1.0;
                self.surf_period = rng.range(8.0, 12.0) * (1.0 - 0.15 * ss(5.0, 15.0, p.wind));
            }
            let ph = self.surf_ph;
            self.surf_env = if ph < 0.55 { 0.3 + 0.7 * (ph / 0.55).powi(2) } else { 0.25 + (-(ph - 0.55) * 6.0).exp() };
            l.surf = 0.15 * p.surf * size * self.surf_env;
        }
        l.lake = 0.08 * p.lake * (0.4 + 0.6 * ss(1.0, 8.0, p.wind));
        if hit(rng, p.lake * 0.5) {
            recipes::plink(spawn, rng);
        }
        l.river = 0.12 * p.river;
        l.rapids = 0.12 * p.river * p.river_steep;
        if hit(rng, p.river * (10.0 + 60.0 * p.river_steep)) {
            recipes::bubble(spawn, rng, 0.02 * (0.5 + p.river));
        }
        l.fall = p.waterfall;

        // ---- rain (mm/h), drizzle, hail
        let r = ss(0.0, 10.0, p.rain);
        l.rain_hiss = 0.05 * r;
        l.leaf_rate = (300.0 + 1_500.0 * r) * p.rain_on[rain_on::LEAVES] * (p.rain > 0.05) as u8 as f32;
        l.leaf = 0.12 * p.rain_on[rain_on::LEAVES];
        l.pave_rate = 800.0 * r * p.rain_on[rain_on::PAVING];
        l.pave = 0.06 * r * p.rain_on[rain_on::PAVING];
        l.roof_rate = 400.0 * r * p.rain_on[rain_on::ROOF];
        l.roof = 0.25 * p.rain_on[rain_on::ROOF];
        l.drizzle = 0.03 * p.drizzle;
        l.hail_rate = 300.0 * p.hail;
        if p.rain > 0.05 {
            for _ in 0..3 {
                if hit(rng, p.rain_on[rain_on::WATER] * (2.0 + 30.0 * r)) {
                    recipes::plink(spawn, rng);
                }
            }
            if hit(rng, p.exposed * (1.0 + 12.0 * r)) {
                recipes::shell_ping(spawn, rng, 0.0);
            }
        }
        if hit(rng, p.exposed * p.hail * 25.0) {
            recipes::shell_ping(spawn, rng, 1.0);
        }

        // ---- animals
        l.drone = 0.025 * p.fauna[fauna::DRONE];
        self.cicada_swell += BLOCK_S / 7.0;
        let swell = 0.5 - 0.5 * (self.cicada_swell.fract() * TAU).cos();
        l.cicada = 0.06 * p.fauna[fauna::CICADAS] * (0.2 + 0.8 * swell);
        let chirp_hz = dolbear_cpm(p.temp_c) / 60.0;
        if p.fauna[fauna::CRICKETS] > 0.01 && chirp_hz > 0.0 {
            for (i, t) in self.crickets.iter_mut().enumerate() {
                *t += BLOCK_S * chirp_hz * (0.97 + 0.02 * i as f32);
                if *t >= 1.0 {
                    *t -= 1.0;
                    let pn = [-0.7, -0.2, 0.3, 0.8][i];
                    for k in 0..3 {
                        spawn.push(
                            Part::tone(crate::parts::Src::Sine, 4_400.0 * (1.0 + 0.03 * i as f32))
                                .env(0.002, 0.006, 0.015)
                                .at(k as f32 * 0.03)
                                .gain(0.03 * p.fauna[fauna::CRICKETS])
                                .pan(pn)
                                .bus(Bus::Amb),
                        );
                    }
                }
            }
        }
        for (k, rate) in BIRD_RATE.iter().enumerate() {
            if *rate > 0.0 && hit(rng, rate * p.fauna[k]) {
                match k {
                    fauna::BIRDS => recipes::bird_phrase(spawn, rng),
                    fauna::FROGS => recipes::frog(spawn, rng),
                    fauna::OWLS => recipes::owl(spawn, rng, p.temp_c > 18.0),
                    fauna::GULLS => recipes::gull(spawn, rng),
                    fauna::COWBELLS => recipes::cowbell(spawn, rng),
                    fauna::ICE => recipes::ice(spawn, rng),
                    fauna::LOONS => recipes::loon(spawn, rng),
                    _ => {}
                }
            }
        }

        // ---- city
        l.traffic_far = 0.25 * p.traffic_far;
        l.traffic_near = 0.12 * p.traffic_near;
        if hit(rng, 0.04 * p.traffic_near) {
            recipes::horn(spawn, rng);
        }
        if hit(rng, 0.15 * p.traffic_minor) {
            recipes::car_pass(spawn, rng);
        }
        l.crowd = 0.05 * p.crowd;
        l.industry = p.industry;
        if hit(rng, 0.3 * p.industry) {
            recipes::clank(spawn, rng);
        }
        if hit(rng, 0.008 * p.industry) {
            recipes::reverse_beeps(spawn, rng);
        }
        l.hum = 0.012 * p.hum;
        l.hum_f = p.grid_hz;
        l.sodium = 0.004 * p.sodium;
        if spawn.len() < 20 && hit(rng, p.rail / 80.0) {
            recipes::train(spawn, rng);
        }

        // ---- vehicles
        let ts = p.tyre_speed.max(0.0);
        let tg = ss(0.5, 25.0, ts) * (0.4 + 0.6 * ss(5.0, 40.0, ts));
        l.tyre = 0.08 * tg * p.tyre_on[tyre::ASPHALT];
        self.tyre.set(600.0 + 12.0 * ts, 0.8);
        l.whine = 0.01 * tg * p.tyre_on[tyre::ASPHALT];
        l.whine_f = 9.0 * ts;
        l.gravel_rate = (30.0 + 25.0 * ts) * p.tyre_on[tyre::GRAVEL] * (ts > 0.5) as u8 as f32;
        l.gravel = 0.25 * tg.max(0.2) * p.tyre_on[tyre::GRAVEL];
        l.dirt = 0.2 * tg * p.tyre_on[tyre::DIRT];
        l.skid = 0.08 * p.skid * (0.3 + 0.7 * p.tyre_on[tyre::ASPHALT]);
        self.skid_lp.set(3_000.0);
        l.stall = 0.05 * ss(0.3, 0.6, p.stall);
        self.stall_lp.set(3_500.0);
        l.cabin = 0.08 * p.cabin_wind;

        // ---- UI: altimeter beeps (continuous at deployment height) and ground-proximity warning
        if p.altimeter >= 12.0 {
            l.alt_tone = 0.03;
        } else if p.altimeter > 0.0 {
            self.alt_t += BLOCK_S * p.altimeter;
            if self.alt_t >= 1.0 {
                self.alt_t -= 1.0;
                spawn.push(Part::tone(crate::parts::Src::Sine, 2_800.0).env(0.003, 1.0, 0.06).gain(0.03).bus(Bus::Ui));
            }
        } else {
            self.alt_t = 0.95;
        }
        l.gpw = 0.035 * ss(0.05, 0.2, p.gpw) * (0.6 + 0.4 * p.gpw);
        l.gpw_rate = 4.0 + 8.0 * p.gpw;
        l
    }

    #[inline]
    fn sample(&mut self, l: &Lv, o: &mut Out) {
        // ear wind
        if l.ear > 1e-5 {
            let buf = (1.0 + l.rough * 0.6 * self.buffet.tick()).max(0.0);
            let a = self.ear[0].tick() * buf;
            let b = self.ear[1].tick() * buf;
            let h = self.howl.tick() * l.howl + self.whistle.tick() * l.whistle;
            let g = l.ear * 3.0;
            o.air.0 += (a * g + h) * l.ear_pan.0;
            o.air.1 += (b * g + h) * l.ear_pan.1;
        }
        if l.flutter > 1e-5 {
            let m = osc(&mut self.flutter_ph, l.flutter_hz);
            let x = self.flutter.tick() * m * m * l.flutter * 3.0;
            o.air.0 += x;
            o.air.1 += x;
        }
        if l.suit > 1e-5 {
            let s = saw(&mut self.suit_ph, l.suit_f);
            let fabric = self.suit_lp.lp(s) * 0.6 + self.suit_buzz.tick() * (0.5 + 0.5 * s) * 2.0;
            let x = fabric * l.suit;
            o.air.0 += x;
            o.air.1 += x;
        }
        if l.groan > 1e-5 {
            let x = osc(&mut self.groan_ph, 70.0) * l.groan;
            o.body.0 += x;
            o.body.1 += x;
        }
        if l.canopy > 1e-5 {
            let x = self.canopy.tick() * l.canopy * 2.0;
            o.air.0 += x;
            o.air.1 += x;
        }
        // boots
        for i in 0..2 {
            let (pl, pr) = pan(if i == 0 { -0.35 } else { 0.35 });
            if l.boot[i] > 1e-5 {
                if self.rng.chance(l.crack_rate[i]) {
                    self.crack_env[i] = self.rng.range(0.4, 1.0);
                }
                self.crack_env[i] *= 0.995;
                let x = self.hiss[i].tick() * 2.0 + self.rumble[i].tick() * 0.8 + self.crack[i].tick() * self.crack_env[i];
                let y = x * l.boot[i];
                o.air.0 += y * pl;
                o.air.1 += y * pr;
            }
            if l.thrum[i] > 1e-5 {
                let ph = &mut self.thrum_ph[i];
                let s = sin_t(*ph) + 0.5 * sin_t(*ph * 2.0);
                *ph = (*ph + l.boot_f[i] / SR).fract();
                let y = s * l.thrum[i];
                o.body.0 += y * pl;
                o.body.1 += y * pr;
            }
        }
        if l.ab > 1e-5 {
            let x = self.ab.tick() * l.ab;
            o.air.0 += x;
            o.air.1 += x;
        }
        if l.blast_rumble > 1e-5 {
            let fl = (1.0 + 0.5 * self.blast_w.tick()).max(0.0);
            let mut x = self.blast_rumble.tick() * l.blast_rumble;
            for k in 0..blast::N {
                if l.blast[k] > 1e-5 {
                    x += self.blast[k].tick() * l.blast[k] * if k == blast::GRASS { fl } else { 1.0 };
                }
            }
            o.air.0 += x;
            o.air.1 += x;
        }
        if l.charge > 1e-5 {
            let x = osc(&mut self.charge_ph, l.charge_f) * l.charge;
            o.body.0 += x;
            o.body.1 += x;
        }
        if l.reentry > 1e-4 {
            let crackle = (1.0 + 0.8 * self.reentry_w.tick()).max(0.0);
            let x = (self.reentry.tick() * 0.5 + self.reentry_hi.tick() * 0.25 * crackle) * l.reentry;
            o.body.0 += x;
            o.body.1 += x;
        }
        if l.servo > 1e-5 {
            let x = self.servo_lp.lp(saw(&mut self.servo_ph, l.servo_f)) * l.servo;
            o.body.0 += x;
            o.body.1 += x;
        }
        if l.space > 1e-5 {
            let ph = &mut self.space_ph;
            let s = sin_t(*ph) + 0.5 * sin_t(*ph * 2.0);
            *ph = (*ph + 117.0 / SR).fract();
            o.body.0 += s * l.space;
            o.body.1 += s * l.space;
        }

        // ---- world (Amb)
        let mut al = 0.0;
        let mut ar = 0.0;
        if l.wind > 1e-5 {
            let wh = self.wind_whistle.tick() * l.wind_whistle * 4.0;
            al += self.wind[0].tick() * l.wind * 2.0 + wh;
            ar += self.wind[1].tick() * l.wind * 2.0 + wh;
        }
        for i in 0..veg::N {
            if l.veg[i] > 1e-5 {
                let r = &VEG[i];
                let mut amp = (1.0 + r.depth * 0.8 * self.veg_w[i].tick()).max(0.0) * l.veg[i] * 2.0;
                let mut pn = 0.0;
                if i == veg::GRASS {
                    let s = osc(&mut self.grass_ph, 0.12);
                    amp *= 0.6 + 0.4 * s;
                    pn = 0.5 * s;
                }
                let a = self.veg[i][0].tick() * amp;
                let b = self.veg[i][1].tick() * amp;
                al += a * (1.0 - pn);
                ar += b * (1.0 + pn);
            }
        }
        if l.surf > 1e-5 {
            let x = (self.surf[0].tick() + self.surf[1].tick()) * l.surf * 2.0;
            al += x;
            ar += x;
        }
        if l.lake > 1e-5 {
            let lap = (0.5 + 0.5 * self.lake_w.tick()).max(0.0);
            let x = self.lake.tick() * lap * l.lake * 2.0;
            al += x;
            ar += x;
        }
        if l.river > 1e-5 {
            let a = self.river[0].tick() * l.river * 2.0 + self.rapids.tick() * l.rapids;
            let b = self.river[1].tick() * l.river * 1.5 + self.rapids.tick() * l.rapids;
            al += a;
            ar += b;
        }
        if l.fall > 1e-5 {
            let lo = self.fall_lo.tick() * 0.4 * l.fall;
            al += lo + self.fall[0].tick() * 0.25 * l.fall * 2.0;
            ar += lo + self.fall[1].tick() * 0.25 * l.fall * 2.0;
        }
        if l.rain_hiss > 1e-5 || l.drizzle > 1e-5 || l.hail_rate > 0.0 {
            al += self.rain[0].tick() * l.rain_hiss * 2.0;
            ar += self.rain[1].tick() * l.rain_hiss * 2.0;
            for (k, f) in self.leaf.iter_mut().enumerate() {
                let x = if self.rng.chance(l.leaf_rate * 0.5) { self.rng.bi() } else { 0.0 };
                let y = f.run(x, Mode::Bp) * l.leaf;
                if k == 0 { al += y } else { ar += y }
            }
            if l.pave > 1e-5 {
                let tick = if self.rng.chance(l.pave_rate) { self.rng.bi() * 0.5 } else { 0.0 };
                let x = self.pave.tick() * l.pave + self.pave.run(tick) * 0.1;
                al += x;
                ar += x;
            }
            if l.roof_rate > 0.0 {
                let x = if self.rng.chance(l.roof_rate) { self.rng.bi() } else { 0.0 };
                let y = self.roof.run(x, Mode::Bp) * l.roof;
                al += y;
                ar += y;
            }
            let d = self.drizzle.tick() * l.drizzle * 2.0;
            al += d;
            ar += d;
            if self.rng.chance(l.hail_rate) {
                let x = self.hail.run(self.rng.bi()) * 0.3;
                let p = self.rng.bi();
                al += x * (1.0 - p);
                ar += x * (1.0 + p);
            }
        }
        if l.drone > 1e-5 {
            let m = (1.0 + 0.3 * self.drone_w.tick()).max(0.0);
            let mut x = 0.0;
            for b in &mut self.drone {
                x += b.tick();
            }
            al += x * l.drone * m * 4.0;
            ar += x * l.drone * m * 4.0;
        }
        if l.cicada > 1e-5 {
            let m = 0.15 + 0.85 * (0.5 + 0.5 * osc(&mut self.cicada_ph, 180.0));
            let x = self.cicada.tick() * m * l.cicada * 3.0;
            al += x * 0.8;
            ar += x;
        }
        if l.traffic_far > 1e-5 {
            let x = self.traffic[0].tick() * l.traffic_far + self.traffic[1].tick() * l.traffic_far * 0.3;
            al += x;
            ar += x;
        }
        if l.traffic_near > 1e-5 {
            let m = (0.6 + 0.6 * self.near_w.tick()).max(0.0);
            let x = self.near.tick() * m * l.traffic_near * 2.0;
            al += x;
            ar += x;
        }
        if l.crowd > 1e-5 {
            for (k, b) in self.crowd.iter_mut().enumerate() {
                let m = self.crowd_w[k].tick().max(0.0);
                let x = b.tick() * m * l.crowd * 3.0;
                if k % 2 == 0 { al += x } else { ar += x }
            }
        }
        if l.industry > 1e-5 {
            let x = osc(&mut self.ind_ph, 120.0) * 0.03 * l.industry + self.ind.tick() * 0.04 * l.industry;
            al += x;
            ar += x;
        }
        if l.hum > 1e-6 || l.sodium > 1e-6 {
            // mains hum: transformers buzz at twice the line frequency, plus odd-ish harmonics
            let ph = self.hum_ph;
            self.hum_ph = (ph + l.hum_f / SR).fract();
            let s = 0.3 * sin_t(ph) + sin_t(2.0 * ph) + 0.25 * sin_t(4.0 * ph) + 0.12 * sin_t(6.0 * ph);
            let buzz = ((ph * 4.0).fract() * 2.0 - 1.0) * l.sodium;
            al += s * l.hum + buzz;
            ar += s * l.hum + buzz;
        }
        o.amb.0 += al;
        o.amb.1 += ar;

        // ---- vehicles (Air)
        if l.tyre > 1e-5 || l.whine > 1e-5 {
            let x = self.tyre.tick() * l.tyre * 2.0 + osc(&mut self.whine_ph, l.whine_f) * l.whine;
            o.air.0 += x;
            o.air.1 += x;
        }
        if l.gravel_rate > 0.0 {
            let imp = if self.rng.chance(l.gravel_rate) { self.rng.bi() * 3.0 } else { 0.0 };
            let x = self.gravel.run(imp + self.rng.bi() * 0.05) * l.gravel;
            o.air.0 += x;
            o.air.1 += x;
        }
        if l.dirt > 1e-5 {
            let m = (1.0 + 0.5 * self.dirt_w.tick()).max(0.0);
            let x = self.dirt.tick() * m * l.dirt;
            o.air.0 += x;
            o.air.1 += x;
        }
        if l.skid > 1e-5 {
            let f = 950.0 * (1.0 + 0.03 * self.skid_w.tick());
            let x = self.skid_lp.lp(saw(&mut self.skid_ph, f)) * l.skid;
            o.air.0 += x;
            o.air.1 += x;
        }
        if l.cabin > 1e-5 {
            let x = self.cabin.tick() * l.cabin * 2.0;
            o.air.0 += x;
            o.air.1 += x;
        }
        // ---- UI
        if l.stall > 1e-5 {
            let s = if self.stall_ph < 0.5 { 0.7 } else { -0.7 };
            self.stall_ph = (self.stall_ph + 1_600.0 / SR).fract();
            let x = self.stall_lp.lp(s) * l.stall;
            o.ui.0 += x;
            o.ui.1 += x;
        }
        if l.alt_tone > 0.0 {
            let x = osc(&mut self.alt_ph, 2_800.0) * l.alt_tone;
            o.ui.0 += x;
            o.ui.1 += x;
        }
        if l.gpw > 1e-5 {
            self.gpw_gate = (self.gpw_gate + l.gpw_rate / SR).fract();
            let f = if self.gpw_gate < 0.5 { 1_000.0 } else { 1_500.0 };
            let x = osc(&mut self.gpw_ph, f) * l.gpw;
            o.ui.0 += x;
            o.ui.1 += x;
        }
    }
}

// ---------------------------------------------------------------- the mixer

pub struct Scape {
    rx: Option<Receiver<Msg>>,
    target: Params,
    p: Params,
    rng: Rng,
    parts: Vec<Part>,
    spawn: Vec<Part>,
    seed: u32,
    buf: [f32; BLOCK * 2],
    pos: usize,
    k: f32,
    beds: Beds,
    amb_lp: [OnePole; 2],
    echo: [Delay; 2],
}

impl Scape {
    pub fn new(rx: Receiver<Msg>) -> Scape {
        let mut s = Scape::offline();
        s.rx = Some(rx);
        s
    }

    /// A scape without a channel, driven by `set` / `event` (tests, offline renders).
    pub fn offline() -> Scape {
        let mut p = Params::default();
        p.master = 0.0; // fade in
        Scape {
            rx: None,
            target: Params::default(),
            p,
            rng: Rng::new(0xa0d1_0001),
            parts: Vec::with_capacity(MAX_PARTS),
            spawn: Vec::with_capacity(MAX_PARTS),
            seed: 1,
            buf: [0.0; BLOCK * 2],
            pos: BLOCK * 2,
            k: 1.0 - (-BLOCK_S / 0.06).exp(),
            beds: Beds::new(),
            amb_lp: [OnePole::default(); 2],
            echo: [Delay::new(0.5), Delay::new(0.5)],
        }
    }

    pub fn set(&mut self, p: Params) {
        self.target = p;
    }

    pub fn event(&mut self, e: Event) {
        recipes::expand(e, &self.target, &mut self.rng, &mut self.spawn);
    }

    pub fn active_parts(&self) -> usize {
        self.parts.len()
    }

    fn drain(&mut self) {
        let Some(rx) = &self.rx else { return };
        while let Ok(m) = rx.try_recv() {
            match m {
                Msg::Params(p) => self.target = p,
                Msg::Event(e) => recipes::expand(e, &self.target, &mut self.rng, &mut self.spawn),
            }
        }
    }

    fn render_block(&mut self) {
        self.drain();
        self.p.smooth_toward(&self.target, self.k);
        let p = self.p;
        let lv = self.beds.prepare(&p, &mut self.rng, &mut self.spawn);
        for mut part in self.spawn.drain(..) {
            if self.parts.len() >= MAX_PARTS {
                break;
            }
            self.seed = self.seed.wrapping_add(0x9e37_79b9);
            part.seed(self.seed);
            self.parts.push(part);
        }
        let air = p.air_gate();
        let amb_g = p.world_gate() * p.duck();
        let muffle = 16_000.0 * (1.0 - 0.75 * p.snow) * (1.0 - 0.5 * p.fog);
        for f in &mut self.amb_lp {
            f.set(muffle);
        }
        let echo_wet = 0.35 * p.echo * (1.0 - 0.6 * p.snow) * air;
        let d = p.echo_delay_s.clamp(0.01, 0.45) * SR;
        let master = p.master;
        let mix = p.mix.map(|g| g.clamp(0.0, 1.0));
        for i in 0..BLOCK {
            let mut o = Out::default();
            self.beds.sample(&lv, &mut o);
            for part in &mut self.parts {
                let s = part.tick();
                o.add(part.bus, s);
            }
            let [gb, ga, gm, gu] = mix;
            o.body.0 *= gb;
            o.body.1 *= gb;
            let al = self.amb_lp[0].lp(o.amb.0) * amb_g * gm;
            let ar = self.amb_lp[1].lp(o.amb.1) * amb_g * gm;
            let air_l = o.air.0 * air * ga;
            let air_r = o.air.1 * air * ga;
            let mut l = o.body.0 + air_l + al + o.ui.0 * gu;
            let mut r = o.body.1 + air_r + ar + o.ui.1 * gu;
            if echo_wet > 1e-4 {
                // street-canyon slap-back: the facades across the street throw your own sounds back
                let el = self.echo[0].read(d);
                let er = self.echo[1].read(d * 1.13);
                self.echo[0].write(o.body.0 * 0.6 + air_l + el * 0.3);
                self.echo[1].write(o.body.1 * 0.6 + air_r + er * 0.3);
                l += er * echo_wet;
                r += el * echo_wet;
            }
            self.buf[i * 2] = soft(l * master);
            self.buf[i * 2 + 1] = soft(r * master);
        }
        self.parts.retain(|p| !p.done());
        self.pos = 0;
    }

    /// Next interleaved sample (L, R, L, R...).
    #[inline]
    pub fn next_sample(&mut self) -> f32 {
        if self.pos >= BLOCK * 2 {
            self.render_block();
        }
        let s = self.buf[self.pos];
        self.pos += 1;
        s
    }

    /// Fill an interleaved stereo buffer.
    pub fn render(&mut self, out: &mut [f32]) {
        for s in out {
            *s = self.next_sample();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::Surface;

    fn rms(s: &mut Scape, secs: f32) -> f32 {
        let mut buf = vec![0.0; (secs * SR) as usize * 2];
        s.render(&mut buf);
        assert!(buf.iter().all(|x| x.is_finite()));
        let tail = &buf[buf.len() / 2..];
        (tail.iter().map(|x| x * x).sum::<f32>() / tail.len() as f32).sqrt()
    }

    #[test]
    fn default_is_quiet() {
        let mut s = Scape::offline();
        s.set(Params::default());
        assert!(rms(&mut s, 1.0) < 1e-3);
    }

    #[test]
    fn mix_gain_mutes_its_bus() {
        let mut p = Params::default();
        p.traffic_far = 1.0;
        p.crowd = 1.0;
        let mut s = Scape::offline();
        s.set(p);
        let on = rms(&mut s, 1.5);
        p.mix[crate::params::bus::AMB] = 0.0;
        let mut s = Scape::offline();
        s.set(p);
        let off = rms(&mut s, 1.5);
        assert!(on > 1e-3 && off < on * 0.05, "{on} {off}");
    }

    #[test]
    fn rain_and_wind_make_sound() {
        let mut s = Scape::offline();
        let mut p = Params::default();
        p.rain = 8.0;
        p.rain_on = [0.6, 0.4, 0.0, 0.0];
        p.wind = 9.0;
        p.veg[veg::BROADLEAF] = 1.0;
        s.set(p);
        let r = rms(&mut s, 1.5);
        assert!(r > 0.005 && r < 0.7, "{r}");
    }

    #[test]
    fn space_silences_the_world_but_not_the_body() {
        let mut p = Params::default();
        p.rain = 8.0;
        p.rain_on = [1.0, 0.0, 0.0, 0.0];
        p.traffic_far = 1.0;
        p.exposed = 0.0;
        let mut s = Scape::offline();
        s.set(p);
        let ground = rms(&mut s, 1.5);
        p.density = 0.0;
        let mut s = Scape::offline();
        s.set(p);
        let space = rms(&mut s, 1.5);
        p.servo = 1.0;
        let mut s = Scape::offline();
        s.set(p);
        let body = rms(&mut s, 1.5);
        assert!(space < ground * 0.25, "world in space: {space} vs ground {ground}");
        assert!(body > space * 1.5, "servos {body} vs {space}");
    }

    #[test]
    fn events_play_and_finish() {
        let mut s = Scape::offline();
        s.set(Params::default());
        let mut buf = vec![0.0; 9_600];
        s.render(&mut buf); // fade in
        s.event(Event::Footstep { surface: Surface::Gravel, force: 0.8, pan: -0.5, sprint: false });
        s.render(&mut buf);
        let e: f32 = buf.iter().map(|x| x * x).sum();
        assert!(e > 1e-3, "{e}");
        let mut long = vec![0.0; 96_000];
        s.render(&mut long);
        assert_eq!(s.active_parts(), 0);
    }

    #[test]
    fn everything_at_once_stays_finite_and_bounded() {
        let mut p = Params::default();
        p.q = 5_000.0;
        p.mach = 0.95;
        p.spread = 1.0;
        p.freefall = 1.0;
        p.wingsuit = 1.0;
        p.canopy = 1.0;
        p.boots = [4.0, 4.0];
        p.afterburner = 1.0;
        p.blast = 1.0;
        p.blast_kind = [1.0; blast::N];
        p.reentry = 1.0;
        p.servo = 1.0;
        p.wind = 25.0;
        p.gust = 10.0;
        p.veg = [1.0; veg::N];
        p.surf = 1.0;
        p.river = 1.0;
        p.waterfall = 1.0;
        p.rain = 60.0;
        p.hail = 1.0;
        p.rain_on = [1.0; rain_on::N];
        p.fauna = [1.0; fauna::N];
        p.temp_c = 28.0;
        p.traffic_far = 1.0;
        p.traffic_near = 1.0;
        p.crowd = 1.0;
        p.industry = 1.0;
        p.hum = 1.0;
        p.rail = 50.0;
        p.echo = 1.0;
        p.tyre_speed = 40.0;
        p.tyre_on = [1.0; tyre::N];
        p.skid = 1.0;
        p.stall = 1.0;
        p.altimeter = 20.0;
        p.gpw = 1.0;
        let mut s = Scape::offline();
        s.set(p);
        for _ in 0..20 {
            s.event(Event::Thunder { distance_m: 300.0 });
            s.event(Event::Land { surface: Surface::Rock, impact: 20.0 });
        }
        let mut buf = vec![0.0; 96_000];
        s.render(&mut buf);
        let bad = buf.iter().find(|x| !(x.is_finite() && x.abs() <= 1.0));
        assert!(bad.is_none(), "{bad:?}");
        assert!(s.active_parts() <= MAX_PARTS);
    }

    #[test]
    fn wind_is_quiet_in_thin_air_at_the_same_speed() {
        let v = 100.0;
        let loud = {
            let mut p = Params::default();
            p.q = crate::params::dynamic_pressure(1.225, v);
            let mut s = Scape::offline();
            s.set(p);
            rms(&mut s, 1.0)
        };
        let thin = {
            let mut p = Params::default();
            p.density = 0.09; // ~18 km
            p.q = crate::params::dynamic_pressure(0.09, v);
            let mut s = Scape::offline();
            s.set(p);
            rms(&mut s, 1.0)
        };
        assert!(thin < loud * 0.4, "{thin} vs {loud}");
    }
}
