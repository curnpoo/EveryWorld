//! Terrain provider (Phase 2 W3): draws ed_planet's selected nodes as one shared node mesh,
//! displaced on the GPU from height pages (P2-D1, D2, D7).
//!
//! * One 65 x 65 vertex grid (64 cells, quads split on the 00-11 diagonal like
//!   `HeightPage::sample`) plus a 256-vertex skirt ring, one index buffer, one instanced draw:
//!   instance = drawn node.
//! * Heights come from a GPU page pool: full 257 x 257 u16 pages (`PAGE_WORDS` u32 per slot),
//!   LRU by frame, uploaded from the residency's decoded pages. Grid vertex (i, j) reads page
//!   sample (4i, 4j).
//! * Positions use ed_planet's surface rule exactly, `dir * (radius(dir) + h)` with `dir` the
//!   node lattice direction (`NodeId::grid_point`), computed relative to the node origin with
//!   difference formulas (`shaders/terrain_common.slang`). The node origin minus the camera is
//!   formed in f64 on the CPU; only that small offset reaches the GPU in f32 (P2-D7).
//! * Geomorph (CDLOD, P2-D2): each vertex blends toward the parent's surface (parent heights
//!   at even vertices, edge or 00-11 diagonal midpoints at odd ones) over a per-level distance
//!   band, so same-level neighbours always agree. On an edge whose neighbour is one level
//!   coarser the vertices are pinned to that neighbour's (morphing) edge: no T-junction crack.
//!   Skirts (a quarter cell deep) cover what f32 rounding leaves.
//! * Writes the visibility buffer (x: [`TERRAIN_VIS_BIT`] | draw index, y: primitive) and, when
//!   the view has a shadow atlas, the sun cascades (the same `VisTargets::shadow` path as
//!   provider 1).
//!
//! Cost (M3 Pro, 2560 x 1440, the Zermatt ground-to-orbit harness in `terrain_harness.rs`):
//! see the harness report; off switch: do not register the provider.

use crate::graph::{FrameGraph, PassKind, Plan, Res};
use crate::provider::{GeometryProvider, ProviderOutputs, SharedInputs, VisTargets};
use crate::scene::InstancePools;
use crate::{DEPTH_FORMAT, Tracked, VIS_FORMAT, gpu};
use ed_geo::node::warp;
use ed_geo::{HeightPage, NodeId, PAGE_N, PAGE_SAMPLES};
use ed_gpu::{
    Backend, Compare, Cull, DepthDesc, Device, LoadOp, RenderPipelineDesc, RenderPipelineId, ResourceState, ShaderCode, ShaderDesc, ShaderId,
    ShaderStage, StoreOp, Topology, Usage,
};
use ed_planet::{Body, BodyPose, Residency, SelectConfig, Selection};
use glam::{DMat3, DVec3, Vec3};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// Grid cells per node side.
pub const CELLS: u32 = 64;
/// Grid vertices per side.
pub const SIDE: u32 = CELLS + 1;
pub const GRID_VERTS: u32 = SIDE * SIDE;
/// Skirt ring vertices.
pub const RING: u32 = 4 * CELLS;
pub const VERTS: u32 = GRID_VERTS + RING;
pub const GRID_TRIS: u32 = CELLS * CELLS * 2;
pub const TRIS: u32 = GRID_TRIS + 2 * RING;
pub const INDEX_COUNT: u32 = TRIS * 3;
/// Words per drawn node (`TerrainDraw` in terrain_common.slang).
pub const DRAW_WORDS: usize = 40;
/// u32 words per page pool slot (257 x 257 u16, padded to whole words).
pub const PAGE_WORDS: usize = PAGE_SAMPLES.div_ceil(2);
/// Visibility word x of a terrain pixel: this bit | draw index.
pub const TERRAIN_VIS_BIT: u32 = 0x8000_0000;
pub const NO_SLOT: u32 = u32::MAX;
/// Flags word bits (`TerrainDraw::slots[2]`).
pub const F_COARSE_MASK: u32 = 0xF;
pub const F_QX: u32 = 1 << 4;
pub const F_QY: u32 = 1 << 5;
pub const F_SKIRT: u32 = 1 << 6;
/// Page samples per grid cell.
const STRIDE: usize = 4;
/// The morph band uses the node's nominal cell size (equator, face centre) times this, the
/// smallest ratio of a node's actual `edge_m` to the nominal one over a cube face: the band
/// then always ends before the selector merges the node (no pop at the merge).
const EDGE_FACTOR: f64 = 0.92;

/// Provider settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerrainConfig {
    /// Most nodes drawn per view and frame (the rest are dropped, counted in the stats).
    pub max_draws: u32,
    /// GPU page pool slots (132 KB each).
    pub pool_slots: u32,
    /// Draw skirts (off only for crack measurements).
    pub skirts: bool,
    /// Clear the visibility, depth and shadow targets (when terrain is the first or only
    /// provider). Otherwise they are loaded, after provider 1.
    pub clear_targets: bool,
    /// The selector's settings (morph bands follow its error metric).
    pub select: SelectConfig,
}

impl Default for TerrainConfig {
    fn default() -> Self {
        Self { max_draws: 4096, pool_slots: 1024, skirts: true, clear_targets: false, select: SelectConfig::default() }
    }
}

/// One drawn node (`TerrainDraw` in terrain_common.slang).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TerrainDraw {
    pub id: Option<NodeId>,
    pub origin: [f32; 4],
    pub dir_c: [f32; 4],
    pub cc: [f32; 4],
    pub fu: [f32; 4],
    pub fv: [f32; 4],
    pub shape: [f32; 4],
    pub morph: [f32; 4],
    pub polar: [f32; 4],
    pub slots: [u32; 4],
    pub hq: [f32; 4],
}

impl TerrainDraw {
    pub fn words(&self) -> [u32; DRAW_WORDS] {
        let mut w = [0u32; DRAW_WORDS];
        let rows = [self.origin, self.dir_c, self.cc, self.fu, self.fv, self.shape, self.morph, self.polar];
        for (r, row) in rows.iter().enumerate() {
            for (c, x) in row.iter().enumerate() {
                w[r * 4 + c] = x.to_bits();
            }
        }
        w[32..36].copy_from_slice(&self.slots);
        for (c, x) in self.hq.iter().enumerate() {
            w[36 + c] = x.to_bits();
        }
        w
    }
}

