//! Builds a cluster LOD hierarchy for one building dump and prints levels/errors.
//! `cargo run --release -p ed_gpu --example hierarchy -- <building.bin>`
use ed_gpu::cluster::{build_hierarchy, hierarchy_visible_tris, projected_error_px};
use std::env;

fn load(path: &str) -> (Vec<[f32; 3]>, Vec<u32>) {
    let b = std::fs::read(path).expect("read");
    assert_eq!(&b[0..4], b"EWB1");
    let nv = u32::from_le_bytes(b[4..8].try_into().unwrap()) as usize;
    let ni = u32::from_le_bytes(b[8..12].try_into().unwrap()) as usize;
    let mut at = 12;
    let mut floats = |n: usize| -> Vec<f32> {
        let v = b[at..at + 4 * n].chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect();
        at += 4 * n;
        v
    };
    let p = floats(3 * nv);
    let _n = floats(3 * nv);
    let _c = floats(4 * nv);
    let idx: Vec<u32> = b[at..at + 4 * ni].chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect();
    let pos: Vec<[f32; 3]> = p.chunks_exact(3).map(|v| [v[0], v[1], v[2]]).collect();
    (pos, idx)
}

fn all_planes() -> [[f32; 4]; 6] {
    [[0.0, 0.0, 0.0, 1.0]; 6]
}

fn main() {
    let path = env::args().nth(1).unwrap_or_else(|| "building.bin".into());
    let (pos, idx) = load(&path);
    let hier = build_hierarchy(&pos, &idx);
    println!("levels: {}", hier.len());
    for (i, m) in hier.levels.iter().enumerate() {
        let tris: u32 = m.clusters.iter().map(|c| c.tri_count).sum();
        let err = m.clusters.first().map(|c| c.error).unwrap_or(0.0);
        let perr = m.clusters.first().map(|c| c.parent_error).unwrap_or(f32::INFINITY);
        println!("  level {}: {} clusters, {} tris, error {:.4} m, parent_error {:.4}", i, m.clusters.len(), tris, err, perr);
    }
    // visible tris at distances
    let center = [0.0f32, 4.0, 0.0];
    for dist in [50.0f32, 200.0, 800.0] {
        let cam = [center[0], center[1], center[2] + dist];
        let planes = all_planes();
        for thr in [1.0f32, 2.0] {
            let vt = hierarchy_visible_tris(&hier, &planes, cam, thr, 720.0, 50.0f32.to_radians());
            println!("  dist {:.0}m thr {}px -> {} tris", dist, thr, vt);
        }
    }
    let _ = projected_error_px;
}
