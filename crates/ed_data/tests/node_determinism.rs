//! Two runs and two processes give byte-identical pages; accuracy against the analytic field.

mod node_common;
use ed_data::node::{HEIGHT_STEP_M, node_heights, source_zoom};
use ed_geo::node::{BodyId, NodeId};
use node_common::*;

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3))
}

fn fixed_nodes() -> Vec<NodeId> {
    [(0u8, 0u8, 0u32, 0u32), (2, 3, 5, 2), (4, 6, 31, 32), (5, 8, 128, 127), (1, 9, 300, 17), (3, 11, 1000, 2000)]
        .iter()
        .map(|&(f, l, x, y)| NodeId::new(BodyId(0), f, l, x, y).unwrap())
        .collect()
}

fn digest() -> String {
    let src = Synth::new();
    fixed_nodes().iter().map(|&id| format!("{:016x}", fnv(&node_heights(&src, id).to_bytes()))).collect::<Vec<_>>().join(",")
}

#[test]
fn two_runs_identical() {
    eprintln!("DIGEST_ALL={}", digest());
    let src = Synth::new();
    for id in fixed_nodes() {
        let a = node_heights(&src, id);
        let b = node_heights(&Synth::new(), id);
        assert_eq!(a.to_bytes(), b.to_bytes());
        assert_eq!(a, node_heights(&src, id));
    }
}

#[test]
fn two_processes_identical() {
    if std::env::var("NODE_DIGEST_CHILD").is_ok() {
        println!("DIGEST={}", digest());
        return;
    }
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["two_processes_identical", "--exact", "--nocapture", "--test-threads=1"])
        .env("NODE_DIGEST_CHILD", "1")
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    let got = text.lines().find_map(|l| l.split_once("DIGEST=").map(|(_, d)| d.trim())).expect("child printed no digest");
    assert_eq!(got, digest());
}

#[test]
fn height_error_within_bound() {
    let src = Synth::new();
    let mut rng = Rng(77);
    let (mut worst, mut worst_bound) = (0.0f64, 0.0f64);
    for _ in 0..120 {
        let id = random_node(&mut rng, 11);
        let p = node_heights(&src, id);
        assert!(p.min_m <= p.max_m);
        // Resampling bound: bilinear error <= h^2/8 * (|f_xx| + |f_yy|) with h the pixel pitch
        // in radians at the source zoom (Mercator pitch in latitude is no larger).
        let h = 2.0 * std::f64::consts::PI / (256.0 * (1u64 << source_zoom(id.level())) as f64);
        // + 1 mm: the test's geodetic conversion (ed_geo, iterative) and ed_data's (one-step
        // Bowring, libm) differ by ~1e-9 deg, which shows up on steep synthetic slopes.
        let bound = (HEIGHT_STEP_M as f64) / 2.0 + h * h / 8.0 * 2.0 * F_PP + 1e-3;
        for _ in 0..200 {
            let (i, j) = (rng.below(257) as u32, rng.below(257) as u32);
            let (lat, lon) = lat_lon(id, i, j);
            if lat.abs() > 85.0 {
                continue; // beyond the Mercator limit the source is extended, not analytic
            }
            let err = (p.height(i as usize, j as usize) as f64 - analytic(lat, lon)).abs();
            assert!(err <= bound, "{id} ({i},{j}) err {err} > {bound}");
            if err > worst {
                worst = err;
                worst_bound = bound;
            }
        }
    }
    eprintln!("worst height error {worst:.4} m (bound there {worst_bound:.4} m; step {HEIGHT_STEP_M} m)");
}

#[test]
fn poles_are_finite_and_stable() {
    let src = Synth::new();
    for face in [4u8, 5] {
        for level in 0..=8u8 {
            let n = 1u32 << level;
            let id = NodeId::new(BodyId(0), face, level, n / 2, n / 2).unwrap();
            let p = node_heights(&src, id);
            assert!(p.min_m.is_finite() && p.max_m.is_finite() && p.min_m <= p.max_m);
            assert_eq!(p, node_heights(&src, id));
        }
    }
}
