//! Metal buffer upload micro-benchmark: shared `newBufferWithBytes` vs
//! `newBufferWithLength` + memcpy vs private buffer + GPU blit.
//!
//! `cargo run --release -p ed_metal --example bufbench`
//!
//! Answers "how fast can we get mesh vertex data into a buffer on this GPU",
//! which decides whether streaming uploads should be spread, staged, or cut.

use ed_metal::context::MetalContext;
use objc2_metal::*;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::time::Instant;

fn main() {
    let ctx = MetalContext::new().expect("metal");
    println!("device: {}", ctx.name());
    let mbs = [4usize, 16, 64, 128, 256];
    for mb in mbs {
        let bytes = mb * (1 << 20);
        let src: Vec<u8> = vec![7u8; bytes];

        // 1. newBufferWithBytes (shared): alloc + copy, one call.
        let t = Instant::now();
        let b = unsafe { ctx.device.newBufferWithBytes_length_options(NonNull::new(src.as_ptr() as *mut c_void).unwrap(), bytes, MTLResourceOptions::StorageModeShared) };
        let with_bytes = t.elapsed().as_secs_f64();
        drop(b);

        // 2. newBufferWithLength (shared) + CPU memcpy.
        let t = Instant::now();
        let b = ctx.device.newBufferWithLength_options(bytes, MTLResourceOptions::StorageModeShared);
        let alloc = t.elapsed().as_secs_f64();
        let t = Instant::now();
        if let Some(b) = &b {
            unsafe { std::ptr::copy_nonoverlapping(src.as_ptr(), b.contents().as_ptr() as *mut u8, bytes) };
        }
        let memcpy = t.elapsed().as_secs_f64();
        drop(b);

        // 3. Private buffer + GPU blit from a shared staging buffer.
        let t = Instant::now();
        let stage = unsafe { ctx.device.newBufferWithBytes_length_options(NonNull::new(src.as_ptr() as *mut c_void).unwrap(), bytes, MTLResourceOptions::StorageModeShared) };
        let dst = ctx.device.newBufferWithLength_options(bytes, MTLResourceOptions::StorageModePrivate);
        let mut blit = 0.0;
        if let (Some(stage), Some(dst)) = (&stage, &dst) {
            let cb = ctx.queue.commandBuffer().unwrap();
            let be = cb.blitCommandEncoder().unwrap();
            unsafe { be.copyFromBuffer_sourceOffset_toBuffer_destinationOffset_size(stage, 0, dst, 0, bytes) };
            be.endEncoding();
            cb.commit();
            let tb = Instant::now();
            cb.waitUntilCompleted();
            blit = tb.elapsed().as_secs_f64();
        }
        let private = t.elapsed().as_secs_f64();
        drop((stage, dst));

        let gbs = |s: f64| (bytes as f64 / (1 << 30) as f64) / s.max(1e-9);
        println!(
            "{mb:>4} MB | withBytes {:7.2} ms ({:5.1} GB/s) | alloc {:7.2} ms | memcpy {:7.2} ms ({:5.1} GB/s) | private+blit {:7.2} ms (blit {:6.2} ms, {:5.1} GB/s)",
            with_bytes * 1e3, gbs(with_bytes), alloc * 1e3, memcpy * 1e3, gbs(memcpy), private * 1e3, blit * 1e3, gbs(private),
        );
    }
}
