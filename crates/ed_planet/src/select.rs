//! The budgeted selector (P2-D4), culls (P2-D5), parent-until-children draw set (P2-D6),
//! camera-relative output (P2-D7) and the height query (P2-D3).

use crate::body::{Body, BodyPose};
use crate::cull::{Frustum, Horizon};
use crate::page::FacePos;
use crate::quadtree::Quadtree;
use crate::residency::{ByteLru, NodeState, Residency};
use ed_geo::{BodyId, NodeId};
use glam::{DMat3, DQuat, DVec3, Mat3, Vec3};
use std::cmp::Ordering;
use std::collections::{BTreeSet, BinaryHeap};

/// One camera, in the world frame (f64).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub position: DVec3,
    /// Camera to world. The camera looks down its -Z with +Y up.
    pub rotation: DQuat,
    pub velocity: DVec3,
    /// Vertical field of view (radians).
    pub fov_y: f64,
    /// Viewport in pixels (width, height).
    pub viewport: [u32; 2],
    /// Seconds along `velocity` for the look-ahead eye.
    pub look_ahead_s: f64,
}

impl View {
    pub fn look_ahead(&self) -> DVec3 {
        self.position + self.velocity * self.look_ahead_s
    }
    /// Pixels per radian at the image centre: `height / (2 tan(fov / 2))`.
    pub fn pixels_per_radian(&self) -> f64 {
        self.viewport[1].max(1) as f64 / (2.0 * libm::tan(self.fov_y * 0.5))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelectConfig {
    /// Residency budget: the wanted set's bytes never exceed it (unless the roots alone do).
    pub budget_bytes: u64,
    /// Target projected error (pixels). The error proxy is the cell size of the node's draw
    /// grid (`edge / error_cells`), not a measured height deviation, so it is pessimistic for
    /// smooth ground; 16 px gives ~600 draw nodes at 2 km altitude, 1440p.
    pub pixel_error: f64,
    /// Threshold factor to split a node that was a leaf last frame.
    pub split: f64,
    /// Threshold factor to keep a node split that was split last frame.
    pub merge: f64,
    /// Cells across a node used for its geometric error: `edge / error_cells` (P2-D1 grid).
    pub error_cells: f64,
    /// Node records not wanted for this many frames are dropped.
    pub keep_frames: u64,
}

impl Default for SelectConfig {
    fn default() -> Self {
        SelectConfig { budget_bytes: 512 << 20, pixel_error: 16.0, split: 1.0, merge: 0.8, error_cells: 64.0, keep_frames: 120 }
    }
}

/// One node to draw this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawNode {
    pub id: NodeId,
    /// Node origin (centre on the ellipsoid) minus the camera, world axes, computed in f64.
    pub origin: Vec3,
    /// East, north, up at the node origin, world axes.
    pub basis: Mat3,
    /// Camera distance (m) where the morph toward the parent's lattice starts and ends
    /// (CDLOD, P2-D2). `f32::MAX` for roots.
    pub morph: [f32; 2],
    /// GPU page atlas slot, `u32::MAX` if the residency has none.
    pub page_slot: u32,
}

/// The selector's output for one frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Selection {
    pub frame: u64,
    /// Every node in the selected tree (interior and leaves), sorted. The loader keeps these
    /// resident and requests the absent ones.
    pub wanted: Vec<NodeId>,
    pub wanted_bytes: u64,
    /// Interior nodes of the selected tree, sorted.
    pub split: Vec<NodeId>,
    /// Nodes to draw, sorted by id; every one is complete.
    pub draw: Vec<DrawNode>,
    /// Roots with nothing drawable (their own chunks are not complete yet).
    pub holes: Vec<NodeId>,
    /// Split candidates rejected because the budget would be exceeded.
    pub budget_limited: u32,
}