/// Face axes (normal, u, v) of `ed_geo::node`'s cube mapping: `cube(face, u, v) = n + u U + v V`.
pub fn face_axes(face: u8) -> [DVec3; 3] {
    let (x, y, z) = (DVec3::X, DVec3::Y, DVec3::Z);
    match face {
        0 => [x, y, z],
        1 => [-x, -y, z],
        2 => [y, -x, z],
        3 => [-y, x, z],
        4 => [z, y, -x],
        _ => [-z, y, x],
    }
}

/// Distance band (m) over which level `level` morphs to its parent: the parent (twice the
/// error) merges back at `end` (ed_planet's `draw_node` rule, with a per-level cell size).
pub fn morph_band(body: &Body, level: u8, cfg: &SelectConfig, pixels_per_radian: f64) -> [f32; 2] {
    if level == 0 {
        return [f32::MAX; 2];
    }
    let edge = body.equatorial_m * std::f64::consts::FRAC_PI_2 / (1u64 << level) as f64 * EDGE_FACTOR;
    let end = 2.0 * edge / cfg.error_cells * pixels_per_radian / (cfg.pixel_error * cfg.merge);
    [(0.7 * end) as f32, end as f32]
}

/// Per-node GPU constants. `eye` is the camera in the world frame; `pose` maps the body into it.
#[allow(clippy::too_many_arguments)]
pub fn node_draw(id: NodeId, body: &Body, pose: &BodyPose, eye: DVec3, bands: [[f32; 2]; 2], skirts: bool, page: (u32, &HeightPage), parent: Option<(u32, &HeightPage)>, coarse: u32) -> TerrainDraw {
    let [fnrm, fu, fv] = face_axes(id.face());
    let n = id.side() as u64 * CELLS as u64;
    let c = CELLS as u64;
    let tu = warp(id.x() as u64 * c + c / 2, n);
    let tv = warp(id.y() as u64 * c + c / 2, n);
    let cc = fnrm + fu * tu + fv * tv;
    let lc = cc.length();
    let dir_c = cc / lc;
    let rc = body.radius(dir_c);
    let rot = DMat3::from_quat(pose.rotation);
    let origin = rot * (dir_c * rc) + (pose.position - eye);
    let (a, b) = (body.equatorial_m, body.polar_m);
    let e = 1.0 - a * a / (b * b);
    let sc = (1.0 - e * dir_c.z * dir_c.z).sqrt();
    let delta = std::f64::consts::PI / (2 * n) as f64;
    let cell = a * std::f64::consts::FRAC_PI_2 / n as f64;
    let skirt = cell * 0.25 + 0.5;
    let v4 = |v: DVec3, w: f64| [v.x as f32, v.y as f32, v.z as f32, w as f32];
    let (qx, qy) = (id.x() & 1, id.y() & 1);
    let mut flags = coarse & F_COARSE_MASK;
    if qx == 1 {
        flags |= F_QX;
    }
    if qy == 1 {
        flags |= F_QY;
    }
    if skirts {
        flags |= F_SKIRT;
    }
    TerrainDraw {
        id: Some(id),
        origin: v4(origin, skirt),
        dir_c: v4(rot * dir_c, rc),
        cc: v4(rot * cc, lc),
        fu: v4(rot * fu, tu),
        fv: v4(rot * fv, tv),
        shape: [delta as f32, a as f32, e as f32, sc as f32],
        morph: [bands[0][0], bands[0][1], bands[1][0], bands[1][1]],
        polar: v4(rot * DVec3::Z, 0.0),
        slots: [page.0, parent.map_or(NO_SLOT, |p| p.0), flags, id.level() as u32],
        hq: [page.1.scale, page.1.offset, parent.map_or(0.0, |p| p.1.scale), parent.map_or(0.0, |p| p.1.offset)],
    }
}

/// Camera-relative box of a drawn node: min (xyz) and max (xyz), metres. Nine lattice points
/// (corners, edge midpoints, centre) at the page's lowest and highest height, grown by the
/// curvature between them, the skirt and a metre.
pub fn node_bound(body: &Body, pose: &BodyPose, eye: DVec3, id: NodeId, page: &HeightPage, skirt: f64) -> [f32; 6] {
    let rot = DMat3::from_quat(pose.rotation);
    let (mut lo, mut hi) = (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN));
    for j in [0, 32, 64] {
        for i in [0, 32, 64] {
            let d = id.grid_point(i, j, CELLS);
            for h in [page.min_m as f64, page.max_m as f64] {
                let p = rot * body.point(d, h) + (pose.position - eye);
                lo = lo.min(p);
                hi = hi.max(p);
            }
        }
    }
    let half_span = std::f64::consts::FRAC_PI_4 / id.side() as f64 * 1.42;
    let grow = body.equatorial_m * half_span * half_span / 8.0 + skirt + 1.0;
    let (lo, hi) = (lo - grow, hi + grow);
    [lo.x as f32, lo.y as f32, lo.z as f32, hi.x as f32, hi.y as f32, hi.z as f32]
}

/// Does a camera-relative box touch an orthographic cascade (clip x, y in -1..1, z 0..1 with
/// 1 toward the sun)? Casters in front of the box count (depth clamp), behind it do not.
pub fn cascade_touches(m: &glam::Mat4, b: &[f32; 6]) -> bool {
    if m.row(3) != glam::Vec4::W {
        return true; // Not an orthographic matrix (or unset): draw everything.
    }
    let (mut lo, mut hi) = (glam::Vec3::splat(f32::MAX), glam::Vec3::splat(f32::MIN));
    for k in 0..8 {
        let c = Vec3::new(b[if k & 1 == 0 { 0 } else { 3 }], b[if k & 2 == 0 { 1 } else { 4 }], b[if k & 4 == 0 { 2 } else { 5 }]);
        let p = m.transform_point3(c);
        lo = lo.min(p);
        hi = hi.max(p);
    }
    lo.x <= 1.0 && hi.x >= -1.0 && lo.y <= 1.0 && hi.y >= -1.0 && hi.z >= 0.0
}

// ---------------------------------------------------------------------------------------------
// CPU mirror of terrain_common.slang (tests, the harness's CPU shading and hole detection).
// ---------------------------------------------------------------------------------------------

/// Heights of a draw: own page and optional parent page, as the GPU reads them.
pub struct DrawPages<'a> {
    pub own: &'a HeightPage,
    pub parent: Option<&'a HeightPage>,
}

