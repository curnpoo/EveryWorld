//! Present smoke test without a window system: a detached `CAMetalLayer` (no `NSWindow`, no
//! winit) gets a surface, renders N frames with a resize in the middle, and is destroyed while
//! the device keeps running. macOS only (both backends present through the layer).
#![cfg(target_os = "macos")]

#[macro_use]
mod common;
use common::*;
use ed_gpu::ResourceState::*;
use ed_gpu::*;

backend_tests!(present_frames_to_detached_metal_layer);

fn present_frames_to_detached_metal_layer(dev: &dyn Device) {
    use objc2_quartz_core::CAMetalLayer;
    let layer = CAMetalLayer::new();
    let handle = WindowHandle { window: objc2::rc::Retained::as_ptr(&layer) as *mut std::ffi::c_void, display: std::ptr::null_mut() };
    let mut config = SurfaceConfig { width: 64, height: 64, format: Format::Bgra8Unorm, vsync: false, hdr: false };
    let surface = dev.create_surface(handle, &config).expect("create_surface");
    const FRAMES: u32 = 12;
    let (mut presented, mut lost) = (0, 0);
    for i in 0..FRAMES {
        if i == FRAMES / 2 {
            config.width = 96;
            config.height = 48;
            dev.configure_surface(surface, &config).expect("resize");
        }
        let mut e = dev.begin_frame().unwrap();
        let target = match dev.acquire(surface) {
            Ok(t) => t,
            Err(Error::SurfaceLost) => {
                // Allowed once per resize: reconfigure and skip the frame.
                lost += 1;
                dev.end_frame(e).unwrap();
                dev.configure_surface(surface, &config).expect("reconfigure");
                continue;
            }
            Err(err) => panic!("acquire frame {i}: {err}"),
        };
        let shade = i as f32 / FRAMES as f32;
        e.transition(&[tex(target, Undefined, RenderTarget)]);
        e.begin_render_pass(&RenderPassDesc { label: "present clear", colors: &[color(target, LoadOp::Clear([shade, 0.2, 1.0 - shade, 1.0]))], depth: None });
        e.end_render_pass();
        e.transition(&[tex(target, RenderTarget, Present)]);
        match dev.end_frame(e) {
            Ok(()) => presented += 1,
            Err(Error::SurfaceLost) => {
                lost += 1;
                dev.configure_surface(surface, &config).expect("reconfigure");
            }
            Err(err) => panic!("end_frame {i}: {err}"),
        }
    }
    println!("{:?}: presented {presented} of {FRAMES} frames, {lost} surface-lost", dev.caps().backend);
    assert!(presented >= FRAMES - 2, "presented only {presented} of {FRAMES} frames ({lost} lost)");
    let size = layer.drawableSize();
    assert_eq!((size.width as u32, size.height as u32), (96, 48), "layer drawable size follows the resize");
    dev.wait_idle();
    dev.destroy_surface(surface);
    // The device keeps working after the surface is gone.
    for _ in 0..3 {
        let e = dev.begin_frame().unwrap();
        dev.end_frame(e).unwrap();
    }
    dev.wait_idle();
}
