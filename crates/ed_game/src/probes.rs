//! Lightweight frame-cost probes for the perf logger (atomics, no system params).
//!
//! Systems push durations; `perf.rs` folds them into its once-per-second row.
//! Kept separate so hot systems don't grow past Bevy's system-param limits.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static APPLY_US: AtomicU64 = AtomicU64::new(0);
static RETIRE_US: AtomicU64 = AtomicU64::new(0);
static NATIVE_US: AtomicU64 = AtomicU64::new(0);
static UPLOAD_US: AtomicU64 = AtomicU64::new(0);
static PREPQ: AtomicU64 = AtomicU64::new(0);
static READYQ: AtomicU64 = AtomicU64::new(0);
static UPLOADED: AtomicU64 = AtomicU64::new(0);
static UPBYTES: AtomicU64 = AtomicU64::new(0);

fn add(slot: &AtomicU64, d: Duration) {
    slot.fetch_add(d.as_micros().min(u64::MAX as u128) as u64, Ordering::Relaxed);
}

pub fn add_apply(d: Duration) { add(&APPLY_US, d); }
pub fn add_retire(d: Duration) { add(&RETIRE_US, d); }
pub fn add_native(d: Duration) { add(&NATIVE_US, d); }
pub fn add_upload(d: Duration) { add(&UPLOAD_US, d); }

/// One mesh upload: count and bytes touched (for ms/MB attribution).
pub fn add_upload_stat(bytes: usize) {
    UPLOADED.fetch_add(1, Ordering::Relaxed);
    UPBYTES.fetch_add(bytes as u64, Ordering::Relaxed);
}

/// Backlog gauges (last value seen, not accumulated).
pub fn set_prepq(n: usize) { PREPQ.store(n as u64, Ordering::Relaxed); }
pub fn set_readyq(n: usize) { READYQ.store(n as u64, Ordering::Relaxed); }
pub fn prepq() -> u64 { PREPQ.load(Ordering::Relaxed) }
pub fn readyq() -> u64 { READYQ.load(Ordering::Relaxed) }

pub struct ProbeRow {
    pub apply_ms: f32,
    pub retire_ms: f32,
    pub native_ms: f32,
    pub upload_ms: f32,
    pub prepq: u64,
    pub readyq: u64,
    pub uploaded: u64,
    pub up_mb: f32,
}

/// Drain accumulated counters (call once per perf row).
pub fn take() -> ProbeRow {
    let us = |s: &AtomicU64| s.swap(0, Ordering::Relaxed) as f32 / 1000.0;
    ProbeRow {
        apply_ms: us(&APPLY_US),
        retire_ms: us(&RETIRE_US),
        native_ms: us(&NATIVE_US),
        upload_ms: us(&UPLOAD_US),
        prepq: PREPQ.load(Ordering::Relaxed),
        readyq: READYQ.load(Ordering::Relaxed),
        uploaded: UPLOADED.swap(0, Ordering::Relaxed),
        up_mb: UPBYTES.swap(0, Ordering::Relaxed) as f32 / (1 << 20) as f32,
    }
}