fn page_h(p: &HeightPage, scale: f32, offset: f32, i: usize, j: usize) -> f32 {
    offset + scale * p.samples[j * PAGE_N + i] as f32
}

fn v3(a: [f32; 4]) -> Vec3 {
    Vec3::new(a[0], a[1], a[2])
}

fn own_h(d: &TerrainDraw, pg: &DrawPages, i: u32, j: u32) -> f32 {
    page_h(pg.own, d.hq[0], d.hq[1], i as usize * STRIDE, j as usize * STRIDE)
}

/// `t_lattice`: lattice point (i, j) at height h, node-relative.
pub fn cpu_lattice(d: &TerrainDraw, i: u32, j: u32, h: f32) -> Vec3 {
    let tu = ((i as f32 - 32.0) * d.shape[0]).tan();
    let tv = ((j as f32 - 32.0) * d.shape[0]).tan();
    let du = tu * (1.0 + d.fu[3] * d.fu[3]) / (1.0 - d.fu[3] * tu);
    let dv = tv * (1.0 + d.fv[3] * d.fv[3]) / (1.0 - d.fv[3] * tv);
    let delta = du * v3(d.fu) + dv * v3(d.fv);
    let cc = v3(d.cc);
    let lc = d.cc[3];
    let k = 2.0 * cc.dot(delta) + delta.dot(delta);
    let l = (lc * lc + k).sqrt();
    let dd = delta / l - cc * (k / ((lc + l) * l * lc));
    let dir = v3(d.dir_c) + dd;
    let polar = v3(d.polar);
    let dzc = v3(d.dir_c).dot(polar);
    let ddz = dd.dot(polar);
    let dz = dzc + ddz;
    let e = d.shape[2];
    let s = (1.0 - e * dz * dz).sqrt();
    let sc = d.shape[3];
    let dr = d.shape[1] * e * ddz * (dz + dzc) / (s * sc * (s + sc));
    let r = d.dir_c[3] + dr;
    dd * r + v3(d.dir_c) * dr + dir * h
}

fn cpu_base(d: &TerrainDraw, pg: &DrawPages, i: u32, j: u32) -> Vec3 {
    cpu_lattice(d, i, j, own_h(d, pg, i, j))
}

fn cpu_target_even(d: &TerrainDraw, pg: &DrawPages, i: u32, j: u32) -> Vec3 {
    let border = i == 0 || j == 0 || i == CELLS || j == CELLS;
    let h = match pg.parent {
        Some(p) if !border && d.slots[1] != NO_SLOT => {
            let qx = if d.slots[2] & F_QX != 0 { 128 } else { 0 };
            let qy = if d.slots[2] & F_QY != 0 { 128 } else { 0 };
            page_h(p, d.hq[2], d.hq[3], qx + 2 * i as usize, qy + 2 * j as usize)
        }
        _ => own_h(d, pg, i, j),
    };
    cpu_lattice(d, i, j, h)
}

fn cpu_target(d: &TerrainDraw, pg: &DrawPages, i: u32, j: u32) -> Vec3 {
    match (i & 1 == 0, j & 1 == 0) {
        (true, true) => cpu_target_even(d, pg, i, j),
        (false, true) => 0.5 * (cpu_target_even(d, pg, i - 1, j) + cpu_target_even(d, pg, i + 1, j)),
        (true, false) => 0.5 * (cpu_target_even(d, pg, i, j - 1) + cpu_target_even(d, pg, i, j + 1)),
        (false, false) => 0.5 * (cpu_target_even(d, pg, i - 1, j - 1) + cpu_target_even(d, pg, i + 1, j + 1)),
    }
}

fn morph_factor(rel: Vec3, band: [f32; 2]) -> f32 {
    ((rel.length() - band[0]) / (band[1] - band[0]).max(1e-3)).clamp(0.0, 1.0)
}

fn edge_at(edge: u32, e: u32) -> (u32, u32) {
    match edge {
        0 => (0, e),
        1 => (CELLS, e),
        2 => (e, 0),
        _ => (e, CELLS),
    }
}

fn cpu_coarse_even(d: &TerrainDraw, pg: &DrawPages, edge: u32, e: u32) -> Vec3 {
    let (i, j) = edge_at(edge, e);
    let p = cpu_base(d, pg, i, j);
    if e & 3 == 0 {
        return p;
    }
    let m = morph_factor(v3(d.origin) + p, [d.morph[2], d.morph[3]]);
    let (bi, bj) = edge_at(edge, e - 2);
    let (ci, cj) = edge_at(edge, e + 2);
    let mid = 0.5 * (cpu_base(d, pg, bi, bj) + cpu_base(d, pg, ci, cj));
    p.lerp(mid, m)
}

fn cpu_coarse(d: &TerrainDraw, pg: &DrawPages, edge: u32, e: u32) -> Vec3 {
    if e & 1 == 0 {
        return cpu_coarse_even(d, pg, edge, e);
    }
    0.5 * (cpu_coarse_even(d, pg, edge, e - 1) + cpu_coarse_even(d, pg, edge, e + 1))
}

/// `t_grid_vertex`: final node-relative position of grid vertex (i, j).
pub fn cpu_grid_vertex(d: &TerrainDraw, pg: &DrawPages, i: u32, j: u32) -> Vec3 {
    let coarse = d.slots[2] & F_COARSE_MASK;
    if coarse & 1 != 0 && i == 0 {
        return cpu_coarse(d, pg, 0, j);
    }
    if coarse & 2 != 0 && i == CELLS {
        return cpu_coarse(d, pg, 1, j);
    }
    if coarse & 4 != 0 && j == 0 {
        return cpu_coarse(d, pg, 2, i);
    }
    if coarse & 8 != 0 && j == CELLS {
        return cpu_coarse(d, pg, 3, i);
    }
    let p = cpu_base(d, pg, i, j);
    let m = morph_factor(v3(d.origin) + p, [d.morph[0], d.morph[1]]);
    if m <= 0.0 {
        return p;
    }
    p.lerp(cpu_target(d, pg, i, j), m)
}

/// `t_ring`: skirt ring vertex s, counter-clockwise seen from above.
pub fn ring(s: u32) -> (u32, u32) {
    let e = s % CELLS;
    match s / CELLS {
        0 => (e, 0),
        1 => (CELLS, e),
        2 => (CELLS - e, CELLS),
        _ => (0, CELLS - e),
    }
}

