//! GPU timer test on the path the device picks (counter samples on Apple silicon). Its own
//! test binary so no other test competes for the GPU while it measures.
#![cfg(target_os = "macos")]

mod common;
use common::*;
use ed_gpu::*;

#[test]
fn gpu_timer_measures_heavy_pass() {
    let dev = device();
    assert_ne!(dev.caps().timers, TimerSource::None);
    let (heavy, light) = heavy_and_light(&*dev);
    println!("timer path {:?}: heavy {heavy:.3} ms, light {light:.3} ms", dev.caps().timers);
    assert!(heavy > 0.5 && heavy < 10_000.0, "heavy pass took {heavy} ms");
    assert!(light < heavy / 4.0, "light {light} ms vs heavy {heavy} ms");
}
