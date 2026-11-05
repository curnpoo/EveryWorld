//! Event and call recipes: data tables plus small builders that turn an
//! `Event` (or an ambient call scheduled by a bed) into `Part`s.

use crate::dsp::{Mode, Rng};
use crate::params::{Event, Params, SOUND_SPEED, Surface};
use crate::parts::{Bus, Part, Src};

/// How one surface sounds under a foot (`docs/plans/sound.md` §1 table).
#[derive(Clone, Copy, Debug)]
pub struct StepRecipe {
    /// Low body thud (Hz, gain).
    pub thud_hz: f32,
    pub thud: f32,
    /// Main noise burst: filter, centre, Q, decay, gain, attack.
    pub mode: Mode,
    pub fc: f32,
    pub q: f32,
    pub decay: f32,
    pub noise: f32,
    pub attack: f32,
    /// Crunch grains: count, centre, spread (s), gain.
    pub grains: u8,
    pub grain_fc: f32,
    pub spread: f32,
    pub grain: f32,
    /// Tonal ring (rock knock, wood boom, metal clang): Hz, gain, decay.
    pub ring_hz: f32,
    pub ring: f32,
    pub ring_decay: f32,
    /// Lift-off sound after the plant (sand trickle, mud suck, drips): gain.
    pub tail: f32,
    /// How loud the metal heel clank is on this surface.
    pub heel: f32,
}

const fn step(
    thud_hz: f32,
    thud: f32,
    mode: Mode,
    fc: f32,
    q: f32,
    decay: f32,
    noise: f32,
    attack: f32,
    grains: u8,
    grain_fc: f32,
    spread: f32,
    grain: f32,
    ring_hz: f32,
    ring: f32,
    ring_decay: f32,
    tail: f32,
    heel: f32,
) -> StepRecipe {
    StepRecipe { thud_hz, thud, mode, fc, q, decay, noise, attack, grains, grain_fc, spread, grain, ring_hz, ring, ring_decay, tail, heel }
}

/// One row per `Surface`, same order.
pub const STEPS: [StepRecipe; 16] = [
    //    thud        noise                                       grains                      ring              tail heel
    step(90.0, 0.25, Mode::Bp, 2_500.0, 0.8, 0.06, 0.22, 0.015, 0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.5), // Grass: swish + muffled thud
    step(100.0, 0.2, Mode::Bp, 4_200.0, 1.0, 0.04, 0.18, 0.006, 8, 5_000.0, 0.06, 0.12, 0.0, 0.0, 0.0, 0.0, 0.6), // DryGrass: brittle crackle
    step(70.0, 0.14, Mode::Lp, 600.0, 0.7, 0.08, 0.07, 0.02, 0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.25), // Moss: spongy press
    step(85.0, 0.25, Mode::Bp, 3_000.0, 0.8, 0.05, 0.14, 0.004, 10, 3_500.0, 0.09, 0.15, 0.0, 0.0, 0.0, 0.0, 0.5), // ForestFloor: leaves + twigs
    step(80.0, 0.4, Mode::Lp, 1_500.0, 0.7, 0.03, 0.2, 0.002, 3, 2_500.0, 0.03, 0.08, 0.0, 0.0, 0.0, 0.0, 0.7), // Dirt: dull gritty thump
    step(75.0, 0.3, Mode::Lp, 1_100.0, 0.7, 0.05, 0.18, 0.004, 5, 1_800.0, 0.07, 0.1, 0.0, 0.0, 0.0, 0.08, 0.6), // Farmland: clods + crop rustle
    step(60.0, 0.15, Mode::Bp, 1_800.0, 0.6, 0.12, 0.22, 0.025, 0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.12, 0.35), // Sand: "hff" + trickle
    step(90.0, 0.3, Mode::Bp, 2_200.0, 0.7, 0.07, 0.3, 0.002, 16, 3_000.0, 0.1, 0.2, 0.0, 0.0, 0.0, 0.0, 0.8), // Gravel: wide crunch, rattle
    step(140.0, 0.35, Mode::Hp, 2_500.0, 0.7, 0.015, 0.3, 0.0008, 0, 0.0, 0.0, 0.0, 1_900.0, 0.12, 0.05, 0.0, 1.2), // Rock: bright knock + ring
    step(70.0, 0.2, Mode::Lp, 900.0, 0.7, 0.1, 0.2, 0.02, 0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.4), // Snow (warm slush; cold squeak added in code)
    step(120.0, 0.35, Mode::Hp, 2_000.0, 0.7, 0.012, 0.35, 0.0008, 0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0), // Paving: clean tap
    step(60.0, 0.3, Mode::Lp, 500.0, 0.7, 0.09, 0.25, 0.01, 0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.2, 0.3), // Mud: squelch + suck
    step(80.0, 0.15, Mode::Bp, 1_200.0, 0.5, 0.1, 0.4, 0.004, 0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.15, 0.2), // Water: splash + drips
    step(110.0, 0.2, Mode::Bp, 800.0, 2.0, 0.04, 0.15, 0.001, 0, 0.0, 0.0, 0.0, 110.0, 0.35, 0.12, 0.0, 0.8), // Wood deck: hollow boom
    step(150.0, 0.15, Mode::Bp, 2_000.0, 1.5, 0.02, 0.15, 0.0008, 0, 0.0, 0.0, 0.0, 600.0, 0.3, 0.35, 0.0, 1.0), // Metal: clang
    step(45.0, 0.35, Mode::Lp, 200.0, 0.7, 0.05, 0.0, 0.002, 0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0), // Moon: felt, not heard
];