/// `t_vertex`: vertex `v` of the shared mesh, camera-relative.
pub fn cpu_vertex(d: &TerrainDraw, pg: &DrawPages, v: u32) -> Vec3 {
    let o = v3(d.origin);
    if v < GRID_VERTS {
        return o + cpu_grid_vertex(d, pg, v % SIDE, v / SIDE);
    }
    let (i, j) = ring(v - GRID_VERTS);
    let p = cpu_grid_vertex(d, pg, i, j);
    let depth = if d.slots[2] & F_SKIRT != 0 { d.origin[3] } else { 0.0 };
    let centre = o - v3(d.dir_c) * d.dir_c[3];
    let up = (o + p - centre).normalize();
    o + p - up * depth
}

/// `t_triangle`: vertex numbers of primitive `prim` (the index buffer's order).
pub fn triangle(prim: u32) -> [u32; 3] {
    if prim < GRID_TRIS {
        let q = prim >> 1;
        let (i, j) = (q % CELLS, q / CELLS);
        let v00 = j * SIDE + i;
        let (v10, v01) = (v00 + 1, v00 + SIDE);
        let v11 = v01 + 1;
        return if prim & 1 == 0 { [v00, v10, v11] } else { [v00, v11, v01] };
    }
    let k = prim - GRID_TRIS;
    let s = k >> 1;
    let s1 = (s + 1) % RING;
    let (p, q) = (ring(s), ring(s1));
    let (tp, tq) = (p.1 * SIDE + p.0, q.1 * SIDE + q.0);
    let (a, b) = (GRID_VERTS + s, GRID_VERTS + s1);
    if k & 1 == 0 { [a, b, tq] } else { [a, tq, tp] }
}

/// The shared index buffer.
pub fn indices() -> Vec<u32> {
    (0..TRIS).flat_map(triangle).collect()
}

/// Edge mask of `id` among `drawn`: bit set where the edge neighbour is one level coarser.
pub fn coarse_mask(id: NodeId, drawn: &BTreeSet<NodeId>) -> u32 {
    let mut m = 0;
    for (bit, (dx, dy)) in [(-1, 0), (1, 0), (0, -1), (0, 1)].into_iter().enumerate() {
        let nb = id.neighbour(dx, dy);
        if nb != id && !drawn.contains(&nb) && nb.parent().is_some_and(|p| drawn.contains(&p)) {
            m |= 1 << bit;
        }
    }
    m
}

// ---------------------------------------------------------------------------------------------
// Page pool
// ---------------------------------------------------------------------------------------------

struct PoolEntry {
    slot: u32,
    stamp: u64,
    page: Arc<HeightPage>,
}

/// GPU height page pool bookkeeping: `NodeId -> slot`, least recently used first (ties by id).
struct Pool {
    map: BTreeMap<NodeId, PoolEntry>,
    free: Vec<u32>,
    pending: Vec<(u32, Vec<u8>)>,
}

impl Pool {
    fn new(slots: u32) -> Self {
        Self { map: BTreeMap::new(), free: (0..slots).rev().collect(), pending: Vec::new() }
    }
    /// Slot of `id`'s page, uploading it if needed. `None` when every slot is in use this frame.
    fn ensure(&mut self, id: NodeId, page: &Arc<HeightPage>, stamp: u64) -> Option<u32> {
        if let Some(e) = self.map.get_mut(&id) {
            if Arc::ptr_eq(&e.page, page) || e.page.samples == page.samples && e.page.offset == page.offset {
                e.stamp = stamp;
                return Some(e.slot);
            }
            // The residency replaced the page: upload again into the same slot.
            e.page = page.clone();
            e.stamp = stamp;
            let slot = e.slot;
            self.pending.push((slot, page_bytes(page)));
            return Some(slot);
        }
        let slot = match self.free.pop() {
            Some(s) => s,
            None => {
                let (&victim, _) = self.map.iter().filter(|(_, e)| e.stamp < stamp).min_by_key(|(id, e)| (e.stamp, **id))?;
                self.map.remove(&victim).expect("victim").slot
            }
        };
        self.map.insert(id, PoolEntry { slot, stamp, page: page.clone() });
        self.pending.push((slot, page_bytes(page)));
        Some(slot)
    }
}

fn page_bytes(p: &HeightPage) -> Vec<u8> {
    let mut b = Vec::with_capacity(PAGE_WORDS * 4);
    for s in &p.samples {
        b.extend_from_slice(&s.to_le_bytes());
    }
    b.resize(PAGE_WORDS * 4, 0);
    b
}

// ---------------------------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------------------------

/// Counters of the latest [`TerrainProvider::update`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TerrainStats {
    /// Nodes drawn.
    pub drawn: u32,
    /// Drawn nodes with a coarser edge neighbour (stitched edges).
    pub stitched: u32,
    /// Nodes dropped: no free pool slot or over `max_draws`.
    pub dropped: u32,
    /// Drawn nodes whose residency had no height page (drawn flat at 0 m).
    pub flat_fallbacks: u32,
    /// Pages uploaded by this update.
    pub uploads: u32,
    /// Pool slots in use.
    pub pool_used: u32,
}

/// Where the camera is on the curved body: what the atmosphere's flat-world inputs
/// (`AtmosphereSettings::surface_y`, `surface_alt_km`, the hard-coded +Y up) become.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraGround {
    /// Local up at the camera (ellipsoid normal), world axes.
    pub up: Vec3,
    /// Camera height above the ellipsoid along its direction (radial, m).
    pub camera_alt_m: f64,
    /// Terrain height under the camera (m above the ellipsoid), when the page is resident.
    pub ground_alt_m: Option<f64>,
}

struct TerrainView {
    /// Main draws, then each cascade's culled copies (`ranges`).
    draws: Tracked,
    entries: Vec<TerrainDraw>,
    /// Camera-relative box per entry (min xyz, max xyz).
    bounds: Vec<[f32; 6]>,
    /// Per cascade: first draw in the buffer and count (set by `prepare`).
    ranges: [(u32, u32); 4],
    imports: Vec<(Res, u8)>,
}

/// Provider for ed_planet terrain (Phase 2 W3).
pub struct TerrainProvider {
    pub config: TerrainConfig,
    built: Vec<ShaderId>,
    raster: RenderPipelineId,
    shadow: RenderPipelineId,
    index: Tracked,
    pool_buf: Tracked,
    pool: Pool,
    views: Vec<TerrainView>,
    cur: usize,
    stamp: u64,
    stats: TerrainStats,
    /// The eye of the latest update (draw origins are relative to it).
    eye: DVec3,
}

