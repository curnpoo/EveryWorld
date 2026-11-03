//! Mixer timing: `cargo run --release -p ed_engine_audio --example bench`.
//! Six voices (the game's MAX_AUDIBLE) under load for 20 s of audio, with
//! voices starting, switching car and releasing along the way; reports the
//! average and worst 128-frame block against the 2.67 ms budget at 48 kHz.
use ed_engine_audio::mixer::{self, Command, Spatial, FRAMES};
use ed_engine_audio::{car_index, Event, Params};
use std::sync::atomic::Ordering::Relaxed;

fn main() {
    let ids = ["huracan", "gt3", "mustang", "f1", "chiron", "jet"];
    let (h, mut mx) = mixer::channel(48_000);
    let (mut l, mut r) = ([0f32; FRAMES], [0f32; FRAMES]);
    let mut worst = 0u128;
    let blocks = 20 * 48_000 / FRAMES;
    let t_all = std::time::Instant::now();
    for b in 0..blocks {
        let t = b as f64 * FRAMES as f64 / 48_000.0;
        if b % 400 == 0 {
            for (s, id) in ids.iter().enumerate() {
                h.send(Command::Start { slot: s as u8, car: car_index(id).unwrap() as u16, seed: s as u32 + 1 });
            }
        }
        if b % 400 == 200 {
            h.send(Command::SetCar { slot: 2, car: car_index("v24").unwrap() as u16 });
            h.send(Command::Release { slot: 5 });
            for s in 0..5 {
                h.send(Command::Event { slot: s, event: Event::Lift, amt: 1.0 });
            }
        }
        if b % 3 == 0 {
            for s in 0..6u8 {
                let rpm = 2000.0 + 3000.0 * (1.0 + (t * 0.7 + s as f64).sin());
                h.send(Command::Update { slot: s, params: Params { rpm, load: 0.8, shaft: 2000.0, ..Default::default() }, spatial: Spatial { gain: 0.5, pan: 0.3, width: 0.5, air_hz: 8000.0 } });
            }
        }
        let t0 = std::time::Instant::now();
        mx.render(&mut l, &mut r);
        worst = worst.max(t0.elapsed().as_nanos());
    }
    let per = t_all.elapsed().as_secs_f64() / blocks as f64;
    println!(
        "6 voices: avg {:.0} us/block ({:.1}% of 2.67 ms), worst {:.0} us; mixer avg {:.0} us",
        per * 1e6,
        per / (FRAMES as f64 / 48_000.0) * 100.0,
        worst as f64 / 1000.0,
        h.shared.total_ns.load(Relaxed) as f64 / h.shared.blocks.load(Relaxed) as f64 / 1000.0
    );
}
