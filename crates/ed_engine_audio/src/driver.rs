//! From a game's own vehicle state to the voice's controls and events.
//!
//! The game already has car physics with rpm, gear and throttle
//! (`ed_game/src/vehicles.rs`), so the kit's `drivetrain.js` is not ported as
//! a whole: the game picks the gears and the wheels set the revs. What the
//! game lacks, and a convincing sound needs, is ported from `drivetrain.js`
//! here: the start-up sequence (crank, catch, flare, settle), clutch slip
//! pulling away, the rev blend and ignition cut through a shift, the
//! downshift blip, the rev limiter, engine braking (overrun), the lift-off
//! and stop events, and the gentle pull-away.
//!
//! The game's rev range is mapped onto the kit car's: game idle → kit idle,
//! game upshift rpm → the kit car's full-throttle shift point, so a GT3
//! screams to 8,900 where the game's generic engine shifts at 6,300, and the
//! pitch tracks the game's own revs either side of that.
//!
//! Turbines (jet, pod, warp) follow the throttle and airspeed like
//! `Drivetrain._noGearbox`; the steam train's cylinders follow its wheels.

use crate::cars::{catalog, Car, Gearbox};
use crate::dsp::{clamp, lerp, smoothstep, Rng};
use crate::voice::{Event, Params};

/// How long a pending start-up flare lasts (drivetrain.js FLARE_S).
const FLARE_S: f64 = 0.4;

fn follow(dt: f64, tau: f64) -> f64 {
    1.0 - (-dt / tau.max(1e-4)).exp()
}

/// What the game knows about the vehicle this frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct DriveInput {
    /// Seconds since the last update.
    pub dt: f64,
    /// Accelerator 0..1.
    pub throttle: f64,
    /// Road (or air) speed, m/s, magnitude.
    pub speed: f64,
    /// The game's crank rpm (gearbox vehicles: wheel speed through the gear).
    pub engine_rpm: f64,
    /// The game's current gear (any numbering; a change is a shift).
    pub gear: i32,
    /// Gearbox output rpm (gearbox whine); 0 when stopped.
    pub shaft_rpm: f64,
    /// Steam whistle 0..1 (steam train only).
    pub horn: f64,
}

/// The game's own engine: the rev range its gearbox works in.
#[derive(Clone, Copy, Debug)]
pub struct GameEngine {
    pub idle: f64,
    /// Where the game's gearbox changes up.
    pub upshift: f64,
    /// Top speed for turbines (m/s): spool climbs with airspeed up to here.
    pub v_max: f64,
}

impl Default for GameEngine {
    fn default() -> Self {
        Self { idle: 850.0, upshift: 6300.0, v_max: 120.0 }
    }
}

/// Events and clicks produced by one update (fixed capacity, no allocation).
#[derive(Clone, Copy, Debug, Default)]
pub struct Events {
    pub events: [(Option<Event>, f64); 4],
    pub n: usize,
    pub clicks: [f64; 2],
    pub n_clicks: usize,
}