#[derive(Clone, Copy, PartialEq)]
struct Cand {
    prio: f64,
    id: NodeId,
}
impl Eq for Cand {}
impl Ord for Cand {
    fn cmp(&self, o: &Self) -> Ordering {
        self.prio.total_cmp(&o.prio).then_with(|| o.id.cmp(&self.id))
    }
}
impl PartialOrd for Cand {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

struct Ctx {
    body: usize,
    pose: BodyPose,
    eye: DVec3,
    ahead: DVec3,
    horizon: Horizon,
}

/// Edge neighbours at the same level.
pub fn edge_neighbours(n: NodeId) -> [NodeId; 4] {
    [n.neighbour(1, 0), n.neighbour(-1, 0), n.neighbour(0, 1), n.neighbour(0, -1)]
}

/// The planet runtime: bodies, node records, the CPU cache LRU and last frame's split set.
pub struct Planet {
    bodies: Vec<Body>,
    tree: Quadtree,
    frame: u64,
    prev_split: BTreeSet<NodeId>,
    wanted: BTreeSet<NodeId>,
    /// CPU chunk cache bookkeeping; resident wanted nodes are touched every frame.
    pub cache: ByteLru,
}

impl Planet {
    pub fn new(mut bodies: Vec<Body>) -> Planet {
        bodies.sort_by_key(|b| b.id);
        bodies.dedup_by_key(|b| b.id);
        Planet { bodies, tree: Quadtree::new(), frame: 0, prev_split: BTreeSet::new(), wanted: BTreeSet::new(), cache: ByteLru::new() }
    }

    pub fn body(&self, id: BodyId) -> Option<&Body> {
        self.bodies.binary_search_by_key(&id, |b| b.id).ok().map(|i| &self.bodies[i])
    }
    pub fn bodies(&self) -> &[Body] {
        &self.bodies
    }
    pub fn nodes(&self) -> &Quadtree {
        &self.tree
    }
    pub fn frame(&self) -> u64 {
        self.frame
    }

    /// Evicts least recently wanted resident nodes from the CPU cache down to `cap` bytes,
    /// never a node wanted this frame. The loader frees what is returned.
    pub fn evict(&mut self, cap: u64) -> Vec<NodeId> {
        let wanted = &self.wanted;
        self.cache.evict_to(cap, |id| wanted.contains(&id))
    }

    /// Metres above the ellipsoid at body-fixed `pos` and the level of the node that answered:
    /// the finest node whose height page is resident, descending from the root without
    /// skipping levels. `None` if the body is unknown or its root page is not resident.
    pub fn height(&self, res: &dyn Residency, body: BodyId, pos: DVec3) -> Option<(f64, u8)> {
        self.body(body)?;
        let fp = FacePos::from_dir(body, pos);
        let mut best = None;
        for level in 0..=NodeId::MAX_LEVEL {
            let (id, u, v) = fp.node(level);
            match res.height_page(id) {
                Some(p) => best = Some((p.sample(u, v), level)),
                None => break,
            }
        }
        best
    }

    /// Ensures the record, refreshes it from the residency and stamps it wanted.
    fn visit(&mut self, id: NodeId, body: usize, parent: Option<u32>, res: &dyn Residency) -> u32 {
        let b = &self.bodies[body];
        let range = match parent {
            Some(p) => self.tree.at(p).inherit(b),
            None => (b.min_h, b.max_h),
        };
        let slot = self.tree.ensure(id, b, range);
        let frame = self.frame;
        let n = self.tree.at_mut(slot);
        n.chunk_mask = res.chunks(id);
        n.state = n.chunk_mask.state();
        n.bytes = res.bytes(id).max(1);
        n.last_wanted_frame = frame;
        if let Some(p) = res.height_page(id) {
            n.set_range(b, p.min, p.max, true);
        } else if !n.h_known {
            n.set_range(b, range.0, range.1, false);
        }
        slot
    }

