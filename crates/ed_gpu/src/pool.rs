//! Backend-independent suballocator for streaming GPU data.
//!
//! A [`Pool`] hands out aligned byte ranges inside one large GPU buffer owned by the backend.
//! It only does bookkeeping: it never touches GPU memory. Frees that the GPU may still be
//! reading must go through [`Pool::free_deferred`] and are released by [`Pool::retire`] once
//! the frame that last used them has completed.
//!
//! Allocation is first-fit over an offset-sorted free list. Freeing coalesces with both
//! neighbours, so the free list always holds maximal, non-adjacent blocks.

/// A byte range inside a [`Pool`]. `offset` and `size` are multiples of the pool alignment.
/// `size` is the rounded size actually reserved, which is what must be passed back to
/// [`Pool::free`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PoolAlloc {
    pub offset: u64,
    pub size: u64,
}

/// Suballocator over the byte range `[0, capacity)`.
#[derive(Debug)]
pub struct Pool {
    capacity: u64,
    alignment: u64,
    /// Free blocks sorted by offset, non-overlapping, never adjacent (always coalesced).
    free_list: Vec<PoolAlloc>,
    /// Sum of the sizes in `free_list`.
    free_total: u64,
    /// Frees waiting for the GPU to finish frame `.0`, in the order they were queued.
    deferred: Vec<(u64, PoolAlloc)>,
}

impl Pool {
    /// Creates a pool over `capacity` bytes with every allocation aligned to `alignment`.
    ///
    /// `alignment` must be a non-zero power of two. `capacity` is rounded down to a multiple
    /// of `alignment`; query [`Pool::capacity`] for the effective value.
    ///
    /// # Panics
    /// If `alignment` is not a power of two.
    pub fn new(capacity: u64, alignment: u64) -> Self {
        assert!(alignment.is_power_of_two(), "pool alignment must be a power of two, got {alignment}");
        let capacity = capacity & !(alignment - 1);
        let free_list = if capacity > 0 {
            vec![PoolAlloc { offset: 0, size: capacity }]
        } else {
            Vec::new()
        };
        Pool { capacity, alignment, free_list, free_total: capacity, deferred: Vec::new() }
    }

    /// Reserves `size` bytes (rounded up to the alignment; a zero request reserves one
    /// alignment unit). Returns `None` if no free block is large enough.
    pub fn alloc(&mut self, size: u64) -> Option<PoolAlloc> {
        let size = self.round_up(size.max(1))?;
        let idx = self.free_list.iter().position(|b| b.size >= size)?;
        let block = self.free_list[idx];
        let out = PoolAlloc { offset: block.offset, size };
        if block.size == size {
            self.free_list.remove(idx);
        } else {
            self.free_list[idx] = PoolAlloc { offset: block.offset + size, size: block.size - size };
        }
        self.free_total -= size;
        Some(out)
    }

    /// Returns a range to the pool immediately, coalescing with adjacent free blocks.
    ///
    /// The GPU must no longer be reading the range. Use [`Pool::free_deferred`] otherwise.
    /// Freeing a range that overlaps free memory (including a double free) panics in debug
    /// builds.
    pub fn free(&mut self, a: PoolAlloc) {
        debug_assert!(
            a.size > 0 && a.size % self.alignment == 0 && a.offset % self.alignment == 0,
            "freed range {a:?} is not a non-empty, aligned range (alignment {})",
            self.alignment
        );
        debug_assert!(
            a.offset.checked_add(a.size).is_some_and(|end| end <= self.capacity),
            "freed range {a:?} exceeds pool capacity {}",
            self.capacity
        );

        // First free block that starts after `a`; its predecessor is the only other
        // candidate for overlap or left-adjacency.
        let idx = self.free_list.partition_point(|b| b.offset <= a.offset);
        if idx > 0 {
            let prev = self.free_list[idx - 1];
            debug_assert!(
                prev.offset + prev.size <= a.offset,
                "double free or overlap: {a:?} intersects free block {prev:?}"
            );
        }
        if let Some(next) = self.free_list.get(idx) {
            debug_assert!(
                a.offset + a.size <= next.offset,
                "double free or overlap: {a:?} intersects free block {next:?}"
            );
        }

        // Insert, merging with the left neighbour when adjacent.
        let mut at = idx;
        let merged_left = idx > 0 && {
            let prev = self.free_list[idx - 1];
            prev.offset + prev.size == a.offset
        };
        if merged_left {
            at = idx - 1;
            self.free_list[at].size += a.size;
        } else {
            self.free_list.insert(idx, a);
        }

        // Merge with the right neighbour when adjacent.
        let end = self.free_list[at].offset + self.free_list[at].size;
        if self.free_list.get(at + 1).is_some_and(|next| next.offset == end) {
            let next = self.free_list.remove(at + 1);
            self.free_list[at].size += next.size;
        }

        self.free_total += a.size;
    }

