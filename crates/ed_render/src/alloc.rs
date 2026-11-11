//! Range allocation inside the shared geometry buffers (vertices, indices, skin data), with
//! frees deferred until the frames that may still read a range have finished.
//!
//! First fit over a `BTreeMap` of free ranges (deterministic: same calls, same offsets),
//! coalescing neighbours on free. Frees go through [`Deferred`]: a range freed while frames
//! are in flight becomes reusable once the device reports those frames complete.

use std::collections::BTreeMap;

#[derive(Clone, Debug, Default)]
pub(crate) struct RangeAlloc {
    capacity: u32,
    /// start -> length
    free: BTreeMap<u32, u32>,
    used: u32,
}

impl RangeAlloc {
    pub fn new(capacity: u32) -> Self {
        let mut free = BTreeMap::new();
        if capacity > 0 {
            free.insert(0, capacity);
        }
        Self { capacity, free, used: 0 }
    }

    /// First free range of `n` elements, or `None`.
    pub fn alloc(&mut self, n: u32) -> Option<u32> {
        if n == 0 {
            return Some(0);
        }
        let (&start, &len) = self.free.iter().find(|(_, l)| **l >= n)?;
        self.free.remove(&start);
        if len > n {
            self.free.insert(start + n, len - n);
        }
        self.used += n;
        Some(start)
    }

    pub fn free(&mut self, start: u32, n: u32) {
        if n == 0 {
            return;
        }
        debug_assert!(start + n <= self.capacity);
        self.used -= n;
        let (mut s, mut l) = (start, n);
        if let Some((&ps, &pl)) = self.free.range(..start).next_back()
            && ps + pl == start
        {
            self.free.remove(&ps);
            s = ps;
            l += pl;
        }
        if let Some(&nl) = self.free.get(&(start + n)) {
            self.free.remove(&(start + n));
            l += nl;
        }
        self.free.insert(s, l);
    }

    /// Elements in use.
    pub fn used(&self) -> u32 {
        self.used
    }
    #[cfg(test)]
    /// Highest end of any allocation (the part of the buffer ever written).
    pub fn high_water(&self) -> u32 {
        match self.free.iter().next_back() {
            Some((&s, &l)) if s + l == self.capacity => s,
            _ => self.capacity,
        }
    }
    #[cfg(test)]
    /// Number of separate free ranges (fragmentation).
    pub fn free_ranges(&self) -> usize {
        self.free.len()
    }
}

/// What a deferred release returns to its allocator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Release {
    Vertices(u32, u32),
    Indices(u32, u32),
    SkinSource(u32, u32),
    Palette(u32, u32),
    MeshSlot(u32),
    SkinSlot(u32),
    TextureSlot(u32),
}

/// Releases waiting for frames to finish: `(frames that must complete, release)`.
#[derive(Debug, Default)]
pub(crate) struct Deferred {
    pending: Vec<(u64, Release)>,
}

impl Deferred {
    pub fn push(&mut self, after_frames: u64, r: Release) {
        self.pending.push((after_frames, r));
    }
    /// Takes every release whose frames have completed, in the order they were pushed.
    pub fn ready(&mut self, completed: u64) -> Vec<Release> {
        let mut out = Vec::new();
        self.pending.retain(|&(after, r)| {
            if after <= completed {
                out.push(r);
                false
            } else {
                true
            }
        });
        out
    }
    pub fn len(&self) -> usize {
        self.pending.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_fit_coalesces_and_reuses() {
        let mut a = RangeAlloc::new(100);
        let x = a.alloc(10).unwrap();
        let y = a.alloc(20).unwrap();
        let z = a.alloc(30).unwrap();
        assert_eq!((x, y, z), (0, 10, 30));
        a.free(y, 20);
        assert_eq!(a.alloc(15), Some(10));
        a.free(10, 15);
        a.free(x, 10);
        a.free(z, 30);
        assert_eq!((a.used(), a.free_ranges()), (0, 1));
        assert_eq!(a.alloc(100), Some(0));
        assert_eq!(a.alloc(1), None);
    }

    #[test]
    fn churn_stays_bounded() {
        let mut a = RangeAlloc::new(1000);
        let mut live = Vec::new();
        for i in 0..500u32 {
            let n = 5 + (i * 7919) % 40;
            let s = a.alloc(n).expect("fits");
            live.push((s, n));
            if live.len() > 10 {
                let (s, n) = live.remove((i as usize * 31) % live.len());
                a.free(s, n);
            }
        }
        assert!(a.high_water() <= 1000);
    }

    #[test]
    fn deferred_waits_for_frames() {
        let mut d = Deferred::default();
        d.push(3, Release::MeshSlot(1));
        d.push(5, Release::MeshSlot(2));
        assert!(d.ready(2).is_empty());
        assert_eq!(d.ready(3), [Release::MeshSlot(1)]);
        assert_eq!(d.ready(9), [Release::MeshSlot(2)]);
        assert_eq!(d.len(), 0);
    }
}
