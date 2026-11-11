//! 300 proving frames: device allocations do not grow after warm-up.

use ew::args::Args;
use ew::camera::PathKind;

#[test]
fn no_memory_growth_over_300_proving_frames() {
    let args = Args {
        headless: true,
        width: 640,
        height: 360,
        frames: Some(300),
        path: PathKind::Proving,
        pop: true,
        out: std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("mem"),
        ..Args::default()
    };
    let r = ew::headless::run(&args).unwrap();
    assert!(r.alloc_after_warmup > 0);
    assert_eq!(r.alloc_end, r.alloc_after_warmup, "device memory grew:\n{}", r.summary);
    assert!(r.summary.contains("total:           mean 100050"), "{}", r.summary);
}
