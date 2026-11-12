//! 300 proving frames: device allocations do not grow after warm-up.

use ew::args::Args;
use ew::camera::PathKind;

mod common;

#[test]
fn no_memory_growth_over_300_proving_frames() {
    if !common::gpu() {
        return;
    }
    let args = Args {
        headless: true,
        width: 640,
        height: 360,
        frames: Some(300),
        path: PathKind::Proving,
        pop: true,
        out: std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("mem"),
        packs: common::packs(),
        ..Args::default()
    };
    let r = ew::headless::run(&args).unwrap();
    assert!(r.alloc_after_warmup > 0);
    assert_eq!(r.alloc_end, r.alloc_after_warmup, "device memory grew:\n{}", r.summary);
    assert!(r.instances >= 100_050, "{}", r.instances);
    assert!(r.summary.contains(&format!("total:           mean {},", r.instances)), "{}", r.summary);
}