pub fn recipe(s: Surface) -> &'static StepRecipe {
    &STEPS[s as usize]
}

/// A cloud of tiny noise clicks (crunch, debris, crackle, drips).
pub fn grains(out: &mut Vec<Part>, rng: &mut Rng, n: usize, start: f32, spread: f32, fc: f32, gain: f32, pan: f32, bus: Bus) {
    for _ in 0..n {
        let f = fc * rng.range(0.7, 1.4);
        out.push(
            Part::noise(Mode::Bp, f, 1.2)
                .env(0.0005, rng.range(0.003, 0.012), 0.03)
                .at(start + rng.f() * spread)
                .gain(gain * rng.range(0.4, 1.0))
                .pan(pan + rng.bi() * 0.3)
                .bus(bus),
        );
    }
}

/// The robot under every step: metal heel clank, servo whine, hydraulic tss.
fn robot_layer(out: &mut Vec<Part>, rng: &mut Rng, force: f32, pan: f32, heel: f32, sprint: bool) {
    let f = rng.range(1_050.0, 1_250.0);
    out.push(Part::tone(Src::Sine, f).partials([2.71, 5.13, 0.0]).env(0.0005, 0.035, 0.12).gain(0.1 * heel * force).pan(pan));
    let (a, b) = if sprint { (1_100.0, 1_650.0) } else { (700.0, 1_050.0) };
    out.push(Part::tone(Src::Saw, a).sweep(b, 0.05).lowpass(2_800.0).env(0.012, 0.06, 0.14).gain(0.035 * force).pan(pan * 0.5));
    out.push(Part::noise(Mode::Hp, 5_000.0, 0.7).env(0.004, 0.04, 0.1).at(0.06).gain(0.03 * force).pan(pan * 0.5));
}

