//! Node records: a slab `Vec<Node>` with a `NodeId -> index` map for lookup only.
//!
//! Nothing iterates the map or depends on slab order for output: callers iterate sorted
//! `NodeId` collections and look nodes up here.

use crate::body::Body;
use crate::residency::{ChunkMask, NodeState};
use ed_geo::NodeId;
use glam::DVec3;
use std::collections::HashMap;

/// What the planet knows about one node.
#[derive(Clone, Debug)]
pub struct Node {
    pub id: NodeId,
    pub state: NodeState,
    /// Bytes of the node's required chunks (from `Residency::bytes`).
    pub bytes: u64,
    /// Height range used for bounds (m above the ellipsoid). Exact once the node's height page
    /// is resident; until then inherited from the parent, padded (see `inherit`).
    pub min_h: f32,
    pub max_h: f32,
    /// True when `min_h`/`max_h` come from this node's own page.
    pub h_known: bool,
    /// Bounding sphere, body-fixed f64 centre.
    pub bound_centre: DVec3,
    pub bound_radius: f32,
    pub last_wanted_frame: u64,
    pub chunk_mask: ChunkMask,
    /// Approximate edge length of the node at the surface (m).
    pub edge_m: f64,
    /// Unit direction of the node centre.
    pub centre_dir: DVec3,
    /// Corners, edge midpoints and centre (unit directions) for bounds.
    dirs: [DVec3; 9],
}

impl Node {
    fn new(id: NodeId, body: &Body, min_h: f32, max_h: f32) -> Node {
        let mut dirs = [DVec3::ZERO; 9];
        for j in 0..3u32 {
            for i in 0..3u32 {
                dirs[(j * 3 + i) as usize] = id.grid_point(i, j, 2);
            }
        }
        let centre_dir = dirs[4];
        let edge_m = (dirs[0] - dirs[2]).length().max((dirs[0] - dirs[6]).length()) * body.radius(centre_dir);
        let mut n = Node {
            id,
            state: NodeState::Absent,
            bytes: 0,
            min_h,
            max_h,
            h_known: false,
            bound_centre: DVec3::ZERO,
            bound_radius: 0.0,
            last_wanted_frame: 0,
            chunk_mask: ChunkMask::default(),
            edge_m,
            centre_dir,
            dirs,
        };
        n.rebound(body);
        n
    }

    /// Recomputes the bounding sphere from the 9 sample directions at both heights. The 2 %
    /// pad covers the surface bulging between samples (largest at L0, where samples are 45°
    /// apart; the farthest point of a cap from its centre is on its boundary).
    fn rebound(&mut self, body: &Body) {
        let mid = (self.min_h as f64 + self.max_h as f64) * 0.5;
        let c = body.point(self.centre_dir, mid);
        let mut r2: f64 = 0.0;
        for d in self.dirs {
            for h in [self.min_h as f64, self.max_h as f64] {
                r2 = r2.max((body.point(d, h) - c).length_squared());
            }
        }
        self.bound_centre = c;
        self.bound_radius = (r2.sqrt() * 1.02 + 1.0) as f32;
    }

    /// Geometric error of drawing this node's grid of `cells` cells (m).
    pub fn error_m(&self, cells: f64) -> f64 {
        self.edge_m / cells
    }

    /// Height range a child inherits while its own page is absent: this node's range widened
    /// by `edge / 64`. A box-filtered parent can lose sub-cell peaks; at slopes up to 2:1
    /// that loss summed over all finer levels stays under `edge / 64`.
    pub fn inherit(&self, body: &Body) -> (f32, f32) {
        let pad = (self.edge_m / 64.0) as f32;
        ((self.min_h - pad).max(body.min_h), (self.max_h + pad).min(body.max_h))
    }

    /// Sets the height range; recomputes bounds only when it changed.
    pub(crate) fn set_range(&mut self, body: &Body, min_h: f32, max_h: f32, known: bool) {
        self.h_known = known;
        if min_h != self.min_h || max_h != self.max_h {
            self.min_h = min_h;
            self.max_h = max_h;
            self.rebound(body);
        }
    }
}

/// Slab of node records.
#[derive(Default, Debug)]
pub struct Quadtree {
    nodes: Vec<Option<Node>>,
    free: Vec<u32>,
    index: HashMap<NodeId, u32>,
}

impl Quadtree {
    pub fn new() -> Quadtree {
        Quadtree::default()
    }

    pub fn len(&self) -> usize {
        self.index.len()
    }
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.index.get(&id).map(|&i| self.nodes[i as usize].as_ref().expect("live slot"))
    }

    pub(crate) fn slot(&self, id: NodeId) -> Option<u32> {
        self.index.get(&id).copied()
    }
    pub(crate) fn at(&self, i: u32) -> &Node {
        self.nodes[i as usize].as_ref().expect("live slot")
    }
    pub(crate) fn at_mut(&mut self, i: u32) -> &mut Node {
        self.nodes[i as usize].as_mut().expect("live slot")
    }

    /// The node's slot, creating it with the given height range if absent.
    pub(crate) fn ensure(&mut self, id: NodeId, body: &Body, range: (f32, f32)) -> u32 {
        if let Some(&i) = self.index.get(&id) {
            return i;
        }
        let node = Node::new(id, body, range.0, range.1);
        let i = match self.free.pop() {
            Some(i) => {
                self.nodes[i as usize] = Some(node);
                i
            }
            None => {
                self.nodes.push(Some(node));
                (self.nodes.len() - 1) as u32
            }
        };
        self.index.insert(id, i);
        i
    }

    /// Drops records not wanted since `before` (a frame number). Slab order only decides
    /// slot reuse, never output.
    pub(crate) fn collect(&mut self, before: u64) {
        for i in 0..self.nodes.len() {
            if let Some(n) = &self.nodes[i]
                && n.last_wanted_frame < before
            {
                self.index.remove(&n.id);
                self.nodes[i] = None;
                self.free.push(i as u32);
            }
        }
    }

    /// All ids, sorted.
    pub fn ids(&self) -> Vec<NodeId> {
        let mut v: Vec<NodeId> = self.index.keys().copied().collect();
        v.sort_unstable();
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed_geo::BodyId;

    #[test]
    fn bound_contains_surface_samples() {
        let body = Body::earth();
        let mut q = Quadtree::new();
        for level in [0u8, 1, 4, 10, 16] {
            let id = NodeId::new(BodyId(3), 2, level, 0, (1 << level) - 1).unwrap();
            let slot = q.ensure(id, &body, (-100.0, 3000.0));
            let n = q.at(slot).clone();
            for j in 0..=16 {
                for i in 0..=16 {
                    let d = id.grid_point(i, j, 16);
                    for h in [-100.0, 3000.0] {
                        let dist = (body.point(d, h) - n.bound_centre).length();
                        assert!(dist <= n.bound_radius as f64, "L{level} {dist} > {}", n.bound_radius);
                    }
                }
            }
        }
    }
}
