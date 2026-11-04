//! Least-recently-seen bookkeeping.
//!
//! Streaming keeps tiles that left the view loaded (turning around shouldn't
//! rebuild the city behind you), but only up to a cap. [`Lru`] remembers when
//! each key was last useful and names the oldest ones once over the cap.

use std::collections::HashMap;
use std::hash::Hash;

#[derive(Debug)]
pub struct Lru<K> {
    last: HashMap<K, u64>,
    clock: u64,
}

impl<K: Hash + Eq + Clone> Default for Lru<K> {
    fn default() -> Self {
        Self { last: HashMap::new(), clock: 0 }
    }
}

impl<K: Hash + Eq + Clone> Lru<K> {
    /// Advance time; call once per update before touching keys.
    pub fn tick(&mut self) {
        self.clock += 1;
    }

    /// Mark `k` as useful now (inserting it if new).
    pub fn touch(&mut self, k: &K) {
        let now = self.clock;
        match self.last.get_mut(k) {
            Some(t) => *t = now,
            None => {
                self.last.insert(k.clone(), now);
            }
        }
    }

    pub fn remove(&mut self, k: &K) {
        self.last.remove(k);
    }

    pub fn len(&self) -> usize {
        self.last.len()
    }

    pub fn is_empty(&self) -> bool {
        self.last.is_empty()
    }

    pub fn contains(&self, k: &K) -> bool {
        self.last.contains_key(k)
    }

    /// Ticks since `k` was last touched.
    pub fn age(&self, k: &K) -> Option<u64> {
        self.last.get(k).map(|t| self.clock - t)
    }

    /// Keys to drop so that at most `cap` remain, oldest first. Keys touched
    /// this tick are never returned. Does not remove them: the caller drops
    /// what it can and calls [`Self::remove`].
    pub fn over_cap(&self, cap: usize) -> Vec<K> {
        if self.last.len() <= cap {
            return Vec::new();
        }
        let mut v: Vec<(&K, u64)> = self.last.iter().filter(|(_, t)| **t < self.clock).map(|(k, t)| (k, *t)).collect();
        v.sort_by_key(|(_, t)| *t);
        v.into_iter().take(self.last.len() - cap).map(|(k, _)| k.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evicts_oldest_first_and_spares_current() {
        let mut l = Lru::default();
        for k in 0..5u32 {
            l.tick();
            l.touch(&k);
        }
        l.tick();
        l.touch(&0); // 0 is fresh again
        assert_eq!(l.over_cap(3), vec![1, 2]);
        assert_eq!(l.age(&1), Some(4));
        assert!(l.over_cap(5).is_empty());
        // Everything touched this tick is protected even over the cap.
        let mut m = Lru::default();
        m.tick();
        m.touch(&1u32);
        m.touch(&2u32);
        assert!(m.over_cap(0).is_empty());
    }
}