/// A footstep: the surface sound plus the robot layer.
pub fn footstep(out: &mut Vec<Part>, rng: &mut Rng, s: Surface, force: f32, pan: f32, sprint: bool, temp_c: f32) {
    let r = recipe(s);
    let f = force.clamp(0.05, 1.5);
    out.push(Part::tone(Src::Sine, r.thud_hz * 1.6).sweep(r.thud_hz, 0.015).env(0.001, 0.04, 0.12).gain(r.thud * f).pan(pan * 0.3));
    if r.noise > 0.0 {
        let mut n = Part::noise(r.mode, r.fc * rng.range(0.85, 1.15), r.q).env(r.attack, r.decay, r.decay * 5.0 + r.attack).gain(r.noise * f).pan(pan * 0.4);
        if s == Surface::Mud {
            n = n.am(38.0, 0.6);
        }
        out.push(n);
    }
    if r.grains > 0 {
        let n = ((r.grains as f32) * (0.6 + 0.6 * f)) as usize;
        grains(out, rng, n, 0.003, r.spread, r.grain_fc, r.grain * f, pan * 0.4, Bus::Body);
    }
    if r.ring > 0.0 {
        out.push(Part::tone(Src::Sine, r.ring_hz * rng.range(0.95, 1.05)).partials([2.4, 3.9, 0.0]).env(0.0008, r.ring_decay, r.ring_decay * 5.0).gain(r.ring * f).pan(pan * 0.4));
    }
    match s {
        Surface::ForestFloor if rng.f() < 0.08 => {
            // a stick snaps now and then
            out.push(Part::noise(Mode::Bp, 1_800.0, 3.0).env(0.0005, 0.006, 0.03).at(0.02).gain(0.5 * f).pan(pan));
        }
        Surface::Sand => grains(out, rng, 6, 0.15, 0.2, 6_000.0, r.tail * f, pan * 0.4, Bus::Body),
        Surface::Mud => out.push(Part::noise(Mode::Bp, 300.0, 3.0).sweep(900.0, 0.08).env(0.02, 0.06, 0.15).at(0.18).gain(r.tail * f).pan(pan * 0.4)),
        Surface::Water => {
            for i in 0..4 {
                out.push(Part::tone(Src::Sine, rng.range(1_800.0, 3_800.0)).sweep(rng.range(2_500.0, 5_000.0), 0.01).env(0.001, 0.012, 0.04).at(0.15 + i as f32 * rng.range(0.05, 0.12)).gain(r.tail * f).pan(pan + rng.bi() * 0.3));
            }
        }
        Surface::Farmland => out.push(Part::noise(Mode::Bp, 3_500.0, 0.8).env(0.04, 0.08, 0.25).at(0.1).gain(r.tail * f).pan(pan)),
        Surface::Snow if temp_c < -5.0 => {
            // cold snow squeaks
            out.push(Part::tone(Src::Saw, rng.range(1_300.0, 1_700.0)).sweep(rng.range(700.0, 950.0), 0.05).lowpass(4_000.0).am(90.0, 0.5).env(0.01, 0.05, 0.12).gain(0.08 * f).pan(pan * 0.4));
        }
        _ => {}
    }
    robot_layer(out, rng, f, pan, r.heel, sprint);
}