const GEN_MSL: &str = include_str!("../shaders/gen/msl/terrain.metal");
const GEN_VS: &[u8] = include_bytes!("../shaders/gen/spv/terrain.vs_terrain.spv");
const GEN_FS: &[u8] = include_bytes!("../shaders/gen/spv/terrain.fs_terrain.spv");
const GEN_VS_SHADOW: &[u8] = include_bytes!("../shaders/gen/spv/terrain.vs_terrain_shadow.spv");

fn spv_words(b: &[u8]) -> Vec<u32> {
    b.as_chunks::<4>().0.iter().map(|c| u32::from_le_bytes(*c)).collect()
}

impl TerrainProvider {
    pub fn new(dev: &dyn Device, config: TerrainConfig) -> Result<Self, ed_gpu::Error> {
        let mut built = Vec::new();
        let r = Self::pipelines(dev, &mut built);
        let (raster, shadow) = match r {
            Ok(p) => p,
            Err(e) => {
                for s in built {
                    dev.destroy_shader(s);
                }
                return Err(e);
            }
        };
        let upload = Usage { shader_read: true, copy_dst: true, ..Default::default() };
        let index = Tracked::new(dev, "terrain indices", INDEX_COUNT as u64 * 4, upload)?;
        dev.write_buffer(index.id, 0, &gpu::bytes(&indices()))?;
        let pool_buf = Tracked::new(dev, "terrain page pool", config.pool_slots as u64 * PAGE_WORDS as u64 * 4, upload)?;
        let view0 = Self::new_view(dev, config.max_draws)?;
        Ok(Self { config, built, raster, shadow, index, pool_buf, pool: Pool::new(config.pool_slots), views: vec![view0], cur: 0, stamp: 0, stats: TerrainStats::default(), eye: DVec3::ZERO })
    }

    fn new_view(dev: &dyn Device, max_draws: u32) -> Result<TerrainView, ed_gpu::Error> {
        let per_frame = Usage { shader_read: true, cpu_write: true, ..Default::default() };
        // Room for the view's draws and up to four cascade lists.
        let size = max_draws as u64 * 5 * DRAW_WORDS as u64 * 4;
        Ok(TerrainView { draws: Tracked::new(dev, "terrain draws", size, per_frame)?, entries: Vec::new(), bounds: Vec::new(), ranges: [(0, 0); 4], imports: Vec::new() })
    }

    fn pipelines(dev: &dyn Device, built: &mut Vec<ShaderId>) -> Result<(RenderPipelineId, RenderPipelineId), ed_gpu::Error> {
        let vk = dev.caps().backend == Backend::Vulkan;
        let (vs_w, fs_w, vss_w) = (spv_words(GEN_VS), spv_words(GEN_FS), spv_words(GEN_VS_SHADOW));
        let mut shader = |entry: &str, stage, spv: &[u32]| -> Result<ShaderId, ed_gpu::Error> {
            let code = if vk { ShaderCode::SpirV(spv) } else { ShaderCode::MslSource(GEN_MSL) };
            let s = dev.create_shader(&ShaderDesc { label: entry, stage, entry, code })?;
            built.push(s);
            Ok(s)
        };
        let vs = shader("vs_terrain", ShaderStage::Vertex, &vs_w)?;
        let fs = shader("fs_terrain", ShaderStage::Fragment, &fs_w)?;
        let vss = shader("vs_terrain_shadow", ShaderStage::Vertex, &vss_w)?;
        let raster = dev.create_render_pipeline(&RenderPipelineDesc {
            label: "terrain",
            vertex: vs,
            fragment: Some(fs),
            color_formats: &[VIS_FORMAT],
            blend: ed_gpu::Blend::Opaque,
            depth: Some(DepthDesc { format: DEPTH_FORMAT, write: true, compare: Compare::GreaterEqual, bias: 0.0, slope_bias: 0.0, clamp: false }),
            cull: Cull::Back,
            topology: Topology::Triangles,
            samples: 1,
        })?;
        let shadow = dev.create_render_pipeline(&RenderPipelineDesc {
            label: "terrain shadow",
            vertex: vss,
            fragment: None,
            color_formats: &[],
            blend: ed_gpu::Blend::Opaque,
            depth: Some(DepthDesc { format: DEPTH_FORMAT, write: true, compare: Compare::GreaterEqual, bias: 0.0, slope_bias: 0.0, clamp: true }),
            cull: Cull::None,
            topology: Topology::Triangles,
            samples: 1,
        });
        match shadow {
            Ok(s) => Ok((raster, s)),
            Err(e) => {
                dev.destroy_render_pipeline(raster);
                Err(e)
            }
        }
    }

    pub fn stats(&self) -> TerrainStats {
        self.stats
    }

    /// Draws per sun cascade after culling (the latest `prepare` of this view).
    pub fn cascade_draws(&self) -> [u32; 4] {
        self.views[self.cur].ranges.map(|r| r.1)
    }

    /// This view's draws of the latest update, in draw (instance) order.
    pub fn draws(&self) -> &[TerrainDraw] {
        &self.views[self.cur].entries
    }

