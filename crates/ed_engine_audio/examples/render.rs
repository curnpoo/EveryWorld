//! Render a vehicle to a WAV so you can listen without running the game.
//!
//!   cargo run --release -p ed_engine_audio --example render -- huracan out.wav
//!   cargo run --release -p ed_engine_audio --example render -- jet out.wav
//!   cargo run --release -p ed_engine_audio --example render -- gt3 out.wav --flyby
//!   cargo run --release -p ed_engine_audio --example render -- gt3 out.wav --controls ref/gt3.controls.json
//!   cargo run --release -p ed_engine_audio --example render -- --list
//!
//! Default: a scripted drive through the game's own car model (the same
//! 6-speed automatic and rev range as `ed_game/src/vehicles.rs`) and the
//! same adapter the game uses (`driver::Driver`): start-up, idle, a full
//! pull through the gears, lift-off, cruise, braking to a stop.
//! `--flyby`: the same car passes the listener at speed, 8 m away, through
//! the game's mixer (distance gain, pan, air absorption).
//! `--controls`: replay a kit `render.mjs` controls file instead (what the
//! parity tests do).

use ed_engine_audio::controls::Controls;
use ed_engine_audio::driver::{DriveInput, Driver, Events, GameEngine};
use ed_engine_audio::mixer::{self, spatialize, Command, DistanceModel};
use ed_engine_audio::{car, car_index, catalog, wav, EngineVoice, SAMPLE_RATE};
use std::path::PathBuf;

// The game's car (vehicles.rs).
const GEARS: [f64; 6] = [3.6, 2.2, 1.5, 1.15, 0.92, 0.76];
const FINAL_DRIVE: f64 = 3.7;
const WHEEL_R: f64 = 0.34;
const MASS: f64 = 1400.0;

/// The game car's longitudinal model, reduced to one dimension.
struct GameCar {
    v: f64,
    gear: usize,
    rpm: f64,
}

impl GameCar {
    fn step(&mut self, dt: f64, thr: f64, brk: f64) {
        let wheel_rps = self.v.abs() / (std::f64::consts::TAU * WHEEL_R);
        self.rpm = (wheel_rps * 60.0 * GEARS[self.gear] * FINAL_DRIVE).max(850.0);
        if self.rpm > 6300.0 && self.gear < GEARS.len() - 1 {
            self.gear += 1;
        } else if self.rpm < 2200.0 && self.gear > 0 {
            self.gear -= 1;
        }
        let x = (self.rpm - 4500.0) / 3500.0;
        let torque = 420.0 * (1.0 - 0.6 * x * x).max(0.2);
        let drive = torque * GEARS[self.gear] * FINAL_DRIVE / WHEEL_R * thr;
        let mut f = drive - 0.5 * 1.2 * 0.7 * self.v * self.v - 32.0 * self.v;
        f -= brk * 0.9 * MASS * 9.81 * self.v.signum();
        self.v = (self.v + f / MASS * dt).max(0.0);
    }
    fn shaft(&self) -> f64 {
        self.v / (std::f64::consts::TAU * WHEEL_R) * 60.0 * FINAL_DRIVE
    }
}

