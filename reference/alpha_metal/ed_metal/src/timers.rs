//! Per-pass GPU timing via counter sample buffers at render-pass stage boundaries.
//!
//! A ring of `RING` sample buffers, one per in-flight frame. `attach` reserves a start/end pair
//! for a pass; `resolve_frame` for frame N is called only after frame N+RING-1 was encoded, by
//! which time N is complete (drawable count caps in-flight frames), so nothing ever blocks.

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_foundation::{NSRange, NSString};
use objc2_metal::*;

pub const RING: usize = 3;
pub const MAX_PASSES: usize = 64;

pub struct GpuTimers {
    bufs: Vec<Retained<ProtocolObject<dyn MTLCounterSampleBuffer>>>,
    names: Vec<Vec<String>>,
    frame: usize,
    cur: usize,
    /// ns per GPU tick, from CPU/GPU timestamp calibration.
    ns_per_tick: f64,
    cal: (u64, u64),
    pub supported: bool,
}

impl GpuTimers {
    pub fn new(device: &ProtocolObject<dyn MTLDevice>) -> Self {
        let mut t = GpuTimers { bufs: vec![], names: vec![vec![]; RING], frame: 0, cur: 0, ns_per_tick: 1.0, cal: (0, 0), supported: false };
        if !device.supportsCounterSampling(MTLCounterSamplingPoint::AtStageBoundary) {
            return t;
        }
        let Some(sets) = device.counterSets() else { return t };
        let Some(set) = (0..sets.count()).map(|i| sets.objectAtIndex(i)).find(|s| s.name().to_string() == "timestamp") else { return t };
        for i in 0..RING {
            let d = MTLCounterSampleBufferDescriptor::new();
            d.setCounterSet(Some(&set));
            d.setStorageMode(MTLStorageMode::Shared);
            unsafe { d.setSampleCount(MAX_PASSES * 2) };
            d.setLabel(&NSString::from_str(&format!("ed_timers_{i}")));
            match device.newCounterSampleBufferWithDescriptor_error(&d) {
                Ok(b) => t.bufs.push(b),
                Err(_) => return t,
            }
        }
        t.cal = t.sample_clocks(device);
        t.supported = true;
        t
    }

    fn sample_clocks(&self, device: &ProtocolObject<dyn MTLDevice>) -> (u64, u64) {
        let (mut cpu, mut gpu) = (0u64, 0u64);
        unsafe { device.sampleTimestamps_gpuTimestamp(std::ptr::NonNull::from(&mut cpu), std::ptr::NonNull::from(&mut gpu)) };
        (cpu, gpu)
    }

    /// Refresh the tick->ns ratio (call occasionally; clocks drift slightly).
    pub fn recalibrate(&mut self, device: &ProtocolObject<dyn MTLDevice>) {
        if !self.supported {
            return;
        }
        let now = self.sample_clocks(device);
        let (dc, dg) = (now.0.saturating_sub(self.cal.0), now.1.saturating_sub(self.cal.1));
        if dg > 1_000_000 && dc > 1_000_000 {
            self.ns_per_tick = dc as f64 / dg as f64;
        }
    }

    pub fn begin_frame(&mut self) {
        self.cur = self.frame % RING;
        self.names[self.cur].clear();
    }

    /// Reserve a timestamp pair on `pass` for `name`. No-op if unsupported or out of slots.
    pub fn attach(&mut self, pass: &MTLRenderPassDescriptor, name: &str) {
        if !self.supported || self.names[self.cur].len() >= MAX_PASSES {
            return;
        }
        let i = self.names[self.cur].len();
        let a = unsafe { pass.sampleBufferAttachments().objectAtIndexedSubscript(0) };
        a.setSampleBuffer(Some(&self.bufs[self.cur]));
        unsafe {
            a.setStartOfVertexSampleIndex(i * 2);
            a.setEndOfFragmentSampleIndex(i * 2 + 1);
        }
        self.names[self.cur].push(name.to_owned());
    }

    /// Same as `attach`, for a compute pass.
    pub fn attach_compute(&mut self, pass: &MTLComputePassDescriptor, name: &str) {
        if !self.supported || self.names[self.cur].len() >= MAX_PASSES {
            return;
        }
        let i = self.names[self.cur].len();
        let a = unsafe { pass.sampleBufferAttachments().objectAtIndexedSubscript(0) };
        a.setSampleBuffer(Some(&self.bufs[self.cur]));
        unsafe {
            a.setStartOfEncoderSampleIndex(i * 2);
            a.setEndOfEncoderSampleIndex(i * 2 + 1);
        }
        self.names[self.cur].push(name.to_owned());
    }

    /// Same as `attach`, for a blit pass.
    pub fn attach_blit(&mut self, pass: &MTLBlitPassDescriptor, name: &str) {
        if !self.supported || self.names[self.cur].len() >= MAX_PASSES {
            return;
        }
        let i = self.names[self.cur].len();
        let a = unsafe { pass.sampleBufferAttachments().objectAtIndexedSubscript(0) };
        a.setSampleBuffer(Some(&self.bufs[self.cur]));
        unsafe {
            a.setStartOfEncoderSampleIndex(i * 2);
            a.setEndOfEncoderSampleIndex(i * 2 + 1);
        }
        self.names[self.cur].push(name.to_owned());
    }

    pub fn end_frame(&mut self) {
        self.frame += 1;
    }

    /// Times for the frame encoded `RING - 1` frames ago. Empty until the ring has filled.
    pub fn resolve_oldest(&self) -> Vec<(String, f32)> {
        if !self.supported || self.frame < RING {
            return vec![];
        }
        let slot = (self.frame + 1) % RING; // oldest buffer in the ring
        let names = &self.names[slot];
        if names.is_empty() {
            return vec![];
        }
        let range = NSRange { location: 0, length: names.len() * 2 };
        let Some(data) = (unsafe { self.bufs[slot].resolveCounterRange(range) }) else { return vec![] };
        let bytes = unsafe { data.as_bytes_unchecked() };
        let stamps: Vec<u64> = bytes.chunks_exact(8).map(|c| u64::from_ne_bytes(c.try_into().unwrap())).collect();
        names
            .iter()
            .enumerate()
            .filter_map(|(i, n)| {
                let (s, e) = (*stamps.get(i * 2)?, *stamps.get(i * 2 + 1)?);
                // MTLCounterErrorValue (u64::MAX) means the sample was not taken.
                if s == u64::MAX || e == u64::MAX || e < s {
                    return None;
                }
                Some((n.clone(), ((e - s) as f64 * self.ns_per_tick / 1.0e6) as f32))
            })
            .collect()
    }
}