/// Expand an event into parts.
pub fn expand(ev: Event, p: &Params, rng: &mut Rng, out: &mut Vec<Part>) {
    match ev {
        Event::Footstep { surface, force, pan, sprint } => footstep(out, rng, surface, force, pan, sprint, p.temp_c),
        Event::Land { surface, impact } => {
            let g = (impact / 10.0).clamp(0.15, 1.6);
            if impact < 3.0 {
                footstep(out, rng, surface, 0.6 * g + 0.3, -0.2, false, p.temp_c);
                footstep(out, rng, surface, 0.6 * g + 0.3, 0.2, false, p.temp_c);
                return;
            }
            footstep(out, rng, surface, 1.0 + g * 0.3, -0.25, false, p.temp_c);
            footstep(out, rng, surface, 1.0 + g * 0.3, 0.25, false, p.temp_c);
            let r = recipe(surface);
            if r.grains > 0 || matches!(surface, Surface::Sand | Surface::Water | Surface::Snow | Surface::Dirt) {
                grains(out, rng, (6.0 + 14.0 * g) as usize, 0.01, 0.15 + 0.3 * g, r.grain_fc.max(1_500.0), 0.15 * g, 0.0, Bus::Body);
            }
            // servo grunt
            out.push(Part::tone(Src::Saw, 320.0).sweep(170.0, 0.12).lowpass(1_500.0).env(0.01, 0.15, 0.4).gain(0.08 * g));
            if impact >= 9.0 {
                // slam: deep body boom + metallic strain groan
                out.push(Part::brown(130.0).env(0.002, 0.35, 1.2).gain(0.9 * g));
                out.push(Part::tone(Src::Sine, 70.0).sweep(38.0, 0.2).env(0.002, 0.3, 0.9).gain(0.6 * g));
                out.push(Part::tone(Src::Saw, 95.0).sweep(62.0, 0.6).lowpass(900.0).am(7.0, 0.5).env(0.15, 0.6, 1.6).at(0.1).gain(0.1 * g));
            }
        }
        Event::Launch { surface, charge } => {
            let c = charge.clamp(0.0, 1.0);
            out.push(Part::brown(160.0).env(0.002, 0.09, 0.4).gain(0.25 + 0.5 * c));
            out.push(Part::noise(Mode::Hp, 3_000.0, 0.7).env(0.003, 0.12, 0.35).gain(0.08 + 0.12 * c));
            let r = recipe(surface);
            if r.grains > 0 {
                grains(out, rng, (4.0 + 10.0 * c) as usize, 0.0, 0.12, r.grain_fc, 0.12, 0.0, Bus::Body);
            } else if r.noise > 0.0 {
                out.push(Part::noise(r.mode, r.fc, r.q).env(0.004, r.decay * 1.5, 0.3).gain(r.noise * (0.5 + c)));
            }
        }
        Event::ChargeFull => {
            for i in 0..3 {
                out.push(Part::noise(Mode::Hp, 4_000.0, 0.8).env(0.0003, 0.004, 0.02).at(i as f32 * 0.025).gain(0.25));
            }
        }
        Event::Ignite { boot } => {
            let pan = if boot == 0 { -0.45 } else { 0.45 };
            let d = boot as f32 * 0.045;
            out.push(Part::noise(Mode::Bp, 900.0, 0.8).env(0.0005, 0.03, 0.15).at(d).gain(0.7).pan(pan).bus(Bus::Air));
            out.push(Part::brown(220.0).env(0.001, 0.06, 0.25).at(d).gain(0.5).pan(pan).bus(Bus::Air));
            grains(out, rng, 8, d + 0.01, 0.12, 3_500.0, 0.25, pan, Bus::Air);
            out.push(Part::tone(Src::Sine, 60.0).env(0.001, 0.05, 0.15).at(d).gain(0.15).pan(pan)); // felt thunk
        }
        Event::BoostKick { stage } => {
            let g = 0.55 + 0.1 * stage.min(4) as f32;
            out.push(Part::tone(Src::Sine, 58.0).sweep(32.0, 0.1).env(0.002, 0.18, 0.6).gain(0.7 * g));
            out.push(Part::brown(110.0).env(0.002, 0.12, 0.5).gain(0.6 * g).bus(Bus::Air));
            out.push(Part::noise(Mode::Lp, 600.0, 0.7).env(0.001, 0.05, 0.2).gain(0.35 * g).bus(Bus::Air));
        }
        Event::Cutoff { burn_s } => {
            out.push(Part::noise(Mode::Bp, 700.0, 0.8).sweep(280.0, 0.2).env(0.005, 0.15, 0.6).gain(0.3).bus(Bus::Air));
            if burn_s > 3.0 {
                let n = ((burn_s / 2.0) as usize).min(10);
                let span = (burn_s * 0.4).clamp(1.5, 5.0);
                for _ in 0..n {
                    out.push(Part::tone(Src::Sine, rng.range(3_000.0, 6_200.0)).partials([2.32, 4.25, 0.0]).env(0.0005, 0.04, 0.2).at(rng.range(0.4, span)).gain(rng.range(0.02, 0.05)).pan(rng.bi() * 0.4));
                }
            }
        }
        Event::ChuteOpen => {
            out.push(Part::noise(Mode::Bp, 2_500.0, 0.7).env(0.02, 0.15, 0.35).gain(0.2).bus(Bus::Air));
            out.push(Part::noise(Mode::Bp, 3_000.0, 1.5).sweep(6_000.0, 0.2).am(60.0, 0.9).env(0.01, 0.2, 0.25).at(0.3).gain(0.15).bus(Bus::Air));
            out.push(Part::noise(Mode::Hp, 1_500.0, 0.7).env(0.0005, 0.02, 0.15).at(0.55).gain(1.0).bus(Bus::Air));
            out.push(Part::brown(180.0).env(0.002, 0.25, 0.9).at(0.55).gain(0.9).bus(Bus::Air));
            out.push(Part::tone(Src::Saw, 180.0).sweep(115.0, 0.25).lowpass(1_200.0).am(25.0, 0.7).env(0.02, 0.3, 0.8).at(0.62).gain(0.12));
        }
        Event::SuitOpen => {
            out.push(Part::brown(250.0).env(0.01, 0.15, 0.5).gain(0.6).bus(Bus::Air));
            out.push(Part::noise(Mode::Bp, 1_500.0, 0.6).env(0.03, 0.12, 0.4).gain(0.35).bus(Bus::Air));
            out.push(Part::tone(Src::Sine, 880.0).env(0.003, 0.08, 0.2).gain(0.06).bus(Bus::Ui));
        }
        Event::SuitClose => {
            out.push(Part::noise(Mode::Bp, 1_100.0, 0.6).env(0.03, 0.1, 0.35).gain(0.25).bus(Bus::Air));
            out.push(Part::tone(Src::Sine, 660.0).env(0.003, 0.08, 0.2).gain(0.06).bus(Bus::Ui));
        }
        Event::Whoosh { speed, pan, size } => {
            // Doppler: the pitch falls as it passes; louder and brighter the faster you are.
            let s = crate::dsp::smoothstep(8.0, 60.0, speed);
            let hi = 700.0 * (1.0 + speed / 120.0);
            out.push(Part::noise(Mode::Bp, hi, 1.1).sweep(hi * 0.45, 0.12).env(0.05, 0.1, 0.45).gain(0.55 * s * size.clamp(0.2, 1.0)).pan_sweep(pan * 0.3, pan).bus(Bus::Air));
            out.push(Part::brown(200.0).env(0.03, 0.1, 0.35).gain(0.25 * s * size).pan(pan).bus(Bus::Air));
        }
        Event::Thunder { distance_m } => thunder(out, rng, distance_m),
        Event::Thump { strength, pan } => {
            let s = strength.clamp(0.0, 1.0);
            out.push(Part::brown(120.0).env(0.001, 0.08, 0.3).gain(0.6 * s).pan(pan).bus(Bus::Air));
            out.push(Part::noise(Mode::Bp, 400.0, 1.0).env(0.0008, 0.04, 0.15).gain(0.25 * s).pan(pan).bus(Bus::Air));
        }
        Event::ModeChirp { mode } => {
            let (a, b) = match mode {
                0 => (660.0, 520.0),
                1 => (520.0, 880.0),
                2 => (880.0, 660.0),
                3 => (440.0, 660.0),
                _ => (990.0, 1_320.0),
            };
            out.push(Part::tone(Src::Sine, a).sweep(b, 0.03).env(0.003, 0.06, 0.16).gain(0.07).bus(Bus::Ui));
        }
        Event::Splash { force } => {
            let f = force.clamp(0.1, 1.5);
            out.push(Part::noise(Mode::Bp, 1_200.0, 0.5).env(0.003, 0.15, 0.6).gain(0.6 * f).bus(Bus::Air));
            out.push(Part::brown(200.0).env(0.002, 0.1, 0.4).gain(0.4 * f).bus(Bus::Air));
            for _ in 0..(4.0 + 8.0 * f) as usize {
                out.push(Part::tone(Src::Sine, rng.range(1_500.0, 3_500.0)).sweep(rng.range(2_500.0, 5_000.0), 0.01).env(0.001, 0.015, 0.04).at(rng.range(0.1, 0.8)).gain(0.05 * f).pan(rng.bi() * 0.5).bus(Bus::Air));
            }
        }
        Event::Bump { what, force } => {
            let f = force.clamp(0.1, 1.5);
            match what {
                0 => {
                    out.push(Part::tone(Src::Sine, 180.0).sweep(140.0, 0.02).env(0.0005, 0.06, 0.2).gain(0.5 * f));
                    out.push(Part::noise(Mode::Bp, 900.0, 2.0).env(0.0005, 0.03, 0.1).gain(0.3 * f));
                }
                1 => {
                    out.push(Part::brown(200.0).env(0.0008, 0.05, 0.2).gain(0.6 * f));
                    out.push(Part::noise(Mode::Lp, 1_200.0, 0.7).env(0.0005, 0.02, 0.08).gain(0.3 * f));
                }
                2 => {
                    out.push(Part::noise(Mode::Hp, 2_000.0, 0.7).env(0.0003, 0.015, 0.06).gain(0.5 * f));
                    out.push(Part::tone(Src::Sine, 300.0).env(0.0005, 0.04, 0.15).gain(0.3 * f));
                }
                _ => {
                    out.push(Part::tone(Src::Sine, 420.0).partials([2.4, 4.1, 6.3]).env(0.0005, 0.3, 1.0).gain(0.3 * f));
                    out.push(Part::noise(Mode::Bp, 2_000.0, 1.0).env(0.0005, 0.02, 0.08).gain(0.3 * f));
                }
            }
        }
    }
}