    /// Selects the wanted tree and the draw set for one view.
    pub fn select(&mut self, view: &View, poses: &[BodyPose], res: &dyn Residency, cfg: &SelectConfig) -> Selection {
        self.frame += 1;
        let k = view.pixels_per_radian();
        let aspect = view.viewport[0].max(1) as f64 / view.viewport[1].max(1) as f64;
        let frustum = Frustum::new(view.rotation, view.fov_y, aspect);
        let ahead_w = view.look_ahead();

        let mut ctxs: Vec<Ctx> = Vec::new();
        let mut poses: Vec<BodyPose> = poses.to_vec();
        poses.sort_by_key(|p| p.id);
        poses.dedup_by_key(|p| p.id);
        for pose in poses {
            if let Ok(body) = self.bodies.binary_search_by_key(&pose.id, |b| b.id) {
                let eye = pose.to_body(view.position);
                let horizon = Horizon::new(eye, self.bodies[body].occluder_radius());
                ctxs.push(Ctx { body, pose, eye, ahead: pose.to_body(ahead_w), horizon });
            }
        }

        let mut tree: BTreeSet<NodeId> = BTreeSet::new();
        let mut split: BTreeSet<NodeId> = BTreeSet::new();
        let mut bytes: u64 = 0;
        let mut heap: BinaryHeap<Cand> = BinaryHeap::new();
        let mut budget_limited = 0u32;

        // Projected error and visibility of one node; pushes a split candidate.
        let consider = |me: &Planet, slot: u32, ctx: &Ctx, heap: &mut BinaryHeap<Cand>| {
            let n = me.tree.at(slot);
            let body = &me.bodies[ctx.body];
            if n.id.level() >= body.max_level.min(NodeId::MAX_LEVEL) {
                return;
            }
            let r = n.bound_radius as f64;
            let c = n.bound_centre;
            if ctx.horizon.hidden(c, r) || !frustum.visible(ctx.pose.rotation * (c - ctx.eye), r) {
                return;
            }
            let dist = patch_distance(n, body, ctx.eye).min(patch_distance(n, body, ctx.ahead)).max(0.01);
            let err = n.error_m(cfg.error_cells) * k / dist;
            let f = if me.prev_split.contains(&n.id) { cfg.merge } else { cfg.split };
            if err > cfg.pixel_error * f {
                let kids = n.id.children().expect("below max level");
                let cost: u64 = kids.iter().map(|&c| res.bytes(c).max(1)).sum();
                heap.push(Cand { prio: err / cost as f64, id: n.id });
            }
        };

        for (ci, ctx) in ctxs.iter().enumerate() {
            let body_id = self.bodies[ctx.body].id;
            for face in 0..6 {
                let id = NodeId::new(body_id, face, 0, 0, 0).expect("root");
                let slot = self.visit(id, ctx.body, None, res);
                tree.insert(id);
                bytes += self.tree.at(slot).bytes;
                consider(self, slot, &ctxs[ci], &mut heap);
            }
        }

        let ctx_of = |ctxs: &[Ctx], bodies: &[Body], id: NodeId| -> usize {
            ctxs.iter().position(|c| bodies[c.body].id == id.body()).expect("body context")
        };

        let mut need: Vec<NodeId> = Vec::new();
        while let Some(cand) = heap.pop() {
            if split.contains(&cand.id) {
                continue;
            }
            need.clear();
            plan(cand.id, &tree, &split, &mut need);
            let cost: u64 = need.iter().flat_map(|n| n.children().expect("split below max level")).map(|c| res.bytes(c).max(1)).sum();
            if bytes + cost > cfg.budget_bytes {
                budget_limited += 1;
                continue;
            }
            for &n in &need {
                let ci = ctx_of(&ctxs, &self.bodies, n);
                let pslot = self.tree.slot(n).expect("planned node is in the tree");
                split.insert(n);
                for c in n.children().expect("below max level") {
                    let slot = self.visit(c, ctxs[ci].body, Some(pslot), res);
                    tree.insert(c);
                    bytes += self.tree.at(slot).bytes;
                    consider(self, slot, &ctxs[ci], &mut heap);
                }
            }
        }

        // Residency bookkeeping, in sorted order.
        for &id in &tree {
            let n = self.tree.get(id).expect("visited");
            if n.state >= NodeState::Resident {
                self.cache.touch(id, n.bytes, self.frame);
            }
        }

        // Parent until children (P2-D6), then keep drawn neighbours within one level.
        let complete = |me: &Planet, id: NodeId| me.tree.get(id).is_some_and(|n| n.chunk_mask.complete());
        let mut expanded: BTreeSet<NodeId> = BTreeSet::new();
        for &n in &split {
            let present = n.level() == 0 || n.parent().is_some_and(|p| expanded.contains(&p));
            if present && n.children().expect("split").iter().all(|&c| complete(self, c)) {
                expanded.insert(n);
            }
        }
        loop {
            let present = |e: &BTreeSet<NodeId>, m: NodeId| m.level() == 0 || m.parent().is_some_and(|p| e.contains(&p));
            let bad: Vec<NodeId> =
                expanded.iter().copied().filter(|&n| n.level() > 0 && edge_neighbours(n).iter().any(|&m| !present(&expanded, m))).collect();
            if bad.is_empty() {
                break;
            }
            expanded.retain(|&e| !bad.iter().any(|&b| b.contains(e)));
        }

        let mut draw: Vec<DrawNode> = Vec::new();
        let mut holes: Vec<NodeId> = Vec::new();
        let mut stack: Vec<NodeId> = tree.iter().copied().filter(|n| n.level() == 0).rev().collect();
        while let Some(n) = stack.pop() {
            if expanded.contains(&n) {
                stack.extend(n.children().expect("split").iter().rev());
            } else if complete(self, n) {
                let ctx = &ctxs[ctx_of(&ctxs, &self.bodies, n)];
                draw.push(self.draw_node(n, ctx, view, res, cfg, k));
            } else {
                holes.push(n);
            }
        }
        draw.sort_by_key(|d| d.id);
        holes.sort_unstable();

        self.tree.collect(self.frame.saturating_sub(cfg.keep_frames));
        let wanted: Vec<NodeId> = tree.iter().copied().collect();
        self.wanted = tree;
        self.prev_split = split.clone();
        Selection { frame: self.frame, wanted, wanted_bytes: bytes, split: split.into_iter().collect(), draw, holes, budget_limited }
    }