/// (throttle, brake) over time.
fn script(t: f64) -> (f64, f64) {
    match t {
        t if t < 3.5 => (0.0, 0.0),
        t if t < 13.0 => (1.0, 0.0),
        t if t < 16.0 => (0.0, 0.0),
        t if t < 22.0 => (0.3, 0.0),
        t if t < 26.0 => (0.0, 0.4),
        _ => (0.0, 0.0),
    }
}
const LEN: f64 = 28.0;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--list") || args.len() < 2 {
        for c in &catalog().cars {
            println!("{:10} {:24} {}", c.id, c.name, c.tagline);
        }
        if args.len() < 2 {
            eprintln!("\nusage: render <car id> <out.wav> [--flyby | --controls <file>]");
        }
        return;
    }
    let id = &args[0];
    let out = PathBuf::from(&args[1]);
    let c = car(id).unwrap_or_else(|| panic!("no car '{id}' (try --list)"));
    let sr = SAMPLE_RATE as f64;

    if let Some(i) = args.iter().position(|a| a == "--controls") {
        let ctl = Controls::parse(&std::fs::read_to_string(&args[i + 1]).unwrap()).unwrap();
        let (l, r) = ctl.render(c, None);
        wav::write(&out, ctl.sample_rate, &l, Some(&r)).unwrap();
        println!("replayed {} frames -> {}", ctl.frames.len(), out.display());
        return;
    }

    let flyby = args.iter().any(|a| a == "--flyby");
    let chunk = (sr / 60.0) as usize; // the game updates at ~60 Hz
    let n = (LEN * sr) as usize;
    let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
    let mut game = GameCar { v: 0.0, gear: 0, rpm: 850.0 };
    let mut drv = Driver::new(c, GameEngine::default());
    let turbine = c.engine.mode.is_some();
    let mut ev = Events::default();
    let t0 = std::time::Instant::now();

    if flyby {
        // Through the mixer: already running, cruising past at ~25-30 m/s.
        let (h, mut mx) = mixer::channel(SAMPLE_RATE);
        drv.start_running();
        h.send(Command::Start { slot: 0, car: car_index(id).unwrap() as u16, seed: 7 });
        game.v = 28.0;
        game.gear = 3;
        let model = DistanceModel::default();
        let mut x = -LEN * 0.5 * 28.0;
        let mut i = 0;
        while i < n {
            let dt = chunk as f64 / sr;
            let thr = 0.55;
            game.step(dt, thr, 0.0);
            x += game.v * dt;
            let p = drv.update(
                &DriveInput { dt, throttle: thr, speed: game.v, engine_rpm: game.rpm, gear: game.gear as i32, shaft_rpm: game.shaft(), horn: 0.0 },
                &mut ev,
            );
            for (e, a) in ev.iter() {
                h.send(Command::Event { slot: 0, event: e, amt: a as f32 });
            }
            for &a in ev.clicks() {
                h.send(Command::Click { slot: 0, amt: a as f32 });
            }
            let sp = spatialize([x as f32, 0.0, -8.0], [0.0; 3], [1.0, 0.0, 0.0], &model);
            h.send(Command::Update { slot: 0, params: p, spatial: sp });
            let m = chunk.min(n - i);
            mx.render(&mut l[i..i + m], &mut r[i..i + m]);
            i += m;
        }
    } else {
        let mut voice = EngineVoice::new(sr, c, 7);
        voice.set_user(0.75, 0.5);
        drv.start();
        let mut i = 0;
        while i < n {
            let t = i as f64 / sr;
            let dt = chunk as f64 / sr;
            let (thr, brk) = script(t);
            if turbine {
                // Turbines: an airspeed that follows thrust (the game's plane).
                game.v = (game.v + (thr * 12.0 - 0.0008 * game.v * game.v - brk * 8.0) * dt).max(0.0);
            } else {
                game.step(dt, thr, brk);
            }
            let p = drv.update(
                &DriveInput { dt, throttle: thr, speed: game.v, engine_rpm: game.rpm, gear: game.gear as i32, shaft_rpm: game.shaft(), horn: 0.0 },
                &mut ev,
            );
            for &a in ev.clicks() {
                voice.click(a);
            }
            for (e, a) in ev.iter() {
                voice.event(e, a);
            }
            voice.set_params(&p);
            let m = chunk.min(n - i);
            voice.render(&mut l[i..i + m], &mut r[i..i + m]);
            i += m;
        }
    }
    let cpu = t0.elapsed().as_secs_f64() / LEN;
    wav::write(&out, SAMPLE_RATE, &l, Some(&r)).unwrap();
    println!("{} ({}) -> {}  [{:.2}% of one core]", c.name, if flyby { "flyby" } else { "drive" }, out.display(), cpu * 100.0);
}
