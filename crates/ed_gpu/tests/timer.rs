//! GPU timer test on the path each device picks (counter samples on Apple silicon, timestamp
//! queries on Vulkan). Its own test binary, and the backends take turns ([`exclusive`]), so no
//! other test competes for the GPU while it measures.

#[macro_use]
mod common;
use common::*;
use ed_gpu::*;

backend_tests!(exclusive; gpu_timer_measures_heavy_pass);

fn gpu_timer_measures_heavy_pass(dev: &dyn Device) {
    assert_ne!(dev.caps().timers, TimerSource::None);
    let (heavy, light) = heavy_and_light(dev);
    println!("timer path {:?} on {:?}: heavy {heavy:.3} ms, light {light:.3} ms", dev.caps().timers, dev.caps().backend);
    assert!(heavy > 0.5 && heavy < 10_000.0, "heavy pass took {heavy} ms");
    assert!(light < heavy / 4.0, "light {light} ms vs heavy {heavy} ms");
}
