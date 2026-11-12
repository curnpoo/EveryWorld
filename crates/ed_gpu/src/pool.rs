//! Backend-independent bookkeeping: generational handle pools and bindless slot allocation.

use std::num::NonZeroU32;

/// Generational slot pool. `insert` returns `(index, generation)`; a removed slot bumps its
/// generation so stale handles miss.
pub(crate) struct Pool<T> {
    slots: Vec<(NonZeroU32, Option<T>)>,
    free: Vec<u32>,
    live: usize,
}

impl<T> Default for Pool<T> {
    fn default() -> Self {
        Self { slots: Vec::new(), free: Vec::new(), live: 0 }
    }
}

impl<T> Pool<T> {
    pub fn insert(&mut self, value: T) -> (u32, NonZeroU32) {
        self.live += 1;
        if let Some(i) = self.free.pop() {
            let slot = &mut self.slots[i as usize];
            slot.1 = Some(value);
            (i, slot.0)
        } else {
            let i = self.slots.len() as u32;
            self.slots.push((NonZeroU32::MIN, Some(value)));
            (i, NonZeroU32::MIN)
        }
    }

    pub fn get(&self, index: u32, generation: NonZeroU32) -> Option<&T> {
        match self.slots.get(index as usize) {
            Some((g, Some(v))) if *g == generation => Some(v),
            _ => None,
        }
    }

    pub fn get_mut(&mut self, index: u32, generation: NonZeroU32) -> Option<&mut T> {
        match self.slots.get_mut(index as usize) {
            Some((g, Some(v))) if *g == generation => Some(v),
            _ => None,
        }
    }

    pub fn remove(&mut self, index: u32, generation: NonZeroU32) -> Option<T> {
        let slot = self.slots.get_mut(index as usize)?;
        if slot.0 != generation || slot.1.is_none() {
            return None;
        }
        let v = slot.1.take();
        // Wrap past u32::MAX back to 1; a handle that old is long gone.
        slot.0 = slot.0.checked_add(1).unwrap_or(NonZeroU32::MIN);
        self.free.push(index);
        self.live -= 1;
        v
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.live
    }

    /// Removes every live value (device teardown).
    #[allow(dead_code)]
    pub fn drain(&mut self) -> Vec<T> {
        self.free.clear();
        self.live = 0;
        let out = self.slots.iter_mut().filter_map(|s| s.1.take()).collect();
        self.slots.clear();
        out
    }
}

/// Bindless slot allocator. Freed slots wait until the frame that freed them has finished on
/// the GPU (`retire`), so an index is never reused while an in-flight frame may read it.
pub(crate) struct Slots {
    capacity: u32,
    next: u32,
    free: Vec<u32>,
    /// (frames that must complete first, slot)
    pending: Vec<(u64, u32)>,
}

impl Slots {
    pub fn new(capacity: u32) -> Self {
        Self { capacity, next: 0, free: Vec::new(), pending: Vec::new() }
    }

    pub fn alloc(&mut self) -> Option<u32> {
        // Lowest free index first keeps tables compact and deterministic.
        if let Some(i) = self.free.pop() {
            return Some(i);
        }
        if self.next < self.capacity {
            self.next += 1;
            return Some(self.next - 1);
        }
        None
    }

    /// Free `slot` once `completed_frames >= after`.
    pub fn free_after(&mut self, slot: u32, after: u64) {
        self.pending.push((after, slot));
    }

    /// Returns the slots that became free.
    pub fn retire(&mut self, completed_frames: u64, out: &mut Vec<u32>) {
        let mut i = 0;
        while i < self.pending.len() {
            if self.pending[i].0 <= completed_frames {
                let (_, s) = self.pending.swap_remove(i);
                out.push(s);
            } else {
                i += 1;
            }
        }
        for &s in out.iter() {
            self.free.push(s);
        }
        // Pop yields the lowest index first.
        self.free.sort_unstable_by(|a, b| b.cmp(a));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_handle_misses() {
        let mut p = Pool::default();
        let (i, g) = p.insert(5);
        assert_eq!(p.get(i, g), Some(&5));
        assert_eq!(p.remove(i, g), Some(5));
        assert_eq!(p.get(i, g), None);
        let (i2, g2) = p.insert(6);
        assert_eq!(i2, i);
        assert_ne!(g2, g);
        assert_eq!(p.get(i, g), None);
    }

    #[test]
    fn slots_wait_for_frames() {
        let mut s = Slots::new(2);
        let a = s.alloc().unwrap();
        let _b = s.alloc().unwrap();
        assert!(s.alloc().is_none());
        s.free_after(a, 5);
        let mut out = vec![];
        s.retire(4, &mut out);
        assert!(out.is_empty() && s.alloc().is_none());
        s.retire(5, &mut out);
        assert_eq!(out, vec![a]);
        assert_eq!(s.alloc(), Some(a));
    }
}