    /// The pages a draw reads (CPU copies of what the pool holds).
    pub fn draw_pages(&self, d: &TerrainDraw) -> Option<DrawPages<'_>> {
        let id = d.id?;
        let own = &self.pool.map.get(&id)?.page;
        let parent = id.parent().filter(|_| d.slots[1] != NO_SLOT).and_then(|p| self.pool.map.get(&p)).map(|e| &*e.page);
        Some(DrawPages { own, parent })
    }

    /// Builds this view's draws from a selection (outside a frame; page uploads go out with
    /// the next `flush`). `eye` is the camera in the world frame (the f64 truth, as
    /// `ed_planet::View::position`), `pixels_per_radian` the view's
    /// (`ed_planet::View::pixels_per_radian`).
    pub fn update(&mut self, sel: &Selection, res: &dyn Residency, bodies: &[Body], poses: &[BodyPose], eye: DVec3, pixels_per_radian: f64) -> TerrainStats {
        self.stamp += 1;
        self.eye = eye;
        let stamp = self.stamp;
        let before = self.pool.pending.len();
        let drawn: BTreeSet<NodeId> = sel.draw.iter().map(|d| d.id).collect();
        let mut st = TerrainStats::default();
        let mut entries = Vec::with_capacity(sel.draw.len());
        let mut bounds = Vec::with_capacity(sel.draw.len());
        let flat = Arc::new(HeightPage::flat(0.0));
        for dn in &sel.draw {
            let id = dn.id;
            let (Some(body), Some(pose)) = (bodies.iter().find(|b| b.id == id.body()), poses.iter().find(|p| p.id == id.body())) else {
                st.dropped += 1;
                continue;
            };
            if entries.len() as u32 >= self.config.max_draws {
                st.dropped += 1;
                continue;
            }
            let page = match res.height_page(id) {
                Some(p) => p,
                None => {
                    st.flat_fallbacks += 1;
                    flat.clone()
                }
            };
            let Some(slot) = self.pool.ensure(id, &page, stamp) else {
                st.dropped += 1;
                continue;
            };
            let parent = id.parent().and_then(|p| res.height_page(p).map(|pg| (p, pg)));
            let parent = parent.and_then(|(p, pg)| self.pool.ensure(p, &pg, stamp).map(|s| (s, pg)));
            let coarse = coarse_mask(id, &drawn);
            if coarse != 0 {
                st.stitched += 1;
            }
            let cfg = &self.config.select;
            let bands = [morph_band(body, id.level(), cfg, pixels_per_radian), morph_band(body, id.level().saturating_sub(1), cfg, pixels_per_radian)];
            let d = node_draw(id, body, pose, eye, bands, self.config.skirts, (slot, &page), parent.as_ref().map(|(s, p)| (*s, &**p)), coarse);
            bounds.push(node_bound(body, pose, eye, id, &page, d.origin[3] as f64));
            entries.push(d);
        }
        st.drawn = entries.len() as u32;
        st.uploads = (self.pool.pending.len() - before) as u32;
        st.pool_used = self.pool.map.len() as u32;
        self.views[self.cur].entries = entries;
        self.views[self.cur].bounds = bounds;
        self.stats = st;
        st
    }

    /// The camera's place on the body, for the atmosphere and anything else that assumed a
    /// flat world (see [`CameraGround`]).
    pub fn camera_ground(body: &Body, pose: &BodyPose, eye: DVec3, ground: Option<f64>) -> CameraGround {
        let p = pose.to_body(eye);
        let d = p.normalize_or(DVec3::Z);
        let up = (pose.rotation * body.up(d)).as_vec3();
        CameraGround { up, camera_alt_m: p.length() - body.radius(d), ground_alt_m: ground }
    }
}

impl GeometryProvider for TerrainProvider {
    fn name(&self) -> &'static str {
        "terrain"
    }

    fn flush(&mut self, dev: &dyn Device, _pools: &InstancePools) -> Result<(), ed_gpu::Error> {
        for (slot, bytes) in self.pool.pending.drain(..) {
            dev.write_buffer(self.pool_buf.id, slot as u64 * PAGE_WORDS as u64 * 4, &bytes)?;
        }
        Ok(())
    }

    fn poll(&mut self, _dev: &dyn Device) {}

    /// Needs `FrameData.light` already in `fd` (the cascade matrices): each cascade draws only
    /// the nodes whose bounds touch its box.
    fn prepare(&mut self, dev: &dyn Device, _pools: &InstancePools, _width: u32, _height: u32, fd: &mut [u32]) -> Result<(), ed_gpu::Error> {
        let v = &mut self.views[self.cur];
        if v.entries.is_empty() {
            v.ranges = [(0, 0); 4];
            return Ok(());
        }
        let mut words: Vec<u32> = v.entries.iter().flat_map(|e| e.words()).collect();
        let mut next = v.entries.len() as u32;
        for c in 0..4 {
            let at = gpu::FD_LIGHT + c * 16;
            let m = glam::Mat4::from_cols_array(&std::array::from_fn(|k| f32::from_bits(fd.get(at + k).copied().unwrap_or(0))));
            let first = next;
            for (e, b) in v.entries.iter().zip(&v.bounds) {
                if cascade_touches(&m, b) {
                    words.extend_from_slice(&e.words());
                    next += 1;
                }
            }
            v.ranges[c] = (first, next - first);
        }
        let bytes = gpu::bytes(&words);
        dev.fill_frame_buffer(v.draws.id, &mut |dst| dst[..bytes.len()].copy_from_slice(&bytes))
    }

    fn record<'a>(&mut self, g: &mut FrameGraph<'a>, _frame_index: u64, inp: &SharedInputs, out: VisTargets) -> ProviderOutputs {
        let v = &mut self.views[self.cur];
        let n = v.entries.len() as u32;
        let draws = g.import_buffer("terrain draws", v.draws.id, v.draws.state, None);
        let pool = g.import_buffer("terrain page pool", self.pool_buf.id, self.pool_buf.state, Some(ResourceState::CopyDst));
        let index = g.import_buffer("terrain indices", self.index.id, self.index.state, None);
        v.imports = vec![(draws, 0), (pool, 1), (index, 2)];
        let consts = |cascade: u32, base: u32| gpu::constants(&[inp.frame.1, v.draws.bindless.0, self.pool_buf.bindless.0, cascade, base]);
        let (cv, idx_id, pipe, spipe) = (consts(0, 0), self.index.id, self.raster, self.shadow);
        let clear = self.config.clear_targets;
        let load = if clear { LoadOp::Clear([0.0; 4]) } else { LoadOp::Load };
        let read = ResourceState::ShaderRead;
        g.add_pass("terrain", PassKind::Raster)
            .read(inp.frame.0, read)
            .read(draws, read)
            .read(pool, read)
            .read(index, read)
            .color(out.vis, load, StoreOp::Store)
            .depth(out.depth, load, StoreOp::Store, true)
            .exec(move |ctx| {
                if n == 0 {
                    return;
                }
                ctx.enc.set_index_buffer(idx_id, 0);
                ctx.enc.set_render_pipeline(pipe);
                ctx.enc.set_draw_constants(&cv);
                ctx.enc.draw_indexed(0..INDEX_COUNT, 0, 0..n);
            });
        // No caster in any cascade: no pass (unless it has to clear the atlas).
        let casters = v.ranges.iter().any(|r| r.1 > 0);
        if let Some(sh) = out.shadow.filter(|_| casters || clear) {
            let (cascades, tile) = (sh.cascades.min(4), sh.tile);
            let ranges = v.ranges;
            let cs: Vec<_> = (0..cascades).map(|c| consts(c, ranges[c as usize].0)).collect();
            g.add_pass("terrain_shadow", PassKind::Raster)
                .read(inp.frame.0, read)
                .read(draws, read)
                .read(pool, read)
                .read(index, read)
                .depth(sh.atlas, load, StoreOp::Store, true)
                .exec(move |ctx| {
                    if n == 0 {
                        return;
                    }
                    ctx.enc.set_index_buffer(idx_id, 0);
                    ctx.enc.set_render_pipeline(spipe);
                    for (c, (cc, (_, count))) in cs.iter().zip(ranges).enumerate() {
                        if count == 0 {
                            continue;
                        }
                        let c = c as u32;
                        let (x, y) = ((c % 2) * tile, (c / 2) * tile);
                        ctx.enc.set_viewport(x as f32, y as f32, tile as f32, tile as f32);
                        ctx.enc.set_scissor(x, y, tile, tile);
                        ctx.enc.set_draw_constants(cc);
                        ctx.enc.draw_indexed(0..INDEX_COUNT, 0, 0..count);
                    }
                });
        }
        ProviderOutputs { meshes: (draws, v.draws.bindless.0), state: (pool, self.pool_buf.bindless.0), hzb: None }
    }

    fn finish(&mut self, plan: &Plan) {
        let v = &mut self.views[self.cur];
        for &(r, which) in &v.imports {
            let s = plan.final_states[r.index()];
            match which {
                0 => v.draws.state = s,
                1 => self.pool_buf.state = s,
                _ => self.index.state = s,
            }
        }
    }

    fn set_view(&mut self, dev: &dyn Device, slot: usize) -> Result<(), ed_gpu::Error> {
        while self.views.len() <= slot {
            let v = Self::new_view(dev, self.config.max_draws)?;
            self.views.push(v);
        }
        self.cur = slot;
        Ok(())
    }

    fn pass_names(&self) -> &'static [&'static str] {
        &["terrain", "terrain_shadow"]
    }

    fn destroy(self: Box<Self>, dev: &dyn Device) {
        dev.destroy_render_pipeline(self.raster);
        dev.destroy_render_pipeline(self.shadow);
        for s in self.built {
            dev.destroy_shader(s);
        }
        self.index.destroy(dev);
        self.pool_buf.destroy(dev);
        for v in self.views {
            v.draws.destroy(dev);
        }
    }
}

