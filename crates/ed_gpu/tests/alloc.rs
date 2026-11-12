//! Allocation accounting and deferred destroy. Its own test binary, and the tests hold
//! [`exclusive`] while they run, because `allocated_bytes` on Metal is per `MTLDevice`, which
//! every device in the process shares (MoltenVK's included).

#[macro_use]
mod common;
use common::*;
use ed_gpu::ResourceState::*;
use ed_gpu::*;

backend_tests!(exclusive; churn_10k_buffers_and_textures_without_growth, deferred_destroy_keeps_in_flight_resources_alive);

fn churn_10k_buffers_and_textures_without_growth(dev: &dyn Device) {
    let churn = |n: u32| {
        for i in 0..n {
            let b = dev.create_buffer(&BufferDesc { label: "churn buffer", size: 64 * 1024, usage: Usage { shader_read: true, copy_dst: true, ..usage() } }).unwrap();
            let t = dev
                .create_texture(&TextureDesc {
                    label: "churn texture",
                    kind: TextureKind::D2,
                    format: Format::Rgba8Unorm,
                    width: 64,
                    height: 64,
                    depth_or_layers: 1,
                    mips: 7,
                    samples: 1,
                    usage: Usage { shader_read: true, copy_dst: true, ..usage() },
                })
                .unwrap();
            // Every tenth pair also goes through the bindless tables.
            if i % 10 == 0 {
                let (bb, tb) = (dev.bind_buffer(b).unwrap(), dev.bind_texture(t).unwrap());
                dev.unbind_buffer(bb);
                dev.unbind_texture(tb);
            }
            dev.destroy_buffer(b);
            dev.destroy_texture(t);
            // Frames keep running, so deferred frees retire as they would in the game.
            if i % 100 == 0 {
                let e = dev.begin_frame().unwrap();
                dev.end_frame(e).unwrap();
            }
        }
        for _ in 0..4 {
            let e = dev.begin_frame().unwrap();
            dev.end_frame(e).unwrap();
        }
        dev.wait_idle();
    };
    churn(200); // warm-up: tables, fences, residency set, heaps settle
    let base = dev.allocated_bytes();
    // The measure must see allocations at all: 16 live 64 KiB buffers show up.
    let live: Vec<_> = (0..16).map(|_| dev.create_buffer(&BufferDesc { label: "live", size: 64 * 1024, usage: Usage { shader_read: true, ..usage() } }).unwrap()).collect();
    let with_live = dev.allocated_bytes();
    assert!(with_live >= base + 16 * 64 * 1024, "allocated_bytes did not see 1 MiB of buffers ({base} -> {with_live})");
    live.into_iter().for_each(|b| dev.destroy_buffer(b));
    churn(10_000);
    let after = dev.allocated_bytes();
    println!("allocated before {base} B, after 10k pairs {after} B");
    // One pair is ~80 KiB; 10k leaked pairs would be ~800 MB. Allow 1 MiB of noise.
    assert!(after <= base + (1 << 20), "allocated grew from {base} to {after} bytes");
}

/// Resources destroyed while a frame that uses them is in flight stay alive until that frame
/// completes; their handles are dead at once; the memory comes back afterwards.
fn deferred_destroy_keeps_in_flight_resources_alive(dev: &dyn Device) {
    const N: u32 = 1 << 16;
    let cs = shader(dev, ShaderStage::Compute, "fill", FILL_MSL, spv!("fill.comp"));
    let pso = compute_pipeline(dev, cs, 64);
    let rb = readback_buffer(dev, N as u64 * 4);
    // Settle the device so allocated_bytes is stable before the measured resources exist.
    for _ in 0..3 {
        let e = dev.begin_frame().unwrap();
        dev.end_frame(e).unwrap();
    }
    dev.wait_idle();
    let base = dev.allocated_bytes();
    let big = 8u64 << 20;
    let victim = dev.create_buffer(&BufferDesc { label: "victim", size: big, usage: Usage { shader_write: true, copy_src: true, ..usage() } }).unwrap();
    let vt = dev
        .create_texture(&TextureDesc {
            label: "victim texture",
            kind: TextureKind::D2,
            format: Format::Rgba8Unorm,
            width: 1024,
            height: 1024,
            depth_or_layers: 1,
            mips: 1,
            samples: 1,
            usage: Usage { render_target: true, ..usage() },
        })
        .unwrap();
    let vi = dev.bind_buffer(victim).unwrap();
    assert!(dev.allocated_bytes() >= base + big, "allocated_bytes must see the 8 MiB buffer");
    let mut e = dev.begin_frame().unwrap();
    e.transition(&[buf(victim, Undefined, ShaderWrite), tex(vt, Undefined, RenderTarget)]);
    e.begin_render_pass(&RenderPassDesc { label: "touch texture", colors: &[color(vt, LoadOp::Clear([0.0; 4]))], depth: None });
    e.end_render_pass();
    e.begin_compute_pass("fill victim");
    e.set_compute_pipeline(pso);
    e.set_compute_constants(&constants(&[vi.0, N, 11]));
    e.dispatch(N / 64, 1, 1);
    e.end_compute_pass();
    e.transition(&[buf(victim, ShaderWrite, CopySrc), buf(rb, Undefined, CopyDst)]);
    e.copy_buffer(victim, 0, rb, 0, N as u64 * 4);
    dev.end_frame(e).unwrap();
    // Destroy right away, without waiting: the GPU may still be running the frame.
    dev.unbind_buffer(vi);
    dev.destroy_buffer(victim);
    dev.destroy_texture(vt);
    // The handles are dead immediately; a new buffer must not revive them.
    assert!(dev.bind_buffer(victim).is_err(), "destroyed buffer handle must be rejected");
    let other = dev.create_buffer(&BufferDesc { label: "slot reuse", size: 256, usage: Usage { shader_read: true, ..usage() } }).unwrap();
    assert_ne!(other, victim);
    assert!(dev.bind_buffer(victim).is_err(), "stale handle stays dead after slot reuse");
    // The frame's work still completed against the live buffer.
    for (i, v) in words(dev, rb).into_iter().enumerate() {
        assert_eq!(v, i as u32 * 3 + 11, "element {i}");
    }
    dev.destroy_buffer(other);
    // Frames retire the deferred frees.
    for _ in 0..4 {
        let e = dev.begin_frame().unwrap();
        dev.end_frame(e).unwrap();
    }
    dev.wait_idle();
    let after = dev.allocated_bytes();
    assert!(after < base + big, "deferred frees did not return memory: base {base}, after {after}");
}
