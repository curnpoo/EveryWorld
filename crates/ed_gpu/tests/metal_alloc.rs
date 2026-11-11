//! Create/destroy churn must not grow device memory. Its own test binary because
//! `allocated_bytes` is per Metal device, which every device in a process shares.
#![cfg(target_os = "macos")]

mod common;
use common::*;
use ed_gpu::*;

#[test]
fn churn_10k_buffers_and_textures_without_growth() {
    let dev = device();
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
