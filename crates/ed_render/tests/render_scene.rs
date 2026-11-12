//! Offscreen GPU tests of Renderer v0 at 256x256 (Metal). Compiles to nothing off macOS.
#![cfg(target_os = "macos")]

mod common;
use common::*;
use ed_render::*;

fn is_sky(p: [u8; 4]) -> bool {
    p[2] > p[0] && p[2] > p[1] && p[2] > 120
}
fn is_red(p: [u8; 4]) -> bool {
    p[0] > 100 && p[0] as i32 > p[1] as i32 + 60 && p[0] as i32 > p[2] as i32 + 60
}
fn is_ground(p: [u8; 4]) -> bool {
    // Grey-green slab: green channel highest or close, not blue-dominated, not saturated red.
    p[1] >= p[2] && p[1] + 10 >= p[0] && p[1] > 40 && !is_red(p)
}

#[test]
fn test_scene_pixels_are_sky_ground_and_cube() {
    let Some(mut rig) = Rig::new() else { return };
    let px = rig.image(TestScene::test_camera());
    let top = at(&px, W / 2, 4);
    let centre = at(&px, W / 2, H / 2);
    let bottom = at(&px, W / 2, H - 4);
    let left_bottom = at(&px, 8, H - 8);
    assert!(is_sky(top), "top centre should be sky, got {top:?}");
    assert!(is_red(centre), "centre should be the red cube, got {centre:?}");
    assert!(is_ground(bottom), "bottom centre should be ground, got {bottom:?}");
    assert!(is_ground(left_bottom), "bottom left should be ground, got {left_bottom:?}");
    assert!(px.iter().all(|p| p[3] == 255), "output is opaque");
    rig.destroy();
}

#[test]
fn image_is_identical_across_runs() {
    let Some(mut a) = Rig::new() else { return };
    let ha = hash(&a.image(TestScene::test_camera()));
    // Same device, more frames in between, then the same view again.
    for _ in 0..5 {
        a.frame(TestScene::test_camera(), SceneMode::Scene, false);
    }
    let ha2 = hash(&a.image(TestScene::test_camera()));
    a.destroy();
    let Some(mut b) = Rig::new() else { return };
    let hb = hash(&b.image(TestScene::test_camera()));
    b.destroy();
    assert_eq!(ha, ha2, "same device, two renders");
    assert_eq!(ha, hb, "two renderers");
}

#[test]
fn camera_position_keeps_f64_precision() {
    // The scene's origin is ~6,500 km from 0, where f32 spacing is 0.5 m. A 5 cm camera move
    // must still change the image: only (origin - camera), computed in f64, reaches the GPU.
    let Some(mut rig) = Rig::new() else { return };
    let cam = TestScene::test_camera();
    let base = hash(&rig.image(cam));
    let mut moved = cam;
    moved.position.x += 0.05;
    assert_ne!(base, hash(&rig.image(moved)), "a 5 cm move must change the image");
    rig.destroy();
}

#[test]
fn per_pass_timers_return_every_declared_pass() {
    let Some(mut rig) = Rig::new() else { return };
    for _ in 0..4 {
        rig.frame(TestScene::test_camera(), SceneMode::Scene, false);
    }
    rig.dev.wait_idle();
    let t = rig.renderer.as_mut().unwrap().timings(&*rig.dev).expect("timings after wait_idle");
    let names = rig.r().pass_names(&FrameParams::default());
    for name in &names {
        assert!(t.pass_ms(name).is_some(), "no timing row for {name}: {t:?}");
    }
    assert_eq!(t.passes.len(), names.len());
    let scene = t.pass_ms("raster_late").unwrap() + t.pass_ms("raster_early").unwrap() + t.pass_ms("resolve").unwrap();
    assert!(t.frame_ms > 0.0 && t.frame_ms < 100.0, "frame {} ms", t.frame_ms);
    assert!(scene >= 0.0 && scene <= t.frame_ms + 0.01, "scene {scene} ms within frame {} ms", t.frame_ms);
    println!("timings: {t:?}");
    // Empty frames report their own passes.
    for _ in 0..4 {
        rig.frame(TestScene::test_camera(), SceneMode::Empty, false);
    }
    rig.dev.wait_idle();
    let t = rig.renderer.as_mut().unwrap().timings(&*rig.dev).unwrap();
    for name in rig.r().pass_names(&FrameParams { mode: SceneMode::Empty, ..Default::default() }) {
        assert!(t.pass_ms(name).is_some(), "empty frame: no timing row for {name}");
    }
    rig.destroy();
}
