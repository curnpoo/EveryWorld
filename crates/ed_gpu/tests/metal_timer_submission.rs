//! The submission-time fallback (`ED_GPU_TIMERS=submission`), in its own process because the
//! switch is read from the environment when the device is created.
#![cfg(target_os = "macos")]

mod common;
use ed_gpu::*;

#[test]
fn submission_timer_fallback_measures_heavy_pass() {
    // SAFETY: set before any other thread of this test binary reads the environment (this
    // binary has one test).
    unsafe { std::env::set_var("ED_GPU_TIMERS", "submission") };
    let dev = common::device();
    assert_eq!(dev.caps().timers, TimerSource::Submission);
    let (heavy, light) = common::heavy_and_light(&*dev);
    println!("timer path {:?}: heavy {heavy:.3} ms, light {light:.3} ms", dev.caps().timers);
    assert!(heavy > 0.5 && heavy < 10_000.0, "heavy pass took {heavy} ms");
}
