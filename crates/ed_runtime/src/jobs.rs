//! Keyed job pools with priorities and cancellation.
//!
//! Bevy's `AsyncComputeTaskPool` is first come, first served, gets about a
//! quarter of the cores by default, and is shared by every system. Tile
//! builds there blocked on network IO while near-camera work queued behind
//! them. A [`JobPool`] fixes the three things streaming needs:
//!
//! - **Priority**: the lowest value runs first, and it can be changed while
//!   queued ([`JobPool::set_priority`]) as the camera moves.
//! - **Cancellation**: queued jobs that went stale are dropped before they
//!   cost anything ([`JobPool::cancel`], [`JobPool::retain_queued`]).
//! - **Separate pools**: a wide IO pool for downloads and a narrow CPU pool
//!   for generation, so neither starves the other.
//!
//! Each key is in the pool at most once (queued or running). Results come
//! back through [`JobPool::try_recv`]; a job that panicked returns `None`.

use crossbeam_channel::{Receiver, Sender};
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::panic::AssertUnwindSafe;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

type Job<T> = Box<dyn FnOnce() -> T + Send + 'static>;

struct Queued<T> {
    priority: f32,
    seq: u64,
    job: Job<T>,
}

struct State<K, T> {
    queue: HashMap<K, Queued<T>>,
    running: HashSet<K>,
    seq: u64,
    shutdown: bool,
}

struct Shared<K, T> {
    state: Mutex<State<K, T>>,
    wake: Condvar,
    tx: Sender<(K, Option<T>)>,
}

/// A fixed set of worker threads running keyed jobs by priority.
pub struct JobPool<K, T> {
    shared: Arc<Shared<K, T>>,
    rx: Receiver<(K, Option<T>)>,
    threads: Vec<JoinHandle<()>>,
}

impl<K, T> JobPool<K, T>
where
    K: Hash + Eq + Clone + Send + 'static,
    T: Send + 'static,
{
    /// Start `threads` workers (at least one), named `{name}-{i}`.
    pub fn new(name: &str, threads: usize) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        let shared = Arc::new(Shared {
            state: Mutex::new(State { queue: HashMap::new(), running: HashSet::new(), seq: 0, shutdown: false }),
            wake: Condvar::new(),
            tx,
        });
        let threads = (0..threads.max(1))
            .map(|i| {
                let s = shared.clone();
                std::thread::Builder::new().name(format!("{name}-{i}")).spawn(move || worker(&s)).expect("spawn job worker")
            })
            .collect();
        Self { shared, rx, threads }
    }

    /// Queue `job` under `key`. Returns false (and drops `job`) if the key is
    /// already queued or running; use [`Self::set_priority`] to re-rank it.
    pub fn submit(&self, key: K, priority: f32, job: impl FnOnce() -> T + Send + 'static) -> bool {
        let mut st = self.shared.state.lock().unwrap();
        if st.queue.contains_key(&key) || st.running.contains(&key) {
            return false;
        }
        st.seq += 1;
        let seq = st.seq;
        st.queue.insert(key, Queued { priority, seq, job: Box::new(job) });
        drop(st);
        self.shared.wake.notify_one();
        true
    }

    /// Change a queued job's priority. No effect once it is running.
    pub fn set_priority(&self, key: &K, priority: f32) {
        if let Some(q) = self.shared.state.lock().unwrap().queue.get_mut(key) {
            q.priority = priority;
        }
    }

    /// Drop a queued job. Running jobs finish (their result still arrives).
    pub fn cancel(&self, key: &K) -> bool {
        self.shared.state.lock().unwrap().queue.remove(key).is_some()
    }

    /// Cancel every queued job whose key fails `keep`. Returns how many.
    pub fn retain_queued(&self, mut keep: impl FnMut(&K) -> bool) -> usize {
        let mut st = self.shared.state.lock().unwrap();
        let before = st.queue.len();
        st.queue.retain(|k, _| keep(k));
        before - st.queue.len()
    }

    /// Queued or running.
    pub fn contains(&self, key: &K) -> bool {
        let st = self.shared.state.lock().unwrap();
        st.queue.contains_key(key) || st.running.contains(key)
    }

    pub fn is_running(&self, key: &K) -> bool {
        self.shared.state.lock().unwrap().running.contains(key)
    }

    /// (queued, running)
    pub fn load(&self) -> (usize, usize) {
        let st = self.shared.state.lock().unwrap();
        (st.queue.len(), st.running.len())
    }

    /// Every key queued or running right now.
    pub fn keys(&self) -> Vec<K> {
        let st = self.shared.state.lock().unwrap();
        st.queue.keys().chain(st.running.iter()).cloned().collect()
    }

    pub fn threads(&self) -> usize {
        self.threads.len()
    }

    /// Next finished job, if any. `None` in the value means it panicked.
    pub fn try_recv(&self) -> Option<(K, Option<T>)> {
        self.rx.try_recv().ok()
    }
}

