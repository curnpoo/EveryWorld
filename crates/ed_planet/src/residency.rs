//! What is resident: the `Residency` trait the loader implements, an in-memory fake for tests,
//! and the CPU-side chunk cache LRU (by bytes).

use crate::page::HeightPage;
use ed_geo::NodeId;
use ed_pack::ChunkKind;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

/// Per-node chunk bits, one bit per `ChunkKind` id (`1 << kind.id()`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ChunkMask {
    /// Chunks the manifest declares required for this node's rung (P2-D6).
    pub required: u64,
    /// Requested from a source and not yet arrived.
    pub requested: u64,
    /// Decoded in CPU memory.
    pub resident: u64,
    /// Uploaded to the GPU (committed at a frame boundary).
    pub uploaded: u64,
}

impl ChunkMask {
    pub fn bit(kind: ChunkKind) -> u64 {
        1u64 << kind.id()
    }
    /// The Phase 2 terrain rung: one `terrain` chunk.
    pub fn terrain() -> ChunkMask {
        ChunkMask { required: Self::bit(ChunkKind::Terrain), ..Default::default() }
    }
    /// All required chunks are resident and uploaded. A node with no declared requirement is
    /// never complete (its manifest is not known yet).
    pub fn complete(&self) -> bool {
        self.required != 0 && self.resident & self.uploaded & self.required == self.required
    }
    pub fn state(&self) -> NodeState {
        if self.complete() {
            NodeState::Uploaded
        } else if self.required != 0 && self.resident & self.required == self.required {
            NodeState::Resident
        } else if self.requested | self.resident != 0 {
            NodeState::Requested
        } else {
            NodeState::Absent
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NodeState {
    #[default]
    Absent,
    Requested,
    Resident,
    Uploaded,
}

/// What the loader knows about nodes. No IO happens through this trait; it is a view of the
/// loader's tables. Every answer must be a deterministic function of the loader's state.
pub trait Residency {
    /// Bytes the node's required chunks take when resident: the manifest sizes, or a fixed
    /// estimate before the manifest is known. Must be greater than zero.
    fn bytes(&self, id: NodeId) -> u64;
    fn chunks(&self, id: NodeId) -> ChunkMask;
    /// The decoded height page, when the `terrain` chunk is resident.
    fn height_page(&self, id: NodeId) -> Option<Arc<HeightPage>>;
    /// The node's slot in the GPU page atlas, when uploaded.
    fn page_slot(&self, id: NodeId) -> Option<u32>;
    /// The node's canopy page (trees design §0), when built. Optional: a node without one draws
    /// bare ground (no forest), never a hole.
    fn canopy_page(&self, _id: NodeId) -> Option<Arc<ed_geo::CanopyPage>> {
        None
    }

    fn complete(&self, id: NodeId) -> bool {
        self.chunks(id).complete()
    }
}

#[derive(Clone, Debug)]
struct FakeNode {
    mask: ChunkMask,
    page: Option<Arc<HeightPage>>,
    slot: Option<u32>,
}

/// In-memory residency for tests and tools. Nodes not listed are absent with the default
/// byte size, or the size from `bytes_fn` when set.
pub struct FakeResidency {
    pub default_bytes: u64,
    pub bytes_fn: Option<Box<dyn Fn(NodeId) -> u64>>,
    /// When set, a node not listed answers with this (lets tests make "everything complete").
    pub complete_fn: Option<Box<dyn Fn(NodeId) -> bool>>,
    nodes: BTreeMap<NodeId, FakeNode>,
    next_slot: u32,
}

impl Default for FakeResidency {
    fn default() -> Self {
        FakeResidency::new(300_000)
    }
}

impl FakeResidency {
    pub fn new(default_bytes: u64) -> FakeResidency {
        FakeResidency { default_bytes, bytes_fn: None, complete_fn: None, nodes: BTreeMap::new(), next_slot: 0 }
    }

    /// Makes a node complete (resident and uploaded) with an optional height page.
    pub fn insert(&mut self, id: NodeId, page: Option<HeightPage>) {
        let mut mask = ChunkMask::terrain();
        mask.resident = mask.required;
        mask.uploaded = mask.required;
        let slot = Some(self.next_slot);
        self.next_slot += 1;
        self.nodes.insert(id, FakeNode { mask, page: page.map(Arc::new), slot });
    }

    /// Sets a node's mask (for partial states).
    pub fn set_mask(&mut self, id: NodeId, mask: ChunkMask) {
        self.nodes.entry(id).or_insert(FakeNode { mask, page: None, slot: None }).mask = mask;
    }

    pub fn remove(&mut self, id: NodeId) {
        self.nodes.remove(&id);
    }
}

impl Residency for FakeResidency {
    fn bytes(&self, id: NodeId) -> u64 {
        self.bytes_fn.as_ref().map_or(self.default_bytes, |f| f(id)).max(1)
    }
    fn chunks(&self, id: NodeId) -> ChunkMask {
        if let Some(n) = self.nodes.get(&id) {
            return n.mask;
        }
        let mut m = ChunkMask::terrain();
        if self.complete_fn.as_ref().is_some_and(|f| f(id)) {
            m.resident = m.required;
            m.uploaded = m.required;
        }
        m
    }
    fn height_page(&self, id: NodeId) -> Option<Arc<HeightPage>> {
        self.nodes.get(&id).and_then(|n| n.page.clone())
    }
    fn page_slot(&self, id: NodeId) -> Option<u32> {
        self.nodes.get(&id).and_then(|n| n.slot)
    }
}

/// CPU chunk cache bookkeeping: least recently wanted first, by bytes.
///
/// Lookups use a hash map; eviction walks a `BTreeSet` of `(stamp, NodeId)`, so the order is
/// deterministic. Wanted nodes are never evicted (P2 §2: "nothing wanted is evicted").
#[derive(Default, Debug)]
pub struct ByteLru {
    entries: HashMap<NodeId, (u64, u64)>,
    order: BTreeSet<(u64, NodeId)>,
    total: u64,
}

impl ByteLru {
    pub fn new() -> ByteLru {
        ByteLru::default()
    }

    /// Records that `id` (taking `bytes`) was wanted at `stamp` (a frame number).
    pub fn touch(&mut self, id: NodeId, bytes: u64, stamp: u64) {
        if let Some((b, s)) = self.entries.insert(id, (bytes, stamp)) {
            self.order.remove(&(s, id));
            self.total -= b;
        }
        self.order.insert((stamp, id));
        self.total += bytes;
    }

    pub fn remove(&mut self, id: NodeId) -> bool {
        match self.entries.remove(&id) {
            Some((b, s)) => {
                self.order.remove(&(s, id));
                self.total -= b;
                true
            }
            None => false,
        }
    }

    pub fn total_bytes(&self) -> u64 {
        self.total
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn contains(&self, id: NodeId) -> bool {
        self.entries.contains_key(&id)
    }

    /// Removes least recently wanted entries until the total is at most `cap`, skipping any
    /// for which `wanted` is true. Returns the evicted ids in eviction order.
    pub fn evict_to(&mut self, cap: u64, wanted: impl Fn(NodeId) -> bool) -> Vec<NodeId> {
        let mut out = Vec::new();
        if self.total <= cap {
            return out;
        }
        let mut over = self.total - cap;
        for &(_, id) in &self.order {
            if over == 0 {
                break;
            }
            if wanted(id) {
                continue;
            }
            let b = self.entries[&id].0;
            out.push(id);
            over = over.saturating_sub(b);
        }
        for &id in &out {
            self.remove(id);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed_geo::BodyId;

    fn n(x: u32) -> NodeId {
        NodeId::new(BodyId(3), 0, 5, x, 0).unwrap()
    }

    #[test]
    fn lru_evicts_oldest_unwanted() {
        let mut l = ByteLru::new();
        for i in 0..5 {
            l.touch(n(i), 100, i as u64);
        }
        l.touch(n(0), 100, 10);
        assert_eq!(l.total_bytes(), 500);
        let ev = l.evict_to(250, |id| id == n(1));
        assert_eq!(ev, vec![n(2), n(3), n(4)]);
        assert_eq!(l.total_bytes(), 200);
        assert!(l.contains(n(0)) && l.contains(n(1)));
    }

    #[test]
    fn mask_states() {
        let mut m = ChunkMask::terrain();
        assert_eq!(m.state(), NodeState::Absent);
        m.requested = m.required;
        assert_eq!(m.state(), NodeState::Requested);
        m.resident = m.required;
        assert_eq!(m.state(), NodeState::Resident);
        m.uploaded = m.required;
        assert_eq!(m.state(), NodeState::Uploaded);
        assert!(m.complete());
        assert!(!ChunkMask::default().complete());
    }
}
