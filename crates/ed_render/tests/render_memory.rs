//! 300 frames must not grow device memory after warm-up. Its own test binary because
//! `allocated_bytes` is per Metal device, which every device in a process shares.
#![cfg(target_os = "macos")]

mod common;
use common::*;
use ed_render::*;

#[test]
fn three_hundred_frames_without_growth() {
    let mut rig = Rig::new();
    let mut cam = TestScene::test_camera();
    for _ in 0..30 {
        rig.frame(cam, SceneMode::Scene, false);
    }
    rig.dev.wait_idle();
    let base = rig.dev.allocated_bytes();
    let created = rig.renderer.as_ref().unwrap().transient_textures_created();
    for i in 0..300 {
        cam.yaw += 0.01;
        rig.frame(cam, SceneMode::Scene, i % 50 == 0);
    }
    rig.dev.wait_idle();
    let after = rig.dev.allocated_bytes();
    let created_after = rig.renderer.as_ref().unwrap().transient_textures_created();
    println!("allocated after warm-up {base} B, after 300 frames {after} B; transients created {created} -> {created_after}");
    assert_eq!(created, created_after, "no transient textures created after warm-up");
    assert!(after <= base, "allocated grew from {base} to {after} bytes");
    rig.destroy();
}
