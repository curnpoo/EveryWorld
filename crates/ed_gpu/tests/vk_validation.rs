//! Validation errors raised while a frame is recorded or submitted reach the caller: Vulkan's
//! `end_frame` returns `Err`, and panics under `ED_GPU_VK_VALIDATION=require`. Provokes one ed_gpu does not check itself: an indexed draw that reads
//! past the end of its index buffer (VUID-vkCmdDrawIndexed-robustBufferAccess2-08798).
#![cfg(feature = "vulkan")]

#[macro_use]
mod common;
use common::*;
use ed_gpu::ResourceState::*;
use ed_gpu::*;

#[test]
fn submit_time_validation_error_is_reported() {
    let Some(dev) = device(Backend::Vulkan) else {
        return;
    };
    let dev = &*dev;
    let vs = shader(
        dev,
        ShaderStage::Vertex,
        "vs_full",
        FULLSCREEN_VS,
        spv!("full.vert"),
    );
    let pso = pipeline(dev, vs, flat_fs(dev), None);
    let rt = render_target(dev, "oob index");
    let ib = dev
        .create_buffer(&BufferDesc {
            label: "4 indices",
            size: 16,
            usage: Usage {
                shader_read: true,
                copy_dst: true,
                ..usage()
            },
        })
        .unwrap();
    dev.write_buffer(ib, 0, &bytes_u32(&[0, 1, 2, 0])).unwrap();
    let mut e = dev.begin_frame().unwrap();
    e.transition(&[
        tex(rt, Undefined, RenderTarget),
        buf(ib, CopyDst, ShaderRead),
    ]);
    e.begin_render_pass(&RenderPassDesc {
        label: "oob",
        colors: &[color(rt, LoadOp::Clear([0.0; 4]))],
        depth: None,
    });
    e.set_render_pipeline(pso);
    e.set_index_buffer(ib, 0);
    e.draw_indexed(0..6, 0, 0..1); // 24 bytes of indices from a 16-byte buffer
    e.end_render_pass();
    let require = std::env::var("ED_GPU_VK_VALIDATION").as_deref() == Ok("require");
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| dev.end_frame(e)));
    match r {
        Err(p) => {
            let msg = p.downcast_ref::<String>().cloned().unwrap_or_default();
            assert!(
                require,
                "end_frame panicked without ED_GPU_VK_VALIDATION=require: {msg}"
            );
            assert!(msg.contains("Vulkan validation:"), "{msg}");
        }
        Ok(Err(err)) => {
            assert!(!require, "require mode must panic: {err}");
            assert!(err.to_string().contains("Vulkan validation:"), "{err}");
        }
        Ok(Ok(())) => {
            assert!(!require, "the wrong layout passed validation");
            eprintln!("SKIP: no validation layer, nothing reported");
        }
    }
    dev.wait_idle();
    // The next frame is clean again.
    let e = dev.begin_frame().unwrap();
    dev.end_frame(e).unwrap();
}
