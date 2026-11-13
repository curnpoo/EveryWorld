//! Trunk collision and felling for the near trees (G10).
//!
//! Standing trunks are static capsules in the sim, for the resident trees within [`RADIUS_M`]
//! of the focus (the 3 x 3 terrain block reaches at least ~150 m), added in sorted `ObjectId`
//! order and dropped beyond [`DROP_M`] (hysteresis). The set depends only on the focus path
//! and the stands, which are pure functions of their nodes. The override records and the
//! felling live in [`ed_sim::Trunks`]; this file only chooses which trunks exist.

use ed_geo::ObjectId;
use ed_sim::{TrunkDesc, Trunks, World};
use glam::DVec3;
use std::collections::{BTreeSet, VecDeque};

/// Trunks this near the focus (m) get a body.
pub const RADIUS_M: f64 = 90.0;
/// Trunks beyond this (m) lose it.
pub const DROP_M: f64 = 120.0;
/// The set is rebuilt after the focus moves this far (m)...
const MOVE_M: f64 = 8.0;
/// ...or after this many frames.
const EVERY_FRAMES: u32 = 120;
/// Scan work per frame (see `NearTrees::trunk_scan_step`).
pub const SCAN_UNITS: i32 = 3000;
/// Bodies added, and bodies removed, per frame at most.
pub const OPS_PER_FRAME: usize = 16;

#[derive(Default)]
pub struct TreeCollision {
    pub trunks: Trunks,
    last: Option<DVec3>,
    since: u32,
    nodes: usize,
    /// A rescan was requested while one was running.
    want: bool,
    /// Where the running scan started.
    scan_focus: DVec3,
    add_q: VecDeque<TrunkDesc>,
    rm_q: VecDeque<ObjectId>,
    pub stats: TreeCollisionStats,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct TreeCollisionStats {
    pub standing: usize,
    pub added: usize,
    pub removed: usize,
    pub last_ms: f64,
    /// Most bodies added plus removed in one frame so far.
    pub max_ops: usize,
    pub pending: usize,
}

impl TreeCollision {
    pub fn new() -> TreeCollision {
        TreeCollision::default()
    }

    /// Whether the candidate set should be recomputed now (the focus moved, the stands changed
    /// or a while passed). The caller then asks the stands for [`RADIUS_M`] and calls
    /// [`TreeCollision::apply`].
    pub fn due(&mut self, focus: DVec3, nodes: usize) -> bool {
        self.since += 1;
        let moved = self.last.is_none_or(|l| (l - focus).length() > MOVE_M);
        if moved || nodes != self.nodes || self.since >= EVERY_FRAMES {
            self.last = Some(focus);
            self.nodes = nodes;
            self.since = 0;
            true
        } else {
            false
        }
    }

    /// Brings the static trunks in line with `cands` (sorted by id, all within the radius of
    /// `focus`): drops standing trunks beyond [`DROP_M`] or no longer resident, adds the rest.
    pub fn apply(&mut self, world: &mut World, focus: DVec3, cands: &[TrunkDesc]) {
        let t0 = std::time::Instant::now();
        let keep: BTreeSet<ObjectId> = cands.iter().filter(|d| (d.base - focus).truncate().length() <= DROP_M).map(|d| d.id).collect();
        let drop: Vec<ObjectId> = self.trunks.standing().filter(|id| !keep.contains(id)).collect();
        let mut removed = 0;
        for id in drop {
            removed += self.trunks.remove(world, id) as usize;
        }
        let mut added = 0;
        for d in cands {
            if (d.base - focus).truncate().length() <= RADIUS_M {
                added += self.trunks.add(world, *d) as usize;
            }
        }
        self.stats = TreeCollisionStats { standing: self.trunks.standing_count(), added, removed, last_ms: t0.elapsed().as_secs_f64() * 1e3, ..self.stats };
    }

    /// Notes that a rescan is wanted (call [`TreeCollision::due`] first).
    pub fn request(&mut self) {
        self.want = true;
    }

    pub fn scanning_wanted(&self) -> bool {
        self.want
    }

    /// Takes the wanted rescan: the caller starts a scan at `focus`.
    pub fn begin_scan(&mut self, focus: DVec3) {
        self.want = false;
        self.scan_focus = focus;
    }

    /// A finished scan: queues the difference against the standing trunks. Removals are by id;
    /// additions nearest-first from the scan focus, ties by id (total order, so the same on
    /// every run).
    pub fn finish_scan(&mut self, cands: &[TrunkDesc]) {
        let f = self.scan_focus;
        let keep: BTreeSet<ObjectId> = cands.iter().filter(|d| (d.base - f).truncate().length() <= DROP_M).map(|d| d.id).collect();
        self.rm_q = self.trunks.standing().filter(|id| !keep.contains(id)).collect();
        let mut add: Vec<(f64, TrunkDesc)> = cands
            .iter()
            .filter(|d| !self.trunks.is_standing(d.id))
            .map(|d| ((d.base - f).truncate().length(), *d))
            .filter(|(m, _)| *m <= RADIUS_M)
            .collect();
        add.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.id.cmp(&b.1.id)));
        self.add_q = add.into_iter().map(|x| x.1).collect();
    }

    /// Applies at most [`OPS_PER_FRAME`] queued adds and as many removals. The queues are
    /// built from sorted data and drained in order by count, so which bodies exist after frame
    /// N depends only on the focus path, the stands and N. Adds beyond [`DROP_M`] of the
    /// current `focus` are skipped. Returns bodies touched.
    pub fn drain(&mut self, world: &mut World, focus: DVec3) -> usize {
        let (mut added, mut removed) = (0, 0);
        while removed < OPS_PER_FRAME {
            let Some(id) = self.rm_q.pop_front() else { break };
            removed += self.trunks.remove(world, id) as usize;
        }
        let mut tried = 0;
        while added < OPS_PER_FRAME && tried < OPS_PER_FRAME * 4 {
            let Some(d) = self.add_q.pop_front() else { break };
            tried += 1;
            if (d.base - focus).truncate().length() <= DROP_M {
                added += self.trunks.add(world, d) as usize;
            }
        }
        let ops = added + removed;
        self.stats.standing = self.trunks.standing_count();
        self.stats.added += added;
        self.stats.removed += removed;
        self.stats.max_ops = self.stats.max_ops.max(ops);
        self.stats.pending = self.add_q.len() + self.rm_q.len();
        ops
    }
}