    /// Queues `a` to be freed once `retire` is called with `completed_frame >= frame`.
    /// Use this for any range the GPU may still read in `frame`.
    pub fn free_deferred(&mut self, a: PoolAlloc, frame: u64) {
        self.deferred.push((frame, a));
    }

    /// Frees every deferred range whose frame is `<= completed_frame`, in queue order.
    /// Ranges for later frames stay queued.
    pub fn retire(&mut self, completed_frame: u64) {
        if self.deferred.is_empty() {
            return;
        }
        let pending = std::mem::take(&mut self.deferred);
        for (frame, a) in pending {
            if frame <= completed_frame {
                self.free(a);
            } else {
                self.deferred.push((frame, a));
            }
        }
    }

    /// Number of ranges still waiting in the deferred queue.
    pub fn deferred_len(&self) -> usize {
        self.deferred.len()
    }

    /// Bytes reserved by live allocations and by deferred frees not yet retired.
    pub fn used(&self) -> u64 {
        self.capacity - self.free_total
    }

    /// Bytes not reserved.
    pub fn free_bytes(&self) -> u64 {
        self.free_total
    }

    /// Size of the largest contiguous free block, or 0 if the pool is full.
    pub fn largest_free(&self) -> u64 {
        self.free_list.iter().map(|b| b.size).max().unwrap_or(0)
    }

    /// External fragmentation in `[0, 1]`: `1 - largest_free / free_bytes`.
    /// Zero when the pool is empty or full.
    pub fn fragmentation(&self) -> f32 {
        if self.free_total == 0 {
            return 0.0;
        }
        1.0 - self.largest_free() as f32 / self.free_total as f32
    }

    /// Effective capacity in bytes (a multiple of the alignment).
    pub fn capacity(&self) -> u64 {
        self.capacity
    }