fn worker<K: Hash + Eq + Clone, T>(s: &Shared<K, T>) {
    loop {
        let (key, job) = {
            let mut st = s.state.lock().unwrap();
            loop {
                if st.shutdown {
                    return;
                }
                if !st.queue.is_empty() {
                    break;
                }
                st = s.wake.wait(st).unwrap();
            }
            // Lowest priority value first; FIFO among equals. Queues hold at
            // most a few hundred tiles, so a scan beats keeping a heap in sync
            // with re-prioritisation.
            let key = st
                .queue
                .iter()
                .min_by(|a, b| a.1.priority.total_cmp(&b.1.priority).then(a.1.seq.cmp(&b.1.seq)))
                .map(|(k, _)| k.clone())
                .unwrap();
            let q = st.queue.remove(&key).unwrap();
            st.running.insert(key.clone());
            (key, q.job)
        };
        let out = std::panic::catch_unwind(AssertUnwindSafe(job));
        if out.is_err() {
            log::error!("job panicked on {}", std::thread::current().name().unwrap_or("worker"));
        }
        // Clear `running` and publish the result under the lock, so a caller
        // that sees the key as idle can already receive its result.
        let mut st = s.state.lock().unwrap();
        st.running.remove(&key);
        let _ = s.tx.send((key, out.ok()));
    }
}

impl<K, T> Drop for JobPool<K, T> {
    fn drop(&mut self) {
        if let Ok(mut st) = self.shared.state.lock() {
            st.shutdown = true;
            st.queue.clear();
        }
        self.shared.wake.notify_all();
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
}

/// Thread counts for the default pools on this machine: (io, cpu).
///
/// CPU workers leave cores for Bevy's main and render threads and its own
/// compute pool (ground cover, building detail). IO threads mostly wait on
/// sockets, so there are more of them.
pub fn default_threads() -> (usize, usize) {
    let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
    let cpu = cores.saturating_sub(3).clamp(1, 8);
    (8, cpu)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};

    fn drain<K: Hash + Eq + Clone + Send + 'static, T: Send + 'static>(p: &JobPool<K, T>, n: usize) -> Vec<(K, Option<T>)> {
        let t0 = Instant::now();
        let mut out = Vec::new();
        while out.len() < n {
            assert!(t0.elapsed() < Duration::from_secs(10), "timed out");
            match p.try_recv() {
                Some(r) => out.push(r),
                None => std::thread::sleep(Duration::from_millis(1)),
            }
        }
        out
    }

    /// Hold the single worker busy until the gate opens.
    fn block(p: &JobPool<u32, u32>, gate: &Arc<AtomicBool>) {
        let g = gate.clone();
        assert!(p.submit(0, 0.0, move || {
            while !g.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(1));
            }
            0
        }));
        while !p.is_running(&0) {
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn runs_in_priority_order() {
        let p: JobPool<u32, u32> = JobPool::new("t", 1);
        let gate = Arc::new(AtomicBool::new(false));
        block(&p, &gate);
        for (k, pr) in [(1, 5.0), (2, 1.0), (3, 3.0), (4, 1.0)] {
            assert!(p.submit(k, pr, move || k));
        }
        gate.store(true, Ordering::SeqCst);
        let order: Vec<u32> = drain(&p, 5).into_iter().map(|(k, _)| k).collect();
        // FIFO between the two priority-1 jobs.
        assert_eq!(order, vec![0, 2, 4, 3, 1]);
    }

    #[test]
    fn reprioritise_and_cancel() {
        let p: JobPool<u32, u32> = JobPool::new("t", 1);
        let gate = Arc::new(AtomicBool::new(false));
        block(&p, &gate);
        for k in 1..=4 {
            p.submit(k, k as f32, move || k);
        }
        p.set_priority(&4, 0.0);
        assert!(p.cancel(&2));
        assert_eq!(p.retain_queued(|k| *k != 3), 1);
        gate.store(true, Ordering::SeqCst);
        let order: Vec<u32> = drain(&p, 3).into_iter().map(|(k, _)| k).collect();
        assert_eq!(order, vec![0, 4, 1]);
        std::thread::sleep(Duration::from_millis(20));
        assert!(p.try_recv().is_none());
    }

    #[test]
    fn one_entry_per_key() {
        let p: JobPool<u32, u32> = JobPool::new("t", 1);
        let gate = Arc::new(AtomicBool::new(false));
        block(&p, &gate);
        assert!(!p.submit(0, 0.0, || 9), "running key rejected");
        assert!(p.submit(7, 1.0, || 7));
        assert!(!p.submit(7, 0.0, || 8), "queued key rejected");
        assert!(p.contains(&7));
        gate.store(true, Ordering::SeqCst);
        let mut got: Vec<(u32, Option<u32>)> = drain(&p, 2);
        got.sort();
        assert_eq!(got, vec![(0, Some(0)), (7, Some(7))]);
        assert!(!p.contains(&7));
    }

    #[test]
    fn panics_are_reported_and_pool_survives() {
        let p: JobPool<u32, u32> = JobPool::new("t", 2);
        p.submit(1, 0.0, || panic!("boom"));
        p.submit(2, 1.0, || 2);
        let mut got = drain(&p, 2);
        got.sort();
        assert_eq!(got, vec![(1, None), (2, Some(2))]);
    }

    #[test]
    fn uses_all_threads() {
        let p: JobPool<u32, ()> = JobPool::new("t", 4);
        let t0 = Instant::now();
        for k in 0..4 {
            p.submit(k, 0.0, || std::thread::sleep(Duration::from_millis(100)));
        }
        drain(&p, 4);
        assert!(t0.elapsed() < Duration::from_millis(350), "jobs should overlap: {:?}", t0.elapsed());
    }
}