/// Lightning: a crack if close, then a rolling rumble, all arriving late at 343 m/s
/// (about 3 s per km), duller and longer the further away.
pub fn thunder(out: &mut Vec<Part>, rng: &mut Rng, d: f32) {
    let delay = d / SOUND_SPEED;
    let near = (1.0 - d / 1_000.0).max(0.0);
    if near > 0.0 {
        out.push(Part::noise(Mode::Hp, 2_000.0, 0.7).env(0.0005, 0.05, 0.3).at(delay).gain(1.0 * near).bus(Bus::Amb));
        let pn = rng.bi() * 0.3;
        grains(out, rng, 10, delay, 0.25, 3_000.0, 0.4 * near, pn, Bus::Amb);
    }
    let lp = 60.0 + 420.0 * (-d / 3_000.0).exp();
    let decay = (2.0 + d / 2_000.0).min(6.0);
    let g = (1.2 * 1_000.0 / (d + 1_000.0)).max(0.08);
    let pan = rng.bi() * 0.6;
    out.push(Part::brown(lp).env(0.3, decay, decay * 2.5).am(1.3, 0.5).at(delay).gain(g).pan(pan).bus(Bus::Amb));
    out.push(Part::brown(lp * 0.8).env(0.6, decay * 0.8, decay * 2.0).am(0.7, 0.6).at(delay + rng.range(0.5, 2.0)).gain(g * 0.7).pan(-pan).bus(Bus::Amb));
}

