//! Steps the 1,000-box scene and prints the final state hash and a digest of every per-tick hash.
//! Run it twice (or on two machines) and compare the stdout line.
//!
//! `cargo run -p ed_sim --release --example replay_hash -- [steps]` (default 3600 = 60 s).
//! Timing goes to stderr, is measured outside the simulation and is not part of the comparison.

use ed_sim::{DT, SimConfig, SimInput, demo};
use std::time::Instant;

fn main() {
    let steps: u32 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(3600);
    let (mut world, _) = demo::thousand_boxes(&SimConfig::default());
    let mut hashes = Vec::with_capacity(steps as usize);
    let mut step_ms = Vec::with_capacity(steps as usize);
    let mut awake = Vec::with_capacity(steps as usize);
    for _ in 0..steps {
        let tick = world.tick();
        let t0 = Instant::now();
        world.step(&SimInput { tick, commands: &[], utc: tick as f64 * DT });
        step_ms.push(t0.elapsed().as_secs_f64() * 1000.0);
        hashes.push(world.state_hash());
        awake.push(world.awake_body_count());
    }
    let last = hashes.last().copied().unwrap_or(0);
    // Must match demo::thousand_box_hashes (checked by the determinism tests).
    println!("steps={steps} final_hash={last:016x} sequence_digest={:016x}", demo::digest(&hashes));

    let window = |a: usize, b: usize| {
        let b = b.min(step_ms.len());
        if a >= b {
            return;
        }
        let s = &step_ms[a..b];
        let mean = s.iter().sum::<f64>() / s.len() as f64;
        let mut sorted = s.to_vec();
        sorted.sort_by(|x, y| x.total_cmp(y));
        let p95 = sorted[(sorted.len() * 95 / 100).min(sorted.len() - 1)];
        let max = sorted[sorted.len() - 1];
        let aw = awake[a..b].iter().sum::<usize>() / (b - a);
        eprintln!("ticks {a:>4}..{b:<4}  mean {mean:.3} ms  p95 {p95:.3} ms  max {max:.3} ms  awake bodies (mean) {aw}");
    };
    window(0, 60);
    window(0, 600);
    window(60, 300);
    window(300, 900);
    window(900, steps as usize);
    window(0, steps as usize);
}