    fn round_up(&self, n: u64) -> Option<u64> {
        let mask = self.alignment - 1;
        n.checked_add(mask).map(|v| v & !mask)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic LCG (Knuth MMIX constants) for the stress test.
    struct Lcg(u64);

    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            self.0 >> 33
        }
    }

    /// Checks that the free list is sorted and coalesced, and that free blocks, live
    /// allocations and deferred frees tile `[0, capacity)` exactly with no overlap.
    fn check_invariants(p: &Pool, live: &[PoolAlloc]) {
        for w in p.free_list.windows(2) {
            assert!(w[0].offset + w[0].size < w[1].offset, "free list not coalesced or sorted");
        }
        let free_sum: u64 = p.free_list.iter().map(|b| b.size).sum();
        assert_eq!(free_sum, p.free_total);

        let deferred_sum: u64 = p.deferred.iter().map(|(_, a)| a.size).sum();
        let live_sum: u64 = live.iter().map(|a| a.size).sum();
        assert_eq!(p.used(), live_sum + deferred_sum, "used must equal live + deferred");
        assert_eq!(p.used() + p.free_bytes(), p.capacity(), "used + free must equal capacity");

        let mut ranges: Vec<PoolAlloc> = p
            .free_list
            .iter()
            .copied()
            .chain(live.iter().copied())
            .chain(p.deferred.iter().map(|&(_, a)| a))
            .collect();
        ranges.sort_by_key(|r| r.offset);
        let mut cursor = 0;
        for r in &ranges {
            assert_eq!(r.offset, cursor, "overlap or gap at {r:?}");
            assert_eq!(r.offset % p.alignment, 0);
            cursor = r.offset + r.size;
        }
        assert_eq!(cursor, p.capacity(), "ranges do not cover the pool");
    }

    #[test]
    fn capacity_rounds_down_to_alignment() {
        let p = Pool::new(1000, 64);
        assert_eq!(p.capacity(), 960);
        assert_eq!(p.free_bytes(), 960);
        assert_eq!(p.used(), 0);
    }

    #[test]
    #[should_panic]
    fn non_power_of_two_alignment_panics() {
        let _ = Pool::new(1024, 48);
    }

    #[test]
    fn alignment_and_rounding() {
        let mut p = Pool::new(1024, 64);
        let a = p.alloc(1).unwrap();
        assert_eq!(a, PoolAlloc { offset: 0, size: 64 });
        let b = p.alloc(65).unwrap();
        assert_eq!(b, PoolAlloc { offset: 64, size: 128 });
        let c = p.alloc(64).unwrap();
        assert_eq!(c.offset, 192);
        let z = p.alloc(0).unwrap();
        assert_eq!(z.size, 64);
        for x in [a, b, c, z] {
            assert_eq!(x.offset % 64, 0);
            assert_eq!(x.size % 64, 0);
        }
        assert_eq!(p.used(), 64 + 128 + 64 + 64);
        check_invariants(&p, &[a, b, c, z]);
    }

    #[test]
    fn alignment_one_is_exact() {
        let mut p = Pool::new(100, 1);
        let a = p.alloc(10).unwrap();
        let b = p.alloc(7).unwrap();
        assert_eq!(a, PoolAlloc { offset: 0, size: 10 });
        assert_eq!(b, PoolAlloc { offset: 10, size: 7 });
    }

    #[test]
    fn exhaustion() {
        let mut p = Pool::new(256, 64);
        let allocs: Vec<_> = (0..4).map(|_| p.alloc(64).expect("should fit")).collect();
        assert!(p.alloc(1).is_none());
        assert!(p.alloc(64).is_none());
        assert_eq!(p.free_bytes(), 0);
        assert_eq!(p.largest_free(), 0);
        assert_eq!(p.fragmentation(), 0.0);
        check_invariants(&p, &allocs);
    }

    #[test]
    fn oversized_and_huge_requests_fail_cleanly() {
        let mut p = Pool::new(256, 64);
        assert!(p.alloc(512).is_none());
        assert!(p.alloc(u64::MAX).is_none());
        assert!(p.alloc(u64::MAX - 10).is_none());
        check_invariants(&p, &[]);
    }

    #[test]
    fn coalesce_right_neighbour_then_left() {
        // Free the first two blocks in order; the second merges left into the first.
        let mut p = Pool::new(256, 64);
        let a = p.alloc(64).unwrap();
        let b = p.alloc(64).unwrap();
        let c = p.alloc(64).unwrap();
        p.free(a);
        p.free(b);
        // [a,b] merged into [0,128); the untouched tail [192,256) is a separate block.
        assert_eq!(p.free_list, vec![PoolAlloc { offset: 0, size: 128 }, PoolAlloc { offset: 192, size: 64 }]);
        assert_eq!(p.largest_free(), 128);
        p.free(c);
        assert_eq!(p.largest_free(), 256);
        assert_eq!(p.free_list.len(), 1);
        check_invariants(&p, &[]);
    }

    #[test]
    fn coalesce_both_sides() {
        let mut p = Pool::new(256, 64);
        let a = p.alloc(64).unwrap();
        let b = p.alloc(64).unwrap();
        let c = p.alloc(64).unwrap();
        let d = p.alloc(64).unwrap();
        p.free(a);
        p.free(c);
        assert_eq!(p.free_list.len(), 2, "a and c are not adjacent");
        p.free(b); // merges with a on the left and c on the right
        assert_eq!(p.free_list, vec![PoolAlloc { offset: 0, size: 192 }]);
        p.free(d);
        assert_eq!(p.free_list, vec![PoolAlloc { offset: 0, size: 256 }]);
        check_invariants(&p, &[]);
    }

    #[test]
    fn coalesce_right_only_and_left_only() {
        let mut p = Pool::new(256, 64);
        let a = p.alloc(64).unwrap();
        let b = p.alloc(64).unwrap();
        let _c = p.alloc(64).unwrap();
        // Free b first: it has no free neighbours, so it becomes its own block.
        p.free(b);
        assert_eq!(p.free_list[0], PoolAlloc { offset: 64, size: 64 });
        // Free a: it merges right into b's block (a is the left neighbour, b the right).
        p.free(a);
        // Free list: [0,128) and the untouched tail [192,256).
        assert_eq!(p.free_list, vec![PoolAlloc { offset: 0, size: 128 }, PoolAlloc { offset: 192, size: 64 }]);
    }

    #[test]
    fn reuses_freed_space_first_fit() {
        let mut p = Pool::new(512, 64);
        let a = p.alloc(128).unwrap();
        let _b = p.alloc(64).unwrap();
        p.free(a);
        let c = p.alloc(128).unwrap();
        assert_eq!(c.offset, 0);
    }

    #[test]
    fn fragmentation_metric() {
        let mut p = Pool::new(256, 64);
        assert_eq!(p.fragmentation(), 0.0, "empty pool: one contiguous block");
        let a = p.alloc(64).unwrap();
        let _b = p.alloc(64).unwrap();
        let c = p.alloc(64).unwrap();
        let _d = p.alloc(64).unwrap();
        assert_eq!(p.fragmentation(), 0.0, "full pool");
        p.free(a);
        p.free(c);
        // 128 free bytes, but only 64 contiguous.
        assert_eq!(p.free_bytes(), 128);
        assert_eq!(p.largest_free(), 64);
        assert_eq!(p.fragmentation(), 0.5);
    }

    #[test]
    fn fragmentation_zero_when_contiguous() {
        let mut p = Pool::new(256, 64);
        let a = p.alloc(64).unwrap();
        let b = p.alloc(64).unwrap();
        p.free(a);
        // Free: [0,64) and [128,256). Largest is 128 of 192 free bytes.
        assert_eq!(p.free_bytes(), 192);
        assert_eq!(p.largest_free(), 128);
        assert!((p.fragmentation() - 1.0 / 3.0).abs() < 1e-6);
        // Freeing b joins everything into one block.
        p.free(b);
        assert_eq!(p.largest_free(), 256);
        assert_eq!(p.fragmentation(), 0.0);
    }

    #[test]
    fn deferred_retire_ordering() {
        let mut p = Pool::new(256, 64);
        let a = p.alloc(64).unwrap();
        let b = p.alloc(64).unwrap();
        p.free_deferred(a, 5);
        p.free_deferred(b, 3);
        assert_eq!(p.deferred_len(), 2);
        assert_eq!(p.used(), 128, "deferred ranges still count as used");

        p.retire(2);
        assert_eq!(p.deferred_len(), 2, "nothing completed yet");
        assert_eq!(p.used(), 128);

        p.retire(3);
        assert_eq!(p.deferred_len(), 1);
        assert_eq!(p.used(), 64, "frame 3 freed b only");

        p.retire(4);
        assert_eq!(p.deferred_len(), 1);
        assert_eq!(p.used(), 64);

        p.retire(10);
        assert_eq!(p.deferred_len(), 0);
        assert_eq!(p.used(), 0);
        assert_eq!(p.largest_free(), 256);
        check_invariants(&p, &[]);
    }

    #[test]
    fn deferred_range_not_reused_before_retire() {
        let mut p = Pool::new(128, 64);
        let a = p.alloc(128).unwrap();
        p.free_deferred(a, 1);
        assert!(p.alloc(64).is_none(), "GPU may still read the deferred range");
        p.retire(0);
        assert!(p.alloc(64).is_none());
        p.retire(1);
        assert!(p.alloc(128).is_some());
    }

    #[test]
    fn retire_keeps_queue_order_for_same_frame() {
        let mut p = Pool::new(256, 64);
        let a = p.alloc(64).unwrap();
        let b = p.alloc(64).unwrap();
        p.free_deferred(b, 7);
        p.free_deferred(a, 7);
        p.retire(7);
        assert_eq!(p.deferred_len(), 0);
        assert_eq!(p.used(), 0);
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "double free or overlap")]
    fn double_free_panics_in_debug() {
        let mut p = Pool::new(256, 64);
        let a = p.alloc(64).unwrap();
        p.free(a);
        p.free(a);
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "double free or overlap")]
    fn overlapping_free_panics_in_debug() {
        let mut p = Pool::new(256, 64);
        let _a = p.alloc(128).unwrap();
        let b = p.alloc(64).unwrap();
        p.free(b);
        // Overlaps the already-free block `b`.
        p.free(PoolAlloc { offset: 128, size: 128 });
    }

    #[test]
    fn randomized_stress_no_overlap() {
        const CAP: u64 = 1 << 16;
        const ALIGN: u64 = 16;
        let mut p = Pool::new(CAP, ALIGN);
        let mut live: Vec<PoolAlloc> = Vec::new();
        let mut rng = Lcg(0xC0FF_EE00_1234_5678);
        let mut successes = 0u32;
        let mut failures = 0u32;

        for step in 0..20_000u64 {
            let r = rng.next();
            if live.is_empty() || r % 3 != 0 {
                let size = 1 + rng.next() % 2048;
                match p.alloc(size) {
                    Some(a) => {
                        assert!(a.size >= size);
                        assert_eq!(a.offset % ALIGN, 0);
                        assert_eq!(a.size % ALIGN, 0);
                        live.push(a);
                        successes += 1;
                    }
                    None => failures += 1,
                }
            } else {
                let i = (rng.next() as usize) % live.len();
                let a = live.swap_remove(i);
                if rng.next() % 4 == 0 {
                    p.free_deferred(a, step / 50);
                } else {
                    p.free(a);
                }
            }
            if step % 50 == 0 {
                p.retire(step / 50);
            }
            if step % 97 == 0 {
                check_invariants(&p, &live);
            }
        }

        // Drain everything and confirm the pool returns to one free block.
        for a in live.drain(..) {
            p.free(a);
        }
        p.retire(u64::MAX);
        check_invariants(&p, &[]);
        assert_eq!(p.free_list, vec![PoolAlloc { offset: 0, size: CAP }]);
        assert!(successes > 0 && failures > 0, "stress should hit both paths");
    }
}