// ---- ambient calls, scheduled by beds ----

fn far(rng: &mut Rng) -> (f32, f32) {
    let g = rng.range(0.2, 1.0);
    (g, 2_500.0 + 9_000.0 * g)
}

/// A songbird phrase: 3-8 swept sine notes, 2-6 kHz.
pub fn bird_phrase(out: &mut Vec<Part>, rng: &mut Rng) {
    let (g, lp) = far(rng);
    let pan = rng.bi() * 0.9;
    let base = rng.range(2_200.0, 4_800.0);
    let n = 3 + (rng.f() * 6.0) as usize;
    let mut t = 0.0;
    for _ in 0..n {
        let f0 = base * rng.range(0.8, 1.3);
        let f1 = f0 * rng.range(0.7, 1.5);
        let len = rng.range(0.04, 0.12);
        let mut p = Part::tone(Src::Sine, f0).sweep(f1, len * 0.4).env(0.005, len * 0.6, len).at(t).gain(0.05 * g).pan(pan).lowpass(lp).bus(Bus::Amb);
        if rng.f() < 0.3 {
            p = p.am(rng.range(25.0, 60.0), 0.6); // trill
        }
        out.push(p);
        t += len + rng.range(0.02, 0.09);
    }
}

/// "hoo ... hoo-hoo" (or a scops owl's single beep in warm biomes).
pub fn owl(out: &mut Vec<Part>, rng: &mut Rng, warm: bool) {
    let (g, lp) = far(rng);
    let pan = rng.bi() * 0.8;
    if warm {
        out.push(Part::tone(Src::Sine, 1_200.0).env(0.01, 0.08, 0.16).gain(0.05 * g).pan(pan).lowpass(lp).bus(Bus::Amb));
        return;
    }
    let f = rng.range(340.0, 420.0);
    for (i, t) in [0.0, 0.7, 0.95].iter().enumerate() {
        let len = if i == 0 { 0.45 } else { 0.22 };
        out.push(Part::tone(Src::Sine, f).sweep(f * 0.94, len).env(0.05, len * 0.6, len).at(*t).gain(0.07 * g).pan(pan).lowpass(lp).bus(Bus::Amb));
    }
}

pub fn frog(out: &mut Vec<Part>, rng: &mut Rng) {
    let (g, lp) = far(rng);
    let pan = rng.bi() * 0.9;
    let f = rng.range(260.0, 520.0);
    let n = 1 + (rng.f() * 3.0) as usize;
    for i in 0..n {
        out.push(Part::tone(Src::Square, f).am(rng.range(18.0, 30.0), 1.0).lowpass(lp.min(1_400.0)).env(0.01, 0.12, 0.22).at(i as f32 * 0.3).gain(0.06 * g).pan(pan).bus(Bus::Amb));
    }
}

pub fn gull(out: &mut Vec<Part>, rng: &mut Rng) {
    let (g, lp) = far(rng);
    let pan = rng.bi() * 0.9;
    for i in 0..(1 + (rng.f() * 3.0) as usize) {
        let f = rng.range(1_500.0, 2_000.0);
        out.push(Part::tone(Src::Saw, f).sweep(f * 0.6, 0.2).lowpass(lp.min(3_200.0)).env(0.02, 0.2, 0.4).at(i as f32 * 0.45).gain(0.04 * g).pan(pan).bus(Bus::Amb));
    }
}

pub fn cowbell(out: &mut Vec<Part>, rng: &mut Rng) {
    let (g, lp) = far(rng);
    let f = rng.range(520.0, 780.0);
    out.push(Part::tone(Src::Sine, f).partials([2.76, 5.4, 8.93]).env(0.001, 0.45, 1.4).gain(0.05 * g).pan(rng.bi() * 0.9).lowpass(lp).bus(Bus::Amb));
}

