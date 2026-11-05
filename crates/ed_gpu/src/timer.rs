//! Per-pass GPU time (m2.md rule 5). Backends push raw samples in milliseconds; this smooths
//! them and hands the HUD a stable, sorted list. No GPU types here.

use std::collections::HashMap;

/// Exponential moving average per pass name, plus last sample and peak.
#[derive(Default, Debug, Clone)]
pub struct PassTimes {
    map: HashMap<String, Entry>,
    /// EMA weight of a new sample, 0..1. 0.1 is stable at 60 fps.
    alpha: f32,
}

#[derive(Debug, Clone, Copy, Default)]
struct Entry {
    avg: f32,
    last: f32,
    peak: f32,
    samples: u32,
}

impl PassTimes {
    pub fn new() -> Self {
        PassTimes { map: HashMap::new(), alpha: 0.1 }
    }

    pub fn record(&mut self, pass: &str, ms: f32) {
        if !ms.is_finite() || ms < 0.0 {
            return;
        }
        let alpha = if self.alpha == 0.0 { 0.1 } else { self.alpha };
        let e = self.map.entry(pass.to_owned()).or_default();
        e.avg = if e.samples == 0 { ms } else { e.avg + (ms - e.avg) * alpha };
        e.last = ms;
        e.peak = e.peak.max(ms);
        e.samples += 1;
    }

    pub fn avg(&self, pass: &str) -> Option<f32> {
        self.map.get(pass).map(|e| e.avg)
    }

    pub fn last(&self, pass: &str) -> Option<f32> {
        self.map.get(pass).map(|e| e.last)
    }

    /// Sum of smoothed pass times: the GPU cost the graph claims per frame.
    pub fn total_avg(&self) -> f32 {
        self.map.values().map(|e| e.avg).sum()
    }

    /// `(name, avg_ms)` sorted by cost, most expensive first.
    pub fn sorted(&self) -> Vec<(&str, f32)> {
        let mut v: Vec<_> = self.map.iter().map(|(k, e)| (k.as_str(), e.avg)).collect();
        v.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(b.0)));
        v
    }

    pub fn reset_peaks(&mut self) {
        for e in self.map.values_mut() {
            e.peak = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ema_converges_and_sorts() {
        let mut t = PassTimes::new();
        for _ in 0..200 {
            t.record("main", 4.0);
            t.record("shadow", 1.0);
        }
        assert!((t.avg("main").unwrap() - 4.0).abs() < 1e-3);
        assert_eq!(t.sorted()[0].0, "main");
        assert!((t.total_avg() - 5.0).abs() < 1e-2);
    }

    #[test]
    fn first_sample_is_exact_and_bad_samples_ignored() {
        let mut t = PassTimes::new();
        t.record("p", 3.0);
        assert_eq!(t.avg("p"), Some(3.0));
        t.record("p", f32::NAN);
        t.record("p", -1.0);
        assert_eq!(t.last("p"), Some(3.0));
    }
}
