//! M4b: the scripted character route against the baked pack collision. Runs into the nearest
//! building, then jumps onto the authored barrel; never penetrates or falls through; the replay
//! hash is identical across two runs in this process and two `ew --sim-route` processes.

mod common;
use ed_assets::collision::surface;
use ew::route::{PENETRATION_TOL, RouteReport, run};

fn layers() -> std::path::PathBuf {
    common::workspace().join("assets-src/layers")
}

fn check(r: &RouteReport) {
    print!("{}", r.lines());
    assert!(r.max_penetration <= PENETRATION_TOL, "capsule penetrated {:.4} m at tick {}", r.max_penetration, r.worst_tick);
    assert!(r.lowest_feet > -0.05, "fell through the ground: feet z {}", r.lowest_feet);
    assert_eq!(r.wall_surface, surface::BUILDING, "the wall leg ends against a building");
    assert!(r.wall_gap.abs() < 0.05, "stopped at the wall, not short of it: clearance {}", r.wall_gap);
    assert!(r.stood_on_crate, "the character jumped onto the barrel");
    assert!(r.replay_ok, "the recorded replay re-runs to the same hashes");
}

fn field<'a>(out: &'a str, key: &str) -> &'a str {
    out.lines().find_map(|l| l.strip_prefix(key)).unwrap_or_else(|| panic!("no {key} in:\n{out}")).trim()
}

#[test]
fn route_hits_the_wall_reaches_the_prop_and_replays_identically() {
    let packs = common::packs();
    let a = run(&packs, &layers()).expect("route (rebake proving.ewpack if it has no collision chunks)");
    check(&a);
    let b = run(&packs, &layers()).unwrap();
    assert_eq!((a.final_hash, &a.replay_blake3), (b.final_hash, &b.replay_blake3), "two runs, one process");

    // Two more processes.
    for _ in 0..2 {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_ew")).args(["--sim-route", "--packs"]).arg(&packs).arg("--layers").arg(layers()).output().unwrap();
        let s = String::from_utf8_lossy(&out.stdout);
        assert!(out.status.success(), "ew --sim-route failed: {}", String::from_utf8_lossy(&out.stderr));
        assert_eq!(field(&s, "final_hash:"), format!("{:016x}", a.final_hash), "process final hash");
        assert_eq!(field(&s, "replay_blake3:"), a.replay_blake3, "process replay bytes");
    }
}