pub fn ice(out: &mut Vec<Part>, rng: &mut Rng) {
    let (g, _) = far(rng);
    if rng.f() < 0.6 {
        out.push(Part::tone(Src::Saw, rng.range(45.0, 80.0)).sweep(rng.range(30.0, 50.0), 0.4).am(rng.range(12.0, 22.0), 0.8).lowpass(600.0).env(0.1, 0.5, 1.2).gain(0.12 * g).pan(rng.bi() * 0.8).bus(Bus::Amb));
    } else {
        out.push(Part::brown(300.0).env(0.0008, 0.05, 0.3).gain(0.4 * g).pan(rng.bi() * 0.8).bus(Bus::Amb));
    }
}

pub fn loon(out: &mut Vec<Part>, rng: &mut Rng) {
    let (g, lp) = far(rng);
    let f = rng.range(700.0, 800.0);
    out.push(Part::tone(Src::Sine, f).sweep(f * 1.5, 0.5).am(6.0, 0.3).env(0.2, 0.9, 2.2).gain(0.05 * g).pan(rng.bi() * 0.9).lowpass(lp).bus(Bus::Amb));
}

/// A single car passing on a minor road, panning across with a Doppler drop.
pub fn car_pass(out: &mut Vec<Part>, rng: &mut Rng) {
    let (g, lp) = far(rng);
    let dir: f32 = if rng.f() < 0.5 { -1.0 } else { 1.0 };
    out.push(Part::noise(Mode::Bp, 900.0, 0.7).sweep(480.0, 1.8).lowpass(lp).env(1.6, 1.0, 4.0).gain(0.15 * g).pan_sweep(-0.8 * dir, 0.8 * dir).bus(Bus::Amb));
}

pub fn horn(out: &mut Vec<Part>, rng: &mut Rng) {
    let (g, lp) = far(rng);
    let pan = rng.bi() * 0.8;
    let len = rng.range(0.15, 0.5);
    for f in [420.0, 520.0] {
        out.push(Part::tone(Src::Square, f).lowpass(lp.min(2_500.0)).env(0.01, 2.0, len).gain(0.025 * g).pan(pan).bus(Bus::Amb));
    }
}

pub fn clank(out: &mut Vec<Part>, rng: &mut Rng) {
    let (g, lp) = far(rng);
    out.push(Part::tone(Src::Sine, rng.range(220.0, 420.0)).partials([2.3, 3.9, 6.1]).env(0.0005, 0.2, 0.8).gain(0.06 * g).pan(rng.bi() * 0.8).lowpass(lp).bus(Bus::Amb));
}

pub fn reverse_beeps(out: &mut Vec<Part>, rng: &mut Rng) {
    let (g, lp) = far(rng);
    let pan = rng.bi() * 0.8;
    for i in 0..5 {
        out.push(Part::tone(Src::Sine, 1_000.0).env(0.005, 2.0, 0.5).at(i as f32 * 1.0).gain(0.03 * g).pan(pan).lowpass(lp).bus(Bus::Amb));
    }
}

/// A train passing: rising rumble, wheel clack at the bogie spacing, horn,
/// sometimes a flange screech. `speed` m/s.
pub fn train(out: &mut Vec<Part>, rng: &mut Rng) {
    let (g, lp) = far(rng);
    let g = g.max(0.4);
    let speed = rng.range(15.0, 40.0);
    let dir: f32 = if rng.f() < 0.5 { -1.0 } else { 1.0 };
    let len = 18.0;
    out.push(Part::brown(220.0).env(6.0, 4.0, len).gain(0.5 * g).pan_sweep(-0.7 * dir, 0.7 * dir).bus(Bus::Amb));
    out.push(Part::noise(Mode::Bp, 800.0, 0.6).lowpass(lp).env(6.0, 3.0, len * 0.9).gain(0.12 * g).pan_sweep(-0.7 * dir, 0.7 * dir).bus(Bus::Amb));
    // ~ 25 m cars, two bogies each: a clack pair every car length.
    let car_dt = 25.0 / speed;
    let mut t: f32 = 4.0;
    while t < 13.0 && out.len() < 220 {
        let k = 1.0 - ((t - 8.5) / 4.5).abs(); // loudest at closest approach
        let pn = dir * (t - 8.5) / 6.0;
        for dt in [0.0, 2.6 / speed] {
            out.push(Part::noise(Mode::Hp, 1_500.0, 0.7).env(0.0005, 0.01, 0.04).at(t + dt).gain(0.3 * g * k.max(0.1)).pan(pn).lowpass(lp).bus(Bus::Amb));
        }
        t += car_dt;
    }
    for f in [311.0, 370.0, 440.0] {
        out.push(Part::tone(Src::Square, f).lowpass(2_000.0).env(0.05, 3.0, 1.5).at(3.0).gain(0.04 * g).pan(-0.4 * dir).bus(Bus::Amb));
    }
    if rng.f() < 0.3 {
        out.push(Part::tone(Src::Sine, 3_200.0).am(9.0, 0.6).env(0.4, 2.0, 3.0).at(8.0).gain(0.03 * g).bus(Bus::Amb));
    }
}