    fn draw_node(&self, id: NodeId, ctx: &Ctx, view: &View, res: &dyn Residency, cfg: &SelectConfig, k: f64) -> DrawNode {
        let body = &self.bodies[ctx.body];
        let n = self.tree.get(id).expect("drawn node has a record");
        let origin_b = body.point(n.centre_dir, 0.0);
        let rel = ctx.pose.rotation * origin_b + (ctx.pose.position - view.position);
        let basis = DMat3::from_quat(ctx.pose.rotation) * body.frame(n.centre_dir);
        let morph = if id.level() == 0 {
            [f32::MAX; 2]
        } else {
            // The parent (twice the error) merges back at this distance.
            let end = 2.0 * n.error_m(cfg.error_cells) * k / (cfg.pixel_error * cfg.merge);
            [(0.7 * end) as f32, end as f32]
        };
        DrawNode { id, origin: rel.as_vec3(), basis: basis.as_mat3(), morph, page_slot: res.page_slot(id).unwrap_or(u32::MAX) }
    }
}

/// Distance from `eye` (body-fixed) to a node's patch, used for its projected error.
///
/// The bounding sphere is a poor distance proxy while a node's heights are unknown (it is
/// then tens of km tall), so the patch is treated as a curved slab: a horizontal term (chord
/// between the eye's and the node's directions at the surface, less the half diagonal) and a
/// vertical term (eye altitude outside `[min_h, max_h]`). Never larger than the true distance
/// to any point of the patch by more than the curvature sagitta, which is below the node size.
fn patch_distance(n: &crate::quadtree::Node, body: &Body, eye: DVec3) -> f64 {
    let el = eye.length();
    if el <= 0.0 {
        return 0.0;
    }
    let ed = eye / el;
    let r = body.radius(ed);
    let alt = el - r;
    let dh = ((ed - n.centre_dir).length() * r - n.edge_m * 0.82).max(0.0);
    let dv = (alt - n.max_h as f64).max(n.min_h as f64 - alt).max(0.0);
    (dh * dh + dv * dv).sqrt()
}

/// True if `m` is in the tree, or will be once the nodes in `need` are split.
fn present(tree: &BTreeSet<NodeId>, need: &[NodeId], m: NodeId) -> bool {
    tree.contains(&m) || m.parent().is_some_and(|p| need.contains(&p))
}

/// The splits needed before `n` can split with every edge neighbour at most one level apart:
/// each same-level edge neighbour of `n` must exist in the tree. Prerequisites first.
fn plan(n: NodeId, tree: &BTreeSet<NodeId>, split: &BTreeSet<NodeId>, need: &mut Vec<NodeId>) {
    if split.contains(&n) || need.contains(&n) {
        return;
    }
    if !present(tree, need, n) {
        plan(n.parent().expect("roots are always present"), tree, split, need);
    }
    for m in edge_neighbours(n) {
        if !present(tree, need, m) {
            plan(m.parent().expect("roots are always present"), tree, split, need);
        }
    }
    need.push(n);
}