impl Events {
    fn push(&mut self, e: Event, amt: f64) {
        if self.n < self.events.len() {
            self.events[self.n] = (Some(e), amt);
            self.n += 1;
        }
    }
    fn click(&mut self, amt: f64) {
        if self.n_clicks < self.clicks.len() {
            self.clicks[self.n_clicks] = amt;
            self.n_clicks += 1;
        }
    }
    pub fn iter(&self) -> impl Iterator<Item = (Event, f64)> + '_ {
        self.events[..self.n].iter().filter_map(|(e, a)| e.map(|e| (e, *a)))
    }
    pub fn clicks(&self) -> &[f64] {
        &self.clicks[..self.n_clicks]
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Mode {
    Gearbox,
    Turbine,
    Direct,
}

#[derive(Clone, Copy, Debug)]
struct Shift {
    up: bool,
    from: f64,
    t: f64,
    dur: f64,
    th: f64,
}

/// Turns a game vehicle's state into [`Params`] and events for one voice.
pub struct Driver {
    car: &'static Car,
    box_: Gearbox,
    mode: Mode,
    pub game: GameEngine,
    /// How eagerly the "driver" blips and bangs (drivetrain.js intensity).
    pub intensity: f64,
    /// Sport: downshifts blip and pop (drivetrain.js only fires 'down' in Sport).
    pub sport: bool,
    rng: Rng,
    running: bool,
    start_t: f64,
    barked: bool,
    t: f64,
    rpm: f64,
    load: f64,
    overrun: f64,
    pull_away: f64,
    starter: f64,
    throttle: f64,
    peak_th: f64,
    limiter_t: f64,
    gear: i32,
    shift: Option<Shift>,
    moving: bool,
    out: Params,
}

impl Driver {
    pub fn new(car: &'static Car, game: GameEngine) -> Self {
        let e = &car.engine;
        let mode = match e.mode.as_deref() {
            Some("turbine") => Mode::Turbine,
            Some("direct") => Mode::Direct,
            _ => Mode::Gearbox,
        };
        let gb = catalog().gearbox.get(&e.gearbox).or_else(|| catalog().gearbox.get("dct")).copied().unwrap_or_default();
        Self {
            car,
            box_: gb,
            mode,
            game,
            intensity: 0.75,
            sport: true,
            rng: Rng::new(0x5eed),
            running: false,
            start_t: 99.0,
            barked: false,
            t: 0.0,
            rpm: 0.0,
            load: 0.0,
            overrun: 0.0,
            pull_away: 0.0,
            starter: 0.0,
            throttle: 0.0,
            peak_th: 0.0,
            limiter_t: 0.0,
            gear: 0,
            shift: None,
            moving: false,
            out: Params::default(),
        }
    }

    pub fn car(&self) -> &'static Car {
        self.car
    }

    /// The controls from the last [`Driver::update`].
    pub fn params(&self) -> Params {
        self.out
    }

    pub fn running(&self) -> bool {
        self.running
    }

    /// Crank and fire up (the start-up sequence plays over the next ~2 s).
    pub fn start(&mut self) {
        self.running = true;
        self.start_t = 0.0;
        self.barked = false;
        self.rpm = 0.0;
        self.load = 0.0;
        self.overrun = 0.0;
        self.shift = None;
        self.limiter_t = 0.0;
    }

    /// Already running (e.g. traffic that was never "started" on screen).
    pub fn start_running(&mut self) {
        self.start();
        self.start_t = 99.0;
        self.rpm = self.car.engine.idle;
    }

    /// Switch the engine off: the revs die away.
    pub fn stop(&mut self) {
        self.running = false;
    }

    /// Map the game's crank rpm onto the kit car's rev range.
    pub fn map_rpm(&self, game_rpm: f64) -> f64 {
        let e = &self.car.engine;
        let (gi, gu) = (self.game.idle, self.game.upshift.max(self.game.idle + 1.0));
        let (ki, ku) = (e.idle, e.shift[1].max(e.idle + 1.0));
        ki + (game_rpm - gi) / (gu - gi) * (ku - ki)
    }

    /// One frame. Returns the controls for the voice; events and clicks for
    /// this frame are written to `ev`.
    pub fn update(&mut self, inp: &DriveInput, ev: &mut Events) -> Params {
        *ev = Events::default();
        let dt = inp.dt.max(1e-4);
        self.t += dt;
        let e = &self.car.engine;
        if !self.running {
            self.rpm += (0.0 - self.rpm) * follow(dt, 0.3);
            self.load += (0.0 - self.load) * follow(dt, 0.05);
            self.out = Params { rpm: self.rpm, load: self.load, ..Params::default() };
            return self.out;
        }
        let v = inp.speed.abs();
        let stopped = v < 0.3;
        let k = if inp.throttle > self.throttle { follow(dt, 0.05) } else { follow(dt, 0.06) };
        self.throttle += (clamp(inp.throttle, 0.0, 1.0) - self.throttle) * k;
        let th = self.throttle;

        // Start-up sequence.
        self.start_t += dt;
        let st = e.start;
        if self.start_t < st.crank + FLARE_S + st.settle {
            self.gear = inp.gear;
            return self.starting(dt, ev);
        }
        self.starter = 0.0;

        // Coming to a stop (air brakes, steam blow-down).
        if v > 3.0 {
            self.moving = true;
        } else if stopped && self.moving {
            self.moving = false;
            ev.push(Event::Stop, 1.0);
        }

        let (mut target, mut tau, mut load);
        let mut cut = 0.0;
        match self.mode {
            Mode::Direct => {
                // Steam: the wheels drive the cylinders.
                let r = e.ratios.first().copied().unwrap_or(1.0);
                target = v / e.tire * 60.0 * r * e.final_drive;
                tau = 0.03;
                load = if th < 0.03 { 0.05 } else { 0.15 + 0.85 * th };
                if stopped {
                    target = 0.0;
                    tau = 0.2;
                    load = 0.0;
                }
            }
            Mode::Turbine => {
                // The spool follows the throttle and climbs with speed.
                let f = clamp(0.62 * th + 0.45 * clamp(v / self.game.v_max.max(1.0), 0.0, 1.0), 0.0, 1.0);
                target = lerp(e.idle, e.redline, f);
                tau = if target > self.rpm { e.rev_up } else { e.rev_down };
                load = if th < 0.03 { 0.0 } else { 0.1 + 0.9 * th };
            }
            Mode::Gearbox => {
                // The game's gear changes become shifts with their sound.
                if inp.gear != self.gear {
                    let up = inp.gear > self.gear;
                    self.gear = inp.gear;
                    let dur = if up { self.box_.up * lerp(1.15, 0.85, th) } else { self.box_.down };
                    self.shift = Some(Shift { up, from: self.rpm, t: 0.0, dur, th });
                    if up && th > 0.4 {
                        ev.push(Event::Up, th);
                    }
                    if !up && self.sport {
                        ev.push(Event::Down, 0.5);
                    }
                }
                let locked = self.map_rpm(inp.engine_rpm).max(e.idle);
                // Clutch slip / torque converter: pulling away holds the revs up.
                let slip = e.idle + (e.launch - e.idle) * smoothstep(th * 1.2) * (1.0 - smoothstep(v / 8.0));
                target = locked.max(slip).max(e.idle);
                tau = 0.04;
                load = if th < 0.03 { 0.0 } else { 0.12 + 0.88 * th };
                if let Some(mut s) = self.shift {
                    let b = self.box_;
                    s.t += dt;
                    let x = clamp(s.t / s.dur.max(1e-3), 0.0, 1.0);
                    let to = locked.max(e.idle);
                    if s.up {
                        if b.clutch {
                            // Clutch in: the revs fall freely, then the clutch bites.
                            target = to;
                            tau = if x < 0.7 { e.rev_down * 0.9 } else { 0.06 };
                            if x < 0.75 {
                                load = 0.02;
                            }
                        } else {
                            target = lerp(s.from, to, smoothstep(x));
                            tau = 0.03;
                            if b.cut > 0.0 && s.th > 0.45 && s.t < b.cut {
                                cut = 1.0;
                            }
                            if b.cut == 0.0 {
                                load *= 0.55;
                            }
                        }
                    } else {
                        // Downshift: blip the throttle to match the revs.
                        target = lerp(s.from, to * 1.03, smoothstep(x * 1.4));
                        tau = if b.clutch { 0.08 } else { 0.04 };
                        if x < 0.7 {
                            load = load.max(b.blip * (0.5 + 0.5 * self.intensity));
                        }
                    }
                    if s.t >= s.dur {
                        if b.clutch {
                            ev.click(0.5 + 0.3 * s.th);
                        }
                        self.shift = None;
                    } else {
                        self.shift = Some(s);
                    }
                }
            }
        }

        // Idle: a slow, gentle wander, a few rpm at most.
        if e.idle > 0.0 && self.mode == Mode::Gearbox && target < e.idle * 1.15 {
            target += 4.0 * (self.t * 0.7).sin() + (self.t * 1.9).sin();
        }
        self.rpm += (target - self.rpm) * follow(dt, tau);

        // Rev limiter: cut the spark, let the revs drop, fire again.
        if self.mode == Mode::Gearbox {
            if self.limiter_t > 0.0 {
                self.limiter_t -= dt;
                cut = 1.0;
                self.rpm = self.rpm.min(e.limiter) - dt * 4000.0;
            } else if self.rpm >= e.limiter {
                self.limiter_t = 0.055;
                self.rpm = e.limiter;
                cut = 1.0;
            }
        }
        if self.rpm < 0.0 {
            self.rpm = 0.0;
        }

        let coasting = load < 0.02 && self.rpm > e.idle * 1.25;
        // Snapping off the throttle from high revs: one bang (and the BOV).
        if coasting && self.overrun < 0.05 && self.rpm > e.redline * 0.35 && self.peak_th > 0.45 {
            ev.push(Event::Lift, (0.4 + 0.6 * self.peak_th).min(1.0));
        }
        self.peak_th = th.max(self.peak_th - dt * 1.5);
        self.overrun += (if coasting { 1.0 } else { 0.0 } - self.overrun) * follow(dt, 0.12);
        let k = if load > self.load { 0.04 } else { 0.08 };
        self.load += (load - self.load) * follow(dt, k);
        // Pulling away gently from a stop eases the low boom.
        let pull = if self.mode == Mode::Gearbox && (v > 0.3 || th > 0.03) {
            (1.0 - smoothstep(v / 4.5)) * (1.0 - smoothstep((th - 0.35) / 0.3))
        } else {
            0.0
        };
        self.pull_away += (pull - self.pull_away) * follow(dt, 0.15);
        self.out = Params {
            rpm: self.rpm,
            load: self.load,
            overrun: self.overrun,
            cut,
            starter: 0.0,
            shaft: if stopped { 0.0 } else { inp.shaft_rpm.abs() },
            horn: if self.mode == Mode::Direct { inp.horn } else { 0.0 },
            pull_away: self.pull_away,
            body: 0.0,
        };
        self.out
    }

    // Start-up (drivetrain.js _starting): the starter cranks, the first
    // cylinders catch unevenly, the engine flares (and may bark), then
    // settles to idle.
    fn starting(&mut self, dt: f64, ev: &mut Events) -> Params {
        let e = &self.car.engine;
        let st = e.start;
        let t = self.start_t;
        let (target, load, tau);
        let mut cut = 0.0;
        self.overrun = 0.0;
        if t < st.crank {
            // Cranking: the crank slows on every compression stroke.
            self.starter = 1.0;
            let strokes = st.crank_rpm / 60.0 * (if e.cyl > 0 { e.cyl } else { 8 }) as f64 / 2.0;
            target = st.crank_rpm * (1.0 + 0.28 * (t * std::f64::consts::PI * 2.0 * strokes).sin());
            tau = 0.015;
            let mut l = 0.0;
            cut = 1.0;
            // Near the end the first cylinders catch, unevenly.
            if t > st.crank * 0.7 && self.rng.next() < 0.35 {
                cut = 0.0;
                l = 0.6;
            }
            load = l;
        } else if t < st.crank + FLARE_S {
            // Caught: it flares up, the starter lets go.
            self.starter = 0.0;
            target = e.idle * st.flare;
            tau = e.rev_up * 0.8;
            load = 0.9;
            if !self.barked && st.bark > 0.0 && t > st.crank + FLARE_S * 0.6 {
                self.barked = true;
                ev.push(Event::Start, st.bark);
            }
        } else {
            // Settle back to idle; cars with a bark crackle on the way down.
            self.starter = 0.0;
            let x = (t - st.crank - FLARE_S) / st.settle.max(1e-3);
            target = lerp(e.idle * st.flare.min(1.35), e.idle, smoothstep(x));
            tau = e.rev_down;
            load = if x < 0.15 { 0.0 } else { 0.15 };
            if st.bark > 0.3 && x < 0.25 {
                self.overrun = 1.0;
            }
        }
        self.rpm += (target - self.rpm) * follow(dt, tau);
        self.load += (load - self.load) * follow(dt, 0.03);
        self.out = Params { rpm: self.rpm, load: self.load, overrun: self.overrun, cut, starter: self.starter, ..Params::default() };
        self.out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cars::car;

    fn drive(d: &mut Driver, secs: f64, inp: DriveInput) -> (Params, Vec<(Event, f64)>) {
        let mut evs = Vec::new();
        let mut p = Params::default();
        let mut ev = Events::default();
        let mut t = 0.0;
        while t < secs {
            p = d.update(&DriveInput { dt: 1.0 / 60.0, ..inp }, &mut ev);
            evs.extend(ev.iter());
            t += 1.0 / 60.0;
        }
        (p, evs)
    }

    #[test]
    fn starts_up_and_idles() {
        let mut d = Driver::new(car("gt3").unwrap(), GameEngine::default());
        d.start();
        let (p, evs) = drive(&mut d, 0.3, DriveInput { engine_rpm: 850.0, ..Default::default() });
        assert!(p.starter > 0.5, "cranking");
        assert!(evs.is_empty());
        let (p, _) = drive(&mut d, 4.0, DriveInput { engine_rpm: 850.0, ..Default::default() });
        assert!((p.rpm - 850.0).abs() < 30.0, "settled at the GT3's idle: {}", p.rpm);
        assert_eq!(p.starter, 0.0);
    }

    #[test]
    fn maps_game_revs_and_fires_shift_and_lift_events() {
        let mut d = Driver::new(car("huracan").unwrap(), GameEngine::default());
        d.start_running();
        // Full throttle at the game's upshift point: the Huracán's shift point.
        let (p, _) = drive(&mut d, 1.0, DriveInput { throttle: 1.0, speed: 20.0, engine_rpm: 6300.0, gear: 1, shaft_rpm: 1500.0, ..Default::default() });
        assert!((p.rpm - 8400.0).abs() < 60.0, "{}", p.rpm);
        assert!(p.load > 0.95);
        // Shift up under full throttle: an 'up' event and a short cut (DCT).
        let mut ev = Events::default();
        let p = d.update(&DriveInput { dt: 1.0 / 60.0, throttle: 1.0, speed: 20.0, engine_rpm: 4500.0, gear: 2, shaft_rpm: 1500.0, ..Default::default() }, &mut ev);
        assert_eq!(ev.iter().next().map(|e| e.0), Some(Event::Up));
        assert_eq!(p.cut, 1.0);
        // Lift off at high revs: one 'lift', then engine braking.
        let (p, evs) = drive(&mut d, 1.0, DriveInput { throttle: 0.0, speed: 20.0, engine_rpm: 5500.0, gear: 2, shaft_rpm: 1500.0, ..Default::default() });
        assert_eq!(evs.iter().filter(|e| e.0 == Event::Lift).count(), 1);
        assert!(p.overrun > 0.9 && p.load < 0.01);
    }

    #[test]
    fn limiter_cuts_and_stop_fires() {
        let mut d = Driver::new(car("mustang").unwrap(), GameEngine::default());
        d.start_running();
        let mut cuts = 0;
        let mut ev = Events::default();
        for _ in 0..120 {
            let p = d.update(&DriveInput { dt: 1.0 / 60.0, throttle: 1.0, speed: 50.0, engine_rpm: 7000.0, gear: 5, shaft_rpm: 3000.0, ..Default::default() }, &mut ev);
            if p.cut > 0.5 {
                cuts += 1;
            }
            assert!(p.rpm <= car("mustang").unwrap().engine.limiter + 1.0);
        }
        assert!(cuts > 10, "bounces off the limiter");
        let (_, evs) = drive(&mut d, 0.5, DriveInput { speed: 0.0, engine_rpm: 850.0, gear: 0, ..Default::default() });
        assert!(evs.iter().any(|e| e.0 == Event::Stop));
    }

    #[test]
    fn jet_spool_follows_throttle() {
        let mut d = Driver::new(car("jet").unwrap(), GameEngine::default());
        d.start_running();
        let (idle, _) = drive(&mut d, 3.0, DriveInput::default());
        let (full, _) = drive(&mut d, 6.0, DriveInput { throttle: 1.0, speed: 100.0, ..Default::default() });
        let e = &car("jet").unwrap().engine;
        assert!((idle.rpm - e.idle).abs() < 100.0);
        assert!(full.rpm > e.redline * 0.95 && full.load > 0.95);
    }
}