/// Rain pinging on the robot's shell: a tiny inharmonic metal ping.
pub fn shell_ping(out: &mut Vec<Part>, rng: &mut Rng, hard: f32) {
    let f = rng.range(2_200.0, 5_600.0) * (1.0 - 0.3 * hard);
    out.push(Part::tone(Src::Sine, f).partials([2.32, 4.25, 0.0]).env(0.0003, rng.range(0.03, 0.09), 0.25).gain(rng.range(0.015, 0.045) * (1.0 + hard)).pan(rng.bi() * 0.8));
}

/// A raindrop plinking on water: a rising bubble.
pub fn plink(out: &mut Vec<Part>, rng: &mut Rng) {
    let f = rng.range(1_400.0, 3_800.0);
    out.push(Part::tone(Src::Sine, f).sweep(f * 1.6, 0.008).env(0.0005, 0.012, 0.04).gain(rng.range(0.01, 0.03)).pan(rng.bi() * 0.9).bus(Bus::Amb));
}

/// A babbling-brook bubble.
pub fn bubble(out: &mut Vec<Part>, rng: &mut Rng, g: f32) {
    let f = rng.range(300.0, 1_400.0);
    out.push(Part::tone(Src::Sine, f).sweep(f * 1.7, 0.015).env(0.001, 0.015, 0.05).gain(g * rng.range(0.3, 1.0)).pan(rng.bi() * 0.8).bus(Bus::Amb));
}

/// Palm fronds knocking, banana leaves slapping, reed stems rattling.
pub fn clack(out: &mut Vec<Part>, rng: &mut Rng, fc: f32, q: f32, g: f32) {
    out.push(Part::noise(Mode::Bp, fc * rng.range(0.8, 1.25), q).env(0.0005, 0.012, 0.05).gain(g * rng.range(0.3, 1.0)).pan(rng.bi() * 0.9).bus(Bus::Amb));
}

/// Canopy fabric flapping / wingsuit luffing.
pub fn flap(out: &mut Vec<Part>, rng: &mut Rng, g: f32) {
    out.push(Part::noise(Mode::Lp, rng.range(300.0, 600.0), 1.0).env(0.003, 0.04, 0.12).gain(g).pan(rng.bi() * 0.5).bus(Bus::Air));
}

/// Space: suit-radio telemetry blip.
pub fn blip(out: &mut Vec<Part>, rng: &mut Rng) {
    let f = if rng.f() < 0.5 { 2_400.0 } else { 1_800.0 };
    out.push(Part::tone(Src::Sine, f).env(0.002, 0.03, 0.05).gain(0.02).pan(rng.bi() * 0.2).bus(Bus::Ui));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_surface_makes_a_step() {
        let mut rng = Rng::new(1);
        for i in 0..16u8 {
            let s: Surface = unsafe { std::mem::transmute(i) };
            let mut out = Vec::new();
            footstep(&mut out, &mut rng, s, 0.6, 0.0, false, 10.0);
            assert!(out.len() >= 4, "{s:?}");
        }
    }

    #[test]
    fn harder_landings_are_bigger() {
        let mut rng = Rng::new(2);
        let p = Params::default();
        let count = |impact: f32, rng: &mut Rng| {
            let mut out = Vec::new();
            expand(Event::Land { surface: Surface::Gravel, impact }, &p, rng, &mut out);
            out.iter().map(|x| x.gain * x.len).sum::<f32>()
        };
        let soft = count(1.5, &mut rng);
        let mid = count(6.0, &mut rng);
        let hard = count(15.0, &mut rng);
        assert!(soft < mid && mid < hard, "{soft} {mid} {hard}");
    }

    #[test]
    fn thunder_arrives_late() {
        let mut rng = Rng::new(3);
        let mut out = Vec::new();
        thunder(&mut out, &mut rng, 3_430.0);
        assert!(out.iter().all(|p| p.delay >= 10.0 - 1e-3));
    }
}