#[cfg(test)]
#[path = "terrain_harness.rs"]
mod harness;

#[cfg(test)]
mod tests {
    use super::*;
    use ed_geo::BodyId;
    use glam::DQuat;

    fn earth() -> Body {
        Body::earth()
    }

    fn page_fn(f: impl Fn(DVec3) -> f64, id: NodeId) -> HeightPage {
        // Global quantisation (0.5 m steps) so every level decodes a point to the same bits.
        let q: Vec<i64> = (0..PAGE_N * PAGE_N).map(|s| (f(id.grid_point((s % PAGE_N) as u32, (s / PAGE_N) as u32, 256)) / 0.5).round() as i64).collect();
        let lo = *q.iter().min().unwrap();
        let hi = *q.iter().max().unwrap();
        HeightPage { samples: q.iter().map(|&v| (v - lo) as u16).collect(), scale: 0.5, offset: (lo as f64 * 0.5) as f32, min_m: (lo as f64 * 0.5) as f32, max_m: (hi as f64 * 0.5) as f32 }
    }

    fn hills(d: DVec3) -> f64 {
        1500.0 + 900.0 * (d.x * 4000.0).sin() * (d.y * 3100.0).cos() + 300.0 * (d.z * 9000.0).sin()
    }

    fn zermatt_node(level: u8) -> NodeId {
        let g = ed_geo::Geodetic { lat_deg: 46.02, lon_deg: 7.75, alt_m: 0.0 }.to_ecef();
        NodeId::from_unit(BodyId(earth().id.0), DVec3::new(g.x, g.y, g.z).normalize(), level)
    }

    fn pose_at(body: &Body, id: NodeId) -> (BodyPose, DVec3) {
        // World = ENU at the node centre (x east, y up, z south), origin on the ellipsoid.
        let d = id.centre();
        let f = body.frame(d);
        let rot = DMat3::from_cols(f.x_axis, f.z_axis, -f.y_axis).transpose();
        let q = DQuat::from_mat3(&rot);
        let anchor = body.point(d, 0.0);
        (BodyPose { id: body.id, rotation: q, position: -(q * anchor) }, anchor)
    }

    #[test]
    fn index_buffer_is_ccw_from_outside_and_covers_the_grid() {
        let idx = indices();
        assert_eq!(idx.len() as u32, INDEX_COUNT);
        assert!(idx.iter().all(|&v| v < VERTS));
        // Every grid vertex is used; every skirt vertex too.
        let used: BTreeSet<u32> = idx.iter().copied().collect();
        assert_eq!(used.len() as u32, VERTS);
        // Winding: the grid's triangles face outward (+up) for a flat node.
        let body = earth();
        let id = zermatt_node(10);
        let (pose, anchor) = pose_at(&body, id);
        let page = HeightPage::flat(0.0);
        let eye = pose.rotation * anchor + pose.position + DVec3::new(0.0, 100.0, 0.0);
        let d = node_draw(id, &body, &pose, eye, [[f32::MAX; 2]; 2], true, (0, &page), None, 0);
        let pg = DrawPages { own: &page, parent: None };
        for prim in [0, 1, 777, 8191, GRID_TRIS, GRID_TRIS + 1, TRIS - 1, GRID_TRIS + 200] {
            let t = triangle(prim);
            let p: Vec<Vec3> = t.iter().map(|&v| cpu_vertex(&d, &pg, v)).collect();
            let n = (p[1] - p[0]).cross(p[2] - p[0]);
            let c = (p[0] + p[1] + p[2]) / 3.0;
            if prim < GRID_TRIS {
                assert!(n.y > 0.0, "grid triangle {prim} faces down");
            } else {
                // Skirts face away from the node centre (horizontally).
                let centre = Vec3::from_array([d.origin[0], d.origin[1], d.origin[2]]);
                let out = (c - centre).with_y(0.0);
                assert!(n.with_y(0.0).dot(out) > 0.0, "skirt triangle {prim} faces inward");
            }
        }
    }

