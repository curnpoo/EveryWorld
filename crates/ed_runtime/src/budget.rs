//! Per-frame work budgets.
//!
//! Main-thread steps (spawning a tile's entities, uploading meshes) are cut
//! into items with an estimated cost. Each frame gets a fixed number of cost
//! units plus a wall-clock cap; the first item of a frame always runs, so a
//! single oversized item can't stall the queue forever.

use std::time::{Duration, Instant};

/// Work allowed this frame, in abstract cost units and wall-clock time.
#[derive(Debug)]
pub struct FrameBudget {
    units: u32,
    spent: u32,
    deadline: Option<Instant>,
    items: u32,
}

impl FrameBudget {
    /// A budget of `units` cost units, and at most `time` of wall clock from now.
    pub fn new(units: u32, time: Option<Duration>) -> Self {
        Self { units, spent: 0, deadline: time.map(|t| Instant::now() + t), items: 0 }
    }

    /// Spend `cost` if it fits. The first item each frame always fits.
    pub fn try_spend(&mut self, cost: u32) -> bool {
        if self.items > 0 {
            if self.spent.saturating_add(cost) > self.units {
                return false;
            }
            if self.deadline.is_some_and(|d| Instant::now() >= d) {
                return false;
            }
        }
        self.spent = self.spent.saturating_add(cost);
        self.items += 1;
        true
    }

    /// Whether anything is left (cheap check before peeking at work).
    pub fn exhausted(&self) -> bool {
        self.items > 0 && (self.spent >= self.units || self.deadline.is_some_and(|d| Instant::now() >= d))
    }

    pub fn spent(&self) -> u32 {
        self.spent
    }

    pub fn items(&self) -> u32 {
        self.items
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_item_always_runs() {
        let mut b = FrameBudget::new(10, None);
        assert!(b.try_spend(1_000));
        assert!(!b.try_spend(1));
        assert_eq!(b.items(), 1);
    }

    #[test]
    fn packs_small_items() {
        let mut b = FrameBudget::new(10, None);
        for _ in 0..5 {
            assert!(b.try_spend(2));
        }
        assert!(!b.try_spend(1));
        assert!(b.exhausted());
        assert_eq!(b.spent(), 10);
    }

    #[test]
    fn deadline_stops_after_first() {
        let mut b = FrameBudget::new(1_000, Some(Duration::ZERO));
        assert!(b.try_spend(1));
        assert!(!b.try_spend(1));
    }
}