    #[test]
    fn gpu_vertex_math_matches_the_radial_rule() {
        let body = earth();
        for level in [0u8, 3, 8, 12, 16, 20] {
            let id = zermatt_node(level);
            let page = page_fn(hills, id);
            let (pose, _) = pose_at(&body, id);
            // Camera a little above the node's corner (f64 truth).
            let eye_b = body.point(id.grid_point(3, 5, 64), 2000.0);
            let eye = pose.rotation * eye_b + pose.position;
            let d = node_draw(id, &body, &pose, eye, [[f32::MAX; 2]; 2], false, (0, &page), None, 0);
            let pg = DrawPages { own: &page, parent: None };
            // f32 error: under 1 cm plus 1e-6 of the distance (sub-pixel) plus 1e-7 of the
            // node's size (the offset from the node origin is f32).
            let edge = body.equatorial_m * std::f64::consts::FRAC_PI_2 / id.side() as f64;
            for (i, j) in [(0, 0), (64, 64), (0, 64), (32, 32), (17, 45), (63, 1), (5, 5)] {
                let h = page.height(4 * i as usize, 4 * j as usize);
                let truth = pose.rotation * body.point(id.grid_point(i, j, 64), h) + pose.position - eye;
                let got = cpu_vertex(&d, &pg, j * SIDE + i).as_dvec3();
                let err = (got - truth).length();
                assert!(err < 0.01 + 1e-6 * truth.length() + 1e-7 * edge, "level {level} ({i},{j}): {got:?} vs {truth:?} ({err} m)");
            }
        }
    }

    #[test]
    fn full_morph_is_the_parent_mesh_and_neighbours_share_edges() {
        let body = earth();
        let child = zermatt_node(13);
        let parent = child.parent().unwrap();
        let (pose, _) = pose_at(&body, child);
        let eye = pose.position + pose.rotation * body.point(child.centre(), 5000.0);
        let (cp, pp) = (page_fn(hills, child), page_fn(hills, parent));
        // Morph complete everywhere (band ends at 0).
        let d = node_draw(child, &body, &pose, eye, [[0.0, 1e-3], [f32::MAX; 2]], false, (0, &cp), Some((1, &pp)), 0);
        let dp = node_draw(parent, &body, &pose, eye, [[f32::MAX; 2]; 2], false, (1, &pp), None, 0);
        let pg = DrawPages { own: &cp, parent: Some(&pp) };
        let ppg = DrawPages { own: &pp, parent: None };
        let (qx, qy) = (child.x() & 1, child.y() & 1);
        // Parent vertex (pi, pj) and the parent surface on its 00-11 diagonal.
        let pv = |pi: u32, pj: u32| cpu_vertex(&dp, &ppg, pj * SIDE + pi);
        for (i, j) in [(2u32, 2u32), (3, 2), (2, 3), (3, 3), (40, 17), (41, 18), (63, 63), (0, 31), (31, 0)] {
            let got = cpu_vertex(&d, &pg, j * SIDE + i);
            let (pi, pj) = (qx * 32 + i / 2, qy * 32 + j / 2);
            let want = match (i % 2, j % 2) {
                (0, 0) => pv(pi, pj),
                (1, 0) => 0.5 * (pv(pi, pj) + pv(pi + 1, pj)),
                (0, 1) => 0.5 * (pv(pi, pj) + pv(pi, pj + 1)),
                _ => 0.5 * (pv(pi, pj) + pv(pi + 1, pj + 1)),
            };
            assert!((got - want).length() < 0.02, "({i},{j}) {got:?} vs {want:?}");
        }
    }

    #[test]
    fn coarse_edges_follow_the_coarser_neighbour() {
        let body = earth();
        let f = zermatt_node(12);
        // A left child (even x), so its left neighbour has another parent.
        let f = if f.x() & 1 == 1 { f.neighbour(1, 0) } else { f };
        // A node whose left (i = 0) neighbour is drawn one level coarser.
        let left = f.neighbour(-1, 0);
        let c = left.parent().unwrap();
        assert!(!c.contains(f));
        let mut drawn = BTreeSet::new();
        drawn.insert(f);
        drawn.insert(c);
        assert_eq!(coarse_mask(f, &drawn) & 1, 1);
        let (pose, _) = pose_at(&body, f);
        let eye = pose.position + pose.rotation * body.point(f.centre(), 3000.0);
        let k = 1440.0 / (2.0 * (0.5f64).tan());
        let cfg = SelectConfig::default();
        let bands = |l: u8| [morph_band(&body, l, &cfg, k), morph_band(&body, l - 1, &cfg, k)];
        let (fp, cpg) = (page_fn(hills, f), page_fn(hills, c));
        let df = node_draw(f, &body, &pose, eye, bands(12), false, (0, &fp), None, coarse_mask(f, &drawn));
        let dc = node_draw(c, &body, &pose, eye, bands(11), false, (1, &cpg), None, 0);
        let (pf, pc) = (DrawPages { own: &fp, parent: None }, DrawPages { own: &cpg, parent: None });
        // Points of the coarse node's lattice along the shared edge, as segments.
        let cverts: Vec<Vec3> = (0..=64).flat_map(|j| (0..=64).map(move |i| (i, j))).map(|(i, j)| cpu_vertex(&dc, &pc, j * SIDE + i)).collect();
        for e in 0..=64 {
            let p = cpu_vertex(&df, &pf, e * SIDE);
            // Distance from p to the closest coarse edge segment.
            let mut best = f32::MAX;
            for a in 0..65usize {
                for b in [a + 1, a + SIDE as usize] {
                    if b >= cverts.len() {
                        continue;
                    }
                    let (s0, s1) = (cverts[a], cverts[b]);
                    let t = ((p - s0).dot(s1 - s0) / (s1 - s0).length_squared()).clamp(0.0, 1.0);
                    best = best.min((s0 + (s1 - s0) * t - p).length());
                }
            }
            // The shared edge is one of the coarse node's borders; check every border.
            let mut border = f32::MAX;
            for edge in 0..4u32 {
                for s in 0..64u32 {
                    let (a0, a1) = (edge_at(edge, s), edge_at(edge, s + 1));
                    let (s0, s1) = (cverts[(a0.1 * SIDE + a0.0) as usize], cverts[(a1.1 * SIDE + a1.0) as usize]);
                    let t = ((p - s0).dot(s1 - s0) / (s1 - s0).length_squared()).clamp(0.0, 1.0);
                    border = border.min((s0 + (s1 - s0) * t - p).length());
                }
            }
            let _ = best;
            assert!(border < 0.02, "fine edge vertex {e} is {border} m off the coarse edge");
        }
    }
}
